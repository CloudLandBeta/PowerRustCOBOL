// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50 (R10, R11): **the Viewer behaves on the WebAssembly path as it does under the
//! interpreter.**
//!
//! One responsive form with three Viewers, a ListBox that logs their events in order, six status labels
//! and eleven buttons:
//! - `VWR-CHAT` (`Streamed`) holds a conversation. `BTN-CHAT` appends Markdown, HTML and raw messages,
//!   extends and replaces them, extends one that does not exist and jumps to the latest. `BTN-NEW` starts
//!   a new conversation, registers an older one and appends to the new one. `BTN-SELECT` selects the older
//!   one and appends to it;
//! - `VWR-DOC` loads a text file through `SetProperty` (`BTN-LOAD`), is zoomed by a `MOVE` to its
//!   member, and fails to load a file that is not there, which leaves the document on screen
//!   (`BTN-BAD`). `BTN-FIND` opens the find bar on a word, `BTN-STEP` steps through the matches
//!   (FindNext twice, FindPrevious once) and `BTN-CLOSE` closes the bar;
//! - `BTN-BYTES` gives `VWR-BYTES` a document as bytes and saves it to a path, saves the conversation as
//!   a PDF, and asks to print the conversation, which has no document to print;
//! - `BTN-SAVE` asks for a destination for the bytes (the dialog is answered with a path), and
//!   `BTN-CANCEL` asks for one for the text file (the dialog is answered with a cancel).
//!
//! Every message's text is sent as written, the spaces around it kept, through both an `INVOKE` and an
//! inline call.
//!
//! The script runs headless in real time under `rcrun run-form` (the interpreter) and in the program
//! `rcrun build` makes on the WebAssembly path. The dumps must be equal, the pictures equal in every
//! pixel, every dialog answer taken, and the files written the same. No document is ever handed to the
//! operating system: the one print asked for has nothing to print.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-VIEWER";

/// The bytes `BTN-BYTES` gives `VWR-BYTES`, which both saves must write back unchanged.
const BYTES: &str = "# Notes from bytes";

/// The text document `VWR-DOC` loads.
const NOTES: &str = "Field notes\n\nThe first line of the notes.\nA second line, and a third line.\nOne more line at the end.\n";

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

