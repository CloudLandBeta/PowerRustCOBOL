// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An `IndexedFile` control's verbs on a web form (spec 111).
//!
//! The file lives with the Backend. Every `OPEN`, `CLOSE`, `START`, `READ`,
//! `WRITE`, `REWRITE` and `DELETE` of the form's program on a file an
//! IndexedFile control declares — the developer's own and the control's
//! generated `<id>-OPEN … <id>-CLOSE` paragraphs alike — becomes a
//! `CALL "<endpoint>" WITH WEBSOCKET LINKAGE` to the program the Build
//! generates (`cobolt_codegen::remote_controls::indexed`), in the parsed
//! program only: the developer's source is never touched.
//!
//! The backend keeps nothing between calls (plan O4), so the browser keeps the
//! file's cursor: after a read, the next sequential read goes `GT` (or `LT`)
//! that record; after a `START`, from where it positioned. The record area
//! travels as the call's `RECORD-IMAGE`, its primary key being the key.
//!
//! The verbs' own phrases (`AT END`, `INVALID KEY`, `INTO`, `FROM`) run in the
//! browser, on the status the backend answered. What cannot be served is
//! refused, at the developer's line: a `ROLLBACK` (each call is durable when
//! it answers), a read or `START` on an alternate key, a `START … NOT =`.

use std::collections::HashMap;

use cobolt_ast::expr::{CmpOp, Expr, Literal};
use cobolt_ast::program::{AccessMode, DataSection, Program};
use cobolt_ast::stmt::{OpenMode, ReadDirection, Stmt};
use cobolt_codegen::remote_controls::indexed::{self as ix, frontend_names, frontend_position, frontend_working_storage};
use cobolt_codegen::remote_controls::SiteError;
use cobolt_forms::{Control, Form};
use cobolt_lexer::Span;

use crate::web_remote::{add_working_storage, declared_names, fill, flatten, parse_statements, parse_working_storage};

/// One IndexedFile control whose file the form's program declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedFile {
    /// The control, as the form names it.
    pub control: String,
    /// The `SELECT` name in the program.
    pub file: String,
    /// The 01 record of the file's FD.
    pub record: String,
    /// Where the status goes (upper case), beside the lowering's own item.
    pub status: Vec<String>,
    /// The file's number among the form's served files, from 1.
    pub number: usize,
    /// The `RECORD KEY` data name.
    pub key: String,
    /// The `ALTERNATE RECORD KEY` data names.
    pub alternates: Vec<String>,
    pub access: AccessMode,
}

fn up(s: &str) -> String {
    s.trim().to_ascii_uppercase()
}

/// The stem of a path, `-` for `_`, upper case.
fn stem(path: &str) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let base = base.rsplit_once('.').map_or(base, |(l, _)| l);
    up(&base.replace('_', "-"))
}

/// The served IndexedFile controls of `form`, found in `program` and the
/// programs nested in it (a handler declares its own `SELECT` and `FD`): a
/// control that names a `.cidx` and whose file a program `SELECT`s (a form that
/// declares none uses the control through its PartiQL members, which hold the
/// file themselves). A file is listed once for each program that declares it;
/// its `number` is the control's, from 1, over the form.
pub fn served_files(program: &Program, form: &Form) -> Vec<ServedFile> {
    let mut controls = Vec::new();
    flatten(&form.controls, &mut controls);
    let controls: Vec<&Control> = controls.into_iter().filter(|c| ix::is_served(c)).collect();
    let mut found = Vec::new();
    files_in_tree(program, &controls, &mut found);
    // The numbering: the controls that are served somewhere, in the form's order.
    let numbering: Vec<String> = controls
        .iter()
        .filter(|c| found.iter().any(|f: &ServedFile| f.control.eq_ignore_ascii_case(&c.id)))
        .map(|c| c.id.clone())
        .collect();
    for f in &mut found {
        f.number = numbering.iter().position(|n| n.eq_ignore_ascii_case(&f.control)).map_or(0, |i| i + 1);
    }
    found
}

fn files_in_tree(program: &Program, controls: &[&Control], out: &mut Vec<ServedFile>) {
    out.extend(files_of(program, controls));
    for n in &program.nested_programs {
        files_in_tree(n, controls, out);
    }
}

