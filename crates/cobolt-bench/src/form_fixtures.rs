// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T34: the form fixtures of `cobolt-bench pixel-parity`.** These are projects
//! made for the pairs mode, beside the five reference projects. At M2 no reference project takes the
//! WebAssembly path yet: PowerSpatial's Common Code has an INDEXED disk file (M3), and the other
//! four hold canvases, graphics objects, AWS controls or a FileDropZone (M6). A fixture holds only
//! controls the runner serves, so the pairs mode always compares at least one pair.
//!
//! Each fixture is written here, in code, as the runner twins write theirs. It has its forms, the
//! project file, and a script per form. The project has forms only (no main program), so the Build
//! generates every form's COBOL itself, from the `.cfrm` the fixture saves.
//!
//! T35: the checks of `.github/workflows/desktop-wasm.yml` include this file by path
//! (`cobolt-runner/examples/desktop_wasm_ci.rs`). There a fixture is built by `rcrun` with no Rust
//! toolchain on the machine, and its module runs on every platform's runner.

use std::path::{Path, PathBuf};

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

/// One fixture project.
pub(crate) struct FormFixture {
    pub name: &'static str,
    /// `(file relative to the project, design)`, the main form first.
    pub forms: Vec<(String, Form)>,
    /// `(form file's stem, form ID, steps)`.
    pub scripts: Vec<(String, String, Vec<Value>)>,
    /// `(control::property, value)`: what the scripts' reads must see, under both programs. Equal
    /// runs alone could both have done nothing; these say the handlers ran.
    pub expect: Vec<(String, String)>,
}

fn handler(statements: &str) -> String {
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{statements}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, PropValue)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    form.controls.push(c);
}

fn bind(form: &mut Form, id: &str, event: &str, statements: &str) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(statements) });
}

