// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-036 T9 — the designer's canvas paints what the page's stylesheets decide (R6, R7, R8,
//! R19, R20; AC4, AC5, AC12). Real frames of the real `DesignerPanel::show`; the colour read back is
//! the fill the frame painted.

use super::*;
use crate::llm::LlmConfig;
use super::super::css_rule_question::{Answer, CssRemoveConfirm, CssRuleQuestion, RemoveInfo, RuleInfo};
use cobolt_forms::css::Scheme;
use cobolt_forms::{Form, PropValue};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);
const SHEET: &str = "#PNL-1 { background-color: rgb(10 132 255) }\n@media (prefers-color-scheme: dark) { #PNL-1 { background-color: rgb(171 52 18) } }\n";

fn flatten(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| flatten(s, out)),
        s => out.push(s.clone()),
    }
}

fn form() -> Form {
    let mut f = Form::new("F", "F", 400, 300);
    let mut p = Control::new("PNL-1", ControlType::Panel, 20, 20);
    p.rect = cobolt_forms::model::Rect::new(20, 20, 160, 80);
    p.set_prop("Transparency", PropValue::String("50".into()));
    f.controls.push(p);
    f
}

fn designer(web: bool, sheet: &str, scheme: Scheme) -> DesignerPanel {
    let mut d = DesignerPanel::new(form());
    d.properties.web_project = web;
    d.properties.css_sheets = vec![("webforms/F.css".into(), sheet.to_owned())];
    d.css_scheme = scheme;
    d
}

/// `frames` frames of `d`; the shapes of the last.
fn frames(d: &mut DesignerPanel, ctx: &egui::Context, frames: usize) -> Vec<egui::Shape> {
    let llm = LlmConfig::load_defaults_for_test();
    let mut shapes = Vec::new();
    for t in 0..frames {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
        input.max_texture_side = Some(8192);
        input.time = Some(t as f64 * 0.05);
        let mut out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let _ = d.show(ui, &mut None, &[], &llm, None, None);
            });
        });
        out.textures_delta.clear();
        shapes.clear();
        out.shapes.iter().for_each(|c| flatten(&c.shape, &mut shapes));
    }
    shapes
}

/// The fill of the panel's face: the rectangle at the panel's place on the canvas whose fill is opaque.
fn panel_fill(d: &DesignerPanel, shapes: &[egui::Shape]) -> Option<Color32> {
    let origin = d.canvas_origin?;
    shapes.iter().find_map(|s| match s {
        egui::Shape::Rect(r) if r.fill.a() == 255 && (r.rect.min - (origin + Vec2::new(20.0, 20.0))).length() < 1.0 && (r.rect.width() - 160.0).abs() < 1.0 => Some(r.fill),
        _ => None,
    })
}

fn close(got: Color32, want: [u8; 3]) -> bool {
    got.to_array()[..3].iter().zip(want).all(|(g, w)| (*g as i32 - w as i32).abs() <= 1)
}

