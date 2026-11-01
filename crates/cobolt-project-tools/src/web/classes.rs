// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a browser cannot have** (WASM-004 R5, AC4): the class D controls and
//! operations of a web or mobile form, one refusal each, in the build gate's
//! `file:line: construct — alternative` format. The class and the alternative
//! come from the one classification (`cobolt_forms::web_class`); Check and
//! Build both reach this through the generator's form loading, so the developer
//! sees it before building.
//!
//! - a control of a class D type (a plug-in control) — at its `<Control>` line
//!   in the form file;
//! - a ToolBar button whose action is class D (`run-app`, `open-terminal`,
//!   `print`, `share`) — at the ToolBar's line;
//! - a handler's call of a class D method (a Viewer's `Print`, `Share`) — at the
//!   developer's own handler line.

use cobolt_wasm::module::SourceLine;
use cobolt_compiler::ProjectKind;
use cobolt_forms::web_class::{alternative_of, class_of, unavailable_operation, Class};
use cobolt_forms::model::{Control, ControlType, Form};

use super::{WebDiagnostic, WebDiagnosticKind};

/// The line (from 1) of control `id`'s element in the form file `cfrm`, or 0.
fn control_line(cfrm: &str, id: &str) -> usize {
    let attr = format!("id=\"{id}\"");
    cfrm.lines()
        .position(|l| {
            let t = l.trim_start();
            t.starts_with("<Control") && t.contains(&attr)
        })
        .map_or(0, |i| i + 1)
}

fn at(rel: &str, line: usize) -> String {
    if line == 0 {
        rel.to_owned()
    } else {
        format!("{rel}:{line}")
    }
}

fn refusal(location: String, construct: String, alternative: &str) -> WebDiagnostic {
    WebDiagnostic {
        location,
        kind: WebDiagnosticKind::UnavailableOnWeb,
        message: format!("{construct} — {alternative}"),
        subject: construct,
        alternative: alternative.to_owned(),
    }
}

fn all_controls<'a>(cs: &'a [Control], out: &mut Vec<&'a Control>) {
    for c in cs {
        out.push(c);
        all_controls(&c.children, out);
    }
}

/// The class D controls of `form` and the class D ToolBar actions, each at its
/// line in `cfrm` (the form file `rel`). None on a Desktop project's form.
pub fn unavailable_controls(kind: ProjectKind, rel: &str, cfrm: &str, form: &Form) -> Vec<WebDiagnostic> {
    if kind.is_desktop() {
        return Vec::new();
    }
    let mut controls = Vec::new();
    all_controls(&form.controls, &mut controls);
    let mut out = Vec::new();
    for c in controls {
        let location = at(rel, control_line(cfrm, &c.id));
        if class_of(&c.control_type) == Class::D {
            let alternative = alternative_of(&c.control_type).unwrap_or_default();
            out.push(refusal(location.clone(), format!("{} ({})", c.id, c.control_type.as_str()), alternative));
        }
        if c.control_type == ControlType::ToolBar {
            let def = cobolt_forms::toolbar::ToolbarDef::from_control(c);
            for (_, b) in def.buttons() {
                let action = b.action();
                if let Some(e) = unavailable_operation(&ControlType::ToolBar, action.verb()) {
                    out.push(refusal(location.clone(), format!("{} button `{}` action {}", c.id, b.id, action.to_action_string()), e.alternative));
                }
            }
        }
    }
    out
}

/// The controls of `form` a screen reader must name and cannot (WASM-004 T34, R10; operator, 2026-10-31):
/// no `AccessibleLabel`, no caption of its own, no Label just before it in tab order and no Tooltip
/// (`cobolt_forms::accessible`). One warning each, at its line in `cfrm`, naming the form and the control.
/// None on a Desktop project's form.
pub fn unnamed_controls(kind: ProjectKind, rel: &str, cfrm: &str, form: &Form) -> Vec<WebDiagnostic> {
    use cobolt_forms::accessible::{designed, name_source, needs_name, NameSource};
    if kind.is_desktop() {
        return Vec::new();
    }
    let mut controls = Vec::new();
    all_controls(&form.controls, &mut controls);
    let ranks: Vec<usize> = controls.iter().map(|c| form.layer_rank(form.layer_of(&c.id))).collect();
    let get = designed(&controls);
    (0..controls.len())
        .filter(|&i| needs_name(&controls[i].control_type))
        .filter(|&i| name_source(&controls, i, &|j| ranks[j], &get) == NameSource::None)
        .map(|i| {
            let c = controls[i];
            let subject = format!("{} ▸ {} ({})", form.name, c.id, c.control_type.as_str());
            WebDiagnostic {
                location: at(rel, control_line(cfrm, &c.id)),
                kind: WebDiagnosticKind::NoAccessibleName,
                message: format!("{subject} has no accessible name: a screen reader cannot announce it. Set its AccessibleLabel, put a Label just before it in tab order, or give it a Tooltip"),
                subject,
                alternative: String::new(),
            }
        })
        .collect()
}

