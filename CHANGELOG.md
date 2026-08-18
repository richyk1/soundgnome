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
- **Built-in equalizer.** A 10-band graphic EQ (plus preamp) in the player,
  built on the Web Audio API, so you no longer need a browser EQ extension.
  Opt-in per the on/off toggle, with presets (Bass boost, Vocal, Loudness, …),
  per-band ±12 dB sliders, reset, and settings persisted across reloads. It
  routes the library audio (same-origin) through a `BiquadFilter` chain; default
  playback is untouched until you enable it.
- **Missing files view + re-sync.** A `Tools -> Missing files` tab lists library
  tracks whose audio file has gone from disk (the database keeps a track's
  metadata independently of its file). Each can be re-synced: the track is
  re-downloaded from its original source and re-filed in place, keeping its
  identity, or a clear error is shown when the source can no longer supply audio.
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
- **Player state persists across reloads.** The queue, the current track, its
  playback position, and volume/shuffle/repeat are saved to local storage, so a
  page reload restores the bar and queue instead of clearing them. The track is
  reloaded seeked to where you left off (paused, since browsers block autoplay
  without a gesture); the audio URL is re-resolved rather than persisted.
- **Artwork embedding + backfill.** Cover art is embedded into the audio file at
  tag time, plus a one-shot backfill endpoint and Storage-page button that
  embeds artwork into every existing library file in place (offline-safe).
- **YouTube 256k requirement toggle** (`downloader.youtube_require_256k`): fall
  back to the next best audio when no 256k AAC master exists, or fail strictly.
- **Browser file & folder upload for ingestion.** The Ingest page can now upload
  audio files (or whole folders, preserving structure) straight from the browser
  into a per-session staging area, with bounded-concurrency uploads, per-file
  progress, and an aggregate result (added / duplicate / to-review). Uploads are
  then ingested server-side and the session folder is cleaned up on success.
- **Exact-duplicate ingest skip.** Ingest now hashes each file (SHA-256) and
  skips byte-identical re-uploads before enrichment, so re-pushing songs already
  in the library is sorted out cheaply. The hash is stored as a track reference
  (no schema migration); metadata-based dedup still handles non-identical copies.
