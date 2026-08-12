//! Spotify app credential endpoints.
//!
//! Spotify still runs an open developer programme, so unlike SoundCloud the
//! credential is an app id and secret rather than a session. It grants public
//! catalogue access only, which is what metadata enrichment needs.
//!
//! Storing a pair here deliberately does **not** enable downloading Spotify
//! URLs: that path resolves audio from YouTube Music, and it stays gated behind
//! an explicit `[providers.spotify]` entry in the config file.

use fetcher::spotify::auth;
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
    let client_id = auth::configured_client_id();
    Json(SpotifyStatus {
        connected: client_id.is_some(),
        client_id,
    })
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

    Ok(Json(SpotifyStatus {
        connected: true,
        client_id: Some(client_id.to_string()),
    }))
}

/// Forget the stored Spotify credentials.
#[openapi]
#[delete("/providers/spotify")]
pub fn disconnect() -> Result<Json<SpotifyStatus>, Error> {
    auth::clear_credentials().map_err(|e| internal(e.to_string()))?;
    tracing::info!("Spotify credentials cleared");

    // Config-file credentials outlive the stored pair, so report what is left
    // rather than assuming disconnected.
    let client_id = auth::configured_client_id();
    Ok(Json(SpotifyStatus {
        connected: client_id.is_some(),
        client_id,
    }))
}
