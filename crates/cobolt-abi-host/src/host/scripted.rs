// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A form's animation sequences and its layout scripts on the desktop (DESKTOP-WASM-001 T49, ABI A57):
//! `COBOL::"ANIMATIONSEQUENCE"` and `COBOL::"LAYOUT"`, by the interpreter's own rules
//! (`cobolt_runtime`'s `interpreter/sequence.rs` and `interpreter/layout_script.rs`) over the model's
//! `cobolt_forms::sequence` and `cobolt_forms::layout_script`.
//!
//! The sequences belong to the form instance, as they belong to each form's interpreter: a form that
//! closes takes its sequences with it, and two open forms never share one. The runner fires the entries
//! that came due on its clock ([`Host::set_clock`]) between handlers ([`Host::sequences_tick`]), as the
//! interpreter fires them from its wait for the next event, so an entry that comes due while a handler
//! runs fires when the handler returns. The names a script may use are the controls of the form's design,
//! nested ones included, as the interpreter is told them (`Interpreter::set_control_ids`, from the form's
//! flat `controls`).

use cobolt_forms::sequence::{parse_script, SeqAction, SeqCallError};

use super::{CmdError, Host};

fn sequence_refusal(message: String) -> CmdError {
    CmdError::Bad(format!("COBOL::AnimationSequence: {message}"))
}

fn unknown(e: SeqCallError) -> CmdError {
    match e {
        SeqCallError::Unknown(n) => sequence_refusal(format!("there is no sequence named \"{n}\" (Create it first)")),
    }
}

fn layout_refusal(message: String) -> CmdError {
    CmdError::Bad(format!("COBOL::Layout: {message}"))
}

impl Host {
    /// The clock the sequences run on, in whole milliseconds from any fixed instant (the runner's: the form
    /// host's, virtual in a deterministic run). A host without one reads 0.
    pub fn set_clock(&mut self, clock: fn() -> u64) {
        self.clock = Some(clock);
    }

    fn clock_ms(&self) -> u64 {
        self.clock.map_or(0, |f| f())
    }

    /// The controls of the instance's form, nested ones included, as the design spells them, by upper-case
    /// name: what the interpreter is told the form's controls are (the design's flat `controls`).
    fn own_controls(&self, inst: u32) -> std::collections::HashMap<String, String> {
        let Some(form) = self.trees.get(&inst).and_then(|t| self.forms.get(t.form_idx)).and_then(Option::as_ref) else { return Default::default() };
        form.controls.iter().map(|c| (c.id.to_ascii_uppercase(), c.id.clone())).collect()
    }

    /// `COBOL::"ANIMATIONSEQUENCE" ( action name [script | state] )`. `Status` writes `Playing` or `Stopped`
    /// into the application's `BuiltinAnswer3`; any other action writes the third argument there as it was
    /// sent, for a call whose action the Build could not read (`lower_form`'s `desktop_sequence`).
    pub(super) fn sequence_call(&mut self, inst: u32, args: &[&str]) -> Result<(), CmdError> {
        self.app.remove("builtinanswer3");
        if args.len() < 2 {
            return Err(sequence_refusal("takes an action and a sequence name".into()));
        }
        let action = args[0].trim().to_ascii_uppercase();
        let name = args[1].trim().to_owned();
        if name.is_empty() {
            return Err(sequence_refusal("the sequence name is empty".into()));
        }
        if let Some(third) = args.get(2) {
            self.app.insert("builtinanswer3".into(), (*third).to_owned());
        }
        let now = self.clock_ms();
        let own = self.own_controls(inst);
        let set = self.sequences.entry(inst).or_default();
        match action.as_str() {
            "CREATE" => {
                let Some(script) = args.get(2) else {
                    return Err(sequence_refusal("Create needs the script as its third argument".into()));
                };
                let entries = parse_script(script).map_err(|e| sequence_refusal(format!("script of \"{name}\", {e}")))?;
                if !own.is_empty() {
                    for e in &entries {
                        if !own.contains_key(&e.control.to_ascii_uppercase()) {
                            return Err(sequence_refusal(format!("script of \"{name}\", line {}: this form has no control named \"{}\"", e.line, e.control)));
                        }
                    }
                }
                set.create(&name, entries);
            }
            "START" => set.start(&name, now).map_err(unknown)?,
            "STOP" => set.stop(&name).map_err(unknown)?,
            "RESTART" => set.restart(&name, now).map_err(unknown)?,
            "DELETE" => set.delete(&name).map_err(unknown)?,
            "STATUS" => {
                let playing = set.is_playing(&name).map_err(unknown)?;
                if args.len() < 3 {
                    return Err(sequence_refusal("Status answers into an item: COBOL::AnimationSequence(\"Status\", name, item)".into()));
                }
                self.app.insert("builtinanswer3".into(), if playing { "Playing" } else { "Stopped" }.into());
            }
            other => return Err(sequence_refusal(format!("unknown action \"{other}\" (Create, Start, Stop, Restart, Status or Delete)"))),
        }
        Ok(())
    }

