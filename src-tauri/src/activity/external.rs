//! Activity that other apps report (ADR 0009). Each source is its own lane: its rows never overlap
//! each other, but may overlap the tracker's and other sources'.

use std::collections::BTreeSet;

use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use crate::db::Db;
use crate::db::activities::{self, ActivityRow, NewActivity};
use crate::db::time::{Day, Ms};
use crate::error::{AppError, AppResult};
use crate::tracker::TRACKER_SOURCE;

/// Stored sources are `external:<name>`, so no name can pass for the tracker.
pub const SOURCE_PREFIX: &str = "external:";
const MAX_SOURCE_CHARS: usize = 32;
const MAX_TITLE_CHARS: usize = 500;
const MAX_LINK_CHARS: usize = 2_000;
const MAX_SPAN: Ms = 24 * 3_600_000;
/// Allows for another app's clock running slightly ahead.
const MAX_START_AHEAD: Ms = 60_000;

/// One span from another app. `source` is the bare name (`planner`); `app` defaults to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ActivityInput {
    pub source: String,
    pub title: String,
    pub start: Ms,
    #[serde(default)]
    pub end: Option<Ms>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

/// What `start` wrote: the new open span, and the span of the same source it closed.
#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub row: ActivityRow,
    pub closed: Option<ActivityRow>,
}

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

/// Lowercase letters, digits, `-` and `_`, starting with a letter or digit; never `tracker`.
pub fn validate_source(source: &str) -> AppResult<()> {
    let mut chars = source.chars();
    let shaped = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && source.chars().count() <= MAX_SOURCE_CHARS;
    if !shaped {
        return Err(invalid(
            "source",
            format!(
                "must be 1 to {MAX_SOURCE_CHARS} lowercase letters, digits, - or _, starting with a letter or digit"
            ),
        ));
    }
    if source == TRACKER_SOURCE {
        return Err(invalid("source", "`tracker` is the built-in tracker's"));
    }
    Ok(())
}

pub fn validate(input: &ActivityInput, now: Ms) -> AppResult<()> {
    validate_source(&input.source)?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(invalid("title", "must not be empty"));
    }
    for (field, text, max) in [
        ("title", Some(title), MAX_TITLE_CHARS),
        ("app", input.app.as_deref(), MAX_TITLE_CHARS),
        ("url", input.url.as_deref(), MAX_LINK_CHARS),
        ("path", input.path.as_deref(), MAX_LINK_CHARS),
    ] {
        if text.is_some_and(|t| t.chars().count() > max) {
            return Err(invalid(field, format!("must be at most {max} characters")));
        }
    }
    if input.start > now + MAX_START_AHEAD {
        return Err(invalid("start", "must not be in the future"));
    }
    if let Some(end) = input.end {
        if end <= input.start {
            return Err(invalid("end", "must be after the start"));
        }
        if end - input.start > MAX_SPAN {
            return Err(invalid("end", "must be at most 24 hours after the start"));
        }
    }
    Ok(())
}

fn stored_source(name: &str) -> String {
    format!("{SOURCE_PREFIX}{name}")
}

/// Blank optional text counts as absent.
fn text(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
}

fn new_activity(input: &ActivityInput) -> NewActivity {
    NewActivity {
        source: stored_source(&input.source),
        start: input.start,
        app: text(&input.app).unwrap_or_else(|| input.source.clone()),
        bundle_id: None,
        title: input.title.trim().into(),
        url: text(&input.url),
        path: text(&input.path),
        private: false,
    }
}

/// Refuses when rows of the source `name` share time with `[start, end)`, naming their ids.
async fn refuse_overlap(
    conn: &mut SqliteConnection,
    name: &str,
    start: Ms,
    end: Ms,
) -> AppResult<()> {
    let ids: Vec<i64> = activities::overlapping_on(conn, &stored_source(name), start, end)
        .await?
        .iter()
        .map(|row| row.id)
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let list = ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Err(AppError::Overlap {
        message: format!("Overlaps activity {list} of {name}"),
        details: serde_json::json!({ "ids": ids }),
    })
}

