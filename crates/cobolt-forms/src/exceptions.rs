// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a form shows when one of its handlers fails and nothing catches the failure.
//!
//! Inside a form the failure does not end the run (operator ruling, 2026-09-06): the handler stops,
//! the form keeps the failure as `LastException`, and either its `onUnhandledException` runs or the
//! window shows a critical notice. The interpreter and the desktop runner both do this, in these
//! words, so a program built on either path shows the same notice (DESKTOP-WASM-001 R27).

/// The form event that catches a failure nothing else caught.
pub const UNHANDLED_EXCEPTION_EVENT: &str = "onUnhandledException";

/// The pseudo-property of the form object that asks its window for the critical notice. It is
/// written, never stored: the window raises a notice that waits for its close button.
pub const CRITICAL_EXCEPTION_PROP: &str = "_CriticalException";

/// The failure of a `super::` that names no live form: the main form, a form opened on its own, or
/// one whose opener has closed.
pub const SUPER_IS_NULL: &str = "super is NULL — this form has no parent (the main form, or its opener already closed)";

/// The critical notice for the failure `detail`.
pub fn critical_exception_text(detail: &str) -> String {
    format!(
        "A critical exception has occurred: {detail}. Implement the event handler {UNHANDLED_EXCEPTION_EVENT} to get \
         better control over the exception."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_names_the_failure_and_the_event_that_catches_it() {
        let text = critical_exception_text(SUPER_IS_NULL);
        let checks = [
            ("starts with the interpreter's opening", text.starts_with("A critical exception has occurred: super is NULL")),
            ("names the event", text.contains(" onUnhandledException ")),
            ("ends with the advice", text.ends_with("to get better control over the exception.")),
            ("one space between words", !text.contains("  ")),
        ];
        let passed = checks.iter().filter(|(_, ok)| *ok).count();
        println!("== the_notice_names_the_failure_and_the_event_that_catches_it (DESKTOP-WASM-001 R27) ==");
        println!("notice of {} characters; {passed} of {} checks passed", text.chars().count(), checks.len());
        for (name, ok) in checks {
            assert!(ok, "{name}: {text}");
        }
    }
}
