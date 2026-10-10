// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 amendment A6 (R5–R7, AC3): AWS keys a program types in at run
//! time.
//!
//! `COBOL::"AWS-SET-CREDENTIALS"` hands a connection an access key id, a
//! secret access key and an optional session token. They live in this
//! process's memory, reach exactly one place — the environment of an AWS
//! server started for that connection, which then gets no profile — and
//! appear on no other surface. `AWS-CLEAR-CREDENTIALS` forgets them and ends
//! the server that held them; `AWS-CREDENTIALS-SET` answers Y or N.
//!
//! The planted values are assembled at run time and look like no real AWS
//! key, so the masking real keys get cannot hide a leak: every surface below
//! must simply never contain them. Everything runs against the fake MCP
//! server; no AWS account is involved. The connection registry and the server
//! pool are process-global, so the tests take turns.

#![cfg(feature = "aws")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::aws::connections::{self, AwsConnection};
use cobolt_runtime::aws::pool::{self, Budget, ServerKey};
use cobolt_runtime::aws::{credentials, ops, process};
use cobolt_runtime::{FormEvent, Interpreter};
use serde_json::json;

static TURN: Mutex<()> = Mutex::new(());

// ── What the runtime logs, captured ─────────────────────────────────────────

/// Every `tracing` event any thread emits during the tests, as text.
fn tracing_log() -> &'static Arc<Mutex<String>> {
    static LOG: OnceLock<Arc<Mutex<String>>> = OnceLock::new();
    LOG.get_or_init(|| Arc::new(Mutex::new(String::new())))
}

struct Capture(Arc<Mutex<String>>);

struct Fields<'a>(&'a mut String);

impl tracing::field::Visit for Fields<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, "{}={value:?} ", field.name());
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut line = String::new();
        attrs.record(&mut Fields(&mut line));
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push_str(&line);
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut line = String::new();
        values.record(&mut Fields(&mut line));
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push_str(&line);
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = String::new();
        event.record(&mut Fields(&mut line));
        let mut log = self.0.lock().unwrap_or_else(|e| e.into_inner());
        log.push_str(&line);
        log.push('\n');
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// Where the runtime's event trace is written (`COBOLT_EVENT_TRACE`, which
/// also copies every DISPLAY and Verbose line), switched on for these tests.
fn trace_file() -> PathBuf {
    std::env::temp_dir().join(format!("prc-092-a6-trace-{}.log", std::process::id()))
}

/// Once per test binary, before any program runs: capture every `tracing`
/// event, and turn the event trace on.
fn init() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = tracing::subscriber::set_global_default(Capture(tracing_log().clone()));
        // Every test of this binary calls this first, and the OnceLock holds
        // the others back until it returns, so nothing reads the environment
        // while it changes.
        std::env::set_var("COBOLT_EVENT_TRACE", "all");
        std::env::set_var("COBOLT_EVENT_TRACE_FILE", trace_file());
    });
}

// ── Planted values ──────────────────────────────────────────────────────────

/// Recognisable fake keys, assembled at run time — never a literal that
/// resembles a real key.
#[derive(Clone)]
struct Planted {
    key: String,
    secret: String,
    token: String,
}

fn planted(tag: &str) -> Planted {
    let n = std::process::id().to_string();
    Planted {
        key: ["prc", "a6", tag, "keyid", &n].join("-"),
        secret: ["prc", "a6", tag, "secret", &n].join("_"),
        token: ["prc", "a6", tag, "token", &n].join("."),
    }
}

impl Planted {
    /// Which of the three `text` holds.
    fn found_in(&self, text: &str) -> Vec<&'static str> {
        let mut found = Vec::new();
        for (name, v) in [("key id", &self.key), ("secret", &self.secret), ("token", &self.token)] {
            if text.contains(v.as_str()) {
                found.push(name);
            }
        }
        found
    }
}

// ── Fake servers, one connection each ───────────────────────────────────────

struct Fake {
    dir: PathBuf,
    conn: AwsConnection,
}

