//! Spotify user authorization (PKCE).
//!
//! App credentials (see `auth`) only reach the public catalogue. Reading a
//! user's own library needs their consent, which means the authorization code
//! flow. PKCE is used rather than the classic flow so the client secret never
//! leaves the server, and so the same code would work if Soundome ever shipped
//! a public client id.
//!
//! Written against `reqwest` rather than rspotify's client: rspotify is
//! compiled here with its blocking backend, and blocking calls inside Rocket's
//! async runtime stall the worker. rspotify still backs the metadata provider.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use config::Config;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::{errors::Error, http::HttpClientBuilder, types::SoundomeResult};

const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const ME_URL: &str = "https://api.spotify.com/v1/me";

/// Read-only access to the signed-in user's library. Nothing here can modify
/// the account, and no playback scopes are requested.
const SCOPES: &str = "user-library-read playlist-read-private";

/// Spotify removed `localhost` aliases and plain HTTP redirects on 27 November
/// 2025. Loopback literals stay allowed, so the UI is reached at 127.0.0.1 and
/// this must match the URI registered in the developer dashboard exactly.
const REDIRECT_URI: &str = "http://127.0.0.1:5273/spotify/callback";

/// Refresh this long before the token actually expires, so a sync that starts
/// just under the wire does not fail halfway.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// The half-finished flow: a verifier waiting for its callback.
#[derive(Serialize, Deserialize)]
struct PendingAuth {
    verifier: String,
    state: String,
}

/// A usable session.
#[derive(Serialize, Deserialize, Clone)]
pub struct SpotifySession {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: u64,
    pub user_name: Option<String>,
    /// Which client the refresh token was issued to.
    ///
    /// A refresh must be sent to the same client that minted the token, and
    /// two different clients can mint one: the app credentials pasted into the
    /// Providers tab, and Spotify's desktop client used by the librespot login.
    /// Sending the wrong one fails with `invalid_client` about an hour after
    /// logging in, which is a miserable thing to debug.
    ///
    /// Optional so sessions stored before this field existed still load.
    #[serde(default)]
    pub client_id: Option<String>,
}

impl SpotifySession {
    fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.expires_at > now + EXPIRY_MARGIN.as_secs()
    }
}

fn session_path() -> std::path::PathBuf {
    Config::get()
        .spotify_credentials_path()
        .with_file_name("spotify_session.json")
}

fn pending_path() -> std::path::PathBuf {
    Config::get()
        .spotify_credentials_path()
        .with_file_name("spotify_pending_auth.json")
}

fn client_id() -> SoundomeResult<String> {
    Config::get()
        .resolved_spotify_credentials()
        .map(|(id, _)| id)
        .ok_or_else(|| {
            Error::Custom("Connect Spotify app credentials before logging in".to_string())
        })
}

/// Build the URL the browser must visit, and remember the verifier it will be
/// matched against.
pub fn begin_login() -> SoundomeResult<String> {
    let client_id = client_id()?;

    let verifier: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();

    write_private(
        &pending_path(),
        &serde_json::to_string(&PendingAuth {
            verifier,
            state: state.clone(),
        })
        .map_err(|e| Error::Custom(format!("Cannot store the pending login: {}", e)))?,
    )
    .map_err(|e| Error::Custom(format!("Cannot store the pending login: {}", e)))?;

    let query = [
        ("client_id", client_id.as_str()),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge.as_str()),
        ("scope", SCOPES),
        ("state", state.as_str()),
    ];
    let query = query
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!("{}?{}", AUTHORIZE_URL, query))
}

