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

    /// Whether the list holds a `Repeat(AutoFill, …)`, whose track count
    /// depends on the container's size.
    pub fn auto_fills(&self) -> bool {
        self.entries.iter().any(|e| matches!(e, Entry::AutoFill(_)))
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

/// What a token of a track list is, for an editor (spec 088).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenKind {
    /// One ordinary track: `Npx`, `N`, `N%`, `Nfr`, `Auto`.
    Plain(Track),
    /// One `MinMax(a, b)` track: it is a track, but it cannot be dragged.
    MinMax,
    /// `Repeat(N, …)` or `Repeat(AutoFill, …)`: any number of tracks, which an
    /// edit leaves alone.
    Repeat,
    /// Something the parser skips — no track, no error.
    Unreadable,
}

/// The top-level tokens of a track list exactly as written (a `MinMax(…)` or
/// `Repeat(…)` is one token), so an editor can replace, insert or remove one and
/// leave every other — an unreadable one included — as the developer typed it.
pub fn split(text: &str) -> Vec<String> {
    split_top(text, char::is_whitespace).into_iter().map(str::to_owned).collect()
}

/// Tokens back into a list: one space between them.
pub fn join(tokens: &[String]) -> String {
    tokens.join(" ")
}

/// What one token is, by the parser's own rules: a token the parser would skip
/// is [`TokenKind::Unreadable`].
pub fn kind_of(token: &str) -> TokenKind {
    let lower = token.trim().to_ascii_lowercase();
    if let Some(args) = call(&lower, "repeat") {
        let parts = split_top(args, |c| c == ',');
        let Some((count, rest)) = parts.split_first() else { return TokenKind::Unreadable };
        let has_tracks = rest.iter().flat_map(|p| parse_entries(p)).any(|e| matches!(e, Entry::Track(_)));
        let count = count.trim().replace(['-', '_'], "");
        return if has_tracks && (count == "autofill" || count.parse::<usize>().is_ok()) {
            TokenKind::Repeat
        } else {
            TokenKind::Unreadable
        };
    }
    if let Some(args) = call(&lower, "minmax") {
        let parts = split_top(args, |c| c == ',');
        return match parts.as_slice() {
            [a, b] if breadth(a).is_some() && breadth(b).is_some() => TokenKind::MinMax,
            _ => TokenKind::Unreadable,
        };
    }
    match breadth(&lower) {
        Some(b) => TokenKind::Plain(Track::of(b)),
        None => TokenKind::Unreadable,
    }
}

/// Whether any token of the list is a `Repeat(…)`.
pub fn has_repeat(text: &str) -> bool {
    split(text).iter().any(|t| kind_of(t) == TokenKind::Repeat)
}

/// A track list as an editor sees it: the tokens, and which token holds each
/// track.
#[derive(Clone, Debug, PartialEq)]
pub struct Slots {
    pub tokens: Vec<String>,
    /// For each explicit track (a `Plain` or `MinMax` token), in order, the index
    /// of its token. An unreadable token holds none. Meaningless past a `Repeat`
    /// — check [`Slots::has_repeat`] first.
    pub track_to_token: Vec<usize>,
    pub has_repeat: bool,
}

pub fn slots(text: &str) -> Slots {
    let tokens = split(text);
    let kinds: Vec<TokenKind> = tokens.iter().map(|t| kind_of(t)).collect();
    Slots {
        track_to_token: kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| matches!(k, TokenKind::Plain(_) | TokenKind::MinMax))
            .map(|(i, _)| i)
            .collect(),
        has_repeat: kinds.contains(&TokenKind::Repeat),
        tokens,
    }
}

