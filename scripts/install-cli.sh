#!/bin/zsh
# Links the `timetracker` command to the built app so scripts can launch it.
# Usage: scripts/install-cli.sh [path/to/TimeTracker.app]   (default: the debug build)
set -e
APP="${1:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/debug/bundle/macos/TimeTracker.app}"
BIN="$APP/Contents/MacOS/timetracker"
[[ -x "$BIN" ]] || { echo "Not found: $BIN (run 'bun run build:app' first)"; exit 1; }
mkdir -p "$HOME/.local/bin"
ln -sf "$BIN" "$HOME/.local/bin/timetracker"
echo "Linked ~/.local/bin/timetracker -> $BIN"
