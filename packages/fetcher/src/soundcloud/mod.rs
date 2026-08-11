pub mod auth;

mod mappers;

use ai::AIBackend;
use async_trait::async_trait;
use config::Config;
use fancy_regex::Regex;
use futures::future::join_all;
use mappers::convert_track;
use rsoundcloud::models::track::BasicTrack;
use rsoundcloud::{
    ClientError, CollectionParams, MeApi, PlaylistsApi, ResourceId, SearchApi, SoundCloudClient,
    TracksApi, UsersApi,
};
use shared::{
    errors::Error,
    http::HttpClientBuilder,
    models::{Album, Artist, Platform, Playlist, PlaylistTrack, SimplifiedTrack, Track},
    types::SoundomeResult,
};

use crate::Source;

/// Hard stop for likes pagination. At 50 entries per page this covers 50k
/// likes, well past any real account, and prevents an unbounded loop if
/// SoundCloud ever returns a self-referential `next_href`.
const MAX_LIKES_PAGES: u32 = 1000;

/// Pull the opaque pagination cursor out of a `next_href` returned by
/// api-v2. The value is percent-encoded in the URL and must be handed back
/// decoded, since the client re-encodes query parameters itself.
fn next_cursor(next_href: &str) -> Option<String> {
    reqwest::Url::parse(next_href)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == "offset")
        .map(|(_, value)| value.into_owned())
        .filter(|cursor| !cursor.is_empty())
}

pub struct Soundcloud {
    client: SoundCloudClient,
}

impl Soundcloud {
    /// Maximum number of tracks sent to the AI in a single curation request.
    /// Keeping this small helps the model maintain track boundaries and avoids
    /// "leaking" artist names across unrelated tracks in the same batch.
    const AI_CLEANUP_BATCH_SIZE: usize = 10;

    const TRACK_REGEX: &str = r"^https:\/\/soundcloud\.com\/(?:(?!sets|stats|groups|upload|you|mobile|stream|messages|discover|notifications|terms-of-use|people|pages|jobs|settings|logout|charts|imprint|popular)(?:[a-z0-9\-_]{1,25}))\/(?:(?:(?!sets|playlist|stats|settings|logout|notifications|you|messages)(?:[a-z0-9\-_]{1,100}))(?:\/s\-[a-zA-Z0-9\-_]{1,10})?)(?:\?.*)?$";
    const PLAYLIST_REGEX: &str = r"^https:\/\/soundcloud\.com\/(?:(?!sets|stats|groups|upload|you|mobile|stream|messages|discover|notifications|terms-of-use|people|pages|jobs|settings|logout|charts|imprint|popular)[a-z0-9\-_]{1,25})\/sets\/[a-z0-9\-_]{1,100}(?:\?.*)?$";
    const ARTIST_REGEX: &str = r"^https:\/\/soundcloud\.com\/(?:(?!sets|stats|groups|upload|you|mobile|stream|messages|discover|notifications|terms-of-use|people|pages|jobs|settings|logout|charts|imprint|popular)[a-z0-9\-_]{1,25})\/?(?:\?.*)?$";

    /// Strip query parameters and fragments from a SoundCloud URL for cleaner processing
    fn sanitize_url(url: &str) -> String {
        url.split('?').next().unwrap_or(url).to_string()
    }

    pub async fn new() -> SoundomeResult<Self> {
        // The stored session token is what makes private endpoints such as
        // "my likes" reachable; without it the client stays anonymous and those
        // return 401.
        let auth_token = auth::stored_token();
        if auth_token.is_some() {
            tracing::debug!("SoundCloud client using stored session token");
        }

        let client = match Config::get().proxy.as_ref() {
            Some(proxy_config) if proxy_config.enabled => {
                let reqwest_client = HttpClientBuilder::get_reqwest_client()?;
                let http_client = rsoundcloud::http::HttpClient::new(reqwest_client);
                SoundCloudClient::with_http_client(http_client, None, auth_token).await
            }
            _ => SoundCloudClient::new(None, auth_token).await,
        }
        .map_err(|e| match e {
            ClientError::ClientIDGenerationFailed => {
                Error::Internal("Failed to generate Soundcloud client id".to_string())
            }
            _ => Error::Internal("Failed to create Soundcloud client".to_string()),
        })?;

        Ok(Self { client })
    }