/// The handler calls of class D methods in `program` (`form`'s generated program),
/// each at the developer's handler line through `lines`. None on a Desktop project's form.
pub fn unavailable_calls(
    kind: ProjectKind,
    rel: &str,
    form: &Form,
    program: &cobolt_ast::program::Program,
    lines: &[SourceLine],
) -> Vec<WebDiagnostic> {
    if kind.is_desktop() {
        return Vec::new();
    }
    let mut controls = Vec::new();
    all_controls(&form.controls, &mut controls);
    cobolt_compiler::web_remote::class_d_call_sites(program, form)
        .into_iter()
        .filter_map(|s| {
            let c = controls.iter().find(|c| c.id.eq_ignore_ascii_case(&s.control))?;
            let e = unavailable_operation(&c.control_type, &s.method)?;
            let location = super::generator::location_in(rel, lines, s.line);
            Some(refusal(location, format!("{}::{}", c.id, e.operation.name()), e.alternative))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A form with a plug-in control and a Viewer whose button's handler prints it.
    const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PRINTS" title="Prints" width="640" height="480">
  <Control id="VWR-1" type="Viewer" x="10" y="10" w="300" h="200"></Control>
  <Control id="BTN-PRINT" type="Button" x="10" y="220" w="100" h="30">
    <Property name="Caption">Print</Property>
    <Event name="onClick" paragraph="BTN-PRINT--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           DISPLAY "printing".
           INVOKE VWR-1 "Print".
]]></Event>
  </Control>
  <Control id="PLG-1" type="acme:gizmo" x="10" y="260" w="100" h="30"></Control>
</Form>
"#;

    fn findings(rel: &str) -> Vec<WebDiagnostic> {
        let kind = rel.rsplit_once('.').and_then(|(_, e)| ProjectKind::of_form_extension(e)).unwrap();
        let form = cobolt_forms::xml::load_form_from_str(FORM).unwrap();
        let mut out = unavailable_controls(kind, rel, FORM, &form);
        let (src, map) = cobolt_codegen::generate_with_map(&form);
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let program = parsed.program.expect("the form's program parses");
        let lines: Vec<SourceLine> = (1..=src.lines().count() as u32)
            .filter_map(|g| map.resolve(g).map(|(site, l)| SourceLine { gen_line: g, site: site.display_path(&form.name), line: l, column: 0 }))
            .collect();
        out.extend(unavailable_calls(kind, rel, &form, &program, &lines));
        out
    }

    #[test]
    fn web_classes_a_plug_in_control_and_a_viewers_print_are_refused_at_their_lines() {
        let got = findings("webforms/prints.wcfrm");
        for d in &got {
            println!("{d}");
        }
        let text: Vec<String> = got.iter().map(|d| d.to_string()).collect();
        assert_eq!(got.len(), 2, "{text:#?}");
        assert_eq!(
            text[0],
            "webforms/prints.wcfrm:13: PLG-1 (acme:gizmo) — plug-in controls are not available on the web or on mobile: remove the control or use a built-in one"
        );
        // The INVOKE is the handler's fifth line (ENVIRONMENT, DATA, PROCEDURE, DISPLAY, INVOKE), in `onClick` of BTN-PRINT.
        assert!(text[1].starts_with("webforms/prints.wcfrm (") && text[1].ends_with(":5: VWR-1::Print — use the browser's own Print command, or SaveAsPdf and let the user print the file"), "{}", text[1]);
        assert!(text[1].contains("BTN-PRINT") && text[1].contains("onClick"), "{}", text[1]);
        assert!(got.iter().all(|d| d.kind == WebDiagnosticKind::UnavailableOnWeb && !d.alternative.is_empty()));
        assert!(findings("mobileforms/prints.mcfrm").len() == 2, "a mobile form too");
    }

    #[test]
    fn web_classes_the_same_form_on_the_desktop_gives_none() {
        let got = findings("forms/prints.cfrm");
        println!("desktop form: {} findings", got.len());
        assert!(got.is_empty(), "{got:?}");
    }

    /// WASM-004 T34 (operator, 2026-10-31): a control with no AccessibleLabel, no caption, no Label just before it
    /// in tab order and no Tooltip is a warning naming the form and the control, at its line; never on a desktop form.
    #[test]
    fn web_classes_a_control_a_screen_reader_cannot_name_is_a_warning_at_its_line() {
        let cfrm = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="ORDER" title="Order" width="640" height="480">
  <Control id="LBL-NAME" type="Label" x="10" y="10" w="80" h="24"><Property name="Caption">Name</Property></Control>
  <Control id="TXT-NAME" type="TextBox" x="100" y="10" w="160" h="28"></Control>
  <Control id="TXT-NOTE" type="TextBox" x="100" y="50" w="160" h="28"></Control>
  <Control id="SLD-QTY" type="Slider" x="100" y="90" w="160" h="36"><Property name="Tooltip">Quantity</Property></Control>
  <Control id="SWT-GIFT" type="Switch" x="100" y="130" w="80" h="32"><Property name="AccessibleLabel">Gift wrap</Property></Control>
  <Control id="BTN-OK" type="Button" x="100" y="170" w="100" h="32"><Property name="Caption">OK</Property></Control>
  <Control id="CH-1" type="BarChart" x="300" y="10" w="200" h="150"></Control>
</Form>
"#;
        let form = cobolt_forms::xml::load_form_from_str(cfrm).unwrap();
        let got = unnamed_controls(ProjectKind::Web, "webforms/order.wcfrm", cfrm, &form);
        for d in &got {
            println!("{d}");
        }
        assert_eq!(got.len(), 1, "only TXT-NOTE: {got:#?}");
        assert_eq!(got[0].location, "webforms/order.wcfrm:5");
        assert_eq!(got[0].kind, WebDiagnosticKind::NoAccessibleName);
        assert_eq!(got[0].subject, "ORDER ▸ TXT-NOTE (TextBox)");
        assert!(got[0].message.starts_with("ORDER ▸ TXT-NOTE (TextBox) has no accessible name"), "{}", got[0].message);
        assert_eq!(unnamed_controls(ProjectKind::Mobile, "mobileforms/order.mcfrm", cfrm, &form).len(), 1, "a mobile form too");
        assert!(unnamed_controls(ProjectKind::Desktop, "forms/order.cfrm", cfrm, &form).is_empty());
    }

    #[test]
    fn web_classes_a_toolbar_action_that_starts_a_program_is_refused_at_the_toolbar() {
        let mut def = cobolt_forms::toolbar::ToolbarDef::example();
        let first = def.groups[0].buttons[0].id.clone();
        def.groups[0].buttons[0].action = "run-app:notepad".into();
        let json = def.to_json().unwrap().replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
        let cfrm = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"TB\" title=\"TB\" width=\"640\" height=\"480\">\n  <Control id=\"TB-1\" type=\"ToolBar\" x=\"0\" y=\"0\" w=\"640\" h=\"40\"><Property name=\"{}\">{json}</Property></Control>\n</Form>\n",
            cobolt_forms::toolbar::TOOLBAR_DEF_PROP
        );
        let form = cobolt_forms::xml::load_form_from_str(&cfrm).unwrap();
        let got = unavailable_controls(ProjectKind::Web, "webforms/tb.wcfrm", &cfrm, &form);
        for d in &got {
            println!("{d}");
        }
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].location, "webforms/tb.wcfrm:3");
        assert!(got[0].message.starts_with(&format!("TB-1 button `{first}` action run-app:notepad — a browser cannot start a program")), "{}", got[0].message);
        assert!(unavailable_controls(ProjectKind::Desktop, "forms/tb.cfrm", &cfrm, &form).is_empty());
    }
}
