// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Check for embedded SQL (spec 087 R9, R21, R28, R46).
//!
//! Without contacting any database, every `EXEC SQL` statement is checked for
//! what COBOL can know about it:
//!
//! * every host variable and indicator names exactly one data item the
//!   program can see — its own, or a GLOBAL one of a program around it;
//! * an indicator is a `PIC S9(4)` binary or display item (a table of them
//!   for a host structure);
//! * a cursor is declared before, in the source, the statements that use it,
//!   and only once;
//! * a prepared statement name is prepared before, in the source, it is
//!   executed or described (a `DECLARE … CURSOR FOR name` is not a use);
//! * a `WHENEVER … GO TO` target is a paragraph or section of the program.
//!
//! Unterminated blocks and missing INCLUDEs are the lexer's and the
//! preprocessor's to report, misplaced blocks and R14 the parser's; nothing is
//! reported twice.

use cobolt_ast::data::{DataDecl, Usage};
use cobolt_ast::program::{DataSection, ProcedureBody, Program};
use cobolt_ast::sql::*;
use cobolt_ast::stmt::Stmt;
use cobolt_ast::Span;

use crate::{SemanticDiagnostic, Severity};

/// One declared data item, as Check needs it.
#[derive(Debug, Clone)]
struct Decl {
    name: String,
    /// The names of the groups above it, innermost first.
    ancestors: Vec<String>,
    is_group: bool,
    /// The item or an ancestor has OCCURS.
    in_table: bool,
    occurs: bool,
    /// For an indicator: `PIC S9(n)` with n ≤ 4, no decimals, in a usage that
    /// holds a small signed integer.
    indicator_shape: bool,
    global: bool,
}

fn collect(items: &[DataDecl], ancestors: &[String], in_table: bool, global: bool, out: &mut Vec<Decl>) {
    for d in items {
        if d.level == 88 {
            continue;
        }
        let global = global || d.is_global;
        let occurs = d.occurs.is_some();
        let indicator_shape = d.picture.as_ref().is_some_and(|p| {
            p.template.trim_start().to_ascii_uppercase().starts_with('S')
                && p.digits >= 1
                && p.digits <= 4
                && p.decimals == 0
        }) && matches!(d.usage, Usage::Display | Usage::Binary | Usage::Comp | Usage::Comp5);
        if let Some(n) = &d.name {
            out.push(Decl {
                name: n.to_ascii_uppercase(),
                ancestors: ancestors.to_vec(),
                is_group: !d.children.is_empty(),
                in_table: in_table || occurs,
                occurs,
                indicator_shape,
                global,
            });
        }
        if !d.children.is_empty() {
            let mut anc = Vec::with_capacity(ancestors.len() + 1);
            if let Some(n) = &d.name {
                anc.push(n.to_ascii_uppercase());
            }
            anc.extend(ancestors.iter().cloned());
            collect(&d.children, &anc, in_table || occurs, global, out);
        }
    }
}

fn program_decls(p: &Program) -> Vec<Decl> {
    let mut out = Vec::new();
    if let Some(dd) = &p.data {
        for s in &dd.sections {
            match s {
                DataSection::WorkingStorage(v) | DataSection::LocalStorage(v) | DataSection::Linkage(v) => {
                    collect(v, &[], false, false, &mut out)
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        collect(&fd.records, &[], false, fd.is_global, &mut out)
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
    }
    out
}

/// The items `name` OF `quals` matches: the qualifiers must appear among its
/// ancestors, in order.
fn matches<'a>(decls: &'a [Decl], name: &str, quals: &[String]) -> Vec<&'a Decl> {
    decls
        .iter()
        .filter(|d| {
            if d.name != name {
                return false;
            }
            let mut anc = d.ancestors.iter();
            quals.iter().all(|q| anc.any(|a| a == q))
        })
        .collect()
}

enum Resolved<'a> {
    One(&'a Decl),
    Missing,
    Ambiguous,
}

fn resolve<'a>(decls: &'a [Decl], h: &SqlHostName) -> Resolved<'a> {
    let first = matches(decls, &h.name, &h.quals);
    let second = h.swapped.as_ref().map(|(n, q)| matches(decls, n, q)).unwrap_or_default();
    match (first.len(), second.len()) {
        (1, 0) => Resolved::One(first[0]),
        (0, 1) => Resolved::One(second[0]),
        (0, 0) => Resolved::Missing,
        _ => Resolved::Ambiguous,
    }
}

fn written(h: &SqlHostName) -> String {
    let mut s = format!(":{}", h.name);
    for q in &h.quals {
        s.push_str(" OF ");
        s.push_str(q);
    }
    s
}

struct Ctx<'a> {
    diags: &'a mut Vec<SemanticDiagnostic>,
    /// Every PREPARE in the unit: (statement name, line).
    prepares: Vec<(String, u32)>,
}

impl Ctx<'_> {
    fn error(&mut self, line: u32, message: String) {
        let mut span = Span::dummy();
        span.line = line;
        self.diags.push(SemanticDiagnostic { severity: Severity::Error, message, span });
    }
}

