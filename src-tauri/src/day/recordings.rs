//! Recordings on the day: one block per span for the time column, one card per recording for the timesheet.

use super::{Names, RecordingBlock, TimesheetCard};
use crate::db::recordings::Recording;
use crate::db::time::{Ms, clip};

/// The spans of `recordings` cut to `bounds`, one block each, by start; an open span ends at `now`.
pub fn recording_blocks(
    recordings: &[Recording],
    names: &Names,
    bounds: (Ms, Ms),
    now: Ms,
) -> Vec<RecordingBlock> {
    let mut blocks: Vec<RecordingBlock> = recordings
        .iter()
        .flat_map(|r| r.spans.iter().map(move |s| (r, s)))
        .filter_map(|(r, s)| {
            let (start, end) = clip(s.start, s.end, bounds, now)?;
            let project = names.projects.get(&r.project_id);
            Some(RecordingBlock {
                id: r.id.clone(),
                span_id: s.id.clone(),
                span_count: u32::try_from(r.spans.len()).unwrap_or(u32::MAX),
                start,
                end,
                project_id: r.project_id.clone(),
                project_name: project.map_or_else(String::new, |p| p.name.clone()),
                client_name: project.and_then(|p| p.client_name.clone()),
                color: project.map_or_else(|| "graphite".into(), |p| p.color.clone()),
                shade: project.map_or(4, |p| p.shade),
                note: r.note.clone(),
                tag: r.tag_id.as_ref().and_then(|id| names.tags.get(id).cloned()),
                running: s.end.is_none(),
                clipped_start: s.start < bounds.0,
                clipped_end: s.end.unwrap_or(now) > bounds.1,
            })
        })
        .collect();
    blocks.sort_by_key(|b| b.start);
    blocks
}

