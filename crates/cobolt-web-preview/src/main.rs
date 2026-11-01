// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The headless helper of the web Preview** (WASM-008 R9, R16, R2, R8).
//!
//! The interactive Preview is the developer's browser, or a device (WASM-008
//! §0, the operator's ruling of 2026-10-23); this program is what draws a web
//! form for `render_form` / `run_form` and the tests, with **no window anyone
//! sees**: its window is built invisible and placed off every screen, and on
//! macOS the program takes no place in the Dock.
//!
//! The IDE starts this program, writes JSON lines to its standard input and
//! reads JSON lines from its standard output (`cobolt_project_tools::web::preview`
//! is the IDE's side of the same protocol, and pins the same lines):
//!
//! ```text
//! IDE -> here   {"cmd":"open","url":"http://127.0.0.1:8123/","width":390,"height":844,"dpr":3.0,"dark":false,"locale":"pt-BR"}
//!               {"cmd":"reload"}   {"cmd":"scheme","dark":true}   {"cmd":"close"}
//!               {"cmd":"emulate","dpr":2.0,"locale":"pt-BR","safe_area":[47,0,34,0]}   (any of the three; one reload)
//! here -> IDE   {"event":"ready","visible":false}   {"event":"blocked","url":".."}
//!               {"event":"failed","message":".."}   {"event":"closed"}
//!               {"event":"console","level":"error","text":"..","form":".."}
//!               {"event":"error","message":"..","source":"..","line":3,"form":".."}
//!               {"event":"cobol-error","form":"..","unit":"..","line":7,"code":"..","message":".."}
//!               {"event":"updated","how":"..","ms":12.5,"form":".."}
//! ```
//!
//! `size` and `devtools` are gone with the visible window: they answer
//! `failed`. Any other unknown command is ignored. The web view loads the
//! Preview's own address and nothing else — a navigation to another origin,
//! and a request for a new window, are refused and reported as `blocked`.
//!
//! The page events are the served page's own Preview script's, which posts
//! them here over `window.ipc` and to `/__prc/log` in a browser, so both say
//! the same; the script this program runs first applies only the device
//! emulation (pixel ratio, language, safe-area insets), kept in the page
//! origin's `localStorage` and reapplied on every load.

/// Messages of this program's own, as protocol lines.
mod lines {
    /// `{"event":"failed","message":..}`
    pub fn failed(message: &str) -> String {
        serde_json::json!({ "event": "failed", "message": message }).to_string()
    }

    /// `{"event":"ready","visible":false}`: the page is loading, in a window
    /// that is (or, if the system says otherwise, is not) invisible.
    pub fn ready(visible: bool) -> String {
        format!("{{\"event\":\"ready\",\"visible\":{visible}}}")
    }

    /// The answer to a command the headless helper no longer has.
    pub fn removed(command: &str) -> String {
        failed(&format!("`{command}` is not a command of the headless helper"))
    }

    /// `{"event":"closed"}`
    pub fn closed() -> String {
        serde_json::json!({ "event": "closed" }).to_string()
    }

    /// `{"event":"blocked","url":..}`
    pub fn blocked(url: &str) -> String {
        serde_json::json!({ "event": "blocked", "url": url }).to_string()
    }

    /// What the served page's Preview script posts over `window.ipc` → the
    /// event line for the IDE. It is the same JSON the page posts to
    /// `/__prc/log` in a browser (`cobolt_project_tools::web::devserver::PageEvent`):
    /// the four page events are forwarded with their own fields, as text,
    /// numbers or nothing; anything else is dropped.
    pub fn from_page(message: &str) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(message).ok()?;
        let fields: &[&str] = match v.get("event")?.as_str()? {
            "console" => &["level", "text", "form"],
            "error" => &["message", "source", "line", "form"],
            "cobol-error" => &["form", "unit", "line", "code", "message"],
            "updated" => &["how", "ms", "form"],
            _ => return None,
        };
        let mut out = serde_json::Map::new();
        out.insert("event".into(), v["event"].clone());
        for f in fields {
            if let Some(x) = v.get(*f).filter(|x| x.is_string() || x.is_number()) {
                out.insert((*f).into(), x.clone());
            }
        }
        Some(serde_json::Value::Object(out).to_string())
    }
}

