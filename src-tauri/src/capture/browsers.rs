//! Which apps are browsers, how to ask each for its address, and how to read the answer.

/// How the tracker reads a browser's address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Safari,
    /// Chromium browsers whose scripting dictionary has a window `mode` ("normal" or "incognito").
    ChromiumWithMode,
    /// Chromium browsers that only report the address.
    Chromium,
    /// Not scriptable: titles only.
    Firefox,
}

pub fn browser(bundle_id: &str) -> Option<Browser> {
    Some(match bundle_id {
        "com.apple.Safari" | "com.apple.SafariTechnologyPreview" => Browser::Safari,
        "com.google.Chrome"
        | "com.google.Chrome.canary"
        | "com.brave.Browser"
        | "com.microsoft.edgemac"
        | "org.chromium.Chromium" => Browser::ChromiumWithMode,
        "company.thebrowser.Browser" | "com.vivaldi.Vivaldi" => Browser::Chromium,
        "org.mozilla.firefox" | "org.mozilla.firefoxdeveloperedition" | "org.mozilla.nightly" => {
            Browser::Firefox
        }
        _ => return None,
    })
}

/// The AppleScript that returns the front window's address (Chromium with mode: the mode, a line feed, then
/// the address). The timeout keeps a hung browser from holding the main thread.
pub fn script(bundle_id: &str) -> Option<String> {
    let body = match browser(bundle_id)? {
        Browser::Safari => "return URL of current tab of front window",
        Browser::ChromiumWithMode => {
            "return (mode of front window) & linefeed & (URL of active tab of front window)"
        }
        Browser::Chromium => "return URL of active tab of front window",
        Browser::Firefox => return None,
    };
    Some(format!(
        "with timeout of 1 second\ntell application id \"{bundle_id}\" to {body}\nend timeout"
    ))
}

/// What a browser's script answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub url: Option<String>,
    pub private: bool,
}

/// Reads a script's answer: the mode line (when the browser reports one), then the address.
pub fn parse(bundle_id: &str, output: &str) -> Page {
    let (mode, address) = match browser(bundle_id) {
        Some(Browser::ChromiumWithMode) => output.split_once('\n').unwrap_or(("", output)),
        _ => ("", output),
    };
    let private = mode.trim().eq_ignore_ascii_case("incognito");
    Page {
        url: if private {
            None
        } else {
            super::url::normalize(address)
        },
        private,
    }
}

/// AppleScript errors the tracker tells apart.
pub const NOT_PERMITTED: i64 = -1743;
pub const NO_WINDOW: i64 = -1728;

/// A failed script: no window means no address; any other failure on Safari is taken as a private window,
/// which Safari does not report otherwise.
pub fn failed(bundle_id: &str, code: i64) -> Page {
    Page {
        url: None,
        private: code != NO_WINDOW
            && code != NOT_PERMITTED
            && browser(bundle_id) == Some(Browser::Safari),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFARI: &str = "com.apple.Safari";
    const CHROME: &str = "com.google.Chrome";
    const ARC: &str = "company.thebrowser.Browser";
    const FIREFOX: &str = "org.mozilla.firefox";

    #[test]
    fn bundle_ids_map_to_their_browser_family() {
        for (bundle_id, expected) in [
            (SAFARI, Some(Browser::Safari)),
            ("com.apple.SafariTechnologyPreview", Some(Browser::Safari)),
            (CHROME, Some(Browser::ChromiumWithMode)),
            ("com.brave.Browser", Some(Browser::ChromiumWithMode)),
            ("com.microsoft.edgemac", Some(Browser::ChromiumWithMode)),
            (ARC, Some(Browser::Chromium)),
            ("com.vivaldi.Vivaldi", Some(Browser::Chromium)),
            (FIREFOX, Some(Browser::Firefox)),
            ("org.mozilla.nightly", Some(Browser::Firefox)),
            ("com.apple.finder", None),
            ("", None),
        ] {
            assert_eq!(browser(bundle_id), expected, "{bundle_id}");
        }
    }

    #[test]
    fn scripts_exist_for_scriptable_browsers_and_name_the_bundle_id() {
        for bundle_id in [SAFARI, CHROME, ARC] {
            let script = script(bundle_id).unwrap();
            assert!(
                script.contains(&format!("tell application id \"{bundle_id}\"")),
                "{bundle_id}: {script}"
            );
            assert!(
                script.starts_with("with timeout of 1 second"),
                "{bundle_id}"
            );
        }
    }

    #[test]
    fn firefox_and_unknown_apps_have_no_script() {
        for bundle_id in [FIREFOX, "com.apple.finder"] {
            assert_eq!(script(bundle_id), None, "{bundle_id}");
        }
    }

    #[test]
    fn chrome_output_carries_the_mode_line_before_the_address() {
        assert_eq!(
            parse(CHROME, "normal\nhttps://x.com/#a"),
            Page {
                url: Some("https://x.com/".into()),
                private: false
            }
        );
    }

    #[test]
    fn an_incognito_chrome_window_has_no_address_and_is_private() {
        assert_eq!(
            parse(CHROME, "incognito\nhttps://x.com"),
            Page {
                url: None,
                private: true
            }
        );
    }

    #[test]
    fn chrome_output_without_a_mode_line_is_read_as_the_address() {
        assert_eq!(
            parse(CHROME, "https://x.com/a"),
            Page {
                url: Some("https://x.com/a".into()),
                private: false
            }
        );
    }

    #[test]
    fn other_browsers_answer_with_the_address_alone() {
        for bundle_id in [SAFARI, ARC] {
            assert_eq!(
                parse(bundle_id, "https://X.com/a#b"),
                Page {
                    url: Some("https://x.com/a".into()),
                    private: false
                },
                "{bundle_id}"
            );
            assert_eq!(
                parse(bundle_id, "about:blank"),
                Page {
                    url: None,
                    private: false
                },
                "{bundle_id}"
            );
        }
    }

    #[test]
    fn a_failed_script_never_has_an_address() {
        for bundle_id in [SAFARI, CHROME, ARC, FIREFOX] {
            for code in [NO_WINDOW, NOT_PERMITTED, -600] {
                assert_eq!(failed(bundle_id, code).url, None, "{bundle_id} {code}");
            }
        }
    }

    #[test]
    fn a_safari_failure_other_than_no_window_or_denied_means_a_private_window() {
        for (code, private) in [
            (-600, true),
            (-1700, true),
            (NO_WINDOW, false),
            (NOT_PERMITTED, false),
        ] {
            assert_eq!(failed(SAFARI, code).private, private, "{code}");
        }
    }

    #[test]
    fn a_failure_of_another_browser_is_never_private() {
        for bundle_id in [CHROME, ARC, FIREFOX] {
            for code in [-600, -1700, NO_WINDOW, NOT_PERMITTED] {
                assert!(!failed(bundle_id, code).private, "{bundle_id} {code}");
            }
        }
    }
}
