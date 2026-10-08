import type { Theme } from './theme.svelte';

// Albums, tracks, artists, and playlists without artwork get a deterministic
// pixel sprite: a left-right symmetric 11×11 glyph on a 15×15 near-neutral
// field, ordered-dithered from one hue to another top to bottom. Color lives in
// the glyph; the field only carries a ~4% tint of the top hue.
// Each palette entry is [top hue, bottom hue, field].
const PALETTES: Record<Theme, ReadonlyArray<readonly [string, string, string]>> = {
  dark: [
    ['#44cfff', '#a86af4', '#0d1316'],
    ['#44cfff', '#a86af4', '#110f15'],
    ['#3b6cff', '#44cfff', '#0d0f16'],
    ['#b885ff', '#3b6cff', '#110f15'],
    ['#4ade80', '#44cfff', '#0e1311'],
    ['#a86af4', '#f4644a', '#110f15'],
  ],
  light: [
    ['#0778ad', '#7438dc', '#edf2f4'],
    ['#0778ad', '#7438dc', '#f2eff6'],
    ['#2347c9', '#0778ad', '#eef0f5'],
    ['#6d3fd0', '#2347c9', '#f2eff6'],
    ['#15803d', '#0778ad', '#eef2f0'],
    ['#7438dc', '#c93a22', '#f2eff6'],
  ],
};

// 4×4 Bayer matrix: the ordered-dither thresholds behind the retro gradient.
const BAYER = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5],
];

const FIELD = 15;
const GLYPH = 11;
const INSET = (FIELD - GLYPH) / 2;
const HALF = Math.ceil(GLYPH / 2);
const CACHE_LIMIT = 1200;
const cache = new Map<string, string>();

/** FNV-1a string hash feeding a mulberry32 generator: same seed, same sprite. */
function seededRandom(seed: string): () => number {
  let state = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    state ^= seed.charCodeAt(i);
    state = Math.imul(state, 0x01000193);
  }
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Sprite seed for a library track: its album's when it has one, so a track and its album share a cover. */
export function trackCoverSeed(track: { id: number; album: { id: number | null } | null }): string {
  return track.album?.id != null ? `album:${track.album.id}` : `track:${track.id}`;
}

/**
 * Data URL for the sprite of `seed` (e.g. `album:42`) in `theme`.
 * Render it with `image-rendering: pixelated` at any size.
 */
export function pixelCover(seed: string, theme: Theme): string {
  const key = `${theme}|${seed}`;
  const cached = cache.get(key);
  if (cached) return cached;

  const random = seededRandom(seed);
  const palettes = PALETTES[theme];
  const [top, bottom, field] = palettes[Math.floor(random() * palettes.length)];

  // Fill the left half, densest toward the vertical axis, then mirror it.
  const half: boolean[][] = [];
  for (let y = 0; y < GLYPH; y++) {
    const row: boolean[] = [];
    for (let x = 0; x < HALF; x++) {
      const depth = Math.min(x, y, GLYPH - 1 - y, 2);
      row.push(random() < 0.42 + 0.12 * depth);
    }
    half.push(row);
  }

  let topPath = '';
  let bottomPath = '';
  for (let y = 0; y < GLYPH; y++) {
    const blend = (y + 0.5) / GLYPH;
    for (let x = 0; x < GLYPH; x++) {
      if (!half[y][x < HALF ? x : GLYPH - 1 - x]) continue;
      const px = x + INSET;
      const py = y + INSET;
      const cell = `M${px} ${py}h1v1h-1z`;
      if ((BAYER[py % 4][px % 4] + 0.5) / 16 < blend) bottomPath += cell;
      else topPath += cell;
    }
  }

  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${FIELD} ${FIELD}" shape-rendering="crispEdges">` +
    `<rect width="${FIELD}" height="${FIELD}" fill="${field}"/>` +
    `<path fill="${top}" d="${topPath}"/><path fill="${bottom}" d="${bottomPath}"/></svg>`;
  const url = `data:image/svg+xml,${encodeURIComponent(svg)}`;

  if (cache.size >= CACHE_LIMIT) cache.delete(cache.keys().next().value!);
  cache.set(key, url);
  return url;
}

const MASK_CELLS = 40;
let fadeMask: string | undefined;

/**
 * A radial fade rendered as an ordered dither: solid in the middle, dissolving
 * into scattered pixels toward the edge. Used as a CSS mask so a glow breaks
 * up like the sprites instead of blurring. Same for every cover, so built once.
 */
export function ditherFadeMask(): string {
  if (fadeMask) return fadeMask;
  const center = MASK_CELLS / 2;
  const solid = MASK_CELLS * 0.3; // fully on inside this radius (under the art)
  const edge = MASK_CELLS * 0.5;
  let path = '';
  for (let y = 0; y < MASK_CELLS; y++) {
    for (let x = 0; x < MASK_CELLS; x++) {
      const distance = Math.hypot(x + 0.5 - center, y + 0.5 - center);
      const fade = Math.min(1, Math.max(0, (distance - solid) / (edge - solid)));
      if ((BAYER[y % 4][x % 4] + 0.5) / 16 >= fade) path += `M${x} ${y}h1v1h-1z`;
    }
  }
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${MASK_CELLS} ${MASK_CELLS}" shape-rendering="crispEdges">` +
    `<path fill="#fff" d="${path}"/></svg>`;
  fadeMask = `data:image/svg+xml,${encodeURIComponent(svg)}`;
  return fadeMask;
}
