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

/// Where the `PERFORM` stack lies inside the region: each instance of a form
/// has its own, so a form that is waiting for an event keeps its frames.
#[derive(Debug, Clone, Copy)]
pub struct MemMap {
    pub stack_off: u32,
    pub stack_size: u32,
}

/// Bytes of `PERFORM` stack per region: 512 frames of 8 bytes.
pub const STACK_BYTES: u32 = 4096;

/// One paragraph, flattened out of its section.
pub(crate) struct Para<'a> {
    name: String,
    section: Option<String>,
    stmts: &'a [Stmt],
    line: u32,
}

/// An inline loop being lowered: where `EXIT PERFORM` and `EXIT PERFORM CYCLE`
/// go.
pub(crate) struct LoopCtx {
    exit: BlockId,
    cycle: BlockId,
}

/// A file of the program, as the conformance routines see it.
#[derive(Clone, Copy)]
pub(crate) struct FileInfo {
    /// Address of the static file descriptor.
    desc: u32,
    /// Region offset of the handle slot; the open mode is kept just after it.
    handle_off: u32,
    /// Layout item of the file's first record area.
    rec_item: Option<usize>,
}

/// The names the host commands refer to by number (ABI v1 §5): properties,
/// methods and built-ins, shared by every form of the module and written to the
/// manifest.
#[derive(Debug, Default, Clone)]
pub struct Names {
    pub list: Vec<String>,
    map: HashMap<String, u32>,
}

impl Names {
    /// The number of `name` (case kept as written), interning it on first use.
    pub fn id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.map.get(name) {
            return i;
        }
        let i = self.list.len() as u32;
        self.list.push(name.to_string());
        self.map.insert(name.to_string(), i);
        i
    }
}

/// What the generator knows of the form a program belongs to: its controls, by
/// name, numbered as the manifest numbers them.
#[derive(Debug, Clone, Default)]
pub struct FormCtx {
    /// Upper-case control name -> control id (from 3).
    pub controls: HashMap<String, u32>,
}

pub struct Lowered {
    pub entry: BlockId,
    pub layout: Layout,
    /// Number of program units (the main program, then the nested and following
    /// programs), for the per-unit initialisation the image builds.
    pub units: usize,
    /// Where this region's `PERFORM` stack starts, from the region base.
    pub stack_off: u32,
    /// Region slot where `WAIT-EVENT` parks the event items' operands (`u32::MAX`
    /// when the form never waits).
    pub park_slot: u32,
    /// The region offsets of the items `COBOL-EVENT-ID` / `COBOL-CONTROL-ID`.
    pub event_items: (u32, u32),
}

/// A parameter of a called program: the LINKAGE record it names.
#[derive(Debug, Clone)]
pub(crate) struct ParamInfo {
    /// Region offset of the slot a `CALL` fills with the argument's address.
    pub slot: u32,
    /// The record's layout item.
    pub item: usize,
    pub size: u32,
}

/// What a `CALL` needs to know about a program unit.
#[derive(Debug, Clone)]
pub(crate) struct UnitInfo {
    pub name: String,
    /// Where a `CALL` jumps: the first paragraph, or the exit when there is none.
    pub entry: BlockId,
    /// The exit block: `EXIT PROGRAM`, `GOBACK` and falling off the last
    /// paragraph end up here and return to the caller.
    pub exit: BlockId,
    pub params: Vec<ParamInfo>,
    /// The `RETURNING` item's layout index.
    pub returning: Option<usize>,
    /// Units whose `GLOBAL` items this one sees, itself first.
    pub scope: Vec<u32>,
}

/// A `USE AFTER ERROR` declarative, as an I/O verb in any unit may reach it.
#[derive(Debug, Clone)]
pub(crate) struct Handler {
    /// First block of the handler's paragraphs, and the end block of its last.
    pub entry: BlockId,
    pub end: BlockId,
    pub files: Vec<String>,
    pub modes: Vec<cobolt_ast::program::UseMode>,
    pub catch_all: bool,
    pub global: bool,
}

/// The per-unit part of [`Lower`]'s state, swapped in and out as the units are
/// lowered one after another.
#[derive(Default)]
pub(crate) struct UnitState<'a> {
    paras: Vec<Para<'a>>,
    para_by_name: HashMap<String, Vec<usize>>,
    sections: HashMap<String, (usize, usize)>,
    perform_ends: HashSet<usize>,
    entries: Vec<BlockId>,
    ends: Vec<BlockId>,
    program_end: BlockId,
    files: HashMap<String, FileInfo>,
    alter_slots: HashMap<usize, u32>,
    prologue: BlockId,
    /// Paragraphs before the declaratives' (which are entered only by a USE).
    body_len: usize,
    /// The first and last paragraph of each declarative section.
    decl_ranges: Vec<(usize, usize)>,
}

pub struct Lower<'a, 'b> {
    pub(crate) prog: &'a Program,
    pub layout: Layout,
    pub(crate) pool: &'b mut Pool,
    pub(crate) blocks: &'b mut Vec<Block>,
    pub(crate) map: MemMap,
    pub(crate) cur: Option<BlockId>,
    pub(crate) paras: Vec<Para<'a>>,
    pub(crate) para_by_name: HashMap<String, Vec<usize>>,
    pub(crate) sections: HashMap<String, (usize, usize)>,
    pub(crate) perform_ends: HashSet<usize>,
    pub(crate) entries: Vec<BlockId>,
    pub(crate) ends: Vec<BlockId>,
    pub(crate) loops: Vec<LoopCtx>,
    pub(crate) cur_para: usize,
    pub(crate) next_sentence: Option<BlockId>,
    /// Where control goes when the last paragraph falls off its end.
    pub(crate) program_end: BlockId,
    /// Constructs that could not be compiled; reported together at the end.
    pub(crate) diags: Vec<GenError>,
    /// The NIST gate build: file verbs are compiled.
    pub(crate) conformance: bool,
    pub(crate) tolerant: bool,
    pub(crate) files: HashMap<String, FileInfo>,
    /// Record item -> the file whose area it is.
    pub(crate) record_file: HashMap<usize, String>,
    /// Next free statement temporary (reset for every statement).
    pub(crate) temp_next: u32,
    /// Next free character temporary (reset for every statement).
    pub(crate) ctemp_next: u32,
    /// The `WHEN-COMPILED` text of this build (21 characters).
    pub(crate) when_compiled: String,
    /// Paragraphs that are `ALTER` targets -> the region offset of their slot.
    pub(crate) alter_slots: HashMap<usize, u32>,
    /// Every program unit of the module, indexed by unit number (0 = main).
    pub(crate) units: Vec<UnitInfo>,
    /// Unit names (upper case) -> unit number.
    pub(crate) unit_by_name: HashMap<String, usize>,
    /// The unit being lowered.
    pub(crate) cur_unit: usize,
    /// The prologue block of the unit being lowered.
    pub(crate) prologue: BlockId,
    /// Per unit: its files, and whether each is `GLOBAL` (visible to the
    /// units it contains).
    pub(crate) file_tables: Vec<HashMap<String, (FileInfo, bool)>>,
    /// Names of the current unit's `GLOBAL` files.
    pub(crate) global_files: HashSet<String>,
    /// Paragraphs of the unit that are procedure-division body, not declaratives.
    pub(crate) body_len: usize,
    /// First and last paragraph of each declarative section of the unit.
    pub(crate) decl_ranges: Vec<(usize, usize)>,
    /// Per unit: its USE handlers.
    pub(crate) handlers: Vec<Vec<Handler>>,
    /// Region slot: non-zero while a USE procedure runs (they do not nest).
    pub(crate) use_guard: Option<u32>,
    /// Region slot holding the status of the file verb just performed.
    pub(crate) status_hold: Option<u32>,
    /// The names the host commands carry by number.
    pub(crate) names: &'b mut Names,
    /// The form this program is, when it is one.
    pub(crate) form: Option<FormCtx>,
    /// Region slot where `WAIT-EVENT` leaves the operands the event is delivered to.
    pub(crate) park_slot: Option<u32>,
    /// Units in the forms lowered before this one: `Ins::InitUnit` counts
    /// units across the whole module.
    pub(crate) unit_base: usize,
    /// `EXTERNAL` files: name -> the region slot holding the one open handle.
    pub(crate) external_handles: HashMap<String, u32>,
}

