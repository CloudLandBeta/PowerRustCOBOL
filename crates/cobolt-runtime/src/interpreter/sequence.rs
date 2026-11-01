// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Animation sequences (spec 109 S7, R33): `COBOL::AnimationSequence(action, name
//! [, script | out-item])`.
//!
//! The sequences belong to this interpreter, so they belong to one form
//! instance: a form that closes takes its sequences with it, and two open forms
//! never share one. The entries are fired from the wait for the next event — the
//! one place the program is idle — which polls while a sequence plays and not
//! otherwise, so a form with no sequence waits exactly as before. An entry that
//! comes due while a handler runs fires when the handler returns.

use cobolt_ast::stmt::CallArg;
use cobolt_forms::sequence::{parse_script, SeqAction, SeqCallError};
use cobolt_lexer::Span;

use super::{call_arg_expr, Interpreter};
use crate::error::RuntimeError;

fn refuse(message: String) -> RuntimeError {
    RuntimeError::General { message: format!("COBOL::AnimationSequence: {message}") }
}

fn unknown(e: SeqCallError) -> RuntimeError {
    match e {
        SeqCallError::Unknown(n) => refuse(format!("there is no sequence named \"{n}\" (Create it first)")),
    }
}

impl Interpreter {
    /// The sequence clock: whole milliseconds since this interpreter began, or
    /// the value a test pinned.
    fn sequence_now_ms(&self) -> u64 {
        self.sequence_now.unwrap_or_else(|| self.sequence_epoch.elapsed().as_millis() as u64)
    }

    /// Pin the sequence clock (tests): the next `sequences_tick` reads `ms`.
    #[cfg(test)]
    pub(super) fn pin_sequence_clock(&mut self, ms: u64) {
        self.sequence_now = Some(ms);
    }

    /// `CALL "COBOL-ANIMATIONSEQUENCE" USING action name [script | out-item]`.
    pub(super) fn animation_sequence_call(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 2 {
            return Err(refuse("takes an action and a sequence name".into()));
        }
        let action = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_ascii_uppercase();
        let name = self.eval_call_arg(&using[1], span)?.as_display_string().trim().to_owned();
        if name.is_empty() {
            return Err(refuse("the sequence name is empty".into()));
        }
        let now = self.sequence_now_ms();
        match action.as_str() {
            "CREATE" => {
                let Some(script) = using.get(2) else {
                    return Err(refuse("Create needs the script as its third argument".into()));
                };
                let script = self.eval_call_arg(script, span)?.as_display_string();
                let entries = parse_script(&script).map_err(|e| refuse(format!("script of \"{name}\", {e}")))?;
                // A form that told the interpreter its controls gets its ids
                // checked now, so a typo is a line number and not a silent miss.
                if !self.control_ids.is_empty() {
                    for e in &entries {
                        if !self.control_ids.contains_key(&e.control.to_ascii_uppercase()) {
                            return Err(refuse(format!(
                                "script of \"{name}\", line {}: this form has no control named \"{}\"",
                                e.line, e.control
                            )));
                        }
                    }
                }
                self.sequences.create(&name, entries);
            }
            "START" => self.sequences.start(&name, now).map_err(unknown)?,
            "STOP" => self.sequences.stop(&name).map_err(unknown)?,
            "RESTART" => self.sequences.restart(&name, now).map_err(unknown)?,
            "DELETE" => self.sequences.delete(&name).map_err(unknown)?,
            "STATUS" => {
                let playing = self.sequences.is_playing(&name).map_err(unknown)?;
                let Some(out) = using.get(2) else {
                    return Err(refuse("Status answers into an item: COBOL::AnimationSequence(\"Status\", name, item)".into()));
                };
                let var = self.expr_to_name(call_arg_expr(out));
                self.env.set_str(&var, if playing { "Playing" } else { "Stopped" });
            }
            other => {
                return Err(refuse(format!(
                    "unknown action \"{other}\" (Create, Start, Stop, Restart, Status or Delete)"
                )));
            }
        }
        Ok(())
    }

    /// Fire every entry that has come due. Cheap when nothing plays.
    pub(super) fn sequences_tick(&mut self) {
        if !self.sequences.any_playing() {
            return;
        }
        let now = self.sequence_now_ms();
        for e in self.sequences.advance(now) {
            let ctrl = self.form_spelling(&e.control);
            match e.action {
                SeqAction::Show => self.obj_set(&ctrl, "Visible", "1".into()),
                SeqAction::Hide => self.obj_set(&ctrl, "Visible", "0".into()),
                SeqAction::Play(anim) => self.obj_set(&ctrl, "_PlayAnimation", anim),
            }
        }
    }

