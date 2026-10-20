// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Text layout (WASM-018 R8) with synthetic fonts whose metrics are exact:
//! Latin, Japanese/Chinese, Hebrew and Arabic, bidirectional order, fallback,
//! wrapping and the Unicode line-breaking rules.
#![cfg(feature = "text")]

mod common;

use cobolt_graphics::display_list::consts::TextDirection;
use cobolt_graphics::text::FontSet;
use cobolt_graphics::textapi::{FontSpec, TextBlock, TextEngine, TextRequest};
use common::synthfont;
use tiny_skia::{Path, PathSegment};

fn fonts() -> FontSet {
    let f = FontSet::new();
    f.add_font_data(synthfont::latin("SynthLatin")).unwrap();
    f.add_font_data(synthfont::cjk("SynthCJK")).unwrap();
    f.add_font_data(synthfont::hebrew("SynthHebrew")).unwrap();
    f.add_font_data(synthfont::arabic("SynthArabic")).unwrap();
    f
}

fn lay(f: &FontSet, text: &str, css: &str, dir: TextDirection, wrap: Option<f32>) -> TextBlock {
    let font = FontSpec::parse(css).unwrap();
    f.layout(&TextRequest { text, font: &font, direction: dir, wrap_width: wrap, line_height: None }).unwrap()
}

/// The height, in font units (em = 1000), of each glyph box of a line in drawing order.
fn heights(block: &TextBlock, line: usize, size: f32) -> Vec<i32> {
    let path: &Path = block.lines[line].path.as_ref().expect("the line has ink");
    let mut out = Vec::new();
    let (mut lo, mut hi, mut open) = (f32::MAX, f32::MIN, false);
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                if open {
                    out.push(((hi - lo) * 1000.0 / size).round() as i32);
                }
                lo = p.y;
                hi = p.y;
                open = true;
            }
            PathSegment::LineTo(p) => {
                lo = lo.min(p.y);
                hi = hi.max(p.y);
            }
            _ => {}
        }
    }
    if open {
        out.push(((hi - lo) * 1000.0 / size).round() as i32);
    }
    out
}

fn latin_height(ch: char) -> i32 {
    300 + ((ch as i32 - '!' as i32) % 40) * 10
}

#[test]
fn latin_width_is_the_sum_of_the_advances() {
    let f = fonts();
    // 'H','e','l','l','o' are 600 units each; at 100 px that is 60 px.
    let b = lay(&f, "Hello", "100px SynthLatin", TextDirection::Auto, None);
    assert_eq!(b.lines.len(), 1);
    assert!((b.width - 300.0).abs() < 1e-3, "{}", b.width);
    assert!((b.ascent - 80.0).abs() < 1e-3 && (b.descent - 20.0).abs() < 1e-3, "{} {}", b.ascent, b.descent);
    assert!(!b.rtl);
    // A space is 300 units.
    let b = lay(&f, "a a", "100px SynthLatin", TextDirection::Auto, None);
    assert!((b.width - (60.0 + 30.0 + 60.0)).abs() < 1e-3, "{}", b.width);
}

#[test]
fn japanese_and_chinese_are_full_width() {
    let f = fonts();
    let b = lay(&f, "日本語", "20px SynthCJK", TextDirection::Auto, None);
    assert!((b.width - 60.0).abs() < 1e-3, "{}", b.width);
    let b = lay(&f, "你好世界", "10px SynthCJK", TextDirection::Auto, None);
    assert!((b.width - 40.0).abs() < 1e-3, "{}", b.width);
    assert!(!b.rtl);
}

#[test]
fn a_missing_character_comes_from_the_next_font() {
    let f = fonts();
    // The requested family has no kanji; the CJK font was registered later and is the fallback.
    let b = lay(&f, "Hi日本", "10px SynthLatin", TextDirection::Auto, None);
    assert!((b.width - (6.0 + 6.0 + 10.0 + 10.0)).abs() < 1e-3, "{}", b.width);
    let h = heights(&b, 0, 10.0);
    assert_eq!(h.len(), 4, "four glyph boxes: {h:?}");
    assert_eq!(h[0], latin_height('H'));
    assert_eq!(h[1], latin_height('i'));
}

#[test]
fn hebrew_runs_right_to_left_and_the_paragraph_direction_is_detected() {
    let f = fonts();
    let b = lay(&f, "אבג", "10px SynthHebrew", TextDirection::Auto, None);
    assert!(b.rtl && b.lines[0].rtl);
    // Visual order is the reverse of the logical order: gimel, bet, alef.
    let want: Vec<i32> = [2, 1, 0].iter().map(|i| 400 + i * 15).collect();
    assert_eq!(heights(&b, 0, 10.0), want);
    // The advance is the same whichever way it runs: 500, 520, 540 units at 10 px.
    assert!((b.width - (5.0 + 5.2 + 5.4)).abs() < 1e-3, "{}", b.width);
}

