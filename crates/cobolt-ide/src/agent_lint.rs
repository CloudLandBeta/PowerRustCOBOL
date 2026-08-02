// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Compile-check a change-set before any Pedantic round is spent on it.
//!
//! The machine-validation gate ([`crate::agent::lint_change_set_submission`])
//! already runs ahead of the reviewer, but it only checked STRUCTURE — control
//! types, property keys, event names, the three division headers. It never
//! parsed a line of COBOL, so everything the language actually decides was left
//! to an LLM's opinion, and the reviewer spent rounds on it. In one observed
//! workflow the reviewer raised six defects of which three were false (it
//! demanded `PERFORM` where the contract mandates `CALL`, demanded a
//! `SPECIAL-NAMES` paragraph the contract forbids in a nested body, and
//! demanded removal of an `EXIT` that is in the verb list), while two real ones
//! — comma literals with no `DECIMAL-POINT IS COMMA` in force, and `::Text` on
//! a Label whose text property is `Caption` — passed every gate untouched.
//!
//! Every one of those is a question the toolchain answers exactly. This module
//! asks it: apply the change-set to a COPY of the form, generate the `.cbl`
//! codegen would really produce, then run the same lexer, parser and semantic
//! analyzer the runner uses, and attribute each hard error back to the
//! operation whose body contains it.
//!
//! **Nothing here touches disk or the live form.** The probe is a cloned
//! [`Form`], `cobolt_codegen::generate*` returns a `String`, and neither
//! `cobolt-parser` nor `cobolt-semantic` performs any I/O. A crash mid-check
//! loses heap and nothing else. Validation deliberately runs on a bare `Form`
//! rather than through a `DesignerPanel`: a panel owns save machinery
//! (`cfrm_dir`, `dirty`, undo stacks), and while that is inert here today,
//! validating on a `Form` makes this path structurally unable to persist
//! anything, now or later.

use cobolt_forms::model::{Control, ControlType, Form, PropValue};
use cobolt_lexer::{tokenize, SourceFormat};

use crate::agent::{AgentChangeSet, AgentOp};

/// One proven defect: the operation it belongs to, and what the toolchain said.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileDefect {
    /// `op_ref`-style label, or `None` when the diagnostic landed in generated
    /// scaffolding rather than in any operation's own body. An unattributed
    /// defect must not be blamed on an arbitrary operation — the correction
    /// scoper falls back to a full replacement instead.
    pub op: Option<String>,
    pub message: String,
}

/// Apply the semantic content of a change-set to `form`.
///
/// This reproduces what reaches GENERATED COBOL — controls and their ids and
/// types, properties, handler bodies, procedures, form structure blocks — and
/// deliberately not the designer's geometry behaviour (grid snapping, container
/// carry, overlap nudging). `X`/`Y` cannot change whether a program compiles,
/// and reproducing the panel here would couple validation to a type that owns
/// save machinery.
pub fn apply_for_probe(form: &mut Form, cs: &AgentChangeSet) {
    for op in &cs.operations {
        match op {
            AgentOp::DeployControl {
                control_type,
                id,
                parent_id,
                parent,
                properties,
            } => {
                let ct = ControlType::from_str(control_type);
                let cid = id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{control_type}-probe{}", form.controls.len()));
                // A redeploy of an existing id updates that control, exactly as
                // the real applier does — otherwise the probe would see two
                // controls where the form will hold one.
                let existing = form
                    .controls
                    .iter()
                    .position(|c| c.id.eq_ignore_ascii_case(&cid));
                let idx = match existing {
                    Some(i) => i,
                    None => {
                        form.controls.push(Control::new(cid.clone(), ct, 0, 0));
                        form.controls.len() - 1
                    }
                };
                if let Some(p) = parent.as_ref().or(parent_id.as_ref()) {
                    form.controls[idx].parent = Some(p.clone());
                }
                for (k, v) in properties {
                    if let Some(pv) = json_value_to_prop(v) {
                        form.controls[idx].set_prop(k, pv);
                    }
                }
            }
            AgentOp::SetProperty {
                control_id,
                key,
                value,
            } => {
                let Some(pv) = json_value_to_prop(value) else {
                    continue;
                };
                if let Some(c) = form
                    .controls
                    .iter_mut()
                    .find(|c| c.id.eq_ignore_ascii_case(control_id))
                {
                    c.set_prop(key, pv);
                }
            }
            AgentOp::GenerateEventHandler {
                control_id,
                event,
                code,
            } => {
                if let Some(c) = form
                    .controls
                    .iter_mut()
                    .find(|c| c.id.eq_ignore_ascii_case(control_id))
                {
                    let body = crate::llm::normalize_comments(code);
                    match c.events.iter_mut().find(|b| b.event.eq_ignore_ascii_case(event)) {
                        Some(b) => b.code = body,
                        None => c.events.push(cobolt_forms::model::EventBinding {
                            event: event.clone(),
                            paragraph: format!(
                                "{}--{}",
                                cobolt_codegen::cobol_word(&c.id),
                                event.to_ascii_uppercase()
                            ),
                            code: body,
                        }),
                    }
                }
            }
            AgentOp::CreateProcedure { name, code } => {
                let body = crate::llm::normalize_comments(code);
                match form
                    .user_procedures
                    .iter_mut()
                    .find(|p| p.name.eq_ignore_ascii_case(name))
                {
                    Some(p) => p.code = body,
                    None => form.user_procedures.push(cobolt_forms::model::UserProcedure {
                        name: name.clone(),
                        code: body,
                    }),
                }
            }
            AgentOp::SetFormStructure { block, code } => {
                if let Some(slot) = crate::agent::form_structure_field(form, block) {
                    *slot = crate::llm::normalize_comments(code);
                }
            }
            AgentOp::Message { .. } => {}
        }
    }
}

