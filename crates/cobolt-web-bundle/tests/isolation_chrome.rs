// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Embedding, isolation headers and cross-origin resources, in a real browser** (WASM-017 T13; AC14, AC15).
//!
//! * AC14 — the application mounted in a `<div>` of a host page (WASM-002 R11): a host page that is **not**
//!   cross-origin isolated gets the plain player on the main thread and works; a host page served with the
//!   isolation headers gets the threaded player and the module in a Worker.
//! * AC15 — the headers of each documented static host (`_headers`, nginx, `.htaccess`) carry what
//!   `security_headers` says for each isolation setting (no browser needed); a host that sends no headers
//!   becomes isolated through the service-worker shim (Q10: offered, off by default); and a cross-origin
//!   image, font and map tile load under `credentialless`, under `require-corp` with CORP, and with CORS —
//!   and fail, with a diagnostic the browser itself prints, otherwise.
//!
//! Chrome is the only engine installed here: Firefox, Safari (macOS and iOS) and the Android WebView are not
//! run; the commands for them are in `specs/WASM-PROGRESS.md`. With no Chrome the browser test says it was
//! skipped (`CHROME` names one).

mod support;

use std::path::PathBuf;
use std::time::Instant;

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{BundleOptions, BundleOutput, Embed, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const HELPERS: &str = r#"
window.__pr = {
  app() { return document.getElementById('pr-app'); },
  root() { const a = this.app(); return a && a.shadowRoot; },
  text(form, name) { const r = this.root(); const e = r && r.querySelector(`[data-pr-form="${form}"] [data-name="${name}"]`); return e ? e.textContent : null; },
  handle() { return PowerRustCOBOL.mount(this.app()); },
};
true
"#;

fn shipped_player() -> PlayerInput {
    let dir: PathBuf = support::repo_root().join("assets/web-player");
    PlayerInput {
        plain: Some(support::player_variant(&dir, "plain")),
        threaded: Some(support::player_variant(&dir, "threaded")),
        parts: support::player_parts(&dir),
        ..Default::default()
    }
}

fn bundle(isolation: Isolation, embed: Embed, shim: bool) -> BundleOutput {
    support::assemble(shipped_player(), BundleOptions { isolation, embed, service_worker_shim: shim, ..Default::default() })
}

/// `index.html` mounted in a `<div>` of a page that has its own content.
fn embedding_page(index: &str) -> String {
    let at = index.find("<div id=\"pr-app\"").expect("the host page has #pr-app");
    let end = at + index[at..].find("</div>").expect("closing div") + "</div>".len();
    format!("{}<p id=\"before\">The host page's own text.</p>\n<div id=\"host-box\">{}</div>\n<p id=\"after\">More of the host page.</p>{}", &index[..at], &index[at..end], &index[end..])
}

#[test]
fn each_documented_host_carries_the_headers_of_its_isolation_setting() {
    let mut rows = Vec::new();
    for (name, isolation, coep) in [("off", Isolation::Off, None), ("require-corp", Isolation::RequireCorp, Some("require-corp")), ("credentialless", Isolation::Credentialless, Some("credentialless"))] {
        let out = bundle(isolation, Embed::Window, false);
        for host in ["_headers", "hosting/nginx.conf", "hosting/.htaccess"] {
            let text = out.text(host).unwrap_or_else(|| panic!("{host} is missing for isolation {name}"));
            match coep {
                None => assert!(!text.contains("Cross-Origin-"), "{host} ({name}) must carry no isolation header:\n{text}"),
                Some(v) => {
                    // each host writes the header in its own syntax
                    let (opener, embedder, resource) = match host {
                        "_headers" => ("  Cross-Origin-Opener-Policy: same-origin".to_owned(), format!("  Cross-Origin-Embedder-Policy: {v}"), "  Cross-Origin-Resource-Policy: same-origin".to_owned()),
                        "hosting/nginx.conf" => ("add_header Cross-Origin-Opener-Policy \"same-origin\" always;".to_owned(), format!("add_header Cross-Origin-Embedder-Policy \"{v}\" always;"), "add_header Cross-Origin-Resource-Policy \"same-origin\" always;".to_owned()),
                        _ => ("Header always set Cross-Origin-Opener-Policy \"same-origin\"".to_owned(), format!("Header always set Cross-Origin-Embedder-Policy \"{v}\""), "Header always set Cross-Origin-Resource-Policy \"same-origin\"".to_owned()),
                    };
                    for line in [&opener, &embedder, &resource] {
                        assert!(text.contains(line.as_str()), "{host} ({name}) lacks `{line}`:\n{text}");
                    }
                }
            }
            assert!(text.contains("application/wasm"), "{host} ({name}) must serve .wasm as application/wasm");
            rows.push(format!("{name:<15} {host:<20} ok"));
        }
    }
    println!("isolation headers per documented host: {} hosts x settings checked\n  {}", rows.len(), rows.join("\n  "));
    assert_eq!(rows.len(), 9);
}

#[test]
fn the_shim_page_names_its_trusted_types_policy_and_a_page_without_the_shim_does_not() {
    for (shim, want) in [(true, true), (false, false)] {
        let out = bundle(Isolation::RequireCorp, Embed::Window, shim);
        let index = out.text("index.html").expect("index.html");
        assert!(index.contains("trusted-types default"), "the policy is always there");
        assert_eq!(index.contains("trusted-types default pr-coi"), want, "shim {shim}: the registration script's policy is named in the page's policy only when the shim is on");
        let reg = out.files.iter().find(|f| f.path.starts_with("coi-register.")).map(|f| String::from_utf8_lossy(&f.bytes).into_owned());
        assert_eq!(reg.is_some(), shim);
        if let Some(r) = reg {
            assert!(r.contains("createPolicy('pr-coi'") && r.contains("serviceWorker.ready"), "the script makes a TrustedScriptURL and waits for the worker to be active");
        }
    }
}

type Outcome = Result<String, String>;

fn choice(p: &mut Page) -> Result<(String, bool), String> {
    let c = p.eval("PowerRustCOBOL.mount(document.getElementById('pr-app')).then(a => a.choice.player + '/' + a.choice.mode)")?;
    let iso = p.eval("self.crossOriginIsolated")?;
    Ok((c.as_str().unwrap_or("?").to_owned(), iso == serde_json::json!(true)))
}

fn embedded(chrome: &Chrome, isolation: Isolation) -> Outcome {
    let out = bundle(isolation, Embed::Element { selector: "#pr-app".into() }, false);
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    support::write_bundle(&out, dir.path());
    let index = std::fs::read_to_string(dir.path().join("index.html")).map_err(|e| e.to_string())?;
    std::fs::write(dir.path().join("embed.html"), embedding_page(&index)).map_err(|e| e.to_string())?;
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation, ..Default::default() }).map_err(|e| e.to_string())?;
    let mut p = chrome.new_page()?;
    p.resize(1024, 768)?;
    p.navigate(&format!("{}/embed.html", server.url()))?;
    p.eval(HELPERS)?;
    p.wait_for("__pr.handle().then(a => Object.keys(JSON.parse(a.player.snapshot())).length > 0)", 40)?;
    let (c, iso) = choice(&mut p)?;
    let mut sort = String::new();
    if iso {
        // the player the page chose, started once more here so that its Rayon sort can be called: 50 000 rows
        let r = p.eval(
            "PowerRustCOBOL.mount(document.getElementById('pr-app')).then(async a => { const pv = a.config.player[a.choice.player]; \
             const m = await import(new URL(pv.js, location.href).href + '?probe'); \
             await m.default({ module_or_path: new URL(pv.wasm, location.href).href }); \
             await m.initThreadPool(Math.max(1, Math.min(8, navigator.hardwareConcurrency - 1))); \
             return m.sort_workers(50000); })",
        );
        sort = match r {
            Ok(v) => format!(", sort {}", v.as_str().unwrap_or("?")),
            Err(e) => format!(", sort FAILED: {e}"),
        };
    }
    let host_text = p.eval("document.getElementById('before').textContent + ' | ' + document.getElementById('after').textContent")?;
    server.stop();
    if !host_text.as_str().unwrap_or("").contains("host page's own text") {
        return Err("the host page's own content is gone".into());
    }
    Ok(format!("isolated={iso}, {c}{sort}"))
}

