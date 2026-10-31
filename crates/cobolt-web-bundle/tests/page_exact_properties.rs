// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Every `Exact` property gives the computed style it should, in a real browser** (WASM-007 T27: R1; AC2).
//!
//! The fixture's main form gains one control per check, each with its property set to a distinct value; a headless
//! Chrome reads `getComputedStyle` (or the attribute the row names) and compares it with the value the property means,
//! written here by hand from the property's definition — not computed by the player's own code. One line is printed per
//! row: property → expected → got. Every `Exact` row of `cobolt_forms::web_class::FIDELITY` must be checked here or
//! named in [`NOT_DRIVEN`] with the reason; the test fails on a row in neither.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_forms::web_class::{Fidelity, FIDELITY};
use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

/// `Exact` rows a form alone cannot show, and why (none: a Snackbar's notification is shown through `player.invoke`).
const NOT_DRIVEN: &[(&str, &str)] = &[];

/// One check: the control (`id`, type, its properties), the JS expression read on its element `e`, the expected value,
/// and the `Exact` rows it covers.
struct Check {
    rows: &'static [&'static str],
    id: &'static str,
    ty: &'static str,
    props: &'static [(&'static str, &'static str)],
    attrs: &'static str,
    read: &'static str,
    want: &'static str,
}

const fn c(rows: &'static [&'static str], id: &'static str, ty: &'static str, props: &'static [(&'static str, &'static str)], read: &'static str, want: &'static str) -> Check {
    Check { rows, id, ty, props, attrs: "", read, want }
}

/// `getComputedStyle(e)[p]` of the control's element.
macro_rules! cs {
    ($p:literal) => {
        concat!("getComputedStyle(e).", $p)
    };
}

/// The same, on the control's text element when it has one (`.pr-label-text`, a Button's caption), else the element.
macro_rules! text_cs {
    ($p:literal) => {
        concat!("getComputedStyle(e.querySelector('.pr-label-text, .pr-button-text, .pr-caption') || e).", $p)
    };
}

