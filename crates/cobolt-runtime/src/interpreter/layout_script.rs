// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Layout scripts (spec 109 S6, R32): `COBOL::Layout(action, container [, script])`.
//!
//! The script is read and checked here, lowered to `(control, property, value)`
//! lines by `cobolt_forms::layout_script`, and handed to the form host as one
//! write of the pseudo-property `_LayoutScript` on the form object. The host
//! installs the lines as breakpoint overrides — the solver is not touched and
//! the design is not written — so `rcrun run-form`, an embedded child form and
//! the compiled binary all take the same path. A program's own write to a
//! property still wins over the script (056 R64).

use cobolt_ast::stmt::CallArg;
use cobolt_forms::layout_script::{controls_named, parse, to_lines};
use cobolt_lexer::Span;

use super::Interpreter;
use crate::error::RuntimeError;

fn refuse(message: String) -> RuntimeError {
    RuntimeError::General { message: format!("COBOL::Layout: {message}") }
}

impl Interpreter {
    /// `CALL "COBOL-LAYOUT" USING action container [script]`.
    pub(super) fn layout_script_call(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 2 {
            return Err(refuse("takes an action and the container's name".into()));
        }
        let action = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_ascii_uppercase();
        let container = self.eval_call_arg(&using[1], span)?.as_display_string().trim().to_owned();
        if container.is_empty() {
            return Err(refuse("the container's name is empty".into()));
        }
        let owner = self.form_spelling(&container);
        let body = match action.as_str() {
            "APPLY" => {
                let Some(script) = using.get(2) else {
                    return Err(refuse("Apply needs the script as its third argument".into()));
                };
                let script = self.eval_call_arg(script, span)?.as_display_string();
                let sections = parse(&owner, &script).map_err(|e| refuse(format!("script for \"{owner}\", {e}")))?;
                if !self.control_ids.is_empty() {
                    for id in std::iter::once(owner.clone()).chain(controls_named(&sections)) {
                        if !self.control_ids.contains_key(&id.to_ascii_uppercase()) {
                            return Err(refuse(format!("this form has no control named \"{id}\"")));
                        }
                    }
                }
                to_lines(&sections)
            }
            "CLEAR" => String::new(),
            other => return Err(refuse(format!("unknown action \"{other}\" (Apply or Clear)"))),
        };
        let form = self.self_form_object.clone().unwrap_or_else(|| "ME".to_owned());
        self.obj_set(&form, "_LayoutScript", format!("{owner}\n{body}"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse as parse_cobol;

    fn run(body: &str, controls: &[&str]) -> Result<Interpreter, RuntimeError> {
        let source = format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nMAIN.\n{body}\n    STOP RUN.\n");
        let parsed = parse_cobol(tokenize(&source, SourceFormat::Free));
        let mut interp = Interpreter::new(parsed.program.expect("program parses"));
        interp.set_control_ids(controls.iter().copied());
        interp.run().map(|_| interp)
    }

    #[test]
    fn apply_hands_the_host_the_container_and_one_line_per_write() {
        let i = run(
            r#"    COBOL::Layout("Apply", "pnl-filters", ```
        MODE FLEX ROW
        GAP 14
        ORDER BTN-GO LV-DROP
        WHEN Compact
            MODE FLEX COLUMN
    ```)"#,
            &["PNL-FILTERS", "BTN-GO", "LV-DROP"],
        )
        .expect("runs");
        let sent = i.obj_get("ME", "_LayoutScript");
        let (owner, lines) = sent.split_once('\n').unwrap();
        assert_eq!(owner, "PNL-FILTERS", "the container as the form spells it");
        assert!(lines.contains("*\tPNL-FILTERS\tGap\t14"), "{lines}");
        assert!(lines.contains("*\tBTN-GO\tOrder\t1") && lines.contains("*\tLV-DROP\tOrder\t2"), "{lines}");
        assert!(lines.contains("Compact\tPNL-FILTERS\tFlexDirection\tColumn"), "{lines}");
    }

    #[test]
    fn clear_sends_the_owner_with_nothing_after_it() {
        let i = run(r#"    COBOL::Layout("Clear", "PNL")"#, &[]).expect("runs");
        assert_eq!(i.obj_get("ME", "_LayoutScript"), "PNL\n");
    }

    #[test]
    fn a_bad_script_an_unknown_control_or_an_unknown_action_stops_with_a_message() {
        for (body, controls, needle) in [
            (r#"    COBOL::Layout("Apply", "PNL", "MODE STACK")"#, vec![], "line 1"),
            (r#"    COBOL::Layout("Apply", "PNL")"#, vec![], "third argument"),
            (r#"    COBOL::Layout("Apply", "PNL", "ORDER A NOPE")"#, vec!["PNL", "A"], "no control named \"NOPE\""),
            (r#"    COBOL::Layout("Apply", "GONE", "GAP 1")"#, vec!["PNL"], "no control named \"GONE\""),
            (r#"    COBOL::Layout("Spin", "PNL")"#, vec![], "unknown action"),
        ] {
            let err = run(body, &controls).err().unwrap_or_else(|| panic!("{body} should stop")).to_string();
            assert!(err.contains(needle), "{body}: {err}");
        }
    }
}
