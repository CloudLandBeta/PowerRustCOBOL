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
use std::sync::mpsc;

use cobolt_ast::{
    expr::{ArithOp, Condition, CmpOp, DataClass, Expr, FigurativeConstant, Literal,
           SignCond, UnaryOp},
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
    environment::CobolEnvironment,
    error::RuntimeError,
    exec_rust,
    objects::ObjectRegistry,
    value::{CobolValue, CobolNumeric},
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
    Writer { w: std::io::BufWriter<std::fs::File>, org: FileOrganization },
    /// SEQUENTIAL / LINE SEQUENTIAL, opened for input.
    Reader { r: std::io::BufReader<std::fs::File>, org: FileOrganization },
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
    /// `PROCEDURE DIVISION USING …` LINKAGE parameter names (as written), in
    /// order — bound to the caller's `CALL … USING` arguments.
    using: Vec<String>,
}

/// Recursively register a `Program` and all of its `nested_programs` into
/// `registry`, keyed by the program-id (uppercase).
fn register_nested(prog: &Program, registry: &mut HashMap<String, NestedProgram>) {
    let (para_map, para_order) = build_para_map(&prog.procedure.body);

    // Collect this program's own local data items (everything in its DATA
    // DIVISION — they will be added to the env as a scope overlay on call).
    let local_items: Vec<(String, CobolValue)> = if let Some(data) = &prog.data {
        let local_env = CobolEnvironment::from_data_division_with(data, prog.decimal_comma);
        local_env.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        Vec::new()
    };

    let key = prog.identification.program_id.to_ascii_uppercase();
    let using = prog.procedure.using.clone();
    registry.insert(key, NestedProgram { para_map, para_order, local_items, using });

    // Recurse into any nested-programs declared inside this one.
    for child in &prog.nested_programs {
        register_nested(child, registry);
    }
}