/// Check every `EXEC SQL` statement in the compilation unit.
pub fn check(program: &Program, diags: &mut Vec<SemanticDiagnostic>) {
    if !uses_sql(program) {
        return;
    }
    let mut ctx = Ctx { diags, prepares: Vec::new() };
    gather_prepares(program, &mut ctx.prepares);
    check_program(program, &[], &[], &mut ctx);
}

fn uses_sql(p: &Program) -> bool {
    !p.sql_cursors.is_empty() || !program_stmts(p).is_empty() || p.nested_programs.iter().any(uses_sql)
}

/// The program's own `EXEC SQL` statements, in source order.
fn program_stmts(p: &Program) -> Vec<ExecSql> {
    let mut out = Vec::new();
    let paragraphs: Vec<&cobolt_ast::program::Paragraph> = match &p.procedure.body {
        ProcedureBody::Sections(ss) => ss.iter().flat_map(|s| s.paragraphs.iter()).collect(),
        ProcedureBody::Paragraphs(ps) => ps.iter().collect(),
    };
    for pg in paragraphs {
        for s in &pg.stmts {
            s.walk(&mut |st| {
                if let Stmt::ExecSql(e) = st {
                    out.push((**e).clone());
                }
            });
        }
    }
    out
}

fn gather_prepares(p: &Program, out: &mut Vec<(String, u32)>) {
    for e in program_stmts(p) {
        if let SqlKind::Prepare { name, .. } = &e.kind {
            out.push((name.clone(), e.span.line));
        }
    }
    for n in &p.nested_programs {
        gather_prepares(n, out);
    }
}

fn check_program(p: &Program, inherited: &[Decl], outer_cursors: &[SqlCursor], ctx: &mut Ctx) {
    // What this program can see: its own items, then ancestors' GLOBAL ones
    // that it does not shadow.
    let own = program_decls(p);
    let mut visible = own.clone();
    for d in inherited {
        if !own.iter().any(|o| o.name == d.name) {
            visible.push(d.clone());
        }
    }

    // Cursors: declared once, before their uses.
    let mut cursors: Vec<&SqlCursor> = outer_cursors.iter().collect();
    for (i, c) in p.sql_cursors.iter().enumerate() {
        if p.sql_cursors[..i].iter().any(|o| o.name == c.name) || outer_cursors.iter().any(|o| o.name == c.name) {
            ctx.error(c.line, format!("cursor {} is declared twice", c.name));
        }
        cursors.push(c);
        if let SqlCursorQuery::Static(t) = &c.query {
            for h in &t.inputs {
                check_host(h, &visible, false, ctx);
            }
        }
    }
    let use_cursor = |ctx: &mut Ctx, name: &str, line: u32| {
        let declared_before = cursors.iter().any(|c| c.name == name && (c.in_data_division || c.line <= line));
        if !declared_before {
            ctx.error(line, format!("cursor {name} is used before it is declared (DECLARE {name} CURSOR …)"));
        }
    };

    // Paragraphs and sections, for WHENEVER targets.
    let mut labels: Vec<String> = Vec::new();
    match &p.procedure.body {
        ProcedureBody::Sections(ss) => {
            for s in ss {
                labels.push(s.name.to_ascii_uppercase());
                labels.extend(s.paragraphs.iter().map(|pg| pg.name.to_ascii_uppercase()));
            }
        }
        ProcedureBody::Paragraphs(ps) => labels.extend(ps.iter().map(|pg| pg.name.to_ascii_uppercase())),
    }

    let mut reported_targets: Vec<String> = Vec::new();
    for e in &program_stmts(p) {
        let line = e.span.line;
        // A WHENEVER declaration carries just the target it sets (the parser
        // records it on its own statement); a missing target is reported once,
        // on that line.
        if matches!(e.kind, SqlKind::Declarative) {
            for t in [&e.whenever.sqlerror, &e.whenever.sqlwarning, &e.whenever.not_found].into_iter().flatten() {
                let t = t.to_ascii_uppercase();
                if !labels.contains(&t) && !reported_targets.contains(&t) {
                    reported_targets.push(t.clone());
                    ctx.error(line, format!("WHENEVER … GO TO {t}: there is no paragraph or section {t} in this program"));
                }
            }
        }
        match &e.kind {
            SqlKind::Declarative | SqlKind::Commit | SqlKind::Rollback => {}
            SqlKind::Execute(t) => {
                check_text(t, &visible, ctx);
                for part in &t.parts {
                    if let SqlPart::CurrentOf(c) = part {
                        use_cursor(ctx, c, line);
                    }
                }
            }
            SqlKind::SelectInto { text, into } => {
                check_text(text, &visible, ctx);
                for h in into {
                    check_host(h, &visible, true, ctx);
                }
            }
            SqlKind::Open { cursor, using } => {
                use_cursor(ctx, cursor, line);
                check_using(using, &visible, ctx);
            }
            SqlKind::Fetch { cursor, into } => {
                use_cursor(ctx, cursor, line);
                match into {
                    SqlInto::Hosts(hs) => {
                        for h in hs {
                            check_host(h, &visible, true, ctx);
                        }
                    }
                    SqlInto::Descriptor(d) => check_descriptor(d, line, &visible, ctx),
                }
            }
            SqlKind::Close { cursor } => use_cursor(ctx, cursor, line),
            SqlKind::Connect { target, user, password, .. } => {
                for v in [Some(target), user.as_ref(), password.as_ref()].into_iter().flatten() {
                    if let SqlValue::Host(h) = v {
                        check_host(h, &visible, false, ctx);
                    }
                }
            }
            SqlKind::SetConnection(v) | SqlKind::ExecuteImmediate(v) | SqlKind::Disconnect(SqlDisconnect::Named(v)) => {
                if let SqlValue::Host(h) = v {
                    check_host(h, &visible, false, ctx);
                }
            }
            SqlKind::Disconnect(_) => {}
            SqlKind::Prepare { from, .. } => {
                if let SqlValue::Host(h) = from {
                    check_host(h, &visible, false, ctx);
                }
            }
            SqlKind::ExecutePrepared { name, using } => {
                use_statement(ctx, name, line, "EXECUTE");
                check_using(using, &visible, ctx);
            }
            SqlKind::Describe { name, sqlda, .. } => {
                use_statement(ctx, name, line, "DESCRIBE");
                check_descriptor(sqlda, line, &visible, ctx);
            }
        }
    }

    // What the programs inside see: this program's GLOBAL items, then what it
    // inherited; and the cursors of its DATA DIVISION (R28).
    if !p.nested_programs.is_empty() {
        let mut pass: Vec<Decl> = own.iter().filter(|d| d.global).cloned().collect();
        for d in inherited {
            if !pass.iter().any(|o| o.name == d.name) {
                pass.push(d.clone());
            }
        }
        let mut cur: Vec<SqlCursor> = outer_cursors.to_vec();
        cur.extend(p.sql_cursors.iter().filter(|c| c.in_data_division).cloned());
        for n in &p.nested_programs {
            check_program(n, &pass, &cur, ctx);
        }
    }
}

