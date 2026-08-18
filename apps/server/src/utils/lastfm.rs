//! Last.fm scrobbling client.
//!
//! The shared secret never leaves the server: every signed call happens here.
//! The web player detects scrobble timing and posts play events; this module
//! signs them (`api_sig` = md5 of the alphabetically-sorted params concatenated
//! as `key value ... secret`) and calls the Last.fm API.
//!
//! Auth uses the desktop token flow (`auth.getToken` -> user approves the
//! returned URL -> `auth.getSession`), so no callback URL has to be registered.
//! The resulting session key does not expire and is stored beside the database.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use config::Config;
use serde::{Deserialize, Serialize};
use shared::{errors::Error, http::HttpClientBuilder, types::SoundgnomeResult};

const API_ROOT: &str = "https://ws.audioscrobbler.com/2.0/";
const AUTH_URL: &str = "https://www.last.fm/api/auth/";

// ── Credentials (API key + shared secret) ─────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub api_key: String,
    pub api_secret: String,
}

pub fn stored_credentials() -> Option<Credentials> {
    let raw = fs::read_to_string(Config::get().lastfm_credentials_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn store_credentials(api_key: &str, api_secret: &str) -> SoundgnomeResult<()> {
    let creds = Credentials {
        api_key: api_key.trim().to_string(),
        api_secret: api_secret.trim().to_string(),
    };
    if creds.api_key.is_empty() || creds.api_secret.is_empty() {
        return Err(Error::Custom(
            "Last.fm API key and secret are required".into(),
        ));
    }
    let json = serde_json::to_string(&creds)?;
    write_private(&Config::get().lastfm_credentials_path(), &json)
        .map_err(|e| Error::Custom(format!("Failed to store Last.fm credentials: {e}")))
}

pub fn clear_credentials() -> SoundgnomeResult<()> {
    remove_if_exists(&Config::get().lastfm_credentials_path())
}

fn creds_or_err() -> SoundgnomeResult<Credentials> {
    stored_credentials().ok_or_else(|| Error::Custom("Last.fm API credentials are not set".into()))
}

// ── Session (session key + username) ──────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub session_key: String,
    pub username: String,
}

pub fn stored_session() -> Option<Session> {
    let raw = fs::read_to_string(Config::get().lastfm_session_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn store_session(session: &Session) -> SoundgnomeResult<()> {
    let json = serde_json::to_string(session)?;
    write_private(&Config::get().lastfm_session_path(), &json)
        .map_err(|e| Error::Custom(format!("Failed to store Last.fm session: {e}")))
}

pub fn clear_session() -> SoundgnomeResult<()> {
    remove_if_exists(&Config::get().lastfm_session_path())
}

fn session_or_err() -> SoundgnomeResult<Session> {
    stored_session().ok_or_else(|| Error::Custom("Last.fm is not connected".into()))
}

// ── Signing ───────────────────────────────────────────────────────────────────

/// `api_sig` = md5(concat of sorted `key + value` pairs, then the shared secret).
/// `format` and `callback` are excluded from the signature by Last.fm's spec, so
/// they must never appear in `params`.
fn sign(params: &BTreeMap<String, String>, secret: &str) -> String {
    let mut buf = String::new();
    for (key, value) in params {
        buf.push_str(key);
        buf.push_str(value);
    }
    buf.push_str(secret);
    format!("{:x}", md5::compute(buf.as_bytes()))
}

// ── API calls ─────────────────────────────────────────────────────────────────

/// Last.fm returns HTTP 200 even for API errors, carrying `{error, message}`.
fn check_api_error(value: &serde_json::Value) -> SoundgnomeResult<()> {
    if let Some(code) = value.get("error").and_then(|e| e.as_u64()) {
        let message = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(Error::Custom(format!("Last.fm error {code}: {message}")));
    }
    Ok(())
}

/// `auth.getToken` — a request token the user then approves via [`authorize_url`].
pub async fn get_token() -> SoundgnomeResult<String> {
    let creds = creds_or_err()?;
    let mut params = BTreeMap::new();
    params.insert("api_key".to_string(), creds.api_key.clone());
    params.insert("method".to_string(), "auth.getToken".to_string());
    let sig = sign(&params, &creds.api_secret);

    let client = HttpClientBuilder::get_reqwest_client()?;
    let value: serde_json::Value = client
        .get(API_ROOT)
        .query(&[
            ("method", "auth.getToken"),
            ("api_key", creds.api_key.as_str()),
            ("api_sig", sig.as_str()),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| Error::Network(format!("Last.fm getToken request failed: {e}")))?
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Last.fm getToken parse failed: {e}")))?;

    check_api_error(&value)?;
    value
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Custom("Last.fm getToken returned no token".into()))
}

/// The URL the user opens to approve the token.
pub fn authorize_url(token: &str) -> SoundgnomeResult<String> {
    let creds = creds_or_err()?;
    Ok(format!(
        "{AUTH_URL}?api_key={}&token={}",
        creds.api_key, token
    ))
}

/// `auth.getSession` — exchange an approved token for a (non-expiring) session
/// key, which is then stored.
pub async fn get_session(token: &str) -> SoundgnomeResult<Session> {
    let creds = creds_or_err()?;
    let mut params = BTreeMap::new();
    params.insert("api_key".to_string(), creds.api_key.clone());
    params.insert("method".to_string(), "auth.getSession".to_string());
    params.insert("token".to_string(), token.to_string());
    let sig = sign(&params, &creds.api_secret);

    let client = HttpClientBuilder::get_reqwest_client()?;
    let value: serde_json::Value = client
        .get(API_ROOT)
        .query(&[
            ("method", "auth.getSession"),
            ("api_key", creds.api_key.as_str()),
            ("token", token),
            ("api_sig", sig.as_str()),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| Error::Network(format!("Last.fm getSession request failed: {e}")))?
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Last.fm getSession parse failed: {e}")))?;

    check_api_error(&value)?;
    let session = value
        .get("session")
        .ok_or_else(|| Error::Custom("Last.fm getSession returned no session".into()))?;
    let key = session
        .get("key")
        .and_then(|k| k.as_str())
        .ok_or_else(|| Error::Custom("Last.fm session has no key".into()))?;
    let name = session
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    let session = Session {
        session_key: key.to_string(),
        username: name,
    };
    store_session(&session)?;
    Ok(session)
}

/// POST a signed write call (`params` excludes `format`/`api_sig`), returning the
/// parsed JSON so the caller can inspect it.
async fn post_signed(mut params: BTreeMap<String, String>) -> SoundgnomeResult<serde_json::Value> {
    let creds = creds_or_err()?;
    let sig = sign(&params, &creds.api_secret);
    params.insert("api_sig".to_string(), sig);
    params.insert("format".to_string(), "json".to_string());

    let form: Vec<(String, String)> = params.into_iter().collect();
    let client = HttpClientBuilder::get_reqwest_client()?;
    let value: serde_json::Value = client
        .post(API_ROOT)
        .form(&form)
        .send()
        .await
        .map_err(|e| Error::Network(format!("Last.fm request failed: {e}")))?
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Last.fm response parse failed: {e}")))?;
    check_api_error(&value)?;
    Ok(value)
}

/// `track.updateNowPlaying` — tell Last.fm what is playing right now.
pub async fn update_now_playing(
    artist: &str,
    track: &str,
    album: Option<&str>,
    duration: Option<u32>,
) -> SoundgnomeResult<()> {
    let session = session_or_err()?;
    let creds = creds_or_err()?;
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), "track.updateNowPlaying".to_string());
    params.insert("api_key".to_string(), creds.api_key);
    params.insert("sk".to_string(), session.session_key);
    params.insert("artist".to_string(), artist.to_string());
    params.insert("track".to_string(), track.to_string());
    if let Some(album) = album.filter(|a| !a.is_empty()) {
        params.insert("album".to_string(), album.to_string());
    }
    if let Some(duration) = duration.filter(|d| *d > 0) {
        params.insert("duration".to_string(), duration.to_string());
    }
    post_signed(params).await.map(|_| ())
}

/// A single play to record.
pub struct ScrobbleItem {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub duration: Option<u32>,
    /// UTC unix seconds when the track started playing.
    pub timestamp: i64,
}

/// `track.scrobble` — record up to 50 plays in one signed batch.
pub async fn scrobble(items: &[ScrobbleItem]) -> SoundgnomeResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    let session = session_or_err()?;
    let creds = creds_or_err()?;
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), "track.scrobble".to_string());
    params.insert("api_key".to_string(), creds.api_key);
    params.insert("sk".to_string(), session.session_key);
    for (i, item) in items.iter().enumerate() {
        params.insert(format!("artist[{i}]"), item.artist.clone());
        params.insert(format!("track[{i}]"), item.track.clone());
        params.insert(format!("timestamp[{i}]"), item.timestamp.to_string());
        if let Some(album) = item.album.as_deref().filter(|a| !a.is_empty()) {
            params.insert(format!("album[{i}]"), album.to_string());
        }
        if let Some(duration) = item.duration.filter(|d| *d > 0) {
            params.insert(format!("duration[{i}]"), duration.to_string());
        }
    }
    post_signed(params).await.map(|_| ())
}

// ── File helpers (owner-only, like the other stored credentials) ──────────────

fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents.as_bytes())
}

fn remove_if_exists(path: &Path) -> SoundgnomeResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Custom(format!("Failed to remove {path:?}: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_sig_sorts_then_appends_secret() {
        let mut params = BTreeMap::new();
        params.insert("method".to_string(), "auth.getToken".to_string());
        params.insert("api_key".to_string(), "KEY".to_string());
        // Sorted: api_key,KEY + method,auth.getToken, then secret SECRET.
        // md5("api_keyKEYmethodauth.getTokenSECRET")
        assert_eq!(sign(&params, "SECRET"), "66ee63a18da3c919f987b342697d913c");
    }
}
