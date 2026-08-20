use std::sync::Arc;

use domain::services::ServiceLayer;
use rocket::fs::NamedFile;
use rocket::{delete, get, http::Status, patch, post, put, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::models::{Album, Artist, Platform, Rating, Reference, ReferenceType, Track};

use crate::utils::{database::Db, error::CustomError, response::Success};

// ================================================================================================
// DTOs
// ================================================================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackArtistDto {
    pub id: Option<i32>,
    pub name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackAlbumDto {
    pub id: Option<i32>,
    pub title: String,
}

// ================================================================================================
// Shared reference DTO (used by all entity routes)
// ================================================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReferenceDto {
    pub id: Option<i32>,
    pub ref_type: String,
    pub platform: String,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
}

pub fn reference_to_dto(r: Reference) -> ReferenceDto {
    ReferenceDto {
        id: r.id,
        ref_type: r.ref_type.as_ref().to_string(),
        platform: r.platform.as_ref().to_string(),
        external_id: r.external_id,
        external_url: r.external_url,
    }
}

/// True for internal bookkeeping references (acoustic fingerprint, content hash)
/// stored under the `soundome:` URL scheme. They are dedup metadata, not
/// user-facing links, and the fingerprint blob is large (~8 KB each), so they are
/// excluded from API responses.
pub fn is_internal_reference(r: &Reference) -> bool {
    r.external_url
        .as_deref()
        .is_some_and(|u| u.starts_with("soundome:"))
}

/// Map a track's references to DTOs, dropping internal bookkeeping references.
pub fn references_to_dto(refs: Vec<Reference>) -> Vec<ReferenceDto> {
    refs.into_iter()
        .filter(|r| !is_internal_reference(r))
        .map(reference_to_dto)
        .collect()
}

/// Body for manually adding a reference to any entity.
///
/// `platform` and `external_id` are optional: when omitted (or blank/"Unknown"),
/// they are inferred from `external_url` — see `Reference::infer_platform_and_id`.
/// This lets the web UI's "Add reference" form ask for just a `ref_type` and a link,
/// while still accepting explicit values for edge cases inference can't cover
/// (e.g. SoundCloud/Bandcamp, whose URLs don't embed a stable id).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddReferenceBody {
    /// One of: Source, Provider, Metadata, Reference
    pub ref_type: String,
    /// One of: Spotify, SoundCloud, MusicBrainz, YoutubeMusic, Youtube, Bandcamp, Unknown.
    /// Optional — inferred from `external_url` when absent.
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub external_url: Option<String>,
}

impl AddReferenceBody {
    pub fn into_reference(self) -> Reference {
        let external_url = self.external_url.filter(|u| !u.trim().is_empty());

        // Best-effort inference from the URL, used whenever the caller does not pin
        // `platform`/`external_id` explicitly (the common case: the UI only asks for a link).
        let inferred = external_url
            .as_deref()
            .map(Reference::infer_platform_and_id);

        let platform = self
            .platform
            .filter(|p| !p.trim().is_empty() && !p.eq_ignore_ascii_case("unknown"))
            .map(|p| Platform::from_str(&p))
            .or_else(|| inferred.as_ref().map(|(platform, _)| platform.clone()))
            .unwrap_or(Platform::Unknown);

        let external_id = self
            .external_id
            .filter(|id| !id.trim().is_empty())
            .or_else(|| inferred.and_then(|(_, id)| id));

        Reference {
            id: None,
            ref_type: ReferenceType::from_str(&self.ref_type),
            platform,
            external_id,
            external_url,
        }
    }
}

// ================================================================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackDto {
    pub id: i32,
    pub title: String,
    pub artists: Vec<TrackArtistDto>,
    pub album: Option<TrackAlbumDto>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub label: Option<String>,
    pub file_path: Option<String>,
    pub needs_validation: bool,
    /// Probed audio quality, absent when there is no local file or it cannot
    /// be read.
    pub quality: Option<TrackQualityDto>,
    pub references: Vec<ReferenceDto>,
    pub rating: Option<Rating>,
}

