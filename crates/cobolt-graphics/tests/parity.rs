// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **One scene, the same picture on every host** (WASM-018 T30, AC1, AC3).
//!
//! `tests/golden/parity-scene.txt` is a list of Canvas2D calls (`Method|arg|arg`, a text with
//! accents and with Japanese and Chinese in it). This test runs it natively through the
//! [`Registry`] the desktop uses and compares the picture with `tests/golden/parity-canvas.png`
//! byte for byte; it also writes `parity-canvas.fnv`, the fingerprint the other hosts compare with:
//! the interpreter (`cobolt-runtime`) and the player's canvas part built for `wasm32` and run in a
//! real Chrome (`cobolt-web-bundle/tests/graphics_parity_chrome.rs`).
//!
//! `GOLDEN_UPDATE=1` rewrites both files.
#![cfg(feature = "text")]

mod common;

use std::time::Instant;

use cobolt_graphics::registry::{ObjectKind, Registry};
use cobolt_graphics::value::Value;
use cobolt_graphics::{Limits, PixelBuffer};
use common::{fingerprint, golden_dir};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 120;

/// A text argument as the player's part and the interpreter hand it over: a whole number, a number, or text.
fn value_of(text: &str) -> Value {
    let t = text.trim();
    t.parse::<i64>().map(Value::Int).or_else(|_| t.parse::<f64>().map(Value::Num)).unwrap_or_else(|_| Value::Text(text.to_owned()))
}

fn font() -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/web-fonts/Ubuntu-Light.ttf")).expect("the stand-in font")
}

/// Run the scene natively; the picture as straight-alpha RGBA.
fn native_picture() -> Vec<u8> {
    let scene = std::fs::read_to_string(golden_dir().join("parity-scene.txt")).expect("the scene");
    let mut reg = Registry::new(Limits::default());
    reg.fonts().add_font_data(font()).expect("the font");
    let h = reg.create(ObjectKind::Canvas2D, 1, &[Value::Int(WIDTH as i64), Value::Int(HEIGHT as i64), Value::Num(1.0)]).expect("a canvas");
    for line in scene.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.split('|');
        let method = parts.next().unwrap();
        let args: Vec<Value> = parts.map(value_of).collect();
        reg.invoke(h, method, &args).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    reg.with_canvas2d(h, |c, _| {
        let (w, h) = c.device_size();
        let mut px = vec![0u8; w as usize * h as usize * 4];
        c.write_rgba(&mut px);
        Ok(px)
    })
    .expect("the picture")
}

#[test]
fn the_scene_is_the_golden_picture_byte_for_byte_and_its_fingerprint_is_written_for_the_other_hosts() {
    let t0 = Instant::now();
    let px = native_picture();
    let draw_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let picture = PixelBuffer::from_rgba(WIDTH, HEIGHT, px.clone()).unwrap();
    let print = format!("{WIDTH}x{HEIGHT} {:016x}\n", fingerprint(&px));
    let fnv = golden_dir().join("parity-canvas.fnv");
    if std::env::var("GOLDEN_UPDATE").map(|v| v == "1").unwrap_or(false) {
        std::fs::write(&fnv, &print).unwrap();
    }
    common::assert_golden("parity-canvas", &picture, common::Tolerance::EXACT);
    assert_eq!(std::fs::read_to_string(&fnv).expect("run once with GOLDEN_UPDATE=1"), print, "the fingerprint file is the picture's");
    let inked = px.chunks_exact(4).filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200).count();
    assert!(inked > 100, "the white text was lettered: {inked} pixels");
    println!("T30 parity native: {WIDTH}x{HEIGHT} drawn in {draw_ms:.1} ms, fingerprint {}, {inked} white text pixels, 0 pixels differ from the golden", print.trim());
}

/// An answer as the player's part and the interpreter show it.
fn show(v: &Value) -> String {
    match v {
        Value::Nothing => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Num(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Value::Text(t) => t.clone(),
        Value::Bytes(b) => format!("{} bytes", b.len()),
        Value::Floats(f) => f.iter().map(f64::to_string).collect::<Vec<_>>().join(" "),
        Value::Ints(i) => i.iter().map(i64::to_string).collect::<Vec<_>>().join(" "),
    }
}

/// Run `tests/golden/parity-answers.txt` (`label|Kind|Method|args`: an object is made when its label first appears); one answer per line.
fn native_answers() -> String {
    let script = std::fs::read_to_string(golden_dir().join("parity-answers.txt")).expect("the script");
    let mut reg = Registry::new(Limits::default());
    reg.fonts().add_font_data(font()).expect("the font");
    let mut made: Vec<(String, cobolt_graphics::handles::Handle)> = Vec::new();
    let mut out = String::new();
    for line in script.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.split('|');
        let (label, kind, method) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
        let handle = match made.iter().find(|(l, _)| l == label) {
            Some((_, h)) => *h,
            None => {
                let kind = ObjectKind::from_name(kind).expect("a kind");
                let args = if kind == ObjectKind::Canvas2D { vec![Value::Int(160), Value::Int(120), Value::Num(1.0)] } else { Vec::new() };
                let h = reg.create(kind, 1, &args).expect("an object");
                made.push((label.to_owned(), h));
                h
            }
        };
        let args: Vec<Value> = parts.map(value_of).collect();
        let answer = reg.invoke(handle, method, &args).unwrap_or_else(|e| panic!("{line}: {e}"));
        out.push_str(&format!("{method}: {}\n", show(&answer)));
    }
    out
}

