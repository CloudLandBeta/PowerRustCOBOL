// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T7/T11 — the live stack, driven by a fabricated clock.
//!
//! **Nothing here sleeps.** `tick` takes `now`, so a full lifetime — raise,
//! hover for two seconds, resume, expire — is played out in microseconds and
//! lands on the same numbers every run. Every test reports the milliseconds it
//! actually measured rather than asserting silently (GOLDEN RULE #7).

use std::time::{Duration, Instant};

use cobolt_forms::model::{Control, ControlType, PropValue, Rect};
use cobolt_forms::snackbar::{mint, DismissReason, SnackVisual};
use cobolt_form_host::snackbar_stack::{
    SnackEvent, SnackbarStack, ANIM_MS, CRITICAL_ENTRANCE_MS, ENTRANCE_MS,
};

/// The longest a single arrival can take: the glide that makes room for it,
/// then its own entrance.
const ARRIVAL_MS: u64 = ANIM_MS + ENTRANCE_MS;

/// A template with the given property overrides, already minted.
fn visual(set: &[(&str, PropValue)]) -> SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    for (k, v) in set {
        c.set_prop(*k, v.clone());
    }
    mint(&c).0
}

const SURFACE: Rect = Rect { x: 0, y: 0, w: 1000, h: 700 };

/// Every notification is 300x56 — the arithmetic under test is the stack's, not
/// the text measurer's.
fn fixed_size(_: &SnackVisual) -> (f32, f32) {
    (300.0, 56.0)
}

fn at(base: Instant, ms: u64) -> Instant {
    base + Duration::from_millis(ms)
}

/// Raise `n` notifications and let each one land before the next starts.
///
/// The stack admits **one arrival at a time** (operator, 2026-09-01), so a test
/// that wants three up can no longer raise three in the same instant and lay
/// them out — the second and third would still be waiting their turn. Returns
/// the ids and the instant by which all of them are up and still.
fn raise_settled(
    s: &mut SnackbarStack,
    ctrl: &str,
    v: impl Fn() -> SnackVisual,
    t0: Instant,
    n: usize,
) -> (Vec<u64>, Instant) {
    let mut ids = Vec::new();
    let mut t = t0;
    for _ in 0..n {
        ids.push(s.raise(ctrl, v(), t).expect("raised"));
        // The frame that admits it, then one after its arrival has played out.
        s.layout(SURFACE, &fixed_size, t);
        t = at(t, ARRIVAL_MS);
        s.layout(SURFACE, &fixed_size, t);
    }
    (ids, t)
}

#[test]
fn a_notification_expires_on_its_own_timeout() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    // Info's own default: 4000 ms (§7), reached through the `-1` sentinel.
    let v = visual(&[]);
    assert_eq!(v.timeout_ms, 4000, "Info's category timeout");
    let id = s.raise("SNACK-1", v, t0).expect("raised");

    eprintln!("\n  t(ms)   live   remaining(ms)   note");
    eprintln!("  -----   ----   -------------   ----");
    for ms in [0u64, 1000, 3999] {
        s.tick(at(t0, ms), None);
        let rem = s.live()[0].remaining_ms(at(t0, ms)).unwrap();
        eprintln!("  {ms:>5}   {:>4}   {rem:>13}   up", s.live().len());
        assert_eq!(s.live().len(), 1, "still up at {ms}ms");
    }
    s.tick(at(t0, 4000), None);
    eprintln!("  {:>5}   {:>4}   {:>13}   expired", 4000, s.live().len(), "-");
    assert!(s.live().is_empty(), "AC6: gone at its timeout");

    // §6 — onTimeout fires BEFORE onClosing, and onClosed last.
    let evs = s.drain_events();
    let kinds: Vec<String> = evs.iter().map(describe).collect();
    assert_eq!(
        kinds,
        vec![
            format!("Shown({id})"),
            format!("Timeout({id})"),
            format!("Closing({id},Timeout)"),
            format!("Closed({id},Timeout)"),
        ],
        "event order"
    );
    eprintln!("  → AC6/R5: expired at exactly 4000 ms; events {kinds:?}\n");
}

fn describe(e: &SnackEvent) -> String {
    match e {
        SnackEvent::Shown { id, .. } => format!("Shown({id})"),
        SnackEvent::Timeout { id, .. } => format!("Timeout({id})"),
        SnackEvent::Closing { id, reason, .. } => format!("Closing({id},{})", reason.as_str()),
        SnackEvent::Closed { id, reason, .. } => format!("Closed({id},{})", reason.as_str()),
        SnackEvent::ButtonClick { id, button_id, index, .. } => {
            format!("ButtonClick({id},{button_id},{index})")
        }
    }
}

#[test]
fn timeout_zero_never_expires() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    // Critical's category default IS 0 — it stays until dismissed (§7).
    let v = visual(&[("Category", PropValue::String("Critical".into()))]);
    assert_eq!(v.timeout_ms, 0, "Critical stays");
    s.raise("SNACK-1", v, t0).expect("raised");

    for hours in [1u64, 24, 24 * 30] {
        s.tick(at(t0, hours * 3_600_000), None);
        assert_eq!(s.live().len(), 1, "R6: still up after {hours}h");
        assert!(s.live()[0].remaining_ms(at(t0, hours * 3_600_000)).is_none());
    }
    // An explicit Timeout = 0 behaves the same way (R6), on any category.
    let mut s2 = SnackbarStack::new();
    s2.raise("SNACK-1", visual(&[("Timeout", PropValue::Int(0))]), t0).expect("raised");
    s2.tick(at(t0, 86_400_000), None);
    assert_eq!(s2.live().len(), 1, "R6: an explicit 0 never expires either");
    eprintln!("\n  R6 — Critical and an explicit Timeout=0 both still up after 30 days / 24 h\n");
}

