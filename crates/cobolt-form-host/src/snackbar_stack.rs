// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Snackbar's **live** stack (spec 055) — the part that has a lifetime.
//!
//! `cobolt-forms` is a pure renderer: given controls and state, it paints. It has
//! no clock and owns nothing that outlives a frame. A notification has all three
//! — a lifetime, a timeout and a hover-pause — so the stack lives here, where the
//! host already owns cross-frame state, and the engine is handed a list of *what
//! to draw this frame* (plan §1).
//!
//! Both live surfaces consume this crate, so `rcrun run-form` and a compiled
//! binary get one implementation and cannot drift (R25).
//!
//! # The clock is a parameter
//!
//! Every method that needs "now" takes it. Nothing here calls `Instant::now()`
//! and nothing sleeps, so the tests drive a whole notification lifetime — raise,
//! hover, resume, expire — in microseconds and always reach the same answer.
//! Timing state that reads its own clock is timing state that flakes (plan §5).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use cobolt_forms::model::Rect;
use cobolt_forms::snackbar::{stack_layout, DismissReason, SnackAnchor, SnackVisual};

/// One notification that is actually up.
#[derive(Debug, Clone)]
pub struct LiveNotification {
    /// Unique for the life of the stack. Monotonic, so a larger id is younger.
    pub id: u64,
    /// The Snackbar control that raised it — `DismissAll()` is scoped to this
    /// (R9), and so are `MaximumVisible` / `OverflowBehavior`.
    pub ctrl_id: String,
    /// The snapshot `Show()` minted (D2). Editing the template afterwards must
    /// not rewrite a message already on screen.
    pub visual: SnackVisual,
    pub raised_at: Instant,
    /// Time spent paused under the pointer, already banked.
    paused_total: Duration,
    /// When the current pause began; `None` when not paused.
    paused_since: Option<Instant>,
    /// Where it was last laid out — the host hit-tests the pointer against this.
    pub rect: Option<Rect>,
}

impl LiveNotification {
    /// How long this notification has been *counting* — wall time since it was
    /// raised, less every moment the pointer held it (R7).
    pub fn elapsed(&self, now: Instant) -> Duration {
        let wall = now.saturating_duration_since(self.raised_at);
        let paused = self.paused_total
            + self
                .paused_since
                .map(|s| now.saturating_duration_since(s))
                .unwrap_or_default();
        wall.saturating_sub(paused)
    }

    /// Milliseconds left before it expires; `None` when `Timeout` is 0 — it
    /// never expires by itself (R6).
    pub fn remaining_ms(&self, now: Instant) -> Option<i64> {
        if self.visual.timeout_ms <= 0 {
            return None;
        }
        Some(self.visual.timeout_ms - self.elapsed(now).as_millis() as i64)
    }

    /// True while the pointer is holding its timeout.
    pub fn is_paused(&self) -> bool {
        self.paused_since.is_some()
    }
}

/// What the stack did, for the host to turn into COBOL events.
///
/// The stack does not know what a handler is — it reports, and the host
/// dispatches. That keeps the whole lifetime testable without an interpreter.
#[derive(Debug, Clone, PartialEq)]
pub enum SnackEvent {
    Shown { id: u64, ctrl_id: String },
    /// Raised when the timeout elapsed, **before** `Closing` (spec §6).
    Timeout { id: u64, ctrl_id: String },
    Closing { id: u64, ctrl_id: String, reason: DismissReason },
    Closed { id: u64, ctrl_id: String, reason: DismissReason },
    ButtonClick { id: u64, ctrl_id: String, button_id: String, index: usize },
}

/// The per-surface stack (Q1/Q2: one per surface, and a notification belongs to
/// the surface it was raised on).
#[derive(Debug, Default)]
pub struct SnackbarStack {
    /// Live notifications, **oldest first**.
    live: Vec<LiveNotification>,
    /// Held back by `OverflowBehavior::Queue`, oldest first.
    queued: VecDeque<(String, SnackVisual)>,
    next_id: u64,
    /// Drained by the host each frame.
    events: Vec<SnackEvent>,
}

