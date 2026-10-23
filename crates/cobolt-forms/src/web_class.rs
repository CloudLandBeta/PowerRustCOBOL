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

/// How faithfully the browser reproduces an appearance property (WASM-004 R6, WASM-007 R1).
/// The Properties panel hides a `NotOnWeb` row on a web or mobile form and marks an
/// `Approximate` one in its tooltip; the player's CSS declarations are keyed by these rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    Exact,
    Approximate,
    NotOnWeb,
}

/// One fidelity row: a model property and how the web draws it.
#[derive(Debug, Clone, Copy)]
pub struct PropertyFidelity {
    pub property: &'static str,
    pub fidelity: Fidelity,
    pub note: &'static str,
}

const fn f(property: &'static str, fidelity: Fidelity, note: &'static str) -> PropertyFidelity {
    PropertyFidelity { property, fidelity, note }
}

/// Every appearance property and how faithfully the web draws it. The player's `css.rs` holds
/// the CSS for each of these rows and no other (a test in the player fails otherwise).
pub const FIDELITY: &[PropertyFidelity] = {
    use Fidelity::{Approximate, Exact, NotOnWeb};
    &[
        f("BackgroundColor", Exact, ""),
        f("BackgroundGradientEnabled", Exact, ""),
        f("BackgroundGradientStartColor", Exact, ""),
        f("BackgroundGradientEndColor", Exact, ""),
        f("BackgroundGradientDirection", Exact, ""),
        f("ForegroundColor", Exact, ""),
        f("FontName", Exact, "a name the browser does not have falls back to the system font"),
        f("FontSize", Exact, "pixels"),
        f("Bold", Exact, ""),
        f("Italic", Exact, ""),
        f("Underline", Exact, ""),
        f("Strikethrough", Exact, ""),
        f("Tooltip", Exact, ""),
        f("Cursor", Exact, "Default|Hand|Text|Wait|Crosshair|No|SizeAll|SizeNS|SizeWE"),
        f("Padding", Exact, ""),
        f("PaddingLeft", Exact, ""),
        f("PaddingTop", Exact, ""),
        f("PaddingRight", Exact, ""),
        f("PaddingBottom", Exact, ""),
        f("Transparency", Exact, "fades the frame, never the content"),
        f("CornerRadius", Exact, "drawn corners of spec 057 (children clipped to the arc) are overflow: hidden on the container"),
        f("BorderStyle", Approximate, "Raised/Sunken/Inset/Outset become a two-tone border"),
        f("BorderColor", Exact, ""),
        f("BorderWidth", Exact, ""),
        f("ShadowEnabled", Approximate, "the neumorphic dual shadow is two box-shadows"),
        f("ShadowColor", Exact, ""),
        f("ShadowLightColor", Exact, ""),
        f("ShadowOpacity", Exact, "percent"),
        f("ShadowDirection", Exact, "eight directions"),
        f("ShadowDistance", Exact, "0-60"),
        f("ShadowBlur", Exact, ""),
        f("ShadowBlurStrength", Approximate, "-20..20"),
        f("TextAlignment", Exact, ""),
        f("VerticalAlignment", Exact, ""),
        f("WordWrap", Exact, ""),
        f("RotateX", Approximate, "no per-pixel perspective of the painter's content"),
        f("RotateY", Approximate, ""),
        f("PerspectiveDistance", Approximate, ""),
        f("PerspectiveStrength", Approximate, ""),
        f("BorderGradientEnabled", NotOnWeb, "charts only; the frame is a plain border"),
        f("BorderBlur", NotOnWeb, "an operating-system effect"),
        f("BorderTransparency", NotOnWeb, ""),
        f("ZOrder", Exact, ""),
        f("Visible", Exact, ""),
        f("Enabled", Exact, ""),
        f("OsBlur", NotOnWeb, "the operating-system blur behind a window has no page equivalent; backdrop-filter is not applied by default"),
    ]
};

