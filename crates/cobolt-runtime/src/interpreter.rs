// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tree-walking interpreter for COBOL programs.
//!
//! # Execution model
//!
//! The interpreter maintains a flat *paragraph map* built from the PROCEDURE
//! DIVISION.  `run()` iterates through paragraphs in declaration order,
//! executing statements inside each one.  Control-flow signals (GO TO, STOP
//! RUN, GOBACK) are propagated as special `RuntimeError` variants and caught at
//! the appropriate level.
//!
//! ## Control flow
//!
//! | Signal                        | Mechanism                          |
//! |-------------------------------|------------------------------------|
//! | STOP RUN                      | `Err(RuntimeError::StopRun)` → `run()` |
//! | GOBACK                        | `Err(RuntimeError::GoBack)` → `run()` |
//! | GO TO *paragraph*             | `Err(RuntimeError::GoTo{..})` → `run()` |
//! | PERFORM *paragraph*           | Recursive `exec_stmts()` call      |
//! | PERFORM … UNTIL/TIMES/VARYING | Rust loop inside `exec_perform`    |

use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::mpsc;
use std::sync::Arc;

use cobolt_ast::{
    expr::{
        ArithOp, CmpOp, Condition, DataClass, Expr, FigurativeConstant, Literal, SignCond, UnaryOp,
    },
    program::{AccessMode, AlternateKey, FileOrganization, ProcedureBody, Program, UseMode},
    stmt::{
        AcceptSource, CallArg, EvalSubject, ExitKind, InspectRegion, InspectSpec, OpenMode,
        PerformTarget, ReplaceWhat, Stmt, TallyFor, UnstringTarget, VaryingAfter, WhenClause,
        WhenValue,
    },
};
use cobolt_lexer::Span;

use crate::{
    channels::{FormEvent, StateUpdate},
    db_runtime::DbRegistry,
    environment::{new_external_store, CobolEnvironment, ExternalStore},
    error::RuntimeError,
    exec_rust,
    objects::{ObjectRegistry, PathSeg, PropertyValue},
    value::{CobolNumeric, CobolValue},
};

// ── Inline-PERFORM loop control ─────────────────────────────────────────────

/// Outcome of running one inline-PERFORM loop body.
enum LoopStep {
    /// Continue to the next iteration (normal completion or `EXIT PERFORM CYCLE`).
    Continue,
    /// Terminate the loop (`EXIT PERFORM`).
    Break,
    /// A real error / non-loop control signal that must propagate.
    Err(RuntimeError),
}

// ── Databinding diagnostics ───────────────────────────────────────────────────

/// `true` while the project's *databind trace* diagnostic is on, i.e. when
/// `COBOLT_DATABIND_TRACE` holds a truthy value (`1`/`true`/`on`). Presence
/// alone is not enough: the IDE always sets the var (to `0` when the diagnostic
/// is off) and re-syncs it while the form runs, so the value is read on every
/// call rather than cached.
fn databind_trace_enabled() -> bool {
    std::env::var("COBOLT_DATABIND_TRACE")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

/// Append one line to `databinding.log` in the platform's diagnostics directory
/// (see [`crate::diag_path`] — `/tmp` on Linux/macOS, `%TEMP%` on Windows).
/// Best-effort: a failed write is ignored. Call through [`databind_trace!`],
/// which keeps the write (and the formatting) behind the diagnostic.
fn databind_trace_write(args: std::fmt::Arguments<'_>) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::diag_path::diagnostics_file("databinding.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{args}");
    }
}

/// Percent-encode a query-string value per RFC 3986 (unreserved characters —
/// ALPHA / DIGIT / `-` / `.` / `_` / `~` — pass through, everything else
/// becomes `%XX`). Spec 039 T15: the WebSearch `SEARCH` verb's own query,
/// search-engine id, and resolved API key all flow through this — none of
/// this project's existing HTTP helpers (`ureq`, `http_runtime::HttpClient`)
/// build a URL from parts, so a multi-word query would otherwise truncate at
/// its first unescaped space (the same limitation the generated `<id>-
/// SEARCH` COBOL paragraph's own comment documents, T14).
fn percent_encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Append one line to `/tmp/databinding.log`, but only while the databind trace
/// diagnostic is on — these sites run per row per refresh, and the ungated
/// writes they replace grew the file without bound.
macro_rules! databind_trace {
    ($($arg:tt)*) => {
        if databind_trace_enabled() {
            databind_trace_write(format_args!($($arg)*));
        }
    };
}

// ── File I/O ──────────────────────────────────────────────────────────────────

/// Static description of a SELECT … ASSIGN file (from FILE-CONTROL + FD).
#[derive(Debug, Clone)]
struct FileSpec {
    /// ASSIGN target — either a literal path or the name of a data item that
    /// holds the path (resolved at OPEN time).
    assign: String,
    organization: FileOrganization,
    /// ACCESS MODE (SEQUENTIAL / RANDOM / DYNAMIC).
    access: AccessMode,
    /// FILE STATUS data-item name (receives the 2-char status code), if any.
    status_field: Option<String>,
    /// The FD's 01-level record names (the buffer WRITE/READ act on).
    record_names: Vec<String>,
    /// RECORD KEY field name (INDEXED files).
    record_key: Option<String>,
    /// ALTERNATE RECORD KEY entries (INDEXED files).
    alternate_keys: Vec<AlternateKey>,
    /// STORAGE IS MEMORY | DISK (INDEXED files).
    storage_mode: cobolt_ast::program::StorageMode,
    /// WITH COMPRESSION — compress stored record data.
    data_compressing: bool,
    /// WITH PERSISTENCE — for STORAGE IS MEMORY, save to disk on CLOSE.
    persist: bool,
    /// Byte layout of the primary FD record (subfield offsets/widths).
    layout: crate::files::RecordLayout,
}

/// A currently-open file handle. The variant follows the file's ORGANIZATION,
/// so the verbs dispatch by file type (RELATIVE will add a variant here).
enum OpenFile {
    /// SEQUENTIAL / LINE SEQUENTIAL, opened for output/extend.
    Writer {
        w: std::io::BufWriter<std::fs::File>,
        org: FileOrganization,
    },
    /// SEQUENTIAL / LINE SEQUENTIAL, opened for input.
    Reader {
        r: std::io::BufReader<std::fs::File>,
        org: FileOrganization,
    },
    /// INDEXED (ISAM) — a keyed engine (in-memory or on-disk) handles every
    /// verb. The concrete backend is chosen by STORAGE MODE.
    Indexed(Box<dyn crate::indexed::IndexedStore>),
}

// ── Nested program registry ───────────────────────────────────────────────────

/// A compiled representation of one COBOL-85 nested program.
///
/// Nested programs share the outer program's environment (GLOBAL items are
/// naturally accessible because they live in the same `CobolEnvironment`
/// store).  Each nested program may also declare its own WORKING-STORAGE;
/// those items are pushed onto the outer env for the duration of the call
/// and removed on GOBACK.
#[derive(Debug)]
struct NestedProgram {
    /// Paragraph name → statement list.
    para_map: IndexMap<String, Vec<Stmt>>,
    /// Paragraph names in declaration order.
    para_order: Vec<String>,
    /// Local WORKING-STORAGE items declared inside this nested program.
    /// Format: `(uppercase_name, initial_value)`.
    local_items: Vec<(String, CobolValue)>,
    /// Symbol metadata for the local items above, used by debugger snapshots.
    local_symbols: Vec<(String, crate::environment::ItemSym)>,
    /// `PROCEDURE DIVISION USING …` LINKAGE parameter names (as written), in
    /// order — bound to the caller's `CALL … USING` arguments.
    using: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct BindingRuntimeState {
    loaded: bool,
    populated: bool,
    dirty: bool,
    read_only: bool,
    row_key: String,
    pending_value: String,
    last_status: String,
}

/// Recursively register a `Program` and all of its `nested_programs` into
/// `registry`, keyed by the program-id (uppercase).
fn register_nested(prog: &Program, registry: &mut HashMap<String, NestedProgram>) {
    let (para_map, para_order) = build_para_map(&prog.procedure.body);

    // Collect this program's own local data items (everything in its DATA
    // DIVISION — they will be added to the env as a scope overlay on call).
    let local_items: Vec<(String, CobolValue)> = if let Some(data) = &prog.data {
        let local_env = CobolEnvironment::from_data_division_with_origin(
            data,
            prog.decimal_comma,
            &prog.identification.program_id,
        );
        local_env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        Vec::new()
    };
    let local_symbols: Vec<(String, crate::environment::ItemSym)> = if let Some(data) = &prog.data {
        CobolEnvironment::from_data_division_with_origin(
            data,
            prog.decimal_comma,
            &prog.identification.program_id,
        )
        .symbol_entries()
    } else {
        Vec::new()
    };

    let key = prog.identification.program_id.to_ascii_uppercase();
    let using = prog.procedure.using.clone();
    registry.insert(
        key,
        NestedProgram {
            para_map,
            para_order,
            local_items,
            local_symbols,
            using,
        },
    );

    // Recurse into any nested-programs declared inside this one.
    for child in &prog.nested_programs {
        register_nested(child, registry);
    }
}

/// Reconcile a freshly built environment's `EXTERNAL` items with the run-unit
/// store: adopt any value already published by an earlier program activation,
/// otherwise publish this program's initial value as the run-unit copy. Keys are
/// the environment's canonical storage keys (already EXTERNAL-filtered).
fn seed_external_store(env: &mut CobolEnvironment, store: &ExternalStore) {
    let names: Vec<String> = env.external_names().iter().cloned().collect();
    if names.is_empty() {
        return;
    }
    let mut guard = store.lock().unwrap_or_else(|e| e.into_inner());
    for name in names {
        if let Some(shared) = guard.get(&name).cloned() {
            env.raw_set(&name, shared); // adopt the existing run-unit value
        } else if let Some(v) = env.raw_get(&name).cloned() {
            guard.insert(name, v); // publish our initial value
        }
    }
}

impl Interpreter {
    /// Install this program's compiled `EXEC RUST` blocks (spec 041 R2).
    ///
    /// Called by the generated `main.rs` with the generated module's `register`
    /// function, before [`Interpreter::run`]. Takes the installer rather than a
    /// built table so the generated crate never has to name
    /// [`crate::exec_rust::ExecRustRegistry`] itself.
    ///
    /// Nothing registers these in a plain interpreted run, which is exactly why
    /// executing a block without building first is a hard error and not a
    /// silent no-op.
    pub fn register_exec_rust_blocks(
        &mut self,
        install: impl FnOnce(&mut crate::exec_rust::ExecRustRegistry),
    ) {
        install(&mut self.exec_rust);
    }
}

/// A program's `REPOSITORY` bindings, keyed by uppercase COBOL class name.
fn repository_map(program: &Program) -> HashMap<String, String> {
    program
        .repository
        .iter()
        .map(|(k, v)| (k.to_ascii_uppercase(), v.clone()))
        .collect()
}

/// Construct the program's `OBJECT REFERENCE` items (spec 005 Rust-FFI bridge):
/// resolve each item's class via the program's `REPOSITORY` bindings to a Rust
/// type, create the object (seeded from the item's `VALUE`, if any), store the
/// handle id in the environment, and return `item-key → rust-type`.
fn build_object_refs(
    program: &Program,
    env: &mut CobolEnvironment,
    bridge: &mut crate::rust_bridge::RustBridge,
) -> HashMap<String, String> {
    let repo = repository_map(program);
    let mut refs = HashMap::new();
    for (key, id) in create_object_refs(program, &repo, bridge, &mut refs) {
        env.set_i64(&key, id);
    }
    refs
}

/// Give every `USAGE OBJECT REFERENCE` item declared **inside a nested program**
/// its own live object, and record the handle in that program's local template.
///
/// Every RAD event handler is a nested program with its own DATA DIVISION, so
/// this is where a handler's `OBJECT REFERENCE` items live — and only the
/// outermost program's were ever constructed, leaving a handler-local item with
/// no handle at all ("EXEC RUST cannot bind …: handle 0 is not live").
///
/// The handle goes into the program's local template, not the environment: a
/// nested program's locals are pushed on CALL and popped on GOBACK, so a handle
/// written straight into the shared environment would leak the item into the
/// containing program's scope. Seeding the template also gives the object the
/// same static lifetime as the item — one object per item, preserved across
/// calls, exactly like a top-level one.
///
/// Classes resolve against the containing program's `REPOSITORY` as well as the
/// nested program's own, since a generated handler declares none of its own.
fn seed_nested_object_refs(
    program: &Program,
    outer_repo: &HashMap<String, String>,
    registry: &mut HashMap<String, NestedProgram>,
    bridge: &mut crate::rust_bridge::RustBridge,
    refs: &mut HashMap<String, String>,
) {
    for nested in &program.nested_programs {
        let mut repo = outer_repo.clone();
        repo.extend(repository_map(nested));

        let handles = create_object_refs(nested, &repo, bridge, refs);
        if let Some(np) = registry.get_mut(&nested.identification.program_id.to_ascii_uppercase()) {
            for (key, id) in handles {
                let handle = CobolValue::from_i64(id);
                match np
                    .local_items
                    .iter_mut()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&key))
                {
                    Some((_, slot)) => *slot = handle,
                    None => np.local_items.push((key, handle)),
                }
            }
        }

        seed_nested_object_refs(nested, &repo, registry, bridge, refs);
    }
}

/// Create one object per `OBJECT REFERENCE` item declared in `program`'s own
/// DATA DIVISION, recording `item-key → rust-type` in `refs`. Returns the
/// `(item-key, handle id)` pairs for the caller to store where the item lives.
fn create_object_refs(
    program: &Program,
    repo: &HashMap<String, String>,
    bridge: &mut crate::rust_bridge::RustBridge,
    refs: &mut HashMap<String, String>,
) -> Vec<(String, i64)> {
    use cobolt_ast::program::DataSection;
    let mut handles = Vec::new();
    if let Some(data) = &program.data {
        for section in &data.sections {
            if let DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) = section
            {
                for decl in items {
                    collect_object_ref(decl, repo, bridge, refs, &mut handles);
                }
            }
        }
    }
    handles
}

fn collect_object_ref(
    decl: &cobolt_ast::data::DataDecl,
    repo: &HashMap<String, String>,
    bridge: &mut crate::rust_bridge::RustBridge,
    refs: &mut HashMap<String, String>,
    handles: &mut Vec<(String, i64)>,
) {
    use cobolt_ast::data::Usage;
    if matches!(decl.usage, Usage::ObjectReference) {
        if let (Some(name), Some(class)) = (&decl.name, &decl.object_class) {
            if let Some(rust_type) = repo.get(&class.to_ascii_uppercase()) {
                let args = initial_bridge_args(&decl.value);
                // A class the curated bridge cannot construct still gets a real,
                // unique handle (spec 041 R22): a developer-defined type is built
                // by the first compiled block that binds it, and the id 0 this
                // used to hand out aliased every such item onto one another.
                let id = match bridge.create(rust_type, &args) {
                    Ok(crate::rust_bridge::BridgeValue::Handle(id)) => id,
                    _ => bridge.create_uninitialised(rust_type),
                };
                let key = name.to_ascii_uppercase();
                refs.insert(key.clone(), rust_type.clone());
                handles.push((key, id));
            }
        }
    }
    for child in &decl.children {
        collect_object_ref(child, repo, bridge, refs, handles);
    }
}

/// Initial constructor argument(s) from an `OBJECT REFERENCE` item's VALUE clause.
fn initial_bridge_args(value: &Option<Literal>) -> Vec<crate::rust_bridge::BridgeValue> {
    use crate::rust_bridge::BridgeValue;
    match value {
        Some(Literal::String(s)) => vec![BridgeValue::Str(s.clone())],
        Some(Literal::Integer(n)) => vec![BridgeValue::Int(*n)],
        _ => Vec::new(),
    }
}

/// Marshal a COBOL value into a bridge value for a Rust call argument.
fn cobol_to_bridge(v: &CobolValue) -> crate::rust_bridge::BridgeValue {
    use crate::rust_bridge::BridgeValue;
    match v {
        CobolValue::Numeric(_) => match v.as_i64() {
            Some(i) => BridgeValue::Int(i),
            None => BridgeValue::Float(v.as_f64()),
        },
        CobolValue::Float(f) => BridgeValue::Float(*f),
        CobolValue::String { .. } => BridgeValue::Str(v.as_display_string().trim_end().to_string()),
        CobolValue::Unset => BridgeValue::Null,
    }
}

/// Marshal a bridge result back into a COBOL value.
fn bridge_to_cobol(b: crate::rust_bridge::BridgeValue) -> CobolValue {
    use crate::rust_bridge::BridgeValue;
    match b {
        BridgeValue::Int(n) => CobolValue::from_i64(n),
        BridgeValue::Float(x) => CobolValue::from_f64(x),
        BridgeValue::Bool(t) => CobolValue::from_i64(t as i64),
        BridgeValue::Handle(id) => CobolValue::from_i64(id),
        BridgeValue::Str(s) => {
            let n = s.len();
            CobolValue::from_str(&s, n)
        }
        BridgeValue::Null => CobolValue::from_str("", 0),
    }
}

/// Build the file registry from the program's FILE-CONTROL (SELECT) entries and
/// FILE SECTION (FD) records: `(logical name → spec, record name → file name)`.
fn build_file_specs(program: &Program) -> (HashMap<String, FileSpec>, HashMap<String, String>) {
    use cobolt_ast::program::DataSection;

    let mut specs: HashMap<String, FileSpec> = HashMap::new();
    let mut record_to_file: HashMap<String, String> = HashMap::new();

    // Collect each FD's 01-record names + the primary record's byte layout.
    let mut fd_records: HashMap<String, Vec<String>> = HashMap::new();
    let mut fd_layout: HashMap<String, crate::files::RecordLayout> = HashMap::new();
    if let Some(data) = &program.data {
        for section in &data.sections {
            if let DataSection::FileSection(fds) = section {
                for fd in fds {
                    let names: Vec<String> = fd
                        .records
                        .iter()
                        .filter_map(|r| r.name.clone())
                        .map(|n| n.to_ascii_uppercase())
                        .collect();
                    let fkey = fd.name.to_ascii_uppercase();
                    if let Some(first) = fd.records.first() {
                        fd_layout.insert(fkey.clone(), crate::files::compute_layout(first));
                    }
                    fd_records.insert(fkey, names);
                }
            }
        }
    }

    if let Some(env) = &program.environment {
        if let Some(io) = &env.input_output {
            for fc in &io.file_controls {
                let key = fc.name.to_ascii_uppercase();
                let record_names = fd_records.get(&key).cloned().unwrap_or_default();
                for rn in &record_names {
                    record_to_file.insert(rn.clone(), key.clone());
                }
                specs.insert(
                    key.clone(),
                    FileSpec {
                        assign: fc.assign.clone(),
                        organization: fc.organization,
                        access: fc.access,
                        status_field: fc.file_status.clone().map(|s| s.to_ascii_uppercase()),
                        record_names,
                        record_key: fc.record_key.clone().map(|s| s.to_ascii_uppercase()),
                        alternate_keys: fc.alternate_keys.clone(),
                        storage_mode: fc.storage_mode,
                        data_compressing: fc.data_compressing,
                        persist: fc.persist,
                        layout: fd_layout.get(&key).cloned().unwrap_or_default(),
                    },
                );
            }
        }
    }

    (specs, record_to_file)
}

/// Map the AST open mode onto the indexed engine's.
fn map_open_mode(m: OpenMode) -> crate::indexed::OpenMode {
    use crate::indexed::OpenMode as I;
    match m {
        OpenMode::Input => I::Input,
        OpenMode::Output => I::Output,
        OpenMode::Extend => I::Extend,
        OpenMode::InputOutput => I::Io,
    }
}

/// Build an indexed engine for `spec` from its layout + key fields. The concrete
/// backend follows `STORAGE MODE`: MEMORY → the in-RAM engine; DISK → the
/// persistent paged B+tree engine. `WITH COMPRESSION` applies to both.
fn make_indexed_engine(
    spec: &FileSpec,
    path: &str,
    engine: crate::indexed::IndexedEngine,
    log_level: crate::indexed_log::LogLevel,
    log_format: crate::indexed_log::LogFormat,
) -> Box<dyn crate::indexed::IndexedStore> {
    use crate::indexed::{IndexedEngine, IndexedFile, KeySpec};
    use crate::indexed_disk::DiskIndexedFile;
    use crate::indexed_redb::RedbIndexedFile;
    use cobolt_ast::program::StorageMode;
    let layout = &spec.layout;
    let reclen = layout.len.max(1);
    let primary = spec
        .record_key
        .as_deref()
        .and_then(|k| layout.key_spec(k, false))
        .unwrap_or(KeySpec {
            offset: 0,
            len: reclen,
            duplicates: false,
        });
    // Build alternate KeySpecs and their field names in lock-step (skipping any
    // alternate key field that isn't present in the FD record layout).
    let mut alts = Vec::new();
    let mut names: Vec<Option<String>> = vec![spec.record_key.clone()];
    for ak in &spec.alternate_keys {
        if let Some(ks) = layout.key_spec(&ak.field, ak.with_duplicates) {
            alts.push(ks);
            names.push(Some(ak.field.clone()));
        }
    }
    let compressing = spec.data_compressing;
    // The redb engine is a disk substrate; selecting it routes DISK storage to
    // the crash-safe ACID engine. MEMORY storage always uses the in-RAM engine.
    if engine == IndexedEngine::Redb && spec.storage_mode == StorageMode::Disk {
        let mut e = RedbIndexedFile::new(path, reclen, primary, alts);
        e.set_key_names(names);
        e.set_compressing(compressing);
        e.set_log_level(log_level);
        e.set_log_format(log_format);
        return Box::new(e);
    }
    match spec.storage_mode {
        StorageMode::Memory => {
            let mut e = IndexedFile::new(path, reclen, primary, alts);
            e.set_key_names(names);
            e.set_compressing(compressing);
            e.set_persist(spec.persist);
            Box::new(e)
        }
        StorageMode::Disk => {
            // Rust / RM-COBOL / Fujitsu currently share the PRCIDXD1 container.
            let mut e = DiskIndexedFile::new(path, reclen, primary, alts);
            e.set_key_names(names);
            e.set_compressing(compressing);
            Box::new(e)
        }
    }
}

/// Translate a COBOL relational operator (from `START`) to a key search op.
fn map_start_op(op: cobolt_ast::expr::CmpOp) -> crate::indexed::StartOp {
    use crate::indexed::StartOp as S;
    use cobolt_ast::expr::CmpOp;
    match op {
        CmpOp::Eq => S::Eq,
        CmpOp::Gt => S::Gt,
        CmpOp::Ge => S::Ge,
        CmpOp::Lt => S::Lt,
        CmpOp::Le => S::Le,
        CmpOp::Ne => S::Ge, // not standard for START; treat as ≥
    }
}

// ── Interpreter ───────────────────────────────────────────────────────────────

/// What `COBOL-WAIT-EVENT` should present next (spec 032).
enum WaitOutcome {
    /// A real UI event from the form window.
    Ui(FormEvent),
    /// An async lifecycle event to dispatch: `(control-id, event-id)`.
    AsyncDispatch(String, String),
    /// The event channel is gone (UI closed) or absent (CLI mode).
    Disconnected,
}

/// Tree-walking COBOL interpreter.
pub struct Interpreter {
    /// The parsed program (retained for metadata access).
    pub program: Program,
    /// Runtime data store — all DATA DIVISION items live here.
    pub env: CobolEnvironment,
    /// Run-unit-wide EXTERNAL store (spec 005). EXTERNAL items are seeded into
    /// it at construction and synced at run boundaries; clone the `Arc` (via
    /// [`external_store`](Self::external_store)) to share it with another
    /// interpreter so a multi-module run unit sees one physical copy.
    external_store: ExternalStore,
    /// Curated Rust-FFI bridge: owns the live Rust objects referenced from COBOL
    /// `OBJECT REFERENCE` items (spec 005 T9/T10).
    rust_bridge: crate::rust_bridge::RustBridge,
    /// `OBJECT REFERENCE` item key (uppercase) → bound Rust type external name
    /// (e.g. `S` → `Rust.String`). The item's storage holds the bridge handle id.
    object_refs: HashMap<String, String>,
    /// PowerRustCOBOL form/control object registry.
    pub objects: ObjectRegistry,
    /// Compiled `EXEC RUST` blocks, registered by the generated `main.rs`
    /// before the run (spec 041 R2). Empty in a plain interpreted run, which is
    /// why executing a block without building first is a hard error rather
    /// than a silent no-op.
    pub exec_rust: crate::exec_rust::ExecRustRegistry,
    /// Property "shadows": a receiving property reference used by any verb is
    /// resolved to a synthetic env item preloaded with the property's current
    /// value; after each statement these are written back to the object store.
    /// Maps synthetic-env-key → (control, member-access path). The path is a
    /// single `[Prop(name)]` for a flat property, or a deeper sequence for a
    /// nested place (`Grid::Rows(I)::Value`) (spec 011).
    property_shadows: std::collections::HashMap<String, (String, Vec<PathSeg>)>,
    /// Paragraph name (uppercase) → statement list.
    para_map: IndexMap<String, Vec<Stmt>>,
    /// Paragraph names in declaration order (for fall-through and THRU ranges).
    para_order: Vec<String>,
    /// COBOL-85 nested programs: program-id (uppercase) → compiled program.
    nested_registry: HashMap<String, NestedProgram>,
    /// Persistent local WORKING-STORAGE per nested program (program-id uppercase →
    /// last-seen local values). Procedures are **static by default** (spec 009
    /// R10): a program's locals are initialised once (from its DATA DIVISION) on
    /// the first CALL and **preserved** across subsequent CALLs; `CANCEL` drops the
    /// entry so the next CALL re-initialises. `INITIALIZE` (unchanged) is the
    /// developer's in-procedure reset.
    program_locals: HashMap<String, Vec<(String, CobolValue)>>,
    /// Current PERFORM nesting depth (overflow guard).
    perform_depth: usize,
    /// Database runtime engine (Phase 8) — manages SQLite connections.
    db: DbRegistry,
    /// HTTP client (Phase 10) — manages persistent headers and sends requests.
    http: crate::http_runtime::HttpClient,
    /// Generated data-binding helper CALL state, keyed by binding id.
    binding_states: HashMap<String, BindingRuntimeState>,

    // ── GUI Form Runtime channels (Phase 6) ───────────────────────────────────
    /// Receives UI events (button clicks, text changes, etc.) from the form window.
    /// When `Some`, `COBOL-WAIT-EVENT` blocks on `recv()` instead of quitting.
    event_rx: Option<mpsc::Receiver<FormEvent>>,
    /// Receives UI-driven property changes (slider drag, text edit, combo
    /// select, …) from the form window, so `COBOL-GET-PROPERTY` / `"Value" OF
    /// Ctrl` see the live value a handler is responding to. Drained into the
    /// object registry by `COBOL-WAIT-EVENT` right before a handler dispatches.
    input_rx: Option<mpsc::Receiver<StateUpdate>>,
    /// Sends property-change notifications to the form window UI thread.
    /// Used by `COBOL-SET-PROPERTY`.
    state_tx: Option<mpsc::Sender<StateUpdate>>,
    /// Sends DISPLAY output to the IDE output panel (instead of stdout).
    display_tx: Option<mpsc::Sender<String>>,

    // ── 037 Multi-form window supervisor seam ─────────────────────────────────
    /// Requests to the window supervisor (OpenFormSync/Async, handle methods,
    /// self close). `None` ⇒ single-form runtime (calls become runtime errors).
    form_host_tx: Option<mpsc::Sender<crate::form_host::FormRequest>>,
    /// The supervisor's handle for THIS interpreter's window.
    self_window_handle: String,
    /// The form's own object name — `me::` resolves to it (spec 037 D4).
    self_form_object: Option<String>,
    /// 049 R28/R29 — the handle of the form that LOADED or OPENED this one:
    /// what `super` resolves to. `None` = no parent (the main form, a console
    /// run, or an async opener that closed — R32/R46).
    super_window_handle: Option<String>,
    /// Handles of forms that CLOSED, broadcast by the host; drained lazily so
    /// windowHandler data-items become NULL (R24).
    form_closed_rx: Option<mpsc::Receiver<String>>,
    /// windowHandler variables: UPPER var name → live handle id (`None` =
    /// NULL — the form closed or was never opened).
    window_handle_vars: HashMap<String, Option<String>>,
    /// Cooperative cancellation flag shared with the host (`FormRuntime`). When
    /// set, execution aborts between statements with `RuntimeError::Cancelled`
    /// so a looping / long-running handler can never hang the UI thread on
    /// close, relaunch, or exit.
    cancel: Option<Arc<AtomicBool>>,
    /// Depth of the UI→interpreter event queue, shared with the host so it can
    /// coalesce timer ticks (skip enqueuing a new `onTick` while a previous
    /// event is still outstanding). Decremented as each `COBOL-WAIT-EVENT`
    /// consumes an event.
    event_pending: Option<Arc<AtomicUsize>>,
    /// Live chart data pushed by the `COBOL-CHART-*` runtime calls: control id →
    /// ordered (label, value) points. Serialised into the control's `__ChartData`
    /// property (via a `StateUpdate`) so the GUI chart renderer plots it. Empty ⇒
    /// the designer's representative sample preview is shown instead.
    chart_data: HashMap<String, Vec<(String, f64)>>,

    // ── Async I/O operations (spec 032) ───────────────────────────────────────
    /// Cloned into each background worker thread; the worker posts its
    /// `AsyncOpResult` here when the blocking call finishes.
    async_result_tx: mpsc::Sender<crate::async_op::AsyncOpResult>,
    /// Drained (non-blocking) by `COBOL-WAIT-EVENT` to apply completed async
    /// operations and enqueue their lifecycle events.
    async_result_rx: mpsc::Receiver<crate::async_op::AsyncOpResult>,
    /// In-flight operation per control id — at most one at a time (a second
    /// start while `Busy` is rejected).
    async_pending: HashMap<String, crate::async_op::PendingOp>,
    /// Live generation per control. A delivered result whose generation no
    /// longer matches (cancelled / timed-out / superseded) is discarded.
    async_generations: HashMap<String, Arc<AtomicU64>>,
    /// Completed async operations awaiting dispatch to COBOL as
    /// `(control-id, event-id)`, one presented per `COBOL-WAIT-EVENT` return.
    async_dispatch_queue: std::collections::VecDeque<(String, String)>,

    // ── Debugger channels (Phase 7) ───────────────────────────────────────────
    /// Receives `DebugCmd` from the IDE debugger panel (Continue, StepOver, Pause).
    debug_cmd_rx: Option<mpsc::Receiver<crate::debugger::DebugCmd>>,
    /// Sends `DebugEvent` to the IDE debugger panel (Paused, Resumed, Finished).
    debug_event_tx: Option<mpsc::Sender<crate::debugger::DebugEvent>>,
    /// Active breakpoints shared between the IDE and the interpreter.
    breakpoints: Option<crate::debugger::Breakpoints>,
    /// When `true`, pause before the very next statement (StepOver mode).
    debug_stepping: bool,
    /// Name of the paragraph currently being executed (for Paused events).
    current_paragraph: String,

    // ── File I/O ──────────────────────────────────────────────────────────────
    /// Logical file name → static SELECT/FD description.
    file_specs: HashMap<String, FileSpec>,
    /// FD record (01) name → owning logical file name.
    record_to_file: HashMap<String, String>,
    /// Logical file name → currently-open handle.
    open_files: HashMap<String, OpenFile>,
    /// Selected indexed (ISAM) file engine (default: the built-in Rust engine).
    indexed_engine: crate::indexed::IndexedEngine,
    /// Per-file INDEXED observability log level (redb engine; default Off).
    indexed_log_level: crate::indexed_log::LogLevel,
    /// INDEXED observability log line format (logfmt text or NDJSON).
    indexed_log_format: crate::indexed_log::LogFormat,
    /// SORT/MERGE work buffers (SD file name → released/merged record bytes).
    sort_buffers: HashMap<String, Vec<Vec<u8>>>,
    /// RETURN cursor per SD file (index of the next record to hand back).
    sort_cursors: HashMap<String, usize>,
    /// `ALTER` overrides: paragraph name → the `GO TO` target it now proceeds to.
    alter_map: HashMap<String, String>,
    /// The COBOL program's command-line arguments (excludes the program name).
    program_args: Vec<String>,
    /// 1-based argument pointer for `ACCEPT … FROM ARGUMENT-VALUE`
    /// (set by `DISPLAY n UPON ARGUMENT-NUMBER`).
    argument_pointer: usize,
    /// Variable name set by `DISPLAY "VAR" UPON ENVIRONMENT-NAME`, read back by
    /// `ACCEPT … FROM ENVIRONMENT-VALUE`.
    env_name_register: String,
    /// `USE AFTER STANDARD ERROR` declarative handlers (top-level program).
    declaratives: Vec<DeclHandler>,
    /// Re-entrancy guard so a declarative's own I/O cannot re-trigger it.
    in_declarative: bool,
    /// Logical file name → the mode it was last OPENed with (for mode-based USE).
    open_modes: HashMap<String, OpenMode>,
}

/// A runtime-ready `USE AFTER STANDARD ERROR` handler: which files / open-modes
/// it covers and the statements to run when a matching I/O error occurs.
#[derive(Clone)]
struct DeclHandler {
    files: Vec<String>,
    modes: Vec<UseMode>,
    catch_all: bool,
    stmts: Vec<Stmt>,
}

const MAX_PERFORM_DEPTH: usize = 512;

impl Interpreter {
    // ── Construction ──────────────────────────────────────────────────────────

    /// Create a new interpreter from a parsed program.
    ///
    /// The DATA DIVISION is walked to initialise all data items to their
    /// default / VALUE clause values. A private run-unit EXTERNAL store is
    /// created; use [`with_external_store`](Self::with_external_store) to share
    /// one across several interpreters (a multi-module run unit).
    pub fn new(program: Program) -> Self {
        Self::with_external_store(program, new_external_store())
    }

    /// Like [`new`](Self::new), but joins an existing run-unit EXTERNAL store so
    /// that `EXTERNAL` data is the same physical copy across every interpreter
    /// built with the same store.
    pub fn with_external_store(program: Program, external_store: ExternalStore) -> Self {
        let mut env = if let Some(data) = &program.data {
            CobolEnvironment::from_data_division_with_origin(
                data,
                program.decimal_comma,
                &program.identification.program_id,
            )
        } else {
            CobolEnvironment::new()
        };
        // Reconcile this program's EXTERNAL items with the run unit: adopt any
        // value already published by an earlier activation, else publish ours.
        seed_external_store(&mut env, &external_store);
        // Construct the program's Rust-FFI object references (spec 005): each
        // `OBJECT REFERENCE` item gets a live Rust object (from its VALUE, if any)
        // and its handle id stored in the environment.
        let mut rust_bridge = crate::rust_bridge::RustBridge::new();
        let mut object_refs = build_object_refs(&program, &mut env, &mut rust_bridge);
        let (para_map, para_order) = build_para_map(&program.procedure.body);

        // Register all COBOL-85 nested programs (recursively).
        let mut nested_registry: HashMap<String, NestedProgram> = HashMap::new();
        for nested in &program.nested_programs {
            register_nested(nested, &mut nested_registry);
        }
        // A nested program — every RAD event handler is one — declares its own
        // `OBJECT REFERENCE` items; they need objects too, seeded into the
        // program's local template so they are in place from its first CALL.
        seed_nested_object_refs(
            &program,
            &repository_map(&program),
            &mut nested_registry,
            &mut rust_bridge,
            &mut object_refs,
        );

        let (file_specs, record_to_file) = build_file_specs(&program);

        // Flatten the parsed DECLARATIVES into runtime-ready handlers.
        let declaratives: Vec<DeclHandler> = program
            .procedure
            .declaratives
            .iter()
            .map(|u| DeclHandler {
                files: u.files.clone(),
                modes: u.modes.clone(),
                catch_all: u.catch_all,
                stmts: u.stmts.clone(),
            })
            .collect();

        // Async I/O result channel (spec 032): workers send AsyncOpResult here;
        // COBOL-WAIT-EVENT drains it. Created per interpreter, always present.
        let (async_result_tx, async_result_rx) = mpsc::channel();

        Self {
            program,
            env,
            external_store,
            rust_bridge,
            object_refs,
            objects: ObjectRegistry::new(),
            exec_rust: crate::exec_rust::ExecRustRegistry::new(),
            property_shadows: std::collections::HashMap::new(),
            para_map,
            para_order,
            nested_registry,
            program_locals: HashMap::new(),
            perform_depth: 0,
            db: DbRegistry::new(),
            http: crate::http_runtime::HttpClient::new(),
            binding_states: HashMap::new(),
            event_rx: None,
            input_rx: None,
            state_tx: None,
            display_tx: None,
            form_host_tx: None,
            self_window_handle: crate::form_host::ROOT_HANDLE.to_string(),
            self_form_object: None,
            super_window_handle: None,
            form_closed_rx: None,
            window_handle_vars: HashMap::new(),
            cancel: None,
            event_pending: None,
            chart_data: HashMap::new(),
            async_result_tx,
            async_result_rx,
            async_pending: HashMap::new(),
            async_generations: HashMap::new(),
            async_dispatch_queue: std::collections::VecDeque::new(),
            debug_cmd_rx: None,
            debug_event_tx: None,
            breakpoints: None,
            debug_stepping: false,
            current_paragraph: String::new(),
            file_specs,
            record_to_file,
            open_files: HashMap::new(),
            indexed_engine: crate::indexed::IndexedEngine::default(),
            indexed_log_level: crate::indexed_log::LogLevel::Off,
            indexed_log_format: crate::indexed_log::LogFormat::Text,
            sort_buffers: HashMap::new(),
            sort_cursors: HashMap::new(),
            alter_map: HashMap::new(),
            // Default to this process's args (correct for a compiled binary; the
            // CLI overrides with the program's own args via set_program_args).
            program_args: std::env::args().skip(1).collect(),
            argument_pointer: 1,
            env_name_register: String::new(),
            declaratives,
            in_declarative: false,
            open_modes: HashMap::new(),
        }
    }

    /// Set the COBOL program's command-line arguments (for `ACCEPT FROM
    /// COMMAND-LINE` / `ARGUMENT-NUMBER` / `ARGUMENT-VALUE`).
    pub fn set_program_args(&mut self, args: Vec<String>) {
        self.program_args = args;
    }

    /// 037 — connect this interpreter to the multi-form window supervisor:
    /// `host_tx` carries OpenForm/handle requests, `self_handle` is the
    /// supervisor's id for THIS window, `form_object` is the form's own
    /// object name (the `me::` receiver), and `closed_rx` broadcasts closed
    /// handles so windowHandler data-items become NULL (R24).
    pub fn set_form_host(
        &mut self,
        host_tx: mpsc::Sender<crate::form_host::FormRequest>,
        self_handle: &str,
        form_object: &str,
        closed_rx: mpsc::Receiver<String>,
    ) {
        self.form_host_tx = Some(host_tx);
        self.self_window_handle = self_handle.to_string();
        self.self_form_object = Some(form_object.trim().to_ascii_uppercase());
        self.form_closed_rx = Some(closed_rx);
    }

    /// 049 R28/R29 — bind `super`: `parent_handle` is the supervisor handle of
    /// the form that loaded or opened this one. Called by the spawn glue on
    /// BOTH load paths (menu load and `OpenFormSync`/`OpenFormAsync`); never
    /// called for the main form, whose `super` stays NULL (R32).
    pub fn set_super_form(&mut self, parent_handle: &str) {
        self.super_window_handle = Some(parent_handle.to_string());
    }

    /// True when `name` (any case) is the `super` receiver (049 R28).
    fn is_super(&self, name: &str) -> bool {
        name.trim().eq_ignore_ascii_case("SUPER")
    }

    /// 049 R31 — resolve a `super`-rooted path to (target handle, remaining
    /// path): the bound parent, then one `SUPERHANDLE` walk per leading
    /// `super` SEGMENT (`super::super::X` → the grandparent). A missing link
    /// anywhere raises the R32 NULL error — each step's binding is the
    /// supervisor's live caller edge, so a closed ancestor fails honestly.
    fn resolve_super_target(
        &mut self,
        path: &[PathSeg],
    ) -> Result<(String, Vec<PathSeg>), RuntimeError> {
        // R46 — a closed opener NULLs `super` before any use, so the error is
        // the standard NULL one, never a stale-handle supervisor error.
        self.drain_closed_handles();
        let Some(mut handle) = self.super_window_handle.clone() else {
            return Err(self.super_is_null_error());
        };
        let mut i = 0;
        while let Some(PathSeg::Prop(p)) = path.get(i) {
            if !p.eq_ignore_ascii_case("SUPER") {
                break;
            }
            let up = self.window_method_roundtrip(&handle, "SUPERHANDLE", Vec::new())?;
            if up.trim().is_empty() {
                return Err(self.super_is_null_error());
            }
            handle = up;
            i += 1;
        }
        Ok((handle, path[i..].to_vec()))
    }

    /// 049 R28/R31 — read `super[::super…]::X`: `GETPROPERTY` on the resolved
    /// ancestor handle, answered from that form's published surface.
    /// `Ok(None)` when the tail is not a plain single-property read.
    fn super_prop_read(&mut self, path: &[PathSeg]) -> Result<Option<CobolValue>, RuntimeError> {
        let (handle, rest) = self.resolve_super_target(path)?;
        let Some(key) = single_prop_key(&rest) else {
            return Ok(None);
        };
        let value = self.window_method_roundtrip(&handle, "GETPROPERTY", vec![key])?;
        Ok(Some(prop_to_value(Some(PropertyValue::String(value)))))
    }

    /// The standard NULL-`super` error (R32, the 037 R24 NULL-handle
    /// precedent): raised whenever `super` is referenced with no live parent.
    fn super_is_null_error(&self) -> RuntimeError {
        RuntimeError::General {
            message: "super is NULL — this form has no parent (the main form, or \
                      its opener already closed)"
                .into(),
        }
    }

    /// Drain the closed-handle broadcast: any windowHandler variable holding
    /// a closed handle becomes NULL (R24). Cheap; called lazily at every
    /// handle touch.
    fn drain_closed_handles(&mut self) {
        let Some(rx) = &self.form_closed_rx else {
            return;
        };
        let closed: Vec<String> = rx.try_iter().collect();
        if closed.is_empty() {
            return;
        }
        for slot in self.window_handle_vars.values_mut() {
            if slot.as_deref().map(|h| closed.iter().any(|c| c == h)) == Some(true) {
                *slot = None;
            }
        }
        // 049 R46 — an async child does not pin its opener: when the opener
        // closes, `super` becomes NULL and referencing it raises the standard
        // error (the R24 handle precedent).
        if self
            .super_window_handle
            .as_deref()
            .map(|h| closed.iter().any(|c| c == h))
            == Some(true)
        {
            self.super_window_handle = None;
        }
        // Mirror NULL into the data items so `IF H = NULL` style checks and
        // DISPLAY show the emptied value.
        let vars: Vec<String> = self
            .window_handle_vars
            .iter()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.clone())
            .collect();
        for var in vars {
            self.env.set_str(&var, "");
        }
    }

    /// 049 R30 — the canonical object-registry key for a member-chain or
    /// method receiver: the `me` receiver maps to the form's own object name,
    /// so `me::Width` and `<FORM-NAME>::Width` read and write the SAME entry.
    /// Before this, `me::Title` silently registered a phantom `"ME"` control —
    /// and the host's form-level taps (which match the form object name, e.g.
    /// the FormState mirror and the FullScreen echo) could never see it.
    /// Without a form context (`self_form_object` unset) the name passes
    /// through unchanged.
    fn member_root_key(&self, root: &str) -> String {
        if root.trim().eq_ignore_ascii_case("ME") {
            if let Some(form) = &self.self_form_object {
                return form.clone();
            }
        }
        root.to_string()
    }

    /// True when `name` (any case) is the `me` receiver or the form's own
    /// object name (spec 037 D4).
    fn is_me(&self, name: &str) -> bool {
        let upper = name.trim().to_ascii_uppercase();
        upper == "ME"
            || self
                .self_form_object
                .as_deref()
                .map(|f| f == upper)
                .unwrap_or(false)
    }

    /// One supervisor round-trip: send `HandleMethod` on `handle`, block on
    /// the reply. The shared plumbing of the windowHandler, `me::` and
    /// `super::` (049) receivers.
    fn window_method_roundtrip(
        &mut self,
        handle: &str,
        method: &str,
        args: Vec<String>,
    ) -> Result<String, RuntimeError> {
        use crate::form_host::FormRequest;
        let Some(tx) = self.form_host_tx.clone() else {
            return Err(RuntimeError::General {
                message: "no window supervisor in this runtime".into(),
            });
        };
        let (rtx, rrx) = mpsc::channel();
        tx.send(FormRequest::HandleMethod {
            handle: handle.to_string(),
            method: method.to_string(),
            args,
            reply: rtx,
        })
        .map_err(|_| RuntimeError::General {
            message: "window supervisor is gone".into(),
        })?;
        match rrx.recv() {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(msg)) => Err(RuntimeError::General { message: msg }),
            Err(_) => Err(RuntimeError::General {
                message: "window supervisor dropped the reply".into(),
            }),
        }
    }

    /// Does this method open a form — so its `RETURNING` data-item must be
    /// bound as a windowHandler variable (037 R20/R24)? Covers the `me::`
    /// pair and the SideMenu pair (051 R22); a bare `starts_with("OPENFORM")`
    /// missed the latter.
    pub(crate) fn method_returns_window_handle(method: &str) -> bool {
        let m = method.trim().to_ascii_uppercase();
        m.starts_with("OPENFORM") || m.starts_with("OPENSTANDALONEFORM")
    }

    /// 037 R20/R21/R28 — the one OpenForm road to the supervisor, shared by
    /// `me::"OpenFormSync"/"OpenFormAsync"` (caller = own window) and the
    /// SideMenu's `OpenStandAloneForm*` pair (caller = the shell, 051 R18).
    /// Sync blocks here until the child closes when modal (the default); the
    /// deferred reply then carries None ⇒ NULL handle (R24).
    fn open_form_via_supervisor(
        &mut self,
        caller: String,
        sync: bool,
        method: &str,
        strings: &[String],
    ) -> Result<CobolValue, RuntimeError> {
        use crate::form_host::FormRequest;
        let none = CobolValue::from_str("", 0);
        let Some(tx) = self.form_host_tx.clone() else {
            return Err(RuntimeError::General {
                message: format!(
                    "{} needs the multi-form runtime (run the form, not check)",
                    method.trim()
                ),
            });
        };
        let form_id = strings.first().cloned().unwrap_or_default();
        if form_id.is_empty() {
            return Err(RuntimeError::General {
                message: format!("{}: the form id argument is required", method.trim()),
            });
        }
        // Optional trailing parameters (R21): empty/absent ⇒ the target
        // form's RAD-designed value (resolved by the host).
        let opt_s = |i: usize| strings.get(i).filter(|s| !s.is_empty()).cloned();
        let opt_i = |i: usize| strings.get(i).and_then(|s| s.parse::<i64>().ok());
        let modal = if sync {
            strings
                .get(6)
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(true) // comma-form default (R21)
        } else {
            false // Async is never modal (R20)
        };
        let (rtx, rrx) = mpsc::channel();
        tx.send(FormRequest::OpenForm {
            caller,
            form_id,
            sync,
            window_state: opt_s(1),
            x: opt_i(2),
            y: opt_i(3),
            width: opt_i(4),
            height: opt_i(5),
            modal,
            reply: rtx,
        })
        .map_err(|_| RuntimeError::General {
            message: "window supervisor is gone".into(),
        })?;
        // Modal Sync blocks HERE until the child closes (R28).
        match rrx.recv() {
            Ok(Some(handle)) => {
                let n = handle.len();
                Ok(CobolValue::from_str(&handle, n))
            }
            Ok(None) => Ok(none),
            Err(_) => Err(RuntimeError::General {
                message: "window supervisor dropped the reply".into(),
            }),
        }
    }

    /// 049 — push one own-form property to the supervisor so `super::X` /
    /// `handle::"GetProperty"` in OTHER forms read the current value.
    /// Fire-and-forget; a missing supervisor is simply a console run.
    fn publish_own_form_prop(&mut self, key: &str, value: &str) {
        if let Some(tx) = &self.form_host_tx {
            let _ = tx.send(crate::form_host::FormRequest::PublishFormProps {
                handle: self.self_window_handle.clone(),
                props: vec![(key.to_string(), value.to_string())],
            });
        }
    }

    /// 037 — dispatch `object::method(args)` when it is a window-supervisor
    /// call: `me::"OpenFormSync"/"OpenFormAsync"` (R20), window methods on
    /// `me`, or any method on a windowHandler variable (R23). `Ok(None)` ⇒
    /// not window-related — the caller falls through to the regular
    /// object-method dispatch. Invoking through a NULL handle raises the
    /// standard runtime error (AC13).
    fn try_exec_window_call(
        &mut self,
        object: &str,
        method: &str,
        args: &[CobolValue],
    ) -> Result<Option<CobolValue>, RuntimeError> {
        use crate::form_host::FormRequest;
        let m = method.trim().to_ascii_uppercase();
        let strings: Vec<String> = args
            .iter()
            .map(|v| v.as_display_string().trim().to_string())
            .collect();
        let none = CobolValue::from_str("", 0);
        self.drain_closed_handles();
        let obj_upper = object.trim().to_ascii_uppercase();

        // ── A windowHandler variable as the receiver ──────────────────────
        if let Some(slot) = self.window_handle_vars.get(&obj_upper).cloned() {
            let Some(handle) = slot else {
                return Err(RuntimeError::General {
                    message: format!(
                        "windowHandler {} is NULL — the form it referred to is closed",
                        object.trim()
                    ),
                });
            };
            let Some(tx) = self.form_host_tx.clone() else {
                return Err(RuntimeError::General {
                    message: "no window supervisor in this runtime".into(),
                });
            };
            let (rtx, rrx) = mpsc::channel();
            tx.send(FormRequest::HandleMethod {
                handle,
                method: m,
                args: strings,
                reply: rtx,
            })
            .map_err(|_| RuntimeError::General {
                message: "window supervisor is gone".into(),
            })?;
            return match rrx.recv() {
                Ok(Ok(value)) => {
                    let n = value.len();
                    Ok(Some(CobolValue::from_str(&value, n.max(1))))
                }
                Ok(Err(msg)) => Err(RuntimeError::General { message: msg }),
                Err(_) => Err(RuntimeError::General {
                    message: "window supervisor dropped the reply".into(),
                }),
            };
        }

        // ── `super::` receiver (049 R28) — a pre-bound handle: every window
        // method routes to the PARENT form exactly as through a windowHandler
        // variable, so the whole R23 surface applies. NULL parent ⇒ the
        // standard error (R32).
        if self.is_super(object) {
            let Some(handle) = self.super_window_handle.clone() else {
                return Err(self.super_is_null_error());
            };
            let value = self.window_method_roundtrip(&handle, &m, strings)?;
            let n = value.len();
            return Ok(Some(CobolValue::from_str(&value, n.max(1))));
        }

        // ── 051 — the SideMenu control as receiver ────────────────────────
        // The sidebar owns the shell's navigation, so it also owns the
        // programmatic door to a standalone child window:
        // `INVOKE SideMenu-1 "OpenStandAloneFormSync"/"…Async" USING …`.
        // Whoever invokes it, the opened window's parent is the SHELL — the
        // root form — exactly like the sidebar's own menu actions (R18/R21).
        if matches!(
            m.as_str(),
            "OPENSTANDALONEFORMSYNC" | "OPENSTANDALONEFORMASYNC"
        ) && self
            .objects
            .get(&obj_upper)
            .map(|o| o.class == "SideMenu")
            .unwrap_or(false)
        {
            let sync = m == "OPENSTANDALONEFORMSYNC";
            let caller = crate::form_host::ROOT_HANDLE.to_string();
            return self
                .open_form_via_supervisor(caller, sync, method, &strings)
                .map(Some);
        }

        // ── `me::` receivers ──────────────────────────────────────────────
        if !self.is_me(object) {
            return Ok(None);
        }
        match m.as_str() {
            "OPENFORMSYNC" | "OPENFORMASYNC" => {
                let sync = m == "OPENFORMSYNC";
                let caller = self.self_window_handle.clone();
                self.open_form_via_supervisor(caller, sync, method, &strings)
                    .map(Some)
            }
            // Window methods on the form's own window.
            "SETFULLSCREEN" | "SETTITLEVISIBLE" | "SETWINDOWSTATE" | "FOCUS" | "SETFOCUS" => {
                let Some(tx) = self.form_host_tx.clone() else {
                    // Single-form runtime: harmless no-op so generated code
                    // stays runnable under Check.
                    return Ok(Some(none));
                };
                let (rtx, rrx) = mpsc::channel();
                tx.send(FormRequest::HandleMethod {
                    handle: self.self_window_handle.clone(),
                    method: m,
                    args: strings,
                    reply: rtx,
                })
                .map_err(|_| RuntimeError::General {
                    message: "window supervisor is gone".into(),
                })?;
                match rrx.recv() {
                    Ok(Ok(_)) => Ok(Some(none)),
                    Ok(Err(msg)) => Err(RuntimeError::General { message: msg }),
                    Err(_) => Err(RuntimeError::General {
                        message: "window supervisor dropped the reply".into(),
                    }),
                }
            }
            "CLOSE" => {
                if let Some(tx) = self.form_host_tx.clone() {
                    let _ = tx.send(FormRequest::CloseSelf {
                        caller: self.self_window_handle.clone(),
                    });
                }
                Ok(Some(none))
            }
            _ => Ok(None),
        }
    }

    /// 037 — `H::FormState` property READ through a windowHandler (R23).
    /// `Ok(None)` ⇒ not a handle property, fall through.
    fn try_window_handle_prop(
        &mut self,
        root: &str,
        path: &[crate::objects::PathSeg],
    ) -> Result<Option<CobolValue>, RuntimeError> {
        use crate::objects::PathSeg;
        let [PathSeg::Prop(prop)] = path else {
            return Ok(None);
        };
        if !prop.eq_ignore_ascii_case("FormState") {
            return Ok(None);
        }
        let upper = root.trim().to_ascii_uppercase();
        if !self.window_handle_vars.contains_key(&upper) {
            return Ok(None);
        }
        self.try_exec_window_call(root, "GetFormState", &[])
    }

    /// Select the indexed (ISAM) file engine for this run. All engines present
    /// identical observable COBOL behaviour; only the on-disk container differs.
    pub fn set_indexed_engine(&mut self, engine: crate::indexed::IndexedEngine) {
        if engine != crate::indexed::IndexedEngine::Rust {
            tracing::info!(
                "indexed engine '{}' selected; delegating to the Rust engine \
                 (behaviour-compatible) — native container not yet available",
                engine.name()
            );
        }
        self.indexed_engine = engine;
    }

    /// Set the per-file INDEXED observability log level (redb engine only).
    pub fn set_indexed_log_level(&mut self, level: crate::indexed_log::LogLevel) {
        self.indexed_log_level = level;
    }

    /// Set the INDEXED observability log line format (text/logfmt or JSON).
    pub fn set_indexed_log_format(&mut self, format: crate::indexed_log::LogFormat) {
        self.indexed_log_format = format;
    }

    /// Create an interpreter wired to the GUI Form Runtime Engine channels.
    ///
    /// - `event_rx`  — receives `FormEvent` from the UI (button clicks, etc.)
    /// - `state_tx`  — sends `StateUpdate` to the UI (SET-PROPERTY changes)
    /// - `display_tx`— sends DISPLAY output lines to the IDE output panel
    pub fn new_with_channels(
        program: Program,
        event_rx: mpsc::Receiver<FormEvent>,
        state_tx: mpsc::Sender<StateUpdate>,
        display_tx: mpsc::Sender<String>,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.event_rx = Some(event_rx);
        interp.state_tx = Some(state_tx);
        interp.display_tx = Some(display_tx);
        interp
    }

    /// Attach the UI→interpreter property-sync channel. UI-driven value changes
    /// (slider drag, text edit, combo select, …) arrive here and are folded into
    /// the object registry by `COBOL-WAIT-EVENT`, so an event handler reads the
    /// live value rather than the seeded default.
    pub fn set_input_channel(&mut self, input_rx: mpsc::Receiver<StateUpdate>) {
        self.input_rx = Some(input_rx);
    }

    /// Attach the cooperative cancellation flag. Once the host sets it, the
    /// interpreter aborts between statements with `RuntimeError::Cancelled`
    /// (treated as a clean exit by `run`), so a looping handler stops promptly.
    pub fn set_cancel_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancel = Some(flag);
    }

    /// Attach the shared event-queue-depth counter. `COBOL-WAIT-EVENT`
    /// decrements it as it consumes each event, letting the host coalesce
    /// timer ticks against a still-full queue.
    pub fn set_event_counter(&mut self, counter: Arc<AtomicUsize>) {
        self.event_pending = Some(counter);
    }

    /// `true` when the host has requested cancellation.
    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// Drain any pending UI-driven property updates into the object registry.
    /// Called just before an event handler runs so getters see the live value.
    fn drain_input(&mut self) {
        if let Some(rx) = &self.input_rx {
            let pending: Vec<StateUpdate> = rx.try_iter().collect();
            for upd in pending {
                self.objects
                    .set_property(&upd.ctrl_id, &upd.prop, upd.value);
            }
        }
    }

    // ── Async I/O operations (spec 032) ───────────────────────────────────────

    /// Poll interval used while at least one async operation is in flight, so a
    /// blocked `COBOL-WAIT-EVENT` still notices completions and timeouts with no
    /// other UI activity. When nothing is pending the wait blocks normally.
    const ASYNC_POLL_MS: u64 = 40;

    /// Is this REST control in async mode? REST is **async by default** (spec
    /// 032 / operator decision) — async unless its `Mode` property is explicitly
    /// `Sync`, so legacy forms with no `Mode` property also run async.
    fn rest_is_async(&self, obj: &str) -> bool {
        !self.obj_get(obj, "Mode").trim().eq_ignore_ascii_case("sync")
    }

    /// Effective REST timeout in milliseconds: `TimeoutMs` if set, else the
    /// legacy `TimeoutSeconds × 1000`, else 0 (no interpreter-side timeout).
    fn rest_timeout_ms(&self, obj: &str) -> u64 {
        let ms = self.obj_get(obj, "TimeoutMs").trim().parse::<u64>().unwrap_or(0);
        if ms > 0 {
            return ms;
        }
        let secs = self
            .obj_get(obj, "TimeoutSeconds")
            .trim()
            .parse::<u64>()
            .unwrap_or(0);
        secs.saturating_mul(1000)
    }

    /// Spawn a background REST operation (spec 032). Returns immediately.
    ///
    /// Rejects (no-op) if the control is already `Busy`. Otherwise bumps the
    /// control's generation, sets `Busy = 1`, records the pending op, and spawns
    /// a detached worker that performs the blocking HTTP call and posts an
    /// `AsyncOpResult` back over `async_result_tx`.
    fn spawn_rest_op(&mut self, obj: &str, verb: &str, url: String, body: String) -> CobolValue {
        if self.async_pending.contains_key(obj) {
            // One in-flight op per control — ignore the second start (R6).
            return CobolValue::from_str("", 0);
        }
        let timeout_ms = self.rest_timeout_ms(obj);
        let generation = {
            let gen = self
                .async_generations
                .entry(obj.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        };
        self.obj_set(obj, "Busy", "1".into());
        self.async_pending.insert(
            obj.to_string(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                timeout_ms,
            },
        );

        let tx = self.async_result_tx.clone();
        let http = self.http.clone();
        let ctrl_id = obj.to_string();
        let verb = verb.to_ascii_uppercase();
        // The interpreter-side timeout sweep owns `onTimeout` semantics (R13);
        // the transport timeout is a longer thread-lifetime backstop so a stalled
        // worker can't leak, without racing the sweep. `0` = no timeout at all.
        let transport_timeout = if timeout_ms > 0 {
            timeout_ms.saturating_add(5_000)
        } else {
            0
        };
        std::thread::spawn(move || {
            let (b, st) = match verb.as_str() {
                "POST" => http.send_with_body_timeout("POST", &url, &body, transport_timeout),
                "PUT" => http.send_with_body_timeout("PUT", &url, &body, transport_timeout),
                "DELETE" => http.delete_with_timeout(&url, transport_timeout),
                _ => http.get_with_timeout(&url, transport_timeout),
            };
            // The sync convention: a transport error yields status 0 with the
            // error text as the body; a real HTTP response (incl. 4xx/5xx) keeps
            // its status. Map accordingly.
            let outcome = if st == 0 {
                crate::async_op::AsyncOutcome::HttpError { message: b }
            } else {
                crate::async_op::AsyncOutcome::HttpSuccess { body: b, status: st }
            };
            let _ = tx.send(crate::async_op::AsyncOpResult {
                ctrl_id,
                generation,
                outcome,
            });
        });

        // Async verbs have no meaningful same-statement return value.
        CobolValue::from_str("", 0)
    }

    /// Spawn a background `google_maps` operation (spec 039 T11) — the
    /// Maps control's Geocode/ReverseGeocode/Directions/DistanceMatrix/
    /// PlacesSearch verbs. Mirrors [`Self::spawn_rest_op`] exactly (same
    /// `async_pending`/`async_generations`/`async_result_tx` bookkeeping,
    /// same `AsyncOpResult` delivery — `drain_async_ops` needs no changes
    /// at all to handle this) with one difference: `google_maps` is
    /// `reqwest`+async, so the worker thread privately builds a small
    /// `tokio::Runtime` and `.block_on()`s the call inside it (plan.md §4
    /// Decision 5) — the interpreter's own event loop stays fully
    /// synchronous; only this one background thread ever touches tokio.
    ///
    /// The API key is read from `_ResolvedMapsApiKey`, a runtime-only
    /// property the host seeds at Run/Build time (T12) — never a literal
    /// on the control (R31).
    fn spawn_maps_op(&mut self, obj: &str, verb: &str, args: Vec<String>) -> CobolValue {
        if self.async_pending.contains_key(obj) {
            return CobolValue::from_str("", 0); // one in-flight op per control
        }
        let api_key = self.obj_get(obj, "_ResolvedMapsApiKey");
        if api_key.trim().is_empty() {
            // R33: "not configured" — fail synchronously with no worker
            // thread at all, rather than a network call that would 400.
            self.obj_set(obj, "LastError", "Maps API key not configured".into());
            self.async_dispatch_queue
                .push_back((obj.to_string(), "onError".to_string()));
            return CobolValue::from_str("", 0);
        }
        let timeout_ms = self.rest_timeout_ms(obj);
        let generation = {
            let gen = self
                .async_generations
                .entry(obj.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        };
        self.obj_set(obj, "Busy", "1".into());
        self.async_pending.insert(
            obj.to_string(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                timeout_ms,
            },
        );

        let tx = self.async_result_tx.clone();
        let ctrl_id = obj.to_string();
        let verb = verb.to_owned();
        std::thread::spawn(move || {
            let outcome = match crate::maps_bridge::run(&api_key, &verb, &args) {
                Ok(body) => crate::async_op::AsyncOutcome::HttpSuccess { body, status: 200 },
                Err(message) => crate::async_op::AsyncOutcome::HttpError { message },
            };
            let _ = tx.send(crate::async_op::AsyncOpResult {
                ctrl_id,
                generation,
                outcome,
            });
        });

        CobolValue::from_str("", 0)
    }

    /// Parse a WebSearch control's `ResponseBody` (the raw Google Custom
    /// Search JSON API response) into `(title, snippet, link)` tuples, one
    /// per result item (spec 039 T15/R29). An empty or malformed body (not
    /// yet searched, or an error body) yields an empty Vec rather than an
    /// error — the same "absent data reads as nothing, not a crash"
    /// tolerance `refresh_marker_binding` already applies to a bad row.
    fn web_search_items(&self, obj: &str) -> Vec<(String, String, String)> {
        let body = self.obj_get(obj, "ResponseBody");
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) else {
            return Vec::new();
        };
        let Some(items) = parsed.get("items").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| {
                let field = |k: &str| {
                    item.get(k)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_owned()
                };
                (field("title"), field("snippet"), field("link"))
            })
            .collect()
    }

    /// Queue a control lifecycle event for dispatch on the next
    /// `COBOL-WAIT-EVENT` return (spec 021; rides the spec-032 dispatch
    /// queue). Events without a bound handler are dropped by the generated
    /// dispatch code, so queuing is always safe.
    fn queue_control_event(&mut self, obj: &str, event: &str) {
        self.async_dispatch_queue
            .push_back((obj.to_string(), event.to_string()));
    }

    /// Cancel any in-flight operation on `obj` (spec 032 R10/R11). Runs entirely
    /// on the interpreter thread — bumps the generation (so the abandoned
    /// worker's eventual result is discarded), clears `Busy`, and queues
    /// `onCancelled`. A no-op when nothing is in flight.
    fn cancel_async_op(&mut self, obj: &str) {
        if self.async_pending.remove(obj).is_some() {
            if let Some(g) = self.async_generations.get(obj) {
                g.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            self.obj_set(obj, "Busy", "0".into());
            self.async_dispatch_queue
                .push_back((obj.to_string(), "onCancelled".to_string()));
        }
    }

    /// Drain completed async results and sweep for timeouts. Applies each
    /// current-generation result to its control (writes outputs, clears `Busy`)
    /// and enqueues the corresponding lifecycle event; discards stale results.
    fn drain_async_ops(&mut self) {
        // 1. Apply delivered results.
        let results: Vec<crate::async_op::AsyncOpResult> =
            self.async_result_rx.try_iter().collect();
        for r in results {
            let live = self
                .async_generations
                .get(&r.ctrl_id)
                .map(|g| g.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(0);
            if r.generation != live {
                continue; // stale — cancelled / timed-out / superseded
            }
            self.async_pending.remove(&r.ctrl_id);
            match r.outcome {
                crate::async_op::AsyncOutcome::HttpSuccess { body, status } => {
                    self.obj_set(&r.ctrl_id, "ResponseBody", body);
                    self.obj_set(&r.ctrl_id, "StatusCode", status.to_string());
                    self.obj_set(&r.ctrl_id, "Busy", "0".into());
                    self.async_dispatch_queue
                        .push_back((r.ctrl_id, "onComplete".to_string()));
                }
                crate::async_op::AsyncOutcome::HttpError { message } => {
                    self.obj_set(&r.ctrl_id, "LastError", message);
                    self.obj_set(&r.ctrl_id, "StatusCode", "0".into());
                    self.obj_set(&r.ctrl_id, "Busy", "0".into());
                    self.async_dispatch_queue
                        .push_back((r.ctrl_id, "onError".to_string()));
                }
            }
        }

        // 2. Timeout sweep.
        let now = std::time::Instant::now();
        let timed_out: Vec<String> = self
            .async_pending
            .iter()
            .filter(|(_, op)| {
                op.timeout_ms > 0
                    && now.duration_since(op.started_at).as_millis() as u64 > op.timeout_ms
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in timed_out {
            self.async_pending.remove(&id);
            if let Some(g) = self.async_generations.get(&id) {
                g.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            self.obj_set(&id, "Busy", "0".into());
            self.async_dispatch_queue
                .push_back((id, "onTimeout".to_string()));
        }
    }

    /// Block for the next thing `COBOL-WAIT-EVENT` should present: either a real
    /// UI event, an async lifecycle dispatch, or channel disconnect. While async
    /// ops are pending it polls (`recv_timeout`) so completions/timeouts are
    /// noticed even with no UI activity; otherwise it blocks on `recv()`.
    fn next_wait_outcome(&mut self) -> WaitOutcome {
        loop {
            self.drain_async_ops();
            if let Some((ctrl, event_id)) = self.async_dispatch_queue.pop_front() {
                return WaitOutcome::AsyncDispatch(ctrl, event_id);
            }
            let has_pending = !self.async_pending.is_empty();
            let rx = match self.event_rx.as_ref() {
                Some(rx) => rx,
                None => return WaitOutcome::Disconnected,
            };
            if has_pending {
                match rx.recv_timeout(std::time::Duration::from_millis(Self::ASYNC_POLL_MS)) {
                    Ok(ev) => return WaitOutcome::Ui(ev),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return WaitOutcome::Disconnected
                    }
                }
            } else {
                match rx.recv() {
                    Ok(ev) => return WaitOutcome::Ui(ev),
                    Err(_) => return WaitOutcome::Disconnected,
                }
            }
        }
    }

    /// Create an interpreter wired to the IDE debugger channels.
    ///
    /// - `debug_cmd_rx`  — receives `DebugCmd` from the IDE (Continue, StepOver, Pause)
    /// - `debug_event_tx`— sends `DebugEvent` to the IDE (Paused, Resumed, Finished)
    /// - `breakpoints`   — shared set of active breakpoint line numbers
    pub fn new_with_debug_channels(
        program: Program,
        debug_cmd_rx: mpsc::Receiver<crate::debugger::DebugCmd>,
        debug_event_tx: mpsc::Sender<crate::debugger::DebugEvent>,
        breakpoints: crate::debugger::Breakpoints,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.attach_debug_channels(debug_cmd_rx, debug_event_tx, breakpoints);
        interp
    }

    /// Attach IDE-debugger channels to an interpreter that was constructed for
    /// another mode (e.g. a GUI form session via `new_with_channels`). The
    /// program starts paused at line 1, exactly like `new_with_debug_channels`
    /// — this is how `rcrun run-form --debug` runs a live, interactive form
    /// under debugger control.
    pub fn attach_debug_channels(
        &mut self,
        debug_cmd_rx: mpsc::Receiver<crate::debugger::DebugCmd>,
        debug_event_tx: mpsc::Sender<crate::debugger::DebugEvent>,
        breakpoints: crate::debugger::Breakpoints,
    ) {
        self.debug_cmd_rx = Some(debug_cmd_rx);
        self.debug_event_tx = Some(debug_event_tx);
        self.breakpoints = Some(breakpoints);
        self.debug_stepping = true; // start paused at line 1
    }

    /// Seed the visual-object registry with a form's controls and their
    /// designed properties, so that property references (`"Caption" OF Ctrl`)
    /// and method getters (`Ctrl::GetCaption()`) return the configured values
    /// before any setter runs. Object and property names are matched
    /// case-insensitively by the registry.
    pub fn seed_objects<I, P>(&mut self, controls: I)
    where
        I: IntoIterator<Item = (String, String, P)>,
        P: IntoIterator<Item = (String, String)>,
    {
        // 049 — the designed form-entry props are published to the supervisor
        // after seeding, so `super::X` in other forms reads them (R33).
        let mut own_form_props: Vec<(String, String)> = Vec::new();
        for (id, class, props) in controls {
            if !self.objects.contains(&id) {
                self.objects.register(&id, class.clone());
            }
            // Run-form: seed props for GroupBox (including databound repeating ones)
            let props_vec: Vec<(String, String)> = props.into_iter().collect();
            if class.eq_ignore_ascii_case("GroupBox")
                || id.to_ascii_lowercase().contains("groupbox")
            {
                tracing::debug!(target: "databinding", "RUN-FORM seed GroupBox {} with databind props if any", id);
            }
            for (k, v) in &props_vec {
                self.objects.set_property(&id, &k, v.clone());
            }
            if self
                .self_form_object
                .as_deref()
                .map(|f| f.eq_ignore_ascii_case(id.trim()))
                .unwrap_or(false)
            {
                own_form_props = props_vec.clone();
            }

            // Double seed repeating GroupBoxes under their ArrayName
            let is_repeating = props_vec.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("IsRepeatingGroup")
                    && (v == "1" || v.eq_ignore_ascii_case("true"))
            });
            if class.eq_ignore_ascii_case("GroupBox") && is_repeating {
                let array_name = props_vec
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("ArrayName"))
                    .map(|(_, v)| v.trim())
                    .unwrap_or("");
                let array_id = if array_name.is_empty() {
                    id.clone()
                } else {
                    array_name.to_string()
                };
                tracing::debug!(target: "databinding", "Seeding repeating GroupBox array_id={}, design_id={}", array_id, id);
                databind_trace!(
                    "Seeding repeating GroupBox array_id={}, design_id={}",
                    array_id,
                    id
                );
                if !self.objects.contains(&array_id) {
                    self.objects.register(&array_id, class.clone());
                }
                self.objects
                    .set_property(&array_id, "_DesignControlId", id.clone());
                for (k, v) in &props_vec {
                    self.objects.set_property(&array_id, k, v.clone());
                }
            }
        }
        if !own_form_props.is_empty() {
            if let Some(tx) = &self.form_host_tx {
                let _ = tx.send(crate::form_host::FormRequest::PublishFormProps {
                    handle: self.self_window_handle.clone(),
                    props: own_form_props,
                });
            }
        }
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    /// Run the program to completion.
    ///
    /// Execution starts at the first paragraph and falls through subsequent
    /// paragraphs in declaration order.  GO TO, STOP RUN, and GOBACK are
    /// handled as control-flow signals; all other errors bubble up to the
    /// caller.
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        // Pull the run unit's current EXTERNAL values in, run, then publish
        // ours back so a later activation in the same run unit sees them.
        self.load_external();
        let result = self.run_inner();
        self.flush_external();
        result
    }

    /// A clone of this interpreter's run-unit EXTERNAL store. Pass it to
    /// [`with_external_store`](Self::with_external_store) on another interpreter
    /// (e.g. a second form module) so both share one physical copy of every
    /// EXTERNAL item.
    pub fn external_store(&self) -> ExternalStore {
        self.external_store.clone()
    }

    /// Copy the run unit's current EXTERNAL values into the local environment.
    fn load_external(&mut self) {
        let names: Vec<String> = self.env.external_names().iter().cloned().collect();
        if names.is_empty() {
            return;
        }
        let guard = self
            .external_store
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for name in names {
            if let Some(v) = guard.get(&name).cloned() {
                self.env.raw_set(&name, v);
            }
        }
    }

    /// Publish the local EXTERNAL values back to the run-unit store.
    fn flush_external(&mut self) {
        let names: Vec<String> = self.env.external_names().iter().cloned().collect();
        if names.is_empty() {
            return;
        }
        let mut guard = self
            .external_store
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for name in names {
            if let Some(v) = self.env.raw_get(&name).cloned() {
                guard.insert(name, v);
            }
        }
    }

    fn run_inner(&mut self) -> Result<(), RuntimeError> {
        let mut idx = 0usize;
        while idx < self.para_order.len() {
            let name = self.para_order[idx].clone();
            self.current_paragraph = name.clone();
            let stmts = match self.para_map.get(&name) {
                Some(s) => s.clone(),
                None => {
                    idx += 1;
                    continue;
                }
            };
            match self.exec_stmts(&stmts) {
                // EXIT PARAGRAPH/SECTION and NEXT SENTENCE end the current
                // paragraph; sequential flow then continues with the next one.
                Ok(())
                | Err(RuntimeError::ExitParagraph)
                | Err(RuntimeError::ExitSection)
                | Err(RuntimeError::NextSentence) => idx += 1,
                Err(RuntimeError::GoTo { target }) => {
                    let upper = target.to_ascii_uppercase();
                    match self.para_order.iter().position(|n| n == &upper) {
                        Some(pos) => idx = pos,
                        None => {
                            return Err(RuntimeError::UndefinedParagraph {
                                name: upper,
                                span: Span::dummy(),
                            })
                        }
                    }
                }
                // Normal program termination signals — treat as success.
                // `Cancelled` is a host-requested cooperative abort, also clean.
                Err(RuntimeError::StopRun)
                | Err(RuntimeError::GoBack)
                | Err(RuntimeError::Cancelled) => {
                    self.send_debug_finished();
                    self.db.close_all();
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        }
        self.send_debug_finished();
        self.db.close_all();
        Ok(())
    }

    fn send_debug_finished(&self) {
        if let Some(tx) = &self.debug_event_tx {
            let _ = tx.send(crate::debugger::DebugEvent::Finished);
        }
    }

    /// Execute a set of paragraphs given an explicit map + order (used for
    /// nested-program dispatch where the para_map differs from the outer one).
    ///
    /// Handles GO TO within the nested program's own paragraph space.
    /// GOBACK is propagated as-is so the caller can treat it as a normal return.
    fn run_para_sequence(
        &mut self,
        para_map: &IndexMap<String, Vec<Stmt>>,
        para_order: &[String],
    ) -> Result<(), RuntimeError> {
        let mut idx = 0usize;
        while idx < para_order.len() {
            let name = &para_order[idx];
            self.current_paragraph = name.clone();
            let stmts = match para_map.get(name) {
                Some(s) => s.clone(),
                None => {
                    idx += 1;
                    continue;
                }
            };
            match self.exec_stmts(&stmts) {
                Ok(())
                | Err(RuntimeError::ExitParagraph)
                | Err(RuntimeError::ExitSection)
                | Err(RuntimeError::NextSentence) => idx += 1,
                Err(RuntimeError::GoTo { target }) => {
                    let upper = target.to_ascii_uppercase();
                    match para_order.iter().position(|n| n == &upper) {
                        Some(pos) => idx = pos,
                        None => {
                            return Err(RuntimeError::UndefinedParagraph {
                                name: upper,
                                span: Span::dummy(),
                            })
                        }
                    }
                }
                // GOBACK / STOP RUN / errors propagate to caller.
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    // ── Statement dispatch ────────────────────────────────────────────────────

    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<(), RuntimeError> {
        let mut i = 0;
        while i < stmts.len() {
            // Cooperative cancellation: bail out promptly between statements so a
            // long-running or looping handler (e.g. a Timer tick) can never hang
            // the UI thread when the form/IDE is closing. This chokepoint also
            // covers every PERFORM iteration and paragraph body, which route
            // through here.
            if self.is_cancelled() {
                return Err(RuntimeError::Cancelled);
            }
            let stmt = &stmts[i];
            if matches!(stmt, Stmt::SentenceEnd { .. }) {
                i += 1;
                continue;
            }
            self.debug_check(stmt)?;
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(RuntimeError::NextSentence) => {
                    // Skip to the statement after the next sentence boundary in
                    // this list; if there is none, propagate to the enclosing
                    // list (ultimately ending the paragraph).
                    let mut j = i + 1;
                    while j < stmts.len() && !matches!(stmts[j], Stmt::SentenceEnd { .. }) {
                        j += 1;
                    }
                    if j < stmts.len() {
                        i = j + 1;
                        continue;
                    }
                    return Err(RuntimeError::NextSentence);
                }
                Err(e) => return Err(e),
            }
            i += 1;
        }
        Ok(())
    }

    /// Called before every statement when a debug session is active.
    ///
    /// Pauses execution (blocking on `debug_cmd_rx`) when:
    ///   - `debug_stepping` is true (StepOver mode), OR
    ///   - the statement's source line matches an active breakpoint.
    ///
    /// While paused, sends `DebugEvent::Paused` with a full variable snapshot.
    /// Resumes on `DebugCmd::Continue`, `DebugCmd::StepOver`, or `DebugCmd::StepIn`.
    fn debug_check(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        // Short-circuit when no debug session is attached.
        let (Some(cmd_rx), Some(ev_tx)) =
            (self.debug_cmd_rx.as_ref(), self.debug_event_tx.as_ref())
        else {
            return Ok(());
        };

        let span = stmt_span(stmt);
        let line = span.map(|s| s.line).unwrap_or(0);

        // Decide whether to pause.
        let hit_breakpoint = line > 0
            && self
                .breakpoints
                .as_ref()
                .map(|bp| bp.lock().map(|set| set.contains(&line)).unwrap_or(false))
                .unwrap_or(false);

        if !self.debug_stepping && !hit_breakpoint {
            // Check for async Pause command without blocking.
            match cmd_rx.try_recv() {
                Ok(crate::debugger::DebugCmd::Pause) => self.debug_stepping = true,
                _ => return Ok(()),
            }
        }

        // Build variable snapshot.
        let vars: Vec<crate::debugger::VarSnapshot> = self
            .env
            .iter()
            .map(|(k, v)| {
                let (scope, pic, origin) = self.debug_var_details(k);
                crate::debugger::VarSnapshot {
                    name: k.clone(),
                    scope,
                    pic,
                    origin,
                    value: v.as_display_string(),
                }
            })
            .collect();

        let _ = ev_tx.send(crate::debugger::DebugEvent::Paused {
            line: line,
            col: span.map(|s| s.col).unwrap_or(0),
            paragraph: self.current_paragraph.clone(),
            vars,
        });

        // Block until the IDE sends a command.
        self.debug_stepping = false; // reset; StepOver re-enables it below
        loop {
            match cmd_rx.recv() {
                Ok(crate::debugger::DebugCmd::Continue) => {
                    let _ = ev_tx.send(crate::debugger::DebugEvent::Resumed);
                    break;
                }
                Ok(crate::debugger::DebugCmd::StepOver) | Ok(crate::debugger::DebugCmd::StepIn) => {
                    self.debug_stepping = true; // pause again after this stmt
                    let _ = ev_tx.send(crate::debugger::DebugEvent::Resumed);
                    break;
                }
                Ok(crate::debugger::DebugCmd::Pause) => {
                    // Already paused; just re-send paused (no-op).
                }
                Err(_) => {
                    // Channel dropped — IDE closed. Stop the program.
                    return Err(RuntimeError::StopRun);
                }
            }
        }

        Ok(())
    }

    fn debug_var_details(&self, key: &str) -> (String, String, String) {
        let base = key.split_once('(').map(|(name, _)| name).unwrap_or(key);
        if let Some(symbol) = self.env.symbol(base) {
            let scope = symbol
                .scope
                .map(|scope| {
                    format!(
                        "{} ({})",
                        if symbol.is_global { "Global" } else { "Local" },
                        scope.abbrev()
                    )
                })
                .unwrap_or_else(|| "Local (WS)".to_owned());
            (
                scope,
                symbol.pic.clone(),
                if symbol.origin.is_empty() {
                    self.program.identification.program_id.clone()
                } else {
                    symbol.origin.clone()
                },
            )
        } else {
            (
                "Local (WS)".to_owned(),
                String::new(),
                self.program.identification.program_id.clone(),
            )
        }
    }

    #[allow(clippy::too_many_lines)]
    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        let result = self.dispatch_stmt(stmt);
        // Any property reference used as a receiving field by this statement is
        // written back to its control here, so property receivers work with any
        // verb (ADD, COMPUTE, STRING INTO, ACCEPT, INITIALIZE, …), not just MOVE.
        self.flush_property_shadows();
        result
    }

    fn dispatch_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            // ── Data movement ─────────────────────────────────────────────────
            Stmt::Move { from, to, .. } => self.exec_move(from, to),
            Stmt::MoveCorresponding { from, to, .. } => {
                let from_key = self.resolve_lvalue(from);
                let to_key = self.resolve_lvalue(to);
                self.move_corresponding(&from_key, &to_key)
            }
            Stmt::AddCorresponding { from, to, .. } => {
                let from_key = self.resolve_lvalue(from);
                let to_key = self.resolve_lvalue(to);
                self.arith_corresponding(&from_key, &to_key, false)
            }
            Stmt::SubtractCorresponding { from, to, .. } => {
                let from_key = self.resolve_lvalue(from);
                let to_key = self.resolve_lvalue(to);
                self.arith_corresponding(&from_key, &to_key, true)
            }

            Stmt::Initialize {
                items, replacing, ..
            } => self.exec_initialize(items, replacing),

            // ── Arithmetic ────────────────────────────────────────────────────
            Stmt::Add {
                operands,
                to,
                giving,
                on_size_error,
                not_on_size_error,
                span,
            } => self.exec_add(
                operands,
                to,
                giving,
                on_size_error,
                not_on_size_error,
                *span,
            ),
            Stmt::Subtract {
                operands,
                from,
                giving,
                on_size_error,
                not_on_size_error,
                span,
            } => self.exec_subtract(
                operands,
                from,
                giving,
                on_size_error,
                not_on_size_error,
                *span,
            ),
            Stmt::Multiply {
                lhs,
                by,
                giving,
                rounded,
                on_size_error,
                not_on_size_error,
                span,
            } => self.exec_multiply(
                lhs,
                by,
                giving,
                *rounded,
                on_size_error,
                not_on_size_error,
                *span,
            ),
            Stmt::Divide {
                lhs,
                by,
                giving,
                remainder,
                rounded,
                on_size_error,
                not_on_size_error,
                span,
            } => self.exec_divide(
                lhs,
                by,
                giving,
                remainder.as_ref(),
                *rounded,
                on_size_error,
                not_on_size_error,
                *span,
            ),
            Stmt::Compute {
                targets,
                expr,
                on_size_error,
                not_on_size_error,
                span,
            } => self.exec_compute(targets, expr, on_size_error, not_on_size_error, *span),

            // ── Control flow ──────────────────────────────────────────────────
            Stmt::If {
                condition,
                then_stmts,
                else_stmts,
                ..
            } => self.exec_if(condition, then_stmts, else_stmts),
            Stmt::Evaluate {
                subjects,
                whens,
                other_stmts,
                ..
            } => self.exec_evaluate(subjects, whens, other_stmts),
            Stmt::Perform { target, span } => self.exec_perform(target, *span),
            Stmt::Search {
                all,
                table,
                varying,
                at_end,
                whens,
                ..
            } => self.exec_search(*all, table, varying.as_ref(), at_end, whens),
            Stmt::GoTo { target, .. } => {
                // An ALTER may have redirected this paragraph's GO TO.
                let t = self
                    .alter_map
                    .get(&self.current_paragraph)
                    .cloned()
                    .unwrap_or_else(|| target.clone());
                Err(RuntimeError::GoTo { target: t })
            }
            Stmt::Alter { from, to, .. } => {
                self.alter_map.insert(from.to_ascii_uppercase(), to.clone());
                Ok(())
            }
            Stmt::Unlock { file, .. } => {
                // Release any record locks held on the file (INDEXED engine).
                let fkey = file.to_ascii_uppercase();
                if let Some(OpenFile::Indexed(engine)) = self.open_files.get_mut(&fkey) {
                    engine.unlock();
                }
                Ok(())
            }
            Stmt::Cancel { programs, .. } => self.exec_cancel(programs),
            Stmt::Commit { .. } => {
                // Make every open INDEXED file's changes durable; start a new tx.
                for f in self.open_files.values_mut() {
                    if let OpenFile::Indexed(engine) = f {
                        engine.commit();
                    }
                }
                Ok(())
            }
            Stmt::Rollback { .. } => {
                // Undo every open INDEXED file's changes since the last COMMIT.
                for f in self.open_files.values_mut() {
                    if let OpenFile::Indexed(engine) = f {
                        engine.rollback();
                    }
                }
                Ok(())
            }
            Stmt::SetPointer {
                address_of,
                targets,
                source,
                ..
            } => self.exec_set_pointer(address_of.as_ref(), targets, source),
            Stmt::GoToDepending {
                targets,
                depending,
                span,
            } => self.exec_go_to_depending(targets, depending, *span),
            Stmt::Continue { .. } => Ok(()),
            // NEXT SENTENCE transfers control past the next sentence boundary
            // (a SentenceEnd marker); handled by exec_stmts.
            Stmt::NextSentence { .. } => Err(RuntimeError::NextSentence),
            Stmt::SentenceEnd { .. } => Ok(()),
            Stmt::Exit { kind, .. } => match kind {
                ExitKind::Point => Ok(()),
                ExitKind::Program => Err(RuntimeError::GoBack),
                ExitKind::Perform => Err(RuntimeError::ExitPerform { cycle: false }),
                ExitKind::PerformCycle => Err(RuntimeError::ExitPerform { cycle: true }),
                ExitKind::Paragraph => Err(RuntimeError::ExitParagraph),
                ExitKind::Section => Err(RuntimeError::ExitSection),
            },

            // ── I/O ───────────────────────────────────────────────────────────
            Stmt::Accept {
                target,
                from,
                screen,
                span,
            } => self.exec_accept(target, from.as_ref(), screen.as_ref(), *span),
            Stmt::Display {
                operands,
                no_advancing,
                screen,
                upon,
                ..
            } => self.exec_display(operands, *no_advancing, screen.as_ref(), upon.as_deref()),
            Stmt::Open {
                mode,
                files,
                lock,
                registered_user,
                span,
                ..
            } => self.exec_open(*mode, files, *lock, registered_user.as_ref(), *span),
            Stmt::Close { files, .. } => self.exec_close(files),
            Stmt::Write {
                record,
                from,
                invalid_key,
                not_invalid_key,
                span,
                ..
            } => self.exec_write(record, from.as_ref(), invalid_key, not_invalid_key, *span),
            Stmt::Read {
                file,
                into,
                key,
                direction,
                lock,
                at_end,
                not_at_end,
                invalid_key,
                not_invalid_key,
                span,
            } => self.exec_read(
                file,
                into.as_ref(),
                key.as_ref(),
                *direction,
                *lock,
                at_end,
                not_at_end,
                invalid_key,
                not_invalid_key,
                *span,
            ),
            Stmt::Rewrite {
                record,
                from,
                invalid_key,
                not_invalid_key,
                span,
            } => self.exec_rewrite(record, from.as_ref(), invalid_key, not_invalid_key, *span),
            Stmt::Delete {
                file,
                invalid_key,
                not_invalid_key,
                span,
            } => self.exec_delete(file, invalid_key, not_invalid_key, *span),
            Stmt::Start {
                file,
                key,
                invalid_key,
                not_invalid_key,
                span,
            } => self.exec_start(file, key.as_ref(), invalid_key, not_invalid_key, *span),

            // ── String handling ───────────────────────────────────────────────
            Stmt::String_ {
                operands,
                into,
                pointer,
                on_overflow,
                not_on_overflow,
                span,
            } => self.exec_string(
                operands,
                into,
                pointer.as_ref(),
                on_overflow,
                not_on_overflow,
                *span,
            ),
            Stmt::Unstring {
                from,
                delimited_by,
                all,
                into,
                pointer,
                tallying,
                on_overflow,
                not_on_overflow,
                span,
            } => self.exec_unstring(
                from,
                delimited_by,
                *all,
                into,
                pointer.as_ref(),
                tallying.as_ref(),
                on_overflow,
                not_on_overflow,
                *span,
            ),
            Stmt::Inspect { target, spec, span } => self.exec_inspect(target, spec, *span),

            // ── Sorting ───────────────────────────────────────────────────────
            Stmt::Sort {
                file,
                keys,
                using,
                giving,
                input_proc,
                output_proc,
                duplicates: _,
                span,
            } => self.exec_sort(
                file,
                keys,
                using,
                giving,
                input_proc.as_deref(),
                output_proc.as_deref(),
                *span,
            ),
            Stmt::Merge {
                file,
                keys,
                using,
                giving,
                output_proc,
                span,
            } => self.exec_sort(
                file,
                keys,
                using,
                giving,
                None,
                output_proc.as_deref(),
                *span,
            ),
            Stmt::Release { record, from, .. } => self.exec_release(record, from.as_ref()),
            Stmt::Return {
                file,
                into,
                at_end,
                not_at_end,
                ..
            } => self.exec_return(file, into.as_ref(), at_end, not_at_end),

            // ── Subprogram linkage ────────────────────────────────────────────
            Stmt::Call {
                program,
                using,
                returning,
                on_exception,
                not_on_exception,
                span,
            } => self.exec_call(
                program,
                using,
                returning.as_ref(),
                on_exception,
                not_on_exception,
                *span,
            ),

            Stmt::Invoke {
                object,
                method,
                args,
                returning,
                comma_form: _,
                span,
            } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval_expr(a, *span)?);
                }
                // 037 — window-supervisor calls (OpenForm*, handle methods,
                // me:: window methods) dispatch first: they can raise runtime
                // errors and OpenForm* must BIND the returning data-item as a
                // windowHandler variable (R20/R23/R24).
                if let Some(result) = self.try_exec_window_call(object, method, &vals)? {
                    if let Some(dest) = returning {
                        let s = result.as_display_string().trim().to_string();
                        let name = self.expr_to_name(dest);
                        if Self::method_returns_window_handle(method) {
                            self.window_handle_vars.insert(
                                name.to_ascii_uppercase(),
                                if s.is_empty() { None } else { Some(s.clone()) },
                            );
                        }
                        self.env.set_str(&name, &s);
                    }
                    return Ok(());
                }
                let result = self.exec_method(object, method, &vals);
                if let Some(dest) = returning {
                    // RETURNING into a member chain (`… RETURNING B::Caption`)
                    // assigns that place; otherwise into a data item (spec 011).
                    if matches!(dest, Expr::Member { .. }) {
                        self.assign_member(dest, &result)?;
                    } else {
                        let s = result.as_display_string();
                        let n = self.expr_to_name(dest);
                        self.env.set_str(&n, s.trim());
                    }
                }
                Ok(())
            }

            // Inline member-access chain used as a statement (spec 011):
            // `Grid-1::Rows(I)::Delete()`, `obj::UpperCase()` (result discarded).
            Stmt::InvokeExpr { expr, .. } => {
                self.eval_expr(expr, expr.span())?;
                Ok(())
            }

            // ── Program termination ───────────────────────────────────────────
            Stmt::Stop { run: true, .. } => Err(RuntimeError::StopRun),
            Stmt::Stop {
                run: false,
                literal,
                ..
            } => {
                if let Some(lit) = literal {
                    let s = match lit {
                        Literal::String(s) => s.clone(),
                        Literal::Integer(n) => n.to_string(),
                        _ => String::new(),
                    };
                    if !s.is_empty() {
                        println!("{s}");
                    }
                }
                Ok(())
            }
            Stmt::GoBack { .. } => Err(RuntimeError::GoBack),

            // ── EXEC RUST ─────────────────────────────────────────────────────
            Stmt::ExecRust { .. } => exec_rust::execute(
                stmt,
                &mut self.env,
                &mut self.objects,
                &mut self.rust_bridge,
                &self.exec_rust,
                self.state_tx.as_ref(),
            ),

            // ── TRY / CATCH EXCEPTION / FINALLY ──────────────────────────────
            Stmt::TryCatch {
                try_stmts,
                exception_var,
                catch_stmts,
                rust_exception_var,
                rust_catch_stmts,
                finally_stmts,
                ..
            } => {
                // Execute the TRY body. Each clause catches ONLY its own class
                // (spec 041 R24): a COBOL exception never reaches
                // CATCH RUST-EXCEPTION, and a contained Rust panic is never
                // swallowed by a plain CATCH EXCEPTION.
                let try_result = self.exec_stmts(try_stmts);

                let handled = match &try_result {
                    Err(RuntimeError::UserException { message }) => {
                        // Bind the exception message to the named variable if given.
                        let msg = message.clone();
                        if let Some(var) = exception_var {
                            self.env.set_str(var, &msg);
                        }
                        // Run the CATCH body.
                        self.exec_stmts(catch_stmts)?;
                        true
                    }
                    // A panic is caught only when a RUST-EXCEPTION clause was
                    // actually written; without one it propagates below, after
                    // FINALLY has run (R25).
                    Err(RuntimeError::RustPanic { message })
                        if rust_exception_var.is_some() || !rust_catch_stmts.is_empty() =>
                    {
                        let msg = message.clone();
                        if let Some(var) = rust_exception_var {
                            self.env.set_str(var, &msg);
                        }
                        self.exec_stmts(rust_catch_stmts)?;
                        true
                    }
                    _ => false,
                };

                // Always run FINALLY regardless of outcome.
                self.exec_stmts(finally_stmts)?;

                // If the error was not a UserException (or there was no catch),
                // propagate it now (after FINALLY ran).
                if !handled {
                    try_result?;
                }
                Ok(())
            }

            Stmt::Throw { message, span } => {
                let val = self.eval_expr(message, *span)?;
                Err(RuntimeError::UserException {
                    message: val.as_display_string(),
                })
            }

            // ── PowerCOBOL extensions ─────────────────────────────────────────
            Stmt::WindowOp { op, .. } => {
                tracing::debug!("WindowOp: {:?}", op);
                Ok(())
            }
            Stmt::ControlSet {
                control,
                property,
                value,
                span,
            } => {
                let ctrl = self.expr_to_name(control);
                let val = self.eval_expr(value, *span)?;
                self.objects
                    .set_property(&ctrl, property, val.as_display_string());
                Ok(())
            }
        }
    }

    // ── MOVE ─────────────────────────────────────────────────────────────────

    fn exec_move(&mut self, from: &Expr, to: &[Expr]) -> Result<(), RuntimeError> {
        let val = self.eval_expr(from, from.span())?;
        // A numeric source moved to an alphanumeric receiver de-edits to its
        // zero-padded digit string (left-justified), per COBOL MOVE rules.
        let src_digits = match from {
            Expr::Identifier(s, _) => self.env.deedited_digits(s),
            _ => None,
        };
        for target in to {
            // Reference-modified receiver: partial (spliced) assignment.
            if let Expr::RefMod {
                base,
                start,
                length,
                span,
            } = target
            {
                self.assign_refmod(base, start, length.as_deref(), &val, *span)?;
                continue;
            }
            // Member-access receiver: `MOVE value TO control::property` /
            // `… TO Grid::Rows(I)::Value` (spec 011). A method-call tail is not a
            // receiving field → `assign_member` raises an error.
            if matches!(target, Expr::Member { .. }) {
                self.assign_member(target, &val)?;
                continue;
            }
            let name = self.resolve_lvalue(target);
            // `SET 88-name TO TRUE|FALSE` arrives here as MOVE 1|0 → set the
            // host item to (a value satisfying / violating) the condition.
            if let Some(info) = self.env.cond_name(&name).cloned() {
                self.set_condition(&info, !val.is_zero());
                continue;
            }
            // A 66-level RENAMES receiver distributes across its covered items.
            if self.env.is_renames(&name) {
                self.env.set_renames(&name, &val.as_display_string());
                continue;
            }
            // An OBJECT REFERENCE receiver names an *object*; its storage slot
            // holds the bridge handle. The value has to be written through that
            // handle — storing it in the slot would overwrite the id and strand
            // the object, and the item would then fail to bind in the next block
            // ("handle 0 is not live").
            if self.object_refs.contains_key(&name) {
                self.assign_object_ref(&name, &val)?;
                continue;
            }
            match &src_digits {
                Some(digits) if self.env.is_alphanumeric_field(&name) => {
                    self.env.set_str_left(&name, digits);
                }
                _ => self.env.set(&name, val.clone()),
            }
        }
        Ok(())
    }

    /// Write `val` into the Rust object an `OBJECT REFERENCE` item names.
    ///
    /// A class the bridge cannot build from a COBOL value — a collection, or a
    /// developer-defined type — is a type error the developer needs to see: the
    /// alternative is to keep the write and lose the object, which is the defect
    /// this replaced.
    fn assign_object_ref(&mut self, key: &str, val: &CobolValue) -> Result<(), RuntimeError> {
        let id = self.env.get_i64(key).unwrap_or(0);
        let class = self.object_refs.get(key).cloned().unwrap_or_default();
        self.rust_bridge
            .assign(id, &cobol_to_bridge(val))
            .map_err(|e| RuntimeError::General {
                message: format!("cannot move a value into {key} ({class}): {e}"),
            })
    }

    /// Set the host item of an 88-level condition-name so the condition becomes
    /// `truthy` (its first VALUE) or false (a value outside its VALUE set).
    fn set_condition(&mut self, info: &crate::environment::CondName, truthy: bool) {
        use cobolt_ast::data::ConditionValue;
        if truthy {
            if let Some(cv) = info.values.first() {
                let v = match cv {
                    ConditionValue::Single(lit) => literal_to_value(lit),
                    ConditionValue::Range(lo, _) => literal_to_value(lo),
                };
                self.env.set(&info.parent, v);
            }
        } else {
            // SET … TO FALSE (no FALSE clause): pick the smallest small integer
            // that does not satisfy any declared VALUE.
            let mut candidate = 0i64;
            while self.value_satisfies(info, candidate) && candidate < 1000 {
                candidate += 1;
            }
            self.env.set(&info.parent, CobolValue::from_i64(candidate));
        }
    }

    /// `true` if integer `n` satisfies one of the condition-name's VALUEs.
    fn value_satisfies(&self, info: &crate::environment::CondName, n: i64) -> bool {
        use cobolt_ast::data::ConditionValue;
        let pv = CobolValue::from_i64(n);
        info.values.iter().any(|cv| match cv {
            ConditionValue::Single(lit) => compare_values(&pv, &literal_to_value(lit), CmpOp::Eq),
            ConditionValue::Range(lo, hi) => {
                compare_values(&pv, &literal_to_value(lo), CmpOp::Ge)
                    && compare_values(&pv, &literal_to_value(hi), CmpOp::Le)
            }
        })
    }

    // ── MOVE / ADD / SUBTRACT CORRESPONDING ─────────────────────────────────────

    /// `MOVE CORRESPONDING g1 TO g2`: for each pair of subordinate items that
    /// share a name, move (recursing through matching groups, moving matching
    /// elementary items). Items present in only one group are left untouched.
    fn move_corresponding(&mut self, from_key: &str, to_key: &str) -> Result<(), RuntimeError> {
        for (fk, tk, both_groups) in self.corr_pairs(from_key, to_key) {
            if both_groups {
                self.move_corresponding(&fk, &tk)?;
            } else {
                let val = self
                    .env
                    .get(&fk)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let src_digits = self.env.deedited_digits(&fk);
                match src_digits {
                    Some(digits) if self.env.is_alphanumeric_field(&tk) => {
                        self.env.set_str_left(&tk, &digits)
                    }
                    _ => self.env.set(&tk, val),
                }
            }
        }
        Ok(())
    }

    /// `ADD/SUBTRACT CORRESPONDING g1 TO/FROM g2`: combine each matching pair of
    /// elementary numeric items, recursing through matching groups.
    fn arith_corresponding(
        &mut self,
        from_key: &str,
        to_key: &str,
        subtract: bool,
    ) -> Result<(), RuntimeError> {
        for (fk, tk, both_groups) in self.corr_pairs(from_key, to_key) {
            if both_groups {
                self.arith_corresponding(&fk, &tk, subtract)?;
            } else {
                let a = self
                    .env
                    .get(&fk)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let cur = self
                    .env
                    .get(&tk)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let result = if subtract {
                    cur.sub_val(&a)
                } else {
                    cur.add_val(&a)
                };
                self.store_arith(&tk, result, false, false);
            }
        }
        Ok(())
    }

    /// Matching subordinate pairs of two groups: `(from_child_key,
    /// to_child_key, both_are_groups)` for every leaf name they share.
    fn corr_pairs(&self, from_key: &str, to_key: &str) -> Vec<(String, String, bool)> {
        let from_sym = match self.env.symbol(from_key) {
            Some(s) => s.clone(),
            None => return Vec::new(),
        };
        let to_sym = match self.env.symbol(to_key) {
            Some(s) => s.clone(),
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for (i, child) in from_sym.children.iter().enumerate() {
            if let Some(j) = to_sym.children.iter().position(|c| c == child) {
                let fk = from_sym.child_keys[i].clone();
                let tk = to_sym.child_keys[j].clone();
                let fg = self.env.symbol(&fk).map(|s| s.is_group).unwrap_or(false);
                let tg = self.env.symbol(&tk).map(|s| s.is_group).unwrap_or(false);
                out.push((fk, tk, fg && tg));
            }
        }
        out
    }

    // ── Pointers (SET ADDRESS OF / SET ptr TO ADDRESS OF / NULL) ────────────────

    fn exec_set_pointer(
        &mut self,
        address_of: Option<&Expr>,
        targets: &[Expr],
        source: &cobolt_ast::stmt::PointerSource,
    ) -> Result<(), RuntimeError> {
        use cobolt_ast::stmt::PointerSource;
        // Resolve the source to the storage key it addresses (None = NULL).
        let target_key: Option<String> = match source {
            PointerSource::Null => None,
            PointerSource::AddressOf(e) => Some(self.expr_to_name(e)),
            PointerSource::Pointer(e) => {
                let id = self.eval_expr(e, e.span())?.as_i64().unwrap_or(0);
                self.env.addr_target(id)
            }
        };
        if let Some(item) = address_of {
            // `SET ADDRESS OF item TO …` — (re)alias item onto target's storage.
            let alias = self.canonical_no_alias(item);
            match &target_key {
                Some(t) => self.env.set_alias(&alias, t),
                None => self.env.clear_alias(&alias),
            }
        } else {
            // `SET ptr … TO ADDRESS OF x` — store the address id (0 = NULL).
            let id = match &target_key {
                Some(t) => self.env.addr_of(t),
                None => 0,
            };
            for tgt in targets {
                let name = self.resolve_lvalue(tgt);
                self.env.set(&name, CobolValue::from_i64(id));
            }
        }
        Ok(())
    }

    /// Canonical key for an lvalue **without** following an address alias.
    fn canonical_no_alias(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name, _) => self.env.canonical_name(name, &[]),
            Expr::Qualified { name, of, .. } => self.env.canonical_name(name, &collect_quals(of)),
            _ => self.expr_to_name(expr),
        }
    }

    // ── SEARCH / SEARCH ALL ─────────────────────────────────────────────────────

    fn exec_search(
        &mut self,
        all: bool,
        table: &Expr,
        varying: Option<&Expr>,
        at_end: &[Stmt],
        whens: &[(Condition, Vec<Stmt>)],
    ) -> Result<(), RuntimeError> {
        let table_name = self.expr_to_name(table);
        let sym = self.env.symbol(&table_name).cloned();
        // Table size = the table's own OCCURS count (its last dimension).
        let size = sym
            .as_ref()
            .map(|s| {
                if s.occurs > 0 {
                    s.occurs
                } else {
                    s.dims.last().copied().unwrap_or(0)
                }
            })
            .unwrap_or(0);
        // Index = VARYING item, else the table's first INDEXED BY index.
        let index_name = match varying {
            Some(v) => self.expr_to_name(v),
            None => sym
                .as_ref()
                .and_then(|s| s.index_names.first().cloned())
                .unwrap_or_default(),
        };
        if index_name.is_empty() || size == 0 {
            return self.exec_stmts(at_end);
        }

        // ── SEARCH ALL: binary search over a table ordered on its declared
        // ASCENDING/DESCENDING KEY(s). Requires exactly one WHEN whose condition
        // is a conjunction of equality tests on the key item(s), major to minor.
        let keys = sym.as_ref().map(|s| s.keys.clone()).unwrap_or_default();
        if all && !keys.is_empty() && whens.len() == 1 {
            let (cond, body) = &whens[0];
            // Equality comparisons of the WHEN, in major-to-minor order.
            let mut comps: Vec<(&Expr, &Expr, Span)> = Vec::new();
            flatten_eq_comparisons(cond, &mut comps);

            let mut lo: i64 = 1;
            let mut hi: i64 = size as i64;
            while lo <= hi {
                let mid = lo + (hi - lo) / 2;
                self.env.set_i64(&index_name, mid);
                if self.eval_condition(cond)? {
                    return self.exec_stmts(body);
                }
                // Direction: the first key whose value at `mid` differs from its
                // target decides which half to keep (adjusted for DESCENDING).
                let mut dir = std::cmp::Ordering::Equal;
                for (lhs, rhs, span) in &comps {
                    let lv = self.eval_expr(lhs, *span)?;
                    let rv = self.eval_expr(rhs, *span)?;
                    let ord = cob_ordering(&lv, &rv);
                    if ord != std::cmp::Ordering::Equal {
                        let field = self.expr_to_name(lhs).to_ascii_uppercase();
                        let ascending = keys
                            .iter()
                            .find(|(k, _)| *k == field)
                            .map(|(_, a)| *a)
                            .unwrap_or(true);
                        dir = if ascending { ord } else { ord.reverse() };
                        break;
                    }
                }
                match dir {
                    std::cmp::Ordering::Less => lo = mid + 1,
                    std::cmp::Ordering::Greater => hi = mid - 1,
                    // Keys equal but WHEN false (or no usable key comparison) →
                    // the target is not present.
                    std::cmp::Ordering::Equal => break,
                }
            }
            return self.exec_stmts(at_end);
        }

        // ── Serial SEARCH (and SEARCH ALL fallback when no keys are declared):
        // SEARCH ALL scans from the start; serial SEARCH from the current index.
        let start = if all {
            1
        } else {
            self.env.get_i64(&index_name).unwrap_or(1).max(1)
        };
        let mut i = start;
        while i <= size as i64 {
            self.env.set_i64(&index_name, i);
            for (cond, body) in whens {
                if self.eval_condition(cond)? {
                    return self.exec_stmts(body);
                }
            }
            i += 1;
        }
        // No WHEN matched within the table → run AT END.
        self.exec_stmts(at_end)
    }

    // ── INITIALIZE (category-aware) ─────────────────────────────────────────────

    fn exec_initialize(
        &mut self,
        items: &[Expr],
        replacing: &[(cobolt_ast::stmt::InitCategory, Expr)],
    ) -> Result<(), RuntimeError> {
        // Evaluate each REPLACING value once.
        let mut repl = Vec::with_capacity(replacing.len());
        for (cat, e) in replacing {
            repl.push((*cat, self.eval_expr(e, e.span())?));
        }
        for item in items {
            // Member chain (`INITIALIZE obj::value`) → reset that property to its
            // category default (spec 011). A method-call tail is skipped.
            if matches!(item, Expr::Member { .. }) {
                if let Ok((root, Resolved::Path(path))) = self.resolve_member(item) {
                    let def = self.init_default_for_member(&root, &path);
                    self.set_member(&root, &path, def);
                }
                continue;
            }
            // A bare identifier that is **not** a declared data item is treated as
            // a control object: `INITIALIZE obj` implicitly resets its `Value`
            // property (spec 011). `INITIALIZE obj name` thus inits each operand
            // by its own rules (control → Value, data item → PIC default).
            if let Expr::Identifier(id, _) = item {
                if !self.env.contains(id) {
                    let path = vec![PathSeg::Prop("Value".into())];
                    let def = self.init_default_for_member(id, &path);
                    self.set_member(id, &path, def);
                    continue;
                }
            }
            let name = self.resolve_lvalue(item);
            // Walk the DATA DIVISION for the item's declaration so groups recurse
            // into their elementary children; fall back to field-cap inference.
            let decl = self
                .program
                .data
                .as_ref()
                .and_then(|d| find_decl_in_division(d, &name))
                .cloned();
            match decl {
                Some(d) if repl.is_empty() => self.init_decl(&d),
                Some(d) => self.init_decl_replacing(&d, &repl),
                None => self.init_by_caps(&name),
            }
        }
        Ok(())
    }

    /// The category default for a control property being `INITIALIZE`d: a
    /// numeric-looking value resets to `"0"`, anything else to the empty string
    /// (spec 011 — properties have no PIC, so the current value's shape decides).
    fn init_default_for_member(&self, root: &str, path: &[PathSeg]) -> String {
        let cur = self
            .objects
            .get_path(root, path)
            .map(|v| v.to_string())
            .unwrap_or_default();
        if !cur.trim().is_empty() && crate::value::parse_decimal(cur.trim()).is_some() {
            "0".to_string()
        } else {
            String::new()
        }
    }

    /// `INITIALIZE … REPLACING`: set each subordinate elementary item whose
    /// category matches a REPLACING entry to that value; leave others untouched.
    fn init_decl_replacing(
        &mut self,
        d: &cobolt_ast::data::DataDecl,
        repl: &[(cobolt_ast::stmt::InitCategory, CobolValue)],
    ) {
        use cobolt_ast::data::PicKind;
        use cobolt_ast::stmt::InitCategory;
        if !d.children.is_empty() {
            for c in &d.children {
                if c.level != 88 && c.level != 66 {
                    self.init_decl_replacing(c, repl);
                }
            }
            return;
        }
        let Some(name) = &d.name else { return };
        let cat = match d.picture.as_ref().map(|p| p.kind) {
            Some(PicKind::Alphabetic) => InitCategory::Alphabetic,
            Some(PicKind::Alphanumeric) => InitCategory::Alphanumeric,
            Some(PicKind::Numeric) => InitCategory::Numeric,
            Some(PicKind::AlphanumericEdited) => InitCategory::AlphanumericEdited,
            Some(PicKind::NumericEdited) => InitCategory::NumericEdited,
            None => return,
        };
        if let Some((_, val)) = repl.iter().find(|(c, _)| *c == cat) {
            let key = self.env.resolve_name(name, &[]);
            if self.env.is_alphanumeric_field(&key) {
                self.env.set_str_left(&key, &val.as_display_string());
            } else {
                self.env.set(&key, val.clone());
            }
        }
    }

    /// Recursively initialise a declaration: groups recurse; elementary items
    /// reset to ZERO (numeric) or SPACES (everything else).
    fn init_decl(&mut self, d: &cobolt_ast::data::DataDecl) {
        use cobolt_ast::data::PicKind;
        if !d.children.is_empty() {
            for c in &d.children {
                self.init_decl(c);
            }
        } else if let Some(name) = &d.name {
            let key = name.to_ascii_uppercase();
            let numeric = matches!(
                d.picture.as_ref().map(|p| p.kind),
                Some(PicKind::Numeric) | Some(PicKind::NumericEdited)
            );
            if numeric {
                let decimals = d
                    .picture
                    .as_ref()
                    .map(|p| p.decimals)
                    .unwrap_or(0)
                    .min(u8::MAX as u16) as u8;
                self.env
                    .set(&key, CobolValue::Numeric(CobolNumeric::new(0, decimals)));
            } else {
                let width = self.env.display_string(&key).map(|s| s.len()).unwrap_or(0);
                self.env.set_str(&key, &" ".repeat(width));
            }
        }
    }

    /// Initialise an item not found in the AST, using field-capacity inference
    /// (numeric → 0, otherwise spaces preserving width).
    fn init_by_caps(&mut self, name: &str) {
        if self.env.integer_capacity(name).is_some() {
            self.env.set(name, CobolValue::from_i64(0));
        } else {
            let width = self.env.display_string(name).map(|s| s.len()).unwrap_or(0);
            self.env.set_str(name, &" ".repeat(width));
        }
    }

    // ── Arithmetic ────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn exec_add(
        &mut self,
        operands: &[Expr],
        to: &[(Expr, bool)],
        giving: &[(Expr, bool)],
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let sum = self.eval_sum(operands, span)?;
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if !giving.is_empty() {
            // `ADD a … TO b … GIVING c …` → c = sum(a…) + sum(b…).
            let mut total = sum;
            for (t, _) in to {
                let v = self.eval_expr(t, span)?;
                total = total.add_val(&v);
            }
            for (g, rounded) in giving {
                let name = self.resolve_lvalue(g);
                size_err |= self.store_arith(&name, total.clone(), *rounded, has);
            }
        } else {
            for (t, rounded) in to {
                let name = self.resolve_lvalue(t);
                let cur = self
                    .env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let result = cur.add_val(&sum);
                size_err |= self.store_arith(&name, result, *rounded, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_subtract(
        &mut self,
        operands: &[Expr],
        from: &[(Expr, bool)],
        giving: &[(Expr, bool)],
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let sub = self.eval_sum(operands, span)?;
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if !giving.is_empty() {
            // `SUBTRACT a … FROM base GIVING c …` → c = base − sum(a…).
            let base = if from.is_empty() {
                CobolValue::from_i64(0)
            } else {
                self.eval_expr(&from[0].0, span)?
            };
            let result = base.sub_val(&sub);
            for (g, rounded) in giving {
                let name = self.resolve_lvalue(g);
                size_err |= self.store_arith(&name, result.clone(), *rounded, has);
            }
        } else {
            for (f, rounded) in from {
                let name = self.resolve_lvalue(f);
                let cur = self
                    .env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let result = cur.sub_val(&sub);
                size_err |= self.store_arith(&name, result, *rounded, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_multiply(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: &[(Expr, bool)],
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let l = self.eval_expr(lhs, span)?;
        let r = self.eval_expr(by, span)?;
        let result = l.mul_val(&r);
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if giving.is_empty() {
            // `MULTIPLY a BY b [ROUNDED]` → b = a × b.
            let name = self.resolve_lvalue(by);
            size_err = self.store_arith(&name, result, rounded, has);
        } else {
            for (g, gr) in giving {
                let name = self.resolve_lvalue(g);
                size_err |= self.store_arith(&name, result.clone(), *gr, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_divide(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: &[(Expr, bool)],
        remainder: Option<&Expr>,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let l = self.eval_expr(lhs, span)?;
        let r = self.eval_expr(by, span)?;
        let quotient = match l.div_val(&r) {
            Some(q) => q,
            None => {
                // Division by zero raises a size error if a handler is present.
                if !on_size_error.is_empty() {
                    return self.run_size_error(true, on_size_error, not_on_size_error);
                }
                return Err(RuntimeError::DivisionByZero { span });
            }
        };

        if let Some(rem_expr) = remainder {
            // COBOL REMAINDER uses the *integer* quotient: rem = dividend − (intq × divisor).
            let int_q = CobolValue::from_i64(quotient.as_i64().unwrap_or(0));
            let rem_val = l.sub_val(&int_q.mul_val(&r));
            let rname = self.resolve_lvalue(rem_expr);
            self.env.set(&rname, rem_val);
        }

        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if giving.is_empty() {
            // No GIVING: store the quotient back into the dividend (`lhs`).
            let name = self.resolve_lvalue(lhs);
            size_err = self.store_arith(&name, quotient, rounded, has);
        } else {
            for (g, gr) in giving {
                let name = self.resolve_lvalue(g);
                size_err |= self.store_arith(&name, quotient.clone(), *gr, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    fn exec_compute(
        &mut self,
        targets: &[(Expr, bool)],
        expr: &Expr,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let val = self.eval_expr(expr, span)?;
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        for (target, rounded) in targets {
            let name = self.resolve_lvalue(target);
            size_err |= self.store_arith(&name, val.clone(), *rounded, has);
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    /// After an arithmetic store, run the appropriate conditional imperative.
    fn run_size_error(
        &mut self,
        size_err: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
    ) -> Result<(), RuntimeError> {
        if size_err {
            if !on_size_error.is_empty() {
                self.exec_stmts(on_size_error)?;
            }
        } else if !not_on_size_error.is_empty() {
            self.exec_stmts(not_on_size_error)?;
        }
        Ok(())
    }

    /// Store an arithmetic result into `name`, returning `true` if a size error
    /// occurred (the value's integer part exceeds the field's PIC capacity).
    ///
    /// When `rounded`, round (half away from zero) to the field's scale;
    /// otherwise `assign` truncates, per COBOL's default. On a size error *with*
    /// a handler (`suppress_on_overflow`), the field is left unchanged.
    fn store_arith(
        &mut self,
        name: &str,
        value: CobolValue,
        rounded: bool,
        suppress_on_overflow: bool,
    ) -> bool {
        let value = if rounded {
            let scale = match self.env.get(name) {
                Some(CobolValue::Numeric(f)) => Some(f.decimals),
                _ => None,
            };
            match (scale, value.as_exact()) {
                (Some(s), Some(num)) => CobolValue::Numeric(num.round_to(s)),
                _ => value,
            }
        } else {
            value
        };

        // Size error: does the integer part fit the receiving field's capacity?
        let overflow = match (self.env.integer_capacity(name), value.as_exact()) {
            (Some(cap), Some(num)) => num.integer_digit_count() > cap as u32,
            _ => false,
        };

        if overflow && suppress_on_overflow {
            // Leave the receiving field unchanged; caller runs ON SIZE ERROR.
            return true;
        }
        self.env.set(name, value);
        overflow
    }

    /// Sum a list of expressions to a single `CobolValue`.
    fn eval_sum(&mut self, operands: &[Expr], span: Span) -> Result<CobolValue, RuntimeError> {
        let mut total = CobolValue::from_i64(0);
        for op in operands {
            let v = self.eval_expr(op, span)?;
            total = total.add_val(&v);
        }
        Ok(total)
    }

    // ── Control flow ──────────────────────────────────────────────────────────

    fn exec_if(
        &mut self,
        condition: &Condition,
        then_stmts: &[Stmt],
        else_stmts: &[Stmt],
    ) -> Result<(), RuntimeError> {
        if self.eval_condition(condition)? {
            self.exec_stmts(then_stmts)
        } else {
            self.exec_stmts(else_stmts)
        }
    }

    fn exec_evaluate(
        &mut self,
        subjects: &[EvalSubject],
        whens: &[WhenClause],
        other_stmts: &[Stmt],
    ) -> Result<(), RuntimeError> {
        for (idx, when) in whens.iter().enumerate() {
            // A WHEN whose every column is OTHER is the catch-all.
            let is_other = !when.values.is_empty()
                && when.values.iter().all(|v| matches!(v, WhenValue::Other));
            let matched = if is_other {
                true
            } else if when.values.is_empty() {
                // An empty selector only arises from a stacked WHEN; it cannot
                // match on its own — its alternatives precede it.
                false
            } else {
                // Each column is matched against the corresponding subject; the
                // WHEN matches when every column matches (ALSO = AND).
                let mut all = true;
                for (i, val) in when.values.iter().enumerate() {
                    let subj = match subjects.get(i) {
                        Some(s) => s,
                        None => {
                            all = false;
                            break;
                        }
                    };
                    if !self.when_value_matches(subj, val)? {
                        all = false;
                        break;
                    }
                }
                all
            };
            if matched {
                // Stacked WHEN: two or more consecutive WHEN phrases share the
                // single imperative that follows them. The matched selector may
                // itself be empty — borrow the next clause that carries
                // statements (or fall through to WHEN OTHER if none does).
                let mut j = idx;
                while j < whens.len() && whens[j].stmts.is_empty() {
                    j += 1;
                }
                return if j < whens.len() {
                    self.exec_stmts(&whens[j].stmts)
                } else {
                    self.exec_stmts(other_stmts)
                };
            }
        }
        // WHEN OTHER / no match
        self.exec_stmts(other_stmts)
    }

    fn when_value_matches(
        &mut self,
        subject: &EvalSubject,
        val: &WhenValue,
    ) -> Result<bool, RuntimeError> {
        match (subject, val) {
            (_, WhenValue::Any) => Ok(true),
            (_, WhenValue::Other) => Ok(false), // handled specially in exec_evaluate
            (s, WhenValue::Not(inner)) => Ok(!self.when_value_matches(s, inner)?),
            (EvalSubject::True_, WhenValue::Condition(c)) => self.eval_condition(c),
            (EvalSubject::False_, WhenValue::Condition(c)) => Ok(!self.eval_condition(c)?),
            (EvalSubject::Expr(e), WhenValue::Literal(lit)) => {
                let subj = self.eval_expr(e, e.span())?;
                let lv = literal_to_value(lit);
                Ok(compare_values(&subj, &lv, CmpOp::Eq))
            }
            (EvalSubject::Expr(e), WhenValue::Range(lo, hi)) => {
                let subj = self.eval_expr(e, e.span())?;
                let lo_v = literal_to_value(lo);
                let hi_v = literal_to_value(hi);
                Ok(compare_values(&subj, &lo_v, CmpOp::Ge)
                    && compare_values(&subj, &hi_v, CmpOp::Le))
            }
            (EvalSubject::Expr(e), WhenValue::Condition(c)) => {
                // EVALUATE expr WHEN condition — treat condition as boolean check
                let _ = e;
                self.eval_condition(c)
            }
            _ => Ok(false),
        }
    }

    /// Run a performed paragraph/section body, absorbing the signals that mean
    /// "return from this paragraph/section": `EXIT PARAGRAPH`, `EXIT SECTION`,
    /// and `NEXT SENTENCE` reaching the end.
    fn exec_para_body(&mut self, stmts: &[Stmt]) -> Result<(), RuntimeError> {
        match self.exec_stmts(stmts) {
            Err(RuntimeError::ExitParagraph)
            | Err(RuntimeError::ExitSection)
            | Err(RuntimeError::NextSentence) => Ok(()),
            other => other,
        }
    }

    /// Run one inline-PERFORM loop body, translating `EXIT PERFORM [CYCLE]`
    /// signals into loop control: `CYCLE` → next iteration, plain → break.
    fn exec_loop_body(&mut self, stmts: &[Stmt]) -> LoopStep {
        match self.exec_stmts(stmts) {
            Ok(()) => LoopStep::Continue,
            Err(RuntimeError::ExitPerform { cycle: true }) => LoopStep::Continue,
            Err(RuntimeError::ExitPerform { cycle: false }) => LoopStep::Break,
            Err(e) => LoopStep::Err(e),
        }
    }

    fn exec_perform(&mut self, target: &PerformTarget, span: Span) -> Result<(), RuntimeError> {
        if self.perform_depth >= MAX_PERFORM_DEPTH {
            return Err(RuntimeError::PerformDepthExceeded {
                max: MAX_PERFORM_DEPTH,
            });
        }
        self.perform_depth += 1;
        let result = self.exec_perform_inner(target, span);
        self.perform_depth -= 1;
        // Absorb GoBack inside a PERFORM (it means "return from this PERFORM").
        match result {
            Err(RuntimeError::GoBack) => Ok(()),
            other => other,
        }
    }

    fn exec_perform_inner(
        &mut self,
        target: &PerformTarget,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match target {
            PerformTarget::Paragraph(name, s) => {
                let stmts = self.para_stmts(name, *s)?;
                // Track the active paragraph so ALTER overrides resolve correctly.
                let prev =
                    std::mem::replace(&mut self.current_paragraph, name.to_ascii_uppercase());
                let r = self.exec_para_body(&stmts);
                self.current_paragraph = prev;
                r
            }
            PerformTarget::Section(name, s) => {
                // Treat a section PERFORM as executing all paragraphs in it.
                // We collect paragraphs whose names start with SECTION-NAME-*
                // (or exactly match).  Simplified: just find by name.
                match self.para_stmts(name, *s) {
                    Ok(stmts) => self.exec_para_body(&stmts),
                    Err(_) => {
                        // Try section as a block of paragraphs
                        let upper = name.to_ascii_uppercase();
                        let stmts = self.collect_section_stmts(&upper);
                        self.exec_para_body(&stmts)
                    }
                }
            }
            PerformTarget::Thru { from, to, span: s } => {
                let stmts = self.thru_stmts(from, to, *s)?;
                self.exec_para_body(&stmts)
            }
            PerformTarget::Inline { stmts } => match self.exec_loop_body(stmts) {
                LoopStep::Continue | LoopStep::Break => Ok(()),
                LoopStep::Err(e) => Err(e),
            },
            PerformTarget::Times { count, stmts } => {
                let n = self.eval_expr(count, span)?.as_i64().unwrap_or(0).max(0);
                for _ in 0..n {
                    match self.exec_loop_body(stmts) {
                        LoopStep::Continue => {}
                        LoopStep::Break => break,
                        LoopStep::Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            PerformTarget::Until {
                condition,
                test_before,
                stmts,
            } => {
                if *test_before {
                    while !self.eval_condition(condition)? {
                        match self.exec_loop_body(stmts) {
                            LoopStep::Continue => {}
                            LoopStep::Break => break,
                            LoopStep::Err(e) => return Err(e),
                        }
                    }
                } else {
                    loop {
                        match self.exec_loop_body(stmts) {
                            LoopStep::Continue => {}
                            LoopStep::Break => break,
                            LoopStep::Err(e) => return Err(e),
                        }
                        if self.eval_condition(condition)? {
                            break;
                        }
                    }
                }
                Ok(())
            }
            PerformTarget::Varying {
                var,
                from,
                by,
                until,
                stmts,
                after,
            } => self.exec_perform_varying(var, from, by, until, stmts, after, span),
        }
    }

    fn exec_perform_varying(
        &mut self,
        var: &Expr,
        from: &Expr,
        by: &Expr,
        until: &Condition,
        stmts: &[Stmt],
        after: &[VaryingAfter],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let from_val = self.eval_expr(from, span)?;
        let var_name = self.resolve_lvalue(var);
        self.env.set(&var_name, from_val);

        // Initialise AFTER variables
        for aft in after {
            let aft_from = self.eval_expr(&aft.from, span)?;
            let aft_name = self.resolve_lvalue(&aft.var);
            self.env.set(&aft_name, aft_from);
        }

        loop {
            if self.eval_condition(until)? {
                break;
            }

            // Inner AFTER loops (right-most varies fastest). `EXIT PERFORM`
            // (without CYCLE) anywhere inside breaks out of the whole VARYING.
            if self.exec_perform_after(after, stmts, span)? {
                break;
            }

            // Increment outer variable
            let by_val = self.eval_expr(by, span)?;
            let cur = self
                .env
                .get(&var_name)
                .cloned()
                .unwrap_or_else(|| CobolValue::from_i64(0));
            self.env.set(&var_name, cur.add_val(&by_val));
        }
        Ok(())
    }

    /// Returns `Ok(true)` when an `EXIT PERFORM` (no CYCLE) requested the entire
    /// VARYING be terminated; `EXIT PERFORM CYCLE` continues the innermost loop.
    fn exec_perform_after(
        &mut self,
        after: &[VaryingAfter],
        stmts: &[Stmt],
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if after.is_empty() {
            return match self.exec_loop_body(stmts) {
                LoopStep::Continue => Ok(false),
                LoopStep::Break => Ok(true),
                LoopStep::Err(e) => Err(e),
            };
        }
        let (head, tail) = (&after[0], &after[1..]);
        let from_val = self.eval_expr(&head.from, span)?;
        let var_name = self.resolve_lvalue(&head.var);
        self.env.set(&var_name, from_val);

        loop {
            if self.eval_condition(&head.until)? {
                break;
            }
            if self.exec_perform_after(tail, stmts, span)? {
                return Ok(true);
            }
            let by_val = self.eval_expr(&head.by, span)?;
            let cur = self
                .env
                .get(&var_name)
                .cloned()
                .unwrap_or_else(|| CobolValue::from_i64(0));
            self.env.set(&var_name, cur.add_val(&by_val));
        }
        Ok(false)
    }

    fn exec_go_to_depending(
        &mut self,
        targets: &[String],
        depending: &Expr,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let idx = self.eval_expr(depending, span)?.as_i64().unwrap_or(0);
        if idx >= 1 && (idx as usize) <= targets.len() {
            Err(RuntimeError::GoTo {
                target: targets[(idx - 1) as usize].clone(),
            })
        } else {
            Ok(()) // out-of-range → fall through
        }
    }

    // ── ACCEPT / DISPLAY ──────────────────────────────────────────────────────

    fn exec_accept(
        &mut self,
        target: &Expr,
        from: Option<&AcceptSource>,
        screen: Option<&cobolt_ast::stmt::ScreenPhrase>,
        _span: Span,
    ) -> Result<(), RuntimeError> {
        let name = self.resolve_lvalue(target);
        // Extended ACCEPT with a screen position: place the cursor first (CLI).
        if let (Some(sc), None) = (screen, &self.display_tx) {
            use std::io::Write;
            let (row, col) = self.screen_pos(sc);
            print!("\x1b[{row};{col}H");
            let _ = std::io::stdout().flush();
        }
        match from {
            None => {
                // Read one line from stdin.
                use std::io::BufRead;
                let stdin = std::io::stdin();
                let mut line = String::new();
                let _ = stdin.lock().read_line(&mut line);
                let s = line
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_owned();
                self.env.set_str(&name, &s);
            }
            Some(AcceptSource::Date) => self.env.set_str(&name, &runtime_date()),
            Some(AcceptSource::Time) => self.env.set_str(&name, &runtime_time()),
            Some(AcceptSource::Day) => self.env.set_str(&name, &runtime_julian_day()),
            Some(AcceptSource::DayOfWeek) => self.env.set_i64(&name, runtime_day_of_week()),
            Some(AcceptSource::CommandLine) => {
                self.env.set_str(&name, &self.program_args.join(" "));
            }
            Some(AcceptSource::ArgumentNumber) => {
                self.env.set_i64(&name, self.program_args.len() as i64);
            }
            Some(AcceptSource::ArgumentValue) => {
                let val = self
                    .program_args
                    .get(self.argument_pointer.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                self.env.set_str(&name, &val);
            }
            Some(AcceptSource::EnvironmentValue) => {
                let val = std::env::var(&self.env_name_register).unwrap_or_default();
                self.env.set_str(&name, &val);
            }
            Some(AcceptSource::EscapeKey) => self.env.set_str(&name, "00"),
            Some(AcceptSource::CrtStatus) => self.env.set_str(&name, "0000"),
            Some(AcceptSource::Environment(var)) => {
                let val = std::env::var(var).unwrap_or_default();
                self.env.set_str(&name, &val);
            }
        }
        Ok(())
    }

    fn exec_display(
        &mut self,
        operands: &[Expr],
        no_advancing: bool,
        screen: Option<&cobolt_ast::stmt::ScreenPhrase>,
        upon: Option<&str>,
    ) -> Result<(), RuntimeError> {
        // `DISPLAY … UPON {ARGUMENT-NUMBER | ENVIRONMENT-NAME}` sets a register
        // consumed by a later ACCEPT — it produces no output.
        match upon.map(|u| u.to_ascii_uppercase()) {
            Some(ref u) if u == "ARGUMENT-NUMBER" => {
                if let Some(op) = operands.first() {
                    let n = self.eval_expr(op, op.span())?.as_i64().unwrap_or(1);
                    self.argument_pointer = n.max(1) as usize;
                }
                return Ok(());
            }
            Some(ref u) if u == "ENVIRONMENT-NAME" => {
                if let Some(op) = operands.first() {
                    self.env_name_register = self
                        .eval_expr(op, op.span())?
                        .as_display_string()
                        .trim()
                        .to_string();
                }
                return Ok(());
            }
            _ => {}
        }
        let mut out = String::new();
        for op in operands {
            // A bare numeric data item displays as its full fixed-width digit
            // string (leading zeros per PIC); everything else renders verbatim.
            let s = match op {
                // A data-item reference (plain, qualified, or subscripted) shows
                // its full fixed-width digit string (leading zeros per PIC) via
                // the resolved storage key; literals/expressions render verbatim.
                Expr::Identifier(..) | Expr::Qualified { .. } | Expr::Subscript { .. } => {
                    let key = self.resolve_lvalue(op);
                    // An OBJECT REFERENCE item displays its bridge VALUE, not
                    // its handle id — same dereference the Identifier eval arm
                    // performs; the env's fixed-width path knows nothing of it.
                    if self.object_refs.contains_key(&key) {
                        self.eval_expr(op, op.span())?.as_display_string()
                    } else {
                        match self.env.display_string(&key) {
                            Some(s) => s,
                            None => self.eval_expr(op, op.span())?.as_display_string(),
                        }
                    }
                }
                _ => self.eval_expr(op, op.span())?.as_display_string(),
            };
            out.push_str(&s);
        }
        // GUI mode: send through the display channel so the IDE output panel
        // receives the text (cursor positioning is meaningless there).
        if let Some(tx) = &self.display_tx {
            let _ = tx.send(out.clone());
            return Ok(());
        }
        // CLI mode: honour the extended-screen position / attributes with ANSI.
        use std::io::Write;
        if let Some(sc) = screen {
            let (row, col) = self.screen_pos(sc);
            let attrs = screen_attrs(sc);
            print!("\x1b[{row};{col}H{attrs}{out}\x1b[0m");
            let _ = std::io::stdout().flush();
        } else if no_advancing {
            print!("{out}");
            let _ = std::io::stdout().flush();
        } else {
            println!("{out}");
        }
        Ok(())
    }

    /// Resolve a screen phrase to a 1-based `(row, col)` terminal position.
    fn screen_pos(&mut self, sc: &cobolt_ast::stmt::ScreenPhrase) -> (i64, i64) {
        if let Some(at) = &sc.at {
            let v = self
                .eval_expr(at, at.span())
                .map(|x| x.as_i64().unwrap_or(0))
                .unwrap_or(0);
            return ((v / 100).max(1), (v % 100).max(1));
        }
        let row = sc
            .line
            .as_ref()
            .and_then(|e| self.eval_expr(e, e.span()).ok())
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1);
        let col = sc
            .col
            .as_ref()
            .and_then(|e| self.eval_expr(e, e.span()).ok())
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1);
        (row, col)
    }

    // ── STRING ────────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn exec_string(
        &mut self,
        operands: &[(Expr, Option<Expr>)],
        into: &Expr,
        pointer: Option<&Expr>,
        on_overflow: &[Stmt],
        not_on_overflow: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let mut result = String::new();
        for (src_expr, delim_expr) in operands {
            let (src, is_alpha_item) = self.string_operand(src_expr, span)?;
            if let Some(delim_e) = delim_expr {
                let delim = self.eval_expr(delim_e, span)?.as_display_string();
                let delim_upper = delim.trim().to_ascii_uppercase();
                if delim_upper == "SIZE" {
                    result.push_str(&src);
                } else if delim_upper == "SPACE" || delim_upper == "SPACES" {
                    result.push_str(src.trim_end());
                } else if let Some(pos) = src.find(delim.as_str()) {
                    result.push_str(&src[..pos]);
                } else {
                    result.push_str(&src);
                }
            } else if is_alpha_item {
                // No DELIMITED BY: a plain alphanumeric data item defaults to
                // DELIMITED BY SPACES (drop the trailing space padding).
                result.push_str(src.trim_end());
            } else {
                // No DELIMITED BY: literals, numeric / numeric-edited items,
                // function results and computed values default to DELIMITED BY
                // SIZE (the whole value is moved).
                result.push_str(&src);
            }
        }
        let name = self.resolve_lvalue(into);
        let capacity = self
            .env
            .display_string(&name)
            .map(|s| s.len())
            .unwrap_or(usize::MAX);

        let overflowed = match pointer {
            // ── WITH POINTER: place from the 1-based pointer position, preserve
            // the bytes before it, and advance the pointer past the last byte
            // moved. Overflow when the assembled text does not fit from there.
            Some(ptr_e) => {
                let ptr_name = self.resolve_lvalue(ptr_e);
                let start = self.env.get_i64(&ptr_name).unwrap_or(1).max(1) as usize;
                let mut dest: Vec<char> = {
                    let cur = self.env.display_string(&name).unwrap_or_default();
                    let mut v: Vec<char> = cur.chars().collect();
                    if capacity != usize::MAX {
                        v.resize(capacity, ' ');
                    }
                    v
                };
                let mut idx = start - 1;
                let mut placed = 0usize;
                let mut overflow = start - 1 >= capacity && !result.is_empty();
                for ch in result.chars() {
                    if capacity != usize::MAX && idx >= capacity {
                        overflow = true;
                        break;
                    }
                    if idx < dest.len() {
                        dest[idx] = ch;
                    }
                    idx += 1;
                    placed += 1;
                }
                let new_val: String = dest.into_iter().collect();
                self.env.set_str(&name, &new_val);
                self.env.set_i64(&ptr_name, (start + placed) as i64);
                overflow
            }
            // ── No POINTER: replace the receiving field (left-justified,
            // space-padded by set_str). Overflow when the text is too wide.
            None => {
                let overflow = result.len() > capacity;
                self.env.set_str(&name, &result);
                overflow
            }
        };
        if overflowed {
            self.exec_stmts(on_overflow)?;
        } else {
            self.exec_stmts(not_on_overflow)?;
        }
        Ok(())
    }

    // ── UNSTRING ──────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn exec_unstring(
        &mut self,
        from: &Expr,
        delimited_by: &[Expr],
        _all: bool,
        into: &[UnstringTarget],
        _pointer: Option<&Expr>,
        _tallying: Option<&Expr>,
        on_overflow: &[Stmt],
        not_on_overflow: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let src = self.eval_expr(from, span)?.as_display_string();
        let delims: Vec<String> = delimited_by
            .iter()
            .map(|d| {
                self.eval_expr(d, span)
                    .unwrap_or_else(|_| CobolValue::from_str(" ", 1))
                    .as_display_string()
            })
            .collect();

        // Split source by all delimiters in sequence.
        let mut parts: Vec<String> = vec![src];
        for delim in &delims {
            let mut new_parts = Vec::new();
            for part in &parts {
                for sub in part.split(delim.as_str()) {
                    new_parts.push(sub.to_string());
                }
            }
            parts = new_parts;
        }

        for (i, target) in into.iter().enumerate() {
            let name = self.resolve_lvalue(&target.target);
            let val = parts.get(i).map(|s| s.as_str()).unwrap_or("");
            self.env.set_str(&name, val);
            if let Some(count_expr) = &target.count {
                let cname = self.resolve_lvalue(count_expr);
                self.env.set_i64(&cname, val.len() as i64);
            }
        }
        // Overflow: more source fields than receiving fields (unprocessed data).
        if parts.len() > into.len() {
            self.exec_stmts(on_overflow)?;
        } else {
            self.exec_stmts(not_on_overflow)?;
        }
        Ok(())
    }

    // ── INSPECT ───────────────────────────────────────────────────────────────

    /// Resolve a `BEFORE/AFTER INITIAL` region to a `[lo, hi)` byte window of
    /// `s`. `AFTER INITIAL d` starts just past the first `d`; `BEFORE INITIAL d`
    /// ends just before the first `d` (searched from `lo`). Whole field by default.
    fn inspect_window(
        &mut self,
        s: &str,
        region: &InspectRegion,
        span: Span,
    ) -> Result<(usize, usize), RuntimeError> {
        let lo = match &region.after {
            Some(e) => {
                let d = self.eval_expr(e, span)?.as_display_string();
                match (d.is_empty(), s.find(&d)) {
                    (false, Some(p)) => p + d.len(),
                    _ => s.len(),
                }
            }
            None => 0,
        };
        let hi = match &region.before {
            Some(e) => {
                let d = self.eval_expr(e, span)?.as_display_string();
                match (d.is_empty(), s[lo..].find(&d)) {
                    (false, Some(p)) => lo + p,
                    _ => s.len(),
                }
            }
            None => s.len(),
        };
        Ok((lo.min(s.len()), hi.max(lo).min(s.len())))
    }

    fn exec_inspect(
        &mut self,
        target: &Expr,
        spec: &InspectSpec,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let name = self.resolve_lvalue(target);
        let val = self
            .env
            .get(&name)
            .cloned()
            .unwrap_or_else(|| CobolValue::from_str("", 0));
        let mut s = val.as_display_string();

        match spec {
            InspectSpec::Tallying(tallies) => {
                for tally in tallies {
                    let ctr_name = self.resolve_lvalue(&tally.counter);
                    // INSPECT TALLYING accumulates onto the counter's value.
                    let mut count = self.env.get_i64(&ctr_name).unwrap_or(0);
                    for (kind, region) in &tally.for_ {
                        let (lo, hi) = self.inspect_window(&s, region, span)?;
                        let win = &s[lo..hi];
                        count += match kind {
                            TallyFor::Characters => win.len() as i64,
                            TallyFor::All(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                if pat.is_empty() {
                                    0
                                } else {
                                    win.matches(pat.as_str()).count() as i64
                                }
                            }
                            TallyFor::Leading(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                win.chars().take_while(|c| pat.contains(*c)).count() as i64
                            }
                            TallyFor::Trailing(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                win.chars().rev().take_while(|c| pat.contains(*c)).count() as i64
                            }
                        };
                    }
                    self.env.set_i64(&ctr_name, count);
                }
            }
            InspectSpec::Replacing(replaces) => {
                for rep in replaces {
                    let by = self.eval_expr(&rep.by, span)?.as_display_string();
                    let (lo, hi) = self.inspect_window(&s, &rep.region, span)?;
                    let mut win = s[lo..hi].to_string();
                    match &rep.what {
                        ReplaceWhat::All(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            if !pat.is_empty() {
                                win = win.replace(pat.as_str(), &by);
                            }
                        }
                        ReplaceWhat::First(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            if let Some(pos) = win.find(pat.as_str()) {
                                win.replace_range(pos..pos + pat.len(), &by);
                            }
                        }
                        ReplaceWhat::Leading(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            while !pat.is_empty() && win.starts_with(pat.as_str()) {
                                let end = pat.len();
                                let repl_len = by.len().min(end);
                                win.replace_range(0..end, &by[..repl_len]);
                            }
                        }
                        ReplaceWhat::Trailing(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            while !pat.is_empty() && win.ends_with(pat.as_str()) {
                                let start = win.len() - pat.len();
                                let repl_len = by.len().min(pat.len());
                                win.replace_range(start.., &by[..repl_len]);
                            }
                        }
                        ReplaceWhat::Characters => {
                            let fill = by.chars().next().unwrap_or(' ');
                            win = win.chars().map(|_| fill).collect();
                        }
                    }
                    s = format!("{}{}{}", &s[..lo], win, &s[hi..]);
                }
                self.env.set_str(&name, &s);
            }
            InspectSpec::Converting { from, to } => {
                let from_s = self.eval_expr(from, span)?.as_display_string();
                let to_s = self.eval_expr(to, span)?.as_display_string();
                for (fc, tc) in from_s.chars().zip(to_s.chars()) {
                    s = s.replace(fc, &tc.to_string());
                }
                self.env.set_str(&name, &s);
            }
            InspectSpec::TallyingReplacing(tallies, replaces) => {
                self.exec_inspect(target, &InspectSpec::Tallying(tallies.clone()), span)?;
                self.exec_inspect(target, &InspectSpec::Replacing(replaces.clone()), span)?;
            }
        }
        Ok(())
    }

    // ── File I/O (SEQUENTIAL / LINE SEQUENTIAL) ──────────────────────────────
    //
    // Two organisations are supported, modelled on the COBOL-85 sequential file
    // verbs: record SEQUENTIAL (fixed-length records, no terminators) and LINE
    // SEQUENTIAL (newline-terminated text records, trailing spaces not stored).
    // Each operation updates the file's FILE STATUS item (if declared) with the
    // usual codes: 00 ok, 10 end-of-file, 30 permanent error, 35 not found.

    /// Resolve a SELECT … ASSIGN target to a filesystem path. If the assign
    /// value names a data item, that item's current (trimmed) value is used;
    /// otherwise the assign string itself is the path.
    fn resolve_assign_path(&self, assign: &str) -> String {
        let key = assign.trim().to_ascii_uppercase();
        if let Some(v) = self.env.get_string(&key) {
            return v.trim_end().to_string();
        }
        assign.trim().to_string()
    }

    /// Set a file's FILE STATUS data item (if declared) to a 2-character code.
    fn set_file_status(&mut self, file: &str, code: &str) {
        if let Some(field) = self
            .file_specs
            .get(file)
            .and_then(|s| s.status_field.clone())
        {
            self.env.set_str(&field, code);
        }
    }

    /// Invoke the matching `USE AFTER STANDARD ERROR` declarative when a file
    /// operation produced an error status that the statement did not handle with
    /// its own AT END / INVALID KEY phrase.
    ///
    /// `phrase_present` is true when the I/O statement carried the applicable
    /// AT END / INVALID KEY phrase (which takes precedence over the declarative).
    fn fire_declarative(
        &mut self,
        file: &str,
        code: &str,
        phrase_present: bool,
    ) -> Result<(), RuntimeError> {
        // FILE STATUS class 0 (`0x`) is success/informational — no error.
        if code.starts_with('0') || phrase_present {
            return Ok(());
        }
        if self.in_declarative || self.declaratives.is_empty() {
            return Ok(());
        }
        let file_uc = file.to_ascii_uppercase();
        let mode = self.open_modes.get(&file_uc).copied();
        let idx = self.declaratives.iter().position(|h| {
            if h.files.iter().any(|f| f == &file_uc) {
                return true;
            }
            if let Some(m) = mode {
                let um = match m {
                    OpenMode::Input => UseMode::Input,
                    OpenMode::Output => UseMode::Output,
                    OpenMode::InputOutput => UseMode::Io,
                    OpenMode::Extend => UseMode::Extend,
                };
                if h.modes.contains(&um) {
                    return true;
                }
            }
            h.catch_all
        });
        if let Some(i) = idx {
            let stmts = self.declaratives[i].stmts.clone();
            self.in_declarative = true;
            let r = self.exec_stmts(&stmts);
            self.in_declarative = false;
            r?;
        }
        Ok(())
    }

    /// `OPEN`. `_lock` is `WITH LOCK` (exclusive); advisory in the single-run-unit
    /// model — recorded for fidelity but does not change single-process behaviour.
    /// `SHARING` is likewise advisory.
    fn exec_open(
        &mut self,
        mode: OpenMode,
        files: &[String],
        _lock: bool,
        registered_user: Option<&cobolt_ast::expr::Expr>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        use std::fs::OpenOptions;
        use std::io::{BufReader, BufWriter};

        // `OPEN … WITH REGISTERED USER {literal | data-item}` — evaluate once for
        // this OPEN; recorded in the INDEXED observability log.
        let reg_user: Option<String> = match registered_user {
            Some(e) => self
                .eval_expr(e, span)
                .ok()
                .map(|v| v.as_display_string().trim_end().to_string()),
            None => None,
        };

        for raw in files {
            let file = raw.to_ascii_uppercase();
            // Remember the open-mode for mode-qualified USE declaratives.
            self.open_modes.insert(file.clone(), mode);
            let Some(spec) = self.file_specs.get(&file).cloned() else {
                tracing::warn!("OPEN: unknown file '{}'", raw);
                continue;
            };
            let path = self.resolve_assign_path(&spec.assign);
            let org = spec.organization;

            // ── INDEXED: dispatch to the keyed engine ──────────────────────
            if org == FileOrganization::Indexed {
                let mut engine = make_indexed_engine(
                    &spec,
                    &path,
                    self.indexed_engine,
                    self.indexed_log_level,
                    self.indexed_log_format,
                );
                engine.set_registered_user(reg_user.clone());
                let code = engine.open(map_open_mode(mode));
                self.open_files
                    .insert(file.clone(), OpenFile::Indexed(engine));
                self.set_file_status(&file, code);
                self.fire_declarative(&file, code, false)?;
                continue;
            }

            // ── SEQUENTIAL / LINE SEQUENTIAL ───────────────────────────────
            let result: std::io::Result<OpenFile> = match mode {
                OpenMode::Output => std::fs::File::create(&path).map(|f| OpenFile::Writer {
                    w: BufWriter::new(f),
                    org,
                }),
                OpenMode::Extend => OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .map(|f| OpenFile::Writer {
                        w: BufWriter::new(f),
                        org,
                    }),
                OpenMode::Input => std::fs::File::open(&path).map(|f| OpenFile::Reader {
                    r: BufReader::new(f),
                    org,
                }),
                OpenMode::InputOutput => OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path)
                    .map(|f| OpenFile::Reader {
                        r: BufReader::new(f),
                        org,
                    }),
            };

            match result {
                Ok(handle) => {
                    self.open_files.insert(file.clone(), handle);
                    self.set_file_status(&file, "00");
                }
                Err(e) => {
                    tracing::warn!("OPEN '{}' ({}) failed: {}", raw, path, e);
                    let code = if matches!(mode, OpenMode::Input)
                        && e.kind() == std::io::ErrorKind::NotFound
                    {
                        "35"
                    } else {
                        "30"
                    };
                    self.set_file_status(&file, code);
                    self.fire_declarative(&file, code, false)?;
                }
            }
        }
        Ok(())
    }

    fn exec_close(&mut self, files: &[String]) -> Result<(), RuntimeError> {
        use std::io::Write as _;
        for raw in files {
            let file = raw.to_ascii_uppercase();
            if let Some(mut handle) = self.open_files.remove(&file) {
                let code = match &mut handle {
                    OpenFile::Writer { w, .. } => {
                        let _ = w.flush();
                        "00"
                    }
                    OpenFile::Reader { .. } => "00",
                    OpenFile::Indexed(engine) => engine.close(),
                };
                self.set_file_status(&file, code);
                self.fire_declarative(&file, code, false)?;
            } else {
                self.set_file_status(&file, "42"); // CLOSE of a file not open
                self.fire_declarative(&file, "42", false)?;
            }
        }
        Ok(())
    }

    /// Run the `INVALID KEY` / `NOT INVALID KEY` imperative phrase of a keyed
    /// file verb according to its resulting status. Success is "00" (or "02",
    /// duplicate-alternate created) → NOT INVALID KEY; anything else → INVALID KEY.
    fn run_key_outcome(
        &mut self,
        code: &str,
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
    ) -> Result<(), RuntimeError> {
        if code == "00" || code == "02" {
            self.exec_stmts(not_invalid_key)
        } else {
            self.exec_stmts(invalid_key)
        }
    }

    // ── SORT / MERGE / RELEASE / RETURN ─────────────────────────────────────────

    /// `RELEASE record [FROM src]` — materialise the SD record and append it to
    /// the sort work buffer.
    fn exec_release(&mut self, record: &Expr, from: Option<&Expr>) -> Result<(), RuntimeError> {
        if let Some(src) = from {
            self.exec_move(src, std::slice::from_ref(record))?;
        }
        let rec_name = self.expr_to_name(record);
        let Some(file) = self.record_to_file.get(&rec_name).cloned() else {
            tracing::warn!("RELEASE: record '{}' is not part of any SD", rec_name);
            return Ok(());
        };
        let buf = match self.file_specs.get(&file) {
            Some(spec) => spec.layout.materialize(&self.env),
            None => self
                .env
                .get_string(&rec_name)
                .unwrap_or_default()
                .into_bytes(),
        };
        self.sort_buffers.entry(file).or_default().push(buf);
        Ok(())
    }

    /// `RETURN file [INTO id] AT END … [NOT AT END …]` — hand back the next
    /// sorted record, or run the AT END phrase when the run is exhausted.
    fn exec_return(
        &mut self,
        file: &str,
        into: Option<&Expr>,
        at_end: &[Stmt],
        not_at_end: &[Stmt],
    ) -> Result<(), RuntimeError> {
        let fkey = file.to_ascii_uppercase();
        let cur = *self.sort_cursors.get(&fkey).unwrap_or(&0);
        let rec = self
            .sort_buffers
            .get(&fkey)
            .and_then(|v| v.get(cur))
            .cloned();
        match rec {
            Some(b) => {
                self.sort_cursors.insert(fkey.clone(), cur + 1);
                if let Some(spec) = self.file_specs.get(&fkey).cloned() {
                    spec.layout.distribute(&mut self.env, &b);
                }
                if let Some(tgt) = into {
                    let s = String::from_utf8_lossy(&b).into_owned();
                    let tname = self.expr_to_name(tgt);
                    self.env.set_str(&tname, &s);
                }
                self.exec_stmts(not_at_end)
            }
            None => self.exec_stmts(at_end),
        }
    }

    /// Execute `SORT` / `MERGE`: fill the work buffer (INPUT PROCEDURE releases
    /// or USING files), sort by the declared keys, then deliver (OUTPUT
    /// PROCEDURE returns or GIVING files).
    #[allow(clippy::too_many_arguments)]
    fn exec_sort(
        &mut self,
        file: &str,
        keys: &[cobolt_ast::stmt::SortKey],
        using: &[String],
        giving: &[String],
        input_proc: Option<&str>,
        output_proc: Option<&str>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let fkey = file.to_ascii_uppercase();
        self.sort_buffers.insert(fkey.clone(), Vec::new());
        self.sort_cursors.insert(fkey.clone(), 0);

        // ── Phase 1: collect records ──────────────────────────────────────
        if let Some(ip) = input_proc {
            self.exec_perform(&PerformTarget::Section(ip.to_string(), span), span)?;
        } else {
            for uf in using {
                let recs = self.read_all_records(uf);
                self.sort_buffers
                    .entry(fkey.clone())
                    .or_default()
                    .extend(recs);
            }
        }

        // ── Phase 2: sort by keys ─────────────────────────────────────────
        self.sort_records(&fkey, keys, span)?;

        // ── Phase 3: deliver records ──────────────────────────────────────
        if let Some(op) = output_proc {
            self.sort_cursors.insert(fkey.clone(), 0);
            self.exec_perform(&PerformTarget::Section(op.to_string(), span), span)?;
        } else {
            let recs = self.sort_buffers.get(&fkey).cloned().unwrap_or_default();
            for gf in giving {
                self.write_all_records(gf, &recs)?;
            }
        }
        Ok(())
    }

    /// Stable-sort the work buffer of `fkey` by the SORT keys (ascending or
    /// descending per key), comparing the SD record's key fields.
    fn sort_records(
        &mut self,
        fkey: &str,
        keys: &[cobolt_ast::stmt::SortKey],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(spec) = self.file_specs.get(fkey).cloned() else {
            return Ok(());
        };
        let recs = self.sort_buffers.remove(fkey).unwrap_or_default();
        // Precompute each record's (key-value, ascending) vector.
        let mut keyed: Vec<(Vec<(CobolValue, bool)>, Vec<u8>)> = Vec::with_capacity(recs.len());
        for bytes in recs {
            spec.layout.distribute(&mut self.env, &bytes);
            let mut kv = Vec::new();
            for k in keys {
                for f in &k.fields {
                    kv.push((self.eval_expr(f, span)?, k.ascending));
                }
            }
            keyed.push((kv, bytes));
        }
        keyed.sort_by(|a, b| {
            for ((av, asc), (bv, _)) in a.0.iter().zip(b.0.iter()) {
                let ord = cob_ordering(av, bv);
                if ord != std::cmp::Ordering::Equal {
                    return if *asc { ord } else { ord.reverse() };
                }
            }
            std::cmp::Ordering::Equal
        });
        self.sort_buffers.insert(
            fkey.to_string(),
            keyed.into_iter().map(|(_, b)| b).collect(),
        );
        Ok(())
    }

    /// Open `file` for input, read every record (raw bytes), and close it.
    fn read_all_records(&mut self, file: &str) -> Vec<Vec<u8>> {
        use std::io::{BufRead as _, Read as _};
        let fkey = file.to_ascii_uppercase();
        let _ = self.exec_open(
            OpenMode::Input,
            &[file.to_string()],
            false,
            None,
            Span::dummy(),
        );
        let rlen = self
            .file_specs
            .get(&fkey)
            .map(|s| s.layout.len.max(1))
            .unwrap_or(1);
        let mut out = Vec::new();
        loop {
            let rec = match self.open_files.get_mut(&fkey) {
                Some(OpenFile::Reader { r, org }) => match org {
                    FileOrganization::LineSequential => {
                        let mut line = String::new();
                        match r.read_line(&mut line) {
                            Ok(0) => None,
                            Ok(_) => {
                                while line.ends_with('\n') || line.ends_with('\r') {
                                    line.pop();
                                }
                                Some(line.into_bytes())
                            }
                            Err(_) => None,
                        }
                    }
                    _ => {
                        let mut bytes = vec![0u8; rlen];
                        match r.read_exact(&mut bytes) {
                            Ok(()) => Some(bytes),
                            Err(_) => None,
                        }
                    }
                },
                _ => None,
            };
            match rec {
                Some(b) => out.push(b),
                None => break,
            }
        }
        let _ = self.exec_close(&[file.to_string()]);
        out
    }

    /// Open `file` for output, write every record, and close it.
    fn write_all_records(&mut self, file: &str, recs: &[Vec<u8>]) -> Result<(), RuntimeError> {
        use std::io::Write as _;
        let fkey = file.to_ascii_uppercase();
        self.exec_open(
            OpenMode::Output,
            &[file.to_string()],
            false,
            None,
            Span::dummy(),
        )?;
        for b in recs {
            if let Some(OpenFile::Writer { w, org }) = self.open_files.get_mut(&fkey) {
                let _ = match org {
                    FileOrganization::LineSequential => {
                        let s = String::from_utf8_lossy(b);
                        writeln!(w, "{}", s.trim_end())
                    }
                    _ => w.write_all(b),
                };
            }
        }
        self.exec_close(&[file.to_string()])
    }

    fn exec_write(
        &mut self,
        record: &Expr,
        from: Option<&Expr>,
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        use std::io::Write as _;
        // WRITE rec FROM src ⇒ move src into the record buffer first.
        if let Some(src) = from {
            self.exec_move(src, std::slice::from_ref(record))?;
        }
        let rec_name = self.expr_to_name(record);
        let Some(file) = self.record_to_file.get(&rec_name).cloned() else {
            tracing::warn!("WRITE: record '{}' is not part of any FD", rec_name);
            return Ok(());
        };
        // Materialize the record buffer from its subfields (works for group and
        // elementary records alike).
        let buf = match self.file_specs.get(&file) {
            Some(spec) => spec.layout.materialize(&self.env),
            None => self
                .env
                .get_string(&rec_name)
                .unwrap_or_default()
                .into_bytes(),
        };

        let code = match self.open_files.get_mut(&file) {
            // ── INDEXED ────────────────────────────────────────────────────
            Some(OpenFile::Indexed(engine)) => engine.write(&buf),
            // ── SEQUENTIAL / LINE SEQUENTIAL ───────────────────────────────
            Some(OpenFile::Writer { w, org }) => {
                let r = match org {
                    FileOrganization::LineSequential => {
                        let s = String::from_utf8_lossy(&buf);
                        writeln!(w, "{}", s.trim_end())
                    }
                    _ => w.write_all(&buf),
                };
                match r {
                    Ok(()) => "00",
                    Err(e) => {
                        tracing::warn!("WRITE failed: {e}");
                        "30"
                    }
                }
            }
            _ => {
                tracing::warn!("WRITE to '{}' which is not open for output", file);
                "48"
            }
        };
        self.set_file_status(&file, code);
        self.run_key_outcome(code, invalid_key, not_invalid_key)?;
        self.fire_declarative(&file, code, !invalid_key.is_empty())?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_read(
        &mut self,
        file_name: &str,
        into: Option<&Expr>,
        key: Option<&Expr>,
        direction: cobolt_ast::stmt::ReadDirection,
        lock: Option<bool>,
        at_end: &[Stmt],
        not_at_end: &[Stmt],
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        use crate::indexed::{status, ReadDir};
        use cobolt_ast::stmt::ReadDirection;
        use std::io::BufRead as _;

        let file = file_name.to_ascii_uppercase();
        let Some(spec) = self.file_specs.get(&file).cloned() else {
            tracing::warn!("READ: unknown file '{}'", file_name);
            return Ok(());
        };
        let rec_name = spec.record_names.first().cloned();

        // Pre-compute indexed inputs (immutable borrows) before touching the
        // mutable handle: random vs sequential, the key field value, and the
        // key of reference (primary = 0, else the matching alternate index).
        // NEXT/PREVIOUS force sequential; an unqualified READ is random by
        // RECORD KEY under RANDOM or DYNAMIC access, sequential otherwise.
        let sequential_dir = direction != ReadDirection::Default;
        let random = !sequential_dir
            && (key.is_some() || matches!(spec.access, AccessMode::Random | AccessMode::Dynamic));
        let read_dir = if direction == ReadDirection::Previous {
            ReadDir::Previous
        } else {
            ReadDir::Next
        };
        let key_field = key
            .map(|e| self.expr_to_name(e))
            .or_else(|| spec.record_key.clone());
        let key_bytes = key_field
            .as_ref()
            .and_then(|kf| spec.layout.field_value(&self.env, kf));
        let kor = match &key_field {
            Some(kf) if spec.record_key.as_deref() == Some(kf.as_str()) => 0,
            Some(kf) => spec
                .alternate_keys
                .iter()
                .position(|ak| ak.field.eq_ignore_ascii_case(kf))
                .map(|i| i + 1)
                .unwrap_or(0),
            None => 0,
        };

        // Fetch one record + a status code, dispatched by organization.
        let (buf, code): (Option<Vec<u8>>, &str) = match self.open_files.get_mut(&file) {
            Some(OpenFile::Indexed(engine)) => {
                if random {
                    engine.set_key_of_reference(kor);
                    match &key_bytes {
                        Some(kb) => engine.read_key(kb),
                        None => (None, status::NOT_FOUND),
                    }
                } else {
                    engine.read_seq(read_dir)
                }
            }
            Some(OpenFile::Reader { r, org }) => match org {
                // LINE SEQUENTIAL: newline-delimited text records.
                FileOrganization::LineSequential => {
                    let mut line = String::new();
                    match r.read_line(&mut line) {
                        Ok(0) => (None, status::EOF),
                        Ok(_) => {
                            while line.ends_with('\n') || line.ends_with('\r') {
                                line.pop();
                            }
                            (Some(line.into_bytes()), status::OK)
                        }
                        Err(e) => {
                            tracing::warn!("READ failed: {e}");
                            (None, "30")
                        }
                    }
                }
                // Record SEQUENTIAL: fixed-length records, no terminator — read
                // exactly one record's worth of bytes per READ.
                _ => {
                    use std::io::Read as _;
                    let rlen = spec.layout.len.max(1);
                    let mut bytes = vec![0u8; rlen];
                    match r.read_exact(&mut bytes) {
                        Ok(()) => (Some(bytes), status::OK),
                        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            (None, status::EOF)
                        }
                        Err(e) => {
                            tracing::warn!("READ failed: {e}");
                            (None, "30")
                        }
                    }
                }
            },
            _ => (None, status::NOT_OPEN_INPUT),
        };

        // `READ … WITH NO LOCK` releases the lock the engine takes under I-O.
        if lock == Some(false) {
            if let Some(OpenFile::Indexed(engine)) = self.open_files.get_mut(&file) {
                engine.unlock();
            }
        }

        self.set_file_status(&file, code);
        // Pick the success / failure handler. Random reads branch on INVALID KEY,
        // sequential reads on AT END; fall back to whichever phrase was supplied.
        fn pick<'a>(primary: &'a [Stmt], fallback: &'a [Stmt]) -> &'a [Stmt] {
            if !primary.is_empty() {
                primary
            } else {
                fallback
            }
        }
        let (ok_branch, fail_branch): (&[Stmt], &[Stmt]) = if random {
            (pick(not_invalid_key, not_at_end), pick(invalid_key, at_end))
        } else {
            (pick(not_at_end, not_invalid_key), pick(at_end, invalid_key))
        };
        if code == status::OK {
            if let Some(b) = &buf {
                spec.layout.distribute(&mut self.env, b);
                if let Some(tgt) = into {
                    // READ … INTO: also deliver the record image to the target.
                    let s = String::from_utf8_lossy(b).into_owned();
                    let tname = self.expr_to_name(tgt);
                    self.env.set_str(&tname, &s);
                }
            }
            let _ = rec_name;
            self.exec_stmts(ok_branch)?;
        } else {
            self.exec_stmts(fail_branch)?;
        }
        // On an unhandled error status, run the file's USE declarative. The
        // statement "handled" the condition only if it supplied the matching
        // AT END / INVALID KEY phrase (a non-empty failure branch).
        self.fire_declarative(&file, code, !fail_branch.is_empty())?;
        Ok(())
    }

    // ── REWRITE / DELETE / START (dispatched by file organization) ──────────────

    fn exec_rewrite(
        &mut self,
        record: &Expr,
        from: Option<&Expr>,
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(src) = from {
            self.exec_move(src, std::slice::from_ref(record))?;
        }
        let rec_name = self.expr_to_name(record);
        let Some(file) = self.record_to_file.get(&rec_name).cloned() else {
            tracing::warn!("REWRITE: record '{}' is not part of any FD", rec_name);
            return Ok(());
        };
        let Some(spec) = self.file_specs.get(&file).cloned() else {
            return Ok(());
        };
        let buf = spec.layout.materialize(&self.env);
        let random = spec.access != AccessMode::Sequential; // RANDOM or DYNAMIC address by key
        let code = match self.open_files.get_mut(&file) {
            Some(OpenFile::Indexed(engine)) => {
                engine.rewrite(&buf, if random { Some(buf.as_slice()) } else { None })
            }
            Some(_) => {
                tracing::warn!(
                    "REWRITE on a non-indexed file '{}' is not yet supported",
                    file
                );
                "30"
            }
            None => crate::indexed::status::NOT_OPEN_IO,
        };
        self.set_file_status(&file, code);
        self.run_key_outcome(code, invalid_key, not_invalid_key)?;
        self.fire_declarative(&file, code, !invalid_key.is_empty())?;
        Ok(())
    }

    fn exec_delete(
        &mut self,
        file_name: &str,
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        use crate::indexed::status;
        let file = file_name.to_ascii_uppercase();
        let Some(spec) = self.file_specs.get(&file).cloned() else {
            return Ok(());
        };
        let random = spec.access != AccessMode::Sequential; // RANDOM or DYNAMIC address by key
                                                            // RANDOM DELETE addresses the record by the current RECORD KEY value;
                                                            // sequential/dynamic DELETE removes the current (last read) record.
        let key_bytes = spec
            .record_key
            .as_deref()
            .and_then(|k| spec.layout.field_value(&self.env, k));
        let code = match self.open_files.get_mut(&file) {
            Some(OpenFile::Indexed(engine)) => {
                engine.delete(if random { key_bytes.as_deref() } else { None })
            }
            Some(_) => {
                tracing::warn!("DELETE on a non-indexed file '{}' is not valid", file);
                "37"
            }
            None => status::NOT_OPEN_IO,
        };
        self.set_file_status(&file, code);
        self.run_key_outcome(code, invalid_key, not_invalid_key)?;
        self.fire_declarative(&file, code, !invalid_key.is_empty())?;
        Ok(())
    }

    fn exec_start(
        &mut self,
        file_name: &str,
        key: Option<&(cobolt_ast::expr::CmpOp, Expr)>,
        invalid_key: &[Stmt],
        not_invalid_key: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        use crate::indexed::status;
        let file = file_name.to_ascii_uppercase();
        let Some(spec) = self.file_specs.get(&file).cloned() else {
            return Ok(());
        };
        let (op, key_field) = match key {
            Some((op, e)) => (*op, self.expr_to_name(e)),
            None => (
                cobolt_ast::expr::CmpOp::Eq,
                spec.record_key.clone().unwrap_or_default(),
            ),
        };
        let key_bytes = spec.layout.field_value(&self.env, &key_field);
        let kor = if spec.record_key.as_deref() == Some(key_field.as_str()) {
            0
        } else {
            spec.alternate_keys
                .iter()
                .position(|ak| ak.field.eq_ignore_ascii_case(&key_field))
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        let code = match self.open_files.get_mut(&file) {
            Some(OpenFile::Indexed(engine)) => {
                engine.set_key_of_reference(kor);
                match &key_bytes {
                    Some(kb) => engine.start(map_start_op(op), kb),
                    None => status::NOT_FOUND,
                }
            }
            Some(_) => {
                tracing::warn!("START on a non-indexed file '{}' is not valid", file);
                "30"
            }
            None => status::NOT_OPEN_INPUT,
        };
        self.set_file_status(&file, code);
        // START's "record not found" status (23) is the invalid-key condition.
        self.run_key_outcome(code, invalid_key, not_invalid_key)?;
        Ok(())
    }

    // ── CALL ──────────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    /// `CANCEL program …` — re-initialise each named (nested) program's
    /// WORKING-STORAGE to its declared initial values, so the next `CALL` starts
    /// fresh (as the standard requires after CANCEL).
    fn exec_cancel(&mut self, programs: &[Expr]) -> Result<(), RuntimeError> {
        for prog in programs {
            let name = self
                .eval_expr(prog, prog.span())?
                .as_display_string()
                .trim()
                .to_ascii_uppercase();
            // Static lifecycle (009 R10): CANCEL discards the program's persisted
            // local state so its next CALL re-initialises from the DATA DIVISION.
            // (Between calls the locals live only in `program_locals`, not `env`,
            // so dropping the entry is the reset.)
            self.program_locals.remove(&name);
            if !self.nested_registry.contains_key(&name) {
                // Legacy/flat program names: nothing persisted — no-op.
            }
        }
        Ok(())
    }

    fn exec_call(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        _returning: Option<&Expr>,
        on_exception: &[Stmt],
        not_on_exception: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let prog_name = self
            .eval_expr(program, span)?
            .as_display_string()
            .trim()
            .to_ascii_uppercase();

        // `NOT ON EXCEPTION` runs only when the call resolved (i.e. unless we
        // fall into the unresolved-program branch below).
        let mut resolved = true;
        match prog_name.as_str() {
            // ── Built-in runtime calls (COBOL-* prefix) ────────────
            // COBOL-INIT-FORM USING form-name  — initialise the form; no-op in CLI mode
            "COBOL-INIT-FORM" | "COBOLT-INIT-FORM" => {
                // Nothing to do in non-GUI (CLI) mode.
            }

            // ── Generated data-binding helper calls ─────────────────────────
            "COBOL-BINDING-LOAD" if using.len() >= 2 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let status_name = self.expr_to_name(call_arg_expr(&using[1]));
                let status = self.binding_load(binding_id.trim());
                self.env.set_str(&status_name, &status);
            }
            "COBOL-BINDING-SET-READ-ONLY" if using.len() >= 2 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let flag = self.eval_call_arg(&using[1], span)?.as_display_string();
                self.binding_set_read_only(binding_id.trim(), flag.trim() != "0");
            }
            "COBOL-BINDING-POPULATE" if using.len() >= 2 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                tracing::debug!(target: "databinding", "RUN-FORM CALL COBOL-BINDING-POPULATE \"{}\"", binding_id);
                let status_name = self.expr_to_name(call_arg_expr(&using[1]));
                let status = self.binding_populate(binding_id.trim());
                self.env.set_str(&status_name, &status);
            }
            "COBOL-BINDING-MARK-CLEAN" if using.len() >= 2 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let dirty_name = self.expr_to_name(call_arg_expr(&using[1]));
                self.binding_mark_clean(binding_id.trim());
                self.env.set(&dirty_name, CobolValue::from_i64(0));
            }
            "COBOL-BINDING-SET-PENDING" if using.len() >= 4 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let row_key = self.eval_call_arg(&using[1], span)?.as_display_string();
                let value = self.eval_call_arg(&using[2], span)?.as_display_string();
                let dirty_name = self.expr_to_name(call_arg_expr(&using[3]));
                self.binding_set_pending(binding_id.trim(), row_key.trim(), value.trim_end());
                self.env.set(&dirty_name, CobolValue::from_i64(1));
            }
            "COBOL-BINDING-UPDATE" if using.len() >= 3 => {
                let binding_id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let row_key = self.eval_call_arg(&using[1], span)?.as_display_string();
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let status = self.binding_update(binding_id.trim(), row_key.trim());
                self.env.set_str(&status_name, &status);
            }

            // COBOL-WAIT-EVENT USING event-id control-id
            // GUI mode: block until the UI sends a FormEvent, then populate the two fields.
            // CLI mode: immediately set COBOL-QUIT = 1 so the event loop exits cleanly.
            "COBOL-WAIT-EVENT" | "COBOLT-WAIT-EVENT" => {
                if self.event_rx.is_some() {
                    // Wait for the next thing to present: a real UI event, an
                    // async completion (spec 032), or channel disconnect. While
                    // async ops are pending this polls so completions/timeouts
                    // surface even with no UI activity in flight.
                    match self.next_wait_outcome() {
                        WaitOutcome::Ui(ev) => {
                            // One event left the queue — let the host coalesce
                            // timer ticks against the now-shallower backlog.
                            if let Some(c) = &self.event_pending {
                                let prev = c.load(std::sync::atomic::Ordering::Relaxed);
                                if prev > 0 {
                                    c.store(prev - 1, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                            // Fold any UI-driven value changes (the slider drag /
                            // text edit that produced this event, etc.) into the
                            // object registry so the handler reads the live value.
                            self.drain_input();
                            // For array member events, make the 1-based card index
                            // available to the handler (generated as CONTROL-ARRAY-INDEX
                            // in LINKAGE for array-member event handlers).
                            self.env
                                .set_str("CONTROL-ARRAY-INDEX", &ev.instance_index.to_string());
                            // Populate COBOL-EVENT-ID and COBOL-CONTROL-ID (args 0 and 1).
                            if using.len() >= 1 {
                                let n = self.expr_to_name(call_arg_expr(&using[0]));
                                self.env.set_str(&n, &ev.event_id);
                            }
                            if using.len() >= 2 {
                                let n = self.expr_to_name(call_arg_expr(&using[1]));
                                self.env.set_str(&n, &ev.ctrl_id);
                            }
                            // Sentinel: UI closed the form → exit event loop.
                            if ev.ctrl_id == "__QUIT__" {
                                self.env.set_str("COBOL-QUIT", "1");
                            }
                        }
                        WaitOutcome::AsyncDispatch(ctrl_id, event_id) => {
                            // An async operation finished — present it to COBOL
                            // exactly like a UI event so the existing EVALUATE
                            // dispatch runs the bound onComplete/onError/…handler.
                            self.drain_input();
                            self.env.set_str("CONTROL-ARRAY-INDEX", "0");
                            if using.len() >= 1 {
                                let n = self.expr_to_name(call_arg_expr(&using[0]));
                                self.env.set_str(&n, &event_id);
                            }
                            if using.len() >= 2 {
                                let n = self.expr_to_name(call_arg_expr(&using[1]));
                                self.env.set_str(&n, &ctrl_id);
                            }
                        }
                        WaitOutcome::Disconnected => {
                            // Channel disconnected (UI closed) → stop the loop.
                            self.env.set_str("COBOL-QUIT", "1");
                        }
                    }
                } else {
                    // CLI mode — no UI attached, terminate the event loop immediately.
                    for arg in using.iter().take(2) {
                        let e = call_arg_expr(arg);
                        let n = self.expr_to_name(e);
                        self.env.set_str(&n, "");
                    }
                    self.env.set_str("COBOL-QUIT", "1");
                }
            }

            // COBOL-SET-PROPERTY obj prop value
            "COBOL-SET-PROPERTY" | "COBOLT-SET-PROPERTY" if using.len() >= 3 => {
                let obj = self.eval_call_arg(&using[0], span)?.as_display_string();
                let prop = self.eval_call_arg(&using[1], span)?.as_display_string();
                let val = self.eval_call_arg(&using[2], span)?.as_display_string();
                let obj_t = obj.trim().to_owned();
                let prop_t = prop.trim().to_owned();
                let val_t = val.trim().to_owned();
                self.objects.set_property(&obj_t, &prop_t, val_t.clone());
                // GUI mode: notify the UI thread so the form window updates.
                if let Some(tx) = &self.state_tx {
                    let _ = tx.send(StateUpdate::new(obj_t.clone(), prop_t.clone(), val_t));
                }
            }

            // COBOL-GET-PROPERTY obj prop dest
            "COBOL-GET-PROPERTY" | "COBOLT-GET-PROPERTY" if using.len() >= 3 => {
                let obj = self.eval_call_arg(&using[0], span)?.as_display_string();
                let prop = self.eval_call_arg(&using[1], span)?.as_display_string();
                if let Some(pv) = self.objects.get_property(obj.trim(), prop.trim()) {
                    let val_s = pv.to_string();
                    let n = self.expr_to_name(call_arg_expr(&using[2]));
                    self.env.set_str(&n, &val_s);
                }
            }

            // ── Text file output ──────────────────────────────────────────────
            //
            // COBOL-APPEND-FILE USING path text [status]
            //   Append `text` followed by a newline to the file at `path`
            //   (creating it if it does not exist). Optional `status` receives
            //   "" on success or an error message on failure.
            //
            // COBOL-WRITE-FILE  USING path text [status]
            //   Same, but truncates/overwrites the file first (use to (re)write a
            //   header line).
            "COBOL-APPEND-FILE" | "COBOLT-APPEND-FILE" | "COBOL-WRITE-FILE"
            | "COBOLT-WRITE-FILE"
                if using.len() >= 2 =>
            {
                use std::io::Write as _;
                let append = prog_name.contains("APPEND");
                let path = self.eval_call_arg(&using[0], span)?.as_display_string();
                let text = self.eval_call_arg(&using[1], span)?.as_display_string();
                // COBOL fixed-length fields are space-padded; trim the trailing
                // padding so files don't accumulate runs of spaces.
                let path = path.trim().to_owned();
                let text = text.trim_end().to_owned();

                let result = std::fs::OpenOptions::new()
                    .create(true)
                    .append(append)
                    .write(true)
                    .truncate(!append)
                    .open(&path)
                    .and_then(|mut f| writeln!(f, "{text}"));

                if using.len() >= 3 {
                    let n = self.expr_to_name(call_arg_expr(&using[2]));
                    match &result {
                        Ok(()) => self.env.set_str(&n, ""),
                        Err(e) => self.env.set_str(&n, &e.to_string()),
                    }
                }
                if let Err(e) = result {
                    tracing::warn!("{prog_name} failed for '{path}': {e}");
                }
            }

            // ── Chart runtime calls ───────────────────────────────────────────
            // Push live data to the GUI chart renderer via the control's
            // `__ChartData` property (one `label\tvalue` per line). Under the CLI
            // runner there is no `state_tx`, so these safely update the in-memory
            // store only (no rendering — charts are a GUI surface).
            //
            // COBOL-CHART-SET-TABLE chart-id table count  (bulk replace)
            "COBOL-CHART-SET-TABLE" if using.len() >= 3 => {
                let id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let raw = self.eval_call_arg(&using[1], span)?.as_display_string();
                let count = self.eval_call_arg(&using[2], span)?.as_f64() as usize;
                let id = id.trim().to_ascii_uppercase();
                self.chart_data
                    .insert(id.clone(), parse_chart_table(&raw, count));
                self.push_chart_data(&id);
            }
            // COBOL-CHART-ADD-POINT chart-id label value  (append one point)
            "COBOL-CHART-ADD-POINT" if using.len() >= 3 => {
                let id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let label = self.eval_call_arg(&using[1], span)?.as_display_string();
                let value = self.eval_call_arg(&using[2], span)?.as_f64();
                let id = id.trim().to_ascii_uppercase();
                self.chart_data
                    .entry(id.clone())
                    .or_default()
                    .push((label.trim().to_owned(), value));
                self.push_chart_data(&id);
            }
            // COBOL-CHART-CLEAR chart-id
            "COBOL-CHART-CLEAR" if !using.is_empty() => {
                let id = self.eval_call_arg(&using[0], span)?.as_display_string();
                let id = id.trim().to_ascii_uppercase();
                self.chart_data.remove(&id);
                self.push_chart_data(&id);
            }
            // COBOL-CHART-REFRESH chart-id  (re-send current data → repaint)
            "COBOL-CHART-REFRESH" if !using.is_empty() => {
                let id = self.eval_call_arg(&using[0], span)?.as_display_string();
                self.push_chart_data(&id.trim().to_ascii_uppercase());
            }
            // ── Database Runtime Engine (Phase 8) — SQL built-ins ─────────────
            //
            // The backend (SQLite / PostgreSQL / MySQL) is chosen from the
            // connection string's scheme; the CALL surface below is identical
            // for every engine. See `docs/database-runtime.md`.
            //
            // COBOL-OPEN-DB   USING conn-string-var, handle-var, status-var
            //   Opens a database connection (SQLite file/`:memory:`,
            //   `postgres://…`, or `mysql://…`). Stores the integer handle in
            //   handle-var (PIC 9(9)) and clears status-var on success, or
            //   writes an error message into status-var on failure.
            "COBOL-OPEN-DB" if using.len() >= 3 => {
                let conn_str = self.eval_call_arg(&using[0], span)?.as_display_string();
                let conn_str = conn_str.trim().to_owned();
                let handle_name = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                match self.db.open(&conn_str) {
                    Ok(h) => {
                        self.env.set(&handle_name, CobolValue::from_i64(h as i64));
                        self.env.set_str(&status_name, "");
                    }
                    Err(e) => {
                        self.env.set(&handle_name, CobolValue::from_i64(0));
                        self.env.set_str(&status_name, &e);
                        tracing::warn!("COBOL-OPEN-DB failed: {e}");
                    }
                }
            }

            // COBOL-EXEC-SQL  USING handle-var, query-var, row-count-var, status-var
            //   Execute the SQL in query-var on the connection identified by
            //   handle-var.  Stores row / affected count in row-count-var.
            "COBOL-EXEC-SQL" if using.len() >= 4 => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let query = self.eval_call_arg(&using[1], span)?.as_display_string();
                let query = query.trim().to_owned();
                let count_name = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                match self.db.exec(handle, &query) {
                    Ok(n) => {
                        self.env.set(&count_name, CobolValue::from_i64(n as i64));
                        self.env.set_str(&status_name, "");
                    }
                    Err(e) => {
                        self.env.set(&count_name, CobolValue::from_i64(0));
                        self.env.set_str(&status_name, &e);
                        tracing::warn!("COBOL-EXEC-SQL failed: {e}");
                    }
                }
            }

            // COBOL-FETCH-ROW USING handle-var, col-index-var, dest-var, status-var
            //   Reads column col-index (1-based) of the current row into dest-var.
            //   status-var is cleared on success or contains an error.
            "COBOL-FETCH-ROW" if using.len() >= 4 => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let col_idx = self.eval_call_arg(&using[1], span)?.as_i64().unwrap_or(1) as usize;
                let dest_name = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                if handle == 0 || self.db.is_exhausted(handle) {
                    self.env.set_str(&dest_name, "");
                    self.env.set_str(&status_name, "No current row");
                } else {
                    let val = self.db.fetch_col(handle, col_idx);
                    self.env.set_str(&dest_name, &val);
                    self.env.set_str(&status_name, "");
                }
            }

            // COBOL-NEXT-ROW  USING handle-var, more-flag-var
            //   Advances the cursor.  Sets more-flag-var to 'Y' if another
            //   row exists, or 'N' when the result set is exhausted.
            "COBOL-NEXT-ROW" if using.len() >= 2 => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let flag_name = self.expr_to_name(call_arg_expr(&using[1]));
                let has_more = self.db.next_row(handle);
                self.env
                    .set_str(&flag_name, if has_more { "Y" } else { "N" });
            }

            // COBOL-ROW-COUNT USING handle-var, count-var
            //   Stores the total number of rows in the last result set.
            "COBOL-ROW-COUNT" if using.len() >= 2 => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let count_name = self.expr_to_name(call_arg_expr(&using[1]));
                let n = self.db.row_count(handle);
                self.env.set(&count_name, CobolValue::from_i64(n as i64));
            }

            // COBOL-CLOSE-DB  USING handle-var
            //   Closes the connection identified by handle-var and frees
            //   resources.  Silently ignores unknown handles.
            "COBOL-CLOSE-DB" if !using.is_empty() => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                self.db.close(handle);
            }

            // ── Phase 10: HTTP REST client built-in CALLs ─────────────────────
            //
            // COBOL-HTTP-GET   USING url-var, response-var, status-var
            //   Performs an HTTP GET.  Writes the response body into response-var
            //   and the numeric status code (200, 404, …) into status-var.
            //   On network error status-var is set to 0.
            "COBOL-HTTP-GET" if using.len() >= 3 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (body, status) = self.http.get(url.trim());
                self.env.set_str(&resp_name, &body);
                self.env
                    .set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-POST  USING url-var, body-var, response-var, status-var
            //   Performs an HTTP POST with body-var as the request body.
            //   Content-Type defaults to application/json.
            "COBOL-HTTP-POST" if using.len() >= 4 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.post(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env
                    .set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-PUT   USING url-var, body-var, response-var, status-var
            "COBOL-HTTP-PUT" if using.len() >= 4 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.put(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env
                    .set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-DELETE  USING url-var, response-var, status-var
            "COBOL-HTTP-DELETE" if using.len() >= 3 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (resp, status) = self.http.delete(url.trim());
                self.env.set_str(&resp_name, &resp);
                self.env
                    .set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-SET-HEADER  USING name-var, value-var
            //   Adds / overwrites a persistent request header sent on every
            //   subsequent COBOL-HTTP-GET / POST / PUT / DELETE call.
            "COBOL-HTTP-SET-HEADER" if using.len() >= 2 => {
                let name = self.eval_call_arg(&using[0], span)?.as_display_string();
                let value = self.eval_call_arg(&using[1], span)?.as_display_string();
                self.http.set_header(name.trim(), value.trim());
            }

            // COBOL-HTTP-CLEAR-HEADERS  (no arguments)
            //   Removes all persistent request headers.
            "COBOL-HTTP-CLEAR-HEADERS" => {
                self.http.clear_headers();
            }

            // ── COBOL-85 nested program CALL ──────────────────────────────────
            _ if self.nested_registry.contains_key(&prog_name) => {
                // Clone the para_map, para_order, local_items, and USING
                // parameter names out of the registry before any mutable borrow.
                let (para_map, para_order, local_items, local_symbols, params) = {
                    let np = &self.nested_registry[&prog_name];
                    (
                        np.para_map.clone(),
                        np.para_order.clone(),
                        np.local_items.clone(),
                        np.local_symbols.clone(),
                        np.using.clone(),
                    )
                };

                // Pair each LINKAGE parameter with the caller's argument:
                // (param-key, arg-key, by_reference).
                let bindings: Vec<(String, String, bool)> = params
                    .iter()
                    .zip(using.iter())
                    .map(|(p, a)| {
                        let pk = p.to_ascii_uppercase();
                        let ak = self.expr_to_name(call_arg_expr(a)).to_ascii_uppercase();
                        let by_ref = matches!(a, CallArg::ByReference(_));
                        (pk, ak, by_ref)
                    })
                    .collect();

                // Push the nested program's local WS + LINKAGE items into the
                // shared env. GLOBAL items from the outer program are already
                // there and are NOT overwritten.
                //
                // Static lifecycle (009 R10): the program is **static by default**
                // — its locals are initialised once (from `local_items`, its DATA
                // DIVISION) and then **preserved** across calls. We push the
                // *persisted* snapshot (or the fresh template on the first call),
                // and `CANCEL` drops the snapshot to re-initialise next time.
                let snapshot = self
                    .program_locals
                    .get(&prog_name)
                    .cloned()
                    .unwrap_or_else(|| local_items.clone());
                let inserted_keys = self.env.push_local_scope(&snapshot, &local_symbols);

                // Copy-in: bind each parameter to the caller argument's value
                // (after the local scope so it overrides the persisted slot).
                for (pk, ak, _) in &bindings {
                    if let Some(v) = self.env.get(ak).cloned() {
                        self.env.set(pk, v);
                    }
                }

                // Run the nested program's paragraphs in declaration order,
                // with ITS procedures installed as the ones `PERFORM` and
                // `GO TO` resolve against. Procedure names are strictly
                // program-local in COBOL-85 — there is no GLOBAL for them — so
                // while this program runs, the containing program's paragraphs
                // are not reachable and its own always are.
                let saved_map = std::mem::replace(&mut self.para_map, para_map.clone());
                let saved_order = std::mem::replace(&mut self.para_order, para_order.clone());
                let result = self.run_para_sequence(&para_map, &para_order);
                self.para_map = saved_map;
                self.para_order = saved_order;

                // Copy-out: BY REFERENCE arguments receive the parameter's final
                // value (BY CONTENT / BY VALUE are not written back).
                for (pk, ak, by_ref) in &bindings {
                    if *by_ref {
                        if let Some(v) = self.env.get(pk).cloned() {
                            self.env.set(ak, v);
                        }
                    }
                }

                // Save the program's local values back into its persistent store
                // (before popping) so the next CALL resumes with them (static).
                let mut saved = Vec::with_capacity(snapshot.len());
                for (k, prev) in &snapshot {
                    let cur = self.env.get(k).cloned().unwrap_or_else(|| prev.clone());
                    saved.push((k.clone(), cur));
                }
                self.program_locals.insert(prog_name.clone(), saved);

                // Remove the nested program's local items from the shared env
                // regardless of outcome (their state lives in `program_locals`).
                self.env.pop_local_scope(&inserted_keys);

                match result {
                    Ok(()) | Err(RuntimeError::GoBack) => {} // GOBACK = normal return
                    Err(e) => return Err(e),
                }
            }

            // ── Internal paragraph CALL (flat / legacy programs) ──────────────
            _ => {
                if self.para_map.contains_key(&prog_name) {
                    let stmts = self.para_map[&prog_name].clone();
                    match self.exec_stmts(&stmts) {
                        Err(RuntimeError::GoBack) => {} // normal sub-program return
                        other => other?,
                    }
                } else {
                    // Unresolved CALL → run the ON EXCEPTION / ON OVERFLOW body.
                    resolved = false;
                    tracing::warn!("CALL to unknown program '{}'", prog_name);
                    if !on_exception.is_empty() {
                        self.exec_stmts(on_exception)?;
                    }
                }
            }
        }
        // A successful CALL runs its NOT ON EXCEPTION / NOT ON OVERFLOW body.
        if resolved && !not_on_exception.is_empty() {
            self.exec_stmts(not_on_exception)?;
        }
        Ok(())
    }

    fn binding_state_mut(&mut self, binding_id: &str) -> &mut BindingRuntimeState {
        self.binding_states
            .entry(binding_id.to_ascii_uppercase())
            .or_default()
    }

    fn binding_load(&mut self, binding_id: &str) -> String {
        eprintln!("[RUN-FORM-DATABIND] LOAD {}", binding_id);
        tracing::debug!(target: "databinding", "RUN-FORM BINDING-LOAD id={}", binding_id);
        let state = self.binding_state_mut(binding_id);
        state.loaded = true;
        state.last_status.clear();
        String::new()
    }

    fn binding_set_read_only(&mut self, binding_id: &str, read_only: bool) {
        let state = self.binding_state_mut(binding_id);
        state.read_only = read_only;
    }

    fn binding_populate(&mut self, binding_id: &str) -> String {
        tracing::debug!(target: "databinding", "RUN-FORM POPULATE {}", binding_id);
        tracing::debug!(target: "databinding", "RUN-FORM BINDING-POPULATE id={}", binding_id);
        let state = self.binding_state_mut(binding_id);
        if !state.loaded {
            state.last_status = "NOT-LOADED".into();
            return state.last_status.clone();
        }
        state.populated = true;
        state.last_status.clear();
        String::new()
    }

    fn binding_mark_clean(&mut self, binding_id: &str) {
        let state = self.binding_state_mut(binding_id);
        state.dirty = false;
        state.last_status.clear();
    }

    fn binding_set_pending(&mut self, binding_id: &str, row_key: &str, value: &str) {
        let state = self.binding_state_mut(binding_id);
        state.row_key = row_key.to_owned();
        state.pending_value = value.to_owned();
        state.dirty = true;
        state.last_status.clear();
    }

    fn binding_update(&mut self, binding_id: &str, row_key: &str) -> String {
        let state = self.binding_state_mut(binding_id);
        if state.read_only {
            state.last_status = "READ-ONLY".into();
            return state.last_status.clone();
        }
        if row_key.trim().is_empty() && state.row_key.trim().is_empty() {
            state.last_status = "MISSING-ROW-KEY".into();
            return state.last_status.clone();
        }
        if !row_key.trim().is_empty() {
            state.row_key = row_key.to_owned();
        }
        state.dirty = false;
        state.pending_value.clear();
        state.last_status.clear();
        String::new()
    }

    fn refresh_datagrid_binding(&mut self, control_id: &str) -> usize {
        // Run-form only (for comparing datagrid-1 success vs groupbox failure with same data).
        tracing::debug!(target: "databinding", "RUN-FORM refresh_datagrid_binding {}", control_id);
        if !self
            .obj_get(control_id, "_BindingKind")
            .eq_ignore_ascii_case("CobolTable")
        {
            return 0;
        }
        let fields = self
            .obj_get(control_id, "_BindingFields")
            .split(|ch| matches!(ch, '\n' | '\r' | ',' | ';' | '\t'))
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if fields.is_empty() {
            return 0;
        }
        let row_count = fields
            .iter()
            .filter_map(|field| self.env.symbol(field))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);
        if row_count == 0 {
            self.obj_set(control_id, "Rows", String::new());
            return 0;
        }

        let mut rows = Vec::new();
        for row_index in 1..=row_count {
            let cells = fields
                .iter()
                .map(|field| {
                    let key = crate::environment::subscript_key(field, &[row_index as i64]);
                    self.env
                        .get(&key)
                        .map(|value| value.as_display_string().trim_end().to_owned())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            if cells.iter().any(|cell| !cell.trim().is_empty()) {
                rows.push(cells.join("\t"));
            }
        }
        let row_count = rows.len();
        let rows_joined = rows.join("\n");
        tracing::debug!(target: "databinding", "RUN-FORM DataGrid {} set Rows ({} rows)", control_id, row_count);
        self.obj_set(control_id, "Rows", rows_joined);
        row_count
    }

    fn refresh_binding(&mut self, control_id: &str) -> usize {
        if !self
            .obj_get(control_id, "_BindingKind")
            .eq_ignore_ascii_case("CobolTable")
        {
            return 0;
        }
        // Spec 039 R21: a standalone Knob/Gauge/Switch, seeded with a single
        // field + target property instead of `_BindingArray`/table Rows.
        let scalar_field = self.obj_get(control_id, "_BindingScalarField");
        if !scalar_field.trim().is_empty() {
            return self.refresh_scalar_binding(control_id, scalar_field.trim());
        }
        // Spec 039 T13/R22: a Maps control's Markers collection, seeded with
        // one source field per marker attribute instead of `_BindingArray`/
        // table Rows or a single scalar field.
        if !self.obj_get(control_id, "_BindingMarkerFields").trim().is_empty() {
            return self.refresh_marker_binding(control_id);
        }
        // Prefer explicit array flag (seeded for ControlArray) or IsRepeatingGroup
        let is_array = self.obj_get(control_id, "_BindingArray") == "1"
            || self.obj_get(control_id, "IsRepeatingGroup") == "1";
        if is_array {
            return self.refresh_control_array_binding(control_id);
        }
        // default to datagrid logic
        self.refresh_datagrid_binding(control_id)
    }

    /// Read the WS table fields seeded in `_BindingMarkerFields`
    /// (`id\tlat\tlng\tlabel\tinfo`, any entry may be empty except lat/lng)
    /// and rebuild the Maps control's `Markers` property from them — one
    /// marker per populated row, same row-count-driven shape as
    /// `refresh_datagrid_binding`. A row with an unparseable/empty lat or
    /// lng is skipped (mirrors `cobolt_forms::parse_map_markers`'s "one bad
    /// row shouldn't blank the rest of the map" tolerance); an empty id
    /// falls back to the 1-based row number.
    fn refresh_marker_binding(&mut self, control_id: &str) -> usize {
        let spec = self.obj_get(control_id, "_BindingMarkerFields");
        let mut parts = spec.split('\t');
        let id_field = parts.next().unwrap_or("").trim().to_owned();
        let lat_field = parts.next().unwrap_or("").trim().to_owned();
        let lng_field = parts.next().unwrap_or("").trim().to_owned();
        let label_field = parts.next().unwrap_or("").trim().to_owned();
        let info_field = parts.next().unwrap_or("").trim().to_owned();
        if lat_field.is_empty() || lng_field.is_empty() {
            return 0;
        }

        let row_count = [&lat_field, &lng_field]
            .into_iter()
            .filter_map(|field| self.env.symbol(field))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);
        if row_count == 0 {
            self.obj_set(control_id, "Markers", String::new());
            return 0;
        }

        let read = |env: &crate::environment::CobolEnvironment, field: &str, row: usize| -> String {
            if field.is_empty() {
                return String::new();
            }
            let key = crate::environment::subscript_key(field, &[row as i64]);
            env.get(&key)
                .map(|value| value.as_display_string().trim().to_owned())
                .unwrap_or_default()
        };

        let mut lines = Vec::new();
        for row in 1..=row_count {
            let lat = read(&self.env, &lat_field, row);
            let lng = read(&self.env, &lng_field, row);
            if lat.parse::<f64>().is_err() || lng.parse::<f64>().is_err() {
                continue;
            }
            let id = {
                let raw = read(&self.env, &id_field, row);
                if raw.is_empty() { row.to_string() } else { raw }
            };
            let label = read(&self.env, &label_field, row);
            let info = read(&self.env, &info_field, row);
            lines.push(format!("{id}\t{lat}\t{lng}\t{label}\t{info}"));
        }
        let count = lines.len();
        self.obj_set(control_id, "Markers", lines.join("\n"));
        count
    }

    /// Read a single (non-indexed, or first-row-if-a-table) WS field's
    /// current value and write it into the target Knob/Gauge's `Value` or
    /// Switch's `Checked` (`_BindingScalarProperty`, seeded by
    /// `form_runtime.rs`). Returns `1` when a value was written, `0`
    /// otherwise — mirrors `refresh_datagrid_binding`'s row-count-as-signal
    /// convention (a Boolean would be a new, one-off return shape).
    fn refresh_scalar_binding(&mut self, control_id: &str, field: &str) -> usize {
        let property = self.obj_get(control_id, "_BindingScalarProperty");
        let property = if property.trim().is_empty() {
            "Value".to_owned()
        } else {
            property
        };
        let Some(symbol) = self.env.symbol(field) else {
            return 0;
        };
        let value = if let Some(&last_dim) = symbol.dims.last() {
            // A table field: the scalar target takes its first populated row.
            if last_dim == 0 {
                return 0;
            }
            let key = crate::environment::subscript_key(field, &[1]);
            self.env.get(&key)
        } else {
            self.env.get(field)
        };
        let Some(value) = value else {
            return 0;
        };
        let text = value.as_display_string().trim().to_owned();
        tracing::debug!(target: "databinding", "RUN-FORM scalar binding {} <- {}={}", control_id, field, text);
        self.obj_set(control_id, &property, text);
        1
    }

    fn refresh_control_array_binding(&mut self, group_id: &str) -> usize {
        // Run-form refresh for databound repeating GroupBox (ControlArray).
        // Recomputes ItemCount from the current COBOL table (like DataGrid does for Rows),
        // so that render will destroy old instances and recreate the correct number,
        // re-stamping PlacementEffect / deployment animations on the new cards.
        tracing::debug!(target: "databinding", "RUN-FORM refresh_control_array for {}", group_id);

        let design_id = self.obj_get(group_id, "_DesignControlId");
        let group_for_id = if !design_id.is_empty() {
            &design_id
        } else {
            group_id
        };

        let fields: Vec<String> = self
            .obj_get(group_id, "_BindingFields")
            .split(|ch| matches!(ch, '\n' | '\r' | ',' | ';' | '\t'))
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(str::to_owned)
            .collect();
        if fields.is_empty() {
            tracing::debug!(target: "databinding", "refresh_control_array_binding: group_id={} fields is empty!", group_id);
            databind_trace!(
                "refresh_control_array_binding: group_id={} fields is empty!",
                group_id
            );
            return 0;
        }
        let row_count = fields
            .iter()
            .filter_map(|field| self.env.symbol(field))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);

        tracing::debug!(target: "databinding", "refresh_control_array_binding: group_id={}, design_id={}, row_count={}, fields={:?}", group_id, design_id, row_count, fields);
        databind_trace!(
            "refresh_control_array_binding: group_id={}, design_id={}, row_count={}, fields={:?}",
            group_id,
            design_id,
            row_count,
            fields
        );

        // Set ItemCount directly (bypass the obj_set hook that would re-enter
        // refresh_control_array_binding and cause recursion on the count set).
        if !self.objects.contains(group_id) {
            self.objects.register(group_id, "Control");
        }
        self.objects
            .set_property(group_id, "ItemCount", row_count.to_string());
        if !design_id.is_empty() {
            if !self.objects.contains(&design_id) {
                self.objects.register(&design_id, "Control");
            }
            self.objects
                .set_property(&design_id, "ItemCount", row_count.to_string());
        }
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(StateUpdate::new(
                group_for_id.to_string(),
                "ItemCount".to_string(),
                row_count.to_string(),
            ));
        }

        // Force re-application of card effects on next expansion (like initial load)
        let placement = self.obj_get(group_id, "PlacementEffect");
        if !placement.trim().is_empty() && row_count > 0 {
            self.obj_set(group_id, "_CardEffect", placement.clone());
        }
        // Bump bind seq so that the appear clock key (group+N+seq) changes even
        // when ItemCount is the same number; this makes RefreshBinding replay
        // PlacementEffect animations on the recreated cards.
        let seq = self
            .obj_get(group_id, "_BindSeq")
            .parse::<i64>()
            .unwrap_or(0)
            + 1;
        self.obj_set(group_id, "_BindSeq", seq.to_string());

        // Hydrate per-instance member values from the live COBOL table rows (using
        // the seeded _BindingMappings). This ensures cards show current data after
        // RefreshBinding, exactly as they would after initial POPULATE.
        let maps = self.obj_get(group_id, "_BindingMappings");
        if !maps.trim().is_empty() && row_count > 0 {
            for line in maps.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() != 3 {
                    continue;
                }
                let src = parts[0].trim();
                let member = parts[1].trim();
                let prop = parts[2].trim();
                if src.is_empty() || member.is_empty() || prop.is_empty() {
                    continue;
                }
                // First pass: collect all values (to support cycling real data for
                // all cards, including ones "beyond initial view" or with unset
                // high indices in the table).
                let mut all_vals: Vec<String> = Vec::new();
                for r in 1..=row_count {
                    let key = crate::environment::subscript_key(src, &[r as i64]);
                    let v = self
                        .env
                        .get(&key)
                        .map(|v| v.as_display_string().trim_end().to_owned())
                        .unwrap_or_default();
                    all_vals.push(v);
                }
                let real_vals: Vec<String> = all_vals
                    .into_iter()
                    .filter(|v| !v.trim().is_empty())
                    .collect();
                // Second pass: push (cycling reals if any, so all cards get databound
                // data analogous to the preview seed fix).
                for r in 1..=row_count {
                    let val = if !real_vals.is_empty() {
                        real_vals[(r - 1) % real_vals.len()].clone()
                    } else {
                        // re-compute (will be default/empty)
                        let key = crate::environment::subscript_key(src, &[r as i64]);
                        self.env
                            .get(&key)
                            .map(|v| v.as_display_string().trim_end().to_owned())
                            .unwrap_or_default()
                    };

                    self.set_member_indexed(
                        member,
                        &[PathSeg::Prop(prop.to_owned())],
                        val.clone(),
                        r,
                    );

                    let indexed_member = format!("{group_for_id}.{group_for_id}-{r}.{member}");

                    databind_trace!(
                        "  r={}, member={}, prop={}, val={:?}, indexed_member={}",
                        r,
                        member,
                        prop,
                        val,
                        indexed_member
                    );

                    if let Some(tx) = &self.state_tx {
                        let _ = tx.send(
                            StateUpdate::new(member.to_string(), prop.to_owned(), val.clone())
                                .with_index(r),
                        );
                        let _ = tx.send(StateUpdate::new(
                            indexed_member,
                            prop.to_owned(),
                            val.clone(),
                        ));
                    }
                }
            }
        }

        row_count
    }

    fn datagrid_column_names(&self, control_id: &str) -> Vec<String> {
        self.obj_get(control_id, "Columns")
            .lines()
            .filter_map(|line| {
                let spec = line.trim();
                if spec.is_empty() {
                    return None;
                }
                Some(
                    spec.split_once(':')
                        .map(|(name, _)| name.trim())
                        .unwrap_or(spec)
                        .to_owned(),
                )
            })
            .collect()
    }

    fn datagrid_column_specs(&self, control_id: &str) -> Vec<(String, String)> {
        self.obj_get(control_id, "Columns")
            .lines()
            .filter_map(|line| {
                let spec = line.trim();
                if spec.is_empty() {
                    return None;
                }
                let (name, ty) = spec
                    .split_once(':')
                    .map(|(name, ty)| (name.trim(), ty.trim()))
                    .unwrap_or((spec, "string"));
                Some((name.to_owned(), ty.to_owned()))
            })
            .collect()
    }

    fn datagrid_advanced_json(&self, control_id: &str) -> Option<serde_json::Value> {
        let raw = self.obj_get(control_id, "AdvancedGrid");
        serde_json::from_str(raw.trim()).ok()
    }

    fn datagrid_display_columns(&self, control_id: &str) -> Vec<(usize, String, String)> {
        let specs = self.datagrid_column_specs(control_id);
        let mut columns = Vec::new();
        if let Some(advanced) = self.datagrid_advanced_json(control_id) {
            if let Some(advanced_columns) = advanced.get("columns").and_then(|v| v.as_array()) {
                for column in advanced_columns {
                    if column
                        .get("visible")
                        .and_then(|v| v.as_bool())
                        .map(|visible| !visible)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let source = column
                        .get("source_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let title = column
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or(source);
                    let id = column
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if let Some(index) = specs.iter().position(|(name, _)| {
                        name.eq_ignore_ascii_case(source)
                            || name.eq_ignore_ascii_case(title)
                            || name.eq_ignore_ascii_case(id)
                    }) {
                        columns.push((index, title.to_owned(), specs[index].1.clone()));
                    }
                }
            }
        }
        if columns.is_empty() {
            specs
                .into_iter()
                .enumerate()
                .map(|(index, (name, ty))| (index, name, ty))
                .collect()
        } else {
            columns
        }
    }

    fn datagrid_rows(&self, control_id: &str) -> Vec<Vec<String>> {
        self.obj_get(control_id, "Rows")
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect()
    }

    fn set_datagrid_rows(&mut self, control_id: &str, rows: &[Vec<String>]) {
        let value = rows
            .iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n");
        self.obj_set(control_id, "Rows", value);
    }

    fn datagrid_cell_index(value: &str) -> Option<usize> {
        let n = value.trim().parse::<usize>().ok()?;
        if n == 0 {
            Some(0)
        } else {
            Some(n - 1)
        }
    }

    fn set_datagrid_runtime_kv(&mut self, control_id: &str, prop: &str, key: &str, value: &str) {
        let key = key.trim();
        if key.is_empty() {
            return;
        }
        let mut entries = self
            .obj_get(control_id, prop)
            .lines()
            .filter_map(|line| {
                let (k, v) = line.split_once('=')?;
                let k = k.trim();
                if k.is_empty() || k.eq_ignore_ascii_case(key) {
                    None
                } else {
                    Some(format!("{k}={}", v.trim()))
                }
            })
            .collect::<Vec<_>>();
        if !value.trim().is_empty() {
            entries.push(format!("{key}={}", value.trim()));
        }
        self.obj_set(control_id, prop, entries.join("\n"));
    }

    fn datagrid_selected_text(&self, control_id: &str) -> String {
        let selected = self.obj_get(control_id, "_SelectedText");
        if selected.is_empty() {
            self.obj_get(control_id, "SelectedText")
        } else {
            selected
        }
    }

    fn datagrid_csv_escape(value: &str, delimiter: char) -> String {
        if value.contains(delimiter) || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_owned()
        }
    }

    fn datagrid_filter_pairs(&self, control_id: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(advanced) = self.datagrid_advanced_json(control_id) {
            if let Some(filters) = advanced.get("filters").and_then(|v| v.as_array()) {
                for filter in filters {
                    let active = filter
                        .get("active")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let column = filter
                        .get("column_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .trim();
                    let value = filter
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .trim();
                    if active && !column.is_empty() && !value.is_empty() {
                        pairs.push((column.to_owned(), value.to_owned()));
                    }
                }
            }
        }
        for line in self.obj_get(control_id, "_RuntimeColumnFilters").lines() {
            let Some((column, value)) = line.split_once('=') else {
                continue;
            };
            let column = column.trim();
            let value = value.trim();
            if !column.is_empty() && !value.is_empty() {
                pairs.retain(|(existing, _)| !existing.eq_ignore_ascii_case(column));
                pairs.push((column.to_owned(), value.to_owned()));
            }
        }
        pairs
    }

    fn datagrid_filtered_rows(&self, control_id: &str) -> Vec<Vec<String>> {
        let rows = self.datagrid_rows(control_id);
        let export_mode = self.obj_get(control_id, "CSVExportMode");
        if matches!(
            export_mode.trim().to_ascii_lowercase().as_str(),
            "all" | "allrows" | "all_rows"
        ) {
            return rows;
        }
        let filters = self.datagrid_filter_pairs(control_id);
        if filters.is_empty() {
            return rows;
        }
        let columns = self.datagrid_display_columns(control_id);
        let specs = self.datagrid_column_specs(control_id);
        rows.into_iter()
            .filter(|row| {
                filters.iter().all(|(column, needle)| {
                    let source_index = columns
                        .iter()
                        .find_map(|(source_index, title, _)| {
                            let matches = title.eq_ignore_ascii_case(column)
                                || specs
                                    .get(*source_index)
                                    .map(|(name, _)| name.eq_ignore_ascii_case(column))
                                    .unwrap_or(false);
                            matches.then_some(*source_index)
                        })
                        .or_else(|| {
                            specs
                                .iter()
                                .position(|(name, _)| name.eq_ignore_ascii_case(column))
                        });
                    let Some(source_index) = source_index else {
                        return false;
                    };
                    row.get(source_index)
                        .map(|value| {
                            value
                                .to_ascii_lowercase()
                                .contains(&needle.to_ascii_lowercase())
                        })
                        .unwrap_or(false)
                })
            })
            .collect()
    }

    fn datagrid_export_csv(&self, control_id: &str) -> String {
        let delimiter = self
            .obj_get(control_id, "CSVDelimiter")
            .chars()
            .next()
            .unwrap_or(',');
        let mut lines = Vec::new();
        let columns = self.datagrid_display_columns(control_id);
        if !columns.is_empty() {
            let header = columns
                .iter()
                .map(|(_, title, _)| Self::datagrid_csv_escape(title, delimiter))
                .collect::<Vec<_>>()
                .join(&delimiter.to_string());
            lines.push(header);
        }
        for row in self.datagrid_filtered_rows(control_id) {
            lines.push(
                columns
                    .iter()
                    .map(|(source_index, _, _)| {
                        row.get(*source_index).map(String::as_str).unwrap_or("")
                    })
                    .map(|cell| Self::datagrid_csv_escape(cell, delimiter))
                    .collect::<Vec<_>>()
                    .join(&delimiter.to_string()),
            );
        }
        lines.join("\n")
    }

    fn eval_call_arg(&mut self, arg: &CallArg, span: Span) -> Result<CobolValue, RuntimeError> {
        self.eval_expr(call_arg_expr(arg), span)
    }

    /// `true` when the object was seeded as one of the six chart control types,
    /// so chart-specific method arms only fire on actual charts.
    fn is_chart_object(&self, obj: &str) -> bool {
        matches!(
            self.objects.get(obj).map(|o| o.class.as_str()),
            Some("BarChart" | "LineChart" | "PieChart" | "AreaChart" | "ScatterChart" | "DonutChart")
        )
    }

    /// Serialise a chart's current points and push them to the GUI as the
    /// control's `__ChartData` property — one `label<TAB>value` per line. Sent
    /// empty when the chart has no data, so the renderer falls back to the sample
    /// preview. A no-op under the CLI runner (no `state_tx`).
    fn push_chart_data(&self, id: &str) {
        let Some(tx) = &self.state_tx else {
            return;
        };
        let serialized = self
            .chart_data
            .get(id)
            .map(|pts| {
                pts.iter()
                    .map(|(l, v)| format!("{}\t{}", l.replace(['\t', '\n'], " "), v))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let _ = tx.send(StateUpdate::new(
            id.to_owned(),
            "__ChartData".to_owned(),
            serialized,
        ));
    }

    // ── Expression evaluation ─────────────────────────────────────────────────

    /// Evaluate an expression to a `CobolValue`.
    /// Resolve a PowerCOBOL-style property reference to `(control, property-key)`,
    /// evaluating any subscripts. A nested path becomes a composite key, e.g.
    /// `"Text" OF "ListItems" (4) OF Listview1` → `("Listview1", "ListItems(4).Text")`.
    // ── Visual-object method dispatch (INVOKE / obj::method) ────────────────────

    /// Set a control property and notify the UI thread (auto-registers the
    /// object so the change is never silently dropped).
    fn obj_set(&mut self, obj: &str, prop: &str, val: String) {
        if !self.objects.contains(obj) {
            self.objects.register(obj, "Control");
        }
        self.objects.set_property(obj, prop, val.clone());
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(StateUpdate::new(
                obj.to_string(),
                prop.to_string(),
                val.clone(),
            ));
        }
        // 049 — an own-form property write is mirrored to the supervisor so
        // other forms' `super::X` reads stay current.
        if self
            .self_form_object
            .as_deref()
            .map(|f| f.eq_ignore_ascii_case(obj.trim()))
            .unwrap_or(false)
        {
            self.publish_own_form_prop(prop, &val);
        }
        // For a databound ControlArray, any set of ItemCount should re-hydrate
        // the current table rows into the (new) instances so cards aren't just
        // clones of the template/first row.
        if prop.eq_ignore_ascii_case("ItemCount")
            && (self.obj_get(obj, "_BindingArray") == "1"
                || self.obj_get(obj, "IsRepeatingGroup") == "1")
        {
            // Recompute + push member values for current env data. Safe to call;
            // it will read the (just-set or prior) count from dims.
            let _ = self.refresh_control_array_binding(obj);
        }
    }

    /// Read a control property as a string (`""` when unset).
    fn obj_get(&self, obj: &str, prop: &str) -> String {
        self.objects
            .get_property(obj, prop)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    /// Resolve a User Control property reference. `Child.Prop` is routed to the
    /// deployed child object `{receiver}-{Child}` when that child exists; otherwise
    /// the original receiver/property pair is preserved for backward compatibility.
    fn resolve_control_property_ref(&self, obj: &str, prop: &str) -> (String, String) {
        let obj = obj.trim();
        let prop = prop.trim();
        if let Some((child, child_prop)) = prop.split_once('.') {
            let child = child.trim();
            let child_prop = child_prop.trim();
            if !child.is_empty() && !child_prop.is_empty() {
                let qualified = format!("{obj}-{child}");
                if self.objects.contains(&qualified) {
                    return (qualified, child_prop.to_owned());
                }
            }
        }
        (obj.to_owned(), prop.to_owned())
    }

    // ── Member-access chains (spec 011) ─────────────────────────────────────────

    /// Flatten an [`Expr::Member`] chain into its root control name and an ordered
    /// list of segments, evaluating each segment's subscript/call arguments.
    fn lower_member_chain(
        &mut self,
        expr: &Expr,
    ) -> Result<(String, Vec<MemberSeg>), RuntimeError> {
        match expr {
            Expr::Member {
                recv,
                member,
                args,
                parens,
                span,
            } => {
                let (root, mut segs) = self.lower_member_chain(recv)?;
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval_expr(a, *span)?);
                }
                segs.push(MemberSeg {
                    member: member.clone(),
                    parens: *parens,
                    args: vals,
                });
                Ok((root, segs))
            }
            // 049 R30 — the root is canonicalised here, at the single point
            // every member-chain consumer (read, assign, INITIALIZE, shadow
            // flush, APPEND, methods) receives it: `me` becomes the form's
            // object name.
            Expr::Identifier(name, _) => Ok((self.member_root_key(name), Vec::new())),
            // A non-identifier root is unusual; fall back to its display name.
            other => {
                let name = self.expr_to_name(other);
                Ok((self.member_root_key(&name), Vec::new()))
            }
        }
    }

    /// Resolve a member chain to either an addressable **path** (a property or a
    /// collection element — readable and assignable) or a **method call** on a
    /// place (an rvalue only). A `parens` segment is an *index* when its member
    /// names a collection (list/legacy-string) and an *argument* is present;
    /// otherwise it is a method call (terminal).
    fn resolve_member(&mut self, expr: &Expr) -> Result<(String, Resolved), RuntimeError> {
        let (root, segs) = self.lower_member_chain(expr)?;
        let mut path: Vec<PathSeg> = Vec::new();
        for seg in segs {
            if !seg.parens {
                // A bare member is always a property (readable / assignable).
                path.push(PathSeg::Prop(seg.member));
                continue;
            }
            // `member( … )` is a **method call** when the name is a known method
            // (or there are no arguments — `Foo()` can only be a call); otherwise
            // it is a **collection index** (`Items(4)`, `Rows(I)`). This decision
            // does not depend on the collection already existing, so writes that
            // auto-vivify nested structure classify correctly (spec 011).
            if is_known_method(&seg.member) || seg.args.is_empty() {
                return Ok((
                    root,
                    Resolved::Method {
                        path,
                        method: seg.member,
                        args: seg.args,
                    },
                ));
            }
            let idx = seg.args[0]
                .as_display_string()
                .trim()
                .parse::<i64>()
                .unwrap_or(0)
                .max(0) as usize;
            path.push(PathSeg::Prop(seg.member));
            path.push(PathSeg::Index(idx));
        }
        Ok((root, Resolved::Path(path)))
    }

    /// Evaluate a member chain to a value (the rvalue / GET form).
    fn eval_member(&mut self, expr: &Expr) -> Result<CobolValue, RuntimeError> {
        if let Expr::Member {
            recv,
            member,
            args,
            span,
            ..
        } = expr
        {
            let is_var_or_lit = match recv.as_ref() {
                Expr::Literal(..) => true,
                Expr::Identifier(name, _) => self.env.contains(name),
                Expr::Qualified { name, .. } => self.env.contains(name),
                Expr::Member { .. } => true,
                Expr::Subscript { .. } => true,
                Expr::RefMod { .. } => true,
                Expr::Arithmetic { .. } => true,
                _ => false,
            };

            if is_var_or_lit {
                let recv_val = self.eval_expr(recv, *span)?;
                let m = member.to_ascii_uppercase();
                let mut evaluated_args = Vec::with_capacity(args.len());
                for arg in args {
                    evaluated_args.push(self.eval_expr(arg, *span)?);
                }

                match m.as_str() {
                    "REPLACE" => {
                        let target = evaluated_args
                            .get(0)
                            .map(|v| v.as_display_string())
                            .unwrap_or_default();
                        let replacement = evaluated_args
                            .get(1)
                            .map(|v| v.as_display_string())
                            .unwrap_or_default();
                        let res_str = recv_val.as_display_string().replace(&target, &replacement);
                        let len = res_str.len();
                        return Ok(CobolValue::from_str(&res_str, len));
                    }
                    "TOUPPERCASE" | "UPPERCASE" | "UPPER" => {
                        let res_str = recv_val.as_display_string().to_uppercase();
                        let len = res_str.len();
                        return Ok(CobolValue::from_str(&res_str, len));
                    }
                    "TOLOWERCASE" | "LOWERCASE" | "LOWER" => {
                        let res_str = recv_val.as_display_string().to_lowercase();
                        let len = res_str.len();
                        return Ok(CobolValue::from_str(&res_str, len));
                    }
                    "TRIM" => {
                        let res_str = recv_val.as_display_string().trim().to_string();
                        let len = res_str.len();
                        return Ok(CobolValue::from_str(&res_str, len));
                    }
                    "LEN" | "LENGTH" => {
                        let count = recv_val.as_display_string().chars().count();
                        return Ok(CobolValue::Numeric(CobolNumeric::integer(count as i64)));
                    }
                    "SPLIT" => {
                        let sep = evaluated_args
                            .get(0)
                            .map(|v| v.as_display_string())
                            .unwrap_or_else(|| " ".to_string());
                        let text = recv_val.as_display_string();
                        let splits: Vec<&str> = text.split(&sep).collect();
                        let elem = splits.first().cloned().unwrap_or("").to_string();
                        let len = elem.len();
                        return Ok(CobolValue::from_str(&elem, len));
                    }
                    _ => {}
                }
            }
        }

        let (root, res) = self.resolve_member(expr)?;
        match res {
            Resolved::Path(path) => {
                // 049 R28 — `super::X` reads the PARENT form's published
                // property surface through the supervisor.
                if self.is_super(&root) {
                    if let Some(v) = self.super_prop_read(&path)? {
                        return Ok(v);
                    }
                }
                // 037 — `H::FormState` reads through a windowHandler (R23).
                if let Some(v) = self.try_window_handle_prop(&root, &path)? {
                    return Ok(v);
                }
                Ok(prop_to_value(self.objects.get_path(&root, &path)))
            }
            Resolved::Method { path, method, args } => {
                // 049 R31 — a window method on a CHAINED super receiver
                // (`super::super::"Close"()`): resolve the ancestor, then the
                // ordinary handle-method round-trip.
                if self.is_super(&root) && !path.is_empty() {
                    let (handle, rest) = self.resolve_super_target(&path)?;
                    if rest.is_empty() {
                        let strings: Vec<String> = args
                            .iter()
                            .map(|v| v.as_display_string().trim().to_string())
                            .collect();
                        let value =
                            self.window_method_roundtrip(&handle, &method, strings)?;
                        let n = value.len();
                        return Ok(CobolValue::from_str(&value, n.max(1)));
                    }
                    // 049 R44 — a MENU OBJECT on the ancestor form:
                    // `super::<menu-id>::Collapse()` / `Open()`. Pane-wide by
                    // decision (spec Q10); the state persists under R9.
                    if rest.len() == 1 {
                        let m = method.trim().to_ascii_uppercase();
                        if m == "COLLAPSE" || m == "OPEN" {
                            let menu_id = match &rest[0] {
                                PathSeg::Prop(id) => id.clone(),
                                PathSeg::Index(_) => String::new(),
                            };
                            let on = if m == "COLLAPSE" { "1" } else { "0" };
                            self.window_method_roundtrip(
                                &handle,
                                "SETMENUPANECOLLAPSED",
                                vec![on.to_string(), menu_id],
                            )?;
                            return Ok(CobolValue::from_str("", 1));
                        }
                    }
                }
                // 037 — inline window calls (`me::"OpenFormSync"(…)`,
                // `H::"Close"()`) raise real runtime errors and block on
                // modal opens, so they cannot go through the infallible
                // widget dispatcher.
                if path.is_empty() {
                    if let Some(v) = self.try_exec_window_call(&root, &method, &args)? {
                        return Ok(v);
                    }
                }
                Ok(self.exec_member_method(&root, &path, &method, &args))
            }
        }
    }

    /// Assign `val` to a member chain used as a receiving field. A method-call
    /// tail is not a receiving field (spec 011) → a runtime error.
    /// The 1-based repeating-group instance index of a `Member(idx)::Prop` write,
    /// i.e. a subscript applied **directly to the control identifier** at the root
    /// of the member chain (`Button-1(I)::Caption`). A subscript deeper in the
    /// chain (`Grid::Rows(2)::Value`) is a collection index, not an instance, so it
    /// is ignored. `0` when the target is a plain scalar control member.
    fn member_instance_index(&mut self, target: &Expr) -> usize {
        let mut root = target;
        while let Expr::Member { recv, .. } = root {
            root = recv;
        }
        if let Expr::Subscript { base, indices, .. } = root {
            if matches!(base.as_ref(), Expr::Identifier(..)) {
                if let Some(first) = indices.first() {
                    if let Ok(v) = self.eval_expr(first, target.span()) {
                        return v
                            .as_display_string()
                            .trim()
                            .parse::<i64>()
                            .unwrap_or(0)
                            .max(0) as usize;
                    }
                }
            }
        }
        0
    }

    fn assign_member(&mut self, target: &Expr, val: &CobolValue) -> Result<(), RuntimeError> {
        let instance = self.member_instance_index(target);
        let (root, res) = self.resolve_member(target)?;
        match res {
            Resolved::Path(path) => {
                let v = val.as_display_string().trim().to_owned();
                // 049 R28/R31 — `MOVE … TO super[::super…]::X` writes the
                // resolved ancestor's property through the supervisor
                // (write-through, blocking, so a NULL link raises the R32
                // error here and now).
                if self.is_super(&root) {
                    let (handle, rest) = self.resolve_super_target(&path)?;
                    let Some(key) = single_prop_key(&rest) else {
                        return Err(RuntimeError::General {
                            message: "super only exposes form properties — \
                                      a nested path cannot be assigned through it"
                                .into(),
                        });
                    };
                    self.window_method_roundtrip(&handle, "SETPROPERTY", vec![key, v])?;
                    return Ok(());
                }
                self.set_member_indexed(&root, &path, v, instance);
                Ok(())
            }
            Resolved::Method { method, .. } => Err(RuntimeError::General {
                message: format!(
                    "'{root}::{method}' is a method call, not a receiving field — call it as a \
                     statement instead of using it as a MOVE/assignment target"
                ),
            }),
        }
    }

    /// Dispatch a method on a nested place. With an empty `path` the receiver is
    /// the root control, so the call is delegated to the universal/per-widget
    /// dispatcher [`Self::exec_method`] (and the Rust-FFI bridge). Otherwise it is
    /// a collection verb (`delete`/`count`/`clear`/`add`) or a scalar transform
    /// (`toUpperCase`/`toLowerCase`/`trim`/`len`).
    fn exec_member_method(
        &mut self,
        root: &str,
        path: &[PathSeg],
        method: &str,
        args: &[CobolValue],
    ) -> CobolValue {
        if path.is_empty() {
            return self.exec_method(root, method, args);
        }
        let m = method.to_ascii_uppercase();
        let arg0 = args
            .first()
            .map(|v| v.as_display_string().trim().to_string())
            .unwrap_or_default();
        let none = CobolValue::from_str("", 0);
        match m.as_str() {
            "DELETE" | "REMOVE" => {
                if matches!(path.last(), Some(PathSeg::Index(_))) {
                    self.objects.remove_path(root, path);
                } else if !arg0.is_empty() {
                    let mut p = path.to_vec();
                    p.push(PathSeg::Index(arg0.parse::<usize>().unwrap_or(0)));
                    self.objects.remove_path(root, &p);
                }
                none
            }
            "COUNT" | "SIZE" => {
                let n = self.objects.path_len(root, path).unwrap_or(0);
                CobolValue::Numeric(CobolNumeric::integer(n as i64))
            }
            "CLEAR" => {
                self.set_member(root, path, String::new());
                none
            }
            "ADD" | "APPEND" => {
                self.append_member(root, path, &arg0);
                none
            }
            _ => {
                let cur = self
                    .objects
                    .get_path(root, path)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let out = match m.as_str() {
                    "TOUPPERCASE" | "UPPERCASE" | "UPPER" => cur.to_uppercase(),
                    "TOLOWERCASE" | "LOWERCASE" | "LOWER" => cur.to_lowercase(),
                    "TRIM" => cur.trim().to_string(),
                    "LEN" | "LENGTH" => {
                        return CobolValue::Numeric(CobolNumeric::integer(
                            cur.chars().count() as i64
                        ));
                    }
                    // Unknown method on a value → no effect; return the value.
                    _ => cur,
                };
                let n = out.len();
                CobolValue::from_str(&out, n)
            }
        }
    }

    /// Write a string value to a member place and notify the UI. A flat
    /// `[Prop(name)]` path emits `StateUpdate(control, name, value)` so existing
    /// UI bindings update; a deeper path emits a best-effort joined key.
    fn set_member(&mut self, root: &str, path: &[PathSeg], val: String) {
        self.set_member_indexed(root, path, val, 0);
    }

    /// As [`set_member`], but tags the UI notification with a repeating-group
    /// `instance` (1-based) so the host routes `Member(idx)::Prop` writes to the
    /// right cloned card. `instance == 0` is a scalar control (unchanged).
    fn set_member_indexed(&mut self, root: &str, path: &[PathSeg], val: String, instance: usize) {
        self.objects
            .set_path(root, path, PropertyValue::String(val.clone()));
        if let Some(tx) = &self.state_tx {
            let key = match path {
                [PathSeg::Prop(name)] => name.clone(),
                _ => path_display(path),
            };
            let _ = tx.send(
                StateUpdate::new(root.to_string(), key, val.clone()).with_index(instance),
            );
        }
        // 049 — own-form property writes (me::X / <FORM-NAME>::X) are
        // mirrored to the supervisor for other forms' `super::X` reads.
        if self
            .self_form_object
            .as_deref()
            .map(|f| f.eq_ignore_ascii_case(root.trim()))
            .unwrap_or(false)
        {
            if let Some(key) = single_prop_key(path) {
                self.publish_own_form_prop(&key, &val);
            }
        }
    }

    /// Append an item to the list (or legacy newline-string) at `path`.
    fn append_member(&mut self, root: &str, path: &[PathSeg], item: &str) {
        match self.objects.get_path(root, path) {
            Some(PropertyValue::List(mut items)) => {
                items.push(PropertyValue::String(item.to_string()));
                self.objects
                    .set_path(root, path, PropertyValue::List(items));
            }
            Some(PropertyValue::String(s)) => {
                let nv = if s.is_empty() {
                    item.to_string()
                } else {
                    format!("{s}\n{item}")
                };
                self.objects.set_path(root, path, PropertyValue::String(nv));
            }
            _ => {
                self.objects.set_path(
                    root,
                    path,
                    PropertyValue::List(vec![PropertyValue::String(item.to_string())]),
                );
            }
        }
    }

    /// Execute a control method (`obj::method(args)` / `INVOKE obj "method"`).
    /// Most methods are thin sugar over property get/set — which the form
    /// runtime mirrors to the live UI — and getters return a value (for the
    /// expression form and `RETURNING`).
    /// Number of live Rust-FFI objects (for tests / leak checks). 0 ⇒ none held.
    pub fn rust_object_count(&self) -> usize {
        self.rust_bridge.live_count()
    }

    /// Dispatch a method on an `OBJECT REFERENCE` item into the Rust bridge,
    /// marshaling the COBOL arguments in and the result back out (spec 005 T10).
    fn invoke_rust(&mut self, key: &str, method: &str, args: &[CobolValue]) -> CobolValue {
        let id = self.env.get_i64(key).unwrap_or(0);
        let bargs: Vec<crate::rust_bridge::BridgeValue> =
            args.iter().map(cobol_to_bridge).collect();
        // The curated bridge methods are lowercase snake_case (`len`,
        // `to_uppercase`, …). The inline `obj::method()` form arrives with the
        // method uppercased by the COBOL lexer (`LEN`), whereas `INVOKE … "len"`
        // preserves the literal; lowercasing here makes both forms dispatch (R16).
        let method = method.to_ascii_lowercase();
        match self.rust_bridge.invoke(id, &method, &bargs) {
            Ok(v) => bridge_to_cobol(v),
            Err(e) => {
                tracing::warn!("Rust bridge {key}::{method}: {e}");
                CobolValue::from_str("", 0)
            }
        }
    }

    fn exec_method(&mut self, object: &str, method: &str, args: &[CobolValue]) -> CobolValue {
        // 049 R30 — `INVOKE ME "SetProperty" …` must land on the form object,
        // not a phantom "ME" control (same canonicalisation as member chains).
        let object = self.member_root_key(object);
        let obj = object.trim();
        // Rust-FFI object reference? Route the call into the bridge before the
        // UI-widget method dispatch below (spec 005 T10). The method name is kept
        // as written (Rust methods are case-sensitive: `len`, `to_uppercase`, …).
        let upper = obj.to_ascii_uppercase();
        if self.object_refs.contains_key(&upper) {
            return self.invoke_rust(&upper, method, args);
        }
        let m = method.to_ascii_uppercase();
        let arg = |i: usize| {
            args.get(i)
                .map(|v| v.as_display_string().trim().to_string())
                .unwrap_or_default()
        };
        let val = |s: String| {
            let n = s.len();
            CobolValue::from_str(&s, n)
        };
        let truthy = |s: &str| {
            let t = s.trim();
            t == "1"
                || t.eq_ignore_ascii_case("true")
                || t.eq_ignore_ascii_case("yes")
                || t.eq_ignore_ascii_case("on")
        };
        let b01 = |s: &str| {
            if truthy(s) {
                "1".to_string()
            } else {
                "0".to_string()
            }
        };
        let none = CobolValue::from_str("", 0);
        let parse_i = |s: String| s.trim().parse::<i64>().unwrap_or(0);

        match m.as_str() {
            // ── Universal lifecycle / visibility ──
            "SHOW" => {
                self.obj_set(obj, "Visible", "1".into());
                none
            }
            "HIDE" => {
                self.obj_set(obj, "Visible", "0".into());
                none
            }
            "ENABLE" => {
                self.obj_set(obj, "Enabled", "1".into());
                none
            }
            "DISABLE" => {
                self.obj_set(obj, "Enabled", "0".into());
                none
            }
            "SETFOCUS" | "FOCUS" => {
                self.obj_set(obj, "Focused", "1".into());
                none
            }
            "BRINGTOFRONT" => {
                self.obj_set(obj, "ZOrder", "10000".into());
                none
            }
            "SENDTOBACK" => {
                self.obj_set(obj, "ZOrder", "-10000".into());
                none
            }
            "REFRESH" | "VALIDATE" => {
                // On a chart, Refresh re-sends the current data (same contract
                // as CALL "COBOL-CHART-REFRESH"); elsewhere it stays a no-op.
                if self.is_chart_object(obj) {
                    self.push_chart_data(&obj.to_ascii_uppercase());
                }
                none
            }
            // ── Geometry ──
            "MOVETO" => {
                self.obj_set(obj, "X", arg(0));
                self.obj_set(obj, "Y", arg(1));
                none
            }
            "RESIZE" => {
                self.obj_set(obj, "Width", arg(0));
                self.obj_set(obj, "Height", arg(1));
                none
            }
            // ── Generic property access ──
            "SETPROPERTY" => {
                let p = arg(0);
                let (target, key) = self.resolve_control_property_ref(obj, &p);
                self.obj_set(&target, &key, arg(1));
                none
            }
            "GETPROPERTY" => {
                let p = arg(0);
                let (target, key) = self.resolve_control_property_ref(obj, &p);
                val(self.obj_get(&target, &key))
            }
            // ── Text / caption ──
            "SETCAPTION" => {
                self.obj_set(obj, "Caption", arg(0));
                none
            }
            "SETTEXT" => {
                self.obj_set(obj, "Text", arg(0));
                none
            }
            "GETCAPTION" => val(self.obj_get(obj, "Caption")),
            "GETTEXT" => val(self.obj_get(obj, "Text")),
            "APPENDTEXT" => {
                let cur = self.obj_get(obj, "Text");
                self.obj_set(obj, "Text", format!("{cur}{}", arg(0)));
                none
            }
            "SETCOLOR" => {
                self.obj_set(obj, "ForegroundColor", arg(0));
                none
            }
            "SELECTALL" => none,
            "CLEAR" => {
                // On a chart, Clear drops the pushed data series (same contract
                // as CALL "COBOL-CHART-CLEAR") — the renderer falls back to its
                // sample preview until new points arrive.
                if self.is_chart_object(obj) {
                    let key = obj.to_ascii_uppercase();
                    self.chart_data.remove(&key);
                    self.push_chart_data(&key);
                }
                self.obj_set(obj, "Text", String::new());
                self.obj_set(obj, "Items", String::new());
                none
            }
            // ── Charts (inline methods — same data path as COBOL-CHART-*) ──
            "ADDPOINT" | "ADD-POINT" => {
                let key = obj.to_ascii_uppercase();
                let label = arg(0);
                let value = args.get(1).map(|v| v.as_f64()).unwrap_or(0.0);
                self.chart_data
                    .entry(key.clone())
                    .or_default()
                    .push((label, value));
                self.push_chart_data(&key);
                none
            }
            // ── Checkbox / radio ──
            "ISCHECKED" => val(b01(&self.obj_get(obj, "Checked"))),
            "SETCHECKED" => {
                let v = b01(&arg(0));
                self.obj_set(obj, "Checked", v);
                none
            }
            "SELECT" => {
                self.obj_set(obj, "Checked", "1".into());
                none
            }
            "TOGGLE" => {
                let c = self.obj_get(obj, "Checked");
                let nv = if truthy(&c) { "0" } else { "1" };
                self.obj_set(obj, "Checked", nv.into());
                none
            }
            // ── Numeric value (progress/slider/numeric/datetime) ──
            "SETVALUE" => {
                self.obj_set(obj, "Value", arg(0));
                none
            }
            "GETVALUE" => val(self.obj_get(obj, "Value")),
            "INCREMENT" => {
                let st = parse_i(self.obj_get(obj, "Step"));
                let st = if st == 0 { 1 } else { st };
                let v = parse_i(self.obj_get(obj, "Value"));
                self.obj_set(obj, "Value", (v + st).to_string());
                none
            }
            "DECREMENT" => {
                let st = parse_i(self.obj_get(obj, "Step"));
                let st = if st == 0 { 1 } else { st };
                let v = parse_i(self.obj_get(obj, "Value"));
                self.obj_set(obj, "Value", (v - st).to_string());
                none
            }
            "RESET" => {
                let min = self.obj_get(obj, "Minimum");
                let m2 = if min.trim().is_empty() {
                    "0".to_string()
                } else {
                    min
                };
                self.obj_set(obj, "Value", m2);
                none
            }
            // ── Items (list / combo) ──
            "ADDITEM" => {
                let cur = self.obj_get(obj, "Items");
                let nv = if cur.is_empty() {
                    arg(0)
                } else {
                    format!("{cur}\n{}", arg(0))
                };
                self.obj_set(obj, "Items", nv);
                none
            }
            "REMOVEITEM" => {
                let idx = arg(0).trim().parse::<usize>().unwrap_or(usize::MAX);
                let cur = self.obj_get(obj, "Items");
                let mut lines: Vec<String> = cur.lines().map(|l| l.to_string()).collect();
                if idx < lines.len() {
                    lines.remove(idx);
                }
                self.obj_set(obj, "Items", lines.join("\n"));
                none
            }
            "GETSELECTED" => val(self.obj_get(obj, "Value")),
            "GETSELECTEDINDEX" | "GETINDEX" => val(self.obj_get(obj, "SelectedIndex")),
            "SETSELECTEDINDEX" | "SETINDEX" => {
                self.obj_set(obj, "SelectedIndex", arg(0));
                none
            }
            "GETCOUNT" => {
                let cur = self.obj_get(obj, "Items");
                let n = if cur.trim().is_empty() {
                    0
                } else {
                    cur.lines().count()
                };
                val(n.to_string())
            }
            // ── DataGrid data binding ──
            "GETROWCOUNT" => val(self.datagrid_rows(obj).len().to_string()),
            "GETCELLVALUE" => {
                let row = Self::datagrid_cell_index(&arg(0));
                let col = Self::datagrid_cell_index(&arg(1));
                let value = row
                    .zip(col)
                    .and_then(|(row, col)| {
                        self.datagrid_rows(obj)
                            .get(row)
                            .and_then(|cells| cells.get(col))
                            .cloned()
                    })
                    .unwrap_or_default();
                val(value)
            }
            "SETCELLVALUE" => {
                if let (Some(row), Some(col)) = (
                    Self::datagrid_cell_index(&arg(0)),
                    Self::datagrid_cell_index(&arg(1)),
                ) {
                    let mut rows = self.datagrid_rows(obj);
                    let target_cols = self.datagrid_column_names(obj).len().max(col + 1);
                    while rows.len() <= row {
                        rows.push(vec![String::new(); target_cols]);
                    }
                    if rows[row].len() <= col {
                        rows[row].resize(col + 1, String::new());
                    }
                    rows[row][col] = arg(2);
                    self.set_datagrid_rows(obj, &rows);
                }
                none
            }
            "ADDROW" => {
                let mut rows = self.datagrid_rows(obj);
                let row = if args.is_empty() {
                    vec![String::new(); self.datagrid_column_names(obj).len()]
                } else {
                    arg(0).split('\t').map(str::to_owned).collect()
                };
                rows.push(row);
                self.set_datagrid_rows(obj, &rows);
                none
            }
            "DELETEROW" => {
                if let Some(row) = Self::datagrid_cell_index(&arg(0)) {
                    let mut rows = self.datagrid_rows(obj);
                    if row < rows.len() {
                        rows.remove(row);
                        self.set_datagrid_rows(obj, &rows);
                    }
                }
                none
            }
            "CLEARROWS" => {
                self.obj_set(obj, "Rows", String::new());
                none
            }
            "SORT" => {
                if let Some(col) = Self::datagrid_cell_index(&arg(0)) {
                    let mut rows = self.datagrid_rows(obj);
                    rows.sort_by(|left, right| {
                        left.get(col)
                            .map(String::as_str)
                            .unwrap_or("")
                            .cmp(right.get(col).map(String::as_str).unwrap_or(""))
                    });
                    self.set_datagrid_rows(obj, &rows);
                }
                none
            }
            "SETFILTER" => {
                let column = arg(0);
                let value = arg(1);
                self.set_datagrid_runtime_kv(obj, "_RuntimeColumnFilters", &column, &value);
                self.set_datagrid_runtime_kv(obj, "ColumnFilters", &column, &value);
                none
            }
            "CLEARFILTERS" => {
                self.obj_set(obj, "_RuntimeColumnFilters", String::new());
                self.obj_set(obj, "ColumnFilters", String::new());
                none
            }
            "FREEZECOLUMNS" => {
                self.obj_set(obj, "_RuntimeFrozenColumns", arg(0));
                self.obj_set(obj, "FrozenColumns", arg(0));
                none
            }
            "FREEZEROWS" => {
                self.obj_set(obj, "_RuntimeFrozenRows", arg(0));
                self.obj_set(obj, "FrozenRows", arg(0));
                none
            }
            "SETROWHEIGHT" => {
                self.obj_set(obj, "_RuntimeRowHeight", arg(0));
                self.obj_set(obj, "RowHeight", arg(0));
                none
            }
            "SETCOLUMNWIDTH" => {
                self.set_datagrid_runtime_kv(obj, "_RuntimeColumnWidths", &arg(0), &arg(1));
                none
            }
            "GETSELECTEDTEXT" => val(self.datagrid_selected_text(obj)),
            "COPYSELECTION" => {
                self.obj_set(obj, "_CopySelection", "1".into());
                val(self.datagrid_selected_text(obj))
            }
            "EXPORTCSV" => val(self.datagrid_export_csv(obj)),
            "REFRESHBINDING" => {
                tracing::debug!(target: "databinding", "RUN-FORM {} REFRESHBINDING", obj);
                let n = self.refresh_binding(obj);
                val(n.to_string())
            }
            // ── Timer ──
            "START" => {
                self.obj_set(obj, "Enabled", "1".into());
                none
            }
            "STOP" => {
                self.obj_set(obj, "Enabled", "0".into());
                none
            }
            "SETINTERVAL" => {
                self.obj_set(obj, "Interval", arg(0));
                none
            }
            "ISENABLED" => val(b01(&self.obj_get(obj, "Enabled"))),
            // ── Animation ──
            "PLAYANIMATION" | "PLAY" => {
                let a = if args.is_empty() {
                    "1".to_string()
                } else {
                    arg(0)
                };
                self.obj_set(obj, "_PlayAnimation", a);
                none
            }
            "STOPANIMATION" => {
                self.obj_set(obj, "_StopAnimation", "1".into());
                none
            }
            "PAUSE" => {
                self.obj_set(obj, "_PauseAnimation", "1".into());
                none
            }
            // ── AgentObject extras ──
            "CLOSE" => {
                // SqlDatabase::Close closes the connection; otherwise hide a window.
                let h = parse_i(self.obj_get(obj, "_Handle"));
                if h > 0 {
                    self.db.close(h as u32);
                } else {
                    self.obj_set(obj, "Visible", "0".into());
                }
                none
            }
            // Pre-existing generic accessor (`Result` property) for a
            // no-argument call; spec 039 T15/R29 adds WebSearch's indexed
            // `INVOKE <id> 'GetResult' USING <n>` on the same method name —
            // an argument present means "indexed WebSearch result", absent
            // preserves the original behaviour for every other caller.
            "GETRESULT" if args.is_empty() => val(self.obj_get(obj, "Result")),
            "GETRESULT" => {
                let n: usize = arg(0).trim().parse().unwrap_or(0);
                let items = self.web_search_items(obj);
                if n >= 1 && n <= items.len() {
                    let (title, snippet, link) = &items[n - 1];
                    val(format!("{title}\t{snippet}\t{link}"))
                } else {
                    val(String::new())
                }
            }
            "SETTITLE" => {
                self.obj_set(obj, "Title", arg(0));
                none
            }
            // AgentObject (LLM): prompt/model are stored; Ask records the prompt
            // and returns the last reply property (filled by the host LLM bridge).
            "SETPROMPT" => {
                self.obj_set(obj, "SystemPrompt", arg(0));
                none
            }
            "SETMODEL" => {
                self.obj_set(obj, "Model", arg(0));
                none
            }
            "ASK" => {
                self.obj_set(obj, "Prompt", arg(0));
                let reply = self.obj_get(obj, "LastReply");
                // spec 021: a non-empty reply is a delivered response.
                if !reply.trim().is_empty() {
                    self.queue_control_event(obj, "onResponse");
                }
                val(reply)
            }
            // ── REST / HTTP client ──
            // Async by default (spec 032): unless `Mode = Sync`, the verb spawns
            // a background worker, sets `Busy = 1`, and returns immediately; the
            // response arrives later as an onComplete/onError event. `Mode = Sync`
            // keeps the original blocking, same-statement-result behaviour.
            "GET" => {
                if self.rest_is_async(obj) {
                    self.spawn_rest_op(obj, "GET", arg(0), String::new())
                } else {
                    let (b, st) = self.http.get(&arg(0));
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            "POST" => {
                if self.rest_is_async(obj) {
                    self.spawn_rest_op(obj, "POST", arg(0), arg(1))
                } else {
                    let (b, st) = self.http.post(&arg(0), &arg(1));
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            "PUT" => {
                if self.rest_is_async(obj) {
                    self.spawn_rest_op(obj, "PUT", arg(0), arg(1))
                } else {
                    let (b, st) = self.http.put(&arg(0), &arg(1));
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            "DELETE" => {
                if self.rest_is_async(obj) {
                    self.spawn_rest_op(obj, "DELETE", arg(0), String::new())
                } else {
                    let (b, st) = self.http.delete(&arg(0));
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            // ── WebSearch (spec 039 T15): Google Custom Search JSON API ──
            // Reuses `spawn_rest_op`/the plain `ureq` transport (unlike Maps,
            // which needed the async `google_maps` crate + its own worker) —
            // a Custom Search call is a plain signed GET.
            "SEARCH" => {
                let api_key = self.obj_get(obj, "_ResolvedSearchApiKey");
                if api_key.trim().is_empty() {
                    // R33: "not configured" — fail synchronously, no request.
                    self.obj_set(obj, "LastError", "Web Search API key not configured".into());
                    self.async_dispatch_queue
                        .push_back((obj.to_string(), "onError".to_string()));
                    return CobolValue::from_str("", 0);
                }
                let cx = self.obj_get(obj, "SearchEngineId");
                let q = self.obj_get(obj, "Query");
                let num = self
                    .obj_get(obj, "NumResults")
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(10)
                    .clamp(1, 10); // the Custom Search API's own per-request cap
                // The API's `safe` param is two-valued ("off"/"active"); our
                // friendlier Off/Medium/High property (T14) collapses Medium
                // and High to the API's single "active" level.
                let safe = if self.obj_get(obj, "SafeSearch").eq_ignore_ascii_case("off") {
                    "off"
                } else {
                    "active"
                };
                let url = format!(
                    "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={num}&safe={safe}",
                    percent_encode_query(&api_key),
                    percent_encode_query(&cx),
                    percent_encode_query(&q),
                );
                if self.rest_is_async(obj) {
                    self.spawn_rest_op(obj, "GET", url, String::new())
                } else {
                    let (b, st) = self.http.get(&url);
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            // R29: read-only accessors over `ResponseBody`'s raw JSON,
            // computed fresh on each call rather than cached in separate
            // properties eagerly populated on completion — there is no
            // per-control-type hook in the generic async delivery path
            // (`drain_async_ops`/`obj_set`, shared with RestClient), and
            // adding one just for this would be a bigger, riskier change
            // than parsing `ResponseBody` (already a plain property) each
            // time one of these is invoked.
            "RESULTCOUNT" => val(self.web_search_items(obj).len().to_string()),
            "TOPTITLE" => val(
                self.web_search_items(obj)
                    .first()
                    .map(|r| r.0.clone())
                    .unwrap_or_default(),
            ),
            "TOPSNIPPET" => val(
                self.web_search_items(obj)
                    .first()
                    .map(|r| r.1.clone())
                    .unwrap_or_default(),
            ),
            "TOPLINK" => val(
                self.web_search_items(obj)
                    .first()
                    .map(|r| r.2.clone())
                    .unwrap_or_default(),
            ),
            // ── Maps (spec 039 T11): Directions/Geocoding/Places/
            // Distance-Matrix, backed by the `google_maps` crate — always
            // async (the tokio-runtime spin-up cost makes a same-statement
            // Sync variant not worth offering, unlike RestClient's GET/POST).
            "GEOCODE" => self.spawn_maps_op(obj, "GEOCODE", vec![arg(0)]),
            "REVERSEGEOCODE" => self.spawn_maps_op(obj, "REVERSEGEOCODE", vec![arg(0), arg(1)]),
            "DIRECTIONS" => self.spawn_maps_op(obj, "DIRECTIONS", vec![arg(0), arg(1)]),
            "DISTANCEMATRIX" => {
                self.spawn_maps_op(obj, "DISTANCEMATRIX", vec![arg(0), arg(1)])
            }
            "PLACESSEARCH" => self.spawn_maps_op(obj, "PLACESSEARCH", vec![arg(0), arg(1)]),
            // Markers accessors (R18) — the property write path
            // (`SET <mapid>::Markers TO ...`) already works generically
            // (Markers is a plain string property like any other), these
            // are the ergonomic alternative so a developer doesn't have to
            // hand-format the tab/newline-separated shape themselves. Not
            // duplicated here as a shared parser with `cobolt_forms::
            // parse_map_markers` — `cobolt-runtime` does not depend on
            // `cobolt-forms` outside tests, and the shape is one line of
            // string formatting, not worth a cross-crate dependency for.
            "ADDMARKER" => {
                let (id, lat, lng, label, info) = (arg(0), arg(1), arg(2), arg(3), arg(4));
                let line = format!("{id}\t{lat}\t{lng}\t{label}\t{info}");
                let existing = self.obj_get(obj, "Markers");
                let updated = if existing.trim().is_empty() {
                    line
                } else {
                    format!("{existing}\n{line}")
                };
                self.obj_set(obj, "Markers", updated);
                none
            }
            "REMOVEMARKER" => {
                let target_id = arg(0);
                let existing = self.obj_get(obj, "Markers");
                let updated: String = existing
                    .lines()
                    .filter(|line| {
                        line.split('\t').next().unwrap_or("") != target_id.as_str()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.obj_set(obj, "Markers", updated);
                none
            }
            "CALL" => {
                let verb = arg(0).to_ascii_uppercase();
                if self.rest_is_async(obj) {
                    let (url, body) = match verb.as_str() {
                        "POST" | "PUT" => (arg(1), arg(2)),
                        _ => (arg(1), String::new()),
                    };
                    self.spawn_rest_op(obj, &verb, url, body)
                } else {
                    let (b, st) = match verb.as_str() {
                        "POST" => self.http.post(&arg(1), &arg(2)),
                        "PUT" => self.http.put(&arg(1), &arg(2)),
                        "DELETE" => self.http.delete(&arg(1)),
                        _ => self.http.get(&arg(1)),
                    };
                    self.obj_set(obj, "ResponseBody", b.clone());
                    self.obj_set(obj, "StatusCode", st.to_string());
                    val(b)
                }
            }
            // ── Async operation control (spec 032) — all async-capable controls ──
            "CANCEL" => {
                self.cancel_async_op(obj);
                none
            }
            "ISBUSY" => val(b01(&self.obj_get(obj, "Busy"))),
            "SETHEADER" => {
                self.http.set_header(arg(0), arg(1));
                none
            }
            "CLEARHEADERS" => {
                self.http.clear_headers();
                none
            }
            "SETTIMEOUT" => {
                self.obj_set(obj, "Timeout", arg(0));
                none
            }
            // ── SQL database ──
            "OPEN" => match self.db.open(&arg(0)) {
                Ok(h) => {
                    self.obj_set(obj, "_Handle", h.to_string());
                    self.obj_set(obj, "StatusCode", "0".into());
                    // spec 021: connection lifecycle events (dispatched by the
                    // event loop on the next COBOL-WAIT-EVENT).
                    self.queue_control_event(obj, "onConnectOk");
                    val(h.to_string())
                }
                Err(e) => {
                    self.obj_set(obj, "LastError", e);
                    self.obj_set(obj, "StatusCode", "1".into());
                    self.queue_control_event(obj, "onConnectError");
                    val("0".to_string())
                }
            },
            "EXECUTE" | "EXEC" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                match self.db.exec(h, &arg(0)) {
                    Ok(n) => {
                        self.queue_control_event(obj, "onQueryComplete");
                        val(n.to_string())
                    }
                    Err(e) => {
                        self.obj_set(obj, "LastError", e);
                        self.queue_control_event(obj, "onQueryError");
                        val("0".to_string())
                    }
                }
            }
            "QUERY" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                match self.db.exec(h, &arg(0)) {
                    Ok(_) => {
                        self.queue_control_event(obj, "onQueryComplete");
                        val(self.db.row_count(h).to_string())
                    }
                    Err(e) => {
                        self.obj_set(obj, "LastError", e);
                        self.queue_control_event(obj, "onQueryError");
                        val("0".to_string())
                    }
                }
            }
            "FETCH" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                let fetched = self.db.next_row(h);
                if fetched {
                    self.queue_control_event(obj, "onRowFetched");
                }
                val(if fetched { "1" } else { "0" }.to_string())
            }
            "FETCHALL" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                val(self.db.row_count(h).to_string())
            }
            // ── Property accessor (spec 010 R9) ──
            // A member that is not an explicit method is a **property**:
            //   `GET-<prop>` → get · `SET-<prop>` USING → set · bare `<prop>` →
            //   get with no USING arg, set with a USING arg. A numeric value is
            //   returned as a NUMBER so `IF C::Width > …` / arithmetic stay
            //   algebraic. Property names are case-insensitive (`obj_get/obj_set`).
            _ => {
                let getval = |s: String| -> CobolValue {
                    if !s.trim().is_empty() {
                        if let Some(num) = crate::value::parse_decimal(s.trim()) {
                            return CobolValue::Numeric(num);
                        }
                    }
                    let n = s.len();
                    CobolValue::from_str(&s, n)
                };
                if m.starts_with("GET-") {
                    getval(self.obj_get(obj, &method[4..]))
                } else if m.starts_with("SET-") {
                    self.obj_set(obj, &method[4..], arg(0));
                    none
                } else if args.is_empty() {
                    getval(self.obj_get(obj, method))
                } else {
                    self.obj_set(obj, method, arg(0));
                    none
                }
            }
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr, span: Span) -> Result<CobolValue, RuntimeError> {
        match expr {
            Expr::Literal(lit, _) => Ok(literal_to_value(lit)),

            // Member-access chain read as a value: `obj::Caption`,
            // `Grid::Rows(I)::Value`, `obj::Value::toUpperCase()` (spec 011).
            Expr::Member { .. } => self.eval_member(expr),

            Expr::Identifier(name, _) => {
                let key = self.env.resolve_name(name, &[]);
                // A 66-level RENAMES item synthesizes its value from the items
                // it regroups.
                if self.env.is_renames(&key) {
                    let s = self.env.renames_value(&key).unwrap_or_default();
                    let n = s.len();
                    return Ok(CobolValue::from_str(&s, n));
                }
                // An OBJECT REFERENCE item's slot holds the bridge HANDLE ID —
                // an internal number. Reading the item from COBOL must yield
                // the value behind it, or `SET Label-1::Caption TO
                // clicked-button` shows the id of the second item declared
                // ("2") forever, whatever the block computed. Types with no
                // scalar rendering fall through to the handle.
                if self.object_refs.contains_key(&key) {
                    if let Some(id) = self.env.get_i64(&key) {
                        if let Some(v) = self.rust_bridge.peek(id) {
                            return Ok(bridge_to_cobol(v));
                        }
                    }
                }
                Ok(self.env.get(&key).cloned().unwrap_or_else(|| {
                    tracing::debug!("Identifier '{key}' not found in environment — using 0");
                    CobolValue::from_i64(0)
                }))
            }

            Expr::Qualified { name, of, .. } => {
                let quals = collect_quals(of);
                let key = self.env.resolve_name(name, &quals);
                Ok(self
                    .env
                    .get(&key)
                    .cloned()
                    .unwrap_or(CobolValue::from_i64(0)))
            }

            Expr::Subscript {
                base,
                indices,
                span: s,
            } => {
                if let Expr::Member {
                    recv, member, args, ..
                } = base.as_ref()
                {
                    if member.to_ascii_uppercase() == "SPLIT" {
                        let text = self.eval_expr(recv, *s)?.as_display_string();
                        let sep = if let Some(first_arg) = args.first() {
                            self.eval_expr(first_arg, *s)?.as_display_string()
                        } else {
                            " ".to_string()
                        };
                        let splits: Vec<&str> = text.split(&sep).collect();

                        let idx = self.eval_indices(indices, *s);
                        if let Some(&i) = idx.first() {
                            let i_usize = (i - 1).max(0) as usize; // 1-based index to 0-based
                            let elem = splits
                                .get(i_usize)
                                .map(|&x| x.to_string())
                                .unwrap_or_default();
                            let len = elem.len();
                            return Ok(CobolValue::from_str(&elem, len));
                        }
                    }
                }

                // Table reference `t(i[,j…])` → the occurrence's storage slot.
                let base_name = self.expr_to_name(base);
                let idx = self.eval_indices(indices, *s);
                let key = crate::environment::subscript_key(&base_name, &idx);
                Ok(self
                    .env
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0)))
            }

            Expr::RefMod {
                base,
                start,
                length,
                span: s,
            } => {
                // Reference modification (sender): `base(start:[length])`.
                let text = self.eval_expr(base, *s)?.as_display_string();
                let bytes = text.as_bytes();
                let start_i = self.eval_expr(start, *s)?.as_i64().unwrap_or(1).max(1) as usize; // 1-based
                let begin = (start_i - 1).min(bytes.len());
                let len = match length {
                    Some(l) => self.eval_expr(l, *s)?.as_i64().unwrap_or(0).max(0) as usize,
                    None => bytes.len().saturating_sub(begin),
                };
                let end = (begin + len).min(bytes.len());
                let s = String::from_utf8_lossy(&bytes[begin..end]).into_owned();
                let n = s.len();
                Ok(CobolValue::from_str(&s, n))
            }

            Expr::FunctionCall {
                name,
                args,
                span: s,
            } => self.eval_function(name, args, *s),

            Expr::Arithmetic {
                op,
                lhs,
                rhs,
                span: s,
            } => {
                let l = self.eval_expr(lhs, *s)?;
                let r = self.eval_expr(rhs, *s)?;
                let result = match op {
                    ArithOp::Add => l.add_val(&r),
                    ArithOp::Sub => l.sub_val(&r),
                    ArithOp::Mul => l.mul_val(&r),
                    ArithOp::Div => l
                        .div_val(&r)
                        .ok_or(RuntimeError::DivisionByZero { span: *s })?,
                    // Exponentiation is inherently floating-point.
                    ArithOp::Pow => CobolValue::from_f64(l.as_f64().powf(r.as_f64())),
                    ArithOp::Concat => {
                        let l_str = l.as_display_string();
                        let r_str = r.as_display_string();
                        let res = format!("{}{}", l_str, r_str);
                        let len = res.len();
                        CobolValue::from_str(&res, len)
                    }
                };
                Ok(result)
            }

            Expr::Unary {
                op,
                operand,
                span: s,
            } => {
                let v = self.eval_expr(operand, *s)?;
                Ok(match op {
                    // 0 − v keeps exact decimals; Pos is a no-op.
                    UnaryOp::Neg => CobolValue::from_i64(0).sub_val(&v),
                    UnaryOp::Pos => v,
                })
            }
        }
    }

    // ── Intrinsic functions ───────────────────────────────────────────────────

    fn eval_function(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<CobolValue, RuntimeError> {
        match name.to_ascii_uppercase().as_str() {
            "LENGTH" => {
                let v = self.eval_expr(&args[0], span)?;
                let len = match &v {
                    CobolValue::String { bytes, .. } => bytes.len(),
                    _ => v.as_display_string().len(),
                };
                Ok(CobolValue::from_i64(len as i64))
            }
            "UPPER-CASE" => {
                let s = self
                    .eval_expr(&args[0], span)?
                    .as_display_string()
                    .to_ascii_uppercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "LOWER-CASE" => {
                let s = self
                    .eval_expr(&args[0], span)?
                    .as_display_string()
                    .to_ascii_lowercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "NUMVAL" | "NUMVAL-C" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let f: f64 = s
                    .trim()
                    .replace(',', "")
                    .replace('$', "")
                    .replace('£', "")
                    .parse()
                    .unwrap_or(0.0);
                Ok(CobolValue::from_f64(f))
            }
            "MAX" => {
                let vals = self.eval_args(args, span)?;
                let max = vals
                    .iter()
                    .map(|v| v.as_f64())
                    .fold(f64::NEG_INFINITY, f64::max);
                Ok(CobolValue::from_f64(max))
            }
            "MIN" => {
                let vals = self.eval_args(args, span)?;
                let min = vals
                    .iter()
                    .map(|v| v.as_f64())
                    .fold(f64::INFINITY, f64::min);
                Ok(CobolValue::from_f64(min))
            }
            "SQRT" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(v.sqrt()))
            }
            "MOD" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(CobolValue::from_f64(a - (a / b).floor() * b))
            }
            "REM" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(CobolValue::from_f64(a - (a / b).trunc() * b))
            }
            "ABS" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(v.abs()))
            }
            "INTEGER" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_i64(v.floor() as i64))
            }
            "INTEGER-PART" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_i64(v.trunc() as i64))
            }
            "RANDOM" => {
                // FUNCTION RANDOM [ ( seed ) ]
                // With a seed argument, (re)seed the generator and return the
                // first value of that sequence; with no argument, return the
                // next value of the current sequence (COBOL-85). Same seed →
                // same sequence (reproducible). For a fresh sequence each run,
                // seed from a varying value, e.g. `ACCEPT ws-time FROM TIME`
                // then `FUNCTION RANDOM(ws-time)`.
                if let Some(seed_expr) = args.first() {
                    let seed = self.eval_expr(seed_expr, span)?.as_i64().unwrap_or(0);
                    seed_random(seed as u64);
                }
                Ok(CobolValue::from_f64(pseudo_random()))
            }
            "CURRENT-DATE" => {
                let s = current_date_string();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "TRIM" => {
                let s = self
                    .eval_expr(&args[0], span)?
                    .as_display_string()
                    .trim()
                    .to_owned();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "REVERSE" => {
                let s: String = self
                    .eval_expr(&args[0], span)?
                    .as_display_string()
                    .chars()
                    .rev()
                    .collect();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "CONCATENATE" => {
                let vals = self.eval_args(args, span)?;
                let s: String = vals.iter().map(|v| v.as_display_string()).collect();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            // ── Character / ordinal ───────────────────────────────────────────
            "ORD" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let b = s.bytes().next().unwrap_or(0);
                Ok(CobolValue::from_i64(b as i64 + 1)) // 1-based ordinal
            }
            "CHAR" => {
                let n = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(1);
                let s = ((n.clamp(1, 256) - 1) as u8 as char).to_string();
                Ok(CobolValue::from_str(&s, 1))
            }
            "ORD-MAX" | "ORD-MIN" => {
                let vals = self.eval_args(args, span)?;
                let cmp = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal);
                let pick = if name.eq_ignore_ascii_case("ORD-MAX") {
                    vals.iter()
                        .enumerate()
                        .max_by(|a, b| cmp(a.1.as_f64(), b.1.as_f64()))
                } else {
                    vals.iter()
                        .enumerate()
                        .min_by(|a, b| cmp(a.1.as_f64(), b.1.as_f64()))
                };
                Ok(CobolValue::from_i64(
                    pick.map(|(i, _)| i as i64 + 1).unwrap_or(0),
                ))
            }
            // ── Statistics over the argument list ─────────────────────────────
            "SUM" => {
                let mut total = CobolValue::from_i64(0);
                for v in self.eval_args(args, span)? {
                    total = total.add_val(&v);
                }
                Ok(total)
            }
            "MEAN" => {
                let vals = self.eval_args(args, span)?;
                if vals.is_empty() {
                    return Ok(CobolValue::from_i64(0));
                }
                let s: f64 = vals.iter().map(|v| v.as_f64()).sum();
                Ok(CobolValue::from_f64(s / vals.len() as f64))
            }
            "MEDIAN" => {
                let mut xs: Vec<f64> = self
                    .eval_args(args, span)?
                    .iter()
                    .map(|v| v.as_f64())
                    .collect();
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let m = if xs.is_empty() {
                    0.0
                } else if xs.len() % 2 == 1 {
                    xs[xs.len() / 2]
                } else {
                    (xs[xs.len() / 2 - 1] + xs[xs.len() / 2]) / 2.0
                };
                Ok(CobolValue::from_f64(m))
            }
            "MIDRANGE" | "RANGE" => {
                let xs: Vec<f64> = self
                    .eval_args(args, span)?
                    .iter()
                    .map(|v| v.as_f64())
                    .collect();
                let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let r = if name.eq_ignore_ascii_case("RANGE") {
                    hi - lo
                } else {
                    (lo + hi) / 2.0
                };
                Ok(CobolValue::from_f64(r))
            }
            "VARIANCE" | "STANDARD-DEVIATION" => {
                let xs: Vec<f64> = self
                    .eval_args(args, span)?
                    .iter()
                    .map(|v| v.as_f64())
                    .collect();
                if xs.is_empty() {
                    return Ok(CobolValue::from_i64(0));
                }
                let mean = xs.iter().sum::<f64>() / xs.len() as f64;
                let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
                Ok(CobolValue::from_f64(
                    if name.eq_ignore_ascii_case("VARIANCE") {
                        var
                    } else {
                        var.sqrt()
                    },
                ))
            }
            // ── Math ──────────────────────────────────────────────────────────
            "FACTORIAL" => {
                let n = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0).max(0);
                let mut f: i128 = 1;
                for k in 2..=n as i128 {
                    f *= k;
                }
                Ok(CobolValue::from_i64(f as i64))
            }
            "SIN" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().sin(),
            )),
            "COS" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().cos(),
            )),
            "TAN" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().tan(),
            )),
            "ASIN" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().asin(),
            )),
            "ACOS" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().acos(),
            )),
            "ATAN" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().atan(),
            )),
            "LOG" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().ln(),
            )),
            "LOG10" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().log10(),
            )),
            "EXP" => Ok(CobolValue::from_f64(
                self.eval_expr(&args[0], span)?.as_f64().exp(),
            )),
            "EXP10" => Ok(CobolValue::from_f64(
                10f64.powf(self.eval_expr(&args[0], span)?.as_f64()),
            )),
            "PI" => Ok(CobolValue::from_f64(std::f64::consts::PI)),
            "STORED-CHAR-LENGTH" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                Ok(CobolValue::from_i64(s.trim_end().len() as i64))
            }
            "WHEN-COMPILED" => {
                let s = current_date_string();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            // ── Date / day conversions (standard base: 1601-01-01 = day 1) ──
            "INTEGER-OF-DATE" => {
                let yyyymmdd = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0);
                Ok(CobolValue::from_i64(integer_of_date(yyyymmdd)))
            }
            "DATE-OF-INTEGER" => {
                let n = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0);
                Ok(CobolValue::from_i64(date_of_integer(n)))
            }
            "INTEGER-OF-DAY" => {
                let yyyyddd = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0);
                Ok(CobolValue::from_i64(integer_of_day(yyyyddd)))
            }
            "DAY-OF-INTEGER" => {
                let n = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0);
                Ok(CobolValue::from_i64(day_of_integer(n)))
            }
            "FRACTION-PART" => {
                let x = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(x - x.trunc()))
            }
            "ANNUITY" => {
                // Ratio of one payment to the present value of a series of `n`
                // payments at interest `rate`: rate / (1 − (1+rate)^−n).
                let rate = self.eval_expr(&args[0], span)?.as_f64();
                let n = self.eval_expr(&args[1], span)?.as_f64();
                let v = if rate == 0.0 {
                    if n == 0.0 {
                        0.0
                    } else {
                        1.0 / n
                    }
                } else {
                    rate / (1.0 - (1.0 + rate).powf(-n))
                };
                Ok(CobolValue::from_f64(v))
            }
            "PRESENT-VALUE" => {
                // PRESENT-VALUE(rate, amt1 [amt2 …]) = Σ amt_i / (1+rate)^i.
                let rate = self.eval_expr(&args[0], span)?.as_f64();
                let mut total = 0.0;
                for (i, a) in args.iter().skip(1).enumerate() {
                    let amt = self.eval_expr(a, span)?.as_f64();
                    total += amt / (1.0 + rate).powi(i as i32 + 1);
                }
                Ok(CobolValue::from_f64(total))
            }
            "YEAR-TO-YYYY" => {
                // Expand a 2-digit year using a sliding window (default 50).
                let yy = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0);
                let window = if args.len() > 1 {
                    self.eval_expr(&args[1], span)?.as_i64().unwrap_or(50)
                } else {
                    50
                };
                let cur_year = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let days = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        / 86400;
                    ymd_from_days(days).0 as i64
                };
                let max_year = cur_year + window;
                let mut yyyy = (max_year / 100) * 100 + yy;
                if yyyy > max_year {
                    yyyy -= 100;
                }
                Ok(CobolValue::from_i64(yyyy))
            }
            "BYTE-LENGTH" | "LENGTH-AN" => {
                let v = self.eval_expr(&args[0], span)?;
                let len = match &v {
                    CobolValue::String { bytes, .. } => bytes.len(),
                    _ => v.as_display_string().len(),
                };
                Ok(CobolValue::from_i64(len as i64))
            }
            "NUMVAL-F" => {
                // Like NUMVAL but honours an exponent (`1.5E3`).
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let f: f64 = s.trim().replace(['+', ' '], "").parse().unwrap_or(0.0);
                Ok(CobolValue::from_f64(f))
            }
            "TEST-NUMVAL" => {
                // 0 if the string is a valid NUMVAL argument, else the 1-based
                // position of the first offending character.
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let t = s.trim();
                let ok = !t.is_empty()
                    && t.chars()
                        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | ',' | ' '));
                if ok && t.parse::<f64>().is_ok() {
                    Ok(CobolValue::from_i64(0))
                } else {
                    let pos = t
                        .chars()
                        .position(|c| {
                            !(c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | ',' | ' '))
                        })
                        .map(|p| p as i64 + 1)
                        .unwrap_or(t.len() as i64 + 1);
                    Ok(CobolValue::from_i64(pos))
                }
            }
            _ => {
                tracing::warn!("Unknown intrinsic function '{}' — returning 0", name);
                Ok(CobolValue::from_i64(0))
            }
        }
    }

    fn eval_args(&mut self, args: &[Expr], span: Span) -> Result<Vec<CobolValue>, RuntimeError> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.eval_expr(a, span)?);
        }
        Ok(out)
    }

    // ── Condition evaluation ──────────────────────────────────────────────────

    /// Evaluate a boolean condition.
    pub fn eval_condition(&mut self, cond: &Condition) -> Result<bool, RuntimeError> {
        match cond {
            Condition::Comparison { lhs, op, rhs, span } => {
                let l = self.eval_expr(lhs, *span)?;
                let r = self.eval_expr(rhs, *span)?;
                Ok(compare_values(&l, &r, *op))
            }
            Condition::Not(inner, _) => Ok(!self.eval_condition(inner)?),
            Condition::And(a, b, _) => Ok(self.eval_condition(a)? && self.eval_condition(b)?),
            Condition::Or(a, b, _) => Ok(self.eval_condition(a)? || self.eval_condition(b)?),
            Condition::ClassTest {
                expr,
                negated,
                class,
                span,
            } => {
                let v = self.eval_expr(expr, *span)?;
                let s = v.as_display_string();
                let result = match class {
                    DataClass::Numeric => v.is_numeric() || s.trim().parse::<f64>().is_ok(),
                    DataClass::Alphabetic => s.chars().all(|c| c.is_ascii_alphabetic() || c == ' '),
                    DataClass::AlphabeticLower => {
                        s.chars().all(|c| c.is_ascii_lowercase() || c == ' ')
                    }
                    DataClass::AlphabeticUpper => {
                        s.chars().all(|c| c.is_ascii_uppercase() || c == ' ')
                    }
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::SignTest {
                expr,
                negated,
                sign,
                span,
            } => {
                let v = self.eval_expr(expr, *span)?.as_f64();
                let result = match sign {
                    SignCond::Positive => v > 0.0,
                    SignCond::Negative => v < 0.0,
                    SignCond::Zero => v == 0.0,
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::ConditionName(name, _) => {
                use cobolt_ast::data::ConditionValue;
                // 88-level condition-name: true when the parent (host) item holds
                // one of the declared VALUEs (or falls within a THRU range).
                if let Some(info) = self.env.cond_name(name).cloned() {
                    let pv = self
                        .env
                        .get(&info.parent)
                        .cloned()
                        .unwrap_or_else(|| CobolValue::from_i64(0));
                    for cv in &info.values {
                        let hit = match cv {
                            ConditionValue::Single(lit) => {
                                compare_values(&pv, &literal_to_value(lit), CmpOp::Eq)
                            }
                            ConditionValue::Range(lo, hi) => {
                                compare_values(&pv, &literal_to_value(lo), CmpOp::Ge)
                                    && compare_values(&pv, &literal_to_value(hi), CmpOp::Le)
                            }
                        };
                        if hit {
                            return Ok(true);
                        }
                    }
                    return Ok(false);
                }
                // Fallback (undeclared): truthy if the slot is non-zero/non-space.
                let upper = name.to_ascii_uppercase();
                let v = self
                    .env
                    .get(&upper)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                Ok(!v.is_zero())
            }

            Condition::NameOrAbbrev {
                subject,
                op,
                name,
                span,
            } => {
                // `a = b OR c`: if `c` is a known 88-level condition-name, treat
                // it as one; otherwise it is the abbreviation object `a = c`.
                if self.env.cond_name(name).is_some() {
                    return self.eval_condition(&Condition::ConditionName(name.clone(), *span));
                }
                let l = self.eval_expr(subject, *span)?;
                let key = self.env.resolve_name(name, &[]);
                let r = self
                    .env
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                Ok(compare_values(&l, &r, *op))
            }
        }
    }

    // ── Paragraph / section helpers ───────────────────────────────────────────

    fn para_stmts(&self, name: &str, span: Span) -> Result<Vec<Stmt>, RuntimeError> {
        let upper = name.to_ascii_uppercase();
        self.para_map
            .get(&upper)
            .cloned()
            .ok_or(RuntimeError::UndefinedParagraph { name: upper, span })
    }

    /// Collect all paragraphs that belong to a section (identified by
    /// consecutive paragraphs after the named entry in `para_order`).
    fn collect_section_stmts(&self, section_upper: &str) -> Vec<Stmt> {
        let mut found = false;
        let mut result = Vec::new();
        for name in &self.para_order {
            if name == section_upper {
                found = true;
                continue;
            }
            if found {
                // Stop at the next section marker (paragraphs inside a section
                // are typically named SECTION-name-PARAGRAPH-name or just
                // listed consecutively; we collect until end for simplicity).
                if let Some(stmts) = self.para_map.get(name) {
                    result.extend_from_slice(stmts);
                }
            }
        }
        result
    }

    fn thru_stmts(&self, from: &str, to: &str, span: Span) -> Result<Vec<Stmt>, RuntimeError> {
        let from_u = from.to_ascii_uppercase();
        let to_u = to.to_ascii_uppercase();
        let from_pos = self
            .para_order
            .iter()
            .position(|n| n == &from_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph {
                name: from_u.clone(),
                span,
            })?;
        let to_pos = self
            .para_order
            .iter()
            .position(|n| n == &to_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph {
                name: to_u.clone(),
                span,
            })?;

        let mut stmts = Vec::new();
        for i in from_pos..=to_pos {
            if let Some(ps) = self.para_map.get(&self.para_order[i]) {
                stmts.extend_from_slice(ps);
            }
        }
        Ok(stmts)
    }

    /// Evaluate a list of subscript index expressions to 1-based integers.
    fn eval_indices(&mut self, indices: &[Expr], span: Span) -> Vec<i64> {
        indices
            .iter()
            .map(|e| {
                self.eval_expr(e, span)
                    .ok()
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1)
            })
            .collect()
    }

    /// Resolve an assignment target to its storage key, evaluating subscripts.
    /// (`RefMod` targets are handled separately by `assign_refmod`.)
    fn resolve_lvalue(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Subscript {
                base,
                indices,
                span,
            } => {
                let base_name = self.expr_to_name(base);
                let idx = self.eval_indices(indices, *span);
                crate::environment::subscript_key(&base_name, &idx)
            }
            // A member-access receiver (`ctrl::prop`, `Grid::Rows(I)::Value`) is
            // backed by a synthetic env item seeded with the property's current
            // value; `flush_property_shadows` writes it back to the place after
            // the statement, so *any* verb that resolves its receiving field
            // through here gains property-receiver support (spec 011). A
            // method-call tail is not a receiving field — return a throwaway key.
            Expr::Member { .. } => {
                if let Ok((root, Resolved::Path(path))) = self.resolve_member(expr) {
                    let synth = format!("__PROP${root}${}", path_display(&path));
                    let cur = self
                        .objects
                        .get_path(&root, &path)
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let trimmed = cur.trim();
                    // Seed the synthetic item by the value's shape: a numeric item
                    // for a number (so `ADD … TO ctrl::Value` / `COMPUTE` stay
                    // arithmetic), otherwise a roomy alphanumeric item (so
                    // `STRING/UNSTRING INTO ctrl::Text` has space to write).
                    match crate::value::parse_decimal(trimmed) {
                        Some(num) if !trimmed.is_empty() => {
                            self.env.set(&synth, CobolValue::Numeric(num));
                        }
                        _ => {
                            let cap = trimmed.len().max(4096);
                            self.env.set(&synth, CobolValue::from_str(trimmed, cap));
                        }
                    }
                    self.property_shadows.insert(synth.clone(), (root, path));
                    synth
                } else {
                    "__UNKNOWN__".to_owned()
                }
            }
            _ => self.expr_to_name(expr),
        }
    }

    /// Write any property "shadows" back to their controls (called after every
    /// statement). See [`Self::resolve_lvalue`].
    fn flush_property_shadows(&mut self) {
        if self.property_shadows.is_empty() {
            return;
        }
        let shadows: Vec<(String, (String, Vec<PathSeg>))> =
            self.property_shadows.drain().collect();
        for (synth, (ctrl, path)) in shadows {
            let v = self
                .env
                .get(&synth)
                .map(|cv| cv.as_display_string())
                .unwrap_or_default()
                .trim()
                .to_owned();
            self.set_member(&ctrl, &path, v);
        }
    }

    /// Extract the canonical storage key for an lvalue expression, resolving
    /// any `OF`/`IN` qualification to disambiguate duplicated names.
    fn expr_to_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name, _) => self.env.resolve_name(name, &[]),
            Expr::Qualified { name, of, .. } => {
                let quals = collect_quals(of);
                self.env.resolve_name(name, &quals)
            }
            Expr::Subscript { base, .. } => self.expr_to_name(base),
            Expr::RefMod { base, .. } => self.expr_to_name(base),
            _ => "__UNKNOWN__".to_owned(),
        }
    }

    /// Resolve one `STRING` sending operand to `(characters, is_plain_alphanumeric)`.
    ///
    /// A **data-item** reference is rendered in its *field* form, exactly as the
    /// item's characters are stored: a USAGE-DISPLAY numeric item shows its full
    /// PIC-width digit string (leading zeros, leading `-` when negative), a
    /// numeric-edited item shows its edited characters, and an alphanumeric item
    /// shows its bytes. Literals, function results and computed expressions use
    /// their evaluated value.
    ///
    /// The returned flag is `true` only for a *plain alphanumeric* item, which
    /// is what drives the default `DELIMITED BY SPACES` behaviour (trailing
    /// space padding dropped) when no `DELIMITED BY` clause is written. Every
    /// other operand defaults to `DELIMITED BY SIZE`.
    fn string_operand(&mut self, e: &Expr, span: Span) -> Result<(String, bool), RuntimeError> {
        if matches!(
            e,
            Expr::Identifier(..) | Expr::Qualified { .. } | Expr::Subscript { .. }
        ) {
            let name = self.resolve_lvalue(e);
            if let Some(chars) = self.env.display_string(&name) {
                let is_alpha = self.env.is_alphanumeric_field(&name);
                return Ok((chars, is_alpha));
            }
        }
        Ok((self.eval_expr(e, span)?.as_display_string(), false))
    }

    /// Assign `val` into the reference-modified region of a target:
    /// `base(start:[length])` — splice `val` (space-padded / truncated to the
    /// region width) into the base field's bytes.
    fn assign_refmod(
        &mut self,
        base: &Expr,
        start: &Expr,
        length: Option<&Expr>,
        val: &CobolValue,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let name = self.expr_to_name(base);
        let mut cur = self
            .env
            .display_string(&name)
            .unwrap_or_default()
            .into_bytes();
        let start_i = self.eval_expr(start, span)?.as_i64().unwrap_or(1).max(1) as usize; // 1-based
        let begin = (start_i - 1).min(cur.len());
        let region = match length {
            Some(l) => self.eval_expr(l, span)?.as_i64().unwrap_or(0).max(0) as usize,
            None => cur.len().saturating_sub(begin),
        };
        let end = (begin + region).min(cur.len());
        let mut repl = val.as_display_string().into_bytes();
        repl.resize(end - begin, b' '); // pad/truncate to the region width
        cur.splice(begin..end, repl);
        self.env.set_str(&name, &String::from_utf8_lossy(&cur));
        Ok(())
    }
}

// ── DATA DIVISION lookup (for INITIALIZE) ─────────────────────────────────────

/// Find the declaration of `name` anywhere in the WORKING-STORAGE / LOCAL-STORAGE
/// / LINKAGE sections (recursing into group items).
fn find_decl_in_division<'a>(
    div: &'a cobolt_ast::program::DataDivision,
    name: &str,
) -> Option<&'a cobolt_ast::data::DataDecl> {
    use cobolt_ast::program::DataSection;
    for sec in &div.sections {
        let decls = match sec {
            DataSection::WorkingStorage(d)
            | DataSection::LocalStorage(d)
            | DataSection::Linkage(d) => d,
            _ => continue,
        };
        for d in decls {
            if let Some(found) = find_decl(d, name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_decl<'a>(
    d: &'a cobolt_ast::data::DataDecl,
    name: &str,
) -> Option<&'a cobolt_ast::data::DataDecl> {
    if d.name
        .as_deref()
        .map(|n| n.eq_ignore_ascii_case(name))
        .unwrap_or(false)
    {
        return Some(d);
    }
    for c in &d.children {
        if let Some(f) = find_decl(c, name) {
            return Some(f);
        }
    }
    None
}

// ── Debugger span extractor ───────────────────────────────────────────────────

/// Return the source span of a statement as `Some(span)`.
///
/// Delegates to `Stmt::span()` which covers every variant.
#[inline]
fn stmt_span(stmt: &Stmt) -> Option<Span> {
    Some(stmt.span())
}

// ── Paragraph map builder ─────────────────────────────────────────────────────

fn build_para_map(body: &ProcedureBody) -> (IndexMap<String, Vec<Stmt>>, Vec<String>) {
    let mut map: IndexMap<String, Vec<Stmt>> = IndexMap::new();
    let mut order: Vec<String> = Vec::new();

    match body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                let key = para.name.to_ascii_uppercase();
                order.push(key.clone());
                map.insert(key, para.stmts.clone());
            }
        }
        ProcedureBody::Sections(sections) => {
            for section in sections {
                // Optionally register the section name itself as an entry.
                let sec_key = section.name.to_ascii_uppercase();
                order.push(sec_key.clone());
                map.insert(sec_key, Vec::new()); // empty placeholder
                for para in &section.paragraphs {
                    let key = para.name.to_ascii_uppercase();
                    order.push(key.clone());
                    map.insert(key, para.stmts.clone());
                }
            }
        }
    }
    (map, order)
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Convert an AST literal to a runtime `CobolValue`.
pub fn literal_to_value(lit: &Literal) -> CobolValue {
    match lit {
        Literal::Integer(n) => CobolValue::from_i64(*n),
        Literal::Float(f) => CobolValue::from_f64(*f),
        Literal::Decimal(m, s) => CobolValue::Numeric(CobolNumeric::new(*m, *s)),
        Literal::String(s) => CobolValue::from_str(s, s.len()),
        Literal::Figurative(fig) => match fig {
            FigurativeConstant::Zero => CobolValue::from_i64(0),
            FigurativeConstant::Space => CobolValue::spaces(1),
            FigurativeConstant::HighValue => CobolValue::figurative_high_values(1),
            FigurativeConstant::LowValue => CobolValue::figurative_low_values(1),
            FigurativeConstant::Quote => CobolValue::from_str("\"", 1),
            FigurativeConstant::Null => CobolValue::from_i64(0),
            FigurativeConstant::All(inner) => literal_to_value(inner),
        },
    }
}

/// Flatten an `AND`-chain of equality comparisons into `(lhs, rhs, span)`
/// tuples in major-to-minor (textual) order. Used by the `SEARCH ALL` binary
/// search to find the discriminating key when a WHEN does not match at `mid`.
fn flatten_eq_comparisons<'a>(cond: &'a Condition, out: &mut Vec<(&'a Expr, &'a Expr, Span)>) {
    match cond {
        Condition::And(a, b, _) => {
            flatten_eq_comparisons(a, out);
            flatten_eq_comparisons(b, out);
        }
        Condition::Comparison { lhs, rhs, span, .. } => out.push((lhs, rhs, *span)),
        _ => {}
    }
}

/// Compare two `CobolValue`s using the given operator.
/// Total ordering of two COBOL values, derived from `compare_values`, for SORT.
fn cob_ordering(a: &CobolValue, b: &CobolValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if compare_values(a, b, CmpOp::Eq) {
        Ordering::Equal
    } else if compare_values(a, b, CmpOp::Lt) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

pub fn compare_values(l: &CobolValue, r: &CobolValue, op: CmpOp) -> bool {
    // Numeric comparison when both sides are numeric.
    if l.is_numeric() && r.is_numeric() {
        // Exact integer comparison when both are fixed-point decimals.
        if let (Some(a), Some(b)) = (l.as_exact(), r.as_exact()) {
            use std::cmp::Ordering;
            let ord = a.cmp(&b);
            return match op {
                CmpOp::Eq => ord == Ordering::Equal,
                CmpOp::Ne => ord != Ordering::Equal,
                CmpOp::Lt => ord == Ordering::Less,
                CmpOp::Le => ord != Ordering::Greater,
                CmpOp::Gt => ord == Ordering::Greater,
                CmpOp::Ge => ord != Ordering::Less,
            };
        }
        let lf = l.as_f64();
        let rf = r.as_f64();
        return match op {
            CmpOp::Eq => (lf - rf).abs() < 1e-10,
            CmpOp::Ne => (lf - rf).abs() >= 1e-10,
            CmpOp::Lt => lf < rf,
            CmpOp::Le => lf <= rf,
            CmpOp::Gt => lf > rf,
            CmpOp::Ge => lf >= rf,
        };
    }
    // Cross-type: numeric vs string — compare as f64 if parsable, else string.
    if l.is_numeric() || r.is_numeric() {
        let lf = l.as_f64();
        let rf = r.as_f64();
        return match op {
            CmpOp::Eq => (lf - rf).abs() < 1e-10,
            CmpOp::Ne => (lf - rf).abs() >= 1e-10,
            CmpOp::Lt => lf < rf,
            CmpOp::Le => lf <= rf,
            CmpOp::Gt => lf > rf,
            CmpOp::Ge => lf >= rf,
        };
    }
    // Alphanumeric comparison. Per COBOL rules the shorter operand is padded on
    // the RIGHT with spaces to the length of the longer one, then compared
    // byte-by-byte. This makes e.g. `"BTN-OK"` (a literal) equal to a `PIC X(64)`
    // field holding "BTN-OK" followed by trailing spaces.
    let ls = l.as_display_string();
    let rs = r.as_display_string();
    let width = ls.len().max(rs.len());
    let lp = format!("{ls:<width$}");
    let rp = format!("{rs:<width$}");
    match op {
        CmpOp::Eq => lp == rp,
        CmpOp::Ne => lp != rp,
        CmpOp::Lt => lp < rp,
        CmpOp::Le => lp <= rp,
        CmpOp::Gt => lp > rp,
        CmpOp::Ge => lp >= rp,
    }
}

/// Extract the expression from a `CallArg`.
fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
    }
}

/// Parse the raw bytes of a chart data table into `(label, value)` points, using
/// the standard row layout the codegen emits: a `PIC X(64)` label followed by a
/// `PIC 9(18)V9(6)` value (24 DISPLAY digits, 6 implied decimals). Best-effort —
/// rows that don't fit the fixed stride are skipped. Handlers that build a table
/// with a different layout should prefer `COBOL-CHART-ADD-POINT`, which is
/// layout-independent.
fn parse_chart_table(raw: &str, count: usize) -> Vec<(String, f64)> {
    const LABEL_LEN: usize = 64;
    const VAL_DIGITS: usize = 24; // 9(18)V9(6)
    const VAL_SCALE: f64 = 1_000_000.0; // V9(6)
    const ROW: usize = LABEL_LEN + VAL_DIGITS;
    let chars: Vec<char> = raw.chars().collect();
    let mut out = Vec::new();
    for i in 0..count {
        let start = i * ROW;
        if start + ROW > chars.len() {
            break;
        }
        let label: String = chars[start..start + LABEL_LEN].iter().collect();
        let digits: String = chars[start + LABEL_LEN..start + ROW]
            .iter()
            .filter(|c| c.is_ascii_digit())
            .collect();
        let value = digits.parse::<f64>().unwrap_or(0.0) / VAL_SCALE;
        out.push((label.trim().to_owned(), value));
    }
    out
}

/// ANSI SGR prefix for a screen phrase's display attributes (`""` if none).
fn screen_attrs(sc: &cobolt_ast::stmt::ScreenPhrase) -> String {
    let mut s = String::new();
    if sc.highlight {
        s.push_str("\x1b[1m");
    }
    if sc.reverse {
        s.push_str("\x1b[7m");
    }
    if sc.underline {
        s.push_str("\x1b[4m");
    }
    s
}

/// Flatten the `OF`/`IN` qualifier chain of a [`Expr::Qualified`] `of` operand
/// into an innermost-first list of qualifier names: `A OF B OF C` → `[B, C]`.
fn collect_quals(of: &Expr) -> Vec<String> {
    match of {
        Expr::Identifier(n, _) => vec![n.to_ascii_uppercase()],
        Expr::Qualified { name, of, .. } => {
            let mut v = vec![name.to_ascii_uppercase()];
            v.extend(collect_quals(of));
            v
        }
        Expr::Subscript { base, .. } => collect_quals(base),
        _ => Vec::new(),
    }
}

// ── Member-access chain support (spec 011) ──────────────────────────────────

/// One lowered segment of an [`Expr::Member`] chain (arguments evaluated).
struct MemberSeg {
    member: String,
    parens: bool,
    args: Vec<CobolValue>,
}

/// The result of resolving a member chain: an addressable place (property or
/// collection element) or a method call on a place.
enum Resolved {
    /// A readable / assignable place — a property or an indexed element.
    Path(Vec<PathSeg>),
    /// A method call on the place reached by `path` (rvalue only).
    Method {
        path: Vec<PathSeg>,
        method: String,
        args: Vec<CobolValue>,
    },
}

/// `true` if `name` is a recognised control/collection **method** (so a parens
/// segment is a call rather than a collection subscript). Methods are a closed
/// vocabulary; property and collection names are open — hence the asymmetry. Kept
/// in step with [`Interpreter::exec_method`] and [`Interpreter::exec_member_method`].
/// A `GET-`/`SET-` prefix is always a method (explicit accessor, spec 010).
fn is_known_method(name: &str) -> bool {
    let m = name.to_ascii_uppercase();
    if m.starts_with("GET-") || m.starts_with("SET-") {
        return true;
    }
    matches!(
        m.as_str(),
        // Universal lifecycle / visibility / geometry
        "SHOW" | "HIDE" | "ENABLE" | "DISABLE" | "SETFOCUS" | "FOCUS"
            | "BRINGTOFRONT" | "SENDTOBACK" | "REFRESH" | "VALIDATE"
            | "MOVETO" | "RESIZE"
        // Generic / text / caption
            | "SETPROPERTY" | "GETPROPERTY" | "SETCAPTION" | "SETTEXT"
            | "GETCAPTION" | "GETTEXT" | "APPENDTEXT" | "SETCOLOR" | "SELECTALL"
            | "CLEAR"
        // Checkbox / radio
            | "ISCHECKED" | "SETCHECKED" | "SELECT" | "TOGGLE"
        // Numeric value
            | "SETVALUE" | "GETVALUE" | "INCREMENT" | "DECREMENT" | "RESET"
        // Items / list / combo
            | "ADDITEM" | "REMOVEITEM" | "GETSELECTED" | "GETSELECTEDINDEX"
            | "GETINDEX" | "SETSELECTEDINDEX" | "SETINDEX" | "GETCOUNT"
        // DataGrid
            | "GETROWCOUNT" | "GETCELLVALUE" | "SETCELLVALUE" | "ADDROW"
            | "DELETEROW" | "CLEARROWS" | "SORT" | "SETFILTER" | "CLEARFILTERS"
            | "FREEZECOLUMNS" | "FREEZEROWS" | "SETROWHEIGHT" | "SETCOLUMNWIDTH"
            | "GETSELECTEDTEXT" | "COPYSELECTION" | "EXPORTCSV"
        // Databound controls (DataGrid + repeating GroupBox/ControlArray)
            | "REFRESHBINDING"
        // Charts (AddPoint appends one label/value point; Clear/Refresh above)
            | "ADDPOINT" | "ADD-POINT"
        // Timer / animation
            | "START" | "STOP" | "SETINTERVAL" | "ISENABLED"
            | "PLAYANIMATION" | "PLAY" | "STOPANIMATION" | "PAUSE"
        // Agent / window / SQL / HTTP
            | "CLOSE" | "GETRESULT" | "SETTITLE" | "SETPROMPT" | "SETMODEL"
            | "ASK" | "GET" | "POST" | "PUT" | "DELETE" | "CALL" | "SETHEADER"
            | "CLEARHEADERS" | "SETTIMEOUT" | "OPEN" | "EXECUTE" | "EXEC"
            | "QUERY" | "FETCH" | "FETCHALL"
        // Collection verbs + scalar transforms (exec_member_method)
            | "COUNT" | "SIZE" | "REMOVE" | "ADD" | "APPEND"
            | "TOUPPERCASE" | "UPPERCASE" | "UPPER"
            | "TOLOWERCASE" | "LOWERCASE" | "LOWER" | "TRIM" | "LEN" | "LENGTH"
    )
}

/// Convert a stored [`PropertyValue`] to a `CobolValue`, parsing a numeric-looking
/// scalar to a `Numeric` so comparisons / arithmetic stay algebraic.
fn prop_to_value(pv: Option<PropertyValue>) -> CobolValue {
    let s = pv.map(|v| v.to_string()).unwrap_or_default();
    if !s.trim().is_empty() {
        if let Some(num) = crate::value::parse_decimal(s.trim()) {
            return CobolValue::Numeric(num);
        }
    }
    let n = s.len();
    CobolValue::from_str(&s, n)
}

/// Render a member path as a human-readable key for a `StateUpdate` on a nested
/// place, e.g. `[Prop(Rows), Index(2), Prop(Value)]` → `"Rows(2).Value"`.
/// The bare property name of a single-`Prop` path (`[Prop(X)]`), else `None`
/// (049 — the shape `super` exposes: form properties only).
fn single_prop_key(path: &[PathSeg]) -> Option<String> {
    match path {
        [PathSeg::Prop(k)] => Some(k.clone()),
        _ => None,
    }
}

fn path_display(path: &[PathSeg]) -> String {
    let mut out = String::new();
    for seg in path {
        match seg {
            PathSeg::Prop(name) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(name);
            }
            PathSeg::Index(i) => {
                out.push('(');
                out.push_str(&i.to_string());
                out.push(')');
            }
        }
    }
    out
}

// ── Date / time utilities (no external crate dependency) ─────────────────────

/// Return the current date as `YYYYMMDD` (8 chars).
fn runtime_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    days_to_ymd(days)
}

/// Return the current time as `HHMMSScc` (8 chars, cc = centiseconds).
fn runtime_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // COBOL-85 TIME is HHMMSSss where `ss` is HUNDREDTHS of a second
    // (centiseconds, 00–99) — the standard's finest resolution, not
    // milliseconds. Populate it for real so `ACCEPT … FROM TIME` (and the
    // time portion of FUNCTION CURRENT-DATE) varies sub-second.
    let cs = now.subsec_nanos() / 10_000_000; // ns → centiseconds, 0–99
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}{m:02}{s:02}{cs:02}")
}

/// Days in `month` (1–12) of `year`, accounting for leap years.
fn cob_days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year.max(0) as u64) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// COBOL `INTEGER-OF-DATE(yyyymmdd)`: days since the base 1600-12-31
/// (so 1601-01-01 → 1).
fn integer_of_date(yyyymmdd: i64) -> i64 {
    let (y, m, d) = (yyyymmdd / 10000, (yyyymmdd / 100) % 100, yyyymmdd % 100);
    let mut days = 0i64;
    for yy in 1601..y {
        days += if is_leap(yy as u64) { 366 } else { 365 };
    }
    for mm in 1..m {
        days += cob_days_in_month(y, mm);
    }
    days + d
}

/// COBOL `DATE-OF-INTEGER(n)` → `yyyymmdd` (inverse of `integer_of_date`).
fn date_of_integer(n: i64) -> i64 {
    let mut rem = n;
    let mut y = 1601i64;
    loop {
        let dy = if is_leap(y as u64) { 366 } else { 365 };
        if rem > dy {
            rem -= dy;
            y += 1;
        } else {
            break;
        }
    }
    let mut m = 1i64;
    loop {
        let dm = cob_days_in_month(y, m);
        if rem > dm {
            rem -= dm;
            m += 1;
        } else {
            break;
        }
    }
    y * 10000 + m * 100 + rem
}

/// COBOL `INTEGER-OF-DAY(yyyyddd)`: days since 1600-12-31 from a Julian date.
fn integer_of_day(yyyyddd: i64) -> i64 {
    let (y, ddd) = (yyyyddd / 1000, yyyyddd % 1000);
    let mut days = 0i64;
    for yy in 1601..y {
        days += if is_leap(yy as u64) { 366 } else { 365 };
    }
    days + ddd
}

/// COBOL `DAY-OF-INTEGER(n)` → `yyyyddd` (inverse of `integer_of_day`).
fn day_of_integer(n: i64) -> i64 {
    let mut rem = n;
    let mut y = 1601i64;
    loop {
        let dy = if is_leap(y as u64) { 366 } else { 365 };
        if rem > dy {
            rem -= dy;
            y += 1;
        } else {
            break;
        }
    }
    y * 1000 + rem
}

/// Return Julian day as `YYDDD` (5 chars).
fn runtime_julian_day() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    let (y, _, _) = ymd_from_days(days);
    let jan1 = days_since_epoch_jan1(y);
    let doy = days - jan1 + 1;
    format!("{:02}{:03}", y % 100, doy)
}

/// Return day-of-week as `i64`: 1 = Monday … 7 = Sunday (ISO 8601).
fn runtime_day_of_week() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    // 1970-01-01 was a Thursday → day 4
    ((days + 3) % 7 + 1) as i64
}

/// Return a 21-char CURRENT-DATE string: `YYYYMMDDHHMMSSCC+HHMM`.
fn current_date_string() -> String {
    format!("{}{}-0000", runtime_date(), runtime_time())
}

// ── Simple calendar arithmetic (no external crate) ────────────────────────────

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_year(year: u64) -> u64 {
    if is_leap(year) {
        366
    } else {
        365
    }
}

/// Days since Unix epoch for January 1 of `year`.
fn days_since_epoch_jan1(year: u64) -> u64 {
    let mut d = 0u64;
    let mut y = 1970u64;
    while y < year {
        d += days_in_year(y);
        y += 1;
    }
    d
}

/// Convert days-since-epoch to (year, month, day).
fn ymd_from_days(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = days_in_year(year);
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn days_to_ymd(days: u64) -> String {
    let (y, m, d) = ymd_from_days(days);
    format!("{y:04}{m:02}{d:02}")
}

// ── Pseudo-random number generator ───────────────────────────────────────────

static RANDOM_STATE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(6364136223846793005);

/// One step of the PCG-style LCG: returns `(next_state, value)` where `value`
/// is in `[0, 1)`. Pure — no shared state — so it is deterministically testable.
fn lcg_step(state: u64) -> (u64, f64) {
    let next = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    // Top 53 bits → double in [0, 1)
    (next, (next >> 11) as f64 / (1u64 << 53) as f64)
}

/// Turn a user seed into an LCG state, mixing once so that adjacent small seeds
/// (1, 2, 3 …) yield well-separated sequences from the very first value.
fn mix_seed(seed: u64) -> u64 {
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

/// (Re)seed the generator. `FUNCTION RANDOM(seed)` calls this; the same seed
/// always reproduces the same sequence (COBOL-85 semantics).
fn seed_random(seed: u64) {
    RANDOM_STATE.store(mix_seed(seed), std::sync::atomic::Ordering::Relaxed);
}

/// Return the next pseudo-random `f64` in `[0, 1)`.
fn pseudo_random() -> f64 {
    use std::sync::atomic::Ordering;
    let s = RANDOM_STATE.load(Ordering::Relaxed);
    let (next, v) = lcg_step(s);
    RANDOM_STATE.store(next, Ordering::Relaxed);
    v
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_ast::expr::{CmpOp, Literal};
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    #[test]
    fn runtime_time_is_eight_digit_hhmmssss() {
        // COBOL-85 TIME register: HHMMSSss (8 digits), hundredths of a second.
        let t = runtime_time();
        assert_eq!(t.len(), 8, "TIME must be 8 digits (HHMMSSss), got {t:?}");
        assert!(t.chars().all(|c| c.is_ascii_digit()), "non-digit in {t:?}");
        let hh: u32 = t[0..2].parse().unwrap();
        let mm: u32 = t[2..4].parse().unwrap();
        let ss: u32 = t[4..6].parse().unwrap();
        let cs: u32 = t[6..8].parse().unwrap();
        assert!(hh < 24, "hours out of range: {hh}");
        assert!(mm < 60, "minutes out of range: {mm}");
        assert!(ss < 60, "seconds out of range: {ss}");
        assert!(cs < 100, "centiseconds out of range: {cs}");
    }

    #[test]
    fn rng_same_seed_reproduces_distinct_seeds_differ() {
        // Pure LCG math (no shared state) so this is deterministic under
        // parallel test execution. `FUNCTION RANDOM(seed)` reseeds via
        // `mix_seed`, so the same seed must reproduce the sequence and
        // different seeds must diverge, all within [0, 1).
        let (_, a) = lcg_step(mix_seed(12345));
        let (_, a_again) = lcg_step(mix_seed(12345));
        assert_eq!(a, a_again, "same seed must reproduce the same value");
        let (_, b) = lcg_step(mix_seed(999));
        assert!(a != b, "different seeds must produce different values");
        assert!((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b));
    }

    #[test]
    fn compare_integers() {
        let a = CobolValue::from_i64(10);
        let b = CobolValue::from_i64(5);
        assert!(compare_values(&a, &b, CmpOp::Gt));
        assert!(compare_values(&b, &a, CmpOp::Lt));
        assert!(compare_values(&a, &a, CmpOp::Eq));
    }

    #[test]
    fn compare_strings() {
        let a = CobolValue::from_str("ALPHA", 5);
        let b = CobolValue::from_str("BETA", 4);
        assert!(compare_values(&a, &b, CmpOp::Lt));
        assert!(compare_values(&a, &a, CmpOp::Eq));
    }

    #[test]
    fn literal_to_value_integer() {
        let v = literal_to_value(&Literal::Integer(42));
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn literal_to_value_string() {
        let v = literal_to_value(&Literal::String("HELLO".to_owned()));
        assert_eq!(v.as_display_string(), "HELLO");
    }

    #[test]
    fn datagrid_refresh_binding_updates_rows_from_cobol_table() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GRID-REFRESH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ACTOR-TABLE.
   05 WS-ACTOR-ROW OCCURS 2 TIMES.
      10 ACTOR-ID      PIC 9(09).
      10 ACTOR-CAPTION PIC X(40).
      10 ACTOR-SALARY  PIC S9(9)V99.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "ActorGrid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                (
                    "_BindingFields".to_owned(),
                    "ACTOR-ID\nACTOR-CAPTION\nACTOR-SALARY".to_owned(),
                ),
            ],
        )]);
        interp.env.set_str("ACTOR-ID(1)", "000000001");
        interp.env.set_str("ACTOR-CAPTION(1)", "Leonardo DiCaprio");
        interp.env.set_str("ACTOR-SALARY(1)", "30000000.00");
        interp.env.set_str("ACTOR-ID(2)", "000000002");
        interp.env.set_str("ACTOR-CAPTION(2)", "Joe Pesci");
        interp.env.set_str("ACTOR-SALARY(2)", "12000000.00");

        assert_eq!(interp.refresh_datagrid_binding("ActorGrid"), 2);
        let rows = state_rx
            .try_iter()
            .filter(|update| update.ctrl_id == "ActorGrid" && update.prop == "Rows")
            .map(|update| update.value)
            .last()
            .expect("RefreshBinding should publish Rows");
        assert_eq!(
            rows,
            "1\tLeonardo DiCaprio\t30000000.00\n2\tJoe Pesci\t12000000.00"
        );
    }

    #[test]
    fn scalar_control_refresh_binding_writes_value_from_cobol_field() {
        // Spec 039 R21/T6: a standalone Knob bound to a plain (non-table) WS
        // field — the field's current value becomes the Knob's Value the
        // same way `refresh_datagrid_binding` (above) populates Rows, via
        // the `_BindingScalarField`/`_BindingScalarProperty` seeded props
        // `form_runtime.rs` writes for a `ScalarControl` target.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. KNOB-REFRESH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEMPERATURE PIC 9(3) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "TempKnob".to_owned(),
            "Knob".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                ("_BindingScalarField".to_owned(), "WS-TEMPERATURE".to_owned()),
                ("_BindingScalarProperty".to_owned(), "Value".to_owned()),
            ],
        )]);
        interp.env.set_str("WS-TEMPERATURE", "072");

        assert_eq!(interp.refresh_binding("TempKnob"), 1);
        let value = state_rx
            .try_iter()
            .filter(|update| update.ctrl_id == "TempKnob" && update.prop == "Value")
            .map(|update| update.value)
            .last()
            .expect("refresh_binding should publish Value");
        // Numeric display drops the picture's leading zero padding (72, not
        // 072) — this test only cares that the field's real value (72)
        // reached the control, not COBOL's own numeric-edit formatting.
        assert_eq!(value, "72");
    }

    #[test]
    fn switch_control_refresh_binding_writes_checked_property_not_value() {
        // A Switch's seeded property name is Checked, not Value (R21) — the
        // scalar refresh writes wherever `_BindingScalarProperty` says, not
        // a hardcoded "Value".
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SWITCH-REFRESH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ALARM-ON PIC 9 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "AlarmSwitch".to_owned(),
            "Switch".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                ("_BindingScalarField".to_owned(), "WS-ALARM-ON".to_owned()),
                ("_BindingScalarProperty".to_owned(), "Checked".to_owned()),
            ],
        )]);
        interp.env.set_str("WS-ALARM-ON", "1");

        assert_eq!(interp.refresh_binding("AlarmSwitch"), 1);
        let updates: Vec<_> = state_rx
            .try_iter()
            .filter(|update| update.ctrl_id == "AlarmSwitch")
            .collect();
        assert!(
            updates.iter().any(|u| u.prop == "Checked" && u.value == "1"),
            "expected a Checked update, got {updates:?}"
        );
        assert!(
            !updates.iter().any(|u| u.prop == "Value"),
            "must not write Value for a Switch target, got {updates:?}"
        );
    }

    #[test]
    fn maps_marker_refresh_binding_populates_markers_from_cobol_table() {
        // Spec 039 T13/R22: a Maps control bound to a CobolTable source with
        // lat/lng/label(/id/info) mapped fields — mirrors
        // `datagrid_refresh_binding_updates_rows_from_cobol_table` above,
        // but the seeded shape is `_BindingMarkerFields` (positional
        // id\tlat\tlng\tlabel\tinfo), not `_BindingFields`.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAP-REFRESH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PLACE-TABLE.
   05 WS-PLACE-ROW OCCURS 2 TIMES.
      10 PLACE-ID    PIC X(10).
      10 PLACE-LAT   PIC S9(3)V9(4).
      10 PLACE-LNG   PIC S9(3)V9(4).
      10 PLACE-NAME  PIC X(40).
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "Map1".to_owned(),
            "Maps".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                (
                    "_BindingMarkerFields".to_owned(),
                    "PLACE-ID\tPLACE-LAT\tPLACE-LNG\tPLACE-NAME\t".to_owned(),
                ),
            ],
        )]);
        interp.env.set_str("PLACE-ID(1)", "HQ");
        interp.env.set_str("PLACE-LAT(1)", "040.7128");
        interp.env.set_str("PLACE-LNG(1)", "-074.0060");
        interp.env.set_str("PLACE-NAME(1)", "Headquarters");
        interp.env.set_str("PLACE-ID(2)", "OFC");
        interp.env.set_str("PLACE-LAT(2)", "034.0522");
        interp.env.set_str("PLACE-LNG(2)", "-118.2437");
        interp.env.set_str("PLACE-NAME(2)", "Office");

        assert_eq!(interp.refresh_binding("Map1"), 2);
        let markers = state_rx
            .try_iter()
            .filter(|update| update.ctrl_id == "Map1" && update.prop == "Markers")
            .map(|update| update.value)
            .last()
            .expect("refresh_binding should publish Markers");
        // `as_display_string()` on a `PIC S9(3)V9(4)` field drops the
        // leading-zero padding (40.7128, not 040.7128) the same way the
        // plain `PIC 9(3)` field does in
        // `scalar_control_refresh_binding_writes_value_from_cobol_field`
        // above — this test only cares that the field's real numeric value
        // reached `Markers`, not COBOL's own zero-padding.
        assert_eq!(
            markers,
            "HQ\t40.7128\t-74.0060\tHeadquarters\t\nOFC\t34.0522\t-118.2437\tOffice\t"
        );
    }

    #[test]
    fn maps_marker_refresh_binding_skips_a_row_with_unparseable_lat() {
        // A bad row from a partially-typed edit shouldn't blank the rest of
        // the map — same tolerance `cobolt_forms::parse_map_markers` already
        // has for a malformed `Markers` line.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAP-REFRESH-BAD-ROW.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PLACE-TABLE.
   05 WS-PLACE-ROW OCCURS 2 TIMES.
      10 PLACE-LAT   PIC X(10).
      10 PLACE-LNG   PIC S9(3)V9(4).
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([(
            "Map1".to_owned(),
            "Maps".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                (
                    "_BindingMarkerFields".to_owned(),
                    "\tPLACE-LAT\tPLACE-LNG\t\t".to_owned(),
                ),
            ],
        )]);
        interp.env.set_str("PLACE-LAT(1)", "NOT-A-NUMBER");
        interp.env.set_str("PLACE-LNG(1)", "-074.0060");
        interp.env.set_str("PLACE-LAT(2)", "034.0522");
        interp.env.set_str("PLACE-LNG(2)", "-118.2437");

        assert_eq!(interp.refresh_binding("Map1"), 1);
        assert_eq!(
            interp.obj_get("Map1", "Markers"),
            "2\t034.0522\t-118.2437\t\t"
        );
    }

    // ── Spec 039 T11: Maps data bridge (google_maps + tokio worker) ────────

    #[test]
    fn maps_op_without_a_configured_key_fails_synchronously_no_worker_spawned() {
        // R33: "not configured" is a synchronous, no-network-call failure —
        // confirmed here by the ABSENCE of a pending op (a real spawn would
        // have inserted one) and an immediate onError, not by waiting on
        // any thread.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAPS-NOKEY.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([("Map1".to_owned(), "Maps".to_owned(), vec![])]);
        // No `_ResolvedMapsApiKey` seeded at all.

        let result = interp.exec_method("Map1", "GEOCODE", &[CobolValue::from_str("Paris", 5)]);
        let _ = result;
        assert!(
            !interp.async_pending.contains_key("Map1"),
            "no worker should have been spawned with no API key configured"
        );
        assert_eq!(
            interp.async_dispatch_queue.back(),
            Some(&("Map1".to_owned(), "onError".to_owned())),
            "a missing key must queue onError immediately"
        );
        assert!(
            interp.obj_get("Map1", "LastError").to_lowercase().contains("not configured"),
            "LastError should explain the key is missing, got {:?}",
            interp.obj_get("Map1", "LastError")
        );
    }

    #[test]
    fn maps_op_delivered_result_updates_response_body_and_fires_on_complete() {
        // Proves the delivery HALF of the bridge — `drain_async_ops`
        // applying an `AsyncOpResult` — without a real network call: a
        // real `spawn_maps_op` worker thread would send exactly this
        // shape over `async_result_tx` once `maps_bridge::run` returns
        // (tested independently — this is deliberately a stub result, the
        // same boundary `datagrid_refresh_binding_updates_rows_from_cobol_
        // table` above draws around `refresh_binding` vs. real COBOL
        // table population).
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAPS-DELIVER.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "Map1".to_owned(),
            "Maps".to_owned(),
            vec![("_ResolvedMapsApiKey".to_owned(), "test-key".to_owned())],
        )]);

        // Same call path a real GEOCODE would take, up to the point of
        // actually spawning the worker thread and touching the network.
        let generation = {
            let gen = interp
                .async_generations
                .entry("Map1".to_owned())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        };
        interp.async_pending.insert(
            "Map1".to_owned(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                timeout_ms: 0,
            },
        );
        interp
            .async_result_tx
            .send(crate::async_op::AsyncOpResult {
                ctrl_id: "Map1".to_owned(),
                generation,
                outcome: crate::async_op::AsyncOutcome::HttpSuccess {
                    body: "48.8566\t2.3522\tParis, France".to_owned(),
                    status: 200,
                },
            })
            .unwrap();

        interp.drain_async_ops();

        assert!(
            !interp.async_pending.contains_key("Map1"),
            "the pending op should be cleared once its result is applied"
        );
        assert_eq!(
            interp.async_dispatch_queue.back(),
            Some(&("Map1".to_owned(), "onComplete".to_owned()))
        );
        let updates: Vec<_> = state_rx
            .try_iter()
            .filter(|u| u.ctrl_id == "Map1")
            .collect();
        assert!(
            updates
                .iter()
                .any(|u| u.prop == "ResponseBody" && u.value == "48.8566\t2.3522\tParis, France"),
            "expected the geocode result in ResponseBody, got {updates:?}"
        );
    }

    #[test]
    fn maps_add_marker_appends_and_remove_marker_filters_by_id() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAPS-MARKERS.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([("Map1".to_owned(), "Maps".to_owned(), vec![])]);

        interp.exec_method(
            "Map1",
            "AddMarker",
            &[
                CobolValue::from_str("PIN-1", 5),
                CobolValue::from_str("40.7128", 7),
                CobolValue::from_str("-74.0060", 8),
                CobolValue::from_str("HQ", 2),
                CobolValue::from_str("Headquarters", 12),
            ],
        );
        assert_eq!(
            interp.obj_get("Map1", "Markers"),
            "PIN-1\t40.7128\t-74.0060\tHQ\tHeadquarters"
        );

        interp.exec_method(
            "Map1",
            "AddMarker",
            &[
                CobolValue::from_str("PIN-2", 5),
                CobolValue::from_str("34.0522", 7),
                CobolValue::from_str("-118.2437", 9),
                CobolValue::from_str("Office", 6),
                CobolValue::from_str("", 0),
            ],
        );
        assert_eq!(
            interp.obj_get("Map1", "Markers"),
            "PIN-1\t40.7128\t-74.0060\tHQ\tHeadquarters\nPIN-2\t34.0522\t-118.2437\tOffice\t"
        );

        interp.exec_method(
            "Map1",
            "RemoveMarker",
            &[CobolValue::from_str("PIN-1", 5)],
        );
        assert_eq!(
            interp.obj_get("Map1", "Markers"),
            "PIN-2\t34.0522\t-118.2437\tOffice\t"
        );
    }

    // ── Spec 039 T15: WebSearch runtime + credentials ───────────────────────

    #[test]
    fn web_search_op_without_a_configured_key_fails_synchronously_no_worker_spawned() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SEARCH-NOKEY.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([("Search1".to_owned(), "WebSearch".to_owned(), vec![])]);
        // No `_ResolvedSearchApiKey` seeded at all.

        let _ = interp.exec_method("Search1", "SEARCH", &[]);
        assert!(
            !interp.async_pending.contains_key("Search1"),
            "no worker should have been spawned with no API key configured"
        );
        assert_eq!(
            interp.async_dispatch_queue.back(),
            Some(&("Search1".to_owned(), "onError".to_owned())),
            "a missing key must queue onError immediately"
        );
        assert!(
            interp
                .obj_get("Search1", "LastError")
                .to_lowercase()
                .contains("not configured"),
            "LastError should explain the key is missing, got {:?}",
            interp.obj_get("Search1", "LastError")
        );
    }

    #[test]
    fn web_search_invoke_under_async_mode_sets_busy_and_spawns_a_worker() {
        // Mirrors the existing async RestClient GET coverage (spec 032) —
        // proves SEARCH goes through the SAME `spawn_rest_op` gate (Busy=1,
        // one in-flight op recorded, empty same-statement return), not a
        // separate, untested path.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SEARCH-ASYNC.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([(
            "Search1".to_owned(),
            "WebSearch".to_owned(),
            vec![
                ("_ResolvedSearchApiKey".to_owned(), "test-key".to_owned()),
                ("Mode".to_owned(), "Async".to_owned()),
                ("SearchEngineId".to_owned(), "cx-123".to_owned()),
                ("Query".to_owned(), "best pizza".to_owned()),
            ],
        )]);

        let result = interp.exec_method("Search1", "SEARCH", &[]);
        assert_eq!(result.as_display_string(), "");
        assert!(
            interp.async_pending.contains_key("Search1"),
            "SEARCH under Async mode should record a pending op"
        );
        assert_eq!(interp.obj_get("Search1", "Busy"), "1");
    }

    #[test]
    fn web_search_delivered_result_updates_response_body_and_fires_on_complete() {
        // Same delivery-half boundary as `maps_op_delivered_result_updates_
        // response_body_and_fires_on_complete` above — SEARCH reuses the
        // fully generic `spawn_rest_op`/`drain_async_ops` path (unlike Maps,
        // which needed its own bridge), so this also doubles as regression
        // coverage that reuse didn't change RestClient's own behaviour.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SEARCH-DELIVER.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "Search1".to_owned(),
            "WebSearch".to_owned(),
            vec![("_ResolvedSearchApiKey".to_owned(), "test-key".to_owned())],
        )]);

        let generation = {
            let gen = interp
                .async_generations
                .entry("Search1".to_owned())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        };
        interp.async_pending.insert(
            "Search1".to_owned(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                timeout_ms: 0,
            },
        );
        let body = r#"{"items":[{"title":"Best Pizza","snippet":"Great pies.","link":"https://example.com/1"}]}"#;
        interp
            .async_result_tx
            .send(crate::async_op::AsyncOpResult {
                ctrl_id: "Search1".to_owned(),
                generation,
                outcome: crate::async_op::AsyncOutcome::HttpSuccess {
                    body: body.to_owned(),
                    status: 200,
                },
            })
            .unwrap();

        interp.drain_async_ops();

        assert_eq!(
            interp.async_dispatch_queue.back(),
            Some(&("Search1".to_owned(), "onComplete".to_owned()))
        );
        let updates: Vec<_> = state_rx
            .try_iter()
            .filter(|u| u.ctrl_id == "Search1")
            .collect();
        assert!(
            updates
                .iter()
                .any(|u| u.prop == "ResponseBody" && u.value == body),
            "expected the raw JSON body in ResponseBody, got {updates:?}"
        );
    }

    #[test]
    fn web_search_accessors_parse_result_count_top_result_and_indexed_result() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SEARCH-ACCESSORS.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        let body = r#"{"items":[
            {"title":"Best Pizza","snippet":"Great pies.","link":"https://example.com/1"},
            {"title":"Second Best Pizza","snippet":"Also good.","link":"https://example.com/2"}
        ]}"#;
        interp.seed_objects([(
            "Search1".to_owned(),
            "WebSearch".to_owned(),
            vec![("ResponseBody".to_owned(), body.to_owned())],
        )]);

        assert_eq!(
            interp
                .exec_method("Search1", "ResultCount", &[])
                .as_display_string(),
            "2"
        );
        assert_eq!(
            interp
                .exec_method("Search1", "TopTitle", &[])
                .as_display_string(),
            "Best Pizza"
        );
        assert_eq!(
            interp
                .exec_method("Search1", "TopSnippet", &[])
                .as_display_string(),
            "Great pies."
        );
        assert_eq!(
            interp
                .exec_method("Search1", "TopLink", &[])
                .as_display_string(),
            "https://example.com/1"
        );
        assert_eq!(
            interp
                .exec_method("Search1", "GetResult", &[CobolValue::from_str("2", 1)])
                .as_display_string(),
            "Second Best Pizza\tAlso good.\thttps://example.com/2"
        );
        // Out-of-range index reads as empty, not a crash.
        assert_eq!(
            interp
                .exec_method("Search1", "GetResult", &[CobolValue::from_str("99", 2)])
                .as_display_string(),
            ""
        );
    }

    #[test]
    fn web_search_accessors_read_as_empty_before_any_search() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SEARCH-EMPTY.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([("Search1".to_owned(), "WebSearch".to_owned(), vec![])]);

        assert_eq!(
            interp
                .exec_method("Search1", "ResultCount", &[])
                .as_display_string(),
            "0"
        );
        assert_eq!(
            interp
                .exec_method("Search1", "TopTitle", &[])
                .as_display_string(),
            ""
        );
    }

    #[test]
    fn percent_encode_query_escapes_spaces_and_reserved_characters() {
        assert_eq!(percent_encode_query("best pizza"), "best%20pizza");
        assert_eq!(percent_encode_query("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode_query("safe-word_2024.txt~"), "safe-word_2024.txt~");
    }

    #[test]
    fn datagrid_methods_publish_runtime_overrides_and_manage_rows() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GRID-METHODS.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "ActorGrid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                (
                    "Columns".to_owned(),
                    "Actor Id:number\nActor Caption:string\nActor Salary:number".to_owned(),
                ),
                (
                    "Rows".to_owned(),
                    "2\tJoe Pesci\t12000000\n1\tLeonardo DiCaprio\t30000000".to_owned(),
                ),
                ("CSVDelimiter".to_owned(), ",".to_owned()),
                ("_SelectedText".to_owned(), "Joe Pesci".to_owned()),
            ],
        )]);

        let cv = |s: &str| CobolValue::from_str(s, s.len());
        assert_eq!(
            interp
                .exec_method("ActorGrid", "GetRowCount", &[])
                .as_display_string(),
            "2"
        );
        assert_eq!(
            interp
                .exec_method("ActorGrid", "GetCellValue", &[cv("1"), cv("2")])
                .as_display_string(),
            "Joe Pesci"
        );

        interp.exec_method("ActorGrid", "SetFilter", &[cv("Actor Caption"), cv("Joe")]);
        interp.exec_method("ActorGrid", "FreezeColumns", &[cv("1")]);
        interp.exec_method("ActorGrid", "FreezeRows", &[cv("2")]);
        interp.exec_method("ActorGrid", "SetRowHeight", &[cv("28")]);
        interp.exec_method(
            "ActorGrid",
            "SetColumnWidth",
            &[cv("Actor Caption"), cv("220")],
        );
        interp.exec_method("ActorGrid", "Sort", &[cv("1")]);
        interp.exec_method("ActorGrid", "SetCellValue", &[cv("2"), cv("2"), cv("Leo")]);
        assert_eq!(
            interp
                .exec_method("ActorGrid", "CopySelection", &[])
                .as_display_string(),
            "Joe Pesci"
        );

        let updates = state_rx
            .try_iter()
            .map(|update| (update.prop, update.value))
            .collect::<Vec<_>>();
        assert!(updates.iter().any(|(prop, value)| {
            prop == "_RuntimeColumnFilters" && value == "Actor Caption=Joe"
        }));
        assert!(updates
            .iter()
            .any(|(prop, value)| prop == "_RuntimeFrozenColumns" && value == "1"));
        assert!(updates
            .iter()
            .any(|(prop, value)| prop == "_RuntimeFrozenRows" && value == "2"));
        assert!(updates
            .iter()
            .any(|(prop, value)| prop == "_RuntimeRowHeight" && value == "28"));
        assert!(updates.iter().any(|(prop, value)| {
            prop == "_RuntimeColumnWidths" && value == "Actor Caption=220"
        }));
        assert!(updates
            .iter()
            .any(|(prop, value)| prop == "_CopySelection" && value == "1"));
        assert_eq!(
            interp
                .exec_method("ActorGrid", "ExportCSV", &[])
                .as_display_string(),
            "Actor Id,Actor Caption,Actor Salary"
        );

        interp.exec_method("ActorGrid", "ClearFilters", &[]);
        assert_eq!(
            interp
                .exec_method("ActorGrid", "ExportCSV", &[])
                .as_display_string(),
            "Actor Id,Actor Caption,Actor Salary\n1,Leonardo DiCaprio,30000000\n2,Leo,12000000"
        );
        assert_eq!(interp.obj_get("ActorGrid", "_RuntimeColumnFilters"), "");
    }

    #[test]
    fn array_member_write_carries_instance_index() {
        // `Member(idx)::Prop = v` must tag its UI notification with the 1-based
        // repeating-group instance index (idx), while a scalar member stays 0 —
        // this is what lets the host route each write to the right cloned card.
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. ARR.
PROCEDURE DIVISION.
MAIN.
    MOVE \"Bob\" TO Button-1(2)::Caption.
    MOVE \"Al\"  TO Button-1(1)::Caption.
    MOVE \"flat\" TO Label-9::Caption.
    STOP RUN.
";
        let program = parse(tokenize(source, SourceFormat::Free))
            .program
            .expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([
            ("Button-1".to_owned(), "Button".to_owned(), vec![]),
            ("Label-9".to_owned(), "Label".to_owned(), vec![]),
        ]);
        let _ = interp.run();

        let ups: Vec<_> = state_rx.try_iter().collect();
        let by_val = |v: &str| ups.iter().find(|u| u.value == v).expect("update present");
        let bob = by_val("Bob");
        // COBOL upper-cases the member name; the routing is what matters here.
        assert!(bob.prop.eq_ignore_ascii_case("Caption"));
        assert_eq!(bob.instance_index, 2, "Button-1(2) → instance 2");
        assert_eq!(by_val("Al").instance_index, 1, "Button-1(1) → instance 1");
        assert_eq!(by_val("flat").instance_index, 0, "scalar member → 0");
    }

    #[test]
    fn datagrid_csv_export_uses_display_order_and_export_mode() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GRID-CSV.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let mut interp = Interpreter::new(program);
        interp.seed_objects([(
            "ActorGrid".to_owned(),
            "DataGrid".to_owned(),
            vec![
                (
                    "Columns".to_owned(),
                    "ID:number\nNAME:string\nSTATUS:string".to_owned(),
                ),
                (
                    "Rows".to_owned(),
                    "1\tLeonardo DiCaprio\tActive\n2\tJoe Pesci\tClosed".to_owned(),
                ),
                ("CSVDelimiter".to_owned(), ",".to_owned()),
                ("CSVExportMode".to_owned(), "Filtered".to_owned()),
                (
                    "AdvancedGrid".to_owned(),
                    r#"{"schema_version":1,"columns":[{"id":"NAME","title":"Actor Name","source_name":"NAME","value_type":"string","width":180.0,"visible":true},{"id":"ID","title":"Actor Id","source_name":"ID","value_type":"number","width":80.0,"visible":true}],"frozen_columns":0,"frozen_rows":0,"row_height":22,"row_overrides":[],"filters":[{"column_id":"STATUS","value":"Active","active":true}],"csv_export_mode":"Filtered","csv_delimiter":",","grid_line_style":"Solid","selectable_text":true}"#.to_owned(),
                ),
            ],
        )]);

        assert_eq!(
            interp
                .exec_method("ActorGrid", "ExportCSV", &[])
                .as_display_string(),
            "Actor Name,Actor Id\nLeonardo DiCaprio,1"
        );

        interp.exec_method(
            "ActorGrid",
            "SetProperty",
            &[
                CobolValue::from_str("CSVExportMode", 13),
                CobolValue::from_str("All", 3),
            ],
        );
        assert_eq!(
            interp
                .exec_method("ActorGrid", "ExportCSV", &[])
                .as_display_string(),
            "Actor Name,Actor Id\nLeonardo DiCaprio,1\nJoe Pesci,2"
        );
    }

    #[test]
    fn ymd_epoch() {
        // Unix epoch = 1970-01-01
        assert_eq!(days_to_ymd(0), "19700101");
    }

    #[test]
    fn ymd_known_date() {
        // 2024-01-01: 54 years since 1970 (with leap years)
        let d = days_to_ymd(19723); // 19723 days = 2024-01-01
        assert!(d.starts_with("202"), "got {d}");
    }

    #[test]
    fn test_control_array_refresh() {
        let source = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. CTRL-ARRAY-TEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ACTOR-TABLE.
   05 WS-ACTOR-ROW OCCURS 2 TIMES.
      10 ACTOR-ID      PIC 9(09).
      10 ACTOR-CAPTION PIC X(40).
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
        let parsed = parse(tokenize(source, SourceFormat::Free));
        let program = parsed.program.expect("program should parse");
        let (state_tx, state_rx) = std::sync::mpsc::channel();
        let mut interp = Interpreter::new(program);
        interp.state_tx = Some(state_tx);
        interp.seed_objects([(
            "groupbox-2".to_owned(),
            "GroupBox".to_owned(),
            vec![
                ("IsRepeatingGroup".to_owned(), "true".to_owned()),
                ("ArrayName".to_owned(), "ActorArray".to_owned()),
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                (
                    "_BindingFields".to_owned(),
                    "ACTOR-ID\nACTOR-CAPTION".to_owned(),
                ),
                (
                    "_BindingMappings".to_owned(),
                    "ACTOR-CAPTION\tlabel-3\tCaption".to_owned(),
                ),
                ("_BindingArray".to_owned(), "1".to_owned()),
            ],
        )]);
        interp.env.set_str("ACTOR-ID(1)", "000000001");
        interp.env.set_str("ACTOR-CAPTION(1)", "Leonardo DiCaprio");
        interp.env.set_str("ACTOR-ID(2)", "000000002");
        interp.env.set_str("ACTOR-CAPTION(2)", "Joe Pesci");

        assert_eq!(interp.refresh_control_array_binding("ActorArray"), 2);

        let updates: Vec<_> = state_rx.try_iter().collect();
        println!("UPDATES: {:?}", updates);
    }
}