/// The served files `program` itself declares (`number` still 0).
fn files_of(program: &Program, controls: &[&Control]) -> Vec<ServedFile> {
    let Some(io) = program.environment.as_ref().and_then(|e| e.input_output.as_ref()) else { return Vec::new() };
    let mut out: Vec<ServedFile> = Vec::new();
    for ctrl in controls {
        let name = up(&cobolt_codegen::indexed_file_name(ctrl));
        let Some(fc) = io.file_controls.iter().find(|fc| up(&fc.name) == name || stem(&fc.assign) == name.replace('_', "-")) else {
            continue;
        };
        let Some(record) = record_of(program, &fc.name) else { continue };
        let id = cobolt_codegen::cobol_word(&ctrl.id);
        let mut status = vec![fc.file_status.as_deref().map(up).unwrap_or_default()];
        status.push(up(&prop(ctrl, "StatusDataItem").unwrap_or_else(|| format!("WS-{id}-STATUS"))));
        status.retain(|s| !s.is_empty());
        status.dedup();
        out.push(ServedFile {
            control: ctrl.id.clone(),
            file: fc.name.clone(),
            record,
            status,
            number: 0,
            key: fc.record_key.clone().unwrap_or_default(),
            alternates: fc.alternate_keys.iter().map(|k| k.field.clone()).collect(),
            access: fc.access,
        });
    }
    out
}

fn prop(ctrl: &Control, name: &str) -> Option<String> {
    ctrl.get_prop(name).map(|v| v.as_str().trim().to_owned()).filter(|s| !s.is_empty())
}

/// The first 01 record of `file`'s FD.
fn record_of(program: &Program, file: &str) -> Option<String> {
    let data = program.data.as_ref()?;
    data.sections.iter().find_map(|s| match s {
        DataSection::FileSection(fds) => fds
            .iter()
            .find(|fd| fd.name.eq_ignore_ascii_case(file))
            .and_then(|fd| fd.records.first())
            .and_then(|r| r.name.clone()),
        _ => None,
    })
}

/// Rewrite every verb of `program` on a served file into its call to the
/// backend. Answers how many verbs were lowered (0 leaves the program
/// untouched), or every verb it refuses.
pub fn lower_indexed_files(program: &mut Program, form: &Form) -> Result<usize, Vec<SiteError>> {
    let files = served_files(program, form);
    if files.is_empty() {
        return Ok(0);
    }
    let count = files.iter().map(|f| f.number).max().unwrap_or(0);
    let mut errs = Vec::new();
    // A developer's item named like one of the lowering's would be ambiguous.
    let reserved = frontend_names(count);
    let mut taken = Vec::new();
    declared_names(program, &mut taken);
    for (name, line) in taken {
        let u = up(&name);
        if reserved.iter().any(|r| *r == u) {
            errs.push(SiteError {
                line,
                message: format!("{}: the item {name} takes a name the web build needs for an IndexedFile's calls; rename it", form.name),
            });
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut controls = Vec::new();
    flatten(&form.controls, &mut controls);
    let controls: Vec<&Control> = controls.into_iter().filter(|c| ix::is_served(c)).collect();
    let numbering: Vec<ServedFile> = files.iter().fold(Vec::new(), |mut acc, f| {
        if !acc.iter().any(|a: &ServedFile| a.number == f.number) {
            acc.push(f.clone());
        }
        acc
    });
    let mut cx = Cx { form: &form.name, files: Vec::new(), globals: std::collections::HashSet::new(), controls: &controls, endpoints: HashMap::new(), errs: Vec::new(), done: 0 };
    for f in &numbering {
        for op in ix::SERVED {
            match cobolt_codegen::remote_controls::endpoint(form, &f.control, op) {
                Ok(e) => {
                    cx.endpoints.insert((f.number, *op), e);
                }
                Err(m) => cx.errs.push(SiteError { line: 0, message: format!("{}: IndexedFile {} — {m}", form.name, f.control) }),
            }
        }
    }
    if !cx.errs.is_empty() {
        return Err(cx.errs);
    }
    lower_program(program, &mut cx, &numbering_of(&files));
    if !cx.errs.is_empty() {
        return Err(cx.errs);
    }
    let done = cx.done;
    let decls = parse_working_storage(&frontend_working_storage(count))
        .map_err(|m| vec![SiteError { line: 0, message: format!("{}: {m}", form.name) }])?;
    add_working_storage(program, decls);
    Ok(done)
}

/// Control id → number, for the programs lowered below.
fn numbering_of(files: &[ServedFile]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for f in files {
        if !out.iter().any(|(c, _)| c.eq_ignore_ascii_case(&f.control)) {
            out.push((f.control.clone(), f.number));
        }
    }
    out
}

struct Cx<'a> {
    form: &'a str,
    /// The files of the program being lowered, those of the programs it is
    /// nested in included (their FDs are `GLOBAL`).
    files: Vec<ServedFile>,
    controls: &'a [&'a Control],
    /// The `GLOBAL` data names of the programs enclosing the one lowered: the
    /// status the control would write is written only where the program can see
    /// the item.
    globals: std::collections::HashSet<String>,
    endpoints: HashMap<(usize, &'static str), String>,
    errs: Vec<SiteError>,
    done: usize,
}

impl Cx<'_> {
    fn file(&self, name: &str) -> Option<&ServedFile> {
        self.files.iter().find(|f| f.file.eq_ignore_ascii_case(name.trim()))
    }

    fn file_of_record(&self, record: &str) -> Option<&ServedFile> {
        self.files.iter().find(|f| f.record.eq_ignore_ascii_case(name_of(record).trim()))
    }

    fn refuse(&mut self, line: u32, what: &str, why: &str) {
        self.errs.push(SiteError { line, message: format!("{}: {what} — {why}", self.form) });
    }

    fn endpoint(&self, f: &ServedFile, op: &'static str) -> String {
        self.endpoints.get(&(f.number, op)).cloned().unwrap_or_default()
    }
}

