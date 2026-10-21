use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::db::time::Day;
use crate::events::LAUNCH_EVENT;
use crate::ipc::IpcMessage;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::window;

/// Something the page acts on once it listens: from the launch that started the app, a later launch,
/// or a deep link.
#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LaunchEvent {
    Error {
        message: String,
    },
    /// Something the user should know that went fine, such as a data migration.
    Notice {
        message: String,
    },
    Activate,
    ShowDay {
        day: Day,
    },
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
            IpcMessage::Show { day, .. } => LaunchEvent::ShowDay {
                day: day.unwrap_or_else(Day::today),
            },
            _ => return false,
        };
        self.events.lock_unpoisoned().push(event);
        true
    }

    pub fn notice(&self, message: String) {
        self.events
            .lock_unpoisoned()
            .push(LaunchEvent::Notice { message });
    }

    pub fn take_events(&self) -> Vec<LaunchEvent> {
        std::mem::take(&mut *self.events.lock_unpoisoned())
    }
}

/// Also shows the window, except for a hidden activation.
pub fn queue(handle: &AppHandle, message: IpcMessage) {
    let show = !matches!(message, IpcMessage::Activate { hidden: true });
    if handle.state::<AppState>().launches.push(message)
        && let Err(e) = handle.emit(LAUNCH_EVENT, ())
    {
        log::warn!("Couldn't notify the page of a launch: {e}");
    }
    if show {
        window::show_main(handle);
    }
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

    #[test]
    fn show_with_a_day_becomes_a_show_day_event_whether_or_not_it_focuses() {
        let day = Day::parse("2026-10-21").unwrap();
        for focus in [true, false] {
            let queue = LaunchQueue::default();
            assert!(
                queue.push(IpcMessage::Show {
                    day: Some(day),
                    focus
                }),
                "focus {focus}"
            );
            assert_eq!(
                queue.take_events(),
                vec![LaunchEvent::ShowDay { day }],
                "focus {focus}"
            );
        }
    }

    #[test]
    fn show_without_a_day_becomes_a_show_day_event_for_today() {
        let queue = LaunchQueue::default();
        assert!(queue.push(IpcMessage::Show {
            day: None,
            focus: true
        }));
        let events = queue.take_events();
        assert!(
            matches!(events.as_slice(), [LaunchEvent::ShowDay { .. }]),
            "{events:?}"
        );
    }

    #[test]
    fn messages_for_the_cli_only_produce_no_event() {
        let messages = [
            IpcMessage::Hide,
            IpcMessage::Quit,
            IpcMessage::Status,
            IpcMessage::Resume,
            IpcMessage::Pause {
                until: crate::tracker::PauseUntil::Tomorrow,
            },
            IpcMessage::ActivityAdd { items: Vec::new() },
            IpcMessage::ActivityStop {
                source: "planner".into(),
                end: None,
            },
        ];
        for message in messages {
            let queue = LaunchQueue::default();
            assert!(!queue.push(message.clone()), "{message:?}");
            assert_eq!(queue.take_events(), vec![], "{message:?}");
        }
    }

    #[test]
    fn show_day_event_serializes_with_a_type_tag_and_the_day() {
        let day = Day::parse("2026-10-21").unwrap();
        assert_eq!(
            serde_json::to_value(LaunchEvent::ShowDay { day }).unwrap(),
            serde_json::json!({ "type": "showDay", "day": "2026-10-21" })
        );
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

    #[test]
    fn a_notice_is_queued_in_order_and_serializes_with_a_type_tag() {
        let queue = LaunchQueue::default();
        queue.push(IpcMessage::Activate { hidden: false });
        queue.notice("Moved".into());

        let events = queue.take_events();

        assert_eq!(
            events,
            vec![
                LaunchEvent::Activate,
                LaunchEvent::Notice {
                    message: "Moved".into()
                }
            ]
        );
        assert_eq!(
            serde_json::to_value(&events[1]).unwrap(),
            serde_json::json!({ "type": "notice", "message": "Moved" })
        );
    }
}