    /// SoundCloud's own URL for the signed-in user's likes. Treating it as a
    /// playlist URL means the whole existing sync pipeline (tasks, schedules,
    /// progress, M3U8 export) works on likes with no new plumbing.
    pub const LIKES_URL: &'static str = "https://soundcloud.com/you/likes";

    /// True for the likes pseudo-playlist, in the few spellings a user might
    /// paste (with or without scheme, `www.`, trailing slash, or query).
    pub fn is_likes_url(url: &str) -> bool {
        let trimmed = Self::sanitize_url(url)
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .to_lowercase();

        matches!(
            trimmed.as_str(),
            "soundcloud.com/you/likes" | "soundcloud.com/you/favorites"
        )
    }

    /// The liked tracks as domain models, without touching the database or
    /// downloading anything. Backs the read-only Likes view.
    pub async fn list_liked_tracks(&self) -> Result<Vec<Track>, Error> {
        Ok(self
            .get_all_liked_tracks()
            .await?
            .into_iter()
            .map(|basic_track| mappers::convert_basic_track(basic_track, None))
            .collect())
    }

    /// Resolve a playable audio URL for a track, for preview only.
    ///
    /// SoundCloud advertises several transcodings per track; the `progressive`
    /// MP3 is the only one a plain `<audio>` element can play (the rest are
    /// HLS). Its URL is not the stream itself but an indirection that returns a
    /// short-lived signed CDN link, so callers must expect it to expire.
    pub async fn resolve_stream_url(&self, track_id: u64) -> Result<String, Error> {
        let raw = self
            .client
            .api_get(
                &format!("/tracks/{}", track_id),
                std::collections::HashMap::new(),
            )
            .await
            .map_err(|e| Error::Network(format!("Failed to fetch track {}: {}", track_id, e)))?;

        let track: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| Error::Internal(format!("Failed to parse track response: {}", e)))?;

        let transcoding_url = track
            .pointer("/media/transcodings")
            .and_then(|t| t.as_array())
            .and_then(|transcodings| {
                transcodings.iter().find(|t| {
                    t.pointer("/format/protocol").and_then(|p| p.as_str()) == Some("progressive")
                })
            })
            .and_then(|t| t.get("url"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                // Usually means the track is preview-only, geo-blocked, or
                // DRM-protected: it has HLS transcodings but no progressive one.
                Error::NotFound(format!("playable stream for SoundCloud track {}", track_id))
            })?;

        // The advertised transcoding URL is absolute, but the client prepends
        // its own API base and appends the `client_id` the media endpoint
        // requires, so hand it just the path.
        let path = reqwest::Url::parse(transcoding_url)
            .map_err(|e| Error::Internal(format!("Unusable transcoding URL: {}", e)))?
            .path()
            .to_string();

