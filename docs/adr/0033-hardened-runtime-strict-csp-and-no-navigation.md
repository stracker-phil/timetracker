# 0033. Hardened runtime with Apple Events, strict CSP and no navigation

## Status

Accepted (2026-11-04), implemented. Inherited from Taskboard ADR 0049; the Apple Events entitlement is
TimeTracker's own. Tightens [0013](0013-no-network.md).

## Context

TimeTracker renders text it doesn't control: every window title, document path and browser address it
captures, in a webview that can call app commands. The app had no hardened runtime, its CSP named only
`default-src`, `img-src`, `style-src` and `connect-src`, and nothing stopped a webview from navigating away.

## Decision

- **Hardened runtime.** `scripts/sign-app.sh` signs with `--options runtime` and
  `src-tauri/entitlements.plist`. The plist holds one entitlement,
  `com.apple.security.automation.apple-events`: without it the hardened runtime refuses the browser address
  lookup (`capture/browsers.rs`, `NSAppleScript` error -1743) without a prompt. `NSAppleEventsUsageDescription`
  stays in `Info.plist`. The app isn't sandboxed.
- **Stricter CSP**, in `csp` and `devCsp`: `script-src 'self'` (the dev server keeps `'unsafe-inline'`),
  `style-src-elem 'self'`, and `'none'` for `frame-src`, `object-src`, `base-uri` and `form-action`. `style`
  attributes stay allowed because React sets them, and `img-src 'self' data:` stays for app icons.
- **No navigation.** A plugin on every webview (`window/navigation.rs` `guard`, `is_app_page`) allows only
  the bundled pages and, in debug builds, the dev server. It blocks and logs anything else. That covers the
  main window, the timer picker and the away prompt.
- Rejected:
  - the App Sandbox, because capture reads other apps' windows and scripts browsers;
  - the hardened runtime without the entitlement, because addresses would be lost silently;
  - a CSP only, because it doesn't stop a top-level navigation.

## Consequences

- Inline `<style>` and `<script>` elements are blocked, so styles live in `src/styles/`.
- A new entitlement needs a reason here and a line in the plist.
- There is no notarization.
