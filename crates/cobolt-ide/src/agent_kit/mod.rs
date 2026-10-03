// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The coding-agent tools in the IDE** (spec 080, spec 084).
//!
//! The IDE serves the project tools of `cobolt-project-tools` over HTTP on
//! `127.0.0.1` ([`ide_host`]). What tells a coding agent how to use them — the
//! rules, skills, reviewer and reference — is no longer exported into each
//! project (spec 080's kit): it is served live by the tool servers and carried
//! by the Claude Code plugin that Configure Claude Code installs
//! ([`crate::claude_code`]).

// Agent-neutral content and the reference pack live in `cobolt-project-tools`
// so `rcrun` serves them too (spec 084 T3); re-exported under their old paths.
pub use cobolt_project_tools::{content, reference};
pub mod ide_host;

#[cfg(test)]
mod tests {
    /// The reference documents `cobolt-project-tools` embeds for `rcrun` are
    /// byte-for-byte the ones the IDE's documentation viewer embeds (spec 084
    /// T3) — one source file each, two embeddings, no drift.
    #[test]
    fn the_reference_pack_embeds_the_same_documents_as_the_ide() {
        let pairs = [
            ("developers-guide-en.md", cobolt_project_tools::reference::DEVELOPERS_GUIDE),
            ("cobol85-supported-syntax-en.md", cobolt_project_tools::reference::SUPPORTED_SYNTAX),
        ];
        for (name, embedded) in pairs {
            assert_eq!(crate::docs_embed::embedded_doc(name), Some(embedded), "{name} differs");
        }
        println!("reference pack: {} documents identical to the IDE's embedded copies ({} bytes)",
            pairs.len(), pairs.iter().map(|(_, t)| t.len()).sum::<usize>());
    }
}
