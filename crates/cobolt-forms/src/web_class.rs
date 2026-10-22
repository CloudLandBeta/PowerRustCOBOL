// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The classification of every control for the web** (WASM-004 §2, R2) — one table.
//!
//! | Class | Meaning |
//! |---|---|
//! | **A** | becomes ordinary HTML/CSS, behaviour from the model |
//! | **B** | drawn by the desktop painter today; here an SVG, an `<img>` or a `<canvas>` inside its own element |
//! | **C** | works through the backend (WASM-005): disk, database, network, a secret |
//! | **D** | needs the local machine; not available |
//! | **E** | a client object that runs in the player with no backend |
//!
//! A control has one primary class; where some operations differ the note says which. A test
//! fails when [`ControlType::ALL`] and this table disagree (WASM-004 AC1), so a control added to
//! the model is classified or the build is red.
//!
//! The new controls of the series that the model does not carry yet — **WebLink**,
//! **Authentication**, **Canvas2D**, **Canvas3D** and the five E objects — reach the player as
//! `ControlType::Custom` (the model's own way of holding a type it does not know) and are
//! recognised here by name: [`player_kind_of`].
//!
//! It lives in this crate (WASM-001 T14, plan D-T14a) because the IDE's toolbox, the browser
//! player and the later Check must read *one* table, and `cobolt-forms` is the only crate all of
//! them depend on. Not behind a feature: the player uses this crate with `default-features = false`.

use crate::model::ControlType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    A,
    B,
    C,
    D,
    E,
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub control: &'static str,
    pub class: Class,
    /// What it becomes in the browser.
    pub becomes: &'static str,
    /// Operations that take another class than the primary one.
    pub note: &'static str,
}

const fn r(control: &'static str, class: Class, becomes: &'static str, note: &'static str) -> Row {
    Row { control, class, becomes, note }
}

use Class::*;

/// The table: every `ControlType::ALL` entry, once.
pub const TABLE: &[Row] = &[
    r("Button", A, "<button>", ""),
    r("TextBox", A, "<input> / <textarea>", ""),
    r("Label", A, "<span>", ""),
    r("CheckBox", A, "<input type=checkbox> in a <label>", ""),
    r("RadioButton", A, "<input type=radio> in a <label>", ""),
    r("ListBox", A, "<ul role=listbox>", ""),
    r("ComboBox", A, "<select>", ""),
    r("GroupBox", A, "<fieldset>", ""),
    r("Panel", A, "<div>", ""),
    r("TabControl", A, "tablist + panels (ARIA tabs)", ""),
    r("DataGrid", C, "<table> with virtualised rows", "A for in-memory Rows; C for IndexedFile and SQL bindings"),
    r("PictureBox", A, "<img> / <div> with background", ""),
    r("Animator", B, "<img> (the browser plays GIF/WebP/APNG)", "the control's richer API is carried by commands"),
    r("ProgressBar", A, "<progress>", ""),
    r("MenuBar", A, "role=menubar", ""),
    r("SideMenu", A, "<nav> rail with a ContentPane", ""),
    r("ToolBar", A, "role=toolbar", "D for the actions that start a program"),
    r("StatusBar", A, "<footer role=status>", ""),
    r("Line", A, "<div> border / inline SVG line", ""),
    r("DateTimePicker", A, "<input type=date|time|datetime-local>", ""),
    r("NumericUpDown", A, "<input type=number>", ""),
    r("TreeView", A, "<ul role=tree>", ""),
    r("Splitter", A, "two panes and a drag handle", ""),
    r("Timer", A, "no element; a timer in the player", ""),
    r("Shape", A, "inline SVG", ""),
    r("AgentObject", C, "no element", "the model key stays on the backend"),
    r("RestClient", C, "no element", ""),
    r("SqlDatabase", C, "no element", ""),
    r("IndexedFile", C, "no element", ""),
    r("KnowledgeBase", C, "no element", ""),
    r("Slider", A, "<input type=range>", ""),
    r("BarChart", B, "inline SVG", ""),
    r("LineChart", B, "inline SVG", ""),
    r("PieChart", B, "inline SVG", ""),
    r("AreaChart", B, "inline SVG", ""),
    r("ScatterChart", B, "inline SVG", ""),
    r("DonutChart", B, "inline SVG", ""),
    r("RadarChart", B, "inline SVG", ""),
    r("Knob", B, "inline SVG", ""),
    r("Gauge", B, "inline SVG", ""),
    r("Switch", A, "<button role=switch>", ""),
    r("FileDropZone", C, "drop target and <input type=file>", "the player moves the bytes; the program never sees them (WASM-017 R13)"),
    r("Maps", B, "tile <img> grid", "C for geocode and directions"),
    r("WebSearch", C, "no element", ""),
    r("Snackbar", A, "a stack of role=status notifications", ""),
    r("Viewer", B, "DOM nodes built from a parsed, allow-listed tree", "C for open and save; D for print and share"),
    r("AwsMcp", C, "no element", ""),
    r("AwsLambda", C, "no element", ""),
    r("AwsKnowledgeBase", C, "no element", ""),
    r("AwsAgentCore", C, "no element", ""),
    r("AwsAgentMemory", C, "no element", ""),
    r("AwsS3Tables", C, "no element", ""),
    r("AwsGlue", C, "no element", ""),
    r("AwsDynamoDB", C, "no element", ""),
    r("AwsS3", C, "no element", ""),
    r("AwsS3Vectors", C, "no element", ""),
    r("AwsRekognition", C, "no element", ""),
    r("AwsPolly", C, "no element", ""),
    r("AwsComprehend", C, "no element", ""),
    r("AwsTextract", C, "no element", ""),
    r("AwsEC2", C, "no element", ""),
    r("AwsCognito", C, "no element", "a desktop/backend control: the Authentication control replaces it in a browser"),
];

