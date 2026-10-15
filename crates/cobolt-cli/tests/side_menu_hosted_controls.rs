// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Controls inside a SideMenu's rail** (spec 066 Phases B and C) — through the
//! real `rcrun`, as the application shell draws them.
//!
//! A shell whose rail has a header band (a Panel under the logo, holding a
//! control) and a menu row that hosts a control, run headless with a picture. The
//! two controls are painted in colours nothing else on the form uses, so the
//! picture says where they are: inside the rail, in the band and in the row, while
//! it is open — and nowhere at all once the operator has folded it. `HOME` is a
//! private folder, so the remembered pane state (`shell.toml`) is the test's own.

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_forms::menu::{save_menu, MenuDefinition, MenuItem};
use cobolt_forms::{save_form, Control, ControlType, Form, Rect};
use serde_json::{json, Value};

const FORM_W: i32 = 960;
const FORM_H: i32 = 600;
const RAIL_W: i32 = 240;
const HEADER_H: i32 = 100;
const BAND_H: i32 = 60;
const FOOTER_H: i32 = 60;

/// The colour of the control in the header band, and of the one a row hosts.
const BAND_INK: [u8; 3] = [255, 0, 255];
const ROW_INK: [u8; 3] = [0, 255, 0];

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn marker(id: &str, parent: &str, x: i32, y: i32, ink: [u8; 3]) -> Control {
    let mut c = Control::new(id, ControlType::TextBox, x, y);
    c.rect = Rect::new(x, y, 150, 28);
    c.parent = Some(parent.to_owned());
    c.set_prop("BackgroundColor", format!("#{:02X}{:02X}{:02X}", ink[0], ink[1], ink[2]));
    c
}