/// Log a line in the event list.
fn log(what: &str) -> String {
    format!("INVOKE LST-EV \"AddItem\" USING {what}")
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    fs::write(root.join("notes.txt"), NOTES).unwrap();
    let mut form = Form::new(FORM, "Viewer", 760, 530);
    form.main_form = true;
    form.responsive = true;
    control(&mut form, "VWR-CHAT", ControlType::Viewer, (20, 20, 440, 260), &[("Layout", "Streamed")]);
    control(&mut form, "VWR-DOC", ControlType::Viewer, (480, 20, 260, 200), &[]);
    control(&mut form, "VWR-BYTES", ControlType::Viewer, (480, 230, 260, 50), &[]);
    control(&mut form, "LST-EV", ControlType::ListBox, (20, 290, 440, 190), &[]);
    for (i, id) in ["LBL-ID", "LBL-HIST", "LBL-DOC", "LBL-ERR", "LBL-FIND", "LBL-PROG"].iter().enumerate() {
        control(&mut form, id, ControlType::Label, (480, 290 + 32 * i as i32, 260, 28), &[("Caption", "")]);
    }
    let buttons = ["BTN-CHAT", "BTN-NEW", "BTN-SELECT", "BTN-LOAD", "BTN-BAD", "BTN-FIND", "BTN-STEP", "BTN-CLOSE", "BTN-BYTES", "BTN-SAVE", "BTN-CANCEL"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20 + 66 * i as i32, 490, 62, 28), &[("Caption", id.trim_start_matches("BTN-"))]);
    }

    // The Viewers' events, in the order they come.
    on(&mut form, "VWR-CHAT", "onContentRendered", &[], &[&log("\"chat rendered\"")]);
    on(&mut form, "VWR-CHAT", "onConversationCreated", &[], &[&log("\"chat created\"")]);
    on(&mut form, "VWR-CHAT", "onConversationSelected", &[], &[&log("\"chat selected\"")]);
    on(&mut form, "VWR-CHAT", "onSaveComplete", &[], &[&log("\"chat saved\"")]);
    on(&mut form, "VWR-CHAT", "onError", &[], &[&log("VWR-CHAT::LastError")]);
    on(&mut form, "VWR-DOC", "onLoaded", &[], &[&log("\"doc loaded\"")]);
    on(&mut form, "VWR-DOC", "onLoadProgress", &[], &["MOVE VWR-DOC::Progress TO LBL-PROG::Caption"]);
    on(&mut form, "VWR-DOC", "onError", &[], &[&log("VWR-DOC::LastError")]);
    on(&mut form, "VWR-DOC", "onFindOpened", &[], &[&log("\"find opened\"")]);
    on(&mut form, "VWR-DOC", "onFindClosed", &[], &[&log("\"find closed\"")]);
    on(&mut form, "VWR-DOC", "onSaveCancelled", &[], &[&log("\"doc save cancelled\"")]);
    on(&mut form, "VWR-BYTES", "onLoaded", &[], &[&log("\"bytes loaded\"")]);
    on(&mut form, "VWR-BYTES", "onSaveComplete", &[], &[&log("\"bytes saved\"")]);
    on(&mut form, "VWR-BYTES", "onError", &[], &[&log("VWR-BYTES::LastError")]);

    let ws = ["01 WS-ID PIC X(8)."];
    on(
        &mut form,
        "BTN-CHAT",
        "onClick",
        &ws,
        &[
            "SET WS-ID TO VWR-CHAT::AppendMarkdown(\" **Hello** from COBOL \", \"user\")",
            "SET WS-ID TO VWR-CHAT::AppendHtml(\"<p>Reply <b>one</b></p>\", \"agent\")",
            "INVOKE VWR-CHAT \"AppendToMessage\" USING WS-ID \" and *more*\" \"markdown\"",
            "SET WS-ID TO VWR-CHAT::AppendRaw(\"raw <b>text</b>\")",
            "INVOKE VWR-CHAT \"ReplaceMessage\" USING WS-ID \"replaced, still raw <i>\" \"raw\"",
            "INVOKE VWR-CHAT \"AppendToMessage\" USING \"m99\" \"lost\" \"raw\"",
            "INVOKE VWR-CHAT \"JumpToLatest\"",
            "MOVE WS-ID TO LBL-ID::Caption",
        ],
    );
    on(
        &mut form,
        "BTN-NEW",
        "onClick",
        &ws,
        &[
            "INVOKE VWR-CHAT \"NewConversation\"",
            "INVOKE VWR-CHAT \"RegisterConversation\" USING \"c-old\" \"An older chat\"",
            "MOVE VWR-CHAT::HistoryList TO LBL-HIST::Caption",
            "SET WS-ID TO VWR-CHAT::AppendMarkdown(\"A *new* start\", \"user\")",
        ],
    );
    on(
        &mut form,
        "BTN-SELECT",
        "onClick",
        &ws,
        &[
            "INVOKE VWR-CHAT \"SelectConversation\" USING \"c-old\"",
            "SET WS-ID TO VWR-CHAT::AppendMarkdown(\"Restored from **history**\", \"agent\")",
            "MOVE VWR-CHAT::HistoryList TO LBL-HIST::Caption",
            "MOVE VWR-CHAT::ConversationId TO LBL-ID::Caption",
        ],
    );
    on(
        &mut form,
        "BTN-LOAD",
        "onClick",
        &[],
        &["INVOKE VWR-DOC \"SetProperty\" USING \"Source\" \"notes.txt\"", "MOVE \"150\" TO VWR-DOC::Zoom", "MOVE VWR-DOC::Format TO LBL-DOC::Caption"],
    );
    on(
        &mut form,
        "BTN-BAD",
        "onClick",
        &[],
        &["INVOKE VWR-DOC \"SetProperty\" USING \"Source\" \"missing.txt\"", "MOVE VWR-DOC::LastError TO LBL-ERR::Caption", "MOVE VWR-DOC::Source TO LBL-DOC::Caption"],
    );
    on(&mut form, "BTN-FIND", "onClick", &[], &["INVOKE VWR-DOC \"Find\" USING \"line\""]);
    on(
        &mut form,
        "BTN-STEP",
        "onClick",
        &[],
        &["INVOKE VWR-DOC \"FindNext\"", "INVOKE VWR-DOC \"FindNext\"", "INVOKE VWR-DOC \"FindPrevious\"", "MOVE VWR-DOC::SearchCurrentMatch TO LBL-FIND::Caption"],
    );
    on(&mut form, "BTN-CLOSE", "onClick", &[], &["INVOKE VWR-DOC \"FindClose\""]);
    on(
        &mut form,
        "BTN-BYTES",
        "onClick",
        &[],
        &[
            &format!("INVOKE VWR-BYTES \"LoadBytes\" USING \"{BYTES}\""),
            "INVOKE VWR-BYTES \"SaveAs\" USING \"out/bytes.md\"",
            "INVOKE VWR-CHAT \"SaveAsPdf\" USING \"out/chat.pdf\"",
            "INVOKE VWR-CHAT \"Print\"",
        ],
    );
    on(&mut form, "BTN-SAVE", "onClick", &[], &["INVOKE VWR-BYTES \"SaveAs\""]);
    on(&mut form, "BTN-CANCEL", "onClick", &[], &["INVOKE VWR-DOC \"SaveAs\""]);

    let cfrm = root.join("forms/parity-viewer.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-viewer.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("parityviewer.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityviewer\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-viewer.cfrm\"]\ngenerated = [\"generated/parity-viewer.cbl\"]\n",
    )
    .unwrap();
    manifest
}

