#!/bin/zsh
# Captures one view in all six Mode × Scheme combinations of the running debug app (docs/ux.md, Scheme audit):
# screenshot-looks.sh <out-dir> [view]  writes <out-dir>/<scheme>-<mode>-<view>.png (view: day by default).
# The settings are restored afterwards.
DIR=${1:?usage: screenshot-looks.sh <out-dir> [view]}
VIEW=${2:-day}
HERE=${0:A:h}
mkdir -p "$DIR"
"$HERE/js.sh" "window.__looksBefore = window.timetracker.getState().settings; window.timetracker.getState().setView('$VIEW')"
for scheme in default light dark; do
  for mode in light dark; do
    "$HERE/js.sh" "(() => { const s = window.timetracker.getState(); void s.saveSettings({ ...s.settings, mode: '$mode', scheme: '$scheme' }, { quiet: true }); })()"
    sleep 1
    "$HERE/screenshot.sh" "$DIR/$scheme-$mode-$VIEW.png"
  done
done
"$HERE/js.sh" "void window.timetracker.getState().saveSettings(window.__looksBefore, { quiet: true })"
