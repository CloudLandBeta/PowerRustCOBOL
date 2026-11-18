// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `EXEC SQL` (DESKTOP-WASM-001 T41, ABI A41): the runner's half of the import `sql_exec`.
//!
//! The module keeps the program's data; the interpreter's own SQL code runs each statement. For
//! every program that has `EXEC SQL` the runner keeps a small interpreter per form instance, built
//! from the declarations the module's `prsql` section carries, and joins them all to one run unit,
//! as the form host joins a Cargo-built program's interpreters. Before a statement the module sends
//! the value of each cell (each elementary item the statement may read or write); the runner sets
//! them in the program's interpreter, runs the statement there, and answers the condition it ended
//! in and the cells whose value changed (`cobolt_wasm_abi::sql::wire`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cobolt_abi_host::host::{SqlControl, SqlEffects, SqlPort};
use cobolt_ast::program::Program;
use cobolt_ast::sql::ExecSql;
use cobolt_runtime::environment::{subscript_key, CobolEnvironment};
use cobolt_runtime::db_runtime::{open_named, ControlEffects, ControlStep, DbBuiltin, DbConn, DbReceiver, DbRegistry};
use cobolt_runtime::esql::state::Outcome;
use cobolt_runtime::esql::SqlRunUnit;
use cobolt_runtime::interpreter::Interpreter;
use cobolt_runtime::value::{CobolNumeric, CobolValue};
use cobolt_wasm_abi::sql::{db, wire, SqlCell, SqlSection, SQL_FORMAT};

/// One statement, read from the section once.
struct Statement {
    program: usize,
    exec: ExecSql,
    cells: Vec<SqlCell>,
}

/// A value as it crosses the import.
enum Value {
    Dec(CobolNumeric),
    Float(f64),
    Bytes(Vec<u8>),
}

/// What a run's SQL work is made from: the module's `prsql` section, read once.
#[derive(Clone)]
pub struct SqlSetup {
    programs: Arc<Vec<Program>>,
    statements: Arc<Vec<Statement>>,
}

/// The SQL work of one run.
pub struct SqlRun {
    setup: SqlSetup,
    unit: Arc<Mutex<SqlRunUnit>>,
    /// Each program's interpreter, per form instance (`None`: a console program, or a form whose
    /// instance is not known yet).
    interpreters: HashMap<(usize, Option<u32>), Interpreter>,
    /// Each form instance's identity in the run unit's tables, shared by its programs' interpreters,
    /// as one form's programs share its one interpreter's cursors and prepared statements.
    instances: HashMap<Option<u32>, u64>,
    /// The form instance the module is calling into.
    calling: Option<u32>,
    /// Programs whose interpreter was made during the current call with no instance known.
    made_in_call: Vec<usize>,
    /// The application's main form: its window closing ends the run unit.
    root: Option<u32>,
}

impl SqlSetup {
    /// The section's statements and programs; `None` for a module without the section.
    pub fn from_module(section: Option<&[u8]>) -> Result<Option<SqlSetup>, String> {
        let Some(bytes) = section else { return Ok(None) };
        let s: SqlSection = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if s.format == 0 || s.format > SQL_FORMAT {
            return Err(format!("format {} is newer than this runner reads ({SQL_FORMAT})", s.format));
        }
        let programs = s
            .programs
            .into_iter()
            .map(|p| serde_json::from_value::<Program>(p.program).map_err(|e| format!("program {}: {e}", p.name)))
            .collect::<Result<Vec<_>, _>>()?;
        let statements = s
            .statements
            .into_iter()
            .enumerate()
            .map(|(i, st)| {
                let exec = serde_json::from_value::<ExecSql>(st.exec).map_err(|e| format!("statement {i}: {e}"))?;
                let program = st.program as usize;
                if program >= programs.len() {
                    return Err(format!("statement {i} names program {program}, which the section does not have"));
                }
                Ok(Statement { program, exec, cells: st.cells })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Some(SqlSetup { programs: Arc::new(programs), statements: Arc::new(statements) }))
    }
}

impl SqlRun {
    /// A run's SQL work, with no connection yet.
    pub fn new(setup: SqlSetup) -> Self {
        SqlRun { setup, unit: Arc::default(), interpreters: HashMap::new(), instances: HashMap::new(), calling: None, made_in_call: Vec::new(), root: None }
    }

