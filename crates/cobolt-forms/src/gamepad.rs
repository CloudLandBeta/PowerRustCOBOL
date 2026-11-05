// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Gamepad input of Canvas2D and Canvas3D (WASM-018 T28, R13, AC12) — **the part every host shares**.
//!
//! A host that can read a gamepad (the browser's Gamepad API in the player; a desktop host that has a source, see below)
//! hands the pads it sees to the canvas as a list of [`Pad`]s. This module turns two successive lists into the events a
//! program binds (`onGamepadButton`, `onGamepadAxis`) with the argument text every host raises, and keeps the pollable
//! state the program reads (`GamepadAxis(axis)`, `GamepadButton(button)`), so a handler reads the same numbers on every host.
//!
//! **The desktop has no operating-system source yet.** `gilrs`, the crate the plan named, links `libudev` on Linux through
//! a build script that finds a system library, which WASM-018 R4 and the dependency guard forbid (the guard is not relaxed;
//! plan §5, and the test `gilrs_is_refused_by_the_dependency_guard`). [`feed`] is the entry a desktop source will call; until
//! one is written the desktop receives a gamepad only from a host that feeds it (tests do).

use std::sync::Mutex;

/// One gamepad at one moment: the axes in -1..1 and the buttons as pressed or not, in the order the host reports them (the
/// browser's "standard" mapping, a desktop source maps its own to the same indices).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pad {
    /// The pad's slot, 0 for the first.
    pub index: u32,
    pub axes: Vec<f32>,
    pub buttons: Vec<bool>,
}

/// What changed between two looks at the pads.
#[derive(Clone, Debug, PartialEq)]
pub enum PadEvent {
    Button { pad: u32, button: u32, pressed: bool },
    Axis { pad: u32, axis: u32, value: f32 },
}

/// An axis moves an event only when it changes by at least this much: a stick at rest jitters in the last digits.
pub const AXIS_STEP: f32 = 0.01;

impl PadEvent {
    /// The event the program binds.
    pub fn name(&self) -> &'static str {
        match self {
            PadEvent::Button { .. } => "onGamepadButton",
            PadEvent::Axis { .. } => "onGamepadAxis",
        }
    }

    /// The argument text, the same on every host: `pad button 1|0` and `pad axis value` (value with three decimals).
    pub fn argument(&self) -> String {
        match self {
            PadEvent::Button { pad, button, pressed } => format!("{pad} {button} {}", u8::from(*pressed)),
            PadEvent::Axis { pad, axis, value } => format!("{pad} {axis} {value:.3}"),
        }
    }
}

/// The events that take `prev` to `next`: a button that went down or up, an axis that moved by [`AXIS_STEP`] or more, and,
/// for a pad that disappeared, the release of every button it held and the return of every axis to 0.
pub fn diff(prev: &[Pad], next: &[Pad]) -> Vec<PadEvent> {
    let mut out = Vec::new();
    let find = |pads: &[Pad], i: u32| pads.iter().find(|p| p.index == i).cloned();
    let mut indices: Vec<u32> = prev.iter().chain(next.iter()).map(|p| p.index).collect();
    indices.sort_unstable();
    indices.dedup();
    for i in indices {
        let (a, b) = (find(prev, i).unwrap_or(Pad { index: i, ..Pad::default() }), find(next, i).unwrap_or(Pad { index: i, ..Pad::default() }));
        for k in 0..a.buttons.len().max(b.buttons.len()) {
            let (was, is) = (a.buttons.get(k).copied().unwrap_or(false), b.buttons.get(k).copied().unwrap_or(false));
            if was != is {
                out.push(PadEvent::Button { pad: i, button: k as u32, pressed: is });
            }
        }
        for k in 0..a.axes.len().max(b.axes.len()) {
            let (was, is) = (a.axes.get(k).copied().unwrap_or(0.0), b.axes.get(k).copied().unwrap_or(0.0));
            if (was - is).abs() >= AXIS_STEP {
                out.push(PadEvent::Axis { pad: i, axis: k as u32, value: is });
            }
        }
    }
    out
}

