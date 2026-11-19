// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `TRY … CATCH … FINALLY … END-TRY` and `THROW` (DESKTOP-WASM-001 T45), computed as the
//! interpreter computes them.
//!
//! A `TRY` pushes a frame on the stack of `PERFORM` and `CALL` frames whose end word is
//! [`TRY_MARK`] (no block's index) and whose return word is the `TRY`'s landing block. A `THROW`
//! drops frames down to the nearest such frame and lands there with the resume slot holding
//! [`THROWN`]: every `TRY` takes a `THROW`, with a `CATCH` clause or without one, as the
//! interpreter's does. A `THROW` that finds no frame ends the run with the interpreter's words.
//!
//! When some `TRY` of the module has a `FINALLY`, the statements that leave a `TRY` without the
//! generator seeing where it is run that `FINALLY` on the way, as the interpreter's signal for
//! them runs it while it leaves the `TRY`: `STOP RUN`, the main program's `GOBACK` and `EXIT
//! PROGRAM`, and a called program's exit drop frames down to the nearest `TRY` frame and land
//! there with the resume slot holding the block that goes on once `FINALLY` has run. A jump the
//! generator does see (`GO TO`, `EXIT PARAGRAPH`, `EXIT SECTION`, `EXIT PERFORM`) drops the frames
//! of the `TRY`s it leaves and runs their `FINALLY`s itself, innermost first ([`Lower::escape`]).
//!
//! A `FINALLY` is lowered once: every way into it first stores, in the `TRY`'s own slot, the block
//! it goes on to.

use crate::error::{ErrorKind, GenResult};
use crate::ir::{BlockId, Ins, G};
use crate::lower::{Lower, TryCtx};
use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::stmt::Stmt;
use cobolt_wasm_rt::exc::{THROWN, TRY_MARK};
use cobolt_wasm_rt::fd::{K_ALNUM, K_GROUP};
use wasm_encoder::{BlockType, Instruction as I, MemArg};

/// Why a `THROW` whose operand is not alphanumeric text is refused.
const D_THROW_OPERAND: &str = "a THROW in a WebAssembly module names an alphanumeric literal or an alphanumeric or group item; the interpreter's text of any other value is not computed in a module, so the Build takes the Cargo path";
/// Why a `CATCH` item that is not alphanumeric is refused.
const D_CATCH_ITEM: &str = "a CATCH item in a WebAssembly module is an alphanumeric or group item; the message is not put into an item of another class in a module, so the Build takes the Cargo path";
/// Why a jump the generator cannot follow is refused inside a `TRY`.
const D_TRY_JUMP: &str = "inside a TRY, a WebAssembly module leaves the TRY only by a jump it can follow (GO TO a paragraph, EXIT PARAGRAPH, EXIT SECTION, EXIT PERFORM); this one goes where only the run knows, so the Build takes the Cargo path";

fn word(offset: u32) -> MemArg {
    MemArg { offset: offset as u64, align: 2, memory_index: 0 }
}

impl<'a, 'b> Lower<'a, 'b> {
    /// The region slot a landing reads: [`THROWN`] after a `THROW`, otherwise the block an
    /// unwinding `STOP RUN`, `GOBACK` or program exit goes on to once `FINALLY` has run. One slot
    /// serves the module: nothing of the program runs between its store and the landing's read.
    pub(crate) fn resume_slot(&mut self) -> u32 {
        match self.try_resume {
            Some(o) => o,
            None => {
                let o = self.layout.alloc_hidden(4);
                self.try_resume = Some(o);
                o
            }
        }
    }

    /// Store block `b`'s index in the region slot at `off`.
    fn store_pc(&mut self, off: u32, b: BlockId) {
        self.base();
        self.emit(Ins::Pc(b));
        self.raw(I::I32Store(word(off)));
    }

    /// Drop the frame on top of the stack.
    fn drop_frame(&mut self) {
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(8);
        self.raw(I::I32Sub);
        self.emit(Ins::GlobalSet(G::Psp));
    }