/// Measured properties of the audio file backing a track.
///
/// Probed from disk on read rather than stored: a re-tagged or replaced file
/// then reports its real quality instead of a stale row. The probe only reads
/// container headers, so it is cheap enough to do per response.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackQualityDto {
    /// Short codec label, e.g. "FLAC", "AAC", "MP3".
    pub format: String,
    pub bitrate_kbps: u32,
    pub lossless: bool,
}

impl TrackQualityDto {
    fn probe(track: &Track) -> Option<Self> {
        let quality = crate::utils::quality_cache::probe(track)?;
        let extension = track
            .file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        // An m4a holds either AAC or ALAC, and only the lossless flag tells
        // them apart, so the label is derived from both.
        let format = match extension.as_str() {
            "m4a" | "mp4" | "aac" => {
                if quality.lossless {
                    "ALAC"
                } else {
                    "AAC"
                }
            }
            "mp3" => "MP3",
            "flac" => "FLAC",
            "opus" => "OPUS",
            "ogg" => {
                if quality.lossless {
                    "PCM"
                } else {
                    "VORBIS"
                }
            }
            "wav" | "wave" => "WAV",
            _ => {
                if quality.lossless {
                    "PCM"
                } else {
                    "LOSSY"
                }
            }
        };

        Some(Self {
            format: format.to_string(),
            bitrate_kbps: quality.bitrate_bps / 1000,
            lossless: quality.lossless,
        })
    }
}

impl TrackDto {
    fn from_track(track: Track) -> Option<Self> {
        // Probe before the struct is torn apart: it needs `file_path`.
        let quality = TrackQualityDto::probe(&track);

        Some(Self {
            id: track.id?,
            title: track.title,
            artists: track
                .artists
                .into_iter()
                .map(|a| TrackArtistDto {
                    id: a.id,
                    name: a.name,
                })
                .collect(),
            album: track.album.map(|a| TrackAlbumDto {
                id: a.id,
                title: a.title,
            }),
            date: track.date,
            genre: track.genre,
            cover: track.cover,
            duration: track.duration,
            track_number: track.track_number,
            disc_number: track.disc_number,
            label: track.label,
            file_path: track
                .file_path
                .and_then(|p| p.to_str().map(|s| s.to_string())),
            needs_validation: track.needs_validation,
            quality,
            references: references_to_dto(track.references),
            rating: None,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTrackBody {
    pub title: Option<String>,
    pub artists: Option<Vec<String>>,
    pub album_title: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub label: Option<String>,
    pub cover: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetRatingBody {
    /// New rating, or null to clear it.
    pub rating: Option<Rating>,
}

// ================================================================================================
// Routes
// ================================================================================================

#[openapi]
#[get("/tracks")]
pub async fn get_all(
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<Vec<TrackDto>>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    db.run(move |conn| -> shared::types::SoundgnomeResult<(Vec<Track>, Vec<(i32, Rating)>)> {
        let tracks = services.track_service.get_all(conn)?;
        let ratings = services.track_service.get_ratings(conn)?;
        Ok((tracks, ratings))
    })
        .await
        .map(|(tracks, ratings)| {
            let ratings: std::collections::HashMap<i32, Rating> = ratings.into_iter().collect();
            Json(
                tracks
                    .into_iter()
                    .filter_map(|t| {
                        let id = t.id?;
                        let mut dto = TrackDto::from_track(t)?;
                        dto.rating = ratings.get(&id).copied();
                        Some(dto)
                    })
                    .collect(),
            )
        })
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::InternalServerError,
                code: "Internal".to_string(),
                message: err.to_string(),
            })
        })
}

#[openapi]
#[get("/tracks/<id>")]
pub async fn get(
    id: i32,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<TrackDto>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    db.run(move |conn| -> shared::types::SoundgnomeResult<(Track, Option<Rating>)> {
        let track = services.track_service.get_by_id(conn, id)?;
        let rating = services
            .track_service
            .get_ratings(conn)?
            .into_iter()
            .find(|(tid, _)| *tid == id)
            .map(|(_, r)| r);
        Ok((track, rating))
    })
        .await
        .and_then(|(track, rating)| {
            let mut dto = TrackDto::from_track(track)
                .ok_or_else(|| shared::errors::Error::Database("Track has no id".to_string()))?;
            dto.rating = rating;
            Ok(dto)
        })
        .map(Json)
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::NotFound,
                code: "NotFound".to_string(),
                message: err.to_string(),
            })
        })
}

