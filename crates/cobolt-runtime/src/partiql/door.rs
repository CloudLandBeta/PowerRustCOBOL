// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The IndexedFile control's PartiQL members (spec 108), one set of rules for every host.**
//!
//! The interpreter and the desktop runner (DESKTOP-WASM-001 T48) resolve the file a control names,
//! prepare a statement, open the file, run the statement and publish what it did with these
//! functions, so a control's answers, its properties and the file it leaves behind are the same on
//! both paths. What stays with each host is where the open file lives (the interpreter's run unit,
//! the runner's file store), the lock that guards it, and the program's data items.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::indexed::{status, IndexedEngine, IndexedStore, OpenMode};
use crate::indexed_log::{LogFormat, LogLevel};
use crate::partiql::analyze::{analyze, Analyzed, RStmt};
use crate::partiql::exec::{bind, execute, execute_unit, Arg, ColInfo, Item, Outcome};
use crate::partiql::parser::{parse_script, parse_statement, Parsed};
use crate::partiql::schema::Schema;
use crate::partiql::value::Value;
use crate::partiql::PartiqlError;

/// An error that is the file's, not the statement's: its FILE STATUS and its text.
pub fn file_error(code: &'static str, message: impl Into<String>) -> PartiqlError {
    PartiqlError { status: code, exception: "", message: message.into(), pos: None }
}

/// The control's `OpenMode` property as a mode: `INPUT` unless it says otherwise.
pub fn open_mode_of(text: &str) -> OpenMode {
    match text.trim().to_ascii_uppercase().as_str() {
        "OUTPUT" => OpenMode::Output,
        "EXTEND" => OpenMode::Extend,
        "I-O" | "IO" | "INPUT-OUTPUT" => OpenMode::Io,
        _ => OpenMode::Input,
    }
}

/// Whether a property holds one of the words that turn a switch on (`AutoOpen`).
pub fn is_on(text: &str) -> bool {
    matches!(text.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

/// Whether the statements commit as they complete (`AutoCommit`, R24): on or off as the property
/// says; with no word, on for a program that declares no `SELECT` for the file (`declares` false),
/// off for one that does: it drives the file with the verbs and keeps its transaction.
pub fn auto_commit(property: &str, declares: bool) -> bool {
    match property.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => !declares,
    }
}

/// Whether a program whose `SELECT`s are `files` (each its name and the `ASSIGN` it names)
/// declares the file of `table` at `data_path`: by the name, or by the file it assigns.
pub fn declares_file<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>, table: &str, data_path: &Path) -> bool {
    files
        .into_iter()
        .any(|(name, assign)| name.eq_ignore_ascii_case(table) || Path::new(assign).file_name().is_some_and(|f| Some(f) == data_path.file_name()))
}

/// The path a file is known by, so two names for one file are one file.
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The file a control names, resolved.
pub struct Target {
    /// The `IndexedFile` property as it was read; a change re-resolves.
    pub cidx: String,
    pub def: Arc<cobolt_indexed::IndexedDefinition>,
    pub schema: Arc<Schema>,
    pub data_path: PathBuf,
}

/// The definition, schema and data path the `.cidx` `cidx` names (the `IndexedFile` property,
/// trimmed). Reads the `.cidx` and the data file's header only: nothing is opened or created.
pub fn resolve(cidx: &str) -> Result<Target, PartiqlError> {
    if cidx.is_empty() {
        return Err(file_error(status::FILE_NOT_FOUND, "the IndexedFile property names no definition (.cidx)"));
    }
    let def_file = cobolt_forms::assets::resolve(cidx);
    let def = cobolt_indexed::load_indexed(&def_file).map_err(|_| file_error(status::FILE_NOT_FOUND, format!("cannot read the definition {cidx}")))?;
    let table = Path::new(cidx).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_uppercase();
    let data_path = cobolt_forms::assets::resolve(&def.assign_path);
    let from_def = Schema::from_definition(&table, &def)
        .map_err(|m| file_error(status::ATTR_MISMATCH, format!("the definition does not describe a record: {m}")))?;
    let mut schema = from_def.clone();
    if data_path.exists() {
        if let Ok(Some(info)) = crate::indexed_import::inspect_any_path(&data_path) {
            if info.record_format.max_len() as usize != from_def.record_len {
                return Err(file_error(
                    status::ATTR_MISMATCH,
                    format!("the data file's records are {} bytes; the definition says {}", info.record_format.max_len(), from_def.record_len),
                ));
            }
            // The layout the file carries wins (R10) when it agrees.
            if let Some(text) = info.definition {
                let (p, a) = Schema::keys_of_definition(&def);
                if let Ok(s) = Schema::from_fd_text(&table, &text, p, a) {
                    if s.record_len == from_def.record_len {
                        schema = s;
                    }
                }
            }
        }
    }
    Ok(Target { cidx: cidx.to_string(), def: Arc::new(def), schema: Arc::new(schema), data_path })
}

