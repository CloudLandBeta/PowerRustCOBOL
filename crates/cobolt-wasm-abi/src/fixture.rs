// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Test-fixture program modules.** A hand-assembled WebAssembly module that
//! exports the whole ABI v1 surface (§4), imports exactly §3, and does *scripted*
//! things when its handlers run — set a property, call a method, ask a query,
//! suspend on a remote request, yield, trap. It stands in for the module
//! `cobolt-wasm` will generate, so the player, the loader and the host page can be
//! driven end to end before the generator exists. **Nothing in a product build
//! enables this feature.**
//!
//! One [`FixtureSpec`] produces both variants: `multi_memory = true` gives the
//! two-memory module (0 = data, 1 = arena), `false` the single-memory one in which
//! the arena is a region of memory 0 starting at `pr_arena_base()`; the single
//! variant contains no multi-memory instruction at all.
//!
//! How it runs: every handler is cut into *segments* at its suspension, yield and
//! end points. A segment is a function `() -> i32` returning a status. The global
//! `seg` names the segment to run next, so `pr_resume` simply calls the table entry
//! `seg` points at. Commands a segment emits are written into the batch region and
//! flushed when the segment ends, when the region is full and before every query —
//! the batching rules of WASM-017 R11.

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, DataSection, ElementSection, Elements, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemArg, MemorySection, MemoryType, Module,
    RefType, TableSection, TableType, TypeSection, ValType,
};

use crate::batch::{BatchWriter, PathSpec};
use crate::codes::{ctrl as ctrl_id, err, status};
use crate::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, Manifest, VariantInfo, MANIFEST_SECTION};

/// Where a scripted command lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The form itself (`THIS`).
    Form,
    /// The form that opened this one (`super::`).
    Super,
    /// The application object (`COBOL::`).
    App,
    /// A control of the running form, by name.
    Control(String),
}

impl Target {
    pub fn control(name: &str) -> Target {
        Target::Control(name.to_owned())
    }
}

/// One scripted action of a handler.
#[derive(Debug, Clone)]
pub enum Step {
    /// `SET target::name = value`
    Set { target: Target, name: String, value: String },
    /// `SET target::name(args…) = value` — an indexed member
    SetAt { target: Target, name: String, args: Vec<String>, value: String },
    /// `CALL target::name(args…)`, result discarded
    Call { target: Target, name: String, args: Vec<String> },
    Display(String),
    Log { level: u32, text: String },
    /// A query that ends the pending batch (`GET target::name`). The answer's length
    /// is kept; follow with [`Step::EchoAnswer`] to put it somewhere visible.
    Get { target: Target, name: String },
    /// A query `CALLR target::name(args…)`.
    CallResult { target: Target, name: String, args: Vec<String> },
    /// A query `COUNT target::name`.
    Count { target: Target, name: String },
    /// `SET target::name = <the last answer, as text>`
    EchoAnswer { target: Target, name: String },
    /// `SET target::name = <the first event argument>`
    EchoArg0 { target: Target, name: String },
    /// `SET target::name = <the answer frame of the last remote request>`
    EchoFrame { target: Target, name: String },
    /// Send this request frame with `remote` and suspend; the task resumes at the next step.
    Remote(Vec<u8>),
    /// Suspend without a request (the host resumes it itself: a dialog, a timer).
    Suspend,
    /// Yield to the host and carry on at the next step.
    Yield,
    /// `STOP RUN`
    Stop,
    /// Execute `unreachable` — a trap.
    Trap,
    /// Emit an `ERROR` command, call `abort`, then trap.
    Error { code: u32, message: String },
    /// Emit `count` copies of `SET target::name = value`, flushing whenever the batch is full.
    Burst { target: Target, name: String, value: String, count: u32 },
    /// A loop of `iterations` cheap iterations that yields every `slice` of them.
    Spin { iterations: u32, slice: u32 },
    /// Call `now` and `random` once each (so a host's counters see them).
    Clock,
}

#[derive(Debug, Clone)]
pub struct FixtureHandler {
    /// `None` binds the form's own event (`onLoad`, `onClose`, …).
    pub control: Option<String>,
    pub event: String,
    pub steps: Vec<Step>,
}

impl FixtureHandler {
    pub fn on(control: &str, event: &str, steps: Vec<Step>) -> Self {
        FixtureHandler { control: Some(control.to_owned()), event: event.to_owned(), steps }
    }
    pub fn form(event: &str, steps: Vec<Step>) -> Self {
        FixtureHandler { control: None, event: event.to_owned(), steps }
    }
}

#[derive(Debug, Clone)]
pub struct FixtureForm {
    pub name: String,
    pub main: bool,
    pub instances: u32,
    /// `(name, type)` — numbered from 3 in this order.
    pub controls: Vec<(String, String)>,
    pub handlers: Vec<FixtureHandler>,
}

#[derive(Debug, Clone)]
pub struct FixtureSpec {
    pub project: String,
    pub multi_memory: bool,
    pub limits: Limits,
    pub forms: Vec<FixtureForm>,
}

/// A built fixture: the module, its manifest and the interned tables.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub wasm: Vec<u8>,
    pub manifest: Manifest,
}

// ── addresses inside the data memory ────────────────────────────────────────

/// `live[id]` — one byte per instance id (max 128 instances).
const LIVE_ADDR: u32 = 0x0000;
/// `inst_form[id]`
const INST_FORM_ADDR: u32 = 0x0100;
/// `form_live[form]`
const FORM_LIVE_ADDR: u32 = 0x0200;
/// `form_max[form]` — initialised by a data segment.
const FORM_MAX_ADDR: u32 = 0x0300;
/// The handler table: 4 bytes per entry — form, ctrl, event, first segment.
const HANDLER_TABLE_ADDR: u32 = 0x0400;
/// The task table (ABI amendment A5): up to [`TASKS`] tasks that may be suspended at once.
/// `task_live[t]` is a byte; the other three are `i32` arrays indexed by `t * 4`.
const TASK_LIVE_ADDR: u32 = 0x0600;
const TASK_INST_ADDR: u32 = 0x0610;
const TASK_SEG_ADDR: u32 = 0x0620;
const TASK_CLOSE_ADDR: u32 = 0x0630;
const TASKS: i32 = 4;
/// Command templates and frames start here.
const TEMPLATE_ADDR: u32 = 0x1000;
const DATA_PAGES: u32 = 1;
const PAGE: u32 = 65536;

const EV_ONLOAD: u32 = 0;
const EV_ONCLOSE: u32 = 1;

// ── globals ─────────────────────────────────────────────────────────────────
const G_CUR_INST: u32 = 0;
const G_ARGC: u32 = 1;
const G_USED: u32 = 2;
const G_SEG: u32 = 3;
const G_TASK: u32 = 4;
const G_LAST_STATUS: u32 = 5;
const G_QLEN: u32 = 6;
const G_RLEN: u32 = 7;
const G_REQ: u32 = 8;
const G_SPIN_LEFT: u32 = 9;
const G_ACC: u32 = 10;
/// The task the running segment belongs to.
const G_CUR_TASK: u32 = 11;
const GLOBAL_COUNT: u32 = 12;

// ── type indexes ────────────────────────────────────────────────────────────
const T_I32_V: u32 = 0; // (i32) -> ()
const T_I32_I32: u32 = 1; // (i32) -> i32
const T_V_F64: u32 = 2; // () -> f64
const T_V_I32: u32 = 3; // () -> i32
const T_I32I32_I32: u32 = 4; // (i32,i32) -> i32
const T_I32X3_I32: u32 = 5; // (i32,i32,i32) -> i32
const T_I32X4_I32: u32 = 6; // (i32,i32,i32,i32) -> i32

// ── function indexes ────────────────────────────────────────────────────────
const F_FLUSH: u32 = 0;
const F_QUERY: u32 = 1;
const F_REMOTE: u32 = 2;
const F_NOW: u32 = 3;
const F_RANDOM: u32 = 4;
const F_ABORT: u32 = 5;
const F_PR_ABI: u32 = 6;
const F_PR_VARIANT: u32 = 7;
const F_PR_ARENA_BASE: u32 = 8;
const F_PR_INIT: u32 = 9;
const F_PR_OPEN_FORM: u32 = 10;
const F_PR_CLOSE_FORM: u32 = 11;
const F_PR_EVENT: u32 = 12;
const F_PR_RESUME: u32 = 13;
const F_PR_CANCEL: u32 = 14;
const F_PR_TICK: u32 = 15;
const F_PR_STATUS: u32 = 16;
const F_PR_DBG_LIVE: u32 = 17;
const F_PR_DBG_REGION: u32 = 18;
const F_FIND_HANDLER: u32 = 19;
const F_RUN_CURRENT: u32 = 20;
const F_NEW_TASK: u32 = 21;
const F_FIRST_SEGMENT: u32 = 22;

