# Web admin interface

Soundgnome ships a lightweight web admin panel served directly by the Rocket server. It covers the full day-to-day management workflow: submitting downloads, reviewing pending validations, browsing and editing the library, monitoring background tasks, and managing scheduled syncs.

## Architecture

| Layer | Technology | Location |
|---|---|---|
| Backend API | Rocket 0.5 + `rocket_okapi` | `apps/server/src/routes/` |
| Frontend | Svelte 5 + Vite + TypeScript | `apps/web/` |
| Build output | Static files served by Rocket | `data/web/` |

Rocket mounts the compiled SPA at `/` via `FileServer::from("data/web")` and exposes all API endpoints under `/api`.

## Running

```bash
# Development (Rocket + Vite with HMR)
pnpm dev

# Build only the frontend
pnpm web:build
```

Build in the checkout whose `data/web/` the running Rocket process serves. Building
another checkout does not update that server's website.

## Mobile layout and playback

- Phones use a bottom tab bar with Home, Search, Library, and **More**: icon-only
  pixel tabs (each labelled for screen readers and as a tooltip) and a violet
  tick on the active tab. A floating player card sits above it only when a track
  is loaded, including while paused. Tap the artwork/title for full Now Playing
  controls: the card morphs into the sheet and its thumbnail into the big cover
  (View Transition API, Safari 18+ and Chromium; otherwise the sheet slides).
  The card keeps Play/Pause within reach. More opens a bottom sheet and restores
  focus to its trigger on close.
- The tab bar and player card are opaque rather than a translucent blur layer,
  so the full-screen Now Playing sheet keeps its viewport positioning and
  scrolling large grids adds no compositing work. Forced colors keep a readable
  native selected state.
- The Library title row and section tabs stay pinned while lists scroll. Phone
  track rows are single, cozy 52px rows with no Edit or like/dislike controls:
  rate the playing track from Now Playing, and edit track info from a desktop
  browser. The Disliked list keeps its Delete button on phones.
- Albums, tracks, artists, and playlists without artwork show a generated pixel
  sprite seeded by the item (`apps/web/src/lib/pixel-art.ts`), so an album keeps
  the same cover everywhere it appears. Real artwork replaces it when available.
- The visual system (tokens, type, icons, mark, components) is documented in
  `apps/web/DESIGN.md`; product context for design work lives in
  `apps/web/PRODUCT.md`.
- Page spacing, form sizing, and touch targets are shared across the interface;
  controls are at least 44px tall and text inputs use 16px text to avoid Safari's
  automatic input zoom.
- The phone Library keeps four collection tabs in one row of underline tabs,
  with a **Liked** heart and Refresh icon in the header. Search stays visible; the
  adjacent **Sort and view options** button reveals secondary controls.
- Each card's actions button opens an actions sheet with labeled 48px rows for editing,
  selection, ratings, or deletion as applicable. Sheets mount only when opened,
  restore focus on close, and close before handing off to the Edit dialog.
