---
name: Soundgnome
description: A self-hosted music library that reads like a well-made instrument panel.
colors:
  night-ground: "#000000"
  paper-ground: "#ffffff"
  panel: "#0a0a0a"
  surface: "#0f0f10"
  surface-2: "#19191b"
  ink: "#fafafa"
  text: "#ededed"
  muted: "#a1a1a1"
  muted-2: "#8a8a8a"
  text-disabled: "#4f4f4f"
  hairline: "rgba(255, 255, 255, 0.09)"
  hairline-soft: "rgba(255, 255, 255, 0.06)"
  hairline-strong: "rgba(255, 255, 255, 0.14)"
  hairline-heavy: "rgba(255, 255, 255, 0.22)"
  signal-violet: "#a86af4"
  signal-violet-2: "#b885ff"
  on-accent: "#0a0a0a"
  live-cyan: "#44cfff"
  live-blue: "#3b6cff"
  moss-green: "#4ade80"
  tape-amber: "#f5b04a"
  clip-red: "#f4644a"
  brand-warm: "#f4644a"
  float: "#0f0f10"
  overlay: "rgba(0, 0, 0, 0.72)"
typography:
  display:
    fontFamily: "Geist, ui-sans-serif, system-ui, -apple-system, sans-serif"
    fontSize: "32px"
    fontWeight: 600
    lineHeight: 1.05
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "Geist, ui-sans-serif, system-ui, -apple-system, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    letterSpacing: "-0.02em"
  title:
    fontFamily: "Geist, ui-sans-serif, system-ui, -apple-system, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Geist, ui-sans-serif, system-ui, -apple-system, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Geist Mono, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "11px"
    fontWeight: 500
    letterSpacing: "0.08em"
  data:
    fontFamily: "Geist Mono, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "12px"
    fontWeight: 400
    fontFeature: "tnum"
rounded:
  badge: "4px"
  chip: "6px"
  control: "8px"
  card: "12px"
  panel: "16px"
spacing:
  "4": "4px"
  "6": "6px"
  "8": "8px"
  "10": "10px"
  "12": "12px"
  "14": "14px"
  "16": "16px"
  "20": "20px"
  "24": "24px"
  "28": "28px"
  "32": "32px"
  "48": "48px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.night-ground}"
    rounded: "{rounded.control}"
    padding: "0 16px"
    height: "36px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "36px"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.clip-red}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "36px"
  chip-filter:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.chip}"
    padding: "0 10px"
    height: "30px"
  chip-filter-active:
    backgroundColor: "color-mix(in srgb, #a86af4 18%, transparent)"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.chip}"
  tab:
    backgroundColor: "transparent"
    textColor: "{colors.muted-2}"
    typography: "{typography.label}"
    padding: "0 2px"
    height: "44px"
  tab-active:
    textColor: "{colors.ink}"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "10px 12px"
  card-cover:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.control}"
  badge:
    backgroundColor: "{colors.tape-amber}"
    textColor: "{colors.on-accent}"
    typography: "{typography.data}"
    rounded: "{rounded.badge}"
    size: "18px"
---

# Design System: Soundgnome

## Overview

**Creative North Star: "The Instrument Panel"**

Soundgnome is a music library that reads like a well-made instrument panel, not a streaming storefront. Retro yet professional, clean: a bare ground (pure black at night, white paper by day), hairline alpha rules carrying all structure, data set in Geist Mono, and pixel art as the only ornament. Every missing cover becomes a deterministic pixel sprite; every icon sits on a 24-unit pixel grid.

Density is operational: thousands of items in virtualized grids, two columns on a phone, six on desktop, with state readable at a glance (amber needs review, cyan is running or playing). The system refuses the category default: no glossy gradient cards, no blurred glass, no rounded pills.

**Key Characteristics:**
- Black/white ground with near-black/near-white surfaces and alpha hairlines.
- Geist for the interface, Geist Mono for counts, durations, dates, and small uppercase control labels.
- Two signals only: violet for what you chose, cyan for what is live; ink is the primary action.
- Pixelarticons at 16/24/48px; 15×15 ordered-dither sprite covers; a pixel gnome mark.
- Flat at rest; shadows only on floating layers.

## Colors

A monochrome instrument with two signal lamps; status colours appear only where they carry meaning. Every token has a night value (canonical here) and a day value pushed darker for AA (sidecar `colorMeta`).

