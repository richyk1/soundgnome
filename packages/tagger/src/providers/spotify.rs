//! Spotify metadata enrichment provider.
//!
//! Uses the single Spotify user session (the librespot login) via the shared
//! `fetcher::spotify::webapi` client. There is no app client-credentials path:
//! enrichment is available whenever Spotify is connected, and unavailable
//! otherwise.

use fetcher::spotify::{session, webapi};
use shared::models::{ReferenceType, Track};
use shared::utils::enums::Match;
use shared::utils::string::{string_similarity, SimilarityAlgorithm};

use crate::TagProvider;

pub struct Spotify;

impl Spotify {
    const EXACT_MATCH_THRESHOLD: f64 = 0.8;
    const PARTIAL_MATCH_THRESHOLD: f64 = 0.5;

    /// Available once the user has connected Spotify. Enrichment reuses that
    /// session's Web API token; nothing here fetches audio.
    pub fn new() -> Option<Self> {
        if session::stored_session().is_some() {
            Some(Self)
        } else {
            tracing::debug!("Spotify metadata provider: not connected, skipping");
            None
        }
    }

    async fn search_tracks(&self, query: &str) -> Vec<Track> {
        match webapi::search_tracks(query, 10, ReferenceType::Metadata).await {
            Ok(tracks) => tracks,
            Err(e) => {
                tracing::warn!("Spotify search failed for query {:?}: {}", query, e);
                Vec::new()
            }
        }
    }

    /// Search Spotify for an artist by name and return the first image URL found.
    /// Best-effort: returns `None` on any error or when no image is available.
    async fn fetch_artist_icon(&self, artist_name: &str) -> Option<String> {
        webapi::artist_icon(artist_name).await
    }

    /// Backfill `icon` on a track's artists that currently have none. One search
    /// per artist without an icon; best-effort, never fails the enrichment.
    async fn enrich_artist_icons(&self, track: &mut Track) {
        for artist in &mut track.artists {
            if artist.icon.is_some() {
                continue;
            }
            if let Some(url) = self.fetch_artist_icon(&artist.name).await {
                artist.icon = Some(url);
            }
        }
    }
}

impl TagProvider for Spotify {
    async fn get_best_match_from_track(&self, track: &Track) -> Match<Track> {
        // A Spotify-sourced track already carries full Spotify metadata, so
        // searching Spotify again is redundant and, across a bulk Liked Songs
        // sync, the main cause of Web API rate limiting. Leave enrichment to
        // MusicBrainz/Bandcamp for these.
        if track
            .references
            .iter()
            .any(|r| r.platform == shared::models::Platform::Spotify)
        {
            return Match::None;
        }

        let query = format!(
            "{} {}",
            track.artists.first().map(|a| a.name.as_str()).unwrap_or(""),
            track.title
        );

        let candidates = self.search_tracks(&query).await;

        let result = candidates
            .into_iter()
            .map(|candidate| {
                let score = track.compare(&candidate);
                (score, candidate)
            })
            .filter(|(score, _)| *score > 0.0)
            .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(Match::None, |(best_score, best_track)| {
                if best_score > Self::EXACT_MATCH_THRESHOLD {
                    Match::Exact(best_track)
                } else if best_score > Self::PARTIAL_MATCH_THRESHOLD {
                    Match::Partial(best_track)
                } else {
                    Match::None
                }
            });

        // Enrich artist icons on the matched track.
        match result {
            Match::Exact(mut t) => {
                self.enrich_artist_icons(&mut t).await;
                Match::Exact(t)
            }
            Match::Partial(mut t) => {
                self.enrich_artist_icons(&mut t).await;
                Match::Partial(t)
            }
            Match::None => Match::None,
        }
    }

    async fn get_match_from_query(&self, query: &str) -> Match<Track> {
        let normalized_query = query.replace("- ", "");
        let tracks = self.search_tracks(query).await;

        tracks
            .iter()
            .map(|track| {
                let match_score = string_similarity(
                    &normalized_query,
                    &format!("{} {}", track.artists[0].name, track.title),
                    SimilarityAlgorithm::SorensenDice,
                );
                (match_score, track)
            })
            .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(Match::None, |(best_score, best_track)| {
                if best_score > Self::EXACT_MATCH_THRESHOLD {
                    Match::Exact(best_track.clone())
                } else if best_score > Self::PARTIAL_MATCH_THRESHOLD {
                    Match::Partial(best_track.clone())
                } else {
                    Match::None
                }
            })
    }

    async fn get_matches_from_query(&self, query: &str) -> Vec<Track> {
        self.search_tracks(query).await
    }
}
