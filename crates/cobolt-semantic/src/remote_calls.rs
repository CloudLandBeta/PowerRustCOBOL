// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` at Check time (WASM-005 R2, R4, R7).
//!
//! Three things are judged here, each from what the unit itself holds plus —
//! for the last two — the backend's interface description when the project has
//! imported one:
//!
//! 1. **The arguments are groups of WORKING-STORAGE** (R2). Each argument and
//!    the `RETURNING` receiver must be a plain data-item name that is declared
//!    in a WORKING-STORAGE, LOCAL-STORAGE or LINKAGE section and has a wire
//!    form: an elementary item is a one-field group (its bare scalar), a group
//!    is an object. A literal, an expression, a table element, a file record,
//!    an item with `USAGE COMP-1`/`POINTER` or a `DEPENDING ON` item outside
//!    its group are named.
//! 2. **The name is one the backend exports** (R4). A literal target absent from
//!    the description is an error — and `ON EXCEPTION` does not excuse it: a
//!    local `CALL` may name an optional program, but a remote name that the
//!    contract does not list can never answer.
//! 3. **The layouts fit** (R7). Each argument's layout must be accepted by the
//!    parameter it lands on, and what comes back must be readable by it.

use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::{DataSection, ProcedureBody, Program};
use cobolt_ast::stmt::{CallArg, CallConvention, Stmt};
use cobolt_wire::compat::{fits, fits_projected};
use cobolt_wire::contract::{Direction, InterfaceDescription};
use cobolt_wire::layout::{from_decl, Item};

use crate::{SemanticDiagnostic, Severity};

/// Check every `WEBSOCKET` call in `program` and in the programs it contains.
pub fn check(program: &Program, interface: Option<&InterfaceDescription>, diagnostics: &mut Vec<SemanticDiagnostic>) {
    walk(program, &mut Vec::new(), interface, diagnostics);
}

fn walk<'a>(
    prog: &'a Program,
    ancestors: &mut Vec<&'a Program>,
    interface: Option<&InterfaceDescription>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    {
        let anc: &[&Program] = ancestors;
        let mut visit = |stmt: &Stmt| {
            if matches!(stmt, Stmt::Call { convention: CallConvention::WebSocket, .. }) {
                check_call(stmt, prog, anc, interface, diagnostics);
            }
        };
        match &prog.procedure.body {
            ProcedureBody::Sections(sections) => {
                for sec in sections {
                    for para in &sec.paragraphs {
                        para.stmts.iter().for_each(|s| s.walk(&mut visit));
                    }
                }
            }
            ProcedureBody::Paragraphs(paras) => {
                for para in paras {
                    para.stmts.iter().for_each(|s| s.walk(&mut visit));
                }
            }
        }
        for decl in &prog.procedure.declaratives {
            decl.stmts.iter().for_each(|s| s.walk(&mut visit));
            for para in &decl.paras {
                para.stmts.iter().for_each(|s| s.walk(&mut visit));
            }
        }
    }
    ancestors.push(prog);
    for nested in &prog.nested_programs {
        walk(nested, ancestors, interface, diagnostics);
    }
    ancestors.pop();
}

fn err(span: cobolt_lexer::Span, message: String) -> SemanticDiagnostic {
    SemanticDiagnostic { severity: Severity::Error, message, span }
}

