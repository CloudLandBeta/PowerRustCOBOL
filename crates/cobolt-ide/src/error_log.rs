// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Every error the developer is shown, on its way to the IDE console
//! (operator, 2026-08-09).
//!
//! **The rule: an error shown on screen is also written to the Output panel.**
//! A modal is dismissed and an inline validation message clears on the next
//! keystroke — either way the text is gone, and with it the only description of
//! what went wrong. The console is the record: it survives the dialog, it can be
//! scrolled back through, and it can be copied into a bug report. An error the
//! developer saw but can no longer produce is a support conversation nobody can
//! finish.
//!
//! # Why a shared buffer rather than a parameter
//!
//! The panels that raise these errors do not own the Output panel and cannot
//! reach it: `PropertiesPanel`, `IndexedGrid` and the modals are handed the
//! state they edit, not the IDE. Threading an output handle through every one of
//! them would touch far more code than the rule is worth, and would still be
//! easy to forget at the next call site. Instead the error is recorded here as
//! it is raised, and [`CoboltApp`](crate::app) drains it into the console once a
//! frame. Same thread-local shape as [`crate::theme::set_active`], for the same
//! reason.
//!
//! # The one discipline that matters
//!
//! Call [`record`] where the error is **assigned**, never where it is
//! **rendered**. An immediate-mode UI repaints its error label sixty times a
//! second; recording at the label would file sixty identical lines a second.
//! [`drain`] collapses consecutive duplicates as a backstop, but the backstop is
//! not the design.

use std::cell::RefCell;

thread_local! {
    static PENDING: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// How many unread messages the buffer will hold.
///
/// A bounded buffer because this must never become the reason the IDE runs out
/// of memory: a repainting loop that files an error every frame would otherwise
/// grow it without limit. Once full the OLDEST are dropped — a burst of errors
/// is nearly always one root cause repeating, and the newest lines are the ones
/// describing the state the developer is actually in.
const MAX_PENDING: usize = 256;

/// Record an error that is being shown to the developer.
///
/// Blank messages are ignored: some call sites clear an error by assigning an
/// empty string, and "something went wrong, but not really" is worse than
/// silence in a log.
pub fn record(message: impl Into<String>) {
    let message = message.into();
    if message.trim().is_empty() {
        return;
    }
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        // Consecutive duplicates collapse: a call site inside a per-frame path
        // must not fill the console with one repeated line.
        if pending.last().map(|last| last == &message).unwrap_or(false) {
            return;
        }
        if pending.len() >= MAX_PENDING {
            pending.remove(0);
        }
        pending.push(message);
    });
}

/// Take everything recorded since the last call. Called once a frame by the
/// IDE, which writes each line to the Output panel.
pub fn drain() -> Vec<String> {
    PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recording puts a line in the console's queue, and draining hands over
    /// every line exactly once.
    #[test]
    fn a_recorded_error_is_drained_once() {
        let _ = drain(); // isolate from other tests on this thread

        record("could not save the form");
        record("the build was refused");
        let first = drain();
        assert_eq!(first, vec!["could not save the form", "the build was refused"]);
        assert!(
            drain().is_empty(),
            "a drained error must not be reported twice"
        );
        println!("drained {} error(s), second drain empty", first.len());
    }

    /// The backstop against a call site that ended up inside a repaint path.
    #[test]
    fn consecutive_duplicates_collapse() {
        let _ = drain();

        for _ in 0..60 {
            record("invalid width");
        }
        let drained = drain();
        assert_eq!(
            drained,
            vec!["invalid width"],
            "60 frames of one error must not be 60 console lines"
        );

        // Distinct errors are all kept, and the same text recurring LATER is a
        // genuine second occurrence rather than a repaint.
        record("a");
        record("b");
        record("a");
        assert_eq!(drain(), vec!["a", "b", "a"]);
        println!("60 repeats collapsed to 1; a/b/a preserved");
    }

    /// **The rule itself, checked against the real panels.**
    ///
    /// Each of these types owns an error the developer sees and cannot reach
    /// the Output panel to report it. Setting one through its own setter must
    /// leave a line in the queue — that is the whole contract, and this is the
    /// test that fails if a future setter forgets it.
    #[test]
    fn setting_a_panel_error_records_it_for_the_console() {
        let _ = drain();

        let mut dialog = crate::panels::indexed_new_dialog::NewIndexedDialog::new();
        dialog.set_raw_error("bad record description");

        let mut grid = crate::panels::indexed_grid::IndexedGridPanel::new();
        grid.set_error("key not found");

        let recorded = drain();
        assert_eq!(
            recorded,
            vec!["bad record description", "key not found"],
            "a panel error that never reaches the console leaves no trace"
        );
        // …and the error is still on screen, not merely logged.
        assert_eq!(dialog.raw_error.as_deref(), Some("bad record description"));
        assert_eq!(grid.error.as_deref(), Some("key not found"));
        println!("panel errors recorded for the console: {recorded:?}");
    }

    /// Clearing an error by assigning an empty string must not file a blank
    /// console line.
    #[test]
    fn blank_messages_are_not_recorded() {
        let _ = drain();
        record("");
        record("   \n ");
        assert!(drain().is_empty());
    }

    /// The buffer is bounded, and it is the OLDEST that go: the newest lines
    /// describe the state the developer is actually looking at.
    #[test]
    fn the_buffer_is_bounded_and_keeps_the_newest() {
        let _ = drain();
        for i in 0..(MAX_PENDING + 50) {
            record(format!("error {i}"));
        }
        let drained = drain();
        assert_eq!(drained.len(), MAX_PENDING);
        assert_eq!(drained.first().unwrap(), "error 50", "oldest were dropped");
        assert_eq!(
            drained.last().unwrap(),
            &format!("error {}", MAX_PENDING + 49)
        );
        println!(
            "bounded at {MAX_PENDING}: kept {}..={}",
            drained.first().unwrap(),
            drained.last().unwrap()
        );
    }
}
