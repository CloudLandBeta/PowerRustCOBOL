// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The member calls on a web form's class C controls (WASM-005 T11; the
//! frontend's lowering of T14 builds on it).
//!
//! On the web a class C control's method runs on the backend, in a program the
//! Build generates per call site (`cobolt_codegen::remote_controls`). This
//! finds those sites in the form's parsed program: every `CTRL::Method(…)` and
//! every `INVOKE CTRL "Method" …` whose receiver is one of the form's controls
//! of a type the class C catalogue lists, with each argument's literal text
//! when it is a literal.
//!
//! The walk goes over the program's **serialised** form. The AST is
//! `Serialize`, so this reaches every expression of every statement — nested
//! programs, conditions, `EVALUATE` subjects, `STRING` operands — without a
//! visitor per statement kind that would have to keep up with the enum; each
//! node found is read back as the typed [`Expr`] / [`Stmt`] it is.

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::Program;
use cobolt_ast::stmt::Stmt;
use cobolt_codegen::remote_controls::{CallSite, SiteArg, SiteError};
use cobolt_forms::{Control, Form};
use cobolt_lexer::Span;

/// The member calls on `form`'s class C controls in `program`, in source
/// order. A call on any other receiver, and a property read (no parentheses),
/// is not one.
pub fn class_c_call_sites(program: &Program, form: &Form) -> Vec<CallSite> {
    let mut controls = Vec::new();
    flatten(&form.controls, &mut controls);
    let is_class_c = |name: &str| {
        controls.iter().any(|c| {
            c.id.eq_ignore_ascii_case(name.trim())
                && cobolt_wire::controls::of_control(c.control_type.as_str()).next().is_some()
        })
    };
    let mut sites = Vec::new();
    let Ok(tree) = serde_json::to_value(program) else { return sites };
    walk(&tree, &mut |node| {
        let Some((tag, _)) = node.as_object().and_then(|o| (o.len() == 1).then(|| o.iter().next()).flatten()) else {
            return;
        };
        match tag.as_str() {
            "Member" => {
                if let Ok(Expr::Member { recv, member, args, parens: true, span }) = serde_json::from_value::<Expr>(node.clone()) {
                    if let Expr::Identifier(root, _) = recv.as_ref() {
                        if is_class_c(root) {
                            sites.push(site(root, &member, &args, span.line, span.col));
                        }
                    }
                }
            }
            "Invoke" => {
                if let Ok(Stmt::Invoke { object, method, args, span, .. }) = serde_json::from_value::<Stmt>(node.clone()) {
                    if is_class_c(&object) {
                        sites.push(site(&object, &method, &args, span.line, span.col));
                    }
                }
            }
            _ => {}
        }
    });
    sites.sort_by_key(|s| (s.line, s.column));
    sites
}

fn site(control: &str, method: &str, args: &[Expr], line: u32, column: u32) -> CallSite {
    CallSite {
        control: control.trim().to_owned(),
        method: method.trim().trim_matches('"').trim_matches('\'').to_owned(),
        line,
        column,
        args: args.iter().map(arg).collect(),
    }
}

/// A literal argument's text, as the handler wrote its value.
fn arg(e: &Expr) -> SiteArg {
    match e {
        Expr::Literal(l, _) => match l {
            Literal::String(s) | Literal::National(s) => SiteArg::Literal(s.clone()),
            Literal::Integer(n) => SiteArg::Literal(n.to_string()),
            Literal::IntegerDigits(n, digits) => SiteArg::Literal(format!("{n:0width$}", width = *digits as usize)),
            Literal::Decimal(m, scale) => SiteArg::Literal(decimal_text(*m, *scale)),
            _ => SiteArg::Expression,
        },
        _ => SiteArg::Expression,
    }
}

/// `mantissa × 10^-scale` as written: `314`, 2 → `3.14`.
fn decimal_text(mantissa: i128, scale: u8) -> String {
    if scale == 0 {
        return mantissa.to_string();
    }
    let sign = if mantissa < 0 { "-" } else { "" };
    let digits = format!("{:0width$}", mantissa.unsigned_abs(), width = scale as usize + 1);
    let (int, frac) = digits.split_at(digits.len() - scale as usize);
    format!("{sign}{int}.{frac}")
}

