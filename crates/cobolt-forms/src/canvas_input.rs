// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Input and frame pacing of Canvas2D and Canvas3D (WASM-018 T19, R13, R14).
//!
//! A canvas is **focusable** and has an accessible name (`AccessibleLabel`, else its id). It raises
//! `onPointerDown/Move/Up/Enter/Leave`, `onWheel`, `onKeyDown/Up` and `onTouchStart/Move/End` - only the
//! ones a handler is bound to - and keeps **pollable state** as properties the program reads:
//! `PointerX`, `PointerY` (canvas pixels) and the held keys (`IsKeyDown("ArrowLeft")`).
//!
//! With `TargetFps` above zero it raises `onFrame` at that rate, writing the elapsed milliseconds to
//! `ElapsedMs` first; with `FixedStep` on, it raises steps of exactly `1000 / TargetFps` ms however the
//! display paces (up to [`MAX_STEPS`] a frame - past that the time is dropped, so a stall never becomes a
//! burst). A minimised window draws no frames, so it raises none.

use crate::render::{RenderOutput, UiEvent};
use crate::Control;
use egui::{Event, Rect, Sense, Ui};

pub use crate::frame_clock::{FrameClock, MAX_STEPS};

fn num(ctrl: &Control, prop: &str) -> f64 {
    ctrl.get_prop(prop).and_then(|v| v.as_str().trim().parse::<f64>().ok()).unwrap_or(0.0)
}

