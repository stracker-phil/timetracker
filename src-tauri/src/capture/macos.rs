//! The macOS source: the only module that talks to AppKit, Accessibility, CoreGraphics and AppleScript.

use std::collections::{HashMap, HashSet};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, msg_send};
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
    NSAppleEventDescriptor, NSAppleScript, NSAppleScriptErrorNumber, NSDictionary,
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSNotificationName,
    NSNumber, NSString,
};
use tauri::AppHandle;
use tokio::sync::broadcast;

use super::browsers::{self, NO_WINDOW, NOT_PERMITTED, Page};
use super::{PermissionStatus, Snapshot, Source, SystemEvent, private, url};
use crate::db::time::now_ms;
use crate::lock::MutexExt;
use crate::store::settings::CaptureSettings;

/// A hung app must not stall the poll.
const AX_TIMEOUT_SECONDS: f32 = 0.5;
/// `kCGAnyInputEventType`.
const ANY_INPUT: CGEventType = CGEventType(!0);
/// How long a poll waits for a browser's address; the script's own timeout is 1 s.
const SCRIPT_BUDGET: Duration = Duration::from_millis(1_500);
/// A failure without an AppleScript error number (the script did not run or did not answer).
const SCRIPT_FAILED: i64 = 0;
const FAILURE_LOG_INTERVAL: Duration = Duration::from_secs(3_600);

pub struct MacSource {
    app: AppHandle,
    events: broadcast::Sender<SystemEvent>,
    prompted: AtomicBool,
    /// The last pid whose focused window could not be read, so each app's failure is logged once.
    window_failed: Mutex<Option<i32>>,
    /// Browsers the user refused Automation for; not asked again this run.
    refused: Mutex<HashSet<String>>,
    /// When each browser's last script failure was logged.
    script_failures: Mutex<HashMap<String, Instant>>,
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
            app: app.clone(),
            events,
            prompted: AtomicBool::new(false),
            window_failed: Mutex::new(None),
            refused: Mutex::new(HashSet::new()),
            script_failures: Mutex::new(HashMap::new()),
        })
    }

    fn focused_window(&self, pid: i32) -> Option<CFRetained<AXUIElement>> {
        match ax_focused_window(pid) {
            Ok(window) => Some(window),
            Err(err) => {
                let mut failed = self.window_failed.lock_unpoisoned();
                if *failed != Some(pid) {
                    log::debug!("No focused window for pid {pid}: AXError {}", err.0);
                    *failed = Some(pid);
                }
                None
            }
        }
    }

    /// The page of a browser's front window; nothing for other apps, with addresses turned off, or for
    /// a window already known to be private.
    fn browser_page(
        &self,
        bundle_id: &str,
        private_title: bool,
        capture: &CaptureSettings,
    ) -> Page {
        if private_title
            || !capture.capture_urls
            || self.refused.lock_unpoisoned().contains(bundle_id)
        {
            return Page::default();
        }
        let Some(script) = browsers::script(bundle_id) else {
            return Page::default();
        };
        match self.run_script(script) {
            Ok(output) => browsers::parse(bundle_id, &output),
            Err(code) => {
                self.note_script_failure(bundle_id, code);
                browsers::failed(bundle_id, code)
            }
        }
    }

    /// NSAppleScript runs on the main thread; a browser that does not answer in time is skipped this poll.
    fn run_script(&self, source: String) -> Result<String, i64> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.app
            .run_on_main_thread(move || {
                let _ = tx.send(execute_script(&source));
            })
            .map_err(|_| SCRIPT_FAILED)?;
        rx.recv_timeout(SCRIPT_BUDGET).unwrap_or(Err(SCRIPT_FAILED))
    }

    fn note_script_failure(&self, bundle_id: &str, code: i64) {
        if code == NOT_PERMITTED {
            if self.refused.lock_unpoisoned().insert(bundle_id.to_string()) {
                log::info!(
                    "URL capture for {bundle_id} was refused; allow it in System Settings → Privacy & Security → Automation"
                );
            }
            return;
        }
        if code == NO_WINDOW {
            return;
        }
        let mut failures = self.script_failures.lock_unpoisoned();
        let due = failures
            .get(bundle_id)
            .is_none_or(|at| at.elapsed() >= FAILURE_LOG_INTERVAL);
        if due {
            log::debug!("Couldn't read the address of {bundle_id}: AppleScript error {code}");
            failures.insert(bundle_id.to_string(), Instant::now());
        }
    }
}

impl Source for MacSource {
    fn snapshot(&self, capture: &CaptureSettings) -> Option<Snapshot> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let pid = app.processIdentifier();
        let bundle_id = app.bundleIdentifier().map(|id| id.to_string());
        let name = app
            .localizedName()
            .map(|name| name.to_string())
            .or_else(|| bundle_id.clone())
            .unwrap_or_default();
        let window = self.focused_window(pid);
        let title = window
            .as_deref()
            .and_then(|w| string_attribute(w, "AXTitle"))
            .unwrap_or_default();
        let id = bundle_id.as_deref().unwrap_or_default();
        let private_title = private::is_private_title(id, &title);
        let page = self.browser_page(id, private_title, capture);
        let path = document_path(id, window.as_deref(), capture);
        Some(Snapshot {
            at: now_ms(),
            app: name,
            bundle_id,
            pid,
            title,
            url: page.url,
            path,
            private: private_title || page.private,
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

/// The file open in the front window of a document app.
fn document_path(
    bundle_id: &str,
    window: Option<&AXUIElement>,
    capture: &CaptureSettings,
) -> Option<String> {
    if !capture.capture_documents || browsers::browser(bundle_id).is_some() {
        return None;
    }
    string_attribute(window?, "AXDocument").and_then(|value| url::file_path(&value))
}

fn ax_focused_window(pid: i32) -> Result<CFRetained<AXUIElement>, AXError> {
    unsafe {
        let app = AXUIElement::new_application(pid);
        app.set_messaging_timeout(AX_TIMEOUT_SECONDS);
        attribute(&app, "AXFocusedWindow")?
            .downcast::<AXUIElement>()
            .map_err(|_| AXError::Failure)
    }
}

/// A string attribute of `element`; `None` when it is missing or not a string.
fn string_attribute(element: &AXUIElement, name: &str) -> Option<String> {
    let value = unsafe { attribute(element, name) }.ok()?;
    value.downcast_ref::<CFString>().map(ToString::to_string)
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

/// Runs on the main thread. Returns the script's text, or its AppleScript error number.
fn execute_script(source: &str) -> Result<String, i64> {
    let script = NSAppleScript::initWithSource(NSAppleScript::alloc(), &NSString::from_str(source))
        .ok_or(SCRIPT_FAILED)?;
    let mut error: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
    // The generated binding assumes a result, but a failed script returns nil.
    let result: Option<Retained<NSAppleEventDescriptor>> =
        unsafe { msg_send![&script, executeAndReturnError: Some(&mut error)] };
    match result {
        Some(descriptor) => Ok(descriptor
            .stringValue()
            .map(|text| text.to_string())
            .unwrap_or_default()),
        None => Err(error
            .as_deref()
            .and_then(error_number)
            .unwrap_or(SCRIPT_FAILED)),
    }
}

fn error_number(info: &NSDictionary<NSString, AnyObject>) -> Option<i64> {
    let number = info.objectForKey(unsafe { NSAppleScriptErrorNumber })?;
    number
        .downcast_ref::<NSNumber>()
        .map(|n| n.integerValue() as i64)
}