#[cfg(feature = "physics")]
#[test]
fn the_measured_widths_and_the_settled_stack_are_the_answers_the_other_hosts_share() {
    let t0 = Instant::now();
    let got = native_answers();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let path = golden_dir().join("parity-answers.golden");
    if std::env::var("GOLDEN_UPDATE").map(|v| v == "1").unwrap_or(false) {
        std::fs::write(&path, &got).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&path).expect("run once with GOLDEN_UPDATE=1"), got, "the answers differ from the pinned transcript");
    let widths: Vec<&str> = got.lines().filter(|l| l.starts_with("MeasureText")).collect();
    let fingerprint = got.lines().last().unwrap_or("");
    println!("T30 parity answers native: {} answers in {ms:.1} ms; {} text measurements, stack settled, {fingerprint}", got.lines().count(), widths.len());
}

/// AC3: what `MeasureText` says a text spans is what `DrawText` inks: the ink never leaves the measured box, and its edges are the
/// measured ink edges (to a pixel).
#[test]
fn measure_text_says_where_draw_text_puts_the_ink() {
    let t0 = Instant::now();
    let mut reg = Registry::new(Limits::default());
    reg.fonts().add_font_data(font()).expect("the font");
    let h = reg.create(ObjectKind::Canvas2D, 1, &[Value::Int(200), Value::Int(40), Value::Num(1.0)]).unwrap();
    reg.invoke(h, "SetFont", &[Value::Text("16px Ubuntu".into())]).unwrap();
    reg.invoke(h, "SetFillColor", &[Value::Text("#FFFFFF".into())]).unwrap();
    let mut cases = 0;
    let mut worst = 0.0f64;
    for text in ["Hello, world", "Olá, mundo", "Hola, señor", "Bonjour, français", "WAVE To Ty"] {
        reg.invoke(h, "ClearRect", &[Value::Int(0), Value::Int(0), Value::Int(200), Value::Int(40)]).unwrap();
        let m = match reg.invoke(h, "MeasureText", &[Value::Text(text.into())]).unwrap() {
            Value::Floats(f) => f,
            other => panic!("metrics expected, got {other:?}"),
        };
        let (left, right) = (m[1], m[2]);
        reg.invoke(h, "DrawText", &[Value::Text(text.into()), Value::Int(10), Value::Int(28), Value::Int(0), Value::Int(1)]).unwrap();
        reg.invoke(h, "Flush", &[]).unwrap();
        let px = reg
            .with_canvas2d(h, |c, _| {
                let (w, hh) = c.device_size();
                let mut px = vec![0u8; w as usize * hh as usize * 4];
                c.write_rgba(&mut px);
                Ok(px)
            })
            .unwrap();
        let cols: Vec<usize> = (0..200usize).filter(|x| (0..40usize).any(|y| px[(y * 200 + x) * 4 + 3] > 0)).collect();
        let (first, last) = (*cols.first().expect("ink") as f64, *cols.last().unwrap() as f64 + 1.0);
        let (want_l, want_r) = (10.0 + left, 10.0 + right);
        // The measured box holds the ink (a pixel: the last column is partly covered), and it is tight: no more than 3 px of slack on a side.
        let (slack_l, slack_r) = (first - want_l, want_r - last);
        worst = worst.max(slack_l).max(slack_r);
        assert!(slack_l >= -1.0 && slack_r >= -1.0, "{text}: ink {first}..{last} leaves the measured box {want_l}..{want_r}");
        assert!(slack_l <= 3.0 && slack_r <= 3.0, "{text}: ink {first}..{last}, measured {want_l}..{want_r}: the box is not tight");
        cases += 1;
    }
    println!("T30 AC3 native: {cases} texts, the widest gap between the ink and the measured ink box is {worst:.2} px, {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
}
