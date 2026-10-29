// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `IndexedFile` control's PartiQL members (spec 108, R20–R22) and the file
//! it holds (R7–R10).
//!
//! Routed **by the control's class** before the general method dispatch, as the
//! AWS and KnowledgeBase controls are: `Get`, `Execute` and `Fetch` mean
//! something else on other controls. The language itself lives in
//! [`crate::partiql`]; this is the glue that gives it a file, the arguments, and
//! the control's result set, `StatusCode`, `LastError` and properties.
//!
//! **Who holds the file (D1).** The run unit's `open_files` is the one
//! registry of open keyed engines, so the control opens its file there and the
//! `COMMIT` / `ROLLBACK` verbs act on it with every other. A file the control
//! opened is closed when the run ends.

use std::path::PathBuf;
use std::sync::Arc;

use super::{Interpreter, OpenFile};
use crate::indexed::{status, IndexedEngine, IndexedStore, OpenMode};
use crate::partiql::analyze::{analyze, Analyzed};
use crate::partiql::exec::{bind, execute, execute_unit, Arg, ColInfo, Item, Outcome};
use crate::partiql::parser::{parse_script, parse_statement};
use crate::partiql::schema::Schema;
use crate::partiql::value::Value;
use crate::partiql::PartiqlError;
use crate::value::CobolValue;

/// What one `IndexedFile` control keeps between calls.
#[derive(Default)]
pub(crate) struct PartiqlControl {
    items: Vec<Item>,
    /// The next item `Fetch` delivers.
    cursor: usize,
    /// The item `Fetch` last delivered, for `Get`.
    fetched: Option<Item>,
    columns: Arc<Vec<ColInfo>>,
    /// The properties the last `Fetch` wrote, blanked by the next.
    published: Vec<String>,
    target: Option<Target>,
}

/// The last `COBOL::"PARTIQL"` result on one file: its items, the next one
/// `PARTIQL-FETCH` delivers, and the text of the last error.
#[derive(Default)]
pub(crate) struct FileResults {
    items: Vec<Item>,
    cursor: usize,
    error: String,
}

/// A file a control opened and still holds.
pub(crate) struct OwnedFile {
    key: String,
    path: PathBuf,
    mode: OpenMode,
}

