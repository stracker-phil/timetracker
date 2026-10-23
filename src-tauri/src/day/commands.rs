use std::time::Instant;

use tauri::State;

use super::apps::{KnownApp, known_apps};
use super::{DayTotals, DayView, load, totals_of};
use crate::db::time::{Day, now_ms};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn day_view(state: State<'_, AppState>, day: String) -> AppResult<DayView> {
    let day = Day::parse(&day)?;
    let started = Instant::now();
    let result = load(&state.data.db()?, day, &state.settings.get(), now_ms()).await?;
    let blocks: usize = result.lanes.iter().map(|l| l.blocks.len()).sum();
    log::debug!(
        "day_view {day}: {blocks} blocks in {} ms",
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub async fn day_totals(state: State<'_, AppState>, day: Day) -> AppResult<DayTotals> {
    totals_of(&state.data.db()?, day, now_ms()).await
}

#[tauri::command]
pub async fn apps_list(state: State<'_, AppState>) -> AppResult<Vec<KnownApp>> {
    known_apps(&state.data.db()?, &state.settings.get(), now_ms()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::{RecordingBlock, view_of};
    use crate::db::activities::NewActivity;
    use crate::db::away::{AwayReason, NewAway};
    use crate::db::time::Ms;
    use crate::db::{Db, activities, away, projects, tags};
    use crate::store::settings::Settings;
    use crate::store::settings::{Continuity, Noise};
    use crate::test_support::{activity, client_with_color, project, recording, temp_db};

    const HOUR: Ms = 3_600_000;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn activity_spans(view: &DayView) -> Vec<(Ms, Ms)> {
        view.lanes
            .iter()
            .find(|l| l.id == "activity")
            .unwrap()
            .blocks
            .iter()
            .map(|b| (b.start, b.end))
            .collect()
    }

    #[tokio::test]
    async fn a_span_crossing_midnight_shows_clipped_on_both_days() {
        let db = Db::open_memory().await.unwrap();
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let midnight = tuesday.bounds().0;
        activity(&db, midnight - HOUR, Some(midnight + 2 * HOUR), "Code", "").await;
        let now = midnight + 10 * HOUR;

        let first = load(&db, monday, &Settings::default(), now).await.unwrap();
        let second = load(&db, tuesday, &Settings::default(), now).await.unwrap();

        assert_eq!(activity_spans(&first), [(midnight - HOUR, midnight)]);
        assert_eq!(activity_spans(&second), [(midnight, midnight + 2 * HOUR)]);
        assert_eq!(first.totals.tracked_ms, HOUR);
        assert_eq!(second.totals.tracked_ms, 2 * HOUR);
    }

    #[tokio::test]
    async fn load_returns_the_day_with_its_bounds_and_leaves_other_days_out() {
        let db = Db::open_memory().await.unwrap();
        let (today, next) = (day("2026-01-05"), day("2026-01-06"));
        let bounds = today.bounds();
        activity(&db, bounds.0 + HOUR, Some(bounds.0 + 2 * HOUR), "Code", "").await;
        activity(
            &db,
            next.bounds().0 + HOUR,
            Some(next.bounds().0 + 2 * HOUR),
            "Mail",
            "",
        )
        .await;

        let view = load(&db, today, &Settings::default(), bounds.1 + HOUR)
            .await
            .unwrap();

        assert_eq!((view.day, view.bounds), (today, bounds));
        assert_eq!(
            activity_spans(&view),
            [(bounds.0 + HOUR, bounds.0 + 2 * HOUR)]
        );
    }

    #[tokio::test]
    async fn an_open_activity_and_an_open_sleep_span_end_at_now() {
        let db = Db::open_memory().await.unwrap();
        let today = day("2026-01-05");
        let start = today.bounds().0 + HOUR;
        activity(&db, start, None, "Code", "").await;
        away::insert(
            &db,
            NewAway {
                start: start + HOUR,
                reason: AwayReason::Sleep,
            },
        )
        .await
        .unwrap();
        let now = start + 3 * HOUR;

        let view = load(&db, today, &Settings::default(), now).await.unwrap();

        assert_eq!(activity_spans(&view), [(start, now)]);
        assert_eq!(view.barriers, [(start + HOUR, now)]);
        assert_eq!(view.totals.away_ms, 2 * HOUR);
    }

    #[tokio::test]
    async fn load_applies_the_app_overrides_of_the_settings() {
        let db = Db::open_memory().await.unwrap();
        let today = day("2026-01-05");
        let start = today.bounds().0 + HOUR;
        activities::insert(
            &db,
            NewActivity {
                source: "tracker".into(),
                start,
                app: "Mail".into(),
                bundle_id: Some("com.apple.mail".into()),
                title: String::new(),
                url: None,
                path: None,
                private: false,
            },
        )
        .await
        .unwrap();
        let settings = Settings::default()
            .with_app(
                "com.apple.mail",
                &crate::store::settings::AppSettingPatch {
                    own_lane: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();

        let view = load(&db, today, &settings, start + HOUR).await.unwrap();

        let ids: Vec<&str> = view.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["summary", "app:com.apple.mail", "activity"]);
    }

    #[tokio::test]
    async fn load_shows_recordings_with_their_project_client_and_tags() {
        use crate::db::projects::ProjectInput;
        use crate::db::recordings::{NewRecording, insert};

        let db = Db::open_memory().await.unwrap();
        let today = day("2026-01-05");
        let start = today.bounds().0 + HOUR;
        let acme = client_with_color(&db, "Acme", "teal").await;
        let portal = projects::insert(
            &db,
            ProjectInput {
                client_id: Some(acme.id),
                name: "Portal".into(),
                color: String::new(),
                shade: Some(3),
                billable: false,
            },
        )
        .await
        .unwrap();
        let alpha = tags::insert(&db, "Alpha").await.unwrap();
        tags::set_emoji(&db, &alpha.id, Some("🧪")).await.unwrap();
        let saved = insert(
            &db,
            NewRecording {
                id: "r1".into(),
                project_id: portal.id.clone(),
                spans: vec![(start, Some(start + HOUR))],
                note: "fix login".into(),
                tag_id: Some(alpha.id),
            },
        )
        .await
        .unwrap();
        let span_id = saved.spans[0].id.clone();

        let view = load(&db, today, &Settings::default(), start + 5 * HOUR)
            .await
            .unwrap();

        assert_eq!(
            view.recordings,
            [RecordingBlock {
                id: saved.id,
                span_id,
                span_count: 1,
                start,
                end: start + HOUR,
                project_id: portal.id,
                project_name: "Portal".into(),
                client_name: Some("Acme".into()),
                color: "teal".into(),
                shade: 3,
                note: "fix login".into(),
                tag: Some("Alpha".into()),
                tag_emoji: Some("🧪".into()),
                running: false,
                clipped_start: false,
                clipped_end: false,
            }]
        );
        assert_eq!(view.timesheet.len(), 1);
        assert_eq!(view.timesheet[0].tag.as_deref(), Some("Alpha"));
        assert_eq!(view.timesheet[0].tag_emoji.as_deref(), Some("🧪"));
        assert_eq!(view.totals.recorded_ms, HOUR);
    }

    #[tokio::test]
    async fn a_recording_across_midnight_shows_clipped_on_both_days_and_covers_activity() {
        let db = Db::open_memory().await.unwrap();
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let midnight = tuesday.bounds().0;
        let p = project(&db, "Portal").await;
        recording(&db, &p.id, midnight - HOUR, Some(midnight + 2 * HOUR)).await;
        activity(&db, midnight - HOUR, Some(midnight + HOUR), "Code", "").await;
        let now = midnight + 10 * HOUR;

        let first = load(&db, monday, &Settings::default(), now).await.unwrap();
        let second = load(&db, tuesday, &Settings::default(), now).await.unwrap();

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
        assert_eq!(
            (first.totals.recorded_ms, second.totals.recorded_ms),
            (HOUR, 2 * HOUR)
        );
        let covered = |view: &DayView| {
            let blocks = &view
                .lanes
                .iter()
                .find(|l| l.id == "activity")
                .unwrap()
                .blocks;
            !blocks.is_empty() && blocks.iter().all(|b| b.covered)
        };
        assert!(covered(&first) && covered(&second));
    }

    #[tokio::test]
    async fn a_stored_recording_with_spans_on_two_days_shows_one_block_per_span_on_each_day() {
        use crate::db::recordings::{NewRecording, insert};

        let db = Db::open_memory().await.unwrap();
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let midnight = tuesday.bounds().0;
        let p = project(&db, "Portal").await;
        let saved = insert(
            &db,
            NewRecording {
                id: "r1".into(),
                project_id: p.id,
                spans: vec![
                    (midnight - 3 * HOUR, Some(midnight - 2 * HOUR)),
                    (midnight - HOUR, Some(midnight + HOUR)),
                    (midnight + 5 * HOUR, Some(midnight + 6 * HOUR)),
                ],
                note: String::new(),
                tag_id: None,
            },
        )
        .await
        .unwrap();
        let now = midnight + 10 * HOUR;

        let first = load(&db, monday, &Settings::default(), now).await.unwrap();
        let second = load(&db, tuesday, &Settings::default(), now).await.unwrap();

        let shape = |view: &DayView| {
            assert!(view.recordings.iter().all(|b| b.id == "r1"));
            view.recordings
                .iter()
                .map(|b| {
                    let clipped = (b.clipped_start, b.clipped_end);
                    (b.span_id.clone(), b.span_count, b.start, b.end, clipped)
                })
                .collect::<Vec<_>>()
        };
        let span = |i: usize| saved.spans[i].id.clone();
        let at = |hours: i64| midnight + hours * HOUR;
        assert_eq!(
            shape(&first),
            [
                (span(0), 3, at(-3), at(-2), (false, false)),
                (span(1), 3, at(-1), at(0), (false, true)),
            ]
        );
        assert_eq!(
            shape(&second),
            [
                (span(1), 3, at(0), at(1), (true, false)),
                (span(2), 3, at(5), at(6), (false, false)),
            ]
        );
        assert_eq!(
            (first.totals.recorded_ms, second.totals.recorded_ms),
            (2 * HOUR, 2 * HOUR)
        );
    }

    #[tokio::test]
    async fn a_running_recording_shows_up_to_now() {
        let db = Db::open_memory().await.unwrap();
        let today = day("2026-01-05");
        let start = today.bounds().0 + HOUR;
        let p = project(&db, "Portal").await;
        recording(&db, &p.id, start, None).await;

        let view = load(&db, today, &Settings::default(), start + 2 * HOUR)
            .await
            .unwrap();

        let block = &view.recordings[0];
        assert_eq!((block.end, block.running), (start + 2 * HOUR, true));
        assert_eq!(view.totals.recorded_ms, 2 * HOUR);
    }

    #[tokio::test]
    #[ignore = "perf: 3,000 rows on a temp SQLite file"]
    async fn a_day_with_three_thousand_activities_loads_quickly() {
        const ROWS: i64 = 3_000;
        let (_dir, db) = temp_db().await;
        let today = day("2026-01-05");
        let start = today.bounds().0;
        let mut tx = db.pool().begin().await.unwrap();
        for i in 0..ROWS {
            let row = NewActivity {
                source: "tracker".into(),
                start: start + i * 28_000,
                app: format!("App {}", i % 7),
                bundle_id: Some(format!("test.app{}", i % 7)),
                title: format!("Window {i}"),
                url: None,
                path: None,
                private: false,
            };
            activities::insert_on(&mut tx, &row, Some(start + i * 28_000 + 60_000))
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();

        let mut settings = Settings::default();
        settings.timeline.summary_lane = false;
        settings.timeline.group_by_app = false;
        settings.timeline.continuity = Continuity::Off;
        settings.timeline.noise = Noise::Off;

        let started = Instant::now();
        let view = load(&db, today, &settings, start + 24 * HOUR)
            .await
            .unwrap();
        let elapsed = started.elapsed();

        let blocks: usize = view.lanes.iter().map(|l| l.blocks.len()).sum();
        println!("load of {ROWS} activities took {elapsed:?}");
        assert_eq!(blocks, ROWS as usize);
        assert!(elapsed.as_millis() < 500, "took {elapsed:?}");
    }

    #[tokio::test]
    #[ignore = "db: temp SQLite file"]
    async fn totals_of_a_stored_day_clip_to_it_and_match_the_view() {
        let (_dir, db) = temp_db().await;
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let midnight = tuesday.bounds().0;
        let p = project(&db, "Portal").await;
        recording(&db, &p.id, midnight - HOUR, Some(midnight + HOUR)).await;
        recording(&db, &p.id, midnight + 3 * HOUR, None).await;
        activity(&db, midnight - HOUR, Some(midnight + 2 * HOUR), "Code", "").await;
        activity(&db, midnight + HOUR, Some(midnight + 3 * HOUR), "Mail", "").await;
        away::insert(
            &db,
            NewAway {
                start: midnight + 4 * HOUR,
                reason: AwayReason::Idle,
            },
        )
        .await
        .unwrap();
        let now = midnight + 6 * HOUR;

        let first = totals_of(&db, monday, now).await.unwrap();
        let second = totals_of(&db, tuesday, now).await.unwrap();

        assert_eq!(
            first,
            DayTotals {
                recorded_ms: HOUR,
                tracked_ms: HOUR,
                away_ms: 0,
            }
        );
        assert_eq!(
            second,
            DayTotals {
                recorded_ms: HOUR + 3 * HOUR,
                tracked_ms: 3 * HOUR,
                away_ms: 2 * HOUR,
            }
        );
        let settings = Settings::default();
        assert_eq!(
            second,
            load(&db, tuesday, &settings, now).await.unwrap().totals
        );
        assert_eq!(
            first,
            view_of(&db, monday, &settings, now).await.unwrap().totals
        );
    }

    mod suggestions {
        use super::*;
        use crate::rules::store;
        use crate::rules::testing::{on, plain_settings, save_rule};
        use crate::rules::types::RuleField::App;
        use crate::rules::types::RuleOp;

        fn spans(view: &DayView) -> Vec<(Ms, Ms)> {
            view.suggestions.iter().map(|s| (s.start, s.end)).collect()
        }

        async fn slack_rule(db: &Db) -> crate::rules::types::Rule {
            let p = project(db, "Portal").await;
            save_rule(db, "Slack", &p.id, &[(App, RuleOp::Is, "Slack")]).await
        }

        #[tokio::test]
        async fn load_suggests_the_unrecorded_time_of_the_saved_rules() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            let rule = slack_rule(&db).await;
            activity(&db, on(today, 9, 0), Some(on(today, 10, 0)), "Slack", "").await;
            activity(&db, on(today, 10, 0), Some(on(today, 11, 0)), "Code", "").await;
            let settings = plain_settings(Continuity::Off);

            let view = load(&db, today, &settings, on(today, 20, 0)).await.unwrap();

            assert_eq!(view.suggestions.len(), 1);
            let suggestion = &view.suggestions[0];
            assert_eq!(
                (
                    suggestion.rule_ids.as_slice(),
                    suggestion.project_name.as_str(),
                    suggestion.start,
                    suggestion.end
                ),
                (
                    [rule.id.clone()].as_slice(),
                    "Portal",
                    on(today, 9, 0),
                    on(today, 10, 0)
                )
            );
        }

        async fn store(db: &Db, project_id: &str, today: Day, from: (i64, i64), to: (i64, i64)) {
            use crate::db::recordings::Span;
            use crate::db::stored_suggestions::{self, StoredSuggestion};
            let suggestion = StoredSuggestion {
                id: "s1".into(),
                project_id: project_id.into(),
                note: "review".into(),
                tag_id: None,
                source: "claude".into(),
                day: today,
                spans: vec![Span {
                    start: on(today, from.0, from.1),
                    end: on(today, to.0, to.1),
                }],
                created_at: 1,
                updated_at: 1,
            };
            let mut conn = db.pool().acquire().await.unwrap();
            stored_suggestions::put_on(&mut conn, &suggestion)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn load_shows_the_stored_suggestions_and_the_rules_fill_the_time_left() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            let rule = slack_rule(&db).await;
            activity(&db, on(today, 9, 0), Some(on(today, 11, 0)), "Slack", "").await;
            store(&db, &rule.project_id, today, (9, 30), (10, 0)).await;
            let settings = plain_settings(Continuity::Off);

            let view = load(&db, today, &settings, on(today, 20, 0)).await.unwrap();

            assert_eq!(
                spans(&view),
                [
                    (on(today, 9, 0), on(today, 9, 30)),
                    (on(today, 9, 30), on(today, 10, 0)),
                    (on(today, 10, 0), on(today, 11, 0)),
                ]
            );
            let owners: Vec<Option<&str>> = view
                .suggestions
                .iter()
                .map(|s| s.stored_id.as_deref())
                .collect();
            assert_eq!(owners, [None, Some("s1"), None]);
        }

        #[tokio::test]
        async fn view_of_leaves_out_stored_suggestions() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            let p = project(&db, "Portal").await;
            store(&db, &p.id, today, (9, 0), (10, 0)).await;
            let settings = plain_settings(Continuity::Off);

            let view = view_of(&db, today, &settings, on(today, 20, 0))
                .await
                .unwrap();

            assert!(view.suggestions.is_empty());
        }

        #[tokio::test]
        async fn view_of_and_a_day_without_rules_have_no_suggestions() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            activity(&db, on(today, 9, 0), Some(on(today, 10, 0)), "Slack", "").await;
            let settings = plain_settings(Continuity::Off);
            let before = load(&db, today, &settings, on(today, 20, 0)).await.unwrap();
            slack_rule(&db).await;

            let view = view_of(&db, today, &settings, on(today, 20, 0))
                .await
                .unwrap();

            assert!(before.suggestions.is_empty());
            assert!(view.suggestions.is_empty());
        }

        #[tokio::test]
        async fn disabled_rules_and_rules_of_archived_projects_suggest_nothing() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            activity(&db, on(today, 9, 0), Some(on(today, 10, 0)), "Slack", "").await;
            let settings = plain_settings(Continuity::Off);
            let disabled = slack_rule(&db).await;
            store::set_enabled(&db, &disabled.id, false).await.unwrap();
            let archived = project(&db, "Old").await;
            save_rule(
                &db,
                "Slack too",
                &archived.id,
                &[(App, RuleOp::Is, "Slack")],
            )
            .await;
            projects::set_archived(&db, &archived.id, true)
                .await
                .unwrap();

            let view = load(&db, today, &settings, on(today, 20, 0)).await.unwrap();

            assert!(view.suggestions.is_empty());
        }

        #[tokio::test]
        async fn recordings_cut_their_time_out_of_the_suggestions() {
            let db = Db::open_memory().await.unwrap();
            let today = day("2026-01-05");
            let rule = slack_rule(&db).await;
            activity(&db, on(today, 9, 0), Some(on(today, 10, 0)), "Slack", "").await;
            recording(
                &db,
                &rule.project_id,
                on(today, 9, 20),
                Some(on(today, 9, 40)),
            )
            .await;
            let settings = plain_settings(Continuity::Off);

            let view = load(&db, today, &settings, on(today, 20, 0)).await.unwrap();

            assert_eq!(
                spans(&view),
                [
                    (on(today, 9, 0), on(today, 9, 20)),
                    (on(today, 9, 40), on(today, 10, 0)),
                ]
            );
        }

        #[tokio::test]
        async fn the_continuity_setting_bridges_gaps_unless_the_user_was_asleep_or_locked_out() {
            let (first, second) = (
                |d| (on(d, 9, 0), on(d, 9, 10)),
                |d| (on(d, 9, 15), on(d, 9, 30)),
            );
            let cases = [
                ("continuity off", Continuity::Off, None, false),
                ("a short gap", Continuity::Regular, None, true),
                (
                    "an idle gap",
                    Continuity::Regular,
                    Some(AwayReason::Idle),
                    true,
                ),
                (
                    "a sleep",
                    Continuity::Regular,
                    Some(AwayReason::Sleep),
                    false,
                ),
                ("a lock", Continuity::Regular, Some(AwayReason::Lock), false),
                (
                    "a pause",
                    Continuity::Regular,
                    Some(AwayReason::Paused),
                    false,
                ),
            ];
            for (name, continuity, away_reason, joined) in cases {
                let db = Db::open_memory().await.unwrap();
                let today = day("2026-01-05");
                slack_rule(&db).await;
                for (start, end) in [first(today), second(today)] {
                    activity(&db, start, Some(end), "Slack", "").await;
                }
                if let Some(reason) = away_reason {
                    let start = on(today, 9, 11);
                    away::insert(&db, NewAway { start, reason }).await.unwrap();
                    away::close_open(&db, on(today, 9, 14)).await.unwrap();
                }

                let view = load(&db, today, &plain_settings(continuity), on(today, 20, 0))
                    .await
                    .unwrap();

                let expected = if joined {
                    vec![(on(today, 9, 0), on(today, 9, 30))]
                } else {
                    vec![first(today), second(today)]
                };
                assert_eq!(spans(&view), expected, "{name}");
            }
        }
    }
}