/// The project: one main form with a SideMenu rail — logo header, a band for the
/// developer's controls, rows, footer — and a TextBox in the band and one that
/// the third row hosts. The hosted one is designed far from the rail, to show it
/// is placed by its row and not left where it was designed.
fn project(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("prc-rail-controls-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }

    let mut form = Form::new("SHELL", "Rail controls", FORM_W as u32, FORM_H as u32);
    form.main_form = true;
    let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
    side.rect = Rect::new(0, 0, RAIL_W, FORM_H);
    side.set_prop("HeaderHeight", HEADER_H as i64);
    side.set_prop("HeaderBandHeight", BAND_H as i64);
    side.set_prop("FooterHeight", FOOTER_H as i64);
    form.controls.push(side);
    // The two Panels the rail owns for its bands, created as the designer does.
    form.sync_side_menu_footer_panels();
    form.sync_side_menu_header_panels();
    let band = cobolt_forms::model::side_menu_header_id("SideMenu-1");
    // A child of the band is designed where it shows on the canvas — in form
    // coordinates, inside the band below the logo.
    form.controls.push(marker("TXT-BAND", &band, 20, HEADER_H + 16, BAND_INK));
    // Designed at (600, 300): nowhere near the rail.
    form.controls.push(marker("TXT-ROW", "SideMenu-1", 600, 300, ROW_INK));
    save_form(&form, &root.join("forms/shell.cfrm")).unwrap();

    let mut picker = MenuItem::new_action("picker", "Picker");
    picker.control = Some("TXT-ROW".into());
    let def = MenuDefinition {
        menu: vec![MenuItem::new_action("home", "Home"), MenuItem::new_action("reports", "Reports"), picker],
        hash: String::new(),
    };
    save_menu(&root.join("forms/SideMenu-1.menu.yaml"), &def).unwrap();

    let form = cobolt_forms::load_form(&root.join("forms/shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/shell.cbl"), cobolt_codegen::generate(&form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("railcontrols.project.toml"),
        "[project]\nname = \"RailControls\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/shell.cfrm\"]\ngenerated = [\"generated/shell.cbl\"]\n",
    )
    .unwrap();
    root
}

/// Where the shell remembers the rail for `SHELL` under `home`.
fn remembered(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let data = home.join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let data = home.join(".local/share");
    data.join("cobolt/apps/SHELL/shell.toml")
}

/// Run the shell headless and answer its picture. `collapsed` is the choice the
/// operator remembered from an earlier session.
fn picture(tag: &str, collapsed: bool) -> (image::RgbaImage, PathBuf) {
    let root = project(tag);
    let home = root.join("home");
    let state = remembered(&home);
    std::fs::create_dir_all(state.parent().unwrap()).unwrap();
    std::fs::write(&state, format!("collapsed = {collapsed}\n")).unwrap();
    let script = root.join("steps.json");
    std::fs::write(&script, Value::Array(vec![json!({"wait_ms": 1200})]).to_string()).unwrap();
    let png = root.join("shell.png");
    let out = Command::new(rcrun())
        .args([
            "run-form",
            "forms/shell.cfrm",
            "generated/shell.cbl",
            "--designer",
            "--headless",
            script.to_str().unwrap(),
            "--headless-limit",
            "60",
            "--headless-png",
            png.to_str().unwrap(),
        ])
        .env("HOME", &home)
        .current_dir(&root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(
        !stdout.contains("runtime_error"),
        "the program reported a runtime error:\n{stdout}"
    );
    let img = image::open(&png).unwrap_or_else(|e| panic!("no picture at {}: {e}\n{stdout}", png.display())).to_rgba8();
    (img, root)
}

/// The bounding box, in points, of the pixels painted `ink` — `None` when none is.
/// A tolerance, because the glass fill is composited over what is behind it.
fn bounds(img: &image::RgbaImage, scale: f32, ink: [u8; 3]) -> Option<(f32, f32, f32, f32)> {
    let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 40;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut n = 0u32;
    for (x, y, p) in img.enumerate_pixels() {
        if near(p[0], ink[0]) && near(p[1], ink[1]) && near(p[2], ink[2]) {
            n += 1;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    // A handful of stray pixels is antialiasing, not a control.
    (n >= 200).then(|| (x0 as f32 / scale, y0 as f32 / scale, (x1 + 1) as f32 / scale, (y1 + 1) as f32 / scale))
}

#[test]
fn a_band_control_and_a_hosted_control_are_in_the_rail_when_open_and_gone_when_collapsed() {
    let (open, root_open) = picture("open", false);
    let scale = open.width() as f32 / FORM_W as f32;
    assert!(scale >= 1.0, "picture {}x{} for a {FORM_W}x{FORM_H} form", open.width(), open.height());

    let band = bounds(&open, scale, BAND_INK).expect("the control in the header band is drawn while the rail is open");
    let row = bounds(&open, scale, ROW_INK).expect("the control a row hosts is drawn while the rail is open");
    println!("open rail ({}x{} px, scale {scale}): band control {band:?}, hosted control {row:?}", open.width(), open.height());

    // Inside the rail, in the band under the logo.
    assert!(band.0 >= 0.0 && band.2 <= RAIL_W as f32, "band control inside the rail's width: {band:?}");
    assert!(
        band.1 >= HEADER_H as f32 - 1.0 && band.3 <= (HEADER_H + BAND_H) as f32 + 1.0,
        "band control inside the band [{HEADER_H}, {}]: {band:?}",
        HEADER_H + BAND_H
    );
    // In the rail, among the rows: below the band, above the footer — and not
    // where it was designed (600, 300).
    assert!(row.0 >= 0.0 && row.2 <= RAIL_W as f32, "hosted control inside the rail's width: {row:?}");
    assert!(
        row.1 >= (HEADER_H + BAND_H) as f32 && row.3 <= (FORM_H - FOOTER_H) as f32,
        "hosted control between the band and the footer: {row:?}"
    );
    assert!(row.1 > band.3, "the row is below the band: row {row:?}, band {band:?}");

    let (folded, root_folded) = picture("folded", true);
    let fscale = folded.width() as f32 / FORM_W as f32;
    let band_f = bounds(&folded, fscale, BAND_INK);
    let row_f = bounds(&folded, fscale, ROW_INK);
    println!("folded rail: band control {band_f:?}, hosted control {row_f:?}");
    // The folded picture is a real, different rail — not a blank window.
    let differing = open.pixels().zip(folded.pixels()).filter(|(a, b)| a != b).count();
    assert!(differing > 5_000, "the collapsed rail must look different from the open one ({differing} pixels differ)");
    println!("open and folded pictures differ in {differing} of {} pixels", open.width() * open.height());
    assert_eq!(band_f, None, "the header band's control is hidden while the rail is collapsed");
    assert_eq!(row_f, None, "the hosted control is hidden while the rail is collapsed — and not left at its designed place");

    let _ = std::fs::remove_dir_all(root_open);
    let _ = std::fs::remove_dir_all(root_folded);
}
