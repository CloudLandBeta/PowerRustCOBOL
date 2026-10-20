// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! From a program's statements to blocks.
//!
//! The control flow is lowered to a graph of [`Block`]s (WASM-PLAN P3): every
//! `IF`, loop, `PERFORM` of a paragraph, `GO TO` and fall-through between
//! paragraphs ends a block, and a block names the next one by returning its
//! table index. Nothing uses the WebAssembly call stack, so a task can stop at
//! any block boundary and resume later (WASM-010 R9).
//!
//! What a statement *does* to data is a sequence of calls into the routines of
//! `cobolt-wasm-rt`, each handed the address of the current region and of
//! static operand records. That is what keeps the generated code small and the
//! semantics in one tested place.

use crate::error::{GenError, GenResult};
use crate::ir::*;
use crate::layout::Layout;
use crate::pool::{Pool, Sub};
use cobolt_ast::data::ConditionValue;
use cobolt_ast::expr::*;
use cobolt_ast::program::{DataSection, ProcedureBody, Program};
use cobolt_ast::stmt::*;
use cobolt_wasm_rt::arith::{OP_ADD, OP_DIV, OP_MUL, OP_NEG, OP_POW, OP_SUB, ST_PHRASE, ST_ROUNDED};
use cobolt_wasm_rt::cmp::{CLASS_ALPHABETIC, CLASS_LOWER, CLASS_NUMERIC, CLASS_UPPER, OP_EQ, OP_GE, OP_GT, OP_LE, OP_LT, OP_NE};
use cobolt_wasm_rt::cmp::{SIGN_NEGATIVE, SIGN_POSITIVE, SIGN_ZERO};
use cobolt_wasm_rt::opd::{FIG_HIGH, FIG_LOW, FIG_NULL, FIG_QUOTE, FIG_SPACE, FIG_ZERO};
use std::collections::{HashMap, HashSet};
use wasm_encoder::Instruction as I;

/// Fixed addresses the generated code refers to, decided before lowering.
#[derive(Debug, Clone, Copy)]
pub struct MemMap {
    pub pstack_base: u32,
    pub pstack_end: u32,
}

/// One paragraph, flattened out of its section.
struct Para<'a> {
    name: String,
    section: Option<String>,
    stmts: &'a [Stmt],
    line: u32,
}

/// An inline loop being lowered: where `EXIT PERFORM` and `EXIT PERFORM CYCLE`
/// go.
struct LoopCtx {
    exit: BlockId,
    cycle: BlockId,
}

pub struct Lowered {
    pub entry: BlockId,
    pub layout: Layout,
}

pub struct Lower<'a, 'b> {
    prog: &'a Program,
    pub layout: Layout,
    pool: &'b mut Pool,
    blocks: &'b mut Vec<Block>,
    map: MemMap,
    cur: Option<BlockId>,
    paras: Vec<Para<'a>>,
    para_by_name: HashMap<String, Vec<usize>>,
    sections: HashMap<String, (usize, usize)>,
    perform_ends: HashSet<usize>,
    entries: Vec<BlockId>,
    ends: Vec<BlockId>,
    loops: Vec<LoopCtx>,
    cur_para: usize,
    next_sentence: Option<BlockId>,
    /// Where control goes when the last paragraph falls off its end.
    program_end: BlockId,
}

fn up(s: &str) -> String {
    s.to_ascii_uppercase()
}

impl<'a, 'b> Lower<'a, 'b> {
    pub fn lower(
        prog: &'a Program,
        pool: &'b mut Pool,
        blocks: &'b mut Vec<Block>,
        map: MemMap,
    ) -> GenResult<Lowered> {
        let mut layout = Layout::new();
        if let Some(d) = &prog.data {
            for sec in &d.sections {
                match sec {
                    DataSection::WorkingStorage(decls) | DataSection::LocalStorage(decls) => layout.add_section(decls)?,
                    DataSection::FileSection(fds) => {
                        for fd in fds {
                            layout.add_section(&fd.records)?;
                        }
                    }
                    DataSection::Linkage(_) => {
                        return Err(GenError::new(
                            0,
                            "LINKAGE SECTION in the main program is not supported in a WebAssembly module",
                        ))
                    }
                    DataSection::Screen(_) => {
                        return Err(GenError::new(0, "SCREEN SECTION is not supported in a WebAssembly module; use a form"))
                    }
                }
            }
        }
        layout.finish();

        let mut l = Lower {
            prog,
            layout,
            pool,
            blocks,
            map,
            cur: None,
            paras: Vec::new(),
            para_by_name: HashMap::new(),
            sections: HashMap::new(),
            perform_ends: HashSet::new(),
            entries: Vec::new(),
            ends: Vec::new(),
            loops: Vec::new(),
            cur_para: 0,
            next_sentence: None,
            program_end: 0,
        };
        l.flatten()?;
        l.prescan()?;
        let entry = l.new_block("program");
        l.program_end = l.new_block("program-end");
        for i in 0..l.paras.len() {
            let n = format!("{}", l.paras[i].name);
            let e = l.new_block(&format!("{n}"));
            let x = l.new_block(&format!("{n}.end"));
            l.entries.push(e);
            l.ends.push(x);
        }
        // The program-end block.
        l.begin(l.program_end);
        l.emit(Ins::Raw(I::I32Const(ret::DONE)));
        l.finish_raw();

        // Entry: straight into the first paragraph.
        l.begin(entry);
        if let Some(first) = l.entries.first().copied() {
            l.jump(first);
        } else {
            l.jump(l.program_end);
        }
        for i in 0..l.paras.len() {
            l.lower_para(i)?;
        }
        Ok(Lowered { entry, layout: l.layout })
    }

    // ── block plumbing ────────────────────────────────────────────────────

    fn new_block(&mut self, name: &str) -> BlockId {
        self.blocks.push(Block { name: name.to_string(), code: Vec::new(), succ: Vec::new(), done: false });
        self.blocks.len() - 1
    }

    fn begin(&mut self, id: BlockId) {
        self.cur = Some(id);
    }

    fn emit(&mut self, i: Ins) {
        let id = self.cur.expect("emit with no current block");
        self.blocks[id].code.push(i);
    }

    fn raw(&mut self, i: I<'static>) {
        self.emit(Ins::Raw(i));
    }

    fn i32c(&mut self, v: i32) {
        self.raw(I::I32Const(v));
    }

    /// Terminate the current block by returning the code already on the stack.
    fn finish_raw(&mut self) {
        if let Some(id) = self.cur.take() {
            self.blocks[id].done = true;
        }
    }

    /// End the current block with a jump to `target`.
    fn jump(&mut self, target: BlockId) {
        let id = self.cur.expect("jump with no current block");
        self.blocks[id].code.push(Ins::Pc(target));
        self.blocks[id].succ.push(target);
        self.blocks[id].done = true;
        self.cur = None;
    }