fn shim(chrome: &Chrome) -> Outcome {
    let out = bundle(Isolation::Off, Embed::Window, true);
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    support::write_bundle(&out, dir.path());
    // a host that sends no isolation header at all
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::Off, ..Default::default() }).map_err(|e| e.to_string())?;
    let mut p = chrome.new_page()?;
    p.call("Log.enable", serde_json::json!({}))?;
    p.call("Network.enable", serde_json::json!({}))?;
    p.resize(1024, 768)?;
    let t0 = Instant::now();
    p.navigate(&format!("{}/index.html", server.url()))?;
    // the shim registers its worker, which re-serves the page with the headers, and the page reloads itself once
    if let Err(e) = p.wait_for("self.crossOriginIsolated === true", 25) {
        let regs = p.eval("navigator.serviceWorker.getRegistrations().then(r => r.map(x => (x.active ? 'active' : x.installing ? 'installing' : x.waiting ? 'waiting' : 'none') + ' ' + x.scope).join('; ') || 'no registration')").map(|v| v.as_str().unwrap_or("").to_owned()).unwrap_or_default();
        return Err(format!("the shim never isolated the page: {e}; registrations: {regs}; console: {:?}; exceptions: {:?}; browser: {:?}", p.console, p.exceptions, p.browser_log));
    }
    let isolated_ms = t0.elapsed().as_millis();
    p.eval(HELPERS)?;
    p.wait_for("__pr.handle().then(a => Object.keys(JSON.parse(a.player.snapshot())).length > 0)", 40)?;
    let (c, iso) = choice(&mut p)?;
    server.stop();
    if !iso {
        return Err("not isolated after the shim".into());
    }
    Ok(format!("isolated after the shim in {isolated_ms} ms, {c}"))
}

