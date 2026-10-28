// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Assembling the module: memory map, linked routines, block functions, the
//! dispatcher, the ABI exports and the custom sections (WASM-ABI-v1).

use crate::error::{GenError, GenResult};
use crate::image::{self, InitOp};
use crate::layout::Layout;
use crate::ir::*;
use crate::link::{Blob, Mapper};
use crate::lower::{FormCtx, Lower, Lowered};
use crate::pool::Pool;
use cobolt_ast::program::Program;
use cobolt_wasm_rt::state::WS_SIZE;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, DataSection, ElementSection, Elements, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemArg,
    MemoryType, Module, NameMap, NameSection, RefType, TableSection, TableType, TypeSection, ValType,
};

static RT_WASM: &[u8] = include_bytes!("../assets/rt.wasm");

/// The ABI version this generator produces (WASM-ABI-v1).
pub const ABI_VERSION: i32 = 1;

/// Sizes of the regions of the arena (ABI v1 §2).
#[derive(Debug, Clone, Copy)]
pub struct Arena {
    pub batch: u32,
    pub answer: u32,
    pub frame: u32,
}

impl Arena {
    pub fn total(&self) -> u32 {
        self.batch + self.answer + 2 * self.frame
    }
}

impl Default for Arena {
    fn default() -> Self {
        // The batch and answer regions are the routines' own staging sizes,
        // so a module carries every SET and reads every answer they can hold.
        Arena { batch: cobolt_wasm_rt::state::BATCH_SIZE, answer: cobolt_wasm_rt::state::ANSWER_SIZE, frame: 64 * 1024 }
    }
}

/// The file heap a module with in-memory files gets unless told otherwise:
/// 16 MiB, pages a browser commits only as they are written.
pub const DEFAULT_FILE_HEAP: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Options {
    /// Two memories (arena in memory 1) or one.
    pub multi_memory: bool,
    /// The NIST gate build: accepts the file verbs through host imports.
    pub conformance: bool,
    /// Treat an undeclared data-name as an implicit item (the NIST harness's
    /// `COBOL_TOLERATE_UNDECLARED`); never set for a Web or Mobile build.
    pub tolerate_undeclared: bool,
    pub arena: Arena,
    /// Blocks run before the module yields to the host.
    pub budget: i32,
    pub name: String,
    /// The 21 characters `FUNCTION WHEN-COMPILED` answers; `None` takes the
    /// time of the build (UTC), which a reproducible build overrides.
    pub when_compiled: Option<String>,
    /// Bytes of memory 0 for the records of in-memory INDEXED files and SORT
    /// work files (WASM-010 Q8). Reserved only when the program has such a
    /// file; there is no record limit (Q-new-1): when it is full a WRITE ends
    /// with FILE STATUS 89.
    pub file_heap: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { multi_memory: true, conformance: false, tolerate_undeclared: false, arena: Arena::default(), budget: 20_000, name: "program".into(), when_compiled: None, file_heap: DEFAULT_FILE_HEAP }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sizes {
    pub module_bytes: usize,
    pub rt_functions: usize,
    pub rt_bytes: usize,
    pub blocks: usize,
    pub data_pages: u32,
}

pub struct Compiled {
    pub wasm: Vec<u8>,
    pub manifest: serde_json::Value,
    pub sizes: Sizes,
    /// What compiled with a word to the developer (`ACCEPT` ignored, …).
    pub warnings: Vec<crate::error::GenWarning>,
    /// The line table, as the `prlines` section carries it (R14).
    pub lines: Vec<LineEntry>,
}

struct Types {
    list: Vec<(Vec<ValType>, Vec<ValType>)>,
}

impl Types {
    fn intern(&mut self, p: &[ValType], r: &[ValType]) -> u32 {
        for (i, (a, b)) in self.list.iter().enumerate() {
            if a == p && b == r {
                return i as u32;
            }
        }
        self.list.push((p.to_vec(), r.to_vec()));
        (self.list.len() - 1) as u32
    }
}

fn wp(v: &wasmparser::ValType) -> ValType {
    match v {
        wasmparser::ValType::I32 => ValType::I32,
        wasmparser::ValType::I64 => ValType::I64,
        wasmparser::ValType::F32 => ValType::F32,
        wasmparser::ValType::F64 => ValType::F64,
        _ => ValType::I32,
    }
}

/// The globals the form exports add: the running form, the event delivered.
const G_FORM: u32 = 9;
const G_EVENT: u32 = 10;

/// Bytes of one row of the form table, and of the instance table.
///
/// Form table row: `+0` control-name table, `+4` number of control ids (3 +
/// controls), `+8` first region, `+12` region size, `+16` first instance, `+20`
/// instances, `+24` entry block, `+28` park slot, `+32` PERFORM-stack offset,
/// `+36` `COBOL-QUIT` operand (0: none), `+40` the literal 1, `+44`/`+48` the
/// `ERROR` command a full form answers with, `+52` 1 for the main form, `+56`
/// where a start-up stopped by a run-time error goes on and `+60` 1 when the
/// form has that point (`Lowered::startup_resume`).
///
/// Instance (task) row (ABI A5: one task per instance): `+0` state (0 free, 1
/// idle at `WAIT-EVENT`, 2 running or paused), `+4` form, `+8`/`+12` where the
/// idle instance continues and its PERFORM-stack pointer, `+16`/`+20` the same
/// for a paused (yielded or suspended) run, `+24` opener, `+28` flags (bit 0
/// has an idle point, bit 1 closing, bit 2 paused).
const FT_ROW: u32 = 64;
const INST_ROW: u32 = 32;

const fn align(n: u32, a: u32) -> u32 {
    (n + a - 1) & !(a - 1)
}

/// The most pages memory 0 may have (WASM-017 R7): the 32-bit limit, 4 GiB.
/// A lower product cap is the operator's to set (Q-17b, open).
pub const MAX_DATA_PAGES: u32 = 65_536;

/// Refuse a memory 0 that would end at `end` bytes past what [`MAX_DATA_PAGES`]
/// allows, naming the form whose instances weigh most, its largest item and the
/// arithmetic (WASM-017 R7, AC5) — `Instances` or the table is the lever.
fn memory_ceiling(forms: &[FormSpec<'_>], layouts: &[&Layout], end: u64) -> GenResult<()> {
    let limit = u64::from(MAX_DATA_PAGES) * 65_536;
    if end.div_ceil(65_536) <= u64::from(MAX_DATA_PAGES) && end <= u64::from(u32::MAX) {
        return Ok(());
    }
    let weight = |i: usize| (u64::from(layouts[i].total) + 15) / 16 * 16 * u64::from(forms[i].instances.max(1));
    let fi = (0..forms.len()).max_by_key(|&i| weight(i)).unwrap_or(0);
    let layout = layouts[fi];
    let largest = layout
        .roots
        .iter()
        .map(|&r| &layout.items[r])
        .filter(|it| !it.hidden)
        .max_by_key(|it| u64::from(it.size) * u64::from(it.occurs.as_ref().map_or(1, |o| o.0)));
    let (item, item_bytes, line) = largest.map_or(("(none)".to_owned(), 0, 0), |it| {
        (it.name.clone().unwrap_or_else(|| "FILLER".into()), u64::from(it.size) * u64::from(it.occurs.as_ref().map_or(1, |o| o.0)), it.span_line)
    });
    let region = (u64::from(layout.total) + 15) / 16 * 16;
    let instances = forms[fi].instances.max(1);
    Err(GenError::kinded(
        line,
        crate::error::ErrorKind::Other,
        item.clone(),
        format!(
            "form {}: the data memory would be {end} bytes, more than the {limit} a WebAssembly memory holds; \
             its storage is {region} bytes × {instances} instances = {} bytes, the largest item {item} ({item_bytes} bytes) — \
             lower Instances or the table",
            forms[fi].name,
            region * u64::from(instances),
        ),
    )
    .in_form(fi))
}

fn mem0(offset: u64, align: u32) -> MemArg {
    MemArg { offset, align, memory_index: 0 }
}

enum Body {
    Rt(u32),
    Encoded(Function),
}

struct Def {
    name: String,
    ty: u32,
    body: Body,
}

/// One form of a project: the program the form generates (the form program, its
/// handler programs and the Common Code it calls, as sibling programs of one
/// source), what the manifest says of it, and how many instances may be open.
pub struct FormSpec<'a> {
    pub name: String,
    pub main: bool,
    pub instances: u32,
    pub program: &'a Program,
    /// `(name, type)` of each control, in manifest order; ids start at 3.
    pub controls: Vec<(String, String)>,
    /// `(control name — empty for the form itself, event name)` of each bound handler.
    pub bindings: Vec<(String, String)>,
    /// The form's layers above the base, in stack order (spec 091; ABI A9).
    pub layers: Vec<String>,
    /// Where each generated line came from, as the caller's source map resolves
    /// it (`cobolt_codegen::SourceMap::resolve`, the mapping Check uses): the
    /// line table maps a run-time error to the developer's line (R14). Empty
    /// for a program that is its own source.
    pub lines: Vec<SourceLine>,
}

/// One generated line and the place the developer wrote it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceLine {
    /// The line of the generated `.cbl` (what `$line` holds).
    pub gen_line: u32,
    /// The site, as `CodeSite::display_path` writes it (`MAIN ▸ BTN-OK ▸ onClick`).
    pub site: String,
    /// The line within the site's own text.
    pub line: u32,
    pub column: u32,
}

/// One row of the module's line table (`prlines`): a value of `$line` in unit
/// `unit` (a form's program, numbered as the manifest's forms) and where it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineEntry {
    pub unit: u32,
    pub unit_name: String,
    pub gen_line: u32,
    pub site: String,
    pub line: u32,
    pub column: u32,
}

