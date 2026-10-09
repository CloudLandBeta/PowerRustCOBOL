//! Spec 091, slice 2c — **only the base has layout behaviour** (R21).
//!
//! `Dock`, `Anchor` and the flex, grid and flow layouts of spec 056 belong to the
//! base. A control in a layer keeps its designed `X`, `Y`, `Width` and `Height`,
//! and so does everything inside a container in a layer (R8). A layer neither
//! takes room from the base's docks, nor moves with its anchors, nor raises the
//! form's smallest size (R13).

use std::collections::BTreeMap;

use cobolt_forms::layout::apply::{laid_out_controls, LAID_OUT};
use cobolt_forms::layout::{solve, window_min_size, LayoutInput, LayoutOutput};
use cobolt_forms::model::{Control, ControlType, PropValue, Rect};

fn ctrl(id: &str, ct: ControlType, r: (i32, i32, i32, i32), z: i32) -> Control {
    let mut c = Control::new(id, ct, r.0, r.1);
    c.rect = Rect::new(r.0, r.1, r.2, r.3);
    c.z_order = z;
    c
}

fn dock(mut c: Control, how: &str) -> Control {
    c.set_prop("Dock", PropValue::String(how.into()));
    c
}

fn anchor(mut c: Control, how: &str) -> Control {
    c.set_prop("Anchor", PropValue::String(how.into()));
    c
}

fn in_layer(mut c: Control, layer: &str) -> Control {
    c.layer = Some(layer.into());
    c
}

fn child_of(mut c: Control, parent: &str) -> Control {
    c.parent = Some(parent.into());
    c
}

/// The base: a Top dock, a Fill dock and a bottom-right anchored button — the
/// layout the docks-and-anchors spec's own acceptance test uses.
fn base() -> Vec<Control> {
    vec![
        dock(ctrl("T", ControlType::Panel, (0, 0, 10, 50), 1), "Top"),
        dock(ctrl("F", ControlType::Panel, (0, 0, 10, 10), 2), "Fill"),
        anchor(ctrl("B", ControlType::Button, (700, 550, 80, 30), 0), "Bottom,Right"),
    ]
}

/// What a layer might hold, every one asking to be laid out.
fn layer_controls() -> Vec<Control> {
    vec![
        in_layer(dock(ctrl("LP", ControlType::Panel, (10, 10, 300, 200), 3), "Fill"), "Layer-1"),
        child_of(dock(ctrl("LK", ControlType::Button, (5, 5, 60, 20), 1), "Fill"), "LP"),
        in_layer(anchor(ctrl("LB", ControlType::Button, (700, 550, 80, 30), 4), "Bottom,Right"), "Layer-1"),
        in_layer(dock(ctrl("LT", ControlType::Panel, (0, 0, 10, 40), 5), "Top"), "Layer-2"),
    ]
}

fn run(controls: &[Control], available: (f32, f32)) -> LayoutOutput {
    let bag = BTreeMap::new();
    solve(&LayoutInput::new(controls, (800.0, 600.0), available, &bag, &[]))
}

#[test]
fn a_layer_does_not_change_where_the_base_is_laid_out_091() {
    let alone = run(&base(), (1000.0, 700.0));
    let mut with = base();
    with.extend(layer_controls());
    let layered = run(&with, (1000.0, 700.0));

    // The base's docks and anchors give the answer they gave without the layer:
    // the layer's Fill did not take the remainder, its Top did not take a strip.
    for id in ["T", "F", "B"] {
        assert_eq!(layered.rects[id], alone.rects[id], "{id}: the base is laid out as if the layer were not there");
    }
    assert_eq!(layered.rects["F"], alone.rects["F"]);
    eprintln!("\n  base T/F/B at 1000x700: {:?} {:?} {:?}", alone.rects["T"], alone.rects["F"], alone.rects["B"]);
}

