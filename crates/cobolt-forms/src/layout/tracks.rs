// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Grid track lists** (spec 056 R55): the text of `GridColumns` and
//! `GridRows`.
//!
//! Tokens, separated by spaces, any case: `Npx` (a bare number is pixels),
//! `N%`, `Nfr`, `Auto`, `MinMax(a, b)`, `Repeat(N, tracks…)` and
//! `Repeat(AutoFill, tracks…)` — as many repetitions as fit. A token the
//! parser does not understand is skipped, never an error: a half-typed value
//! in the Properties pane must not blank the form.

use crate::layout::defaults;

/// One end of a track's size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Breadth {
    Px(f32),
    /// Of the container's size on that axis.
    Percent(f32),
    /// A share of what the other tracks leave.
    Fr(f32),
    /// The largest size among the items that sit only in this track.
    Auto,
}

/// A track: its minimum and maximum size. A plain `Nfr` is
/// `MinMax(Auto, Nfr)`, as in CSS — a flexible track never crushes its
/// content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub min: Breadth,
    pub max: Breadth,
}

impl Track {
    fn of(b: Breadth) -> Track {
        match b {
            Breadth::Fr(_) => Track { min: Breadth::Auto, max: b },
            _ => Track { min: b, max: b },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Entry {
    Track(Track),
    /// `Repeat(AutoFill, …)`: repeated as often as fits.
    AutoFill(Vec<Track>),
}

/// A parsed track list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackList {
    entries: Vec<Entry>,
}

impl TrackList {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The tracks for a container `avail` long on this axis (`None` when it
    /// sizes to its content), with `gap` between tracks: an `AutoFill`
    /// repetition becomes as many copies as fit, at least one.
    pub fn expand(&self, avail: Option<f32>, gap: f32) -> Vec<Track> {
        let fixed = |t: &Track, avail: Option<f32>| -> Option<f32> {
            let one = |b: Breadth| match b {
                Breadth::Px(v) => Some(v),
                Breadth::Percent(p) => avail.map(|a| p / defaults::PERCENT * a),
                _ => None,
            };
            one(t.max).or_else(|| one(t.min))
        };
        let mut out = Vec::new();
        let others: Vec<&Track> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Track(t) => Some(t),
                Entry::AutoFill(_) => None,
            })
            .collect();
        for e in &self.entries {
            match e {
                Entry::Track(t) => out.push(*t),
                Entry::AutoFill(rep) => {
                    let mut count = 1usize;
                    if let Some(a) = avail {
                        let other_size: f32 = others.iter().map(|t| fixed(t, avail).unwrap_or_default()).sum();
                        let rep_size: f32 = rep.iter().map(|t| fixed(t, avail).unwrap_or_default()).sum();
                        if rep_size > 0.0 {
                            let fits = |k: usize| {
                                let tracks = others.len() + k * rep.len();
                                other_size + k as f32 * rep_size + tracks.saturating_sub(1) as f32 * gap
                                    <= a + defaults::EPSILON
                            };
                            while fits(count + 1) {
                                count += 1;
                            }
                        }
                    }
                    for _ in 0..count {
                        out.extend(rep.iter().copied());
                    }
                }
            }
        }
        out
    }
}

/// Parse a track list.
pub fn parse(text: &str) -> TrackList {
    TrackList { entries: parse_entries(text) }
}

fn parse_entries(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for tok in split_top(text, char::is_whitespace) {
        let lower = tok.to_ascii_lowercase();
        if let Some(args) = call(&lower, "repeat") {
            let parts = split_top(args, |c| c == ',');
            let Some((count, rest)) = parts.split_first() else { continue };
            let inner: Vec<Track> = rest
                .iter()
                .flat_map(|p| parse_entries(p))
                .filter_map(|e| match e {
                    Entry::Track(t) => Some(t),
                    Entry::AutoFill(_) => None,
                })
                .collect();
            if inner.is_empty() {
                continue;
            }
            let count = count.trim().replace(['-', '_'], "");
            if count == "autofill" {
                out.push(Entry::AutoFill(inner));
            } else if let Ok(n) = count.parse::<usize>() {
                for _ in 0..n {
                    out.extend(inner.iter().map(|t| Entry::Track(*t)));
                }
            }
        } else if let Some(args) = call(&lower, "minmax") {
            let parts = split_top(args, |c| c == ',');
            if let [a, b] = parts.as_slice() {
                if let (Some(min), Some(max)) = (breadth(a), breadth(b)) {
                    // A flexible minimum is meaningless (CSS): it reads as Auto.
                    let min = if matches!(min, Breadth::Fr(_)) { Breadth::Auto } else { min };
                    out.push(Entry::Track(Track { min, max }));
                }
            }
        } else if let Some(b) = breadth(&lower) {
            out.push(Entry::Track(Track::of(b)));
        }
    }
    out
}

/// `name(args)` → `args`.
fn call<'a>(tok: &'a str, name: &str) -> Option<&'a str> {
    tok.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

