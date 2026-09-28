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
    // The menu is designed, so the designer and the preview show it; the
    // chat form relabels each row in the current language (R46) and holds
    // the rows shut until an agent has a model. Every form row opens its
    // form EMBEDDED in the ContentPane, by its designed action. New
    // conversation heads the menu, as it does in a chat app (1.70.280).
    let designed: Vec<(&str, Option<&str>)> =
        menu.menu.iter().map(|i| (i.id.as_str(), i.action.as_deref())).collect();
    assert_eq!(
        designed,
        [
            ("newc", None),
            ("chat", Some("home")),
            ("tpcs", Some("open-form:topics-form")),
            ("docs", Some("open-form:documents-form")),
            ("fils", Some("open-form:files-form")),
            ("prmt", Some("open-form:prompts-form")),
            ("sett", Some("open-form:settings-form")),
        ]
    );
    let chat = std::fs::read_to_string(project().join("generated/chat-form.cbl")).unwrap();
    for (id, _) in &designed {
        assert!(chat.contains(&format!("SideMenu-1::SetItemLabel(\"{id}\"")), "the chat form labels menu row {id}");
    }
    // Chat waits for a topic as well (1.70.280: with no topic there is
    // nothing to chat about); RAG settings, where a model is set up, never
    // waits for anything.
    for id in ["newc", "chat", "tpcs", "docs", "fils", "prmt"] {
        assert!(chat.contains(&format!("SideMenu-1::SetItemEnabled(\"{id}\"")), "row {id} waits for a model");
    }
    assert!(!chat.contains("SetItemEnabled(\"sett\""), "RAG settings is never shut");
    // No form opens in a window of its own — except the generic dialogs a
    // question is asked in (New conversation picks its topic in PICK-FORM,
    // 1.70.280).
    for (i, _) in chat.match_indices("OpenFormSync\"(\"") {
        let target: String = chat[i + 15..].chars().take_while(|c| *c != '"').collect();
        assert!(["PICK-FORM", "CONFIRM-FORM"].contains(&target.as_str()), "the chat opens {target} in a window of its own");
    }
    // The flags are image buttons: an empty Caption (a missing one shows the
    // id, which then takes the whole button and leaves the image no room).
    let chat_form = cobolt_forms::load_form(&project().join("forms/chat-form.cfrm")).unwrap();
    for code in ["en", "pt", "es", "fr", "jp", "cn"] {
        let flag = chat_form.controls.iter().find(|c| c.id == format!("Flag-{code}")).expect("a flag");
        assert_eq!(flag.get_prop("Caption").map(|v| v.to_string()).as_deref(), Some(""), "Flag-{code} has no caption");
        assert_eq!(
            flag.get_prop("IconPath").map(|v| v.to_string()),
            Some(format!("assets/flags/{code}.png")),
            "Flag-{code} shows its flag"
        );
        assert!(project().join(format!("assets/flags/{code}.png")).exists());
    }
    // Every form on the pane follows a language picked while it is there.
    for rel in forms() {
        let form = cobolt_forms::load_form(&project().join(&rel)).unwrap();
        if form.form_format == cobolt_forms::model::FormFormat::Embedded {
            assert!(form.controls.iter().any(|c| c.id == "Tmr-Lang"), "{rel} watches the language");
        }
    }
    // RAG settings is a summary whose four groups each open a modal dialog
    // (operator, 2026-09-25): those four are windows, opened by the summary.
    let settings = std::fs::read_to_string(project().join("generated/settings-form.cbl")).unwrap();
    let dialogs = ["kb-folder-form", "providers-form", "model-form", "agents-form"];
    for rel in forms() {
        let form = cobolt_forms::load_form(&project().join(&rel)).unwrap();
        let stem = Path::new(&rel).file_stem().unwrap().to_string_lossy().to_string();
        if dialogs.contains(&stem.as_str()) {
            assert_eq!(form.form_format, cobolt_forms::model::FormFormat::Standalone, "{rel} is a modal dialog");
            // At least 800 x 450, and every control inside it: the connections
            // dialog is larger, for its CRUD tabs (1072 x 552).
            assert!(form.width >= 800 && form.height >= 450, "{rel}: a dialog is at least 800 x 450");
            for c in &form.controls {
                assert!(
                    c.rect.x + c.rect.w <= form.width as i32 && c.rect.y + c.rect.h <= form.height as i32,
                    "{rel}: {} runs past the dialog's edge",
                    c.id
                );
            }
            let id = cobolt_compiler::main_form_guard::form_id(Path::new(&rel));
            assert!(settings.contains(&format!("OpenFormSync\"(\"{id}\")")), "RAG settings opens {id}");
        } else if ["confirm-form", "pick-form", "preview-form"].contains(&stem.as_str()) {
            // The generic dialogs any form asks through (a yes/no, a choice,
            // a document shown): windows of their own, sized to what they hold.
            assert_eq!(form.form_format, cobolt_forms::model::FormFormat::Standalone, "{rel} is a modal dialog");
            for c in &form.controls {
                assert!(
                    c.rect.x + c.rect.w <= form.width as i32 && c.rect.y + c.rect.h <= form.height as i32,
                    "{rel}: {} runs past the dialog's edge",
                    c.id
                );
            }
        } else if !rel.ends_with("chat-form.cfrm") {
            assert_eq!(form.form_format, cobolt_forms::model::FormFormat::Embedded, "{rel} opens in the ContentPane");
            assert!(form.controls.iter().all(|c| c.id != "Btn-Close"), "{rel}: an embedded form has no Close button");
        }
    }

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

