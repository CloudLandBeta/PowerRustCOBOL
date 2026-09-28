// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A conversation scrolls all the way to its first line (operator,
//! 2026-09-27: "I cannot scroll the conversation to see the start of the
//! answer"). One agent answer taller than the pane — a heading, a table, a
//! list — and the wheel must reach its top, then come back to its end.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

const FORM: Vec2 = Vec2::new(900.0, 700.0);

fn chat(html: &str) -> Control {
    let mut c = Control::new("VWR-CHAT", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String("Streamed".into()));
    c.set_prop("_ConversationHtml", PropValue::String(html.into()));
    c
}

/// One frame of `controls`; the scroll position the chat wrote back, if any.
fn frame_all(
    ctx: &egui::Context,
    controls: &mut [Control],
    chat: usize,
    form: Vec2,
    events: Vec<egui::Event>,
) -> Option<f32> {
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), form));
    input.events = events;
    let mut updates = Vec::new();
    let snapshot = controls.to_vec();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let inp = RenderInput {
                controls: &snapshot,
                state: &DesignedState,
                form_size: form,
                glass: true,
                mode: RenderMode::Interactive,
                active_tabs: &active,
                backdrop: Default::default(),
            };
            updates = cobolt_forms::render::render_form(ui, &inp).prop_updates;
        });
    });
    full.textures_delta.clear();
    // What the host does: the write-backs go onto the control.
    let mut at = None;
    let c = &mut controls[chat];
    for (id, k, v) in updates {
        if id != c.id {
            continue;
        }
        if k == "View1ScrollPosition" {
            at = v.parse().ok();
        }
        match v.parse::<i64>() {
            Ok(n) => c.set_prop(k.as_str(), PropValue::Int(n)),
            Err(_) => c.set_prop(k.as_str(), PropValue::String(v)),
        }
    }
    at
}

fn frame(ctx: &egui::Context, c: &mut Control, events: Vec<egui::Event>) -> Option<f32> {
    let mut one = [c.clone()];
    let at = frame_all(ctx, &mut one, 0, FORM, events);
    *c = one[0].clone();
    at
}

fn wheel(dy: f32) -> Vec<egui::Event> {
    wheel_at(pos2(400.0, 300.0), dy)
}

fn wheel_at(p: egui::Pos2, dy: f32) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        },
    ]
}

fn tall_conversation() -> String {
    let mut answer = String::from("# Contract approvals\n\nThe approval depends on the value.\n\n");
    answer.push_str("| Contract value | Approvals |\n|---|---|\n");
    for i in 1..=30 {
        answer.push_str(&format!("| Up to {i} million | Director {i} + Legal |\n"));
    }
    answer.push_str("\nSpecial cases:\n\n");
    for i in 1..=20 {
        answer.push_str(&format!("- Case {i}: the Legal Director approves it.\n"));
    }
    let mut conv = Conversation::new();
    conv.append_as(AppendMode::Markdown, "How do I write a contract?", MessageRole::User);
    conv.append_as(AppendMode::Markdown, &answer, MessageRole::Agent);
    conv.to_html()
}

#[test]
fn a_tall_answer_scrolls_to_its_first_line_and_back() {
    let ctx = egui::Context::default();
    let mut c = chat(&tall_conversation());

    // It opens at its end, as a conversation does.
    let mut at = 0.0f32;
    for _ in 0..4 {
        if let Some(v) = frame(&ctx, &mut c, Vec::new()) {
            at = v;
        }
    }
    let end = at;
    assert!(end > 500.0, "the answer is taller than the pane and the view follows its end: {end}");

    // The wheel, upward, until it stops moving.
    let mut notches = 0;
    for _ in 0..400 {
        notches += 1;
        let before = at;
        if let Some(v) = frame(&ctx, &mut c, wheel(120.0)) {
            at = v;
        }
        if at == 0.0 || (notches > 20 && at == before) {
            break;
        }
    }
    let _ = frame(&ctx, &mut c, Vec::new()).map(|v| at = v);
    println!("\n  ── a conversation scrolls to its first line ──\n  opened at {end:.0} px (its end); {notches} wheel notches up → {at:.0} px");
    assert_eq!(at, 0.0, "the wheel reaches the first line of the conversation");

    // And back down to the end.
    for _ in 0..400 {
        if let Some(v) = frame(&ctx, &mut c, wheel(-120.0)) {
            at = v;
        }
        if at >= end {
            break;
        }
    }
    println!("  and back down → {at:.0} px of {end:.0}\n");
    assert!(at >= end - 1.0, "the wheel comes back to the end: {at} of {end}");
}

