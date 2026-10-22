//! App icons from the installed apps, cached as PNG files; the page gets them as data URLs.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;

/// Pixels per side: 32 points at @2x.
pub const ICON_PX: u32 = 64;

/// Reads an installed app's icon.
pub trait IconSource: Send + Sync {
    /// A PNG of `ICON_PX` square; `None` when no app with this bundle id is installed.
    fn png(&self, bundle_id: &str) -> Option<Vec<u8>>;
}

pub struct AppIcons {
    dir: PathBuf,
    source: Arc<dyn IconSource>,
    /// Ids without an icon this run, so they are not looked up again.
    missing: Mutex<HashSet<String>>,
}

impl AppIcons {
    pub fn new(dir: PathBuf, source: Arc<dyn IconSource>) -> Self {
        Self {
            dir,
            source,
            missing: Mutex::new(HashSet::new()),
        }
    }

    /// The cached PNG, else one read from the source and cached; `None` for an unknown app.
    pub fn icon(&self, bundle_id: &str) -> Option<Vec<u8>> {
        if !is_bundle_id(bundle_id) || self.missing.lock_unpoisoned().contains(bundle_id) {
            return None;
        }
        let file = self.dir.join(format!("{bundle_id}.png"));
        if let Ok(png) = std::fs::read(&file) {
            return Some(png);
        }
        let Some(png) = self.source.png(bundle_id) else {
            self.missing.lock_unpoisoned().insert(bundle_id.to_string());
            return None;
        };
        if let Err(e) = std::fs::create_dir_all(&self.dir).and_then(|_| std::fs::write(&file, &png))
        {
            log::warn!("Cannot cache the icon of {bundle_id}: {e}");
        }
        Some(png)
    }

    /// `data:image/png;base64,…` per id that has an icon; ids without one are left out.
    pub fn data_urls(&self, bundle_ids: &[String]) -> HashMap<String, String> {
        bundle_ids
            .iter()
            .filter_map(|id| {
                let png = self.icon(id)?;
                Some((
                    id.clone(),
                    format!("data:image/png;base64,{}", STANDARD.encode(png)),
                ))
            })
            .collect()
    }
}

