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

running() { pgrep -f 'TimeTracker( \(Debug\))?\.app/Contents/MacOS/timetracker( --hidden)?$' >/dev/null; }
# Re-signing a running app's files can get it killed, so it quits first.
running && WAS_RUNNING=1 && "$ROOT/scripts/stop-app.sh" "$APP/Contents/MacOS/timetracker"
"$ROOT/scripts/sign-app.sh" "$APP" com.philippstracker.timetracker

mkdir -p "$HOME/Applications" "$HOME/.local/bin"
ln -sfn "$APP" "$LINK"
ln -sf "$BIN" "$HOME/.local/bin/timetracker"
echo "Installed the release build as ~/Applications/TimeTracker.app and ~/.local/bin/timetracker"

[[ -n "$WAS_RUNNING" ]] || exit 0
# Launch Services can refuse right after the old app quit (error -600), so try a few times.
for _ in {1..10}; do
  open -g "$LINK" 2>/dev/null
  sleep 1
  running && { echo "Restarted TimeTracker on the new release build"; exit 0; }
done
echo "TimeTracker did not start again; open it from ~/Applications" >&2
exit 1
