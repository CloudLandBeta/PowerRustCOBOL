// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The statements of phase B: tables and strings (`SEARCH`, `INSPECT`,
//! `STRING`, `UNSTRING`), group operations (`CORRESPONDING`, `INITIALIZE`) and
//! `ALTER`.

use crate::error::{GenError, GenResult};
use crate::lower::Lower;
use cobolt_ast::data::PicKind;
use cobolt_ast::expr::*;
use cobolt_ast::stmt::*;
use cobolt_wasm_rt::arith::{OP_ADD, OP_SUB, ST_PHRASE, ST_ROUNDED};
use cobolt_wasm_rt::cmp::{OP_GT, OP_LE};
use cobolt_wasm_rt::fd::*;
use wasm_encoder::Instruction as I;

/// A lowered intrinsic-function call: its record, and whether it yields
/// characters (otherwise a number).
pub(crate) struct FuncCall {
    pub rec: u32,
    pub chars: bool,
}

impl Lower<'_, '_> {
    // ── intrinsic functions ───────────────────────────────────────────────

    /// Is `e` an alphanumeric argument (a string literal, a character item, a
    /// character-valued function)?
    fn expr_is_chars(&mut self, e: &Expr) -> bool {
        match e {
            Expr::Literal(Literal::String(_), _) => true,
            Expr::Identifier(..) | Expr::Qualified { .. } => match self.resolve_item(e) {
                Ok(i) => {
                    let it = &self.layout.items[i];
                    it.level != 88 && !it.fd.is_numeric() && it.fd.kind != K_NUM_EDITED
                }
                Err(_) => false,
            },
            Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => self.expr_is_chars(base),
            Expr::FunctionCall { name, args, .. } => {
                let up = name.to_ascii_uppercase();
                if up == "WHEN-COMPILED" {
                    return true;
                }
                let flat: Vec<Expr> = args.iter().flat_map(|a| self.all_elements(a).unwrap_or_else(|_| vec![a.clone()])).collect();
                matches!(self.function_id(&up, &flat), Some(id) if id >= 64)
            }
            _ => false,
        }
    }

    /// The runtime number of function `up` for these (flattened) arguments.
    fn function_id(&mut self, up: &str, flat: &[Expr]) -> Option<u32> {
        use cobolt_wasm_rt::func::{id, NAMES};
        let char_args = flat.iter().any(|a| self.expr_is_chars(a));
        match up {
            "MAX" if char_args => Some(id::MAX_CHARS),
            "MIN" if char_args => Some(id::MIN_CHARS),
            _ => NAMES.iter().find(|(n, _)| *n == up).map(|(_, i)| *i),
        }
    }

    /// `ALL` subscripts spelled out: `T (ALL, 2)` is one element per occurrence
    /// of the first dimension, row-major. A variable-length table is refused:
    /// its count is a run-time fact.
    fn all_elements(&mut self, e: &Expr) -> GenResult<Vec<Expr>> {
        let Expr::Subscript { base, indices, span } = e else {
            return Ok(vec![e.clone()]);
        };
        if !indices.iter().any(|i| matches!(i, Expr::AllSubscript(_))) {
            return Ok(vec![e.clone()]);
        }
        let line = e.span().line;
        let idx = self.resolve_item(base)?;
        let chain = self.layout.occurs_chain(idx);
        if chain.len() != indices.len() {
            return self.err(line, "the subscripts do not match the table's dimensions");
        }
        let mut combos: Vec<Vec<Expr>> = vec![Vec::new()];
        for (pos, ix) in indices.iter().enumerate() {
            let choices: Vec<Expr> = if matches!(ix, Expr::AllSubscript(_)) {
                let (_, max, odo) = &chain[pos];
                if odo.is_some() {
                    return self.err(line, "ALL over a variable-length table is not supported in a WebAssembly module yet");
                }
                (1..=*max as i64).map(|n| Expr::Literal(Literal::Integer(n), *span)).collect()
            } else {
                vec![ix.clone()]
            };
            let mut next = Vec::with_capacity(combos.len() * choices.len());
            for c in &combos {
                for ch in &choices {
                    let mut v = c.clone();
                    v.push(ch.clone());
                    next.push(v);
                }
            }
            combos = next;
        }
        Ok(combos.into_iter().map(|indices| Expr::Subscript { base: base.clone(), indices, span: *span }).collect())
    }