/// A statement, or a unit of them (`ExecuteTransaction`), parsed: the first thing a member call
/// does, before it looks for the file.
pub struct Statements {
    parsed: Vec<Parsed>,
    unit: bool,
}

/// Parse `text` (the statement, its trailing spaces gone); `unit` for `ExecuteTransaction`.
pub fn parse(text: &str, unit: bool) -> Result<Statements, PartiqlError> {
    let parsed = if unit { parse_script(text)? } else { vec![parse_statement(text)?] };
    Ok(Statements { parsed, unit })
}

impl Statements {
    /// Check the statements against the file's `schema` and bind them to `args`: everything that
    /// can fail before the file is opened.
    pub fn prepare(&self, schema: &Schema, args: &[Arg]) -> Result<Prepared, PartiqlError> {
        let mut analyzed: Vec<Analyzed> = Vec::with_capacity(self.parsed.len());
        for p in &self.parsed {
            analyzed.push(analyze(p, schema, self.unit)?);
        }
        let refs: Vec<&Analyzed> = analyzed.iter().collect();
        let params = bind(&refs, args)?;
        Ok(Prepared { analyzed, params, unit: self.unit })
    }
}

/// Statements ready to run.
pub struct Prepared {
    analyzed: Vec<Analyzed>,
    params: Vec<Value>,
    unit: bool,
}

impl Prepared {
    /// Run them on the open file.
    pub fn run(&self, store: &mut dyn IndexedStore, schema: &Schema) -> Outcome {
        if self.unit {
            execute_unit(store, schema, &self.analyzed, &self.params)
        } else {
            execute(store, schema, &self.analyzed[0], &self.params)
        }
    }

    /// Whether any statement changes the file (anything but `SELECT` and `EXISTS`): such
    /// statements commit as they complete when `AutoCommit` is on.
    pub fn writes(&self) -> bool {
        self.analyzed.iter().any(|a| !matches!(a.stmt, RStmt::Select(_) | RStmt::Exists(_)))
    }
}

/// What a host configures for the engines it opens.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    /// The engine a new file is made with; an existing file opens with the one that wrote it.
    pub engine: IndexedEngine,
    pub log_level: LogLevel,
    pub log_format: LogFormat,
}

/// Make the data file from the definition when it is not there (R10), for a control that may
/// write; a read-only open finds nothing (35).
pub fn create_if_missing(def: &cobolt_indexed::IndexedDefinition, data_path: &Path, mode: OpenMode) -> Result<(), PartiqlError> {
    if data_path.exists() {
        return Ok(());
    }
    if mode == OpenMode::Input {
        return Err(file_error(status::FILE_NOT_FOUND, "the data file does not exist"));
    }
    if let Some(dir) = data_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    crate::indexed_ide::create_empty_from_definition(def, data_path).map_err(|e| file_error(status::IO_ERROR, format!("cannot create the data file: {e}")))
}

