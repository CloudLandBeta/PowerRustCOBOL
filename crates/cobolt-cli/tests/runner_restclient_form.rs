// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T42b (R10, R11, ABI A46): **the RestClient control behaves on the WebAssembly
//! path as it does under the interpreter.**
//!
//! A stub web server on this machine answers every request with what it received: the verb, the
//! path, the `Authorization`, `X-API-Key`, `X-One` and `X-Shared` headers and the body; `/v1/slow`
//! answers after 2.5 seconds and `/v1/status/404` answers 404. One form with four RestClients:
//! `REST-1` (`Mode = Sync`, `BaseURL` with a trailing slash, `AuthType = Bearer`, `DefaultHeaders`
//! with a comment and a line that is not a header), `REST-2` (asynchronous, `DefaultMethod = post`,
//! `AuthType = APIKey`), and `REST-3` (Sync) and `REST-4` (asynchronous) on an address where nothing
//! listens. The credentials come from the environment (`COBOLT_CONTROL_KEY_<FORM>__<CONTROL>`), as a
//! form file never holds one.
//!
//! - **The Sync script**, run under a pinned clock and seed: `Get` of a relative path, `Post` with a
//!   body, `Call` with a lower-case verb and a `?query`, `Delete` with no argument (the `BaseURL`),
//!   `SetHeader` shared with `COBOL::"HTTP-GET"` and then `ClearHeaders`, a 404, a transport failure
//!   (status 0, the method answering why), and `SetTimeout`.
//! - **The Async script**, run in real time: `Get` answers nothing at once and the control is busy, a
//!   second request while it runs is ignored, `onComplete` reads `ResponseBody`, `StatusCode` and
//!   `IsBusy`; `Call` with no verb takes `DefaultMethod`; `TimeoutMs = 300` raises `onTimeout` with
//!   `LastError` and the late answer is dropped; `Cancel()` lets one go (`onCancelled`, no longer
//!   busy); a transport failure raises `onError` with `StatusCode` 0.
//!
//! Each script runs headless under `rcrun run-form` (the interpreter) and in the program
//! `rcrun build` makes on the WebAssembly path. The dumps must be equal and the pictures equal in
//! every pixel.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-REST";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

