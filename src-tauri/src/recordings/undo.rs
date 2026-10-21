//! The undo and redo history of recording changes, per app run.

use std::collections::VecDeque;

use serde::Serialize;
use ts_rs::TS;

use super::ops::Op;

const LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct UndoStatus {
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub label: String,
    pub op: Op,
}

#[derive(Debug, Default)]
pub struct UndoStack {
    done: VecDeque<Entry>,
    undone: Vec<Entry>,
}

impl UndoStack {
    /// A new change: undoable, and nothing is left to redo. The oldest entry goes past `LIMIT`.
    pub fn push(&mut self, entry: Entry) {
        self.undone.clear();
        self.done.push_back(entry);
        if self.done.len() > LIMIT {
            self.done.pop_front();
        }
    }

    pub fn last_done(&self) -> Option<&Entry> {
        self.done.back()
    }

    pub fn last_undone(&self) -> Option<&Entry> {
        self.undone.last()
    }

    /// Moves the last change to the redo list, once its inverse was applied.
    pub fn mark_undone(&mut self) {
        if let Some(entry) = self.done.pop_back() {
            self.undone.push(entry);
        }
    }

    /// Moves the last undone change back, once it was applied again.
    pub fn mark_redone(&mut self) {
        if let Some(entry) = self.undone.pop() {
            self.done.push_back(entry);
        }
    }

    pub fn status(&self) -> UndoStatus {
        UndoStatus {
            undo_label: self.last_done().map(|e| e.label.clone()),
            redo_label: self.last_undone().map(|e| e.label.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recordings::testing::row;

    fn entry(label: &str) -> Entry {
        Entry {
            label: label.into(),
            op: Op::Delete {
                recording: row("r", 0, 10),
            },
        }
    }

    fn labels(undo: Option<&str>, redo: Option<&str>) -> UndoStatus {
        UndoStatus {
            undo_label: undo.map(str::to_string),
            redo_label: redo.map(str::to_string),
        }
    }

    /// Undoes everything and returns the labels, newest first.
    fn drain(stack: &mut UndoStack) -> Vec<String> {
        let mut names = Vec::new();
        while let Some(entry) = stack.last_done().cloned() {
            names.push(entry.label);
            stack.mark_undone();
        }
        names
    }

    #[test]
    fn an_empty_stack_has_nothing_to_undo_or_redo() {
        let mut stack = UndoStack::default();

        assert_eq!(stack.status(), labels(None, None));
        assert!(stack.last_done().is_none());
        assert!(stack.last_undone().is_none());
        stack.mark_undone();
        stack.mark_redone();
        assert_eq!(stack.status(), labels(None, None));
    }

    #[test]
    fn the_stack_is_bounded_at_100_and_drops_the_oldest() {
        let mut stack = UndoStack::default();
        for i in 0..101 {
            stack.push(entry(&format!("op {i}")));
        }

        let undone = drain(&mut stack);

        assert_eq!(undone.len(), 100);
        assert_eq!(undone.first().map(String::as_str), Some("op 100"));
        assert_eq!(undone.last().map(String::as_str), Some("op 1"));
    }

    #[test]
    fn undoing_moves_the_last_change_to_the_redo_list_and_redoing_moves_it_back() {
        let mut stack = UndoStack::default();
        stack.push(entry("first"));
        stack.push(entry("second"));
        assert_eq!(stack.status(), labels(Some("second"), None));

        stack.mark_undone();
        assert_eq!(stack.status(), labels(Some("first"), Some("second")));

        stack.mark_undone();
        assert_eq!(stack.status(), labels(None, Some("first")));

        stack.mark_redone();
        assert_eq!(stack.status(), labels(Some("first"), Some("second")));

        stack.mark_redone();
        assert_eq!(stack.status(), labels(Some("second"), None));
    }

    #[test]
    fn the_stored_op_comes_back_unchanged() {
        let mut stack = UndoStack::default();
        let pushed = entry("first");
        stack.push(pushed.clone());

        assert_eq!(stack.last_done().map(|e| e.op.clone()), Some(pushed.op));
    }

    #[test]
    fn a_new_change_clears_the_redo_list() {
        let mut stack = UndoStack::default();
        stack.push(entry("first"));
        stack.push(entry("second"));
        stack.mark_undone();
        assert!(stack.last_undone().is_some());

        stack.push(entry("third"));

        assert_eq!(stack.status(), labels(Some("third"), None));
        assert_eq!(drain(&mut stack), ["third", "first"]);
    }

    #[test]
    fn the_status_serializes_with_camel_case_labels() {
        assert_eq!(
            serde_json::to_value(labels(Some("Move recording"), None)).unwrap(),
            serde_json::json!({"undoLabel": "Move recording", "redoLabel": null})
        );
    }
}
