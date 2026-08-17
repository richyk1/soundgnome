//! Spotify audio authentication for the librespot-backed downloader.
//!
//! librespot needs a Spotify **Premium** account and its own session, which is
//! separate from the metadata OAuth in `fetcher::spotify`: reading the public
//! catalogue (metadata) and pulling the encrypted audio stream are different
//! privileges. Only Spotify's own client id is granted the `streaming` scope,
//! so we cannot reuse the metadata login's client id; we run librespot's OAuth
//! flow against Spotify's desktop client id instead.
//!
//! Login is a paste-back OAuth flow. `begin_login` returns the authorize URL
//! and remembers the PKCE verifier; after approving, the browser lands on the
//! registered `127.0.0.1:8898` redirect (dead on a remote host), and the user
//! copies that URL into the UI, which calls `complete_login`. On success the
//! session hands back a long-lived reusable credentials blob, which
//! [`build_cache`] persists to `credentials.json`. Every later download loads
//! that blob and never prompts again. No local callback listener is used, so
//! this works over a tunnel or a remote/tailnet deployment.

use config::Config;
use librespot_core::{authentication::Credentials, cache::Cache, config::SessionConfig, Session};
use serde::{Deserialize, Serialize};
use shared::{errors::Error, http::HttpClientBuilder, types::SoundgnomeResult};
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::Mutex;

/// Spotify's own desktop client id. It is the only client whitelisted for the
/// `streaming` scope, and its registered loopback redirect is the one below.
const CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Loopback redirect registered for [`CLIENT_ID`] (the only redirect Spotify
/// accepts for it). The browser lands here after approval; the user copies the
/// resulting URL back into the UI (see [`complete_login`]).
const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";

/// The minimum scope that unlocks audio-key and file access.
// One approval covers audio and the library: `streaming` unlocks the audio key
// and file access, the rest lets the Web API list Liked Songs and playlists, so
// the user never registers an app or logs in twice.
const SCOPES: &[&str] = &["streaming", "user-library-read", "playlist-read-private"];

fn cache_dir() -> PathBuf {
    Config::get().librespot_cache_dir()
}

/// The credentials cache. Only the credentials path is set: we never keep a
/// local audio cache, downloads are streamed straight to the staging file.
pub fn build_cache() -> SoundgnomeResult<Cache> {
    Cache::new(Some(cache_dir()), None::<PathBuf>, None::<PathBuf>, None)
        .map_err(|e| Error::Custom(format!("librespot cache init failed: {e}")))
}

/// The one live librespot session, shared across every download.
///
/// librespot sessions are meant to be long-lived. Each [`Session::connect`] is
/// a full Access Point login handshake, so connecting once per track (as a bulk
/// Liked Songs sync does) trips Spotify's connection/auth rate limits and adds
/// a handshake of latency to every download. Downloads run serially (the task
/// executor is single-worker), so a single reused session is enough.
static SESSION: LazyLock<Mutex<Option<Session>>> = LazyLock::new(|| Mutex::new(None));

/// A connected librespot session, reused across downloads.
///
/// Reconnects only when there is no session yet or the previous one has been
/// invalidated. Errors when no credentials are cached, so callers can fall back
/// to the YouTube download path.
pub async fn session() -> SoundgnomeResult<Session> {
    let mut guard = SESSION.lock().await;

    if let Some(existing) = guard.as_ref() {
        if !existing.is_invalid() {
            return Ok(existing.clone());
        }
    }

    let cache = build_cache()?;
    let credentials = cache
        .credentials()
        .ok_or_else(|| Error::Custom("Spotify audio (librespot) is not connected".to_string()))?;

    // `store_credentials = false`: the reusable blob is already written at login
    // time, so a reconnect must not rewrite it on every call.
    let session = Session::new(SessionConfig::default(), Some(cache));
    session
        .connect(credentials, false)
        .await
        .map_err(|e| Error::Custom(format!("Spotify session connect failed: {e}")))?;

    *guard = Some(session.clone());
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

/// Register the librespot-backed Web API token minter with the fetcher, so its
/// Spotify catalogue and Liked Songs reads use a token minted from the live
/// session instead of a fragile OAuth refresh token (which Spotify revokes).
///
/// Uses the login5 token (the modern desktop-client token spclient itself
/// uses), not the legacy keymaster token, which 403s for `user-library-read`.
pub fn register_token_minter() {
    fetcher::spotify::session::set_token_minter(Box::new(|| {
        Box::pin(async {
            let session = session().await?;
            let token = session
                .login5()
                .auth_token()
                .await
                .map_err(|e| Error::Custom(format!("Spotify login5 token request failed: {e}")))?;
            Ok(fetcher::spotify::session::MintedToken {
                access_token: token.access_token,
                expires_in: token.expires_in.as_secs(),
            })
        })
    }));
}

/// A login in progress: the PKCE verifier and CSRF state, kept between the
/// authorize step and the paste-back step (two separate requests).
#[derive(Serialize, Deserialize)]
struct PendingLogin {
    verifier: String,
    state: String,
}

fn pending_path() -> PathBuf {
    cache_dir().join("pending_login.json")
}

fn store_pending(pending: &PendingLogin) -> SoundgnomeResult<()> {
    let path = pending_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Custom(format!("Cannot create {}: {e}", parent.display())))?;
    }
    let body = serde_json::to_string(pending)
        .map_err(|e| Error::Custom(format!("Cannot serialise the pending login: {e}")))?;
    std::fs::write(&path, body)
        .map_err(|e| Error::Custom(format!("Cannot store the pending login: {e}")))
}