/// Names the failing item of a batch in the error, as `items[2].title`.
fn in_item(error: AppError, index: usize) -> AppError {
    match error {
        AppError::InvalidField { field, message } => AppError::InvalidField {
            field: format!("items[{index}].{field}"),
            message,
        },
        AppError::Overlap { message, details } => AppError::Overlap {
            message: format!("items[{index}]: {message}"),
            details,
        },
        other => other,
    }
}

/// A valid item's `end`; `add` writes closed spans only.
fn closed_end(item: &ActivityInput, now: Ms) -> AppResult<Ms> {
    validate(item, now)?;
    item.end.ok_or_else(|| {
        invalid(
            "end",
            "is required; `activity start` opens a span without one",
        )
    })
}

/// Inserts closed spans, all or none.
pub async fn add_many(db: &Db, items: &[ActivityInput], now: Ms) -> AppResult<Vec<ActivityRow>> {
    if items.is_empty() {
        return Err(AppError::InvalidInput("No activity to add".into()));
    }
    let named = |error: AppError, index: usize| {
        if items.len() > 1 {
            in_item(error, index)
        } else {
            error
        }
    };
    let ends = items
        .iter()
        .enumerate()
        .map(|(index, item)| closed_end(item, now).map_err(|e| named(e, index)))
        .collect::<AppResult<Vec<Ms>>>()?;
    let mut tx = db.pool().begin().await?;
    let mut rows = Vec::with_capacity(items.len());
    for (index, (item, end)) in items.iter().zip(ends).enumerate() {
        // Rows inserted earlier in the transaction count, so a batch cannot overlap itself.
        refuse_overlap(&mut tx, &item.source, item.start, end)
            .await
            .map_err(|e| named(e, index))?;
        let id = activities::insert_on(&mut tx, &new_activity(item), Some(end)).await?;
        rows.push(activities::get_on(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(rows)
}

/// Opens a span of the source, closing its open one at `input.start`. At most one span per source is open.
pub async fn start(db: &Db, input: &ActivityInput, now: Ms) -> AppResult<Started> {
    validate(input, now)?;
    if input.end.is_some() {
        return Err(invalid(
            "end",
            "is not taken by `activity start`; `activity add` writes a closed span",
        ));
    }
    let source = stored_source(&input.source);
    let mut tx = db.pool().begin().await?;
    let open = activities::open_by_source_on(&mut tx, &source).await?;
    if let Some(open) = &open {
        if input.start < open.start {
            return Err(invalid(
                "start",
                format!("is before the start of the open activity {}", open.id),
            ));
        }
        activities::close_open_on(&mut tx, &source, input.start).await?;
    }
    refuse_overlap(&mut tx, &input.source, input.start, Ms::MAX).await?;
    let id = activities::insert_on(&mut tx, &new_activity(input), None).await?;
    let row = activities::get_on(&mut tx, id).await?;
    let closed = match open {
        Some(open) => Some(activities::get_on(&mut tx, open.id).await?),
        None => None,
    };
    tx.commit().await?;
    Ok(Started { row, closed })
}

/// Closes the source's open span at `end` (default `now`); `None` when none is open.
pub async fn stop(
    db: &Db,
    source: &str,
    end: Option<Ms>,
    now: Ms,
) -> AppResult<Option<ActivityRow>> {
    validate_source(source)?;
    let end = end.unwrap_or(now);
    if end > now + MAX_SPAN {
        return Err(invalid("end", "must be at most 24 hours from now"));
    }
    let stored = stored_source(source);
    let mut tx = db.pool().begin().await?;
    let Some(open) = activities::open_by_source_on(&mut tx, &stored).await? else {
        return Ok(None);
    };
    if end < open.start {
        return Err(invalid(
            "end",
            format!("is before the start of the open activity {}", open.id),
        ));
    }
    activities::close_open_on(&mut tx, &stored, end).await?;
    let row = activities::get_on(&mut tx, open.id).await?;
    tx.commit().await?;
    Ok(Some(row))
}

/// The names of the sources that reported activity, the latest first.
pub async fn sources(db: &Db) -> AppResult<Vec<String>> {
    Ok(activities::sources_with_prefix(db, SOURCE_PREFIX)
        .await?
        .into_iter()
        .filter_map(|s| s.strip_prefix(SOURCE_PREFIX).map(String::from))
        .collect())
}

/// The local days the rows share time with, in order; an open row runs until `now`.
pub fn days_touched<Tz: TimeZone>(rows: &[ActivityRow], now: Ms, tz: &Tz) -> Vec<Day> {
    let mut days = BTreeSet::new();
    for row in rows {
        let end = row.end.unwrap_or(now).max(row.start);
        // `end` is exclusive: a span ending at midnight does not touch the next day.
        let last = if end > row.start { end - 1 } else { end };
        days.extend(Day::range(
            &Day::of_in(row.start, tz),
            &Day::of_in(last, tz),
        ));
    }
    days.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::activity;
    use chrono::Utc;
    use chrono_tz::Europe::Vienna;

    const HOUR: Ms = 3_600_000;
    const NOW: Ms = 1_800_000_000_000;
    const BASE: Ms = NOW - 10 * HOUR;

    fn input(source: &str, title: &str, start: Ms, end: Option<Ms>) -> ActivityInput {
        ActivityInput {
            source: source.into(),
            title: title.into(),
            start,
            end,
            app: None,
            url: None,
            path: None,
        }
    }

    fn planner(start: Ms, end: Option<Ms>) -> ActivityInput {
        input("planner", "Plan", start, end)
    }

    fn valid_with(change: impl FnOnce(&mut ActivityInput)) -> ActivityInput {
        let mut item = planner(NOW - HOUR, Some(NOW));
        change(&mut item);
        item
    }

    fn field_of(error: AppError) -> String {
        match error {
            AppError::InvalidField { field, .. } => field,
            other => panic!("not an invalid field: {other:?}"),
        }
    }

    fn overlap_of(error: AppError) -> (String, serde_json::Value) {
        match error {
            AppError::Overlap { message, details } => (message, details),
            other => panic!("not an overlap: {other:?}"),
        }
    }

    async fn open_rows(db: &Db) -> Vec<ActivityRow> {
        activities::list_for_range(db, 0, Ms::MAX)
            .await
            .unwrap()
            .into_iter()
            .filter(|row| row.end.is_none())
            .collect()
    }

    #[test]
    fn source_names_must_be_lowercase_and_not_the_tracker() {
        let too_long = "a".repeat(33);
        let longest = "a".repeat(32);
        for source in [
            "",
            "Planner",
            "-planner",
            "_planner",
            too_long.as_str(),
            "my planner",
            "plan.ner",
            "tracker",
        ] {
            let error = validate_source(source).unwrap_err();
            assert_eq!(
                error.kind(),
                crate::error::ErrorKind::InvalidInput,
                "{source}"
            );
            assert_eq!(field_of(error), "source", "{source}");
        }
        for source in [
            "planner",
            "0abc",
            "my-planner_2",
            longest.as_str(),
            "tracker2",
        ] {
            assert_eq!(validate_source(source), Ok(()), "{source}");
        }
    }

    #[test]
    fn invalid_fields_are_named_in_the_error() {
        let cases: Vec<(&str, ActivityInput)> = vec![
            ("source", valid_with(|i| i.source = "Planner".into())),
            ("source", valid_with(|i| i.source = "tracker".into())),
            ("title", valid_with(|i| i.title = String::new())),
            ("title", valid_with(|i| i.title = " \t\n".into())),
            ("title", valid_with(|i| i.title = "a".repeat(501))),
            ("app", valid_with(|i| i.app = Some("a".repeat(501)))),
            ("url", valid_with(|i| i.url = Some("a".repeat(2_001)))),
            ("path", valid_with(|i| i.path = Some("a".repeat(2_001)))),
            ("start", valid_with(|i| i.start = NOW + 60_001)),
            ("end", valid_with(|i| i.end = Some(i.start))),
            ("end", valid_with(|i| i.end = Some(i.start - 1))),
            ("end", valid_with(|i| i.end = Some(i.start + 24 * HOUR + 1))),
        ];
        for (index, (field, item)) in cases.into_iter().enumerate() {
            let error = validate(&item, NOW).unwrap_err();
            assert_eq!(
                error.kind(),
                crate::error::ErrorKind::InvalidInput,
                "case {index}"
            );
            assert_eq!(field_of(error), field, "case {index}");
        }
    }

    #[test]
    fn inputs_at_the_limits_are_valid() {
        let cases: Vec<ActivityInput> = vec![
            valid_with(|i| i.source = "a".repeat(32)),
            valid_with(|i| i.title = "a".repeat(500)),
            valid_with(|i| i.app = Some("a".repeat(500))),
            valid_with(|i| i.url = Some("a".repeat(2_000))),
            valid_with(|i| i.path = Some("a".repeat(2_000))),
            valid_with(|i| {
                i.start = NOW + 60_000;
                i.end = None;
            }),
            valid_with(|i| i.end = Some(i.start + 24 * HOUR)),
            valid_with(|i| {
                i.start = NOW;
                i.end = Some(NOW + 23 * HOUR);
            }),
            valid_with(|i| i.end = None),
        ];
        for (index, item) in cases.iter().enumerate() {
            assert_eq!(validate(item, NOW), Ok(()), "case {index}");
        }
    }

    #[test]
    fn activity_input_uses_camel_case_and_optional_fields() {
        let parsed: ActivityInput =
            serde_json::from_str(r#"{"source":"planner","title":"Plan","start":5}"#).unwrap();
        assert_eq!(parsed, input("planner", "Plan", 5, None));
        assert_eq!(
            serde_json::to_value(input("planner", "Plan", 5, Some(9))).unwrap(),
            serde_json::json!({
                "source": "planner", "title": "Plan", "start": 5, "end": 9,
                "app": null, "url": null, "path": null
            })
        );
    }

    #[tokio::test]
    async fn add_many_stores_closed_spans_of_the_prefixed_source_with_cleaned_text() {
        let db = Db::open_memory().await.unwrap();
        let first = ActivityInput {
            url: Some("  ".into()),
            path: Some(String::new()),
            ..input("planner", "  Plan sprint  ", BASE, Some(BASE + HOUR))
        };
        let second = ActivityInput {
            app: Some(" Editor ".into()),
            url: Some(" https://example.com ".into()),
            path: Some("/a/b".into()),
            ..input("planner", "Write", BASE + HOUR, Some(BASE + 2 * HOUR))
        };

        let rows = add_many(&db, &[first, second], NOW).await.unwrap();

        assert_eq!(
            rows,
            [
                ActivityRow {
                    id: rows[0].id,
                    source: "external:planner".into(),
                    start: BASE,
                    end: Some(BASE + HOUR),
                    app: "planner".into(),
                    bundle_id: None,
                    title: "Plan sprint".into(),
                    url: None,
                    path: None,
                    private: false,
                },
                ActivityRow {
                    id: rows[1].id,
                    source: "external:planner".into(),
                    start: BASE + HOUR,
                    end: Some(BASE + 2 * HOUR),
                    app: "Editor".into(),
                    bundle_id: None,
                    title: "Write".into(),
                    url: Some("https://example.com".into()),
                    path: Some("/a/b".into()),
                    private: false,
                },
            ]
        );
        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            rows
        );
    }

    #[tokio::test]
    async fn add_many_refuses_an_empty_list() {
        let db = Db::open_memory().await.unwrap();
        let error = add_many(&db, &[], NOW).await.unwrap_err();
        assert!(matches!(error, AppError::InvalidInput(_)), "{error:?}");
    }

    #[tokio::test]
    async fn add_many_refuses_an_item_without_an_end() {
        let db = Db::open_memory().await.unwrap();

        let single = add_many(&db, &[planner(BASE, None)], NOW)
            .await
            .unwrap_err();
        assert_eq!(field_of(single), "end");

        let batch = [planner(BASE, Some(BASE + HOUR)), planner(BASE + HOUR, None)];
        let in_batch = add_many(&db, &batch, NOW).await.unwrap_err();
        assert_eq!(field_of(in_batch), "items[1].end");
        assert_eq!(activities::count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_many_names_the_failing_item_only_in_batches() {
        let db = Db::open_memory().await.unwrap();
        let bad = input("planner", " ", BASE, Some(BASE + HOUR));

        let single = add_many(&db, std::slice::from_ref(&bad), NOW)
            .await
            .unwrap_err();
        let batch = add_many(&db, &[planner(BASE, Some(BASE + HOUR)), bad], NOW)
            .await
            .unwrap_err();

        assert_eq!(field_of(single), "title");
        assert_eq!(field_of(batch), "items[1].title");
        assert_eq!(activities::count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_many_refuses_an_overlap_with_the_same_source_naming_the_row() {
        let db = Db::open_memory().await.unwrap();
        let existing = add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap();
        let id = existing[0].id;

        let error = add_many(&db, &[planner(BASE + HOUR / 2, Some(BASE + 2 * HOUR))], NOW)
            .await
            .unwrap_err();

        let (message, details) = overlap_of(error);
        assert_eq!(message, format!("Overlaps activity {id} of planner"));
        assert_eq!(details, serde_json::json!({ "ids": [id] }));
        assert_eq!(activities::count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn add_many_lists_every_conflicting_row() {
        let db = Db::open_memory().await.unwrap();
        let existing = add_many(
            &db,
            &[
                planner(BASE, Some(BASE + HOUR)),
                planner(BASE + 2 * HOUR, Some(BASE + 3 * HOUR)),
            ],
            NOW,
        )
        .await
        .unwrap();
        let (a, b) = (existing[0].id, existing[1].id);

        let error = add_many(&db, &[planner(BASE, Some(BASE + 4 * HOUR))], NOW)
            .await
            .unwrap_err();

        let (message, details) = overlap_of(error);
        assert_eq!(message, format!("Overlaps activity {a}, {b} of planner"));
        assert_eq!(details, serde_json::json!({ "ids": [a, b] }));
    }

    #[tokio::test]
    async fn add_many_allows_touching_edges_and_other_lanes() {
        let db = Db::open_memory().await.unwrap();
        add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap();
        activity(&db, BASE, Some(BASE + HOUR), "Code", "").await;

        let touching = planner(BASE + HOUR, Some(BASE + 2 * HOUR));
        let before = planner(BASE - HOUR, Some(BASE));
        let other = input("calendar", "Meeting", BASE, Some(BASE + HOUR));

        let rows = add_many(&db, &[touching, before, other], NOW)
            .await
            .unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(activities::count(&db).await.unwrap(), 5);
    }

    #[tokio::test]
    async fn add_many_writes_nothing_when_a_later_item_overlaps_an_existing_row() {
        let db = Db::open_memory().await.unwrap();
        let existing = add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap();
        let id = existing[0].id;
        let batch = [
            planner(BASE + 2 * HOUR, Some(BASE + 3 * HOUR)),
            planner(BASE + HOUR / 2, Some(BASE + HOUR)),
        ];

        let error = add_many(&db, &batch, NOW).await.unwrap_err();

        let (message, details) = overlap_of(error);
        assert_eq!(
            message,
            format!("items[1]: Overlaps activity {id} of planner")
        );
        assert_eq!(details, serde_json::json!({ "ids": [id] }));
        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            existing
        );
    }

    #[tokio::test]
    async fn add_many_refuses_a_batch_whose_items_overlap_each_other() {
        let db = Db::open_memory().await.unwrap();
        let batch = [
            planner(BASE, Some(BASE + HOUR)),
            planner(BASE + HOUR / 2, Some(BASE + 2 * HOUR)),
        ];

        let error = add_many(&db, &batch, NOW).await.unwrap_err();

        let (message, details) = overlap_of(error);
        assert!(
            message.starts_with("items[1]: Overlaps activity "),
            "{message}"
        );
        assert!(message.ends_with(" of planner"), "{message}");
        assert_eq!(details["ids"].as_array().map(Vec::len), Some(1));
        assert_eq!(activities::count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_many_does_not_name_the_item_of_a_single_overlap() {
        let db = Db::open_memory().await.unwrap();
        add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap();

        let error = add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap_err();

        let (message, _) = overlap_of(error);
        assert!(message.starts_with("Overlaps activity "), "{message}");
    }

    #[tokio::test]
    async fn start_opens_a_span() {
        let db = Db::open_memory().await.unwrap();

        let started = start(&db, &planner(BASE, None), NOW).await.unwrap();

        assert_eq!(started.closed, None);
        assert_eq!(
            started.row,
            ActivityRow {
                id: started.row.id,
                source: "external:planner".into(),
                start: BASE,
                end: None,
                app: "planner".into(),
                bundle_id: None,
                title: "Plan".into(),
                url: None,
                path: None,
                private: false,
            }
        );
        assert_eq!(open_rows(&db).await, [started.row]);
    }

    #[tokio::test]
    async fn start_closes_the_open_span_of_the_source_at_the_new_start() {
        let db = Db::open_memory().await.unwrap();
        let first = start(&db, &planner(BASE, None), NOW).await.unwrap().row;

        let second = start(&db, &input("planner", "Next", BASE + HOUR, None), NOW)
            .await
            .unwrap();

        assert_eq!(
            second.closed,
            Some(ActivityRow {
                end: Some(BASE + HOUR),
                ..first
            })
        );
        assert_eq!(second.row.title, "Next");
        assert_eq!(second.row.end, None);
        assert_eq!(open_rows(&db).await, [second.row]);
    }

    #[tokio::test]
    async fn repeated_starts_leave_one_open_span_per_source() {
        let db = Db::open_memory().await.unwrap();
        for step in 0..4 {
            start(&db, &planner(BASE + step * HOUR, None), NOW)
                .await
                .unwrap();
        }

        let open = open_rows(&db).await;

        assert_eq!(open.len(), 1);
        assert_eq!(open[0].start, BASE + 3 * HOUR);
        assert_eq!(activities::count(&db).await.unwrap(), 4);
    }

    #[tokio::test]
    async fn start_leaves_other_sources_open_spans_alone() {
        let db = Db::open_memory().await.unwrap();
        let calendar = start(&db, &input("calendar", "Meeting", BASE, None), NOW)
            .await
            .unwrap()
            .row;
        let tracker = activity(&db, BASE, None, "Code", "").await;

        let started = start(&db, &planner(BASE + HOUR, None), NOW).await.unwrap();

        assert_eq!(started.closed, None);
        let mut open = open_rows(&db).await;
        open.sort_by_key(|row| row.id);
        assert_eq!(open, [calendar, tracker, started.row]);
    }

    #[tokio::test]
    async fn start_before_the_open_spans_start_is_refused_and_writes_nothing() {
        let db = Db::open_memory().await.unwrap();
        let open = start(&db, &planner(BASE, None), NOW).await.unwrap().row;

        let error = start(&db, &planner(BASE - HOUR, None), NOW)
            .await
            .unwrap_err();

        assert_eq!(field_of(error), "start");
        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            [open]
        );
    }

    #[tokio::test]
    async fn start_refuses_an_end() {
        let db = Db::open_memory().await.unwrap();

        let error = start(&db, &planner(BASE, Some(BASE + HOUR)), NOW)
            .await
            .unwrap_err();

        assert_eq!(field_of(error), "end");
        assert_eq!(activities::count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn start_refuses_invalid_input() {
        let db = Db::open_memory().await.unwrap();

        let error = start(&db, &input("Planner", "Plan", BASE, None), NOW)
            .await
            .unwrap_err();

        assert_eq!(field_of(error), "source");
    }

    #[tokio::test]
    async fn start_inside_a_later_closed_span_of_the_source_is_refused() {
        let db = Db::open_memory().await.unwrap();
        let later = add_many(&db, &[planner(BASE + HOUR, Some(BASE + 2 * HOUR))], NOW)
            .await
            .unwrap();
        let id = later[0].id;

        let error = start(&db, &planner(BASE + HOUR / 2, None), NOW)
            .await
            .unwrap_err();

        let (message, details) = overlap_of(error);
        assert_eq!(message, format!("Overlaps activity {id} of planner"));
        assert_eq!(details, serde_json::json!({ "ids": [id] }));
        assert_eq!(
            activities::list_for_range(&db, 0, Ms::MAX).await.unwrap(),
            later
        );
    }

    #[tokio::test]
    async fn start_overlapping_a_later_span_keeps_the_open_span_open() {
        let db = Db::open_memory().await.unwrap();
        let open = start(&db, &planner(BASE, None), NOW).await.unwrap().row;
        let later = activity_row_of_source(&db, "external:planner", BASE + 3 * HOUR).await;

        let error = start(&db, &planner(BASE + HOUR, None), NOW)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Overlap { .. }), "{error:?}");
        let rows = activities::list_for_range(&db, 0, Ms::MAX).await.unwrap();
        assert_eq!(rows, [open, later]);
    }

    async fn activity_row_of_source(db: &Db, source: &str, start: Ms) -> ActivityRow {
        let id = activities::insert_on(
            &mut *db.pool().acquire().await.unwrap(),
            &NewActivity {
                source: source.into(),
                start,
                app: "planner".into(),
                bundle_id: None,
                title: "Later".into(),
                url: None,
                path: None,
                private: false,
            },
            Some(start + HOUR),
        )
        .await
        .unwrap();
        activities::get_on(&mut *db.pool().acquire().await.unwrap(), id)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn stop_closes_the_open_span_at_the_given_end() {
        let db = Db::open_memory().await.unwrap();
        let open = start(&db, &planner(BASE, None), NOW).await.unwrap().row;

        let stopped = stop(&db, "planner", Some(BASE + HOUR), NOW).await.unwrap();

        assert_eq!(
            stopped,
            Some(ActivityRow {
                end: Some(BASE + HOUR),
                ..open
            })
        );
        assert!(open_rows(&db).await.is_empty());
    }

    #[tokio::test]
    async fn stop_without_an_end_closes_at_now() {
        let db = Db::open_memory().await.unwrap();
        start(&db, &planner(BASE, None), NOW).await.unwrap();

        let stopped = stop(&db, "planner", None, NOW).await.unwrap().unwrap();

        assert_eq!(stopped.end, Some(NOW));
    }

    #[tokio::test]
    async fn stop_with_nothing_open_returns_none() {
        let db = Db::open_memory().await.unwrap();
        add_many(&db, &[planner(BASE, Some(BASE + HOUR))], NOW)
            .await
            .unwrap();
        activity(&db, BASE, None, "Code", "").await;

        assert_eq!(stop(&db, "planner", None, NOW).await.unwrap(), None);
        assert_eq!(open_rows(&db).await.len(), 1);
    }

    #[tokio::test]
    async fn stop_only_closes_the_named_source() {
        let db = Db::open_memory().await.unwrap();
        start(&db, &planner(BASE, None), NOW).await.unwrap();
        let calendar = start(&db, &input("calendar", "Meeting", BASE, None), NOW)
            .await
            .unwrap()
            .row;

        stop(&db, "planner", None, NOW).await.unwrap();

        assert_eq!(open_rows(&db).await, [calendar]);
    }

    #[tokio::test]
    async fn stop_before_the_open_spans_start_is_refused_and_keeps_it_open() {
        let db = Db::open_memory().await.unwrap();
        let open = start(&db, &planner(BASE, None), NOW).await.unwrap().row;

        let error = stop(&db, "planner", Some(BASE - 1), NOW).await.unwrap_err();

        assert_eq!(field_of(error), "end");
        assert_eq!(open_rows(&db).await, [open]);
    }

    #[tokio::test]
    async fn stop_more_than_24_hours_ahead_is_refused() {
        let db = Db::open_memory().await.unwrap();
        let open = start(&db, &planner(BASE, None), NOW).await.unwrap().row;

        let error = stop(&db, "planner", Some(NOW + 24 * HOUR + 1), NOW)
            .await
            .unwrap_err();
        assert_eq!(field_of(error), "end");
        assert_eq!(open_rows(&db).await, [open]);

        let stopped = stop(&db, "planner", Some(NOW + 24 * HOUR), NOW)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.end, Some(NOW + 24 * HOUR));
    }

    #[tokio::test]
    async fn stop_refuses_an_invalid_source_name() {
        let db = Db::open_memory().await.unwrap();
        for source in ["Planner", "", "tracker"] {
            let error = stop(&db, source, None, NOW).await.unwrap_err();
            assert_eq!(field_of(error), "source", "{source}");
        }
    }

    #[tokio::test]
    async fn sources_are_bare_names_latest_activity_first_without_the_tracker() {
        let db = Db::open_memory().await.unwrap();
        add_many(
            &db,
            &[
                planner(BASE, Some(BASE + HOUR)),
                input("zed", "Edit", BASE + 2 * HOUR, Some(BASE + 3 * HOUR)),
                input("calendar", "Meeting", BASE + HOUR, Some(BASE + 2 * HOUR)),
            ],
            NOW,
        )
        .await
        .unwrap();
        activity(&db, BASE + 5 * HOUR, Some(BASE + 6 * HOUR), "Code", "").await;

        assert_eq!(sources(&db).await.unwrap(), ["zed", "calendar", "planner"]);
    }

    #[tokio::test]
    async fn sources_of_an_empty_database_are_empty() {
        let db = Db::open_memory().await.unwrap();
        assert!(sources(&db).await.unwrap().is_empty());
    }

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> Ms {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0)
            .unwrap()
            .timestamp_millis()
    }

    fn span(start: Ms, end: Option<Ms>) -> ActivityRow {
        ActivityRow {
            id: 1,
            source: "external:planner".into(),
            start,
            end,
            app: "planner".into(),
            bundle_id: None,
            title: "Plan".into(),
            url: None,
            path: None,
            private: false,
        }
    }

    fn days(names: &[&str]) -> Vec<Day> {
        names.iter().map(|d| Day::parse(d).unwrap()).collect()
    }

    #[test]
    fn a_span_within_one_local_day_touches_that_day() {
        let rows = [span(utc(2026, 1, 15, 10, 0), Some(utc(2026, 1, 15, 11, 0)))];
        let now = utc(2026, 1, 20, 0, 0);
        assert_eq!(days_touched(&rows, now, &Vienna), days(&["2026-01-15"]));
    }

    #[test]
    fn a_span_crossing_local_midnight_touches_both_days() {
        // 23:30 to 00:30 in Vienna (UTC+1).
        let rows = [span(
            utc(2026, 1, 15, 22, 30),
            Some(utc(2026, 1, 15, 23, 30)),
        )];
        let now = utc(2026, 1, 20, 0, 0);
        assert_eq!(
            days_touched(&rows, now, &Vienna),
            days(&["2026-01-15", "2026-01-16"])
        );
    }

    #[test]
    fn a_span_ending_exactly_at_local_midnight_does_not_touch_the_next_day() {
        let rows = [span(
            utc(2026, 1, 15, 22, 30),
            Some(utc(2026, 1, 15, 23, 0)),
        )];
        let now = utc(2026, 1, 20, 0, 0);
        assert_eq!(days_touched(&rows, now, &Vienna), days(&["2026-01-15"]));
    }

    #[test]
    fn an_open_row_runs_until_now() {
        let rows = [span(utc(2026, 1, 15, 10, 0), None)];
        let now = utc(2026, 1, 17, 12, 0);
        assert_eq!(
            days_touched(&rows, now, &Vienna),
            days(&["2026-01-15", "2026-01-16", "2026-01-17"])
        );
    }

    #[test]
    fn an_empty_span_touches_its_own_day() {
        let at = utc(2026, 1, 15, 10, 0);
        assert_eq!(
            days_touched(&[span(at, Some(at))], at, &Vienna),
            days(&["2026-01-15"])
        );
    }

    #[test]
    fn touched_days_are_merged_and_sorted() {
        let rows = [
            span(utc(2026, 1, 17, 10, 0), Some(utc(2026, 1, 17, 11, 0))),
            span(utc(2026, 1, 15, 10, 0), Some(utc(2026, 1, 15, 11, 0))),
            span(utc(2026, 1, 15, 12, 0), Some(utc(2026, 1, 15, 13, 0))),
        ];
        let now = utc(2026, 1, 20, 0, 0);
        assert_eq!(
            days_touched(&rows, now, &Vienna),
            days(&["2026-01-15", "2026-01-17"])
        );
        assert!(days_touched(&[], now, &Vienna).is_empty());
    }
}