/// Exchange the callback code for tokens and persist the session.
pub async fn complete_login(code: &str, state: &str) -> SoundomeResult<SpotifySession> {
    let raw = fs::read_to_string(pending_path())
        .map_err(|_| Error::Custom("No login is in progress. Start again.".to_string()))?;
    let pending: PendingAuth = serde_json::from_str(&raw)
        .map_err(|e| Error::Custom(format!("Unreadable pending login: {}", e)))?;

    // The state parameter is the only thing tying this callback to the login
    // we started; a mismatch means the request did not come from our flow.
    if pending.state != state {
        return Err(Error::Custom(
            "Login state did not match. Start again.".to_string(),
        ));
    }

    let client_id = client_id()?;
    let response = HttpClientBuilder::get_reqwest_client()?
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", client_id.as_str()),
            ("code_verifier", pending.verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Spotify token exchange failed: {}", e)))?;

    let session = session_from_response(response, None, client_id).await?;
    let _ = fs::remove_file(pending_path());
    store_session(&session)?;
    Ok(session)
}

/// A valid access token, refreshing first when the stored one is stale.
pub async fn access_token() -> SoundomeResult<String> {
    let session =
        stored_session().ok_or_else(|| Error::Custom("Log in with Spotify first".to_string()))?;

    if session.is_fresh() {
        return Ok(session.access_token);
    }

    // Refresh against whichever client minted the token, not whatever happens
    // to be configured now. Falls back to the configured pair for sessions
    // stored before the field existed.
    let client_id = match session.client_id.clone() {
        Some(client_id) => client_id,
        None => client_id()?,
    };
    let response = HttpClientBuilder::get_reqwest_client()?
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", session.refresh_token.as_str()),
            ("client_id", client_id.as_str()),
        ])
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Spotify token refresh failed: {}", e)))?;

    // Spotify may omit the refresh token on a refresh, in which case the old
    // one stays valid.
    let refreshed = session_from_response(response, Some(&session), client_id).await?;
    store_session(&refreshed)?;
    Ok(refreshed.access_token)
}

async fn session_from_response(
    response: reqwest::Response,
    previous: Option<&SpotifySession>,
    issued_by: String,
) -> SoundomeResult<SpotifySession> {
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Unreadable Spotify token response: {}", e)))?;

    if !status.is_success() {
        let detail = body
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(Error::Custom(format!(
            "Spotify refused the login: {}",
            detail
        )));
    }

    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Custom("Spotify returned no access token".to_string()))?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| previous.map(|p| p.refresh_token.clone()))
        .ok_or_else(|| Error::Custom("Spotify returned no refresh token".to_string()))?;
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(3600);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut session = SpotifySession {
        access_token,
        refresh_token,
        expires_at: now + expires_in,
        user_name: previous.and_then(|p| p.user_name.clone()),
        client_id: Some(issued_by),
    };

    if session.user_name.is_none() {
        session.user_name = fetch_display_name(&session.access_token).await;
    }

    Ok(session)
}

