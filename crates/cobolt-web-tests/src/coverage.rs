// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What each control type's tests cover** (WASM-015 T8, R4): one row per `ControlType::ALL` entry, with the
//! decision for it. A control whose property round-trip exists in `tests/controls/` (the desktop) is
//! `CoveredNow`; so are the class A/B controls the desktop families lacked (they get a file in T9 and a web
//! round-trip in `controls_web`). A class C control runs through the backend and its round-trip is the
//! backend-call test of WASM-005: `KnownGap` with that reason. The table is in the `builtin_support` pattern:
//! a control added to `ControlType::ALL` without a row fails the test that reads it.
//!
//! The spec counted 62 control types and 27 uncovered; the toolbox now has 71 and 28 (the 7 of class A/B,
//! 20 of class C and Authentication, which WASM-009 added). The decisions are the plan's working assumption
//! (Q-new-2), applied to the real list.

/// What is decided for one control type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Covered by a round-trip test now.
    CoveredNow,
    /// Not covered here, with why.
    KnownGap(&'static str),
}

/// The reason every class C row gives.
pub const CLASS_C: &str = "runs through the backend; its round-trip is WASM-005's backend-call test";

/// One row per `ControlType::ALL` entry: (name, class letter, decision).
pub const COVERAGE: &[(&str, char, Decision)] = &[
    ("Button", 'A', Decision::CoveredNow),
    ("TextBox", 'A', Decision::CoveredNow),
    ("Label", 'A', Decision::CoveredNow),
    ("Hyperlink", 'A', Decision::CoveredNow),
    ("CheckBox", 'A', Decision::CoveredNow),
    ("RadioButton", 'A', Decision::CoveredNow),
    ("ListBox", 'A', Decision::CoveredNow),
    ("ComboBox", 'A', Decision::CoveredNow),
    ("GroupBox", 'A', Decision::CoveredNow),
    ("Panel", 'A', Decision::CoveredNow),
    ("TabControl", 'A', Decision::CoveredNow),
    ("DataGrid", 'C', Decision::CoveredNow),
    ("PictureBox", 'A', Decision::CoveredNow),
    ("Animator", 'B', Decision::CoveredNow),
    ("ProgressBar", 'A', Decision::CoveredNow),
    ("MenuBar", 'A', Decision::CoveredNow),
    ("SideMenu", 'A', Decision::CoveredNow),
    ("ToolBar", 'A', Decision::CoveredNow),
    ("StatusBar", 'A', Decision::CoveredNow),
    ("Line", 'A', Decision::CoveredNow),
    ("DateTimePicker", 'A', Decision::CoveredNow),
    ("NumericUpDown", 'A', Decision::CoveredNow),
    ("TreeView", 'A', Decision::CoveredNow),
    ("Splitter", 'A', Decision::CoveredNow),
    ("Timer", 'A', Decision::CoveredNow),
    ("Shape", 'A', Decision::CoveredNow),
    ("AgentObject", 'C', Decision::CoveredNow),
    ("RestClient", 'C', Decision::CoveredNow),
    ("SqlDatabase", 'C', Decision::CoveredNow),
    ("IndexedFile", 'C', Decision::KnownGap(CLASS_C)),
    ("KnowledgeBase", 'C', Decision::KnownGap(CLASS_C)),
    ("Slider", 'A', Decision::CoveredNow),
    ("BarChart", 'B', Decision::CoveredNow),
    ("LineChart", 'B', Decision::CoveredNow),
    ("PieChart", 'B', Decision::CoveredNow),
    ("AreaChart", 'B', Decision::CoveredNow),
    ("ScatterChart", 'B', Decision::CoveredNow),
    ("DonutChart", 'B', Decision::CoveredNow),
    ("RadarChart", 'B', Decision::CoveredNow),
    ("Knob", 'B', Decision::CoveredNow),
    ("Gauge", 'B', Decision::CoveredNow),
    ("Switch", 'A', Decision::CoveredNow),
    ("FileDropZone", 'C', Decision::KnownGap(CLASS_C)),
    ("Maps", 'B', Decision::CoveredNow),
    ("WebSearch", 'C', Decision::KnownGap(CLASS_C)),
    ("Snackbar", 'A', Decision::CoveredNow),
    ("Viewer", 'B', Decision::CoveredNow),
    ("Authentication", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsMcp", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsLambda", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsKnowledgeBase", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsAgentCore", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsAgentMemory", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsS3Tables", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsGlue", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsDynamoDB", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsS3", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsS3Vectors", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsRekognition", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsPolly", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsComprehend", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsTextract", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsEC2", 'C', Decision::KnownGap(CLASS_C)),
    ("AwsCognito", 'C', Decision::KnownGap(CLASS_C)),
    ("Canvas2D", 'B', Decision::CoveredNow),
    ("Canvas3D", 'B', Decision::CoveredNow),
    ("ImageBuffer", 'E', Decision::CoveredNow),
    ("Plot", 'E', Decision::CoveredNow),
    ("Scene3D", 'E', Decision::CoveredNow),
    ("Physics2D", 'E', Decision::CoveredNow),
    ("Physics3D", 'E', Decision::CoveredNow),
];

/// The names of `all` that have no row, and the rows whose name is not in `all`.
pub fn mismatches<'a>(rows: &'a [(&'a str, char, Decision)], all: &[&'a str]) -> (Vec<&'a str>, Vec<&'a str>) {
    let missing = all.iter().copied().filter(|n| !rows.iter().any(|r| r.0 == *n)).collect();
    let extra = rows.iter().map(|r| r.0).filter(|n| !all.contains(n)).collect();
    (missing, extra)
}
