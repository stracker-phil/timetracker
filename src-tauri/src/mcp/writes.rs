//! The tools that change data. They ask the running app, as the CLI does, since the server's database is
//! read-only (ADR 0017). Time goes in as a suggestion the user reviews in the Day view unless `final` says
//! to record it (ADR 0028); the timer is live control and always direct.

use chrono::Local;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Implementation;
use rmcp::schemars::JsonSchema;
use rmcp::service::RoleServer;
use rmcp::{Peer, tool, tool_router};
use serde::Deserialize;

use super::{TimeTrackerServer, reason, ui};
use crate::cli::{reply, time};
use crate::db::recordings::Span;
use crate::db::time::now_ms;
use crate::ipc::IpcMessage;
use crate::recordings::Overlap;
use crate::recordings::cli::RecordRequest;
use crate::scripting::writes::EditRequest;
use crate::timers::commands::{TimerStartRequest, TimerUpdateRequest};

/// The source of suggestions from a client that does not name itself.
const MCP_SOURCE: &str = "mcp";

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct SpanArg {
    /// 09:30 (today), 2026-10-21T09:30 (local) or with an offset; now
    start: String,
    /// As `start`
    end: String,
}

#[derive(Deserialize, JsonSchema, Default)]
#[schemars(crate = "rmcp::schemars")]
struct FinalArgs {
    /// Change the timesheet itself instead of leaving a suggestion for the user to review
    #[serde(default, rename = "final")]
    final_write: bool,
    /// Keep it a suggestion even with `final`
    #[serde(default)]
    draft: bool,
    /// With `final`: cut recordings in the way instead of refusing the overlap
    #[serde(default)]
    replace: bool,
}

impl FinalArgs {
    fn is_final(&self) -> bool {
        self.final_write && !self.draft
    }

