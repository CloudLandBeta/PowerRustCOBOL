// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T49 (R10, R11, ABI A57): **animation sequences, animations and layout scripts behave
//! on the WebAssembly path as they do under the interpreter.**
//!
//! One responsive form with five labels, two of them carrying a programmatic `vanish` animation (a fade
//! that rests at its end, so a played one is gone from the picture while it stays `Visible`), a panel
//! designed as a flex row holding two labels, three status labels and seven buttons:
//! - `BTN-SEQ` creates `INTRO` (hide `LBL-A` at once, `LBL-B` and the panel's `LBL-P2` after 150 ms, play
//!   `LBL-C`'s `vanish` after 300 ms), starts it with the action and the name in another case and padded,
//!   and shows its status, `Playing`;
//! - `BTN-STATUS` shows the status again once every entry fired: `Stopped`;
//! - `BTN-AGAIN` creates and starts `BACK`, which shows the three hidden labels again;
//! - `BTN-STOP` creates `LATER` (hide `LBL-E` after 400 ms), starts it, stops it and shows `Stopped`,
//!   restarts it and deletes it: `LBL-E` is never hidden;
//! - `BTN-PLAY` plays `LBL-D`'s `vanish` through the control's own `PlayAnimation`;
//! - `BTN-LAYOUT` applies a layout script to the panel (a column with a gap of 6, and a row with a gap of
//!   20 while the window is Compact), the window is made Compact and wide again, and `BTN-CLEAR` takes
//!   the script away;
//! - `BTN-ERR` starts a sequence that was never created. The form survives it, as it survives every
//!   unhandled exception (the operator's ruling of 2026-09-06): both engines keep the interpreter's
//!   message in the form's `LastException`, show it in a Critical notification, and go on.
//!
//! The script runs headless in real time under `rcrun run-form` (the interpreter) and in the program
//! `rcrun build` makes on the WebAssembly path. Every picture is taken after what it shows has come to
//! rest. The dumps must be equal and the pictures equal in every pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::model::{AnimKind, AnimRepeat, AnimTrigger, AnimationDef, EasingKind};
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-MOTION";

/// The message both engines keep in the form's `LastException` and show, at `BTN-ERR`.
const NO_SEQUENCE: &str = "COBOL::AnimationSequence: there is no sequence named \"NOPE\" (Create it first)";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

// ── The fixture ──────────────────────────────────────────────────────────────

/// A handler with its own WORKING-STORAGE items.
fn handler(ws: &[&str], statements: &[&str]) -> String {
    let ws = ws.iter().map(|s| format!("       {s}")).collect::<Vec<_>>().join("\n");
    let body = statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n");
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{ws}\n       PROCEDURE DIVISION.\n{body}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, &str)]) -> usize {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, PropValue::String((*v).into()));
    }
    form.controls.push(c);
    form.controls.len() - 1
}

/// Bind `event` of control `id` to a handler.
fn on(form: &mut Form, id: &str, event: &str, ws: &[&str], statements: &[&str]) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(ws, statements) });
}

