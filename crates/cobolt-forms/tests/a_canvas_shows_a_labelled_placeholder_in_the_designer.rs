#![cfg(feature = "render")]
//! WASM-018 T20 (R17, AC13 Designer): in the Form Designer a Canvas2D and a Canvas3D are a labelled
//! placeholder - the control's name and kind, its designed size and, for the 3D one, the backend - drawn
//! centred in the control; with a picture (the run form's path) the placeholder text is gone.
//!
//! The form is saved to a `.cfrm` and loaded again, so what is painted is what a designer opens.

use cobolt_forms::model::{Control, ControlType, Form, Rect as MRect};
use cobolt_forms::paint::{draw_control, set_designer_backend};
use cobolt_forms::xml::{form_to_string, load_form_from_str};
use egui::{Pos2, Vec2};

/// Every text `draw_control` painted, with where its anchor lay.
fn texts(ctrl: &Control, tex: Option<egui::TextureId>) -> Vec<(String, Pos2)> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0)));
    fn walk(s: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos + Vec2::new(t.galley.size().x / 2.0, t.galley.size().y / 2.0))),
            _ => {}
        }
    }
    let mut found = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root, |ui| {
            draw_control(&ui.painter().clone(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, tex);
        });
    });
    full.textures_delta.clear();
    for cs in &full.shapes {
        walk(&cs.shape, &mut found);
    }
    found
}

#[test]
fn both_canvases_are_labelled_placeholders_with_name_size_and_backend() {
    let mut form = Form::new("MAIN-FORM", "Canvases", 640, 480);
    for (id, kind, x, w, h) in [("GAME-VIEW", ControlType::Canvas2D, 10, 320, 240), ("SCENE-VIEW", ControlType::Canvas3D, 340, 200, 150)] {
        let mut c = Control::new(id, kind, x, 20);
        c.rect = MRect::new(x, 20, w, h);
        form.controls.push(c);
    }
    let reloaded = load_form_from_str(&form_to_string(&form).expect("save")).expect("a designer opens it");
    set_designer_backend("Metal");

    let started = std::time::Instant::now();
    let flat = reloaded.controls.clone();
    let two = flat.iter().find(|c| c.id == "GAME-VIEW").unwrap();
    let three = flat.iter().find(|c| c.id == "SCENE-VIEW").unwrap();
    let t2 = texts(two, None);
    let t3 = texts(three, None);
    println!("T20: Canvas2D painted {:?}", t2.iter().map(|t| t.0.as_str()).collect::<Vec<_>>());
    println!("T20: Canvas3D painted {:?} ({} us)", t3.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), started.elapsed().as_micros());

    let said = |t: &[(String, Pos2)], s: &str| t.iter().any(|(x, _)| x == s);
    assert!(said(&t2, "GAME-VIEW  Canvas2D") && said(&t2, "320 x 240"), "{t2:?}");
    assert!(!t2.iter().any(|(x, _)| x.contains("Backend")), "a 2D canvas has no backend");
    assert!(said(&t3, "SCENE-VIEW  Canvas3D") && said(&t3, "200 x 150") && said(&t3, "Backend: Metal"), "{t3:?}");

    // centred horizontally in the control's own rectangle, the lines stacked top to bottom
    let (cx2, cx3) = (two.rect.x as f32 + two.rect.w as f32 / 2.0, three.rect.x as f32 + three.rect.w as f32 / 2.0);
    for (x, p) in t2.iter().filter(|t| t.0.contains('x')) {
        assert!((p.x - cx2).abs() < 1.5, "{x}: {} vs {cx2}", p.x);
    }
    for (x, p) in &t3 {
        assert!((p.x - cx3).abs() < 1.5, "{x}: {} vs {cx3}", p.x);
    }
    let ys: Vec<f32> = ["SCENE-VIEW  Canvas3D", "200 x 150", "Backend: Metal"].iter().map(|s| t3.iter().find(|t| t.0 == *s).unwrap().1.y).collect();
    assert!(ys[0] < ys[1] && ys[1] < ys[2], "name, then size, then backend: {ys:?}");
    assert!(ys[1] > three.rect.y as f32 && ys[1] < (three.rect.y + three.rect.h) as f32, "inside the control: {ys:?}");

    // no backend on this machine: the placeholder says so, readably, in the IDE's language
    set_designer_backend("");
    cobolt_forms::paint::set_designer_no_adapter_text("3D cannot be drawn on this machine: no usable graphics adapter was found.");
    let none = texts(three, None);
    assert!(said(&none, "Backend: None") && said(&none, "3D cannot be drawn on this machine: no usable graphics adapter was found."), "{none:?}");
    println!("T22: Canvas3D without a backend painted {:?}", none.iter().map(|t| t.0.as_str()).collect::<Vec<_>>());

    // a picture replaces the placeholder
    let shown = texts(two, Some(egui::TextureId::User(1)));
    assert!(shown.is_empty(), "with a picture there is no placeholder text: {shown:?}");
}