/// The program units of a source: `prog`, then everything nested in it or
/// written after it, depth first, each with the unit that contains it.
fn collect_units<'a>(prog: &'a Program, parent: Option<usize>, out: &mut Vec<(&'a Program, Option<usize>)>) {
    let me = out.len();
    out.push((prog, parent));
    for n in &prog.nested_programs {
        collect_units(n, Some(me), out);
    }
}

fn up_name(s: &str) -> String {
    s.to_ascii_uppercase()
}

fn up(s: &str) -> String {
    s.to_ascii_uppercase()
}

impl<'a, 'b> Lower<'a, 'b> {
    pub fn lower(
        prog: &'a Program,
        pool: &'b mut Pool,
        blocks: &'b mut Vec<Block>,
        names: &'b mut Names,
        form: Option<FormCtx>,
        unit_base: usize,
        conformance: bool,
        tolerant: bool,
        when_compiled: String,
    ) -> GenResult<Lowered> {
        // The units: the main program, then every program nested in it or
        // written after it, depth first. A name is visible to its own unit and
        // (when GLOBAL) to the units it contains.
        let mut order: Vec<(&'a Program, Option<usize>)> = Vec::new();
        collect_units(prog, None, &mut order);
        let scope_of = |u: usize| -> Vec<u32> {
            let mut v = vec![u as u32];
            let mut cur = order[u].1;
            while let Some(p) = cur {
                v.push(p as u32);
                cur = order[p].1;
            }
            v
        };

        // Storage first, for every unit, so a CALL can name any callee's records.
        let mut layout = Layout::new();
        // The region's own PERFORM stack comes first.
        let stack_off = layout.alloc_hidden(STACK_BYTES);
        let map = MemMap { stack_off, stack_size: STACK_BYTES };
        for (u, (p, _)) in order.iter().enumerate() {
            layout.begin_unit(u as u32, scope_of(u));
            if let Some(d) = &p.data {
                for sec in &d.sections {
                    match sec {
                        DataSection::WorkingStorage(decls) | DataSection::LocalStorage(decls) => layout.add_section(decls)?,
                        DataSection::FileSection(fds) => {
                            for fd in fds {
                                layout.add_file_records_of(&fd.name, fd.is_external, &fd.records)?;
                            }
                        }
                        DataSection::Linkage(decls) => layout.add_linkage(decls)?,
                        DataSection::Screen(_) => {
                            return Err(GenError::new(0, "SCREEN SECTION is not supported in a WebAssembly module; use a form"))
                        }
                    }
                }
            }
            layout.end_unit();
        }
        layout.finish();

        // What each unit offers a CALL: its USING parameters and RETURNING item.
        let mut units: Vec<UnitInfo> = Vec::new();
        for (u, (p, _)) in order.iter().enumerate() {
            layout.set_scope(u as u32, scope_of(u));
            let mut params = Vec::new();
            for name in &p.procedure.using {
                let item = layout.lookup(name, &[]).map_err(|m| GenError::new(0, format!("PROCEDURE DIVISION USING {name}: {m}")))?;
                let it = &layout.items[item];
                let Some(slot) = it.link else {
                    return Err(GenError::new(
                        it.span_line,
                        format!("PROCEDURE DIVISION USING {name}: it is not a LINKAGE SECTION record"),
                    ));
                };
                params.push(ParamInfo { slot, item, size: it.size });
            }
            let returning = match &p.procedure.returning {
                Some(n) => Some(layout.lookup(n, &[]).map_err(|m| GenError::new(0, format!("RETURNING {n}: {m}")))?),
                None => None,
            };
            units.push(UnitInfo {
                name: p.identification.program_id.to_ascii_uppercase(),
                entry: 0,
                exit: 0,
                params,
                returning,
                scope: scope_of(u),
            });
        }
        layout.set_scope(0, scope_of(0));

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
            diags: Vec::new(),
            conformance,
            tolerant,
            files: HashMap::new(),
            record_file: HashMap::new(),
            temp_next: 0,
            ctemp_next: 0,
            when_compiled,
            alter_slots: HashMap::new(),
            unit_by_name: units.iter().enumerate().map(|(i, u)| (u.name.clone(), i)).collect(),
            units,
            cur_unit: 0,
            prologue: 0,
            file_tables: vec![HashMap::new(); order.len()],
            global_files: HashSet::new(),
            body_len: 0,
            decl_ranges: Vec::new(),
            handlers: vec![Vec::new(); order.len()],
            use_guard: None,
            status_hold: None,
            names,
            form,
            park_slot: None,
            unit_base,
            external_handles: HashMap::new(),
        };

        // Phase A, every unit: find the paragraphs and make their blocks, so a
        // CALL (which jumps into another unit) knows where it goes.
        let mut states: Vec<UnitState<'a>> = Vec::new();
        for (u, (p, _)) in order.iter().enumerate() {
            l.enter_unit(u, p);
            l.build_files()?;
            l.file_tables[u] = l.files.iter().map(|(n, f)| (n.clone(), (*f, l.global_files.contains(n)))).collect();
            l.flatten()?;
            l.prescan()?;
            l.find_alters()?;
            l.prologue = l.new_block(if u == 0 { "program" } else { "program-prologue" });
            l.program_end = l.new_block(if u == 0 { "program-end" } else { "program-exit" });
            for i in 0..l.paras.len() {
                let n = format!("{}", l.paras[i].name);
                let e = l.new_block(&format!("{n}"));
                let x = l.new_block(&format!("{n}.end"));
                l.entries.push(e);
                l.ends.push(x);
            }
            // A declarative is entered by a PERFORM from the verb that failed.
            for &(_, last) in &l.decl_ranges {
                l.perform_ends.insert(last);
            }
            l.handlers[u] = l
                .decl_ranges
                .iter()
                .zip(&p.procedure.declaratives)
                .map(|(&(first, last), up)| Handler {
                    entry: l.entries[first],
                    end: l.ends[last],
                    files: up.files.clone(),
                    modes: up.modes.clone(),
                    catch_all: up.catch_all,
                    global: up.is_global,
                })
                .collect();
            let first_body = if l.body_len > 0 { l.entries.first().copied() } else { None };
            l.units[u].entry = first_body.unwrap_or(l.program_end);
            l.units[u].exit = l.program_end;
            states.push(l.take_state());
        }
        let main_entry = states[0].prologue;
        let main_first = if states[0].body_len > 0 { states[0].entries[0] } else { states[0].program_end };

        // Phase B, every unit: the code.
        for (u, (p, _)) in order.iter().enumerate() {
            l.enter_unit(u, p);
            l.put_state(&mut states[u]);
            // The prologues run once, in a chain, before the main program's
            // first paragraph: ALTER slots, the collating sequence, switches.
            let after = if u + 1 < order.len() { states[u + 1].prologue } else { main_first };
            l.generate_unit(u, after)?;
            states[u] = l.take_state();
        }
        if !l.diags.is_empty() {
            let mut d = std::mem::take(&mut l.diags);
            let mut first = d.remove(0);
            first.more = d;
            return Err(first);
        }
        let count = order.len();
        let park_slot = l.park_slot.unwrap_or(u32::MAX);
        Ok(Lowered { entry: main_entry, layout: l.layout, units: count, stack_off, park_slot, event_items: (0, 0) })
    }

    /// Make unit `u` (program `p`) the one being lowered.
    fn enter_unit(&mut self, u: usize, p: &'a Program) {
        self.prog = p;
        self.cur_unit = u;
        let scope = self.units[u].scope.clone();
        self.layout.set_scope(u as u32, scope);
        self.take_state();
        self.loops.clear();
        self.cur_para = 0;
        self.next_sentence = None;
    }