/// A fade of 200 ms that a program starts by name, and that rests at its end.
fn vanish() -> AnimationDef {
    AnimationDef {
        name: "vanish".into(),
        trigger: AnimTrigger::Programmatic,
        kind: AnimKind::FadeOut,
        duration_ms: 200,
        delay_ms: 0,
        easing: EasingKind::Linear,
        repeat: AnimRepeat::Once,
        repeat_delay_ms: 0,
        slide_dx: 0,
        slide_dy: 0,
        definition: String::new(),
    }
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let mut form = Form::new(FORM, "Motion", 760, 440);
    form.main_form = true;
    form.responsive = true;
    let label = |form: &mut Form, id: &str, y: i32, caption: &str| {
        control(form, id, ControlType::Label, (20, y, 200, 28), &[("Caption", caption), ("BackgroundColor", "#1E6EDC"), ("ForeColor", "#FFFFFF")])
    };
    label(&mut form, "LBL-A", 20, "Alpha");
    label(&mut form, "LBL-B", 56, "Beta");
    let c = label(&mut form, "LBL-C", 92, "Gamma");
    form.controls[c].animations.push(vanish());
    let d = label(&mut form, "LBL-D", 128, "Delta");
    form.controls[d].animations.push(vanish());
    label(&mut form, "LBL-E", 164, "Echo");
    for (i, id) in ["LBL-ST1", "LBL-ST2", "LBL-ST3"].iter().enumerate() {
        control(&mut form, id, ControlType::Label, (20, 210 + 32 * i as i32, 200, 28), &[("Caption", "")]);
    }
    control(&mut form, "PNL-1", ControlType::Panel, (260, 20, 460, 260), &[("LayoutMode", "Flex"), ("FlexDirection", "Row"), ("BackgroundColor", "#E8ECF4")]);
    for id in ["LBL-P1", "LBL-P2"] {
        let i = control(&mut form, id, ControlType::Label, (0, 0, 120, 40), &[("Caption", id.trim_start_matches("LBL-")), ("BackgroundColor", "#12A150"), ("ForeColor", "#FFFFFF")]);
        form.controls[i].parent = Some("PNL-1".into());
    }
    let buttons = ["BTN-SEQ", "BTN-STATUS", "BTN-AGAIN", "BTN-STOP", "BTN-PLAY", "BTN-LAYOUT", "BTN-CLEAR", "BTN-ERR"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20 + 90 * i as i32, 380, 84, 28), &[("Caption", id.trim_start_matches("BTN-"))]);
    }
    let status = |name: &str, into: &str| [format!("COBOL::\"ANIMATIONSEQUENCE\" ( \"Status\" \"{name}\" WS-ST )"), format!("MOVE WS-ST TO {into}::Caption")];
    let ws = ["01 WS-ST PIC X(8)."];
    let [s1, s2] = status("INTRO", "LBL-ST1");
    on(
        &mut form,
        "BTN-SEQ",
        "onClick",
        &ws,
        &[
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Create\" \"INTRO\" \"HIDE lbl-a AFTER 0; HIDE LBL-B AFTER 150; HIDE LBL-P2 AFTER 150; PLAY LBL-C 'vanish' AFTER 300\" )",
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"start\" \" intro \" )",
            &s1,
            &s2,
        ],
    );
    let [s1, s2] = status("INTRO", "LBL-ST2");
    on(&mut form, "BTN-STATUS", "onClick", &ws, &[&s1, &s2]);
    on(
        &mut form,
        "BTN-AGAIN",
        "onClick",
        &[],
        &["COBOL::\"ANIMATIONSEQUENCE\" ( \"Create\" \"BACK\" \"SHOW LBL-A AFTER 0; SHOW LBL-B AFTER 100; SHOW LBL-P2 AFTER 100\" )", "COBOL::\"ANIMATIONSEQUENCE\" ( \"Start\" \"BACK\" )"],
    );
    let [s1, s2] = status("LATER", "LBL-ST3");
    on(
        &mut form,
        "BTN-STOP",
        "onClick",
        &ws,
        &[
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Create\" \"LATER\" \"HIDE LBL-E AFTER 400\" )",
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Start\" \"LATER\" )",
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Stop\" \"LATER\" )",
            &s1,
            &s2,
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Restart\" \"LATER\" )",
            "COBOL::\"ANIMATIONSEQUENCE\" ( \"Delete\" \"LATER\" )",
        ],
    );
    on(&mut form, "BTN-PLAY", "onClick", &[], &["INVOKE LBL-D \"PlayAnimation\" USING \"vanish\""]);
    on(&mut form, "BTN-LAYOUT", "onClick", &[], &["COBOL::\"LAYOUT\" ( \"Apply\" \"pnl-1\" \"MODE FLEX COLUMN; GAP 6; WHEN Compact; MODE FLEX ROW; GAP 20\" )"]);
    on(&mut form, "BTN-CLEAR", "onClick", &[], &["COBOL::\"LAYOUT\" ( \"Clear\" \"PNL-1\" )"]);
    on(&mut form, "BTN-ERR", "onClick", &[], &["COBOL::\"ANIMATIONSEQUENCE\" ( \"Start\" \"NOPE\" )"]);

    let cfrm = root.join("forms/parity-motion.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-motion.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("paritymotion.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritymotion\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-motion.cfrm\"]\ngenerated = [\"generated/parity-motion.cbl\"]\n",
    )
    .unwrap();
    manifest
}

/// The script, and the names of its pictures in order.
fn script() -> (Value, Vec<&'static str>) {
    let event = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let wait = |ms: u64| json!({"wait_ms": ms});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let size = |w: u32, h: u32| json!({"size": [w, h]});
    let names = vec!["m1-intro", "m2-back-stopped", "m3-played", "m4-layout", "m5-compact", "m6-cleared-compact", "m7-cleared", "m8-refused"];
    let steps = vec![
        wait(300),
        event("BTN-SEQ"),
        wait(1200),
        dump(),
        shot(names[0]),
        event("BTN-STATUS"),
        wait(300),
        dump(),
        event("BTN-AGAIN"),
        wait(800),
        event("BTN-STOP"),
        wait(1000),
        dump(),
        shot(names[1]),
        event("BTN-PLAY"),
        wait(800),
        dump(),
        shot(names[2]),
        event("BTN-LAYOUT"),
        wait(500),
        dump(),
        shot(names[3]),
        size(560, 440),
        wait(500),
        shot(names[4]),
        event("BTN-CLEAR"),
        wait(500),
        shot(names[5]),
        size(760, 440),
        wait(500),
        dump(),
        shot(names[6]),
        event("BTN-ERR"),
        wait(1500),
        dump(),
        shot(names[7]),
    ];
    (Value::Array(steps), names)
}

