//! The Reports screen's donuts: the rounded time by client, by project and by tag, as ring slices.

use serde::Serialize;
use ts_rs::TS;

use super::groups::{NO_TAG_KEY, NO_TAG_LABEL};
use super::table::{NO_CLIENT_KEY, NO_CLIENT_LABEL};
use super::{ReportRow, index_of, project_label};
use crate::db::tags::Tag;
use crate::store::settings::ACCENTS;

/// The hue of what has no client or tag.
const NEUTRAL: &str = "graphite";

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReportCharts {
    pub by_client: Vec<ChartSlice>,
    pub by_project: Vec<ChartSlice>,
    pub by_tag: Vec<ChartSlice>,
}

/// One slice of a donut, largest first; "No client" and "Untagged" come last.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ChartSlice {
    pub key: String,
    pub label: String,
    /// An accent name (`ACCENTS`).
    pub color: String,
    /// The project's shade of `color`, only by project.
    pub shade: Option<u8>,
    pub rounded_ms: i64,
    /// Of the total, rounded to a whole percent.
    pub percent: u8,
    /// Where the slice starts and how much of the ring it takes, as fractions of the whole.
    pub offset: f64,
    pub share: f64,
}

pub(super) fn charts(rows: &[ReportRow], tags: &[Tag]) -> ReportCharts {
    ReportCharts {
        by_client: slices(rows, |row| match &row.client_id {
            Some(id) => slice(
                id,
                row.client.as_deref().unwrap_or_default(),
                row.client_color.as_deref().unwrap_or(NEUTRAL),
                None,
            ),
            None => slice(NO_CLIENT_KEY, NO_CLIENT_LABEL, NEUTRAL, None),
        }),
        by_project: slices(rows, |row| {
            slice(
                &row.project_id,
                &project_label(row),
                &row.color,
                Some(row.shade),
            )
        }),
        by_tag: slices(rows, |row| match &row.tag_id {
            Some(id) => slice(
                id,
                row.tag.as_deref().unwrap_or_default(),
                tag_color(tags, id),
                None,
            ),
            None => slice(NO_TAG_KEY, NO_TAG_LABEL, NEUTRAL, None),
        }),
    }
}

/// A slice without its time yet.
fn slice(key: &str, label: &str, color: &str, shade: Option<u8>) -> ChartSlice {
    ChartSlice {
        key: key.into(),
        label: label.into(),
        color: color.into(),
        shade,
        rounded_ms: 0,
        percent: 0,
        offset: 0.0,
        share: 0.0,
    }
}

fn slices(rows: &[ReportRow], slice_of: impl Fn(&ReportRow) -> ChartSlice) -> Vec<ChartSlice> {
    let mut slices: Vec<ChartSlice> = Vec::new();
    for row in rows {
        let next = slice_of(row);
        let index = index_of(&mut slices, |s| s.key == next.key, || next.clone());
        slices[index].rounded_ms += row.rounded_ms;
    }
    slices.sort_by_cached_key(|s| {
        let neutral = s.key == NO_CLIENT_KEY || s.key == NO_TAG_KEY;
        (neutral, -s.rounded_ms, s.label.to_lowercase())
    });
    let total: i64 = slices.iter().map(|s| s.rounded_ms).sum();
    let mut offset = 0.0;
    for slice in &mut slices {
        slice.share = if total > 0 {
            slice.rounded_ms as f64 / total as f64
        } else {
            0.0
        };
        slice.percent = (slice.share * 100.0).round() as u8;
        slice.offset = offset;
        offset += slice.share;
    }
    slices
}

