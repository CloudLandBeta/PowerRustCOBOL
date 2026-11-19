// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47a (R10, R11, ABI A48): **the WebSearch control behaves on the WebAssembly
//! path as it does under the interpreter.**
//!
//! A stub SearXNG instance on this machine answers `…/search` with two results. The first result
//! echoes what the stub received: the query, `format`, `safesearch` and the `Accept` header. A
//! path under `/slow` answers after 2.5 seconds. One form has five WebSearch controls, all with
//! `Verbose` on:
//! - `WEB-1`: `Mode = Sync`, an `Endpoint` with a trailing slash, `SafeSearch = High`.
//! - `WEB-2`: asynchronous, `SafeSearch = Medium`.
//! - `WEB-3`: Google with no key anywhere, so it is not configured.
//! - `WEB-4`: asynchronous, at an address where nothing listens.
//! - `WEB-5`: asynchronous, on the slow path, `TimeoutMs = 300`.
//!
//! No test run reaches a real provider. The project's search key variable is removed from both
//! engines' environment, and every address is this machine's.
//!
//! - **The Sync script**, run under a pinned clock and seed: `Search` answers the body at once,
//!   then `ResultCount`, `TopTitle`, `TopSnippet`, `TopLink`, `GetResult(2)`, `GetResult(9)` and
//!   `StatusCode` are read. Then the Google control: `onError` with `LastError` saying it is not
//!   configured, and nothing sent.
//! - **The Async script**, run in real time:
//!   - `Search` with a `Query` set at run time answers nothing at once, and the control is busy;
//!     a second search while it runs is ignored.
//!   - `onResultsReceived`, then `onComplete`, each reading the results.
//!   - `TimeoutMs = 300` raises `onTimeout`, and the late answer is dropped.
//!   - `Cancel()` raises `onCancelled`, and the control is no longer busy.
//!   - A transport failure raises `onError`, with `StatusCode` 0.
//!
//! Each script runs headless under `rcrun run-form` (the interpreter) and in the program
//! `rcrun build` makes on the WebAssembly path. The dumps must be equal, the pictures equal in
//! every pixel, and the lines `Verbose` narrates (`[search …]`) equal, line for line.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-SEARCH";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

/// The order of `WEB-2`'s and `WEB-5`'s endings, seen by every handler.
const FORM_WS: &str = "       01 WS-SEQ                  GLOBAL PIC 9(2) VALUE 0.";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// One request the stub answers: two results, the first echoing what it received.
fn answer(mut s: TcpStream) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let target = head.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("").to_owned();
    let accept = head
        .lines()
        .skip(1)
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("Accept")).map(|(_, v)| v.trim().to_owned()))
        .unwrap_or_else(|| "-".into());
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let param = |name: &str| query.split('&').find_map(|p| p.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_owned())).unwrap_or_else(|| "-".into());
    if path.starts_with("/slow") {
        std::thread::sleep(Duration::from_millis(2500));
    }
    let text = if path.ends_with("/search") {
        json!({"results": [
            {"title": format!("First {}", param("q")), "content": format!("fmt={} safe={} accept={accept}", param("format"), param("safesearch")), "url": "https://example.test/1"},
            {"title": "Second", "content": "two", "url": "https://example.test/2"}
        ]})
        .to_string()
    } else {
        format!("not a search: {path}")
    };
    let status = if path.ends_with("/search") { "200 OK" } else { "404 Not Found" };
    let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
    let _ = s.write_all(reply.as_bytes());
    let _ = s.flush();
}

/// The stub's port; it answers every connection on a thread of its own until the test ends.
fn stub() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            std::thread::spawn(move || answer(s));
        }
    });
    port
}

/// A port where nothing listens.
fn closed_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// A handler with its own WORKING-STORAGE items (`ws`, possibly empty).
fn handler(ws: &str, statements: &str) -> String {
    let ws = if ws.is_empty() { String::new() } else { format!("       WORKING-STORAGE SECTION.\n{ws}\n") };
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n{ws}       PROCEDURE DIVISION.\n{statements}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, PropValue)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    form.controls.push(c);
}

/// Bind `event` of control `id` to a handler.
fn bind(form: &mut Form, id: &str, event: &str, ws: &str, statements: &str) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(ws, statements) });
}

/// The statements of a handler, one per line.
fn lines(statements: &[&str]) -> String {
    statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n")
}