struct Layout {
    multi: bool,
    arena_mem: u32,
    arena_base: u32,
    batch_off: u32,
    batch_size: u32,
    answer_off: u32,
    frame_out_off: u32,
    frame_in_off: u32,
}

impl Layout {
    fn abs(&self, off: u32) -> u32 {
        self.arena_base + off
    }
}

fn arena_arg(l: &Layout, offset: u64) -> MemArg {
    MemArg { offset, align: 0, memory_index: l.arena_mem }
}
fn data_arg(offset: u64) -> MemArg {
    MemArg { offset, align: 0, memory_index: 0 }
}

/// A command template placed in the data memory.
#[derive(Clone)]
struct Template {
    addr: u32,
    len: u32,
    /// Does the command start with a path, so byte 1 is the instance id to patch?
    patch_inst: bool,
}

struct Names {
    names: Vec<String>,
}
impl Names {
    fn id(&mut self, n: &str) -> u32 {
        if let Some(i) = self.names.iter().position(|x| x == n) {
            return i as u32;
        }
        self.names.push(n.to_owned());
        (self.names.len() - 1) as u32
    }
}

struct Builder {
    layout: Layout,
    names: Names,
    events: Vec<String>,
    data: Vec<u8>, // template bytes, from TEMPLATE_ADDR
    funcs: Vec<Function>, // segment functions
    seg_plan: Vec<SegmentPlan>,
}

#[derive(Clone)]
enum End {
    Done,
    Suspend,
    Remote(Template),
    Yield,
    Stop,
    SpinStart { iterations: u32, slice: u32 },
    /// The generated loop segment: runs `slice` iterations, yields until `iterations` are done.
    SpinLoop { slice: u32 },
}

struct SegmentPlan {
    steps: Vec<(Step, Option<Vec<Template>>)>,
    end: End,
    next: Option<usize>,
}

impl Builder {
    fn event_id(&mut self, n: &str) -> u32 {
        if let Some(i) = self.events.iter().position(|x| x == n) {
            return i as u32;
        }
        self.events.push(n.to_owned());
        (self.events.len() - 1) as u32
    }

    fn template(&mut self, bytes: &[u8]) -> Template {
        self.template_with(bytes, true)
    }

    fn template_with(&mut self, bytes: &[u8], patch_inst: bool) -> Template {
        let addr = TEMPLATE_ADDR + self.data.len() as u32;
        self.data.extend_from_slice(bytes);
        // Keep templates 4-byte aligned; nothing relies on it but it keeps dumps readable.
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
        Template { addr, len: bytes.len() as u32, patch_inst }
    }

    fn target_ctrl(&self, form: &FixtureForm, t: &Target) -> u32 {
        match t {
            Target::Form => ctrl_id::FORM,
            Target::Super => ctrl_id::SUPER,
            Target::App => ctrl_id::APP,
            Target::Control(n) => {
                let i = form
                    .controls
                    .iter()
                    .position(|(c, _)| c.eq_ignore_ascii_case(n))
                    .unwrap_or_else(|| panic!("fixture: form `{}` has no control `{n}`", form.name));
                ctrl_id::FIRST_CONTROL + i as u32
            }
        }
    }

    /// Encode a step's command(s) with instance 0; the module patches byte 1 at run time.
    fn commands_for(&mut self, form: &FixtureForm, step: &Step) -> Option<Vec<Template>> {
        let mut w = BatchWriter::new();
        match step {
            Step::Set { target, name, value } => {
                let p = PathSpec::prop(0, self.target_ctrl(form, target), self.names.id(name));
                w.set(&p, value);
            }
            Step::SetAt { target, name, args, value } => {
                let a: Vec<&str> = args.iter().map(String::as_str).collect();
                let p = PathSpec::call(0, self.target_ctrl(form, target), self.names.id(name), &a);
                w.set(&p, value);
            }
            Step::Call { target, name, args } => {
                let a: Vec<&str> = args.iter().map(String::as_str).collect();
                let p = PathSpec::call(0, self.target_ctrl(form, target), self.names.id(name), &a);
                w.call(&p);
            }
            Step::Display(t) => {
                w.display(t);
                return Some(vec![self.template_with(w.as_bytes(), false)]);
            }
            Step::Log { level, text } => {
                w.log(*level, text);
                return Some(vec![self.template_with(w.as_bytes(), false)]);
            }
            Step::Get { target, name } => {
                let p = PathSpec::prop(0, self.target_ctrl(form, target), self.names.id(name));
                w.get(&p);
            }
            Step::CallResult { target, name, args } => {
                let a: Vec<&str> = args.iter().map(String::as_str).collect();
                let p = PathSpec::call(0, self.target_ctrl(form, target), self.names.id(name), &a);
                w.call_result(&p);
            }
            Step::Count { target, name } => {
                let p = PathSpec::prop(0, self.target_ctrl(form, target), self.names.id(name));
                w.count(&p);
            }
            Step::EchoAnswer { target, name } | Step::EchoArg0 { target, name } | Step::EchoFrame { target, name } => {
                // The 6-byte header of `SET`: op, inst, ctrl, nseg=1, name, nargs=0.
                // The text length and bytes are appended by the module.
                let ctrl = self.target_ctrl(form, target);
                let nm = self.names.id(name);
                assert!(ctrl < 128 && nm < 128, "fixture: echo targets need one-byte ids");
                let bytes = [crate::codes::op::SET, 0, ctrl as u8, 1, nm as u8, 0];
                return Some(vec![self.template(&bytes)]);
            }
            Step::Burst { target, name, value, .. } => {
                let p = PathSpec::prop(0, self.target_ctrl(form, target), self.names.id(name));
                w.set(&p, value);
            }
            Step::Error { code, message } => {
                w.error(*code, message, 0, 0, 0, 0);
                return Some(vec![self.template_with(w.as_bytes(), false)]);
            }
            _ => return None,
        }
        Some(vec![self.template(w.as_bytes())])
    }
}

// ── code emission ───────────────────────────────────────────────────────────

struct Asm<'l> {
    f: Function,
    l: &'l Layout,
}

impl<'l> Asm<'l> {
    fn new(l: &'l Layout, locals: &[(u32, ValType)]) -> Self {
        Asm { f: Function::new(locals.iter().copied()), l }
    }
    fn i(&mut self, ins: I<'_>) -> &mut Self {
        self.f.instruction(&ins);
        self
    }
    fn c(&mut self, v: i32) -> &mut Self {
        self.i(I::I32Const(v))
    }
    fn gget(&mut self, g: u32) -> &mut Self {
        self.i(I::GlobalGet(g))
    }
    fn gset(&mut self, g: u32) -> &mut Self {
        self.i(I::GlobalSet(g))
    }
    fn gset_const(&mut self, g: u32, v: i32) -> &mut Self {
        self.c(v).gset(g)
    }

    /// `if used > 0 { flush(used); used = 0 }`
    fn flush_pending(&mut self) {
        self.gget(G_USED);
        self.c(0);
        self.i(I::I32GtU);
        self.i(I::If(BlockType::Empty));
        self.gget(G_USED);
        self.i(I::Call(F_FLUSH));
        self.gset_const(G_USED, 0);
        self.i(I::End);
    }

    /// Make room for `len` more bytes in the batch region, flushing first if needed.
    fn ensure_room(&mut self, len_const: Option<u32>) {
        // used + len > batch_size ?
        self.gget(G_USED);
        match len_const {
            Some(n) => {
                self.c(n as i32);
            }
            None => {
                self.i(I::LocalGet(0)); // dynamic length in local 0
            }
        }
        self.i(I::I32Add);
        self.c(self.l.batch_size as i32);
        self.i(I::I32GtU);
        self.i(I::If(BlockType::Empty));
        self.flush_pending();
        self.i(I::End);
    }

