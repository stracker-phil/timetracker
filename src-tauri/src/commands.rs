use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::autostart;
use crate::db::time::Day;
use crate::error::{AppError, AppResult};
use crate::events::{self, DataKind};
use crate::launch::LaunchEvent;
use crate::menu;
use crate::state::AppState;
use crate::store::keybindings::Keybindings;
use crate::store::settings::{AppSettingPatch, LogLevel, Settings};
use crate::store::ui_state::UiState;
use crate::timers;
use crate::tracker::Tracker;
use crate::window;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// What a settings change has to apply outside the store.
#[derive(Debug, PartialEq)]
pub struct SettingsEffects {
    pub log_level: Option<log::LevelFilter>,
    pub launch_at_login: Option<bool>,
}

/// Without `previous` (at startup) everything applies. A `--log-level` from the CLI wins over
/// the stored level for the app's lifetime.
pub fn settings_effects(
    previous: Option<&Settings>,
    saved: &Settings,
    log_override: Option<LogLevel>,
) -> SettingsEffects {
    let log_level = match log_override {
        Some(cli_level) => previous.is_none().then_some(cli_level),
        None => changed(previous, saved, |s| s.log_level),
    };
    SettingsEffects {
        log_level: log_level.map(Into::into),
        launch_at_login: changed(previous, saved, |s| s.launch_at_login),
    }
}

/// The saved value of a field when it differs from the previous one, or always at startup.
fn changed<T: PartialEq>(
    previous: Option<&Settings>,
    saved: &Settings,
    field: impl Fn(&Settings) -> T,
) -> Option<T> {
    previous
        .is_none_or(|p| field(p) != field(saved))
        .then(|| field(saved))
}

pub fn apply_settings(app: &AppHandle, previous: Option<&Settings>, saved: &Settings) {
    let effects = settings_effects(previous, saved, app.state::<AppState>().log_override);
    if let Some(level) = effects.log_level {
        log::set_max_level(level);
    }
    // At startup this reconciles a LaunchAgent that went missing (a moved or reinstalled bundle).
    if let Some(enabled) = effects.launch_at_login
        && let Err(e) = autostart::apply(app, enabled)
    {
        log::warn!("{e}");
    }
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> AppResult<Settings> {
    // The page's copy may predate a data folder move, which owns `dataDir`.
    settings.data_dir = state.settings.get().data_dir;
    // A shortcut the system refuses is reported on its row and not saved.
    if settings.timer_shortcut != state.settings.get().timer_shortcut {
        timers::shortcut::register(&app, &settings.timer_shortcut)?;
    }
    save_settings(&app, &state, settings).await
}

#[tauri::command]
pub async fn app_settings_set(
    app: AppHandle,
    state: State<'_, AppState>,
    bundle_id: String,
    patch: AppSettingPatch,
) -> AppResult<Settings> {
    let (previous, saved) = state.settings.update_app(bundle_id, patch).await?;
    settings_saved(&app, &previous, &saved);
    Ok(saved)
}

pub(crate) async fn save_settings(
    app: &AppHandle,
    state: &AppState,
    settings: Settings,
) -> AppResult<Settings> {
    let (previous, saved) = state.settings.update(settings).await?;
    settings_saved(app, &previous, &saved);
    Ok(saved)
}

fn settings_saved(app: &AppHandle, previous: &Settings, saved: &Settings) {
    apply_settings(app, Some(previous), saved);
    // The timeline settings and the title formats shape every day's view and the calendar, so they reload.
    if previous.timeline != saved.timeline
        || previous.apps != saved.apps
        || previous.day_title != saved.day_title
        || previous.calendar_title != saved.calendar_title
    {
        events::data_changed(app, DataKind::Settings, None);
    }
    if let Some(tracker) = app.try_state::<std::sync::Arc<Tracker>>() {
        tracker.set_settings(saved.tracker.clone());
        tracker.set_capture(saved.capture.clone());
    }
    log::info!("Settings updated");
}

#[tauri::command]
pub fn get_keybindings(state: State<'_, AppState>) -> Keybindings {
    state.keybindings.get()
}

#[tauri::command]
pub async fn set_keybindings(
    app: AppHandle,
    state: State<'_, AppState>,
    bindings: Keybindings,
) -> AppResult<Keybindings> {
    let saved = state.keybindings.update(bindings).await?;
    menu::rebuild(&app);
    Ok(saved)
}

#[tauri::command]
pub fn get_ui_state(state: State<'_, AppState>) -> UiState {
    state.ui_state.get().restorable(Day::today())
}

#[tauri::command]
pub async fn set_ui_state(state: State<'_, AppState>, ui: UiState) -> AppResult<UiState> {
    state.ui_state.set(ui).await
}

/// The welcome sheet was finished; it shows again only after "Show welcome again".
#[tauri::command]
pub async fn onboarding_done(state: State<'_, AppState>) -> AppResult<()> {
    state.ui_state.finish_onboarding().await
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle) -> AppResult<()> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}

/// http(s) only, and without credentials, which the browser would send on.
pub fn is_web_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https") && u.username().is_empty() && u.password().is_none()
    })
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    if !is_web_url(&url) {
        return Err(AppError::InvalidInput(format!("Not a web link: {url}")));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}

