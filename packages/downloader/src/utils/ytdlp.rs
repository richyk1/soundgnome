use config::{models::DownloaderConfig, Config};
use serde::Deserialize;
use shared::{errors::Error, http::ProxyRotator, types::SoundgnomeResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::{io::AsyncReadExt, process::Command};

/// Max attempts (including the first) for transient failures such as
/// rate limiting / bot-detection 403s from YouTube. These are known to be
/// intermittent: the same URL can fail on one run and succeed on the next
/// (see docs/operations/youtube-search-configuration.md).
const MAX_ATTEMPTS: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(2);

/// SoundCloud allows roughly 600 requests per 10 minutes and answers 429 past
/// that. The window only clears with time, so back off in minutes, not seconds.
const QUOTA_RETRY_DELAY: Duration = Duration::from_secs(120);

/// Seconds yt-dlp waits between its own requests. A single track download costs
/// several (metadata, format info, media, thumbnail), so an unpaced sync of a
/// few hundred tracks burns the quota in minutes. One second per request keeps
/// a long sync just under the limit.
const SLEEP_BETWEEN_REQUESTS: &str = "1";

pub async fn download_with_ytdlp(
    url: &str,
    file_name: &str,
    base_library_dir: PathBuf,
    youtube: bool,
) -> Result<PathBuf, Error> {
    let base_library_dir = base_library_dir
        .to_str()
        .ok_or(Error::InvalidPath(base_library_dir.clone()))?;
    let output_path = format!("{}/{}.%(ext)s", base_library_dir, file_name);

    let config = Config::get();
    // Resolved per download, not cached: the user can connect or disconnect
    // SoundCloud from the UI while the server is running.
    let cookies = config.resolved_cookies_file();
    let stdout = run_ytdlp_with_retry(|| {
        build_download_args(
            url,
            &output_path,
            &config.downloader,
            cookies.as_deref(),
            youtube,
        )
    })
    .await?;

    // yt-dlp prints the final file path (after post-processing and the move to
    // its output location) via `--print after_move:%(filepath)s`. The extension
    // is whatever was actually produced (flac/m4a/mp3/...), so take it verbatim
    // instead of assuming ".mp3". Use the last non-empty line.
    let final_path = String::from_utf8_lossy(&stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
        .ok_or(Error::NotFound("downloaded file path".to_string()))?;

    let final_path = repack_lossless_to_flac(final_path).await?;
    // When `youtube_require_256k` is set, a YouTube track that could not be
    // fetched at the 256k tier is a failure, not a downgrade. yt-dlp reports the
    // adaptive m4a bitrate conservatively, so verify the actual file and reject
    // anything below 256k. When the flag is off, the selector already fell back
    // to the next best tier, so no gate is applied.
    if youtube && config.downloader.youtube_require_256k {
        ensure_sabr_quality(&final_path).await?;
    }
    Ok(final_path)
}

/// Below this, a YouTube download did not land the 256k AAC master.
const MIN_SABR_BITRATE_BPS: u64 = 200_000;

/// Reject a YouTube download that came back below the 256k tier. With a Premium
/// account a track that cannot be fetched at 256k is an error, not a downgrade.
/// yt-dlp advertises the adaptive m4a at ~130k even when it can deliver 256k, so
/// the true quality is only known after downloading: a file under
/// `MIN_SABR_BITRATE_BPS` means no 256k master was obtained (throttle, SABR
/// degradation, or no such master); delete it and error rather than archive it.
async fn ensure_sabr_quality(path: &Path) -> SoundgnomeResult<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| Error::InvalidPath(path.to_path_buf()))?;
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=bit_rate",
            "-of",
            "default=nk=1:nw=1",
            path_str,
        ])
        .output()
        .await?;
    let bitrate: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if bitrate < MIN_SABR_BITRATE_BPS {
        let _ = std::fs::remove_file(path);
        return Err(Error::Custom(format!(
            "YouTube returned only {} kbps for this track (no 256k master); \
             refusing to archive below 256k",
            bitrate / 1000
        )));
    }
    Ok(())
}

/// What the source can currently supply, without downloading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailableQuality {
    /// True when the best selectable format is a lossless original.
    pub lossless: bool,
    /// Advertised average bitrate in kbps, when the source reports one.
    pub bitrate_kbps: Option<u32>,
}

