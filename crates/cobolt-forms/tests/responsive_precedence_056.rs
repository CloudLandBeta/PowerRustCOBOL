#![cfg(feature = "render")]
//! **Spec 056 T4.7 — a responsive form, every mechanism that moves controls,
//! one order (R26, R27, AC11, AC12), and the same answer on every engine
//! surface (R23, AC10).**
//!
//! One fixture, designed 600 × 400 and drawn at 800 × 500 (+200, +100), with
//! `FontScaling = Fluid` (factor 800 / 600 = 4/3):
//!
//! | control | designed | what places it |
//! |---|---|---|
//! | `BTN` Button | (500, 20, 80, 30) | `Anchor = Top,Right` → x 700 |
//! | `DOCK` Panel | (0, 360, 600, 40) | `Dock = Bottom` → (0, 460, 800, 40) |
//! | `AUTO` Label | (20, 20, 10, 10), AutoSize, 15 pt | measured at 20 pt BEFORE layout |
//! | `SPLIT` Splitter | (20, 60, 300, 200) | all four anchors → 500 × 300; its panes reflow on THAT rect |
//! | `P2-LBL` in pane 2 | — | carried by the pane (owner-positioned) |
//! | `P1-LBL` in pane 1 | `Anchor = Top,Right`, `Dock = Fill` | ignored: its owner places it |
//! | `CARD` repeating group, 2 items | (340, 60, 200, 60) | `Anchor = Top,Right` → x 540; instances step from THERE |
//! | `NAME` in `CARD` | `Anchor = Top,Right`, `Dock = Fill` | ignored: carried with the card |
//! | `BOX` Panel | (460, 280, 120, 60) | `Anchor = Bottom,Right` → (660, 380) |
//! | `IN` in `BOX` | (470, 290, 80, 20) | follows the laid-out box |
//!
//! A container moved by COBOL is not here: on a responsive form a geometry
//! write goes through the inverse mapping into the DESIGNED rect and the
//! layout re-runs (R38), so it is verified with T7.2, where that lands.

use std::collections::{BTreeMap, HashMap};

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::layout::apply::{prepare_with_rail, FormSpec, LaidOutState};
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{render_faces, render_form, render_form_with_chrome, FormState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, Form, PropValue};
use egui::{pos2, Rect, Vec2};

const DESIGNED: (f32, f32) = (600.0, 400.0);
const AVAILABLE: Vec2 = Vec2::new(800.0, 500.0);

fn ctrl(id: &str, ct: ControlType, r: (i32, i32, i32, i32), parent: Option<&str>) -> Control {
    let mut c = Control::new(id, ct, r.0, r.1);
    c.rect = MRect::new(r.0, r.1, r.2, r.3);
    c.parent = parent.map(str::to_owned);
    c
}

fn with(mut c: Control, key: &str, v: impl Into<PropValue>) -> Control {
    c.set_prop(key, v.into());
    c
}

