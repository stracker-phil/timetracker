//! The menu bar item: the tracker's status line, Pause (or Resume), Open and Quit.

use std::sync::{Arc, Mutex};

use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::db::time::now_ms;
use crate::error::AppResult;
use crate::lock::MutexExt;
use crate::tracker::{PauseUntil, Tracker, TrackerState, TrackerStatus};
use crate::window;

const TRAY_ID: &str = "main";
const STATUS_ID: &str = "status";
const RESUME_ID: &str = "resume";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";
const PAUSES: [(&str, &str, PauseUntil); 4] = [
    ("pause-15", "For 15 Minutes", PauseUntil::Minutes(15)),
    ("pause-60", "For 1 Hour", PauseUntil::Minutes(60)),
    ("pause-tomorrow", "Until Tomorrow", PauseUntil::Tomorrow),
    ("pause-resumed", "Until Resumed", PauseUntil::Resumed),
];

/// Template images: macOS tints them for the menu bar's appearance.
const ICON: &[u8] = include_bytes!("../icons/tray/tray@2x.png");
const ICON_PAUSED: &[u8] = include_bytes!("../icons/tray/paused@2x.png");
const ICON_ATTENTION: &[u8] = include_bytes!("../icons/tray/attention@2x.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayIcon {
    Normal,
    Paused,
    Attention,
}

impl TrayIcon {
    fn of(state: &TrackerState) -> TrayIcon {
        match state {
            TrackerState::Paused { .. } => TrayIcon::Paused,
            TrackerState::NeedsPermission => TrayIcon::Attention,
            TrackerState::Running | TrackerState::Away { .. } => TrayIcon::Normal,
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            TrayIcon::Normal => ICON,
            TrayIcon::Paused => ICON_PAUSED,
            TrayIcon::Attention => ICON_ATTENTION,
        }
    }
}

/// The status item and the icon the menu was last built for, so `sync` only relabels when it can.
struct TrayMenu {
    status: MenuItem<Wry>,
    icon: TrayIcon,
}

struct TrayHandle(Mutex<TrayMenu>);

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let (menu, status) = build_menu(app, TrayIcon::Normal, "Tracking not started")?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip("TimeTracker")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu_event(app, event.id().as_ref()))
        .build(app)?;
    app.manage(TrayHandle(Mutex::new(TrayMenu {
        status,
        icon: TrayIcon::Normal,
    })));
    Ok(())
}

/// Follows a tracker status: relabels the status line, and rebuilds the menu and icon when the
/// tracker enters or leaves a pause or the permission problem.
pub fn sync(app: &AppHandle, status: &TrackerStatus) {
    if let Err(e) = try_sync(app, status) {
        log::warn!("Couldn't update the tray: {e}");
    }
}

fn try_sync(app: &AppHandle, status: &TrackerStatus) -> tauri::Result<()> {
    let (Some(handle), Some(tray)) = (app.try_state::<TrayHandle>(), app.tray_by_id(TRAY_ID))
    else {
        return Ok(());
    };
    let mut current = handle.0.lock_unpoisoned();
    let icon = TrayIcon::of(&status.state);
    if icon == current.icon {
        return current.status.set_text(&status.text);
    }
    let (menu, status_item) = build_menu(app, icon, &status.text)?;
    tray.set_menu(Some(menu))?;
    tray.set_icon(Some(Image::from_bytes(icon.bytes())?))?;
    tray.set_icon_as_template(true)?;
    *current = TrayMenu {
        status: status_item,
        icon,
    };
    Ok(())
}

fn build_menu(
    app: &AppHandle,
    icon: TrayIcon,
    text: &str,
) -> tauri::Result<(Menu<Wry>, MenuItem<Wry>)> {
    let status = MenuItem::with_id(app, STATUS_ID, text, false, None::<&str>)?;
    let pause_or_resume = pause_or_resume_item(app, icon)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            pause_or_resume.as_ref(),
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, OPEN_ID, "Open TimeTracker", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT_ID, "Quit TimeTracker", true, None::<&str>)?,
        ],
    )?;
    Ok((menu, status))
}

/// Resume while paused, else the Pause submenu.
fn pause_or_resume_item(
    app: &AppHandle,
    icon: TrayIcon,
) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    Ok(if icon == TrayIcon::Paused {
        Box::new(MenuItem::with_id(
            app,
            RESUME_ID,
            "Resume Tracking",
            true,
            None::<&str>,
        )?)
    } else {
        let items = PAUSES
            .iter()
            .map(|(id, label, _)| MenuItem::with_id(app, *id, *label, true, None::<&str>))
            .collect::<tauri::Result<Vec<_>>>()?;
        let refs: Vec<&dyn IsMenuItem<Wry>> =
            items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
        Box::new(Submenu::with_items(app, "Pause Tracking", true, &refs)?)
    })
}

fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        OPEN_ID => window::show_main(app),
        QUIT_ID => app.exit(0),
        RESUME_ID => resume(app),
        _ => {
            if let Some((_, _, until)) = PAUSES.iter().find(|(pause_id, ..)| *pause_id == id) {
                pause(app, *until);
            }
        }
    }
}

fn pause(app: &AppHandle, until: PauseUntil) {
    with_tracker(app, move |tracker| async move {
        tracker.pause(until, now_ms()).await
    });
}

fn resume(app: &AppHandle) {
    with_tracker(app, |tracker| async move { tracker.resume(now_ms()).await });
}

/// Runs `call` on the tracker in the background; without a tracker (no data store) nothing happens.
fn with_tracker<F, Fut>(app: &AppHandle, call: F)
where
    F: FnOnce(Arc<Tracker>) -> Fut + Send + 'static,
    Fut: Future<Output = AppResult<TrackerStatus>> + Send,
{
    let Some(tracker) = app.try_state::<Arc<Tracker>>().map(|t| t.inner().clone()) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        if let Err(e) = call(tracker).await {
            log::warn!("Couldn't change the pause from the tray: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::away::AwayReason;

    #[test]
    fn the_icon_follows_the_tracker_state() {
        let away = |reason| TrackerState::Away { since: 0, reason };
        for (state, expected) in [
            (TrackerState::Running, TrayIcon::Normal),
            (away(AwayReason::Idle), TrayIcon::Normal),
            (away(AwayReason::Sleep), TrayIcon::Normal),
            (away(AwayReason::Lock), TrayIcon::Normal),
            (away(AwayReason::Paused), TrayIcon::Normal),
            (TrackerState::Paused { until: None }, TrayIcon::Paused),
            (TrackerState::Paused { until: Some(1) }, TrayIcon::Paused),
            (TrackerState::NeedsPermission, TrayIcon::Attention),
        ] {
            assert_eq!(TrayIcon::of(&state), expected, "{state:?}");
        }
    }
}