/// R44-R46 — every text a user sees comes from the form's translation table,
/// in all six languages: each designed caption and hint is re-applied by
/// `PC-TEXTS`, and each table row is filled in every language.
#[test]
fn every_visible_text_is_translated_into_six_languages() {
    let t = std::time::Instant::now();
    let mut rows = Vec::new();
    for rel in forms() {
        let path = project().join(&rel);
        let form = cobolt_forms::load_form(&path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(project().join("generated").join(format!("{stem}.cbl"))).unwrap();
        let mut applied = 0;
        for c in &form.controls {
            for prop in ["Caption", "HintText", "Hint"] {
                let text = c.get_prop(prop).map(|v| v.to_string()).unwrap_or_default();
                // Empty in the design (the loader shows the id): set at run time.
                // No lowercase letter ("1-3", "API"): the same in every language.
                // The product's name is not translated either.
                if text.trim().is_empty()
                    || text == c.id
                    || text.trim() == "PowerChat"
                    || !text.chars().any(|ch| ch.is_lowercase())
                {
                    continue;
                }
                let needle = format!(" TO {}::{prop}", c.id);
                assert!(
                    src.lines().any(|l| l.contains("FUNCTION TRIM(T-") && l.trim_end().ends_with(&needle)),
                    "{rel}: {}::{prop} ({text:?}) is shown but never translated",
                    c.id
                );
                applied += 1;
            }
        }
        // The table: a comment naming each text, then one FILLER per language.
        // A form that designs no text of its own — `preview-form`, whose
        // title and button the caller hands over already translated — has
        // nothing to put in one.
        let lines: Vec<&str> = src.lines().collect();
        let Some(start) = lines.iter().position(|l| l.contains("01 PC-TEXT-DATA")) else {
            assert_eq!(applied, 0, "{rel} shows designed text, so it needs a translation table");
            rows.push(format!("  {:<28}   no designed text — the caller supplies it", rel));
            continue;
        };
        let end = lines.iter().position(|l| l.contains("01 PC-TEXT-TABLE REDEFINES")).unwrap();
        let mut texts = 0;
        let mut i = start + 1;
        while i < end {
            let id = lines[i].trim().trim_start_matches("*>").trim();
            let values: Vec<String> = lines[i + 1..i + 7]
                .iter()
                .map(|l| {
                    let v = &l[l.find("VALUE \"").expect("a VALUE") + 7..l.rfind('"').unwrap()];
                    v.replace("\"\"", "\"")
                })
                .collect();
            assert!(values.iter().all(|v| !v.trim().is_empty()), "{rel}: {id} is empty in some language: {values:?}");
            assert!(values.iter().any(|v| v != &values[0]), "{rel}: {id} is the same in all six languages");
            texts += 1;
            i += 7;
        }
        rows.push(format!("  {:<28} {:>3} texts x 6 languages, {:>2} designed captions/hints re-applied", rel, texts, applied));
    }
    println!("\n  ── 071 PowerChat languages ──────────────────────\n{}\n  {:.0} ms\n", rows.join("\n"), t.elapsed().as_secs_f64() * 1000.0);
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
