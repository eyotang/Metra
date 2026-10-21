#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$project_dir/src-tauri/target/release/metra"

if [[ ! -x "$binary" ]]; then
  printf 'Build Metra first: npm run build -- --no-bundle\n' >&2
  exit 1
fi

# Global positioning requires XWayland even when the desktop prefers Wayland.
export GDK_BACKEND=x11
exec "$binary" "$@"