/// One card per recording, in the order of `blocks` (by start), adding up the day's parts of its spans.
pub fn timesheet(blocks: &[RecordingBlock]) -> Vec<TimesheetCard> {
    let mut cards: Vec<TimesheetCard> = Vec::new();
    for block in blocks {
        if let Some(card) = cards.iter_mut().find(|c| c.id == block.id) {
            card.duration_ms += block.end - block.start;
            card.end = card.end.max(block.end);
            card.span_ids.push(block.span_id.clone());
            card.running |= block.running;
            continue;
        }
        cards.push(TimesheetCard {
            id: block.id.clone(),
            project_id: block.project_id.clone(),
            project_name: block.project_name.clone(),
            client_name: block.client_name.clone(),
            color: block.color.clone(),
            shade: block.shade,
            note: block.note.clone(),
            tag: block.tag.clone(),
            duration_ms: block.end - block.start,
            start: block.start,
            end: block.end,
            span_ids: vec![block.span_id.clone()],
            running: block.running,
        });
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::testing::*;
    use crate::day::{DayInput, DayView, ProjectRef, totals, view};
    use crate::db::recordings::RecordingSpan;
    use serde_json::json;

    #[test]
    fn recorded_time_sums_the_parts_of_recordings_inside_the_day() {
        let input = DayInput {
            bounds: (HOUR, 2 * HOUR),
            now: mins(115),
            ..input_with_recordings(vec![
                recording("a", mins(30), Some(mins(90))),
                recording("b", mins(100), Some(mins(200))),
                recording("c", mins(110), None),
                recording("d", 0, Some(HOUR)),
                recording("e", 3 * HOUR, Some(4 * HOUR)),
            ])
        };

        let view = view(&input, &settings());

        assert_eq!(view.totals.recorded_ms, mins(30) + mins(20) + mins(5));
        assert_eq!(view.recordings.len(), 3);
    }

    #[test]
    fn a_recording_across_midnight_shows_clipped_on_both_days() {
        let midnight = DAY_LENGTH;
        let rows = vec![recording("r", midnight - HOUR, Some(midnight + 2 * HOUR))];
        let first = DayInput {
            bounds: (0, midnight),
            ..input_with_recordings(rows.clone())
        };
        let second = DayInput {
            bounds: (midnight, 2 * midnight),
            ..input_with_recordings(rows)
        };

        let first = view(&first, &settings());
        let second = view(&second, &settings());

        let shape = |view: &DayView| {
            view.recordings
                .iter()
                .map(|b| (b.start, b.end, b.clipped_start, b.clipped_end))
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(&first), [(midnight - HOUR, midnight, false, true)]);
        assert_eq!(
            shape(&second),
            [(midnight, midnight + 2 * HOUR, true, false)]
        );
        assert_eq!(first.totals.recorded_ms, HOUR);
        assert_eq!(second.totals.recorded_ms, 2 * HOUR);
    }

    #[test]
    fn a_recording_with_spans_on_two_days_has_one_block_per_span_on_each_day() {
        let midnight = DAY_LENGTH;
        let rows = vec![recording_of_spans(
            "r",
            &[
                (midnight - 3 * HOUR, midnight - 2 * HOUR),
                (midnight - HOUR, midnight + HOUR),
                (midnight + 5 * HOUR, midnight + 6 * HOUR),
            ],
        )];
        let first = DayInput {
            bounds: (0, midnight),
            ..input_with_recordings(rows.clone())
        };
        let second = DayInput {
            bounds: (midnight, 2 * midnight),
            ..input_with_recordings(rows)
        };

        let first = view(&first, &settings());
        let second = view(&second, &settings());

        let shape = |view: &DayView| {
            assert!(view.recordings.iter().all(|b| b.id == "r"));
            view.recordings
                .iter()
                .map(|b| {
                    let clipped = (b.clipped_start, b.clipped_end);
                    (b.span_id.clone(), b.span_count, b.start, b.end, clipped)
                })
                .collect::<Vec<_>>()
        };
        let at = |hours: i64| midnight + hours * HOUR;
        let id = String::from;
        assert_eq!(
            shape(&first),
            [
                (id("r-span0"), 3, at(-3), at(-2), (false, false)),
                (id("r-span1"), 3, at(-1), at(0), (false, true)),
            ]
        );
        assert_eq!(
            shape(&second),
            [
                (id("r-span1"), 3, at(0), at(1), (true, false)),
                (id("r-span2"), 3, at(5), at(6), (false, false)),
            ]
        );
    }

    #[test]
    fn recorded_time_of_a_day_counts_only_the_parts_of_spans_inside_it() {
        let midnight = DAY_LENGTH;
        let rows = vec![recording_of_spans(
            "r",
            &[
                (midnight - 3 * HOUR, midnight - 2 * HOUR),
                (midnight - HOUR, midnight + HOUR),
                (midnight + 5 * HOUR, midnight + 6 * HOUR),
            ],
        )];

        let totals_on = |bounds| {
            totals(&DayInput {
                bounds,
                ..input_with_recordings(rows.clone())
            })
            .recorded_ms
        };

        assert_eq!(totals_on((0, midnight)), 2 * HOUR);
        assert_eq!(totals_on((midnight, 2 * midnight)), 2 * HOUR);
        assert_eq!(totals_on((midnight - 2 * HOUR, midnight - HOUR)), 0);
    }

    #[test]
    fn a_running_span_after_closed_ones_is_the_only_running_block() {
        let mut r = recording_of_spans("r", &[(HOUR, 2 * HOUR)]);
        r.spans.push(RecordingSpan {
            id: "r-open".into(),
            start: 3 * HOUR,
            end: None,
        });
        let input = DayInput {
            now: 4 * HOUR,
            ..input_with_recordings(vec![r])
        };

        let view = view(&input, &settings());

        let blocks: Vec<_> = view
            .recordings
            .iter()
            .map(|b| (b.span_id.as_str(), b.end, b.running))
            .collect();
        assert_eq!(
            blocks,
            [("r-span0", 2 * HOUR, false), ("r-open", 4 * HOUR, true)]
        );
        assert_eq!(view.totals.recorded_ms, 2 * HOUR);
    }

    #[test]
    fn blocks_of_different_recordings_are_ordered_by_start_across_their_spans() {
        let input = input_with_recordings(vec![
            recording_of_spans("a", &[(HOUR, 2 * HOUR), (5 * HOUR, 6 * HOUR)]),
            recording_of_spans("b", &[(3 * HOUR, 4 * HOUR)]),
        ]);

        let view = view(&input, &settings());

        let ids: Vec<_> = view.recordings.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "a"]);
    }

    #[test]
    fn a_recording_block_carries_the_project_and_the_tag_name() {
        let mut input = input_with_recordings(vec![
            Recording {
                note: "fix login".into(),
                tag_id: Some("t-alpha".into()),
                ..recording("late", mins(120), Some(mins(150)))
            },
            recording("early", mins(60), Some(mins(90))),
        ]);
        input.names.projects.insert(
            "p1".into(),
            ProjectRef {
                client_name: Some("Acme".into()),
                color: "teal".into(),
                shade: 7,
                ..project_ref("Portal")
            },
        );
        input.names.tags.insert("t-alpha".into(), "Alpha".into());

        let view = view(&input, &settings());

        assert_eq!(
            view.recordings,
            [
                RecordingBlock {
                    id: "early".into(),
                    span_id: "early-span".into(),
                    span_count: 1,
                    start: mins(60),
                    end: mins(90),
                    project_id: "p1".into(),
                    project_name: "Portal".into(),
                    client_name: Some("Acme".into()),
                    color: "teal".into(),
                    shade: 7,
                    note: String::new(),
                    tag: None,
                    running: false,
                    clipped_start: false,
                    clipped_end: false,
                },
                RecordingBlock {
                    id: "late".into(),
                    span_id: "late-span".into(),
                    span_count: 1,
                    start: mins(120),
                    end: mins(150),
                    project_id: "p1".into(),
                    project_name: "Portal".into(),
                    client_name: Some("Acme".into()),
                    color: "teal".into(),
                    shade: 7,
                    note: "fix login".into(),
                    tag: Some("Alpha".into()),
                    running: false,
                    clipped_start: false,
                    clipped_end: false,
                },
            ]
        );
    }

    #[test]
    fn a_recording_block_has_no_tag_when_its_tag_is_gone() {
        let input = input_with_recordings(vec![Recording {
            tag_id: Some("t-gone".into()),
            ..recording("r1", mins(60), Some(mins(90)))
        }]);

        let view = view(&input, &settings());

        assert_eq!(view.recordings[0].tag, None);
    }

    #[test]
    fn a_running_recording_runs_to_now_and_is_clipped_at_the_end_of_the_day_when_now_is_later() {
        for (now, expected_end, clipped_end) in [
            (mins(90), mins(90), false),
            (3 * DAY_LENGTH, DAY_LENGTH, true),
        ] {
            let input = DayInput {
                now,
                ..input_with_recordings(vec![recording("r", HOUR, None)])
            };

            let view = view(&input, &settings());

            let block = &view.recordings[0];
            assert_eq!(
                (block.start, block.end, block.running, block.clipped_end),
                (HOUR, expected_end, true, clipped_end),
                "now {now}"
            );
        }
    }

    #[test]
    fn a_recording_of_an_unknown_project_falls_back_to_graphite() {
        let input = input_with_recordings(vec![recording("r", HOUR, Some(2 * HOUR))]);

        let view = view(&input, &settings());

        let block = &view.recordings[0];
        assert_eq!(
            (block.project_name.as_str(), block.client_name.clone()),
            ("", None)
        );
        assert_eq!((block.color.as_str(), block.shade), ("graphite", 4));
    }

    fn named_input(recordings: Vec<Recording>) -> DayInput {
        let mut input = input_with_recordings(recordings);
        input
            .names
            .projects
            .insert("p1".into(), project_ref("Portal"));
        input
    }

    #[test]
    fn the_timesheet_has_one_card_per_recording_in_the_order_of_their_first_span() {
        let input = DayInput {
            now: 4 * HOUR,
            ..named_input(vec![
                recording("b", 3 * HOUR, None),
                recording_of_spans("a", &[(HOUR, 2 * HOUR), (5 * HOUR, 6 * HOUR)]),
            ])
        };

        let view = view(&input, &settings());

        let card = |id: &str, start, end, duration_ms, span_ids: &[&str], running| TimesheetCard {
            id: id.into(),
            project_id: "p1".into(),
            project_name: "Portal".into(),
            client_name: None,
            color: "blue".into(),
            shade: 4,
            note: String::new(),
            tag: None,
            duration_ms,
            start,
            end,
            span_ids: span_ids.iter().map(|s| s.to_string()).collect(),
            running,
        };
        assert_eq!(
            view.timesheet,
            [
                card(
                    "a",
                    HOUR,
                    6 * HOUR,
                    2 * HOUR,
                    &["a-span0", "a-span1"],
                    false
                ),
                card("b", 3 * HOUR, 4 * HOUR, HOUR, &["b-span"], true),
            ]
        );
    }

    #[test]
    fn a_timesheet_card_adds_up_only_the_part_of_its_spans_inside_the_day() {
        let midnight = DAY_LENGTH;
        let rows = vec![recording_of_spans(
            "r",
            &[
                (midnight - 3 * HOUR, midnight - 2 * HOUR),
                (midnight - HOUR, midnight + 2 * HOUR),
            ],
        )];

        let card_on = |bounds| {
            let input = DayInput {
                bounds,
                ..named_input(rows.clone())
            };
            let card = view(&input, &settings()).timesheet.remove(0);
            (card.duration_ms, card.start, card.end, card.span_ids)
        };

        assert_eq!(
            card_on((0, midnight)),
            (
                2 * HOUR,
                midnight - 3 * HOUR,
                midnight,
                vec!["r-span0".to_string(), "r-span1".to_string()]
            )
        );
        assert_eq!(
            card_on((midnight, 2 * midnight)),
            (
                2 * HOUR,
                midnight,
                midnight + 2 * HOUR,
                vec!["r-span1".to_string()]
            )
        );
    }

    #[test]
    fn a_day_without_recordings_has_an_empty_timesheet() {
        assert!(
            view(&input(vec![], vec![]), &settings())
                .timesheet
                .is_empty()
        );
    }

    #[test]
    fn timesheet_cards_serialize_with_camel_case_keys() {
        let input = named_input(vec![Recording {
            note: "n".into(),
            ..recording("r1", HOUR, Some(2 * HOUR))
        }]);

        let view = view(&input, &settings());

        assert_eq!(
            serde_json::to_value(&view.timesheet).unwrap(),
            json!([{
                "id": "r1",
                "projectId": "p1",
                "projectName": "Portal",
                "clientName": null,
                "color": "blue",
                "shade": 4,
                "note": "n",
                "tag": null,
                "durationMs": HOUR,
                "start": HOUR,
                "end": 2 * HOUR,
                "spanIds": ["r1-span"],
                "running": false
            }])
        );
    }

    #[test]
    fn a_recording_block_serializes_with_camel_case_keys() {
        let mut input = input_with_recordings(vec![Recording {
            note: "n".into(),
            tag_id: Some("t1".into()),
            ..recording("r1", HOUR, Some(2 * HOUR))
        }]);
        input
            .names
            .projects
            .insert("p1".into(), project_ref("Portal"));
        input.names.tags.insert("t1".into(), "admin".into());

        let view = view(&input, &settings());

        assert_eq!(
            serde_json::to_value(&view.recordings).unwrap(),
            json!([{
                "id": "r1",
                "spanId": "r1-span",
                "spanCount": 1,
                "start": HOUR,
                "end": 2 * HOUR,
                "projectId": "p1",
                "projectName": "Portal",
                "clientName": null,
                "color": "blue",
                "shade": 4,
                "note": "n",
                "tag": "admin",
                "running": false,
                "clippedStart": false,
                "clippedEnd": false
            }])
        );
    }
}
