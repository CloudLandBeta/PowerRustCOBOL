// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096 (R12; AC10): **a program tilts a chart the way it changes any other
//! property — and gets the same answers on the three hosts that run a form**:
//! `rcrun run-form`, a child form loaded into an application shell's
//! ContentPane, and a built binary (`--ignored`: it compiles one).
//!
//! Spec 097 adds `PerspectiveStrength`: written with 40 (which must arrive), then
//! with 150 (which must come back as 100, the top of its range). Spec 098 adds the
//! corner pin: `PinTopLeftY` written with 24, `PinBottomRightX` with 20000 (→ 9999).
//!
//! One form with a BarChart and buttons. One handler writes
//! `CHART-1::RotateY` = 30 and `CHART-1::RotateX` = 200 — a value outside the
//! range, which must be brought into it (60) wherever it is written; the other
//! reads `RotateX` back into a label. The test prints each host's readings as a
//! row, so a divergence is visible to the eye as well as to the assertion; and,
//! under Run Form, takes the picture before and after the click and counts the
//! pixels of the chart that changed — the chart really is drawn with the new
//! outline on the next frame.

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_forms::model::FormFormat;
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue};
use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// A handler body in the form the designer stores one.
fn handler(statements: &[&str]) -> String {
    let body = statements.join("\n           ");
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           {body}\n")
}

fn button(form: &mut Form, id: &str, y: i32, statements: &[&str]) {
    let mut c = Control::new(id, ControlType::Button, 360, y);
    c.rect = cobolt_forms::model::Rect::new(360, y, 160, 28);
    c.set_prop("Caption", PropValue::String(id.into()));
    c.events.push(EventBinding {
        event: "onClick".into(),
        paragraph: format!("{id}--ONCLICK"),
        code: handler(statements),
    });
    form.controls.push(c);
}

/// The form every host runs.
fn chart_form() -> Form {
    let mut f = Form::new("PERSPECTIVE-DEMO", "Perspective", 640, 380);
    // Loadable as a window and into a shell's pane, so every host can run it.
    f.form_format = FormFormat::Both;
    let mut chart = Control::new("CHART-1", ControlType::BarChart, 20, 20);
    chart.rect = cobolt_forms::model::Rect::new(20, 20, 320, 220);
    f.controls.push(chart);
    button(
        &mut f,
        "BTN-TILT",
        20,
        &[
            "MOVE 30 TO CHART-1::RotateY",
            "MOVE 200 TO CHART-1::RotateX",
            "MOVE 40 TO CHART-1::PerspectiveStrength",
            "MOVE 24 TO CHART-1::PinTopLeftY",
        ],
    );
    button(
        &mut f,
        "BTN-CLAMP",
        92,
        &["MOVE 150 TO CHART-1::PerspectiveStrength", "MOVE 20000 TO CHART-1::PinBottomRightX"],
    );
    button(&mut f, "BTN-READ", 56, &["MOVE CHART-1::RotateX TO LBL-RESULT::Caption"]);
    let mut result = Control::new("LBL-RESULT", ControlType::Label, 360, 130);
    result.rect = cobolt_forms::model::Rect::new(360, 130, 200, 24);
    result.set_prop("Caption", PropValue::String("-".into()));
    f.controls.push(result);
    f
}

