#![cfg(feature = "render")]
//! A Slider is ONE assembly — track, thumb, ticks and min/max labels — and it
//! sits centred in its rect.
//!
//! It used to be two things: the track centred on the rect while the labels
//! were pinned to `rect.max.y`. Every pixel of extra height then opened a gap
//! at the TOP only, so a 780x75 slider carried 22 px of dead space above the
//! track and 3 px below — the selection frame looked loose on one side and
//! tight on the other.

use cobolt_forms::{Control, ControlType, PropValue, Rect as MRect};

/// The bounding box of everything the control painted.
fn ink_bounds(ctrl: &Control) -> egui::Rect {
    let ctx = egui::Context::default();
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 400.0),
            )),
            ..Default::default()
        },
        |ui| {
            cobolt_forms::paint::draw_control(
                ui.painter(),
                egui::Pos2::ZERO,
                ctrl,
                false,
                true,
                1.0,
                1.0,
                None,
            );
        },
    );
    full.textures_delta.clear();
    fn walk(s: &egui::Shape, b: &mut Option<egui::Rect>) {
        let r = s.visual_bounding_rect();
        // Ignore the unbounded/degenerate rects a clipped shape can report.
        if r.is_positive() && r.width() < 5000.0 && r.height() < 5000.0 {
            *b = Some(match *b {
                Some(cur) => cur.union(r),
                None => r,
            });
        }
        if let egui::Shape::Vec(v) = s {
            v.iter().for_each(|s| walk(s, b));
        }
    }
    let mut bounds = None;
    for cs in &full.shapes {
        walk(&cs.shape, &mut bounds);
    }
    bounds.expect("a slider paints something")
}

fn slider(w: i32, h: i32) -> Control {
    let mut c = Control::new("S", ControlType::Slider, 0, 0);
    c.rect = MRect::new(0, 0, w, h);
    c.set_prop("Value", PropValue::Int(40));
    c
}

#[test]
fn a_horizontal_slider_is_centred_as_one_assembly() {
    println!("\n  ── Slider ink inside its rect ──");
    for (w, h) in [(200, 36), (200, 48), (780, 75), (200, 120)] {
        let b = ink_bounds(&slider(w, h));
        let top = b.min.y;
        let bottom = h as f32 - b.max.y;
        println!(
            "  {w}x{h:<4} ink y[{:.1}..{:.1}]  top {top:.1}  bottom {bottom:.1}",
            b.min.y, b.max.y
        );
        // Centred: the two margins match. They cannot be exactly equal — the
        // thumb and the label round differently — so allow a few pixels.
        assert!(
            (top - bottom).abs() <= 5.0,
            "{w}x{h}: the assembly must sit centred, got top {top:.1} vs bottom {bottom:.1}"
        );
        // And it must stay INSIDE the control on every side.
        assert!(
            top >= -0.5 && bottom >= -0.5,
            "{w}x{h}: the slider must not paint outside its rect \
             (top {top:.1}, bottom {bottom:.1})"
        );
    }
    println!();
}

/// From the default height upwards the assembly fits, so it stays inside the
/// rect on both edges.
///
/// Below that it cannot: the thumb alone is ~30 px, so a 10 px slider has to
/// overflow whatever we do. That was true before this change too — it is the
/// price of a rect smaller than the control's own furniture, not a regression —
/// so the guarantee is stated where it can actually hold.
#[test]
fn a_slider_at_or_above_its_default_height_stays_inside_its_rect() {
    for h in [36, 40, 48, 75, 120] {
        let b = ink_bounds(&slider(200, h));
        let (top, bottom) = (b.min.y, h as f32 - b.max.y);
        assert!(
            top >= -0.5 && bottom >= -0.5,
            "200x{h}: painted outside its rect (top {top:.1}, bottom {bottom:.1})"
        );
    }
}
