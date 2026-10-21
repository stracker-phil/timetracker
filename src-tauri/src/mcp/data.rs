//! The data tools: the database opened read-only on first use, the settings read on every call.

use std::path::PathBuf;

use chrono::Local;
use clap::ValueEnum;
use serde::Serialize;
use tokio::sync::OnceCell;

use crate::cli::{report, time};
use crate::day;
use crate::db::activities::ActivityRow;
use crate::db::away::AwayRow;
use crate::db::time::{Day, Ms, now_ms, to_local_iso};
use crate::db::{self, DB_FILE, Db, activities, away, clients, projects, tags};
use crate::error::{AppError, AppResult};
use crate::remote::StatusReport;
use crate::reports::cli::{ReportRequest, filter_of};
use crate::reports::{self, Range, ReportFilter, ReportGroup, ReportResult, ReportRow};
use crate::store::settings::{Group, Rounding, Settings};
use crate::tracker::TrackerState;

pub const DEFAULT_RECORDINGS: u32 = 500;
pub const MAX_RECORDINGS: u32 = 5000;
pub const ACTIVITY_HIDDEN: &str = "Activity is not exposed to MCP. Enable it in Settings → Data.";

/// What to report on; names resolve like the CLI's `--project` and `--tag`.
pub struct ReportQuery {
    pub range: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub group_by: Option<String>,
    pub client: Option<String>,
    pub project: Option<String>,
    pub tag: Option<String>,
}

pub struct RecordingsQuery {
    pub from: String,
    pub to: String,
    pub project: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct ReportOut {
    pub from: Day,
    pub to: Day,
    pub group_by: Group,
    pub rounding: Rounding,
    pub total_min: f64,
    pub total_rounded_min: f64,
    /// Grouped by tag, a recording with several tags counts in each of their groups.
    pub tag_overlap: bool,
    pub groups: Vec<ReportGroupOut>,
}

#[derive(Serialize)]
pub struct ReportGroupOut {
    pub label: String,
    pub minutes: f64,
    pub rounded_min: f64,
    pub notes: Vec<NoteOut>,
}

#[derive(Serialize)]
pub struct NoteOut {
    pub text: String,
    pub days: Vec<Day>,
}

#[derive(Serialize)]
pub struct RecordingOut {
    pub id: String,
    pub day: Day,
    pub start_iso: String,
    pub end_iso: String,
    pub duration_min: f64,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub note: String,
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct ProjectOut {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub archived: bool,
}

#[derive(Serialize)]
pub struct TagOut {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct DaySummary {
    pub day: Day,
    pub recorded_min: f64,
    pub tracked_min: f64,
    pub away_min: f64,
    pub by_project: Vec<SummaryGroup>,
    pub by_tag: Vec<SummaryGroup>,
}

#[derive(Serialize)]
pub struct SummaryGroup {
    pub name: String,
    pub minutes: f64,
    /// Distinct notes in the order they first occur.
    pub notes: Vec<String>,
}

#[derive(Serialize)]
pub struct ActivityOut {
    pub activity: Vec<SpanOut>,
    pub away: Vec<AwayOut>,
}

#[derive(Serialize)]
pub struct SpanOut {
    pub start_iso: String,
    pub end_iso: String,
    pub app: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `tracker` or `external:<name>`.
    pub source: String,
}

#[derive(Serialize)]
pub struct AwayOut {
    pub start_iso: String,
    pub end_iso: String,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct StatusOut {
    pub version: String,
    pub tracker: Option<TrackerOut>,
    pub timer: Option<TimerOut>,
    pub window_visible: bool,
}

#[derive(Serialize)]
pub struct TrackerOut {
    pub state: TrackerState,
    pub text: String,
    pub current: Option<CurrentOut>,
}

/// The title and URL only when activity is exposed.
#[derive(Serialize)]
pub struct CurrentOut {
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub since_iso: String,
}

#[derive(Serialize)]
pub struct TimerOut {
    pub project: String,
    pub note: String,
    pub start_iso: String,
    pub elapsed_min: f64,
}

/// Minutes to one decimal.
fn minutes(ms: Ms) -> f64 {
    (ms as f64 / 6_000.0).round() / 10.0
}

pub fn day_of(text: &str) -> AppResult<Day> {
    time::day(text, now_ms(), &Local).map_err(AppError::InvalidInput)
}

fn period_of(range: Option<String>, from: Option<String>, to: Option<String>) -> AppResult<Range> {
    let words = match (range, from, to) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(AppError::InvalidInput(
                "Give either `range` or `from` and `to`, not both".into(),
            ));
        }
        (Some(range), None, None) => vec![range],
        (None, Some(from), Some(to)) => vec![from, to],
        (None, Some(day), None) | (None, None, Some(day)) => vec![day],
        (None, None, None) => Vec::new(),
    };
    report::range(&words, now_ms(), &Local).map_err(AppError::InvalidInput)
}

fn group_of(text: Option<String>) -> AppResult<Group> {
    let Some(text) = text else {
        return Ok(Group::Project);
    };
    Group::from_str(&text, false).map_err(|_| {
        AppError::InvalidInput(format!(
            "Not a grouping: `{text}` (project, tag, day, week or month)"
        ))
    })
}

pub struct Core {
    config_dir: PathBuf,
    db: OnceCell<Db>,
}

impl Core {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            db: OnceCell::new(),
        }
    }

