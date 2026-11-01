// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Build's own hello-world in Chrome** (WASM-002 T19: AC1, AC2). A project made from the IDE's Web
//! template by the agents' tools (`create_project`, `open_project`, `check`, `build` — the tools `rcrun mcp`
//! serves) is built by the real Build, served by the development server and driven in a headless Chrome:
//!
//! - AC1: the page shows the form and answers a click — the template's handler asks for a name with the box
//!   empty, then greets the name typed;
//! - AC2: the same bundle mounted into one `<div>` of a page with content of its own fills only that `<div>`,
//!   follows its resize, and a computed style asserted on each side shows that neither page's CSS reaches the
//!   other.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use std::sync::Arc;

use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{self, WebBuildRequest};
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

/// The host page's own stylesheet: a sized box, and rules that would reach the application's button and a
/// `pr-` class if CSS leaked in, or be overridden if the application's leaked out.
const HOST_CSS: &str = "body { background: rgb(250, 240, 230); }\n\
#host-box { width: 420px; height: 300px; margin: 40px; border: 1px solid rgb(0, 0, 0); }\n\
button { background: rgb(255, 0, 0) !important; font-size: 40px !important; }\n\
p.pr-surface, p.pr-form { font-family: Georgia, serif; color: rgb(10, 20, 30); }\n";

/// The Build's `index.html` as one `<div>` of a page with content before and after it: the page leaves the
/// window-filling mode (`pr-window`) for the embedded one (`pr-embedded`), which is all `Embed::Element`
/// changes in the page (`cobolt-web-bundle` `lib.rs`, `body_class`/`app_class`).
fn embedding_page(index: &str) -> String {
    let index = index.replacen("<body class=\"pr-window\">", "<body>", 1);
    let at = index.find("<div id=\"pr-app\"").expect("the host page has #pr-app");
    let end = at + index[at..].find("</div>").expect("closing div") + "</div>".len();
    let app = index[at..end].replacen("class=\"pr-window\"", "class=\"pr-embedded\"", 1);
    assert!(app.contains("pr-embedded"), "{app}");
    let head = &index[..at];
    let head = head.replacen("</title>", "</title>\n<link rel=\"stylesheet\" href=\"host.css\">", 1);
    format!(
        "{head}<p id=\"before\" class=\"pr-surface\">The host page's own text.</p>\n<button id=\"host-button\">Host</button>\n\
         <div id=\"host-box\">{app}</div>\n<p id=\"after\">More of the host page.</p>{}",
        &index[end..]
    )
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; \
    const el = id => root && root.querySelector('[data-name=\"' + id + '\"]'); \
    const input = id => { const e = el(id); return e && (e.matches('input,textarea') ? e : e.querySelector('input,textarea')); }; \
    const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; }; \
    const caption = () => el('LBL-RESULT') ? el('LBL-RESULT').textContent : null;";

fn eval(page: &mut cdp::Page, body: &str) -> Value {
    page.eval(&format!("(() => {{ {LIB} {body} }})()"))
        .unwrap_or_else(|e| panic!("{e}; console {:?}; exceptions {:?}", page.console, page.exceptions))
}