// ── The frontend's lowering (T14) ───────────────────────────────────────────

/// One served call site, as the lowering rewrites it.
struct Lowering {
    /// The control as the form spells it (what `SET-PROPERTY`/`RAISE-EVENT` name).
    control: String,
    line: u32,
    column: u32,
    endpoint: String,
    row: &'static cobolt_wire::controls::Operation,
    /// The site's value item number (from 1, source order over the form).
    value: usize,
}

/// Rewrite every class C method call of `form`'s program into a call to the
/// backend program the Build generates for it (WASM-005 T14, R12, R16; plan
/// §1.3 point 3, D3). For each site, before the statement that holds it:
///
/// - the sent arguments (those the catalogue does not fix at Build, R17) are
///   moved into `RC-PARAMS`, and `CALL "<endpoint>" WITH WEBSOCKET LINKAGE
///   USING BY VALUE RC-PARAMS RETURNING RC-RESULT` is made — the endpoint the
///   backend program has, numbered by the same `served_sites`;
/// - **NOT ON EXCEPTION**: the answer's value goes to the site's own value item,
///   the properties the backend reported are set on the control by name
///   (`COBOL::"SET-PROPERTY"`), `LastError` is set from the answer, and the
///   events the backend's control raised are raised on this one
///   (`COBOL::"RAISE-EVENT"`), so their handlers run after this handler, as on
///   the desktop (operator's T11 decision 3, T14 decisions 1 and 2);
/// - **ON EXCEPTION** (supplied: a method call has none of its own): the
///   failure's message goes to the control's `LastError` and `onError` is
///   raised, as a failing method does on the desktop (T14 decision 3).
///
/// The statement then reads the value item where it called the method; a
/// method called as a statement, or by `INVOKE`, becomes the call alone (and
/// the `MOVE` of its `RETURNING`). The groups are declared `GLOBAL` in the
/// form's program from `cobolt_codegen::remote_controls::frontend_working_storage`.
///
/// Run by the web build on the parsed program of a web or mobile form, after
/// Common Code is linked; never on a desktop form, and never on the
/// developer's source files (user code is sacred). Answers how many sites were
/// lowered (0 leaves the program untouched), or every site it refuses: one in
/// a loop or `SEARCH`/`EVALUATE … WHEN` condition, or in an `AND`/`OR` — the
/// desktop would make that call conditionally or repeatedly, a single hoisted
/// call cannot — and a form whose own items take a name the groups need.
pub fn lower_class_c_calls(program: &mut Program, form: &Form) -> Result<usize, Vec<SiteError>> {
    use cobolt_codegen::remote_controls::{frontend_names, frontend_working_storage, served_sites};
    let sites = class_c_call_sites(program, form);
    let served = served_sites(form, &sites);
    if served.is_empty() {
        return Ok(0);
    }
    let mut errs = Vec::new();
    let mut plan = Vec::new();
    for (i, s) in served.iter().enumerate() {
        match &s.endpoint {
            Ok(e) => plan.push(Lowering {
                control: s.control.id.clone(),
                line: s.site.line,
                column: s.site.column,
                endpoint: e.clone(),
                row: s.row,
                value: i + 1,
            }),
            Err(m) => errs.push(SiteError { line: s.site.line, message: format!("{}: {}::{} — {m}", form.name, s.control.id, s.row.method) }),
        }
    }
    let row = plan.first().map(|p| p.row).unwrap_or(served[0].row);
    if let Some(p) = plan.iter().find(|p| p.row.params != row.params || p.row.result != row.result) {
        errs.push(SiteError {
            line: p.line,
            message: format!("{}: {}::{} has a parameter layout of its own, which the web build cannot lower yet", form.name, p.control, p.row.method),
        });
    }
    // A developer's item named like a member of the groups would be ambiguous.
    let reserved = frontend_names(row);
    let mut taken = Vec::new();
    declared_names(program, &mut taken);
    for (name, line) in taken {
        let up = name.to_ascii_uppercase();
        if reserved.iter().any(|r| *r == up) || up.starts_with("RC-VALUE-") {
            errs.push(SiteError {
                line,
                message: format!(
                    "{}: the item {name} takes a name the web build needs for the class C controls' calls (the members of {} and {}); rename it",
                    form.name,
                    cobolt_codegen::remote_controls::frontend_groups().0,
                    cobolt_codegen::remote_controls::frontend_groups().1
                ),
            });
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut cx = Cx { plan: &plan, errs: Vec::new(), done: 0, form: &form.name };
    lower_program(program, &mut cx);
    if !cx.errs.is_empty() {
        return Err(cx.errs);
    }
    let done = cx.done;
    // Declared in the form's own program, where every handler sees them.
    let decls = parse_working_storage(&frontend_working_storage(row, plan.len()))
        .map_err(|m| vec![SiteError { line: 0, message: format!("{}: {m}", form.name) }])?;
    add_working_storage(program, decls);
    Ok(done)
}

struct Cx<'a> {
    plan: &'a [Lowering],
    errs: Vec<SiteError>,
    done: usize,
    form: &'a str,
}

impl Cx<'_> {
    /// The plan entry of the site of `control` at `line`/`column`.
    fn site(&self, control: &str, line: u32, column: u32) -> Option<usize> {
        self.plan.iter().position(|p| p.line == line && p.column == column && p.control.eq_ignore_ascii_case(control.trim()))
    }

    fn refuse(&mut self, i: usize, why: &str) {
        let p = &self.plan[i];
        self.errs.push(SiteError {
            line: p.line,
            message: format!("{}: {}::{} — {why}; on the web it runs on the backend, so call it in a statement of its own first and use the item it fills", self.form, p.control, p.row.method),
        });
    }

    /// The web build could not lower site `i` (a defect of the build, said
    /// rather than leaving the call to run in the browser).
    fn fail(&mut self, i: usize, why: &str) {
        let p = &self.plan[i];
        self.errs.push(SiteError { line: p.line, message: format!("{}: {}::{} — {why}", self.form, p.control, p.row.method) });
    }
}

