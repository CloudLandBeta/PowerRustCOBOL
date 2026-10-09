#![cfg(feature = "render")]
//! Spec 091, slice 2a — what the ONE render engine does with layers, on every
//! path that paints a form: the interactive form, the static form, and the
//! designer's canvas (`render_faces`).
//!
//! Stacking (R9, R10), a hidden layer painting nothing (R12), a layer's own
//! background sitting between its neighbours (R15), a new layer changing nothing
//! that is drawn (R15), the passes that paint over controls running per layer,
//! and the keyboard walking the base before the layers (Q10).
//!
//! The paint ORDER is read from the shape list the engine hands egui — shapes
//! are painted in the order they are listed — so these tests name the control
//! whose text they look for, never a pixel.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, Layer, PropValue};
use cobolt_forms::render::{
    render_faces, render_form, Backdrop, DesignedState, FormState, RenderInput, RenderMode,
};
use egui::{pos2, Color32, Rect, Shape, Vec2};

const FORM: (f32, f32) = (600.0, 400.0);

/// A state that shows exactly the layers it names.
struct Shown(&'static [&'static str]);
impl FormState for Shown {
    fn layer_visible(&self, name: &str) -> bool {
        self.0.iter().any(|n| n.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Copy, Debug)]
enum Path {
    /// `render_form`, interactive: the running form.
    Run,
    /// `render_form`, static: the preview and the compiled binary's faces.
    Static,
    /// `render_faces`: the designer canvas.
    Canvas,
}
const PATHS: [Path; 3] = [Path::Run, Path::Static, Path::Canvas];

fn layer(name: &str, color: &str) -> Layer {
    let mut l = Layer::new(name);
    l.backdrop.color = color.into();
    l
}

fn label(id: &str, text: &str, layer: Option<&str>, z: i32) -> Control {
    let mut c = Control::new(id, ControlType::Label, 20, 20);
    c.rect = cobolt_forms::model::Rect::new(20, 20, 200, 30);
    c.set_prop("Caption", PropValue::String(text.into()));
    c.layer = layer.map(str::to_owned);
    c.z_order = z;
    c
}

fn flatten(shapes: &[egui::epaint::ClippedShape]) -> Vec<Shape> {
    fn walk(s: &Shape, out: &mut Vec<Shape>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            other => out.push(other.clone()),
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

/// Every shape one frame paints, in paint order.
fn paint(
    controls: &[Control],
    layers: Vec<Layer>,
    state: &dyn FormState,
    path: Path,
) -> Vec<Shape> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(FORM.0, FORM.1)));
    let mut full = ctx.run_ui(raw, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                let inp = RenderInput {
                    controls,
                    state,
                    form_size: Vec2::new(FORM.0, FORM.1),
                    glass: false,
                    mode: match path {
                        Path::Run => RenderMode::Interactive,
                        _ => RenderMode::Static,
                    },
                    active_tabs: &active,
                    backdrop: Backdrop { paint: false, layers: layers.clone(), ..Default::default() },
                };
                match path {
                    Path::Run | Path::Static => {
                        let _ = render_form(ui, &inp);
                    }
                    Path::Canvas => {
                        let painter = ui.painter().clone();
                        let _ = render_faces(&painter, pos2(0.0, 0.0), &inp);
                    }
                }
            });
    });
    full.textures_delta.clear();
    flatten(&full.shapes)
}

/// Where in the paint order the text `needle` is painted, if it is.
fn text_at(shapes: &[Shape], needle: &str) -> Option<usize> {
    shapes.iter().position(|s| match s {
        Shape::Text(t) => t.galley.text().contains(needle),
        _ => false,
    })
}

/// Where in the paint order a rectangle filled with `color` and as big as the
/// form is painted — a layer's background.
fn backdrop_at(shapes: &[Shape], color: Color32) -> Option<usize> {
    shapes.iter().position(|s| match s {
        Shape::Rect(r) => {
            r.fill == color && r.rect.width() >= FORM.0 - 1.0 && r.rect.height() >= FORM.1 - 1.0
        }
        _ => false,
    })
}

const RED: Color32 = Color32::from_rgba_premultiplied(255, 0, 0, 255);

// ── Stacking (R9, R10; AC5 painting half; parity across the paths) ──────────

