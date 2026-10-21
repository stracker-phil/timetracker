use super::*;
use crate::capture::fake::{self, FakeSource};
use crate::test_support::activity;

const T: Ms = 1_800_000_000_000;
const SECOND: Ms = 1_000;
const MINUTE: Ms = 60_000;

struct Fixture {
    db: Db,
    source: Arc<FakeSource>,
    tracker: Arc<Tracker>,
}

async fn fixture() -> Fixture {
    let db = Db::open_memory().await.unwrap();
    let source = FakeSource::new();
    let tracker = Tracker::new(db.clone(), source.clone(), TrackerSettings::default(), T);
    Fixture {
        db,
        source,
        tracker,
    }
}

/// Activity rows as (start, end, title), by start.
async fn spans(db: &Db) -> Vec<(Ms, Option<Ms>, String)> {
    activities::list_for_range(db, 0, Ms::MAX)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.start, r.end, r.title))
        .collect()
}

async fn aways(db: &Db) -> Vec<(Ms, Option<Ms>, AwayReason)> {
    away::list_for_range(db, 0, Ms::MAX)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.start, r.end, r.reason))
        .collect()
}

fn title(title: &str, start: Ms, end: Option<Ms>) -> (Ms, Option<Ms>, String) {
    (start, end, title.into())
}

#[test]
fn tracker_states_serialize_in_the_shape_the_page_uses() {
    for (state, expected) in [
        (
            TrackerState::Running,
            serde_json::json!({"kind": "running"}),
        ),
        (
            TrackerState::NeedsPermission,
            serde_json::json!({"kind": "needsPermission"}),
        ),
        (
            TrackerState::Paused { until: Some(5) },
            serde_json::json!({"kind": "paused", "until": 5}),
        ),
        (
            TrackerState::Paused { until: None },
            serde_json::json!({"kind": "paused", "until": null}),
        ),
        (
            TrackerState::Away {
                since: 7,
                reason: AwayReason::Idle,
            },
            serde_json::json!({"kind": "away", "since": 7, "reason": "idle"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&state).unwrap(), expected, "{state:?}");
    }
}

#[test]
fn pause_requests_deserialize_from_the_page_shape() {
    for (json, expected) in [
        (serde_json::json!({"minutes": 15}), PauseUntil::Minutes(15)),
        (serde_json::json!("tomorrow"), PauseUntil::Tomorrow),
        (serde_json::json!("resumed"), PauseUntil::Resumed),
    ] {
        assert_eq!(
            serde_json::from_value::<PauseUntil>(json.clone()).unwrap(),
            expected,
            "{json}"
        );
    }
}

#[tokio::test]
async fn the_first_snapshot_opens_a_row_and_the_status_shows_it() {
    let f = fixture().await;
    f.source.show("Code", "main.rs");

    f.tracker.tick(T).await.unwrap();

    let rows = activities::list_for_range(&f.db, 0, Ms::MAX).await.unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.source, "tracker");
    assert_eq!((row.start, row.end), (T, None));
    assert_eq!(row.app, "Code");
    assert_eq!(row.bundle_id.as_deref(), Some("test.code"));
    assert_eq!(row.title, "main.rs");
    let status = f.tracker.status();
    assert_eq!(status.state, TrackerState::Running);
    assert_eq!(status.current.as_ref(), Some(row));
    assert_eq!(status.text, "Tracking · Code");
}

#[tokio::test]
async fn a_title_change_closes_the_row_and_opens_the_next_at_the_same_instant() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    f.source.show("Code", "b.rs");
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    assert_eq!(
        spans(&f.db).await,
        [
            title("a.rs", T, Some(T + 5 * SECOND)),
            title("b.rs", T + 5 * SECOND, None)
        ]
    );
}

#[tokio::test]
async fn an_unchanged_snapshot_keeps_the_single_row_open() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");

    for i in 0..4 {
        f.tracker.tick(T + i * 2 * SECOND).await.unwrap();
    }

    assert_eq!(spans(&f.db).await, [title("a.rs", T, None)]);
}

