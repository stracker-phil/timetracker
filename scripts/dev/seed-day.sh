#!/bin/zsh
# Fills a day with synthetic activity through `timetracker activity add --json`, for performance checks:
# `count` spans spread over three sources (`seed-1`..`seed-3`), so lanes and packing get exercised.
# Usage: scripts/dev/seed-day.sh 2020-01-06 [count]
# Remove them again: sqlite3 "<data dir>/timetracker.sqlite" "DELETE FROM activities WHERE source LIKE 'external:seed-%'"
set -eu
day=${1:?usage: seed-day.sh YYYY-MM-DD [count]}
count=${2:-3000}
cli=${TIMETRACKER_CLI:-$(command -v timetracker || echo "src-tauri/target/debug/bundle/macos/TimeTracker.app/Contents/MacOS/timetracker")}
start_ms=$(( $(date -j -f "%Y-%m-%d %H:%M:%S" "$day 00:00:00" +%s) * 1000 ))
python3 -I - "$start_ms" "$count" <<'PY' | "$cli" activity add --json >/dev/null
import json, sys
start, count = int(sys.argv[1]), int(sys.argv[2])
sources = 3
step = 86_400_000 * sources // count
rows = []
for i in range(count):
    source = i % sources
    at = start + (i // sources) * step + source * step // 3
    rows.append({"source": f"seed-{source + 1}", "title": f"Seed block {i}", "app": f"Seed {source + 1}", "start": at, "end": at + step - 1_000})
print(json.dumps(rows))
PY
echo "Added $count spans to $day"
