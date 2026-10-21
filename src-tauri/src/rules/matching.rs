//! Conditions compiled once per view and evaluated in memory against activity blocks.

use super::types::{ConditionInput, Rule, RuleField, RuleOp};
use crate::day::ActivityBlock;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Any,
    One,
    Char(char),
}

/// A condition's value, lowercased.
#[derive(Debug, Clone, PartialEq)]
enum Matcher {
    Contains(String),
    Is(String),
    BeginsWith(String),
    Pattern(Vec<Token>),
}

impl Matcher {
    fn new(op: RuleOp, value: &str) -> Self {
        let value = value.to_lowercase();
        match op {
            RuleOp::Contains => Self::Contains(value),
            RuleOp::Is => Self::Is(value),
            RuleOp::BeginsWith => Self::BeginsWith(value),
            RuleOp::Pattern => Self::Pattern(
                value
                    .chars()
                    .map(|c| match c {
                        '*' => Token::Any,
                        '?' => Token::One,
                        c => Token::Char(c),
                    })
                    .collect(),
            ),
        }
    }

    /// `value` is lowercased. An empty value only matches `Is ""`.
    fn matches(&self, value: &str) -> bool {
        if value.is_empty() {
            return matches!(self, Self::Is(v) if v.is_empty());
        }
        match self {
            Self::Contains(v) => value.contains(v.as_str()),
            Self::Is(v) => value == v,
            Self::BeginsWith(v) => value.starts_with(v.as_str()),
            Self::Pattern(tokens) => glob(tokens, &value.chars().collect::<Vec<_>>()),
        }
    }
}

/// Backtracks only to the last `*`, so it stays linear in practice.
fn glob(tokens: &[Token], text: &[char]) -> bool {
    let (mut t, mut p) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match tokens.get(p) {
            Some(Token::Any) => {
                star = Some((p, t));
                p += 1;
            }
            Some(Token::One) => (p, t) = (p + 1, t + 1),
            Some(Token::Char(c)) if *c == text[t] => (p, t) = (p + 1, t + 1),
            _ => match star {
                Some((star_p, star_t)) => {
                    star = Some((star_p, star_t + 1));
                    (p, t) = (star_p + 1, star_t + 1);
                }
                None => return false,
            },
        }
    }
    tokens[p..].iter().all(|token| *token == Token::Any)
}

/// A rule with its conditions compiled; matching reads only this.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    pub rule_id: String,
    pub rule_name: String,
    pub project_id: String,
    pub note: String,
    pub tag_ids: Vec<String>,
    matchers: Vec<(RuleField, Matcher)>,
}

impl Compiled {
    pub fn new(rule: &Rule) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            project_id: rule.project_id.clone(),
            note: rule.note.clone(),
            tag_ids: rule.tag_ids.clone(),
            matchers: rule
                .conditions
                .iter()
                .map(|c| (c.field, Matcher::new(c.op, &c.value)))
                .collect(),
        }
    }

    /// Only `conditions`, for testing them before the rule exists.
    pub fn conditions(conditions: &[ConditionInput]) -> Self {
        Self {
            rule_id: String::new(),
            rule_name: String::new(),
            project_id: String::new(),
            note: String::new(),
            tag_ids: Vec::new(),
            matchers: conditions
                .iter()
                .map(|c| (c.field, Matcher::new(c.op, &c.value)))
                .collect(),
        }
    }
}

/// The enabled rules in position order.
pub fn compile(rules: &[Rule]) -> Vec<Compiled> {
    let mut enabled: Vec<&Rule> = rules.iter().filter(|r| r.enabled).collect();
    enabled.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
    enabled.into_iter().map(Compiled::new).collect()
}

/// Every condition holds. A private block, or a rule without conditions, never matches.
pub fn matches(rule: &Compiled, block: &ActivityBlock) -> bool {
    !block.private
        && !rule.matchers.is_empty()
        && rule
            .matchers
            .iter()
            .all(|(field, matcher)| matcher.matches(&field_value(block, *field)))
}

pub fn first_match<'a>(rules: &'a [Compiled], block: &ActivityBlock) -> Option<&'a Compiled> {
    rules.iter().find(|rule| matches(rule, block))
}

