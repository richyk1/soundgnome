use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    // ============================================================================================
    // Generic errors
    // ============================================================================================
    #[error("not found: {0}")]
    NotFound(String),
    #[error("no match {0} found for {1}")]
    NoMatch(String, String),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("cache error: {0}")]
    Cache(String),
    #[error("rate limit exceeded: {0}")]
    RateLimit(String),
    #[error("not implemented error: {0}")]
    NotImplemented(String),

    // ============================================================================================
    // Domain errors
    // ============================================================================================

    // Track
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("track already exists: {0}")]
    TrackExists(String),
    #[error("track download failed: {0}")]
    TrackDownloadFailed(String),
    #[error("track processing failed: {0}")]
    TrackProcessingFailed(String),
    #[error("track metadata error: {0}")]
    TrackMetadataError(String),

    #[error("{0} provider is not available")]
    ProviderUnavailable(String),

    // ============================================================================================
    // Technical errors
    // ============================================================================================

    // HTTP
    #[error("{0} http error: {1}")]
    Http(String, String),

    // Database
    #[error("database error: {0}")]
    Database(String),

    // Other
    #[error("custom error: {0}")]
    Custom(String),
    #[error("unknown error")]
    Unknown,

    // AI
    #[error("no AI backend configured")]
    NoAIBackend,

    // String
    #[error("string template error: {0}")]
    TemplateRenderingError(tinytemplate::error::Error),
    #[error("invalid path: {0}")]
    InvalidPath(std::path::PathBuf),

    // CLI Parsing
    #[error("{0}")]
    Io(std::io::Error),
    #[error("{0}")]
    Json(serde_json::Error),
    #[error("parse error")]
    Parse,
    #[error("missing argument")]
    MissingArg,
    #[error("invalid argument")]
    InvalidArg,
    #[error("process timeout")]
    ProcessTimeout,
    #[error("task cancelled")]
    Cancelled,
    #[error("{}", subprocess_error_reason(&.stderr))]
    ExitCode { code: i32, stderr: String },

    #[error("SoundCloud track is DRM protected and cannot be downloaded directly: {0}")]
    SoundCloudDrmProtected(String),
}

/// Extract a human-readable failure reason from a subprocess's captured stderr.
///
/// yt-dlp and ffmpeg print the real cause on a line beginning with `ERROR:`
/// (e.g. `ERROR: [youtube] <id>: Requested format is not available`). Surface
/// that line - stripped of the noisy `[extractor] <id>:` prefix - so callers and
/// the UI see the actual reason instead of a generic "process error". Falls back
/// to the last non-empty stderr line, then to a generic message when empty.
fn subprocess_error_reason(stderr: &str) -> String {
    let line = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .or_else(|| stderr.lines().map(str::trim).rev().find(|l| !l.is_empty()));
    match line {
        Some(l) => {
            let msg = l.strip_prefix("ERROR:").map(str::trim).unwrap_or(l);
            // yt-dlp extractor errors read "[youtube] <id>: <message>"; peel both
            // layers, but only when the "[extractor]" prefix is actually present
            // so unrelated messages containing ": " are left intact.
            let msg = match msg.strip_prefix('[').and_then(|r| r.split_once("] ")) {
                Some((_, after)) => after.split_once(": ").map(|(_, m)| m).unwrap_or(after),
                None => msg,
            };
            let trimmed = msg.trim();
            if trimmed.is_empty() {
                "process failed with no error output".to_string()
            } else {
                trimmed.to_string()
            }
        }
        None => "process failed with no error output".to_string(),
    }
}

#[cfg(feature = "diesel_integration")]
impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        Error::Database(format!("Database error: {}", err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<tinytemplate::error::Error> for Error {
    fn from(err: tinytemplate::error::Error) -> Self {
        Error::TemplateRenderingError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytdlp_format_error_is_surfaced_cleanly() {
        let stderr = "[youtube] rN7jdDyI7Zc: Downloading webpage\n\
                      ERROR: [youtube] rN7jdDyI7Zc: Requested format is not available. Use --list-formats for a list of available formats";
        let e = Error::ExitCode {
            code: 1,
            stderr: stderr.to_string(),
        };
        assert_eq!(
            e.to_string(),
            "Requested format is not available. Use --list-formats for a list of available formats"
        );
    }

    #[test]
    fn video_unavailable_is_surfaced() {
        let e = Error::ExitCode {
            code: 1,
            stderr: "ERROR: [youtube] abc: Video unavailable".to_string(),
        };
        assert_eq!(e.to_string(), "Video unavailable");
    }

    #[test]
    fn non_extractor_error_is_left_intact() {
        // No "[extractor] id:" prefix -> the message with a colon must not be truncated.
        let e = Error::ExitCode {
            code: 2,
            stderr: "ERROR: unable to download: connection reset".to_string(),
        };
        assert_eq!(e.to_string(), "unable to download: connection reset");
    }

    #[test]
    fn empty_stderr_falls_back_to_generic() {
        let e = Error::ExitCode {
            code: 1,
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "process failed with no error output");
    }
}
