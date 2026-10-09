#!/usr/bin/env node
// Renders the favicon, Apple touch icon, and PWA icons from the pixel mark in
// src/lib/brand-mark.js. Icons always sit on the night ground.

import sharp from 'sharp'
import fs from 'node:fs/promises'
import path from 'node:path'
import { crc32 } from 'node:zlib'
import { MARK_HEIGHT, MARK_WIDTH, markPaths } from '../src/lib/brand-mark.js'

const publicDir = path.resolve(process.cwd(), 'public')
const paths = markPaths()
const COLORS = { ground: '#000000', hat: '#a86af4', beard: '#fafafa', nose: '#f4644a' }
// Provenance travels inside each PNG (a tEXt chunk read by Impeccable's asset scan).
const PROVENANCE = 'Rendered by apps/web/scripts/generate-all-icons.mjs from the pixel mark in apps/web/src/lib/brand-mark.js; no image model.'

function withTextChunk(png, keyword, text) {
  const data = Buffer.from(`${keyword}\0${text}`, 'latin1')
  const chunk = Buffer.alloc(12 + data.length)
  chunk.writeUInt32BE(data.length, 0)
  chunk.write('tEXt', 4, 'latin1')
  data.copy(chunk, 8)
  chunk.writeUInt32BE(crc32(chunk.subarray(4, 8 + data.length)), 8 + data.length)
  // IEND is always the final 12 bytes; text chunks go just before it.
  const iend = png.length - 12
  return Buffer.concat([png.subarray(0, iend), chunk, png.subarray(iend)])
}

// scale = pixels per mark pixel. Even scales on even canvases center the
// odd-width mark on whole pixels. radius 0 = full bleed (iOS rounds its own
// icon; maskable icons keep the mark inside the 80% safe circle).
const ICONS = [
  { file: 'favicon.png', size: 32, scale: 2, radius: 7 },
  { file: 'apple-touch-icon.png', size: 180, scale: 8, radius: 0 },
  { file: 'pwa-192x192.png', size: 192, scale: 8, radius: 42 },
  { file: 'pwa-512x512.png', size: 512, scale: 20, radius: 112 },
  { file: 'pwa-192x192-maskable.png', size: 192, scale: 8, radius: 0 },
  { file: 'pwa-512x512-maskable.png', size: 512, scale: 20, radius: 0 },
]

for (const { file, size, scale, radius } of ICONS) {
  const x = (size - MARK_WIDTH * scale) / 2
  const y = (size - MARK_HEIGHT * scale) / 2
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <rect width="${size}" height="${size}" rx="${radius}" fill="${COLORS.ground}"/>
  <g transform="translate(${x} ${y}) scale(${scale})" shape-rendering="crispEdges">
    <path fill="${COLORS.hat}" d="${paths.hat}"/>
    <path fill="${COLORS.beard}" d="${paths.beard}"/>
    <path fill="${COLORS.nose}" d="${paths.nose}"/>
  </g>
</svg>`
  const png = await sharp(Buffer.from(svg)).png().toBuffer()
  await fs.writeFile(path.join(publicDir, file), withTextChunk(png, 'impeccable:prompt', PROVENANCE))
  console.log(`✓ Generated ${file} (${size}x${size})`)
}