#[test]
fn hover_holds_the_timeout_and_leaving_resumes_it() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s.raise("SNACK-1", visual(&[("Timeout", PropValue::Int(3000))]), t0).expect("raised");
    // Lay it out so there is a rect to hover.
    let rects = s.layout(SURFACE, &fixed_size, t0);
    let (_, r) = rects[0];
    let inside = ((r.x + r.w / 2) as f32, (r.y + r.h / 2) as f32);
    let outside = (5.0, 5.0);

    eprintln!("\n  t(ms)   pointer    paused   remaining(ms)");
    eprintln!("  -----   --------   ------   -------------");
    let mut report = |s: &SnackbarStack, ms: u64, what: &str| {
        let n = &s.live()[0];
        let rem = n.remaining_ms(at(t0, ms)).unwrap();
        eprintln!("  {ms:>5}   {what:<8}   {:<6}   {rem:>13}", n.is_paused());
        rem
    };

    s.tick(at(t0, 1000), None);
    let r1 = report(&s, 1000, "off");
    assert_eq!(r1, 2000, "1 s consumed of 3 s");

    // Pointer arrives at 1000 and stays until 3000 — two seconds held.
    s.tick(at(t0, 1000), Some(inside));
    s.tick(at(t0, 2000), Some(inside));
    let r2 = report(&s, 2000, "over");
    assert_eq!(r2, 2000, "R7: the timeout did not advance while hovered");

    s.tick(at(t0, 3000), Some(inside));
    let r3 = report(&s, 3000, "over");
    assert_eq!(r3, 2000, "R7: still held after 2 s of hover");
    assert!(s.live()[0].is_paused());

    // It leaves at 3000; the remaining 2 s now run from there.
    s.tick(at(t0, 3000), Some(outside));
    let r4 = report(&s, 3000, "off");
    assert_eq!(r4, 2000, "resumed with exactly what was left");
    assert!(!s.live()[0].is_paused());

    s.tick(at(t0, 4000), Some(outside));
    let r5 = report(&s, 4000, "off");
    assert_eq!(r5, 1000, "counting again");
    assert_eq!(s.live().len(), 1, "not yet expired");

    s.tick(at(t0, 5000), Some(outside));
    eprintln!("  {:>5}   {:<8}   {:<6}   {:>13}", 5000, "off", "-", "expired");
    assert!(s.live().is_empty(), "expires 2 s after the pointer left, not 2 s after it arrived");

    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert!(evs.contains(&format!("Closed({id},Timeout)")));
    eprintln!("  → R7: 2000 ms held across the hover; total life 5000 ms for a 3000 ms timeout\n");
}

#[test]
fn pause_on_hover_off_means_the_pointer_is_ignored() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    s.raise(
        "SNACK-1",
        visual(&[("Timeout", PropValue::Int(2000)), ("PauseTimeoutOnHover", PropValue::Bool(false))]),
        t0,
    )
    .expect("raised");
    let rects = s.layout(SURFACE, &fixed_size, t0);
    let (_, r) = rects[0];
    let inside = ((r.x + r.w / 2) as f32, (r.y + r.h / 2) as f32);
    s.tick(at(t0, 1000), Some(inside));
    assert!(!s.live()[0].is_paused(), "PauseTimeoutOnHover=false must not pause");
    s.tick(at(t0, 2000), Some(inside));
    assert!(s.live().is_empty(), "expired on schedule despite the pointer sitting on it");
    eprintln!("\n  PauseTimeoutOnHover=false — expired at 2000 ms with the pointer on it\n");
}

#[test]
fn dismissing_the_middle_of_three_closes_the_gap() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    // One arrival at a time, so each has to land before the next is raised.
    let (ids, t) = raise_settled(&mut s, "SNACK-1", v, t0, 3);
    let (a, b, c) = (ids[0], ids[1], ids[2]);

    let before = s.layout(SURFACE, &fixed_size, t);
    eprintln!("\n  before — {:?}", before.iter().map(|(i, r)| (*i, r.y)).collect::<Vec<_>>());
    assert_eq!(before.len(), 3, "all three are up before the dismissal");

    assert!(s.dismiss(b, DismissReason::User), "the middle one goes");

    // The survivors GLIDE into the gap now (operator, 2026-09-01) rather than
    // snapping into it, so at the instant of the dismissal they have not moved
    // yet. The hole is closed by the layout, not by the first frame after it.
    let instant = s.layout(SURFACE, &fixed_size, t);
    let moved_at_once = instant.iter().map(|(_, r)| r.y).collect::<Vec<_>>();

    let after = s.layout(SURFACE, &fixed_size, at(t, ANIM_MS));
    eprintln!("  after  — {:?}", after.iter().map(|(i, r)| (*i, r.y)).collect::<Vec<_>>());
    assert_ne!(
        moved_at_once,
        after.iter().map(|(_, r)| r.y).collect::<Vec<_>>(),
        "the reflow must be animated, not instantaneous"
    );

    assert_eq!(after.len(), 2);
    let ys: Vec<i32> = after.iter().map(|(_, r)| r.y).collect();
    let gap = (ys[1] - ys[0]).abs() - 56;
    assert_eq!(gap, 8, "AC5/R14: once settled, survivors are exactly StackSpacing apart — no hole left behind");
    assert!(after.iter().any(|(i, _)| *i == a) && after.iter().any(|(i, _)| *i == c));

    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert!(evs.contains(&format!("Closed({b},User)")), "reported verbatim: {evs:?}");
    eprintln!("  → AC5: middle dismissed, gap {gap} pt (= StackSpacing 8), 2 survivors reflowed over {ANIM_MS} ms\n");
}