impl Drop for Fake {
    fn drop(&mut self) {
        pool::stop(&ServerKey { connection: self.conn.id.clone(), server: "lambda".into() });
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fake {
    fn env(&self) -> String {
        std::fs::read_to_string(self.dir.join("env.txt")).unwrap_or_default()
    }
    fn argv(&self) -> String {
        std::fs::read_to_string(self.dir.join("argv.txt")).unwrap_or_default()
    }
    fn pids(&self) -> Vec<u32> {
        std::fs::read_to_string(self.dir.join("pids.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    }
    /// The variable `name`'s values in the environment the server was given.
    fn var(&self, name: &str) -> Vec<String> {
        let prefix = format!("{name}=");
        self.env().lines().filter_map(|l| l.strip_prefix(&prefix)).map(str::to_owned).collect()
    }
}

/// A connection named `conn-<tag>` (profile `profile`) whose Lambda server is
/// the fake, run with `extra` merged into its script; `command` replaces the
/// fake (a program that does not exist, say).
fn fake(tag: &str, profile: &str, extra: serde_json::Value, command: Option<&str>) -> Fake {
    let dir = std::env::temp_dir().join(format!("prc-092-a6-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let reply = r#"Function app-f returned: [{"id":1,"name":"Ana"}]"#;
    let mut script = json!({
        "tools": [{"name": "app_f", "inputSchema": {"type": "object", "properties": {"parameters": {"type": "object"}}}}],
        "answers": {"app_f": {"content": [{"type": "text", "text": reply}], "isError": false}},
        "pid_file": dir.join("pids.txt").to_string_lossy(),
        "call_log": dir.join("calls.jsonl").to_string_lossy(),
        "env_dump": dir.join("env.txt").to_string_lossy(),
        "argv_dump": dir.join("argv.txt").to_string_lossy(),
    });
    for (k, v) in extra.as_object().unwrap() {
        script[k] = v.clone();
    }
    let script_path = dir.join("script.json");
    std::fs::write(&script_path, script.to_string()).unwrap();
    let routes_override = format!(
        "[servers.lambda]\ncommand = '{}'\nargs = ['{}']\n",
        command.unwrap_or(env!("CARGO_BIN_EXE_fake_mcp")),
        script_path.display()
    );
    let conn = AwsConnection {
        id: format!("id-{tag}"),
        name: format!("conn-{tag}"),
        profile: profile.into(),
        region: "eu-west-1".into(),
        routes_override,
        ..Default::default()
    };
    Fake { dir, conn }
}

fn publish(fakes: &[&Fake]) {
    connections::publish(fakes.iter().map(|f| f.conn.clone()).collect());
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    false
}

// ── COBOL ───────────────────────────────────────────────────────────────────

fn program(body: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AWSKEYS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-STATUS         PIC X(200).
       01 WS-FLAG           PIC X.
       01 WS-NAME           PIC X(30) VALUE "app-f".
       01 WS-PAY            PIC X(200) VALUE '{{"id": 7}}'.
       01 WS-RC             PIC X(400).
       01 WS-I              PIC 9 VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
{body}
           STOP RUN.
"#
    );
    let r = parse(tokenize(&src, SourceFormat::Free));
    assert!(r.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", r.diagnostics);
    r.program.expect("no program")
}

/// `AWS-SET-CREDENTIALS` for `conn`, the status displayed as `SET <conn>`.
fn set(conn: &str, p: &Planted) -> String {
    format!(
        "           COBOL::\"AWS-SET-CREDENTIALS\" ( \"{conn}\" \"{}\" \"{}\" \"{}\" WS-STATUS )\n           DISPLAY \"SET {conn} \" WS-STATUS\n",
        p.key, p.secret, p.token
    )
}

fn set_raw(conn: &str, key: &str, secret: &str, token: &str) -> String {
    format!(
        "           COBOL::\"AWS-SET-CREDENTIALS\" ( \"{conn}\" \"{key}\" \"{secret}\" \"{token}\" WS-STATUS )\n           DISPLAY \"SET {conn} \" WS-STATUS\n"
    )
}

fn clear(conn: &str) -> String {
    format!("           COBOL::\"AWS-CLEAR-CREDENTIALS\" ( \"{conn}\" WS-STATUS )\n           DISPLAY \"CLEAR {conn} \" WS-STATUS\n")
}

fn flag(conn: &str) -> String {
    format!("           COBOL::\"AWS-CREDENTIALS-SET\" ( \"{conn}\" WS-FLAG )\n           DISPLAY \"FLAG {conn} \" WS-FLAG\n")
}

/// A synchronous `Invoke` on `ctrl`: what it returned and its `LastError`.
fn invoke(ctrl: &str) -> String {
    format!(
        "           INVOKE {ctrl} \"Invoke\" USING WS-NAME WS-PAY RETURNING WS-RC\n           DISPLAY \"BODY {ctrl} \" WS-RC\n           MOVE {ctrl}::LastError TO WS-RC\n           DISPLAY \"ERROR {ctrl} \" WS-RC\n"
    )
}

/// Every property of the control, and its first row, displayed.
fn every_property(ctrl: &str) -> String {
    let c = cobolt_forms::aws_catalog::by_name("AwsLambda").unwrap();
    let mut names: Vec<&str> = cobolt_forms::aws_catalog::SHARED_SEEDS.iter().map(|(n, _)| *n).collect();
    names.extend(c.seeds.iter().map(|(n, _)| *n));
    names.extend(c.runtime.iter().copied());
    let mut out = String::new();
    for n in names {
        let _ = writeln!(out, "           MOVE {ctrl}::{n} TO WS-RC\n           DISPLAY \"PROP {ctrl}.{n} \" WS-RC");
    }
    let _ = writeln!(out, "           INVOKE {ctrl} \"GetRow\" USING WS-I RETURNING WS-RC\n           DISPLAY \"ROW {ctrl} \" WS-RC");
    out
}

struct Run {
    lines: Vec<String>,
    millis: u128,
}

impl Run {
    fn line(&self, prefix: &str) -> String {
        self.lines.iter().find_map(|l| l.strip_prefix(prefix)).map(|s| s.trim().to_owned()).unwrap_or_default()
    }
    fn text(&self) -> String {
        self.lines.join("\n")
    }
}

/// Run `prog` with `controls` (id, connection, extra properties) seeded as
/// AwsLambda controls; the program ends by itself, or at the `onClick` sent
/// after `quit_after`.
fn run(prog: cobolt_ast::program::Program, controls: &[(&str, &str, &[(&str, &str)])], quit_after: Option<Duration>) -> Run {
    run_with(prog, controls, Vec::new(), quit_after)
}

/// [`run`], with `others` (id, class, properties) seeded beside the AWS
/// controls.
fn run_with(
    prog: cobolt_ast::program::Program,
    controls: &[(&str, &str, &[(&str, &str)])],
    others: Vec<(String, String, Vec<(String, String)>)>,
    quit_after: Option<Duration>,
) -> Run {
    init();
    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let seeded: Vec<(String, String, Vec<(String, String)>)> = controls
        .iter()
        .map(|(id, conn, own)| {
            let mut props: Vec<(String, String)> = [
                ("Connection", *conn),
                ("Mode", "Sync"),
                ("AllowWrite", "true"),
                ("TimeoutMs", "5000"),
                ("StartTimeoutMs", "10000"),
                ("FunctionName", ""),
                ("Verbose", "true"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
            for (k, v) in own.iter() {
                props.retain(|(n, _)| n != k);
                props.push((k.to_string(), v.to_string()));
            }
            (id.to_string(), "AwsLambda".to_string(), props)
        })
        .chain(others)
        .collect();
    let started = Instant::now();
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
        interp.seed_objects(seeded);
        let _ = interp.run();
    });
    if let Some(d) = quit_after {
        thread::sleep(d);
        let _ = event_tx.send(FormEvent::new("BTN-1", "onClick"));
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the program never finished");
        thread::sleep(Duration::from_millis(10));
    }
    drop(event_tx);
    Run { lines: display_rx.try_iter().map(|l| l.trim_end().to_string()).collect(), millis: started.elapsed().as_millis() }
}

// ── Scanning ────────────────────────────────────────────────────────────────

/// One surface scanned: its name, how many bytes, what it held.
struct Scan {
    surface: String,
    bytes: usize,
    found: Vec<&'static str>,
}

fn scan(surface: impl Into<String>, text: &str, p: &Planted) -> Scan {
    Scan { surface: surface.into(), bytes: text.len(), found: p.found_in(text) }
}

/// Every file under `dir` (recursively), except `skip`.
fn files_under(dir: &Path, skip: &[PathBuf], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if skip.iter().any(|s| p.starts_with(s) || p == *s) {
            continue;
        }
        if p.is_dir() {
            files_under(&p, skip, out);
        } else if p.metadata().map(|m| m.len() < 16 * 1024 * 1024).unwrap_or(false) {
            out.push(p);
        }
    }
}

// ── The tests ───────────────────────────────────────────────────────────────

/// AC3 / R5–R7, the whole surface: the server that should receive the keys
/// gets exactly the three variables and no profile; nothing else — another
/// connection's server, an unrelated child, the arguments, the protocol
/// traffic, Verbose, every property and row, every error (provoked six ways),
/// the logs, the event trace, `Debug` renderings, a saved form, the
/// connection record and the files on disk — holds any of them.
#[test]
fn the_keys_reach_their_server_and_no_other_surface() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    init();
    let p = planted("scan");
    // A: the connection given keys. B: another connection, on its profile.
    let a = fake("scan-a", "test", json!({"stderr_on_call": "fake: working"}), None);
    let b = fake("scan-b", "other", json!({}), None);
    // Failures, each on a connection that holds the keys.
    let start_fails = fake("scan-start", "test", json!({"exit_on_start": "fake: cannot start"}), None);
    let tool_fails = fake("scan-tool", "test", json!({"fail_with": "ValidationException: no such function"}), None);
    let refused = fake("scan-refused", "test", json!({"fail_with": "InvalidClientTokenId: The security token included in the request is invalid."}), None);
    let silent = fake("scan-silent", "test", json!({"silent": ["app_f"]}), None);
    let missing = fake("scan-missing", "test", json!({}), Some("prc-no-such-program-092"));
    let all = [&a, &b, &start_fails, &tool_fails, &refused, &silent, &missing];
    publish(&all);

    let mut body = String::new();
    // By NAME for A (its id is checked below), and for every failing one.
    for f in [&a, &start_fails, &tool_fails, &refused, &silent, &missing] {
        body.push_str(&set(&f.conn.name, &p));
    }
    body.push_str(&flag(&a.conn.id)); // by id: the same connection
    body.push_str(&flag(&b.conn.name));
    body.push_str(&set("conn-no-such", &p));
    body.push_str(&clear("conn-no-such"));
    for ctrl in ["AWS-A", "AWS-B", "AWS-START", "AWS-TOOL", "AWS-REFUSED", "AWS-SILENT", "AWS-MISSING"] {
        body.push_str(&invoke(ctrl));
    }
    body.push_str(&every_property("AWS-A"));
    // One asynchronous call too: Verbose prints its response.
    body.push_str(
        "           INVOKE AWS-ASYNC \"Invoke\" USING WS-NAME WS-PAY RETURNING WS-RC\n\
                    PERFORM UNTIL COBOL-QUIT = 1\n\
                        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
                        DISPLAY \"EVENT \" COBOL-EVENT-ID\n\
                        IF COBOL-EVENT-ID = \"onComplete\" OR COBOL-EVENT-ID = \"onError\"\n\
                            MOVE 1 TO COBOL-QUIT\n\
                        END-IF\n\
                    END-PERFORM\n",
    );
    let controls: [(&str, &str, &[(&str, &str)]); 8] = [
        ("AWS-A", &a.conn.name, &[]),
        ("AWS-B", &b.conn.name, &[]),
        ("AWS-START", &start_fails.conn.name, &[]),
        ("AWS-TOOL", &tool_fails.conn.name, &[]),
        ("AWS-REFUSED", &refused.conn.name, &[]),
        ("AWS-SILENT", &silent.conn.name, &[("TimeoutMs", "300")]),
        ("AWS-MISSING", &missing.conn.name, &[]),
        ("AWS-ASYNC", &a.conn.name, &[("Mode", "Async")]),
    ];
    let r = run(program(&body), &controls, None);

    // ── The one place they belong ──
    for f in [&a, &start_fails, &tool_fails, &refused, &silent, &missing] {
        assert_eq!(r.line(&format!("SET {} ", f.conn.name)), "OK", "{:?}", r.lines);
    }
    assert_eq!(r.line(&format!("FLAG {} ", a.conn.id)), "Y", "found by id as well as by name");
    assert_eq!(r.line(&format!("FLAG {} ", b.conn.name)), "N");
    assert_eq!(r.line("BODY AWS-A "), r#"[{"id":1,"name":"Ana"}]"#, "{:?}", r.lines);
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [p.key.clone()]);
    assert_eq!(a.var("AWS_SECRET_ACCESS_KEY"), [p.secret.clone()]);
    assert_eq!(a.var("AWS_SESSION_TOKEN"), [p.token.clone()]);
    assert!(a.var("AWS_SECURITY_TOKEN").is_empty());
    assert!(a.var("AWS_PROFILE").is_empty() && a.var("AWS_DEFAULT_PROFILE").is_empty(), "no profile with keys:\n{}", a.env());
    assert_eq!(a.var("AWS_REGION"), ["eu-west-1"]);
    assert!(!a.argv().contains("--profile") && !a.argv().contains("test"), "no profile argument:\n{}", a.argv());
    // B, on its profile, never sees them.
    assert_eq!(b.var("AWS_PROFILE"), ["other"], "{}", b.env());
    assert!(b.var("AWS_ACCESS_KEY_ID").is_empty());

    // ── The errors say what happened, in words ──
    let unknown = r.line("SET conn-no-such ");
    assert!(unknown.contains("no AWS connection named \"conn-no-such\""), "{unknown}");
    assert!(r.line("CLEAR conn-no-such ").contains("no AWS connection named"), "{:?}", r.lines);
    let refused_error = r.line("ERROR AWS-REFUSED ");
    assert!(refused_error.contains("AWS credentials") && refused_error.contains("AWS-SET-CREDENTIALS"), "{refused_error}");
    assert!(!refused_error.contains("profile"), "keys, not a profile, were refused: {refused_error}");
    for ctrl in ["AWS-START", "AWS-TOOL", "AWS-SILENT", "AWS-MISSING"] {
        assert!(!r.line(&format!("ERROR {ctrl} ")).is_empty(), "{ctrl} reported nothing: {:?}", r.lines);
    }
    assert!(r.lines.iter().any(|l| l.contains("[aws AWS-A] request")), "Verbose narrated the call: {:?}", r.lines);
    assert!(r.lines.iter().any(|l| l.contains("[aws AWS-ASYNC] response")), "Verbose narrated the async answer: {:?}", r.lines);

    assert!(credentials::is_set(&a.conn.name) && !credentials::is_set(&b.conn.name), "the Rust view agrees with Y/N");

    // ── Every other surface ──
    // The scanners themselves must be live: a tracing event is captured, and
    // the one file the keys belong in is found holding all three.
    tracing::warn!(probe = "a6-capture-is-live");
    assert!(tracing_log().lock().unwrap().contains("a6-capture-is-live"), "the tracing capture is not installed");
    assert_eq!(p.found_in(&a.env()), ["key id", "secret", "token"], "the scanner finds them where they belong");
    let mut scans = vec![
        scan("program output: Verbose, properties, row, statuses, errors", &r.text(), &p),
        scan("tracing events, every level", &tracing_log().lock().unwrap(), &p),
        scan("event trace file (COBOLT_EVENT_TRACE)", &std::fs::read_to_string(trace_file()).unwrap_or_default(), &p),
        scan("this process's own environment", &std::env::vars().map(|(k, v)| format!("{k}={v}\n")).collect::<String>(), &p),
        scan("server A's arguments", &a.argv(), &p),
        scan("server B's environment (another connection)", &b.env(), &p),
        scan("server B's arguments", &b.argv(), &p),
    ];
    for f in all {
        scan_into(&mut scans, &format!("{}: protocol traffic", f.conn.name), &f.dir.join("calls.jsonl"), &p);
        let key = ServerKey { connection: f.conn.id.clone(), server: "lambda".into() };
        scans.push(scan(format!("{}: server stderr kept by the runtime", f.conn.name), &pool::stderr_of(&key).unwrap_or_default(), &p));
    }
    // An unrelated child: inherits this process's environment as is.
    let other = std::env::temp_dir().join(format!("prc-092-a6-child-{}", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("script.json"), json!({"env_dump": other.join("env.txt").to_string_lossy()}).to_string()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_fake_mcp"))
        .arg(other.join("script.json"))
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    scans.push(scan("an unrelated child's environment", &std::fs::read_to_string(other.join("env.txt")).unwrap(), &p));
    let _ = std::fs::remove_dir_all(&other);

    // Debug renderings of everything that holds them.
    let prop = |name: &str| if name == "AllowWrite" { "true".to_string() } else { String::new() };
    let args = vec!["app-f".to_string(), "{}".to_string()];
    let view = ops::ControlView {
        control_type: "AwsLambda",
        method: "Invoke",
        args: &args,
        prop: &prop,
        control: "AWS-A",
        allow_write: true,
        connection_allows_write: true,
    };
    let prepared = ops::prepare(&view, &a.conn).expect("prepares");
    assert!(prepared.launch.credentials.is_some(), "the prepared launch carries the keys");
    assert_eq!(prepared.launch.profile, None, "…and no profile");
    let outcome = ops::execute(
        &ops::prepare(&view, &refused.conn).unwrap(),
        Budget { call: Duration::from_secs(5), start: Duration::from_secs(10) },
    );
    let debug = format!(
        "{prepared:?}\n{prepared:#?}\n{:?}\n{:#?}\n{outcome:?}\n{:?}",
        prepared.launch,
        prepared.launch.credentials,
        connections::find(&a.conn.name)
    );
    scans.push(scan("Debug of Prepared, Launch, Credentials, the outcome, the connection", &debug, &p));

    // What a project and a form are made of: the connection record (the
    // project file and a built application's baked catalogue) and a .cfrm
    // holding every property value the running control reported.
    let record = cobolt_forms::connections::Catalogue { aws: connections::find(&a.conn.name).into_iter().collect(), ..Default::default() };
    let record_path = a.dir.join("catalogue.json");
    std::fs::write(&record_path, record.to_json()).unwrap();
    let mut form = cobolt_forms::Form::new("KEYS-FORM", "Keys", 400, 300);
    let mut ctl = cobolt_forms::Control::new("AWS-A", cobolt_forms::ControlType::AwsLambda, 0, 0);
    for l in r.lines.iter().filter_map(|l| l.strip_prefix("PROP AWS-A.")) {
        let (name, value) = l.split_once(' ').unwrap_or((l, ""));
        ctl.set_prop(name.to_string(), value.trim().to_string());
    }
    form.add_control(ctl);
    let cfrm = a.dir.join("keys-form.cfrm");
    cobolt_forms::save_form(&form, &cfrm).expect("saves");
    assert!(std::fs::read_to_string(&cfrm).unwrap().contains("AWS-A"));

    // Every file the run could have written: these tests' folders, the
    // working directory, the temp directory — the servers' environment dumps
    // excepted, which is where the keys belong (server A's, and those of the
    // failing servers that were given them too; B's is scanned above).
    let mut files = Vec::new();
    for f in all {
        files_under(&f.dir, &[f.dir.join("env.txt")], &mut files);
    }
    let cwd = std::env::current_dir().unwrap();
    files_under(&cwd, &[cwd.join("target")], &mut files);
    for e in std::fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
        let path = e.path();
        if path.is_file() && path.metadata().map(|m| m.len() < 16 * 1024 * 1024).unwrap_or(false) {
            files.push(path);
        }
    }
    let mut file_bytes = 0;
    let mut file_hits = Vec::new();
    for path in &files {
        if let Ok(bytes) = std::fs::read(path) {
            file_bytes += bytes.len();
            if !p.found_in(&String::from_utf8_lossy(&bytes)).is_empty() {
                file_hits.push(path.display().to_string());
            }
        }
    }
    scans.push(Scan {
        surface: format!("{} files on disk (a saved .cfrm and the connection record among them)", files.len()),
        bytes: file_bytes,
        found: if file_hits.is_empty() { vec![] } else { vec!["a file"] },
    });
    assert!(file_hits.is_empty(), "the keys were written to disk: {file_hits:?}");

    // ── Summary ──
    let leaked: Vec<&Scan> = scans.iter().filter(|s| !s.found.is_empty()).collect();
    eprintln!("\n  ── A6: AWS keys typed in at run time ({} ms) ──", r.millis);
    eprintln!("    receiver  server A: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_SESSION_TOKEN; no AWS_PROFILE; no --profile");
    eprintln!("    errors    unknown connection: {unknown}");
    eprintln!("              keys refused:       {refused_error}");
    for ctrl in ["AWS-START", "AWS-TOOL", "AWS-SILENT", "AWS-MISSING"] {
        eprintln!("              {ctrl:<19} {}", r.line(&format!("ERROR {ctrl} ")));
    }
    for s in &scans {
        eprintln!("    {:<74} {:>9} bytes  {}", s.surface, s.bytes, if s.found.is_empty() { "clean".to_string() } else { format!("HELD {:?}", s.found) });
    }
    eprintln!("    {} surfaces scanned, {} held a planted value", scans.len(), leaked.len());
    assert!(leaked.is_empty(), "planted values leaked: {:?}", leaked.iter().map(|s| (&s.surface, &s.found)).collect::<Vec<_>>());
}

fn scan_into(scans: &mut Vec<Scan>, surface: &str, path: &Path, p: &Planted) {
    scans.push(scan(surface, &std::fs::read_to_string(path).unwrap_or_default(), p));
}

/// R6: clearing the keys ends the server that held them at once, and the next
/// call runs on a fresh server with the profile and no keys.
#[test]
fn clearing_the_keys_ends_their_server_and_the_next_one_has_none() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = planted("clear");
    let a = fake("clear", "test", json!({}), None);
    publish(&[&a]);
    let name = a.conn.name.clone();
    let controls: [(&str, &str, &[(&str, &str)]); 1] = [("AWS-A", &name, &[])];

    let first = run(program(&format!("{}{}{}", set(&name, &p), invoke("AWS-A"), flag(&name))), &controls, None);
    assert_eq!(first.line(&format!("FLAG {name} ")), "Y");
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [p.key.clone()]);
    let held_by = a.pids();
    assert_eq!(held_by.len(), 1);
    assert!(alive(held_by[0]) || cfg!(not(unix)), "the server runs while the keys are set");

    let cleared = run(program(&format!("{}{}", clear(&name), flag(&name))), &controls, None);
    assert_eq!(cleared.line(&format!("CLEAR {name} ")), "OK");
    assert_eq!(cleared.line(&format!("FLAG {name} ")), "N");
    assert!(!alive(held_by[0]), "the server holding the keys must end when they are cleared");

    let after = run(program(&invoke("AWS-A")), &controls, None);
    assert_eq!(after.line("BODY AWS-A "), r#"[{"id":1,"name":"Ana"}]"#, "{:?}", after.lines);
    assert_eq!(a.pids().len(), 2, "a fresh server");
    assert!(p.found_in(&a.env()).is_empty(), "the fresh server holds none of the keys:\n{}", a.env());
    assert!(a.var("AWS_ACCESS_KEY_ID").is_empty());
    assert_eq!(a.var("AWS_PROFILE"), ["test"], "back on the profile");
    eprintln!(
        "\n  ── A6 clear ── set+call {} ms, clear {} ms (server {} ended), next call {} ms on server {} with AWS_PROFILE=test and no keys",
        first.millis, cleared.millis, held_by[0], after.millis, a.pids()[1]
    );
}

/// R5: new keys end the server holding the old ones at once; the next call's
/// server has the new keys and none of the old.
#[test]
fn changing_the_keys_restarts_the_server_with_the_new_ones() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let old = planted("old");
    let new = planted("new");
    let a = fake("change", "test", json!({}), None);
    publish(&[&a]);
    let name = a.conn.name.clone();
    let controls: [(&str, &str, &[(&str, &str)]); 1] = [("AWS-A", &name, &[])];

    let first = run(program(&format!("{}{}", set(&name, &old), invoke("AWS-A"))), &controls, None);
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [old.key.clone()], "{:?}", first.lines);
    let old_pid = a.pids()[0];

    let changed = run(program(&set(&name, &new)), &controls, None);
    assert_eq!(changed.line(&format!("SET {name} ")), "OK");
    assert!(!alive(old_pid), "the server holding the old keys must end when they change");

    let after = run(program(&invoke("AWS-A")), &controls, None);
    assert_eq!(after.line("BODY AWS-A "), r#"[{"id":1,"name":"Ana"}]"#, "{:?}", after.lines);
    assert_eq!(a.pids().len(), 2);
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [new.key.clone()]);
    assert_eq!(a.var("AWS_SECRET_ACCESS_KEY"), [new.secret.clone()]);
    assert_eq!(a.var("AWS_SESSION_TOKEN"), [new.token.clone()]);
    assert!(old.found_in(&a.env()).is_empty(), "none of the old keys");
    eprintln!(
        "\n  ── A6 change ── first server {old_pid} ended at the change ({} ms); the next call ({} ms) ran on {} with the new keys only",
        changed.millis,
        after.millis,
        a.pids()[1]
    );
}

/// The pool's own guarantee, beneath the built-ins: a launch with other keys
/// is another launch, so the running server is replaced and its process ends.
#[test]
fn a_launch_with_other_keys_replaces_the_running_server() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    init();
    let one = planted("pool1");
    let two = planted("pool2");
    let a = fake("pool", "test", json!({}), None);
    let key = ServerKey { connection: a.conn.id.clone(), server: "lambda".into() };
    let base = process::Launch {
        command: env!("CARGO_BIN_EXE_fake_mcp").into(),
        args: vec![a.dir.join("script.json").to_string_lossy().into_owned()],
        profile: Some("test".into()),
        ..Default::default()
    };
    let budget = Budget { call: Duration::from_secs(5), start: Duration::from_secs(10) };
    let keys = |p: &Planted| Arc::new(process::Credentials::new(p.key.clone(), p.secret.clone(), p.token.clone()));
    let first = base.clone().with_credentials(keys(&one));
    pool::call_tool(&key, &first, cobolt_mcp::Mode::Handshake, "app_f", json!({}), budget).unwrap();
    pool::call_tool(&key, &first, cobolt_mcp::Mode::Handshake, "app_f", json!({}), budget).unwrap();
    assert_eq!(a.pids().len(), 1, "the same launch shares one server");
    let second = base.clone().with_credentials(keys(&two));
    pool::call_tool(&key, &second, cobolt_mcp::Mode::Handshake, "app_f", json!({}), budget).unwrap();
    let pids = a.pids();
    assert_eq!(pids.len(), 2, "other keys, another server");
    assert!(!alive(pids[0]), "the old server ended");
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [two.key.clone()]);
    pool::call_tool(&key, &base, cobolt_mcp::Mode::Handshake, "app_f", json!({}), budget).unwrap();
    assert_eq!(a.pids().len(), 3, "no keys at all is another launch too");
    assert!(two.found_in(&a.env()).is_empty() && one.found_in(&a.env()).is_empty());
    eprintln!("\n  ── A6 pool ── same keys: 1 server for 2 calls; other keys: replaced (old ended); none: replaced again");
}

/// R5: an empty token (spaces) leaves AWS_SESSION_TOKEN out.
#[test]
fn an_empty_token_is_left_out_of_the_environment() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = planted("notoken");
    let a = fake("notoken", "test", json!({}), None);
    publish(&[&a]);
    let name = a.conn.name.clone();
    let controls: [(&str, &str, &[(&str, &str)]); 1] = [("AWS-A", &name, &[])];
    let r = run(program(&format!("{}{}", set_raw(&name, &p.key, &p.secret, "    "), invoke("AWS-A"))), &controls, None);
    assert_eq!(r.line(&format!("SET {name} ")), "OK");
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [p.key.clone()]);
    assert_eq!(a.var("AWS_SECRET_ACCESS_KEY"), [p.secret.clone()]);
    assert!(a.var("AWS_SESSION_TOKEN").is_empty(), "{}", a.env());
    assert!(a.var("AWS_PROFILE").is_empty());
    eprintln!("\n  ── A6 no token ── key id and secret passed, AWS_SESSION_TOKEN left out ({} ms)", r.millis);
}