/// Ask yt-dlp what it *would* download for `url`, without fetching any audio.
///
/// This is one metadata request, roughly a second, and it is what makes
/// upgrade decisions possible: a file already in the library is only worth
/// re-fetching when the source now offers something better.
pub async fn probe_available_quality(url: &str) -> SoundgnomeResult<AvailableQuality> {
    let config = Config::get();
    let cookies = config.resolved_cookies_file();

    let stdout = run_ytdlp_with_retry(|| {
        let mut args = vec![
            url.to_string(),
            "-f".to_string(),
            config.downloader.format_selector(),
            "--simulate".to_string(),
            "--print".to_string(),
            // Extension identifies lossless containers; abr covers the rest.
            "%(ext)s|%(abr)s".to_string(),
        ];
        if let Some(cookies) = cookies.as_deref() {
            args.push("--cookies".to_string());
            args.push(cookies.to_string_lossy().into_owned());
        }
        append_proxy_arg(&mut args);
        args
    })
    .await?;

    let line = String::from_utf8_lossy(&stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.contains('|'))
        .map(str::to_string)
        .ok_or_else(|| Error::NotFound(format!("available formats for {}", url)))?;

    let (ext, abr) = line.split_once('|').unwrap_or((line.as_str(), ""));
    let ext = ext.trim().to_lowercase();

    Ok(AvailableQuality {
        lossless: ext == "flac" || UNTAGGABLE_LOSSLESS.contains(&ext.as_str()),
        // yt-dlp prints "NA" when a format carries no bitrate, which is normal
        // for lossless originals.
        bitrate_kbps: abr.trim().parse::<f64>().ok().map(|abr| abr as u32),
    })
}

/// Containers that carry lossless audio but cannot hold the tags Soundgnome
/// writes. Uploaders most often offer WAV, so these must not be rejected.
const UNTAGGABLE_LOSSLESS: [&str; 3] = ["wav", "aiff", "aif"];

/// Repack a lossless-but-untaggable download into FLAC.
///
/// This is a container change, not a re-encode: FLAC stores the same samples,
/// so nothing is lost, the file gets roughly 40% smaller, and the tagger can
/// finally write the metadata and the SOUNDOME_ID anchor.
///
/// Any other extension is returned untouched.
async fn repack_lossless_to_flac(path: PathBuf) -> Result<PathBuf, Error> {
    let is_untaggable_lossless = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| UNTAGGABLE_LOSSLESS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false);

    if !is_untaggable_lossless {
        return Ok(path);
    }

    let flac_path = path.with_extension("flac");
    tracing::info!(
        "Repacking lossless original {} to FLAC",
        path.file_name().unwrap_or_default().to_string_lossy()
    );

    let output = Command::new("ffmpeg")
        // -nostdin plus a null stdin: ffmpeg reads the terminal for interactive
        // keys by default, and a background child that touches a TTY gets
        // SIGTTIN and stops dead, hanging the whole serial task queue.
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&path)
        // Keep the samples as they are; only the container and coding change.
        .args(["-c:a", "flac", "-compression_level", "8"])
        .arg(&flac_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .await
        .map_err(|e| Error::Custom(format!("ffmpeg is required to repack WAV originals: {}", e)))?;

    if !output.status.success() {
        // Leave the original in place: a lossy fallback would be worse than a
        // visible failure the user can retry.
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Custom(format!(
            "ffmpeg failed to repack {} to FLAC: {}",
            path.display(),
            stderr.trim()
        )));
    }

    if let Err(e) = tokio::fs::remove_file(&path).await {
        tracing::warn!("Could not remove {} after repacking: {}", path.display(), e);
    }

    Ok(flac_path)
}