/// Open the control's engine on `path` in `mode` for the operator `user` (`OperatorName`; blank
/// for none): the engine, or the FILE STATUS that refused it, as an error with that status's text.
pub fn open_engine(def: &cobolt_indexed::IndexedDefinition, path: &str, mode: OpenMode, user: &str, cfg: EngineConfig) -> Result<Box<dyn IndexedStore>, PartiqlError> {
    let mut engine = engine(def, path, cfg);
    if !user.trim().is_empty() {
        engine.set_registered_user(Some(user.trim().to_string()));
    }
    let code = engine.open(mode);
    if !code.starts_with('0') {
        return Err(file_error(code, crate::partiql::exec::status_text(code)));
    }
    Ok(engine)
}

/// The engine for a definition, chosen as an `OPEN` verb chooses it: the container's own magic
/// first, then the configured engine.
pub fn engine(def: &cobolt_indexed::IndexedDefinition, path: &str, cfg: EngineConfig) -> Box<dyn IndexedStore> {
    use crate::indexed::IndexedFile;
    use crate::indexed_disk::DiskIndexedFile;
    use crate::indexed_redb::RedbIndexedFile;
    let (primary, alts) = crate::indexed_ide::key_specs_from_def(def);
    let reclen = (def.record_length() as usize).max(1);
    let names: Vec<Option<String>> = std::iter::once(def.keys.primary.name.clone()).chain(def.keys.alternates.iter().map(|k| k.name.clone())).collect();
    let text = Some(crate::indexed_ide::definition_text_of(def));
    let engine = crate::file_core::detect_container_engine(path).unwrap_or(cfg.engine);
    if engine == IndexedEngine::Redb && def.storage == cobolt_indexed::StorageMode::Disk {
        let mut e = RedbIndexedFile::new(path, reclen, primary, alts);
        e.set_key_names(names);
        e.set_compressing(def.compression);
        e.set_definition(text);
        e.set_log_level(cfg.log_level);
        e.set_log_format(cfg.log_format);
        return Box::new(e);
    }
    match def.storage {
        cobolt_indexed::StorageMode::Memory => {
            let mut e = IndexedFile::new(path, reclen, primary, alts);
            e.set_key_names(names);
            e.set_compressing(def.compression);
            e.set_persist(def.persistence);
            e.set_definition(text);
            Box::new(e)
        }
        cobolt_indexed::StorageMode::Disk => {
            let mut e = DiskIndexedFile::new(path, reclen, primary, alts);
            e.set_key_names(names);
            e.set_compressing(def.compression);
            e.set_definition(text);
            Box::new(e)
        }
    }
}

/// How the program wrote an argument of a member call, so it is typed as R3 says. A desktop
/// module names each with one character ([`ArgKind::of_char`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// A numeric data item, a numeric literal or an arithmetic expression: typed by its value.
    Number,
    /// Anything else the program wrote (an alphanumeric item or literal, a group): a String.
    Value,
    /// A control's property: untyped text (O2), whatever it looks like.
    Property,
}

impl ArgKind {
    /// `N` a number, `P` a property, anything else a value.
    pub fn of_char(c: char) -> ArgKind {
        match c {
            'N' => ArgKind::Number,
            'P' => ArgKind::Property,
            _ => ArgKind::Value,
        }
    }

    /// The character a desktop module sends for this kind.
    pub fn as_char(self) -> char {
        match self {
            ArgKind::Number => 'N',
            ArgKind::Property => 'P',
            ArgKind::Value => 'S',
        }
    }
}

/// One `?` argument from its text and how it was written: a number by its value (characters
/// that spell no number stay a String, so the statement fails and names the argument rather than
/// matching an invented number), a property as untyped text, anything else as a String.
pub fn arg_of(text: &str, kind: ArgKind) -> Arg {
    match kind {
        ArgKind::Property => Arg::Text(text.to_string()),
        ArgKind::Number => match crate::value::parse_decimal(text.trim()) {
            Some(n) => Arg::Value(Value::Num(n)),
            None => Arg::Value(Value::text(text)),
        },
        ArgKind::Value => Arg::Value(Value::text(text)),
    }
}

/// What `Get` was asked for.
pub enum GetBy {
    /// A number's whole part: the column's place (below 1 finds nothing).
    Place(i128),
    /// Text: a place when it spells one, else a column's name.
    Text(String),
    /// No argument.
    None,
}

