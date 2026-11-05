// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T32: the `examples/PowerGraphics` project. Its four desktop forms check clean, and the Web project derived from
//! them (the same forms saved as `webforms/*.wcfrm`) builds with the generator; the build ships the light graphics part for
//! the image lab, the middle one for the game and the charts, and the 3D one only where the 3D viewer is - a project made of
//! the first three forms ships no 3D part at all.

use cobolt_forms::web_class::graphics_part_of;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::{Path, PathBuf};

const FORMS: [&str; 4] = ["game-form", "charts-form", "imagelab-form", "viewer3d-form"];

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerGraphics")
}

/// A Web project in a temporary folder holding the desktop forms `which` of the example.
fn derive(tag: &str, which: &[&str]) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("prc-t32-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    let mut list = Vec::new();
    for name in which {
        let form = cobolt_forms::xml::load_form(&example().join(format!("forms/{name}.cfrm"))).unwrap_or_else(|e| panic!("{name}: {e}"));
        cobolt_forms::xml::save_form(&form, &dir.join(format!("webforms/{name}.wcfrm"))).unwrap();
        list.push(format!("\"webforms/{name}.wcfrm\""));
    }
    // The project's own font: the game draws its text in Ubuntu, and a Web build ships every font a form names.
    std::fs::create_dir_all(dir.join("Assets/fonts")).unwrap();
    for f in ["Ubuntu-Light.ttf", "UFL.txt"] {
        std::fs::copy(example().join("Assets/fonts").join(f), dir.join("Assets/fonts").join(f)).unwrap();
    }
    let manifest = dir.join("PowerGraphicsWeb.project.toml");
    std::fs::write(&manifest, format!("[project]\nname = \"PowerGraphicsWeb.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nforms = [{}]\nassets = [\"Assets/fonts/Ubuntu-Light.ttf\", \"Assets/fonts/UFL.txt\"]\n", list.join(", "))).unwrap();
    (dir, manifest)
}

/// The graphics parts a build report says it shipped, by name (`canvas`, `graphics`, `graphics3d`).
fn parts_in(report: &cobolt_project_tools::web::BuildReport) -> Vec<String> {
    let mut v: Vec<String> = report
        .size
        .parts
        .iter()
        .filter_map(|p| p.name.strip_prefix("player/part-"))
        .filter_map(|n| n.split('-').next())
        .filter(|n| cobolt_forms::web_class::GRAPHICS_PARTS.contains(n))
        .map(str::to_owned)
        .collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn the_example_forms_derive_a_web_project_that_ships_the_three_dimensional_part_only_for_its_viewer() {
    let started = std::time::Instant::now();

    // Which part each form asks for.
    let need: Vec<(&str, Option<&str>)> = FORMS
        .iter()
        .map(|n| {
            let f = cobolt_forms::xml::load_form(&example().join(format!("forms/{n}.cfrm"))).unwrap();
            (*n, graphics_part_of(&f.controls))
        })
        .collect();
    assert_eq!(need, [("game-form", Some("graphics")), ("charts-form", Some("graphics")), ("imagelab-form", Some("canvas")), ("viewer3d-form", Some("graphics3d"))]);

    // All four: the build goes through and ships the three parts.
    let (dir, manifest) = derive("all", &FORMS);
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let t = std::time::Instant::now();
    let report = match GeneratorBackend.build(&req) {
        Ok(r) => r,
        Err(WebBuildError::Refused(ds)) => panic!("refused: {:#?}", ds.iter().map(|d| d.to_string()).collect::<Vec<_>>()),
        Err(e) => panic!("{e}"),
    };
    let all_ms = t.elapsed().as_millis();
    let all = parts_in(&report);
    assert!(all.iter().any(|p| p == "graphics3d"), "the 3D viewer's part ships: {all:?}");
    let _ = std::fs::remove_dir_all(&dir);

    // Without the viewer: no 3D part.
    let (dir, manifest) = derive("flat", &FORMS[..3]);
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let flat = parts_in(&report);
    assert!(!flat.iter().any(|p| p == "graphics3d"), "no form needs the 3D part: {flat:?}");
    assert!(flat.iter().any(|p| p == "graphics") && flat.iter().any(|p| p == "canvas"), "{flat:?}");
    let _ = std::fs::remove_dir_all(&dir);

    println!(
        "T32 PowerGraphics on the Web: 4 forms derived and built in {all_ms} ms; parts per form {need:?}; parts shipped with the viewer {all:?}, without it {flat:?}; total {} ms",
        started.elapsed().as_millis()
    );
}
