// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The web test crate (WASM-015 D4): what the browser, backend and module runners share. It holds no
//! product code (`publish = false`); its tests print one summary line, `→ N passed, M failed`, and skip,
//! by name, when the machine lacks what they drive.

use std::path::PathBuf;

/// The kinds of machine requirement a test can have, each with the variable that turns its absence from
/// a skip into a failure (so CI that must run it cannot skip it silently).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// The system web view (the Preview helper): `PRC_REQUIRE_WEBVIEW=1`.
    WebView,
    /// The browser end to end (a built player, a served bundle): `PRC_WEB_E2E=1`.
    WebE2e,
}

impl Need {
    fn variable(self) -> &'static str {
        match self {
            Need::WebView => "PRC_REQUIRE_WEBVIEW",
            Need::WebE2e => "PRC_WEB_E2E",
        }
    }
}

/// Decide whether `test` runs. `available` says whether the machine has what it needs. When it does not,
/// the test prints `skipped: <what> (<test>)` and passes — unless the need's variable is `1`, when the
/// absence fails it. Returns whether to run.
pub fn gate(test: &str, need: Need, available: bool, what: &str) -> bool {
    gate_with(test, need, available, what, std::env::var(need.variable()).ok().as_deref())
}

/// [`gate`] with the variable's value given, so a test can pin both rules without touching the environment.
pub fn gate_with(test: &str, need: Need, available: bool, what: &str, required: Option<&str>) -> bool {
    let run = decide(test, need, available, what, required);
    if !run {
        println!("skipped: {what} ({test})");
    }
    run
}

/// The rule of [`gate_with`] without its `skipped:` line, so the helper's own test prints no skip that the
/// build workflow, which fails on any skip line, would take for a real one.
fn decide(test: &str, need: Need, available: bool, what: &str, required: Option<&str>) -> bool {
    if available {
        return true;
    }
    if required == Some("1") {
        panic!("{test}: {what} is required ({}=1) and is not available", need.variable());
    }
    false
}

/// The summary line every test of this crate ends with.
pub fn summary(passed: usize, failed: usize) -> String {
    format!("→ {passed} passed, {failed} failed")
}

/// The empty project skeleton the runners start from: `fixtures/empty-project/`.
pub fn empty_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join("empty-project")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skip helper: present runs, absent skips and passes, absent with the variable fails.
    #[test]
    fn the_gate_runs_skips_or_fails() {
        assert!(gate_with("t", Need::WebView, true, "the web view", None));
        assert!(!decide("t", Need::WebView, false, "the web view", None));
        assert!(!decide("t", Need::WebE2e, false, "a built player", Some("0")));
        let failed = std::panic::catch_unwind(|| decide("t", Need::WebE2e, false, "a built player", Some("1")));
        let msg = failed.unwrap_err().downcast::<String>().unwrap();
        assert!(msg.contains("PRC_WEB_E2E=1"), "{msg}");
        assert_eq!(summary(3, 0), "→ 3 passed, 0 failed");
        println!("{}", summary(3, 0));
    }

    #[test]
    fn the_fixture_skeleton_exists() {
        assert!(empty_project().is_dir());
    }
}

/// What each control type's tests cover (WASM-015 T8).
pub mod coverage;
