use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

use config::Config;
use diesel::SqliteConnection;
use fetcher::{curate_source_url, Fetcher, Source};
use rusty_chromaprint::{match_fingerprints, Configuration, Fingerprinter};
use sha2::{Digest, Sha256};
use shared::models::ReferenceType;
use shared::{
    errors::Error,
    models::{Album, AlbumType, Artist, Platform, Playlist, Reference, TaskTrackValidation, Track},
    types::SoundgnomeResult,
    utils::enums::Match,
    utils::fs::sanitize_filename,
};
use uuid::Uuid;

use super::{
    album_service::AlbumService,
    artist_service::ArtistService,
    playlist_service::PlaylistService,
    task_service::TaskService,
    track_service::{TrackService, ValidationPatch},
};
pub use tagger::enricher::MatchCandidate;

/// Outcome of ingesting one local file, so the batch job can categorize results
/// (new vs. duplicate vs. needs-review) for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    /// New track added to the library.
    New,
    /// Matched an existing track; the new file was better and replaced it.
    Replaced,
    /// Matched an existing track; kept the existing audio (metadata merged).
    Duplicate,
    /// Saved for manual validation (enrichment inconclusive).
    NeedsValidation,
}

/// A collision-free staging file name. Many tracks share a sanitized title
/// (e.g. "Lust", "Intro", "Stone cold."), so staging under the title alone
/// lets a later download clobber an earlier track's staged file before it is
/// organized, which then fails the move with "No such file or directory".
/// The final library name stays title-based (see `organizer::move_track_file`),
/// so this uuid prefix never reaches the library.
fn staging_name(title: &str) -> String {
    format!("{}-{}", Uuid::new_v4(), sanitize_filename(title))
}

/// Extract an 11-char YouTube video id from a watch/short URL, if present.
fn youtube_video_id(url: &str) -> Option<String> {
    let take_id = |s: &str| -> Option<String> {
        let id: String = s
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        (id.len() == 11).then_some(id)
    };
    if let Some(i) = url.find("v=") {
        if let Some(id) = take_id(&url[i + 2..]) {
            return Some(id);
        }
    }
    if let Some(i) = url.find("youtu.be/") {
        if let Some(id) = take_id(&url[i + 9..]) {
            return Some(id);
        }
    }
    None
}

/// The API endpoint that serves a track's embedded cover art (see the
/// `GET /tracks/<id>/cover` route). Used as the `cover` URL for locally-ingested
/// files whose artwork is stored inside the audio container rather than at an
/// external URL.
fn embedded_cover_url(id: i32) -> String {
    format!("/api/tracks/{id}/cover")
}

/// Best-effort cover-art URL for a track whose source metadata carried none,
/// derived from its references. YouTube -> thumbnail built from the video id;
/// Spotify -> album art via the public, auth-free oEmbed endpoint. Used so
/// downloads can embed artwork into the file (offline-safe) rather than relying
/// on the client to fetch it at play time.
async fn resolve_cover_url(track: &Track) -> Option<String> {
    for r in &track.references {
        let url = r.external_url.as_deref().unwrap_or_default();
        if url.contains("youtube.com") || url.contains("youtu.be") {
            if let Some(id) = youtube_video_id(url) {
                return Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"));
            }
        }
    }

    let spotify_url = track.references.iter().find_map(|r| {
        let url = r.external_url.clone()?;
        url.contains("open.spotify.com/track").then_some(url)
    })?;
    tokio::task::spawn_blocking(move || -> Option<String> {
        let resp = reqwest::blocking::Client::new()
            .get("https://open.spotify.com/oembed")
            .query(&[("url", spotify_url.as_str())])
            .send()
            .ok()?
            .error_for_status()
            .ok()?;
        let json: serde_json::Value = resp.json().ok()?;
        json.get("thumbnail_url")?.as_str().map(String::from)
    })
    .await
    .ok()
    .flatten()
}

/// Result of a one-shot artwork backfill over the library.
#[derive(Debug, Default, Clone)]
pub struct ArtworkBackfillSummary {
    pub total: usize,
    pub embedded: usize,
    pub no_art: usize,
    pub no_file: usize,
    pub missing_file: usize,
    pub errors: usize,
}

/// Result of a one-shot acoustic-fingerprint backfill over the library.
#[derive(Debug, Default, Clone)]
pub struct FingerprintBackfillSummary {
    pub total: usize,
    pub fingerprinted: usize,
    pub already_had: usize,
    pub no_file: usize,
    pub errors: usize,
}

/// One duplicate cluster found by [`DownloadService::dedupe_library`]: the copy
/// that is kept and the copies removed (or that would be, in a dry run).
#[derive(Debug, serde::Serialize)]
pub struct DedupeCluster {
    pub keeper: DedupeTrack,
    pub removed: Vec<DedupeTrack>,
}

/// A track in a dedup report.
#[derive(Debug, serde::Serialize)]
pub struct DedupeTrack {
    pub id: i32,
    pub title: String,
    pub artist: String,
    pub duration: Option<i32>,
    pub quality: String,
    pub needs_validation: bool,
    pub rating: Option<String>,
    pub file_path: Option<String>,
}

/// Outcome of a library-wide acoustic dedup pass.
#[derive(Debug, serde::Serialize)]
pub struct DedupeReport {
    pub applied: bool,
    pub groups_examined: usize,
    pub clusters: Vec<DedupeCluster>,
    pub tracks_removed: usize,
    pub bytes_freed: u64,
}

/// Fetch cover-art bytes for a URL, preferring a higher-resolution variant of
/// known image hosts and falling back to the original when the upgrade is not
/// available. Runs the blocking request off the async runtime.
async fn fetch_cover_bytes(url: String) -> Option<Vec<u8>> {
    for candidate in higher_res_cover_candidates(&url) {
        let bytes = tokio::task::spawn_blocking(move || {
            reqwest::blocking::get(&candidate)
                .and_then(|r| r.error_for_status())
                .and_then(|r| r.bytes().map(|b| b.to_vec()))
                .ok()
        })
        .await
        .ok()
        .flatten();
        if bytes.is_some() {
            return bytes;
        }
    }
    None
}

/// Higher-resolution cover variants to try, in order, ending with the original.
///
/// - SoundCloud artwork (`i1.sndcdn.com/artworks-...-large.jpg`) defaults to
///   `-large` at only 100x100; the `-t<N>x<N>` tokens go far higher, so
///   `t1080x1080` then `t500x500` are tried before the original.
/// - YouTube thumbnails ship at fixed sizes; `hqdefault` is only 480x360.
///   `maxresdefault` (1280x720) exists for most music uploads but 404s on some,
///   so `sddefault` (640x480) then the original follow it.
/// - Google-hosted images (YouTube Music covers) encode size in a `=w<W>-h<H>`
///   suffix that can be requested larger; Google clamps to the source
///   resolution, so asking for 1200 is safe.
/// - Spotify images encode size in the id prefix (`00001e02` = 300,
///   `0000b273` = 640, its maximum); the 300 variant is rewritten to 640.
fn higher_res_cover_candidates(url: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    if url.contains("ytimg.com/vi/") || url.contains("img.youtube.com/vi/") {
        for name in ["maxresdefault", "sddefault", "hqdefault"] {
            if let Some(upgraded) = replace_youtube_thumb_name(url, name) {
                candidates.push(upgraded);
            }
        }
    } else if url.contains("googleusercontent.com") || url.contains("ggpht.com") {
        if let Some(upgraded) = bump_google_image_size(url, 1200) {
            candidates.push(upgraded);
        }
    } else if url.contains("sndcdn.com/artworks-") {
        for size in ["t1080x1080", "t500x500"] {
            if let Some(upgraded) = replace_soundcloud_size(url, size) {
                candidates.push(upgraded);
            }
        }
    } else if url.contains("scdn.co/image/") || url.contains("spotifycdn.com/image/") {
        if url.contains("ab67616d00001e02") {
            candidates.push(url.replace("ab67616d00001e02", "ab67616d0000b273"));
        }
    }

    if !candidates.iter().any(|c| c == url) {
        candidates.push(url.to_string());
    }
    candidates
}

/// Rewrite the size token in a YouTube thumbnail URL (`.../vi/<id>/<name>.jpg`).
fn replace_youtube_thumb_name(url: &str, name: &str) -> Option<String> {
    let (prefix, rest) = url.split_once("/vi/")?;
    let (id, _file) = rest.split_once('/')?;
    Some(format!("{prefix}/vi/{id}/{name}.jpg"))
}

/// Replace or append the `=w<W>-h<H>` size suffix on a Google-hosted image URL.
fn bump_google_image_size(url: &str, size: u32) -> Option<String> {
    let base = url.split_once('=').map(|(b, _)| b).unwrap_or(url);
    Some(format!("{base}=w{size}-h{size}-l90-rj"))
}

/// Rewrite the trailing `-<size>.<ext>` token on a SoundCloud artwork URL.
fn replace_soundcloud_size(url: &str, size: &str) -> Option<String> {
    let (path, ext) = url.rsplit_once('.')?;
    let (base, _token) = path.rsplit_once('-')?;
    Some(format!("{base}-{size}.{ext}"))
}

pub struct DownloadService {
    track_service: Arc<TrackService>,
    album_service: Arc<AlbumService>,
    artist_service: Arc<ArtistService>,
    playlist_service: Arc<PlaylistService>,
    task_service: Arc<TaskService>,
}

// TODO: manage "to validate" tracks
impl DownloadService {
    /// Persist live progress for a maintenance backfill task. No-op when there is
    /// no backing task row (e.g. an internal call).
    #[allow(clippy::too_many_arguments)]
    fn report_backfill(
        &self,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
        kind: &str,
        processed: usize,
        total: usize,
        ok: i32,
        skipped: i32,
        errors: i32,
    ) {
        let Some(tid) = task_id else {
            return;
        };
        let _ = self
            .task_service
            .update_progress(conn, tid, processed as i32, total as i32);
        let stats = shared::models::TaskStats {
            backfill: Some(shared::models::BackfillProgress {
                kind: kind.to_string(),
                ok,
                skipped,
                errors,
            }),
            ..Default::default()
        };
        let _ = self.task_service.update_stats(conn, tid, &stats);
    }

    /// One-shot maintenance pass: embed cover art into every library file that
    /// can resolve one, so the collection keeps its artwork offline. Non-
    /// destructive — it only (re)writes tags on the existing file in place and
    /// fills a missing `cover` URL in the DB; it never re-downloads audio.
    pub async fn backfill_artwork(
        &self,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
    ) -> SoundgnomeResult<ArtworkBackfillSummary> {
        let tracks = self.track_service.get_all_finalized(conn)?;
        let total = tracks.len();
        let mut s = ArtworkBackfillSummary {
            total,
            ..Default::default()
        };
        tracing::info!("Artwork backfill: starting over {} tracks", total);
        self.report_backfill(conn, task_id, "artwork", 0, total, 0, 0, 0);

        for (i, mut track) in tracks.into_iter().enumerate() {
            self.report_backfill(
                conn,
                task_id,
                "artwork",
                i,
                total,
                s.embedded as i32,
                (s.no_art + s.no_file + s.missing_file) as i32,
                s.errors as i32,
            );
            let Some(path) = track.file_path.clone() else {
                s.no_file += 1;
                continue;
            };
            if !path.exists() {
                s.missing_file += 1;
                continue;
            }

            let cover_url = match &track.cover {
                Some(u) => Some(u.clone()),
                None => resolve_cover_url(&track).await,
            };
            let Some(url) = cover_url else {
                // No external artwork URL, but the file itself may carry embedded
                // art (common for locally-ingested files). Point `cover` at the
                // on-demand endpoint instead of re-embedding what is already there.
                if tagger::file::read_cover_from_path(&path).is_some() {
                    if let Some(id) = track.id {
                        track.cover = Some(embedded_cover_url(id));
                        match self.track_service.update(conn, id, &track) {
                            Ok(_) => s.embedded += 1,
                            Err(e) => {
                                tracing::warn!(
                                    "Backfill: could not persist embedded cover for {}: {}",
                                    id,
                                    e
                                );
                                s.errors += 1;
                            }
                        }
                        continue;
                    }
                }
                s.no_art += 1;
                continue;
            };

            // If the file already carries embedded art (downloads embed it, and
            // many local imports have it too), there is nothing to fetch or
            // rewrite - just ensure `cover` points at something displayable. This
            // keeps the backfill cheap and idempotent instead of re-encoding
            // every file in the library on every run.
            if tagger::file::read_cover_from_path(&path).is_some() {
                if track.cover.is_none() {
                    if let Some(id) = track.id {
                        track.cover = Some(url);
                        if let Err(e) = self.track_service.update(conn, id, &track) {
                            tracing::warn!(
                                "Backfill: could not persist cover for {}: {}",
                                id,
                                e
                            );
                        }
                    }
                }
                s.embedded += 1;
                continue;
            }

            let Some(bytes) = fetch_cover_bytes(url.clone()).await else {
                s.errors += 1;
                continue;
            };

            match tagger::file::tag_file_with_track_and_cover(&path, &track, Some(&bytes)) {
                Ok(()) => {
                    if track.cover.is_none() {
                        if let Some(id) = track.id {
                            track.cover = Some(url);
                            if let Err(e) = self.track_service.update(conn, id, &track) {
                                tracing::warn!(
                                    "Backfill: could not persist cover for {}: {}",
                                    id,
                                    e
                                );
                            }
                        }
                    }
                    s.embedded += 1;
                }
                Err(e) => {
                    tracing::warn!("Backfill: failed to embed art into {:?}: {}", path, e);
                    s.errors += 1;
                }
            }

            if (i + 1) % 50 == 0 {
                tracing::info!(
                    "Artwork backfill: {}/{} processed ({} embedded)",
                    i + 1,
                    total,
                    s.embedded
                );
            }
        }

        self.report_backfill(
            conn,
            task_id,
            "artwork",
            total,
            total,
            s.embedded as i32,
            (s.no_art + s.no_file + s.missing_file) as i32,
            s.errors as i32,
        );

        tracing::info!(
            "Artwork backfill complete: {} embedded, {} no-art, {} missing-file, {} no-path, {} errors (of {})",
            s.embedded, s.no_art, s.missing_file, s.no_file, s.errors, total
        );
        Ok(s)
    }