fn breadth(t: &str) -> Option<Breadth> {
    let t = t.trim().to_ascii_lowercase();
    if t == "auto" {
        return Some(Breadth::Auto);
    }
    let num = |s: &str| s.trim().parse::<f32>().ok().filter(|v| *v >= 0.0);
    if let Some(v) = t.strip_suffix("fr") {
        return num(v).map(Breadth::Fr);
    }
    if let Some(v) = t.strip_suffix('%') {
        return num(v).map(Breadth::Percent);
    }
    num(t.strip_suffix("px").unwrap_or(&t)).map(Breadth::Px)
}

/// Split at `sep` characters outside parentheses; empty pieces dropped.
fn split_top(text: &str, sep: impl Fn(char) -> bool) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 && sep(ch) => {
                out.push(&text[start..i]);
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out.into_iter().map(str::trim).filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(v: f32) -> Track {
        Track { min: Breadth::Px(v), max: Breadth::Px(v) }
    }
    fn fr(v: f32) -> Track {
        Track { min: Breadth::Auto, max: Breadth::Fr(v) }
    }

    #[test]
    fn tokens_parse_in_any_case_and_bad_ones_are_skipped() {
        let t = parse("200px 1fr 2FR  auto 25% MinMax(100px, 1fr) 40 nonsense Repeat(2, 50px)");
        assert_eq!(
            t.expand(Some(1000.0), 0.0),
            vec![
                px(200.0),
                fr(1.0),
                fr(2.0),
                Track { min: Breadth::Auto, max: Breadth::Auto },
                Track { min: Breadth::Percent(25.0), max: Breadth::Percent(25.0) },
                Track { min: Breadth::Px(100.0), max: Breadth::Fr(1.0) },
                px(40.0),
                px(50.0),
                px(50.0),
            ]
        );
        assert!(parse("").is_empty());
    }

    /// AC25 — `Repeat(AutoFill, MinMax(160px, 1fr))`: as many columns of at
    /// least 160 as fit. 200 → 1; 500 → 3 (480 ≤ 500 < 640); 820 → 5
    /// (800 ≤ 820). With a 10 px gap, 500 fits 2·160 + 10 = 330 and
    /// 3·160 + 20 = 500 → 3.
    #[test]
    fn auto_fill_repeats_as_often_as_fits() {
        let t = parse("Repeat(AutoFill, MinMax(160px, 1fr))");
        for (w, gap, n) in [(200.0, 0.0, 1), (500.0, 0.0, 3), (820.0, 0.0, 5), (500.0, 10.0, 3), (100.0, 0.0, 1)] {
            assert_eq!(t.expand(Some(w), gap).len(), n, "{w} px, gap {gap}");
        }
        assert_eq!(parse("repeat(auto-fill, minmax(160px, 1fr))").expand(Some(820.0), 0.0).len(), 5);
        println!("AutoFill MinMax(160px, 1fr): 200 → 1, 500 → 3, 820 → 5 columns");
    }
}
