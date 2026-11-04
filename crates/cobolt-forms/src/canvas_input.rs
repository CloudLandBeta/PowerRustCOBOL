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

/// The most fixed steps one display frame may carry.
pub const MAX_STEPS: usize = 5;

/// A frame deadline reached a whisker early still counts (repaint scheduling has millisecond granularity).
const TOLERANCE_MS: f64 = 1.0;

/// The pacing of one canvas's `onFrame`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameClock {
    last_ms: Option<f64>,
    owed_ms: f64,
    due_ms: f64,
}

impl FrameClock {
    /// The elapsed times (ms) of the `onFrame` events due at `now_ms`. Variable: at most one, the time
    /// since the last. Fixed: as many whole steps of `1000 / fps` as have accrued.
    pub fn advance(&mut self, now_ms: f64, fps: f64, fixed: bool) -> Vec<f64> {
        let Some(last) = self.last_ms.replace(now_ms) else { return Vec::new() };
        if fps <= 0.0 {
            return Vec::new();
        }
        let step = 1000.0 / fps;
        let gap = (now_ms - last).max(0.0);
        if !fixed {
            // the schedule advances by exactly one step, so the mean rate is the target however the display
            // quantises it; a frame that lands late by more than a step resyncs instead of repaying
            if self.due_ms == 0.0 {
                self.due_ms = last + step;
            }
            if now_ms + TOLERANCE_MS < self.due_ms {
                self.last_ms = Some(last);
                return Vec::new();
            }
            self.due_ms = if now_ms - self.due_ms > step { now_ms + step } else { self.due_ms + step };
            return vec![gap];
        }
        self.owed_ms += gap;
        let mut out = Vec::new();
        while self.owed_ms + TOLERANCE_MS >= step && out.len() < MAX_STEPS {
            self.owed_ms = (self.owed_ms - step).max(0.0);
            out.push(step);
        }
        if out.len() == MAX_STEPS {
            self.owed_ms = 0.0;
        }
        out
    }

    /// The window was hidden: forget the past so showing it again does not repay it.
    pub fn pause(&mut self) {
        self.last_ms = None;
        self.owed_ms = 0.0;
        self.due_ms = 0.0;
    }
}

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

    /// Variable pacing: the rate holds within a tolerance, the elapsed time is the real gap.
    #[test]
    fn on_frame_runs_at_the_target_rate_within_a_tolerance() {
        let mut c = FrameClock::default();
        let (fps, display_hz) = (30.0, 144.0);
        let mut t = 0.0;
        let mut frames = Vec::new();
        let started = std::time::Instant::now();
        while t < 10_000.0 {
            frames.extend(c.advance(t, fps, false));
            t += 1000.0 / display_hz;
        }
        let rate = frames.len() as f64 / 10.0;
        let mean = frames.iter().sum::<f64>() / frames.len() as f64;
        println!("T19: onFrame at TargetFps 30 on a 144 Hz display, 10 s: {} frames = {rate:.2}/s, mean elapsed {mean:.2} ms ({} us to compute)", frames.len(), started.elapsed().as_micros());
        assert!((rate - fps).abs() <= 3.0, "{rate}");
        assert!(frames.iter().all(|d| *d >= 1000.0 / fps - 1000.0 / display_hz - 0.01), "no delta is shorter than the display could make it");
        assert!((mean - 1000.0 / fps).abs() < 1.5, "the mean elapsed time is the period: {mean}");
    }

    /// Fixed step: every delta is exactly 1000/fps, and the count follows the elapsed time.
    #[test]
    fn fixed_step_deltas_are_exact_whatever_the_display_does() {
        let mut c = FrameClock::default();
        let fps = 60.0;
        let step = 1000.0 / fps;
        let mut t = 0.0;
        let mut steps = Vec::new();
        for gap in [7.0, 31.0, 5.0, 60.0, 16.0, 16.0, 2.0, 40.0].iter().cycle().take(400) {
            steps.extend(c.advance(t, fps, true));
            t += gap;
        }
        let expected = (t / step).floor();
        println!("T19: fixed step 60 fps over {t:.0} ms: {} steps (expected about {expected}), every delta {step:.4} ms", steps.len());
        assert!(steps.iter().all(|d| (*d - step).abs() < 1e-9));
        assert!((steps.len() as f64 - expected).abs() <= MAX_STEPS as f64 * 2.0 + 5.0, "{} vs {expected}", steps.len());
    }

    /// A stall never becomes a burst; hidden time is not repaid.
    #[test]
    fn a_stall_is_not_repaid_and_hidden_means_no_frames() {
        let mut c = FrameClock::default();
        c.advance(0.0, 60.0, true);
        let burst = c.advance(10_000.0, 60.0, true);
        assert_eq!(burst.len(), MAX_STEPS);
        c.pause();
        assert!(c.advance(20_000.0, 60.0, false).is_empty(), "the first frame after a pause only sets the mark");
        assert!(c.advance(20_005.0, 0.0, false).is_empty(), "TargetFps 0: no frames");
    }

    #[test]
    fn held_keys_are_found_by_name_ignoring_case() {
        assert!(is_key_held("ArrowLeft,Space", "arrowleft"));
        assert!(!is_key_held("ArrowLeft", "ArrowRight"));
        assert!(!is_key_held("", ""));
        assert_eq!(key_name(egui::Key::ArrowLeft), "ArrowLeft");
    }
}