    /// Append the command template to the batch, patching the instance id.
    fn emit_template(&mut self, t: &Template) {
        self.ensure_room(Some(t.len));
        // dst = arena batch + used
        let batch_abs = self.l.abs(self.l.batch_off) as i32;
        self.c(batch_abs);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.c(t.addr as i32);
        self.c(t.len as i32);
        self.i(I::MemoryCopy { src_mem: 0, dst_mem: self.l.arena_mem });
        // patch the instance id at byte 1 of a command that has a path
        if t.patch_inst {
            self.c(batch_abs + 1);
            self.gget(G_USED);
            self.i(I::I32Add);
            self.gget(G_CUR_INST);
            self.i(I::I32Store8(arena_arg(self.l, 0)));
        }
        self.gget(G_USED);
        self.c(t.len as i32);
        self.i(I::I32Add);
        self.gset(G_USED);
    }

    /// Append `SET` header template + text(len bytes copied from `src`), where the
    /// length is in local 0 and the source address (absolute, arena) in local 1.
    fn emit_echo(&mut self, header: &Template) {
        // len in local 0 (<= 16383). Need header(6) + varint(1|2) + len bytes.
        // Reserve worst case: len + 8.
        self.i(I::LocalGet(0));
        self.c(8);
        self.i(I::I32Add);
        self.i(I::LocalSet(2)); // local2 = len + 8
        // if used + local2 > batch_size { flush }
        self.gget(G_USED);
        self.i(I::LocalGet(2));
        self.i(I::I32Add);
        self.c(self.l.batch_size as i32);
        self.i(I::I32GtU);
        self.i(I::If(BlockType::Empty));
        self.flush_pending();
        self.i(I::End);
        let batch_abs = self.l.abs(self.l.batch_off) as i32;
        // copy header
        self.c(batch_abs);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.c(header.addr as i32);
        self.c(header.len as i32);
        self.i(I::MemoryCopy { src_mem: 0, dst_mem: self.l.arena_mem });
        // patch inst
        self.c(batch_abs + 1);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.gget(G_CUR_INST);
        self.i(I::I32Store8(arena_arg(self.l, 0)));
        // varint of len at used+6 (1 or 2 bytes), local3 = bytes taken by the varint
        self.i(I::LocalGet(0));
        self.c(128);
        self.i(I::I32LtU);
        self.i(I::If(BlockType::Empty));
        // one byte
        self.c(batch_abs + 6);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.i(I::LocalGet(0));
        self.i(I::I32Store8(arena_arg(self.l, 0)));
        self.c(1);
        self.i(I::LocalSet(3));
        self.i(I::Else);
        // two bytes: (len & 0x7f) | 0x80 then len >> 7
        self.c(batch_abs + 6);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.i(I::LocalGet(0));
        self.c(0x7f);
        self.i(I::I32And);
        self.c(0x80);
        self.i(I::I32Or);
        self.i(I::I32Store8(arena_arg(self.l, 0)));
        self.c(batch_abs + 7);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.i(I::LocalGet(0));
        self.c(7);
        self.i(I::I32ShrU);
        self.i(I::I32Store8(arena_arg(self.l, 0)));
        self.c(2);
        self.i(I::LocalSet(3));
        self.i(I::End);
        // copy the text: dst = batch + used + 6 + local3
        self.c(batch_abs + 6);
        self.gget(G_USED);
        self.i(I::I32Add);
        self.i(I::LocalGet(3));
        self.i(I::I32Add);
        self.i(I::LocalGet(1)); // src absolute
        self.i(I::LocalGet(0)); // len
        self.i(I::MemoryCopy { src_mem: self.l.arena_mem, dst_mem: self.l.arena_mem });
        // used += 6 + local3 + len
        self.gget(G_USED);
        self.c(6);
        self.i(I::I32Add);
        self.i(I::LocalGet(3));
        self.i(I::I32Add);
        self.i(I::LocalGet(0));
        self.i(I::I32Add);
        self.gset(G_USED);
    }

    fn set_echo_source(&mut self, step: &Step) {
        let l = self.l;
        match step {
            Step::EchoAnswer { .. } => {
                // len = qlen, src = answer region
                self.gget(G_QLEN);
                self.i(I::LocalSet(0));
                self.c(l.abs(l.answer_off) as i32);
                self.i(I::LocalSet(1));
            }
            Step::EchoArg0 { .. } => {
                // the first event argument: varint length (<128) then bytes, at the answer region
                self.c(l.abs(l.answer_off) as i32);
                self.i(I::I32Load8U(arena_arg(l, 0)));
                self.i(I::LocalSet(0));
                self.c(l.abs(l.answer_off) as i32 + 1);
                self.i(I::LocalSet(1));
            }
            Step::EchoFrame { .. } => {
                self.gget(G_RLEN);
                self.i(I::LocalSet(0));
                self.c(l.abs(l.frame_in_off) as i32);
                self.i(I::LocalSet(1));
            }
            _ => unreachable!(),
        }
    }
}

fn emit_step(a: &mut Asm<'_>, step: &Step, tpl: &Option<Vec<Template>>) {
    match step {
        Step::Set { .. } | Step::SetAt { .. } | Step::Call { .. } | Step::Display(_) | Step::Log { .. } => {
            for t in tpl.as_ref().unwrap() {
                a.emit_template(t);
            }
        }
        Step::Get { .. } | Step::CallResult { .. } | Step::Count { .. } => {
            for t in tpl.as_ref().unwrap() {
                a.emit_template(t);
            }
            // The batch now ends with the query: hand it over and keep the answer length.
            a.gget(G_USED);
            a.i(I::Call(F_QUERY));
            a.gset(G_QLEN);
            a.gset_const(G_USED, 0);
        }
        Step::EchoAnswer { .. } | Step::EchoArg0 { .. } | Step::EchoFrame { .. } => {
            a.set_echo_source(step);
            a.emit_echo(&tpl.as_ref().unwrap()[0]);
        }
        Step::Burst { count, .. } => {
            // for i in 0..count { emit }
            a.c(*count as i32);
            a.i(I::LocalSet(0));
            a.i(I::Block(BlockType::Empty));
            a.i(I::Loop(BlockType::Empty));
            a.i(I::LocalGet(0));
            a.i(I::I32Eqz);
            a.i(I::BrIf(1));
            a.emit_template(&tpl.as_ref().unwrap()[0]);
            a.i(I::LocalGet(0));
            a.c(1);
            a.i(I::I32Sub);
            a.i(I::LocalSet(0));
            a.i(I::Br(0));
            a.i(I::End);
            a.i(I::End);
        }
        Step::Error { .. } => {
            a.emit_template(&tpl.as_ref().unwrap()[0]);
            a.gget(G_USED);
            a.i(I::Call(F_ABORT));
            a.gset_const(G_USED, 0);
            a.i(I::Unreachable);
        }
        Step::Trap => {
            a.i(I::Unreachable);
        }
        Step::Clock => {
            a.i(I::Call(F_NOW));
            a.i(I::Drop);
            a.i(I::Call(F_RANDOM));
            a.i(I::Drop);
        }
        // boundary steps are handled by the segment end
        Step::Remote(_) | Step::Suspend | Step::Yield | Step::Stop | Step::Spin { .. } => unreachable!("boundary inside a segment"),
    }
}