fn build_download_args(
    url: &str,
    output_path: &str,
    config: &DownloaderConfig,
    cookies_file: Option<&Path>,
    youtube: bool,
) -> Vec<String> {
    let mut args = vec![url.to_string(), "-f".to_string()];
    if youtube {
        // YouTube: prefer the 256k AAC master (itag 141) via the web_music client,
        // which serves it as a direct https/dash stream for audio-only tracks.
        // yt-dlp handles pot + nsig. Requires the SABR-capable yt-dlp build, a PO
        // token provider on 127.0.0.1:4416, deno on PATH, and a logged-in Premium
        // `cookies_file`.
        //
        // With `youtube_require_256k`, request only 141: a track with no 256k
        // master fails with "requested format is not available" and the caller
        // propagates it. Otherwise fall back to 130k AAC (140), then the best
        // taggable audio, so tracks without a 256k master still download.
        let selector = if config.youtube_require_256k {
            "141-1/141-dashy/141"
        } else {
            "141-1/141-dashy/140-1/140-dashy/bestaudio[ext=m4a]/bestaudio[ext=mp3]"
        };
        args.push(selector.to_string());
        args.push("--extractor-args".to_string());
        args.push(
            "youtube:formats=duplicate;player_client=web_music;webpage_client=web_music"
                .to_string(),
        );
    } else {
        args.push(config.format_selector());
    }
    // Take the audio stream out of whatever container it arrives in, keeping the
    // source codec (no --audio-format means no re-encode).
    args.push("--extract-audio".to_string());

    // Transcode only when a specific format is requested; "best" keeps native.
    if let Some((format, quality)) = config.transcode_target() {
        args.push("--audio-format".to_string());
        args.push(format.to_string());
        args.push("--audio-quality".to_string());
        args.push(quality.to_string());
    }

    // No --embed-thumbnail on purpose. yt-dlp cannot embed into WAV or AIFF, so
    // it fails the very lossless originals worth downloading. The tagger embeds
    // the real cover art from the source metadata later anyway
    // (`tag_file_with_track_and_cover`), which is both higher resolution and
    // survives the repack to FLAC.

    // SoundCloud only serves downloadable originals (FLAC) to authenticated
    // clients; cookies also unlock age/region-gated tracks.
    if let Some(cookies) = cookies_file {
        args.push("--cookies".to_string());
        args.push(cookies.to_string_lossy().into_owned());
    }

    // Stay under SoundCloud's quota during long syncs, and let yt-dlp ride out
    // a block itself before our own retry loop takes over.
    args.push("--sleep-requests".to_string());
    args.push(SLEEP_BETWEEN_REQUESTS.to_string());
    args.push("--extractor-retries".to_string());
    args.push("3".to_string());
    args.push("--retry-sleep".to_string());
    args.push("extractor:30".to_string());

    // Download for real and print the final file path after post-processing.
    args.push("--no-simulate".to_string());
    args.push("--print".to_string());
    args.push("after_move:%(filepath)s".to_string());

    args.push("--output".to_string());
    args.push(output_path.to_string());

    append_proxy_arg(&mut args);

    args
}

/// Minimal shape of a single JSON object emitted by
/// `yt-dlp "ytsearchN:<query>" --dump-json --skip-download --flat-playlist`.
/// yt-dlp emits many more fields (thumbnails, view_count, description, ...);
/// only the ones Soundgnome actually needs are modeled here, the rest are
/// ignored by serde.
#[derive(Debug, Deserialize)]
struct YtDlpSearchEntry {
    id: String,
    title: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    /// Seconds, as a float. Absent for some live streams/premieres.
    #[serde(default)]
    duration: Option<f64>,
}

/// A single YouTube search result, already narrowed down to what Soundgnome needs.
#[derive(Debug, Clone)]
pub struct YtDlpSearchResult {
    pub id: String,
    pub title: String,
    pub author: String,
    /// Duration in whole seconds, when yt-dlp reports one.
    pub duration: Option<i32>,
}