/// The native save panel; `None` when the user cancels.
#[tauri::command]
pub async fn choose_save_path(
    app: AppHandle,
    file_name: String,
    extension: String,
) -> AppResult<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&file_name)
        .add_filter(&extension, &[extension.as_str()])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let chosen = rx
        .await
        .map_err(|_| AppError::Io("The save panel closed without an answer".into()))?;
    let Some(path) = chosen else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|e| AppError::Io(format!("The chosen path is not a file path: {e}")))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// The native folder panel, which can also create a folder; `None` when the user cancels.
#[tauri::command]
pub async fn choose_folder(app: AppHandle) -> AppResult<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let chosen = rx
        .await
        .map_err(|_| AppError::Io("The folder panel closed without an answer".into()))?;
    let Some(path) = chosen else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|e| AppError::Io(format!("The chosen folder is not a file path: {e}")))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Opens the folder containing `path` in Finder, with `path` selected.
#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> AppResult<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn take_launch_events(state: State<'_, AppState>) -> Vec<LaunchEvent> {
    state.launches.take_events()
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    window::hide_main(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn log_frontend_error(message: String) {
    log::error!(target: "frontend", "{}", sanitize_log_text(&message));
}

const MAX_LOGGED_BYTES: usize = 4096;

/// Bounds page-supplied text and escapes line breaks, so it cannot flood the log or
/// forge log lines.
pub fn sanitize_log_text(text: &str) -> String {
    text[..text.floor_char_boundary(MAX_LOGGED_BYTES)]
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::Scheme;

    #[test]
    fn web_url_rules_cover_scheme_case_and_credentials() {
        for (url, allowed) in [
            ("https://example.com/path", true),
            ("http://localhost:8080", true),
            ("HTTPS://example.com", true),
            ("file:///etc/passwd", false),
            ("javascript:alert(1)", false),
            ("mailto:a@b.c", false),
            ("not a url", false),
            ("https://user:pw@example.com", false),
            ("https://user@example.com", false),
        ] {
            assert_eq!(is_web_url(url), allowed, "{url}");
        }
    }

    #[test]
    fn logged_text_is_cut_on_a_char_boundary() {
        let text = format!("{}é", "a".repeat(MAX_LOGGED_BYTES - 1));
        assert_eq!(sanitize_log_text(&text), "a".repeat(MAX_LOGGED_BYTES - 1));
        assert_eq!(
            sanitize_log_text(&"x".repeat(10_000)),
            "x".repeat(MAX_LOGGED_BYTES)
        );
    }

    #[test]
    fn logged_text_escapes_line_breaks() {
        assert_eq!(sanitize_log_text("a\nb\rc"), "a\\nb\\rc");
    }

    fn settings(edit: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = Settings::default();
        edit(&mut settings);
        settings
    }

    #[test]
    fn startup_without_override_applies_everything() {
        let saved = settings(|s| {
            s.log_level = LogLevel::Warn;
            s.launch_at_login = true;
        });
        assert_eq!(
            settings_effects(None, &saved, None),
            SettingsEffects {
                log_level: Some(log::LevelFilter::Warn),
                launch_at_login: Some(true),
            }
        );
    }

    #[test]
    fn a_scheme_change_alone_applies_nothing_outside_the_page() {
        let previous = Settings::default();
        let nothing = SettingsEffects {
            log_level: None,
            launch_at_login: None,
        };
        for saved in [
            settings(|s| s.light_scheme = Scheme::Dark),
            settings(|s| s.dark_scheme = Scheme::Light),
        ] {
            assert_eq!(
                settings_effects(Some(&previous), &saved, None),
                nothing,
                "{saved:?}"
            );
        }
    }

    #[test]
    fn unchanged_settings_apply_nothing() {
        let current = Settings::default();
        assert_eq!(
            settings_effects(Some(&current), &current, None),
            SettingsEffects {
                log_level: None,
                launch_at_login: None,
            }
        );
    }

    #[test]
    fn changed_log_level_is_applied() {
        let previous = Settings::default();
        let saved = settings(|s| s.log_level = LogLevel::Debug);
        assert_eq!(
            settings_effects(Some(&previous), &saved, None).log_level,
            Some(log::LevelFilter::Debug)
        );
    }

    #[test]
    fn cli_log_override_beats_a_changed_log_level() {
        let previous = Settings::default();
        let saved = settings(|s| s.log_level = LogLevel::Trace);
        assert_eq!(
            settings_effects(Some(&previous), &saved, Some(LogLevel::Error)).log_level,
            None
        );
    }

    #[test]
    fn cli_log_override_is_applied_at_startup() {
        let saved = settings(|s| s.log_level = LogLevel::Trace);
        assert_eq!(
            settings_effects(None, &saved, Some(LogLevel::Error)).log_level,
            Some(log::LevelFilter::Error)
        );
    }

    #[test]
    fn launch_at_login_is_applied_only_when_it_changes() {
        let off = Settings::default();
        let on = settings(|s| s.launch_at_login = true);
        assert_eq!(
            settings_effects(Some(&off), &on, None).launch_at_login,
            Some(true)
        );
        assert_eq!(
            settings_effects(Some(&on), &off, None).launch_at_login,
            Some(false)
        );
        assert_eq!(settings_effects(Some(&on), &on, None).launch_at_login, None);
    }
}
