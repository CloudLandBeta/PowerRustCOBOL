// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Snackbar's action buttons answer the pointer.
//!
//! They painted one flat well whatever the pointer did, so a click landed —
//! `draw_snackbars` has hit-tested the reported rects since the control shipped
//! — and nothing on screen acknowledged it (operator, 2026-09-01). A button that
//! never changes reads as decoration.
//!
//! The standing trap this file avoids is the one its neighbours name: a shape
//! COUNT proves nothing. Every assertion here compares the button well's
//! **colour** between pointer states, at the rect the painter itself reported.

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::{draw_snackbar, SnackPointer};
use cobolt_forms::snackbar::mint;
use egui::{Color32, Pos2, Rect, Vec2};

const AT: Rect = Rect {
    min: Pos2 { x: 100.0, y: 100.0 },
    max: Pos2 { x: 500.0, y: 156.0 },
};

fn two_button_snack() -> cobolt_forms::snackbar::SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    c.set_prop(
        "Buttons",
        PropValue::String("retry|Retry|refresh|Left|true\nclose|Close||Left|true".into()),
    );
    mint(&c).0
}

/// Paint once under `pointer`; answer the button rects and the fill each one's
/// well was given.
fn wells(
    v: &cobolt_forms::snackbar::SnackVisual,
    pointer: SnackPointer,
) -> (Vec<Rect>, Vec<Color32>) {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));

    let mut reported = Vec::new();
    let mut painted: Vec<(Rect, Color32)> = Vec::new();
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                reported = draw_snackbar(ui.painter(), AT, v, None, 1.0, pointer).buttons;
            });
    });
    full.textures_delta.clear();

    fn collect(s: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            _ => {}
        }
    }
    for cs in &full.shapes {
        collect(&cs.shape, &mut painted);
    }

    // The well is the filled rect drawn AT the button's own rect.
    let fills = reported
        .iter()
        .map(|br| {
            painted
                .iter()
                .find(|(r, _)| (r.min - br.min).length() < 0.5 && (r.max - br.max).length() < 0.5)
                .map(|(_, c)| *c)
                .unwrap_or_else(|| panic!("no well painted at the reported button rect {br:?}"))
        })
        .collect();
    (reported, fills)
}

#[test]
fn a_buttons_well_changes_under_the_pointer_and_again_when_it_is_held() {
    let v = two_button_snack();

    let (rects, idle) = wells(&v, SnackPointer::inert());
    assert_eq!(rects.len(), 2, "the fixture must paint two buttons");

    let over_first = SnackPointer {
        pos: Some(rects[0].center()),
        held: false,
    };
    let held_first = SnackPointer {
        pos: Some(rects[0].center()),
        held: true,
    };

    let (_, hovered) = wells(&v, over_first);
    let (_, pressed) = wells(&v, held_first);

    assert_ne!(
        idle[0], hovered[0],
        "hovering a button must change its well: idle {:?} vs hovered {:?}",
        idle[0], hovered[0]
    );
    assert_ne!(
        hovered[0], pressed[0],
        "pressing must differ from merely hovering: hovered {:?} vs pressed {:?}",
        hovered[0], pressed[0]
    );
    assert_ne!(idle[0], pressed[0], "pressed must differ from idle too");

    // The engagement reads as the layer becoming more present, not less.
    assert!(
        hovered[0].a() > idle[0].a() && pressed[0].a() > hovered[0].a(),
        "idle {} < hovered {} < pressed {} in alpha",
        idle[0].a(),
        hovered[0].a(),
        pressed[0].a()
    );

    eprintln!(
        "\n  Snackbar button well — idle a={}, hovered a={}, pressed a={}\n",
        idle[0].a(),
        hovered[0].a(),
        pressed[0].a()
    );
}

#[test]
fn only_the_button_under_the_pointer_reacts() {
    // One well lighting up the whole row would say the wrong thing about which
    // button a click is about to hit.
    let v = two_button_snack();
    let (rects, idle) = wells(&v, SnackPointer::inert());
    let (_, hovered) = wells(
        &v,
        SnackPointer {
            pos: Some(rects[0].center()),
            held: false,
        },
    );

    assert_ne!(idle[0], hovered[0], "the hovered button must react");
    assert_eq!(
        idle[1], hovered[1],
        "its neighbour must not: idle {:?} vs {:?}",
        idle[1], hovered[1]
    );
}

#[test]
fn a_pointer_outside_every_button_leaves_them_all_idle() {
    // Hovering the notification's text is not hovering a button.
    let v = two_button_snack();
    let (rects, idle) = wells(&v, SnackPointer::inert());
    let outside = Pos2::new(AT.min.x + 4.0, AT.center().y);
    assert!(
        !rects.iter().any(|r| r.contains(outside)),
        "the probe point must miss every button"
    );

    let (_, elsewhere) = wells(
        &v,
        SnackPointer {
            pos: Some(outside),
            held: true,
        },
    );
    assert_eq!(idle, elsewhere, "no button may react to a pointer off it");
}