#[test]
fn a_higher_layer_paints_after_a_lower_one_whatever_the_z_order_on_every_path_091() {
    let controls = [
        // Listed in an order that matches neither the layers nor the z_order.
        label("L2", "TXT-CCC", Some("Layer-2"), -5),
        label("BASE", "TXT-AAA", None, 209),
        label("L1", "TXT-BBB", Some("Layer-1"), 0),
    ];
    let layers = || vec![layer("Layer-1", "#00000000"), layer("Layer-2", "#00000000")];
    eprintln!("\n  path     base   Layer-1   Layer-2   (position in the paint order)");
    for path in PATHS {
        let shapes = paint(&controls, layers(), &DesignedState, path);
        let (a, b, c) = (
            text_at(&shapes, "TXT-AAA").unwrap_or_else(|| panic!("{path:?}: base not painted")),
            text_at(&shapes, "TXT-BBB").unwrap_or_else(|| panic!("{path:?}: Layer-1 not painted")),
            text_at(&shapes, "TXT-CCC").unwrap_or_else(|| panic!("{path:?}: Layer-2 not painted")),
        );
        eprintln!("  {:<7}  {a:>4}   {b:>7}   {c:>7}", format!("{path:?}"));
        assert!(
            a < b && b < c,
            "{path:?}: the base (z 209) must paint before Layer-1 (z 0) before Layer-2 (z -5): {a} {b} {c}"
        );
    }
}

#[test]
fn a_form_whose_controls_name_layers_it_does_not_define_paints_by_z_order_alone_091() {
    // No layer table: the same controls are R40 "undefined layer" controls, shown
    // in the base, so only z_order orders them — the form with no layers.
    let controls = [
        label("L2", "TXT-CCC", Some("Layer-2"), -5),
        label("BASE", "TXT-AAA", None, 209),
        label("L1", "TXT-BBB", Some("Layer-1"), 0),
    ];
    for path in PATHS {
        let shapes = paint(&controls, Vec::new(), &DesignedState, path);
        let (a, b, c) = (
            text_at(&shapes, "TXT-AAA").unwrap(),
            text_at(&shapes, "TXT-BBB").unwrap(),
            text_at(&shapes, "TXT-CCC").unwrap(),
        );
        assert!(c < b && b < a, "{path:?}: z_order only: -5, 0, 209 — got {c} {b} {a}");
    }
}

// ── A hidden layer paints nothing (R12, AC6) ────────────────────────────────

#[test]
fn a_hidden_layer_paints_neither_its_background_nor_its_controls_091() {
    let controls = [
        label("BASE", "TXT-AAA", None, 0),
        label("L1", "TXT-BBB", Some("Layer-1"), 0),
        label("L2", "TXT-CCC", Some("Layer-2"), 0),
    ];
    let layers = || vec![layer("Layer-1", "#FF0000FF"), layer("Layer-2", "#00000000")];
    for path in PATHS {
        // Everything shown: all three texts and the red background.
        let all = paint(&controls, layers(), &Shown(&["Layer-1", "Layer-2"]), path);
        assert!(text_at(&all, "TXT-BBB").is_some(), "{path:?}");
        assert!(backdrop_at(&all, RED).is_some(), "{path:?}: Layer-1's background");

        // Layer-1 hidden: its text AND its background are gone, the others stay.
        let hid = paint(&controls, layers(), &Shown(&["Layer-2"]), path);
        assert!(text_at(&hid, "TXT-AAA").is_some(), "{path:?}: the base is never hidden");
        assert!(text_at(&hid, "TXT-BBB").is_none(), "{path:?}: a hidden layer's control");
        assert!(backdrop_at(&hid, RED).is_none(), "{path:?}: a hidden layer's background");
        assert!(text_at(&hid, "TXT-CCC").is_some(), "{path:?}: a layer above stays");

        // Every layer hidden — how a running form starts (R35).
        let none = paint(&controls, layers(), &Shown(&[]), path);
        assert!(text_at(&none, "TXT-AAA").is_some(), "{path:?}");
        assert!(text_at(&none, "TXT-BBB").is_none() && text_at(&none, "TXT-CCC").is_none(), "{path:?}");
    }
}