fn use_statement(ctx: &mut Ctx, name: &str, line: u32, verb: &str) {
    if !ctx.prepares.iter().any(|(n, l)| n == name && *l <= line) {
        ctx.error(line, format!("{verb} {name}: the statement is not prepared before this line (PREPARE {name} FROM …)"));
    }
}

fn check_text(t: &SqlText, visible: &[Decl], ctx: &mut Ctx) {
    for h in &t.inputs {
        check_host(h, visible, false, ctx);
    }
}

fn check_using(u: &SqlUsing, visible: &[Decl], ctx: &mut Ctx) {
    if let SqlUsing::Hosts(hs) = u {
        for h in hs {
            check_host(h, visible, false, ctx);
        }
    }
}

fn check_descriptor(d: &SqlHostName, line: u32, visible: &[Decl], ctx: &mut Ctx) {
    match resolve(visible, d) {
        Resolved::One(decl) if decl.is_group => {}
        Resolved::One(_) => ctx.error(line, format!("{} is not a descriptor: it must be a group item laid out as the SQLDA", d.name)),
        Resolved::Missing => ctx.error(
            line,
            format!("descriptor {} is not declared (EXEC SQL INCLUDE SQLDA END-EXEC declares one)", d.name),
        ),
        Resolved::Ambiguous => ctx.error(line, format!("descriptor {} is ambiguous; qualify it with OF", d.name)),
    }
}

fn check_host(h: &SqlHostRef, visible: &[Decl], _output: bool, ctx: &mut Ctx) {
    let var = match resolve(visible, &h.var) {
        Resolved::One(d) => Some(d.clone()),
        Resolved::Missing => {
            ctx.error(h.line, format!("host variable {} is not declared in the DATA DIVISION", written(&h.var)));
            None
        }
        Resolved::Ambiguous => {
            ctx.error(
                h.line,
                format!("host variable {} is ambiguous: more than one item has that name; qualify it with OF", written(&h.var)),
            );
            None
        }
    };
    let Some(ind) = &h.indicator else { return };
    match resolve(visible, ind) {
        Resolved::One(i) => {
            let structure = var.as_ref().is_some_and(|v| v.is_group);
            let ok = if structure {
                // A host structure takes a table of indicators, named without
                // a subscript.
                i.indicator_shape && i.in_table || i.is_group && i.occurs
            } else {
                i.indicator_shape && !i.occurs
            };
            if !ok {
                let want = if structure {
                    "a table of PIC S9(4) COMP-5 items (one per item of the structure)"
                } else {
                    "a PIC S9(4) item in COMP-5, COMP, BINARY or DISPLAY usage"
                };
                ctx.error(h.line, format!("indicator {} must be {want}", written(ind)));
            }
        }
        Resolved::Missing => {
            ctx.error(h.line, format!("indicator {} is not declared in the DATA DIVISION", written(ind)))
        }
        Resolved::Ambiguous => ctx.error(h.line, format!("indicator {} is ambiguous; qualify it with OF", written(ind))),
    }
}