    fn settings(&self) -> Settings {
        Settings::peek(&self.config_dir)
    }

    pub fn expose_activity(&self) -> bool {
        self.settings().mcp.expose_activity
    }

    /// Opened on first use, so the handshake succeeds before the app ever ran; a failed open is tried
    /// again on the next call.
    async fn db(&self) -> AppResult<&Db> {
        self.db
            .get_or_try_init(|| async {
                let file = db::data_dir(&self.settings(), &self.config_dir).join(DB_FILE);
                Db::open_read_only(&file).await
            })
            .await
    }

    async fn run(&self, request: ReportRequest) -> AppResult<ReportResult> {
        let db = self.db().await?;
        let filter = filter_of(db, request).await?;
        self.run_filter(&filter).await
    }

    async fn run_filter(&self, filter: &ReportFilter) -> AppResult<ReportResult> {
        reports::run(self.db().await?, filter, Day::today(), now_ms()).await
    }

    /// The report and its text, as the Reports screen copies it.
    pub async fn report(&self, query: ReportQuery) -> AppResult<(ReportResult, String)> {
        let request = ReportRequest {
            range: period_of(query.range, query.from, query.to)?,
            group_by: group_of(query.group_by)?,
            rounding: self.settings().reports.rounding,
            clients: query.client.into_iter().collect(),
            projects: query.project.into_iter().collect(),
            tags: query.tag.into_iter().collect(),
        };
        let result = self.run(request).await?;
        let text = reports::text(&result);
        Ok((result, text))
    }

    pub async fn recordings(&self, query: RecordingsQuery) -> AppResult<Vec<RecordingOut>> {
        let (from, to) = (day_of(&query.from)?, day_of(&query.to)?);
        let request = ReportRequest {
            range: Range::Custom { from, to },
            group_by: Group::Project,
            rounding: Rounding::None,
            clients: Vec::new(),
            projects: query.project.into_iter().collect(),
            tags: query.tag.into_iter().collect(),
        };
        let db = self.db().await?;
        let filter = ReportFilter {
            include_archived: true,
            ..filter_of(db, request).await?
        };
        let result = self.run_filter(&filter).await?;
        let mut rows: Vec<ReportRow> = result.groups.into_iter().flat_map(|g| g.rows).collect();
        rows.sort_by(|a, b| (a.start, &a.id).cmp(&(b.start, &b.id)));
        let limit = query
            .limit
            .unwrap_or(DEFAULT_RECORDINGS)
            .clamp(1, MAX_RECORDINGS) as usize;
        Ok(rows
            .into_iter()
            .take(limit)
            .map(|row| RecordingOut {
                start_iso: to_local_iso(row.start),
                end_iso: to_local_iso(row.end),
                duration_min: minutes(row.duration_ms),
                id: row.id,
                day: row.day,
                project: row.project,
                client: row.client,
                note: row.note,
                tags: row.tags,
            })
            .collect())
    }

    pub async fn projects(&self) -> AppResult<Vec<ProjectOut>> {
        let db = self.db().await?;
        let include_archived = true;
        let clients = clients::list(db, include_archived).await?;
        Ok(projects::list(db, include_archived)
            .await?
            .into_iter()
            .map(|project| ProjectOut {
                client: project
                    .client_id
                    .as_ref()
                    .and_then(|id| clients.iter().find(|c| &c.id == id))
                    .map(|c| c.name.clone()),
                id: project.id,
                name: project.name,
                color: project.color,
                archived: project.archived_at.is_some(),
            })
            .collect())
    }