    /// One-shot acoustic-fingerprint backfill: compute and store a Chromaprint
    /// fingerprint for every finalized library track that lacks one, so the acoustic
    /// dedup tier can recognize re-uploads of songs already in the library (which
    /// predate fingerprinting).
    pub async fn backfill_fingerprints(
        &self,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
    ) -> SoundgnomeResult<FingerprintBackfillSummary> {
        let tracks = self.track_service.get_all_finalized(conn)?;
        let total = tracks.len();
        let mut s = FingerprintBackfillSummary {
            total,
            ..Default::default()
        };
        tracing::info!("Fingerprint backfill: starting over {} tracks", total);
        self.report_backfill(conn, task_id, "fingerprint", 0, total, 0, 0, 0);

        for (i, track) in tracks.into_iter().enumerate() {
            self.report_backfill(
                conn,
                task_id,
                "fingerprint",
                i,
                total,
                s.fingerprinted as i32,
                (s.already_had + s.no_file) as i32,
                s.errors as i32,
            );
            let Some(id) = track.id else {
                continue;
            };
            let already = track.references.iter().any(|r| {
                r.external_url
                    .as_deref()
                    .is_some_and(|u| u.starts_with(CHROMAPRINT_PREFIX))
            });
            if already {
                s.already_had += 1;
                continue;
            }
            let Some(path) = track.file_path.clone() else {
                s.no_file += 1;
                continue;
            };
            if !path.exists() {
                s.no_file += 1;
                continue;
            }

            match compute_fingerprint(&path) {
                Ok(fp) => {
                    let reference = Reference {
                        id: None,
                        ref_type: ReferenceType::Metadata,
                        platform: Platform::Unknown,
                        external_id: None,
                        external_url: Some(format!(
                            "{CHROMAPRINT_PREFIX}{}",
                            encode_fingerprint(&fp)
                        )),
                    };
                    if let Err(e) = self.track_service.add_reference(conn, id, reference) {
                        tracing::warn!("Fingerprint backfill: could not store fp for {id}: {e}");
                        s.errors += 1;
                    } else {
                        s.fingerprinted += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Fingerprint backfill: failed for {path:?}: {e}");
                    s.errors += 1;
                }
            }

            if (i + 1) % 50 == 0 {
                tracing::info!(
                    "Fingerprint backfill: {}/{} processed ({} fingerprinted)",
                    i + 1,
                    total,
                    s.fingerprinted
                );
            }
        }

        self.report_backfill(
            conn,
            task_id,
            "fingerprint",
            total,
            total,
            s.fingerprinted as i32,
            (s.already_had + s.no_file) as i32,
            s.errors as i32,
        );

        tracing::info!(
            "Fingerprint backfill complete: {} fingerprinted, {} already had, {} no-file, {} errors (of {})",
            s.fingerprinted,
            s.already_had,
            s.no_file,
            s.errors,
            total
        );
        Ok(s)
    }

    /// Library-wide acoustic dedup. Groups tracks by normalized title+artist, then
    /// within each group clusters the copies that acoustically match (Chromaprint
    /// overlap) - so only genuine same-recording copies are merged, never different
    /// versions that merely share a title. For each cluster it keeps the best
    /// COMPLETE copy (finalized, full-length, then highest audio quality) and, when
    /// `apply` is set, deletes the rest (file + row), transferring a like/dislike to
    /// the keeper if it had none. `apply=false` only reports the plan.
    pub fn dedupe_library(
        &self,
        conn: &mut SqliteConnection,
        apply: bool,
        loose: bool,
    ) -> SoundgnomeResult<DedupeReport> {
        use std::collections::HashMap;

        // Stored fingerprints, keyed by track id (one query for the whole library).
        let mut fps: HashMap<i32, Vec<u32>> = HashMap::new();
        for (id, encoded) in self
            .track_service
            .fingerprint_candidates(conn, i32::MIN, i32::MAX)?
        {
            if let Some(fp) = encoded
                .strip_prefix(CHROMAPRINT_PREFIX)
                .and_then(decode_fingerprint)
            {
                fps.insert(id, fp);
            }
        }

        // Ratings, keyed by track id, so a like/dislike survives merging.
        let ratings: HashMap<i32, shared::models::Rating> =
            self.track_service.get_ratings(conn)?.into_iter().collect();

        // Group by normalized (title, sorted lowercased artist names).
        let mut groups: HashMap<(String, Vec<String>), Vec<Track>> = HashMap::new();
        for t in self.track_service.get_all(conn)? {
            let mut arts: Vec<String> =
                t.artists.iter().map(|a| a.name.trim().to_lowercase()).collect();
            arts.sort();
            groups
                .entry((t.title.trim().to_lowercase(), arts))
                .or_default()
                .push(t);
        }

        let mut report = DedupeReport {
            applied: apply,
            groups_examined: 0,
            clusters: Vec::new(),
            tracks_removed: 0,
            bytes_freed: 0,
        };

        for group in groups.into_values() {
            if group.len() < 2 {
                continue;
            }
            report.groups_examined += 1;

            // Cluster within the group by acoustic match (both need a fingerprint).
            let mut clusters: Vec<Vec<usize>> = Vec::new();
            for (i, t) in group.iter().enumerate() {
                let fp_i = t.id.and_then(|id| fps.get(&id));
                let mut placed = false;
                for cl in clusters.iter_mut() {
                    if cl.iter().any(|&j| {
                        same_recording(
                            fp_i,
                            group[j].id.and_then(|id| fps.get(&id)),
                            t.duration,
                            group[j].duration,
                            loose,
                        )
                    }) {
                        cl.push(i);
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    clusters.push(vec![i]);
                }
            }

            for cl in clusters {
                if cl.len() < 2 {
                    continue;
                }
                let max_dur = cl.iter().filter_map(|&i| group[i].duration).max().unwrap_or(0);
                // Rank: finalized > complete (not truncated) > quality > organized > newest.
                let scored: Vec<(usize, (bool, bool, Option<shared::models::AudioQuality>, bool, i32))> =
                    cl.iter()
                        .map(|&i| {
                            let t = &group[i];
                            let complete = match t.duration {
                                Some(d) => max_dur == 0 || d as f64 >= 0.9 * max_dur as f64,
                                None => true,
                            };
                            let organized = t
                                .file_path
                                .as_ref()
                                .map(|p| p.to_string_lossy().contains("library/"))
                                .unwrap_or(false);
                            (
                                i,
                                (
                                    !t.needs_validation,
                                    complete,
                                    t.audio_quality(),
                                    organized,
                                    t.id.unwrap_or(0),
                                ),
                            )
                        })
                        .collect();
                let keeper_i = scored.iter().max_by(|a, b| a.1.cmp(&b.1)).map(|s| s.0).unwrap();
                let keeper = &group[keeper_i];
                let keeper_id = keeper.id;
                let mut keeper_rating = keeper.id.and_then(|id| ratings.get(&id).cloned());

                let mut removed = Vec::new();
                for &li in cl.iter().filter(|&&i| i != keeper_i) {
                    let loser = &group[li];
                    let loser_rating = loser.id.and_then(|id| ratings.get(&id).cloned());
                    removed.push(dedupe_track_summary(loser, loser_rating.clone()));
                    if let Some(p) = loser.file_path.as_ref() {
                        report.bytes_freed += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                    }
                    report.tracks_removed += 1;

                    if apply {
                        // Preserve a like/dislike on the keeper if it had none.
                        if keeper_rating.is_none() {
                            if let (Some(kid), Some(r)) = (keeper_id, loser_rating) {
                                let _ = self.track_service.set_rating(conn, kid, Some(r.clone()));
                                keeper_rating = Some(r);
                            }
                        }
                        self.track_service
                            .delete_track_file_if_unreferenced(conn, loser);
                        if let Some(lid) = loser.id {
                            self.track_service.delete_by_id(conn, lid)?;
                        }
                    }
                }

                report.clusters.push(DedupeCluster {
                    keeper: dedupe_track_summary(keeper, keeper_rating),
                    removed,
                });
            }
        }

        tracing::info!(
            "Library dedup ({}): {} groups, {} clusters, {} tracks removed, {} bytes",
            if apply { "applied" } else { "dry-run" },
            report.groups_examined,
            report.clusters.len(),
            report.tracks_removed,
            report.bytes_freed
        );
        Ok(report)
    }

    pub fn new(
        track_service: Arc<TrackService>,
        album_service: Arc<AlbumService>,
        artist_service: Arc<ArtistService>,
        playlist_service: Arc<PlaylistService>,
        task_service: Arc<TaskService>,
    ) -> Self {
        Self {
            track_service,
            album_service,
            artist_service,
            playlist_service,
            task_service,
        }
    }

    /// Main entry point for downloading a track from a given URL (from any supported platform)
    pub async fn download_track_from_url(
        &self,
        url: &str,
        conn: &mut SqliteConnection,
    ) -> SoundgnomeResult<Track> {
        // Strip tracking/share query params (e.g. `si`, `utm_*`) so two submissions
        // of the same link that only differ by tracking noise dedupe correctly
        // against the `external_url` check right below.
        let curated_url = curate_source_url(url);
        let url = curated_url.as_str();

        tracing::info!("===========\nDownloading track from {:?}\n------", url);

        // Already owned tracks are refused, unless the source can now supply
        // better audio. The quality comparison later in the pipeline decides
        // whether the new file actually replaces the old one.
        if let Some(existing) = self.track_service.get_by_url(conn, url) {
            if !self.should_upgrade(&existing, url, None).await {
                return Err(Error::TrackExists(existing.display()));
            }
            tracing::info!("Refetching {} for a quality upgrade", existing.display());
        }

        let fetcher = Fetcher::new().await;

        // Fetch track info from URL
        let mut track = fetcher.get_track_from_url(url).await?;
        fetcher.clean_track_metadata(&mut track).await?;
        tracing::info!(
            "Fetched track info from {}: {}",
            track.get_source_platform().as_ref(),
            track.display()
        );

        // Orchestrator workflow
        let final_track = self.orchestrator_workflow(conn, track).await?;
        Ok(final_track)
    }

    /// Whether an already-owned track is worth downloading again.
    ///
    /// Only SoundCloud is considered: it is the one source that hands out
    /// lossless originals, and only to authenticated clients, so a track first
    /// grabbed without a session (or before the downloader accepted WAV) is
    /// often stored well below what the source can give.
    ///
    /// `original_available` is the source's own answer, which arrives free with
    /// the listing. When it is known, no request is made at all. Only an
    /// unknown answer falls back to asking yt-dlp, because doing that per track
    /// across a whole library trips SoundCloud's rate limiter (it starts
    /// answering 403 after roughly seventy requests).
    async fn should_upgrade(
        &self,
        existing: &Track,
        url: &str,
        original_available: Option<bool>,
    ) -> bool {
        // A missing or unreadable file always warrants a re-download regardless of
        // source platform or the `upgrade_existing` setting: restoring a lost file
        // is a repair, not an optional quality upgrade.
        let Some(stored) = existing.audio_quality() else {
            tracing::info!("Refetch: {} has no readable file", existing.display());
            return true;
        };

        let config = Config::get();
        if !config.downloader.upgrade_existing {
            return false;
        }

        if existing.get_source_platform() != shared::models::Platform::SoundCloud {
            return false;
        }

        // Already lossless: SoundCloud has nothing better than the original.
        if stored.lossless {
            return false;
        }

        match original_available {
            Some(false) => false,
            Some(true) => {
                tracing::info!(
                    "Upgrade: {} is {} kbps lossy and the source offers an original",
                    existing.display(),
                    stored.bitrate_bps / 1000
                );
                true
            }
            None => self.probe_for_upgrade(existing, url, stored).await,
        }
    }

    /// Ask yt-dlp what the source would serve. One request, so this is only for
    /// single-track downloads, never a whole sync.
    async fn probe_for_upgrade(
        &self,
        existing: &Track,
        url: &str,
        stored: shared::models::AudioQuality,
    ) -> bool {
        let available = match downloader::probe_available_quality(url).await {
            Ok(available) => available,
            // A failed probe is not evidence of a better source. Leave the file
            // alone rather than churn on a rate limit or a flaky network.
            Err(e) => {
                tracing::warn!("Upgrade probe failed for {}: {}", url, e);
                return false;
            }
        };

        if available.lossless {
            tracing::info!(
                "Upgrade: {} is {} kbps lossy, source offers a lossless original",
                existing.display(),
                stored.bitrate_bps / 1000
            );
            return true;
        }

        let stored_kbps = stored.bitrate_bps as f32 / 1000.0;
        let margin = Config::get().downloader.upgrade_bitrate_margin;
        match available.bitrate_kbps {
            Some(available_kbps) if available_kbps as f32 > stored_kbps * margin => {
                tracing::info!(
                    "Upgrade: {} is {:.0} kbps, source offers {} kbps",
                    existing.display(),
                    stored_kbps,
                    available_kbps
                );
                true
            }
            _ => false,
        }
    }

    /// Main entry point for downloading a playlist from a given URL (from any supported platform).
    /// `task_id` is optional; when provided, progress is persisted to the task table in real-time.
    pub async fn sync_playlist_from_url(
        &self,
        url: &str,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> SoundgnomeResult<Vec<Track>> {
        // Strip tracking/share query params so two syncs of "the same" playlist
        // link (e.g. with vs without `?si=...&utm_source=...`) curate to the same
        // `source_url` instead of `PlaylistService::upsert` creating a duplicate
        // playlist row.
        let curated_url = curate_source_url(url);
        let url = curated_url.as_str();

        tracing::info!(
            "====================\nDownloading playlist from {:?}\n---------",
            url
        );

        let fetcher = Fetcher::new().await;

        // Fetch playlist metadata and upsert in DB
        let playlist_meta = fetcher
            .get_playlist_from_url(url)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Could not fetch playlist metadata ({}), using URL as name",
                    e
                );
                // Extract platform from URL for better fallback naming. "music.youtube.com"
                // must be checked before the plain "youtube.com" branch, since it also
                // contains "youtube.com" as a substring.
                let (platform, name) = if url.contains("spotify.com") {
                    (shared::models::Platform::Spotify, url.to_string())
                } else if url.contains("soundcloud.com") {
                    (shared::models::Platform::SoundCloud, url.to_string())
                } else if url.contains("music.youtube.com") {
                    (shared::models::Platform::YoutubeMusic, url.to_string())
                } else if url.contains("youtube.com") || url.contains("youtu.be") {
                    (shared::models::Platform::Youtube, url.to_string())
                } else {
                    (shared::models::Platform::Unknown, url.to_string())
                };
                shared::models::Playlist {
                    id: None,
                    name,
                    source: platform,
                    source_url: Some(url.to_string()),
                    cover: None,
                }
            });
        let playlist = self.playlist_service.upsert(conn, &playlist_meta)?;
        let playlist_id = playlist.id.expect("persisted playlist must have an id");
        tracing::info!(
            "Playlist upserted in DB: \"{}\" (id={})",
            playlist.name,
            playlist_id
        );

        // Update task label to the actual playlist name with platform indicator.
        if let Some(tid) = task_id {
            let label = format!("[{}] {}", playlist.source, playlist.name);
            if let Err(e) = self.task_service.update_label(conn, tid, &label) {
                tracing::warn!("Failed to update task label to playlist name: {}", e);
            }
        }

        let playlist_tracks = fetcher.get_playlist_tracks_from_url(url).await?;
        let total_tracks = playlist_tracks.len();
        tracing::info!("Found {} tracks in playlist", total_tracks);

        // Filter out existing tracks (link them to the playlist anyway) and collect new ones
        let mut new_tracks: Vec<(Option<i32>, Track)> = Vec::new();
        let mut stats = shared::models::TaskStats::default();
        for pt in &playlist_tracks {
            let track = &pt.track;
            let track_url = track
                .get_source()
                .and_then(|s| s.external_url.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let position = pt.position.map(|p| p as i32);
            if let Some(existing) = self.track_service.get_by_url(conn, &track_url) {
                let track_id = existing.id.expect("persisted track must have an id");

                // Already owned, but the source may now offer better audio. The
                // download path below re-runs the quality comparison and only
                // replaces the file when the new one really is better.
                if self
                    .should_upgrade(&existing, &track_url, pt.original_available)
                    .await
                {
                    if let Err(e) =
                        self.playlist_service
                            .add_track(conn, playlist_id, track_id, position)
                    {
                        tracing::error!("Failed to link track {} to playlist: {}", track_id, e);
                    }
                    new_tracks.push((position, track.clone()));
                    continue;
                }

                tracing::warn!(
                    "   -> Track already exists in DB, linking to playlist: {}",
                    track.display()
                );
                if let Err(e) =
                    self.playlist_service
                        .add_track(conn, playlist_id, track_id, position)
                {
                    tracing::error!(
                        "Failed to link existing track {} to playlist: {}",
                        track_id,
                        e
                    );
                }
                stats.skipped += 1;
                if let Some(tid) = task_id {
                    let current = stats.skipped;
                    if let Err(e) =
                        self.task_service
                            .update_progress(conn, tid, current, total_tracks as i32)
                    {
                        tracing::warn!("Failed to update task progress: {}", e);
                    }
                    if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                        tracing::warn!("Failed to update task stats: {}", e);
                    }
                }
            } else {
                new_tracks.push((position, track.clone()));
            }
        }

        tracing::info!(
            "{} new tracks to download after filtering existing ones",
            new_tracks.len()
        );

        // Clean metadata for all new tracks
        let mut new_track_values: Vec<Track> = new_tracks.iter().map(|(_, t)| t.clone()).collect();
        self.clean_tracks_metadata_with_progress(
            &fetcher,
            conn,
            &mut new_track_values,
            task_id,
            &mut stats,
        )
        .await;

        // Process each new track and link it to the playlist
        let mut new_processed_tracks = Vec::new();
        for (i, (position, _)) in new_tracks.iter().enumerate() {
            // Check for cancellation before processing next track
            if cancel_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
            {
                tracing::info!(
                    "Playlist sync cancelled after processing {}/{} new tracks",
                    i,
                    new_tracks.len()
                );
                return Err(Error::Cancelled);
            }

            let track = &new_track_values[i];
            tracing::info!("Processing track: {}", track.display());
            match self.orchestrator_workflow(conn, track.clone()).await {
                Ok(t) => {
                    tracing::info!("Successfully processed track: {}", t.display());
                    if t.needs_validation {
                        stats.to_validate += 1;
                        stats.to_validate_tracks.push(TaskTrackValidation {
                            track: t.display(),
                            track_id: t.id,
                            reason: t.validation_reason.clone(),
                        });
                    } else {
                        stats.downloaded += 1;
                    }
                    if let Some(track_id) = t.id {
                        if let Err(e) =
                            self.playlist_service
                                .add_track(conn, playlist_id, track_id, *position)
                        {
                            tracing::error!(
                                "Failed to link new track {} to playlist: {}",
                                track_id,
                                e
                            );
                        }
                    }
                    new_processed_tracks.push(t);
                }
                Err(e) => {
                    stats.errors.push(shared::models::TaskTrackError {
                        track: track.display(),
                        reason: e.to_string(),
                        track_id: None,
                        provider_url: track.get_provider().and_then(|p| p.external_url.clone()),
                    });
                    tracing::error!("Error downloading track {}: {:?}", track.display(), e);
                }
            }
            if let Some(tid) = task_id {
                let current = stats.skipped + (i as i32) + 1;
                if let Err(e) =
                    self.task_service
                        .update_progress(conn, tid, current, total_tracks as i32)
                {
                    tracing::warn!("Failed to update task progress: {}", e);
                }
                if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                    tracing::warn!("Failed to update task stats: {}", e);
                }
            }
        }