### Primary
- **Ink** (#fafafa night / #0a0a0a day): the primary fill. Primary buttons and the play key are solid ink on ground.
- **Signal Violet** (#a86af4 night / #7438dc day): what you chose. Selection, active tab underline, active filter chip, links on hover, focus ring, caret, liked state.

### Secondary
- **Live Cyan** (#44cfff night / #06699a day): what is happening now. Playback progress, running tasks, info callouts, dither progress fills. Paired **Live Blue** (#3b6cff / #2347c9) as its secondary stop.

### Tertiary (status)
- **Moss Green** (#4ade80 / #11692f): success callouts and completed states.
- **Tape Amber** (#f5b04a / #9a4a0a): needs review. Warning badges, warn-tinted cover borders, validation rows.
- **Clip Red** (#f4644a / #b8321c): errors and destructive actions.
- **Brand Warm** (#f4644a / #c93a22): the mark's nose only.

### Neutral
- **Night Ground** (#000000) / **Paper Ground** (#ffffff): the page.
- **Panel / Surface / Surface 2** (#0a0a0a, #0f0f10, #19191b night; #fafafa, #f5f5f5, #ebebeb day): sidebar, inputs and cover wells, hover fills.
- **Text / Muted / Muted 2 / Disabled** (#ededed, #a1a1a1, #8a8a8a, #4f4f4f night; #171717, #555555, #6b6b6b, #b3b3b3 day).
- **Hairlines** (white alpha 0.06 / 0.09 / 0.14 / 0.22 at night; black alpha at the same steps by day): soft row dividers, default borders, float borders, hover borders.

### Named Rules
**The Two-Signal Rule.** Violet marks what you chose; cyan marks what is live. Never swap them, and never use either as decoration.

**The Ink Key Rule.** The primary action is solid ink (`--text-bright` on `--bg`), not a coloured button.

**The Glyph-Carries-Colour Rule.** Sprite covers keep a neutral field; colour lives in the glyph only.

## Typography

**Display Font:** Geist (with ui-sans-serif, system-ui)
**Body Font:** Geist
**Label/Mono Font:** Geist Mono (with ui-monospace, SF Mono, Menlo)

**Character:** A neutral grotesk for reading paired with a mono for measurement; the mono is the instrument's readout.

### Hierarchy
- **Display** (600, 32px desktop / 28px phone, 1.05, -0.035em): page titles.
- **Headline** (600, 22px): now-playing title in the full player.
- **Title** (600, 14–15px, -0.01em): section titles, empty-state titles.
- **Body** (400–500, 13–14px, 1.35–1.45): card titles (14/500), card subs and buttons (13), callouts and tables (14). Hints max 40ch.
- **Label** (Geist Mono 500, 11px, 0.06–0.08em, uppercase): tabs, filter chips, table heads.
- **Data** (Geist Mono 400, 12px, tabular-nums): counts, header meta line, durations, dates.

### Named Rules
**The Mono-for-Data Rule.** Geist Mono is for counts, durations, dates, and small uppercase control labels only; never for prose or headings.

**The No-Kicker Rule.** No eyebrows above headings; the meta line sits beneath the title in mono.

**The 11px Floor.** No text below 11px.

## Layout

Desktop: a 244px hairline sidebar (mark, navigation, mono counts, queue) beside the main column, with a 96px player dock spanning the bottom. Page padding `--space-page` is 32px. Phone (≤860px or coarse pointer): sidebar collapses, padding drops to 16px, a 56px icon-only tab bar docks at the bottom with a 64px floating player card 8px above it, and a More sheet holds secondary destinations. Card grid: `minmax(110–168px, 1fr)` auto-fill, gaps 20/12px on phone and 28/20px from 768px. Headers stack below 640px. Rhythm runs on 4/8 steps (4, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 48). Touch targets are at least 44px.

**Direction (not a rule):** Stencil's hairline data-row rhythm could carry more of the list views.

## Elevation & Depth

Flat by default. Hairline alpha borders carry structure; surfaces step tonally (ground → panel → surface → surface-2). Shadows appear only on floating layers: dialogs, sheets, toasts, popovers.

### Shadow Vocabulary
- **Float** (`inset 0 1px 0 rgba(255,255,255,0.06), 0 24px 60px -12px rgba(0,0,0,0.8)` night; `0 24px 60px -12px rgba(0,0,0,0.22)` day): modals, sheets, popovers, toasts.
- **Shadow** (`0 1px 2px rgba(0,0,0,0.5), 0 12px 32px -18px rgba(0,0,0,0.9)`): lifted floating controls.
- **Shadow sm** (`0 1px 2px rgba(0,0,0,0.5)`): small floating chips.
- **Rim** (`inset 0 1px 0 rgba(255,255,255,0.06)`): top-edge highlight on floats.
- **Focus ring** (`0 0 0 1px var(--accent), 0 0 0 4px` accent at 22%): every focused control.

### Named Rules
**The Flat-By-Default Rule.** Nothing on the ground casts a shadow; if it floats, it gets `--float-shadow` and a `--float-border`.

## Shapes

Small, square-ish radii, never pills: badge 4px, chip 6px, control and cover 8px, card container 12px, panel/dialog 16px. Borders are 1px alpha hairlines; covers draw theirs as an inset shadow so the image never shifts. Artist avatars are the one circle. Icons are pixel masks (`shape-rendering: crispEdges`) on a 24-unit grid, shown at 16, 24, or 48px so pixels land whole.

## Components

Quiet instruments: hairline-framed, mono-labelled, with ink keys for primary actions.

### Buttons
- **Shape:** gently squared (8px), min-height 36px; compact `.btn-sm` 30px / 12px text.
- **Primary:** solid ink, ground-coloured text, 13px/600, padding 0 16px. Hover mixes ink 86% into ground; active 78%.
- **Secondary:** transparent with a hairline, 13px/500, padding 0 14px; hover fills surface-2.
- **Danger:** clip-red text and hairline; hover tints 12% red.
- **Feedback:** paint-only (no transforms) on player and virtualized controls. Icons inside are 16px.
- **Play key:** 40px solid ink square (8px radius) with a solid 24px play glyph.

### Chips
- **Filter:** 30px, 6px radius, hairline border, mono 11px uppercase muted. Hover strengthens border; active fills violet at 18% with a 45% violet border and ink text. Like a tape deck's function keys.
- **Touch chip:** opaque hairline chip on covers (card actions), never glass.

### Cards / Containers
- **Cards have no chrome:** the cover carries an 8px radius, surface well, and an inset hairline (heavy on hover, amber at 60% when it needs review); title (14/500), sub (13 muted) and mono meta (11) sit beneath.
- **Covers:** real art when present, otherwise `PixelCover` with a stable seed (15×15 dither sprite, neutral field, coloured glyph, theme-aware).
- **Containers:** batch tools and tables use a 12px hairline frame.

### Inputs / Fields
- **Style:** surface fill, hairline, 8px radius, padding 10px 12px, 16px text (no iOS zoom). Search fields 36px / 14px.
- **Focus:** violet border plus `--focus-ring`.

### Navigation
- **Tabs:** mono 11px uppercase muted labels over a hairline, 44px tall, 24px gaps; active turns ink with a 2px violet underline that scales in.
- **Sidebar:** 244px hairline-separated; nav items with 16px icons and mono counts.
- **Tab bar (phone):** 24px pixel icons only (the label is the accessible name and tooltip), violet tick on the active tab.
- **Floating player (phone):** a 64px `--float` card with a hairline border and float shadow, inset 8px above the tab bar; tapping it morphs into Now Playing (View Transition API).
- **Breadcrumb:** 14px muted crumbs, ink current item.

### Badges, Callouts, Empty States
- **Badge:** 18px amber square, 4px radius, mono 11/600.
- **Callout:** full 1px hairline tinted 35% by meaning over a 10–12% tinted ground, 16px icon plus body.
- **Empty state:** one 48px pixel glyph, 15px title, 14px muted hint (40ch).
- **Loading:** a pixel icon turning in eight hard steps (`.pxi-spin`), like an LCD busy glyph.

### Brand Mark
A pixel gnome whose hat and beard mirror a waveform around the brim; Brand Warm on the nose only.

## Do's and Don'ts

### Do:
- **Do** use violet for chosen state and cyan for live state, nothing else.
- **Do** make the primary action an ink key (`--text-bright` on `--bg`).
- **Do** set counts, durations, and dates in Geist Mono with tabular numerals.
- **Do** use pixel icons at 16, 24, or 48px only.
- **Do** render missing covers with `PixelCover` and a stable seed.
- **Do** let the theme follow local time: day 07:00–19:00, night otherwise.
- **Do** keep touch targets at 44px and text at 11px or above.

### Don't:
- **Don't** use glossy gradient cards, blurred glass, or rounded pills.
- **Don't** put kickers or eyebrows above headings.
- **Don't** shadow anything that sits on the ground.
- **Don't** colour sprite fields; colour lives in the glyph.
- **Don't** use mono for prose or headings.
