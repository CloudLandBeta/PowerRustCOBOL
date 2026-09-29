// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

#![cfg(feature = "render")]
//! **GroupBox caption styles** — `CaptionBackgroundStyle`, `CaptionShape`,
//! `CaptionSize`, `CaptionPadding`, `CaptionAlignment`.
//!
//! The geometry is checked by hand-derived numbers for every shape and size;
//! a headless render then shows the box is painted only when a background is
//! chosen, and that the default caption lands where the classic legend did.

use cobolt_forms::model::Rect as MRect;
use cobolt_forms::paint::{
    groupbox_caption_geometry, CaptionShape, CaptionSize, GroupBoxCaptionStyle,
};
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, vec2, Pos2, Rect};

/// A 400 × 200 GroupBox at the origin with corner radius 8.
fn gb(props: &[(&str, &str)]) -> Control {
    let mut c = Control::new("GB", ControlType::GroupBox, 0, 0);
    c.rect = MRect::new(0, 0, 400, 200);
    c.set_prop("Caption", PropValue::String("Customer".into()));
    c.set_prop("CornerRadius", PropValue::Int(8));
    for (k, v) in props {
        let v = match v.parse::<i64>() {
            Ok(n) => PropValue::Int(n),
            Err(_) => PropValue::String((*v).into()),
        };
        c.set_prop(*k, v);
    }
    c
}

fn geom(props: &[(&str, &str)]) -> (Vec<Pos2>, Pos2) {
    let c = gb(props);
    let g = groupbox_caption_geometry(&c, pos2(0.0, 0.0), vec2(60.0, 12.0), &GroupBoxCaptionStyle::of(&c));
    (g.outline, g.text_pos)
}

/// Text 60 × 12, padding 4 → box height 16, top −8, bottom +8.
/// The classic legend's text starts at CornerRadius + 10 = 18.
#[test]
fn every_shape_and_size_lands_where_derived() {
    // Rectangle, Text size: 60 + 2·4 = 68 wide, from 18 − 4 = 14 to 82.
    let (o, t) = geom(&[]);
    assert_eq!(t, pos2(18.0, 0.0), "the default caption is the classic legend");
    assert_eq!(o, vec![pos2(14.0, -8.0), pos2(82.0, -8.0), pos2(82.0, 8.0), pos2(14.0, 8.0)]);

    // AngledLeft: slant = 16/2 = 8, inset 4 + 4 = 8 → box 10..86.
    // `\ … \`: top edge 10..78, bottom edge 18..86.
    let (o, t) = geom(&[("CaptionShape", "AngledLeft")]);
    assert_eq!(t.x, 18.0);
    assert_eq!(o, vec![pos2(10.0, -8.0), pos2(78.0, -8.0), pos2(86.0, 8.0), pos2(18.0, 8.0)]);

    // AngledRight: `/ … /`: top edge 18..86, bottom edge 10..78.
    let (o, _) = geom(&[("CaptionShape", "AngledRight")]);
    assert_eq!(o, vec![pos2(18.0, -8.0), pos2(86.0, -8.0), pos2(78.0, 8.0), pos2(10.0, 8.0)]);

    // Pill: two half-circles of radius 8 closing 14..82.
    let (o, _) = geom(&[("CaptionShape", "Pill")]);
    let b = Rect::from_points(&o);
    assert!((b.min.x - 14.0).abs() < 1e-3 && (b.max.x - 82.0).abs() < 1e-3, "{b:?}");
    assert!((b.min.y + 8.0).abs() < 1e-3 && (b.max.y - 8.0).abs() < 1e-3, "{b:?}");

    // Full: the whole top border, 0..400, text centred at 200 − 30 = 170.
    let (o, t) = geom(&[("CaptionSize", "Full")]);
    assert_eq!((o[0].x, o[1].x), (0.0, 400.0));
    assert_eq!(t.x, 170.0);

    // Inner: between the corners, 8..392, text still centred.
    let (o, t) = geom(&[("CaptionSize", "Inner"), ("CaptionShape", "Pill")]);
    let b = Rect::from_points(&o);
    assert!((b.min.x - 8.0).abs() < 1e-3 && (b.max.x - 392.0).abs() < 1e-3, "{b:?}");
    assert_eq!(t.x, 170.0);

    // Alignment inside a Full caption: Left → 0 + 4; Right → 400 − 4 − 60.
    assert_eq!(geom(&[("CaptionSize", "Full"), ("CaptionAlignment", "Left")]).1.x, 4.0);
    assert_eq!(geom(&[("CaptionSize", "Full"), ("CaptionAlignment", "Right")]).1.x, 336.0);
    // A Text caption centred between the corners: (8 + 392)/2 − 30 = 170.
    assert_eq!(geom(&[("CaptionAlignment", "Center")]).1.x, 170.0);

    // Padding 10 → height 22, width 60 + 20.
    let (o, _) = geom(&[("CaptionPadding", "10")]);
    let b = Rect::from_points(&o);
    assert_eq!((b.width(), b.height()), (80.0, 22.0));
    println!("caption geometry: 4 shapes × Text, Full, Inner, 4 alignments and padding checked against hand-derived rects");
}

