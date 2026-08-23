use id3::TagLike;
use shared::{
    errors::Error,
    models::{Album, Artist, Track},
    types::SoundgnomeResult,
};
use std::path::{Path, PathBuf};

// ================================================================================================
// SOUNDOME_ID custom-tag constants
// ================================================================================================

const SOUNDOME_ID_KEY: &str = "SOUNDOME_ID";
const MP4_MEAN: &str = "com.soundgnome";
const MP4_NAME: &str = "ID";

// ================================================================================================
// Public API
// ================================================================================================

/**
 * Reads the tag from a file and returns a converted Track object.
 */
pub fn get_track_from_file(file_path: &PathBuf) -> SoundgnomeResult<Track> {
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::probe::Probe;

    tracing::info!("Reading tag from file: {:?}", file_path);

    // lofty reads every container we ingest (opus/ogg/wav included), unlike
    // audiotags which only handles mp3/flac/m4a. Readable-but-untagged files come
    // back with no tag and fall back to a filename-derived title.
    let tagged = Probe::open(file_path)
        .map_err(|e| Error::Custom(format!("Cannot open audio file: {e}")))?
        .read()
        .map_err(|e| Error::Custom(format!("Error reading audio file: {e:?}")))?;

    let duration = {
        let secs = tagged.properties().duration().as_secs();
        (secs > 0).then_some(secs as i32)
    };
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let mut track = lofty_tag_to_track(tag, duration, file_path);

    // Best-effort: read the SOUNDOME_ID custom tag
    track.soundome_id = read_soundome_id_from_file(file_path);

    Ok(track)
}

/// Extract the embedded cover picture from an audio file, if any. Returns the
/// raw image bytes and its MIME type (e.g. `"image/png"`). Prefers the front
/// cover, falling back to the first embedded picture. Reads via lofty, which
/// covers every container we ingest. Best-effort: unreadable files yield `None`.
pub fn read_cover_from_path(file_path: &Path) -> Option<(Vec<u8>, String)> {
    use lofty::file::TaggedFileExt;
    use lofty::picture::PictureType;
    use lofty::probe::Probe;

    let tagged = Probe::open(file_path).ok()?.read().ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let pics = tag.pictures();
    let pic = pics
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pics.first())?;
    let mime = pic
        .mime_type()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());
    Some((pic.data().to_vec(), mime))
}

/// Downscale raw image bytes to a small JPEG thumbnail no larger than `max_px`
/// on its long edge, preserving aspect ratio. Serves fast list-sized covers
/// instead of the multi-megabyte artwork embedded in downloads. Returns `None`
/// if the bytes are not a decodable image.
pub fn make_thumbnail(bytes: &[u8], max_px: u32) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let img = image::load_from_memory(bytes).ok()?;
    // `thumbnail` is fast and preserves aspect ratio, fitting within max_px^2.
    let thumb = img.thumbnail(max_px, max_px).to_rgb8();
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(thumb)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .ok()?;
    Some(out.into_inner())
}

/**
 * Tag an audio file with the provided track information.
 * Also writes the SOUNDOME_ID custom tag when `track.soundome_id` is set.
 * Optionally writes cover art when `cover_bytes` is provided.
 */
pub fn tag_file_with_track(file_path: &PathBuf, track: &Track) -> SoundgnomeResult<()> {
    tag_file_with_track_and_cover(file_path, track, None)
}

/// Like `tag_file_with_track` but also embeds raw cover art bytes.
pub fn tag_file_with_track_and_cover(
    file_path: &PathBuf,
    track: &Track,
    cover_bytes: Option<&[u8]>,
) -> SoundgnomeResult<()> {
    // Ogg carries Vorbis comments, which audiotags cannot write.
    if crate::ogg::handles(file_path) {
        return crate::ogg::tag_file(file_path, track, cover_bytes, track.soundome_id.as_deref());
    }

    use lofty::config::WriteOptions;
    use lofty::file::TaggedFileExt;
    use lofty::prelude::TagExt;

    let mut tagged = lofty::probe::Probe::open(file_path)
        .map_err(|e| Error::Custom(format!("Cannot open audio file: {e}")))?
        .read()
        .map_err(|e| Error::Custom(format!("Error reading audio file: {e:?}")))?;

    // Create a tag of the file's native type when it has none yet (a freshly
    // downloaded WAV or an untagged MP3), then fill it in. lofty writes every
    // container we ingest, including WAV, which audiotags could not.
    if tagged.primary_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .expect("a primary tag exists or was just inserted");

    apply_track_to_tag(tag, track);

    if let Some(bytes) = cover_bytes {
        let picture = lofty::picture::Picture::new_unchecked(
            lofty::picture::PictureType::CoverFront,
            Some(lofty::picture::MimeType::Jpeg),
            None,
            bytes.to_vec(),
        );
        tag.set_picture(0, picture);
    }

    tag.save_to_path(file_path, WriteOptions::default())
        .map_err(|e| Error::Custom(format!("Error writing audio tags: {e:?}")))?;

    // Write the SOUNDOME_ID custom tag if present
    if let Some(sid) = &track.soundome_id {
        write_soundome_id_tag(file_path, sid)?;
    }

    Ok(())
}

