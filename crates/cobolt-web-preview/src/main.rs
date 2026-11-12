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
//!               {"cmd":"render","path":"/tmp/form.png"}
//!               {"cmd":"script","steps":[{"click":{"control":"BTN-GO"}}],"time_limit_s":20}
//! here -> IDE   {"event":"ready","visible":false}   {"event":"blocked","url":".."}
//!               {"event":"failed","message":".."}   {"event":"closed"}
//!               {"event":"loading","url":".."}   (the page starts loading: the first time, and every reload)
//!               {"event":"console","level":"error","text":"..","form":".."}
//!               {"event":"error","message":"..","source":"..","line":3,"form":".."}
//!               {"event":"cobol-error","form":"..","unit":"..","line":7,"code":"..","message":".."}
//!               {"event":"updated","how":"..","ms":12.5,"form":".."}
//!               {"event":"rendered","path":"..","width":1170,"height":2532}
//!               {"event":"scripted","values":[..],"text":"..","errors":[..]}
//! ```
//!
//! `render` and `script` wait for the page to finish loading, and for each
//! other; each has a time limit and then answers `failed`. `render` is the
//! engine's own snapshot (module `snapshot`); `script` runs a closed list of
//! steps through a fixed runner in the page (module `steps`), with no `eval`.
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

    /// `{"event":"loading","url":..}`: the page has started loading — its first
    /// load, and every reload after it (WASM-008 T47), whether or not a script runs.
    pub fn loading(url: &str) -> String {
        serde_json::json!({ "event": "loading", "url": url }).to_string()
    }

    /// `{"event":"rendered","path":..,"width":..,"height":..}`: the PNG is written,
    /// and these are its pixels, read back from the file.
    pub fn rendered(path: &str, width: u32, height: u32) -> String {
        serde_json::json!({ "event": "rendered", "path": path, "width": width, "height": height }).to_string()
    }

    /// `{"event":"scripted","values":[..],"text":"..","errors":[..]}`
    pub fn scripted(values: &serde_json::Value, text: &str, errors: &[String]) -> String {
        serde_json::json!({ "event": "scripted", "values": values, "text": text, "errors": errors }).to_string()
    }

    /// What the step runner posts at the end of a script → the `scripted` line,
    /// if it is the answer to the script `nonce` (the page cannot make one up).
    pub fn from_runner(message: &str, nonce: u64) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(message).ok()?;
        if v.get("event")?.as_str()? != super::steps::RUNNER_EVENT || v.get("nonce")?.as_u64()? != nonce {
            return None;
        }
        let values = v.get("values").filter(|x| x.is_array()).cloned().unwrap_or_else(|| serde_json::json!([]));
        let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
        let errors: Vec<String> = v
            .get("errors")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        Some(scripted(&values, text, &errors))
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
      s.setProperty('--pr-safe-top', p[0] + 'px');
      s.setProperty('--pr-safe-right', p[1] + 'px');
      s.setProperty('--pr-safe-bottom', p[2] + 'px');
      s.setProperty('--pr-safe-left', p[3] + 'px');
    };
    if (document.documentElement) { apply(); } else { document.addEventListener('DOMContentLoaded', apply); }
  }
})();"#;

/// The `script` command: a closed set of steps, checked here and run by the
/// fixed [`steps::RUNNER`] in the page. There is no `eval` of caller text: a
/// step's text, names and properties reach the page as JSON data, never as code
/// (WASM-008 T41, the operator's decision of 2026-11-01: `click` and `type`
/// fire real DOM events on the rendered page).
mod steps {
    use serde_json::{json, Value};

    /// The `event` the runner posts its answer under, over `window.ipc`.
    pub const RUNNER_EVENT: &str = "__prc-scripted";

    /// The longest `wait` one step may ask for.
    pub const MAX_WAIT_MS: u64 = 30_000;

    /// The window properties the `page` step reads, and nothing else (WASM-008 T47).
    pub const PAGE_PROPERTIES: [&str; 6] =
        ["innerWidth", "innerHeight", "devicePixelRatio", "crossOriginIsolated", "location.pathname", "navigator.userAgent"];

    /// The roles a `page` step may name as its `frame`.
    pub const FRAMES: [&str; 1] = ["phone-frame"];

    /// What a step addresses: a control by name (`[data-name=..]`), or a role —
    /// the player's `form`, `snackbar`, `layer:<NAME>` (WASM-015 plan §3.2), and the
    /// Preview's `preview-errors` (the failed build's overlay) and `phone-frame`
    /// (the frame of `/__prc/phone`), both outside the player (WASM-008 T47).
    fn target(v: Option<&Value>) -> Result<String, String> {
        let s = v.and_then(Value::as_str).ok_or("needs `control`, a control's name or a role")?;
        let name = s.strip_prefix("layer:").unwrap_or(s);
        let ok = !name.is_empty()
            && name.len() <= 64
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if ok {
            Ok(s.to_owned())
        } else {
            Err(format!("`{s}` is not a control name or a role"))
        }
    }

