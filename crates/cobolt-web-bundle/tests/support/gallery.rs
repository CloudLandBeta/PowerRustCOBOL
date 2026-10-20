// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The gallery**: a form with one of every control the player draws, written as the `.cfrm` the
//! designer would save, laid around the fixture program's own controls. It is what a real browser is
//! pointed at to see — and to script — every class A and B control in one page.

use cobolt_forms::menu::{save_menu, MenuDefinition, MenuItem};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[allow(clippy::too_many_arguments)]
fn ctl(id: &str, ty: &str, x: i32, y: i32, w: i32, h: i32, parent: Option<&str>, tab: Option<u32>, props: &[(&str, &str)]) -> String {
    let mut s = format!("  <Control id=\"{id}\" type=\"{ty}\" x=\"{x}\" y=\"{y}\" w=\"{w}\" h=\"{h}\"");
    if let Some(p) = parent {
        s.push_str(&format!(" parent=\"{p}\""));
    }
    if let Some(t) = tab {
        s.push_str(&format!(" tab=\"{t}\""));
    }
    s.push_str(">");
    for (k, v) in props {
        s.push_str(&format!("<Property name=\"{k}\">{}</Property>", esc(v)));
    }
    s.push_str("</Control>\n");
    s
}

/// The Spatial look the IDE stamps on a Button, Panel or GroupBox (the plugin's golden rule 2), plus `extra`.
pub fn spatial(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v = vec![
        ("BackgroundColor", "#36383EB3"),
        ("BackgroundGradientEnabled", "true"),
        ("BackgroundGradientStartColor", "#4E4E4EB3"),
        ("BackgroundGradientEndColor", "#000000B3"),
        ("BackgroundGradientDirection", "South"),
        ("ForegroundColor", "#FFFFFFFF"),
        ("BorderStyle", "None"),
        ("ShadowEnabled", "true"),
        ("ShadowOpacity", "6"),
        ("ShadowColor", "#000000FF"),
        ("ShadowLightColor", "#4E4E4EFF"),
        ("ShadowDirection", "SouthEast"),
        ("ShadowDistance", "7"),
        ("ShadowBlur", "true"),
        ("ShadowBlurStrength", "8"),
    ];
    v.extend_from_slice(extra);
    v
}

/// A 1 x 1 PNG, stretched by the control.
pub const PNG_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

pub const DOC_MD: &str = "# The gallery document\n\nA *Markdown* file of the application, **read by the page** and drawn as nodes.\n\n- one\n- two\n\n| A | B |\n|---|--:|\n| 1 | 2 |\n\n[a link](https://example.com/) and `code`.\n\n```mermaid\nflowchart LR\n  A[Start] --> B[Review] --> C[Done]\n```\n";

pub fn menu_yaml(items: Vec<MenuItem>) -> String {
    let dir = tempfile::tempdir().expect("a temp dir");
    let p = dir.path().join("m.menu.yaml");
    save_menu(&p, &MenuDefinition { menu: items, hash: String::new() }).expect("the menu saves");
    std::fs::read_to_string(&p).expect("the menu reads back")
}

/// The File/Help menu bar of the gallery.
pub fn menu_bar_yaml() -> String {
    let mut file = MenuItem::new_action("file", "File");
    let mut open = MenuItem::new_action("open", "Open");
    open.accelerator = Some("Ctrl+O".into());
    open.icon = Some("doc-open".into());
    let mut quit = MenuItem::new_action("quit", "Quit");
    quit.enabled = false;
    file.items = vec![open, MenuItem::new_separator("s1"), quit];
    let help = MenuItem::new_action("help", "Help");
    menu_yaml(vec![file, help])
}

/// The rail of the shell variant: Home and a form to load.
pub fn side_menu_yaml(target: &str) -> String {
    let mut home = MenuItem::new_action("home", "Home");
    home.icon = Some("home".into());
    let mut go = MenuItem::new_action("go", "Child form");
    go.action = Some(format!("open-form:{target}"));
    go.badge = Some("New".into());
    let section = MenuItem::new_separator("sec");
    menu_yaml(vec![MenuItem { label: "WORK".into(), ..section }, home, go])
}

pub struct Options {
    pub responsive: bool,
}