#[test]
fn the_canvas_paints_the_stylesheets_colour_in_each_scheme_and_a_desktop_form_keeps_its_properties() {
    let t0 = std::time::Instant::now();
    let mut cases = 0;
    let ctx = egui::Context::default();
    let mut light = designer(true, SHEET, Scheme::Light);
    let s = frames(&mut light, &ctx, 3);
    let got = panel_fill(&light, &s).expect("the panel's face is painted");
    assert!(close(got, [10, 132, 255]), "Light: the rule's colour, unfaded by Transparency 50: {got:?}");
    cases += 1;

    let ctx = egui::Context::default();
    let mut dark = designer(true, SHEET, Scheme::Dark);
    let s = frames(&mut dark, &ctx, 3);
    let got = panel_fill(&dark, &s).expect("the panel's face is painted");
    assert!(close(got, [171, 52, 18]), "Dark: the @media rule's colour: {got:?}");
    cases += 1;

    // AC12: the engine's picture of the same form and sheets has the canvas's pixel.
    for (scheme, want) in [(Scheme::Light, [10u8, 132, 255]), (Scheme::Dark, [171, 52, 18])] {
        let opts = cobolt_forms::snapshot::SnapshotOptions {
            sheets: vec![cobolt_forms::snapshot::SnapshotSheet { label: "webforms/F.css".into(), text: SHEET.into(), origin: cobolt_forms::css_effective::Origin::Form }],
            color_scheme: scheme,
            ..Default::default()
        };
        let img = cobolt_forms::snapshot::render_form_image(&form(), &opts);
        let px = img.pixels[60 * img.size[0] + 100].to_srgba_unmultiplied();
        assert!(close(Color32::from_rgba_unmultiplied(px[0], px[1], px[2], px[3]), want), "{scheme:?}: the picture's pixel is the canvas's: {px:?}");
        cases += 1;
    }

    // R19: a Desktop form with a stylesheet beside it paints the stored properties.
    let ctx = egui::Context::default();
    let mut desktop = designer(false, SHEET, Scheme::Light);
    let s = frames(&mut desktop, &ctx, 3);
    let faded = panel_fill(&desktop, &s);
    assert!(faded.is_none_or(|c| !close(c, [10, 132, 255])), "a Desktop form is not restyled: {faded:?}");
    cases += 1;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: the canvas paints the stylesheet");
    println!("│ cases: Panel at Transparency 50, #PNL-1 rule + @media dark; Light, Dark, engine picture in both, Desktop unchanged");
    println!("│ time: {ms:.0} ms for {cases} cases ({:.1} cases/sec)", cases as f64 / (ms / 1000.0));
    println!("└─ {cases}/{cases} passed, 0 failed");
}

#[test]
fn a_frame_with_nothing_changed_computes_nothing_and_an_edit_shows_at_once() {
    let ctx = egui::Context::default();
    let mut d = designer(true, SHEET, Scheme::Light);
    frames(&mut d, &ctx, 3);
    let (parses, computed) = (d.css_cache.stats.sheet_parses, d.css_cache.stats.computed);
    let t0 = std::time::Instant::now();
    frames(&mut d, &ctx, 30);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!((d.css_cache.stats.sheet_parses, d.css_cache.stats.computed), (parses, computed), "R20: 30 unchanged frames compute nothing");
    // R8 / AC5: the sheet's text changes (an Apply, an edit made elsewhere): the next frame paints it.
    d.properties.css_sheets = vec![("webforms/F.css".into(), "#PNL-1 { background-color: #00ff00 }".into())];
    let t1 = std::time::Instant::now();
    let s = frames(&mut d, &ctx, 1);
    let edit_ms = t1.elapsed().as_secs_f64() * 1000.0;
    let got = panel_fill(&d, &s).expect("painted");
    assert!(close(got, [0, 255, 0]), "the edited sheet is painted by the next frame: {got:?}");
    println!("┌─ designer_css_effective: no recompute, edit shown");
    println!("│ 30 unchanged frames: {ms:.0} ms ({:.0} frames/sec), sheet parses {parses}, controls computed {computed}, both unchanged", 30.0 / (ms / 1000.0));
    println!("│ one frame after the sheet changed: {edit_ms:.1} ms");
    println!("└─ 2/2 passed, 0 failed");
}

