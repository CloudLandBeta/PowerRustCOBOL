// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`prc-runner`, the PowerRustCOBOL runner** (DESKTOP-WASM-001 T15, plan §1.1 and §4).
//!
//! A Desktop program built on the WebAssembly path is a copy of this executable with the program
//! inside it (T14). When it starts, the runner:
//!
//! 1. reads the payload from its own file (`PRLOC` overlay, or the `__PRPAYLOAD` segment);
//! 2. reads the module's manifest and accepts every ABI from 1 to [`ABI_VERSION`](cobolt_wasm_abi::codes::ABI_VERSION) (R5, C2);
//! 3. refuses, naming them, the capabilities and imports it does not provide (R4), in the system's
//!    language ([`messages`]);
//! 4. loads the Build's machine code, or the cache's, or compiles at load (T13, R16);
//! 5. links exactly the imports the module declares ([`linker_for`]): `pr` for the run, and the
//!    console part of `prn` (A27, A29, A30);
//! 6. runs the program to its end and exits with the code it handed to `prn.exit` (0 when none).
//!
//! A module with forms (its `prforms` section is not empty) is a form application ([`app`], T30):
//! its windows are the form host's, the very windows a program built on the Cargo path shows, and
//! its programs run on one program thread, driven by the ABI host the browser's player uses
//! ([`forms`]). That links the form host, the forms' painter and the interpreter's runtime crate,
//! whose clock and channels the form host shares; the interpreter itself never runs here.
//!
//! What `DISPLAY` writes goes to standard output, a line per command; `ACCEPT` from the console
//! reads standard input; the environment and the arguments are this process's own. The clock and
//! the random source honour the test pins ([`pins`], R28).
//!
//! The runner never names itself after its crate: its words are "the PowerRustCOBOL runner".

#[cfg(feature = "forms")]
mod agent;
#[cfg(feature = "forms")]
pub mod app;
#[cfg(feature = "forms")]
mod audio;
#[cfg(feature = "forms")]
mod aws;
pub mod backend;
pub mod files;
#[cfg(feature = "forms")]
pub mod forms;
#[cfg(feature = "forms")]
mod gfx;
mod http;
#[cfg(feature = "forms")]
mod kb;
#[cfg(feature = "forms")]
mod rest;
pub mod messages;
pub mod pins;
mod partiql;
mod sql;
#[cfg(feature = "forms")]
mod viewer;
pub mod verbs;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use cobolt_program_package::{Kind, PackageError};
use cobolt_runner_engine::{Cache, MachineCode};
use cobolt_wasm_abi::batch::{decode_batch, Command};
use cobolt_wasm_abi::codes::{cap, IMPORT_MODULE, NATIVE_IMPORT_MODULE};
use cobolt_wasm_abi::manifest::{Manifest, ManifestError, MANIFEST_SECTION};
use wasmtime::{Caller, Engine, Linker, Memory, Store};

pub use messages::{Lang, Msg};
pub use pins::Pins;

/// The capabilities this runner provides (A27). T15: console programs; T32: the file dialogs (A33);
/// T38: files on disk of every organisation (A34); T49: the sound player (A36); T48: the AWS controls (A38);
/// T39c: the text files and pictures a built-in names by its path (A39); T41: `EXEC SQL` (A41);
/// T42: the HTTP built-ins (A45); T47b: the KnowledgeBase control (A49); T47c: the AgentObject (A50);
/// T48: the PartiQL built-ins on the program's own files (A56); T49: the animation sequences and the
/// layout scripts (A57).
#[cfg(feature = "forms")]
pub const PROVIDED_CAPABILITIES: &[&str] = &[
    cap::CONSOLE,
    cap::ENV,
    cap::DIALOGS,
    cap::AUDIO,
    cap::AWS,
    cap::FILES_SEQUENTIAL,
    cap::FILES_LINE_SEQUENTIAL,
    cap::FILES_RELATIVE,
    cap::FILES_INDEXED,
    cap::FILES_TEXT,
    cap::IMAGES,
    cap::SQL,
    cap::HTTP,
    cap::KB,
    cap::AGENT,
    cap::AGENT_FILES,
    cap::AGENT_KB,
    cap::MODELS,
    cap::MCP_SEARCH,
    cap::PARTIQL,
    cap::PARTIQL_FILES,
    cap::SEQUENCES,
    cap::LAYOUTS,
    cap::VIEWER,
];

/// What a runner with no windows always provides (spec 114 R5, R6, T17), the **core** group: the console, the environment,
/// the files of every organisation, the text files and pictures a built-in names, the HTTP built-ins and the PartiQL
/// built-ins on the program's own files.
pub const CORE_CAPABILITIES: &[&str] = &[
    cap::CONSOLE,
    cap::ENV,
    cap::FILES_SEQUENTIAL,
    cap::FILES_LINE_SEQUENTIAL,
    cap::FILES_RELATIVE,
    cap::FILES_INDEXED,
    cap::FILES_TEXT,
    cap::IMAGES,
    cap::HTTP,
    cap::PARTIQL,
    cap::PARTIQL_FILES,
];

/// What a program with no window can use, by the groups this build has: [`CORE_CAPABILITIES`], plus `EXEC SQL` and the
/// database objects with the feature `sql`. A module that declares another capability is refused when it starts, naming it.
pub fn headless_capabilities() -> Vec<&'static str> {
    let mut list = CORE_CAPABILITIES.to_vec();
    if cfg!(feature = "sql") {
        list.push(cap::SQL);
    }
    list
}

/// Publish the SQL connections of the `sql-connections.toml` beside the running executable, as a Cargo-built program does
/// when it starts (the `<APP>_SQL_<NAME>_*` variables override them, `app_name` being the application's name). A Backend
/// runner calls this before it serves, so that `CONNECT TO 'NAME'` and `sql-connection:NAME` resolve (spec 114 T23).
pub fn install_sql_connections(app_name: &str) {
    sql::install_connections(app_name);
}

/// [`install_sql_connections`] for the `sql-connections.toml` in `dir`.
pub fn install_sql_connections_in(dir: &std::path::Path, app_name: &str) {
    sql::install_connections_in(dir, app_name);
}