fn checks() -> Vec<Check> {
    vec![
        c(&["BackgroundColor"], "X-BG", "Label", &[("BackgroundColor", "#112233")], cs!("backgroundColor"), "rgb(17, 34, 51)"),
        c(
            &["BackgroundGradientEnabled", "BackgroundGradientStartColor", "BackgroundGradientEndColor", "BackgroundGradientDirection"],
            "X-GRAD",
            "Label",
            &[("BackgroundGradientEnabled", "true"), ("BackgroundGradientStartColor", "#FF0000"), ("BackgroundGradientEndColor", "#0000FF"), ("BackgroundGradientDirection", "East")],
            cs!("backgroundImage"),
            "linear-gradient(90deg, rgb(255, 0, 0), rgb(0, 0, 255))",
        ),
        c(&["ForegroundColor"], "X-FG", "Label", &[("ForegroundColor", "#445566")], text_cs!("color"), "rgb(68, 85, 102)"),
        c(&["FontName"], "X-FONT", "Label", &[("FontName", "Courier New")], "getComputedStyle(e).fontFamily.split(',')[0].trim()", "\"Courier New\""),
        c(&["FontSize"], "X-SIZE", "Label", &[("FontSize", "21")], cs!("fontSize"), "21px"),
        // The desktop's bold is a second stamp of the caption one whole pixel to the right (`paint_styled_galley`); the page
        // draws it as a shadow of the text and keeps the regular weight. At x = 20 the caption sits on a whole pixel: 1 px.
        c(&["Bold"], "X-BOLD", "Label", &[("Bold", "true"), ("ForegroundColor", "#010203")], "getComputedStyle(e).fontWeight + ' ' + getComputedStyle(e).textShadow", "400 rgb(1, 2, 3) 1px 0px 0px"),
        c(&["Italic"], "X-ITAL", "Label", &[("Italic", "true")], cs!("fontStyle"), "italic"),
        c(&["Underline", "Strikethrough"], "X-DECO", "Label", &[("Underline", "true"), ("Strikethrough", "true")], cs!("textDecorationLine"), "underline line-through"),
        c(&["Tooltip"], "X-TIP", "Label", &[("Tooltip", "a tip")], "e.getAttribute('title')", "a tip"),
        c(&["Cursor"], "X-CUR", "Label", &[("Cursor", "Hand")], cs!("cursor"), "pointer"),
        // `Padding` adds to the caption's inset from the frame (a Label's 3 px across, `paint`'s Label branch): against the
        // same Label without it, 9 px more on each side.
        c(
            &["Padding"],
            "X-PAD",
            "Label",
            &[("Padding", "9")],
            "(() => { const r = root.querySelector('[data-name=\"X-PAD-0\"]'); const d = s => parseFloat(getComputedStyle(e)['padding' + s]) - parseFloat(getComputedStyle(r)['padding' + s]); return d('Left') + ' ' + d('Right'); })()",
            "9 9",
        ),
        Check { rows: &[], id: "X-PAD-0", ty: "Label", props: &[], attrs: "", read: "'helper'", want: "helper" },
        // The side paddings are the layout padding of a container's client area (spec 056): against the same Panel
        // without them, exactly those pixels more inside each side.
        c(
            &["PaddingLeft", "PaddingTop", "PaddingRight", "PaddingBottom"],
            "X-PADS",
            "Panel",
            &[("PaddingLeft", "4"), ("PaddingTop", "5"), ("PaddingRight", "6"), ("PaddingBottom", "7")],
            "(() => { const r = root.querySelector('[data-name=\"X-PADS-0\"]'); return ['Left', 'Top', 'Right', 'Bottom'].map(s => parseFloat(getComputedStyle(e)['padding' + s]) - parseFloat(getComputedStyle(r)['padding' + s])).join(' '); })()",
            "4 5 6 7",
        ),
        Check { rows: &[], id: "X-PADS-0", ty: "Panel", props: &[], attrs: "", read: "'helper'", want: "helper" },
        // `Transparency` fades the frame (the background's alpha), never the content: 50 % of 255 is 128, which Chrome writes 0.5.
        c(&["Transparency"], "X-TRANS", "Label", &[("BackgroundColor", "#112233"), ("Transparency", "50")], "getComputedStyle(e).backgroundColor + ' ' + getComputedStyle(e).opacity", "rgba(17, 34, 51, 0.5) 1"),
        c(&["CornerRadius"], "X-RAD", "Label", &[("CornerRadius", "11")], cs!("borderTopLeftRadius"), "11px"),
        c(
            &["BorderColor", "BorderWidth"],
            "X-BORDER",
            "Label",
            &[("BorderStyle", "Single"), ("BorderColor", "#778899"), ("BorderWidth", "3")],
            "[getComputedStyle(e).borderTopColor, getComputedStyle(e).borderTopWidth, getComputedStyle(e).borderTopStyle].join(' ')",
            "rgb(119, 136, 153) 3px solid",
        ),
        // The shadow is the desktop's stack of rings, each moved by ShadowDistance in ShadowDirection: East 10 → every ring at
        // (10px, 0px), in ShadowColor. The light colour is the Neumorphic pair's, which a regular shadow never draws (on the
        // desktop either), so a distinct one changes nothing.
        c(
            &["ShadowColor", "ShadowDirection", "ShadowDistance", "ShadowLightColor"],
            "X-SHADOW",
            "Label",
            &[("BackgroundColor", "#808080"), ("ShadowEnabled", "true"), ("ShadowColor", "#FF0000"), ("ShadowLightColor", "#00FF00"), ("ShadowDirection", "East"), ("ShadowDistance", "10"), ("ShadowOpacity", "60")],
            "(() => { const s = getComputedStyle(e).boxShadow; const rings = s.split(/,(?![^(]*\\))/).map(r => r.trim()); \
              return rings.length > 1 && rings.every(r => /^rgba?\\(255, 0, 0[,)]/.test(r) && / 10px 0px 0px /.test(r + ' ')) && !s.includes('0, 255, 0') ? 'rings in red at 10px 0px, no green' : s; })()",
            "rings in red at 10px 0px, no green",
        ),
        // ShadowOpacity is the alpha of the shadow's core, the innermost ring (`shadow::rings`: t = 0): 60 % and 20 %.
        Check {
            rows: &["ShadowOpacity"],
            id: "X-SHOP",
            ty: "Label",
            props: &[("BackgroundColor", "#808080"), ("ShadowEnabled", "true"), ("ShadowColor", "#000000"), ("ShadowOpacity", "60")],
            attrs: "",
            read: "(() => { const core = el => Math.max(...(getComputedStyle(el).boxShadow.match(/rgba\\([^)]*\\)/g) || []).map(m => parseFloat(m.split(',')[3]))); \
                   return core(e).toFixed(2) + ' ' + core(root.querySelector('[data-name=\"X-SHOP-20\"]')).toFixed(2); })()",
            want: "0.60 0.20",
        },
        Check { rows: &[], id: "X-SHOP-20", ty: "Label", props: &[("BackgroundColor", "#808080"), ("ShadowEnabled", "true"), ("ShadowColor", "#000000"), ("ShadowOpacity", "20")], attrs: "", read: "'helper'", want: "helper" },
        // ShadowBlur off: one hard shadow; on: the soft stack of rings.
        c(
            &["ShadowBlur"],
            "X-SHBL",
            "Label",
            &[("BackgroundColor", "#808080"), ("ShadowEnabled", "true"), ("ShadowBlur", "false")],
            "(() => { const n = el => getComputedStyle(el).boxShadow.split(/,(?![^(]*\\))/).length; return n(e) + ' < ' + (n(e) < n(root.querySelector('[data-name=\"X-SHADOW\"]'))); })()",
            "1 < true",
        ),
        c(&["TextAlignment"], "X-HALIGN", "Label", &[("TextAlignment", "Right")], "getComputedStyle(e).justifyContent + ' ' + getComputedStyle(e).textAlign", "flex-end right"),
        c(&["VerticalAlignment"], "X-VALIGN", "Label", &[("VerticalAlignment", "Bottom")], cs!("alignItems"), "flex-end"),
        c(&["WordWrap"], "X-WRAP", "Label", &[("WordWrap", "true")], text_cs!("whiteSpace"), "pre-wrap"),
        // A chart's gradient frame: a ring on its `::after`, over a transparent border of the frame's width.
        c(
            &["BorderGradientEnabled", "BorderGradientStartColor", "BorderGradientEndColor", "BorderGradientDirection", "BorderTransparency"],
            "X-CHART",
            "BarChart",
            &[("BorderStyle", "Single"), ("BorderWidth", "4"), ("BorderGradientEnabled", "true"), ("BorderGradientStartColor", "#FF0000"), ("BorderGradientEndColor", "#0000FF"), ("BorderGradientDirection", "West"), ("BorderTransparency", "50")],
            "getComputedStyle(e, '::after').backgroundImage + ' / ' + getComputedStyle(e).borderTopWidth + ' ' + getComputedStyle(e).borderTopColor",
            "linear-gradient(270deg, rgba(255, 0, 0, 0.5), rgba(0, 0, 255, 0.5)) / 4px rgba(0, 0, 0, 0)",
        ),
        // A Snackbar's four corners belong to the notification it shows: `Show` it through the host API and read the
        // notification. Each corner its own value (top-left, top-right, bottom-right, bottom-left in CSS order).
        Check {
            rows: &["CornerRadiusTopLeft", "CornerRadiusTopRight", "CornerRadiusBottomLeft", "CornerRadiusBottomRight"],
            id: "X-SNACK",
            ty: "Snackbar",
            props: &[("CornerRadiusTopLeft", "3"), ("CornerRadiusTopRight", "6"), ("CornerRadiusBottomLeft", "9"), ("CornerRadiusBottomRight", "12"), ("Timeout", "0")],
            attrs: "",
            read: "window.__snack",
            want: "3px 6px 12px 9px",
        },
        Check { rows: &["ZOrder"], id: "X-Z", ty: "Label", props: &[], attrs: " z-order=\"5\"", read: cs!("zIndex"), want: "5" },
        Check { rows: &["Visible"], id: "X-HID", ty: "Label", props: &[], attrs: " visible=\"false\"", read: cs!("display"), want: "none" },
        Check { rows: &["Enabled"], id: "X-OFF", ty: "Button", props: &[], attrs: " enabled=\"false\"", read: "e.getAttribute('aria-disabled') + ' ' + (e.matches('button') ? e.disabled : (e.querySelector('button') || {}).disabled)", want: "true true" },
    ]
}