#[tokio::test]
async fn resetting_the_open_span_makes_the_next_tick_open_a_new_row() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    let open = f.tracker.status().current.unwrap();
    activities::delete_range(&f.db, 0, Ms::MAX).await.unwrap();

    f.tracker.reset_span(&[open.id], T + SECOND).await;
    assert_eq!(f.tracker.status().current, None);
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    assert_eq!(spans(&f.db).await, [title("a.rs", T + 5 * SECOND, None)]);
    assert_eq!(
        f.tracker.status().current.map(|row| row.title),
        Some("a.rs".into())
    );
}

#[tokio::test]
async fn resetting_other_rows_keeps_the_open_span() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    let open = f.tracker.status().current.unwrap();

    f.tracker.reset_span(&[open.id + 100], T + SECOND).await;
    f.tracker.reset_span(&[], T + SECOND).await;
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    assert_eq!(f.tracker.status().current.as_ref(), Some(&open));
    assert_eq!(spans(&f.db).await, [title("a.rs", T, None)]);
}

#[tokio::test]
async fn a_window_in_front_for_under_a_second_is_replaced_in_place() {
    let f = fixture().await;
    f.source.show("Safari", "Tab");
    f.tracker.tick(T).await.unwrap();
    let first = f.tracker.status().current.unwrap();

    f.source.show("Code", "main.rs");
    f.tracker.tick(T + 999).await.unwrap();

    let rows = activities::list_for_range(&f.db, 0, Ms::MAX).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].id, rows[0].start, rows[0].end),
        (first.id, T, None)
    );
    assert_eq!(
        (
            rows[0].app.as_str(),
            rows[0].bundle_id.as_deref(),
            rows[0].title.as_str()
        ),
        ("Code", Some("test.code"), "main.rs")
    );
    assert_eq!(f.tracker.status().current.as_ref(), Some(&rows[0]));
}

#[tokio::test]
async fn a_change_after_exactly_a_second_is_kept_as_its_own_row() {
    let f = fixture().await;
    f.source.show("Safari", "Tab");
    f.tracker.tick(T).await.unwrap();

    f.source.show("Code", "main.rs");
    f.tracker.tick(T + MIN_SPAN).await.unwrap();

    assert_eq!(
        spans(&f.db).await,
        [
            title("Tab", T, Some(T + MIN_SPAN)),
            title("main.rs", T + MIN_SPAN, None)
        ]
    );
}

#[tokio::test]
async fn nothing_frontmost_closes_the_row() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    f.source.set_snapshot(None);
    f.tracker.tick(T + 3 * SECOND).await.unwrap();

    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(T + 3 * SECOND))]);
    let status = f.tracker.status();
    assert_eq!(status.current, None);
    assert_eq!(status.text, "Tracking");
}

#[tokio::test]
async fn idle_time_is_backdated_and_input_resumes_recording() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    f.source.set_idle(300.0);
    f.tracker.tick(T + 400 * SECOND).await.unwrap();

    let since = T + 100 * SECOND;
    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(since))]);
    assert_eq!(aways(&f.db).await, [(since, None, AwayReason::Idle)]);
    let status = f.tracker.status();
    assert_eq!(
        status.state,
        TrackerState::Away {
            since,
            reason: AwayReason::Idle
        }
    );
    assert_eq!(status.current, None);

    f.source.set_idle(0.0);
    let back = T + 410 * SECOND;
    f.tracker.tick(back).await.unwrap();

    assert_eq!(aways(&f.db).await, [(since, Some(back), AwayReason::Idle)]);
    assert_eq!(
        spans(&f.db).await,
        [title("a.rs", T, Some(since)), title("a.rs", back, None)]
    );
    assert_eq!(f.tracker.status().state, TrackerState::Running);
}

