//! Spotify app credential endpoints.
//!
//! Spotify still runs an open developer programme, so unlike SoundCloud the
//! credential is an app id and secret rather than a session. It grants public
//! catalogue access only, which is what metadata enrichment needs.
//!
//! Storing a pair here deliberately does **not** enable downloading Spotify
//! URLs: that path resolves audio from YouTube Music, and it stays gated behind
//! an explicit `[providers.spotify]` entry in the config file.

use fetcher::spotify::{auth, session};
use rocket::{delete, get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::error::{CustomError, Error};

#[derive(Serialize, JsonSchema)]
pub struct SpotifyStatus {
    /// True when a usable client id and secret are configured.
    pub connected: bool,
    /// The client id in effect. The secret is never returned.
    pub client_id: Option<String>,
    /// True when the user has also authorized their own account.
    pub user_connected: bool,
    /// Display name of the authorized account, when known.
    pub user_name: Option<String>,
}

impl SpotifyStatus {
    /// Read the current state from disk. Both halves are independent: app
    /// credentials can exist without a login, and a login cannot exist
    /// without them.
    fn current() -> Self {
        let client_id = auth::configured_client_id();
        let session = session::stored_session();
        Self {
            connected: client_id.is_some(),
            client_id,
            user_connected: session.is_some(),
            user_name: session.and_then(|s| s.user_name),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ConnectRequest {
    pub client_id: String,
    pub client_secret: String,
}

fn bad_request(code: &str, message: String) -> Error {
    Error::Custom(CustomError {
        status: Status::BadRequest,
        code: code.to_string(),
        message,
    })
}

fn internal(message: String) -> Error {
    Error::Custom(CustomError {
        status: Status::InternalServerError,
        code: "Internal".to_string(),
        message,
    })
}

/// Current Spotify credential state.
#[openapi]
#[get("/providers/spotify")]
pub fn get_status() -> Json<SpotifyStatus> {
    Json(SpotifyStatus::current())
}

/// Verify and store Spotify app credentials.
#[openapi]
#[post("/providers/spotify", data = "<body>")]
pub async fn connect(body: Json<ConnectRequest>) -> Result<Json<SpotifyStatus>, Error> {
    let client_id = body.client_id.trim();
    let client_secret = body.client_secret.trim();

    if client_id.is_empty() || client_secret.is_empty() {
        return Err(bad_request(
            "InvalidCredentials",
            "Both the client id and the client secret are required".to_string(),
        ));
    }

    // Exchanging them for a token is the only proof they work; storing an
    // unverified pair would surface as a confusing enrichment failure later.
    let valid = auth::verify_credentials(client_id, client_secret)
        .await
        .map_err(|e| internal(e.to_string()))?;

    if !valid {
        return Err(bad_request(
            "InvalidCredentials",
            "Spotify rejected this client id and secret. Check both values in the developer \
             dashboard."
                .to_string(),
        ));
    }

    auth::store_credentials(client_id, client_secret).map_err(|e| internal(e.to_string()))?;
    tracing::info!("Spotify credentials stored for app {}", client_id);

    Ok(Json(SpotifyStatus::current()))
}

/// Forget the stored Spotify credentials.
#[openapi]
#[delete("/providers/spotify")]
pub fn disconnect() -> Result<Json<SpotifyStatus>, Error> {
    auth::clear_credentials().map_err(|e| internal(e.to_string()))?;
    tracing::info!("Spotify credentials cleared");

    // Dropping app credentials invalidates any login built on them.
    // Config-file credentials may outlive the stored pair, so report what is
    // left rather than assuming disconnected.
    session::clear_session().map_err(|e| internal(e.to_string()))?;
    Ok(Json(SpotifyStatus::current()))
}

#[derive(Serialize, JsonSchema)]
pub struct LoginUrl {
    /// Where the browser must go to approve the request.
    pub authorize_url: String,
}

/// Start the PKCE login and hand back the URL to visit.
#[openapi]
#[post("/providers/spotify/login")]
pub fn login() -> Result<Json<LoginUrl>, Error> {
    let authorize_url =
        session::begin_login().map_err(|e| bad_request("LoginUnavailable", e.to_string()))?;
    Ok(Json(LoginUrl { authorize_url }))
}

#[derive(Deserialize, JsonSchema)]
pub struct CallbackRequest {
    pub code: String,
    pub state: String,
}

/// Finish the login with the code Spotify handed the browser.
///
/// Spotify redirects to the web UI, not to this API, because the registered
/// redirect URI has to be a loopback literal the user actually browses. The UI
/// therefore reads the query string and posts it here.
#[openapi]
#[post("/providers/spotify/callback", data = "<body>")]
pub async fn callback(body: Json<CallbackRequest>) -> Result<Json<SpotifyStatus>, Error> {
    let session = session::complete_login(body.code.trim(), body.state.trim())
        .await
        .map_err(|e| bad_request("LoginFailed", e.to_string()))?;

    tracing::info!(
        "Spotify login completed for {}",
        session.user_name.as_deref().unwrap_or("unknown account")
    );

    Ok(Json(SpotifyStatus::current()))
}

/// Forget the login while keeping the app credentials.
#[openapi]
#[delete("/providers/spotify/session")]
pub fn logout() -> Result<Json<SpotifyStatus>, Error> {
    session::clear_session().map_err(|e| internal(e.to_string()))?;
    tracing::info!("Spotify login cleared");
    Ok(Json(SpotifyStatus::current()))
}

#[derive(Serialize, JsonSchema)]
pub struct SavedTrackDto {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_secs: Option<i32>,
    pub artwork_url: Option<String>,
    pub spotify_url: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SavedTracks {
    pub count: usize,
    pub tracks: Vec<SavedTrackDto>,
}

/// List the signed-in account's Liked Songs.
///
/// Read-only. Spotify serves no audio, so nothing here can be downloaded from
/// Spotify itself; this exists to browse and to match against the library.
#[openapi]
#[get("/spotify/likes")]
pub async fn list_likes() -> Result<Json<SavedTracks>, Error> {
    let tracks = session::saved_tracks()
        .await
        .map_err(|e| bad_request("LikesUnavailable", e.to_string()))?;

    let tracks: Vec<SavedTrackDto> = tracks
        .into_iter()
        .map(|t| SavedTrackDto {
            id: t.id,
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_secs: t.duration_secs,
            artwork_url: t.artwork_url,
            spotify_url: t.spotify_url,
        })
        .collect();

    Ok(Json(SavedTracks {
        count: tracks.len(),
        tracks,
    }))
}
