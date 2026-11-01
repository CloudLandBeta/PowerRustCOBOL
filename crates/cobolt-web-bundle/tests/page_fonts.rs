// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A project's font in a real browser** (WASM-004 T24, Q5). The fixture's output Label names `Brand Sans`, a family the
//! project supplies as `Assets/BrandSans.ttf` (the stand-in face's bytes under another name, so that its width is known):
//! headless Chrome reports the Label's computed `font-family` with that family first, the page's face of that family
//! loaded, and the Label's text exactly as wide as the same text in the stand-in face — and not as wide as in the
//! system's sans-serif, which is what a fallback would letter it in.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, AssetInput, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

const UBUNTU_LIGHT: &[u8] = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");
const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot;";

#[test]
fn page_fonts_a_label_is_lettered_in_the_bundled_family() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        let tag = format!("<Control id=\"{LBL_OUT}\"");
        let at = f.cfrm.find(&tag).expect("the fixture has the output label");
        let close = at + f.cfrm[at..].find("></Control>").unwrap() + 1;
        f.cfrm.insert_str(close, "<Property name=\"FontName\">Brand Sans</Property><Property name=\"FontSize\">20</Property>");
    }
    input.assets = vec![AssetInput { path: "Assets/BrandSans.ttf".into(), bytes: UBUNTU_LIGHT.to_vec() }];
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = root.querySelector('[data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}; console {:?}", page.console));
    page.eval("document.fonts.ready.then(() => true)").unwrap();
    let r = page
        .eval(&format!(
            "(() => {{ {LIB}
              const l = root.querySelector('[data-name=\"{LBL_OUT}\"]');
              const cs = getComputedStyle(l);
              const faces = [...document.fonts].map(f => [f.family.replace(/\"/g, ''), f.status]);
              const width = family => {{ const s = document.createElement('span'); s.style.font = cs.fontSize + ' ' + family; s.style.position = 'absolute'; s.style.whiteSpace = 'pre'; s.textContent = l.textContent; root.appendChild(s); const w = s.getBoundingClientRect().width; s.remove(); return w; }};
              return {{ family: cs.fontFamily, size: cs.fontSize, faces,
                        brand: width('\"Brand Sans\"'), stand_in: width('\"PR Ubuntu Light\"'), system: width('system-ui, sans-serif') }};
            }})()"
        ))
        .unwrap();
    println!("Chrome ({}): Label {LBL_OUT} font-family {} at {}", exe.display(), r["family"], r["size"]);
    println!("  page faces {}", r["faces"]);
    println!("  text width: Brand Sans {} px, stand-in {} px, system sans-serif {} px", r["brand"], r["stand_in"], r["system"]);
    assert!(r["family"].as_str().unwrap().starts_with("\"Brand Sans\""), "the form's family first: {}", r["family"]);
    let faces = r["faces"].as_array().unwrap();
    assert!(faces.iter().any(|f| f[0] == "Brand Sans" && f[1] == "loaded"), "the bundled face was loaded: {faces:?}");
    let (brand, stand_in, system) = (r["brand"].as_f64().unwrap(), r["stand_in"].as_f64().unwrap(), r["system"].as_f64().unwrap());
    assert!((brand - stand_in).abs() < 0.01, "lettered in the bundled file's face");
    assert!((brand - system).abs() > 0.5, "not in the fallback");
    drop(server);
}