- **Acoustic-fingerprint deduplication.** Ingest computes a
  [Chromaprint](https://acoustid.org/chromaprint) acoustic fingerprint (pure-Rust
  [rusty-chromaprint](https://crates.io/crates/rusty-chromaprint), decoded through
  the bundled ffmpeg) and, before enrichment, matches it against stored
  fingerprints of tracks with a comparable duration. This catches the same
  recording re-encoded to a different bitrate or format, which the exact hash and
  (for weakly-tagged files) the title/artist tier both miss. Fingerprints are
  stored as track references, so there is no schema migration and the match is a
  narrow, offline, in-process comparison.
  On a match the higher-quality copy is kept: a better upload replaces the
  existing library file (the superseded file is deleted) and a worse one is
  discarded, so a duplicate never lands in the review queue.
- **Library maintenance tabs with live progress.** The fingerprint and artwork
  backfills each have their own tab under Tools (Fingerprints, Artwork) that runs
  the pass as a tracked background task and shows a live progress bar, processed
  count, and running result tallies, with a persisted last-run summary. The
  fingerprint backfill stores acoustic fingerprints for existing library files
  that predate fingerprinting, so re-uploads of songs already in the library are
  recognized even when their tags differ; both passes are idempotent.

### Changed

- **Validations page redesigned (flatter, fewer alerts).** The nested "card in a
  card in a card" layout is gone: each track is a flat row with its candidates
  listed directly beneath, and matches load automatically as you scroll instead
  of behind a "Show matches" toggle. Approve / Reject / Select are now stateful
  buttons that carry their own loading, success, and error states (spinner, then
  a check or a shake), so a failure is shown on the button plus one concise inline
  line rather than a page-level alert box. The three tinted tab callouts collapsed
  into a single muted hint.
- Web layout now uses the full viewport width (removed the centered `max-width`
  caps on the shell and the library views).
- Rebranded from Soundome to **Soundgnome**: application name, the `SOUNDGNOME__`
  environment-variable prefix, package/crate names, and the published Docker
  image (`ghcr.io/richyk1/soundgnome`).

### Fixed

- **Organizer no longer deletes a file it then fails to move.** `move_track_file`
  removed any existing destination *before* renaming the new file into place, so a
  re-organize whose source and destination resolved to the same path (or whose
  rename then failed) deleted the only copy and left the database pointing at an
  empty path. It now relies on `rename`'s atomic replace and skips a no-op move,
  so a re-organize can no longer lose the audio. Re-syncing an existing track also
  now repairs it when its file is missing instead of keeping the broken row.
- **Finalized library tracks no longer get dragged back into validation.**
  Re-syncing a source already in the library re-derived a partial/no-match from
  the raw source metadata and folded `needs_validation` onto the finalized row,
  so a track you had already approved reappeared in the queue (and, if its file
  had since been deleted, as a dead-end). A track with a `soundome_id` is a
  finalized, user-reviewed entry: re-syncs now leave it alone (the freshly staged
  copy is discarded), and a data migration clears the flag on already-affected
  rows. Metadata lives in the database independently of the audio file, so a
  deleted file leaves the library row intact until re-scanned or re-ingested.
- **DRM-blocked SoundCloud tracks are no longer stranded under the wrong tab.**
  When a SoundCloud download was DRM-blocked *and* its metadata was a weak match,
  the track kept the metadata reason (`Partial match` / `No match`) instead of
  `soundcloud_drm_protected`, because the DRM reason was only set when the track
  was not already flagged for validation. These tracks have no audio file, so
  selecting a metadata candidate there failed with "no staged file and no
  provider_url". DRM now takes precedence, routing them to the `Errors` tab where
  a YouTube source can be picked to fetch the audio; a data migration re-buckets
  the previously stranded rows.
- **Validations `Errors` tab no longer fires a burst of YouTube searches.** Each
  DRM row auto-loaded its YouTube candidates on scroll, and each lookup spawns
  several `yt-dlp` subprocesses; on a tab with many DRM tracks this saturated the
  server and rows spun indefinitely. YouTube candidates now load on demand via a
  "Find YouTube sources" button (the cheap MusicBrainz auto-load is unchanged).
- **Validation Select/Approve errors are surfaced.** A failed approve or candidate
  Select (e.g. the track's audio file is missing) was swallowed as an unhandled
  promise rejection, so the button appeared to do nothing. The Validations page
  now shows the error in a dismissible banner and keeps the card in place.
- **Tracks page loads fast.** The library list returned every track with all of
  its references embedded, including the ~8 KB acoustic-fingerprint blobs, so the
  payload ballooned to ~19 MB for ~1.5k tracks. Internal `soundome:` references
  (fingerprint, content hash) are now excluded from API responses (they are dedup
  bookkeeping, not user-facing links), cutting the response to ~1.4 MB. The
  library list queries were also de-N+1'd (four bulk queries instead of `1 + 3N`),
  and the audio-quality cache is warmed in the background at startup so the first
  load no longer probes every file on the request path. First render dropped from
  ~12 s to well under a second.
- **Validations page loads fast.** `get_pending_validations` hydrated each track
  with its own album/artists/references queries (a `1 + 3N` fan-out, thousands of
  serial queries for a large review queue). It now uses four bulk queries total,
  so the page loads quickly regardless of queue size.
- **No more duplicate match candidates.** MusicBrainz returns the same recording
  once per release, which surfaced as visually identical candidate cards. Matches
  are now deduplicated (by title/artist/album/date/duration), keeping the highest
  score.
- **Clear error when a track's audio file is missing.** Selecting a candidate for
  a track whose file is gone (or whose organized `./library/...` path needed
  resolving) returned a raw `Mp4TagError(NotFound)`. The Select/Approve flow now
  resolves relative library paths against the library dir and reports a plain
  "audio file is missing on disk" message instead.
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