    /// A property or CSS property name: letters, digits and `-`, nothing else.
    fn word(v: Option<&Value>, what: &str) -> Result<String, String> {
        let s = v.and_then(Value::as_str).ok_or_else(|| format!("needs `{what}`"))?;
        let ok = s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '-')
            && s.len() <= 64
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if ok {
            Ok(s.to_owned())
        } else {
            Err(format!("`{s}` is not a {what}"))
        }
    }

    /// One step of a `script` → the runner's form of it. A step is an object
    /// with one key, the step's name:
    ///
    /// ```text
    /// {"click":{"control":"BTN-GO"}}            a real click on the control
    /// {"type":{"control":"TXT-NAME","text":"Ana"}}   real input and change events
    /// {"wait":{"ms":200}}                       then the page settles
    /// {"wait":{"control":"form"}}               until it is on the page (at most 30 s)
    /// {"get":{"control":"LBL-OUT","property":"Text"}}   Text, Value, Caption, Visible, Enabled, Checked
    /// {"text":{"control":".."}}  {"style":{"control":"..","property":"color"}}
    /// {"rect":{"control":".."}}  {"focused":{}}  {"key":{"key":"Tab","shift":false}}
    /// {"snapshot":{}}
    /// {"page":{"property":"innerWidth","frame":"phone-frame"}}   a window property of a fixed list
    /// ```
    pub fn parse_step(step: &Value) -> Result<Value, String> {
        let obj = step.as_object().filter(|o| o.len() == 1).ok_or("a step is an object with one key")?;
        let (name, arg) = obj.iter().next().expect("one key");
        let field = |k: &str| arg.get(k);
        Ok(match name.as_str() {
            "click" | "text" | "rect" => json!({ "op": name, "target": target(field("control"))? }),
            "type" => {
                let text = field("text").and_then(Value::as_str).ok_or("`type` needs `text`")?;
                json!({ "op": "type", "target": target(field("control"))?, "text": text })
            }
            "wait" => match (field("ms").and_then(Value::as_u64), field("control")) {
                (Some(ms), None) => json!({ "op": "wait", "ms": ms.min(MAX_WAIT_MS) }),
                (None, Some(_)) => json!({ "op": "wait", "target": target(field("control"))?, "ms": MAX_WAIT_MS }),
                _ => return Err("`wait` needs `ms`, or `control` to wait for".into()),
            },
            "get" | "style" => {
                json!({ "op": name, "target": target(field("control"))?, "property": word(field("property"), "property")? })
            }
            "focused" | "snapshot" => json!({ "op": name }),
            "key" => {
                let key = word(field("key"), "key")?;
                let shift = field("shift").and_then(Value::as_bool).unwrap_or(false);
                json!({ "op": "key", "key": key, "shift": shift })
            }
            "page" => {
                let property = field("property").and_then(Value::as_str).ok_or("`page` needs `property`")?;
                if !PAGE_PROPERTIES.contains(&property) {
                    return Err(format!("`{property}` is not a page property ({})", PAGE_PROPERTIES.join(", ")));
                }
                match field("frame") {
                    None => json!({ "op": "page", "property": property }),
                    Some(f) => {
                        let frame = f.as_str().filter(|f| FRAMES.contains(f)).ok_or_else(|| format!("`frame` is one of {}", FRAMES.join(", ")))?;
                        json!({ "op": "page", "property": property, "frame": frame })
                    }
                }
            }
            other => return Err(format!("`{other}` is not a step")),
        })
    }

    /// The steps of a `script` line, each checked; the first wrong one is the error.
    pub fn parse(cmd: &Value) -> Result<Vec<Value>, String> {
        let steps = cmd.get("steps").and_then(Value::as_array).ok_or("`script` needs `steps`, a list")?;
        steps
            .iter()
            .enumerate()
            .map(|(i, s)| parse_step(s).map_err(|e| format!("step {}: {e}", i + 1)))
            .collect()
    }

    /// The call the helper makes for a script: the fixed runner, given the
    /// checked steps as a JSON literal.
    pub fn call(nonce: u64, steps: &[Value]) -> String {
        format!("window.__prcSteps({nonce},{});", Value::Array(steps.to_vec()))
    }

    /// The step runner, defined before the page's own scripts on every load
    /// and not replaceable by them. It knows the steps above and nothing else.
    pub const RUNNER: &str = r#"(function () {
  if (window.top !== window || window.__prcSteps) return;
  var said = [], seen = 0, errs = [];
  ['log', 'info'].forEach(function (level) {
    var o = console[level];
    console[level] = function () {
      try { said.push(Array.prototype.map.call(arguments, String).join(' ')); seen++; if (said.length > 10000) said.shift(); } catch (e) {}
      return o.apply(console, arguments);
    };
  });
  var oe = console.error;
  console.error = function () {
    try { errs.push(Array.prototype.map.call(arguments, String).join(' ')); } catch (e) {}
    return oe.apply(console, arguments);
  };
  window.addEventListener('error', function (e) { errs.push(String(e.message || e)); });
  window.addEventListener('unhandledrejection', function (e) { errs.push(String(e.reason && e.reason.message || e.reason)); });
  function scope() { var a = document.getElementById('pr-app'); return (a && a.shadowRoot) || document; }
  function active() { var s = scope(); return (s === document ? document.activeElement : s.activeElement) || null; }
  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }
  function settle() {
    return new Promise(function (resolve) {
      var last = Date.now(), t0 = last;
      var mo = new MutationObserver(function () { last = Date.now(); });
      var o = { subtree: true, childList: true, attributes: true, characterData: true };
      mo.observe(document, o);
      if (scope() !== document) mo.observe(scope(), o);
      (function tick() {
        var now = Date.now();
        if (now - last >= 50 || now - t0 >= 2000) { mo.disconnect(); resolve(); } else { setTimeout(tick, 10); }
      })();
    });
  }
  function shown(el) {
    if (!el || el.closest('[hidden]')) return false;
    var s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden' && el.getClientRects().length > 0;
  }
  function find(t) {
    var all;
    if (t === 'preview-errors') return document.querySelector('.prc-preview-errors');
    if (t === 'phone-frame') return document.getElementById('pr-phone');
    if (t === 'form') { all = scope().querySelectorAll('.pr-form-root'); return all[all.length - 1] || null; }
    if (t === 'snackbar') return scope().querySelector('.pr-snackbar');
    if (t.indexOf('layer:') === 0) return scope().querySelector('[data-layer="' + t.slice(6) + '"]');
    return scope().querySelector('[data-name="' + t + '"]');
  }
  function need(t) { var el = find(t); if (!el) throw new Error('no `' + t + '` on the page'); return el; }
  function field(el) { return el.matches('input,textarea,select,[contenteditable]') ? el : el.querySelector('input,textarea,select,[contenteditable]'); }
  function visible(t) {
    if (t.indexOf('layer:') === 0) {
      var all = scope().querySelectorAll('[data-layer="' + t.slice(6) + '"]');
      if (!all.length) throw new Error('no `' + t + '` on the page');
      for (var i = 0; i < all.length; i++) if (shown(all[i])) return true;
      return false;
    }
    return shown(need(t));
  }
  function read(t, p) {
    if (p === 'Visible') return visible(t);
    var el = need(t), f = field(el);
    switch (p) {
      case 'Text': case 'Value': case 'Caption':
        if (f && 'value' in f && f.tagName !== 'BUTTON') return String(f.value);
        return (el.innerText || el.textContent || '').trim();
      case 'Enabled': return !(el.disabled || (f && f.disabled) || el.getAttribute('aria-disabled') === 'true' || el.classList.contains('pr-disabled'));
      case 'Checked': var c = el.matches('input') ? el : el.querySelector('input[type=checkbox],input[type=radio]'); if (!c) throw new Error('`' + t + '` has no check box'); return !!c.checked;
    }
    throw new Error('`' + p + '` cannot be read from the page; read it with `style`, `text` or `rect`');
  }
  function mouse(el, type) {
    var r = el.getBoundingClientRect(), o = { bubbles: true, cancelable: true, composed: true, view: window, button: 0, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 };
    var E = type.indexOf('pointer') === 0 && window.PointerEvent ? PointerEvent : MouseEvent;
    if (E === PointerEvent) { o.pointerId = 1; o.pointerType = 'mouse'; o.isPrimary = true; }
    el.dispatchEvent(new E(type, o));
  }
  function tabbables() {
    return Array.prototype.filter.call(scope().querySelectorAll('a[href],button,input,select,textarea,[tabindex]'), function (el) {
      return el.tabIndex >= 0 && !el.disabled && !el.closest('[inert]') && shown(el);
    });
  }
  async function step(s) {
    switch (s.op) {
      case 'click': {
        var el = need(s.target);
        if (!shown(el)) throw new Error('`' + s.target + '` is not on show');
        ['pointerdown', 'mousedown'].forEach(function (k) { mouse(el, k); });
        if (typeof el.focus === 'function') el.focus();
        ['pointerup', 'mouseup', 'click'].forEach(function (k) { mouse(el, k); });
        await settle(); return;
      }
      case 'type': {
        var f = field(need(s.target));
        if (!f) throw new Error('`' + s.target + '` takes no typing');
        f.focus();
        if (f.isContentEditable) { f.textContent = s.text; } else {
          var proto = Object.getPrototypeOf(f), d = Object.getOwnPropertyDescriptor(proto, 'value');
          if (d && d.set) d.set.call(f, s.text); else f.value = s.text;
        }
        f.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: s.text }));
        f.dispatchEvent(new Event('change', { bubbles: true }));
        await settle(); return;
      }
      case 'wait':
        if (s.target) {
          var until = Date.now() + s.ms;
          while (!find(s.target)) { if (Date.now() > until) throw new Error('`' + s.target + '` did not appear'); await sleep(25); }
        } else { await sleep(s.ms); }
        await settle(); return;
      case 'get': return read(s.target, s.property);
      case 'text': var te = need(s.target); return (te.innerText || te.textContent || '').trim();
      case 'style': return getComputedStyle(need(s.target)).getPropertyValue(s.property);
      case 'rect': var r = need(s.target).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height };
      case 'focused': var a = active(), n = a && a.closest('[data-name]'); return n ? n.getAttribute('data-name') : null;
      case 'key': {
        var target = active() || document.body, o = { key: s.key, bubbles: true, cancelable: true, shiftKey: !!s.shift };
        var go = target.dispatchEvent(new KeyboardEvent('keydown', o));
        if (go && s.key === 'Tab') {
          var list = tabbables(), i = list.indexOf(active()), next = list[(i + (s.shift ? -1 : 1) + list.length) % list.length];
          if (next) next.focus();
        }
        (active() || document.body).dispatchEvent(new KeyboardEvent('keyup', o));
        await settle(); return;
      }
      case 'snapshot': throw new Error('the page does not hand the player\'s state to the helper (WASM-015)');
      case 'page': {
        var w = window;
        if (s.frame) {
          var fr = need(s.frame), by = Date.now() + 10000;
          while (!(fr.contentDocument && fr.contentDocument.readyState === 'complete' && fr.contentWindow.location.href !== 'about:blank')) {
            if (Date.now() > by) throw new Error('`' + s.frame + '` did not load');
            await sleep(25);
          }
          w = fr.contentWindow;
        }
        switch (s.property) {
          case 'innerWidth': return w.innerWidth;
          case 'innerHeight': return w.innerHeight;
          case 'devicePixelRatio': return w.devicePixelRatio;
          case 'crossOriginIsolated': return !!w.crossOriginIsolated;
          case 'location.pathname': return String(w.location.pathname);
          case 'navigator.userAgent': return String(w.navigator.userAgent);
        }
        throw new Error('`' + s.property + '` is not a page property');
      }
    }
    throw new Error('`' + s.op + '` is not a step');
  }
  Object.defineProperty(window, '__prcSteps', {
    value: async function (nonce, steps) {
      if (document.readyState !== 'complete') await new Promise(function (r) { window.addEventListener('load', r, { once: true }); });
      await settle();
      var values = [], errors = [], from = seen, e0 = errs.length;
      for (var i = 0; i < steps.length; i++) {
        try { var v = await step(steps[i]); if (v !== undefined) values.push(v); }
        catch (e) { errors.push('step ' + (i + 1) + ' (' + steps[i].op + '): ' + (e && e.message || e)); if (/^(get|text|style|rect|page)$/.test(steps[i].op)) values.push(null); }
      }
      var text = said.slice(Math.max(0, said.length - (seen - from))).join('\n');
      errors = errs.slice(e0).concat(errors);
      window.ipc.postMessage(JSON.stringify({ event: '__prc-scripted', nonce: nonce, values: values, text: text, errors: errors }));
    },
    writable: false, configurable: false, enumerable: false
  });
})();"#;

    /// The pixels of a PNG, read from its header; `None` when it is not one.
    pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
        if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
            return None;
        }
        let at = |k: usize| u32::from_be_bytes([bytes[k], bytes[k + 1], bytes[k + 2], bytes[k + 3]]);
        Some((at(16), at(20)))
    }
}

