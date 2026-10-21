//! Private windows recognized by their title.

use super::browsers::{Browser, browser};

const CHROMIUM_SUFFIXES: &[&str] = &[" (Incognito)", " (Private)", " - InPrivate", " — InPrivate"];
const FIREFOX_SUFFIXES: &[&str] = &[
    " — Private Browsing",
    " - Private Browsing",
    " (Private Browsing)",
];

pub fn is_private_title(bundle_id: &str, title: &str) -> bool {
    let suffixes = match browser(bundle_id) {
        Some(Browser::Chromium | Browser::ChromiumWithMode) => CHROMIUM_SUFFIXES,
        Some(Browser::Firefox) => FIREFOX_SUFFIXES,
        Some(Browser::Safari) | None => return false,
    };
    let title = title.trim_end();
    suffixes.iter().any(|suffix| title.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_windows_are_recognized_by_their_browsers_title_suffix() {
        for (bundle_id, title, expected) in [
            ("com.google.Chrome", "New Tab (Incognito)", true),
            ("com.google.Chrome", "Docs (Incognito)  ", true),
            ("com.brave.Browser", "Page (Private)", true),
            ("company.thebrowser.Browser", "Page (Incognito)", true),
            ("com.microsoft.edgemac", "Start - InPrivate", true),
            ("org.mozilla.firefox", "Mozilla — Private Browsing", true),
            ("org.mozilla.firefox", "Mozilla (Private Browsing)", true),
            ("com.google.Chrome", "Inbox - Gmail", false),
            ("com.google.Chrome", "(Incognito) guide", false),
            ("com.microsoft.edgemac", "Start - Microsoft Edge", false),
            ("org.mozilla.firefox", "Mozilla Firefox", false),
            ("org.mozilla.firefox", "Page (Incognito)", false),
            ("com.apple.Safari", "Page (Incognito)", false),
            ("com.apple.Safari", "Page - Private Browsing", false),
            ("com.apple.Terminal", "notes (Incognito)", false),
        ] {
            assert_eq!(
                is_private_title(bundle_id, title),
                expected,
                "{bundle_id}: {title}"
            );
        }
    }
}
