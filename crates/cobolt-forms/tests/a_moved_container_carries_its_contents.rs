#![cfg(feature = "render")]
//! **A container moved at run time must carry its contents with it.**
//!
//! A control's rectangle is form-space ABSOLUTE with a `parent` link, so a
//! COBOL write to a container's `X`/`Y` moved that container's rectangle and
//! nothing else. The operator animated a Panel with a Timer decrementing its
//! `Y` on every tick: the panel slid up the form and every control inside it
//! stayed exactly where it was (2026-09-04). `MoveTo` is two property writes,
//! so it behaved identically.
//!
//! The bug is not Panel-specific — it belongs to the parent link — so this
//! covers every container that can hold controls, and a nested one.

use std::collections::HashMap;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{FormState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

/// Moves ONE control by a delta, exactly as a COBOL `SET Panel::Y TO n` does:
/// the container's own rectangle, and nothing else.
struct Moved {
    id: &'static str,
    dx: i32,
    dy: i32,
}

impl FormState for Moved {
    fn live(&self, base: &Control) -> Control {
        let mut c = base.clone();
        if c.id == self.id {
            c.rect = MRect::new(c.rect.x + self.dx, c.rect.y + self.dy, c.rect.w, c.rect.h);
        }
        c
    }
}

struct Designed;
impl FormState for Designed {}

fn child(id: &str, parent: &str, x: i32, y: i32) -> Control {
    let mut c = Control::new(id, ControlType::Label, x, y);
    c.rect = MRect::new(x, y, 120, 24);
    c.parent = Some(parent.to_owned());
    c.set_prop("Caption", PropValue::String(id.into()));
    c
}

fn container(id: &str, kind: ControlType, x: i32, y: i32) -> Control {
    let mut c = Control::new(id, kind, x, y);
    c.rect = MRect::new(x, y, 320, 200);
    c
}

fn rects(controls: &[Control], state: &dyn FormState) -> HashMap<String, Rect> {
    let size = Vec2::new(900.0, 700.0);
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    let mut out = HashMap::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                out = cobolt_forms::render::render_form(ui, &inp).control_rects;
            });
    });
    full.textures_delta.clear();
    out
}

/// The whole point, for each container type that can hold a control.
#[test]
fn every_container_carries_its_children_when_it_moves() {
    for kind in [
        ControlType::Panel,
        ControlType::GroupBox,
        ControlType::TabControl,
    ] {
        let controls = vec![
            container("BOX-1", kind.clone(), 100, 300),
            child("LBL-INSIDE", "BOX-1", 140, 340),
        ];

        let before = rects(&controls, &Designed);
        let after = rects(
            &controls,
            &Moved {
                id: "BOX-1",
                dx: 0,
                dy: -40,
            },
        );

        let b = before.get("LBL-INSIDE").expect("child drawn before");
        let a = after.get("LBL-INSIDE").expect("child drawn after");
        assert!(
            (a.top() - (b.top() - 40.0)).abs() < 0.5,
            "{kind:?}: the child must travel with its container — was {:.1}, now {:.1}, expected {:.1}",
            b.top(),
            a.top(),
            b.top() - 40.0
        );

        // The container itself moved by the same amount, so the two stay
        // rigid rather than merely both moving.
        let cb = before.get("BOX-1").expect("container drawn before");
        let ca = after.get("BOX-1").expect("container drawn after");
        assert!(
            ((ca.top() - cb.top()) - (a.top() - b.top())).abs() < 0.5,
            "{kind:?}: container and child must move together"
        );
    }
}

/// Nesting accumulates: a Panel inside a Panel, with only the OUTER one moved.
#[test]
fn a_nested_containers_contents_follow_the_outermost_move() {
    let mut inner = container("BOX-INNER", ControlType::Panel, 140, 340);
    inner.rect = MRect::new(140, 340, 200, 120);
    inner.parent = Some("BOX-OUTER".to_owned());

    let controls = vec![
        container("BOX-OUTER", ControlType::Panel, 100, 300),
        inner,
        child("LBL-DEEP", "BOX-INNER", 160, 360),
    ];

    let before = rects(&controls, &Designed);
    let after = rects(
        &controls,
        &Moved {
            id: "BOX-OUTER",
            dx: 25,
            dy: -40,
        },
    );

    for id in ["BOX-INNER", "LBL-DEEP"] {
        let b = before.get(id).unwrap_or_else(|| panic!("{id} before"));
        let a = after.get(id).unwrap_or_else(|| panic!("{id} after"));
        assert!(
            (a.top() - (b.top() - 40.0)).abs() < 0.5 && (a.left() - (b.left() + 25.0)).abs() < 0.5,
            "{id} must follow the outer panel on both axes: was ({:.1},{:.1}), now ({:.1},{:.1})",
            b.left(),
            b.top(),
            a.left(),
            a.top()
        );
    }
}

/// A control moved by its OWN write keeps the coordinates the developer gave
/// it: those are form-space, and adding the parent's delta would move it
/// somewhere nobody asked for.
#[test]
fn a_child_moved_by_its_own_write_is_not_moved_again_by_its_parent() {
    let controls = vec![
        container("BOX-1", ControlType::Panel, 100, 300),
        child("LBL-INSIDE", "BOX-1", 140, 340),
    ];

    struct Both;
    impl FormState for Both {
        fn live(&self, base: &Control) -> Control {
            let mut c = base.clone();
            match c.id.as_str() {
                // The container moves up 40…
                "BOX-1" => c.rect = MRect::new(c.rect.x, c.rect.y - 40, c.rect.w, c.rect.h),
                // …while the child is written to an absolute position of its own.
                "LBL-INSIDE" => c.rect = MRect::new(c.rect.x, 500, c.rect.w, c.rect.h),
                _ => {}
            }
            c
        }
    }

    let before = rects(&controls, &Designed);
    let after = rects(&controls, &Both);

    let b = before.get("LBL-INSIDE").expect("child before");
    let a = after.get("LBL-INSIDE").expect("child after");
    // Designed y is 340; the write says 500. The result must be +160, NOT
    // +120 (which would be the write with the parent's -40 added on top).
    assert!(
        (a.top() - (b.top() + 160.0)).abs() < 0.5,
        "an explicit write must stand alone: was {:.1}, now {:.1}, expected {:.1}",
        b.top(),
        a.top(),
        b.top() + 160.0
    );
}