/// A number as written back into a track list: at most three decimals, no
/// trailing zeros.
fn fmt_num(v: f32) -> String {
    let s = format!("{:.3}", (v * defaults::GRID_EDIT_DECIMAL_SCALE).round() / defaults::GRID_EDIT_DECIMAL_SCALE);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `150px` — pixels are whole numbers.
pub fn fmt_px(v: f32) -> String {
    format!("{}px", v.round() as i64)
}

/// `0.5fr`.
pub fn fmt_fr(v: f32) -> String {
    format!("{}fr", fmt_num(v))
}

/// `25%`.
pub fn fmt_percent(v: f32) -> String {
    format!("{}%", fmt_num(v))
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

    /// Spec 088 T1 — a track list is edited as TOKENS, so what the parser cannot
    /// read survives an edit: splitting and joining keeps every token as written
    /// (an unreadable one included), however the list is spelled.
    #[test]
    fn a_track_list_splits_into_tokens_and_joins_back_unchanged() {
        let cases = [
            "1fr 1fr",
            "100px 200px",
            "1FR  100",
            "Repeat(3, 1fr)",
            "MinMax(100px, 1fr) 2fr",
            "200px nonsense 1fr",
            "Repeat(AutoFill, MinMax(160px, 1fr)) 50%",
            "",
            "   ",
        ];
        for c in cases {
            let tokens = split(c);
            let again = split(&join(&tokens));
            assert_eq!(tokens, again, "split(join(split({c:?}))) must be split({c:?})");
        }
        assert_eq!(split("1FR  100"), ["1FR", "100"], "spelling is kept, spaces between tokens are not");
        assert_eq!(split("MinMax(100px, 1fr) 2fr"), ["MinMax(100px, 1fr)", "2fr"], "a call stays one token");
        assert_eq!(split("200px nonsense 1fr"), ["200px", "nonsense", "1fr"], "an unreadable token is kept");
        assert_eq!(join(&split("  1fr   2fr ")), "1fr 2fr");
        assert!(split("").is_empty() && split("  ").is_empty());
        println!("track lists: {} spellings split and joined back token-for-token", cases.len());
    }

    /// What each token is: a track, a MinMax track, a Repeat (which a drag, an
    /// add and a remove refuse), or something the parser skips.
    #[test]
    fn a_token_says_what_it_is() {
        use TokenKind::*;
        assert_eq!(kind_of("1fr"), Plain(fr(1.0)));
        assert_eq!(kind_of("100"), Plain(px(100.0)));
        assert_eq!(kind_of("25%"), Plain(Track { min: Breadth::Percent(25.0), max: Breadth::Percent(25.0) }));
        assert_eq!(kind_of("Auto"), Plain(Track { min: Breadth::Auto, max: Breadth::Auto }));
        assert_eq!(kind_of("MinMax(100px, 1fr)"), MinMax);
        assert_eq!(kind_of("Repeat(3, 1fr)"), Repeat);
        assert_eq!(kind_of("repeat(auto-fill, 100px)"), Repeat);
        assert_eq!(kind_of("nonsense"), Unreadable);
        assert_eq!(kind_of("Repeat(0, 1fr)"), Repeat, "an empty repeat is still a Repeat the developer wrote");
        assert_eq!(kind_of("Repeat(2)"), Unreadable, "a Repeat with nothing to repeat is skipped by the parser");
        assert!(has_repeat("100px Repeat(2, 1fr)") && !has_repeat("100px 1fr MinMax(1px, 1fr)"));
        println!("token kinds: plain, MinMax, Repeat (fixed and AutoFill) and unreadable told apart");
    }

    /// Which token holds which track: an unreadable token holds none, so the
    /// third TRACK of `1fr nonsense 2fr 3fr` is the FOURTH token.
    #[test]
    fn a_track_knows_which_token_holds_it() {
        let s = slots("1fr nonsense 2fr MinMax(10px, 1fr)");
        assert_eq!(s.tokens.len(), 4);
        assert_eq!(s.track_to_token, vec![0, 2, 3], "nonsense holds no track");
        assert!(!s.has_repeat);
        assert!(slots("100px Repeat(2, 1fr)").has_repeat);
        println!("track → token: [0, 2, 3] over 4 tokens, the unreadable one skipped");
    }

    /// What an edit writes back: whole numbers of pixels, trimmed decimals for
    /// the rest.
    #[test]
    fn written_tracks_are_short_and_exact() {
        assert_eq!(fmt_px(150.0), "150px");
        assert_eq!(fmt_px(149.6), "150px", "pixels round to a whole number");
        assert_eq!(fmt_fr(0.5), "0.5fr");
        assert_eq!(fmt_fr(1.5), "1.5fr");
        assert_eq!(fmt_fr(2.0), "2fr");
        assert_eq!(fmt_fr(1.0 / 3.0), "0.333fr", "at most three decimals");
        assert_eq!(fmt_percent(25.0), "25%");
        assert_eq!(fmt_percent(33.3333), "33.333%");
        // What is written is read back as what was meant.
        for (text, want) in [(fmt_fr(0.5), fr(0.5)), (fmt_px(150.0), px(150.0))] {
            assert_eq!(kind_of(&text), TokenKind::Plain(want), "{text} reads back");
        }
        println!("fmt: 150px, 0.5fr, 1.5fr, 2fr, 0.333fr, 25%, 33.333% — and read back as written");
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