fn build_segment(l: &Layout, plan: &SegmentPlan, seg_index: usize, first_segment_func: u32) -> Function {
    // locals: 0 len/count, 1 src, 2 scratch, 3 varint bytes
    let mut a = Asm::new(l, &[(4, ValType::I32)]);
    match &plan.end {
        End::SpinLoop { slice } => {
            // n = min(slice, spin_left)
            a.c(*slice as i32);
            a.i(I::LocalSet(0));
            a.gget(G_SPIN_LEFT);
            a.i(I::LocalGet(0));
            a.i(I::I32LtU);
            a.i(I::If(BlockType::Empty));
            a.gget(G_SPIN_LEFT);
            a.i(I::LocalSet(0));
            a.i(I::End);
            // loop n times: acc += i
            a.c(0);
            a.i(I::LocalSet(1));
            a.i(I::Block(BlockType::Empty));
            a.i(I::Loop(BlockType::Empty));
            a.i(I::LocalGet(1));
            a.i(I::LocalGet(0));
            a.i(I::I32GeU);
            a.i(I::BrIf(1));
            a.gget(G_ACC);
            a.i(I::LocalGet(1));
            a.i(I::I32Add);
            a.gset(G_ACC);
            a.i(I::LocalGet(1));
            a.c(1);
            a.i(I::I32Add);
            a.i(I::LocalSet(1));
            a.i(I::Br(0));
            a.i(I::End);
            a.i(I::End);
            // spin_left -= n
            a.gget(G_SPIN_LEFT);
            a.i(I::LocalGet(0));
            a.i(I::I32Sub);
            a.gset(G_SPIN_LEFT);
            a.gget(G_SPIN_LEFT);
            a.c(0);
            a.i(I::I32GtU);
            a.i(I::If(BlockType::Empty));
            a.gset_const(G_SEG, seg_index as i32);
            a.c(status::YIELDED);
            a.i(I::Return);
            a.i(I::End);
            // done: tail-call the next segment
            let next = plan.next.expect("a spin loop has a next segment");
            a.gset_const(G_SEG, next as i32);
            a.i(I::Call(first_segment_func + next as u32));
            a.i(I::End);
            return a.f;
        }
        _ => {}
    }
    for (step, tpl) in &plan.steps {
        emit_step(&mut a, step, tpl);
    }
    // the end of the segment
    match &plan.end {
        End::Done => {
            a.flush_pending();
            a.gset_const(G_SEG, -1);
            a.c(status::DONE);
        }
        End::Suspend => {
            a.flush_pending();
            a.gset_const(G_SEG, plan.next.unwrap() as i32);
            a.c(status::SUSPENDED);
        }
        End::Remote(t) => {
            a.flush_pending();
            // copy the frame to frame-out, then ask the host to send it
            a.c(l.abs(l.frame_out_off) as i32);
            a.c(t.addr as i32);
            a.c(t.len as i32);
            a.i(I::MemoryCopy { src_mem: 0, dst_mem: l.arena_mem });
            a.c(t.len as i32);
            a.i(I::Call(F_REMOTE));
            a.gset(G_REQ);
            a.gset_const(G_SEG, plan.next.unwrap() as i32);
            a.c(status::SUSPENDED);
        }
        End::Yield => {
            a.flush_pending();
            a.gset_const(G_SEG, plan.next.unwrap() as i32);
            a.c(status::YIELDED);
        }
        End::Stop => {
            a.flush_pending();
            a.gset_const(G_SEG, -1);
            a.c(status::STOPPED);
        }
        End::SpinStart { iterations, .. } => {
            a.flush_pending();
            a.gset_const(G_SPIN_LEFT, *iterations as i32);
            a.gset_const(G_SEG, plan.next.unwrap() as i32); // the generated loop segment
            a.c(status::YIELDED);
        }
        End::SpinLoop { .. } => unreachable!(),
    }
    a.i(I::End);
    a.f
}