#[test]
fn arabic_is_right_to_left_too() {
    let f = fonts();
    let b = lay(&f, "\u{0627}\u{0628}\u{062A}", "10px SynthArabic", TextDirection::Auto, None);
    assert!(b.rtl);
    let idx = |c: char| c as i32 - 0x0621;
    let want: Vec<i32> = ['\u{062A}', '\u{0628}', '\u{0627}'].iter().map(|c| 350 + idx(*c) * 8).collect();
    assert_eq!(heights(&b, 0, 10.0), want);
}

#[test]
fn mixed_text_is_reordered_per_line() {
    let f = fonts();
    // Left-to-right paragraph with a Hebrew word in the middle: the Hebrew letters are
    // reversed in place and the Latin around them keeps its order.
    let b = lay(&f, "ab אבג cd", "10px SynthLatin", TextDirection::Auto, None);
    assert!(!b.rtl);
    let heb = |i: i32| 400 + i * 15;
    let want = vec![
        latin_height('a'),
        latin_height('b'),
        // the space has no ink and so no box
        heb(2),
        heb(1),
        heb(0),
        latin_height('c'),
        latin_height('d'),
    ];
    assert_eq!(heights(&b, 0, 10.0), want);
}

#[test]
fn a_forced_right_to_left_paragraph_places_latin_runs_in_reverse_order() {
    let f = fonts();
    // UAX #9 rule N1: the space between two left-to-right letters is left-to-right too, so a
    // plain Latin phrase keeps its order even in a right-to-left paragraph.
    let b = lay(&f, "ab cd", "10px SynthLatin", TextDirection::Rtl, None);
    assert!(b.rtl);
    let want = vec![latin_height('a'), latin_height('b'), latin_height('c'), latin_height('d')];
    assert_eq!(heights(&b, 0, 10.0), want);
    // With Hebrew between them the Latin words swap sides; each stays left-to-right inside.
    let b = lay(&f, "ab אב cd", "10px SynthLatin", TextDirection::Rtl, None);
    let heb = |i: i32| 400 + i * 15;
    let want = vec![latin_height('c'), latin_height('d'), heb(1), heb(0), latin_height('a'), latin_height('b')];
    assert_eq!(heights(&b, 0, 10.0), want);
    let b = lay(&f, "אב", "10px SynthHebrew", TextDirection::Ltr, None);
    assert!(!b.rtl, "the base direction can be forced left-to-right");
}

#[test]
fn latin_text_wraps_at_the_spaces() {
    let f = fonts();
    // Each word is 3 x 6 = 18 px and a space 3 px: "aaa bbb" is 39 px.
    let b = lay(&f, "aaa bbb ccc ddd", "10px SynthLatin", TextDirection::Auto, Some(40.0));
    assert_eq!(b.lines.len(), 2);
    assert!((b.lines[0].advance - 39.0).abs() < 1e-3, "the trailing space is not counted: {}", b.lines[0].advance);
    assert!((b.lines[1].advance - 39.0).abs() < 1e-3);
    let step = b.lines[1].baseline - b.lines[0].baseline;
    assert!((step - b.line_height).abs() < 1e-3, "lines are one line height apart");
    // Narrower: one word per line.
    let b = lay(&f, "aaa bbb ccc ddd", "10px SynthLatin", TextDirection::Auto, Some(20.0));
    assert_eq!(b.lines.len(), 4);
    // Wider than everything: one line.
    let b = lay(&f, "aaa bbb ccc ddd", "10px SynthLatin", TextDirection::Auto, Some(1000.0));
    assert_eq!(b.lines.len(), 1);
}

#[test]
fn a_word_wider_than_the_line_overflows_instead_of_vanishing() {
    let f = fonts();
    let b = lay(&f, "abcdefgh ij", "10px SynthLatin", TextDirection::Auto, Some(20.0));
    assert_eq!(b.lines.len(), 2);
    assert!((b.lines[0].advance - 48.0).abs() < 1e-3, "{}", b.lines[0].advance);
    assert!(b.lines[0].path.is_some());
}

