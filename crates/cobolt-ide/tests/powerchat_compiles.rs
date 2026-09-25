// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 071 — the PowerChat example project stays buildable.
//!
//! Every form listed in `examples/PowerChat/PowerChat.project.toml` loads,
//! generates exactly the program committed under `generated/` (run
//! `cargo run -p cobolt-ide --example powerchat_regen` after editing a form),
//! parses with no errors and passes the semantic analyser. The SideMenu's
//! menu file carries a valid hash, the main-form seal matches, and no file in
//! the example carries a credential (spec 071 R3).

use std::path::{Path, PathBuf};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerChat")
}

fn forms() -> Vec<String> {
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(project().join("PowerChat.project.toml")).unwrap()).unwrap();
    manifest["files"]["forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn every_powerchat_form_generates_its_committed_program_and_compiles() {
    let t = std::time::Instant::now();
    let mut rows = Vec::new();
    for rel in forms() {
        let path = project().join(&rel);
        let form = cobolt_forms::load_form(&path).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let src = cobolt_codegen::generate(&form);
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let committed = std::fs::read_to_string(project().join("generated").join(format!("{stem}.cbl")))
            .unwrap_or_default();
        assert!(
            committed == src,
            "generated/{stem}.cbl is stale — run `cargo run -p cobolt-ide --example powerchat_regen`"
        );
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let parse_errors: Vec<String> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| format!("  line {}: {}", d.span.line, d.message))
            .collect();
        assert!(parse_errors.is_empty(), "{rel} does not parse:\n{}", parse_errors.join("\n"));
        let program = parsed.program.expect("a program");
        let sem = cobolt_semantic::analyze(&program);
        let errors: Vec<String> = sem
            .errors()
            .map(|d| format!("  line {}: {}", d.span.line, d.message))
            .collect();
        assert!(errors.is_empty(), "{rel}: {} semantic error(s):\n{}", errors.len(), errors.join("\n"));
        let handlers = src.matches("IS COMMON PROGRAM").count();
        rows.push(format!(
            "  {:<28} {:>3} controls  {:>3} nested programs  {:>5} lines",
            rel,
            form.controls.len(),
            handlers,
            src.lines().count()
        ));
    }
    println!("\n  ── 071 PowerChat forms ──────────────────────────\n{}\n  {:.0} ms\n", rows.join("\n"), t.elapsed().as_secs_f64() * 1000.0);
}

#[test]
fn the_menu_hash_and_the_main_form_seal_are_valid() {
    let menu = cobolt_forms::menu::load_menu(&project().join("forms/SideMenu-1.menu.yaml"))
        .expect("the SideMenu's menu loads with a valid hash");
    let ids: Vec<&str> = menu.menu.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["chat", "newc", "tpcs", "docs", "fils", "sett"]);

    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(project().join("PowerChat.project.toml")).unwrap()).unwrap();
    let ids: Vec<String> = forms()
        .iter()
        .map(|f| cobolt_compiler::main_form_guard::form_id(Path::new(f)))
        .collect();
    let seal = cobolt_compiler::main_form_guard::seal(manifest["project"]["name"].as_str().unwrap(), "CHAT-FORM", &ids);
    assert_eq!(manifest["forms"]["main-form"].as_str(), Some("CHAT-FORM"));
    assert_eq!(manifest["forms"]["main-form-seal"].as_str(), Some(seal.as_str()));
}

/// R3 — no example file carries a key, password or token value.
#[test]
fn no_powerchat_file_carries_a_credential() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&project(), &mut files);
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for prop in ["AgentAPIKey", "EmbeddingAPIKey", "ApiKey", "AuthToken", "Password"] {
            let needle = format!("<Property name=\"{prop}\">");
            if let Some(i) = text.find(&needle) {
                let rest = &text[i + needle.len()..];
                let value = &rest[..rest.find('<').unwrap_or(0)];
                assert!(value.trim().is_empty(), "{} carries a {prop}", f.display());
            }
        }
        assert!(!text.contains("sk-"), "{} looks like it carries a key", f.display());
    }
}
