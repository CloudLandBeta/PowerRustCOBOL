// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Golden guard (spec 053 T2, R8/R30/AC4): the generated `.cbl` bytes do not
//! move. Landed BEFORE the source-map recording touched any codegen writer,
//! so the T5–T8 refactor is provably byte-neutral.
//!
//! Each corpus form's `generate()` output is compared byte-for-byte against a
//! committed snapshot in `tests/golden/`. To regenerate the snapshots after a
//! *deliberate* codegen change, run with `UPDATE_GOLDEN=1` and review the
//! diff — an unreviewed regeneration defeats the guard.

use cobolt_forms::model::{Control, ControlType, EventBinding, Form, PropValue, UserProcedure};
use cobolt_forms::toolbar::{ButtonEvent, ToolbarButton, ToolbarDef, ToolbarGroup, TOOLBAR_DEF_PROP};
use std::path::PathBuf;

/// The corpus: one form per generator surface worth pinning.
fn corpus() -> Vec<(&'static str, Form)> {
    let mut forms: Vec<(&'static str, Form)> = Vec::new();

    // 1. Every in-form code-site kind, untidy input, one empty handler.
    forms.push(("all-sites", cobolt_forms::code_site::all_sites_fixture()));

    // 2. A bare form — pure scaffolding, no user code anywhere.
    forms.push(("empty-form", Form::new("EMPTY-FORM", "Empty", 640, 480)));

    // 3. Controls with a written handler, an empty (stub) handler, and a Timer.
    {
        let mut form = Form::new("CONTROLS-FORM", "Controls", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("BTN-OK", "onClick");
        ev.code = "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"OK PRESSED\".".to_string();
        btn.events.push(ev);
        form.controls.push(btn);
        let mut btn2 = Control::new("BTN-STUB", ControlType::Button, 10, 50);
        btn2.events
            .push(EventBinding::for_control("BTN-STUB", "onClick"));
        form.controls.push(btn2);
        form.controls
            .push(Control::new("TXT-NAME", ControlType::TextBox, 120, 10));
        form.controls
            .push(Control::new("LBL-TITLE", ControlType::Label, 120, 50));
        form.controls
            .push(Control::new("TIMER-1", ControlType::Timer, 0, 0));
        forms.push(("controls-and-stubs", form));
    }

    // 4. The IndexedFile facade paragraphs.
    {
        let mut form = Form::new("CUSTOMER-FORM", "Customers", 800, 600);
        let mut idx = Control::new("CustomerFile", ControlType::IndexedFile, 0, 0);
        idx.set_prop(
            "IndexedFile",
            PropValue::String("indexed/customers.cidx".into()),
        );
        idx.set_prop("OpenMode", PropValue::String("I-O".into()));
        idx.set_prop("AutoOpen", PropValue::Bool(true));
        idx.set_prop("RecordName", PropValue::String("CUSTOMER-REC".into()));
        idx.set_prop("KeyName", PropValue::String("CUSTOMER-ID".into()));
        form.controls.push(idx);
        forms.push(("indexed-file", form));
    }

    // 5. Structure blocks + a user procedure (spec 005 weave).
    {
        let mut form = Form::new("STRUCT-FORM", "Structure", 800, 600);
        form.cobol_structure.special_names = "           DECIMAL-POINT IS COMMA.".into();
        form.cobol_structure.file_control =
            "           SELECT F ASSIGN TO \"f.dat\".".into();
        form.cobol_structure.file_section =
            "       FD  F.\n       01 F-REC PIC X(80).".into();
        form.user_ws_source = "       01 WS-TOTAL PIC 9(9)V99 VALUE ZERO.".into();
        form.user_procedures = vec![UserProcedure {
            name: "RECALC-TOTAL".into(),
            code: "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."
                .into(),
        }];
        forms.push(("structure-and-procedure", form));
    }

    // 6. A toolbar whose buttons carry their own handlers and an action.
    {
        let mut form = Form::new("TOOLBAR-FORM", "Toolbar", 800, 600);
        let mut group = ToolbarGroup::new("file", "File");
        let mut save = ToolbarButton::new("save", "Save");
        let mut click = ButtonEvent::new("onClick");
        click.code = "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"SAVING\".".to_string();
        save.events.push(click);
        group.buttons.push(save);
        let mut open = ToolbarButton::new("open", "Open");
        open.action = "procedure:RECALC".into();
        group.buttons.push(open);
        let def = ToolbarDef {
            groups: vec![group],
            button_gap: 4,
        };
        let mut tb = Control::new("TB-MAIN", ControlType::ToolBar, 0, 0);
        tb.set_prop(TOOLBAR_DEF_PROP, PropValue::String(def.to_json().unwrap()));
        form.controls.push(tb);
        form.user_procedures = vec![UserProcedure {
            name: "RECALC".into(),
            code: "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."
                .into(),
        }];
        forms.push(("toolbar-buttons", form));
    }

    forms
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

#[test]
fn generated_bytes_do_not_move() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let dir = golden_dir();
    let mut files = 0usize;
    let mut bytes = 0usize;

    println!("── golden byte comparison (spec 053 AC4) ────────────────");
    for (name, form) in corpus() {
        let generated = cobolt_codegen::generate(&form);
        let path = dir.join(format!("{name}.cbl"));
        if update {
            std::fs::create_dir_all(&dir).expect("create golden dir");
            std::fs::write(&path, &generated).expect("write golden");
            println!("  {name:<24} snapshot WRITTEN ({} bytes)", generated.len());
            continue;
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing golden snapshot {path:?} ({e}); run once with UPDATE_GOLDEN=1 \
                 and review + commit the snapshots"
            )
        });
        if generated != golden {
            let first_diff = generated
                .lines()
                .zip(golden.lines())
                .position(|(a, b)| a != b)
                .map(|i| i + 1);
            panic!(
                "generated .cbl for corpus form {name:?} moved (R8/R30). \
                 First differing line: {first_diff:?}. If this change is deliberate, \
                 regenerate with UPDATE_GOLDEN=1 and review the diff."
            );
        }
        files += 1;
        bytes += generated.len();
        println!("  {name:<24} {:>7} bytes identical", generated.len());
    }
    if !update {
        println!("  {files} files compared, {bytes} bytes total — all identical");
        assert_eq!(files, 6, "the whole corpus must be compared");
    }
}