    /// End the current block with `if <i32 on the stack> then a else b`.
    fn branch(&mut self, then_b: BlockId, else_b: BlockId) {
        let id = self.cur.expect("branch with no current block");
        let b = &mut self.blocks[id];
        b.code.push(Ins::Raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32))));
        b.code.push(Ins::Pc(then_b));
        b.code.push(Ins::Raw(I::Else));
        b.code.push(Ins::Pc(else_b));
        b.code.push(Ins::Raw(I::End));
        b.succ.push(then_b);
        b.succ.push(else_b);
        b.done = true;
        self.cur = None;
    }

    /// Start a block nobody jumps to — where the statements after a `GO TO`
    /// land. It is pruned if nothing ends up in it.
    fn dead_block(&mut self) {
        let b = self.new_block("unreachable");
        self.begin(b);
    }

    fn line(&mut self, line: u32) {
        self.i32c(line as i32);
        self.emit(Ins::GlobalSet(G::Line));
    }

    fn base(&mut self) {
        self.emit(Ins::GlobalGet(G::Base));
    }

    fn call_rt(&mut self, name: &'static str) {
        self.emit(Ins::CallRt(name));
    }

    // ── paragraphs ────────────────────────────────────────────────────────

    fn flatten(&mut self) -> GenResult<()> {
        let prog = self.prog;
        match &prog.procedure.body {
            ProcedureBody::Paragraphs(ps) => {
                for p in ps {
                    self.paras.push(Para { name: up(&p.name), section: None, stmts: &p.stmts, line: p.span.line });
                }
            }
            ProcedureBody::Sections(ss) => {
                for s in ss {
                    let first = self.paras.len();
                    for p in &s.paragraphs {
                        self.paras.push(Para {
                            name: up(&p.name),
                            section: Some(up(&s.name)),
                            stmts: &p.stmts,
                            line: p.span.line,
                        });
                    }
                    if self.paras.len() > first {
                        self.sections.insert(up(&s.name), (first, self.paras.len() - 1));
                    } else {
                        // An empty section: a header with nothing in it. Give
                        // it one empty paragraph so it can be PERFORMed.
                        self.paras.push(Para { name: String::new(), section: Some(up(&s.name)), stmts: &[], line: s.span.line });
                        self.sections.insert(up(&s.name), (first, first));
                    }
                }
            }
        }
        for (i, p) in self.paras.iter().enumerate() {
            if !p.name.is_empty() {
                self.para_by_name.entry(p.name.clone()).or_default().push(i);
            }
        }
        Ok(())
    }

    /// Which paragraphs end some `PERFORM` (they need the return check).
    fn prescan(&mut self) -> GenResult<()> {
        let mut ends: Vec<usize> = Vec::new();
        let mut err: Option<GenError> = None;
        let paras: Vec<&'a [Stmt]> = self.paras.iter().map(|p| p.stmts).collect();
        for stmts in paras {
            for s in stmts {
                s.walk(&mut |s| {
                    if let Stmt::Perform { target, span } = s {
                        match self.perform_range(target, span.line) {
                            Ok(Some((_, to))) => ends.push(to),
                            Ok(None) => {}
                            Err(e) => {
                                if err.is_none() {
                                    err = Some(e)
                                }
                            }
                        }
                    }
                    // SORT/MERGE procedures are performed too.
                    if let Stmt::Sort { input_proc, output_proc, .. } = s {
                        for p in [input_proc, output_proc].into_iter().flatten() {
                            if let Ok(Some((_, to))) = self.name_range(&p.0, p.1.as_deref(), 0) {
                                ends.push(to);
                            }
                        }
                    }
                });
            }
        }
        if let Some(e) = err {
            return Err(e);
        }
        self.perform_ends = ends.into_iter().collect();
        Ok(())
    }

    /// The paragraphs `[from, to]` a name (paragraph or section), optionally
    /// followed by `THRU name`, designates.
    fn name_range(&self, from: &str, thru: Option<&str>, line: u32) -> GenResult<Option<(usize, usize)>> {
        let f = up(from);
        let start = if let Some(&(a, _)) = self.sections.get(&f) {
            a
        } else if let Some(v) = self.para_by_name.get(&f) {
            v[0]
        } else {
            return Err(GenError::new(line, format!("paragraph or section {from} is not defined")));
        };
        let end_of = |me: &Self, n: &str| -> GenResult<usize> {
            let n = up(n);
            if let Some(&(_, b)) = me.sections.get(&n) {
                Ok(b)
            } else if let Some(v) = me.para_by_name.get(&n) {
                Ok(v[0])
            } else {
                Err(GenError::new(line, format!("paragraph or section {n} is not defined")))
            }
        };
        let end = match thru {
            Some(t) => end_of(self, t)?,
            None => {
                if let Some(&(_, b)) = self.sections.get(&f) {
                    b
                } else {
                    start
                }
            }
        };
        Ok(Some((start, end.max(start))))
    }

    fn perform_range(&self, t: &PerformTarget, line: u32) -> GenResult<Option<(usize, usize)>> {
        match t {
            PerformTarget::Paragraph(n, sp) | PerformTarget::Section(n, sp) => self.name_range(n, None, sp.line.max(line)),
            PerformTarget::Thru { from, to, span } => self.name_range(from, Some(to), span.line.max(line)),
            PerformTarget::QualifiedParagraph { name, section, span } => {
                let n = up(name);
                let s = up(section);
                let hit = self
                    .para_by_name
                    .get(&n)
                    .and_then(|v| v.iter().copied().find(|&i| self.paras[i].section.as_deref() == Some(s.as_str())));
                match hit {
                    Some(i) => Ok(Some((i, i))),
                    None => Err(GenError::new(span.line, format!("paragraph {name} is not in section {section}"))),
                }
            }
            _ => Ok(None),
        }
    }

    fn lower_para(&mut self, i: usize) -> GenResult<()> {
        self.cur_para = i;
        let stmts = self.paras[i].stmts;
        // Split the paragraph into sentences at the parser's markers; the block
        // after each marker is where `NEXT SENTENCE` lands.
        let mut sentence_starts: Vec<usize> = vec![0];
        for (k, s) in stmts.iter().enumerate() {
            if matches!(s, Stmt::SentenceEnd { .. }) {
                sentence_starts.push(k + 1);
            }
        }
        let after: Vec<BlockId> = sentence_starts.iter().skip(1).map(|_| self.new_block("sentence")).collect();
        self.begin(self.entries[i]);
        self.emit_line_of_para(i);
        let mut sent = 0usize;
        for (k, s) in stmts.iter().enumerate() {
            if matches!(s, Stmt::SentenceEnd { .. }) {
                // End of sentence `sent`: continue in its "after" block.
                let nb = after[sent];
                if self.cur.is_some() {
                    self.jump(nb);
                }
                self.begin(nb);
                sent += 1;
                continue;
            }
            // NEXT SENTENCE inside this sentence goes to the next marker's block,
            // or to the end of the paragraph when this is the last sentence.
            self.next_sentence = Some(after.get(sent).copied().unwrap_or(self.ends[i]));
            let _ = k;
            if self.cur.is_none() {
                self.dead_block();
            }
            self.stmt(s)?;
        }
        if self.cur.is_some() {
            let e = self.ends[i];
            self.jump(e);
        }
        self.next_sentence = None;
        self.lower_para_end(i)?;
        Ok(())
    }

    fn emit_line_of_para(&mut self, i: usize) {
        let l = self.paras[i].line;
        if l > 0 {
            self.line(l);
        }
    }

    /// The block at the end of paragraph `i`: return to the caller if a
    /// `PERFORM` ending here is waiting, otherwise fall into the next paragraph.
    fn lower_para_end(&mut self, i: usize) -> GenResult<()> {
        let end = self.ends[i];
        let next = if i + 1 < self.paras.len() { self.entries[i + 1] } else { self.program_end };
        self.begin(end);
        if self.perform_ends.contains(&i) {
            // psp > base && stack[psp-8].end == this block
            self.emit(Ins::GlobalGet(G::Psp));
            self.i32c(self.map.pstack_base as i32);
            self.raw(I::I32GtU);
            self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
            self.emit(Ins::GlobalGet(G::Psp));
            self.i32c(8);
            self.raw(I::I32Sub);
            self.raw(I::I32Load(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
            self.emit(Ins::Pc(end));
            self.raw(I::I32Eq);
            self.raw(I::Else);
            self.i32c(0);
            self.raw(I::End);
            self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
            // pop, return to the saved continuation
            self.emit(Ins::GlobalGet(G::Psp));
            self.i32c(8);
            self.raw(I::I32Sub);
            self.emit(Ins::GlobalSet(G::Psp));
            self.emit(Ins::GlobalGet(G::Psp));
            self.raw(I::I32Load(wasm_encoder::MemArg { offset: 4, align: 2, memory_index: 0 }));
            self.raw(I::Else);
            self.emit(Ins::Pc(next));
            self.raw(I::End);
            // Every continuation a frame can hold is a successor of this block.
            self.blocks[end].succ.push(next);
            self.blocks[end].done = true;
            self.cur = None;
        } else {
            self.jump(next);
        }
        Ok(())
    }

    // ── operands ──────────────────────────────────────────────────────────

    fn err<T>(&self, line: u32, msg: impl Into<String>) -> GenResult<T> {
        Err(GenError::new(line, msg))
    }

    fn quals_of(&self, mut e: &Expr) -> Vec<String> {
        let mut q = Vec::new();
        loop {
            match e {
                Expr::Qualified { name, of, .. } => {
                    q.push(name.clone());
                    e = of;
                }
                Expr::Identifier(n, _) => {
                    q.push(n.clone());
                    break;
                }
                _ => break,
            }
        }
        q
    }

    /// Resolve a (possibly qualified) data-name expression to its layout item.
    fn resolve_item(&self, e: &Expr) -> GenResult<usize> {
        let line = e.span().line;
        let (name, quals): (String, Vec<String>) = match e {
            Expr::Identifier(n, _) => (n.clone(), vec![]),
            Expr::Qualified { name, of, .. } => (name.clone(), self.quals_of(of)),
            _ => return self.err(line, "a data-name was expected"),
        };
        self.layout.lookup(&name, &quals).map_err(|m| GenError::new(line, m))
    }

    fn lit_number(&mut self, lit: &Literal, line: u32) -> GenResult<Option<u32>> {
        Ok(Some(match lit {
            Literal::Integer(n) | Literal::IntegerDigits(n, _) => self.pool.opd_lit_num(*n as i128, 0),
            Literal::Decimal(m, s) => self.pool.opd_lit_num(*m, *s),
            Literal::Float(f) => {
                let (m, s) = float_to_decimal(*f);
                self.pool.opd_lit_num(m, s)
            }
            Literal::String(s) => self.pool.opd_lit_alnum(s.as_bytes()),
            Literal::Figurative(fc) => match fc {
                FigurativeConstant::Zero => self.pool.opd_fig(FIG_ZERO),
                FigurativeConstant::Space => self.pool.opd_fig(FIG_SPACE),
                FigurativeConstant::HighValue => self.pool.opd_fig(FIG_HIGH),
                FigurativeConstant::LowValue => self.pool.opd_fig(FIG_LOW),
                FigurativeConstant::Quote => self.pool.opd_fig(FIG_QUOTE),
                FigurativeConstant::Null => self.pool.opd_fig(FIG_NULL),
                FigurativeConstant::All(inner) => {
                    let bytes: Vec<u8> = match inner.as_ref() {
                        Literal::String(s) => s.as_bytes().to_vec(),
                        other => match other.integer_digits() {
                            Some(d) => d.into_bytes(),
                            None => return self.err(line, "ALL needs a character or digit literal"),
                        },
                    };
                    if bytes.is_empty() {
                        return self.err(line, "ALL with an empty literal");
                    }
                    self.pool.opd_all(&bytes)
                }
            },
            Literal::National(_) | Literal::Utf8(_) => {
                return self.err(line, "national and UTF-8 literals are not supported in a WebAssembly module yet")
            }
        }))
    }

    /// The operand record of a simple reference — a data item (qualified,
    /// subscripted, reference-modified) or a literal. `Ok(None)` when the
    /// expression is a computation, which the caller pushes on the register
    /// stack instead.
    fn operand(&mut self, e: &Expr) -> GenResult<Option<u32>> {
        let line = e.span().line;
        match e {
            Expr::Literal(l, _) => self.lit_number(l, line),
            Expr::Identifier(..) | Expr::Qualified { .. } => {
                let idx = self.resolve_item(e)?;
                self.item_operand(idx, &[], None, line).map(Some)
            }
            Expr::Subscript { base, indices, .. } => {
                let idx = self.resolve_item(base)?;
                self.item_operand(idx, indices, None, line).map(Some)
            }
            Expr::RefMod { base, start, length, .. } => {
                let (idx, indices): (usize, &[Expr]) = match base.as_ref() {
                    Expr::Subscript { base: b, indices, .. } => (self.resolve_item(b)?, indices.as_slice()),
                    other => (self.resolve_item(other)?, &[]),
                };
                let s = match self.operand(start)? {
                    Some(o) => o,
                    None => return self.err(line, "a computed reference-modification start is not supported yet"),
                };
                let l = match length {
                    Some(l) => match self.operand(l)? {
                        Some(o) => o,
                        None => return self.err(line, "a computed reference-modification length is not supported yet"),
                    },
                    None => 0,
                };
                self.item_operand(idx, indices, Some((s, l)), line).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn item_operand(
        &mut self,
        idx: usize,
        indices: &[Expr],
        refmod: Option<(u32, u32)>,
        line: u32,
    ) -> GenResult<u32> {
        let item = self.layout.items[idx].clone();
        if item.level == 88 {
            return self.err(line, format!("{} is a condition-name, not a data item", item.name.clone().unwrap_or_default()));
        }
        if item.level == 66 {
            return self.err(line, "RENAMES items are not supported in a WebAssembly module yet");
        }
        let chain = self.layout.occurs_chain(idx);
        if indices.len() != chain.len() {
            return self.err(
                line,
                format!(
                    "{} needs {} subscript(s) but {} were written",
                    item.name.clone().unwrap_or_default(),
                    chain.len(),
                    indices.len()
                ),
            );
        }
        let mut subs = Vec::new();
        for (e, (stride, max, odo)) in indices.iter().zip(chain.into_iter()) {
            let (sub_expr, add) = split_relative(e);
            let opd = match self.operand(sub_expr)? {
                Some(o) => o,
                None => return self.err(line, "a computed subscript is not supported yet"),
            };
            let odo_opd = match odo {
                Some(name) => {
                    let di = self.layout.lookup(&name, &[]).map_err(|m| GenError::new(line, m))?;
                    self.item_operand(di, &[], None, line)?
                }
                None => 0,
            };
            subs.push(Sub { opd, stride, max, odo: odo_opd, add });
        }
        Ok(self.pool.opd_item(item.off, &item.fd, &subs, refmod))
    }

    /// Push the numeric value of an expression on the register stack.
    fn push_expr(&mut self, e: &Expr) -> GenResult<()> {
        let line = e.span().line;
        if let Some(o) = self.operand(e)? {
            self.base();
            self.i32c(o as i32);
            self.call_rt("rt_push");
            return Ok(());
        }
        match e {
            Expr::Arithmetic { op, lhs, rhs, .. } => {
                let code = match op {
                    ArithOp::Add => OP_ADD,
                    ArithOp::Sub => OP_SUB,
                    ArithOp::Mul => OP_MUL,
                    ArithOp::Div => OP_DIV,
                    ArithOp::Pow => OP_POW,
                    ArithOp::Concat => return self.err(line, "the concatenation operator is not supported yet"),
                };
                self.push_expr(lhs)?;
                self.push_expr(rhs)?;
                self.i32c(code as i32);
                self.call_rt("rt_op");
                Ok(())
            }
            Expr::Unary { op, operand, .. } => {
                self.push_expr(operand)?;
                if matches!(op, UnaryOp::Neg) {
                    self.i32c(OP_NEG as i32);
                    self.call_rt("rt_op");
                }
                Ok(())
            }
            Expr::FunctionCall { name, .. } => self.err(line, format!("FUNCTION {name} is not supported in a WebAssembly module yet")),
            Expr::Member { .. } => self.err(line, "control members are not supported in this build of the generator yet"),
            _ => self.err(line, "this expression is not supported in a WebAssembly module yet"),
        }
    }

    // ── conditions ────────────────────────────────────────────────────────

    /// Leave 1 or 0 on the stack.
    fn cond(&mut self, c: &Condition) -> GenResult<()> {
        let line = c.span().line;
        match c {
            Condition::Comparison { lhs, op, rhs, .. } => self.compare(lhs, cmp_code(*op), rhs, line),
            Condition::Not(inner, _) => {
                self.cond(inner)?;
                self.raw(I::I32Eqz);
                Ok(())
            }
            Condition::And(a, b, _) => {
                self.cond(a)?;
                self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
                self.cond(b)?;
                self.raw(I::Else);
                self.i32c(0);
                self.raw(I::End);
                Ok(())
            }
            Condition::Or(a, b, _) => {
                self.cond(a)?;
                self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
                self.i32c(1);
                self.raw(I::Else);
                self.cond(b)?;
                self.raw(I::End);
                Ok(())
            }
            Condition::ClassTest { expr, negated, class, .. } => {
                let o = self.operand_required(expr, line)?;
                self.base();
                self.i32c(o as i32);
                self.i32c(match class {
                    DataClass::Numeric => CLASS_NUMERIC,
                    DataClass::Alphabetic => CLASS_ALPHABETIC,
                    DataClass::AlphabeticLower => CLASS_LOWER,
                    DataClass::AlphabeticUpper => CLASS_UPPER,
                } as i32);
                self.call_rt("rt_class");
                if *negated {
                    self.raw(I::I32Eqz);
                }
                Ok(())
            }
            Condition::SignTest { expr, negated, sign, .. } => {
                let o = self.operand_required(expr, line)?;
                self.base();
                self.i32c(o as i32);
                self.i32c(match sign {
                    SignCond::Positive => SIGN_POSITIVE,
                    SignCond::Negative => SIGN_NEGATIVE,
                    SignCond::Zero => SIGN_ZERO,
                } as i32);
                self.call_rt("rt_sign");
                if *negated {
                    self.raw(I::I32Eqz);
                }
                Ok(())
            }
            Condition::ConditionName(name, _) => {
                let idx = self.layout.lookup(name, &[]).map_err(|m| GenError::new(line, m))?;
                self.cond_name(idx, &[], line)
            }
            Condition::ConditionRef(e, _) => {
                let (leaf, quals, indices): (String, Vec<String>, Vec<Expr>) = match e.as_ref() {
                    Expr::Subscript { base, indices, .. } => {
                        let (n, q) = split_name(base);
                        (n, q, indices.clone())
                    }
                    other => {
                        let (n, q) = split_name(other);
                        (n, q, vec![])
                    }
                };
                let idx = self.layout.lookup(&leaf, &quals).map_err(|m| GenError::new(line, m))?;
                if self.layout.items[idx].level == 88 {
                    self.cond_name(idx, &indices, line)
                } else {
                    // Not a condition-name: "holds something other than zero".
                    let o = self.operand_required(e, line)?;
                    let z = self.pool.opd_lit_num(0, 0);
                    self.base();
                    self.i32c(o as i32);
                    self.i32c(z as i32);
                    self.i32c(OP_NE as i32);
                    self.call_rt("rt_rel");
                    Ok(())
                }
            }
            Condition::NameOrAbbrev { subject, op, name, .. } => {
                if let Ok(idx) = self.layout.lookup(name, &[]) {
                    if self.layout.items[idx].level == 88 {
                        return self.cond_name(idx, &[], line);
                    }
                }
                let rhs = Expr::Identifier(name.clone(), c.span());
                self.compare(subject, cmp_code(*op), &rhs, line)
            }
            Condition::UserClassTest { .. } => self.err(line, "user-defined CLASS conditions are not supported yet"),
        }
    }

    fn operand_required(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        match self.operand(e)? {
            Some(o) => Ok(o),
            None => self.err(line, "a data item was expected here"),
        }
    }

    fn compare(&mut self, lhs: &Expr, op: u32, rhs: &Expr, line: u32) -> GenResult<()> {
        let (a, b) = (self.operand(lhs)?, self.operand(rhs)?);
        match (a, b) {
            (Some(a), Some(b)) => {
                self.base();
                self.i32c(a as i32);
                self.i32c(b as i32);
                self.i32c(op as i32);
                self.call_rt("rt_rel");
            }
            _ => {
                self.call_rt("rt_begin");
                self.push_expr(lhs)?;
                self.push_expr(rhs)?;
                self.i32c(op as i32);
                self.call_rt("rt_rel_reg");
            }
        }
        let _ = line;
        Ok(())
    }

    /// An `88`: the host item equals one of its values or lies in a range.
    fn cond_name(&mut self, cond_idx: usize, host_indices: &[Expr], line: u32) -> GenResult<()> {
        let item = self.layout.items[cond_idx].clone();
        let host = item.parent.expect("a condition-name has a host");
        let host_opd = self.item_operand(host, host_indices, None, line)?;
        let host_fd = self.layout.items[host].fd;
        let numeric = host_fd.is_numeric();
        let mut first = true;
        for v in &item.cond {
            match v {
                ConditionValue::Single(l) => {
                    let o = self.cond_literal(l, numeric, line)?;
                    self.base();
                    self.i32c(host_opd as i32);
                    self.i32c(o as i32);
                    self.i32c(OP_EQ as i32);
                    self.call_rt("rt_rel");
                }
                ConditionValue::Range(lo, hi) => {
                    let (lo, hi) = (self.cond_literal(lo, numeric, line)?, self.cond_literal(hi, numeric, line)?);
                    self.base();
                    self.i32c(host_opd as i32);
                    self.i32c(lo as i32);
                    self.i32c(OP_GE as i32);
                    self.call_rt("rt_rel");
                    self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
                    self.base();
                    self.i32c(host_opd as i32);
                    self.i32c(hi as i32);
                    self.i32c(OP_LE as i32);
                    self.call_rt("rt_rel");
                    self.raw(I::Else);
                    self.i32c(0);
                    self.raw(I::End);
                }
            }
            if !first {
                // acc OR this: (acc | this) -- both already evaluated
                self.raw(I::I32Or);
            }
            first = false;
        }
        if first {
            self.i32c(0);
        }
        Ok(())
    }

    fn cond_literal(&mut self, l: &Literal, host_numeric: bool, line: u32) -> GenResult<u32> {
        // A numeric host compares against a number; an alphanumeric host
        // against characters, so a numeric literal there is its digits.
        if !host_numeric {
            if let Some(d) = l.integer_digits() {
                return Ok(self.pool.opd_lit_alnum(d.as_bytes()));
            }
        }
        match self.lit_number(l, line)? {
            Some(o) => Ok(o),
            None => self.err(line, "a literal was expected"),
        }
    }

    // ── statements ────────────────────────────────────────────────────────

    fn stmts(&mut self, list: &[Stmt]) -> GenResult<()> {
        for s in list {
            if self.cur.is_none() {
                self.dead_block();
            }
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> GenResult<()> {
        let line = s.span().line;
        if line > 0 {
            self.line(line);
        }
        match s {
            Stmt::Move { from, to, .. } => self.move_(from, to, line),
            Stmt::Add { operands, to, giving, on_size_error, not_on_size_error, .. } => {
                self.add(operands, to, giving, on_size_error, not_on_size_error)
            }
            Stmt::Subtract { operands, from, giving, on_size_error, not_on_size_error, .. } => {
                self.subtract(operands, from, giving, on_size_error, not_on_size_error)
            }
            Stmt::Multiply { lhs, by, giving, rounded, on_size_error, not_on_size_error, .. } => {
                self.multiply(lhs, by, giving, *rounded, on_size_error, not_on_size_error, line)
            }
            Stmt::Divide { lhs, by, giving, remainder, rounded, on_size_error, not_on_size_error, .. } => {
                self.divide(lhs, by, giving, remainder.as_ref(), *rounded, on_size_error, not_on_size_error, line)
            }
            Stmt::Compute { targets, expr, on_size_error, not_on_size_error, .. } => {
                self.compute(targets, expr, on_size_error, not_on_size_error)
            }
            Stmt::If { condition, then_stmts, else_stmts, .. } => self.if_(condition, then_stmts, else_stmts),
            Stmt::Evaluate { subjects, whens, other_stmts, .. } => self.evaluate(subjects, whens, other_stmts, line),
            Stmt::Perform { target, .. } => self.perform(target, line),
            Stmt::GoTo { target, section, .. } => {
                let t = self.goto_target(target, section.as_deref(), line)?;
                self.jump(self.entries[t]);
                Ok(())
            }
            Stmt::GoToDepending { targets, depending, .. } => self.go_to_depending(targets, depending, line),
            Stmt::Continue { .. } | Stmt::SentenceEnd { .. } => Ok(()),
            Stmt::Exit { kind, .. } => self.exit(*kind, line),
            Stmt::NextSentence { .. } => {
                match self.next_sentence {
                    Some(b) => self.jump(b),
                    None => return self.err(line, "NEXT SENTENCE outside a sentence"),
                }
                Ok(())
            }
            Stmt::Display { operands, upon, no_advancing, .. } => self.display(operands, upon.as_deref(), *no_advancing, line),
            Stmt::Stop { run, .. } => {
                let _ = run;
                self.call_rt("rt_flush");
                self.i32c(ret::STOP);
                self.finish_raw();
                Ok(())
            }
            Stmt::GoBack { .. } => {
                self.call_rt("rt_flush");
                self.i32c(ret::DONE);
                self.finish_raw();
                Ok(())
            }
            other => self.err(
                line,
                format!("{} is not supported in a WebAssembly module", crate::support::stmt_name(other)),
            ),
        }
    }

    fn goto_target(&self, target: &str, section: Option<&str>, line: u32) -> GenResult<usize> {
        let t = up(target);
        if let Some(sec) = section {
            let s = up(sec);
            if let Some(v) = self.para_by_name.get(&t) {
                if let Some(&i) = v.iter().find(|&&i| self.paras[i].section.as_deref() == Some(s.as_str())) {
                    return Ok(i);
                }
            }
        }
        if let Some(v) = self.para_by_name.get(&t) {
            return Ok(v[0]);
        }
        if let Some(&(a, _)) = self.sections.get(&t) {
            return Ok(a);
        }
        self.err(line, format!("paragraph {target} is not defined"))
    }

    fn exit(&mut self, kind: ExitKind, line: u32) -> GenResult<()> {
        match kind {
            ExitKind::Point => Ok(()),
            ExitKind::Program => {
                self.call_rt("rt_flush");
                self.i32c(ret::DONE);
                self.finish_raw();
                Ok(())
            }
            ExitKind::Perform => match self.loops.last() {
                Some(l) => {
                    let x = l.exit;
                    self.jump(x);
                    Ok(())
                }
                None => self.err(line, "EXIT PERFORM outside an inline PERFORM"),
            },
            ExitKind::PerformCycle => match self.loops.last() {
                Some(l) => {
                    let c = l.cycle;
                    self.jump(c);
                    Ok(())
                }
                None => self.err(line, "EXIT PERFORM CYCLE outside an inline PERFORM"),
            },
            ExitKind::Paragraph => {
                let e = self.ends[self.cur_para];
                self.jump(e);
                Ok(())
            }
            ExitKind::Section => {
                let last = match self.paras[self.cur_para].section.as_ref().and_then(|s| self.sections.get(s)) {
                    Some(&(_, b)) => b,
                    None => self.cur_para,
                };
                let e = self.ends[last];
                self.jump(e);
                Ok(())
            }
        }
    }

    // ── MOVE ──────────────────────────────────────────────────────────────

    fn is_numeric_item(&self, e: &Expr) -> bool {
        let base = match e {
            Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => base.as_ref(),
            other => other,
        };
        match self.resolve_item(base) {
            Ok(i) => self.layout.items[i].fd.is_numeric(),
            Err(_) => false,
        }
    }

    fn move_(&mut self, from: &Expr, to: &[Expr], line: u32) -> GenResult<()> {
        // A MOVE whose receiver is an 88-level condition-name is `SET … TO TRUE`.
        for t in to {
            if let Expr::Identifier(..) | Expr::Qualified { .. } = t {
                if let Ok(i) = self.resolve_item(t) {
                    if self.layout.items[i].level == 88 {
                        return self.set_condition(i, from, line);
                    }
                }
            }
        }
        for t in to {
            if matches!(t, Expr::Member { .. }) {
                return self.err(line, "control members are not supported in this build of the generator yet");
            }
        }
        // The sender.
        let src = match from {
            // A numeric literal into an alphanumeric receiver moves the
            // characters as written, leading zeros and all.
            Expr::Literal(l @ (Literal::Integer(_) | Literal::IntegerDigits(..)), _)
                if to.iter().all(|t| !self.is_numeric_item(t)) =>
            {
                let d = l.integer_digits().unwrap();
                Some(self.pool.opd_lit_alnum(d.as_bytes()))
            }
            _ => self.operand(from)?,
        };
        match src {
            Some(src) => {
                if to.len() == 1 {
                    let dst = self.receiver(&to[0])?;
                    self.base();
                    self.i32c(src as i32);
                    self.i32c(dst as i32);
                    self.call_rt("rt_move");
                } else {
                    self.base();
                    self.i32c(src as i32);
                    self.call_rt("rt_move_begin");
                    for t in to {
                        let dst = self.receiver(t)?;
                        self.base();
                        self.i32c(dst as i32);
                        self.call_rt("rt_move_to");
                    }
                }
            }
            None => {
                // `MOVE <expression> TO …`: compute, then store into each receiver.
                self.call_rt("rt_begin");
                self.push_expr(from)?;
                for (k, t) in to.iter().enumerate() {
                    let dst = self.receiver(t)?;
                    if k + 1 < to.len() {
                        self.i32c(0);
                        self.call_rt("rt_pick");
                    }
                    self.base();
                    self.i32c(dst as i32);
                    self.i32c(0);
                    self.call_rt("rt_store");
                    self.raw(I::Drop);
                }
            }
        }
        Ok(())
    }

    fn receiver(&mut self, e: &Expr) -> GenResult<u32> {
        let line = e.span().line;
        match self.operand(e)? {
            Some(o) => Ok(o),
            None => self.err(line, "a data item was expected as the receiving field"),
        }
    }

    /// `SET condition-name TO TRUE | FALSE`.
    fn set_condition(&mut self, cond_idx: usize, from: &Expr, line: u32) -> GenResult<()> {
        let truthy = match from {
            Expr::Literal(Literal::Integer(n), _) => *n != 0,
            Expr::Literal(Literal::IntegerDigits(n, _), _) => *n != 0,
            Expr::Identifier(n, _) => !n.eq_ignore_ascii_case("FALSE"),
            _ => true,
        };
        let item = self.layout.items[cond_idx].clone();
        let host = item.parent.expect("host");
        let host_opd = self.item_operand(host, &[], None, line)?;
        let numeric = self.layout.items[host].fd.is_numeric();
        if truthy {
            // The first value that satisfies the condition.
            let first = item.cond.first();
            let lit = match first {
                Some(ConditionValue::Single(l)) | Some(ConditionValue::Range(l, _)) => l.clone(),
                None => return self.err(line, "a condition-name without values cannot be set"),
            };
            let src = self.cond_literal(&lit, numeric, line)?;
            self.base();
            self.i32c(src as i32);
            self.i32c(host_opd as i32);
            self.call_rt("rt_move");
            Ok(())
        } else {
            self.err(line, "SET condition-name TO FALSE is not supported in a WebAssembly module yet")
        }
    }

    // ── arithmetic ────────────────────────────────────────────────────────

    fn flags(rounded: bool, phrase: bool) -> i32 {
        (if rounded { ST_ROUNDED } else { 0 } | if phrase { ST_PHRASE } else { 0 }) as i32
    }

    /// After an arithmetic statement: `ON SIZE ERROR` / `NOT ON SIZE ERROR`.
    fn size_error_tail(&mut self, on: &[Stmt], not_on: &[Stmt]) -> GenResult<()> {
        if on.is_empty() && not_on.is_empty() {
            return Ok(());
        }
        self.call_rt("rt_size_err");
        let then_b = self.new_block("size-error");
        let else_b = self.new_block("no-size-error");
        let join = self.new_block("size-error-join");
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

    fn store_to(&mut self, target: &Expr, rounded: bool, phrase: bool, dup: bool) -> GenResult<()> {
        let dst = self.receiver(target)?;
        if dup {
            self.i32c(0);
            self.call_rt("rt_pick");
        }
        self.base();
        self.i32c(dst as i32);
        self.i32c(Self::flags(rounded, phrase));
        self.call_rt("rt_store");
        self.raw(I::Drop);
        Ok(())
    }

    fn add(
        &mut self,
        operands: &[Expr],
        to: &[(Expr, bool)],
        giving: &[(Expr, bool)],
        on: &[Stmt],
        not_on: &[Stmt],
    ) -> GenResult<()> {
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        self.push_sum(operands)?;
        if !giving.is_empty() {
            for (t, _) in to {
                self.push_expr(t)?;
                self.i32c(OP_ADD as i32);
                self.call_rt("rt_op");
            }
            for (k, (g, r)) in giving.iter().enumerate() {
                self.store_to(g, *r, phrase, k + 1 < giving.len())?;
            }
        } else {
            for (t, r) in to {
                // [sum] -> [sum, cur, sum] -> [sum, cur+sum] -> [sum]
                self.push_expr(t)?;
                self.i32c(1);
                self.call_rt("rt_pick");
                self.i32c(OP_ADD as i32);
                self.call_rt("rt_op");
                self.store_to(t, *r, phrase, false)?;
            }
            self.call_rt("rt_drop");
        }
        self.size_error_tail(on, not_on)
    }

    /// Push `0 + a + b + …`.
    fn push_sum(&mut self, operands: &[Expr]) -> GenResult<()> {
        let z = self.pool.opd_lit_num(0, 0);
        self.base();
        self.i32c(z as i32);
        self.call_rt("rt_push");
        for o in operands {
            self.push_expr(o)?;
            self.i32c(OP_ADD as i32);
            self.call_rt("rt_op");
        }
        Ok(())
    }

    fn subtract(
        &mut self,
        operands: &[Expr],
        from: &[(Expr, bool)],
        giving: &[(Expr, bool)],
        on: &[Stmt],
        not_on: &[Stmt],
    ) -> GenResult<()> {
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        if !giving.is_empty() {
            // base - sum(operands)
            match from.first() {
                Some((f, _)) => self.push_expr(f)?,
                None => {
                    let z = self.pool.opd_lit_num(0, 0);
                    self.base();
                    self.i32c(z as i32);
                    self.call_rt("rt_push");
                }
            }
            self.push_sum(operands)?;
            self.i32c(OP_SUB as i32);
            self.call_rt("rt_op");
            for (k, (g, r)) in giving.iter().enumerate() {
                self.store_to(g, *r, phrase, k + 1 < giving.len())?;
            }
        } else {
            self.push_sum(operands)?;
            for (f, r) in from {
                self.push_expr(f)?;
                self.i32c(1);
                self.call_rt("rt_pick");
                self.i32c(OP_SUB as i32);
                self.call_rt("rt_op");
                self.store_to(f, *r, phrase, false)?;
            }
            self.call_rt("rt_drop");
        }
        self.size_error_tail(on, not_on)
    }

    #[allow(clippy::too_many_arguments)]
    fn multiply(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: &[(Expr, bool)],
        rounded: bool,
        on: &[Stmt],
        not_on: &[Stmt],
        _line: u32,
    ) -> GenResult<()> {
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        self.push_expr(lhs)?;
        self.push_expr(by)?;
        self.i32c(OP_MUL as i32);
        self.call_rt("rt_op");
        if giving.is_empty() {
            self.store_to(by, rounded, phrase, false)?;
        } else {
            for (k, (g, r)) in giving.iter().enumerate() {
                self.store_to(g, *r, phrase, k + 1 < giving.len())?;
            }
        }
        self.size_error_tail(on, not_on)
    }

    #[allow(clippy::too_many_arguments)]
    fn divide(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: &[(Expr, bool)],
        remainder: Option<&Expr>,
        rounded: bool,
        on: &[Stmt],
        not_on: &[Stmt],
        _line: u32,
    ) -> GenResult<()> {
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        // [l r] -> [l r l r] -> [l r q]
        self.push_expr(lhs)?;
        self.push_expr(by)?;
        self.i32c(1);
        self.call_rt("rt_pick");
        self.i32c(1);
        self.call_rt("rt_pick");
        self.i32c(OP_DIV as i32);
        self.call_rt("rt_op");
        if giving.is_empty() {
            self.store_to_keep(lhs, rounded, phrase)?;
        } else {
            for (g, r) in giving {
                self.store_to_keep(g, *r, phrase)?;
            }
        }
        if let Some(rem) = remainder {
            // q' = q truncated to the first receiver's scale; r - l - q' * r
            if let Some((g, _)) = giving.first() {
                let o = self.receiver(g)?;
                self.base();
                self.i32c(o as i32);
                self.call_rt("rt_trunc");
            }
            self.i32c(OP_MUL as i32); // [l, r*q']
            self.call_rt("rt_op");
            self.i32c(OP_SUB as i32); // [l - r*q']
            self.call_rt("rt_op");
            let dst = self.receiver(rem)?;
            self.base();
            self.i32c(dst as i32);
            self.i32c(0);
            self.call_rt("rt_store");
            self.raw(I::Drop);
        } else {
            self.call_rt("rt_drop"); // q
            self.call_rt("rt_drop"); // r
            self.call_rt("rt_drop"); // l
        }
        self.size_error_tail(on, not_on)
    }

    /// Store a copy of the top register, keeping the register.
    fn store_to_keep(&mut self, target: &Expr, rounded: bool, phrase: bool) -> GenResult<()> {
        self.store_to(target, rounded, phrase, true)
    }

    fn compute(
        &mut self,
        targets: &[(Expr, bool)],
        expr: &Expr,
        on: &[Stmt],
        not_on: &[Stmt],
    ) -> GenResult<()> {
        let phrase = !on.is_empty() || !not_on.is_empty();
        self.call_rt("rt_begin");
        self.push_expr(expr)?;
        for (k, (t, r)) in targets.iter().enumerate() {
            self.store_to(t, *r, phrase, k + 1 < targets.len())?;
        }
        self.size_error_tail(on, not_on)
    }

    // ── IF / EVALUATE ─────────────────────────────────────────────────────

    fn if_(&mut self, c: &Condition, then_s: &[Stmt], else_s: &[Stmt]) -> GenResult<()> {
        self.cond(c)?;
        let then_b = self.new_block("then");
        let join = self.new_block("endif");
        let else_b = if else_s.is_empty() { join } else { self.new_block("else") };
        self.branch(then_b, else_b);
        self.begin(then_b);
        self.stmts(then_s)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        if !else_s.is_empty() {
            self.begin(else_b);
            self.stmts(else_s)?;
            if self.cur.is_some() {
                self.jump(join);
            }
        }
        self.begin(join);
        Ok(())
    }

    fn evaluate(
        &mut self,
        subjects: &[EvalSubject],
        whens: &[WhenClause],
        other: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let join = self.new_block("end-evaluate");
        for w in whens {
            let body = self.new_block("when");
            let next = self.new_block("next-when");
            // The WHEN matches when every subject's selection object does.
            let n = w.values.len().min(subjects.len());
            let mut first = true;
            for k in 0..n {
                self.when_match(&subjects[k], &w.values[k], line)?;
                if !first {
                    self.raw(I::I32And);
                }
                first = false;
            }
            if first {
                self.i32c(0);
            }
            self.branch(body, next);
            self.begin(body);
            self.stmts(&w.stmts)?;
            if self.cur.is_some() {
                self.jump(join);
            }
            self.begin(next);
        }
        self.stmts(other)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    fn when_match(&mut self, subject: &EvalSubject, v: &WhenValue, line: u32) -> GenResult<()> {
        match v {
            WhenValue::Any | WhenValue::Other => {
                self.i32c(1);
                Ok(())
            }
            WhenValue::Not(inner) => {
                self.when_match(subject, inner, line)?;
                self.raw(I::I32Eqz);
                Ok(())
            }
            WhenValue::Condition(c) => match subject {
                EvalSubject::True_ => self.cond(c),
                EvalSubject::False_ => {
                    self.cond(c)?;
                    self.raw(I::I32Eqz);
                    Ok(())
                }
                _ => self.err(line, "WHEN condition against a non-conditional EVALUATE subject is not supported yet"),
            },
            WhenValue::Literal(l) => {
                let e = Expr::Literal(l.clone(), cobolt_lexer::Span::new(0, 0, line, 0));
                self.subject_rel(subject, OP_EQ, &e, line)
            }
            WhenValue::Expr(e) => self.subject_rel(subject, OP_EQ, e, line),
            WhenValue::Range(lo, hi) => {
                let sp = cobolt_lexer::Span::new(0, 0, line, 0);
                let (lo, hi) = (Expr::Literal(lo.clone(), sp), Expr::Literal(hi.clone(), sp));
                self.range_match(subject, &lo, &hi, line)
            }
            WhenValue::ExprRange(lo, hi) => self.range_match(subject, lo, hi, line),
        }
    }

    fn range_match(&mut self, subject: &EvalSubject, lo: &Expr, hi: &Expr, line: u32) -> GenResult<()> {
        self.subject_rel(subject, OP_GE, lo, line)?;
        self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
        self.subject_rel(subject, OP_LE, hi, line)?;
        self.raw(I::Else);
        self.i32c(0);
        self.raw(I::End);
        Ok(())
    }

    fn subject_rel(&mut self, subject: &EvalSubject, op: u32, value: &Expr, line: u32) -> GenResult<()> {
        match subject {
            EvalSubject::Expr(e) => self.compare(e, op, value, line),
            EvalSubject::True_ | EvalSubject::False_ | EvalSubject::Cond(_) => {
                self.err(line, "a value WHEN against a conditional EVALUATE subject is not supported yet")
            }
        }
    }

    // ── PERFORM ───────────────────────────────────────────────────────────

    /// Push a frame `(end, return)` and enter the paragraph range, resuming at
    /// `ret_to`.
    fn call_range(&mut self, from: usize, to: usize, ret_to: BlockId) {
        // Overflow check: psp >= end -> run-time error.
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(self.map.pstack_end as i32);
        self.raw(I::I32GeU);
        self.raw(I::If(wasm_encoder::BlockType::Empty));
        self.i32c(cobolt_wasm_rt::state::err::PERFORM_DEPTH as i32);
        self.emit(Ins::CallGlue(Glue::Abort));
        self.raw(I::End);
        // stack[psp] = (end block of `to`, return block)
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(self.ends[to]));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(ret_to));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 4, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(8);
        self.raw(I::I32Add);
        self.emit(Ins::GlobalSet(G::Psp));
        self.jump(self.entries[from]);
    }

    fn perform(&mut self, t: &PerformTarget, line: u32) -> GenResult<()> {
        match t {
            PerformTarget::Paragraph(..)
            | PerformTarget::Section(..)
            | PerformTarget::Thru { .. }
            | PerformTarget::QualifiedParagraph { .. } => {
                let (from, to) = self.perform_range(t, line)?.expect("a paragraph target");
                let k = self.new_block("after-perform");
                self.call_range(from, to, k);
                self.begin(k);
                Ok(())
            }
            PerformTarget::Inline { stmts } => {
                // Plain inline PERFORM: the statements once, with EXIT PERFORM.
                let join = self.new_block("end-perform");
                self.loops.push(LoopCtx { exit: join, cycle: join });
                self.stmts(stmts)?;
                self.loops.pop();
                if self.cur.is_some() {
                    self.jump(join);
                }
                self.begin(join);
                Ok(())
            }
            PerformTarget::Times { count, stmts } => self.perform_times(count, stmts, line),
            PerformTarget::Until { condition, test_before, stmts } => self.perform_until(condition, *test_before, stmts),
            PerformTarget::Varying { var, from, by, until, stmts, after, test_before } => {
                self.perform_varying(var, from, by, until, stmts, after, *test_before, line)
            }
        }
    }

    fn perform_times(&mut self, count: &Expr, stmts: &[Stmt], line: u32) -> GenResult<()> {
        // The count is read once into a hidden eight-byte slot of the region.
        let slot = self.layout.alloc_hidden(8);
        let slot_addr_of = |me: &mut Self| {
            me.base();
        };
        // slot = count (as an integer)
        slot_addr_of(self);
        match self.operand(count)? {
            Some(o) => {
                self.base();
                self.i32c(o as i32);
                self.call_rt("rt_int");
            }
            None => {
                self.call_rt("rt_begin");
                self.push_expr(count)?;
                self.call_rt("rt_reg_int");
            }
        }
        self.raw(I::I64Store(wasm_encoder::MemArg { offset: slot as u64, align: 3, memory_index: 0 }));
        let head = self.new_block("times");
        let body = self.new_block("times-body");
        let exit = self.new_block("end-times");
        self.jump(head);
        self.begin(head);
        // if slot <= 0 exit else body
        self.base();
        self.raw(I::I64Load(wasm_encoder::MemArg { offset: slot as u64, align: 3, memory_index: 0 }));
        self.raw(I::I64Const(0));
        self.raw(I::I64GtS);
        self.branch(body, exit);
        self.begin(body);
        let step = self.new_block("times-step");
        self.loops.push(LoopCtx { exit, cycle: step });
        self.stmts(stmts)?;
        self.loops.pop();
        if self.cur.is_some() {
            self.jump(step);
        }
        self.begin(step);
        // slot -= 1
        self.base();
        self.base();
        self.raw(I::I64Load(wasm_encoder::MemArg { offset: slot as u64, align: 3, memory_index: 0 }));
        self.raw(I::I64Const(1));
        self.raw(I::I64Sub);
        self.raw(I::I64Store(wasm_encoder::MemArg { offset: slot as u64, align: 3, memory_index: 0 }));
        self.jump(head);
        self.begin(exit);
        let _ = line;
        Ok(())
    }

    fn perform_until(&mut self, cond: &Condition, test_before: bool, stmts: &[Stmt]) -> GenResult<()> {
        let head = self.new_block("until");
        let body = self.new_block("until-body");
        let exit = self.new_block("end-until");
        if test_before {
            self.jump(head);
            self.begin(head);
            self.cond(cond)?;
            self.branch(exit, body);
            self.begin(body);
            self.loops.push(LoopCtx { exit, cycle: head });
            self.stmts(stmts)?;
            self.loops.pop();
            if self.cur.is_some() {
                self.jump(head);
            }
        } else {
            // TEST AFTER: body first, then the test.
            self.jump(body);
            self.begin(head);
            self.cond(cond)?;
            self.branch(exit, body);
            self.begin(body);
            self.loops.push(LoopCtx { exit, cycle: head });
            self.stmts(stmts)?;
            self.loops.pop();
            if self.cur.is_some() {
                self.jump(head);
            }
        }
        self.begin(exit);
        Ok(())
    }

    /// `MOVE from TO var`.
    fn assign(&mut self, var: &Expr, from: &Expr) -> GenResult<()> {
        let line = var.span().line;
        match self.operand(from)? {
            Some(src) => {
                let dst = self.receiver(var)?;
                self.base();
                self.i32c(src as i32);
                self.i32c(dst as i32);
                self.call_rt("rt_move");
            }
            None => {
                self.call_rt("rt_begin");
                self.push_expr(from)?;
                let dst = self.receiver(var)?;
                self.base();
                self.i32c(dst as i32);
                self.i32c(0);
                self.call_rt("rt_store");
                self.raw(I::Drop);
            }
        }
        let _ = line;
        Ok(())
    }

    /// `ADD by TO var`.
    fn augment(&mut self, var: &Expr, by: &Expr) -> GenResult<()> {
        self.call_rt("rt_begin");
        self.push_expr(var)?;
        self.push_expr(by)?;
        self.i32c(OP_ADD as i32);
        self.call_rt("rt_op");
        let dst = self.receiver(var)?;
        self.base();
        self.i32c(dst as i32);
        self.i32c(0);
        self.call_rt("rt_store");
        self.raw(I::Drop);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn perform_varying(
        &mut self,
        var: &Expr,
        from: &Expr,
        by: &Expr,
        until: &Condition,
        stmts: &[Stmt],
        after: &[VaryingAfter],
        test_before: bool,
        line: u32,
    ) -> GenResult<()> {
        // The levels, outermost first.
        struct Level<'x> {
            var: &'x Expr,
            from: &'x Expr,
            by: &'x Expr,
            until: &'x Condition,
        }
        let mut levels = vec![Level { var, from, by, until }];
        for a in after {
            levels.push(Level { var: &a.var, from: &a.from, by: &a.by, until: &a.until });
        }
        let n = levels.len();
        let exit = self.new_block("end-varying");
        let body_b = self.new_block("varying-body");
        if test_before {
            // v1=f1; T1: if c1 exit; v2=f2; T2: if c2 {v1+=b1; goto T1}; body; v2+=b2; goto T2
            let tests: Vec<BlockId> = (0..n).map(|_| self.new_block("varying-test")).collect();
            self.assign(levels[0].var, levels[0].from)?;
            self.jump(tests[0]);
            for k in 0..n {
                self.begin(tests[k]);
                // Initialise the inner variable when entering its test from the
                // outer level: done at the end of the outer test below.
                self.cond(levels[k].until)?;
                let on_true = if k == 0 {
                    exit
                } else {
                    // advance the previous level and retest it
                    let adv = self.new_block("varying-advance");
                    adv
                };
                let on_false = if k + 1 < n { self.new_block("varying-init") } else { body_b };
                self.branch(on_true, on_false);
                if k > 0 {
                    self.begin(on_true);
                    self.augment(levels[k - 1].var, levels[k - 1].by)?;
                    self.jump(tests[k - 1]);
                }
                if k + 1 < n {
                    self.begin(on_false);
                    self.assign(levels[k + 1].var, levels[k + 1].from)?;
                    self.jump(tests[k + 1]);
                }
            }
            self.begin(body_b);
            let cycle = self.new_block("varying-step");
            self.loops.push(LoopCtx { exit, cycle });
            self.stmts(stmts)?;
            self.loops.pop();
            if self.cur.is_some() {
                self.jump(cycle);
            }
            self.begin(cycle);
            self.augment(levels[n - 1].var, levels[n - 1].by)?;
            self.jump(tests[n - 1]);
        } else {
            // TEST AFTER: all variables initialised; body; test innermost first.
            for l in &levels {
                self.assign(l.var, l.from)?;
            }
            self.jump(body_b);
            self.begin(body_b);
            let cycle = self.new_block("varying-after");
            self.loops.push(LoopCtx { exit, cycle });
            self.stmts(stmts)?;
            self.loops.pop();
            if self.cur.is_some() {
                self.jump(cycle);
            }
            self.begin(cycle);
            // Level k: if c_k { if k == 0 exit else { v_{k-1} += b_{k-1}; test level k-1; if false: v_k = f_k; goto body } } else { v_k += b_k; goto body }
            let tests: Vec<BlockId> = (0..n).map(|_| self.new_block("varying-test")).collect();
            self.jump(tests[n - 1]);
            for k in (0..n).rev() {
                self.begin(tests[k]);
                self.cond(levels[k].until)?;
                let satisfied = self.new_block("varying-satisfied");
                let step = self.new_block("varying-step");
                self.branch(satisfied, step);
                self.begin(step);
                self.augment(levels[k].var, levels[k].by)?;
                self.jump(body_b);
                self.begin(satisfied);
                if k == 0 {
                    self.jump(exit);
                } else {
                    self.augment(levels[k - 1].var, levels[k - 1].by)?;
                    // retest the outer level; when it is not satisfied the inner
                    // variable restarts: handled by testing level k-1 and, on
                    // false, re-initialising level k.
                    self.cond(levels[k - 1].until)?;
                    let outer_done = self.new_block("varying-outer-done");
                    let restart = self.new_block("varying-restart");
                    self.branch(outer_done, restart);
                    self.begin(restart);
                    self.assign(levels[k].var, levels[k].from)?;
                    self.jump(body_b);
                    self.begin(outer_done);
                    // An outer level that is satisfied ends the loop only when
                    // it is the outermost; otherwise it carries one level out.
                    if k - 1 == 0 {
                        self.jump(exit);
                    } else {
                        self.augment(levels[k - 2].var, levels[k - 2].by)?;
                        self.cond(levels[k - 2].until)?;
                        let done2 = self.new_block("varying-outer-done");
                        let restart2 = self.new_block("varying-restart");
                        self.branch(done2, restart2);
                        self.begin(restart2);
                        for j in (k - 1)..=k {
                            self.assign(levels[j].var, levels[j].from)?;
                        }
                        self.jump(body_b);
                        self.begin(done2);
                        if k - 2 == 0 {
                            self.jump(exit);
                        } else {
                            return self.err(line, "PERFORM VARYING with more than three AFTER levels in TEST AFTER is not supported");
                        }
                    }
                }
            }
        }
        self.begin(exit);
        Ok(())
    }

    fn go_to_depending(&mut self, targets: &[String], depending: &Expr, line: u32) -> GenResult<()> {
        // idx = (integer value of the item) - 1; a value outside 1..=n falls through.
        let after = self.new_block("go-to-fallthrough");
        let tgts: Vec<BlockId> = targets
            .iter()
            .map(|t| self.goto_target(t, None, line).map(|p| self.entries[p]))
            .collect::<GenResult<Vec<_>>>()?;
        match self.operand(depending)? {
            Some(o) => {
                self.base();
                self.i32c(o as i32);
                self.i32c(tgts.len() as i32);
                self.call_rt("rt_goto_index");
            }
            None => {
                self.call_rt("rt_begin");
                self.push_expr(depending)?;
                self.i32c(tgts.len() as i32);
                self.call_rt("rt_reg_goto_index");
            }
        }
        // The block's local 0 holds the index.
        self.raw(I::LocalSet(0));
        self.raw(I::LocalGet(0));
        self.i32c(0);
        self.raw(I::I32LtS);
        self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
        self.emit(Ins::Pc(after));
        self.raw(I::Else);
        for (k, t) in tgts.iter().enumerate() {
            self.raw(I::LocalGet(0));
            self.i32c(k as i32);
            self.raw(I::I32Eq);
            self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
            self.emit(Ins::Pc(*t));
            self.raw(I::Else);
        }
        self.emit(Ins::Pc(after));
        for _ in &tgts {
            self.raw(I::End);
        }
        self.raw(I::End);
        let cur = self.cur.take().unwrap();
        self.blocks[cur].succ.push(after);
        self.blocks[cur].succ.extend(tgts.iter().copied());
        self.blocks[cur].done = true;
        self.begin(after);
        Ok(())
    }

    // ── DISPLAY ───────────────────────────────────────────────────────────

    fn display(&mut self, operands: &[Expr], upon: Option<&str>, no_adv: bool, line: u32) -> GenResult<()> {
        if let Some(u) = upon {
            let u = u.to_ascii_uppercase();
            if u == "ARGUMENT-NUMBER" || u == "ENVIRONMENT-NAME" {
                return self.err(line, format!("DISPLAY UPON {u} is not supported in a WebAssembly module"));
            }
        }
        for o in operands {
            match self.operand(o)? {
                Some(opd) => {
                    self.base();
                    self.i32c(opd as i32);
                    self.call_rt("rt_display");
                }
                None => {
                    self.call_rt("rt_begin");
                    self.push_expr(o)?;
                    self.call_rt("rt_display_reg");
                }
            }
        }
        self.i32c(no_adv as i32);
        self.call_rt("rt_display_end");
        Ok(())
    }
}

fn cmp_code(op: CmpOp) -> u32 {
    match op {
        CmpOp::Eq => OP_EQ,
        CmpOp::Ne => OP_NE,
        CmpOp::Lt => OP_LT,
        CmpOp::Le => OP_LE,
        CmpOp::Gt => OP_GT,
        CmpOp::Ge => OP_GE,
    }
}

/// `I + 1` / `J - 2` as `(I, +1)` / `(J, -2)`; anything else as `(e, 0)`.
fn split_relative(e: &Expr) -> (&Expr, i32) {
    if let Expr::Arithmetic { op, lhs, rhs, .. } = e {
        if let Expr::Literal(Literal::Integer(n), _) = rhs.as_ref() {
            match op {
                ArithOp::Add => return (lhs.as_ref(), *n as i32),
                ArithOp::Sub => return (lhs.as_ref(), -(*n as i32)),
                _ => {}
            }
        }
    }
    (e, 0)
}

fn split_name(e: &Expr) -> (String, Vec<String>) {
    match e {
        Expr::Identifier(n, _) => (n.clone(), vec![]),
        Expr::Qualified { name, of, .. } => {
            let mut q = Vec::new();
            let mut cur = of.as_ref();
            loop {
                match cur {
                    Expr::Qualified { name, of, .. } => {
                        q.push(name.clone());
                        cur = of;
                    }
                    Expr::Identifier(n, _) => {
                        q.push(n.clone());
                        break;
                    }
                    _ => break,
                }
            }
            (name.clone(), q)
        }
        _ => (String::new(), vec![]),
    }
}

/// A float literal as the decimal it spells (shortest round-trip form).
pub fn float_to_decimal_pub(f: f64) -> (i128, u8) {
    float_to_decimal(f)
}

fn float_to_decimal(f: f64) -> (i128, u8) {
    let s = format!("{f}");
    let neg = s.starts_with('-');
    let body = s.trim_start_matches('-');
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    let digits = format!("{int}{frac}");
    let m: i128 = digits.parse().unwrap_or(0);
    (if neg { -m } else { m }, frac.len().min(31) as u8)
}