/// The Developer's Guide example, as written there: the keys go straight
/// from the text boxes the user typed them into, which are cleared at once;
/// the status says OK. Nothing but the server's environment keeps them.
#[test]
fn the_guide_example_hands_over_the_text_boxes_and_clears_them() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = planted("guide");
    let a = fake("guide", "test", json!({}), None);
    publish(&[&a]);
    let name = a.conn.name.clone();
    let body = format!(
        r#"           COBOL::"AWS-SET-CREDENTIALS" ( "{name}" TXT-KEY-ID::Text
               TXT-SECRET::Text TXT-TOKEN::Text WS-STATUS )
           MOVE SPACES TO TXT-KEY-ID::Text TXT-SECRET::Text TXT-TOKEN::Text
           IF WS-STATUS = "OK"
               MOVE "Keys accepted for this session" TO LBL-STATE::Caption
           ELSE
               MOVE WS-STATUS TO LBL-STATE::Caption
           END-IF
           MOVE LBL-STATE::Caption TO WS-RC
           DISPLAY "STATE " WS-RC
           MOVE TXT-KEY-ID::Text TO WS-RC
           DISPLAY "LEFT KEY [" WS-RC "]"
           MOVE TXT-SECRET::Text TO WS-RC
           DISPLAY "LEFT SECRET [" WS-RC "]"
{}"#,
        invoke("AWS-A")
    );
    let text_box = |id: &str, value: &str| {
        (id.to_string(), "TextBox".to_string(), vec![("Text".to_string(), value.to_string())])
    };
    let others = vec![
        text_box("TXT-KEY-ID", &p.key),
        text_box("TXT-SECRET", &p.secret),
        text_box("TXT-TOKEN", &p.token),
        ("LBL-STATE".to_string(), "Label".to_string(), vec![("Caption".to_string(), String::new())]),
    ];
    let controls: [(&str, &str, &[(&str, &str)]); 1] = [("AWS-A", &name, &[])];
    let r = run_with(program(&body), &controls, others, None);
    assert_eq!(r.line("STATE "), "Keys accepted for this session", "{:?}", r.lines);
    assert!(r.lines.iter().any(|l| l.trim_end() == "LEFT KEY [" || l.starts_with("LEFT KEY [ ")), "{:?}", r.lines);
    assert!(p.found_in(&r.text()).is_empty(), "{:?}", r.lines);
    assert_eq!(a.var("AWS_ACCESS_KEY_ID"), [p.key.clone()]);
    assert_eq!(a.var("AWS_SECRET_ACCESS_KEY"), [p.secret.clone()]);
    assert_eq!(a.var("AWS_SESSION_TOKEN"), [p.token.clone()]);
    assert_eq!(r.line("BODY AWS-A "), r#"[{"id":1,"name":"Ana"}]"#, "{:?}", r.lines);
    eprintln!("\n  ── A6 Guide example ── text boxes handed over and cleared; the call ran on the keys ({} ms)", r.millis);
}

