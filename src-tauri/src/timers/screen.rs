//! Where the picker opens from the keyboard: at the top center of the screen the user works on, like
//! Spotlight. That is the screen showing the frontmost app's front window, else the one under the pointer.

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::NSWorkspace;
use objc2_core_foundation::CFArray;
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// How far down its screen the picker's top edge sits, as a share of the screen's height.
const TOP_SHARE: f64 = 0.2;

/// A screen's whole frame (to find it) and its visible frame (to place in), in Cocoa coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Screen {
    frame: NSRect,
    visible: NSRect,
}

/// Moves the window to the top center of the screen the user works on.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn place_on_focused_screen(ns_window: *mut c_void) {
    // SAFETY: the caller passes a live window.
    let window = unsafe { &*ns_window.cast::<AnyObject>() };
    let screens = screens();
    let Some(primary) = screens.first() else {
        return;
    };
    let point = front_window_center(primary.frame.size.height).unwrap_or_else(mouse_location);
    let Some(screen) = screen_at(&screens, point) else {
        return;
    };
    // SAFETY: plain NSWindow calls.
    unsafe {
        let frame: NSRect = msg_send![window, frame];
        let top_left = top_center(screen.visible, frame.size);
        let _: () = msg_send![window, setFrameTopLeftPoint: top_left];
    }
}

/// The screen containing `point`, else the first (the one with the menu bar).
fn screen_at(screens: &[Screen], point: NSPoint) -> Option<Screen> {
    let contains = |r: &NSRect| {
        point.x >= r.origin.x
            && point.x < r.origin.x + r.size.width
            && point.y >= r.origin.y
            && point.y < r.origin.y + r.size.height
    };
    screens
        .iter()
        .find(|s| contains(&s.frame))
        .or(screens.first())
        .copied()
}

/// The top-left corner that centers a window of `size` horizontally in `visible`, its top edge `TOP_SHARE`
/// down, kept inside `visible` (Cocoa coordinates: y grows upwards).
fn top_center(visible: NSRect, size: NSSize) -> NSPoint {
    let (left, bottom) = (visible.origin.x, visible.origin.y);
    let (right, top) = (left + visible.size.width, bottom + visible.size.height);
    let x = (left + (visible.size.width - size.width) / 2.0)
        .min(right - size.width)
        .max(left);
    let y = (top - visible.size.height * TOP_SHARE)
        .max(bottom + size.height)
        .min(top);
    NSPoint::new(x, y)
}

fn screens() -> Vec<Screen> {
    // SAFETY: `+[NSScreen screens]` returns an array of NSScreen; this runs on the main thread.
    unsafe {
        let Some(screens): Option<Retained<AnyObject>> = msg_send![class!(NSScreen), screens]
        else {
            return Vec::new();
        };
        let count: usize = msg_send![&screens, count];
        (0..count)
            .map(|i| {
                let screen: Retained<AnyObject> = msg_send![&screens, objectAtIndex: i];
                Screen {
                    frame: msg_send![&screen, frame],
                    visible: msg_send![&screen, visibleFrame],
                }
            })
            .collect()
    }
}

/// The center of the frontmost app's front window in Cocoa coordinates; `primary_height` is the menu bar
/// screen's, which the window server's top-down coordinates start from.
fn front_window_center(primary_height: f64) -> Option<NSPoint> {
    let pid = NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .processIdentifier();
    let list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    )?;
    let list: *const CFArray = &*list;
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to an NSArray of NSDictionary.
    let windows: &AnyObject = unsafe { &*list.cast() };
    let count: usize = unsafe { msg_send![windows, count] };
    (0..count).find_map(|i| {
        // SAFETY: an element of the window list.
        let info: Retained<AnyObject> = unsafe { msg_send![windows, objectAtIndex: i] };
        // Layer 0 holds normal windows; menus, panels and the menu bar sit above it.
        let owned = number(&info, "kCGWindowOwnerPID")? as i32 == pid;
        if !owned || number(&info, "kCGWindowLayer")? != 0.0 {
            return None;
        }
        window_center(&info, primary_height)
    })
}

/// The center of a window-list entry's bounds, flipped from top-down to Cocoa coordinates.
fn window_center(info: &AnyObject, primary_height: f64) -> Option<NSPoint> {
    let bounds = value(info, "kCGWindowBounds")?;
    let (x, y) = (number(&bounds, "X")?, number(&bounds, "Y")?);
    let (width, height) = (number(&bounds, "Width")?, number(&bounds, "Height")?);
    Some(NSPoint::new(
        x + width / 2.0,
        primary_height - (y + height / 2.0),
    ))
}