fn name_of(record: &str) -> &str {
    record
}

fn lower_program(p: &mut Program, cx: &mut Cx, numbering: &[(String, usize)]) {
    use cobolt_ast::program::ProcedureBody;
    let outer = cx.files.clone();
    let outer_globals = cx.globals.clone();
    let (own_names, own_globals) = data_names(p);
    let visible: std::collections::HashSet<String> = cx.globals.iter().cloned().chain(own_names).collect();
    cx.globals.extend(own_globals);
    let mut own = files_of(p, cx.controls);
    for f in &mut own {
        f.number = numbering.iter().find(|(c, _)| c.eq_ignore_ascii_case(&f.control)).map_or(0, |(_, n)| *n);
        f.status.retain(|s| visible.contains(s));
    }
    // What this program declares shadows what an enclosing one does.
    cx.files.retain(|f| !own.iter().any(|o| o.file.eq_ignore_ascii_case(&f.file)));
    cx.files.extend(own);
    for d in &mut p.procedure.declaratives {
        lower_list(&mut d.stmts, cx, "");
        for para in &mut d.paras {
            lower_list(&mut para.stmts, cx, &para.name);
        }
    }
    match &mut p.procedure.body {
        ProcedureBody::Sections(secs) => {
            for s in secs {
                for para in &mut s.paragraphs {
                    lower_list(&mut para.stmts, cx, &para.name);
                }
            }
        }
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                lower_list(&mut para.stmts, cx, &para.name);
            }
        }
    }
    for n in &mut p.nested_programs {
        lower_program(n, cx, numbering);
    }
    cx.files = outer;
    cx.globals = outer_globals;
}

/// The data names `p` declares itself (not its nested programs), and those of
/// them its nested programs can see.
fn data_names(p: &Program) -> (Vec<String>, Vec<String>) {
    fn walk(d: &[cobolt_ast::data::DataDecl], all: &mut Vec<String>, globals: &mut Vec<String>) {
        for x in d {
            if let Some(n) = &x.name {
                all.push(up(n));
                if x.is_global {
                    globals.push(up(n));
                }
            }
            walk(&x.children, all, globals);
        }
    }
    let (mut all, mut globals) = (Vec::new(), Vec::new());
    if let Some(data) = &p.data {
        for s in &data.sections {
            match s {
                DataSection::WorkingStorage(v) | DataSection::LocalStorage(v) | DataSection::Linkage(v) => walk(v, &mut all, &mut globals),
                DataSection::FileSection(fds) => fds.iter().for_each(|fd| walk(&fd.records, &mut all, &mut globals)),
                DataSection::Screen(_) => {}
            }
        }
    }
    (all, globals)
}

