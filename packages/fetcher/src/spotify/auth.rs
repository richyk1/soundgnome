//! Spotify app credential handling.
//!
//! Unlike SoundCloud, Spotify still runs an open developer programme, so the
//! credential here is an app's client id and secret rather than a scraped
//! session. Those grant access to public catalogue data only: enough to enrich
//! metadata, and deliberately not enough to read a user's own library.

use std::fs;
use std::io::Write;
use std::path::Path;

use config::{models::StoredSpotifyCredentials, Config};
use shared::{errors::Error, http::HttpClientBuilder, types::SoundomeResult};

/// Client-credentials token endpoint. A successful exchange is the only proof
/// that a pasted id and secret actually work.
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Exchange the pair for a token. `Ok(false)` means Spotify rejected them.
pub async fn verify_credentials(client_id: &str, client_secret: &str) -> SoundomeResult<bool> {
    let response = HttpClientBuilder::get_reqwest_client()?
        .post(TOKEN_URL)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Spotify token request failed: {}", e)))?;

    Ok(response.status().is_success())
}

/// Persist the credentials for later runs.
pub fn store_credentials(client_id: &str, client_secret: &str) -> SoundomeResult<()> {
    let path = Config::get().spotify_credentials_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::Custom(format!("Cannot create {}: {}", parent.display(), e)))?;
    }

    let payload = StoredSpotifyCredentials {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|e| Error::Custom(format!("Cannot serialise credentials: {}", e)))?;

    write_private(&path, &body)
        .map_err(|e| Error::Custom(format!("Cannot write {}: {}", path.display(), e)))
}

/// Forget the stored credentials. Succeeds when there was nothing to remove.
pub fn clear_credentials() -> SoundomeResult<()> {
    let path = Config::get().spotify_credentials_path();
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

/// The client id currently in effect, from config or from the stored file.
/// The secret is deliberately not returned: nothing outside this module needs
/// to show it.
pub fn configured_client_id() -> Option<String> {
    Config::get()
        .resolved_spotify_credentials()
        .map(|(client_id, _)| client_id)
}

/// Create the file with owner-only permissions. The secret is a credential.
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
