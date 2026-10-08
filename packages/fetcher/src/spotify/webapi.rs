//! Spotify Web API client backed by the user OAuth token.
//!
//! Every Spotify catalogue lookup (tracks, albums, artists, playlists, search)
//! goes through here, authorized by the single user session obtained from the
//! librespot login (see [`super::session`]). There is no app client-credentials
//! path any more: one Spotify connection covers audio, Liked Songs, and
//! metadata.
//!
//! Written with raw `reqwest` rather than rspotify: rspotify is a blocking
//! (ureq) client, and blocking calls on the async runtime stall Rocket's
//! workers. `super::session::saved_tracks` already talks to the Web API this
//! way; this module extends that to the catalogue endpoints.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use shared::{
    errors::Error,
    http::HttpClientBuilder,
    models::{
        Album, AlbumType, Artist, Platform, Playlist, PlaylistTrack, Reference, ReferenceType,
        Track,
    },
    types::SoundgnomeResult,
};

use super::session;

/// Market used for track relinking/availability, matching the previous rspotify
/// behaviour (Country::France).
const MARKET: &str = "FR";

// ================================================================================================
// Raw response shapes (only the fields Soundgnome consumes)
// ================================================================================================

#[derive(Deserialize)]
struct SpImage {
    url: String,
}

#[derive(Deserialize)]
struct SpArtist {
    #[serde(default)]
    name: String,
    id: Option<String>,
    #[serde(default)]
    external_urls: HashMap<String, String>,
    /// Present only on the full artist object (search / GET /artists/{id}).
    #[serde(default)]
    images: Vec<SpImage>,
}

#[derive(Deserialize)]
struct SpAlbum {
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Vec<SpArtist>,
    album_type: Option<String>,
    #[serde(default)]
    images: Vec<SpImage>,
    release_date: Option<String>,
    id: Option<String>,
    #[serde(default)]
    external_urls: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SpTrack {
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Vec<SpArtist>,
    /// Absent on tracks returned from an album's track listing.
    album: Option<SpAlbum>,
    #[serde(default)]
    duration_ms: i64,
    #[serde(default)]
    track_number: i64,
    #[serde(default)]
    disc_number: i64,
    id: Option<String>,
    #[serde(default)]
    external_urls: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SpPage<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    total: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Deserialize)]
struct SpPlaylistItem {
    added_at: Option<String>,
    track: Option<SpTrack>,
}

#[derive(Deserialize)]
struct SpTracksSearch {
    tracks: SpPage<SpTrack>,
}

#[derive(Deserialize)]
struct SpArtistsSearch {
    artists: SpPage<SpArtist>,
}

#[derive(Deserialize)]
struct SpAlbumsSearch {
    albums: SpPage<SpAlbum>,
}

#[derive(Deserialize)]
struct SpPlaylistMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    images: Vec<SpImage>,
}

// ================================================================================================
// Mapping to shared models
//
// Nested artists and albums always carry `Metadata` references; only the
// top-level track reference varies (a fetched source track is `Source`, a
// tagger match candidate is `Metadata`), so `map_track` takes the ref type.
// ================================================================================================

fn spotify_ref(
    ref_type: ReferenceType,
    id: Option<String>,
    urls: &HashMap<String, String>,
) -> Reference {
    Reference {
        id: None,
        ref_type,
        platform: Platform::Spotify,
        external_id: id,
        external_url: urls.get("spotify").cloned(),
    }
}

fn parse_album_type(album_type: Option<&str>) -> AlbumType {
    match album_type {
        Some("album") => AlbumType::Album,
        Some("single") => AlbumType::Single,
        Some("compilation") => AlbumType::Compilation,
        _ => AlbumType::Unknown,
    }
}

fn map_artist(artist: &SpArtist) -> Artist {
    Artist {
        id: None,
        name: artist.name.clone(),
        icon: artist.images.first().map(|image| image.url.clone()),
        references: vec![spotify_ref(
            ReferenceType::Metadata,
            artist.id.clone(),
            &artist.external_urls,
        )],
    }
}

fn map_album(album: &SpAlbum) -> Album {
    Album {
        id: None,
        title: album.name.clone(),
        artists: album.artists.iter().map(map_artist).collect(),
        album_type: parse_album_type(album.album_type.as_deref()),
        cover: album.images.first().map(|image| image.url.clone()),
        date: album.release_date.clone(),
        references: vec![spotify_ref(
            ReferenceType::Metadata,
            album.id.clone(),
            &album.external_urls,
        )],
    }
}

