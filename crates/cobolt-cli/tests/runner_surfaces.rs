// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T35: **the same form on every surface** (R9; AC14).
//!
//! The reference form of `engine_reference_form_parity_static_vs_faces` (a Panel holding an
//! AreaChart, a PictureBox and a TextBox, and a Label beside it, on a 400 by 300 form) is pictured
//! as it opens, three ways:
//!
//! 1. by the designer's render path: `cobolt_forms::snapshot::render_form_image`, the picture the
//!    designer's preview and the coding agent's `render_form` tool make, with no program running;
//! 2. under `rcrun run-form`, the interpreter, as the designer's Run Form starts it;
//! 3. in the program `rcrun build` makes on the WebAssembly path: the runner.
//!
//! The three pictures must be equal in every pixel. The geometry half of AC14 (the designer's
//! canvas entry `render_faces` against `render_form`) is the guard itself, in `cobolt-forms`.
//!
//! The form has no handler: what is compared is how each surface draws the design, so nothing a
//! program does at open may change it. The runs are pinned (clock and seed) as T30's are.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "SURFACES";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn control(id: &str, kind: ControlType, rect: (i32, i32, i32, i32), parent: Option<&str>, props: &[(&str, PropValue)]) -> Control {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    c.parent = parent.map(str::to_owned);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    c
}

/// The reference form's project in `root`: the form, its generated program, a main program that
/// only ends, and the project file. The form's path and the manifest's.
fn project(root: &Path) -> (PathBuf, PathBuf) {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "Surfaces", 400, 300);
    form.main_form = true;
    form.controls = vec![
        control("PNL-1", ControlType::Panel, (10, 10, 300, 200), None, &[]),
        control("CHT-1", ControlType::AreaChart, (20, 30, 120, 80), Some("PNL-1"), &[]),
        control("PIC-1", ControlType::PictureBox, (20, 120, 60, 60), Some("PNL-1"), &[]),
        control("TXT-1", ControlType::TextBox, (150, 40, 120, 24), Some("PNL-1"), &[("Text", s("Ann"))]),
        control("LBL-1", ControlType::Label, (10, 230, 100, 20), None, &[("Caption", s("Reference"))]),
    ];
    let cfrm = root.join("forms/surfaces.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/surfaces.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("surfaces.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"surfaces\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/surfaces.cfrm\"]\ngenerated = [\"generated/surfaces.cbl\"]\n",
    )
    .unwrap();
    (cfrm, manifest)
}

/// The report line of a headless run.
fn report(stdout: &str) -> Value {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(cobolt_form_host::headless::RESULT_MARKER))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"))
}

/// Pixels that differ between two pictures (all of them when the sizes differ or a picture is
/// missing), and the first picture's size.
fn differing_pixels(a: &Path, b: &Path) -> (u64, (u32, u32)) {
    let (Ok(x), Ok(y)) = (image::open(a), image::open(b)) else { return (u64::MAX, (0, 0)) };
    let (x, y) = (x.to_rgba8(), y.to_rgba8());
    if x.dimensions() != y.dimensions() {
        return (u64::from(x.width()) * u64::from(x.height()), x.dimensions());
    }
    (x.pixels().zip(y.pixels()).filter(|(p, q)| p != q).count() as u64, x.dimensions())
}

/// Pixels that are not fully transparent: the picture holds the form, not nothing.
fn inked_pixels(png: &Path) -> u64 {
    image::open(png).map(|i| i.to_rgba8().pixels().filter(|p| p.0[3] != 0).count() as u64).unwrap_or(0)
}

