# Spotify integration

Spotify is a single connection that powers everything Spotify-related in Soundgnome:

- **Audio download** — tracks whose source is Spotify are downloaded directly from Spotify (via librespot), not matched on YouTube.
- **Liked Songs sync** — list and download the signed-in account's Liked Songs.
- **Source adapter** — resolving Spotify track, playlist, album, and artist URLs into metadata.
- **Metadata enrichment** — looking up track details (cover art, track/disc number, release date, artist photos) when enriching any source.

All of this uses one login. There is no app to register and no client id or secret to paste.

## Prerequisites

A **Spotify Premium** account. That is the only requirement.

## Connecting

1. Open **Tools -> Providers -> Spotify** and click **Connect**.
2. A Spotify authorization tab opens. Approve the request.
3. Spotify redirects your browser to `http://127.0.0.1:8898/login?code=...`. This page will not load (it points at the server's loopback, which your browser cannot reach). That is expected.
4. Copy the whole URL from the address bar and paste it into the field on the Spotify card, then click **Complete connection**.

That is it. The credentials are cached on the server, so you only do this once; you will not be asked again unless you disconnect.

> Why paste instead of an automatic redirect? Spotify only accepts the fixed `127.0.0.1:8898` redirect for its desktop client, which a remote or self-hosted server cannot receive directly. Pasting the URL back works over a tailnet or any remote deployment without an SSH tunnel.

The single approval grants three scopes: `streaming` (audio), `user-library-read` and `playlist-read-private` (Liked Songs and playlists). Once connected, `GET /api/providers` lists `"Spotify"`.

## What the connection provides

### Audio download

A Spotify-sourced track is downloaded straight from Spotify as Ogg Vorbis (320 kbps) and tagged in place. Supported URL types:

| URL pattern | What is synced |
|---|---|
| `open.spotify.com/track/...` | Single track |
| `open.spotify.com/playlist/...` | Full playlist (async background task) |
| `open.spotify.com/album/...` | Full album (async background task) |
| `open.spotify.com/artist/...` | All artist tracks (async background task) |

### Liked Songs

Click **Sync my Liked Songs** on the Spotify card (or submit `https://open.spotify.com/collection/tracks`). Soundgnome creates a Liked Songs playlist and downloads every liked track not already in your library, as a background task you can follow on the **Tasks** page.

### Metadata enrichment (tagger)

Even when the source is SoundCloud or YouTube Music, Soundgnome can query Spotify during enrichment for a better match. This reuses the same connection's Web API token and is controlled by `tagger.metadata_providers`:

```toml
[tagger]
# Default order: MusicBrainz first (durable IDs), then Bandcamp, then Spotify
metadata_providers = ["musicbrainz", "bandcamp", "spotify"]

# For local file ingest, Spotify is tried first (better cover art and track numbers)
ingest_metadata_providers = ["spotify", "musicbrainz", "bandcamp"]
```

Spotify enrichment adds cover art, release date, track and disc number, artist Spotify IDs (used for deduplication and linking), and artist photo URLs.

## Behaviour when not connected

If Spotify is not connected:

- Spotify URLs return a `ProviderUnavailable` error, and the download page shows a message.
- The Spotify enrichment provider is silently skipped; MusicBrainz and Bandcamp still run.
- The server starts and operates normally for all other sources.

There is no crash or degraded startup. You will see a `debug`-level log line: `"Spotify metadata provider: not connected, skipping"`.

## Proxy

Spotify Web API traffic (URL resolution, Liked Songs, enrichment) goes through the shared `HttpClientBuilder`, so it honours the `[proxy]` config section. The librespot audio session connects to Spotify's access points directly and does not use the shared proxy.

## Troubleshooting

**"ProviderUnavailable: Spotify" when pasting a Spotify URL**
-> Spotify is not connected. Connect it in Tools -> Providers -> Spotify.

**Login says "No authorization code found" or "Login state did not match"**
-> Paste the entire redirect URL from the address bar (the one starting `http://127.0.0.1:8898/login?code=`), and complete the login in the same session you started it from.

**Liked Songs sync fails with a rate-limit / "Try again later" message**
-> Spotify is temporarily rate-limiting the account (it returns a `Retry-After`, sometimes many hours). Wait it out; the listing is cached for 5 minutes to avoid re-tripping it.

**Spotify appears in `/api/providers` but enrichment still uses MusicBrainz only**
-> Normal. The enrichment order is `["musicbrainz", "bandcamp", "spotify"]` by default; if MusicBrainz finds an exact match first, Spotify is not queried.

**Artist photos are missing**
-> Spotify enrichment performs a best-effort secondary lookup per artist. It can fail silently for artists with non-exact name matches.
