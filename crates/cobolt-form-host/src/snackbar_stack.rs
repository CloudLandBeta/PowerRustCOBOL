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
use cobolt_forms::snackbar::{
    stack_layout, DismissReason, SnackAnchor, SnackCategory, SnackVisual,
};

/// How long a **movement** or a **fade-out** runs (operator, 2026-09-01).
pub const ANIM_MS: u64 = 300;

/// How long an **entrance** runs (operator, 2026-09-01: 300 ms was too brief to
/// read as an arrival). Twice the length of the other two effects on purpose —
/// arriving is the one moment the reader is meant to notice.
pub const ENTRANCE_MS: u64 = 600;

/// What a `Critical` notification enters in instead. The most urgent category is
/// the one that should already be there when the reader looks up, so it snaps in
/// rather than easing in — the only place a category changes an effect.
pub const CRITICAL_ENTRANCE_MS: u64 = 200;

/// How long *this* notification's entrance runs.
fn entrance_ms(visual: &SnackVisual) -> u64 {
    match visual.category {
        SnackCategory::Critical => CRITICAL_ENTRANCE_MS,
        _ => ENTRANCE_MS,
    }
}

/// The scale an entrance starts from: the notification **one pixel across**
/// (operator, 2026-09-01 — 85 % of full size was barely a movement, and the
/// growth has to fill the whole 600 ms run to read as one).
///
/// Derived from the rect rather than fixed, because a single scalar is applied
/// to both sides: dividing by the longer one puts that side at exactly a pixel
/// and the shorter one under it, so it grows from a point without its aspect
/// distorting on the way up. It never zooms *out* — leaving is a fade.
fn entrance_start_scale(rect: Rect) -> f32 {
    let longest = rect.w.max(rect.h).max(1) as f32;
    (1.0 / longest).clamp(0.0, 1.0)
}

/// Decelerating ease. Motion that starts fast and settles reads as physical;
/// a linear slide reads as a scripted animation of itself.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// How far through an effect lasting `ms` `now` is, in `0.0..=1.0`.
///
/// Each effect passes its own length: the three no longer share one number, and
/// a hard-wired constant here is how a Critical entrance would silently inherit
/// somebody else's duration.
fn progress(now: Instant, since: Instant, ms: u64) -> f32 {
    let elapsed = now.saturating_duration_since(since).as_millis() as f32;
    (elapsed / ms.max(1) as f32).clamp(0.0, 1.0)
}

fn lerp_i32(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 + (b as f32 - a as f32) * t).round() as i32
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(
        lerp_i32(a.x, b.x, t),
        lerp_i32(a.y, b.y, t),
        lerp_i32(a.w, b.w, t),
        lerp_i32(a.h, b.h, t),
    )
}

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
    /// When its **entrance** begins; `None` while it is still waiting its turn.
    ///
    /// One notification arrives at a time (operator, 2026-09-01). A raise while
    /// another is still coming in waits, and when its turn comes it takes its
    /// slot *silently* first — that is what sets the ones already up gliding —
    /// and only starts entering [`ANIM_MS`] later, into the room they made. A
    /// newcomer is therefore never painted over a neighbour still in motion.
    /// With its run empty there is nothing to move and this is the raise itself.
    ///
    /// It is also the moment the timeout starts counting: a notification is
    /// given its full `Timeout` from when it becomes visible, exactly as a
    /// queued one already was.
    enters_at: Option<Instant>,
    /// Time spent paused under the pointer, already banked.
    paused_total: Duration,
    /// When the current pause began; `None` when not paused.
    paused_since: Option<Instant>,
    /// Where it was last **drawn** — the host hit-tests the pointer against
    /// this, so while it is gliding you click what you can see, not where the
    /// layout intends to put it.
    pub rect: Option<Rect>,
    /// Where the layout wants it, which is where it is gliding to.
    target: Option<Rect>,
    /// The rect it is gliding *from*, and when that glide began.
    move_from: Option<(Rect, Instant)>,
}