    /// Run statement `stmt` with the cell values `input`; answers the bytes to write back, or why
    /// the statement could not run at all.
    pub fn exec(&mut self, stmt: u32, input: &[u8]) -> Result<Vec<u8>, String> {
        let st = self.setup.statements.get(stmt as usize).ok_or_else(|| format!("the program has no EXEC SQL statement {stmt}"))?;
        let key = (st.program, self.calling);
        if !self.interpreters.contains_key(&key) {
            let mut interp = Interpreter::new(self.setup.programs[st.program].clone());
            interp.set_sql_run_unit(self.unit.clone(), false);
            match self.instances.get(&self.calling) {
                Some(&id) => interp.set_sql_instance(id),
                None => {
                    self.instances.insert(self.calling, interp.sql_instance());
                }
            }
            self.interpreters.insert(key, interp);
            if self.calling.is_none() {
                self.made_in_call.push(st.program);
            }
        }
        let interp = self.interpreters.get_mut(&key).expect("made above");
        let keys: Vec<String> = st.cells.iter().map(|c| cell_key(&interp.env, c)).collect::<Result<_, _>>()?;
        let mut at = 0;
        let mut before = Vec::with_capacity(keys.len());
        for k in &keys {
            match take(input, &mut at).ok_or("the program sent fewer values than the statement has cells")? {
                Value::Dec(n) => interp.env.set(k, CobolValue::Numeric(n)),
                Value::Float(f) => interp.env.set(k, CobolValue::Float(f)),
                Value::Bytes(b) => interp.env.set_verbatim_bytes(k, &b),
            }
            let mut v = Vec::new();
            put(&interp.env, k, &mut v);
            before.push(v);
        }
        let outcome = interp.exec_sql_outcome(&st.exec);
        let mut out = vec![match outcome {
            Outcome::Success => wire::OUT_SUCCESS,
            Outcome::Warning => wire::OUT_WARNING,
            Outcome::NoData => wire::OUT_NOT_FOUND,
            Outcome::Error => wire::OUT_ERROR,
        }];
        out.extend_from_slice(&0u32.to_le_bytes());
        let mut changed = 0u32;
        for (i, k) in keys.iter().enumerate() {
            let mut v = Vec::new();
            put(&interp.env, k, &mut v);
            if v != before[i] {
                out.extend_from_slice(&(i as u32).to_le_bytes());
                out.extend_from_slice(&v);
                changed += 1;
            }
        }
        out[1..5].copy_from_slice(&changed.to_le_bytes());
        Ok(out)
    }

    /// A call into the module begins: `inst` is the form instance it acts on, when it names one.
    pub fn begin_call(&mut self, inst: Option<u32>) {
        self.calling = inst;
        self.made_in_call.clear();
    }

    /// The form instance an `OpenForm` call made is `inst`: the interpreters its program made while
    /// it started, before the instance was known, are its own. The first is the main form.
    pub fn opened_form(&mut self, inst: u32) {
        self.root.get_or_insert(inst);
        if let Some(id) = self.instances.remove(&None) {
            self.instances.entry(Some(inst)).or_insert(id);
        }
        for p in std::mem::take(&mut self.made_in_call) {
            if let Some(i) = self.interpreters.remove(&(p, None)) {
                self.interpreters.insert((p, Some(inst)), i);
            }
        }
    }

    /// Form instance `inst`'s window closed. The main form's ends the run unit, as the main form's
    /// interpreter does; any other lets go of its cursors and prepared statements, as a child
    /// form's interpreter does.
    pub fn forget_instance(&mut self, inst: u32) {
        if self.root == Some(inst) {
            self.end(true);
            return;
        }
        self.instances.remove(&Some(inst));
        let gone: Vec<(usize, Option<u32>)> = self.interpreters.keys().filter(|k| k.1 == Some(inst)).copied().collect();
        for k in gone {
            if let Some(mut i) = self.interpreters.remove(&k) {
                i.sql_end(true);
            }
        }
    }

