#!/bin/zsh
# Makes TimeTracker available in Spotlight/Launchpad by linking the built app into ~/Applications.
# The link points at the build output, so every rebuild is picked up automatically.
# Usage: scripts/install-app.sh [--debug] [path/to/TimeTracker.app]   (default: the release build)
set -e
PROFILE=release BUILD="bun run release"
[[ "$1" == "--debug" ]] && { PROFILE=debug BUILD="bun run build:app"; shift; }
APP="${1:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/$PROFILE/bundle/macos/TimeTracker.app}"
[[ -d "$APP" ]] || { echo "Not found: $APP (run '$BUILD' first)"; exit 1; }
mkdir -p "$HOME/Applications"
ln -sfn "$APP" "$HOME/Applications/TimeTracker.app"
echo "Linked ~/Applications/TimeTracker.app -> $APP"
