// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Regenerate what the IDE would, for the PowerChat example (spec 071), so
//! the example can be written and maintained without driving the IDE:
//!
//! 1. every form in the manifest is loaded and saved back with the defaults
//!    `Control::new` gives each control filled in (a hand-written `.cfrm`
//!    carries only the properties it cares about);
//! 2. its program is generated into `generated/<form>.cbl`;
//! 3. every `forms/*.menu.yaml` is re-saved, which writes its integrity hash;
//! 4. the manifest's `main-form-seal` is recomputed.
//!
//! `cargo run -p cobolt-ide --example powerchat_regen` — then commit the
//! result. `crates/cobolt-ide/tests/powerchat_compiles.rs` fails when any of
//! it is stale.

use std::path::{Path, PathBuf};

fn project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerChat")
}

fn fill_defaults(controls: &mut [cobolt_forms::Control]) {
    for c in controls.iter_mut() {
        let fresh = cobolt_forms::Control::new(c.id.clone(), c.control_type.clone(), c.rect.x as i32, c.rect.y as i32);
        for (k, v) in fresh.properties {
            if !c.properties.contains_key(&k) {
                c.properties.insert(k, v);
            }
        }
        fill_defaults(&mut c.children);
    }
}

fn main() {
    let dir = project_dir();
    let manifest_path = dir.join("PowerChat.project.toml");
    let manifest_text = std::fs::read_to_string(&manifest_path).expect("PowerChat.project.toml");
    let manifest: toml::Value = toml::from_str(&manifest_text).expect("manifest parses");
    let project_name = manifest["project"]["name"].as_str().expect("[project] name").to_string();
    let forms: Vec<String> = manifest["files"]["forms"]
        .as_array()
        .expect("[files] forms")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let mut main_id = None;
    let mut form_ids = Vec::new();
    for rel in &forms {
        let path = dir.join(rel);
        let mut form = cobolt_forms::load_form(&path).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        fill_defaults(&mut form.controls);
        cobolt_forms::save_form(&form, &path).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let form = cobolt_forms::load_form(&path).expect("reloads");
        let id = cobolt_compiler::main_form_guard::form_id(&path);
        if form.main_form {
            main_id = Some(id.clone());
        }
        form_ids.push(id);
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let cbl = dir.join("generated").join(format!("{stem}.cbl"));
        std::fs::create_dir_all(cbl.parent().unwrap()).unwrap();
        std::fs::write(&cbl, cobolt_codegen::generate(&form)).unwrap();
        println!("  form {rel} → generated/{stem}.cbl");
    }

    for entry in std::fs::read_dir(dir.join("forms")).unwrap().flatten() {
        let path = entry.path();
        if path.to_string_lossy().ends_with(".menu.yaml") {
            let text = std::fs::read_to_string(&path).unwrap();
            let def: cobolt_forms::menu::MenuDefinition =
                serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            cobolt_forms::menu::save_menu(&path, &def).unwrap();
            println!("  menu {} sealed", path.file_name().unwrap().to_string_lossy());
        }
    }

    let main_id = main_id.expect("one form carries main-form=\"true\"");
    let seal = cobolt_compiler::main_form_guard::seal(&project_name, &main_id, &form_ids);
    let mut out = String::new();
    for line in manifest_text.lines() {
        if line.trim_start().starts_with("main-form-seal") {
            out.push_str(&format!("main-form-seal = \"{seal}\""));
        } else if line.trim_start().starts_with("main-form ") || line.trim_start().starts_with("main-form=") {
            out.push_str(&format!("main-form = \"{main_id}\""));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    std::fs::write(&manifest_path, out).unwrap();
    println!("  main form {main_id}, seal {}…", &seal[..12]);
}
