// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The IndexedFile control's PartiQL members** (DESKTOP-WASM-001 T48, ABI A55).
//!
//! The ABI host hands the runner a method called on an IndexedFile; it runs by the interpreter's own
//! rules (`cobolt_runtime::partiql::door`): the file the control names is resolved, the statement
//! parsed, checked and bound, the file opened, the statement run and its result published. The file
//! lives in the program's file store ([`crate::files::Files`]), so a control works on the program's
//! own open file when the program holds it (spec 108 R9), the program's `OPEN` takes over a file a
//! control opened, and `COMMIT` and `ROLLBACK` reach it, as under the interpreter.
//!
//! What the module sends: the arguments as the program wrote them, every character kept, and after
//! them one more, the kind of each argument after the statement (`N` a number, `P` a control's
//! property, `S` anything else), so each is typed as the interpreter types it by its declaration
//! (R3). At the form's start each IndexedFile hears `_START` with the program's `SELECT`s (names
//! and `ASSIGN`s, TAB-separated), for `AutoOpen` and `AutoCommit`, which depend on whether the
//! program declares the control's file.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cobolt_abi_host::host::{PartiqlControl, PartiqlEffects, PartiqlPort};
use cobolt_runtime::indexed::status;
use cobolt_runtime::indexed_log::{LogFormat, LogLevel};
use cobolt_runtime::partiql::door::{self, ArgKind, ControlState, EngineConfig, GetBy, Target};
use cobolt_runtime::partiql::exec::{Arg, Outcome};
use cobolt_runtime::partiql::PartiqlError;

use crate::files::Files;

/// What one IndexedFile control of one form instance keeps between calls.
#[derive(Default)]
struct Ctl {
    state: ControlState,
    target: Option<Target>,
}

/// The runner's IndexedFile controls (T48).
pub struct Port {
    files: Rc<RefCell<Files>>,
    /// By form instance and the control's name (upper case), from the first call that resolved its
    /// file or ran a statement, as the interpreter keeps them.
    ctls: HashMap<(u32, String), Ctl>,
    /// Each form instance's `SELECT`s: `(name, ASSIGN)`.
    programs: HashMap<u32, Vec<(String, String)>>,
}

/// The kinds the module sent after the arguments, and the arguments.
fn split_kinds<'a>(args: &'a [&'a str]) -> (&'a str, &'a [&'a str]) {
    match args.split_last() {
        Some((kinds, rest)) => (kinds, rest),
        None => ("", args),
    }
}

impl Port {
    pub fn new(files: Rc<RefCell<Files>>) -> Port {
        Port { files, ctls: HashMap::new(), programs: HashMap::new() }
    }

    /// Whether instance `inst`'s program declares the file of `table` at `data_path`.
    fn declares(&self, inst: u32, table: &str, data_path: &std::path::Path) -> bool {
        let files = self.programs.get(&inst).map(Vec::as_slice).unwrap_or(&[]);
        door::declares_file(files.iter().map(|(n, a)| (n.as_str(), a.as_str())), table, data_path)
    }

    /// The file the control names, resolved once for each value of its `IndexedFile` property.
    fn resolve(&mut self, key: &(u32, String), c: &PartiqlControl<'_>) -> Result<(), PartiqlError> {
        let cidx = (c.prop)("IndexedFile").trim().to_string();
        if self.ctls.get(key).and_then(|k| k.target.as_ref()).is_some_and(|t| t.cidx == cidx) {
            return Ok(());
        }
        let target = door::resolve(&cidx)?;
        self.ctls.entry(key.clone()).or_default().target = Some(target);
        Ok(())
    }

    /// The engines' configuration: the run's engine for a new file (the project's choice, as for
    /// the program's own files), and the program's own files' log.
    fn engine_config(&self) -> EngineConfig {
        EngineConfig { engine: self.files.borrow().engine(), log_level: LogLevel::Off, log_format: LogFormat::Text }
    }

    /// Make sure the control's file is open for instance `inst`: the program's own open file there,
    /// a file one of the instance's controls opened, or a new one, created when it is not there and
    /// the control may write (R10).
    fn open(&mut self, inst: u32, key: &(u32, String), c: &PartiqlControl<'_>) -> Result<(), PartiqlError> {
        let cfg = self.engine_config();
        let t = self.ctls.get(key).and_then(|k| k.target.as_ref()).ok_or_else(|| door::file_error(status::LOGIC_ERROR, "no file"))?;
        let mut files = self.files.borrow_mut();
        files.acting_for(inst);
        if files.partiql_engine(inst, &door::canonical(&t.data_path)).is_some() {
            return Ok(());
        }
        let mode = door::open_mode_of(&(c.prop)("OpenMode"));
        door::create_if_missing(&t.def, &t.data_path, mode)?;
        let path = t.data_path.to_string_lossy().to_string();
        let lock = match cobolt_runtime::file_core::share_lock(&path, false, None) {
            cobolt_runtime::file_core::ShareLock::Held(h) => Some(h),
            cobolt_runtime::file_core::ShareLock::NotLocked => None,
            cobolt_runtime::file_core::ShareLock::Refused => return Err(door::file_error(status::UNAVAILABLE, "the file is in use by another run")),
        };
        let engine = door::open_engine(&t.def, &path, mode, &(c.prop)("OperatorName"), cfg)?;
        files.partiql_hold(inst, door::canonical(&t.data_path), engine, mode, lock);
        Ok(())
    }

