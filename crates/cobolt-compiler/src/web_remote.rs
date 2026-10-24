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
use cobolt_codegen::remote_controls::{CallSite, SiteArg};
use cobolt_forms::{Control, Form};

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
    fn a_numeric_literal_keeps_its_written_digits() {
        assert_eq!(decimal_text(314, 2), "3.14");
        assert_eq!(decimal_text(-5, 3), "-0.005");
        assert_eq!(decimal_text(42, 0), "42");
    }
}