fn json_value_to_prop(v: &serde_json::Value) -> Option<PropValue> {
    match v {
        serde_json::Value::String(s) => Some(PropValue::String(s.clone())),
        serde_json::Value::Bool(b) => Some(PropValue::Bool(*b)),
        serde_json::Value::Number(n) => n.as_i64().map(PropValue::Int),
        _ => None,
    }
}

/// Compile the form as it would stand after `dependencies` and then `cs`, and
/// return every hard error the toolchain proves.
///
/// `dependencies` are the change-sets of approved upstream tasks, applied first.
/// They matter: a task that only adds handlers runs while its controls still
/// live in an earlier task's unapplied change-set, and validating it alone
/// would report every control as missing.
///
/// Warnings are deliberately excluded. A warning is not proof, and a gate that
/// spends a correction round on one is the false-positive tax this module
/// exists to remove.
pub fn compile_defects(
    live: &Form,
    dependencies: &[AgentChangeSet],
    cs: &AgentChangeSet,
) -> Vec<CompileDefect> {
    let mut probe = live.clone();
    for dep in dependencies {
        apply_for_probe(&mut probe, dep);
    }
    apply_for_probe(&mut probe, cs);

    let (source, user_lines) = cobolt_codegen::generate_with_user_lines(&probe);
    let tokens = tokenize(&source, SourceFormat::detect(&source));
    let parsed = cobolt_parser::parse(tokens);

    let mut out: Vec<CompileDefect> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == cobolt_parser::Severity::Error)
        .map(|d| CompileDefect {
            op: attribute(cs, &source, &user_lines, d.span.line),
            message: d.message.clone(),
        })
        .collect();

    // Semantic analysis only runs on a program the parser recovered; feeding it
    // a half-parsed tree produces cascade errors that blame the wrong operation.
    if out.is_empty() {
        if let Some(program) = parsed.program {
            let sem = cobolt_semantic::analyze(&program);
            out.extend(
                sem.diagnostics
                    .iter()
                    .filter(|d| d.severity == cobolt_semantic::Severity::Error)
                    .map(|d| CompileDefect {
                        op: attribute(cs, &source, &user_lines, d.span.line),
                        message: d.message.clone(),
                    }),
            );
        }
    }
    out
}