/// The emulation an `open` command carries, in the order the page reads it.
#[derive(Debug, Default, PartialEq)]
#[cfg_attr(not(any(target_os = "macos", target_os = "windows", target_os = "linux")), allow(dead_code))]
struct Emulation {
    dpr: Option<f64>,
    dark: Option<bool>,
    locale: Option<String>,
    safe_area: Option<[u64; 4]>,
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows", target_os = "linux")), allow(dead_code))]
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

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        // GTK cannot start without a display; say so instead of failing inside it.
        println!("{}", lines::failed("WebKitGTK needs a display (DISPLAY or WAYLAND_DISPLAY; Xvfb on a server)"));
        println!("{}", lines::closed());
        std::process::exit(3);
    }
    run::run();
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn main() {
    // No web view on this system (see Cargo.toml): say so in the protocol and stop;
    // the tools then answer that a web form cannot be drawn here.
    println!("{}", lines::failed("this program has no web view on this operating system"));
    println!("{}", lines::closed());
    std::process::exit(3);
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod run {
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::io::{BufRead, Write};
    use std::rc::Rc;
    use std::time::Duration;

    use tao::dpi::{LogicalPosition, LogicalSize};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
    use tao::window::{Theme, Window, WindowBuilder};
    use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

    use super::{lines, may_navigate, origin_of, snapshot, steps, Emulation, INIT_SCRIPT};

    /// What the reader thread, the timers and the web view hand the event loop.
    pub enum UserEvent {
        Command(serde_json::Value),
        /// Standard input ended: the IDE is gone.
        InputClosed,
        /// The page has started loading again: a script running on the old page
        /// will never answer, and later commands wait for the new one.
        Loading,
        /// The page has finished loading: the commands that wait for it may run.
        Loaded,
        /// A script or a render has had its time; if it is still the one
        /// waiting, it is answered `failed`.
        TimeUp(u64),
    }

    pub fn say(line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    struct Shown {
        window: Window,
        view: WebView,
        /// The viewport, in CSS pixels, and the pixel ratio a `render` draws at.
        width: f64,
        height: f64,
        dpr: f64,
    }

    /// The one `script` or `render` being answered, by its number. A second one
    /// waits until the first is answered, so each answer is the answer to one command.
    #[derive(Default)]
    pub struct Busy {
        pub next: Cell<u64>,
        pub script: Cell<Option<u64>>,
        pub render: Cell<Option<u64>>,
    }

    impl Busy {
        fn any(&self) -> bool {
            self.script.get().is_some() || self.render.get().is_some()
        }
        fn number(&self) -> u64 {
            // Not guessable by the page from the previous one alone.
            let n = self.next.get().wrapping_add(1);
            self.next.set(n);
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64).unwrap_or(0);
            (n << 32) | (t & 0xffff_ffff)
        }
    }

    /// Where the window is put: off every screen, so that an engine that will
    /// not paint a hidden window still has nothing on show (plan §5).
    const OFF_SCREEN: LogicalPosition<f64> = LogicalPosition::new(-32000.0, -32000.0);

    /// A render that takes longer than this is answered `failed`.
    const RENDER_LIMIT: Duration = Duration::from_secs(15);

    fn time_up(proxy: &EventLoopProxy<UserEvent>, n: u64, after: Duration) {
        let proxy = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(after);
            let _ = proxy.send_event(UserEvent::TimeUp(n));
        });
    }

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
        let reader = proxy.clone();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if reader.send_event(UserEvent::Command(v)).is_err() {
                        return;
                    }
                }
            }
            let _ = reader.send_event(UserEvent::InputClosed);
        });

        let mut shown: Option<Shown> = None;
        let busy = Rc::new(Busy::default());
        // `render` and `script` wait for the page's load, and for each other.
        let mut loaded = false;
        let mut waiting: VecDeque<serde_json::Value> = VecDeque::new();
        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::Wait;
            let mut commands: Vec<serde_json::Value> = Vec::new();
            match event {
                Event::UserEvent(UserEvent::InputClosed) => {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                Event::UserEvent(UserEvent::Loading) => {
                    loaded = false;
                    if busy.script.take().is_some() {
                        say(&lines::failed("the page loaded again while the script ran"));
                    }
                }
                Event::UserEvent(UserEvent::Loaded) => loaded = true,
                Event::UserEvent(UserEvent::TimeUp(n)) => {
                    if busy.script.get() == Some(n) {
                        busy.script.set(None);
                        say(&lines::failed("the script did not finish in its time"));
                    }
                    if busy.render.get() == Some(n) {
                        busy.render.set(None);
                        say(&lines::failed(&format!("{} took no picture in its time", snapshot::ENGINE)));
                    }
                }
                Event::UserEvent(UserEvent::Command(cmd)) => commands.push(cmd),
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    say(&lines::closed());
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                _ => {}
            }
            // What waited, in order, once nothing holds it back; then what came now.
            while loaded && !busy.any() && !waiting.is_empty() {
                let cmd = waiting.pop_front().expect("not empty");
                handle(cmd, &mut shown, &busy, &proxy, &mut loaded, &mut waiting, target, control_flow);
            }
            for cmd in commands {
                handle(cmd, &mut shown, &busy, &proxy, &mut loaded, &mut waiting, target, control_flow);
            }
        });
    }

    /// The web view in the window: through its raw handle on macOS and Windows,
    /// inside the window's GTK box on Linux (wry's WebKitGTK backend).
    fn build_view(builder: WebViewBuilder, window: &Window) -> Result<WebView, String> {
        #[cfg(target_os = "linux")]
        {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            let vbox = window.default_vbox().ok_or("the window has no GTK box")?;
            builder.build_gtk(vbox).map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "linux"))]
        builder.build(window).map_err(|e| e.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle(
        cmd: serde_json::Value,
        shown: &mut Option<Shown>,
        busy: &Rc<Busy>,
        proxy: &EventLoopProxy<UserEvent>,
        loaded: &mut bool,
        waiting: &mut VecDeque<serde_json::Value>,
        target: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
        control_flow: &mut ControlFlow,
    ) {
        let name = cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_owned();
        let num = |key: &str| cmd.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
        match name.as_str() {
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
                let ipc_busy = Rc::clone(busy);
                let load_proxy = proxy.clone();
                let builder = WebViewBuilder::new()
                    .with_url(&url)
                    .with_initialization_script(format!("{}{INIT_SCRIPT}{}", emulation.seed_script(), steps::RUNNER))
                    .with_ipc_handler(move |request| {
                        let body = request.body();
                        if let Some(n) = ipc_busy.script.get() {
                            if let Some(line) = lines::from_runner(body, n) {
                                ipc_busy.script.set(None);
                                say(&line);
                                return;
                            }
                        }
                        if let Some(line) = lines::from_page(body) {
                            say(&line);
                        }
                    })
                    .with_on_page_load_handler(move |event, url| {
                        let _ = load_proxy.send_event(match event {
                            PageLoadEvent::Started => {
                                // Said at once, script or not: "no page reload" is then read, not inferred.
                                say(&lines::loading(&url));
                                UserEvent::Loading
                            }
                            PageLoadEvent::Finished => UserEvent::Loaded,
                        });
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
                    });
                match build_view(builder, &window) {
                    Ok(view) => {
                        let visible = window.is_visible();
                        let dpr = emulation.dpr.unwrap_or_else(|| window.scale_factor());
                        snapshot::prepare(&window, &view, w, h, dpr);
                        *shown = Some(Shown { window, view, width: w, height: h, dpr });
                        say(&lines::ready(visible));
                    }
                    Err(e) => {
                        say(&lines::failed(&format!("the web view could not be made: {e}")));
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            "size" | "devtools" => say(&lines::removed(&name)),
            "reload" => {
                if let Some(s) = shown {
                    *loaded = false;
                    let _ = s.view.reload();
                }
            }
            "scheme" => {
                if let Some(s) = shown {
                    let dark = cmd.get("dark").and_then(|v| v.as_bool()).unwrap_or(false);
                    s.window.set_theme(Some(if dark { Theme::Dark } else { Theme::Light }));
                }
            }
            "emulate" => {
                if let Some(s) = shown {
                    // The same fields an `open` carries; the page loads again once.
                    let seed = Emulation::from_open(&cmd).seed_script();
                    *loaded = false;
                    let _ = s.view.evaluate_script(&format!("{seed}location.reload();"));
                }
            }
            "script" | "render" => {
                let Some(s) = shown.as_ref() else {
                    say(&lines::failed(&format!("`{name}` needs a page: send `open` first")));
                    return;
                };
                if !*loaded || busy.any() {
                    waiting.push_back(cmd);
                    return;
                }
                if name == "script" {
                    let checked = match steps::parse(&cmd) {
                        Ok(c) => c,
                        Err(e) => {
                            say(&lines::failed(&e));
                            return;
                        }
                    };
                    let limit = cmd.get("time_limit_s").and_then(|v| v.as_u64()).unwrap_or(20).clamp(1, 120);
                    let n = busy.number();
                    busy.script.set(Some(n));
                    time_up(proxy, n, Duration::from_secs(limit));
                    if let Err(e) = s.view.evaluate_script(&steps::call(n, &checked)) {
                        busy.script.set(None);
                        say(&lines::failed(&format!("the script could not start: {e}")));
                    }
                } else {
                    let path = cmd.get("path").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                    if path.is_empty() {
                        say(&lines::failed("`render` needs `path`, the PNG file to write"));
                        return;
                    }
                    let n = busy.number();
                    busy.render.set(Some(n));
                    time_up(proxy, n, RENDER_LIMIT);
                    let done = Rc::clone(busy);
                    snapshot::take(&s.view, s.width, s.height, s.dpr, path, move |answer| {
                        if done.render.get() == Some(n) {
                            done.render.set(None);
                            say(&answer);
                        }
                    });
                }
            }
            "close" => {
                say(&lines::closed());
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    }
}

/// The engine's own picture of the viewport (WASM-008 T41, the operator's
/// decision of 2026-11-01): WKWebView's `takeSnapshotWithConfiguration` on
/// macOS, WebView2's `CapturePreview` on Windows, WebKitGTK's
/// `get_snapshot` on Linux. wry has no snapshot of its
/// own, so each engine is reached through wry's platform handle. The PNG is
/// `width × dpr` by `height × dpr` pixels; an engine that cannot take the
/// picture answers `failed` naming it, never a blank picture.
#[cfg(target_os = "macos")]
mod snapshot {
    use objc2::rc::Retained;
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage};
    use objc2_foundation::{NSDictionary, NSError, NSNumber, NSPoint, NSRect, NSSize};
    use objc2_web_kit::WKSnapshotConfiguration;
    use wry::{WebView, WebViewExtMacOS};

    use super::lines;

    pub const ENGINE: &str = "WebKit (WKWebView)";

    /// Nothing to do on macOS: the view is the window's, at the viewport's size.
    pub fn prepare(_window: &tao::window::Window, _view: &WebView, _w: f64, _h: f64, _dpr: f64) {}

    pub fn take(view: &WebView, w: f64, h: f64, dpr: f64, path: String, answer: impl Fn(String) + 'static) {
        let Some(mtm) = MainThreadMarker::new() else {
            return answer(lines::failed(&format!("{ENGINE}: not on the main thread")));
        };
        let web = view.webview();
        let config = unsafe { WKSnapshotConfiguration::new(mtm) };
        unsafe {
            config.setRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h)));
            config.setSnapshotWidth(Some(&NSNumber::new_f64(w)));
            config.setAfterScreenUpdates(true);
        }
        let block = block2::RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
            let line = if image.is_null() {
                let why = unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "no picture".into());
                lines::failed(&format!("{ENGINE}: {why}"))
            } else {
                match write_png(unsafe { &*image }, w, h, dpr, &path) {
                    Ok((pw, ph)) => lines::rendered(&path, pw, ph),
                    Err(e) => lines::failed(&format!("{ENGINE}: {e}")),
                }
            };
            answer(line);
        });
        unsafe { web.takeSnapshotWithConfiguration_completionHandler(Some(&config), &block) };
    }

    /// The picture drawn into a bitmap of the asked pixels, written as a PNG.
    fn write_png(image: &NSImage, w: f64, h: f64, dpr: f64, path: &str) -> Result<(u32, u32), String> {
        let (pw, ph) = ((w * dpr).round() as isize, (h * dpr).round() as isize);
        let rep: Retained<NSBitmapImageRep> = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                pw,
                ph,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }
        .ok_or("no bitmap of that size")?;
        rep.setSize(NSSize::new(w, h));
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).ok_or("no drawing context")?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        image.drawInRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h)));
        context.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        let data = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }.ok_or("no PNG")?;
        let bytes = data.to_vec();
        std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        super::steps::png_size(&bytes).ok_or_else(|| "the PNG has no header".into())
    }
}