    /// The end of the run unit: every connection's open work committed after a normal end (`STOP
    /// RUN`, `GOBACK`, the main window closing), rolled back after an error, then every connection
    /// closed, by the interpreter's own code. Doing it again does nothing.
    pub fn end(&mut self, normal: bool) {
        if let Some(i) = self.interpreters.values_mut().next() {
            i.set_sql_run_unit(self.unit.clone(), true);
            i.sql_end(normal);
            i.set_sql_run_unit(self.unit.clone(), false);
        }
    }
}

/// The SQL built-ins' connections (T41b, ABI A42): `COBOL::"OPEN-DB"` and the five after it. Each form
/// instance keeps its own registry, as each form's interpreter keeps its own, shared by its handlers
/// and the procedures they call; a console program has one (`None`). The interpreter's own code runs
/// each call (`DbRegistry::builtin`), a connection's name resolved through the SQL connections the
/// Build wrote beside the executable, as `CONNECT TO` resolves it.
#[derive(Default)]
pub struct Builtins {
    registries: HashMap<Option<u32>, DbRegistry>,
    /// The form instance the module is calling into.
    calling: Option<u32>,
    /// The registry with no instance was made during the current call.
    made_in_call: bool,
    /// The SqlDatabase statements on threads (T41c): generation → (registry, the control's name).
    /// Kept until the connection comes back, whatever became of the statement.
    jobs: HashMap<u64, (Option<u32>, String)>,
    /// The statement each SqlDatabase is running: (registry, CONTROL) → its generation. `Cancel` and
    /// a time-out take it away, so its result is dropped.
    running: HashMap<(Option<u32>, String), u64>,
    generation: u64,
}

impl Builtins {
    /// Built-in `op` (`cobolt_wasm_abi::sql::db`) with the handle and the column as the program's
    /// `as_i64` read them and its text untrimmed; answers each receiver's value, laid out as the ABI
    /// says, in at most `cap` bytes (a text is cut to fit), or `None` for an operation it does not know.
    pub fn call(&mut self, op: u32, handle: i64, number: i64, text: &[u8], cap: usize) -> Option<Vec<u8>> {
        let text = String::from_utf8_lossy(text);
        let text = text.trim();
        // As the interpreter's `as_i64().unwrap_or(..) as u32` and `as usize` take them.
        let handle = handle as u32;
        let call = match op {
            db::OPEN => DbBuiltin::Open(text),
            db::EXEC => DbBuiltin::Exec { handle, query: text },
            db::FETCH_ROW => DbBuiltin::FetchRow { handle, column: number as usize },
            db::NEXT_ROW => DbBuiltin::NextRow(handle),
            db::ROW_COUNT => DbBuiltin::RowCount(handle),
            db::CLOSE => DbBuiltin::Close(handle),
            _ => return None,
        };
        if self.calling.is_none() && !self.registries.contains_key(&None) {
            self.made_in_call = true;
        }
        let registry = self.registries.entry(self.calling).or_insert_with(DbRegistry::new);
        let catalog = cobolt_runtime::esql::catalog::current();
        let receivers = registry.builtin(call, |db, conn| open_named(db, conn, catalog.as_deref()));
        Some(encode(&receivers, cap))
    }

    /// A call into the module begins: `inst` is the form instance it acts on, when it names one.
    pub fn begin_call(&mut self, inst: Option<u32>) {
        self.calling = inst;
        self.made_in_call = false;
    }

    /// The form instance an `OpenForm` call made is `inst`: connections its program opened while it
    /// started, before the instance was known, are its own.
    pub fn opened_form(&mut self, inst: u32) {
        if std::mem::take(&mut self.made_in_call) {
            if let Some(r) = self.registries.remove(&None) {
                self.registries.insert(Some(inst), r);
                // And the SqlDatabase statements running on them.
                for (key, _) in self.jobs.values_mut() {
                    if key.is_none() {
                        *key = Some(inst);
                    }
                }
                self.running = std::mem::take(&mut self.running).into_iter().map(|((k, c), g)| ((k.or(Some(inst)), c), g)).collect();
            }
        }
    }