/// Write `SOUNDOME_ID` as a custom tag frame into the file.
///
/// | Format    | Tag frame              |
/// |-----------|------------------------|
/// | MP3 / ID3 | `TXXX:SOUNDOME_ID`     |
/// | FLAC      | Vorbis comment         |
/// | MP4 / M4A | `----:com.soundgnome:ID` |
pub fn write_soundome_id_tag(file_path: &PathBuf, soundome_id: &str) -> SoundgnomeResult<()> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "mp3" => write_soundome_id_id3(file_path, soundome_id),
        "flac" => write_soundome_id_flac(file_path, soundome_id),
        "m4a" | "mp4" | "aac" => write_soundome_id_mp4(file_path, soundome_id),
        "ogg" | "oga" | "opus" => crate::ogg::write_soundome_id(file_path, soundome_id),
        // For unknown / unsupported formats log a warning and continue.
        other => {
            tracing::warn!(
                "Cannot write SOUNDOME_ID: unsupported extension {:?} for {:?}",
                other,
                file_path
            );
            Ok(())
        }
    }
}

/// Read `SOUNDOME_ID` from the custom tag of an audio file.
/// Returns `None` if the tag is absent or the format is unsupported.
pub fn read_soundome_id_from_file(file_path: &PathBuf) -> Option<String> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())?;

    match ext.as_str() {
        "mp3" => read_soundome_id_id3(file_path),
        "flac" => read_soundome_id_flac(file_path),
        "m4a" | "mp4" | "aac" => read_soundome_id_mp4(file_path),
        "ogg" | "oga" | "opus" => crate::ogg::read_soundome_id(file_path),
        _ => None,
    }
}

// ================================================================================================
// Format-specific helpers
// ================================================================================================

fn write_soundome_id_id3(file_path: &PathBuf, soundome_id: &str) -> SoundgnomeResult<()> {
    let mut tag = id3::Tag::read_from_path(file_path).unwrap_or_else(|e| {
        tracing::warn!(
            "Could not read existing ID3 tags from {:?}, will create new tag: {}",
            file_path,
            e
        );
        id3::Tag::default()
    });

    // Remove any existing SOUNDOME_ID TXXX frame to avoid duplicates.
    tag.remove_extended_text(Some(SOUNDOME_ID_KEY), None);

    tag.add_frame(id3::frame::ExtendedText {
        description: SOUNDOME_ID_KEY.to_string(),
        value: soundome_id.to_string(),
    });

    tag.write_to_path(file_path, id3::Version::Id3v24)
        .map_err(|e| Error::Custom(format!("Failed to write ID3 TXXX frame: {}", e)))
}

fn read_soundome_id_id3(file_path: &PathBuf) -> Option<String> {
    let tag = id3::Tag::read_from_path(file_path).ok()?;
    let value = tag
        .extended_texts()
        .find(|t| t.description == SOUNDOME_ID_KEY)
        .map(|t| t.value.clone());
    value
}

fn write_soundome_id_flac(file_path: &PathBuf, soundome_id: &str) -> SoundgnomeResult<()> {
    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| Error::Custom(format!("Failed to read FLAC tags: {}", e)))?;

    let comments = tag.vorbis_comments_mut();
    // Replace any existing entry
    comments.remove(SOUNDOME_ID_KEY);
    comments.set(SOUNDOME_ID_KEY, vec![soundome_id.to_string()]);

    tag.write_to_path(file_path)
        .map_err(|e| Error::Custom(format!("Failed to write FLAC Vorbis comment: {}", e)))
}

fn read_soundome_id_flac(file_path: &PathBuf) -> Option<String> {
    let tag = metaflac::Tag::read_from_path(file_path).ok()?;
    tag.vorbis_comments()
        .and_then(|vc| vc.get(SOUNDOME_ID_KEY))
        .and_then(|v| v.first())
        .cloned()
}

