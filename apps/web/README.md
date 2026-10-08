# Soundgnome web app

This package contains the Svelte admin interface for Soundgnome.

## Responsibilities

- Submit track and playlist URLs to the server.
- Display recent downloads.
- Review and approve or reject tracks that need manual validation.
- Poll server-side task state when playlist downloads run in the background.
- Manage sync schedules and inspect storage through the Tools tabs, alongside provider connections and library maintenance.
- Play audio through the persistent native audio player with MediaSession integration.

## Runtime model

- Development: Vite serves the SPA on `http://localhost:5173` and proxies API traffic to Rocket. Set `SOUNDGNOME_API_URL` when Rocket runs on a different host or port.
- Production build: static assets are emitted into `data/web/` and served by the Rocket server.
- PWA: static assets are precached, but navigations and API calls remain network-only to preserve authentication.

## Useful commands

From the repository root:

```bash
pnpm web:dev
pnpm web:build
pnpm web:check
```

## Related documentation

- [../../docs/workflows/web-admin.md](../../docs/workflows/web-admin.md)
- [../../README.md](../../README.md)