    /// Form instance `inst`'s window closed: its connections close, as a closed form's interpreter
    /// drops its own.
    pub fn forget_instance(&mut self, inst: u32) {
        self.registries.remove(&Some(inst));
        self.running.retain(|(k, _), _| *k != Some(inst));
    }

    /// The run ends: every connection closes, as the interpreter closes them all at the end of the run.
    pub fn end(&mut self) {
        for r in self.registries.values_mut() {
            r.close_all();
        }
        self.registries.clear();
    }
}

/// A SqlDatabase statement's news, on its way to the program thread (T41c).
pub(crate) enum SqlNews {
    /// A statement on a thread ended; its connection comes back with it.
    Done { generation: u64, inst: u32, handle: u32, query: bool, conn: DbConn, result: Result<usize, String> },
    /// A statement's `TimeoutMs` (`ms`) passed.
    TimedOut { generation: u64, inst: u32, ms: u64 },
}

/// What [`Builtins::control`] leaves to its caller.
enum ControlOutcome {
    Done(SqlEffects),
    /// Run `sql` on `conn`, lent from connection `handle`, on a thread; the control is now busy.
    Spawn { generation: u64, handle: u32, sql: String, query: bool, conn: DbConn, timeout_ms: u64 },
}

/// What the host does with what a method did. A data item a property names is never written: the
/// Build sends a SqlDatabase that names one down the Cargo path (T41c).
fn host_effects(fx: ControlEffects, answer: Option<&str>) -> SqlEffects {
    SqlEffects {
        props: fx.props,
        events: fx.events.into_iter().map(str::to_owned).collect(),
        answer: answer.map(str::to_owned).unwrap_or(fx.answer),
    }
}

impl Builtins {
    /// The method `method` (upper case) of the SqlDatabase `c`, on the registry of the instance the
    /// module is calling into, as the built-ins use it: the interpreter's own code
    /// (`DbRegistry::control`), and its asynchronous rules (spec 032): a second statement while one
    /// runs is ignored, and every statement started answers `0` at once.
    fn control(&mut self, c: &SqlControl<'_>, method: &str, args: &[&str]) -> Option<ControlOutcome> {
        let key = self.calling;
        if key.is_none() && !self.registries.contains_key(&None) {
            self.made_in_call = true;
        }
        let registry = self.registries.entry(key).or_insert_with(DbRegistry::new);
        let catalog = cobolt_runtime::esql::catalog::current();
        let args: Vec<String> = args.iter().map(|a| a.trim().to_owned()).collect();
        let step = registry.control(method, &args, c.prop, |db, conn| open_named(db, conn, catalog.as_deref()))?;
        let (handle, sql, query) = match step {
            ControlStep::Done(fx) => return Some(ControlOutcome::Done(host_effects(fx, None))),
            ControlStep::Async { handle, sql, query } => (handle, sql, query),
        };
        let ctl = c.name.trim().to_ascii_uppercase();
        if self.running.contains_key(&(key, ctl.clone())) {
            return Some(ControlOutcome::Done(SqlEffects { answer: "0".into(), ..SqlEffects::default() }));
        }
        let conn = match registry.lend(handle) {
            Ok(conn) => conn,
            Err(e) => return Some(ControlOutcome::Done(host_effects(ControlEffects::lend_refused(e), Some("0")))),
        };
        self.generation += 1;
        let generation = self.generation;
        self.running.insert((key, ctl), generation);
        self.jobs.insert(generation, (key, c.name.to_owned()));
        let timeout_ms = (c.prop)("TimeoutMs").trim().parse::<u64>().unwrap_or(0);
        Some(ControlOutcome::Spawn { generation, handle, sql, query, conn, timeout_ms })
    }

    /// `Cancel` on the SqlDatabase named `ctl`: whether a statement of it was running.
    fn cancel(&mut self, ctl: &str) -> bool {
        self.running.remove(&(self.calling, ctl.trim().to_ascii_uppercase())).is_some()
    }

