//! A small, believable history for the README screenshots (`timetracker dev seed --showcase`). Every
//! session is recorded except the last one of the most recent day, so that Day view also shows activity
//! still to record.

use chrono::{Datelike, Weekday};
use sqlx::{QueryBuilder, Sqlite};

mod data;

use self::data::{Block, INTERNAL_COLOR, INTERRUPTIONS, PROJECTS, SCHEDULES, TAGS, Window};
use super::seed::{Rng, SeedStats, ensure_tags, insert_recording, local_ms};
use crate::db::away::AwayReason;
use crate::db::time::{Day, Ms};
use crate::db::{Db, clients, projects};
use crate::error::AppResult;

pub const WORKDAYS: usize = 15;

const MINUTE: Ms = 60_000;

/// Clients, projects and tags are reused by name when they exist.
pub async fn seed_showcase(db: &Db, seed: u64) -> AppResult<SeedStats> {
    let project_ids = ensure_projects(db).await?;
    let tag_ids = ensure_tags(db, &TAGS).await?;
    let mut rng = Rng(seed);
    let mut stats = SeedStats {
        projects: project_ids.len() as u32,
        tags: tag_ids.len() as u32,
        ..SeedStats::default()
    };
    for (index, day) in workdays_before(Day::today(), WORKDAYS)
        .into_iter()
        .enumerate()
    {
        let schedule = SCHEDULES[index % SCHEDULES.len()];
        let shift = rng.between(-2, 3) * 5;
        let leave_last_open = index == 0;
        let mut tx = db.pool().begin().await?;
        let sessions = schedule
            .iter()
            .filter(|b| matches!(b, Block::Session { .. }))
            .count();
        let mut session_index = 0;
        for block in schedule.iter() {
            match *block {
                Block::Away { start, end, reason } => {
                    let span = (local_ms(day, start + shift), local_ms(day, end + shift));
                    insert_away(&mut tx, span, reason).await?;
                    stats.away_spans += 1;
                }
                Block::Session {
                    start,
                    end,
                    work,
                    project,
                    note,
                    tag,
                } => {
                    let span = (local_ms(day, start + shift), local_ms(day, end + shift));
                    let windows = activities(&mut rng, span, work.windows());
                    stats.activities += windows.len() as u64;
                    insert_activities(&mut tx, &windows).await?;
                    session_index += 1;
                    if leave_last_open && session_index == sessions {
                        continue;
                    }
                    let tag_id = tag.map(|t| tag_ids[t].as_str());
                    insert_recording(&mut tx, &project_ids[project], note, tag_id, span).await?;
                    stats.recordings += 1;
                }
            }
        }
        tx.commit().await?;
        stats.days += 1;
    }
    Ok(stats)
}

/// The `count` workdays before `today`, the most recent first.
fn workdays_before(today: Day, count: usize) -> Vec<Day> {
    let mut days = Vec::with_capacity(count);
    let mut day = today.prev();
    while days.len() < count {
        if !matches!(day.date().weekday(), Weekday::Sat | Weekday::Sun) {
            days.push(day);
        }
        day = day.prev();
    }
    days
}

/// Back-to-back windows that fill the span, never the same one twice in a row, with an occasional short
/// interruption.
fn activities(rng: &mut Rng, (start, end): (Ms, Ms), windows: &[Window]) -> Vec<(Ms, Ms, Window)> {
    let mut rows = Vec::new();
    let mut cursor = start;
    let mut last = usize::MAX;
    while cursor < end {
        let (window, minutes) = if rows.len() > 1 && rng.below(10) == 0 {
            (
                INTERRUPTIONS[rng.below(INTERRUPTIONS.len())],
                rng.between(2, 5),
            )
        } else {
            let mut pick = rng.below(windows.len());
            if pick == last && windows.len() > 1 {
                pick = (pick + 1) % windows.len();
            }
            last = pick;
            (windows[pick], rng.between(10, 35))
        };
        let next = (cursor + minutes * MINUTE).min(end);
        rows.push((cursor, next, window));
        cursor = next;
    }
    rows
}