fn lower_list(list: &mut Vec<Stmt>, cx: &mut Cx, para: &str) {
    let old = std::mem::take(list);
    for mut s in old {
        for inner in s.child_lists_mut() {
            lower_list(inner, cx, para);
        }
        match lowered(&s, cx, para) {
            Lowered::Keep => list.push(s),
            Lowered::Replace(stmts) => list.extend(stmts),
            Lowered::Refused => list.push(s),
        }
    }
}

enum Lowered {
    Keep,
    Replace(Vec<Stmt>),
    Refused,
}

/// The status of the facade's own `COBOL::"FILE-STATUS" ( "file" item )` call
/// and the `COMMIT`/`ROLLBACK` verbs are not the browser's to run.
fn lowered(s: &Stmt, cx: &mut Cx, para: &str) -> Lowered {
    match s {
        Stmt::Open { mode, files, extra_modes, span, sharing, lock, registered_user, .. } => {
            let mut groups = vec![(*mode, files.clone())];
            groups.extend(extra_modes.iter().cloned());
            if !groups.iter().flat_map(|(_, f)| f).any(|f| cx.file(f).is_some()) {
                return Lowered::Keep;
            }
            let mut out = Vec::new();
            let mut rest: Vec<(OpenMode, Vec<String>)> = Vec::new();
            for (m, fs) in groups {
                let (mine, other): (Vec<String>, Vec<String>) = fs.into_iter().partition(|f| cx.file(f).is_some());
                for f in mine {
                    let served = cx.file(&f).cloned().expect("partitioned");
                    let word = match m {
                        OpenMode::Input => "INPUT",
                        OpenMode::Output => "OUTPUT",
                        OpenMode::InputOutput => "I-O",
                        OpenMode::Extend => "EXTEND",
                    };
                    match open_statements(cx, &served, word, *span) {
                        Ok(mut v) => out.append(&mut v),
                        Err(e) => cx.refuse(span.line, &format!("OPEN {f}"), &e),
                    }
                    cx.done += 1;
                }
                if !other.is_empty() {
                    rest.push((m, other));
                }
            }
            if let Some((m, f)) = rest.first().cloned() {
                out.push(Stmt::Open {
                    mode: m,
                    files: f,
                    sharing: *sharing,
                    lock: *lock,
                    registered_user: registered_user.clone(),
                    span: *span,
                    extra_modes: rest[1..].to_vec(),
                });
            }
            Lowered::Replace(out)
        }
        Stmt::Close { files, span, .. } => {
            if !files.iter().any(|f| cx.file(f).is_some()) {
                return Lowered::Keep;
            }
            let mut out = Vec::new();
            let mut other = Vec::new();
            for f in files {
                match cx.file(f).cloned() {
                    Some(served) => {
                        match close_statements(cx, &served, *span) {
                            Ok(mut v) => out.append(&mut v),
                            Err(e) => cx.refuse(span.line, &format!("CLOSE {f}"), &e),
                        }
                        cx.done += 1;
                    }
                    None => other.push(f.clone()),
                }
            }
            if !other.is_empty() {
                if let Stmt::Close { locked, reel, span, .. } = s {
                    out.push(Stmt::Close { files: other, locked: locked.clone(), span: *span, reel: reel.clone() });
                }
            }
            Lowered::Replace(out)
        }
        Stmt::Read { file, into, key, direction, at_end, not_at_end, invalid_key, not_invalid_key, span, .. } => {
            let Some(f) = cx.file(file).cloned() else { return Lowered::Keep };
            if let Some(Expr::Identifier(k, _)) = key {
                if f.alternates.iter().any(|a| a.eq_ignore_ascii_case(k.trim())) {
                    cx.refuse(span.line, &format!("READ {file} KEY IS {k}"), "the backend serves a file on its primary key; an alternate key cannot be read through it");
                    return Lowered::Refused;
                }
            }
            let sequential = match direction {
                ReadDirection::Next | ReadDirection::Previous => true,
                ReadDirection::Default => {
                    key.is_none()
                        && invalid_key.is_empty()
                        && not_invalid_key.is_empty()
                        && (f.access == AccessMode::Sequential || !at_end.is_empty() || !not_at_end.is_empty())
                }
            };
            let result = if sequential {
                let previous = matches!(direction, ReadDirection::Previous);
                read_sequential(cx, &f, previous, into.as_ref(), at_end, not_at_end, *span)
            } else {
                read_random(cx, &f, into.as_ref(), invalid_key, not_invalid_key, *span)
            };
            finish(cx, result, span.line, &format!("READ {file}"))
        }
        Stmt::Write { record, from, invalid_key, not_invalid_key, span, .. } => {
            let Expr::Identifier(r, _) = record else { return Lowered::Keep };
            let Some(f) = cx.file_of_record(r).cloned() else { return Lowered::Keep };
            let result = change(cx, &f, "WRITE", from.as_ref(), invalid_key, not_invalid_key, *span);
            finish(cx, result, span.line, &format!("WRITE {r}"))
        }
        Stmt::Rewrite { record, from, invalid_key, not_invalid_key, span } => {
            let Expr::Identifier(r, _) = record else { return Lowered::Keep };
            let Some(f) = cx.file_of_record(r).cloned() else { return Lowered::Keep };
            let result = change(cx, &f, "REWRITE", from.as_ref(), invalid_key, not_invalid_key, *span);
            finish(cx, result, span.line, &format!("REWRITE {r}"))
        }
        Stmt::Delete { file, invalid_key, not_invalid_key, span } => {
            let Some(f) = cx.file(file).cloned() else { return Lowered::Keep };
            let result = change(cx, &f, "DELETE", None, invalid_key, not_invalid_key, *span);
            finish(cx, result, span.line, &format!("DELETE {file}"))
        }
        Stmt::Start { file, key, invalid_key, not_invalid_key, span } => {
            let Some(f) = cx.file(file).cloned() else { return Lowered::Keep };
            let result = start(cx, &f, key.as_ref(), invalid_key, not_invalid_key, *span);
            finish(cx, result, span.line, &format!("START {file}"))
        }
        Stmt::Commit { span } => {
            // Each call is durable when it answers: the COMMIT has nothing left to do.
            let moves: String = cx.files.iter().map(|f| status_moves(f, "\"00\"")).collect();
            if moves.is_empty() {
                return Lowered::Replace(vec![Stmt::Continue { span: *span }]);
            }
            match parse_statements(&moves) {
                Ok(stmts) => {
                    cx.done += 1;
                    Lowered::Replace(stmts.into_iter().map(|s| respan(s, *span)).collect())
                }
                Err(e) => {
                    cx.refuse(span.line, "COMMIT", &e);
                    Lowered::Refused
                }
            }
        }
        Stmt::Rollback { span } => {
            // The control's own `<id>-ROLLBACK` paragraph answers "not supported"
            // (94); a ROLLBACK anywhere else would be silent about it.
            let own = cx.files.iter().find(|f| para.eq_ignore_ascii_case(&format!("{}-ROLLBACK", cobolt_codegen::cobol_word(&f.control)))).cloned();
            match own {
                Some(f) => {
                    let moves: String = status_moves(&f, "\"94\"");
                    match parse_statements(&moves) {
                        Ok(stmts) => Lowered::Replace(stmts.into_iter().map(|s| respan(s, *span)).collect()),
                        Err(e) => {
                            cx.refuse(span.line, "ROLLBACK", &e);
                            Lowered::Refused
                        }
                    }
                }
                None => {
                    cx.refuse(
                        span.line,
                        "ROLLBACK",
                        "each call to the backend is durable when it answers, so there is nothing to undo; write the changes you may want undone to a work file, or leave them out",
                    );
                    Lowered::Refused
                }
            }
        }
        Stmt::Call { program: Expr::Literal(Literal::String(p), _), using, .. }
            if matches!(p.to_ascii_uppercase().as_str(), "COBOL-FILE-STATUS" | "FILE-STATUS") =>
        {
            let named = using.first().map(|a| format!("{a:?}")).unwrap_or_default();
            if cx.files.iter().any(|f| named.to_ascii_uppercase().contains(&format!("\"{}\"", f.file.to_ascii_uppercase()))) {
                Lowered::Replace(Vec::new())
            } else {
                Lowered::Keep
            }
        }
        _ => Lowered::Keep,
    }
}

