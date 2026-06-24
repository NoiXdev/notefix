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
        if let Some(c) = regex::Regex::new(&p).ok().and_then(|re| re.captures(html)).and_then(|c| c.get(1)) {
            let v = c.as_str().trim();
            if !v.is_empty() { return Some(html_unescape(v)); }
        }
    }
    None
}

fn title_tag(html: &str) -> Option<String> {
    regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()
        .and_then(|re| re.captures(html)).and_then(|c| c.get(1))
        .map(|m| html_unescape(m.as_str().trim()))
        .filter(|s| !s.is_empty())
}

fn host(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("").trim_start_matches("www.").to_string()
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'")
}

pub fn parse_og(html: &str, url: &str) -> LinkMeta {
    LinkMeta {
        url: url.to_string(),
        title: meta_content(html, "og:title").or_else(|| title_tag(html)).unwrap_or_default(),
        description: meta_content(html, "og:description").or_else(|| meta_content(html, "description")).unwrap_or_default(),
        image: meta_content(html, "og:image").unwrap_or_default(),
        site: meta_content(html, "og:site_name").unwrap_or_else(|| host(url)),
    }
}

#[tauri::command]
pub async fn fetch_link_meta(url: String) -> Result<LinkMeta, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("invalid url".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("NotefixBot/1.0 (+link-preview)")
        .build()
        .map_err(|e| e.to_string())?;
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
}
