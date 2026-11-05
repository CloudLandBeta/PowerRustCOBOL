// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The canvas part built for `wasm32`, drawing the desktop's picture** (WASM-018 T30, AC1).
//!
//! `cobolt-graphics/tests/golden/parity-scene.txt` is run in a real Chrome through the shipped `canvas` part
//! (the same calls, by name, a form's COBOL makes), and the picture it answers is compared, pixel by pixel, with
//! `parity-canvas.png`, which the desktop's native `Registry` produced and pins byte for byte
//! (`cobolt-graphics/tests/parity.rs`). The number that matters is the count of differing pixels: **0**.
//!
//! The text has accents and Japanese and Chinese in it. The shipped font (Ubuntu Light) has no Japanese or Chinese
//! glyphs, so those two strings are the font's missing-glyph box on every host; what the test proves is that the
//! box, the Latin text and the shapes come out the same, not that a CJK font is in use.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use support::cdp::{find_chrome, Chrome};

const PAGE: &str = r##"<!doctype html><meta charset="utf-8"><title>parity</title><script type="module">
const R = { done: false };
window.R = R;
try {
  const m = await import('./parts/canvas/canvas.js');
  await m.default('./parts/canvas/canvas_bg.wasm');
  const g = new m.Graphics();
  g.add_font(new Uint8Array(await (await fetch('./Ubuntu-Light.ttf')).arrayBuffer()));
  const scene = (await (await fetch('./parity-scene.txt')).text()).split('\n').filter(l => l.trim());
  g.make(1, 'Canvas2D', 160, 120, 1);
  const t0 = performance.now();
  for (const line of scene) {
    const [method, ...args] = line.split('|');
    g.call(1, method, JSON.stringify(args));
  }
  R.drawMs = performance.now() - t0;
  R.width = g.device_width(1);
  R.height = g.device_height(1);
  R.rgba = Array.from(g.rgba(1));
  R.calls = scene.length;

  // The answers: text measurements and a settled stack of boxes, one line per call (`Method: answer`).
  const script = (await (await fetch('./parity-answers.txt')).text()).split('\n').filter(l => l.trim());
  const canvasPart = new m.Graphics();
  canvasPart.add_font(new Uint8Array(await (await fetch('./Ubuntu-Light.ttf')).arrayBuffer()));
  const gm = await import('./parts/graphics/graphics.js');
  await gm.default('./parts/graphics/graphics_bg.wasm');
  const physicsPart = new gm.Graphics();
  const labels = [];
  const lines = [];
  const t1 = performance.now();
  for (const line of script) {
    const [label, kind, method, ...args] = line.split('|');
    const part = kind === 'Physics2D' ? physicsPart : canvasPart;
    let ctl = labels.indexOf(label) + 1;
    if (!ctl) { labels.push(label); ctl = labels.length; part.make(ctl, kind, 160, 120, 1); }
    lines.push(method + ': ' + part.call(ctl, method, JSON.stringify(args)));
  }
  R.answersMs = performance.now() - t1;
  R.answers = lines.join('\n') + '\n';

  // AC7: the 24-megapixel blur, timed (the part runs on the page's own thread here: the time is the time the page is held).
  canvasPart.make(9, 'ImageBuffer', 0, 0, 1);
  canvasPart.call(9, 'Create', JSON.stringify(['6000', '4000', '#336699']));
  canvasPart.call(9, 'SetPixel', JSON.stringify(['3000', '2000', '#FFFFFF']));
  const t2 = performance.now();
  canvasPart.call(9, 'BlurGaussian', JSON.stringify(['2.5']));
  R.blurMs = performance.now() - t2;
  R.blurPixels = 6000 * 4000;
} catch (e) {
  R.error = String(e && e.stack || e);
}
R.done = true;
</script>"##;

