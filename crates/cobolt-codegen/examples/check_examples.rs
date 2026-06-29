// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Coverage guard (spec 004): for each per-control example project, load its
//! `.cfrm` and confirm the subject control has one event binding per
//! `supported_events` and one property button per default property (plus the
//! geometry + PlayAnimation buttons). Reports actual counts; exits non-zero on
//! any gap. Run from the repo root:
//!   cargo run -p cobolt-codegen --example check_examples

use cobolt_forms::{load_form, Control, ControlType};
use std::path::Path;

fn all_controls() -> Vec<ControlType> {
    use ControlType::*;
    vec![
        Button,
        Label,
        TextBox,
        CheckBox,
        RadioButton,
        ComboBox,
        ListBox,
        NumericUpDown,
        DateTimePicker,
        GroupBox,
        Panel,
        TabControl,
        Splitter,
        DataGrid,
        TreeView,
        PictureBox,
        Animator,
        ProgressBar,
        Slider,
        Line,
        Shape,
        MenuBar,
        ToolBar,
        StatusBar,
        Timer,
        AgentObject,
        RestClient,
        SqlDatabase,
        BarChart,
        LineChart,
        PieChart,
        AreaChart,
        ScatterChart,
        DonutChart,
    ]
}

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

fn main() {
    let mut gaps = 0;
    for ct in all_controls() {
        let folder = kebab(ct.as_str());
        let path = format!("examples/{folder}/forms/{folder}.cfrm");
        let form = match load_form(Path::new(&path)) {
            Ok(f) => f,
            Err(e) => {
                println!("GAP {folder}: cannot load {path}: {e}");
                gaps += 1;
                continue;
            }
        };
        let subject = form.controls.iter().find(|c| c.id == "SUBJ");
        let buttons = form
            .controls
            .iter()
            // exclude the subject control (which is itself a Button in the
            // `button` example) — count only the property buttons.
            .filter(|c| c.control_type == ControlType::Button && c.id != "SUBJ")
            .count();
        // Expected buttons = props + geometry(4) + PlayAnimation(1).
        let expected_props = Control::new("X", ct.clone(), 0, 0).properties.len();
        let expected_btns = expected_props + 5;
        let want_events = ct.supported_events().len();
        let got_events = subject.map(|s| s.events.len()).unwrap_or(0);

        let ev_ok = got_events == want_events;
        let btn_ok = buttons == expected_btns;
        if !ev_ok || !btn_ok {
            gaps += 1;
        }
        println!(
            "{} {folder}: events {}/{}  buttons {}/{}",
            if ev_ok && btn_ok { "ok " } else { "GAP" },
            got_events,
            want_events,
            buttons,
            expected_btns
        );
    }
    println!("\n{} gap(s)", gaps);
    if gaps > 0 {
        std::process::exit(1);
    }
}
