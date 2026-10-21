//! `timetracker mcp`: a read-only MCP server on stdio, built on `rmcp`. It reads the database itself, so
//! it works while the app is closed, and never starts Tauri; stdout carries only JSON-RPC.

mod data;
mod ui;

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::schemars::JsonSchema;
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::ipc::IpcMessage;
use crate::remote::StatusReport;
use data::{Core, RecordingsQuery, ReportQuery};

const INSTRUCTIONS: &str = "Read-only access to TimeTracker, a local time tracker. Three layers: \
activity (what windows were in front; private, available only when the user enabled it), recordings \
(time the user assigned to a project, with a one-line note and at most one tag; what reports show) and projects \
(optionally under a client). Days are local calendar days `YYYY-MM-DD` (or today, yesterday); times \
are local ISO 8601 with offset, durations are minutes. Nothing here writes. `status` and `open_day` \
need the app running.";

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct ReportArgs {
    /// today (default), yesterday, week, last-week, month, last-month or a day; not with from/to
    range: Option<String>,
    /// The first day, with `to`
    from: Option<String>,
    /// The last day (inclusive), with `from`
    to: Option<String>,
    /// project (default), tag, day, week or month
    group_by: Option<String>,
    /// Only this client: its name, a unique start of it, or its id
    client: Option<String>,
    /// Only this project: its name, a unique start of it, or its id
    project: Option<String>,
    /// Only this tag: its name, a unique start of it, or its id
    tag: Option<String>,
    /// text (default: the text the user pastes into a timesheet) or json
    format: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct RecordingsArgs {
    /// The first day
    from: String,
    /// The last day, inclusive
    to: String,
    /// Only this project: its name, a unique start of it, or its id
    project: Option<String>,
    /// Only this tag: its name, a unique start of it, or its id
    tag: Option<String>,
    /// At most this many recordings (default 500, at most 5000)
    limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct DayArgs {
    /// YYYY-MM-DD, today or yesterday
    day: String,
}

#[derive(Clone)]
pub struct TimeTrackerServer {
    core: Arc<Core>,
    socket: PathBuf,
}

impl From<ReportArgs> for ReportQuery {
    fn from(args: ReportArgs) -> Self {
        Self {
            range: args.range,
            from: args.from,
            to: args.to,
            group_by: args.group_by,
            client: args.client,
            project: args.project,
            tag: args.tag,
        }
    }
}

impl From<RecordingsArgs> for RecordingsQuery {
    fn from(args: RecordingsArgs) -> Self {
        Self {
            from: args.from,
            to: args.to,
            project: args.project,
            tag: args.tag,
            limit: args.limit,
        }
    }
}

enum ReportFormat {
    Text,
    Json,
}

impl ReportFormat {
    fn of(format: Option<&str>) -> Result<Self, String> {
        match format {
            None | Some("text") => Ok(Self::Text),
            Some("json") => Ok(Self::Json),
            Some(other) => Err(format!("Not a format: `{other}` (text or json)")),
        }
    }
}

/// A tool's error as the text the client shows.
fn reason<T>(result: Result<T, impl Display>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

fn json(value: &impl Serialize) -> Result<String, String> {
    reason(serde_json::to_string_pretty(value))
}

fn answer<T: Serialize>(result: AppResult<T>) -> Result<String, String> {
    reason(result).and_then(|value| json(&value))
}

#[tool_handler]
impl ServerHandler for TimeTrackerServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "timetracker",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

#[tool_router]
impl TimeTrackerServer {
    pub fn new(core: Arc<Core>, socket: PathBuf) -> Self {
        Self { core, socket }
    }

    #[tool(
        description = "Hours and notes of a range grouped by project, tag, day, week or month, rounded as the user's report settings say; plain text ready to paste, or json"
    )]
    async fn report(&self, Parameters(args): Parameters<ReportArgs>) -> Result<String, String> {
        let format = ReportFormat::of(args.format.as_deref())?;
        let (result, text) = reason(self.core.report(args.into()).await)?;
        match format {
            ReportFormat::Text => Ok(text),
            ReportFormat::Json => json(&data::report_out(&result)),
        }
    }

    #[tool(
        description = "The recordings of a range of days by start: project, client, note, tag (or null), local start and end, minutes; a recording across midnight is listed once per day"
    )]
    async fn recordings(
        &self,
        Parameters(args): Parameters<RecordingsArgs>,
    ) -> Result<String, String> {
        answer(self.core.recordings(args.into()).await)
    }

    #[tool(description = "Every project with its client, color and whether it is archived")]
    async fn projects(&self) -> Result<String, String> {
        answer(self.core.projects().await)
    }

    #[tool(description = "Every tag")]
    async fn tags(&self) -> Result<String, String> {
        answer(self.core.tags().await)
    }

    #[tool(
        description = "A day's recorded, tracked and away minutes, and its recorded minutes and notes by project and by tag"
    )]
    async fn day_summary(&self, Parameters(args): Parameters<DayArgs>) -> Result<String, String> {
        answer(self.core.day_summary(&args.day).await)
    }

    #[tool(
        description = "A day's captured activity (app, window title, URL or document, source) and away spans; only when the user exposed activity in Settings → Data"
    )]
    async fn activity(&self, Parameters(args): Parameters<DayArgs>) -> Result<String, String> {
        answer(self.core.activity(&args.day).await)
    }

    #[tool(
        description = "The running app's tracker state, current app and running timer with its elapsed minutes"
    )]
    async fn status(&self) -> Result<String, String> {
        let value = ui::ask(&self.socket, IpcMessage::Status).await?;
        let report: StatusReport = reason(serde_json::from_value(value))?;
        json(&data::status_out(report, self.core.expose_activity()))
    }

    #[tool(description = "Opens a day's Day view in the running app and brings its window up")]
    async fn open_day(&self, Parameters(args): Parameters<DayArgs>) -> Result<String, String> {
        let day = reason(data::day_of(&args.day))?;
        let message = IpcMessage::Show {
            day: Some(day),
            focus: true,
        };
        ui::ask(&self.socket, message).await?;
        Ok(format!("Opened {day} in TimeTracker"))
    }
}

