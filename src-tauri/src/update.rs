// Lightweight update check: compare the running version against the latest
// published GitHub release. Notify only — no download or install.

use serde::Serialize;
use std::time::Duration;

const REPO: &str = "NoiXdev/notefix";
const RELEASES_URL: &str = "https://github.com/NoiXdev/notefix/releases/latest";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    pub url: String,
}

/// True when `latest` is a strictly higher version than `current`. Either may
/// carry a leading `v`; compared component-wise as dot-separated integers
/// (missing/garbage components count as 0).
pub fn is_newer(current: &str, latest: &str) -> bool {
    fn parts(s: &str) -> Vec<u64> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }
    let (c, l) = (parts(current), parts(latest));
    for i in 0..c.len().max(l.len()) {
        let (cv, lv) = (
            c.get(i).copied().unwrap_or(0),
            l.get(i).copied().unwrap_or(0),
        );
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Notefix (update-check)")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let latest = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if latest.is_empty() {
        return Err("no release tag found".into());
    }
    let url = body
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(RELEASES_URL)
        .to_string();

    Ok(UpdateInfo {
        update_available: is_newer(&current, &latest),
        current,
        latest,
        url,
    })
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn detects_newer_versions() {
        assert!(is_newer("0.1.2", "0.1.3"));
        assert!(is_newer("0.1.2", "v0.2.0"));
        assert!(is_newer("0.1.2", "1.0.0"));
        assert!(is_newer("0.1", "0.1.1"));
    }

    #[test]
    fn ignores_same_or_older() {
        assert!(!is_newer("0.1.2", "0.1.2"));
        assert!(!is_newer("0.1.2", "v0.1.2"));
        assert!(!is_newer("0.1.3", "0.1.2"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }
}