/// Lowercased; "" when the block has none.
fn field_value(block: &ActivityBlock, field: RuleField) -> String {
    let url = block.url.as_deref().unwrap_or("").to_lowercase();
    match field {
        RuleField::App => block.app.to_lowercase(),
        RuleField::Title => block.title.to_lowercase(),
        RuleField::Url => url,
        RuleField::Domain => domain(&url).to_string(),
        RuleField::Path => block.path.as_deref().unwrap_or("").to_lowercase(),
        RuleField::Source => block.source.to_lowercase(),
    }
}

/// The host of a lowercased URL, without user info, port and a leading `www.`.
fn domain(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next().unwrap_or("");
    host.strip_prefix("www.").unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing::*;
    use RuleField::*;
    use RuleOp::*;

    fn matched(conditions: &[Spec], block: &ActivityBlock) -> bool {
        matches(&Compiled::new(&rule("r", "p", conditions)), block)
    }

    fn page() -> ActivityBlock {
        with_path(
            with_url(
                block(0, 10, "Slack", "Weekly Standup"),
                "https://www.Example.com:8080/Docs?q=1",
            ),
            "/Users/me/Notes.txt",
        )
    }

    #[test]
    fn each_op_reads_its_field_of_the_block() {
        let cases = [
            (App, Is, "slack", true),
            (App, Is, "SLACK", true),
            (App, Is, "slac", false),
            (App, Contains, "LAC", true),
            (App, Contains, "slackk", false),
            (App, BeginsWith, "sl", true),
            (App, BeginsWith, "lack", false),
            (Title, Contains, "standup", true),
            (Title, Is, "weekly standup", true),
            (Title, Is, "standup", false),
            (Title, Is, "", false),
            (Title, BeginsWith, "WEEKLY", true),
            (Url, Is, "https://www.example.com:8080/docs?q=1", true),
            (Url, Contains, "?Q=1", true),
            (Url, BeginsWith, "https://", true),
            (Domain, Is, "example.com", true),
            (Domain, Is, "www.example.com", false),
            (Domain, Contains, "ample", true),
            (Domain, BeginsWith, "example", true),
            (Path, BeginsWith, "/users/me", true),
            (Path, Contains, "notes", true),
            (Source, Is, "tracker", true),
            (Source, Is, "external:calendar", false),
        ];
        for (field, op, value, expected) in cases {
            assert_eq!(
                matched(&[(field, op, value)], &page()),
                expected,
                "{field:?} {op:?} {value:?}"
            );
        }
    }

    #[test]
    fn matching_ignores_case_beyond_ascii() {
        let block = block(0, 10, "ÄRGER", "");

        assert!(matched(&[(App, Is, "ärger")], &block));
        assert!(matched(&[(App, Contains, "Ä")], &block));
    }

    #[test]
    fn patterns_are_globs_anchored_at_both_ends() {
        let cases = [
            ("*", "anything", true),
            ("weekly*", "Weekly Standup", true),
            ("*standup", "Weekly Standup", true),
            ("*stand*", "Weekly Standup", true),
            ("eekly*", "Weekly Standup", false),
            ("weekly", "Weekly Standup", false),
            ("a*b*c", "aXXbYYc", true),
            ("a*b*c", "aXXbYY", false),
            ("*ab", "aab", true),
            ("**a", "a", true),
            ("*a*", "b", false),
            ("a?c", "abc", true),
            ("a?c", "ac", false),
            ("a?c", "abbc", false),
            ("?rger", "Ärger", true),
            ("standup", "STANDUP", true),
            ("stand", "standup", false),
        ];
        for (pattern, title, expected) in cases {
            assert_eq!(
                matched(&[(Title, Pattern, pattern)], &block(0, 10, "App", title)),
                expected,
                "{pattern:?} on {title:?}"
            );
        }
    }

    #[test]
    fn the_domain_is_the_host_without_www_user_info_and_port() {
        let cases = [
            ("https://www.example.com/a", "example.com"),
            ("HTTP://WWW.Example.COM", "example.com"),
            ("http://user:pw@www.example.com:8080/x", "example.com"),
            ("example.com/path", "example.com"),
            ("https://sub.example.com?x=1", "sub.example.com"),
            ("https://example.com#top", "example.com"),
            ("https://wwwx.com/", "wwwx.com"),
        ];
        for (url, domain) in cases {
            let block = with_url(block(0, 10, "Safari", ""), url);
            assert!(matched(&[(Domain, Is, domain)], &block), "{url}");
        }
    }

    #[test]
    fn an_empty_block_value_only_matches_is_empty() {
        let bare = block(0, 10, "Slack", "");
        let cases = [
            (Title, Is, "", true),
            (Url, Is, "", true),
            (Domain, Is, "", true),
            (Path, Is, "", true),
            (Title, Contains, "", false),
            (Title, BeginsWith, "", false),
            (Title, Pattern, "*", false),
            (Url, Contains, "x", false),
            (Domain, Contains, "e", false),
            (Path, BeginsWith, "/", false),
            (App, Is, "", false),
        ];
        for (field, op, value, expected) in cases {
            assert_eq!(
                matched(&[(field, op, value)], &bare),
                expected,
                "{field:?} {op:?} {value:?}"
            );
        }
    }

    #[test]
    fn every_condition_must_hold() {
        let block = block(0, 10, "Slack", "Weekly Standup");
        let cases = [
            (vec![(App, Is, "slack"), (Title, Contains, "standup")], true),
            (vec![(App, Is, "slack"), (Title, Contains, "retro")], false),
            (vec![(App, Is, "mail"), (Title, Contains, "standup")], false),
        ];
        for (conditions, expected) in cases {
            assert_eq!(matched(&conditions, &block), expected, "{conditions:?}");
        }
    }

    #[test]
    fn a_rule_without_conditions_never_matches() {
        assert!(!matched(&[], &block(0, 10, "Slack", "")));
    }

    #[test]
    fn a_private_block_never_matches() {
        let hidden = private(block(0, 10, "Slack", ""));

        assert!(!matched(&[(App, Is, "slack")], &hidden));
        assert!(!matched(&[(Title, Is, "")], &hidden));
    }

    #[test]
    fn compile_keeps_enabled_rules_in_position_order() {
        let late = Rule {
            position: 2,
            ..app_is("late", "p", "Slack")
        };
        let early = Rule {
            position: 1,
            ..app_is("early", "p", "Slack")
        };
        let off = Rule {
            position: 0,
            enabled: false,
            ..app_is("off", "p", "Slack")
        };

        let compiled = compile(&[late, off, early]);

        let ids: Vec<&str> = compiled.iter().map(|c| c.rule_id.as_str()).collect();
        assert_eq!(ids, ["early", "late"]);
    }

    #[test]
    fn the_first_rule_by_position_that_matches_wins() {
        let slack_in_a = Rule {
            position: 0,
            ..rule(
                "a",
                "p1",
                &[(App, Is, "Slack"), (Title, Contains, "standup")],
            )
        };
        let any_slack = Rule {
            position: 1,
            ..app_is("b", "p2", "Slack")
        };
        let compiled = compile(&[any_slack, slack_in_a]);
        let first = |block: &ActivityBlock| {
            first_match(&compiled, block).map(|c| (c.rule_id.as_str(), c.project_id.as_str()))
        };

        assert_eq!(first(&block(0, 10, "Slack", "Standup")), Some(("a", "p1")));
        assert_eq!(first(&block(0, 10, "Slack", "Retro")), Some(("b", "p2")));
        assert_eq!(first(&block(0, 10, "Mail", "Standup")), None);
    }

    #[test]
    fn a_disabled_rule_is_skipped_for_the_next_one() {
        let off = Rule {
            enabled: false,
            ..app_is("off", "p1", "Slack")
        };
        let on = Rule {
            position: 1,
            ..app_is("on", "p2", "Slack")
        };
        let compiled = compile(&[off, on]);

        let found = first_match(&compiled, &block(0, 10, "Slack", ""));

        assert_eq!(found.map(|c| c.rule_id.as_str()), Some("on"));
    }
}