/// The origin of an address: `http://127.0.0.1:8123` of `http://127.0.0.1:8123/x?y`.
fn origin_of(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    format!("{scheme}://{authority}")
}

/// May the web view go to `target`, given the origin it was opened on? Its own
/// origin, and a blank page; nothing else (WASM-008 R8).
fn may_navigate(origin: &str, target: &str) -> bool {
    target == "about:blank"
        || target == origin
        || (target.starts_with(origin) && matches!(target.as_bytes().get(origin.len()), Some(b'/' | b'?' | b'#')))
}

/// The script run before every page: the device emulation the IDE has asked
/// for, kept in `localStorage`. The console, error and update capture is the
/// served page's own Preview script (the dev server's), which posts over
/// `window.ipc` here and to `/__prc/log` in a browser, so both say the same.
const INIT_SCRIPT: &str = r#"(function () {
  function stored(key) { try { return window.localStorage.getItem('__prc_' + key); } catch (e) { return null; } }
  var dpr = parseFloat(stored('dpr'));
  if (dpr > 0) { Object.defineProperty(window, 'devicePixelRatio', { get: function () { return dpr; }, configurable: true }); }
  var locale = stored('locale');
  if (locale) {
    Object.defineProperty(navigator, 'language', { get: function () { return locale; }, configurable: true });
    Object.defineProperty(navigator, 'languages', { get: function () { return [locale]; }, configurable: true });
  }
  var safe = stored('safe');
  if (safe) {
    var p = safe.split(',');
    var apply = function () {
      var s = document.documentElement.style;
      s.setProperty('--prc-safe-area-top', p[0] + 'px');
      s.setProperty('--prc-safe-area-right', p[1] + 'px');
      s.setProperty('--prc-safe-area-bottom', p[2] + 'px');
      s.setProperty('--prc-safe-area-left', p[3] + 'px');
    };
    if (document.documentElement) { apply(); } else { document.addEventListener('DOMContentLoaded', apply); }
  }
})();"#;

/// The emulation an `open` command carries, in the order the page reads it.
#[derive(Debug, Default, PartialEq)]
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
struct Emulation {
    dpr: Option<f64>,
    dark: Option<bool>,
    locale: Option<String>,
    safe_area: Option<[u64; 4]>,
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
impl Emulation {
    /// What an `open` line says; a field that is missing, or of the wrong kind, is left out.
    fn from_open(cmd: &serde_json::Value) -> Self {
        let safe_area = cmd.get("safe_area").and_then(|v| v.as_array()).and_then(|a| {
            let n: Vec<u64> = a.iter().filter_map(|x| x.as_u64()).collect();
            <[u64; 4]>::try_from(n).ok()
        });
        Self {
            dpr: cmd.get("dpr").and_then(|v| v.as_f64()).filter(|v| *v > 0.0 && v.is_finite()),
            dark: cmd.get("dark").and_then(|v| v.as_bool()),
            locale: cmd.get("locale").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_owned),
            safe_area,
        }
    }

