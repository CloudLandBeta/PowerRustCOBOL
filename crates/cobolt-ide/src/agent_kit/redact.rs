// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **No secret and nothing personal in the kit** (spec 080 R4, T4.5).
//!
//! The AI export's own redaction (`ai_bundle`), applied where the kit can
//! carry a developer's details (plan D8, operator ruling on F1):
//!
//! - the values the export **inserts** — the project's name, the `rcrun`
//!   path — have the home folder, login, git name and e-mail replaced before
//!   any text is rendered ([`scrub_value`]);
//! - every file about to be written is scanned for any key stored on this
//!   machine, and every file the export **generates** for a personal detail
//!   that is still there ([`check`]). The reference documents copied verbatim
//!   from the binary are scanned for keys only — a login such as `main` would
//!   otherwise match ordinary words in the Guide.
//!
//! Any hit refuses the export before the first byte is written.

use crate::ai_bundle::{find_key, Personal};
use crate::i18n::Tr;
use crate::llm::LlmConfig;

use super::stamp::Planned;

/// Why an export was refused (R4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A key stored on this machine (named by its slot, never its value).
    Key { file: String, slot: String },
    /// A personal detail (named by its placeholder kind) that could not be
    /// replaced.
    Personal { file: String, kind: &'static str },
}

impl Refusal {
    /// The Output line, in the IDE's language.
    pub fn message(&self, tr: &Tr) -> String {
        let reason = match self {
            Refusal::Key { file, slot } => tr.agent_kit_refused_key.replacen("{}", slot, 1).replacen("{}", file, 1),
            Refusal::Personal { file, kind } => {
                tr.agent_kit_refused_personal.replacen("{}", kind, 1).replacen("{}", file, 1)
            }
        };
        tr.agent_kit_refused.replacen("{}", &reason, 1)
    }
}

/// An inserted value with every personal detail replaced; and how many were.
pub fn scrub_value(personal: &Personal, value: &str) -> (String, usize) {
    personal.scrub(value)
}

/// What a passing scan looked at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scan {
    pub files: usize,
    pub generated: usize,
}