/// Every import this runner provides, with the ABI that introduced it: `(abi, module, name)`. A row
/// is never removed, so a module of any earlier ABI finds every import it was built with (R5).
/// Not yet provided: `pr.remote` (the backend), and the conformance imports, which a desktop Build
/// never makes.
pub const PROVIDED_IMPORTS: &[(u32, &str, &str)] = &[
    (1, IMPORT_MODULE, "flush"),
    (1, IMPORT_MODULE, "abort"),
    (1, IMPORT_MODULE, "now"),
    (1, IMPORT_MODULE, "random"),
    // A form module asks the ABI host for what the program reads (T30).
    (1, IMPORT_MODULE, "query"),
    (1, NATIVE_IMPORT_MODULE, "con_read"),
    (1, NATIVE_IMPORT_MODULE, "env_get"),
    (1, NATIVE_IMPORT_MODULE, "exit"),
    (1, NATIVE_IMPORT_MODULE, "tz_offset"),
    // T38: files on disk (A34).
    (1, NATIVE_IMPORT_MODULE, "f_open"),
    (1, NATIVE_IMPORT_MODULE, "f_close"),
    (1, NATIVE_IMPORT_MODULE, "f_read"),
    (1, NATIVE_IMPORT_MODULE, "f_write"),
    (1, NATIVE_IMPORT_MODULE, "f_rewrite"),
    (1, NATIVE_IMPORT_MODULE, "f_delete"),
    (1, NATIVE_IMPORT_MODULE, "f_start"),
    (1, NATIVE_IMPORT_MODULE, "f_tx"),
    // T39c: APPEND-FILE and WRITE-FILE, IMAGE-FIT and IMAGE-INFO (A39).
    (1, NATIVE_IMPORT_MODULE, "t_write"),
    (1, NATIVE_IMPORT_MODULE, "img_fit"),
    (1, NATIVE_IMPORT_MODULE, "img_info"),
    // T41: `EXEC SQL` (A41).
    (1, NATIVE_IMPORT_MODULE, "sql_exec"),
    // T41b: the SQL built-ins (A42).
    (1, NATIVE_IMPORT_MODULE, "sql_db"),
    // T41d: `COBOL::SqlDatabase` (A44).
    (1, NATIVE_IMPORT_MODULE, "sql_obj"),
    // T42: the HTTP built-ins (A45).
    (1, NATIVE_IMPORT_MODULE, "http_call"),
    // T48: `COBOL::"PARTIQL"` and its two siblings (A56).
    (1, NATIVE_IMPORT_MODULE, "pq_file"),
    // A60: the Backend's verbs (spec 114 M5).
    (1, NATIVE_IMPORT_MODULE, "ws_fail"),
    (1, NATIVE_IMPORT_MODULE, "ws_progress"),
    (1, NATIVE_IMPORT_MODULE, "ws_cancelled"),
    (1, NATIVE_IMPORT_MODULE, "ws_pause"),
    (1, NATIVE_IMPORT_MODULE, "ws_emit"),
    (1, NATIVE_IMPORT_MODULE, "ws_claim"),
    (1, NATIVE_IMPORT_MODULE, "ws_file_path"),
    (1, NATIVE_IMPORT_MODULE, "ws_file_publish"),
];

/// `pr_run`'s answers (the generator's `ret`): the run finished, or `STOP RUN`.
const RUN_DONE: i32 = -1;
const RUN_STOP: i32 = -4;
/// `con_read` and `env_get`: the end of the input, and "not there".
const END_OF_INPUT: i32 = -10;
const NOT_THERE: i32 = -1;

/// Run the program inside this executable and answer its exit code.
pub fn run_self() -> i32 {
    let lang = Lang::system();
    let exe = match std::env::current_exe().and_then(std::fs::read) {
        Ok(b) => b,
        Err(e) => return refuse(lang, Msg::CannotStart(e.to_string())),
    };
    run_executable(&exe, lang)
}