#[openapi]
#[patch("/tracks/<id>", format = "application/json", data = "<body>")]
pub async fn update(
    id: i32,
    body: Json<UpdateTrackBody>,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<TrackDto>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    let body = body.into_inner();

    db.run(move |conn| -> shared::types::SoundgnomeResult<Track> {
        let old_track = services.track_service.get_by_id(conn, id)?;
        let mut track = old_track.clone();

        if let Some(title) = body.title {
            track.title = title;
        }
        if let Some(genre) = body.genre {
            track.genre = Some(genre);
        }
        if let Some(date) = body.date {
            track.date = Some(date);
        }
        if let Some(tn) = body.track_number {
            track.track_number = Some(tn);
        }
        if let Some(dn) = body.disc_number {
            track.disc_number = Some(dn);
        }
        if let Some(label) = body.label {
            track.label = Some(label);
        }
        if let Some(cover) = body.cover {
            track.cover = Some(cover);
        }

        if let Some(names) = body.artists {
            // Deduplicate artist names and reuse or create Artist records
            let mut artists = Vec::new();
            for name in names {
                let artist = Artist {
                    id: None,
                    name,
                    icon: None,
                    references: vec![],
                };
                let saved = services.artist_service.create_or_ignore(conn, &artist)?;
                artists.push(saved);
            }
            track.artists = artists;
        }

        if let Some(album_title) = body.album_title {
            // Preserve album ID and other metadata, updating the title
            let existing_album = track.album.clone();
            let new_album = Album {
                id: existing_album.as_ref().and_then(|a| a.id),
                title: album_title.clone(),
                artists: existing_album
                    .as_ref()
                    .map(|a| a.artists.clone())
                    .unwrap_or_default(),
                album_type: shared::models::AlbumType::Album,
                cover: existing_album.as_ref().and_then(|a| a.cover.clone()),
                date: existing_album.as_ref().and_then(|a| a.date.clone()),
                references: existing_album
                    .as_ref()
                    .map(|a| a.references.clone())
                    .unwrap_or_default(),
            };

            // If album has an ID, update it; otherwise, create_or_ignore will handle it in create_or_update
            if let Some(album_id) = new_album.id {
                if let Err(e) = services.album_service.update(conn, album_id, &new_album) {
                    tracing::warn!("Failed to update album: {}", e);
                }
            }

            track.album = Some(new_album);
        }

        // Update file if it exists and metadata changed
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                if let Err(e) = services
                    .download_service
                    .update_track_file_metadata(&old_track, &mut track)
                    .await
                {
                    tracing::warn!("Failed to update track file metadata: {}", e);
                    // Don't fail the entire request if file update fails
                }
            })
        });

        services.track_service.create_or_update(conn, &track)
    })
    .await
    .and_then(|track| {
        TrackDto::from_track(track)
            .ok_or_else(|| shared::errors::Error::Database("Track has no id".to_string()))
    })
    .map(Json)
    .map_err(|err| {
        crate::utils::error::Error::Custom(CustomError {
            status: Status::InternalServerError,
            code: "Internal".to_string(),
            message: err.to_string(),
        })
    })
}

#[openapi]
#[put("/tracks/<id>/rating", format = "application/json", data = "<body>")]
pub async fn set_rating(
    id: i32,
    body: Json<SetRatingBody>,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<TrackDto>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    let rating = body.into_inner().rating;
    let result = db
        .run(move |conn| -> shared::types::SoundgnomeResult<Track> {
            services.track_service.set_rating(conn, id, rating)?;
            services.track_service.get_by_id(conn, id)
        })
        .await;

    // Best-effort: mirror the like to Last.fm loved tracks when a session is
    // connected. Detached so it never delays or fails the rating write.
    if let Ok(track) = &result {
        sync_lastfm_love(track, rating);
    }

    result
        .and_then(|track| {
            let mut dto = TrackDto::from_track(track)
                .ok_or_else(|| shared::errors::Error::Database("Track has no id".to_string()))?;
            dto.rating = rating;
            Ok(dto)
        })
        .map(Json)
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::InternalServerError,
                code: "Internal".to_string(),
                message: err.to_string(),
            })
        })
}