fn simple(l: &Layout, build: impl FnOnce(&mut Asm<'_>)) -> Function {
    let mut a = Asm::new(l, &[]);
    build(&mut a);
    a.i(I::End);
    a.f
}

/// Build the module for `spec`.
pub fn build(spec: &FixtureSpec) -> Fixture {
    let limits = spec.limits.clone();
    let arena_pages = limits.arena_size.div_ceil(PAGE);
    let l = if spec.multi_memory {
        Layout {
            multi: true,
            arena_mem: 1,
            arena_base: 0,
            batch_off: 0,
            batch_size: limits.batch_size,
            answer_off: limits.batch_size,
            frame_out_off: limits.batch_size + limits.answer_size,
            frame_in_off: limits.batch_size + limits.answer_size + limits.frame_size,
        }
    } else {
        Layout {
            multi: false,
            arena_mem: 0,
            arena_base: DATA_PAGES * PAGE,
            batch_off: 0,
            batch_size: limits.batch_size,
            answer_off: limits.batch_size,
            frame_out_off: limits.batch_size + limits.answer_size,
            frame_in_off: limits.batch_size + limits.answer_size + limits.frame_size,
        }
    };
    let mut b = Builder {
        layout: l,
        names: Names { names: Vec::new() },
        events: vec!["onLoad".to_owned(), "onClose".to_owned()],
        data: Vec::new(),
        funcs: Vec::new(),
        seg_plan: Vec::new(),
    };

    // ── plan the segments of every handler ─────────────────────────────────
    let mut handler_rows: Vec<(u32, u32, u32, u32)> = Vec::new(); // form, ctrl, event, first seg
    let mut manifest_forms: Vec<FormEntry> = Vec::new();
    for (fi, form) in spec.forms.iter().enumerate() {
        let mut entry_handlers = Vec::new();
        for h in &form.handlers {
            let ctrl = match &h.control {
                None => ctrl_id::FORM,
                Some(n) => b.target_ctrl(form, &Target::Control(n.clone())),
            };
            let ev = b.event_id(&h.event);
            let first = b.seg_plan.len();
            // cut at boundaries
            let mut current: Vec<(Step, Option<Vec<Template>>)> = Vec::new();
                    let steps = h.steps.clone();
            let mut idx = first;
            for step in steps {
                let boundary = match &step {
                    Step::Remote(frame) => {
                        let t = b.template(frame);
                        Some(End::Remote(t))
                    }
                    Step::Suspend => Some(End::Suspend),
                    Step::Yield => Some(End::Yield),
                    Step::Stop => Some(End::Stop),
                    Step::Spin { iterations, slice } => Some(End::SpinStart { iterations: *iterations, slice: *slice }),
                    _ => None,
                };
                match boundary {
                    Some(end) => {
                        let spin_slice = if let End::SpinStart { slice, .. } = &end { Some(*slice) } else { None };
                        let is_stop = matches!(end, End::Stop);
                        b.seg_plan.push(SegmentPlan { steps: std::mem::take(&mut current), end, next: if is_stop { None } else { Some(idx + 1) } });
                        idx += 1;
                        if let Some(slice) = spin_slice {
                            // the generated loop segment sits between; its `next` is the segment after it
                            b.seg_plan.push(SegmentPlan { steps: Vec::new(), end: End::SpinLoop { slice }, next: Some(idx + 1) });
                            idx += 1;
                        }
                    }
                    None => {
                        let tpl = b.commands_for(form, &step);
                        current.push((step, tpl));
                    }
                }
            }
            b.seg_plan.push(SegmentPlan { steps: current, end: End::Done, next: None });
            handler_rows.push((fi as u32, ctrl, ev, first as u32));
            entry_handlers.push(HandlerEntry { ctrl, event: ev, entry: first as u32 });
        }
        manifest_forms.push(FormEntry {
            id: fi as u32,
            name: form.name.clone(),
            main: form.main,
            instances: form.instances,
            region_size: 64,
            controls: form
                .controls
                .iter()
                .enumerate()
                .map(|(i, (n, t))| ControlEntry { id: ctrl_id::FIRST_CONTROL + i as u32, name: n.clone(), kind: t.clone() })
                .collect(),
            handlers: entry_handlers,
        });
    }
    assert!(b.seg_plan.len() < 250, "fixture: too many segments for the one-byte handler table");
    // The handler table is in the module's data: load it from the rows now that all event ids exist.
    let total_instances: u32 = spec.forms.iter().map(|f| f.instances).sum();
    assert!(total_instances <= 120, "fixture: at most 120 instances");
    assert!(spec.forms.len() <= 32);

    // ── emit the segment functions ─────────────────────────────────────────
    let nseg = b.seg_plan.len();
    let first_seg_func = F_FIRST_SEGMENT;
    let mut seg_funcs: Vec<Function> = Vec::new();
    for (i, plan) in b.seg_plan.iter().enumerate() {
        seg_funcs.push(build_segment(&b.layout, plan, i, first_seg_func));
    }
    b.funcs = seg_funcs;

    let l = &b.layout;
    // ── data segments ──────────────────────────────────────────────────────
    let mut data = DataSection::new();
    let form_max: Vec<u8> = spec.forms.iter().map(|f| f.instances as u8).collect();
    data.active(0, &ConstExpr::i32_const(FORM_MAX_ADDR as i32), form_max);
    let mut table = Vec::new();
    for (f, c, e, s) in &handler_rows {
        table.extend_from_slice(&[*f as u8, *c as u8, *e as u8, *s as u8]);
    }
    assert!(table.len() < (TEMPLATE_ADDR - HANDLER_TABLE_ADDR) as usize, "fixture: handler table overflow");
    if !table.is_empty() {
        data.active(0, &ConstExpr::i32_const(HANDLER_TABLE_ADDR as i32), table);
    }
    assert!(TEMPLATE_ADDR as usize + b.data.len() < (DATA_PAGES * PAGE) as usize, "fixture: templates overflow the data page");
    if !b.data.is_empty() {
        data.active(0, &ConstExpr::i32_const(TEMPLATE_ADDR as i32), b.data.clone());
    }

    // ── types ──────────────────────────────────────────────────────────────
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []);
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([], [ValType::F64]);
    types.ty().function([], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I32, ValType::I32, ValType::I32], [ValType::I32]);

    let mut imports = ImportSection::new();
    imports.import("pr", "flush", EntityType::Function(T_I32_V));
    imports.import("pr", "query", EntityType::Function(T_I32_I32));
    imports.import("pr", "remote", EntityType::Function(T_I32_I32));
    imports.import("pr", "now", EntityType::Function(T_V_F64));
    imports.import("pr", "random", EntityType::Function(T_V_F64));
    imports.import("pr", "abort", EntityType::Function(T_I32_V));

    // function section: types of the defined functions, in index order
    let mut funcs = FunctionSection::new();
    funcs.function(T_V_I32); // pr_abi
    funcs.function(T_V_I32); // pr_variant
    funcs.function(T_V_I32); // pr_arena_base
    funcs.function(T_V_I32); // pr_init
    funcs.function(T_I32I32_I32); // pr_open_form(form, opener)
    funcs.function(T_I32_I32); // pr_close_form(inst)
    funcs.function(T_I32X4_I32); // pr_event
    funcs.function(T_I32I32_I32); // pr_resume(task, len)
    funcs.function(T_I32_V); // pr_cancel
    funcs.function(T_V_I32); // pr_tick
    funcs.function(T_V_I32); // pr_status
    funcs.function(T_V_I32); // pr_dbg_live
    funcs.function(T_V_I32); // pr_dbg_region_used
    funcs.function(T_I32X3_I32); // find_handler(form, ctrl, event)
    funcs.function(T_V_I32); // run_current
    funcs.function(T_I32I32_I32); // new_task(inst, closing_inst)
    for _ in 0..nseg {
        funcs.function(T_V_I32);
    }

    let mut table_sec = TableSection::new();
    table_sec.table(TableType { element_type: RefType::FUNCREF, table64: false, minimum: nseg as u64, maximum: Some(nseg as u64), shared: false });

    let mut mems = MemorySection::new();
    let manifest_variant_multi = spec.multi_memory;
    if spec.multi_memory {
        mems.memory(MemoryType { minimum: DATA_PAGES as u64, maximum: Some(DATA_PAGES as u64), memory64: false, shared: false, page_size_log2: None });
        mems.memory(MemoryType { minimum: arena_pages as u64, maximum: Some(arena_pages as u64), memory64: false, shared: false, page_size_log2: None });
    } else {
        let total = (DATA_PAGES + arena_pages) as u64;
        mems.memory(MemoryType { minimum: total, maximum: Some(total), memory64: false, shared: false, page_size_log2: None });
    }

    let mut globals = GlobalSection::new();
    for i in 0..GLOBAL_COUNT {
        let init = match i {
            G_CUR_INST | G_SEG | G_TASK | G_CUR_TASK => -1,
            _ => 0,
        };
        globals.global(GlobalType { val_type: ValType::I32, mutable: true, shared: false }, &ConstExpr::i32_const(init));
    }

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("arena", ExportKind::Memory, if spec.multi_memory { 1 } else { 0 });
    for (name, idx) in [
        ("pr_abi", F_PR_ABI),
        ("pr_variant", F_PR_VARIANT),
        ("pr_arena_base", F_PR_ARENA_BASE),
        ("pr_init", F_PR_INIT),
        ("pr_open_form", F_PR_OPEN_FORM),
        ("pr_close_form", F_PR_CLOSE_FORM),
        ("pr_event", F_PR_EVENT),
        ("pr_resume", F_PR_RESUME),
        ("pr_cancel", F_PR_CANCEL),
        ("pr_tick", F_PR_TICK),
        ("pr_status", F_PR_STATUS),
        ("pr_dbg_live", F_PR_DBG_LIVE),
        ("pr_dbg_region_used", F_PR_DBG_REGION),
    ] {
        exports.export(name, ExportKind::Func, idx);
    }

    let mut elems = ElementSection::new();
    let seg_indexes: Vec<u32> = (0..nseg as u32).map(|i| F_FIRST_SEGMENT + i).collect();
    elems.active(Some(0), &ConstExpr::i32_const(0), Elements::Functions(std::borrow::Cow::Owned(seg_indexes)));

    // ── the fixed functions ────────────────────────────────────────────────
    let mut code = CodeSection::new();
    let nforms = spec.forms.len() as i32;
    let total = total_instances as i32;
    let multi_bit = if manifest_variant_multi { crate::codes::variant::MULTI_MEMORY } else { 0 };
    code.function(&simple(l, |a| {
        a.c(1);
    })); // pr_abi
    code.function(&simple(l, |a| {
        a.c(multi_bit);
    })); // pr_variant (bit 0 = multi-memory)
    code.function(&simple(l, |a| {
        a.c(l.arena_base as i32);
    })); // pr_arena_base
    code.function(&simple(l, |a| {
        a.gset_const(G_CUR_INST, -1);
        a.gset_const(G_TASK, -1);
        a.gset_const(G_USED, 0);
        a.c(0);
    })); // pr_init

    // pr_open_form(form, opener) -> inst | -3 | -4
    code.function(&{
        let mut a = Asm::new(l, &[(3, ValType::I32)]); // 2 id, 3 handler, 4 scratch
        // form out of range
        a.i(I::LocalGet(0));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::LocalGet(0));
        a.c(nforms);
        a.i(I::I32GeS);
        a.i(I::I32Or);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        // form_live[form] >= form_max[form] ?
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(FORM_LIVE_ADDR as u64)));
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(FORM_MAX_ADDR as u64)));
        a.i(I::I32GeU);
        a.i(I::If(BlockType::Empty));
        a.c(err::NO_FREE_INSTANCE);
        a.i(I::Return);
        a.i(I::End);
        // find the lowest free id
        a.c(0);
        a.i(I::LocalSet(2));
        a.i(I::Block(BlockType::Empty));
        a.i(I::Loop(BlockType::Empty));
        a.i(I::LocalGet(2));
        a.c(total);
        a.i(I::I32GeU);
        a.i(I::BrIf(1));
        a.i(I::LocalGet(2));
        a.i(I::I32Load8U(data_arg(LIVE_ADDR as u64)));
        a.i(I::I32Eqz);
        a.i(I::BrIf(1));
        a.i(I::LocalGet(2));
        a.c(1);
        a.i(I::I32Add);
        a.i(I::LocalSet(2));
        a.i(I::Br(0));
        a.i(I::End);
        a.i(I::End);
        a.i(I::LocalGet(2));
        a.c(total);
        a.i(I::I32GeU);
        a.i(I::If(BlockType::Empty));
        a.c(err::NO_FREE_INSTANCE);
        a.i(I::Return);
        a.i(I::End);
        // live[id] = 1; inst_form[id] = form; form_live[form] += 1
        a.i(I::LocalGet(2));
        a.c(1);
        a.i(I::I32Store8(data_arg(LIVE_ADDR as u64)));
        a.i(I::LocalGet(2));
        a.i(I::LocalGet(0));
        a.i(I::I32Store8(data_arg(INST_FORM_ADDR as u64)));
        a.i(I::LocalGet(0));
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(FORM_LIVE_ADDR as u64)));
        a.c(1);
        a.i(I::I32Add);
        a.i(I::I32Store8(data_arg(FORM_LIVE_ADDR as u64)));
        a.i(I::LocalGet(2));
        a.gset(G_CUR_INST);
        // onLoad handler?
        a.i(I::LocalGet(0));
        a.c(ctrl_id::FORM as i32);
        a.c(EV_ONLOAD as i32);
        a.i(I::Call(F_FIND_HANDLER));
        a.i(I::LocalSet(3));
        a.i(I::LocalGet(3));
        a.c(0);
        a.i(I::I32GeS);
        a.i(I::If(BlockType::Empty));
        a.i(I::LocalGet(2));
        a.c(-1);
        a.i(I::Call(F_NEW_TASK));
        a.i(I::Drop);
        a.i(I::LocalGet(3));
        a.gset(G_SEG);
        a.i(I::Call(F_RUN_CURRENT));
        a.i(I::Drop);
        a.i(I::Else);
        a.gset_const(G_LAST_STATUS, 0);
        a.i(I::End);
        a.i(I::LocalGet(2));
        a.i(I::End);
        a.f
    }); // pr_open_form

    // pr_close_form(inst) -> status
    code.function(&{
        let mut a = Asm::new(l, &[(1, ValType::I32)]); // 1 handler
        a.i(I::LocalGet(0));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::LocalGet(0));
        a.c(total);
        a.i(I::I32GeS);
        a.i(I::I32Or);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(LIVE_ADDR as u64)));
        a.i(I::I32Eqz);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.gset(G_CUR_INST);
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(INST_FORM_ADDR as u64)));
        a.c(ctrl_id::FORM as i32);
        a.c(EV_ONCLOSE as i32);
        a.i(I::Call(F_FIND_HANDLER));
        a.i(I::LocalSet(1));
        a.i(I::LocalGet(1));
        a.c(0);
        a.i(I::I32GeS);
        a.i(I::If(BlockType::Result(ValType::I32)));
        a.i(I::LocalGet(0));
        a.i(I::LocalGet(0));
        a.i(I::Call(F_NEW_TASK));
        a.i(I::Drop);
        a.i(I::LocalGet(1));
        a.gset(G_SEG);
        a.i(I::Call(F_RUN_CURRENT));
        a.i(I::Else);
        free_instance(&mut a, 0);
        a.gset_const(G_LAST_STATUS, 0);
        a.c(0);
        a.i(I::End);
        a.i(I::End);
        a.f
    }); // pr_close_form

    // pr_event(inst, ctrl, event, argc) -> status
    code.function(&{
        let mut a = Asm::new(l, &[(1, ValType::I32)]); // 4 handler
        a.i(I::LocalGet(0));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::LocalGet(0));
        a.c(total);
        a.i(I::I32GeS);
        a.i(I::I32Or);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(LIVE_ADDR as u64)));
        a.i(I::I32Eqz);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.gset(G_CUR_INST);
        a.i(I::LocalGet(3));
        a.gset(G_ARGC);
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(INST_FORM_ADDR as u64)));
        a.i(I::LocalGet(1));
        a.i(I::LocalGet(2));
        a.i(I::Call(F_FIND_HANDLER));
        a.i(I::LocalSet(4));
        a.i(I::LocalGet(4));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.c(-1);
        a.i(I::Call(F_NEW_TASK));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::If(BlockType::Empty));
        a.c(err::GENERIC); // all tasks are in use
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(4));
        a.gset(G_SEG);
        a.i(I::Call(F_RUN_CURRENT));
        a.i(I::End);
        a.f
    }); // pr_event

    // pr_resume(task, len) -> status
    code.function(&{
        let mut a = Asm::new(l, &[]);
        // an unknown or finished task
        a.i(I::LocalGet(0));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::LocalGet(0));
        a.c(TASKS);
        a.i(I::I32GeS);
        a.i(I::I32Or);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(TASK_LIVE_ADDR as u64)));
        a.i(I::I32Eqz);
        a.i(I::If(BlockType::Empty));
        a.c(err::UNKNOWN_TARGET);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.gset(G_CUR_TASK);
        a.i(I::LocalGet(0));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::I32Load(data_arg(TASK_SEG_ADDR as u64)));
        a.gset(G_SEG);
        a.i(I::LocalGet(0));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::I32Load(data_arg(TASK_INST_ADDR as u64)));
        a.gset(G_CUR_INST);
        a.i(I::LocalGet(1));
        a.gset(G_RLEN);
        a.i(I::Call(F_RUN_CURRENT));
        a.i(I::End);
        a.f
    });
    // pr_cancel(task): -1 discards the run that trapped (ABI A5)
    code.function(&{
        let mut a = Asm::new(l, &[(2, ValType::I32)]); // 1 = t, 2 = closing inst
        a.i(I::LocalGet(0));
        a.c(0);
        a.i(I::I32LtS);
        a.i(I::If(BlockType::Result(ValType::I32)));
        a.gget(G_CUR_TASK);
        a.i(I::Else);
        a.i(I::LocalGet(0));
        a.i(I::End);
        a.i(I::LocalSet(1));
        a.i(I::LocalGet(1));
        a.c(0);
        a.i(I::I32GeS);
        a.i(I::LocalGet(1));
        a.c(TASKS);
        a.i(I::I32LtS);
        a.i(I::I32And);
        a.i(I::If(BlockType::Empty));
        a.i(I::LocalGet(1));
        a.i(I::I32Load8U(data_arg(TASK_LIVE_ADDR as u64)));
        a.i(I::If(BlockType::Empty));
        // a cancelled close still releases the instance
        a.i(I::LocalGet(1));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::I32Load(data_arg(TASK_CLOSE_ADDR as u64)));
        a.i(I::LocalSet(2));
        a.i(I::LocalGet(2));
        a.c(0);
        a.i(I::I32GeS);
        a.i(I::If(BlockType::Empty));
        free_instance(&mut a, 2);
        a.i(I::End);
        a.i(I::LocalGet(1));
        a.c(0);
        a.i(I::I32Store8(data_arg(TASK_LIVE_ADDR as u64)));
        a.i(I::End);
        a.i(I::End);
        a.gset_const(G_TASK, -1);
        a.gset_const(G_SEG, -1);
        a.gset_const(G_USED, 0);
        a.i(I::End);
        a.f
    });
    // pr_tick() -> task
    code.function(&simple(l, |a| {
        a.gget(G_TASK);
    }));
    // pr_status()
    code.function(&simple(l, |a| {
        a.gget(G_LAST_STATUS);
    }));
    // pr_dbg_live(): sum of live[id]
    code.function(&{
        let mut a = Asm::new(l, &[(2, ValType::I32)]); // 0 id, 1 sum
        a.c(0);
        a.i(I::LocalSet(0));
        a.i(I::Block(BlockType::Empty));
        a.i(I::Loop(BlockType::Empty));
        a.i(I::LocalGet(0));
        a.c(total);
        a.i(I::I32GeU);
        a.i(I::BrIf(1));
        a.i(I::LocalGet(1));
        a.i(I::LocalGet(0));
        a.i(I::I32Load8U(data_arg(LIVE_ADDR as u64)));
        a.i(I::I32Add);
        a.i(I::LocalSet(1));
        a.i(I::LocalGet(0));
        a.c(1);
        a.i(I::I32Add);
        a.i(I::LocalSet(0));
        a.i(I::Br(0));
        a.i(I::End);
        a.i(I::End);
        a.i(I::LocalGet(1));
        a.i(I::End);
        a.f
    });
    // pr_dbg_region_used(): 64 bytes per live instance
    code.function(&simple(l, |a| {
        a.i(I::Call(F_PR_DBG_LIVE));
        a.c(64);
        a.i(I::I32Mul);
    }));
    // find_handler(form, ctrl, event) -> first segment | -1
    code.function(&{
        let mut a = Asm::new(l, &[(2, ValType::I32)]); // 3 i, 4 addr
        a.c(0);
        a.i(I::LocalSet(3));
        a.i(I::Block(BlockType::Empty));
        a.i(I::Loop(BlockType::Empty));
        a.i(I::LocalGet(3));
        a.c(handler_rows.len() as i32);
        a.i(I::I32GeU);
        a.i(I::BrIf(1));
        // addr = HANDLER_TABLE_ADDR + i*4
        a.i(I::LocalGet(3));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::LocalSet(4));
        a.i(I::LocalGet(4));
        a.i(I::I32Load8U(data_arg(HANDLER_TABLE_ADDR as u64)));
        a.i(I::LocalGet(0));
        a.i(I::I32Eq);
        a.i(I::LocalGet(4));
        a.i(I::I32Load8U(data_arg(HANDLER_TABLE_ADDR as u64 + 1)));
        a.i(I::LocalGet(1));
        a.i(I::I32Eq);
        a.i(I::I32And);
        a.i(I::LocalGet(4));
        a.i(I::I32Load8U(data_arg(HANDLER_TABLE_ADDR as u64 + 2)));
        a.i(I::LocalGet(2));
        a.i(I::I32Eq);
        a.i(I::I32And);
        a.i(I::If(BlockType::Empty));
        a.i(I::LocalGet(4));
        a.i(I::I32Load8U(data_arg(HANDLER_TABLE_ADDR as u64 + 3)));
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(3));
        a.c(1);
        a.i(I::I32Add);
        a.i(I::LocalSet(3));
        a.i(I::Br(0));
        a.i(I::End);
        a.i(I::End);
        a.c(-1);
        a.i(I::End);
        a.f
    });
    // run_current() -> status: call table[seg](), record the status and the task
    code.function(&{
        let mut a = Asm::new(l, &[(2, ValType::I32)]); // 0 = status, 1 = closing instance
        a.gget(G_SEG);
        a.i(I::CallIndirect { type_index: T_V_I32, table_index: 0 });
        a.i(I::LocalSet(0));
        a.i(I::LocalGet(0));
        a.gset(G_LAST_STATUS);
        // A task left suspended or yielded keeps its next segment; a finished one frees its slot.
        a.i(I::LocalGet(0));
        a.c(status::SUSPENDED);
        a.i(I::I32Eq);
        a.i(I::LocalGet(0));
        a.c(status::YIELDED);
        a.i(I::I32Eq);
        a.i(I::I32Or);
        a.i(I::If(BlockType::Empty));
        // task_seg[cur] = seg; the id pr_tick reports
        a.gget(G_CUR_TASK);
        a.c(4);
        a.i(I::I32Mul);
        a.gget(G_SEG);
        a.i(I::I32Store(data_arg(TASK_SEG_ADDR as u64)));
        a.gget(G_CUR_TASK);
        a.gset(G_TASK);
        a.i(I::Else);
        a.gset_const(G_TASK, -1);
        a.gget(G_CUR_TASK);
        a.c(0);
        a.i(I::I32Store8(data_arg(TASK_LIVE_ADDR as u64)));
        // a finished close releases the instance
        a.gget(G_CUR_TASK);
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::I32Load(data_arg(TASK_CLOSE_ADDR as u64)));
        a.i(I::LocalSet(1));
        a.i(I::LocalGet(1));
        a.c(0);
        a.i(I::I32GeS);
        a.i(I::If(BlockType::Empty));
        free_instance(&mut a, 1);
        a.gget(G_CUR_TASK);
        a.c(4);
        a.i(I::I32Mul);
        a.c(-1);
        a.i(I::I32Store(data_arg(TASK_CLOSE_ADDR as u64)));
        a.i(I::End);
        a.i(I::End);
        a.i(I::LocalGet(0));
        a.i(I::End);
        a.f
    });
    // new_task(inst, closing_inst) -> slot | -1
    code.function(&{
        let mut a = Asm::new(l, &[(1, ValType::I32)]); // 2 = t
        a.c(0);
        a.i(I::LocalSet(2));
        a.i(I::Block(BlockType::Empty));
        a.i(I::Loop(BlockType::Empty));
        a.i(I::LocalGet(2));
        a.c(TASKS);
        a.i(I::I32GeU);
        a.i(I::BrIf(1));
        a.i(I::LocalGet(2));
        a.i(I::I32Load8U(data_arg(TASK_LIVE_ADDR as u64)));
        a.i(I::I32Eqz);
        a.i(I::BrIf(1));
        a.i(I::LocalGet(2));
        a.c(1);
        a.i(I::I32Add);
        a.i(I::LocalSet(2));
        a.i(I::Br(0));
        a.i(I::End);
        a.i(I::End);
        a.i(I::LocalGet(2));
        a.c(TASKS);
        a.i(I::I32GeU);
        a.i(I::If(BlockType::Empty));
        a.c(-1);
        a.i(I::Return);
        a.i(I::End);
        a.i(I::LocalGet(2));
        a.c(1);
        a.i(I::I32Store8(data_arg(TASK_LIVE_ADDR as u64)));
        a.i(I::LocalGet(2));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::LocalGet(0));
        a.i(I::I32Store(data_arg(TASK_INST_ADDR as u64)));
        a.i(I::LocalGet(2));
        a.c(4);
        a.i(I::I32Mul);
        a.i(I::LocalGet(1));
        a.i(I::I32Store(data_arg(TASK_CLOSE_ADDR as u64)));
        a.i(I::LocalGet(2));
        a.gset(G_CUR_TASK);
        a.i(I::LocalGet(0));
        a.gset(G_CUR_INST);
        a.i(I::LocalGet(2));
        a.i(I::End);
        a.f
    });
    for f in &b.funcs {
        code.function(f);
    }

    // ── the manifest, embedded and returned ────────────────────────────────
    let manifest = Manifest {
        abi: crate::codes::ABI_VERSION,
        generator: "cobolt-wasm-abi fixture".to_owned(),
        project: spec.project.clone(),
        variant: VariantInfo { multi_memory: spec.multi_memory, conformance: false },
        limits: Limits { data_pages: DATA_PAGES, ..limits },
        names: b.names.names.clone(),
        events: b.events.clone(),
        forms: manifest_forms,
        builtins: Vec::new(),
        imports: crate::codes::PRODUCTION_IMPORTS.iter().map(|s| (*s).to_owned()).collect(),
        sizes: Default::default(),
    };
    manifest.validate().expect("the fixture's own manifest is valid");
    let manifest_json = serde_json::to_string(&manifest).unwrap();

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&funcs);
    module.section(&table_sec);
    module.section(&mems);
    module.section(&globals);
    module.section(&exports);
    module.section(&elems);
    module.section(&code);
    module.section(&data);
    module.section(&CustomSection { name: std::borrow::Cow::Borrowed(MANIFEST_SECTION), data: std::borrow::Cow::Owned(manifest_json.into_bytes()) });
    let wasm = module.finish();
    let _ = b.layout.multi;
    Fixture { wasm, manifest }
}