/// The fixture project in `root`, its WebSearch controls pointed at the stub on `port` and at
/// `closed`, where nothing listens; its manifest's path.
fn project(root: &Path, port: u16, closed: u16) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let on = PropValue::Bool(true);
    let mut form = Form::new(FORM, "WebSearch form", 760, 560);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-FIND", "BTN-NC", "BTN-AFIND", "BTN-ASLOW", "BTN-ACANCEL", "BTN-AERR"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    let labels = [
        "LBL-COUNT", "LBL-TITLE", "LBL-SNIP", "LBL-LINK", "LBL-R2", "LBL-R9", "LBL-ST", "LBL-NC", "LBL-ASYNC", "LBL-RES", "LBL-DONE", "LBL-ATIME", "LBL-ACANCEL",
        "LBL-ADONE5", "LBL-AERR",
    ];
    for (i, id) in labels.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 30 * i as i32, 580, 26), &[("Caption", s("-"))]);
    }
    let stub_at = |path: &str| format!("http://127.0.0.1:{port}{path}");
    let searxng = |endpoint: String| vec![("Provider", s("SearXNG")), ("Endpoint", PropValue::String(endpoint)), ("Verbose", on.clone())];
    let mut web1 = searxng(stub_at("/"));
    web1.extend([("Mode", s("Sync")), ("Query", s("cobol forms")), ("SafeSearch", s("High")), ("NumResults", PropValue::Int(3))]);
    control(&mut form, "WEB-1", ControlType::WebSearch, (680, 500, 56, 56), &web1);
    let mut web2 = searxng(stub_at(""));
    web2.extend([("SafeSearch", s("Medium"))]);
    control(&mut form, "WEB-2", ControlType::WebSearch, (620, 500, 56, 56), &web2);
    control(&mut form, "WEB-3", ControlType::WebSearch, (560, 500, 56, 56), &[("Provider", s("Google")), ("Query", s("never sent")), ("Verbose", on.clone())]);
    control(&mut form, "WEB-4", ControlType::WebSearch, (500, 500, 56, 56), &searxng(format!("http://127.0.0.1:{closed}")));
    let mut web5 = searxng(stub_at("/slow"));
    web5.extend([("TimeoutMs", PropValue::Int(300))]);
    control(&mut form, "WEB-5", ControlType::WebSearch, (440, 500, 56, 56), &web5);

    // ── Sync: WEB-1 and the Google control that is not configured ──
    bind(
        &mut form,
        "BTN-FIND",
        "onClick",
        "       01 WS-BODY                 PIC X(20).",
        &lines(&[
            "MOVE WEB-1::Search() TO WS-BODY",
            "MOVE WEB-1::ResultCount() TO LBL-COUNT::Caption",
            "MOVE WEB-1::TopTitle() TO LBL-TITLE::Caption",
            "MOVE WEB-1::TopSnippet() TO LBL-SNIP::Caption",
            "MOVE WEB-1::TopLink() TO LBL-LINK::Caption",
            "MOVE WEB-1::GetResult(2) TO LBL-R2::Caption",
            "MOVE WEB-1::GetResult(9) TO LBL-R9::Caption",
            "MOVE WEB-1::StatusCode TO LBL-ST::Caption",
            "MOVE WS-BODY TO LBL-ASYNC::Caption",
        ]),
    );
    bind(&mut form, "BTN-NC", "onClick", "", &lines(&["WEB-3::Search()"]));
    bind(&mut form, "WEB-3", "onError", "", &lines(&["MOVE WEB-3::LastError TO LBL-NC::Caption"]));

    // ── Async: WEB-2, WEB-4 and WEB-5 ──
    bind(
        &mut form,
        "BTN-AFIND",
        "onClick",
        "       01 WS-R                    PIC X(10).\n       01 WS-R2                   PIC X(10).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE \"rust cobol\" TO WEB-2::Query",
            "MOVE WEB-2::Search() TO WS-R",
            "MOVE WEB-2::IsBusy() TO WS-B",
            "MOVE WEB-2::Search() TO WS-R2",
            "STRING \"answer [\" WS-R \"] busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(
        &mut form,
        "WEB-2",
        "onResultsReceived",
        "       01 WS-N                    PIC 9(2).\n       01 WS-T                    PIC X(60).",
        &lines(&[
            "ADD 1 TO WS-SEQ",
            "MOVE WEB-2::ResultCount() TO WS-N",
            "STRING \"results \" WS-SEQ \" count \" WS-N DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-RES::Caption",
        ]),
    );
    bind(
        &mut form,
        "WEB-2",
        "onComplete",
        "       01 WS-C                    PIC 9(3).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(60).",
        &lines(&[
            "ADD 1 TO WS-SEQ",
            "MOVE WEB-2::StatusCode TO WS-C",
            "MOVE WEB-2::IsBusy() TO WS-B",
            "STRING \"complete \" WS-SEQ \" status \" WS-C \" busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-DONE::Caption",
            "MOVE WEB-2::TopTitle() TO LBL-TITLE::Caption",
        ]),
    );
    bind(&mut form, "BTN-ASLOW", "onClick", "", &lines(&["WEB-5::Search()"]));
    bind(&mut form, "WEB-5", "onTimeout", "", &lines(&["MOVE WEB-5::LastError TO LBL-ATIME::Caption"]));
    bind(&mut form, "WEB-5", "onCancelled", "", &lines(&["MOVE \"cancelled\" TO LBL-ACANCEL::Caption"]));
    bind(&mut form, "WEB-5", "onComplete", "", &lines(&["ADD 1 TO WS-SEQ", "MOVE \"a late answer\" TO LBL-ADONE5::Caption"]));
    bind(
        &mut form,
        "BTN-ACANCEL",
        "onClick",
        "       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(20).",
        &lines(&[
            "MOVE 0 TO WEB-5::TimeoutMs",
            "WEB-5::Search()",
            "WEB-5::Cancel()",
            "MOVE WEB-5::IsBusy() TO WS-B",
            "STRING \"cancel busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(&mut form, "BTN-AERR", "onClick", "", &lines(&["WEB-4::Search()"]));
    bind(
        &mut form,
        "WEB-4",
        "onError",
        "       01 WS-S                    PIC 9(3).\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE WEB-4::StatusCode TO WS-S",
            "IF WEB-4::LastError = SPACES",
            "    STRING \"error status \" WS-S \" no text\" DELIMITED BY SIZE INTO WS-T",
            "ELSE",
            "    STRING \"error status \" WS-S \" with text\" DELIMITED BY SIZE INTO WS-T",
            "END-IF",
            "MOVE WS-T TO LBL-AERR::Caption",
        ]),
    );

    let cfrm = root.join("forms/parity-search.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-search.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("paritysearch.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritysearch\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-search.cfrm\"]\ngenerated = [\"generated/parity-search.cbl\"]\n",
    )
    .unwrap();
    manifest
}