    pub async fn tags(&self) -> AppResult<Vec<TagOut>> {
        Ok(tags::list(self.db().await?)
            .await?
            .into_iter()
            .map(|tag| TagOut {
                id: tag.id,
                name: tag.name,
            })
            .collect())
    }

    pub async fn day_summary(&self, day: &str) -> AppResult<DaySummary> {
        let day = day_of(day)?;
        let settings = self.settings();
        let totals = day::commands::view_of(self.db().await?, day, &settings, now_ms())
            .await?
            .totals;
        let grouped = |group_by| ReportFilter {
            range: Range::Custom { from: day, to: day },
            group_by,
            include_archived: true,
            ..ReportFilter::default()
        };
        let by_project = self.run_filter(&grouped(Group::Project)).await?;
        let by_tag = self.run_filter(&grouped(Group::Tag)).await?;
        Ok(DaySummary {
            day,
            recorded_min: minutes(totals.recorded_ms),
            tracked_min: minutes(totals.tracked_ms),
            away_min: minutes(totals.away_ms),
            by_project: summary_groups(by_project.groups),
            by_tag: summary_groups(by_tag.groups),
        })
    }

    pub async fn activity(&self, day: &str) -> AppResult<ActivityOut> {
        if !self.expose_activity() {
            return Err(AppError::PermissionRequired(ACTIVITY_HIDDEN.into()));
        }
        let (from, to) = day_of(day)?.bounds();
        let now = now_ms();
        let db = self.db().await?;
        Ok(ActivityOut {
            activity: spans_out(activities::list_clipped(db, from, to, now).await?, now),
            away: away_out(away::list_clipped(db, from, to, now).await?, now),
        })
    }
}

/// A private span keeps only its app.
fn spans_out(rows: Vec<ActivityRow>, now: Ms) -> Vec<SpanOut> {
    rows.into_iter()
        .map(|row| {
            let (title, url, path) = if row.private {
                (String::new(), None, None)
            } else {
                (row.title, row.url, row.path)
            };
            SpanOut {
                start_iso: to_local_iso(row.start),
                end_iso: to_local_iso(row.end.unwrap_or(now)),
                app: row.app,
                title,
                url,
                path,
                source: row.source,
            }
        })
        .collect()
}

fn away_out(rows: Vec<AwayRow>, now: Ms) -> Vec<AwayOut> {
    rows.into_iter()
        .map(|row| AwayOut {
            start_iso: to_local_iso(row.start),
            end_iso: to_local_iso(row.end.unwrap_or(now)),
            reason: row.reason.label(),
        })
        .collect()
}

fn summary_groups(groups: Vec<ReportGroup>) -> Vec<SummaryGroup> {
    groups
        .into_iter()
        .map(|group| SummaryGroup {
            name: group.label,
            minutes: minutes(group.duration_ms),
            notes: group.notes.into_iter().map(|note| note.text).collect(),
        })
        .collect()
}

