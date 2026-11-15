// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the runner needs from the project before the program starts: the values the Cargo path
//! bakes into the generated `main.rs` (`cobolt-compiler`: `generate_main_rs_for` and the `bake_*`
//! functions). The type only: T28 fills it from the project with the one function that also feeds
//! the Cargo path, so both paths start from the same values.

use serde::{Deserialize, Serialize};

/// The runner's settings. Every field has a default, so settings written by an older or newer
/// Build still read: a field this reader lacks is passed by, a field the Build lacked takes its default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunnerSettings {
    /// The application's name (`APP_NAME`).
    pub app_name: String,
    /// The application's version (`APP_VERSION`).
    pub version: String,
    /// The form the application starts on; empty for a program with no forms.
    pub main_form: String,
    /// The project's default form theme.
    pub theme: String,
    /// The window's entrance and exit effects, and whether a restore plays the entrance again.
    pub entrance_effect: String,
    pub exit_effect: String,
    pub entrance_on_restore: bool,
    /// The project's connection catalogue, as the JSON the Cargo path bakes (`{}` when none).
    pub connections_json: String,
    /// `[agents] file_memory_limit_mb`, already defaulted.
    pub file_memory_limit_mb: u64,
    /// The keyboard focus ring.
    pub focus_ring: FocusRing,
    /// The indexed-file engine (`[ide] indexed_engine`).
    pub indexed_engine: String,
    /// `[project] locale` and `[project] currency`.
    pub locale: String,
    pub currency: String,
    /// `[links] schemes`: the URL schemes the program may open.
    pub link_schemes: Vec<String>,
    /// The program calls its backend (`CALL … WITH WEBSOCKET LINKAGE`, `WS-UPLOAD`, `WS-DOWNLOAD`),
    /// so the runner starts the backend client once, before anything runs.
    pub remote: bool,
}

/// The keyboard focus ring's settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusRing {
    pub enabled: bool,
    pub color: String,
    pub pulse: bool,
}
