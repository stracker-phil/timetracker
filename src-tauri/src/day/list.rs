//! The List view: the Day view's blocks grouped by what they show (source, app and label), longest first.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use super::{ActivityBlock, DayView, LaneKind};
use crate::db::time::{Day, Ms};

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ListView {
    pub day: Day,
    /// By total time, longest first; then by first start.
    pub rows: Vec<ListRow>,
    pub total_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ListRow {
    pub key: String,
    pub app: String,
    pub bundle_id: Option<String>,
    /// What the timeline shows for these blocks: the document name or the app, by the title setting.
    pub label: String,
    /// The longest block's window title.
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub source: String,
    pub private: bool,
    pub total_ms: i64,
    /// The part of `total_ms` inside recordings.
    pub covered_ms: i64,
    pub first_start: Ms,
    pub last_end: Ms,
    /// The blocks' spans, touching ones merged, by start.
    pub spans: Vec<(Ms, Ms)>,
    /// The time between the spans: what a recording from `first_start` to `last_end` adds.
    pub gap_ms: i64,
    pub block_ids: Vec<String>,
}

fn merged(mut spans: Vec<(Ms, Ms)>) -> Vec<(Ms, Ms)> {
    spans.sort_unstable();
    let mut out: Vec<(Ms, Ms)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match out.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

/// The time `spans` and `recorded` share; both merged.
fn shared_ms(spans: &[(Ms, Ms)], recorded: &[(Ms, Ms)]) -> Ms {
    spans
        .iter()
        .flat_map(|&(start, end)| {
            recorded
                .iter()
                .map(move |&(r_start, r_end)| (end.min(r_end) - start.max(r_start)).max(0))
        })
        .sum()
}

/// `None` for an empty group.
fn row_of(key: String, blocks: &[&ActivityBlock], recorded: &[(Ms, Ms)]) -> Option<ListRow> {
    let longest = blocks
        .iter()
        .copied()
        .max_by_key(|b| (b.end - b.start, std::cmp::Reverse(b.start)))?;
    let spans = merged(blocks.iter().map(|b| (b.start, b.end)).collect());
    let total_ms: Ms = spans.iter().map(|(start, end)| end - start).sum();
    let (first_start, last_end) = (spans.first()?.0, spans.last()?.1);
    Some(ListRow {
        key,
        app: longest.app.clone(),
        bundle_id: longest.bundle_id.clone(),
        label: longest.label.clone(),
        title: longest.title.clone(),
        url: longest.url.clone(),
        path: longest.path.clone(),
        source: longest.source.clone(),
        private: longest.private,
        total_ms,
        covered_ms: shared_ms(&spans, recorded),
        first_start,
        last_end,
        gap_ms: last_end - first_start - total_ms,
        spans,
        block_ids: blocks.iter().map(|b| b.id.clone()).collect(),
    })
}

/// The view's blocks (every lane but the summary) grouped as the timeline labels them.
pub fn list(view: &DayView) -> ListView {
    let mut groups: HashMap<String, Vec<&ActivityBlock>> = HashMap::new();
    for block in view
        .lanes
        .iter()
        .filter(|lane| lane.kind != LaneKind::Summary)
        .flat_map(|lane| &lane.blocks)
    {
        let key = format!("{}\u{1}{}\u{1}{}", block.source, block.app_key, block.label);
        groups.entry(key).or_default().push(block);
    }
    let recorded = merged(view.recordings.iter().map(|r| (r.start, r.end)).collect());
    let mut rows: Vec<ListRow> = groups
        .into_iter()
        .filter_map(|(key, blocks)| row_of(key, &blocks, &recorded))
        .collect();
    rows.sort_by(|a, b| {
        b.total_ms
            .cmp(&a.total_ms)
            .then(a.first_start.cmp(&b.first_start))
            .then_with(|| a.key.cmp(&b.key))
    });
    ListView {
        day: view.day,
        total_ms: rows.iter().map(|r| r.total_ms).sum(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{
        activity, day, from_source, hidden, input, mins, private, recording, render, rows,
        settings, with_path, with_timeline, with_url,
    };
    use super::super::{DayInput, ViewSettings, view};
    use super::*;
    use crate::store::settings::Titles;

    fn by_app() -> ViewSettings {
        with_timeline(settings(), |t| t.titles = Titles::App)
    }

    fn rows_of(spec: &[(i64, i64, &str, &str)], settings: &ViewSettings) -> ListView {
        list(&render(rows(spec), settings))
    }

    /// `(label, total_min)` of each row.
    fn totals(list: &ListView) -> Vec<(&str, i64)> {
        list.rows
            .iter()
            .map(|r| (r.label.as_str(), r.total_ms / mins(1)))
            .collect()
    }

    #[test]
    fn a_day_without_activity_lists_nothing() {
        let list = rows_of(&[], &settings());
        assert_eq!(list.day, day());
        assert!(list.rows.is_empty());
        assert_eq!(list.total_ms, 0);
    }

    #[test]
    fn titles_by_app_make_a_whole_app_one_row() {
        let list = rows_of(
            &[
                (0, 10, "Code", "a.rs"),
                (10, 20, "Mail", "Inbox"),
                (20, 30, "Code", "b.rs"),
            ],
            &by_app(),
        );

        assert_eq!(totals(&list), [("Code", 20), ("Mail", 10)]);
        let code = &list.rows[0];
        assert_eq!(code.app, "Code");
        assert_eq!(code.bundle_id.as_deref(), Some("test.code"));
        assert_eq!(code.source, "tracker");
        assert_eq!(code.title, "a.rs");
        assert_eq!(code.spans, [(mins(0), mins(10)), (mins(20), mins(30))]);
        assert_eq!((code.first_start, code.last_end), (mins(0), mins(30)));
        assert_eq!(code.block_ids, ["a1", "a3"]);
    }

    #[test]
    fn document_titles_make_a_row_per_document() {
        let list = rows_of(
            &[
                (0, 10, "Code", "a.rs"),
                (10, 26, "Code", "b.rs"),
                (26, 30, "Code", "a.rs"),
            ],
            &settings(),
        );

        assert_eq!(totals(&list), [("b.rs", 16), ("a.rs", 14)]);
        assert_eq!(
            list.rows[1].spans,
            [(mins(0), mins(10)), (mins(26), mins(30))]
        );
    }

    #[test]
    fn touching_and_overlapping_blocks_merge_into_one_span_counted_once() {
        let list = rows_of(
            &[
                (0, 10, "Code", "a.rs"),
                (10, 20, "Code", "b.rs"),
                (15, 30, "Code", "c.rs"),
            ],
            &by_app(),
        );

        assert_eq!(list.rows.len(), 1);
        assert_eq!(list.rows[0].spans, [(mins(0), mins(30))]);
        assert_eq!(list.rows[0].total_ms, mins(30));
    }

    #[test]
    fn the_gap_is_the_time_between_a_rows_spans() {
        let list = rows_of(
            &[(0, 10, "Code", "a.rs"), (20, 30, "Code", "b.rs")],
            &by_app(),
        );

        assert_eq!(list.rows[0].gap_ms, mins(10));
        let touching = rows_of(
            &[(0, 10, "Code", "a.rs"), (10, 20, "Code", "b.rs")],
            &by_app(),
        );
        assert_eq!(touching.rows[0].gap_ms, 0);
    }

    #[test]
    fn rows_sort_by_total_then_first_start() {
        let list = rows_of(
            &[
                (0, 5, "Mail", ""),
                (5, 15, "Code", ""),
                (15, 20, "Chat", ""),
                (20, 40, "Zed", ""),
            ],
            &by_app(),
        );

        assert_eq!(
            totals(&list),
            [("Zed", 20), ("Code", 10), ("Mail", 5), ("Chat", 5)]
        );
    }

    #[test]
    fn the_total_sums_the_rows() {
        let list = rows_of(
            &[(0, 10, "Code", "a.rs"), (10, 25, "Mail", "Inbox")],
            &settings(),
        );

        assert_eq!(list.total_ms, mins(25));
    }

    #[test]
    fn the_summary_lane_adds_no_rows_and_no_time() {
        let settings = with_timeline(by_app(), |t| t.summary_lane = true);
        let list = rows_of(&[(0, 10, "Code", "a.rs"), (20, 30, "Mail", "")], &settings);

        assert_eq!(totals(&list), [("Code", 10), ("Mail", 10)]);
        assert_eq!(list.total_ms, mins(20));
    }

    #[test]
    fn covered_time_is_the_part_inside_recordings() {
        let acts = rows(&[(0, 10, "Code", "a.rs"), (20, 30, "Code", "b.rs")]);
        let input = DayInput {
            recordings: vec![
                recording("r1", mins(5), Some(mins(15))),
                recording("r2", mins(15), Some(mins(25))),
            ],
            ..input(acts, Vec::new())
        };

        let list = list(&view(&input, &by_app()));

        assert_eq!(list.rows[0].total_ms, mins(20));
        assert_eq!(list.rows[0].covered_ms, mins(10));
    }

    #[test]
    fn without_recordings_nothing_is_covered() {
        let list = rows_of(&[(0, 10, "Code", "a.rs")], &settings());

        assert_eq!(list.rows[0].covered_ms, 0);
    }

    #[test]
    fn the_longest_block_names_the_row() {
        let acts = vec![
            with_url(
                with_path(activity(1, mins(0), Some(mins(5)), "Code", "short"), "/s"),
                "https://short",
            ),
            with_url(
                with_path(activity(2, mins(10), Some(mins(30)), "Code", "long"), "/l"),
                "https://long",
            ),
        ];

        let list = list(&render(acts, &by_app()));

        let row = &list.rows[0];
        assert_eq!(row.label, "Code");
        assert_eq!(row.title, "long");
        assert_eq!(row.url.as_deref(), Some("https://long"));
        assert_eq!(row.path.as_deref(), Some("/l"));
    }

    #[test]
    fn hidden_apps_are_absent() {
        let settings = hidden(by_app(), "test.mail");
        let list = rows_of(
            &[(0, 10, "Code", "a.rs"), (10, 30, "Mail", "Inbox")],
            &settings,
        );

        assert_eq!(totals(&list), [("Code", 10)]);
        assert_eq!(list.total_ms, mins(10));
    }

    #[test]
    fn private_activity_shows_as_private_without_its_details() {
        let acts = vec![
            private(with_url(
                activity(1, mins(0), Some(mins(10)), "Chrome", "Bank login"),
                "https://bank.example.com",
            )),
            activity(2, mins(10), Some(mins(15)), "Chrome", "Docs"),
        ];

        let list = list(&render(acts, &settings()));

        assert_eq!(totals(&list), [("Private", 10), ("Docs", 5)]);
        let row = &list.rows[0];
        assert!(row.private);
        assert_eq!((row.title.as_str(), row.url.as_deref()), ("Private", None));
        assert!(!list.rows[1].private);
    }

    #[test]
    fn another_source_gets_its_own_row() {
        let acts = vec![
            activity(1, mins(0), Some(mins(10)), "git", "commit"),
            from_source(
                activity(2, mins(0), Some(mins(20)), "git", "commit"),
                "external:git",
            ),
        ];

        let list = list(&render(acts, &settings()));

        assert_eq!(
            list.rows
                .iter()
                .map(|r| (r.source.as_str(), r.total_ms))
                .collect::<Vec<_>>(),
            [("external:git", mins(20)), ("tracker", mins(10))]
        );
    }

    #[test]
    fn the_list_serializes_in_the_shape_the_page_uses() {
        let list = rows_of(&[(0, 10, "Code", "a.rs")], &settings());
        let row = &list.rows[0];

        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::json!({
                "day": "2026-01-05",
                "rows": [{
                    "key": row.key,
                    "app": "Code",
                    "bundleId": "test.code",
                    "label": "a.rs",
                    "title": "a.rs",
                    "url": null,
                    "path": null,
                    "source": "tracker",
                    "private": false,
                    "totalMs": mins(10),
                    "coveredMs": 0,
                    "firstStart": 0,
                    "lastEnd": mins(10),
                    "gapMs": 0,
                    "spans": [[0, mins(10)]],
                    "blockIds": ["a1"]
                }],
                "totalMs": mins(10)
            })
        );
    }
}
