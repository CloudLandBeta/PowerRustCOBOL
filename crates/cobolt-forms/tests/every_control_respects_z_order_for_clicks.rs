#![cfg(feature = "render")]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A click goes to the control that is on top.**
//!
//! `every_control_respects_z_and_layer_order.rs` holds what is *painted* to the stacking order.
//! This holds what *answers*: two controls drawn one over the other, both with an `onClick`
//! handler, and a click where they overlap. The one drawn on top is the one the person sees
//! and pressed; the one under it must not report a click, change its own value, or take the
//! focus.
//!
//! Run for every visual control type, once with a Button above it (z-order, then a layer) and
//! once with the Button below it, on the run form (`render_form`, interactive).

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{EventBinding, Layer};
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType};
use egui::{Pos2, Rect, Vec2};

/// What a click at the overlap did: the controls that reported an `onClick`, and the controls
/// whose own value a click changed.
#[derive(Debug, Default)]
struct Outcome {
    clicked: Vec<String>,
    changed: Vec<String>,
}

fn bound(mut c: Control) -> Control {
    c.events.push(EventBinding::new("onClick", format!("{}-CLICK", c.id)));
    c
}

fn place(id: &str, ty: ControlType, layer: Option<&str>, z: i32) -> Control {
    let mut c = Control::new(id, ty, 40, 40);
    c.rect.w = 300;
    c.rect.h = 200;
    c.layer = layer.map(str::to_owned);
    c.z_order = z;
    bound(c)
}

/// What the frames of a script reported, with the frame each came in.
#[derive(Debug, Default)]
struct Reported {
    /// `(frame, control, event)`.
    events: Vec<(usize, String, String)>,
    /// `(frame, control, property, value)`.
    props: Vec<(usize, String, String, String)>,
}

fn pointer(at: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// A click at `at`: the pointer arrives, the button goes down, the button comes up.
fn a_click(at: Pos2) -> Vec<Vec<egui::Event>> {
    vec![vec![egui::Event::PointerMoved(at)], vec![pointer(at, true)], vec![pointer(at, false)], vec![]]
}

/// One form, drawn frame after frame on one context, so a script can look at the screen and
/// then act on what it shows.
struct Session<'a> {
    ctx: egui::Context,
    controls: &'a [Control],
    layers: &'a [Layer],
    active: ActiveTabs,
    frame: usize,
    reported: Reported,
}

impl<'a> Session<'a> {
    fn new(controls: &'a [Control], layers: &'a [Layer]) -> Self {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut s = Self { ctx, controls, layers, active: ActiveTabs::new(), frame: 0, reported: Reported::default() };
        s.run(vec![]);
        s
    }

    /// Draw one frame with `events`; answers the texts it painted, with where.
    fn run(&mut self, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
        let i = self.frame;
        self.frame += 1;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
            time: Some(i as f64 * 0.06),
            events,
            focused: true,
            ..Default::default()
        };
        let (controls, layers, active) = (self.controls, self.layers, &self.active);
        let reported = &mut self.reported;
        let mut full = self.ctx.run_ui(raw, |root_ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(500.0, 400.0));
                let input = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: Vec2::new(500.0, 400.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: active,
                    backdrop: Backdrop { layers: layers.to_vec(), ..Backdrop::default() },
                };
                let out = render_form(ui, &input);
                reported.events.extend(out.events.iter().map(|e| (i, e.ctrl_id.clone(), e.event.clone())));
                reported
                    .props
                    .extend(out.prop_updates.iter().map(|(id, p, v)| (i, id.clone(), p.clone(), v.clone())));
            });
        });
        full.textures_delta.clear();
        fn texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.galley.rect.translate(t.pos.to_vec2()))),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut painted = Vec::new();
        for cs in &full.shapes {
            texts(&cs.shape, &mut painted);
        }
        painted
    }
}

/// Draws the form once per entry of `frames` (a first, quiet frame is added), on one context.
fn drive(controls: &[Control], layers: &[Layer], frames: Vec<Vec<egui::Event>>) -> Reported {
    let mut session = Session::new(controls, layers);
    for events in frames {
        session.run(events);
    }
    session.reported
}

fn click(controls: &[Control], layers: &[Layer]) -> Outcome {
    let r = drive(controls, layers, a_click(Pos2::new(190.0, 140.0)));
    let mut outcome = Outcome::default();
    outcome.clicked = r.events.iter().filter(|(_, _, e)| e == "onClick").map(|(_, id, _)| id.clone()).collect();
    // From the press on: a control settles its own state on the first frames, click or not.
    outcome.changed = r.props.iter().filter(|(frame, ..)| *frame >= 3).map(|(_, id, ..)| id.clone()).collect();
    outcome.clicked.sort();
    outcome.clicked.dedup();
    outcome.changed.sort();
    outcome.changed.dedup();
    outcome
}