/// One resource of another origin, loaded the way a page would: an image (a map tile is one), or a font.
fn load(p: &mut Page, kind: &str, url: &str, cors: bool) -> Result<String, String> {
    let js = match kind {
        "image" => format!(
            "new Promise(r => {{ const i = new Image(); {} i.onload = () => r('loaded'); i.onerror = () => r('blocked'); i.src = {url:?}; }})",
            if cors { "i.crossOrigin = 'anonymous';" } else { "" }
        ),
        _ => format!("new FontFace('x' + Math.random(), 'url({url})').load().then(() => 'loaded', () => 'blocked')"),
    };
    Ok(p.eval(&js)?.as_str().unwrap_or("?").to_owned())
}

fn cross_origin(chrome: &Chrome) -> Result<Vec<String>, String> {
    let assets = support::repo_root().join("assets/web-fonts/Ubuntu-Light.ttf");
    let png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44,
        0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    // three resource servers on other ports: no header, CORP: cross-origin, CORS
    let mut servers = Vec::new();
    let mut dirs = Vec::new();
    for headers in [vec![], vec![("Cross-Origin-Resource-Policy".to_owned(), "cross-origin".to_owned())], vec![("Access-Control-Allow-Origin".to_owned(), "*".to_owned())]] {
        let d = tempfile::tempdir().map_err(|e| e.to_string())?;
        std::fs::write(d.path().join("tile.png"), png).map_err(|e| e.to_string())?;
        std::fs::copy(&assets, d.path().join("font.ttf")).map_err(|e| e.to_string())?;
        servers.push(serve(d.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::Off, extra_headers: headers, ..Default::default() }).map_err(|e| e.to_string())?);
        dirs.push(d);
    }
    let (plain, corp, cors) = (servers[0].url(), servers[1].url(), servers[2].url());
    let mut lines = Vec::new();
    for (setting, isolation) in [("credentialless", Isolation::Credentialless), ("require-corp", Isolation::RequireCorp)] {
        let page_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        std::fs::write(page_dir.path().join("probe.html"), "<!doctype html><title>probe</title><p>probe</p>").map_err(|e| e.to_string())?;
        let host = serve(page_dir.path(), "127.0.0.1:0", ServeOptions { isolation, ..Default::default() }).map_err(|e| e.to_string())?;
        let mut p = chrome.new_page()?;
        p.call("Log.enable", serde_json::json!({}))?;
        p.call("Network.enable", serde_json::json!({}))?;
        p.navigate(&format!("{}/probe.html", host.url()))?;
        if p.eval("self.crossOriginIsolated")? != serde_json::json!(true) {
            return Err(format!("the probe page is not isolated under {setting}"));
        }
        // (what is loaded, from which server, with CORS mode, expected under credentialless, expected under require-corp)
        let cases: [(&str, &str, &str, bool, &str, &str); 8] = [
            ("image", "tile.png", "no header", false, "loaded", "blocked"),
            ("image", "tile.png", "CORP cross-origin", false, "loaded", "loaded"),
            ("image", "tile.png", "CORS", true, "loaded", "loaded"),
            ("image", "tile.png", "no header, CORS mode", true, "blocked", "blocked"),
            ("font", "font.ttf", "CORS", true, "loaded", "loaded"),
            ("font", "font.ttf", "no header", true, "blocked", "blocked"),
            ("font", "font.ttf", "CORP cross-origin only", true, "blocked", "blocked"),
            ("image", "tile.png", "CORP cross-origin, CORS mode", true, "blocked", "blocked"),
        ];
        for (kind, file, from, cors_mode, under_cl, under_rc) in cases {
            let base = if from.starts_with("CORP") { &corp } else if from.starts_with("CORS") { &cors } else { &plain };
            let want = if setting == "credentialless" { under_cl } else { under_rc };
            let before = p.browser_log.len();
            let got = load(&mut p, kind, &format!("{base}/{file}"), cors_mode)?;
            // the browser reports a block a moment after the page sees it
            p.eval("new Promise(r => setTimeout(r, 200))")?;
            let diag: Vec<String> = p.browser_log[before..].iter().filter(|l| !l.contains("favicon")).cloned().collect();
            if got != want {
                return Err(format!("{setting}: {kind} from `{from}` was {got}, expected {want}; browser said {diag:?}"));
            }
            let note = if got == "blocked" { format!(" — browser: {}", diag.first().map(String::as_str).unwrap_or("(no message)")) } else { String::new() };
            lines.push(format!("{setting:<14} {kind:<5} {from:<30} {got}{note}"));
            if got == "blocked" && diag.is_empty() {
                return Err(format!("{setting}: {kind} from `{from}` was blocked without any diagnostic from the browser"));
            }
        }
        host.stop();
    }
    for s in servers {
        s.stop();
    }
    Ok(lines)
}