/// The fidelity row of a property (case-insensitive); `None` for a property not in [`FIDELITY`].
pub fn fidelity_of(property: &str) -> Option<&'static PropertyFidelity> {
    FIDELITY.iter().find(|r| r.property.eq_ignore_ascii_case(property))
}

/// One operation of a control that takes another class than the control's primary one
/// (WASM-004 R2), named as the code that a program writes names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// A method, spelled as `cobolt_ast::methods::is_known_method` spells it.
    Method(&'static str),
    /// A ToolBar button's action, by `crate::toolbar::ToolbarAction::verb`.
    ToolbarVerb(&'static str),
    /// A use of a property (DataGrid's in-memory `Rows`).
    Property(&'static str),
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Operation::Method(n) | Operation::ToolbarVerb(n) | Operation::Property(n) => n,
        }
    }
}

/// A per-operation exception, with the English alternative Check prints beside a class D use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    pub operation: Operation,
    pub class: Class,
    pub alternative: &'static str,
}

const fn x(operation: Operation, class: Class, alternative: &'static str) -> Exception {
    Exception { operation, class, alternative }
}

const THROUGH_THE_BACKEND: &str = "runs through the backend (WASM-005) with the same COBOL call";

const MAPS_EXCEPTIONS: &[Exception] = &[
    x(Operation::Method("Geocode"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("ReverseGeocode"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("Directions"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("DistanceMatrix"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("PlacesSearch"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("TraceRoad"), C, THROUGH_THE_BACKEND),
];

// `LoadBytes` is not an exception: it takes bytes the program already holds, no disk.
const VIEWER_EXCEPTIONS: &[Exception] = &[
    x(Operation::Method("SaveAs"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("SaveAsPdf"), C, THROUGH_THE_BACKEND),
    x(Operation::Method("Print"), D, "use the browser's own Print command, or SaveAsPdf and let the user print the file"),
    x(Operation::Method("Share"), D, "offer a Hyperlink to the document instead"),
];

// The spec names as D the actions that start a program, open a terminal or print through the
// OS; `share` follows the Viewer's Share. `screenshot`, `copy`, `cut` and `paste` are not named
// and stay A (the browser's clipboard, a capture of the page) — WASM-004 tasks.md, a working
// assumption to confirm.
const TOOLBAR_EXCEPTIONS: &[Exception] = &[
    x(Operation::ToolbarVerb("run-app"), D, "a browser cannot start a program: open a Hyperlink or call a backend procedure"),
    x(Operation::ToolbarVerb("open-terminal"), D, "a browser has no terminal: remove the action"),
    x(Operation::ToolbarVerb("print"), D, "use the browser's own Print command"),
    x(Operation::ToolbarVerb("share"), D, "offer a Hyperlink the user can share"),
];

// Q2 working assumption: DataGrid is C for its IndexedFile and SQL bindings, A for in-memory `Rows`.
const DATAGRID_EXCEPTIONS: &[Exception] = &[x(Operation::Property("Rows"), A, "")];

/// The operations of `t` that take another class than [`class_of`] gives (WASM-004 R2).
pub fn exceptions_of(t: &ControlType) -> &'static [Exception] {
    match t {
        ControlType::Maps => MAPS_EXCEPTIONS,
        ControlType::Viewer => VIEWER_EXCEPTIONS,
        ControlType::ToolBar => TOOLBAR_EXCEPTIONS,
        ControlType::DataGrid => DATAGRID_EXCEPTIONS,
        _ => &[],
    }
}

/// The class D exception of `t` named `operation` (case-insensitive), if there is one.
pub fn unavailable_operation(t: &ControlType, operation: &str) -> Option<&'static Exception> {
    exceptions_of(t).iter().find(|e| e.class == D && e.operation.name().eq_ignore_ascii_case(operation))
}

/// What Check prints beside a class D control (WASM-004 R5). `None` for any other class.
pub fn alternative_of(t: &ControlType) -> Option<&'static str> {
    match class_of(t) {
        D => Some("plug-in controls are not available on the web or on mobile: remove the control or use a built-in one"),
        _ => None,
    }
}

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
    /// The spelling the player uses today (its CSS class, its tests).
    pub fn name(self) -> &'static str {
        PLAYER_KINDS.iter().find(|(k, _)| *k == self).map(|(_, n)| *n).unwrap()
    }
    /// The control's name in the series (what the KB, Check and the agents say). `WebLink` was
    /// renamed **Hyperlink** by WASM-006; the player keeps the old spelling until WASM-006
    /// renames it, and both spellings are recognised by [`player_kind_of`].
    pub fn series_name(self) -> &'static str {
        match self {
            PlayerKind::WebLink => "Hyperlink",
            k => k.name(),
        }
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
        ControlType::Custom { control_id, .. } => PLAYER_KINDS
            .iter()
            .find(|(k, n)| n.eq_ignore_ascii_case(control_id) || k.series_name().eq_ignore_ascii_case(control_id))
            .map(|(k, _)| *k),
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
    fn every_series_name_is_recognised_once_and_hyperlink_keeps_its_old_spelling() {
        let names: Vec<&str> = PLAYER_KINDS.iter().map(|(k, _)| k.series_name()).collect();
        let set: BTreeSet<&str> = names.iter().copied().collect();
        println!("series kinds: {names:?}");
        assert_eq!(names.len(), set.len(), "a series name is listed twice");
        for (k, _) in PLAYER_KINDS {
            assert_eq!(player_kind_of(&ControlType::from_str(k.series_name())), Some(*k), "{}", k.series_name());
            assert!(!ControlType::ALL.iter().any(|t| t.as_str().eq_ignore_ascii_case(k.series_name())), "{} is a model type now: move its row to TABLE", k.series_name());
        }
        assert_eq!(player_kind_of(&ControlType::from_str("Hyperlink")), Some(PlayerKind::WebLink));
        assert_eq!(player_kind_of(&ControlType::from_str("WebLink")), Some(PlayerKind::WebLink), "the old spelling");
    }

    #[test]
    fn every_exception_names_an_operation_the_control_has() {
        let mut n = 0;
        for t in ControlType::ALL {
            for e in exceptions_of(t) {
                n += 1;
                println!("{} {:?} -> {:?}", t.as_str(), e.operation, e.class);
                assert_ne!(e.class, class_of(t), "{} {:?} repeats the primary class", t.as_str(), e.operation);
                if e.class == D {
                    assert!(!e.alternative.is_empty(), "a class D operation carries its alternative");
                }
                match e.operation {
                    Operation::Method(m) => assert!(cobolt_ast::methods::is_known_method(m), "{m} is not in the method vocabulary"),
                    Operation::ToolbarVerb(v) => {
                        assert_eq!(*t, ControlType::ToolBar);
                        assert!(crate::toolbar::ToolbarAction::VERBS.contains(&v), "{v} is not a ToolbarAction verb");
                    }
                    Operation::Property(p) => {
                        let c = crate::model::Control::new("X", t.clone(), 0, 0);
                        assert!(c.get_prop(p).is_some(), "{} has no property {p}", t.as_str());
                    }
                }
            }
        }
        assert_eq!(n, 15, "Maps 6, Viewer 4, ToolBar 4, DataGrid 1");
        assert_eq!(unavailable_operation(&ControlType::Viewer, "PRINT").map(|e| e.class), Some(D));
        assert!(unavailable_operation(&ControlType::Viewer, "LoadBytes").is_none(), "no disk: not an exception");
        assert!(unavailable_operation(&ControlType::Maps, "Geocode").is_none(), "C is available");
        assert!(alternative_of(&ControlType::from_str("acme:thing")).is_some());
        assert!(alternative_of(&ControlType::Button).is_none());
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
