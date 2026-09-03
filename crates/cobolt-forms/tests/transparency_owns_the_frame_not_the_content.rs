#![cfg(feature = "render")]
//! One rule, every control: `Transparency` is the FRAME's, and the drop shadow
//! belongs to the frame.
//!
//! Two operator reports on 2026-09-03, one defect underneath:
//!
//! > "any control, transparency affects the control frame. if the transparency
//! > is set to 100%, the dropshadow is applied to the elements of the control
//! > instead the frame"
//!
//! > "Label with transparency set to 100% has no frame meaning they are
//! > invisible (transparency applies to the frame, not the glyphs) and
//! > dropshadow in this case is applied to the glyphs, not the frame (which is
//! > invisible, not removed)."
//!
//! At 100 % the frame is INVISIBLE, not removed — it keeps its space, its
//! corners and its properties — and its shadow goes invisible with it. What is
//! left behind is the content: the caption, the glyph, the value, each at its
//! own full strength. A shadow that outlives the face it belongs to reads as a
//! shadow cast by the letters, which is what the operator saw.
//!
//! The shadow is given an unmistakable colour so the assertion is about the
//! shadow itself and not about a shape count, which proves nothing.

use cobolt_forms::model::{Control, ControlType, PropValue, Rect as MRect};
use cobolt_forms::paint::draw_control;
use egui::{Color32, Pos2, Vec2};

/// Every fill colour `draw_control` emitted, and every text it laid out.
#[derive(Default)]
struct Painted {
    fills: Vec<Color32>,
    texts: Vec<String>,
}

fn walk(s: &egui::Shape, p: &mut Painted) {
    match s {
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p)),
        egui::Shape::Rect(r) => p.fills.push(r.fill),
        egui::Shape::Circle(c) => p.fills.push(c.fill),
        egui::Shape::Path(pp) => p.fills.push(pp.fill),
        egui::Shape::Mesh(m) => p.fills.extend(m.vertices.iter().map(|v| v.color)),
        egui::Shape::Text(t) => p.texts.push(t.galley.job.text.clone()),
        _ => {}
    }
}

/// Paint one control and report what landed on the surface.
fn paint(ctrl: &Control) -> Painted {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    let mut p = Painted::default();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root, |ui| {
                let painter = ui.painter().clone();
                let before = painter.add(egui::Shape::Noop);
                draw_control(&painter, Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                let _ = before;
            });
    });
    full.textures_delta.clear();
    for cs in &full.shapes {
        walk(&cs.shape, &mut p);
    }
    p
}

/// A control with an unmistakable red drop shadow switched on.
fn shadowed(kind: ControlType, transparency: i64) -> Control {
    let mut c = Control::new("C-1", kind, 40, 40);
    c.rect = MRect::new(40, 40, 160, 48);
    for (k, v) in [
        ("Caption", "Hello"),
        ("Text", "Hello"),
        ("ShadowColor", "#FF0000"),
        ("ShadowDirection", "South"),
        ("ForegroundColor", "#101010"),
    ] {
        c.set_prop(k, PropValue::String(v.to_owned()));
    }
    c.set_prop("ShadowEnabled", PropValue::Bool(true));
    c.set_prop("ShadowBlur", PropValue::Bool(false));
    c.set_prop("ShadowOpacity", PropValue::Int(100));
    c.set_prop("ShadowDistance", PropValue::Int(10));
    c.set_prop("CornerRadius", PropValue::Int(12));
    c.set_prop("Transparency", PropValue::Int(transparency));
    c
}

/// How much red the surface carries — the shadow's own colour, and nothing
/// else in these controls is red.
fn red_shapes(p: &Painted) -> usize {
    p.fills
        .iter()
        .filter(|c| c.a() > 0 && c.r() > 60 && c.g() < 40 && c.b() < 40)
        .count()
}

const KINDS: [(&str, ControlType); 4] = [
    ("Label", ControlType::Label),
    ("Button", ControlType::Button),
    ("Panel", ControlType::Panel),
    ("TextBox", ControlType::TextBox),
];

#[test]
fn an_opaque_frame_still_casts_its_drop_shadow() {
    // The control: without this, "no shadow at 100 %" would also pass on a
    // renderer that had stopped drawing shadows at all.
    for (name, kind) in KINDS {
        let p = paint(&shadowed(kind.clone(), 0));
        assert!(
            red_shapes(&p) > 0,
            "{name} at Transparency = 0 must still cast its drop shadow"
        );
    }
}

