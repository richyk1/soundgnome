pub mod session;
pub mod webapi;

use async_trait::async_trait;
use shared::{
    errors::Error,
    models::{Album, Artist, Playlist, PlaylistTrack, ReferenceType, Track},
    types::SoundgnomeResult,
};

use crate::Source;

/// Spotify catalogue source, authorized by the single user session obtained
/// from the librespot login (see [`session`] / `downloader::spotify::auth`).
/// There is no app client-credentials path: connecting Spotify once covers
/// audio, Liked Songs, and metadata.
pub struct Spotify;

impl Spotify {
    /// Spotify's own URL for the signed-in user's Liked Songs. Treating it as a
    /// playlist URL reuses the whole sync, task and schedule pipeline, exactly
    /// as the SoundCloud likes feed does.
    pub const LIKED_URL: &'static str = "https://open.spotify.com/collection/tracks";

    /// True for the Liked Songs pseudo-playlist, in the spellings a user might
    /// paste or a button might send.
    pub fn is_liked_url(url: &str) -> bool {
        let trimmed = url
            .split('?')
            .next()
            .unwrap_or(url)
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .to_lowercase();

        matches!(
            trimmed.as_str(),
            "open.spotify.com/collection/tracks" | "spotify:liked" | "spotify:collection:tracks"
        )
    }

    pub fn new() -> Self {
        Spotify
    }

    /// Extracts the id from a spotify url
    /// (e.g. https://open.spotify.com/track/xxxxxxx?si=yyyyyyy -> xxxxxxx).
    fn url_to_id(url: &str) -> SoundgnomeResult<String> {
        let id = url
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("");

        if id.is_empty() {
            Err(Error::InvalidUrl(url.to_string()))
        } else {
            Ok(id.to_string())
        }
    }
}

impl Default for Spotify {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for Spotify {
    async fn get_track_from_url(&self, url: &str) -> SoundgnomeResult<Track> {
        let id = Self::url_to_id(url)?;
        webapi::get_track(&id, ReferenceType::Source).await
    }

    async fn get_tracks_from_query(&self, query: &str) -> SoundgnomeResult<Vec<Track>> {
        webapi::search_tracks(query, 20, ReferenceType::Source).await
    }

    async fn get_playlist_from_url(&self, url: &str) -> SoundgnomeResult<Playlist> {
        let id = Self::url_to_id(url)?;
        webapi::get_playlist(&id, url).await
    }

    async fn get_playlist_tracks_from_url(
        &self,
        url: &str,
    ) -> SoundgnomeResult<Vec<PlaylistTrack>> {
        let id = Self::url_to_id(url)?;
        webapi::get_playlist_tracks(&id, ReferenceType::Source).await
    }

    async fn get_artist_from_url(&self, url: &str) -> SoundgnomeResult<Artist> {
        let id = Self::url_to_id(url)?;
        webapi::get_artist(&id).await
    }

    async fn get_artist_tracks_from_url(&self, url: &str) -> SoundgnomeResult<Vec<Track>> {
        let id = Self::url_to_id(url)?;
        webapi::get_artist_tracks(&id, ReferenceType::Source).await
    }

    async fn get_artists_from_query(&self, search: &str) -> SoundgnomeResult<Vec<Artist>> {
        webapi::search_artists(search, 20).await
    }

    async fn get_album_from_url(&self, url: &str) -> SoundgnomeResult<Album> {
        let id = Self::url_to_id(url)?;
        webapi::get_album(&id).await
    }

    async fn get_albums_from_query(&self, search: &str) -> SoundgnomeResult<Vec<Album>> {
        webapi::search_albums(search, 20).await
    }

    async fn get_album_tracks_from_url(&self, url: &str) -> SoundgnomeResult<Vec<Track>> {
        let id = Self::url_to_id(url)?;
        webapi::get_album_tracks(&id, ReferenceType::Source).await
    }

    async fn clean_track_metadata(&self, _track: &mut Track) -> SoundgnomeResult<()> {
        Ok(())
    }

    async fn clean_tracks_metadata(
        &self,
        _tracks: &mut Vec<&mut Track>,
        _on_batch: Option<&mut (dyn FnMut(usize, usize) + Send)>,
    ) -> SoundgnomeResult<()> {
        Ok(())
    }

    fn is_valid_track_url(url: &str) -> bool {
        url.contains("open.spotify.com/track/")
    }

    fn is_valid_playlist_url(url: &str) -> bool {
        Spotify::is_liked_url(url) || url.contains("open.spotify.com/playlist/")
    }

    fn is_valid_artist_url(url: &str) -> bool {
        url.contains("open.spotify.com/artist/")
    }

    fn is_valid_album_url(url: &str) -> bool {
        url.contains("open.spotify.com/album/")
    }
}
