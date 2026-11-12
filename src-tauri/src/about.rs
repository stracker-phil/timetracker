//! Settings → About and Status: the facts about this build and where it keeps its files.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::error::AppResult;
use crate::state::AppState;

/// Where `scripts/install.sh` links the `timetracker` command, under the home folder.
const INSTALLED_CLI: &str = ".local/bin/timetracker";

/// What the tracker records: the real frontmost windows, or the debug build's made-up ones (ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum CaptureKind {
    Real,
    Demo,
}

/// Whether the `timetracker` command on the PATH reaches this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum CliLink {
    Installed,
    NotInstalled,
    /// Debug builds are never linked; their CLI is called by path.
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CommandLine {
    pub link: CliLink,
    /// The link's place, or this build's binary for a debug build.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct AppStatus {
    pub name: String,
    pub version: String,
    pub debug: bool,
    pub capture: CaptureKind,
    /// Settings, keybindings and the socket.
    pub settings_folder: String,
    /// The database file, wherever Settings → Data moved it.
    pub database: String,
    pub log_folder: String,
    pub command_line: CommandLine,
}

fn capture_kind(debug: bool, real_capture: bool) -> CaptureKind {
    if debug && !real_capture {
        CaptureKind::Demo
    } else {
        CaptureKind::Real
    }
}

fn command_line(debug: bool, home: Option<&Path>, exe: &Path) -> CommandLine {
    if debug {
        return CommandLine {
            link: CliLink::NotApplicable,
            path: exe.display().to_string(),
        };
    }
    let link = home.map(|home| home.join(INSTALLED_CLI));
    let installed = link.as_deref().is_some_and(|link| link.exists());
    CommandLine {
        link: if installed {
            CliLink::Installed
        } else {
            CliLink::NotInstalled
        },
        path: link.map_or_else(
            || INSTALLED_CLI.to_string(),
            |link| link.display().to_string(),
        ),
    }
}

#[cfg(debug_assertions)]
fn real_capture() -> bool {
    crate::dev::real_capture()
}

#[cfg(not(debug_assertions))]
fn real_capture() -> bool {
    true
}

fn display(path: Option<PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

#[tauri::command]
pub fn app_status(app: AppHandle, state: State<'_, AppState>) -> AppResult<AppStatus> {
    let debug = cfg!(debug_assertions);
    let exe = std::env::current_exe().unwrap_or_default();
    Ok(AppStatus {
        name: crate::APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        debug,
        capture: capture_kind(debug, real_capture()),
        settings_folder: state.config_dir.display().to_string(),
        database: state.data.file().display().to_string(),
        log_folder: display(app.path().app_log_dir().ok()),
        command_line: command_line(debug, std::env::home_dir().as_deref(), &exe),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_with_link() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let link = home.path().join(INSTALLED_CLI);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::fs::write(&link, "").unwrap();
        home
    }

    #[test]
    fn debug_builds_capture_demo_windows_unless_real_capture_is_on() {
        for (debug, real, expected) in [
            (true, false, CaptureKind::Demo),
            (true, true, CaptureKind::Real),
            (false, false, CaptureKind::Real),
            (false, true, CaptureKind::Real),
        ] {
            assert_eq!(
                capture_kind(debug, real),
                expected,
                "debug={debug} real={real}"
            );
        }
    }

    #[test]
    fn debug_builds_report_their_own_binary_as_not_applicable() {
        let home = home_with_link();
        let exe = Path::new("/example/target/debug/timetracker");
        assert_eq!(
            command_line(true, Some(home.path()), exe),
            CommandLine {
                link: CliLink::NotApplicable,
                path: "/example/target/debug/timetracker".to_string(),
            }
        );
    }

    #[test]
    fn release_build_with_the_link_in_place_is_installed() {
        let home = home_with_link();
        let expected = home.path().join(INSTALLED_CLI).display().to_string();
        assert_eq!(
            command_line(false, Some(home.path()), Path::new("/example/timetracker")),
            CommandLine {
                link: CliLink::Installed,
                path: expected
            }
        );
    }

    #[test]
    fn release_build_without_the_link_is_not_installed_and_names_where_it_goes() {
        let home = tempfile::tempdir().unwrap();
        let expected = home.path().join(INSTALLED_CLI).display().to_string();
        assert_eq!(
            command_line(false, Some(home.path()), Path::new("/example/timetracker")),
            CommandLine {
                link: CliLink::NotInstalled,
                path: expected
            }
        );
    }

    #[test]
    fn release_build_without_a_home_folder_is_not_installed() {
        assert_eq!(
            command_line(false, None, Path::new("/example/timetracker")),
            CommandLine {
                link: CliLink::NotInstalled,
                path: INSTALLED_CLI.to_string(),
            }
        );
    }
}