    /// Look down the stack for the nearest `TRY` frame. Found: local 1 is 1 and the frame is
    /// dropped (`psp` points at it, so its landing is the word at `psp + 4`). Not found: local 1 is
    /// 0 and the stack is as it was.
    fn find_try_frame(&mut self) {
        self.i32c(0);
        self.raw(I::LocalSet(1));
        self.emit(Ins::GlobalGet(G::Psp));
        self.raw(I::LocalSet(0));
        self.raw(I::Block(BlockType::Empty));
        self.raw(I::Loop(BlockType::Empty));
        // Nothing left below: no TRY is open.
        self.raw(I::LocalGet(0));
        self.stack_lo();
        self.raw(I::I32LeU);
        self.raw(I::BrIf(1));
        self.raw(I::LocalGet(0));
        self.i32c(8);
        self.raw(I::I32Sub);
        self.raw(I::LocalSet(0));
        self.raw(I::LocalGet(0));
        self.raw(I::I32Load(word(0)));
        self.i32c(TRY_MARK);
        self.raw(I::I32Ne);
        self.raw(I::BrIf(0));
        self.i32c(1);
        self.raw(I::LocalSet(1));
        self.raw(I::LocalGet(0));
        self.emit(Ins::GlobalSet(G::Psp));
        self.raw(I::End);
        self.raw(I::End);
    }

    /// End the current block by going to the landing of the frame `psp` points at.
    fn to_landing(&mut self) {
        self.emit(Ins::GlobalGet(G::Psp));
        self.raw(I::I32Load(word(4)));
        self.finish_raw();
    }

    /// `STOP RUN`, and the main program's `GOBACK` and `EXIT PROGRAM`, in a module where a `TRY`
    /// has a `FINALLY`: while a `TRY` is open, go to its landing first; its `FINALLY` runs, and the
    /// statement starts again here, until no `TRY` is left. The caller then emits the end itself.
    /// In any other module this emits nothing, so its bytes stay as they were.
    pub(crate) fn leave_open_tries_first(&mut self) {
        if !self.has_finally {
            return;
        }
        let again = self.new_block("leave-tries");
        self.jump(again);
        self.begin(again);
        let land = self.new_block("leave-try");
        let go = self.new_block("leave-run");
        self.find_try_frame();
        self.raw(I::LocalGet(1));
        self.branch(land, go);
        self.begin(land);
        let resume = self.resume_slot();
        self.store_pc(resume, again);
        self.to_landing();
        self.begin(go);
    }

    /// Leave the innermost `n` open `TRY`s for `target`: each one's frame is dropped and its
    /// `FINALLY` runs, innermost first, then control goes to `target`. With no `TRY` to leave this
    /// is the plain jump.
    pub(crate) fn escape(&mut self, n: usize, target: BlockId) {
        let first = self.tries.len() - n;
        for i in (first..self.tries.len()).rev() {
            self.drop_frame();
            if let Some((fin, slot)) = self.tries[i].finally {
                let next = self.new_block("leave-try");
                self.store_pc(slot, next);
                self.jump(fin);
                self.begin(next);
            }
        }
        self.jump(target);
    }

    /// The refusal of a jump inside a `TRY` whose destination only the run knows.
    pub(crate) fn refuse_in_try(&self, line: u32, name: &str) -> GenResult<()> {
        if self.tries.is_empty() {
            return Ok(());
        }
        self.err_kind(line, ErrorKind::Other, name.to_string(), format!("{name}: {D_TRY_JUMP}"))
    }

    /// `TRY … [CATCH [EXCEPTION] item] … [FINALLY] … END-TRY`. A `CATCH RUST-EXCEPTION` clause
    /// takes a panic of `EXEC RUST`, which a module never has, so it is not lowered: the
    /// interpreter would never run it either.
    pub(crate) fn try_catch(&mut self, body: &[Stmt], var: Option<&str>, catch: &[Stmt], finally: &[Stmt], line: u32) -> GenResult<()> {
        let into = match var {
            Some(v) => self.catch_operand(v, line)?,
            None => None,
        };
        let landing = self.new_block("try-landing");
        let join = self.new_block("end-try");
        let fin = if finally.is_empty() { None } else { Some((self.new_block("finally"), self.layout.alloc_hidden(4))) };
        // Leave for `join`, through FINALLY when there is one.
        let leave = |me: &mut Self| match fin {
            Some((f, slot)) => {
                me.store_pc(slot, join);
                me.jump(f);
            }
            None => me.jump(join),
        };

        // The frame: (TRY_MARK, the landing).
        self.emit(Ins::GlobalGet(G::Psp));
        self.stack_hi();
        self.raw(I::I32GeU);
        self.raw(I::If(BlockType::Empty));
        self.i32c(cobolt_wasm_rt::state::err::PERFORM_DEPTH as i32);
        self.call_rt("rt_fail");
        self.raw(I::End);
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(TRY_MARK);
        self.raw(I::I32Store(word(0)));
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(landing));
        self.raw(I::I32Store(word(4)));
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(8);
        self.raw(I::I32Add);
        self.emit(Ins::GlobalSet(G::Psp));