/// The tally of `REST-2`'s endings, seen by every handler.
const FORM_WS: &str = "       01 WS-ADONE                GLOBAL PIC 9(2) VALUE 0.";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// One request the stub answers: what it received, or 404, or slowly.
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
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let (verb, path) = (first.next().unwrap_or("").to_owned(), first.next().unwrap_or("").to_owned());
    let header = |name: &str| {
        head.lines()
            .skip(1)
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_owned()))
    };
    let length: usize = header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    while buf.len() < head_end + length {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let body = String::from_utf8_lossy(&buf[head_end..(head_end + length).min(buf.len())]).into_owned();
    let shown = |name: &str| header(name).unwrap_or_else(|| "-".into());
    let (status, text) = if path == "/v1/status/404" {
        ("404 Not Found", "missing".to_owned())
    } else {
        if path == "/v1/slow" {
            std::thread::sleep(Duration::from_millis(2500));
        }
        ("200 OK", format!("{verb} {path} a={} k={} one={} s={} b={body}", shown("Authorization"), shown("X-API-Key"), shown("X-One"), shown("X-Shared")))
    };
    let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
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

/// The fixture project in `root`, its RestClients pointed at the stub on `port` and at `closed`, where
/// nothing listens; its manifest's path.
fn project(root: &Path, port: u16, closed: u16) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let base = format!("http://127.0.0.1:{port}/v1/");
    let nowhere = format!("http://127.0.0.1:{closed}/");
    let mut form = Form::new(FORM, "RestClient form", 760, 560);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-GET", "BTN-POST", "BTN-CALL", "BTN-HDR", "BTN-404", "BTN-ERR", "BTN-TMO", "BTN-AGET", "BTN-ACALL", "BTN-ASLOW", "BTN-ACANCEL", "BTN-AERR"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    let labels = [
        "LBL-GET", "LBL-ST", "LBL-POST", "LBL-CALL", "LBL-DEL", "LBL-HDR", "LBL-AFTER", "LBL-404", "LBL-ERR", "LBL-TMO", "LBL-ASYNC", "LBL-ADONE", "LBL-ABODY",
        "LBL-ATIME", "LBL-ACANCEL", "LBL-AERR",
    ];
    for (i, id) in labels.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 30 * i as i32, 580, 26), &[("Caption", s("-"))]);
    }
    control(
        &mut form,
        "REST-1",
        ControlType::RestClient,
        (680, 500, 56, 56),
        &[("Mode", s("Sync")), ("BaseURL", s(&base)), ("AuthType", s("Bearer")), ("DefaultHeaders", s("X-One: 1\n# a comment\nnot a header"))],
    );
    control(&mut form, "REST-2", ControlType::RestClient, (620, 500, 56, 56), &[("BaseURL", s(&base)), ("DefaultMethod", s("post")), ("AuthType", s("APIKey"))]);
    control(&mut form, "REST-3", ControlType::RestClient, (560, 500, 56, 56), &[("Mode", s("Sync")), ("BaseURL", s(&nowhere))]);
    control(&mut form, "REST-4", ControlType::RestClient, (500, 500, 56, 56), &[("BaseURL", s(&nowhere))]);

    // ── Sync: REST-1 and REST-3 ──
    bind(&mut form, "BTN-GET", "onClick", "", &lines(&["MOVE REST-1::Get(\"users/7\") TO LBL-GET::Caption", "MOVE REST-1::StatusCode TO LBL-ST::Caption"]));
    bind(&mut form, "BTN-POST", "onClick", "", &lines(&["MOVE REST-1::Post(\"items\" \"{1}\") TO LBL-POST::Caption"]));
    bind(
        &mut form,
        "BTN-CALL",
        "onClick",
        "",
        &lines(&["MOVE REST-1::Call(\"patch\" \"?q=1\" \"zz\") TO LBL-CALL::Caption", "MOVE REST-1::Delete(\" \") TO LBL-DEL::Caption"]),
    );
    bind(
        &mut form,
        "BTN-HDR",
        "onClick",
        "       01 WS-URL                  PIC X(60).\n       01 WS-FULL                 PIC X(80).\n       01 WS-RESP                 PIC X(80).\n       01 WS-STATUS               PIC 9(4).",
        &lines(&[
            "REST-1::SetHeader(\"X-Shared\" \"s1\")",
            "MOVE REST-1::BaseURL TO WS-URL",
            "STRING WS-URL DELIMITED BY SPACE \"plain\" DELIMITED BY SIZE INTO WS-FULL",
            "COBOL::\"HTTP-GET\" ( WS-FULL WS-RESP WS-STATUS )",
            "MOVE WS-RESP TO LBL-HDR::Caption",
            "REST-1::ClearHeaders()",
            "MOVE REST-1::Get(\"after\") TO LBL-AFTER::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-404",
        "onClick",
        "       01 WS-B                    PIC X(10).\n       01 WS-S                    PIC 9(3).\n       01 WS-T                    PIC X(30).",
        &lines(&[
            "MOVE REST-1::Get(\"status/404\") TO WS-B",
            "MOVE REST-1::StatusCode TO WS-S",
            "STRING WS-B DELIMITED BY SPACE \" \" WS-S DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-404::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-ERR",
        "onClick",
        "       01 WS-B                    PIC X(200).\n       01 WS-S                    PIC 9(3).\n       01 WS-T                    PIC X(30).",
        &lines(&[
            "MOVE REST-3::Get(\"x\") TO WS-B",
            "MOVE REST-3::StatusCode TO WS-S",
            "IF WS-B = SPACES",
            "    STRING \"status \" WS-S \" no text\" DELIMITED BY SIZE INTO WS-T",
            "ELSE",
            "    STRING \"status \" WS-S \" with text\" DELIMITED BY SIZE INTO WS-T",
            "END-IF",
            "MOVE WS-T TO LBL-ERR::Caption",
        ]),
    );
    bind(&mut form, "BTN-TMO", "onClick", "", &lines(&["REST-1::SetTimeout(\"9\")", "MOVE REST-1::Timeout TO LBL-TMO::Caption"]));

    // ── Async: REST-2 and REST-4 ──
    bind(
        &mut form,
        "BTN-AGET",
        "onClick",
        "       01 WS-R                    PIC X(10).\n       01 WS-R2                   PIC X(10).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE REST-2::Get(\"users/8\") TO WS-R",
            "MOVE REST-2::IsBusy() TO WS-B",
            "MOVE REST-2::Get(\"users/9\") TO WS-R2",
            "STRING \"answer [\" WS-R \"] busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(
        &mut form,
        "REST-2",
        "onComplete",
        "       01 WS-C                    PIC 9(3).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "ADD 1 TO WS-ADONE",
            "MOVE REST-2::StatusCode TO WS-C",
            "MOVE REST-2::IsBusy() TO WS-B",
            "STRING \"done \" WS-ADONE \" status \" WS-C \" busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ADONE::Caption",
            "MOVE REST-2::ResponseBody TO LBL-ABODY::Caption",
        ]),
    );
    bind(&mut form, "REST-2", "onTimeout", "", &lines(&["MOVE REST-2::LastError TO LBL-ATIME::Caption"]));
    bind(&mut form, "REST-2", "onCancelled", "", &lines(&["MOVE \"cancelled\" TO LBL-ACANCEL::Caption"]));
    bind(&mut form, "BTN-ACALL", "onClick", "       01 WS-R                    PIC X(10).", &lines(&["MOVE REST-2::Call(\" \" \"items\" \"zz\") TO WS-R"]));
    bind(
        &mut form,
        "BTN-ASLOW",
        "onClick",
        "       01 WS-R                    PIC X(10).",
        &lines(&["MOVE 300 TO REST-2::TimeoutMs", "MOVE REST-2::Get(\"slow\") TO WS-R"]),
    );
    bind(
        &mut form,
        "BTN-ACANCEL",
        "onClick",
        "       01 WS-R                    PIC X(10).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(20).",
        &lines(&[
            "MOVE 0 TO REST-2::TimeoutMs",
            "MOVE REST-2::Get(\"slow\") TO WS-R",
            "REST-2::Cancel()",
            "MOVE REST-2::IsBusy() TO WS-B",
            "STRING \"cancel busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(&mut form, "BTN-AERR", "onClick", "       01 WS-R                    PIC X(10).", &lines(&["MOVE REST-4::Get(\"x\") TO WS-R"]));
    bind(
        &mut form,
        "REST-4",
        "onError",
        "       01 WS-S                    PIC 9(3).\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE REST-4::StatusCode TO WS-S",
            "IF REST-4::LastError = SPACES",
            "    STRING \"error status \" WS-S \" no text\" DELIMITED BY SIZE INTO WS-T",
            "ELSE",
            "    STRING \"error status \" WS-S \" with text\" DELIMITED BY SIZE INTO WS-T",
            "END-IF",
            "MOVE WS-T TO LBL-AERR::Caption",
        ]),
    );

    let cfrm = root.join("forms/parity-rest.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-rest.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("parityrest.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityrest\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-rest.cfrm\"]\ngenerated = [\"generated/parity-rest.cbl\"]\n",
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
    let sync = vec!["s1-verbs", "s2-headers", "s3-endings"];
    let sync_steps = vec![
        wait(200),
        click("BTN-GET"),
        click("BTN-POST"),
        click("BTN-CALL"),
        dump(),
        shot(sync[0]),
        click("BTN-HDR"),
        dump(),
        shot(sync[1]),
        click("BTN-404"),
        click("BTN-ERR"),
        click("BTN-TMO"),
        dump(),
        shot(sync[2]),
    ];
    let asynchronous = vec!["a1-get", "a2-call", "a3-timed-out", "a4-cancelled", "a5-error"];
    let async_steps = vec![
        wait(200),
        click("BTN-AGET"),
        wait(1500),
        dump(),
        shot(asynchronous[0]),
        click("BTN-ACALL"),
        wait(1500),
        dump(),
        shot(asynchronous[1]),
        click("BTN-ASLOW"),
        wait(4000),
        dump(),
        shot(asynchronous[2]),
        click("BTN-ACANCEL"),
        wait(4000),
        dump(),
        shot(asynchronous[3]),
        click("BTN-AERR"),
        wait(1500),
        dump(),
        shot(asynchronous[4]),
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

/// Run every script with `run` (the script's path, whether it is pinned): the dumps, in order.
fn run_all(scripts: &[(PathBuf, bool)], mut run: impl FnMut(&Path, bool) -> String) -> Vec<Value> {
    let mut out = Vec::new();
    for (path, pinned) in scripts {
        out.extend(dumps(&report(&run(path, *pinned))));
    }
    out
}

/// Which engine is about to run.
#[derive(Clone, Copy, PartialEq)]
enum Engine {
    Interpreter,
    Runner,
}

/// What both engines did with the same scripts.
struct Twin {
    interp: Vec<Value>,
    wasm: Vec<Value>,
    interp_shots: PathBuf,
    runner_shots: PathBuf,
    runner_ms: f64,
    build_ms: f64,
    interp_ms: f64,
    wasm_ms: f64,
}

/// Run `paths` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly
/// path and run them in the built program. `env` is given to both; `before` runs before each engine.
fn run_twin(root: &Path, manifest: &Path, paths: &[(PathBuf, bool)], env: &[(String, String)], before: &dyn Fn(Engine)) -> Twin {
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

    // ── 1. The interpreter: rcrun run-form ──
    before(Engine::Interpreter);
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let interp = run_all(paths, |script, pinned| {
        let mut c = Command::new(rcrun());
        c.args(["run-form", "forms/parity-rest.cfrm", "generated/parity-rest.cbl", "--designer", "--headless"])
            .arg(script)
            .args(["--headless-limit", "60"])
            .current_dir(root)
            .env("HOME", &home)
            .env("PRC_HEADLESS_SHOTS", &interp_shots)
            .envs(env.iter().map(|(k, v)| (k, v)));
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
    before(Engine::Runner);
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
    let exe = root.join("bin").join(format!("parityrest{}", std::env::consts::EXE_SUFFIX));
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
            .envs(env.iter().map(|(k, v)| (k, v)));
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

/// The comparison of the two engines' dumps and pictures: the checks, the lines that report them,
/// and the pixels compared and differing.
struct Comparison {
    checks: Vec<(String, bool)>,
    dump_lines: Vec<String>,
    shot_lines: Vec<String>,
    pixels_total: u64,
    differing_total: u64,
}

fn compare(twin: &Twin, shots: &[&str]) -> Comparison {
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), twin.interp.len() == shots.len() && twin.wasm.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.iter().zip(twin.wasm.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let (mut pixels_total, mut differing_total, mut shot_lines) = (0u64, 0u64, Vec::new());
    for name in shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&twin.interp_shots.join(&file), &twin.runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }
    Comparison { checks, dump_lines, shot_lines, pixels_total, differing_total }
}

/// Print a result block and fail on any failed check; the work folder is removed when all pass.
fn finish(title: &str, header: &[String], twin: &Twin, cmp: Comparison, mut checks: Vec<(String, bool)>, root: &Path) {
    let mut all = cmp.checks;
    all.append(&mut checks);
    let passed = all.iter().filter(|(_, ok)| *ok).count();
    println!("== {title} ==");
    for l in header {
        println!("{l}");
    }
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter runs {:.0} ms; runner runs {:.0} ms", twin.interp_ms, twin.wasm_ms);
    for l in &cmp.shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {}, differing: {}", cmp.pixels_total, cmp.differing_total);
    for l in &cmp.dump_lines {
        println!("{l}");
    }
    for (what, ok) in &all {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", all.len());
    let failed: Vec<&String> = all.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    if failed.is_empty() {
        let _ = fs::remove_dir_all(root);
    } else {
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
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
fn the_restclient_control_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t42b-form-{}", std::process::id()));
    let (port, closed) = (stub(), closed_port());
    let manifest = project(&root, port, closed);
    let scripts = scripts();
    let paths = write_scripts(&root, &scripts);
    // The credentials, from the environment, as a form file never holds one.
    let env: Vec<(String, String)> = [("REST-1", "t0k"), ("REST-2", "k2")]
        .iter()
        .map(|(ctl, key)| (cobolt_forms::connections::control_key_env(FORM, ctl), (*key).to_owned()))
        .collect();
    let twin = run_twin(&root, &manifest, &paths, &env, &|_| {});
    let shots: Vec<&str> = scripts.iter().flat_map(|(_, _, s, _)| s.iter().copied()).collect();
    let cmp = compare(&twin, &shots);
    let at = |i: usize, id: &str| twin.wasm.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let checks: Vec<(String, bool)> = vec![
        (
            "Get(\"users/7\") joins BaseURL with one slash, sends the Bearer token and DefaultHeaders' one header".into(),
            at(0, "LBL-GET").trim() == "GET /v1/users/7 a=Bearer t0k k=- one=1 s=- b=",
        ),
        ("StatusCode 200".into(), at(0, "LBL-ST").trim() == "200"),
        ("Post(\"items\" \"{1}\") sends its body".into(), at(0, "LBL-POST").trim() == "POST /v1/items a=Bearer t0k k=- one=1 s=- b={1}"),
        ("Call(\"patch\" \"?q=1\" \"zz\"): the verb in upper case, a query appended to BaseURL".into(), at(0, "LBL-CALL").trim() == "PATCH /v1?q=1 a=Bearer t0k k=- one=1 s=- b=zz"),
        ("Delete with an empty argument requests BaseURL".into(), at(0, "LBL-DEL").trim() == "DELETE /v1/ a=Bearer t0k k=- one=1 s=- b="),
        ("SetHeader reaches COBOL::\"HTTP-GET\" (one client per window)".into(), at(1, "LBL-HDR").trim() == "GET /v1/plain a=- k=- one=- s=s1 b="),
        ("ClearHeaders: the next request sends none of them".into(), at(1, "LBL-AFTER").trim() == "GET /v1/after a=Bearer t0k k=- one=1 s=- b="),
        ("a 404 is an answer: its body and its status".into(), at(2, "LBL-404").trim() == "missing 404"),
        ("a transport failure: status 0, the method answers why".into(), at(2, "LBL-ERR").trim() == "status 000 with text"),
        ("SetTimeout(\"9\") writes Timeout".into(), at(2, "LBL-TMO").trim() == "9"),
        ("an asynchronous Get answers nothing at once and the control is busy".into(), at(3, "LBL-ASYNC").trim() == "answer [          ] busy 1"),
        ("onComplete once (the second Get was ignored), status 200, no longer busy".into(), at(3, "LBL-ADONE").trim() == "done 01 status 200 busy 0"),
        ("its ResponseBody, with the API key header".into(), at(3, "LBL-ABODY").trim() == "GET /v1/users/8 a=- k=k2 one=- s=- b="),
        ("Call with no verb takes DefaultMethod (post)".into(), at(4, "LBL-ABODY").trim() == "POST /v1/items a=- k=k2 one=- s=- b=zz" && at(4, "LBL-ADONE").trim() == "done 02 status 200 busy 0"),
        ("TimeoutMs 300: onTimeout, LastError says so".into(), at(5, "LBL-ATIME").trim() == "No answer within 1 second: the call was cancelled."),
        ("the timed-out request's late answer is dropped".into(), at(5, "LBL-ADONE").trim() == "done 02 status 200 busy 0"),
        ("Cancel(): onCancelled, no longer busy, its late answer dropped".into(), at(6, "LBL-ACANCEL").trim() == "cancelled" && at(6, "LBL-ASYNC").trim() == "cancel busy 0" && at(6, "LBL-ADONE").trim() == "done 02 status 200 busy 0"),
        ("a transport failure on its own thread: onError, StatusCode 0, LastError says why".into(), at(7, "LBL-AERR").trim() == "error status 000 with text"),
    ];
    let header = vec![
        format!("form {FORM}: 4 RestClient controls (Sync with Bearer and DefaultHeaders, asynchronous with APIKey and DefaultMethod post, Sync and asynchronous where nothing listens), 12 buttons, 16 event handlers"),
        "  Sync: Get, Post, Call (lower-case verb, ?query), Delete (BaseURL), SetHeader shared with COBOL::\"HTTP-GET\", ClearHeaders, a 404, a transport failure, SetTimeout".into(),
        "  Async: Get answers nothing and busy, a second ignored, onComplete with ResponseBody / StatusCode / IsBusy, Call with DefaultMethod,".into(),
        "        TimeoutMs 300 and onTimeout, Cancel() and onCancelled, onError with StatusCode 0".into(),
        format!("a stub server on 127.0.0.1:{port}; sync script pinned (PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}); async script in real time; {} dumps, {} pictures", shots.len(), shots.len()),
    ];
    finish("the_restclient_control_agrees_on_both_engines (DESKTOP-WASM-001 T42b: R10, R11, ABI A46)", &header, &twin, cmp, checks, &root);
}