    /// Fire every entry that has come due on the clock, in every instance not inside a handler (whose
    /// entries fire when it is free, as the interpreter's fire at its wait for the next event): `SHOW` and
    /// `HIDE` write the control's `Visible`, `PLAY` starts its animation, as the interpreter's writes do.
    /// Answers whether any fired.
    pub fn sequences_tick(&mut self) -> bool {
        if !self.sequences.values().any(|s| s.any_playing()) {
            return false;
        }
        let now = self.clock_ms();
        let mut fired = false;
        let insts: Vec<u32> = self.sequences.keys().copied().filter(|&i| !self.busy(i)).collect();
        for inst in insts {
            let due = self.sequences.get_mut(&inst).map(|s| s.advance(now)).unwrap_or_default();
            for e in due {
                let Some(tree) = self.trees.get_mut(&inst) else { continue };
                let Some(ctl) = tree.by_name(&e.control) else { continue };
                fired = true;
                match e.action {
                    SeqAction::Show | SeqAction::Hide => {
                        let value = if e.action == SeqAction::Show { "1" } else { "0" };
                        let ev = tree.set(ctl, "Visible", value);
                        self.enqueue_requests(inst, ev);
                    }
                    SeqAction::Play(anim) => tree.command(ctl, "playanimation", vec![anim]),
                }
            }
        }
        fired
    }

    /// How long until the next entry is due, in milliseconds, or `None` when no sequence plays.
    pub fn sequences_due_in(&self) -> Option<u64> {
        let now = self.clock_ms();
        self.sequences.values().filter_map(|s| s.next_due_in(now)).min()
    }

    /// `COBOL::"LAYOUT" ( action container [script] )`: `Apply` reads the script for the container and
    /// writes it, one override per line after the container's name, to the form's `_LayoutScript`, which
    /// the form host installs as breakpoint overrides; `Clear` writes the name alone.
    pub(super) fn layout_call(&mut self, inst: u32, args: &[&str]) -> Result<(), CmdError> {
        use cobolt_forms::layout_script::{controls_named, parse, to_lines};
        if args.len() < 2 {
            return Err(layout_refusal("takes an action and the container's name".into()));
        }
        let action = args[0].trim().to_ascii_uppercase();
        let container = args[1].trim().to_owned();
        if container.is_empty() {
            return Err(layout_refusal("the container's name is empty".into()));
        }
        let own = self.own_controls(inst);
        let owner = own.get(&container.to_ascii_uppercase()).cloned().unwrap_or(container);
        let body = match action.as_str() {
            "APPLY" => {
                let Some(script) = args.get(2) else {
                    return Err(layout_refusal("Apply needs the script as its third argument".into()));
                };
                let sections = parse(&owner, script).map_err(|e| layout_refusal(format!("script for \"{owner}\", {e}")))?;
                if !own.is_empty() {
                    for id in std::iter::once(owner.clone()).chain(controls_named(&sections)) {
                        if !own.contains_key(&id.to_ascii_uppercase()) {
                            return Err(layout_refusal(format!("this form has no control named \"{id}\"")));
                        }
                    }
                }
                to_lines(&sections)
            }
            "CLEAR" => String::new(),
            other => return Err(layout_refusal(format!("unknown action \"{other}\" (Apply or Clear)"))),
        };
        let Some(tree) = self.trees.get_mut(&inst) else { return Ok(()) };
        let ev = tree.set(0, "_LayoutScript", &format!("{owner}\n{body}"));
        self.enqueue_requests(inst, ev);
        Ok(())
    }
}