#[test]
fn overflow_applies_the_configured_policy_and_says_what_it_did() {
    let t0 = Instant::now();
    eprintln!("\n  policy          raised   live ids     queued   dropped");
    eprintln!("  -------------   ------   ----------   ------   -------");

    // Queue — the surplus waits, and arrives when room frees.
    {
        let mut s = SnackbarStack::new();
        let v = || visual(&[("Timeout", PropValue::Int(0)), ("MaximumVisible", PropValue::Int(2))]);
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(s.raise("SNACK-1", v(), at(t0, i * 10)));
        }
        let live: Vec<u64> = s.live().iter().map(|n| n.id).collect();
        eprintln!("  {:<13}   {:>6}   {:<10}   {:>6}   {}", "Queue", 4, format!("{live:?}"), s.queued_len(), "none");
        assert_eq!(live, vec![1, 2], "only MaximumVisible are up");
        assert_eq!(s.queued_len(), 2, "R15: the surplus is HELD, not dropped");
        assert_eq!(ids[2], None, "a queued raise reports no id");

        // Free a slot; the queue drains in arrival order.
        s.dismiss(1, DismissReason::User);
        s.tick(at(t0, 100), None);
        let live2: Vec<u64> = s.live().iter().map(|n| n.id).collect();
        assert_eq!(live2, vec![2, 3], "the oldest queued one arrived");
        assert_eq!(s.queued_len(), 1);
        // Its timeout counts from when it BECAME VISIBLE, not from Show().
        assert_eq!(s.live()[1].raised_at, at(t0, 100));
    }

    // DiscardOldest — room is made, and the victim is reported as Overflow.
    {
        let mut s = SnackbarStack::new();
        let v = || visual(&[
            ("Timeout", PropValue::Int(0)),
            ("MaximumVisible", PropValue::Int(2)),
            ("OverflowBehavior", PropValue::String("DiscardOldest".into())),
        ]);
        for i in 0..4 {
            s.raise("SNACK-1", v(), at(t0, i * 10));
        }
        let live: Vec<u64> = s.live().iter().map(|n| n.id).collect();
        let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
        let dropped: Vec<&String> = evs.iter().filter(|e| e.contains("Overflow")).collect();
        eprintln!(
            "  {:<13}   {:>6}   {:<10}   {:>6}   {}",
            "DiscardOldest", 4, format!("{live:?}"), s.queued_len(),
            dropped.iter().filter(|e| e.starts_with("Closed")).count()
        );
        assert_eq!(live, vec![3, 4], "the newest two survive");
        assert_eq!(s.queued_len(), 0);
        // AC11 — what was dropped is OBSERVABLE, by id and by reason.
        assert!(evs.contains(&"Closed(1,Overflow)".to_string()), "{evs:?}");
        assert!(evs.contains(&"Closed(2,Overflow)".to_string()), "{evs:?}");
    }

    // DiscardNewest — the arrival never appears at all.
    {
        let mut s = SnackbarStack::new();
        let v = || visual(&[
            ("Timeout", PropValue::Int(0)),
            ("MaximumVisible", PropValue::Int(2)),
            ("OverflowBehavior", PropValue::String("DiscardNewest".into())),
        ]);
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(s.raise("SNACK-1", v(), at(t0, i * 10)));
        }
        let live: Vec<u64> = s.live().iter().map(|n| n.id).collect();
        eprintln!(
            "  {:<13}   {:>6}   {:<10}   {:>6}   {}",
            "DiscardNewest", 4, format!("{live:?}"), s.queued_len(), 2
        );
        assert_eq!(live, vec![1, 2], "the first two hold their place");
        assert_eq!(s.queued_len(), 0, "nothing is held back");
        assert_eq!(ids[2], None);
        assert_eq!(ids[3], None, "the caller sees the refusal");
        // Nothing was ever Shown for the refused pair, so no Closed either —
        // the absence IS the report, and the `None` return says so at the call.
        let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
        assert_eq!(evs.iter().filter(|e| e.starts_with("Shown")).count(), 2);
    }
    eprintln!("  → AC11/R15: 3 policies, ids that survived and what was dropped both reported\n");
}

#[test]
fn a_button_with_dismiss_on_click_raises_then_dismisses() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s
        .raise(
            "SNACK-1",
            visual(&[
                ("Timeout", PropValue::Int(0)),
                ("Buttons", PropValue::String("retry|Retry|refresh|Left|true\nlater|Later||None|false".into())),
            ]),
            t0,
        )
        .expect("raised");

    // Button 1 keeps it up (DismissOnClick = false).
    assert!(s.click_button(id, 1));
    assert_eq!(s.live().len(), 1, "R8: dismiss=false leaves it up");
    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert_eq!(evs, vec![format!("Shown({id})"), format!("ButtonClick({id},later,1)")]);

    // Button 0 dismisses — and the CLICK is reported before the close (AC7).
    assert!(s.click_button(id, 0));
    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert_eq!(
        evs,
        vec![
            format!("ButtonClick({id},retry,0)"),
            format!("Closing({id},Action)"),
            format!("Closed({id},Action)"),
        ],
        "AC7: onButtonClick THEN the dismissal, with reason Action"
    );
    assert!(s.live().is_empty());
    eprintln!("\n  AC7/R8 — dismiss=false kept it up; dismiss=true fired ButtonClick then Closed(Action)\n");
}