        tracing::info!(
            "Playlist \"{}\": {} downloaded, {} to validate, {} skipped, {} errors (total {})",
            playlist.name,
            stats.downloaded,
            stats.to_validate,
            stats.skipped,
            stats.errors.len(),
            total_tracks,
        );

        // Best-effort: export updated playlist as an M3U8 file.
        self.export_playlist_m3u8(conn, &playlist, playlist_id);

        Ok(new_processed_tracks)
    }

    /// Main entry point for downloading/syncing all tracks from an artist URL.
    /// `task_id` is optional; when provided, progress is persisted to the task table in real-time.
    pub async fn sync_artist_from_url(
        &self,
        url: &str,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> SoundgnomeResult<Vec<Track>> {
        // Strip tracking/share query params for consistency with the other
        // `*_from_url` entry points (see `sync_playlist_from_url`).
        let curated_url = curate_source_url(url);
        let url = curated_url.as_str();

        tracing::info!(
            "====================\nSyncing artist from {:?}\n---------",
            url
        );

        let fetcher = Fetcher::new().await;

        // Fetch artist metadata and upsert in DB
        let artist_meta = fetcher.get_artist_from_url(url).await?;
        let artist = self.artist_service.create_or_ignore(conn, &artist_meta)?;
        let artist_id = artist.id.expect("persisted artist must have an id");
        tracing::info!(
            "Artist upserted in DB: \"{}\" (id={})",
            artist.name,
            artist_id
        );

        // Update task label to the artist name with platform indicator.
        if let Some(tid) = task_id {
            let platform = artist
                .get_source()
                .map(|r| r.platform.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let label = format!("[{}] {}", platform, artist.name);
            if let Err(e) = self.task_service.update_label(conn, tid, &label) {
                tracing::warn!("Failed to update task label to artist name: {}", e);
            }
        }

        // Fetch all tracks from this artist
        let artist_tracks = fetcher.get_artist_tracks_from_url(url).await?;
        let total_tracks = artist_tracks.len();
        tracing::info!("Found {} tracks for artist", total_tracks);

        // Filter out existing tracks and collect new ones
        let mut new_tracks: Vec<Track> = Vec::new();
        let mut stats = shared::models::TaskStats::default();
        for track in &artist_tracks {
            let track_url = track
                .get_source()
                .and_then(|s| s.external_url.clone())
                .unwrap_or_else(|| "unknown".to_string());
            if self.track_service.get_by_url(conn, &track_url).is_some() {
                tracing::warn!("   -> Track already exists in DB: {}", track.display());
                stats.skipped += 1;
                if let Some(tid) = task_id {
                    let current = stats.skipped;
                    if let Err(e) =
                        self.task_service
                            .update_progress(conn, tid, current, total_tracks as i32)
                    {
                        tracing::warn!("Failed to update task progress: {}", e);
                    }
                    if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                        tracing::warn!("Failed to update task stats: {}", e);
                    }
                }
            } else {
                new_tracks.push(track.clone());
            }
        }

        tracing::info!(
            "{} new tracks to download after filtering existing ones",
            new_tracks.len()
        );

        // Clean metadata for all new tracks
        self.clean_tracks_metadata_with_progress(
            &fetcher,
            conn,
            &mut new_tracks,
            task_id,
            &mut stats,
        )
        .await;

        // Process each new track
        let mut new_processed_tracks = Vec::new();
        for (i, track) in new_tracks.iter().enumerate() {
            // Check for cancellation before processing next track
            if cancel_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
            {
                tracing::info!(
                    "Artist sync cancelled after processing {}/{} new tracks",
                    i,
                    new_tracks.len()
                );
                return Err(Error::Cancelled);
            }

            tracing::info!("Processing track: {}", track.display());
            match self.orchestrator_workflow(conn, track.clone()).await {
                Ok(t) => {
                    tracing::info!("Successfully processed track: {}", t.display());
                    if t.needs_validation {
                        stats.to_validate += 1;
                        stats.to_validate_tracks.push(TaskTrackValidation {
                            track: t.display(),
                            track_id: t.id,
                            reason: t.validation_reason.clone(),
                        });
                    } else {
                        stats.downloaded += 1;
                    }
                    new_processed_tracks.push(t);
                }
                Err(e) => {
                    stats.errors.push(shared::models::TaskTrackError {
                        track: track.display(),
                        reason: e.to_string(),
                        track_id: None,
                        provider_url: track.get_provider().and_then(|p| p.external_url.clone()),
                    });
                    tracing::error!("Error downloading track {}: {:?}", track.display(), e);
                }
            }
            if let Some(tid) = task_id {
                let current = stats.skipped + (i as i32) + 1;
                if let Err(e) =
                    self.task_service
                        .update_progress(conn, tid, current, total_tracks as i32)
                {
                    tracing::warn!("Failed to update task progress: {}", e);
                }
                if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                    tracing::warn!("Failed to update task stats: {}", e);
                }
            }
        }

        tracing::info!(
            "Artist \"{}\": {} downloaded, {} to validate, {} skipped, {} errors (total {})",
            artist.name,
            stats.downloaded,
            stats.to_validate,
            stats.skipped,
            stats.errors.len(),
            total_tracks,
        );

        Ok(new_processed_tracks)
    }

    /// Main entry point for downloading/syncing all tracks from an album URL.
    /// `task_id` is optional; when provided, progress is persisted to the task table in real-time.
    pub async fn sync_album_from_url(
        &self,
        url: &str,
        conn: &mut SqliteConnection,
        task_id: Option<i32>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> SoundgnomeResult<Vec<Track>> {
        // Strip tracking/share query params for consistency with the other
        // `*_from_url` entry points (see `sync_playlist_from_url`).
        let curated_url = curate_source_url(url);
        let url = curated_url.as_str();

        tracing::info!(
            "====================\nSyncing album from {:?}\n---------",
            url
        );

        let fetcher = Fetcher::new().await;

        // Fetch album metadata
        let album_meta = fetcher.get_album_from_url(url).await?;
        tracing::info!(
            "Album: \"{}\" by {}",
            album_meta.title,
            album_meta
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Update task label to the album title with platform indicator.
        if let Some(tid) = task_id {
            let platform = album_meta
                .get_source()
                .map(|r| r.platform.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let label = format!("[{}] {}", platform, album_meta.title);
            if let Err(e) = self.task_service.update_label(conn, tid, &label) {
                tracing::warn!("Failed to update task label to album title: {}", e);
            }
        }

        // Fetch all tracks from this album
        let album_tracks = fetcher.get_album_tracks_from_url(url).await?;
        let total_tracks = album_tracks.len();
        tracing::info!("Found {} tracks in album", total_tracks);

        // Filter out existing tracks and collect new ones
        let mut new_tracks: Vec<Track> = Vec::new();
        let mut stats = shared::models::TaskStats::default();
        for track in &album_tracks {
            let track_url = track
                .get_source()
                .and_then(|s| s.external_url.clone())
                .unwrap_or_else(|| "unknown".to_string());
            if self.track_service.get_by_url(conn, &track_url).is_some() {
                tracing::warn!("   -> Track already exists in DB: {}", track.display());
                stats.skipped += 1;
                if let Some(tid) = task_id {
                    let current = stats.skipped;
                    if let Err(e) =
                        self.task_service
                            .update_progress(conn, tid, current, total_tracks as i32)
                    {
                        tracing::warn!("Failed to update task progress: {}", e);
                    }
                    if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                        tracing::warn!("Failed to update task stats: {}", e);
                    }
                }
            } else {
                new_tracks.push(track.clone());
            }
        }

        tracing::info!(
            "{} new tracks to download after filtering existing ones",
            new_tracks.len()
        );

        // Clean metadata for all new tracks
        self.clean_tracks_metadata_with_progress(
            &fetcher,
            conn,
            &mut new_tracks,
            task_id,
            &mut stats,
        )
        .await;

        // Process each new track
        let mut new_processed_tracks = Vec::new();
        for (i, track) in new_tracks.iter().enumerate() {
            // Check for cancellation before processing next track
            if cancel_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
            {
                tracing::info!(
                    "Album sync cancelled after processing {}/{} new tracks",
                    i,
                    new_tracks.len()
                );
                return Err(Error::Cancelled);
            }

            tracing::info!("Processing track: {}", track.display());
            match self.orchestrator_workflow(conn, track.clone()).await {
                Ok(t) => {
                    tracing::info!("Successfully processed track: {}", t.display());
                    if t.needs_validation {
                        stats.to_validate += 1;
                        stats.to_validate_tracks.push(TaskTrackValidation {
                            track: t.display(),
                            track_id: t.id,
                            reason: t.validation_reason.clone(),
                        });
                    } else {
                        stats.downloaded += 1;
                    }
                    new_processed_tracks.push(t);
                }
                Err(e) => {
                    stats.errors.push(shared::models::TaskTrackError {
                        track: track.display(),
                        reason: e.to_string(),
                        track_id: None,
                        provider_url: track.get_provider().and_then(|p| p.external_url.clone()),
                    });
                    tracing::error!("Error downloading track {}: {:?}", track.display(), e);
                }
            }
            if let Some(tid) = task_id {
                let current = stats.skipped + (i as i32) + 1;
                if let Err(e) =
                    self.task_service
                        .update_progress(conn, tid, current, total_tracks as i32)
                {
                    tracing::warn!("Failed to update task progress: {}", e);
                }
                if let Err(e) = self.task_service.update_stats(conn, tid, &stats) {
                    tracing::warn!("Failed to update task stats: {}", e);
                }
            }
        }

        tracing::info!(
            "Album \"{}\": {} downloaded, {} to validate, {} skipped, {} errors (total {})",
            album_meta.title,
            stats.downloaded,
            stats.to_validate,
            stats.skipped,
            stats.errors.len(),
            total_tracks,
        );

        Ok(new_processed_tracks)
    }

    /// Ingest all audio files found in `ingest_dir`, one by one.
    ///
    /// Progress and per-track stats are persisted live to `task_id` so the UI
    /// can poll `GET /api/tasks/:id` for real-time feedback.
    pub async fn ingest_local_dir(
        &self,
        conn: &mut SqliteConnection,
        ingest_dir: &Path,
        task_id: i32,
    ) -> SoundgnomeResult<usize> {
        let audio_extensions = ["mp3", "flac", "m4a", "mp4", "aac", "ogg", "opus", "wav"];

        // When ingesting the shared server dir, skip the `_uploads` staging subtree:
        // those belong to browser uploads and are ingested via their own session
        // dir. When `ingest_dir` is itself a session dir (already under `_uploads`),
        // this exclusion is inert and its files are ingested normally.
        let root_is_upload = ingest_dir.components().any(|c| c.as_os_str() == "_uploads");

        // Collect all audio files first so we know the total upfront.
        let files: Vec<PathBuf> = walkdir::WalkDir::new(ingest_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                root_is_upload || !e.path().components().any(|c| c.as_os_str() == "_uploads")
            })
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| audio_extensions.contains(&x.to_lowercase().as_str()))
                    .unwrap_or(false)
            })
            .filter(|e| {
                // Skip partial-download artifacts (e.g. "Song.temp.m4a"): incomplete
                // files with no decodable audio.
                !e.file_name().to_string_lossy().to_lowercase().contains(".temp.")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        let total = files.len() as i32;
        tracing::info!("Ingest dir {:?}: found {} audio files", ingest_dir, total);

        let concurrency = ingest_concurrency();
        tracing::info!("Ingest: preparing files with up to {} parallel workers", concurrency);

        let mut stats = shared::models::TaskStats::default();

        // Prepare (tag-read + hash + fingerprint) runs in parallel on blocking
        // threads; the DB commit stays serial on the single connection. A sliding
        // window keeps `concurrency` files decoding ahead of the commit so the
        // expensive ffmpeg decode is overlapped instead of paid one-at-a-time.
        let mut inflight: std::collections::VecDeque<(
            usize,
            PathBuf,
            tokio::task::JoinHandle<SoundgnomeResult<PreparedIngest>>,
        )> = std::collections::VecDeque::new();
        let mut next_idx = 0usize;
        while next_idx < files.len() && inflight.len() < concurrency {
            let p = files[next_idx].clone();
            inflight.push_back((
                next_idx,
                p.clone(),
                tokio::task::spawn_blocking(move || prepare_ingest_file(&p)),
            ));
            next_idx += 1;
        }

        while let Some((i, file_path, handle)) = inflight.pop_front() {
            // Keep the window full while we commit this file (DB work is serial).
            if next_idx < files.len() {
                let p = files[next_idx].clone();
                inflight.push_back((
                    next_idx,
                    p.clone(),
                    tokio::task::spawn_blocking(move || prepare_ingest_file(&p)),
                ));
                next_idx += 1;
            }

            tracing::info!("Ingesting [{}/{}]: {:?}", i + 1, total, file_path);

            let commit_result = match handle.await {
                Ok(Ok(prepared)) => self.commit_ingest(conn, prepared).await,
                Ok(Err(e)) => Err(e),
                Err(join_err) => {
                    Err(Error::Custom(format!("ingest prepare task failed: {join_err}")))
                }
            };

            match commit_result {
                Ok((t, outcome)) => match outcome {
                    IngestOutcome::NeedsValidation => {
                        stats.to_validate += 1;
                        stats
                            .to_validate_tracks
                            .push(shared::models::TaskTrackValidation {
                                track: t.display(),
                                track_id: t.id,
                                reason: t.validation_reason.clone(),
                            });
                    }
                    IngestOutcome::New => {
                        stats.downloaded += 1;
                    }
                    IngestOutcome::Replaced | IngestOutcome::Duplicate => {
                        stats.skipped += 1;
                        stats
                            .skipped_tracks
                            .push(shared::models::TaskTrackValidation {
                                track: t.display(),
                                track_id: t.id,
                                reason: Some(match outcome {
                                    IngestOutcome::Replaced => "Upgraded existing copy".to_string(),
                                    _ => "Already in library".to_string(),
                                }),
                            });
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to ingest {:?}: {}", file_path, e);
                    stats.errors.push(shared::models::TaskTrackError {
                        track: file_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| file_path.display().to_string()),
                        reason: e.to_string(),
                        track_id: None,
                        provider_url: None,
                    });
                }
            }

            if let Err(e) = self
                .task_service
                .update_progress(conn, task_id, (i + 1) as i32, total)
            {
                tracing::warn!("Failed to update ingest task progress: {}", e);
            }
            if let Err(e) = self.task_service.update_stats(conn, task_id, &stats) {
                tracing::warn!("Failed to update ingest task stats: {}", e);
            }
        }

        tracing::info!(
            "Ingest dir complete: {} ingested, {} to validate, {} errors",
            stats.downloaded,
            stats.to_validate,
            stats.errors.len()
        );

        Ok(stats.errors.len())
    }

    // ============================================================================================
    // == Local file ingest
    // ============================================================================================

    /// Ingest a single local audio file into the library. Thin wrapper: prepare
    /// (pure, off-DB) then commit (DB + network). Batch ingest parallelizes the
    /// prepare step; see `ingest_local_dir`.
    pub async fn ingest_local_file(
        &self,
        conn: &mut SqliteConnection,
        file_path: &Path,
    ) -> SoundgnomeResult<(Track, IngestOutcome)> {
        let prepared = prepare_ingest_file(file_path)?;
        self.commit_ingest(conn, prepared).await
    }

    /// Commit a prepared file. All DB/network work lives here so `prepare` stays
    /// pure and parallelizable.
    ///
    /// Workflow (mirrors `docs/workflows/download.md` — "Import a local file"):
    /// 1. Exact-duplicate short-circuit (content hash).
    /// 2. Acoustic-fingerprint short-circuit.
    /// 3. Enrich via MusicBrainz — skipped when the file's tags are already complete.
    /// 4. Deduplicate by title/artist; stage for validation when enrichment is weak.
    /// 5. Tag, organise, and persist the winner.
    async fn commit_ingest(
        &self,
        conn: &mut SqliteConnection,
        prepared: PreparedIngest,
    ) -> SoundgnomeResult<(Track, IngestOutcome)> {
        let PreparedIngest {
            mut track,
            file_path,
            content_hash,
            fingerprint,
            has_cover,
        } = prepared;

        tracing::info!("===========\nIngesting local file: {:?}\n------", file_path);
        tracing::info!("Read tags from file: {}", track.display());

        // Step 1: Exact-duplicate short-circuit via the raw-bytes hash.
        let content_key = format!("soundome:sha256:{content_hash}");
        if let Some(existing) = self.track_service.get_by_url(conn, &content_key) {
            tracing::info!(
                "Ingest: exact duplicate (content hash) of {}, skipping",
                existing.display()
            );
            return Ok((existing, IngestOutcome::Duplicate));
        }
        track.references.push(Reference {
            id: None,
            ref_type: ReferenceType::Metadata,
            platform: Platform::Unknown,
            external_id: None,
            external_url: Some(content_key),
        });

        // Step 2: Acoustic-fingerprint short-circuit (catches re-encodes/rebitrates).
        if let Some(fp) = &fingerprint {
            if let Some(existing) = self.dedupe_by_fingerprint(conn, &track, fp).await {
                tracing::info!("Ingest: acoustic match with {}", existing.display());
                return self
                    .resolve_existing_match(conn, existing, &track, &file_path)
                    .await;
            }
            track.references.push(Reference {
                id: None,
                ref_type: ReferenceType::Metadata,
                platform: Platform::Unknown,
                external_id: None,
                external_url: Some(format!("{CHROMAPRINT_PREFIX}{}", encode_fingerprint(fp))),
            });
        }

        // Step 3: Enrich metadata — but trust already-complete tags and skip the
        // (rate-limited) network lookup for them.
        let (should_validate, existing_track_opt) = if tags_complete(&track) {
            tracing::info!(
                "Ingest: complete tags, skipping metadata enrichment for {}",
                track.display()
            );
            (false, None)
        } else {
            self.enrich_metada(conn, &mut track, true).await?
        };

        if should_validate {
            tracing::warn!(
                "Ingest: saving for manual validation — reason={:?}",
                track.validation_reason
            );
            let staged_path = self.stage_local_file(&file_path)?;
            track.file_path = Some(staged_path);
            let mut saved = self.save_track(conn, &track).await?;
            self.set_embedded_cover_if_missing(conn, &mut saved, has_cover);
            return Ok((saved, IngestOutcome::NeedsValidation));
        }

        // Step 4: Deduplication by title/artist.
        let existing_track = if existing_track_opt.is_some() {
            existing_track_opt
        } else {
            self.dedupe_track(conn, &track).await
        };

        match existing_track {
            Some(existing_track) => {
                self.resolve_existing_match(conn, existing_track, &track, &file_path)
                    .await
            }
            None => {
                tracing::info!("Ingest: no existing track, finalising");
                self.process_track_file(&mut track, &file_path).await?;
                let mut inserted = self.save_track(conn, &track).await?;
                self.set_embedded_cover_if_missing(conn, &mut inserted, has_cover);
                Ok((inserted, IngestOutcome::New))
            }
        }
    }

    /// When a freshly-ingested file carries embedded cover art but has no `cover`
    /// URL (local files rarely reference external artwork), point `cover` at the
    /// on-demand endpoint that serves the file's embedded picture. The art already
    /// lives in the file, so nothing is copied. Best-effort: a failed DB update
    /// leaves the track without artwork rather than failing the ingest.
    fn set_embedded_cover_if_missing(
        &self,
        conn: &mut SqliteConnection,
        track: &mut Track,
        has_embedded_cover: bool,
    ) {
        if !has_embedded_cover || track.cover.is_some() {
            return;
        }
        let Some(id) = track.id else { return };
        track.cover = Some(embedded_cover_url(id));
        if let Err(e) = self.track_service.update(conn, id, track) {
            tracing::warn!("Ingest: could not persist embedded cover for {}: {}", id, e);
            track.cover = None;
        }
    }

    /// Copy a local file into the staging directory so it can be processed without
    /// modifying the original location. Returns the path of the staged copy.
    ///
    /// The staged filename is prefixed with a UUID to guarantee uniqueness even when
    /// multiple files share the same original name (e.g. two different `track.mp3`
    /// from different ingest sessions).
    fn stage_local_file(&self, source: &Path) -> SoundgnomeResult<PathBuf> {
        let staging_dir = PathBuf::from(&Config::get().general.temp_download_dir);
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| Error::Custom(format!("Could not create staging dir: {e}")))?;

        let file_name = source
            .file_name()
            .ok_or_else(|| Error::Custom("Source path has no file name".to_string()))?
            .to_string_lossy();

        // Prefix with a UUID so two files named identically never collide in staging.
        let unique_name = format!("{}-{}", Uuid::new_v4(), file_name);
        let dest = staging_dir.join(&unique_name);

        std::fs::copy(source, &dest)
            .map_err(|e| Error::Custom(format!("Failed to stage local file: {e}")))?;
        tracing::debug!("Staged local file {:?} → {:?}", source, dest);
        Ok(dest)
    }

    // ============================================================================================
    // == Sub private and utils methods
    // ============================================================================================

    /// Re-query metadata providers for a pending track and return scored candidates.
    /// Used by the validation UI to show potential matches.
    pub async fn get_match_candidates(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
    ) -> SoundgnomeResult<Vec<tagger::enricher::MatchCandidate>> {
        let track = self.track_service.get_by_id(conn, id)?;
        let candidates = tagger::enricher::get_candidates_for_track(&track).await;
        Ok(candidates)
    }

    /// Called after a user approves a pending validation through the web UI.
    ///
    /// The track already has an audio file in the staging folder (downloaded at fetch time).
    /// This method applies the optional metadata `patch`, tags the staged file, moves it
    /// to the library, and clears the validation flag.
    pub async fn finalize_validated_track(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        patch: ValidationPatch,
    ) -> SoundgnomeResult<Track> {
        // 1. Load current track from DB
        let mut track = self.track_service.get_by_id(conn, id)?;

        // 2. Apply metadata patch
        if let Some(title) = patch.title {
            track.title = title;
        }
        if let Some(genre) = patch.genre {
            track.genre = Some(genre);
        }
        if let Some(date) = patch.date {
            track.date = Some(date);
        }
        if let Some(tn) = patch.track_number {
            track.track_number = Some(tn);
        }
        if let Some(dn) = patch.disc_number {
            track.disc_number = Some(dn);
        }
        if let Some(label) = patch.label {
            track.label = Some(label);
        }

        if let Some(names) = patch.artists {
            let mut artists: Vec<Artist> = Vec::with_capacity(names.len());
            for name in names {
                let artist = Artist {
                    id: None,
                    name,
                    icon: None,
                    references: vec![],
                };
                let saved = self.artist_service.create_or_ignore(conn, &artist)?;
                artists.push(saved);
            }
            track.artists = artists;
        }

        if let Some(album_title) = patch.album_title {
            match track.album.as_mut() {
                Some(album) => album.title = album_title,
                None => {
                    track.album = Some(Album {
                        id: None,
                        title: album_title,
                        artists: vec![],
                        album_type: AlbumType::Album,
                        cover: None,
                        date: None,
                        references: vec![],
                    });
                }
            }
        }

        // 3. Resolve the audio file path: use the staged file if present, otherwise
        //    download from the provider URL supplied by the user (DRM fallback).
        let file_path = if let Some(staged) = track.file_path.clone() {
            // Organized tracks store a library-relative path (e.g. `./library/...`),
            // staged tracks an absolute temp path. Resolve the relative form against
            // the library dir, then confirm the audio is actually on disk so a
            // missing file surfaces a clear message instead of a raw tag-read error.
            let resolved = if staged.exists() || staged.is_absolute() {
                staged
            } else {
                let base = PathBuf::from(&Config::get().general.base_library_dir);
                base.join(staged.strip_prefix(&base).unwrap_or(staged.as_path()))
            };
            if !resolved.exists() {
                return Err(Error::Custom(format!(
                    "The audio file for this track is missing on disk ({}). It may have been moved or deleted. Reject the track, or re-ingest the file.",
                    resolved.display()
                )));
            }
            resolved
        } else {
            let provider_url = patch.provider_url.as_ref().ok_or_else(|| {
                Error::Custom(format!(
                    "track {} has no staged file and no provider_url was provided",
                    id
                ))
            })?;

            tracing::info!(
                "No staged file for track {} — downloading from provider: {}",
                id,
                provider_url
            );

            let provider_platform = if provider_url.contains("music.youtube.com") {
                Platform::YoutubeMusic
            } else {
                Platform::Youtube
            };

            let provider_ref = Reference {
                id: None,
                ref_type: ReferenceType::Provider,
                platform: provider_platform,
                external_id: None,
                external_url: Some(provider_url.clone()),
            };
            track.references.push(provider_ref.clone());

            let source_ref = track
                .get_source()
                .ok_or_else(|| Error::Custom(format!("track {} has no source reference", id)))?;

            let staging_dir = PathBuf::from(&Config::get().general.temp_download_dir);
            downloader::download(
                &source_ref,
                &provider_ref,
                &staging_name(&track.title),
                staging_dir,
            )
            .await?
        };
        track.file_path = Some(file_path.clone());
        self.process_track_file(&mut track, &file_path).await?;

        // 4. Clear validation flag and persist
        track.needs_validation = false;
        track.validation_reason = None;

        self.save_track(conn, &track).await
    }

    // ============================================================================================
    // == Sub private and utils methods (internal)
    // ============================================================================================

    /// Search YouTube and YouTube Music for provider candidates matching a pending track.
    /// Returns all results unfiltered so the user can select the correct video manually.
    /// Intended for DRM-protected SoundCloud tracks that could not be auto-downloaded.
    pub async fn get_youtube_provider_candidates(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
    ) -> SoundgnomeResult<Vec<tagger::enricher::MatchCandidate>> {
        let track = self.track_service.get_by_id(conn, id)?;
        let results = downloader::search_youtube_candidates(&track).await?;

        let candidates = results
            .into_iter()
            .map(|t| {
                let provider = t
                    .get_provider()
                    .and_then(|r| r.external_url.clone())
                    .map(|u| {
                        if u.contains("music.youtube.com") {
                            "youtube_music"
                        } else {
                            "youtube"
                        }
                    })
                    .unwrap_or("youtube")
                    .to_string();
                tagger::enricher::MatchCandidate {
                    track: t,
                    score: 1.0,
                    provider,
                }
            })
            .collect();

        Ok(candidates)
    }

    // ============================================================================================
    // == Thumbnail-from-references (manual edit UI)
    // ============================================================================================

    /// Best-effort: resolve an artist's photo by re-querying whichever of its existing
    /// references point to a provider that exposes artist images (Spotify, SoundCloud,
    /// YouTube Music), then persist the first image found as the artist's `icon`.
    ///
    /// Used by the manual edit UI's "Fetch from references" action. Returns `Ok(None)`
    /// (not an error) when no reference resolves to an image, so the caller can tell
    /// "nothing found" apart from a network or database failure.
    pub async fn fetch_artist_icon_from_references(
        &self,
        conn: &mut SqliteConnection,
        artist_id: i32,
    ) -> SoundgnomeResult<Option<Artist>> {
        let mut artist = self.artist_service.get_by_id(conn, artist_id)?;
        let fetcher = Fetcher::new().await;

        let mut found_icon = None;
        for reference in &artist.references {
            let Some(url) = reference.external_url.as_deref() else {
                continue;
            };
            if !Fetcher::is_valid_artist_url(url) {
                continue;
            }
            match fetcher.get_artist_from_url(url).await {
                Ok(fetched) if fetched.icon.is_some() => {
                    found_icon = fetched.icon;
                    break;
                }
                Ok(_) => {}
                Err(e) => tracing::debug!(
                    "fetch_artist_icon_from_references: reference {} did not resolve: {}",
                    url,
                    e
                ),
            }
        }

        let Some(icon) = found_icon else {
            return Ok(None);
        };

        artist.icon = Some(icon);
        let saved = self.artist_service.update(conn, artist_id, &artist)?;
        Ok(Some(saved))
    }

    /// Same idea as `fetch_artist_icon_from_references`, but resolves and persists an
    /// album's `cover` instead.
    pub async fn fetch_album_cover_from_references(
        &self,
        conn: &mut SqliteConnection,
        album_id: i32,
    ) -> SoundgnomeResult<Option<Album>> {
        let mut album = self.album_service.get_by_id(conn, album_id)?;
        let fetcher = Fetcher::new().await;

        let mut found_cover = None;
        for reference in &album.references {
            let Some(url) = reference.external_url.as_deref() else {
                continue;
            };
            if !Fetcher::is_valid_album_url(url) {
                continue;
            }
            match fetcher.get_album_from_url(url).await {
                Ok(fetched) if fetched.cover.is_some() => {
                    found_cover = fetched.cover;
                    break;
                }
                Ok(_) => {}
                Err(e) => tracing::debug!(
                    "fetch_album_cover_from_references: reference {} did not resolve: {}",
                    url,
                    e
                ),
            }
        }

        let Some(cover) = found_cover else {
            return Ok(None);
        };

        album.cover = Some(cover);
        let saved = self.album_service.update(conn, album_id, &album)?;
        Ok(Some(saved))
    }

    /// Clean metadata for `tracks` via the fetcher, reporting live per-batch progress
    /// into `stats.ai_curation` (persisted through `task_id`) so the frontend can show
    /// an "AI curation in progress" indicator while SoundCloud batches are processed.
    async fn clean_tracks_metadata_with_progress(
        &self,
        fetcher: &Fetcher,
        conn: &mut SqliteConnection,
        tracks: &mut [Track],
        task_id: Option<i32>,
        stats: &mut shared::models::TaskStats,
    ) {
        if let Some(tid) = task_id {
            stats.ai_curation = Some(shared::models::AiCurationProgress {
                processed: 0,
                total: tracks.len() as i32,
            });
            if let Err(e) = self.task_service.update_stats(conn, tid, stats) {
                tracing::warn!("Failed to update task stats: {}", e);
            }
        }

        let mut on_batch = |processed: usize, total: usize| {
            if let Some(tid) = task_id {
                stats.ai_curation = Some(shared::models::AiCurationProgress {
                    processed: processed as i32,
                    total: total as i32,
                });
                if let Err(e) = self.task_service.update_stats(conn, tid, stats) {
                    tracing::warn!("Failed to update AI curation progress: {}", e);
                }
            }
        };

        if let Err(e) = fetcher
            .clean_tracks_metadata(
                &mut tracks.iter_mut().collect::<Vec<_>>(),
                Some(&mut on_batch),
            )
            .await
        {
            tracing::warn!("Failed to clean tracks title and artist name: {}", e);
        }

        if task_id.is_some() {
            stats.ai_curation = None;
        }
    }

    async fn orchestrator_workflow(
        &self,
        conn: &mut SqliteConnection,
        track: Track,
    ) -> SoundgnomeResult<Track> {
        let mut track = track;

        // Step 1: Enrich metadata
        tracing::info!("Getting metadata via tagger providers");
        let (should_validate, mut existing_track) =
            self.enrich_metada(conn, &mut track, false).await?;

        // Step 2: Try to download to staging.
        // SoundCloud DRM-protected tracks will return SoundCloudDrmProtected instead of a hard error.
        tracing::info!("Searching and downloading track from provider (staging)");
        let file_path_opt = match self.download_track(&mut track).await {
            Ok(path) => Some(path),
            Err(Error::SoundCloudDrmProtected(_)) => {
                tracing::warn!("SoundCloud track is DRM protected");

                // Before falling back to manual validation, check whether a Spotify metadata
                // reference is already attached (e.g. from enrichment) and, if so, retry
                // automatically via the existing Spotify → YouTube/YouTube Music matching
                // flow instead of immediately requiring manual YouTube selection.
                match self.try_download_via_spotify_match(&mut track).await {
                    Some(path) => Some(path),
                    None => {
                        tracing::warn!(
                            "No usable Spotify match — marking for manual YouTube selection"
                        );
                        // DRM means there is no downloadable audio at all, which the
                        // metadata-match tabs cannot resolve (Select there only re-tags an
                        // existing staged file). It must override any weak-metadata reason
                        // `enrich_metada` set earlier, so the track lands in the DRM tab where
                        // the user can pick a YouTube source to actually fetch the audio.
                        track.needs_validation = true;
                        track.validation_reason = Some("soundcloud_drm_protected".to_string());
                        None
                    }
                }
            }
            Err(e) => return Err(e),
        };
        // Repair path: if this source is already a finalized library track whose
        // audio file has gone missing, adopt the fresh download into it — keeping
        // its reviewed metadata and identity — regardless of the enrich/dedup
        // outcome below (which would otherwise keep the broken, fileless row or
        // discard the download as "no better quality").
        if let Some(new_file) = &file_path_opt {
            if let Some(existing) = self.existing_staged_track(conn, &track) {
                if !existing.needs_validation && !self.library_file_present(&existing) {
                    tracing::info!(
                        "Repairing missing library file for {} from re-download",
                        existing.display()
                    );
                    let mut repaired = existing;
                    self.process_track_file(&mut repaired, new_file).await?;
                    return self.save_track(conn, &repaired).await;
                }
            }
        }

        if should_validate || file_path_opt.is_none() {
            if let Some(existing) = self.existing_staged_track(conn, &track) {
                if !existing.needs_validation {
                    // Already finalized and reviewed (any missing file was repaired
                    // above). A re-sync must not re-validate it: discard the freshly
                    // staged copy and keep the library entry as-is.
                    tracing::info!(
                        "Source already finalized as {} — skipping re-validation",
                        existing.display()
                    );
                    if let Some(staged) = &file_path_opt {
                        if let Err(e) = std::fs::remove_file(staged) {
                            tracing::warn!(
                                "Could not remove staging file {}: {}",
                                staged.display(),
                                e
                            );
                        }
                    }
                    return Ok(existing);
                }

                // Still pending validation: reuse its row so the queue keeps one
                // entry per track (a second row would orphan a staged file).
                tracing::warn!(
                    "Track saved for manual validation — reason={:?}",
                    track.validation_reason
                );
                return self.replace_staged_track(conn, existing, track).await;
            }

            tracing::warn!(
                "Track saved for manual validation — reason={:?}",
                track.validation_reason
            );
            let saved_track = self.save_track(conn, &track).await?;
            return Ok(saved_track);
        }

        let file_path = file_path_opt.expect("checked is_none above");

        // Step 3: Deduplication
        if existing_track.is_none() {
            tracing::info!("Deduping track in database");
            existing_track = self.dedupe_track(conn, &track).await;
        }

        match existing_track {
            Some(existing_track) => {
                tracing::info!(
                    "Existing track found in DB: {}, will compare quality",
                    existing_track.display()
                );

                let mut existing_track = existing_track;
                let new_track_is_better_quality = self
                    .track_service
                    .is_better_quality(&existing_track, &track);

                if new_track_is_better_quality {
                    tracing::warn!("New one has better quality, will replace");

                    // Merge nested metadata refs (album/artists) from the new track, then swap source/provider.
                    let mut track_for_merge = track.clone();
                    normalize_album_and_artist_refs_as_metadata(&mut track_for_merge);
                    existing_track.transpose_refs(&track_for_merge);
                    apply_source_provider_replacement(&mut existing_track, &track);

                    self.process_track_file(&mut existing_track, &file_path)
                        .await?;
                    let updated_track = self.save_track(conn, &existing_track).await?;
                    Ok(updated_track)
                } else {
                    tracing::warn!("New one has no better quality, skipping");

                    // Keep current audio source/provider, but keep Spotify (and downloader provider) as Metadata refs.
                    let mut track_for_merge = track.clone();
                    normalize_album_and_artist_refs_as_metadata(&mut track_for_merge);
                    demote_track_source_and_provider_to_metadata(&mut track_for_merge);
                    existing_track.transpose_refs(&track_for_merge);

                    let updated_track = self.save_track(conn, &existing_track).await?;
                    let _ = self.track_service.delete_track_file(&track)?;
                    Ok(updated_track)
                }
            }
            None => {
                tracing::info!("No existing track found in DB, processing new track");
                // Final Step: Tagging, moving and saving in DB
                self.process_track_file(&mut track, &file_path).await?;
                let inserted_track = self.save_track(conn, &track).await?;
                Ok(inserted_track)
            }
        }
    }

    /// Enrich metadata using metadata providers, and deduplicate entities in DB
    ///
    /// `for_ingest` — when `true`, uses `ingest_metadata_providers` from config
    /// (Spotify-first by default) instead of the standard download order.
    ///
    /// Returns:
    /// - boolean indicating if the track should be marked as "to validate"
    /// - boolean indicating if the track should be compared in quality (already exists in DB)
    async fn enrich_metada(
        &self,
        conn: &mut SqliteConnection,
        track: &mut Track,
        for_ingest: bool,
    ) -> SoundgnomeResult<(bool, Option<Track>)> {
        // Check if album/artists with same source ref url exist in DB and associate them
        let existing_album = track.album.as_ref().and_then(|a| {
            a.get_source()
                .or_else(|| a.get_metadata())
                .and_then(|s| s.external_url)
                .and_then(|url| self.album_service.get_by_url(conn, &url))
        });
        if let Some(existing_album) = existing_album {
            track.album = Some(existing_album);
        }

        for artist in &mut track.artists {
            if let Some(existing_artist) = artist
                .get_source()
                .or_else(|| artist.get_metadata())
                .and_then(|s| s.external_url)
                .and_then(|url| self.artist_service.get_by_url(conn, &url))
            {
                *artist = existing_artist;
            }
        }

        // Get metadata from all enabled providers
        let best_match = if for_ingest {
            tagger::enricher::get_best_match_from_track_for_ingest(track).await
        } else {
            tagger::enricher::get_best_match_from_track(track).await
        };

        // Apply best match metadata
        if let Match::Exact(matched_track) = best_match {
            // TODO: Check if ref already exists in DB, if yes then apply references recursively to track and unfound album/artists
            tracing::info!(
                "Exact match found from metadata provider: {:?}",
                matched_track.get_metadata().and_then(|m| m.external_url)
            );
            // find for existing tracks in the database

            if let Some(mb_ref) = matched_track
                .get_metadata()
                .and_then(|s| s.external_url.clone())
            {
                if let Some(existing_track) = self.track_service.get_by_url(conn, &mb_ref) {
                    tracing::warn!(
                        "Track already exists in DB with MusicBrainz ref: {}, skipping enrichment",
                        existing_track.display()
                    );
                    return Ok((false, Some(existing_track)));
                }
            }

            // Check if album/artists with same musicbrainz source url exist in DB and associate them
            let existing_album = track.album.as_ref().and_then(|a| {
                a.get_source()
                    .or_else(|| a.get_metadata())
                    .and_then(|s| s.external_url)
                    .and_then(|url| self.album_service.get_by_url(conn, &url))
            });
            if let Some(existing_album) = existing_album {
                track.album = Some(existing_album);
            }

            for artist in &mut track.artists {
                if let Some(existing_artist) = artist
                    .get_source()
                    .or_else(|| artist.get_metadata())
                    .and_then(|s| s.external_url)
                    .and_then(|url| self.artist_service.get_by_url(conn, &url))
                {
                    *artist = existing_artist;
                }
            }

            // Exact match: treat the enricher result as an authoritative, high-confidence
            // source so cleaner metadata (e.g. artist names) always replaces noisy source data.
            track.transpose_metadata_from_source(&matched_track);
            Ok((false, None)) // no need to validate
        } else if let Match::Partial(matched_track) = best_match {
            // Partial match: keep current (source) metadata, but attach MusicBrainz IDs/URLs for later review.
            // Do NOT transpose album data from partial match to avoid introducing incorrect album info.
            tracing::warn!(
                "Partial match found from metadata providers - will mark for validation"
            );

            track.transpose_refs_without_album(&matched_track);
            track.needs_validation = true;
            track.validation_reason = Some("metadata_partial_match".to_string());

            Ok((true, None))
        } else {
            // TODO: No match -> mark as "to validate"
            tracing::warn!("No match found from metadata providers");
            track.needs_validation = true;
            track.validation_reason = Some("metadata_no_match".to_string());
            Ok((true, None))
        }
    }

    /// Searches for the best download URL and downloads the track
    ///
    /// Returns the downloaded track with updated references and file_path
    /// Searches for the best download URL and downloads the track to the staging folder.
    /// The staging path is stored in `track.file_path`.
    async fn download_track(&self, track: &mut Track) -> SoundgnomeResult<PathBuf> {
        // Get the best download URL
        let provider_ref = downloader::search(track).await?;
        tracing::info!(
            "Found download URL from {:?}: {:?}",
            provider_ref.platform,
            provider_ref.external_url
        );
        track.references.push(provider_ref.clone());

        let staging_dir = PathBuf::from(&Config::get().general.temp_download_dir);

        // Download the track to staging
        let file_path = downloader::download(
            &track
                .get_source()
                .ok_or(Error::Custom("track source not defined".to_string()))?,
            &provider_ref,
            &staging_name(&track.title),
            staging_dir,
        )
        .await?;
        tracing::info!("Downloaded track to staging: {:?}", file_path);
        track.file_path = file_path.clone().into();

        Ok(file_path)
    }

    /// When a SoundCloud download fails as DRM-protected, check whether the track already
    /// carries a Spotify `Metadata` reference (typically attached during enrichment in
    /// `enrich_metada`, since Spotify is one of the tagger metadata providers). If so, retry
    /// via the existing Spotify → YouTube/YouTube Music matching flow (`downloader::search`'s
    /// `Platform::Spotify` branch) instead of immediately requiring manual YouTube selection.
    ///
    /// The track's `Source` reference is left untouched — SoundCloud is still where the user
    /// asked Soundgnome to import from. Only the resolved `Provider` reference and staged
    /// `file_path` are attached, and only on success.
    ///
    /// Returns `Some(path)` when the fallback download succeeded. Returns `None` when there is
    /// no Spotify metadata reference, or the fallback search/download itself failed, so the
    /// caller can fall back to the existing manual validation flow unchanged.
    async fn try_download_via_spotify_match(&self, track: &mut Track) -> Option<PathBuf> {
        let spotify_ref = track
            .references
            .iter()
            .find(|r| r.ref_type == ReferenceType::Metadata && r.platform == Platform::Spotify)?
            .clone();

        tracing::info!(
            "DRM-protected SoundCloud track has a known Spotify reference ({:?}) — retrying via Spotify matching flow",
            spotify_ref.external_url
        );

        // Reuse `downloader::search`'s existing Spotify matching flow (YouTube Music, falling
        // back to YouTube) by presenting the Spotify reference as the `Source` on a throwaway
        // probe. `track`'s own `Source` reference (SoundCloud) is not modified.
        let mut probe = track.clone();
        probe
            .references
            .retain(|r| r.ref_type != ReferenceType::Source);
        probe.references.push(Reference {
            id: None,
            ref_type: ReferenceType::Source,
            platform: Platform::Spotify,
            external_id: spotify_ref.external_id,
            external_url: spotify_ref.external_url,
        });

        let provider_ref = match downloader::search(&probe).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Spotify-match fallback search found no candidate: {}", e);
                return None;
            }
        };
        tracing::info!(
            "Spotify-match fallback resolved a download URL from {:?}: {:?}",
            provider_ref.platform,
            provider_ref.external_url
        );

        // Download using the real (SoundCloud) source reference — `downloader::download`
        // already supports pairing a SoundCloud source with a YouTube/YouTube Music provider,
        // the same dispatch used by the manual DRM-fallback validation flow in
        // `finalize_validated_track`.
        let source_ref = track.get_source()?;
        let staging_dir = PathBuf::from(&Config::get().general.temp_download_dir);
        match downloader::download(
            &source_ref,
            &provider_ref,
            &staging_name(&track.title),
            staging_dir,
        )
        .await
        {
            Ok(path) => {
                track.file_path = Some(path.clone());
                track.references.push(provider_ref);
                Some(path)
            }
            Err(e) => {
                tracing::warn!("Spotify-match fallback download failed: {}", e);
                None
            }
        }
    }

    /// Simple deduplication based on comparition of title and artist(s) against existing tracks in DB
    async fn dedupe_track(&self, conn: &mut SqliteConnection, track: &Track) -> Option<Track> {
        let result = self
            .track_service
            .find_track_by_title_and_artist(conn, track);

        match result {
            Some(existing_track) => {
                tracing::warn!(
                    "Similar track found in DB: {}, will compare quality",
                    existing_track.display()
                );
                Some(existing_track)
            }
            None => None,
        }
    }

    /// Acoustic (Chromaprint) deduplication: compare `new_fp` against the stored
    /// fingerprints of tracks with a comparable duration and return the first that
    /// overlaps strongly enough to be the same recording. Catches re-encodes and
    /// format changes that the exact-hash and title/artist tiers miss.
    async fn dedupe_by_fingerprint(
        &self,
        conn: &mut SqliteConnection,
        track: &Track,
        new_fp: &[u32],
    ) -> Option<Track> {
        let (min_secs, max_secs) = match track.duration {
            Some(d) => (
                d - FINGERPRINT_DURATION_TOLERANCE_SECS,
                d + FINGERPRINT_DURATION_TOLERANCE_SECS,
            ),
            None => (i32::MIN, i32::MAX),
        };

        let candidates = self
            .track_service
            .fingerprint_candidates(conn, min_secs, max_secs)
            .unwrap_or_else(|e| {
                tracing::warn!("Ingest: fingerprint candidate lookup failed: {e}");
                Vec::new()
            });

        for (track_id, encoded) in candidates {
            let Some(cand_fp) = encoded
                .strip_prefix(CHROMAPRINT_PREFIX)
                .and_then(decode_fingerprint)
            else {
                continue;
            };
            let overlap = matched_overlap_secs(new_fp, &cand_fp);
            let is_match = match track.duration {
                Some(d) if d > 0 => overlap >= FINGERPRINT_MIN_COVERAGE * d as f32,
                _ => overlap >= FINGERPRINT_MIN_ABS_MATCH_SECS,
            };
            if is_match {
                tracing::info!(
                    "Ingest: acoustic fingerprint match (track_id={}, overlap={:.1}s)",
                    track_id,
                    overlap
                );
                if let Ok(existing) = self.track_service.get_by_id(conn, track_id) {
                    return Some(existing);
                }
            } else if overlap > 0.0 {
                tracing::debug!(
                    "Ingest: fingerprint near-miss (track_id={}, overlap={:.1}s)",
                    track_id,
                    overlap
                );
            }
        }
        None
    }

    /// Resolve an ingested file against an already-known duplicate (found by
    /// acoustic fingerprint or by title/artist): keep whichever copy is higher
    /// quality. When the incoming file wins, it replaces the existing audio (and
    /// the lower-quality original is deleted); otherwise the existing audio is kept
    /// and only useful metadata is merged in. Either way the library ends with a
    /// single, best-quality copy.
    async fn resolve_existing_match(
        &self,
        conn: &mut SqliteConnection,
        mut existing_track: Track,
        track: &Track,
        file_path: &Path,
    ) -> SoundgnomeResult<(Track, IngestOutcome)> {
        tracing::info!(
            "Ingest: duplicate of existing track {}, comparing quality",
            existing_track.display()
        );

        let new_is_better = self.track_service.is_better_quality(&existing_track, track);

        if new_is_better {
            tracing::info!("Ingest: uploaded file is higher quality, replacing existing copy");

            // Remember the current library file so it can be discarded once the
            // higher-quality upload is organized into place (the new file may land
            // at a different path when the format/extension differs).
            let old_path = existing_track.file_path.clone();

            let mut track_for_merge = track.clone();
            normalize_album_and_artist_refs_as_metadata(&mut track_for_merge);
            existing_track.transpose_refs(&track_for_merge);
            apply_source_provider_replacement(&mut existing_track, track);

            self.process_track_file(&mut existing_track, file_path)
                .await?;
            let updated = self.save_track(conn, &existing_track).await?;

            if let Some(old) = old_path {
                if existing_track.file_path.as_ref() != Some(&old) && old.exists() {
                    if let Err(e) = std::fs::remove_file(&old) {
                        tracing::warn!("Ingest: could not remove superseded file {old:?}: {e}");
                    }
                }
            }
            Ok((updated, IngestOutcome::Replaced))
        } else {
            tracing::info!("Ingest: existing copy is equal or higher quality, keeping it");

            // Keep existing audio; merge useful metadata from the ingested file.
            let mut track_for_merge = track.clone();
            normalize_album_and_artist_refs_as_metadata(&mut track_for_merge);
            demote_track_source_and_provider_to_metadata(&mut track_for_merge);
            existing_track.transpose_refs(&track_for_merge);

            let updated = self.save_track(conn, &existing_track).await?;
            Ok((updated, IngestOutcome::Duplicate))
        }
    }

    /// Tag the downloaded file with the track metadata, then move it to the correct location
    async fn process_track_file(
        &self,
        track: &mut Track,
        file_path: &Path,
    ) -> SoundgnomeResult<()> {
        // Assign a SOUNDOME_ID if the track does not already have one.
        if track.soundome_id.is_none() {
            track.soundome_id = Some(Uuid::new_v4().to_string());
            tracing::debug!("Assigned SOUNDOME_ID: {:?}", track.soundome_id);
        }

        // The `file_path` argument is the file that was just downloaded/staged and
        // must be tagged and moved into the library. Always adopt it as the track's
        // path so the organizer moves *this* file. In the dedup-replace path the
        // track carries a stale existing-library path; honoring that instead fails
        // the move with ENOENT (the old file may be gone or in a different folder).
        track.file_path = Some(file_path.to_path_buf());

        // When the source metadata carried no cover, derive one from the track's
        // references so the artwork gets embedded into the file at tag time. This
        // keeps art in the library offline, instead of resolving it per-play.
        if track.cover.is_none() {
            if let Some(url) = resolve_cover_url(track).await {
                tracing::info!("Resolved cover art for '{}' from references", track.title);
                track.cover = Some(url);
            }
        }

        // Best-effort: download cover art (highest available resolution) and
        // embed it in the file.
        let cover_bytes: Option<Vec<u8>> = match track.cover.clone() {
            Some(url) => fetch_cover_bytes(url).await,
            None => None,
        };

        tagger::file::tag_file_with_track_and_cover(
            &file_path.to_path_buf(),
            track,
            cover_bytes.as_deref(),
        )?;
        tracing::info!("Tagged file with track metadata");

        // Move the file to the correct location
        let base_library_dir = Config::get().general.base_library_dir.clone();
        organizer::move_track_file(track, &base_library_dir)?;

        Ok(())
    }

    /// Re-tag and reorganize a track file if its metadata (especially artist or album) has changed.
    /// This is used when a user edits track metadata via the API.
    ///
    /// Returns true if the file was moved to a new location.
    pub async fn update_track_file_metadata(
        &self,
        old_track: &Track,
        new_track: &mut Track,
    ) -> SoundgnomeResult<bool> {
        // Check if the track has a file to update
        let mut file_path = match &old_track.file_path {
            Some(path) => path.clone(),
            None => {
                tracing::debug!("Track has no file, skipping file update");
                return Ok(false);
            }
        };

        // Resolve relative paths by joining with base_library_dir, but avoid duplication
        if !file_path.is_absolute() {
            let base_dir = PathBuf::from(&Config::get().general.base_library_dir);
            let base_dir_str = base_dir.to_string_lossy();
            let file_path_str = file_path.to_string_lossy();

            // Only join if the file_path doesn't already start with the base_dir
            if !file_path_str.starts_with(base_dir_str.as_ref()) {
                file_path = base_dir.join(&file_path);
            }
            tracing::debug!("Resolved path to: {:?}", file_path);
        }

        // Check if file still exists
        if !file_path.exists() {
            tracing::warn!(
                "Track file does not exist at {:?}, skipping file update",
                file_path
            );
            return Ok(false);
        }

        // Check if artist, album, or title metadata has changed
        let artist_names_changed = old_track
            .artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            != new_track
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>();

        let album_changed = old_track.album.as_ref().map(|a| a.title.clone())
            != new_track.album.as_ref().map(|a| a.title.clone());

        let title_changed = old_track.title != new_track.title;

        let location_changed = artist_names_changed || album_changed || title_changed;

        // Re-tag the file with new metadata
        tracing::info!("Re-tagging file with updated metadata");
        tagger::file::tag_file_with_track(&file_path, new_track)?;

        // If any location-affecting metadata changed (artist, album, or title), reorganize the file.
        // This is safer than renaming directly because the organizer handles path normalization.
        if location_changed {
            tracing::info!(
                "Metadata changed (artist={}, album={}, title={}), reorganizing file",
                artist_names_changed,
                album_changed,
                title_changed
            );
            // Update the file_path in new_track to the resolved absolute path before organizing
            new_track.file_path = Some(file_path);
            let base_library_dir = Config::get().general.base_library_dir.clone();
            organizer::move_track_file(new_track, &base_library_dir)?;

            // Normalize the file_path back to relative for storage in DB
            // The file_path is now absolute, so make it relative to base_library_dir
            if let Some(abs_path) = &new_track.file_path {
                let base_path = std::path::PathBuf::from(&base_library_dir);
                if let Ok(rel_path) = abs_path.strip_prefix(&base_path) {
                    let rel_path_str = format!("./{}", rel_path.to_string_lossy());
                    new_track.file_path = Some(std::path::PathBuf::from(rel_path_str));
                    tracing::debug!("Normalized path to relative: {:?}", new_track.file_path);
                }
            }
            Ok(true)
        } else {
            tracing::debug!("File location unchanged, no reorganization needed");
            Ok(false)
        }
    }

    /// Resolve a possibly-relative library `file_path` against `base_library_dir`,
    /// mirroring [`Self::update_track_file_metadata`]'s resolution rules.
    fn resolve_library_path(&self, file_path: &Path) -> PathBuf {
        if file_path.is_absolute() {
            return file_path.to_path_buf();
        }
        let base = PathBuf::from(&Config::get().general.base_library_dir);
        if file_path
            .to_string_lossy()
            .starts_with(base.to_string_lossy().as_ref())
        {
            file_path.to_path_buf()
        } else {
            base.join(file_path)
        }
    }

    /// Whether the track's audio file exists on disk (resolving relative paths).
    pub fn library_file_present(&self, track: &Track) -> bool {
        track
            .file_path
            .as_ref()
            .is_some_and(|fp| self.resolve_library_path(fp).exists())
    }

    /// Finalized library tracks whose audio file is missing on disk. A track with a
    /// `soundome_id` was organized into the library; if its file is gone it is a
    /// broken, resyncable entry rather than a validation candidate. Staged
    /// (`needs_validation`) rows and rows still pointing at cleaned-up staging
    /// files are excluded.
    pub fn list_missing_files(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>> {
        Ok(self
            .track_service
            .get_all_finalized(conn)?
            .into_iter()
            .filter(|t| t.soundome_id.is_some() && !t.needs_validation)
            .filter(|t| !self.library_file_present(t))
            .collect())
    }

    /// The row already holding this source URL, if any.
    fn existing_staged_track(&self, conn: &mut SqliteConnection, track: &Track) -> Option<Track> {
        let url = track.get_source().and_then(|s| s.external_url)?;
        self.track_service.get_by_url(conn, &url)
    }

    /// Fold a freshly downloaded copy into the row that already exists, keeping
    /// whichever file is better and deleting the other.
    async fn replace_staged_track(
        &self,
        conn: &mut SqliteConnection,
        existing: Track,
        mut track: Track,
    ) -> SoundgnomeResult<Track> {
        let new_is_better = self.track_service.is_better_quality(&existing, &track);

        let discarded = if new_is_better {
            tracing::info!(
                "Replacing staged file for {} with the better download",
                existing.display()
            );
            existing.file_path.clone()
        } else {
            tracing::info!(
                "Keeping the existing file for {}, the new download is not better",
                existing.display()
            );
            let new_file = track.file_path.clone();
            track.file_path = existing.file_path.clone();
            new_file
        };

        if let Some(path) = discarded {
            if Some(&path) != track.file_path.as_ref() {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Could not remove {}: {}", path.display(), e);
                }
            }
        }

        // Same row, so the validation queue keeps one entry per track.
        track.id = existing.id;
        self.save_track(conn, &track).await
    }

    /// Save the track in the database
    async fn save_track(
        &self,
        conn: &mut SqliteConnection,
        track: &Track,
    ) -> SoundgnomeResult<Track> {
        let inserted_track = self.track_service.create_or_update(conn, track)?;
        tracing::info!("Saved track in the database");
        Ok(inserted_track)
    }

    /// Best-effort M3U8 export: fetch playlist tracks from DB and write the file.
    /// Failures are logged as warnings and do not propagate.
    fn export_playlist_m3u8(
        &self,
        conn: &mut SqliteConnection,
        playlist: &Playlist,
        playlist_id: i32,
    ) {
        match self.playlist_service.export_m3u8(conn, playlist_id) {
            Ok(path) => tracing::info!("M3U8 playlist exported: {:?}", path),
            Err(e) => tracing::warn!(
                "M3U8 export failed for playlist \"{}\": {}",
                playlist.name,
                e
            ),
        }
    }
}