    /// How long the wait may sleep before the next entry is due, or `None` when
    /// no sequence plays.
    pub(super) fn sequences_wait_ms(&self) -> Option<u64> {
        self.sequences.next_due_in(self.sequence_now_ms())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    /// A free-format program whose MAIN holds `body`, run to its STOP RUN with
    /// the sequence clock pinned at 1000.
    fn run(body: &str) -> Interpreter {
        let source = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 WS-S PIC X(8).\n01 WS-SC PIC X(8).\nPROCEDURE DIVISION.\nMAIN.\n{body}\n    STOP RUN.\n"
        );
        let parsed = parse(tokenize(&source, SourceFormat::Free));
        let mut interp = Interpreter::new(parsed.program.expect("program parses"));
        interp.pin_sequence_clock(1000);
        interp.run().expect("runs");
        interp
    }

    fn at(interp: &mut Interpreter, ms: u64) {
        interp.pin_sequence_clock(ms);
        interp.sequences_tick();
    }

    #[test]
    fn a_created_and_started_sequence_fires_its_entries_on_the_clock() {
        // The script is a ``` block, as the Guide writes it.
        let mut i = run(
            r#"    COBOL::AnimationSequence("Create", "INTRO", ```
        SHOW C-KPI                  AFTER 0
        SHOW C-BAR                  AFTER 50

        PLAY    C-RADAR "fade-in"   AFTER 100
        HIDE    LBL-WAIT            AFTER 100
    ```)
    COBOL::AnimationSequence("Start", "INTRO")
    COBOL::AnimationSequence("Status", "INTRO", WS-S)"#,
        );
        assert_eq!(i.env.get("WS-S").map(|v| v.as_display_string()).as_deref().map(str::trim), Some("Playing"));
        at(&mut i, 1000);
        assert_eq!(i.obj_get("C-KPI", "Visible"), "true");
        assert_eq!(i.obj_get("C-BAR", "Visible"), "", "not yet: 50 ms are still to pass");
        at(&mut i, 1049);
        assert_eq!(i.obj_get("C-BAR", "Visible"), "");
        at(&mut i, 1050);
        assert_eq!(i.obj_get("C-BAR", "Visible"), "true");
        at(&mut i, 1100);
        assert_eq!(i.obj_get("C-RADAR", "_PlayAnimation"), "fade-in");
        assert_eq!(i.obj_get("LBL-WAIT", "Visible"), "false");
        assert!(i.sequences_wait_ms().is_none(), "nothing left to wait for");
    }

    #[test]
    fn stop_drops_the_entries_still_to_come_and_status_says_stopped() {
        let mut i = run(
            "    COBOL::AnimationSequence(\"Create\", \"INTRO\", \"SHOW A AFTER 0; SHOW B AFTER 100\")\n\
             \x20   COBOL::AnimationSequence(\"Start\", \"INTRO\")\n\
             \x20   COBOL::AnimationSequence(\"Stop\", \"INTRO\")\n\
             \x20   COBOL::AnimationSequence(\"Status\", \"INTRO\", WS-S)",
        );
        assert_eq!(i.env.get("WS-S").map(|v| v.as_display_string()).as_deref().map(str::trim), Some("Stopped"));
        at(&mut i, 5000);
        assert_eq!(i.obj_get("A", "Visible"), "");
        assert_eq!(i.obj_get("B", "Visible"), "");
    }

    #[test]
    fn a_bad_script_or_an_unknown_name_stops_the_run_with_a_message() {
        let source = |body: &str| {
            format!(
                "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nMAIN.\n{body}\n    STOP RUN.\n"
            )
        };
        for (body, needle) in [
            ("    COBOL::AnimationSequence(\"Create\", \"X\", \"FLY A AFTER 1\")", "line 1"),
            ("    COBOL::AnimationSequence(\"Start\", \"NOPE\")", "no sequence named"),
            ("    COBOL::AnimationSequence(\"Spin\", \"X\")", "unknown action"),
            ("    COBOL::AnimationSequence(\"Status\", \"X\")", "no sequence named"),
        ] {
            let parsed = parse(tokenize(&source(body), SourceFormat::Free));
            let mut interp = Interpreter::new(parsed.program.expect("program parses"));
            let err = interp.run().expect_err(body).to_string();
            assert!(err.contains(needle), "{body}: {err}");
        }
    }

    #[test]
    fn a_form_that_named_its_controls_gets_a_typo_reported_at_create() {
        let source = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nMAIN.\n\
                      \x20   COBOL::AnimationSequence(\"Create\", \"X\", ```\nSHOW C-KPI\nSHOW C-BRA AFTER 5\n```)\n    STOP RUN.\n";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let mut interp = Interpreter::new(parsed.program.expect("program parses"));
        interp.set_control_ids(["C-KPI", "C-BAR"]);
        let err = interp.run().expect_err("typo").to_string();
        assert!(err.contains("line 2") && err.contains("C-BRA"), "{err}");
    }
}
