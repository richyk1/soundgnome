use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub logs: LogsConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub ai: AiConfig,
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub tagger: TaggerConfig,
    #[serde(default)]
    pub downloader: DownloaderConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub playlists: PlaylistsConfig,
}

impl Config {
    /// Where the SoundCloud `oauth_token` cookie submitted through the UI is
    /// stored. Kept next to the database so it survives restarts and lands on
    /// the same mounted volume in Docker.
    pub fn soundcloud_cookies_path(&self) -> PathBuf {
        Path::new(&self.database.url)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("soundcloud_cookies.txt")
    }

    /// The file to hand yt-dlp via `--cookies`, if any.
    ///
    /// An explicit `downloader.cookies_file` always wins: an operator who
    /// mounted a full browser cookie jar should not have it silently replaced
    /// by a token pasted into the UI.
    pub fn resolved_cookies_file(&self) -> Option<PathBuf> {
        if let Some(explicit) = self.downloader.cookies_file.as_deref() {
            return Some(PathBuf::from(explicit));
        }
        let stored = self.soundcloud_cookies_path();
        stored.is_file().then_some(stored)
    }

    /// Where Spotify app credentials submitted through the UI are stored.
    /// Kept beside the SoundCloud session for the same reasons.
    pub fn spotify_credentials_path(&self) -> PathBuf {
        Path::new(&self.database.url)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("spotify_credentials.json")
    }

    /// Directory holding librespot's reusable Spotify credentials blob
    /// (`credentials.json`). Kept beside the database for the same reasons as
    /// the other stored credentials: it survives restarts and lands on the same
    /// mounted volume in Docker.
    pub fn librespot_cache_dir(&self) -> PathBuf {
        Path::new(&self.database.url)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("spotify_librespot")
    }

    /// Where Last.fm API credentials (key + shared secret) submitted through the
    /// UI are stored. Kept beside the other stored credentials.
    pub fn lastfm_credentials_path(&self) -> PathBuf {
        Path::new(&self.database.url)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("lastfm_credentials.json")
    }

    /// Where the connected Last.fm user session (session key + username) is
    /// stored. The session key does not expire, so this persists the login.
    pub fn lastfm_session_path(&self) -> PathBuf {
        Path::new(&self.database.url)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("lastfm_session.json")
    }
}

// ===============================================================================
// General
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct GeneralConfig {
    #[serde(default = "GeneralConfig::default_base_library_dir")]
    pub base_library_dir: String,
    #[serde(default = "GeneralConfig::default_temp_download_dir")]
    pub temp_download_dir: String,
    /// Directory watched for local audio files to ingest.
    /// Files submitted via `POST /api/library/ingest` without an explicit path
    /// are resolved relative to this directory.
    /// Defaults to `./ingest`.
    /// ENV: SOUNDGNOME__GENERAL__INGEST_DIR
    #[serde(default = "GeneralConfig::default_ingest_dir")]
    pub ingest_dir: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            base_library_dir: Self::default_base_library_dir(),
            temp_download_dir: Self::default_temp_download_dir(),
            ingest_dir: Self::default_ingest_dir(),
        }
    }
}

impl GeneralConfig {
    fn default_base_library_dir() -> String {
        "./library".to_string()
    }
    fn default_temp_download_dir() -> String {
        "./temp".to_string()
    }
    fn default_ingest_dir() -> String {
        "./ingest".to_string()
    }
}

// ===============================================================================
// Logs
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct LogsConfig {
    #[serde(default = "LogsConfig::default_level")]
    pub level: String,
    #[serde(default)]
    pub enable_reqwest_logging: bool,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            level: Self::default_level(),
            enable_reqwest_logging: false,
        }
    }
}

impl LogsConfig {
    fn default_level() -> String {
        "info".to_string()
    }
}

// ===============================================================================
// Database
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct DatabaseConfig {
    #[serde(default = "DatabaseConfig::default_url")]
    pub url: String,
    pub pool_size: Option<u32>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: Self::default_url(),
            pool_size: None,
        }
    }
}

impl DatabaseConfig {
    fn default_url() -> String {
        "./data/soundgnome.db".to_string()
    }
}

// ===============================================================================
// AI
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Ordered list of AI provider names to try. The first available provider is used;
    /// if it fails, the next one is attempted. Supported values: "ollama", "openrouter".
    #[serde(default = "AiConfig::default_provider_order")]
    pub provider_order: Vec<String>,
    pub openrouter: Option<OpenRouterConfig>,
    pub ollama: Option<OllamaConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_order: Self::default_provider_order(),
            openrouter: None,
            ollama: None,
        }
    }
}