/// Compile one program as a module that runs to its end (`pr_run`): the form a
/// conformance run, a test or a console program takes.
pub fn compile_program(prog: &Program, opts: &Options) -> GenResult<Compiled> {
    let spec = FormSpec { name: opts.name.clone(), main: true, instances: 1, program: prog, controls: vec![], bindings: vec![], layers: vec![], lines: vec![] };
    compile_forms(std::slice::from_ref(&spec), opts, false)
}

/// Compile the forms of a project into one module with the form exports of
/// ABI v1 §4 (`pr_open_form`, `pr_event`, …).
pub fn compile_project(forms: &[FormSpec<'_>], opts: &Options) -> GenResult<Compiled> {
    if forms.is_empty() {
        return Err(GenError::new(0, "a project needs at least one form"));
    }
    compile_forms(forms, opts, true)
}

/// The program modules of a Web or Mobile project, as `ModuleVariants` asks
/// for them (WASM-017 R9), and what they have to say.
pub struct WebModules {
    pub multi: Option<Compiled>,
    pub single: Option<Compiled>,
    pub warnings: Vec<crate::error::GenWarning>,
}

/// Compile the project's forms into the variants asked for: the
/// multi-memory module, the single-memory one, or both (WASM-010 R1, T19).
pub fn compile_web_project(forms: &[FormSpec<'_>], opts: &Options, multi: bool, single: bool) -> GenResult<WebModules> {
    if !multi && !single {
        return Err(GenError::new(0, "no program-module variant was asked for"));
    }
    let one = |m: bool| compile_project(forms, &Options { multi_memory: m, ..opts.clone() });
    let multi = if multi { Some(one(true)?) } else { None };
    let single = if single { Some(one(false)?) } else { None };
    let warnings = multi.as_ref().or(single.as_ref()).map(|c| c.warnings.clone()).unwrap_or_default();
    Ok(WebModules { multi, single, warnings })
}

struct Built {
    lowered: Lowered,
    img: image::Image,
    first_unit: usize,
    template_base: u32,
    region_start: u32,
    region_size: u32,
    first_inst: u32,
}

fn compile_forms(forms: &[FormSpec<'_>], opts: &Options, project: bool) -> GenResult<Compiled> {
    let blob = Blob::load(RT_WASM).map_err(|e| GenError::new(0, e.to_string()))?;

    // ── memory map ───────────────────────────────────────────────────────
    let rt_ws = align(blob.data_end, 16);
    let after_ws = align(rt_ws + WS_SIZE, 16);
    let arena_total = opts.arena.total();
    let (arena_base, consts_base) = if opts.multi_memory {
        (0, after_ws)
    } else {
        let a = after_ws;
        (a, align(a + arena_total, 16))
    };

    // ── lowering ─────────────────────────────────────────────────────────
    let mut pool = Pool::new(consts_base);
    let mut blocks: Vec<Block> = Vec::new();
    let mut names = crate::lower::Names::default();
    let when_compiled = opts.when_compiled.clone().unwrap_or_else(|| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        String::from_utf8_lossy(&cobolt_wasm_rt::func::current_date_text(ms)).into_owned()
    });
    let mut lowered_forms: Vec<(Lowered, usize)> = Vec::new();
    let mut unit_base = 0usize;
    for spec in forms {
        let ctx = if project {
            let mut controls = HashMap::new();
            for (i, (n, _)) in spec.controls.iter().enumerate() {
                controls.insert(n.to_ascii_uppercase(), 3 + i as u32);
            }
            let first_layer = 3 + spec.controls.len() as u32;
            let layers = spec.layers.iter().enumerate().map(|(k, n)| (n.to_ascii_uppercase(), first_layer + k as u32)).collect();
            Some(FormCtx { controls, layers, layer_names: spec.layers.clone(), name: spec.name.clone() })
        } else {
            None
        };
        let l = Lower::lower(
            spec.program,
            &mut pool,
            &mut blocks,
            &mut names,
            ctx,
            unit_base,
            opts.conformance,
            opts.tolerate_undeclared,
            when_compiled.clone(),
        )
        .map_err(|e| if project { e.in_form(lowered_forms.len()) } else { e })?;
        let first = unit_base;
        unit_base += l.units;
        lowered_forms.push((l, first));
    }
    let warnings: Vec<crate::error::GenWarning> = lowered_forms.iter().flat_map(|(l, _)| l.warnings.iter().cloned()).collect();
    // The line table: every value `$line` takes, per form, and where the
    // developer wrote it — through the caller's source map when it has one,
    // the generated line itself otherwise.
    let mut line_table: Vec<LineEntry> = Vec::new();
    for (fi, (l, _)) in lowered_forms.iter().enumerate() {
        let f = &forms[fi];
        for &g in &l.lines {
            let e = match f.lines.iter().find(|s| s.gen_line == g) {
                Some(s) => LineEntry { unit: fi as u32, unit_name: f.name.clone(), gen_line: g, site: s.site.clone(), line: s.line, column: s.column },
                None => LineEntry { unit: fi as u32, unit_name: f.name.clone(), gen_line: g, site: f.name.clone(), line: g, column: 0 },
            };
            line_table.push(e);
        }
    }

    // Reachable blocks, entry first.
    let mut order: Vec<BlockId> = Vec::new();
    let mut seen: HashMap<BlockId, u32> = HashMap::new();
    let mut stack: Vec<BlockId> = lowered_forms.iter().rev().map(|(l, _)| l.entry).collect();
    while let Some(b) = stack.pop() {
        if seen.contains_key(&b) {
            continue;
        }
        seen.insert(b, order.len() as u32);
        order.push(b);
        // Every block a block can name — as a jump, a branch, a frame it
        // pushes — appears in its code as a block reference.
        for ins in blocks[b].code.iter().rev() {
            if let Ins::Pc(t) = ins {
                stack.push(*t);
            }
        }
    }
    for &b in &order {
        if !blocks[b].done {
            return Err(GenError::new(0, format!("internal error: block {} was never terminated", blocks[b].name)));
        }
    }
    let entry_pcs: Vec<u32> = lowered_forms.iter().map(|(l, _)| seen[&l.entry]).collect();
    let startup_pcs: Vec<Option<u32>> = lowered_forms.iter().map(|(l, _)| l.startup_resume.and_then(|b| seen.get(&b).copied())).collect();

    // ── what the form exports need to find at run time ───────────────────
    // The names of the controls and events, as the form's event loop compares
    // them: each (address, length) pair is in the constant pool.
    let mut events: Vec<String> = Vec::new();
    for f in forms {
        for (_, e) in &f.bindings {
            if !events.contains(e) {
                events.push(e.clone());
            }
        }
    }
    let mut ev_pairs: Vec<u32> = Vec::new();
    for e in &events {
        let a = pool.intern(e.as_bytes());
        ev_pairs.push(a);
        ev_pairs.push(e.len() as u32);
    }
    let ev_tbl = pool.spec(&ev_pairs);
    let mut ctrl_tbls: Vec<u32> = Vec::new();
    for f in forms {
        let mut pairs: Vec<u32> = Vec::new();
        let mut put = |s: &str, pool: &mut Pool| {
            let a = pool.intern(s.as_bytes());
            pairs.push(a);
            pairs.push(s.len() as u32);
        };
        put(&f.name, &mut pool);
        put("super", &mut pool);
        put("COBOL", &mut pool);
        for (n, _) in &f.controls {
            put(n, &mut pool);
        }
        ctrl_tbls.push(pool.spec(&pairs));
    }
    let quit_addr = pool.intern(b"__QUIT__");
    let quit_ev_addr = pool.intern(b"Quit");
    // The ERROR command `pr_open_form` flushes when every instance of a form is
    // open (WASM-017 R2): built here, a constant of the module.
    let mut full_cmds: Vec<(u32, u32)> = Vec::new();
    for (fi, f) in forms.iter().enumerate() {
        let mut w = cobolt_wasm_abi::batch::BatchWriter::new();
        let msg = format!("no free instance of form {}: all {} are open", f.name, f.instances.max(1));
        w.error((-cobolt_wasm_abi::codes::err::NO_FREE_INSTANCE) as u32, &msg, fi as u32, 0, fi as u32, 0);
        let bytes = w.into_bytes();
        full_cmds.push((pool.intern(&bytes), bytes.len() as u32));
    }
    // The names the host commands carry by number (the manifest's `names`).
    let name_list: Vec<String> = names.list.clone();
    let builtin_list: Vec<String> = names.builtins.iter().cloned().collect();

    // ── image and memory map, continued ──────────────────────────────────
    let mut built: Vec<Built> = Vec::new();
    let ftab_base = align(consts_base + pool.bytes.len() as u32, 16);
    let ftab_size = if project { forms.len() as u32 * FT_ROW } else { 0 };
    let mut cursor = align(ftab_base + ftab_size, 16);
    // The ceiling before the images are built — their native build allocates
    // each region — on everything but the templates; again, exactly, below.
    let layouts: Vec<&Layout> = lowered_forms.iter().map(|(l, _)| &l.layout).collect();
    let fixed_end = |start: u64| -> u64 {
        let regions: u64 =
            layouts.iter().zip(forms).map(|(l, f)| (u64::from(l.total) + 15) / 16 * 16 * u64::from(f.instances.max(1))).sum();
        let insts: u64 = forms.iter().map(|f| u64::from(f.instances.max(1))).sum::<u64>() * u64::from(INST_ROW);
        let heap = if lowered_forms.iter().any(|(l, _)| l.uses_heap) { u64::from(opts.file_heap & !15) + 16 } else { 0 };
        (start + 15) / 16 * 16 + regions + 16 + insts + heap
    };
    memory_ceiling(forms, &layouts, fixed_end(u64::from(cursor)))?;
    let mut images: Vec<image::Image> = Vec::new();
    for (i, (l, _)) in lowered_forms.iter().enumerate() {
        images.push(image::build(&l.layout, forms[i].program.decimal_comma, forms[i].program.currency)?);
    }
    let mut template_bases: Vec<u32> = Vec::new();
    let mut templates_end = u64::from(cursor);
    for img in &images {
        templates_end = (templates_end + 15) / 16 * 16 + img.blob.len() as u64;
    }
    memory_ceiling(forms, &layouts, fixed_end(templates_end))?;
    drop(layouts);
    for img in &images {
        template_bases.push(cursor);
        cursor = align(cursor + img.blob.len() as u32, 16);
    }
    let mut first_inst = 0u32;
    let mut region_starts: Vec<u32> = Vec::new();
    for (i, (l, _)) in lowered_forms.iter().enumerate() {
        region_starts.push(cursor);
        cursor += align(l.layout.total, 16) * forms[i].instances.max(1);
    }
    let total_instances: u32 = forms.iter().map(|f| f.instances.max(1)).sum();
    let inst_tbl_base = align(cursor, 16);
    let mut end = inst_tbl_base + total_instances * INST_ROW;
    // The file heap (in-memory files, SORT) closes memory 0.
    let uses_heap = lowered_forms.iter().any(|(l, _)| l.uses_heap);
    let heap_base = align(end, 16);
    let heap_size = if uses_heap { opts.file_heap & !15 } else { 0 };
    if uses_heap {
        end = heap_base + heap_size;
    }
    for (i, ((l, fu), img)) in lowered_forms.into_iter().zip(images).enumerate() {
        built.push(Built {
            region_size: align(l.layout.total, 16),
            lowered: l,
            img,
            first_unit: fu,
            template_base: template_bases[i],
            region_start: region_starts[i],
            first_inst,
        });
        first_inst += forms[i].instances.max(1);
    }
    // The program-mode exports run the one form.
    let region_base = built[0].region_start;
    let template_base = built[0].template_base;
    let stack_off = built[0].lowered.stack_off;
    let data_pages = end.div_ceil(65536).max(1);
    let arena_pages = arena_total.div_ceil(65536).max(1);

    // ── which routines ───────────────────────────────────────────────────
    let mut roots: BTreeSet<&'static str> = BTreeSet::new();
    roots.insert("rt_init");
    roots.insert("rt_flush");
    if uses_heap {
        roots.insert("rt_heap_init");
        if project {
            roots.insert("rt_heap_release");
            roots.insert("rt_heap_used");
        }
    }
    if project {
        for r in ["rt_set_instance", "rt_deliver", "rt_cancel_run", "rt_move"] {
            roots.insert(r);
        }
    }
    for &b in &order {
        for ins in &blocks[b].code {
            if let Ins::CallRt(n) = ins {
                roots.insert(*n);
            }
        }
    }
    let root_list: Vec<&str> = roots.iter().copied().collect();
    let closure = blob.closure(&root_list).map_err(|e| GenError::new(0, e.to_string()))?;

    // ── types ────────────────────────────────────────────────────────────
    let mut types = Types { list: Vec::new() };
    let mut blob_types: HashMap<u32, u32> = HashMap::new();
    for (i, t) in blob.types.iter().enumerate() {
        let p: Vec<ValType> = t.params().iter().map(wp).collect();
        let r: Vec<ValType> = t.results().iter().map(wp).collect();
        blob_types.insert(i as u32, types.intern(&p, &r));
    }
    let ty_block = types.intern(&[], &[ValType::I32]);
    let ty_i32_void = types.intern(&[ValType::I32], &[]);
    let ty_i32_i32_void = types.intern(&[ValType::I32, ValType::I32], &[]);
    let ty_i32_ret = types.intern(&[], &[ValType::I32]);
    let ty_void = types.intern(&[], &[]);

    // ── function index space ────────────────────────────────────────────
    // The glue functions the module needs: the two it always has, plus the
    // file ones a conformance build's routines import. Each is backed by a
    // host import of the same name in the `pr` namespace.
    const GLUE_ORDER: [&str; 11] =
        ["flush", "abort", "query", "remote", "now", "random", "file_open", "file_close", "file_write", "file_read", "switch_state"];
    let mut glue_needed: BTreeSet<&'static str> = BTreeSet::new();
    glue_needed.insert("flush");
    glue_needed.insert("abort");
    for &i in &closure.imports {
        let (m, n) = blob.import_name(i);
        let name = GLUE_ORDER.iter().copied().find(|g| *g == n && m == "glue");
        match name {
            Some(g) => {
                if (g.starts_with("file_") || g == "switch_state") && !opts.conformance {
                    return Err(GenError::kinded(
                        0,
                        crate::error::ErrorKind::BackendOnly,
                        "file input/output",
                        "file input/output is not available in a browser module: it runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE",
                    ));
                }
                glue_needed.insert(g);
            }
            None => return Err(GenError::new(0, format!("the routine blob imports glue.{n}, which the generator does not provide"))),
        }
    }
    let glue_list: Vec<&'static str> = GLUE_ORDER.iter().copied().filter(|g| glue_needed.contains(g)).collect();
    let ty_i32_i32_i32_ret = types.intern(&[ValType::I32, ValType::I32, ValType::I32], &[ValType::I32]);
    let ty_i32_i32_i32_i32_ret = types.intern(&[ValType::I32; 4], &[ValType::I32]);
    let ty_i32_i32_ret = types.intern(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let ty_i32_ret1 = types.intern(&[ValType::I32], &[ValType::I32]);
    // (host import type, glue function type)
    let ty_f64_ret = types.intern(&[], &[ValType::F64]);
    let glue_types = |name: &str| -> (u32, u32) {
        match name {
            "now" | "random" => (ty_f64_ret, ty_f64_ret),
            "query" => (ty_i32_ret1, ty_i32_i32_i32_i32_ret),
            "remote" => (ty_i32_ret1, ty_i32_i32_ret),
            "flush" => (ty_i32_void, ty_i32_i32_void),
            "abort" => (ty_i32_void, ty_i32_void),
            "file_open" => (ty_i32_i32_ret, ty_i32_i32_i32_ret),
            "file_close" => (ty_i32_ret1, ty_i32_ret1),
            "file_write" => (ty_i32_i32_i32_ret, ty_i32_i32_i32_i32_ret),
            "switch_state" => (ty_i32_ret1, ty_i32_i32_ret),
            _ => (ty_i32_i32_ret, ty_i32_i32_i32_ret), // file_read
        }
    };
    let n_imports = glue_list.len() as u32;
    let imp_of = |name: &str| -> u32 { glue_list.iter().position(|g| *g == name).unwrap() as u32 };
    let mut defs: Vec<Def> = Vec::new();
    let mut glue_final: HashMap<&'static str, u32> = HashMap::new();
    for g in &glue_list {
        let idx = n_imports + defs.len() as u32;
        glue_final.insert(*g, idx);
        defs.push(Def { name: format!("glue.{g}"), ty: glue_types(g).1, body: Body::Encoded(Function::new([])) });
    }
    let g_flush = glue_final["flush"];
    let g_abort = glue_final["abort"];
    let imp_flush = imp_of("flush");
    let imp_abort = imp_of("abort");

    let mut rt_final: HashMap<u32, u32> = HashMap::new();
    for &f in &closure.funcs {
        let idx = n_imports + defs.len() as u32;
        rt_final.insert(f, idx);
        let (p, r) = blob.func_type(blob.type_of(f));
        let ty = types.intern(&p.iter().map(wp).collect::<Vec<_>>(), &r.iter().map(wp).collect::<Vec<_>>());
        defs.push(Def { name: blob.name_of(f), ty, body: Body::Rt(f) });
    }
    // The blob's glue imports map to our glue functions.
    for &i in &closure.imports {
        let (_, n) = blob.import_name(i);
        rt_final.insert(i, glue_final[n]);
    }

    let block_fn_base = n_imports + defs.len() as u32;
    let mut table_funcs: Vec<u32> = Vec::new();
    // Globals: 0 stack pointer, 1 base, 2 psp, 3 fuel, 4 line, 5 pc.
    let gl = |g: G| -> u32 {
        match g {
            G::Base => 1,
            G::Psp => 2,
            G::Fuel => 3,
            G::Line => 4,
            G::Resume => 6,
        }
    };
    let g_pc = 5u32;

    let mut rt_index_by_name: HashMap<&'static str, u32> = HashMap::new();
    for n in &roots {
        let bi = blob.export(n).ok_or_else(|| GenError::new(0, format!("the routine blob has no {n}")))?;
        rt_index_by_name.insert(*n, rt_final[&bi]);
    }

    for (k, &b) in order.iter().enumerate() {
        let block = &blocks[b];
        let mut f = Function::new([(2, ValType::I32)]);
        for ins in &block.code {
            match ins {
                Ins::Raw(i) => {
                    f.instruction(i);
                }
                Ins::CallRt(n) => {
                    f.instruction(&I::Call(rt_index_by_name[n]));
                }
                Ins::CallGlue(Glue::Flush) => {
                    f.instruction(&I::Call(g_flush));
                }
                Ins::CallGlue(Glue::Abort) => {
                    f.instruction(&I::Call(g_abort));
                }
                Ins::Pc(t) => {
                    f.instruction(&I::I32Const(seen[t] as i32));
                }
                Ins::GlobalGet(g) => {
                    f.instruction(&I::GlobalGet(gl(*g)));
                }
                Ins::GlobalSet(g) => {
                    f.instruction(&I::GlobalSet(gl(*g)));
                }
                // The per-unit initialisers are defined right after the blocks.
                Ins::InitUnit(u) => {
                    f.instruction(&I::GlobalGet(1));
                    f.instruction(&I::Call(block_fn_base + order.len() as u32 + *u as u32));
                }
            }
        }
        f.instruction(&I::End);
        let idx = block_fn_base + k as u32;
        table_funcs.push(idx);
        defs.push(Def { name: format!("{}#{}", block.name, k), ty: ty_block, body: Body::Encoded(f) });
    }

    // ── initialisers: each unit's storage (CANCEL) and each form's whole region ─
    // They take the base of the region they work on, so one function serves
    // every instance of a form.
    let emit_ops = |f: &mut Function, ops: &[InitOp], template_base: u32| {
        for op in ops {
            match op {
                InitOp::Fill { off, len, byte } => {
                    f.instruction(&I::LocalGet(0));
                    f.instruction(&I::I32Const(*off as i32));
                    f.instruction(&I::I32Add);
                    f.instruction(&I::I32Const(*byte as i32));
                    f.instruction(&I::I32Const(*len as i32));
                    f.instruction(&I::MemoryFill(0));
                }
                InitOp::Copy { off, blob_off, len } => {
                    f.instruction(&I::LocalGet(0));
                    f.instruction(&I::I32Const(*off as i32));
                    f.instruction(&I::I32Add);
                    f.instruction(&I::I32Const((template_base + blob_off) as i32));
                    f.instruction(&I::I32Const(*len as i32));
                    f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: 0 });
                }
            }
        }
    };
    let total_units = built.iter().map(|b| b.lowered.units).sum::<usize>();
    for b in &built {
        for u in 0..b.lowered.units {
            let mut f = Function::new([]);
            if let Some(ops) = b.img.unit_ops.get(u) {
                emit_ops(&mut f, ops, b.template_base);
            }
            f.instruction(&I::End);
            defs.push(Def { name: format!("init_unit_{}", b.first_unit + u), ty: ty_i32_void, body: Body::Encoded(f) });
        }
    }
    let init_form_base = block_fn_base + order.len() as u32 + total_units as u32;
    for (fi, b) in built.iter().enumerate() {
        let mut f = Function::new([]);
        emit_ops(&mut f, &b.img.ops, b.template_base);
        f.instruction(&I::End);
        defs.push(Def { name: format!("init_form_{fi}"), ty: ty_i32_void, body: Body::Encoded(f) });
    }
    let _ = ty_void;

    // ── glue bodies ──────────────────────────────────────────────────────
    let arena_mem = if opts.multi_memory { 1 } else { 0 };
    let arena_at = arena_base;
    {
        // glue.flush(addr, len): copy to the arena batch, call pr.flush(len)
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::Call(imp_flush));
        f.instruction(&I::End);
        defs[(g_flush - n_imports) as usize].body = Body::Encoded(f);
    }
    let answer_at = arena_at + opts.arena.batch;
    if glue_final.contains_key("now") {
        let mut f = Function::new([]);
        f.instruction(&I::Call(imp_of("now")));
        f.instruction(&I::End);
        defs[(glue_final["now"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("random") {
        let mut f = Function::new([]);
        f.instruction(&I::Call(imp_of("random")));
        f.instruction(&I::End);
        defs[(glue_final["random"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("query") {
        // glue.query(addr, len, ans, cap): the batch (commands and one query)
        // goes to the arena batch; the host leaves the answer text in the
        // arena answer region; min(n, cap) bytes are copied back to `ans`.
        let mut f = Function::new([(1, ValType::I32)]);
        emit(
            &mut f,
            &[
                I::I32Const(arena_at as i32),
                I::LocalGet(0),
                I::LocalGet(1),
                I::MemoryCopy { src_mem: 0, dst_mem: arena_mem },
                I::LocalGet(1),
                I::Call(imp_of("query")),
                I::LocalSet(4),
                I::LocalGet(4),
                I::I32Const(0),
                I::I32GtS,
                I::If(BlockType::Empty),
                I::LocalGet(2),
                I::I32Const(answer_at as i32),
                I::LocalGet(4),
                I::LocalGet(3),
                I::LocalGet(4),
                I::LocalGet(3),
                I::I32LtU,
                I::Select,
                I::MemoryCopy { src_mem: arena_mem, dst_mem: 0 },
                I::End,
                I::LocalGet(4),
                I::End,
            ],
        );
        defs[(glue_final["query"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("remote") {
        // glue.remote(addr, len): the request frame goes to frame-out (§2); a
        // frame larger than frame-out is the host's -5 without a copy.
        let frame_out = answer_at + opts.arena.answer;
        let mut f = Function::new([]);
        emit(
            &mut f,
            &[
                I::LocalGet(1),
                I::I32Const(opts.arena.frame as i32),
                I::I32GtU,
                I::If(BlockType::Empty),
                I::I32Const(-5),
                I::Return,
                I::End,
                I::I32Const(frame_out as i32),
                I::LocalGet(0),
                I::LocalGet(1),
                I::MemoryCopy { src_mem: 0, dst_mem: arena_mem },
                I::LocalGet(1),
                I::Call(imp_of("remote")),
                I::End,
            ],
        );
        defs[(glue_final["remote"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("switch_state") {
        // glue.switch_state(addr, len): the name goes to the arena batch.
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::Call(imp_of("switch_state")));
        f.instruction(&I::End);
        defs[(glue_final["switch_state"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_open") {
        // glue.file_open(addr, len, mode): the name goes to the arena batch.
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::Call(imp_of("file_open")));
        f.instruction(&I::End);
        defs[(glue_final["file_open"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_close") {
        let mut f = Function::new([]);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::Call(imp_of("file_close")));
        f.instruction(&I::End);
        defs[(glue_final["file_close"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_write") {
        // glue.file_write(handle, addr, len, adv)
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::Call(imp_of("file_write")));
        f.instruction(&I::End);
        defs[(glue_final["file_write"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_read") {
        // glue.file_read(handle, addr, cap): the host leaves the record in the
        // arena answer area; copy min(n, cap) bytes back.
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::Call(imp_of("file_read")));
        f.instruction(&I::LocalSet(3));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32GtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::I32Const(answer_at as i32));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::I32LtU);
        f.instruction(&I::Select);
        f.instruction(&I::MemoryCopy { src_mem: arena_mem, dst_mem: 0 });
        f.instruction(&I::End);
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::End);
        defs[(glue_final["file_read"] - n_imports) as usize].body = Body::Encoded(f);
    }
    {
        // glue.abort(code): ERROR command with the line, then pr.abort, then trap.
        // A run-time error of the routines is a COBOL run-time error: its code
        // goes out as 1000 + the routine's number (ABI A4). Every varint is
        // written at a fixed width (a padded LEB128 still decodes): code, the
        // text's length, unit, form and event in two bytes, the line in five.
        // The text is the one the routines left at `WS_ERR_TEXT` (`fail_text`;
        // empty after a plain `fail`, which leaves the text to the host), so
        // the fields after it start at `5 + L`: local 1 holds L, local 2 that
        // address.
        let mut f = Function::new([(2, ValType::I32)]);
        let mem0 = MemArg { offset: 0, align: 0, memory_index: arena_mem };
        let ws_text = rt_ws + cobolt_wasm_rt::state::WS_ERR_TEXT;
        // The address of byte `off` of the header (`None`) or of the fields
        // after the text (`Some(())`).
        let addr = |f: &mut Function, tail: bool, off: u32| {
            if tail {
                f.instruction(&I::LocalGet(2));
                f.instruction(&I::I32Const(off as i32));
                f.instruction(&I::I32Add);
            } else {
                f.instruction(&I::I32Const((arena_at + off) as i32));
            }
        };
        // A two-byte varint of the value on top of a computation `push`.
        let var2 = |f: &mut Function, tail: bool, off: u32, push: &dyn Fn(&mut Function)| {
            addr(f, tail, off);
            push(f);
            f.instruction(&I::I32Const(0x7F));
            f.instruction(&I::I32And);
            f.instruction(&I::I32Const(0x80));
            f.instruction(&I::I32Or);
            f.instruction(&I::I32Store8(mem0));
            addr(f, tail, off + 1);
            push(f);
            f.instruction(&I::I32Const(7));
            f.instruction(&I::I32ShrU);
            f.instruction(&I::I32Const(0x7F));
            f.instruction(&I::I32And);
            f.instruction(&I::I32Store8(mem0));
        };
        // L = min(the text's length, ERR_TEXT_MAX); t = arena + 5 + L.
        emit(
            &mut f,
            &[
                I::I32Const(ws_text as i32),
                I::I32Load(MemArg { offset: 0, align: 2, memory_index: 0 }),
                I::LocalTee(1),
                I::I32Const(cobolt_wasm_rt::state::ERR_TEXT_MAX as i32),
                I::LocalGet(1),
                I::I32Const(cobolt_wasm_rt::state::ERR_TEXT_MAX as i32),
                I::I32LtU,
                I::Select,
                I::LocalSet(1),
                I::I32Const((arena_at + 5) as i32),
                I::LocalGet(1),
                I::I32Add,
                I::LocalSet(2),
                // Read once: a later error the generated code raises itself
                // (a PERFORM too deep) must not carry this one's text.
                I::I32Const(ws_text as i32),
                I::I32Const(0),
                I::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }),
            ],
        );
        addr(&mut f, false, 0);
        f.instruction(&I::I32Const(0x04));
        f.instruction(&I::I32Store8(mem0));
        var2(&mut f, false, 1, &|f: &mut Function| {
            f.instruction(&I::LocalGet(0));
            f.instruction(&I::I32Const(1000));
            f.instruction(&I::I32Add);
        });
        var2(&mut f, false, 3, &|f: &mut Function| {
            f.instruction(&I::LocalGet(1));
        });
        emit(
            &mut f,
            &[
                I::I32Const((arena_at + 5) as i32),
                I::I32Const((ws_text + 4) as i32),
                I::LocalGet(1),
                I::MemoryCopy { src_mem: 0, dst_mem: arena_mem },
            ],
        );
        var2(&mut f, true, 0, &|f: &mut Function| {
            f.instruction(&I::GlobalGet(G_FORM));
        }); // unit: the form's program (prlines)
        for k in 0..5u32 {
            // line as a 5-byte padded varint
            addr(&mut f, true, 2 + k);
            f.instruction(&I::GlobalGet(4));
            f.instruction(&I::I32Const((7 * k) as i32));
            f.instruction(&I::I32ShrU);
            f.instruction(&I::I32Const(if k < 4 { 0x7F } else { 0x0F }));
            f.instruction(&I::I32And);
            if k < 4 {
                f.instruction(&I::I32Const(0x80));
                f.instruction(&I::I32Or);
            }
            f.instruction(&I::I32Store8(mem0));
        }
        var2(&mut f, true, 7, &|f: &mut Function| {
            f.instruction(&I::GlobalGet(G_FORM));
        });
        var2(&mut f, true, 9, &|f: &mut Function| {
            f.instruction(&I::GlobalGet(G_EVENT));
        });
        // The command's length: 5 + L + 11.
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::I32Const(16));
        f.instruction(&I::I32Add);
        f.instruction(&I::Call(imp_abort));
        f.instruction(&I::Unreachable);
        f.instruction(&I::End);
        defs[(g_abort - n_imports) as usize].body = Body::Encoded(f);
    }

    // ── dispatcher and exports ───────────────────────────────────────────
    let run_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::Loop(BlockType::Empty));
        f.instruction(&I::GlobalGet(g_pc));
        f.instruction(&I::CallIndirect { type_index: ty_block, table_index: 0 });
        f.instruction(&I::LocalTee(0));
        f.instruction(&I::GlobalSet(g_pc));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32LtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::Return);
        f.instruction(&I::End);
        f.instruction(&I::GlobalGet(3));
        f.instruction(&I::I32Const(1));
        f.instruction(&I::I32Sub);
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::GlobalGet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32LtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::I32Const(-2));
        f.instruction(&I::Return);
        f.instruction(&I::End);
        f.instruction(&I::Br(0));
        f.instruction(&I::End);
        f.instruction(&I::Unreachable);
        f.instruction(&I::End);
        defs.push(Def { name: "run".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }
    let exp_const = |name: &str, v: i32, defs: &mut Vec<Def>, ty: u32| -> u32 {
        let idx = n_imports + defs.len() as u32;
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(v));
        f.instruction(&I::End);
        defs.push(Def { name: name.into(), ty, body: Body::Encoded(f) });
        idx
    };
    let e_abi = exp_const("pr_abi", ABI_VERSION, &mut defs, ty_i32_ret);
    let variant = (opts.multi_memory as i32) | ((opts.conformance as i32) << 1);
    let e_variant = exp_const("pr_variant", variant, &mut defs, ty_i32_ret);
    let e_arena = exp_const("pr_arena_base", arena_base as i32, &mut defs, ty_i32_ret);

    let init_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(rt_ws as i32));
        f.instruction(&I::Call(rt_index_by_name["rt_init"]));
        if uses_heap {
            f.instruction(&I::I32Const(heap_base as i32));
            f.instruction(&I::I32Const(heap_size as i32));
            f.instruction(&I::Call(rt_index_by_name["rt_heap_init"]));
        }
        f.instruction(&I::I32Const((region_base + stack_off) as i32));
        f.instruction(&I::GlobalSet(2));
        f.instruction(&I::I32Const(opts.budget));
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::End);
        defs.push(Def { name: "pr_init".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }
    let run_prog_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::Call(init_idx));
        f.instruction(&I::Drop);
        f.instruction(&I::I32Const(region_base as i32));
        f.instruction(&I::GlobalSet(1));
        f.instruction(&I::I32Const(region_base as i32));
        f.instruction(&I::Call(init_form_base));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::GlobalSet(g_pc));
        f.instruction(&I::Loop(BlockType::Empty));
        f.instruction(&I::Call(run_idx));
        f.instruction(&I::LocalTee(0));
        f.instruction(&I::I32Const(-2));
        f.instruction(&I::I32Eq);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::I32Const(opts.budget));
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::Br(1));
        f.instruction(&I::End);
        f.instruction(&I::End);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::End);
        defs.push(Def { name: "pr_run".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }
    let form_exports = if project {
        let ft_rows: Vec<FormRow> = built
            .iter()
            .enumerate()
            .map(|(fi, b)| FormRow {
                ctrl_tbl: ctrl_tbls[fi],
                n_ctrl: 3 + forms[fi].controls.len() as u32,
                region_start: b.region_start,
                region_size: b.region_size,
                first_inst: b.first_inst,
                instances: forms[fi].instances.max(1),
                entry_pc: entry_pcs[fi],
                startup_pc: startup_pcs[fi],
                park_slot: b.lowered.park_slot,
                stack_off: b.lowered.stack_off,
                quit: b.lowered.quit.unwrap_or((0, 0)),
                full_cmd: full_cmds[fi],
                main: forms[fi].main,
            })
            .collect();
        let ctx = ProjectFns {
            ftab_base,
            inst_tbl_base,
            total_instances,
            n_forms: forms.len() as u32,
            n_events: events.len() as u32,
            ev_tbl,
            quit_addr,
            quit_ev_addr,
            budget: opts.budget,
            g_pc,
            run_idx,
            init_form_base,
            g_flush,
            rt: &rt_index_by_name,
            ty_i32_ret,
            uses_heap,
            frame_in_len_at: rt_ws + cobolt_wasm_rt::state::WS_FRAME_IN_LEN,
            frame_at: rt_ws + cobolt_wasm_rt::state::WS_FRAME,
            frame_cap: opts.arena.frame.min(cobolt_wasm_rt::state::FRAME_SIZE),
            frame_in: arena_base + opts.arena.batch + opts.arena.answer + opts.arena.frame,
            arena_mem: if opts.multi_memory { 1 } else { 0 },
        };
        let fx = emit_project_fns(&ctx, n_imports, &mut defs, &mut types);
        Some((fx, ft_rows))
    } else {
        None
    };

    // ── emit sections ────────────────────────────────────────────────────
    let mut module = Module::new();

    let mut ts = TypeSection::new();
    for (p, r) in &types.list {
        ts.ty().function(p.iter().copied(), r.iter().copied());
    }
    module.section(&ts);

    let mut imports = ImportSection::new();
    for g in &glue_list {
        imports.import("pr", g, EntityType::Function(glue_types(g).0));
    }
    module.section(&imports);

    let mut fs = FunctionSection::new();
    for d in &defs {
        fs.function(d.ty);
    }
    module.section(&fs);

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        minimum: table_funcs.len() as u64,
        maximum: Some(table_funcs.len() as u64),
        table64: false,
        shared: false,
    });
    module.section(&tables);

    let mut mems = wasm_encoder::MemorySection::new();
    mems.memory(MemoryType {
        minimum: data_pages as u64,
        maximum: Some(data_pages as u64),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    if opts.multi_memory {
        mems.memory(MemoryType {
            minimum: arena_pages as u64,
            maximum: Some(arena_pages as u64),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
    }
    module.section(&mems);

    let mut globals = GlobalSection::new();
    let gt = GlobalType { val_type: ValType::I32, mutable: true, shared: false };
    globals.global(gt, &ConstExpr::i32_const(blob.stack_top as i32));
    // 1 base, 2 psp, 3 fuel, 4 line, 5 pc, 6 resume, 7 task (instance + 1, or 0),
    // 8 status, 9 the running form, 10 the event being delivered
    for _ in 1..=10 {
        globals.global(gt, &ConstExpr::i32_const(0));
    }
    module.section(&globals);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    // `$line` (ABI §10, A10): the line of the statement about to run.
    exports.export("pr_line", ExportKind::Global, 4);
    exports.export("arena", ExportKind::Memory, if opts.multi_memory { 1 } else { 0 });
    exports.export("pr_abi", ExportKind::Func, e_abi);
    exports.export("pr_variant", ExportKind::Func, e_variant);
    exports.export("pr_arena_base", ExportKind::Func, e_arena);
    exports.export("pr_init", ExportKind::Func, init_idx);
    match &form_exports {
        Some((fx, _)) => {
            for (name, idx) in &fx.exports {
                exports.export(name, ExportKind::Func, *idx);
            }
        }
        None => {
            exports.export("pr_run", ExportKind::Func, run_prog_idx);
        }
    }
    module.section(&exports);

    let mut elems = ElementSection::new();
    elems.active(Some(0), &ConstExpr::i32_const(0), Elements::Functions(std::borrow::Cow::Owned(table_funcs.clone())));
    module.section(&elems);

    // Code, in function-index order.
    let mut code = CodeSection::new();
    let mut mapper_types = blob_types.clone();
    let mut rt_bytes = 0usize;
    {
        let mut mapper = Mapper { func: &rt_final, types: &mapper_types, stack_global: 0 };
        for d in &defs {
            match &d.body {
                Body::Encoded(f) => {
                    code.function(f);
                }
                Body::Rt(bi) => {
                    let before = code.byte_len();
                    blob.emit_body(*bi, &mut mapper, &mut code).map_err(|e| GenError::new(0, e.to_string()))?;
                    rt_bytes += code.byte_len() - before;
                }
            }
        }
    }
    mapper_types.clear();
    module.section(&code);

    let mut data = DataSection::new();
    for (addr, bytes) in &blob.data {
        data.active(0, &ConstExpr::i32_const(*addr as i32), bytes.iter().copied());
    }
    if !pool.bytes.is_empty() {
        data.active(0, &ConstExpr::i32_const(consts_base as i32), pool.bytes.iter().copied());
    }
    for b in &built {
        if !b.img.blob.is_empty() {
            data.active(0, &ConstExpr::i32_const(b.template_base as i32), b.img.blob.iter().copied());
        }
    }
    if let Some((_, rows)) = &form_exports {
        let mut bytes: Vec<u8> = Vec::with_capacity(rows.len() * FT_ROW as usize);
        for r in rows {
            let mut row = [0u32; (FT_ROW / 4) as usize];
            row[0] = r.ctrl_tbl;
            row[1] = r.n_ctrl;
            row[2] = r.region_start;
            row[3] = r.region_size;
            row[4] = r.first_inst;
            row[5] = r.instances;
            row[6] = r.entry_pc;
            row[7] = r.park_slot;
            row[8] = r.stack_off;
            row[9] = r.quit.0;
            row[10] = r.quit.1;
            row[11] = r.full_cmd.0;
            row[12] = r.full_cmd.1;
            row[13] = r.main as u32;
            if let Some(pc) = r.startup_pc {
                row[14] = pc;
                row[15] = 1;
            }
            for w in row {
                bytes.extend_from_slice(&w.to_le_bytes());
            }
        }
        data.active(0, &ConstExpr::i32_const(ftab_base as i32), bytes);
    }
    module.section(&data);

    // Names.
    let mut names = NameSection::new();
    let mut fnames = NameMap::new();
    for g in &glue_list {
        fnames.append(imp_of(g), &format!("pr.{g}"));
    }
    for (i, d) in defs.iter().enumerate() {
        fnames.append(n_imports + i as u32, &d.name);
    }
    names.functions(&fnames);
    module.section(&names);

    let abi_manifest = build_manifest(&ManifestInput {
        opts,
        forms,
        built: &built,
        events: &events,
        names: &name_list,
        builtins: builtin_list,
        rt_functions: closure.funcs.len(),
        rt_bytes,
        imports: glue_list.iter().map(|g| g.to_string()).collect(),
        arena_total,
        data_pages,
        total_instances,
    });
    let mut manifest = serde_json::to_value(&abi_manifest).expect("the manifest serialises");
    // Where the generator put things, for diagnostics; hosts ignore it.
    manifest["regions"] = serde_json::json!({ "rt_workspace": rt_ws, "perform_stack": region_base + stack_off, "constants": consts_base, "template": template_base, "region": region_base });
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    module.section(&CustomSection { name: std::borrow::Cow::Borrowed("prmanifest"), data: std::borrow::Cow::Borrowed(&manifest_bytes) });
    let lines_bytes = serde_json::to_vec(&line_table).unwrap();
    module.section(&CustomSection { name: std::borrow::Cow::Borrowed("prlines"), data: std::borrow::Cow::Borrowed(&lines_bytes) });

    let wasm = module.finish();
    let _ = BTreeMap::<u8, u8>::new();
    let sizes = Sizes {
        module_bytes: wasm.len(),
        rt_functions: closure.funcs.len(),
        rt_bytes,
        blocks: order.len(),
        data_pages,
    };
    Ok(Compiled { wasm, manifest, sizes, warnings, lines: line_table })
}

// ── the project module's form exports (ABI v1 §4, A1, A2, A5) ───────────────

/// What one row of the form table holds (see [`FT_ROW`]).
struct FormRow {
    ctrl_tbl: u32,
    n_ctrl: u32,
    region_start: u32,
    region_size: u32,
    first_inst: u32,
    instances: u32,
    entry_pc: u32,
    startup_pc: Option<u32>,
    park_slot: u32,
    stack_off: u32,
    quit: (u32, u32),
    full_cmd: (u32, u32),
    main: bool,
}

/// What the form exports need to know of the module being assembled.
struct ProjectFns<'a> {
    ftab_base: u32,
    inst_tbl_base: u32,
    total_instances: u32,
    n_forms: u32,
    n_events: u32,
    ev_tbl: u32,
    quit_addr: u32,
    quit_ev_addr: u32,
    budget: i32,
    g_pc: u32,
    run_idx: u32,
    init_form_base: u32,
    g_flush: u32,
    rt: &'a HashMap<&'static str, u32>,
    ty_i32_ret: u32,
    /// The module has a file heap: a released instance gives its blocks back.
    uses_heap: bool,
    /// Where `pr_resume` leaves a remote answer for the routines (ABI §4):
    /// its length word and its bytes in memory 0, how many fit, and frame-in.
    frame_in_len_at: u32,
    frame_at: u32,
    frame_cap: u32,
    frame_in: u32,
    arena_mem: u32,
}

struct FormFns {
    exports: Vec<(&'static str, u32)>,
}

fn emit(f: &mut Function, ins: &[I<'static>]) {
    for i in ins {
        f.instruction(i);
    }
}

fn ld(off: u64) -> I<'static> {
    I::I32Load(mem0(off, 2))
}

fn stw(off: u64) -> I<'static> {
    I::I32Store(mem0(off, 2))
}

/// `dst = tbl + idx * row`.
fn row_addr(f: &mut Function, idx: u32, tbl: u32, row: u32, dst: u32) {
    emit(f, &[I::LocalGet(idx), I::I32Const(row as i32), I::I32Mul, I::I32Const(tbl as i32), I::I32Add, I::LocalSet(dst)]);
}

/// `dst = ft.region_start + (inst - ft.first_inst) * ft.region_size`.
fn region_of(f: &mut Function, inst: u32, ft: u32, dst: u32) {
    emit(
        f,
        &[
            I::LocalGet(ft),
            ld(8),
            I::LocalGet(inst),
            I::LocalGet(ft),
            ld(16),
            I::I32Sub,
            I::LocalGet(ft),
            ld(12),
            I::I32Mul,
            I::I32Add,
            I::LocalSet(dst),
        ],
    );
}

/// Return `-4` (no such instance) or `-2` (busy) unless instance row `row` is
/// idle; the status is recorded for `pr_status`.
fn require_idle(f: &mut Function, row: u32, scratch: u32) {
    emit(
        f,
        &[
            I::LocalGet(row),
            ld(0),
            I::I32Const(1),
            I::I32Ne,
            I::If(BlockType::Empty),
            I::I32Const(-4),
            I::I32Const(-2),
            I::LocalGet(row),
            ld(0),
            I::I32Eqz,
            I::Select,
            I::LocalTee(scratch),
            I::GlobalSet(8),
            I::LocalGet(scratch),
            I::Return,
            I::End,
        ],
    );
}

/// Return `-4` when `local` is not below `limit` (as unsigned: a negative id too).
fn require_below(f: &mut Function, local: u32, limit: u32) {
    emit(
        f,
        &[
            I::LocalGet(local),
            I::I32Const(limit as i32),
            I::I32GeU,
            I::If(BlockType::Empty),
            I::I32Const(-4),
            I::GlobalSet(8),
            I::I32Const(-4),
            I::Return,
            I::End,
        ],
    );
}

fn emit_project_fns(c: &ProjectFns<'_>, n_imports: u32, defs: &mut Vec<Def>, types: &mut Types) -> FormFns {
    let i32_ = ValType::I32;
    let ty_ii_v = types.intern(&[i32_, i32_], &[]);
    let ty_i_v = types.intern(&[i32_], &[]);
    let ty_iii_r = types.intern(&[i32_; 3], &[i32_]);
    let ty_ii_r = types.intern(&[i32_, i32_], &[i32_]);
    let ty_i_r = types.intern(&[i32_], &[i32_]);
    let ty_iiii_r = types.intern(&[i32_; 4], &[i32_]);
    let rt = |n: &str| c.rt[n];
    let (tbl, ft_base) = (c.inst_tbl_base, c.ftab_base);
    let next = |defs: &mut Vec<Def>, name: &str, ty: u32, f: Function| -> u32 {
        let idx = n_imports + defs.len() as u32;
        defs.push(Def { name: name.into(), ty, body: Body::Encoded(f) });
        idx
    };

    // init_dispatch(form, base): the form's initialiser on the region at base.
    let mut f = Function::new([]);
    for fi in 0..c.n_forms {
        emit(&mut f, &[I::LocalGet(0), I::I32Const(fi as i32), I::I32Eq, I::If(BlockType::Empty), I::LocalGet(1), I::Call(c.init_form_base + fi), I::End]);
    }
    f.instruction(&I::End);
    let init_dispatch = next(defs, "init_dispatch", ty_ii_v, f);

    // release(inst): the region back to its initial state, the row free, and
    // the instance's records in the file heap given back.
    let mut f = Function::new([(4, i32_)]); // row 1, ft 2, base 3, form 4
    if c.uses_heap {
        emit(&mut f, &[I::LocalGet(0), I::Call(rt("rt_heap_release"))]);
    }
    row_addr(&mut f, 0, tbl, INST_ROW, 1);
    emit(&mut f, &[I::LocalGet(1), ld(4), I::LocalSet(4)]);
    row_addr(&mut f, 4, ft_base, FT_ROW, 2);
    region_of(&mut f, 0, 2, 3);
    emit(
        &mut f,
        &[
            I::LocalGet(4),
            I::LocalGet(3),
            I::Call(init_dispatch),
            I::LocalGet(1),
            I::I32Const(0),
            stw(0),
            I::LocalGet(1),
            I::I32Const(0),
            stw(28),
            I::End,
        ],
    );
    let release = next(defs, "release", ty_i_v, f);

    // run_task(inst, pc, psp) -> status: run the instance's task from `pc`
    // until it parks, yields, suspends or ends.
    let mut f = Function::new([(7, i32_)]); // row 3, ft 4, r 5, base 6, status 7, form 8, flags 9
    row_addr(&mut f, 0, tbl, INST_ROW, 3);
    emit(&mut f, &[I::LocalGet(3), ld(4), I::LocalSet(8)]);
    row_addr(&mut f, 8, ft_base, FT_ROW, 4);
    region_of(&mut f, 0, 4, 6);
    emit(
        &mut f,
        &[
            I::LocalGet(6),
            I::GlobalSet(1),
            I::LocalGet(2),
            I::GlobalSet(2),
            I::LocalGet(1),
            I::GlobalSet(c.g_pc),
            I::I32Const(c.budget),
            I::GlobalSet(3),
            I::LocalGet(0),
            I::I32Const(1),
            I::I32Add,
            I::GlobalSet(7),
            I::LocalGet(8),
            I::GlobalSet(G_FORM),
            I::LocalGet(3),
            I::I32Const(2),
            stw(0),
            I::LocalGet(0),
            I::Call(rt("rt_set_instance")),
            I::Call(c.run_idx),
            I::LocalSet(5),
            I::LocalGet(3),
            ld(28),
            I::LocalSet(9),
            I::Block(BlockType::Empty),
            // PARK: idle at WAIT-EVENT — the handler is done.
            I::LocalGet(5),
            I::I32Const(ret::PARK),
            I::I32Eq,
            I::If(BlockType::Empty),
            I::LocalGet(3),
            I::GlobalGet(6),
            stw(8),
            I::LocalGet(3),
            I::GlobalGet(2),
            stw(12),
            I::LocalGet(9),
            I::I32Const(1),
            I::I32Or,
            I::I32Const(!4),
            I::I32And,
            I::LocalSet(9),
            I::LocalGet(3),
            I::LocalGet(9),
            stw(28),
            I::I32Const(0),
            I::LocalSet(7),
            I::LocalGet(9),
            I::I32Const(2),
            I::I32And,
            I::If(BlockType::Empty),
            I::LocalGet(0),
            I::Call(release),
            I::Else,
            I::LocalGet(3),
            I::I32Const(1),
            stw(0),
            I::End,
            I::Br(1),
            I::End,
        ],
    );
    // YIELD (budget) and SUSPEND (a wait): paused, to be resumed.
    for (code, pc_from, status) in [(-2, Some(c.g_pc), 2), (ret::SUSPEND, None, 1)] {
        emit(&mut f, &[I::LocalGet(5), I::I32Const(code), I::I32Eq, I::If(BlockType::Empty), I::LocalGet(3)]);
        f.instruction(&I::GlobalGet(pc_from.unwrap_or(6)));
        emit(
            &mut f,
            &[
                stw(16),
                I::LocalGet(3),
                I::GlobalGet(2),
                stw(20),
                I::LocalGet(3),
                I::LocalGet(9),
                I::I32Const(4),
                I::I32Or,
                stw(28),
                I::Call(rt("rt_flush")),
                I::I32Const(status),
                I::LocalSet(7),
                I::Br(1),
                I::End,
            ],
        );
    }
    // DONE / STOP RUN: the form program ended; STOPPED for the main form.
    emit(
        &mut f,
        &[
            I::Call(rt("rt_flush")),
            I::I32Const(0),
            I::LocalSet(7),
            I::LocalGet(9),
            I::I32Const(2),
            I::I32And,
            I::I32Eqz,
            I::If(BlockType::Empty),
            I::LocalGet(4),
            ld(52),
            I::If(BlockType::Empty),
            I::I32Const(3),
            I::LocalSet(7),
            I::End,
            I::End,
            I::LocalGet(0),
            I::Call(release),
            I::End,
            I::LocalGet(7),
            I::GlobalSet(8),
            I::LocalGet(7),
            I::End,
        ],
    );
    let run_task = next(defs, "run_task", ty_iii_r, f);

    // pr_open_form(form, opener) -> instance id
    let mut f = Function::new([(5, i32_)]); // i 2, end 3, ft 4, row 5, base 6
    require_below(&mut f, 0, c.n_forms);
    row_addr(&mut f, 0, ft_base, FT_ROW, 4);
    emit(
        &mut f,
        &[
            I::LocalGet(4),
            ld(16),
            I::LocalSet(2),
            I::LocalGet(2),
            I::LocalGet(4),
            ld(20),
            I::I32Add,
            I::LocalSet(3),
            I::Block(BlockType::Empty),
            I::Loop(BlockType::Empty),
            I::LocalGet(2),
            I::LocalGet(3),
            I::I32GeU,
            I::If(BlockType::Empty),
            I::LocalGet(4),
            ld(44),
            I::LocalGet(4),
            ld(48),
            I::Call(c.g_flush),
            I::I32Const(-3),
            I::GlobalSet(8),
            I::I32Const(-3),
            I::Return,
            I::End,
        ],
    );
    row_addr(&mut f, 2, tbl, INST_ROW, 5);
    emit(&mut f, &[I::LocalGet(5), ld(0), I::I32Eqz, I::BrIf(1), I::LocalGet(2), I::I32Const(1), I::I32Add, I::LocalSet(2), I::Br(0), I::End, I::End]);
    emit(
        &mut f,
        &[
            I::LocalGet(5),
            I::LocalGet(0),
            stw(4),
            I::LocalGet(5),
            I::LocalGet(1),
            stw(24),
            I::LocalGet(5),
            I::I32Const(0),
            stw(28),
            I::LocalGet(5),
            I::I32Const(2),
            stw(0),
        ],
    );
    region_of(&mut f, 2, 4, 6);
    emit(
        &mut f,
        &[
            I::LocalGet(0),
            I::LocalGet(6),
            I::Call(init_dispatch),
            I::LocalGet(2),
            I::LocalGet(4),
            ld(24),
            I::LocalGet(6),
            I::LocalGet(4),
            ld(32),
            I::I32Add,
            I::Call(run_task),
            I::Drop,
            I::LocalGet(2),
            I::End,
        ],
    );
    let open = next(defs, "pr_open_form", ty_ii_r, f);

    // pr_event(inst, ctrl, event, argc) -> status
    let mut f = Function::new([(6, i32_)]); // row 4, ft 5, base 6, form 7, cbl 8, ev 9
    require_below(&mut f, 0, c.total_instances);
    row_addr(&mut f, 0, tbl, INST_ROW, 4);
    require_idle(&mut f, 4, 9);
    emit(&mut f, &[I::LocalGet(4), ld(4), I::LocalSet(7)]);
    row_addr(&mut f, 7, ft_base, FT_ROW, 5);
    emit(
        &mut f,
        &[
            I::LocalGet(1),
            I::LocalGet(5),
            ld(4),
            I::I32GeU,
            I::LocalGet(2),
            I::I32Const(c.n_events as i32),
            I::I32GeU,
            I::I32Or,
            I::If(BlockType::Empty),
            I::I32Const(-4),
            I::GlobalSet(8),
            I::I32Const(-4),
            I::Return,
            I::End,
        ],
    );
    region_of(&mut f, 0, 5, 6);
    emit(
        &mut f,
        &[
            I::LocalGet(5),
            ld(0),
            I::LocalGet(1),
            I::I32Const(8),
            I::I32Mul,
            I::I32Add,
            I::LocalSet(8),
            I::LocalGet(2),
            I::I32Const(8),
            I::I32Mul,
            I::I32Const(c.ev_tbl as i32),
            I::I32Add,
            I::LocalSet(9),
            I::LocalGet(6),
            I::LocalGet(5),
            ld(28),
            I::LocalGet(8),
            ld(0),
            I::LocalGet(8),
            ld(4),
            I::LocalGet(9),
            ld(0),
            I::LocalGet(9),
            ld(4),
            I::Call(rt("rt_deliver")),
            I::LocalGet(2),
            I::GlobalSet(G_EVENT),
            I::LocalGet(0),
            I::LocalGet(4),
            ld(8),
            I::LocalGet(4),
            ld(12),
            I::Call(run_task),
            I::End,
        ],
    );
    let event = next(defs, "pr_event", ty_iiii_r, f);

    // pr_resume(task, len) -> status
    let mut f = Function::new([(1, i32_)]); // row 2
    require_below(&mut f, 0, c.total_instances);
    row_addr(&mut f, 0, tbl, INST_ROW, 2);
    emit(
        &mut f,
        &[
            I::LocalGet(2),
            ld(0),
            I::I32Const(2),
            I::I32Ne,
            I::LocalGet(2),
            ld(28),
            I::I32Const(4),
            I::I32And,
            I::I32Eqz,
            I::I32Or,
            I::If(BlockType::Empty),
            I::I32Const(-2),
            I::GlobalSet(8),
            I::I32Const(-2),
            I::Return,
            I::End,
            // A remote answer of `len` bytes: into memory 0 for the routines, its
            // length beside it (u32::MAX when it does not fit).
            I::I32Const(c.frame_in_len_at as i32),
            I::LocalGet(1),
            I::I32Const(-1),
            I::LocalGet(1),
            I::I32Const(c.frame_cap as i32),
            I::I32LeU,
            I::Select,
            stw(0),
            I::LocalGet(1),
            I::I32Const(0),
            I::I32GtS,
            I::LocalGet(1),
            I::I32Const(c.frame_cap as i32),
            I::I32LeU,
            I::I32And,
            I::If(BlockType::Empty),
            I::I32Const(c.frame_at as i32),
            I::I32Const(c.frame_in as i32),
            I::LocalGet(1),
            I::MemoryCopy { src_mem: c.arena_mem, dst_mem: 0 },
            I::End,
            I::LocalGet(0),
            I::LocalGet(2),
            ld(16),
            I::LocalGet(2),
            ld(20),
            I::Call(run_task),
            I::End,
        ],
    );
    let resume = next(defs, "pr_resume", ty_ii_r, f);

    // pr_cancel(task): -1 is the run that trapped (A5). An instance that had
    // reached its event loop goes back to it (idle); one whose start-up
    // stopped — a run-time error in `onLoad`, perhaps after a remote call —
    // goes on from its form's start-up point to the event loop, as the
    // desktop abandons the handler and keeps the form; `pr_tick()` then names
    // it and `pr_status()` says how that run ended. Otherwise it is freed.
    let mut f = Function::new([(3, i32_)]); // row 1, ft 2, base 3
    emit(
        &mut f,
        &[
            I::LocalGet(0),
            I::I32Const(-1),
            I::I32Eq,
            I::If(BlockType::Empty),
            I::GlobalGet(7),
            I::I32Const(1),
            I::I32Sub,
            I::LocalSet(0),
            I::End,
            I::LocalGet(0),
            I::I32Const(c.total_instances as i32),
            I::I32GeU,
            I::If(BlockType::Empty),
            I::Return,
            I::End,
            I::Call(rt("rt_cancel_run")),
        ],
    );
    row_addr(&mut f, 0, tbl, INST_ROW, 1);
    emit(
        &mut f,
        &[
            I::LocalGet(1),
            ld(28),
            I::I32Const(1),
            I::I32And,
            I::If(BlockType::Empty),
            I::LocalGet(1),
            I::I32Const(1),
            stw(0),
            I::LocalGet(1),
            I::I32Const(1),
            stw(28),
            I::I32Const(0),
            I::GlobalSet(7),
            I::Return,
            I::End,
        ],
    );
    emit(&mut f, &[I::LocalGet(1), ld(4), I::LocalSet(3)]);
    row_addr(&mut f, 3, ft_base, FT_ROW, 2);
    emit(
        &mut f,
        &[
            // A start-up point, and the instance is not being closed.
            I::LocalGet(2),
            ld(60),
            I::LocalGet(1),
            ld(28),
            I::I32Const(2),
            I::I32And,
            I::I32Eqz,
            I::I32And,
            I::If(BlockType::Empty),
        ],
    );
    region_of(&mut f, 0, 2, 3);
    emit(
        &mut f,
        &[
            I::LocalGet(0),
            I::LocalGet(2),
            ld(56),
            I::LocalGet(3),
            I::LocalGet(2),
            ld(32),
            I::I32Add,
            I::Call(run_task),
            I::Drop,
            // pr_tick() names the instance (a park clears the running task).
            I::LocalGet(0),
            I::I32Const(1),
            I::I32Add,
            I::GlobalSet(7),
            I::Return,
            I::End,
            I::LocalGet(0),
            I::Call(release),
            I::I32Const(0),
            I::GlobalSet(7),
            I::End,
        ],
    );
    let cancel = next(defs, "pr_cancel", ty_i_v, f);

    let mut f = Function::new([]);
    emit(&mut f, &[I::GlobalGet(7), I::I32Const(1), I::I32Sub, I::End]);
    let tick = next(defs, "pr_tick", c.ty_i32_ret, f);
    let mut f = Function::new([]);
    emit(&mut f, &[I::GlobalGet(8), I::End]);
    let status = next(defs, "pr_status", c.ty_i32_ret, f);

    // pr_close_form(inst) -> status: `__QUIT__` ends the form's event loop, as
    // on the desktop; the region is released when the closing run is DONE (A2).
    let mut f = Function::new([(4, i32_)]); // row 1, ft 2, base 3, form 4
    require_below(&mut f, 0, c.total_instances);
    row_addr(&mut f, 0, tbl, INST_ROW, 1);
    require_idle(&mut f, 1, 4);
    emit(&mut f, &[I::LocalGet(1), ld(4), I::LocalSet(4)]);
    row_addr(&mut f, 4, ft_base, FT_ROW, 2);
    region_of(&mut f, 0, 2, 3);
    emit(
        &mut f,
        &[
            I::LocalGet(1),
            I::LocalGet(1),
            ld(28),
            I::I32Const(2),
            I::I32Or,
            stw(28),
            I::LocalGet(2),
            ld(36),
            I::If(BlockType::Empty),
            I::LocalGet(3),
            I::LocalGet(2),
            ld(40),
            I::LocalGet(2),
            ld(36),
            I::Call(rt("rt_move")),
            I::End,
            I::LocalGet(3),
            I::LocalGet(2),
            ld(28),
            I::I32Const(c.quit_addr as i32),
            I::I32Const(8),
            I::I32Const(c.quit_ev_addr as i32),
            I::I32Const(4),
            I::Call(rt("rt_deliver")),
            I::I32Const(0),
            I::GlobalSet(G_EVENT),
            I::LocalGet(0),
            I::LocalGet(1),
            ld(8),
            I::LocalGet(1),
            ld(12),
            I::Call(run_task),
            I::End,
        ],
    );
    let close = next(defs, "pr_close_form", ty_i_r, f);

    // pr_dbg_live / pr_dbg_region_used (A3).
    let dbg = |defs: &mut Vec<Def>, name: &str, bytes: bool| -> u32 {
        let mut f = Function::new([(3, i32_)]); // i 0, n 1, row 2
        emit(&mut f, &[I::Block(BlockType::Empty), I::Loop(BlockType::Empty), I::LocalGet(0), I::I32Const(c.total_instances as i32), I::I32GeU, I::BrIf(1)]);
        row_addr(&mut f, 0, tbl, INST_ROW, 2);
        emit(&mut f, &[I::LocalGet(2), ld(0), I::If(BlockType::Empty), I::LocalGet(1)]);
        if bytes {
            emit(&mut f, &[I::LocalGet(2), ld(4), I::I32Const(FT_ROW as i32), I::I32Mul, I::I32Const(ft_base as i32), I::I32Add, ld(12)]);
        } else {
            f.instruction(&I::I32Const(1));
        }
        emit(&mut f, &[I::I32Add, I::LocalSet(1), I::End, I::LocalGet(0), I::I32Const(1), I::I32Add, I::LocalSet(0), I::Br(0), I::End, I::End, I::LocalGet(1)]);
        // An instance's data includes its records in the file heap.
        if bytes && c.uses_heap {
            emit(&mut f, &[I::Call(rt("rt_heap_used")), I::I32Add]);
        }
        f.instruction(&I::End);
        let idx = n_imports + defs.len() as u32;
        defs.push(Def { name: name.into(), ty: c.ty_i32_ret, body: Body::Encoded(f) });
        idx
    };
    let live = dbg(defs, "pr_dbg_live", false);
    let used = dbg(defs, "pr_dbg_region_used", true);

    FormFns {
        exports: vec![
            ("pr_open_form", open),
            ("pr_close_form", close),
            ("pr_event", event),
            ("pr_resume", resume),
            ("pr_cancel", cancel),
            ("pr_tick", tick),
            ("pr_status", status),
            ("pr_dbg_live", live),
            ("pr_dbg_region_used", used),
        ],
    }
}

// ── the manifest (ABI v1 §10), through the ABI crate (WASM-010 D3) ──────────

struct ManifestInput<'a, 'p> {
    opts: &'a Options,
    forms: &'a [FormSpec<'p>],
    built: &'a [Built],
    events: &'a [String],
    names: &'a [String],
    builtins: Vec<String>,
    rt_functions: usize,
    rt_bytes: usize,
    imports: Vec<String>,
    arena_total: u32,
    data_pages: u32,
    total_instances: u32,
}

fn build_manifest(m: &ManifestInput<'_, '_>) -> cobolt_wasm_abi::manifest::Manifest {
    use cobolt_wasm_abi::manifest::*;
    let forms = m
        .forms
        .iter()
        .zip(m.built)
        .enumerate()
        .map(|(fi, (f, b))| {
            let controls: Vec<ControlEntry> =
                f.controls.iter().enumerate().map(|(i, (n, t))| ControlEntry { id: 3 + i as u32, name: n.clone(), kind: t.clone() }).collect();
            let handlers = f
                .bindings
                .iter()
                .filter_map(|(c, e)| {
                    let ctrl = if c.is_empty() { Some(0) } else { controls.iter().find(|x| x.name.eq_ignore_ascii_case(c)).map(|x| x.id) }?;
                    let event = m.events.iter().position(|x| x == e)? as u32;
                    Some(HandlerEntry { ctrl, event, entry: 0 })
                })
                .collect();
            let first_layer = 3 + controls.len() as u32;
            let layers = f.layers.iter().enumerate().map(|(k, n)| LayerEntry { id: first_layer + k as u32, name: n.clone(), index: k as u32 }).collect();
            FormEntry { id: fi as u32, name: f.name.clone(), main: f.main, instances: f.instances.max(1), region_size: b.region_size, controls, handlers, layers }
        })
        .collect();
    Manifest {
        abi: ABI_VERSION as u32,
        generator: format!("cobolt-wasm {}", env!("CARGO_PKG_VERSION")),
        project: m.opts.name.clone(),
        variant: VariantInfo { multi_memory: m.opts.multi_memory, conformance: m.opts.conformance },
        limits: Limits {
            arena_size: m.arena_total,
            batch_size: m.opts.arena.batch,
            answer_size: m.opts.arena.answer,
            frame_size: m.opts.arena.frame,
            budget_blocks: m.opts.budget.max(0) as u32,
            data_pages: m.data_pages,
            tasks: m.total_instances,
        },
        names: m.names.to_vec(),
        events: m.events.to_vec(),
        forms,
        builtins: m.builtins.clone(),
        imports: m.imports.clone(),
        sizes: Sizes { module: 0, rt_functions: m.rt_functions as u64, rt_bytes: m.rt_bytes as u64 },
    }
}