fn read_pending() -> SoundgnomeResult<PendingLogin> {
    let raw = std::fs::read_to_string(pending_path())
        .map_err(|_| Error::Custom("No Spotify login is in progress. Start again.".to_string()))?;
    serde_json::from_str(&raw).map_err(|e| Error::Custom(format!("Unreadable pending login: {e}")))
}

/// Start the flow: remember the PKCE verifier and hand back the URL to approve.
///
/// No callback listener is run: the registered redirect points at the server
/// host's loopback, unreachable from a remote browser. The user pastes the
/// redirect URL back into the UI, which calls [`complete_login`].
pub fn begin_login() -> SoundgnomeResult<String> {
    let verifier = random_string(64);
    let challenge = pkce_challenge(&verifier);
    let state = random_string(16);

    store_pending(&PendingLogin {
        verifier,
        state: state.clone(),
    })?;

    Ok(authorize_url(&challenge, &state))
}

/// Finish the flow from the URL Spotify redirected the browser to (a full
/// `127.0.0.1:8898/login?code=...&state=...` address, or a bare code).
pub async fn complete_login(redirect: &str) -> SoundgnomeResult<String> {
    let redirect = redirect.trim();
    let params = query_params(redirect);

    if let Some(error) = params.get("error") {
        return Err(Error::Custom(format!("Spotify refused the login: {error}")));
    }

    let pending = read_pending()?;

    // Accept the full redirect URL or a pasted bare code.
    let code = match params.get("code") {
        Some(code) => code.clone(),
        None if !redirect.is_empty() && !redirect.contains('?') => redirect.to_string(),
        None => {
            return Err(Error::Custom(
                "No authorization code found. Paste the whole address bar after approving."
                    .to_string(),
            ))
        }
    };

    // The state is present only when the full URL is pasted; enforce it then.
    if let Some(state) = params.get("state") {
        if state != &pending.state {
            return Err(Error::Custom(
                "Login state did not match. Start the connection again.".to_string(),
            ));
        }
    }

    let username = finish_login(&code, &pending.verifier).await?;
    let _ = std::fs::remove_file(pending_path());
    Ok(username)
}

/// Exchange the code, store a reusable credentials blob, and keep the Web API
/// half of the same approval.
async fn finish_login(code: &str, verifier: &str) -> SoundgnomeResult<String> {
    let token = exchange_code(code, verifier).await?;

    // The same approval also covers the library scopes, so hand the tokens to
    // the Web API session store. Without this the user would have to register
    // an app and log in a second time just to list their Liked Songs.
    if let Some(refresh_token) = token.refresh_token.clone() {
        if let Err(e) = fetcher::spotify::session::store_user_token(
            token.access_token.clone(),
            refresh_token,
            token.expires_in,
            CLIENT_ID.to_string(),
        )
        .await
        {
            // Audio still works without it, so this is a warning, not a failure.
            tracing::warn!("Could not store the Spotify Web API session: {e}");
        }
    }

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

fn random_string(len: usize) -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn authorize_url(challenge: &str, state: &str) -> String {
    let query = [
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
        ("scope", &SCOPES.join(" ")),
        ("state", state),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", k, urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");

    format!("https://accounts.spotify.com/authorize?{query}")
}

fn query_params(target: &str) -> std::collections::HashMap<String, String> {
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or_default();
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), percent_decode(v)))
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Trade the authorization code for an access token.
async fn exchange_code(code: &str, verifier: &str) -> SoundgnomeResult<TokenResponse> {
    let response = HttpClientBuilder::get_reqwest_client()?
        .post("https://accounts.spotify.com/api/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Spotify token exchange failed: {e}")))?;

    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Unreadable Spotify token response: {e}")))?;

    if !status.is_success() {
        let detail = body
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(Error::Custom(format!(
            "Spotify refused the login: {detail}"
        )));
    }

    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| Error::Custom("Spotify returned no access token".to_string()))?;

    Ok(TokenResponse {
        access_token,
        // Present for the authorization code flow. Absent only if Spotify
        // changes the contract, in which case the Web API half is skipped.
        refresh_token: body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        expires_in: body
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600),
    })
}

/// What Spotify hands back for an authorization code.
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_the_rfc_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_carries_the_registered_redirect() {
        let url = authorize_url("chal", "st4te");
        assert!(url.contains("client_id=65b708073fc0480ea92a077233ca87bd"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8898%2Flogin"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("scope=streaming"));
    }

    #[test]
    fn parses_a_callback_target() {
        let params = query_params("/login?code=abc%2D123&state=xyz");
        assert_eq!(params.get("code").map(String::as_str), Some("abc-123"));
        assert_eq!(params.get("state").map(String::as_str), Some("xyz"));
        assert!(query_params("/login").is_empty(), "no query, no params");
    }
}

/// Forget the stored credentials. Succeeds when there was nothing to remove.
pub fn clear() -> SoundgnomeResult<()> {
    // Drop the shared session too, so a later reconnect cannot reuse the
    // now-disconnected account. `try_lock` because this is a sync fn; if a
    // download holds the lock the stale session is invalidated on next use.
    if let Ok(mut guard) = SESSION.try_lock() {
        if let Some(session) = guard.take() {
            session.shutdown();
        }
    }

    match std::fs::remove_dir_all(cache_dir()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Custom(format!("Cannot clear librespot cache: {e}"))),
    }
}