#[test]
fn the_same_form_draws_alike_in_the_designer_rcrun_and_the_runner() {
    let root = std::env::temp_dir().join(format!("prc-t35-{}", std::process::id()));
    let (cfrm, manifest) = project(&root);
    let steps = json!([{"wait_ms": 200}, {"shot": "open"}]);
    let script_path = root.join("steps.json");
    fs::write(&script_path, steps.to_string()).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();

    // ── The runner, built beside rcrun, as a release has it ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 1. The designer's render path: the form as it opens, no program ──
    let t = Instant::now();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    let image = cobolt_forms::snapshot::render_form_image(&form, &cobolt_forms::snapshot::SnapshotOptions::default());
    let designer_png = root.join("designer.png");
    fs::write(&designer_png, cobolt_forms::raster::to_png(&image).unwrap()).unwrap();
    let designer_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 2. The interpreter: rcrun run-form, as the designer's Run Form starts it ──
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/surfaces.cfrm", "generated/surfaces.cbl", "--designer", "--headless"])
        .arg(&script_path)
        .args(["--headless-limit", "60"])
        .current_dir(&root)
        .env("HOME", &home)
        .env("PRC_TEST_CLOCK", PIN_CLOCK)
        .env("PRC_TEST_SEED", PIN_SEED)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;
    let interp_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{interp_out}\n{}", String::from_utf8_lossy(&out.stderr));
    let interp = report(&interp_out);

    // ── 3. The runner: rcrun build on the WebAssembly path, then the program ──
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(&manifest)
        .arg("--quiet")
        .env(cobolt_compiler::BUILD_DIR_ENV, root.join("builds"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env_remove(cobolt_compiler::RUNNER_ENV)
        .output()
        .expect("run rcrun build");
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let build_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{build_out}\n{}", String::from_utf8_lossy(&out.stderr));
    let wasm_path = build_out.lines().any(|l| l.trim() == "Path   : WebAssembly");
    let exe = root.join("bin").join(format!("surfaces{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(&exe)
        .current_dir(&root)
        .env("HOME", &home)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_TEST_CLOCK", PIN_CLOCK)
        .env("PRC_TEST_SEED", PIN_SEED)
        .env("PRC_HEADLESS_SCRIPT", &script_path)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    let wasm_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{wasm_out}\n{}", String::from_utf8_lossy(&out.stderr));
    let wasm = report(&wasm_out);

    // ── The comparison ──
    let (interp_png, runner_png) = (interp_shots.join("open.png"), runner_shots.join("open.png"));
    let pairs = [
        ("the designer's picture and rcrun run-form's", &designer_png, &interp_png),
        ("rcrun run-form's and the runner's", &interp_png, &runner_png),
        ("the designer's picture and the runner's", &designer_png, &runner_png),
    ];
    let mut checks: Vec<(String, bool)> = vec![
        ("rcrun build took the WebAssembly path".into(), wasm_path),
        ("neither run reached its limit".into(), interp["timed_out"] != json!(true) && wasm["timed_out"] != json!(true)),
    ];
    let inked = inked_pixels(&designer_png);
    checks.push((format!("the designer's picture holds the form ({inked} pixels inked)"), inked > 0));
    let mut lines = Vec::new();
    let mut compared = 0u64;
    for (what, a, b) in pairs {
        let (diff, (w, h)) = differing_pixels(a, b);
        compared += u64::from(w) * u64::from(h);
        let shown = if diff == u64::MAX { "missing".to_string() } else { diff.to_string() };
        lines.push(format!("    {what}: {w}x{h}, {shown} differing pixels"));
        checks.push((format!("{what}: 0 differing pixels"), diff == 0));
    }

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_same_form_draws_alike_in_the_designer_rcrun_and_the_runner (DESKTOP-WASM-001 T35: R9; AC14) ==");
    println!("form {FORM}, 400x300: Panel PNL-1 holding AreaChart CHT-1, PictureBox PIC-1 and TextBox TXT-1; Label LBL-1 (the guard's reference form); no handler");
    println!("pins: PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}; script: wait 200 ms, one picture");
    println!("runner built in {runner_ms:.0} ms; rcrun build (WebAssembly path) {build_ms:.0} ms");
    println!("designer picture {designer_ms:.0} ms; interpreter run {interp_ms:.0} ms; runner run {wasm_ms:.0} ms");
    for l in &lines {
        println!("{l}");
    }
    println!("pixels compared: {compared}");
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    if failed.is_empty() {
        let _ = fs::remove_dir_all(&root);
    } else {
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
