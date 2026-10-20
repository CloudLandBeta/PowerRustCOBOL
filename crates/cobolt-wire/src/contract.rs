// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The interface description: what a backend exports (WASM-003 R4,
//! WASM-014 R1).
//!
//! It is written when the backend is built, read by the frontend project so
//! its `CALL … WITH WEBSOCKET LINKAGE` can be checked at Check time, embedded in
//! the backend so the server knows what it serves, and compared with the last
//! released one so a breaking change cannot ship by accident (WASM-014 R5).

use serde::{Deserialize, Serialize};

use crate::layout::Item;

/// The `format` word of a description file.
pub const FORMAT: &str = "prc-interface";

/// Where a project keeps its backend's interface description, relative to the
/// project folder (the backend is the `backend/` folder of a Web or Mobile
/// project, WASM-001 D13). Written by the backend's Build, imported by the
/// frontend's Check.
pub const PROJECT_PATH: &str = "backend/contract/interface.json";

/// The version of the description file's own shape.
pub const FORMAT_VERSION: u32 = 1;

/// The wire protocol this build speaks (WASM-014 R3).
pub const PROTOCOL: u32 = 1;

/// The oldest wire protocol this build still serves.
pub const PROTOCOL_MIN: u32 = 1;

/// Which way a parameter's value travels. A callee does not declare how it was
/// called (`BY VALUE` / `BY REFERENCE` is the caller's choice), so the
/// description carries what the developer exported it for; `InOut` is the
/// conservative default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    In,
    Out,
    #[default]
    InOut,
}

/// One parameter of an exported program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    /// The parameter's name in the callee's LINKAGE SECTION (documentation; the
    /// wire matches by position).
    pub name: String,
    #[serde(default)]
    pub direction: Direction,
    pub layout: Item,
}

/// A deprecation notice (WASM-014 R6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deprecation {
    pub message: String,
    /// The contract version in which the program goes away.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removal: Option<String>,
}

/// One exported program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramSig {
    /// The exported name, upper-cased.
    pub name: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returning: Option<Param>,
    /// The caller needs one of these roles (empty: any authenticated caller).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// Callable without a token (the sign-in programs, a health check).
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
    /// Safe to repeat after a lost connection (WASM-014 R9).
    #[serde(default, skip_serializing_if = "is_false")]
    pub idempotent: bool,
    /// May run beside other calls of the same connection; otherwise calls of
    /// one connection run in the order they were made (WASM-014 R11).
    #[serde(default, skip_serializing_if = "is_false")]
    pub independent: bool,
    /// Reports progress and can be cancelled (WASM-014 R13).
    #[serde(default, skip_serializing_if = "is_false")]
    pub long_running: bool,
    /// A stream the backend publishes on (WASM-014 R14).
    #[serde(default, skip_serializing_if = "is_false")]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecation>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl ProgramSig {
    pub fn new(name: &str) -> Self {
        ProgramSig {
            name: name.to_ascii_uppercase(),
            params: Vec::new(),
            returning: None,
            roles: Vec::new(),
            public: false,
            idempotent: false,
            independent: false,
            long_running: false,
            stream: false,
            deprecated: None,
        }
    }
}

/// What a backend exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDescription {
    pub format: String,
    pub format_version: u32,
    /// The contract's own version, `major.minor.patch` (WASM-014 R1).
    pub contract: String,
    /// The wire protocol it speaks.
    pub protocol: u32,
    pub programs: Vec<ProgramSig>,
}

impl InterfaceDescription {
    pub fn new(contract: &str) -> Self {
        InterfaceDescription {
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION,
            contract: contract.to_string(),
            protocol: PROTOCOL,
            programs: Vec::new(),
        }
    }

    /// The program called `name` (case-insensitive).
    pub fn program(&self, name: &str) -> Option<&ProgramSig> {
        self.programs.iter().find(|p| p.name.eq_ignore_ascii_case(name.trim()))
    }

    /// Read the description a project has imported: its backend's
    /// [`PROJECT_PATH`] under `project_dir`. `Ok(None)` when the project has no
    /// backend description; `Err` when there is a file and it cannot be read —
    /// a corrupt contract must be seen, never skipped.
    pub fn load_project(project_dir: &std::path::Path) -> Result<Option<Self>, String> {
        let path = project_dir.join(PROJECT_PATH);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_json(&text).map(Some).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let d: InterfaceDescription = serde_json::from_str(text).map_err(|e| format!("not an interface description: {e}"))?;
        if d.format != FORMAT {
            return Err(format!("not an interface description: format is \"{}\", expected \"{FORMAT}\"", d.format));
        }
        if d.format_version > FORMAT_VERSION {
            return Err(format!("interface description format {} is newer than this build reads ({FORMAT_VERSION})", d.format_version));
        }
        Ok(d)
    }
}
