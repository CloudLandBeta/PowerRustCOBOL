// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `build` for real (spec 080 AC6, build half). Ignored by default: it needs
//! cargo and the SDK and takes minutes. Run it with
//! `cargo test -p cobolt-project-tools --test build -- --ignored --nocapture`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cobolt_project_tools::{FileList, HeadlessHost, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::json;

/// The headless host, with this source checkout as the SDK root (a test
/// executable lives in a target folder the compiler's own search would not
/// find the workspace from).
struct WithSdk(HeadlessHost);

impl ProjectHost for WithSdk {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        self.0.project()
    }
    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        self.0.record(rel, list)
    }
    fn workspace_root(&self) -> Option<PathBuf> {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap())
    }
    fn external_crates(&self) -> Vec<String> {
        Vec::new()
    }
    fn version(&self) -> String {
        "test".into()
    }
}

#[test]
#[ignore = "needs cargo and the SDK; takes minutes"]
fn build_produces_the_binary() {
    let dir = std::env::temp_dir().join(format!("prc-080-realbuild-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("forms")).unwrap();
    std::fs::write(
        dir.join("BuildDemo.project.toml"),
        "[project]\nname = \"BuildDemo\"\nstructure = 1\nversion = \"1.0.0\"\nmain = \"\"\n\
         [files]\nforms = [\"forms/MAIN.cfrm\"]\n",
    )
    .unwrap();
    let mut form = cobolt_forms::Form::new("MAIN", "Build demo", 320, 200);
    form.main_form = true;
    cobolt_forms::save_form(&form, &dir.join("forms/MAIN.cfrm")).unwrap();

    let mut tools = ProjectTools::new(WithSdk(HeadlessHost::new(&dir, "test")));
    let started = Instant::now();
    let mut calls = 0;
    let last = loop {
        let v = tools.call("build", &json!({})).unwrap();
        calls += 1;
        println!("  call {calls}: {v}");
        if v["status"] != "running" {
            break v;
        }
        assert!(started.elapsed() < Duration::from_secs(60 * 60), "build never finished");
    };
    assert_eq!(last["status"], "built", "{last}");
    let rel = last["binary"].as_str().unwrap();
    let bin = dir.join(rel);
    let size = std::fs::metadata(&bin).unwrap().len();
    assert!(size > 0);
    println!(
        "build: {} in {:.1} s over {calls} call(s); binary {rel}, {size} bytes",
        last["status"],
        started.elapsed().as_secs_f64()
    );
}