/// The standard fixture form with the gallery's controls added and the form made large enough for them.
pub fn gallery_cfrm(standard: &str, o: &Options) -> String {
    let rows: String = (1..=60).map(|i| format!("Item {i}\t{}\n", i * 7 % 23)).collect();
    let mut s = String::new();
    // Column A
    s += &ctl("LBL-G", "Label", 240, 10, 200, 24, None, None, &[("Caption", "A Label — ação 日本語")]);
    s += &ctl("BTN-G", "Button", 240, 40, 200, 30, None, None, &spatial(&[("Caption", "A Button")]));
    s += &ctl("TXT-G", "TextBox", 240, 80, 200, 28, None, None, &[("Text", "héllo 日本語")]);
    s += &ctl("TXT-M", "TextBox", 240, 114, 200, 60, None, None, &[("Text", "line one\nline two"), ("Multiline", "1")]);
    s += &ctl("CHK-G", "CheckBox", 240, 184, 200, 22, None, None, &[("Caption", "Check me"), ("Checked", "1")]);
    s += &ctl("RAD-1", "RadioButton", 240, 210, 200, 22, None, None, &[("Caption", "Radio one"), ("GroupName", "g1"), ("Selected", "1")]);
    s += &ctl("RAD-2", "RadioButton", 240, 234, 200, 22, None, None, &[("Caption", "Radio two"), ("GroupName", "g1")]);
    s += &ctl("LST-G", "ListBox", 240, 262, 200, 90, None, None, &[("Items", "Alpha\nBeta\nGamma\nDelta\nEpsilon"), ("SelectedIndex", "2")]);
    s += &ctl("CMB-G", "ComboBox", 240, 360, 200, 28, None, None, &[("Items", "Red\nGreen\nBlue")]);
    s += &ctl("CMB-L", "ComboBox", 240, 394, 200, 28, None, None, &[("Items", "Red\nGreen\nBlue"), ("DropDownStyle", "DropDownList"), ("SelectedIndex", "2")]);
    s += &ctl("NUD-G", "NumericUpDown", 240, 428, 120, 28, None, None, &[("Value", "7"), ("Minimum", "0"), ("Maximum", "10")]);
    s += &ctl("DTP-G", "DateTimePicker", 240, 462, 160, 28, None, None, &[("Value", "2026-10-20")]);
    s += &ctl("SLD-G", "Slider", 240, 496, 200, 24, None, None, &[("Value", "40"), ("Minimum", "0"), ("Maximum", "100")]);
    s += &ctl("PRG-G", "ProgressBar", 240, 526, 200, 20, None, None, &[("Value", "65"), ("ShowValue", "1")]);
    s += &ctl("SWT-G", "Switch", 240, 556, 52, 26, None, None, &[("Checked", "1")]);
    s += &ctl("KNB-G", "Knob", 240, 590, 100, 100, None, None, &[("Value", "35")]);
    s += &ctl("GAU-1", "Gauge", 350, 590, 120, 100, None, None, &[("Value", "62"), ("GaugeStyle", "Radial"), ("WarningThreshold", "50"), ("CriticalThreshold", "80")]);
    s += &ctl("GAU-2", "Gauge", 240, 696, 200, 40, None, None, &[("Value", "45"), ("GaugeStyle", "Linear")]);
    s += &ctl("GAU-3", "Gauge", 450, 696, 70, 70, None, None, &[("Value", "80"), ("GaugeStyle", "Donut")]);
    s += &ctl("PIC-G", "PictureBox", 240, 744, 100, 60, None, None, &[("ImagePath", PNG_DATA_URI), ("SizeMode", "Stretch"), ("ShowFrame", "1")]);
    // Column B
    s += &ctl("GRP-G", "GroupBox", 540, 10, 240, 120, None, None, &spatial(&[("Caption", "A GroupBox")]));
    s += &ctl("LBL-IN-GRP", "Label", 556, 40, 150, 22, Some("GRP-G"), None, &[("Caption", "inside the group")]);
    s += &ctl("PNL-G", "Panel", 540, 140, 240, 90, None, None, &spatial(&[]));
    s += &ctl("BTN-IN-PNL", "Button", 556, 160, 120, 30, Some("PNL-G"), None, &spatial(&[("Caption", "in a panel")]));
    s += &ctl("TAB-G", "TabControl", 540, 240, 240, 130, None, None, &[("Tabs", "One\nTwo")]);
    s += &ctl("LBL-TAB1", "Label", 556, 290, 150, 22, Some("TAB-G"), Some(1), &[("Caption", "page one")]);
    s += &ctl("LBL-TAB2", "Label", 556, 290, 150, 22, Some("TAB-G"), Some(2), &[("Caption", "page two")]);
    s += &ctl("SPL-G", "Splitter", 540, 380, 240, 120, None, None, &[("SplitPosition", "40")]);
    s += &ctl("SPL-G-Pane1", "Panel", 542, 382, 94, 116, Some("SPL-G"), None, &[("IsSplitterPane", "1")]);
    s += &ctl("LBL-P1", "Label", 546, 390, 80, 22, Some("SPL-G-Pane1"), None, &[("Caption", "left pane")]);
    s += &ctl("SPL-G-Pane2", "Panel", 640, 382, 138, 116, Some("SPL-G"), None, &[("IsSplitterPane", "2")]);
    s += &ctl("LBL-P2", "Label", 644, 390, 100, 22, Some("SPL-G-Pane2"), None, &[("Caption", "right pane")]);
    s += &ctl("LIN-G", "Line", 540, 510, 240, 4, None, None, &[("LineColor", "#FFAA00"), ("LineThickness", "3")]);
    s += &ctl("SHP-1", "Shape", 540, 522, 70, 50, None, None, &[("ShapeType", "Rectangle"), ("FillColor", "#3B82F6")]);
    s += &ctl("SHP-2", "Shape", 620, 522, 50, 50, None, None, &[("ShapeType", "Circle"), ("FillColor", "#10B981")]);
    s += &ctl("SHP-3", "Shape", 680, 522, 60, 50, None, None, &[("ShapeType", "Triangle"), ("FillColor", "#F59E0B")]);
    s += &ctl("TRE-G", "TreeView", 540, 584, 240, 130, None, None, &[("Items", "Root\n  Child A\n  Child B\nOther"), ("CheckBoxes", "1"), ("SelectedNode", "Child A")]);
    s += &ctl("VWR-G", "Viewer", 540, 722, 240, 200, None, None, &[("Source", "Assets/doc.md")]);
    // Column C
    s += &ctl("GRD-G", "DataGrid", 800, 10, 380, 150, None, None, &[("Columns", "Name:string\nQty:number"), ("Rows", &rows)]);
    let data = "Q1\t10\t4\nQ2\t24\t8\nQ3\t17\t12\nQ4\t30\t9";
    let chart = |id: &str, ty: &str, x: i32, y: i32, w: i32, h: i32| ctl(id, ty, x, y, w, h, None, None, &[("__ChartData", data), ("Title", ty), ("SeriesLabels", "North,South"), ("Width", &w.to_string()), ("Height", &h.to_string())]);
    s += &chart("BAR-G", "BarChart", 800, 170, 380, 160);
    s += &chart("LNE-G", "LineChart", 800, 340, 185, 140);
    s += &chart("ARE-G", "AreaChart", 995, 340, 185, 140);
    s += &chart("PIE-G", "PieChart", 800, 490, 185, 140);
    s += &chart("DON-G", "DonutChart", 995, 490, 185, 140);
    s += &chart("SCA-G", "ScatterChart", 800, 640, 185, 140);
    s += &chart("RDR-G", "RadarChart", 995, 640, 185, 140);
    s += &ctl("STB-G", "StatusBar", 800, 790, 380, 24, None, None, &[("Items", "Ready\nOnline")]);
    let tb = r#"{"groups":[{"id":"g1","buttons":[{"id":"open","label":"Open","tooltip":"Open a file","icon":"doc-open"},{"id":"save","label":"Save"},{"id":"print","label":"Print","action":"print:x.pdf"}]}]}"#;
    s += &ctl("TBR-G", "ToolBar", 800, 820, 380, 36, None, None, &[(cobolt_forms::toolbar::TOOLBAR_DEF_PROP, tb)]);
    s += &ctl("MNB-G", "MenuBar", 800, 862, 380, 26, None, None, &[]);
    s += &ctl("MAP-G", "Maps", 800, 896, 380, 220, None, None, &[("CenterLat", "51.5074"), ("CenterLng", "-0.1278"), ("Zoom", "11"), ("Width", "380"), ("Height", "220"), ("Markers", "lon\t51.5074\t-0.1278\tLondon\tThe capital")]);
    s += &ctl("DZ-G", "FileDropZone", 540, 930, 240, 70, None, None, &[("Hint", "Drop files here")]);
    s += &ctl("LNK-G", "WebLink", 240, 812, 200, 24, None, None, &[("Url", "https://example.com/"), ("Caption", "example.com"), ("Target", "NewTab")]);
    s += &ctl("SNK-G", "Snackbar", 240, 840, 56, 56, None, None, &[("Text", "Saved"), ("Category", "Info"), ("Buttons", "undo|Undo")]);
    s += &ctl("AUTH-G", "Authentication", 240, 904, 290, 220, None, None, &spatial(&[]));

    let mut xml = standard.to_owned();
    let size = format!("width=\"1220\" height=\"1130\" background=\"#2E3138FF\" index-base=\"1\"{}", if o.responsive { " responsive=\"true\"" } else { "" });
    xml = xml.replacen("width=\"640\" height=\"480\"", &size, 1);
    xml.replacen("</Form>", &format!("{s}</Form>"), 1)
}

/// The standard form with a SideMenu rail (the shell variant).
pub fn shell_cfrm(standard: &str) -> String {
    // The program's own controls move right of the rail.
    let mut xml = standard.replace("x=\"10\"", "x=\"240\"");
    xml = xml.replacen("width=\"640\" height=\"480\"", "width=\"900\" height=\"600\" responsive=\"true\"", 1);
    let rail = ctl("SIDE-1", "SideMenu", 0, 0, 220, 600, None, None, &[("AppTitle", "Gallery"), ("Collapsed", "0")]);
    xml.replacen("</Form>", &format!("{rail}</Form>"), 1)
}
