#!/bin/zsh
# Captures the debug app's window, also in the background: screenshot.sh <out.png> [--overlay]
# Picks its largest normal window so sheets don't hide the app; the release app is never captured (its window
# shows real data, ADR 0031). With --overlay it picks the open panel instead (the timer picker or the away
# prompt): the visible window on the highest level above normal windows.
MODE=${2:-window}
WID=$(swift - "$MODE" <<'SWIFT' 2>/dev/null
import CoreGraphics
let overlay = CommandLine.arguments[1] == "--overlay"
let all = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as! [[String: Any]]
// macOS reports a window on another Space as not on screen; then any window of the app counts.
let ownWindows = all.filter { ($0["kCGWindowOwnerName"] as? String) == "TimeTracker (Debug)" }
let onscreen = ownWindows.filter { ($0["kCGWindowIsOnscreen"] as? Bool) == true }
var best: (id: Int, layer: Int, area: Double) = (0, -1, 0)
for w in (onscreen.isEmpty ? ownWindows : onscreen) {
  let layer = w["kCGWindowLayer"] as? Int ?? 0
  if overlay {
    guard layer > 0, (w["kCGWindowAlpha"] as? Double ?? 0) > 0.5 else { continue }
  } else if layer != 0 {
    continue
  }
  let b = w["kCGWindowBounds"] as! [String: Double]
  let area = b["Width"]! * b["Height"]!
  if layer > best.layer || (layer == best.layer && area > best.area) {
    best = (w["kCGWindowNumber"] as! Int, layer, area)
  }
}
print(best.id)
SWIFT
)
[[ -n "$WID" && "$WID" != 0 ]] || { echo "No window of the debug app to capture" >&2; exit 1; }
screencapture -x -o -l "$WID" "$1"
