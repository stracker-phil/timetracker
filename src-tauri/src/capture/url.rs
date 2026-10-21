//! Addresses and paths as the tracker stores them.

use url::Url;

/// Pages that say nothing about the work: a blank tab or a browser's new-tab page.
const EMPTY_PAGES: &[&str] = &[
    "about:blank",
    "about:newtab",
    "about:home",
    "chrome://newtab",
    "edge://newtab",
    "arc://newtab",
];

/// A browser address for storage: a URL loses its fragment and gets a lower-case scheme and host. `None` for
/// blank and new-tab pages, and for text that is neither a URL nor starts with `http` or `file`.
pub fn normalize(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let Ok(mut url) = Url::parse(raw) else {
        let lower = raw.to_ascii_lowercase();
        return (lower.starts_with("http") || lower.starts_with("file")).then(|| raw.to_string());
    };
    url.set_fragment(None);
    let text = url.to_string();
    let bare = text.trim_end_matches('/');
    if EMPTY_PAGES.contains(&bare) {
        return None;
    }
    Some(text)
}

/// The lower-case host of `url` without a leading `www.`.
pub fn domain(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// The path of a `file://` URL (as `AXDocument` reports it), percent-decoded.
pub fn file_path(value: &str) -> Option<String> {
    let url = Url::parse(value.trim()).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    let path = url.to_file_path().ok()?;
    Some(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_the_fragment_and_keeps_the_query() {
        for (raw, expected) in [
            ("https://x.com/page#section", "https://x.com/page"),
            (
                "https://x.com/page?q=1&r=2#top",
                "https://x.com/page?q=1&r=2",
            ),
            ("https://x.com#a", "https://x.com/"),
        ] {
            assert_eq!(normalize(raw).as_deref(), Some(expected), "{raw}");
        }
    }

    #[test]
    fn normalize_lowercases_scheme_and_host_but_not_the_path() {
        assert_eq!(
            normalize("HTTPS://Example.COM/Path/A").as_deref(),
            Some("https://example.com/Path/A")
        );
    }

    #[test]
    fn normalize_trims_surrounding_whitespace() {
        assert_eq!(
            normalize("  https://x.com/a \n").as_deref(),
            Some("https://x.com/a")
        );
    }

    #[test]
    fn normalize_keeps_file_urls() {
        assert_eq!(
            normalize("file:///Users/me/a.txt").as_deref(),
            Some("file:///Users/me/a.txt")
        );
    }

    #[test]
    fn normalize_drops_empty_and_new_tab_pages() {
        for raw in [
            "about:blank",
            "about:newtab",
            "chrome://newtab/",
            "chrome://newtab",
            "edge://newtab/",
            "arc://newtab/",
        ] {
            assert_eq!(normalize(raw), None, "{raw}");
        }
    }

    #[test]
    fn normalize_drops_blank_text_and_text_that_is_not_a_url() {
        for raw in ["", "   ", "\n", "not a url", "missing value"] {
            assert_eq!(normalize(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn domain_is_the_lowercase_host_without_www_or_port() {
        for (url, expected) in [
            ("https://www.example.com/a", "example.com"),
            ("https://example.com:8080/a", "example.com"),
            ("https://WWW.Example.COM:8443/a?q=1", "example.com"),
            ("https://login.bank.example.com/x", "login.bank.example.com"),
            ("http://192.168.0.1:3000/", "192.168.0.1"),
        ] {
            assert_eq!(domain(url).as_deref(), Some(expected), "{url}");
        }
    }

    #[test]
    fn domain_of_something_without_a_host_is_none() {
        for url in ["", "not a url", "about:blank", "example.com"] {
            assert_eq!(domain(url), None, "{url:?}");
        }
    }

    #[test]
    fn file_path_decodes_percent_escapes() {
        for (value, expected) in [
            ("file:///Users/me/My%20Doc.txt", "/Users/me/My Doc.txt"),
            ("file:///Users/me/a.txt", "/Users/me/a.txt"),
            ("  file:///Users/me/a.txt ", "/Users/me/a.txt"),
        ] {
            assert_eq!(file_path(value).as_deref(), Some(expected), "{value:?}");
        }
    }

    #[test]
    fn file_path_of_other_schemes_and_garbage_is_none() {
        for value in ["https://x.com/a.txt", "/Users/me/a.txt", "", "not a url"] {
            assert_eq!(file_path(value), None, "{value:?}");
        }
    }
}
