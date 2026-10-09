// The Soundgnome mark: a pixel gnome whose hat and beard are one waveform
// mirrored around the brim, its zero line. One character per pixel:
// h hat, b brim and beard, n nose, '.' empty. Shared by BrandMark.svelte and
// scripts/generate-all-icons.mjs so the logo and app icons never drift.
export const MARK_ROWS = [
  '.......h.......',
  '......hhh......',
  '......hhh......',
  '.....hhhhh.....',
  '.....hhhhh.....',
  '....hhhhhhh....',
  '....hhhhhhh....',
  '..bbbbbbbbbbb..',
  '...b.bnnnb.b...',
  '...b.bnnnb.b...',
  '...b.b.b.b.b...',
  '...b.b.b.b.b...',
  '.....b.b.b.....',
  '.......b.......',
]

export const MARK_WIDTH = MARK_ROWS[0].length
export const MARK_HEIGHT = MARK_ROWS.length

/**
 * One SVG path per part, horizontal runs merged, in mark-pixel units.
 * @returns {{ hat: string, beard: string, nose: string }}
 */
export function markPaths() {
  /** @type {Record<string, string>} */
  const paths = { h: '', b: '', n: '' }
  MARK_ROWS.forEach((row, y) => {
    for (let x = 0; x < row.length; ) {
      const part = row[x]
      let end = x + 1
      while (end < row.length && row[end] === part) end++
      if (part in paths) paths[part] += `M${x} ${y}h${end - x}v1h-${end - x}z`
      x = end
    }
  })
  return { hat: paths.h, beard: paths.b, nose: paths.n }
}
