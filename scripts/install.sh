#!/bin/zsh
# Installs the release build as the one TimeTracker of this machine: ~/Applications/TimeTracker.app links to
# it and the `timetracker` command links through that app. A running TimeTracker (any build) restarts on it.
# `bun run release` ends here; debug builds are never installed (scripts/dev/launch.sh runs them by path).
set -e
ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/TimeTracker.app"
[[ -d "$APP" ]] || { echo "Not found: $APP (run 'bun run release')"; exit 1; }
LINK="$HOME/Applications/TimeTracker.app"
BIN="$LINK/Contents/MacOS/timetracker"

mkdir -p "$HOME/Applications" "$HOME/.local/bin"
ln -sfn "$APP" "$LINK"
ln -sf "$BIN" "$HOME/.local/bin/timetracker"
echo "Installed the release build as ~/Applications/TimeTracker.app and ~/.local/bin/timetracker"

pgrep -f 'TimeTracker( \(Debug\))?\.app/Contents/MacOS/timetracker( --hidden)?$' >/dev/null || exit 0
"$ROOT/scripts/stop-app.sh" "$BIN"
open -g "$LINK"
echo "Restarted TimeTracker on the new release build"
