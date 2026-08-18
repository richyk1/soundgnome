//! Last.fm connection + scrobbling endpoints.
//!
//! Connect flow: `POST /providers/lastfm/credentials` stores the API key/secret,
//! `POST /providers/lastfm/login` returns a URL to approve, and
//! `POST /providers/lastfm/callback` exchanges the approved token for a session.
//! Once connected, the player posts plays to `/lastfm/now-playing` and
//! `/lastfm/scrobble`, which are signed and forwarded server-side.

use rocket::{delete, get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::error::{CustomError, Error};
use crate::utils::lastfm;

fn bad_request(code: &str, message: String) -> Error {
    Error::Custom(CustomError {
        status: Status::BadRequest,
        code: code.to_string(),
        message,
    })
}

// ── Status ────────────────────────────────────────────────────────────────────

#[derive(Serialize, JsonSchema)]
pub struct LastfmStatus {
    /// API key + secret have been provided.
    pub configured: bool,
    /// A user session is connected (scrobbling is possible).
    pub connected: bool,
    pub username: Option<String>,
}

impl LastfmStatus {
    fn current() -> Self {
        let session = lastfm::stored_session();
        Self {
            configured: lastfm::stored_credentials().is_some(),
            connected: session.is_some(),
            username: session.map(|s| s.username),
        }
    }
}

/// Current Last.fm connection state.
#[openapi]
#[get("/providers/lastfm")]
pub fn get_status() -> Json<LastfmStatus> {
    Json(LastfmStatus::current())
}

// ── Credentials ─────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct LastfmCredentials {
    pub api_key: String,
    pub api_secret: String,
}

/// Store the Last.fm API key + shared secret (created at last.fm/api/account/create).
#[openapi]
#[post("/providers/lastfm/credentials", data = "<body>")]
pub fn set_credentials(body: Json<LastfmCredentials>) -> Result<Json<LastfmStatus>, Error> {
    let body = body.into_inner();
    lastfm::store_credentials(&body.api_key, &body.api_secret)
        .map_err(|e| bad_request("InvalidCredentials", e.to_string()))?;
    Ok(Json(LastfmStatus::current()))
}

// ── Auth ────────────────────────────────────────────────────────────────────

#[derive(Serialize, JsonSchema)]
pub struct LastfmLogin {
    /// URL to open and approve on Last.fm.
    pub url: String,
    /// Token to pass back to the callback once approved.
    pub token: String,
}

/// Begin the login: mint a request token and return the URL to approve.
#[openapi]
#[post("/providers/lastfm/login")]
pub async fn login() -> Result<Json<LastfmLogin>, Error> {
    let token = lastfm::get_token()
        .await
        .map_err(|e| bad_request("LoginFailed", e.to_string()))?;
    let url =
        lastfm::authorize_url(&token).map_err(|e| bad_request("LoginFailed", e.to_string()))?;
    Ok(Json(LastfmLogin { url, token }))
}

#[derive(Deserialize, JsonSchema)]
pub struct LastfmCallback {
    pub token: String,
}

/// Finish the login by exchanging the approved token for a session.
#[openapi]
#[post("/providers/lastfm/callback", data = "<body>")]
pub async fn callback(body: Json<LastfmCallback>) -> Result<Json<LastfmStatus>, Error> {
    lastfm::get_session(&body.into_inner().token)
        .await
        .map_err(|e| bad_request("CallbackFailed", e.to_string()))?;
    Ok(Json(LastfmStatus::current()))
}

/// Disconnect the Last.fm account (keeps the stored API credentials).
#[openapi]
#[delete("/providers/lastfm")]
pub fn disconnect() -> Result<Json<LastfmStatus>, Error> {
    lastfm::clear_session().map_err(|e| bad_request("DisconnectFailed", e.to_string()))?;
    Ok(Json(LastfmStatus::current()))
}

// ── Scrobbling ──────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct NowPlaying {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub duration_secs: Option<u32>,
}

/// Report the currently playing track to Last.fm.
#[openapi]
#[post("/lastfm/now-playing", data = "<body>")]
pub async fn now_playing(body: Json<NowPlaying>) -> Result<Json<serde_json::Value>, Error> {
    let body = body.into_inner();
    lastfm::update_now_playing(
        &body.artist,
        &body.track,
        body.album.as_deref(),
        body.duration_secs,
    )
    .await
    .map_err(|e| bad_request("NowPlayingFailed", e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize, JsonSchema)]
pub struct ScrobbleReq {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub duration_secs: Option<u32>,
    /// UTC unix seconds when the track started playing.
    pub timestamp: i64,
}

#[derive(Deserialize, JsonSchema)]
pub struct ScrobbleBatch {
    pub scrobbles: Vec<ScrobbleReq>,
}

/// Record one or more completed plays (batch, up to 50).
#[openapi]
#[post("/lastfm/scrobble", data = "<body>")]
pub async fn scrobble(body: Json<ScrobbleBatch>) -> Result<Json<serde_json::Value>, Error> {
    let items: Vec<lastfm::ScrobbleItem> = body
        .into_inner()
        .scrobbles
        .into_iter()
        .map(|s| lastfm::ScrobbleItem {
            artist: s.artist,
            track: s.track,
            album: s.album,
            duration: s.duration_secs,
            timestamp: s.timestamp,
        })
        .collect();
    lastfm::scrobble(&items)
        .await
        .map_err(|e| bad_request("ScrobbleFailed", e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "ok": true, "count": items.len() }),
    ))
}