#[test]
fn dismiss_all_clears_only_that_controls_notifications() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    s.raise("SNACK-A", v(), t0);
    s.raise("SNACK-A", v(), at(t0, 10));
    s.raise("SNACK-B", v(), at(t0, 20));
    assert_eq!(s.live().len(), 3);

    let n = s.dismiss_all("SNACK-A");
    assert_eq!(n, 2, "R9: both of A's");
    assert_eq!(s.live().len(), 1, "B's is untouched");
    assert_eq!(s.live()[0].ctrl_id, "SNACK-B");

    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert!(evs.contains(&"Closed(1,Programmatic)".to_string()), "{evs:?}");
    assert!(evs.contains(&"Closed(2,Programmatic)".to_string()), "{evs:?}");
    assert!(!evs.iter().any(|e| e.starts_with("Closed(3")), "B's must not close");
    eprintln!("\n  R9 — DismissAll on SNACK-A closed 2 with reason Programmatic; SNACK-B's 1 untouched\n");
}

#[test]
fn every_dismiss_reason_is_reported_verbatim() {
    let t0 = Instant::now();
    eprintln!("\n  reason         how it was produced");
    eprintln!("  ------------   -------------------");
    let mut seen: Vec<&'static str> = Vec::new();

    // Timeout
    let mut s = SnackbarStack::new();
    s.raise("S", visual(&[("Timeout", PropValue::Int(100))]), t0);
    // The first frame after the raise is where it is admitted, and a timeout
    // counts from there — the host ticks every frame, so that is the next one.
    s.tick(t0, None);
    s.tick(at(t0, 100), None);
    assert!(s.drain_events().iter().any(|e| matches!(e, SnackEvent::Closed { reason: DismissReason::Timeout, .. })));
    seen.push("Timeout");
    eprintln!("  {:<12}   {}", "Timeout", "tick past the timeout");

    // User
    let mut s = SnackbarStack::new();
    let id = s.raise("S", visual(&[("Timeout", PropValue::Int(0))]), t0).unwrap();
    s.dismiss(id, DismissReason::User);
    assert!(s.drain_events().iter().any(|e| matches!(e, SnackEvent::Closed { reason: DismissReason::User, .. })));
    seen.push("User");
    eprintln!("  {:<12}   {}", "User", "dismiss(id, User)");

    // Action
    let mut s = SnackbarStack::new();
    let id = s
        .raise("S", visual(&[("Timeout", PropValue::Int(0)), ("Buttons", PropValue::String("ok|OK".into()))]), t0)
        .unwrap();
    s.click_button(id, 0);
    assert!(s.drain_events().iter().any(|e| matches!(e, SnackEvent::Closed { reason: DismissReason::Action, .. })));
    seen.push("Action");
    eprintln!("  {:<12}   {}", "Action", "a DismissOnClick button");

    // Programmatic
    let mut s = SnackbarStack::new();
    s.raise("S", visual(&[("Timeout", PropValue::Int(0))]), t0);
    s.dismiss_all("S");
    assert!(s.drain_events().iter().any(|e| matches!(e, SnackEvent::Closed { reason: DismissReason::Programmatic, .. })));
    seen.push("Programmatic");
    eprintln!("  {:<12}   {}", "Programmatic", "DismissAll()");

    // Overflow
    let mut s = SnackbarStack::new();
    let v = || visual(&[
        ("Timeout", PropValue::Int(0)),
        ("MaximumVisible", PropValue::Int(1)),
        ("OverflowBehavior", PropValue::String("DiscardOldest".into())),
    ]);
    s.raise("S", v(), t0);
    s.raise("S", v(), at(t0, 10));
    assert!(s.drain_events().iter().any(|e| matches!(e, SnackEvent::Closed { reason: DismissReason::Overflow, .. })));
    seen.push("Overflow");
    eprintln!("  {:<12}   {}", "Overflow", "DiscardOldest making room");

    assert_eq!(seen.len(), 5, "all five reasons");
    eprintln!("  → 5/5 dismiss reasons produced and reported verbatim\n");
}