fn fixture() -> (Vec<Control>, BTreeMap<String, PropValue>) {
    let mut form = Form::new("PREC", "Precedence", DESIGNED.0 as u32, DESIGNED.1 as u32);
    form.controls.push(with(ctrl("SPLIT", ControlType::Splitter, (20, 60, 300, 200), None), "Anchor", "Top,Bottom,Left,Right"));
    form.sync_splitter_panes();
    let pane1 = cobolt_forms::splitter::pane_id("SPLIT", 1);
    let pane2 = cobolt_forms::splitter::pane_id("SPLIT", 2);
    let p1 = form.find_control(&pane1).expect("pane 1").rect;
    let p2 = form.find_control(&pane2).expect("pane 2").rect;
    let mut p1_lbl = ctrl("P1-LBL", ControlType::Label, (p1.x + 10, p1.y + 10, 60, 20), Some(&pane1));
    p1_lbl = with(with(p1_lbl, "Anchor", "Top,Right"), "Dock", "Fill");
    form.controls.push(p1_lbl);
    form.controls.push(ctrl("P2-LBL", ControlType::Label, (p2.x + 10, p2.y + 10, 60, 20), Some(&pane2)));

    form.controls.push(with(ctrl("BTN", ControlType::Button, (500, 20, 80, 30), None), "Anchor", "Top,Right"));
    form.controls.push(with(ctrl("DOCK", ControlType::Panel, (0, 360, 600, 40), None), "Dock", "Bottom"));
    let auto = ctrl("AUTO", ControlType::Label, (20, 20, 10, 10), None);
    form.controls.push(with(with(with(auto, "AutoSize", true), "Caption", "Fluid type"), "FontSize", 15i64));

    let mut card = ctrl("CARD", ControlType::GroupBox, (340, 60, 200, 60), None);
    card = with(card, "Anchor", "Top,Right");
    card = with(card, "IsRepeatingGroup", true);
    card = with(card, "DataSource", "Customers");
    card = with(card, "ItemCount", 2i64);
    card = with(card, "LayoutDirection", "Vertical");
    card = with(card, "ItemSpacing", 10i64);
    form.controls.push(card);
    let name = ctrl("NAME", ControlType::Label, (350, 70, 80, 20), Some("CARD"));
    form.controls.push(with(with(name, "Anchor", "Top,Right"), "Dock", "Fill"));

    form.controls.push(with(ctrl("BOX", ControlType::Panel, (460, 280, 120, 60), None), "Anchor", "Bottom,Right"));
    form.controls.push(ctrl("IN", ControlType::Label, (470, 290, 80, 20), Some("BOX")));

    let layout = BTreeMap::from([("FontScaling".to_owned(), PropValue::String("Fluid".into()))]);
    (form.controls, layout)
}

/// The designed state, as the canvas's.
struct Designed;
impl FormState for Designed {}

#[derive(Clone, Copy, Debug)]
enum Surface {
    Canvas,
    Run,
    Preview,
}

/// One engine surface, two frames, the second kept: every control's rect and
/// the font size it was painted at.
fn draw(s: Surface, controls: &[Control], layout: &BTreeMap<String, PropValue>) -> (HashMap<String, Rect>, HashMap<String, f32>) {
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    let active = ActiveTabs::new();
    let spec = FormSpec { designed_size: DESIGNED, layout, breakpoints: &[] };
    let laid_state = LaidOutState { inner: &Designed };
    let (mut rects, mut fonts) = (HashMap::new(), HashMap::new());
    for _ in 0..2 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), AVAILABLE));
        input.max_texture_side = Some(8192);
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let p = prepare_with_rail(ui.ctx(), controls, &Designed, &spec, AVAILABLE, None);
                fonts = p
                    .controls
                    .iter()
                    .map(|c| (c.id.clone(), cobolt_forms::paint::ctrl_font_size(&laid_state.live(c))))
                    .collect();
                let inp = RenderInput {
                    controls: &p.controls,
                    state: &laid_state,
                    form_size: p.form_size,
                    glass: true,
                    mode: match s {
                        Surface::Canvas => RenderMode::Static,
                        _ => RenderMode::Interactive,
                    },
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                rects = match s {
                    Surface::Canvas => render_faces(&ui.painter().clone(), ui.min_rect().min, &inp).control_rects,
                    Surface::Run => render_form(ui, &inp).control_rects,
                    Surface::Preview => render_form_with_chrome(ui, &inp, None).control_rects,
                };
            });
        });
        full.textures_delta.clear();
    }
    (rects, fonts)
}

fn at(rects: &HashMap<String, Rect>, id: &str) -> (f32, f32, f32, f32) {
    let r = rects.get(id).unwrap_or_else(|| panic!("{id} drawn; drawn: {:?}", rects.keys().collect::<Vec<_>>()));
    (r.min.x, r.min.y, r.width(), r.height())
}