fn check_call(
    stmt: &Stmt,
    prog: &Program,
    ancestors: &[&Program],
    interface: Option<&InterfaceDescription>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let Stmt::Call { program, using, returning, span, .. } = stmt else { return };
    let target = match program {
        Expr::Literal(Literal::String(t), _) => Some(t.trim().to_string()),
        _ => None,
    };
    let label = target.clone().unwrap_or_else(|| "<name held in a data item>".to_string());

    // 1. The arguments.
    let mut layouts: Vec<Option<Item>> = Vec::new();
    for (i, arg) in using.iter().enumerate() {
        let (CallArg::ByReference(e) | CallArg::ByValue(e) | CallArg::ByContent(e)) = arg;
        layouts.push(layout_of(e, prog, ancestors, &format!("CALL \"{label}\" WITH WEBSOCKET LINKAGE, argument {}", i + 1), *span, diagnostics));
    }
    let ret_layout = returning
        .as_ref()
        .and_then(|e| layout_of(e, prog, ancestors, &format!("CALL \"{label}\" WITH WEBSOCKET LINKAGE, RETURNING"), *span, diagnostics));

    // 2. The name, and 3. the fit — only with an interface description.
    let (Some(iface), Some(name)) = (interface, target.as_deref()) else { return };
    let Some(sig) = iface.program(name) else {
        let known: Vec<&str> = iface.programs.iter().map(|p| p.name.as_str()).collect();
        let near = crate::resolver::closest_object(&name.to_ascii_uppercase(), known.iter().copied());
        let hint = near.map(|n| format!(" Did you mean \"{n}\"?")).unwrap_or_default();
        diagnostics.push(err(
            *span,
            format!(
                "CALL \"{name}\" WITH WEBSOCKET LINKAGE: the backend's interface description (contract {}) exports no program of that name, so the call can never be answered.{hint}",
                iface.contract
            ),
        ));
        return;
    };
    if let Some(dep) = &sig.deprecated {
        let removal = dep.removal.as_ref().map(|r| format!(" (removed in contract {r})")).unwrap_or_default();
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Warning,
            message: format!("CALL \"{name}\" WITH WEBSOCKET LINKAGE: the program is deprecated{removal}: {}", dep.message),
            span: *span,
        });
    }
    if using.len() != sig.params.len() {
        diagnostics.push(err(
            *span,
            format!(
                "CALL \"{name}\" WITH WEBSOCKET LINKAGE: the backend's program takes {} argument(s), the call passes {}.",
                sig.params.len(),
                using.len()
            ),
        ));
        return;
    }
    for (i, ((arg, caller), param)) in using.iter().zip(layouts.iter()).zip(sig.params.iter()).enumerate() {
        let Some(caller) = caller else { continue };
        let by_ref = matches!(arg, CallArg::ByReference(_));
        let mut found = Vec::new();
        if param.direction != Direction::Out {
            found.extend(fits(caller, &param.layout));
        }
        if by_ref && param.direction != Direction::In {
            found.extend(fits_projected(&param.layout, caller));
        }
        for m in found {
            diagnostics.push(err(
                *span,
                format!(
                    "CALL \"{name}\" WITH WEBSOCKET LINKAGE, argument {} ({}): the group does not fit the backend's parameter {} — {m}.",
                    i + 1,
                    caller.name,
                    param.name
                ),
            ));
        }
    }
    match (&ret_layout, &sig.returning, returning.is_some()) {
        (Some(caller), Some(ret), _) => {
            for m in fits_projected(&ret.layout, caller) {
                diagnostics.push(err(
                    *span,
                    format!("CALL \"{name}\" WITH WEBSOCKET LINKAGE, RETURNING {}: the backend's answer {} cannot be read into it — {m}.", caller.name, ret.name),
                ));
            }
        }
        (_, None, true) => diagnostics.push(err(
            *span,
            format!("CALL \"{name}\" WITH WEBSOCKET LINKAGE … RETURNING: the backend's program returns nothing."),
        )),
        _ => {}
    }
}

/// The layout of an argument, reporting why it cannot travel.
fn layout_of(
    e: &Expr,
    prog: &Program,
    ancestors: &[&Program],
    what: &str,
    span: cobolt_lexer::Span,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> Option<Item> {
    let (name, quals): (&str, Vec<&str>) = match e {
        Expr::Identifier(n, _) => (n.as_str(), Vec::new()),
        Expr::Qualified { name, of, .. } => {
            let mut q = Vec::new();
            let mut cur = of.as_ref();
            loop {
                match cur {
                    Expr::Identifier(n, _) => {
                        q.push(n.as_str());
                        break;
                    }
                    Expr::Qualified { name, of, .. } => {
                        q.push(name.as_str());
                        cur = of.as_ref();
                    }
                    _ => break,
                }
            }
            (name.as_str(), q)
        }
        Expr::Subscript { .. } => {
            diagnostics.push(err(span, format!("{what}: pass the whole group, not one element of a table.")));
            return None;
        }
        _ => {
            diagnostics.push(err(
                span,
                format!("{what}: an argument must be a data item declared in WORKING-STORAGE — a literal, an expression or a reference modification cannot travel."),
            ));
            return None;
        }
    };
    // The unit's own program first, then the programs that contain it.
    let mut scopes: Vec<&Program> = vec![prog];
    scopes.extend(ancestors.iter().rev().copied());
    let mut in_file_section = false;
    for p in scopes {
        let Some(data) = &p.data else { continue };
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items) | DataSection::LocalStorage(items) | DataSection::Linkage(items) => {
                    if let Some(decl) = find(items, name, &quals) {
                        return match from_decl(decl) {
                            Ok(item) => Some(item),
                            Err(e) => {
                                diagnostics.push(err(span, format!("{what}: {name} cannot travel — {e}.")));
                                None
                            }
                        };
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        if find(&fd.records, name, &quals).is_some() {
                            in_file_section = true;
                        }
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
    }
    if in_file_section {
        diagnostics.push(err(span, format!("{what}: {name} is a file record; an argument must be declared in WORKING-STORAGE — move the record into a group first.")));
    }
    // Undeclared names are the resolver's to report.
    None
}

/// The declaration named `name` anywhere under `items`, whose ancestors include
/// `quals` (nearest first) in order.
fn find<'a>(items: &'a [DataDecl], name: &str, quals: &[&str]) -> Option<&'a DataDecl> {
    fn dfs<'a>(decl: &'a DataDecl, name: &str, quals: &[&str], path: &mut Vec<&'a str>) -> Option<&'a DataDecl> {
        if decl.level == 88 || decl.level == 66 {
            return None;
        }
        if decl.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            // `path` holds the ancestors, outermost first; the qualifiers are nearest first.
            let mut it = path.iter().rev();
            let ok = quals.iter().all(|q| it.any(|a| a.eq_ignore_ascii_case(q)));
            if ok {
                return Some(decl);
            }
        }
        if let Some(n) = decl.name.as_deref() {
            path.push(n);
        }
        let r = decl.children.iter().find_map(|c| dfs(c, name, quals, path));
        if decl.name.is_some() {
            path.pop();
        }
        r
    }
    items.iter().find_map(|d| dfs(d, name, quals, &mut Vec::new()))
}