#[cfg(target_os = "windows")]
mod snapshot {
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Controller3, COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG};
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{STGM_CREATE, STGM_READWRITE};
    use windows::Win32::UI::Shell::SHCreateStreamOnFileEx;
    use wry::{WebView, WebViewExtWindows};

    use super::lines;

    pub const ENGINE: &str = "WebView2";

    /// The web view draws at the asked pixel ratio, not the monitor's: the
    /// window is `width × dpr` physical pixels and WebView2 rasterizes at `dpr`,
    /// so the page still sees a `width × height` CSS viewport.
    pub fn prepare(window: &tao::window::Window, view: &WebView, w: f64, h: f64, dpr: f64) {
        if let Ok(c3) = view.controller().cast::<ICoreWebView2Controller3>() {
            unsafe {
                let _ = c3.SetShouldDetectMonitorScaleChanges(false);
                let _ = c3.SetRasterizationScale(dpr);
            }
        }
        window.set_inner_size(tao::dpi::PhysicalSize::new((w * dpr).round(), (h * dpr).round()));
    }

    pub fn take(view: &WebView, _w: f64, _h: f64, _dpr: f64, path: String, answer: impl Fn(String) + 'static) {
        let answer = std::rc::Rc::new(answer);
        let later = std::rc::Rc::clone(&answer);
        let stream = match unsafe { SHCreateStreamOnFileEx(&HSTRING::from(path.as_str()), (STGM_CREATE | STGM_READWRITE).0, 0, true, None) } {
            Ok(s) => s,
            Err(e) => return answer(lines::failed(&format!("{ENGINE}: {path}: {e}"))),
        };
        let keep = stream.clone();
        let file = path.clone();
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
            drop(keep);
            let line = match result {
                Err(e) => lines::failed(&format!("{ENGINE}: {e}")),
                Ok(()) => match std::fs::read(&file).ok().as_deref().and_then(super::steps::png_size) {
                    Some((pw, ph)) => lines::rendered(&file, pw, ph),
                    None => lines::failed(&format!("{ENGINE}: no PNG was written")),
                },
            };
            later(line);
            Ok(())
        }));
        if let Err(e) = unsafe { view.webview().CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, &stream, &handler) } {
            answer(lines::failed(&format!("{ENGINE}: {e}")));
        }
    }
}

