// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The page in a real browser** (WASM-016 T22; the operator's decision of 2026-10-23). The node
//! tests run the player headless, where no page is drawn; this one draws it. The Build's bundle around
//! the fixture program is served cross-origin isolated by the bundle's own server, a headless Chrome
//! opens it — as the whole window, and mounted in a `<div>` of another page — and the test acts as a
//! user (presses the mouse, types, resizes the window) and asserts on the DOM and on computed styles.
//!
//! The page is drawn by Leptos (WASM-016 T24; until T24 the same scenarios also ran on the hand-written
//! patch it replaced). A WebSocket backend in the test, speaking the connection of
//! `examples/mock_backend.rs`, answers the program's remote calls, is held back, is stopped and
//! started again (AC4, AC5, AC13), and refuses every connection for a whole retry cycle (AC14). One line is
//! printed per scenario and placement.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{BundleOptions, Embed, Isolation, PlayerInput};
use cobolt_wasm_abi::fixture::standard::{BACKEND_UNREACHABLE, CHILD_FORM, GREETING, LBL_OUT, LST_1, MAIN_FORM, TXT_IN};
use support::cdp::{find_chrome, Chrome, Page};

/// Helpers the scenarios evaluate in the page; the application is inside `#pr-app`'s shadow root.
const HELPERS: &str = r#"
window.__pr = {
  app() { return document.getElementById('pr-app'); },
  root() { const a = this.app(); return a && a.shadowRoot; },
  ctl(form, name) { const r = this.root(); return r && r.querySelector(`[data-pr-form="${form}"] [data-name="${name}"]`); },
  rect(el) { if (!el) return null; const b = el.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; },
  text(form, name) { const e = this.ctl(form, name); return e ? e.textContent : null; },
  input(form, name) { const e = this.ctl(form, name); return e && (e.matches('input,textarea') ? e : e.querySelector('input,textarea')); },
  handle() { return PowerRustCOBOL.mount(this.app()); },
};
true
"#;

#[derive(Clone, Copy, PartialEq)]
enum Placement {
    Window,
    Div,
}

impl Placement {
    fn name(self) -> &'static str {
        match self {
            Placement::Window => "window",
            Placement::Div => "div",
        }
    }
}

/// `index.html` mounted in a sized `<div>` of a page that has its own content, with its own stylesheet
/// (the page's policy allows `'self'` stylesheets; it allows no inline style).
fn embedding_page(index: &str) -> String {
    let at = index.find("<div id=\"pr-app\"").expect("the host page has #pr-app");
    let end = at + index[at..].find("</div>").expect("closing div") + "</div>".len();
    let head = index.replacen("</title>", "</title>\n<link rel=\"stylesheet\" href=\"host.css\">", 1);
    let shift = head.len() - index.len();
    format!("{}<p id=\"before\">The host page's own text.</p>\n<div id=\"host-box\">{}</div>\n<p id=\"after\">More of the host page.</p>{}", &head[..at + shift], &head[at + shift..end + shift], &head[end + shift..])
}

/// A backend on the loopback that speaks the backend's protocol as far as the player uses it
/// (`cobolt_wire::proto`, WASM-014 T6: `hello` answered `hello`, `{"t":"call","id","key",…}` answered
/// `{"t":"reply","id","args","returning"}`, as `examples/mock_backend.rs`): it answers every call `pong` after `delay_ms`,
/// or holds it forever (`delay_ms` = `u64::MAX`). [`Backend::kill`] drops it with its connections, as a
/// server that goes down; [`Backend::start`] on the same port brings it back. [`Backend::refuse`] keeps the port
/// but closes every connection, the open ones and each new one at once — a backend that is down for good, whose
/// listener still counts the attempts ([`Backend::attempts`]).
struct Backend {
    port: u16,
    delay_ms: Arc<AtomicU64>,
    calls: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    refusing: Arc<AtomicBool>,
    attempts: Arc<Mutex<Vec<Instant>>>,
    conns: Arc<Mutex<Vec<TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}