/// Wrapper parity (R29): `generate_with_user_lines` returns exactly the ranges
/// it returned **before** the source-map refactor. The snapshots were captured
/// pre-refactor, so a drift here means the debugger's view of "user code"
/// moved.
#[test]
fn user_line_ranges_do_not_move() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let dir = golden_dir();
    let mut files = 0usize;

    println!("── user-line range parity (spec 053 R29) ────────────────");
    for (name, form) in corpus() {
        let (_, ranges) = cobolt_codegen::generate_with_user_lines(&form);
        let rendered = ranges
            .iter()
            .map(|(s, e)| format!("{s}-{e}"))
            .collect::<Vec<_>>()
            .join("\n");
        let path = dir.join(format!("{name}.ranges.txt"));
        if update {
            std::fs::create_dir_all(&dir).expect("create golden dir");
            std::fs::write(&path, &rendered).expect("write ranges golden");
            println!("  {name:<24} ranges WRITTEN ({} ranges)", ranges.len());
            continue;
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("missing ranges snapshot {path:?} ({e}); run once with UPDATE_GOLDEN=1")
        });
        assert_eq!(
            rendered, golden,
            "user-line ranges for corpus form {name:?} moved (R29)"
        );
        files += 1;
        println!("  {name:<24} {:>2} ranges identical", ranges.len());
    }
    if !update {
        assert_eq!(files, 6, "the whole corpus must be compared");
    }
}