#[test]
fn the_badge_lists_what_the_canvas_cannot_paint_up_to_eight_then_how_many_more() {
    let tr = crate::i18n::current_tr(&egui::Context::default());
    let mk = |n: usize| -> Vec<cobolt_forms::css_effective::Unpainted> {
        (0..n)
            .map(|i| cobolt_forms::css_effective::Unpainted { css: "width".into(), value: format!("{i}%"), sheet: Some("webforms/F.css".into()), line: i + 1, why: cobolt_forms::css_effective::Why::Geometry })
            .collect()
    };
    let one = css_badge_text(&tr, &mk(1));
    assert_eq!(one.lines().count(), 2, "{one}");
    assert!(one.contains("webforms/F.css:1  width: 0%"), "{one}");
    let ten = css_badge_text(&tr, &mk(10));
    assert_eq!(ten.lines().count(), 1 + 8 + 1, "header, eight lines, the rest: {ten}");
    assert!(ten.ends_with(&tr.css_badge_more.replace("{}", "2")), "{ten}");
    println!("┌─ designer_css_effective: the badge text\n│ 1 declaration: 2 lines; 10 declarations: header + 8 + 'more' = 10 lines\n└─ 2/2 passed, 0 failed");

    // A real frame: a width in percent is geometry, the badge's tooltip names it, the colour is painted.
    let ctx = egui::Context::default();
    let mut d = designer(true, "#PNL-1 { width: 50%; background-color: #00ff00 }", Scheme::Light);
    let s = frames(&mut d, &ctx, 3);
    assert!(close(panel_fill(&d, &s).expect("painted"), [0, 255, 0]));
    let view = d.css_style_view().expect("a view");
    let u = view.unpainted.get("PNL-1").expect("the width is not painted");
    assert!(u.iter().any(|x| x.css == "width" && x.line == 1), "{u:?}");
    assert_eq!(d.form.controls[0].rect.w, 160, "a width: 50% keeps its designed rectangle");
}

// ── T10: the Properties panel ────────────────────────────────────────────────────────────────────

fn props_panel(sheet: &str) -> super::super::properties::PropertiesPanel {
    let mut p = super::super::properties::PropertiesPanel::new();
    p.web_project = true;
    p.css_sheets = vec![("webforms/F.css".into(), sheet.to_owned())];
    p
}

fn buttons(n: usize) -> Form {
    let mut f = Form::new("F", "F", 400, 300);
    for i in 1..=n {
        let mut b = Control::new(&format!("BTN-{i}"), ControlType::Button, 10, 10 + 40 * i as i32);
        b.set_prop("CssClass", PropValue::String("go".into()));
        f.controls.push(b);
    }
    f
}

#[test]
fn the_properties_panel_shows_the_effective_value_and_a_mark_in_each_language() {
    let t0 = std::time::Instant::now();
    let form = buttons(1);
    let c = form.controls[0].clone();
    let mut p = props_panel("#BTN-1 { background-color: rgb(10 132 255) }\n");
    let eff = p.css_effective_control(&form, &c).expect("a rule decides a value");
    assert_eq!(cobolt_forms::paint::parse_color(eff.get_prop("BackgroundColor").unwrap().as_str()), Color32::from_rgb(10, 132, 255), "AC3: the row's value is the page's");
    assert_ne!(c.get_prop("BackgroundColor").map(|v| v.as_str().to_owned()), eff.get_prop("BackgroundColor").map(|v| v.as_str().to_owned()), "the stored value is not the displayed one");
    let mut langs = 0;
    for lang in crate::i18n::Language::ALL {
        let tr = lang.tr();
        let marks = p.css_marks_for(&form, &c, &tr);
        let m = marks.get("backgroundcolor").expect("the mark");
        assert!(m.contains("webforms/F.css:1"), "{lang:?}: the mark names the sheet and the line: {m}");
        let stored = c.get_prop("BackgroundColor").map(|v| v.as_str().to_owned()).unwrap_or_default();
        assert!(stored.is_empty() || m.contains(&stored), "{lang:?}: the stored value is in the tooltip: {m}");
        langs += 1;
    }
    // A desktop form is shown as stored.
    p.web_project = false;
    assert!(p.css_effective_control(&form, &c).is_none() && p.css_marks_for(&form, &c, &crate::i18n::Language::English.tr()).is_empty());
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: the Properties panel\n│ BackgroundColor row = rgb(10 132 255), mark names webforms/F.css:1 in {langs} languages, Desktop unchanged\n│ time: {ms:.1} ms\n└─ {}/{} passed, 0 failed", langs + 2, langs + 2);
}

