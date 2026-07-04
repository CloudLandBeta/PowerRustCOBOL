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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FormEvent {
    /// The COBOL control ID (e.g. `"BTN-OK"`). For repeating group members this is
    /// the base id (without #N suffix); the instance index is in `instance_index`.
    pub ctrl_id: String,
    /// The event name (e.g. `"onClick"`, `"onChange"`, `"onGotFocus"`, `"onLostFocus"`).
    pub event_id: String,
    /// 1-based instance index when the control is inside a repeating GroupBox
    /// (ControlArray). 0 means scalar (not array member).
    pub instance_index: usize,
}

impl FormEvent {
    pub fn new(ctrl_id: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self {
            ctrl_id: ctrl_id.into(),
            event_id: event_id.into(),
            instance_index: 0,
        }
    }

    pub fn with_index(mut self, idx: usize) -> Self {
        self.instance_index = idx;
        self
    }

    /// Convenience: an `"onClick"` event on `ctrl_id`.
    pub fn click(ctrl_id: impl Into<String>) -> Self {
        Self::new(ctrl_id, "onClick")
    }

    /// Convenience: an `"onChange"` event carrying a new value.
    pub fn change(ctrl_id: impl Into<String>, _new_value: impl Into<String>) -> Self {
        Self::new(ctrl_id, "onChange")
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateUpdate {
    /// The COBOL control ID. For a repeating-group (ControlArray) member this is
    /// the **base** id (no instance suffix); the instance is `instance_index`.
    pub ctrl_id: String,
    /// The property name (e.g. `"Caption"`, `"Text"`, `"Enabled"`, `"Visible"`).
    pub prop: String,
    /// The new value as a string (booleans: `"0"`/`"1"`).
    pub value: String,
    /// 1-based instance index when the control is a member of a repeating GroupBox
    /// (ControlArray), i.e. written as `Member(idx)::Prop`. 0 = scalar control.
    #[serde(default)]
    pub instance_index: usize,
}

impl StateUpdate {
    pub fn new(
        ctrl_id: impl Into<String>,
        prop: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            ctrl_id: ctrl_id.into(),
            prop: prop.into(),
            value: value.into(),
            instance_index: 0,
        }
    }

    /// Set the 1-based repeating-group instance index for an array-member write.
    pub fn with_index(mut self, idx: usize) -> Self {
        self.instance_index = idx;
        self
    }
}

// ── IPC messages for out-of-process form runner (Approach 1) ──────────────────

/// Binary messages exchanged over stdio pipes between the IDE and a
/// separate interpreter process for "Run Form".
///
/// Length-prefixed bincode encoding is used on both sides.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FormIpcMessage {
    // IDE → Runner
    Event(FormEvent),
    Input(StateUpdate),
    Quit,

    // Runner → IDE
    State(StateUpdate),
    Display(String),
    Error(String),
    /// Interpreter has exited cleanly.
    Done,
}
