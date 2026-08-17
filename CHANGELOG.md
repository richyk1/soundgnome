# Changelog

All notable changes to Soundgnome are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Soundgnome is a fork of [Soundome](https://github.com/barthofu/soundome) by
Bartholomé Gili. It diverged from Soundome `v0.2.8`; everything below is what
changed after that point.

## [0.3.0] - 2026-08-17

First Soundgnome release. Continues the version line from the inherited Soundome history (v0.1.0-v0.2.8 already exist upstream).

### Added

- **Direct Spotify audio downloads** via [librespot](https://github.com/librespot-org/librespot).
  A connected Spotify **Premium** session now streams and decrypts the track
  audio directly, instead of always matching the track on YouTube. YouTube /
  YouTube Music remain the fallback when no Spotify session is available.
- **Spotify library sync.** App-credential (public catalogue) auth plus a
  per-user authorization flow (PKCE) to read the signed-in user's Liked Songs
  and private playlists, with the display name and liked songs cached.
- **Spotify-style web interface (full redesign).** Three-pane app shell
  (sidebar navigation + "Your library" sub-nav + play queue, main content, and a
  full-width bottom player), Archivo typography, a Lineicons icon set, and a
  violet-on-black theme.
- **Global, persistent audio player.** One player mounted in the app shell so
  playback and the player bar survive navigation between pages. Includes queue,
  shuffle, previous/next, repeat, seek, and volume; a real waveform computed
  from local audio; and on-demand artwork resolution (Spotify oEmbed / YouTube
  thumbnail) when a track has no stored cover.
- **Click-to-play track rows.** The whole track row is the play control; the `#`
  column doubles as the play/pause affordance. The per-row play button was
  removed.
- **Artwork embedding + backfill.** Cover art is embedded into the audio file at
  tag time, plus a one-shot backfill endpoint and Storage-page button that
  embeds artwork into every existing library file in place (offline-safe).
- **YouTube 256k requirement toggle** (`downloader.youtube_require_256k`): fall
  back to the next best audio when no 256k AAC master exists, or fail strictly.

### Changed

- Web layout now uses the full viewport width (removed the centered `max-width`
  caps on the shell and the library views).
- Rebranded from Soundome to **Soundgnome**: application name, the `SOUNDGNOME__`
  environment-variable prefix, package/crate names, and the published Docker
  image (`ghcr.io/richyk1/soundgnome`).

### Fixed

- Dedup "replace" path no longer fails with `ENOENT` when the previously-staged
  file moved; the freshly-downloaded file's real path is honored on move.
- Per-track download failures surface the real reason (e.g. "Requested format is
  not available") instead of a generic "process error".
- Retrying a task that finished **completed-with-errors** now re-runs and
  re-attempts the errored tracks (previously only failed/interrupted tasks
  could retry).
- `dev-sync` runtime-directory excludes are anchored to the repo root, so the
  `apps/web/src/lib/library/` source directory is no longer skipped by the
  `library/` exclude.

### Compatibility

- The `soundome_id` track identifier (database column and the ID embedded in
  audio-file tags) is intentionally **unchanged**, so Soundgnome reads libraries
  previously organized by Soundome without a migration or re-tag.
- The environment-variable prefix changed from `SOUNDOME__` to `SOUNDGNOME__`,
  and the default database filename from `soundome.db` to `soundgnome.db`.
  Existing deployments must update their environment/compose and either rename
  the database file or point `SOUNDGNOME__DATABASE__URL` at the existing one.

[0.3.0]: https://github.com/richyk1/soundgnome/releases/tag/v0.3.0
