// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 S5 (R29, R35, AC15): a form that opts into compact storage loses
//! the `WS-<ID>` group of a control no other line names, and nothing else.

use cobolt_codegen::generate;
use cobolt_forms::{Control, ControlType, Form};

fn form(compact: bool, user_ws: &str) -> Form {
    let mut f = Form::new("MAIN-FORM", "Demo", 640, 480);
    f.controls = vec![
        Control::new("BTN-OK", ControlType::Button, 10, 10),
        Control::new("TXT-A", ControlType::TextBox, 10, 50),
        Control::new("LBL-1", ControlType::Label, 10, 90),
    ];
    f.user_ws_source = user_ws.to_string();
    f.compact_storage = compact;
    f
}

#[test]
fn off_by_default_every_control_keeps_its_group() {
    let out = generate(&form(false, ""));
    for id in ["BTN-OK", "TXT-A", "LBL-1"] {
        assert!(out.contains(&format!("01 WS-{id}.")), "{id} missing:\n{out}");
    }
}

#[test]
fn compact_leaves_out_the_groups_nobody_names() {
    let out = generate(&form(true, ""));
    for id in ["BTN-OK", "TXT-A", "LBL-1"] {
        assert!(!out.contains(&format!("WS-{id}")), "{id} kept:\n{out}");
    }
}

#[test]
fn compact_keeps_a_group_that_authored_storage_names() {
    let ws = "       01 WS-COPY REDEFINES WS-TXT-A-TEXT PIC X(5).\n";
    let out = generate(&form(true, ws));
    assert!(out.contains("01 WS-TXT-A."), "TXT-A dropped:\n{out}");
    assert!(!out.contains("01 WS-BTN-OK."), "BTN-OK kept:\n{out}");
    assert!(!out.contains("01 WS-LBL-1."), "LBL-1 kept:\n{out}");
}

#[test]
fn a_word_that_only_ends_in_the_name_does_not_keep_it() {
    // `XWS-TXT-A` is another word; `WS-TXT-A-COUNT` begins with the name and
    // keeps the group, because a false retention is the safe error.
    let other = generate(&form(true, "       01 XWS-TXT-A PIC X.\n"));
    assert!(!other.contains("01 WS-TXT-A."));
    let prefixed = generate(&form(true, "       01 WS-TXT-A-COUNT PIC 9.\n"));
    assert!(prefixed.contains("01 WS-TXT-A."));
}

#[test]
fn compact_output_is_the_full_output_minus_whole_groups() {
    let full = generate(&form(false, ""));
    let compact = generate(&form(true, ""));
    let kept: Vec<&str> = compact.lines().collect();
    let dropped: Vec<&str> = full.lines().filter(|l| !kept.contains(l)).collect();
    assert!(!dropped.is_empty());
    assert!(
        dropped.iter().all(|l| l.contains("WS-") || l.trim().is_empty()),
        "something other than a group went: {dropped:?}"
    );
}

// ── T5.2: the helper paragraphs nothing names ────────────────────────────────

use cobolt_forms::model::{AnimTrigger, AnimationDef, EventBinding, UserProcedure};

/// A chart with a clicked animation, a button with a handler, and a procedure
/// whose body performs `perform_in_proc` (or nothing).
fn helper_form(compact: bool, proc_code: &str) -> Form {
    let mut f = Form::new("MAIN-FORM", "Demo", 640, 480);
    let mut chart = Control::new("CH-BAR", ControlType::BarChart, 10, 10);
    let mut anim = AnimationDef::new("pulse");
    anim.trigger = AnimTrigger::OnClick;
    chart.animations.push(anim);
    let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 200);
    btn.events.push(EventBinding {
        event: "onClick".into(),
        paragraph: "BTN-OK--ONCLICK".into(),
        code: "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. BTN-OK--ONCLICK.\n       PROCEDURE DIVISION.\n           DISPLAY \"CLICKED\".\n".into(),
    });
    f.controls = vec![chart, btn];
    if !proc_code.is_empty() {
        f.user_procedures.push(UserProcedure { name: "DO-IT".into(), code: proc_code.into() });
    }
    f.compact_storage = compact;
    f
}

const HELPERS: [&str; 7] = [
    "COBOL-PLAY-ANIMATION.",
    "COBOL-STOP-ANIMATION.",
    "CH-BAR-PLAY-PULSE.",
    "CH-BAR-SET-TABLE.",
    "CH-BAR-ADD-POINT.",
    "CH-BAR-CLEAR.",
    "CH-BAR-REFRESH.",
];

fn has_para(text: &str, label: &str) -> bool {
    text.lines().any(|l| l.trim() == label && l.starts_with("       ") && !l.starts_with("        "))
}

#[test]
fn off_by_default_every_helper_paragraph_is_written() {
    let out = generate(&helper_form(false, ""));
    for p in HELPERS {
        assert!(has_para(&out, p), "{p} missing:\n{out}");
    }
}

#[test]
fn compact_leaves_out_the_helpers_nobody_names() {
    let out = generate(&helper_form(true, ""));
    for p in HELPERS {
        assert!(!has_para(&out, p), "{p} kept:\n{out}");
    }
    assert!(out.contains("COBOL-EVENT-LOOP."), "the event loop is not a helper");
}

