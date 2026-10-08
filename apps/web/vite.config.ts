import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { VitePWA } from 'vite-plugin-pwa'
import type { AtRule, Node, Plugin, Rule } from 'postcss'

// iOS keeps a tapped element in :hover until the next tap, so a pressed button
// stays highlighted. Move every :hover selector (component styles included)
// into @media (hover: hover); non-hover selectors in the same rule stay put.
const hoverOnlyWhereSupported: Plugin = {
  postcssPlugin: 'hover-only-where-supported',
  Rule(rule: Rule, { AtRule }) {
    if (!rule.selector.includes(':hover')) return
    for (let p: Node | undefined = rule.parent; p; p = p.parent) {
      if (p.type === 'atrule' && (p as AtRule).params.includes('hover: hover')) return
    }
    const hovering = rule.selectors.filter((s) => s.includes(':hover'))
    const media = new AtRule({ name: 'media', params: '(hover: hover)' })
    media.append(rule.clone({ selectors: hovering }))
    rule.after(media)
    const rest = rule.selectors.filter((s) => !s.includes(':hover'))
    if (rest.length) rule.selectors = rest
    else rule.remove()
  },
}

// https://vite.dev/config/
export default defineConfig({
  css: { postcss: { plugins: [hoverOnlyWhereSupported] } },
  plugins: [
    svelte(),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['favicon.png', 'apple-touch-icon.png'],
      manifest: {
        name: 'Soundgnome',
        short_name: 'Soundgnome',
        description: 'Personal music library manager',
        theme_color: '#000000',
        background_color: '#000000',
        display: 'standalone',
        scope: '/',
        start_url: '/',
        icons: [
          {
            src: 'pwa-192x192.png',
            sizes: '192x192',
            type: 'image/png',
            purpose: 'any',
          },
          {
            src: 'pwa-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'any',
          },
          {
            src: 'pwa-192x192-maskable.png',
            sizes: '192x192',
            type: 'image/png',
            purpose: 'maskable',
          },
          {
            src: 'pwa-512x512-maskable.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
        categories: ['music', 'productivity'],
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,svg,png,ico,json,webmanifest}'],
        cleanupOutdatedCaches: true,
        // The app sits behind Cloudflare Access. Serving the SPA shell from cache
        // for navigations bypasses the Access login, so once the session lapses
        // the shell still loads but every /api call is 302'd to the login and the
        // fetch throws "Failed to fetch". Disable the cached navigation fallback
        // and force navigations to the network so the edge always runs Access
        // (and re-prompts login when needed). Static assets stay precached.
        navigateFallbackDenylist: [/./],
        runtimeCaching: [
          {
            urlPattern: ({ request }) => request.mode === 'navigate',
            handler: 'NetworkOnly',
          },
          {
            // Never cache the API (it's Access-gated + live); always hit network.
            urlPattern: ({ url }) => url.pathname.startsWith('/api/'),
            handler: 'NetworkOnly',
          },
        ],
      },
      devOptions: {
        enabled: true,
        navigateFallback: 'index.html',
      },
    }),
  ],
  build: {
    outDir: '../../data/web',
    emptyOutDir: true,
  },
  server: {
    host: '0.0.0.0',
    proxy: {
      // Override when the API runs elsewhere or on another port, e.g. a remote
      // box where 8000 is already taken: SOUNDGNOME_API_URL=http://localhost:8100
      '/api': process.env.SOUNDGNOME_API_URL ?? 'http://localhost:8000',
    },
  },
})
