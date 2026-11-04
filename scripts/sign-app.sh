#!/bin/zsh
# Signs a built TimeTracker bundle so macOS keeps its privacy grants across rebuilds (ADR 0032). Uses
# $TIMETRACKER_SIGN_IDENTITY, else a valid "Apple Development" certificate (Xcode → Settings → Accounts →
# Manage Certificates; a free Apple ID is enough), else the "TimeTracker Local Signing" certificate of
# scripts/create-signing-cert.sh, else signs ad hoc (the grants then last one build).
# Signs with the hardened runtime and the Apple Events entitlement, which the browser address lookup needs
# (ADR 0033). Usage: scripts/sign-app.sh path/to/App.app bundle.identifier
set -e
APP=$1 IDENTIFIER=$2
ENTITLEMENTS="${0:A:h}/../src-tauri/entitlements.plist"
IDENTITY=${TIMETRACKER_SIGN_IDENTITY:-}
if [[ -z "$IDENTITY" ]]; then
  # By hash: an expired certificate of the same name would make the name ambiguous.
  IDENTITY=$(security find-identity -v -p codesigning | awk '/"Apple Development: / { print $2; exit }')
fi
if [[ -z "$IDENTITY" ]] && security find-identity -p codesigning | grep -q '"TimeTracker Local Signing"'; then
  IDENTITY="TimeTracker Local Signing"
fi
codesign --force --options runtime --entitlements "$ENTITLEMENTS" --sign "${IDENTITY:--}" \
  --identifier "$IDENTIFIER" "$APP" 2>/dev/null
[[ -n "$IDENTITY" ]] || echo "Signed ad hoc: grant Accessibility again after each rebuild (see scripts/sign-app.sh)"
