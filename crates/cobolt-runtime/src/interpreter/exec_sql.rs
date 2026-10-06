// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Executing `EXEC SQL` statements (spec 087 R6–R24, R34–R36, R48).
//!
//! One statement, start to end:
//!
//! 1. its host variables are read — a group is a host structure and stands
//!    for its elementary items; a negative indicator sends NULL — and bound as
//!    parameters, never spliced (R13);
//! 2. the run unit is locked only for the driver call;
//! 3. every output is converted and checked FIRST, and assigned only if every
//!    one fits (R11, R24): a number that does not fit its item, or NULL with
//!    no indicator, fails the statement and changes nothing;
//! 4. every status item the program declares is set — stand-alone
//!    `SQLSTATE`/`SQLCODE`/`SQLMSG` and the SQLCA's fields (R17);
//! 5. the debugger's SQL channel gets one line (R48), a password never in it;
//! 6. `WHENEVER` acts, through the interpreter's own `GO TO` (R20). An SQL
//!    failure is never a runtime error by itself (R22).

use cobolt_ast::program::Program;
use cobolt_ast::sql::{
    ExecSql, SqlCursor, SqlCursorQuery, SqlDisconnect, SqlHostName, SqlHostRef, SqlInto, SqlKind, SqlPart, SqlText,
    SqlUsing, SqlValue as SqlValue_,
};

use super::Interpreter;
use crate::error::RuntimeError;
use crate::esql::backend::{self, BackendKind};
use crate::esql::session::Session;
use crate::esql::state::{outcome, sqlcode, Outcome};
use crate::esql::value::{parse_decimal, SqlValue};
use crate::esql::{code, is_connection_string, SqlError};
use crate::value::{CobolNumeric, CobolValue};

/// Where this program's status items live (each name may be declared more
/// than once: a stand-alone `SQLSTATE` and the SQLCA's).
#[derive(Debug, Clone, Default)]
pub(crate) struct StatusKeys {
    sqlstate: Vec<String>,
    sqlcode: Vec<String>,
    sqlmsg: Vec<String>,
    message_length: Vec<String>,
    rows: Vec<String>,
    native: Vec<String>,
    warning: Vec<String>,
    truncated: Vec<String>,
    connection: Vec<String>,
}

/// How a statement ended, before the status items are set.
struct Done {
    state: String,
    message: String,
    rows: i64,
    native: i64,
}

impl Done {
    fn ok(rows: i64) -> Self {
        Self { state: code::SUCCESS.into(), message: String::new(), rows, native: 0 }
    }
}

impl From<SqlError> for Done {
    fn from(e: SqlError) -> Self {
        Self { state: e.sqlstate, message: e.message, rows: 0, native: e.native }
    }
}

/// What the debugger's SQL channel shows of one statement.
#[derive(Default)]
struct Trace {
    text: String,
    values: Vec<String>,
}

/// One output slot: an elementary item and its indicator.
struct Slot {
    key: String,
    indicator: Option<String>,
}

/// A converted output, ready to assign.
struct Ready {
    key: String,
    value: Option<CobolValue>,
    indicator: Option<(String, i64)>,
    /// Text was cut to fit the item (`01004`).
    cut: bool,
}

/// A fresh identity for an interpreter in the run unit's cursor table.
pub(crate) fn next_instance() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Every cursor each program of a tree can name (R28): its own, and the
/// DATA DIVISION cursors of the programs around it.
fn index_cursors(p: &Program, inherited: &[SqlCursor], into: &mut std::collections::HashMap<(String, String), SqlCursor>) {
    let owner = p.identification.program_id.to_ascii_uppercase();
    for c in inherited {
        into.insert((owner.clone(), c.name.clone()), c.clone());
    }
    for c in &p.sql_cursors {
        into.insert((owner.clone(), c.name.clone()), c.clone());
    }
    let mut pass: Vec<SqlCursor> = inherited.to_vec();
    pass.extend(p.sql_cursors.iter().filter(|c| c.in_data_division).cloned());
    for n in &p.nested_programs {
        index_cursors(n, &pass, into);
    }
}

/// A password inside a connection URL (`scheme://user:secret@host`) masked.
fn mask_url(s: &str) -> String {
    if let Some(p) = s.find("://") {
        let rest = &s[p + 3..];
        if let Some(at) = rest.find('@') {
            if let Some(colon) = rest[..at].find(':') {
                return format!("{}://{}:******{}", &s[..p], &rest[..colon], &rest[at..]);
            }
        }
    }
    s.to_string()
}

impl Interpreter {
    /// This interpreter's run unit — what a host hands to the forms it opens.
    pub fn sql_run_unit(&self) -> std::sync::Arc<std::sync::Mutex<crate::esql::SqlRunUnit>> {
        self.sql_unit.clone()
    }

    /// Share `unit` (R37): every form of the application uses the same SQL
    /// connections. `root` is the interpreter that ends the run unit — the
    /// main program or main form; a child form joins with `root` false.
    pub fn set_sql_run_unit(&mut self, unit: std::sync::Arc<std::sync::Mutex<crate::esql::SqlRunUnit>>, root: bool) {
        self.sql_unit = unit;
        self.sql_root = root;
    }

