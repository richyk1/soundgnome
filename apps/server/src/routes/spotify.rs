//! Spotify Liked Songs listing.
//!
//! This module now only lists the signed-in account's Liked Songs. The Spotify
//! connection itself lives in `routes::spotify_audio` via librespot.

use fetcher::spotify::session;
use rocket::{get, http::Status, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::Serialize;

use crate::utils::error::{CustomError, Error};

fn bad_request(code: &str, message: String) -> Error {
    Error::Custom(CustomError {
        status: Status::BadRequest,
        code: code.to_string(),
        message,
    })
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