#[cfg(target_os = "linux")]
mod snapshot {
    use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
    use wry::{WebView, WebViewExtUnix};

    use super::lines;

    pub const ENGINE: &str = "WebKitGTK";

    /// WebKitGTK gives its view a size when the window is mapped. The window is made hidden, so the view
    /// stayed 0 x 0 and a snapshot of it failed with `There was an error creating the snapshot` (the
    /// first run of the helper's tests on GitHub's Linux runner, run 49). The window is shown, where no
    /// one sees it: it sits at `OFF_SCREEN`, and `ready` still says `visible: false` (that is the
    /// window's state when it opened). macOS and Windows draw a hidden window and need nothing.
    pub fn prepare(window: &tao::window::Window, _view: &WebView, _w: f64, _h: f64, _dpr: f64) {
        window.set_visible(true);
    }

    pub fn take(view: &WebView, w: f64, h: f64, dpr: f64, path: String, answer: impl Fn(String) + 'static) {
        view.webview().snapshot(SnapshotRegion::Visible, SnapshotOptions::NONE, None::<&webkit2gtk::gio::Cancellable>, move |result| {
            let line = match result.map_err(|e| e.to_string()).and_then(|s| write_png(s, w, h, dpr, &path)) {
                Ok((pw, ph)) => lines::rendered(&path, pw, ph),
                Err(e) => lines::failed(&format!("{ENGINE}: {e}")),
            };
            answer(line);
        });
    }

