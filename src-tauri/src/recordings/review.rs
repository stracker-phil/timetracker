//! Reviewing suggestions: what settles them (dismissals, a stored suggestion's deletion) as one undoable
//! change.

use super::ops::Op;
use super::{Change, Recordings};
use crate::error::AppResult;

/// "Approve suggestion" for one, "Approve 3 suggestions" for several.
pub fn suggestions_label(verb: &str, count: usize) -> String {
    match count {
        1 => format!("{verb} suggestion"),
        n => format!("{verb} {n} suggestions"),
    }
}

impl Recordings<'_> {
    /// Writes the ops that dismiss each suggestion as one undoable change; returns how many suggestions.
    pub async fn dismiss_with(&self, suggestions: Vec<Vec<Op>>) -> AppResult<Change<usize>> {
        let mut undo = self.undo.lock().await;
        let count = suggestions.len();
        let label = suggestions_label("Dismiss", count);
        let ops = suggestions.into_iter().flatten().collect();
        let op = Op::Batch {
            label: label.clone(),
            ops,
        };
        let days = self.apply_and_push(&mut undo, label, op).await?;
        Ok(Change { value: count, days })
    }
}