#[tokio::test]
async fn idle_just_below_the_threshold_keeps_recording() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.source.set_idle(299.0);

    f.tracker.tick(T + 400 * SECOND).await.unwrap();

    assert!(aways(&f.db).await.is_empty());
    assert_eq!(spans(&f.db).await, [title("a.rs", T + 400 * SECOND, None)]);
}

#[tokio::test]
async fn an_idle_span_never_starts_before_the_tracker_started_running() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.tracker
        .pause(PauseUntil::Resumed, T + MINUTE)
        .await
        .unwrap();

    f.source.set_idle(1_000.0);
    let resumed = T + 2 * MINUTE;
    f.tracker.resume(resumed).await.unwrap();

    assert_eq!(
        aways(&f.db).await,
        [
            (T + MINUTE, Some(resumed), AwayReason::Paused),
            (resumed, None, AwayReason::Idle)
        ]
    );
}

#[tokio::test]
async fn changed_idle_minutes_change_the_threshold() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.source.set_idle(200.0);
    let now = T + 250 * SECOND;
    f.tracker.tick(now).await.unwrap();
    assert_eq!(f.tracker.status().state, TrackerState::Running);

    f.tracker.set_settings(TrackerSettings {
        idle_minutes: 3,
        ..TrackerSettings::default()
    });
    f.tracker.tick(now).await.unwrap();

    assert_eq!(
        f.tracker.status().state,
        TrackerState::Away {
            since: now - 200 * SECOND,
            reason: AwayReason::Idle
        }
    );
}

#[tokio::test]
async fn system_events_open_and_close_away_spans() {
    let cases = [
        (SystemEvent::Sleep, SystemEvent::Wake, AwayReason::Sleep),
        (SystemEvent::Lock, SystemEvent::Unlock, AwayReason::Lock),
        (
            SystemEvent::SessionInactive,
            SystemEvent::SessionActive,
            AwayReason::Lock,
        ),
    ];
    for (start_event, end_event, reason) in cases {
        let f = fixture().await;
        f.source.show("Code", "a.rs");
        f.tracker.tick(T).await.unwrap();

        f.tracker
            .on_event(start_event, T + 10 * SECOND)
            .await
            .unwrap();

        let started = T + 10 * SECOND;
        assert_eq!(
            spans(&f.db).await,
            [title("a.rs", T, Some(started))],
            "{reason:?}"
        );
        assert_eq!(aways(&f.db).await, [(started, None, reason)], "{reason:?}");
        assert_eq!(
            f.tracker.status().state,
            TrackerState::Away {
                since: started,
                reason
            },
            "{reason:?}"
        );

        let ended = T + 20 * SECOND;
        f.tracker.on_event(end_event, ended).await.unwrap();

        assert_eq!(
            aways(&f.db).await,
            [(started, Some(ended), reason)],
            "{reason:?}"
        );
        assert_eq!(
            spans(&f.db).await,
            [title("a.rs", T, Some(started)), title("a.rs", ended, None)],
            "{reason:?}"
        );
        assert_eq!(
            f.tracker.status().state,
            TrackerState::Running,
            "{reason:?}"
        );
    }
}

#[tokio::test]
async fn sleeping_while_idle_ends_the_idle_span_and_starts_a_sleep_span() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.source.set_idle(300.0);
    f.tracker.tick(T + 400 * SECOND).await.unwrap();

    let slept = T + 500 * SECOND;
    f.tracker.on_event(SystemEvent::Sleep, slept).await.unwrap();

    assert_eq!(
        aways(&f.db).await,
        [
            (T + 100 * SECOND, Some(slept), AwayReason::Idle),
            (slept, None, AwayReason::Sleep)
        ]
    );
}

