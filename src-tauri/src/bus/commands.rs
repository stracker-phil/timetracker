//! The app's own commands (windows, Settings, quitting, folders) and the `page:` family; the timer and the
//! tracker register theirs in their modules.

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::{autostart, menu, window};

type Bus = CommandBus<AppHandle>;

/// `page:<id>` runs the page command `<id>` in the focused window.
pub const PAGE_PREFIX: &str = "page:";

pub fn register(bus: &mut Bus) {
    bus.add(
        Command::new("app:settings", "Settings…")
            .help("Open Settings")
            .run_now(|app: &AppHandle| window::settings::open(app, None)),
    );
    bus.add_family(
        Command::new("app:settings:", "Settings Page")
            .help("Open Settings at a page or section")
            .hidden()
            .family("<page>")
            .run(|app, page| async move {
                window::settings::open(&app, Some(&page));
                Ok(())
            }),
    );
    bus.add_family(
        Command::new(PAGE_PREFIX, "Window Action")
            .help("Run one of the window's own commands (the menus' items for what the page does)")
            .hidden()
            .family("<command>")
            .enabled_for(|app, id| menu::sync::page_item(app, id).enabled)
            .checked_for(|app, id| menu::sync::page_item(app, id).checked)
            .run(|app, id| async move {
                window::run_page_command(&app, &id);
                Ok(())
            }),
    );
    bus.add(
        Command::new("app:show", "Show TimeTracker")
            .help("Show the main window and bring it to the front")
            .run_now(window::show_main),
    );
    bus.add(
        Command::new("window:close", "Close Window")
            .help("Hide the window in front, the main window or Settings")
            .hidden()
            .run_now(window::close_focused),
    );
    bus.add(
        Command::new("app:background", "Hide to Menu Bar")
            .help("Hide the window and the Dock icon; tracking goes on")
            .run_now(window::hide_to_menu_bar),
    );
    bus.add(
        Command::new("app:quit", "Quit TimeTracker")
            .quits()
            .run_now(|app: &AppHandle| app.exit(0)),
    );
    bus.add(
        Command::new("app:launchAtLogin", "Launch at Login")
            .help("Start TimeTracker in the menu bar at login, or stop doing so")
            .checked_when(|app: &AppHandle| app.state::<AppState>().settings.get().launch_at_login)
            .run(|app| async move { toggle_launch_at_login(&app).await }),
    );
    bus.add(
        Command::new("app:logs", "Open Log Folder")
            .run(|app| async move { crate::commands::open_log_folder(app) }),
    );
    bus.add(
        Command::new("app:dataFolder", "Show Data Folder")
            .help("Show the database file in Finder")
            .run(|app| async move { reveal_data_folder(&app) }),
    );
    bus.add(
        Command::new("app:readme", "README")
            .help("Open the README in the default Markdown app")
            .run(|app| async move { crate::help::open_readme(app) }),
    );
}

async fn toggle_launch_at_login(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let on = !state.settings.get().launch_at_login;
    autostart::set(app, &state, on).await.map(drop)
}

fn reveal_data_folder(app: &AppHandle) -> AppResult<()> {
    let file = app.state::<AppState>().data.file();
    app.opener()
        .reveal_item_in_dir(file)
        .map_err(|e| AppError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Commands;

    #[test]
    fn every_app_command_is_registered() {
        let bus = Commands::registered();

        for id in [
            "app:settings",
            "app:show",
            "app:background",
            "app:quit",
            "app:launchAtLogin",
            "app:logs",
            "app:dataFolder",
            "app:readme",
            "window:close",
            "timer:start",
            "timer:startUnderTray",
            "timer:stop",
            "timer:toggle",
            "tracker:resume",
        ] {
            assert!(bus.knows(id), "{id}");
        }
        assert!(!bus.knows("app:nope"));
        assert!(!bus.knows("settings.toggle"));
    }

    #[test]
    fn settings_at_a_page_resolves_to_a_hidden_family_with_the_page_as_argument() {
        let bus = Commands::registered();

        let info = bus.info("app:settings:capture").unwrap();

        assert!(info.hidden);
        assert_eq!(info.title, "Settings Page");
        assert_eq!(bus.family("app:settings:").unwrap().arg, "<page>");
    }

    #[test]
    fn a_page_command_resolves_to_a_hidden_family_with_the_command_as_argument() {
        let bus = Commands::registered();

        let info = bus.info("page:view.day").unwrap();

        assert!(info.hidden);
        assert_eq!(info.title, "Window Action");
        assert_eq!(bus.family(PAGE_PREFIX).unwrap().arg, "<command>");
    }

    #[test]
    fn plain_settings_is_a_visible_command_of_its_own() {
        let bus = Commands::registered();

        let info = bus.info("app:settings").unwrap();

        assert!(!info.hidden);
        assert_eq!(info.title, "Settings…");
        assert_eq!(info.help, "Open Settings");
    }

    #[test]
    fn exactly_the_window_close_the_tray_picker_and_the_families_are_hidden() {
        let bus = Commands::registered();

        let mut hidden: Vec<&str> = bus
            .list()
            .filter(|listed| listed.info().hidden)
            .map(|listed| listed.id())
            .collect();
        hidden.sort_unstable();

        assert_eq!(
            hidden,
            [
                "app:settings:",
                "page:",
                "timer:startUnderTray",
                "window:close"
            ]
        );
    }

    #[test]
    fn only_quit_quits_the_app() {
        let bus = Commands::registered();

        let quitting: Vec<&str> = bus
            .list()
            .filter(|listed| listed.info().quits)
            .map(|listed| listed.id())
            .collect();

        assert_eq!(quitting, ["app:quit"]);
    }
}
