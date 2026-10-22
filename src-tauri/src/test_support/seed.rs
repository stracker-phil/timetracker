//! A deterministic data set of several years for the performance test and `timetracker dev seed`. Every
//! workday from 08:00 to 18:00 local holds `ACTIVITIES_PER_DAY` activity spans over 12 apps and 40 titles,
//! `AWAY_PER_DAY` away spans and `RECORDINGS_PER_DAY` recordings over 5 projects with notes and tags.
//! The same seed writes the same rows (only recording ids differ).

use chrono::{Datelike, Local, Months, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite};

use crate::db::away::AwayReason;
use crate::db::time::{Day, Ms};
use crate::db::{Db, clients, projects, tags};
use crate::error::AppResult;

pub const ACTIVITIES_PER_DAY: usize = 500;
pub const AWAY_PER_DAY: usize = 3;
pub const RECORDINGS_PER_DAY: usize = 6;

const MINUTE: Ms = 60_000;

const APPS: [(&str, &str); 12] = [
    ("Code", "com.microsoft.VSCode"),
    ("Safari", "com.apple.Safari"),
    ("Mail", "com.apple.mail"),
    ("Slack", "com.tinyspeck.slackmacgap"),
    ("Terminal", "com.apple.Terminal"),
    ("Xcode", "com.apple.dt.Xcode"),
    ("Figma", "com.figma.Desktop"),
    ("Notes", "com.apple.Notes"),
    ("Calendar", "com.apple.iCal"),
    ("Finder", "com.apple.finder"),
    ("Zoom", "us.zoom.xos"),
    ("Preview", "com.apple.Preview"),
];

/// `(index into APPS, title)`.
const TITLES: [(usize, &str); 40] = [
    (0, "main.rs — timetracker"),
    (0, "seed.rs — timetracker"),
    (0, "README.md — website"),
    (0, "api.ts — mobile-app"),
    (0, "settings.json — platform"),
    (1, "Pull request #482 · GitHub"),
    (1, "Rust documentation"),
    (1, "SQLite query planner"),
    (1, "Issue tracker — Platform"),
    (1, "Staging — Website"),
    (2, "Inbox"),
    (2, "Re: Quarterly planning"),
    (2, "Invoice October"),
    (3, "#general"),
    (3, "#platform-dev"),
    (3, "Direct message"),
    (4, "cargo test"),
    (4, "ssh deploy@staging"),
    (4, "git rebase"),
    (5, "MobileApp.xcodeproj"),
    (5, "Simulator — iPhone"),
    (5, "Instruments"),
    (6, "Landing page"),
    (6, "Onboarding flow"),
    (6, "Design system"),
    (7, "Meeting notes"),
    (7, "Ideas"),
    (7, "Todo"),
    (8, "Week 42"),
    (8, "Team standup"),
    (9, "Downloads"),
    (9, "Projects"),
    (9, "Screenshots"),
    (10, "Zoom Meeting"),
    (10, "Client call"),
    (10, "Sprint review"),
    (11, "contract.pdf"),
    (11, "mockup.png"),
    (11, "report.pdf"),
    (11, "receipt.pdf"),
];

/// `(client, project)`.
const PROJECTS: [(Option<&str>, &str); 5] = [
    (Some("Acme"), "Website"),
    (Some("Acme"), "Mobile app"),
    (Some("Globex"), "Platform"),
    (Some("Globex"), "Support"),
    (None, "Internal"),
];

const TAGS: [&str; 5] = ["meeting", "review", "planning", "bugfix", "research"];

const NOTES: [&str; 8] = [
    "",
    "Standup and planning",
    "Code review",
    "Fixed the login bug",
    "Release preparation",
    "Customer call",
    "Refactoring",
    "Documentation",
];

/// What a seed wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedStats {
    pub days: u32,
    pub activities: u64,
    pub away_spans: u64,
    pub recordings: u64,
    pub projects: u32,
    pub tags: u32,
}

/// Seeds the workdays of the `years` years up to yesterday.
pub async fn seed_years(db: &Db, years: u32, seed: u64) -> AppResult<SeedStats> {
    let last = Day::today().prev();
    let first = Day::from_date(last.date() - Months::new(12 * years)).next();
    seed_days(db, first, last, seed).await
}