#[test]
fn a_value_the_canvas_cannot_read_is_shown_as_written_and_a_mixed_selection_is_marked() {
    let tr = crate::i18n::Language::English.tr();
    let form = buttons(3);
    // Unknown: a colour function the table does not read.
    let mut p = props_panel("#BTN-1 { background-color: color-mix(in srgb, red, blue) }\n");
    let unknown = p.css_unknown_of(&form, &form.controls[0], &tr);
    println!("unknown: {unknown:?}");
    assert!(unknown.get("backgroundcolor").is_some_and(|t| t.contains("color-mix")), "{unknown:?}");

    // Mixed: two of three buttons get a colour from a rule that names them.
    let mut p = props_panel("#BTN-1 { background-color: #112233 }\n#BTN-2 { background-color: #112233 }\n#BTN-3 { background-color: #445566 }\n");
    let ids: Vec<String> = form.controls.iter().map(|c| c.id.clone()).collect();
    let mixed = p.css_mixed_of(&form, &ids, &tr);
    let m = mixed.get("backgroundcolor").expect("three buttons, two colours: mixed");
    assert!(m.contains("#112233ff") && m.contains("#445566ff"), "{m}");
    let same = p.css_mixed_of(&form, &ids[..2], &tr);
    assert!(!same.contains_key("backgroundcolor"), "two buttons with the same colour are not mixed: {same:?}");
    println!("┌─ designer_css_effective: unknown and mixed\n│ color-mix() shown read-only; 3 buttons / 2 colours marked mixed, 2 buttons / 1 colour not\n└─ 3/3 passed, 0 failed");
}

// ── T11: writing the value back ──────────────────────────────────────────────────────────────────

use cobolt_forms::css_effective::Origin;

struct Fixture {
    dir: std::path::PathBuf,
    d: DesignerPanel,
}

