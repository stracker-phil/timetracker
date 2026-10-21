#!/bin/zsh
# Links the `timetracker` command to the built app so scripts can launch it.
# Usage: scripts/install-cli.sh [--debug] [path/to/TimeTracker.app]   (default: the release build)
set -e
PROFILE=release BUILD="bun run release"
[[ "$1" == "--debug" ]] && { PROFILE=debug BUILD="bun run build:app"; shift; }
APP="${1:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/$PROFILE/bundle/macos/TimeTracker.app}"
BIN="$APP/Contents/MacOS/timetracker"
[[ -x "$BIN" ]] || { echo "Not found: $BIN (run '$BUILD' first)"; exit 1; }
mkdir -p "$HOME/.local/bin"
ln -sf "$BIN" "$HOME/.local/bin/timetracker"
echo "Linked ~/.local/bin/timetracker -> $BIN"
