# SoundCloud

SoundCloud is one of the best-supported sources in Soundome. No credentials are required. This page covers what works, what does not, and how to get the best results.

## No configuration required

SoundCloud works out of the box. Simply paste a URL and Soundome handles the rest.

```
https://soundcloud.com/artist-name/track-title
https://soundcloud.com/artist-name/sets/playlist-name
https://soundcloud.com/artist-name
```

URL parameters (`?si=...`, `?utm_source=...`) are stripped automatically before processing.

## Supported URL types

| URL pattern | What is synced |
|---|---|
| `soundcloud.com/<artist>/<track>` | Single track |
| `soundcloud.com/<artist>/sets/<playlist>` | Full playlist (async background task) |
| `soundcloud.com/<artist>` (profile root) | All artist uploads (async background task) |

> **Albums:** SoundCloud album URLs use the same `/sets/` path structure as playlists and are routed through the playlist sync path. The result is the same.

## The metadata noise problem

SoundCloud allows uploaders to write anything in the title and artist fields. Common issues:

- Titles containing the artist name: `ArtistName - Track Title (Original Mix)`
- Credited artists embedded in the title: `Track Title ft. Another Artist`
- Release tags and catalog numbers: `[FREE DL]`, `[001]`, `[OUT NOW]`
- Platform credits: `(Soundcloud Exclusive)`, `via NCS`
- Collaborations written in non-standard formats: `Artist1 b2b Artist2`, `Artist1 vs Artist2 & Artist3`

Without cleanup, these noisy values go directly into your library's file tags and folder names.

## AI metadata cleanup

Soundome can call an AI model to extract and clean the metadata before enrichment. This is the recommended setup for any heavy SoundCloud usage.

**What AI cleanup does:**

1. Parses the artist name out of the title when they are concatenated
2. Separates featured artists from the title into the artists field
3. Removes catalog numbers, release tags, and platform credits from titles
4. Normalises capitalisation and punctuation
5. Expands non-standard collaboration formats (`b2b`, `vs`, `&`, `Ft.`) into a consistent list

**What AI cleanup does not do:**

- It does not look up the track on any external database
- It does not guess the genre or album
- It only processes the raw title and artist strings provided by SoundCloud

### Enabling AI cleanup

See [guides/ai-metadata.md](ai-metadata.md) for the full setup. The short version:

```toml
[ai]
enabled = true
provider_order = ["ollama"]  # or ["openrouter"], or both as fallback

[ai.ollama]
model = "qwen2.5:7b"  # any model with reliable JSON output
```

When AI is enabled, every SoundCloud source batch is cleaned before the enrichment step. The cleaned values are what get enriched, tagged, and saved — not the raw SoundCloud strings.

When AI is disabled, metadata passes through uncleaned. The enrichment step (MusicBrainz, Bandcamp) will still try to find a match, but noisy titles lower the confidence score and increase the number of tracks flagged for manual validation.

## Connecting a SoundCloud account

Anonymous downloads top out at the public stream, usually around 160 kbps AAC. Connecting your own session unlocks uploader-provided originals (often FLAC) and age or region gated tracks.

There is no OAuth flow to offer. SoundCloud stopped issuing API credentials to new applications in 2021, so no self-hosted app can register one. What yt-dlp accepts, and therefore what Soundome asks for, is the `oauth_token` cookie from your own logged-in browser session.

**To connect:**

1. Log in to soundcloud.com in your browser.
2. Open devtools, then Application or Storage, then Cookies, then `soundcloud.com`.
3. Copy the value of the `oauth_token` cookie.
4. In Soundome, open **Tools**, then the **Providers** tab, paste the value, and click **Connect**.

Soundome verifies the token against SoundCloud before storing it, so a bad paste fails immediately rather than silently degrading later downloads. The token is written to `soundcloud_cookies.txt` next to the database, with owner-only permissions, and passed to yt-dlp as `--cookies` on every download.

The connection state is re-verified on each visit to the Providers tab. If you log out of that browser session, SoundCloud invalidates the token and the card shows as disconnected again.

**Notes and limits:**

- Availability of a lossless original is per track. It depends on the uploader enabling downloads and on the track still having downloads left, so connecting raises the ceiling rather than guaranteeing FLAC.
- An explicit `downloader.cookies_file` in `config.toml` takes precedence. Set it when you want to supply a full browser cookie jar instead.
- The token is a live session credential. Treat the file like a password and keep it out of version control.

## Syncing your likes

Once connected, **Tools**, then **Providers**, offers **Sync my likes**. The same thing happens if you paste `https://soundcloud.com/you/likes` into the Download box.