        let raw = self
            .client
            .api_get(&path, std::collections::HashMap::new())
            .await
            .map_err(|e| Error::Network(format!("Failed to resolve stream URL: {}", e)))?;

        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(str::to_string))
            .ok_or_else(|| Error::NotFound(format!("signed stream URL for track {}", track_id)))
    }

    /// Every track the signed-in user has liked, newest first.
    ///
    /// Liked *playlists* also appear in this feed and are skipped: expanding
    /// them would silently pull in hundreds of tracks the user never liked
    /// individually.
    async fn get_all_liked_tracks(&self) -> Result<Vec<BasicTrack>, Error> {
        if auth::stored_token().is_none() {
            return Err(Error::Custom(
                "SoundCloud is not connected. Add your session token in Tools, then Providers."
                    .to_string(),
            ));
        }

        let me = self.client.get_me().await.map_err(|e| {
            Error::Custom(format!(
                "Could not identify the connected SoundCloud account: {}",
                e
            ))
        })?;
        let user_id = me.user.id;

        let limit = 50u32;
        // The likes feed is cursor paginated: `offset` is an opaque token
        // handed back in `next_href`, not a row count. Numeric offsets are
        // rejected outright with a 400.
        let mut cursor: Option<String> = None;
        let mut page = 0u32;
        let mut liked: Vec<BasicTrack> = Vec::new();
        let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut skipped_playlists = 0usize;

        loop {
            let uri = format!("/users/{}/likes", user_id);
            let mut query = std::collections::HashMap::new();
            query.insert("limit".to_string(), limit.to_string());
            query.insert("linked_partitioning".to_string(), "1".to_string());
            if let Some(cursor) = cursor.as_deref() {
                query.insert("offset".to_string(), cursor.to_string());
            }

            let result = self.client.api_get(&uri, query).await.map_err(|e| {
                Error::Network(format!("Failed to fetch likes page {}: {}", page, e))
            })?;

            let json: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| Error::Internal(format!("Failed to parse likes response: {}", e)))?;

            let items = match json.get("collection").and_then(|c| c.as_array()) {
                Some(items) if !items.is_empty() => items.clone(),
                _ => break,
            };
            // Each entry is `{created_at, kind, track | playlist}`.
            for item in items {
                let Some(track_value) = item.get("track") else {
                    skipped_playlists += 1;
                    continue;
                };
                match serde_json::from_value::<BasicTrack>(track_value.clone()) {
                    Ok(track) => {
                        if seen_ids.insert(track.track.id) {
                            liked.push(track);
                        }
                    }
                    // One unusual entry must not abort a 600-track sync.
                    Err(e) => tracing::warn!("Skipping unreadable liked track: {}", e),
                }
            }

            // A short page does NOT mean the end: SoundCloud filters deleted and
            // private entries server side after slicing, so pages routinely come
            // back under `limit` with more still to come. `next_href` is the only
            // reliable terminator.
            match json
                .get("next_href")
                .and_then(|v| v.as_str())
                .and_then(next_cursor)
            {
                Some(next) => cursor = Some(next),
                None => break,
            }

            page += 1;
            if page > MAX_LIKES_PAGES {
                tracing::warn!(
                    "Stopping likes pagination after {} pages, {} tracks so far",
                    page,
                    liked.len()
                );
                break;
            }
        }

        tracing::info!(
            "Fetched {} liked tracks ({} liked playlists skipped)",
            liked.len(),
            skipped_playlists
        );

        Ok(liked)
    }

    // =================
    // Utils
    // =================

    /// Fetch all tracks for a user with pagination (the default API only returns one page).
    /// Also fetches tracks from the user's albums since those are not included in `/tracks`.
    async fn get_all_user_tracks(&self, url: &str) -> Result<Vec<BasicTrack>, Error> {
        // Resolve user to get their ID
        let user = self
            .client
            .get_user(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("Soundcloud artist from {}", url)))?;

        let user_id = user.user.id;
        let mut all_tracks: Vec<BasicTrack> = Vec::new();
        let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

        // 1. Fetch direct uploads (singles) with pagination
        let limit = 50u32;
        let mut offset = 0u32;

        loop {
            let uri = format!("/users/{}/tracks", user_id);
            let mut query = std::collections::HashMap::new();
            query.insert("limit".to_string(), limit.to_string());
            query.insert("offset".to_string(), offset.to_string());
            query.insert("linked_partitioning".to_string(), "1".to_string());

            let result = self.client.api_get(&uri, query).await.map_err(|e| {
                Error::Network(format!(
                    "Failed to fetch user tracks page at offset {}: {}",
                    offset, e
                ))
            })?;

            let json: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| Error::Internal(format!("Failed to parse tracks response: {}", e)))?;

            let collection = json.get("collection").and_then(|c| c.as_array());
            let page_tracks: Vec<BasicTrack> = match collection {
                Some(items) if !items.is_empty() => serde_json::from_value(
                    serde_json::Value::Array(items.clone()),
                )
                .map_err(|e| Error::Internal(format!("Failed to deserialize tracks: {}", e)))?,
                _ => break,
            };

            let page_len = page_tracks.len();
            for track in page_tracks {
                if seen_ids.insert(track.track.id) {
                    all_tracks.push(track);
                }
            }

            if page_len < limit as usize {
                break;
            }

            let has_next = json.get("next_href").and_then(|v| v.as_str()).is_some();
            if !has_next {
                break;
            }

            offset += limit;
        }

        tracing::info!(
            "Fetched {} direct tracks for SoundCloud user",
            all_tracks.len()
        );

        // 2. Fetch tracks from the user's albums (these are not included in /tracks)
        let mut album_offset = 0u32;

        loop {
            let uri = format!("/users/{}/albums", user_id);
            let mut query = std::collections::HashMap::new();
            query.insert("limit".to_string(), limit.to_string());
            query.insert("offset".to_string(), album_offset.to_string());
            query.insert("linked_partitioning".to_string(), "1".to_string());

            let result = self.client.api_get(&uri, query).await.map_err(|e| {
                Error::Network(format!(
                    "Failed to fetch user albums at offset {}: {}",
                    album_offset, e
                ))
            })?;

            let json: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| Error::Internal(format!("Failed to parse albums response: {}", e)))?;

            let collection = json.get("collection").and_then(|c| c.as_array());
            let albums = match collection {
                Some(items) if !items.is_empty() => items.clone(),
                _ => break,
            };

            let page_len = albums.len();

            for album_value in &albums {
                // Each album has a "tracks" array with track objects
                let tracks_arr = album_value.get("tracks").and_then(|t| t.as_array());
                if let Some(tracks) = tracks_arr {
                    for track_value in tracks {
                        // Album tracks can be BasicTrack or MiniTrack (incomplete).
                        // Only include those with full info (have "title" and "permalink_url").
                        if track_value.get("title").is_some() && track_value.get("media").is_some()
                        {
                            if let Ok(track) =
                                serde_json::from_value::<BasicTrack>(track_value.clone())
                            {
                                if seen_ids.insert(track.track.id) {
                                    all_tracks.push(track);
                                }
                            }
                        }
                    }
                }
            }

            if page_len < limit as usize {
                break;
            }

            let has_next = json.get("next_href").and_then(|v| v.as_str()).is_some();
            if !has_next {
                break;
            }

            album_offset += limit;
        }

        tracing::info!(
            "Fetched {} total tracks for SoundCloud user (including albums)",
            all_tracks.len()
        );
        Ok(all_tracks)
    }

    async fn get_complete_track_from_music_track(
        &self,
        track: rsoundcloud::models::track::Track,
    ) -> Track {
        let album = self
            .client
            .get_track_albums(ResourceId::Id(track.track.id))
            .await
            .ok()
            .and_then(|albums| albums.into_iter().find(|a| a.album_playlist.is_album));
        convert_track(track, album)
    }

    pub async fn clean_tracks_title_and_artist_name(
        &self,
        tracks: &mut [&mut Track],
        mut on_batch: Option<&mut (dyn FnMut(usize, usize) + Send)>,
    ) -> SoundomeResult<()> {
        // AI cleanup is an enhancement, never a requirement: an unconfigured or
        // disabled backend must leave the raw SoundCloud metadata in place
        // rather than failing the download. Only backend availability is
        // tolerated here; a configured backend that errors mid-run still
        // propagates, since that is a real failure the user should see.
        let ai_client = match ai::AIClient::new() {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!(
                    "Skipping AI metadata cleanup for {} track(s): {}",
                    tracks.len(),
                    e
                );
                return Ok(());
            }
        };

        let prompt = ai::prompts::clean_track_title_and_artist_name(false)?;

        // Process in small chunks to avoid token limit issues, reduce timeout risk, and
        // prevent the AI from confusing/leaking artist names across unrelated tracks.
        let chunk_size = Self::AI_CLEANUP_BATCH_SIZE;
        let mut i = 0;

        while i < tracks.len() {
            let end = usize::min(i + chunk_size, tracks.len());
            let chunk = &mut tracks[i..end];

            let simplified_tracks: Vec<SimplifiedTrack> = chunk
                .iter()
                .map(|track| SimplifiedTrack {
                    id: track
                        .get_source()
                        .and_then(|track_ref| track_ref.external_id)
                        .unwrap_or_default(),
                    title: track.title.clone(),
                    artists: track.artists.iter().map(|a| a.name.clone()).collect(),
                })
                .collect();

            // Send to AI for processing
            tracing::info!(
                "Sending {} tracks to AI for processing",
                simplified_tracks.len()
            );
            let processed_tracks = ai_client
                .generate_with_data(&prompt, simplified_tracks.clone())
                .await
                .map_err(|e| Error::Internal(format!("AI processing failed: {}", e)))?;

            tracing::info!("Processed tracks: {:#?}", processed_tracks);

            // Index AI output by `id` so we're immune to reordering, drops, or
            // duplicates in the response. Anything the AI didn't return keeps
            // its original metadata (safer than a positional mis-assignment).
            let mut by_id: std::collections::HashMap<String, &SimplifiedTrack> =
                std::collections::HashMap::with_capacity(processed_tracks.len());
            for processed in &processed_tracks {
                by_id.insert(processed.id.clone(), processed);
            }

            for (idx, input) in simplified_tracks.iter().enumerate() {
                let processed = match by_id.get(&input.id) {
                    Some(p) => *p,
                    None => {
                        tracing::warn!(
                            "AI curation dropped track id={} (title={:?}); keeping original metadata",
                            input.id,
                            input.title
                        );
                        continue;
                    }
                };

                // Validate every proposed artist name is actually present in the
                // input title or input artists. This is the hard guardrail against
                // cross-track leakage (e.g. "ZadernaS" quietly becoming "Mylacid").
                let validated_artists: Vec<String> = processed
                    .artists
                    .iter()
                    .filter(|name| Self::artist_name_is_supported(name, input))
                    .cloned()
                    .collect();

                let final_artists = if validated_artists.is_empty() {
                    tracing::warn!(
                        "AI curation for track id={} produced no valid artist name (proposed={:?}); falling back to original artists",
                        input.id,
                        processed.artists
                    );
                    input.artists.clone()
                } else {
                    if validated_artists.len() != processed.artists.len() {
                        let rejected: Vec<&String> = processed
                            .artists
                            .iter()
                            .filter(|name| !validated_artists.contains(name))
                            .collect();
                        tracing::warn!(
                            "AI curation for track id={} proposed unsupported artist name(s) {:?}; dropping them",
                            input.id,
                            rejected
                        );
                    }
                    validated_artists
                };

                chunk[idx].title = processed.title.clone();
                chunk[idx].artists = final_artists
                    .iter()
                    .enumerate()
                    .map(|(j, name)| Artist {
                        id: None,
                        name: name.clone(),
                        icon: chunk[idx]
                            .artists
                            .get(j)
                            .and_then(|artist| artist.icon.clone()),
                        references: chunk[idx]
                            .artists
                            .get(j)
                            .map(|artist| artist.references.clone())
                            .unwrap_or_default(),
                    })
                    .collect();
            }

            i += chunk_size;

            // Report progress after each batch so callers can surface live curation
            // status (e.g. "processed X / Y tracks") to the user.
            if let Some(cb) = on_batch.as_mut() {
                cb(end, tracks.len());
            }
        }

        Ok(())
    }

    /// Returns true when `name` appears (after normalization) as a substring
    /// of either the input title or one of the input artists for the same track.
    /// This is a hard guardrail: the AI is only allowed to keep or split names
    /// that were already there, never to invent or borrow from another track.
    fn artist_name_is_supported(name: &str, input: &SimplifiedTrack) -> bool {
        // Normalize by removing spaces, underscores, and dashes to allow flexible matching
        // (e.g. "Habits Sales" == "Habits_Sales" == "Habits-Sales").
        let normalize_for_comparison = |s: &str| -> String {
            shared::utils::string::normalize_string(s).replace([' ', '_', '-'], "")
        };

        let normalized_name = normalize_for_comparison(name);
        // A completely empty normalization (e.g. an emoji-only name) can't be
        // usefully validated — reject it defensively.
        if normalized_name.is_empty() {
            return false;
        }

        let normalized_title = normalize_for_comparison(&input.title);
        if normalized_title.contains(&normalized_name) {
            return true;
        }

        input
            .artists
            .iter()
            .any(|a| normalize_for_comparison(a).contains(&normalized_name))
    }
}