#[test]
fn a_container_in_a_hidden_layer_takes_its_children_with_it_091() {
    let mut panel = Control::new("PNL", ControlType::Panel, 0, 0);
    panel.rect = cobolt_forms::model::Rect::new(0, 0, 300, 100);
    panel.layer = Some("Layer-1".into());
    let mut kid = label("KID", "TXT-KID", None, 0);
    kid.parent = Some("PNL".into());
    let controls = [panel, kid];
    for path in PATHS {
        let shown = paint(&controls, vec![layer("Layer-1", "#00000000")], &Shown(&["Layer-1"]), path);
        assert!(text_at(&shown, "TXT-KID").is_some(), "{path:?}: shown");
        let hidden = paint(&controls, vec![layer("Layer-1", "#00000000")], &Shown(&[]), path);
        assert!(text_at(&hidden, "TXT-KID").is_none(), "{path:?}: a child follows its container's layer (R8)");
    }
}

// ── A layer's own background (R15, AC8) ─────────────────────────────────────

#[test]
fn a_layer_background_paints_above_the_layers_below_and_under_its_own_controls_091() {
    let controls = [
        label("BASE", "TXT-AAA", None, 0),
        label("L1", "TXT-BBB", Some("Layer-1"), 0),
    ];
    for path in PATHS {
        let shapes = paint(&controls, vec![layer("Layer-1", "#FF0000FF")], &DesignedState, path);
        let (base, scrim, own) = (
            text_at(&shapes, "TXT-AAA").unwrap(),
            backdrop_at(&shapes, RED).unwrap_or_else(|| panic!("{path:?}: no background")),
            text_at(&shapes, "TXT-BBB").unwrap(),
        );
        assert!(
            base < scrim && scrim < own,
            "{path:?}: base text {base}, Layer-1 background {scrim}, Layer-1 text {own}"
        );
    }
}

#[test]
fn a_layer_with_no_control_still_paints_its_background_in_its_place_091() {
    let controls = [
        label("BASE", "TXT-AAA", None, 0),
        label("TOP", "TXT-CCC", Some("Layer-3"), 0),
    ];
    // Layer-1 sits between the base and the layer that holds a control; Layer-2
    // above the last control, and neither holds anything.
    let layers = vec![
        layer("Layer-1", "#FF0000FF"),
        layer("Layer-2", "#0000FFFF"),
        layer("Layer-3", "#00000000"),
        layer("Layer-4", "#00FF00FF"),
    ];
    let blue = Color32::from_rgba_premultiplied(0, 0, 255, 255);
    let green = Color32::from_rgba_premultiplied(0, 255, 0, 255);
    for path in PATHS {
        let s = paint(&controls, layers.clone(), &DesignedState, path);
        let (base, red, blu, top, grn) = (
            text_at(&s, "TXT-AAA").unwrap(),
            backdrop_at(&s, RED).unwrap_or_else(|| panic!("{path:?}: Layer-1 background")),
            backdrop_at(&s, blue).unwrap_or_else(|| panic!("{path:?}: Layer-2 background")),
            text_at(&s, "TXT-CCC").unwrap(),
            backdrop_at(&s, green).unwrap_or_else(|| panic!("{path:?}: Layer-4 background")),
        );
        assert!(
            base < red && red < blu && blu < top && top < grn,
            "{path:?}: base {base} < L1 bg {red} < L2 bg {blu} < L3 text {top} < L4 bg {grn}"
        );
    }
}

/// What a frame DRAWS, ignoring fills that draw nothing: a signature per shape.
fn drawn(shapes: &[Shape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if r.fill.a() == 0 && r.stroke.width == 0.0 => None,
            Shape::Rect(r) => Some(format!("rect {:?} {:?} {:?}", r.rect, r.fill, r.stroke)),
            Shape::Text(t) => Some(format!("text {:?} {:?}", t.pos, t.galley.text())),
            other => Some(format!("{:?}", other.visual_bounding_rect())),
        })
        .collect()
}

#[test]
fn a_new_layer_changes_nothing_that_is_drawn_091() {
    let controls = [
        label("BASE", "TXT-AAA", None, 0),
        label("L1", "TXT-BBB", Some("Layer-1"), 0),
    ];
    // The same controls as a form with no layers would show them in the base…
    let plain: Vec<Control> = controls
        .iter()
        .cloned()
        .map(|mut c| {
            c.layer = None;
            c
        })
        .collect();
    for path in PATHS {
        let none = paint(&plain, Vec::new(), &DesignedState, path);
        // …and with the layer added, fully transparent, as `Layer::new` makes it.
        let with = paint(&controls, vec![Layer::new("Layer-1")], &DesignedState, path);
        assert_eq!(
            drawn(&with),
            drawn(&none),
            "{path:?}: a transparent layer must draw exactly what no layer draws"
        );
    }
}

