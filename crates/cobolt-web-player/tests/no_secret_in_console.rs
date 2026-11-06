// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! No token, password or marked field in the browser console or in an error report (WASM-013 R13, T18; AC10, the
//! player's half — the backend's is `cobolt-server/tests/no_secret_in_logs.rs`).
//!
//! The console of the page is written from exactly three places: the `ERROR` report of a run-time error
//! (`RunError::describe`), the program's own `DISPLAY` and log lines, and the connection's warnings (`Out::Warning`).
//! This test plants a password in a **password-mode control** (the field the program marks) and a token on the
//! connection, then makes every error the player can make *about those values* — an argument that is not a parameter, a
//! window choice, a form id, a subscript, an unknown method or property, a failed call, a refused token, a reply that is
//! not JSON, a timeout, a lost connection — and searches everything the console would show, and the `Debug` of every
//! effect and every connection output, for the password and the token. It prints how many lines it searched.
//!
//! Two positive controls keep a zero from being vacuous: with the control **not** in password mode the same error does
//! quote the value, and the planted values do travel on the wire (the test's server reads them).

mod common;

use common::Rig;
use cobolt_wasm_abi::fixture::standard::*;
use cobolt_wasm_abi::fixture::{build, standard_spec, FixtureHandler, Step, Target};
use cobolt_web_player::host::{parse_params, Effect, HostOptions};
use cobolt_web_player::net::{Client, Config, Out, Socket};

/// What the user typed into the password box.
const PASSWORD: &str = "Tr0ub4dor&3-planted";
/// The session token of the connection (a JWT-shaped string, as a provider issues).
const TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJwbGFudGVkIn0.planted-signature";
/// The token the refresh brings.
const NEW_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJyZWZyZXNoZWQifQ.refreshed-signature";

// ── what the page's console shows ───────────────────────────────────────────

/// The lines `api.rs` writes to the console for the effects of the host: `DISPLAY` text, log lines, error reports.
fn console_of(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Display(t) => Some(t.clone()),
            Effect::Log { level, text } => Some(format!("[pr:{level}] {text}")),
            Effect::RuntimeError(r) => Some(r.describe()),
            _ => None,
        })
        .collect()
}

fn hits(haystack: &str, needles: &[&str]) -> usize {
    needles.iter().map(|n| haystack.matches(n).count()).sum()
}

// ── part 1: the host, with a password-mode control ──────────────────────────

const BUTTONS: [&str; 7] = ["BTN-PW-PARAM", "BTN-PW-WINDOW", "BTN-PW-FORM", "BTN-PW-INDEX", "BTN-PW-METHOD", "BTN-PW-PROPERTY", "BTN-PW-REMOTE"];