    /// Lower a call: evaluate its arguments (nested calls and computed
    /// arguments first) and build the record the routines read.
    pub(crate) fn function_call(&mut self, name: &str, args: &[Expr], line: u32) -> GenResult<FuncCall> {
        let up = name.to_ascii_uppercase();
        let mut flat: Vec<Expr> = Vec::new();
        for a in args {
            flat.extend(self.all_elements(a)?);
        }
        if flat.len() > cobolt_wasm_rt::func::MAX_ARGS {
            return self.err(line, format!("FUNCTION {name} takes at most {} arguments here", cobolt_wasm_rt::func::MAX_ARGS));
        }
        let Some(fid) = self.function_id(&up, &flat) else {
            return self.err(line, format!("FUNCTION {name} is not supported in a WebAssembly module yet"));
        };
        let mut words: Vec<u32> = vec![fid, flat.len() as u32, u32::from(self.prog.decimal_comma), self.prog.currency as u32];
        for a in &flat {
            words.push(self.operand_or_temp(a)?);
        }
        let rec = self.pool.spec(&words);
        Ok(FuncCall { rec, chars: fid >= 64 })
    }

    /// The operand for a character-valued function (evaluated into its
    /// temporary), or `None` for a numeric one.
    pub(crate) fn function_operand(&mut self, name: &str, args: &[Expr], line: u32) -> GenResult<Option<u32>> {
        let up = name.to_ascii_uppercase();
        if up == "WHEN-COMPILED" {
            let text = self.when_compiled.clone();
            return Ok(Some(self.pool.opd_lit_alnum(text.as_bytes())));
        }
        let mut flat: Vec<Expr> = Vec::new();
        for a in args {
            flat.extend(self.all_elements(a)?);
        }
        match self.function_id(&up, &flat) {
            Some(id) if id >= 64 => {}
            _ => return Ok(None),
        }
        let f = self.function_call(name, args, line)?;
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.err(line, "this statement needs more than four character-valued functions");
        }
        self.ctemp_next += 1;
        self.base();
        self.i32c(f.rec as i32);
        self.i32c(k as i32);
        self.call_rt("rt_func_chars");
        Ok(Some(self.pool.opd_ctemp(k)))
    }

    // ── computed operands ─────────────────────────────────────────────────

    /// An operand record for `e`, computing it into a temporary first when it
    /// is an expression (a subscript `I + J`, a reference-modification bound,
    /// a delimiter built by `FUNCTION`).
    pub(crate) fn operand_or_temp(&mut self, e: &Expr) -> GenResult<u32> {
        if let Some(o) = self.operand(e)? {
            return Ok(o);
        }
        let line = e.span().line;
        let k = self.temp_next;
        if k >= cobolt_wasm_rt::state::NTEMP as u32 {
            return self.err(line, "this statement needs more than eight computed operands");
        }
        self.temp_next += 1;
        self.push_expr(e)?;
        self.i32c(k as i32);
        self.call_rt("rt_save_temp");
        Ok(self.pool.opd_temp(k))
    }

    // ── overflow phrases ──────────────────────────────────────────────────

    /// With an overflow flag (0/1) on the stack, run `ON OVERFLOW` or
    /// `NOT ON OVERFLOW`; with neither phrase, discard it.
    pub(crate) fn overflow_tail(&mut self, on: &[Stmt], not_on: &[Stmt]) -> GenResult<()> {
        if on.is_empty() && not_on.is_empty() {
            self.raw(I::Drop);
            return Ok(());
        }
        let then_b = self.new_block("overflow");
        let else_b = self.new_block("no-overflow");
        let join = self.new_block("overflow-join");
        self.branch(then_b, else_b);
        self.begin(then_b);
        self.stmts(on)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(else_b);
        self.stmts(not_on)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    // ── INSPECT ───────────────────────────────────────────────────────────

    fn region(&mut self, r: &InspectRegion) -> GenResult<(u32, u32)> {
        let before = match &r.before {
            Some(e) => self.operand_or_temp(e)?,
            None => 0,
        };
        let after = match &r.after {
            Some(e) => self.operand_or_temp(e)?,
            None => 0,
        };
        Ok((before, after))
    }

    pub(crate) fn inspect_(&mut self, target: &Expr, spec: &InspectSpec, line: u32) -> GenResult<()> {
        let t = self.receiver(target)?;
        // (kind, counter, pattern, by, before, after)
        let mut ops: Vec<[u32; 6]> = Vec::new();
        let mut tallies = |me: &mut Self, ts: &[TallySpec], ops: &mut Vec<[u32; 6]>| -> GenResult<()> {
            for ts in ts {
                let counter = me.receiver(&ts.counter)?;
                for (kind, region) in &ts.for_ {
                    let (before, after) = me.region(region)?;
                    let (k, pat) = match kind {
                        TallyFor::Characters => (0, 0),
                        TallyFor::All(e) => (1, me.operand_or_temp(e)?),
                        TallyFor::Leading(e) => (2, me.operand_or_temp(e)?),
                        TallyFor::Trailing(e) => (3, me.operand_or_temp(e)?),
                    };
                    ops.push([k, counter, pat, 0, before, after]);
                }
            }
            Ok(())
        };
        let replaces = |me: &mut Self, rs: &[ReplaceSpec], ops: &mut Vec<[u32; 6]>| -> GenResult<()> {
            for r in rs {
                let (before, after) = me.region(&r.region)?;
                let by = me.operand_or_temp(&r.by)?;
                let (k, pat) = match &r.what {
                    ReplaceWhat::Characters => (4, 0),
                    ReplaceWhat::All(e) => (5, me.operand_or_temp(e)?),
                    ReplaceWhat::Leading(e) => (6, me.operand_or_temp(e)?),
                    ReplaceWhat::Trailing(e) => (7, me.operand_or_temp(e)?),
                    ReplaceWhat::First(e) => (8, me.operand_or_temp(e)?),
                };
                ops.push([k, 0, pat, by, before, after]);
            }
            Ok(())
        };
        match spec {
            InspectSpec::Tallying(ts) => tallies(self, ts, &mut ops)?,
            InspectSpec::Replacing(rs) => replaces(self, rs, &mut ops)?,
            InspectSpec::TallyingReplacing(ts, rs) => {
                tallies(self, ts, &mut ops)?;
                replaces(self, rs, &mut ops)?;
            }
            InspectSpec::Converting { from, to } => {
                let (f, t2) = (self.operand_or_temp(from)?, self.operand_or_temp(to)?);
                ops.push([9, 0, f, t2, 0, 0]);
            }
            InspectSpec::ConvertingIn { from, to, region } => {
                let (before, after) = self.region(region)?;
                let (f, t2) = (self.operand_or_temp(from)?, self.operand_or_temp(to)?);
                ops.push([9, 0, f, t2, before, after]);
            }
        }
        if ops.len() > 16 {
            return self.err(line, "INSPECT with more than sixteen phrases is not supported");
        }
        let mut words = vec![ops.len() as u32];
        for o in &ops {
            words.extend_from_slice(o);
        }
        let spec_addr = self.pool.spec(&words);
        self.base();
        self.i32c(t as i32);
        self.i32c(spec_addr as i32);
        self.call_rt("rt_inspect");
        Ok(())
    }

    // ── STRING / UNSTRING ─────────────────────────────────────────────────

    pub(crate) fn string_(
        &mut self,
        operands: &[(Expr, Option<Expr>)],
        into: &Expr,
        pointer: Option<&Expr>,
        on: &[Stmt],
        not_on: &[Stmt],
    ) -> GenResult<()> {
        let into_o = self.receiver(into)?;
        let ptr = match pointer {
            Some(p) => self.receiver(p)?,
            None => 0,
        };
        let mut words = vec![operands.len() as u32, into_o, ptr];
        for (src, delim) in operands {
            words.push(self.operand_or_temp(src)?);
            words.push(match delim {
                Some(d) => self.operand_or_temp(d)?,
                None => 0,
            });
        }
        let spec = self.pool.spec(&words);
        self.base();
        self.i32c(spec as i32);
        self.call_rt("rt_string");
        self.overflow_tail(on, not_on)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn unstring_(
        &mut self,
        from: &Expr,
        delimited_by: &[Expr],
        all: bool,
        into: &[UnstringTarget],
        pointer: Option<&Expr>,
        tallying: Option<&Expr>,
        on: &[Stmt],
        not_on: &[Stmt],
    ) -> GenResult<()> {
        let src = self.operand_or_temp(from)?;
        let ptr = match pointer {
            Some(p) => self.receiver(p)?,
            None => 0,
        };
        let tally = match tallying {
            Some(t) => self.receiver(t)?,
            None => 0,
        };
        let mut words = vec![all as u32, src, ptr, tally, delimited_by.len() as u32];
        for d in delimited_by {
            words.push(self.operand_or_temp(d)?);
        }
        words.push(into.len() as u32);
        for t in into {
            words.push(self.receiver(&t.target)?);
            words.push(match &t.delimiter {
                Some(d) => self.receiver(d)?,
                None => 0,
            });
            words.push(match &t.count {
                Some(c) => self.receiver(c)?,
                None => 0,
            });
        }
        let spec = self.pool.spec(&words);
        self.base();
        self.i32c(spec as i32);
        self.call_rt("rt_unstring");
        self.overflow_tail(on, not_on)
    }

    // ── SEARCH ────────────────────────────────────────────────────────────

    /// `idx += 1`.
    fn bump(&mut self, idx_opd: u32) {
        let one = self.pool.opd_lit_num(1, 0);
        self.call_rt("rt_begin");
        self.base();
        self.i32c(idx_opd as i32);
        self.call_rt("rt_push");
        self.base();
        self.i32c(one as i32);
        self.call_rt("rt_push");
        self.i32c(OP_ADD as i32);
        self.call_rt("rt_op");
        self.base();
        self.i32c(idx_opd as i32);
        self.i32c(0);
        self.call_rt("rt_store");
        self.raw(I::Drop);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_(
        &mut self,
        all: bool,
        table: &Expr,
        varying: Option<&Expr>,
        at_end: &[Stmt],
        whens: &[(Condition, Vec<Stmt>)],
        line: u32,
    ) -> GenResult<()> {
        // The table element: `table` names the item carrying OCCURS (possibly
        // subscripted in its outer dimensions).
        let (base_expr, outer): (&Expr, &[Expr]) = match table {
            Expr::Subscript { base, indices, .. } => (base.as_ref(), indices.as_slice()),
            other => (other, &[]),
        };
        let tidx = self.resolve_item(base_expr)?;
        let item = self.layout.items[tidx].clone();
        let Some((max, odo)) = item.occurs.clone() else {
            return self.err(line, "SEARCH needs a table (an item with OCCURS)");
        };
        // The search index and the item stepped alongside it.
        let own = item.index_names.first().cloned();
        let own_idx = match &own {
            Some(n) => self.layout.index_item(n),
            None => None,
        };
        let (idx_item, stepped): (usize, Option<usize>) = match varying {
            Some(v) => {
                let vi = self.resolve_item(match v {
                    Expr::Subscript { base, .. } => base.as_ref(),
                    other => other,
                })?;
                // VARYING one of this table's own indexes (any of its INDEXED BY
                // names) makes that index the driver.
                let is_own = self.layout.items[vi]
                    .name
                    .as_ref()
                    .is_some_and(|n| item.index_names.iter().any(|x| x == n));
                match (is_own, own_idx) {
                    (true, _) => (vi, None),
                    (false, Some(oi)) => (oi, Some(vi)),
                    (false, None) => (vi, None),
                }
            }
            None => match own_idx {
                Some(oi) => (oi, None),
                None => return self.err(line, "SEARCH needs an INDEXED BY index or a VARYING item"),
            },
        };
        let idx_opd = self.item_operand(idx_item, &[], None, line)?;
        let stepped_opd = match stepped {
            Some(si) => Some(self.item_operand(si, &[], None, line)?),
            None => None,
        };
        // The number of active occurrences.
        let size_opd = match odo {
            Some(name) => {
                let di = self.layout.lookup(&name, &[]).map_err(|m| GenError::new(line, m))?;
                self.item_operand(di, &[], None, line)?
            }
            None => self.pool.opd_lit_num(max as i128, 0),
        };
        let _ = outer;
        let join = self.new_block("end-search");
        let head = self.new_block("search");
        let at_end_b = self.new_block("search-at-end");

        // SEARCH ALL with exactly one WHEN and declared keys: binary search.
        if all && !item.keys.is_empty() && whens.len() == 1 {
            return self.search_all(&item.keys, idx_opd, size_opd, &whens[0], at_end, join, head, at_end_b, line);
        }

        // Serial search (and SEARCH ALL without keys): from the current index
        // (from 1 for ALL).
        if all {
            let one = self.pool.opd_lit_num(1, 0);
            self.base();
            self.i32c(one as i32);
            self.i32c(idx_opd as i32);
            self.call_rt("rt_move");
        }
        self.jump(head);
        self.begin(head);
        // idx > size -> AT END
        self.base();
        self.i32c(idx_opd as i32);
        self.i32c(size_opd as i32);
        self.i32c(OP_GT as i32);
        self.call_rt("rt_rel");
        let body_start = self.new_block("search-when");
        self.branch(at_end_b, body_start);
        self.begin(body_start);
        for (cond, body) in whens {
            let hit = self.new_block("search-hit");
            let next = self.new_block("search-next");
            self.cond(cond)?;
            self.branch(hit, next);
            self.begin(hit);
            self.stmts(body)?;
            if self.cur.is_some() {
                self.jump(join);
            }
            self.begin(next);
        }
        // No WHEN matched: step and loop.
        self.bump(idx_opd);
        if let Some(s) = stepped_opd {
            self.bump(s);
        }
        self.jump(head);
        self.begin(at_end_b);
        self.stmts(at_end)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn search_all(
        &mut self,
        keys: &[(String, bool)],
        idx_opd: u32,
        size_opd: u32,
        when: &(Condition, Vec<Stmt>),
        at_end: &[Stmt],
        join: usize,
        head: usize,
        at_end_b: usize,
        line: u32,
    ) -> GenResult<()> {
        use wasm_encoder::MemArg;
        // lo, hi, mid live in three hidden eight-byte slots.
        let slot = self.layout.alloc_hidden(24);
        let (lo, hi, mid) = (slot as u64, slot as u64 + 8, slot as u64 + 16);
        let m = |off: u64| MemArg { offset: off, align: 3, memory_index: 0 };
        // The equality tests of the WHEN, major key first.
        let mut pairs: Vec<(&Expr, &Expr)> = Vec::new();
        fn flatten<'x>(c: &'x Condition, out: &mut Vec<(&'x Expr, &'x Expr)>) {
            match c {
                Condition::And(a, b, _) => {
                    flatten(a, out);
                    flatten(b, out);
                }
                Condition::Comparison { lhs, rhs, .. } => out.push((lhs, rhs)),
                _ => {}
            }
        }
        flatten(&when.0, &mut pairs);
        // lo = 1; hi = size
        self.base();
        self.raw(I::I64Const(1));
        self.raw(I::I64Store(m(lo)));
        self.base();
        self.base();
        self.i32c(size_opd as i32);
        self.call_rt("rt_int");
        self.raw(I::I64Store(m(hi)));
        self.jump(head);
        self.begin(head);
        // lo > hi -> AT END
        self.base();
        self.raw(I::I64Load(m(lo)));
        self.base();
        self.raw(I::I64Load(m(hi)));
        self.raw(I::I64GtS);
        let probe = self.new_block("search-all-probe");
        self.branch(at_end_b, probe);
        self.begin(probe);
        // mid = lo + (hi - lo) / 2 ; index = mid
        self.base();
        self.base();
        self.raw(I::I64Load(m(lo)));
        self.base();
        self.raw(I::I64Load(m(hi)));
        self.base();
        self.raw(I::I64Load(m(lo)));
        self.raw(I::I64Sub);
        self.raw(I::I64Const(2));
        self.raw(I::I64DivS);
        self.raw(I::I64Add);
        self.raw(I::I64Store(m(mid)));
        self.base();
        self.i32c(idx_opd as i32);
        self.base();
        self.raw(I::I64Load(m(mid)));
        self.call_rt("rt_set_int");
        // WHEN true -> body
        let hit = self.new_block("search-all-hit");
        let miss = self.new_block("search-all-miss");
        self.cond(&when.0)?;
        self.branch(hit, miss);
        self.begin(hit);
        self.stmts(&when.1)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(miss);
        // dir (local 1): the first key whose value differs decides the half.
        self.i32c(0);
        self.raw(I::LocalSet(1));
        for (l, r) in pairs {
            let field = match l {
                Expr::Subscript { base, .. } => self.resolve_item(base).ok(),
                other => self.resolve_item(other).ok(),
            }
            .and_then(|i| self.layout.items[i].name.clone())
            .unwrap_or_default();
            let ascending = keys.iter().find(|(k, _)| *k == field).map(|(_, a)| *a).unwrap_or(true);
            let (lo_opd, ro) = (self.operand_or_temp(l)?, self.operand_or_temp(r)?);
            self.raw(I::LocalGet(1));
            self.raw(I::I32Eqz);
            self.raw(I::If(wasm_encoder::BlockType::Empty));
            self.base();
            self.i32c(lo_opd as i32);
            self.i32c(ro as i32);
            self.call_rt("rt_cmp3");
            self.raw(I::LocalSet(1));
            if !ascending {
                self.i32c(0);
                self.raw(I::LocalGet(1));
                self.raw(I::I32Sub);
                self.raw(I::LocalSet(1));
            }
            self.raw(I::End);
        }
        let go_lo = self.new_block("search-all-lo");
        let rest = self.new_block("search-all-rest");
        self.raw(I::LocalGet(1));
        self.i32c(0);
        self.raw(I::I32LtS);
        self.branch(go_lo, rest);
        self.begin(go_lo);
        // lo = mid + 1
        self.base();
        self.base();
        self.raw(I::I64Load(m(mid)));
        self.raw(I::I64Const(1));
        self.raw(I::I64Add);
        self.raw(I::I64Store(m(lo)));
        self.jump(head);
        self.begin(rest);
        // dir is carried in local 1 only within a block; recompute for the
        // upper half by re-running the comparison block.
        let upper = self.new_block("search-all-hi");
        // keys differ and dir > 0 -> hi = mid - 1 ; equal keys but WHEN false -> not found
        self.i32c(0);
        self.raw(I::LocalSet(1));
        // (re-evaluate: cheap, and keeps each block free of cross-block state)
        let mut pairs2: Vec<(&Expr, &Expr)> = Vec::new();
        flatten(&when.0, &mut pairs2);
        for (l, r) in pairs2 {
            let field = match l {
                Expr::Subscript { base, .. } => self.resolve_item(base).ok(),
                other => self.resolve_item(other).ok(),
            }
            .and_then(|i| self.layout.items[i].name.clone())
            .unwrap_or_default();
            let ascending = keys.iter().find(|(k, _)| *k == field).map(|(_, a)| *a).unwrap_or(true);
            let (lo_opd, ro) = (self.operand_or_temp(l)?, self.operand_or_temp(r)?);
            self.raw(I::LocalGet(1));
            self.raw(I::I32Eqz);
            self.raw(I::If(wasm_encoder::BlockType::Empty));
            self.base();
            self.i32c(lo_opd as i32);
            self.i32c(ro as i32);
            self.call_rt("rt_cmp3");
            self.raw(I::LocalSet(1));
            if !ascending {
                self.i32c(0);
                self.raw(I::LocalGet(1));
                self.raw(I::I32Sub);
                self.raw(I::LocalSet(1));
            }
            self.raw(I::End);
        }
        self.raw(I::LocalGet(1));
        self.i32c(0);
        self.raw(I::I32GtS);
        self.branch(upper, at_end_b);
        self.begin(upper);
        // hi = mid - 1
        self.base();
        self.base();
        self.raw(I::I64Load(m(mid)));
        self.raw(I::I64Const(1));
        self.raw(I::I64Sub);
        self.raw(I::I64Store(m(hi)));
        self.jump(head);
        // AT END
        self.begin(at_end_b);
        self.stmts(at_end)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        let _ = (line, OP_LE);
        Ok(())
    }

    // ── CORRESPONDING ─────────────────────────────────────────────────────

    fn corr_pairs(&self, from: usize, to: usize, out: &mut Vec<(usize, usize, bool)>) {
        for &fc in &self.layout.items[from].children {
            let fi = &self.layout.items[fc];
            let Some(fname) = &fi.name else { continue };
            let Some(&tc) = self.layout.items[to]
                .children
                .iter()
                .find(|&&c| self.layout.items[c].name.as_deref() == Some(fname.as_str()))
            else {
                continue;
            };
            let ti = &self.layout.items[tc];
            // An item described with REDEFINES takes no part (COBOL-85 6.18.4 GR1).
            if fi.redefines.is_some() || ti.redefines.is_some() {
                continue;
            }
            let both = fi.is_group && ti.is_group;
            out.push((fc, tc, both));
            if both {
                self.corr_pairs(fc, tc, out);
            }
        }
    }

    pub(crate) fn corresponding(
        &mut self,
        kind: u8,
        from: &Expr,
        to: &Expr,
        rounded: bool,
        on: &[Stmt],
        not_on: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let mut split = |me: &mut Self, e: &Expr| -> (usize, Vec<Expr>) {
            let (b, ix) = match e {
                Expr::Subscript { base, indices, .. } => (base.as_ref(), indices.clone()),
                other => (other, vec![]),
            };
            (me.resolve_item(b).unwrap_or(usize::MAX), ix)
        };
        let (fg, fidx) = split(self, from);
        let (tg, tidx) = split(self, to);
        if fg == usize::MAX || tg == usize::MAX {
            return self.err(line, "CORRESPONDING needs two group items");
        }
        let mut pairs = Vec::new();
        self.corr_pairs(fg, tg, &mut pairs);
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        for (fc, tc, both) in pairs {
            if both && kind != 0 {
                continue;
            }
            if both {
                continue; // groups recurse through their own pairs
            }
            let (fi, ti) = (self.layout.items[fc].clone(), self.layout.items[tc].clone());
            // Items with OCCURS of their own are not paired.
            if self.layout.occurs_chain(fc).len() != fidx.len() || self.layout.occurs_chain(tc).len() != tidx.len() {
                continue;
            }
            let fo = self.item_operand(fc, &fidx, None, line)?;
            let to_o = self.item_operand(tc, &tidx, None, line)?;
            if kind == 0 {
                self.base();
                self.i32c(fo as i32);
                self.i32c(to_o as i32);
                self.call_rt("rt_move");
            } else {
                if !(fi.fd.is_numeric() && ti.fd.is_numeric()) {
                    continue;
                }
                self.call_rt("rt_regs_reset");
                self.base();
                self.i32c(to_o as i32);
                self.call_rt("rt_push");
                self.base();
                self.i32c(fo as i32);
                self.call_rt("rt_push");
                self.i32c(if kind == 1 { OP_ADD } else { OP_SUB } as i32);
                self.call_rt("rt_op");
                self.base();
                self.i32c(to_o as i32);
                self.i32c((if rounded { ST_ROUNDED } else { 0 } | if phrase { ST_PHRASE } else { 0 }) as i32);
                self.call_rt("rt_store");
                self.raw(I::Drop);
            }
        }
        if kind != 0 {
            self.size_error_tail(on, not_on)?;
        }
        Ok(())
    }

    // ── INITIALIZE ────────────────────────────────────────────────────────

    pub(crate) fn initialize_(&mut self, items: &[Expr], replacing: &[(InitCategory, Expr)], line: u32) -> GenResult<()> {
        let mut entries: Vec<[u32; 11]> = Vec::new();
        for target in items {
            let (b, ix): (&Expr, &[Expr]) = match target {
                Expr::Subscript { base, indices, .. } => (base.as_ref(), indices.as_slice()),
                other => (other, &[]),
            };
            if matches!(b, Expr::Member { .. }) {
                return self.err(line, "INITIALIZE of a control property is not supported in this build of the generator yet");
            }
            let root = self.resolve_item(b)?;
            // The target's own chain is fixed by its subscripts; deeper OCCURS
            // levels of leaves below it are initialised in full.
            let fixed = self.layout.occurs_chain(root).len().min(ix.len());
            let mut leaves = Vec::new();
            self.init_leaves(root, &mut leaves);
            for leaf in leaves {
                let item = self.layout.items[leaf].clone();
                let chain = self.layout.occurs_chain(leaf);
                // Operand of the first occurrence: the target's subscripts for
                // the fixed levels, 1 for the rest.
                let one = Expr::Literal(Literal::Integer(1), cobolt_lexer::Span::new(0, 0, line, 0));
                let mut sub_exprs: Vec<Expr> = ix[..fixed].to_vec();
                while sub_exprs.len() < chain.len() {
                    sub_exprs.push(one.clone());
                }
                let first = self.item_operand(leaf, &sub_exprs, None, line)?;
                // Replacement value by category.
                let repl = if replacing.is_empty() {
                    0
                } else {
                    let Some(cat) = item.pic_kind.map(init_category) else { continue };
                    match replacing.iter().find(|(c, _)| *c == cat) {
                        Some((_, e)) => self.operand_or_temp(e)?,
                        None => continue,
                    }
                };
                let free: Vec<(u32, u32)> = chain[fixed..].iter().map(|(stride, max, _)| (*stride, *max)).collect();
                if free.len() > 4 {
                    return self.err(line, "INITIALIZE below more than four nested OCCURS is not supported");
                }
                let mut e = [0u32; 11];
                e[0] = first;
                e[1] = repl;
                e[2] = free.len() as u32;
                for (k, (stride, max)) in free.iter().enumerate() {
                    e[3 + 2 * k] = *stride;
                    e[4 + 2 * k] = *max;
                }
                entries.push(e);
            }
        }
        let mut words = vec![entries.len() as u32];
        for e in &entries {
            words.extend_from_slice(e);
        }
        let spec = self.pool.spec(&words);
        self.base();
        self.i32c(spec as i32);
        self.call_rt("rt_initialize");
        Ok(())
    }

    /// The elementary items under `idx` that INITIALIZE resets: not FILLER, not
    /// described with REDEFINES.
    fn init_leaves(&self, idx: usize, out: &mut Vec<usize>) {
        let it = &self.layout.items[idx];
        if it.redefines.is_some() {
            return;
        }
        if !it.is_group {
            if it.name.is_some() && it.level != 88 && it.level != 66 {
                out.push(idx);
            }
            return;
        }
        for &c in &it.children {
            self.init_leaves(c, out);
        }
    }

    // ── ALTER ─────────────────────────────────────────────────────────────

    pub(crate) fn alter_(&mut self, from: &str, to: &str, line: u32) -> GenResult<()> {
        let f = from.to_ascii_uppercase();
        let t = to.to_ascii_uppercase();
        let Some(&fi) = self.para_by_name.get(&f).and_then(|v| v.first()) else {
            return self.err(line, format!("paragraph {from} is not defined"));
        };
        let Some(&ti) = self.para_by_name.get(&t).and_then(|v| v.first()) else {
            return self.err(line, format!("paragraph {to} is not defined"));
        };
        let Some(&slot) = self.alter_slots.get(&fi) else {
            return self.err(line, format!("paragraph {from} has no GO TO to alter"));
        };
        self.base();
        self.emit(crate::ir::Ins::Pc(self.entries[ti]));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: slot as u64, align: 2, memory_index: 0 }));
        Ok(())
    }
}

/// `INITIALIZE … REPLACING` category of a picture.
fn init_category(k: PicKind) -> InitCategory {
    match k {
        PicKind::Alphabetic => InitCategory::Alphabetic,
        PicKind::Alphanumeric => InitCategory::Alphanumeric,
        PicKind::Numeric => InitCategory::Numeric,
        PicKind::AlphanumericEdited => InitCategory::AlphanumericEdited,
        PicKind::NumericEdited => InitCategory::NumericEdited,
        PicKind::National => InitCategory::National,
        PicKind::Utf8 => InitCategory::Utf8,
    }
}

#[allow(dead_code)]
const _: () = {
    let _ = (K_ALNUM, OP_LE);
};
