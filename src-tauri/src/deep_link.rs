//! `timetracker://` links from other apps.

use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;

pub fn listen(handle: &AppHandle) {
    handle.deep_link().on_open_url(|event| {
        for url in event.urls() {
            log::info!("Received the deep link {url}");
        }
    });
}