/// The standard fixture with seven more buttons, each of which hands the password to the player in a place where an error
/// message could quote it — the way `CALL COBOL::OpenFormAsync("DETAIL", TXT-IN::Text)` would.
fn spec(multi: bool) -> cobolt_wasm_abi::fixture::FixtureSpec {
    let mut s = standard_spec(multi);
    let main = &mut s.forms[0];
    for b in BUTTONS {
        main.controls.push((b.into(), "Button".into()));
    }
    let on = |b: &str, step: Step| FixtureHandler::on(b, "onClick", vec![step]);
    let open = |args: Vec<String>| Step::Call { target: Target::App, name: "OpenFormAsync".into(), args };
    main.handlers.extend([
        // a second argument that is not `name=value`
        on("BTN-PW-PARAM", open(vec![CHILD_FORM.into(), PASSWORD.into()])),
        // the reserved window choice, given the password for its value
        on("BTN-PW-WINDOW", open(vec![CHILD_FORM.into(), format!("pr-window={PASSWORD}")])),
        // the password where the form's name goes
        on("BTN-PW-FORM", open(vec![PASSWORD.into()])),
        // a subscript that is not a number
        on("BTN-PW-INDEX", Step::SetAt { target: Target::control(LST_1), name: "Items".into(), args: vec![PASSWORD.into()], value: "x".into() }),
        // a method the control does not have, called with the password
        on("BTN-PW-METHOD", Step::Call { target: Target::control(TXT_IN), name: "NoSuchMethod".into(), args: vec![PASSWORD.into()] }),
        // a property the control does not have, set to the password
        on("BTN-PW-PROPERTY", Step::Set { target: Target::control(TXT_IN), name: "NoSuchProperty".into(), value: PASSWORD.into() }),
        // a remote call that carries the password; its failure comes back as an error frame
        on("BTN-PW-REMOTE", Step::Remote(format!(r#"{{"program":"LOGIN","args":[{{"mode":"value","json":"{PASSWORD}"}}],"returning":false}}"#).into_bytes())),
    ]);
    s
}

fn rig(multi: bool) -> Rig {
    Rig::with_spec(build(&spec(multi)), multi, HostOptions::default())
}

/// Click every button once and return what each produced on the console, in order.
fn press_all(rig: &mut Rig, debug: &mut String) -> Vec<(&'static str, Vec<String>)> {
    let mut out = Vec::new();
    for b in BUTTONS {
        rig.host.borrow_mut().effects.clear();
        rig.click(0, b);
        if b == "BTN-PW-REMOTE" {
            // The reply to the call: an error, as a refused sign-in would be.
            for (id, _) in rig.take_requests() {
                let frame = format!(r#"{{"id":{id},"error":{{"class":"authentication","code":2001,"message":"the sign-in was refused"}}}}"#);
                rig.host.borrow_mut().remote_answer(id, frame.into_bytes());
            }
            rig.drive();
        }
        debug.push_str(&format!("{:?}\n", rig.host.borrow().effects));
        out.push((b, console_of(&rig.host.borrow().effects)));
    }
    out
}

#[test]
fn a_password_boxs_text_is_in_no_error_report_and_no_console_line() {
    let mut searched = 0usize;
    let mut reports = 0usize;
    let mut debug = String::new();
    let mut table = Vec::new();
    for multi in [true, false] {
        let mut rig = rig(multi);
        rig.start();
        let ctl = rig.control(0, TXT_IN);
        // The user types the password. The control is NOT yet in password mode: the positive control.
        rig.host.borrow_mut().user_change(0, ctl, "Text", PASSWORD);
        let plain = press_all(&mut rig, &mut String::new());
        let leaked: Vec<&str> = plain.iter().filter(|(_, lines)| lines.iter().any(|l| l.contains(PASSWORD))).map(|(b, _)| *b).collect();
        assert!(
            leaked.contains(&"BTN-PW-FORM"),
            "the positive control: with a plain box a form id that is the text is quoted by the report, so that the zero below means something; \
             the reports were: {plain:#?}"
        );

        // Now the box is a password box (the mark the program gave it).
        {
            let mut h = rig.host.borrow_mut();
            h.trees.get_mut(&0).unwrap().set(ctl, "PasswordCharacter", "*");
        }
        assert_eq!(rig.get(0, TXT_IN, "Text"), PASSWORD, "the planted text is the control's value");
        let marked = press_all(&mut rig, &mut debug);
        let mut produced = 0;
        for (button, lines) in &marked {
            for l in lines {
                searched += 1;
                assert_eq!(hits(l, &[PASSWORD, TOKEN]), 0, "R13: `{button}` put the password on the console: {l}");
            }
            produced += lines.len();
        }
        reports += produced;
        assert!(produced >= 5, "the buttons made errors for the test to search: {marked:#?}");
        // The form id is the one place left that quotes an argument: the report is cleaned of the box's text.
        let form = &marked.iter().find(|(b, _)| *b == "BTN-PW-FORM").unwrap().1;
        assert!(form.iter().any(|l| l.contains("<redacted>")), "the report of the form id is cleaned: {form:?}");
        if multi {
            table = marked.iter().map(|(b, lines)| format!("  {b:<16} {} report(s){}", lines.len(), lines.first().map(|l| format!(": {l}")).unwrap_or_default())).collect();
        }
        // The password is still in the box: the cleaning is of the report, not of the value.
        assert_eq!(rig.get(0, TXT_IN, "Text"), PASSWORD);
    }
    // And the diagnostics: the `Debug` of everything the host produced while the box was a password box.
    assert_eq!(hits(&debug, &[PASSWORD, TOKEN]), 0, "the effects' Debug carries neither: {debug}");
    println!("host: 2 module variants x {} buttons, {reports} error reports ({searched} console lines) and {} bytes of effect Debug searched for the password and the token: 0 hits\n{}", BUTTONS.len(), debug.len(), table.join("\n"));
}

#[test]
fn a_parameter_that_is_refused_is_named_by_its_position_never_by_its_text() {
    // The refusals of `parse_params` themselves, without the cleaning of the report to lean on.
    let long = format!("k={}", "x".repeat(600));
    // A name-shaped secret, for the cases where the text is a valid name.
    const NAME: &str = "hunter2-planted";
    let cases: [(&str, Vec<String>, &str); 5] = [
        ("not name=value", vec![PASSWORD.into()], "parameter 1 is not name=value"),
        ("a name that is not one", vec!["id=1".into(), format!("{PASSWORD} bad=2")], "parameter 2"),
        ("a value that is too long", vec![format!("{NAME}={}", "y".repeat(600))], "parameter 1"),
        ("a name given twice", vec![format!("{NAME}=1"), format!("{}=2", NAME.to_ascii_uppercase())], "twice"),
        ("a long value under a plain name", vec![long], "parameter 1"),
    ];
    for (what, args, expect) in cases {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let e = parse_params(&refs).expect_err(what);
        assert!(e.contains(expect), "{what}: {e}");
        assert_eq!(hits(&e, &[PASSWORD, NAME, &NAME.to_ascii_uppercase(), "Tr0ub4dor"]), 0, "R13: {what} quotes the argument: {e}");
    }
    println!("parse_params: 5 refusals, none quotes the argument it refused (0 hits for the password, a name-shaped secret in either case, or the password's first word)");
}

// ── part 2: the connection ──────────────────────────────────────────────────

const URL: &str = "wss://api.example.com/ws";
const SERVER_SEED: [u8; 32] = [5; 32];
const SERVER_NONCE: [u8; 16] = [6; 16];

fn entropy(buf: &mut [u8]) -> bool {
    buf.fill(0x11);
    true
}

/// A server that reads what the client seals, so that the test can see that the planted values travel.
#[derive(Default)]
struct Wire {
    /// Everything the client sent, opened.
    sent: Vec<String>,
    /// How many connections the client has tried to open.
    opened: usize,
    rx: Option<cobolt_wire::seal::Receiver>,
}

impl Socket for Wire {
    fn open(&mut self, _url: &str) {
        self.opened += 1;
    }
    fn send(&mut self, text: &str) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
            let peer = v["key"].as_str().and_then(cobolt_wire::seal::decode_b64::<32>).zip(v["nonce"].as_str().and_then(cobolt_wire::seal::decode_b64::<16>));
            if let (Some("hello"), Some((key, nonce))) = (v["t"].as_str(), peer) {
                let keys = cobolt_wire::seal::KeyPair::from_bytes(SERVER_SEED);
                let (_tx, rx) = cobolt_wire::seal::establish(cobolt_wire::seal::Role::Server, v["protocol"].as_u64().unwrap_or(0) as u32, &keys, &SERVER_NONCE, &key, &nonce).unwrap().split();
                self.rx = Some(rx);
            }
        }
        self.sent.push(text.to_owned());
    }
    fn send_binary(&mut self, bytes: &[u8]) {
        let text = self.rx.as_mut().expect("a frame before the hello").open(bytes).expect("the frame opens");
        self.sent.push(text);
    }
    fn close(&mut self) {}
}

fn hello_ok() -> String {
    let key = cobolt_wire::seal::encode_b64(&cobolt_wire::seal::KeyPair::from_bytes(SERVER_SEED).public());
    let nonce = cobolt_wire::seal::encode_b64(&SERVER_NONCE);
    format!(r#"{{"t":"hello","protocol":{},"protocol_min":{},"contract":"1.0.0","accepted":"ok","message":"","server":"test","max_frame_bytes":1048576,"key":"{key}","nonce":"{nonce}"}}"#, cobolt_web_player::net::PROTOCOL, cobolt_web_player::net::PROTOCOL)
}

/// The text the module would receive for an output, and the console line where the page would write one.
fn shown(outs: &[Out], console: &mut Vec<String>, answers: &mut Vec<String>) {
    for o in outs {
        match o {
            Out::Warning(w) => console.push(w.clone()),
            Out::Answer { frame, .. } => answers.push(String::from_utf8_lossy(frame).into_owned()),
            _ => {}
        }
    }
}

#[test]
fn a_token_and_a_password_in_a_call_reach_neither_a_console_warning_nor_an_error_answer() {
    let cfg = Config { origins: vec![URL.into()], entropy: Some(entropy), call_timeout_ms: 5_000, ..Config::default() }.with_backend("1.0.0", vec![]);
    let mut c = Client::new(cfg, Wire::default());
    let (mut console, mut answers, mut debug) = (Vec::new(), Vec::new(), String::new());
    let mut take = |c: &mut Client<Wire>, console: &mut Vec<String>, answers: &mut Vec<String>| {
        let outs = c.take_out();
        debug.push_str(&format!("{outs:?}\n"));
        shown(&outs, console, answers);
    };

    // Sign in (a token), and a call that carries the password.
    c.set_token(Some(TOKEN.into()));
    c.connect(URL, 0).unwrap();
    let frame = |program: &str| format!(r#"{{"program":"{program}","args":[{{"mode":"value","json":"{PASSWORD}"}}],"returning":false}}"#).into_bytes();
    let login = c.request(&frame("LOGIN"), 1);
    c.on_open(2);
    c.on_message(&hello_ok());
    c.on_message(r#"{"t":"auth-ok","sub":"u-1","expires_in":300}"#);
    take(&mut c, &mut console, &mut answers);

    // 1. The token has expired: the player asks for a new one and repeats the call under it.
    c.on_message(&format!(r#"{{"t":"error","id":{login},"class":"authentication","code":2002,"message":"the token has expired"}}"#));
    c.set_token(Some(NEW_TOKEN.into()));
    take(&mut c, &mut console, &mut answers);

    // 2. The call is refused, validated, failed: the backend's own words come back as the answer.
    c.on_message(&format!(r#"{{"t":"error","id":{login},"class":"authorisation","code":3001,"message":"not allowed"}}"#));
    let validate = c.request(&frame("VALIDATE"), 10);
    c.on_message(&format!(r#"{{"t":"error","id":{validate},"class":"validation","code":1001,"message":"the request is not valid","detail":{{"field":"args[0]"}}}}"#));
    take(&mut c, &mut console, &mut answers);

    // 3. A frame that is not JSON, one that echoes the planted values back, and a connection-level failure.
    c.on_message(&format!("this is not json {PASSWORD} {TOKEN}"));
    c.on_message(r#"{"t":"error","class":"transport","code":4001,"message":"the backend is going away"}"#);
    take(&mut c, &mut console, &mut answers);

    // 4. A call nobody answers.
    let silent = c.request(&frame("SILENT"), 100);
    c.tick(100 + 5_000);
    take(&mut c, &mut console, &mut answers);
    assert!(answers.iter().any(|a| a.contains(&format!("\"id\":{silent}")) && a.contains("did not answer in time")), "the timeout reached the module: {answers:?}");

    // 5. The connection is lost with a call in flight, and every attempt to get it back is refused.
    let lost = c.request(&frame("LOST"), 6_000);
    c.on_close(6_001);
    // Every attempt is refused at once (a stopped backend) until the policy gives up and the call is failed.
    let mut tried = c.socket().opened;
    while let Some(d) = c.next_deadline().filter(|d| *d <= 10 * 60 * 1000) {
        c.tick(d);
        if c.socket().opened > tried {
            tried = c.socket().opened;
            c.on_error(d);
            c.on_close(d);
        }
    }
    take(&mut c, &mut console, &mut answers);
    assert!(answers.iter().any(|a| a.contains(&format!("\"id\":{lost}"))), "the lost call was answered with its failure: {answers:?}");

    // The positive control: the values did travel, sealed, to the server.
    let wire = c.socket().sent.join("\n");
    assert!(wire.contains(PASSWORD), "the password was in the call the client sent");
    assert!(wire.contains(TOKEN) && wire.contains(NEW_TOKEN), "both tokens were in what the client sent");

    // The search.
    assert!(console.len() >= 2, "the connection wrote warnings for the test to search: {console:?}");
    assert!(answers.len() >= 4, "the module was answered with errors for the test to search: {answers:?}");
    for l in console.iter().chain(answers.iter()) {
        assert_eq!(hits(l, &[PASSWORD, TOKEN, NEW_TOKEN]), 0, "R13: the connection showed a secret: {l}");
    }
    assert_eq!(hits(&debug, &[PASSWORD, TOKEN, NEW_TOKEN]), 0, "the Debug of the connection's outputs carries no secret: {debug}");
    assert!(!format!("{:?}", c.cfg).contains(TOKEN), "the configuration never holds a token");
    println!(
        "connection: {} console warnings and {} error answers ({} bytes) and {} bytes of output Debug searched for the password and both tokens: 0 hits (the wire carried them: {} bytes)",
        console.len(),
        answers.len(),
        console.iter().chain(answers.iter()).map(String::len).sum::<usize>(),
        debug.len(),
        wire.len()
    );
}
