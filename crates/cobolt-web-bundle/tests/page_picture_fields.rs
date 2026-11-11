// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A box with a PICTURE takes what the user types and shows its edited form once the user has left it, in a real browser**
//! (2.0.431).
//!
//! Two halves of one rule, the desktop's (`render.rs`, the TextBox arm): a numeric-edited picture (`ZZ,ZZ9.99`,
//! `99/99/9999`, `99B99999B9999`) shows the plain value under the caret and the edited one at rest; a character picture
//! (`X(5)`, `A(10)`) has no other form and shows what it holds. Until 2.0.431 the Web player padded a character picture to
//! its width at all times: the box of an `X(5)` held five spaces before the first key, its `maxlength` of 5 was reached, and
//! **no key could be typed into it at all**. Only a browser shows it (the box's own `maxlength` does the refusing).
//!
//! A headless Chrome clicks into each box, types key by key, leaves with Tab and reads what the box shows; a button's
//! handler then reads `Text` back, which is what the program sees: the value, never the edited view. With no Chrome on the
//! machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const FORM: &str = "main-form";

/// One box: its name, its picture, what it shows before anything is typed (an edited picture shows the edited form of
/// zero, as on the desktop; every other box is empty), the keys pressed, what the box holds after them (a refused key
/// leaves no trace), and what it shows once left.
struct Case {
    id: &'static str,
    picture: &'static str,
    empty: &'static str,
    keys: &'static str,
    held: &'static str,
    at_rest: &'static str,
}

const CASES: &[Case] = &[
    // Numeric-edited: the plain value while typing, the edited form at rest.
    Case { id: "TXT-NUM", picture: "ZZ,ZZ9.99", empty: "     0.00", keys: "1234.5", held: "1234.5", at_rest: " 1,234.50" },
    Case { id: "TXT-PHONE", picture: "99B99999B9999", empty: "00 00000 0000", keys: "21964326264", held: "21964326264", at_rest: "21 96432 6264" },
    Case { id: "TXT-DATE", picture: "99/99/9999", empty: "00/00/0000", keys: "11112026", held: "11112026", at_rest: "11/11/2026" },
    // A number with no editing symbol: digits only, shown as typed. The letter is refused.
    Case { id: "TXT-DIGITS", picture: "9(4)", empty: "", keys: "12a34", held: "1234", at_rest: "1234" },
    // Character pictures: what is typed is what is shown; the sixth character of an X(5) and the digit of an A(10) are refused.
    Case { id: "TXT-CHAR", picture: "X(5)", empty: "", keys: "abcdef", held: "abcde", at_rest: "abcde" },
    Case { id: "TXT-ALPHA", picture: "A(10)", empty: "", keys: "Ana 1Lu", held: "Ana Lu", at_rest: "Ana Lu" },
    // No picture: the box is what it always was.
    Case { id: "TXT-PLAIN", picture: "", empty: "", keys: "hello", held: "hello", at_rest: "hello" },
];

/// The button whose handler reads `Text` of `box_id` into `LBL-OUT`.
fn reader(box_id: &str) -> String {
    format!("BTN-{}", box_id.trim_start_matches("TXT-"))
}

fn spec(multi: bool) -> FixtureSpec {
    let mut controls = vec![("LBL-OUT".to_owned(), "Label".to_owned())];
    let mut handlers = vec![FixtureHandler::form("onLoad", vec![Step::Set { target: Target::control("LBL-OUT"), name: "Caption".into(), value: "loaded".into() }])];
    for c in CASES {
        controls.push((c.id.to_owned(), "TextBox".to_owned()));
    }
    for c in CASES {
        controls.push((reader(c.id), "Button".to_owned()));
        handlers.push(FixtureHandler::on(
            &reader(c.id),
            "onClick",
            vec![Step::Get { target: Target::control(c.id), name: "Text".into() }, Step::EchoAnswer { target: Target::control("LBL-OUT"), name: "Caption".into() }],
        ));
    }
    FixtureSpec { project: "pictures".into(), multi_memory: multi, limits: Limits::default(), forms: vec![FixtureForm { name: FORM.into(), main: true, instances: 1, controls, handlers }] }
}

/// The fixture's form (one control under the other, 34 apart) made tall enough for all of them, the buttons moved to a second
/// column, and each box given its picture.
fn form_with_pictures(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    // `index-base="1"`: a form without it is read as one saved when indexes counted from 0.
    let mut xml = plain.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"560\" index-base=\"1\"");
    for (k, c) in CASES.iter().enumerate() {
        if !c.picture.is_empty() {
            let open = format!("<Control id=\"{}\" type=\"TextBox\"", c.id);
            let at = xml.find(&open).unwrap_or_else(|| panic!("{} is in the fixture's form", c.id));
            let close = at + xml[at..].find("></Control>").expect("the control's end") + 1;
            xml.insert_str(close, &format!("<Property name=\"Picture\">{}</Property>", c.picture));
        }
        // The button beside its box.
        let open = format!("<Control id=\"{}\" type=\"Button\" x=\"10\" y=\"", reader(c.id));
        let at = xml.find(&open).unwrap_or_else(|| panic!("{} is in the fixture's form", reader(c.id)));
        let y_at = at + open.len();
        let y_end = y_at + xml[y_at..].find('"').expect("the y value's end");
        xml.replace_range(y_at..y_end, &(10 + (k + 1) * 34).to_string());
        xml.replace_range(at..at + open.len(), &format!("<Control id=\"{}\" type=\"Button\" x=\"230\" y=\"", reader(c.id)));
    }
    xml
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const box = id => { const e = el(id); return e && e.querySelector('input'); };
  const caption = () => { const e = el('LBL-OUT'); return e ? e.textContent : null; };
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

