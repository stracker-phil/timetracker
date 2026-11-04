# 0032. Builds are signed with an Apple Development certificate

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0053.

## Context

macOS keys the Accessibility grant (window titles, document paths) and each browser's Automation grant
(addresses) to the app's code signature. Builds were signed ad hoc or with the self-signed "TimeTracker Local
Signing" certificate of `scripts/create-signing-cert.sh`. Ad hoc, every rebuild is a new app that asks again.
With the local certificate the grants hold, but the certificate has no Team ID. Taskboard found that the
Keychain then trusts only one build's hash, and `codesign` asks for the key on every build.

## Decision

- `scripts/sign-app.sh` signs with `$TIMETRACKER_SIGN_IDENTITY`. Otherwise it uses the first valid "Apple
  Development" certificate (made in Xcode; a free Apple ID is enough), picked by its SHA-1 hash because an
  expired certificate may share its name. Otherwise it uses the local certificate, and otherwise it signs ad
  hoc.
- `scripts/create-signing-cert.sh` stays as the fallback. On a second run it only allows `codesign` to use
  the key without asking (`security set-key-partition-list`).
- `install.sh` and `dev/launch.sh` re-sign every bundle after `tauri build` (whose own signature stays ad
  hoc, `bundle.macOS.signingIdentity: "-"`).
- Rejected:
  - the local certificate only (no Team ID, and a password prompt per build);
  - ad hoc only (the grants reset on every rebuild);
  - notarized Developer ID signing (needs a paid membership, and the app is never distributed signed).

## Consequences

- Rebuilds keep their grants and sign without a prompt.
- Switching to the Apple certificate changes the signature once: the Accessibility grant and each browser's
  Automation grant must be given again in System Settings → Privacy & Security, or tracking silently loses
  titles and addresses.
- The certificate expires yearly; Xcode renews it, and the script picks the valid one.