    /// `Execute` / `ExecuteTransaction`: parse, resolve, prepare, open, run, and commit when
    /// `AutoCommit` says so, in the interpreter's order.
    fn outcome(&mut self, inst: u32, key: &(u32, String), c: &PartiqlControl<'_>, text: &str, args: &[Arg], unit: bool) -> Outcome {
        let statements = match door::parse(text, unit) {
            Ok(s) => s,
            Err(e) => return Outcome::failed(e),
        };
        if let Err(e) = self.resolve(key, c) {
            return Outcome::failed(e);
        }
        let (schema, data_path) = {
            let t = self.ctls[key].target.as_ref().expect("resolved");
            (t.schema.clone(), t.data_path.clone())
        };
        let prepared = match statements.prepare(&schema, args) {
            Ok(p) => p,
            Err(e) => return Outcome::failed(e),
        };
        if let Err(e) = self.open(inst, key, c) {
            return Outcome::failed(e);
        }
        let commit = door::auto_commit(&(c.prop)("AutoCommit"), self.declares(inst, &schema.table, &data_path));
        let mut files = self.files.borrow_mut();
        let Some(store) = files.partiql_engine(inst, &door::canonical(&data_path)) else {
            return Outcome::failed(door::file_error(status::LOGIC_ERROR, "the file is not open"));
        };
        let outcome = prepared.run(store, &schema);
        if outcome.error.is_none() && prepared.writes() && commit {
            store.commit();
        }
        outcome
    }

    /// `_START` at the form's start: the program's `SELECT`s, then `AutoOpen` for a control whose
    /// file the program does not declare (its facade is not generated, R10).
    fn start(&mut self, inst: u32, key: &(u32, String), c: &PartiqlControl<'_>, args: &[&str]) {
        let names = args.first().copied().unwrap_or("");
        let assigns = args.get(1).copied().unwrap_or("");
        let files: Vec<(String, String)> =
            names.split('\t').zip(assigns.split('\t')).filter(|(n, _)| !n.is_empty()).map(|(n, a)| (n.to_string(), a.to_string())).collect();
        self.programs.insert(inst, files);
        if !door::is_on(&(c.prop)("AutoOpen")) || self.resolve(key, c).is_err() {
            return;
        }
        let declared = {
            let t = self.ctls[key].target.as_ref().expect("resolved");
            self.declares(inst, &t.schema.table, &t.data_path)
        };
        if !declared {
            let _ = self.open(inst, key, c);
        }
    }
}

impl PartiqlPort for Port {
    fn call(&mut self, inst: u32, c: &PartiqlControl<'_>, method: &str, args: &[&str]) -> Option<PartiqlEffects> {
        let key = (inst, c.name.trim().to_ascii_uppercase());
        match method {
            "_START" => {
                self.start(inst, &key, c, args);
                Some(PartiqlEffects::default())
            }
            "EXECUTE" | "EXECUTETRANSACTION" => {
                let (kinds, args) = split_kinds(args);
                let text = args.first().copied().unwrap_or("").trim_end();
                let mut kinds = kinds.chars();
                let rest: Vec<Arg> = args.iter().skip(1).map(|t| door::arg_of(t, ArgKind::of_char(kinds.next().unwrap_or('S')))).collect();
                let outcome = self.outcome(inst, &key, c, text, &rest, method == "EXECUTETRANSACTION");
                let mut props = Vec::new();
                let applied = self.ctls.entry(key).or_default().state.apply(outcome, &mut |p, v| props.push((p.to_string(), v)));
                Some(PartiqlEffects { props, answer: applied.count.to_string() })
            }
            "FETCH" => {
                let mut props = Vec::new();
                let Some(ctl) = self.ctls.get_mut(&key) else {
                    return Some(PartiqlEffects { props: vec![("StatusCode".into(), "10".into())], answer: String::new() });
                };
                let fetched = ctl.state.fetch(&mut |p, v| props.push((p.to_string(), v)));
                Some(PartiqlEffects { props, answer: fetched.row })
            }
            "GET" => {
                let ctl = self.ctls.get(&key)?;
                let (kinds, args) = split_kinds(args);
                let which = match args.first() {
                    None => GetBy::None,
                    Some(t) if kinds.starts_with('N') => match cobolt_runtime::value::parse_decimal(t.trim()) {
                        Some(n) => GetBy::Place(n.mantissa.max(0) / 10i128.pow(n.decimals as u32)),
                        None => GetBy::Text(t.to_string()),
                    },
                    Some(t) => GetBy::Text(t.to_string()),
                };
                Some(PartiqlEffects { props: Vec::new(), answer: ctl.state.get(which) })
            }
            "COLUMNNAMES" => {
                let ctl = self.ctls.get(&key)?;
                Some(PartiqlEffects { props: Vec::new(), answer: ctl.state.column_names().join("\t") })
            }
            "COLUMNCOUNT" => Some(PartiqlEffects { props: Vec::new(), answer: self.ctls.get(&key).map_or(0, |c| c.state.column_count()).to_string() }),
            _ => None,
        }
    }

    fn release(&mut self, inst: u32) {
        self.ctls.retain(|(i, _), _| *i != inst);
        self.programs.remove(&inst);
    }
}
