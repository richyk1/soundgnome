//! Memoised audio-quality probes.
//!
//! Probing is cheap per file (a few milliseconds of header parsing) but the
//! library list returns every track at once, so the cost scales with the
//! library: roughly 2.7 seconds for 600 tracks on every page load.
//!
//! Results are cached per path and invalidated by modification time and size,
//! so a re-tagged, re-encoded or replaced file is re-probed automatically while
//! an unchanged one is free.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use shared::models::{AudioQuality, Track};

/// What the cached entry was derived from. If either changes, the file is not
/// the one we measured.
#[derive(PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

struct Entry {
    stamp: FileStamp,
    /// `None` records "probed and unreadable", so a broken file is not
    /// re-probed on every request either.
    quality: Option<AudioQuality>,
}

static CACHE: LazyLock<Mutex<HashMap<PathBuf, Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn stamp_of(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(FileStamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

/// Quality of the track's audio file, from cache when the file is unchanged.
pub fn probe(track: &Track) -> Option<AudioQuality> {
    let path = track.file_path.as_ref()?;
    let stamp = stamp_of(path)?;

    // A poisoned lock would mean a panic mid-probe. Recover rather than
    // propagate: a stale cache is not worth failing a request over.
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(entry) = cache.get(path) {
        if entry.stamp == stamp {
            return entry.quality;
        }
    }

    let quality = track.audio_quality();
    cache.insert(path.clone(), Entry { stamp, quality });
    quality
}