// ── Running both engines ─────────────────────────────────────────────────────

/// The report line of a headless run.
fn report(stdout: &str) -> Value {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(cobolt_form_host::headless::RESULT_MARKER))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"))
}

/// Every dump of a report, in order.
fn dumps(report: &Value) -> Vec<Value> {
    report["steps"].as_array().map(|s| s.iter().filter(|st| st.get("dump").is_some()).map(|st| st["state"].clone()).collect()).unwrap_or_default()
}

/// Pixels that differ between two pictures (all of them when the sizes differ or a picture is missing).
fn differing_pixels(a: &Path, b: &Path) -> (u64, (u32, u32)) {
    let (Ok(x), Ok(y)) = (image::open(a), image::open(b)) else { return (u64::MAX, (0, 0)) };
    let (x, y) = (x.to_rgba8(), y.to_rgba8());
    if x.dimensions() != y.dimensions() {
        return (u64::from(x.width()) * u64::from(x.height()), x.dimensions());
    }
    (x.pixels().zip(y.pixels()).filter(|(p, q)| p != q).count() as u64, x.dimensions())
}

/// The keys whose values differ between two dumps, as `control: …`.
fn dump_differences(a: &Value, b: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else { return vec!["(not a dump)".into()] };
    let mut ids: Vec<&String> = a.keys().chain(b.keys()).collect();
    ids.sort();
    ids.dedup();
    let shown = |v: Option<&Value>| v.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
    for id in ids {
        if a.get(id) != b.get(id) {
            out.push(format!("{id}: interpreter {} / runner {}", shown(a.get(id)), shown(b.get(id))));
        }
    }
    out
}

