// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The statements that wait (WASM-010 T16, R9, R10; ABI §4, §12.1, A5, A6):
//! `CALL … WITH WEBSOCKET LINKAGE` and the verbs of the application object
//! and the form that suspend the task. Each ends its block with `SUSPEND`, the
//! block to continue in held in `$resume`; the host runs other instances'
//! events meanwhile and resumes the task with `pr_resume`.

use crate::error::{GenError, GenResult};
use crate::ir::*;
use crate::lower::Lower;
use crate::lower_form::{CTRL_APP, CTRL_FORM};
use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::Expr;
use cobolt_ast::program::DataSection;
use cobolt_ast::stmt::{CallArg, Stmt};
use wasm_encoder::Instruction as I;

/// The verbs of the application object (`COBOL::`, ctrl 2) the page implements
/// (ABI §12.1, amendment A7). They are the browser's, not built-ins of the
/// desktop runtime, so they are named here and nowhere in `BUILTINS`.
pub const APP_VERBS: &[&str] = &[
    "MESSAGEBOX",
    "ALERT",
    "CONFIRM",
    "INPUTBOX",
    "SLEEP",
    "WAIT",
    "OPENFORMSYNC",
    "OPENFORMASYNC",
    "NAVIGATE",
    "OPENURL",
    "DOWNLOAD",
    "COPYTOCLIPBOARD",
    "SETTITLE",
    "GETPROPERTY",
    "SETPROPERTY",
    "STOPRUN",
    "EXIT",
];

/// Is `name` a verb of the application object (any case)?
pub fn is_app_verb(name: &str) -> bool {
    let n = name.trim().to_ascii_uppercase();
    APP_VERBS.contains(&n.as_str())
}

/// Does the verb `verb` of control `ctrl` suspend the task (ABI §12.1)?
pub fn suspends(ctrl: u32, verb: &str) -> bool {
    let v = verb.trim().to_ascii_uppercase();
    match ctrl {
        CTRL_APP => matches!(
            v.as_str(),
            "MESSAGEBOX" | "ALERT" | "CONFIRM" | "INPUTBOX" | "SLEEP" | "WAIT" | "OPENFORMSYNC"
        ),
        CTRL_FORM => v == "OPENFORMSYNC",
        _ => false,
    }
}

/// The declaration named by `path` (names from the record down) among `decls`.
fn find_decl<'d>(decls: &'d [DataDecl], path: &[String]) -> Option<&'d DataDecl> {
    let (first, rest) = path.split_first()?;
    let d = decls.iter().find(|d| {
        d.name
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(first))
    })?;
    if rest.is_empty() {
        Some(d)
    } else {
        find_decl(&d.children, rest)
    }
}

impl Lower<'_, '_> {
    /// End the current block with `SUSPEND`; the code that follows runs when
    /// the host resumes the task.
    pub(crate) fn suspend_here(&mut self, what: &str) {
        let k = self.new_block(&format!("after-{what}"));
        self.emit(Ins::Pc(k));
        self.emit(Ins::GlobalSet(G::Resume));
        self.i32c(ret::SUSPEND);
        self.finish_raw();
        self.begin(k);
    }

    /// After a host call on `ctrl`'s `verb`: suspend when the verb waits.
    pub(crate) fn suspend_if_waiting(&mut self, ctrl: u32, verb: &str) {
        if self.form.is_some() && suspends(ctrl, verb) {
            self.suspend_here(verb);
        }
    }

    /// The declaration of layout item `idx`, as its program wrote it.
    fn decl_of(&self, idx: usize) -> Option<&DataDecl> {
        let mut path = Vec::new();
        let mut cur = Some(idx);
        while let Some(i) = cur {
            path.push(self.layout.items[i].name.clone()?);
            cur = self.layout.items[i].parent;
        }
        path.reverse();
        let prog = self.programs.get(self.layout.items[idx].unit as usize)?;
        prog.data.as_ref()?.sections.iter().find_map(|s| match s {
            DataSection::WorkingStorage(d)
            | DataSection::LocalStorage(d)
            | DataSection::Linkage(d) => find_decl(d, &path),
            DataSection::FileSection(fds) => {
                fds.iter().find_map(|fd| find_decl(&fd.records, &path))
            }
            DataSection::Screen(_) => None,
        })
    }

