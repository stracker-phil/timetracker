#!/bin/zsh
# Checks that nothing jumps in the running debug app: e2e.sh [day] (default: the last workday, in the showcase).
# Opens the Day view, clicks View options and the first summary block twice each, and measures the main area,
# the toolbar, the first summary block and the clicked control after every click. Exits 1 on any drift of 1px
# or more (.claude/rules/stable-controls.md). Start the app first: bun run build:app && scripts/dev/launch.sh
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN="$ROOT/src-tauri/target/debug/bundle/macos/TimeTracker (Debug).app/Contents/MacOS/timetracker"
LOG="${TMPDIR:-/tmp}/timetracker-dev.log"
MARK="E2E-$(date +%s)-$$"
"$BIN" status >/dev/null 2>&1 || { echo "The debug app isn't running: scripts/dev/launch.sh"; exit 1; }
DAY=${1:-}
if [[ -z "$DAY" ]]; then
  DAY=$(date +%Y-%m-%d)
  while true; do
    DAY=$(date -j -v-1d -f %Y-%m-%d "$DAY" +%Y-%m-%d)
    [[ $(date -j -f %Y-%m-%d "$DAY" +%u) -le 5 ]] && break
  done
fi
"$BIN" show "$DAY" >/dev/null
sleep 1.5

"$ROOT/scripts/dev/js.sh" "(async () => {
  const report = (text) => window.__TAURI_INTERNALS__.invoke('log_frontend_error', { message: '$MARK ' + text });
  const wait = (ms) => new Promise((done) => setTimeout(done, ms));
  const rect = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return [r.x, r.y, r.width, r.height].map(Math.round).join(',');
  };
  const firstBlock = () => document.querySelector('.summary-block[data-block-id]');
  const parts = (control) => ({
    main: rect(document.querySelector('.main')),
    toolbar: rect(document.querySelector('.toolbar')),
    block: rect(firstBlock()),
    control: rect(control),
  });
  const drift = [];
  const compare = (step, before, after) => {
    for (const name of Object.keys(before)) {
      // A day without activity has no block; only a block on both sides is compared.
      if (name === 'block' && (before.block === null || after.block === null)) continue;
      if (before[name] !== after[name]) drift.push(step + ': ' + name + ' ' + before[name] + ' -> ' + after[name]);
    }
  };
  const clickTwice = async (name, find) => {
    const control = find();
    if (!control) return drift.push(name + ': not found');
    const before = parts(control);
    control.click();
    await wait(700);
    compare(name + ' once', before, parts(control));
    control.click();
    await wait(700);
    compare(name + ' twice', before, parts(control));
  };
  try {
    await clickTwice('View options', () => document.querySelector('[aria-label=\"View options\"]'));
    await clickTwice('Summary block', firstBlock);
    report(drift.length === 0 ? 'OK' : 'DRIFT ' + drift.join(' | '));
  } catch (error) {
    report('ERROR ' + error);
  }
})()"

for _ in {1..30}; do
  sleep 0.5
  line=$(grep -a -o "$MARK [^\"]*" "$LOG" | head -1) && [[ -n "$line" ]] && break
done
result=${line#"$MARK "}
case "$result" in
  OK) echo "No drift"; exit 0 ;;
  "") echo "No result in $LOG"; exit 1 ;;
  *) echo "${result//' | '/$'\n'}"; exit 1 ;;
esac