/// Reflect a rating change onto the connected Last.fm account's loved tracks.
/// `liked` loves the track; a dislike or a cleared rating unloves it. No-op
/// unless a Last.fm session is connected; runs detached and never fails the
/// request. Artists are joined the same way the scrobbler formats them so the
/// love attaches to the same track Last.fm already knows from scrobbles.
fn sync_lastfm_love(track: &Track, rating: Option<Rating>) {
    use crate::utils::lastfm;

    if lastfm::stored_session().is_none() {
        return;
    }
    let artist = track
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if artist.is_empty() {
        return;
    }
    let title = track.title.clone();
    let liked = matches!(rating, Some(Rating::Liked));
    rocket::tokio::spawn(async move {
        let result = if liked {
            lastfm::love(&artist, &title).await
        } else {
            lastfm::unlove(&artist, &title).await
        };
        if let Err(e) = result {
            tracing::warn!("Last.fm love update for '{artist} - {title}' failed: {e}");
        }
    });
}

#[openapi]
#[delete("/tracks/<id>")]
pub async fn delete(
    id: i32,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<Success>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    db.run(move |conn| services.track_service.delete_by_id(conn, id))
        .await
        .map(|_| Json(Success { success: true }))
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::InternalServerError,
                code: "Internal".to_string(),
                message: err.to_string(),
            })
        })
}

/// Download the audio file for a track.
#[openapi]
#[get("/tracks/<id>/download")]
pub async fn download_file(
    id: i32,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<NamedFile, crate::utils::error::Error> {
    let services = Arc::clone(services);

    let track = db
        .run(move |conn| services.track_service.get_by_id(conn, id))
        .await
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: match err {
                    shared::errors::Error::NotFound(_) => Status::NotFound,
                    _ => Status::InternalServerError,
                },
                code: "NotFound".to_string(),
                message: err.to_string(),
            })
        })?;

    let file_path = track.file_path.ok_or_else(|| {
        crate::utils::error::Error::Custom(CustomError {
            status: Status::NotFound,
            code: "NoFile".to_string(),
            message: "Track has no local file".to_string(),
        })
    })?;

    NamedFile::open(&file_path).await.map_err(|_| {
        crate::utils::error::Error::Custom(CustomError {
            status: Status::NotFound,
            code: "FileNotFound".to_string(),
            message: format!("Audio file not found on disk: {}", file_path.display()),
        })
    })
}

// ================================================================================================
// Reference sub-resource
// ================================================================================================

/// List all references attached to a track.
#[openapi]
#[get("/tracks/<id>/references")]
pub async fn get_references(
    id: i32,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<Vec<ReferenceDto>>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    db.run(move |conn| services.track_service.get_by_id(conn, id))
        .await
        .map(|track| Json(references_to_dto(track.references)))
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::NotFound,
                code: "NotFound".to_string(),
                message: err.to_string(),
            })
        })
}

/// Add a reference to a track.
#[openapi]
#[post(
    "/tracks/<id>/references",
    format = "application/json",
    data = "<body>"
)]
pub async fn add_reference(
    id: i32,
    body: Json<AddReferenceBody>,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<Vec<ReferenceDto>>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    let reference = body.into_inner().into_reference();

    db.run(move |conn| services.track_service.add_reference(conn, id, reference))
        .await
        .map(|refs| Json(references_to_dto(refs)))
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::InternalServerError,
                code: "Internal".to_string(),
                message: err.to_string(),
            })
        })
}

/// Remove a single reference from a track by its reference row ID.
#[openapi]
#[delete("/tracks/<_id>/references/<ref_id>")]
pub async fn delete_reference(
    _id: i32,
    ref_id: i32,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<Json<Success>, crate::utils::error::Error> {
    let services = Arc::clone(services);
    db.run(move |conn| services.track_service.delete_reference(conn, ref_id))
        .await
        .map(|_| Json(Success { success: true }))
        .map_err(|err| {
            crate::utils::error::Error::Custom(CustomError {
                status: Status::InternalServerError,
                code: "Internal".to_string(),
                message: err.to_string(),
            })
        })
}