#[test]
fn an_invisible_frame_casts_no_drop_shadow() {
    for (name, kind) in KINDS {
        let p = paint(&shadowed(kind.clone(), 100));
        assert_eq!(
            red_shapes(&p),
            0,
            "{name} at Transparency = 100 has an INVISIBLE frame, so its frame \
             shadow must be invisible too — a shadow left behind reads as one \
             cast by the control's own glyphs"
        );
    }
}

#[test]
fn an_invisible_frame_still_shows_its_content() {
    // Transparency is the frame's business, never the caption's.
    //
    // A Panel is absent on purpose: it has no Caption and no Text — the
    // catalogue gives those to Label/Button/CheckBox/RadioButton/GroupBox and
    // `Text` to a TextBox — so it has no content to keep, and demanding one
    // would be testing a property the control does not have.
    for (name, kind) in [
        ("Label", ControlType::Label),
        ("Button", ControlType::Button),
        ("TextBox", ControlType::TextBox),
    ] {
        let p = paint(&shadowed(kind.clone(), 100));

        assert!(
            p.texts.iter().any(|t| t.contains("Hello")),
            "{name} at Transparency = 100 must still show its content at full \
             strength — transparency applies to the frame, not the glyphs \
             (texts painted: {:?})",
            p.texts
        );
    }
}

#[test]
fn an_invisible_frame_keeps_its_corner_radius() {
    // "Invisible, not removed": the frame still occupies space and its own
    // properties stay meaningful, so nothing may drop them on the way through.
    for (name, kind) in KINDS {
        let c = shadowed(kind.clone(), 100);
        assert_eq!(
            cobolt_forms::paint::corner_radius(&c),
            12.0,
            "{name} at Transparency = 100 must still report its CornerRadius"
        );
    }
}

/// A partly transparent frame is FADED, never dropped.
///
/// A CheckBox used to lose its whole frame once it reached 30 % transparency,
/// so a developer who asked for a half-visible card got none — the threshold
/// was there to stop a frame shadow hanging in mid-air around a face that had
/// gone, which is the shadow defect above and is fixed at its source now.
#[test]
fn a_half_transparent_frame_is_faded_not_dropped() {
    for (name, kind) in [
        ("CheckBox", ControlType::CheckBox),
        ("TextBox", ControlType::TextBox),
        ("Panel", ControlType::Panel),
    ] {
        let mut c = shadowed(kind.clone(), 50);
        c.set_prop("BackgroundColor", PropValue::String("#2255CCFF".to_owned()));
        let half = paint(&c);
        let mut c = shadowed(kind.clone(), 100);
        c.set_prop("BackgroundColor", PropValue::String("#2255CCFF".to_owned()));
        let gone = paint(&c);

        let face = |p: &Painted| p.fills.iter().filter(|c| c.a() > 0).count();
        assert!(
            face(&half) > face(&gone),
            "{name} at 50 % must paint more of its face than at 100 % — a \
             transparency in between fades the frame, it does not remove it \
             ({} fills vs {})",
            face(&half),
            face(&gone)
        );
    }
}

/// A container's own `BackgroundColor` reaches its face whether or not the
/// Liquid Glass toggle is on.
///
/// A container's `fill` is the theme's card rather than its own colour ("their
/// content comes from children"), and the glass path passes the chosen colour
/// separately as an underlay — so a Panel showed it under glass and ignored it
/// with glass off. One property, two answers.
#[test]
fn a_container_shows_its_own_background_colour_with_or_without_glass() {
    for glass in [false, true] {
        let mut c = Control::new("P-1", ControlType::Panel, 20, 20);
        c.rect = MRect::new(20, 20, 200, 80);
        c.set_prop("BackgroundColor", PropValue::String("#FF0000FF".to_owned()));

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let mut p = Painted::default();
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root, |ui| {
                    cobolt_forms::paint::draw_control(
                        &ui.painter().clone(),
                        Pos2::ZERO,
                        &c,
                        false,
                        glass,
                        1.0,
                        1.0,
                        None,
                    );
                });
        });
        full.textures_delta.clear();
        for cs in &full.shapes {
            walk(&cs.shape, &mut p);
        }
        assert!(
            p.fills
                .iter()
                .any(|c| c.a() > 200 && c.r() > 200 && c.g() < 40 && c.b() < 40),
            "a Panel set to #FF0000 must paint red with glass = {glass}, got {:?}",
            p.fills
        );
    }
}
