#![cfg(feature = "render")]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 110: clicking a TextBox's caption, or pressing Alt+its mnemonic, puts the
//! focus in the box. The caption takes no focus of its own.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{control_widget_id, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Event, Key, Modifiers, PointerButton, Rect, Vec2};

fn form() -> Vec<Control> {
    let mut code = Control::new("EDT-CODIGO", ControlType::TextBox, 20, 40);
    code.rect = MRect::new(20, 40, 200, 24);
    let mut name = Control::new("EDT-NOME", ControlType::TextBox, 20, 120);
    name.rect = MRect::new(20, 120, 200, 24);
    name.set_prop("CaptionText", PropValue::String("&Nome:".into()));
    vec![code, name]
}

fn frame(ctx: &egui::Context, controls: &[Control], events: Vec<Event>) {
    let size = Vec2::new(400.0, 240.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let inp = RenderInput { css_final: None,
                controls,
                state: &DesignedState,
                form_size: size,
                glass: true,
                mode: RenderMode::Interactive,
                active_tabs: &active,
                backdrop: Default::default(),
            };
            cobolt_forms::render::render_form(ui, &inp);
        });
    });
    full.textures_delta.clear();
}

fn focused(ctx: &egui::Context, id: &str) -> bool {
    ctx.memory(|m| m.has_focus(control_widget_id(None, id)))
}

fn click(at: egui::Pos2) -> Vec<Vec<Event>> {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    vec![vec![Event::PointerMoved(at)], vec![button(true)], vec![button(false)], vec![]]
}

#[test]
fn caption_focus_report() {
    let controls = form();
    let cap = controls[1].caption_layout().expect("a caption");
    let centre = pos2(cap.rect.x + cap.rect.w / 2.0, cap.rect.y + cap.rect.h / 2.0);
    let mut passed = 0;

    // Click on the caption: the box gets the focus.
    let ctx = egui::Context::default();
    frame(&ctx, &controls, vec![]);
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, "EDT-CODIGO")));
    frame(&ctx, &controls, vec![]);
    assert!(focused(&ctx, "EDT-CODIGO"), "precondition");
    for events in click(centre) {
        frame(&ctx, &controls, events);
    }
    assert!(focused(&ctx, "EDT-NOME"), "a click on the caption must focus the box");
    passed += 1;

    // Alt+N: the box gets the focus.
    let ctx = egui::Context::default();
    frame(&ctx, &controls, vec![]);
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, "EDT-CODIGO")));
    frame(&ctx, &controls, vec![]);
    let alt = Modifiers { alt: true, ..Modifiers::NONE };
    let key = Event::Key { key: Key::N, physical_key: None, pressed: true, repeat: false, modifiers: alt };
    frame(&ctx, &controls, vec![key]);
    frame(&ctx, &controls, vec![]);
    assert!(focused(&ctx, "EDT-NOME"), "Alt+N must focus the box");
    passed += 1;

    // N without Alt does not move the focus.
    let ctx = egui::Context::default();
    frame(&ctx, &controls, vec![]);
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, "EDT-CODIGO")));
    frame(&ctx, &controls, vec![]);
    let plain = Event::Key { key: Key::N, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&ctx, &controls, vec![plain]);
    assert!(!focused(&ctx, "EDT-NOME"), "N without Alt must not move the focus");
    passed += 1;

    println!("TextBox caption focus (spec 110)");
    println!("  cases: click on caption, Alt+mnemonic, mnemonic key without Alt");
    println!("  passed {passed} / failed 0");
}

/// A TextBox with a caption inside a container lands where its box does: the caption is
/// computed from the box, so the container needs no knowledge of it. (The product has no
/// "subform" or "outlet" control; a Panel and a form embedded in a ContentPane draw their
/// children the same way, through the container's origin.)
#[test]
fn a_caption_inside_a_container_follows_its_box() {
    let mut panel = Control::new("PNL", ControlType::Panel, 50, 30);
    panel.rect = MRect::new(50, 30, 320, 180);
    let mut name = Control::new("EDT-NOME", ControlType::TextBox, 150, 100);
    name.rect = MRect::new(150, 100, 200, 24);
    name.parent = Some("PNL".into());
    name.set_prop("CaptionText", PropValue::String("&Nome:".into()));
    name.set_prop("CaptionPosition", PropValue::String("Left".into()));
    let controls = vec![panel, name];

    let cap = controls[1].caption_layout().expect("a caption");
    assert!(cap.rect.x + cap.rect.w <= 150.0 && cap.rect.x >= 50.0, "Left: the caption ends before the box starts");
    let centre = pos2(cap.rect.x + cap.rect.w / 2.0, cap.rect.y + cap.rect.h / 2.0);

    let ctx = egui::Context::default();
    frame(&ctx, &controls, vec![]);
    ctx.memory_mut(|m| m.surrender_focus(control_widget_id(None, "EDT-NOME")));
    frame(&ctx, &controls, vec![]);
    assert!(!focused(&ctx, "EDT-NOME"), "precondition");
    for events in click(centre) {
        frame(&ctx, &controls, events);
    }
    assert!(focused(&ctx, "EDT-NOME"), "a click on the caption inside the panel must focus the box");
    println!("TextBox caption inside a container (spec 110): click on the Left caption focused the box; passed 1 / failed 0");
}
