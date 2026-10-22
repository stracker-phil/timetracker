//! Window titles cleaned before labelling: spinners and dingbats that apps put in front of a title
//! (`◐ Docs`, `✳ Build`, `⠋ npm`) would otherwise split one document into many blocks.

/// Terminal and many editors join the parts of a title with this.
const SEPARATOR: &str = " \u{2014} ";

/// `raw` without the symbols leading it or any of its ` — ` parts. A symbol is any character outside
/// ASCII that is not a letter or digit; ASCII punctuation (`(3) Inbox`, `#general`, `~/src`) stays.
pub(super) fn clean_title(raw: &str) -> String {
    raw.split(SEPARATOR)
        .map(without_leading_symbols)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

fn without_leading_symbols(part: &str) -> &str {
    part.trim_start_matches(|c: char| c.is_whitespace() || (!c.is_ascii() && !c.is_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_spinners_dingbats_and_emoji_are_stripped() {
        for (raw, expected) in [
            ("\u{25D0} Docs", "Docs"),
            ("\u{2733} Build", "Build"),
            ("\u{280B} npm", "npm"),
            ("\u{1F525} Deploy", "Deploy"),
            ("\u{25D0}\u{2733} \u{1F525}Docs", "Docs"),
            ("Docs", "Docs"),
            ("", ""),
        ] {
            assert_eq!(clean_title(raw), expected, "{raw:?}");
        }
    }

    #[test]
    fn each_part_of_a_title_joined_by_an_em_dash_is_cleaned() {
        for (raw, expected) in [
            (
                "timetracker \u{2014} \u{25D0} Docs",
                "timetracker \u{2014} Docs",
            ),
            (
                "timetracker \u{2014} \u{25D1} Docs",
                "timetracker \u{2014} Docs",
            ),
            (
                "\u{2733} a \u{2014} \u{280B} b \u{2014} \u{1F525} c",
                "a \u{2014} b \u{2014} c",
            ),
        ] {
            assert_eq!(clean_title(raw), expected, "{raw:?}");
        }
    }

    #[test]
    fn parts_that_are_only_symbols_vanish_with_their_separator() {
        for (raw, expected) in [
            ("\u{25D0} \u{2014} Docs", "Docs"),
            ("Docs \u{2014} \u{25D0}", "Docs"),
        ] {
            assert_eq!(clean_title(raw), expected, "{raw:?}");
        }
    }

    #[test]
    fn ascii_punctuation_and_non_ascii_letters_stay() {
        for title in [
            "(3) Inbox",
            "#general",
            "~/src",
            "[draft] notes",
            "Docs \u{25D0} later",
            "\u{00DC}ber uns",
            "\u{65E5}\u{672C}\u{8A9E}",
            "2 \u{2014} 3",
        ] {
            assert_eq!(clean_title(title), title, "{title:?}");
        }
    }

    #[test]
    fn a_title_of_only_symbols_becomes_empty() {
        for raw in [
            "\u{25D0}",
            "\u{2733} \u{280B}",
            "\u{1F525}",
            "\u{25D0} \u{2014} \u{25D1}",
        ] {
            assert_eq!(clean_title(raw), "", "{raw:?}");
        }
    }
}
