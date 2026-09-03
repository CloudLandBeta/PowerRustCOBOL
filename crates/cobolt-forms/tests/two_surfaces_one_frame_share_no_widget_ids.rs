#![cfg(feature = "render")]
//! Two form surfaces in one egui frame never collide on a widget id.
//!
//! The shell paints the main form's SideMenu footer fragment into the rail and,
//! in the same frame, a whole other form into the ContentPane. Both derive
//! widget ids from control ids, so PowerDemo3's `sidebar-form` (whose footer
//! holds a `TextBox-1`) and `ferris-says-form` (which also has a `TextBox-1`)
//! asked egui for the same id and it painted "🔥 Second use of widget ID 9F79"
//! over the Ferris form's "Type your message" box (operator, 2026-09-02).
//!
//! The first attempt at a fix salted the two passes with `ui.push_id`, and the
//! id in the screenshot did not change — because the engine's control ids are
//! ABSOLUTE (`Id::new(("rt_ctrl", <control id>))`), built so a host can
//! reconstruct them later without knowing where in the `Ui` tree a control was
//! drawn. `push_id` cannot reach them. The id space has to be passed in, which
//! is `render_form_scoped`.
//!
//! A screenshot cannot say WHICH two widgets collided — the id is a hash and
//! both ends have to be found — so this asserts on egui's own detector instead:
//! the warning it paints when `warn_on_id_clash` is on. The last case is the
//! control: the same two forms WITHOUT a scope must still clash, or this test
//! would pass on a renderer that had stopped drawing anything at all.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::render::{
    render_form, render_form_scoped, Backdrop, DesignedState, RenderInput, RenderMode,
};
use egui::{pos2, Rect, Vec2};

/// A form fragment holding one TextBox with the given id.
fn one_textbox(id: &str, y: i32) -> Vec<Control> {
    let mut c = Control::new(id, ControlType::TextBox, 20, y);
    c.rect = cobolt_forms::model::Rect::new(20, y, 240, 30);
    c.properties
        .insert("Text".to_owned(), PropValue::String("hello".to_owned()));
    vec![c]
}

/// Render `footer` and `pane` as two surfaces of ONE frame and return every
/// id-clash warning egui painted.
///
/// `scope_footer` mirrors what the host does: the footer fragment renders in
/// its own id space, the pane occupant in the plain one.
fn clashes(footer: &[Control], pane: &[Control], scope_footer: bool) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.warn_on_id_clash = true);
    let active = ActiveTabs::new();
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(900.0, 600.0)));

    fn input_for<'a>(controls: &'a [Control], active: &'a ActiveTabs) -> RenderInput<'a> {
        RenderInput {
            controls,
            state: &DesignedState,
            form_size: Vec2::new(400.0, 200.0),
            glass: true,
            mode: RenderMode::Interactive,
            active_tabs: active,
            backdrop: Backdrop {
                paint: false,
                ..Default::default()
            },
        }
    }


    let mut full = ctx.run_ui(raw, |root_ui| {
        // The rail's footer band, top-left.
        let band = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 200.0));
        let mut child = root_ui.new_child(egui::UiBuilder::new().max_rect(band));
        let inp = input_for(footer, &active);
        if scope_footer {
            child.push_id("sidemenu-footer", |ui| {
                render_form_scoped(ui, &inp, None, egui::Id::new("cobolt-sidemenu-footer"));
            });
        } else {
            child.push_id("sidemenu-footer", |ui| {
                render_form(ui, &inp);
            });
        }

        // The ContentPane, well clear of the band so a clash cannot be excused
        // as egui's "same rect, so it is the same widget" allowance.
        let pane_rect = Rect::from_min_size(pos2(420.0, 300.0), Vec2::new(400.0, 200.0));
        let mut pane_ui = root_ui.new_child(egui::UiBuilder::new().max_rect(pane_rect));
        let inp = input_for(pane, &active);
        pane_ui.push_id("content-pane", |ui| {
            render_form(ui, &inp);
        });
    });
    full.textures_delta.clear();

    let mut found = Vec::new();
    fn walk(s: &egui::Shape, found: &mut Vec<String>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, found)),
            egui::Shape::Text(t) => {
                let text = t.galley.job.text.clone();
                if text.contains("use of widget ID") {
                    found.push(text);
                }
            }
            _ => {}
        }
    }
    for cs in &full.shapes {
        walk(&cs.shape, &mut found);
    }
    found
}

#[test]
fn a_scoped_footer_and_a_pane_occupant_sharing_a_control_id_do_not_clash() {
    let found = clashes(&one_textbox("TextBox-1", 10), &one_textbox("TextBox-1", 10), true);
    assert!(
        found.is_empty(),
        "the footer and the pane occupant both hold TextBox-1 and egui still \
         reported an id clash: {found:?}"
    );
}

#[test]
fn two_surfaces_with_different_control_ids_never_clashed_and_still_do_not() {
    let found = clashes(&one_textbox("Footer-Box", 10), &one_textbox("TextBox-1", 10), true);
    assert!(found.is_empty(), "unexpected id clash: {found:?}");
}

#[test]
fn the_same_pair_unscoped_still_clashes_so_the_detector_is_real() {
    // The control. Without this, a renderer that drew nothing would pass the
    // test above and the bug would look fixed.
    let found = clashes(&one_textbox("TextBox-1", 10), &one_textbox("TextBox-1", 10), false);
    assert!(
        !found.is_empty(),
        "two unscoped surfaces sharing TextBox-1 must still collide — if they \
         do not, this suite is no longer measuring anything"
    );
}