/// Stream a file through SHA-256 and return the lowercase hex digest. Used to
/// detect byte-identical re-uploads during ingest.
fn sha256_file(path: &Path) -> SoundgnomeResult<String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| Error::Custom(format!("Failed to open {path:?}: {e}")))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| Error::Custom(format!("Failed to hash {path:?}: {e}")))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// URL prefix under which a track's Chromaprint acoustic fingerprint is stored as a
/// Metadata reference. MUST match the LIKE pattern in
/// `DieselTrackRepository::fingerprint_candidates`.
const CHROMAPRINT_PREFIX: &str = "soundome:chromaprint:";

/// Cap acoustic-fingerprint decoding to the opening of each track. Chromaprint
/// matches on the first couple of minutes, so decoding whole files is wasted work.
const FINGERPRINT_MAX_SECS: &str = "120";

/// Everything needed to commit one ingested file, computed off the DB/network so
/// it can run on a blocking thread in parallel (see `ingest_local_dir`).
struct PreparedIngest {
    track: Track,
    file_path: PathBuf,
    /// Hex SHA-256 of the raw bytes (exact-duplicate key).
    content_hash: String,
    /// Acoustic fingerprint, or `None` when decoding failed (best-effort).
    fingerprint: Option<Vec<u32>>,
    /// Whether the source file carries embedded cover art.
    has_cover: bool,
}