fn write_soundome_id_mp4(file_path: &PathBuf, soundome_id: &str) -> SoundgnomeResult<()> {
    let mut tag = mp4ameta::Tag::read_from_path(file_path)
        .map_err(|e| Error::Custom(format!("Failed to read MP4 tags: {}", e)))?;

    let fourcc = mp4ameta::FreeformIdent::new(MP4_MEAN, MP4_NAME);
    tag.remove_data_of(&fourcc);
    tag.set_data(fourcc, mp4ameta::Data::Utf8(soundome_id.to_string()));

    tag.write_to_path(file_path)
        .map_err(|e| Error::Custom(format!("Failed to write MP4 freeform atom: {}", e)))
}

fn read_soundome_id_mp4(file_path: &PathBuf) -> Option<String> {
    let tag = mp4ameta::Tag::read_from_path(file_path).ok()?;
    let fourcc = mp4ameta::FreeformIdent::new(MP4_MEAN, MP4_NAME);
    let value = tag.strings_of(&fourcc).next().map(|s| s.to_string());
    value
}

// ================================================================================================
// Mappers
// ================================================================================================

fn apply_track_to_tag(tag: &mut lofty::tag::Tag, track: &Track) {
    use lofty::prelude::Accessor;
    use lofty::tag::ItemKey;

    tag.set_title(track.title.clone());

    let artist = track
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(";");
    if !artist.is_empty() {
        tag.set_artist(artist);
    }

    if let Some(album) = track.album.as_ref() {
        tag.set_album(album.title.clone());
        let album_artist = album
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if !album_artist.is_empty() {
            tag.insert_text(ItemKey::AlbumArtist, album_artist);
        }
    }

    if let Some(genre) = track.genre.as_ref() {
        tag.set_genre(genre.clone());
    }

    if let Some(date) = track.date.as_ref() {
        if let Some(year) = date.get(0..4).and_then(|y| y.parse::<u32>().ok()) {
            tag.set_year(year);
        }
        tag.insert_text(ItemKey::RecordingDate, date.clone());
    }

    if let Some(track_number) = track.track_number {
        tag.set_track(track_number as u32);
    }
    if let Some(disc_number) = track.disc_number {
        tag.set_disk(disc_number as u32);
    }

    tag.set_comment("Downloaded by Soundgnome\n---".to_string());
}

/// Split a raw artist string on the standard multi-artist tag delimiters (`/` for
/// ID3, `;` for others). Deliberately not `,`/`&` to avoid mangling real names.
fn split_artist_names(raw: &str) -> Vec<String> {
    raw.split(['/', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn lofty_tag_to_track(
    tag: Option<&lofty::tag::Tag>,
    duration: Option<i32>,
    file_path: &PathBuf,
) -> Track {
    use lofty::prelude::{Accessor, ItemKey};

    // Title falls back to the file name so untagged files still get a usable name
    // (they land in the review queue for the user to fix).
    let title = tag
        .and_then(|t| t.title())
        .map(|c| c.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            file_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        });

    // Opus/Vorbis repeat the ARTIST key; ID3/MP4 join with `/` or `;`. Handle both.
    let artists: Vec<Artist> = tag
        .map(|t| {
            let mut names: Vec<String> = t
                .get_strings(&ItemKey::TrackArtist)
                .flat_map(split_artist_names)
                .collect();
            if names.is_empty() {
                if let Some(a) = t.artist() {
                    names = split_artist_names(&a);
                }
            }
            names
        })
        .unwrap_or_default()
        .into_iter()
        .map(|name| Artist {
            id: None,
            name,
            icon: None,
            references: Vec::new(),
        })
        .collect();

    let date = tag.and_then(|t| {
        t.get_string(&ItemKey::RecordingDate)
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| t.year().map(|y| format!("{y:04}")))
    });

    let album = tag
        .and_then(|t| t.album())
        .map(|a| a.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|album_title| Album {
            id: None,
            title: album_title,
            artists: tag
                .and_then(|t| t.get_string(&ItemKey::AlbumArtist))
                .map(split_artist_names)
                .unwrap_or_default()
                .into_iter()
                .map(|name| Artist {
                    id: None,
                    name,
                    icon: None,
                    references: Vec::new(),
                })
                .collect(),
            album_type: shared::models::AlbumType::Unknown,
            date: date.clone(),
            cover: None,
            references: Vec::new(),
        });

    Track {
        id: None,
        needs_validation: false,
        validation_reason: None,
        soundome_id: None,
        title,
        artists,
        album,
        genre: tag
            .and_then(|t| t.genre())
            .map(|g| g.trim().to_string())
            .filter(|s| !s.is_empty()),
        date,
        cover: None,
        disc_number: tag.and_then(|t| t.disk()).map(|d| d as i32),
        track_number: tag.and_then(|t| t.track()).map(|n| n as i32),
        duration,
        label: None,
        file_path: None,
        references: Vec::new(),
    }
}
