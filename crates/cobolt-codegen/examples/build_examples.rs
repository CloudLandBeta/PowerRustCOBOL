// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Builder (spec 004): emit the per-control test example projects under
//! `examples/<control>/`. For each toolbox control it constructs a form with the
//! subject control (one console DISPLAY per supported event), a button per
//! property (+ geometry + PlayAnimation) that changes it from COBOL, then writes
//! `forms/<c>.cfrm`, the codegen output `generated/<c>.cbl`, `cobolt.toml`, and a
//! per-project `README.md`.
//!
//! Usage:
//!   cargo run -p cobolt-codegen --example build_examples            # all controls
//!   cargo run -p cobolt-codegen --example build_examples -- label   # one control

use cobolt_forms::model::PropValue;
use cobolt_forms::{save_form, Control, ControlType, EventBinding, Form};
use std::fs;
use std::path::Path;

fn all_controls() -> Vec<ControlType> {
    use ControlType::*;
    vec![
        Button, Label, TextBox, CheckBox, RadioButton, ComboBox, ListBox, NumericUpDown,
        DateTimePicker, GroupBox, Panel, TabControl, Splitter, DataGrid, TreeView, PictureBox,
        Animator, ProgressBar, Slider, Line, Shape, MenuBar, ToolBar, StatusBar, Timer,
        AgentObject, RestClient, SqlDatabase, BarChart, LineChart, PieChart, AreaChart,
        ScatterChart, DonutChart,
    ]
}

/// kebab-case folder name, e.g. "DateTimePicker" -> "date-time-picker".
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// SCREAMING id token: "BackgroundColor" -> "BACKGROUNDCOLOR".
fn screaming(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '-' })
        .collect()
}

/// A type-appropriate runtime test value for a property, based on its default.
fn test_value(prop: &str, default: &PropValue) -> String {
    match default {
        PropValue::Bool(b) => if *b { "0".into() } else { "1".into() },
        PropValue::Int(n) => (n + 10).to_string(),
        PropValue::String(s) if s.starts_with('#') => {
            // colour: pick a contrasting value
            if prop.to_ascii_lowercase().contains("background") { "#0066CC".into() }
            else { "#CC3300".into() }
        }
        PropValue::String(_) => "TEST".into(),
    }
}

/// Full handler body (ENVIRONMENT..PROCEDURE) that DISPLAYs one line.
fn display_handler(text: &str) -> String {
    format!(
        "       ENVIRONMENT DIVISION.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      LINKAGE SECTION.\n\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20          DISPLAY \"{text}\".\n"
    )
}

/// Full handler body that sets one property on the subject and then DISPLAYs the
/// value read back with `GetProperty` — proving the set actually took effect (a
/// fixed "… set" message would prove nothing). USING is on its own line so no
/// line exceeds COBOL fixed-format column 72 (the build parses fixed format).
fn setprop_handler(subject: &str, prop: &str, value: &str) -> String {
    format!(
        "       ENVIRONMENT DIVISION.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      LINKAGE SECTION.\n\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20          INVOKE {subject} \"SetProperty\"\n\
         \x20              USING \"{prop}\" \"{value}\"\n\
         \x20          DISPLAY {subject}::GetProperty(\"{prop}\").\n"
    )
}

/// Full handler body that INVOKEs a one-arg method then DISPLAYs.
fn invoke_handler(subject: &str, method: &str, arg: &str, note: &str) -> String {
    format!(
        "       ENVIRONMENT DIVISION.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      LINKAGE SECTION.\n\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20          INVOKE {subject} \"{method}\"\n\
         \x20              USING \"{arg}\"\n\
         \x20          DISPLAY \"{note}\".\n"
    )
}