#[tokio::test]
async fn pausing_closes_the_activity_and_ticks_record_nothing_until_the_end() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    let paused = T + 5 * SECOND;
    let until = paused + 15 * MINUTE;
    let status = f
        .tracker
        .pause(PauseUntil::Minutes(15), paused)
        .await
        .unwrap();

    assert_eq!(status.state, TrackerState::Paused { until: Some(until) });
    assert!(status.text.starts_with("Paused until"), "{}", status.text);
    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(paused))]);
    assert_eq!(aways(&f.db).await, [(paused, None, AwayReason::Paused)]);

    f.source.show("Code", "b.rs");
    f.tracker.tick(until - 1).await.unwrap();
    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(paused))]);
    assert_eq!(
        f.tracker.status().state,
        TrackerState::Paused { until: Some(until) }
    );

    f.tracker.tick(until).await.unwrap();

    assert_eq!(
        aways(&f.db).await,
        [(paused, Some(until), AwayReason::Paused)]
    );
    assert_eq!(
        spans(&f.db).await,
        [title("a.rs", T, Some(paused)), title("b.rs", until, None)]
    );
    assert_eq!(f.tracker.status().state, TrackerState::Running);
}

#[tokio::test]
async fn pausing_again_only_moves_the_end() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.tracker
        .pause(PauseUntil::Minutes(10), T + SECOND)
        .await
        .unwrap();

    let status = f
        .tracker
        .pause(PauseUntil::Minutes(30), T + MINUTE)
        .await
        .unwrap();

    assert_eq!(
        status.state,
        TrackerState::Paused {
            until: Some(T + MINUTE + 30 * MINUTE)
        }
    );
    assert_eq!(aways(&f.db).await, [(T + SECOND, None, AwayReason::Paused)]);
}

#[tokio::test]
async fn resume_ends_the_pause_at_once_and_records_again() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.tracker
        .pause(PauseUntil::Minutes(15), T + SECOND)
        .await
        .unwrap();

    let resumed = T + 5 * SECOND;
    let status = f.tracker.resume(resumed).await.unwrap();

    assert_eq!(status.state, TrackerState::Running);
    assert_eq!(
        aways(&f.db).await,
        [(T + SECOND, Some(resumed), AwayReason::Paused)]
    );
    assert_eq!(
        spans(&f.db).await,
        [
            title("a.rs", T, Some(T + SECOND)),
            title("a.rs", resumed, None)
        ]
    );
}

#[tokio::test]
async fn pausing_until_tomorrow_ends_at_the_next_local_midnight() {
    let f = fixture().await;

    let status = f.tracker.pause(PauseUntil::Tomorrow, T).await.unwrap();

    assert_eq!(
        status.state,
        TrackerState::Paused {
            until: Some(Day::of(T).next().bounds().0)
        }
    );
    assert_eq!(status.text, "Paused until tomorrow");
}

#[tokio::test]
async fn an_open_ended_pause_lasts_until_resumed() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");

    let status = f.tracker.pause(PauseUntil::Resumed, T).await.unwrap();
    assert_eq!(status.state, TrackerState::Paused { until: None });
    assert_eq!(status.text, "Paused");

    f.tracker.tick(T + 1_000 * MINUTE).await.unwrap();

    assert_eq!(
        f.tracker.status().state,
        TrackerState::Paused { until: None }
    );
    assert!(spans(&f.db).await.is_empty());
    assert_eq!(aways(&f.db).await, [(T, None, AwayReason::Paused)]);
}

#[tokio::test]
async fn denied_permission_records_nothing_and_needs_permission() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.source.set_permission(PermissionStatus::Denied);

    f.tracker.tick(T).await.unwrap();

    assert!(spans(&f.db).await.is_empty());
    let status = f.tracker.status();
    assert_eq!(status.state, TrackerState::NeedsPermission);
    assert_eq!(status.permission, PermissionStatus::Denied);
    assert_eq!(status.text, "Needs permission");
}

#[tokio::test]
async fn losing_the_permission_closes_the_open_activity() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    f.source.set_permission(PermissionStatus::Denied);
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(T + 5 * SECOND))]);
    assert_eq!(f.tracker.status().current, None);
}

