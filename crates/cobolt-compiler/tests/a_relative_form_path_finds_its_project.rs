// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run-form forms/main.cfrm`, run in the project folder, gives the
//! form as a path relative to the working folder. Climbing its ancestors
//! ends at an EMPTY path — the working folder itself, where the manifest
//! is — which `read_dir` cannot open, so the manifest was never found and no
//! child form could find its generated program (found running PowerSpatial).
//!
//! Its own test binary: it changes the working folder, which no other test
//! in the process may see.

use std::path::Path;

#[test]
fn a_relative_form_path_finds_the_manifest_in_the_working_folder() {
    let dir = std::env::temp_dir().join(format!("prc-relative-manifest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("forms")).unwrap();
    std::fs::create_dir_all(dir.join("generated")).unwrap();
    std::fs::write(
        dir.join("Demo.project.toml"),
        "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\nmain = \"\"\n\n[files]\n\
         forms = [\"forms/toolbar-form.cfrm\"]\ngenerated = [\"generated/toolbar-form.cbl\"]\n",
    )
    .unwrap();
    std::fs::write(dir.join("forms/toolbar-form.cfrm"), "<Form/>").unwrap();
    std::fs::write(dir.join("generated/toolbar-form.cbl"), "x").unwrap();

    std::env::set_current_dir(&dir).unwrap();
    let cfrm = Path::new("forms/toolbar-form.cfrm");
    let manifest = cobolt_compiler::find_project_manifest(cfrm).expect("the manifest in the working folder");
    assert!(manifest.ends_with("Demo.project.toml"), "{manifest:?}");
    let program = cobolt_compiler::form_program_path(cfrm, "TOOLBAR-FORM").expect("its generated program");
    assert!(program.ends_with("generated/toolbar-form.cbl"), "{program:?}");
    println!("relative forms/toolbar-form.cfrm -> {manifest:?}, program {program:?}");

    std::env::set_current_dir(std::env::temp_dir()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
