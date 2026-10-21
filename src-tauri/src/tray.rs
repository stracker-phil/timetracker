//! The menu bar item: the tracker's status line, the timer (its time and project as the item's title),
//! Pause (or Resume), Open and Quit.

use std::sync::{Arc, Mutex};

use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::db::time::now_ms;
use crate::error::AppResult;
use crate::lock::MutexExt;
use crate::recordings::timer::TimerInput;
use crate::timers::{self, RecentTimer, RunningTimer, picker};
use crate::tracker::{PauseUntil, Tracker, TrackerState, TrackerStatus};
use crate::window;

const TRAY_ID: &str = "main";
const STATUS_ID: &str = "status";
const RESUME_ID: &str = "resume";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";
const TIMER_START_ID: &str = "timer.start";
const TIMER_STOP_ID: &str = "timer.stop";
const TIMER_RECENT_PREFIX: &str = "timer.recent.";
const TITLE_PROJECT_CHARS: usize = 24;
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

/// What the timer part of the menu shows.
#[derive(Debug, Clone, Default)]
struct TimerMenu {
    /// `Project, 1:23` while a timer runs.
    running: Option<String>,
    recent: Vec<RecentTimer>,
}

/// The status item and what the menu was last built for, so `sync` only relabels when it can.
struct TrayMenu {
    status: MenuItem<Wry>,
    icon: TrayIcon,
    timer: TimerMenu,
}