fn finish(cx: &mut Cx, result: Result<Vec<Stmt>, String>, line: u32, what: &str) -> Lowered {
    match result {
        Ok(v) => {
            cx.done += 1;
            Lowered::Replace(v)
        }
        Err(e) => {
            cx.refuse(line, what, &e);
            Lowered::Refused
        }
    }
}

fn respan(s: Stmt, at: Span) -> Stmt {
    let mut tree = match serde_json::to_value(&s) {
        Ok(t) => t,
        Err(_) => return s,
    };
    fill(&mut tree, &[], at);
    serde_json::from_value(tree).unwrap_or(s)
}

fn status_moves(f: &ServedFile, value: &str) -> String {
    f.status.iter().map(|s| format!("           MOVE {value} TO {s}\n")).collect()
}

/// `CALL endpoint … ON EXCEPTION … NOT ON EXCEPTION <ok>`: the failure leaves
/// status `30` and the transport's reason where `COBOL::"WS-LAST-ERROR"` reads
/// it.
fn exchange(f: &ServedFile, endpoint: &str, before: &str, ok: &str) -> String {
    use ix::{FRONTEND_ERR_CLASS as C, FRONTEND_ERR_CODE as D, FRONTEND_ERR_MESSAGE as M, FRONTEND_PARAMS as P, FRONTEND_RESULT as R, FRONTEND_STATUS as S};
    format!(
        "{before}           CALL \"{endpoint}\" WITH WEBSOCKET LINKAGE
               USING BY VALUE {P} RETURNING {R}
               ON EXCEPTION
                   MOVE \"30\" TO {S}
{fail}                   CALL \"COBOL-WS-LAST-ERROR\" USING {C} {D} {M} END-CALL
               NOT ON EXCEPTION
                   MOVE FILE-STATUS OF {R} TO {S}
{moves}{ok}           END-CALL\n",
        fail = status_moves(f, "\"30\"").replace("           MOVE", "                   MOVE"),
        moves = status_moves(f, S).replace("           MOVE", "                   MOVE"),
    )
}

