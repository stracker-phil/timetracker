//! The picker as a non-activating panel, like Spotlight: it shows on every space, full-screen ones too, and
//! takes the keyboard without activating TimeTracker, so the app in front stays in front when it closes.

use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};

const NONACTIVATING_PANEL: usize = 1 << 7;
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
/// `NSStatusWindowLevel`: above full-screen windows and floating panels.
const STATUS_WINDOW_LEVEL: isize = 25;
/// The ivar tao's `TaoWindow` adds; the panel class repeats it so the swapped window keeps its layout.
const FOCUSABLE: &CStr = c"focusable";

extern "C-unwind" fn yes(_: &AnyObject, _: Sel) -> Bool {
    Bool::YES
}

extern "C-unwind" fn no(_: &AnyObject, _: Sel) -> Bool {
    Bool::NO
}

/// An `NSPanel` that takes the keyboard although it has no title bar.
fn panel_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"TimeTrackerPanel", class!(NSPanel))
            .expect("the panel class is declared once");
        builder.add_ivar::<Bool>(FOCUSABLE);
        // SAFETY: both selectors take no arguments and return BOOL.
        unsafe {
            builder.add_method(
                sel!(canBecomeKeyWindow),
                yes as extern "C-unwind" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(canBecomeMainWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
        }
        builder.register()
    })
}

/// Whether an object of `from` can become a `to` in place: same size and the same ivar where it was.
fn same_layout(from: &AnyClass, to: &AnyClass) -> bool {
    let offset = |class: &AnyClass| class.instance_variable(FOCUSABLE).map(|ivar| ivar.offset());
    let Some(focusable) = offset(from) else {
        return false;
    };
    from.instance_size() == to.instance_size() && offset(to) == Some(focusable)
}

/// Turns the window into the panel; false (and nothing changes) when its class has an unexpected layout.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn make_panel(ns_window: *mut c_void) -> bool {
    // SAFETY: the caller passes a live window.
    let window = unsafe { &*ns_window.cast::<AnyObject>() };
    let panel = panel_class();
    if window.class() != panel {
        if !same_layout(window.class(), panel) {
            log::warn!(
                "The timer picker stays a plain window: {:?} has an unexpected layout",
                window.class().name()
            );
            return false;
        }
        // SAFETY: checked above that the panel class matches the window's size and ivar.
        unsafe { AnyObject::set_class(window, panel) };
    }
    // SAFETY: plain NSWindow / NSPanel setters.
    unsafe {
        let mask: usize = msg_send![window, styleMask];
        let _: () = msg_send![window, setStyleMask: mask | NONACTIVATING_PANEL];
        let _: () = msg_send![
            window,
            setCollectionBehavior: CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | IGNORES_CYCLE
        ];
        let _: () = msg_send![window, setLevel: STATUS_WINDOW_LEVEL];
        let _: () = msg_send![window, setHidesOnDeactivate: false];
    }
    true
}

/// Brings the window to the front and gives it the keyboard without activating the app.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn show(ns_window: *mut c_void) {
    // SAFETY: the caller passes a live window.
    let window = unsafe { &*ns_window.cast::<AnyObject>() };
    // SAFETY: plain NSWindow calls.
    unsafe {
        let _: () = msg_send![window, orderFrontRegardless];
        let _: () = msg_send![window, makeKeyWindow];
    }
}
