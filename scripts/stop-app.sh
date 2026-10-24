#!/bin/zsh
# Quits the running TimeTracker of the given binary's build (release or debug) through its socket and waits
# until it is gone; the other build keeps running. `running` lists that build's app processes: its binary
# without a subcommand (the CLI and MCP have one).
# Usage: scripts/stop-app.sh path/to/timetracker
case $1 in
  *"TimeTracker (Debug).app/"*) BUNDLE='TimeTracker \(Debug\)\.app' ;;
  *) BUNDLE='TimeTracker\.app' ;;
esac
running() { pgrep -f "$BUNDLE/Contents/MacOS/timetracker( --hidden)?\$"; }
running >/dev/null || exit 0
"$1" quit >/dev/null 2>&1
for _ in {1..50}; do
  running >/dev/null || exit 0
  sleep 0.1
done
echo "TimeTracker did not quit; quit it from the menu bar and try again" >&2
exit 1