#[test]
fn cjk_breaks_between_ideographs_but_not_before_closing_punctuation() {
    let f = fonts();
    // Ten pixels per ideograph; 25 px fits two.
    let b = lay(&f, "日本語日本語", "10px SynthCJK", TextDirection::Auto, Some(25.0));
    assert_eq!(b.lines.len(), 3);
    for l in &b.lines {
        assert!((l.advance - 20.0).abs() < 1e-3, "{}", l.advance);
    }
    // UAX #14: no break before the full stop, so it travels with the character before it.
    let b = lay(&f, "日本。語", "10px SynthCJK", TextDirection::Auto, Some(20.0));
    let advances: Vec<f32> = b.lines.iter().map(|l| l.advance).collect();
    assert_eq!(advances, vec![10.0, 20.0, 10.0], "日 | 本。 | 語");
    // And not after the opening bracket either.
    let b = lay(&f, "日「本」語", "10px SynthCJK", TextDirection::Auto, Some(20.0));
    let first = b.lines[0].advance;
    assert!(first <= 10.0 + 1e-3, "the bracket does not stay at the end of a line: {:?}", b.lines.iter().map(|l| l.advance).collect::<Vec<_>>());
}

#[test]
fn line_feeds_end_lines_and_blank_lines_stay() {
    let f = fonts();
    assert_eq!(lay(&f, "a\nb", "10px SynthLatin", TextDirection::Auto, None).lines.len(), 2);
    assert_eq!(lay(&f, "a\n\nb", "10px SynthLatin", TextDirection::Auto, None).lines.len(), 3);
    assert_eq!(lay(&f, "a\r\nb", "10px SynthLatin", TextDirection::Auto, None).lines.len(), 2);
    let b = lay(&f, "", "10px SynthLatin", TextDirection::Auto, None);
    assert_eq!(b.lines.len(), 1);
    assert_eq!(b.lines[0].advance, 0.0);
    assert!(b.lines[0].path.is_none());
}

#[test]
fn an_explicit_line_height_is_used() {
    let f = fonts();
    let font = FontSpec::parse("10px SynthLatin").unwrap();
    let b = f
        .layout(&TextRequest { text: "a\nb", font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: Some(25.0) })
        .unwrap();
    assert!((b.lines[1].baseline - 25.0).abs() < 1e-4);
    assert_eq!(b.line_height, 25.0);
}

#[test]
fn layouts_repeat_exactly_and_from_many_threads() {
    let f = std::sync::Arc::new(fonts());
    let text = "Hello אבג 日本語 world, a longer line to wrap at some width";
    let one = lay(&f, text, "12px SynthLatin", TextDirection::Auto, Some(90.0));
    let sig = |b: &TextBlock| -> Vec<(u32, usize)> {
        b.lines.iter().map(|l| (l.advance.to_bits(), l.path.as_ref().map(|p| p.points().len()).unwrap_or(0))).collect()
    };
    let want = sig(&one);
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let f = f.clone();
            std::thread::spawn(move || sig(&lay(&f, text, "12px SynthLatin", TextDirection::Auto, Some(90.0))))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), want);
    }
}

#[test]
fn fonts_are_found_by_family_name_without_regard_to_case() {
    let f = fonts();
    let b = lay(&f, "aaaa", "10px 'synthlatin'", TextDirection::Auto, None);
    assert!((b.width - 24.0).abs() < 1e-3);
    assert!(f.families().contains(&"SynthLatin".to_string()));
}

#[test]
fn bad_font_data_is_refused_readably() {
    let f = FontSet::new();
    let e = f.add_font_data(vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap_err();
    assert_eq!(e.code(), "font", "{e}");
    let lim = cobolt_graphics::Limits { max_font_bytes: 100, ..Default::default() };
    let g = FontSet::with_limits(lim);
    let e = g.add_font_data(synthfont::latin("Big")).unwrap_err();
    assert!(e.to_string().contains("max_font_bytes"), "{e}");
    // Asking an empty set to lay text out is an error, not a panic.
    let font = FontSpec::default();
    let e = f.layout(&TextRequest { text: "x", font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).unwrap_err();
    assert!(e.to_string().contains("no font"), "{e}");
}

#[test]
fn text_beyond_the_limit_is_refused() {
    let f = fonts();
    let font = FontSpec::parse("10px SynthLatin").unwrap();
    let long = "a".repeat(2 * 1024 * 1024);
    let e = f.layout(&TextRequest { text: &long, font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).unwrap_err();
    assert!(e.to_string().contains("max_text_bytes"), "{e}");
}

#[test]
fn degenerate_inputs_never_panic() {
    let f = fonts();
    for text in ["\u{200F}", "\u{202E}abc\u{202C}", "\u{0301}", "a\u{0301}\u{0301}", "\0", "\u{FFFF}", "🙂", " ", "   ", "\n", "\t"] {
        for wrap in [None, Some(1.0), Some(100.0)] {
            for dir in [TextDirection::Auto, TextDirection::Ltr, TextDirection::Rtl] {
                let _ = lay(&f, text, "10px SynthLatin", dir, wrap);
            }
        }
    }
}