    /// The end of this interpreter's part of the run unit (R38). The root
    /// commits every connection's open work after a normal end — `STOP RUN`,
    /// `GOBACK`, the main window closing — and rolls it back after an error or
    /// a cancel (an IDE Stop), then closes every connection. A child form
    /// only releases its own cursors and prepared statements.
    pub(crate) fn sql_end(&mut self, normal: bool) {
        let Ok(mut unit) = self.sql_unit.lock() else { return };
        if !self.sql_root {
            unit.release_instance(self.sql_instance);
            return;
        }
        if unit.sessions.is_empty() {
            return;
        }
        for s in unit.sessions.iter_mut() {
            let ended = if normal { s.commit() } else { s.rollback() };
            if let Err(e) = ended {
                // A commit that fails leaves nothing half-done: undo it.
                let _ = s.rollback();
                if let Some(tx) = &self.debug_event_tx {
                    let _ = tx.send(crate::debugger::DebugEvent::Output {
                        text: format!("SQL connection {} could not commit at the end of the run: {}", s.name, e.message),
                        channel: crate::debugger::OutputChannel::Problems,
                    });
                }
            }
        }
        unit.cursors.clear();
        unit.prepared.clear();
        unit.sessions.clear();
        unit.current = None;
    }

    /// Give this run the SQL connections it can name (the IDE's in-process
    /// runner; other hosts install the catalog for the whole process).
    pub fn set_sql_catalog(&mut self, catalog: crate::esql::catalog::SqlCatalog) {
        if let Ok(mut u) = self.sql_unit.lock() {
            u.catalog = Some(std::sync::Arc::new(catalog));
        }
    }

    /// Run one `EXEC SQL` statement.
    pub(crate) fn exec_sql(&mut self, e: &ExecSql) -> Result<(), RuntimeError> {
        if matches!(e.kind, SqlKind::Declarative) {
            return Ok(());
        }
        let mut trace = Trace::default();
        let done = match self.sql_run(&e.kind, &e.owner, &mut trace) {
            Ok(d) => d,
            Err(err) => Done::from(err),
        };
        self.sql_set_status(&done);
        self.sql_trace(&trace, &done);
        let target = match outcome(&done.state) {
            Outcome::Error => &e.whenever.sqlerror,
            Outcome::Warning => &e.whenever.sqlwarning,
            Outcome::NoData => &e.whenever.not_found,
            Outcome::Success => &None,
        };
        match target {
            Some(t) => Err(RuntimeError::GoTo { target: t.clone(), section: None }),
            None => Ok(()),
        }
    }