const IMAGE: &str = "RECORD-IMAGE OF RC-IX-PARAMS";
const MODE: &str = "KEY-MODE OF RC-IX-PARAMS";

fn slot(name: &str) -> String {
    format!("                   DISPLAY \"RC-SLOT-{name}\"\n")
}

/// Parse `src`, fill the placeholders (`RC-ARG-k`) with `args`, then splice the
/// developer's own phrases in place of the `RC-SLOT-*` markers.
fn build(src: &str, args: &[&Expr], slots: &[(&str, &[Stmt])], at: Span) -> Result<Vec<Stmt>, String> {
    let stmts = parse_statements(src)?;
    let mut tree = serde_json::to_value(&stmts).map_err(|e| e.to_string())?;
    fill(&mut tree, args, at);
    let mut out = serde_json::from_value::<Vec<Stmt>>(tree).map_err(|e| e.to_string())?;
    splice(&mut out, slots);
    Ok(out)
}

fn splice(list: &mut Vec<Stmt>, slots: &[(&str, &[Stmt])]) {
    let old = std::mem::take(list);
    for mut s in old {
        if let Stmt::Display { operands, .. } = &s {
            if let [Expr::Literal(Literal::String(t), _)] = operands.as_slice() {
                if let Some(name) = t.strip_prefix("RC-SLOT-") {
                    match slots.iter().find(|(n, _)| *n == name) {
                        Some((_, body)) if !body.is_empty() => list.extend(body.iter().cloned()),
                        _ => list.push(Stmt::Continue { span: s.span() }),
                    }
                    continue;
                }
            }
        }
        for inner in s.child_lists_mut() {
            splice(inner, slots);
        }
        list.push(s);
    }
}