fn lower_program(p: &mut Program, cx: &mut Cx) {
    use cobolt_ast::program::ProcedureBody;
    for d in &mut p.procedure.declaratives {
        lower_list(&mut d.stmts, cx);
        for para in &mut d.paras {
            lower_list(&mut para.stmts, cx);
        }
    }
    match &mut p.procedure.body {
        ProcedureBody::Sections(secs) => {
            for s in secs {
                for para in &mut s.paragraphs {
                    lower_list(&mut para.stmts, cx);
                }
            }
        }
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                lower_list(&mut para.stmts, cx);
            }
        }
    }
    for n in &mut p.nested_programs {
        lower_program(n, cx);
    }
}

fn lower_list(list: &mut Vec<Stmt>, cx: &mut Cx) {
    let old = std::mem::take(list);
    for mut s in old {
        for inner in s.child_lists_mut() {
            lower_list(inner, cx);
        }
        // `INVOKE CTRL "Method" USING … [RETURNING r]`: the call, then the MOVE.
        if let Stmt::Invoke { object, method: _, args, returning, span, .. } = &s {
            if let Some(i) = cx.site(object, span.line, span.col) {
                let value = cobolt_codegen::remote_controls::frontend_value(cx.plan[i].value);
                match call_statements(&cx.plan[i], args, *span) {
                    Ok(stmts) => list.extend(stmts),
                    Err(m) => cx.fail(i, &m),
                }
                if let Some(r) = returning {
                    list.push(Stmt::Move { from: Expr::Identifier(value, *span), to: vec![r.clone()], span: *span });
                }
                cx.done += 1;
                continue;
            }
        }
        // The statement's own expressions: each site becomes its value item.
        let mut tree = match serde_json::to_value(&s) {
            Ok(t) => t,
            Err(e) => {
                cx.errs.push(SiteError { line: s.span().line, message: format!("{}: this statement cannot be read for the web build: {e}", cx.form) });
                list.push(s);
                continue;
            }
        };
        let mut found: Vec<(usize, Vec<Expr>, Span)> = Vec::new();
        let mut refused: Vec<(usize, &'static str)> = Vec::new();
        let stmt_span = s.span();
        // Where a hoisted call would not be the desktop's call: refused.
        let whole = match &s {
            Stmt::Perform { .. } => Some("it is in a loop's condition, which is tested again on every turn"),
            Stmt::Search { .. } => Some("it is in a SEARCH condition, which is tested once per entry"),
            _ => None,
        };
        let eval_whens = matches!(s, Stmt::Evaluate { .. });
        if let serde_json::Value::Object(top) = &mut tree {
            if let Some(serde_json::Value::Object(body)) = top.values_mut().next() {
                for (field, v) in body.iter_mut() {
                    let why = whole.or_else(|| (eval_whens && field == "whens").then_some("it is in a WHEN of an EVALUATE, which is evaluated only until one matches"));
                    replace_sites(v, cx, why, &mut found, &mut refused);
                }
            }
        }
        for (i, why) in refused {
            cx.refuse(i, why);
        }
        if found.is_empty() {
            list.push(s);
            continue;
        }
        let new = match serde_json::from_value::<Stmt>(tree) {
            Ok(n) => n,
            Err(e) => {
                cx.fail(found[0].0, &format!("the statement cannot be rewritten: {e}"));
                list.push(s);
                continue;
            }
        };
        for (i, args, _) in &found {
            match call_statements(&cx.plan[*i], args, stmt_span) {
                Ok(stmts) => list.extend(stmts),
                Err(m) => cx.fail(*i, &m),
            }
            cx.done += 1;
        }
        // A method called as a statement: the call is all there is.
        if matches!(&new, Stmt::InvokeExpr { expr: Expr::Identifier(..), .. }) {
            continue;
        }
        list.push(new);
    }
}

/// Replace every served site under `v` by its value item, children first (an
/// argument that is itself a site is called first). `why` refuses what is
/// found instead; an `AND`/`OR` refuses its operands.
fn replace_sites(
    v: &mut serde_json::Value,
    cx: &Cx,
    why: Option<&'static str>,
    found: &mut Vec<(usize, Vec<Expr>, Span)>,
    refused: &mut Vec<(usize, &'static str)>,
) {
    let tag = match v {
        serde_json::Value::Object(o) if o.len() == 1 => o.keys().next().cloned(),
        _ => None,
    };
    let inner_why = match tag.as_deref() {
        Some("And") | Some("Or") => why.or(Some("it is in an AND or OR condition, whose second part the desktop tests only when the first does not decide it")),
        _ => why,
    };
    match v {
        serde_json::Value::Array(items) => items.iter_mut().for_each(|i| replace_sites(i, cx, inner_why, found, refused)),
        serde_json::Value::Object(map) => map.values_mut().for_each(|i| replace_sites(i, cx, inner_why, found, refused)),
        _ => {}
    }
    if tag.as_deref() != Some("Member") {
        return;
    }
    let Ok(Expr::Member { recv, args, parens: true, span, .. }) = serde_json::from_value::<Expr>(v.clone()) else { return };
    let Expr::Identifier(root, _) = recv.as_ref() else { return };
    let Some(i) = cx.plan.iter().position(|p| p.line == span.line && p.column == span.col && p.control.eq_ignore_ascii_case(root.trim())) else {
        return;
    };
    if let Some(w) = inner_why {
        refused.push((i, w));
        return;
    }
    let value = cobolt_codegen::remote_controls::frontend_value(cx.plan[i].value);
    if let Ok(id) = serde_json::to_value(Expr::Identifier(value, span)) {
        *v = id;
        found.push((i, args, span));
    }
}

/// The statements of one lowered call (see [`lower_class_c_calls`]), every
/// span the developer's statement's, so a diagnostic, a breakpoint and the
/// module's line table all point at the line they wrote.
fn call_statements(p: &Lowering, args: &[Expr], at: Span) -> Result<Vec<Stmt>, String> {
    use cobolt_codegen::remote_controls::{frontend_groups, frontend_value, FRONTEND_ERR_CLASS, FRONTEND_ERR_CODE, FRONTEND_ERR_MESSAGE, FRONTEND_INDEX};
    let (params, result) = frontend_groups();
    let value = frontend_value(p.value);
    let ctrl = p.control.replace('"', "\"\"");
    let sent: Vec<&Expr> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| !p.row.fixed.iter().any(|(pos, _)| *pos as usize == i + 1))
        .map(|(_, a)| a)
        .collect();
    let mut src = String::new();
    src.push_str(&format!("           MOVE {} TO ARG-COUNT OF {params}\n", sent.len()));
    for k in 1..=sent.len() {
        src.push_str(&format!("           MOVE RC-ARG-{k} TO ARG-VALUE OF {params} ( {k} )\n"));
    }
    // Every inner CALL has its END-CALL: without one, the outer call's NOT ON
    // EXCEPTION would belong to the last CALL before it.
    src.push_str(&format!(
        "           CALL \"{}\" WITH WEBSOCKET LINKAGE
               USING BY VALUE {params} RETURNING {result}
               ON EXCEPTION
                   MOVE SPACES TO {value}
                   CALL \"COBOL-WS-LAST-ERROR\" USING {FRONTEND_ERR_CLASS} {FRONTEND_ERR_CODE} {FRONTEND_ERR_MESSAGE} END-CALL
                   CALL \"COBOL-SET-PROPERTY\" USING \"{ctrl}\" \"LastError\" {FRONTEND_ERR_MESSAGE} END-CALL
                   CALL \"COBOL-RAISE-EVENT\" USING \"{ctrl}\" \"onError\" END-CALL
               NOT ON EXCEPTION
                   MOVE RESULT-VALUE OF {result} TO {value}
                   PERFORM VARYING {FRONTEND_INDEX} FROM 1 BY 1 UNTIL {FRONTEND_INDEX} > PROP-COUNT OF {result}
                       CALL \"COBOL-SET-PROPERTY\" USING \"{ctrl}\" PROP-NAME OF {result} ( {FRONTEND_INDEX} ) PROP-VALUE OF {result} ( {FRONTEND_INDEX} ) END-CALL
                   END-PERFORM
                   CALL \"COBOL-SET-PROPERTY\" USING \"{ctrl}\" \"LastError\" LAST-ERROR OF {result} END-CALL
                   PERFORM VARYING {FRONTEND_INDEX} FROM 1 BY 1 UNTIL {FRONTEND_INDEX} > EVENT-COUNT OF {result}
                       CALL \"COBOL-RAISE-EVENT\" USING \"{ctrl}\" EVENT-NAME OF {result} ( {FRONTEND_INDEX} ) END-CALL
                   END-PERFORM
           END-CALL\n",
        p.endpoint
    ));
    if sent.len() > cobolt_wire::controls::MAX_ARGS {
        return Err(format!("it sends {} arguments; a call to the backend carries at most {}", sent.len(), cobolt_wire::controls::MAX_ARGS));
    }
    let stmts = parse_statements(&src)?;
    // The shape the template must have: the MOVEs, then one WEBSOCKET call
    // whose two phrases hold their statements (not an inner CALL's).
    let ok = stmts.len() == sent.len() + 2
        && matches!(stmts.last(), Some(Stmt::Call { on_exception, not_on_exception, .. }) if on_exception.len() == 4 && not_on_exception.len() == 4);
    if !ok {
        let shape: Vec<String> = stmts
            .iter()
            .map(|s| match s {
                Stmt::Call { on_exception, not_on_exception, .. } => format!("CALL({}, {})", on_exception.len(), not_on_exception.len()),
                other => cobolt_wasm::support::stmt_name(other).to_string(),
            })
            .collect();
        return Err(format!("the web build made a call it cannot read back (a defect of the build): {}", shape.join(" ")));
    }
    // The placeholders become the handler's own argument expressions.
    let mut tree = serde_json::to_value(&stmts).map_err(|e| e.to_string())?;
    fill(&mut tree, &sent, at);
    serde_json::from_value::<Vec<Stmt>>(tree).map_err(|e| e.to_string())
}

/// Put `args[k-1]` where the template has `RC-ARG-k`, and `at` for every span
/// of the template's own.
fn fill(v: &mut serde_json::Value, args: &[&Expr], at: Span) {
    if let serde_json::Value::Object(o) = v {
        if let Some(serde_json::Value::Array(id)) = o.get("Identifier") {
            if let Some(k) = id.first().and_then(|n| n.as_str()).and_then(|n| n.strip_prefix("RC-ARG-")).and_then(|k| k.parse::<usize>().ok()) {
                if let Some(e) = args.get(k.wrapping_sub(1)).and_then(|a| serde_json::to_value(a).ok()) {
                    *v = e;
                    return;
                }
            }
        }
        if o.len() == 4 && ["start", "end", "line", "col"].iter().all(|k| o.contains_key(*k)) {
            if let Ok(s) = serde_json::to_value(at) {
                *v = s;
            }
            return;
        }
    }
    match v {
        serde_json::Value::Array(items) => items.iter_mut().for_each(|i| fill(i, args, at)),
        serde_json::Value::Object(map) => map.values_mut().for_each(|i| fill(i, args, at)),
        _ => {}
    }
}

fn parse_template(src: &str) -> Result<Program, String> {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect();
    if !errors.is_empty() {
        return Err(format!("the generated statements do not parse: {}", errors.join("; ")));
    }
    parsed.program.ok_or_else(|| "the generated statements do not parse".to_owned())
}

fn parse_statements(body: &str) -> Result<Vec<Stmt>, String> {
    use cobolt_ast::program::ProcedureBody;
    let src = format!("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. RC-LOWERING.\n       PROCEDURE DIVISION.\n       RC-SITE.\n{body}           .\n");
    let p = parse_template(&src)?;
    let stmts = match p.procedure.body {
        ProcedureBody::Paragraphs(mut paras) if !paras.is_empty() => paras.swap_remove(0).stmts,
        _ => return Err("the generated statements do not parse".into()),
    };
    Ok(stmts.into_iter().filter(|s| !matches!(s, Stmt::SentenceEnd { .. })).collect())
}

fn parse_working_storage(text: &str) -> Result<Vec<cobolt_ast::data::DataDecl>, String> {
    use cobolt_ast::program::DataSection;
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. RC-LOWERING.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{text}       PROCEDURE DIVISION.\n           GOBACK.\n"
    );
    let p = parse_template(&src)?;
    p.data
        .and_then(|d| d.sections.into_iter().find_map(|s| if let DataSection::WorkingStorage(v) = s { Some(v) } else { None }))
        .ok_or_else(|| "the generated declarations do not parse".to_owned())
}