/// The class of a control type by its model name. `Custom` plugin controls are class D
/// (WASM-004: until a later spec); the player-level kinds are listed in [`PLAYER_KINDS`].
pub fn class_of(t: &ControlType) -> Class {
    if let Some(k) = player_kind_of(t) {
        return k.class();
    }
    let name = t.as_str();
    TABLE.iter().find(|row| row.control == name).map(|row| row.class).unwrap_or(D)
}

/// Can a Web or Mobile project use this control? Everything but class D (WASM-001 R14,
/// WASM-004 R5).
pub fn available_on_web(t: &ControlType) -> bool {
    class_of(t) != D
}

/// Controls that exist only in the browser series and that the model carries as `Custom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKind {
    WebLink,
    Authentication,
    Canvas2D,
    Canvas3D,
    ImageBuffer,
    Plot,
    Scene3D,
    Physics2D,
    Physics3D,
}

pub const PLAYER_KINDS: &[(PlayerKind, &str)] = &[
    (PlayerKind::WebLink, "WebLink"),
    (PlayerKind::Authentication, "Authentication"),
    (PlayerKind::Canvas2D, "Canvas2D"),
    (PlayerKind::Canvas3D, "Canvas3D"),
    (PlayerKind::ImageBuffer, "ImageBuffer"),
    (PlayerKind::Plot, "Plot"),
    (PlayerKind::Scene3D, "Scene3D"),
    (PlayerKind::Physics2D, "Physics2D"),
    (PlayerKind::Physics3D, "Physics3D"),
];

impl PlayerKind {
    pub fn name(self) -> &'static str {
        PLAYER_KINDS.iter().find(|(k, _)| *k == self).map(|(_, n)| *n).unwrap()
    }
    pub fn class(self) -> Class {
        match self {
            PlayerKind::WebLink => A,
            PlayerKind::Authentication => C,
            PlayerKind::Canvas2D | PlayerKind::Canvas3D => B,
            _ => E,
        }
    }
    /// Does it have a place on the page?
    pub fn visual(self) -> bool {
        matches!(self, PlayerKind::WebLink | PlayerKind::Authentication | PlayerKind::Canvas2D | PlayerKind::Canvas3D)
    }
}

/// The player-level kind of a `Custom` control the model holds under one of those names.
pub fn player_kind_of(t: &ControlType) -> Option<PlayerKind> {
    match t {
        ControlType::Custom { control_id, .. } => PLAYER_KINDS.iter().find(|(_, n)| n.eq_ignore_ascii_case(control_id)).map(|(k, _)| *k),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_table_and_the_model_agree_on_every_control() {
        // WASM-004 AC1: ControlType::ALL and the table disagree -> the test fails.
        let model: BTreeSet<&str> = ControlType::ALL.iter().map(|t| t.as_str()).collect();
        let table: Vec<&str> = TABLE.iter().map(|r| r.control).collect();
        let table_set: BTreeSet<&str> = table.iter().copied().collect();
        assert_eq!(table.len(), table_set.len(), "a control is listed twice");
        assert_eq!(model.difference(&table_set).collect::<Vec<_>>(), Vec::<&&str>::new(), "controls missing from the table");
        assert_eq!(table_set.difference(&model).collect::<Vec<_>>(), Vec::<&&str>::new(), "table rows the model does not have");
        assert_eq!(TABLE.len(), 62, "the catalogue is 62 types; count it, do not trust this line");
    }

    #[test]
    fn the_classes_have_the_sizes_the_spec_describes() {
        let n = |c: Class| TABLE.iter().filter(|r| r.class == c).count();
        println!("class A: {}  B: {}  C: {}  D: {}  E: {}", n(A), n(B), n(C), n(D), n(E));
        assert_eq!(n(A) + n(B) + n(C) + n(D) + n(E), 62);
        assert_eq!(n(B), 12, "7 charts, Knob, Gauge, Animator, Maps, Viewer");
        assert_eq!(n(C), 8 + 16, "8 service controls (+ DataGrid, FileDropZone) and the 16 AWS controls");
    }

    #[test]
    fn the_series_controls_are_found_by_name() {
        let t = ControlType::from_str("WebLink");
        assert_eq!(player_kind_of(&t), Some(PlayerKind::WebLink));
        assert_eq!(class_of(&t), Class::A);
        assert_eq!(class_of(&ControlType::from_str("Authentication")), Class::C);
        assert_eq!(class_of(&ControlType::from_str("Canvas3D")), Class::B);
        assert_eq!(class_of(&ControlType::from_str("Physics2D")), Class::E);
        assert_eq!(class_of(&ControlType::from_str("acme:thing")), Class::D, "a plug-in control is not available");
    }

    #[test]
    fn a_plug_in_control_is_not_available_on_the_web_and_a_button_is() {
        let plug_in = ControlType::from_str("acme:thing");
        assert!(matches!(plug_in, ControlType::Custom { .. }), "a namespaced name is a plug-in control");
        assert!(!available_on_web(&plug_in));
        assert!(available_on_web(&ControlType::Button));
        assert!(available_on_web(&ControlType::from_str("WebLink")), "a series control is not a plug-in");
        let unavailable: Vec<&str> = ControlType::ALL.iter().filter(|t| !available_on_web(t)).map(|t| t.as_str()).collect();
        println!("built-in controls not available on the web: {} {:?}", unavailable.len(), unavailable);
        assert!(unavailable.is_empty(), "no built-in control is class D today (plan D-T14b)");
    }
}