/// Seeds the workdays of `first..=last`, one transaction per day. Clients, projects and tags are reused
/// by name when they exist.
pub async fn seed_days(db: &Db, first: Day, last: Day, seed: u64) -> AppResult<SeedStats> {
    let project_ids = ensure_projects(db).await?;
    let tag_ids = ensure_tags(db).await?;
    let mut rng = Rng(seed);
    let mut stats = SeedStats {
        projects: project_ids.len() as u32,
        tags: tag_ids.len() as u32,
        ..SeedStats::default()
    };
    for day in Day::range(&first, &last) {
        if matches!(day.date().weekday(), Weekday::Sat | Weekday::Sun) {
            continue;
        }
        let plan = plan_day(day, &mut rng, &project_ids, &tag_ids);
        write(db, &plan).await?;
        stats.days += 1;
        stats.activities += plan.activities.len() as u64;
        stats.away_spans += plan.away.len() as u64;
        stats.recordings += plan.recordings.len() as u64;
    }
    Ok(stats)
}

/// splitmix64: small, fast and deterministic, so the seed needs no `rand`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `0..n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// `lo..hi`.
    fn between(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo) as u64) as i64
    }
}

struct Activity {
    start: Ms,
    end: Ms,
    title: usize,
}

struct Away {
    start: Ms,
    end: Ms,
    reason: AwayReason,
}

struct Recording {
    start: Ms,
    end: Ms,
    project_id: String,
    note: &'static str,
    tag_id: Option<String>,
}

struct DayPlan {
    activities: Vec<Activity>,
    away: Vec<Away>,
    recordings: Vec<Recording>,
}

fn plan_day(day: Day, rng: &mut Rng, project_ids: &[String], tag_ids: &[String]) -> DayPlan {
    let open = eight_o_clock(day);
    let close = open + 600 * MINUTE;
    let mut away_at = |from: i64, to: i64, min_len: i64, max_len: i64, reason| {
        let start = open + rng.between(from, to) * MINUTE;
        let end = start + rng.between(min_len, max_len) * MINUTE;
        Away { start, end, reason }
    };
    let away = vec![
        away_at(60, 150, 5, 20, AwayReason::Idle),
        away_at(225, 255, 30, 60, AwayReason::Lock),
        away_at(390, 510, 5, 20, AwayReason::Idle),
    ];
    let away_ms: Ms = away.iter().map(|a| a.end - a.start).sum();
    DayPlan {
        activities: activities(rng, open, close, close - open - away_ms, &away),
        recordings: recordings(rng, open, project_ids, tag_ids),
        away,
    }
}

/// `ACTIVITIES_PER_DAY` back-to-back spans that share the working time; a span that runs into an away
/// span ends there and the next starts when it ends.
fn activities(rng: &mut Rng, open: Ms, close: Ms, working: Ms, away: &[Away]) -> Vec<Activity> {
    let weights: Vec<i64> = (0..ACTIVITIES_PER_DAY)
        .map(|_| rng.between(200, 1800))
        .collect();
    let total: i64 = weights.iter().sum();
    let mut next_away = away.iter().peekable();
    let mut cursor = open;
    let mut rows = Vec::with_capacity(ACTIVITIES_PER_DAY);
    for weight in weights {
        let start = cursor;
        let mut end = (start + working * weight / total).min(close);
        cursor = end;
        if let Some(a) = next_away.next_if(|a| end >= a.start) {
            end = a.start;
            cursor = a.end;
        }
        rows.push(Activity {
            start,
            end: end.max(start + 1),
            title: rng.below(TITLES.len()),
        });
    }
    rows
}

/// One recording of 60 to 90 minutes in each 100-minute slot from 08:00, so they never overlap.
fn recordings(
    rng: &mut Rng,
    open: Ms,
    project_ids: &[String],
    tag_ids: &[String],
) -> Vec<Recording> {
    (0..RECORDINGS_PER_DAY as i64)
        .map(|slot| {
            let start = open + (slot * 100 + rng.between(0, 10)) * MINUTE;
            let end = start + rng.between(60, 90) * MINUTE;
            let tag = rng.below(tag_ids.len());
            let tag_id = (rng.below(3) > 0).then(|| tag_ids[tag].clone());
            Recording {
                start,
                end,
                project_id: project_ids[rng.below(project_ids.len())].clone(),
                note: NOTES[rng.below(NOTES.len())],
                tag_id,
            }
        })
        .collect()
}

fn eight_o_clock(day: Day) -> Ms {
    let eight = day
        .date()
        .and_hms_opt(8, 0, 0)
        .expect("08:00 is a valid time");
    match eight.and_local_timezone(Local).earliest() {
        Some(t) => t.timestamp_millis(),
        None => day.bounds().0 + 8 * 60 * MINUTE,
    }
}