/// Append `decls` to the WORKING-STORAGE of `program`, making the section (and
/// the DATA DIVISION) when it has none.
fn add_working_storage(program: &mut Program, decls: Vec<cobolt_ast::data::DataDecl>) {
    use cobolt_ast::program::{DataDivision, DataSection};
    let data = program.data.get_or_insert_with(|| DataDivision { sections: Vec::new(), span: program.procedure.span });
    if let Some(DataSection::WorkingStorage(ws)) = data.sections.iter_mut().find(|s| matches!(s, DataSection::WorkingStorage(_))) {
        ws.extend(decls);
        return;
    }
    // WORKING-STORAGE comes after the FILE SECTION and before the others.
    let at = data.sections.iter().position(|s| !matches!(s, DataSection::FileSection(_))).unwrap_or(data.sections.len());
    data.sections.insert(at, DataSection::WorkingStorage(decls));
}

/// Every data name `program` and its nested programs declare, with its line.
fn declared_names(program: &Program, out: &mut Vec<(String, u32)>) {
    use cobolt_ast::program::DataSection;
    fn items(d: &[cobolt_ast::data::DataDecl], out: &mut Vec<(String, u32)>) {
        for x in d {
            if let Some(n) = &x.name {
                out.push((n.clone(), x.span.line));
            }
            items(&x.children, out);
        }
    }
    if let Some(data) = &program.data {
        for s in &data.sections {
            match s {
                DataSection::WorkingStorage(v) | DataSection::LocalStorage(v) | DataSection::Linkage(v) => items(v, out),
                DataSection::FileSection(fds) => fds.iter().for_each(|fd| items(&fd.records, out)),
                DataSection::Screen(_) => {}
            }
        }
    }
    for n in &program.nested_programs {
        declared_names(n, out);
    }
}