impl AiConfig {
    fn default_provider_order() -> Vec<String> {
        vec!["openrouter".to_string()]
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct OllamaConfig {
    /// Base URL of the Ollama instance, e.g. "http://192.168.1.10". Default: "http://localhost".
    pub host: Option<String>,
    /// Port of the Ollama instance. Default: 11434.
    pub port: Option<u16>,
    /// Model identifier, e.g. "llama3.2" or "qwen2.5:7b".
    pub model: Option<String>,
    /// HTTP request timeout in seconds.
    pub timeout: Option<u64>,
}

// ===============================================================================
// Tagger
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct TaggerConfig {
    /// List of enabled metadata provider names, in priority order.
    /// Supported values: "musicbrainz", "bandcamp", "spotify"
    #[serde(default = "TaggerConfig::default_providers")]
    pub metadata_providers: Vec<String>,

    /// Provider order used specifically for local-file ingest.
    /// Defaults to `["spotify", "musicbrainz", "bandcamp"]` so that Spotify's
    /// richer metadata (cover art, ISRC, track_number) takes priority over
    /// MusicBrainz when ingesting files from disk.
    /// ENV: SOUNDGNOME__TAGGER__INGEST_METADATA_PROVIDERS
    #[serde(default = "TaggerConfig::default_ingest_providers")]
    pub ingest_metadata_providers: Vec<String>,
}

impl Default for TaggerConfig {
    fn default() -> Self {
        Self {
            metadata_providers: Self::default_providers(),
            ingest_metadata_providers: Self::default_ingest_providers(),
        }
    }
}

impl TaggerConfig {
    fn default_providers() -> Vec<String> {
        vec![
            "musicbrainz".to_string(),
            "bandcamp".to_string(),
            "spotify".to_string(),
        ]
    }

    fn default_ingest_providers() -> Vec<String> {
        vec![
            "spotify".to_string(),
            "musicbrainz".to_string(),
            "bandcamp".to_string(),
        ]
    }
}

// ===============================================================================
// Downloader
// ===============================================================================

/// Audio download quality/format options passed to yt-dlp.
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct DownloaderConfig {
    /// Output audio format. `"best"` (default) keeps the best *taggable* source
    /// audio without re-encoding: SoundCloud downloadable originals (usually
    /// FLAC) when credentials are provided, otherwise the native AAC/MP3 stream.
    /// Any other value forces a transcode to that codec via yt-dlp
    /// `--audio-format`. Only tagger-writable codecs are supported: `"mp3"`,
    /// `"flac"`, `"m4a"` (aac). Untaggable targets (opus, wav, ...) would fail
    /// finalization, so avoid them.
    /// ENV: SOUNDGNOME__DOWNLOADER__AUDIO_FORMAT
    #[serde(default = "DownloaderConfig::default_audio_format")]
    pub audio_format: String,

    /// yt-dlp `--audio-quality` value (`"0"` = best VBR ... `"9"` = worst).
    /// Only applied when `audio_format` forces a transcode.
    /// ENV: SOUNDGNOME__DOWNLOADER__AUDIO_QUALITY
    #[serde(default = "DownloaderConfig::default_audio_quality")]
    pub audio_quality: String,

    /// Prefer a SoundCloud uploader's downloadable original file (often FLAC)
    /// over the streamed transcodes. Requires `cookies_file` — SoundCloud only
    /// exposes originals to authenticated clients.
    /// ENV: SOUNDGNOME__DOWNLOADER__PREFER_ORIGINAL
    #[serde(default = "DownloaderConfig::default_prefer_original")]
    pub prefer_original: bool,

    /// Re-download a track already in the library when the source can supply
    /// better audio, for example a lossless original that was not offered (or
    /// not reachable) the first time. Costs one metadata request per already
    /// owned track during a sync.
    /// ENV: SOUNDGNOME__DOWNLOADER__UPGRADE_EXISTING
    #[serde(default = "DownloaderConfig::default_upgrade_existing")]
    pub upgrade_existing: bool,

    /// How much better a lossy source must be before it replaces a lossy file,
    /// as a ratio of the stored bitrate. Lossless always wins regardless.
    /// Keeps a 161 vs 160 kbps difference from causing pointless churn.
    /// ENV: SOUNDGNOME__DOWNLOADER__UPGRADE_BITRATE_MARGIN
    #[serde(default = "DownloaderConfig::default_upgrade_bitrate_margin")]
    pub upgrade_bitrate_margin: f32,

    /// Match Spotify tracks on YouTube when Spotify audio is not connected.
    /// Off by default: a YouTube match is a different recording at a different
    /// quality, so substituting one silently is worse than failing.
    /// ENV: SOUNDGNOME__DOWNLOADER__ALLOW_YOUTUBE_FOR_SPOTIFY
    #[serde(default)]
    pub allow_youtube_for_spotify: bool,

    /// Require the 256k AAC master (itag 141) for YouTube downloads. When `true`,
    /// a track with no 256k master (or where it is unavailable) errors instead of
    /// downloading a lower tier — for Premium users who refuse anything below 256k.
    /// When `false` (default), 256k is still preferred but the download falls back
    /// to the next best taggable audio (e.g. 130k AAC) rather than failing.
    /// ENV: SOUNDGNOME__DOWNLOADER__YOUTUBE_REQUIRE_256K
    #[serde(default)]
    pub youtube_require_256k: bool,

    /// Path to a Netscape-format cookies file passed to yt-dlp `--cookies`.
    /// Enables SoundCloud original (FLAC) downloads and age/region-gated content.
    /// ENV: SOUNDGNOME__DOWNLOADER__COOKIES_FILE
    pub cookies_file: Option<String>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            audio_format: Self::default_audio_format(),
            audio_quality: Self::default_audio_quality(),
            prefer_original: Self::default_prefer_original(),
            upgrade_existing: Self::default_upgrade_existing(),
            upgrade_bitrate_margin: Self::default_upgrade_bitrate_margin(),
            allow_youtube_for_spotify: false,
            youtube_require_256k: false,
            cookies_file: None,
        }
    }
}

