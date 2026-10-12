// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The "layout" golden rule (operator, 2026-10-12): the model it shows an agent
//! — and the COBOL program that runs it — must themselves obey every layout
//! rule the text states, or the agent is taught one thing and shown another.

use cobolt_project_tools::content::APPLICATION_RULES;

/// The rules of the "layout" golden rule that a line-by-line scan can decide.
/// Returns one message per breach; empty means the source is in the layout.
fn lint(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut breaches = Vec::new();
    let mut seen_code = false;
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }
        let prev_blank = i > 0 && lines[i - 1].trim().is_empty();

        // (2) a blank line before every DIVISION and SECTION, except ENVIRONMENT DIVISION.
        let header = t.ends_with("DIVISION.") || t.ends_with("SECTION.");
        if header && seen_code && !t.starts_with("ENVIRONMENT") && !prev_blank {
            breaches.push(format!("line {n}: no blank line before `{t}`"));
        }
        seen_code = true;

        // (3) + (4) MOVE / SET: verb, then TO in column 50 followed by 4 spaces.
        for (verb, gap) in [("MOVE", 4usize), ("SET", 5usize)] {
            if !t.starts_with(&format!("{verb} ")) {
                continue;
            }
            let at = indent(line) + verb.len();
            if !(line[at..].starts_with(&" ".repeat(gap)) && !line[at + gap..].starts_with(' ')) {
                breaches.push(format!("line {n}: `{verb}` is not followed by {gap} spaces"));
            }
            match line.find(" TO ") {
                Some(p) => {
                    if p + 1 != 49 || !line[51..].starts_with("    ") {
                        breaches.push(format!("line {n}: TO is not in column 50 + 4 spaces: {t}"));
                    }
                }
                None => {
                    // The operand is too long: TO is in column 50 of the next line,
                    // and a blank line follows that statement.
                    let next = lines.get(i + 1).copied().unwrap_or("");
                    if !(next.trim_start().starts_with("TO ") && indent(next) == 49) {
                        breaches.push(format!("line {n}: TO is neither on this line nor in column 50 of the next"));
                    }
                    if !lines.get(i + 2).map_or(true, |l| l.trim().is_empty()) {
                        breaches.push(format!("line {n}: no blank line after a statement whose TO is on the next line"));
                    }
                }
            }
        }

        // (7) a method is a statement of its own: no INVOKE before object::method().
        if t.starts_with("INVOKE ") && t.contains("::") {
            breaches.push(format!("line {n}: INVOKE before a method call: {t}"));
        }

        // Comments: the code's column, and a blank line before the code below.
        if t.starts_with("*>") && !lines[i.saturating_sub(1)].trim_start().starts_with("*>") {
            let mut j = i;
            while lines.get(j).map_or(false, |l| l.trim_start().starts_with("*>")) {
                if indent(lines[j]) != indent(line) {
                    breaches.push(format!("line {}: comment lines start in different columns", j + 1));
                }
                j += 1;
            }
            if lines.get(j).map_or(false, |l| !l.trim().is_empty()) {
                breaches.push(format!("line {}: no blank line after the comment", j + 1));
            }
            if let Some(code) = lines[j..].iter().find(|l| !l.trim().is_empty()) {
                if indent(code) != indent(line) {
                    breaches.push(format!("line {n}: comment is not in the column of the code below it"));
                }
            }
        }
    }
    breaches
}

/// The model printed inside the rule — everything after "The model:".
fn model_in_rule() -> String {
    let (_, rule) = APPLICATION_RULES.iter().find(|(id, _)| *id == "layout").expect("a `layout` rule");
    rule.split_once("The model:\n").expect("the rule ends with its model").1.to_owned()
}

#[test]
fn the_model_in_the_layout_rule_obeys_the_layout() {
    let model = model_in_rule();
    let breaches = lint(&model);
    assert!(breaches.is_empty(), "{breaches:#?}\n{model}");
}

#[test]
fn the_layout_program_obeys_the_layout() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/code-style/test-layout-style.cbl");
    let src = std::fs::read_to_string(path).expect("read test-layout-style.cbl");
    let breaches = lint(&src);
    assert!(breaches.is_empty(), "{breaches:#?}");
}

/// The lint is not vacuous: it names each breach it is built to find.
#[test]
fn the_lint_finds_what_it_is_built_to_find() {
    let bad = "       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       PROCEDURE DIVISION.\n           MOVE 1 TO WS-A\n           INVOKE TXT-1::Clear()\n           *> comment\n           SET     WS-B                          TO    2\n";
    let breaches = lint(bad);
    for needle in ["no blank line before `WORKING-STORAGE SECTION.`", "`MOVE` is not followed by 4 spaces", "TO is not in column 50", "INVOKE before a method call", "no blank line"] {
        assert!(breaches.iter().any(|b| b.contains(needle)), "missing `{needle}` in {breaches:#?}");
    }
}
