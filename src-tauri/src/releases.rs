// "What's New" changelog: fetch the published GitHub releases for the app
// repo. Read-only, unauthenticated. The frontend decides which releases are
// new since the version the user last saw (see src/version.ts).

use serde::{Deserialize, Serialize};
use std::time::Duration;

const REPO: &str = "NoiXdev/notefix";

/// One GitHub release, as needed by the frontend. Deserialized straight from
/// GitHub's snake_case JSON (field names already match); serialized back out
/// to the webview as camelCase.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ReleaseInfo {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub body: String,
    pub published_at: String,
    #[serde(default)]
    pub prerelease: bool,
}

/// Fetch every published release for the app repo, newest first (as GitHub
/// returns them). Any network/parse failure becomes an `Err(message)` — the
/// frontend handles that gracefully (it just skips showing the dialog).
#[tauri::command]
pub async fn github_releases() -> Result<Vec<ReleaseInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("notefix")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=30"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }

    resp.json::<Vec<ReleaseInfo>>()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // A trimmed-down sample of what GitHub's releases API actually returns,
    // including fields we don't model (url, html_url, draft, author, …) to
    // confirm those are simply ignored.
    fn sample_json() -> String {
        json!([
            {
                "url": "https://api.github.com/repos/NoiXdev/notefix/releases/1",
                "html_url": "https://github.com/NoiXdev/notefix/releases/tag/v0.6.0",
                "tag_name": "v0.6.0",
                "name": "v0.6.0",
                "body": "### Added\n- Apps page\n\n### Fixed\n- Sync race",
                "published_at": "2026-08-20T10:00:00Z",
                "prerelease": false,
                "draft": false,
                "author": { "login": "noidee" }
            },
            {
                "tag_name": "v0.5.1",
                "name": "",
                "body": "- Minor fixes",
                "published_at": "2026-07-01T08:30:00Z",
                "prerelease": true
            }
        ])
        .to_string()
    }

    #[test]
    fn deserializes_github_releases_json() {
        let releases: Vec<ReleaseInfo> = serde_json::from_str(&sample_json()).expect("valid JSON");
        assert_eq!(releases.len(), 2);

        assert_eq!(releases[0].tag_name, "v0.6.0");
        assert_eq!(releases[0].name, "v0.6.0");
        assert_eq!(
            releases[0].body,
            "### Added\n- Apps page\n\n### Fixed\n- Sync race"
        );
        assert_eq!(releases[0].published_at, "2026-08-20T10:00:00Z");
        assert!(!releases[0].prerelease);

        assert_eq!(releases[1].tag_name, "v0.5.1");
        assert_eq!(releases[1].name, "");
        assert!(releases[1].prerelease);
    }

    #[test]
    fn serializes_to_camel_case_for_the_frontend() {
        let releases: Vec<ReleaseInfo> = serde_json::from_str(&sample_json()).expect("valid JSON");
        let out = serde_json::to_value(&releases[0]).expect("serializable");
        assert_eq!(out["tagName"], "v0.6.0");
        assert_eq!(out["publishedAt"], "2026-08-20T10:00:00Z");
        assert!(out.get("tag_name").is_none());
        assert!(out.get("published_at").is_none());
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let minimal =
            json!([{"tag_name": "v1.0.0", "published_at": "2026-01-01T00:00:00Z"}]).to_string();
        let releases: Vec<ReleaseInfo> = serde_json::from_str(&minimal).expect("valid JSON");
        assert_eq!(releases[0].name, "");
        assert_eq!(releases[0].body, "");
        assert!(!releases[0].prerelease);
    }
}