/// `ParityControls`: one form of the controls the runner serves, drawn and written by COBOL: text,
/// a check box and a switch, two radio buttons, a progress bar, a gauge, a knob and a slider whose
/// values COBOL sets, a list COBOL fills and counts, a bar chart as designed, and a timer.
fn parity_controls() -> FormFixture {
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new("PARITY-CONTROLS", "Parity controls", 720, 460);
    form.main_form = true;
    control(&mut form, "LBL-TITLE", ControlType::Label, (20, 16, 320, 24), &[("Caption", s("-"))]);
    control(&mut form, "TXT-NAME", ControlType::TextBox, (20, 48, 220, 28), &[("Text", s(""))]);
    control(&mut form, "BTN-GREET", ControlType::Button, (256, 48, 120, 28), &[("Caption", s("Greet"))]);
    control(&mut form, "CHK-ON", ControlType::CheckBox, (20, 92, 120, 24), &[("Caption", s("On"))]);
    control(&mut form, "SWT-ON", ControlType::Switch, (150, 90, 52, 28), &[]);
    control(&mut form, "BTN-TOGGLE", ControlType::Button, (256, 90, 120, 28), &[("Caption", s("Toggle"))]);
    control(&mut form, "RAD-A", ControlType::RadioButton, (400, 92, 120, 22), &[("Caption", s("First"))]);
    control(&mut form, "RAD-B", ControlType::RadioButton, (530, 92, 120, 22), &[("Caption", s("Second"))]);
    control(&mut form, "PRG-1", ControlType::ProgressBar, (20, 136, 220, 22), &[]);
    control(&mut form, "SLD-1", ControlType::Slider, (20, 170, 220, 36), &[]);
    control(&mut form, "GAU-1", ControlType::Gauge, (256, 130, 140, 90), &[]);
    control(&mut form, "KNB-1", ControlType::Knob, (410, 130, 90, 90), &[]);
    control(&mut form, "BTN-STEP", ControlType::Button, (520, 150, 120, 28), &[("Caption", s("Step"))]);
    control(&mut form, "LST-NAMES", ControlType::ListBox, (20, 230, 220, 140), &[]);
    control(&mut form, "BTN-ADD", ControlType::Button, (256, 230, 120, 28), &[("Caption", s("Add"))]);
    control(&mut form, "LBL-COUNT", ControlType::Label, (256, 270, 120, 24), &[("Caption", s("0"))]);
    control(&mut form, "BAR-1", ControlType::BarChart, (400, 230, 300, 160), &[]);
    control(&mut form, "BTN-START", ControlType::Button, (20, 390, 120, 28), &[("Caption", s("Start"))]);
    control(&mut form, "LBL-TICKS", ControlType::Label, (150, 392, 200, 24), &[("Caption", s("-"))]);
    control(&mut form, "TMR-1", ControlType::Timer, (680, 420, 24, 24), &[("Interval", PropValue::Int(100)), ("Enabled", PropValue::Bool(false))]);
    bind(&mut form, "BTN-GREET", "onClick", "           MOVE TXT-NAME::Text TO LBL-TITLE::Caption");
    bind(
        &mut form,
        "BTN-TOGGLE",
        "onClick",
        "           IF CHK-ON::Checked = 1\n\
         \x20              MOVE 1 TO SWT-ON::Checked\n\
         \x20          ELSE\n\
         \x20              MOVE 0 TO SWT-ON::Checked\n\
         \x20          END-IF",
    );
    bind(
        &mut form,
        "BTN-STEP",
        "onClick",
        "           MOVE 40 TO PRG-1::Value\n\
         \x20          MOVE 80 TO SLD-1::Value\n\
         \x20          MOVE 65 TO GAU-1::Value\n\
         \x20          MOVE 30 TO KNB-1::Value",
    );
    bind(
        &mut form,
        "BTN-ADD",
        "onClick",
        "           LST-NAMES::AddItem( TXT-NAME::Text )\n\
         \x20          MOVE LST-NAMES::GetCount() TO LBL-COUNT::Caption",
    );
    bind(&mut form, "BTN-START", "onClick", "           MOVE 1 TO TMR-1::Enabled");
    bind(
        &mut form,
        "TMR-1",
        "onTick",
        "           MOVE \"ticked\" TO LBL-TICKS::Caption\n\
         \x20          MOVE 0 TO TMR-1::Enabled",
    );

    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let set_name = |v: &str| json!({"set": {"control": "TXT-NAME", "property": "Text", "value": v}});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let steps = vec![
        json!({"wait_ms": 200}),
        dump(),
        shot("00-open"),
        set_name("Ann"),
        click("BTN-GREET"),
        json!({"read": {"control": "LBL-TITLE", "property": "Caption"}}),
        dump(),
        shot("01-greet"),
        json!({"click": {"control": "CHK-ON"}}),
        click("BTN-TOGGLE"),
        json!({"click": {"control": "RAD-B"}}),
        dump(),
        shot("02-toggle"),
        click("BTN-STEP"),
        dump(),
        shot("03-step"),
        click("BTN-ADD"),
        set_name("Bob"),
        click("BTN-ADD"),
        json!({"read": {"control": "LBL-COUNT", "property": "Caption"}}),
        dump(),
        shot("04-add"),
        click("BTN-START"),
        json!({"wait_ms": 400}),
        json!({"read": {"control": "LBL-TICKS", "property": "Caption"}}),
        dump(),
        shot("05-timer"),
    ];
    FormFixture {
        name: "ParityControls",
        forms: vec![("forms/parity-controls.cfrm".into(), form)],
        scripts: vec![("parity-controls".into(), "PARITY-CONTROLS".into(), steps)],
        expect: [("LBL-TITLE::Caption", "Ann"), ("LBL-COUNT::Caption", "2"), ("LBL-TICKS::Caption", "ticked")]
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

/// Every fixture, in order.
pub(crate) fn all() -> Vec<FormFixture> {
    vec![parity_controls()]
}

/// Every `read` and `dump` of a run's report, in order: what it was, and what it saw.
pub(crate) fn reads_of(steps: &[Value]) -> Vec<(String, Value)> {
    steps
        .iter()
        .filter_map(|st| {
            let step = st.get("step").and_then(Value::as_u64).unwrap_or(0);
            if let Some(what) = st.get("read").and_then(Value::as_str) {
                Some((format!("step {step}: read {what}"), st.get("value").cloned().unwrap_or(Value::Null)))
            } else if st.get("dump").is_some() {
                Some((format!("step {step}: dump"), st.get("state").cloned().unwrap_or(Value::Null)))
            } else {
                None
            }
        })
        .collect()
}

/// What `steps` (a run's report) missed of `expect`: each `(control::property, value)` must be the
/// value of the run's last read of that control and property.
pub(crate) fn missed_expectations(steps: &[Value], expect: &[(String, String)]) -> Vec<String> {
    expect
        .iter()
        .filter_map(|(what, want)| {
            let got = steps.iter().rev().find(|st| st.get("read").and_then(Value::as_str) == Some(what.as_str()));
            match got.map(|st| st.get("value").cloned().unwrap_or(Value::Null)) {
                Some(Value::String(s)) if s == *want => None,
                Some(v) => Some(format!("{what} is {v}, expected {want:?}")),
                None => Some(format!("{what} was never read, expected {want:?}")),
            }
        })
        .collect()
}

/// Writes `fixture`'s forms and project file under `to`; the project file's path.
pub(crate) fn lay_out(fixture: &FormFixture, to: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to.join("forms")).map_err(|e| format!("{}: {e}", to.display()))?;
    for (rel, form) in &fixture.forms {
        cobolt_forms::save_form(form, &to.join(rel)).map_err(|e| format!("{}/{rel}: {e}", fixture.name))?;
    }
    let bin = fixture.name.to_ascii_lowercase();
    let list = fixture.forms.iter().map(|(rel, _)| format!("\"{rel}\"")).collect::<Vec<_>>().join(", ");
    let manifest = to.join(format!("{bin}.project.toml"));
    std::fs::write(&manifest, format!("[project]\nname = \"{bin}\"\nversion = \"0.1.0\"\nmain = \"\"\n\n[files]\nforms = [{list}]\n"))
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    Ok(manifest)
}