/// The pads as the property a canvas keeps for polling: one line per pad, `index|a0,a1,...|0110` (axes with three decimals,
/// buttons as 1 and 0). Empty when no pad is connected.
pub fn snapshot(pads: &[Pad]) -> String {
    pads.iter()
        .map(|p| {
            let axes: Vec<String> = p.axes.iter().map(|a| format!("{a:.3}")).collect();
            let buttons: String = p.buttons.iter().map(|b| if *b { '1' } else { '0' }).collect();
            format!("{}|{}|{buttons}", p.index, axes.join(","))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `GamepadAxis`: the value of `axis` of pad `pad` in a [`snapshot`]; 0 for a pad or an axis that is not there.
pub fn axis_of(snapshot: &str, pad: u32, axis: u32) -> f32 {
    pad_line(snapshot, pad).and_then(|l| l.split('|').nth(1)?.split(',').nth(axis as usize)?.parse::<f32>().ok()).unwrap_or(0.0)
}

/// `GamepadButton`: 1 when `button` of pad `pad` is down in a [`snapshot`], else 0.
pub fn button_of(snapshot: &str, pad: u32, button: u32) -> u32 {
    pad_line(snapshot, pad).and_then(|l| l.split('|').nth(2)?.chars().nth(button as usize)).map_or(0, |c| u32::from(c == '1'))
}

fn pad_line(snapshot: &str, pad: u32) -> Option<&str> {
    snapshot.lines().find(|l| l.split('|').next().and_then(|i| i.parse::<u32>().ok()) == Some(pad))
}

/// What the host's source last reported, for the canvases of this process to look at each frame.
static CURRENT: Mutex<Vec<Pad>> = Mutex::new(Vec::new());

/// A host's gamepad source reports the pads it sees now (all of them, every time).
pub fn feed(pads: Vec<Pad>) {
    if let Ok(mut c) = CURRENT.lock() {
        *c = pads;
    }
}

/// The pads the host last reported.
pub fn current() -> Vec<Pad> {
    CURRENT.lock().map(|c| c.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(axes: &[f32], buttons: &str) -> Pad {
        Pad { index: 0, axes: axes.to_vec(), buttons: buttons.chars().map(|c| c == '1').collect() }
    }

    #[test]
    fn two_looks_at_the_pads_make_the_events_a_program_binds() {
        let a = vec![pad(&[0.0, 0.0], "00")];
        let b = vec![pad(&[0.5, 0.004], "10")];
        let ev = diff(&a, &b);
        assert_eq!(ev, [PadEvent::Button { pad: 0, button: 0, pressed: true }, PadEvent::Axis { pad: 0, axis: 0, value: 0.5 }], "a jitter under the step is no event");
        assert_eq!((ev[0].name(), ev[0].argument().as_str()), ("onGamepadButton", "0 0 1"));
        assert_eq!((ev[1].name(), ev[1].argument().as_str()), ("onGamepadAxis", "0 0 0.500"));
        assert!(diff(&b, &b).is_empty(), "nothing changed, nothing raised");
    }

    #[test]
    fn a_pad_that_is_unplugged_releases_what_it_held() {
        let held = vec![pad(&[-1.0], "011")];
        let ev = diff(&held, &[]);
        assert_eq!(ev.len(), 3, "{ev:?}");
        assert!(ev.contains(&PadEvent::Button { pad: 0, button: 1, pressed: false }) && ev.contains(&PadEvent::Button { pad: 0, button: 2, pressed: false }));
        assert!(ev.contains(&PadEvent::Axis { pad: 0, axis: 0, value: 0.0 }));
        let plugged = diff(&[], &held);
        assert_eq!(plugged.len(), 3, "a pad that appears holding things announces them: {plugged:?}");
    }

    #[test]
    fn the_snapshot_is_polled_by_pad_axis_and_button_and_anything_missing_is_zero() {
        let pads = vec![Pad { index: 0, axes: vec![0.25, -1.0], buttons: vec![true, false, true] }, Pad { index: 2, axes: vec![1.0], buttons: vec![false, true] }];
        let s = snapshot(&pads);
        assert_eq!(s, "0|0.250,-1.000|101\n2|1.000|01");
        assert_eq!((axis_of(&s, 0, 0), axis_of(&s, 0, 1), axis_of(&s, 2, 0)), (0.25, -1.0, 1.0));
        assert_eq!((button_of(&s, 0, 0), button_of(&s, 0, 1), button_of(&s, 2, 1)), (1, 0, 1));
        assert_eq!((axis_of(&s, 1, 0), axis_of(&s, 0, 9), button_of(&s, 0, 9), button_of("", 0, 0)), (0.0, 0.0, 0, 0));
    }
}