    /// The script that puts these values where [`INIT_SCRIPT`] reads them, so the
    /// first load already has them. Empty when there is nothing to say.
    fn seed_script(&self) -> String {
        let mut script = String::new();
        let mut set = |key: &str, value: String| {
            let literal = serde_json::Value::String(value).to_string();
            script.push_str(&format!("try{{window.localStorage.setItem('__prc_{key}',{literal});}}catch(e){{}}"));
        };
        if let Some(dpr) = self.dpr {
            set("dpr", dpr.to_string());
        }
        if let Some(locale) = &self.locale {
            set("locale", locale.clone());
        }
        if let Some([t, r, b, l]) = self.safe_area {
            set("safe", format!("{t},{r},{b},{l}"));
        }
        script
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main() {
    run::run();
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    // No web view here yet (see Cargo.toml): say so in the protocol and stop, so
    // the IDE shows the Preview in the system browser instead.
    println!("{}", lines::failed("this program has no web view on this operating system"));
    println!("{}", lines::closed());
    std::process::exit(3);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod run {
    use std::io::{BufRead, Write};

    use tao::dpi::{LogicalPosition, LogicalSize};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::window::{Theme, Window, WindowBuilder};
    use wry::{NewWindowResponse, WebView, WebViewBuilder};

    use super::{lines, may_navigate, origin_of, Emulation, INIT_SCRIPT};

    /// What the reader thread hands the event loop.
    enum UserEvent {
        Command(serde_json::Value),
        /// Standard input ended: the IDE is gone.
        InputClosed,
    }

    fn say(line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    struct Shown {
        window: Window,
        view: WebView,
    }

    /// Where the window is put: off every screen, so that an engine that will
    /// not paint a hidden window still has nothing on show (plan §5).
    const OFF_SCREEN: LogicalPosition<f64> = LogicalPosition::new(-32000.0, -32000.0);

    pub fn run() {
        #[allow(unused_mut)]
        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        #[cfg(target_os = "macos")]
        {
            use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
            // No Dock icon and no menu bar: nothing of this program is on show.
            event_loop.set_activation_policy(ActivationPolicy::Accessory);
        }
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if proxy.send_event(UserEvent::Command(v)).is_err() {
                        return;
                    }
                }
            }
            let _ = proxy.send_event(UserEvent::InputClosed);
        });

        let mut shown: Option<Shown> = None;
        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::UserEvent(UserEvent::InputClosed) => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::UserEvent(UserEvent::Command(cmd)) => {
                    let name = cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
                    let num = |key: &str| cmd.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                    match name {
                        "open" if shown.is_none() => {
                            let url = cmd.get("url").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                            let (w, h) = (num("width").max(200) as f64, num("height").max(200) as f64);
                            let origin = origin_of(&url);
                            let emulation = Emulation::from_open(&cmd);
                            let mut builder = WindowBuilder::new()
                                .with_visible(false)
                                .with_position(OFF_SCREEN)
                                .with_decorations(false)
                                .with_inner_size(LogicalSize::new(w, h));
                            if let Some(dark) = emulation.dark {
                                builder = builder.with_theme(Some(if dark { Theme::Dark } else { Theme::Light }));
                            }
                            let window = match builder.build(target) {
                                Ok(w) => w,
                                Err(e) => {
                                    say(&lines::failed(&format!("the window could not be made: {e}")));
                                    *control_flow = ControlFlow::Exit;
                                    return;
                                }
                            };
                            let allowed = origin.clone();
                            let nav_origin = origin.clone();
                            let view = WebViewBuilder::new()
                                .with_url(&url)
                                .with_initialization_script(format!("{}{INIT_SCRIPT}", emulation.seed_script()))
                                .with_ipc_handler(|request| {
                                    if let Some(line) = lines::from_page(request.body()) {
                                        say(&line);
                                    }
                                })
                                .with_navigation_handler(move |target_url| {
                                    if may_navigate(&nav_origin, &target_url) {
                                        true
                                    } else {
                                        say(&lines::blocked(&target_url));
                                        false
                                    }
                                })
                                .with_new_window_req_handler(move |target_url, _features| {
                                    let _ = &allowed;
                                    say(&lines::blocked(&target_url));
                                    NewWindowResponse::Deny
                                })
                                .build(&window);
                            match view {
                                Ok(view) => {
                                    let visible = window.is_visible();
                                    shown = Some(Shown { window, view });
                                    say(&lines::ready(visible));
                                }
                                Err(e) => {
                                    say(&lines::failed(&format!("the web view could not be made: {e}")));
                                    *control_flow = ControlFlow::Exit;
                                }
                            }
                        }
                        "size" | "devtools" => say(&lines::removed(name)),
                        "reload" => {
                            if let Some(s) = &shown {
                                let _ = s.view.reload();
                            }
                        }
                        "scheme" => {
                            if let Some(s) = &shown {
                                let dark = cmd.get("dark").and_then(|v| v.as_bool()).unwrap_or(false);
                                s.window.set_theme(Some(if dark { Theme::Dark } else { Theme::Light }));
                            }
                        }
                        "emulate" => {
                            if let Some(s) = &shown {
                                // The same fields an `open` carries; the page loads again once.
                                let seed = Emulation::from_open(&cmd).seed_script();
                                let _ = s.view.evaluate_script(&format!("{seed}location.reload();"));
                            }
                        }
                        "close" => {
                            say(&lines::closed());
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => {}
                    }
                }
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    say(&lines::closed());
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines this program writes are the lines the IDE reads
    /// (`cobolt_project_tools::web::preview::Event::parse` pins the same text).
    #[test]
    fn the_events_are_the_ides_contract() {
        assert_eq!(lines::ready(false), r#"{"event":"ready","visible":false}"#);
        assert_eq!(lines::removed("size"), r#"{"event":"failed","message":"`size` is not a command of the headless helper"}"#);
        assert_eq!(lines::closed(), r#"{"event":"closed"}"#);
        assert_eq!(lines::failed("no web view"), r#"{"event":"failed","message":"no web view"}"#);
        assert_eq!(lines::blocked("https://example.com/"), r#"{"event":"blocked","url":"https://example.com/"}"#);
        // The page events, as the served Preview script posts them (WASM-008 T33).
        // (An object's keys are written in alphabetical order; the IDE reads them by name.)
        let read = |page: &str| serde_json::from_str::<serde_json::Value>(&lines::from_page(page).unwrap()).unwrap();
        assert_eq!(
            read(r#"{"event":"console","level":"error","text":"boom","form":"MAIN-FORM"}"#),
            serde_json::json!({ "event": "console", "level": "error", "text": "boom", "form": "MAIN-FORM" })
        );
        assert_eq!(
            read(r#"{"event":"error","message":"x is not defined","source":"app.js","line":12,"form":"F"}"#),
            serde_json::json!({ "event": "error", "message": "x is not defined", "source": "app.js", "line": 12, "form": "F" })
        );
        assert_eq!(
            read(r#"{"event":"cobol-error","form":"F","unit":"BTN-CLICK","line":7,"code":"EC-SIZE","message":"m","x":{"y":1}}"#),
            serde_json::json!({ "event": "cobol-error", "form": "F", "unit": "BTN-CLICK", "line": 7, "code": "EC-SIZE", "message": "m" })
        );
        assert_eq!(
            read(r#"{"event":"updated","how":"stylesheet","ms":12.5,"form":{"nested":true}}"#),
            serde_json::json!({ "event": "updated", "how": "stylesheet", "ms": 12.5 }),
            "a field of the wrong kind is left out"
        );
        assert!(lines::from_page("not json").is_none());
        assert!(lines::from_page(r#"{"event":"ready"}"#).is_none(), "the page cannot speak for the helper");
        assert!(lines::from_page(r#"{"t":"console","text":"old shape"}"#).is_none());
        assert!(lines::from_page(r#"{"nothing":1}"#).is_none());
    }

    /// The page may go to its own address and nowhere else (WASM-008 R8).
    #[test]
    fn the_web_view_stays_on_the_previews_own_origin() {
        let origin = origin_of("http://127.0.0.1:8123/index.html?x=1");
        assert_eq!(origin, "http://127.0.0.1:8123");
        for ok in ["http://127.0.0.1:8123", "http://127.0.0.1:8123/", "http://127.0.0.1:8123/a/b.js", "http://127.0.0.1:8123/?q=1", "about:blank"] {
            assert!(may_navigate(&origin, ok), "{ok}");
        }
        for bad in [
            "https://example.com/",
            "http://127.0.0.1:8124/",
            "http://127.0.0.1:81230/",
            "http://127.0.0.1:8123.evil.com/",
            "http://127.0.0.1:8123@evil.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "",
        ] {
            assert!(!may_navigate(&origin, bad), "{bad}");
        }
    }

    /// An `open` that carries the emulation seeds the page's first load; one
    /// that carries none seeds nothing (WASM-008 R5).
    #[test]
    fn an_open_line_seeds_the_first_load() {
        let plain: serde_json::Value = serde_json::from_str(r#"{"cmd":"open","url":"http://127.0.0.1:1/","width":390,"height":844}"#).unwrap();
        assert_eq!(Emulation::from_open(&plain), Emulation::default());
        assert_eq!(Emulation::from_open(&plain).seed_script(), "");

        let full: serde_json::Value = serde_json::from_str(
            r#"{"cmd":"open","url":"http://127.0.0.1:1/","width":390,"height":844,"dpr":3.0,"dark":true,"locale":"pt-BR","safe_area":[47,0,34,0]}"#,
        )
        .unwrap();
        let e = Emulation::from_open(&full);
        assert_eq!(e, Emulation { dpr: Some(3.0), dark: Some(true), locale: Some("pt-BR".into()), safe_area: Some([47, 0, 34, 0]) });
        let script = e.seed_script();
        for needle in ["'__prc_dpr',\"3\"", "'__prc_locale',\"pt-BR\"", "'__prc_safe',\"47,0,34,0\""] {
            assert!(script.contains(needle), "{needle} in {script}");
        }

        // Nonsense is left out rather than applied.
        let bad: serde_json::Value =
            serde_json::from_str(r#"{"cmd":"open","dpr":-1,"dark":"yes","locale":"","safe_area":[1,2,3]}"#).unwrap();
        assert_eq!(Emulation::from_open(&bad), Emulation::default());
    }

    /// `emulate` carries the same fields as `open` (the IDE pins this exact line),
    /// and a field it leaves out is a key the page keeps as it was.
    #[test]
    fn an_emulate_line_changes_only_what_it_names() {
        let both: serde_json::Value =
            serde_json::from_str(r#"{"cmd":"emulate","dpr":2.0,"locale":"pt-BR","safe_area":[47,0,34,0]}"#).unwrap();
        let e = Emulation::from_open(&both);
        assert_eq!(e, Emulation { dpr: Some(2.0), dark: None, locale: Some("pt-BR".into()), safe_area: Some([47, 0, 34, 0]) });
        let only_insets: serde_json::Value = serde_json::from_str(r#"{"cmd":"emulate","safe_area":[0,0,0,0]}"#).unwrap();
        let script = Emulation::from_open(&only_insets).seed_script();
        assert!(script.contains("'__prc_safe',\"0,0,0,0\""), "{script}");
        assert!(!script.contains("__prc_dpr") && !script.contains("__prc_locale"), "{script}");
    }

    #[test]
    fn the_script_keeps_what_the_ide_sets() {
        // The commands store under `__prc_<key>`; the script reads the same keys back.
        for needle in ["'__prc_' + key", "stored('dpr')", "stored('locale')", "stored('safe')", "--prc-safe-area-top"] {
            assert!(INIT_SCRIPT.contains(needle), "{needle}");
        }
        // The capture is the served page's (one script for the browser and here).
        for gone in ["window.ipc.postMessage", "unhandledrejection", "console[level]"] {
            assert!(!INIT_SCRIPT.contains(gone), "{gone}");
        }
    }
}
