#![cfg(feature = "render")]
//! A `BackgroundColor` the developer chose is the control's face — with or
//! without a background gradient.
//!
//! > "Back color is not being applied when Background gradient is not
//! > selected." (operator, 2026-09-03)
//!
//! "Not selected" is the operative half. The colour was reaching the painter
//! all along: Liquid Glass drew it, then laid twenty frost bands over it
//! running from about 12 % to 44 % white, so pure red arrived washed pink and
//! the property read as dead. Switch a gradient on and the same colour came out
//! exact, because the gradient branch paints its mesh with nothing over it.
//! One property, two answers, depending on a checkbox that has nothing to do
//! with it.
//!
//! Reported on a TextBox; it was never a TextBox defect — Button and Panel took
//! the same veil through the same shared face path, so all three are pinned
//! here. The default look is pinned too: a control whose colour nobody chose
//! keeps its frost, which is what makes it glass.

use cobolt_forms::model::{Control, ControlType, GlassStyle, PropValue, Rect as MRect};
use cobolt_forms::paint::{draw_control, set_glass_style};
use egui::{Color32, Pos2, Vec2};

/// Every filled rect `draw_control` emitted, with its geometry — the frost is
/// identified by its SHAPE (a full-width one-pixel band), so a rim or a
/// specular highlight cannot be mistaken for it.
fn fills(ctrl: &Control, style: GlassStyle) -> Vec<(egui::Rect, Color32)> {
    let ctx = egui::Context::default();
    set_glass_style(&ctx, style);
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    fn walk(s: &egui::Shape, p: &mut Vec<(egui::Rect, Color32)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p)),
            egui::Shape::Rect(r) => p.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root, |ui| {
                draw_control(&ui.painter().clone(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
            });
    });
    full.textures_delta.clear();
    for cs in &full.shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

fn control(kind: ControlType, background: Option<&str>, gradient: bool) -> Control {
    let mut c = Control::new("C-1", kind, 20, 20);
    c.rect = MRect::new(20, 20, 160, 40);
    if let Some(bg) = background {
        c.set_prop("BackgroundColor", PropValue::String(bg.to_owned()));
    }
    c.set_prop("BackgroundGradientEnabled", PropValue::Bool(gradient));
    if gradient {
        c.set_prop(
            "BackgroundGradientStartColor",
            PropValue::String(background.unwrap_or("#FF0000FF").to_owned()),
        );
        c.set_prop(
            "BackgroundGradientEndColor",
            PropValue::String(background.unwrap_or("#FF0000FF").to_owned()),
        );
    }
    c
}

/// How much frost landed on the face: the summed alpha of the pale, full-width,
/// one-pixel-tall bands the glass styles stack over it.
///
/// A sum rather than a count, so a HALVED frost reads correctly — twenty faint
/// bands are still twenty bands.
///
/// Matched by geometry as well as colour, and only in the control's BOTTOM
/// half: the frost runs the full height, while Enhanced's top highlight band
/// and Classic's rim are edge cues that make the material read as glass and
/// stay whatever colour the face is. Counting those as frost would demand the
/// chosen colour erase the glass look itself, which is not what was asked for.
fn veil_weight(f: &[(egui::Rect, Color32)]) -> u32 {
    // The control is 40 px tall at y = 20.
    let below = 20.0 + 40.0 * 0.5;
    // Shape alone identifies them: down there, a full-width one-pixel rect is a
    // frost band and nothing else. Colour is deliberately NOT tested — these
    // are premultiplied, so a faint band's channels are as small as its alpha
    // and a brightness threshold would score a halved frost as no frost at all.
    f.iter()
        .filter(|(r, _)| r.width() >= 140.0 && r.height() <= 2.0 && r.min.y >= below)
        .map(|(_, c)| c.a() as u32)
        .sum()

}



const FROSTED: [GlassStyle; 2] = [GlassStyle::Classic, GlassStyle::Enhanced];
const KINDS: [(&str, ControlType); 3] = [
    ("TextBox", ControlType::TextBox),
    ("Button", ControlType::Button),
    ("Panel", ControlType::Panel),
];

#[test]
fn a_chosen_colour_is_not_repainted_white_by_the_frost() {
    for style in FROSTED {
        for (name, kind) in KINDS {
            let f = fills(&control(kind.clone(), Some("#FF0000FF"), false), style);
            assert_eq!(
                veil_weight(&f),
                0,
                "{name} under {style:?} with an opaque BackgroundColor and NO \
                 gradient must show that colour, not a frosted version of it"
            );
        }
    }
}

#[test]
fn the_flat_answer_matches_the_gradient_answer() {
    // The two must not disagree: switching a gradient on and off is what made
    // the same property appear to work and then stop.
    for style in FROSTED {
        for (name, kind) in KINDS {
            let flat = fills(&control(kind.clone(), Some("#FF0000FF"), false), style);
            let grad = fills(&control(kind.clone(), Some("#FF0000FF"), true), style);
            assert_eq!(
                veil_weight(&flat),
                veil_weight(&grad),
                "{name} under {style:?}: the gradient toggle must not decide \
                 whether BackgroundColor survives"
            );
        }
    }
}

#[test]
fn a_control_nobody_coloured_keeps_its_frost() {
    // The control. Liquid Glass IS the frost — losing it everywhere would also
    // satisfy the assertions above.
    for style in FROSTED {
        for (name, kind) in KINDS {
            let f = fills(&control(kind.clone(), None, false), style);
            assert!(
                veil_weight(&f) > 0,
                "{name} under {style:?} with no chosen BackgroundColor must keep \
                 the frost — that is what makes it glass"
            );
        }
    }
}

#[test]
fn a_translucent_chosen_colour_keeps_its_share_of_the_frost() {
    // Half-opaque: the developer asked to see through the face, and frost is
    // exactly what a see-through face is made of. Its share, not all of it.
    for style in FROSTED {
        let half = veil_weight(&fills(
            &control(ControlType::TextBox, Some("#FF000080"), false),
            style,
        ));
        let none = veil_weight(&fills(&control(ControlType::TextBox, None, false), style));
        assert!(
            half > 0 && half < none,
            "a half-transparent BackgroundColor under {style:?} keeps SOME frost              ({half}) but less than an uncoloured control ({none})"
        );
    }
}