impl DownloaderConfig {
    fn default_audio_format() -> String {
        "best".to_string()
    }
    fn default_audio_quality() -> String {
        "0".to_string()
    }
    fn default_prefer_original() -> bool {
        true
    }
    fn default_upgrade_existing() -> bool {
        true
    }
    /// 15% better, so a re-encode at a nominally similar bitrate does not
    /// trigger a pointless replace.
    fn default_upgrade_bitrate_margin() -> f32 {
        1.15
    }

    /// True when the source codec is kept as-is (no re-encode).
    pub fn is_native(&self) -> bool {
        let f = self.audio_format.trim();
        f.is_empty() || f.eq_ignore_ascii_case("best")
    }

    /// The yt-dlp `-f` format selector.
    ///
    /// Native mode keeps the source codec, but only for containers that end up
    /// taggable. FLAC, M4A and MP3 are taggable as they are. WAV and AIFF are
    /// not, yet they are the most common originals uploaders offer, so they are
    /// selected here and losslessly repacked into FLAC after download (see
    /// `downloader::utils::ytdlp`). Raw Opus/WebM stays excluded: repacking it
    /// would either lose the audio or produce another untaggable file.
    ///
    /// If a source exposes nothing usable the download fails cleanly (yt-dlp:
    /// "requested format is not available"); set `audio_format` to a codec to
    /// force a transcode in that case.
    pub fn format_selector(&self) -> String {
        if self.is_native() {
            let mut parts: Vec<&str> = Vec::new();
            if self.prefer_original {
                parts.extend([
                    "download[ext=flac]",
                    "download[ext=wav]",
                    "download[ext=aiff]",
                    "download[ext=aif]",
                    "download[ext=m4a]",
                    "download[ext=mp3]",
                ]);
            }
            parts.extend(["bestaudio[ext=m4a]", "bestaudio[ext=mp3]"]);
            parts.join("/")
        } else if self.prefer_original {
            "download/bestaudio/best".to_string()
        } else {
            "bestaudio/best".to_string()
        }
    }

    /// `Some((codec, quality))` when a transcode is requested, otherwise `None`.
    pub fn transcode_target(&self) -> Option<(&str, &str)> {
        if self.is_native() {
            None
        } else {
            Some((self.audio_format.trim(), self.audio_quality.trim()))
        }
    }
}

// ===============================================================================
// Proxy
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub urls: Vec<String>, // List of proxy URLs with embedded credentials if needed
    pub strategy: Option<ProxyStrategy>, // Proxy rotation strategy
    pub no_proxy: Option<Vec<String>>, // List of domains to exclude from proxy
}

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub enum ProxyStrategy {
    #[serde(rename = "round_robin")]
    RoundRobin,
    #[serde(rename = "random")]
    Random,
    #[serde(rename = "sticky_per_hour")]
    StickyPerHour,
    #[serde(rename = "first_available")]
    FirstAvailable,
}

// ===============================================================================
// Server
// ===============================================================================

/// Optional server binding overrides. When omitted, Rocket.toml values apply.
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(unused)]
pub struct ServerConfig {
    /// IP address or hostname to bind. E.g. "0.0.0.0" or "127.0.0.1".
    /// ENV: SOUNDGNOME__SERVER__HOST
    pub host: Option<String>,
    /// TCP port to listen on.
    /// ENV: SOUNDGNOME__SERVER__PORT
    pub port: Option<u16>,
}

// ===============================================================================
// Playlists
// ===============================================================================

