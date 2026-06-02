// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cross-thread channel types for the GUI Form Runtime Engine (Phase 6).
//!
//! These types flow between the **UI thread** (egui) and the **interpreter
//! thread** (the COBOL event loop):
//!
//! ```text
//! UI thread                          Interpreter thread
//! ─────────────────────────────      ──────────────────────────────────────
//! FormRuntime.event_tx  ──────────►  Interpreter.event_rx
//!                                        COBOL-WAIT-EVENT blocks here
//!
//! FormRuntime.state_rx  ◄──────────  Interpreter.state_tx
//!                                        COBOL-SET-PROPERTY writes here
//!
//! FormRuntime.display_rx ◄─────────  Interpreter.display_tx
//!                                        DISPLAY statement writes here
//! ```

// ── UI → Interpreter ──────────────────────────────────────────────────────────

/// An event produced by user interaction in the running form window.
///
/// The interpreter thread blocks in `COBOL-WAIT-EVENT` until one of these
/// arrives.  It then populates `COBOL-CONTROL-ID` and `COBOL-EVENT-ID`
/// from this struct and returns, letting the COBOL event loop dispatch.
#[derive(Debug, Clone)]
pub struct FormEvent {
    /// The COBOL control ID (e.g. `"BTN-OK"`).
    pub ctrl_id:  String,
    /// The event name (e.g. `"Click"`, `"Change"`, `"GotFocus"`, `"LostFocus"`).
    pub event_id: String,
}

impl FormEvent {
    pub fn new(ctrl_id: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self { ctrl_id: ctrl_id.into(), event_id: event_id.into() }
    }

    /// Convenience: a `"Click"` event on `ctrl_id`.
    pub fn click(ctrl_id: impl Into<String>) -> Self {
        Self::new(ctrl_id, "Click")
    }

    /// Convenience: a `"Change"` event carrying a new value.
    pub fn change(ctrl_id: impl Into<String>, _new_value: impl Into<String>) -> Self {
        Self::new(ctrl_id, "Change")
    }

    /// Sentinel sent by the UI when the form window is closed, so the
    /// interpreter can see `COBOL-QUIT = 1` and exit cleanly.
    pub fn quit() -> Self {
        Self::new("__QUIT__", "Quit")
    }
}

// ── Interpreter → UI ──────────────────────────────────────────────────────────

/// A property change produced by the interpreter (via `COBOL-SET-PROPERTY`).
///
/// The UI thread reads these each frame and updates its local control-state
/// map, so the form window reflects COBOL-driven mutations immediately.
#[derive(Debug, Clone)]
pub struct StateUpdate {
    /// The COBOL control ID.
    pub ctrl_id:  String,
    /// The property name (e.g. `"Caption"`, `"Text"`, `"Enabled"`, `"Visible"`).
    pub prop:     String,
    /// The new value as a string (booleans: `"0"`/`"1"`).
    pub value:    String,
}

impl StateUpdate {
    pub fn new(
        ctrl_id: impl Into<String>,
        prop:    impl Into<String>,
        value:   impl Into<String>,
    ) -> Self {
        Self {
            ctrl_id: ctrl_id.into(),
            prop:    prop.into(),
            value:   value.into(),
        }
    }
}