/// AC11 / R26 — every mechanism operates, in order, on the LAID-OUT rects.
#[test]
fn every_mechanism_operates_on_the_laid_out_rects_in_the_order_of_r26() {
    let (controls, layout) = fixture();
    let (r, fonts) = draw(Surface::Run, &controls, &layout);

    // (4) the layout itself.
    assert_eq!(at(&r, "BTN"), (700.0, 20.0, 80.0, 30.0), "Top,Right keeps 20 px from the right");
    assert_eq!(at(&r, "DOCK"), (0.0, 460.0, 800.0, 40.0), "Dock = Bottom spans the new width");

    // (2)+(3) AutoSize measured at the EFFECTIVE size: 15 pt × 4/3 = 20 pt.
    assert!((fonts["AUTO"] - 20.0).abs() < 0.01, "Fluid: 15 pt at 4/3 is 20 pt, got {}", fonts["AUTO"]);
    let measured = {
        let ctx = egui::Context::default();
        ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
        let mut out = (0.0, 0.0);
        let input = egui::RawInput { max_texture_side: Some(8192), ..Default::default() };
        let mut full = ctx.run_ui(input, |_| {
            let mut big = controls.iter().find(|c| c.id == "AUTO").unwrap().clone();
            big.set_prop("FontSize", PropValue::Int(20));
            let small = controls.iter().find(|c| c.id == "AUTO").unwrap();
            let b = cobolt_forms::paint::autosize_rect(&ctx, &big).unwrap();
            let s = cobolt_forms::paint::autosize_rect(&ctx, small).unwrap();
            out = (b.w as f32, s.w as f32);
        });
        full.textures_delta.clear();
        out
    };
    let auto = at(&r, "AUTO");
    assert_eq!(auto.2, measured.0, "the label is as wide as its caption at 20 pt, not at 15 pt ({})", measured.1);

    // (5a) Splitter panes reflow on the LAID-OUT splitter: 500 × 300 now.
    let split = at(&r, "SPLIT");
    assert_eq!((split.2, split.3), (500.0, 300.0));
    let pane1 = cobolt_forms::splitter::pane_id("SPLIT", 1);
    let pane2 = cobolt_forms::splitter::pane_id("SPLIT", 2);
    let (p1, p2) = (at(&r, &pane1), at(&r, &pane2));
    assert!(p1.2 > 200.0 && p2.2 > 200.0, "each pane takes about half of 500: {p1:?} {p2:?}");
    assert!((p2.0 + p2.2 - (split.0 + split.2)).abs() <= 2.0, "pane 2 ends at the laid-out splitter's right edge: {p2:?} in {split:?}");
    let p2_lbl = at(&r, "P2-LBL");
    assert_eq!((p2_lbl.0 - p2.0, p2_lbl.1 - p2.1), (10.0, 10.0), "a pane's control travels with its pane");

    // (5b) the repeating group instances step from the LAID-OUT template.
    let c1 = at(&r, "CARD.CARD-1");
    let c2 = at(&r, "CARD.CARD-2");
    assert_eq!((c1.0, c1.1), (540.0, 60.0), "the template is anchored Top,Right");
    assert_eq!((c2.0, c2.1), (540.0, 60.0 + 60.0 + 10.0), "instance 2 is one card + spacing below instance 1");

    // A container carries its child to where the layout put it.
    assert_eq!(at(&r, "BOX"), (660.0, 380.0, 120.0, 60.0), "anchored Bottom,Right");
    assert_eq!(at(&r, "IN"), (670.0, 390.0, 80.0, 20.0), "the child follows its container");
    println!(
        "056 T4.7 precedence at 800×500: BTN x {} · DOCK y {} w {} · AUTO {} wide at {} pt · SPLIT {}×{} → panes {} + {} · CARD-2 y {} · BOX y {} → IN y {}",
        at(&r, "BTN").0, at(&r, "DOCK").1, at(&r, "DOCK").2, auto.2, fonts["AUTO"], split.2, split.3, p1.2, p2.2, c2.1, at(&r, "BOX").1, at(&r, "IN").1
    );
}