#[test]
fn a_layer_background_obeys_its_transparency_091() {
    let controls = [label("BASE", "TXT-AAA", None, 0)];
    let mut half = layer("Layer-1", "#FF0000FF");
    half.backdrop.transparency = 50;
    for path in PATHS {
        let s = paint(&controls, vec![half.clone()], &DesignedState, path);
        // 50 % of an opaque red: premultiplied (127, 0, 0, 127).
        let found = s.iter().any(|sh| match sh {
            Shape::Rect(r) => r.fill.r() > 100 && r.fill.r() < 140 && r.fill.a() == r.fill.r(),
            _ => false,
        });
        assert!(found, "{path:?}: a half-transparent red background");
        assert!(backdrop_at(&s, RED).is_none(), "{path:?}: not the opaque one");
    }
}

// ── The passes that paint over controls run per layer (spec 090 / 017) ──────

#[test]
fn a_base_groupbox_caption_does_not_land_above_a_layer_control_091() {
    let mut group = Control::new("GRP", ControlType::GroupBox, 10, 10);
    group.rect = cobolt_forms::model::Rect::new(10, 10, 300, 200);
    group.set_prop("Caption", PropValue::String("TXT-CAPTION".into()));
    // A layer control right over the group's caption.
    let over = label("OVER", "TXT-OVER", Some("Layer-1"), 0);
    let controls = [group, over];

    // The group's caption is a deferred pass — painted after the controls —
    // so with no layers it follows `OVER`; with a layer the base's pass runs
    // when the base ends, UNDER the layer's control.
    for path in PATHS {
        let layered = paint(&controls, vec![layer("Layer-1", "#00000000")], &DesignedState, path);
        let (cap, over) = (
            text_at(&layered, "TXT-CAPTION").unwrap_or_else(|| panic!("{path:?}: caption")),
            text_at(&layered, "TXT-OVER").unwrap(),
        );
        assert!(cap < over, "{path:?}: the base's caption ({cap}) must not paint over Layer-1's control ({over})");

        let mut flat = controls.clone();
        flat[1].layer = None;
        let plain = paint(&flat, Vec::new(), &DesignedState, path);
        let (cap, over) = (text_at(&plain, "TXT-CAPTION").unwrap(), text_at(&plain, "TXT-OVER").unwrap());
        assert!(over < cap, "{path:?}: unchanged for a form with no layers — the caption is deferred ({cap} after {over})");
    }
}

// ── The keyboard walks the base, then each layer upward (Q10) ───────────────

fn textbox(id: &str, layer: Option<&str>, tab_order: u32, y: i32) -> Control {
    let mut c = Control::new(id, ControlType::TextBox, 10, y);
    c.rect = cobolt_forms::model::Rect::new(10, y, 200, 24);
    c.tab_order = tab_order;
    c.layer = layer.map(str::to_owned);
    c
}

/// The controls that took the focus, in order, as Tab is pressed `presses` times.
fn tab_walk(controls: &[Control], layers: Vec<Layer>, state: &dyn FormState, presses: usize) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let tab = |pressed: bool| egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    };
    let mut frames: Vec<Vec<egui::Event>> = vec![vec![]];
    for _ in 0..presses {
        frames.push(vec![tab(true)]);
        frames.push(vec![tab(false)]);
        frames.push(vec![]);
    }
    let mut focused = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(FORM.0, FORM.1)));
        raw.focused = true;
        raw.time = Some(i as f64 * 0.05);
        raw.events = events;
        ctx.run_ui(raw, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(FORM.0, FORM.1));
                    let inp = RenderInput {
                        controls,
                        state,
                        form_size: Vec2::new(FORM.0, FORM.1),
                        glass: false,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop { paint: false, layers: layers.clone(), ..Default::default() },
                    };
                    let out = render_form(ui, &inp);
                    for e in out.events.iter().filter(|e| e.event == "onGotFocus") {
                        focused.push(e.ctrl_id.clone());
                    }
                });
        })
        .textures_delta
        .clear();
    }
    focused
}