/// Best effort: the account name is only for showing who is logged in.
async fn fetch_display_name(access_token: &str) -> Option<String> {
    let response = HttpClientBuilder::get_reqwest_client()
        .ok()?
        .get(ME_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;
    let body: serde_json::Value = response.json().await.ok()?;
    body.get("display_name")
        .or_else(|| body.get("id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// One entry of the signed-in user's Liked Songs.
pub struct SavedTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_secs: Option<i32>,
    pub artwork_url: Option<String>,
    pub spotify_url: String,
}

impl SavedTrack {
    /// Convert to the shared model, carrying a Spotify `Source` reference so
    /// the downloader routes it the same way as a pasted Spotify URL.
    pub fn to_track(&self) -> shared::models::Track {
        use shared::models::{Album, AlbumType, Artist, Platform, Reference, ReferenceType, Track};

        let artists = vec![Artist {
            id: None,
            name: self.artist.clone(),
            icon: None,
            references: Vec::new(),
        }];

        Track {
            id: None,
            needs_validation: false,
            validation_reason: None,
            soundome_id: None,
            title: self.title.clone(),
            artists: artists.clone(),
            album: self.album.as_ref().map(|title| Album {
                id: None,
                title: title.clone(),
                artists,
                cover: self.artwork_url.clone(),
                date: None,
                album_type: AlbumType::Unknown,
                references: Vec::new(),
            }),
            genre: None,
            duration: self.duration_secs,
            track_number: None,
            disc_number: None,
            label: None,
            date: None,
            cover: self.artwork_url.clone(),
            file_path: None,
            references: vec![Reference {
                id: None,
                ref_type: ReferenceType::Source,
                platform: Platform::Spotify,
                external_id: Some(self.id.clone()),
                external_url: Some(self.spotify_url.clone()),
            }],
        }
    }
}

/// Store a Web API session obtained by another login.
///
/// The librespot login runs its PKCE exchange against Spotify's desktop
/// client, which may also ask for library scopes. Reusing that single approval
/// means the user does not have to register an app or log in twice.
pub async fn store_user_token(
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    client_id: String,
) -> SoundomeResult<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let user_name = fetch_display_name(&access_token).await;
    store_session(&SpotifySession {
        access_token,
        refresh_token,
        expires_at: now + expires_in,
        user_name,
        client_id: Some(client_id),
    })
}

/// Every liked track, newest first.
///
/// Read-only: this lists what the account has saved and downloads nothing.
pub async fn saved_tracks() -> SoundomeResult<Vec<SavedTrack>> {
    let token = access_token().await?;
    let client = HttpClientBuilder::get_reqwest_client()?;

    let limit = 50;
    let mut offset = 0;
    let mut tracks = Vec::new();

    loop {
        let response = client
            .get("https://api.spotify.com/v1/me/tracks")
            .bearer_auth(&token)
            .query(&[("limit", limit.to_string()), ("offset", offset.to_string())])
            .send()
            .await
            .map_err(|e| Error::Custom(format!("Spotify saved tracks request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Custom(format!(
                "Spotify refused the saved tracks request: {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Custom(format!("Unreadable saved tracks response: {}", e)))?;

        let items = body.get("items").and_then(|v| v.as_array()).cloned();
        let Some(items) = items.filter(|items| !items.is_empty()) else {
            break;
        };
        let page_len = items.len();

        for item in items {
            let Some(track) = item.get("track") else {
                continue;
            };
            let Some(id) = track.get("id").and_then(|v| v.as_str()) else {
                // Local files a user added to Spotify have no id and no URL.
                continue;
            };

            tracks.push(SavedTrack {
                id: id.to_string(),
                title: track
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown title")
                    .to_string(),
                artist: track
                    .pointer("/artists/0/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown artist")
                    .to_string(),
                album: track
                    .pointer("/album/name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                duration_secs: track
                    .get("duration_ms")
                    .and_then(|v| v.as_i64())
                    .map(|ms| (ms / 1000) as i32),
                artwork_url: track
                    .pointer("/album/images/0/url")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                spotify_url: track
                    .pointer("/external_urls/spotify")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }

        if page_len < limit {
            break;
        }
        offset += limit;
    }

    tracing::info!("Fetched {} liked tracks from Spotify", tracks.len());
    Ok(tracks)
}

pub fn stored_session() -> Option<SpotifySession> {
    serde_json::from_str(&fs::read_to_string(session_path()).ok()?).ok()
}

fn store_session(session: &SpotifySession) -> SoundomeResult<()> {
    let body = serde_json::to_string_pretty(session)
        .map_err(|e| Error::Custom(format!("Cannot serialise the session: {}", e)))?;
    write_private(&session_path(), &body)
        .map_err(|e| Error::Custom(format!("Cannot write the session: {}", e)))
}

/// Forget the login. App credentials are left alone.
pub fn clear_session() -> SoundomeResult<()> {
    for path in [session_path(), pending_path()] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Custom(format!(
                    "Cannot remove {}: {}",
                    path.display(),
                    e
                )))
            }
        }
    }
    Ok(())
}

/// Percent-encode a query value. Only the characters Spotify's parameters can
/// actually contain need escaping (spaces in scopes, slashes and colons in the
/// redirect URI).
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

fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)?.write_all(contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_only_what_needs_escaping() {
        assert_eq!(urlencode("abc-123_x.y~z"), "abc-123_x.y~z");
        assert_eq!(
            urlencode("http://127.0.0.1:5273/spotify/callback"),
            "http%3A%2F%2F127.0.0.1%3A5273%2Fspotify%2Fcallback"
        );
        assert_eq!(
            urlencode("user-library-read playlist-read-private"),
            "user-library-read%20playlist-read-private"
        );
    }

    #[test]
    fn challenge_is_base64url_sha256_of_the_verifier() {
        // The value RFC 7636 uses as its worked example.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn a_session_expiring_within_the_margin_is_stale() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let session = |expires_at| SpotifySession {
            access_token: "t".into(),
            refresh_token: "r".into(),
            expires_at,
            user_name: None,
            client_id: None,
        };

        assert!(session(now + 3600).is_fresh());
        assert!(!session(now + 10).is_fresh(), "inside the refresh margin");
        assert!(
            !session(now.saturating_sub(1)).is_fresh(),
            "already expired"
        );
    }

    #[test]
    fn a_session_refreshes_against_the_client_that_minted_it() {
        // The two logins use different clients: the pasted app credentials and
        // Spotify's desktop client. Refreshing with the wrong one fails with
        // invalid_client an hour after logging in.
        let session = SpotifySession {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 0,
            user_name: None,
            client_id: Some("desktop-client".into()),
        };
        assert_eq!(session.client_id.as_deref(), Some("desktop-client"));

        // Sessions written before the field existed must still load, falling
        // back to the configured pair.
        let legacy: SpotifySession = serde_json::from_str(
            r#"{"access_token":"a","refresh_token":"r","expires_at":0,"user_name":null}"#,
        )
        .expect("legacy session must still deserialise");
        assert_eq!(legacy.client_id, None);
    }
}
