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

/// The latest-release endpoint URL. Pulled out so it's testable without a
/// network call.
fn latest_release_url() -> String {
    format!("https://api.github.com/repos/{REPO}/releases/latest")
}

/// Build the `UpdateInfo` result from GitHub's `releases/latest` JSON body,
/// given the running app's version. Pulled out of [`check_for_update`] so the
/// parsing/error-classification logic is testable without a network call.
fn build_update_info(current: String, body: &serde_json::Value) -> Result<UpdateInfo, String> {
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

#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Notefix (update-check)")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(latest_release_url())
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    build_update_info(current, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn latest_release_url_is_well_formed() {
        assert_eq!(
            latest_release_url(),
            "https://api.github.com/repos/NoiXdev/notefix/releases/latest"
        );
    }

    #[test]
    fn build_update_info_flags_available_update() {
        let body = json!({
            "tag_name": "v0.8.0",
            "html_url": "https://github.com/NoiXdev/notefix/releases/tag/v0.8.0",
        });
        let info = build_update_info("0.7.0".to_string(), &body).unwrap();
        assert_eq!(info.current, "0.7.0");
        assert_eq!(info.latest, "v0.8.0");
        assert!(info.update_available);
        assert_eq!(
            info.url,
            "https://github.com/NoiXdev/notefix/releases/tag/v0.8.0"
        );
    }

    #[test]
    fn build_update_info_no_update_when_already_current() {
        let body = json!({ "tag_name": "v0.7.0" });
        let info = build_update_info("0.7.0".to_string(), &body).unwrap();
        assert!(!info.update_available);
        // No html_url in the body -> falls back to the static releases page.
        assert_eq!(info.url, RELEASES_URL);
    }

    #[test]
    fn build_update_info_missing_tag_is_error() {
        match build_update_info("0.7.0".to_string(), &json!({})) {
            Err(msg) => assert_eq!(msg, "no release tag found"),
            Ok(_) => panic!("expected an error for a body with no tag_name"),
        }
    }
}