#[test]
fn tab_walks_the_base_first_then_each_layer_from_the_lowest_up_091() {
    // The layers' text boxes have the LOWEST TabOrder, so a walk that ignored
    // layers would start in a layer.
    let controls = [
        textbox("L2-TB", Some("Layer-2"), 0, 150),
        textbox("L1-TB", Some("Layer-1"), 1, 100),
        textbox("BASE-1", None, 5, 10),
        textbox("BASE-2", None, 6, 50),
    ];
    let layers = || vec![layer("Layer-1", "#00000000"), layer("Layer-2", "#00000000")];
    let walk = tab_walk(&controls, layers(), &Shown(&["Layer-1", "Layer-2"]), 5);
    eprintln!("\n  Tab walk: {walk:?}");
    assert_eq!(walk, ["BASE-1", "BASE-2", "L1-TB", "L2-TB", "BASE-1"]);
}

#[test]
fn tab_skips_the_controls_of_a_hidden_layer_091() {
    let controls = [
        textbox("L1-TB", Some("Layer-1"), 0, 100),
        textbox("BASE-1", None, 5, 10),
        textbox("BASE-2", None, 6, 50),
        textbox("L2-TB", Some("Layer-2"), 0, 150),
    ];
    let layers = || vec![layer("Layer-1", "#00000000"), layer("Layer-2", "#00000000")];
    let walk = tab_walk(&controls, layers(), &Shown(&["Layer-2"]), 4);
    eprintln!("\n  Tab walk, Layer-1 hidden: {walk:?}");
    assert_eq!(walk, ["BASE-1", "BASE-2", "L2-TB", "BASE-1"], "Layer-1's box is never reached");
}

// ── A layer is cut at the form's edge (R13, R14, AC7 straight-edge half) ─────

/// Every shape a frame paints with the clip it was painted under: what the
/// screen actually shows of a shape is its bounds INSIDE that clip.
fn painted_with_clip(
    controls: &[Control],
    layers: Vec<Layer>,
    path: Path,
    screen: (f32, f32),
) -> Vec<(Shape, Rect)> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(screen.0, screen.1)));
    let mut full = ctx.run_ui(raw, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: Vec2::new(FORM.0, FORM.1),
                    glass: false,
                    mode: match path {
                        Path::Run => RenderMode::Interactive,
                        _ => RenderMode::Static,
                    },
                    active_tabs: &active,
                    backdrop: Backdrop { paint: false, layers: layers.clone(), ..Default::default() },
                };
                match path {
                    Path::Run | Path::Static => {
                        let _ = render_form(ui, &inp);
                    }
                    Path::Canvas => {
                        let painter = ui.painter().clone();
                        let _ = render_faces(&painter, pos2(0.0, 0.0), &inp);
                    }
                }
            });
    });
    full.textures_delta.clear();
    fn walk(s: &Shape, clip: Rect, out: &mut Vec<(Shape, Rect)>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            other => out.push((other.clone(), clip)),
        }
    }
    let mut out = Vec::new();
    for cs in &full.shapes {
        walk(&cs.shape, cs.clip_rect, &mut out);
    }
    out
}

/// The furthest right any shape of the frame reaches ONCE CLIPPED.
fn furthest_right(shapes: &[(Shape, Rect)]) -> f32 {
    shapes
        .iter()
        .filter_map(|(s, clip)| {
            let b = s.visual_bounding_rect();
            (b.is_finite() && b.is_positive()).then(|| b.intersect(*clip).max.x)
        })
        .filter(|x| x.is_finite())
        .fold(f32::MIN, f32::max)
}

fn straddling(ct: &ControlType, layer: Option<&str>) -> Control {
    let mut c = Control::new("STRADDLE", ct.clone(), 500, 100);
    c.rect = cobolt_forms::model::Rect::new(500, 100, 200, 120);
    c.layer = layer.map(str::to_owned);
    c
}

#[test]
fn a_layer_control_of_every_type_is_cut_at_the_form_s_edge_091() {
    // The screen is bigger than the form, so the base — which may use the room a
    // bigger window offers — can show a control past the form's edge, and the
    // layer must not.
    let screen = (900.0, 600.0);
    let edge = FORM.0 + 0.5;
    let types: Vec<ControlType> = ControlType::ALL.to_vec();
    assert!(types.len() >= 40, "the whole catalogue: {}", types.len());
    let mut base_bleeds = 0usize;
    let mut leaks: Vec<String> = Vec::new();
    for ct in &types {
        for path in PATHS {
            let layered = painted_with_clip(
                &[straddling(ct, Some("Layer-1"))],
                vec![Layer::new("Layer-1")],
                path,
                screen,
            );
            let x = furthest_right(&layered);
            if x > edge {
                leaks.push(format!("{} on {path:?}: reaches x = {x:.1}", ct.as_str()));
            }
        }
        // The same control in the base, on the canvas: the proof the check can
        // see a bleed at all.
        let base = painted_with_clip(&[straddling(ct, None)], Vec::new(), Path::Canvas, screen);
        if furthest_right(&base) > edge {
            base_bleeds += 1;
        }
    }
    eprintln!(
        "\n  {} control types x 3 paths in a layer: {} reach past x = {} ({base_bleeds} of the same types DO in the base)\n",
        types.len(),
        leaks.len(),
        FORM.0
    );
    assert!(
        base_bleeds >= 20,
        "the check must be able to see a bleed: only {base_bleeds} types painted past the edge in the base"
    );
    assert!(leaks.is_empty(), "a layer paints outside the form's rectangle:\n  {}", leaks.join("\n  "));
}