/// A project holding the form and the shell the embedded run loads it into,
/// with their generated programs.
fn project(tag: &str) -> Project {
    let root = std::env::temp_dir().join(format!("prc-096-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let form = chart_form();
    cobolt_forms::save_form(&form, &root.join("forms/perspective-demo.cfrm")).unwrap();
    std::fs::write(root.join("generated/perspective-demo.cbl"), cobolt_codegen::generate(&form)).unwrap();
    // The shell: any application shell works, so borrow the AWS demos'.
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    for file in ["aws-shell.cfrm", "SideMenu-1.menu.yaml"] {
        std::fs::copy(shell.join(file), root.join("forms").join(file)).unwrap();
    }
    let shell_form = cobolt_forms::load_form(&root.join("forms/aws-shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/aws-shell.cbl"), cobolt_codegen::generate(&shell_form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = "[project]\nname = \"PerspectiveHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/perspective-demo.cfrm\", \"forms/aws-shell.cfrm\"]\n\
         generated = [\"generated/perspective-demo.cbl\", \"generated/aws-shell.cbl\"]\n";
    std::fs::write(root.join("perspectivehosts.project.toml"), manifest).unwrap();
    Project { root }
}

/// One headless step list. `tilt` clicks the button that writes the properties;
/// `open` loads the form into the shell's pane first.
fn steps(open: Option<&str>, tilt: bool) -> Value {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let read = |ctrl: &str, prop: &str| json!({"read": {"control": ctrl, "property": prop}});
    let wait = || json!({"wait_ms": 500});
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    // R2 — the neutral values.
    s.extend([
        read("CHART-1", "RotateX"),
        read("CHART-1", "RotateY"),
        read("CHART-1", "PerspectiveDistance"),
        read("CHART-1", "PerspectiveStrength"),
        read("CHART-1", "PinTopLeftY"),
    ]);
    if tilt {
        s.extend([
            click("BTN-TILT"),
            wait(),
            // R12 — a write is read back; the one outside the range is clamped.
            read("CHART-1", "RotateY"),
            read("CHART-1", "RotateX"),
            // Spec 097 — a strength of 40 arrives…
            read("CHART-1", "PerspectiveStrength"),
            // Spec 098 — and a pin of 24.
            read("CHART-1", "PinTopLeftY"),
            click("BTN-READ"),
            wait(),
            // …and COBOL reads the property as it reads any other.
            read("LBL-RESULT", "Caption"),
            // …and one of 150 is brought back into the range, as is a pin of 20000.
            click("BTN-CLAMP"),
            wait(),
            read("CHART-1", "PerspectiveStrength"),
            read("CHART-1", "PinBottomRightX"),
        ]);
    }
    Value::Array(s)
}

const WANT: [&str; 12] = ["0", "0", "50", "100", "0", "30", "60", "40", "24", "60", "100", "9999"];
const READS: [&str; 12] = [
    "CHART-1::RotateX at start",
    "CHART-1::RotateY at start",
    "CHART-1::PerspectiveDistance at start",
    "CHART-1::PerspectiveStrength at start",
    "CHART-1::PinTopLeftY at start",
    "RotateY after MOVE 30",
    "RotateX after MOVE 200 (clamped)",
    "PerspectiveStrength after MOVE 40",
    "PinTopLeftY after MOVE 24",
    "COBOL reads CHART-1::RotateX into a label",
    "PerspectiveStrength after MOVE 150 (clamped)",
    "PinBottomRightX after MOVE 20000 (clamped)",
];

fn readings(stdout: &str) -> Vec<String> {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    report["steps"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .filter_map(|st| st["value"].as_str().map(|v| v.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn run_form(p: &Project, form: &str, program: &str, script: &Value, png: Option<&Path>) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let mut cmd = Command::new(rcrun());
    cmd.args(["run-form", form, program, "--designer", "--headless", script_path.to_str().unwrap(), "--headless-limit", "60"]);
    if let Some(png) = png {
        cmd.args(["--headless-png", png.to_str().unwrap()]);
    }
    let out = cmd.current_dir(&p.root).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

/// What the script reads, in order: [`WANT`] as owned strings.
fn want() -> Vec<String> {
    WANT.iter().map(|s| s.to_string()).collect()
}

fn report(host: &str, got: &[String]) {
    println!("\n  096 AC10 — {host}");
    for (i, w) in WANT.iter().enumerate() {
        let g = got.get(i).map(String::as_str).unwrap_or("(missing)");
        println!("    {:<46} {:<8} {}", READS[i], g, if g == *w { "ok" } else { "DIFFERS" });
    }
}

fn decode(png: &Path) -> image::RgbaImage {
    image::open(png).unwrap_or_else(|e| panic!("{}: {e}", png.display())).to_rgba8()
}

#[test]
fn a_program_tilts_a_chart_under_run_form_and_as_an_embedded_child_form() {
    let p = project("rf");

    // The neutral picture and the tilted one, from the same form under Run Form.
    let before_png = p.root.join("before.png");
    let after_png = p.root.join("after.png");
    run_form(&p, "forms/perspective-demo.cfrm", "generated/perspective-demo.cbl", &steps(None, false), Some(&before_png));
    let direct = readings(&run_form(
        &p,
        "forms/perspective-demo.cfrm",
        "generated/perspective-demo.cbl",
        &steps(None, true),
        Some(&after_png),
    ));
    report("rcrun run-form", &direct);
    assert_eq!(direct, want(), "under Run Form");

    // The chart is drawn with the new outline on the next frame: its region of the picture changed.
    let (a, b) = (decode(&before_png), decode(&after_png));
    assert_eq!((a.width(), a.height()), (b.width(), b.height()), "the picture keeps its size");
    let mut changed = 0usize;
    let mut total = 0usize;
    for y in 20..240u32.min(a.height()) {
        for x in 20..340u32.min(a.width()) {
            total += 1;
            let (pa, pb) = (a.get_pixel(x, y), b.get_pixel(x, y));
            if pa.0.iter().zip(pb.0.iter()).any(|(u, v)| (*u as i32 - *v as i32).abs() > 24) {
                changed += 1;
            }
        }
    }
    println!(
        "    the chart's region of the picture: {changed} of {total} pixels differ after the tilt ({:.1} %)",
        changed as f64 * 100.0 / total as f64
    );
    assert!(changed * 100 / total.max(1) >= 5, "the chart looks the same after RotateY 30 / RotateX 60");

    let embedded = readings(&run_form(
        &p,
        "forms/aws-shell.cfrm",
        "generated/aws-shell.cbl",
        &steps(Some("perspective-demo"), true),
        None,
    ));
    report("embedded child form (the shell's ContentPane)", &embedded);
    assert_eq!(embedded, want(), "as an embedded child form");
}

/// The built binary gives the same answers. Builds one: `--ignored`.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn a_program_tilts_a_chart_in_a_built_binary() {
    let p = project("bin");
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    // Unoptimised: this proves behaviour, not speed.
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
    let built = cobolt_compiler::build_project(&p.root.join("perspectivehosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let script = p.root.join("bin-steps.json");
    std::fs::write(&script, steps(Some("perspective-demo"), true).to_string()).unwrap();
    let out = Command::new(&built.binary_path)
        .env("PRC_HEADLESS_SCRIPT", &script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let got = readings(&stdout);
    report("the built binary", &got);
    assert_eq!(got, want(), "in the built binary");
}
