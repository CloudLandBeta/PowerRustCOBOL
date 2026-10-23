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
use cobolt_wasm_rt::cmd::{CMD_CALL, Q_CALLR, Q_GET};
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
    fn chain<'e>(e: &'e Expr) -> Option<(&'e Expr, Vec<Seg<'e>>)> {
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
        let mut arg_opds: Vec<Vec<u32>> = Vec::with_capacity(segs.len());
        for s in segs {
            let mut v = Vec::with_capacity(s.args.len());
            for a in s.args {
                v.push(self.operand_or_temp(a)?);
            }
            arg_opds.push(v);
        }
        if ctrl == CTRL_APP {
            if let Some(first) = segs.first() {
                self.names.builtins.insert(first.name.to_string());
            }
        }
        self.i32c(ctrl as i32);
        self.call_rt("rt_path_begin");
        for (s, opds) in segs.iter().zip(arg_opds) {
            let id = self.names.id(s.name);
            self.i32c(id as i32);
            self.call_rt("rt_path_seg");
            for o in opds {
                self.base();
                self.i32c(o as i32);
                self.call_rt("rt_path_arg");
            }
        }
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
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        self.build_path(ctrl, &segs)?;
        self.base();
        self.i32c(value as i32);
        self.call_rt("rt_cmd_set");
        Ok(())
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
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        let k = self.ctemp_next;
        let opd = self.host_read(ctrl, &segs, line)?;
        self.i32c(k as i32);
        self.call_rt("rt_ctemp_widen");
        Ok((k, opd))
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
        self.check_layer_rules(root, ctrl, &segs, false, line)?;
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

    /// Read `ctrl::segs…` from the host into a new character temporary.
    fn host_read(&mut self, ctrl: u32, segs: &[Seg<'_>], line: u32) -> GenResult<u32> {
        let k = self.next_ctemp(line)?;
        self.build_path(ctrl, segs)?;
        let last_parens = segs.last().map(|s| s.parens).unwrap_or(false);
        self.i32c(if last_parens { Q_CALLR as i32 } else { Q_GET as i32 });
        self.i32c(k as i32);
        self.i32c(1);
        self.call_rt("rt_cmd_query");
        Ok(self.pool.opd_ctemp(k))
    }

    fn next_ctemp(&mut self, line: u32) -> GenResult<u32> {
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
        use cobolt_ast::methods::is_data_item_method;
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a member was expected");
        };
        let (mut recv, local) = match self.host_root(root) {
            Some(ctrl) => {
                let host_len = segs.iter().skip(1).position(|s| is_data_item_method(s.name)).map_or(segs.len(), |i| i + 1);
                self.check_layer_rules(root, ctrl, &segs[..host_len], false, line)?;
                let r = self.host_read(ctrl, &segs[..host_len], line)?;
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

    /// `INVOKE Ctrl 'Method' USING args [RETURNING x]`.
    pub(crate) fn invoke_(&mut self, object: &str, method: &str, args: &[Expr], returning: Option<&Expr>, line: u32) -> GenResult<()> {
        let root = Expr::Identifier(object.to_string(), cobolt_lexer::Span::new(0, 0, line, 0));
        let Some(ctrl) = self.host_root(&root) else {
            return self.err(line, format!("INVOKE {object}: it is not a control of this form"));
        };
        let seg = Seg { name: method, args, parens: true };
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
                self.build_path(ctrl, std::slice::from_ref(&seg))?;
                self.i32c(Q_CALLR as i32);
                self.i32c(k as i32);
                self.i32c(1);
                self.call_rt("rt_cmd_query");
                let src = self.pool.opd_ctemp(k);
                self.move_operand_to(src, r, line)?;
                self.suspend_if_waiting(ctrl, method);
            }
        }
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

    /// A built-in `CALL "COBOL-NAME"` that belongs to the page.
    pub(crate) fn builtin_call(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        match name {
            // The page builds the form when it opens it.
            "INIT-FORM" => Ok(()),
            "WAIT-EVENT" => self.wait_event(using, line),
            // `COBOL-SET-PROPERTY obj prop value` / `COBOL-GET-PROPERTY obj prop dest`
            // name the control and the property in literals.
            "SET-PROPERTY" | "GET-PROPERTY" if using.len() >= 3 => {
                let ex = |a: &CallArg| match a {
                    CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                };
                let (obj, prop, third) = (ex(&using[0]), ex(&using[1]), ex(&using[2]));
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
            other => {
                // Any other service the page offers is a call on the application
                // object with the arguments as text.
                let args: Vec<Expr> = using
                    .iter()
                    .map(|a| match a {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                    })
                    .collect();
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
                let seg = Seg { name: other, args: &args, parens: true };
                self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
                self.call_rt("rt_cmd_call");
                self.suspend_if_waiting(CTRL_APP, other);
                Ok(())
            }
        }
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