fn canonical(p: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The file a control names, resolved.
struct Target {
    /// The `IndexedFile` property as it was read; a change re-resolves.
    cidx: String,
    def: Arc<cobolt_indexed::IndexedDefinition>,
    schema: Arc<Schema>,
    data_path: PathBuf,
}

fn file_error(code: &'static str, message: impl Into<String>) -> PartiqlError {
    PartiqlError { status: code, exception: "", message: message.into(), pos: None }
}

fn open_mode_of(text: &str) -> OpenMode {
    match text.trim().to_ascii_uppercase().as_str() {
        "OUTPUT" => OpenMode::Output,
        "EXTEND" => OpenMode::Extend,
        "I-O" | "IO" | "INPUT-OUTPUT" => OpenMode::Io,
        _ => OpenMode::Input,
    }
}

impl Interpreter {
    pub(crate) fn is_partiql(&self, obj: &str) -> bool {
        self.object_class(obj.trim()).is_some_and(|c| c == "IndexedFile")
    }

    fn partiql_key(obj: &str) -> String {
        obj.trim().to_ascii_uppercase()
    }

    /// Note which arguments of the call about to be made came from a control's
    /// property: their text is untyped (O2), whatever it looks like.
    pub(crate) fn partiql_note_origins(&mut self, args: &[cobolt_ast::expr::Expr]) {
        use cobolt_ast::expr::Expr;
        self.partiql_arg_origin.clear();
        if args.is_empty() {
            return;
        }
        let origins: Vec<Option<String>> = args
            .iter()
            .map(|a| match a {
                Expr::Member { recv, member, args, parens: false, .. } if args.is_empty() => match recv.as_ref() {
                    Expr::Identifier(root, _)
                        if self.objects.contains(root.trim()) && !self.env.is_declared(root, &[]) =>
                    {
                        Some(self.obj_get(root.trim(), member))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        self.partiql_arg_origin = origins;
    }

    /// The members of the control (R20). `None` leaves a method to the rest of
    /// the dispatch.
    pub(crate) fn partiql_method(&mut self, obj: &str, m: &str, args: &[CobolValue]) -> Option<String> {
        match m {
            "EXECUTE" | "EXECUTETRANSACTION" => {
                let unit = m == "EXECUTETRANSACTION";
                let origins = std::mem::take(&mut self.partiql_arg_origin);
                let text = args.first().map(|v| v.as_display_string()).unwrap_or_default();
                let text = text.trim_end().to_string();
                let rest: Vec<Arg> = args
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(i, v)| match origins.get(i).and_then(|o| o.clone()) {
                        Some(raw) => Arg::Text(raw),
                        None => match v {
                            CobolValue::Numeric(n) => Arg::Value(Value::Num(n.clone())),
                            other => Arg::Value(Value::text(&other.as_display_string())),
                        },
                    })
                    .collect();
                let outcome = self.partiql_outcome(obj, &text, &rest, unit);
                Some(self.partiql_apply(obj, outcome).to_string())
            }
            "FETCH" => Some(self.partiql_fetch(obj)),
            "GET" => {
                let key = Self::partiql_key(obj);
                let which = args.first().cloned();
                let ctl = self.partiql.get(&key)?;
                let Some(item) = &ctl.fetched else { return Some(String::new()) };
                let position = match &which {
                    Some(CobolValue::Numeric(n)) => Some(n.mantissa.max(0) as usize / 10usize.pow(n.decimals as u32)),
                    Some(v) => v.as_display_string().trim().parse::<usize>().ok(),
                    None => None,
                };
                let index = match position {
                    Some(p) => p.checked_sub(1),
                    None => {
                        let name = which.map(|v| v.as_display_string().trim().to_ascii_uppercase()).unwrap_or_default();
                        item.cols.iter().position(|c| c.name.to_ascii_uppercase() == name)
                    }
                };
                Some(index.map(|i| item.text(i)).unwrap_or_default())
            }
            "COLUMNNAMES" => {
                let key = Self::partiql_key(obj);
                let ctl = self.partiql.get(&key)?;
                Some(ctl.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join("\t"))
            }
            "COLUMNCOUNT" => {
                let key = Self::partiql_key(obj);
                Some(self.partiql.get(&key).map_or(0, |c| c.columns.len()).to_string())
            }
            _ => None,
        }
    }

    // ── Running a statement ──

    fn partiql_outcome(&mut self, obj: &str, text: &str, args: &[Arg], unit: bool) -> Outcome {
        let parsed = if unit { parse_script(text) } else { parse_statement(text).map(|p| vec![p]) };
        let parsed = match parsed {
            Ok(p) => p,
            Err(e) => return Outcome::failed(e),
        };
        let schema = match self.partiql_resolve(obj) {
            Ok(s) => s,
            Err(e) => return Outcome::failed(e),
        };
        let mut analyzed: Vec<Analyzed> = Vec::with_capacity(parsed.len());
        for p in &parsed {
            match analyze(p, &schema, unit) {
                Ok(a) => analyzed.push(a),
                Err(e) => return Outcome::failed(e),
            }
        }
        let refs: Vec<&Analyzed> = analyzed.iter().collect();
        let params = match bind(&refs, args) {
            Ok(p) => p,
            Err(e) => return Outcome::failed(e),
        };
        let fkey = match self.partiql_open_file(obj) {
            Ok(k) => k,
            Err(e) => return Outcome::failed(e),
        };
        let mut handle = self.open_files.remove(&fkey);
        let outcome = match handle.as_mut() {
            Some(OpenFile::Indexed(store)) => {
                if unit {
                    execute_unit(store.as_mut(), &schema, &analyzed, &params)
                } else {
                    execute(store.as_mut(), &schema, &analyzed[0], &params)
                }
            }
            _ => Outcome::failed(file_error(status::LOGIC_ERROR, "the file is not open")),
        };
        if outcome.error.is_none() && analyzed.iter().any(|a| !matches!(a.stmt, crate::partiql::analyze::RStmt::Select(_) | crate::partiql::analyze::RStmt::Exists(_))) {
            let data_path = self.partiql.get(&Self::partiql_key(obj)).and_then(|c| c.target.as_ref()).map(|t| t.data_path.clone());
            if let Some(dp) = data_path {
                if self.partiql_auto_commit(obj, &schema.table, &dp) {
                    if let Some(OpenFile::Indexed(store)) = handle.as_mut() {
                        store.commit();
                    }
                }
            }
        }
        if let Some(h) = handle {
            self.open_files.insert(fkey, h);
        }
        outcome
    }

    /// Publish what a statement did: the result set, `ResultCount`,
    /// `StatusCode`, `LastError`, and `StatusDataItem`. Answers the count.
    fn partiql_apply(&mut self, obj: &str, o: Outcome) -> usize {
        let key = Self::partiql_key(obj);
        let ctl = self.partiql.entry(key).or_default();
        let stale = std::mem::take(&mut ctl.published);
        let n = o.items.len();
        let last_error = o.last_error();
        let has_columns = !o.columns.is_empty();
        ctl.items = o.items;
        ctl.cursor = 0;
        ctl.fetched = None;
        ctl.columns = o.columns;
        for p in stale {
            self.obj_set(obj, &p, String::new());
        }
        self.obj_set(obj, "ResultCount", n.to_string());
        self.obj_set(obj, "StatusCode", o.status.to_string());
        self.obj_set(obj, "LastError", last_error);
        self.partiql_status_item(obj, o.status);
        // Bindings with no query of their own show the new result (R34).
        if o.error.is_none() && has_columns {
            self.partiql_follow(obj);
        }
        o.count
    }

    fn partiql_status_item(&mut self, obj: &str, code: &str) {
        let item = self.obj_get(obj, "StatusDataItem");
        let item = item.trim();
        if !item.is_empty() && self.env.is_declared(item, &[]) {
            self.env.set_str(item, code);
        }
    }

    /// `Fetch()`: the next item, TAB-separated; past the last, empty with `10`.
    fn partiql_fetch(&mut self, obj: &str) -> String {
        let key = Self::partiql_key(obj);
        let (item, stale) = {
            let Some(ctl) = self.partiql.get_mut(&key) else {
                self.obj_set(obj, "StatusCode", "10".to_string());
                self.partiql_status_item(obj, "10");
                return String::new();
            };
            if ctl.cursor >= ctl.items.len() {
                ctl.fetched = None;
                (None, std::mem::take(&mut ctl.published))
            } else {
                let item = ctl.items[ctl.cursor].clone();
                ctl.cursor += 1;
                ctl.fetched = Some(item.clone());
                (Some(item), std::mem::take(&mut ctl.published))
            }
        };
        for p in stale {
            self.obj_set(obj, &p, String::new());
        }
        let Some(item) = item else {
            self.obj_set(obj, "StatusCode", "10".to_string());
            self.partiql_status_item(obj, "10");
            return String::new();
        };
        let mut published = Vec::new();
        for (i, col) in item.cols.iter().enumerate() {
            if let Some(p) = &col.prop {
                self.obj_set(obj, p, item.text(i));
                published.push(p.clone());
            }
        }
        if let Some(ctl) = self.partiql.get_mut(&key) {
            ctl.published = published;
        }
        let into = self.obj_get(obj, "CurrentRecordDataItem");
        let into = into.trim();
        if !into.is_empty() && !item.rec.is_empty() && self.env.is_declared(into, &[]) {
            self.env.set_group_bytes(into, &item.rec);
        }
        self.obj_set(obj, "StatusCode", "00".to_string());
        self.partiql_status_item(obj, "00");
        (0..item.values.len())
            .map(|i| item.text(i).replace('\t', " "))
            .collect::<Vec<_>>()
            .join("\t")
    }

    // ── The file ──

    /// The definition, schema and data path the control names. Reads the
    /// `.cidx` and the data file's header only: nothing is opened or created.
    fn partiql_resolve(&mut self, obj: &str) -> Result<Arc<Schema>, PartiqlError> {
        let cidx = self.obj_get(obj, "IndexedFile").trim().to_string();
        let key = Self::partiql_key(obj);
        if let Some(t) = self.partiql.get(&key).and_then(|c| c.target.as_ref()) {
            if t.cidx == cidx {
                return Ok(t.schema.clone());
            }
        }
        if cidx.is_empty() {
            return Err(file_error(status::FILE_NOT_FOUND, "the IndexedFile property names no definition (.cidx)"));
        }
        let def_file = cobolt_forms::assets::resolve(&cidx);
        let def = cobolt_indexed::load_indexed(&def_file)
            .map_err(|_| file_error(status::FILE_NOT_FOUND, format!("cannot read the definition {cidx}")))?;
        let table = std::path::Path::new(&cidx)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();
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
        let schema = Arc::new(schema);
        let ctl = self.partiql.entry(key).or_default();
        ctl.target = Some(Target { cidx, def: Arc::new(def), schema: schema.clone(), data_path });
        Ok(schema)
    }

    /// The `open_files` key of the control's file, opening it when need be.
    fn partiql_open_file(&mut self, obj: &str) -> Result<String, PartiqlError> {
        let key = Self::partiql_key(obj);
        let (def, schema, data_path) = {
            let t = self.partiql.get(&key).and_then(|c| c.target.as_ref()).ok_or_else(|| file_error(status::LOGIC_ERROR, "no file"))?;
            (t.def.clone(), t.schema.clone(), t.data_path.clone())
        };
        if let Some(k) = self.partiql_open.get(&canonical(&data_path)) {
            if self.open_files.contains_key(k) {
                return Ok(k.clone());
            }
        }
        let mode = open_mode_of(&self.obj_get(obj, "OpenMode"));
        let path = data_path.to_string_lossy().to_string();
        if !data_path.exists() {
            // R10: a file that is not there is made from the definition — for a
            // program that may write. A read-only open finds nothing (35).
            if mode == OpenMode::Input {
                return Err(file_error(status::FILE_NOT_FOUND, "the data file does not exist"));
            }
            if let Some(dir) = data_path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            crate::indexed_ide::create_empty_from_definition(&def, &data_path)
                .map_err(|e| file_error(status::IO_ERROR, format!("cannot create the data file: {e}")))?;
        }
        let fkey = format!("{}:{}", schema.table, key);
        if !self.acquire_share_lock(&fkey, &path, super::share_lock_is_exclusive(None, false)) {
            return Err(file_error(status::UNAVAILABLE, "the file is in use by another run"));
        }
        let mut engine = self.partiql_engine(&def, &path);
        let user = self.obj_get(obj, "OperatorName");
        if !user.trim().is_empty() {
            engine.set_registered_user(Some(user.trim().to_string()));
        }
        let code = engine.open(mode);
        if !code.starts_with('0') {
            self.forget_failed_open(&fkey);
            return Err(file_error(code, super::partiql_status_text(code)));
        }
        self.open_files.insert(fkey.clone(), OpenFile::Indexed(engine));
        let canon = canonical(&data_path);
        self.partiql_open.insert(canon.clone(), fkey.clone());
        self.partiql_owned.push(OwnedFile { key: fkey.clone(), path: canon, mode });
        Ok(fkey)
    }

    /// The engine for a definition, chosen as an `OPEN` verb chooses it: the
    /// container's own magic first, then the run's configured engine.
    fn partiql_engine(&self, def: &cobolt_indexed::IndexedDefinition, path: &str) -> Box<dyn IndexedStore> {
        use crate::indexed::IndexedFile;
        use crate::indexed_disk::DiskIndexedFile;
        use crate::indexed_redb::RedbIndexedFile;
        let (primary, alts) = crate::indexed_ide::key_specs_from_def(def);
        let reclen = (def.record_length() as usize).max(1);
        let names: Vec<Option<String>> = std::iter::once(def.keys.primary.name.clone())
            .chain(def.keys.alternates.iter().map(|k| k.name.clone()))
            .collect();
        let text = Some(crate::indexed_ide::definition_text_of(def));
        let engine = super::detect_container_engine(path).unwrap_or(self.indexed_engine);
        if engine == IndexedEngine::Redb && def.storage == cobolt_indexed::StorageMode::Disk {
            let mut e = RedbIndexedFile::new(path, reclen, primary, alts);
            e.set_key_names(names);
            e.set_compressing(def.compression);
            e.set_definition(text);
            e.set_log_level(self.indexed_log_level);
            e.set_log_format(self.indexed_log_format);
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

    // ── Data binding (spec 108, R30–R37) ──

    /// The arguments a binding's parameter list names: a data item by its
    /// PICTURE, `CONTROL::Property` as untyped text (O2).
    fn partiql_binding_args(&self, list: &str) -> Vec<Arg> {
        list.split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| match p.split_once("::") {
                Some((ctl, prop)) => Arg::Text(self.obj_get(ctl.trim(), prop.trim())),
                None => {
                    let name = self.env.resolve_name(p, &[]);
                    match self.env.get(&name) {
                        Some(CobolValue::Numeric(n)) => Arg::Value(Value::Num(n.clone())),
                        Some(other) => Arg::Value(Value::text(&other.as_display_string())),
                        None => Arg::Value(Value::Str(String::new())),
                    }
                }
            })
            .collect()
    }

    fn partiql_publish_failure(&mut self, ctl: &str, o: &Outcome) {
        self.obj_set(ctl, "StatusCode", o.status.to_string());
        self.obj_set(ctl, "LastError", o.last_error());
    }

    /// Fill a bound target from a PartiQL result set: run the binding's query on
    /// its control in a result set of its own (the control's is untouched), or —
    /// with no query — follow the control's. A failure keeps what the target
    /// shows and sets the control's `StatusCode` / `LastError` (R33).
    pub(crate) fn refresh_partiql_binding(&mut self, id: &str) -> usize {
        let ctl = self.obj_get(id, "_BindingPartiqlControl").trim().to_string();
        if ctl.is_empty() {
            return 0;
        }
        let own = self.obj_get(id, "BindingQuery");
        let query = if own.trim().is_empty() { self.obj_get(id, "_BindingPartiqlQuery") } else { own };
        let (items, columns): (Vec<Item>, Arc<Vec<ColInfo>>) = if query.trim().is_empty() {
            match self.partiql.get(&Self::partiql_key(&ctl)) {
                Some(c) => (c.items.clone(), c.columns.clone()),
                None => return 0,
            }
        } else {
            let args = self.partiql_binding_args(&self.obj_get(id, "_BindingPartiqlParams"));
            match parse_statement(query.trim()) {
                Ok(p) if matches!(p.statement, crate::partiql::parser::Statement::Select(_)) => {}
                Ok(_) => {
                    let o = Outcome::failed(PartiqlError::validation("a binding query must be a SELECT", None));
                    self.partiql_publish_failure(&ctl, &o);
                    return 0;
                }
                Err(e) => {
                    self.partiql_publish_failure(&ctl, &Outcome::failed(e));
                    return 0;
                }
            }
            let o = self.partiql_outcome(&ctl, query.trim(), &args, false);
            if o.error.is_some() {
                self.partiql_publish_failure(&ctl, &o);
                return 0;
            }
            (o.items, o.columns)
        };
        let wanted: Vec<String> = self
            .obj_get(id, "_BindingFields")
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect();
        let at: Vec<Option<usize>> = if wanted.is_empty() {
            (0..columns.len()).map(Some).collect()
        } else {
            wanted.iter().map(|w| columns.iter().position(|c| c.name.eq_ignore_ascii_case(w))).collect()
        };
        let cell = |item: &Item, i: &Option<usize>| i.map(|i| item.text(i).replace(['\t', '\n'], " ")).unwrap_or_default();
        let n = items.len();
        if self.obj_get(id, "_BindingList") == "1" {
            // Display column first, then — for `SelectedValue` — the value column.
            let shown: Vec<String> = items.iter().map(|it| at.first().map(|i| cell(it, i)).unwrap_or_default()).collect();
            let values: Vec<String> = items.iter().map(|it| at.get(1).map(|i| cell(it, i)).unwrap_or_default()).collect();
            self.obj_set(id, "Items", shown.join("\n"));
            self.obj_set(id, "_BindingValues", values.join("\n"));
        } else {
            let rows: Vec<String> =
                items.iter().map(|it| at.iter().map(|i| cell(it, i)).collect::<Vec<_>>().join("\t")).collect();
            self.obj_set(id, "Rows", rows.join("\n"));
        }
        n
    }

    /// Every PartiQL binding that follows `ctl`'s result set shows the new one.
    pub(crate) fn partiql_follow(&mut self, ctl: &str) {
        let followers: Vec<String> = self
            .objects
            .iter()
            .filter(|(_, o)| o.class != "IndexedFile")
            .map(|(id, _)| id.clone())
            .filter(|id| {
                self.obj_get(id, "_BindingKind").eq_ignore_ascii_case("PartiQl")
                    && self.obj_get(id, "_BindingPartiqlControl").trim().eq_ignore_ascii_case(ctl.trim())
                    && self.obj_get(id, "BindingQuery").trim().is_empty()
                    && self.obj_get(id, "_BindingPartiqlQuery").trim().is_empty()
            })
            .collect();
        for id in followers {
            self.refresh_partiql_binding(&id);
        }
    }

    /// R37: `BindingQuery` takes a `SELECT`; another statement is refused with
    /// `94` on the source control and the previous query stays.
    pub(crate) fn partiql_accept_query(&mut self, id: &str, text: &str) -> bool {
        if text.trim().is_empty() {
            return true;
        }
        let ctl = self.obj_get(id, "_BindingPartiqlControl").trim().to_string();
        let problem = match parse_statement(text.trim()) {
            Ok(p) if matches!(p.statement, crate::partiql::parser::Statement::Select(_)) => return true,
            Ok(_) => PartiqlError::validation("a binding query must be a SELECT", None),
            Err(e) => e,
        };
        if !ctl.is_empty() {
            self.partiql_publish_failure(&ctl, &Outcome::failed(problem));
        }
        false
    }

    /// `SelectedValue` of a PartiQL-bound list (the value field of the selected
    /// item) or grid (the value column of the clicked row; the value column is
    /// the first key field, else the first column). Empty before any pick.
    pub(crate) fn partiql_selected_value(&self, id: &str) -> Option<String> {
        let get = |p: &str| self.objects.get_property(id, p).map(|v| v.to_string()).unwrap_or_default();
        if !get("_BindingKind").eq_ignore_ascii_case("PartiQl") {
            return None;
        }
        if get("_BindingList") == "1" {
            let index: i64 = get("SelectedIndex").trim().parse().unwrap_or(0);
            let values = get("_BindingValues");
            let value = if index >= 1 { values.split('\n').nth(index as usize - 1).unwrap_or("").to_string() } else { String::new() };
            return Some(value);
        }
        let row: usize = get("ClickedRow").trim().parse().unwrap_or(0);
        if row == 0 {
            return Some(String::new());
        }
        let fields: Vec<String> = get("_BindingFields").split(',').map(|f| f.trim().to_string()).collect();
        let keys = get("_BindingKeyFields");
        let col = keys
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .find_map(|k| fields.iter().position(|f| f.eq_ignore_ascii_case(k)))
            .unwrap_or(0);
        let rows = get("Rows");
        let line = rows.split('\n').nth(row - 1).unwrap_or("");
        Some(line.split('\t').nth(col).unwrap_or("").to_string())
    }

    /// A program's `OPEN` of an INDEXED file succeeded: remember where it is,
    /// so a control that names the same file borrows this engine (R9).
    pub(crate) fn partiql_note_open(&mut self, file: &str, path: &str) {
        self.partiql_open.insert(canonical(std::path::Path::new(path)), file.to_string());
    }

    /// The program's `OPEN` of `file` meets a file a control holds: take that
    /// engine over under the program's name (true), or — when the control opened
    /// it for less than the program wants — close the control's and let the verb
    /// open its own (false). `false` also when no control holds the file.
    pub(crate) fn partiql_adopt(&mut self, file: &str, path: &str, mode: cobolt_ast::stmt::OpenMode) -> bool {
        let canon = canonical(std::path::Path::new(path));
        let Some(pos) = self.partiql_owned.iter().position(|o| o.path == canon) else { return false };
        let want = super::map_open_mode(mode);
        let owned = self.partiql_owned.remove(pos);
        let compatible = want == owned.mode || (want == OpenMode::Input && owned.mode == OpenMode::Io);
        if !compatible || self.open_files.contains_key(file) {
            if let Some(OpenFile::Indexed(mut s)) = self.open_files.remove(&owned.key) {
                s.close();
            }
            self.share_locks.remove(&owned.key);
            self.partiql_open.remove(&canon);
            return false;
        }
        if let Some(h) = self.open_files.remove(&owned.key) {
            self.open_files.insert(file.to_string(), h);
        }
        if let Some(l) = self.share_locks.remove(&owned.key) {
            self.share_locks.insert(file.to_string(), l);
        }
        self.partiql_open.insert(canon, file.to_string());
        true
    }

    /// Whether the statements commit as they complete (`AutoCommit`, R24): on
    /// by default for a program that declares no `SELECT` for this file, off for
    /// one that does — it drives the file with the verbs and keeps its transaction.
    fn partiql_auto_commit(&self, obj: &str, table: &str, data_path: &std::path::Path) -> bool {
        match self.obj_get(obj, "AutoCommit").trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => !self.file_specs.iter().any(|(name, spec)| {
                name.eq_ignore_ascii_case(table)
                    || std::path::Path::new(&spec.assign).file_name().is_some_and(|f| Some(f) == data_path.file_name())
            }),
        }
    }

    // ── COBOL::"PARTIQL" (Q6, D12): the same statements on a file the program's
    // own FD opened, for programs that have no control.

    /// The schema of a file the program declares, from its FD layout and keys.
    fn partiql_spec_schema(&self, file: &str) -> Option<Schema> {
        use crate::partiql::schema::KeyInput;
        let spec = self.file_specs.get(file)?;
        let layout = &spec.layout;
        let key_of = |name: &str, quals: &[String], parts: &[super::KeyField], dup: bool, alt: bool| -> Option<KeyInput> {
            let ranges: Vec<(String, usize, usize)> = if parts.is_empty() {
                let f = layout.field_qualified(name, quals)?;
                vec![(f.name.clone(), f.offset, f.len)]
            } else {
                parts
                    .iter()
                    .map(|p| layout.field_qualified(&p.name, &p.quals).map(|f| (f.name.clone(), f.offset, f.len)))
                    .collect::<Option<Vec<_>>>()?
            };
            Some(KeyInput { name: alt.then(|| name.to_string()), parts: ranges, duplicates: dup })
        };
        let primary = key_of(spec.record_key.as_deref()?, &spec.record_key_quals, &spec.record_key_parts, false, false)?;
        let alternates: Vec<KeyInput> = spec
            .alternate_keys
            .iter()
            .filter_map(|ak| key_of(&ak.field, &ak.quals, &ak.parts, ak.with_duplicates, true))
            .collect();
        Some(Schema::from_layout(file, layout, primary, alternates))
    }

    fn partiql_set_item(&mut self, arg: &super::CallArg, value: &str, numeric: bool) {
        let n = self.expr_to_name(super::call_arg_expr(arg));
        let n = self.env.resolve_name(&n, &[]);
        if numeric {
            self.env.set(&n, CobolValue::from_i64(value.parse().unwrap_or(0)));
        } else if self.env.is_group(&n) {
            self.env.set_group(&n, value);
        } else {
            self.env.set_str(&n, value);
        }
    }

    pub(crate) fn partiql_builtin_run(
        &mut self,
        using: &[super::CallArg],
        span: super::Span,
    ) -> Result<(), super::RuntimeError> {
        let file = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_ascii_uppercase();
        let text = self.eval_call_arg(&using[1], span)?.as_display_string().trim_end().to_string();
        let mut args = Vec::new();
        for a in &using[4..] {
            args.push(match self.eval_call_arg(a, span)? {
                CobolValue::Numeric(n) => Arg::Value(Value::Num(n)),
                other => Arg::Value(Value::text(&other.as_display_string())),
            });
        }
        let outcome = self.partiql_file_outcome(&file, &text, &args);
        let mut res = FileResults::default();
        res.error = outcome.last_error();
        let count = outcome.count;
        let status = outcome.status;
        res.items = outcome.items;
        self.partiql_files.insert(file, res);
        self.partiql_set_item(&using[2], &count.to_string(), true);
        self.partiql_set_item(&using[3], status, false);
        Ok(())
    }

    fn partiql_file_outcome(&mut self, file: &str, text: &str, args: &[Arg]) -> Outcome {
        let parsed = match parse_statement(text) {
            Ok(p) => p,
            Err(e) => return Outcome::failed(e),
        };
        let Some(schema) = self.partiql_spec_schema(file) else {
            return Outcome::failed(file_error(status::FILE_NOT_FOUND, "the program declares no such indexed file"));
        };
        let a = match analyze(&parsed, &schema, false) {
            Ok(a) => a,
            Err(e) => return Outcome::failed(e),
        };
        let params = match bind(&[&a], args) {
            Ok(p) => p,
            Err(e) => return Outcome::failed(e),
        };
        match self.open_files.remove(file) {
            Some(OpenFile::Indexed(mut store)) => {
                let o = execute(store.as_mut(), &schema, &a, &params);
                self.open_files.insert(file.to_string(), OpenFile::Indexed(store));
                o
            }
            other => {
                if let Some(h) = other {
                    self.open_files.insert(file.to_string(), h);
                }
                Outcome::failed(file_error(status::NOT_OPEN_INPUT, "the file is not open"))
            }
        }
    }

    pub(crate) fn partiql_builtin_fetch(
        &mut self,
        using: &[super::CallArg],
        span: super::Span,
    ) -> Result<(), super::RuntimeError> {
        let file = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_ascii_uppercase();
        let (row, code) = match self.partiql_files.get_mut(&file) {
            Some(r) if r.cursor < r.items.len() => {
                let item = &r.items[r.cursor];
                r.cursor += 1;
                ((0..item.values.len()).map(|i| item.text(i).replace('\t', " ")).collect::<Vec<_>>().join("\t"), "00")
            }
            _ => (String::new(), "10"),
        };
        self.partiql_set_item(&using[1], &row, false);
        self.partiql_set_item(&using[2], code, false);
        Ok(())
    }

    pub(crate) fn partiql_builtin_error(
        &mut self,
        using: &[super::CallArg],
        span: super::Span,
    ) -> Result<(), super::RuntimeError> {
        let file = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_ascii_uppercase();
        let text = self.partiql_files.get(&file).map(|r| r.error.clone()).unwrap_or_default();
        self.partiql_set_item(&using[1], &text, false);
        Ok(())
    }

    /// The run is ending: close every file a control opened (R8) and release
    /// its lock.
    pub(crate) fn partiql_close_owned(&mut self) {
        for owned in std::mem::take(&mut self.partiql_owned) {
            let fkey = owned.key;
            if let Some(OpenFile::Indexed(mut store)) = self.open_files.remove(&fkey) {
                store.close();
            }
            self.share_locks.remove(&fkey);
            self.partiql_open.retain(|_, k| *k != fkey);
        }
    }
}
