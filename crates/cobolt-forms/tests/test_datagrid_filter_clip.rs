#![cfg(feature = "render")]
//! A DataGrid's filter row must not narrow the clip for the rest of the form.
//!
//! The filter inputs are egui widgets, so they are drawn through the shared
//! `Ui` rather than the painter, and the grid narrows that `Ui`'s clip to keep
//! a scrolling column from painting over the frozen band. It then put back the
//! wrong rectangle: not the clip it found, but that clip INTERSECTED WITH THE
//! GRID'S OWN — its container's content area. Everything drawn afterwards
//! inherited it.
//!
//! By z-order that is every control stacked above the grid. They were painted,
//! and `control_rects` recorded them at exactly the right screen positions, so
//! every measurement said they were fine — they were simply clipped away
//! unseen (operator, 2026-09-01: three RadioButtons and a Label's face vanished
//! from a form; turning `ShowColumnFilters` off on that one grid brought them
//! all back).
//!
//! The grid is inside a Panel here because that is what makes the leak bite: a
//! top-level grid's clip is the whole surface, so narrowing to it changes
//! nothing and the bug hides.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

/// The form: a Panel low down holding a filtered DataGrid, and a Label ABOVE
/// the panel with a HIGHER z-order, so it is drawn after the grid.
fn form(filters: bool) -> Vec<Control> {
    let mut panel = Control::new("Panel-1", ControlType::Panel, 16, 272);
    panel.rect = MRect::new(16, 272, 1256, 416);
    panel.z_order = 0;

    let mut grid = Control::new("DataGrid-1", ControlType::DataGrid, 40, 296);
    grid.rect = MRect::new(40, 296, 1211, 373);
    grid.z_order = 60;
    grid.parent = Some("Panel-1".to_owned());
    grid.set_prop("ShowColumnFilters", PropValue::Bool(filters));
    grid.set_prop(
        "Columns",
        PropValue::String("A:string\nB:string\nC:string".into()),
    );

    // Above the panel, drawn AFTER the grid.
    let mut above = Control::new("RadioButton-1", ControlType::RadioButton, 560, 16);
    above.rect = MRect::new(560, 16, 120, 45);
    above.z_order = 62;
    above.set_prop("Caption", PropValue::String("English".into()));

    vec![panel, grid, above]
}

/// Render one frame and report how many shapes land inside `probe`.
fn shapes_in(controls: &[Control], probe: Rect) -> usize {
    let size = Vec2::new(1288.0, 720.0);
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                let _ = cobolt_forms::render::render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();

    // The probe must be intersected with each shape's OWN clip rect. A shape
    // that is emitted and then clipped away is invisible on screen but still
    // present in `full.shapes` — counting it is what made an earlier version of
    // this test pass against the bug it exists to catch.
    fn walk(s: &egui::Shape, probe: Rect, n: &mut usize) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, probe, n)),
            egui::Shape::Rect(r) => {
                if r.fill.a() > 16 && probe.intersects(r.rect) {
                    *n += 1;
                }
            }
            egui::Shape::Circle(c) => {
                if c.fill.a() > 16
                    && probe.intersects(Rect::from_center_size(
                        c.center,
                        Vec2::splat(c.radius * 2.0),
                    ))
                {
                    *n += 1;
                }
            }
            egui::Shape::Text(t) => {
                if probe.intersects(t.visual_bounding_rect()) {
                    *n += 1;
                }
            }
            _ => {}
        }
    }
    let mut n = 0;
    for cs in &full.shapes {
        // Only what survives THIS shape's clip can be seen.
        let visible = probe.intersect(cs.clip_rect);
        if visible.is_positive() {
            walk(&cs.shape, visible, &mut n);
        }
    }
    n
}

/// The control above the grid is painted whether the filter row is on or off.
#[test]
fn a_filtered_datagrid_does_not_clip_the_controls_drawn_after_it() {
    // The radio's own area, well above the panel that holds the grid.
    let probe = Rect::from_min_size(pos2(560.0, 16.0), Vec2::new(120.0, 45.0));

    let without = shapes_in(&form(false), probe);
    let with = shapes_in(&form(true), probe);

    println!(
        "\n  shapes painted in the radio's area\n\
         \x20   ShowColumnFilters = false : {without}\n\
         \x20   ShowColumnFilters = true  : {with}\n"
    );

    assert!(
        without > 0,
        "precondition: with the filter row OFF the control above the grid must \
         be painted, or this test proves nothing"
    );
    assert_eq!(
        with, without,
        "turning the filter row ON changed what is painted ABOVE the grid: {with} \
         shapes instead of {without}. The filter inputs narrow the shared Ui's \
         clip and must restore the clip they found — restoring the grid's own \
         (narrower) clip instead silently erases every control drawn after it."
    );
}