fn open_statements(cx: &Cx, f: &ServedFile, mode: &str, at: Span) -> Result<Vec<Stmt>, String> {
    let (pos_mode, _) = frontend_position(f.number);
    let ok = format!("                   IF {s}(1:1) = \"0\"\n                       MOVE SPACES TO {pos_mode}\n                   END-IF\n", s = ix::FRONTEND_STATUS);
    let src = exchange(f, &cx.endpoint(f, "OPEN"), &format!("           MOVE \"{mode}\" TO {MODE}\n           MOVE SPACES TO {IMAGE}\n"), &ok);
    build(&src, &[], &[], at)
}

fn close_statements(cx: &Cx, f: &ServedFile, at: Span) -> Result<Vec<Stmt>, String> {
    let (pos_mode, _) = frontend_position(f.number);
    let ok = format!("                   MOVE SPACES TO {pos_mode}\n");
    let src = exchange(f, &cx.endpoint(f, "CLOSE"), &format!("           MOVE SPACES TO {MODE}\n           MOVE SPACES TO {IMAGE}\n"), &ok);
    build(&src, &[], &[], at)
}

fn read_sequential(
    cx: &Cx,
    f: &ServedFile,
    previous: bool,
    into: Option<&Expr>,
    at_end: &[Stmt],
    not_at_end: &[Stmt],
    at: Span,
) -> Result<Vec<Stmt>, String> {
    let (pos_mode, pos_image) = frontend_position(f.number);
    let (op, next_mode) = if previous { ("READ-PREVIOUS", "LT") } else { ("READ-NEXT", "GT") };
    let into_move = if into.is_some() { format!("                       MOVE {} TO RC-ARG-1\n", f.record) } else { String::new() };
    let ok = format!(
        "                   IF PAST-END OF RC-IX-RESULT = 1
{end}                   ELSE
                       IF {s}(1:1) = \"0\"
                           MOVE RESULT-IMAGE OF RC-IX-RESULT TO {record}
{into_move}                           MOVE \"{next_mode}\" TO {pos_mode}
                           MOVE RESULT-IMAGE OF RC-IX-RESULT TO {pos_image}
{not_end}                       END-IF
                   END-IF\n",
        end = slot("AT-END"),
        not_end = slot("NOT-AT-END").replace("                   DISPLAY", "                           DISPLAY"),
        s = ix::FRONTEND_STATUS,
        record = f.record,
    );
    let before = format!("           MOVE {pos_mode} TO {MODE}\n           MOVE {pos_image} TO {IMAGE}\n");
    let src = exchange(f, &cx.endpoint(f, op), &before, &ok);
    let args: Vec<&Expr> = into.into_iter().collect();
    build(&src, &args, &[("AT-END", at_end), ("NOT-AT-END", not_at_end)], at)
}

fn read_random(cx: &Cx, f: &ServedFile, into: Option<&Expr>, invalid_key: &[Stmt], not_invalid_key: &[Stmt], at: Span) -> Result<Vec<Stmt>, String> {
    let (pos_mode, pos_image) = frontend_position(f.number);
    let into_move = if into.is_some() { format!("                       MOVE {} TO RC-ARG-1\n", f.record) } else { String::new() };
    let ok = format!(
        "                   IF {s}(1:1) = \"0\"
                       MOVE RESULT-IMAGE OF RC-IX-RESULT TO {record}
{into_move}                       MOVE \"GT\" TO {pos_mode}
                       MOVE RESULT-IMAGE OF RC-IX-RESULT TO {pos_image}
{ok_slot}                   ELSE
                       IF {s}(1:1) = \"2\"
{bad_slot}                       END-IF
                   END-IF\n",
        ok_slot = slot("NOT-INVALID-KEY").replace("                   DISPLAY", "                       DISPLAY"),
        bad_slot = slot("INVALID-KEY").replace("                   DISPLAY", "                           DISPLAY"),
        s = ix::FRONTEND_STATUS,
        record = f.record,
    );
    let before = format!("           MOVE SPACES TO {MODE}\n           MOVE {} TO {IMAGE}\n", f.record);
    let src = exchange(f, &cx.endpoint(f, "READ-INVALID"), &before, &ok);
    let args: Vec<&Expr> = into.into_iter().collect();
    build(&src, &args, &[("INVALID-KEY", invalid_key), ("NOT-INVALID-KEY", not_invalid_key)], at)
}

