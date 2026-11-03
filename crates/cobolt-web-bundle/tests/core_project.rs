// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The scenario project of WASM-017 T11 (`tests/projects/core/`), built by the product's own Web Build.

mod support;

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};

#[test]
fn the_scenario_project_builds_to_a_bundle() {
    let dir = support::scenario_project("probe");
    let req = WebBuildRequest::from_manifest(&dir.join("Core.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    for p in report.size.parts.iter().filter(|p| p.name.contains("program")) {
        println!("{}: {} bytes", p.name, p.bytes);
    }
    println!("bundle at {}", report.bundle_dir.display());
    assert!(report.bundle_dir.join("index.html").is_file());
}
