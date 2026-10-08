//! Spotify audio (librespot) session credential endpoints.
//!
//! This is the Premium librespot audio session, which streams tracks directly
//! from Spotify as AAC. It is separate from the metadata-only Spotify provider
//! (`routes::spotify`): connecting here authorizes a real playback session, so
//! it requires a Spotify Premium account. Login is a paste-back flow: the UI
//! opens the authorize URL, the user approves and copies the redirect URL back
//! into the UI, which posts it here. Once connected, tracks whose source is
//! Spotify download directly from Spotify instead of being matched on YouTube.

use downloader::spotify::auth;
use rocket::{delete, get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::error::{CustomError, Error};

#[derive(Serialize, JsonSchema)]
pub struct SpotifyAudioLogin {
    /// Where the browser must go to approve the streaming session.
    pub authorize_url: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SpotifyAudioStatus {
    /// True when a reusable librespot credentials blob is stored.
    pub connected: bool,
    /// Spotify account username the stored credentials belong to, when known.
    pub username: Option<String>,
}

impl SpotifyAudioStatus {
    fn current() -> Self {
        Self {
            connected: auth::is_connected(),
            username: auth::connected_username(),
        }
    }
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

/// Current Spotify audio (librespot) connection state.
#[openapi]
#[get("/providers/spotify-audio")]
pub fn get_status() -> Json<SpotifyAudioStatus> {
    Json(SpotifyAudioStatus::current())
}

/// Start the login and hand back the URL to approve.
///
/// The UI opens this URL; after approving, the browser lands on Spotify's
/// registered `127.0.0.1:8898` redirect, which the user pastes back to
/// [`callback`]. No server-side listener is involved, so this works remotely.
#[openapi]
#[post("/providers/spotify-audio/login")]
pub fn login() -> Result<Json<SpotifyAudioLogin>, Error> {
    let authorize_url =
        auth::begin_login().map_err(|e| bad_request("LoginUnavailable", e.to_string()))?;

    Ok(Json(SpotifyAudioLogin { authorize_url }))
}

#[derive(Deserialize, JsonSchema)]
pub struct SpotifyAudioCallback {
    /// The full URL Spotify redirected the browser to after approval
    /// (`http://127.0.0.1:8898/login?code=...&state=...`), or the bare code.
    pub redirect_url: String,
}

/// Finish the login from the redirect URL the user pasted back.
#[openapi]
#[post("/providers/spotify-audio/callback", data = "<body>")]
pub async fn callback(body: Json<SpotifyAudioCallback>) -> Result<Json<SpotifyAudioStatus>, Error> {
    let username = auth::complete_login(body.redirect_url.trim())
        .await
        .map_err(|e| bad_request("LoginFailed", e.to_string()))?;

    tracing::info!("Spotify audio (librespot) connected for {username}");

    Ok(Json(SpotifyAudioStatus {
        connected: true,
        username: Some(username),
    }))
}

/// Forget the stored Spotify audio (librespot) credentials.
#[openapi]
#[delete("/providers/spotify-audio")]
pub fn disconnect() -> Result<Json<SpotifyAudioStatus>, Error> {
    auth::clear().map_err(|e| internal(e.to_string()))?;

    tracing::info!("Spotify audio (librespot) credentials cleared");

    Ok(Json(SpotifyAudioStatus {
        connected: false,
        username: None,
    }))
}
