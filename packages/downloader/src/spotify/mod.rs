//! Spotify direct-audio download provider, backed by librespot.
//!
//! Unlike the other providers, this pulls the audio straight from Spotify
//! instead of matching the track on YouTube. It requires a connected Spotify
//! **Premium** session (see [`auth`]).
//!
//! Spotify serves the stream as Ogg Vorbis wrapped in a proprietary container.
//! librespot decrypts it; we skip the container header to recover a standard
//! Ogg Vorbis stream and write it as `.ogg` (the tagger writes Vorbis comments
//! directly, see `tagger::ogg`). Spotify's audio-key channel and stream
//! throttle rate-limit bursts, so per-track fetches retry transient failures
//! with backoff.

pub mod auth;

use std::{
    io::{Seek, SeekFrom},
    path::PathBuf,
    time::Duration,
};

use async_trait::async_trait;
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::{FileId, SpotifyId, SpotifyUri};
use librespot_metadata::{audio::file::AudioFileFormat, Metadata, Track as SpotifyTrack};
use shared::{
    errors::Error,
    models::{Platform, Reference, ReferenceType, Track},
    types::SoundgnomeResult,
};

use crate::Provider;

/// Byte offset where the real Ogg Vorbis stream begins inside Spotify's
/// container (`SPOTIFY_OGG_HEADER_END` in librespot-playback). The bytes before
/// it carry Spotify's own normalisation data, which we discard.
const SPOTIFY_OGG_HEADER_END: u64 = 0xa7;

/// Ogg Vorbis qualities to try, best first. Only Ogg Vorbis is requested: it is
/// what every Premium account can serve, and the decode path below assumes it.
const PREFERRED_FORMATS: &[AudioFileFormat] = &[
    AudioFileFormat::OGG_VORBIS_320,
    AudioFileFormat::OGG_VORBIS_160,
    AudioFileFormat::OGG_VORBIS_96,
];

/// Streaming throttle hint for `AudioFile::open`, sized for 320 kbps.
const BYTES_PER_SECOND: usize = 320 * 1024 / 8;

/// Retry policy for transient Spotify audio-delivery errors (audio-key channel
/// "service unavailable", stream "wait timeout"). Bulk syncs trip these once a
/// handful of tracks pull in quick succession; a short backoff paces the
/// channel and lets it recover.
const MAX_ATTEMPTS: usize = 4;
const RETRY_BACKOFF_SECS: [u64; 3] = [5, 15, 30];

pub struct Spotify;

/// Extract the base62 track id from a source/provider value, accepting an
/// `open.spotify.com/track/<id>` URL, a `spotify:track:<id>` URI, or a bare id.
fn extract_track_id(value: &str) -> SoundgnomeResult<String> {
    if let Some(rest) = value.strip_prefix("spotify:track:") {
        return Ok(rest.to_string());
    }
    if let Some(idx) = value.find("/track/") {
        let id = value[idx + "/track/".len()..]
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    if !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(value.to_string());
    }
    Err(Error::Custom(format!(
        "cannot extract a Spotify track id from '{value}'"
    )))
}

