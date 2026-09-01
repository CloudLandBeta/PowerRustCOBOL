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
use cobolt_form_host::snackbar_stack::{SnackEvent, SnackbarStack};

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
    let rects = s.layout(SURFACE, &fixed_size);
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
    let rects = s.layout(SURFACE, &fixed_size);
    let (_, r) = rects[0];
    let inside = ((r.x + r.w / 2) as f32, (r.y + r.h / 2) as f32);
    s.tick(at(t0, 1000), Some(inside));
    assert!(!s.live()[0].is_paused(), "PauseTimeoutOnHover=false must not pause");
    s.tick(at(t0, 2000), Some(inside));
    assert!(s.live().is_empty(), "expired on schedule despite the pointer sitting on it");
    eprintln!("\n  PauseTimeoutOnHover=false — expired at 2000 ms with the pointer on it\n");
}

#[test]
fn dismissing_the_middle_of_three_closes_the_gap_immediately() {
    let t0 = Instant::now();
    let mut s = SnackbarStack::new();
    let v = || visual(&[("Timeout", PropValue::Int(0))]);
    let a = s.raise("SNACK-1", v(), t0).unwrap();
    let b = s.raise("SNACK-1", v(), at(t0, 10)).unwrap();
    let c = s.raise("SNACK-1", v(), at(t0, 20)).unwrap();

    let before = s.layout(SURFACE, &fixed_size);
    eprintln!("\n  before — {:?}", before.iter().map(|(i, r)| (*i, r.y)).collect::<Vec<_>>());

    assert!(s.dismiss(b, DismissReason::User), "the middle one goes");
    let after = s.layout(SURFACE, &fixed_size);
    eprintln!("  after  — {:?}", after.iter().map(|(i, r)| (*i, r.y)).collect::<Vec<_>>());

    assert_eq!(after.len(), 2);
    let ys: Vec<i32> = after.iter().map(|(_, r)| r.y).collect();
    let gap = (ys[1] - ys[0]).abs() - 56;
    assert_eq!(gap, 8, "AC5/R14: survivors exactly StackSpacing apart — no hole left behind");
    assert!(after.iter().any(|(i, _)| *i == a) && after.iter().any(|(i, _)| *i == c));

    let evs: Vec<String> = s.drain_events().iter().map(describe).collect();
    assert!(evs.contains(&format!("Closed({b},User)")), "reported verbatim: {evs:?}");
    eprintln!("  → AC5: middle dismissed, gap {gap} pt (= StackSpacing 8), 2 survivors reflowed\n");
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
    s.raise("SNACK-BOTTOM", bottom.clone(), t0);
    s.raise("SNACK-TOP", top.clone(), at(t0, 10));
    s.raise("SNACK-BOTTOM", bottom, at(t0, 20));

    let rects = s.layout(SURFACE, &fixed_size);
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