- Navigation uses [same-document view transitions](https://developer.mozilla.org/en-US/docs/Web/API/Document/startViewTransition)
  when available and a short main-content fade otherwise. Snapshots are clipped
  above the dock so player controls remain visible and interactive.
  System **Reduce Motion** disables navigation, tab-indicator, and sheet movement;
  scrolling and virtual-card mounts are not animated.
- The shell and dialogs follow the visible viewport, including Safari toolbar
  and keyboard changes, and respect device safe-area insets. During keyboard
  editing, obstructing footer and selection controls are hidden without removing
  the audio element. Search retains the focused field's scroll position.
- Album and artist grids render only visible rows plus overscan. Narrow phone
  grids use two columns, and floating selection actions leave clearance below
  the results. Hover and selection updates do not reset the scroll position.
- Themes follow the device's local clock: light from **07:00–18:59**, dark from
  **19:00–06:59**. The next boundary is scheduled automatically; returning to the
  page rechecks the clock.
- iOS uses native audio playback to avoid routing background audio through Web
  Audio. EQ and volume normalization are unavailable on iOS; saved preferences
  remain intact, and desktop audio processing is retained. Use device volume
  controls on iOS.
- The single app-wide player survives page navigation. [Media Session](https://www.w3.org/TR/mediasession/)
  supplies title, artist, artwork, playback state, position, and play/pause,
  previous/next, and seeking actions using the same queue as the website.
- Start playback in the foreground if Safari requests a user gesture. A blocked
  play attempt stays paused and offers Play rather than claiming playback started.
  Calls, other audio, browser eviction, or force-closing the app can still interrupt
  playback; background and lock-screen behavior must be checked on a real iPhone.
- Scrolling performance and pixel-icon sharpness still require verification on an
  actual iPhone; Chromium with a phone viewport or iPhone user agent is not Safari.

Opening from the home screen uses the same website in standalone mode, without
Safari's address and navigation bars. It is not a native app and does not bypass
iOS background restrictions. Local queue/preferences may be separate from Safari.
Service-worker caching and offline PWA support require HTTPS.


## Pages

### Download (Home)

- URL input form accepting Spotify, SoundCloud, YouTube, and YouTube Music links
- On phones, the URL field and primary Download button each have their own full-width row.
- Track-versus-playlist detection based on URL patterns
- Inline download result feedback: success banner with track title and artists, or error message
- Tracks flagged `needs_validation` are highlighted with a `review` badge
- Recent-downloads list, refreshed automatically after each submission

### Library

Tabbed view for browsing and editing the entire library.

**Tabs:** Artists · Albums · Tracks · Playlists, with a **Liked** shortcut.

**Artists tab**

- List or grid view; toggle with the view buttons in the toolbar
- Search by name (`S` to focus)
- **Similar filter** — highlights artists whose names differ by only a few characters, useful for spotting duplicates
- Click an artist to drill into their albums and tracks
- **Multi-select**: open a card's **…** menu and tap **Select**, or use `Shift`+click. Selecting two or more artists exposes the floating **Merge** action.

**Albums tab**

- Grid or list view; search by title or artist
- Click an album to see its tracks
- Open an album card's **…** menu and tap **Select**, or use `Shift`+click, to select albums for merging.

**Tracks tab**

- Filterable by status: All / Needs review / Lossless / Liked
- Search by title or artist
- Edit or delete individual tracks

**Playlists tab**

- Lists all synced playlists
- Click a playlist to see its tracks

**Drill navigation**

- Breadcrumb bar shows the current path (e.g. Artists › Artist name › Album title)
- `Backspace` navigates up one level
- Browser back/forward buttons are supported (hash-based routing)

**Edit modal**

- Available for tracks, albums, and artists
- Supports cover/photo upload by file picker, drag-and-drop, or a direct image URL (albums/artists)
- Albums and artists have a **Fetch from references** button next to the image URL field: it re-queries whichever of the entity's existing references point to a supported provider (Spotify, SoundCloud, YouTube Music) and, on the first match, saves the returned image as the cover/icon. Disabled when there are no references yet; shows an error if none of them resolve to an image.
- **References panel**: add a reference by picking a `ref_type` (Source/Provider/Metadata/Reference) and pasting a link — the platform and external id are inferred automatically from the URL. An **Advanced** disclosure allows overriding the inferred platform or setting an id-only reference (no URL), for platforms whose URLs don't embed a stable id (SoundCloud, Bandcamp).
- `Enter` saves, `Esc` cancels

### Validations

Lists all tracks flagged `needs_validation = true`. These are tracks whose metadata could not be matched with sufficient confidence during the download pipeline.

Each card shows:

- Cover, title, artists, album, genre, date, duration, and track/disc numbers
- `validation_reason` explaining why the track was held back
- Staged file path (the audio file already downloaded to the temp directory)

**Available actions:**

| Action | Behavior |
|---|---|
| **Edit** | Open inline metadata form (fields: title, artists, album, genre, date, track #, disc #, label) |
| **Show matches** | Fetch alternative metadata candidates from providers (shown when reason is `metadata_partial_match`) |
| **Select** (on a candidate) | Apply that candidate's metadata directly and approve |
| **Approve** | Apply any edits, tag the staged file, move it to the library, clear the validation flag |
| **Reject** | Delete the track row and its staged file from the database |

**Search:** filter by title, artist, album, or validation reason.

### Tasks

Background job monitor (playlist syncs, downloads).

- Auto-refreshes every 3 seconds
- Each task shows: label, status badge, progress bar (Running/Completed), error message, last update time
- **Retry** available for Pending, Running, Failed, and Cancelled tasks
- **Cancel** available for Running and Pending tasks

### Sync Schedules

Configure playlists to sync automatically at a fixed interval.

- Add a schedule: playlist URL, optional label, interval in minutes
- Per-schedule actions: **Pause / Resume**, **Sync now** (triggers an immediate background task), **Delete**
- Shows last-run and next-run timestamps

## Keyboard shortcuts

### Library

| Key | Action |
|---|---|
| `S` | Focus the search field |
| `E` | Edit the item under the cursor |
| `Backspace` | Go up one level (album → artist → list) |
| `Shift`+click | Select an artist for merge |
| `M` | Start merge (requires ≥ 2 artists selected) |
| `Esc` | Cancel merge picking / clear selection |

### Validations (TrackCard)

| Key | Action |
|---|---|
| `E` (while hovering a card) | Open inline edit form |
| `Esc` | Close the inline edit form |

### Edit modal

| Key | Action |
|---|---|
| `Enter` | Save changes |
| `Esc` | Cancel without saving |

### Global

| Key | Action |
|---|---|
| `?` | Open / close the help panel |

## Validation workflow

When metadata enrichment returns a partial or no match, the track is saved for manual review. The audio file has already been downloaded to staging at this point, so no re-download is needed on approval.

```
fetch metadata
    ↓
metadata enrichment (MusicBrainz, Bandcamp, Spotify…)
    ↓
download audio → staging (temp_download_dir/)
    ↓
Confident match?
 ├── yes → tag + move to library → done
 └── no  → save to DB with needs_validation=true
                    ↓
            User reviews via Validations page
                    ↓
            (optional) edit metadata or pick a candidate
                    ↓
            Approve → tag staged file + move to library
                   or
            Reject  → delete from DB
```

## API routes

All routes are documented interactively at `/swagger`.

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/download` | Submit a track or playlist URL |
| `GET` | `/api/tracks` | List all library tracks |
| `GET` | `/api/tracks/recent?limit=N` | List the most recent tracks |
| `GET` | `/api/albums` | List all albums |
| `GET` | `/api/artists` | List all artists |
| `GET` | `/api/playlists` | List all playlists |
| `GET` | `/api/validations` | List tracks pending validation |
| `PATCH` | `/api/validations/:id` | Approve and finalize a pending track |
| `DELETE` | `/api/validations/:id` | Reject and delete a pending track |
| `GET` | `/api/tasks` | List background tasks |
| `POST` | `/api/tasks/:id/retry` | Retry a task |
| `POST` | `/api/tasks/:id/cancel` | Cancel a task |
| `GET` | `/api/sync-schedules` | List sync schedules |
| `POST` | `/api/sync-schedules` | Create a sync schedule |
| `PATCH` | `/api/sync-schedules/:id` | Update a sync schedule |
| `DELETE` | `/api/sync-schedules/:id` | Delete a sync schedule |
| `POST` | `/api/sync-schedules/:id/trigger` | Trigger a sync immediately |
| `GET` | `/metrics` | Prometheus metrics (tracks, albums, artists, playlists, tasks by status) |

## Frontend file structure

```
apps/web/
├── src/
│   ├── App.svelte              # nav, routing, help modal trigger
│   ├── app.css
│   ├── main.ts
│   ├── lib/
│   │   ├── api.ts              # typed API client
│   │   ├── types.ts            # shared DTO types
│   │   ├── HelpModal.svelte    # keyboard shortcuts & page guide popup
│   │   ├── TrackCard.svelte    # validation card with inline edit
│   │   └── library/
│   │       ├── store.svelte.ts # reactive library state
│   │       ├── ArtistTab.svelte
│   │       ├── AlbumTab.svelte
│   │       ├── TracksTab.svelte
│   │       ├── PlaylistsTab.svelte
│   │       ├── TrackTable.svelte
│   │       └── EditModal.svelte
│   └── pages/
│       ├── Home.svelte
│       ├── Library.svelte
│       ├── Validations.svelte
│       ├── Tasks.svelte
│       └── SyncSchedules.svelte
├── vite.config.ts
└── package.json
```
