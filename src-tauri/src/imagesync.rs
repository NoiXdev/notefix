// src-tauri/src/imagesync.rs
//
// S2b image sync. Pure path-set diff + thin HTTP against the server's image
// endpoints. Orchestration (collect referenced, filesystem, lock handling) lives
// in commands.rs::run_image_phase (next task). Path-addressed: a relpath is
// immutable bytes, so transfer is decided by path presence alone.

use std::collections::HashSet;

use serde_json::Value;

/// Referenced+present-locally paths the server does not have yet → upload.
pub fn to_upload(local: &HashSet<String>, server: &HashSet<String>) -> Vec<String> {
    local.difference(server).cloned().collect()
}

/// Referenced paths that exist on the server but not locally → download.
pub fn to_download(
    referenced: &HashSet<String>,
    local: &HashSet<String>,
    server: &HashSet<String>,
) -> Vec<String> {
    referenced
        .iter()
        .filter(|p| server.contains(*p) && !local.contains(*p))
        .cloned()
        .collect()
}

fn base(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

/// GET the manifest → the set of server-held image relpaths.
pub async fn fetch_manifest(
    server_url: &str,
    token: &str,
    workspace_id: &str,
) -> Result<HashSet<String>, String> {
    let url = format!(
        "{}/api/workspaces/{}/images",
        base(server_url),
        workspace_id
    );
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("manifest HTTP {}", resp.status().as_u16()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect())
}

/// POST one image (multipart path + file). `mime` must be the real image mime
/// so the server's `image` validation rule passes.
pub async fn upload_image(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    path: &str,
    bytes: Vec<u8>,
    mime: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/api/workspaces/{}/images",
        base(server_url),
        workspace_id
    );
    let file_name = path.rsplit('/').next().unwrap_or("image").to_string();
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime)
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("path", path.to_string())
        .part("file", part);
    let resp = client()?
        .post(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("upload HTTP {}", resp.status().as_u16()));
    }
    Ok(())
}

/// GET one image's bytes by path.
pub async fn download_image(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    path: &str,
) -> Result<Vec<u8>, String> {
    let url = format!("{}/api/workspaces/{}/image", base(server_url), workspace_id);
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .query(&[("path", path)])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status().as_u16()));
    }
    Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn upload_is_local_minus_server() {
        let local = set(&["a/x.png", "b/y.png", "c/z.png"]);
        let server = set(&["b/y.png"]);
        let mut up = to_upload(&local, &server);
        up.sort();
        assert_eq!(up, vec!["a/x.png".to_string(), "c/z.png".to_string()]);
    }

    #[test]
    fn download_is_referenced_on_server_minus_local() {
        let referenced = set(&["a/x.png", "b/y.png", "d/w.png"]);
        let local = set(&["a/x.png"]);
        let server = set(&["a/x.png", "b/y.png"]); // d/w.png not on server → not downloaded
        let mut dl = to_download(&referenced, &local, &server);
        dl.sort();
        assert_eq!(dl, vec!["b/y.png".to_string()]);
    }

    #[test]
    fn empty_sets_yield_nothing() {
        assert!(to_upload(&set(&[]), &set(&[])).is_empty());
        assert!(to_download(&set(&[]), &set(&[]), &set(&[])).is_empty());
    }
}