    /// News of a statement: `(the instance, the control's name, what the host does)`, or `None` when
    /// the statement was cancelled or ran out of time, or its window has closed. A connection that
    /// comes back goes home first, whatever became of its statement, as under the interpreter.
    pub(crate) fn delivered(&mut self, news: SqlNews) -> Option<(u32, String, SqlEffects)> {
        match news {
            SqlNews::Done { generation, inst, handle, query, conn, result } => {
                let (key, name) = self.jobs.remove(&generation)?;
                let registry = self.registries.get_mut(&key)?;
                registry.give_back(handle, conn);
                let running = (key, name.trim().to_ascii_uppercase());
                if self.running.get(&running) != Some(&generation) {
                    return None;
                }
                self.running.remove(&running);
                let fx = self.registries.get(&key)?.control_delivered(handle, query, result);
                Some((inst, name, host_effects(fx, None)))
            }
            SqlNews::TimedOut { generation, inst, ms } => {
                let running = self.running.iter().find(|(_, g)| **g == generation).map(|(k, _)| k.clone())?;
                self.running.remove(&running);
                let name = self.jobs.get(&generation).map(|(_, n)| n.clone())?;
                let fx = SqlEffects {
                    props: vec![("LastError".into(), cobolt_runtime::async_op::timed_out(ms)), ("Busy".into(), "0".into())],
                    events: vec!["onTimeout".into()],
                    answer: String::new(),
                };
                Some((inst, name, fx))
            }
        }
    }
}

/// The runner's SqlDatabase (T41c): the host's port, over the connections the SQL built-ins use.
/// A statement of a control whose `Mode` is `Async` runs on a thread of its own and comes back to the
/// program thread as [`SqlNews`], with a second thread waiting out its `TimeoutMs`.
pub(crate) struct Port {
    pub db: Rc<RefCell<Builtins>>,
    pub send: Arc<dyn Fn(SqlNews) + Send + Sync>,
}

impl SqlPort for Port {
    fn call(&mut self, inst: u32, c: &SqlControl<'_>, method: &str, args: &[&str]) -> Option<SqlEffects> {
        let outcome = self.db.borrow_mut().control(c, method, args)?;
        match outcome {
            ControlOutcome::Done(fx) => Some(fx),
            ControlOutcome::Spawn { generation, handle, sql, query, conn, timeout_ms } => {
                let send = Arc::clone(&self.send);
                std::thread::spawn(move || {
                    let mut conn = conn;
                    let result = conn.run(&sql);
                    send(SqlNews::Done { generation, inst, handle, query, conn, result });
                });
                if timeout_ms > 0 {
                    let send = Arc::clone(&self.send);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
                        send(SqlNews::TimedOut { generation, inst, ms: timeout_ms });
                    });
                }
                Some(host_effects(ControlEffects::started(), Some("0")))
            }
        }
    }

    fn cancel(&mut self, _inst: u32, ctl: &str) -> bool {
        self.db.borrow_mut().cancel(ctl)
    }

    fn release(&mut self, inst: u32) {
        self.db.borrow_mut().forget_instance(inst);
    }
}

/// The receivers' values, laid out as `cobolt_wasm_abi::sql::db` says, in at most `cap` bytes: a text
/// that does not fit is cut, the first one first.
fn encode(receivers: &[DbReceiver], cap: usize) -> Vec<u8> {
    let fixed: usize = receivers.iter().map(|r| if matches!(r, DbReceiver::Number(_)) { 9 } else { 5 }).sum();
    let mut room = cap.saturating_sub(fixed);
    let mut out = Vec::new();
    for r in receivers {
        match r {
            DbReceiver::Number(n) => {
                out.push(db::NUMBER);
                out.extend_from_slice(&n.to_le_bytes());
            }
            DbReceiver::Text(t) => {
                let b = &t.as_bytes()[..t.len().min(room)];
                room -= b.len();
                out.push(db::TEXT);
                out.extend_from_slice(&(b.len() as u32).to_le_bytes());
                out.extend_from_slice(b);
            }
        }
    }
    out.truncate(cap);
    out
}

