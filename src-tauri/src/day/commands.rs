use std::collections::HashMap;
use std::time::Instant;

use tauri::State;

use super::apps::{KnownApp, known_apps};
use super::{DayInput, DayView, ProjectRef, ViewSettings, view};
use crate::db::time::{Day, Ms, now_ms};
use crate::db::{Db, activities, away, clients, projects, recordings, tags};
use crate::error::AppResult;
use crate::state::AppState;
use crate::store::settings::Settings;

/// Reads the rows that share time with the day (spans from the day before included) and builds its view.
pub async fn load(db: &Db, day: Day, settings: &Settings, now: Ms) -> AppResult<DayView> {
    let bounds = day.bounds();
    let input = DayInput {
        day,
        bounds,
        now,
        activities: activities::list_for_range(db, bounds.0, bounds.1).await?,
        away: away::list_for_range(db, bounds.0, bounds.1).await?,
        recordings: recordings::list_for_range(db, bounds.0, bounds.1).await?,
        projects: project_refs(db).await?,
        tags: tags::list(db)
            .await?
            .into_iter()
            .map(|t| (t.id, t.name))
            .collect(),
    };
    Ok(view(&input, &ViewSettings::from(settings)))
}

/// Archived projects and clients included: old recordings still show their names.
async fn project_refs(db: &Db) -> AppResult<HashMap<String, ProjectRef>> {
    let clients = clients::list(db, true).await?;
    Ok(projects::list(db, true)
        .await?
        .into_iter()
        .map(|p| {
            let client_name = p
                .client_id
                .as_ref()
                .and_then(|id| clients.iter().find(|c| &c.id == id))
                .map(|c| c.name.clone());
            let project = ProjectRef {
                name: p.name,
                client_name,
                color: p.color,
                shade: p.shade,
            };
            (p.id, project)
        })
        .collect())
}

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
pub async fn apps_list(state: State<'_, AppState>) -> AppResult<Vec<KnownApp>> {
    known_apps(&state.data.db()?, &state.settings.get(), now_ms()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::RecordingBlock;
    use crate::db::activities::NewActivity;
    use crate::db::away::{AwayReason, NewAway};
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
    async fn an_open_activity_and_an_open_away_span_end_at_now() {
        let db = Db::open_memory().await.unwrap();
        let today = day("2026-01-05");
        let start = today.bounds().0 + HOUR;
        activity(&db, start, None, "Code", "").await;
        away::insert(
            &db,
            NewAway {
                start: start + HOUR,
                reason: AwayReason::Idle,
            },
        )
        .await
        .unwrap();
        let now = start + 3 * HOUR;

        let view = load(&db, today, &Settings::default(), now).await.unwrap();

        assert_eq!(activity_spans(&view), [(start, now)]);
        assert_eq!(
            view.away
                .iter()
                .map(|a| (a.start, a.end))
                .collect::<Vec<_>>(),
            [(start + HOUR, now)]
        );
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
        assert_eq!(ids, ["summary", "activity", "app:com.apple.mail"]);
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
            },
        )
        .await
        .unwrap();
        let zeta = tags::insert(&db, "zeta").await.unwrap();
        let alpha = tags::insert(&db, "Alpha").await.unwrap();
        let saved = insert(
            &db,
            NewRecording {
                id: "r1".into(),
                project_id: portal.id.clone(),
                start,
                end: Some(start + HOUR),
                note: "fix login".into(),
                tag_ids: vec![zeta.id, alpha.id],
            },
        )
        .await
        .unwrap();

        let view = load(&db, today, &Settings::default(), start + 5 * HOUR)
            .await
            .unwrap();

        assert_eq!(
            view.recordings,
            [RecordingBlock {
                id: saved.id,
                start,
                end: start + HOUR,
                project_id: portal.id,
                project_name: "Portal".into(),
                client_name: Some("Acme".into()),
                color: "teal".into(),
                shade: 3,
                note: "fix login".into(),
                tags: vec!["Alpha".into(), "zeta".into()],
                running: false,
                clipped_start: false,
                clipped_end: false,
            }]
        );
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
                start: start + i * 10_000,
                app: format!("App {}", i % 7),
                bundle_id: Some(format!("test.app{}", i % 7)),
                title: format!("Window {i}"),
                url: None,
                path: None,
                private: false,
            };
            activities::insert_on(&mut tx, &row, Some(start + i * 10_000 + 9_000))
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
        let view = load(&db, today, &settings, start + 12 * HOUR)
            .await
            .unwrap();
        let elapsed = started.elapsed();

        let blocks: usize = view.lanes.iter().map(|l| l.blocks.len()).sum();
        println!("load of {ROWS} activities took {elapsed:?}");
        assert_eq!(blocks, ROWS as usize);
        assert!(elapsed.as_millis() < 500, "took {elapsed:?}");
    }
}