    /// Move the per-unit state out, leaving it empty.
    fn take_state(&mut self) -> UnitState<'a> {
        UnitState {
            paras: std::mem::take(&mut self.paras),
            para_by_name: std::mem::take(&mut self.para_by_name),
            sections: std::mem::take(&mut self.sections),
            perform_ends: std::mem::take(&mut self.perform_ends),
            entries: std::mem::take(&mut self.entries),
            ends: std::mem::take(&mut self.ends),
            program_end: std::mem::take(&mut self.program_end),
            files: std::mem::take(&mut self.files),
            alter_slots: std::mem::take(&mut self.alter_slots),
            prologue: std::mem::take(&mut self.prologue),
            body_len: std::mem::take(&mut self.body_len),
            decl_ranges: std::mem::take(&mut self.decl_ranges),
        }
    }

    /// Put a unit's state back.
    fn put_state(&mut self, st: &mut UnitState<'a>) {
        self.paras = std::mem::take(&mut st.paras);
        self.para_by_name = std::mem::take(&mut st.para_by_name);
        self.sections = std::mem::take(&mut st.sections);
        self.perform_ends = std::mem::take(&mut st.perform_ends);
        self.entries = std::mem::take(&mut st.entries);
        self.ends = std::mem::take(&mut st.ends);
        self.program_end = st.program_end;
        self.files = std::mem::take(&mut st.files);
        self.alter_slots = std::mem::take(&mut st.alter_slots);
        self.prologue = st.prologue;
        self.body_len = st.body_len;
        self.decl_ranges = std::mem::take(&mut st.decl_ranges);
    }

    /// The code of unit `u`: its prologue (continuing to `after`), its
    /// paragraphs and its exit.
    fn generate_unit(&mut self, u: usize, after: BlockId) -> GenResult<()> {
        // The exit: the main program ends the run; a called program returns to
        // its caller by dropping every frame down to the call frame.
        self.begin(self.program_end);
        if u == 0 {
            self.emit(Ins::Raw(I::I32Const(ret::DONE)));
            self.finish_raw();
        } else {
            self.unit_exit_code();
        }

        // Prologue.
        self.begin(self.prologue);
        self.init_alter_slots()?;
        if u == 0 {
            // PROGRAM COLLATING SEQUENCE: install the table before anything compares.
            if let Some(name) = &self.prog.collating_sequence {
                let up_name = name.to_ascii_uppercase();
                if let Some((_, spec)) = self.prog.alphabets.iter().find(|(n, _)| n.to_ascii_uppercase() == up_name) {
                    if let Some(t) = crate::collate::table(spec) {
                        let addr = self.pool.intern(&t);
                        self.i32c(addr as i32);
                        self.call_rt("rt_set_collation");
                    }
                }
            }
        }
        // External switches (conformance): the host's answer seeds each one's
        // mnemonic item before the first statement, as the interpreter does.
        if self.conformance && !self.prog.switch_names.is_empty() {
            self.seed_switches()?;
        }
        self.jump(after);
        for i in 0..self.paras.len() {
            self.lower_para(i)?;
        }
        Ok(())
    }

    /// Push the lowest address of the region's PERFORM stack.
    pub(crate) fn stack_lo(&mut self) {
        self.emit(Ins::GlobalGet(G::Base));
        self.i32c(self.map.stack_off as i32);
        self.raw(I::I32Add);
    }

    /// Push the address just past the region's PERFORM stack.
    pub(crate) fn stack_hi(&mut self) {
        self.emit(Ins::GlobalGet(G::Base));
        self.i32c((self.map.stack_off + self.map.stack_size) as i32);
        self.raw(I::I32Add);
    }

    /// The exit block of a called program: pop frames until the call frame
    /// (whose end word is this block) is on top, then return to its caller.
    fn unit_exit_code(&mut self) {
        let me = self.program_end;
        let psp = |s: &mut Self| s.emit(Ins::GlobalGet(G::Psp));
        // psp <= base: nothing to return to.
        psp(self);
        self.stack_lo();
        self.raw(I::I32LeU);
        self.raw(I::If(wasm_encoder::BlockType::Empty));
        self.i32c(cobolt_wasm_rt::state::err::PERFORM_DEPTH as i32);
        self.emit(Ins::CallGlue(Glue::Abort));
        self.raw(I::End);
        self.raw(I::Block(wasm_encoder::BlockType::Empty));
        self.raw(I::Loop(wasm_encoder::BlockType::Empty));
        psp(self);
        self.i32c(8);
        self.raw(I::I32Sub);
        self.emit(Ins::GlobalSet(G::Psp));
        psp(self);
        self.raw(I::I32Load(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
        self.emit(Ins::Pc(me));
        self.raw(I::I32Eq);
        self.raw(I::BrIf(1));
        self.raw(I::Br(0));
        self.raw(I::End);
        self.raw(I::End);
        psp(self);
        self.raw(I::I32Load(wasm_encoder::MemArg { offset: 4, align: 2, memory_index: 0 }));
        self.finish_raw();
    }

    /// Emit the call that asks the host for the state of every external
    /// switch the program declares (`SPECIAL-NAMES. SWITCH-1 IS SW-1`).
    fn seed_switches(&mut self) -> GenResult<()> {
        let names = self.prog.switch_names.clone();
        let mut table: Vec<u8> = Vec::new();
        let mut rows: Vec<[u32; 5]> = Vec::new();
        for (external, mnemonic) in &names {
            let Ok(item) = self.layout.lookup(mnemonic, &[]) else { continue };
            let off = self.layout.items[item].off;
            let ext = self.pool.intern(external.as_bytes());
            let mn = self.pool.intern(mnemonic.as_bytes());
            rows.push([ext, external.len() as u32, mn, mnemonic.len() as u32, off]);
        }
        if rows.is_empty() {
            return Ok(());
        }
        table.extend_from_slice(&(rows.len() as u32).to_le_bytes());
        for r in rows {
            for w in r {
                table.extend_from_slice(&w.to_le_bytes());
            }
        }
        let addr = self.pool.intern(&table);
        self.base();
        self.i32c(addr as i32);
        self.call_rt("rt_switches");
        Ok(())
    }

    // ── files (conformance builds) ────────────────────────────────────────

    pub(crate) fn build_files(&mut self) -> GenResult<()> {
        if !self.conformance {
            return Ok(());
        }
        let prog = self.prog;
        let Some(io) = prog.environment.as_ref().and_then(|e| e.input_output.as_ref()) else { return Ok(()) };
        // record names per file, from the FILE SECTION
        let mut records: HashMap<String, Vec<String>> = HashMap::new();
        let mut external_fds: HashSet<String> = HashSet::new();
        self.global_files.clear();
        if let Some(d) = &prog.data {
            for sec in &d.sections {
                if let DataSection::FileSection(fds) = sec {
                    for fd in fds {
                        if fd.is_global {
                            self.global_files.insert(up(&fd.name));
                        }
                        if fd.is_external {
                            external_fds.insert(up(&fd.name));
                        }
                        records.insert(
                            up(&fd.name),
                            fd.records.iter().filter_map(|r| r.name.as_ref().map(|n| up(n))).collect(),
                        );
                    }
                }
            }
        }
        for fc in &io.file_controls {
            let name = up(&fc.name);
            // An EXTERNAL file is one open file for every unit that declares it.
            let handle_off = if external_fds.contains(&name) {
                match self.external_handles.get(&name).copied() {
                    Some(h) => h,
                    None => {
                        let h = self.layout.alloc_hidden(4);
                        self.external_handles.insert(name.clone(), h);
                        h
                    }
                }
            } else {
                self.layout.alloc_hidden(4)
            };
            let status_opd = match &fc.file_status {
                Some(sn) => match self.layout.lookup(sn, &[]) {
                    Ok(i) => self.item_operand(i, &[], None, fc.span.line)?,
                    Err(_) => 0,
                },
                None => 0,
            };
            let org = match fc.organization {
                cobolt_ast::program::FileOrganization::LineSequential => 1,
                _ => 0,
            };
            let desc = self.pool.file_desc(fc.assign.as_bytes(), org, fc.optional, handle_off, status_opd);
            let mut rec_item = None;
            for (k, rn) in records.get(&name).cloned().unwrap_or_default().iter().enumerate() {
                if let Ok(i) = self.layout.lookup(rn, &[]) {
                    if k == 0 {
                        rec_item = Some(i);
                    }
                    self.record_file.insert(i, name.clone());
                }
            }
            self.files.insert(name, FileInfo { desc, handle_off, rec_item });
        }
        Ok(())
    }

    pub(crate) fn file(&self, name: &str, line: u32) -> GenResult<FileInfo> {
        if !self.conformance {
            return self.err(
                line,
                "file input/output is not available in a browser module: it runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE",
            );
        }
        let n = up(name);
        if let Some(f) = self.files.get(&n) {
            return Ok(*f);
        }
        // A GLOBAL file of a unit that contains this one.
        for &u in self.units[self.cur_unit].scope.iter().skip(1) {
            if let Some((f, true)) = self.file_tables[u as usize].get(&n) {
                return Ok(*f);
            }
        }
        Err(GenError::new(line, format!("file {name} is not defined")))
    }

    pub(crate) fn open_(&mut self, mode: OpenMode, files: &[String], line: u32) -> GenResult<()> {
        let m = match mode {
            OpenMode::Input => 0,
            OpenMode::Output => 1,
            OpenMode::InputOutput => 2,
            OpenMode::Extend => 3,
        };
        for f in files {
            let fi = self.file(f, line)?;
            self.base();
            self.i32c(fi.desc as i32);
            self.i32c(m);
            self.call_rt("rt_file_open");
            self.file_verb_done(f, fi.handle_off, false);
        }
        Ok(())
    }

    pub(crate) fn close_(&mut self, files: &[String], line: u32) -> GenResult<()> {
        for f in files {
            let fi = self.file(f, line)?;
            self.base();
            self.i32c(fi.desc as i32);
            self.call_rt("rt_file_close");
            self.file_verb_done(f, fi.handle_off, false);
        }
        Ok(())
    }

    /// The file whose record area `e` names.
    pub(crate) fn file_of_record(&mut self, e: &Expr, line: u32) -> GenResult<(String, FileInfo)> {
        let base = match e {
            Expr::Subscript { base, .. } => base.as_ref(),
            other => other,
        };
        let idx = self.resolve_item(base)?;
        let mut cur = Some(idx);
        while let Some(i) = cur {
            if let Some(f) = self.record_file.get(&i) {
                let name = f.clone();
                let info = self.file(&name, line)?;
                return Ok((name, info));
            }
            cur = self.layout.items[i].parent;
        }
        self.err(line, "the record is not in a file")
    }

    pub(crate) fn write_(
        &mut self,
        record: &Expr,
        from: Option<&Expr>,
        advancing: Option<&AdvancingClause>,
        line: u32,
    ) -> GenResult<()> {
        if !self.conformance {
            return self.file("", line).map(|_| ());
        }
        let (file_name, fi) = self.file_of_record(record, line)?;
        let rec = self.receiver(record)?;
        if let Some(src) = from {
            self.move_(src, std::slice::from_ref(record), line)?;
        }
        // ADVANCING phrase -> the `adv` word (see `cobolt_wasm_rt::file`)
        use cobolt_wasm_rt::file::{ADV_BEFORE, ADV_NONE, ADV_PAGE};
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(rec as i32);
        match advancing {
            None => self.i32c(ADV_NONE as i32),
            Some(a) => {
                let flag = if a.before { ADV_BEFORE } else { 0 };
                match &a.lines {
                    Expr::Literal(Literal::Integer(n), _) => self.i32c((flag | (*n as u32 & 0xFFFF)) as i32),
                    Expr::Identifier(n, _) if n.eq_ignore_ascii_case("PAGE") => self.i32c((flag | ADV_PAGE) as i32),
                    other => match self.operand(other)? {
                        // A data item: its integer value; a mnemonic-name (C01) or
                        // anything unresolved: a page.
                        Some(o) => {
                            self.i32c(flag as i32);
                            self.base();
                            self.i32c(o as i32);
                            self.call_rt("rt_int");
                            self.raw(I::I32WrapI64);
                            self.i32c(0xFFFF);
                            self.raw(I::I32And);
                            self.raw(I::I32Or);
                        }
                        None => self.i32c((flag | ADV_PAGE) as i32),
                    },
                }
            }
        }
        self.call_rt("rt_file_write");
        self.file_verb_done(&file_name, fi.handle_off, false);
        Ok(())
    }

    pub(crate) fn read_(
        &mut self,
        file: &str,
        into: Option<&Expr>,
        at_end: &[Stmt],
        not_at_end: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let fi = self.file(file, line)?;
        let rec_item = fi.rec_item.ok_or_else(|| GenError::new(line, format!("file {file} has no record")))?;
        let rec = self.item_operand(rec_item, &[], None, line)?;
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(rec as i32);
        self.call_rt("rt_file_read");
        self.file_verb_done(file, fi.handle_off, !at_end.is_empty());
        let end_b = self.new_block("at-end");
        let ok_b = self.new_block("not-at-end");
        let join = self.new_block("end-read");
        // The status the READ left, after any USE procedure has run.
        let hold = self.status_slot();
        self.base();
        self.raw(I::I32Load(wasm_encoder::MemArg { offset: hold as u64, align: 2, memory_index: 0 }));
        self.i32c(10);
        self.raw(I::I32Eq);
        self.branch(end_b, ok_b);
        self.begin(end_b);
        self.stmts(at_end)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(ok_b);
        if let Some(target) = into {
            let sender = Expr::Identifier(self.layout.items[rec_item].name.clone().unwrap_or_default(), cobolt_lexer::Span::new(0, 0, line, 0));
            self.move_(&sender, std::slice::from_ref(target), line)?;
        }
        self.stmts(not_at_end)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    // ── block plumbing ────────────────────────────────────────────────────

    pub(crate) fn new_block(&mut self, name: &str) -> BlockId {
        self.blocks.push(Block { name: name.to_string(), code: Vec::new(), succ: Vec::new(), done: false });
        self.blocks.len() - 1
    }

    pub(crate) fn begin(&mut self, id: BlockId) {
        self.cur = Some(id);
    }

    pub(crate) fn emit(&mut self, i: Ins) {
        let id = self.cur.expect("emit with no current block");
        self.blocks[id].code.push(i);
    }

    pub(crate) fn raw(&mut self, i: I<'static>) {
        self.emit(Ins::Raw(i));
    }

    pub(crate) fn i32c(&mut self, v: i32) {
        self.raw(I::I32Const(v));
    }

    /// Terminate the current block by returning the code already on the stack.
    pub(crate) fn finish_raw(&mut self) {
        if let Some(id) = self.cur.take() {
            self.blocks[id].done = true;
        }
    }

    /// End the current block with a jump to `target`.
    pub(crate) fn jump(&mut self, target: BlockId) {
        let id = self.cur.expect("jump with no current block");
        self.blocks[id].code.push(Ins::Pc(target));
        self.blocks[id].succ.push(target);
        self.blocks[id].done = true;
        self.cur = None;
    }

    /// End the current block with `if <i32 on the stack> then a else b`.
    pub(crate) fn branch(&mut self, then_b: BlockId, else_b: BlockId) {
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
    pub(crate) fn dead_block(&mut self) {
        let b = self.new_block("unreachable");
        self.begin(b);
    }

    pub(crate) fn line(&mut self, line: u32) {
        self.i32c(line as i32);
        self.emit(Ins::GlobalSet(G::Line));
    }

    pub(crate) fn base(&mut self) {
        self.emit(Ins::GlobalGet(G::Base));
    }

    pub(crate) fn call_rt(&mut self, name: &'static str) {
        self.emit(Ins::CallRt(name));
    }

    // ── ALTER ─────────────────────────────────────────────────────────────

    /// Which paragraphs some `ALTER` redirects: each gets a slot in the region
    /// holding the block its `GO TO` currently leads to.
    pub(crate) fn find_alters(&mut self) -> GenResult<()> {
        let mut names: Vec<String> = Vec::new();
        for p in &self.paras {
            for st in p.stmts {
                st.walk(&mut |s| {
                    if let Stmt::Alter { from, .. } = s {
                        names.push(up(from));
                    }
                });
            }
        }
        for n in names {
            if let Some(&i) = self.para_by_name.get(&n).and_then(|v| v.first()) {
                if !self.alter_slots.contains_key(&i) {
                    let slot = self.layout.alloc_hidden(4);
                    self.alter_slots.insert(i, slot);
                }
            }
        }
        Ok(())
    }

    /// Store each altered paragraph's original `GO TO` target in its slot.
    pub(crate) fn init_alter_slots(&mut self) -> GenResult<()> {
        let slots: Vec<(usize, u32)> = self.alter_slots.iter().map(|(a, b)| (*a, *b)).collect();
        for (para, slot) in slots {
            let first_goto = self.paras[para].stmts.iter().find_map(|s| match s {
                Stmt::GoTo { target, section, span } => Some((target.clone(), section.clone(), span.line)),
                _ => None,
            });
            let dest = match first_goto {
                Some((t, sec, line)) if !t.is_empty() => {
                    let p = self.goto_target(&t, sec.as_deref(), line)?;
                    self.entries[p]
                }
                _ => self.program_end,
            };
            self.base();
            self.emit(Ins::Pc(dest));
            self.raw(I::I32Store(wasm_encoder::MemArg { offset: slot as u64, align: 2, memory_index: 0 }));
        }
        Ok(())
    }

    // ── paragraphs ────────────────────────────────────────────────────────

    pub(crate) fn flatten(&mut self) -> GenResult<()> {
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
        self.body_len = self.paras.len();
        // The declaratives' paragraphs follow the body but are never fallen into:
        // a USE procedure is performed by the verb whose error it handles.
        self.decl_ranges.clear();
        for up in &prog.procedure.declaratives {
            let first = self.paras.len();
            if !up.stmts.is_empty() || up.paras.is_empty() {
                self.paras.push(Para { name: String::new(), section: Some(up.section.clone()), stmts: &up.stmts, line: up.span.line });
            }
            for pg in &up.paras {
                self.paras.push(Para { name: up_name(&pg.name), section: Some(up.section.clone()), stmts: &pg.stmts, line: pg.span.line });
            }
            let last = self.paras.len() - 1;
            self.sections.insert(up.section.clone(), (first, last));
            self.decl_ranges.push((first, last));
        }
        for (i, p) in self.paras.iter().enumerate() {
            if !p.name.is_empty() {
                self.para_by_name.entry(p.name.clone()).or_default().push(i);
            }
        }
        Ok(())
    }

    /// Which paragraphs end some `PERFORM` (they need the return check).
    pub(crate) fn prescan(&mut self) -> GenResult<()> {
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
    pub(crate) fn name_range(&self, from: &str, thru: Option<&str>, line: u32) -> GenResult<Option<(usize, usize)>> {
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
        // The end may precede the start physically (NC102A PFM-TEST-F1-10): the
        // PERFORM returns when control reaches the end of that paragraph, by
        // whatever route, so the range is not clamped.
        Ok(Some((start, end)))
    }

    pub(crate) fn perform_range(&self, t: &PerformTarget, line: u32) -> GenResult<Option<(usize, usize)>> {
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

    pub(crate) fn lower_para(&mut self, i: usize) -> GenResult<()> {
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
            self.stmt_guarded(s);
        }
        if self.cur.is_some() {
            let e = self.ends[i];
            self.jump(e);
        }
        self.next_sentence = None;
        self.lower_para_end(i)?;
        Ok(())
    }

    pub(crate) fn emit_line_of_para(&mut self, i: usize) {
        let l = self.paras[i].line;
        if l > 0 {
            self.line(l);
        }
    }

    /// The block at the end of paragraph `i`: return to the caller if a
    /// `PERFORM` ending here is waiting, otherwise fall into the next paragraph.
    pub(crate) fn lower_para_end(&mut self, i: usize) -> GenResult<()> {
        let end = self.ends[i];
        // The last paragraph of the body does not fall into the declaratives.
        let next = if i + 1 < self.paras.len() && i + 1 != self.body_len { self.entries[i + 1] } else { self.program_end };
        self.begin(end);
        if self.perform_ends.contains(&i) {
            // psp > base && stack[psp-8].end == this block
            self.emit(Ins::GlobalGet(G::Psp));
            self.stack_lo();
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

    pub(crate) fn err<T>(&self, line: u32, msg: impl Into<String>) -> GenResult<T> {
        Err(GenError::new(line, msg))
    }

    pub(crate) fn quals_of(&self, mut e: &Expr) -> Vec<String> {
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
    pub(crate) fn resolve_item(&mut self, e: &Expr) -> GenResult<usize> {
        let line = e.span().line;
        let (name, quals): (String, Vec<String>) = match e {
            Expr::Identifier(n, _) => (n.clone(), vec![]),
            Expr::Qualified { name, of, .. } => (name.clone(), self.quals_of(of)),
            _ => return self.err(line, "a data-name was expected"),
        };
        match self.layout.lookup(&name, &quals) {
            Ok(i) => Ok(i),
            Err(m) => {
                // The CCVS85 X-cards stand for names the installation supplies;
                // a conformance build reads them as the interpreter does under
                // COBOL_TOLERATE_UNDECLARED.
                if self.tolerant && quals.is_empty() && m.ends_with("is not defined") {
                    Ok(self.layout.add_implicit(&name))
                } else {
                    Err(GenError::new(line, m))
                }
            }
        }
    }

    pub(crate) fn lit_number(&mut self, lit: &Literal, line: u32) -> GenResult<Option<u32>> {
        Ok(Some(match lit {
            Literal::Integer(n) => self.pool.opd_lit_num(*n as i128, 0),
            Literal::IntegerDigits(n, written) => self.pool.opd_lit_num_written(*n as i128, 0, *written),
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
    pub(crate) fn operand(&mut self, e: &Expr) -> GenResult<Option<u32>> {
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
                let s = self.operand_or_temp(start)?;
                let l = match length {
                    Some(l) => self.operand_or_temp(l)?,
                    None => 0,
                };
                self.item_operand(idx, indices, Some((s, l)), line).map(Some)
            }
            Expr::Unary { op: UnaryOp::Pos, operand, .. } => self.operand(operand),
            // A character-valued function is an operand once evaluated into its
            // temporary; a numeric one is a computation (`push_expr`).
            Expr::FunctionCall { name, args, .. } => self.function_operand(name, args, line),
            // A control's property or method result is read from the host.
            Expr::Member { .. } if self.is_host_member(e) => self.member_read(e, line).map(Some),
            _ => Ok(None),
        }
    }

    pub(crate) fn item_operand(
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
            let opd = self.operand_or_temp(sub_expr)?;
            let odo_opd = match odo {
                Some(name) => {
                    let di = self.layout.lookup(&name, &[]).map_err(|m| GenError::new(line, m))?;
                    self.item_operand(di, &[], None, line)?
                }
                None => 0,
            };
            subs.push(Sub { opd, stride, max, odo: odo_opd, add });
        }
        let mut fd = self.pool.fd_with_spec(&item.fd, item.pic_template.as_deref(), self.prog.decimal_comma, self.prog.currency);
        // A group that holds an `OCCURS … DEPENDING ON` table is as long as the
        // table's current count makes it (COBOL-85 5.8): the descriptor points
        // at `[stride, max, counter operand]` and `locate` shortens the group.
        if item.is_group {
            if let Some((stride, max, name)) = self.layout.odo_inside(idx) {
                let di = self.layout.lookup(&name, &[]).map_err(|m| GenError::new(line, m))?;
                let odo_opd = self.item_operand(di, &[], None, line)?;
                let mut rec = Vec::with_capacity(12);
                rec.extend_from_slice(&stride.to_le_bytes());
                rec.extend_from_slice(&max.to_le_bytes());
                rec.extend_from_slice(&odo_opd.to_le_bytes());
                fd.aux = self.pool.intern(&rec);
            }
        }
        Ok(self.pool.opd_item_in(item.off, &fd, &subs, refmod, item.link))
    }

    /// Push the numeric value of an expression on the register stack.
    pub(crate) fn push_expr(&mut self, e: &Expr) -> GenResult<()> {
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
            Expr::FunctionCall { name, args, .. } => {
                let f = self.function_call(name, args, line)?;
                if f.chars {
                    return self.err(line, format!("FUNCTION {name} yields characters, which arithmetic cannot use"));
                }
                self.base();
                self.i32c(f.rec as i32);
                self.call_rt("rt_func");
                Ok(())
            }
            Expr::Member { .. } => self.err(line, "control members are not supported in this build of the generator yet"),
            _ => self.err(line, "this expression is not supported in a WebAssembly module yet"),
        }
    }

    // ── conditions ────────────────────────────────────────────────────────

    /// Leave 1 or 0 on the stack.
    pub(crate) fn cond(&mut self, c: &Condition) -> GenResult<()> {
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
                let o = self.operand_or_temp(expr)?;
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
            Condition::UserClassTest { expr, negated, class, .. } => {
                let o = self.operand_required(expr, line)?;
                let name = class.to_ascii_uppercase();
                let Some((_, ranges)) = self.prog.classes.iter().find(|(n, _)| n.to_ascii_uppercase() == name) else {
                    return self.err(line, format!("class {class} is not defined"));
                };
                // A 256-bit bitmap of the characters that belong.
                let mut table = [0u8; 32];
                for (lo, hi) in ranges {
                    for c in (*lo as u32)..=(*hi as u32) {
                        if c < 256 {
                            table[(c >> 3) as usize] |= 1 << (c & 7);
                        }
                    }
                }
                let t = self.pool.intern(&table);
                self.base();
                self.i32c(o as i32);
                self.i32c(t as i32);
                self.call_rt("rt_class_user");
                if *negated {
                    self.raw(I::I32Eqz);
                }
                Ok(())
            }
        }
    }

    pub(crate) fn operand_required(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        match self.operand(e)? {
            Some(o) => Ok(o),
            None => self.err(line, "a data item was expected here"),
        }
    }

    pub(crate) fn compare(&mut self, lhs: &Expr, op: u32, rhs: &Expr, line: u32) -> GenResult<()> {
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
    pub(crate) fn cond_name(&mut self, cond_idx: usize, host_indices: &[Expr], line: u32) -> GenResult<()> {
        let item = self.layout.items[cond_idx].clone();
        if item.level != 88 {
            // Not a condition-name: "holds something other than zero".
            let o = self.item_operand(cond_idx, host_indices, None, line)?;
            let z = self.pool.opd_lit_num(0, 0);
            self.base();
            self.i32c(o as i32);
            self.i32c(z as i32);
            self.i32c(OP_NE as i32);
            self.call_rt("rt_rel");
            return Ok(());
        }
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

    pub(crate) fn cond_literal(&mut self, l: &Literal, host_numeric: bool, line: u32) -> GenResult<u32> {
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

    pub(crate) fn stmts(&mut self, list: &[Stmt]) -> GenResult<()> {
        for s in list {
            if self.cur.is_none() {
                self.dead_block();
            }
            self.stmt_guarded(s);
        }
        Ok(())
    }

    /// Lower one statement; a construct that cannot be compiled is recorded
    /// and the statement skipped, so one run reports them all.
    pub(crate) fn stmt_guarded(&mut self, s: &Stmt) {
        if let Err(e) = self.stmt(s) {
            self.diags.push(e);
            if self.cur.is_none() {
                self.dead_block();
            }
        }
    }

    pub(crate) fn stmt(&mut self, s: &Stmt) -> GenResult<()> {
        self.temp_next = 0;
        self.ctemp_next = 0;
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
                if let Some(&slot) = self.alter_slots.get(&self.cur_para) {
                    // An altered GO TO: the destination is whatever the slot holds.
                    self.base();
                    self.raw(I::I32Load(wasm_encoder::MemArg { offset: slot as u64, align: 2, memory_index: 0 }));
                    let cur = self.cur.take().expect("current block");
                    self.blocks[cur].done = true;
                    return Ok(());
                }
                let t = self.goto_target(target, section.as_deref(), line)?;
                self.jump(self.entries[t]);
                Ok(())
            }
            Stmt::Alter { from, to, .. } => self.alter_(from, to, line),
            Stmt::Inspect { target, spec, .. } => self.inspect_(target, spec, line),
            Stmt::String_ { operands, into, pointer, on_overflow, not_on_overflow, .. } => {
                self.string_(operands, into, pointer.as_ref(), on_overflow, not_on_overflow)
            }
            Stmt::Unstring { from, delimited_by, all, into, pointer, tallying, on_overflow, not_on_overflow, .. } => {
                self.unstring_(from, delimited_by, *all, into, pointer.as_ref(), tallying.as_ref(), on_overflow, not_on_overflow)
            }
            Stmt::Search { all, table, varying, at_end, whens, .. } => {
                self.search_(*all, table, varying.as_ref(), at_end, whens, line)
            }
            Stmt::MoveCorresponding { from, to, .. } => self.corresponding(0, from, to, false, &[], &[], line),
            Stmt::AddCorresponding { from, to, rounded, on_size_error, not_on_size_error, .. } => {
                self.corresponding(1, from, to, *rounded, on_size_error, not_on_size_error, line)
            }
            Stmt::SubtractCorresponding { from, to, rounded, on_size_error, not_on_size_error, .. } => {
                self.corresponding(2, from, to, *rounded, on_size_error, not_on_size_error, line)
            }
            Stmt::Initialize { items, replacing, .. } => self.initialize_(items, replacing, line),
            Stmt::GoToDepending { targets, depending, .. } => self.go_to_depending(targets, depending, line),
            Stmt::Open { mode, files, extra_modes, .. } => {
                self.open_(*mode, files, line)?;
                for (m, fs) in extra_modes {
                    self.open_(*m, fs, line)?;
                }
                Ok(())
            }
            Stmt::Close { files, .. } => self.close_(files, line),
            Stmt::Write { record, from, advancing, .. } => self.write_(record, from.as_ref(), advancing.as_ref(), line),
            Stmt::Read { file, into, at_end, not_at_end, .. } => self.read_(file, into.as_ref(), at_end, not_at_end, line),
            Stmt::Accept { target, from, .. } => self.accept_(target, from.as_ref(), line),
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
            Stmt::GoBack { .. } => self.exit_program(),
            Stmt::Call { program, using, returning, on_exception, not_on_exception, .. } => {
                self.call_(program, using, returning.as_ref(), on_exception, not_on_exception, line)
            }
            Stmt::Cancel { programs, .. } => self.cancel_(programs, line),
            Stmt::InvokeExpr { expr, .. } => self.member_call(expr, line),
            Stmt::Invoke { object, method, args, returning, .. } => self.invoke_(object, method, args, returning.as_ref(), line),
            Stmt::ControlSet { control, property, value, .. } => {
                let target = Expr::Member {
                    recv: Box::new(control.clone()),
                    member: property.clone(),
                    args: vec![],
                    parens: false,
                    span: cobolt_lexer::Span::new(0, 0, line, 0),
                };
                let v = self.operand_or_temp(value)?;
                self.member_set(&target, v, line)
            }
            Stmt::WindowOp { op, .. } => self.window_op(op, line),
            other => {
                let name = crate::support::stmt_name(other);
                match crate::support::support(other) {
                    crate::support::Support::BackendOnly(why) | crate::support::Support::Unsupported(why) => {
                        self.err(line, format!("{name} is not available in a browser module: {why}"))
                    }
                    _ => self.err(line, format!("{name} is not yet supported by the WebAssembly generator")),
                }
            }
        }
    }

    pub(crate) fn goto_target(&self, target: &str, section: Option<&str>, line: u32) -> GenResult<usize> {
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

    /// `EXIT PROGRAM` / `GOBACK`: a called program returns to its caller; the
    /// main program ends the run.
    pub(crate) fn exit_program(&mut self) -> GenResult<()> {
        if self.cur_unit == 0 {
            self.call_rt("rt_flush");
            self.i32c(ret::DONE);
            self.finish_raw();
        } else {
            let exit = self.program_end;
            self.jump(exit);
        }
        Ok(())
    }

    pub(crate) fn exit(&mut self, kind: ExitKind, line: u32) -> GenResult<()> {
        match kind {
            ExitKind::Point => Ok(()),
            ExitKind::Program => self.exit_program(),
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

    pub(crate) fn is_numeric_item(&mut self, e: &Expr) -> bool {
        let base = match e {
            Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => base.as_ref(),
            other => other,
        };
        match self.resolve_item(base) {
            Ok(i) => self.layout.items[i].fd.is_numeric(),
            Err(_) => false,
        }
    }

    pub(crate) fn move_(&mut self, from: &Expr, to: &[Expr], line: u32) -> GenResult<()> {
        // A MOVE whose receiver is an 88-level condition-name is `SET … TO TRUE`;
        // each such receiver is set, the others are moved to.
        let mut plain: Vec<Expr> = Vec::new();
        let mut conds: Vec<usize> = Vec::new();
        for t in to {
            let mut is_cond = false;
            if let Expr::Identifier(..) | Expr::Qualified { .. } = t {
                if let Ok(i) = self.resolve_item(t) {
                    if self.layout.items[i].level == 88 {
                        conds.push(i);
                        is_cond = true;
                    }
                }
            }
            if !is_cond {
                plain.push(t.clone());
            }
        }
        if !conds.is_empty() {
            for c in conds {
                self.set_condition(c, from, line)?;
            }
            if plain.is_empty() {
                return Ok(());
            }
            return self.move_plain(from, &plain, line);
        }
        self.move_plain(from, to, line)
    }

    pub(crate) fn move_plain(&mut self, from: &Expr, to: &[Expr], line: u32) -> GenResult<()> {
        // A control member as a receiver is a write to the host.
        if to.iter().any(|t| matches!(t, Expr::Member { .. })) {
            if let Some(bad) = to.iter().find(|t| matches!(t, Expr::Member { .. }) && !self.is_host_member(t)) {
                let _ = bad;
                return self.err(line, "a value method cannot be a receiving field");
            }
            let src = self.operand_or_temp(from)?;
            for t in to {
                self.move_operand_to(src, t, line)?;
            }
            return Ok(());
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

    pub(crate) fn receiver(&mut self, e: &Expr) -> GenResult<u32> {
        let line = e.span().line;
        match self.operand(e)? {
            Some(o) => Ok(o),
            None => self.err(line, "a data item was expected as the receiving field"),
        }
    }

    /// `SET condition-name TO TRUE | FALSE`.
    pub(crate) fn set_condition(&mut self, cond_idx: usize, from: &Expr, line: u32) -> GenResult<()> {
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

    pub(crate) fn flags(rounded: bool, phrase: bool) -> i32 {
        (if rounded { ST_ROUNDED } else { 0 } | if phrase { ST_PHRASE } else { 0 }) as i32
    }

    /// After an arithmetic statement: `ON SIZE ERROR` / `NOT ON SIZE ERROR`.
    pub(crate) fn size_error_tail(&mut self, on: &[Stmt], not_on: &[Stmt]) -> GenResult<()> {
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

    pub(crate) fn store_to(&mut self, target: &Expr, rounded: bool, phrase: bool, dup: bool) -> GenResult<()> {
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

    pub(crate) fn add(
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
    pub(crate) fn push_sum(&mut self, operands: &[Expr]) -> GenResult<()> {
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

    pub(crate) fn subtract(
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
    pub(crate) fn multiply(
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
    pub(crate) fn divide(
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
    pub(crate) fn store_to_keep(&mut self, target: &Expr, rounded: bool, phrase: bool) -> GenResult<()> {
        self.store_to(target, rounded, phrase, true)
    }

    pub(crate) fn compute(
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

    pub(crate) fn if_(&mut self, c: &Condition, then_s: &[Stmt], else_s: &[Stmt]) -> GenResult<()> {
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

    pub(crate) fn evaluate(
        &mut self,
        subjects: &[EvalSubject],
        whens: &[WhenClause],
        other: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let join = self.new_block("end-evaluate");
        // A condition-name written as a subject is a condition; a data
        // reference the parser read as a condition is an operand.
        let mut re: Vec<EvalSubject> = Vec::with_capacity(subjects.len());
        for s in subjects {
            re.push(match s {
                EvalSubject::Expr(e @ (Expr::Identifier(..) | Expr::Qualified { .. })) => match self.resolve_item(e) {
                    Ok(i) if self.layout.items[i].level == 88 => {
                        let name = self.layout.items[i].name.clone().unwrap_or_default();
                        EvalSubject::Cond(Condition::ConditionName(name, e.span()))
                    }
                    _ => s.clone(),
                },
                EvalSubject::Cond(Condition::ConditionRef(e, _)) => {
                    let (leaf, quals) = match e.as_ref() {
                        Expr::Subscript { base, .. } => split_name(base),
                        other => split_name(other),
                    };
                    match self.layout.lookup(&leaf, &quals) {
                        Ok(i) if self.layout.items[i].level != 88 => EvalSubject::Expr(e.as_ref().clone()),
                        _ => s.clone(),
                    }
                }
                EvalSubject::Cond(Condition::ConditionName(n, sp)) => match self.layout.lookup(n, &[]) {
                    Ok(i) if self.layout.items[i].level != 88 => EvalSubject::Expr(Expr::Identifier(n.clone(), *sp)),
                    _ => s.clone(),
                },
                _ => s.clone(),
            });
        }
        let subjects: &[EvalSubject] = &re;
        // Stacked WHENs: consecutive phrases share the statements of the next
        // one that has any (or WHEN OTHER's when none does).
        let bodies: Vec<BlockId> = whens.iter().map(|_| self.new_block("when")).collect();
        let other_b = self.new_block("when-other");
        for (wi, w) in whens.iter().enumerate() {
            let body = match whens[wi..].iter().position(|x| !x.stmts.is_empty()) {
                Some(d) => bodies[wi + d],
                None => other_b,
            };
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
            if !w.stmts.is_empty() {
                self.begin(bodies[wi]);
                self.stmts(&w.stmts)?;
                if self.cur.is_some() {
                    self.jump(join);
                }
            }
            self.begin(next);
        }
        self.jump(other_b);
        self.begin(other_b);
        self.stmts(other)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    pub(crate) fn when_match(&mut self, subject: &EvalSubject, v: &WhenValue, line: u32) -> GenResult<()> {
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
                // `EVALUATE X NUMERIC WHEN condition`: both truth values agree.
                EvalSubject::Cond(sc) => {
                    self.cond(sc)?;
                    self.cond(c)?;
                    self.raw(I::I32Eq);
                    Ok(())
                }
                EvalSubject::Expr(_) => {
                    // A bare data-name as the selection object is an operand, not
                    // a condition: only a declared 88-level is one (the same
                    // ambiguity `Condition::NameOrAbbrev` carries).
                    let obj: Option<Expr> = match c {
                        Condition::ConditionName(n, sp) => match self.layout.lookup(n, &[]) {
                            Ok(i) if self.layout.items[i].level != 88 => Some(Expr::Identifier(n.clone(), *sp)),
                            Err(_) if self.tolerant => Some(Expr::Identifier(n.clone(), *sp)),
                            _ => None,
                        },
                        Condition::ConditionRef(e, _) => {
                            let (leaf, quals) = match e.as_ref() {
                                Expr::Subscript { base, .. } => split_name(base),
                                other => split_name(other),
                            };
                            match self.layout.lookup(&leaf, &quals) {
                                Ok(i) if self.layout.items[i].level != 88 => Some(e.as_ref().clone()),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    match obj {
                        Some(e) => self.subject_rel(subject, OP_EQ, &e, line),
                        // A genuine condition selects on its own truth value.
                        None => self.cond(c),
                    }
                }
            },
            WhenValue::Literal(l) => {
                if let EvalSubject::Cond(sc) = subject {
                    // `WHEN TRUE` / `WHEN FALSE` reach here as the literals 1 and 0.
                    let wanted = matches!(l, Literal::Integer(1));
                    self.cond(sc)?;
                    self.i32c(wanted as i32);
                    self.raw(I::I32Eq);
                    return Ok(());
                }
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

    pub(crate) fn range_match(&mut self, subject: &EvalSubject, lo: &Expr, hi: &Expr, line: u32) -> GenResult<()> {
        self.subject_rel(subject, OP_GE, lo, line)?;
        self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
        self.subject_rel(subject, OP_LE, hi, line)?;
        self.raw(I::Else);
        self.i32c(0);
        self.raw(I::End);
        Ok(())
    }

    pub(crate) fn subject_rel(&mut self, subject: &EvalSubject, op: u32, value: &Expr, line: u32) -> GenResult<()> {
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
    pub(crate) fn call_range(&mut self, from: usize, to: usize, ret_to: BlockId) {
        let (end, entry) = (self.ends[to], self.entries[from]);
        self.frame_jump(end, entry, ret_to);
    }

    /// Push a frame `(end, ret_to)` and jump to `entry`: a `PERFORM` of blocks
    /// that may lie in another unit.
    pub(crate) fn frame_jump(&mut self, end: BlockId, entry: BlockId, ret_to: BlockId) {
        // Overflow check: psp >= end -> run-time error.
        self.emit(Ins::GlobalGet(G::Psp));
        self.stack_hi();
        self.raw(I::I32GeU);
        self.raw(I::If(wasm_encoder::BlockType::Empty));
        self.i32c(cobolt_wasm_rt::state::err::PERFORM_DEPTH as i32);
        self.emit(Ins::CallGlue(Glue::Abort));
        self.raw(I::End);
        // stack[psp] = (end block, return block)
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(end));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.emit(Ins::Pc(ret_to));
        self.raw(I::I32Store(wasm_encoder::MemArg { offset: 4, align: 2, memory_index: 0 }));
        self.emit(Ins::GlobalGet(G::Psp));
        self.i32c(8);
        self.raw(I::I32Add);
        self.emit(Ins::GlobalSet(G::Psp));
        self.jump(entry);
    }

    pub(crate) fn perform(&mut self, t: &PerformTarget, line: u32) -> GenResult<()> {
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

    pub(crate) fn perform_times(&mut self, count: &Expr, stmts: &[Stmt], line: u32) -> GenResult<()> {
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

    pub(crate) fn perform_until(&mut self, cond: &Condition, test_before: bool, stmts: &[Stmt]) -> GenResult<()> {
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
    pub(crate) fn assign(&mut self, var: &Expr, from: &Expr) -> GenResult<()> {
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
    pub(crate) fn augment(&mut self, var: &Expr, by: &Expr) -> GenResult<()> {
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
    pub(crate) fn perform_varying(
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
            // COBOL-85 6.20.4 GR10(b): every variable is set to its FROM, then
            // condition 1 is tested; true ends the loop. Otherwise condition 2
            // (and on inward): a true one steps the variable of the level
            // outside it, sets every variable inside that level to its FROM and
            // tests again; the innermost condition false runs the body, then
            // steps the innermost variable.
            //   init all; T0: c0 -> exit | T1; T1: c1 -> A0 | T2; ...;
            //   T(n-1): c(n-1) -> A(n-2) | body;   A(k): v(k)+=b(k); v(k+1..)=f; goto T(k)
            let tests: Vec<BlockId> = (0..n).map(|_| self.new_block("varying-test")).collect();
            let advance: Vec<BlockId> = (0..n.saturating_sub(1)).map(|_| self.new_block("varying-advance")).collect();
            for l in &levels {
                self.assign(l.var, l.from)?;
            }
            self.jump(tests[0]);
            for k in 0..n {
                self.begin(tests[k]);
                self.cond(levels[k].until)?;
                let on_true = if k == 0 { exit } else { advance[k - 1] };
                let on_false = if k + 1 < n { tests[k + 1] } else { body_b };
                self.branch(on_true, on_false);
            }
            for k in 0..n.saturating_sub(1) {
                self.begin(advance[k]);
                self.augment(levels[k].var, levels[k].by)?;
                for j in k + 1..n {
                    self.assign(levels[j].var, levels[j].from)?;
                }
                self.jump(tests[k]);
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
            // TEST AFTER (COBOL-85 6.20.4): every variable starts at its FROM, the
            // body runs, then the innermost condition is tested. False: step it
            // and run the body again. True: test the next outer condition as it
            // stands (a variable is stepped only after its own condition came out
            // false, NC201A PFM-TEST-F4-14); the first outer condition that is
            // false steps its variable, restarts every variable inside it at its
            // FROM and runs the body; the outermost condition true ends the loop.
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
            // `outer[k]` (k >= 1): step level k-1, test it.
            let tests: Vec<BlockId> = (0..n).map(|_| self.new_block("varying-test")).collect();
            let outer: Vec<BlockId> = (0..n).map(|_| self.new_block("varying-outer")).collect();
            self.jump(tests[n - 1]);
            // innermost test
            self.begin(tests[n - 1]);
            self.cond(levels[n - 1].until)?;
            let step_in = self.new_block("varying-step");
            let sat_in = if n == 1 { exit } else { outer[n - 1] };
            self.branch(sat_in, step_in);
            self.begin(step_in);
            self.augment(levels[n - 1].var, levels[n - 1].by)?;
            self.jump(body_b);
            for k in (1..n).rev() {
                self.begin(outer[k]);
                self.cond(levels[k - 1].until)?;
                let restart = self.new_block("varying-restart");
                let on_true = if k - 1 == 0 { exit } else { outer[k - 1] };
                self.branch(on_true, restart);
                self.begin(restart);
                self.augment(levels[k - 1].var, levels[k - 1].by)?;
                for j in k..n {
                    self.assign(levels[j].var, levels[j].from)?;
                }
                self.jump(body_b);
            }
            let _ = (tests, line);
        }
        self.begin(exit);
        Ok(())
    }

    pub(crate) fn go_to_depending(&mut self, targets: &[String], depending: &Expr, line: u32) -> GenResult<()> {
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

    // ── ACCEPT ────────────────────────────────────────────────────────────

    pub(crate) fn accept_(&mut self, target: &Expr, from: Option<&AcceptSource>, line: u32) -> GenResult<()> {
        use cobolt_wasm_rt::accept::*;
        let kind = match from {
            None => {
                if !self.conformance {
                    return self.err(
                        line,
                        "ACCEPT from the operator has no meaning in a browser module: the form's controls are the input",
                    );
                }
                ACC_CONSOLE
            }
            Some(AcceptSource::Date) => ACC_DATE,
            Some(AcceptSource::Time) => ACC_TIME,
            Some(AcceptSource::Day) => ACC_DAY,
            Some(AcceptSource::DayOfWeek) => ACC_DAY_OF_WEEK,
            // A mnemonic bound to the console behaves as the plain form.
            Some(AcceptSource::Mnemonic(_)) if self.conformance => ACC_CONSOLE,
            Some(other) => {
                return self.err(line, format!("ACCEPT FROM {other:?} is not supported in a WebAssembly module"));
            }
        };
        let t = self.receiver(target)?;
        self.base();
        self.i32c(t as i32);
        self.i32c(kind as i32);
        self.call_rt("rt_accept");
        Ok(())
    }

    // ── DISPLAY ───────────────────────────────────────────────────────────

    pub(crate) fn display(&mut self, operands: &[Expr], upon: Option<&str>, no_adv: bool, line: u32) -> GenResult<()> {
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