impl LiveNotification {
    /// How long this notification has been *counting* — wall time since it
    /// began entering, less every moment the pointer held it (R7).
    ///
    /// Not since it was *raised*: with arrivals queued one at a time, a message
    /// third in line can be raised a second before it is on screen, and a
    /// timeout that had been running all that while would give its reader less
    /// than the `Timeout` asked for — or, on a short one, expire it before it
    /// was ever drawn. It is zero until it starts entering.
    pub fn elapsed(&self, now: Instant) -> Duration {
        let Some(enters_at) = self.enters_at else {
            return Duration::ZERO;
        };
        let wall = now.saturating_duration_since(enters_at);
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

    /// How far through its entrance it is; `0.0` while it is still waiting its
    /// turn or making room.
    ///
    /// Wall time since it started entering — deliberately **not** [`elapsed`],
    /// which stops for a hover. Pausing a timeout is a promise to the reader;
    /// freezing a half-played entrance under the pointer is a glitch.
    fn entrance(&self, now: Instant) -> f32 {
        match self.enters_at {
            Some(enters_at) => progress(now, enters_at, entrance_ms(&self.visual)),
            None => 0.0,
        }
    }

    /// Has it started entering? Until it has, it holds a slot so the others can
    /// glide clear of it, but it is not painted, not hit-tested and leaves no
    /// remnant if it is closed — it was never on screen to leave one.
    fn is_visible(&self, now: Instant) -> bool {
        self.enters_at.map(|e| now >= e).unwrap_or(false)
    }

    /// Has it finished arriving? The next notification in its run waits for
    /// this.
    fn has_arrived(&self, now: Instant) -> bool {
        self.enters_at.is_some() && self.entrance(now) >= 1.0
    }

    /// Where to draw it this frame: its target, or a point along the glide
    /// towards it. Only movement is interpolated — the entrance zoom is a
    /// scale about the centre, not a change of rect, so a notification arrives
    /// AT its destination rather than flying in from somewhere.
    fn drawn_rect(&self, now: Instant) -> Option<Rect> {
        let target = self.target?;
        match self.move_from {
            Some((from, since)) => {
                let t = progress(now, since, ANIM_MS);
                if t >= 1.0 {
                    Some(target)
                } else {
                    Some(lerp_rect(from, target, ease_out(t)))
                }
            }
            None => Some(target),
        }
    }
}

/// A notification that has already closed, kept only so it can fade out.
///
/// It is **not live**: `onClosing`/`onClosed` have already fired, it counts
/// towards no `MaximumVisible`, it is laid out no more and it cannot be
/// clicked. All that survives is 300 ms of fade where it last stood.
///
/// Keeping it out of `live` is the whole trick. Leaving it in — so it could
/// "finish leaving" before removal — would stall overflow for the length of its
/// own animation and defer the events a handler is waiting on. A dismissal is
/// instantaneous in every respect except the picture.
#[derive(Debug, Clone)]
pub struct FadingNotification {
    pub id: u64,
    pub visual: SnackVisual,
    /// Frozen where it was last drawn: survivors close the gap around it.
    pub rect: Rect,
    started: Instant,
}

/// One thing to paint this frame, live or fading.
#[derive(Debug, Clone, Copy)]
pub struct SnackDraw<'a> {
    pub id: u64,
    pub visual: &'a SnackVisual,
    pub rect: Rect,
    /// Scale about the rect's centre. Below `1.0` only during an entrance.
    pub scale: f32,
    /// `1.0` is opaque.
    pub alpha: f32,
    /// False for a fading remnant — it is already closed, so its buttons must
    /// not be hit-tested.
    pub interactive: bool,
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
    /// Closed, but still fading out. Not live in any sense that counts.
    fading: Vec<FadingNotification>,
    /// The last instant the stack was told about.
    ///
    /// `dismiss`, `click_button` and `DismissAll()` are answers to something the
    /// operator just did, and none of them carries a clock. A fade has to start
    /// somewhere, so it starts from the most recent tick — at worst one frame
    /// stale, since the host ticks before it draws.
    last_tick: Option<Instant>,
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

    /// Nothing to show and nothing pending — including no fade still running,
    /// because the host skips the whole draw when this is true and a remnant
    /// would blink out instead of fading.
    pub fn is_empty(&self) -> bool {
        self.live.is_empty() && self.queued.is_empty() && self.fading.is_empty()
    }

    /// The remnants still fading out.
    pub fn fading(&self) -> &[FadingNotification] {
        &self.fading
    }

