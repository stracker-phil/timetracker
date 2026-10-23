#!/bin/zsh
# Signs a built TimeTracker bundle so macOS keeps its Accessibility grant across rebuilds: the grant belongs
# to the identifier and the signing certificate, not to the build. Uses $TIMETRACKER_SIGN_IDENTITY, else the
# "TimeTracker Local Signing" certificate of scripts/create-signing-cert.sh, else signs ad hoc (the grant
# then lasts one build). Usage: scripts/sign-app.sh path/to/App.app bundle.identifier
set -e
APP=$1 IDENTIFIER=$2
IDENTITY=${TIMETRACKER_SIGN_IDENTITY:-}
if [[ -z "$IDENTITY" ]] && security find-identity -p codesigning | grep -q '"TimeTracker Local Signing"'; then
  IDENTITY="TimeTracker Local Signing"
fi
codesign --force --sign "${IDENTITY:--}" --identifier "$IDENTIFIER" "$APP" 2>/dev/null
[[ -n "$IDENTITY" ]] || echo "Signed ad hoc: grant Accessibility again after each rebuild (scripts/create-signing-cert.sh fixes that)"
