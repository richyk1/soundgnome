use std::sync::Arc;

use diesel::SqliteConnection;
use shared::{
    models::{Album, Artist, Playlist, SyncSchedule, Task, Track},
    types::SoundgnomeResult,
};

pub struct RepositoryLayer {
    pub track: Arc<dyn TrackRepository>,
    pub album: Arc<dyn AlbumRepository>,
    pub artist: Arc<dyn ArtistRepository>,
    pub playlist: Arc<dyn PlaylistRepository>,
    pub task: Arc<dyn TaskRepository>,
    pub sync_schedule: Arc<dyn SyncScheduleRepository>,
}

// ================================================================================================

pub trait TrackRepository: Send + Sync {
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Track>;
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>>;
    fn create(&self, conn: &mut SqliteConnection, new_track: &Track) -> SoundgnomeResult<Track>;
    fn update(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        updated_track: &Track,
    ) -> SoundgnomeResult<Track>;
    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;

    fn get_recent(&self, conn: &mut SqliteConnection, limit: i64) -> SoundgnomeResult<Vec<Track>>;
    fn get_pending_validations(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>>;
    fn get_by_url(&self, conn: &mut SqliteConnection, url: &str) -> SoundgnomeResult<Track>;
    /// Fetch stored acoustic-fingerprint references as `(track_id, encoded_fingerprint)`
    /// for tracks whose duration is within `[min_secs, max_secs]` or unknown. Used to
    /// narrow acoustic (Chromaprint) dedup candidates before pairwise comparison.
    fn fingerprint_candidates(
        &self,
        conn: &mut SqliteConnection,
        min_secs: i32,
        max_secs: i32,
    ) -> SoundgnomeResult<Vec<(i32, String)>>;
    fn count(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64>;
    fn count_pending_validations(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64>;
    fn create_references(
        &self,
        conn: &mut SqliteConnection,
        track_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    /// Replace all references for a track (delete existing, then insert provided ones)
    fn set_references(
        &self,
        conn: &mut SqliteConnection,
        track_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    /// Look up a track by its `soundome_id` anchor UUID.
    fn get_by_soundome_id(
        &self,
        conn: &mut SqliteConnection,
        soundome_id: &str,
    ) -> SoundgnomeResult<Option<Track>>;
    /// Return all tracks that have a non-null `file_path` (i.e. finalized tracks).
    fn get_all_finalized(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>>;
    /// Delete a single reference row by its own ID.
    fn delete_reference(&self, conn: &mut SqliteConnection, ref_id: i32) -> SoundgnomeResult<()>;
    // /// Find a track by unique fields (e.g. title + artists + album)
    // fn find_by_unique_fields(&self, conn: &mut SqliteConnection, track: &Track) -> SoundgnomeResult<Option<Track>>;
}

pub trait AlbumRepository: Send + Sync {
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Album>;
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Album>>;
    fn create(&self, conn: &mut SqliteConnection, new_album: &Album) -> SoundgnomeResult<Album>;
    fn update(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        updated_album: &Album,
    ) -> SoundgnomeResult<Album>;
    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;

    fn get_by_url(&self, conn: &mut SqliteConnection, url: &str) -> SoundgnomeResult<Album>;
    fn create_references(
        &self,
        conn: &mut SqliteConnection,
        album_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    fn create_or_ignore(
        &self,
        conn: &mut SqliteConnection,
        album: &Album,
    ) -> SoundgnomeResult<Album>;
    fn count(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64>;
    /// Count the number of tracks linked to this album.
    fn count_tracks(&self, conn: &mut SqliteConnection, album_id: i32) -> SoundgnomeResult<i64>;
    /// Find an existing album by title **and** primary artist name(s).
    ///
    /// Returns the first album whose normalised title matches **and** whose
    /// artist set intersects with `artist_names` (case-insensitive).  Returns
    /// `None` when no such album exists.
    fn find_by_title_and_artists(
        &self,
        conn: &mut SqliteConnection,
        title: &str,
        artist_names: &[String],
    ) -> SoundgnomeResult<Option<Album>>;
    /// Replace all references for an album (delete existing, then insert provided ones)
    fn set_references(
        &self,
        conn: &mut SqliteConnection,
        album_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    /// Delete a single reference row by its own ID.
    fn delete_reference(&self, conn: &mut SqliteConnection, ref_id: i32) -> SoundgnomeResult<()>;
    // /// Find an album by unique fields (e.g. title + artists + date)
    // fn find_by_unique_fields(&self, conn: &mut SqliteConnection, album: &Album) -> SoundgnomeResult<Option<Album>>;
    /// Merge all source albums into `target_id`: re-point tracks, artist relations, and references, then delete sources.
    fn merge_into(
        &self,
        conn: &mut SqliteConnection,
        source_ids: &[i32],
        target_id: i32,
    ) -> SoundgnomeResult<()>;
}

pub trait ArtistRepository: Send + Sync {
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Artist>;
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Artist>>;
    fn create(&self, conn: &mut SqliteConnection, new_artist: &Artist) -> SoundgnomeResult<Artist>;
    fn update(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        updated_artist: &Artist,
    ) -> SoundgnomeResult<Artist>;
    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;

    fn get_by_url(&self, conn: &mut SqliteConnection, url: &str) -> SoundgnomeResult<Artist>;
    fn create_references(
        &self,
        conn: &mut SqliteConnection,
        artist_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    fn create_track_relationship(
        &self,
        conn: &mut SqliteConnection,
        artist_id: i32,
        track_id: i32,
    ) -> SoundgnomeResult<()>;
    fn create_album_relationship(
        &self,
        conn: &mut SqliteConnection,
        artist_id: i32,
        album_id: i32,
    ) -> SoundgnomeResult<()>;
    fn create_or_ignore(
        &self,
        conn: &mut SqliteConnection,
        artist: &Artist,
    ) -> SoundgnomeResult<Artist>;
    /// Replace all references for an artist (delete existing, then insert provided ones)
    fn set_references(
        &self,
        conn: &mut SqliteConnection,
        artist_id: i32,
        references: &[shared::models::Reference],
    ) -> SoundgnomeResult<()>;
    /// Replace all artists attached to a given track
    fn set_track_artists(
        &self,
        conn: &mut SqliteConnection,
        track_id: i32,
        artist_ids: &[i32],
    ) -> SoundgnomeResult<()>;
    /// Replace all artists attached to a given album
    fn set_album_artists(
        &self,
        conn: &mut SqliteConnection,
        album_id: i32,
        artist_ids: &[i32],
    ) -> SoundgnomeResult<()>;
    /// Merge all source artists into `target_id`: re-point tracks, albums, and references, then delete sources.
    fn merge_into(
        &self,
        conn: &mut SqliteConnection,
        source_ids: &[i32],
        target_id: i32,
    ) -> SoundgnomeResult<()>;
    fn count(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64>;
    /// Count the number of tracks linked to this artist (via artist_tracks).
    fn count_tracks(&self, conn: &mut SqliteConnection, artist_id: i32) -> SoundgnomeResult<i64>;
    /// Delete a single reference row by its own ID.
    fn delete_reference(&self, conn: &mut SqliteConnection, ref_id: i32) -> SoundgnomeResult<()>;
    // /// Find an artist by unique fields (e.g. name)
    // fn find_by_unique_fields(&self, conn: &mut SqliteConnection, artist: &Artist) -> SoundgnomeResult<Option<Artist>>;
}

// ================================================================================================

pub trait PlaylistRepository: Send + Sync {
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Playlist>>;
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Playlist>;
    /// Returns `None` if no playlist with this source URL exists yet.
    fn get_by_source_url(
        &self,
        conn: &mut SqliteConnection,
        url: &str,
    ) -> SoundgnomeResult<Option<Playlist>>;
    fn create(
        &self,
        conn: &mut SqliteConnection,
        playlist: &Playlist,
    ) -> SoundgnomeResult<Playlist>;
    fn update_last_sync(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    /// Link a track to a playlist. Silently ignores duplicate entries.
    fn add_track(
        &self,
        conn: &mut SqliteConnection,
        playlist_id: i32,
        track_id: i32,
        position: Option<i32>,
    ) -> SoundgnomeResult<()>;
    /// Return all finalized tracks belonging to a playlist, ordered by position.
    fn get_tracks(
        &self,
        conn: &mut SqliteConnection,
        playlist_id: i32,
    ) -> SoundgnomeResult<Vec<Track>>;
    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    fn count(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64>;
}

// ================================================================================================

pub trait TaskRepository: Send + Sync {
    fn create(&self, conn: &mut SqliteConnection, task: &Task) -> SoundgnomeResult<Task>;
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Task>;
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Task>>;
    fn set_running(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    fn update_progress(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        progress: i32,
        total: i32,
    ) -> SoundgnomeResult<()>;
    fn set_completed(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    fn set_failed(&self, conn: &mut SqliteConnection, id: i32, error: &str)
        -> SoundgnomeResult<()>;
    fn set_cancelled(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    fn get_by_status(
        &self,
        conn: &mut SqliteConnection,
        status: &str,
    ) -> SoundgnomeResult<Vec<Task>>;
    fn reset_for_retry(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    fn count_by_status(&self, conn: &mut SqliteConnection, status: &str) -> SoundgnomeResult<i64>;
    /// Update the task label in-place (e.g. to the fetched playlist/artist/album name).
    fn update_label(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        label: &str,
    ) -> SoundgnomeResult<()>;
    /// Persist the live per-category stats (downloaded / to_validate / skipped / errors).
    fn update_stats(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        stats: &shared::models::TaskStats,
    ) -> SoundgnomeResult<()>;
}

// ================================================================================================

pub trait SyncScheduleRepository: Send + Sync {
    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<SyncSchedule>>;
    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<SyncSchedule>;
    fn create(
        &self,
        conn: &mut SqliteConnection,
        schedule: &SyncSchedule,
    ) -> SoundgnomeResult<SyncSchedule>;
    fn update(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        schedule: &SyncSchedule,
    ) -> SoundgnomeResult<SyncSchedule>;
    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
    /// Returns all schedules that are enabled and whose next_run is in the past (or NULL).
    fn get_due(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<SyncSchedule>>;
    /// Record that a schedule ran now and compute the next_run time.
    fn mark_ran(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()>;
}
