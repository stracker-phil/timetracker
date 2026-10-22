//! "What did you do?" after a long away span: the tracker reports each away span it closes, and one long
//! enough opens the `away` window prefilled with its snapped times.

pub mod commands;
mod window;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;
use ts_rs::TS;

use crate::db::away::{AwayReason, AwayRow};
use crate::db::recordings::Recording;
use crate::db::time::{Day, Ms};
use crate::error::{AppError, AppResult};
use crate::events::AWAY_PROMPT_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::recordings::{Change, Overlap, RecordingInput, Recordings, Span, snap};
use crate::state::AppState;
use crate::store::settings::TrackerSettings;
use crate::tracker::Tracker;

/// An answer must lie within this many days, today included.
const ANSWER_DAYS: u32 = 7;
const MINUTE_MS: Ms = 60_000;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct AwayPrompt {
    pub span: AwayRow,
    /// The span's end; prompts are only made for closed spans.
    pub ended_at: Ms,
    pub away_ms: i64,
    /// The span snapped as a recording would be.
    pub suggested: Span,
}

/// The open prompt; one at a time.
#[derive(Debug, Default)]
pub struct AwayPromptState {
    current: std::sync::Mutex<Option<AwayPrompt>>,
}

impl AwayPromptState {
    pub fn current(&self) -> Option<AwayPrompt> {
        self.current.lock_unpoisoned().clone()
    }

    /// Replaces any open prompt.
    pub fn set(&self, prompt: AwayPrompt) {
        *self.current.lock_unpoisoned() = Some(prompt);
    }

    pub fn clear(&self) {
        *self.current.lock_unpoisoned() = None;
    }
}

/// A closed span that is long enough, while the prompt is on. A pause never asks: the user chose it.
pub fn should_prompt(span: &AwayRow, settings: &TrackerSettings) -> bool {
    let Some(end) = span.end else {
        return false;
    };
    settings.away_prompt_enabled
        && span.reason != AwayReason::Paused
        && end - span.start >= Ms::from(settings.away_prompt_minutes) * MINUTE_MS
}

/// Both ends snapped; at least one snap step long.
pub fn suggestion(span: &AwayRow, snap_minutes: u32) -> Span {
    let step = Ms::from(snap_minutes.max(1)) * MINUTE_MS;
    let start = snap(span.start, snap_minutes);
    let end = snap(span.end.unwrap_or(span.start), snap_minutes);
    Span {
        start,
        end: if end > start { end } else { start + step },
    }
}

/// Records the answer to the open prompt and closes it. An `overlap` keeps the prompt, so the user can
/// change the times.
pub async fn answer(
    prompts: &AwayPromptState,
    service: &Recordings<'_>,
    input: RecordingInput,
    now: Ms,
) -> AppResult<Change<Recording>> {
    if prompts.current().is_none() {
        return Err(AppError::NotFound("No away prompt is open".into()));
    }
    let first_day = (1..ANSWER_DAYS).fold(Day::of(now), |day, _| day.prev());
    let outside = |s: &Span| s.start < first_day.bounds().0 || s.end > now + MINUTE_MS;
    if input.spans.iter().any(outside) {
        return Err(AppError::InvalidField {
            field: "start".into(),
            message: format!("must lie within the last {ANSWER_DAYS} days"),
        });
    }
    let change = service.create(input, Overlap::Cancel).await?;
    prompts.clear();
    Ok(change)
}

/// The prompt for a span the tracker just closed, when it asks for one.
pub fn prompt_for(
    span: AwayRow,
    tracker: &TrackerSettings,
    snap_minutes: u32,
) -> Option<AwayPrompt> {
    let ended_at = span.end?;
    should_prompt(&span, tracker).then(|| AwayPrompt {
        suggested: suggestion(&span, snap_minutes),
        ended_at,
        away_ms: ended_at - span.start,
        span,
    })
}

/// Prompts for every long away span the tracker closes while the app runs; spans closed by startup
/// recovery never reach it.
pub fn watch(app: &AppHandle, tracker: &Tracker) {
    let mut ended = tracker.subscribe_away();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let span = match ended.recv().await {
                Ok(span) => span,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let settings = app.state::<AppState>().settings.get();
            let snap_minutes = u32::from(settings.timeline.snap_minutes);
            if let Some(prompt) = prompt_for(span, &settings.tracker, snap_minutes) {
                show(&app, prompt);
            }
        }
    });
}