    fn overlap(&self) -> Overlap {
        if self.replace {
            Overlap::Replace
        } else {
            Overlap::Cancel
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct RecordArgs {
    /// The project: its name, a unique start of it, or its id. Leave it out when unsure: the suggestion
    /// waits for the user to pick one. Required with `final`
    project: Option<String>,
    /// At least one; all on one day for a suggestion
    spans: Vec<SpanArg>,
    /// One line
    note: Option<String>,
    /// A tag name
    tag: Option<String>,
    #[serde(flatten)]
    final_args: FinalArgs,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct EditArgs {
    /// A suggestion's id (from `suggestions`) or a recording's (from `recordings`)
    id: String,
    /// A new project: its name, a unique start of it, or its id
    project: Option<String>,
    /// A new note
    note: Option<String>,
    /// A new tag name
    tag: Option<String>,
    /// Remove the tag
    #[serde(default)]
    clear_tag: bool,
    /// New spans, replacing all of them
    spans: Option<Vec<SpanArg>>,
    /// On a suggestion, `final` records it with the changes; on a recording it is required
    #[serde(flatten)]
    final_args: FinalArgs,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct DeleteArgs {
    /// A suggestion's id (dismissed) or a recording's (needs `final`)
    id: String,
    #[serde(flatten)]
    final_args: FinalArgs,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct ApproveArgs {
    /// Suggestion ids from `suggestions`; each becomes a recording
    ids: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct TimerStartArgs {
    /// The project: its name, a unique start of it, or its id
    project: String,
    /// One line
    note: Option<String>,
    /// A tag name
    tag: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct TimerUpdateArgs {
    /// A new project: its name, a unique start of it, or its id
    project: Option<String>,
    /// A new note
    note: Option<String>,
    /// A new tag name
    tag: Option<String>,
    /// Remove the tag
    #[serde(default)]
    clear_tag: bool,
}

fn spans(args: &[SpanArg]) -> Result<Vec<Span>, String> {
    let now = now_ms();
    args.iter()
        .map(|span| {
            Ok(Span {
                start: time::instant(&span.start, now, &Local)?,
                end: time::instant(&span.end, now, &Local)?,
            })
        })
        .collect()
}

/// The client's name from the handshake as the suggestions' "Suggested by".
fn source_of(peer: &Peer<RoleServer>) -> String {
    peer.peer_info()
        .map(|info| {
            let client: &Implementation = &info.client_info;
            client.title.clone().unwrap_or_else(|| client.name.clone())
        })
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
        .map(|name| name.chars().take(40).collect())
        .unwrap_or_else(|| MCP_SOURCE.to_string())
}

#[tool_router(router = write_router, vis = "pub(super)")]
impl TimeTrackerServer {
    /// Asks the app and answers with the line the CLI prints.
    async fn told(&self, message: IpcMessage) -> Result<String, String> {
        let value = ui::ask(&self.socket, message.clone()).await?;
        Ok(reason(reply::render(&message, &value, now_ms(), &Local))?
            .unwrap_or_else(|| "done".into()))
    }

    #[tool(
        description = "Suggests time, for a project or for the user to assign one, which the user reviews in the Day view; with final it records it instead. Answers with the suggestion's id"
    )]
    async fn record(
        &self,
        peer: Peer<RoleServer>,
        Parameters(args): Parameters<RecordArgs>,
    ) -> Result<String, String> {
        let request = RecordRequest {
            project: args.project,
            spans: spans(&args.spans)?,
            note: args.note.unwrap_or_default(),
            tag: args.tag,
            overlap: args.final_args.overlap(),
            final_write: args.final_args.is_final(),
            source: Some(source_of(&peer)),
        };
        self.told(IpcMessage::Record { request }).await
    }

    #[tool(
        description = "Changes a suggestion; with final records it with the changes. Changing a recording needs final"
    )]
    async fn edit(&self, Parameters(args): Parameters<EditArgs>) -> Result<String, String> {
        let request = EditRequest {
            id: args.id,
            project: args.project,
            note: args.note,
            tag: args.tag,
            clear_tag: args.clear_tag,
            spans: args.spans.as_deref().map(spans).transpose()?,
            final_write: args.final_args.is_final(),
            overlap: args.final_args.overlap(),
        };
        self.told(IpcMessage::Edit { request }).await
    }

    #[tool(description = "Dismisses a suggestion; deleting a recording needs final")]
    async fn delete(&self, Parameters(args): Parameters<DeleteArgs>) -> Result<String, String> {
        let message = IpcMessage::Delete {
            id: args.id,
            final_write: args.final_args.is_final(),
        };
        self.told(message).await
    }

    #[tool(description = "Records suggestions as they are, one recording each")]
    async fn approve(&self, Parameters(args): Parameters<ApproveArgs>) -> Result<String, String> {
        self.told(IpcMessage::Approve { ids: args.ids }).await
    }

    #[tool(
        description = "Starts the timer for a project now; a running timer stops. Today's recording with the same project, note and tag continues"
    )]
    async fn timer_start(
        &self,
        Parameters(args): Parameters<TimerStartArgs>,
    ) -> Result<String, String> {
        let request = TimerStartRequest {
            project: args.project,
            note: args.note.unwrap_or_default(),
            tag: args.tag,
        };
        self.told(IpcMessage::TimerStart { request }).await
    }

    #[tool(
        description = "Opens the timer picker on the user's screen, so the user can start, change or stop the timer; the main window stays hidden"
    )]
    async fn timer_picker(&self) -> Result<String, String> {
        ui::ask(&self.socket, IpcMessage::TimerPicker).await?;
        Ok("Opened the timer picker".into())
    }

    #[tool(description = "Stops the running timer")]
    async fn timer_stop(&self) -> Result<String, String> {
        self.told(IpcMessage::TimerStop).await
    }

    #[tool(
        description = "Changes the running timer's project, note or tag; what is left out stays"
    )]
    async fn timer_update(
        &self,
        Parameters(args): Parameters<TimerUpdateArgs>,
    ) -> Result<String, String> {
        let request = TimerUpdateRequest {
            project: args.project,
            note: args.note,
            tag: args.tag,
            clear_tag: args.clear_tag,
        };
        self.told(IpcMessage::TimerUpdate { request }).await
    }
}
