# Development setup

The website can run locally against an existing server, or together with a local
Rust backend. No PostgreSQL, Redis, or separate database service is required.

## Frontend-only development

Install Node.js 24 and pnpm 10.8.0, then clone the repository:

```bash
git clone https://github.com/richyk1/soundgnome.git
cd soundgnome
pnpm install --frozen-lockfile
SOUNDGNOME_API_URL=http://worker:8100 pnpm web:dev
```

Open `http://localhost:5173`. Vite proxies `/api` to `SOUNDGNOME_API_URL`; without
that variable it uses `http://localhost:8000`. The machine must be able to reach
the configured server (for example, through the tailnet for `worker`).

This mode needs no local Rust toolchain, database, media files, or provider
credentials. It uses the remote server's real library: edits, deletions, and
downloads affect that server.

## Full local development

Install:

- Node.js 24 and pnpm 10.8.0
- Rust stable and a native C build toolchain
- `pkg-config` and OpenSSL development libraries on Linux
- `ffmpeg` and `yt-dlp` for audio processing and downloads
  (see [YouTube configuration](../operations/youtube-search-configuration.md))

From the repository root:

```bash
git submodule update --init --recursive
pnpm install --frozen-lockfile
cp config.example.toml config.toml
pnpm web:build
pnpm dev
```

The first Rust build takes longer than subsequent starts. Build the website once
before starting the server because Rocket mounts `data/web/` even in development.

This starts:

- the Rocket server on `http://localhost:8000`
- the Vite frontend on `http://localhost:5173`

## Runtime configuration and database

The server reads `config.toml` and `SOUNDGNOME__...` environment overrides. A local
`.env` file is optional for the server; the CLI still requires one. Provider
credentials are only needed for integrations you use. See
[configuration.md](configuration.md).

SQLite is bundled with the backend. Startup creates the configured database file
and applies embedded migrations automatically. The default in the example config
is `./data/soundgnome.db`; the server injects that path into Rocket's database
configuration. `diesel_cli` is only needed for manual migration/schema development:

```bash
cargo install diesel_cli --no-default-features --features sqlite
diesel migration run
```

## Moving development to another machine

Push source changes and the dependency submodule commit before cloning elsewhere.
Do not transfer `target/`, `node_modules/`, or temporary downloads as part of the
source checkout. Keep `.env` files, provider credentials, and `config.toml` out of
Git and transfer them securely only when needed.

A new local backend can start with an empty library. To retain an existing
collection, transfer a consistent SQLite backup, covers, and audio files, then
reconcile stored filesystem paths with the new library location. Do not copy a
live SQLite file without accounting for its WAL.

## Useful commands

```bash
pnpm cargo:test
pnpm cargo:clippy
pnpm cargo:fmt
pnpm web:check
pnpm web:build
```

## Working notes

- The CLI crate exists but is still minimal.
- The server is currently the main entry point for the end-to-end workflow.
- The frontend build is emitted into `data/web/` and served by Rocket outside Vite development mode.