#[test]
fn two_controls_with_different_anchors_get_their_own_runs() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let bottom = visual(&[("Timeout", PropValue::Int(0)), ("StackAnchor", PropValue::String("BottomRight".into()))]);
    let top = visual(&[("Timeout", PropValue::Int(0)), ("StackAnchor", PropValue::String("TopLeft".into()))]);
    // The arrival queue is per RUN: two anchors are two stacks that cannot
    // overlap, so neither waits for the other and both of these come in at once.
    s.raise("SNACK-BOTTOM", bottom.clone(), t0);
    s.raise("SNACK-TOP", top.clone(), at(t0, 10));
    s.layout(SURFACE, &fixed_size, at(t0, 10));

    // The BottomRight run's second one does wait for the first one to land.
    let t = at(t0, 10 + ARRIVAL_MS);
    s.raise("SNACK-BOTTOM", bottom, t);
    s.layout(SURFACE, &fixed_size, t);
    let t = at(t, ARRIVAL_MS);

    let rects = s.layout(SURFACE, &fixed_size, t);
    let by_id: std::collections::HashMap<u64, Rect> = rects.iter().copied().collect();
    // The two BottomRight ones stack together at the bottom right; the TopLeft
    // one is on its own at the top left — not interleaved into one column.
    assert!(by_id[&1].y > 500 && by_id[&3].y > 500, "the BottomRight pair is at the bottom");
    assert_eq!(by_id[&2].y, 16, "the TopLeft one sits on its own margin");
    assert_eq!(by_id[&2].x, 16);
    assert_eq!(by_id[&1].x, 1000 - 16 - 300, "the BottomRight run hugs the right");
    // And the pair is spaced from each other, not from the unrelated one.
    assert_eq!((by_id[&3].y - by_id[&1].y).abs(), 56 + 8);
    eprintln!(
        "\n  two anchors — BottomRight ids 1,3 at y {} / {}; TopLeft id 2 at ({}, {})\n",
        by_id[&1].y, by_id[&3].y, by_id[&2].x, by_id[&2].y
    );
}

// ── Entrance, movement and exit (operator, 2026-09-01) ───────────────────────
//
// The Snackbar shipped without animation: notifications appeared, jumped and
// vanished. Every effect below runs for exactly ANIM_MS, and every assertion
// here drives a fabricated clock — nothing sleeps, so a whole animation plays
// out in microseconds and always reaches the same answer.

/// The scale and alpha the stack wants for one notification this frame.
fn effect(s: &SnackbarStack, id: u64, now: Instant) -> (f32, f32) {
    s.to_draw(now)
        .into_iter()
        .find(|d| d.id == id)
        .map(|d| (d.scale, d.alpha))
        .unwrap_or_else(|| panic!("nothing to draw for {id}"))
}

fn y_of(id: u64, rects: &[(u64, Rect)]) -> i32 {
    rects.iter().find(|(i, _)| *i == id).expect("laid out").1.y
}

#[test]
fn a_lone_raise_zooms_and_fades_in_over_600ms() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s
        .raise("SNACK-1", visual(&[("Timeout", PropValue::Int(0))]), t0)
        .expect("raised");
    // Nothing is up, so nothing has to glide clear: it enters straight away.
    s.layout(SURFACE, &fixed_size, t0);

    let (s0, a0) = effect(&s, id, t0);
    let (s1, a1) = effect(&s, id, at(t0, ENTRANCE_MS / 2));
    let (s2, a2) = effect(&s, id, at(t0, ENTRANCE_MS));

    eprintln!("\n  t(ms)   scale   alpha");
    eprintln!("  -----   -----   -----");
    for (ms, sc, al) in [(0, s0, a0), (ENTRANCE_MS / 2, s1, a1), (ENTRANCE_MS, s2, a2)] {
        eprintln!("  {ms:>5}   {sc:>5.3}   {al:>5.3}");
    }

    assert!(s0 < 1.0 && a0 < 0.01, "it starts under size and invisible");
    assert!(s1 > s0 && a1 > a0, "it is part-way in at half time");
    assert!(s1 < 1.0 && a1 < 1.0, "and not finished at half time");
    // Still going at what used to be the whole effect: the entrance was
    // doubled, and the other two effects were not.
    let (_, a_old) = effect(&s, id, at(t0, ANIM_MS));
    assert!(a_old < 1.0, "still arriving at {ANIM_MS} ms — the old duration");
    assert!(
        (s2 - 1.0).abs() < 1e-6 && (a2 - 1.0).abs() < 1e-6,
        "full size and opaque at exactly {ENTRANCE_MS} ms: scale {s2}, alpha {a2}"
    );
    eprintln!("  → zoom + fade in, {a_old:.3} opaque at {ANIM_MS} ms, complete at {ENTRANCE_MS} ms\n");
}

#[test]
fn the_entrance_grows_from_one_pixel_to_full_size() {
    // Operator, 2026-09-01: it must "grow all the way 1 pixel to final size
    // within the 600 ms run". 85 % of full size was barely a movement.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s
        .raise("SNACK-1", visual(&[("Timeout", PropValue::Int(0))]), t0)
        .expect("raised");
    let rects = s.layout(SURFACE, &fixed_size, t0);
    let full = rects[0].1;

    eprintln!("\n  t(ms)   scale    drawn w x h (full {} x {})", full.w, full.h);
    eprintln!("  -----   ------   -------------------------");
    let mut seen: Vec<(u64, f32, f32, f32)> = Vec::new();
    for ms in [0, ENTRANCE_MS / 4, ENTRANCE_MS / 2, ENTRANCE_MS * 3 / 4, ENTRANCE_MS] {
        let (sc, _) = effect(&s, id, at(t0, ms));
        let (w, h) = (full.w as f32 * sc, full.h as f32 * sc);
        eprintln!("  {ms:>5}   {sc:>6.4}   {w:>8.2} x {h:.2}");
        seen.push((ms, sc, w, h));
    }

    // One pixel across the longer side at the start — not 85 %, not zero.
    let (_, s0, w0, _) = seen[0];
    assert!(
        (w0 - 1.0).abs() < 0.01,
        "it must start ONE pixel across the longer side, drew {w0} pt (scale {s0})"
    );
    // Growing the whole way, every quarter of the run.
    for pair in seen.windows(2) {
        assert!(
            pair[1].1 > pair[0].1,
            "still growing at {} ms: {} → {}",
            pair[1].0,
            pair[0].1,
            pair[1].1
        );
    }
    // And exactly full size at the end, not before.
    let (_, s_last, w_last, h_last) = *seen.last().unwrap();
    assert!(
        (s_last - 1.0).abs() < 1e-6,
        "full size at exactly {ENTRANCE_MS} ms, got scale {s_last}"
    );
    assert!(seen[3].1 < 1.0, "not already full size three quarters through");
    eprintln!(
        "  → 1.00 pt → {w_last:.2} x {h_last:.2} pt across {ENTRANCE_MS} ms, growing throughout\n"
    );
}