/// Also keeps names and paths out of the cache folder's file names.
fn is_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeSource {
        icons: Mutex<HashMap<String, Vec<u8>>>,
        everything: Mutex<Option<Vec<u8>>>,
    }

    impl FakeSource {
        fn with(entries: &[(&str, &[u8])]) -> Arc<Self> {
            let fake = Self::default();
            for (id, png) in entries {
                fake.icons
                    .lock()
                    .unwrap()
                    .insert((*id).to_string(), png.to_vec());
            }
            Arc::new(fake)
        }

        fn answer_every_id_with(png: &[u8]) -> Arc<Self> {
            let fake = Self::default();
            *fake.everything.lock().unwrap() = Some(png.to_vec());
            Arc::new(fake)
        }

        fn replace(&self, id: &str, png: Option<&[u8]>) {
            let mut icons = self.icons.lock().unwrap();
            match png {
                Some(png) => icons.insert(id.to_string(), png.to_vec()),
                None => icons.remove(id),
            };
        }
    }

    impl IconSource for FakeSource {
        fn png(&self, bundle_id: &str) -> Option<Vec<u8>> {
            if let Some(png) = self.everything.lock().unwrap().clone() {
                return Some(png);
            }
            self.icons.lock().unwrap().get(bundle_id).cloned()
        }
    }

    fn cache_dir(root: &tempfile::TempDir) -> PathBuf {
        root.path().join("icons")
    }

    #[test]
    fn a_miss_returns_the_source_png_and_caches_it_as_a_file() {
        let root = tempfile::tempdir().unwrap();
        let source = FakeSource::with(&[("com.apple.Safari", b"safari-png")]);
        let icons = AppIcons::new(cache_dir(&root), source);

        assert_eq!(icons.icon("com.apple.Safari"), Some(b"safari-png".to_vec()));
        assert_eq!(
            std::fs::read(cache_dir(&root).join("com.apple.Safari.png")).unwrap(),
            b"safari-png"
        );
    }

    #[test]
    fn a_cached_icon_is_returned_even_when_the_source_changes_or_forgets_it() {
        let root = tempfile::tempdir().unwrap();
        let source = FakeSource::with(&[("com.apple.Safari", b"first")]);
        let icons = AppIcons::new(cache_dir(&root), source.clone());
        assert_eq!(icons.icon("com.apple.Safari"), Some(b"first".to_vec()));

        source.replace("com.apple.Safari", Some(b"second"));
        assert_eq!(icons.icon("com.apple.Safari"), Some(b"first".to_vec()));

        source.replace("com.apple.Safari", None);
        assert_eq!(icons.icon("com.apple.Safari"), Some(b"first".to_vec()));
    }

    #[test]
    fn a_new_instance_on_the_same_folder_reads_the_cached_icon() {
        let root = tempfile::tempdir().unwrap();
        let source = FakeSource::with(&[("com.apple.Safari", b"first")]);
        AppIcons::new(cache_dir(&root), source)
            .icon("com.apple.Safari")
            .unwrap();

        let other = AppIcons::new(cache_dir(&root), FakeSource::with(&[]));
        assert_eq!(other.icon("com.apple.Safari"), Some(b"first".to_vec()));

        let changed = AppIcons::new(
            cache_dir(&root),
            FakeSource::with(&[("com.apple.Safari", b"other")]),
        );
        assert_eq!(changed.icon("com.apple.Safari"), Some(b"first".to_vec()));
    }

    #[test]
    fn an_unknown_app_gives_none_and_writes_no_file() {
        let root = tempfile::tempdir().unwrap();
        let icons = AppIcons::new(cache_dir(&root), FakeSource::with(&[]));

        assert_eq!(icons.icon("com.example.Missing"), None);
        assert_eq!(icons.icon("com.example.Missing"), None);
        assert!(!cache_dir(&root).join("com.example.Missing.png").exists());
    }

    #[test]
    fn invalid_ids_give_none_and_write_no_file() {
        let root = tempfile::tempdir().unwrap();
        let icons = AppIcons::new(cache_dir(&root), FakeSource::answer_every_id_with(b"png"));

        for id in ["", "../x", ".hidden", "name with spaces"] {
            assert_eq!(icons.icon(id), None, "{id:?}");
        }

        assert!(!root.path().join("x.png").exists());
        assert!(!cache_dir(&root).join(".hidden.png").exists());
        assert!(!cache_dir(&root).join("name with spaces.png").exists());
        assert!(!cache_dir(&root).join(".png").exists());
    }

    #[test]
    fn data_urls_encode_known_ids_and_leave_out_unknown_ones() {
        let root = tempfile::tempdir().unwrap();
        let source = FakeSource::with(&[("com.a.One", b"one"), ("com.b.Two", b"two")]);
        let icons = AppIcons::new(cache_dir(&root), source);

        let urls = icons.data_urls(&[
            "com.a.One".to_string(),
            "com.unknown.App".to_string(),
            "../x".to_string(),
            "com.b.Two".to_string(),
        ]);

        let expected = HashMap::from([
            (
                "com.a.One".to_string(),
                format!("data:image/png;base64,{}", STANDARD.encode(b"one")),
            ),
            (
                "com.b.Two".to_string(),
                format!("data:image/png;base64,{}", STANDARD.encode(b"two")),
            ),
        ]);
        assert_eq!(urls, expected);
    }
}

#[tauri::command]
pub async fn app_icons(
    icons: State<'_, Arc<AppIcons>>,
    bundle_ids: Vec<String>,
) -> AppResult<HashMap<String, String>> {
    let icons = Arc::clone(&icons);
    tokio::task::spawn_blocking(move || icons.data_urls(&bundle_ids))
        .await
        .map_err(|e| AppError::Io(format!("Reading app icons failed: {e}")))
}

/// The installed apps' icons.
pub fn installed() -> Arc<dyn IconSource> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::InstalledApps)
    }
    #[cfg(not(target_os = "macos"))]
    {
        struct NoIcons;
        impl IconSource for NoIcons {
            fn png(&self, _: &str) -> Option<Vec<u8>> {
                None
            }
        }
        Arc::new(NoIcons)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext,
        NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    use super::{ICON_PX, IconSource};

    pub struct InstalledApps;

    impl IconSource for InstalledApps {
        fn png(&self, bundle_id: &str) -> Option<Vec<u8>> {
            let workspace = NSWorkspace::sharedWorkspace();
            let url =
                workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
            let path = url.path()?;
            let image = workspace.iconForFile(&path);
            let px = ICON_PX as isize;
            let bitmap = unsafe {
                NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                    NSBitmapImageRep::alloc(),
                    std::ptr::null_mut(),
                    px,
                    px,
                    8,
                    4,
                    true,
                    false,
                    NSDeviceRGBColorSpace,
                    0,
                    0,
                )
            }?;
            let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
            // The current context is per thread, so drawing here leaves the main thread's alone.
            NSGraphicsContext::saveGraphicsState_class();
            NSGraphicsContext::setCurrentContext(Some(&context));
            let side = f64::from(ICON_PX);
            image.drawInRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side, side)));
            context.flushGraphics();
            NSGraphicsContext::restoreGraphicsState_class();
            let png = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            }?;
            Some(png.to_vec())
        }
    }
}