/// Pure, blocking per-file work: read tags, infer the track number, hash the raw
/// bytes, and compute the acoustic fingerprint. No DB, no network — safe to run
/// on a blocking thread so many files decode at once.
fn prepare_ingest_file(file_path: &Path) -> SoundgnomeResult<PreparedIngest> {
    let mut track = tagger::file::get_track_from_file(&file_path.to_path_buf())
        .map_err(|e| Error::Custom(format!("Failed to read audio tags: {e}")))?;
    if track.track_number.is_none() {
        track.track_number = infer_track_number_from_filename(file_path);
    }
    track.file_path = Some(file_path.to_path_buf());
    let content_hash = sha256_file(file_path)?;
    let fingerprint = match compute_fingerprint(file_path) {
        Ok(fp) => Some(fp),
        Err(e) => {
            tracing::warn!("Ingest: fingerprint unavailable for {file_path:?}: {e}");
            None
        }
    };
    let has_cover = tagger::file::read_cover_from_path(file_path).is_some();
    Ok(PreparedIngest {
        track,
        file_path: file_path.to_path_buf(),
        content_hash,
        fingerprint,
        has_cover,
    })
}

/// True when a file's own tags are complete enough to trust without a metadata
/// provider lookup: a non-empty title, at least one named artist, and an album.
fn tags_complete(track: &Track) -> bool {
    !track.title.trim().is_empty()
        && track.artists.iter().any(|a| !a.name.trim().is_empty())
        && track
            .album
            .as_ref()
            .is_some_and(|a| !a.title.trim().is_empty())
}