async fn write(db: &Db, plan: &DayPlan) -> AppResult<()> {
    let mut tx = db.pool().begin().await?;
    let mut insert: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"INSERT INTO activities (source, start, "end", app, bundle_id, title, url, path, private) "#,
    );
    insert.push_values(&plan.activities, |mut row, a| {
        let (app_index, title) = TITLES[a.title];
        let (app, bundle_id) = APPS[app_index];
        let url = (app == "Safari").then(|| format!("https://example.com/{}", a.title));
        row.push_bind("tracker")
            .push_bind(a.start)
            .push_bind(a.end)
            .push_bind(app)
            .push_bind(bundle_id)
            .push_bind(title)
            .push_bind(url)
            .push_bind(None::<String>)
            .push_bind(false);
    });
    insert.build().execute(&mut *tx).await?;

    let mut insert: QueryBuilder<Sqlite> =
        QueryBuilder::new(r#"INSERT INTO away_spans (start, "end", reason) "#);
    insert.push_values(&plan.away, |mut row, a| {
        row.push_bind(a.start).push_bind(a.end).push_bind(a.reason);
    });
    insert.build().execute(&mut *tx).await?;

    for r in &plan.recordings {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO recordings (id, project_id, note, tag_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?5)"#,
        )
        .bind(&id)
        .bind(&r.project_id)
        .bind(r.note)
        .bind(&r.tag_id)
        .bind(r.end)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO recording_spans (id, recording_id, start, "end") VALUES (?1, ?1, ?2, ?3)"#,
        )
        .bind(&id)
        .bind(r.start)
        .bind(r.end)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn ensure_projects(db: &Db) -> AppResult<Vec<String>> {
    let mut ids = Vec::with_capacity(PROJECTS.len());
    for (client_name, name) in PROJECTS {
        let client_id = match client_name {
            Some(client_name) => Some(ensure_client(db, client_name).await?),
            None => None,
        };
        let existing = projects::list(db, true)
            .await?
            .into_iter()
            .find(|p| p.name == name && p.client_id == client_id);
        let project = match existing {
            Some(project) => project,
            None => {
                let billable = client_id.is_some();
                let input = projects::ProjectInput {
                    client_id,
                    name: name.into(),
                    color: "teal".into(),
                    shade: None,
                    billable,
                };
                projects::insert(db, input).await?
            }
        };
        ids.push(project.id);
    }
    Ok(ids)
}

async fn ensure_client(db: &Db, name: &str) -> AppResult<String> {
    if let Some(client) = clients::list(db, true)
        .await?
        .into_iter()
        .find(|c| c.name == name)
    {
        return Ok(client.id);
    }
    let input = clients::ClientInput {
        name: name.into(),
        color: String::new(),
    };
    Ok(clients::insert(db, input).await?.id)
}

async fn ensure_tags(db: &Db) -> AppResult<Vec<String>> {
    let existing = tags::list(db).await?;
    let mut ids = Vec::with_capacity(TAGS.len());
    for name in TAGS {
        let id = match existing.iter().find(|t| t.name.eq_ignore_ascii_case(name)) {
            Some(tag) => tag.id.clone(),
            None => tags::insert(db, name).await?.id,
        };
        ids.push(id);
    }
    Ok(ids)
}

