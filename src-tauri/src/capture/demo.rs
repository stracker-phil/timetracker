//! The debug build's capture (ADR 0031): it replays the showcase's made-up windows, so a debug database,
//! its screenshots and its logs never hold the owner's real apps, titles or addresses.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::{PermissionStatus, Snapshot, Source, SystemEvent};
use crate::db::time::{Ms, now_ms};
use crate::store::settings::CaptureSettings;
use crate::test_support::showcase::{Window, demo_windows};

/// How long each demo window stays in front.
const DWELL_MS: Ms = 3 * 60_000;

pub struct DemoSource {
    windows: Vec<Window>,
    /// Never sends; held so that receivers stay open.
    events: broadcast::Sender<SystemEvent>,
}

impl DemoSource {
    pub fn new() -> Arc<DemoSource> {
        Arc::new(DemoSource {
            windows: demo_windows().collect(),
            events: broadcast::channel(1).0,
        })
    }

    /// The window in front at `at`, cycling through the list every `DWELL_MS`.
    fn snapshot_at(&self, at: Ms) -> Snapshot {
        let slot = at.div_euclid(DWELL_MS);
        let index = slot.rem_euclid(self.windows.len() as i64) as usize;
        let (app, bundle_id, title, url) = self.windows[index];
        Snapshot {
            at,
            app: app.into(),
            bundle_id: Some(bundle_id.into()),
            pid: index as i32 + 1, // pid 0 is no process
            title: title.into(),
            url: url.map(Into::into),
            path: None,
            private: false,
        }
    }
}

impl Source for DemoSource {
    fn snapshot(&self, _capture: &CaptureSettings) -> Option<Snapshot> {
        Some(self.snapshot_at(now_ms()))
    }

    fn idle_seconds(&self) -> f64 {
        0.0
    }

    fn events(&self) -> broadcast::Receiver<SystemEvent> {
        self.events.subscribe()
    }

    fn permission(&self) -> PermissionStatus {
        PermissionStatus::Granted
    }

    fn request_permission(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows() -> Vec<Window> {
        demo_windows().collect()
    }

    fn key(snapshot: &Snapshot) -> (String, Option<String>, String, Option<String>) {
        (
            snapshot.app.clone(),
            snapshot.bundle_id.clone(),
            snapshot.title.clone(),
            snapshot.url.clone(),
        )
    }

    fn window_key(window: &Window) -> (String, Option<String>, String, Option<String>) {
        let (app, bundle, title, url) = *window;
        (
            app.into(),
            Some(bundle.into()),
            title.into(),
            url.map(Into::into),
        )
    }

    fn is_demo_window(snapshot: &Snapshot) -> bool {
        windows().iter().map(window_key).any(|k| k == key(snapshot))
    }

    #[test]
    fn a_snapshot_is_one_of_the_demo_windows() {
        let source = DemoSource::new();
        let snapshot = source.snapshot_at(5 * DWELL_MS);
        assert!(is_demo_window(&snapshot), "{snapshot:?}");
        assert_eq!(snapshot.at, 5 * DWELL_MS);
        assert_eq!(snapshot.path, None);
        assert!(!snapshot.private);

        let captured = source
            .snapshot(&CaptureSettings::default())
            .expect("a snapshot");
        assert!(is_demo_window(&captured), "{captured:?}");
    }

    #[test]
    fn the_same_window_stays_in_front_within_one_slot() {
        let source = DemoSource::new();
        let first = source.snapshot_at(2 * DWELL_MS);
        let last = source.snapshot_at(3 * DWELL_MS - 1);
        assert_eq!(key(&first), key(&last));
        assert_eq!(first.pid, last.pid);
    }

    #[test]
    fn the_next_slot_shows_the_next_window() {
        let source = DemoSource::new();
        let all = windows();
        assert!(all.len() > 1);
        for (slot, (app, _, title, _)) in all.iter().enumerate() {
            let snapshot = source.snapshot_at(slot as Ms * DWELL_MS);
            assert_eq!(
                (snapshot.app.as_str(), snapshot.title.as_str()),
                (*app, *title)
            );
        }
        assert_ne!(
            key(&source.snapshot_at(0)),
            key(&source.snapshot_at(DWELL_MS))
        );
    }

    #[test]
    fn the_windows_wrap_around_after_the_last_one() {
        let source = DemoSource::new();
        let count = windows().len() as i64;
        assert_eq!(
            key(&source.snapshot_at(count * DWELL_MS)),
            key(&source.snapshot_at(0))
        );
    }

    #[test]
    fn zero_and_negative_times_still_give_a_window() {
        let source = DemoSource::new();
        let count = windows().len() as i64;
        for at in [0, -1, -DWELL_MS, -count * DWELL_MS - 1, i64::MIN / 2] {
            let snapshot = source.snapshot_at(at);
            assert_eq!(snapshot.at, at, "{at}");
        }
        assert_eq!(
            key(&source.snapshot_at(-1)),
            key(&source.snapshot_at((count - 1) * DWELL_MS))
        );
    }

    #[test]
    fn the_source_is_permitted_and_never_idle() {
        let source = DemoSource::new();
        assert_eq!(source.permission(), PermissionStatus::Granted);
        assert_eq!(source.idle_seconds(), 0.0);
    }
}
