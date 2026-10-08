//! SoundCloud session credential endpoints.
//!
//! There is no OAuth app flow to offer: SoundCloud has not issued new API
//! credentials since 2021. What the user can supply is the `oauth_token` cookie
//! from their own browser session, which is what yt-dlp accepts. These routes
//! verify that token against SoundCloud and persist it for the downloader.

use fetcher::soundcloud::{auth, Soundcloud};
use rocket::{delete, get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::error::{CustomError, Error};

#[derive(Serialize, JsonSchema)]
pub struct SoundcloudStatus {
    /// True when a session token is stored and usable by the downloader.
    pub connected: bool,
    /// Account handle the stored token belongs to, when known.
    pub username: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ConnectRequest {
    /// Value of the `oauth_token` cookie from a logged-in soundcloud.com session.
    pub token: String,
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

/// Current SoundCloud connection state.
///
/// The stored token is re-verified against SoundCloud on every call, so a
/// session revoked from the SoundCloud side shows as disconnected rather than
/// failing later mid-download.
#[openapi]
#[get("/providers/soundcloud")]
pub async fn get_status() -> Result<Json<SoundcloudStatus>, Error> {
    let Some(token) = auth::stored_token() else {
        return Ok(Json(SoundcloudStatus {
            connected: false,
            username: None,
        }));
    };

    match auth::verify_token(&token).await {
        Ok(Some(username)) => Ok(Json(SoundcloudStatus {
            connected: true,
            username: Some(username),
        })),
        Ok(None) => Ok(Json(SoundcloudStatus {
            connected: false,
            username: None,
        })),
        // A network failure is not proof the token is bad. Report it instead of
        // silently telling the user they are disconnected.
        Err(e) => Err(internal(e.to_string())),
    }
}

/// Verify and store a SoundCloud session token.
#[openapi]
#[post("/providers/soundcloud", data = "<body>")]
pub async fn connect(body: Json<ConnectRequest>) -> Result<Json<SoundcloudStatus>, Error> {
    let token = body.token.trim();
    if token.is_empty() {
        return Err(bad_request("InvalidToken", "Token is empty".to_string()));
    }

    let username = auth::verify_token(token)
        .await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| {
            bad_request(
                "InvalidToken",
                "SoundCloud rejected this token. Copy the oauth_token cookie again from a \
                 logged-in soundcloud.com session."
                    .to_string(),
            )
        })?;

    auth::store_token(token).map_err(|e| internal(e.to_string()))?;
    tracing::info!("SoundCloud session stored for {}", username);

    Ok(Json(SoundcloudStatus {
        connected: true,
        username: Some(username),
    }))
}

/// Forget the stored SoundCloud session token.
#[openapi]
#[delete("/providers/soundcloud")]
pub fn disconnect() -> Result<Json<SoundcloudStatus>, Error> {
    auth::clear_token().map_err(|e| internal(e.to_string()))?;
    tracing::info!("SoundCloud session cleared");

    Ok(Json(SoundcloudStatus {
        connected: false,
        username: None,
    }))
}

#[derive(Serialize, JsonSchema)]
pub struct StreamUrl {
    /// Short-lived signed CDN URL, playable directly by an audio element.
    pub url: String,
}

/// Resolve a preview stream URL for a SoundCloud track.
///
/// The returned URL is signed and expires, so clients should re-resolve rather
/// than cache it.
#[openapi]
#[get("/soundcloud/likes/<id>/stream")]
pub async fn stream_url(id: u64) -> Result<Json<StreamUrl>, Error> {
    let soundcloud = Soundcloud::new()
        .await
        .map_err(|e| internal(e.to_string()))?;

    let url = soundcloud
        .resolve_stream_url(id)
        .await
        .map_err(|e| bad_request("StreamUnavailable", e.to_string()))?;

    Ok(Json(StreamUrl { url }))
}
