//! Streaming of library audio files to the browser.
//!
//! An `<audio>` element needs byte ranges to seek, and Rocket's `NamedFile`
//! answers every request with the whole file, so this module implements the
//! `Range` half of RFC 7233 directly.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::services::ServiceLayer;
use rocket::{
    get,
    http::{ContentType, Status},
    response::{self, Responder},
    tokio::io::{AsyncReadExt, AsyncSeekExt},
    Request, Response,
};
use rocket_okapi::openapi;

use crate::utils::{database::Db, error::CustomError};

/// Largest slice served for a single ranged request.
///
/// A partial response is allowed to be shorter than the client asked for, and
/// browsers simply come back for the next chunk. Capping here keeps a seek in a
/// long lossless file from buffering the whole thing into memory.
const MAX_CHUNK: u64 = 2 * 1024 * 1024;

/// Either a whole file streamed from disk, or one byte range held in memory.
pub enum AudioResponse {
    /// No `Range` header: stream the file, nothing is buffered.
    Full {
        file: rocket::tokio::fs::File,
        len: u64,
        content_type: ContentType,
    },
    /// `Range` request: a bounded slice, answered with 206.
    Partial {
        bytes: Vec<u8>,
        start: u64,
        end: u64,
        total: u64,
        content_type: ContentType,
    },
}

impl<'r> Responder<'r, 'static> for AudioResponse {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        match self {
            AudioResponse::Full {
                file,
                len,
                content_type,
            } => Response::build()
                .header(content_type)
                .raw_header("Accept-Ranges", "bytes")
                .sized_body(len as usize, file)
                .ok(),
            AudioResponse::Partial {
                bytes,
                start,
                end,
                total,
                content_type,
            } => Response::build()
                .status(Status::PartialContent)
                .header(content_type)
                .raw_header("Accept-Ranges", "bytes")
                .raw_header(
                    "Content-Range",
                    format!("bytes {}-{}/{}", start, end, total),
                )
                .sized_body(bytes.len(), Cursor::new(bytes))
                .ok(),
        }
    }
}

impl rocket_okapi::response::OpenApiResponderInner for AudioResponse {
    fn responses(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
    ) -> rocket_okapi::Result<okapi::openapi3::Responses> {
        use okapi::openapi3::{RefOr, Response as OpenApiResponse, Responses};

        let mut responses = Responses::default();
        for (status, description) in [
            ("200", "The whole audio file."),
            ("206", "The requested byte range of the audio file."),
        ] {
            responses.responses.insert(
                status.to_string(),
                RefOr::Object(OpenApiResponse {
                    description: description.to_string(),
                    ..Default::default()
                }),
            );
        }

        Ok(responses)
    }
}

/// Result of interpreting a `Range` header against a file of a known size.
#[derive(Debug, PartialEq)]
enum ParsedRange {
    /// A satisfiable inclusive byte range, already capped to `MAX_CHUNK`.
    Range(u64, u64),
    /// Syntactically valid but the start is at or past the end of the file
    /// (RFC 7233 -> 416 Range Not Satisfiable).
    Unsatisfiable,
    /// Not a range we honor (missing/malformed/inverted); ignore it and send the
    /// whole file per RFC 7233.
    Ignore,
}

/// Parse a single-range `bytes=` header for a file of `total` bytes (`total > 0`).
///
/// Multi-range requests are not supported; browsers do not use them for audio,
/// and a caller asking for one gets the first range it listed.
fn parse_range(header: &str, total: u64) -> ParsedRange {
    let Some(spec) = header
        .strip_prefix("bytes=")
        .and_then(|s| s.split(',').next())
        .map(str::trim)
    else {
        return ParsedRange::Ignore;
    };
    let Some((raw_start, raw_end)) = spec.split_once('-') else {
        return ParsedRange::Ignore;
    };

    let (start, end) = if raw_start.is_empty() {
        // Suffix form, "-500" means the last 500 bytes.
        let Ok(from_end) = raw_end.trim().parse::<u64>() else {
            return ParsedRange::Ignore;
        };
        if from_end == 0 {
            // "bytes=-0" cannot be satisfied.
            return ParsedRange::Unsatisfiable;
        }
        (total.saturating_sub(from_end), total - 1)
    } else {
        let Ok(start) = raw_start.parse::<u64>() else {
            return ParsedRange::Ignore;
        };
        let end = match raw_end.trim() {
            "" => total - 1,
            value => match value.parse::<u64>() {
                Ok(v) => v.min(total - 1),
                Err(_) => return ParsedRange::Ignore,
            },
        };
        (start, end)
    };

    // A start at or past the end of the file is unsatisfiable.
    if start >= total {
        return ParsedRange::Unsatisfiable;
    }
    // An inverted range (start after end) is nonsensical; ignore it.
    if start > end {
        return ParsedRange::Ignore;
    }

    ParsedRange::Range(start, end.min(start + MAX_CHUNK - 1))
}