#[test]
fn a_control_in_a_layer_is_laid_out_nowhere_091() {
    let mut with = base();
    with.extend(layer_controls());
    let o = run(&with, (1000.0, 700.0));
    for id in ["LP", "LK", "LB", "LT"] {
        assert!(!o.rects.contains_key(id), "{id} has no laid-out rectangle");
        assert!(!o.font_sizes.contains_key(id), "{id} keeps its designed font: it is scaled by no layout");
    }
    for id in ["T", "F", "B"] {
        assert!(o.rects.contains_key(id) && o.font_sizes.contains_key(id), "{id} is laid out");
    }

    // What the host applies: the layer's controls keep their DESIGNED rectangle,
    // inside a container too, and are not marked laid out.
    let applied = laid_out_controls(&with, &o);
    let by = |id: &str| applied.iter().find(|c| c.id == id).unwrap();
    for (id, r) in [
        ("LP", Rect::new(10, 10, 300, 200)),
        ("LK", Rect::new(5, 5, 60, 20)),
        ("LB", Rect::new(700, 550, 80, 30)),
        ("LT", Rect::new(0, 0, 10, 40)),
    ] {
        assert_eq!(by(id).rect, r, "{id} keeps its designed X, Y, Width and Height (R21)");
        assert!(by(id).get_prop(LAID_OUT).is_none(), "{id}: not marked laid out, so AutoSize still measures it");
    }
    for id in ["T", "F", "B"] {
        assert!(by(id).get_prop(LAID_OUT).is_some(), "{id} is marked laid out");
    }
    assert_ne!(by("F").rect, Rect::new(0, 0, 10, 10), "the base's Fill was laid out");
}

#[test]
fn a_layer_never_raises_the_form_s_smallest_size_091() {
    let bag = BTreeMap::new();
    let alone = window_min_size(&base(), (800.0, 600.0), &bag, &[]);
    // A stretched control that needs 3,000 px: in the base it holds the window
    // open that wide.
    let huge = || {
        let mut c = anchor(ctrl("HUGE", ControlType::TextBox, (20, 20, 300, 30), 9), "Top,Left,Right");
        c.set_prop("MinWidth", PropValue::Int(3000));
        c
    };
    let mut with = base();
    with.push(in_layer(huge(), "Layer-1"));
    with.extend(layer_controls());
    let layered = window_min_size(&with, (800.0, 600.0), &bag, &[]);
    assert_eq!(layered, alone, "a layer does not change the form's size (R13)");

    // The same control in the base DOES raise it: the check can see a change.
    let mut in_base = base();
    in_base.push(huge());
    let raised = window_min_size(&in_base, (800.0, 600.0), &bag, &[]);
    assert!(raised.0 > 2500.0 && raised.0 > alone.0, "the negative control: {raised:?} against {alone:?}");
}

#[test]
fn a_control_naming_an_undefined_layer_keeps_its_designed_rectangle_091() {
    // Drawn with the base (R40), but never docked into a layout it was not
    // designed for.
    let mut with = base();
    with.push(in_layer(dock(ctrl("GHOST", ControlType::Panel, (30, 30, 100, 100), 7), "Fill"), "NoSuchLayer"));
    let o = run(&with, (1000.0, 700.0));
    assert!(!o.rects.contains_key("GHOST"));
    let applied = laid_out_controls(&with, &o);
    assert_eq!(applied.iter().find(|c| c.id == "GHOST").unwrap().rect, Rect::new(30, 30, 100, 100));
    // …and the base is untouched by it.
    assert_eq!(o.rects["F"], run(&base(), (1000.0, 700.0)).rects["F"]);
}

#[test]
fn a_form_with_no_layer_is_laid_out_exactly_as_before_091() {
    // The base alone, and the base with controls whose `layer` is the base's own
    // name, give the same answer: nothing moved for a form that uses no layer.
    let plain = run(&base(), (1000.0, 700.0));
    let mut named = base();
    for c in &mut named {
        c.layer = Some("Form".into());
    }
    assert_eq!(run(&named, (1000.0, 700.0)), plain);
    assert_eq!(laid_out_controls(&base(), &plain).len(), 3);
}