fn wait(page: &mut cdp::Page, cond: &str, what: &str) {
    page.wait_for(&format!("(() => {{ {LIB} return {cond}; }})()"), 30)
        .unwrap_or_else(|e| panic!("{what}: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
}

/// AC1 on whatever page is open: an empty box asks for a name, a typed one is greeted.
fn greet(page: &mut cdp::Page, placement: &str) {
    wait(page, "!!el('BTN-GREET') && !!input('TXT-NAME') && caption() !== null", "the form was not drawn");
    page.click_at(&format!("(() => {{ {LIB} return rect(el('BTN-GREET')); }})()")).unwrap();
    wait(page, "caption() === 'Type your name first.'", "a click with the box empty did not ask for a name");
    page.click_at(&format!("(() => {{ {LIB} return rect(input('TXT-NAME')); }})()")).unwrap();
    page.type_text("Ada").unwrap();
    wait(page, "input('TXT-NAME').value === 'Ada'", "the box did not take the typed name");
    page.click_at(&format!("(() => {{ {LIB} return rect(el('BTN-GREET')); }})()")).unwrap();
    wait(page, "caption() === 'Hello, Ada'", "a click with \"Ada\" typed did not greet her");
    println!("AC1 ({placement}): empty box -> \"Type your name first.\"; \"Ada\" typed -> \"Hello, Ada\"");
}

#[test]
fn the_web_templates_build_answers_a_click_and_embeds_in_a_div_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };

    // ── The project, through the agents' tools: the Web template, opened, checked and built ──
    let base = std::env::temp_dir().join(format!("prc-hello-chrome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("Hello");
    let mut tools = ProjectTools::new(HeadlessHost::new(base.clone(), "test"));
    let made = tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "Hello", "template": "web"})).unwrap();
    let manifest = folder.join(made["project"].as_str().unwrap());
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    let checked = tools.call("check", &json!({})).unwrap();
    assert_eq!(checked["errors"], 0, "the Web template checks clean: {checked}");
    web::link(Arc::new(GeneratorBackend));
    let built = tools.call("build", &json!({})).unwrap();
    assert_eq!(built["status"], "built", "{built}");
    assert_eq!(built["engine"], "cobolt-wasm", "{built}");
    assert_eq!(built["report"]["budget"]["over"], false, "the size report carries the budget line (R8): {built}");
    println!(
        "build: status {}, engine {}, bundle {}, {} bytes in {} parts, variants {}, warnings {}",
        built["status"],
        built["engine"],
        built["bundle"],
        built["report"]["size"]["total_bytes"],
        built["report"]["size"]["parts"].as_array().map_or(0, Vec::len),
        built["variants"],
        built["report"]["warnings"]
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let bundle = folder.join(built["bundle"].as_str().unwrap());
    let index = std::fs::read_to_string(bundle.join("index.html")).unwrap();
    std::fs::write(bundle.join("embed.html"), embedding_page(&index)).unwrap();
    std::fs::write(bundle.join("host.css"), HOST_CSS).unwrap();

    let mut server =
        devserver::start(&bundle, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));

    // ── AC1: the page is the application, a click is answered ──
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    let version = page.call("Browser.getVersion", json!({})).map(|v| v["product"].as_str().unwrap_or("?").to_owned()).unwrap_or_else(|_| exe.display().to_string());
    page.resize(1100, 700).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    greet(&mut page, "window");
    drop(page);

    // ── AC2: the same bundle in one <div> of another page ──
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/embed.html", server.url())).unwrap_or_else(|e| panic!("{e}"));
    greet(&mut page, "div");

    let geometry = "const box = document.getElementById('host-box'); const b = box.getBoundingClientRect(); \
        const a = document.getElementById('pr-app').getBoundingClientRect(); \
        return { box: { x: b.x, y: b.y, w: b.width, h: b.height }, app: { x: a.x, y: a.y, w: a.width, h: a.height }, \
        contain: getComputedStyle(document.getElementById('pr-app')).contain, title_w: el('LBL-TITLE').getBoundingClientRect().width };";
    let fits = |g: &Value| {
        let n = |v: &Value| v.as_f64().unwrap_or(f64::NAN);
        // The box has a 1 px border (content-box sizing): the application fills its content box exactly.
        (n(&g["app"]["x"]) - n(&g["box"]["x"]) - 1.0).abs() <= 0.5
            && (n(&g["app"]["y"]) - n(&g["box"]["y"]) - 1.0).abs() <= 0.5
            && (n(&g["app"]["w"]) - (n(&g["box"]["w"]) - 2.0)).abs() <= 0.5
            && (n(&g["app"]["h"]) - (n(&g["box"]["h"]) - 2.0)).abs() <= 0.5
    };
    let g0 = eval(&mut page, geometry);
    println!("AC2 at 420x300: {g0}");
    assert!(fits(&g0), "the application fills its <div>: {g0}");
    assert!(g0["contain"].as_str().unwrap_or("").contains("paint"), "nothing paints past the <div>: {g0}");

    // Nothing of the application shows outside the box: the page's own colour beside and below it.
    let (bx, by, bw, bh) = (g0["box"]["x"].as_f64().unwrap(), g0["box"]["y"].as_f64().unwrap(), g0["box"]["w"].as_f64().unwrap(), g0["box"]["h"].as_f64().unwrap());
    for (what, x, y) in [("left of", bx - 6.0, by + bh / 2.0), ("right of", bx + bw + 6.0, by + bh / 2.0), ("below", bx + bw / 2.0, by + bh + 6.0)] {
        let px = page.capture(x, y, 1.0, 1.0).unwrap().2[0];
        assert_eq!(px, [250, 240, 230], "{what} the <div> the host page shows its own colour, not the application");
    }

    // The <div> is resized by the host page, not the window: the application follows it.
    eval(&mut page, "const b = document.getElementById('host-box'); b.style.width = '640px'; b.style.height = '420px'; return true;");
    page.wait_for("(() => { const a = document.getElementById('pr-app').getBoundingClientRect(); return Math.abs(a.width - 640) < 1 && Math.abs(a.height - 420) < 1; })()", 10)
        .unwrap_or_else(|e| panic!("the application did not follow its <div>: {e}"));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let g1 = eval(&mut page, geometry);
    println!("AC2 at 640x420: {g1}");
    assert!(fits(&g1), "the application follows its <div>: {g1}");
    assert!(
        g1["title_w"].as_f64().unwrap() > g0["title_w"].as_f64().unwrap(),
        "the form re-laid out to the wider <div> (LBL-TITLE is anchored left and right): {g0} -> {g1}"
    );

    // No CSS leaks, either way.
    let styles = eval(
        &mut page,
        "const s = e => { const c = getComputedStyle(e); return { bg: c.backgroundColor, size: c.fontSize, font: c.fontFamily, color: c.color }; }; \
         const btn = el('BTN-GREET'); const b = btn.matches('button') ? btn : (btn.querySelector('button') || btn); \
         return { app_button: s(b), host_button: s(document.getElementById('host-button')), host_p: s(document.getElementById('before')) };",
    );
    println!("AC2 styles: {styles}");
    assert_eq!(styles["host_button"]["bg"], "rgb(255, 0, 0)", "the host's rule applies to its own button");
    assert_ne!(styles["app_button"]["bg"], "rgb(255, 0, 0)", "the host's button rule does not reach the application's button");
    assert_ne!(styles["app_button"]["size"], "40px", "the host's font size does not reach the application's button");
    assert_eq!(styles["host_p"]["color"], "rgb(10, 20, 30)", "the application's .pr-surface rules do not reach the host's <p class=\"pr-surface\">");
    assert!(styles["host_p"]["font"].as_str().unwrap_or("").contains("Georgia"), "{styles}");

    println!("{version}: AC1 in the window and in a <div>, AC2 (fills, follows a resize to 640x420, no leak either way)");
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&base);
}