/// Map a 1-based line of the generated source back to the operation whose body
/// occupies it.
///
/// `user_lines` are codegen's own ranges of developer-authored code, in emission
/// order. A line outside every range is generated scaffolding and belongs to no
/// operation.
fn attribute(
    cs: &AgentChangeSet,
    source: &str,
    user_lines: &[(u32, u32)],
    line: u32,
) -> Option<String> {
    if !user_lines.iter().any(|(a, b)| line >= *a && line <= *b) {
        return None;
    }
    // The enclosing nested program names itself: `PROGRAM-ID. BTN-OK--ONCLICK.`
    // for a handler, `PROGRAM-ID. RECALC IS COMMON PROGRAM.` for a procedure.
    let above: Vec<&str> = source.lines().take(line as usize).collect();
    let prog = above.iter().rev().find_map(|l| {
        let t = l.trim();
        t.strip_prefix("PROGRAM-ID.")
            .map(|rest| rest.trim().trim_end_matches('.').to_ascii_uppercase())
    })?;
    let prog_name = prog
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();

    cs.operations.iter().find_map(|op| match op {
        AgentOp::GenerateEventHandler {
            control_id, event, ..
        } => {
            let expect = format!(
                "{}--{}",
                cobolt_codegen::cobol_word(control_id).to_ascii_uppercase(),
                event.to_ascii_uppercase()
            );
            (expect == prog_name).then(|| format!("generate_event_handler {control_id}.{event}"))
        }
        AgentOp::CreateProcedure { name, .. } => {
            (cobolt_codegen::cobol_word(name).to_ascii_uppercase() == prog_name)
                .then(|| format!("create_procedure {name}"))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::parse_change_set;

    fn form() -> Form {
        Form::new("CHECKBOXES-FORM", "Checkboxes", 640, 480)
    }

    /// A clean change-set must produce ZERO defects. A false positive here is
    /// worse than the gap this closes, because it burns the correction budget
    /// deterministically, every run.
    #[test]
    fn a_valid_change_set_is_silent() {
        let cs = parse_change_set(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"CheckBox","id":"CHK-1","properties":{"Caption":"Big Mac"}},
  {"op":"generate_event_handler","control_id":"CHK-1","event":"onCheckedChanged","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01  WS-N  PIC S9(4) COMP-5 VALUE 0.\n\n       PROCEDURE DIVISION.\n           ADD 1 TO WS-N."}
]}"#,
        )
        .unwrap();
        let defects = compile_defects(&form(), &[], &cs);
        assert!(defects.is_empty(), "clean change-set flagged: {defects:?}");
    }

    /// The probe must see what earlier approved tasks created. Validating a
    /// handler task alone would report its control as missing — which is what
    /// the live workflow does today, running T2 with `CONTROLS: (none)`.
    #[test]
    fn dependencies_are_applied_before_the_submission() {
        let deps = vec![parse_change_set(
            r#"{"operations":[{"op":"deploy_control","control_type":"CheckBox","id":"CHK-1","properties":{}}]}"#,
        )
        .unwrap()];
        let cs = parse_change_set(
            r#"{"operations":[
  {"op":"generate_event_handler","control_id":"CHK-1","event":"onCheckedChanged","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."}
]}"#,
        )
        .unwrap();
        assert!(
            compile_defects(&form(), &deps, &cs).is_empty(),
            "the dependency's control must be visible to the probe"
        );
    }

    /// The probe is a clone: the live form is untouched, and nothing is written.
    #[test]
    fn the_live_form_is_never_mutated() {
        let live = form();
        let before = cobolt_codegen::generate(&live);
        let cs = parse_change_set(
            r#"{"operations":[{"op":"set_form_structure","block":"SPECIAL-NAMES","code":"       DECIMAL-POINT IS COMMA."}]}"#,
        )
        .unwrap();
        let _ = compile_defects(&live, &[], &cs);
        assert_eq!(
            cobolt_codegen::generate(&live),
            before,
            "validation must not mutate the form it was handed"
        );
        assert!(live.cobol_structure.special_names.trim().is_empty());
    }

    /// The deadlock, settled by the toolchain instead of argued over.
    ///
    /// The reviewer demanded `PERFORM UPDATE-RECEIPT` for a common procedure,
    /// which is a SEPARATE program: procedure names are never visible outside
    /// the program declaring them, so that `PERFORM` can never resolve, and
    /// three correction rounds went on disputing it. `cobolt-semantic` now
    /// analyzes contained programs and reports the missing target as an error,
    /// so the gate proves it before a review round is spent.
    ///
    /// Two changes were needed and only together: the severity (a target that
    /// cannot resolve in any run is not a warning) AND the traversal (handler
    /// bodies are contained programs, and nothing used to analyze them).
    #[test]
    fn a_dangling_perform_is_proven_wrong() {
        let cs = parse_change_set(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"CheckBox","id":"CHK-1","properties":{}},
  {"op":"create_procedure","name":"UPDATE-RECEIPT","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."},
  {"op":"generate_event_handler","control_id":"CHK-1","event":"onCheckedChanged","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           PERFORM UPDATE-RECEIPT."}
]}"#,
        )
        .unwrap();
        let defects = compile_defects(&form(), &[], &cs);
        assert!(!defects.is_empty(), "the dangling PERFORM must be caught");
        assert!(
            defects
                .iter()
                .any(|d| d.message.contains("same program")),
            "the diagnostic must say why it cannot resolve: {defects:?}"
        );
        assert!(
            defects.iter().any(|d| d.op.as_deref()
                == Some("generate_event_handler CHK-1.onCheckedChanged")),
            "and name the operation carrying it: {defects:?}"
        );

        // CALL is the correct form for a common procedure, and must stay clean.
        let with_call = parse_change_set(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"CheckBox","id":"CHK-1","properties":{}},
  {"op":"create_procedure","name":"UPDATE-RECEIPT","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."},
  {"op":"generate_event_handler","control_id":"CHK-1","event":"onCheckedChanged","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CALL \"UPDATE-RECEIPT\"."}
]}"#,
        )
        .unwrap();
        assert!(
            compile_defects(&form(), &[], &with_call).is_empty(),
            "CALL reaches a common procedure and must not be flagged"
        );
    }

    /// `VALUE 8,49` with no `DECIMAL-POINT IS COMMA` in force used to take the
    /// value 8 in silence — neither a valid numeric literal (the decimal point
    /// is `.` here) nor a valid separator comma (COBOL-85 wants a space after
    /// one), and reported by nobody. The parser now names it and says where to
    /// declare the convention, and the gate proves it before a review round is
    /// spent.
    #[test]
    fn a_comma_literal_without_the_clause_is_proven_wrong() {
        let handler = r#"{"operations":[
  {"op":"deploy_control","control_type":"CheckBox","id":"CHK-1","properties":{}},
  {"op":"generate_event_handler","control_id":"CHK-1","event":"onCheckedChanged","code":"       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01  WS-PRICE  PIC 9(5)V99 VALUE 8,49.\n\n       PROCEDURE DIVISION.\n           CONTINUE."}
]}"#;
        let defects = compile_defects(&form(), &[], &parse_change_set(handler).unwrap());
        assert!(!defects.is_empty(), "the comma literal must be caught");
        assert!(
            defects.iter().any(|d| d.message.contains("DECIMAL-POINT IS COMMA")),
            "the diagnostic must point at the cause: {defects:?}"
        );
        assert!(
            defects.iter().any(|d| d.op.as_deref()
                == Some("generate_event_handler CHK-1.onCheckedChanged")),
            "and at the operation carrying it: {defects:?}"
        );

        // And the SAME code is legal once the change-set declares the
        // convention on the form — which is what `set_form_structure` is for.
        let with_clause = handler.replace(
            r#"{"operations":["#,
            "{\"operations\":[\n  {\"op\":\"set_form_structure\",\"block\":\"SPECIAL-NAMES\",\"code\":\"       DECIMAL-POINT IS COMMA.\"},",
        );
        assert!(
            compile_defects(&form(), &[], &parse_change_set(&with_clause).unwrap()).is_empty(),
            "with the clause in force, 8,49 is a decimal literal"
        );
    }

    /// `set_form_structure` reaches the probe, so a change-set that adds
    /// `DECIMAL-POINT IS COMMA` is judged with the clause in force.
    #[test]
    fn form_structure_from_the_same_change_set_is_in_force() {
        let cs = parse_change_set(
            r#"{"operations":[
  {"op":"set_form_structure","block":"SPECIAL-NAMES","code":"       DECIMAL-POINT IS COMMA."}
]}"#,
        )
        .unwrap();
        let mut probe = form();
        apply_for_probe(&mut probe, &cs);
        assert!(cobolt_codegen::generate(&probe).contains("DECIMAL-POINT IS COMMA"));
    }
}
