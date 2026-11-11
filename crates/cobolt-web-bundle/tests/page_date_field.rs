// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A date field is typed into or picked from its calendar, in a real browser** (2.0.432).
//!
//! A DateTimePicker that edits a date alone is, on a page, a text box with a calendar button. A headless Chrome types a
//! date key by key and leaves the box; presses the button and reads the calendar the browser shows: that it is a modal
//! dialog in the top layer (a Panel that clips its contents does not clip it), where it stands, what its colours are and
//! how well they read, which days it marks as holy days for the page's locale (`pt-BR`: Brazil) and what it calls them;
//! jumps to another year and month with the two lists and the step buttons; presses a day, Today, Clear, Escape and the
//! page behind the calendar; and after each of them reads `Value` back through a handler of the program, which is ISO
//! whatever the field shows. With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const FORM: &str = "main-form";

fn reads_value(button: &str, field: &str) -> FixtureHandler {
    FixtureHandler::on(
        button,
        "onClick",
        vec![Step::Get { target: Target::control(field), name: "Value".into() }, Step::EchoAnswer { target: Target::control("LBL-OUT"), name: "Caption".into() }],
    )
}

fn spec(multi: bool) -> FixtureSpec {
    let control = |name: &str, kind: &str| (name.to_owned(), kind.to_owned());
    FixtureSpec {
        project: "datefield".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: FORM.into(),
            main: true,
            instances: 1,
            controls: vec![control("LBL-OUT", "Label"), control("PNL-1", "Panel"), control("DT-BR", "DateTimePicker"), control("DT-ISO", "DateTimePicker"), control("BTN-BR", "Button"), control("BTN-ISO", "Button")],
            handlers: vec![
                FixtureHandler::form("onLoad", vec![Step::Set { target: Target::control("LBL-OUT"), name: "Caption".into(), value: "loaded".into() }]),
                reads_value("BTN-BR", "DT-BR"),
                reads_value("BTN-ISO", "DT-ISO"),
            ],
        }],
    }
}

/// The fixture's form with `DT-BR` a white `dd/MM/yyyy` field inside a Panel barely larger than it (a Panel clips what is
/// inside it), `DT-ISO` a `Short` field with no ground of its own, and a button beside each.
fn form(plain: &str) -> String {
    let place = |xml: &str, id: &str, kind: &str, to: &str, inside: &str| {
        let open = format!("<Control id=\"{id}\" type=\"{kind}\" ");
        let at = xml.find(&open).unwrap_or_else(|| panic!("{id} is in the fixture's form"));
        let end = at + xml[at..].find("></Control>").expect("the control's end");
        format!("{}{open}{to}>{inside}</Control>{}", &xml[..at], &xml[end + "></Control>".len()..])
    };
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    // `index-base="1"`: a form without it is read as one saved when indexes counted from 0.
    let xml = plain.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"480\" index-base=\"1\"");
    let xml = place(&xml, "PNL-1", "Panel", "x=\"10\" y=\"50\" w=\"230\" h=\"44\"", "");
    let br = "<Property name=\"Format\">Custom</Property><Property name=\"CustomFormat\">dd/MM/yyyy</Property><Property name=\"BackgroundColor\">#FFFFFFFF</Property>\
              <Property name=\"ForegroundColor\">#1E1B4BFF</Property>";
    let xml = place(&xml, "DT-BR", "DateTimePicker", "x=\"18\" y=\"58\" w=\"200\" h=\"28\" parent=\"PNL-1\"", br);
    // A see-through ground is no ground to read a calendar on: this field's calendar takes the theme's surface and text.
    let xml = place(&xml, "DT-ISO", "DateTimePicker", "x=\"18\" y=\"110\" w=\"200\" h=\"28\"", "<Property name=\"BackgroundColor\">#00000000</Property>");
    let xml = place(&xml, "BTN-BR", "Button", "x=\"260\" y=\"58\" w=\"120\" h=\"28\"", "");
    place(&xml, "BTN-ISO", "Button", "x=\"260\" y=\"110\" w=\"120\" h=\"28\"", "")
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const box = id => el(id).querySelector('input');
  const cal = id => el(id).querySelector('dialog.pr-cal');
  const part = (id, name) => cal(id) && cal(id).querySelector('[data-part="' + name + '"]');
  const day = (id, date) => cal(id) && cal(id).querySelector('[data-part="day"][data-id="' + date + '"]');
  const caption = () => { const e = el('LBL-OUT'); return e ? e.textContent : null; };
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
  const lum = c => { const v = c.match(/[\d.]+/g).slice(0, 3).map(Number).map(x => { x /= 255; return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); }); return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2]; };
  const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)]; return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