async fn insert_activities(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    rows: &[(Ms, Ms, Window)],
) -> AppResult<()> {
    let mut insert: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"INSERT INTO activities (source, start, "end", app, bundle_id, title, url, path, private) "#,
    );
    insert.push_values(
        rows,
        |mut row, (start, end, (app, bundle_id, title, url))| {
            row.push_bind("tracker")
                .push_bind(start)
                .push_bind(end)
                .push_bind(app)
                .push_bind(bundle_id)
                .push_bind(title)
                .push_bind(url)
                .push_bind(None::<String>)
                .push_bind(false);
        },
    );
    insert.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_away(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    (start, end): (Ms, Ms),
    reason: AwayReason,
) -> AppResult<()> {
    sqlx::query(r#"INSERT INTO away_spans (start, "end", reason) VALUES (?1, ?2, ?3)"#)
        .bind(start)
        .bind(end)
        .bind(reason)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn ensure_projects(db: &Db) -> AppResult<Vec<String>> {
    let mut ids = Vec::with_capacity(PROJECTS.len());
    for (client, name) in PROJECTS {
        let client_id = match client {
            Some((client, color)) => Some(ensure_client(db, client, color).await?),
            None => None,
        };
        let existing = projects::list(db, true)
            .await?
            .into_iter()
            .find(|p| p.name == name && p.client_id == client_id);
        let id = match existing {
            Some(project) => project.id,
            None => {
                let input = projects::ProjectInput {
                    billable: client_id.is_some(),
                    client_id,
                    name: name.into(),
                    color: INTERNAL_COLOR.into(),
                    shade: None,
                };
                projects::insert(db, input).await?.id
            }
        };
        ids.push(id);
    }
    Ok(ids)
}

async fn ensure_client(db: &Db, name: &str, color: &str) -> AppResult<String> {
    let existing = clients::list(db, true).await?;
    if let Some(client) = existing.into_iter().find(|c| c.name == name) {
        return Ok(client.id);
    }
    let input = clients::ClientInput {
        name: name.into(),
        color: color.into(),
    };
    Ok(clients::insert(db, input).await?.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::activities::{self, ActivityRow};
    use crate::db::away::{self, AwayRow};
    use crate::db::recordings::{self, Recording};
    use crate::db::tags;

    async fn seeded(seed: u64) -> (Db, SeedStats) {
        let db = Db::open_memory().await.expect("open db");
        let stats = seed_showcase(&db, seed).await.expect("seed showcase");
        (db, stats)
    }

    fn everything() -> (Ms, Ms) {
        (0, Day::today().next().bounds().1)
    }

    async fn activities_of(db: &Db) -> Vec<ActivityRow> {
        let (from, to) = everything();
        activities::list_for_range(db, from, to).await.unwrap()
    }

    async fn away_of(db: &Db) -> Vec<AwayRow> {
        let (from, to) = everything();
        away::list_for_range(db, from, to).await.unwrap()
    }

    async fn recordings_of(db: &Db) -> Vec<Recording> {
        let (from, to) = everything();
        recordings::list_for_range(db, from, to).await.unwrap()
    }

    fn expected_workdays() -> Vec<Day> {
        let mut days = Vec::new();
        let mut day = Day::today().prev();
        while days.len() < WORKDAYS {
            if !matches!(day.date().weekday(), Weekday::Sat | Weekday::Sun) {
                days.push(day);
            }
            day = day.prev();
        }
        days
    }

    fn closed(row_end: Option<Ms>) -> Ms {
        row_end.expect("span is closed")
    }

    #[tokio::test]
    async fn stats_match_the_rows_in_the_database() {
        let (db, stats) = seeded(1).await;
        assert_eq!(stats.days as usize, WORKDAYS);
        assert_eq!(activities_of(&db).await.len() as u64, stats.activities);
        assert_eq!(away_of(&db).await.len() as u64, stats.away_spans);
        assert_eq!(recordings_of(&db).await.len() as u64, stats.recordings);
        assert_eq!(
            stats.projects as usize,
            projects::list(&db, true).await.unwrap().len()
        );
        let tag_names: Vec<String> = tags::list(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name.to_lowercase())
            .collect();
        assert_eq!(
            TAGS.iter()
                .filter(|n| tag_names.iter().any(|t| t == *n))
                .count(),
            stats.tags as usize
        );
    }

    #[tokio::test]
    async fn rows_fall_on_exactly_the_last_workdays_before_today() {
        let (db, _) = seeded(1).await;
        let mut days: Vec<Day> = activities_of(&db)
            .await
            .iter()
            .map(|a| Day::of(a.start))
            .chain(away_of(&db).await.iter().map(|a| Day::of(a.start)))
            .chain(recordings_of(&db).await.iter().map(|r| Day::of(r.start())))
            .collect();
        days.sort_by_key(|d| d.bounds().0);
        days.dedup();
        let mut expected = expected_workdays();
        expected.sort_by_key(|d| d.bounds().0);
        assert_eq!(days, expected);
    }

    #[tokio::test]
    async fn activity_and_away_spans_are_closed_and_never_overlap_within_a_day() {
        let (db, _) = seeded(7).await;
        let mut spans: Vec<(Ms, Ms)> = activities_of(&db)
            .await
            .iter()
            .map(|a| (a.start, closed(a.end)))
            .chain(away_of(&db).await.iter().map(|a| (a.start, closed(a.end))))
            .collect();
        spans.sort();
        for span in &spans {
            assert!(span.0 < span.1, "{span:?}");
            assert_eq!(Day::of(span.0), Day::of(span.1 - 1), "{span:?}");
        }
        for pair in spans.windows(2) {
            assert!(pair[0].1 <= pair[1].0, "{pair:?}");
        }
    }

    #[tokio::test]
    async fn every_activity_is_a_tracker_span_with_an_app_and_a_bundle_id() {
        let (db, _) = seeded(3).await;
        for a in activities_of(&db).await {
            assert_eq!(a.source, "tracker");
            assert!(!a.app.is_empty(), "{a:?}");
            assert!(
                a.bundle_id.as_deref().is_some_and(|id| !id.is_empty()),
                "{a:?}"
            );
        }
    }

    #[tokio::test]
    async fn every_day_has_a_lunch_lock_and_a_short_idle_break() {
        let (db, _) = seeded(3).await;
        let away = away_of(&db).await;
        for day in expected_workdays() {
            let reasons: Vec<AwayReason> = away
                .iter()
                .filter(|a| Day::of(a.start) == day)
                .map(|a| a.reason)
                .collect();
            assert_eq!(
                reasons,
                [AwayReason::Lock, AwayReason::Idle],
                "{}",
                day.date()
            );
        }
    }

    #[tokio::test]
    async fn recordings_have_one_closed_span_a_note_and_never_overlap() {
        let (db, _) = seeded(3).await;
        let recordings = recordings_of(&db).await;
        for r in &recordings {
            assert_eq!(r.spans.len(), 1, "{}", r.id);
            assert!(
                r.spans[0].end.is_some_and(|end| end > r.spans[0].start),
                "{}",
                r.id
            );
            assert!(!r.note.is_empty(), "{}", r.id);
        }
        for pair in recordings.windows(2) {
            assert!(pair[0].end().unwrap() <= pair[1].start(), "{pair:?}");
        }
    }

    #[tokio::test]
    async fn only_the_most_recent_day_has_activity_left_to_record() {
        let (db, _) = seeded(5).await;
        let recordings = recordings_of(&db).await;
        let latest = expected_workdays()[0];
        for a in activities_of(&db).await {
            let covered = recordings
                .iter()
                .any(|r| r.start() <= a.start && closed(a.end) <= r.end().unwrap());
            if Day::of(a.start) == latest {
                continue;
            }
            assert!(covered, "{a:?}");
        }

        let last_recorded_end = recordings
            .iter()
            .filter(|r| Day::of(r.start()) == latest)
            .map(|r| r.end().unwrap())
            .max()
            .expect("the latest day has recordings");
        assert!(
            activities_of(&db)
                .await
                .iter()
                .any(|a| Day::of(a.start) == latest && a.start >= last_recorded_end)
        );
    }

    #[tokio::test]
    async fn clients_and_projects_are_billable_exactly_when_they_have_a_client() {
        let (db, _) = seeded(1).await;
        let clients = clients::list(&db, true).await.unwrap();
        let mut client_names: Vec<(String, String)> = clients
            .iter()
            .map(|c| (c.name.clone(), c.color.clone()))
            .collect();
        client_names.sort();
        assert_eq!(
            client_names,
            [
                ("Fernweh Travel".to_string(), "teal".to_string()),
                ("Lumen Labs".to_string(), "purple".to_string()),
                ("Northwind".to_string(), "blue".to_string()),
            ]
        );
        let projects = projects::list(&db, true).await.unwrap();
        assert_eq!(projects.len(), 5);
        for project in &projects {
            assert_eq!(
                project.billable,
                project.client_id.is_some(),
                "{}",
                project.name
            );
        }
        let internal = projects.iter().find(|p| p.name == "Internal").unwrap();
        assert_eq!(
            (internal.client_id.clone(), internal.color.as_str()),
            (None, "gold")
        );
    }

    #[tokio::test]
    async fn seeding_twice_reuses_clients_projects_and_tags() {
        let (db, first) = seeded(1).await;
        let counts = |db: &Db| {
            let db = db.clone();
            async move {
                (
                    clients::list(&db, true).await.unwrap().len(),
                    projects::list(&db, true).await.unwrap().len(),
                    tags::list(&db).await.unwrap().len(),
                )
            }
        };
        let before = counts(&db).await;
        let second = seed_showcase(&db, 2).await.unwrap();
        assert_eq!(counts(&db).await, before);
        assert_eq!((before.0, before.1), (3, 5));
        let tag_names: Vec<String> = tags::list(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name.to_lowercase())
            .collect();
        for name in TAGS {
            assert_eq!(tag_names.iter().filter(|n| *n == name).count(), 1, "{name}");
        }
        assert_eq!((second.projects, second.tags), (first.projects, first.tags));
    }
}