    fn sql_lock(&self) -> Result<std::sync::MutexGuard<'_, crate::esql::SqlRunUnit>, SqlError> {
        self.sql_unit.lock().map_err(|_| SqlError::new(code::GENERAL, "the SQL connections are unusable after a crash"))
    }

    fn sql_current_kind(&self) -> Result<BackendKind, SqlError> {
        let mut unit = self.sql_lock()?;
        Ok(unit.current_mut()?.kind())
    }

    fn sql_run(&mut self, kind: &SqlKind, owner: &str, trace: &mut Trace) -> Result<Done, SqlError> {
        if matches!(
            kind,
            SqlKind::Execute(_)
                | SqlKind::SelectInto { .. }
                | SqlKind::Open { .. }
                | SqlKind::Commit
                | SqlKind::Rollback
                | SqlKind::ExecutePrepared { .. }
                | SqlKind::ExecuteImmediate(_)
                | SqlKind::Describe { .. }
        ) {
            self.sql_ensure_current()?;
        }
        match kind {
            SqlKind::Declarative => Ok(Done::ok(0)),
            SqlKind::Execute(t) => {
                let bk = self.sql_current_kind()?;
                let (sql, params) = self.sql_bind(t, bk, owner, trace)?;
                let n = self.sql_lock()?.current_mut()?.execute(&sql, &params, true)?;
                // An INSERT, UPDATE or DELETE that touched no row found no data.
                let verb = sql.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
                if n == 0 && matches!(verb.as_str(), "INSERT" | "UPDATE" | "DELETE" | "MERGE") {
                    return Ok(Done { state: code::NO_DATA.into(), message: "no row was affected".into(), rows: 0, native: 0 });
                }
                Ok(Done::ok(n as i64))
            }
            SqlKind::SelectInto { text, into } => {
                let bk = self.sql_current_kind()?;
                let (sql, params) = self.sql_bind(text, bk, owner, trace)?;
                let rows = self.sql_lock()?.current_mut()?.query(&sql, &params)?;
                match rows.rows.len() {
                    0 => Ok(Done { state: code::NO_DATA.into(), message: "no row was found".into(), rows: 0, native: 0 }),
                    1 => {
                        let state = self.sql_assign(into, &rows.rows[0])?;
                        Ok(Done { state, message: String::new(), rows: 1, native: 0 })
                    }
                    n => Err(SqlError::new(
                        code::CARDINALITY,
                        format!("SELECT … INTO returned {n} rows; it must return one (use a cursor to read several)"),
                    )),
                }
            }
            SqlKind::Commit | SqlKind::Rollback => {
                let committed = matches!(kind, SqlKind::Commit);
                trace.text = if committed { "COMMIT" } else { "ROLLBACK" }.into();
                let mut unit = self.sql_lock()?;
                let session = unit.current_mut()?;
                let name = session.name.clone();
                if committed {
                    session.commit()?;
                } else {
                    session.rollback()?;
                }
                // R30: cursors end with the unit of work, WITH HOLD ones
                // surviving a COMMIT.
                unit.end_unit(&name, committed);
                Ok(Done::ok(0))
            }
            SqlKind::Connect { target, alias, user, password: _ } => {
                let target = self.sql_value_text(target)?;
                let shown_user = match user {
                    Some(u) => format!(" USER {}", self.sql_value_text(u)?),
                    None => String::new(),
                };
                trace.text = format!("CONNECT TO {}{shown_user}{}", mask_url(&target), if user.is_some() { " USING ******" } else { "" });
                self.sql_connect(&target, alias.as_deref())
            }
            SqlKind::SetConnection(v) => {
                let name = self.sql_value_text(v)?;
                trace.text = format!("SET CONNECTION {name}");
                let mut unit = self.sql_lock()?;
                match unit.find(&name) {
                    Some(i) => {
                        unit.current = Some(i);
                        Ok(Done::ok(0))
                    }
                    None => Err(SqlError::new(code::NO_CONNECTION, format!("there is no connection named {name}"))),
                }
            }
            SqlKind::Disconnect(which) => {
                let name = match which {
                    SqlDisconnect::Named(v) => Some(self.sql_value_text(v)?),
                    _ => None,
                };
                let mut unit = self.sql_lock()?;
                match which {
                    SqlDisconnect::All => {
                        trace.text = "DISCONNECT ALL".into();
                        unit.close_all();
                    }
                    SqlDisconnect::Current => {
                        trace.text = "DISCONNECT CURRENT".into();
                        match unit.current {
                            Some(i) => unit.close(i),
                            None => return Err(SqlError::new(code::NO_CONNECTION, "no SQL connection is current")),
                        }
                    }
                    SqlDisconnect::Named(_) => {
                        let name = name.unwrap_or_default();
                        trace.text = format!("DISCONNECT {name}");
                        match unit.find(&name) {
                            Some(i) => unit.close(i),
                            None => return Err(SqlError::new(code::NO_CONNECTION, format!("there is no connection named {name}"))),
                        }
                    }
                }
                Ok(Done::ok(0))
            }
            SqlKind::Open { cursor, using } => self.sql_open(owner, cursor, using, trace),
            SqlKind::Fetch { cursor, into } => self.sql_fetch(owner, cursor, into, trace),
            SqlKind::Close { cursor } => {
                trace.text = format!("CLOSE {cursor}");
                let key = (self.sql_instance, owner.to_string(), cursor.clone());
                match self.sql_lock()?.cursors.remove(&key) {
                    Some(_) => Ok(Done::ok(0)),
                    None => Err(SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {cursor} is not open"))),
                }
            }
            SqlKind::Prepare { name, from } => {
                // R45: the text as it is NOW; later changes to its item do not
                // change the prepared statement.
                let text = self.sql_value_text(from)?;
                let text = cobolt_lexer::sql::strip_line_comments(&text).trim().to_string();
                trace.text = format!("PREPARE {name} FROM {text}");
                self.sql_lock()?.prepared.insert((self.sql_instance, name.clone()), text);
                Ok(Done::ok(0))
            }
            SqlKind::ExecuteImmediate(v) => {
                let text = self.sql_value_text(v)?;
                let text = cobolt_lexer::sql::strip_line_comments(&text).trim().to_string();
                trace.text = text.clone();
                let bk = self.sql_current_kind()?;
                let sql = crate::esql::rewrite::question_marks(&text, bk);
                let n = self.sql_lock()?.current_mut()?.execute(&sql, &[], true)?;
                Ok(Done::ok(n as i64))
            }
            SqlKind::ExecutePrepared { name, using } => {
                let text = self.sql_prepared(name)?;
                let bk = self.sql_current_kind()?;
                let params = self.sql_using_values(using)?;
                let sql = crate::esql::rewrite::question_marks(&text, bk);
                trace.text = format!("EXECUTE {name}: {sql}");
                trace.values = params.iter().map(|v| v.display()).collect();
                let n = self.sql_lock()?.current_mut()?.execute(&sql, &params, true)?;
                Ok(Done::ok(n as i64))
            }
            SqlKind::Describe { name, sqlda, input } => self.sql_describe(name, sqlda, *input, trace),
        }
    }

    /// The declaration of the cursor `name` as program `owner` sees it.
    fn sql_cursor_decl(&mut self, owner: &str, name: &str) -> Result<SqlCursor, SqlError> {
        if self.sql_cursor_decls.is_none() {
            let mut map = std::collections::HashMap::new();
            index_cursors(&self.program, &[], &mut map);
            self.sql_cursor_decls = Some(map);
        }
        self.sql_cursor_decls
            .as_ref()
            .and_then(|m| m.get(&(owner.to_string(), name.to_string())))
            .cloned()
            .ok_or_else(|| SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {name} is not declared")))
    }

    /// `OPEN cursor` (R25): evaluate its host variables now and read its rows.
    fn sql_open(&mut self, owner: &str, cursor: &str, using: &SqlUsing, trace: &mut Trace) -> Result<Done, SqlError> {
        let key = (self.sql_instance, owner.to_string(), cursor.to_string());
        if self.sql_lock()?.cursors.contains_key(&key) {
            return Err(SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {cursor} is already open")));
        }
        let decl = self.sql_cursor_decl(owner, cursor)?;
        let bk = self.sql_current_kind()?;
        let (mut sql, params) = match &decl.query {
            SqlCursorQuery::Static(t) => {
                if !matches!(using, SqlUsing::None) {
                    return Err(SqlError::new(
                        code::NOT_SUPPORTED,
                        "OPEN … USING is for a cursor over a prepared statement",
                    ));
                }
                self.sql_bind(t, bk, owner, trace)?
            }
            // R43: the prepared statement as it stands when the cursor opens.
            SqlCursorQuery::Prepared(s) => {
                let text = self.sql_prepared(s)?;
                let params = self.sql_using_values(using)?;
                trace.values = params.iter().map(|v| v.display()).collect();
                (crate::esql::rewrite::question_marks(&text, bk), params)
            }
        };
        let keyed = decl.positioned && bk == BackendKind::Sqlite;
        if bk == BackendKind::Sqlite {
            sql = crate::esql::rewrite::strip_for_update(&sql);
            if keyed {
                sql = crate::esql::rewrite::sqlite_keyed_select(&sql)?;
            }
        }
        trace.text = format!("OPEN {cursor}: {sql}");
        let mut unit = self.sql_lock()?;
        let session = unit.current_mut()?;
        let name = session.name.clone();
        let rows = session.query(&sql, &params)?;
        unit.cursors.insert(
            key,
            crate::esql::OpenCursor {
                session: name,
                rows: rows.rows,
                next: 0,
                with_hold: decl.with_hold,
                keyed,
                current: None,
            },
        );
        Ok(Done::ok(0))
    }

    /// `FETCH cursor INTO …` (R25): the next row, or no data past the last.
    fn sql_fetch(&mut self, owner: &str, cursor: &str, into: &SqlInto, trace: &mut Trace) -> Result<Done, SqlError> {
        trace.text = format!("FETCH {cursor}");
        let key = (self.sql_instance, owner.to_string(), cursor.to_string());
        let (row, keyed) = {
            let mut unit = self.sql_lock()?;
            let Some(cur) = unit.cursors.get_mut(&key) else {
                return Err(SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {cursor} is not open")));
            };
            if cur.next >= cur.rows.len() {
                cur.current = None;
                return Ok(Done { state: code::NO_DATA.into(), message: "no more rows".into(), rows: 0, native: 0 });
            }
            let row = cur.rows[cur.next].clone();
            cur.next += 1;
            cur.current = if cur.keyed { row.first().cloned() } else { Some(SqlValue::Null) };
            (row, cur.keyed)
        };
        let values = if keyed { &row[1..] } else { &row[..] };
        let state = match into {
            SqlInto::Hosts(h) => self.sql_assign(h, values)?,
            SqlInto::Descriptor(d) => self.sqlda_fetch(d, values)?,
        };
        Ok(Done { state, message: String::new(), rows: 1, native: 0 })
    }

    /// A prepared statement's text, or `07003`.
    fn sql_prepared(&self, name: &str) -> Result<String, SqlError> {
        self.sql_lock()?
            .prepared
            .get(&(self.sql_instance, name.to_string()))
            .cloned()
            .ok_or_else(|| SqlError::new(code::PREPARED_NOT_FOUND, format!("statement {name} is not prepared")))
    }

    /// The parameters a `USING` clause supplies (R42, R43).
    fn sql_using_values(&self, using: &SqlUsing) -> Result<Vec<SqlValue>, SqlError> {
        match using {
            SqlUsing::None => Ok(Vec::new()),
            SqlUsing::Hosts(hs) => {
                let mut out = Vec::new();
                for h in hs {
                    out.extend(self.sql_input_values(h)?);
                }
                Ok(out)
            }
            SqlUsing::Descriptor(d) => self.sqlda_inputs(d),
        }
    }

    // ── The SQL descriptor area (R41, R44) ────────────────────────────────

    /// The storage key of field `field` of descriptor `d`, entry `i` (1-based).
    fn sqlda_key(&self, d: &SqlHostName, field: &str, i: Option<usize>) -> String {
        let mut quals = vec![d.name.clone()];
        quals.extend(d.quals.iter().cloned());
        let k = self.env.resolve_name(field, &quals);
        match i {
            Some(i) => crate::environment::subscript_key(&k, &[i as i64]),
            None => k,
        }
    }

    fn sqlda_int(&self, key: &str) -> i64 {
        self.env.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
    }

    fn sqlda_set_int(&mut self, key: &str, v: i64) {
        self.env.set(key, CobolValue::Numeric(CobolNumeric::integer(v)));
    }

    /// How many entries descriptor `d` holds: its `SQLDA-CAPACITY`, never more
    /// than its `OCCURS`.
    fn sqlda_capacity(&self, d: &SqlHostName) -> Result<usize, SqlError> {
        let tag = self.sqlda_key(d, "SQLDA-TAG", None);
        if self.env.symbol(&tag).is_none() {
            return Err(SqlError::new(code::GENERAL, format!("{} is not an SQL descriptor area", d.name)));
        }
        let cap = self.sqlda_int(&self.sqlda_key(d, "SQLDA-CAPACITY", None)).max(0) as usize;
        let occurs = self
            .env
            .symbol(&self.sqlda_key(d, "SQLDA-ENTRY", None))
            .map(|s| s.occurs)
            .unwrap_or(0);
        Ok(cap.min(occurs))
    }

    /// `DESCRIBE [INPUT] s INTO d` (R44): the result columns, or the
    /// parameters. Too small a descriptor gets `SQLDA-NEEDED` and `01005`,
    /// and no entry is touched.
    fn sql_describe(&mut self, name: &str, d: &SqlHostName, input: bool, trace: &mut Trace) -> Result<Done, SqlError> {
        let text = self.sql_prepared(name)?;
        let bk = self.sql_current_kind()?;
        let sql = crate::esql::rewrite::question_marks(&text, bk);
        trace.text = format!("DESCRIBE{} {name}: {sql}", if input { " INPUT" } else { "" });
        let (params, cols) = self.sql_lock()?.current_mut()?.describe(&sql)?;
        let entries: Vec<crate::esql::sqlda::Described> = if input {
            // SQLite and MySQL do not report parameter types: unknown, not guessed.
            (1..=params).map(|i| crate::esql::sqlda::Described::unknown(format!("?{i}"))).collect()
        } else {
            cols.iter()
                .map(|c| crate::esql::sqlda::describe_declared(&c.name, c.decl_type.as_deref(), c.nullable))
                .collect()
        };
        let cap = self.sqlda_capacity(d)?;
        let needed = entries.len();
        let k = self.sqlda_key(d, "SQLDA-NEEDED", None);
        self.sqlda_set_int(&k, needed as i64);
        if needed > cap {
            return Ok(Done {
                state: code::DESCRIPTOR_TOO_SMALL.into(),
                message: format!("the descriptor holds {cap} entries; {needed} are needed"),
                rows: 0,
                native: 0,
            });
        }
        let k = self.sqlda_key(d, "SQLDA-COUNT", None);
        self.sqlda_set_int(&k, needed as i64);
        for (i, e) in entries.iter().enumerate() {
            let n = Some(i + 1);
            let k = self.sqlda_key(d, "SQLDA-NAME", n);
            self.env.set_str(&k, &e.name);
            let k = self.sqlda_key(d, "SQLDA-TYPE-NAME", n);
            self.env.set_str(&k, &e.type_name);
            for (field, v) in [
                ("SQLDA-TYPE", e.code),
                ("SQLDA-LENGTH", e.length),
                ("SQLDA-PRECISION", e.precision),
                ("SQLDA-SCALE", e.scale),
                ("SQLDA-NULLABLE", e.nullable),
            ] {
                let k = self.sqlda_key(d, field, n);
                self.sqlda_set_int(&k, v);
            }
        }
        Ok(Done::ok(0))
    }

    /// The parameters descriptor `d` supplies: through each entry's data
    /// pointer, or the text inside the entry when the pointer is NULL.
    fn sqlda_inputs(&self, d: &SqlHostName) -> Result<Vec<SqlValue>, SqlError> {
        let count = self.sqlda_int(&self.sqlda_key(d, "SQLDA-COUNT", None)).max(0) as usize;
        let cap = self.sqlda_capacity(d)?;
        if count > cap {
            return Err(SqlError::new(code::DESCRIPTOR_TOO_SMALL, format!("SQLDA-COUNT {count} exceeds the descriptor's {cap} entries")));
        }
        let mut out = Vec::with_capacity(count);
        for i in 1..=count {
            let data = self.env.addr_target(self.sqlda_int(&self.sqlda_key(d, "SQLDA-DATA", Some(i))));
            let ind = match self.env.addr_target(self.sqlda_int(&self.sqlda_key(d, "SQLDA-IND-PTR", Some(i)))) {
                Some(k) => self.sqlda_int(&k),
                None => self.sqlda_int(&self.sqlda_key(d, "SQLDA-IND", Some(i))),
            };
            out.push(if ind < 0 {
                SqlValue::Null
            } else {
                match data {
                    Some(k) => self.sql_read_item(&k),
                    None => self.sql_read_item(&self.sqlda_key(d, "SQLDA-VALUE", Some(i))),
                }
            });
        }
        Ok(out)
    }

    /// `FETCH … USING DESCRIPTOR d`: each column into the item its entry
    /// points at, or as text into the entry — all or none (R11, R41).
    fn sqlda_fetch(&mut self, d: &SqlHostName, row: &[SqlValue]) -> Result<String, SqlError> {
        let count = self.sqlda_int(&self.sqlda_key(d, "SQLDA-COUNT", None)).max(0) as usize;
        if count != row.len() {
            return Err(SqlError::new(
                code::WRONG_TARGET_COUNT,
                format!("the row has {} columns but the descriptor describes {count}", row.len()),
            ));
        }
        let mut ready = Vec::with_capacity(count);
        for (i, v) in row.iter().enumerate() {
            let n = Some(i + 1);
            let data = self.env.addr_target(self.sqlda_int(&self.sqlda_key(d, "SQLDA-DATA", n)));
            let ind_ptr = self.env.addr_target(self.sqlda_int(&self.sqlda_key(d, "SQLDA-IND-PTR", n)));
            let slot = match data {
                Some(key) => Slot { key, indicator: ind_ptr.or_else(|| Some(self.sqlda_key(d, "SQLDA-IND", n))) },
                None => Slot { key: self.sqlda_key(d, "SQLDA-VALUE", n), indicator: Some(self.sqlda_key(d, "SQLDA-IND", n)) },
            };
            ready.push(self.sql_convert(slot, v)?);
        }
        Ok(self.sql_apply(ready))
    }

    /// The SQL connections this run can name.
    /// Open a connection for the `COBOL::"OPEN-DB"` built-in or an
    /// `SqlDatabase` control: a connection string, or `sql-connection:NAME`
    /// for one of the project's SQL connections (R40), resolved exactly as
    /// `CONNECT TO 'NAME'` resolves it. The connection is the registry's own,
    /// never one of `EXEC SQL`'s.
    pub(crate) fn db_open(&mut self, conn: &str) -> Result<u32, String> {
        let Some(name) = crate::esql::catalog::named_connection(conn) else {
            return self.db.open(conn);
        };
        let catalog = self.sql_catalog().ok_or_else(|| {
            format!("there is no SQL connection named '{name}': this program was started without the project's SQL connections")
        })?;
        let target = catalog.resolve(name, &crate::esql::catalog::process_env).map_err(|e| e.message)?;
        self.db.open_target(&target)
    }

    fn sql_catalog(&self) -> Option<std::sync::Arc<crate::esql::catalog::SqlCatalog>> {
        self.sql_unit.lock().ok().and_then(|u| u.catalog.clone()).or_else(crate::esql::catalog::current)
    }

    /// R35: with no current connection, the default SQL connection is
    /// connected and made current; without one, the statement gets `08003`.
    fn sql_ensure_current(&mut self) -> Result<(), SqlError> {
        if self.sql_lock()?.current.is_some() {
            return Ok(());
        }
        let default = self.sql_catalog().and_then(|c| c.default_connection().map(|d| d.name.clone()));
        match default {
            Some(name) => self.sql_connect(&name, None).map(|_| ()),
            None => Err(SqlError::new(
                code::NO_CONNECTION,
                "no SQL connection is current, and the project marks none as the default",
            )),
        }
    }

    /// `CONNECT TO target [AS alias]` (R34): a project's SQL connection by
    /// name, or a clearly-shaped connection string; anything else is `08001`.
    fn sql_connect(&mut self, target: &str, alias: Option<&str>) -> Result<Done, SqlError> {
        let named = self.sql_catalog().and_then(|c| c.find(target).map(|d| (c.clone(), d.name.clone())));
        let (opened_as, open): (String, Box<dyn FnOnce() -> Result<Box<dyn crate::esql::backend::Backend>, SqlError>>) =
            match named {
                Some((cat, def_name)) => (
                    def_name.clone(),
                    Box::new(move || {
                        let t = cat.resolve(&def_name, &crate::esql::catalog::process_env)?;
                        backend::open_target(&t)
                    }),
                ),
                None if is_connection_string(target) => {
                    let t = target.to_string();
                    (target.trim().to_string(), Box::new(move || backend::open(&t)))
                }
                None => {
                    return Err(SqlError::new(
                        code::CANNOT_CONNECT,
                        format!("there is no SQL connection named '{}' in this project", target.trim()),
                    ))
                }
            };
        let name = alias.map(str::to_string).unwrap_or(opened_as);
        {
            let unit = self.sql_lock()?;
            if unit.find(&name).is_some() {
                return Err(SqlError::new(code::CONNECTION_EXISTS, format!("a connection named {name} is already open")));
            }
        }
        let b = open()?;
        let mut unit = self.sql_lock()?;
        unit.sessions.push(Session::new(name, b));
        unit.current = Some(unit.sessions.len() - 1);
        Ok(Done::ok(0))
    }

    /// The text of a value a statement names: a literal, a host variable's
    /// content, or a bare word.
    fn sql_value_text(&mut self, v: &SqlValue_) -> Result<String, SqlError> {
        Ok(match v {
            SqlValue_::Literal(s) => s.clone(),
            SqlValue_::Name(n) => n.clone(),
            SqlValue_::Host(h) => {
                let key = self.sql_host_key(&h.var)?;
                self.sql_read_item(&key).display().trim().to_string()
            }
        })
    }

    /// The storage key of a host name, trying both readings of `:A.B`.
    fn sql_host_key(&self, h: &SqlHostName) -> Result<String, SqlError> {
        let exists = |k: &str| self.env.symbol(k).is_some() || self.env.contains(k);
        let first = self.env.resolve_name(&h.name, &h.quals);
        if exists(&first) {
            return Ok(first);
        }
        if let Some((n, q)) = &h.swapped {
            let second = self.env.resolve_name(n, q);
            if exists(&second) {
                return Ok(second);
            }
        }
        Err(SqlError::new(code::GENERAL, format!("host variable :{} is not declared", h.name)))
    }

    /// The elementary items a host variable stands for (R8).
    fn sql_leaves(&self, key: &str) -> Result<Vec<String>, SqlError> {
        if self.env.is_group(key) {
            self.env.host_structure_leaves(key).map_err(|item| {
                SqlError::new(code::GENERAL, format!("host structure {key} holds {item}, which has OCCURS"))
            })
        } else {
            Ok(vec![key.to_string()])
        }
    }

    /// The indicator keys for `n` items: one for an elementary host variable,
    /// the table's occurrences for a host structure (R9).
    fn sql_indicator_keys(&self, ind: &SqlHostName, n: usize, structure: bool) -> Result<Vec<String>, SqlError> {
        let key = self.sql_host_key(ind)?;
        if !structure {
            return Ok(vec![key]);
        }
        let table = if self.env.is_group(&key) {
            self.env.symbol(&key).and_then(|s| s.child_keys.first().cloned()).unwrap_or(key.clone())
        } else {
            key.clone()
        };
        Ok((1..=n as i64).map(|i| crate::environment::subscript_key(&table, &[i])).collect())
    }

    /// An item's value for the database.
    fn sql_read_item(&self, key: &str) -> SqlValue {
        match self.env.get(key) {
            Some(CobolValue::Numeric(n)) => SqlValue::decimal(n.mantissa, n.decimals),
            Some(CobolValue::Float(f)) => SqlValue::Float(*f),
            Some(CobolValue::String { bytes, .. }) => {
                if !self.env.is_alphanumeric_field(key) && self.env.integer_capacity(key).is_some() {
                    if let Some(n) = self.env.deedited_value(key) {
                        return SqlValue::decimal(n.mantissa, n.decimals);
                    }
                }
                // Trailing spaces are padding, not data: comparing `'ABC'` with
                // a `PIC X(10)` item holding "ABC" must match on every database.
                SqlValue::Text(String::from_utf8_lossy(bytes).trim_end_matches(' ').to_string())
            }
            Some(CobolValue::Unset) | None => match self.env.group_value(key) {
                Some(g) => SqlValue::Text(g.trim_end_matches(' ').to_string()),
                None => SqlValue::Null,
            },
        }
    }

    /// The values one input host reference sends (R8, R12).
    fn sql_input_values(&self, h: &SqlHostRef) -> Result<Vec<SqlValue>, SqlError> {
        let key = self.sql_host_key(&h.var)?;
        let structure = self.env.is_group(&key);
        let leaves = self.sql_leaves(&key)?;
        let inds = match &h.indicator {
            Some(i) => Some(self.sql_indicator_keys(i, leaves.len(), structure)?),
            None => None,
        };
        Ok(leaves
            .iter()
            .enumerate()
            .map(|(i, leaf)| {
                let null = inds
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .and_then(|k| self.env.get(k))
                    .and_then(|v| v.as_i64())
                    .is_some_and(|n| n < 0);
                if null {
                    SqlValue::Null
                } else {
                    self.sql_read_item(leaf)
                }
            })
            .collect())
    }

    /// A statement's text for `kind`, with its parameters in order.
    fn sql_bind(&self, t: &SqlText, kind: BackendKind, owner: &str, trace: &mut Trace) -> Result<(String, Vec<SqlValue>), SqlError> {
        let mut sql = String::new();
        let mut params = Vec::new();
        for part in &t.parts {
            match part {
                SqlPart::Text(s) => sql.push_str(s),
                SqlPart::Input(n) => {
                    let h = t.inputs.get(*n).ok_or_else(|| SqlError::new(code::GENERAL, "internal: missing input"))?;
                    let vals = self.sql_input_values(h)?;
                    let marks: Vec<String> =
                        (0..vals.len()).map(|i| crate::esql::rewrite::placeholder(kind, params.len() + i)).collect();
                    sql.push_str(&marks.join(", "));
                    params.extend(vals);
                }
                SqlPart::CurrentOf(c) => {
                    // R26: the row the cursor last fetched, by its key.
                    let key = (self.sql_instance, owner.to_string(), c.clone());
                    let unit = self.sql_lock()?;
                    let Some(cur) = unit.cursors.get(&key) else {
                        return Err(SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {c} is not open")));
                    };
                    let Some(row) = cur.current.clone() else {
                        return Err(SqlError::new(code::INVALID_CURSOR_STATE, format!("cursor {c} is not on a row")));
                    };
                    if !cur.keyed || kind != BackendKind::Sqlite {
                        return Err(SqlError::new(
                            code::NOT_SUPPORTED,
                            format!("WHERE CURRENT OF {c} is not available for this cursor"),
                        ));
                    }
                    sql.push_str("rowid = ");
                    sql.push_str(&crate::esql::rewrite::placeholder(kind, params.len()));
                    params.push(row);
                }
            }
        }
        trace.text = sql.clone();
        trace.values = params.iter().map(|v| v.display()).collect();
        Ok((sql, params))
    }

    /// Convert a fetched row into the targets, and assign them all — or, if
    /// any one cannot take its value, none (R11, R24). Returns the SQLSTATE:
    /// `00000`, or `01004` when text was truncated.
    fn sql_assign(&mut self, targets: &[SqlHostRef], row: &[SqlValue]) -> Result<String, SqlError> {
        let mut slots: Vec<Slot> = Vec::new();
        for h in targets {
            let key = self.sql_host_key(&h.var)?;
            let structure = self.env.is_group(&key);
            let leaves = self.sql_leaves(&key)?;
            let inds = match &h.indicator {
                Some(i) => Some(self.sql_indicator_keys(i, leaves.len(), structure)?),
                None => None,
            };
            for (i, leaf) in leaves.into_iter().enumerate() {
                slots.push(Slot { key: leaf, indicator: inds.as_ref().and_then(|v| v.get(i).cloned()) });
            }
        }
        if slots.len() != row.len() {
            return Err(SqlError::new(
                code::WRONG_TARGET_COUNT,
                format!("the row has {} columns but INTO names {} items", row.len(), slots.len()),
            ));
        }
        let mut ready = Vec::with_capacity(slots.len());
        for (slot, value) in slots.into_iter().zip(row) {
            ready.push(self.sql_convert(slot, value)?);
        }
        Ok(self.sql_apply(ready))
    }

    /// Assign values that have all been converted and checked; the SQLSTATE
    /// is `01004` when any text was cut.
    fn sql_apply(&mut self, ready: Vec<Ready>) -> String {
        let truncated = ready.iter().any(|r| r.cut);
        for r in ready {
            if let Some(v) = r.value {
                self.env.set(&r.key, v);
            }
            if let Some((k, n)) = r.indicator {
                self.env.set(&k, CobolValue::Numeric(CobolNumeric::integer(n)));
            }
        }
        if truncated { code::TRUNCATED.into() } else { code::SUCCESS.into() }
    }

    /// One value for one item, checked (R10, R11, R15).
    fn sql_convert(&self, slot: Slot, value: &SqlValue) -> Result<Ready, SqlError> {
        let key = slot.key;
        if *value == SqlValue::Null {
            return match slot.indicator {
                Some(ind) => Ok(Ready { key, value: None, indicator: Some((ind, -1)), cut: false }),
                None => Err(SqlError::new(
                    code::NULL_NO_INDICATOR,
                    format!("a NULL value was read into {key}, which has no indicator variable"),
                )),
            };
        }
        let zero_ind = slot.indicator.clone().map(|k| (k, 0));
        // A floating item takes the value as a float.
        if let Some(CobolValue::Float(_)) = self.env.get(&key) {
            let f = match value {
                SqlValue::Int(i) => *i as f64,
                SqlValue::Float(f) => *f,
                SqlValue::Decimal { .. } | SqlValue::Text(_) => value.display().trim().parse().map_err(|_| {
                    SqlError::new(code::INVALID_CHARACTER_VALUE, format!("{} is not a number for {key}", value.display()))
                })?,
                SqlValue::Bool(b) => *b as i64 as f64,
                _ => return Err(SqlError::new(code::INVALID_CHARACTER_VALUE, format!("binary data cannot go into {key}"))),
            };
            return Ok(Ready { key, value: Some(CobolValue::Float(f)), indicator: zero_ind, cut: false });
        }
        let numeric = !self.env.is_alphanumeric_field(&key) && self.env.integer_capacity(&key).is_some();
        if numeric {
            let (m, scale) = match value {
                SqlValue::Int(i) => (*i as i128, 0u8),
                SqlValue::Decimal { mantissa, scale } => (*mantissa, *scale),
                SqlValue::Bool(b) => (*b as i128, 0),
                SqlValue::Float(_) | SqlValue::Text(_) => parse_decimal(&value.display()).ok_or_else(|| {
                    SqlError::new(code::INVALID_CHARACTER_VALUE, format!("'{}' is not a number for {key}", value.display()))
                })?,
                SqlValue::Bytes(_) => {
                    return Err(SqlError::new(code::INVALID_CHARACTER_VALUE, format!("binary data cannot go into {key}")))
                }
                SqlValue::Null => unreachable!(),
            };
            let dp = self.env.decimal_places(&key).unwrap_or(0);
            // Extra decimal places are dropped (truncation toward zero, as a
            // MOVE does); missing ones are zeros.
            let m = if scale > dp {
                m / 10i128.pow((scale - dp) as u32)
            } else {
                m.checked_mul(10i128.pow((dp - scale) as u32)).ok_or_else(|| {
                    SqlError::new(code::OUT_OF_RANGE, format!("{} does not fit {key}", value.display()))
                })?
            };
            let int_digits = (m.unsigned_abs() / 10u128.pow(dp as u32)).to_string();
            let int_len = if int_digits == "0" { 0 } else { int_digits.len() };
            let cap = self.env.integer_capacity(&key).unwrap_or(0) as usize;
            if int_len > cap || (m < 0 && self.env.is_unsigned_numeric(&key)) {
                return Err(SqlError::new(
                    code::OUT_OF_RANGE,
                    format!("{} does not fit {key} (numeric value out of range)", value.display()),
                ));
            }
            return Ok(Ready { key, value: Some(CobolValue::Numeric(CobolNumeric::new(m, dp))), indicator: zero_ind, cut: false });
        }
        // Alphanumeric: the text, cut to the item; the indicator then holds
        // the original length (R11).
        let text = value.display();
        let cap = self.env.alphanumeric_capacity(&key).unwrap_or_else(|| self.env.stored_width(&key).max(1));
        let len = text.chars().count();
        let kept: String = text.chars().take(cap).collect();
        let indicator = slot.indicator.map(|k| (k, if len > cap { len as i64 } else { 0 }));
        Ok(Ready { key, value: Some(CobolValue::from_str(&kept, cap)), indicator, cut: len > cap })
    }

    /// Set every status item the program declares (R17).
    fn sql_set_status(&mut self, d: &Done) {
        if self.sql_status.is_none() {
            let k = |n: &str| self.env.keys_of_leaf(n);
            self.sql_status = Some(StatusKeys {
                sqlstate: k("SQLSTATE"),
                sqlcode: k("SQLCODE"),
                sqlmsg: k("SQLMSG"),
                message_length: k("SQLCA-MESSAGE-LENGTH"),
                rows: k("SQLCA-ROWS"),
                native: k("SQLCA-NATIVE-CODE"),
                warning: k("SQLCA-WARNING"),
                truncated: k("SQLCA-TRUNCATED"),
                connection: k("SQLCA-CONNECTION"),
            });
        }
        let keys = self.sql_status.clone().unwrap_or_default();
        let sqlcode = sqlcode(&d.state);
        let conn = self
            .sql_unit
            .lock()
            .ok()
            .and_then(|u| u.current.and_then(|i| u.sessions.get(i).map(|s| s.name.clone())))
            .unwrap_or_default();
        let warning = outcome(&d.state) == Outcome::Warning;
        for k in &keys.sqlstate {
            self.env.set_str(k, &d.state);
        }
        for k in &keys.sqlcode {
            self.env.set(k, CobolValue::Numeric(CobolNumeric::integer(sqlcode)));
        }
        let mut msg_keys = keys.sqlmsg.clone();
        msg_keys.extend(self.env.keys_of_leaf("SQLCA-MESSAGE"));
        for k in &msg_keys {
            self.env.set_str(k, &d.message);
        }
        for k in &keys.message_length {
            self.env.set(k, CobolValue::Numeric(CobolNumeric::integer(d.message.len().min(512) as i64)));
        }
        for k in &keys.rows {
            self.env.set(k, CobolValue::Numeric(CobolNumeric::integer(d.rows)));
        }
        for k in &keys.native {
            self.env.set(k, CobolValue::Numeric(CobolNumeric::integer(d.native)));
        }
        for k in &keys.warning {
            self.env.set_str(k, if warning { "W" } else { " " });
        }
        for k in &keys.truncated {
            self.env.set_str(k, if d.state == code::TRUNCATED { "W" } else { " " });
        }
        for k in &keys.connection {
            self.env.set_str(k, &conn);
        }
    }

    /// One line on the debugger's SQL channel (R48).
    fn sql_trace(&self, t: &Trace, d: &Done) {
        if self.debug_event_tx.is_none() {
            return;
        }
        let mut line = t.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.values.is_empty() {
            line.push_str(&format!("   values [{}]", t.values.join(", ")));
        }
        line.push_str(&format!("   SQLSTATE {} SQLCODE {} rows {}", d.state, sqlcode(&d.state), d.rows));
        if !d.message.is_empty() {
            line.push_str(&format!("   {}", d.message));
        }
        self.debug_out(crate::debugger::OutputChannel::Sql, line);
    }
}