pub fn report_out(result: &ReportResult) -> ReportOut {
    ReportOut {
        from: result.from,
        to: result.to,
        group_by: result.filter.group_by,
        rounding: result.filter.rounding,
        total_min: minutes(result.total_ms),
        total_rounded_min: minutes(result.total_rounded_ms),
        tag_overlap: result.tag_overlap,
        groups: result
            .groups
            .iter()
            .map(|group| ReportGroupOut {
                label: group.label.clone(),
                minutes: minutes(group.duration_ms),
                rounded_min: minutes(group.rounded_ms),
                notes: group
                    .notes
                    .iter()
                    .map(|note| NoteOut {
                        text: note.text.clone(),
                        days: note.days.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The app's status report in minutes and local times; window titles only when activity is exposed.
pub fn status_out(report: StatusReport, expose_activity: bool) -> StatusOut {
    StatusOut {
        version: report.version,
        tracker: report.tracker.map(|tracker| TrackerOut {
            state: tracker.state,
            text: tracker.text,
            current: tracker.current.map(|row| current_out(row, expose_activity)),
        }),
        timer: report.timer.map(|timer| TimerOut {
            project: timer.project_name,
            note: timer.recording.note,
            start_iso: to_local_iso(timer.recording.start),
            elapsed_min: minutes(timer.elapsed_ms),
        }),
        window_visible: report.window_visible,
    }
}

fn current_out(row: ActivityRow, expose_activity: bool) -> CurrentOut {
    let shown = expose_activity && !row.private;
    CurrentOut {
        app: row.app,
        title: shown.then_some(row.title),
        url: row.url.filter(|_| shown),
        since_iso: to_local_iso(row.start),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::db::activities::NewActivity;
    use crate::db::away::{AwayReason, NewAway};
    use crate::db::projects::{Project, ProjectInput};
    use crate::db::recordings::{self, NewRecording, Recording};
    use crate::db::tags::Tag;
    use crate::error::ErrorKind;
    use crate::test_support::{client, project, temp_config_dir, temp_db};

    const DAY: &str = "2026-10-20";

    fn day() -> Day {
        Day::parse(DAY).unwrap()
    }

    fn at(hour: i64, minute: i64) -> Ms {
        day().bounds().0 + (hour * 60 + minute) * 60_000
    }

    fn iso(hour: i64, minute: i64) -> String {
        to_local_iso(at(hour, minute))
    }

    fn write_settings(dir: &Path, value: Value) {
        std::fs::write(dir.join("settings.json"), value.to_string()).unwrap();
    }

    async fn add_recording(
        db: &Db,
        project: &Project,
        (start, end): (Ms, Ms),
        note: &str,
        tags: &[&Tag],
    ) -> Recording {
        recordings::insert(
            db,
            NewRecording {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project.id.clone(),
                start,
                end: Some(end),
                note: note.into(),
                tag_ids: tags.iter().map(|t| t.id.clone()).collect(),
            },
        )
        .await
        .unwrap()
    }

    fn new_activity(source: &str, start: Ms, app: &str, title: &str) -> NewActivity {
        NewActivity {
            source: source.into(),
            start,
            app: app.into(),
            bundle_id: None,
            title: title.into(),
            url: None,
            path: None,
            private: false,
        }
    }

    async fn add_span(db: &Db, activity: NewActivity, end: Ms) {
        let id = activities::insert(db, activity).await.unwrap();
        sqlx::query(r#"UPDATE activities SET "end" = ?2 WHERE id = ?1"#)
            .bind(id)
            .bind(end)
            .execute(db.pool())
            .await
            .unwrap();
    }

    /// 2026-10-20: Portal (client Acme) 09:00-10:30 "fix login" and 13:00-14:00 "review", both tagged
    /// development; Podcast 08:00-08:25 "outline" and 11:00-11:30 "edit episode", untagged. Tracker
    /// activity 09:00-10:30, an external span 11:00-11:30, away 12:00-12:20.
    struct Seed {
        dir: TempDir,
        db: Db,
        core: Core,
        portal: Project,
        podcast: Project,
        development: Tag,
        login: Recording,
    }

    async fn seeded() -> Seed {
        let (dir, db) = temp_db().await;
        let acme = client(&db, "Acme").await;
        let portal = projects::insert(
            &db,
            ProjectInput {
                client_id: Some(acme.id),
                name: "Portal".into(),
                color: "blue".into(),
                shade: None,
            },
        )
        .await
        .unwrap();
        let podcast = project(&db, "Podcast").await;
        let development = tags::list(&db)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.name == "development")
            .unwrap();
        add_recording(&db, &podcast, (at(11, 0), at(11, 30)), "edit episode", &[]).await;
        let login = add_recording(
            &db,
            &portal,
            (at(9, 0), at(10, 30)),
            "fix login",
            &[&development],
        )
        .await;
        add_recording(
            &db,
            &portal,
            (at(13, 0), at(14, 0)),
            "review",
            &[&development],
        )
        .await;
        add_recording(&db, &podcast, (at(8, 0), at(8, 25)), "outline", &[]).await;

        add_span(
            &db,
            NewActivity {
                path: Some("/src/main.rs".into()),
                ..new_activity("tracker", at(9, 0), "Xcode", "main.rs")
            },
            at(10, 0),
        )
        .await;
        add_span(
            &db,
            NewActivity {
                url: Some("https://bank.example/accounts".into()),
                private: true,
                ..new_activity("tracker", at(10, 0), "Safari", "Accounts")
            },
            at(10, 30),
        )
        .await;
        add_span(
            &db,
            NewActivity {
                url: Some("https://plan.example/standup".into()),
                ..new_activity("external:planner", at(11, 0), "Planner", "Standup")
            },
            at(11, 30),
        )
        .await;
        away::insert(
            &db,
            NewAway {
                start: at(12, 0),
                reason: AwayReason::Idle,
            },
        )
        .await
        .unwrap();
        away::close_open(&db, at(12, 20)).await.unwrap();

        let core = Core::new(dir.path().to_path_buf());
        Seed {
            dir,
            db,
            core,
            portal,
            podcast,
            development,
            login,
        }
    }

    fn report_query(range: Option<&str>) -> ReportQuery {
        ReportQuery {
            range: range.map(String::from),
            from: None,
            to: None,
            group_by: None,
            client: None,
            project: None,
            tag: None,
        }
    }

    fn recordings_query(
        project: Option<&str>,
        tag: Option<&str>,
        limit: Option<u32>,
    ) -> RecordingsQuery {
        RecordingsQuery {
            from: DAY.into(),
            to: DAY.into(),
            project: project.map(String::from),
            tag: tag.map(String::from),
            limit,
        }
    }

    async fn recorded_projects(seed: &Seed, query: RecordingsQuery) -> Vec<String> {
        seed.core
            .recordings(query)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.project)
            .collect()
    }

    fn named<'a>(groups: &'a Value, name_key: &str, name: &str) -> &'a Value {
        groups
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g[name_key] == name)
            .unwrap_or_else(|| panic!("no group {name} in {groups}"))
    }

    #[tokio::test]
    async fn todays_report_by_project_is_the_report_text_of_the_same_filter() {
        let seed = seeded().await;
        let today = Day::today().bounds().0;
        add_recording(
            &seed.db,
            &seed.portal,
            (today + 3_600_000, today + 7_200_000),
            "today work",
            &[],
        )
        .await;

        let (result, text) = seed.core.report(report_query(Some("today"))).await.unwrap();

        let expected = reports::run(&seed.db, &ReportFilter::default(), Day::today(), now_ms())
            .await
            .unwrap();
        assert_eq!(text, reports::text(&expected));
        assert_eq!(result.total_ms, 3_600_000);
    }

    #[tokio::test]
    async fn a_report_without_a_range_is_todays() {
        let seed = seeded().await;

        let (result, _) = seed.core.report(report_query(None)).await.unwrap();

        assert_eq!((result.from, result.to), (Day::today(), Day::today()));
    }

    #[tokio::test]
    async fn the_json_report_is_in_minutes_and_rounded_as_the_settings_say() {
        let seed = seeded().await;
        write_settings(
            seed.dir.path(),
            json!({"reports": {"rounding": "nearest30"}}),
        );

        let (result, _) = seed.core.report(report_query(Some(DAY))).await.unwrap();

        let out = serde_json::to_value(report_out(&result)).unwrap();
        let mut keys: Vec<&String> = out.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "from",
                "group_by",
                "groups",
                "rounding",
                "tag_overlap",
                "to",
                "total_min",
                "total_rounded_min",
            ]
        );
        assert_eq!(out["from"], DAY);
        assert_eq!(out["to"], DAY);
        assert_eq!(out["group_by"], "project");
        assert_eq!(out["rounding"], "nearest30");
        assert_eq!(out["total_min"], 205.0);
        assert_eq!(out["total_rounded_min"], 210.0);
        assert_eq!(out["tag_overlap"], false);
        assert_eq!(out["groups"].as_array().unwrap().len(), 2);
        assert_eq!(
            named(&out["groups"], "label", "Acme / Portal"),
            &json!({
                "label": "Acme / Portal",
                "minutes": 150.0,
                "rounded_min": 150.0,
                "notes": [
                    {"text": "fix login", "days": [DAY]},
                    {"text": "review", "days": [DAY]},
                ],
            })
        );
        assert_eq!(
            named(&out["groups"], "label", "Podcast"),
            &json!({
                "label": "Podcast",
                "minutes": 55.0,
                "rounded_min": 60.0,
                "notes": [
                    {"text": "outline", "days": [DAY]},
                    {"text": "edit episode", "days": [DAY]},
                ],
            })
        );
    }

    #[tokio::test]
    async fn a_report_takes_a_range_of_two_days() {
        let seed = seeded().await;
        let mut query = report_query(None);
        query.from = Some("2026-10-19".into());
        query.to = Some(DAY.into());

        let (result, _) = seed.core.report(query).await.unwrap();

        assert_eq!(
            (result.from.to_string(), result.to.to_string()),
            ("2026-10-19".to_string(), DAY.to_string())
        );
        assert_eq!(result.total_ms, 205 * 60_000);
    }

    #[tokio::test]
    async fn a_report_refuses_a_range_together_with_from_or_to() {
        let seed = seeded().await;
        for (from, to) in [(Some(DAY), None), (None, Some(DAY)), (Some(DAY), Some(DAY))] {
            let mut query = report_query(Some("week"));
            query.from = from.map(String::from);
            query.to = to.map(String::from);

            let err = seed.core.report(query).await.unwrap_err();

            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{from:?} {to:?}");
            assert_eq!(
                err.to_string(),
                "Give either `range` or `from` and `to`, not both"
            );
        }
    }

    #[tokio::test]
    async fn a_report_refuses_an_unknown_grouping() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.group_by = Some("year".into());

        let err = seed.core.report(query).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "Not a grouping: `year` (project, tag, day, week or month)"
        );
    }

    #[tokio::test]
    async fn a_report_refuses_text_that_is_not_a_range() {
        let seed = seeded().await;

        let err = seed
            .core
            .report(report_query(Some("soon")))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn a_report_by_tag_narrows_to_a_tag_by_its_name_start() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.group_by = Some("tag".into());
        query.tag = Some("dev".into());

        let (result, _) = seed.core.report(query).await.unwrap();

        let labels: Vec<&str> = result.groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["development"]);
        assert_eq!(result.total_ms, 150 * 60_000);
    }

    #[tokio::test]
    async fn a_report_narrows_to_a_client_and_to_a_project() {
        let seed = seeded().await;
        let mut by_client = report_query(Some(DAY));
        by_client.client = Some("acme".into());
        let mut by_project = report_query(Some(DAY));
        by_project.project = Some("Podcast".into());

        let (clients, _) = seed.core.report(by_client).await.unwrap();
        let (projects, _) = seed.core.report(by_project).await.unwrap();

        assert_eq!(clients.total_ms, 150 * 60_000);
        assert_eq!(projects.total_ms, 55 * 60_000);
    }

    #[tokio::test]
    async fn a_report_names_the_tag_it_cannot_find() {
        let seed = seeded().await;
        let mut query = report_query(Some(DAY));
        query.tag = Some("nope".into());

        let err = seed.core.report(query).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "No tag matches `nope`");
    }

    #[tokio::test]
    async fn recordings_come_by_start_with_local_iso_times_and_minutes() {
        let seed = seeded().await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        let notes: Vec<&str> = rows.iter().map(|r| r.note.as_str()).collect();
        assert_eq!(notes, ["outline", "fix login", "edit episode", "review"]);
        assert_eq!(
            serde_json::to_value(&rows[1]).unwrap(),
            json!({
                "id": seed.login.id,
                "day": DAY,
                "start_iso": iso(9, 0),
                "end_iso": iso(10, 30),
                "duration_min": 90.0,
                "project": "Portal",
                "client": "Acme",
                "note": "fix login",
                "tags": ["development"],
            })
        );
        assert_eq!(
            serde_json::to_value(&rows[0]).unwrap()["duration_min"],
            25.0
        );
    }

    #[tokio::test]
    async fn recordings_without_a_client_have_no_client_field() {
        let seed = seeded().await;

        let rows = seed
            .core
            .recordings(recordings_query(None, None, None))
            .await
            .unwrap();

        let outline = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(outline["project"], "Podcast");
        assert!(outline.get("client").is_none(), "{outline}");
    }

    #[tokio::test]
    async fn recordings_resolve_a_project_by_its_unique_prefix() {
        let seed = seeded().await;

        let projects = recorded_projects(&seed, recordings_query(Some("port"), None, None)).await;

        assert_eq!(projects, ["Portal", "Portal"]);
    }

    #[tokio::test]
    async fn recordings_refuse_an_ambiguous_prefix_and_name_both_projects() {
        let seed = seeded().await;

        let err = seed
            .core
            .recordings(recordings_query(Some("po"), None, None))
            .await
            .err()
            .unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        let message = err.to_string();
        assert!(
            message.starts_with("`po` matches several projects: "),
            "{message}"
        );
        for project in [&seed.portal, &seed.podcast] {
            let candidate = format!("{} ({})", project.name, project.id);
            assert!(message.contains(&candidate), "{candidate} in {message}");
        }
    }

    #[tokio::test]
    async fn recordings_filter_by_tag() {
        let seed = seeded().await;

        let projects =
            recorded_projects(&seed, recordings_query(None, Some("development"), None)).await;

        assert_eq!(projects, ["Portal", "Portal"]);
    }

    #[tokio::test]
    async fn recordings_keep_those_of_archived_projects() {
        let seed = seeded().await;
        projects::set_archived(&seed.db, &seed.podcast.id, true)
            .await
            .unwrap();

        let projects = recorded_projects(&seed, recordings_query(None, None, None)).await;

        assert_eq!(projects.len(), 4);
    }

    #[tokio::test]
    async fn the_recordings_limit_keeps_the_first_by_start_and_is_at_least_one() {
        let seed = seeded().await;
        for (limit, expected) in [
            (Some(2), vec!["outline", "fix login"]),
            (Some(0), vec!["outline"]),
            (
                Some(10),
                vec!["outline", "fix login", "edit episode", "review"],
            ),
            (None, vec!["outline", "fix login", "edit episode", "review"]),
        ] {
            let rows = seed
                .core
                .recordings(recordings_query(None, None, limit))
                .await
                .unwrap();

            let notes: Vec<&str> = rows.iter().map(|r| r.note.as_str()).collect();
            assert_eq!(notes, expected, "{limit:?}");
        }
    }

    #[tokio::test]
    async fn the_recordings_limit_is_capped_at_5000() {
        let seed = seeded().await;
        sqlx::query(
            r#"WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5010)
               INSERT INTO recordings (id, project_id, start, "end", note, created_at, updated_at)
               SELECT 'bulk-' || i, ?1, ?2 + i * 1000, ?2 + i * 1000 + 500, '', 0, 0 FROM n"#,
        )
        .bind(&seed.podcast.id)
        .bind(at(15, 0))
        .execute(seed.db.pool())
        .await
        .unwrap();

        let rows = seed
            .core
            .recordings(recordings_query(None, None, Some(1_000_000)))
            .await
            .unwrap();

        assert_eq!(rows.len(), 5000);
    }

    #[tokio::test]
    async fn recordings_refuse_a_day_that_does_not_exist() {
        let seed = seeded().await;
        let mut query = recordings_query(None, None, None);
        query.to = "2026-13-01".into();

        let err = seed.core.recordings(query).await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn projects_list_their_client_color_and_archived_flag() {
        let seed = seeded().await;
        projects::set_archived(&seed.db, &seed.podcast.id, true)
            .await
            .unwrap();

        let out = serde_json::to_value(seed.core.projects().await.unwrap()).unwrap();

        assert_eq!(out.as_array().unwrap().len(), 2);
        assert_eq!(
            named(&out, "name", "Portal"),
            &json!({
                "id": seed.portal.id,
                "name": "Portal",
                "color": seed.portal.color,
                "client": "Acme",
                "archived": false,
            })
        );
        assert_eq!(
            named(&out, "name", "Podcast"),
            &json!({
                "id": seed.podcast.id,
                "name": "Podcast",
                "color": seed.podcast.color,
                "archived": true,
            })
        );
    }

    #[tokio::test]
    async fn tags_list_every_tag_with_its_id() {
        let seed = seeded().await;

        let out = serde_json::to_value(seed.core.tags().await.unwrap()).unwrap();

        let mut names: Vec<&str> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        names.sort();
        let mut expected = db::tags::DEFAULT_TAGS.to_vec();
        expected.sort();
        assert_eq!(names, expected);
        assert_eq!(
            named(&out, "name", "development"),
            &json!({"id": seed.development.id, "name": "development"})
        );
    }

    #[tokio::test]
    async fn a_day_summary_totals_the_day_and_lists_notes_in_time_order() {
        let seed = seeded().await;

        let summary = seed.core.day_summary(DAY).await.unwrap();

        let out = serde_json::to_value(&summary).unwrap();
        assert_eq!(out["day"], DAY);
        assert_eq!(out["recorded_min"], 205.0);
        assert_eq!(out["tracked_min"], 90.0);
        assert_eq!(out["away_min"], 20.0);
        assert_eq!(
            named(&out["by_project"], "name", "Acme / Portal"),
            &json!({"name": "Acme / Portal", "minutes": 150.0, "notes": ["fix login", "review"]})
        );
        assert_eq!(
            named(&out["by_project"], "name", "Podcast"),
            &json!({"name": "Podcast", "minutes": 55.0, "notes": ["outline", "edit episode"]})
        );
        assert_eq!(
            named(&out["by_tag"], "name", "development"),
            &json!({"name": "development", "minutes": 150.0, "notes": ["fix login", "review"]})
        );
        assert_eq!(
            named(&out["by_tag"], "name", "No tag"),
            &json!({"name": "No tag", "minutes": 55.0, "notes": ["outline", "edit episode"]})
        );
    }

    #[tokio::test]
    async fn the_summary_of_an_empty_day_is_zero() {
        let seed = seeded().await;

        let out = serde_json::to_value(seed.core.day_summary("2026-10-19").await.unwrap()).unwrap();

        assert_eq!(
            out,
            json!({
                "day": "2026-10-19",
                "recorded_min": 0.0,
                "tracked_min": 0.0,
                "away_min": 0.0,
                "by_project": [],
                "by_tag": [],
            })
        );
    }

    #[tokio::test]
    async fn a_day_summary_refuses_a_day_that_does_not_exist() {
        let seed = seeded().await;

        let err = seed.core.day_summary("last tuesday").await.err().unwrap();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "Not a day: `last tuesday` (use YYYY-MM-DD, today or yesterday)"
        );
    }

    #[tokio::test]
    async fn activity_is_refused_until_the_user_exposes_it() {
        let seed = seeded().await;
        assert!(!seed.core.expose_activity());

        for settings in [None, Some(json!({"mcp": {"exposeActivity": false}}))] {
            if let Some(settings) = settings {
                write_settings(seed.dir.path(), settings);
            }

            let err = seed.core.activity(DAY).await.err().unwrap();

            assert_eq!(err.kind(), ErrorKind::PermissionRequired);
            assert_eq!(err.to_string(), ACTIVITY_HIDDEN);
        }
    }

    #[tokio::test]
    async fn exposed_activity_lists_spans_and_away_spans_with_private_rows_reduced_to_the_app() {
        let seed = seeded().await;
        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": true}}));

        let out = serde_json::to_value(seed.core.activity(DAY).await.unwrap()).unwrap();

        assert_eq!(
            out,
            json!({
                "activity": [
                    {
                        "start_iso": iso(9, 0),
                        "end_iso": iso(10, 0),
                        "app": "Xcode",
                        "title": "main.rs",
                        "path": "/src/main.rs",
                        "source": "tracker",
                    },
                    {
                        "start_iso": iso(10, 0),
                        "end_iso": iso(10, 30),
                        "app": "Safari",
                        "title": "",
                        "source": "tracker",
                    },
                    {
                        "start_iso": iso(11, 0),
                        "end_iso": iso(11, 30),
                        "app": "Planner",
                        "title": "Standup",
                        "url": "https://plan.example/standup",
                        "source": "external:planner",
                    },
                ],
                "away": [
                    {"start_iso": iso(12, 0), "end_iso": iso(12, 20), "reason": "idle"},
                ],
            })
        );
    }

    #[tokio::test]
    async fn the_settings_are_read_again_on_every_call() {
        let seed = seeded().await;
        assert!(seed.core.activity(DAY).await.is_err());

        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": true}}));
        assert!(seed.core.expose_activity());
        assert!(seed.core.activity(DAY).await.is_ok());

        write_settings(seed.dir.path(), json!({"mcp": {"exposeActivity": false}}));
        assert!(!seed.core.expose_activity());
        assert!(seed.core.activity(DAY).await.is_err());
    }

    #[tokio::test]
    async fn a_missing_database_file_says_there_is_no_data_yet() {
        let dir = temp_config_dir();
        let file = dir.path().join("data").join(DB_FILE);
        let core = Core::new(dir.path().to_path_buf());
        let expected = format!("TimeTracker has no data yet ({} not found)", file.display());

        for _ in 0..2 {
            let err = core.projects().await.err().unwrap();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            assert_eq!(err.to_string(), expected);
        }
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn the_database_is_found_once_the_app_has_created_it() {
        let dir = temp_config_dir();
        let core = Core::new(dir.path().to_path_buf());
        assert!(core.tags().await.is_err());

        Db::open(&dir.path().join("data").join(DB_FILE))
            .await
            .unwrap();

        assert!(!core.tags().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn without_a_settings_file_the_data_folder_is_in_the_config_dir() {
        let seed = seeded().await;
        assert!(!seed.dir.path().join("settings.json").exists());

        let names: Vec<String> = seed
            .core
            .projects()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();

        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn the_data_folder_setting_points_to_the_database() {
        let config = temp_config_dir();
        let elsewhere = temp_config_dir();
        let db = Db::open(&elsewhere.path().join(DB_FILE)).await.unwrap();
        project(&db, "Elsewhere").await;
        write_settings(
            config.path(),
            json!({"dataDir": elsewhere.path().display().to_string()}),
        );
        let core = Core::new(config.path().to_path_buf());

        let names: Vec<String> = core
            .projects()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();

        assert_eq!(names, ["Elsewhere"]);
    }
}
