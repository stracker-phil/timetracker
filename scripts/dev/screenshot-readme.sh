#!/bin/zsh
# Captures the README screenshots from the running debug app, seeded with `timetracker dev seed --showcase`
# (best on an empty debug data folder): the Day view of the last workday in the Default and the Dark scheme,
# and Reports for last week. Writes day.png, day-dark.png and reports.png to <out-dir> (default
# docs/screens); the settings, view and zoom are restored afterwards.
set -e
HERE=${0:A:h}
DIR=${1:-$HERE/../../docs/screens}
mkdir -p "$DIR"
day=$(date +%Y-%m-%d)
while true; do
  day=$(date -j -v-1d -f %Y-%m-%d "$day" +%Y-%m-%d)
  [[ $(date -j -f %Y-%m-%d "$day" +%u) -le 5 ]] && break
done
ZOOM=1.5
# Scrolls the timeline so 08:24 is at the top.
SCROLL="document.querySelectorAll('*').forEach(e => { if (e.scrollHeight > 1800 && e.clientHeight > 300 && /auto|scroll/.test(getComputedStyle(e).overflowY)) e.scrollTop = 8.4 * 60 * $ZOOM; })"
scheme() {
  "$HERE/js.sh" "(() => { const s = window.timetracker.getState(); void s.saveSettings({ ...s.settings, lightScheme: '$1', darkScheme: '$1' }, { quiet: true }); })()"
}

"$HERE/js.sh" "(() => { const s = window.timetracker.getState(); window.__readmeBefore = { settings: s.settings, zoom: s.uiState.timelineZoom, view: s.view }; s.setTimelineZoom($ZOOM); s.goToDay('$day'); })()"
for name in default:day dark:day-dark; do
  scheme ${name%%:*}
  sleep 1.5
  "$HERE/js.sh" "$SCROLL"
  sleep 0.5
  "$HERE/screenshot.sh" "$DIR/${name#*:}.png"
done
scheme default
"$HERE/js.sh" "(() => { const s = window.timetracker.getState(); s.setView('reports'); s.setReportFilter({ range: { kind: 'lastWeek' } }); })()"
sleep 2
"$HERE/screenshot.sh" "$DIR/reports.png"
"$HERE/js.sh" "(() => { const s = window.timetracker.getState(), b = window.__readmeBefore; void s.saveSettings(b.settings, { quiet: true }); if (b.zoom) s.setTimelineZoom(b.zoom); s.setView(b.view); })()"
echo "Wrote day.png, day-dark.png and reports.png ($day) to $DIR"