/// R5/R6: a connection the project does not have, or a missing key or
/// secret, is refused in words — and nothing is stored anywhere.
#[test]
fn a_refused_set_stores_nothing() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = planted("refused");
    let a = fake("refused", "test", json!({}), None);
    let b = fake("refused-b", "other", json!({}), None);
    publish(&[&a, &b]);
    let name = a.conn.name.clone();
    let body = [
        set("conn-nope", &p),
        flag("conn-nope"),
        set_raw(&name, "  ", &p.secret, &p.token),
        set_raw(&name, &p.key, " ", ""),
        set_raw(&name, &format!("{} x", p.key), &p.secret, ""),
        flag(&name),
        // An empty name means the project's only connection — and this
        // project has two.
        set("", &p),
        invoke("AWS-A"),
    ]
    .concat();
    let controls: [(&str, &str, &[(&str, &str)]); 1] = [("AWS-A", &name, &[])];
    let r = run(program(&body), &controls, None);
    let statuses: Vec<&str> = r.lines.iter().filter_map(|l| l.strip_prefix("SET ")).collect();
    assert_eq!(statuses.len(), 5, "{:?}", r.lines);
    assert!(statuses[0].contains("no AWS connection named \"conn-nope\""), "{}", statuses[0]);
    for s in &statuses[1..4] {
        assert!(s.contains("nothing was stored"), "{s}");
    }
    assert!(statuses[4].contains("no AWS connection was named"), "{}", statuses[4]);
    assert_eq!(r.line("FLAG conn-nope "), "N");
    assert_eq!(r.line(&format!("FLAG {name} ")), "N", "the refused sets stored nothing");
    assert!(p.found_in(&r.text()).is_empty(), "a status repeated a value: {:?}", r.lines);
    assert!(a.var("AWS_ACCESS_KEY_ID").is_empty() && a.var("AWS_PROFILE") == ["test"], "{}", a.env());
    eprintln!("\n  ── A6 refused ── {} refusals, each in words, none storing anything:", statuses.len());
    for s in statuses {
        eprintln!("    {s}");
    }
}
