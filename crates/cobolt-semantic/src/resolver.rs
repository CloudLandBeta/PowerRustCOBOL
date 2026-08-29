// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Name resolution pass.
//!
//! Walks all statements and checks that:
//!
//! * Every data-item reference (`Expr::Identifier`) names a declared item.
//! * Every PERFORM target names a declared paragraph or section.
//! * Every GO TO target names a declared paragraph.
//! * CALL targets that are literals are left unchecked (external programs).
//!
//! Unknown names produce [`Severity::Warning`] rather than hard errors
//! because COBOL programs commonly reference items from copybooks or
//! runtime libraries that are not present in the source being analysed.

use cobolt_ast::{
    expr::{Condition, Expr, Literal},
    program::{ProcedureBody, Program},
    stmt::{CallArg, PerformTarget, Stmt, WhenValue},
};

use crate::{symbol_table::SymbolTable, SemanticDiagnostic, Severity};

/// 049 R33 — the universal form surface: the property set EVERY form carries,
/// so a bare `me::X` / `super::…::X` is checkable at build time whatever form
/// the receiver turns out to be. Must match the form-entry seed in
/// `cobolt-form-host::seeding::build_object_seed`.
pub const UNIVERSAL_FORM_PROPS: &[&str] = &[
    "Name",
    "Title",
    "Width",
    "Height",
    "X",
    "Y",
    "WindowState",
    "FullScreen",
    "TitleVisible",
    "CanMinimize",
    "CanMaximize",
    "FormState",
    "FormFormat",
    "BackgroundColor",
    "Transparency",
    // The breadcrumb reset guard: on, a click on this form's own breadcrumb
    // segment fires `onResetRejected` instead of starting the form over.
    "PreventReset",
];

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn resolve(
    program: &Program,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    form_formats: Option<&std::collections::HashMap<String, crate::FormLoadFormat>>,
) {
    let mut ctx = ResolveCtx {
        symbols,
        diagnostics,
        form_formats,
    };
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                for stmt in &para.stmts {
                    ctx.resolve_stmt(stmt);
                }
            }
        }
        ProcedureBody::Sections(secs) => {
            for sec in secs {
                for para in &sec.paragraphs {
                    for stmt in &para.stmts {
                        ctx.resolve_stmt(stmt);
                    }
                }
            }
        }
    }
}

// ── Internal context ──────────────────────────────────────────────────────────

struct ResolveCtx<'a> {
    symbols: &'a SymbolTable,
    diagnostics: &'a mut Vec<SemanticDiagnostic>,
    /// 049 R17 — the project's form formats (UPPERCASE id → format), when a
    /// project supplied them. `None` disables the load-path check.
    form_formats: Option<&'a std::collections::HashMap<String, crate::FormLoadFormat>>,
}