fn text(page: &mut Page, body: &str) -> String {
    let v = page.eval(&js(body)).unwrap_or_else(|e| panic!("{e}"));
    v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())
}

fn until(page: &mut Page, body: &str, what: &str) {
    page.wait_for(&js(body), 10).unwrap_or_else(|e| panic!("{what} ({e})"));
}

fn press(page: &mut Page, element: &str) {
    page.click_at(&js(&format!("return rect({element});"))).unwrap_or_else(|e| panic!("a press on {element}: {e}"));
}

/// Press the button that reads `Value` of `field` and wait for the label to say `want`. When the label says it already (the
/// same value read twice running), the other field is read first, so that this read is seen to happen: the two fields never
/// hold the same value in this test.
fn reads(page: &mut Page, field: &str, want: &str, after: &str) {
    let want_js = serde_json::to_string(want).unwrap();
    let button = |f: &str| format!("el('BTN-{}')", f.trim_start_matches("DT-"));
    if page.eval(&js(&format!("return caption() === {want_js};"))).unwrap() == serde_json::json!(true) {
        press(page, &button(if field == "DT-BR" { "DT-ISO" } else { "DT-BR" }));
        page.wait_for(&js(&format!("return caption() !== {want_js};")), 10).unwrap_or_else(|e| panic!("after {after}, the other field read the same value {want:?} ({e})"));
    }
    press(page, &button(field));
    page.wait_for(&js(&format!("return caption() === {want_js};")), 10)
        .unwrap_or_else(|e| panic!("after {after}, the program read {:?} from {field}, not {want:?} ({e})", page.eval(&js("return caption();")).unwrap()));
}

fn open_calendar(page: &mut Page, field: &str) {
    press(page, &format!("el('{field}').querySelector('[data-part=\"open\"]')"));
    until(page, &format!("const d = cal('{field}'); return !!d && d.matches(':modal');"), &format!("the calendar of {field} did not open as a modal dialog"));
}

fn closed(page: &mut Page, field: &str, after: &str) {
    until(page, &format!("return !cal('{field}');"), &format!("the calendar of {field} is still there after {after}"));
}

/// Type `keys` over whatever the box of `field` holds, and leave it with Tab.
fn type_over(page: &mut Page, field: &str, keys: &str) {
    press(page, &format!("box('{field}')"));
    until(page, &format!("return root.activeElement === box('{field}');"), &format!("{field} did not take the focus"));
    page.eval(&js(&format!("box('{field}').select(); return true;"))).unwrap();
    for k in keys.chars() {
        page.press_key(&k.to_string()).unwrap();
    }
    page.press_key("Tab").unwrap();
}