#[test]
fn the_canvas_part_in_chrome_draws_the_desktops_picture_with_no_pixel_different() {
    let started = std::time::Instant::now();
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to one)");
        return;
    };
    let root = support::repo_root();
    let golden = root.join("crates/cobolt-graphics/tests/golden");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), PAGE).unwrap();
    let to = dir.path().join("parts/canvas");
    std::fs::create_dir_all(&to).unwrap();
    for f in ["canvas.js", "canvas_bg.wasm"] {
        std::fs::copy(root.join("assets/web-player/parts/canvas").join(f), to.join(f)).unwrap();
    }
    let to = dir.path().join("parts/graphics");
    std::fs::create_dir_all(&to).unwrap();
    for f in ["graphics.js", "graphics_bg.wasm"] {
        std::fs::copy(root.join("assets/web-player/parts/graphics").join(f), to.join(f)).unwrap();
    }
    std::fs::copy(golden.join("parity-answers.txt"), dir.path().join("parity-answers.txt")).unwrap();
    std::fs::copy(root.join("assets/web-fonts/Ubuntu-Light.ttf"), dir.path().join("Ubuntu-Light.ttf")).unwrap();
    std::fs::copy(golden.join("parity-scene.txt"), dir.path().join("parity-scene.txt")).unwrap();
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions::default()).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for("window.R && window.R.done", 120).unwrap_or_else(|e| panic!("the page did not finish: {e}"));
    let r = page.eval("window.R").unwrap();
    assert!(r["error"].is_null(), "the part failed in the browser: {}", r["error"]);

    let got: Vec<u8> = r["rgba"].as_array().expect("the picture").iter().map(|v| v.as_u64().unwrap() as u8).collect();
    let (w, h) = (r["width"].as_u64().unwrap() as u32, r["height"].as_u64().unwrap() as u32);

    let mut decoder = png::Decoder::new(std::fs::File::open(golden.join("parity-canvas.png")).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut want = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut want).unwrap();
    assert_eq!((info.width, info.height), (w, h), "the picture has the golden's size");
    assert_eq!(info.color_type, png::ColorType::Rgba, "the golden is RGBA");
    want.truncate(info.buffer_size());

    let differing = want.chunks_exact(4).zip(got.chunks_exact(4)).filter(|(a, b)| a != b).count();
    let worst = want.iter().zip(&got).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    println!(
        "T30 parity wasm32 in Chrome: {w}x{h}, {} calls in {:.1} ms; {differing} of {} pixels differ from the desktop's picture (worst channel {worst}); {} ms in all",
        r["calls"],
        r["drawMs"].as_f64().unwrap_or(0.0),
        w * h,
        started.elapsed().as_millis()
    );
    assert_eq!(differing, 0, "{differing} pixels differ between the desktop's picture and the canvas part's");

    // AC3 and AC6: the same measurements, the same settled stack, the same state fingerprint as the desktop's registry.
    let want = std::fs::read_to_string(golden.join("parity-answers.golden")).unwrap();
    let got = r["answers"].as_str().expect("the answers");
    let differing_lines = want.lines().zip(got.lines()).filter(|(a, b)| a != b).count() + want.lines().count().abs_diff(got.lines().count());
    println!(
        "T30 answers wasm32 in Chrome: {} answers in {:.1} ms; {differing_lines} differ from the desktop's ({} text measurements, 3 boxes settled, fingerprint {})",
        got.lines().count(),
        r["answersMs"].as_f64().unwrap_or(0.0),
        got.lines().filter(|l| l.starts_with("MeasureText")).count(),
        got.lines().find(|l| l.starts_with("GetStateFingerprint")).unwrap_or("")
    );
    println!("T30 AC7 wasm32 in Chrome: a {} pixel Gaussian blur (sigma 2.5) took {:.0} ms on the page's own thread", r["blurPixels"], r["blurMs"].as_f64().unwrap_or(0.0));
    assert!(r["blurMs"].as_f64().unwrap_or(f64::MAX) < 60_000.0, "the 24 MP blur finished");
    assert_eq!(got, want, "an answer differs between the desktop's registry and the parts in Chrome");
}