fn walk(v: &serde_json::Value, f: &mut impl FnMut(&serde_json::Value)) {
    f(v);
    match v {
        serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, f)),
        serde_json::Value::Object(map) => map.values().for_each(|i| walk(i, f)),
        _ => {}
    }
}

fn flatten<'a>(list: &'a [Control], out: &mut Vec<&'a Control>) {
    for c in list {
        out.push(c);
        flatten(&c.children, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::ControlType;

    fn form() -> Form {
        let mut f = Form::new("SHOP", "Shop", 400, 300);
        f.controls.push(Control::new("REST-1", ControlType::RestClient, 0, 0));
        f.controls.push(Control::new("AGENT-1", ControlType::AgentObject, 0, 0));
        f.controls.push(Control::new("LBL-1", ControlType::Label, 0, 0));
        f
    }

    fn parse(src: &str) -> Program {
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect();
        assert!(errors.is_empty(), "{errors:?}");
        parsed.program.unwrap()
    }

    #[test]
    fn every_member_call_on_a_class_c_control_is_found_with_its_literals() {
        let program = parse(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-BODY PIC X(80).
       01 WS-ANSWER PIC X(200).
       PROCEDURE DIVISION.
           MOVE REST-1::Get(\"customers\") TO WS-ANSWER
           IF REST-1::Post(\"orders\" WS-BODY) = SPACES
               DISPLAY \"none\"
           END-IF
           INVOKE AGENT-1 \"Ask\" USING WS-BODY RETURNING WS-ANSWER
           MOVE REST-1::BaseURL TO WS-ANSWER
           MOVE LBL-1::GetCaption() TO WS-ANSWER
           STOP RUN.
",
        );
        let sites = class_c_call_sites(&program, &form());
        let got: Vec<(String, String, u32, Vec<SiteArg>)> =
            sites.into_iter().map(|s| (s.control, s.method, s.line, s.args)).collect();
        assert_eq!(
            got,
            vec![
                ("REST-1".into(), "GET".into(), 8, vec![SiteArg::Literal("customers".into())]),
                ("REST-1".into(), "POST".into(), 9, vec![SiteArg::Literal("orders".into()), SiteArg::Expression]),
                ("AGENT-1".into(), "Ask".into(), 12, vec![SiteArg::Expression]),
            ],
            "a property read and a call on a class A control are not sites"
        );
    }

    #[test]
    fn a_lowered_call_is_the_moves_and_one_websocket_call_with_both_phrases() {
        let row = cobolt_wire::controls::find("RestClient", "POST").unwrap();
        let p = Lowering { control: "REST-1".into(), line: 9, column: 12, endpoint: "RC-4-SHOP-6-REST-1-POST-1".into(), row, value: 1 };
        let at = Span::new(0, 0, 9, 12);
        let args = [Expr::Literal(Literal::String("orders".into()), at), Expr::Identifier("WS-BODY".into(), at)];
        let stmts = call_statements(&p, &args, at).unwrap_or_else(|e| {
            let src = parse_statements("           MOVE 1 TO X\n").map(|s| format!("{s:?}")).unwrap_or_default();
            panic!("{e}\n{src}")
        });
        assert_eq!(stmts.len(), 3, "ARG-COUNT, the one sent argument, the call");
        let Stmt::Call { program, using, returning, convention, .. } = &stmts[2] else { panic!("{:?}", stmts[2]) };
        assert_eq!(*convention, cobolt_ast::stmt::CallConvention::WebSocket);
        assert!(matches!(program, Expr::Literal(Literal::String(s), _) if s == "RC-4-SHOP-6-REST-1-POST-1"));
        assert_eq!(using.len(), 1);
        assert!(returning.is_some());
        // Every span is the developer's statement's.
        assert!(serde_json::to_string(&stmts).unwrap().matches("\"line\":").all(|_| true));
        let text = serde_json::to_string(&stmts).unwrap();
        assert!(!text.contains("RC-ARG-"), "every placeholder filled");
        assert!(text.contains("WS-BODY"));
        assert!(!text.contains("\"line\":1,") && !text.contains("\"line\":5,"), "no span of the template's own");
    }

    #[test]
    fn a_numeric_literal_keeps_its_written_digits() {
        assert_eq!(decimal_text(314, 2), "3.14");
        assert_eq!(decimal_text(-5, 3), "-0.005");
        assert_eq!(decimal_text(42, 0), "42");
    }
}