/// Configuration for playlist-related features.
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(unused)]
pub struct PlaylistsConfig {
    /// Directory where `.m3u8` playlist files are written.
    /// May be relative (to the working directory) or absolute.
    /// Defaults to `{base_library_dir}/.playlists/` when absent.
    pub m3u8_dir: Option<String>,
}

// ===============================================================================
// Tests
// ===============================================================================

#[cfg(test)]
mod downloader_cfg {
    use super::*;

    /// Build a `DownloaderConfig` with explicit format/quality/prefer_original,
    /// no cookies file. Fields are `pub` so we construct directly.
    fn cfg(audio_format: &str, audio_quality: &str, prefer_original: bool) -> DownloaderConfig {
        DownloaderConfig {
            audio_format: audio_format.to_string(),
            audio_quality: audio_quality.to_string(),
            prefer_original,
            cookies_file: None,
            ..DownloaderConfig::default()
        }
    }

    /// Containers a native download may produce. FLAC, M4A and MP3 are tagged
    /// as they are; WAV and AIFF are repacked into FLAC by the downloader
    /// before tagging. Anything else (Opus, WebM) must never be selected.
    const FINALISABLE_SUFFIXES: [&str; 6] = [
        "[ext=flac]",
        "[ext=wav]",
        "[ext=aiff]",
        "[ext=aif]",
        "[ext=m4a]",
        "[ext=mp3]",
    ];

    #[test]
    fn is_native_true_for_best_and_empty() {
        for f in ["best", "BEST", "  best  ", ""] {
            assert!(
                cfg(f, "0", true).is_native(),
                "audio_format {f:?} should be native"
            );
        }
    }

    #[test]
    fn is_native_false_for_transcode_codecs() {
        for f in ["mp3", "flac", "m4a"] {
            assert!(
                !cfg(f, "0", true).is_native(),
                "audio_format {f:?} should force a transcode (not native)"
            );
        }
    }

    #[test]
    fn format_selector_native_prefer_original() {
        assert_eq!(
            cfg("best", "0", true).format_selector(),
            "download[ext=flac]/download[ext=wav]/download[ext=aiff]/download[ext=aif]/\
             download[ext=m4a]/download[ext=mp3]/bestaudio[ext=m4a]/bestaudio[ext=mp3]"
        );
    }

    #[test]
    fn format_selector_native_no_prefer_original() {
        assert_eq!(
            cfg("best", "0", false).format_selector(),
            "bestaudio[ext=m4a]/bestaudio[ext=mp3]"
        );
    }

    #[test]
    fn format_selector_transcode_prefer_original() {
        assert_eq!(
            cfg("mp3", "0", true).format_selector(),
            "download/bestaudio/best"
        );
    }

    #[test]
    fn format_selector_transcode_no_prefer_original() {
        assert_eq!(cfg("mp3", "0", false).format_selector(), "bestaudio/best");
    }

    #[test]
    fn native_selector_only_contains_taggable_containers() {
        for prefer_original in [true, false] {
            let selector = cfg("best", "0", prefer_original).format_selector();
            for token in selector.split('/') {
                assert!(
                    FINALISABLE_SUFFIXES
                        .iter()
                        .any(|suffix| token.ends_with(suffix)),
                    "native selector token {token:?} (prefer_original={prefer_original}) \
                     is not a taggable container; bare bestaudio/best would risk \
                     untaggable Opus/WebM"
                );
            }
        }
    }

    #[test]
    fn transcode_selector_keeps_bare_fallback() {
        // Transcode mode re-encodes, so a bare `best` fallback is intentional
        // and must be present (the opposite of the native invariant).
        for prefer_original in [true, false] {
            let selector = cfg("mp3", "0", prefer_original).format_selector();
            let tokens: Vec<&str> = selector.split('/').collect();
            assert!(
                tokens.contains(&"best"),
                "transcode selector {selector:?} (prefer_original={prefer_original}) \
                 should keep a bare `best` fallback"
            );
        }
    }

    #[test]
    fn transcode_target_none_when_native() {
        assert_eq!(DownloaderConfig::default().transcode_target(), None);
        assert_eq!(cfg("  best  ", "0", true).transcode_target(), None);
    }

    #[test]
    fn transcode_target_some_when_transcoding() {
        assert_eq!(cfg("mp3", "0", true).transcode_target(), Some(("mp3", "0")));
    }

    #[test]
    fn transcode_target_trims_whitespace() {
        assert_eq!(
            cfg(" flac ", " 5 ", true).transcode_target(),
            Some(("flac", "5"))
        );
    }

    #[test]
    fn default_matches_documented_defaults() {
        let d = DownloaderConfig::default();
        assert_eq!(d.audio_format, "best");
        assert_eq!(d.audio_quality, "0");
        assert!(d.prefer_original);
        assert_eq!(d.cookies_file, None);
    }
}
