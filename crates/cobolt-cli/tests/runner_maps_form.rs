// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11): **the Maps control behaves on the WebAssembly path as it does under
//! the interpreter.**
//!
//! One form with a Maps control over four tiles a test folder holds (`PRC_TEST_MAP_TILES`: no tile
//! server is reached), a marker designed at its centre, a ListBox its events write to, and five buttons:
//! - `BTN-COLL` adds markers (one with all five arguments, one with four), adds one and removes it,
//!   adds two routes and replaces one by its id, removes the other, adds a region with five arguments
//!   and one with seven, and removes the first;
//! - `BTN-CLEAR` clears the routes and the regions;
//! - `BTN-NOKEY` asks for a place with no Google key (none is in the environment) and for a road
//!   route with no OpenRouteService key: neither sends anything, each writes its reason in `LastError`
//!   at once (the handler logs it) and raises `onError`, whose handlers run after the click's and so
//!   both read the last reason;
//! - `BTN-ASYNC` asks for a road route whose origin is no number, which runs on a thread of its own
//!   and fails there before any request is made: the control is busy at once, a place asked for
//!   while it runs is ignored (before its missing key is looked at), and the ending raises `onError`;
//! - `BTN-CANCEL` asks for the same road route and cancels it: `onCancelled`, not busy, and the ending
//!   is dropped.
//!
//! Then a real pointer click on the map's centre selects the designed marker and opens its card.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path. The dumps must be equal and the pictures equal in every pixel.
//!
//! What it does not reach: a Google service or OpenRouteService answering. A test never sends a request
//! to a real provider, and the answer's road is a RestClient's ending (`rest_door::delivered`), which
//! the RestClient twin covers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-MAPS";

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

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, &str)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, PropValue::String((*v).into()));
    }
    form.controls.push(c);
}

/// Bind `event` of control `id` to a handler.
fn on(form: &mut Form, id: &str, event: &str, ws: &[&str], statements: &[&str]) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(ws, statements) });
}