/// Parallel-prepare width for batch ingest. `general.ingest_concurrency` overrides;
/// 0 = auto (CPU count, clamped to a sane range). The DB commit stays serial.
fn ingest_concurrency() -> usize {
    let configured = Config::get().general.ingest_concurrency;
    if configured > 0 {
        return configured;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8)
}

/// Duration window (seconds) for narrowing acoustic dedup candidates: only tracks
/// whose length is within this many seconds of the incoming file are compared.
/// Kept wide enough to still catch re-encodes with slightly different trailing
/// silence; the fingerprint overlap check is the real identity gate.
const FINGERPRINT_DURATION_TOLERANCE_SECS: i32 = 30;

/// A matched segment counts toward coverage only when its alignment score is at or
/// below this. Chromaprint scores are Hamming-distance based (0 = identical), so
/// re-encodes of the same master score very low while unrelated audio scores high.
const FINGERPRINT_MAX_SEGMENT_SCORE: f64 = 8.0;

/// Fraction of the incoming track that low-score matched segments must cover for
/// the two recordings to be treated as the same.
const FINGERPRINT_MIN_COVERAGE: f32 = 0.50;

/// Absolute matched seconds required when the incoming track's duration is unknown
/// (so coverage-by-fraction cannot be computed).
const FINGERPRINT_MIN_ABS_MATCH_SECS: f32 = 45.0;