    /// Is any effect in flight — an entrance, a glide, a fade, or a notification
    /// still queued behind one of them?
    ///
    /// The host paces its repaints off this. A stack merely waiting for a
    /// timeout needs a frame occasionally; one mid-animation needs them at
    /// screen rate, or an effect is drawn half a dozen times and steps instead
    /// of moving. A notification waiting its turn counts too: its turn comes
    /// during a frame, so the frames have to keep arriving.
    pub fn is_animating(&self, now: Instant) -> bool {
        !self.fading.is_empty()
            || self.live.iter().any(|n| {
                n.enters_at.is_none()
                    || n.entrance(now) < 1.0
                    || n.move_from
                        .map(|(_, since)| progress(now, since, ANIM_MS) < 1.0)
                        .unwrap_or(false)
            })
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
            // It arrives when its run is free, which [`admit`](Self::admit)
            // decides — the same tick when nothing else is coming in.
            enters_at: None,
            paused_total: Duration::ZERO,
            paused_since: None,
            rect: None,
            target: None,
            move_from: None,
        });
        self.events.push(SnackEvent::Shown { id, ctrl_id: ctrl_id.to_owned() });
        id
    }

    /// Remove the notification at `pos`, emitting `Closing` then `Closed`.
    ///
    /// Both events fire **now**, and it stops being live **now** — a handler
    /// waiting on `onClosed` is not made to wait out an animation, and the slot
    /// it freed is available to the next notification immediately. What lingers
    /// is a picture: if it had been drawn, it leaves a remnant that fades over
    /// 300 ms where it stood ([`FadingNotification`]).
    fn close_at(&mut self, pos: usize, reason: DismissReason) {
        let n = self.live.remove(pos);
        // A notification closed before its entrance began was never drawn, so
        // there is nothing to fade — a remnant of it would be the reader's
        // first and only sight of a message that is already gone.
        let now = self.last_tick.unwrap_or(n.raised_at);
        if let Some(rect) = n.rect.filter(|_| n.is_visible(now)) {
            self.fading.push(FadingNotification {
                id: n.id,
                visual: n.visual.clone(),
                rect,
                started: self.last_tick.unwrap_or(n.raised_at),
            });
        }
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
        // The surface itself is going: there is nothing left to fade against.
        self.fading.clear();
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

    /// Let the next notification in each run start arriving — **one at a time**
    /// (operator, 2026-09-01).
    ///
    /// Raising three in a handler used to put three entrances on screen at once,
    /// all playing over each other. They queue instead: a notification is
    /// admitted only once every older one in its run has finished entering.
    ///
    /// Admission is not the entrance. It is the moment the newcomer joins the
    /// layout, which is what gives the ones already up a new destination and
    /// sets them gliding. Its own entrance starts [`ANIM_MS`] later — once that
    /// room has actually been made — so it is never drawn on top of a neighbour
    /// still moving. With the run empty there is nothing to move out of the way
    /// and it enters immediately.
    ///
    /// A **run** is one anchor's column: two Snackbars anchored to opposite
    /// corners are separate stacks that never overlap, and making one wait for
    /// the other would be a queue nobody could see the reason for.
    ///
    /// Called from both [`tick`](Self::tick) and [`layout`](Self::layout), so
    /// admission does not depend on which the caller reaches first.
    fn admit(&mut self, now: Instant) {
        let mut anchors: Vec<SnackAnchor> = Vec::new();
        for n in &self.live {
            if n.enters_at.is_none() && !anchors.contains(&n.visual.anchor) {
                anchors.push(n.visual.anchor);
            }
        }
        for anchor in anchors {
            let idxs: Vec<usize> = self
                .live
                .iter()
                .enumerate()
                .filter(|(_, n)| n.visual.anchor == anchor)
                .map(|(i, _)| i)
                .collect();
            // `live` is in raise order, so this walks the run oldest first.
            let mut occupied = 0usize;
            for i in idxs {
                if self.live[i].enters_at.is_some() {
                    // Already on its way in — and if it has not landed yet,
                    // nobody behind it may start.
                    occupied += 1;
                    if !self.live[i].has_arrived(now) {
                        break;
                    }
                    continue;
                }
                // Its turn. Anything already up has to glide clear first.
                let room = if occupied == 0 { 0 } else { ANIM_MS };
                self.live[i].enters_at = Some(now + Duration::from_millis(room));
                break;
            }
        }
    }

    /// Advance the stack to `now`.
    ///
    /// `pointer` is the pointer position in the same coordinate space as the
    /// rects [`layout`](Self::layout) produced — `None` when it is off the
    /// surface. Expires whatever ran out, then promotes anything queued into
    /// the room that freed.
    pub fn tick(&mut self, now: Instant, pointer: Option<(f32, f32)>) {
        self.last_tick = Some(now);
        self.admit(now);

        // 0. Reap remnants whose fade has finished. They are already closed —
        //    this only stops drawing them.
        self.fading.retain(|f| progress(now, f.started, ANIM_MS) < 1.0);

        // 1. Hover pause/resume (R7), before expiry — a notification the pointer
        //    is over must not expire in the same tick that noticed the hover.
        for n in &mut self.live {
            if !n.visual.pause_on_hover {
                continue;
            }
            // One holding its slot while the others glide clear is not on
            // screen yet, so the pointer cannot be over it however much its
            // rect says otherwise.
            let hovered = match (pointer, n.rect) {
                (Some((px, py)), Some(r)) if n.is_visible(now) => {
                    r.contains(px.round() as i32, py.round() as i32)
                }
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

    /// Everything to paint this frame, in paint order: fading remnants first so
    /// a survivor gliding into the gap passes over the ghost, then the live
    /// ones oldest-first (the newest lands on top).
    ///
    /// The three effects (operator, 2026-09-01):
    ///
    /// * **entrance**, [`ENTRANCE_MS`] — the notification whose turn it is grows
    ///   from [`entrance_start_scale`] — one pixel — to full size and fades in,
    ///   *at its destination*, taking the whole run to get there. Older
    ///   ones are long past their own entrance window, which is what makes "only
    ///   one is ever entering" true without anyone tracking who is newest. A
    ///   `Critical` one takes [`CRITICAL_ENTRANCE_MS`] instead.
    /// * **movement**, [`ANIM_MS`] — handled in [`layout`](Self::layout): a
    ///   changed destination is glided to, never jumped to.
    /// * **exit**, [`ANIM_MS`] — a closed notification fades where it stood. It
    ///   does **not** zoom out.
    ///
    /// One admitted but still waiting out that movement is emitted here at
    /// alpha zero and **not** interactive: it is holding its slot so the others
    /// can glide clear, and a button nobody can see must not be clickable.
    pub fn to_draw(&self, now: Instant) -> Vec<SnackDraw<'_>> {
        let mut out = Vec::with_capacity(self.fading.len() + self.live.len());
        for f in &self.fading {
            out.push(SnackDraw {
                id: f.id,
                visual: &f.visual,
                rect: f.rect,
                // Leaving is a fade, never a zoom out.
                scale: 1.0,
                // Linear, deliberately. `ease_out` is right for an arrival —
                // it decelerates into place — but running it backwards on a
                // departure throws away 87% of the opacity in the first half
                // and then crawls, which reads as a blink rather than a fade.
                // A notification leaving should dim evenly.
                alpha: 1.0 - progress(now, f.started, ANIM_MS),
                interactive: false,
            });
        }
        for n in &self.live {
            let Some(rect) = n.rect.or(n.target) else { continue };
            let t = ease_out(n.entrance(now));
            let start = entrance_start_scale(rect);
            out.push(SnackDraw {
                id: n.id,
                visual: &n.visual,
                rect,
                scale: start + (1.0 - start) * t,
                alpha: t,
                interactive: n.is_visible(now),
            });
        }
        out
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
    ///
    /// Only **admitted** notifications are laid out. One still waiting its turn
    /// has no place in the stack yet — giving it one would open a gap for a
    /// message that is not coming for another half second.
    pub fn layout(
        &mut self,
        surface: Rect,
        measure: &dyn Fn(&SnackVisual) -> (f32, f32),
        now: Instant,
    ) -> Vec<(u64, Rect)> {
        self.admit(now);
        let mut out = Vec::with_capacity(self.live.len());
        let anchors: Vec<SnackAnchor> = {
            let mut seen: Vec<SnackAnchor> = Vec::new();
            for n in &self.live {
                if n.enters_at.is_some() && !seen.contains(&n.visual.anchor) {
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
                .filter(|(_, n)| n.visual.anchor == anchor && n.enters_at.is_some())
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
                let n = &mut self.live[i];
                // A destination that changed starts a glide from wherever it is
                // being drawn right now — which is what makes a dismissal close
                // the gap smoothly instead of snapping the survivors up. A
                // notification seeing its target for the first time has nowhere
                // to glide from and simply appears there.
                if n.target != Some(r) {
                    n.move_from = n.rect.map(|from| (from, now));
                    n.target = Some(r);
                }
                let shown = n.drawn_rect(now).unwrap_or(r);
                n.rect = Some(shown);
                out.push((n.id, shown));
            }
        }
        out
    }
}