fn change(
    cx: &Cx,
    f: &ServedFile,
    verb: &'static str,
    from: Option<&Expr>,
    invalid_key: &[Stmt],
    not_invalid_key: &[Stmt],
    at: Span,
) -> Result<Vec<Stmt>, String> {
    let ok = format!(
        "                   IF {s}(1:1) = \"0\"
{ok_slot}                   ELSE
                       IF {s}(1:1) = \"2\"
{bad_slot}                       END-IF
                   END-IF\n",
        ok_slot = slot("NOT-INVALID-KEY").replace("                   DISPLAY", "                       DISPLAY"),
        bad_slot = slot("INVALID-KEY").replace("                   DISPLAY", "                           DISPLAY"),
        s = ix::FRONTEND_STATUS,
    );
    let moved = if from.is_some() { format!("           MOVE RC-ARG-1 TO {}\n", f.record) } else { String::new() };
    let before = format!("{moved}           MOVE SPACES TO {MODE}\n           MOVE {} TO {IMAGE}\n", f.record);
    let src = exchange(f, &cx.endpoint(f, verb), &before, &ok);
    let args: Vec<&Expr> = from.into_iter().collect();
    build(&src, &args, &[("INVALID-KEY", invalid_key), ("NOT-INVALID-KEY", not_invalid_key)], at)
}

fn start(cx: &Cx, f: &ServedFile, key: Option<&(CmpOp, Expr)>, invalid_key: &[Stmt], not_invalid_key: &[Stmt], at: Span) -> Result<Vec<Stmt>, String> {
    let (pos_mode, pos_image) = frontend_position(f.number);
    let (word, key_move) = match key {
        None => ("EQ", String::new()),
        Some((op, expr)) => {
            let word = match op {
                CmpOp::Eq => "EQ",
                CmpOp::Gt => "GT",
                CmpOp::Ge => "GE",
                CmpOp::Lt => "LT",
                CmpOp::Le => "LE",
                CmpOp::Ne => return Err("the backend positions on =, >, >=, < and <=, not on NOT =".into()),
            };
            let mut mv = String::new();
            if let Expr::Identifier(k, _) = expr {
                if f.alternates.iter().any(|a| a.eq_ignore_ascii_case(k.trim())) {
                    return Err(format!("{k} is an alternate key; the backend serves a file on its primary key"));
                }
                if !k.eq_ignore_ascii_case(&f.key) && !f.key.is_empty() {
                    mv = format!("           MOVE RC-ARG-1 TO {}\n", f.key);
                }
            } else if !f.key.is_empty() {
                mv = format!("           MOVE RC-ARG-1 TO {}\n", f.key);
            }
            (word, mv)
        }
    };
    let ok = format!(
        "                   IF {s}(1:1) = \"0\"
                       MOVE {MODE} TO {pos_mode}
                       MOVE {IMAGE} TO {pos_image}
{ok_slot}                   ELSE
                       IF {s}(1:1) = \"2\"
{bad_slot}                       END-IF
                   END-IF\n",
        ok_slot = slot("NOT-INVALID-KEY").replace("                   DISPLAY", "                       DISPLAY"),
        bad_slot = slot("INVALID-KEY").replace("                   DISPLAY", "                           DISPLAY"),
        s = ix::FRONTEND_STATUS,
    );
    let before = format!("{key_move}           MOVE \"{word}\" TO {MODE}\n           MOVE {} TO {IMAGE}\n", f.record);
    let src = exchange(f, &cx.endpoint(f, "START"), &before, &ok);
    let args: Vec<&Expr> = key.map(|(_, e)| e).into_iter().collect();
    build(&src, &args, &[("INVALID-KEY", invalid_key), ("NOT-INVALID-KEY", not_invalid_key)], at)
}
