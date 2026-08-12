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
use shared::{errors::Error, http::HttpClientBuilder, types::SoundomeResult};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

/// Start the flow: hand back the URL to approve, and wait for the callback in
/// the background.
///
/// librespot's own OAuth helper only prints the authorization URL and then
/// blocks, which forces the operator to read a server log. Doing the PKCE
/// exchange here instead lets the UI open the link itself, so connecting is a
/// button press rather than a terminal session.
pub async fn begin_login() -> SoundomeResult<String> {
    let verifier = random_string(64);
    let challenge = pkce_challenge(&verifier);
    let state = random_string(16);

    // Bind before returning: if the port is busy the caller must hear about it
    // now, not after the user has already approved in Spotify.
    let listener = TcpListener::bind(LISTEN_ADDR).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            Error::Custom(format!(
                "Another authorization is already waiting on {LISTEN_ADDR}. \
                 Finish that one, or wait {CALLBACK_TIMEOUT_SECS}s for it to expire."
            ))
        } else {
            Error::Custom(format!("Cannot listen on {LISTEN_ADDR}: {e}"))
        }
    })?;

    let url = authorize_url(&challenge, &state);
    tokio::spawn(async move {
        match await_callback(listener, &state).await {
            Ok(code) => match finish_login(&code, &verifier).await {
                Ok(username) => {
                    tracing::info!("Spotify audio (librespot) connected for {username}")
                }
                Err(e) => tracing::error!("Spotify audio login failed: {e}"),
            },
            Err(e) => tracing::error!("Spotify audio authorization failed: {e}"),
        }
    });

    Ok(url)
}

/// Exchange the code, then store a reusable credentials blob.
async fn finish_login(code: &str, verifier: &str) -> SoundomeResult<String> {
    let token = exchange_code(code, verifier).await?;

    // `connect` with `store_credentials = true` swaps the short-lived access
    // token for a reusable blob and writes it into the cache.
    let cache = build_cache()?;
    let session = Session::new(SessionConfig::default(), Some(cache));
    session
        .connect(Credentials::with_access_token(token), true)
        .await
        .map_err(|e| Error::Custom(format!("Spotify session connect failed: {e}")))?;

    Ok(session.username())
}

/// Where Spotify redirects after approval. Fixed by [`CLIENT_ID`]'s
/// registration, so it cannot be moved to another port.
const LISTEN_ADDR: &str = "127.0.0.1:8898";

/// How long the listener waits before giving the port back.
const CALLBACK_TIMEOUT_SECS: u64 = 300;

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

/// Accept one browser request, answer it, and return the authorization code.
///
/// Stray requests (a health check, a prefetch) are answered and ignored rather
/// than ending the wait, which is what made librespot's one-shot listener so
/// easy to break.
async fn await_callback(listener: TcpListener, expected_state: &str) -> SoundomeResult<String> {
    let deadline = Duration::from_secs(CALLBACK_TIMEOUT_SECS);

    tokio::time::timeout(deadline, async {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| Error::Custom(format!("Callback accept failed: {e}")))?;

            let mut buffer = vec![0u8; 4096];
            let read = stream.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();

            let target = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or_default()
                .to_string();

            let params = query_params(&target);
            let code = params.get("code").cloned();
            let state = params.get("state").cloned();
            let denied = params.get("error").cloned();

            let body = if denied.is_some() {
                "Spotify authorization was denied. You can close this tab."
            } else if code.is_some() && state.as_deref() == Some(expected_state) {
                "Spotify audio connected. You can close this tab."
            } else {
                "Waiting for the Spotify authorization callback."
            };
            let _ = stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await;

            if let Some(denied) = denied {
                return Err(Error::Custom(format!("Spotify refused the login: {denied}")));
            }
            match (code, state) {
                (Some(code), Some(state)) if state == expected_state => return Ok(code),
                // Anything else is not our callback; keep listening.
                _ => continue,
            }
        }
    })
    .await
    .map_err(|_| Error::Custom("Timed out waiting for Spotify authorization".to_string()))?
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
async fn exchange_code(code: &str, verifier: &str) -> SoundomeResult<String> {
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

    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| Error::Custom("Spotify returned no access token".to_string()))
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
pub fn clear() -> SoundomeResult<()> {
    match std::fs::remove_dir_all(cache_dir()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Custom(format!("Cannot clear librespot cache: {e}"))),
    }
}
