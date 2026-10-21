//! Help menu targets that live outside the app: the README.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};

pub const README: &str = "README.md";

/// The README bundled into the app's resources, else the repository's (debug builds run from it).
pub fn readme_path(resource_dir: Option<&Path>) -> Option<PathBuf> {
    resource_dir
        .map(|dir| dir.join(README))
        .into_iter()
        .chain(cfg!(debug_assertions).then(repo_readme))
        .find(|path| path.is_file())
}

/// The README at the root of the repository this build was compiled from.
fn repo_readme() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(README)
}

/// Opens the README in the default app for Markdown files.
#[tauri::command]
pub fn open_readme(app: AppHandle) -> AppResult<()> {
    let resources = app.path().resource_dir().ok();
    let path = readme_path(resources.as_deref())
        .ok_or_else(|| AppError::NotFound("The README is not part of this build".into()))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_readme_is_preferred() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(README), "bundled").unwrap();
        assert_eq!(readme_path(Some(dir.path())), Some(dir.path().join(README)));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_builds_fall_back_to_the_repository_readme() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_readme();
        assert_eq!(readme_path(Some(dir.path())), Some(repo.clone()));
        assert_eq!(readme_path(None), Some(repo));
    }
}