impl Backend {
    fn start(port: u16, delay_ms: u64, calls: Arc<Mutex<Vec<String>>>) -> Backend {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("the backend's port is free");
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (delay, stop, conns) = (Arc::new(AtomicU64::new(delay_ms)), Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(Vec::new())));
        let (refusing, attempts) = (Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(Vec::new())));
        let (d, st, cs, log, rf, at) = (delay.clone(), stop.clone(), conns.clone(), calls.clone(), refusing.clone(), attempts.clone());
        let thread = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                at.lock().unwrap().push(Instant::now());
                if rf.load(Ordering::SeqCst) {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                stream.set_nonblocking(false).unwrap();
                cs.lock().unwrap().push(stream.try_clone().unwrap());
                let (d, log) = (d.clone(), log.clone());
                std::thread::spawn(move || {
                    let Ok(mut ws) = tungstenite::accept(stream) else { return };
                    while let Ok(msg) = ws.read() {
                        let tungstenite::Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<serde_json::Value>(text.as_str()) else { continue };
                        if v["t"] == "hello" {
                            let hello = serde_json::json!({"t": "hello", "protocol": 1, "protocol_min": 1, "contract": v["contract"], "accepted": "ok", "message": "", "server": "e2e", "max_frame_bytes": 1 << 20});
                            if ws.send(tungstenite::Message::text(hello.to_string())).is_err() {
                                break;
                            }
                            continue;
                        }
                        if v["t"] != "call" {
                            continue;
                        }
                        log.lock().unwrap().push(text.to_string());
                        let wait = d.load(Ordering::SeqCst);
                        if wait == u64::MAX {
                            continue;
                        }
                        std::thread::sleep(Duration::from_millis(wait));
                        let id = v["id"].as_u64().unwrap_or(0);
                        if ws.send(tungstenite::Message::text(serde_json::json!({"t": "reply", "id": id, "args": [], "returning": "pong"}).to_string())).is_err() {
                            break;
                        }
                    }
                });
            }
        });
        Backend { port, delay_ms: delay, calls, stop, refusing, attempts, conns, thread: Some(thread) }
    }

    /// Close every connection and refuse the next ones (`true`), or take them again (`false`). Answers when it began.
    fn refuse(&self, on: bool) -> Instant {
        self.refusing.store(on, Ordering::SeqCst);
        if on {
            self.attempts.lock().unwrap().clear();
            for c in self.conns.lock().unwrap().drain(..) {
                let _ = c.shutdown(std::net::Shutdown::Both);
            }
        }
        Instant::now()
    }

    /// When each connection reached the listener since [`Backend::refuse`], in seconds after `t0`.
    fn attempts(&self, t0: Instant) -> Vec<f64> {
        self.attempts.lock().unwrap().iter().map(|t| t.saturating_duration_since(t0).as_secs_f64()).collect()
    }

    fn url(&self) -> String {
        format!("ws://127.0.0.1:{}/ws", self.port)
    }

    fn kill(mut self) -> (u16, Arc<Mutex<Vec<String>>>) {
        self.stop.store(true, Ordering::SeqCst);
        for c in self.conns.lock().unwrap().drain(..) {
            let _ = c.shutdown(std::net::Shutdown::Both);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        (self.port, self.calls.clone())
    }
}

/// What the Preview would serve after an edit (WASM-016 R16), beside the bundle under `preview/`: a stylesheet, the main
/// form's description with BTN-GO moved to x 300, and the program rebuilt with an `onLoad` that writes "loaded v2".
fn write_preview_edits(dir: &Path) {
    use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec, Step};
    let out = dir.join("preview");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("stylesheet.v2.css"), "[data-name=\"LBL-OUT\"] { letter-spacing: 3px; }\n").unwrap();
    let fx = build(&standard_spec(true));
    let cfrm = standard_cfrm(&fx, MAIN_FORM).replacen("id=\"BTN-GO\" type=\"Button\" x=\"10\"", "id=\"BTN-GO\" type=\"Button\" x=\"300\"", 1);
    assert!(cfrm.contains("x=\"300\""), "the edit applies: {cfrm}");
    let (_, description) = cobolt_web_bundle::form_description(&cfrm).unwrap();
    std::fs::write(out.join("main-form.v2.json"), cobolt_forms::description::to_json(&description)).unwrap();
    for (multi, name) in [(true, "program.v2.wasm"), (false, "program.single.v2.wasm")] {
        let mut spec = standard_spec(multi);
        let on_load = spec.forms[0].handlers.iter_mut().find(|h| h.control.is_none() && h.event == "onLoad").expect("the main form's onLoad");
        on_load.steps = vec![Step::Set { target: cobolt_wasm_abi::fixture::Target::control(LBL_OUT), name: "Caption".into(), value: "loaded v2".into() }];
        let v2 = build(&spec);
        std::fs::write(out.join(name), &v2.wasm).unwrap();
        if multi {
            std::fs::write(out.join("manifest.v2.json"), serde_json::to_string_pretty(&v2.manifest).unwrap()).unwrap();
        }
    }
}

const HOST_CSS: &str = "#host-box { width: 700px; height: 600px; margin: 30px; border: 1px solid #888; }\n";

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

struct Run<'a> {
    page: &'a mut Page,
    placement: Placement,
    backend: &'a mut Option<Backend>,
    dir: &'a Path,
    results: Vec<(String, Result<String, String>)>,
}

impl Run<'_> {
    fn scenario(&mut self, name: &str, f: impl FnOnce(&mut Page, Placement) -> Result<String, String>) {
        let r = f(self.page, self.placement);
        self.results.push((name.to_owned(), r));
    }

    fn with_backend(&mut self, name: &str, f: impl FnOnce(&mut Page, &mut Option<Backend>) -> Result<String, String>) {
        let r = f(self.page, self.backend);
        self.results.push((name.to_owned(), r));
    }
}

fn text_of(p: &mut Page, form: &str, name: &str) -> Result<String, String> {
    Ok(p.eval(&format!("__pr.text({}, {})", q(form), q(name)))?.as_str().unwrap_or("").to_owned())
}

fn wait_text(p: &mut Page, form: &str, name: &str, want: &str) -> Result<(), String> {
    p.wait_for(&format!("__pr.text({}, {}) === {}", q(form), q(name), q(want)), 20)
        .map(|_| ())
        .map_err(|e| format!("{e}; {name} reads {:?}", text_of(p, form, name).unwrap_or_default()))
}

