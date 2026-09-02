use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LinkMeta {
    pub url: String,
    pub title: String,
    pub description: String,
    pub image: String,
    pub site: String,
}

fn meta_content(html: &str, key: &str) -> Option<String> {
    let k = regex::escape(key);
    // property/name=key … content="…"  (beide Attribut-Reihenfolgen)
    let pats = [
        format!(r#"<meta[^>]+(?:property|name)=["']{k}["'][^>]+content=["']([^"']*)["']"#),
        format!(r#"<meta[^>]+content=["']([^"']*)["'][^>]+(?:property|name)=["']{k}["']"#),
    ];
    for p in pats {
        if let Some(c) = regex::Regex::new(&p)
            .ok()
            .and_then(|re| re.captures(html))
            .and_then(|c| c.get(1))
        {
            let v = c.as_str().trim();
            if !v.is_empty() {
                return Some(html_unescape(v));
            }
        }
    }
    None
}

fn title_tag(html: &str) -> Option<String> {
    regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .ok()
        .and_then(|re| re.captures(html))
        .and_then(|c| c.get(1))
        .map(|m| html_unescape(m.as_str().trim()))
        .filter(|s| !s.is_empty())
}

fn host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_string()
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

pub fn parse_og(html: &str, url: &str) -> LinkMeta {
    LinkMeta {
        url: url.to_string(),
        title: meta_content(html, "og:title")
            .or_else(|| title_tag(html))
            .unwrap_or_default(),
        description: meta_content(html, "og:description")
            .or_else(|| meta_content(html, "description"))
            .unwrap_or_default(),
        image: meta_content(html, "og:image").unwrap_or_default(),
        site: meta_content(html, "og:site_name").unwrap_or_else(|| host(url)),
    }
}

/// Only `http(s)://` URLs are fetched — pulled out of [`fetch_link_meta`] so
/// this guard is testable without a network call.
fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("NotefixBot/1.0 (+link-preview)")
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_link_meta(url: String) -> Result<LinkMeta, String> {
    if !is_http_url(&url) {
        return Err("invalid url".to_string());
    }
    let client = http_client()?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let html = resp.text().await.map_err(|e| e.to_string())?;
    Ok(parse_og(&html, &url))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_og_tags_and_falls_back_to_title() {
        let html = r#"<html><head><meta property="og:title" content="Hello"><meta property="og:description" content="Desc"><meta property="og:image" content="http://x/i.png"></head><body></body></html>"#;
        let m = parse_og(html, "https://www.example.com/p");
        assert_eq!(m.title, "Hello");
        assert_eq!(m.description, "Desc");
        assert_eq!(m.image, "http://x/i.png");
        assert_eq!(m.site, "example.com");
    }
    #[test]
    fn title_fallback_and_host_site() {
        let m = parse_og("<title>Just Title</title>", "https://foo.bar/x");
        assert_eq!(m.title, "Just Title");
        assert_eq!(m.site, "foo.bar");
    }
    #[test]
    fn empty_when_nothing() {
        let m = parse_og("<html></html>", "https://a.b");
        assert_eq!(m.title, "");
        assert_eq!(m.site, "a.b");
    }

    #[test]
    fn description_falls_back_to_plain_description_meta() {
        let html = r#"<meta name="description" content="Plain desc">"#;
        let m = parse_og(html, "https://a.b");
        assert_eq!(m.description, "Plain desc");
    }

    #[test]
    fn empty_meta_content_is_skipped_and_falls_back_to_title_tag() {
        let html = r#"<meta property="og:title" content=""><title>Fallback Title</title>"#;
        let m = parse_og(html, "https://a.b");
        assert_eq!(m.title, "Fallback Title");
    }

    #[test]
    fn meta_content_matches_reversed_attribute_order() {
        // content="…" before property="…" — the second regex pattern.
        let html = r#"<meta content="Reversed" property="og:title">"#;
        let m = parse_og(html, "https://a.b");
        assert_eq!(m.title, "Reversed");
    }

    #[test]
    fn html_unescape_decodes_common_entities() {
        let html = r#"<meta property="og:title" content="Tom &amp; Jerry &lt;3&gt; &quot;fun&quot; &#39;ok&#39;">"#;
        let m = parse_og(html, "https://a.b");
        assert_eq!(m.title, "Tom & Jerry <3> \"fun\" 'ok'");
    }

    #[test]
    fn http_client_builds_successfully() {
        assert!(http_client().is_ok());
    }

    #[test]
    fn is_http_url_accepts_http_and_https_only() {
        assert!(is_http_url("http://example.com"));
        assert!(is_http_url("https://example.com"));
        assert!(!is_http_url("ftp://example.com"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url(""));
    }

    #[tokio::test]
    async fn fetch_link_meta_rejects_non_http_urls_without_a_network_call() {
        // The scheme guard returns before the first `.await`, so this
        // exercises the command end-to-end with no real network access.
        match fetch_link_meta("javascript:alert(1)".to_string()).await {
            Err(msg) => assert_eq!(msg, "invalid url"),
            Ok(_) => panic!("non-http scheme must be rejected"),
        }
    }
}