fn form(plain: &str, checks: &[Check]) -> String {
    let mut extra = String::new();
    for (i, ch) in checks.iter().enumerate() {
        let (x, y) = (20 + (i % 5) as i32 * 120, 120 + (i / 5) as i32 * 60);
        let props: String = ch.props.iter().map(|(k, v)| format!("<Property name=\"{k}\">{v}</Property>")).collect();
        extra.push_str(&format!(
            "  <Control id=\"{}\" type=\"{}\" x=\"{x}\" y=\"{y}\" w=\"100\" h=\"40\"{}><Property name=\"Caption\">ab cd</Property>{props}</Control>\n",
            ch.id, ch.ty, ch.attrs
        ));
    }
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    plain.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"480\" index-base=\"1\"").replace("</Form>", &format!("{extra}</Form>"))
}

#[test]
fn every_exact_property_gives_its_computed_style_in_chrome() {
    let checks = checks();
    // Every Exact row is checked or named with its reason — and nothing else is claimed.
    let exact: Vec<&str> = FIDELITY.iter().filter(|r| r.fidelity == Fidelity::Exact).map(|r| r.property).collect();
    let covered: Vec<&str> = checks.iter().flat_map(|c| c.rows.iter().copied()).chain(NOT_DRIVEN.iter().map(|(p, _)| *p)).collect();
    let missing: Vec<&&str> = exact.iter().filter(|p| !covered.contains(p)).collect();
    assert!(missing.is_empty(), "Exact rows neither checked nor named in NOT_DRIVEN: {missing:?}");
    let extra: Vec<&&str> = covered.iter().filter(|p| !exact.contains(p)).collect();
    assert!(extra.is_empty(), "rows claimed here that are not Exact: {extra:?}");

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
        f.cfrm = form(&f.cfrm, &checks);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1000, 800).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    let lib = "const root = document.getElementById('pr-app').shadowRoot;";
    page.wait_for(&format!("(() => {{ {lib} const l = root.querySelector('[data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}; console {:?}", page.console));

    // The notification of `X-SNACK`, shown as a program's `Show` would.
    page.eval("(async () => { const a = await PowerRustCOBOL.mount(document.getElementById('pr-app')); a.player.invoke(0, 'X-SNACK', 'Show', ['shown']); return true; })()").unwrap();
    let snack = page.wait_for(&format!("(() => {{ {lib} const t = root.querySelector('.pr-snackbar'); if (!t) return null; const s = getComputedStyle(t); \
         window.__snack = [s.borderTopLeftRadius, s.borderTopRightRadius, s.borderBottomRightRadius, s.borderBottomLeftRadius].join(' '); return window.__snack; }})()"), 10);
    assert!(snack.is_ok(), "the Snackbar's notification did not show: {snack:?}; console {:?}", page.console);
    let mut lines = Vec::new();
    let mut mismatches = Vec::new();
    for ch in checks.iter().filter(|c| !c.rows.is_empty()) {
        let got = page
            .eval(&format!("(() => {{ {lib} const e = root.querySelector('[data-name=\"{}\"]'); if (!e && {:?} !== 'X-SNACK') return 'no element'; return String({}); }})()", ch.id, ch.id, ch.read))
            .unwrap_or_else(|e| serde_json::Value::String(format!("error: {e}")));
        let got = got.as_str().unwrap_or("").to_owned();
        let ok = got == ch.want;
        lines.push(format!("  {} {} → {:?} → {:?}", if ok { "ok  " } else { "FAIL" }, ch.rows.join(", "), ch.want, got));
        if !ok {
            mismatches.push(format!("{}: expected {:?}, got {:?}", ch.rows.join(", "), ch.want, got));
        }
    }
    let checked: usize = checks.iter().map(|c| c.rows.len()).sum();
    println!("AC2 (Chrome {}): {checked} of {} Exact rows checked in {} controls, {} not driven ({}); {} mismatches", exe.display(), exact.len(), checks.iter().filter(|c| !c.rows.is_empty()).count(), NOT_DRIVEN.len(), NOT_DRIVEN.iter().map(|(p, _)| *p).collect::<Vec<_>>().join(", "), mismatches.len());
    for l in &lines {
        println!("{l}");
    }
    drop(server);
    assert!(mismatches.is_empty(), "Exact rows that did not give their style:\n{}", mismatches.join("\n"));
}
