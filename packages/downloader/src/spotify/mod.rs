//! Spotify direct-audio download provider, backed by librespot.
//!
//! Unlike the other providers, this pulls the audio straight from Spotify
//! instead of matching the track on YouTube. It requires a connected Spotify
//! **Premium** session (see [`auth`]).
//!
//! Spotify serves the stream as Ogg Vorbis wrapped in a proprietary container.
//! librespot decrypts it; we skip the container header to recover a standard
//! Ogg Vorbis stream, then transcode it to AAC/`.m4a`. The transcode exists
//! because the tagger cannot write tags to Ogg Vorbis: `.m4a` is a lossy,
//! taggable container that the pipeline (tagger, quality probe, organizer)
//! already handles, and it stays honestly labelled as lossy audio.

pub mod auth;

use std::{
    io::{Seek, SeekFrom},
    path::PathBuf,
};

use async_trait::async_trait;
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::{FileId, SpotifyId, SpotifyUri};
use librespot_metadata::{audio::file::AudioFileFormat, Metadata, Track as SpotifyTrack};
use shared::{
    errors::Error,
    models::{Platform, Reference, ReferenceType, Track},
    types::SoundomeResult,
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

pub struct Spotify;

/// Extract the base62 track id from a source/provider value, accepting an
/// `open.spotify.com/track/<id>` URL, a `spotify:track:<id>` URI, or a bare id.
fn extract_track_id(value: &str) -> SoundomeResult<String> {
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

#[async_trait]
impl Provider for Spotify {
    /// The provider *is* the Spotify track: no matching is needed, so this just
    /// echoes the source as a Spotify provider reference.
    async fn search(&self, track: &Track) -> SoundomeResult<Reference> {
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
    ) -> SoundomeResult<PathBuf> {
        let track_id = extract_track_id(url)?;
        let spotify_id = SpotifyId::from_base62(&track_id)
            .map_err(|e| Error::Custom(format!("invalid Spotify track id '{track_id}': {e:?}")))?;

        let session = auth::connect_session().await?;

        let track = SpotifyTrack::get(&session, &SpotifyUri::Track { id: spotify_id })
            .await
            .map_err(|e| Error::Custom(format!("Spotify track metadata failed: {e}")))?;

        let file_id: FileId = PREFERRED_FORMATS
            .iter()
            .find_map(|format| track.files.0.get(format).copied())
            .ok_or_else(|| {
                Error::Custom(format!(
                    "no Ogg Vorbis stream available for Spotify track '{track_id}' \
                     (region-restricted or Premium required)"
                ))
            })?;

        let key = session
            .audio_key()
            .request(spotify_id, file_id)
            .await
            .map_err(|e| Error::Custom(format!("Spotify audio key request failed: {e}")))?;

        let audio = AudioFile::open(&session, file_id, BYTES_PER_SECOND)
            .await
            .map_err(|e| Error::Custom(format!("Spotify audio stream open failed: {e}")))?;

        let ogg_path = base_library_dir.join(format!("{file_name}.ogg"));
        let write_path = ogg_path.clone();

        // Decrypt + drain on a blocking thread: `AudioDecrypt`'s Read blocks the
        // caller while the async fetch task (driven by `session`) fills buffers.
        tokio::task::spawn_blocking(move || -> SoundomeResult<()> {
            let mut decrypted = AudioDecrypt::new(Some(key), audio);
            decrypted
                .seek(SeekFrom::Start(SPOTIFY_OGG_HEADER_END))
                .map_err(|e| Error::Custom(format!("Spotify stream seek failed: {e}")))?;
            let mut out = std::fs::File::create(&write_path)?;
            std::io::copy(&mut decrypted, &mut out)?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Custom(format!("Spotify download task failed: {e}")))??;

        // Keep `session` alive until the stream is fully drained above.
        drop(session);

        // No transcode. The stream is already 320 kbps Vorbis, and re-encoding
        // it to AAC would lose quality for nothing: the tagger writes Vorbis
        // comments directly (see `tagger::ogg`).
        Ok(ogg_path)
    }

    fn is_valid_url(url: &str) -> bool {
        url.contains("open.spotify.com/track/")
    }
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