/// Rocket's extension table has no entry for the containers a music library
/// actually holds, and `application/octet-stream` makes some browsers refuse to
/// play the response, so map the audio ones explicitly.
fn content_type_for(path: &Path) -> ContentType {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_lowercase();

    match ext.as_str() {
        "m4a" | "mp4" | "aac" => ContentType::new("audio", "mp4"),
        "mp3" => ContentType::new("audio", "mpeg"),
        "flac" => ContentType::new("audio", "flac"),
        "ogg" | "opus" => ContentType::new("audio", "ogg"),
        "wav" => ContentType::new("audio", "wav"),
        _ => ContentType::from_extension(&ext).unwrap_or(ContentType::Binary),
    }
}

fn not_found(code: &str, message: String) -> crate::utils::error::Error {
    crate::utils::error::Error::Custom(CustomError {
        status: Status::NotFound,
        code: code.to_string(),
        message,
    })
}

fn range_not_satisfiable(total: u64) -> crate::utils::error::Error {
    crate::utils::error::Error::Custom(CustomError {
        status: Status::RangeNotSatisfiable,
        code: "RangeNotSatisfiable".to_string(),
        message: format!("Requested range not satisfiable; file is {total} bytes"),
    })
}

/// Stream a library track's audio file, with range support so the browser can
/// seek without downloading the whole file first.
#[openapi]
#[get("/tracks/<id>/audio")]
pub async fn stream(
    id: i32,
    range: Option<crate::utils::range::RangeHeader>,
    db: Db,
    services: &rocket::State<Arc<ServiceLayer>>,
) -> Result<AudioResponse, crate::utils::error::Error> {
    let services = Arc::clone(services);

    let track = db
        .run(move |conn| services.track_service.get_by_id(conn, id))
        .await
        .map_err(|err| not_found("NotFound", err.to_string()))?;

    let path: PathBuf = track
        .file_path
        .ok_or_else(|| not_found("NoFile", "Track has no local file".to_string()))?;

    let mut file = rocket::tokio::fs::File::open(&path)
        .await
        .map_err(|e| not_found("FileNotFound", format!("{}: {}", path.display(), e)))?;
    let total = file
        .metadata()
        .await
        .map_err(|e| not_found("FileUnreadable", e.to_string()))?
        .len();
    let content_type = content_type_for(&path);

    if total == 0 {
        return Err(not_found("EmptyFile", "Audio file is empty".to_string()));
    }

    let (start, end) = match range.as_ref().map(|r| parse_range(&r.0, total)) {
        Some(ParsedRange::Range(start, end)) => (start, end),
        Some(ParsedRange::Unsatisfiable) => return Err(range_not_satisfiable(total)),
        // No header, or a header we ignore: send the whole file.
        Some(ParsedRange::Ignore) | None => {
            return Ok(AudioResponse::Full {
                file,
                len: total,
                content_type,
            });
        }
    };

    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| not_found("SeekFailed", e.to_string()))?;

    let mut bytes = vec![0u8; (end - start + 1) as usize];
    file.read_exact(&mut bytes)
        .await
        .map_err(|e| not_found("ReadFailed", e.to_string()))?;

    Ok(AudioResponse::Partial {
        bytes,
        start,
        end,
        total,
        content_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: u64 = 10_000;

    #[test]
    fn parses_a_closed_range() {
        assert_eq!(parse_range("bytes=0-499", TOTAL), ParsedRange::Range(0, 499));
        assert_eq!(parse_range("bytes=500-999", TOTAL), ParsedRange::Range(500, 999));
    }

    #[test]
    fn open_ended_range_runs_to_the_end() {
        // This is what a browser sends first for an audio element.
        assert_eq!(parse_range("bytes=0-", TOTAL), ParsedRange::Range(0, TOTAL - 1));
        assert_eq!(
            parse_range("bytes=9000-", TOTAL),
            ParsedRange::Range(9000, TOTAL - 1)
        );
    }

    #[test]
    fn suffix_range_counts_back_from_the_end() {
        assert_eq!(
            parse_range("bytes=-500", TOTAL),
            ParsedRange::Range(9500, TOTAL - 1)
        );
        // Longer than the file: clamp to the whole file rather than underflow.
        assert_eq!(
            parse_range("bytes=-99999", TOTAL),
            ParsedRange::Range(0, TOTAL - 1)
        );
    }

    #[test]
    fn caps_an_oversized_range() {
        let huge = 10 * MAX_CHUNK;
        assert_eq!(
            parse_range("bytes=0-", huge),
            ParsedRange::Range(0, MAX_CHUNK - 1),
            "a full-file request must be chunked, not buffered whole"
        );
    }

    #[test]
    fn unsatisfiable_ranges_signal_416() {
        assert_eq!(
            parse_range("bytes=10000-", TOTAL),
            ParsedRange::Unsatisfiable,
            "start past end"
        );
        assert_eq!(
            parse_range("bytes=-0", TOTAL),
            ParsedRange::Unsatisfiable,
            "empty suffix"
        );
    }

    #[test]
    fn malformed_ranges_are_ignored() {
        assert_eq!(parse_range("bytes=600-500", TOTAL), ParsedRange::Ignore, "inverted");
        assert_eq!(parse_range("items=0-10", TOTAL), ParsedRange::Ignore, "wrong unit");
        assert_eq!(parse_range("bytes=abc-def", TOTAL), ParsedRange::Ignore, "not numbers");
    }
}
