use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::events::LAUNCH_EVENT;
use crate::ipc::{HandlerResult, IpcMessage};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::window;

/// Something the page should act on after a launch: the one that started the app, or another one.
#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LaunchEvent {
    Error { message: String },
    Activate,
}

#[derive(Default)]
pub struct LaunchQueue {
    events: Mutex<Vec<LaunchEvent>>,
}

impl LaunchQueue {
    /// Returns true when the message produced an event for the page.
    pub fn push(&self, message: IpcMessage) -> bool {
        let event = match message {
            IpcMessage::Error { message } => LaunchEvent::Error { message },
            IpcMessage::Activate { hidden: false } => LaunchEvent::Activate,
            _ => return false,
        };
        self.events.lock_unpoisoned().push(event);
        true
    }

    pub fn take_events(&self) -> Vec<LaunchEvent> {
        std::mem::take(&mut *self.events.lock_unpoisoned())
    }
}

/// Handles a message from another TimeTracker process (activation, CLI error, dev eval).
pub async fn on_ipc_message(handle: AppHandle, message: IpcMessage) -> HandlerResult {
    #[cfg(debug_assertions)]
    if let IpcMessage::Eval { js } = &message {
        crate::dev::eval(&handle, js);
        return Ok(serde_json::Value::Null);
    }
    log::info!("Received {} from another process", message.kind());
    let show = !matches!(message, IpcMessage::Activate { hidden: true });
    if handle.state::<AppState>().launches.push(message)
        && let Err(e) = handle.emit(LAUNCH_EVENT, ())
    {
        log::warn!("Couldn't notify the page of a launch: {e}");
    }
    if show {
        window::show_main(&handle);
    }
    Ok(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_becomes_an_error_event() {
        let queue = LaunchQueue::default();
        assert!(queue.push(IpcMessage::Error {
            message: "boom".into()
        }));
        assert_eq!(
            queue.take_events(),
            vec![LaunchEvent::Error {
                message: "boom".into()
            }]
        );
    }

    #[test]
    fn visible_activation_becomes_an_activate_event() {
        let queue = LaunchQueue::default();
        assert!(queue.push(IpcMessage::Activate { hidden: false }));
        assert_eq!(queue.take_events(), vec![LaunchEvent::Activate]);
    }

    #[test]
    fn hidden_activation_produces_no_event() {
        let queue = LaunchQueue::default();
        assert!(!queue.push(IpcMessage::Activate { hidden: true }));
        assert_eq!(queue.take_events(), vec![]);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn eval_produces_no_event() {
        let queue = LaunchQueue::default();
        assert!(!queue.push(IpcMessage::Eval { js: "1".into() }));
        assert_eq!(queue.take_events(), vec![]);
    }

    #[test]
    fn events_are_taken_once_in_order() {
        let queue = LaunchQueue::default();
        queue.push(IpcMessage::Error {
            message: "first".into(),
        });
        queue.push(IpcMessage::Activate { hidden: false });
        assert_eq!(
            queue.take_events(),
            vec![
                LaunchEvent::Error {
                    message: "first".into()
                },
                LaunchEvent::Activate
            ]
        );
        assert_eq!(queue.take_events(), vec![]);
    }

    #[test]
    fn launch_events_serialize_with_a_type_tag() {
        assert_eq!(
            serde_json::to_value(LaunchEvent::Error {
                message: "boom".into()
            })
            .unwrap(),
            serde_json::json!({ "type": "error", "message": "boom" })
        );
        assert_eq!(
            serde_json::to_value(LaunchEvent::Activate).unwrap(),
            serde_json::json!({ "type": "activate" })
        );
    }
}