#[tokio::test]
async fn a_granted_permission_starts_recording_on_the_next_tick() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.source.set_permission(PermissionStatus::Denied);
    f.tracker.tick(T).await.unwrap();

    f.source.set_permission(PermissionStatus::Granted);
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    let status = f.tracker.status();
    assert_eq!(status.state, TrackerState::Running);
    assert_eq!(status.permission, PermissionStatus::Granted);
    assert_eq!(spans(&f.db).await, [title("a.rs", T + 5 * SECOND, None)]);
}

#[tokio::test]
async fn requesting_permission_returns_the_status_and_starts_recording_when_granted() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.source.set_permission(PermissionStatus::Denied);
    f.tracker.tick(T).await.unwrap();
    assert_eq!(
        f.tracker.request_permission(T + SECOND).await.unwrap(),
        PermissionStatus::Denied
    );

    f.source.set_permission(PermissionStatus::Granted);
    let permission = f.tracker.request_permission(T + 5 * SECOND).await.unwrap();

    assert_eq!(permission, PermissionStatus::Granted);
    assert_eq!(f.tracker.status().state, TrackerState::Running);
    assert_eq!(spans(&f.db).await, [title("a.rs", T + 5 * SECOND, None)]);
}

#[tokio::test]
async fn recover_closes_stale_tracker_and_away_spans_only() {
    let f = fixture().await;
    let stale = activity(&f.db, T, None, "Code", "a.rs").await;
    let external = activities::insert(
        &f.db,
        NewActivity {
            source: "external:git".into(),
            start: T,
            app: "git".into(),
            bundle_id: None,
            title: "commit".into(),
            url: None,
            path: None,
            private: false,
        },
    )
    .await
    .unwrap();
    away::insert(
        &f.db,
        NewAway {
            start: T,
            reason: AwayReason::Idle,
        },
    )
    .await
    .unwrap();

    f.tracker.recover(T + 10 * MINUTE).await.unwrap();

    let rows = activities::list_for_range(&f.db, 0, Ms::MAX).await.unwrap();
    let end_of = |id: i64| rows.iter().find(|r| r.id == id).unwrap().end;
    assert_eq!(end_of(stale.id), Some(T + 2 * SECOND));
    assert_eq!(end_of(external), None);
    assert_eq!(
        aways(&f.db).await,
        [(T, Some(T + 10 * MINUTE), AwayReason::Idle)]
    );
}

/// What the subscriber got, as (start, end, reason).
fn drain(rx: &mut tokio::sync::broadcast::Receiver<AwayRow>) -> Vec<(Ms, Option<Ms>, AwayReason)> {
    let mut got = Vec::new();
    while let Ok(row) = rx.try_recv() {
        got.push((row.start, row.end, row.reason));
    }
    got
}

#[tokio::test]
async fn input_after_idle_sends_the_closed_idle_span() {
    let f = fixture().await;
    let mut rx = f.tracker.subscribe_away();
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.source.set_idle(300.0);
    f.tracker.tick(T + 400 * SECOND).await.unwrap();
    assert!(drain(&mut rx).is_empty());

    f.source.set_idle(0.0);
    let back = T + 410 * SECOND;
    f.tracker.tick(back).await.unwrap();

    assert_eq!(
        drain(&mut rx),
        [(T + 100 * SECOND, Some(back), AwayReason::Idle)]
    );
}

#[tokio::test]
async fn unlocking_sends_the_closed_lock_span() {
    let f = fixture().await;
    let mut rx = f.tracker.subscribe_away();
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.tracker
        .on_event(SystemEvent::Lock, T + 10 * SECOND)
        .await
        .unwrap();
    assert!(drain(&mut rx).is_empty());

    f.tracker
        .on_event(SystemEvent::Unlock, T + 20 * MINUTE)
        .await
        .unwrap();

    assert_eq!(
        drain(&mut rx),
        [(T + 10 * SECOND, Some(T + 20 * MINUTE), AwayReason::Lock)]
    );
}