/// `album_context` supplies the parent album for tracks returned from an album's
/// track listing (those omit their own `album` field).
fn map_track(track: &SpTrack, album_context: Option<&SpAlbum>, track_ref: ReferenceType) -> Track {
    let album = track.album.as_ref().or(album_context);

    Track {
        id: None,
        needs_validation: false,
        validation_reason: None,
        soundome_id: None,
        title: track.name.clone(),
        artists: track.artists.iter().map(map_artist).collect(),
        album: album.map(map_album),
        genre: None,
        duration: Some((track.duration_ms / 1000) as i32),
        file_path: None,
        track_number: Some(track.track_number as i32),
        disc_number: Some(track.disc_number as i32),
        label: None,
        date: album.and_then(|a| a.release_date.clone()),
        cover: album.and_then(|a| a.images.first().map(|image| image.url.clone())),
        references: vec![spotify_ref(
            track_ref,
            track.id.clone(),
            &track.external_urls,
        )],
    }
}

fn parse_added_at(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn map_playlist_item(
    item: &SpPlaylistItem,
    position: u32,
    track_ref: ReferenceType,
) -> Option<PlaylistTrack> {
    let track = item.track.as_ref()?;
    Some(PlaylistTrack {
        // Spotify never exposes a lossless original.
        original_available: Some(false),
        id: None,
        track: map_track(track, None, track_ref),
        added_at: parse_added_at(item.added_at.as_deref()),
        position: Some(position),
    })
}

// ================================================================================================
// HTTP
// ================================================================================================

async fn api_get<T: DeserializeOwned>(path: &str, query: &[(&str, &str)]) -> SoundgnomeResult<T> {
    let token = session::access_token().await?;
    let client = HttpClientBuilder::get_reqwest_client()?;

    let response = client
        .get(format!("https://api.spotify.com/v1/{path}"))
        .bearer_auth(token)
        .query(query)
        .send()
        .await
        .map_err(|e| Error::Network(format!("Spotify request /v1/{path} failed: {e}")))?;

    if !response.status().is_success() {
        return Err(Error::NotFound(format!(
            "Spotify /v1/{path} returned {}",
            response.status()
        )));
    }

    response
        .json::<T>()
        .await
        .map_err(|e| Error::Custom(format!("Unreadable Spotify response for /v1/{path}: {e}")))
}

// ================================================================================================
// Public API
// ================================================================================================

pub async fn get_track(id: &str, track_ref: ReferenceType) -> SoundgnomeResult<Track> {
    let track: SpTrack = api_get(&format!("tracks/{id}"), &[("market", MARKET)]).await?;
    Ok(map_track(&track, None, track_ref))
}

pub async fn search_tracks(
    query: &str,
    limit: u32,
    track_ref: ReferenceType,
) -> SoundgnomeResult<Vec<Track>> {
    let limit = limit.to_string();
    let res: SpTracksSearch = api_get(
        "search",
        &[
            ("q", query),
            ("type", "track"),
            ("limit", &limit),
            ("offset", "0"),
        ],
    )
    .await?;
    Ok(res
        .tracks
        .items
        .iter()
        .map(|track| map_track(track, None, track_ref.clone()))
        .collect())
}

pub async fn get_playlist(id: &str, source_url: &str) -> SoundgnomeResult<Playlist> {
    // `fields` keeps the payload tiny and dodges rspotify's historical panic on
    // playlists that omit `tracks`.
    let meta: SpPlaylistMeta =
        api_get(&format!("playlists/{id}"), &[("fields", "id,name,images")]).await?;
    Ok(Playlist {
        id: None,
        name: meta.name,
        source: Platform::Spotify,
        source_url: Some(source_url.to_string()),
        cover: meta.images.first().map(|image| image.url.clone()),
    })
}

pub async fn get_playlist_tracks(
    id: &str,
    track_ref: ReferenceType,
) -> SoundgnomeResult<Vec<PlaylistTrack>> {
    let mut all = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 50;

    loop {
        let (limit_s, offset_s) = (limit.to_string(), offset.to_string());
        let page: SpPage<SpPlaylistItem> = api_get(
            &format!("playlists/{id}/tracks"),
            &[
                ("market", MARKET),
                ("limit", &limit_s),
                ("offset", &offset_s),
            ],
        )
        .await?;

        for (i, item) in page.items.iter().enumerate() {
            if let Some(track) = map_playlist_item(item, offset + i as u32, track_ref.clone()) {
                all.push(track);
            }
        }

        if page.next.is_none() || page.items.is_empty() {
            break;
        }
        offset += limit;
    }

    Ok(all)
}

pub async fn get_artist(id: &str) -> SoundgnomeResult<Artist> {
    let artist: SpArtist = api_get(&format!("artists/{id}"), &[]).await?;
    Ok(map_artist(&artist))
}

pub async fn get_artist_tracks(id: &str, track_ref: ReferenceType) -> SoundgnomeResult<Vec<Track>> {
    let mut all = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 50;

    loop {
        let (limit_s, offset_s) = (limit.to_string(), offset.to_string());
        let albums: SpPage<SpAlbum> = api_get(
            &format!("artists/{id}/albums"),
            &[
                ("include_groups", "album,single"),
                ("market", MARKET),
                ("limit", &limit_s),
                ("offset", &offset_s),
            ],
        )
        .await?;

        for album in &albums.items {
            let Some(album_id) = album.id.as_deref() else {
                continue;
            };
            // First page of tracks per album, matching the previous behaviour.
            let tracks: SpPage<SpTrack> = api_get(
                &format!("albums/{album_id}/tracks"),
                &[("market", MARKET), ("limit", "50"), ("offset", "0")],
            )
            .await?;
            for track in &tracks.items {
                all.push(map_track(track, Some(album), track_ref.clone()));
            }
        }

        if albums.items.is_empty() || albums.offset + albums.items.len() as u32 >= albums.total {
            break;
        }
        offset += limit;
    }

    Ok(all)
}

pub async fn search_artists(query: &str, limit: u32) -> SoundgnomeResult<Vec<Artist>> {
    let limit = limit.to_string();
    let res: SpArtistsSearch = api_get(
        "search",
        &[
            ("q", query),
            ("type", "artist"),
            ("limit", &limit),
            ("offset", "0"),
        ],
    )
    .await?;
    Ok(res.artists.items.iter().map(map_artist).collect())
}

/// Best-effort artist image lookup by name, for tagger icon enrichment.
pub async fn artist_icon(name: &str) -> Option<String> {
    let res: SpArtistsSearch = api_get(
        "search",
        &[
            ("q", name),
            ("type", "artist"),
            ("limit", "1"),
            ("offset", "0"),
        ],
    )
    .await
    .ok()?;
    res.artists
        .items
        .into_iter()
        .next()
        .and_then(|artist| artist.images.into_iter().next().map(|image| image.url))
}

pub async fn get_album(id: &str) -> SoundgnomeResult<Album> {
    let album: SpAlbum = api_get(&format!("albums/{id}"), &[]).await?;
    Ok(map_album(&album))
}

pub async fn search_albums(query: &str, limit: u32) -> SoundgnomeResult<Vec<Album>> {
    let limit = limit.to_string();
    let res: SpAlbumsSearch = api_get(
        "search",
        &[
            ("q", query),
            ("type", "album"),
            ("limit", &limit),
            ("offset", "0"),
        ],
    )
    .await?;
    Ok(res.albums.items.iter().map(map_album).collect())
}

pub async fn get_album_tracks(id: &str, track_ref: ReferenceType) -> SoundgnomeResult<Vec<Track>> {
    // Album context (tracks from the listing omit their own album field).
    let album: SpAlbum = api_get(&format!("albums/{id}"), &[]).await?;

    let mut all = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 50;

    loop {
        let (limit_s, offset_s) = (limit.to_string(), offset.to_string());
        let page: SpPage<SpTrack> = api_get(
            &format!("albums/{id}/tracks"),
            &[
                ("market", MARKET),
                ("limit", &limit_s),
                ("offset", &offset_s),
            ],
        )
        .await?;

        for track in &page.items {
            all.push(map_track(track, Some(&album), track_ref.clone()));
        }

        if page.items.is_empty() || page.offset + page.items.len() as u32 >= page.total {
            break;
        }
        offset += limit;
    }

    Ok(all)
}
