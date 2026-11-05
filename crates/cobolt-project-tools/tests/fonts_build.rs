// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T27 (R8, Q7) on a real Web project: every font the project references ships, an unresolvable family stops
//! the Build, a font that forbids embedding is refused, and the project's COBOL is read and never written.

use cobolt_forms::model::{Control, ControlType, Form, PropValue};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::PathBuf;

const UBUNTU: &[u8] = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");

/// The font with its `OS/2.fsType` set to `fs`.
fn with_fs_type(fs: u16) -> Vec<u8> {
    let mut b = UBUNTU.to_vec();
    let tables = u16::from_be_bytes([b[4], b[5]]) as usize;
    for i in 0..tables {
        let rec = 12 + i * 16;
        if &b[rec..rec + 4] == b"OS/2" {
            let off = u32::from_be_bytes([b[rec + 8], b[rec + 9], b[rec + 10], b[rec + 11]]) as usize;
            b[off + 8..off + 10].copy_from_slice(&fs.to_be_bytes());
        }
    }
    b
}

/// A Web project: one form whose label names `label_font`; Common Code `src/draw.cbl` with `cobol`; the font files
/// under `Assets/` given by name.
fn project(tag: &str, label_font: &str, cobol: &str, assets: &[(&str, Vec<u8>)]) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("prc-t27-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("Assets")).unwrap();
    let names: Vec<String> = assets.iter().map(|(n, _)| format!("\"Assets/{n}\"")).collect();
    std::fs::write(
        dir.join("Fonts.project.toml"),
        format!(
            "[project]\nname = \"Fonts.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
             [files]\nforms = [\"webforms/main-form.wcfrm\"]\nsources = [\"src/draw.cbl\"]\nassets = [{}]\n",
            names.join(", ")
        ),
    )
    .unwrap();
    std::fs::write(dir.join("src/draw.cbl"), cobol).unwrap();
    for (n, b) in assets {
        std::fs::write(dir.join("Assets").join(n), b).unwrap();
    }
    let mut form = Form::new("main-form", "Fonts", 400, 300);
    let mut label = Control::new("LBL-1", ControlType::Label, 10, 10);
    label.set_prop("FontName", PropValue::String(label_font.into()));
    form.controls.push(label);
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.wcfrm")).unwrap();
    (dir.clone(), cobol.to_owned())
}

fn refusals(r: Result<cobolt_project_tools::web::BuildReport, WebBuildError>) -> Vec<String> {
    match r {
        Err(WebBuildError::Refused(ds)) => ds.iter().map(|d| d.to_string()).collect(),
        Err(e) => panic!("{e}"),
        Ok(_) => Vec::new(),
    }
}

#[test]
fn the_fonts_a_project_references_ship_and_an_unresolvable_or_restricted_one_stops_the_build() {
    let started = std::time::Instant::now();
    let cobol = "       *> a data item names no font: warned\n           CNV-1::SetFont( WS-FACE )\n";

    // 1. A family no file supplies stops the Build, naming it and the form.
    let (dir, _) = project("missing", "Zzz Nowhere Sans", cobol, &[]);
    let req = WebBuildRequest::from_manifest(&dir.join("Fonts.project.toml")).unwrap();
    let ds = refusals(GeneratorBackend.build(&req));
    assert!(ds.iter().any(|d| d.contains("Zzz Nowhere Sans") && d.contains("form main-form") && d.contains("Assets/")), "{ds:?}");
    assert!(!req.out_dir.join("index.html").exists(), "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);

    // 2. The project's own file supplies the family: the Build goes on, the font ships and is listed with its licence;
    //    the SetFont of a data item is a warning and the COBOL is untouched.
    let (dir, text) = project("own", "Zzz Nowhere Sans", cobol, &[("ZzzNowhereSans.ttf", UBUNTU.to_vec())]);
    let req = WebBuildRequest::from_manifest(&dir.join("Fonts.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let fonts: Vec<&str> = report.fonts.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(fonts, ["Assets/ZzzNowhereSans.ttf"], "the project's own font, listed once");
    let f = &report.fonts[0];
    assert_eq!((f.origin, f.size), ("Assets/", UBUNTU.len() as u64));
    assert!(f.licence.as_ref().is_some_and(|l| l.may_embed), "{f:?}");
    assert!(report.warnings.iter().any(|w| w.to_string().contains("SetFont") && w.to_string().contains("src/draw.cbl:2")), "{:?}", report.warnings);
    assert_eq!(std::fs::read_to_string(dir.join("src/draw.cbl")).unwrap(), text, "the project's COBOL is read, never written");
    let j = report.to_json();
    assert_eq!(j["fonts"][0]["origin"], "Assets/", "{j}");
    assert!(j["fonts"][0]["embedding"].is_string());
    let on_disk = std::fs::read_dir(req.out_dir.join("assets")).map(|d| d.flatten().count()).unwrap_or(0);
    assert!(on_disk >= 1, "the font is in the bundle");
    let _ = std::fs::remove_dir_all(&dir);

    // 3. A font that forbids embedding is refused, named by its file.
    let (dir, _) = project("locked", "Zzz Nowhere Sans", cobol, &[("ZzzNowhereSans.ttf", with_fs_type(0x0002))]);
    let req = WebBuildRequest::from_manifest(&dir.join("Fonts.project.toml")).unwrap();
    let ds = refusals(GeneratorBackend.build(&req));
    assert!(ds.iter().any(|d| d.contains("Assets/ZzzNowhereSans.ttf") && d.contains("forbids embedding")), "{ds:?}");
    let _ = std::fs::remove_dir_all(&dir);

    println!(
        "T27 fonts on a Web project: unresolvable family → refused naming family and form; own Assets/ font → shipped ({} bytes, {}); restricted font (fsType 2) → refused; data-item SetFont → warning at src/draw.cbl:2; COBOL untouched · {} ms",
        UBUNTU.len(),
        report.fonts[0].licence.as_ref().map_or("no licence", |l| l.embedding),
        started.elapsed().as_millis()
    );
}