#[tokio::test]
async fn resuming_sends_the_closed_pause_span() {
    let f = fixture().await;
    let mut rx = f.tracker.subscribe_away();
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.tracker
        .pause(PauseUntil::Resumed, T + SECOND)
        .await
        .unwrap();
    assert!(drain(&mut rx).is_empty());

    f.tracker.resume(T + 30 * MINUTE).await.unwrap();

    assert_eq!(
        drain(&mut rx),
        [(T + SECOND, Some(T + 30 * MINUTE), AwayReason::Paused)]
    );
}

#[tokio::test]
async fn a_span_closed_by_entering_another_away_state_is_not_sent() {
    let f = fixture().await;
    let mut rx = f.tracker.subscribe_away();
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();
    f.source.set_idle(300.0);
    f.tracker.tick(T + 400 * SECOND).await.unwrap();

    f.tracker
        .on_event(SystemEvent::Sleep, T + 500 * SECOND)
        .await
        .unwrap();

    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn recover_sends_nothing() {
    let f = fixture().await;
    let mut rx = f.tracker.subscribe_away();
    away::insert(
        &f.db,
        NewAway {
            start: T,
            reason: AwayReason::Idle,
        },
    )
    .await
    .unwrap();

    f.tracker.recover(T + 10 * MINUTE).await.unwrap();

    assert!(drain(&mut rx).is_empty());
}

fn browser_snapshot(title: &str, url: Option<&str>) -> Snapshot {
    Snapshot {
        url: url.map(str::to_string),
        ..fake::snapshot("Chrome", title)
    }
}

fn private_snapshot(title: &str, url: &str) -> Snapshot {
    Snapshot {
        private: true,
        url: Some(url.into()),
        path: Some("/Users/me/secret.txt".into()),
        ..fake::snapshot("Chrome", title)
    }
}

fn excluding(apps: &[&str], domains: &[&str]) -> CaptureSettings {
    CaptureSettings {
        excluded_apps: apps.iter().map(|a| a.to_string()).collect(),
        excluded_domains: domains.iter().map(|d| d.to_string()).collect(),
        ..CaptureSettings::default()
    }
}

async fn rows(db: &Db) -> Vec<ActivityRow> {
    activities::list_for_range(db, 0, Ms::MAX).await.unwrap()
}

#[tokio::test]
async fn a_private_window_is_stored_as_the_app_alone() {
    let f = fixture().await;
    f.source.set_snapshot(Some(private_snapshot(
        "Bank login",
        "https://bank.example.com/",
    )));

    f.tracker.tick(T).await.unwrap();

    let rows = rows(&f.db).await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.app, "Chrome");
    assert_eq!(row.bundle_id.as_deref(), Some("test.chrome"));
    assert_eq!(row.title, "Private");
    assert_eq!((row.url.as_deref(), row.path.as_deref()), (None, None));
    assert!(row.private);
}

#[tokio::test]
async fn consecutive_private_windows_stay_one_row() {
    let f = fixture().await;
    f.source
        .set_snapshot(Some(private_snapshot("First", "https://a.example.com/")));
    f.tracker.tick(T).await.unwrap();

    f.source
        .set_snapshot(Some(private_snapshot("Second", "https://b.example.com/")));
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    assert_eq!(spans(&f.db).await, [title("Private", T, None)]);
}

#[tokio::test]
async fn an_excluded_app_closes_the_current_row_and_records_nothing() {
    let f = fixture().await;
    f.source.show("Code", "a.rs");
    f.tracker.tick(T).await.unwrap();

    f.tracker.set_capture(excluding(&["test.code"], &[]));
    f.tracker.tick(T + 5 * SECOND).await.unwrap();
    f.tracker.tick(T + 10 * SECOND).await.unwrap();

    assert_eq!(spans(&f.db).await, [title("a.rs", T, Some(T + 5 * SECOND))]);
    assert_eq!(f.tracker.status().current, None);
}

