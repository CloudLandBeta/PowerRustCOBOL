// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL`, `CANCEL`, and the way a called program returns.
//!
//! The module holds every program unit of the source — the main program, the
//! programs nested in it and the ones written after it — so a `CALL` of one of
//! them is a transfer inside the module, not a load. It is lowered as a
//! `PERFORM` of the callee's whole procedure: the caller pushes a frame whose
//! *end* word is the callee's exit block and whose *return* word is the block
//! after the `CALL`, binds the arguments to the callee's LINKAGE slots, and
//! jumps to the callee's first paragraph. `EXIT PROGRAM`, `GOBACK` and falling
//! off the last paragraph all arrive at the exit block, which drops every frame
//! down to that call frame and returns through it (the callee may have been
//! inside any number of `PERFORM`s).
//!
//! All units share one region: a callee's storage lies at its own addresses, so
//! `GLOBAL` and `EXTERNAL` items need no machinery beyond name scoping and one
//! allocation. Storage is static: it keeps its values between calls until
//! `CANCEL` puts it back to the state the DATA DIVISION describes.

use crate::error::GenResult;
use crate::ir::*;
use crate::lower::Lower;
use cobolt_ast::expr::*;
use cobolt_ast::stmt::*;
use wasm_encoder::Instruction as I;

impl Lower<'_, '_> {
    /// Is `e` a reference to a data item (as opposed to a literal, a function
    /// or a computation)?
    fn is_item_ref(e: &Expr) -> bool {
        matches!(e, Expr::Identifier(..) | Expr::Qualified { .. } | Expr::Subscript { .. } | Expr::RefMod { .. })
    }

    /// `CALL program [USING …] [RETURNING …] [ON EXCEPTION …] [NOT ON EXCEPTION …]`.
    pub(crate) fn call_(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        returning: Option<&Expr>,
        on_exc: &[Stmt],
        not_exc: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        if let Expr::Literal(Literal::String(name), _) = program {
            let up = name.trim().to_ascii_uppercase();
            if up.starts_with("COBOL-") || up.starts_with("COBOLT-") {
                let bare = up.trim_start_matches("COBOLT-").trim_start_matches("COBOL-");
                return self.builtin_call(bare, using, line);
            }
            return match self.unit_by_name.get(&up).copied() {
                Some(c) if c != 0 => self.call_unit(c, using, returning, not_exc, line),
                // A paragraph of this program: the interpreter runs it (its
                // "internal paragraph CALL"), returning no value.
                _ if self.para_by_name.contains_key(&up) => {
                    let p = self.para_by_name[&up][0];
                    let after = self.new_block("after-paragraph-call");
                    self.call_range(p, p, after);
                    self.begin(after);
                    self.stmts(not_exc)
                }
                // Not a program of this module: the exception path.
                _ => self.call_missing(on_exc),
            };
        }
        self.call_dynamic(program, using, returning, on_exc, not_exc, line)
    }

    /// A `CALL` whose callee is not there: `ON EXCEPTION`, or a run-time error.
    fn call_missing(&mut self, on_exc: &[Stmt]) -> GenResult<()> {
        if on_exc.is_empty() {
            self.call_rt("rt_program_not_found");
            Ok(())
        } else {
            self.stmts(on_exc)
        }
    }

    /// A `CALL` of unit `c`, then `NOT ON EXCEPTION`.
    fn call_unit(&mut self, c: usize, using: &[CallArg], returning: Option<&Expr>, not_exc: &[Stmt], line: u32) -> GenResult<()> {
        let after = self.new_block("after-call");
        // A form's start-up calls its `onLoad` handler from the first paragraph
        // of the main program, with nothing performed: where it goes on is the
        // point a start-up stopped by a run-time error continues from.
        if self.form.is_some()
            && self.cur_unit == 0
            && self.cur_para == 0
            && self.startup_resume.is_none()
            && self.units[c].name.eq_ignore_ascii_case(&format!("{}--ONLOAD", self.units[0].name))
        {
            self.startup_resume = Some(after);
        }
        self.enter_callee(c, using, after, line)?;
        self.begin(after);
        if let Some(r) = returning {
            self.move_returning(c, r, line)?;
        }
        self.stmts(not_exc)
    }

