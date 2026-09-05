#![cfg(feature = "render")]
//! A themed RadioButton paints THE SAME in the designer and in the run form.
//!
//! "The radios show in the designer but not in Run Form" has been reported and
//! investigated three times, and each round cost days because nothing pinned
//! the claim down. This does.
//!
//! It is deliberately not a shape COUNT check. Counting shapes proves nothing —
//! a control painted off-surface and a control never painted look identical
//! that way. This compares the control's body fill, its indicator circle and
//! its caption, colour by colour, across both engine paths and two themes whose
//! toggle handling genuinely differs (Liquid Glass supplies no Toggle surface
//! and falls back to the control's own `CheckColor`; Elegance supplies one).
//!
//! The control is the operator's own RadioButton-1 from `datagrid-form`,
//! property for property — including `Transparency = 56`, which is what makes
//! its near-black body translucent over a busy backdrop. That is the form
//! asking for translucency and getting it, not the engine losing the control.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::render::{render_faces, render_form, Backdrop, RenderInput, RenderMode};
use egui::{pos2, Color32, Rect, Vec2};
use std::sync::Arc;

/// The operator's RadioButton-1, property for property.
fn operator_radio() -> Control {
    let mut c = Control::new("RadioButton-1", ControlType::RadioButton, 560, 16);
    c.rect = cobolt_forms::model::Rect::new(560, 16, 120, 45);
    for (k, v) in [
        ("Caption", "English"),
        ("BackgroundColor", "#010101FF"),
        ("BorderColor", "#8C8CA0"),
        ("BorderStyle", "Single"),
        ("BorderWidth", "1"),
        ("CheckBoxBorderColor", "#8C8CA0"),
        ("CheckBoxBorderStyle", "Single"),
        ("CheckBoxBorderWidth", "1"),
        ("CheckColor", "#9DFF00FF"),
        ("CheckSize", "54"),
        ("CornerRadius", "15"),
        ("FontName", "Arial"),
        ("FontSize", "17"),
        ("ForegroundColor", "#FFFFFFFF"),
        ("Transparency", "56"),
        ("GroupName", "lang-grp"),
    ] {
        c.properties.insert(k.to_owned(), PropValue::String(v.to_owned()));
    }
    c.properties.insert("Selected".to_owned(), PropValue::Bool(true));
    c
}

#[derive(Default, Debug)]
struct Painted {
    rects: Vec<(Rect, Color32)>,
    circles: Vec<(egui::Pos2, f32, Color32)>,
    texts: usize,
}

fn walk(s: &egui::Shape, p: &mut Painted) {
    match s {
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p)),
        egui::Shape::Rect(r) => p.rects.push((r.rect, r.fill)),
        egui::Shape::Circle(c) => p.circles.push((c.center, c.radius, c.fill)),
        egui::Shape::Text(_) => p.texts += 1,
        _ => {}
    }
}

fn capture(
    theme: Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
    run: bool,
) -> (Vec<(Rect, Color32)>, Vec<(egui::Pos2, f32, Color32)>, usize) {
    let controls = vec![operator_radio()];
    let ctx = egui::Context::default();
    cobolt_forms::paint::set_surface_theme(&ctx, theme);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(1211.0, 720.0)));
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &cobolt_forms::render::DesignedState,
                    form_size: Vec2::new(1211.0, 720.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop { paint: false, ..Default::default() },
                };
                if run {
                    let _ = render_form(ui, &inp);
                } else {
                    let painter = ui.painter().clone();
                    let _ = render_faces(&painter, pos2(0.0, 0.0), &inp);
                }
            });
    });
    full.textures_delta.clear();
    let mut p = Painted::default();
    for cs in &full.shapes {
        walk(&cs.shape, &mut p);
    }
    // Only what lands on the radio itself.
    let own = Rect::from_min_size(pos2(560.0, 16.0), Vec2::new(120.0, 45.0));
    let near = |r: Rect| r.intersects(own);
    let rects: Vec<_> = p.rects.iter().filter(|(r, _)| near(*r)).collect();
    let circles: Vec<_> = p
        .circles
        .iter()
        .filter(|(c, rad, _)| near(Rect::from_center_size(*c, Vec2::splat(rad * 2.0))))
        .collect();
    (
        rects.into_iter().copied().collect(),
        circles.into_iter().copied().collect(),
        p.texts,
    )
}

#[test]
fn a_themed_radio_paints_identically_in_the_designer_and_the_run_form() {
    for (name, t) in [
        ("Liquid Glass", cobolt_forms::surface_theme::liquid_glass()),
        ("Elegance", cobolt_forms::surface_theme::elegance()),
    ] {
        let (d_rects, d_circles, d_text) = capture(Arc::clone(&t), false);
        let (r_rects, r_circles, r_text) = capture(t, true);

        assert!(
            !d_rects.is_empty() && !d_circles.is_empty(),
            "{name}: the designer painted nothing for the radio at all"
        );
        assert_eq!(
            d_rects, r_rects,
            "{name}: the run form's rects differ from the designer's. Compare the \
             FILLS, not the count — an off-surface control and an unpainted one \
             look the same by count."
        );
        assert_eq!(
            d_circles, r_circles,
            "{name}: the indicator circle differs between designer and run form"
        );
        assert_eq!(
            d_text, r_text,
            "{name}: the caption is drawn on one surface and not the other"
        );

        // The body really is translucent — that is `Transparency = 56` being
        // honoured, and it is why the control reads faintly over a busy
        // backdrop. If this ever becomes opaque, the property stopped working.
        // Several rects share the body's exact bounds — the glass stack lays
        // fully transparent ones over it — so take the one that actually
        // carries paint rather than whichever comes first.
        let body_alpha = d_rects
            .iter()
            .filter(|(r, _)| r.width() > 100.0 && r.height() > 40.0)
            .map(|(_, f)| f.a())
            .max()
            .expect("the radio's own body rect");
        assert!(
            body_alpha > 0 && body_alpha < 255,
            "{name}: the body should be translucent (Transparency = 56); got alpha {body_alpha}"
        );

        println!(
            "  {name}: designer == run form — {} rects, {} circles, {} caption(s); body alpha {}",
            d_rects.len(),
            d_circles.len(),
            d_text,
            body_alpha
        );
    }
}
