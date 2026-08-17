/**
 * Shared audio-player types + the context key for the single, app-wide player.
 *
 * The player is mounted once in the shell (`App.svelte`), not inside a page, so
 * playback and the player bar survive navigation. Pages drive it through the
 * `GLOBAL_PLAYER` context. Types live here (a plain `.ts`) rather than in the
 * `.svelte` module so non-component files can import them.
 */

/** Minimal description of whatever is playing, built by the calling page. */
export interface PlayerTrack {
  id: number;
  title: string;
  artist: string;
  artwork: string | null;
  durationSecs: number | null;
  /** Precomputed waveform peaks url (SoundCloud); enables the scrubber. */
  waveformUrl?: string | null;
  /** Spotify track URL — used to fetch album art on demand when artwork is null. */
  spotifyUrl?: string | null;
  /**
   * Which backend the audio comes from. Scopes both URL resolution and track
   * identity, so a library track and a SoundCloud like that happen to share a
   * numeric id are never confused.
   */
  source?: TrackSource;
}

export type TrackSource = 'library' | 'soundcloud';

/** What a parent gets back through `bind:this` on the player component. */
export interface PlayerHandle {
  toggle(track: PlayerTrack, queue?: PlayerTrack[]): Promise<void>;
  /** Toggle play/pause of the currently-loaded track (no-op if nothing is loaded). */
  playPause(): void;
  isCurrent(id: number, source?: TrackSource): boolean;
  isPlaying(id: number, source?: TrackSource): boolean;
  isResolving(id: number, source?: TrackSource): boolean;
}

/** Context key for the persistent, shell-mounted player. */
export const GLOBAL_PLAYER = Symbol('global-player');

/** The surface pages use to drive the persistent player. */
export interface GlobalPlayer {
  /** Start a track, optionally adopting `queue` for prev/next/shuffle. */
  play(track: PlayerTrack, queue?: PlayerTrack[]): void;
  isCurrent(id: number, source?: TrackSource): boolean;
  isPlaying(id: number, source?: TrackSource): boolean;
  isResolving(id: number, source?: TrackSource): boolean;
}