    /// Bind the arguments, push the call frame and jump into unit `c`.
    fn enter_callee(&mut self, c: usize, using: &[CallArg], after: BlockId, line: u32) -> GenResult<()> {
        // An INITIAL program starts every call in its initial state (COBOL-85
        // 6.8, WASM-PARITY #10): as if it had been CANCELled.
        if self.units[c].initial {
            self.emit(Ins::InitUnit(self.unit_base + c));
        }
        self.bind_args(c, using, line)?;
        // Overflow check: psp >= end -> run-time error.
        self.emit(Ins::GlobalGet(G::Psp));
        self.stack_hi();
        self.raw(I::I32GeU);
        self.raw(I::If(wasm_encoder::BlockType::Empty));
        self.i32c(cobolt_wasm_rt::state::err::PERFORM_DEPTH as i32);
        self.emit(Ins::CallGlue(Glue::Abort));
        self.raw(I::End);
        // stack[psp] = (the callee's exit block, the block after the CALL)
        let (exit, entry) = (self.units[c].exit, self.units[c].entry);
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(exit));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(after));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 4, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(8);
        self.raw(I::I32Add);
        self.emit(Ins::GlobalSet(G::Psp));
        self.jump(entry);
        Ok(())
    }

    /// Point each of the callee's LINKAGE slots at its argument.
    fn bind_args(&mut self, c: usize, using: &[CallArg], line: u32) -> GenResult<()> {
        let params = self.units[c].params.clone();
        for (i, p) in params.iter().enumerate() {
            let Some(arg) = using.get(i) else {
                // Omitted: the slot is empty, and using the record is an error.
                self.base();
                self.i32c(0);
                self.raw(I::I32Store(wasm_encoder::MemArg { offset: p.slot as u64, align: 2, memory_index: 0 }));
                continue;
            };
            let (expr, by_ref) = match arg {
                CallArg::ByReference(e) => (e, true),
                CallArg::ByContent(e) | CallArg::ByValue(e) => (e, false),
            };
            if let CallArg::ByValue(e) = arg {
                // BY VALUE: the argument's value, moved into a copy that has the
                // parameter's own description — its USAGE, its PICTURE
                // (WASM-PARITY #9). BY CONTENT shares the argument's bytes, as
                // COBOL-85 defines it for descriptions that agree.
                let src = self.operand_or_temp(e)?;
                let tmp = self.layout.alloc_hidden(p.size.max(1));
                let pitem = self.layout.items[p.item].clone();
                let pfd = self.pool.fd_with_spec(&pitem.fd, pitem.pic_template.as_deref(), self.prog.decimal_comma, self.prog.currency);
                let dst = self.pool.opd_item(tmp, &pfd, &[], None);
                self.base();
                self.i32c(src as i32);
                self.i32c(dst as i32);
                self.call_rt("rt_move");
                self.base();
                self.base();
                self.i32c(tmp as i32);
                self.raw(I::I32Add);
                self.raw(I::I32Store(wasm_encoder::MemArg { offset: p.slot as u64, align: 2, memory_index: 0 }));
                continue;
            }
            if by_ref && Self::is_item_ref(expr) {
                let o = self.operand_or_temp(expr)?;
                self.base();
                self.i32c(p.slot as i32);
                self.i32c(o as i32);
                self.call_rt("rt_bind");
                continue;
            }
            // BY CONTENT, BY VALUE, or a literal / computed value: a copy.
            let src = self.operand_or_temp(expr)?;
            let arg_size = if Self::is_item_ref(expr) {
                match self.resolve_item_of(expr) {
                    Some(i) => self.layout.items[i].size,
                    None => 0,
                }
            } else {
                0
            };
            let tmp_len = p.size.max(arg_size).max(1);
            let tmp = self.layout.alloc_hidden(tmp_len);
            let pitem = self.layout.items[p.item].clone();
            let pfd = self.pool.fd_with_spec(&pitem.fd, pitem.pic_template.as_deref(), self.prog.decimal_comma, self.prog.currency);
            let dst = self.pool.opd_item(tmp, &pfd, &[], None);
            self.base();
            self.i32c(p.slot as i32);
            self.i32c(src as i32);
            self.i32c(tmp as i32);
            self.i32c(dst as i32);
            self.i32c(tmp_len as i32);
            self.call_rt("rt_bind_content");
        }
        // A RETURNING record in the callee's LINKAGE SECTION that no USING
        // parameter brings: the callee writes it, so it gets storage of its own
        // (one area per callee), and the caller reads it back after the call
        // (WASM-PARITY #13).
        if let Some(ri) = self.units[c].returning {
            let mut root = ri;
            while let Some(p) = self.layout.items[root].parent {
                root = p;
            }
            let passed = params.iter().any(|p| p.item == root);
            if let (Some(slot), false) = (self.layout.items[root].link, passed) {
                let off = match self.returning_store.get(&c) {
                    Some(&o) => o,
                    None => {
                        let it = &self.layout.items[root];
                        let size = (it.size * it.occurs.as_ref().map_or(1, |o| o.0)).max(1);
                        let o = self.layout.alloc_hidden(size);
                        self.returning_store.insert(c, o);
                        o
                    }
                };
                self.base();
                self.base();
                self.i32c(off as i32);
                self.raw(I::I32Add);
                self.raw(I::I32Store(wasm_encoder::MemArg { offset: slot as u64, align: 2, memory_index: 0 }));
            }
        }
        let _ = line;
        Ok(())
    }

    /// The layout item an item reference names, if it resolves.
    fn resolve_item_of(&mut self, e: &Expr) -> Option<usize> {
        match e {
            Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => self.resolve_item_of(base),
            other => self.resolve_item(other).ok(),
        }
    }

    /// `RETURNING x`: the callee's RETURNING item moved to `x`.
    fn move_returning(&mut self, c: usize, to: &Expr, line: u32) -> GenResult<()> {
        let Some(ri) = self.units[c].returning else { return Ok(()) };
        let src = self.item_operand(ri, &[], None, line)?;
        let dst = self.receiver(to)?;
        self.base();
        self.i32c(src as i32);
        self.i32c(dst as i32);
        self.call_rt("rt_move");
        Ok(())
    }

    /// The callable units' names, as `rt_find_program` reads them: a count,
    /// then (address, length) pairs, unit 1 first. The main program is not
    /// callable.
    fn program_table(&mut self) -> u32 {
        let names: Vec<String> = self.units.iter().skip(1).map(|u| u.name.clone()).collect();
        let mut words: Vec<u32> = vec![names.len() as u32];
        for n in &names {
            let a = self.pool.intern(n.as_bytes());
            words.push(a);
            words.push(n.len() as u32);
        }
        self.pool.spec(&words)
    }

    /// `CALL identifier`: the name is found at run time.
    fn call_dynamic(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        returning: Option<&Expr>,
        on_exc: &[Stmt],
        not_exc: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        if self.units.len() < 2 {
            // Nothing in the module can be called.
            return self.call_missing(on_exc);
        }
        let name = self.operand_or_temp(program)?;
        let table = self.program_table();
        let hold = self.layout.alloc_hidden(4);
        // hold = index of the program, or -1
        self.base();
        self.base();
        self.i32c(name as i32);
        self.i32c(table as i32);
        self.call_rt("rt_find_program");
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
        let join = self.new_block("end-call");
        let after = self.new_block("after-call");
        let missing = self.new_block("call-missing");
        let mut test = self.new_block("call-test");
        self.jump(test);
        // Each callee returns to a block of its own, which moves that callee's
        // RETURNING item (WASM-PARITY #9), then to the common continuation.
        let mut returns: Vec<(usize, BlockId)> = Vec::new();
        for c in 1..self.units.len() {
            self.begin(test);
            let hit = self.new_block("call-unit");
            let next = if c + 1 < self.units.len() { self.new_block("call-test") } else { missing };
            self.base();
            self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
            self.i32c(c as i32 - 1);
            self.raw(I::I32Eq);
            self.branch(hit, next);
            self.begin(hit);
            let back = match returning {
                Some(_) if self.units[c].returning.is_some() => {
                    let b = self.new_block("after-call-unit");
                    returns.push((c, b));
                    b
                }
                _ => after,
            };
            self.enter_callee(c, using, back, line)?;
            test = next;
        }
        if let Some(r) = returning {
            for (c, b) in returns {
                self.begin(b);
                self.move_returning(c, r, line)?;
                self.jump(after);
            }
        }
        // Found and returned: NOT ON EXCEPTION.
        self.begin(after);
        self.stmts(not_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(missing);
        self.call_missing(on_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    /// `CANCEL program …`: the program's storage returns to its initial state.
    pub(crate) fn cancel_(&mut self, programs: &[Expr], line: u32) -> GenResult<()> {
        for p in programs {
            if let Expr::Literal(Literal::String(name), _) = p {
                if let Some(&c) = self.unit_by_name.get(&name.trim().to_ascii_uppercase()) {
                    if c != 0 {
                        self.emit(Ins::InitUnit(self.unit_base + c));
                    }
                }
                continue;
            }
            if self.units.len() < 2 {
                continue;
            }
            let name = self.operand_or_temp(p)?;
            let table = self.program_table();
            let hold = self.layout.alloc_hidden(4);
            self.base();
            self.base();
            self.i32c(name as i32);
            self.i32c(table as i32);
            self.call_rt("rt_find_program");
            self.raw(I::I32Store(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
            let join = self.new_block("end-cancel");
            let mut test = self.new_block("cancel-test");
            self.jump(test);
            for c in 1..self.units.len() {
                self.begin(test);
                let hit = self.new_block("cancel-unit");
                let next = if c + 1 < self.units.len() { self.new_block("cancel-test") } else { join };
                self.base();
                self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
                self.i32c(c as i32 - 1);
                self.raw(I::I32Eq);
                self.branch(hit, next);
                self.begin(hit);
                self.emit(Ins::InitUnit(self.unit_base + c));
                self.jump(join);
                test = next;
            }
            self.begin(join);
        }
        let _ = line;
        Ok(())
    }

    // ── USE AFTER ERROR (conformance builds) ──────────────────────────────

    /// Region slot that remembers the status of the file verb just performed.
    pub(crate) fn status_slot(&mut self) -> u32 {
        match self.status_hold {
            Some(s) => s,
            None => {
                let s = self.layout.alloc_hidden(4);
                self.status_hold = Some(s);
                s
            }
        }
    }

    fn guard_slot(&mut self) -> u32 {
        match self.use_guard {
            Some(s) => s,
            None => {
                let s = self.layout.alloc_hidden(4);
                self.use_guard = Some(s);
                s
            }
        }
    }

    /// A file verb has just returned its status on the wasm stack: keep it in
    /// its slot, and — when the verb failed and nothing in the statement covers
    /// the failure — perform the first `USE AFTER ERROR` procedure that applies
    /// to this file or to the mode it is open in. `eof_covered` is true when
    /// the statement has an `AT END` phrase, which owns status 10.
    pub(crate) fn file_verb_done(&mut self, file: &str, handle_off: u32, eof_covered: bool) {
        self.file_verb_done_cover(file, handle_off, if eof_covered { Cover::Eof } else { Cover::None });
    }

    /// [`Self::file_verb_done`], with what the statement's own phrase covers:
    /// a status it covers performs no USE procedure (`fire_declarative`'s
    /// `phrase_present`).
    pub(crate) fn file_verb_done_cover(&mut self, file: &str, handle_off: u32, cover: Cover) {
        let hold = self.status_slot();
        // [status] -> hold
        self.raw(I::LocalSet(0));
        self.base();
        self.raw(I::LocalGet(0));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
        // The handlers that can answer: this unit's, and the GLOBAL ones of the
        // units that contain it.
        let name = file.to_ascii_uppercase();
        let mut hs: Vec<crate::lower::Handler> = self.handlers[self.cur_unit].clone();
        for &u in self.units[self.cur_unit].scope.clone().iter().skip(1) {
            hs.extend(self.handlers[u as usize].iter().filter(|h| h.global).cloned());
        }
        if hs.is_empty() || cover == Cover::All {
            return;
        }
        let guard = self.guard_slot();
        let test = self.new_block("use-test");
        let join = self.new_block("use-done");
        // status >= 10 (and not the end of file the statement handles) and no
        // USE procedure running
        self.base();
        self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
        self.i32c(10);
        self.raw(I::I32GeU);
        match cover {
            Cover::Eof => {
                self.base();
                self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
                self.i32c(10);
                self.raw(I::I32Ne);
                self.raw(I::I32And);
            }
            // 21 to 24, the INVALID KEY conditions: (status - 21) > 3, unsigned.
            Cover::Keys => {
                self.base();
                self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
                self.i32c(21);
                self.raw(I::I32Sub);
                self.i32c(3);
                self.raw(I::I32GtU);
                self.raw(I::I32And);
            }
            Cover::None | Cover::All => {}
        }
        self.base();
        self.raw(I::I32Load(wasm_encoder::MemArg { offset: guard as u64, align: 2, memory_index: 0 }));
        self.raw(I::I32Eqz);
        self.raw(I::I32And);
        self.branch(test, join);
        self.begin(test);
        let after = self.new_block("use-returned");
        let mut at = test;
        let _ = &mut at;
        for h in hs {
            let run = self.new_block("use-run");
            let next = self.new_block("use-next");
            // Does this handler cover the file?
            if h.catch_all || h.files.iter().any(|f| *f == name) {
                self.i32c(1);
            } else if h.modes.is_empty() {
                self.i32c(0);
            } else {
                // by the mode the file is open in (stored after the handle)
                let mut first = true;
                for m in &h.modes {
                    let want = match m {
                        cobolt_ast::program::UseMode::Input => 1,
                        cobolt_ast::program::UseMode::Output => 2,
                        cobolt_ast::program::UseMode::Io => 3,
                        cobolt_ast::program::UseMode::Extend => 4,
                    };
                    self.base();
                    self.raw(I::I32Load(wasm_encoder::MemArg { offset: handle_off as u64 + 4, align: 2, memory_index: 0 }));
                    self.i32c(want);
                    self.raw(I::I32Eq);
                    if !first {
                        self.raw(I::I32Or);
                    }
                    first = false;
                }
            }
            self.branch(run, next);
            self.begin(run);
            // guard = 1; perform the handler; guard = 0
            self.base();
            self.i32c(1);
            self.raw(I::I32Store(wasm_encoder::MemArg { offset: guard as u64, align: 2, memory_index: 0 }));
            self.frame_jump(h.end, h.entry, after);
            self.begin(next);
        }
        // No handler applies.
        self.jump(join);
        self.begin(after);
        self.base();
        self.i32c(0);
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: guard as u64, align: 2, memory_index: 0 }));
        self.jump(join);
        self.begin(join);
    }
}

/// Which statuses of a file verb the statement's own phrase handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cover {
    /// None: every error status goes to the USE procedures.
    None,
    /// `AT END` handles status 10.
    Eof,
    /// `INVALID KEY` on a keyed `READ` handles 21 to 24.
    Keys,
    /// The statement handles every status (an `INVALID KEY` phrase on
    /// `WRITE`/`REWRITE`/`DELETE`), or performs no USE procedure (`START`).
    All,
}
