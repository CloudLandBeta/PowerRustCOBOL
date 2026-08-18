#![cfg(feature = "render")]
// The breadcrumb frame is chrome the shell paints between the form's
// background and its controls, and it is NOT a container: a control the
// developer placed over the frame must paint ON TOP of it.
//
// Paint order is the whole claim, so this test reads the order the engine
// actually emitted — the frame's rect must appear in the shape list BEFORE the
// control's, since egui paints a layer in the order it was given.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::render::{
    render_form_with_chrome, Backdrop, DesignedState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType};
use egui::{Color32, Pos2};

/// The frame's fill and the control's fill, picked so neither can be produced
/// by anything else the engine paints.
const FRAME_INK: Color32 = Color32::from_rgb(0x8B, 0x00, 0x00);
const PANEL_INK: Color32 = Color32::from_rgb(0x00, 0xC8, 0x7B);

/// Index of the first rect painted with `ink`, in emission order.
fn first_rect_with(shapes: &[egui::epaint::ClippedShape], ink: Color32) -> Option<usize> {
    fn walk(s: &egui::Shape, ink: Color32, seen: &mut usize, hit: &mut Option<usize>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, ink, seen, hit)),
            egui::Shape::Rect(r) => {
                if hit.is_none() && r.fill == ink {
                    *hit = Some(*seen);
                }
                *seen += 1;
            }
            _ => *seen += 1,
        }
    }
    let (mut seen, mut hit) = (0usize, None);
    for cs in shapes {
        walk(&cs.shape, ink, &mut seen, &mut hit);
    }
    hit
}

#[test]
fn a_control_over_the_breadcrumb_frame_paints_on_top_of_it() {
    // A Panel drawn INSIDE the 64pt frame band — what a developer does when
    // they want a search box or a title up there.
    let mut panel = Control::new("Panel-1", ControlType::Panel, 300, 8);
    panel.rect = cobolt_forms::model::Rect::new(300, 8, 200, 40);
    panel.set_prop("BackgroundColor", "#00C87B");
    panel.set_prop("Transparency", 0);
    let controls = vec![panel];

    let ctx = egui::Context::default();
    let active = ActiveTabs::default();
    let state = DesignedState;
    let frame_band = std::cell::Cell::new(egui::Rect::NOTHING);
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(900.0, 600.0),
            )),
            ..Default::default()
        },
        |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                let input = RenderInput {
                    controls: &controls,
                    state: &state,
                    form_size: egui::vec2(800.0, 500.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                // The shell's slot: the frame, painted on the backdrop.
                let chrome = |painter: &egui::Painter, form_rect: egui::Rect| {
                    let band = egui::Rect::from_min_size(
                        form_rect.min,
                        egui::vec2(form_rect.width(), 64.0),
                    );
                    frame_band.set(band);
                    painter.rect_filled(band, 0.0, FRAME_INK);
                };
                render_form_with_chrome(ui, &input, Some(&chrome));
            });
        },
    );
    full.textures_delta.clear();

    let frame_at = first_rect_with(&full.shapes, FRAME_INK).expect("the frame was painted");
    let panel_at = first_rect_with(&full.shapes, PANEL_INK)
        .expect("the control over the frame was painted");
    assert!(
        frame_at < panel_at,
        "the frame must be painted BEFORE the control that sits on it \
         (frame #{frame_at}, control #{panel_at})"
    );

    // …and the control really does overlap the band, or the order would prove
    // nothing about what the operator sees.
    let band = frame_band.get();
    let control = egui::Rect::from_min_size(
        band.min + egui::vec2(300.0, 8.0),
        egui::vec2(200.0, 40.0),
    );
    assert!(
        band.contains_rect(control),
        "the control was drawn inside the frame band: {control:?} in {band:?}"
    );

    println!(
        "breadcrumb frame paint order — 64pt frame at shape #{frame_at}, the \
         200x40 control over it at #{panel_at}: the control paints on top, and \
         its rect {:?} lies inside the band {:?}",
        control, band
    );
}

/// Without a shell there is no frame, and `render_form` paints nothing extra —
/// the slot is the host's, and an ordinary form never sees it.
#[test]
fn a_form_with_no_chrome_paints_no_frame() {
    let controls = vec![Control::new("Label-1", ControlType::Label, 10, 10)];
    let ctx = egui::Context::default();
    let active = ActiveTabs::default();
    let state = DesignedState;
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            ..Default::default()
        },
        |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                let input = RenderInput {
                    controls: &controls,
                    state: &state,
                    form_size: egui::vec2(300.0, 200.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                render_form_with_chrome(ui, &input, None);
            });
        },
    );
    full.textures_delta.clear();
    assert_eq!(
        first_rect_with(&full.shapes, FRAME_INK),
        None,
        "no chrome was handed in, so none was painted"
    );
    println!("breadcrumb frame — no chrome slot supplied ⇒ nothing painted for it");
}