#[test]
fn compact_keeps_a_helper_the_developers_code_performs() {
    let proc_code = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. DO-IT.\n       PROCEDURE DIVISION.\n           PERFORM CH-BAR-REFRESH.\n";
    let out = generate(&helper_form(true, proc_code));
    assert!(has_para(&out, "CH-BAR-REFRESH."), "named by DO-IT:\n{out}");
    assert!(!has_para(&out, "CH-BAR-CLEAR."), "CLEAR is still unnamed");
    assert!(!has_para(&out, "CH-BAR-SET-TABLE."));
}

#[test]
fn the_source_map_still_points_at_the_developers_lines_after_the_helpers_go() {
    let full = helper_form(false, "");
    let compact = helper_form(true, "");
    let (a, a_ranges) = cobolt_codegen::generate_with_user_lines(&full);
    let (b, b_ranges) = cobolt_codegen::generate_with_user_lines(&compact);
    assert!(b.lines().count() < a.lines().count(), "the compact program is shorter");
    let text = |src: &str, ranges: &[(u32, u32)]| -> Vec<String> {
        let lines: Vec<&str> = src.lines().collect();
        ranges.iter().flat_map(|(s, e)| lines[(*s as usize - 1)..*e as usize].iter().map(|l| l.to_string())).collect()
    };
    let (ta, tb) = (text(&a, &a_ranges), text(&b, &b_ranges));
    assert!(!ta.is_empty() && ta == tb, "the same developer lines, found at their new numbers: {ta:?} vs {tb:?}");
}

// ── Every example form, compact: still a program that parses and checks ──────

use cobolt_lexer::{tokenize, SourceFormat};

fn example_forms() -> Vec<std::path::PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("cfrm") && !p.to_string_lossy().contains(".recovered.") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for project in ["PowerAnalytics", "PowerChat", "PowerCloud", "PowerDemo3", "PowerSpatial"] {
        walk(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(project).join("forms"), &mut out);
    }
    out.sort();
    out
}

/// (parse errors, semantic errors) of a generated program.
fn errors_of(src: &str) -> (usize, usize) {
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    let parse_errors = parsed.diagnostics.iter().filter(|d| d.is_error()).count();
    let semantic_errors = parsed
        .program
        .map(|p| cobolt_semantic::analyze(&p).diagnostics.iter().filter(|d| matches!(d.severity, cobolt_semantic::Severity::Error)).count())
        .unwrap_or(0);
    (parse_errors, semantic_errors)
}

/// AC15, AC16: a form that opts in is generated into a program that is no worse
/// than the full one — no new parse error, no new semantic error — and the
/// developer's lines are the same lines. Counted and reported (rule #7).
#[test]
fn every_example_form_compact_parses_and_checks_no_worse_than_full() {
    let started = std::time::Instant::now();
    let (mut forms, mut lines_full, mut lines_compact, mut groups, mut paras) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut worse = Vec::new();
    for path in example_forms() {
        let mut form = cobolt_forms::load_form(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        form.compact_storage = false;
        let (full, full_ranges) = cobolt_codegen::generate_with_user_lines(&form);
        form.compact_storage = true;
        let (compact, compact_ranges) = cobolt_codegen::generate_with_user_lines(&form);
        let (fe, ce) = (errors_of(&full), errors_of(&compact));
        if ce.0 > fe.0 || ce.1 > fe.1 {
            worse.push(format!("{}: full {fe:?}, compact {ce:?}", path.display()));
        }
        let text = |src: &str, ranges: &[(u32, u32)]| -> Vec<String> {
            let l: Vec<&str> = src.lines().collect();
            ranges.iter().flat_map(|(s, e)| l[(*s as usize - 1)..(*e as usize).min(l.len())].iter().map(|x| x.to_string())).collect()
        };
        if text(&full, &full_ranges) != text(&compact, &compact_ranges) {
            worse.push(format!("{}: the developer's lines moved or changed", path.display()));
        }
        forms += 1;
        lines_full += full.lines().count();
        lines_compact += compact.lines().count();
        let group = |l: &&str| l.starts_with("       01 WS-") && l.trim_end().ends_with('.') && l.split_whitespace().count() == 2;
        let label = |l: &&str| {
            l.starts_with("       ")
                && !l.starts_with("        ")
                && l.trim_end().ends_with('.')
                && l.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                && !l.trim().to_ascii_uppercase().starts_with("01")
        };
        groups += full.lines().filter(group).count() - compact.lines().filter(group).count();
        paras += full.lines().filter(label).count() - compact.lines().filter(label).count();
    }
    println!("── spec 109 S5 compact generation over the example forms ──");
    println!("  forms          : {forms}");
    println!("  lines          : {lines_full} full → {lines_compact} compact ({} saved)", lines_full - lines_compact);
    println!("  groups left out: {groups}");
    println!("  labels left out: {paras} (helper paragraphs)");
    println!("  worse than full: {}", worse.len());
    println!("  elapsed        : {:.2} s", started.elapsed().as_secs_f32());
    assert!(worse.is_empty(), "{}", worse.join("\n"));
}