#[test]
fn a_critical_notification_enters_faster_than_the_rest() {
    // The one place a Category changes an effect (operator, 2026-09-01): the
    // most urgent message should already be there when the reader looks up.
    let t0 = Instant::now();
    eprintln!("\n  category     entrance(ms)   alpha @200ms   alpha @600ms");
    eprintln!("  ----------   ------------   ------------   ------------");
    let mut measured: Vec<(&str, u64)> = Vec::new();
    for (name, want) in [
        ("Critical", CRITICAL_ENTRANCE_MS),
        ("Info", ENTRANCE_MS),
        ("Warning", ENTRANCE_MS),
        ("Error", ENTRANCE_MS),
        ("Question", ENTRANCE_MS),
    ] {
        let mut s = SnackbarStack::new();
        let id = s
            .raise(
                "SNACK-1",
                visual(&[
                    ("Category", PropValue::String(name.into())),
                    ("Timeout", PropValue::Int(0)),
                ]),
                t0,
            )
            .expect("raised");
        s.layout(SURFACE, &fixed_size, t0);

        // Walk the clock forward until it is fully opaque; that IS its
        // duration, to within the millisecond where the cubic ease saturates
        // an f32 a hair early.
        let mut ms = 0u64;
        while ms <= ARRIVAL_MS && effect(&s, id, at(t0, ms)).1 < 1.0 {
            ms += 1;
        }
        eprintln!(
            "  {name:<10}   {ms:>12}   {:>12.3}   {:>12.3}",
            effect(&s, id, at(t0, CRITICAL_ENTRANCE_MS)).1,
            effect(&s, id, at(t0, ENTRANCE_MS)).1
        );
        assert!(ms.abs_diff(want) <= 1, "{name} must take {want} ms to arrive, took {ms}");
        assert!(
            effect(&s, id, at(t0, want / 2)).1 < 1.0,
            "{name} must still be arriving at half of {want} ms"
        );
        assert!(
            (effect(&s, id, at(t0, want)).1 - 1.0).abs() < 1e-6,
            "{name} must be fully in at exactly {want} ms"
        );
        measured.push((name, want));
    }
    assert_eq!(measured[0].1, CRITICAL_ENTRANCE_MS);
    assert!(
        measured[1..].iter().all(|(_, ms)| *ms == ENTRANCE_MS),
        "only Critical is quicker: {measured:?}"
    );
    eprintln!("  → 5/5 categories measured; Critical {CRITICAL_ENTRANCE_MS} ms, the other four {ENTRANCE_MS} ms\n");
}

#[test]
fn only_the_newest_zooms_in() {
    // The older one is long past its own entrance window, so nothing has to
    // track which is newest — the effect is per notification, from its raise.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    let first = s.raise("SNACK-1", v(), t0).unwrap();
    s.layout(SURFACE, &fixed_size, t0);

    // Raised the instant the first one has landed, so it is admitted at once.
    let later = at(t0, ENTRANCE_MS);
    let second = s.raise("SNACK-1", v(), later).unwrap();
    s.layout(SURFACE, &fixed_size, later);

    let (fs, fa) = effect(&s, first, later);
    let (ss, sa) = effect(&s, second, later);
    assert!(
        (fs - 1.0).abs() < 1e-6 && (fa - 1.0).abs() < 1e-6,
        "the settled one must not re-zoom when a newer arrives: scale {fs}, alpha {fa}"
    );
    assert!(ss < 1.0 && sa < 0.01, "the newest is mid-entrance: scale {ss}, alpha {sa}");
    // And it is STILL invisible for the whole glide that makes room for it —
    // that is what stops it being painted over a neighbour still moving.
    let (_, mid_room) = effect(&s, second, at(later, ANIM_MS / 2));
    assert!(mid_room < 0.01, "still holding its slot unseen: alpha {mid_room}");
    let (_, entered) = effect(&s, second, at(later, ANIM_MS + ENTRANCE_MS));
    assert!((entered - 1.0).abs() < 1e-6, "in by {ANIM_MS} + {ENTRANCE_MS} ms: alpha {entered}");
    eprintln!("\n  → only the newest zooms: settled {fs:.3}, newest {ss:.3} → {entered:.3}\n");
}

