// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The coding-agent companion kit** (spec 080).
//!
//! What the IDE does for an external coding agent such as Claude Code: serve
//! the project tools of `cobolt-project-tools` over HTTP on `127.0.0.1`
//! ([`ide_host`]), and — in later phases of the spec — export the kit that
//! tells the agent how to use them.

pub mod ide_host;

/// The kit manifest, relative to the project folder (spec 080 R5, plan §3.2).
/// The IDE reads its `kit_id` to know which project an MCP request is for.
pub const KIT_MANIFEST: &str = ".claude/powerrustcobol-kit.json";

/// The `kit_id` of the kit written into `project_dir`, if it has one.
pub fn kit_id_of(project_dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join(KIT_MANIFEST)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("kit_id")
        .and_then(|k| k.as_str())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
}
