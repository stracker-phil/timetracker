//! A recording and its spans as stored and sent, and the one check of a recording's own spans.

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::db::new_id;
use crate::db::time::Ms;
use crate::error::{AppError, AppResult};

/// A closed stretch of time, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Span {
    pub start: Ms,
    pub end: Ms,
}

/// One stretch of a recording's time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingSpan {
    pub id: String,
    pub start: Ms,
    /// `None` only for the running timer's span.
    pub end: Option<Ms>,
}

impl RecordingSpan {
    pub fn new(start: Ms, end: Option<Ms>) -> Self {
        Self {
            id: new_id(),
            start,
            end,
        }
    }

    /// Its length; an open span counts until `now`.
    pub fn duration(&self, now: Ms) -> Ms {
        (self.end.unwrap_or(now) - self.start).max(0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Recording {
    pub id: String,
    pub project_id: String,
    /// One line (ADR 0018).
    pub note: String,
    pub tag_id: Option<String>,
    pub created_at: Ms,
    pub updated_at: Ms,
    /// At least one, by start, never overlapping each other.
    #[sqlx(skip)]
    pub spans: Vec<RecordingSpan>,
}

impl Recording {
    /// The start of its first span.
    pub fn start(&self) -> Ms {
        self.spans.first().map_or(0, |s| s.start)
    }

    /// The end of its last span; `None` while it runs.
    pub fn end(&self) -> Option<Ms> {
        self.spans.last().and_then(|s| s.end)
    }

    pub fn is_running(&self) -> bool {
        self.spans.iter().any(|s| s.end.is_none())
    }

    /// The open span of the running timer.
    pub fn open_span(&self) -> Option<&RecordingSpan> {
        self.spans.iter().find(|s| s.end.is_none())
    }

    /// The sum of its spans; an open one counts until `now`.
    pub fn duration(&self, now: Ms) -> Ms {
        self.spans.iter().map(|s| s.duration(now)).sum()
    }

    pub fn span(&self, span_id: &str) -> AppResult<&RecordingSpan> {
        self.spans
            .iter()
            .find(|s| s.id == span_id)
            .ok_or_else(|| AppError::NotFound(format!("No span {span_id}")))
    }
}

/// The caller picks the id, so an undo can restore a deleted recording under its old id. Spans get new ids.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRecording {
    pub id: String,
    pub project_id: String,
    pub spans: Vec<(Ms, Option<Ms>)>,
    pub note: String,
    pub tag_id: Option<String>,
}

/// Absent fields stay as they are; `spans` replaces every span, `tagId: null` removes the tag.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RecordingPatch {
    #[ts(optional)]
    pub project_id: Option<String>,
    #[ts(optional)]
    pub spans: Option<Vec<Span>>,
    #[ts(optional)]
    pub note: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub tag_id: Option<Option<String>>,
}

/// A field that is present, even as `null`, is `Some`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// `spans` as the spans of `current`, by start: the n-th keeps the id of the n-th current span.
pub fn respanned(current: &[RecordingSpan], spans: &[Span]) -> Vec<RecordingSpan> {
    let mut sorted = spans.to_vec();
    sorted.sort_by_key(|s| s.start);
    sorted
        .iter()
        .enumerate()
        .map(|(i, s)| RecordingSpan {
            id: current.get(i).map_or_else(new_id, |c| c.id.clone()),
            start: s.start,
            end: Some(s.end),
        })
        .collect()
}

/// At least one span, each ending after it starts, by start without overlaps (`spans`); only the last may
/// be open. The one check of a recording's own spans, for the repository and the service.
pub fn check_spans(spans: &[RecordingSpan]) -> AppResult<()> {
    if spans.is_empty() {
        return Err(AppError::InvalidInput(
            "A recording needs at least one span".into(),
        ));
    }
    for span in spans {
        if span.end.is_some_and(|end| end <= span.start) {
            return Err(AppError::InvalidInput(
                "A recording must end after it starts".into(),
            ));
        }
    }
    for pair in spans.windows(2) {
        match pair[0].end {
            Some(end) if end <= pair[1].start => {}
            _ => {
                return Err(AppError::InvalidField {
                    field: "spans".into(),
                    message: "must not overlap each other".into(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn span(start: Ms, end: Ms) -> Span {
        Span { start, end }
    }

    #[test]
    fn spans_must_be_non_empty_ordered_and_closed_except_the_last() {
        fn spans(times: &[(Ms, Option<Ms>)]) -> Vec<RecordingSpan> {
            times
                .iter()
                .map(|&(s, e)| RecordingSpan::new(s, e))
                .collect()
        }
        let valid: [&[(Ms, Option<Ms>)]; 4] = [
            &[(100, Some(200))],
            &[(100, Some(200)), (200, Some(300))],
            &[(100, Some(200)), (300, None)],
            &[(100, None)],
        ];
        for case in valid {
            assert!(check_spans(&spans(case)).is_ok(), "{case:?}");
        }
        let invalid: [&[(Ms, Option<Ms>)]; 7] = [
            &[],
            &[(100, Some(100))],
            &[(100, Some(50))],
            &[(100, Some(300)), (200, Some(400))],
            &[(100, Some(300)), (150, Some(200))],
            &[(100, None), (200, Some(300))],
            &[(100, None), (200, None)],
        ];
        for case in invalid {
            let err = check_spans(&spans(case)).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{case:?}");
        }
    }

    #[test]
    fn patch_reads_spans_and_leaves_them_absent_by_default() {
        let set: RecordingPatch =
            serde_json::from_str(r#"{"spans": [{"start": 1, "end": 2}, {"start": 3, "end": 4}]}"#)
                .unwrap();
        assert_eq!(set.spans, Some(vec![span(1, 2), span(3, 4)]));
        let absent: RecordingPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(absent, RecordingPatch::default());
        assert_eq!(absent.spans, None);
    }

    #[test]
    fn patch_distinguishes_a_null_tag_from_an_absent_one() {
        let null: RecordingPatch = serde_json::from_str(r#"{"tagId": null}"#).unwrap();
        assert_eq!(null.tag_id, Some(None));
        let set: RecordingPatch = serde_json::from_str(r#"{"tagId": "t"}"#).unwrap();
        assert_eq!(set.tag_id, Some(Some("t".to_string())));
        let absent: RecordingPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.tag_id, None);
    }
}