/// List ALL of the user's Liked Songs via the librespot session's native
/// collection endpoint (`spclient` + login5 auth), avoiding the throttled
/// `/me/tracks` Web API. The first page comes back as a protobuf `Context`;
/// subsequent pages are JSON fetched via `next_page_url`. Resolves per-track
/// metadata (skipping any that fail).
pub async fn liked_tracks() -> SoundgnomeResult<Vec<fetcher::spotify::session::SavedTrack>> {
    let session = auth::session().await?;
    let user = session.username();
    let spclient = session.spclient();

    let ctx = spclient
        .get_context(&format!("spotify:user:{user}:collection"))
        .await
        .map_err(|e| Error::Custom(format!("Spotify collection request failed: {e}")))?;

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut uris: Vec<String> = Vec::new();
    for page in &ctx.pages {
        for t in &page.tracks {
            let u = t.uri();
            if u.starts_with("spotify:track:") && seen.insert(u.to_string()) {
                uris.push(u.to_string());
            }
        }
    }
    let mut next: Option<String> = ctx.pages.iter().rev().find_map(|p| {
        let n = p.next_page_url();
        (!n.is_empty()).then(|| n.to_string())
    });

    // Follow next_page_url until exhausted. A visited set guards against loops.
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some(url) = next.take() {
        if !visited.insert(url.clone()) {
            break;
        }
        let bytes = match spclient.get_next_page(&url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Spotify collection next page failed: {e}");
                break;
            }
        };
        let json: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Spotify collection page parse failed: {e}");
                break;
            }
        };
        collect_track_uris(&json, &mut seen, &mut uris);
        next = find_next_page_url(&json);
    }

    tracing::info!("Spotify collection: {} liked track uris across all pages", uris.len());

    let total = uris.len();
    let mut out = Vec::new();
    for (i, uri) in uris.iter().enumerate() {
        let id = uri.rsplit(':').next().unwrap_or("").to_string();
        let Ok(sid) = SpotifyId::from_base62(&id) else {
            continue;
        };
        let track = match SpotifyTrack::get(&session, &SpotifyUri::Track { id: sid }).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Spotify metadata for {} failed: {}", uri, e);
                continue;
            }
        };
        out.push(fetcher::spotify::session::SavedTrack {
            id: id.clone(),
            title: track.name.clone(),
            artist: track
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            album: Some(track.album.name.clone()),
            duration_secs: Some(track.duration / 1000),
            artwork_url: None,
            spotify_url: format!("https://open.spotify.com/track/{id}"),
        });
        if (i + 1) % 50 == 0 {
            tracing::info!("Spotify collection: resolved metadata {}/{}", i + 1, total);
        }
    }

    if out.is_empty() {
        return Err(Error::Custom(
            "Spotify collection returned no resolvable liked tracks".to_string(),
        ));
    }
    tracing::info!("Spotify collection: {} liked tracks ready to sync", out.len());
    Ok(out)
}

/// Recursively collect `spotify:track:` uris from any `"uri"` fields in a JSON
/// context page (the shape varies), de-duplicating via `seen`.
fn collect_track_uris(
    value: &serde_json::Value,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(u)) = map.get("uri") {
                if u.starts_with("spotify:track:") && seen.insert(u.clone()) {
                    out.push(u.clone());
                }
            }
            for v in map.values() {
                collect_track_uris(v, seen, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_track_uris(v, seen, out);
            }
        }
        _ => {}
    }
}