#[tokio::test]
async fn an_excluded_app_as_the_first_window_inserts_nothing() {
    let f = fixture().await;
    f.tracker.set_capture(excluding(&["test.code"], &[]));
    f.source.show("Code", "a.rs");

    f.tracker.tick(T).await.unwrap();

    assert!(spans(&f.db).await.is_empty());
}

#[tokio::test]
async fn recording_resumes_with_a_new_row_when_a_non_excluded_app_is_in_front() {
    let f = fixture().await;
    f.tracker.set_capture(excluding(&["test.code"], &[]));
    f.source.show("Mail", "Inbox");
    f.tracker.tick(T).await.unwrap();
    f.source.show("Code", "a.rs");
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    f.source.show("Mail", "Inbox");
    f.tracker.tick(T + 10 * SECOND).await.unwrap();

    assert_eq!(
        spans(&f.db).await,
        [
            title("Inbox", T, Some(T + 5 * SECOND)),
            title("Inbox", T + 10 * SECOND, None)
        ]
    );
}

#[tokio::test]
async fn an_excluded_domain_and_its_subdomains_are_not_recorded_but_lookalikes_are() {
    for (url, recorded) in [
        ("https://bank.example.com/x", false),
        ("https://login.bank.example.com/x", false),
        ("https://notbank.example.com", true),
        ("https://example.com/bank.example.com", true),
    ] {
        let f = fixture().await;
        f.tracker.set_capture(excluding(&[], &["bank.example.com"]));
        f.source
            .set_snapshot(Some(browser_snapshot("Page", Some(url))));

        f.tracker.tick(T).await.unwrap();

        let expected = if recorded {
            vec![title("Page", T, None)]
        } else {
            vec![]
        };
        assert_eq!(spans(&f.db).await, expected, "{url}");
    }
}

#[tokio::test]
async fn urls_are_stored_unless_capturing_them_is_off() {
    for (capture_urls, expected) in [(true, Some("https://example.com/a")), (false, None)] {
        let f = fixture().await;
        f.tracker.set_capture(CaptureSettings {
            capture_urls,
            ..CaptureSettings::default()
        });
        f.source.set_snapshot(Some(browser_snapshot(
            "Page",
            Some("https://example.com/a"),
        )));

        f.tracker.tick(T).await.unwrap();

        assert_eq!(
            rows(&f.db).await[0].url.as_deref(),
            expected,
            "{capture_urls}"
        );
    }
}

#[tokio::test]
async fn document_paths_are_stored_unless_capturing_them_is_off() {
    for (capture_documents, expected) in [(true, Some("/Users/me/a.txt")), (false, None)] {
        let f = fixture().await;
        f.tracker.set_capture(CaptureSettings {
            capture_documents,
            ..CaptureSettings::default()
        });
        f.source.set_snapshot(Some(Snapshot {
            path: Some("/Users/me/a.txt".into()),
            ..fake::snapshot("Pages", "a.txt")
        }));

        f.tracker.tick(T).await.unwrap();

        assert_eq!(
            rows(&f.db).await[0].path.as_deref(),
            expected,
            "{capture_documents}"
        );
    }
}

#[tokio::test]
async fn a_new_address_in_the_same_window_title_starts_a_new_row() {
    let f = fixture().await;
    f.source.set_snapshot(Some(browser_snapshot(
        "Docs",
        Some("https://example.com/a"),
    )));
    f.tracker.tick(T).await.unwrap();

    f.source.set_snapshot(Some(browser_snapshot(
        "Docs",
        Some("https://example.com/b"),
    )));
    f.tracker.tick(T + 5 * SECOND).await.unwrap();

    let rows = rows(&f.db).await;
    assert_eq!(
        rows.iter()
            .map(|r| (r.start, r.end, r.url.as_deref()))
            .collect::<Vec<_>>(),
        [
            (T, Some(T + 5 * SECOND), Some("https://example.com/a")),
            (T + 5 * SECOND, None, Some("https://example.com/b"))
        ]
    );
}
