//! Spotify audio (librespot) session credential endpoints.
//!
//! This is the Premium librespot audio session, which streams tracks directly
//! from Spotify as AAC. It is separate from the metadata-only Spotify provider
//! (`routes::spotify`): connecting here authorizes a real playback session, so
//! it requires a Spotify Premium account. Connecting opens a browser on the
//! server host to authorize, which works for a localhost or self-hosted
//! deployment; a remote or headless host needs port 8898 reachable. Once
//! connected, tracks whose source is Spotify download directly from Spotify
//! as `.m4a` instead of being matched on YouTube.

use downloader::spotify::auth;
use rocket::{delete, get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::Serialize;

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
/// Returns immediately: the callback is caught in the background, so the UI can
/// open the link itself instead of the operator reading a server log.
#[openapi]
#[post("/providers/spotify-audio/login")]
pub async fn login() -> Result<Json<SpotifyAudioLogin>, Error> {
    let authorize_url = auth::begin_login()
        .await
        .map_err(|e| bad_request("LoginUnavailable", e.to_string()))?;

    Ok(Json(SpotifyAudioLogin { authorize_url }))
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
