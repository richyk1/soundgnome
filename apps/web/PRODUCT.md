# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Open-source self-hosters who run Soundgnome (Docker image `ghcr.io/richyk1/soundgnome`) on their own machine to build and keep a personal music library. They manage the server themselves, then use the web app day to day from a phone (iPhone Safari and the Home Screen app are the primary scene) and from a desktop browser. The UI must make sense to someone who did not write it.

## Product Purpose

Soundgnome centralizes, downloads, tags, and organizes music from Spotify, SoundCloud, YouTube, YouTube Music, and local files into one library, and plays it back. Success: a user submits a source URL, gets a correctly tagged file in the library, avoids duplicate imports, and resolves ambiguous matches from the web UI without losing the staged audio.

## Positioning

The library stays a plain `Artist/Album/Track` folder tree the user owns, never a proprietary store. Every item keeps its provenance (source, provider, and metadata references), and uncertain matches wait for human review instead of being silently finalized. It is a fork of Soundome that adds direct Spotify downloads (librespot) and Spotify library sync.

## Operating Context

- Server runs headless (Docker or a home machine); the Rocket server serves the web app and API.
- Daily use is browsing, searching, and playing the library on a phone, pasting links to download, and clearing the validation queue.
- Libraries reach thousands of items (the maintainer's: 1,049 albums, 2,254 artists, 2,764 tracks).
- Most albums have no cover art (60 of 1,049 in the maintainer's library), so placeholder art is the dominant visual in the library.

## Capabilities and Constraints

- Surfaces: Home (download by URL, recent downloads), Search, Library (albums, artists, tracks, playlists; metadata editing; references), Liked/Disliked, Ingest (upload or scan local files), Validations (manual review), Activity (live task progress), Tools (sync schedules, storage, providers, artwork, fingerprints, missing files), audio player with EQ and waveform, PWA install and update prompts.
- Must stay fast on iPhone Safari at library scale: grids are virtualized; avoid per-item expensive compositing.
- Work in progress: some surfaces are partial; failure modes must stay explicit.
- Terminology: Source, Provider, Metadata, Reference; validation; ingest; sync.

## Brand Commitments

- Name: Soundgnome.
- Visual reference made binding by the user: Stencil (stencil.so), "retro yet professional, clean".
- Pixel-art icon set across the app.
- A new pixel gnome mark replaces the headphone logo and app icons.
- Theme follows local time: light 07:00–19:00, dark otherwise.

## Evidence on Hand

- Real library data through the API (albums, artists, tracks, playlists, references, tasks).
- No testimonials, customer logos, benchmarks, or usage statistics exist; do not invent them.

## Product Principles

1. Safe automation, explicit review: never hide uncertainty; surface what needs a human.
2. The user's files stay the user's: plain folders, visible provenance.
3. Phone-first and fast at library scale.
4. Legible to strangers: an open-source self-hoster should understand every screen without the maintainer.

## Accessibility & Inclusion

- Touch targets at least 44px; respect safe areas on notched phones.
- Honor reduced motion, reduced transparency, and forced colors.
- WCAG AA contrast in both light and dark themes.