/// A control's property in a dump (upper case or as written).
fn prop(dump: &Value, id: &str, name: &str) -> String {
    let props = &dump[id]["props"];
    props.get(name.to_ascii_uppercase()).or_else(|| props.get(name)).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

/// What one engine did: its report, everything it said, and how long it ran.
struct Seen {
    report: Value,
    dumps: Vec<Value>,
    said: String,
    ms: f64,
}

/// What both engines did with the same script.
struct Twin {
    interp: Seen,
    wasm: Seen,
    interp_shots: PathBuf,
    runner_shots: PathBuf,
    runner_ms: f64,
    build_ms: f64,
}

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly path
/// and run it in the built program. The script ends the program with an error, so neither run is held
/// to a zero exit status; each must report.
fn run_twin(root: &Path, manifest: &Path, script: &Path) -> Twin {
    fs::create_dir_all(root.join("home")).unwrap();
    // ── The runner, built beside rcrun, as a release has it ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo).current_dir(&workspace).args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 1. The interpreter: rcrun run-form ──
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-motion.cfrm", "generated/parity-motion.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "180"])
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let said = format!("{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&stdout);
    let interp = Seen { dumps: dumps(&r), report: r, said, ms: t.elapsed().as_secs_f64() * 1000.0 };

    // ── 2. The runner: rcrun build on the WebAssembly path, then the program ──
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(manifest)
        .arg("--quiet")
        .env(cobolt_compiler::BUILD_DIR_ENV, root.join("builds"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env_remove(cobolt_compiler::RUNNER_ENV)
        .output()
        .expect("run rcrun build");
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let build_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{build_out}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(build_out.lines().any(|l| l.trim() == "Path   : WebAssembly"), "the Build took the WebAssembly path:\n{build_out}");
    let exe = root.join("bin").join(format!("paritymotion{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(&exe)
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "180")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let said = format!("{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&stdout);
    let wasm = Seen { dumps: dumps(&r), report: r, said, ms: t.elapsed().as_secs_f64() * 1000.0 };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn sequences_animations_and_layout_scripts_agree_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t49-motion-{}", std::process::id()));
    let manifest = project(&root);
    let (steps, shots) = script();
    let n_dumps = steps.as_array().map(|s| s.iter().filter(|st| st.get("dump").is_some()).count()).unwrap_or(0);
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script);

    // ── The two engines against each other: dumps and pictures ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {n_dumps} times"), twin.interp.dumps.len() == n_dumps && twin.wasm.dumps.len() == n_dumps));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.dumps.iter().zip(twin.wasm.dumps.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} is equal"), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let (mut pixels_total, mut differing_total, mut shot_lines) = (0u64, 0u64, Vec::new());
    for name in &shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&twin.interp_shots.join(&file), &twin.runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str, p: &str| twin.wasm.dumps.get(i).map(|d| prop(d, id, p)).unwrap_or_default();
    let shown = |i: usize, id: &str| twin.wasm.dumps.get(i).and_then(|d| d[id]["visible"].as_bool());
    // The panel's two labels, side by side or one under the other, from the pictures' layout: the dumps
    // carry no geometry, so the layout is judged by the pictures alone (m4 against m7).
    let (layout_diff, _) = differing_pixels(&twin.runner_shots.join("m4-layout.png"), &twin.runner_shots.join("m7-cleared.png"));
    let (compact_diff, _) = differing_pixels(&twin.runner_shots.join("m5-compact.png"), &twin.runner_shots.join("m6-cleared-compact.png"));
    // The refusal's notification: the last picture against the one before the click.
    let (refused_diff, _) = differing_pixels(&twin.runner_shots.join("m7-cleared.png"), &twin.runner_shots.join("m8-refused.png"));
    let last = n_dumps.saturating_sub(1);
    let ended = |s: &Seen| s.report["program_ended"].as_bool() == Some(true);
    let refused = |s: &Seen| s.dumps.get(last).map(|d| prop(d, FORM, "LastException")).unwrap_or_default() == NO_SEQUENCE;
    checks.extend([
        (
            "INTRO: HIDE at once and after 150 ms, from a name in lower case and of a label inside the panel; PLAY leaves LBL-C Visible (the pictures carry its fade); Status while playing answers Playing".to_string(),
            shown(0, "LBL-A") == Some(false) && shown(0, "LBL-B") == Some(false) && shown(0, "LBL-P2") == Some(false) && shown(0, "LBL-C") == Some(true) && at(0, "LBL-ST1", "Caption") == "Playing",
        ),
        ("Status once every entry fired: Stopped".into(), at(1, "LBL-ST2", "Caption") == "Stopped"),
        ("BACK shows the three labels again".into(), shown(2, "LBL-A") == Some(true) && shown(2, "LBL-B") == Some(true) && shown(2, "LBL-P2") == Some(true)),
        ("LATER: Status after Stop answers Stopped; restarted and deleted, it never hides LBL-E".into(), at(2, "LBL-ST3", "Caption") == "Stopped" && shown(2, "LBL-E") == Some(true)),
        ("PlayAnimation on a control leaves it Visible".into(), shown(3, "LBL-D") == Some(true)),
        ("Layout Apply moves the panel's labels; Clear puts them back (the pictures differ, wide and Compact)".into(), layout_diff > 0 && compact_diff > 0),
        (format!("a sequence never created is refused with: {NO_SEQUENCE} (the form's LastException, on both engines)"), refused(&twin.interp) && refused(&twin.wasm)),
        ("the refusal shows a notification (m8 against m7) and both programs go on".into(), refused_diff > 0 && !ended(&twin.interp) && !ended(&twin.wasm)),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== sequences_animations_and_layout_scripts_agree_on_both_engines (DESKTOP-WASM-001 T49: R10, R11, A57) ==");
    println!("form {FORM}: 5 labels (2 with a programmatic fade), a flex panel holding 2 labels, 3 status labels, 8 buttons, 8 event handlers");
    println!("  AnimationSequence Create, Start (another case, padded), Status (Playing, Stopped), Stop, Restart, Delete; SHOW, HIDE and PLAY entries;");
    println!("  PlayAnimation on a control; Layout Apply (every breakpoint and WHEN Compact) and Clear, at 760 and 560 px; Start of an unknown sequence");
    println!("real time; {n_dumps} dumps and {} pictures per engine", shots.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    println!("layout against cleared: {layout_diff} pixels differ wide, {compact_diff} Compact; the refusal's notification: {refused_diff} pixels");
    for l in &dump_lines {
        println!("{l}");
    }
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    if failed.is_empty() {
        let _ = fs::remove_dir_all(&root);
    } else {
        for (who, s) in [("interpreter", &twin.interp), ("runner", &twin.wasm)] {
            let tail: String = s.said.lines().filter(|l| !l.starts_with(cobolt_form_host::headless::RESULT_MARKER)).collect::<Vec<_>>().join("\n");
            println!("  {who} said:\n{}", tail.chars().take(3000).collect::<String>());
            println!("  {who} report end: ended {:?}, close {:?}", s.report["program_ended"], s.report.get("close"));
        }
        for i in 0..n_dumps {
            println!(
                "    dump {i}: A {:?} B {:?} C {:?} D {:?} E {:?}, st1 {:?} st2 {:?} st3 {:?}",
                shown(i, "LBL-A"),
                shown(i, "LBL-B"),
                shown(i, "LBL-C"),
                shown(i, "LBL-D"),
                shown(i, "LBL-E"),
                at(i, "LBL-ST1", "Caption"),
                at(i, "LBL-ST2", "Caption"),
                at(i, "LBL-ST3", "Caption")
            );
        }
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