/// Find the next-page url in a JSON context page, tolerating key naming.
fn find_next_page_url(value: &serde_json::Value) -> Option<String> {
    ["next_page_url", "nextPageUrl", "next"]
        .into_iter()
        .find_map(|key| {
            value
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
}

/// Register [`liked_tracks`] as the fetcher's liked-songs provider so the
/// Liked Songs sync reads the whole collection natively instead of `/me/tracks`.
pub fn register_liked_provider() {
    fetcher::spotify::session::set_liked_provider(Box::new(|| Box::pin(liked_tracks())));
}

#[async_trait]
impl Provider for Spotify {
    /// The provider *is* the Spotify track: no matching is needed, so this just
    /// echoes the source as a Spotify provider reference.
    async fn search(&self, track: &Track) -> SoundgnomeResult<Reference> {
        let source = track
            .get_source()
            .ok_or_else(|| Error::Custom("track source not defined".to_string()))?;

        Ok(Reference {
            id: None,
            ref_type: ReferenceType::Provider,
            platform: Platform::Spotify,
            external_id: source.external_id.clone(),
            external_url: source.external_url.clone(),
        })
    }

    async fn download(
        &mut self,
        url: &str,
        file_name: &str,
        base_library_dir: PathBuf,
    ) -> SoundgnomeResult<PathBuf> {
        let track_id = extract_track_id(url)?;
        let spotify_id = SpotifyId::from_base62(&track_id)
            .map_err(|e| Error::Custom(format!("invalid Spotify track id '{track_id}': {e:?}")))?;

        let session = auth::session().await?;

        let track = SpotifyTrack::get(&session, &SpotifyUri::Track { id: spotify_id })
            .await
            .map_err(|e| Error::Custom(format!("Spotify track metadata failed: {e}")))?;

        let (key_id, file_id) = resolve_playable(&session, track).await.ok_or_else(|| {
            Error::Custom(format!(
                "no Ogg Vorbis stream available for Spotify track '{track_id}' \
                 (region-restricted or unavailable in this market)"
            ))
        })?;

        let ogg_path = base_library_dir.join(format!("{file_name}.ogg"));

        // Retry transient audio-delivery failures with backoff (see the retry
        // constants). A fresh audio-key request each attempt is what recovers
        // "audio key error"; hard errors (no stream) return immediately.
        for attempt in 1..=MAX_ATTEMPTS {
            match fetch_ogg(&session, key_id, file_id, &ogg_path).await {
                Ok(()) => return Ok(ogg_path),
                Err(e) if attempt < MAX_ATTEMPTS && is_transient(&e) => {
                    let wait = RETRY_BACKOFF_SECS[attempt - 1];
                    tracing::warn!(
                        "Spotify download transient error for '{track_id}' \
                         (attempt {attempt}/{MAX_ATTEMPTS}), retrying in {wait}s: {e}"
                    );
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("retry loop returns on success or on the final attempt's error")
    }

    fn is_valid_url(url: &str) -> bool {
        url.contains("open.spotify.com/track/")
    }
}

/// Fetch, decrypt and write one track's Ogg Vorbis stream. Separated from
/// `download` so it can be retried on transient failures.
async fn fetch_ogg(
    session: &librespot_core::Session,
    spotify_id: SpotifyId,
    file_id: FileId,
    ogg_path: &std::path::Path,
) -> SoundgnomeResult<()> {
    let key = session
        .audio_key()
        .request(spotify_id, file_id)
        .await
        .map_err(|e| Error::Custom(format!("Spotify audio key request failed: {e}")))?;

    let audio = AudioFile::open(session, file_id, BYTES_PER_SECOND)
        .await
        .map_err(|e| Error::Custom(format!("Spotify audio stream open failed: {e}")))?;

    let write_path = ogg_path.to_path_buf();
    // Decrypt + drain on a blocking thread: `AudioDecrypt`'s Read blocks the
    // caller while the async fetch task (driven by `session`) fills buffers.
    tokio::task::spawn_blocking(move || -> SoundgnomeResult<()> {
        let mut decrypted = AudioDecrypt::new(Some(key), audio);
        decrypted
            .seek(SeekFrom::Start(SPOTIFY_OGG_HEADER_END))
            .map_err(|e| Error::Custom(format!("Spotify stream seek failed: {e}")))?;
        let mut out = std::fs::File::create(&write_path)?;
        std::io::copy(&mut decrypted, &mut out)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Custom(format!("Spotify download task failed: {e}")))?
}

/// The Ogg Vorbis `FileId` for a track, best quality first.
fn pick_ogg(track: &SpotifyTrack) -> Option<FileId> {
    PREFERRED_FORMATS
        .iter()
        .find_map(|format| track.files.0.get(format).copied())
}

/// Resolve a playable `(id, file)` for a track, following Spotify's relinking.
///
/// In many markets `Track::get` returns the requested track with an empty
/// `files` map and the real audio under `alternatives` (other track ids for the
/// market-available version). Use the main track's file when present, else the
/// first alternative that has one -- mirroring librespot's player. The returned
/// id is the one that owns the file, which the audio-key request needs.
async fn resolve_playable(
    session: &librespot_core::Session,
    track: SpotifyTrack,
) -> Option<(SpotifyId, FileId)> {
    if let Some(file_id) = pick_ogg(&track) {
        if let SpotifyUri::Track { id } = track.id {
            return Some((id, file_id));
        }
    }
    for alt in track.alternatives.iter() {
        let SpotifyUri::Track { id } = alt else {
            continue;
        };
        if let Ok(alt_track) = SpotifyTrack::get(session, alt).await {
            if let Some(file_id) = pick_ogg(&alt_track) {
                return Some((*id, file_id));
            }
        }
    }
    None
}

/// Transient errors from Spotify's audio-key channel / stream throttle that a
/// short backoff-and-retry usually clears.
fn is_transient(e: &Error) -> bool {
    let s = e.to_string().to_lowercase();
    s.contains("audio key")
        || s.contains("service unavailable")
        || s.contains("deadline")
        || s.contains("timeout")
        || s.contains("timed out")
        || s.contains("channel closed")
        || s.contains("aborted")
        || s.contains("internal error")
}

#[cfg(test)]
mod tests {
    use super::extract_track_id;

    #[test]
    fn extracts_id_from_url_uri_and_bare() {
        let id = "3vJQ0UFRHmNpZK8h7UmU1S";
        assert_eq!(
            extract_track_id(&format!("https://open.spotify.com/track/{id}?si=abc")).unwrap(),
            id
        );
        assert_eq!(
            extract_track_id(&format!("https://open.spotify.com/track/{id}")).unwrap(),
            id
        );
        assert_eq!(
            extract_track_id(&format!("spotify:track:{id}")).unwrap(),
            id
        );
        assert_eq!(extract_track_id(id).unwrap(), id);
        assert!(extract_track_id("https://example.com/foo").is_err());
    }
}
