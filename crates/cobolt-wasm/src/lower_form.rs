// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A form's controls and the host: `Ctrl::Property`, `Ctrl::Method(args)`,
//! `INVOKE`, the built-ins that belong to the page, and `WAIT-EVENT`.
//!
//! None of it touches a control's state in the module. A property is a *path*
//! (WASM-ABI-v1 §5) — instance, control, segments with their argument texts —
//! carried to the host in the command batch: written with `SET`, called with
//! `CALL`, read with a query that returns text. A read leaves its answer in a
//! character temporary that behaves as a number when it spells one, which is how
//! `IF NUD-AGE::Value > 17` compares numerically while `MOVE TXT::Text TO
//! WS-NAME` moves characters.

use crate::error::GenResult;
use crate::ir::*;
use crate::lower::Lower;
use cobolt_ast::expr::*;
use cobolt_ast::stmt::*;
use cobolt_wasm_rt::cmd::{CMD_CALL, Q_CALLR, Q_CALLRW, Q_CALLRWS, Q_GET};
use wasm_encoder::Instruction as I;

/// The reserved control ids of ABI v1 §5.
pub const CTRL_FORM: u32 = 0;
pub const CTRL_SUPER: u32 = 1;
pub const CTRL_APP: u32 = 2;

/// One segment of a member chain: the member's name and its arguments, and
/// whether the source wrote parentheses.
pub(crate) struct Seg<'e> {
    pub name: &'e str,
    pub args: &'e [Expr],
    pub parens: bool,
}

