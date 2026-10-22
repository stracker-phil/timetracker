#!/bin/zsh
# Captures one view in each scheme of the running debug app (docs/ux.md, Scheme audit), in the current macOS
# mode: the app follows macOS, so switch the system's appearance and run it again for the other mode.
# screenshot-looks.sh <out-dir> [view]  writes <out-dir>/<scheme>-<mode>-<view>.png (view: day by default).
# The settings are restored afterwards.
DIR=${1:?usage: screenshot-looks.sh <out-dir> [view]}
VIEW=${2:-day}
HERE=${0:A:h}
MODE=light
[[ $(defaults read -g AppleInterfaceStyle 2>/dev/null) == Dark ]] && MODE=dark
mkdir -p "$DIR"
"$HERE/js.sh" "window.__looksBefore = window.timetracker.getState().settings; window.timetracker.getState().setView('$VIEW')"
for scheme in default light dark; do
  "$HERE/js.sh" "(() => { const s = window.timetracker.getState(); void s.saveSettings({ ...s.settings, ${MODE}Scheme: '$scheme' }, { quiet: true }); })()"
  sleep 1
  "$HERE/screenshot.sh" "$DIR/$scheme-$MODE-$VIEW.png"
done
"$HERE/js.sh" "void window.timetracker.getState().saveSettings(window.__looksBefore, { quiet: true })"