/// Run the program `exe` (an executable's bytes) carries.
pub fn run_executable(exe: &[u8], lang: Lang) -> i32 {
    let payload = match cobolt_program_package::locate(exe) {
        Ok(p) => p,
        Err(PackageError::NoPayload) => return refuse(lang, Msg::NoProgram),
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let payload = match cobolt_program_package::decode(payload) {
        Ok(p) => p,
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let module = match payload.module() {
        Ok(m) => m,
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let code = payload.part(Kind::MachineCode).zip(payload.part(Kind::MachineCodeEngine)).and_then(|(code, engine)| {
        let compat: [u8; 32] = engine.bytes.try_into().ok()?;
        Some(MachineCode { bytes: code.bytes.to_vec(), sha256: code.sha256, compat })
    });
    run_module(module, code.as_ref(), lang)
}

/// Check, load and run `module`, with the Build's machine code when there is some.
pub fn run_module(module: &[u8], code: Option<&MachineCode>, lang: Lang) -> i32 {
    let checked = match check(module) {
        Ok(c) => c,
        Err(msg) => return refuse(lang, msg),
    };
    let (pins, said) = Pins::from_env();
    for s in said {
        eprintln!("{s}");
    }
    let engine = match cobolt_runner_engine::engine() {
        Ok(e) => e,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    let cache = cache_dir().map(|d| Cache::new(d, cobolt_runner_engine::CACHE_LIMIT));
    let loaded = match cobolt_runner_engine::load(&engine, module, code, cache.as_ref()) {
        Ok(l) => l,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    if std::env::var_os("PRC_RUNNER_TRACE").is_some() {
        let why = loaded.skipped.as_ref().map(|s| format!(" ({s:?})")).unwrap_or_default();
        eprintln!("prc-runner: loaded by {:?}{why} in {:.1} ms", loaded.path, loaded.ms);
    }
    // A module with forms is a form application: its windows, and its programs on one thread (T30).
    if !checked.sections.forms.is_empty() {
        #[cfg(feature = "forms")]
        {
            let Checked { abi, imports, manifest, sections, files, sql } = checked;
            return match app::run(engine, loaded.module, abi, imports, manifest, sections, files, sql, lang, pins) {
                Ok(code) => code,
                Err(msg) => refuse(lang, msg),
            };
        }
        // A runner built without windows names what it lacks, as it names a capability.
        #[cfg(not(feature = "forms"))]
        return refuse(lang, Msg::Capabilities(vec!["forms".to_string()]));
    }
    let linker = match linker_for(&engine, checked.abi, &checked.imports) {
        Ok(l) => l,
        Err(msg) => return refuse(lang, msg),
    };
    let mut st = State::new(lang, pins);
    st.files = std::rc::Rc::new(std::cell::RefCell::new(files::Files::new(checked.files.clone())));
    st.sql = checked.sql.clone().map(sql::SqlRun::new);
    let mut store = Store::new(&engine, st);
    let instance = match linker.instantiate(&mut store, &loaded.module) {
        Ok(i) => i,
        Err(e) => return refuse(lang, Msg::CannotStart(format!("{e:#}"))),
    };
    let started = (|| -> Result<wasmtime::TypedFunc<(), i32>, String> {
        let arena = instance.get_memory(&mut store, "arena").ok_or("the module exports no `arena` memory")?;
        let base = instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").map_err(|e| e.to_string())?;
        let base = base.call(&mut store, ()).map_err(|e| e.to_string())?;
        store.data_mut().arena = Some(arena);
        store.data_mut().arena_base = base as u32;
        let abi = instance.get_typed_func::<(), i32>(&mut store, "pr_abi").map_err(|e| e.to_string())?;
        let abi = abi.call(&mut store, ()).map_err(|e| e.to_string())?;
        if abi as u32 != checked.abi {
            return Err(format!("the module answers ABI {abi} and its manifest names ABI {}", checked.abi));
        }
        instance.get_typed_func::<(), i32>(&mut store, "pr_run").map_err(|e| e.to_string())
    })();
    let run = match started {
        Ok(r) => r,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    let result = run.call(&mut store, ());
    let _ = std::io::stdout().flush();
    let st = store.data_mut();
    // The end of the run unit's SQL work, as the interpreter ends it: committed after `STOP RUN` or
    // the program's end, rolled back after an error (T41).
    let normal = st.fatal.is_none() && matches!(result, Ok(RUN_DONE | RUN_STOP));
    if let Some(sql) = st.sql.as_mut() {
        sql.end(normal);
    }
    st.db.borrow_mut().end();
    st.http.borrow_mut().end();
    if let Some(msg) = st.fatal.take() {
        return refuse(lang, msg);
    }
    match result {
        Ok(RUN_DONE | RUN_STOP) => st.exit_code.unwrap_or(0),
        Ok(other) => refuse(lang, Msg::Trapped(format!("the program stopped waiting for an event a console program never gets (status {other})"))),
        Err(e) => refuse(lang, Msg::Trapped(format!("{e:#}"))),
    }
}

/// What a module needs, read before it is loaded.
struct Checked {
    abi: u32,
    imports: Vec<(String, String)>,
    manifest: Manifest,
    /// The forms, menus, settings and art the Build put beside the code (T28).
    sections: cobolt_program_package::DesktopSections,
    /// The files on disk the program declares, and the engine new INDEXED files take (T38).
    files: files::FileSetup,
    /// The program's `EXEC SQL` statements and the declarations they run against (T41).
    sql: Option<sql::SqlSetup>,
}

/// Read the module's manifest and imports: the ABI must be one this runner runs (1 to
/// [`ABI_VERSION`](cobolt_wasm_abi::codes::ABI_VERSION)), and every capability and import one it provides.
fn check(module: &[u8]) -> Result<Checked, Msg> {
    #[cfg(feature = "forms")]
    let provided = PROVIDED_CAPABILITIES.to_vec();
    #[cfg(not(feature = "forms"))]
    let provided = headless_capabilities();
    check_with(module, &provided, true)
}

/// [`check`] against the capabilities `provided` (a Backend runner provides a subset, by variant). `connections` installs
/// the SQL connections the Build wrote beside a Desktop executable; a Backend reads its own from its configuration.
fn check_with(module: &[u8], provided: &[&str], connections: bool) -> Result<Checked, Msg> {
    let scan = cobolt_wasm_abi::scan::scan_module(module).map_err(Msg::Damaged)?;
    let text = scan.custom(MANIFEST_SECTION).ok_or_else(|| Msg::Damaged(format!("the module has no `{MANIFEST_SECTION}` section")))?;
    let manifest: Manifest = std::str::from_utf8(text)
        .ok()
        .and_then(|t| serde_json::from_str(t).ok())
        .ok_or_else(|| Msg::Damaged("the module's manifest cannot be read".into()))?;
    match manifest.validate() {
        Ok(()) => {}
        Err(ManifestError::AbiMismatch { found, expected }) => return Err(Msg::Abi { found, max: expected }),
        Err(e) => return Err(Msg::Damaged(e.to_string())),
    }
    let missing: Vec<String> =
        manifest.capabilities.iter().filter(|c| !provided.contains(&c.as_str())).cloned().collect();
    if !missing.is_empty() {
        return Err(Msg::Capabilities(missing));
    }
    let imports: Vec<(String, String)> = scan.imports.iter().map(|i| (i.module.clone(), i.name.clone())).collect();
    if let Some(name) = missing_imports(manifest.abi, &imports).into_iter().next() {
        return Err(Msg::Import(name));
    }
    let sections = cobolt_program_package::DesktopSections::decode(|name| scan.custom(name)).map_err(|e| Msg::Damaged(e.to_string()))?;
    let files = files::FileSetup::from_module(scan.custom(cobolt_wasm_abi::files::FILES_SECTION), &sections.settings.indexed_engine)
        .map_err(|e| Msg::Damaged(format!("the program's `{}` section: {e}", cobolt_wasm_abi::files::FILES_SECTION)))?;
    let sql = sql::SqlSetup::from_module(scan.custom(cobolt_wasm_abi::sql::SQL_SECTION))
        .map_err(|e| Msg::Damaged(format!("the program's `{}` section: {e}", cobolt_wasm_abi::sql::SQL_SECTION)))?;
    // The SQL connections the Build wrote beside the executable, read as a Cargo-built program reads
    // them at start, for `EXEC SQL` (T41), the SQL built-ins (T41b), the SqlDatabase controls (T41c)
    // and `COBOL::SqlDatabase` (T41d): the capability `sql` says the program has one of them.
    if connections && manifest.capabilities.iter().any(|c| c == cap::SQL) {
        sql::install_connections(&sections.settings.app_name);
    }
    Ok(Checked { abi: manifest.abi, imports, manifest, sections, files, sql })
}

/// The imports in `imports` this runner does not provide to a module of `abi`, as `module.name`.
pub fn missing_imports(abi: u32, imports: &[(String, String)]) -> Vec<String> {
    imports
        .iter()
        .filter(|(m, n)| !PROVIDED_IMPORTS.iter().any(|(since, pm, pn)| *since <= abi && pm == m && pn == n))
        .map(|(m, n)| format!("{m}.{n}"))
        .collect()
}

/// Say `msg` on standard error, in `lang`, and answer the exit code a refused run ends with: 4 for
/// a form the program does not carry, as a Cargo-built program ends, else 1.
fn refuse(lang: Lang, msg: Msg) -> i32 {
    let _ = std::io::stdout().flush();
    eprintln!("{}", messages::text(lang, &msg));
    if matches!(msg, Msg::NoSuchForm { .. }) {
        4
    } else {
        1
    }
}

/// Where machine code compiled at load is kept: `PRC_RUNNER_CACHE` when it is set, else the
/// user's cache folder; `None` when there is none.
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("PRC_RUNNER_CACHE") {
        return Some(PathBuf::from(d));
    }
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Caches"))
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".cache")))
    };
    base.map(|b| b.join("PowerRustCOBOL").join("runner"))
}

// ── the imports ─────────────────────────────────────────────────────────────

/// What the imports share during a run.
pub struct State {
    lang: Lang,
    pins: Pins,
    random: u64,
    arena: Option<Memory>,
    arena_base: u32,
    exit_code: Option<i32>,
    /// What ended the run, when the module aborted.
    fatal: Option<Msg>,
    /// A form module's ABI host and the regions of its arena (T30); `None` for a console program.
    #[cfg(feature = "forms")]
    pub(crate) forms: Option<forms::FormRun>,
    /// The program's files on disk (T38).
    /// The program's files, shared with the IndexedFile controls' port (T48).
    pub(crate) files: std::rc::Rc<std::cell::RefCell<files::Files>>,
    /// The program's `EXEC SQL` work (T41); `None` when it has none.
    pub(crate) sql: Option<sql::SqlRun>,
    /// The SQL built-ins' connections (T41b), which the SqlDatabase controls share (T41c).
    pub(crate) db: std::rc::Rc<std::cell::RefCell<sql::Builtins>>,
    /// The HTTP built-ins' clients, one per window (T42).
    pub(crate) http: std::rc::Rc<std::cell::RefCell<http::Clients>>,
    /// `COBOL::"PARTIQL"`'s results, one set per window (T48).
    pub(crate) pq: std::rc::Rc<std::cell::RefCell<partiql::Builtins>>,
    /// When set, what `DISPLAY` writes is kept here, a line each, instead of going to standard output: a Backend's
    /// `DISPLAY` is for its debug log and is never sent to the caller (spec 114 T21).
    pub(crate) display: Option<Vec<String>>,
    /// The Backend's verbs for the call this run answers (A60, spec 114 T36).
    pub(crate) verbs: verbs::Verbs,
}

impl State {
    fn new(lang: Lang, pins: Pins) -> Self {
        State {
            lang,
            random: pins.random_state(),
            pins,
            arena: None,
            arena_base: 0,
            exit_code: None,
            fatal: None,
            #[cfg(feature = "forms")]
            forms: None,
            files: std::rc::Rc::default(),
            sql: None,
            db: std::rc::Rc::default(),
            http: std::rc::Rc::default(),
            pq: std::rc::Rc::default(),
            display: None,
            verbs: verbs::Verbs::default(),
        }
    }
}

/// The batch the module staged in its arena's batch region (at its start, in a console module).
fn read_batch(caller: &Caller<'_, State>, len: i32) -> Vec<u8> {
    let st = caller.data();
    #[cfg(feature = "forms")]
    let at = st.arena_base + st.forms.as_ref().map_or(0, |f| f.layout.batch_off);
    #[cfg(not(feature = "forms"))]
    let at = st.arena_base;
    let mut buf = vec![0u8; len.max(0) as usize];
    if let Some(mem) = st.arena {
        let _ = mem.read(caller, at as usize, &mut buf);
    }
    buf
}

/// The display lists a batch's `DRAW`s name (ABI A23): the start of the graphics region, as far as the
/// furthest of them reaches. `None` when the batch draws nothing or the module has no region.
#[cfg(feature = "forms")]
fn read_gfx(caller: &Caller<'_, State>, batch: &[u8]) -> Option<Vec<u8>> {
    let st = caller.data();
    let (at, size) = st.forms.as_ref()?.gfx?;
    let end = decode_batch(batch)
        .ok()?
        .iter()
        .filter_map(|c| match c {
            Command::Draw { offset, len, .. } => Some(u64::from(*offset) + u64::from(*len)),
            _ => None,
        })
        .max()?;
    let mut buf = vec![0u8; end.min(u64::from(size)) as usize];
    st.arena?.read(caller, (st.arena_base + at) as usize, &mut buf).ok()?;
    Some(buf)
}

/// Carry out a batch: `DISPLAY` lines to standard output, errors to standard error. In an abort
/// the first error ends the run and is said once, after the output.
fn apply(batch: &[u8], st: &mut State, abort: bool) {
    let out = std::io::stdout();
    let mut out = out.lock();
    match decode_batch(batch) {
        Ok(cmds) => {
            for c in cmds {
                match c {
                    Command::Display(t) => match st.display.as_mut() {
                        Some(lines) => lines.push(t.to_string()),
                        None => {
                            let _ = out.write_all(t.as_bytes());
                            let _ = out.write_all(b"\n");
                        }
                    },
                    Command::Error { code, line, message, .. } => {
                        let msg = Msg::RuntimeError { code, line, text: message.to_string() };
                        if abort {
                            st.fatal.get_or_insert(msg);
                        } else {
                            let _ = out.flush();
                            eprintln!("{}", messages::text(st.lang, &msg));
                        }
                    }
                    // A console program has no controls: nothing else reaches it.
                    _ => {}
                }
            }
        }
        Err(e) => {
            st.fatal.get_or_insert(Msg::Trapped(format!("the program sent a batch that cannot be read: {e}")));
        }
    }
    let _ = out.flush();
}

/// The data memory, where `prn` imports read and write (A29).
fn memory0(caller: &mut Caller<'_, State>) -> Option<Memory> {
    caller.get_export("memory").and_then(|e| e.into_memory())
}

/// What a file import answers when the module handed it memory it cannot reach: a permanent error.
const FILE_FAILED: i32 = 30;

/// `len` bytes of memory 0 at `addr`; `None` when they are not all there.
fn read_mem(caller: &mut Caller<'_, State>, addr: i32, len: i32) -> Option<Vec<u8>> {
    let mem = memory0(caller)?;
    let mut buf = vec![0u8; len.max(0) as usize];
    mem.read(&*caller, addr as u32 as usize, &mut buf).ok()?;
    Some(buf)
}

/// An answer for the module: at most `cap` bytes of `bytes` written at `buf`; their count (0 when
/// memory 0 cannot take them).
fn answer(caller: &mut Caller<'_, State>, buf: i32, cap: i32, bytes: &[u8]) -> i32 {
    let bytes = &bytes[..bytes.len().min(cap.max(0) as usize)];
    let Some(mem) = memory0(caller) else { return 0 };
    mem.write(&mut *caller, buf as u32 as usize, bytes).map(|_| bytes.len() as i32).unwrap_or(0)
}

/// Write `bytes` into memory 0 at `addr`; `None` when they do not fit.
fn write_mem(caller: &mut Caller<'_, State>, addr: i32, bytes: &[u8]) -> Option<()> {
    let mem = memory0(caller)?;
    mem.write(&mut *caller, addr as u32 as usize, bytes).ok()
}

/// The `len` bytes at `addr` as the text a verb reads them as (a character that is not UTF-8 becomes U+FFFD, as the interpreter
/// reads a value's bytes); empty when memory 0 cannot be read.
fn text_at(caller: &mut Caller<'_, State>, addr: i32, len: i32) -> String {
    read_mem(caller, addr, len).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// A verb ended the run with a run-time error: the words go to the operator's log (as the interpreter's own errors do) and the
/// import traps.
fn verb_error(caller: &mut Caller<'_, State>, words: String) -> wasmtime::Error {
    caller.data_mut().fatal.get_or_insert(Msg::Trapped(words.clone()));
    wasmtime::Error::msg(words)
}

/// A file verb's answer: `text`, then the `status` word, at `buf` (at most `cap` bytes between them, the word whole); the
/// answer carries the word's length as its number and the text's as its length (ABI A60).
fn file_answer(caller: &mut Caller<'_, State>, buf: i32, cap: i32, text: String, status: &str) -> i64 {
    let room = (cap.max(0) as usize).saturating_sub(status.len());
    let mut out = text.into_bytes();
    out.truncate(room);
    let text_len = out.len();
    out.extend_from_slice(status.as_bytes());
    match write_mem(caller, buf, &out) {
        Some(()) => cobolt_wasm_abi::http::answer(status.len() as u16, text_len),
        None => cobolt_wasm_abi::http::FAILED,
    }
}

/// A linker with exactly `imports`, each as this runner implements it for a module of `abi`; an
/// import it does not provide is named.
pub fn linker_for(engine: &Engine, abi: u32, imports: &[(String, String)]) -> Result<Linker<State>, Msg> {
    if let Some(name) = missing_imports(abi, imports).into_iter().next() {
        return Err(Msg::Import(name));
    }
    let mut linker: Linker<State> = Linker::new(engine);
    let fail = |e: wasmtime::Error| Msg::CannotStart(e.to_string());
    for (module, name) in imports {
        match (module.as_str(), name.as_str()) {
            (IMPORT_MODULE, "flush") => {
                linker
                    .func_wrap(IMPORT_MODULE, "flush", |mut caller: Caller<'_, State>, len: i32| -> wasmtime::Result<()> {
                        let batch = read_batch(&caller, len);
                        #[cfg(feature = "forms")]
                        let list = read_gfx(&caller, &batch);
                        let st = caller.data_mut();
                        #[cfg(feature = "forms")]
                        if let Some(f) = st.forms.as_mut() {
                            if let Some(list) = list {
                                f.host.set_gfx(list);
                            }
                            f.host.flush(&batch);
                            // A command of the handler failed: its run ends there, as the interpreter's does,
                            // and the ABI host makes the failure the form's unhandled exception.
                            if f.host.halted() {
                                return Err(wasmtime::Error::msg("run-time error"));
                            }
                            return Ok(());
                        }
                        apply(&batch, st, false);
                        Ok(())
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "abort") => {
                linker
                    .func_wrap(IMPORT_MODULE, "abort", |mut caller: Caller<'_, State>, len: i32| -> wasmtime::Result<()> {
                        let batch = read_batch(&caller, len);
                        let st = caller.data_mut();
                        // The ABI host reports the error to the form, and the trap ends the run.
                        #[cfg(feature = "forms")]
                        if let Some(f) = st.forms.as_mut() {
                            f.host.abort(&batch);
                            return Err(wasmtime::Error::msg("run-time error"));
                        }
                        apply(&batch, st, true);
                        Err(wasmtime::Error::msg("run-time error"))
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "query") => {
                linker
                    .func_wrap(IMPORT_MODULE, "query", |mut caller: Caller<'_, State>, len: i32| -> i32 {
                        let bad = cobolt_wasm_abi::codes::err::BAD_ARGUMENT;
                        // The ABI host answers a query; a runner with no windows has none, and a console program asks nothing.
                        #[cfg(not(feature = "forms"))]
                        {
                            let _ = (&mut caller, len);
                            bad
                        }
                        #[cfg(feature = "forms")]
                        {
                            let batch = read_batch(&caller, len);
                            let list = read_gfx(&caller, &batch);
                            let (at, mem, answer) = {
                                let st = caller.data_mut();
                                let Some(f) = st.forms.as_mut() else { return bad };
                                if let Some(list) = list {
                                    f.host.set_gfx(list);
                                }
                                (st.arena_base + f.layout.answer_off, st.arena, f.host.query(&batch))
                            };
                            match (answer, mem) {
                                (Ok(answer), Some(mem)) => match mem.write(&mut caller, at as usize, &answer) {
                                    Ok(()) => answer.len() as i32,
                                    Err(_) => bad,
                                },
                                (Ok(_), None) => bad,
                                (Err(code), _) => code,
                            }
                        }
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "now") => {
                // A form program's clock moves with a deterministic headless run's virtual time, as the
                // interpreter's does (T8); a console program's stays at the pin.
                linker
                    .func_wrap(IMPORT_MODULE, "now", |caller: Caller<'_, State>| -> f64 {
                        let st = caller.data();
                        #[cfg(feature = "forms")]
                        if st.forms.is_some() {
                            return forms::now_ms();
                        }
                        st.pins.now_ms()
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "random") => {
                linker
                    .func_wrap(IMPORT_MODULE, "random", |mut caller: Caller<'_, State>| -> f64 {
                        let st = caller.data_mut();
                        let (next, v) = pins::lcg_step(st.random);
                        st.random = next;
                        v
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "con_read") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "con_read", |mut caller: Caller<'_, State>, addr: i32, cap: i32| -> i32 {
                        let _ = std::io::stdout().flush();
                        let mut line = Vec::new();
                        match std::io::stdin().lock().read_until(b'\n', &mut line) {
                            Ok(0) | Err(_) => return END_OF_INPUT,
                            Ok(_) => {}
                        }
                        while matches!(line.last(), Some(b'\n' | b'\r')) {
                            line.pop();
                        }
                        line.truncate(cap.max(0) as usize);
                        let Some(mem) = memory0(&mut caller) else { return END_OF_INPUT };
                        mem.write(&mut caller, addr as u32 as usize, &line).map(|_| line.len() as i32).unwrap_or(END_OF_INPUT)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "env_get") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "env_get", |mut caller: Caller<'_, State>, kind: i32, n: i32, buf: i32, cap: i32| -> i32 {
                        let Some(mem) = memory0(&mut caller) else { return NOT_THERE };
                        let args = || std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned());
                        let text = match kind {
                            0 => {
                                let mut name = vec![0u8; n.max(0) as usize];
                                if mem.read(&caller, buf as u32 as usize, &mut name).is_err() {
                                    return NOT_THERE;
                                }
                                env_var(&name)
                            }
                            1 => (n as u32 as usize).checked_sub(1).and_then(|i| args().nth(i)),
                            2 => return args().count() as i32,
                            3 => Some(args().collect::<Vec<_>>().join(" ")),
                            _ => None,
                        };
                        let Some(text) = text else { return NOT_THERE };
                        let bytes = &text.as_bytes()[..text.len().min(cap.max(0) as usize)];
                        mem.write(&mut caller, buf as u32 as usize, bytes).map(|_| bytes.len() as i32).unwrap_or(NOT_THERE)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "tz_offset") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "tz_offset", |caller: Caller<'_, State>| -> i32 {
                        let st = caller.data();
                        #[cfg(feature = "forms")]
                        if st.forms.is_some() {
                            return forms::offset_minutes();
                        }
                        st.pins.offset_minutes()
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "exit") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "exit", |mut caller: Caller<'_, State>, code: i32| {
                        caller.data_mut().exit_code = Some(code);
                    })
                    .map_err(fail)?;
            }
            // T38, A34: the files on disk. Each call names the file by its `prfiles` number and its
            // slot, reads and writes memory 0, and answers the FILE STATUS as a number.
            (NATIVE_IMPORT_MODULE, "f_open") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_open",
                        |mut caller: Caller<'_, State>, id: i32, slot: i32, mode: i32, path: i32, path_len: i32, flags: i32| -> i32 {
                            let Some(path) = read_mem(&mut caller, path, path_len) else { return FILE_FAILED };
                            let path = String::from_utf8_lossy(&path).into_owned();
                            let exclusive = flags & cobolt_wasm_abi::files::verb::OPEN_EXCLUSIVE != 0;
                            caller.data_mut().files.borrow_mut().open(id as u32, slot as u32, mode, &path, exclusive)
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_close") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "f_close", |mut caller: Caller<'_, State>, _id: i32, slot: i32, how: i32| -> i32 {
                        caller.data_mut().files.borrow_mut().close(slot as u32, how)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_read") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_read",
                        |mut caller: Caller<'_, State>,
                         id: i32,
                         slot: i32,
                         how: i32,
                         kor: i32,
                         key: i32,
                         key_len: i32,
                         rel: i64,
                         rec: i32,
                         cap: i32,
                         out: i32|
                         -> i32 {
                            let key = if key_len >= 0 { read_mem(&mut caller, key, key_len) } else { None };
                            let (status, record, delivered) =
                                caller.data_mut().files.borrow_mut().read(id as u32, slot as u32, how, kor as u32, key.as_deref(), rel);
                            if let Some(record) = record {
                                let n = record.len().min(cap.max(0) as usize);
                                if write_mem(&mut caller, rec, &record[..n]).is_none()
                                    || write_mem(&mut caller, out, &(record.len() as u32).to_le_bytes()).is_none()
                                {
                                    return FILE_FAILED;
                                }
                            }
                            if let Some(at) = delivered {
                                if write_mem(&mut caller, out.wrapping_add(8), &(at as i64).to_le_bytes()).is_none() {
                                    return FILE_FAILED;
                                }
                            }
                            status
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_write") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_write",
                        |mut caller: Caller<'_, State>, id: i32, slot: i32, rec: i32, len: i32, rel: i64, out: i32| -> i32 {
                            let Some(record) = read_mem(&mut caller, rec, len) else { return FILE_FAILED };
                            let (status, assigned) = caller.data_mut().files.borrow_mut().write(id as u32, slot as u32, &record, rel);
                            if let Some(at) = assigned {
                                if write_mem(&mut caller, out, &(at as i64).to_le_bytes()).is_none() {
                                    return FILE_FAILED;
                                }
                            }
                            status
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_rewrite") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_rewrite",
                        |mut caller: Caller<'_, State>, id: i32, slot: i32, rec: i32, len: i32, rel: i64| -> i32 {
                            let Some(record) = read_mem(&mut caller, rec, len) else { return FILE_FAILED };
                            caller.data_mut().files.borrow_mut().rewrite(id as u32, slot as u32, &record, rel)
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_delete") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_delete",
                        |mut caller: Caller<'_, State>, id: i32, slot: i32, key: i32, key_len: i32, rel: i64| -> i32 {
                            let key = if key_len >= 0 { read_mem(&mut caller, key, key_len) } else { None };
                            caller.data_mut().files.borrow_mut().delete(id as u32, slot as u32, key.as_deref(), rel)
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_start") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "f_start",
                        |mut caller: Caller<'_, State>, id: i32, slot: i32, op: i32, kor: i32, key: i32, key_len: i32, rel: i64| -> i32 {
                            let key = if key_len >= 0 { read_mem(&mut caller, key, key_len) } else { None };
                            caller.data_mut().files.borrow_mut().start(id as u32, slot as u32, op, kor as u32, key.as_deref(), rel)
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "f_tx") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "f_tx", |mut caller: Caller<'_, State>, op: i32, id: i32, slot: i32| -> i32 {
                        caller.data_mut().files.borrow_mut().tx(op, id, slot);
                        0
                    })
                    .map_err(fail)?;
            }
            // T39c, A39: APPEND-FILE (`append` 1) and WRITE-FILE (0), with the interpreter's own
            // steps: the path trimmed and anchored at the application's folder, the text's trailing
            // spaces dropped, one line written. Answers the length of why it failed (0 when written).
            (NATIVE_IMPORT_MODULE, "t_write") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "t_write",
                        |mut caller: Caller<'_, State>, append: i32, path: i32, path_len: i32, text: i32, text_len: i32, buf: i32, cap: i32| -> i32 {
                            let (Some(path), Some(text)) = (read_mem(&mut caller, path, path_len), read_mem(&mut caller, text, text_len)) else {
                                return answer(&mut caller, buf, cap, b"the program handed the runner memory it does not have");
                            };
                            let path = cobolt_runtime::interpreter::anchor_relative_path(String::from_utf8_lossy(&path).trim());
                            let text = String::from_utf8_lossy(&text);
                            let append = append != 0;
                            let written = std::fs::OpenOptions::new()
                                .create(true)
                                .append(append)
                                .write(true)
                                .truncate(!append)
                                .open(&path)
                                .and_then(|mut f| writeln!(f, "{}", text.trim_end()));
                            match written {
                                Ok(()) => 0,
                                Err(e) => answer(&mut caller, buf, cap, e.to_string().as_bytes()),
                            }
                        },
                    )
                    .map_err(fail)?;
            }
            // IMAGE-FIT: the interpreter's picture code, the paths anchored as it anchors them;
            // answers the length of the status word.
            (NATIVE_IMPORT_MODULE, "img_fit") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "img_fit",
                        |mut caller: Caller<'_, State>, src: i32, src_len: i32, dst: i32, dst_len: i32, width: i64, height: i64, buf: i32, cap: i32| -> i32 {
                            use cobolt_runtime::image_runtime as image;
                            let (Some(src), Some(dst)) = (read_mem(&mut caller, src, src_len), read_mem(&mut caller, dst, dst_len)) else {
                                return answer(&mut caller, buf, cap, image::NOFILE.as_bytes());
                            };
                            let anchored = |b: &[u8]| PathBuf::from(cobolt_runtime::interpreter::anchor_relative_path(String::from_utf8_lossy(b).trim()));
                            let word = match image::fit(&anchored(&src), &anchored(&dst), width, height) {
                                Ok(()) => image::OK,
                                Err(word) => word,
                            };
                            answer(&mut caller, buf, cap, word.as_bytes())
                        },
                    )
                    .map_err(fail)?;
            }
            // IMAGE-INFO: the width and the height as two little-endian words, then "FORMAT STATUS"
            // (no format on a refusal, and 0 × 0); answers the length after the two words.
            (NATIVE_IMPORT_MODULE, "img_info") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "img_info", |mut caller: Caller<'_, State>, path: i32, path_len: i32, buf: i32, cap: i32| -> i32 {
                        use cobolt_runtime::image_runtime as image;
                        let got = match read_mem(&mut caller, path, path_len) {
                            Some(p) => image::info(&PathBuf::from(cobolt_runtime::interpreter::anchor_relative_path(String::from_utf8_lossy(&p).trim()))),
                            None => Err(image::NOFILE),
                        };
                        let (width, height, format, word) = match got {
                            Ok(i) => (i.width, i.height, i.format, image::OK),
                            Err(word) => (0, 0, "", word),
                        };
                        let mut out = Vec::with_capacity(8 + format.len() + 1 + word.len());
                        out.extend_from_slice(&width.to_le_bytes());
                        out.extend_from_slice(&height.to_le_bytes());
                        out.extend_from_slice(format.as_bytes());
                        out.push(b' ');
                        out.extend_from_slice(word.as_bytes());
                        (answer(&mut caller, buf, cap, &out) - 8).max(0)
                    })
                    .map_err(fail)?;
            }
            // T41, A41: one `EXEC SQL` statement, run by the interpreter's own SQL code. The cell values
            // come in at `buf`; the answer goes back over them, at most `cap` bytes; answers its length.
            (NATIVE_IMPORT_MODULE, "sql_exec") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "sql_exec", |mut caller: Caller<'_, State>, stmt: i32, buf: i32, len: i32, cap: i32| -> i32 {
                        use cobolt_wasm_abi::sql::wire::FAILED;
                        let Some(input) = read_mem(&mut caller, buf, len) else { return FAILED };
                        let Some(sql) = caller.data_mut().sql.as_mut() else { return FAILED };
                        let out = match sql.exec(stmt as u32, &input) {
                            Ok(out) if out.len() <= cap.max(0) as usize => out,
                            Ok(_) => {
                                eprintln!("prc-runner: the answer to EXEC SQL statement {stmt} does not fit in its space");
                                return FAILED;
                            }
                            Err(e) => {
                                eprintln!("prc-runner: EXEC SQL statement {stmt}: {e}");
                                return FAILED;
                            }
                        };
                        match write_mem(&mut caller, buf, &out) {
                            Some(()) => out.len() as i32,
                            None => FAILED,
                        }
                    })
                    .map_err(fail)?;
            }
            // T41b, A42: a SQL built-in, run by the interpreter's own code over the connections of the
            // form instance the call acts on. The receivers' values go to `buf`, at most `cap` bytes;
            // answers their length.
            (NATIVE_IMPORT_MODULE, "sql_db") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "sql_db",
                        |mut caller: Caller<'_, State>, op: i32, handle: i64, number: i64, text: i32, text_len: i32, buf: i32, cap: i32| -> i32 {
                            use cobolt_wasm_abi::sql::db::FAILED;
                            let Some(text) = read_mem(&mut caller, text, text_len) else { return FAILED };
                            let Some(out) = caller.data_mut().db.borrow_mut().call(op as u32, handle, number, &text, cap.max(0) as usize) else { return FAILED };
                            match write_mem(&mut caller, buf, &out) {
                                Some(()) => out.len() as i32,
                                None => FAILED,
                            }
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "sql_obj") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "sql_obj",
                        |mut caller: Caller<'_, State>, op: i32, member: i32, member_len: i32, text: i32, text_len: i32, buf: i32, cap: i32| -> i32 {
                            use cobolt_wasm_abi::sql::obj::FAILED;
                            let Some(member) = read_mem(&mut caller, member, member_len) else { return FAILED };
                            let Some(text) = read_mem(&mut caller, text, text_len) else { return FAILED };
                            let Some(out) = caller.data_mut().db.borrow_mut().object(op as u32, &member, &text) else { return FAILED };
                            // An answer longer than the module's room is not written: its length says so (A44).
                            if out.len() > cap.max(0) as usize {
                                return i32::try_from(out.len()).unwrap_or(i32::MAX);
                            }
                            match write_mem(&mut caller, buf, out.as_bytes()) {
                                Some(()) => out.len() as i32,
                                None => FAILED,
                            }
                        },
                    )
                    .map_err(fail)?;
            }
            // T42, A45: an HTTP built-in, run by the interpreter's own client of the form instance the
            // call acts on. The answer's text goes to `buf`, at most `cap` bytes (its first, or with
            // `TAIL` its last without its trailing spaces); answers its number and whole length.
            (NATIVE_IMPORT_MODULE, "http_call") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "http_call",
                        |mut caller: Caller<'_, State>, op: i32, a: i32, a_len: i32, b: i32, b_len: i32, buf: i32, cap: i32| -> i64 {
                            use cobolt_wasm_abi::http as h;
                            let Some(a) = read_mem(&mut caller, a, a_len) else { return h::FAILED };
                            let Some(b) = read_mem(&mut caller, b, b_len) else { return h::FAILED };
                            let op = op as u32;
                            let Some((number, text)) = caller.data().http.borrow_mut().call(op, &a, &b) else { return h::FAILED };
                            let (shown, len) = http::part(op, &text, cap.max(0) as usize);
                            match write_mem(&mut caller, buf, shown) {
                                Some(()) => h::answer(number, len),
                                None => h::FAILED,
                            }
                        },
                    )
                    .map_err(fail)?;
            }
            // T48, A56: `COBOL::"PARTIQL"` and its two siblings, run by the interpreter's own rules on
            // the program's open file. The request names the texts in memory 0; the answer's text goes to
            // `buf`, at most `cap` bytes (its first, or with `TAIL` its last without its trailing spaces);
            // answers its number and whole length.
            (NATIVE_IMPORT_MODULE, "pq_file") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "pq_file",
                        |mut caller: Caller<'_, State>, op: i32, req: i32, req_len: i32, buf: i32, cap: i32| -> i64 {
                            use cobolt_wasm_abi::partiql as pq;
                            let op = op as u32;
                            let Some(bytes) = read_mem(&mut caller, req, req_len) else { return pq::FAILED };
                            let Some(r) = pq::decode(op, &bytes) else { return pq::FAILED };
                            let mut text_at = |(at, len): (u32, u32)| read_mem(&mut caller, at as i32, len as i32);
                            let Some(file) = text_at(r.file) else { return pq::FAILED };
                            let Some(text) = text_at(r.text) else { return pq::FAILED };
                            let mut args = Vec::with_capacity(r.args.len());
                            for &(kind, at, len) in &r.args {
                                let Some(t) = text_at((at, len)) else { return pq::FAILED };
                                args.push((kind, t));
                            }
                            let (files, store) = (caller.data().files.clone(), caller.data().pq.clone());
                            let Some((number, text)) = store.borrow_mut().call(&mut files.borrow_mut(), op, &r.files, &file, &text, &args) else { return pq::FAILED };
                            let tail = if op & pq::TAIL != 0 { cobolt_wasm_abi::http::TAIL } else { 0 };
                            let (shown, len) = http::part(tail, &text, cap.max(0) as usize);
                            match write_mem(&mut caller, buf, shown) {
                                Some(()) => pq::answer(number, len),
                                None => pq::FAILED,
                            }
                        },
                    )
                    .map_err(fail)?;
            }
            // A60: the Backend's verbs, for the call being answered (`verbs::Verbs`, which applies the interpreter's own rules).
            // A text the module passes is read where it is, as the interpreter's `as_display_string` reads a value; a run-time
            // error of a verb is a trap with the interpreter's words, and a call the server stops traps as the epoch does.
            (NATIVE_IMPORT_MODULE, "ws_fail") => {
                linker
                    .func_wrap(
                        NATIVE_IMPORT_MODULE,
                        "ws_fail",
                        |mut caller: Caller<'_, State>, code: f64, msg: i32, msg_len: i32, detail: i32, detail_len: i32| -> wasmtime::Result<i32> {
                            let message = text_at(&mut caller, msg, msg_len);
                            let detail = text_at(&mut caller, detail, detail_len);
                            let said = caller.data_mut().verbs.fail(code, &message, &detail);
                            said.map(|()| 0).map_err(|m| verb_error(&mut caller, m))
                        },
                    )
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_progress") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_progress", |mut caller: Caller<'_, State>, percent: f64, msg: i32, msg_len: i32| -> i32 {
                        let message = text_at(&mut caller, msg, msg_len);
                        caller.data_mut().verbs.progress(percent, &message)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_cancelled") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_cancelled", |caller: Caller<'_, State>| -> i32 { i32::from(caller.data().verbs.cancelled_asked()) })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_pause") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_pause", |caller: Caller<'_, State>, ms: f64| -> wasmtime::Result<i32> {
                        caller.data().verbs.pause(ms).map(|()| 0).map_err(|()| wasmtime::Error::from(wasmtime::Trap::Interrupt))
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_emit") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_emit", |mut caller: Caller<'_, State>, buf: i32, len: i32| -> wasmtime::Result<i32> {
                        let json = if len < 0 { String::new() } else { text_at(&mut caller, buf, len) };
                        let item = if len < 0 { Err(-len) } else { Ok(json.as_str()) };
                        let said = caller.data_mut().verbs.emit(item);
                        said.map_err(|m| verb_error(&mut caller, m))
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_claim") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_claim", |mut caller: Caller<'_, State>, name: i32, name_len: i32, buf: i32, cap: i32| -> i32 {
                        let name = text_at(&mut caller, name, name_len);
                        let value = caller.data().verbs.claim(&name);
                        answer(&mut caller, buf, cap, value.as_bytes())
                    })
                    .map_err(fail)?;
            }
            // The path (or the id) and then the status word, at `buf`; the answer's number is the word's length, its length the text's.
            (NATIVE_IMPORT_MODULE, "ws_file_path") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_file_path", |mut caller: Caller<'_, State>, id: i32, id_len: i32, buf: i32, cap: i32| -> i64 {
                        let id = text_at(&mut caller, id, id_len);
                        let (text, status) = caller.data().verbs.file_path(&id);
                        file_answer(&mut caller, buf, cap, text, status)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "ws_file_publish") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "ws_file_publish", |mut caller: Caller<'_, State>, path: i32, path_len: i32, buf: i32, cap: i32| -> i64 {
                        let path = text_at(&mut caller, path, path_len);
                        let (text, status) = caller.data().verbs.publish(&path);
                        file_answer(&mut caller, buf, cap, text, status)
                    })
                    .map_err(fail)?;
            }
            // `missing_imports` let only the rows of PROVIDED_IMPORTS through.
            _ => return Err(Msg::Import(format!("{module}.{name}"))),
        }
    }
    Ok(linker)
}

/// The environment variable named by these bytes, as text; `None` when it is not set.
fn env_var(name: &[u8]) -> Option<String> {
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(name).to_os_string()
    };
    #[cfg(not(unix))]
    let name = std::ffi::OsString::from(String::from_utf8_lossy(name).into_owned());
    // A name no system can hold is not set (and is never handed to the system's lookup).
    if name.is_empty() || name.as_encoded_bytes().iter().any(|b| matches!(b, b'=' | 0)) {
        return None;
    }
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}