fn build_one(ct: &ControlType, examples_dir: &Path) {
    let name = ct.as_str();
    let folder = kebab(name);
    // Short, fixed ids keep codegen's WS fields (FORM-NAME, WS-<id>-TEXT) within
    // COBOL fixed-format column 72 regardless of how long the control name is.
    let subj_id = "SUBJ".to_string();
    let form_id = "TESTFORM".to_string();

    // Subject control + its event DISPLAY handlers.
    let mut subject = Control::new(&subj_id, ct.clone(), 20, 20);
    for ev in ct.supported_events() {
        let mut b = EventBinding::for_control(&subj_id, *ev);
        b.code = display_handler(&format!("{ev} working"));
        subject.events.push(b);
    }
    // Shorten any long default string values (e.g. RestClient BaseURL,
    // AgentObject endpoints) so codegen's `PIC X(n) VALUE '<default>'` WS field
    // stays within COBOL fixed-format column 72. The real value belongs in the
    // project's config when actually running the service control.
    for (_k, v) in subject.properties.iter_mut() {
        if let PropValue::String(s) = v {
            // Colours/enums are short (<= ~12); only long defaults (URLs, etc.)
            // get a short placeholder so the WS VALUE line stays within col 72.
            if s.len() > 12 {
                *s = "x".to_string();
            }
        }
    }

    // Snapshot the default property map before moving `subject` into the form.
    let props: Vec<(String, PropValue)> = subject
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // The button list: one per property, then geometry, then PlayAnimation.
    let mut buttons: Vec<(String, String)> = Vec::new(); // (caption, handler code)
    // Caption = property name (capped at 20 chars so codegen's WS caption field
    // stays within column 72). The handler still uses the full property name.
    let cap20 = |s: &str| if s.len() > 20 { s[..20].to_string() } else { s.to_string() };
    for (prop, def) in &props {
        let val = test_value(prop, def);
        buttons.push((cap20(prop), setprop_handler(&subj_id, prop, &val)));
    }
    for geo in ["X", "Y", "Width", "Height"] {
        buttons.push((
            geo.to_string(),
            setprop_handler(&subj_id, geo, if geo == "X" || geo == "Y" { "60" } else { "180" }),
        ));
    }
    buttons.push((
        "PlayAnimation".into(),
        invoke_handler(&subj_id, "PlayAnimation", "1", "PlayAnimation invoked"),
    ));

    // Lay out: subject top-left, button column on the right.
    let btn_x = 280;
    let btn_w = 320;
    let btn_h = 24;
    let step = 28;
    let height = (80 + (buttons.len() as i32) * step + 20).max(240) as u32;
    let mut form = Form::new(&form_id, &format!("{name} — control test"), 640, height);
    form.controls.push(subject);

    let mut y = 20;
    for (i, (caption, code)) in buttons.into_iter().enumerate() {
        // Short, unique id keeps the codegen WS caption field within column 72.
        let btn_id = format!("B{i:02}");
        let mut btn = Control::new(&btn_id, ControlType::Button, btn_x, y);
        btn.rect.w = btn_w;
        btn.rect.h = btn_h;
        btn.set_prop("Caption", PropValue::String(caption));
        let mut b = EventBinding::for_control(&btn_id, "onClick");
        b.code = code;
        btn.events.push(b);
        form.controls.push(btn);
        y += step;
    }

    // Write files.
    let proj_dir = examples_dir.join(&folder);
    fs::create_dir_all(proj_dir.join("forms")).unwrap();
    fs::create_dir_all(proj_dir.join("generated")).unwrap();

    let cfrm_rel = format!("forms/{folder}.cfrm");
    save_form(&form, &proj_dir.join(&cfrm_rel)).expect("save_form");

    let gen_rel = format!("generated/{folder}.cbl");
    let cbl = cobolt_codegen::generate(&form);
    fs::write(proj_dir.join(&gen_rel), cbl).unwrap();

    let toml = format!(
        "[project]\nname = \"{name} Test\"\nversion = \"1.0.0\"\nmain = \"{gen_rel}\"\n\n\
         [files]\nforms = [\"{cfrm_rel}\"]\ngenerated = [\"{gen_rel}\"]\n"
    );
    fs::write(proj_dir.join("cobolt.toml"), toml).unwrap();

    let readme = format!(
        "# {name} — control test example\n\n\
         Subject control: **{name}**. The form fires a console `DISPLAY \"<Event> working\"`\n\
         for every supported event, and each button changes one property of the control\n\
         from COBOL via `INVOKE {subj_id} \"SetProperty\" USING \"<name>\" \"<value>\"`.\n\n\
         ## Build\n\n```sh\nrcrun build examples/{folder}/cobolt.toml\n```\n\n\
         Or open `cobolt.toml` in the IDE and use **Build**. If the build reports an\n\
         error, read it, fix the form handler, regenerate, and rebuild — it must build\n\
         with zero errors.\n\n\
         > `generated/{folder}.cbl` is codegen output (regenerable from the form);\n\
         > the COBOL lives in the form's event handlers, not hand-edited.\n"
    );
    fs::write(proj_dir.join("README.md"), readme).unwrap();

    println!("built examples/{folder}/  ({} events, {} buttons)", ct.supported_events().len(), props.len() + 5);
}

fn main() {
    let filter = std::env::args().nth(1).map(|s| s.to_lowercase());
    let examples_dir = Path::new("examples");
    fs::create_dir_all(examples_dir).unwrap();

    let mut n = 0;
    for ct in all_controls() {
        if let Some(f) = &filter {
            if kebab(ct.as_str()) != *f && ct.as_str().to_lowercase() != *f {
                continue;
            }
        }
        build_one(&ct, examples_dir);
        n += 1;
    }
    println!("\n{n} project(s) written under examples/");
}