/// The same, on PowerChat's own chat form with every control it carries —
/// what sits around and behind the Viewer decides whether the wheel reaches it.
#[test]
fn powerchats_conversation_scrolls_to_its_first_line() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/PowerChat/forms/chat-form.cfrm");
    let form = cobolt_forms::load_form(&path).expect("PowerChat's chat form loads");
    let size = Vec2::new(form.width as f32, form.height as f32);
    let mut controls = form.controls.clone();
    let chat = controls.iter().position(|c| c.id == "Vwr-Chat").expect("the chat Viewer");
    controls[chat].set_prop("_ConversationHtml", PropValue::String(tall_conversation()));
    let r = controls[chat].rect;
    let over = pos2(r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
    let ctx = egui::Context::default();
    let mut at = 0.0f32;
    for _ in 0..4 {
        if let Some(v) = frame_all(&ctx, &mut controls, chat, size, Vec::new()) {
            at = v;
        }
    }
    let end = at;
    assert!(end > 300.0, "PowerChat's chat follows the end of a tall answer: {end}");
    for _ in 0..200 {
        if let Some(v) = frame_all(&ctx, &mut controls, chat, size, wheel_at(over, 120.0)) {
            at = v;
        }
        if at == 0.0 {
            break;
        }
    }
    println!("\n  PowerChat's chat form: opened at {end:.0} px; the wheel over the chat → {at:.0} px\n");
    assert_eq!(at, 0.0, "the wheel over PowerChat's chat reaches its first line");
}

fn button(p: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn key(k: egui::Key) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos2(400.0, 300.0)),
        egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() },
        egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() },
    ]
}

/// Without a wheel: the scrollbar's thumb dragged to the top reaches the
/// first line, and Home / End take the reader to either end (operator,
/// 2026-09-27: "do not limit how far I can scroll").
#[test]
fn the_scrollbar_and_the_keys_reach_both_ends_without_a_wheel() {
    let ctx = egui::Context::default();
    let mut c = chat(&tall_conversation());
    let mut at = 0.0f32;
    for _ in 0..4 {
        if let Some(v) = frame(&ctx, &mut c, Vec::new()) {
            at = v;
        }
    }
    let end = at;
    assert!(end > 500.0, "it opens at its end: {end}");

    // The Viewer is 40,40 700×560: its bar runs down the right edge, and at
    // the end the thumb sits at the bottom of it.
    let x = 40.0 + 700.0 - cobolt_forms::viewer::STREAM_BAR_MARGIN - cobolt_forms::viewer::STREAM_BAR_WIDTH / 2.0;
    let thumb = pos2(x, 40.0 + 560.0 - 12.0);
    let _ = frame(&ctx, &mut c, vec![egui::Event::PointerMoved(thumb)]);
    let _ = frame(&ctx, &mut c, vec![button(thumb, true)]);
    let mut y = thumb.y;
    while y > 0.0 {
        y -= 40.0;
        if let Some(v) = frame(&ctx, &mut c, vec![egui::Event::PointerMoved(pos2(x, y))]) {
            at = v;
        }
    }
    if let Some(v) = frame(&ctx, &mut c, vec![button(pos2(x, y), false)]) {
        at = v;
    }
    let _ = frame(&ctx, &mut c, Vec::new()).map(|v| at = v);
    println!("\n  thumb dragged from the bottom to the top: {end:.0} px → {at:.0} px");
    assert_eq!(at, 0.0, "the thumb dragged to the top reaches the first line");

    // End, then Home, with the pointer over the conversation.
    for (k, want) in [(egui::Key::End, end), (egui::Key::Home, 0.0)] {
        if let Some(v) = frame(&ctx, &mut c, key(k)) {
            at = v;
        }
        let _ = frame(&ctx, &mut c, Vec::new()).map(|v| at = v);
        println!("  {k:?} → {at:.0} px");
        assert!((at - want).abs() < 1.0, "{k:?} goes to {want}, got {at}");
    }
    println!();
}

/// A trackpad (or the wheel, which egui smooths over several frames) moves
/// the view a few pixels a frame. From the end of a conversation, that must
/// scroll up — every frame, from the first — and not be pulled back to the
/// end because the reader is still "near" it (operator, 2026-09-28: "the
/// scroll is neither smooth nor responsive").
#[test]
fn small_steps_scroll_up_from_the_end_smoothly() {
    let ctx = egui::Context::default();
    let mut c = chat(&tall_conversation());
    let mut at = 0.0f32;
    for _ in 0..4 {
        if let Some(v) = frame(&ctx, &mut c, Vec::new()) {
            at = v;
        }
    }
    let end = at;
    let mut trail = Vec::new();
    for _ in 0..40 {
        if let Some(v) = frame(&ctx, &mut c, wheel(4.0)) {
            at = v;
        }
        trail.push(at);
    }
    println!("\n  4 px a frame from the end ({end:.0}): {:?}\n", &trail[..10]);
    // Every frame moved, by its own step — no frame stood still or jumped back.
    let mut prev = end;
    for (i, &t) in trail.iter().enumerate() {
        assert!(t < prev, "frame {i}: {prev} -> {t} did not move up");
        assert!(prev - t <= 8.0, "frame {i}: {prev} -> {t} jumped");
        prev = t;
    }
    assert!(end - at >= 150.0, "40 small steps travel their distance: {end} -> {at}");
}