#[test]
fn a_second_raise_moves_the_first_one_progressively() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || {
        visual(&[
            ("Timeout", PropValue::Int(0)),
            ("StackAnchor", PropValue::String("BottomRight".into())),
        ])
    };
    let first = s.raise("SNACK-1", v(), t0).unwrap();
    s.layout(SURFACE, &fixed_size, t0);
    let landed = at(t0, ENTRANCE_MS);
    let start = y_of(first, &s.layout(SURFACE, &fixed_size, landed));

    // A second one takes the slot, so the first has somewhere to go. It glides
    // during the newcomer's room-making window, which is ANIM_MS long.
    s.raise("SNACK-1", v(), landed).unwrap();
    let at_once = y_of(first, &s.layout(SURFACE, &fixed_size, landed));
    let midway = y_of(first, &s.layout(SURFACE, &fixed_size, at(landed, ANIM_MS / 2)));
    let settled = y_of(first, &s.layout(SURFACE, &fixed_size, at(landed, ANIM_MS)));

    eprintln!("\n  first notification y — start {start}, at once {at_once}, midway {midway}, settled {settled}");
    assert_ne!(start, settled, "the arrival of a second one must move the first");
    assert_eq!(at_once, start, "it must not jump the instant the second arrives");
    assert!(
        (midway - start).abs() > 0 && (midway - settled).abs() > 0,
        "midway {midway} must be between {start} and {settled}, not at either end"
    );
    let lo = start.min(settled);
    let hi = start.max(settled);
    assert!(midway > lo && midway < hi, "midway {midway} must lie inside {lo}..{hi}");
    eprintln!("  → the reflow is glided, not jumped\n");
}

#[test]
fn three_raised_at_once_arrive_one_at_a_time() {
    // Request 1 (operator, 2026-09-01): raising three in a handler used to put
    // three entrances on screen together, all playing over each other. They
    // queue — a notification starts entering only once the one before it is in.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    let ids: Vec<u64> = (0..3).map(|_| s.raise("SNACK-1", v(), t0).expect("raised")).collect();
    assert_eq!(s.live().len(), 3, "all three are live immediately — only the picture queues");

    // Sample every frame and record, for each, what it was doing.
    let mut first_visible: Vec<Option<u64>> = vec![None; 3];
    let mut arrived: Vec<Option<u64>> = vec![None; 3];
    let mut most_entering_at_once = 0usize;
    for ms in (0..=3 * ARRIVAL_MS).step_by(16) {
        let now = at(t0, ms);
        s.tick(now, None);
        s.layout(SURFACE, &fixed_size, now);
        let mut entering = 0;
        for (k, id) in ids.iter().enumerate() {
            let Some(d) = s.to_draw(now).into_iter().find(|d| d.id == *id) else {
                continue;
            };
            if d.alpha > 0.0 {
                first_visible[k].get_or_insert(ms);
                if d.alpha < 1.0 {
                    entering += 1;
                } else {
                    arrived[k].get_or_insert(ms);
                }
            }
        }
        most_entering_at_once = most_entering_at_once.max(entering);
    }

    eprintln!("\n  #   first visible (ms)   fully in (ms)");
    eprintln!("  -   ------------------   -------------");
    for k in 0..3 {
        eprintln!(
            "  {}   {:>18}   {:>13}",
            k + 1,
            first_visible[k].expect("every one of them arrives"),
            arrived[k].expect("and lands")
        );
    }
    assert_eq!(most_entering_at_once, 1, "at most ONE may be entering in any frame");
    for k in 1..3 {
        assert!(
            first_visible[k].unwrap() >= arrived[k - 1].unwrap(),
            "#{} started entering at {} ms, before #{} had landed at {} ms",
            k + 1,
            first_visible[k].unwrap(),
            k,
            arrived[k - 1].unwrap()
        );
    }
    eprintln!(
        "  → 3 raised in the same instant, {most_entering_at_once} entering at a time, all in by {} ms\n",
        arrived[2].unwrap()
    );
}

#[test]
fn an_arriving_notification_never_overlaps_one_still_moving() {
    // Request 2 (operator, 2026-09-01): "as a new enters, the existing glides to
    // create room to the newcomer (there should be no overlaps)". The newcomer
    // takes its slot INVISIBLY first — which is what sets the others gliding —
    // and only starts to appear once that room has actually been made.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    for _ in 0..3 {
        s.raise("SNACK-1", v(), t0).expect("raised");
    }

    let mut frames = 0usize;
    let mut worst: Option<(u64, u64, u64)> = None; // ms, id, id
    let mut moving_while_visible = 0usize;
    for ms in (0..=3 * ARRIVAL_MS).step_by(8) {
        let now = at(t0, ms);
        s.tick(now, None);
        s.layout(SURFACE, &fixed_size, now);
        // Only what the reader can actually see can overlap for the reader.
        let seen: Vec<_> = s.to_draw(now).into_iter().filter(|d| d.alpha > 0.0).collect();
        frames += 1;
        for (i, a) in seen.iter().enumerate() {
            for b in &seen[i + 1..] {
                let (ra, rb) = (drawn_rect(a), drawn_rect(b));
                if ra.0 < rb.1 && rb.0 < ra.1 {
                    worst.get_or_insert((ms, a.id, b.id));
                    moving_while_visible += 1;
                }
            }
        }
    }

    eprintln!("\n  frames sampled   overlapping pairs   first overlap");
    eprintln!("  --------------   -----------------   -------------");
    eprintln!(
        "  {frames:>14}   {moving_while_visible:>17}   {}",
        worst.map(|(ms, a, b)| format!("{ms} ms, #{a}/#{b}")).unwrap_or_else(|| "none".into())
    );
    assert_eq!(
        moving_while_visible, 0,
        "two visible notifications overlapped: {worst:?}"
    );
    eprintln!("  → {frames} frames across three arrivals, no visible pair ever overlapped\n");
}