struct TrayHandle(Mutex<TrayMenu>);

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let text = "Tracking not started";
    let (menu, status) = build_menu(app, TrayIcon::Normal, text, &TimerMenu::default())?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip("TimeTracker")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu_event(app, event.id().as_ref()))
        // The timer picker opens under the icon, so the positioner needs to know where it is.
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event)
        })
        .build(app)?;
    app.manage(TrayHandle(Mutex::new(TrayMenu {
        status,
        icon: TrayIcon::Normal,
        timer: TimerMenu::default(),
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
    let (menu, status_item) = build_menu(app, icon, &status.text, &current.timer)?;
    tray.set_menu(Some(menu))?;
    tray.set_icon(Some(Image::from_bytes(icon.bytes())?))?;
    tray.set_icon_as_template(true)?;
    current.status = status_item;
    current.icon = icon;
    Ok(())
}

/// Shows the running timer in the menu bar title and the menu, and offers the recent timers.
pub fn set_timer(app: &AppHandle, running: Option<&RunningTimer>, recent: Vec<RecentTimer>) {
    if let Err(e) = try_set_timer(app, running, recent) {
        log::warn!("Couldn't show the timer in the tray: {e}");
    }
}

fn try_set_timer(
    app: &AppHandle,
    running: Option<&RunningTimer>,
    recent: Vec<RecentTimer>,
) -> tauri::Result<()> {
    let (Some(handle), Some(tray)) = (app.try_state::<TrayHandle>(), app.tray_by_id(TRAY_ID))
    else {
        return Ok(());
    };
    let mut current = handle.0.lock_unpoisoned();
    current.timer = TimerMenu {
        running: running.map(|t| format!("{}, {}", t.project_name, clock(t.elapsed_ms))),
        recent,
    };
    let text = current.status.text()?;
    let (menu, status_item) = build_menu(app, current.icon, &text, &current.timer)?;
    tray.set_menu(Some(menu))?;
    tray.set_title(running.map(title).as_deref())?;
    current.status = status_item;
    Ok(())
}

/// `⏱ 1:23 Project`, the project cut short.
fn title(timer: &RunningTimer) -> String {
    let mut project: String = timer
        .project_name
        .chars()
        .take(TITLE_PROJECT_CHARS)
        .collect();
    if timer.project_name.chars().count() > TITLE_PROJECT_CHARS {
        project.push('…');
    }
    format!("⏱ {} {project}", clock(timer.elapsed_ms))
}

fn clock(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

fn build_menu(
    app: &AppHandle,
    icon: TrayIcon,
    text: &str,
    timer: &TimerMenu,
) -> tauri::Result<(Menu<Wry>, MenuItem<Wry>)> {
    let status = MenuItem::with_id(app, STATUS_ID, text, false, None::<&str>)?;
    let pause_or_resume = pause_or_resume_item(app, icon)?;
    let stop_label = match &timer.running {
        Some(running) => format!("Stop Timer ({running})"),
        None => "Stop Timer".into(),
    };
    let recent = recent_items(app, &timer.recent)?;
    let recent_refs: Vec<&dyn IsMenuItem<Wry>> =
        recent.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, TIMER_START_ID, "Start Timer…", true, None::<&str>)?,
            &MenuItem::with_id(
                app,
                TIMER_STOP_ID,
                stop_label,
                timer.running.is_some(),
                None::<&str>,
            )?,
            &Submenu::with_items(app, "Recent Timers", !recent.is_empty(), &recent_refs)?,
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

/// `Project · note` items; their ids carry the index into `TimerMenu.recent`.
fn recent_items(app: &AppHandle, recent: &[RecentTimer]) -> tauri::Result<Vec<MenuItem<Wry>>> {
    recent
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let label = if t.note.is_empty() {
                t.project_name.clone()
            } else {
                format!("{} · {}", t.project_name, t.note)
            };
            MenuItem::with_id(
                app,
                format!("{TIMER_RECENT_PREFIX}{i}"),
                label,
                true,
                None::<&str>,
            )
        })
        .collect()
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
        TIMER_START_ID => picker::open(app),
        TIMER_STOP_ID => stop_timer(app),
        _ if id.starts_with(TIMER_RECENT_PREFIX) => start_recent(app, id),
        _ => {
            if let Some((_, _, until)) = PAUSES.iter().find(|(pause_id, ..)| *pause_id == id) {
                pause(app, *until);
            }
        }
    }
}

fn stop_timer(app: &AppHandle) {
    let app = app.clone();
    in_background("stop", async move {
        timers::commands::stop(&app).await.map(drop)
    });
}

fn start_recent(app: &AppHandle, id: &str) {
    let Some(handle) = app.try_state::<TrayHandle>() else {
        return;
    };
    let index = id[TIMER_RECENT_PREFIX.len()..].parse::<usize>().ok();
    let Some(recent) = index.and_then(|i| handle.0.lock_unpoisoned().timer.recent.get(i).cloned())
    else {
        return;
    };
    let input = TimerInput::from(recent);
    let app = app.clone();
    in_background("start", async move {
        timers::commands::start(&app, input).await.map(drop)
    });
}

/// Runs a timer change from a menu click; a failure is logged.
fn in_background(what: &'static str, change: impl Future<Output = AppResult<()>> + Send + 'static) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = change.await {
            log::warn!("Couldn't {what} the timer from the tray: {e}");
        }
    });
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

    fn running(project_name: &str, elapsed_ms: i64) -> RunningTimer {
        RunningTimer {
            recording: crate::db::recordings::Recording {
                id: "r1".into(),
                project_id: "p1".into(),
                start: 0,
                end: None,
                note: String::new(),
                tag_ids: Vec::new(),
                created_at: 0,
                updated_at: 0,
            },
            project_name: project_name.into(),
            color: "blue".into(),
            shade: 0,
            elapsed_ms,
        }
    }

    #[test]
    fn the_title_shows_the_elapsed_time_and_the_project() {
        for (elapsed_ms, expected) in [
            (0, "⏱ 0:00 Acme"),
            (59_999, "⏱ 0:00 Acme"),
            (60_000, "⏱ 0:01 Acme"),
            (83 * 60_000, "⏱ 1:23 Acme"),
            (26 * 3_600_000, "⏱ 26:00 Acme"),
            (-5_000, "⏱ 0:00 Acme"),
        ] {
            assert_eq!(
                title(&running("Acme", elapsed_ms)),
                expected,
                "{elapsed_ms}"
            );
        }
    }

    #[test]
    fn a_long_project_name_is_cut_to_24_characters_with_an_ellipsis() {
        let exact = "a".repeat(24);
        let long = "b".repeat(25);

        assert_eq!(title(&running(&exact, 60_000)), format!("⏱ 0:01 {exact}"));
        assert_eq!(
            title(&running(&long, 60_000)),
            format!("⏱ 0:01 {}…", "b".repeat(24))
        );
        assert_eq!(
            title(&running(&"é".repeat(30), 60_000)),
            format!("⏱ 0:01 {}…", "é".repeat(24))
        );
    }
}