/// Serves MCP on stdin/stdout until the client disconnects; the process exit code.
pub fn run(config_dir: &Path, socket: PathBuf) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Cannot start the MCP server: {e}");
            return 2;
        }
    };
    let core = Arc::new(Core::new(config_dir.to_path_buf()));
    runtime.block_on(serve(TimeTrackerServer::new(core, socket)))
}

async fn serve(server: TimeTrackerServer) -> i32 {
    let service = match server.serve(rmcp::transport::stdio()).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("MCP handshake failed: {e}");
            return 1;
        }
    };
    match service.waiting().await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("MCP server stopped: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::time::Day;
    use crate::test_support::{project, recording, temp_config_dir, temp_db};
    use serde_json::{Value, json};
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    const TOOLS: [&str; 8] = [
        "activity",
        "day_summary",
        "open_day",
        "projects",
        "recordings",
        "report",
        "status",
        "tags",
    ];

    /// A socket path nothing listens on; macOS limits Unix socket paths to about 104 bytes.
    fn unused_socket() -> (TempDir, PathBuf) {
        let dir = tempfile::Builder::new()
            .prefix("tt")
            .tempdir_in("/tmp")
            .unwrap();
        let socket = dir.path().join("s.sock");
        (dir, socket)
    }

    fn server(config_dir: &Path) -> (TimeTrackerServer, TempDir) {
        let (socket_dir, socket) = unused_socket();
        let core = Core::new(config_dir.to_path_buf());
        (TimeTrackerServer::new(Arc::new(core), socket), socket_dir)
    }

    fn report_args(format: Option<&str>) -> ReportArgs {
        ReportArgs {
            range: Some("today".into()),
            from: None,
            to: None,
            group_by: None,
            client: None,
            project: None,
            tag: None,
            format: format.map(String::from),
        }
    }

    #[test]
    fn the_server_offers_exactly_the_eight_tools() {
        let mut names: Vec<String> = TimeTrackerServer::tool_router()
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();

        assert_eq!(names, TOOLS);
    }

    struct Client {
        reader: BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
        _socket_dir: TempDir,
    }

    impl Client {
        async fn send(&mut self, message: Value) {
            let line = format!("{message}\n");
            self.writer.write_all(line.as_bytes()).await.unwrap();
        }

        async fn reply(&mut self) -> Value {
            let mut line = String::new();
            tokio::time::timeout(Duration::from_secs(5), self.reader.read_line(&mut line))
                .await
                .expect("the server answers")
                .unwrap();
            serde_json::from_str(&line).unwrap()
        }

        async fn call(&mut self, id: u32, method: &str, params: Value) -> Value {
            self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
                .await;
            self.reply().await
        }

        async fn tool(&mut self, name: &str, arguments: Value) -> Value {
            self.call(
                2,
                "tools/call",
                json!({"name": name, "arguments": arguments}),
            )
            .await
        }
    }

    async fn connected(config_dir: &Path) -> (Client, Value) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (server, socket_dir) = server(config_dir);
        tokio::spawn(async move {
            let service = server.serve((server_read, server_write)).await.unwrap();
            let _ = service.waiting().await;
        });
        let (read, writer) = tokio::io::split(client_io);
        let mut client = Client {
            reader: BufReader::new(read),
            writer,
            _socket_dir: socket_dir,
        };
        let init = client
            .call(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "test", "version": "0"}
                }),
            )
            .await;
        client
            .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        (client, init)
    }

    #[tokio::test]
    async fn the_handshake_names_the_server_and_carries_the_instructions() {
        let dir = temp_config_dir();

        let (_client, init) = connected(dir.path()).await;

        assert_eq!(init["result"]["serverInfo"]["name"], "timetracker");
        assert_eq!(init["result"]["instructions"], INSTRUCTIONS);
    }

    #[tokio::test]
    async fn a_client_lists_the_tools_over_json_rpc() {
        let dir = temp_config_dir();
        let (mut client, _) = connected(dir.path()).await;

        let listed = client.call(2, "tools/list", json!({})).await;

        let mut names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, TOOLS);
    }

    #[tokio::test]
    async fn a_client_calls_the_projects_tool_over_json_rpc() {
        let (dir, db) = temp_db().await;
        let portal = project(&db, "Portal").await;
        let (mut client, _) = connected(dir.path()).await;

        let called = client.tool("projects", json!({})).await;

        let text = called["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(text).unwrap(),
            json!([{
                "id": portal.id,
                "name": "Portal",
                "color": "blue",
                "archived": false,
            }])
        );
    }

    #[tokio::test]
    async fn a_refused_tool_call_is_an_error_result_with_the_reason() {
        let (dir, _db) = temp_db().await;
        let (mut client, _) = connected(dir.path()).await;

        let called = client.tool("activity", json!({"day": "today"})).await;

        assert_eq!(called["result"]["isError"], true);
        assert_eq!(
            called["result"]["content"][0]["text"],
            data::ACTIVITY_HIDDEN
        );
    }

    #[tokio::test]
    async fn the_report_tool_answers_text_by_default_and_minutes_as_json() {
        let (dir, db) = temp_db().await;
        let portal = project(&db, "Portal").await;
        let today = Day::today();
        let start = today.bounds().0 + 3_600_000;
        recording(&db, &portal.id, start, Some(start + 3_600_000)).await;
        let (server, _socket) = server(dir.path());

        let text = server.report(Parameters(report_args(None))).await.unwrap();
        let explicit = server
            .report(Parameters(report_args(Some("text"))))
            .await
            .unwrap();
        let json_text = server
            .report(Parameters(report_args(Some("json"))))
            .await
            .unwrap();

        let expected = crate::reports::run(
            &db,
            &crate::reports::ReportFilter::default(),
            today,
            crate::db::time::now_ms(),
        )
        .await
        .unwrap();
        assert_eq!(text, crate::reports::text(&expected));
        assert_eq!(explicit, text);
        assert_eq!(
            serde_json::from_str::<Value>(&json_text).unwrap(),
            json!({
                "from": today.to_string(),
                "to": today.to_string(),
                "group_by": "project",
                "rounding": "none",
                "total_min": 60.0,
                "total_rounded_min": 60.0,
                "groups": [{
                    "label": "Portal",
                    "minutes": 60.0,
                    "rounded_min": 60.0,
                    "notes": [],
                }],
            })
        );
    }

    #[tokio::test]
    async fn the_report_tool_refuses_an_unknown_format() {
        let dir = temp_config_dir();
        let (server, _socket) = server(dir.path());

        let result = server.report(Parameters(report_args(Some("xml")))).await;

        assert_eq!(
            result,
            Err("Not a format: `xml` (text or json)".to_string())
        );
    }

    #[tokio::test]
    async fn data_tools_say_there_is_no_data_before_the_app_ever_ran() {
        let dir = temp_config_dir();
        let (server, _socket) = server(dir.path());
        let file = dir.path().join("data").join(crate::db::DB_FILE);

        let result = server.projects().await;

        assert_eq!(
            result,
            Err(format!(
                "TimeTracker has no data yet ({} not found)",
                file.display()
            ))
        );
    }

    #[tokio::test]
    async fn ui_tools_say_that_timetracker_is_not_running_without_the_app() {
        let dir = temp_config_dir();
        let (server, _socket) = server(dir.path());

        assert_eq!(server.status().await, Err(ui::NOT_RUNNING.to_string()));
        assert_eq!(
            server
                .open_day(Parameters(DayArgs {
                    day: "2026-10-20".into()
                }))
                .await,
            Err(ui::NOT_RUNNING.to_string())
        );
    }

    #[tokio::test]
    async fn open_day_refuses_a_day_that_does_not_exist_before_asking_the_app() {
        let dir = temp_config_dir();
        let (server, _socket) = server(dir.path());

        let result = server
            .open_day(Parameters(DayArgs { day: "soon".into() }))
            .await;

        assert_eq!(
            result,
            Err("Not a day: `soon` (use YYYY-MM-DD, today or yesterday)".to_string())
        );
    }
}
