// Client-side Last.fm scrobbler. The player reports playback here; this module
// decides when a play qualifies (Last.fm's rule: track > 30 s, played for at
// least half its length or 4 minutes, whichever comes first) and posts it to the
// server, which signs and forwards it. Failed sends are queued in localStorage
// and retried, so a brief outage or offline stretch doesn't lose plays.

import {
  getLastfmStatus,
  lastfmNowPlaying,
  lastfmScrobble,
  type LastfmStatusDto,
  type ScrobblePayload,
} from './api';
import type { PlayerTrack } from './player';

const ENABLED_KEY = 'soundgnome:scrobble:enabled';
const QUEUE_KEY = 'soundgnome:scrobble:queue';
const SCROBBLE_AFTER_SECS = 240; // 4 minutes
const MIN_TRACK_SECS = 30;

let status: LastfmStatusDto | null = null;
let statusPromise: Promise<LastfmStatusDto> | null = null;

let enabled = typeof localStorage !== 'undefined' && localStorage.getItem(ENABLED_KEY) !== 'false';

export function isScrobbleEnabled(): boolean {
  return enabled;
}
export function setScrobbleEnabled(value: boolean): void {
  enabled = value;
  try {
    localStorage.setItem(ENABLED_KEY, String(value));
  } catch {
    /* storage unavailable */
  }
}

/** Re-fetch connection status (call after connecting/disconnecting Last.fm). */
export function refreshStatus(): void {
  status = null;
  statusPromise = null;
}

async function ensureStatus(): Promise<LastfmStatusDto> {
  if (status) return status;
  if (!statusPromise) {
    statusPromise = getLastfmStatus()
      .then((s) => (status = s))
      .catch(() => (status = { configured: false, connected: false, username: null }));
  }
  return statusPromise;
}


// ── Current track tracking ────────────────────────────────────────────────────

let startedAt = 0; // unix seconds when the current track began
let scrobbled = false; // already scrobbled the current play
let currentKey = '';

function trackKey(t: PlayerTrack): string {
  return `${t.source ?? 'library'}:${t.id}`;
}

/** Call when a track starts playing. Resets state and kicks a status fetch;
   the actual arming + now-playing happens in `onProgress` once status is known,
   so this also works when the user connects mid-playback. */
export async function onPlay(_track: PlayerTrack): Promise<void> {
  currentKey = '';
  scrobbled = false;
  await ensureStatus();
  void flushQueue();
}

/** Call on playback progress; arms the track (now-playing) the first time it is
   seen while connected, then scrobbles once the threshold is met. */
export function onProgress(track: PlayerTrack, currentTime: number, duration: number): void {
  if (!enabled) return;
  if (!status) {
    // Not fetched yet (e.g. just connected). Kick it; a later tick will act.
    void ensureStatus();
    return;
  }
  if (!status.connected) return;

  const artist = track.artist?.trim();
  const title = track.title?.trim();
  if (!artist || !title) return;

  const key = trackKey(track);
  if (key !== currentKey) {
    // First tick for this track while connected: a fresh play, or the user
    // connected / the page loaded mid-playback. Arm it, reconstructing when it
    // started so the scrobble timestamp is correct, and send now-playing.
    currentKey = key;
    scrobbled = false;
    startedAt = Math.max(0, Math.floor(Date.now() / 1000) - Math.floor(currentTime));
    lastfmNowPlaying({
      artist,
      track: title,
      album: null,
      duration_secs: track.durationSecs ?? null,
    }).catch(() => {
      /* now-playing is best-effort */
    });
  }
  if (scrobbled) return;

  const dur = duration > 0 ? duration : (track.durationSecs ?? 0);
  // >= 30 s: half or 4 min, whichever first. Unknown length: 4 min. Too short: never.
  const threshold =
    dur >= MIN_TRACK_SECS
      ? Math.min(dur / 2, SCROBBLE_AFTER_SECS)
      : dur > 0
        ? Infinity
        : SCROBBLE_AFTER_SECS;

  if (currentTime >= threshold) {
    scrobbled = true;
    enqueue({
      artist,
      track: title,
      album: null,
      duration_secs: dur > 0 ? Math.round(dur) : null,
      timestamp: startedAt,
    });
    void flushQueue();
  }
}

// ── Retry queue ────────────────────────────────────────────────────────────────

function loadQueue(): ScrobblePayload[] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(QUEUE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveQueue(queue: ScrobblePayload[]): void {
  try {
    // Cap so a long offline stretch can't grow unbounded (Last.fm ignores very old ones anyway).
    localStorage.setItem(QUEUE_KEY, JSON.stringify(queue.slice(-100)));
  } catch {
    /* storage unavailable */
  }
}

function enqueue(item: ScrobblePayload): void {
  const queue = loadQueue();
  queue.push(item);
  saveQueue(queue);
}

let flushing = false;

/** Send queued scrobbles in batches; keeps them on failure to retry later. */
export async function flushQueue(): Promise<void> {
  if (flushing) return;
  if (loadQueue().length === 0) return;
  await ensureStatus();
  if (!status?.connected) return;

  flushing = true;
  try {
    // One batch per call (Last.fm accepts up to 50); the next play flushes the rest.
    const batch = loadQueue().slice(0, 50);
    if (batch.length === 0) return;
    await lastfmScrobble(batch);
    saveQueue(loadQueue().slice(batch.length));
  } catch {
    /* keep the queue for a later retry */
  } finally {
    flushing = false;
  }
}