/// In loose dedup mode, two copies in the same title+artist group are treated as
/// the same song when their durations are within this many seconds - even if
/// Chromaprint cannot confirm it (e.g. a different master of the same track).
const LOOSE_DURATION_SECS: i32 = 5;

/// Decode `path` to 44.1 kHz stereo PCM via ffmpeg and compute its Chromaprint
/// acoustic fingerprint. Decoding leans on the ffmpeg binary already required for
/// downloads, so every ingestable format (opus, m4a, ...) is handled uniformly.
fn compute_fingerprint(path: &Path) -> SoundgnomeResult<Vec<u32>> {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-t", FINGERPRINT_MAX_SECS, "-i"])
        .arg(path)
        .args(["-f", "s16le", "-ac", "2", "-ar", "44100", "-"])
        .output()
        .map_err(|e| Error::Custom(format!("ffmpeg spawn failed for {path:?}: {e}")))?;

    if !output.status.success() {
        return Err(Error::Custom(format!(
            "ffmpeg decode failed for {path:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let samples: Vec<i16> = output
        .stdout
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();

    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer
        .start(44100, 2)
        .map_err(|e| Error::Custom(format!("fingerprinter init failed: {e:?}")))?;
    printer.consume(&samples);
    printer.finish();

    let fp = printer.fingerprint().to_vec();
    if fp.is_empty() {
        return Err(Error::Custom(format!("empty fingerprint for {path:?}")));
    }
    Ok(fp)
}

/// Encode a fingerprint as fixed-width hex (8 chars per `u32`) for storage in a
/// reference URL. Zero-dependency and round-trips exactly.
fn encode_fingerprint(fp: &[u32]) -> String {
    let mut s = String::with_capacity(fp.len() * 8);
    for v in fp {
        s.push_str(&format!("{v:08x}"));
    }
    s
}

/// Inverse of [`encode_fingerprint`]. Returns `None` on any malformed input.
fn decode_fingerprint(encoded: &str) -> Option<Vec<u32>> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(8) {
        return None;
    }
    (0..encoded.len())
        .step_by(8)
        .map(|i| u32::from_str_radix(&encoded[i..i + 8], 16).ok())
        .collect()
}

/// Seconds of well-aligned (low-score) overlap between two fingerprints. Zero when
/// they do not match.
fn matched_overlap_secs(fp_a: &[u32], fp_b: &[u32]) -> f32 {
    let config = Configuration::preset_test2();
    let Ok(segments) = match_fingerprints(fp_a, fp_b, &config) else {
        return 0.0;
    };
    segments
        .iter()
        .filter(|s| s.score <= FINGERPRINT_MAX_SEGMENT_SCORE)
        .map(|s| s.duration(&config))
        .sum()
}

/// Whether two tracks are the same recording: both have a fingerprint and their
/// well-aligned overlap covers a large fraction of the shorter one (so a truncated
/// copy still matches the full one, but different songs never do).
fn same_recording(
    a: Option<&Vec<u32>>,
    b: Option<&Vec<u32>>,
    da: Option<i32>,
    db: Option<i32>,
    loose: bool,
) -> bool {
    // Loose mode: near-identical durations within the same title+artist group are
    // the same song even when Chromaprint can't confirm (different masters).
    if loose {
        if let (Some(x), Some(y)) = (da, db) {
            if (x - y).abs() <= LOOSE_DURATION_SECS {
                return true;
            }
        }
    }
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let overlap = matched_overlap_secs(a, b);
    let shorter = match (da, db) {
        (Some(x), Some(y)) => x.min(y).max(1),
        (Some(x), None) | (None, Some(x)) => x.max(1),
        _ => 0,
    };
    if shorter > 0 {
        overlap >= 0.45 * shorter as f32
    } else {
        overlap >= 30.0
    }
}

/// Human-readable quality string for a dedup report entry, e.g. "FLAC 1580kbps lossless".
fn quality_label(track: &Track) -> String {
    let ext = track
        .file_path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .unwrap_or("?")
        .to_uppercase();
    match track.audio_quality() {
        Some(q) => format!(
            "{} {}kbps{}",
            ext,
            q.bitrate_bps / 1000,
            if q.lossless { " lossless" } else { "" }
        ),
        None => ext,
    }
}

fn dedupe_track_summary(track: &Track, rating: Option<shared::models::Rating>) -> DedupeTrack {
    DedupeTrack {
        id: track.id.unwrap_or(0),
        title: track.title.clone(),
        artist: track
            .artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        duration: track.duration,
        quality: quality_label(track),
        needs_validation: track.needs_validation,
        rating: rating.map(|r| format!("{r:?}").to_lowercase()),
        file_path: track.file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
    }
}

fn normalize_album_and_artist_refs_as_metadata(track: &mut Track) {
    if let Some(album) = &mut track.album {
        for r in &mut album.references {
            r.ref_type = ReferenceType::Metadata;
            r.id = None;
        }
        for artist in &mut album.artists {
            for r in &mut artist.references {
                r.ref_type = ReferenceType::Metadata;
                r.id = None;
            }
        }
    }

    for artist in &mut track.artists {
        for r in &mut artist.references {
            r.ref_type = ReferenceType::Metadata;
            r.id = None;
        }
    }
}

fn demote_track_source_and_provider_to_metadata(track: &mut Track) {
    for r in &mut track.references {
        if r.ref_type == ReferenceType::Source || r.ref_type == ReferenceType::Provider {
            r.ref_type = ReferenceType::Metadata;
            r.id = None;
        }
    }
}

fn same_ref_identity(a: &shared::models::Reference, b: &shared::models::Reference) -> bool {
    a.platform == b.platform && a.external_id == b.external_id && a.external_url == b.external_url
}

/// Try to extract a track number from a file name when the embedded tag is absent.
///
/// Recognises common patterns:
///   "08 - Title.flac"   → 8
///   "08_Title.flac"     → 8
///   "08. Title.flac"    → 8
///   "08Title.flac"      → 8  (leading digits only)
///   "Track08.flac"      → ignored (no leading digits)
fn infer_track_number_from_filename(path: &Path) -> Option<i32> {
    let stem = path.file_stem()?.to_string_lossy();
    // Match 1–3 leading digits optionally followed by a separator character.
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    digits
        .parse::<i32>()
        .ok()
        .filter(|&n| (1..=999).contains(&n))
}

fn apply_source_provider_replacement(existing_track: &mut Track, new_track: &Track) {
    let new_source = new_track.get_source();
    let new_provider = new_track.get_provider();

    // If we cannot determine both, do nothing (better to keep existing state).
    let (Some(new_source), Some(new_provider)) = (new_source, new_provider) else {
        return;
    };

    let old_source = existing_track.get_source();
    let old_provider = existing_track.get_provider();

    // Remove all existing Source/Provider refs; we'll re-add exactly one of each.
    existing_track
        .references
        .retain(|r| r.ref_type != ReferenceType::Source && r.ref_type != ReferenceType::Provider);

    let mut new_source = new_source;
    new_source.id = None;
    new_source.ref_type = ReferenceType::Source;
    let mut new_provider = new_provider;
    new_provider.id = None;
    new_provider.ref_type = ReferenceType::Provider;

    existing_track.references.push(new_source.clone());
    existing_track.references.push(new_provider.clone());

    // Demote old source/provider as metadata (dedupe if they were identical).
    let mut candidates: Vec<shared::models::Reference> = Vec::new();
    if let Some(old_source) = old_source {
        if !same_ref_identity(&old_source, &new_source) {
            let mut r = old_source;
            r.id = None;
            r.ref_type = ReferenceType::Metadata;
            candidates.push(r);
        }
    }
    if let Some(old_provider) = old_provider {
        if !same_ref_identity(&old_provider, &new_provider) {
            let mut r = old_provider;
            r.id = None;
            r.ref_type = ReferenceType::Metadata;
            candidates.push(r);
        }
    }

    for candidate in candidates {
        let already = existing_track.references.iter().any(|r| {
            r.ref_type == candidate.ref_type
                && r.platform == candidate.platform
                && r.external_id == candidate.external_id
                && r.external_url == candidate.external_url
        });
        if !already {
            existing_track.references.push(candidate);
        }
    }
}