Your likes are treated as a playlist named `SoundCloud Likes`. That is not cosmetic: it means the whole existing pipeline applies unchanged. The sync runs as a background task with live progress, ordering is preserved, the playlist is exported to M3U8 like any other, and you can put it on a schedule from the Sync tab to keep it current.

Behaviour worth knowing:

- **Authentication is required.** The likes feed returns 401 to anonymous clients, so this only works with a connected account.
- **Liked playlists are skipped.** The feed mixes liked tracks and liked playlists. Expanding the latter would pull in hundreds of tracks you never liked individually, so only track likes are synced. The count of skipped playlists is logged.
- **The total will not match your profile count.** SoundCloud counts likes whose track has since been deleted or made private, and no longer serves them. An account showing 667 likes fetched 594 real tracks and 8 liked playlists in testing.
- **Re-syncing is cheap.** Tracks already in your library are linked to the playlist and skipped rather than downloaded again, so a scheduled sync only fetches what is new.

### Browsing likes before downloading

The **Likes** page lists everything you have liked without downloading or persisting anything. Each row has a play button that previews the track and a download button that fetches just that one.

Preview audio comes straight from SoundCloud's 128 kbps progressive stream, not from your library. The server only resolves a short-lived signed CDN URL and the browser plays it directly, so no audio passes through Soundome. Those URLs expire, and the player silently re-resolves once before reporting a failure.

Downloading from a row does not interrupt playback, so you can keep listening while tracks queue up. Use **Sync my likes** on the Providers tab instead when you want all of them.

## DRM-protected tracks

Some SoundCloud tracks are protected and cannot be downloaded by `scdl`. When this happens, Soundome first attempts an automatic recovery before falling back to manual validation:

1. **Automatic Spotify-match retry.** If the track already carries a Spotify metadata reference (for example attached during MusicBrainz/Spotify enrichment, which runs before the download attempt), Soundome automatically retries the download through the same Spotify → YouTube/YouTube Music matching flow used for Spotify-sourced tracks. The track's source stays SoundCloud; only the resolved provider (YouTube or YouTube Music) and the staged file are affected. When this succeeds, the track proceeds through the normal pipeline (tag, organize, persist) with no manual step required.
2. **Manual fallback.** If no Spotify metadata reference is available, or the automatic retry also fails, the track falls back to manual validation as before:
   - The track is saved to your database as pending validation
   - The **Validations** page shows it with the reason `soundcloud_drm_protected`
   - The audio file has not been downloaded yet — there is no staged file

**To resolve a DRM-protected track manually:**

1. Open the **Validations** page.
2. Find the track with reason `soundcloud_drm_protected`.
3. Click **Show YouTube candidates** — Soundome searches both YouTube and YouTube Music for matching results.
4. Select the correct match from the list.
5. Click **Approve** — Soundome downloads the audio from the selected YouTube URL, tags it with the existing metadata, and finalises the track.

If no YouTube candidate matches, you can also paste any YouTube or YouTube Music URL manually into the provider URL field before approving.

## Playlist sync with noisy metadata

When syncing a large SoundCloud playlist (hundreds of tracks), AI cleanup processes titles in batches of 50 before each track is enriched. If the AI backend is slow or unavailable, the batch falls back to the raw values and continues rather than failing the whole sync.

Expect some tracks to still land in the validation queue even with AI enabled, particularly for:

- Very short titles that match multiple unrelated tracks
- Releases with unusual capitalisation or non-Latin scripts
- Brand-new tracks not yet indexed by MusicBrainz or Bandcamp

These can be resolved from the Validations page at any time.

## Proxy support

SoundCloud requests honour the shared proxy configuration. If you have `[proxy]` configured with `enabled = true`, SoundCloud fetcher and downloader requests will be routed through it.

```toml
[proxy]
enabled = true
urls = ["socks5://user:pass@your-proxy:1080"]
```

See [../operations/proxy-configuration.md](../operations/proxy-configuration.md) for full proxy setup details.

## Troubleshooting

**Track shows `soundcloud_drm_protected` but I can listen to it on SoundCloud normally**
→ Some tracks are available to stream via the browser but not to download via the API. This is a SoundCloud platform restriction. Use the YouTube candidate flow described above.

**Many SoundCloud tracks end up in the validation queue even with AI enabled**
→ This is expected for very noisy metadata. AI cleanup improves the situation significantly but does not guarantee exact MusicBrainz matches. Review the pending tracks in the Validations page and approve or adjust them.

**SoundCloud URL is rejected as invalid**
→ Paste the canonical URL without query parameters. URLs with `?si=` or `?utm_source=` are stripped automatically, but very unusual URL shapes (redirect links, mobile app share URLs) may not be recognised. Copy the URL directly from the SoundCloud web player address bar.