/// A user's click on a control: the pointer goes to its centre, which must hit the control itself (not
/// something drawn over it, not a page that clips it).
fn click(p: &mut Page, form: &str, name: &str) -> Result<(), String> {
    let hit = p.eval(&format!(
        "(() => {{ const e = __pr.ctl({f}, {n}); if (!e) return 'no element'; const b = e.getBoundingClientRect(); const t = __pr.root().elementFromPoint(b.x + b.width / 2, b.y + b.height / 2); const r = (x) => x ? [x.className || x.tagName, ...['x', 'y', 'width', 'height'].map(k => Math.round(x.getBoundingClientRect()[k]))].join(' ') : '-'; return t && (t === e || e.contains(t)) ? 'ok' : 'covered by ' + (t ? t.outerHTML.slice(0, 100) : 'nothing (outside the viewport)') + ' | control ' + r(e) + ' | frame ' + r(e.closest('.pr-panelwin,dialog')) + ' | body ' + r(e.closest('.pr-body')) + ' | layer ' + r(__pr.root().querySelector('.pr-layer')); }})()",
        f = q(form),
        n = q(name)
    ))?;
    if hit != serde_json::json!("ok") {
        return Err(format!("{name} of {form} cannot be clicked: {}", hit.as_str().unwrap_or("?")));
    }
    p.click_at(&format!("__pr.rect(__pr.ctl({}, {}))", q(form), q(name)))
}

/// The "connecting" modal while it is up (WASM-016 R20, AC13): `None` when it is not.
const MODAL: &str = "__pr.root().querySelector('dialog.pr-netmodal[open]')";

/// The captions of the modal in the page's language — the headless browser's, English (WASM-016 R20–R21).
const CONNECTING: &str = "Connecting to the server\u{2026}";
const UNREACHABLE: &str = "The server could not be reached.";

/// Within `tolerance` on every channel.
fn near_colour(c: [u8; 3], want: [u8; 3], tolerance: u8) -> bool {
    c.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= tolerance)
}

/// AC13 while the modal is up: it is over the whole application and holds the focus; a click on the text box behind it,
/// keys and typed text change nothing; it animates; its background samples red, dark red, crimson and black from left to
/// right; what is drawn on it is white. Answers a line of what was measured.
fn check_modal_blocks(p: &mut Page, main: &str) -> Result<String, String> {
    let geometry = p.eval(&format!(
        "(() => {{ const d = {MODAL}, a = __pr.app().getBoundingClientRect(), r = d.getBoundingClientRect(), i = __pr.input({m}, {t}).getBoundingClientRect(); \
         const hit = __pr.root().elementFromPoint(i.x + i.width / 2, i.y + i.height / 2); \
         return {{ app: [a.x, a.y, a.width, a.height].map(Math.round), modal: [r.x, r.y, r.width, r.height].map(Math.round), modal_on_top: !!hit && !!hit.closest('dialog.pr-netmodal'), is_modal: d.matches(':modal') }}; }})()",
        m = q(main),
        t = q(TXT_IN)
    ))?;
    if geometry["app"] != geometry["modal"] || geometry["modal_on_top"] != true || geometry["is_modal"] != true {
        return Err(format!("the modal does not cover the application: {geometry}"));
    }
    // What a user does to the form behind it.
    p.click_at(&format!("__pr.rect(__pr.input({}, {}))", q(main), q(TXT_IN)))?;
    for k in ["x", "Tab", "Escape", "Enter"] {
        p.press_key(k)?;
    }
    p.type_text("zz")?;
    let after = p.eval(&format!(
        "(() => {{ const i = __pr.input({m}, {t}), a = __pr.root().activeElement; return {{ value: i.value, open: !!{MODAL}, focus_in_modal: !!a && !!a.closest('dialog.pr-netmodal') }}; }})()",
        m = q(main),
        t = q(TXT_IN)
    ))?;
    if after["value"] != "written first" || after["open"] != true || after["focus_in_modal"] != true {
        return Err(format!("the form behind the modal was reached: {after}"));
    }
    let look = p.eval(&format!(
        "(() => {{ const d = {MODAL}; const c = (s) => getComputedStyle(d.querySelector(s)); return {{ caption: d.querySelector('.pr-netmodal-caption').textContent, \
         running: d.getAnimations({{ subtree: true }}).filter((a) => a.playState === 'running').length, \
         stroke: c('.pr-netmodal-screen rect').stroke, server: c('.pr-netmodal-server rect').stroke, cloud: c('.pr-netmodal-cloud').stroke, text: c('.pr-netmodal-caption').color }}; }})()"
    ))?;
    let white = "rgb(255, 255, 255)";
    if look["caption"] != CONNECTING || look["running"].as_u64().unwrap_or(0) < 2 || [&look["stroke"], &look["server"], &look["cloud"], &look["text"]].iter().any(|v| **v != white) {
        return Err(format!("the modal looks wrong: {look}"));
    }
    // The background, sampled on a row near the top edge, where nothing is drawn.
    let r = p.eval(&format!("__pr.rect({MODAL})"))?;
    let (x, y, w) = (r["x"].as_f64().unwrap_or(0.0), r["y"].as_f64().unwrap_or(0.0), r["width"].as_f64().unwrap_or(0.0));
    let (cw, _, px) = p.capture(x, y, w, 8.0)?;
    let at = |f: f64| px[4 * cw as usize + ((cw as f64 - 1.0) * f).round() as usize];
    let want = [(0.01, [252, 0, 0], "red"), (0.40, [139, 0, 0], "dark red"), (0.70, [220, 20, 60], "crimson"), (0.99, [7, 1, 2], "black")];
    let mut samples = Vec::new();
    for (f, colour, name) in want {
        let c = at(f);
        if !near_colour(c, colour, 14) {
            return Err(format!("at {:.0} % of the width the background is {c:?}, not {name} {colour:?}", f * 100.0));
        }
        samples.push(format!("{name} {c:?}"));
    }
    let d = p.eval(&format!("__pr.rect({MODAL}.querySelector('.pr-netmodal-drawing'))"))?;
    let (_, _, drawing) = p.capture(d["x"].as_f64().unwrap_or(0.0), d["y"].as_f64().unwrap_or(0.0), d["width"].as_f64().unwrap_or(1.0), d["height"].as_f64().unwrap_or(1.0))?;
    let whites = drawing.iter().filter(|c| c.iter().all(|v| *v >= 235)).count();
    if whites < 100 {
        return Err(format!("only {whites} white pixels in the drawing"));
    }
    Ok(format!(
        "over the application {:?}; click, x, Tab, Escape, Enter and \"zz\" left TXT-IN at \"written first\", the focus in the modal; {} animations running; samples {}; {whites} white pixels drawn",
        geometry["modal"].to_string(),
        look["running"],
        samples.join(", ")
    ))
}