fn value(dictionary: &AnyObject, key: &str) -> Option<Retained<AnyObject>> {
    let key = NSString::from_str(key);
    // SAFETY: `dictionary` is an NSDictionary keyed by strings.
    unsafe { msg_send![dictionary, objectForKey: &*key] }
}

fn number(dictionary: &AnyObject, key: &str) -> Option<f64> {
    let value = value(dictionary, key)?;
    // SAFETY: the window list stores its numbers as NSNumber.
    Some(unsafe { msg_send![&value, doubleValue] })
}

fn mouse_location() -> NSPoint {
    // SAFETY: `+[NSEvent mouseLocation]` takes no arguments.
    unsafe { msg_send![class!(NSEvent), mouseLocation] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn screen(x: f64, y: f64, width: f64, height: f64, menu_bar: f64) -> Screen {
        Screen {
            frame: rect(x, y, width, height),
            visible: rect(x, y, width, height - menu_bar),
        }
    }

    fn primary() -> Screen {
        screen(0.0, 0.0, 1440.0, 900.0, 25.0)
    }

    #[test]
    fn a_point_on_the_primary_screen_picks_it() {
        let screens = [primary(), screen(1440.0, 0.0, 1920.0, 1080.0, 25.0)];
        assert_eq!(
            screen_at(&screens, NSPoint::new(700.0, 400.0)),
            Some(screens[0])
        );
    }

    #[test]
    fn a_point_on_a_second_screen_picks_that_screen() {
        let screens = [primary(), screen(1440.0, 0.0, 1920.0, 1080.0, 25.0)];
        assert_eq!(
            screen_at(&screens, NSPoint::new(2000.0, 500.0)),
            Some(screens[1])
        );
    }

    #[test]
    fn screens_left_of_and_below_the_primary_are_found_by_their_negative_origins() {
        let left = screen(-1920.0, 0.0, 1920.0, 1080.0, 25.0);
        let below = screen(0.0, -1080.0, 1920.0, 1080.0, 25.0);
        let screens = [primary(), left, below];

        assert_eq!(screen_at(&screens, NSPoint::new(-100.0, 300.0)), Some(left));
        assert_eq!(screen_at(&screens, NSPoint::new(500.0, -1.0)), Some(below));
        assert_eq!(screen_at(&screens, NSPoint::new(0.0, 0.0)), Some(primary()));
    }

    #[test]
    fn a_point_on_the_far_edge_belongs_to_the_next_screen() {
        let right = screen(1440.0, 0.0, 1920.0, 1080.0, 25.0);
        let screens = [primary(), right];
        assert_eq!(screen_at(&screens, NSPoint::new(1440.0, 10.0)), Some(right));
    }

    #[test]
    fn a_point_on_no_screen_falls_back_to_the_first() {
        let screens = [primary(), screen(1440.0, 0.0, 1920.0, 1080.0, 25.0)];
        assert_eq!(
            screen_at(&screens, NSPoint::new(-5000.0, 9000.0)),
            Some(screens[0])
        );
    }

    #[test]
    fn without_screens_there_is_no_screen() {
        assert_eq!(screen_at(&[], NSPoint::new(1.0, 1.0)), None);
    }

    #[test]
    fn the_top_edge_sits_a_fifth_of_the_height_below_the_top_and_the_window_is_centered() {
        let visible = rect(0.0, 0.0, 1440.0, 875.0);
        assert_eq!(
            top_center(visible, NSSize::new(600.0, 400.0)),
            NSPoint::new(420.0, 700.0)
        );
    }

    #[test]
    fn a_screen_with_a_nonzero_origin_offsets_the_placement() {
        let visible = rect(1440.0, 100.0, 1920.0, 1055.0);
        assert_eq!(
            top_center(visible, NSSize::new(600.0, 400.0)),
            NSPoint::new(2100.0, 944.0)
        );
    }

    #[test]
    fn a_window_too_tall_for_the_share_is_pushed_up_to_fit_inside_the_screen() {
        let visible = rect(0.0, 0.0, 1000.0, 500.0);
        assert_eq!(
            top_center(visible, NSSize::new(400.0, 450.0)),
            NSPoint::new(300.0, 450.0)
        );
    }

    #[test]
    fn a_window_larger_than_the_screen_keeps_its_top_left_corner_at_the_left_and_top() {
        let visible = rect(0.0, 0.0, 300.0, 200.0);
        assert_eq!(
            top_center(visible, NSSize::new(400.0, 300.0)),
            NSPoint::new(0.0, 200.0)
        );
    }
}