/// The script, and the names of its pictures in order. `asked` is where the answered Save As writes.
fn script(asked: &Path) -> (Value, Vec<&'static str>) {
    let event = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let wait = |ms: u64| json!({"wait_ms": ms});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let names = vec!["v1-chat", "v2-new", "v3-selected", "v4-loaded", "v5-bad", "v6-find", "v7-step", "v8-end"];
    let steps = vec![
        wait(400),
        event("BTN-CHAT"),
        wait(800),
        dump(),
        shot(names[0]),
        event("BTN-NEW"),
        wait(600),
        dump(),
        shot(names[1]),
        event("BTN-SELECT"),
        wait(600),
        dump(),
        shot(names[2]),
        event("BTN-LOAD"),
        wait(1500),
        dump(),
        shot(names[3]),
        event("BTN-BAD"),
        wait(1000),
        dump(),
        shot(names[4]),
        event("BTN-FIND"),
        wait(1000),
        dump(),
        shot(names[5]),
        event("BTN-STEP"),
        wait(800),
        dump(),
        shot(names[6]),
        event("BTN-CLOSE"),
        wait(500),
        dump(),
        event("BTN-BYTES"),
        wait(1000),
        dump(),
        json!({"answer": {"path": asked.to_string_lossy()}}),
        event("BTN-SAVE"),
        wait(1000),
        dump(),
        json!({"answer": {"cancel": true}}),
        event("BTN-CANCEL"),
        wait(1000),
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

/// The properties whose values differ between two dumps, as `control.property: …`.
fn dump_differences(a: &Value, b: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else { return vec!["(not a dump)".into()] };
    let mut ids: Vec<&String> = a.keys().chain(b.keys()).collect();
    ids.sort();
    ids.dedup();
    let shown = |v: Option<&Value>| v.map(|v| v.to_string().chars().take(160).collect::<String>()).unwrap_or_else(|| "-".into());
    for id in ids {
        let (x, y) = (a.get(id), b.get(id));
        if x == y {
            continue;
        }
        let (px, py) = (x.and_then(|v| v["props"].as_object()), y.and_then(|v| v["props"].as_object()));
        match (px, py) {
            (Some(px), Some(py)) => {
                let mut keys: Vec<&String> = px.keys().chain(py.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    if px.get(k) != py.get(k) {
                        out.push(format!("{id}.{k}: interpreter {} / runner {}", shown(px.get(k)), shown(py.get(k))));
                    }
                }
                let strip = |v: &Value| {
                    let mut v = v.clone();
                    if let Some(o) = v.as_object_mut() {
                        o.remove("props");
                    }
                    v
                };
                if x.map(strip) != y.map(strip) {
                    out.push(format!("{id} (besides its properties): interpreter {} / runner {}", shown(x.map(strip).as_ref()), shown(y.map(strip).as_ref())));
                }
            }
            _ => out.push(format!("{id}: interpreter {} / runner {}", shown(x), shown(y))),
        }
    }
    out
}

/// A control's property in a dump (upper case or as written).
fn prop(dump: &Value, id: &str, name: &str) -> String {
    let props = &dump[id]["props"];
    props.get(name.to_ascii_uppercase()).or_else(|| props.get(name)).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

/// What one engine did: its report, everything it said, how long it ran, and the files it wrote.
struct Seen {
    report: Value,
    dumps: Vec<Value>,
    said: String,
    ms: f64,
    out: PathBuf,
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

/// The files a run wrote in `out/`, kept apart under `out-<who>/` so the other engine starts from an
/// empty `out/`.
fn keep_out(root: &Path, who: &str) -> PathBuf {
    let kept = root.join(format!("out-{who}"));
    let _ = fs::remove_dir_all(&kept);
    fs::rename(root.join("out"), &kept).expect("keep the files a run wrote");
    kept
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
    fs::create_dir_all(root.join("out")).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-viewer.cfrm", "generated/parity-viewer.cbl", "--designer", "--headless"])
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
    let interp = Seen { dumps: dumps(&r), report: r, said, ms: t.elapsed().as_secs_f64() * 1000.0, out: keep_out(root, "interpreter") };

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
    let exe = root.join("bin").join(format!("parityviewer{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    fs::create_dir_all(root.join("out")).unwrap();
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
    let wasm = Seen { dumps: dumps(&r), report: r, said, ms: t.elapsed().as_secs_f64() * 1000.0, out: keep_out(root, "runner") };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_viewer_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t50-viewer-{}", std::process::id()));
    let manifest = project(&root);
    let (steps, shots) = script(&root.join("out").join("asked.md"));
    let n_dumps = steps.as_array().map(|s| s.iter().filter(|st| st.get("dump").is_some()).count()).unwrap_or(0);
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script);

    // ── The two engines against each other: dumps, pictures, dialogs and files ──
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
    // The Viewer's own Save As is not a program's dialog, which the report lists: both answers being
    // taken shows both dialogs opened, on both engines.
    let unused = |s: &Seen| s.report.get("answers_unused").cloned().unwrap_or(json!(0));
    checks.push(("both Save As dialogs opened and took their answers, on both engines".into(), unused(&twin.interp) == json!(0) && unused(&twin.wasm) == json!(0)));
    let read = |s: &Seen, f: &str| fs::read(s.out.join(f)).ok();
    for f in ["bytes.md", "asked.md"] {
        checks.push((
            format!("out/{f}: the bytes given, written unchanged by both engines"),
            read(&twin.interp, f).as_deref() == Some(BYTES.as_bytes()) && read(&twin.wasm, f).as_deref() == Some(BYTES.as_bytes()),
        ));
    }
    let pdf = |s: &Seen| read(s, "chat.pdf").is_some_and(|b| b.starts_with(b"%PDF"));
    checks.push(("out/chat.pdf: the conversation as a PDF, from both engines".into(), pdf(&twin.interp) && pdf(&twin.wasm)));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str, p: &str| twin.wasm.dumps.get(i).map(|d| prop(d, id, p)).unwrap_or_default();
    let events = |i: usize| at(i, "LST-EV", "Items");
    let lines = |i: usize| events(i).lines().map(str::to_owned).collect::<Vec<_>>();
    let rendered = lines(0).iter().filter(|l| *l == "chat rendered").count();
    let no_m99 = "no conversation message with id \"m99\"";
    checks.extend([
        (
            "BTN-CHAT: five messages rendered, the extension of one that does not exist refused, the last id answered".to_string(),
            rendered == 5 && lines(0).last().map(String::as_str) == Some(no_m99) && at(0, "LBL-ID", "Caption") == "m3",
        ),
        (
            "BTN-NEW: the conversation archived, a new one created and rendered, the older one registered".into(),
            lines(1).ends_with(&["chat created".to_string(), "chat rendered".to_string()]) && at(1, "LBL-HIST", "Caption").contains("c-old"),
        ),
        (
            "BTN-SELECT: the older conversation selected and taken out of the history".into(),
            lines(2).ends_with(&["chat selected".to_string(), "chat rendered".to_string()]) && at(2, "LBL-ID", "Caption") == "c-old" && !at(2, "LBL-HIST", "Caption").contains("c-old"),
        ),
        (
            "BTN-LOAD: notes.txt loaded through SetProperty, its format read at once, its progress at 100".into(),
            lines(3).last().map(String::as_str) == Some("doc loaded") && !at(3, "LBL-DOC", "Caption").is_empty() && at(3, "LBL-PROG", "Caption") == "100",
        ),
        (
            "BTN-BAD: missing.txt refused with a message; the source reads back notes.txt".into(),
            !at(4, "LBL-ERR", "Caption").is_empty() && at(4, "LBL-DOC", "Caption") == "notes.txt" && lines(4).last().map(String::as_str) == Some(at(4, "LBL-ERR", "Caption").as_str()),
        ),
        ("BTN-FIND and BTN-CLOSE: the find bar opened and closed".into(), lines(5).last().map(String::as_str) == Some("find opened") && lines(7).last().map(String::as_str) == Some("find closed")),
        (
            "BTN-BYTES: the bytes loaded and saved, the PDF saved, the print refused for want of a document".into(),
            lines(8).ends_with(&["bytes loaded".to_string(), "bytes saved".to_string(), "chat saved".to_string(), "no document loaded".to_string()]),
        ),
        ("BTN-SAVE: the answered Save As saved the bytes".into(), lines(9).last().map(String::as_str) == Some("bytes saved")),
        ("BTN-CANCEL: the cancelled Save As raised onSaveCancelled".into(), lines(10).last().map(String::as_str) == Some("doc save cancelled")),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_viewer_agrees_on_both_engines (DESKTOP-WASM-001 T50: R10, R11) ==");
    println!("form {FORM}: 3 Viewers (one Streamed), an event list, 6 status labels, 11 buttons, 25 event handlers");
    println!("  AppendMarkdown (padded, inline), AppendHtml, AppendRaw, AppendToMessage (padded; also of an unknown id), ReplaceMessage, JumpToLatest;");
    println!("  NewConversation, RegisterConversation, SelectConversation, HistoryList, ConversationId;");
    println!("  SetProperty Source (a text file, and a file that is not there), MOVE to Zoom, Format, Progress, LastError;");
    println!("  Find, FindNext, FindPrevious, FindClose; LoadBytes, SaveAs to a path and through the dialog (answered, cancelled),");
    println!("  SaveAsPdf of the conversation, Print with no document");
    println!("real time; {n_dumps} dumps and {} pictures per engine", shots.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    println!("dialog answers left unused: interpreter {}, runner {}", unused(&twin.interp), unused(&twin.wasm));
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
            for i in 0..n_dumps {
                let d = s.dumps.get(i);
                let p = |id: &str, k: &str| d.map(|d| prop(d, id, k)).unwrap_or_default();
                println!("    {who} dump {i}: events {:?}; id {:?} hist {:?} doc {:?} err {:?} find {:?} prog {:?}", p("LST-EV", "Items"), p("LBL-ID", "Caption"), p("LBL-HIST", "Caption"), p("LBL-DOC", "Caption"), p("LBL-ERR", "Caption"), p("LBL-FIND", "Caption"), p("LBL-PROG", "Caption"));
            }
        }
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
