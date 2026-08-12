//! Ogg Vorbis and Opus tagging.
//!
//! `audiotags` covers MP3, FLAC and MP4, which was enough while every download
//! ended up in one of those. Spotify audio arrives as Ogg Vorbis, and
//! transcoding it just to make it taggable would throw away quality for no
//! reason, so Ogg is tagged directly here.
//!
//! This works against lofty's concrete [`VorbisComments`] type rather than its
//! generic `Tag`: the generic API silently drops arbitrary keys, which is where
//! `SOUNDOME_ID` has to live.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::ogg::{OggPictureStorage, VorbisComments};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::TagExt;
use lofty::probe::Probe;
use shared::{errors::Error, models::Track, types::SoundomeResult};

/// Extensions handled here rather than by `audiotags`.
pub const EXTENSIONS: [&str; 3] = ["ogg", "oga", "opus"];

/// Vorbis comment field holding the library anchor.
const SOUNDOME_ID_KEY: &str = "SOUNDOME_ID";

pub fn handles(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Read the existing comments, or start a fresh block.
fn read_comments(path: &Path) -> SoundomeResult<VorbisComments> {
    let tagged = Probe::open(path)
        .map_err(|e| Error::Custom(format!("Cannot open {}: {e}", path.display())))?
        .read()
        .map_err(|e| Error::Custom(format!("Cannot read tags from {}: {e}", path.display())))?;

    Ok(tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .cloned()
        .map(VorbisComments::from)
        .unwrap_or_default())
}

fn save(path: &Path, comments: &VorbisComments) -> SoundomeResult<()> {
    comments
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| Error::Custom(format!("Cannot write tags to {}: {e}", path.display())))
}

/// Replace every value of a multi-valued field.
fn set_all(comments: &mut VorbisComments, key: &str, values: &[String]) {
    let _ = comments.remove(key).count();
    for value in values {
        // Vorbis comments repeat the key rather than joining with a separator,
        // which is how multi-artist tracks stay machine readable.
        comments.push(key.to_string(), value.clone());
    }
}

/// Write track metadata, and optionally cover art.
pub fn tag_file(
    path: &Path,
    track: &Track,
    cover_bytes: Option<&[u8]>,
    soundome_id: Option<&str>,
) -> SoundomeResult<()> {
    let mut comments = read_comments(path)?;

    comments.insert("TITLE".to_string(), track.title.clone());

    let artists: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
    if !artists.is_empty() {
        set_all(&mut comments, "ARTIST", &artists);
    }

    if let Some(album) = &track.album {
        comments.insert("ALBUM".to_string(), album.title.clone());
        let album_artists: Vec<String> = album.artists.iter().map(|a| a.name.clone()).collect();
        if !album_artists.is_empty() {
            set_all(&mut comments, "ALBUMARTIST", &album_artists);
        }
    }

    if let Some(genre) = &track.genre {
        comments.insert("GENRE".to_string(), genre.clone());
    }
    if let Some(date) = &track.date {
        comments.insert("DATE".to_string(), date.clone());
    }
    if let Some(number) = track.track_number {
        comments.insert("TRACKNUMBER".to_string(), number.to_string());
    }
    if let Some(number) = track.disc_number {
        comments.insert("DISCNUMBER".to_string(), number.to_string());
    }
    if let Some(label) = &track.label {
        // lofty (and Picard) round-trip the label as LABEL; ORGANIZATION is
        // remapped on read and would not survive.
        comments.insert("LABEL".to_string(), label.clone());
    }
    if let Some(id) = soundome_id {
        comments.insert(SOUNDOME_ID_KEY.to_string(), id.to_string());
    }

    if let Some(bytes) = cover_bytes {
        comments.remove_picture_type(PictureType::CoverFront);
        let picture = Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Jpeg),
            None,
            bytes.to_vec(),
        );
        // Dimensions are optional metadata; a zeroed block is valid and avoids
        // decoding the image just to tag it.
        comments
            .insert_picture(picture, None)
            .map_err(|e| Error::Custom(format!("Cannot attach cover art: {e}")))?;
    }

    save(path, &comments)
}

/// Write only the library anchor, leaving every other field as it is.
pub fn write_soundome_id(path: &Path, soundome_id: &str) -> SoundomeResult<()> {
    let mut comments = read_comments(path)?;
    comments.insert(SOUNDOME_ID_KEY.to_string(), soundome_id.to_string());
    save(path, &comments)
}

/// Read the library anchor back, if present.
pub fn read_soundome_id(path: &Path) -> Option<String> {
    read_comments(path)
        .ok()?
        .get(SOUNDOME_ID_KEY)
        .map(str::to_string)
}