#[async_trait]
impl Source for Soundcloud {
    async fn get_track_from_url(&self, url: &str) -> SoundomeResult<Track> {
        tracing::info!("Getting SoundCloud track from URL: {}", url);
        let track = self
            .client
            .get_track(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("Soundcloud track from {}", url).to_string()))?;

        Ok(self.get_complete_track_from_music_track(track).await)
    }

    async fn get_tracks_from_query(&self, query: &str) -> Result<Vec<Track>, Error> {
        let tracks = self
            .client
            .search_tracks(query.to_string(), CollectionParams::new(Some(10), None))
            .await
            .map_err(mappers::convert_error)?;

        Ok(join_all(
            tracks
                .iter()
                .map(|track| self.get_complete_track_from_music_track(track.clone())),
        )
        .await)
    }

    async fn get_playlist_from_url(&self, url: &str) -> SoundomeResult<Playlist> {
        if Self::is_likes_url(url) {
            return Ok(Playlist {
                id: None,
                name: "SoundCloud Likes".to_string(),
                source: Platform::SoundCloud,
                source_url: Some(Self::LIKES_URL.to_string()),
                cover: None,
            });
        }

        let playlist = self
            .client
            .get_playlist(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| {
                Error::NotFound(format!("SoundCloud playlist from {}", url).to_string())
            })?;

        let cover = playlist.album_playlist.artwork_url.clone();
        Ok(Playlist {
            id: None,
            name: playlist.album_playlist.title.clone(),
            source: Platform::SoundCloud,
            source_url: Some(url.to_string()),
            cover,
        })
    }

    async fn get_playlist_tracks_from_url(&self, url: &str) -> Result<Vec<PlaylistTrack>, Error> {
        if Self::is_likes_url(url) {
            let liked = self.get_all_liked_tracks().await?;
            return Ok(liked
                .into_iter()
                .enumerate()
                .map(|(i, basic_track)| {
                    // Both flags matter: an uploader can enable downloads and
                    // still have run out of the quota SoundCloud enforces.
                    let original_available = Some(
                        basic_track.track.downloadable && basic_track.track.has_downloads_left,
                    );
                    PlaylistTrack {
                        id: None,
                        track: mappers::convert_basic_track(basic_track, None),
                        added_at: None,
                        position: Some(i as u32),
                        original_available,
                    }
                })
                .collect());
        }

        let tracks = self
            .client
            .get_playlist_tracks(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("SoundCloud playlist tracks from {}", url)))?;

        Ok(join_all(
            tracks
                .into_iter()
                .map(|track| self.get_complete_track_from_music_track(track)),
        )
        .await
        .into_iter()
        .enumerate()
        .map(|(i, track)| PlaylistTrack {
            id: None,
            track,
            added_at: None,
            position: Some(i as u32),
            // The playlist endpoint returns fully hydrated tracks, but the
            // conversion above drops the raw flags, so treat it as unknown and
            // let the caller probe if it cares.
            original_available: None,
        })
        .collect())
    }

    async fn get_artist_from_url(&self, url: &str) -> Result<Artist, Error> {
        let artist = self
            .client
            .get_user(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("Soundcloud artist from {}", url).to_string()))?;
        Ok(mappers::convert_artist(&artist))
    }

    async fn get_artist_tracks_from_url(&self, url: &str) -> Result<Vec<Track>, Error> {
        let tracks = self.get_all_user_tracks(url).await?;

        Ok(tracks
            .into_iter()
            .map(|basic_track| mappers::convert_basic_track(basic_track, None))
            .collect())
    }

    async fn get_artists_from_query(&self, search: &str) -> Result<Vec<Artist>, Error> {
        let users = self
            .client
            .search_users(search.to_string(), CollectionParams::default())
            .await
            .map_err(mappers::convert_error)?;

        Ok(users.iter().map(mappers::convert_artist).collect())
    }

    async fn get_album_from_url(&self, url: &str) -> Result<Album, Error> {
        let album = self
            .client
            .get_playlist(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("Soundcloud album from {}", url).to_string()))?;
        Ok(mappers::convert_basic_album(&album))
    }

    async fn get_albums_from_query(&self, search: &str) -> Result<Vec<Album>, Error> {
        let albums = self
            .client
            .search_albums(search.to_string(), CollectionParams::default())
            .await
            .map_err(mappers::convert_error)?;

        Ok(albums.iter().map(mappers::convert_album).collect())
    }

    async fn get_album_tracks_from_url(&self, url: &str) -> Result<Vec<Track>, Error> {
        // SoundCloud albums are technically playlists, reuse playlist track fetching
        let tracks = self
            .client
            .get_playlist_tracks(ResourceId::Url(url.to_string()))
            .await
            .map_err(|_| Error::NotFound(format!("SoundCloud album tracks from {}", url)))?;

        Ok(join_all(
            tracks
                .into_iter()
                .map(|track| self.get_complete_track_from_music_track(track)),
        )
        .await)
    }

    async fn clean_track_metadata(&self, track: &mut Track) -> SoundomeResult<()> {
        let mut tracks = vec![track];
        self.clean_tracks_metadata(&mut tracks, None).await
    }

    async fn clean_tracks_metadata(
        &self,
        tracks: &mut Vec<&mut Track>,
        on_batch: Option<&mut (dyn FnMut(usize, usize) + Send)>,
    ) -> SoundomeResult<()> {
        self.clean_tracks_title_and_artist_name(tracks, on_batch)
            .await
    }

    fn is_valid_track_url(url: &str) -> bool {
        let sanitized = Self::sanitize_url(url);
        let re = Regex::new(Self::TRACK_REGEX).unwrap(); // safe unwrap
        re.is_match(&sanitized).unwrap_or(false)
    }

    fn is_valid_playlist_url(url: &str) -> bool {
        if Self::is_likes_url(url) {
            return true;
        }
        let sanitized = Self::sanitize_url(url);
        let re = Regex::new(Self::PLAYLIST_REGEX).unwrap(); // safe unwrap
        re.is_match(&sanitized).unwrap_or(false)
    }

    fn is_valid_artist_url(url: &str) -> bool {
        let sanitized = Self::sanitize_url(url);
        // Artist URL must not match track or playlist patterns
        if Self::is_valid_track_url(&sanitized) || Self::is_valid_playlist_url(&sanitized) {
            return false;
        }
        let re = Regex::new(Self::ARTIST_REGEX).unwrap(); // safe unwrap
        re.is_match(&sanitized).unwrap_or(false)
    }

    fn is_valid_album_url(_url: &str) -> bool {
        // SoundCloud albums use the same /sets/ URL pattern as playlists,
        // so album URLs are handled through the playlist path.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_playlist_with_utm_params() {
        let url = "https://soundcloud.com/barthohm/sets/euphoria-part-5?si=e466651555934986ae7e0846301c5894&utm_source=clipboard&utm_medium=text&utm_campaign=social_sharing";
        assert!(
            Soundcloud::is_valid_playlist_url(url),
            "Should accept playlist URL with UTM params"
        );
    }

    #[test]
    fn test_valid_playlist_without_params() {
        let url = "https://soundcloud.com/barthohm/sets/euphoria-part-5";
        assert!(
            Soundcloud::is_valid_playlist_url(url),
            "Should accept playlist URL without params"
        );
    }

    #[test]
    fn test_valid_track_with_utm_params() {
        let url = "https://soundcloud.com/artist/track-name?si=12345&utm_source=clipboard";
        assert!(
            Soundcloud::is_valid_track_url(url),
            "Should accept track URL with UTM params"
        );
    }

    #[test]
    fn test_valid_artist_url() {
        let url = "https://soundcloud.com/barthohm";
        assert!(
            Soundcloud::is_valid_artist_url(url),
            "Should accept artist URL"
        );
    }

    #[test]
    fn test_valid_artist_url_with_trailing_slash_and_params() {
        let url = "https://soundcloud.com/barthohm/?param=value";
        assert!(
            Soundcloud::is_valid_artist_url(url),
            "Should accept artist URL with trailing slash and params"
        );
    }

    fn simplified(title: &str, artists: &[&str]) -> shared::models::SimplifiedTrack {
        shared::models::SimplifiedTrack {
            id: "id1".to_string(),
            title: title.to_string(),
            artists: artists.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn artist_name_supported_when_present_in_title() {
        let input = simplified("GRÄV - Habits Sales & VYRAX", &["Habits_Sales"]);
        // Both "Habits Sales" and "VYRAX" come from the title and must be accepted.
        assert!(Soundcloud::artist_name_is_supported("Habits Sales", &input));
        assert!(Soundcloud::artist_name_is_supported("VYRAX", &input));
    }

    #[test]
    fn artist_name_supported_when_present_in_uploader() {
        let input = simplified("Some Title", &["Habits_Sales"]);
        // Normalization strips the underscore, so the AI-cleaned "Habits Sales"
        // still resolves to the uploader username.
        assert!(Soundcloud::artist_name_is_supported("Habits Sales", &input));
    }

    #[test]
    fn artist_name_rejected_when_not_present_anywhere() {
        let input = simplified("Zorven - Some Track feat. ZadernaS", &["Zorven"]);
        // "Mylacid" is a name from a different track in the same batch — reject.
        assert!(!Soundcloud::artist_name_is_supported("Mylacid", &input));
    }

    #[test]
    fn artist_name_rejected_for_empty_normalization() {
        let input = simplified("Zorven - Some Track", &["Zorven"]);
        // Emoji-only or whitespace-only names cannot be validated.
        assert!(!Soundcloud::artist_name_is_supported("🎵", &input));
        assert!(!Soundcloud::artist_name_is_supported("   ", &input));
    }
}
