---
version: 1
slug: "src-app-svelte"
primary_target: "src/App.svelte"
related_targets: ["src/pages/Library.svelte","src/lib/AudioPlayer.svelte","src/pages/Home.svelte","src/pages/Tools.svelte"]
---

# Surface: Soundgnome web app (shell, library, player, tools)

Mode: Operate. Phone-first (iPhone Safari and the Home Screen app); desktop must stay excellent.
Audience and job: open-source self-hosters browsing, playing, downloading, ingesting, and reviewing their own music library.
Constraints: thousands of items (virtualized grids), theme follows local time (light 07:00–19:00), WCAG AA in both themes, 44px touch targets, no per-item compositing cost on iPhone Safari.

## Direction contract

THESIS: A music library that reads like a well-made instrument panel, not a streaming storefront: bare ground, hairline rules, data set in mono, and every missing cover turned into a deterministic pixel sprite. It refuses the category default of glossy gradient cards, blurred glass, and rounded pills.

OWN-WORLD: Pure black night ground and white paper day ground; near-black/near-white surfaces; alpha hairlines; Geist for the interface, Geist Mono for counts, durations, dates, and small uppercase control labels; violet for what you chose, cyan for what is live, ink for the primary action; pixelarticons on a 24-unit grid; a pixel gnome mark whose hat and beard mirror a waveform around the brim; 15×15 ordered-dither sprite covers.

STORY: The visitor sees their library at once as a mosaic of real art and unique pixel sprites, reads state at a glance (amber needs review, cyan is running or playing), and plays, downloads, or reviews in one or two taps.

FIRST VIEWPORT: Phone: 28px title with a mono count line and icon chips, mono underline section tabs, search, then a two-column cover grid; the mini player and tab bar dock at the bottom (pixel icons, mono labels, violet tick on the active tab). Desktop: 244px hairline sidebar with the mark, navigation, mono counts, and queue; 32px title; six-column grid; 96px player dock.

FORM: Pinned by the user: "retro yet professional, clean", modeled on stencil.so; position 1 of 1. No concept roll: the user pinned the world and asked for impeccable after the redesign, so there is no seed key.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
