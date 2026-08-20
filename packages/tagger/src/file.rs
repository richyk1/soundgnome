use audiotags::{AudioTag, Tag};
use id3::TagLike;
use shared::{
    errors::Error,
    models::{Album, Artist, Track},
    types::SoundgnomeResult,
};
use std::{path::PathBuf, str::FromStr};

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

    let mut tag = Tag::new()
        .read_from_path(file_path)
        .map_err(|e| Error::Custom(format!("Error reading audio tags: {:?}", e)))?;
    convert_track_to_tag(&mut tag, track);

    if let Some(bytes) = cover_bytes {
        tag.set_album_cover(audiotags::Picture {
            mime_type: audiotags::MimeType::Jpeg,
            data: bytes,
        });
    }

    tag.write_to_path(file_path.display().to_string().as_str())
        .map_err(|e| Error::Custom(format!("Error writing audio tags: {:?}", e)))?;

    // Write the SOUNDOME_ID custom tag if present
    if let Some(ref sid) = track.soundome_id {
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

fn convert_track_to_tag(tag: &mut Box<dyn AudioTag + Send + Sync>, track: &Track) {
    tag.set_title(&track.title);
    tag.set_artist(
        track
            .artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<&str>>()
            .join(";")
            .as_str(),
    );
    if let Some(album) = track.album.as_ref() {
        tag.set_album_title(album.title.as_str());
        tag.set_album_artist(
            album
                .artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
                .as_str(),
        );
    }
    if let Some(genre) = track.genre.as_ref() {
        tag.set_genre(genre);
    }
    if let Some(date) = track.date.as_ref() {
        tag.set_date(id3::Timestamp::from_str(date).unwrap_or(id3::Timestamp::default()))
    }
    if let Some(track_number) = track.track_number.as_ref() {
        tag.set_track_number(*track_number as u16);
    }
    if let Some(disc_number) = track.disc_number.as_ref() {
        tag.set_disc_number(*disc_number as u16);
    }
    // tag.album_cover()

    tag.set_comment(
        "Downloaded by Soundgnome\n---".to_string(), // + "\nSource: "
                                                     // + track
                                                     //     .source
                                                     //     .as_ref()
                                                     //     .unwrap_or(&TrackSource::Unknown)
                                                     //     .as_ref()
                                                     // + "\nProvider: "
                                                     // + track
                                                     //     .provider
                                                     //     .as_ref()
                                                     //     .unwrap_or(&TrackProvider::Unknown)
                                                     //     .as_ref(),
    );
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