/// Build the file registry from the program's FILE-CONTROL (SELECT) entries and
/// FILE SECTION (FD) records: `(logical name → spec, record name → file name)`.
fn build_file_specs(
    program: &Program,
) -> (HashMap<String, FileSpec>, HashMap<String, String>) {
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
                    let names: Vec<String> = fd.records.iter()
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
                specs.insert(key.clone(), FileSpec {
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
                });
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
    use cobolt_ast::program::StorageMode;
    use crate::indexed::{IndexedEngine, IndexedFile, KeySpec};
    use crate::indexed_disk::DiskIndexedFile;
    use crate::indexed_redb::RedbIndexedFile;
    let layout = &spec.layout;
    let reclen = layout.len.max(1);
    let primary = spec
        .record_key
        .as_deref()
        .and_then(|k| layout.key_spec(k, false))
        .unwrap_or(KeySpec { offset: 0, len: reclen, duplicates: false });
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

/// Tree-walking COBOL interpreter.
pub struct Interpreter {
    /// The parsed program (retained for metadata access).
    pub program: Program,
    /// Runtime data store — all DATA DIVISION items live here.
    pub env: CobolEnvironment,
    /// PowerRustCOBOL form/control object registry.
    pub objects: ObjectRegistry,
    /// Property "shadows": a receiving property reference used by any verb is
    /// resolved to a synthetic env item preloaded with the property's current
    /// value; after each statement these are written back to the object store.
    /// Maps synthetic-env-key → (control, property-key).
    property_shadows: std::collections::HashMap<String, (String, String)>,
    /// Paragraph name (uppercase) → statement list.
    para_map: IndexMap<String, Vec<Stmt>>,
    /// Paragraph names in declaration order (for fall-through and THRU ranges).
    para_order: Vec<String>,
    /// COBOL-85 nested programs: program-id (uppercase) → compiled program.
    nested_registry: HashMap<String, NestedProgram>,
    /// Current PERFORM nesting depth (overflow guard).
    perform_depth: usize,
    /// Database runtime engine (Phase 8) — manages SQLite connections.
    db: DbRegistry,
    /// HTTP client (Phase 10) — manages persistent headers and sends requests.
    http: crate::http_runtime::HttpClient,

    // ── GUI Form Runtime channels (Phase 6) ───────────────────────────────────
    /// Receives UI events (button clicks, text changes, etc.) from the form window.
    /// When `Some`, `COBOL-WAIT-EVENT` blocks on `recv()` instead of quitting.
    event_rx: Option<mpsc::Receiver<FormEvent>>,
    /// Sends property-change notifications to the form window UI thread.
    /// Used by `COBOL-SET-PROPERTY`.
    state_tx: Option<mpsc::Sender<StateUpdate>>,
    /// Sends DISPLAY output to the IDE output panel (instead of stdout).
    display_tx: Option<mpsc::Sender<String>>,

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
    /// default / VALUE clause values.
    pub fn new(program: Program) -> Self {
        let env = if let Some(data) = &program.data {
            CobolEnvironment::from_data_division_with(data, program.decimal_comma)
        } else {
            CobolEnvironment::new()
        };
        let (para_map, para_order) = build_para_map(&program.procedure.body);

        // Register all COBOL-85 nested programs (recursively).
        let mut nested_registry: HashMap<String, NestedProgram> = HashMap::new();
        for nested in &program.nested_programs {
            register_nested(nested, &mut nested_registry);
        }

        let (file_specs, record_to_file) = build_file_specs(&program);

        // Flatten the parsed DECLARATIVES into runtime-ready handlers.
        let declaratives: Vec<DeclHandler> = program.procedure.declaratives.iter()
            .map(|u| DeclHandler {
                files: u.files.clone(),
                modes: u.modes.clone(),
                catch_all: u.catch_all,
                stmts: u.stmts.clone(),
            })
            .collect();

        Self {
            program,
            env,
            objects: ObjectRegistry::new(),
            property_shadows: std::collections::HashMap::new(),
            para_map,
            para_order,
            nested_registry,
            perform_depth: 0,
            db:   DbRegistry::new(),
            http: crate::http_runtime::HttpClient::new(),
            event_rx:   None,
            state_tx:   None,
            display_tx: None,
            debug_cmd_rx:      None,
            debug_event_tx:    None,
            breakpoints:       None,
            debug_stepping:    false,
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
        program:    Program,
        event_rx:   mpsc::Receiver<FormEvent>,
        state_tx:   mpsc::Sender<StateUpdate>,
        display_tx: mpsc::Sender<String>,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.event_rx   = Some(event_rx);
        interp.state_tx   = Some(state_tx);
        interp.display_tx = Some(display_tx);
        interp
    }

    /// Create an interpreter wired to the IDE debugger channels.
    ///
    /// - `debug_cmd_rx`  — receives `DebugCmd` from the IDE (Continue, StepOver, Pause)
    /// - `debug_event_tx`— sends `DebugEvent` to the IDE (Paused, Resumed, Finished)
    /// - `breakpoints`   — shared set of active breakpoint line numbers
    pub fn new_with_debug_channels(
        program:        Program,
        debug_cmd_rx:   mpsc::Receiver<crate::debugger::DebugCmd>,
        debug_event_tx: mpsc::Sender<crate::debugger::DebugEvent>,
        breakpoints:    crate::debugger::Breakpoints,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.debug_cmd_rx   = Some(debug_cmd_rx);
        interp.debug_event_tx = Some(debug_event_tx);
        interp.breakpoints    = Some(breakpoints);
        interp.debug_stepping = true; // start paused at line 1
        interp
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
        for (id, class, props) in controls {
            if !self.objects.contains(&id) {
                self.objects.register(&id, class);
            }
            for (k, v) in props {
                self.objects.set_property(&id, &k, v);
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
        let mut idx = 0usize;
        while idx < self.para_order.len() {
            let name = self.para_order[idx].clone();
            self.current_paragraph = name.clone();
            let stmts = match self.para_map.get(&name) {
                Some(s) => s.clone(),
                None => { idx += 1; continue; }
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
                        None => return Err(RuntimeError::UndefinedParagraph {
                            name: upper,
                            span: Span::dummy(),
                        }),
                    }
                }
                // Normal program termination signals — treat as success.
                Err(RuntimeError::StopRun) | Err(RuntimeError::GoBack) => {
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
        para_map:   &IndexMap<String, Vec<Stmt>>,
        para_order: &[String],
    ) -> Result<(), RuntimeError> {
        let mut idx = 0usize;
        while idx < para_order.len() {
            let name = &para_order[idx];
            self.current_paragraph = name.clone();
            let stmts = match para_map.get(name) {
                Some(s) => s.clone(),
                None => { idx += 1; continue; }
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
                        None => return Err(RuntimeError::UndefinedParagraph {
                            name: upper,
                            span: Span::dummy(),
                        }),
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
                    while j < stmts.len()
                        && !matches!(stmts[j], Stmt::SentenceEnd { .. })
                    {
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
    /// Resumes on `DebugCmd::Continue` or `DebugCmd::StepOver`.
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
        let hit_breakpoint = line > 0 && self.breakpoints.as_ref().map(|bp| {
            bp.lock().map(|set| set.contains(&line)).unwrap_or(false)
        }).unwrap_or(false);

        if !self.debug_stepping && !hit_breakpoint {
            // Check for async Pause command without blocking.
            match cmd_rx.try_recv() {
                Ok(crate::debugger::DebugCmd::Pause) => self.debug_stepping = true,
                _ => return Ok(()),
            }
        }

        // Build variable snapshot.
        let vars: Vec<crate::debugger::VarSnapshot> = self.env.iter()
            .map(|(k, v)| crate::debugger::VarSnapshot {
                name:  k.clone(),
                value: v.as_display_string(),
            })
            .collect();

        let _ = ev_tx.send(crate::debugger::DebugEvent::Paused {
            line:      line,
            col:       span.map(|s| s.col).unwrap_or(0),
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
                Ok(crate::debugger::DebugCmd::StepOver) => {
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
            Stmt::Move { from, to, .. } =>
                self.exec_move(from, to),
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

            Stmt::Initialize { items, replacing, .. } => self.exec_initialize(items, replacing),

            // ── Arithmetic ────────────────────────────────────────────────────
            Stmt::Add { operands, to, giving, on_size_error, not_on_size_error, span } =>
                self.exec_add(operands, to, giving, on_size_error, not_on_size_error, *span),
            Stmt::Subtract { operands, from, giving, on_size_error, not_on_size_error, span } =>
                self.exec_subtract(operands, from, giving, on_size_error, not_on_size_error, *span),
            Stmt::Multiply { lhs, by, giving, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_multiply(lhs, by, giving, *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Divide { lhs, by, giving, remainder, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_divide(lhs, by, giving, remainder.as_ref(), *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Compute { targets, expr, on_size_error, not_on_size_error, span } =>
                self.exec_compute(targets, expr, on_size_error, not_on_size_error, *span),

            // ── Control flow ──────────────────────────────────────────────────
            Stmt::If { condition, then_stmts, else_stmts, .. } =>
                self.exec_if(condition, then_stmts, else_stmts),
            Stmt::Evaluate { subjects, whens, other_stmts, .. } =>
                self.exec_evaluate(subjects, whens, other_stmts),
            Stmt::Perform { target, span } =>
                self.exec_perform(target, *span),
            Stmt::Search { all, table, varying, at_end, whens, .. } =>
                self.exec_search(*all, table, varying.as_ref(), at_end, whens),
            Stmt::GoTo { target, .. } => {
                // An ALTER may have redirected this paragraph's GO TO.
                let t = self.alter_map.get(&self.current_paragraph)
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
            Stmt::SetPointer { address_of, targets, source, .. } =>
                self.exec_set_pointer(address_of.as_ref(), targets, source),
            Stmt::GoToDepending { targets, depending, span } =>
                self.exec_go_to_depending(targets, depending, *span),
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
            Stmt::Accept { target, from, screen, span } =>
                self.exec_accept(target, from.as_ref(), screen.as_ref(), *span),
            Stmt::Display { operands, no_advancing, screen, upon, .. } =>
                self.exec_display(operands, *no_advancing, screen.as_ref(), upon.as_deref()),
            Stmt::Open { mode, files, lock, registered_user, span, .. } =>
                self.exec_open(*mode, files, *lock, registered_user.as_ref(), *span),
            Stmt::Close { files, .. } =>
                self.exec_close(files),
            Stmt::Write { record, from, invalid_key, not_invalid_key, span, .. } =>
                self.exec_write(record, from.as_ref(), invalid_key, not_invalid_key, *span),
            Stmt::Read { file, into, key, direction, lock, at_end, not_at_end, invalid_key, not_invalid_key, span } =>
                self.exec_read(file, into.as_ref(), key.as_ref(), *direction, *lock,
                    at_end, not_at_end, invalid_key, not_invalid_key, *span),
            Stmt::Rewrite { record, from, invalid_key, not_invalid_key, span } =>
                self.exec_rewrite(record, from.as_ref(), invalid_key, not_invalid_key, *span),
            Stmt::Delete { file, invalid_key, not_invalid_key, span } =>
                self.exec_delete(file, invalid_key, not_invalid_key, *span),
            Stmt::Start { file, key, invalid_key, not_invalid_key, span } => {
                self.exec_start(file, key.as_ref(), invalid_key, not_invalid_key, *span)
            }

            // ── String handling ───────────────────────────────────────────────
            Stmt::String_ { operands, into, pointer, on_overflow, not_on_overflow, span } =>
                self.exec_string(operands, into, pointer.as_ref(), on_overflow, not_on_overflow, *span),
            Stmt::Unstring { from, delimited_by, all, into, pointer, tallying, on_overflow, not_on_overflow, span } =>
                self.exec_unstring(from, delimited_by, *all, into,
                                   pointer.as_ref(), tallying.as_ref(), on_overflow, not_on_overflow, *span),
            Stmt::Inspect { target, spec, span } =>
                self.exec_inspect(target, spec, *span),

            // ── Sorting ───────────────────────────────────────────────────────
            Stmt::Sort { file, keys, using, giving, input_proc, output_proc, duplicates: _, span } =>
                self.exec_sort(file, keys, using, giving,
                    input_proc.as_deref(), output_proc.as_deref(), *span),
            Stmt::Merge { file, keys, using, giving, output_proc, span } =>
                self.exec_sort(file, keys, using, giving, None, output_proc.as_deref(), *span),
            Stmt::Release { record, from, .. } =>
                self.exec_release(record, from.as_ref()),
            Stmt::Return { file, into, at_end, not_at_end, .. } =>
                self.exec_return(file, into.as_ref(), at_end, not_at_end),

            // ── Subprogram linkage ────────────────────────────────────────────
            Stmt::Call { program, using, returning, on_exception, not_on_exception, span } =>
                self.exec_call(program, using, returning.as_ref(), on_exception, not_on_exception, *span),

            Stmt::Invoke { object, method, args, returning, span } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args { vals.push(self.eval_expr(a, *span)?); }
                let result = self.exec_method(object, method, &vals);
                if let Some(dest) = returning {
                    let s = result.as_display_string();
                    let s = s.trim();
                    if let Expr::PropertyRef { control, path, span: ps } = dest {
                        let (ctrl, key) = self.property_ref_key(control, path, *ps);
                        self.obj_set(&ctrl, &key, s.to_string());
                    } else {
                        let n = self.expr_to_name(dest);
                        self.env.set_str(&n, s);
                    }
                }
                Ok(())
            }

            // ── Program termination ───────────────────────────────────────────
            Stmt::Stop { run: true, .. } => Err(RuntimeError::StopRun),
            Stmt::Stop { run: false, literal, .. } => {
                if let Some(lit) = literal {
                    let s = match lit {
                        Literal::String(s)  => s.clone(),
                        Literal::Integer(n) => n.to_string(),
                        _                   => String::new(),
                    };
                    if !s.is_empty() { println!("{s}"); }
                }
                Ok(())
            }
            Stmt::GoBack { .. } => Err(RuntimeError::GoBack),

            // ── EXEC RUST ─────────────────────────────────────────────────────
            Stmt::ExecRust { .. } =>
                exec_rust::execute(stmt, &mut self.env, &mut self.objects),

            // ── TRY / CATCH EXCEPTION / FINALLY ──────────────────────────────
            Stmt::TryCatch { try_stmts, exception_var, catch_stmts, finally_stmts, .. } => {
                // Execute the TRY body, catching any UserException.
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
                Err(RuntimeError::UserException { message: val.as_display_string() })
            }

            // ── PowerCOBOL extensions ─────────────────────────────────────────
            Stmt::WindowOp { op, .. } => {
                tracing::debug!("WindowOp: {:?}", op);
                Ok(())
            }
            Stmt::ControlSet { control, property, value, span } => {
                let ctrl = self.expr_to_name(control);
                let val  = self.eval_expr(value, *span)?;
                self.objects.set_property(&ctrl, property, val.as_display_string());
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
            if let Expr::RefMod { base, start, length, span } = target {
                self.assign_refmod(base, start, length.as_deref(), &val, *span)?;
                continue;
            }
            // PowerCOBOL-style property receiver: write the control's property
            // (the value's type is inferred from the moved value — no temp item).
            if let Expr::PropertyRef { control, path, span } = target {
                let (ctrl, key) = self.property_ref_key(control, path, *span);
                let v = val.as_display_string().trim().to_owned();
                // A control referenced by the property syntax exists by virtue of
                // being on the form — register it on first write if needed.
                if !self.objects.contains(&ctrl) {
                    self.objects.register(&ctrl, "Control");
                }
                self.objects.set_property(&ctrl, &key, v.clone());
                if let Some(tx) = &self.state_tx {
                    let _ = tx.send(StateUpdate::new(ctrl, key, v));
                }
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
            match &src_digits {
                Some(digits) if self.env.is_alphanumeric_field(&name) => {
                    self.env.set_str_left(&name, digits);
                }
                _ => self.env.set(&name, val.clone()),
            }
        }
        Ok(())
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
            ConditionValue::Range(lo, hi) =>
                compare_values(&pv, &literal_to_value(lo), CmpOp::Ge)
                    && compare_values(&pv, &literal_to_value(hi), CmpOp::Le),
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
                let val = self.env.get(&fk).cloned().unwrap_or_else(|| CobolValue::from_i64(0));
                let src_digits = self.env.deedited_digits(&fk);
                match src_digits {
                    Some(digits) if self.env.is_alphanumeric_field(&tk) =>
                        self.env.set_str_left(&tk, &digits),
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
                let a = self.env.get(&fk).cloned().unwrap_or_else(|| CobolValue::from_i64(0));
                let cur = self.env.get(&tk).cloned().unwrap_or_else(|| CobolValue::from_i64(0));
                let result = if subtract { cur.sub_val(&a) } else { cur.add_val(&a) };
                self.store_arith(&tk, result, false, false);
            }
        }
        Ok(())
    }

    /// Matching subordinate pairs of two groups: `(from_child_key,
    /// to_child_key, both_are_groups)` for every leaf name they share.
    fn corr_pairs(&self, from_key: &str, to_key: &str) -> Vec<(String, String, bool)> {
        let from_sym = match self.env.symbol(from_key) { Some(s) => s.clone(), None => return Vec::new() };
        let to_sym = match self.env.symbol(to_key) { Some(s) => s.clone(), None => return Vec::new() };
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
            Expr::Qualified { name, of, .. } =>
                self.env.canonical_name(name, &collect_quals(of)),
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
        let size = sym.as_ref()
            .map(|s| if s.occurs > 0 { s.occurs } else { s.dims.last().copied().unwrap_or(0) })
            .unwrap_or(0);
        // Index = VARYING item, else the table's first INDEXED BY index.
        let index_name = match varying {
            Some(v) => self.expr_to_name(v),
            None => sym.as_ref().and_then(|s| s.index_names.first().cloned()).unwrap_or_default(),
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
                        let ascending = keys.iter()
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
        let start = if all { 1 } else { self.env.get_i64(&index_name).unwrap_or(1).max(1) };
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
            let name = self.resolve_lvalue(item);
            // Walk the DATA DIVISION for the item's declaration so groups recurse
            // into their elementary children; fall back to field-cap inference.
            let decl = self.program.data.as_ref()
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
            Some(PicKind::Alphabetic)        => InitCategory::Alphabetic,
            Some(PicKind::Alphanumeric)      => InitCategory::Alphanumeric,
            Some(PicKind::Numeric)           => InitCategory::Numeric,
            Some(PicKind::AlphanumericEdited) => InitCategory::AlphanumericEdited,
            Some(PicKind::NumericEdited)     => InitCategory::NumericEdited,
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
                let decimals = d.picture.as_ref().map(|p| p.decimals).unwrap_or(0).min(u8::MAX as u16) as u8;
                self.env.set(&key, CobolValue::Numeric(CobolNumeric::new(0, decimals)));
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
                let cur = self.env.get(&name).cloned()
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
                let cur = self.env.get(&name).cloned()
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
                        None => { all = false; break; }
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
            (EvalSubject::False_, WhenValue::Condition(c)) => {
                Ok(!self.eval_condition(c)?)
            }
            (EvalSubject::Expr(e), WhenValue::Literal(lit)) => {
                let subj = self.eval_expr(e, e.span())?;
                let lv   = literal_to_value(lit);
                Ok(compare_values(&subj, &lv, CmpOp::Eq))
            }
            (EvalSubject::Expr(e), WhenValue::Range(lo, hi)) => {
                let subj  = self.eval_expr(e, e.span())?;
                let lo_v  = literal_to_value(lo);
                let hi_v  = literal_to_value(hi);
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
            return Err(RuntimeError::PerformDepthExceeded { max: MAX_PERFORM_DEPTH });
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

    fn exec_perform_inner(&mut self, target: &PerformTarget, span: Span) -> Result<(), RuntimeError> {
        match target {
            PerformTarget::Paragraph(name, s) => {
                let stmts = self.para_stmts(name, *s)?;
                // Track the active paragraph so ALTER overrides resolve correctly.
                let prev = std::mem::replace(
                    &mut self.current_paragraph, name.to_ascii_uppercase());
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
            PerformTarget::Inline { stmts } =>
                match self.exec_loop_body(stmts) {
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
            PerformTarget::Until { condition, test_before, stmts } => {
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
                        if self.eval_condition(condition)? { break; }
                    }
                }
                Ok(())
            }
            PerformTarget::Varying { var, from, by, until, stmts, after } => {
                self.exec_perform_varying(var, from, by, until, stmts, after, span)
            }
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
            if self.eval_condition(until)? { break; }

            // Inner AFTER loops (right-most varies fastest). `EXIT PERFORM`
            // (without CYCLE) anywhere inside breaks out of the whole VARYING.
            if self.exec_perform_after(after, stmts, span)? { break; }

            // Increment outer variable
            let by_val = self.eval_expr(by, span)?;
            let cur = self.env.get(&var_name).cloned()
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
            if self.eval_condition(&head.until)? { break; }
            if self.exec_perform_after(tail, stmts, span)? { return Ok(true); }
            let by_val = self.eval_expr(&head.by, span)?;
            let cur = self.env.get(&var_name).cloned()
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
            Err(RuntimeError::GoTo { target: targets[(idx - 1) as usize].clone() })
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
                let s = line.trim_end_matches('\n').trim_end_matches('\r').to_owned();
                self.env.set_str(&name, &s);
            }
            Some(AcceptSource::Date)      => self.env.set_str(&name, &runtime_date()),
            Some(AcceptSource::Time)      => self.env.set_str(&name, &runtime_time()),
            Some(AcceptSource::Day)       => self.env.set_str(&name, &runtime_julian_day()),
            Some(AcceptSource::DayOfWeek) => self.env.set_i64(&name, runtime_day_of_week()),
            Some(AcceptSource::CommandLine) => {
                self.env.set_str(&name, &self.program_args.join(" "));
            }
            Some(AcceptSource::ArgumentNumber) => {
                self.env.set_i64(&name, self.program_args.len() as i64);
            }
            Some(AcceptSource::ArgumentValue) => {
                let val = self.program_args
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
                    self.env_name_register = self.eval_expr(op, op.span())?
                        .as_display_string().trim().to_string();
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
                    match self.env.display_string(&key) {
                        Some(s) => s,
                        None => self.eval_expr(op, op.span())?.as_display_string(),
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
            let v = self.eval_expr(at, at.span()).map(|x| x.as_i64().unwrap_or(0)).unwrap_or(0);
            return ((v / 100).max(1), (v % 100).max(1));
        }
        let row = sc.line.as_ref()
            .and_then(|e| self.eval_expr(e, e.span()).ok())
            .and_then(|v| v.as_i64()).unwrap_or(1).max(1);
        let col = sc.col.as_ref()
            .and_then(|e| self.eval_expr(e, e.span()).ok())
            .and_then(|v| v.as_i64()).unwrap_or(1).max(1);
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
        let capacity = self.env.display_string(&name).map(|s| s.len()).unwrap_or(usize::MAX);

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
        let delims: Vec<String> = delimited_by.iter()
            .map(|d| self.eval_expr(d, span)
                .unwrap_or_else(|_| CobolValue::from_str(" ", 1))
                .as_display_string())
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

    fn exec_inspect(&mut self, target: &Expr, spec: &InspectSpec, span: Span) -> Result<(), RuntimeError> {
        let name = self.resolve_lvalue(target);
        let val  = self.env.get(&name).cloned()
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
                                if pat.is_empty() { 0 } else { win.matches(pat.as_str()).count() as i64 }
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
                            if !pat.is_empty() { win = win.replace(pat.as_str(), &by); }
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
                let to_s   = self.eval_expr(to,   span)?.as_display_string();
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
        if let Some(field) = self.file_specs.get(file).and_then(|s| s.status_field.clone()) {
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
                    OpenMode::Input       => UseMode::Input,
                    OpenMode::Output      => UseMode::Output,
                    OpenMode::InputOutput => UseMode::Io,
                    OpenMode::Extend      => UseMode::Extend,
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
            let org  = spec.organization;

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
                self.open_files.insert(file.clone(), OpenFile::Indexed(engine));
                self.set_file_status(&file, code);
                self.fire_declarative(&file, code, false)?;
                continue;
            }

            // ── SEQUENTIAL / LINE SEQUENTIAL ───────────────────────────────
            let result: std::io::Result<OpenFile> = match mode {
                OpenMode::Output =>
                    std::fs::File::create(&path)
                        .map(|f| OpenFile::Writer { w: BufWriter::new(f), org }),
                OpenMode::Extend =>
                    OpenOptions::new().create(true).append(true).open(&path)
                        .map(|f| OpenFile::Writer { w: BufWriter::new(f), org }),
                OpenMode::Input =>
                    std::fs::File::open(&path)
                        .map(|f| OpenFile::Reader { r: BufReader::new(f), org }),
                OpenMode::InputOutput =>
                    OpenOptions::new().read(true).write(true).create(true).open(&path)
                        .map(|f| OpenFile::Reader { r: BufReader::new(f), org }),
            };

            match result {
                Ok(handle) => {
                    self.open_files.insert(file.clone(), handle);
                    self.set_file_status(&file, "00");
                }
                Err(e) => {
                    tracing::warn!("OPEN '{}' ({}) failed: {}", raw, path, e);
                    let code = if matches!(mode, OpenMode::Input)
                        && e.kind() == std::io::ErrorKind::NotFound { "35" } else { "30" };
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
                    OpenFile::Writer { w, .. } => { let _ = w.flush(); "00" }
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
            None => self.env.get_string(&rec_name).unwrap_or_default().into_bytes(),
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
        let rec = self.sort_buffers.get(&fkey).and_then(|v| v.get(cur)).cloned();
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
                self.sort_buffers.entry(fkey.clone()).or_default().extend(recs);
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
        let Some(spec) = self.file_specs.get(fkey).cloned() else { return Ok(()); };
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
        self.sort_buffers
            .insert(fkey.to_string(), keyed.into_iter().map(|(_, b)| b).collect());
        Ok(())
    }

    /// Open `file` for input, read every record (raw bytes), and close it.
    fn read_all_records(&mut self, file: &str) -> Vec<Vec<u8>> {
        use std::io::{BufRead as _, Read as _};
        let fkey = file.to_ascii_uppercase();
        let _ = self.exec_open(OpenMode::Input, &[file.to_string()], false, None, Span::dummy());
        let rlen = self.file_specs.get(&fkey).map(|s| s.layout.len.max(1)).unwrap_or(1);
        let mut out = Vec::new();
        loop {
            let rec = match self.open_files.get_mut(&fkey) {
                Some(OpenFile::Reader { r, org }) => match org {
                    FileOrganization::LineSequential => {
                        let mut line = String::new();
                        match r.read_line(&mut line) {
                            Ok(0) => None,
                            Ok(_) => {
                                while line.ends_with('\n') || line.ends_with('\r') { line.pop(); }
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
        self.exec_open(OpenMode::Output, &[file.to_string()], false, None, Span::dummy())?;
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
    ) -> Result<(), RuntimeError>
    {
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
            None => self.env.get_string(&rec_name).unwrap_or_default().into_bytes(),
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
                    Err(e) => { tracing::warn!("WRITE failed: {e}"); "30" }
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
        use std::io::BufRead as _;
        use cobolt_ast::stmt::ReadDirection;
        use crate::indexed::{status, ReadDir};

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
        let read_dir = if direction == ReadDirection::Previous { ReadDir::Previous } else { ReadDir::Next };
        let key_field = key.map(|e| self.expr_to_name(e)).or_else(|| spec.record_key.clone());
        let key_bytes = key_field.as_ref().and_then(|kf| spec.layout.field_value(&self.env, kf));
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
                            while line.ends_with('\n') || line.ends_with('\r') { line.pop(); }
                            (Some(line.into_bytes()), status::OK)
                        }
                        Err(e) => { tracing::warn!("READ failed: {e}"); (None, "30") }
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
                        Err(e) => { tracing::warn!("READ failed: {e}"); (None, "30") }
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
            if !primary.is_empty() { primary } else { fallback }
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
    ) -> Result<(), RuntimeError>
    {
        if let Some(src) = from {
            self.exec_move(src, std::slice::from_ref(record))?;
        }
        let rec_name = self.expr_to_name(record);
        let Some(file) = self.record_to_file.get(&rec_name).cloned() else {
            tracing::warn!("REWRITE: record '{}' is not part of any FD", rec_name);
            return Ok(());
        };
        let Some(spec) = self.file_specs.get(&file).cloned() else { return Ok(()); };
        let buf = spec.layout.materialize(&self.env);
        let random = spec.access != AccessMode::Sequential; // RANDOM or DYNAMIC address by key
        let code = match self.open_files.get_mut(&file) {
            Some(OpenFile::Indexed(engine)) => {
                engine.rewrite(&buf, if random { Some(buf.as_slice()) } else { None })
            }
            Some(_) => {
                tracing::warn!("REWRITE on a non-indexed file '{}' is not yet supported", file);
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
        let Some(spec) = self.file_specs.get(&file).cloned() else { return Ok(()); };
        let random = spec.access != AccessMode::Sequential; // RANDOM or DYNAMIC address by key
        // RANDOM DELETE addresses the record by the current RECORD KEY value;
        // sequential/dynamic DELETE removes the current (last read) record.
        let key_bytes = spec.record_key.as_deref().and_then(|k| spec.layout.field_value(&self.env, k));
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
        let Some(spec) = self.file_specs.get(&file).cloned() else { return Ok(()); };
        let (op, key_field) = match key {
            Some((op, e)) => (*op, self.expr_to_name(e)),
            None => (cobolt_ast::expr::CmpOp::Eq, spec.record_key.clone().unwrap_or_default()),
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
            let name = self.eval_expr(prog, prog.span())?
                .as_display_string().trim().to_ascii_uppercase();
            if let Some(np) = self.nested_registry.get(&name) {
                for (key, val) in np.local_items.clone() {
                    self.env.set(&key.to_ascii_uppercase(), val);
                }
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
        let prog_name = self.eval_expr(program, span)?
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

            // COBOL-WAIT-EVENT USING event-id control-id
            // GUI mode: block until the UI sends a FormEvent, then populate the two fields.
            // CLI mode: immediately set COBOL-QUIT = 1 so the event loop exits cleanly.
            "COBOL-WAIT-EVENT" | "COBOLT-WAIT-EVENT" => {
                if let Some(rx) = &self.event_rx {
                    // Block the interpreter thread until the UI sends an event.
                    match rx.recv() {
                        Ok(ev) => {
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
                        Err(_) => {
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
                let obj  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let prop = self.eval_call_arg(&using[1], span)?.as_display_string();
                let val  = self.eval_call_arg(&using[2], span)?.as_display_string();
                let obj_t  = obj.trim().to_owned();
                let prop_t = prop.trim().to_owned();
                let val_t  = val.trim().to_owned();
                self.objects.set_property(&obj_t, &prop_t, val_t.clone());
                // GUI mode: notify the UI thread so the form window updates.
                if let Some(tx) = &self.state_tx {
                    let _ = tx.send(StateUpdate::new(obj_t, prop_t, val_t));
                }
            }

            // COBOL-GET-PROPERTY obj prop dest
            "COBOL-GET-PROPERTY" | "COBOLT-GET-PROPERTY" if using.len() >= 3 => {
                let obj  = self.eval_call_arg(&using[0], span)?.as_display_string();
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
            "COBOL-APPEND-FILE" | "COBOLT-APPEND-FILE"
            | "COBOL-WRITE-FILE" | "COBOLT-WRITE-FILE" if using.len() >= 2 => {
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

            // ── Chart runtime calls (stubs — real rendering is in the GUI) ────
            // COBOL-CHART-SET-TABLE chart-id table count
            "COBOL-CHART-SET-TABLE" => {
                tracing::debug!(
                    "COBOL-CHART-SET-TABLE: chart='{}' (CLI mode — rendering skipped)",
                    using.first().map(|a| {
                        self.eval_call_arg(a, span)
                            .map(|v| v.as_display_string())
                            .unwrap_or_default()
                    }).unwrap_or_default()
                );
            }
            // COBOL-CHART-ADD-POINT chart-id label value
            "COBOL-CHART-ADD-POINT" => {
                tracing::debug!("COBOL-CHART-ADD-POINT: CLI mode — skipped");
            }
            // COBOL-CHART-CLEAR chart-id
            "COBOL-CHART-CLEAR" => {
                tracing::debug!("COBOL-CHART-CLEAR: CLI mode — skipped");
            }
            // COBOL-CHART-REFRESH chart-id
            "COBOL-CHART-REFRESH" => {
                tracing::debug!("COBOL-CHART-REFRESH: CLI mode — skipped");
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
                let handle_name  = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name  = self.expr_to_name(call_arg_expr(&using[2]));
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
                let query  = self.eval_call_arg(&using[1], span)?.as_display_string();
                let query  = query.trim().to_owned();
                let count_name  = self.expr_to_name(call_arg_expr(&using[2]));
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
                let handle  = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let col_idx = self.eval_call_arg(&using[1], span)?.as_i64().unwrap_or(1) as usize;
                let dest_name   = self.expr_to_name(call_arg_expr(&using[2]));
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
                let handle    = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let flag_name = self.expr_to_name(call_arg_expr(&using[1]));
                let has_more  = self.db.next_row(handle);
                self.env.set_str(&flag_name, if has_more { "Y" } else { "N" });
            }

            // COBOL-ROW-COUNT USING handle-var, count-var
            //   Stores the total number of rows in the last result set.
            "COBOL-ROW-COUNT" if using.len() >= 2 => {
                let handle     = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
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
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (body, status) = self.http.get(url.trim());
                self.env.set_str(&resp_name, &body);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-POST  USING url-var, body-var, response-var, status-var
            //   Performs an HTTP POST with body-var as the request body.
            //   Content-Type defaults to application/json.
            "COBOL-HTTP-POST" if using.len() >= 4 => {
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.post(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-PUT   USING url-var, body-var, response-var, status-var
            "COBOL-HTTP-PUT" if using.len() >= 4 => {
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.put(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-DELETE  USING url-var, response-var, status-var
            "COBOL-HTTP-DELETE" if using.len() >= 3 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (resp, status) = self.http.delete(url.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-SET-HEADER  USING name-var, value-var
            //   Adds / overwrites a persistent request header sent on every
            //   subsequent COBOL-HTTP-GET / POST / PUT / DELETE call.
            "COBOL-HTTP-SET-HEADER" if using.len() >= 2 => {
                let name  = self.eval_call_arg(&using[0], span)?.as_display_string();
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
                let (para_map, para_order, local_items, params) = {
                    let np = &self.nested_registry[&prog_name];
                    (np.para_map.clone(), np.para_order.clone(),
                     np.local_items.clone(), np.using.clone())
                };

                // Pair each LINKAGE parameter with the caller's argument:
                // (param-key, arg-key, by_reference).
                let bindings: Vec<(String, String, bool)> = params.iter()
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
                let inserted_keys = self.env.push_local_scope(&local_items);

                // Copy-in: bind each parameter to the caller argument's value
                // (after the local scope so it overrides the LINKAGE default).
                for (pk, ak, _) in &bindings {
                    if let Some(v) = self.env.get(ak).cloned() {
                        self.env.set(pk, v);
                    }
                }

                // Run the nested program's paragraphs in declaration order.
                let result = self.run_para_sequence(&para_map, &para_order);

                // Copy-out: BY REFERENCE arguments receive the parameter's final
                // value (BY CONTENT / BY VALUE are not written back).
                for (pk, ak, by_ref) in &bindings {
                    if *by_ref {
                        if let Some(v) = self.env.get(pk).cloned() {
                            self.env.set(ak, v);
                        }
                    }
                }

                // Remove the nested program's local items regardless of outcome.
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

    fn eval_call_arg(&mut self, arg: &CallArg, span: Span) -> Result<CobolValue, RuntimeError> {
        self.eval_expr(call_arg_expr(arg), span)
    }

    // ── Expression evaluation ─────────────────────────────────────────────────

    /// Evaluate an expression to a `CobolValue`.
    /// Resolve a PowerCOBOL-style property reference to `(control, property-key)`,
    /// evaluating any subscripts. A nested path becomes a composite key, e.g.
    /// `"Text" OF "ListItems" (4) OF Listview1` → `("Listview1", "ListItems(4).Text")`.
    fn property_ref_key(
        &mut self,
        control: &str,
        path: &[cobolt_ast::expr::PropSeg],
        span: Span,
    ) -> (String, String) {
        let mut parts: Vec<String> = Vec::with_capacity(path.len());
        for seg in path {
            match &seg.index {
                Some(idx) => {
                    let i = self.eval_expr(idx, span).ok().and_then(|v| v.as_i64()).unwrap_or(0);
                    parts.push(format!("{}({})", seg.name, i));
                }
                None => parts.push(seg.name.clone()),
            }
        }
        (control.trim().to_owned(), parts.join("."))
    }

    // ── Visual-object method dispatch (INVOKE / obj::method) ────────────────────

    /// Set a control property and notify the UI thread (auto-registers the
    /// object so the change is never silently dropped).
    fn obj_set(&mut self, obj: &str, prop: &str, val: String) {
        if !self.objects.contains(obj) {
            self.objects.register(obj, "Control");
        }
        self.objects.set_property(obj, prop, val.clone());
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(StateUpdate::new(obj.to_string(), prop.to_string(), val));
        }
    }

    /// Read a control property as a string (`""` when unset).
    fn obj_get(&self, obj: &str, prop: &str) -> String {
        self.objects.get_property(obj, prop).map(|v| v.to_string()).unwrap_or_default()
    }

    /// Execute a widget method (`obj::method(args)` / `INVOKE obj "method"`).
    /// Most methods are thin sugar over property get/set — which the form
    /// runtime mirrors to the live UI — and getters return a value (for the
    /// expression form and `RETURNING`).
    fn exec_method(&mut self, object: &str, method: &str, args: &[CobolValue]) -> CobolValue {
        let obj = object.trim();
        let m   = method.to_ascii_uppercase();
        let arg = |i: usize| args.get(i)
            .map(|v| v.as_display_string().trim().to_string()).unwrap_or_default();
        let val = |s: String| { let n = s.len(); CobolValue::from_str(&s, n) };
        let truthy = |s: &str| {
            let t = s.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
                || t.eq_ignore_ascii_case("on")
        };
        let b01 = |s: &str| if truthy(s) { "1".to_string() } else { "0".to_string() };
        let none = CobolValue::from_str("", 0);
        let parse_i = |s: String| s.trim().parse::<i64>().unwrap_or(0);

        match m.as_str() {
            // ── Universal lifecycle / visibility ──
            "SHOW"     => { self.obj_set(obj, "Visible", "1".into()); none }
            "HIDE"     => { self.obj_set(obj, "Visible", "0".into()); none }
            "ENABLE"   => { self.obj_set(obj, "Enabled", "1".into()); none }
            "DISABLE"  => { self.obj_set(obj, "Enabled", "0".into()); none }
            "SETFOCUS" | "FOCUS" => { self.obj_set(obj, "Focused", "1".into()); none }
            "BRINGTOFRONT" => { self.obj_set(obj, "ZOrder", "10000".into()); none }
            "SENDTOBACK"   => { self.obj_set(obj, "ZOrder", "-10000".into()); none }
            "REFRESH" | "VALIDATE" => none,
            // ── Geometry ──
            "MOVETO" => { self.obj_set(obj, "X", arg(0)); self.obj_set(obj, "Y", arg(1)); none }
            "RESIZE" => { self.obj_set(obj, "Width", arg(0)); self.obj_set(obj, "Height", arg(1)); none }
            // ── Generic property access ──
            "SETPROPERTY" => { let p = arg(0); self.obj_set(obj, &p, arg(1)); none }
            "GETPROPERTY" => val(self.obj_get(obj, &arg(0))),
            // ── Text / caption ──
            "SETCAPTION" => { self.obj_set(obj, "Caption", arg(0)); none }
            "SETTEXT"    => { self.obj_set(obj, "Text", arg(0)); none }
            "GETCAPTION" => val(self.obj_get(obj, "Caption")),
            "GETTEXT"    => val(self.obj_get(obj, "Text")),
            "APPENDTEXT" => { let cur = self.obj_get(obj, "Text"); self.obj_set(obj, "Text", format!("{cur}{}", arg(0))); none }
            "SETCOLOR"   => { self.obj_set(obj, "ForegroundColor", arg(0)); none }
            "SELECTALL"  => none,
            "CLEAR"      => { self.obj_set(obj, "Text", String::new()); self.obj_set(obj, "Items", String::new()); none }
            // ── Checkbox / radio ──
            "ISCHECKED"  => val(b01(&self.obj_get(obj, "Checked"))),
            "SETCHECKED" => { let v = b01(&arg(0)); self.obj_set(obj, "Checked", v); none }
            "SELECT"     => { self.obj_set(obj, "Checked", "1".into()); none }
            "TOGGLE"     => { let c = self.obj_get(obj, "Checked"); let nv = if truthy(&c) { "0" } else { "1" }; self.obj_set(obj, "Checked", nv.into()); none }
            // ── Numeric value (progress/slider/numeric/datetime) ──
            "SETVALUE"  => { self.obj_set(obj, "Value", arg(0)); none }
            "GETVALUE"  => val(self.obj_get(obj, "Value")),
            "INCREMENT" => { let st = parse_i(self.obj_get(obj, "Step")); let st = if st == 0 { 1 } else { st }; let v = parse_i(self.obj_get(obj, "Value")); self.obj_set(obj, "Value", (v + st).to_string()); none }
            "DECREMENT" => { let st = parse_i(self.obj_get(obj, "Step")); let st = if st == 0 { 1 } else { st }; let v = parse_i(self.obj_get(obj, "Value")); self.obj_set(obj, "Value", (v - st).to_string()); none }
            "RESET"     => { let min = self.obj_get(obj, "Minimum"); let m2 = if min.trim().is_empty() { "0".to_string() } else { min }; self.obj_set(obj, "Value", m2); none }
            // ── Items (list / combo) ──
            "ADDITEM" => { let cur = self.obj_get(obj, "Items"); let nv = if cur.is_empty() { arg(0) } else { format!("{cur}\n{}", arg(0)) }; self.obj_set(obj, "Items", nv); none }
            "REMOVEITEM" => { let idx = arg(0).trim().parse::<usize>().unwrap_or(usize::MAX); let cur = self.obj_get(obj, "Items"); let mut lines: Vec<String> = cur.lines().map(|l| l.to_string()).collect(); if idx < lines.len() { lines.remove(idx); } self.obj_set(obj, "Items", lines.join("\n")); none }
            "GETSELECTED" => val(self.obj_get(obj, "Value")),
            "GETSELECTEDINDEX" | "GETINDEX" => val(self.obj_get(obj, "SelectedIndex")),
            "SETSELECTEDINDEX" | "SETINDEX" => { self.obj_set(obj, "SelectedIndex", arg(0)); none }
            "GETCOUNT" => { let cur = self.obj_get(obj, "Items"); let n = if cur.trim().is_empty() { 0 } else { cur.lines().count() }; val(n.to_string()) }
            // ── Timer ──
            "START" => { self.obj_set(obj, "Enabled", "1".into()); none }
            "STOP"  => { self.obj_set(obj, "Enabled", "0".into()); none }
            "SETINTERVAL" => { self.obj_set(obj, "Interval", arg(0)); none }
            "ISENABLED" => val(b01(&self.obj_get(obj, "Enabled"))),
            // ── Animation ──
            "PLAYANIMATION" | "PLAY" => { let a = if args.is_empty() { "1".to_string() } else { arg(0) }; self.obj_set(obj, "_PlayAnimation", a); none }
            "STOPANIMATION" => { self.obj_set(obj, "_StopAnimation", "1".into()); none }
            "PAUSE" => { self.obj_set(obj, "_PauseAnimation", "1".into()); none }
            // ── ModalWindow / AgentObject extras ──
            "CLOSE" => {
                // SqlDatabase::Close closes the connection; otherwise hide a window.
                let h = parse_i(self.obj_get(obj, "_Handle"));
                if h > 0 { self.db.close(h as u32); }
                else     { self.obj_set(obj, "Visible", "0".into()); }
                none
            }
            "GETRESULT" => val(self.obj_get(obj, "Result")),
            "SETTITLE"  => { self.obj_set(obj, "Title", arg(0)); none }
            // AgentObject (LLM): prompt/model are stored; Ask records the prompt
            // and returns the last reply property (filled by the host LLM bridge).
            "SETPROMPT" => { self.obj_set(obj, "SystemPrompt", arg(0)); none }
            "SETMODEL"  => { self.obj_set(obj, "Model", arg(0)); none }
            "ASK"       => { self.obj_set(obj, "Prompt", arg(0)); val(self.obj_get(obj, "LastReply")) }
            // ── REST / HTTP client ──
            "GET" => { let (b, st) = self.http.get(&arg(0)); self.obj_set(obj, "ResponseBody", b.clone()); self.obj_set(obj, "StatusCode", st.to_string()); val(b) }
            "POST" => { let (b, st) = self.http.post(&arg(0), &arg(1)); self.obj_set(obj, "ResponseBody", b.clone()); self.obj_set(obj, "StatusCode", st.to_string()); val(b) }
            "PUT" => { let (b, st) = self.http.put(&arg(0), &arg(1)); self.obj_set(obj, "ResponseBody", b.clone()); self.obj_set(obj, "StatusCode", st.to_string()); val(b) }
            "DELETE" => { let (b, st) = self.http.delete(&arg(0)); self.obj_set(obj, "ResponseBody", b.clone()); self.obj_set(obj, "StatusCode", st.to_string()); val(b) }
            "CALL" => {
                let verb = arg(0).to_ascii_uppercase();
                let (b, st) = match verb.as_str() {
                    "POST"   => self.http.post(&arg(1), &arg(2)),
                    "PUT"    => self.http.put(&arg(1), &arg(2)),
                    "DELETE" => self.http.delete(&arg(1)),
                    _        => self.http.get(&arg(1)),
                };
                self.obj_set(obj, "ResponseBody", b.clone());
                self.obj_set(obj, "StatusCode", st.to_string());
                val(b)
            }
            "SETHEADER"    => { self.http.set_header(arg(0), arg(1)); none }
            "CLEARHEADERS" => { self.http.clear_headers(); none }
            "SETTIMEOUT"   => { self.obj_set(obj, "Timeout", arg(0)); none }
            // ── SQL database ──
            "OPEN" => {
                match self.db.open(&arg(0)) {
                    Ok(h)  => { self.obj_set(obj, "_Handle", h.to_string()); self.obj_set(obj, "StatusCode", "0".into()); val(h.to_string()) }
                    Err(e) => { self.obj_set(obj, "LastError", e); self.obj_set(obj, "StatusCode", "1".into()); val("0".to_string()) }
                }
            }
            "EXECUTE" | "EXEC" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                match self.db.exec(h, &arg(0)) {
                    Ok(n)  => val(n.to_string()),
                    Err(e) => { self.obj_set(obj, "LastError", e); val("0".to_string()) }
                }
            }
            "QUERY" => {
                let h = parse_i(self.obj_get(obj, "_Handle")) as u32;
                match self.db.exec(h, &arg(0)) {
                    Ok(_)  => val(self.db.row_count(h).to_string()),
                    Err(e) => { self.obj_set(obj, "LastError", e); val("0".to_string()) }
                }
            }
            "FETCH"    => { let h = parse_i(self.obj_get(obj, "_Handle")) as u32; val(if self.db.next_row(h) { "1" } else { "0" }.to_string()) }
            "FETCHALL" => { let h = parse_i(self.obj_get(obj, "_Handle")) as u32; val(self.db.row_count(h).to_string()) }
            // ── Unknown method: no-op ──
            _ => none,
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr, span: Span) -> Result<CobolValue, RuntimeError> {
        match expr {
            Expr::Literal(lit, _) => Ok(literal_to_value(lit)),

            // PowerCOBOL-style property reference as a *sending* operand: read the
            // control's current property value (a string — its type is inferred,
            // so no temporary data item is needed).
            Expr::PropertyRef { control, path, span: s } => {
                let (ctrl, key) = self.property_ref_key(control, path, *s);
                let val_s = self.objects.get_property(&ctrl, &key)
                    .map(|pv| pv.to_string())
                    .unwrap_or_default();
                // A purely numeric property (X, Width, Value, …) evaluates as a
                // NUMBER, so comparisons and arithmetic are algebraic —
                // otherwise `IF "X" OF A > "X" OF B` would compare the digit
                // strings character by character ("232" < "64").
                if let Some(num) = crate::value::parse_decimal(val_s.trim()) {
                    if !val_s.trim().is_empty() {
                        return Ok(CobolValue::Numeric(num));
                    }
                }
                let n = val_s.len();
                Ok(CobolValue::from_str(&val_s, n))
            }

            // Visual-object method call as a value: `obj::GetText()`.
            Expr::MethodCall { object, method, args, span: s } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args { vals.push(self.eval_expr(a, *s)?); }
                Ok(self.exec_method(object, method, &vals))
            }

            Expr::Identifier(name, _) => {
                let key = self.env.resolve_name(name, &[]);
                // A 66-level RENAMES item synthesizes its value from the items
                // it regroups.
                if self.env.is_renames(&key) {
                    let s = self.env.renames_value(&key).unwrap_or_default();
                    let n = s.len();
                    return Ok(CobolValue::from_str(&s, n));
                }
                Ok(self.env.get(&key).cloned().unwrap_or_else(|| {
                    tracing::debug!("Identifier '{key}' not found in environment — using 0");
                    CobolValue::from_i64(0)
                }))
            }

            Expr::Qualified { name, of, .. } => {
                let quals = collect_quals(of);
                let key = self.env.resolve_name(name, &quals);
                Ok(self.env.get(&key).cloned().unwrap_or(CobolValue::from_i64(0)))
            }

            Expr::Subscript { base, indices, span: s } => {
                // Table reference `t(i[,j…])` → the occurrence's storage slot.
                let base_name = self.expr_to_name(base);
                let idx = self.eval_indices(indices, *s);
                let key = crate::environment::subscript_key(&base_name, &idx);
                Ok(self.env.get(&key).cloned().unwrap_or_else(|| CobolValue::from_i64(0)))
            }

            Expr::RefMod { base, start, length, span: s } => {
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

            Expr::FunctionCall { name, args, span: s } =>
                self.eval_function(name, args, *s),

            Expr::Arithmetic { op, lhs, rhs, span: s } => {
                let l = self.eval_expr(lhs, *s)?;
                let r = self.eval_expr(rhs, *s)?;
                let result = match op {
                    ArithOp::Add => l.add_val(&r),
                    ArithOp::Sub => l.sub_val(&r),
                    ArithOp::Mul => l.mul_val(&r),
                    ArithOp::Div => l.div_val(&r)
                        .ok_or(RuntimeError::DivisionByZero { span: *s })?,
                    // Exponentiation is inherently floating-point.
                    ArithOp::Pow => CobolValue::from_f64(l.as_f64().powf(r.as_f64())),
                };
                Ok(result)
            }

            Expr::Unary { op, operand, span: s } => {
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
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .to_ascii_uppercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "LOWER-CASE" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .to_ascii_lowercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "NUMVAL" | "NUMVAL-C" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let f: f64 = s.trim()
                    .replace(',', "")
                    .replace('$', "")
                    .replace('£', "")
                    .parse()
                    .unwrap_or(0.0);
                Ok(CobolValue::from_f64(f))
            }
            "MAX" => {
                let vals = self.eval_args(args, span)?;
                let max = vals.iter().map(|v| v.as_f64()).fold(f64::NEG_INFINITY, f64::max);
                Ok(CobolValue::from_f64(max))
            }
            "MIN" => {
                let vals = self.eval_args(args, span)?;
                let min = vals.iter().map(|v| v.as_f64()).fold(f64::INFINITY, f64::min);
                Ok(CobolValue::from_f64(min))
            }
            "SQRT" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(v.sqrt()))
            }
            "MOD" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 { return Err(RuntimeError::DivisionByZero { span }); }
                Ok(CobolValue::from_f64(a - (a / b).floor() * b))
            }
            "REM" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 { return Err(RuntimeError::DivisionByZero { span }); }
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
            "RANDOM" => Ok(CobolValue::from_f64(pseudo_random())),
            "CURRENT-DATE" => {
                let s = current_date_string();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "TRIM" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .trim().to_owned();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "REVERSE" => {
                let s: String = self.eval_expr(&args[0], span)?.as_display_string()
                    .chars().rev().collect();
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
                    vals.iter().enumerate().max_by(|a, b| cmp(a.1.as_f64(), b.1.as_f64()))
                } else {
                    vals.iter().enumerate().min_by(|a, b| cmp(a.1.as_f64(), b.1.as_f64()))
                };
                Ok(CobolValue::from_i64(pick.map(|(i, _)| i as i64 + 1).unwrap_or(0)))
            }
            // ── Statistics over the argument list ─────────────────────────────
            "SUM" => {
                let mut total = CobolValue::from_i64(0);
                for v in self.eval_args(args, span)? { total = total.add_val(&v); }
                Ok(total)
            }
            "MEAN" => {
                let vals = self.eval_args(args, span)?;
                if vals.is_empty() { return Ok(CobolValue::from_i64(0)); }
                let s: f64 = vals.iter().map(|v| v.as_f64()).sum();
                Ok(CobolValue::from_f64(s / vals.len() as f64))
            }
            "MEDIAN" => {
                let mut xs: Vec<f64> = self.eval_args(args, span)?.iter().map(|v| v.as_f64()).collect();
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let m = if xs.is_empty() { 0.0 }
                    else if xs.len() % 2 == 1 { xs[xs.len() / 2] }
                    else { (xs[xs.len() / 2 - 1] + xs[xs.len() / 2]) / 2.0 };
                Ok(CobolValue::from_f64(m))
            }
            "MIDRANGE" | "RANGE" => {
                let xs: Vec<f64> = self.eval_args(args, span)?.iter().map(|v| v.as_f64()).collect();
                let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let r = if name.eq_ignore_ascii_case("RANGE") { hi - lo } else { (lo + hi) / 2.0 };
                Ok(CobolValue::from_f64(r))
            }
            "VARIANCE" | "STANDARD-DEVIATION" => {
                let xs: Vec<f64> = self.eval_args(args, span)?.iter().map(|v| v.as_f64()).collect();
                if xs.is_empty() { return Ok(CobolValue::from_i64(0)); }
                let mean = xs.iter().sum::<f64>() / xs.len() as f64;
                let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
                Ok(CobolValue::from_f64(if name.eq_ignore_ascii_case("VARIANCE") { var } else { var.sqrt() }))
            }
            // ── Math ──────────────────────────────────────────────────────────
            "FACTORIAL" => {
                let n = self.eval_expr(&args[0], span)?.as_i64().unwrap_or(0).max(0);
                let mut f: i128 = 1;
                for k in 2..=n as i128 { f *= k; }
                Ok(CobolValue::from_i64(f as i64))
            }
            "SIN"   => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().sin())),
            "COS"   => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().cos())),
            "TAN"   => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().tan())),
            "ASIN"  => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().asin())),
            "ACOS"  => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().acos())),
            "ATAN"  => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().atan())),
            "LOG"   => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().ln())),
            "LOG10" => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().log10())),
            "EXP"   => Ok(CobolValue::from_f64(self.eval_expr(&args[0], span)?.as_f64().exp())),
            "EXP10" => Ok(CobolValue::from_f64(10f64.powf(self.eval_expr(&args[0], span)?.as_f64()))),
            "PI"    => Ok(CobolValue::from_f64(std::f64::consts::PI)),
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
                    if n == 0.0 { 0.0 } else { 1.0 / n }
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
                } else { 50 };
                let cur_year = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let days = SystemTime::now().duration_since(UNIX_EPOCH)
                        .unwrap_or_default().as_secs() / 86400;
                    ymd_from_days(days).0 as i64
                };
                let max_year = cur_year + window;
                let mut yyyy = (max_year / 100) * 100 + yy;
                if yyyy > max_year { yyyy -= 100; }
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
                    && t.chars().all(|c| c.is_ascii_digit()
                        || matches!(c, '.' | '+' | '-' | ',' | ' '));
                if ok && t.parse::<f64>().is_ok() {
                    Ok(CobolValue::from_i64(0))
                } else {
                    let pos = t.chars()
                        .position(|c| !(c.is_ascii_digit()
                            || matches!(c, '.' | '+' | '-' | ',' | ' ')))
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
            Condition::And(a, b, _)  => Ok(self.eval_condition(a)? && self.eval_condition(b)?),
            Condition::Or(a, b, _)   => Ok(self.eval_condition(a)? || self.eval_condition(b)?),
            Condition::ClassTest { expr, negated, class, span } => {
                let v = self.eval_expr(expr, *span)?;
                let s = v.as_display_string();
                let result = match class {
                    DataClass::Numeric => v.is_numeric()
                        || s.trim().parse::<f64>().is_ok(),
                    DataClass::Alphabetic =>
                        s.chars().all(|c| c.is_ascii_alphabetic() || c == ' '),
                    DataClass::AlphabeticLower =>
                        s.chars().all(|c| c.is_ascii_lowercase() || c == ' '),
                    DataClass::AlphabeticUpper =>
                        s.chars().all(|c| c.is_ascii_uppercase() || c == ' '),
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::SignTest { expr, negated, sign, span } => {
                let v = self.eval_expr(expr, *span)?.as_f64();
                let result = match sign {
                    SignCond::Positive => v > 0.0,
                    SignCond::Negative => v < 0.0,
                    SignCond::Zero     => v == 0.0,
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::ConditionName(name, _) => {
                use cobolt_ast::data::ConditionValue;
                // 88-level condition-name: true when the parent (host) item holds
                // one of the declared VALUEs (or falls within a THRU range).
                if let Some(info) = self.env.cond_name(name).cloned() {
                    let pv = self.env.get(&info.parent).cloned()
                        .unwrap_or_else(|| CobolValue::from_i64(0));
                    for cv in &info.values {
                        let hit = match cv {
                            ConditionValue::Single(lit) =>
                                compare_values(&pv, &literal_to_value(lit), CmpOp::Eq),
                            ConditionValue::Range(lo, hi) =>
                                compare_values(&pv, &literal_to_value(lo), CmpOp::Ge)
                                    && compare_values(&pv, &literal_to_value(hi), CmpOp::Le),
                        };
                        if hit { return Ok(true); }
                    }
                    return Ok(false);
                }
                // Fallback (undeclared): truthy if the slot is non-zero/non-space.
                let upper = name.to_ascii_uppercase();
                let v = self.env.get(&upper).cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                Ok(!v.is_zero())
            }

            Condition::NameOrAbbrev { subject, op, name, span } => {
                // `a = b OR c`: if `c` is a known 88-level condition-name, treat
                // it as one; otherwise it is the abbreviation object `a = c`.
                if self.env.cond_name(name).is_some() {
                    return self.eval_condition(&Condition::ConditionName(name.clone(), *span));
                }
                let l = self.eval_expr(subject, *span)?;
                let key = self.env.resolve_name(name, &[]);
                let r = self.env.get(&key).cloned().unwrap_or_else(|| CobolValue::from_i64(0));
                Ok(compare_values(&l, &r, *op))
            }
        }
    }

    // ── Paragraph / section helpers ───────────────────────────────────────────

    fn para_stmts(&self, name: &str, span: Span) -> Result<Vec<Stmt>, RuntimeError> {
        let upper = name.to_ascii_uppercase();
        self.para_map.get(&upper)
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
        let to_u   = to.to_ascii_uppercase();
        let from_pos = self.para_order.iter().position(|n| n == &from_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph { name: from_u.clone(), span })?;
        let to_pos = self.para_order.iter().position(|n| n == &to_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph { name: to_u.clone(), span })?;

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
        indices.iter()
            .map(|e| self.eval_expr(e, span).ok().and_then(|v| v.as_i64()).unwrap_or(1))
            .collect()
    }

    /// Resolve an assignment target to its storage key, evaluating subscripts.
    /// (`RefMod` targets are handled separately by `assign_refmod`.)
    fn resolve_lvalue(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Subscript { base, indices, span } => {
                let base_name = self.expr_to_name(base);
                let idx = self.eval_indices(indices, *span);
                crate::environment::subscript_key(&base_name, &idx)
            }
            // PowerCOBOL-style property receiver, usable by ANY verb: shadow the
            // property as a synthetic env item preloaded with its current value;
            // `flush_property_shadows` (run after each statement) writes it back.
            Expr::PropertyRef { control, path, span } => {
                let (ctrl, key) = self.property_ref_key(control, path, *span);
                let synth = format!("\u{1}PROP\u{1}{ctrl}\u{1}{key}");
                let cur = self.objects.get_property(&ctrl, &key)
                    .map(|pv| pv.to_string())
                    .unwrap_or_default();
                // Generous capacity so arithmetic / STRING results into a
                // property are never truncated by the shadow field's width.
                self.env.set(&synth, CobolValue::from_str(&cur, cur.len().max(128)));
                self.property_shadows.insert(synth.clone(), (ctrl, key));
                synth
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
        let shadows: Vec<(String, (String, String))> =
            self.property_shadows.drain().collect();
        for (synth, (ctrl, key)) in shadows {
            let v = self.env.get(&synth)
                .map(|cv| cv.as_display_string())
                .unwrap_or_default()
                .trim()
                .to_owned();
            if !self.objects.contains(&ctrl) {
                self.objects.register(&ctrl, "Control");
            }
            self.objects.set_property(&ctrl, &key, v.clone());
            if let Some(tx) = &self.state_tx {
                let _ = tx.send(StateUpdate::new(ctrl, key, v));
            }
        }
    }

    /// Extract the canonical storage key for an lvalue expression, resolving
    /// any `OF`/`IN` qualification to disambiguate duplicated names.
    fn expr_to_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name, _)  => self.env.resolve_name(name, &[]),
            Expr::Qualified { name, of, .. } => {
                let quals = collect_quals(of);
                self.env.resolve_name(name, &quals)
            }
            Expr::Subscript { base, .. } => self.expr_to_name(base),
            Expr::RefMod { base, .. }    => self.expr_to_name(base),
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
        let mut cur = self.env.display_string(&name).unwrap_or_default().into_bytes();
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

fn find_decl<'a>(d: &'a cobolt_ast::data::DataDecl, name: &str) -> Option<&'a cobolt_ast::data::DataDecl> {
    if d.name.as_deref().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false) {
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
    let mut map:   IndexMap<String, Vec<Stmt>> = IndexMap::new();
    let mut order: Vec<String>                 = Vec::new();

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
        Literal::Float(f)   => CobolValue::from_f64(*f),
        Literal::Decimal(m, s) => CobolValue::Numeric(CobolNumeric::new(*m, *s)),
        Literal::String(s)  => CobolValue::from_str(s, s.len()),
        Literal::Figurative(fig) => match fig {
            FigurativeConstant::Zero     => CobolValue::from_i64(0),
            FigurativeConstant::Space    => CobolValue::spaces(1),
            FigurativeConstant::HighValue => CobolValue::figurative_high_values(1),
            FigurativeConstant::LowValue  => CobolValue::figurative_low_values(1),
            FigurativeConstant::Quote    => CobolValue::from_str("\"", 1),
            FigurativeConstant::Null     => CobolValue::from_i64(0),
            FigurativeConstant::All(inner) => literal_to_value(inner),
        },
    }
}

/// Flatten an `AND`-chain of equality comparisons into `(lhs, rhs, span)`
/// tuples in major-to-minor (textual) order. Used by the `SEARCH ALL` binary
/// search to find the discriminating key when a WHEN does not match at `mid`.
fn flatten_eq_comparisons<'a>(
    cond: &'a Condition,
    out: &mut Vec<(&'a Expr, &'a Expr, Span)>,
) {
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

/// ANSI SGR prefix for a screen phrase's display attributes (`""` if none).
fn screen_attrs(sc: &cobolt_ast::stmt::ScreenPhrase) -> String {
    let mut s = String::new();
    if sc.highlight { s.push_str("\x1b[1m"); }
    if sc.reverse   { s.push_str("\x1b[7m"); }
    if sc.underline { s.push_str("\x1b[4m"); }
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
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600)  / 60;
    let s = secs % 60;
    format!("{h:02}{m:02}{s:02}00")
}

/// Days in `month` (1–12) of `year`, accounting for leap years.
fn cob_days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(year.max(0) as u64) { 29 } else { 28 },
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
        if rem > dy { rem -= dy; y += 1; } else { break; }
    }
    let mut m = 1i64;
    loop {
        let dm = cob_days_in_month(y, m);
        if rem > dm { rem -= dm; m += 1; } else { break; }
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
        if rem > dy { rem -= dy; y += 1; } else { break; }
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
    if is_leap(year) { 366 } else { 365 }
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
        if days < dy { break; }
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
        if days < *md { break; }
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

/// Return a pseudo-random `f64` in [0, 1).
fn pseudo_random() -> f64 {
    use std::sync::atomic::Ordering;
    let s = RANDOM_STATE.load(Ordering::Relaxed);
    let next = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    RANDOM_STATE.store(next, Ordering::Relaxed);
    // Top 53 bits → double in [0, 1)
    (next >> 11) as f64 / (1u64 << 53) as f64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_ast::expr::{CmpOp, Literal};

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
        let b = CobolValue::from_str("BETA",  4);
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
}