#[test]
fn a_form_with_layers_and_no_control_still_paints_the_layers_091() {
    // Found by the rounded-window harness: the "are there layers?" switch was
    // read from the controls' ranks, which are empty when there are no controls.
    let layers = vec![layer("Layer-1", "#FF0000FF"), layer("Layer-2", "#0000FFFF")];
    let blue = Color32::from_rgba_premultiplied(0, 0, 255, 255);
    for path in PATHS {
        let s = paint(&[], layers.clone(), &DesignedState, path);
        let (r, b) = (
            backdrop_at(&s, RED).unwrap_or_else(|| panic!("{path:?}: Layer-1's background")),
            backdrop_at(&s, blue).unwrap_or_else(|| panic!("{path:?}: Layer-2's background")),
        );
        assert!(r < b, "{path:?}: Layer-2 paints above Layer-1 ({r} then {b})");
        let hidden = paint(&[], layers.clone(), &Shown(&["Layer-2"]), path);
        assert!(backdrop_at(&hidden, RED).is_none() && backdrop_at(&hidden, blue).is_some(), "{path:?}");
    }
}

// ── The layers hold the pointer (R17–R20, AC9) ───────────────────────────────

fn bound(mut c: Control, event: &str) -> Control {
    c.events.push(cobolt_forms::model::EventBinding {
        event: event.into(),
        paragraph: format!("{}-{}", c.id, event),
        code: "DISPLAY \"x\"".into(),
    });
    c
}

fn button_at(id: &str, layer: Option<&str>, r: (i32, i32, i32, i32), handler: bool) -> Control {
    let mut c = Control::new(id, ControlType::Button, r.0, r.1);
    c.rect = cobolt_forms::model::Rect::new(r.0, r.1, r.2, r.3);
    c.layer = layer.map(str::to_owned);
    if handler { bound(c, "onClick") } else { c }
}

/// The base button every scenario clicks, and the point it is clicked at.
const BASE_BTN: (i32, i32, i32, i32) = (300, 200, 120, 40);
const CLICK: (f32, f32) = (360.0, 220.0);

/// Click at `p`, the way a user does — a few frames to let the form settle, the
/// pointer moves, the button goes down, the button goes up — and report every
/// event the form raised as `(control, event)`.
fn click_at(
    controls: &[Control],
    layers: Vec<Layer>,
    state: &dyn FormState,
    p: (f32, f32),
) -> Vec<(String, String)> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let at = pos2(p.0, p.1);
    let button = |pressed: bool| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let frames: Vec<Vec<egui::Event>> = vec![
        vec![],
        vec![],
        vec![],
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerMoved(at), button(true)],
        vec![egui::Event::PointerMoved(at), button(false)],
        vec![],
    ];
    let mut seen = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(FORM.0, FORM.1)));
        raw.focused = true;
        raw.time = Some(i as f64 * 0.05);
        raw.events = events;
        ctx.run_ui(raw, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(FORM.0, FORM.1));
                    let inp = RenderInput {
                        controls,
                        state,
                        form_size: Vec2::new(FORM.0, FORM.1),
                        glass: false,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop { paint: false, layers: layers.clone(), ..Default::default() },
                    };
                    let out = render_form(ui, &inp);
                    for e in out.events {
                        seen.push((e.ctrl_id, e.event));
                    }
                });
        })
        .textures_delta
        .clear();
    }
    seen
}

fn clicks(seen: &[(String, String)]) -> Vec<&str> {
    seen.iter().filter(|(_, e)| e == "onClick").map(|(c, _)| c.as_str()).collect()
}

fn two_layers() -> Vec<Layer> {
    vec![layer("Layer-1", "#00000000"), layer("Layer-2", "#00000000")]
}

