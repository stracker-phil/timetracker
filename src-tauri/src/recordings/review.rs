//! Reviewing suggestions: dismissals, and a recording that settles its suggestion, each one undoable change.

use super::ops::Op;
use super::{Change, Overlap, RecordingInput, Recordings};
use crate::db::dismissed::Dismissal;
use crate::db::recordings::Recording;
use crate::error::AppResult;

/// "Approve suggestion" for one, "Approve 3 suggestions" for several.
pub fn suggestions_label(verb: &str, count: usize) -> String {
    match count {
        1 => format!("{verb} suggestion"),
        n => format!("{verb} {n} suggestions"),
    }
}

impl Recordings<'_> {
    /// Stores each dismissal as one undoable change; returns how many.
    pub async fn record_dismissals(&self, dismissals: Vec<Dismissal>) -> AppResult<Change<usize>> {
        let mut undo = self.undo.lock().await;
        let count = dismissals.len();
        let label = suggestions_label("Dismiss", count);
        let ops = dismissals
            .into_iter()
            .map(|dismissal| Op::Dismiss { dismissal })
            .collect();
        let op = Op::Batch {
            label: label.clone(),
            ops,
        };
        let days = self.apply_and_push(&mut undo, label, op).await?;
        Ok(Change { value: count, days })
    }
    /// `input` as a new recording that also dismisses what is left of its suggestion.
    pub async fn create_settling(
        &self,
        input: RecordingInput,
        overlap: Overlap,
        dismissal: Dismissal,
    ) -> AppResult<Change<Recording>> {
        self.create_with(input, overlap, vec![Op::Dismiss { dismissal }])
            .await
    }
}