/// Free instance `local` (a local index holding the id): live[id] = 0, form_live[form] -= 1.
fn free_instance(a: &mut Asm<'_>, local: u32) {
    // form_live[inst_form[id]] -= 1
    a.i(I::LocalGet(local));
    a.i(I::I32Load8U(data_arg(INST_FORM_ADDR as u64)));
    a.i(I::LocalGet(local));
    a.i(I::I32Load8U(data_arg(INST_FORM_ADDR as u64)));
    a.i(I::I32Load8U(data_arg(FORM_LIVE_ADDR as u64)));
    a.c(1);
    a.i(I::I32Sub);
    a.i(I::I32Store8(data_arg(FORM_LIVE_ADDR as u64)));
    a.i(I::LocalGet(local));
    a.c(0);
    a.i(I::I32Store8(data_arg(LIVE_ADDR as u64)));
}

// ── ready-made scenarios ────────────────────────────────────────────────────

/// The names of the controls and events of [`standard_spec`], for tests to refer to.
pub mod standard {
    pub const MAIN_FORM: &str = "main-form";
    pub const CHILD_FORM: &str = "child-form";
    pub const LBL_OUT: &str = "LBL-OUT";
    pub const TXT_IN: &str = "TXT-IN";
    pub const LST_1: &str = "LST-1";
    /// What `BTN-GO` writes: ASCII, accents, CJK and an emoji (UTF-8 of 1 to 4 bytes).
    pub const GREETING: &str = "Olá, mundo! 日本語 😀";
    pub const SPIN_ITERATIONS: u32 = 3_000_000;
    pub const SPIN_SLICE: u32 = 100_000;
    pub const BURST_COUNT: u32 = 1000;
}