/// A draw's vertical extent, scaled about its centre the way the host paints it.
fn drawn_rect(d: &cobolt_form_host::snackbar_stack::SnackDraw<'_>) -> (f32, f32) {
    let cy = d.rect.y as f32 + d.rect.h as f32 / 2.0;
    let half = d.rect.h as f32 * d.scale / 2.0;
    (cy - half, cy + half)
}

#[test]
fn a_timeout_fade_changes_alpha_but_never_scale() {
    // Leaving is a fade. It is explicitly NOT a zoom out (operator).
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s
        .raise("SNACK-1", visual(&[("Timeout", PropValue::Int(1000))]), t0)
        .expect("raised");
    s.layout(SURFACE, &fixed_size, t0);

    let expiry = at(t0, 1000);
    s.tick(expiry, None);
    assert!(s.live().is_empty(), "it expired");
    assert_eq!(s.fading().len(), 1, "and left a remnant to fade");

    let (s0, a0) = effect(&s, id, expiry);
    let (s1, a1) = effect(&s, id, at(t0, 1000 + ANIM_MS / 2));
    let (s2, a2) = effect(&s, id, at(t0, 1000 + ANIM_MS));

    eprintln!("\n  fading — t+0 alpha {a0:.3}, t+{} alpha {a1:.3}, t+{ANIM_MS} alpha {a2:.3}", ANIM_MS / 2);
    for (label, sc) in [("start", s0), ("mid", s1), ("end", s2)] {
        assert!((sc - 1.0).abs() < 1e-6, "{label}: scale must stay 1.0, got {sc}");
    }
    assert!(a0 > a1 && a1 > a2, "alpha must fall: {a0} > {a1} > {a2}");
    // Evenly, too: an exit that dumps most of its opacity in the first half
    // reads as a blink. Half the time is half the fade, within rounding.
    assert!(
        (a1 - 0.5).abs() < 0.05,
        "the fade must be even — expected ~0.5 half-way, got {a1}"
    );
    assert!(a2.abs() < 1e-6, "fully transparent at exactly {ANIM_MS} ms, got {a2}");

    // And the remnant is reaped once its fade is done, not before.
    s.tick(at(t0, 1000 + ANIM_MS - 1), None);
    assert_eq!(s.fading().len(), 1, "still fading a millisecond short of the end");
    s.tick(at(t0, 1000 + ANIM_MS), None);
    assert!(s.fading().is_empty(), "reaped at {ANIM_MS} ms");
    eprintln!("  → fade out with no zoom, reaped at {ANIM_MS} ms\n");
}

#[test]
fn a_fading_remnant_is_neither_clickable_nor_counted() {
    // The whole difficulty in one test. A remnant must not vanish instantly —
    // but it must stop occupying a slot the moment it closes, or `MaximumVisible`
    // would stall for the length of an animation.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || {
        visual(&[
            ("Timeout", PropValue::Int(0)),
            ("MaximumVisible", PropValue::Int(1)),
        ])
    };
    let first = s.raise("SNACK-1", v(), t0).expect("raised");
    s.layout(SURFACE, &fixed_size, t0);
    assert!(s.dismiss(first, DismissReason::User), "dismissed");

    // Still being drawn...
    assert_eq!(s.fading().len(), 1, "the remnant is still on screen");
    let drawn = s.to_draw(t0);
    assert_eq!(drawn.len(), 1);
    assert!(!drawn[0].interactive, "a closed notification must not be clickable");
    assert!(!s.is_empty(), "the host must keep drawing while a fade runs");

    // ...and yet the slot is free immediately.
    let second = s.raise("SNACK-1", v(), t0);
    assert!(
        second.is_some(),
        "MaximumVisible must not be held by something that has already closed"
    );
    assert_eq!(s.live().len(), 1, "the newcomer is up");
    eprintln!("\n  → remnant still painted, slot already free\n");
}

#[test]
fn the_stack_reports_when_an_effect_is_in_flight() {
    // The host paces repaints off this: at 50 ms a 600 ms effect would be drawn
    // a dozen times and step instead of moving.
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let id = s
        .raise("SNACK-1", visual(&[("Timeout", PropValue::Int(0))]), t0)
        .expect("raised");
    s.layout(SURFACE, &fixed_size, t0);

    assert!(s.is_animating(t0), "an entrance is an effect in flight");
    assert!(s.is_animating(at(t0, ANIM_MS)), "and still is at {ANIM_MS} ms");
    let landed = at(t0, ENTRANCE_MS);
    assert!(
        !s.is_animating(landed),
        "a settled, idle stack must not ask for screen-rate repaints"
    );

    // One waiting its turn counts too — its turn comes during a frame, so the
    // frames have to keep coming.
    s.raise("SNACK-1", visual(&[("Timeout", PropValue::Int(0))]), landed);
    assert!(s.is_animating(landed), "a notification waiting its turn is an effect pending");

    // A dismissal starts a fade, so painting must resume...
    s.tick(landed, None);
    s.dismiss(id, DismissReason::User);
    assert!(s.is_animating(landed), "a fade is an effect in flight");

    // ...and stop once every remnant has been reaped and the newcomer is in.
    let quiet = at(landed, ARRIVAL_MS);
    s.tick(quiet, None);
    s.layout(SURFACE, &fixed_size, quiet);
    assert!(s.fading().is_empty(), "reaped");
    assert!(
        !s.is_animating(quiet),
        "nothing left to animate once the remnant is gone and the newcomer is in"
    );
    eprintln!("\n  → screen-rate only while an effect is running or pending\n");
}
