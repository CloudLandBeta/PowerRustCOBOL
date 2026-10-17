// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The keyboard while a task runs (spec 104 R22, R23).
//!
//! Called at the **start** of each OS window's pass, before anything reads a key. Every key
//! reader in this crate bottoms out in `InputState::events` (see the Walkthrough's
//! `take_keys`), so filtering that list here is what keeps the IDE's own shortcuts from
//! acting while a step shows — all of them at once, rather than a gate per shortcut that
//! would rot.
//!
//! What gets through:
//! - with focus **inside the live region**, every key — the developer is typing in the
//!   field the step lit;
//! - on a step whose policy is [`KeyPolicy::Viewport`] (the canvas), every key in that
//!   window — arrows and Delete are how a canvas is worked;
//! - otherwise nothing, and focus held outside the live region is let go.
//!
//! `Esc` leaves the innermost thing the developer is in, and only that — one press never does
//! two things (R22):
//! 1. a text field that had the keyboard: the `Esc` is **passed on**, so the field does what
//!    its own `Esc` does — a DataGrid cell editor or a column rename *cancels* the edit (egui
//!    has already taken the focus away by then; swallowing the key would turn the cancel into
//!    a commit);
//! 2. an open popup: the wizard closes it and keeps the key;
//! 3. a mode of the window (a grid's edit mode on the canvas): the `Esc` is passed on, so the
//!    mode leaves itself;
//! 4. otherwise the wizard keeps the key, and it is **Exit**.

use eframe::egui::{self, Event, Key, Rect};

use super::{KeyPolicy, Nav, Plan};

/// Filter this window's input for the frame.
///
/// - `live` is where the overlay last painted the live region in this window;
/// - `had_field` is the text field that held the keyboard when the last frame ended — egui
///   drops the focus on `Esc` before any widget runs, so "was a field focused?" has to be
///   asked of the frame before;
/// - `in_mode` says this window is in a mode its own `Esc` leaves (a grid's edit mode).
///
/// Returns `Some(Nav::Exit)` when `Esc` meant Exit.
pub fn filter(
    ctx: &egui::Context,
    plan: Option<&Plan>,
    live: Option<Rect>,
    had_field: Option<egui::Id>,
    in_mode: bool,
) -> Option<Nav> {
    let plan = plan?;
    let here = plan.viewport == ctx.viewport_id();
    let focused = ctx.memory(|m| m.focused());
    let focus_rect = focused
        .or(had_field)
        .and_then(|id| ctx.read_response(id))
        .map(|r| r.rect);
    let focus_inside = match (focus_rect, live) {
        (Some(f), Some(l)) => here && l.expand(1.0).contains_rect(f),
        _ => false,
    };
    let pass_all = here && plan.keys == KeyPolicy::Viewport;

    let mut nav = None;
    // Whether this frame's `Esc` goes on to the IDE (stages 1 and 3).
    let mut pass_esc = false;
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    if esc {
        if had_field.is_some() || ctx.text_edit_focused() {
            pass_esc = true;
        } else if egui::Popup::is_any_open(ctx) {
            egui::Popup::close_all(ctx);
        } else if in_mode && here {
            pass_esc = true;
        } else {
            nav = Some(Nav::Exit);
        }
        if !pass_esc {
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        Event::Key {
                            key: Key::Escape,
                            ..
                        }
                    )
                });
                i.keys_down.remove(&Key::Escape);
            });
        }
    }

    if !(focus_inside || pass_all) {
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                let escape = matches!(
                    e,
                    Event::Key {
                        key: Key::Escape,
                        ..
                    }
                );
                (pass_esc && escape)
                    || !matches!(
                        e,
                        Event::Key { .. }
                            | Event::Text(_)
                            | Event::Ime(_)
                            | Event::Copy
                            | Event::Cut
                            | Event::Paste(_)
                    )
            });
            let keep_esc = pass_esc && i.keys_down.contains(&Key::Escape);
            i.keys_down.clear();
            if keep_esc {
                i.keys_down.insert(Key::Escape);
            }
        });
        // Focus outside the live region is let go, so nothing there keeps typing.
        if let Some(id) = focused {
            if !esc {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
    }
    nav
}