impl Lower<'_, '_> {
    /// A member chain's root and segments, outermost call last.
    pub(crate) fn chain<'e>(e: &'e Expr) -> Option<(&'e Expr, Vec<Seg<'e>>)> {
        let mut segs = Vec::new();
        let mut cur = e;
        while let Expr::Member { recv, member, args, parens, .. } = cur {
            segs.push(Seg { name: member.as_str(), args: args.as_slice(), parens: *parens });
            cur = recv.as_ref();
        }
        if segs.is_empty() {
            return None;
        }
        segs.reverse();
        Some((cur, segs))
    }

    /// The control id a chain's root names, when the root is the form itself,
    /// the form that opened it, the application object or a control of the
    /// form — and `None` when it is a data item (a value method).
    pub(crate) fn host_root(&mut self, root: &Expr) -> Option<u32> {
        let name = match root {
            Expr::Identifier(n, _) => n.to_ascii_uppercase(),
            _ => return None,
        };
        match name.as_str() {
            "ME" | "THIS" => return Some(CTRL_FORM),
            "SUPER" => return Some(CTRL_SUPER),
            "COBOL" => return Some(CTRL_APP),
            _ => {}
        }
        // A data item of the program wins over a control of the same name only
        // when the form has no such control (the codegen names its fields apart).
        if let Some(f) = &self.form {
            if let Some(&id) = f.controls.get(&name) {
                return Some(id);
            }
            if let Some(&id) = f.layers.get(&name) {
                return Some(id);
            }
        }
        None
    }

    /// Is `ctrl` one of the form's layers?
    fn is_layer(&self, ctrl: u32) -> bool {
        self.form.as_ref().is_some_and(|f| f.layers.values().any(|&id| id == ctrl))
    }

    /// The rules of spec 091 for a member chain on the host (R19, R37, Q11): a
    /// layer has its properties and nothing else, and `Name` is read only; a
    /// control's `Layer` is read only.
    fn check_layer_rules(&self, root: &Expr, ctrl: u32, segs: &[Seg<'_>], write: bool, line: u32) -> GenResult<()> {
        use crate::error::ErrorKind;
        use cobolt_forms::model::{layer_prop, layer_writable};
        let root_name = match root {
            Expr::Identifier(n, _) => n.as_str(),
            _ => "",
        };
        if self.is_layer(ctrl) {
            let prop = match segs {
                [s] if s.args.is_empty() && !s.parens => s.name,
                _ => {
                    let what = segs.iter().map(|s| s.name).collect::<Vec<_>>().join("::");
                    return self.err_kind(line, ErrorKind::UnknownLayerProperty, what.clone(), format!("{root_name}::{what}: a layer has no members but its properties; a layer has {}", layer_props_text()));
                }
            };
            if !layer_prop(prop) {
                return self.err_kind(line, ErrorKind::UnknownLayerProperty, prop, format!("the layer {root_name} has no property {prop}; a layer has {}", layer_props_text()));
            }
            if write && !layer_writable(prop) {
                return self.err(line, format!("{root_name}::{prop} can be read but not written: a layer is renamed in the designer"));
            }
        } else if write && ctrl >= 3 && segs.last().is_some_and(|s| s.name.eq_ignore_ascii_case("Layer")) {
            return self.err_kind(line, ErrorKind::LayerWrite, format!("{root_name}::Layer"), format!("{root_name}::Layer can be read but not written: a control is moved to another layer in the designer"));
        }
        Ok(())
    }

    /// A chain whose root is neither the host, a control, a layer nor a data
    /// item: on a form that has layers it is most likely a layer's name
    /// mistyped, and the refusal names the layers there are (spec 091 R37).
    fn unknown_root(&mut self, root: &Expr, line: u32) -> Option<crate::error::GenError> {
        let Expr::Identifier(name, _) = root else { return None };
        if self.resolve_item(root).is_ok() {
            return None;
        }
        // `COBOL::SqlDatabase::…` reaches here as the one word the parser makes of it: the database is the Backend's.
        if cobolt_ast::methods::is_sql_database_object(name) {
            return Some(crate::error::GenError::kinded(
                line,
                crate::error::ErrorKind::BackendOnly,
                "COBOL::SqlDatabase",
                crate::support::SQL_DATABASE_OBJECT,
            ));
        }
        let f = self.form.as_ref()?;
        if f.layer_names.is_empty() {
            return None;
        }
        Some(crate::error::GenError::kinded(
            line,
            crate::error::ErrorKind::UnknownLayer,
            name.clone(),
            format!(
                "{name} is not a layer of form {}: its layers are {}; a layer has {}",
                f.name,
                f.layer_names.join(", "),
                layer_props_text()
            ),
        ))
    }

    /// A desktop console module has no form, controls or application object, and a built-in
    /// waits for its own milestone (DESKTOP-WASM-001 T10, T16): a member that reaches the host is
    /// refused there with the reason of its Desktop class, so the Build takes the Cargo path. A
    /// desktop module with forms has them (M2, T30): there only a built-in that waits for a later
    /// milestone than the forms' is refused.
    /// `root` is the chain's first name and `first` its first segment's; on the application object
    /// the segment names the built-in.
    pub(crate) fn desktop_gate(&self, ctrl: u32, root: &Expr, first: Option<&str>, line: u32) -> GenResult<()> {
        if !self.native {
            return Ok(());
        }
        // A desktop module with forms is run by the runner's ABI host, which serves the form, its controls and
        // the application object (M2, T30): only what waits for a later milestone is refused.
        let forms_served = self.form.is_some();
        if forms_served && ctrl != CTRL_APP {
            return Ok(());
        }
        let root = match root {
            Expr::Identifier(n, _) => n.as_str(),
            _ => "",
        };
        if ctrl == CTRL_APP {
            if let Some(b) = first.and_then(|n| crate::support::builtin_entry(&n.to_ascii_uppercase())) {
                let why = match b.native {
                    crate::support::Native::Cargo(why) => Some(why),
                    crate::support::Native::Form => (!forms_served).then_some(crate::support::D_FORMS),
                    crate::support::Native::Module | crate::support::Native::Host(_) | crate::support::Native::Files => None,
                };
                return match why {
                    Some(why) => {
                        let subject = format!("COBOL::\"{}\"", b.name);
                        let message = format!("{subject}: {why}");
                        self.err_kind(line, crate::error::ErrorKind::Other, subject, message)
                    }
                    None => Ok(()),
                };
            }
        }
        if forms_served {
            return Ok(());
        }
        let subject = first.map(|n| format!("{root}::{n}")).unwrap_or_else(|| root.to_string());
        self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_FORMS))
    }

    /// Does `e` start a chain that reaches the host?
    pub(crate) fn is_host_member(&mut self, e: &Expr) -> bool {
        match Self::chain(e) {
            Some((root, _)) => self.host_root(root).is_some(),
            None => false,
        }
    }

    /// Evaluate the arguments of every segment (they may read the host or need
    /// temporaries, so they come before the path is started), then start the
    /// path and add each segment with its arguments.
    fn build_path(&mut self, ctrl: u32, segs: &[Seg<'_>]) -> GenResult<()> {
        let arg_opds = self.path_args(segs)?;
        self.emit_path(ctrl, segs, arg_opds);
        Ok(())
    }

    /// The operands of every segment's arguments, evaluated now.
    pub(crate) fn path_args(&mut self, segs: &[Seg<'_>]) -> GenResult<Vec<Vec<u32>>> {
        let mut arg_opds: Vec<Vec<u32>> = Vec::with_capacity(segs.len());
        for s in segs {
            let mut v = Vec::with_capacity(s.args.len());
            for a in s.args {
                v.push(self.operand_or_temp(a)?);
            }
            arg_opds.push(v);
        }
        Ok(arg_opds)
    }

    /// Start the path and add each segment with its (already evaluated) arguments.
    pub(crate) fn emit_path(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: Vec<Vec<u32>>) {
        self.emit_path_keeping(ctrl, segs, arg_opds, None);
    }

    /// [`Self::emit_path`], with argument `whole` of the last segment sent with every character,
    /// the spaces around its text kept (`rt_path_arg_whole`); every other argument is trimmed.
    pub(crate) fn emit_path_keeping(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: Vec<Vec<u32>>, whole: Option<usize>) {
        if ctrl == CTRL_APP {
            if let Some(first) = segs.first() {
                self.names.builtins.insert(first.name.to_string());
            }
        }
        self.i32c(ctrl as i32);
        self.call_rt("rt_path_begin");
        let last = segs.len().saturating_sub(1);
        for (k, (s, opds)) in segs.iter().zip(arg_opds).enumerate() {
            let id = self.names.id(s.name);
            self.i32c(id as i32);
            self.call_rt("rt_path_seg");
            for (j, o) in opds.into_iter().enumerate() {
                self.base();
                self.i32c(o as i32);
                let keep = k == last && whole == Some(j);
                self.call_rt(if keep { "rt_path_arg_whole" } else { "rt_path_arg" });
            }
        }
    }

    /// Ask the host the `super::` reads of `asks` (`GET_WAIT` / `CALLR_WAIT`, the answers in their
    /// character temporaries). The opener may live in another window (ABI A24, WASM-011 R11): the host
    /// then answers each "pending", the task **suspends once, here**, and the same pass runs again after
    /// `pr_resume`, when every answer is at hand. The arguments are evaluated inside the pass, so what
    /// was in the temporaries while other windows' events ran is never read. No statement is run twice:
    /// the pass is placed before the statement that reads (see `lower_super`).
    pub(crate) fn ask_super(&mut self, asks: Vec<crate::lower_super::SuperAsk<'_>>) -> GenResult<()> {
        if asks.is_empty() {
            return Ok(());
        }
        let again = self.new_block("super-ask");
        let waiting = self.new_block("super-wait");
        let answered = self.new_block("super-answered");
        self.jump(again);
        self.begin(again);
        for a in &asks {
            let arg_opds = self.path_args(&a.segs)?;
            self.emit_path(CTRL_SUPER, &a.segs, arg_opds);
            self.i32c(a.op);
            self.i32c(a.k as i32);
            self.i32c(1);
            self.call_rt("rt_cmd_query");
        }
        // Asked of the host, answered "pending" by any of them? Wait for all, then ask again.
        self.call_rt("rt_query_pending");
        self.branch(waiting, answered);
        self.begin(waiting);
        self.suspend_here("super-read");
        self.jump(again);
        self.begin(answered);
        Ok(())
    }

    /// `MOVE x TO Ctrl::Property`: the value goes to the host as text.
    pub(crate) fn member_set(&mut self, target: &Expr, value: u32, line: u32) -> GenResult<()> {
        let Some((root, segs)) = Self::chain(target) else {
            return self.err(line, "a control property was expected as the receiving field");
        };
        let Some(ctrl) = self.host_root(root) else {
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            return self.err(line, "a value method cannot be a receiving field");
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        self.build_path(ctrl, &segs)?;
        self.base();
        self.i32c(value as i32);
        self.call_rt("rt_cmd_set");
        Ok(())
    }

    /// A control property as the receiving field of `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` or
    /// `COMPUTE` (`ADD 1 TO NUD-1::Value`): the result on the top register goes into a temporary and
    /// from there to the host as text, as `MOVE <expression> TO Ctrl::Property` does. `operand`
    /// reads a property; storing into what it answers raised COBOL run-time error 104 (a bad
    /// operand) on the page. `dup` keeps the result for the next receiver.
    pub(crate) fn store_to_member(&mut self, target: &Expr, rounded: bool, dup: bool) -> GenResult<()> {
        let line = target.span().line;
        if rounded {
            return self.err(line, "ROUNDED is not supported with a control property as the receiving field");
        }
        let k = self.temp_next;
        if k >= cobolt_wasm_rt::state::NTEMP as u32 {
            return self.err(line, "this statement needs more than eight computed operands");
        }
        self.temp_next += 1;
        if dup {
            self.i32c(0);
            self.call_rt("rt_pick");
        }
        self.i32c(k as i32);
        self.call_rt("rt_save_temp");
        let src = self.pool.opd_temp(k);
        self.member_set(target, src, line)
    }

    /// A control property as the receiving field of `STRING`: its current text
    /// in a character temporary widened to the temporary's full capacity, so
    /// the statement can write into it. Answers the temporary's index and its
    /// operand; the caller trims it (`rt_ctemp_rtrim`) and writes it back with
    /// [`Self::member_set`].
    pub(crate) fn member_receiver(&mut self, target: &Expr, line: u32) -> GenResult<(u32, u32)> {
        let Some((root, segs)) = Self::chain(target) else {
            return self.err(line, "a control property was expected as the receiving field");
        };
        let Some(ctrl) = self.host_root(root) else {
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            return self.err(line, "a value method cannot be a receiving field");
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        let k = self.ctemp_next;
        let opd = self.host_read(ctrl, &segs, line)?;
        self.i32c(k as i32);
        self.call_rt("rt_ctemp_widen");
        Ok((k, opd))
    }

    /// The display-list opcode of `ctrl::Method(args)` when `ctrl` is a Canvas2D and the method is one
    /// of the commands the routines encode (every operand a number, colour or text; the count right).
    /// Anything else stays a host call, which answers it as it always has.
    fn canvas_command(&self, ctrl: u32, segs: &[Seg<'_>]) -> Option<u8> {
        let f = self.form.as_ref()?;
        if !f.canvases.contains(&ctrl) || segs.len() != 1 {
            return None;
        }
        let s = &segs[0];
        let info = cobolt_graphics_wire::COMMANDS.iter().find(|c| c.opcode < 0x80 && c.name.eq_ignore_ascii_case(s.name))?;
        let scalar = info.operands.iter().all(|(_, k)| matches!(*k, "u8" | "u32" | "f32" | "rgba" | "str"));
        (scalar && s.args.len() == info.operands.len()).then_some(info.opcode)
    }

    /// Before a call or a read on a graphics object other than a Canvas2D (`FormCtx::graphics`),
    /// the display list waiting for a canvas goes to the host (`rt_gfx_send`, ABI A23). The object's
    /// method may draw on the canvas by its handle (`PLT-1::RenderToCanvas( WS-CNV 0 0 320 300 )`),
    /// and it must find there what the program drew before it, as under the interpreter: kept back,
    /// a `Clear` drawn before the call would run after it and wipe out what it drew.
    fn canvas_drawing_first(&mut self, ctrl: u32) {
        if self.form.as_ref().is_some_and(|f| !f.canvases.is_empty() && f.graphics.contains(&ctrl)) {
            self.call_rt("rt_gfx_send");
        }
    }

    /// `Ctrl::Method(args)` as a statement: the call, its result discarded.
    pub(crate) fn member_call(&mut self, e: &Expr, line: u32) -> GenResult<()> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a control member was expected");
        };
        let Some(ctrl) = self.host_root(root) else {
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            // A value method used as a statement has no effect.
            return Ok(());
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, false, line)?;
        // A Canvas2D drawing method is one command of a display list, not a host call (ABI A23).
        if let Some(op) = self.canvas_command(ctrl, &segs) {
            self.build_path(ctrl, &segs)?;
            self.i32c(op as i32);
            self.call_rt("rt_gfx_cmd");
            return Ok(());
        }
        self.canvas_drawing_first(ctrl);
        self.build_path(ctrl, &segs)?;
        self.call_rt("rt_cmd_call");
        if let Some(last) = segs.last() {
            self.suspend_if_waiting(ctrl, last.name);
        }
        let _ = CMD_CALL;
        Ok(())
    }

    /// `Ctrl::Property` or `Ctrl::Method(args)` as a value: the host's answer
    /// text, in a character temporary.
    pub(crate) fn member_read(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a control member was expected");
        };
        let Some(ctrl) = self.host_root(root) else {
            return self.err(line, "not a control member");
        };
        self.host_read(ctrl, &segs, line)
    }

    /// How many leading segments of a chain after a control are the host's: up to the first value
    /// method (`Trim`, `Upper`, …) after the first segment, which applies to the answer here.
    pub(crate) fn host_prefix_len(segs: &[Seg<'_>]) -> usize {
        use cobolt_ast::methods::is_data_item_method;
        segs.iter().skip(1).position(|s| is_data_item_method(s.name)).map_or(segs.len(), |i| i + 1)
    }

    /// Read `ctrl::segs…` from the host into a new character temporary.
    fn host_read(&mut self, ctrl: u32, segs: &[Seg<'_>], line: u32) -> GenResult<u32> {
        let k = self.next_ctemp(line)?;
        self.canvas_drawing_first(ctrl);
        self.build_path(ctrl, segs)?;
        let last_parens = segs.last().map(|s| s.parens).unwrap_or(false);
        self.i32c(if last_parens { Q_CALLR as i32 } else { Q_GET as i32 });
        self.i32c(k as i32);
        self.i32c(1);
        self.call_rt("rt_cmd_query");
        Ok(self.pool.opd_ctemp(k))
    }

    pub(crate) fn next_ctemp(&mut self, line: u32) -> GenResult<u32> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        Ok(k)
    }

    /// A member chain as a value (WASM-010 T11). As the interpreter's
    /// `eval_member` does, a value method (`Trim`, `Upper`, `Lower`, `Replace`,
    /// `Len`, `Split` and their older spellings, `cobolt_ast::methods`) applies
    /// to the value before it — a data item, a literal, or what the host
    /// answered for the segments before it; the segments up to the first value
    /// method after a control are the host's.
    pub(crate) fn member_value(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a member was expected");
        };
        let (mut recv, local) = match self.host_root(root) {
            Some(ctrl) => {
                self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
                let host_len = Self::host_prefix_len(&segs);
                self.check_layer_rules(root, ctrl, &segs[..host_len], false, line)?;
                // A `super::` read the statement asked for before it began (`lower_super`): the answer is there.
                let r = match self.hoisted_super_read(e) {
                    Some(k) => self.pool.opd_ctemp(k),
                    None => self.host_read(ctrl, &segs[..host_len], line)?,
                };
                if host_len == segs.len() {
                    return Ok(r);
                }
                (r, &segs[host_len..])
            }
            None => {
                if let Some(e) = self.unknown_root(root, line) {
                    return Err(e);
                }
                (self.operand_or_temp(root)?, &segs[..])
            }
        };
        let mut k = None;
        for s in local {
            let Some(code) = value_method_code(s.name) else {
                return self.err(
                    line,
                    format!("{} is not a value method of a data item: a data item answers {}", s.name, cobolt_ast::methods::DATA_ITEM_METHODS_TEXT),
                );
            };
            if code == cobolt_wasm_rt::strings::VM_LEN && s.parens {
                return self.err(
                    line,
                    format!("{} is a property of a data item, not a method: write it without the parentheses", s.name),
                );
            }
            let a = match s.args.first() {
                Some(x) => self.operand_or_temp(x)?,
                None => 0,
            };
            let b = match s.args.get(1) {
                Some(x) => self.operand_or_temp(x)?,
                None => 0,
            };
            // Every method of the chain writes the same temporary: the routine
            // reads its operands before it writes the result.
            let slot = match k {
                Some(k) => k,
                None => {
                    let n = self.next_ctemp(line)?;
                    k = Some(n);
                    n
                }
            };
            self.base();
            self.i32c(code as i32);
            self.i32c(recv as i32);
            self.i32c(a as i32);
            self.i32c(b as i32);
            self.i32c(slot as i32);
            self.call_rt("rt_value_method");
            recv = self.pool.opd_ctemp(slot);
        }
        Ok(recv)
    }

    /// `INVOKE Ctrl 'Method' USING args [RETURNING x] [ON EXCEPTION …] [NOT ON EXCEPTION …]`.
    pub(crate) fn invoke_(
        &mut self,
        object: &str,
        method: &str,
        args: &[Expr],
        returning: Option<&Expr>,
        (on_exc, not_exc): (&[Stmt], &[Stmt]),
        line: u32,
    ) -> GenResult<()> {
        let root = Expr::Identifier(object.to_string(), cobolt_lexer::Span::new(0, 0, line, 0));
        let Some(ctrl) = self.host_root(&root) else {
            return self.err(line, format!("INVOKE {object}: it is not a control of this form"));
        };
        let seg = Seg { name: method, args, parens: true };
        if !on_exc.is_empty() || !not_exc.is_empty() {
            if ctrl != CTRL_SUPER || self.form.is_none() {
                return self.err(
                    line,
                    "ON EXCEPTION / NOT ON EXCEPTION on an INVOKE is available for `super::` in a WebAssembly module (the window that opened this one may be closed); a control or the form answers at once, and what it refuses is a run-time error",
                );
            }
            return self.invoke_super_guarded(seg, returning, on_exc, not_exc, line);
        }
        self.canvas_drawing_first(ctrl);
        match returning {
            None => {
                self.build_path(ctrl, std::slice::from_ref(&seg))?;
                self.call_rt("rt_cmd_call");
                self.suspend_if_waiting(ctrl, method);
            }
            Some(r) => {
                let k = self.ctemp_next;
                if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
                    return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
                }
                self.ctemp_next += 1;
                if ctrl == CTRL_SUPER && self.form.is_some() {
                    // The answer may come from another window: ask, wait, ask again (`ask_super`).
                    self.ask_super(vec![crate::lower_super::SuperAsk { segs: vec![seg], op: Q_CALLRW as i32, k }])?;
                } else {
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.i32c(Q_CALLR as i32);
                    self.i32c(k as i32);
                    self.i32c(1);
                    self.call_rt("rt_cmd_query");
                }
                let src = self.pool.opd_ctemp(k);
                self.move_operand_to(src, r, line)?;
                self.suspend_if_waiting(ctrl, method);
            }
        }
        Ok(())
    }

    /// `INVOKE super::Method(…) [RETURNING x] ON EXCEPTION … [NOT ON EXCEPTION …]` (ABI A25, WASM-011 Q3).
    ///
    /// The window that opened this one may have been closed, or never answer: the read is asked soft
    /// (`Q_CALLRWS`), the host reports no run-time error and the module asks `rt_query_failed`. A failed
    /// read runs `ON EXCEPTION` and leaves `x` as it was; one that went through stores its answer in `x`
    /// and runs `NOT ON EXCEPTION`. Without `RETURNING` the answer is dropped, the call is still waited for.
    fn invoke_super_guarded(
        &mut self,
        seg: Seg<'_>,
        returning: Option<&Expr>,
        on_exc: &[Stmt],
        not_exc: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        self.ask_super(vec![crate::lower_super::SuperAsk { segs: vec![seg], op: Q_CALLRWS as i32, k }])?;
        let failed = self.new_block("super-failed");
        let went_through = self.new_block("super-went-through");
        let join = self.new_block("end-invoke");
        self.call_rt("rt_query_failed");
        self.branch(failed, went_through);
        self.begin(went_through);
        if let Some(r) = returning {
            let src = self.pool.opd_ctemp(k);
            self.move_operand_to(src, r, line)?;
        }
        self.stmts(not_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(failed);
        self.stmts(on_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    /// `MOVE <operand> TO <target>` where the sender is already an operand.
    pub(crate) fn move_operand_to(&mut self, src: u32, to: &Expr, line: u32) -> GenResult<()> {
        if matches!(to, Expr::Member { .. }) {
            return self.member_set(to, src, line);
        }
        let dst = self.receiver(to)?;
        self.base();
        self.i32c(src as i32);
        self.i32c(dst as i32);
        self.call_rt("rt_move");
        Ok(())
    }

    /// `SHOW` / `HIDE` / `CLOSE` of the form: a call on the form itself.
    pub(crate) fn window_op(&mut self, op: &WindowOperation, line: u32) -> GenResult<()> {
        let (name, who) = match op {
            WindowOperation::Show(w) => ("Show", w),
            WindowOperation::Hide(w) => ("Hide", w),
            WindowOperation::Close(w) => ("Close", w),
        };
        let _ = who;
        let seg = Seg { name, args: &[], parens: true };
        self.build_path(CTRL_FORM, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        let _ = line;
        Ok(())
    }

    /// `COBOL::"WAIT-EVENT" ( event-id control-id )`: the form is idle. The host
    /// gets what the handler changed, the module stops where it is, and
    /// `pr_event` later writes the event's names into the two items and
    /// continues here.
    pub(crate) fn wait_event(&mut self, using: &[CallArg], line: u32) -> GenResult<()> {
        let mut opds = [0u32; 2];
        for (i, a) in using.iter().take(2).enumerate() {
            let e = match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
            };
            opds[i] = self.operand(e)?.ok_or_else(|| crate::error::GenError::new(line, "WAIT-EVENT takes data items"))?;
        }
        let slot = match self.park_slot {
            Some(s) => s,
            None => {
                let s = self.layout.alloc_hidden(8);
                self.park_slot = Some(s);
                s
            }
        };
        self.base();
        self.i32c(slot as i32);
        self.i32c(opds[0] as i32);
        self.i32c(opds[1] as i32);
        self.call_rt("rt_park");
        self.call_rt("rt_flush");
        let k = self.new_block("after-wait");
        self.emit(Ins::Pc(k));
        self.emit(Ins::GlobalSet(G::Resume));
        self.i32c(ret::PARK);
        self.finish_raw();
        self.begin(k);
        Ok(())
    }

    /// The control a built-in's first argument names: a literal holding a
    /// control's name (as generated code writes it), or the control itself.
    fn literal_control(&mut self, builtin: &str, obj: &Expr, line: u32) -> GenResult<u32> {
        let name = match obj {
            Expr::Literal(Literal::String(o), _) => o.trim().to_string(),
            Expr::Identifier(o, _) => o.clone(),
            _ => return self.err(line, format!("COBOL::\"{builtin}\" in a browser module names the control in a literal")),
        };
        let root = Expr::Identifier(name.clone(), cobolt_lexer::Span::new(0, 0, line, 0));
        match self.host_root(&root) {
            Some(ctrl) => Ok(ctrl),
            None => self.err(line, format!("COBOL::\"{builtin}\": {name} is not a control of this form")),
        }
    }

    /// A built-in `CALL "COBOL-NAME"` that belongs to the page.
    pub(crate) fn builtin_call(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let app = Expr::Identifier("COBOL".to_string(), cobolt_lexer::Span::new(0, 0, line, 0));
        self.desktop_gate(CTRL_APP, &app, Some(name), line)?;
        match name {
            // The page builds the form when it opens it.
            "INIT-FORM" => Ok(()),
            "WAIT-EVENT" => self.wait_event(using, line),
            // `COBOL-SET-PROPERTY obj prop value` / `COBOL-GET-PROPERTY obj prop dest`
            // name the control and the property in literals; SET-PROPERTY may
            // also take the property's name from a data item, as the
            // interpreter does (WASM-005 T14: the backend's answer names the
            // properties it set at run time) — ABI amendment A14's SET_NAMED.
            "SET-PROPERTY" | "GET-PROPERTY" if using.len() >= 3 => {
                let ex = |a: &CallArg| match a {
                    CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                };
                let (obj, prop, third) = (ex(&using[0]), ex(&using[1]), ex(&using[2]));
                if name == "SET-PROPERTY" && !matches!(prop, Expr::Literal(Literal::String(_), _)) {
                    let ctrl = self.literal_control(name, &obj, line)?;
                    let p = self.operand_or_temp(&prop)?;
                    let v = self.operand_or_temp(&third)?;
                    self.base();
                    self.i32c(ctrl as i32);
                    self.i32c(p as i32);
                    self.i32c(v as i32);
                    self.call_rt("rt_cmd_set_named");
                    return Ok(());
                }
                let (Expr::Literal(Literal::String(o), _), Expr::Literal(Literal::String(p), _)) = (&obj, &prop) else {
                    return self.err(line, format!("COBOL::\"{name}\" in a browser module names the control and the property in literals"));
                };
                let root = Expr::Identifier(o.clone(), cobolt_lexer::Span::new(0, 0, line, 0));
                let Some(ctrl) = self.host_root(&root) else {
                    return self.err(line, format!("COBOL::\"{name}\": {o} is not a control of this form"));
                };
                let seg = Seg { name: p.as_str(), args: &[], parens: false };
                if name == "SET-PROPERTY" {
                    let v = self.operand_or_temp(&third)?;
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.base();
                    self.i32c(v as i32);
                    self.call_rt("rt_cmd_set");
                } else {
                    let k = self.ctemp_next;
                    if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
                        return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
                    }
                    self.ctemp_next += 1;
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.i32c(Q_GET as i32);
                    self.i32c(k as i32);
                    self.i32c(1);
                    self.call_rt("rt_cmd_query");
                    let src = self.pool.opd_ctemp(k);
                    self.move_operand_to(src, &third, line)?;
                }
                Ok(())
            }
            // `COBOL::"RAISE-EVENT" ( control event [argument] )`: the page
            // queues the event as the control would raise it (A14's RAISE).
            "RAISE-EVENT" if using.len() >= 2 => {
                let ex = |a: &CallArg| match a {
                    CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                };
                let ctrl = self.literal_control(name, &ex(&using[0]), line)?;
                let ev = self.operand_or_temp(&ex(&using[1]))?;
                let arg = match using.get(2) {
                    Some(a) => self.operand_or_temp(&ex(a))?,
                    None => self.operand_or_temp(&Expr::Literal(Literal::String(String::new()), cobolt_lexer::Span::new(0, 0, line, 0)))?,
                };
                self.base();
                self.i32c(ctrl as i32);
                self.i32c(ev as i32);
                self.i32c(arg as i32);
                self.call_rt("rt_cmd_raise");
                Ok(())
            }
            // `COBOL::"WS-LAST-ERROR" ( class [code [message]] )`: how the last
            // remote call failed, kept by the module from the error frame.
            "WS-LAST-ERROR" => {
                if using.len() > 3 {
                    return self.err(line, "COBOL::\"WS-LAST-ERROR\" in a browser module answers the class, the code and the message; the detail stays with the backend's log");
                }
                let mut opds = [0u32; 3];
                for (i, a) in using.iter().take(3).enumerate() {
                    let e = match a {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
                    };
                    opds[i] = self.receiver(e)?;
                }
                self.base();
                for o in opds {
                    self.i32c(o as i32);
                }
                self.call_rt("rt_ws_last_error");
                Ok(())
            }
            // On the desktop the runner answers the file dialogs (T32); a browser module refuses them below.
            dialog if self.native && crate::lower_remote::is_desktop_dialog(dialog) => self.desktop_file_dialog(dialog, using, line),
            // On the desktop the runner plays the sounds (T49).
            audio if self.native && crate::lower_remote::is_desktop_audio(audio) => self.desktop_audio(audio, using, line),
            // On the desktop the runner keeps an AWS connection's keys (T48).
            aws if self.native && crate::lower_remote::is_desktop_aws(aws) => self.desktop_aws(aws, using, line),
            // The page's own verbs (ABI §12.1) are not the desktop's: under the interpreter a form's
            // windows are `ME`'s and a handle's, and a desktop form has no message box (T6, T32). A
            // desktop module refuses them, so the Build takes the Cargo path, which reports them as the
            // interpreter does.
            page if self.native && crate::lower_remote::is_app_verb(page) && !crate::lower_remote::is_mobile_verb(page) => {
                let subject = format!("COBOL::\"{page}\"");
                self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} is a verb of a web page's application object, which a desktop program does not have"))
            }
            other => {
                // Any other service the page offers is a call on the application
                // object with the arguments as text.
                let args: Vec<Expr> = using
                    .iter()
                    .map(|a| match a {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                    })
                    .collect();
                if crate::lower_remote::is_mobile_verb(other) && !self.mobile {
                    let verb = match other {
                        "SECURESTORE" => "SecureStore",
                        "SECURELOAD" => "SecureLoad",
                        _ => "SecureDelete",
                    };
                    return self.err_kind(
                        line,
                        crate::error::ErrorKind::Unsupported,
                        format!("COBOL::{verb}"),
                        format!("COBOL::{verb} is available in a Mobile project only: it keeps a secret in the phone's own secure store, which a web page does not have"),
                    );
                }
                match crate::support::builtin_support(other) {
                    // A verb of the application object the page implements (ABI §12.1).
                    _ if crate::lower_remote::is_app_verb(other) => {}
                    crate::support::Support::Module | crate::support::Support::Conformance => {}
                    crate::support::Support::BackendOnly(alt) | crate::support::Support::MemoryFiles(alt) => {
                        return self.err_kind(line, crate::error::ErrorKind::BackendOnly, format!("COBOL::\"{other}\""), format!("COBOL::\"{other}\" runs on the backend, not in a browser module: {alt}"));
                    }
                    crate::support::Support::Unsupported(why) => {
                        return self.err_kind(line, crate::error::ErrorKind::Unsupported, format!("COBOL::\"{other}\""), format!("COBOL::\"{other}\" is not available in a browser module: {why}"));
                    }
                }
                // `COBOL::SecureLoad( key value )` (A26): the value the shell found
                // lands in the second argument once the task resumes.
                if other == "SECURELOAD" && args.len() > 2 {
                    return self.err(line, "COBOL::SecureLoad takes the key and, optionally, the data item that receives the value");
                }
                let receiver = if other == "SECURELOAD" { args.get(1).cloned() } else { None };
                let sent: &[Expr] = if receiver.is_some() { &args[..1] } else { &args };
                let seg = Seg { name: other, args: sent, parens: true };
                // `COBOL::"CHART-SET-TABLE"` reads its table at fixed offsets: the table goes whole,
                // the spaces around its text kept, as the interpreter reads the item.
                let whole = (other == "CHART-SET-TABLE").then_some(1);
                let arg_opds = self.path_args(std::slice::from_ref(&seg))?;
                self.emit_path_keeping(CTRL_APP, std::slice::from_ref(&seg), arg_opds, whole);
                self.call_rt("rt_cmd_call");
                self.suspend_if_waiting(CTRL_APP, other);
                if let Some(dest) = receiver {
                    self.app_property_into("SecureValue", &dest, line)?;
                }
                Ok(())
            }
        }
    }

    /// The desktop's file dialogs (DESKTOP-WASM-001 T32, ABI A33): `COBOL::"OPEN-FILE-DIALOG"`,
    /// `"SAVE-FILE-DIALOG"` and `"FOLDER-DIALOG"`, whose arguments are the interpreter's (the
    /// title, the filter, the file name, the start folder, and always last the item that receives
    /// the path). The arguments before the last go to the runner as text; the task waits while the
    /// operator chooses; then the last receives the application's `DialogPath`, spaces when the
    /// operator cancelled, as the interpreter moves the answer.
    fn desktop_file_dialog(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if using.len() < 2 {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes the title and, last, the data item that receives the path"));
        }
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject} waits while the operator chooses, which a desktop module does in a form's handler only"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (sent, dest) = args.split_at(args.len() - 1);
        let seg = Seg { name, args: sent, parens: true };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        self.suspend_if_waiting(CTRL_APP, name);
        self.app_property_into("DialogPath", &dest[0], line)
    }

    /// The desktop's sound player (DESKTOP-WASM-001 T49, ABI A36): `COBOL::"AUDIO-PLAY" ( file
    /// [status] )`, `"AUDIO-PAUSE"`, `"AUDIO-RESUME"` and `"AUDIO-STOP"` ( [status] ), and
    /// `"AUDIO-STATE" ( state [position-ms] [duration-ms] )`, with the interpreter's arguments. The
    /// file goes to the runner as text; the runner answers at once, and each receiver gets its answer
    /// from the application object: `AudioStatus`, or `AudioState`, `AudioPosition` and
    /// `AudioDuration`. Arguments after those the interpreter reads are ignored, as it ignores them.
    fn desktop_audio(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject}: the runner plays a desktop module's sounds in a form's handler only, so a program with no form takes the Cargo path"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (sent, answers): (&[Expr], Vec<(&str, Option<&Expr>)>) = match name {
            "AUDIO-PLAY" | "AUDIO-STATE" if args.is_empty() => {
                let what = if name == "AUDIO-PLAY" { "the file" } else { "the data item that receives the state" };
                return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
            }
            "AUDIO-PLAY" => (&args[..1], vec![("AudioStatus", args.get(1))]),
            "AUDIO-STATE" => (&[], vec![("AudioState", args.first()), ("AudioPosition", args.get(1)), ("AudioDuration", args.get(2))]),
            _ => (&[], vec![("AudioStatus", args.first())]),
        };
        let seg = Seg { name, args: sent, parens: true };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        for (prop, dest) in answers {
            if let Some(dest) = dest {
                self.app_property_into(prop, dest, line)?;
            }
        }
        Ok(())
    }

    /// The keys of an AWS connection typed in at run time (DESKTOP-WASM-001 T48, ABI A38), with the
    /// interpreter's arguments: `COBOL::"AWS-SET-CREDENTIALS" ( connection key-id secret token
    /// [status] )`, `"AWS-CLEAR-CREDENTIALS" ( connection [status] )` and `"AWS-CREDENTIALS-SET" (
    /// connection flag )`. The runner answers at once; `status` gets the application's `AwsStatus`
    /// (`OK`, or why nothing was stored), `flag` its `AwsFlag` (`Y` or `N`). Fewer arguments than the
    /// interpreter needs are refused, so the Build takes the Cargo path, which reports them as the
    /// interpreter does. Arguments after those it reads are ignored, as it ignores them.
    fn desktop_aws(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject}: the runner serves AWS in a form's handler only, so a program with no form takes the Cargo path"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (needed, what) = match name {
            "AWS-SET-CREDENTIALS" => (4, "the connection, the access key id, the secret access key and the session token"),
            "AWS-CREDENTIALS-SET" => (2, "the connection and the data item that receives Y or N"),
            _ => (1, "the connection"),
        };
        if args.len() < needed {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        let (sent, answer): (&[Expr], (&str, Option<&Expr>)) = match name {
            "AWS-SET-CREDENTIALS" => (&args[..4], ("AwsStatus", args.get(4))),
            "AWS-CREDENTIALS-SET" => (&args[..1], ("AwsFlag", args.get(1))),
            _ => (&args[..1], ("AwsStatus", args.get(1))),
        };
        let seg = Seg { name, args: sent, parens: true };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        if let (prop, Some(dest)) = answer {
            self.app_property_into(prop, dest, line)?;
        }
        Ok(())
    }

    /// Move the application object's property `prop` (as text) into `dest`.
    fn app_property_into(&mut self, prop: &str, dest: &Expr, line: u32) -> GenResult<()> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        let value = Seg { name: prop, args: &[], parens: false };
        self.build_path(CTRL_APP, std::slice::from_ref(&value))?;
        self.i32c(Q_GET as i32);
        self.i32c(k as i32);
        self.i32c(1);
        self.call_rt("rt_cmd_query");
        let src = self.pool.opd_ctemp(k);
        self.move_operand_to(src, dest, line)
    }
}

/// What a layer answers, as the end of a refusal: `Name (read only), Visible, …`.
fn layer_props_text() -> String {
    cobolt_forms::model::LAYER_PROPS
        .iter()
        .map(|p| if p.eq_ignore_ascii_case("Name") { format!("{p} (read only)") } else { (*p).to_string() })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The routine's code for a value method, by any of its spellings.
fn value_method_code(name: &str) -> Option<u32> {
    use cobolt_wasm_rt::strings::*;
    let canonical = cobolt_ast::methods::canonical_data_item_method(name).unwrap_or(name);
    Some(match canonical.to_ascii_uppercase().as_str() {
        "TRIM" => VM_TRIM,
        "UPPER" => VM_UPPER,
        "LOWER" => VM_LOWER,
        "REPLACE" => VM_REPLACE,
        "LEN" => VM_LEN,
        "SPLIT" => VM_SPLIT,
        _ => return None,
    })
}

#[allow(dead_code)]
fn _unused(_: I<'static>) {}
