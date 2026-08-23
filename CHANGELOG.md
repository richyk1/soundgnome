# Changelog

All notable changes to Soundgnome are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Soundgnome is a fork of [Soundome](https://github.com/barthofu/soundome) by
Bartholomé Gili. It diverged from Soundome `v0.2.8`; everything below is what
changed after that point.

## [0.3.0] - 2026-08-17

First Soundgnome release. Continues the version line from the inherited Soundome history (v0.1.0-v0.2.8 already exist upstream).

### Added

- **Cover thumbnails for snappy lists.** `GET /api/tracks/<id>/cover` now serves a
  small cached 128px JPEG by default instead of the raw embedded artwork (often
  1-5 MB each), cutting a full list of covers from ~110 MB to under 1 MB so they
  appear instantly. The full-screen now-playing art requests `?size=large`
  (512px), and `?size=full` still returns the untouched original. Thumbnails are
  cached on disk (keyed by file mtime) and precomputed for the whole library by a
  gentle background backfill on startup, mirroring the waveform pipeline.

- **Looser duplicate cleanup.** `POST /api/library/dedupe?loose=true` also merges
  same-title/same-artist copies whose durations are within 5 s even when the
  acoustic fingerprint cannot confirm them (different masters of the same track),
  still keeping the best complete copy. Different versions (durations more than
  5 s apart) are left untouched.

- **Disliked tracks are skipped automatically.** Disliking the track you're
  playing (from the player bar, the Now Playing sheet, or its row) jumps to the
  next one, and next/previous/auto-advance now step over disliked tracks entirely
  - so a thumbs-down means you won't hear it again while it stays disliked.
  Explicitly clicking a disliked track still plays it, and if every remaining
  track is disliked the current one keeps playing.

- **Clean track metadata with AI from the edit panel.** A "Clean title & artists
  with AI" button in the track editor sends the current title and artists to the
  configured AI backend (the same OpenRouter/LiteLLM-compatible cleanup used for
  SoundCloud imports) and fills the form with the suggestion for you to review
  before saving. It strips noise (`[FREE DL]`, `[Original Mix]`, bitrate tags),
  extracts featured artists, disambiguates `Artist - Title`, and preserves remix
  credits, all with strict no-hallucination rules. Backed by a new
  `POST /api/tracks/<id>/ai-clean` endpoint; nothing is persisted until you save,
  and it reports clearly when AI is not configured. The same button is on each
  Validations card, so messy no-match imports (e.g. SoundCloud flips with no
  MusicBrainz match) can be cleaned and approved without hand-editing.

- **Like / dislike from the desktop player bar.** The now-playing track can be
  rated straight from the bar, beside the title, instead of only from track rows
  or the mobile Now Playing sheet. When a Last.fm account is connected, liking a
  track also loves it on Last.fm (`track.love`); clearing the like or disliking
  removes the love. Best-effort and server-side, so it never blocks the rating.

- **Mobile is now a listening app.** On phones the desktop cockpit gives way to a
  listening-first shell: a bottom tab bar (Home / Search / Library) with the
  curation tools (Ingest, Validations, Activity, Tools) tucked behind a **More**
  sheet. Tapping the player bar opens a full-screen **Now Playing** with large
  art, a waveform scrubber, full transport, like/dislike, EQ, volume, and the
  Up Next queue; swipe down to dismiss. Track lists collapse from the desktop
  table into tap-to-play rows, and desktop-only chrome (page headers, sort/view
  toggles, batch tools) is hidden. The desktop layout is unchanged.
- **Like / dislike your library, with a Liked page to match.** Every track row
  and card now has thumbs up/down. The sidebar **Liked** page is now a two-tab
  view of your own library - **Liked** and **Disliked** - instead of listing your
  SoundCloud likes. The Disliked tab doubles as a cleanup queue: remove tracks
  there one by one or with **Delete all**.
- **Native-feel PWA.** Media Session integration puts the current track (title,
  artist, artwork) on the lock screen and notification shade and wires up OS,
  headphone, and Bluetooth transport controls (play/pause/next/prev/seek) plus
  background playback. Added an in-app **Install** button (Android/desktop) with
  an iOS "Add to Home Screen" hint, safe-area insets so content clears the notch
  and home bar, a dark launch splash (was white), and touch tuning (no tap-flash,
  no pull-to-refresh, no double-tap zoom).
- **Last.fm scrobbling.** Connect a Last.fm account under `Tools -> Providers`
  (paste an API key + shared secret, then authorize) and everything you play is
  scrobbled: now-playing on start and a scrobble once a track passes half its
  length or 4 minutes (Last.fm's rule). The shared secret stays server-side (all
  `api_sig` signing happens there); failed sends are queued locally and retried,
  and scrobbling has its own on/off toggle.
- **Direct Spotify audio downloads** via [librespot](https://github.com/librespot-org/librespot).
  A connected Spotify **Premium** session now streams and decrypts the track
  audio directly, instead of always matching the track on YouTube. YouTube /
  YouTube Music remain the fallback when no Spotify session is available.
- **Built-in equalizer.** A parametric EQ in the player, built on the Web Audio
  API, so you no longer need a browser EQ extension. It shows a live
  frequency-response curve and offers manual graphic presets (Bass boost, Vocal,
  Loudness, …) with per-band ±12 dB sliders and a preamp, plus **device
  correction** presets — including a calibrated **AirPods Pro 2** curve — applied
  faithfully (per-band frequency + gain + Q). Opt-in, persisted across reloads.
  It routes the library audio (same-origin) through a `BiquadFilter` chain;
  default playback is untouched until you enable it.
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

- **Feels like an app, not a web page: no stray focus rings, Space always plays.**
  Interactive controls (buttons, cards, the waveform) no longer show a focus
  outline on click or Tab - keyboard use goes through the documented shortcuts
  (press `?`), not Tab traversal, so the ring was only accidental noise; text
  inputs keep their accent focus style. `Space` is now a true global play/pause
  media key: it never scrolls the page and never activates a stray focused control
  (previously it scrolled the library list when nothing was playing, and could
  trigger an accidentally-focused button). `Enter` still activates buttons.

- **Lower-fidelity player waveform.** The scrubber now draws chunky, rounded
  "voice-memo" bars (wider bars with clear gaps, pill caps) instead of a dense
  fine waveform - played bars in the accent, the rest in a muted grey.

- **Redesigned the Tracks page ("Stage").** The library header now leads with a
  `LIBRARY` eyebrow, a large title, and a live subtitle (track count · tracks
  needing validation · last sync). The Tracks toolbar gains filter pills
  (All / Needs review / Lossless / Liked) and inline sort tabs. Track rows are
  artwork-led: cover thumbnail, index, title with an amber validation dot,
  artist · album/genre, mono format + duration, and always-visible like/dislike
  with Edit/Delete revealed on hover. On phones it is an artwork list with the
  mini-player and bottom tabs. Palette and fonts are unchanged.

- **Much faster library ingest.** Batch ingest now prepares files (tag read, hash,
  acoustic fingerprint) across multiple CPU cores instead of one at a time, caps
  fingerprint decoding to the first 120 s of each track, and skips the
  rate-limited MusicBrainz/Spotify lookup for files whose tags are already
  complete (title + artist + album), finalizing them straight from their own tags.
  `general.ingest_concurrency` tunes the worker count (0 = auto). The database
  commit stays serial, so dedup order and results are unchanged.
- **Tracks page shows everything, with less chrome.** Dropped the
  All / Validated / Needs-review filter (the list now shows every track by
  default; the Validations page owns review) and removed the per-row Delete -
  deleting a track now lives in the Liked page's Disliked tab.
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

- **Dedup could delete a file two rows shared.** When two track rows pointed at
  one physical file (a duplicate row), removing one deleted the file and orphaned
  the survivor. The dedup delete step now skips removing a file that any other
  track row still references.

- **Duplicate songs in the library.** Two causes, both fixed. (1) The main Tracks
  list showed pending `needs_validation` tracks alongside finalized ones, so a
  failed/partial download (e.g. a truncated Spotify copy of a song you already own
  as FLAC) appeared as a lower-quality "duplicate". The list now shows finalized
  tracks only; pending ones stay in Validations. (2) A one-shot **library dedup**
  (`POST /api/library/dedupe`, dry-run by default) finds same-recording copies by
  Chromaprint fingerprint and keeps the best COMPLETE copy, removing the rest -
  cleaning up dupes that slipped past the pipeline. A truncated file never wins,
  even if it is lossless.
- **Quality comparison could replace a complete file with a truncated one.**
  `is_better_quality` now refuses a candidate that is materially shorter than the
  copy on disk, and the acoustic-dedup duration window widened from ±8 s to ±30 s.

- **Editing the playing track now updates the now-playing bar.** The bar showed a
  snapshot taken at play time, so an edit (e.g. AI cleanup) to the current track's
  title/artist did not appear until the next play. It now reads the live library
  track, so edits reflect immediately.
- **Shuffle reorders the tracks list instead of scroll-jumping.** Turning on
  shuffle now reshuffles the library list itself (with the current song pinned to
  the top), so the next track is simply the next row - the playing row advances
  one line at a time and the list gently keeps it in view. Turning shuffle off
  restores the normal sort. Previously the play order was hidden, so each shuffle
  advance flung the list to a random far-off song.

- **Initial page load no longer stalls fetching data twice.** On startup the app
  loaded the whole library (tracks, albums, artists, playlists) twice - once from
  the app shell and once from the Library page - so it downloaded and parsed
  several extra megabytes and briefly blanked the lists before refilling. It now
  loads once. Separately, the sidebar's "needs validation" badge was polling the
  full ~1.2 MB validations list every 5 seconds just to read its length; it now
  reuses the count already computed from the loaded tracks, so nothing is
  refetched on a timer.

- **Locally-ingested files now show their embedded cover art.** Ingest read the
  title/artist/genre tags but ignored the artwork embedded in the file, so a
  local import (e.g. a SoundCloud flip downloaded elsewhere) showed a blank
  note placeholder even when the audio carried a cover. Ingest now detects
  embedded artwork and serves it via a new `GET /api/tracks/<id>/cover`
  endpoint (no copy - the art is read straight from the file). The artwork
  backfill (`Tools -> Embed artwork`) picks up already-imported files the same
  way. The library also renders same-origin cover URLs, so uploaded and
  embedded covers display, not just remote `http(s)` artwork.

- **Tracks list was laggy with thousands of rows.** The whole list rendered at
  once, so scrolling and every filter/sort re-laid-out all ~3000 rows (a forced
  layout measured ~460 ms). Rows now use CSS `content-visibility`, so the browser
  skips layout and paint for off-screen rows (~20 ms - a ~23x drop) without a
  virtual-list library.
- **Player waveform took ~1 s to appear, flashing a plain line first.** The
  scrubber decoded the whole file at full sample rate (~890 ms) before it could
  draw. It now decodes at 8 kHz - plenty for a bar overview and roughly 3x faster
  (~340 ms) - and shows a skeleton waveform immediately instead of the bare line,
  so peaks fill in rather than snapping from a line to bars.
- **Waveform peaks are now precomputed server-side and cached, so the scrubber is
  instant.** The browser no longer fetches and decodes megabytes of audio just to
  draw the bars. A new `GET /api/tracks/<id>/waveform` endpoint decodes the file
  once with ffmpeg (8 kHz mono) into a ~4 KB peaks JSON, caches it on disk keyed
  by file mtime, and serves it with a long cache header (cold ~700 ms once, warm
  ~4 ms). A gentle background backfill precomputes the whole library after launch
  (one decode at a time, throttled) so even first play is instant without ever
  starving playback. The client also persists resolved
  peaks in IndexedDB, so replays after a reload draw from cache with zero network
  or decode (~30 ms). Falls back to client-side decoding if the endpoint is
  unavailable.
- **The waveform now animates in from its skeleton.** When peaks resolve, the
  flat placeholder bars grow up to their real heights in a quick left-to-right
  sweep (~450 ms, strong ease-out) instead of snapping, so the placeholder reads
  as the same waveform settling into focus. Respects `prefers-reduced-motion`
  (renders resolved immediately).

- **Enabling the equalizer mid-song silenced the current track.** Turning the EQ
  on builds the Web Audio graph, which creates the audio source on the
  already-playing element; Chrome leaves that resource on its old output path, so
  it went silent until you switched songs. Enabling the EQ now reloads the current
  track through the new graph, keeping its position and play state, so sound
  continues.

- **Reloaded track played silently with the equalizer on.** After a page reload,
  pressing play on the restored track produced no sound until you switched songs.
  The EQ's Web Audio graph was built lazily on first play; created that late on an
  already-loaded audio element, Chrome routes it into a dead `MediaElementSource`
  and the track is silent. The graph is now built when the restored track's `src`
  is set (as it already was for fresh plays), so the restored track resumes with
  full volume.

- **Browsers kept serving a stale build.** The static web app was served with no
  cache headers, so a browser could hold an old `index.html` that pointed at a
  superseded asset bundle and never pick up new builds (for example, missing the
  like/dislike buttons) without a manual hard refresh. The HTML shell is now sent
  `no-cache` so it revalidates every load, while the content-hashed `/assets/`
  are `immutable` - updates now land on their own.

- **Like / dislike were pushed off-screen in the Tracks list.** The track table
  sized itself to its content and overflowed its panel on typical laptop widths,
  so the rightmost Actions column (like, dislike, edit) sat past the right edge
  and needed horizontal scrolling to reach. The table now uses a fixed layout
  that always fits, truncating long titles/albums instead of growing wider, so
  the rating buttons are always visible.

- **Cover art was low resolution.** Embedded artwork used each source's default
  thumbnail: SoundCloud's 100x100 `-large`, YouTube's 480x360 `hqdefault`, and
  Spotify's 300x300 image. Cover fetching now requests the largest variant per
  host (SoundCloud 1080x1080, YouTube `maxresdefault`, YouTube Music 1200,
  Spotify 640) and falls back gracefully when it is unavailable. Run
  **Tools -> Artwork** to re-embed higher-resolution art into existing files.

- **Opus, Ogg, and WAV files failed to ingest.** Tag reading and writing went
  through a crate that only understands MP3, FLAC, and M4A, so every other
  container errored out during ingest, even though those formats were on the
  accepted list. Opus is yt-dlp's default audio format, so a large share of a
  YouTube-sourced library was affected. Tagging now uses `lofty`, which reads
  and writes tags uniformly across all ingested formats (including WAV);
  untagged files fall back to a filename-derived title and go to review instead
  of erroring, and partial-download artifacts (`*.temp.*`) are skipped up front.
- **Control characters in metadata could abort ingest.** A track, artist, or
  album name carrying a NUL or other control character (from noisy tags or a
  provider match) produced an illegal library path and failed the whole file at
  the folder-creation step. Path components are now stripped of control
  characters.
- **Upload sessions were deleted even when some files errored.** After a
  browser-upload ingest finished, the whole staging session was removed,
  including files that had errored and were never actually ingested. A run that
  ends with per-file errors now keeps its session so the failed files can be
  retried.

- **"Update available" button did nothing.** The service worker ships with
  `autoUpdate` (it skips waiting and claims the page itself), but the prompt's
  handler only messaged a non-existent *waiting* worker and then bailed. It now
  reloads to swap in the already-activated new build.
- **Mobile is now edge-to-edge.** The desktop "rounded panels floating on black"
  look left black bars framing the content on phones, and the header/drawer sat
  under the status bar. On phones the shell padding and panel radii are dropped so
  content fills the screen, the menu drawer is full-bleed, and the header, drawer,
  and player bar carry safe-area insets so nothing hides under the notch or home bar.
- **PWA update prompt was in French.** The "update available" banner had
  hardcoded French strings; it's now English ("Update available" / "Update" /
  "Later").
- **Mobile album/artist grid was cramped.** The card grid forced 3 tiny columns
  on phones (truncating titles); it now shows 2 roomy columns on phones and
  scales up on larger screens.
- **Shuffle now matches the shown queue.** In shuffle mode the Next button picked
  a fresh random track on every press, which never agreed with the "up next" list
  (and could replay tracks). Shuffle now builds a stable shuffled play order (the
  current track pinned first); Next/Previous step through it and the sidebar queue
  displays that exact order, so they always agree. The order persists across reloads.
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