#[test]
fn a_click_on_the_empty_area_of_a_layer_reaches_the_button_below_091() {
    // R18: a layer with a Label whose text is far from the button, and nothing
    // else — a click over the base button lands on nothing the layer paints.
    let mut note = Control::new("NOTE", ControlType::Label, 10, 10);
    note.rect = cobolt_forms::model::Rect::new(10, 10, 200, 24);
    note.layer = Some("Layer-1".into());
    let controls = [button_at("BASE-BTN", None, BASE_BTN, true), note];
    let seen = click_at(&controls, two_layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK);
    assert_eq!(clicks(&seen), ["BASE-BTN"], "{seen:?}");

    // The control case: no layers at all.
    let alone = click_at(&[button_at("BASE-BTN", None, BASE_BTN, true)], Vec::new(), &DesignedState, CLICK);
    assert_eq!(clicks(&alone), ["BASE-BTN"], "{alone:?}");
}

#[test]
fn a_click_on_a_layer_button_runs_only_its_handler_091() {
    let controls = [
        button_at("BASE-BTN", None, BASE_BTN, true),
        button_at("LAYER-BTN", Some("Layer-1"), BASE_BTN, true),
    ];
    let seen = click_at(&controls, two_layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK);
    assert_eq!(clicks(&seen), ["LAYER-BTN"], "R19: the control painted above takes the click: {seen:?}");
}

#[test]
fn a_click_on_a_layer_control_with_no_handler_runs_nothing_and_not_the_one_below_091() {
    let controls = [
        button_at("BASE-BTN", None, BASE_BTN, true),
        button_at("LAYER-BTN", Some("Layer-1"), BASE_BTN, false),
    ];
    let seen = click_at(&controls, two_layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK);
    assert!(clicks(&seen).is_empty(), "R19: discarded, not passed down: {seen:?}");
}

#[test]
fn a_passive_control_of_a_layer_keeps_the_click_where_it_paints_091() {
    // A Panel with a face, no handler, over the base button.
    let mut card = Control::new("CARD", ControlType::Panel, 280, 180, );
    card.rect = cobolt_forms::model::Rect::new(280, 180, 200, 100);
    card.layer = Some("Layer-1".into());
    card.set_prop("BackgroundColor", PropValue::String("#3060C0FF".into()));
    let controls = [button_at("BASE-BTN", None, BASE_BTN, true), card];
    let seen = click_at(&controls, two_layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK);
    assert!(clicks(&seen).is_empty(), "R19: a painted Panel owns the click: {seen:?}");

    // Beside the card the base button is still reachable.
    let beside = click_at(
        &[button_at("BASE-BTN", None, (10, 10, 120, 40), true), {
            let mut c = Control::new("CARD", ControlType::Panel, 280, 180);
            c.rect = cobolt_forms::model::Rect::new(280, 180, 200, 100);
            c.layer = Some("Layer-1".into());
            c.set_prop("BackgroundColor", PropValue::String("#3060C0FF".into()));
            c
        }],
        two_layers(),
        &Shown(&["Layer-1", "Layer-2"]),
        (60.0, 30.0),
    );
    assert_eq!(clicks(&beside), ["BASE-BTN"], "{beside:?}");
}

#[test]
fn a_click_beside_a_transparent_label_s_text_reaches_the_button_091() {
    // The label's rectangle covers the button; only its text (top-left) paints.
    let mut note = label("NOTE", "TXT", Some("Layer-1"), 0);
    note.rect = cobolt_forms::model::Rect::new(280, 180, 200, 100);
    let controls = [button_at("BASE-BTN", None, BASE_BTN, true), note];
    let seen = click_at(&controls, two_layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK);
    assert_eq!(clicks(&seen), ["BASE-BTN"], "R18: the Label paints its text only: {seen:?}");
}