/// The four tiles of zoom 2 the map shows, in four colours with a grid, so a tile drawn in the wrong
/// place, or not at all, changes the picture.
fn tiles(dir: &Path) {
    for (x, y, rgb) in [(1u32, 1u32, [214u8, 232, 214]), (2, 1, [214, 222, 240]), (1, 2, [240, 228, 206]), (2, 2, [230, 214, 236])] {
        let img = image::RgbaImage::from_fn(256, 256, |px, py| {
            if px % 32 == 0 || py % 32 == 0 {
                image::Rgba([120, 120, 120, 255])
            } else {
                image::Rgba([rgb[0], rgb[1], rgb[2], 255])
            }
        });
        let file = dir.join("2").join(x.to_string()).join(format!("{y}.png"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        img.save(&file).unwrap();
    }
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    tiles(&root.join("tiles"));
    let mut form = Form::new(FORM, "Maps", 900, 420);
    form.main_form = true;
    control(&mut form, "MAP-1", ControlType::Maps, (20, 20, 360, 240), &[("CenterLat", "0"), ("CenterLng", "0"), ("Zoom", "2"), ("Markers", "M0\t0\t0\tOrigin\tthe centre")]);
    control(&mut form, "LST-LOG", ControlType::ListBox, (400, 20, 480, 240), &[]);
    control(&mut form, "LBL-ASYNC", ControlType::Label, (400, 270, 480, 24), &[("Caption", "")]);
    control(&mut form, "LBL-CANCEL", ControlType::Label, (400, 300, 480, 24), &[("Caption", "")]);
    for (i, id) in ["BTN-COLL", "BTN-CLEAR", "BTN-NOKEY", "BTN-ASYNC", "BTN-CANCEL"].iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20 + 72 * i as i32, 280, 64, 28), &[("Caption", id.trim_start_matches("BTN-"))]);
    }
    // The control's events write to the log, in the order they come.
    on(&mut form, "MAP-1", "onError", &[], &["INVOKE LST-LOG \"AddItem\" USING MAP-1::LastError"]);
    on(&mut form, "MAP-1", "onCancelled", &[], &["INVOKE LST-LOG \"AddItem\" USING \"cancelled\""]);
    on(&mut form, "MAP-1", "onComplete", &[], &["INVOKE LST-LOG \"AddItem\" USING \"complete\""]);
    on(&mut form, "MAP-1", "onMarkerClick", &[], &["INVOKE LST-LOG \"AddItem\" USING MAP-1::SelectedMarkerId"]);
    on(
        &mut form,
        "BTN-COLL",
        "onClick",
        &[],
        &[
            "INVOKE MAP-1 \"AddMarker\" USING \"M1\" \"10\" \"20\" \"One\" \"first\"",
            "INVOKE MAP-1 \"AddMarker\" USING \"M2\" \"-10\" \"-20\" \"Two\"",
            "INVOKE MAP-1 \"AddMarker\" USING \"MX\" \"30\" \"30\" \"Gone\" \"\"",
            "INVOKE MAP-1 \"RemoveMarker\" USING \"MX\"",
            "INVOKE MAP-1 \"AddRoute\" USING \"R1\" \"#E5484D\" \"3\" \"0,0;10,20;-10,-20\"",
            "INVOKE MAP-1 \"AddRoute\" USING \"R2\" \"#1E6EDC\" \"2\" \"5,5;6,6\"",
            "INVOKE MAP-1 \"AddRoute\" USING \"R1\" \"#12A150\" \"4\" \"0,0;20,40\"",
            "INVOKE MAP-1 \"RemoveRoute\" USING \"R2\"",
            "INVOKE MAP-1 \"AddRegion\" USING \"Z1\" \"#E5484D55\" \"#E5484D\" \"2\" \"0,0;10,0;10,10;0,10\"",
            "INVOKE MAP-1 \"AddRegion\" USING \"Z2\" \"#12A15055\" \"#12A150\" \"1\" \"-5,-5;-15,-5;-15,-15\" \"South\" \"a region\"",
            "INVOKE MAP-1 \"AddRegion\" USING \"Z3\" \"#1E6EDC55\" \"#1E6EDC\" \"1\" \"20,-30;25,-30;25,-25\"",
            "INVOKE MAP-1 \"RemoveRegion\" USING \"Z1\"",
        ],
    );
    on(&mut form, "BTN-CLEAR", "onClick", &[], &["INVOKE MAP-1 \"ClearRoutes\"", "INVOKE MAP-1 \"ClearRegions\""]);
    on(
        &mut form,
        "BTN-NOKEY",
        "onClick",
        &[],
        &[
            "INVOKE MAP-1 \"Geocode\" USING \"Paris\"",
            "INVOKE LST-LOG \"AddItem\" USING MAP-1::LastError",
            "INVOKE MAP-1 \"TraceRoad\" USING \" \" \"1\" \"2\" \"3\" \"4\"",
            "INVOKE LST-LOG \"AddItem\" USING MAP-1::LastError",
        ],
    );
    on(
        &mut form,
        "BTN-ASYNC",
        "onClick",
        &["01 WS-B1 PIC X.", "01 WS-B2 PIC X(5).", "01 WS-L PIC X(60)."],
        &[
            "INVOKE MAP-1 \"TraceRoad\" USING \"test-key\" \"abc\" \"0\" \"1\" \"1\"",
            "MOVE MAP-1::IsBusy() TO WS-B1",
            "MOVE MAP-1::Busy TO WS-B2",
            "INVOKE MAP-1 \"Geocode\" USING \"Paris\"",
            "STRING \"busy \" WS-B1 \" prop \" WS-B2 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ASYNC::Caption",
        ],
    );
    on(
        &mut form,
        "BTN-CANCEL",
        "onClick",
        &["01 WS-B1 PIC X.", "01 WS-B2 PIC X.", "01 WS-L PIC X(60)."],
        &[
            "INVOKE MAP-1 \"TraceRoad\" USING \"test-key\" \"abc\" \"0\" \"1\" \"1\"",
            "MOVE MAP-1::IsBusy() TO WS-B1",
            "INVOKE MAP-1 \"Cancel\"",
            "MOVE MAP-1::IsBusy() TO WS-B2",
            "STRING \"before \" WS-B1 \" after \" WS-B2 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-CANCEL::Caption",
        ],
    );

    let cfrm = root.join("forms/parity-maps.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-maps.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("paritymaps.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritymaps\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-maps.cfrm\"]\ngenerated = [\"generated/parity-maps.cbl\"]\n",
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
    let names = vec!["s1-collections", "s2-cleared", "s3-marker"];
    let steps = vec![
        wait(300),
        event("BTN-COLL"),
        wait(300),
        dump(),
        shot(names[0]),
        event("BTN-CLEAR"),
        wait(300),
        dump(),
        shot(names[1]),
        event("BTN-NOKEY"),
        wait(300),
        dump(),
        event("BTN-ASYNC"),
        wait(600),
        dump(),
        event("BTN-CANCEL"),
        wait(600),
        dump(),
        json!({"click": {"control": "MAP-1"}}),
        wait(300),
        dump(),
        shot(names[2]),
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
    props.get(name.to_ascii_uppercase()).or_else(|| props.get(name)).and_then(Value::as_str).unwrap_or("").to_string()
}

/// What one engine did: its dumps and how long it ran.
struct Seen {
    dumps: Vec<Value>,
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

/// `cmd` with the environment of a run: the tiles from the test folder, and no Maps key at all, so
/// nothing can reach a real provider.
fn isolated<'a>(cmd: &'a mut Command, root: &Path) -> &'a mut Command {
    cmd.env("HOME", root.join("home"))
        .env("PRC_TEST_MAP_TILES", root.join("tiles"))
        .env_remove(cobolt_form_host::seeding::GOOGLE_MAPS_API_KEY_ENV)
        .env_remove(cobolt_form_host::seeding::GOOGLE_SEARCH_API_KEY_ENV)
}

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly path
/// and run it in the built program.
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
    let out = isolated(&mut Command::new(rcrun()), root)
        .args(["run-form", "forms/parity-maps.cfrm", "generated/parity-maps.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let interp = Seen { dumps: dumps(&report(&said)), ms: t.elapsed().as_secs_f64() * 1000.0 };

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
    let exe = root.join("bin").join(format!("paritymaps{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let out = isolated(&mut Command::new(&exe), root)
        .current_dir(root)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let wasm = Seen { dumps: dumps(&report(&said)), ms: t.elapsed().as_secs_f64() * 1000.0 };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_maps_control_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t48-maps-{}", std::process::id()));
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
    let log = |i: usize| at(i, "LST-LOG", "Items");
    // Not busy: `0` as the interpreter writes it, or `false` as a property the host types.
    let idle = |i: usize| matches!(at(i, "MAP-1", "Busy").as_str(), "" | "0" | "false");
    checks.extend([
        (
            "markers: one of five arguments, one of four, one added and removed".to_string(),
            at(0, "MAP-1", "Markers") == "M0\t0\t0\tOrigin\tthe centre\nM1\t10\t20\tOne\tfirst\nM2\t-10\t-20\tTwo\t",
        ),
        ("routes: R1 replaced by its id and moved last, R2 removed".into(), at(0, "MAP-1", "Routes") == "R1\t#12A150\t4\t0,0;20,40"),
        (
            "regions: five arguments and seven, Z1 removed".into(),
            at(0, "MAP-1", "Regions") == "Z2\t#12A15055\t#12A150\t1\t-5,-5;-15,-5;-15,-15\tSouth\ta region\nZ3\t#1E6EDC55\t#1E6EDC\t1\t20,-30;25,-30;25,-25\t\t",
        ),
        ("ClearRoutes and ClearRegions empty them; the markers stay".into(), at(1, "MAP-1", "Routes").is_empty() && at(1, "MAP-1", "Regions").is_empty() && at(1, "MAP-1", "Markers") == at(0, "MAP-1", "Markers")),
        (
            "no Google key and no OpenRouteService key: each reason at once, then two onError reading the last, nothing busy".into(),
            log(2).lines().collect::<Vec<_>>() == ["Maps API key not configured", "TraceRoad: no OpenRouteService key was supplied", "TraceRoad: no OpenRouteService key was supplied", "TraceRoad: no OpenRouteService key was supplied"] && idle(2),
        ),
        // `IsBusy()` answers 1; the `Busy` property reads as a truth value.
        ("a road route on its own thread: busy at once (IsBusy 1, Busy true)".into(), at(3, "LBL-ASYNC", "Caption").trim_end() == "busy 1 prop true |"),
        (
            "its ending raises onError with the thread's reason; the place asked meanwhile was ignored".into(),
            log(3).lines().skip(4).collect::<Vec<_>>() == ["TraceRoad: origin latitude 'abc' is not a number"] && idle(3),
        ),
        ("Cancel: onCancelled, not busy after, and the ending dropped".into(), at(4, "LBL-CANCEL", "Caption").trim_end() == "before 1 after 0|" && log(4).lines().skip(5).collect::<Vec<_>>() == ["cancelled"]),
        ("a click on the map's centre selects the designed marker".into(), at(5, "MAP-1", "SelectedMarkerId") == "M0" && log(5).lines().last() == Some("M0")),
        ("no onComplete ever (no request reached a provider)".into(), !log(5).lines().any(|l| l == "complete")),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_maps_control_agrees_on_both_engines (DESKTOP-WASM-001 T48: R10, R11) ==");
    println!("form {FORM}: a Maps control over 4 tiles from a test folder (no tile server), a ListBox log, 5 buttons, 9 event handlers");
    println!("  AddMarker (5 and 4 arguments), RemoveMarker, AddRoute (a replacement by id), RemoveRoute, AddRegion (5 and 7 arguments), RemoveRegion, ClearRoutes, ClearRegions;");
    println!("  Geocode and TraceRoad with no key; TraceRoad on its own thread with IsBusy, Busy and a Geocode ignored while it runs; Cancel; a pointer click on a marker");
    println!("real time; {n_dumps} dumps and {} pictures per engine", shots.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
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
        for i in 0..n_dumps {
            println!("    dump {i}: log {:?}, markers {:?}, routes {:?}, regions {:?}, busy {:?}, selected {:?}, async {:?}, cancel {:?}", log(i), at(i, "MAP-1", "Markers"), at(i, "MAP-1", "Routes"), at(i, "MAP-1", "Regions"), at(i, "MAP-1", "Busy"), at(i, "MAP-1", "SelectedMarkerId"), at(i, "LBL-ASYNC", "Caption"), at(i, "LBL-CANCEL", "Caption"));
        }
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