#[test]
fn embedding_isolation_and_cross_origin_resources_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut report = Vec::new();
    let mut failed = Vec::new();
    fn note(report: &mut Vec<String>, failed: &mut Vec<String>, name: &str, r: Result<String, String>) {
        match r {
            Ok(d) => report.push(format!("  pass  {name}: {d}")),
            Err(e) => {
                report.push(format!("  FAIL  {name}: {e}"));
                failed.push(format!("{name}: {e}"));
            }
        }
    }
    let want = |r: Result<String, String>, expect: &str| r.and_then(|d| if d.contains(expect) { Ok(d) } else { Err(format!("expected `{expect}`, got {d}")) });
    let r = want(embedded(&chrome, Isolation::Off), "isolated=false, plain/main");
    note(&mut report, &mut failed, "AC14 embedded in a host page without isolation headers", r);
    let r = want(embedded(&chrome, Isolation::RequireCorp), "isolated=true, threaded/worker");
    note(&mut report, &mut failed, "AC14 embedded in an isolated host page", r);
    note(&mut report, &mut failed, "AC15 a host without headers, through the service-worker shim", shim(&chrome));
    match cross_origin(&chrome) {
        Ok(lines) => {
            for l in lines {
                report.push(format!("  pass  AC15 {l}"));
            }
        }
        Err(e) => {
            report.push(format!("  FAIL  AC15 cross-origin resources: {e}"));
            failed.push(format!("cross-origin resources: {e}"));
        }
    }
    println!("Chrome ({}): {} failed\n{}", exe.display(), failed.len(), report.join("\n"));
    assert!(failed.is_empty(), "failed in Chrome:\n{}", failed.join("\n"));
}
