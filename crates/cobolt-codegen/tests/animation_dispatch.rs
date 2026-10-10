// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL-PLAY-ANIMATION` / `COBOL-STOP-ANIMATION` choose by the animation's
//! NAME in `WS-ANIM-NAME`, and several controls may carry the same name.
//!
//! An `EVALUATE` runs only the FIRST `WHEN` that matches, so one `WHEN` per
//! control meant that a name shared by four controls played on the first and
//! never reached the other three. One `WHEN` per NAME, holding every control's
//! `INVOKE`, plays them all — and reads as what it is. Each `INVOKE` stays on
//! one line.

use cobolt_codegen::generate;
use cobolt_forms::model::{AnimTrigger, AnimationDef};
use cobolt_forms::{Control, ControlType, Form};

fn control_with(id: &str, animations: &[&str]) -> Control {
    let mut c = Control::new(id, ControlType::Label, 10, 10);
    c.animations = animations.iter().map(|n| AnimationDef::new(*n)).collect();
    c
}

/// The text of one generated paragraph, from its header to the next paragraph.
fn paragraph<'a>(src: &'a str, name: &str) -> &'a str {
    let head = format!("       {name}.\n");
    let from = src.find(&head).unwrap_or_else(|| panic!("no paragraph {name}:\n{src}"));
    let rest = &src[from + head.len()..];
    let end = rest
        .lines()
        .skip(1)
        .find(|l| l.starts_with("       ") && !l.starts_with("        ") && l.trim_end().ends_with('.'))
        .map(|l| rest.find(l).unwrap())
        .unwrap_or(rest.len());
    &rest[..end]
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// Four controls share "intro", one has "other", the form has its own "intro".
fn shared_names_form() -> Form {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    form.controls = vec![
        control_with("C-KPI", &["intro"]),
        control_with("LBL-KPI-T", &["intro"]),
        control_with("LBL-KPI-V", &["intro", "other"]),
        control_with("GAU-UNIV", &["intro"]),
    ];
    form.animations = vec![AnimationDef::new("intro")];
    form
}

#[test]
fn a_name_shared_by_several_controls_gets_one_when_holding_every_invoke() {
    let src = generate(&shared_names_form());
    for (para, method) in [("COBOL-PLAY-ANIMATION", "PlayAnimation"), ("COBOL-STOP-ANIMATION", "StopAnimation")] {
        let p = paragraph(&src, para);
        assert_eq!(count(p, "WHEN \"intro\""), 1, "{para}: one WHEN for the name:\n{p}");
        assert_eq!(count(p, "WHEN \"other\""), 1, "{para}:\n{p}");
        assert_eq!(count(p, "WHEN OTHER"), 1, "{para}:\n{p}");
        // Every control that has the name is reached, in the order they are
        // declared, between its WHEN and the next.
        let when = p.find("WHEN \"intro\"").unwrap();
        let next = p.find("WHEN \"other\"").unwrap();
        assert!(when < next, "{para}: names keep their first-appearance order:\n{p}");
        let intro = &p[when..next];
        let mut at = 0;
        for id in ["C-KPI", "LBL-KPI-T", "LBL-KPI-V", "GAU-UNIV"] {
            let line = format!("INVOKE {id} '{method}' USING BY VALUE \"intro\"");
            let found = intro[at..].find(&line).unwrap_or_else(|| panic!("{para}: `{line}` missing or out of order:\n{p}"));
            at += found + line.len();
        }
        // "other" belongs to one control only.
        assert_eq!(count(&p[next..], "INVOKE LBL-KPI-V "), 1, "{para}:\n{p}");
        assert_eq!(count(&p[next..], "INVOKE C-KPI "), 0, "{para}:\n{p}");
    }
}

#[test]
fn an_invoke_is_never_broken_over_two_lines() {
    let mut form = shared_names_form();
    // A trigger paragraph and a lone animation exercise the other emitters.
    let mut clicky = control_with("BTN-GO", &[]);
    let mut a = AnimationDef::new("pulse");
    a.trigger = AnimTrigger::OnClick;
    clicky.animations = vec![a];
    form.controls.push(clicky);

    let src = generate(&form);
    for (n, line) in src.lines().enumerate() {
        assert!(
            !line.trim_start().starts_with("USING BY VALUE"),
            "line {}: `USING BY VALUE` was pushed onto its own line:\n{}",
            n + 1,
            line
        );
    }
    let trigger = paragraph(&src, "BTN-GO-PLAY-PULSE");
    assert!(trigger.contains("INVOKE BTN-GO 'PlayAnimation' USING BY VALUE \"pulse\".\n"), "{trigger}");
}

#[test]
fn a_single_animation_is_one_unconditional_line() {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    form.controls = vec![control_with("PIC-1", &["fade"])];
    let src = generate(&form);
    assert!(
        paragraph(&src, "COBOL-PLAY-ANIMATION").contains("INVOKE PIC-1 'PlayAnimation' USING BY VALUE \"fade\".\n"),
        "{src}"
    );
    assert!(
        paragraph(&src, "COBOL-STOP-ANIMATION").contains("INVOKE PIC-1 'StopAnimation' USING BY VALUE \"fade\".\n"),
        "{src}"
    );
}

#[test]
fn a_name_only_the_form_has_does_nothing_but_still_parses() {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    form.controls = vec![control_with("PIC-1", &["fade"])];
    form.animations = vec![AnimationDef::new("page-in")];
    let src = generate(&form);
    let p = paragraph(&src, "COBOL-PLAY-ANIMATION");
    assert_eq!(count(p, "WHEN \"page-in\""), 1, "{p}");
    let after = &p[p.find("WHEN \"page-in\"").unwrap()..];
    assert!(after.lines().nth(1).unwrap().trim() == "CONTINUE", "a form-only name is a CONTINUE:\n{p}");
}

#[test]
fn the_generated_program_still_parses_clean() {
    let src = generate(&shared_names_form());
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("line {}: {}", d.span.line, d.message))
        .collect();
    assert!(errors.is_empty(), "the generated program must parse:\n{}", errors.join("\n"));
}