/// What one `IndexedFile` control keeps between calls.
#[derive(Default)]
pub struct ControlState {
    pub(crate) items: Vec<Item>,
    /// The next item `Fetch` delivers.
    pub(crate) cursor: usize,
    /// The item `Fetch` last delivered, for `Get`.
    pub(crate) fetched: Option<Item>,
    pub(crate) columns: Arc<Vec<ColInfo>>,
    /// The properties the last `Fetch` wrote, blanked by the next.
    pub(crate) published: Vec<String>,
}

/// What a statement left on the control.
pub struct Applied {
    /// What `Execute` answers: items in the result set, else records affected.
    pub count: usize,
    /// The FILE STATUS, for `StatusDataItem`.
    pub status: &'static str,
    /// It ran and has columns: bindings with no query of their own follow it (R34).
    pub follow: bool,
}

/// What `Fetch` delivered.
pub struct Fetched {
    /// The item's values, TAB-separated (a TAB inside a value becomes a space); empty past the last.
    pub row: String,
    /// The whole record, for `CurrentRecordDataItem`: `None` past the last, and for an item that
    /// was read from no record.
    pub record: Option<Vec<u8>>,
    /// `00`, or `10` past the last.
    pub status: &'static str,
}

impl ControlState {
    /// Publish what a statement did, writing each property through `set`: the properties the last
    /// `Fetch` wrote blanked, then `ResultCount`, `StatusCode` and `LastError`.
    pub fn apply(&mut self, o: Outcome, set: &mut dyn FnMut(&str, String)) -> Applied {
        let stale = std::mem::take(&mut self.published);
        let n = o.items.len();
        let last_error = o.last_error();
        let follow = o.error.is_none() && !o.columns.is_empty();
        self.items = o.items;
        self.cursor = 0;
        self.fetched = None;
        self.columns = o.columns;
        for p in stale {
            set(&p, String::new());
        }
        set("ResultCount", n.to_string());
        set("StatusCode", o.status.to_string());
        set("LastError", last_error);
        Applied { count: o.count, status: o.status, follow }
    }

    /// `Fetch()`: the next item, the properties of its columns written through `set` (those the
    /// last `Fetch` wrote blanked first) and `StatusCode` `00`; past the last, `StatusCode` `10`.
    pub fn fetch(&mut self, set: &mut dyn FnMut(&str, String)) -> Fetched {
        let item = if self.cursor >= self.items.len() {
            self.fetched = None;
            None
        } else {
            let item = self.items[self.cursor].clone();
            self.cursor += 1;
            self.fetched = Some(item.clone());
            Some(item)
        };
        for p in std::mem::take(&mut self.published) {
            set(&p, String::new());
        }
        let Some(item) = item else {
            set("StatusCode", "10".to_string());
            return Fetched { row: String::new(), record: None, status: "10" };
        };
        for (i, col) in item.cols.iter().enumerate() {
            if let Some(p) = &col.prop {
                set(p, item.text(i));
                self.published.push(p.clone());
            }
        }
        set("StatusCode", "00".to_string());
        let record = (!item.rec.is_empty()).then(|| item.rec.clone());
        let row = (0..item.values.len()).map(|i| item.text(i).replace('\t', " ")).collect::<Vec<_>>().join("\t");
        Fetched { row, record, status: "00" }
    }

    /// `Get(place | name)`: a value of the item `Fetch` last delivered; empty when there is none.
    pub fn get(&self, which: GetBy) -> String {
        let Some(item) = &self.fetched else { return String::new() };
        let position = match &which {
            GetBy::Place(n) => Some((*n).max(0) as usize),
            GetBy::Text(t) => t.trim().parse::<usize>().ok(),
            GetBy::None => None,
        };
        let index = match position {
            Some(p) => p.checked_sub(1),
            None => {
                let name = match which {
                    GetBy::Text(t) => t.trim().to_ascii_uppercase(),
                    _ => String::new(),
                };
                item.cols.iter().position(|c| c.name.to_ascii_uppercase() == name)
            }
        };
        index.map(|i| item.text(i)).unwrap_or_default()
    }

    /// The last result set's column names.
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }

    /// `ColumnCount()`.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}
