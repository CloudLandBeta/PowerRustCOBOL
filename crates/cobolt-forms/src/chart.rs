// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A chart's **pure** parts: its data wire format, and the value tween.
//!
//! Live data reaches a chart as the `__ChartData` property — one
//! `label<TAB>value` per line — pushed by `AddPoint` / `Clear` / the
//! `COBOL-CHART-*` calls. Both the painter and the animating renderer read it,
//! so the parse lives here once rather than being written out twice with a
//! chance to disagree.
//!
//! There is **no clock and no state** here, for the reason [`crate::snackbar`]
//! has none: a tween's lifetime belongs to whoever owns cross-frame state, and
//! the arithmetic has to be drivable from a fabricated `t` in a test.

use crate::model::Control;

/// How long a value animation runs when nothing says otherwise (operator,
/// 2026-09-02).
pub const DEFAULT_ANIM_MS: i64 = 2000;

/// The shortest it may run. Below this a "tween" is a flicker: the eye reads a
/// jump, and the property would be honoured in name only.
pub const MIN_ANIM_MS: i64 = 250;

/// One point of a series.
pub type Point = (String, f32);

/// Parse the `__ChartData` wire format: one `label<TAB>value` per line.
///
/// A line without a tab, or whose value is not a number, is **skipped** rather
/// than defaulted to zero — a zero would be plotted, and a bar that is not
/// there is a better answer than a bar that is wrong.
pub fn parse_chart_data(raw: &str) -> Vec<Point> {
    raw.lines()
        .filter_map(|ln| {
            let mut it = ln.splitn(2, '\t');
            let label = it.next()?.to_owned();
            let value: f32 = it.next()?.trim().parse().ok()?;
            Some((label, value))
        })
        .collect()
}