    /// The group ⇄ JSON descriptor of the item `e` names (in the constants),
    /// and its operand. The layout is accepted or refused by
    /// `cobolt_wire::from_decl`, as on the desktop and the backend.
    fn json_group(&mut self, e: &Expr, line: u32) -> GenResult<(u32, u32)> {
        if !matches!(e, Expr::Identifier(..) | Expr::Qualified { .. }) {
            return self.err(line, "an argument of a WEBSOCKET call must be a data item, written as its name (no subscript, reference modification, literal or expression)");
        }
        let idx = self.resolve_item(e)?;
        let decl = self
            .decl_of(idx)
            .ok_or_else(|| GenError::new(line, "the declaration of the argument was not found"))?
            .clone();
        let item = cobolt_wire::from_decl(&decl).map_err(|err| {
            GenError::new(
                line,
                format!(
                    "{} cannot travel as JSON: {err}",
                    decl.name.clone().unwrap_or_default()
                ),
            )
        })?;
        let blob = crate::json_layout::build(&self.layout, idx, &item)
            .map_err(|m| GenError::new(line, m))?;
        let desc = self.pool.intern(&blob);
        let opd = self.item_operand(idx, &[], None, line)?;
        Ok((desc, opd))
    }

    /// `CALL name USING … WITH WEBSOCKET LINKAGE [RETURNING r]
    /// [ON EXCEPTION …] [NOT ON EXCEPTION …]`.
    pub(crate) fn remote_call(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        returning: Option<&Expr>,
        on_exc: &[Stmt],
        not_exc: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        if self.form.is_none() {
            return self.err(line, "CALL … WITH WEBSOCKET LINKAGE waits for the backend, which only a form's handler can do in a browser module");
        }
        let name = self.operand_or_temp(program)?;
        let mut words: Vec<u32> = vec![using.len() as u32, returning.is_some() as u32];
        for a in using {
            let (e, by_ref) = match a {
                CallArg::ByReference(e) => (e, true),
                CallArg::ByContent(e) | CallArg::ByValue(e) => (e, false),
            };
            let (desc, opd) = self.json_group(e, line)?;
            words.extend_from_slice(&[by_ref as u32, desc, opd]);
        }
        match returning {
            Some(r) => {
                let (desc, opd) = self.json_group(r, line)?;
                words.extend_from_slice(&[desc, opd]);
            }
            None => words.extend_from_slice(&[0, 0]),
        }
        let call = self.pool.spec(&words);
        let failed = self.new_block("remote-failed");
        let sent = self.new_block("remote-sent");
        let ok = self.new_block("remote-ok");
        let join = self.new_block("end-call");
        // The request: an id, or a failure before anything was sent.
        self.base();
        self.i32c(name as i32);
        self.i32c(call as i32);
        self.call_rt("rt_remote_request");
        self.i32c(0);
        self.raw(I::I32LtS);
        self.branch(failed, sent);
        self.begin(sent);
        self.suspend_here("remote-call");
        // Resumed: the answer is in the workspace.
        self.base();
        self.i32c(call as i32);
        self.call_rt("rt_remote_result");
        self.raw(I::I32Eqz);
        self.branch(ok, failed);
        self.begin(ok);
        self.stmts(not_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(failed);
        if on_exc.is_empty() {
            // Without ON EXCEPTION a failed call is a run-time error (R11).
            self.i32c(cobolt_wasm_rt::state::err::REMOTE_FAILED as i32);
            self.emit(Ins::CallGlue(Glue::Abort));
            self.i32c(ret::DONE);
            self.finish_raw();
        } else {
            self.stmts(on_exc)?;
            if self.cur.is_some() {
                self.jump(join);
            }
        }
        self.begin(join);
        Ok(())
    }
}