/// Scan what the export is about to write — the kit's own part of every file
/// that will be written, and the kit manifest — and refuse on the first hit.
pub fn check(planned: &[Planned], manifest: (&str, &str), llm: &LlmConfig, personal: &Personal) -> Result<Scan, Refusal> {
    let mut scan = Scan::default();
    let written = planned
        .iter()
        .filter(|p| p.bytes.is_some())
        .map(|p| (p.rel.as_str(), p.kit_text.as_str(), p.generated))
        .chain(std::iter::once((manifest.0, manifest.1, true)));
    for (rel, text, generated) in written {
        scan.files += 1;
        if let Some(slot) = find_key(text, llm) {
            return Err(Refusal::Key { file: rel.into(), slot });
        }
        if generated {
            scan.generated += 1;
            if let Some(kind) = personal.find(text) {
                return Err(Refusal::Personal { file: rel.into(), kind });
            }
        }
    }
    Ok(scan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_kit::{export, ExportContext, ExportError, Target};
    use crate::llm::provider_key_slot;
    use std::path::{Path, PathBuf};

    const KEY: &str = "sk-ant-SECRET-0123456789";

    fn machine() -> LlmConfig {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.store_api_key(provider_key_slot("anthropic"), KEY);
        llm
    }

    fn me() -> Personal {
        Personal::new(
            Some("/Users/emersonlopes".into()),
            Some("emersonlopes".into()),
            Some("Emerson Lopes".into()),
            Some("emersonlopes@gmail.com".into()),
        )
    }

    fn project(name: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Demo.project.toml"),
            format!("[project]\nname = {}\nstructure = 1\n\n[files]\nforms = []\n", toml_str(name)),
        )
        .unwrap();
        dir
    }

    fn toml_str(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    /// Every file under `dir`, path and bytes — a directory hash in the clear.
    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    fn ctx<'a>(llm: &'a LlmConfig, personal: &'a Personal, rcrun: &str) -> ExportContext<'a> {
        ExportContext {
            ide_version: "1.80.70",
            mcp_port: 5720,
            rcrun: Some(PathBuf::from(rcrun)),
            home: Some(PathBuf::from("/Users/emersonlopes")),
            personal,
            llm,
        }
    }

    /// AC3: details planted in the project name and the rcrun path are
    /// replaced and appear in no written file.
    #[test]
    fn planted_details_are_replaced_and_appear_nowhere() {
        let llm = machine();
        let personal = me();
        let dir = project("emersonlopes Shop of Emerson Lopes (emersonlopes@gmail.com) /Users/emersonlopes/x");
        let report = export(
            dir.path(),
            Target::ClaudeCode,
            &ctx(&llm, &personal, "/Users/emersonlopes/tools/emersonlopes/rcrun"),
        )
        .expect("export");
        let mut scanned = 0;
        for (path, bytes) in snapshot(dir.path()) {
            if path.extension().is_some_and(|e| e == "toml") {
                continue; // the developer's own manifest, untouched by the kit
            }
            let text = String::from_utf8_lossy(&bytes).to_lowercase();
            scanned += 1;
            let generated = !path.to_string_lossy().contains("docs/powerrustcobol/developers-guide")
                && !path.to_string_lossy().contains("docs/powerrustcobol/cobol85");
            if generated {
                for needle in ["emersonlopes", "emerson lopes", "gmail.com", "/users/"] {
                    assert!(!text.contains(needle), "{needle} leaked into {}", path.display());
                }
            }
            assert!(!text.contains(&KEY.to_lowercase()), "the key leaked into {}", path.display());
        }
        let brief = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(brief.contains("<user> Shop of <name removed> (<e-mail removed>) ~/x"), "the name was not scrubbed as expected");
        let mcp = std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
        assert!(mcp.contains("${HOME}/tools/<user>/rcrun"));
        println!(
            "redaction: 4 needles planted in the project name and the rcrun path, {} replacements, \
             {scanned} written files scanned ({} by the export's own scan), none leaked",
            report.replaced, report.scanned
        );
    }

    /// AC3: a stored key planted in an inserted value refuses the export, and
    /// the project is byte-identical afterwards — nothing was written.
    #[test]
    fn a_planted_key_refuses_and_nothing_is_written() {
        let llm = machine();
        let personal = me();
        let mut refusals = 0;
        for (name, rcrun) in [
            (format!("Shop {KEY}"), "/opt/prc/rcrun".to_string()),
            ("Shop".to_string(), format!("/opt/{KEY}/rcrun")),
        ] {
            let dir = project(&name);
            let before = snapshot(dir.path());
            let err = export(dir.path(), Target::ClaudeCode, &ctx(&llm, &personal, &rcrun)).unwrap_err();
            match &err {
                ExportError::Refused(Refusal::Key { slot, .. }) => assert_eq!(slot, &provider_key_slot("anthropic")),
                other => panic!("expected a key refusal, got {other:?}"),
            }
            assert_eq!(snapshot(dir.path()), before, "a refused export wrote something");
            let msg = ExportError::message(&err, &crate::i18n::Language::English.tr());
            assert!(msg.contains("Nothing was written") && !msg.contains(KEY), "{msg}");
            refusals += 1;
        }
        // A personal detail the scrub cannot reach: a login that is an
        // ordinary word of the kit's own text.
        let word_login = Personal::new(None, Some("reviewer".into()), None, None);
        let dir = project("Shop");
        let before = snapshot(dir.path());
        let err = export(dir.path(), Target::ClaudeCode, &ctx(&llm, &word_login, "/opt/prc/rcrun")).unwrap_err();
        assert!(matches!(err, ExportError::Refused(Refusal::Personal { kind: "<user>", .. })), "{err:?}");
        assert_eq!(snapshot(dir.path()), before);
        refusals += 1;
        println!("redaction: {refusals} refusals (key in the name, key in the rcrun path, unremovable login); project unchanged each time");
    }
}