impl SnackbarStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live(&self) -> &[LiveNotification] {
        &self.live
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty() && self.queued.is_empty()
    }

    /// Take everything that happened since the last call.
    pub fn drain_events(&mut self) -> Vec<SnackEvent> {
        std::mem::take(&mut self.events)
    }

    /// How many of this control's notifications are up.
    fn live_for(&self, ctrl_id: &str) -> usize {
        self.live.iter().filter(|n| n.ctrl_id == ctrl_id).count()
    }

    /// Raise a new notification from an already-minted snapshot (R4, D2).
    ///
    /// Returns its id, or `None` when `OverflowBehavior` refused it — queued or
    /// discarded. Which of those happened is observable through
    /// [`queued_len`](Self::queued_len) and the emitted events (R15/AC11); the
    /// stack never drops a notification without saying so.
    pub fn raise(&mut self, ctrl_id: &str, visual: SnackVisual, now: Instant) -> Option<u64> {
        let max = visual.maximum_visible.max(1);
        if self.live_for(ctrl_id) >= max {
            use cobolt_forms::snackbar::OverflowBehavior as OB;
            match visual.overflow {
                OB::Queue => {
                    self.queued.push_back((ctrl_id.to_owned(), visual));
                    return None;
                }
                OB::DiscardNewest => {
                    // The one being raised never appears. Nothing to report as
                    // Closed — it was never Shown — but the caller can see the
                    // `None` and the unchanged live count.
                    return None;
                }
                OB::DiscardOldest => {
                    // Make room by closing this control's oldest, with the
                    // reason that says why it went.
                    if let Some(pos) = self.live.iter().position(|n| n.ctrl_id == ctrl_id) {
                        let id = self.live[pos].id;
                        self.close_at(pos, DismissReason::Overflow);
                        debug_assert!(!self.live.iter().any(|n| n.id == id));
                    }
                }
            }
        }
        Some(self.push(ctrl_id, visual, now))
    }

    fn push(&mut self, ctrl_id: &str, visual: SnackVisual, now: Instant) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.live.push(LiveNotification {
            id,
            ctrl_id: ctrl_id.to_owned(),
            visual,
            raised_at: now,
            paused_total: Duration::ZERO,
            paused_since: None,
            rect: None,
        });
        self.events.push(SnackEvent::Shown { id, ctrl_id: ctrl_id.to_owned() });
        id
    }

    /// Remove the notification at `pos`, emitting `Closing` then `Closed`.
    fn close_at(&mut self, pos: usize, reason: DismissReason) {
        let n = self.live.remove(pos);
        self.events.push(SnackEvent::Closing { id: n.id, ctrl_id: n.ctrl_id.clone(), reason });
        self.events.push(SnackEvent::Closed { id: n.id, ctrl_id: n.ctrl_id, reason });
    }

    /// Dismiss one notification by id. Returns false when it was already gone.
    pub fn dismiss(&mut self, id: u64, reason: DismissReason) -> bool {
        match self.live.iter().position(|n| n.id == id) {
            Some(pos) => {
                self.close_at(pos, reason);
                true
            }
            None => false,
        }
    }

    /// `DismissAll()` — every live notification **of that control** (R9), with
    /// reason `Programmatic`. Anything that control had queued goes too: a
    /// developer who asked for silence does not want the backlog arriving next.
    pub fn dismiss_all(&mut self, ctrl_id: &str) -> usize {
        let mut n = 0;
        while let Some(pos) = self.live.iter().position(|x| x.ctrl_id == ctrl_id) {
            self.close_at(pos, DismissReason::Programmatic);
            n += 1;
        }
        self.queued.retain(|(c, _)| c != ctrl_id);
        n
    }

    /// Dispose of everything on this surface — the form is going away (Q1: a
    /// message about screen A has no meaning on screen B).
    pub fn dispose(&mut self) -> usize {
        let n = self.live.len();
        while !self.live.is_empty() {
            self.close_at(0, DismissReason::Programmatic);
        }
        self.queued.clear();
        n
    }

    /// A button was clicked. Emits `onButtonClick` and, when the button's
    /// `DismissOnClick` is set, dismisses with reason `Action` — in that order
    /// (R8/AC7): the handler must be able to read the notification it was
    /// clicked on before it goes.
    pub fn click_button(&mut self, id: u64, index: usize) -> bool {
        let Some(pos) = self.live.iter().position(|n| n.id == id) else {
            return false;
        };
        let Some(button) = self.live[pos].visual.buttons.get(index).cloned() else {
            return false;
        };
        self.events.push(SnackEvent::ButtonClick {
            id,
            ctrl_id: self.live[pos].ctrl_id.clone(),
            button_id: button.id.clone(),
            index,
        });
        if button.dismiss {
            self.close_at(pos, DismissReason::Action);
        }
        true
    }

    /// Advance the stack to `now`.
    ///
    /// `pointer` is the pointer position in the same coordinate space as the
    /// rects [`layout`](Self::layout) produced — `None` when it is off the
    /// surface. Expires whatever ran out, then promotes anything queued into
    /// the room that freed.
    pub fn tick(&mut self, now: Instant, pointer: Option<(f32, f32)>) {
        // 1. Hover pause/resume (R7), before expiry — a notification the pointer
        //    is over must not expire in the same tick that noticed the hover.
        for n in &mut self.live {
            if !n.visual.pause_on_hover {
                continue;
            }
            let hovered = match (pointer, n.rect) {
                (Some((px, py)), Some(r)) => r.contains(px.round() as i32, py.round() as i32),
                _ => false,
            };
            match (hovered, n.paused_since) {
                (true, None) => n.paused_since = Some(now),
                (false, Some(since)) => {
                    n.paused_total += now.saturating_duration_since(since);
                    n.paused_since = None;
                }
                _ => {}
            }
        }

        // 2. Expiry (R5/R6). `remaining_ms` returns None for Timeout = 0, so a
        //    zero-timeout notification is never a candidate.
        let expired: Vec<u64> = self
            .live
            .iter()
            .filter(|n| n.remaining_ms(now).map(|ms| ms <= 0).unwrap_or(false))
            .map(|n| n.id)
            .collect();
        for id in expired {
            if let Some(pos) = self.live.iter().position(|n| n.id == id) {
                let ctrl_id = self.live[pos].ctrl_id.clone();
                // §6: onTimeout fires BEFORE onClosing.
                self.events.push(SnackEvent::Timeout { id, ctrl_id });
                self.close_at(pos, DismissReason::Timeout);
            }
        }

        // 3. Promote whatever the Queue policy held back, in arrival order.
        loop {
            let Some((ctrl_id, visual)) = self.queued.front().cloned() else {
                break;
            };
            if self.live_for(&ctrl_id) >= visual.maximum_visible.max(1) {
                break;
            }
            self.queued.pop_front();
            // Raised NOW, not when Show() was called: its timeout counts from
            // the moment it became visible, which is the only reading under
            // which a queued notification is seen for its full duration.
            self.push(&ctrl_id, visual, now);
        }
    }

    /// Lay the live notifications out on `surface`, and remember where each
    /// went so [`tick`](Self::tick) can hover-test them (R14 — recomputed every
    /// frame, so a dismissal closes the gap with nothing to clean up).
    ///
    /// `measure` gives each notification's `(width, height)`; the caller
    /// supplies it because measuring needs the font, which lives with the
    /// painter. Notifications are grouped by `Anchor`, so two Snackbar controls
    /// anchored differently each get their own run rather than one interleaved
    /// column.
    pub fn layout(
        &mut self,
        surface: Rect,
        measure: &dyn Fn(&SnackVisual) -> (f32, f32),
    ) -> Vec<(u64, Rect)> {
        let mut out = Vec::with_capacity(self.live.len());
        let anchors: Vec<SnackAnchor> = {
            let mut seen: Vec<SnackAnchor> = Vec::new();
            for n in &self.live {
                if !seen.contains(&n.visual.anchor) {
                    seen.push(n.visual.anchor);
                }
            }
            seen
        };
        for anchor in anchors {
            let idxs: Vec<usize> = self
                .live
                .iter()
                .enumerate()
                .filter(|(_, n)| n.visual.anchor == anchor)
                .map(|(i, _)| i)
                .collect();
            let sizes: Vec<(f32, f32)> = idxs.iter().map(|&i| measure(&self.live[i].visual)).collect();
            // The run's spacing and order come from its NEWEST member — that is
            // the notification whose template the developer touched last, and
            // one run cannot honour two different spacings.
            let newest = *idxs.last().expect("non-empty group");
            let v = &self.live[newest].visual;
            let rects = stack_layout(anchor, v.margin, v.stack_spacing, v.stack_order, surface, &sizes);
            for (&i, r) in idxs.iter().zip(rects) {
                self.live[i].rect = Some(r);
                out.push((self.live[i].id, r));
            }
        }
        out
    }
}