/// Render a series back to the `__ChartData` wire format.
pub fn format_chart_data(points: &[Point]) -> String {
    points
        .iter()
        .map(|(l, v)| format!("{}\t{}", l.replace(['\t', '\n'], " "), v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Is this chart set to animate its values, and for how long?
///
/// `None` when the animation is off. The duration is clamped up to
/// [`MIN_ANIM_MS`]: the property is the developer's to set, but a number that
/// would produce a jump instead of a movement is not honoured as written
/// (operator, 2026-09-02 — "always >= 250ms").
pub fn value_anim_ms(ctrl: &Control) -> Option<i64> {
    let on = ctrl
        .get_prop("AnimateValues")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    if !on {
        return None;
    }
    let ms = ctrl
        .get_prop("AnimationDuration")
        .map(|v| v.as_i64())
        .unwrap_or(DEFAULT_ANIM_MS);
    Some(ms.max(MIN_ANIM_MS))
}

/// Decelerating ease, the same shape a notification's entrance uses: motion
/// that starts fast and settles reads as physical.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The series to draw `t` of the way from `from` to `to`, `t` in `0.0..=1.0`.
///
/// **The labels are `to`'s**, always: the new data decides what the chart is
/// about, and a half-played tween must never leave a point captioned with the
/// name it used to have.
///
/// Values are matched **by position**, which is what makes a series that grew
/// or shrank still animate:
///
/// * a point with a predecessor moves from that predecessor's value;
/// * a point with none — the series got longer — **grows from zero**, so it
///   rises into place rather than appearing at full height;
/// * points that were dropped simply stop being drawn, since the frame only
///   ever has as many points as `to`.
///
/// A value that is not finite is left at its target rather than interpolated:
/// there is no sensible half-way to a NaN, and one would poison the auto-scale
/// for every other point on the chart.
pub fn tween_series(from: &[Point], to: &[Point], t: f32) -> Vec<Point> {
    let k = ease_out(t);
    to.iter()
        .enumerate()
        .map(|(i, (label, target))| {
            if !target.is_finite() {
                return (label.clone(), *target);
            }
            let start = from
                .get(i)
                .map(|(_, v)| *v)
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            (label.clone(), start + (target - start) * k)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(v: &[(&str, f32)]) -> Vec<Point> {
        v.iter().map(|(l, x)| ((*l).to_owned(), *x)).collect()
    }

    #[test]
    fn the_wire_format_round_trips_and_skips_what_it_cannot_read() {
        let raw = "Q1\t128\nQ2\t174.5\nbroken line\nQ3\tnot-a-number\nQ4\t209";
        let p = parse_chart_data(raw);
        eprintln!("\n  line                  parsed");
        eprintln!("  -------------------   ------");
        for ln in raw.lines() {
            let got = parse_chart_data(ln);
            eprintln!("  {:<19}   {}", ln, if got.is_empty() { "skipped" } else { "yes" });
        }
        assert_eq!(p.len(), 3, "3 of 5 lines are readable: {p:?}");
        assert_eq!(p[0], ("Q1".to_owned(), 128.0));
        assert_eq!(p[1], ("Q2".to_owned(), 174.5));
        assert_eq!(p[2], ("Q4".to_owned(), 209.0));
        // And back out again, unchanged for the lines that survived.
        assert_eq!(parse_chart_data(&format_chart_data(&p)), p);
        eprintln!("  → 3/5 lines parsed, round trip exact\n");
    }

    #[test]
    fn a_tween_starts_at_the_old_values_and_lands_on_the_new() {
        let from = pts(&[("Q1", 100.0), ("Q2", 200.0)]);
        let to = pts(&[("Q1", 300.0), ("Q2", 100.0)]);
        eprintln!("\n     t     Q1        Q2");
        eprintln!("  ----   ------   -------");
        let mut last = tween_series(&from, &to, 0.0);
        for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let f = tween_series(&from, &to, t);
            eprintln!("  {t:>4.2}   {:>6.1}   {:>7.1}", f[0].1, f[1].1);
            if t > 0.0 {
                assert!(f[0].1 > last[0].1, "Q1 must rise throughout, stalled at {t}");
                assert!(f[1].1 < last[1].1, "Q2 must fall throughout, stalled at {t}");
            }
            last = f;
        }
        let start = tween_series(&from, &to, 0.0);
        let end = tween_series(&from, &to, 1.0);
        assert_eq!((start[0].1, start[1].1), (100.0, 200.0), "t=0 is the OLD set");
        assert_eq!((end[0].1, end[1].1), (300.0, 100.0), "t=1 is the NEW set");
        eprintln!("  → 100→300 and 200→100, monotonic, exact at both ends\n");
    }

    #[test]
    fn labels_are_the_new_ones_and_a_new_point_grows_from_zero() {
        let from = pts(&[("Jan", 40.0)]);
        let to = pts(&[("Feb", 80.0), ("Mar", 60.0)]);
        let half = tween_series(&from, &to, 0.5);
        eprintln!("\n  half-way: {half:?}");
        assert_eq!(half[0].0, "Feb", "a half-played tween must not show the OLD label");
        assert_eq!(half[1].0, "Mar");
        assert!(
            half[0].1 > 40.0 && half[0].1 < 80.0,
            "the surviving point moves from 40 towards 80, got {}",
            half[0].1
        );
        assert!(
            half[1].1 > 0.0 && half[1].1 < 60.0,
            "the NEW point grows from 0 towards 60, got {}",
            half[1].1
        );
        // And a series that shrank simply draws fewer points.
        let shrunk = tween_series(&to, &from, 0.5);
        assert_eq!(shrunk.len(), 1, "the frame has as many points as the TARGET");
        eprintln!("  → labels follow the new data; a new point rises from 0\n");
    }

    #[test]
    fn the_duration_is_the_developers_but_never_below_the_floor() {
        use crate::model::{ControlType, PropValue};
        let mk = |on: bool, ms: Option<i64>| {
            let mut c = Control::new("CHART-1", ControlType::BarChart, 0, 0);
            c.set_prop("AnimateValues", PropValue::Bool(on));
            if let Some(ms) = ms {
                c.set_prop("AnimationDuration", PropValue::Int(ms));
            }
            c
        };
        eprintln!("\n  AnimateValues   AnimationDuration   effective");
        eprintln!("  -------------   -----------------   ---------");
        let cases: [(bool, Option<i64>, Option<i64>); 6] = [
            (false, None, None),
            (false, Some(5000), None),
            (true, None, Some(DEFAULT_ANIM_MS)),
            (true, Some(5000), Some(5000)),
            (true, Some(250), Some(250)),
            (true, Some(10), Some(MIN_ANIM_MS)),
        ];
        for (on, set, want) in cases {
            let got = value_anim_ms(&mk(on, set));
            eprintln!(
                "  {:<13}   {:<17}   {}",
                on,
                set.map(|v| v.to_string()).unwrap_or_else(|| "(default)".into()),
                got.map(|v| v.to_string()).unwrap_or_else(|| "off".into())
            );
            assert_eq!(got, want, "AnimateValues={on}, AnimationDuration={set:?}");
        }
        eprintln!("  → 6/6: off means off, the default is {DEFAULT_ANIM_MS} ms, the floor is {MIN_ANIM_MS} ms\n");
    }
}