/// The interpreter's storage key of a cell: the item its name and qualification resolve to, and the
/// occurrence its subscripts name.
fn cell_key(env: &CobolEnvironment, c: &SqlCell) -> Result<String, String> {
    let item = env.resolve_name(&c.name, &c.quals);
    if env.symbol(&item).is_none() && !env.contains(&item) {
        return Err(format!("the program has no item {}", c.name));
    }
    Ok(subscript_key(&item, &c.subs))
}

/// One value from `buf` at `at`, which moves past it.
fn take(buf: &[u8], at: &mut usize) -> Option<Value> {
    let tag = *buf.get(*at)?;
    let rest = &buf[*at + 1..];
    let (v, n) = match tag {
        wire::DEC => {
            let m = i128::from_le_bytes(rest.get(..16)?.try_into().ok()?);
            (Value::Dec(CobolNumeric::new(m, *rest.get(16)?)), 17)
        }
        wire::FLOAT => (Value::Float(f64::from_le_bytes(rest.get(..8)?.try_into().ok()?)), 8),
        wire::BYTES => {
            let len = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
            (Value::Bytes(rest.get(4..4 + len)?.to_vec()), 4 + len)
        }
        _ => return None,
    };
    *at += 1 + n;
    Some(v)
}

/// An item's value as the interpreter holds it, onto `out`: a number as its mantissa and scale, a
/// floating value as itself, anything else as its bytes.
fn put(env: &CobolEnvironment, key: &str, out: &mut Vec<u8>) {
    let bytes = |out: &mut Vec<u8>, b: &[u8]| {
        out.push(wire::BYTES);
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(b);
    };
    match env.get(key) {
        Some(CobolValue::Numeric(n)) => {
            out.push(wire::DEC);
            out.extend_from_slice(&n.mantissa.to_le_bytes());
            out.push(n.decimals);
        }
        Some(CobolValue::Float(f)) => {
            out.push(wire::FLOAT);
            out.extend_from_slice(&f.to_le_bytes());
        }
        Some(CobolValue::String { bytes: b, .. }) => bytes(out, b),
        _ => bytes(out, &env.display_bytes(key).unwrap_or_default()),
    }
}

/// The `sql-connections.toml` beside the executable, as a Cargo-built program reads it at start:
/// the connections `CONNECT TO 'NAME'` reaches, with the environment and the application's key
/// store supplying credentials. Without the file a program can still connect by a connection string.
pub(crate) fn install_connections(app_name: &str) {
    let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) else { return };
    let file = dir.join("sql-connections.toml");
    if !file.is_file() {
        return;
    }
    match cobolt_runtime::esql::catalog::SqlCatalog::from_deployment_file(&file, app_name) {
        Ok(catalog) => cobolt_runtime::esql::catalog::install(catalog),
        Err(e) => eprintln!("{app_name}: the SQL connections cannot be read: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value read back is the value written, for each of the three kinds.
    #[test]
    fn values_cross_both_ways() {
        let mut buf = Vec::new();
        buf.push(wire::DEC);
        buf.extend_from_slice(&(-12345i128).to_le_bytes());
        buf.push(2);
        buf.push(wire::FLOAT);
        buf.extend_from_slice(&1.5f64.to_le_bytes());
        buf.push(wire::BYTES);
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(b"ABC");
        let mut at = 0;
        assert!(matches!(take(&buf, &mut at), Some(Value::Dec(n)) if n.mantissa == -12345 && n.decimals == 2));
        assert!(matches!(take(&buf, &mut at), Some(Value::Float(f)) if f == 1.5));
        assert!(matches!(take(&buf, &mut at), Some(Value::Bytes(b)) if b == b"ABC"));
        assert_eq!(at, buf.len());
        assert!(take(&buf, &mut at).is_none(), "nothing past the end");
        assert!(take(&[wire::BYTES, 9, 0, 0, 0, b'A'], &mut 0).is_none(), "a count past the end is refused");
        println!("prsql values: 3 kinds read back, 2 damaged inputs refused");
    }
}