fn shown(page: &mut Page, id: &str) -> String {
    page.eval(&js(&format!("return box('{id}').value;"))).unwrap().as_str().unwrap_or("?").to_owned()
}

/// Wait until box `id` shows `want`; a panic says what it shows instead.
fn wait_shown(page: &mut Page, id: &str, want: &str, when: &str) {
    let want_js = serde_json::to_string(want).unwrap();
    page.wait_for(&js(&format!("return box('{id}').value === {want_js};")), 10)
        .unwrap_or_else(|e| panic!("{id} {when}: it shows {:?}, not {want:?} ({e})", shown(page, id)));
}

/// Click into box `id`, as a user does.
fn click_into(page: &mut Page, id: &str) {
    page.click_at(&js(&format!("return rect(box('{id}'));"))).unwrap();
    page.wait_for(&js(&format!("return root.activeElement === box('{id}');")), 10).unwrap_or_else(|e| panic!("{id} did not take the focus: {e}"));
}

#[test]
fn a_box_with_a_picture_takes_the_keys_and_shows_its_edited_form_after_the_user_leaves_it() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let started = std::time::Instant::now();
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "pictures".into(),
        forms: vec![FormInput { name: FORM.into(), cfrm: form_with_pictures(&standard_cfrm(&multi, FORM)), stylesheet: None }],
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player: PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        options: BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    };
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(800, 640).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&js("return caption() === 'loaded';"), 30).unwrap_or_else(|e| panic!("the application did not start: {e}"));
    let browsing = std::time::Instant::now();

    let mut keys = 0;
    for c in CASES {
        // Before the first key: a character picture's box is empty (nothing of the picture's width stands in it), an
        // edited one shows the edited form of zero, which gives way to the empty value under the caret.
        assert_eq!(shown(&mut page, c.id), c.empty, "{} (PIC {}) before the first key", c.id, c.picture);
        click_into(&mut page, c.id);
        wait_shown(&mut page, c.id, "", &format!("(PIC {}) focused, before the first key", c.picture));
        for k in c.keys.chars() {
            page.press_key(&k.to_string()).unwrap();
            keys += 1;
        }
        // Under the caret: the plain value, with the keys the picture refuses left out.
        wait_shown(&mut page, c.id, c.held, &format!("(PIC {}) after the keys {:?}", c.picture, c.keys));
        page.press_key("Tab").unwrap();
        wait_shown(&mut page, c.id, c.at_rest, &format!("(PIC {}) once left", c.picture));
        // What the program reads is the value, never the edited view.
        page.click_at(&js(&format!("return rect(el('{}'));", reader(c.id)))).unwrap();
        let held_js = serde_json::to_string(c.held).unwrap();
        page.wait_for(&js(&format!("return caption() === {held_js};")), 10)
            .unwrap_or_else(|e| panic!("the program read {:?} from {}, not {:?} ({e})", page.eval(&js("return caption();")).unwrap(), c.id, c.held));
        // The click on the button left the box at rest: it still shows its edited form.
        wait_shown(&mut page, c.id, c.at_rest, &format!("(PIC {}) after the program read it", c.picture));
    }
    // Back into the edited box: the plain value is offered to edit again, and leaving restores the edited form.
    click_into(&mut page, "TXT-NUM");
    wait_shown(&mut page, "TXT-NUM", "1234.5", "focused again");
    page.press_key("Tab").unwrap();
    wait_shown(&mut page, "TXT-NUM", " 1,234.50", "left again");

    // A refused key leaves the caret where it was: with the caret put before "Ana Lu", the digit is refused and the
    // letter that follows goes in at the start, not at the end the browser sends a caret to when a field is rewritten.
    click_into(&mut page, "TXT-ALPHA");
    page.eval(&js("box('TXT-ALPHA').setSelectionRange(0, 0); return true;")).unwrap();
    page.press_key("7").unwrap();
    page.press_key("Z").unwrap();
    wait_shown(&mut page, "TXT-ALPHA", "ZAna Lu", "a refused digit, then a letter, typed at the start");

    println!("== page_picture_fields: a box with a PICTURE in a real browser (headless Chrome) ==");
    for c in CASES {
        println!("  {:<10} PIC {:<14} keys {:<14} held {:<14} at rest {:?}", c.id, if c.picture.is_empty() { "(none)" } else { c.picture }, format!("{:?}", c.keys), format!("{:?}", c.held), c.at_rest);
    }
    println!(
        "  {} boxes, {} keys pressed, {} checks; set-up {} ms, in the browser {} ms; {} passed, 0 failed",
        CASES.len(),
        keys + 2,
        CASES.len() * 6 + 3,
        browsing.duration_since(started).as_millis(),
        browsing.elapsed().as_millis(),
        CASES.len() * 6 + 3,
    );
    drop(server);
}
