// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Lifecycle and limits of the graphics parts in a real browser** (WASM-018 T31, AC9, AC10).
//!
//! * 100 form lifetimes on the `canvas` part (a canvas drawn and presented, an image made and blurred) and on the `graphics`
//!   part (a chart with 50 points, a physics world), each ending in `release`: the registry holds no object after a cycle,
//!   and the module's linear memory (which WebAssembly never gives back) does not grow once it has warmed up: the bytes after
//!   cycle 100 are the bytes after cycle 10.
//! * An oversized canvas, an oversized image, an oversized display list and bytes that are no image are each refused with a
//!   sentence, and the part is still usable afterwards.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use support::cdp::{find_chrome, Chrome};

const PAGE: &str = r##"<!doctype html><meta charset="utf-8"><title>lifecycle</title><script type="module">
const R = { done: false };
window.R = R;
const refused = f => { try { f(); return null; } catch (e) { return String((e && e.message) || e); } };
const call = (g, ctl, m, a = []) => g.call(ctl, m, JSON.stringify(a.map(String)));
try {
  const load = async name => {
    const mod = await import(`./parts/${name}/${name}.js`);
    const exports = await mod.default(`./parts/${name}/${name}_bg.wasm`);
    return { mod, mem: () => exports.memory.buffer.byteLength };
  };
  const canvas = await load('canvas');
  const graphics = await load('graphics');

  const cycleCanvas = (g, n) => {
    g.make(1, 'Canvas2D', 320, 240, 1);
    call(g, 1, 'SetFillColor', ['#336699']);
    call(g, 1, 'FillRect', [0, 0, 320, 240]);
    call(g, 1, 'Present');
    g.rgba(1);
    g.make(2, 'ImageBuffer', 0, 0, 1);
    call(g, 2, 'Create', [200, 150, '#FF8800']);
    call(g, 2, 'BlurGaussian', [2]);
    g.release(1); g.release(2);
  };
  const cycleGraphics = (g, n) => {
    g.make(3, 'Plot', 0, 0, 1);
    const s = call(g, 3, 'AddSeries', ['line', 's']);
    for (let i = 0; i < 50; i++) call(g, 3, 'AddPoint', [s, i, i * i]);
    call(g, 3, 'ToSvg', [200, 120]);
    g.make(4, 'Physics2D', 0, 0, 1);
    call(g, 4, 'CreateWorld', [0, 10]);
    g.release(3); g.release(4);
  };

  const c = new canvas.mod.Graphics();
  const g = new graphics.mod.Graphics();
  const t0 = performance.now();
  const samples = [];
  for (let n = 1; n <= 100; n++) {
    cycleCanvas(c, n); cycleGraphics(g, n);
    if (c.object_count() !== 0 || g.object_count() !== 0) throw new Error(`cycle ${n}: objects left ${c.object_count()} + ${g.object_count()}`);
    if (n === 10 || n === 50 || n === 100) samples.push([n, canvas.mem(), graphics.mem()]);
  }
  R.ms = performance.now() - t0;
  R.samples = samples;

  // The refusals, each a sentence; the part still works afterwards.
  const lim = new canvas.mod.Graphics();
  R.bigCanvas = refused(() => lim.make(1, 'Canvas2D', 100000, 100000, 1));
  lim.make(2, 'ImageBuffer', 0, 0, 1);
  R.bigImage = refused(() => call(lim, 2, 'Create', [100000, 100000]));
  R.badImage = refused(() => call(lim, 2, 'LoadFromBytes', ['this is not an image']));
  lim.make(3, 'Canvas2D', 8, 8, 1);
  const list = new Uint8Array([0x50, 0x52, 0x47, 0x44, 1, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0]);
  R.bigList = refused(() => lim.draw_list(3, list));
  call(lim, 3, 'SetFillColor', ['#FF0000']);
  call(lim, 3, 'FillRect', [0, 0, 4, 4]);
  call(lim, 3, 'Present');
  R.after = call(lim, 3, 'GetPixel', [1, 1]);
} catch (e) {
  R.error = String(e && e.stack || e);
}
R.done = true;
</script>"##;

#[test]
fn a_hundred_part_lifetimes_leave_no_object_and_no_growth_and_hostile_inputs_are_refused() {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to one)");
        return;
    };
    let root = support::repo_root();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), PAGE).unwrap();
    for name in ["canvas", "graphics"] {
        let to = dir.path().join("parts").join(name);
        std::fs::create_dir_all(&to).unwrap();
        for f in [format!("{name}.js"), format!("{name}_bg.wasm")] {
            std::fs::copy(root.join("assets/web-player/parts").join(name).join(&f), to.join(&f)).unwrap();
        }
    }
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions::default()).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for("window.R && window.R.done", 180).unwrap_or_else(|e| panic!("the page did not finish: {e}"));
    let r = page.eval("window.R").unwrap();
    assert!(r["error"].is_null(), "a part failed in the browser: {}", r["error"]);

    let samples = r["samples"].as_array().unwrap();
    let mem = |i: usize, part: usize| samples[i][part].as_u64().unwrap();
    let (c10, c100) = (mem(0, 1), mem(2, 1));
    let (g10, g100) = (mem(0, 2), mem(2, 2));
    assert_eq!(c10, c100, "the canvas part's memory grew between cycle 10 and cycle 100");
    assert_eq!(g10, g100, "the graphics part's memory grew between cycle 10 and cycle 100");
    for k in ["bigCanvas", "bigImage", "badImage", "bigList"] {
        let why = r[k].as_str().unwrap_or("");
        assert!(why.len() > 15, "{k} was not refused with a sentence: {:?}", r[k]);
    }
    assert_eq!(r["after"], "4278190335", "the part still draws after four refusals");
    println!(
        "T31 parts in Chrome: 100 lifetimes of each part in {:.0} ms, 0 objects left after every cycle; linear memory canvas {} KB at cycle 10 and {} KB at cycle 100, graphics {} KB and {} KB; refusals: canvas [{}] image [{}] bad image [{}] list [{}]",
        r["ms"].as_f64().unwrap_or(0.0),
        c10 / 1024,
        c100 / 1024,
        g10 / 1024,
        g100 / 1024,
        r["bigCanvas"].as_str().unwrap_or(""),
        r["bigImage"].as_str().unwrap_or(""),
        r["badImage"].as_str().unwrap_or(""),
        r["bigList"].as_str().unwrap_or("")
    );
}
