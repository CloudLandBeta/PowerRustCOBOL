// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Throwaway dev aid (spec 004): print every toolbox control's supported events
//! and default property keys — the authoritative checklist for authoring the
//! per-control test example projects under `examples/`.
//!
//! Usage: cargo run -p cobolt-forms --example list_controls

use cobolt_forms::{Control, ControlType};

fn main() {
    // The toolbox control set (matches crates/cobolt-ide/.../toolbox.rs TOOLS),
    // ModalWindow excluded (removed).
    let controls = [
        ControlType::Button,
        ControlType::Label,
        ControlType::TextBox,
        ControlType::CheckBox,
        ControlType::RadioButton,
        ControlType::ComboBox,
        ControlType::ListBox,
        ControlType::NumericUpDown,
        ControlType::DateTimePicker,
        ControlType::GroupBox,
        ControlType::Panel,
        ControlType::TabControl,
        ControlType::Splitter,
        ControlType::DataGrid,
        ControlType::TreeView,
        ControlType::PictureBox,
        ControlType::Animator,
        ControlType::ProgressBar,
        ControlType::Slider,
        ControlType::Line,
        ControlType::Shape,
        ControlType::MenuBar,
        ControlType::ToolBar,
        ControlType::StatusBar,
        ControlType::Timer,
        ControlType::AgentObject,
        ControlType::RestClient,
        ControlType::SqlDatabase,
        ControlType::BarChart,
        ControlType::LineChart,
        ControlType::PieChart,
        ControlType::AreaChart,
        ControlType::ScatterChart,
        ControlType::DonutChart,
    ];

    println!("# Toolbox control metadata ({} controls)\n", controls.len());
    for ct in &controls {
        let name = ct.as_str();
        let sample = Control::new(format!("{name}-1"), ct.clone(), 0, 0);
        let props: Vec<&str> = sample.properties.keys().map(|s| s.as_str()).collect();
        let events: Vec<&str> = ct.supported_events().to_vec();
        println!("## {name}");
        println!("- events ({}): {}", events.len(), events.join(", "));
        println!("- props  ({}): {}", props.len(), props.join(", "));
        println!();
    }
}
