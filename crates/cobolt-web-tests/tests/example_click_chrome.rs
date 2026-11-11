// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`examples/PowerWebHello`: the click from the page to the Backend and the greeting back in the label** (WASM-015
//! AC12, T22b). `examples.rs` builds the example, scans it, and calls its Backend's `HELLO` in process; it also loads
//! the page in the system web view and clicks with no name written. The step it could not run is the whole round trip,
//! because the kit's server speaks TLS and no web view trusts the kit's certificate.
//!
//! The page does not need the kit. A click is handled by the page (the `onClick` handler is the generated module); the
//! handler's `CALL "HELLO" WITH WEBSOCKET LINKAGE` is what talks to the Backend. So this test runs the two real halves
//! against each other, the way the Preview does for a developer:
//!
//! * the example is copied, and built by the real Build;
//! * the Backend is the example's own `backend/` folder, started by `cobolt_server::dev::DevServer` (the code behind
//!   `rcrun backend run`): plain `ws://` on a loopback port, a fresh development token, only the page's origin let in;
//! * the page is served by the Preview's server, whose `config.json` is told the Backend's address and token
//!   (`set_dev_backend`), as a Preview does;
//! * a headless Chrome opens the page, types a name in the box and clicks the button; the label must show the
//!   greeting the Backend answered, and the layer the handler shows must appear.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use std::path::{Path, PathBuf};

use cobolt_project_tools::web::devbackend;
use cobolt_project_tools::web::devserver::{self, DevBackend, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use cobolt_server::dev::{DevOptions, DevServer};

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("examples").join("PowerWebHello")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const form = root.querySelector('[data-pr-form=\"MAIN-FORM\"]'); \
    const layer = n => form && form.querySelector(':scope > .pr-form-layer[data-layer=\"' + n + '\"]'); \
    const shown = n => { const l = layer(n); return !!l && !l.hasAttribute('hidden') && getComputedStyle(l).display !== 'none'; }; \
    const text = id => { const e = el(id); return e ? e.textContent.trim() : null; };";

#[test]
fn the_pages_click_reaches_the_backend_and_the_greeting_comes_back() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let base = std::env::temp_dir().join(format!("prc-web-tests-example-click-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("PowerWebHello");
    copy_tree(&example_dir(), &folder);

    // The Preview's order (WASM-008 R6, WASM-013 R12): the page's port is chosen first so the Backend can be told the
    // page's origin; the Backend starts; the page is built for the address the Backend reports (the bundle's
    // `config.json` and the `connect-src` of its Content-Security-Policy are baked by the Build, and a page may connect
    // nowhere else); the page is served on the port chosen.
    let page_port = devbackend::free_loopback_port().expect("a free loopback port");
    let backend = DevServer::start(&DevOptions {
        backend_dir: folder.join("backend"),
        listen: Some("127.0.0.1:0".into()),
        page_origin: Some(devbackend::page_origin_of(page_port)),
        ..DevOptions::default()
    })
    .unwrap_or_else(|e| panic!("the example's Backend did not start: {e:?}"));
    let mut req = WebBuildRequest::from_manifest(&folder.join("PowerWebHello.project.toml")).unwrap_or_else(|e| panic!("{e}"));
    req.web.backend_url = backend.url.clone();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let backend_origin = backend.url.trim_end_matches("/ws");
    assert!(report.connect_src.contains(backend_origin), "the page was built to connect to the Backend {backend_origin}: {}", report.connect_src);
    let page_server = devserver::start(&report.bundle_dir, &ServeOptions { port: page_port, isolation: req.web.isolation, live_reload: false }).unwrap();
    page_server.set_dev_backend(Some(DevBackend { url: backend.url.clone(), token: backend.dev_token("tester", &[]) }));

    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1200, 700).unwrap();
    page.navigate(&page_server.url()).unwrap_or_else(|e| panic!("{e}"));

    let backend_log = || backend.log_lines().join("\n");
    // What a failed wait reports: the box, the label, the `config.json` this page was served.
    let wait = |page: &mut cdp::Page, cond: &str, what: &str| {
        if let Err(e) = page.wait_for(&format!("(() => {{ {LIB} return {cond}; }})()"), 30) {
            let seen = page
                .eval(&format!("(async () => {{ {LIB} const t = el('TXT-NAME'); const i = t && t.querySelector('input'); const c = JSON.parse(await fetch('config.json').then(r => r.text())); if (c.backend) c.backend.dev_token = '…'; return JSON.stringify({{ box: i ? i.value : null, result: text('LBL-RESULT'), backend: c.backend, forms: [...root.querySelectorAll('[data-pr-form]')].map(f => f.getAttribute('data-pr-form')), layers: [...root.querySelectorAll('.pr-form-layer')].map(l => l.outerHTML.slice(0, 160)), shown: root.textContent.replace(/\\s+/g, ' ').slice(0, 500) }}); }})()"))
                .map(|v| v.to_string())
                .unwrap_or_default();
            panic!("{what}: {e}; page {seen}; console {:?}; exceptions {:?}; the Backend's log:\n{}", page.console, page.exceptions, backend_log());
        }
    };
    let value = |page: &mut cdp::Page, body: &str| page.eval(&format!("(() => {{ {LIB} {body} }})()")).unwrap_or_else(|e| panic!("{e}"));

    wait(&mut page, "!!el('BTN-GREET') && !!el('TXT-NAME') && text('LBL-RESULT') === ''", "the page's form did not appear");
    assert_eq!(value(&mut page, "return !!layer('LAYER-DONE');").as_bool(), Some(true), "the page has the layer the handler shows");
    assert!(!value(&mut page, "return shown('LAYER-DONE');").as_bool().unwrap_or(true), "the layer the handler shows starts hidden");

    // The name goes in the box, the button is clicked: the handler's CALL goes to the Backend and the greeting comes back.
    page.click_at(&format!("(() => {{ {LIB} const r = el('TXT-NAME').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }})()")).unwrap();
    page.type_text("Ada").unwrap();
    page.click_at(&format!("(() => {{ {LIB} const r = el('BTN-GREET').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }})()")).unwrap();
    wait(&mut page, "text('LBL-RESULT') !== ''", "the greeting did not come back to the label");
    let greeting = value(&mut page, "return text('LBL-RESULT');").as_str().unwrap_or_default().to_owned();
    println!("  the label shows {greeting:?}");
    assert_eq!(greeting, "Hello, Ada", "the label shows the Backend's greeting; console {:?}; exceptions {:?}", page.console, page.exceptions);
    wait(&mut page, "shown('LAYER-DONE')", "the layer the handler shows did not appear");
    assert!(page.exceptions.is_empty(), "the page raised no exception: {:?}", page.exceptions);

    // The Backend served the call: its log is not empty and names the program.
    let log = backend_log();
    println!("  the Backend's log has {} line(s)", log.lines().count());
    assert!(log.contains("HELLO"), "the Backend's log names the HELLO call:\n{log}");

    drop(page);
    drop(chrome);
    backend.stop();
    drop(page_server);
    println!("PowerWebHello: the page's click reached the Backend and \"Hello, Ada\" came back to the label");
    println!("→ 1 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&base);
}
