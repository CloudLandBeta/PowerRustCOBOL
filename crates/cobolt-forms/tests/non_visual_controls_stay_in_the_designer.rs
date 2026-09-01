#![cfg(feature = "render")]
//! A non-visual control paints its tray card on the DESIGNER canvas and
//! **nothing at all** in the running form.
//!
//! Its badge — a glass card with an icon and a label — is a designer
//! affordance: something to select, drag and inspect while building the form. In
//! the running application it is chrome the operator never asked for, parked at
//! whatever x/y it was dropped at.
//!
//! Nothing skipped these, so every Timer, AgentObject, RestClient, SqlDatabase
//! and IndexedFile drew its badge in the running form too. A Snackbar made it
//! impossible to miss (operator, 2026-09-01): a notification control that paints
//! a permanent "Info" card in the corner is self-evidently wrong in a way a
//! Timer's clock face apparently was not.
//!
//! Deliberately not a shape COUNT comparison between the two paths — a control
//! painted off-surface and a control never painted look identical that way. The
//! run assertion is absolute: **zero** shapes touch the control's own rect.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType};
use cobolt_forms::render::{render_faces, render_form, Backdrop, RenderInput, RenderMode};
use egui::{pos2, Rect, Vec2};

const AT: (i32, i32, i32, i32) = (40, 40, 56, 56);

fn control(ct: ControlType) -> Control {
    let mut c = Control::new("NV-1", ct, AT.0, AT.1);
    c.rect = cobolt_forms::model::Rect::new(AT.0, AT.1, AT.2, AT.3);
    c
}

/// Bounding rects of every shape the given path emits over the control.
fn painted(ct: &ControlType, run: bool) -> Vec<Rect> {
    let controls = vec![control(ct.clone())];
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 400.0)));
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &cobolt_forms::render::DesignedState,
                    form_size: Vec2::new(600.0, 400.0),
                    glass: true,
                    mode: if run { RenderMode::Interactive } else { RenderMode::Static },
                    active_tabs: &active,
                    backdrop: Backdrop { paint: false, ..Default::default() },
                };
                if run {
                    let _ = render_form(ui, &inp);
                } else {
                    let painter = ui.painter().clone();
                    let _ = render_faces(&painter, pos2(0.0, 0.0), &inp, None);
                }
            });
    });
    full.textures_delta.clear();

    fn walk(s: &egui::Shape, out: &mut Vec<Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            other => {
                let r = other.visual_bounding_rect();
                if r.is_finite() && r.is_positive() {
                    out.push(r);
                }
            }
        }
    }
    let mut out = Vec::new();
    for cs in &full.shapes {
        walk(&cs.shape, &mut out);
    }
    let own = Rect::from_min_size(
        pos2(AT.0 as f32, AT.1 as f32),
        Vec2::new(AT.2 as f32, AT.3 as f32),
    );
    out.into_iter().filter(|r| r.intersects(own)).collect()
}

/// Every non-visual type in the catalogue, both paths, reported as a table.
#[test]
fn a_non_visual_control_draws_its_tray_card_only_in_the_designer() {
    let types = [
        ControlType::Timer,
        ControlType::AgentObject,
        ControlType::RestClient,
        ControlType::SqlDatabase,
        ControlType::IndexedFile,
        ControlType::WebSearch,
        ControlType::Snackbar,
    ];
    eprintln!("\n  control        non-visual   designer shapes   run-form shapes");
    eprintln!("  ------------   ----------   ---------------   ---------------");
    let mut leaked = Vec::new();
    for ct in &types {
        assert!(ct.is_non_visual(), "{} must be non-visual", ct.as_str());
        let design = painted(ct, false).len();
        let run = painted(ct, true).len();
        eprintln!("  {:<12}   {:<10}   {design:>15}   {run:>15}", ct.as_str(), "yes");
        if run != 0 {
            leaked.push(format!("{}: {run} shape(s) in the running form", ct.as_str()));
        }
    }
    assert!(
        leaked.is_empty(),
        "non-visual control(s) painted in the running form:\n  {}",
        leaked.join("\n  ")
    );
    eprintln!("  → 7 non-visual types, 0 shapes in the running form\n");
}

/// The other half: hiding them at run time must not blank the designer canvas,
/// which is where the developer actually needs to see and select them.
#[test]
fn the_designer_canvas_still_draws_them() {
    let mut drew = 0usize;
    for ct in &[ControlType::Timer, ControlType::Snackbar, ControlType::IndexedFile] {
        let n = painted(ct, false).len();
        assert!(n > 0, "{} must still paint its tray card on the canvas", ct.as_str());
        drew += n;
    }
    eprintln!("\n  designer canvas — 3 tray cards still painted ({drew} shapes total)\n");
}

/// An ordinary control is untouched by the skip — the guard is about the
/// non-visual flag, not about "controls that happen to be small".
#[test]
fn ordinary_controls_still_paint_in_the_running_form() {
    for ct in &[ControlType::Button, ControlType::Label, ControlType::Panel] {
        let run = painted(ct, true).len();
        assert!(run > 0, "{} must still paint in the running form", ct.as_str());
    }
    eprintln!("\n  regression guard — Button, Label and Panel all still paint at run time\n");
}
