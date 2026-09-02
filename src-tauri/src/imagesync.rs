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

fn manifest_url(server_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/workspaces/{}/images",
        base(server_url),
        workspace_id
    )
}

/// Extract the server-held image relpaths from the manifest endpoint's JSON
/// body. Pulled out of [`fetch_manifest`] so the response-parsing logic is
/// testable without a network call.
fn parse_manifest(body: &Value) -> HashSet<String> {
    body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect()
}

/// GET the manifest → the set of server-held image relpaths.
pub async fn fetch_manifest(
    server_url: &str,
    token: &str,
    workspace_id: &str,
) -> Result<HashSet<String>, String> {
    let resp = client()?
        .get(manifest_url(server_url, workspace_id))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("manifest HTTP {}", resp.status().as_u16()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parse_manifest(&body))
}

fn upload_url(server_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/workspaces/{}/images",
        base(server_url),
        workspace_id
    )
}

/// The multipart file name for an uploaded image: the last `/`-separated
/// segment of its relpath. Pulled out of [`upload_image`] so it's testable
/// without a network call.
fn file_name_from_path(path: &str) -> String {
    path.rsplit('/').next().unwrap_or("image").to_string()
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
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name_from_path(path))
        .mime_str(mime)
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("path", path.to_string())
        .part("file", part);
    let resp = client()?
        .post(upload_url(server_url, workspace_id))
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

fn download_url(server_url: &str, workspace_id: &str) -> String {
    format!("{}/api/workspaces/{}/image", base(server_url), workspace_id)
}

/// GET one image's bytes by path.
pub async fn download_image(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    path: &str,
) -> Result<Vec<u8>, String> {
    let resp = client()?
        .get(download_url(server_url, workspace_id))
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
    use serde_json::json;

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

    #[test]
    fn client_builds_successfully() {
        assert!(client().is_ok());
    }

    #[test]
    fn base_trims_trailing_slash() {
        assert_eq!(base("https://sync.test/"), "https://sync.test");
        assert_eq!(base("https://sync.test"), "https://sync.test");
    }

    #[test]
    fn manifest_upload_and_download_urls_are_well_formed() {
        assert_eq!(
            manifest_url("https://sync.test/", "w1"),
            "https://sync.test/api/workspaces/w1/images"
        );
        assert_eq!(
            upload_url("https://sync.test", "w1"),
            "https://sync.test/api/workspaces/w1/images"
        );
        assert_eq!(
            download_url("https://sync.test", "w1"),
            "https://sync.test/api/workspaces/w1/image"
        );
    }

    #[test]
    fn parse_manifest_extracts_paths_and_ignores_entries_without_one() {
        let body = json!({"data": [
            {"path": "a/x.png"},
            {"path": "b/y.png"},
            {"no_path": true}
        ]});
        assert_eq!(parse_manifest(&body), set(&["a/x.png", "b/y.png"]));
    }

    #[test]
    fn parse_manifest_empty_when_data_field_missing() {
        assert!(parse_manifest(&json!({})).is_empty());
    }

    #[test]
    fn file_name_from_path_takes_the_last_segment() {
        assert_eq!(file_name_from_path("a/b/c.png"), "c.png");
        assert_eq!(file_name_from_path("solo.png"), "solo.png");
    }
}
