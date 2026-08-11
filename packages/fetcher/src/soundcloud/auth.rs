//! SoundCloud session credential handling.
//!
//! SoundCloud closed public API app registration years ago, so there is no
//! OAuth client flow available to a self-hosted app. The only credential a user
//! can supply is the `oauth_token` cookie from their own logged-in browser
//! session, which is exactly what yt-dlp consumes (`--username oauth
//! --password <token>`, or the same value inside a cookies file).
//!
//! The token is stored as a single-entry Netscape cookie file so the existing
//! `--cookies` plumbing in the downloader picks it up with no special casing.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use config::Config;
use shared::{errors::Error, http::HttpClientBuilder, types::SoundomeResult};

/// Endpoint used to check a token. Returns 200 with the account payload for a
/// valid token and 401 for anything else, without needing a `client_id`.
const ME_URL: &str = "https://api-v2.soundcloud.com/me";

/// Cookie lifetime written into the file. SoundCloud's own `oauth_token` cookie
/// is long lived; the real expiry is server side, so this is only about not
/// having yt-dlp discard the entry as stale.
const COOKIE_TTL_SECS: u64 = 365 * 24 * 60 * 60;

/// Ask SoundCloud who owns this token. `Ok(Some(username))` when it is valid,
/// `Ok(None)` when it is rejected.
pub async fn verify_token(token: &str) -> SoundomeResult<Option<String>> {
    let response = HttpClientBuilder::get_reqwest_client()?
        .get(ME_URL)
        .header("Authorization", format!("OAuth {}", token))
        .send()
        .await
        .map_err(|e| Error::Custom(format!("SoundCloud token check failed: {}", e)))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Unreadable SoundCloud response: {}", e)))?;

    // `username` is the handle, `permalink` is the URL slug. Either identifies
    // the account well enough for the UI; fall back so a schema tweak upstream
    // does not turn a valid token into a failure.
    let username = body
        .get("username")
        .or_else(|| body.get("permalink"))
        .and_then(|v| v.as_str())
        .unwrap_or("SoundCloud user")
        .to_string();

    Ok(Some(username))
}

/// Write the token as a Netscape cookie file readable by yt-dlp.
pub fn store_token(token: &str) -> SoundomeResult<()> {
    let path = Config::get().soundcloud_cookies_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::Custom(format!("Cannot create {}: {}", parent.display(), e)))?;
    }

    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        + COOKIE_TTL_SECS;

    write_private(&path, &cookie_file_contents(token, expiry))
        .map_err(|e| Error::Custom(format!("Cannot write {}: {}", path.display(), e)))
}

/// Render the one-entry Netscape cookie file yt-dlp reads.
///
/// Fields are tab separated: domain, include_subdomains, path, secure, expiry,
/// name, value. The header line is not decoration, yt-dlp rejects a file
/// without it.
fn cookie_file_contents(token: &str, expiry: u64) -> String {
    format!(
        "# Netscape HTTP Cookie File\n\
         .soundcloud.com\tTRUE\t/\tTRUE\t{}\toauth_token\t{}\n",
        expiry, token
    )
}

/// Remove the stored token. Succeeds when there was nothing to remove.
pub fn clear_token() -> SoundomeResult<()> {
    let path = Config::get().soundcloud_cookies_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Custom(format!(
            "Cannot remove {}: {}",
            path.display(),
            e
        ))),
    }
}

/// Read back the stored token, if any.
pub fn stored_token() -> Option<String> {
    let path = Config::get().soundcloud_cookies_path();
    parse_token(&fs::read_to_string(path).ok()?)
}

/// Pull the `oauth_token` value out of a Netscape cookie file.
fn parse_token(contents: &str) -> Option<String> {
    contents
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                [.., name, value] if *name == "oauth_token" && !value.is_empty() => {
                    Some(value.to_string())
                }
                _ => None,
            }
        })
        .next()
}

/// Create the file with owner-only permissions. This is a live session
/// credential: anyone who reads it is logged in as the user.
fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_file_round_trips_through_the_parser() {
        let contents = cookie_file_contents("tok-123", 2_000_000_000);

        // yt-dlp rejects a cookie file without this exact header line.
        assert!(contents.starts_with("# Netscape HTTP Cookie File\n"));
        // Seven tab separated fields, or yt-dlp treats the line as malformed.
        let entry = contents.lines().nth(1).expect("missing cookie line");
        assert_eq!(entry.split('\t').count(), 7);

        assert_eq!(parse_token(&contents).as_deref(), Some("tok-123"));
    }

    #[test]
    fn ignores_comments_blank_lines_and_other_cookies() {
        let contents = "# Netscape HTTP Cookie File\n\
                        \n\
                        .soundcloud.com\tTRUE\t/\tTRUE\t0\tsc_anonymous_id\tnope\n\
                        .soundcloud.com\tTRUE\t/\tTRUE\t0\toauth_token\twanted\n";

        assert_eq!(parse_token(contents).as_deref(), Some("wanted"));
    }

    #[test]
    fn rejects_files_without_a_usable_token() {
        assert_eq!(parse_token(""), None);
        assert_eq!(parse_token("# comment only\n"), None);
        // Present but empty value: treat as absent rather than sending an empty
        // Authorization header to SoundCloud.
        assert_eq!(
            parse_token(".soundcloud.com\tTRUE\t/\tTRUE\t0\toauth_token\t\n"),
            None
        );
    }
}
