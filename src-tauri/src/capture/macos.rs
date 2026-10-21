//! The macOS source: the only module that talks to AppKit, Accessibility and CoreGraphics.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidWakeNotification,
    NSWorkspaceSessionDidBecomeActiveNotification, NSWorkspaceSessionDidResignActiveNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement,
    kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGEventType};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSNotificationName,
    NSString,
};
use tauri::AppHandle;
use tokio::sync::broadcast;

use super::{PermissionStatus, Snapshot, Source, SystemEvent};
use crate::db::time::now_ms;
use crate::lock::MutexExt;

/// A hung app must not stall the poll.
const AX_TIMEOUT_SECONDS: f32 = 0.5;
/// `kCGAnyInputEventType`.
const ANY_INPUT: CGEventType = CGEventType(!0);

pub struct MacSource {
    events: broadcast::Sender<SystemEvent>,
    prompted: AtomicBool,
    /// The last pid whose title could not be read, so each app's failure is logged once.
    title_failed: Mutex<Option<i32>>,
}

impl MacSource {
    pub fn new(app: &AppHandle) -> Arc<MacSource> {
        let (events, _) = broadcast::channel(32);
        let sender = events.clone();
        // Notification blocks are delivered on the main thread's run loop, which Tauri runs.
        if let Err(e) = app.run_on_main_thread(move || observe(&sender)) {
            log::error!("Cannot observe app switches, sleep and lock: {e}");
        }
        Arc::new(MacSource {
            events,
            prompted: AtomicBool::new(false),
            title_failed: Mutex::new(None),
        })
    }

    fn window_title(&self, pid: i32) -> String {
        match focused_window_title(pid) {
            Ok(title) => title,
            Err(err) => {
                let mut failed = self.title_failed.lock_unpoisoned();
                if *failed != Some(pid) {
                    log::debug!("No window title for pid {pid}: AXError {}", err.0);
                    *failed = Some(pid);
                }
                String::new()
            }
        }
    }
}

impl Source for MacSource {
    fn snapshot(&self) -> Option<Snapshot> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let pid = app.processIdentifier();
        let bundle_id = app.bundleIdentifier().map(|id| id.to_string());
        let name = app
            .localizedName()
            .map(|name| name.to_string())
            .or_else(|| bundle_id.clone())
            .unwrap_or_default();
        Some(Snapshot {
            at: now_ms(),
            app: name,
            bundle_id,
            pid,
            title: self.window_title(pid),
            url: None,
            path: None,
            private: false,
        })
    }

    fn idle_seconds(&self) -> f64 {
        CGEventSource::seconds_since_last_event_type(
            CGEventSourceStateID::CombinedSessionState,
            ANY_INPUT,
        )
    }

    fn events(&self) -> broadcast::Receiver<SystemEvent> {
        self.events.subscribe()
    }

    fn permission(&self) -> PermissionStatus {
        if unsafe { AXIsProcessTrusted() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    fn request_permission(&self) {
        if self.prompted.swap(true, Ordering::Relaxed) {
            return;
        }
        let options = CFDictionary::<CFString, CFBoolean>::from_slices(
            &[unsafe { kAXTrustedCheckOptionPrompt }],
            &[CFBoolean::new(true)],
        );
        unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
    }
}

fn observe(events: &broadcast::Sender<SystemEvent>) {
    let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
    let workspace_events = unsafe {
        [
            (
                NSWorkspaceDidActivateApplicationNotification,
                SystemEvent::Activated,
            ),
            (NSWorkspaceWillSleepNotification, SystemEvent::Sleep),
            (NSWorkspaceDidWakeNotification, SystemEvent::Wake),
            (
                NSWorkspaceSessionDidResignActiveNotification,
                SystemEvent::SessionInactive,
            ),
            (
                NSWorkspaceSessionDidBecomeActiveNotification,
                SystemEvent::SessionActive,
            ),
        ]
    };
    for (name, event) in workspace_events {
        add_observer(&workspace, name, event, events.clone());
    }
    let distributed = NSDistributedNotificationCenter::defaultCenter();
    for (name, event) in [
        ("com.apple.screenIsLocked", SystemEvent::Lock),
        ("com.apple.screenIsUnlocked", SystemEvent::Unlock),
    ] {
        add_observer(
            &distributed,
            &NSString::from_str(name),
            event,
            events.clone(),
        );
    }
}

fn add_observer(
    center: &NSNotificationCenter,
    name: &NSNotificationName,
    event: SystemEvent,
    events: broadcast::Sender<SystemEvent>,
) {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        let _ = events.send(event);
    });
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    // Observers stay registered for the app's lifetime.
    std::mem::forget(observer);
}

fn focused_window_title(pid: i32) -> Result<String, AXError> {
    unsafe {
        let app = AXUIElement::new_application(pid);
        app.set_messaging_timeout(AX_TIMEOUT_SECONDS);
        let window = attribute(&app, "AXFocusedWindow")?
            .downcast::<AXUIElement>()
            .map_err(|_| AXError::Failure)?;
        let title = attribute(&window, "AXTitle")?;
        Ok(title
            .downcast_ref::<CFString>()
            .map(ToString::to_string)
            .unwrap_or_default())
    }
}

/// # Safety
/// `element` must be a valid accessibility element.
unsafe fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let mut value: *const CFType = std::ptr::null();
    let err = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if err != AXError::Success {
        return Err(err);
    }
    NonNull::new(value.cast_mut())
        .map(|value| unsafe { CFRetained::from_raw(value) })
        .ok_or(AXError::NoValue)
}
