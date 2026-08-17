use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<i32>,
    pub task_type: TaskType,
    pub status: TaskStatus,
    /// JSON payload (e.g. `{"url": "..."}`)
    pub payload: String,
    pub label: Option<String>,
    /// Number of items processed so far.
    pub progress: i32,
    /// Total number of items, if known.
    pub total: Option<i32>,
    pub error: Option<String>,
    /// Per-category breakdown updated live during sync.
    pub stats: Option<TaskStats>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

/// Live per-category counters persisted as JSON in the `stats` column.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskStats {
    /// Tracks fully downloaded and moved to library.
    pub downloaded: i32,
    /// Tracks saved as "needs_validation" (staged, awaiting approval).
    pub to_validate: i32,
    /// Tracks already in library — linked to playlist/artist but not re-downloaded.
    pub skipped: i32,
    /// Per-track failures that did not abort the whole sync.
    pub errors: Vec<TaskTrackError>,
    /// Tracks saved as needs_validation, with details for display and navigation.
    #[serde(default)]
    pub to_validate_tracks: Vec<TaskTrackValidation>,
    /// Tracks skipped as duplicates of existing library tracks (ingest), with details.
    #[serde(default)]
    pub skipped_tracks: Vec<TaskTrackValidation>,
    /// Live progress of an in-flight AI metadata curation phase (e.g. SoundCloud batch
    /// cleanup), if one is currently running. `None` once the phase completes.
    #[serde(default)]
    pub ai_curation: Option<AiCurationProgress>,
    /// Live progress of an in-flight library maintenance backfill (acoustic
    /// fingerprinting or artwork embedding). `None` for sync/ingest tasks.
    #[serde(default)]
    pub backfill: Option<BackfillProgress>,
}

/// Live progress of an AI metadata curation batch loop (title/artist cleanup).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCurationProgress {
    /// Number of tracks processed so far across completed batches.
    pub processed: i32,
    /// Total number of tracks to curate in this phase.
    pub total: i32,
}

/// Live progress of a one-shot library maintenance backfill (fingerprint / artwork).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillProgress {
    /// Which backfill this is: `"fingerprint"` or `"artwork"`.
    pub kind: String,
    /// Items successfully processed (fingerprinted / artwork embedded).
    pub ok: i32,
    /// Items intentionally skipped (already done, no source art, no file).
    pub skipped: i32,
    /// Items that errored without aborting the run.
    pub errors: i32,
}

/// One entry per track saved as "needs_validation" during a sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTrackValidation {
    /// Human-readable display name of the track (e.g. "Artist - Title").
    pub track: String,
    /// DB id of the saved track, for linking to the Validations page.
    pub track_id: Option<i32>,
    /// Reason the track needs validation (e.g. "soundcloud_drm_protected").
    pub reason: Option<String>,
}

/// One entry per track that failed during a sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTrackError {
    /// Human-readable display name of the track (e.g. "Artist - Title").
    pub track: String,
    /// Error message returned by the orchestrator workflow.
    pub reason: String,
    /// Track ID if available, for linking to the library.
    pub track_id: Option<i32>,
    /// Provider URL (audio source) for external link if track failed before DB save.
    pub provider_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AsRefStr)]
pub enum TaskType {
    SyncPlaylist,
    SyncArtist,
    SyncAlbum,
    DownloadTrack,
    /// Batch ingest of all audio files found in the configured `ingest_dir`.
    IngestDir,
    /// One-shot: embed cover art into every library file that lacks it.
    EmbedArtworkBackfill,
    /// One-shot: compute an acoustic fingerprint for every library file that lacks one.
    FingerprintBackfill,
}

impl TaskType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "SyncPlaylist" => TaskType::SyncPlaylist,
            "SyncArtist" => TaskType::SyncArtist,
            "SyncAlbum" => TaskType::SyncAlbum,
            "DownloadTrack" => TaskType::DownloadTrack,
            "IngestDir" => TaskType::IngestDir,
            "EmbedArtworkBackfill" => TaskType::EmbedArtworkBackfill,
            "FingerprintBackfill" => TaskType::FingerprintBackfill,
            _ => TaskType::DownloadTrack,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AsRefStr)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Pending" => TaskStatus::Pending,
            "Running" => TaskStatus::Running,
            "Completed" => TaskStatus::Completed,
            "Failed" => TaskStatus::Failed,
            "Cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        }
    }
}