#[test]
fn an_opaque_layer_background_shields_what_is_below_and_a_translucent_one_does_not_091() {
    let base_btn = || button_at("BASE-BTN", None, BASE_BTN, true);
    // Opaque: nothing below it gets the click, and the background itself is
    // never the target of an event (no handler can be bound to it).
    let opaque = vec![layer("Layer-1", "#FFFFFFFF")];
    let seen = click_at(&[base_btn()], opaque.clone(), &Shown(&["Layer-1"]), CLICK);
    assert!(seen.iter().all(|(_, e)| e != "onClick"), "R20: shielded: {seen:?}");
    assert!(seen.iter().all(|(c, _)| c != "Layer-1"), "no event names the background: {seen:?}");

    // …but a control of that layer, painted above the background, still gets
    // its own click.
    let over = button_at("LAYER-BTN", Some("Layer-1"), BASE_BTN, true);
    let seen = click_at(&[base_btn(), over], opaque, &Shown(&["Layer-1"]), CLICK);
    assert_eq!(clicks(&seen), ["LAYER-BTN"], "{seen:?}");

    // The same background with a little transparency lets the event through.
    let mut see_through = layer("Layer-1", "#FFFFFFFF");
    see_through.backdrop.transparency = 30;
    let seen = click_at(&[base_btn()], vec![see_through], &Shown(&["Layer-1"]), CLICK);
    assert_eq!(clicks(&seen), ["BASE-BTN"], "Q8b: translucent does not shield: {seen:?}");

    // A colour with alpha below 255 lets it through too.
    let seen = click_at(&[base_btn()], vec![layer("Layer-1", "#FFFFFF80")], &Shown(&["Layer-1"]), CLICK);
    assert_eq!(clicks(&seen), ["BASE-BTN"], "{seen:?}");
}

#[test]
fn a_hidden_layer_holds_nothing_091() {
    let controls = [
        button_at("BASE-BTN", None, BASE_BTN, true),
        button_at("LAYER-BTN", Some("Layer-1"), BASE_BTN, true),
    ];
    let hidden = click_at(&controls, vec![layer("Layer-1", "#FFFFFFFF")], &Shown(&[]), CLICK);
    assert_eq!(clicks(&hidden), ["BASE-BTN"], "R12: a hidden layer takes no event, opaque or not: {hidden:?}");
}

#[test]
fn the_same_rules_hold_with_three_layers_091() {
    let layers = || {
        vec![
            layer("Layer-1", "#00000000"),
            layer("Layer-2", "#00000000"),
            layer("Layer-3", "#00000000"),
        ]
    };
    let all = Shown(&["Layer-1", "Layer-2", "Layer-3"]);
    // A button in each of Layer-1 and Layer-3 over the base's; Layer-2 holds
    // nothing there. The highest painted control takes it.
    let controls = [
        button_at("BASE-BTN", None, BASE_BTN, true),
        button_at("L1-BTN", Some("Layer-1"), BASE_BTN, true),
        button_at("L3-BTN", Some("Layer-3"), BASE_BTN, true),
    ];
    assert_eq!(clicks(&click_at(&controls, layers(), &all, CLICK)), ["L3-BTN"]);
    // Layer-3 hidden: Layer-1's button is the highest painted one.
    assert_eq!(
        clicks(&click_at(&controls, layers(), &Shown(&["Layer-1", "Layer-2"]), CLICK)),
        ["L1-BTN"]
    );
    // An opaque Layer-2 shields Layer-1 and the base from the click, but not
    // Layer-3's button above it.
    let mut opaque_mid = layers();
    opaque_mid[1] = layer("Layer-2", "#101010FF");
    assert_eq!(clicks(&click_at(&controls, opaque_mid.clone(), &all, CLICK)), ["L3-BTN"]);
    assert_eq!(
        clicks(&click_at(&controls, opaque_mid, &Shown(&["Layer-1", "Layer-2"]), CLICK)),
        Vec::<&str>::new(),
        "R20: with Layer-3 hidden the opaque Layer-2 shields both below it"
    );
}

#[test]
fn tab_skips_what_an_opaque_layer_hides_from_the_keyboard_too_091() {
    let controls = [
        textbox("BASE-1", None, 1, 10),
        textbox("BASE-2", None, 2, 50),
        textbox("L1-TB", Some("Layer-1"), 1, 100),
    ];
    // Translucent: the base is still on the walk.
    let walk = tab_walk(&controls, vec![layer("Layer-1", "#00000000")], &Shown(&["Layer-1"]), 4);
    assert_eq!(walk, ["BASE-1", "BASE-2", "L1-TB", "BASE-1"]);
    // Opaque: what is under it can be neither seen nor clicked, so Tab never
    // goes there (Q10).
    let walk = tab_walk(&controls, vec![layer("Layer-1", "#202020FF")], &Shown(&["Layer-1"]), 3);
    // One focusable control: Tab leaves the focus on it, so it is announced once.
    assert_eq!(walk, ["L1-TB"], "only the opaque layer's own control ever takes the focus");
}