impl Fixture {
    /// A Web designer on a temporary folder whose sheets are `(file name, project sheet?, text)`.
    fn new(tag: &str, sheets: &[(&str, bool, &str)], form: Form) -> Self {
        let dir = std::env::temp_dir().join(format!("prc-css-t11-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = DesignerPanel::new(form);
        d.properties.web_project = true;
        for (name, project, text) in sheets {
            std::fs::write(dir.join(name), text).unwrap();
            d.properties.css_sheets.push((name.to_string(), text.to_string()));
            d.properties.css_sheet_files.push(dir.join(name));
            if *project {
                d.properties.css_project_sheet = Some(name.to_string());
            }
        }
        Fixture { dir, d }
    }
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn set(f: &mut Fixture, id: &str, key: &str, v: &str) {
    f.d.set_property(id, key, PropValue::String(v.to_owned()));
}

#[test]
fn a_property_a_rule_decides_is_written_into_the_sheet_and_undo_redo_are_byte_exact() {
    let t0 = std::time::Instant::now();
    let original = "/* mine */\n#PNL-1 {\n  background-color: #112233;\n  color: red;\n}\n";
    let mut f = Fixture::new("write", &[("F.css", false, original)], form());
    let stored = f.d.form.controls[0].get_prop("BackgroundColor").map(|v| v.as_str().to_owned());
    set(&mut f, "PNL-1", "BackgroundColor", "#445566ff");
    let written = f.read("F.css");
    assert_eq!(written, "/* mine */\n#PNL-1 {\n  background-color: #445566;\n  color: red;\n}\n", "only the value's bytes changed (AC6, longhand)");
    assert_eq!(f.d.form.controls[0].get_prop("BackgroundColor").map(|v| v.as_str().to_owned()), stored, "the property is not stored beside the rule");
    assert!(f.d.css_written.iter().any(|(p, _)| p.ends_with("F.css")), "the open Preview is told");
    f.d.undo();
    assert_eq!(f.read("F.css"), original, "undo: byte for byte");
    f.d.redo();
    assert_eq!(f.read("F.css"), written, "redo: byte for byte");
    // The sheet changes behind the IDE's back: undo leaves the other program's text and says so (AC9).
    let outside = format!("{written}/* edited elsewhere */\n");
    std::fs::write(f.dir.join("F.css"), &outside).unwrap();
    f.d.undo();
    assert_eq!(f.read("F.css"), outside, "an outside change is kept");
    assert!(matches!(f.d.css_messages.last(), Some(CssMessage::UndoChangedOnDisk(l)) if l == "F.css"), "{:?}", f.d.css_messages);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: writing back\n│ longhand write, undo, redo byte-exact; an outside change kept on undo and said\n│ time: {ms:.1} ms\n└─ 5/5 passed, 0 failed");
}

#[test]
fn a_text_row_that_lost_the_focus_untouched_writes_nothing_and_a_desktop_form_stores_as_before() {
    let original = "#PNL-1 { background-color: #112233 }\n";
    let mut f = Fixture::new("focus", &[("F.css", false, original)], form());
    set(&mut f, "PNL-1", "BackgroundColor", "#112233ff");
    assert_eq!(f.read("F.css"), original);
    assert!(!f.d.dirty, "nothing was recorded: the value is the one in effect");
    // Desktop: the sheet is not read and the property is stored.
    let mut f = Fixture::new("desktop", &[("F.css", false, original)], form());
    f.d.properties.web_project = false;
    set(&mut f, "PNL-1", "BackgroundColor", "#445566ff");
    assert_eq!(f.read("F.css"), original);
    assert!(f.d.form.controls[0].get_prop("BackgroundColor").is_some(), "stored as a property");
}

#[test]
fn an_edit_that_could_not_show_writes_nothing_and_names_the_winner_and_an_open_window_refuses() {
    let mut form = Form::new("F", "F", 400, 300);
    form.controls.push(Control::new("BTN-1", ControlType::Button, 10, 10));
    let project = ".pr-button { color: green !important }\n";
    let own = "#BTN-1 { margin: 0 }\n";
    let mut f = Fixture::new("noeffect", &[("application.css", true, project), ("F.css", false, own)], form);
    set(&mut f, "BTN-1", "ForegroundColor", "#0000ffff");
    f.d.css_answer(Answer::OnlyThisControl); // the rule is shared: the question comes first
    assert_eq!((f.read("application.css"), f.read("F.css")), (project.to_owned(), own.to_owned()), "AC8: both files byte-identical");
    assert_eq!(f.d.css_messages, vec![CssMessage::NoEffect { file: Some("application.css".into()), line: 1 }]);

    let sheet = "#PNL-1 { background-color: #112233 }\n";
    let mut f = Fixture::new("modal", &[("F.css", false, sheet)], self::form());
    f.d.css_modal_unapplied = Some(f.dir.join("F.css"));
    set(&mut f, "PNL-1", "BackgroundColor", "#445566ff");
    assert_eq!(f.read("F.css"), sheet);
    assert_eq!(f.d.css_messages, vec![CssMessage::RefusedUnapplied], "R18: refused while the window holds unapplied text");
}

#[test]
fn thirty_frames_of_a_picker_make_one_write_and_one_undo_step() {
    let original = "#PNL-1 { background-color: #112233 }\n";
    let mut f = Fixture::new("gesture", &[("F.css", false, original)], form());
    f.d.css_tick(true); // the pointer is down: a picker is dragging
    let t0 = std::time::Instant::now();
    for i in 0..30u8 {
        set(&mut f, "PNL-1", "BackgroundColor", &format!("#{:02x}40{:02x}ff", i * 8, 255 - i * 8));
    }
    assert_eq!(f.read("F.css"), original, "nothing is written while the gesture lasts");
    assert!(f.d.css_written.is_empty());
    f.d.css_tick(false);
    let after = f.read("F.css");
    assert!(after.contains("#e84017"), "the last value was written: {after}");
    assert_eq!(f.d.css_written.len(), 1, "one write");
    f.d.undo();
    assert_eq!(f.read("F.css"), original, "one undo step undoes the whole gesture");
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: a gesture\n│ 30 picker changes: 1 file write, 1 undo step ({ms:.1} ms)\n└─ 4/4 passed, 0 failed");
}

// ── T12: the shared-rule question ────────────────────────────────────────────────────────────────


fn three_buttons() -> Form {
    let mut f = Form::new("F", "F", 400, 300);
    for i in 1..=3 {
        f.controls.push(Control::new(&format!("BTN-{i}"), ControlType::Button, 10, 10 + 40 * i));
    }
    f
}

fn colours(d: &mut DesignerPanel) -> Vec<Option<String>> {
    let view = d.css_style_view().expect("a view");
    ["BTN-1", "BTN-2", "BTN-3"].iter().map(|id| view.values.get(*id).and_then(|v| v.iter().find(|(p, _)| *p == "ForegroundColor")).map(|(_, t)| t.clone())).collect()
}

#[test]
fn a_shared_rule_is_asked_about_and_each_answer_does_what_it_says() {
    let sheet = ".pr-button { color: #112233 }\n";
    // The question names the rule and the two others; nothing is written until it is answered.
    let mut f = Fixture::new("q-ask", &[("F.css", false, sheet)], three_buttons());
    set(&mut f, "BTN-1", "ForegroundColor", "#445566ff");
    assert_eq!(f.read("F.css"), sheet, "nothing is written before the answer");
    let q = f.d.css_question.as_ref().expect("the question is held");
    assert_eq!(q.info, RuleInfo { selector: ".pr-button".into(), file: Some("F.css".into()), line: 1, others: 2, at_least: false });

    // Change the rule: all three change.
    f.d.css_answer(Answer::ChangeRule);
    assert_eq!(f.read("F.css"), ".pr-button { color: #445566 }\n");
    assert_eq!(colours(&mut f.d), vec![Some("#445566ff".into()); 3], "AC7: the rule changed all three");

    // Only this control: the rule stays, BTN-1 gets its own rule at the end of the form sheet.
    let mut f = Fixture::new("q-only", &[("F.css", false, sheet)], three_buttons());
    set(&mut f, "BTN-1", "ForegroundColor", "#445566ff");
    f.d.css_answer(Answer::OnlyThisControl);
    let written = f.read("F.css");
    assert!(written.starts_with(sheet) && written.contains("#BTN-1") && written.contains("#445566"), "{written}");
    let c = colours(&mut f.d);
    assert_eq!(c[0].as_deref(), Some("#445566ff"));
    assert_eq!((c[1].as_deref(), c[2].as_deref()), (Some("#112233ff"), Some("#112233ff")), "the other two keep the rule's colour");

    // Cancel: both files and the question are gone, nothing changed.
    let mut f = Fixture::new("q-cancel", &[("F.css", false, sheet)], three_buttons());
    set(&mut f, "BTN-1", "ForegroundColor", "#445566ff");
    f.d.css_answer(Answer::Cancel);
    assert_eq!(f.read("F.css"), sheet);
    assert!(f.d.css_question.is_none() && !f.d.dirty);
    println!("┌─ designer_css_effective: the shared-rule question\n│ .pr-button on 3 buttons: names 2 others; Change the rule -> 3 changed; Only this control -> 1 changed + own rule; Cancel -> nothing\n└─ 3/3 passed, 0 failed");
}

/// The window's rectangle, from egui's memory.
fn rect(ctx: &egui::Context) -> Rect {
    ctx.memory(|m| m.area_rect(CssRuleQuestion::id())).expect("the window is shown")
}

fn frame(ctx: &egui::Context, q: &mut CssRuleQuestion, tr: &crate::i18n::Tr, info: &RuleInfo, events: Vec<egui::Event>) {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
    input.events = events;
    let mut out = ctx.run_ui(input, |ui| {
        let theme = crate::theme::active();
        let _ = q.show(ui.ctx(), &theme, tr, info);
    });
    out.textures_delta.clear();
}

#[test]
fn the_question_window_never_resizes_itself_in_any_language_and_keeps_a_dragged_size() {
    let info = RuleInfo { selector: ".pr-button".into(), file: Some("webforms/application.css".into()), line: 12, others: 7, at_least: true };
    let t0 = std::time::Instant::now();
    let mut frames = 0;
    for lang in crate::i18n::Language::ALL {
        let tr = lang.tr();
        let ctx = egui::Context::default();
        let mut q = CssRuleQuestion::new();
        frame(&ctx, &mut q, &tr, &info, vec![]);
        frame(&ctx, &mut q, &tr, &info, vec![]);
        let first = rect(&ctx);
        for _ in 0..120 {
            frame(&ctx, &mut q, &tr, &info, vec![]);
            frames += 1;
            assert_eq!(rect(&ctx), first, "{lang:?}: the window keeps its rectangle");
        }
        // A drag of the grip: the one writer of the size.
        let grip = first.max - Vec2::new(10.0, 10.0);
        let before = q.size;
        frame(&ctx, &mut q, &tr, &info, vec![egui::Event::PointerMoved(grip)]);
        frame(&ctx, &mut q, &tr, &info, vec![egui::Event::PointerButton { pos: grip, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        frame(&ctx, &mut q, &tr, &info, vec![egui::Event::PointerMoved(grip + Vec2::new(60.0, 40.0))]);
        frame(&ctx, &mut q, &tr, &info, vec![egui::Event::PointerButton { pos: grip + Vec2::new(60.0, 40.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert!(q.size.x > before.x && q.size.y > before.y, "{lang:?}: the grip grew the window: {before:?} -> {:?}", q.size);
        frame(&ctx, &mut q, &tr, &info, vec![]);
        let dragged = rect(&ctx);
        for _ in 0..20 {
            frame(&ctx, &mut q, &tr, &info, vec![]);
            assert_eq!(rect(&ctx), dragged, "{lang:?}: the dragged size is kept");
        }
        for s in [tr.css_shared_title, tr.css_shared_body, tr.css_shared_change_rule, tr.css_shared_only_this, tr.css_shared_at_least] {
            assert!(!s.trim().is_empty(), "{lang:?}: an empty string");
        }
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ css_rule_question: the window\n│ 6 languages x (120 frames held + a grip drag + 20 frames kept)\n│ time: {ms:.0} ms ({:.0} frames/sec for the held frames)\n└─ 6/6 languages passed, 0 failed", frames as f64 / (ms / 1000.0));
}

// ── T13: the row actions ─────────────────────────────────────────────────────────────────────────

#[test]
fn remove_asks_first_takes_out_only_that_declaration_and_undo_puts_it_back() {
    let t0 = std::time::Instant::now();
    let original = "/* mine */\n#PNL-1 {\n  color: red;\n  background-color: #112233;\n  margin: 4px;\n}\n";
    let mut f = Fixture::new("remove", &[("F.css", false, original)], form());
    f.d.css_request_remove("PNL-1", "BackgroundColor");
    let q = f.d.css_remove_question.clone().expect("it asks first");
    assert_eq!(q.info.declaration, "background-color: #112233");
    assert_eq!((q.info.file.as_deref(), q.info.line), (Some("F.css"), 2));
    assert_eq!(f.read("F.css"), original, "nothing is written before the answer");
    // Cancel: nothing changes.
    f.d.css_remove_answer(false);
    assert_eq!(f.read("F.css"), original);
    // Confirm: only that declaration, its own line, byte for byte.
    f.d.css_request_remove("PNL-1", "BackgroundColor");
    f.d.css_remove_answer(true);
    let removed = "/* mine */\n#PNL-1 {\n  color: red;\n  margin: 4px;\n}\n";
    assert_eq!(f.read("F.css"), removed, "AC10: only the declaration left");
    f.d.undo();
    assert_eq!(f.read("F.css"), original, "undo restores it byte for byte");
    f.d.redo();
    assert_eq!(f.read("F.css"), removed);
    // A window holding unapplied text for that sheet refuses (R18).
    f.d.undo();
    f.d.css_modal_unapplied = Some(f.dir.join("F.css"));
    f.d.css_request_remove("PNL-1", "BackgroundColor");
    f.d.css_remove_answer(true);
    assert_eq!(f.read("F.css"), original);
    assert_eq!(f.d.css_messages.last(), Some(&CssMessage::RefusedUnapplied));
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: remove the declaration\n│ ask first, cancel, confirm, undo, redo byte-exact; refused while the window holds unapplied text\n│ time: {ms:.1} ms\n└─ 7/7 passed, 0 failed");
}

#[test]
fn a_css_style_declaration_is_removed_through_the_controls_own_text_and_go_to_names_the_rule() {
    let t0 = std::time::Instant::now();
    let sheet = "\n\n#PNL-1 { background-color: #112233 }\n";
    let mut f = Fixture::new("goto", &[("F.css", false, sheet)], form());
    f.d.css_request_go_to("PNL-1", "BackgroundColor");
    assert_eq!(f.d.css_go_to.take(), Some((f.dir.join("F.css"), 3)), "the sheet and the rule's line");
    // The properties panel offers the actions only where there is a declaration, and no rule to go to for CssStyle.
    let c = f.d.form.find_control("PNL-1").unwrap().clone();
    let src = f.d.properties.css_sources_for(&f.d.form.clone(), &c);
    assert_eq!(src.get("backgroundcolor"), Some(&Some("F.css".to_owned())));
    // The control's own CssStyle (which wins over the sheet) has no sheet.
    f.d.form.find_control_mut("PNL-1").unwrap().properties.insert("CssStyle".into(), PropValue::String("color: blue; background-color: #abcdef".into()));
    f.d.properties.css_sheets_checked = None;
    let c = f.d.form.find_control("PNL-1").unwrap().clone();
    let src = f.d.properties.css_sources_for(&f.d.form.clone(), &c);
    assert_eq!(src.get("backgroundcolor"), Some(&None), "CssStyle has no rule to go to");
    f.d.css_request_go_to("PNL-1", "BackgroundColor");
    assert!(f.d.css_go_to.is_none(), "a CssStyle declaration offers no go-to");
    f.d.css_request_remove("PNL-1", "BackgroundColor");
    assert_eq!(f.d.css_remove_question.as_ref().map(|q| q.info.file.clone()), Some(None));
    f.d.css_remove_answer(true);
    assert_eq!(f.d.form.find_control("PNL-1").unwrap().get_prop("CssStyle").map(|v| v.as_str().to_owned()).as_deref(), Some("color: blue;"), "only that declaration left CssStyle");
    assert_eq!(f.read("F.css"), sheet, "the sheet is untouched");
    f.d.undo();
    assert_eq!(f.d.form.find_control("PNL-1").unwrap().get_prop("CssStyle").map(|v| v.as_str().to_owned()).as_deref(), Some("color: blue; background-color: #abcdef"));
    for lang in crate::i18n::Language::ALL {
        let t = lang.tr();
        for s in [t.css_go_to_rule, t.css_remove_declaration, t.css_remove_confirm, t.css_remove_button] {
            assert!(!s.trim().is_empty(), "{lang:?}: an empty string");
        }
        assert_eq!(t.css_remove_confirm.matches("{}").count(), 2, "{lang:?}: two places");
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: go to the rule, CssStyle removal\n│ go-to names sheet and line; CssStyle has no rule to go to; its declaration removed alone, undone; 6 languages\n│ time: {ms:.1} ms\n└─ 8/8 passed, 0 failed");
}

#[test]
fn the_removal_window_never_resizes_itself_in_any_language() {
    let info = RemoveInfo { declaration: "background-color: #112233".into(), file: Some("webforms/application.css".into()), line: 12 };
    let t0 = std::time::Instant::now();
    let mut frames = 0;
    for lang in crate::i18n::Language::ALL {
        let tr = lang.tr();
        let ctx = egui::Context::default();
        let mut w = CssRemoveConfirm::new();
        let run = |ctx: &egui::Context, w: &mut CssRemoveConfirm| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
            let mut out = ctx.run_ui(input, |ui| {
                let theme = crate::theme::active();
                let _ = w.show(ui.ctx(), &theme, &tr, &info);
            });
            out.textures_delta.clear();
        };
        run(&ctx, &mut w);
        run(&ctx, &mut w);
        let first = ctx.memory(|m| m.area_rect(CssRemoveConfirm::id())).expect("shown");
        for _ in 0..120 {
            run(&ctx, &mut w);
            frames += 1;
            assert_eq!(ctx.memory(|m| m.area_rect(CssRemoveConfirm::id())), Some(first), "{lang:?}: the rectangle is kept");
        }
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ css_remove_confirm: the window\n│ 6 languages x 120 frames held\n│ time: {ms:.0} ms ({:.0} frames/sec)\n└─ 6/6 languages passed, 0 failed", frames as f64 / (ms / 1000.0));
}