#[test]
fn a_date_is_typed_or_picked_from_a_calendar_that_no_panel_clips_and_that_marks_the_countrys_holy_days() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let started = std::time::Instant::now();
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "datefield".into(),
        forms: vec![FormInput { name: FORM.into(), cfrm: form(&standard_cfrm(&multi, FORM)), stylesheet: None }],
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player: PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        options: BundleOptions { isolation: Isolation::RequireCorp, locale: "pt-BR".into(), ..Default::default() },
    };
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(900, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    until(&mut page, "return caption() === 'loaded';", "the application did not start");
    let browsing = std::time::Instant::now();
    let mut checks = 0;
    let mut check = |ok: bool, what: String| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // ── typed ───────────────────────────────────────────────────────────────
    // The empty field shows how its date is typed; digits alone are a date; the program reads ISO.
    check(text(&mut page, "return box('DT-BR').placeholder + '|' + box('DT-BR').value;") == "dd/MM/yyyy|", "the empty field shows its pattern as a hint".into());
    type_over(&mut page, "DT-BR", "25122026");
    until(&mut page, "return box('DT-BR').value === '25/12/2026';", "digits alone were not read as a date and shown in the field's format");
    reads(&mut page, "DT-BR", "2026-12-25", "typing 25122026");
    // With its separators, and a year of two digits.
    type_over(&mut page, "DT-BR", "3/11/26");
    until(&mut page, "return box('DT-BR').value === '03/11/2026';", "3/11/26 was not read as the 3rd of November 2026");
    reads(&mut page, "DT-BR", "2026-11-03", "typing 3/11/26");
    // What is no date changes nothing: the field goes back to the date it holds.
    type_over(&mut page, "DT-BR", "31/02/2026");
    until(&mut page, "return box('DT-BR').value === '03/11/2026';", "a day February does not have was kept in the field");
    reads(&mut page, "DT-BR", "2026-11-03", "typing the 31st of February");
    type_over(&mut page, "DT-BR", "25/12/2026");
    reads(&mut page, "DT-BR", "2026-12-25", "typing 25/12/2026");
    check(true, "four dates typed".into());

    // ── the calendar ────────────────────────────────────────────────────────
    open_calendar(&mut page, "DT-BR");
    check(text(&mut page, "return cal('DT-BR').getAttribute('aria-label');") == "dezembro 2026", "the calendar opens on the month of the date held, named in the page's language".into());
    check(text(&mut page, "return el('DT-BR').querySelector('[data-part=\"open\"]').getAttribute('aria-expanded');") == "true", "the button says the calendar is open".into());
    // Six weeks; the day held is filled and has the focus; Christmas is a holy day in Brazil, by name.
    check(text(&mut page, "return cal('DT-BR').querySelectorAll('[data-part=\"day\"]').length;") == "42", "six weeks of days".into());
    let held = text(&mut page, "const d = cal('DT-BR').querySelector('[aria-pressed=\"true\"]'); return d.dataset.id + '|' + d.className + '|' + d.title + '|' + (root.activeElement === d);");
    check(held.starts_with("2026-12-25|") && held.contains("pr-cal-held") && held.contains("pr-cal-holy") && held.ends_with("|Natal|true"), format!("the day held: {held}"));
    check(text(&mut page, "return cal('DT-BR').querySelector('.pr-cal-names').textContent;") == "25 · Natal", "the month's holy days are named under it".into());
    check(text(&mut page, "return day('DT-BR', '2026-12-08').classList.contains('pr-cal-holy');") == "false", "the 8th of December is no holiday in Brazil".into());
    // In the top layer: the whole calendar is on show although it hangs out of the Panel that holds its field, it stands
    // under the field, inside the window, and what is at its last day is that day.
    let geometry = text(
        &mut page,
        "const d = cal('DT-BR').getBoundingClientRect(), f = el('DT-BR').getBoundingClientRect(), p = el('PNL-1').getBoundingClientRect();
         const last = cal('DT-BR').querySelectorAll('[data-part=\"day\"]')[41]; const lr = last.getBoundingClientRect();
         const at = root.elementFromPoint(lr.x + lr.width / 2, lr.y + lr.height / 2);
         return JSON.stringify({ under: d.top >= f.bottom, left: Math.abs(d.left - f.left) < 1, inside: d.left >= 0 && d.top >= 0 && d.right <= innerWidth && d.bottom <= innerHeight,
           beyondPanel: d.bottom > p.bottom + 100, hit: at === last, w: Math.round(d.width), h: Math.round(d.height) });",
    );
    check(
        geometry.contains("\"under\":true") && geometry.contains("\"left\":true") && geometry.contains("\"inside\":true") && geometry.contains("\"beyondPanel\":true") && geometry.contains("\"hit\":true"),
        format!("where the calendar stands: {geometry}"),
    );
    // Its two colours are the field's ground and the ink that reads on it: black on white is far above 7 to 1, and a day's
    // number, a weekday's name and a holy day's name are all in that ink.
    let colours = text(
        &mut page,
        "const d = cal('DT-BR'), s = getComputedStyle(d); const of = q => getComputedStyle(d.querySelector(q)).color;
         return JSON.stringify({ ground: s.backgroundColor, ink: s.color, ratio: Math.round(ratio(s.backgroundColor, s.color) * 10) / 10,
           same: [of('[data-id=\"2026-12-10\"]'), of('.pr-cal-week span'), of('.pr-cal-names li'), of('select'), of('[data-part=\"today\"]')].every(c => c === s.color) });",
    );
    check(colours.contains("\"ground\":\"rgb(255, 255, 255)\"") && colours.contains("\"ink\":\"rgb(0, 0, 0)\"") && colours.contains("\"same\":true"), format!("the calendar's colours: {colours}"));
    let ratio: f64 = colours.split("\"ratio\":").nth(1).and_then(|r| r.split([',', '}']).next()).and_then(|r| r.parse().ok()).unwrap_or(0.0);
    check(ratio >= 7.0, format!("the ink reads on the ground at {ratio} to 1, under 7"));
    // A picture of the open calendar for whoever judges its look: `PRC_EVIDENCE_DIR` names the folder to write it into.
    if let Ok(folder) = std::env::var("PRC_EVIDENCE_DIR") {
        use base64::Engine;
        let shot = page.call("Page.captureScreenshot", serde_json::json!({ "format": "png" })).unwrap();
        let png = base64::engine::general_purpose::STANDARD.decode(shot["data"].as_str().unwrap_or("")).unwrap();
        std::fs::write(std::path::Path::new(&folder).join("date-field-calendar.png"), png).unwrap();
    }

    // ── another month and year ──────────────────────────────────────────────
    // The year list, then the month list (a list's own pop-up is the browser's: the choice is made as a script makes it).
    page.eval(&js("const s = part('DT-BR', 'year'); s.value = '2027'; s.dispatchEvent(new Event('change', { bubbles: true })); return true;")).unwrap();
    until(&mut page, "return cal('DT-BR').getAttribute('aria-label') === 'dezembro 2027';", "the year list did not jump to 2027");
    page.eval(&js("const s = part('DT-BR', 'month'); s.value = '4'; s.dispatchEvent(new Event('change', { bubbles: true })); return true;")).unwrap();
    until(&mut page, "return cal('DT-BR').getAttribute('aria-label') === 'abril 2027';", "the month list did not jump to April");
    check(text(&mut page, "return part('DT-BR', 'month').value + '|' + part('DT-BR', 'year').value;") == "4|2027", "the lists say the month on show".into());
    // The step buttons, pressed: back to March 2027, whose Good Friday (26) and Easter (28) Brazil keeps.
    press(&mut page, "part('DT-BR', 'previous')");
    until(&mut page, "return cal('DT-BR').getAttribute('aria-label') === 'março 2027';", "the step back did not show March 2027");
    check(text(&mut page, "return Array.from(cal('DT-BR').querySelectorAll('.pr-cal-names li')).map(l => l.textContent).join('|');") == "26 · Sexta-feira Santa|28 · Páscoa", "the movable holy days of March 2027".into());
    press(&mut page, "part('DT-BR', 'next')");
    until(&mut page, "return cal('DT-BR').getAttribute('aria-label') === 'abril 2027';", "the step forward did not show April 2027");
    press(&mut page, "part('DT-BR', 'previous')");
    until(&mut page, "return cal('DT-BR').getAttribute('aria-label') === 'março 2027';", "the step back did not show March 2027 again");
    check(text(&mut page, "return !!cal('DT-BR') && cal('DT-BR').open;") == "true", "the calendar stays open while its month changes".into());

    // ── picking ─────────────────────────────────────────────────────────────
    press(&mut page, "day('DT-BR', '2027-03-26')");
    closed(&mut page, "DT-BR", "a day was pressed");
    until(&mut page, "return box('DT-BR').value === '26/03/2027';", "the day pressed is not what the field shows");
    reads(&mut page, "DT-BR", "2027-03-26", "pressing the 26th of March 2027");
    check(text(&mut page, "return el('DT-BR').querySelector('[data-part=\"open\"]').getAttribute('aria-expanded');") == "false", "the button says the calendar is closed".into());
    // Escape, and a press on the page behind the calendar, close it and change nothing.
    open_calendar(&mut page, "DT-BR");
    page.press_key("Escape").unwrap();
    closed(&mut page, "DT-BR", "Escape");
    open_calendar(&mut page, "DT-BR");
    page.click_at("({ x: 880, y: 680, width: 2, height: 2 })").unwrap();
    closed(&mut page, "DT-BR", "a press on the page behind it");
    reads(&mut page, "DT-BR", "2027-03-26", "closing the calendar twice without picking");
    // Today, and Clear.
    open_calendar(&mut page, "DT-BR");
    press(&mut page, "part('DT-BR', 'today')");
    closed(&mut page, "DT-BR", "Today");
    let today = text(&mut page, "const d = new Date(); const p = n => String(n).padStart(2, '0'); return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate());");
    reads(&mut page, "DT-BR", &today, "pressing Today");
    open_calendar(&mut page, "DT-BR");
    check(text(&mut page, "const d = cal('DT-BR').querySelector('[aria-current=\"date\"]'); return d.dataset.id + '|' + d.classList.contains('pr-cal-today');") == format!("{today}|true"), "today is marked".into());
    press(&mut page, "part('DT-BR', 'clear')");
    closed(&mut page, "DT-BR", "Clear");
    until(&mut page, "return box('DT-BR').value === '';", "Clear left a date in the field");
    reads(&mut page, "DT-BR", "", "pressing Clear");

    // ── a field in the theme's colours ──────────────────────────────────────
    // `DT-ISO` has a see-through ground: its calendar takes the theme's surface and text, and it is typed as ISO.
    type_over(&mut page, "DT-ISO", "20261225");
    until(&mut page, "return box('DT-ISO').value === '2026-12-25';", "a `Short` field did not read 20261225");
    reads(&mut page, "DT-ISO", "2026-12-25", "typing 20261225 into the Short field");
    open_calendar(&mut page, "DT-ISO");
    let theme = text(&mut page, "const s = getComputedStyle(cal('DT-ISO')); return JSON.stringify({ ground: s.backgroundColor, ink: s.color, ratio: Math.round(ratio(s.backgroundColor, s.color) * 10) / 10 });");
    let theme_ratio: f64 = theme.split("\"ratio\":").nth(1).and_then(|r| r.split([',', '}']).next()).and_then(|r| r.parse().ok()).unwrap_or(0.0);
    check(theme.contains("\"ground\":\"rgb(46, 49, 56)\"") && theme.contains("\"ink\":\"rgb(255, 255, 255)\"") && theme_ratio >= 7.0, format!("the theme's surface and text: {theme}"));
    press(&mut page, "part('DT-ISO', 'close')");
    closed(&mut page, "DT-ISO", "Close");

    println!("== page_date_field: a date field and its calendar in a real browser (headless Chrome, locale pt-BR) ==");
    println!("  typed: 25122026, 3/11/26, 31/02/2026 (refused), 25/12/2026, 20261225 (Short); each read back as ISO by the program");
    println!("  calendar: {geometry}");
    println!("  colours of a white field: {colours}; of a field in the theme's colours: {theme}");
    println!("  holy days marked for Brazil: 25 Dec 2026 (Natal), 26 Mar 2027 (Sexta-feira Santa), 28 Mar 2027 (Páscoa); 8 Dec not marked");
    println!("  {checks} checks and 9 values read back; set-up {} ms, in the browser {} ms; all passed, 0 failed", browsing.duration_since(started).as_millis(), browsing.elapsed().as_millis());
    drop(server);
}
