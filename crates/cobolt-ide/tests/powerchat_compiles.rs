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
    // The method is written `OpenFormSync("…")` or, quoted, `"OpenFormSync"("…")`.
    for opener in ["OpenFormSync\"(\"", "OpenFormSync(\""] {
        for (i, _) in chat.match_indices(opener) {
            let target: String = chat[i + opener.len()..].chars().take_while(|c| *c != '"').collect();
            assert!(["PICK-FORM", "CONFIRM-FORM"].contains(&target.as_str()), "the chat opens {target} in a window of its own");
        }
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
            assert!(
                settings.contains(&format!("OpenFormSync\"(\"{id}\")")) || settings.contains(&format!("OpenFormSync(\"{id}\")")),
                "RAG settings opens {id}"
            );
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
                // The statement, not its layout: `MOVE <from> TO ctrl::Prop` or
                // `SET ctrl::Prop TO <from>`, in any spacing, where <from> is
                // `FUNCTION TRIM(T-…)` or `T-…::Trim()`.
                let target = format!("{}::{prop}", c.id);
                let from_table = |from: &str| {
                    from.contains("FUNCTION TRIM(T-") || (from.starts_with("T-") && from.contains("::Trim()"))
                };
                assert!(
                    src.lines().any(|l| {
                        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
                        let l = l.trim_end_matches('.');
                        match l.strip_prefix("MOVE ") {
                            Some(rest) => rest.strip_suffix(&format!(" TO {target}")).is_some_and(from_table),
                            None => l.strip_prefix(&format!("SET {target} TO ")).is_some_and(from_table),
                        }
                    }),
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
        // Spacing between the words is layout (the first clause sits in column 50).
        let end = lines
            .iter()
            .position(|l| l.split_whitespace().collect::<Vec<_>>().join(" ").contains("01 PC-TEXT-TABLE REDEFINES"))
            .unwrap();
        let mut texts = 0;
        // Blank lines are layout (a comment is followed by one), not data.
        let table: Vec<&str> = lines[start + 1..end].iter().copied().filter(|l| !l.trim().is_empty()).collect();
        let mut i = 0;
        while i < table.len() {
            let id = table[i].trim().trim_start_matches("*>").trim();
            let values: Vec<String> = table[i + 1..i + 7]
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

/// 063 R46 (AC19, the half a program decides) — no window of the application changes its own size.
/// A window's size is its designed size; the only thing that may change it is the operator's grip.
/// So no PowerChat COBOL may assign a size to the form it runs in (`Form::` / `FORM::` / `ME::` /
/// `me::` `Width`, `Height`, `Left`, `Top`, `X`, `Y`, `WindowState`), whatever the language it relabels itself in.
#[test]
fn no_powerchat_window_resizes_itself() {
    let mut scanned = 0usize;
    for rel in forms() {
        let text = std::fs::read_to_string(project().join(&rel)).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("*>") {
                continue;
            }
            let upper = code.to_ascii_uppercase();
            for window in ["FORM::", "ME::"] {
                for prop in ["WIDTH", "HEIGHT", "LEFT", "TOP", "X", "Y", "WINDOWSTATE"] {
                    let needle = format!("{window}{prop}");
                    let at = upper.find(&needle);
                    if let Some(at) = at {
                        // A longer name on either side (`SOME::X`, `FORM::WIDTH-2`) is another thing.
                        let before = upper[..at].chars().next_back();
                        let after = upper[at + needle.len()..].chars().next();
                        let word = |c: char| c.is_ascii_alphanumeric() || c == '-';
                        assert!(
                            before.is_some_and(word) || after.is_some_and(word),
                            "{rel}:{} sets the window's own {prop}: {code}",
                            n + 1
                        );
                    }
                }
            }
        }
        scanned += 1;
    }
    println!("\n  063 R46 — {scanned} PowerChat forms scanned: none sets its own window's size or place");
}