        self.tries.push(TryCtx { finally: fin });
        let r = self.stmts(body);
        self.tries.pop();
        r?;
        if self.cur.is_some() {
            self.drop_frame();
            leave(self);
        }

        // The landing: a THROW, or (in a module with a FINALLY) a statement that leaves the TRY
        // unseen and goes on, after FINALLY, to the block in the resume slot.
        self.begin(landing);
        if self.has_finally {
            let resume = self.resume_slot();
            let catch_b = self.new_block("catch");
            let unwind_b = self.new_block("try-unwind");
            self.base();
            self.raw(I::I32Load(word(resume)));
            self.i32c(THROWN);
            self.raw(I::I32Eq);
            self.branch(catch_b, unwind_b);
            self.begin(unwind_b);
            match fin {
                Some((f, slot)) => {
                    self.base();
                    self.base();
                    self.raw(I::I32Load(word(resume)));
                    self.raw(I::I32Store(word(slot)));
                    self.jump(f);
                }
                None => {
                    self.base();
                    self.raw(I::I32Load(word(resume)));
                    self.finish_raw();
                }
            }
            self.begin(catch_b);
        }
        if let Some(o) = into {
            self.base();
            self.i32c(o as i32);
            self.call_rt("rt_catch_bind");
        }
        self.stmts(catch)?;
        if self.cur.is_some() {
            leave(self);
        }

        // FINALLY, once: it goes on to the block its slot holds.
        if let Some((f, slot)) = fin {
            self.begin(f);
            self.stmts(finally)?;
            if self.cur.is_some() {
                self.base();
                self.raw(I::I32Load(word(slot)));
                self.finish_raw();
            }
        }
        self.begin(join);
        Ok(())
    }

    /// `THROW operand`: keep where its text is, and land at the nearest `TRY`, or end the run
    /// with the interpreter's words when none is open.
    pub(crate) fn throw_(&mut self, message: &Expr, line: u32) -> GenResult<()> {
        let o = self.throw_operand(message, line)?;
        let resume = self.resume_slot();
        self.base();
        self.i32c(o as i32);
        self.call_rt("rt_throw_set");
        self.find_try_frame();
        self.raw(I::LocalGet(1));
        self.raw(I::I32Eqz);
        self.raw(I::If(BlockType::Empty));
        self.call_rt("rt_throw_uncaught");
        self.raw(I::End);
        self.base();
        self.i32c(THROWN);
        self.raw(I::I32Store(word(resume)));
        self.to_landing();
        Ok(())
    }

    /// The operand of a `THROW`: alphanumeric text (a literal, an item, a group, a part of one).
    fn throw_operand(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        let text = match e {
            Expr::Literal(Literal::String(_), _) => true,
            Expr::RefMod { .. } => true,
            Expr::Identifier(..) | Expr::Qualified { .. } => self.resolve_item(e).is_ok_and(|i| self.is_text_item(i)),
            Expr::Subscript { base, .. } => self.resolve_item(base).is_ok_and(|i| self.is_text_item(i)),
            _ => false,
        };
        if !text {
            return self.err_kind(line, ErrorKind::Other, "THROW", format!("THROW: {D_THROW_OPERAND}"));
        }
        self.operand_or_temp(e)
    }

    /// The item a `CATCH` names, as an operand: `None` when no item has that name (the
    /// interpreter keeps the message under it, where no statement of the program can read it).
    fn catch_operand(&mut self, var: &str, line: u32) -> GenResult<Option<u32>> {
        let Ok(idx) = self.layout.lookup(var, &[]) else { return Ok(None) };
        if !self.is_text_item(idx) {
            return self.err_kind(line, ErrorKind::Other, var.to_string(), format!("{var}: {D_CATCH_ITEM}"));
        }
        self.item_operand(idx, &[], None, line).map(Some)
    }

    /// An alphanumeric item (`JUSTIFIED` or not, alphabetic or not) or a group.
    pub(crate) fn is_text_item(&self, i: usize) -> bool {
        let it = &self.layout.items[i];
        it.is_group || it.fd.kind == K_GROUP || it.fd.kind == K_ALNUM
    }
}