/// A two-form fixture that exercises every ABI path.
///
/// `main-form` (1 instance): a Label, a TextBox, a ListBox and one button per
/// scenario — click (`BTN-GO`), query (`BTN-QUERY`), remote request (`BTN-REMOTE`),
/// yield (`BTN-YIELD`), trap (`BTN-TRAP`), burst of 1000 writes (`BTN-BURST`),
/// echo of the first event argument (`BTN-ECHO`), COBOL run-time error (`BTN-ERR`),
/// `STOP RUN` (`BTN-STOP`), a second event while suspended is queued (`BTN-SUSPEND`),
/// and an `onLoad` that writes the label. `child-form` (3 instances) has one label
/// and an `onLoad` that writes it, so opening and closing it exercises the
/// instance lifecycle.
pub fn standard_spec(multi_memory: bool) -> FixtureSpec {
    use standard::*;
    let t = Target::control;
    let out = || t(LBL_OUT);
    FixtureSpec {
        project: "fixture".into(),
        multi_memory,
        limits: Limits::default(),
        forms: vec![
            FixtureForm {
                name: MAIN_FORM.into(),
                main: true,
                instances: 1,
                controls: vec![
                    (LBL_OUT.into(), "Label".into()),
                    (TXT_IN.into(), "TextBox".into()),
                    (LST_1.into(), "ListBox".into()),
                    ("BTN-GO".into(), "Button".into()),
                    ("BTN-QUERY".into(), "Button".into()),
                    ("BTN-REMOTE".into(), "Button".into()),
                    ("BTN-YIELD".into(), "Button".into()),
                    ("BTN-TRAP".into(), "Button".into()),
                    ("BTN-BURST".into(), "Button".into()),
                    ("BTN-ECHO".into(), "Button".into()),
                    ("BTN-ERR".into(), "Button".into()),
                    ("BTN-STOP".into(), "Button".into()),
                    ("BTN-SUSPEND".into(), "Button".into()),
                    ("BTN-OPEN".into(), "Button".into()),
                    ("BTN-MODAL".into(), "Button".into()),
                    ("BTN-RAW".into(), "Button".into()),
                ],
                handlers: vec![
                    FixtureHandler::form("onLoad", vec![Step::Set { target: out(), name: "Caption".into(), value: "loaded".into() }]),
                    FixtureHandler::on(
                        "BTN-GO",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: GREETING.into() },
                            Step::Call { target: t(LST_1), name: "AddItem".into(), args: vec!["um".into()] },
                            Step::Display("go".into()),
                            Step::Clock,
                        ],
                    ),
                    FixtureHandler::on(
                        "BTN-QUERY",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: "before".into() },
                            Step::Get { target: t(TXT_IN), name: "Text".into() },
                            Step::EchoAnswer { target: out(), name: "Caption".into() },
                        ],
                    ),
                    FixtureHandler::on(
                        "BTN-REMOTE",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: "calling".into() },
                            Step::Remote(br#"{"program":"PING","payload":{}}"#.to_vec()),
                            Step::EchoFrame { target: out(), name: "Caption".into() },
                        ],
                    ),
                    FixtureHandler::on(
                        "BTN-YIELD",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: "spinning".into() },
                            Step::Spin { iterations: SPIN_ITERATIONS, slice: SPIN_SLICE },
                            Step::Set { target: out(), name: "Caption".into(), value: "spun".into() },
                        ],
                    ),
                    FixtureHandler::on("BTN-TRAP", "onClick", vec![Step::Set { target: out(), name: "Caption".into(), value: "trapping".into() }, Step::Trap]),
                    FixtureHandler::on(
                        "BTN-BURST",
                        "onClick",
                        vec![Step::Burst { target: out(), name: "Caption".into(), value: "x".into(), count: BURST_COUNT }],
                    ),
                    FixtureHandler::on("BTN-ECHO", "onClick", vec![Step::EchoArg0 { target: out(), name: "Caption".into() }]),
                    FixtureHandler::on("BTN-ERR", "onClick", vec![Step::Error { code: 1001, message: "subscript out of range".into() }]),
                    FixtureHandler::on("BTN-STOP", "onClick", vec![Step::Stop]),
                    FixtureHandler::on(
                        "BTN-SUSPEND",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: "waiting".into() },
                            Step::Suspend,
                            Step::Set { target: out(), name: "Caption".into(), value: "resumed".into() },
                        ],
                    ),
                    FixtureHandler::on(
                        "BTN-OPEN",
                        "onClick",
                        vec![Step::Call { target: Target::App, name: "OpenFormAsync".into(), args: vec![CHILD_FORM.into()] }],
                    ),
                    // A read reflects the handler's own earlier write (WASM-017 R11): SET, then GET.
                    FixtureHandler::on(
                        "BTN-RAW",
                        "onClick",
                        vec![
                            Step::Set { target: t(TXT_IN), name: "Text".into(), value: "written first".into() },
                            Step::Get { target: t(TXT_IN), name: "Text".into() },
                            Step::EchoAnswer { target: out(), name: "Caption".into() },
                        ],
                    ),
                    // `OpenFormSync`: the handler suspends until the child closes, while the child's own
                    // handlers run (ABI amendment A5: tasks of different instances interleave).
                    FixtureHandler::on(
                        "BTN-MODAL",
                        "onClick",
                        vec![
                            Step::Set { target: out(), name: "Caption".into(), value: "modal".into() },
                            Step::Call { target: Target::Form, name: "OpenFormSync".into(), args: vec![CHILD_FORM.into()] },
                            Step::Suspend,
                            Step::Set { target: out(), name: "Caption".into(), value: "after modal".into() },
                        ],
                    ),
                ],
            },
            FixtureForm {
                name: CHILD_FORM.into(),
                main: false,
                instances: 3,
                controls: vec![("LBL-CHILD".into(), "Label".into()), ("BTN-CLOSE".into(), "Button".into())],
                handlers: vec![
                    FixtureHandler::form("onLoad", vec![Step::Set { target: Target::control("LBL-CHILD"), name: "Caption".into(), value: "child".into() }]),
                    FixtureHandler::form("onClose", vec![Step::Display("child closing".into())]),
                    FixtureHandler::on("BTN-CLOSE", "onClick", vec![Step::Call { target: Target::Form, name: "Close".into(), args: vec![] }]),
                ],
            },
        ],
    }
}

