//! What the tracker may write: exclusions and private windows are applied before any row exists.

use crate::capture::Snapshot;
use crate::capture::url::domain;
use crate::store::settings::CaptureSettings;

pub const PRIVATE_TITLE: &str = "Private";

/// The app (by bundle id, or by name without one) or the site (the domain or any subdomain of it) is
/// excluded. Entries are lower case (`Settings::sanitized`).
pub fn excluded(snapshot: &Snapshot, capture: &CaptureSettings) -> bool {
    let app = snapshot
        .bundle_id
        .as_deref()
        .unwrap_or(&snapshot.app)
        .to_lowercase();
    if capture.excluded_apps.contains(&app) {
        return true;
    }
    let Some(host) = snapshot.url.as_deref().and_then(domain) else {
        return false;
    };
    capture
        .excluded_domains
        .iter()
        .any(|entry| host == *entry || host.ends_with(&format!(".{entry}")))
}

/// Drops what the settings turn off, and everything but the app of a private window.
pub fn redact(mut snapshot: Snapshot, capture: &CaptureSettings) -> Snapshot {
    if snapshot.private {
        snapshot.title = PRIVATE_TITLE.into();
        snapshot.url = None;
        snapshot.path = None;
    }
    if !capture.capture_urls {
        snapshot.url = None;
    }
    if !capture.capture_documents {
        snapshot.path = None;
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::fake::snapshot;

    fn capture(apps: &[&str], domains: &[&str]) -> CaptureSettings {
        CaptureSettings {
            excluded_apps: apps.iter().map(|a| a.to_string()).collect(),
            excluded_domains: domains.iter().map(|d| d.to_string()).collect(),
            ..CaptureSettings::default()
        }
    }

    fn at(app: &str, url: &str) -> Snapshot {
        Snapshot {
            url: Some(url.into()),
            ..snapshot(app, "Page")
        }
    }

    #[test]
    fn an_app_is_excluded_by_its_bundle_id() {
        let capture = capture(&["test.code"], &[]);
        assert!(excluded(&snapshot("Code", "a.rs"), &capture));
        assert!(!excluded(&snapshot("Mail", "Inbox"), &capture));
    }

    #[test]
    fn without_a_bundle_id_the_app_name_is_matched_case_insensitively() {
        let capture = capture(&["code"], &[]);
        for name in ["Code", "CODE", "code"] {
            let snapshot = Snapshot {
                bundle_id: None,
                ..snapshot(name, "a.rs")
            };
            assert!(excluded(&snapshot, &capture), "{name}");
        }
    }

    #[test]
    fn a_bundle_id_is_matched_case_insensitively() {
        let snapshot = Snapshot {
            bundle_id: Some("Com.Example.App".into()),
            ..snapshot("App", "x")
        };
        assert!(excluded(&snapshot, &capture(&["com.example.app"], &[])));
    }

    #[test]
    fn with_a_bundle_id_the_app_name_does_not_match() {
        assert!(!excluded(
            &snapshot("Code", "a.rs"),
            &capture(&["code"], &[])
        ));
    }

    #[test]
    fn a_domain_and_its_subdomains_are_excluded_but_lookalikes_are_not() {
        let capture = capture(&[], &["bank.example.com"]);
        for (url, expected) in [
            ("https://bank.example.com/", true),
            ("https://www.bank.example.com/", true),
            ("https://login.bank.example.com/x", true),
            ("https://notbank.example.com/", false),
            ("https://example.com/", false),
            ("https://example.com/bank.example.com", false),
        ] {
            assert_eq!(excluded(&at("Browser", url), &capture), expected, "{url}");
        }
    }

    #[test]
    fn nothing_is_excluded_by_default() {
        let capture = CaptureSettings::default();
        assert!(!excluded(&at("Code", "https://example.com/"), &capture));
    }

    #[test]
    fn redact_replaces_a_private_window_with_the_app_alone() {
        let private = Snapshot {
            private: true,
            url: Some("https://secret.example.com/".into()),
            path: Some("/Users/me/secret.txt".into()),
            ..snapshot("Chrome", "Secret")
        };

        let redacted = redact(private, &CaptureSettings::default());

        assert_eq!(
            redacted,
            Snapshot {
                private: true,
                title: PRIVATE_TITLE.into(),
                url: None,
                path: None,
                ..snapshot("Chrome", "Secret")
            }
        );
    }

    #[test]
    fn redact_drops_urls_and_paths_the_settings_turn_off() {
        let full = Snapshot {
            url: Some("https://example.com/".into()),
            path: Some("/Users/me/a.txt".into()),
            ..snapshot("Code", "a")
        };
        for (urls, documents, url, path) in [
            (
                true,
                true,
                Some("https://example.com/"),
                Some("/Users/me/a.txt"),
            ),
            (false, true, None, Some("/Users/me/a.txt")),
            (true, false, Some("https://example.com/"), None),
            (false, false, None, None),
        ] {
            let capture = CaptureSettings {
                capture_urls: urls,
                capture_documents: documents,
                ..CaptureSettings::default()
            };
            let redacted = redact(full.clone(), &capture);
            assert_eq!(redacted.url.as_deref(), url, "{urls} {documents}");
            assert_eq!(redacted.path.as_deref(), path, "{urls} {documents}");
            assert_eq!(redacted.title, "a");
        }
    }
}
