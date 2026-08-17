#!/usr/bin/env bash
# Continuously mirror this checkout to the remote dev box.
#
# One way only, local to remote. The remote owns the runtime state
# (library/, data/, temp/, ingest/) and its own secrets, so those never
# travel in either direction. Two way sync would corrupt the database.
#
# Usage:
#   scripts/dev-sync.sh              # watch and sync on every save
#   scripts/dev-sync.sh --once       # single sync, then exit
#   SOUNDGNOME_REMOTE=box scripts/dev-sync.sh
set -euo pipefail

REMOTE="${SOUNDGNOME_REMOTE:-worker}"
REMOTE_PATH="${SOUNDGNOME_REMOTE_PATH:-dev/soundgnome/}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

EXCLUDES=(
  --exclude 'target/'
  --exclude 'node_modules/'
  --exclude '.git/'
  --exclude '/library/'
  --exclude '/data/'
  --exclude '/temp/'
  --exclude '/ingest/'
  --exclude '.env'
  --exclude 'config.toml'
  --exclude 'apps/web/dev-dist/'
  --exclude '.DS_Store'
)

sync_once() {
  # --delete keeps removals honest, but only inside the paths we own.
  rsync -a --delete "${EXCLUDES[@]}" "$ROOT/" "$REMOTE:$REMOTE_PATH"
  printf '\033[2m%s  synced -> %s\033[0m\n' "$(date +%H:%M:%S)" "$REMOTE"
}

sync_once

if [[ "${1:-}" == "--once" ]]; then
  exit 0
fi

echo "watching $ROOT, syncing to $REMOTE:$REMOTE_PATH (ctrl-c to stop)"

# watchexec honours .gitignore, so target/ and node_modules/ are already out.
exec watchexec \
  --watch "$ROOT" \
  --ignore '/library/**' \
  --ignore '/data/**' \
  --ignore '/temp/**' \
  --debounce 300ms \
  --no-vcs-ignore \
  --ignore-file "$ROOT/.gitignore" \
  -- rsync -a --delete "${EXCLUDES[@]}" "$ROOT/" "$REMOTE:$REMOTE_PATH"