#[test]
fn unknown_values_fall_back_to_the_classic_caption() {
    let s = GroupBoxCaptionStyle::of(&gb(&[("CaptionShape", "Hexagon"), ("CaptionSize", "Huge")]));
    assert_eq!(s.shape, CaptionShape::Rectangle);
    assert_eq!(s.size, CaptionSize::Text);
}

#[test]
fn a_new_and_a_loaded_groupbox_carry_every_caption_property() {
    let keys: Vec<&str> = cobolt_forms::model::groupbox_caption_defaults().iter().map(|(k, _)| *k).collect();
    let fresh = Control::new("G", ControlType::GroupBox, 0, 0);
    for k in &keys {
        assert!(fresh.get_prop(k).is_some(), "Control::new seeds {k}");
    }
    // A GroupBox saved before these existed gets them on load.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="400" height="300">
  <Control id="GB" type="GroupBox" x="10" y="20" w="200" h="100" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Old</Property>
  </Control>
</Form>"#;
    let form = cobolt_forms::load_form_from_str(xml).expect("parses");
    let g = form.controls.iter().find(|c| c.id == "GB").unwrap();
    for k in &keys {
        assert!(g.get_prop(k).is_some(), "the loader backfills {k}");
    }
    assert_eq!(g.get_prop("CaptionBackgroundStyle").unwrap().as_str(), "None");
    println!("caption properties: {} seeded on a new GroupBox and backfilled on a loaded one", keys.len());
}

/// Counts of filled meshes and closed outlines painted inside the caption band.
fn painted(props: &[(&str, &str)]) -> (usize, usize) {
    let mut c = gb(props);
    c.rect = MRect::new(40, 40, 400, 200);
    let controls = vec![c];
    let ctx = egui::Context::default();
    let active = cobolt_forms::containers::ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0)));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let inp = RenderInput {
                controls: &controls,
                state: &DesignedState,
                form_size: vec2(600.0, 400.0),
                glass: true,
                mode: RenderMode::Interactive,
                active_tabs: &active,
                backdrop: Backdrop::default(),
            };
            render_form(ui, &inp);
        });
    });
    full.textures_delta.clear();
    let band = Rect::from_min_max(pos2(0.0, 20.0), pos2(600.0, 60.0));
    let (mut meshes, mut lines) = (0, 0);
    fn walk(s: &egui::Shape, band: Rect, m: &mut usize, l: &mut usize) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, band, m, l)),
            egui::Shape::Mesh(mesh) if !mesh.vertices.is_empty() && mesh.vertices.iter().all(|v| band.contains(v.pos)) => *m += 1,
            egui::Shape::Path(p) if p.closed && p.points.iter().all(|q| band.contains(*q)) => *l += 1,
            _ => {}
        }
    }
    for cs in &full.shapes {
        walk(&cs.shape, band, &mut meshes, &mut lines);
    }
    (meshes, lines)
}

#[test]
fn the_box_is_painted_only_when_a_background_is_chosen() {
    let none = painted(&[("CaptionShape", "Pill"), ("CaptionSize", "Full")]);
    let flat = painted(&[("CaptionBackgroundStyle", "Flat"), ("CaptionShape", "Pill")]);
    let grad = painted(&[("CaptionBackgroundStyle", "Gradient"), ("CaptionShape", "AngledLeft"), ("CaptionSize", "Inner")]);
    assert_eq!(none, (0, 0), "no background: the classic legend, no box");
    assert!(flat.0 >= 1 && flat.1 >= 1, "Flat fills and outlines its box: {flat:?}");
    assert!(grad.0 >= 1 && grad.1 >= 1, "Gradient fills and outlines its box: {grad:?}");
    println!("caption box meshes/outlines — None: {none:?}, Flat pill: {flat:?}, Gradient angled inner: {grad:?}");
}
