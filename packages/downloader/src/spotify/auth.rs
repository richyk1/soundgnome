//! Spotify audio authentication for the librespot-backed downloader.
//!
//! librespot needs a Spotify **Premium** account and its own session, which is
//! separate from the metadata OAuth in `fetcher::spotify`: reading the public
//! catalogue (metadata) and pulling the encrypted audio stream are different
//! privileges. Only Spotify's own client id is granted the `streaming` scope,
//! so we cannot reuse the metadata login's client id; we run librespot's OAuth
//! flow against Spotify's desktop client id instead.
//!
//! The first login runs the interactive OAuth flow (opens the browser on the
//! server host and catches the redirect on a local listener). On success the
//! session hands back a long-lived reusable credentials blob, which
//! [`build_cache`] persists to `credentials.json`. Every later download loads
//! that blob and never prompts again.

use config::Config;
use librespot_core::{authentication::Credentials, cache::Cache, config::SessionConfig, Session};
use shared::{errors::Error, types::SoundomeResult};
use std::path::PathBuf;

/// Spotify's own desktop client id. It is the only client whitelisted for the
/// `streaming` scope, and its registered loopback redirect is the one below.
const CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Loopback redirect registered for [`CLIENT_ID`]. librespot binds a one-shot
/// listener on this host:port to catch the authorization code, so the browser
/// completing the login must be able to reach the server host on port 8898.
const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";

/// The minimum scope that unlocks audio-key and file access.
const SCOPES: &[&str] = &["streaming"];

fn cache_dir() -> PathBuf {
    Config::get().librespot_cache_dir()
}

/// The credentials cache. Only the credentials path is set: we never keep a
/// local audio cache, downloads are streamed straight to the staging file.
pub fn build_cache() -> SoundomeResult<Cache> {
    Cache::new(Some(cache_dir()), None::<PathBuf>, None::<PathBuf>, None)
        .map_err(|e| Error::Custom(format!("librespot cache init failed: {e}")))
}

/// A ready-to-use session built from the persisted credentials blob.
///
/// Errors when no credentials are cached yet, so callers can fall back to the
/// YouTube download path.
pub async fn connect_session() -> SoundomeResult<Session> {
    let cache = build_cache()?;
    let credentials = cache
        .credentials()
        .ok_or_else(|| Error::Custom("Spotify audio (librespot) is not connected".to_string()))?;

    let session = Session::new(SessionConfig::default(), Some(cache));
    session
        .connect(credentials, true)
        .await
        .map_err(|e| Error::Custom(format!("Spotify session connect failed: {e}")))?;
    Ok(session)
}

/// True when a reusable credentials blob is on disk.
pub fn is_connected() -> bool {
    build_cache()
        .ok()
        .and_then(|cache| cache.credentials())
        .is_some()
}

/// The account handle the stored credentials belong to, when known.
pub fn connected_username() -> Option<String> {
    build_cache()
        .ok()
        .and_then(|cache| cache.credentials())
        .and_then(|credentials| credentials.username)
}

/// Run the interactive OAuth flow, then persist a reusable credentials blob.
///
/// Blocks until the operator completes the login in their browser. The OAuth
/// listener step uses a blocking `TcpListener`, so it runs on a blocking thread
/// to keep the async runtime free.
pub async fn login() -> SoundomeResult<String> {
    let token = tokio::task::spawn_blocking(|| {
        librespot_oauth::OAuthClientBuilder::new(CLIENT_ID, REDIRECT_URI, SCOPES.to_vec())
            .open_in_browser()
            .build()
            .map_err(|e| Error::Custom(format!("librespot OAuth setup failed: {e}")))?
            .get_access_token()
            .map_err(|e| Error::Custom(format!("Spotify authorization failed: {e}")))
    })
    .await
    .map_err(|e| Error::Custom(format!("OAuth task failed: {e}")))??;

    // `connect` with `store_credentials = true` swaps the short-lived access
    // token for a reusable blob and writes it into the cache.
    let cache = build_cache()?;
    let session = Session::new(SessionConfig::default(), Some(cache));
    session
        .connect(Credentials::with_access_token(token.access_token), true)
        .await
        .map_err(|e| Error::Custom(format!("Spotify session connect failed: {e}")))?;

    Ok(session.username())
}

/// Forget the stored credentials. Succeeds when there was nothing to remove.
pub fn clear() -> SoundomeResult<()> {
    match std::fs::remove_dir_all(cache_dir()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Custom(format!("Cannot clear librespot cache: {e}"))),
    }
}