/// Whether any activity starts before `day`: a seed into it would overlap real data.
pub async fn has_activity_before(db: &Db, day: Day) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM activities WHERE start < ?1)")
            .bind(day.bounds().0)
            .fetch_one(db.pool())
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::activities::{self, ActivityRow};
    use crate::db::away::{self, AwayRow};
    use crate::db::recordings::{self, Recording as RecordingRow};
    use crate::test_support::activity;

    const HOUR: Ms = 60 * MINUTE;

    fn day(text: &str) -> Day {
        Day::parse(text).expect("valid day")
    }

    fn monday() -> Day {
        day("2026-03-02")
    }

    fn sunday() -> Day {
        day("2026-03-08")
    }

    fn bounds() -> (Ms, Ms) {
        (monday().bounds().0, sunday().bounds().1)
    }

    async fn seeded_week(seed: u64) -> (Db, SeedStats) {
        let db = Db::open_memory().await.expect("open db");
        let stats = seed_days(&db, monday(), sunday(), seed)
            .await
            .expect("seed week");
        (db, stats)
    }

    async fn activities_of(db: &Db) -> Vec<ActivityRow> {
        let (from, to) = bounds();
        activities::list_for_range(db, from, to).await.unwrap()
    }

    async fn away_of(db: &Db) -> Vec<AwayRow> {
        let (from, to) = bounds();
        away::list_for_range(db, from, to).await.unwrap()
    }

    async fn recordings_of(db: &Db) -> Vec<RecordingRow> {
        let (from, to) = bounds();
        recordings::list_for_range(db, from, to).await.unwrap()
    }

    fn workdays() -> Vec<Day> {
        Day::range(&monday(), &sunday())
            .into_iter()
            .filter(|d| !matches!(d.date().weekday(), Weekday::Sat | Weekday::Sun))
            .collect()
    }

    fn in_day<T>(rows: &[T], day: &Day, span: impl Fn(&T) -> (Ms, Ms)) -> Vec<(Ms, Ms)> {
        let (from, to) = day.bounds();
        rows.iter()
            .map(span)
            .filter(|(start, _)| *start >= from && *start < to)
            .collect()
    }

    #[tokio::test]
    async fn stats_count_the_workdays_of_the_range() {
        let (_db, stats) = seeded_week(1).await;
        assert_eq!(
            stats,
            SeedStats {
                days: 5,
                activities: 5 * ACTIVITIES_PER_DAY as u64,
                away_spans: 5 * AWAY_PER_DAY as u64,
                recordings: 5 * RECORDINGS_PER_DAY as u64,
                projects: 5,
                tags: 5,
            }
        );
    }

    #[tokio::test]
    async fn seeded_projects_are_billable_exactly_when_they_have_a_client() {
        let (db, _) = seeded_week(1).await;
        let projects = projects::list(&db, true).await.unwrap();
        assert_eq!(projects.len(), 5);
        for project in projects {
            assert_eq!(
                project.billable,
                project.client_id.is_some(),
                "{}",
                project.name
            );
        }
    }

    #[tokio::test]
    async fn stats_serialize_in_camel_case() {
        let (_db, stats) = seeded_week(1).await;
        assert_eq!(
            serde_json::to_value(&stats).unwrap(),
            serde_json::json!({
                "days": 5,
                "activities": 2500,
                "awaySpans": 15,
                "recordings": 30,
                "projects": 5,
                "tags": 5,
            })
        );
    }

    #[tokio::test]
    async fn rows_in_the_database_match_the_stats() {
        let (db, stats) = seeded_week(1).await;
        assert_eq!(activities_of(&db).await.len() as u64, stats.activities);
        assert_eq!(away_of(&db).await.len() as u64, stats.away_spans);
        assert_eq!(recordings_of(&db).await.len() as u64, stats.recordings);
    }

    #[tokio::test]
    async fn weekends_get_no_rows() {
        let (db, _) = seeded_week(1).await;
        let activities = activities_of(&db).await;
        for weekend in [day("2026-03-07"), day("2026-03-08")] {
            let (from, to) = weekend.bounds();
            assert!(
                activities
                    .iter()
                    .all(|a| a.end.is_some_and(|end| end <= from || a.start >= to)),
                "{}",
                weekend.date()
            );
            assert!(away_of(&db).await.iter().all(|a| a.start < from));
            assert!(recordings_of(&db).await.iter().all(|r| r.start() < from));
        }
    }

    #[tokio::test]
    async fn every_workday_gets_its_share_of_rows() {
        let (db, _) = seeded_week(1).await;
        let (activities, away, recordings) = (
            activities_of(&db).await,
            away_of(&db).await,
            recordings_of(&db).await,
        );
        for day in workdays() {
            let label = day.date().to_string();
            assert_eq!(
                in_day(&activities, &day, |a| (a.start, a.end.unwrap())).len(),
                ACTIVITIES_PER_DAY,
                "{label}"
            );
            assert_eq!(
                in_day(&away, &day, |a| (a.start, a.end.unwrap())).len(),
                AWAY_PER_DAY,
                "{label}"
            );
            assert_eq!(
                in_day(&recordings, &day, |r| (r.start(), r.end().unwrap())).len(),
                RECORDINGS_PER_DAY,
                "{label}"
            );
        }
    }

    #[tokio::test]
    async fn activity_and_away_spans_of_a_workday_are_closed_and_never_overlap() {
        let (db, _) = seeded_week(7).await;
        let activities = activities_of(&db).await;
        let away = away_of(&db).await;
        for day in workdays() {
            let (from, to) = day.bounds();
            let mut spans = in_day(&activities, &day, |a| (a.start, a.end.unwrap()));
            spans.extend(in_day(&away, &day, |a| (a.start, a.end.unwrap())));
            spans.sort();
            let label = day.date().to_string();
            assert!(spans.iter().all(|(s, e)| s < e), "{label}");
            for pair in spans.windows(2) {
                assert!(pair[0].1 <= pair[1].0, "{label}: {pair:?}");
            }
            let (first, last) = (spans[0].0, spans.iter().map(|s| s.1).max().unwrap());
            assert!(first >= from && last <= to, "{label}");
            assert!(last - first <= 10 * HOUR, "{label}");
        }
    }

    #[tokio::test]
    async fn recordings_of_a_workday_never_overlap_and_use_the_seed_projects() {
        let (db, _) = seeded_week(3).await;
        let projects = projects::list(&db, true).await.unwrap();
        let recordings = recordings_of(&db).await;
        for r in &recordings {
            assert!(projects.iter().any(|p| p.id == r.project_id));
            assert!(r.end().unwrap() > r.start());
        }
        for pair in recordings.windows(2) {
            assert!(pair[0].end().unwrap() <= pair[1].start(), "{pair:?}");
        }
    }

    #[tokio::test]
    async fn every_seeded_recording_has_one_closed_span() {
        let (db, _) = seeded_week(3).await;
        for r in recordings_of(&db).await {
            assert_eq!(r.spans.len(), 1, "{}", r.id);
            assert!(r.spans[0].end.is_some(), "{}", r.id);
        }
    }

    #[tokio::test]
    async fn seeding_the_same_range_and_seed_twice_gives_the_same_activities() {
        let (first, _) = seeded_week(42).await;
        let (second, _) = seeded_week(42).await;
        assert_eq!(activities_of(&first).await, activities_of(&second).await);
        assert_eq!(away_of(&first).await, away_of(&second).await);
    }

    #[tokio::test]
    async fn seeding_the_same_range_and_seed_twice_gives_the_same_recordings_apart_from_ids() {
        async fn shape(db: &Db) -> Vec<(String, Ms, Option<Ms>, String, Option<String>)> {
            let projects = projects::list(db, true).await.unwrap();
            let all_tags = tags::list(db).await.unwrap();
            recordings_of(db)
                .await
                .into_iter()
                .map(|r| {
                    let (start, end) = (r.start(), r.end());
                    let project = projects.iter().find(|p| p.id == r.project_id).unwrap();
                    let tag = r
                        .tag_id
                        .and_then(|id| all_tags.iter().find(|t| t.id == id))
                        .map(|t| t.name.clone());
                    (project.name.clone(), start, end, r.note, tag)
                })
                .collect()
        }
        let (first, _) = seeded_week(42).await;
        let (second, _) = seeded_week(42).await;
        assert_eq!(shape(&first).await, shape(&second).await);
    }

    #[tokio::test]
    async fn a_different_seed_gives_different_activities() {
        let (first, _) = seeded_week(1).await;
        let (second, _) = seeded_week(2).await;
        let strip = |rows: Vec<ActivityRow>| -> Vec<(Ms, String)> {
            rows.into_iter().map(|a| (a.start, a.title)).collect()
        };
        assert_ne!(
            strip(activities_of(&first).await),
            strip(activities_of(&second).await)
        );
    }

    #[tokio::test]
    async fn seeding_again_reuses_clients_projects_and_tags_by_name() {
        let (db, _) = seeded_week(1).await;
        let stats = seed_days(&db, day("2026-03-09"), day("2026-03-13"), 1)
            .await
            .unwrap();
        assert_eq!((stats.projects, stats.tags), (5, 5));

        let mut project_names: Vec<String> = projects::list(&db, true)
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        project_names.sort();
        assert_eq!(
            project_names,
            ["Internal", "Mobile app", "Platform", "Support", "Website"]
        );
        assert_eq!(clients::list(&db, true).await.unwrap().len(), 2);
        let tag_names: Vec<String> = tags::list(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name.to_lowercase())
            .collect();
        for name in TAGS {
            assert_eq!(tag_names.iter().filter(|n| *n == name).count(), 1, "{name}");
        }
    }

    #[tokio::test]
    async fn an_empty_database_has_no_activity_before_a_day() {
        let db = Db::open_memory().await.unwrap();
        assert!(!has_activity_before(&db, Day::today()).await.unwrap());
    }

    #[tokio::test]
    async fn activity_on_the_day_itself_is_not_before_it() {
        let db = Db::open_memory().await.unwrap();
        let today = Day::today();
        activity(&db, today.bounds().0 + HOUR, None, "Code", "main.rs").await;
        assert!(!has_activity_before(&db, today).await.unwrap());
    }

    #[tokio::test]
    async fn activity_before_the_day_is_detected() {
        let db = Db::open_memory().await.unwrap();
        let today = Day::today();
        activity(&db, today.bounds().0 - HOUR, None, "Code", "main.rs").await;
        assert!(has_activity_before(&db, today).await.unwrap());
    }
}