fn run_scenarios(page: &mut Page, url: &str, placement: Placement, backend: &mut Option<Backend>, dir: &Path) -> Vec<(String, Result<String, String>)> {
    let mut run = Run { page, placement, backend, dir, results: Vec::new() };
    let main = MAIN_FORM;
    let child = CHILD_FORM;

    run.scenario("load: the main form is drawn and its onLoad wrote the label", |p, _| {
        p.resize(1024, 768)?;
        p.navigate(url)?;
        p.eval(HELPERS)?;
        p.wait_for(&format!("__pr.text({}, {}) === 'loaded'", q(main), q(LBL_OUT)), 30)?;
        let choice = p.eval("__pr.handle().then(a => a.choice.player + '/' + a.choice.mode + '/' + a.choice.program)")?;
        let iso = p.eval("self.crossOriginIsolated")?;
        if iso != serde_json::json!(true) {
            return Err("the page is not cross-origin isolated".into());
        }
        Ok(format!("isolated, {}", choice.as_str().unwrap_or("?")))
    });

    run.scenario("geometry: a control sits where the form puts it", |p, _| {
        let r = p.eval(&format!(
            "(() => {{ const b = __pr.ctl({m}, 'BTN-GO'); const s = b.closest('.pr-surface'); const x = b.getBoundingClientRect(), y = s.getBoundingClientRect(); const cs = getComputedStyle(b); return {{ dx: x.x - y.x, dy: x.y - y.y, w: x.width, h: x.height, display: cs.display, visibility: cs.visibility, position: cs.position }}; }})()",
            m = q(main)
        ))?;
        let near = |k: &str, v: f64| (r[k].as_f64().unwrap_or(-1.0) - v).abs() <= 1.0;
        if !(near("dx", 10.0) && near("dy", 112.0) && near("w", 200.0) && near("h", 28.0)) {
            return Err(format!("BTN-GO is at {r}, the form says x 10 y 112 w 200 h 28"));
        }
        if r["display"] == "none" || r["visibility"] != "visible" {
            return Err(format!("BTN-GO is not shown: {r}"));
        }
        Ok(format!("BTN-GO at (10, 112) 200x28, position {}", r["position"].as_str().unwrap_or("?")))
    });

    run.scenario("click: the handler runs, SET reaches the DOM, CALL reaches the list, DISPLAY the console", |p, _| {
        p.eval(&format!("__pr.input({}, {}).__kept = 1", q(main), q(TXT_IN)))?;
        click(p, main, "BTN-GO")?;
        wait_text(p, main, LBL_OUT, GREETING)?;
        p.wait_for(&format!("__pr.ctl({}, {}).querySelectorAll('li').length === 1", q(main), q(LST_1)), 10)?;
        click(p, main, "BTN-GO")?;
        p.wait_for(&format!("__pr.ctl({}, {}).querySelectorAll('li').length === 2", q(main), q(LST_1)), 10)?;
        let items = p.eval(&format!("[...__pr.ctl({}, {}).querySelectorAll('li')].map(l => l.textContent).join(',')", q(main), q(LST_1)))?;
        if !p.console.iter().any(|l| l.ends_with(": go")) {
            return Err(format!("DISPLAY did not reach the console: {:?}", p.console));
        }
        Ok(format!("label {GREETING:?}, list [{}], console has `go`", items.as_str().unwrap_or("")))
    });

    run.scenario("type: what the user types is what the handler reads, in the same element", |p, _| {
        click(p, main, TXT_IN)?;
        let typed = "digitado ação 日本";
        p.type_text(typed)?;
        let v = p.eval(&format!("__pr.input({}, {}).value", q(main), q(TXT_IN)))?;
        if v != serde_json::json!(typed) {
            return Err(format!("the box holds {v}"));
        }
        click(p, main, "BTN-QUERY")?;
        wait_text(p, main, LBL_OUT, typed)?;
        let kept = p.eval(&format!("__pr.input({}, {}).__kept === 1", q(main), q(TXT_IN)))?;
        if kept != serde_json::json!(true) {
            return Err("the text box's element was replaced by a redraw".into());
        }
        Ok(format!("typed and read back {typed:?}; the box's element survived three redraws"))
    });

    run.scenario("program writes a box: the DOM's value follows", |p, _| {
        click(p, main, "BTN-RAW")?;
        wait_text(p, main, LBL_OUT, "written first")?;
        p.wait_for(&format!("__pr.input({}, {}).value === 'written first'", q(main), q(TXT_IN)), 10)?;
        Ok("TXT-IN value = \"written first\"".into())
    });

    run.scenario("modal: a dialog opens over the form, its handler closes it, the caller resumes", |p, _| {
        click(p, main, "BTN-MODAL")?;
        p.wait_for(&format!("(() => {{ const d = __pr.root().querySelector('dialog[open]'); return !!d && !!d.querySelector('[data-name=\"LBL-CHILD\"]') && __pr.text({}, 'LBL-CHILD') === 'child'; }})()", q(child)), 20)?;
        let modal = p.eval("__pr.root().querySelector('dialog[open]').matches(':modal')")?;
        click(p, child, "BTN-CLOSE")?;
        wait_text(p, main, LBL_OUT, "after modal")?;
        p.wait_for(&format!("!__pr.root().querySelector('[data-pr-form={}]') && !__pr.root().querySelector('dialog[open]')", q(child)), 10)?;
        Ok(format!("dialog :modal = {modal}; closed, caption \"after modal\""))
    });

    run.scenario("resize: the application follows the window, or keeps its element's size", |p, placement| {
        let mut seen = Vec::new();
        for (w, h) in [(900u32, 700u32), (1280, 800)] {
            p.resize(w, h)?;
            p.wait_for(&format!("innerWidth === {w}"), 10)?;
            let r = p.eval("(() => { const a = __pr.app(); const b = a.getBoundingClientRect(); const s = getComputedStyle(a); return { w: b.width, h: b.height, position: s.position, iw: innerWidth, ih: innerHeight }; })()")?;
            let aw = r["w"].as_f64().unwrap_or(0.0);
            match placement {
                Placement::Window => {
                    if (aw - w as f64).abs() > 1.0 || r["position"] != "fixed" {
                        return Err(format!("at {w}x{h} the application is {r}"));
                    }
                }
                Placement::Div => {
                    let bw = p.eval("document.getElementById('host-box').getBoundingClientRect().width")?.as_f64().unwrap_or(0.0);
                    if (aw - 700.0).abs() > 1.0 || (bw - 702.0).abs() > 1.0 || r["position"] != "relative" {
                        return Err(format!("at {w}x{h} the application is {r}, its box {bw}"));
                    }
                }
            }
            seen.push(format!("{w}x{h} -> {aw}px {}", r["position"].as_str().unwrap_or("?")));
        }
        Ok(seen.join("; "))
    });

    run.scenario("disposal: 100 panels opened and closed by clicks leave nothing behind", |p, _| {
        let before = p.eval("__pr.handle().then(async a => ({ inst: JSON.parse(a.player.stats()).instances, live: await a.link.debugLive() }))")?;
        for i in 0..100 {
            click(p, main, "BTN-OPEN")?;
            p.wait_for(&format!("__pr.text({}, 'LBL-CHILD') === 'child'", q(child)), 20).map_err(|e| format!("cycle {i}: {e}"))?;
            if i == 0 {
                // The panel's body is the form's designed size (640 x 480), not collapsed to its title bar.
                let body = p.eval(&format!("__pr.ctl({}, 'BTN-CLOSE').closest('.pr-body').getBoundingClientRect().height", q(child)))?;
                if (body.as_f64().unwrap_or(0.0) - 480.0).abs() > 1.0 {
                    return Err(format!("the panel's body is {body} px high, the form is 480"));
                }
            }
            click(p, child, "BTN-CLOSE")?;
            p.wait_for(&format!("!__pr.root().querySelector('[data-pr-form={}]')", q(child)), 20).map_err(|e| format!("cycle {i}: {e}"))?;
        }
        let after = p.eval("__pr.handle().then(async a => ({ inst: JSON.parse(a.player.stats()).instances, live: await a.link.debugLive() }))")?;
        if before != after {
            return Err(format!("before {before}, after {after}"));
        }
        let frames = p.eval("__pr.root().querySelectorAll('.pr-layer > *').length")?;
        if frames != serde_json::json!(0) {
            return Err(format!("{frames} panel frames are left in the page"));
        }
        Ok(format!("100 cycles; host instances {} and module instances {} before and after", after["inst"], after["live"]))
    });

    run.with_backend("remote call: the handler waits for the backend, two clicks meanwhile run after it, in order (AC4)", |p, b| {
        let b = b.as_ref().ok_or("no backend")?;
        b.delay_ms.store(1500, Ordering::SeqCst);
        let before = b.calls.lock().unwrap().len();
        p.eval(&format!(
            "(() => {{ window.__log = []; const l = __pr.ctl({m}, {o}); new MutationObserver(() => __log.push(l.textContent)).observe(l, {{ childList: true, characterData: true, subtree: true }}); return true; }})()",
            m = q(main),
            o = q(LBL_OUT)
        ))?;
        click(p, main, "BTN-REMOTE")?;
        wait_text(p, main, LBL_OUT, "calling")?;
        click(p, main, "BTN-GO")?;
        click(p, main, "BTN-RAW")?;
        // Still waiting: the clicks are queued behind the suspended handler.
        let held = text_of(p, main, LBL_OUT)?;
        if held != "calling" {
            return Err(format!("the label reads {held:?} while the call is in flight"));
        }
        wait_text(p, main, LBL_OUT, "written first")?;
        let log = p.eval("__log.filter((t, i, a) => i === 0 || a[i - 1] !== t)")?;
        let seq: Vec<String> = log.as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        let at = |pred: &dyn Fn(&str) -> bool| seq.iter().position(|t| pred(t));
        let (c, pong, go, raw) = (at(&|t| t == "calling"), at(&|t| t.contains("pong")), at(&|t| t == GREETING), at(&|t| t == "written first"));
        match (c, pong, go, raw) {
            (Some(c), Some(pg), Some(g), Some(r)) if c < pg && pg < g && g < r => {}
            _ => return Err(format!("the label went {seq:?}")),
        }
        let calls = b.calls.lock().unwrap()[before..].to_vec();
        if calls.len() != 1 || !calls[0].contains("PING") {
            return Err(format!("the backend saw {calls:?}"));
        }
        Ok(format!("label went {}; the backend saw one call", seq.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(" -> ")))
    });

    run.with_backend(
        "backend lost: the modal blocks the application while the page retries by the policy, the call in flight ends in ON EXCEPTION when it reconnects, the modal goes and the form is as it was, a later call succeeds (AC5, AC13, WASM-003 R25)",
        |p, slot| {
            let b = slot.take().ok_or("no backend")?;
            b.delay_ms.store(u64::MAX, Ordering::SeqCst);
            click(p, main, "BTN-REMOTE")?;
            wait_text(p, main, LBL_OUT, "calling")?;
            p.wait_for("__pr.root().querySelector('.pr-net') === null", 10)?;
            // The text box has the focus and a selection; every frame records whether the player is blocked and whether
            // the modal is up.
            p.eval(&format!(
                "(async () => {{ const i = __pr.input({m}, {t}); i.focus(); i.setSelectionRange(2, 5); i.__kept2 = 1; \
                 const a = await __pr.handle(); window.__frames = []; window.__framesStop = false; \
                 const tick = () => {{ if (window.__framesStop) return; __frames.push([JSON.parse(a.player.stats()).blocked, !!{MODAL}]); requestAnimationFrame(tick); }}; tick(); return true; }})()",
                m = q(main),
                t = q(TXT_IN)
            ))?;
            let (port, calls) = b.kill();
            p.wait_for(&format!("!!{MODAL}"), 10)?;
            let blocks = check_modal_blocks(p, main);
            let badge = p.eval("(__pr.root().querySelector('.pr-net') || {}).textContent || ''")?;
            let blocked = p.eval("__pr.handle().then(a => JSON.parse(a.player.stats()).blocked)")?;
            *slot = Some(Backend::start(port, 0, calls));
            let blocks = blocks?;
            // Attempt 1 was refused at the break; attempt 2 opens 10 s after it and reconnects. The lost call
            // (not declared idempotent) then reaches its ON EXCEPTION with the error frame (WASM-003 R25).
            p.wait_for(&format!("(__pr.text({}, {}) || '').includes('\"error\"')", q(main), q(LBL_OUT)), 30)?;
            let failed = text_of(p, main, LBL_OUT)?;
            p.wait_for("!__pr.root().querySelector('dialog.pr-netmodal') && __pr.root().querySelector('.pr-net') === null", 30)?;
            let frames = p.eval("(() => { window.__framesStop = true; const f = window.__frames; return { frames: f.length, blocked: f.filter((x) => x[0]).length, unshown: f.filter((x) => x[0] && !x[1]).length }; })()")?;
            if frames["blocked"].as_u64().unwrap_or(0) == 0 || frames["unshown"] != 0 {
                return Err(format!("a frame was drawn blocked without the modal: {frames}"));
            }
            let kept = p.eval(&format!(
                "(() => {{ const i = __pr.input({m}, {t}); return {{ value: i.value, same: i.__kept2 === 1, focused: __pr.root().activeElement === i, selection: [i.selectionStart, i.selectionEnd] }}; }})()",
                m = q(main),
                t = q(TXT_IN)
            ))?;
            if kept != serde_json::json!({"value": "written first", "same": true, "focused": true, "selection": [2, 5]}) {
                return Err(format!("the form behind the modal changed: {kept}"));
            }
            if blocked != serde_json::json!(true) {
                return Err(format!("the player did not report blocked while it retried: {blocked}"));
            }
            // What was done while the modal was up never reached the form: no click is queued behind the lost call.
            std::thread::sleep(Duration::from_millis(700));
            if text_of(p, main, LBL_OUT)? != failed {
                return Err(format!("the label moved on after the reconnection: {:?}", text_of(p, main, LBL_OUT)?));
            }
            click(p, main, "BTN-REMOTE")?;
            p.wait_for(&format!("(__pr.text({}, {}) || '').includes('pong')", q(main), q(LBL_OUT)), 20)?;
            let after = text_of(p, main, LBL_OUT)?;
            if after.contains("error") {
                return Err(format!("the call after reconnection failed too: {after}"));
            }
            Ok(format!(
                "{blocks}; {} frames blocked, each with the modal up; gone at the reconnection with TXT-IN as it was (focus, selection 2..5); badge {badge}, blocked {blocked}; in flight, at the reconnection: {}; after it: {after:?}",
                frames["blocked"],
                failed.chars().take(90).collect::<String>()
            ))
        },
    );

    run.with_backend("backend down for good: three attempts at 10, 20 and 30 s, then the modal says so, the main form's onBackendUnreachable runs, and Reload starts again (AC14, WASM-016 R21)", |p, slot| {
        let b = slot.as_ref().ok_or("no backend")?;
        p.wait_for("!__pr.root().querySelector('dialog.pr-netmodal')", 10)?;
        p.eval("window.__beforeReload = 1")?;
        let t0 = b.refuse(true);
        let first = p.wait_for(&format!("(() => {{ const d = {MODAL}; return d && d.querySelector('.pr-netmodal-caption').textContent; }})()"), 10)?;
        p.wait_for("(__pr.root().querySelector('dialog.pr-netmodal') || { getAttribute: () => '' }).getAttribute('data-state') === 'unreachable'", 90)?;
        let gave_up = t0.elapsed().as_secs_f64();
        wait_text(p, main, LBL_OUT, BACKEND_UNREACHABLE)?;
        let shown = p.eval(&format!(
            "(() => {{ const d = {MODAL}; return {{ caption: d.querySelector('.pr-netmodal-caption').textContent, buttons: [...d.querySelectorAll('button')].map((b) => b.textContent), running: d.getAnimations({{ subtree: true }}).filter((a) => a.playState === 'running').length }}; }})()"
        ))?;
        std::thread::sleep(Duration::from_secs(2));
        let attempts = b.attempts(t0);
        if first != CONNECTING || shown["caption"] != UNREACHABLE || shown["buttons"] != serde_json::json!(["Reload"]) || shown["running"] != 0 {
            return Err(format!("the modal said {first} then {shown}"));
        }
        let windows_ok = attempts.len() == 3 && attempts.iter().zip([0.0, 10.0, 30.0]).all(|(a, w)| (a - w).abs() <= 1.5);
        if !windows_ok || (gave_up - 60.0).abs() > 2.5 {
            return Err(format!("attempts at {attempts:.2?} s, given up at {gave_up:.2} s; the policy is 0, 10, 30 and 60"));
        }
        // The host page's recovery: Reload, with the backend back.
        b.refuse(false);
        p.click_at(&format!("__pr.rect({MODAL}.querySelector('button'))"))?;
        p.wait_for("window.__beforeReload === undefined && document.readyState === 'complete'", 20)?;
        p.eval(HELPERS)?;
        wait_text(p, main, LBL_OUT, "loaded")?;
        Ok(format!(
            "captions {:?} then {:?} with [Reload], the drawing still; attempts at {} s, given up at {gave_up:.2} s; LBL-OUT {BACKEND_UNREACHABLE:?} from onBackendUnreachable; Reload started the application again",
            first.as_str().unwrap_or(""),
            shown["caption"].as_str().unwrap_or(""),
            attempts.iter().map(|a| format!("{a:.2}")).collect::<Vec<_>>().join(" / ")
        ))
    });

    {
        let dir = run.dir.to_path_buf();
        run.scenario("preview: a new stylesheet, description and module are each taken in place, the page and the player stay (R16, AC11)", |p, _| {
            write_preview_edits(&dir);
            p.eval("(async () => { window.__noReload = 1; const a = await __pr.handle(); a.player.__same = 1; return true; })()")?;
            let step = |p: &mut Page, edit: &str| p.eval(&format!("(async () => {{ const a = await __pr.handle(); const next = JSON.parse(JSON.stringify(a.config)); {edit}; return a.update(next); }})()"));
            // The stylesheet: a computed style changes.
            let r1 = step(p, "next.stylesheet = { url: 'preview/stylesheet.v2.css', integrity: '' }")?;
            p.wait_for(&format!("getComputedStyle(__pr.ctl({}, {})).letterSpacing === '3px'", q(main), q(LBL_OUT)), 10)?;
            // A description: the control moves; the application starts again in the same player.
            let r2 = step(p, &format!("next.forms.find(f => f.name === {}).url = 'preview/main-form.v2.json'; next.forms.find(f => f.name === {}).integrity = ''", q(main), q(main)))?;
            p.wait_for(&format!("(() => {{ const b = __pr.ctl({m}, 'BTN-GO'); if (!b) return false; const s = b.closest('.pr-surface').getBoundingClientRect(); return Math.round(b.getBoundingClientRect().x - s.x) === 300 && __pr.text({m}, {o}) === 'loaded'; }})()", m = q(main), o = q(LBL_OUT)), 20)?;
            // The module: the new program's onLoad runs, and its handlers answer clicks.
            let r3 = step(p, "next.program.multi = { url: 'preview/program.v2.wasm', integrity: '' }; next.program.single = { url: 'preview/program.single.v2.wasm', integrity: '' }; next.manifest = { url: 'preview/manifest.v2.json', integrity: '' }")?;
            wait_text(p, main, LBL_OUT, "loaded v2")?;
            click(p, main, "BTN-GO")?;
            wait_text(p, main, LBL_OUT, GREETING)?;
            let kept = p.eval("(async () => { const a = await __pr.handle(); return { page: window.__noReload === 1, player: a.player.__same === 1, navigations: performance.getEntriesByType('navigation').length, forms: __pr.root().querySelectorAll('[data-pr-form]').length, instances: JSON.parse(a.player.stats()).instances }; })()")?;
            if kept["page"] != true || kept["player"] != true || kept["navigations"] != 1 || kept["forms"] != 1 || kept["instances"] != 1 {
                return Err(format!("not in place: {kept}"));
            }
            Ok(format!("update answered {r1}, {r2}, {r3}; letter-spacing 3px, BTN-GO at x 300, \"loaded v2\"; same page and player, one form on the page"))
        });
    }

    run.scenario("unmount: the application leaves the page", |p, _| {
        let summary = p.eval("PowerRustCOBOL.unmount(__pr.app()).then(() => __pr.root().childNodes.length)")?;
        if summary != serde_json::json!(0) {
            return Err(format!("{summary} nodes are left in the shadow root"));
        }
        Ok("shadow root empty".into())
    });

    run.scenario("no uncaught exception and no console error", |p, _| {
        let errors: Vec<&String> = p.console.iter().filter(|l| l.starts_with("error:")).collect();
        if !p.exceptions.is_empty() || !errors.is_empty() {
            return Err(format!("exceptions {:?}, console errors {errors:?}", p.exceptions));
        }
        Ok(format!("{} console lines, none an error", p.console.len()))
    });

    // Last, after the console check: a panic logs its message and ends the player (WASM-017 R15, AC11).
    run.scenario("panic: a forced panic (test bundle) shows the readable error with the reload action and raises the error event", |p, _| {
        p.eval(
            "(async () => { window.__prErr = null; __pr.app().addEventListener('pr-error', (e) => { window.__prErr = e.detail; }); \
             const a = await __pr.handle(); const m = await import(new URL(a.config.player[a.choice.player].js, location.href).href); \
             try { m.force_panic_for_test(); } catch (_) {} return true; })()",
        )?;
        p.wait_for("!!(__pr.root() && __pr.root().querySelector('.pr-message'))", 20)?;
        let r = p.eval(
            "(() => { const m = __pr.root().querySelector('.pr-message'); return { title: m.querySelector('.pr-message-title').textContent, \
             actions: [...m.querySelectorAll('button')].map((b) => b.textContent), err: window.__prErr && window.__prErr.code }; })()",
        )?;
        let actions: Vec<&str> = r["actions"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
        if !actions.contains(&"Reload") || r["err"] != "panic" || !r["title"].as_str().unwrap_or("").contains("internal error") {
            return Err(format!("after the panic the page shows {r}"));
        }
        Ok(format!("{:?} with actions {actions:?}; pr-error code panic", r["title"].as_str().unwrap_or("")))
    });

    run.results
}

#[test]
fn the_page_drawn_by_leptos_answers_the_user_and_the_backend_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut lines = Vec::new();
    let mut failed = Vec::new();
    let mut passed = 0;
    let player_out = support::built_player();
    for placement in [Placement::Window, Placement::Div] {
        let mut backend = Some(Backend::start(0, 0, Arc::new(Mutex::new(Vec::new()))));
        let backend_url = backend.as_ref().unwrap().url();
        let embed = match placement {
            Placement::Window => Embed::Window,
            Placement::Div => Embed::Element { selector: "#pr-app".into() },
        };
        let out = support::assemble(
            PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
            BundleOptions { isolation: Isolation::RequireCorp, embed, backend_origins: vec![backend_url], ..Default::default() },
        );
        let dir = tempfile::tempdir().unwrap();
        support::write_bundle(&out, dir.path());
        let entry = match placement {
            Placement::Window => "index.html",
            Placement::Div => {
                let index = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
                std::fs::write(dir.path().join("embed.html"), embedding_page(&index)).unwrap();
                std::fs::write(dir.path().join("host.css"), HOST_CSS).unwrap();
                "embed.html"
            }
        };
        let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
        let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
        let url = format!("{}/{entry}", server.url());
        for (name, r) in run_scenarios(&mut page, &url, placement, &mut backend, dir.path()) {
            let tag = format!("{:<6} {name}", placement.name());
            match r {
                Ok(detail) => {
                    passed += 1;
                    lines.push(format!("  pass  {tag}: {detail}"));
                }
                Err(e) => {
                    lines.push(format!("  FAIL  {tag}: {e}"));
                    failed.push(format!("{tag}: {e}"));
                }
            }
        }
        drop(page);
        server.stop();
        if let Some(b) = backend {
            b.kill();
        }
    }
    println!("Chrome ({}): {passed} passed, {} failed", exe.display(), failed.len());
    for l in &lines {
        println!("{l}");
    }
    assert!(failed.is_empty(), "failed in Chrome:\n{}", failed.join("\n"));
}