impl<'a> ResolveCtx<'a> {
    fn warn(&mut self, msg: impl Into<String>, span: cobolt_lexer::Span) {
        self.diagnostics.push(SemanticDiagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span,
        });
    }

    fn error(&mut self, msg: impl Into<String>, span: cobolt_lexer::Span) {
        self.diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span,
        });
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Move { from, to, .. } => {
                self.resolve_expr(from);
                for t in to {
                    self.check_receiving(t);
                    self.resolve_expr(t);
                }
            }
            Stmt::MoveCorresponding { from, to, .. }
            | Stmt::AddCorresponding { from, to, .. }
            | Stmt::SubtractCorresponding { from, to, .. } => {
                self.resolve_expr(from);
                self.resolve_expr(to);
            }
            Stmt::Add {
                operands,
                to,
                giving,
                ..
            } => {
                for e in operands {
                    self.resolve_expr(e);
                }
                for (t, _) in to {
                    self.resolve_expr(t);
                }
                for (g, _) in giving {
                    self.resolve_expr(g);
                }
            }
            Stmt::Subtract {
                operands,
                from,
                giving,
                ..
            } => {
                for e in operands {
                    self.resolve_expr(e);
                }
                for (f, _) in from {
                    self.resolve_expr(f);
                }
                for (g, _) in giving {
                    self.resolve_expr(g);
                }
            }
            Stmt::Multiply {
                lhs, by, giving, ..
            } => {
                self.resolve_expr(lhs);
                self.resolve_expr(by);
                for (g, _) in giving {
                    self.resolve_expr(g);
                }
            }
            Stmt::Divide {
                lhs,
                by,
                giving,
                remainder,
                ..
            } => {
                self.resolve_expr(lhs);
                self.resolve_expr(by);
                for (g, _) in giving {
                    self.resolve_expr(g);
                }
                if let Some(r) = remainder {
                    self.resolve_expr(r);
                }
            }
            Stmt::Compute { targets, expr, .. } => {
                for (t, _) in targets {
                    self.check_receiving(t);
                    self.resolve_expr(t);
                }
                self.resolve_expr(expr);
            }
            Stmt::If {
                condition,
                then_stmts,
                else_stmts,
                ..
            } => {
                self.resolve_condition(condition);
                for s in then_stmts {
                    self.resolve_stmt(s);
                }
                for s in else_stmts {
                    self.resolve_stmt(s);
                }
            }
            Stmt::Evaluate {
                whens, other_stmts, ..
            } => {
                for w in whens {
                    for v in &w.values {
                        if let WhenValue::Condition(c) = v {
                            self.resolve_condition(c);
                        }
                    }
                    for s in &w.stmts {
                        self.resolve_stmt(s);
                    }
                }
                for s in other_stmts {
                    self.resolve_stmt(s);
                }
            }
            Stmt::Perform { target, .. } => match target {
                PerformTarget::Paragraph(name, s) => self.check_procedure(name, *s),
                PerformTarget::Section(name, s) => self.check_procedure(name, *s),
                // Both halves name a procedure; the qualifier only says which
                // copy of a repeated name is meant.
                PerformTarget::QualifiedParagraph {
                    name,
                    section,
                    span,
                } => {
                    self.check_procedure(name, *span);
                    self.check_procedure(section, *span);
                }
                PerformTarget::Thru { from, to, span } => {
                    self.check_procedure(from, *span);
                    self.check_procedure(to, *span);
                }
                PerformTarget::Inline { stmts } | PerformTarget::Times { stmts, .. } => {
                    for s in stmts {
                        self.resolve_stmt(s);
                    }
                }
                PerformTarget::Until {
                    condition, stmts, ..
                } => {
                    self.resolve_condition(condition);
                    for s in stmts {
                        self.resolve_stmt(s);
                    }
                }
                PerformTarget::Varying {
                    var,
                    from,
                    by,
                    until,
                    stmts,
                    ..
                } => {
                    self.resolve_expr(var);
                    self.resolve_expr(from);
                    self.resolve_expr(by);
                    self.resolve_condition(until);
                    for s in stmts {
                        self.resolve_stmt(s);
                    }
                }
            },
            // An empty target is the altered `GO TO.`; the procedure-name comes
            // from an `ALTER` at run time, so there is nothing to resolve here.
            // The qualifier names a SECTION, which `check_procedure` already
            // knows as a procedure-name; the paragraph itself is what has to
            // exist, so only `target` is checked.
            Stmt::GoTo { target, span, .. } => {
                if !target.is_empty() {
                    self.check_procedure(target, *span)
                }
            }
            Stmt::GoToDepending {
                targets,
                depending,
                span,
            } => {
                for t in targets {
                    self.check_procedure(t, *span);
                }
                self.resolve_expr(depending);
            }
            Stmt::Display { operands, .. } => {
                for e in operands {
                    self.resolve_expr(e);
                }
            }
            Stmt::Accept { target, .. } => self.resolve_expr(target),
            Stmt::Call {
                program,
                using,
                returning,
                on_exception,
                not_on_exception,
                ..
            } => {
                self.resolve_expr(program);
                for arg in using {
                    let e = match arg {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
                    };
                    self.resolve_expr(e);
                }
                if let Some(r) = returning {
                    self.resolve_expr(r);
                }
                for s in on_exception {
                    self.resolve_stmt(s);
                }
                for s in not_on_exception {
                    self.resolve_stmt(s);
                }
            }
            Stmt::Write { record, from, .. } => {
                self.resolve_expr(record);
                if let Some(f) = from {
                    self.resolve_expr(f);
                }
            }
            Stmt::Rewrite { record, from, .. } => {
                self.resolve_expr(record);
                if let Some(f) = from {
                    self.resolve_expr(f);
                }
            }
            Stmt::Read {
                into,
                at_end,
                not_at_end,
                ..
            } => {
                if let Some(i) = into {
                    self.resolve_expr(i);
                }
                for s in at_end {
                    self.resolve_stmt(s);
                }
                for s in not_at_end {
                    self.resolve_stmt(s);
                }
            }
            // EXEC RUST — source is opaque Rust; the exec_rust pass handles it.
            Stmt::ExecRust { .. } => {}
            Stmt::TryCatch {
                try_stmts,
                catch_stmts,
                finally_stmts,
                ..
            } => {
                for s in try_stmts {
                    self.resolve_stmt(s);
                }
                for s in catch_stmts {
                    self.resolve_stmt(s);
                }
                for s in finally_stmts {
                    self.resolve_stmt(s);
                }
            }
            Stmt::Throw { message, .. } => {
                self.resolve_expr(message);
            }
            // Visual-object method invocation: object is a control; resolve the
            // argument expressions and the optional RETURNING receiver.
            Stmt::Invoke {
                method,
                args,
                returning,
                comma_form,
                span,
                ..
            } => {
                // 037 R22 — the SPACE (COBOL-standard) form of the checked
                // window methods requires every parameter; a mismatch is a
                // compile-time error naming the method and its signature.
                // The comma form defaults omitted trailing parameters (R21)
                // and is exempt.
                if !comma_form {
                    self.check_open_form_signature(method, args, *span);
                }
                // 049 R17 — the load-path check applies to BOTH spellings:
                // however the call is written, its target must be openable as
                // a window.
                self.check_open_form_target(method, args);
                for a in args {
                    self.resolve_expr(a);
                }
                if let Some(r) = returning {
                    self.resolve_expr(r);
                }
            }
            _ => {}
        }
    }

    /// 037 R22 — compile-time signature check for the window-invocation
    /// methods when written in the SPACE (COBOL-standard) form: every
    /// parameter is required and literal argument types must match. The
    /// comma form (`obj::"OpenFormSync"(…)`) is exempt — its trailing
    /// parameters default from the RAD design (R21).
    fn check_open_form_signature(&mut self, method: &str, args: &[Expr], span: cobolt_lexer::Span) {
        #[derive(Clone, Copy, PartialEq)]
        enum P {
            Text,
            Number,
            Bool,
        }
        let m = method.trim().to_ascii_uppercase();
        // The receiver each pair is written on — the error's example call
        // must name the receiver the developer actually types (051: the
        // standalone pair lives on a SideMenu control, not on `me`).
        let receiver = if m.starts_with("OPENSTANDALONEFORM") {
            "<side-menu-control>"
        } else {
            "me"
        };
        let (params, signature): (&[(&str, P)], &str) = match m.as_str() {
            "OPENFORMSYNC" => (
                &[
                    ("form id", P::Text),
                    ("formWindowState", P::Text),
                    ("x", P::Number),
                    ("y", P::Number),
                    ("width", P::Number),
                    ("height", P::Number),
                    ("modal", P::Bool),
                ],
                "\"OpenFormSync\" USING form-id windowState x y width height modal",
            ),
            "OPENFORMASYNC" => (
                &[
                    ("form id", P::Text),
                    ("formWindowState", P::Text),
                    ("x", P::Number),
                    ("y", P::Number),
                    ("width", P::Number),
                    ("height", P::Number),
                ],
                "\"OpenFormAsync\" USING form-id windowState x y width height",
            ),
            // 051 R24 — the SideMenu pair mirrors the me:: pair exactly:
            // same parameters, same Sync-only trailing modal.
            "OPENSTANDALONEFORMSYNC" => (
                &[
                    ("form id", P::Text),
                    ("formWindowState", P::Text),
                    ("x", P::Number),
                    ("y", P::Number),
                    ("width", P::Number),
                    ("height", P::Number),
                    ("modal", P::Bool),
                ],
                "\"OpenStandAloneFormSync\" USING form-id windowState x y width height modal",
            ),
            "OPENSTANDALONEFORMASYNC" => (
                &[
                    ("form id", P::Text),
                    ("formWindowState", P::Text),
                    ("x", P::Number),
                    ("y", P::Number),
                    ("width", P::Number),
                    ("height", P::Number),
                ],
                "\"OpenStandAloneFormAsync\" USING form-id windowState x y width height",
            ),
            _ => return,
        };
        if args.len() != params.len() {
            self.error(
                format!(
                    "INVOKE {m}: the COBOL-standard (space-separated) form requires all {} \
                     parameters — got {}. Expected: INVOKE {receiver} {signature}. \
                     (Use the comma form {receiver}::\"{}\"(…) for optional parameters.)",
                    params.len(),
                    args.len(),
                    method.trim(),
                ),
                span,
            );
            return;
        }
        for (i, (name, kind)) in params.iter().enumerate() {
            let bad = match (&args[i], kind) {
                (Expr::Literal(Literal::String(_), _), P::Number) => true,
                (
                    Expr::Literal(
                        Literal::Integer(_)
                            | Literal::IntegerDigits(..)
                            | Literal::Float(_)
                            | Literal::Decimal(_, _),
                        _,
                    ),
                    P::Text,
                ) => true,
                _ => false,
            };
            if bad {
                self.error(
                    format!(
                        "INVOKE {m}: parameter {} ({name}) has the wrong type. \
                         Expected: INVOKE {receiver} {signature}.",
                        i + 1,
                    ),
                    args[i].span(),
                );
            }
        }
    }

    /// 049 R17 — `OpenFormSync`/`OpenFormAsync` may only target a form whose
    /// FormFormat allows the standalone path (`Standalone` | `Both`). Checked
    /// when the target is a literal and the project supplied its form map;
    /// a data-item target is dynamic and stays a runtime concern.
    fn check_open_form_target(&mut self, method: &str, args: &[Expr]) {
        let m = method.trim().to_ascii_uppercase();
        // 051 R26 — the SideMenu's OpenStandAloneForm* pair opens the same
        // standalone path, so it obeys the same gate.
        if !matches!(
            m.as_str(),
            "OPENFORMSYNC" | "OPENFORMASYNC" | "OPENSTANDALONEFORMSYNC" | "OPENSTANDALONEFORMASYNC"
        ) {
            return;
        }
        let Some(formats) = self.form_formats else {
            return;
        };
        let Some(Expr::Literal(Literal::String(id), span)) = args.first() else {
            return;
        };
        let key = id.trim().to_ascii_uppercase();
        if let Some(fmt) = formats.get(&key) {
            if !fmt.allows_standalone() {
                self.error(
                    format!(
                        "INVOKE {}: form '{}' has FormFormat Embedded — it is loaded \
                         from a sidebar menu, never opened as a window. Set its \
                         FormFormat to Standalone or Both to open it with {} (049 R17).",
                        method.trim(),
                        id.trim(),
                        method.trim(),
                    ),
                    *span,
                );
            }
        }
    }

    /// 049 R33/R34 — a BARE property on a `me`/`super` receiver must belong to
    /// the universal form surface, at any `super` chain depth. Method calls
    /// (parens) pass through: form-specific procedures dispatch at run time
    /// (R34). Only literal `ME`/`SUPER` roots are statically knowable — a
    /// chain rooted in a control or the form's own name is not touched.
    fn check_form_receiver_property(&mut self, expr: &Expr) {
        // Flatten: walk to the root, collecting (member, parens, span)
        // outermost-last.
        let mut segs: Vec<(&str, bool, cobolt_lexer::Span)> = Vec::new();
        let mut cur = expr;
        while let Expr::Member {
            recv,
            member,
            parens,
            span,
            ..
        } = cur
        {
            segs.push((member.as_str(), *parens, *span));
            cur = recv;
        }
        let Expr::Identifier(root, _) = cur else {
            return;
        };
        let root_upper = root.trim().to_ascii_uppercase();
        if root_upper != "ME" && root_upper != "SUPER" {
            return;
        }
        segs.reverse(); // root-to-leaf
        // Strip the leading `super` SEGMENTS of a super chain (R31) — each is
        // one step up the loader chain, not a property.
        let mut i = 0;
        if root_upper == "SUPER" {
            while segs
                .get(i)
                .map(|(m, parens, _)| !parens && m.eq_ignore_ascii_case("SUPER"))
                .unwrap_or(false)
            {
                i += 1;
            }
        }
        // The checkable shape is exactly one remaining BARE property. A
        // parens tail is a method (runtime dispatch, R34); a deeper path is
        // not a form property and is left to the runtime as well.
        match &segs[i..] {
            [(name, false, span)] => {
                if !UNIVERSAL_FORM_PROPS
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(name))
                {
                    let receiver = if root_upper == "SUPER" {
                        "super"
                    } else {
                        "me"
                    };
                    self.error(
                        format!(
                            "'{receiver}::{name}' is not a form property. The form \
                             surface is: {}. Form-specific procedures are called \
                             with parentheses ({receiver}::\"{name}\"(…)) and \
                             dispatch at run time (049 R33/R34).",
                            UNIVERSAL_FORM_PROPS.join(", ")
                        ),
                        *span,
                    );
                }
            }
            _ => {}
        }
    }

    /// Warn when a member-access chain used as a **receiving field** ends in a
    /// method call (`MOVE name TO obj::UpperCase()`): a method-call result is an
    /// rvalue, not a receiving field (spec 011). An empty-parens tail is
    /// unambiguously a call; an indexed tail (`Items(4)`) carries arguments and
    /// remains assignable, so it is not flagged here.
    fn check_receiving(&mut self, expr: &Expr) {
        if let Expr::Member {
            parens: true,
            args,
            span,
            ..
        } = expr
        {
            if args.is_empty() {
                self.warn("a method-call result is not a receiving field", *span);
            }
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(name, span) => {
                // Skip warnings for CoBolt runtime-injected identifiers.
                // These are declared by the code generator or provided by the
                // runtime as special registers; they will not appear in the
                // user-authored DATA DIVISION.
                let is_runtime = name.starts_with("COBOL-")
                    || name.starts_with("COBOLT-")
                    || matches!(
                        name.as_str(),
                        "RETURN-CODE" | "WHEN-COMPILED" | "LINAGE-COUNTER" | "FORM-NAME"
                    );
                if !is_runtime && !self.symbols.has_data_item(name) && name.len() > 1 {
                    self.warn(
                        format!("identifier '{name}' is not declared in DATA DIVISION"),
                        *span,
                    );
                }
            }
            // Qualified: `A OF B`  — the `of` part is the qualifying group expr
            Expr::Qualified { of, .. } => {
                self.resolve_expr(of);
            }
            // Subscript: `TABLE-ITEM(index)`
            Expr::Subscript { base, indices, .. } => {
                self.resolve_expr(base);
                for idx in indices {
                    self.resolve_expr(idx);
                }
            }
            // Reference modification: `base(start:length)`
            Expr::RefMod {
                base,
                start,
                length,
                ..
            } => {
                self.resolve_expr(base);
                self.resolve_expr(start);
                if let Some(l) = length {
                    self.resolve_expr(l);
                }
            }
            Expr::Unary { operand, .. } => self.resolve_expr(operand),
            // Binary arithmetic
            Expr::Arithmetic { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Expr::FunctionCall { name, args, span } => {
                // An intrinsic the runtime does not implement used to return 0
                // and say nothing, so a program computed a confidently wrong
                // answer from a typo. Naming it at compile time is the whole
                // point — a misspelling is far likelier than a genuinely
                // unknown function, so the message suggests the nearest real
                // name when there is one close enough to be worth suggesting.
                if !cobolt_ast::intrinsics::is_intrinsic(name) {
                    let msg = match cobolt_ast::intrinsics::closest_intrinsic(name) {
                        Some(near) => format!(
                            "'{name}' is not an intrinsic function RustCOBOL implements — did you mean FUNCTION {near}?"
                        ),
                        None => format!(
                            "'{name}' is not an intrinsic function RustCOBOL implements. \
                             The implemented set is listed in docs/cobol85-supported-syntax-en.md."
                        ),
                    };
                    self.error(msg, *span);
                }
                for a in args {
                    self.resolve_expr(a);
                }
            }
            // Member-access chain (`obj::Caption`, `Grid::Rows(I)::Value`): the
            // root object is a form control, not a DATA DIVISION item, so the
            // receiver chain is not name-resolved here — only the subscript /
            // call argument expressions are. A `me`/`super` root gets the 049
            // R33 universal-surface check first.
            Expr::Member { args, .. } => {
                self.check_form_receiver_property(expr);
                for a in args {
                    self.resolve_expr(a);
                }
            }
            // Literals and figurative constants need no resolution.
            Expr::Literal(..) => {}
            // `ALL` in a subscript position is not a name — it is the reserved
            // word standing for every occurrence of the table it subscripts.
            // The table itself is resolved through the enclosing `Subscript`'s
            // base, so there is nothing to look up here.
            Expr::AllSubscript(..) => {}
        }
    }

    fn resolve_condition(&mut self, cond: &Condition) {
        match cond {
            Condition::Comparison { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Condition::And(a, b, _) => {
                self.resolve_condition(a);
                self.resolve_condition(b);
            }
            Condition::Or(a, b, _) => {
                self.resolve_condition(a);
                self.resolve_condition(b);
            }
            Condition::Not(inner, _) => self.resolve_condition(inner),
            Condition::ClassTest { expr, .. } => self.resolve_expr(expr),
            // The class name is declared in SPECIAL-NAMES, not in the DATA
            // DIVISION, so only the operand is a symbol to resolve.
            Condition::UserClassTest { expr, .. } => self.resolve_expr(expr),
            Condition::SignTest { expr, .. } => self.resolve_expr(expr),
            Condition::ConditionName(name, span) => {
                if !self.symbols.has_data_item(name) {
                    self.warn(format!("condition name '{name}' is not declared"), *span);
                }
            }
            Condition::NameOrAbbrev {
                subject,
                name,
                span,
                ..
            } => {
                self.resolve_expr(subject);
                // `name` is either a condition-name or a data item — either is a
                // declared symbol; warn only if it is neither.
                if !self.symbols.has_data_item(name) {
                    self.warn(format!("'{name}' is not declared"), *span);
                }
            }
            // `IF FIRSTZ (1)` / `IF A OF IF-D32` — a condition-name reached
            // through a reference. Resolving the reference checks the name and
            // its qualifiers the same way any other reference is checked.
            Condition::ConditionRef(expr, _) => self.resolve_expr(expr),
        }
    }

    /// A `PERFORM`/`GO TO` target must be a paragraph or section of the SAME
    /// program. Procedure names are never visible outside the program that
    /// declares them — COBOL-85 has no `GLOBAL` for them — so a name that is not
    /// here cannot be reached from here, whatever else the compilation unit
    /// contains. There is no run in which it resolves, which makes it an error
    /// and not a warning.
    fn check_procedure(&mut self, name: &str, span: cobolt_lexer::Span) {
        if !self.symbols.has_procedure(name) {
            self.error(
                format!(
                    "'{name}' is not a paragraph or section of this program. \
                     PERFORM and GO TO reach only procedures declared in the \
                     same program; a paragraph of that name elsewhere in the \
                     compilation unit is not visible here."
                ),
                span,
            );
        }
    }
}