/// A minimal `.cfrm` for form `form_name` of `fixture`: its controls in a column, each of the type
/// the manifest names. What the build would write from the designer; here, for tests of the player
/// and the bundle, which both need a form description to pair with the module.
pub fn standard_cfrm(fixture: &Fixture, form_name: &str) -> String {
    let f = fixture.manifest.form_by_name(form_name).unwrap_or_else(|| panic!("no form `{form_name}` in the fixture"));
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"{}\" title=\"{}\" width=\"640\" height=\"480\">\n",
        f.name, f.name
    );
    for (i, c) in f.controls.iter().enumerate() {
        xml.push_str(&format!(
            "  <Control id=\"{}\" type=\"{}\" x=\"10\" y=\"{}\" w=\"200\" h=\"28\"></Control>\n",
            c.name,
            c.kind,
            10 + i * 34
        ));
    }
    xml.push_str("</Form>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{bundle_problems, scan_module};

    #[test]
    fn both_variants_are_acceptable_bundle_members() {
        for multi in [true, false] {
            let f = build(&standard_spec(multi));
            let problems = bundle_problems(&f.wasm, !multi);
            assert!(problems.is_empty(), "variant multi={multi}: {problems:#?}");
            let scan = scan_module(&f.wasm).unwrap();
            assert_eq!(scan.memories.len(), if multi { 2 } else { 1 });
            assert!(!scan.has_memory_grow);
            assert_eq!(scan.embedded_manifest().unwrap(), f.manifest);
        }
    }

    #[test]
    fn the_single_variant_has_no_multi_memory_instruction_and_the_multi_variant_needs_it() {
        let single = build(&standard_spec(false));
        crate::scan::validates_on_the_baseline(&single.wasm).expect("single-memory validates on the baseline");
        let multi = build(&standard_spec(true));
        assert!(crate::scan::validates_on_the_baseline(&multi.wasm).is_err(), "the multi-memory module needs multi-memory");
        crate::scan::validates_with_multi_memory(&multi.wasm).expect("and validates with it");
    }

    #[test]
    fn the_manifest_names_what_the_scenarios_use() {
        let f = build(&standard_spec(true));
        let m = &f.manifest;
        assert!(m.names.iter().any(|n| n == "Caption"));
        assert!(m.names.iter().any(|n| n == "AddItem"));
        assert_eq!(m.events[0], "onLoad");
        assert_eq!(m.forms.len(), 2);
        assert_eq!(m.forms[1].instances, 3);
        let main = m.main_form();
        let go = main.control_by_name("BTN-GO").unwrap();
        assert!(main.has_handler(go.id, m.event_index("onClick").unwrap()));
        assert!(!main.has_handler(go.id, m.event_index("onLoad").unwrap()));
    }
}