/// A tag's hue by its place in the tag list (by name), cycling through the accents but the neutral one; a
/// tag missing from the list (deleted while the report was read) is neutral.
fn tag_color(tags: &[Tag], id: &str) -> &'static str {
    let Some(index) = tags.iter().position(|t| t.id == id) else {
        return NEUTRAL;
    };
    ACCENTS
        .iter()
        .copied()
        .filter(|a| *a != NEUTRAL)
        .cycle()
        .nth(index)
        .unwrap_or(NEUTRAL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::recordings::Recording;
    use crate::reports::ReportFilter;
    use crate::reports::testing::*;

    fn charts_of(recordings: Vec<Recording>) -> ReportCharts {
        report(&ReportFilter::default(), D, D, recordings).charts
    }

    fn mixed() -> Vec<Recording> {
        vec![
            on("r1", "p-solo", D, 100),
            on("r2", "p-site", D, 30),
            on("r3", "p-portal", D, 50),
            on("r4", "p-api", D, 20),
        ]
    }

    fn keys_of(slices: &[ChartSlice]) -> Vec<&str> {
        slices.iter().map(|s| s.key.as_str()).collect()
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn client_slices_are_largest_first_with_no_client_last() {
        let charts = charts_of(mixed());

        assert_eq!(keys_of(&charts.by_client), ["c-acme", "c-beta", "none"]);
        let labels: Vec<&str> = charts.by_client.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Acme", "Beta", "No client"]);
        let rounded: Vec<i64> = charts.by_client.iter().map(|s| s.rounded_ms).collect();
        assert_eq!(rounded, [minutes(70), minutes(30), minutes(100)]);
    }

    #[test]
    fn project_slices_are_largest_first() {
        let charts = charts_of(mixed());

        assert_eq!(
            keys_of(&charts.by_project),
            ["p-solo", "p-portal", "p-site", "p-api"]
        );
        let labels: Vec<&str> = charts.by_project.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Aardvark", "Acme / Portal", "Beta / Site", "Acme / API"]
        );
    }

    #[test]
    fn tag_slices_put_untagged_last() {
        let charts = charts_of(vec![
            on("r1", "p-portal", D, 90),
            with_tag(on("r2", "p-portal", D, 10), "t-admin"),
            with_tag(on("r3", "p-portal", D, 20), "t-dev"),
        ]);

        assert_eq!(keys_of(&charts.by_tag), ["t-dev", "t-admin", NO_TAG_KEY]);
        assert_eq!(charts.by_tag[2].label, NO_TAG_LABEL);
    }

    #[test]
    fn percents_are_whole_numbers_and_offsets_accumulate_to_one() {
        let charts = charts_of(mixed());

        let slices = &charts.by_client;
        let percents: Vec<u8> = slices.iter().map(|s| s.percent).collect();
        assert_eq!(percents, [35, 15, 50]);
        let (offsets, shares) = ([0.0, 0.35, 0.5], [0.35, 0.15, 0.5]);
        for (i, slice) in slices.iter().enumerate() {
            assert_close(slice.offset, offsets[i]);
            assert_close(slice.share, shares[i]);
        }
        assert_close(slices.iter().map(|s| s.share).sum(), 1.0);
    }

    #[test]
    fn percents_are_rounded_to_the_nearest_whole_percent() {
        let charts = charts_of(vec![
            on("r1", "p-portal", D, 10),
            on("r2", "p-site", D, 10),
            on("r3", "p-solo", D, 10),
        ]);

        let percents: Vec<u8> = charts.by_project.iter().map(|s| s.percent).collect();
        assert_eq!(percents, [33, 33, 33]);
        assert_close(charts.by_project.iter().map(|s| s.share).sum(), 1.0);
    }

    #[test]
    fn an_empty_report_has_empty_charts() {
        assert_eq!(charts_of(vec![]), ReportCharts::default());
        assert!(charts_of(vec![]).by_client.is_empty());
    }

    #[test]
    fn client_and_project_slices_take_their_accent_and_projects_their_shade() {
        let charts = charts_of(mixed());

        let client = |key: &str| charts.by_client.iter().find(|s| s.key == key).unwrap();
        assert_eq!(
            (client("c-acme").color.as_str(), client("c-acme").shade),
            ("blue", None)
        );
        assert_eq!(
            (client("none").color.as_str(), client("none").shade),
            ("graphite", None)
        );
        let project = |key: &str| charts.by_project.iter().find(|s| s.key == key).unwrap();
        assert_eq!(
            (
                project("p-portal").color.as_str(),
                project("p-portal").shade
            ),
            ("blue", Some(4))
        );
        assert_eq!(
            (project("p-site").color.as_str(), project("p-site").shade),
            ("green", Some(7))
        );
    }

    #[test]
    fn tags_take_accents_by_their_place_in_the_tag_list() {
        let charts = charts_of(vec![
            with_tag(on("r1", "p-portal", D, 30), "t-admin"),
            with_tag(on("r2", "p-portal", D, 20), "t-dev"),
            with_tag(on("r3", "p-portal", D, 10), "t-sup"),
            on("r4", "p-portal", D, 5),
        ]);

        let colors: Vec<&str> = charts.by_tag.iter().map(|s| s.color.as_str()).collect();
        assert_eq!(colors, ["orange", "gold", "red", "graphite"]);
    }

    #[test]
    fn a_tag_missing_from_the_tag_list_is_graphite() {
        let result = report(
            &ReportFilter::default(),
            D,
            D,
            vec![with_tag(on("r1", "p-portal", D, 10), "t-admin")],
        );
        let rows = result.clients[0].projects[0].rows.clone();

        assert_eq!(charts(&rows, &[]).by_tag[0].color, "graphite");
    }

    #[test]
    fn tag_accents_cycle_without_graphite() {
        let tags: Vec<Tag> = (0..9)
            .map(|i| Tag {
                id: format!("t{i}"),
                name: format!("tag {i}"),
                emoji: None,
                created_at: 0,
            })
            .collect();
        let result = report(
            &ReportFilter::default(),
            D,
            D,
            vec![with_tag(on("r1", "p-portal", D, 10), "t-admin")],
        );
        let mut rows: Vec<ReportRow> = result.clients[0].projects[0].rows.clone();

        let colors: Vec<String> = [7, 8]
            .iter()
            .map(|&i| {
                rows[0].tag_id = Some(format!("t{i}"));
                charts(&rows, &tags).by_tag[0].color.clone()
            })
            .collect();

        assert_eq!(colors, ["teal", "orange"]);
    }
}
