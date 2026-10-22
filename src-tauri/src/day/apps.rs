//! The apps the timeline settings can override: every app ever tracked (Settings → Timeline) and the day's
//! apps (the Lanes popover).

use serde::Serialize;
use ts_rs::TS;

use super::{ActivityBlock, DayApp, ViewSettings, app_key};
use crate::db::Db;
use crate::db::activities::apps_seen;
use crate::db::time::Ms;
use crate::error::AppResult;
use crate::store::settings::Settings;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct KnownApp {
    /// What its override is keyed by.
    pub app_key: String,
    /// Its latest name; the key for an override of an app never tracked.
    pub app: String,
    pub total_ms: i64,
    pub last_seen: Option<Ms>,
}

/// Every app the tracker recorded, plus every app with an override, by name.
pub async fn known_apps(db: &Db, settings: &Settings, now: Ms) -> AppResult<Vec<KnownApp>> {
    let mut apps: Vec<KnownApp> = apps_seen(db, 0, now)
        .await?
        .into_iter()
        .map(|seen| KnownApp {
            app_key: app_key(seen.bundle_id.as_deref(), &seen.app),
            app: seen.app,
            total_ms: seen.total_ms,
            last_seen: Some(seen.last_seen),
        })
        .collect();
    for key in settings.apps.keys() {
        if !apps.iter().any(|a| &a.app_key == key) {
            apps.push(KnownApp {
                app_key: key.clone(),
                app: key.clone(),
                total_ms: 0,
                last_seen: None,
            });
        }
    }
    apps.sort_by(|a, b| {
        a.app
            .to_lowercase()
            .cmp(&b.app.to_lowercase())
            .then_with(|| a.app_key.cmp(&b.app_key))
    });
    Ok(apps)
}

/// The apps of `blocks` with their active time, most first, then by name.
pub(super) fn day_apps(blocks: &[ActivityBlock], settings: &ViewSettings) -> Vec<DayApp> {
    let mut apps: Vec<DayApp> = Vec::new();
    for block in blocks {
        match apps.iter_mut().find(|a| a.app_key == block.app_key) {
            Some(app) => app.active_ms += block.active_ms,
            None => apps.push(DayApp {
                app_key: block.app_key.clone(),
                app: block.app.clone(),
                bundle_id: block.bundle_id.clone(),
                active_ms: block.active_ms,
                own_lane: settings.app(&block.app_key).is_some_and(|a| a.own_lane),
            }),
        }
    }
    apps.sort_by(|a, b| {
        b.active_ms
            .cmp(&a.active_ms)
            .then_with(|| a.app.to_lowercase().cmp(&b.app.to_lowercase()))
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::activities::{self, NewActivity};
    use crate::store::settings::AppOverride;
    use crate::test_support::activity;

    const MIN: Ms = 60_000;
    const HOUR: Ms = 60 * MIN;
    const NOW: Ms = 10 * HOUR;

    async fn seeded() -> Db {
        let db = Db::open_memory().await.unwrap();
        activity(&db, 0, Some(30 * MIN), "zebra", "").await;
        activity(&db, 2 * HOUR, Some(2 * HOUR + 10 * MIN), "apple", "").await;
        activities::insert(
            &db,
            NewActivity {
                source: "tracker".into(),
                start: HOUR,
                app: "Mail".into(),
                bundle_id: Some("com.apple.mail".into()),
                title: String::new(),
                url: None,
                path: None,
                private: false,
            },
        )
        .await
        .unwrap();
        db
    }

    fn overriding(keys: &[&str]) -> Settings {
        let mut settings = Settings::default();
        for key in keys {
            settings.apps.insert(
                (*key).into(),
                AppOverride {
                    hidden: true,
                    ..AppOverride::default()
                },
            );
        }
        settings
    }

    fn app(app_key: &str, app: &str, total_ms: i64, last_seen: Option<Ms>) -> KnownApp {
        KnownApp {
            app_key: app_key.into(),
            app: app.into(),
            total_ms,
            last_seen,
        }
    }

    #[tokio::test]
    async fn tracker_apps_are_keyed_by_bundle_id_or_by_name_and_carry_their_time() {
        let db = seeded().await;

        let apps = known_apps(&db, &Settings::default(), NOW).await.unwrap();

        assert_eq!(
            apps,
            [
                app("apple", "apple", 10 * MIN, Some(2 * HOUR + 10 * MIN)),
                app("com.apple.mail", "Mail", NOW - HOUR, Some(NOW)),
                app("zebra", "zebra", 30 * MIN, Some(30 * MIN)),
            ]
        );
    }

    #[tokio::test]
    async fn an_override_of_an_app_never_tracked_is_listed_with_no_time() {
        let db = seeded().await;

        let apps = known_apps(&db, &overriding(&["Gone.app"]), NOW)
            .await
            .unwrap();

        assert!(apps.contains(&app("Gone.app", "Gone.app", 0, None)));
        assert_eq!(apps.len(), 4);
    }

    #[tokio::test]
    async fn an_override_of_a_tracked_app_does_not_add_a_second_entry() {
        let db = seeded().await;

        let apps = known_apps(&db, &overriding(&["com.apple.mail", "zebra"]), NOW)
            .await
            .unwrap();

        assert_eq!(apps.len(), 3);
    }

    #[tokio::test]
    async fn apps_are_sorted_by_name_ignoring_case() {
        let db = seeded().await;

        let apps = known_apps(&db, &overriding(&["Beta.app", "alpha.app"]), NOW)
            .await
            .unwrap();

        let names: Vec<&str> = apps.iter().map(|a| a.app.as_str()).collect();
        assert_eq!(names, ["alpha.app", "apple", "Beta.app", "Mail", "zebra"]);
    }

    #[tokio::test]
    async fn without_any_activity_or_override_the_list_is_empty() {
        let db = Db::open_memory().await.unwrap();

        let apps = known_apps(&db, &Settings::default(), NOW).await.unwrap();

        assert!(apps.is_empty());
    }

    #[tokio::test]
    async fn a_known_app_serializes_with_camel_case_keys() {
        let value = serde_json::to_value(app("com.apple.mail", "Mail", 5, None)).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "appKey": "com.apple.mail",
                "app": "Mail",
                "totalMs": 5,
                "lastSeen": null
            })
        );
    }
}