/// AC12 / R27 — a Splitter pane's and a repeating group's contents with
/// `Anchor`/`Dock` set are placed by their owner, as if those were not set.
#[test]
fn owner_positioned_controls_ignore_their_layout_properties() {
    let (controls, layout) = fixture();
    let plain: Vec<Control> = controls
        .iter()
        .map(|c| {
            let mut c = c.clone();
            if matches!(c.id.as_str(), "P1-LBL" | "NAME") {
                let _ = c.properties.remove("Anchor");
                let _ = c.properties.remove("Dock");
            }
            c
        })
        .collect();
    let (with_props, _) = draw(Surface::Run, &controls, &layout);
    let (without, _) = draw(Surface::Run, &plain, &layout);
    for id in ["P1-LBL", "CARD.CARD-1.NAME", "CARD.CARD-2.NAME"] {
        assert_eq!(at(&with_props, id), at(&without, id), "{id}: its owner places it, its Anchor/Dock do not");
    }
    // Where the pane's own rule puts it: the designed pane 1 → pane 1 of the
    // laid-out splitter, under the pane's `ResizeBehavior` — as a divider drag
    // or a COBOL resize of the splitter moves it.
    let split = controls.iter().find(|c| c.id == "SPLIT").unwrap();
    let pane1_ctrl = controls.iter().find(|c| c.id == cobolt_forms::splitter::pane_id("SPLIT", 1)).unwrap();
    let designed_lbl = controls.iter().find(|c| c.id == "P1-LBL").unwrap().rect;
    let laid_split = at(&with_props, "SPLIT");
    let laid_split = MRect::new(laid_split.0 as i32, laid_split.1 as i32, laid_split.2 as i32, laid_split.3 as i32);
    let expected = cobolt_forms::splitter::reflow_in_subtree(
        cobolt_forms::splitter::PaneResize::of(pane1_ctrl),
        1,
        cobolt_forms::splitter::geometry(split, split.rect).pane1,
        cobolt_forms::splitter::geometry(split, laid_split).pane1,
        designed_lbl,
        designed_lbl,
        cobolt_forms::splitter::is_horizontal(split),
    );
    let pane1 = at(&with_props, &cobolt_forms::splitter::pane_id("SPLIT", 1));
    let lbl = at(&with_props, "P1-LBL");
    assert_eq!(
        lbl,
        (expected.x as f32, expected.y as f32, expected.w as f32, expected.h as f32),
        "not docked Fill, not pinned right: where its pane's ResizeBehavior puts it"
    );
    println!("056 T4.7 R27: P1-LBL {lbl:?} in pane 1 {pane1:?}; NAME {:?}", at(&with_props, "CARD.CARD-1.NAME"));
}

/// AC10 (engine half) — canvas, run form and preview draw the same laid-out
/// form: identical rects and identical effective font sizes. The repeating
/// group is left out: the canvas draws its designed template, the run form its
/// run-time instances, responsive or not.
#[test]
fn the_canvas_the_run_form_and_the_preview_agree() {
    let (mut controls, layout) = fixture();
    controls.retain(|c| !matches!(c.id.as_str(), "CARD" | "NAME"));
    let (run, run_fonts) = draw(Surface::Run, &controls, &layout);
    for s in [Surface::Canvas, Surface::Preview] {
        let (other, other_fonts) = draw(s, &controls, &layout);
        let mut ids: Vec<&String> = run.keys().collect();
        ids.sort();
        for id in ids {
            assert_eq!(other.get(id), run.get(id), "{s:?}: {id} differs from the run form");
        }
        assert_eq!(other.len(), run.len(), "{s:?}: the same controls are drawn");
        assert_eq!(other_fonts, run_fonts, "{s:?}: the same effective font sizes");
    }
    println!("056 T4.7 AC10 engine: {} rects and {} font sizes identical on canvas, run form and preview", run.len(), run_fonts.len());
}