fn flag(ctrl: &Control, prop: &str) -> bool {
    ctrl.get_prop(prop).is_some_and(|v| matches!(v.as_str().trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

/// The name a program sees for a key: `ArrowLeft`, `A`, `Space`, `Enter`.
pub fn key_name(key: egui::Key) -> String {
    format!("{key:?}")
}

/// Is `name` in the comma-separated held-keys list (`IsKeyDown`)? Case does not matter.
pub fn is_key_held(held: &str, name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && held.split(',').any(|k| k.trim().eq_ignore_ascii_case(n))
}

/// One canvas, one frame: focus, events, pollable state and `onFrame`.
pub(crate) fn run(ui: &mut Ui, screen: Rect, ctrl: &Control, ctrl_id: egui::Id, enabled: bool, out: &mut RenderOutput) {
    if !enabled {
        return;
    }
    let id = ctrl.id.as_str();
    let bound = |e: &str| ctrl.events.iter().any(|b| b.event.eq_ignore_ascii_case(e));

    // Focusable, with a name for assistive technology.
    let resp = ui.interact(screen, ctrl_id.with("canvas-input"), Sense::click_and_drag());
    let label = ctrl.get_prop("AccessibleLabel").map(|s| s.as_str().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| ctrl.id.clone());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
    if resp.has_focus() {
        // Arrows and Tab belong to the program while the canvas has focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false },
            )
        });
    }

    // Pointer.
    let hover = ui.input(|i| i.pointer.hover_pos());
    let over = resp.hovered() || resp.is_pointer_button_down_on();
    let local = hover.map(|p| ((p.x - screen.min.x).round() as i64, (p.y - screen.min.y).round() as i64));
    let at = local.map(|(x, y)| format!("{x} {y}")).unwrap_or_default();
    let over_mem = ctrl_id.with("canvas-over");
    let was_over = ui.ctx().memory(|m| m.data.get_temp::<bool>(over_mem)).unwrap_or(false);
    ui.ctx().memory_mut(|m| m.data.insert_temp(over_mem, over));
    let (pressed, released, moved, wheel, events) = ui.input(|i| {
        let wheel = i.events.iter().find_map(|e| match e {
            Event::MouseWheel { delta, .. } if delta.x != 0.0 || delta.y != 0.0 => Some(*delta),
            _ => None,
        });
        (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.delta() != egui::Vec2::ZERO, wheel, i.events.clone())
    });
    if over && pressed {
        resp.request_focus(); // a press gives the canvas the keyboard
    }
    if over {
        if let Some((x, y)) = local {
            let mem = ctrl_id.with("canvas-pointer");
            if ui.ctx().memory(|m| m.data.get_temp::<(i64, i64)>(mem)) != Some((x, y)) {
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem, (x, y)));
                out.prop_updates.push((id.to_string(), "PointerX".into(), x.to_string()));
                out.prop_updates.push((id.to_string(), "PointerY".into(), y.to_string()));
            }
        }
    }
    let mut raise = |event: &str, value: String| {
        if bound(event) {
            out.events.push(UiEvent::with_value(id, event, &value));
        }
    };
    if over && !was_over {
        raise("onPointerEnter", at.clone());
    }
    if !over && was_over {
        raise("onPointerLeave", at.clone());
    }
    if resp.is_pointer_button_down_on() && pressed || (over && pressed) {
        raise("onPointerDown", at.clone());
    }
    if released && (over || resp.dragged() || was_over) {
        raise("onPointerUp", at.clone());
    }
    if moved && (over || resp.dragged()) {
        raise("onPointerMove", at.clone());
    }
    if let (true, Some(d)) = (over, wheel) {
        raise("onWheel", format!("{} {}", d.x, d.y));
    }

    // Touch: the touches that land on the canvas.
    for e in &events {
        if let Event::Touch { phase, pos, .. } = e {
            if screen.contains(*pos) {
                let at = format!("{} {}", (pos.x - screen.min.x).round() as i64, (pos.y - screen.min.y).round() as i64);
                match phase {
                    egui::TouchPhase::Start => raise("onTouchStart", at),
                    egui::TouchPhase::Move => raise("onTouchMove", at),
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => raise("onTouchEnd", at),
                }
            }
        }
    }

    // Keyboard, while the canvas has focus; the held keys are pollable state.
    let keys_mem = ctrl_id.with("canvas-keys");
    let mut held: Vec<String> = ui.ctx().memory(|m| m.data.get_temp::<Vec<String>>(keys_mem)).unwrap_or_default();
    let before = held.clone();
    if resp.has_focus() {
        for e in &events {
            if let Event::Key { key, pressed: down, repeat: false, .. } = e {
                let name = key_name(*key);
                if *down {
                    if !held.contains(&name) {
                        held.push(name.clone());
                    }
                    raise("onKeyDown", name);
                } else {
                    held.retain(|k| *k != name);
                    raise("onKeyUp", name);
                }
            }
        }
    } else {
        held.clear();
    }
    if held != before {
        ui.ctx().memory_mut(|m| m.data.insert_temp(keys_mem, held.clone()));
        out.prop_updates.push((id.to_string(), "_Keys".into(), held.join(",")));
    }

    // Gamepads (T28): whatever the host's source reports, whether or not the canvas has the keyboard. The pads the canvas last
    // saw are kept with it; `GamepadAxis` and `GamepadButton` read the `_Pads` snapshot, and a bound handler is raised for
    // each change, with the argument text every host raises (`gamepad::PadEvent::argument`).
    let pads_mem = ctrl_id.with("canvas-pads");
    let seen: Vec<crate::gamepad::Pad> = ui.ctx().memory(|m| m.data.get_temp::<Vec<crate::gamepad::Pad>>(pads_mem)).unwrap_or_default();
    let now = crate::gamepad::current();
    if now != seen {
        for e in crate::gamepad::diff(&seen, &now) {
            raise(e.name(), e.argument());
        }
        out.prop_updates.push((id.to_string(), "_Pads".into(), crate::gamepad::snapshot(&now)));
        ui.ctx().memory_mut(|m| m.data.insert_temp(pads_mem, now));
    }

    // onFrame at TargetFps, with the elapsed time.
    let fps = num(ctrl, "TargetFps");
    let clock_mem = ctrl_id.with("canvas-clock");
    let mut clock = ui.ctx().memory(|m| m.data.get_temp::<FrameClock>(clock_mem)).unwrap_or_default();
    let hidden = ui.input(|i| i.viewport().minimized == Some(true));
    if fps > 0.0 && !hidden {
        for dt in clock.advance(ui.input(|i| i.time) * 1000.0, fps, flag(ctrl, "FixedStep")) {
            if bound("onFrame") {
                out.prop_updates.push((id.to_string(), "ElapsedMs".into(), format!("{dt:.3}")));
                out.events.push(UiEvent::with_value(id, "onFrame", &format!("{dt:.3}")));
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(1.0 / fps.min(240.0)));
    } else {
        clock.pause();
    }
    ui.ctx().memory_mut(|m| m.data.insert_temp(clock_mem, clock));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_keys_are_found_by_name_ignoring_case() {
        assert!(is_key_held("ArrowLeft,Space", "arrowleft"));
        assert!(!is_key_held("ArrowLeft", "ArrowRight"));
        assert!(!is_key_held("", ""));
        assert_eq!(key_name(egui::Key::ArrowLeft), "ArrowLeft");
    }
}