/// The two scripts, each with the names of its pictures in order, and whether it runs under the pins.
fn scripts() -> Vec<(&'static str, Value, Vec<&'static str>, bool)> {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let wait = |ms: u64| json!({"wait_ms": ms});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let sync = vec!["s1-results", "s2-not-configured"];
    let sync_steps = vec![wait(200), click("BTN-FIND"), dump(), shot(sync[0]), click("BTN-NC"), wait(300), dump(), shot(sync[1])];
    let asynchronous = vec!["a1-results", "a2-timed-out", "a3-cancelled", "a4-error"];
    let async_steps = vec![
        wait(200),
        click("BTN-AFIND"),
        wait(1500),
        dump(),
        shot(asynchronous[0]),
        click("BTN-ASLOW"),
        wait(4000),
        dump(),
        shot(asynchronous[1]),
        click("BTN-ACANCEL"),
        wait(4000),
        dump(),
        shot(asynchronous[2]),
        click("BTN-AERR"),
        wait(1500),
        dump(),
        shot(asynchronous[3]),
    ];
    vec![("sync", Value::Array(sync_steps), sync, true), ("async", Value::Array(async_steps), asynchronous, false)]
}

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

/// The lines a WebSearch's `Verbose` narrated, in order.
fn narration(stdout: &str) -> Vec<String> {
    stdout.lines().filter(|l| l.starts_with("[search ")).map(str::to_owned).collect()
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

/// The keys whose values differ between two dumps, as `control.key`.
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

/// A Label's caption in a dump.
fn caption(dump: &Value, id: &str) -> String {
    let props = &dump[id]["props"];
    props.get("CAPTION").or_else(|| props.get("Caption")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// What one engine did with every script: the dumps, and the narrated lines, in order.
#[derive(Default)]
struct Seen {
    dumps: Vec<Value>,
    narration: Vec<String>,
}

/// Run every script with `run` (the script's path, whether it is pinned).
fn run_all(scripts: &[(PathBuf, bool)], mut run: impl FnMut(&Path, bool) -> String) -> Seen {
    let mut seen = Seen::default();
    for (path, pinned) in scripts {
        let said = run(path, *pinned);
        seen.dumps.extend(dumps(&report(&said)));
        seen.narration.extend(narration(&said));
    }
    seen
}

/// What both engines did with the same scripts.
struct Twin {
    interp: Seen,
    wasm: Seen,
    interp_shots: PathBuf,
    runner_shots: PathBuf,
    runner_ms: f64,
    build_ms: f64,
    interp_ms: f64,
    wasm_ms: f64,
}

/// Run `paths` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly
/// path and run them in the built program. Neither engine sees the project's search key.
fn run_twin(root: &Path, manifest: &Path, paths: &[(PathBuf, bool)]) -> Twin {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let no_key = cobolt_form_host::seeding::GOOGLE_SEARCH_API_KEY_ENV;
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

    // ── 1. The interpreter: rcrun run-form ──
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let interp = run_all(paths, |script, pinned| {
        let mut c = Command::new(rcrun());
        c.args(["run-form", "forms/parity-search.cfrm", "generated/parity-search.cbl", "--designer", "--headless"])
            .arg(script)
            .args(["--headless-limit", "60"])
            .current_dir(root)
            .env("HOME", &home)
            .env("PRC_HEADLESS_SHOTS", &interp_shots)
            .env_remove(no_key);
        if pinned {
            c.env("PRC_TEST_CLOCK", PIN_CLOCK).env("PRC_TEST_SEED", PIN_SEED);
        }
        let out = c.output().expect("run rcrun run-form");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        said
    });
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;

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
    let exe = root.join("bin").join(format!("paritysearch{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let wasm = run_all(paths, |script, pinned| {
        let mut c = Command::new(&exe);
        c.current_dir(root)
            .env("HOME", &home)
            .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
            .env("PRC_HEADLESS_SCRIPT", script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .env("PRC_HEADLESS_SHOTS", &runner_shots)
            .env_remove(no_key);
        if pinned {
            c.env("PRC_TEST_CLOCK", PIN_CLOCK).env("PRC_TEST_SEED", PIN_SEED);
        }
        let out = c.output().expect("run the built program");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        said
    });
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms, interp_ms, wasm_ms }
}

/// Write each script beside the project: its path, and whether it runs under the pins.
fn write_scripts(root: &Path, scripts: &[(&'static str, Value, Vec<&'static str>, bool)]) -> Vec<(PathBuf, bool)> {
    scripts
        .iter()
        .map(|(name, steps, _, pinned)| {
            let p = root.join(format!("{name}.json"));
            fs::write(&p, steps.to_string()).unwrap();
            (p, *pinned)
        })
        .collect()
}

#[test]
fn the_websearch_control_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t47a-form-{}", std::process::id()));
    let (port, closed) = (stub(), closed_port());
    let manifest = project(&root, port, closed);
    let scripts = scripts();
    let paths = write_scripts(&root, &scripts);
    let twin = run_twin(&root, &manifest, &paths);
    let shots: Vec<&str> = scripts.iter().flat_map(|(_, _, s, _)| s.iter().copied()).collect();

    // ── The two engines against each other: dumps, pictures, narration ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), twin.interp.dumps.len() == shots.len() && twin.wasm.dumps.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.dumps.iter().zip(twin.wasm.dumps.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
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
    let (said_i, said_w) = (&twin.interp.narration, &twin.wasm.narration);
    let narration_lines: Vec<String> = said_i
        .iter()
        .zip(said_w.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("    line {i}: interpreter {a:?} / runner {b:?}"))
        .collect();
    checks.push((format!("Verbose narrated the same {} lines on both engines", said_i.len()), said_i == said_w && !said_i.is_empty()));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let said = |line: &str| said_w.iter().any(|l| l == line);
    let url = format!("http://127.0.0.1:{port}/search?q=cobol%20forms&format=json&safesearch=2");
    checks.extend([
        ("a Sync Search reads the results at once: ResultCount 2".to_string(), at(0, "LBL-COUNT").trim() == "2"),
        ("TopTitle: the query was sent, encoded".into(), at(0, "LBL-TITLE").trim() == "First cobol%20forms"),
        ("TopSnippet: format=json, SafeSearch High is 2, Accept JSON".into(), at(0, "LBL-SNIP").trim() == "fmt=json safe=2 accept=application/json"),
        ("TopLink".into(), at(0, "LBL-LINK").trim() == "https://example.test/1"),
        ("GetResult(2): title, snippet and link, tab-joined".into(), at(0, "LBL-R2").trim() == "Second\ttwo\thttps://example.test/2"),
        ("GetResult(9): nothing past the last result".into(), at(0, "LBL-R9").trim().is_empty()),
        ("StatusCode 200; Search answered the body".into(), at(0, "LBL-ST").trim() == "200" && at(0, "LBL-ASYNC").starts_with("{\"results\":[{\"")),
        ("the Endpoint's trailing slash is trimmed; the URL narrated".into(), said(&format!("[search WEB-1] url: {url}"))),
        ("Verbose narrates the provider, the headers and the mode".into(), said("[search WEB-1] provider: SearXNG") && said("[search WEB-1] headers: Accept: application/json") && said("[search WEB-1] mode: sync")),
        ("Google with no key: onError, LastError says it is not configured".into(), at(1, "LBL-NC").trim() == "Google search API key not configured"),
        ("… narrated, and nothing sent".into(), said("[search WEB-3] not configured: Google search API key not configured") && !said_w.iter().any(|l| l.starts_with("[search WEB-3] url"))),
        ("an asynchronous Search answers nothing at once and the control is busy".into(), at(2, "LBL-ASYNC").trim() == "answer [          ] busy 1"),
        ("onResultsReceived first, reading two results (the second Search was ignored)".into(), at(2, "LBL-RES").trim() == "results 01 count 02"),
        ("onComplete second, status 200, no longer busy".into(), at(2, "LBL-DONE").trim() == "complete 02 status 200 busy 0"),
        ("the Query set at run time was sent; SafeSearch Medium is 1".into(), at(2, "LBL-TITLE").trim() == "First rust%20cobol" && said(&format!("[search WEB-2] url: http://127.0.0.1:{port}/search?q=rust%20cobol&format=json&safesearch=1"))),
        ("TimeoutMs 300: onTimeout, LastError says so".into(), at(3, "LBL-ATIME").trim() == "No answer within 1 second: the call was cancelled."),
        ("the timed-out search's late answer is dropped".into(), at(3, "LBL-ADONE5").trim() == "-" && !said("[search WEB-5] status: 200")),
        ("Cancel(): onCancelled, no longer busy, its late answer dropped".into(), at(4, "LBL-ACANCEL").trim() == "cancelled" && at(4, "LBL-ASYNC").trim() == "cancel busy 0" && at(4, "LBL-ADONE5").trim() == "-"),
        ("a transport failure: onError, StatusCode 0, LastError says why".into(), at(5, "LBL-AERR").trim() == "error status 000 with text"),
        ("… narrated as a transport error".into(), said_w.iter().any(|l| l.starts_with("[search WEB-4] transport error: "))),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_websearch_control_agrees_on_both_engines (DESKTOP-WASM-001 T47a: R10, R11, ABI A48) ==");
    println!("form {FORM}: 5 WebSearch controls, all Verbose (SearXNG Sync, SearXNG asynchronous, Google not configured, SearXNG where nothing listens, SearXNG slow with TimeoutMs 300), 6 buttons, 10 event handlers");
    println!("  Sync: Search, ResultCount, TopTitle, TopSnippet, TopLink, GetResult(2), GetResult(9), StatusCode; a Google control with no key (onError)");
    println!("  Async: Search answers nothing and busy, a second ignored, onResultsReceived then onComplete, TimeoutMs 300 and onTimeout,");
    println!("        Cancel() and onCancelled, onError with StatusCode 0");
    println!(
        "a stub SearXNG on 127.0.0.1:{port}; the project's search key removed from both engines; sync script pinned (PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}); async script in real time; {} dumps, {} pictures, {} narrated lines",
        shots.len(),
        shots.len(),
        said_i.len()
    );
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter runs {:.0} ms; runner runs {:.0} ms", twin.interp_ms, twin.wasm_ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    for l in dump_lines.iter().chain(narration_lines.iter()) {
        println!("{l}");
    }
    if said_i.len() != said_w.len() {
        println!("    narrated lines: interpreter {}, runner {}", said_i.len(), said_w.len());
        for l in said_i.iter().filter(|l| !said_w.contains(l)) {
            println!("    interpreter only: {l}");
        }
        for l in said_w.iter().filter(|l| !said_i.contains(l)) {
            println!("    runner only: {l}");
        }
    }
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
