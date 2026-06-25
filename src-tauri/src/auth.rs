// src-tauri/src/auth.rs
//
// A1 native auth bridge. PKCE helpers, OAuth discovery + authorization-code
// token exchange against a notefix-server, and token storage in the OS
// keychain via the `keyring` crate. The browser flow itself lives in the
// `server_auth_*` commands (Task 4); this module is the pure plumbing.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KEYCHAIN_SERVICE: &str = "dev.noidee.notefix";

/// The native redirect URI registered with the deep-link scheme. The server's
/// public desktop client is created with exactly this redirect.
pub const REDIRECT_URI: &str = "notefix://auth";

/// PKCE material for one authorization-code flow.
pub struct Pkce {
    pub verifier: String,
    /// base64url(sha256(verifier)), unpadded — the S256 code challenge.
    pub challenge: String,
    /// Opaque CSRF value echoed back on the redirect.
    pub state: String,
}

fn random_b64url(n_bytes: usize) -> String {
    use rand::RngCore;
    let mut bytes = vec![0u8; n_bytes];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Generate a PKCE verifier/challenge (S256) and an opaque state value.
pub fn pkce() -> Pkce {
    // 32 random bytes → 43 base64url chars, within the spec's 43..=128 range.
    let verifier = random_b64url(32);
    let challenge =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_b64url(16);
    Pkce { verifier, challenge, state }
}

/// OAuth discovery payload from `GET /api/oauth/config`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Tokens returned by the token endpoint and stored in the keychain.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub token_type: String,
}

/// Strip a trailing slash so `/api/...` paths join predictably.
pub fn normalize_server_url(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// Discover the server's OAuth endpoints + public client id.
pub async fn fetch_oauth_config(server_url: &str) -> Result<OAuthConfig, String> {
    let url = format!("{}/api/oauth/config", normalize_server_url(server_url));
    let resp = http_client()?.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("oauth config: HTTP {}", resp.status().as_u16()));
    }
    resp.json::<OAuthConfig>().await.map_err(|e| e.to_string())
}

/// Exchange an authorization code (+ PKCE verifier) for tokens.
pub async fn exchange_code(
    token_url: &str,
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<Tokens, String> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("code", code)
        .append_pair("code_verifier", verifier)
        .finish();
    let resp = http_client()?
        .post(token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("token exchange: HTTP {}", resp.status().as_u16()));
    }
    resp.json::<Tokens>().await.map_err(|e| e.to_string())
}

// --- keychain storage -------------------------------------------------------

fn entry(context_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, &format!("server-tokens:{context_id}"))
        .map_err(|e| e.to_string())
}

pub fn store_tokens(context_id: &str, tokens: &Tokens) -> Result<(), String> {
    let json = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    entry(context_id)?.set_password(&json).map_err(|e| e.to_string())
}

pub fn load_tokens(context_id: &str) -> Result<Option<Tokens>, String> {
    match entry(context_id)?.get_password() {
        Ok(json) => serde_json::from_str::<Tokens>(&json).map(Some).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn clear_tokens(context_id: &str) -> Result<(), String> {
    match entry(context_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_s256_of_verifier_urlsafe_unpadded() {
        let p = pkce();
        let expect = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(p.verifier.as_bytes()));
        assert_eq!(p.challenge, expect);
        // S256: sha256 → 32 bytes → 43 unpadded base64url chars, no +/=
        assert_eq!(p.challenge.len(), 43);
        assert!(!p.challenge.contains('=') && !p.challenge.contains('+') && !p.challenge.contains('/'));
    }

    #[test]
    fn verifier_is_urlsafe_and_in_spec_length() {
        let p = pkce();
        assert!(p.verifier.len() >= 43 && p.verifier.len() <= 128);
        assert!(p
            .verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn state_and_verifier_are_random_per_call() {
        let a = pkce();
        let b = pkce();
        assert!(!a.state.is_empty());
        assert_ne!(a.state, b.state);
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(normalize_server_url("https://x.test/"), "https://x.test");
        assert_eq!(normalize_server_url("https://x.test"), "https://x.test");
    }
}