#[test]
fn a_click_goes_to_the_control_on_top() {
    let layers = vec![Layer::new("L1")];
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut answered = 0usize;
    for ty in ControlType::ALL.iter().filter(|t| !t.is_non_visual()) {
        if std::env::var("CLICK_ONLY").is_ok_and(|o| !format!("{ty:?}").eq_ignore_ascii_case(&o)) {
            continue;
        }
        // The control alone answers a click: otherwise the case proves nothing about the order.
        let alone = click(&[place("A", ty.clone(), None, 50)], &layers);
        if !alone.clicked.iter().any(|c| c == "A") {
            continue;
        }
        answered += 1;
        // (what the case is, A, B, the one that should answer)
        let cases: Vec<(&str, Control, Control, &str)> = vec![
            ("a Button above it in z-order", place("A", ty.clone(), None, 50), place("B", ControlType::Button, None, 90), "B"),
            ("a Button below it in z-order", place("A", ty.clone(), None, 50), place("B", ControlType::Button, None, 10), "A"),
            ("a Button in a layer above it", place("A", ty.clone(), None, 80), place("B", ControlType::Button, Some("L1"), 5), "B"),
            ("a Button of the base under its layer", place("A", ty.clone(), Some("L1"), 5), place("B", ControlType::Button, None, 80), "A"),
        ];
        for (what, a, b, on_top) in cases {
            let outcome = click(&[a, b], &layers);
            checked += 1;
            let under = if on_top == "A" { "B" } else { "A" };
            if !outcome.clicked.iter().any(|c| c == on_top) {
                failures.push(format!("{ty:?} with {what}: the control on top ({on_top}) did not answer; clicked {:?}", outcome.clicked));
            }
            if outcome.clicked.iter().any(|c| c == under) {
                failures.push(format!("{ty:?} with {what}: the control UNDER ({under}) answered too; clicked {:?}", outcome.clicked));
            }
            if outcome.changed.iter().any(|c| c == under) {
                failures.push(format!("{ty:?} with {what}: the control UNDER ({under}) changed its own value: {:?}", outcome.changed));
            }
        }
    }
    for f in &failures {
        println!("CLICK WENT THROUGH: {f}");
    }
    println!("{answered} visual control types answer a click; {checked} cases checked, {} failures", failures.len());
    assert!(answered > 10, "the probe reaches the controls: only {answered} answered a click on their own");
    assert!(failures.is_empty(), "{} cases send the click to the wrong control", failures.len());
}

/// What the click is NOT to change: a click on a child still reaches the container it sits in
/// (the container's `onClick` has always run for a click anywhere on it), a see-through overlay and
/// a Label's empty rectangle let the click through to what is under them, as they let the eye
/// through.
#[test]
fn a_click_still_reaches_what_the_eye_sees_through_to() {
    // A container hears a click on its child.
    let mut child = place("C", ControlType::Button, None, 1);
    child.parent = Some("P".into());
    child.rect = cobolt_forms::model::Rect::new(100, 60, 80, 40);
    let parent = place("P", ControlType::Panel, None, 1);
    let r = drive(&[parent, child], &[], a_click(Pos2::new(140.0, 80.0)));
    let mut clicked: Vec<&str> = r.events.iter().filter(|(_, _, e)| e == "onClick").map(|(_, id, _)| id.as_str()).collect();
    clicked.sort();
    clicked.dedup();
    assert_eq!(clicked, ["C", "P"], "a click on the child is the child's, and still its container's");

    // A Panel with no face of its own, drawn over a Button, lets the click through to it.
    let mut ghost = place("G", ControlType::Panel, None, 90);
    ghost.set_prop("HideBackground", cobolt_forms::PropValue::Bool(true));
    ghost.events.clear();
    let r = drive(&[place("A", ControlType::Button, None, 50), ghost], &[], a_click(Pos2::new(190.0, 140.0)));
    let clicked: Vec<&str> = r.events.iter().filter(|(_, _, e)| e == "onClick").map(|(_, id, _)| id.as_str()).collect();
    assert!(clicked.contains(&"A"), "a see-through Panel over a Button: the Button answers {clicked:?}");

    // A Label with no handler, big, drawn over a Button: its empty rectangle is not a wall.
    let mut label = place("L", ControlType::Label, None, 90);
    label.events.clear();
    let r = drive(&[place("A", ControlType::Button, None, 50), label], &[], a_click(Pos2::new(190.0, 140.0)));
    let clicked: Vec<&str> = r.events.iter().filter(|(_, _, e)| e == "onClick").map(|(_, id, _)| id.as_str()).collect();
    assert!(clicked.contains(&"A"), "a Label beside its text: the Button under it answers {clicked:?}");
}

/// An open ComboBox list is drawn over the whole form: a click on one of its items is the
/// list's, even where a control drawn above the combo lies under it.
#[test]
fn an_open_list_takes_the_click_over_a_control_above_its_combo() {
    let mut combo = Control::new("A", ControlType::ComboBox, 40, 40);
    combo.rect.w = 200;
    combo.rect.h = 28;
    combo.set_prop("DropDownStyle", cobolt_forms::PropValue::String("DropDownList".into()));
    combo.set_prop("Items", cobolt_forms::PropValue::String("one\ntwo\nthree".into()));
    combo.z_order = 10;
    // A Button over the place the list opens (its header is clear of it).
    let mut over = place("B", ControlType::Button, None, 90);
    over.rect = cobolt_forms::model::Rect::new(40, 72, 200, 120);
    let controls = [combo, over];
    let mut session = Session::new(&controls, &[]);
    // Open it.
    for events in a_click(Pos2::new(140.0, 54.0)) {
        session.run(events);
    }
    session.run(vec![]);
    let painted = session.run(vec![]);
    let two = painted.iter().find(|(t, _)| t == "two").map(|(_, r)| *r).expect("the list is open and shows its items");
    assert!(two.min.y > 72.0 && two.max.y < 192.0, "the item lies over the Button: {two:?}");
    // Pick the second item.
    for events in a_click(two.center()) {
        session.run(events);
    }
    let r = &session.reported;
    let picked: Vec<&(usize, String, String, String)> =
        r.props.iter().filter(|(_, id, p, _)| id == "A" && p == "SelectedIndex").collect();
    assert_eq!(picked.iter().map(|p| p.3.as_str()).collect::<Vec<_>>(), ["2"], "the click picked the second item: {:?}", r.props);
    assert!(
        !r.events.iter().any(|(_, id, e)| id == "B" && e == "onClick"),
        "and the Button under the list was not clicked: {:?}",
        r.events
    );
}