/// Search YouTube via yt-dlp's `ytsearchN:` pseudo-URL and return up to `limit`
/// results, without downloading anything (`--skip-download --flat-playlist`
/// keeps this to a single, fast metadata-only request per search).
///
/// This replaces the previous Invidious-based search: yt-dlp talks to YouTube
/// directly (through the shared proxy when configured), so there is no more
/// third-party instance to select or fall back to.
pub async fn search_with_ytdlp(
    query: &str,
    limit: usize,
) -> SoundgnomeResult<Vec<YtDlpSearchResult>> {
    let search_spec = format!("ytsearch{}:{}", limit, query);

    let stdout = run_ytdlp_with_retry(|| {
        let mut args = vec![
            search_spec.clone(),
            "--dump-json".to_string(),
            "--skip-download".to_string(),
            "--flat-playlist".to_string(),
        ];
        append_proxy_arg(&mut args);
        args
    })
    .await?;

    // In `--dump-json --flat-playlist` mode yt-dlp prints one JSON object per
    // line (one per search result), not a single JSON document/array.
    let results = String::from_utf8_lossy(&stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(
            |line| match serde_json::from_str::<YtDlpSearchEntry>(line) {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!("Skipping unparsable yt-dlp search result: {}", err);
                    None
                }
            },
        )
        .map(|entry| YtDlpSearchResult {
            id: entry.id,
            title: entry.title,
            author: entry.channel.or(entry.uploader).unwrap_or_default(),
            duration: entry.duration.map(|d| d.round() as i32),
        })
        .collect();

    Ok(results)
}

/// Runs `yt-dlp` via `run_ytdlp`, retrying on transient failures (rate
/// limiting / bot-detection 403s) with a short backoff.
///
/// `build_args` is called fresh on every attempt so that a rotating proxy
/// (see `ProxyRotator`) can pick a different upstream IP on retry instead of
/// repeating the same request that just got rate-limited.
async fn run_ytdlp_with_retry<F>(mut build_args: F) -> SoundgnomeResult<Vec<u8>>
where
    F: FnMut() -> Vec<String>,
{
    let mut attempt = 1;
    loop {
        let args = build_args();
        tracing::info!("Running yt-dlp with args (attempt {}): {:?}", attempt, args);

        match run_ytdlp(&args).await {
            Ok(stdout) => return Ok(stdout),
            Err(Error::ExitCode { code, stderr })
                if attempt < MAX_ATTEMPTS && is_transient_error(&stderr) =>
            {
                // SoundCloud's limit is a quota over a ten minute window, so a
                // two second backoff just burns the remaining attempts. Wait
                // long enough for the window to actually move.
                let delay = if is_quota_error(&stderr) {
                    QUOTA_RETRY_DELAY * attempt
                } else {
                    RETRY_BASE_DELAY * attempt
                };
                tracing::warn!(
                    "yt-dlp failed with a transient error (exit code {}), retrying in {:?} (attempt {}/{}): {}",
                    code,
                    delay,
                    attempt,
                    MAX_ATTEMPTS,
                    stderr.trim()
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// A hard quota rather than a momentary block: waiting seconds will not help.
fn is_quota_error(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("429") || lower.contains("too many requests") || lower.contains("api rate limit")
}

/// Heuristic: does this yt-dlp stderr look like a transient rate-limit / bot
/// detection failure rather than a permanent one (removed video, DRM, region
/// lock, etc.)? These are known to be intermittent for the exact same URL
/// (see docs/operations/youtube-search-configuration.md).
fn is_transient_error(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("403")
        || lower.contains("429")
        || lower.contains("too many requests")
        || lower.contains("rate-limit")
        || lower.contains("rate limit")
        || lower.contains("sign in to confirm")
}

/// Spawn `yt-dlp` with the given args and return its captured stdout.
/// Maps a non-zero exit code to `Error::ExitCode` carrying the captured stderr.
async fn run_ytdlp(args: &[String]) -> SoundgnomeResult<Vec<u8>> {
    let mut child = Command::new("yt-dlp")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .spawn()?;

    // Read stdout asynchronously to prevent buffer overflow
    let mut stdout = Vec::new();
    if let Some(mut child_stdout) = child.stdout.take() {
        tokio::io::copy(&mut child_stdout, &mut stdout).await?;
    }

    // TODO: Implement timeout handling
    let exit_code = child.wait().await?;

    if !exit_code.success() {
        let mut stderr = Vec::new();
        if let Some(mut reader) = child.stderr {
            reader.read_to_end(&mut stderr).await?;
        }
        return Err(Error::ExitCode {
            code: exit_code.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }

    Ok(stdout)
}

/// Append `--proxy <url>` when a proxy is configured via `Config.proxy`.
fn append_proxy_arg(args: &mut Vec<String>) {
    if let Some(proxy_url) = ProxyRotator::get().get_next_proxy() {
        tracing::info!("Using proxy for yt-dlp: {}", proxy_url);
        args.push("--proxy".to_string());
        args.push(proxy_url);
    }
}
