// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The loading effect, in a real browser** (2.0.412): the barber-shop stripes of a control whose `LoadingEffect` is on.
//!
//! The effect (2.0.320) never showed. The player draws into a shadow root, and its rule read `body[data-pr-busy] .pr-loadfx`:
//! inside a shadow root a selector cannot see the page's `<body>`, so no marked control ever striped, whatever the Backend was
//! doing. Since 2.0.412 the player marks its own root (`.pr-root`) while a call is in flight and the rule reads
//! `[data-pr-busy] .pr-loadfx`. A unit test cannot see this: only the browser decides what a selector reaches.
//!
//! The fixture program's main form is given a marked card and an unmarked one. A headless Chrome sets and takes away the busy
//! mark the way the player does and asks the browser what each card's `::after` looks like: the marked card's stripes appear
//! with the mark and go with it, the unmarked card never has any, and a mark on the page's `<body>` alone (what 2.0.320
//! set) reaches nothing.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

/// The fixture's main form with two cards beside its own controls: one marked `LoadingEffect`, one not.
fn cards_form(plain: &str) -> String {
    let card = |id: &str, y: i32, marked: bool| {
        let mark = if marked { "<Property name=\"LoadingEffect\">true</Property>" } else { "" };
        format!("  <Control id=\"{id}\" type=\"Panel\" x=\"300\" y=\"{y}\" w=\"260\" h=\"120\">{mark}</Control>\n")
    };
    plain.replace("</Form>", &format!("{}{}</Form>", card("CARD-MARKED", 20, true), card("CARD-PLAIN", 180, false)))
}

fn open_cards(chrome: &Chrome) -> (Page, Server) {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = cards_form(&f.cfrm);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    (page, server)
}

/// What the browser says about a card's stripes: the opacity of its `::after`, its animation, and whether the card has the class.
const LOOK: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const card = id => root.querySelector('[data-name="' + id + '"]');
  const look = id => {
    const c = card(id), a = getComputedStyle(c, '::after');
    return { marked: c.classList.contains('pr-loadfx'), content: a.content, opacity: a.opacity, animation: a.animationName };
  };
  const busyOn = (el, name) => el.setAttribute('data-pr-busy', name === undefined ? 'moving-stripes' : name);
  const busyOff = el => el.removeAttribute('data-pr-busy');
  const pr = root.querySelector('.pr-root');
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LOOK} {body} }})()")
}

#[test]
fn a_marked_card_shows_its_stripes_while_the_player_is_busy_and_nothing_else_does_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, _server) = open_cards(&chrome);

    // At rest: the marked card has the class and an invisible overlay; the other has neither the class nor an overlay.
    let rest = page.eval(&js("return { marked: look('CARD-MARKED'), plain: look('CARD-PLAIN'), root: !!pr, busy: pr.hasAttribute('data-pr-busy') };")).unwrap();
    assert_eq!(rest["root"], true, "the player's root is in the shadow root");
    assert_eq!(rest["busy"], false, "{rest}");
    assert_eq!(rest["marked"]["marked"], true, "LoadingEffect puts pr-loadfx on the card: {rest}");
    assert_eq!(rest["marked"]["opacity"], "0", "{rest}");
    assert_eq!(rest["plain"]["marked"], false, "{rest}");

    // What 2.0.320 did: the mark on the page's <body> alone. The shadow root's styles cannot see it.
    page.eval(&js("busyOn(document.body); return true;")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    let body_only = page.eval(&js("return look('CARD-MARKED');")).unwrap();
    assert_eq!(body_only["opacity"], "0", "a mark on <body> must not be what the stripes wait for: {body_only}");
    page.eval(&js("busyOff(document.body); return true;")).unwrap();

    // Since 2.0.413 the mark's value is the project's effect: `none`, an effect nothing draws and a bare mark show nothing.
    let mut quiet = Vec::new();
    for name in ["none", "confetti", ""] {
        page.eval(&js(&format!("busyOn(pr, {name:?}); return true;"))).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let look = page.eval(&js("return look('CARD-MARKED');")).unwrap();
        assert_eq!(look["opacity"], "0", "a busy mark naming {name:?} must show nothing: {look}");
        assert_eq!(look["animation"], "none", "{look}");
        quiet.push(format!("{name:?}"));
        page.eval(&js("busyOff(pr); return true;")).unwrap();
    }

    // What the player does since 2.0.412: the mark on its own root, naming the default effect. The stripes come (the opacity eases in over .15 s)...
    page.eval(&js("busyOn(pr); return true;")).unwrap();
    let on = page
        .wait_for(&js("const l = look('CARD-MARKED'); return l.opacity === '1' && l.animation === 'pr-barber';"), 10)
        .map(|_| page.eval(&js("return { marked: look('CARD-MARKED'), plain: look('CARD-PLAIN') };")).unwrap())
        .unwrap_or_else(|e| panic!("the marked card never showed its stripes with the player busy: {e}"));
    // ...the card that was not marked has none...
    assert_eq!(on["plain"]["content"], "none", "an unmarked card has no overlay: {on}");
    assert_eq!(on["plain"]["animation"], "none", "{on}");
    // ...and the answer takes them away.
    page.eval(&js("busyOff(pr); return true;")).unwrap();
    page.wait_for(&js("return look('CARD-MARKED').opacity === '0';"), 10).unwrap_or_else(|e| panic!("the stripes stayed after the answer: {e}"));
    println!(
        "loading effect in Chrome: at rest overlay opacity {}; busy mark on <body> only -> {}; marks naming {} -> 0; on the player's root naming moving-stripes -> {} with animation {}; unmarked card: overlay {}; mark taken away -> 0",
        rest["marked"]["opacity"], body_only["opacity"], quiet.join(", "), on["marked"]["opacity"], on["marked"]["animation"], on["plain"]["content"]
    );
}
