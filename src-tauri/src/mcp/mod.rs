//! `timetracker mcp`: an MCP server on stdio, built on `rmcp`. The read tools read the database itself, so
//! they work while the app is closed; the write tools (`writes.rs`) ask the running app. It never starts
//! Tauri; stdout carries only JSON-RPC.

mod data;
mod ui;
mod writes;

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
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
(optionally under a client; a project is billable or not, and its recordings with it). Days are local calendar days `YYYY-MM-DD` (or today, yesterday); times \
are local ISO 8601 with offset, durations are minutes. Writes go to suggestions the user reviews in the \
Day view (`suggestions` lists them, `approve` records them); pass `final` only when the user asked to change \
the timesheet itself. The timer tools act at once. `status`, `open_day`, the timer and every write need the \
app running.";

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
struct SuggestionsArgs {
    /// The first day: YYYY-MM-DD, today or yesterday
    from: String,
    /// The last day, inclusive (default `from`; at most 62 days)
    to: Option<String>,
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

#[tool_handler(router = Self::all_tools())]
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

    /// A handler serves one router; the write tools have theirs in `writes.rs`.
    fn all_tools() -> ToolRouter<Self> {
        Self::tool_router() + Self::write_router()
    }

    #[tool(
        description = "Hours and notes of a range grouped by project, tag, day, week or month, rounded as the user's report settings say; plain text ready to paste, or json (with the billable minutes and percent)"
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
        description = "The recordings of a range of days by start: project, client, note, tag (or null), local start and end, its spans of the day (local start and end each), minutes (their sum); a recording with spans on several days is listed once per day"
    )]
    async fn recordings(
        &self,
        Parameters(args): Parameters<RecordingsArgs>,
    ) -> Result<String, String> {
        answer(self.core.recordings(args.into()).await)
    }

    #[tool(
        description = "Every project with its client, color, whether it is billable and whether it is archived"
    )]
    async fn projects(&self) -> Result<String, String> {
        answer(self.core.projects().await)
    }

    #[tool(description = "Every client and whether it is archived")]
    async fn clients(&self) -> Result<String, String> {
        answer(self.core.clients().await)
    }

    #[tool(
        description = "The suggestions the Day view shows on a range of days, from rules or from other apps: id, kind (rule or stored), by, project, client, note, tag, spans, minutes"
    )]
    async fn suggestions(
        &self,
        Parameters(args): Parameters<SuggestionsArgs>,
    ) -> Result<String, String> {
        answer(self.core.suggestions(&args.from, args.to.as_deref()).await)
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

    const TOOLS: [&str; 18] = [
        "activity",
        "approve",
        "clients",
        "day_summary",
        "delete",
        "edit",
        "open_day",
        "projects",
        "record",
        "recordings",
        "report",
        "status",
        "suggestions",
        "tags",
        "timer_picker",
        "timer_start",
        "timer_stop",
        "timer_update",
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
    fn the_server_offers_exactly_the_eighteen_read_and_write_tools() {
        let mut names: Vec<String> = TimeTrackerServer::all_tools()
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
        socket: PathBuf,
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
        connected_as(config_dir, json!({"name": "test", "version": "0"})).await
    }

    async fn connected_as(config_dir: &Path, client_info: Value) -> (Client, Value) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (server, socket_dir) = server(config_dir);
        let socket = server.socket.clone();
        tokio::spawn(async move {
            let service = server.serve((server_read, server_write)).await.unwrap();
            let _ = service.waiting().await;
        });
        let (read, writer) = tokio::io::split(client_io);
        let mut client = Client {
            reader: BufReader::new(read),
            writer,
            socket,
            _socket_dir: socket_dir,
        };
        let init = client
            .call(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": client_info
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
                "billable": false,
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
                "billable_min": 0.0,
                "billable_percent": 0,
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

    mod write_tools {
        use super::*;
        use crate::db::recordings::Span;
        use crate::db::stored_suggestions::StoredSuggestion;
        use crate::error::ErrorKind;
        use crate::ipc::{HandlerResult, IpcError, IpcReply, IpcRequest};
        use crate::recordings::Overlap;
        use crate::recordings::cli::RecordRequest;
        use crate::scripting::writes::{EditRequest, Suggested, Written};
        use crate::timers::commands::{TimerStartRequest, TimerUpdateRequest};
        use chrono::{DateTime, Local, TimeZone};
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::net::UnixListener;
        use std::sync::mpsc;

        /// 2026-10-21 at `hour:minute` local time.
        fn local(hour: u32, minute: u32) -> i64 {
            Local
                .with_ymd_and_hms(2026, 10, 21, hour, minute, 0)
                .unwrap()
                .timestamp_millis()
        }

        fn utc(text: &str) -> i64 {
            DateTime::parse_from_rfc3339(text)
                .unwrap()
                .timestamp_millis()
        }

        /// Plays the running app: answers one request per entry of `replies`, in order, and hands over the
        /// messages it received.
        fn fake_app(socket: &Path, replies: Vec<HandlerResult>) -> mpsc::Receiver<IpcMessage> {
            let listener = UnixListener::bind(socket).unwrap();
            let (sent, received) = mpsc::channel();
            std::thread::spawn(move || {
                for outcome in replies {
                    let (stream, _) = listener.accept().unwrap();
                    let mut line = String::new();
                    std::io::BufReader::new(&stream)
                        .read_line(&mut line)
                        .unwrap();
                    let request: IpcRequest = serde_json::from_str(&line).unwrap();
                    sent.send(request.message).unwrap();
                    let reply = match outcome {
                        Ok(result) => IpcReply {
                            id: request.id,
                            ok: true,
                            result,
                            error: None,
                        },
                        Err(error) => IpcReply {
                            id: request.id,
                            ok: false,
                            result: Value::Null,
                            error: Some(error),
                        },
                    };
                    writeln!(&stream, "{}", serde_json::to_string(&reply).unwrap()).unwrap();
                }
            });
            received
        }

        struct Session {
            client: Client,
            app: mpsc::Receiver<IpcMessage>,
            _config: TempDir,
        }

        impl Session {
            async fn call(&mut self, tool: &str, arguments: Value) -> Value {
                self.client.tool(tool, arguments).await["result"].clone()
            }

            fn asked(&self) -> IpcMessage {
                self.app
                    .recv_timeout(Duration::from_secs(5))
                    .expect("the app was asked")
            }
        }

        fn client_info(name: &str) -> Value {
            json!({"name": name, "version": "0"})
        }

        async fn session(client_info: Value, replies: Vec<HandlerResult>) -> Session {
            let config = temp_config_dir();
            let (client, _) = connected_as(config.path(), client_info).await;
            let app = fake_app(&client.socket, replies);
            Session {
                client,
                app,
                _config: config,
            }
        }

        fn text(result: &Value) -> &str {
            result["content"][0]["text"].as_str().unwrap()
        }

        fn nothing() -> HandlerResult {
            Ok(Value::Null)
        }

        fn suggested_reply() -> HandlerResult {
            let written = Written::Suggestion(Suggested {
                suggestion: StoredSuggestion {
                    id: "sg1".into(),
                    project_id: "p1".into(),
                    note: "review".into(),
                    tag_id: None,
                    source: "test".into(),
                    day: Day::parse("2026-10-21").unwrap(),
                    spans: vec![Span {
                        start: local(9, 30),
                        end: local(10, 0),
                    }],
                    created_at: 0,
                    updated_at: 0,
                },
                project_name: "Portal".into(),
            });
            Ok(serde_json::to_value(written).unwrap())
        }

        fn record_args(extra: Value) -> Value {
            let mut args = json!({
                "project": "Portal",
                "spans": [{"start": "2026-10-21T09:30", "end": "2026-10-21T10:00"}],
                "note": "review",
                "tag": "development",
            });
            args.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            args
        }

        async fn recorded_request(client_info: Value, extra: Value) -> RecordRequest {
            let mut session = session(client_info, vec![suggested_reply()]).await;
            session.call("record", record_args(extra)).await;
            match session.asked() {
                IpcMessage::Record { request } => request,
                other => panic!("not a record: {other:?}"),
            }
        }

        #[tokio::test]
        async fn write_tools_say_that_timetracker_is_not_running_without_the_app() {
            let dir = temp_config_dir();
            let (mut client, _) = connected(dir.path()).await;

            for (tool, arguments) in [
                ("record", record_args(json!({}))),
                ("edit", json!({"id": "sg1", "note": "x"})),
                ("delete", json!({"id": "sg1"})),
                ("approve", json!({"ids": ["sg1"]})),
                ("timer_start", json!({"project": "Portal"})),
                ("timer_stop", json!({})),
                ("timer_picker", json!({})),
                ("timer_update", json!({"note": "x"})),
            ] {
                let called = client.tool(tool, arguments).await;

                assert_eq!(called["result"]["isError"], true, "{tool}");
                assert_eq!(text(&called["result"]), ui::NOT_RUNNING, "{tool}");
            }
        }

        #[tokio::test]
        async fn a_bad_span_time_is_refused_before_asking_the_app() {
            let mut session = session(client_info("test"), vec![nothing()]).await;
            let bad = json!([{"start": "09:00", "end": "25:99"}]);

            for (tool, arguments) in [
                ("record", record_args(json!({"spans": bad}))),
                ("edit", json!({"id": "sg1", "spans": bad})),
            ] {
                let called = session.call(tool, arguments).await;

                assert_eq!(called["isError"], true, "{tool}");
                assert_eq!(
                    text(&called),
                    "Not a time: `25:99` (use 2026-10-21T09:30, 09:30 or now)",
                    "{tool}"
                );
            }
            assert!(session.app.try_recv().is_err());
        }

        #[tokio::test]
        async fn record_suggests_by_default_and_answers_with_the_clis_line() {
            let mut session = session(client_info("Claude Desktop"), vec![suggested_reply()]).await;

            let called = session.call("record", record_args(json!({}))).await;

            assert_eq!(
                session.asked(),
                IpcMessage::Record {
                    request: RecordRequest {
                        project: "Portal".into(),
                        spans: vec![Span {
                            start: local(9, 30),
                            end: local(10, 0),
                        }],
                        note: "review".into(),
                        tag: Some("development".into()),
                        overlap: Overlap::Cancel,
                        final_write: false,
                        source: Some("Claude Desktop".into()),
                    }
                }
            );
            assert_ne!(called["isError"], true);
            assert_eq!(
                text(&called),
                "suggested 09:30–10:00 (0:30) for Portal · review; review it in the Day view (id sg1)"
            );
        }

        #[tokio::test]
        async fn record_reads_offsets_and_zulu_times() {
            let extra = json!({"spans": [
                {"start": "2026-10-21T09:30:00+02:00", "end": "2026-10-21T08:00:00Z"},
            ]});

            let request = recorded_request(client_info("test"), extra).await;

            assert_eq!(
                request.spans,
                [Span {
                    start: utc("2026-10-21T07:30:00Z"),
                    end: utc("2026-10-21T08:00:00Z"),
                }]
            );
        }

        #[tokio::test]
        async fn record_leaves_out_note_and_tag_as_empty_and_none() {
            let mut session = session(client_info("test"), vec![suggested_reply()]).await;
            let args = json!({
                "project": "Portal",
                "spans": [{"start": "2026-10-21T09:30", "end": "2026-10-21T10:00"}],
            });

            session.call("record", args).await;

            let IpcMessage::Record { request } = session.asked() else {
                panic!("not a record");
            };
            assert_eq!((request.note.as_str(), request.tag), ("", None));
        }

        #[tokio::test]
        async fn the_suggestions_source_is_the_clients_name_or_title() {
            let long = "x".repeat(50);
            let cases = [
                (client_info("claude-code"), "claude-code".to_string()),
                (
                    json!({"name": "cc", "title": "Claude Code", "version": "0"}),
                    "Claude Code".to_string(),
                ),
                (client_info("  padded  "), "padded".to_string()),
                (client_info("   "), "mcp".to_string()),
                (client_info("two\nlines"), "mcp".to_string()),
                (client_info(&long), "x".repeat(40)),
            ];

            for (info, expected) in cases {
                let request = recorded_request(info.clone(), json!({})).await;

                assert_eq!(request.source, Some(expected), "{info}");
            }
        }

        #[tokio::test]
        async fn final_records_unless_draft_and_replace_cuts_the_overlap() {
            let cases = [
                (json!({}), false, Overlap::Cancel),
                (json!({"final": true}), true, Overlap::Cancel),
                (
                    json!({"final": true, "draft": true}),
                    false,
                    Overlap::Cancel,
                ),
                (json!({"draft": true}), false, Overlap::Cancel),
                (
                    json!({"final": true, "replace": true}),
                    true,
                    Overlap::Replace,
                ),
                (json!({"replace": true}), false, Overlap::Replace),
            ];

            for (extra, final_write, overlap) in cases {
                let request = recorded_request(client_info("test"), extra.clone()).await;

                assert_eq!(
                    (request.final_write, request.overlap),
                    (final_write, overlap),
                    "{extra}"
                );
            }
        }

        #[tokio::test]
        async fn the_apps_refusal_comes_back_as_the_error_text() {
            let refusal = IpcError::new(ErrorKind::Overlap, "Overlaps the recording 09:00–10:00");
            let mut session = session(client_info("test"), vec![Err(refusal)]).await;

            let called = session
                .call("record", record_args(json!({"final": true})))
                .await;

            assert_eq!(called["isError"], true);
            assert_eq!(text(&called), "Overlaps the recording 09:00–10:00");
        }

        #[tokio::test]
        async fn edit_carries_the_id_the_changes_and_the_new_spans() {
            let mut session = session(client_info("test"), vec![suggested_reply()]).await;
            let arguments = json!({
                "id": "sg1",
                "project": "Podcast",
                "note": "edited",
                "clear_tag": true,
                "spans": [{"start": "2026-10-21T09:30", "end": "2026-10-21T10:00"}],
                "final": true,
                "replace": true,
            });

            session.call("edit", arguments).await;

            assert_eq!(
                session.asked(),
                IpcMessage::Edit {
                    request: EditRequest {
                        id: "sg1".into(),
                        project: Some("Podcast".into()),
                        note: Some("edited".into()),
                        tag: None,
                        clear_tag: true,
                        spans: Some(vec![Span {
                            start: local(9, 30),
                            end: local(10, 0),
                        }]),
                        final_write: true,
                        overlap: Overlap::Replace,
                    }
                }
            );
        }

        #[tokio::test]
        async fn edit_leaves_out_what_is_not_given() {
            let mut session = session(client_info("test"), vec![suggested_reply()]).await;

            let called = session
                .call("edit", json!({"id": "sg1", "tag": "focus"}))
                .await;

            assert_eq!(
                session.asked(),
                IpcMessage::Edit {
                    request: EditRequest {
                        id: "sg1".into(),
                        project: None,
                        note: None,
                        tag: Some("focus".into()),
                        clear_tag: false,
                        spans: None,
                        final_write: false,
                        overlap: Overlap::Cancel,
                    }
                }
            );
            assert_eq!(
                text(&called),
                "suggested 09:30–10:00 (0:30) for Portal · review; review it in the Day view (id sg1)"
            );
        }

        #[tokio::test]
        async fn delete_dismisses_unless_final_and_says_what_went() {
            let cases = [
                (json!({}), false),
                (json!({"final": true}), true),
                (json!({"final": true, "draft": true}), false),
            ];

            for (extra, final_write) in cases {
                let reply = Ok(json!({"kind": "suggestion", "id": "sg1"}));
                let mut session = session(client_info("test"), vec![reply]).await;
                let mut arguments = json!({"id": "sg1"});
                arguments
                    .as_object_mut()
                    .unwrap()
                    .extend(extra.as_object().unwrap().clone());

                let called = session.call("delete", arguments).await;

                assert_eq!(
                    session.asked(),
                    IpcMessage::Delete {
                        id: "sg1".into(),
                        final_write
                    },
                    "{extra}"
                );
                assert_eq!(text(&called), "deleted the suggestion sg1", "{extra}");
            }
        }

        #[tokio::test]
        async fn approve_sends_the_ids_and_says_when_nothing_was_approved() {
            let mut session = session(client_info("test"), vec![Ok(json!([]))]).await;

            let called = session.call("approve", json!({"ids": ["a", "b"]})).await;

            assert_eq!(
                session.asked(),
                IpcMessage::Approve {
                    ids: vec!["a".into(), "b".into()]
                }
            );
            assert_eq!(text(&called), "nothing approved");
        }

        #[tokio::test]
        async fn timer_start_sends_the_project_note_and_tag() {
            let mut session = session(client_info("test"), vec![nothing(), nothing()]).await;

            session
                .call(
                    "timer_start",
                    json!({"project": "Portal", "note": "fix", "tag": "focus"}),
                )
                .await;
            let with_all = session.asked();
            session
                .call("timer_start", json!({"project": "Portal"}))
                .await;
            let with_project = session.asked();

            assert_eq!(
                with_all,
                IpcMessage::TimerStart {
                    request: TimerStartRequest {
                        project: "Portal".into(),
                        note: "fix".into(),
                        tag: Some("focus".into()),
                    }
                }
            );
            assert_eq!(
                with_project,
                IpcMessage::TimerStart {
                    request: TimerStartRequest {
                        project: "Portal".into(),
                        note: String::new(),
                        tag: None,
                    }
                }
            );
        }

        #[tokio::test]
        async fn timer_stop_says_when_no_timer_ran() {
            let mut session = session(client_info("test"), vec![nothing()]).await;

            let called = session.call("timer_stop", json!({})).await;

            assert_eq!(session.asked(), IpcMessage::TimerStop);
            assert_eq!(text(&called), "no timer running");
        }

        #[tokio::test]
        async fn timer_picker_asks_the_app_to_open_the_picker() {
            let mut session = session(client_info("test"), vec![nothing()]).await;

            let called = session.call("timer_picker", json!({})).await;

            assert_eq!(session.asked(), IpcMessage::TimerPicker);
            assert_eq!(text(&called), "Opened the timer picker");
        }

        #[tokio::test]
        async fn timer_update_sends_only_what_changes() {
            let mut session = session(client_info("test"), vec![nothing()]).await;

            session
                .call(
                    "timer_update",
                    json!({"project": "Podcast", "note": "edit", "clear_tag": true}),
                )
                .await;

            assert_eq!(
                session.asked(),
                IpcMessage::TimerUpdate {
                    request: TimerUpdateRequest {
                        project: Some("Podcast".into()),
                        note: Some("edit".into()),
                        tag: None,
                        clear_tag: true,
                    }
                }
            );
        }
    }
}