    /// The picture scaled onto a surface of the asked pixels, written as a PNG.
    fn write_png(surface: cairo::Surface, w: f64, h: f64, dpr: f64, path: &str) -> Result<(u32, u32), String> {
        let source = cairo::ImageSurface::try_from(surface).map_err(|_| "the picture is not an image surface".to_owned())?;
        let (pw, ph) = ((w * dpr).round() as i32, (h * dpr).round() as i32);
        let target = cairo::ImageSurface::create(cairo::Format::ARgb32, pw, ph).map_err(|e| e.to_string())?;
        {
            let cr = cairo::Context::new(&target).map_err(|e| e.to_string())?;
            cr.scale(f64::from(pw) / f64::from(source.width().max(1)), f64::from(ph) / f64::from(source.height().max(1)));
            cr.set_source_surface(&source, 0.0, 0.0).map_err(|e| e.to_string())?;
            cr.paint().map_err(|e| e.to_string())?;
        }
        let mut bytes = Vec::new();
        target.write_to_png(&mut bytes).map_err(|e| e.to_string())?;
        std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        super::steps::png_size(&bytes).ok_or_else(|| "the PNG has no header".into())
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
        assert_eq!(lines::loading("http://127.0.0.1:8123/"), r#"{"event":"loading","url":"http://127.0.0.1:8123/"}"#);
        assert!(lines::from_page(r#"{"event":"loading","url":"x"}"#).is_none(), "the page cannot say it is loading");
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

    /// `render` and `script` answer with these lines (the IDE pins the same text).
    #[test]
    fn the_render_and_script_answers_are_the_ides_contract() {
        assert_eq!(lines::rendered("/tmp/a.png", 1170, 2532), r#"{"event":"rendered","height":2532,"path":"/tmp/a.png","width":1170}"#);
        assert_eq!(
            lines::scripted(&serde_json::json!(["Ana", true]), "HELLO", &["step 2 (click): no `X` on the page".into()]),
            r#"{"errors":["step 2 (click): no `X` on the page"],"event":"scripted","text":"HELLO","values":["Ana",true]}"#
        );
        // Only the runner's answer to the script being run becomes `scripted`.
        let answer = r#"{"event":"__prc-scripted","nonce":7,"values":["x"],"text":"t","errors":[]}"#;
        assert_eq!(lines::from_runner(answer, 7).unwrap(), r#"{"errors":[],"event":"scripted","text":"t","values":["x"]}"#);
        assert!(lines::from_runner(answer, 8).is_none(), "another script's number");
        assert!(lines::from_runner(r#"{"event":"console","nonce":7}"#, 7).is_none());
        assert!(lines::from_page(answer).is_none(), "the runner's answer is not a page event");
    }

    /// The steps are a closed set; names and properties are checked words, and a
    /// step's text reaches the page as JSON data (WASM-008 T41: no `eval`).
    #[test]
    fn a_script_is_a_closed_set_of_checked_steps() {
        let cmd: serde_json::Value = serde_json::from_str(
            r#"{"cmd":"script","steps":[
                {"type":{"control":"TXT-NAME","text":"Ana'); alert(1); ('"}},
                {"click":{"control":"BTN-GO"}},
                {"wait":{"ms":999999}},
                {"get":{"control":"layer:LAYER-1","property":"Visible"}},
                {"style":{"control":"form","property":"background-color"}},
                {"text":{"control":"snackbar"}}, {"rect":{"control":"LBL"}}, {"focused":{}},
                {"key":{"key":"Tab","shift":true}}, {"snapshot":{}}]}"#,
        )
        .unwrap();
        let s = steps::parse(&cmd).unwrap();
        assert_eq!(s[0], serde_json::json!({ "op": "type", "target": "TXT-NAME", "text": "Ana'); alert(1); ('" }));
        assert_eq!(s[2], serde_json::json!({ "op": "wait", "ms": steps::MAX_WAIT_MS }), "a wait is capped");
        let until = steps::parse_step(&serde_json::json!({ "wait": { "control": "form" } })).unwrap();
        assert_eq!(until, serde_json::json!({ "op": "wait", "target": "form", "ms": steps::MAX_WAIT_MS }));
        assert!(steps::parse_step(&serde_json::json!({ "wait": {} })).is_err());
        assert_eq!(s[8], serde_json::json!({ "op": "key", "key": "Tab", "shift": true }));
        let call = steps::call(42, &s);
        assert!(call.starts_with("window.__prcSteps(42,[{"), "{call}");
        assert!(call.contains(r#""text":"Ana'); alert(1); ('""#), "the text is a JSON string: {call}");

        for (bad, why) in [
            (r#"{"eval":"alert(1)"}"#, "`eval` is not a step"),
            (r#"{"click":{"control":"a\"]; x"}}"#, "is not a control name"),
            (r#"{"click":{"control":""}}"#, "is not a control name"),
            (r#"{"style":{"control":"LBL","property":"color;x"}}"#, "is not a property"),
            (r#"{"type":{"control":"T"}}"#, "`type` needs `text`"),
            (r#"{"click":{},"wait":{"ms":1}}"#, "one key"),
        ] {
            let cmd = serde_json::json!({ "cmd": "script", "steps": [serde_json::from_str::<serde_json::Value>(bad).unwrap()] });
            let e = steps::parse(&cmd).unwrap_err();
            assert!(e.starts_with("step 1: ") && e.contains(why), "{bad}: {e}");
        }
        assert!(steps::parse(&serde_json::json!({ "cmd": "script" })).is_err());

        // WASM-008 T47: the Preview's two roles, and the `page` step's fixed list of window properties.
        for role in ["preview-errors", "phone-frame"] {
            let s = steps::parse_step(&serde_json::json!({ "text": { "control": role } })).unwrap();
            assert_eq!(s, serde_json::json!({ "op": "text", "target": role }));
            assert!(steps::RUNNER.contains(&format!("t === '{role}'")), "{role} is found by the runner");
        }
        for p in steps::PAGE_PROPERTIES {
            let s = steps::parse_step(&serde_json::json!({ "page": { "property": p } })).unwrap();
            assert_eq!(s, serde_json::json!({ "op": "page", "property": p }));
            assert!(steps::RUNNER.contains(&format!("case '{p}'")), "{p} is read by the runner");
        }
        let framed = steps::parse_step(&serde_json::json!({ "page": { "property": "crossOriginIsolated", "frame": "phone-frame" } })).unwrap();
        assert_eq!(framed, serde_json::json!({ "op": "page", "property": "crossOriginIsolated", "frame": "phone-frame" }));
        for (bad, why) in [
            (serde_json::json!({ "page": {} }), "`page` needs `property`"),
            (serde_json::json!({ "page": { "property": "document.cookie" } }), "is not a page property"),
            (serde_json::json!({ "page": { "property": "localStorage" } }), "is not a page property"),
            (serde_json::json!({ "page": { "property": "innerWidth", "frame": "form" } }), "`frame` is one of phone-frame"),
            (serde_json::json!({ "page": { "property": "innerWidth", "frame": 1 } }), "`frame` is one of phone-frame"),
        ] {
            let e = steps::parse_step(&bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
        }
        // The runner answers under its own event, and defines itself once, unreplaceably.
        assert!(steps::RUNNER.contains(steps::RUNNER_EVENT));
        assert!(steps::RUNNER.contains("writable: false, configurable: false"));
        for gone in ["eval(", "new Function", "innerHTML ="] {
            assert!(!steps::RUNNER.contains(gone), "{gone}");
        }
    }

    #[test]
    fn a_png_header_gives_its_pixels() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&1170u32.to_be_bytes());
        png.extend_from_slice(&2532u32.to_be_bytes());
        assert_eq!(steps::png_size(&png), Some((1170, 2532)));
        assert_eq!(steps::png_size(b"GIF89a................."), None);
        assert_eq!(steps::png_size(&png[..20]), None);
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
        for needle in ["'__prc_' + key", "stored('dpr')", "stored('locale')", "stored('safe')", "--pr-safe-top"] {
            assert!(INIT_SCRIPT.contains(needle), "{needle}");
        }
        // The capture is the served page's (one script for the browser and here).
        for gone in ["window.ipc.postMessage", "unhandledrejection", "console[level]"] {
            assert!(!INIT_SCRIPT.contains(gone), "{gone}");
        }
    }
}