/// Stores the prompt (replacing an open one), refills an open window and brings it up.
fn show(app: &AppHandle, prompt: AwayPrompt) {
    app.state::<AwayPromptState>().set(prompt.clone());
    if window::is_open(app)
        && let Err(e) = app.emit_to(window::LABEL, AWAY_PROMPT_CHANGED_EVENT, &prompt)
    {
        log::warn!("Couldn't send the new away span to its window: {e}");
    }
    window::open(app);
}

/// Closes the prompt without recording.
pub fn skip(app: &AppHandle) {
    app.state::<AwayPromptState>().clear();
    window::close(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::error::ErrorKind;
    use crate::recordings::undo::UndoStack;
    use crate::test_support::project;

    /// A multiple of 15 minutes in UTC, so snapping is the same in every time zone in use.
    const T: Ms = 1_800_000_000_000;
    const MIN: Ms = 60_000;
    const HOUR: Ms = 60 * MIN;

    fn span(start: Ms, end: Option<Ms>, reason: AwayReason) -> AwayRow {
        AwayRow {
            id: 1,
            start,
            end,
            reason,
        }
    }

    fn prompt() -> AwayPrompt {
        let span = span(T - 2 * HOUR, Some(T - HOUR), AwayReason::Idle);
        AwayPrompt {
            suggested: suggestion(&span, 5),
            ended_at: T - HOUR,
            away_ms: HOUR,
            span,
        }
    }

    #[test]
    fn a_closed_span_of_the_minimum_length_prompts() {
        let settings = TrackerSettings::default();
        let minimum = Ms::from(settings.away_prompt_minutes) * MIN;
        for (reason, length, expected) in [
            (AwayReason::Idle, minimum, true),
            (AwayReason::Idle, minimum - 1, false),
            (AwayReason::Idle, 3 * HOUR, true),
            (AwayReason::Lock, minimum, true),
            (AwayReason::Sleep, minimum, true),
            (AwayReason::Sleep, MIN, false),
        ] {
            assert_eq!(
                should_prompt(&span(T, Some(T + length), reason), &settings),
                expected,
                "{reason:?} {length}"
            );
        }
    }

    #[test]
    fn a_pause_never_prompts() {
        assert!(!should_prompt(
            &span(T, Some(T + 5 * HOUR), AwayReason::Paused),
            &TrackerSettings::default()
        ));
    }

    #[test]
    fn an_open_span_never_prompts() {
        assert!(!should_prompt(
            &span(T, None, AwayReason::Idle),
            &TrackerSettings::default()
        ));
    }

    #[test]
    fn nothing_prompts_while_the_prompt_is_off() {
        let settings = TrackerSettings {
            away_prompt_enabled: false,
            ..TrackerSettings::default()
        };
        assert!(!should_prompt(
            &span(T, Some(T + 5 * HOUR), AwayReason::Idle),
            &settings
        ));
    }

    #[test]
    fn the_minimum_length_follows_the_setting() {
        let settings = TrackerSettings {
            away_prompt_minutes: 30,
            ..TrackerSettings::default()
        };
        for (length, expected) in [(29 * MIN, false), (30 * MIN, true)] {
            assert_eq!(
                should_prompt(&span(T, Some(T + length), AwayReason::Idle), &settings),
                expected,
                "{length}"
            );
        }
    }

    #[test]
    fn a_long_closed_span_becomes_a_prompt_with_its_suggestion() {
        let away = span(T + 4 * MIN, Some(T + 40 * MIN), AwayReason::Idle);
        assert_eq!(
            prompt_for(away.clone(), &TrackerSettings::default(), 15),
            Some(AwayPrompt {
                span: away,
                ended_at: T + 40 * MIN,
                away_ms: 36 * MIN,
                suggested: Span {
                    start: T,
                    end: T + 45 * MIN
                },
            })
        );
    }

    #[test]
    fn a_prompt_reports_when_the_span_ended_and_how_long_it_lasted() {
        for (start, length) in [(T, 30 * MIN), (T + 7 * MIN, 95 * MIN), (T - HOUR, 3 * HOUR)] {
            let away = span(start, Some(start + length), AwayReason::Lock);

            let prompt = prompt_for(away, &TrackerSettings::default(), 15).unwrap();

            assert_eq!(prompt.ended_at, start + length, "{start} {length}");
            assert_eq!(prompt.away_ms, length, "{start} {length}");
        }
    }

    #[test]
    fn spans_that_should_not_prompt_make_no_prompt() {
        let settings = TrackerSettings::default();
        for away in [
            span(T, Some(T + MIN), AwayReason::Idle),
            span(T, Some(T + HOUR), AwayReason::Paused),
            span(T, None, AwayReason::Idle),
        ] {
            assert_eq!(prompt_for(away.clone(), &settings, 15), None, "{away:?}");
        }
    }

    #[test]
    fn the_suggestion_snaps_both_ends() {
        let away = span(T + 4 * MIN, Some(T + 40 * MIN), AwayReason::Idle);
        assert_eq!(
            suggestion(&away, 15),
            Span {
                start: T,
                end: T + 45 * MIN
            }
        );
    }

    #[test]
    fn a_span_shorter_than_a_snap_step_suggests_one_step() {
        let away = span(T + MIN, Some(T + 3 * MIN), AwayReason::Idle);
        assert_eq!(
            suggestion(&away, 15),
            Span {
                start: T,
                end: T + 15 * MIN
            }
        );
    }

    #[test]
    fn a_second_prompt_replaces_the_first_and_clear_removes_it() {
        let state = AwayPromptState::default();
        assert_eq!(state.current(), None);
        let first = prompt();
        let second = AwayPrompt {
            span: span(T, Some(T + HOUR), AwayReason::Lock),
            ..prompt()
        };

        state.set(first);
        state.set(second.clone());
        assert_eq!(state.current(), Some(second));

        state.clear();
        assert_eq!(state.current(), None);
    }

    #[test]
    fn the_prompt_serializes_with_camel_case_keys() {
        let value = serde_json::to_value(prompt()).unwrap();
        assert_eq!(
            value["suggested"],
            serde_json::json!({"start": T - 2 * HOUR, "end": T - HOUR})
        );
        assert_eq!(value["span"]["reason"], "idle");
    }

    struct World {
        db: Db,
        undo: tokio::sync::Mutex<UndoStack>,
        project_id: String,
    }

    impl World {
        async fn new() -> Self {
            let db = Db::open_memory().await.unwrap();
            let project_id = project(&db, "Acme").await.id;
            Self {
                db,
                undo: tokio::sync::Mutex::new(UndoStack::default()),
                project_id,
            }
        }

        fn service(&self) -> Recordings<'_> {
            Recordings {
                db: &self.db,
                undo: &self.undo,
                snap_minutes: 5,
            }
        }

        fn input(&self, start: Ms, end: Ms) -> RecordingInput {
            RecordingInput {
                project_id: self.project_id.clone(),
                spans: vec![Span { start, end }],
                note: String::new(),
                tag_id: None,
            }
        }

        async fn stored(&self) -> Vec<(Ms, Option<Ms>)> {
            self.stored_spans()
                .await
                .into_iter()
                .map(|spans| (spans[0].0, spans[spans.len() - 1].1))
                .collect()
        }

        async fn stored_spans(&self) -> Vec<Vec<(Ms, Option<Ms>)>> {
            crate::db::recordings::list_for_range(&self.db, Ms::MIN, Ms::MAX)
                .await
                .unwrap()
                .into_iter()
                .map(|r| r.spans.iter().map(|s| (s.start, s.end)).collect())
                .collect()
        }
    }

    #[tokio::test]
    async fn answering_records_the_span_and_closes_the_prompt() {
        let world = World::new().await;
        let prompts = AwayPromptState::default();
        prompts.set(prompt());

        let change = answer(
            &prompts,
            &world.service(),
            world.input(T - 2 * HOUR, T - HOUR),
            T,
        )
        .await
        .unwrap();

        assert_eq!(change.value.project_id, world.project_id);
        assert_eq!(world.stored().await, [(T - 2 * HOUR, Some(T - HOUR))]);
        assert_eq!(prompts.current(), None);
    }

    #[tokio::test]
    async fn answering_without_an_open_prompt_is_not_found_and_records_nothing() {
        let world = World::new().await;

        let err = answer(
            &AwayPromptState::default(),
            &world.service(),
            world.input(T - 2 * HOUR, T - HOUR),
            T,
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(world.stored().await.is_empty());
    }

    #[tokio::test]
    async fn an_answer_outside_the_last_seven_days_or_in_the_future_is_refused() {
        let world = World::new().await;
        let prompts = AwayPromptState::default();
        prompts.set(prompt());
        let earliest = Day::of(T).bounds().0 - 6 * 24 * HOUR;

        for (name, start, end) in [
            ("before the window", earliest - HOUR, earliest + HOUR),
            ("after now", T - HOUR, T + 2 * MIN),
        ] {
            let err = answer(&prompts, &world.service(), world.input(start, end), T)
                .await
                .unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}");
            assert_eq!(
                err.field().map(|f| f.path.to_string()).as_deref(),
                Some("start"),
                "{name}"
            );
        }
        assert!(world.stored().await.is_empty());
        assert_eq!(prompts.current(), Some(prompt()));
    }

    #[tokio::test]
    async fn an_answer_with_several_spans_is_recorded_as_one_recording() {
        let world = World::new().await;
        let prompts = AwayPromptState::default();
        prompts.set(prompt());
        let input = RecordingInput {
            spans: vec![
                Span {
                    start: T - 4 * HOUR,
                    end: T - 3 * HOUR,
                },
                Span {
                    start: T - 2 * HOUR,
                    end: T - HOUR,
                },
            ],
            ..world.input(0, 0)
        };

        answer(&prompts, &world.service(), input, T).await.unwrap();

        assert_eq!(
            world.stored_spans().await,
            [vec![
                (T - 4 * HOUR, Some(T - 3 * HOUR)),
                (T - 2 * HOUR, Some(T - HOUR)),
            ]]
        );
        assert_eq!(prompts.current(), None);
    }

    #[tokio::test]
    async fn every_span_of_an_answer_must_lie_within_the_last_seven_days() {
        let world = World::new().await;
        let prompts = AwayPromptState::default();
        prompts.set(prompt());
        let earliest = Day::of(T).bounds().0 - 6 * 24 * HOUR;
        let inside = Span {
            start: T - 2 * HOUR,
            end: T - HOUR,
        };

        for (name, outside) in [
            (
                "an early span",
                Span {
                    start: earliest - 2 * HOUR,
                    end: earliest - HOUR,
                },
            ),
            (
                "a span in the future",
                Span {
                    start: T + MIN,
                    end: T + 2 * MIN,
                },
            ),
        ] {
            let input = RecordingInput {
                spans: vec![outside, inside],
                ..world.input(0, 0)
            };
            let swapped = RecordingInput {
                spans: vec![inside, outside],
                ..world.input(0, 0)
            };

            for input in [input, swapped] {
                let err = answer(&prompts, &world.service(), input, T)
                    .await
                    .unwrap_err();
                assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}");
            }
        }
        assert!(world.stored().await.is_empty());
        assert_eq!(prompts.current(), Some(prompt()));
    }

    #[tokio::test]
    async fn an_overlapping_answer_is_refused_and_keeps_the_prompt() {
        let world = World::new().await;
        world
            .service()
            .create(world.input(T - 2 * HOUR, T - HOUR), Overlap::Cancel)
            .await
            .unwrap();
        let prompts = AwayPromptState::default();
        prompts.set(prompt());

        let err = answer(
            &prompts,
            &world.service(),
            world.input(T - 90 * MIN, T - 30 * MIN),
            T,
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Overlap);
        assert_eq!(world.stored().await, [(T - 2 * HOUR, Some(T - HOUR))]);
        assert_eq!(prompts.current(), Some(prompt()));
    }

    #[tokio::test]
    async fn a_note_with_two_lines_is_refused_and_keeps_the_prompt() {
        let world = World::new().await;
        let prompts = AwayPromptState::default();
        prompts.set(prompt());
        let input = RecordingInput {
            note: "one\ntwo".into(),
            ..world.input(T - 2 * HOUR, T - HOUR)
        };

        let err = answer(&prompts, &world.service(), input, T)
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.field().map(|f| f.path.to_string()).as_deref(),
            Some("note")
        );
        assert!(world.stored().await.is_empty());
        assert_eq!(prompts.current(), Some(prompt()));
    }
}
