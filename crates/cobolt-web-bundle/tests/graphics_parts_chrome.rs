// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The three graphics parts of the player, run in a real browser** (WASM-018 T26).
//!
//! `canvas`, `graphics` and `graphics3d` are WebAssembly modules built from one source with the `cobolt-graphics` features each
//! needs. A native test proves the source; only a browser proves the built modules: they load, they carry exactly the kinds their
//! name promises (a lighter part refuses a heavier kind with a sentence), they answer the library's own methods by name, and the
//! 3D part opens the browser's graphics device and copies a rendered picture back — or, on a machine whose browser has no adapter,
//! says so readably (WASM-018 AC5: neither WebGPU nor WebGL2 is a message, not a crash).
//!
//! The shipped parts (`assets/web-player/parts`) are loaded from a static page. With no Chrome on the machine (`CHROME` names
//! one) the test says it was skipped, and why.

mod support;

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use support::cdp::{find_chrome, Chrome};

const PAGE: &str = r##"<!doctype html><meta charset="utf-8"><title>parts</title><script type="module">
const load = async name => {
  const m = await import(`./parts/${name}/${name}.js`);
  await m.default(`./parts/${name}/${name}_bg.wasm`);
  return m;
};
const call = (g, ctl, method, args = []) => g.call(ctl, method, JSON.stringify(args.map(String)));
const refused = (f) => { try { f(); return null; } catch (e) { return String(e && e.message || e); } };
const R = { done: false };
window.R = R;
try {
  // canvas: Canvas2D and ImageBuffer, nothing heavier.
  const canvas = await load('canvas');
  const c = new canvas.Graphics();
  R.canvasKinds = c.kinds();
  c.make(1, 'Canvas2D', 20, 10, 1);
  call(c, 1, 'SetFillColor', ['#FF0000']);
  call(c, 1, 'FillRect', [0, 0, 10, 5]);
  R.pixel = call(c, 1, 'GetPixel', [2, 2]);
  R.handle = call(c, 1, 'GetHandle');
  R.canvasRefusesPlot = refused(() => c.make(2, 'Plot', 0, 0, 1));
  R.canvasObjects = c.object_count();
  c.free();

  // fonts (WASM-018 T27): no text is drawn until the page hands the part a font file; then the text is lettered with it.
  const t = new canvas.Graphics();
  t.make(9, 'Canvas2D', 80, 24, 1);
  call(t, 9, 'SetFillColor', ['#FFFFFF']);
  call(t, 9, 'SetFont', ['16px Ubuntu']);
  R.textWithoutFont = refused(() => call(t, 9, 'MeasureText', ['Hi']));
  const font = new Uint8Array(await (await fetch('./Ubuntu-Light.ttf')).arrayBuffer());
  R.facesAdded = t.add_font(font);
  R.notAFont = refused(() => t.add_font(new Uint8Array([1, 2, 3, 4])));
  call(t, 9, 'DrawText', ['Hi', 2, 18, 0, 1]);
  call(t, 9, 'Present');
  const tp = t.rgba(9);
  let inked = 0;
  for (let i = 3; i < tp.length; i += 4) if (tp[i]) inked++;
  R.inkedPixels = inked;
  t.free();

  // DrawImage (WASM-018 T25): an ImageBuffer of the same registry drawn onto a canvas, then read back.
  const d = new canvas.Graphics();
  d.make(1, 'Canvas2D', 40, 30, 1);
  d.make(2, 'ImageBuffer', 0, 0, 1);
  call(d, 2, 'Create', [10, 10, '#00FF00']);
  call(d, 1, 'DrawImage', [call(d, 2, 'GetHandle'), 5, 5]);
  call(d, 1, 'Present');
  R.imageInside = call(d, 1, 'GetPixel', [8, 8]);
  R.imageOutside = call(d, 1, 'GetPixel', [30, 25]);
  d.free();

  // graphics: plots and 2D physics in the same registry as the canvases.
  const graphics = await load('graphics');
  const g = new graphics.Graphics();
  R.graphicsKinds = g.kinds();
  g.make(3, 'Plot', 0, 0, 1);
  const series = call(g, 3, 'AddSeries', ['line', 'sales']);
  for (let i = 0; i < 8; i++) call(g, 3, 'AddPoint', [series, i, i * i]);
  R.seriesCount = call(g, 3, 'GetSeriesCount');
  R.svg = call(g, 3, 'ToSvg', [200, 120]);
  g.make(4, 'Physics2D', 0, 0, 1);
  call(g, 4, 'CreateWorld', [0, 10]);
  R.worlds = call(g, 4, 'GetWorldCount');
  R.graphicsRefusesScene = refused(() => g.make(5, 'Scene3D', 0, 0, 1));
  g.release(3);
  R.afterRelease = g.object_count();
  g.free();

  // graphics3d: the browser's device, a scene, a Canvas3D, a picture copied out.
  const g3 = await load('graphics3d');
  R.kinds3d = (new g3.Graphics()).kinds();
  let dev = null;
  try { dev = await g3.open_gpu(); } catch (e) { R.noGpu = String(e && e.message || e); }
  if (dev) {
    R.backend = dev.backend();
    const s = new g3.Graphics();
    s.set_gpu(dev);
    s.make(5, 'Scene3D', 0, 0, 1);
    s.make(6, 'Canvas3D', 64, 48, 1);
    call(s, 5, 'AddBox', [1, 1, 1]);
    call(s, 5, 'AddDirectionalLight', [1, 1, 1, 1]);
    call(s, 5, 'SetAmbient', [1, 1, 1, 0.4]);
    const cam = call(s, 5, 'AddCamera', [60, 0.1, 100]);
    call(s, 5, 'SetPosition', [cam, 0, 0, 4]);
    call(s, 6, 'SetScene', [call(s, 5, 'GetHandle')]);
    call(s, 6, 'SetCamera', [cam]);
    R.connect = call(s, 6, 'Connect');
    R.draws = call(s, 6, 'Render');
    const pic = s.picture3d(6);
    R.picW = pic.width(); R.picH = pic.height();
    const px = await pic.finish();
    R.picBytes = px.length;
    let lit = 0;
    const seen = new Set();
    for (let i = 0; i < px.length; i += 4) { if (px[i] || px[i + 1] || px[i + 2]) lit++; seen.add((px[i] << 16) | (px[i + 1] << 8) | px[i + 2]); }
    R.picLit = lit;
    R.picDistinct = seen.size;
    s.free();
  }
} catch (e) {
  R.error = String(e && e.stack || e);
}
R.done = true;
</script>"##;

fn run_page() -> Option<serde_json::Value> {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to one)");
        return None;
    };
    let parts = support::repo_root().join("assets/web-player/parts");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), PAGE).unwrap();
    for name in ["canvas", "graphics", "graphics3d"] {
        let to = dir.path().join("parts").join(name);
        std::fs::create_dir_all(&to).unwrap();
        for f in [format!("{name}.js"), format!("{name}_bg.wasm")] {
            std::fs::copy(parts.join(name).join(&f), to.join(&f)).unwrap_or_else(|e| panic!("{name}/{f}: {e}"));
        }
    }
    std::fs::copy(support::repo_root().join("assets/web-fonts/Ubuntu-Light.ttf"), dir.path().join("Ubuntu-Light.ttf")).unwrap();
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions::default()).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for("window.R && window.R.done", 120).unwrap_or_else(|e| panic!("the parts did not finish: {e}"));
    Some(page.eval("window.R").unwrap())
}

#[test]
fn the_three_parts_load_carry_their_kinds_and_answer_the_librarys_methods_in_a_real_browser() {
    let started = std::time::Instant::now();
    let Some(r) = run_page() else { return };
    assert!(r["error"].is_null(), "a part failed in the browser: {}", r["error"]);
    let kinds = |k: &str| r[k].as_str().unwrap_or("").to_owned();
    // canvas
    assert!(kinds("canvasKinds").contains("Canvas2D") && kinds("canvasKinds").contains("ImageBuffer"), "{}", kinds("canvasKinds"));
    assert!(!kinds("canvasKinds").contains("Plot") && !kinds("canvasKinds").contains("Scene3D"), "the canvas part carries no plots or 3D: {}", kinds("canvasKinds"));
    assert_eq!(r["pixel"], "4278190335", "GetPixel answers the red the part drew (0xFF0000FF)");
    assert_eq!(r["handle"], "1", "a Canvas2D answers its own handle");
    assert!(r["canvasRefusesPlot"].as_str().unwrap_or("").contains("does not carry"), "{}", r["canvasRefusesPlot"]);
    // fonts
    assert!(r["textWithoutFont"].as_str().unwrap_or("").contains("font"), "a text with no font registered says so: {}", r["textWithoutFont"]);
    assert!(r["facesAdded"].as_u64().unwrap_or(0) >= 1, "the shipped font registered: {}", r["facesAdded"]);
    assert!(!r["notAFont"].as_str().unwrap_or("").is_empty(), "a file that is no font is refused with a sentence");
    assert!(r["inkedPixels"].as_u64().unwrap_or(0) > 20, "the text was lettered with the shipped font: {} pixels", r["inkedPixels"]);
    // DrawImage: the green image is where it was put and nowhere else
    assert_eq!(r["imageInside"].as_str(), Some("16711935"), "DrawImage drew the image at (5,5): {}", r["imageInside"]);
    assert_eq!(r["imageOutside"].as_str(), Some("0"), "and left the rest transparent: {}", r["imageOutside"]);
    // graphics
    assert!(kinds("graphicsKinds").contains("Plot") && kinds("graphicsKinds").contains("Physics2D"), "{}", kinds("graphicsKinds"));
    assert!(!kinds("graphicsKinds").contains("Scene3D"), "the graphics part carries no 3D: {}", kinds("graphicsKinds"));
    assert_eq!(r["seriesCount"], "1");
    assert!(r["svg"].as_str().unwrap_or("").contains("<svg"), "a plot renders to SVG in the part: {}", r["svg"]);
    assert_eq!(r["worlds"], "1");
    assert!(r["graphicsRefusesScene"].as_str().unwrap_or("").contains("does not carry"), "{}", r["graphicsRefusesScene"]);
    assert_eq!(r["afterRelease"], 1, "releasing the plot gives its object back (the Physics2D control's remains)");
    // graphics3d
    assert!(kinds("kinds3d").contains("Scene3D") && kinds("kinds3d").contains("Canvas3D") && kinds("kinds3d").contains("Plot"), "{}", kinds("kinds3d"));
    let three_d = if r["noGpu"].is_string() {
        // AC5: no WebGPU and no WebGL2 is a readable message.
        let why = r["noGpu"].as_str().unwrap();
        assert!(!why.trim().is_empty(), "the message is readable");
        format!("no graphics device in this browser, said: {why}")
    } else {
        assert!(matches!(r["connect"].as_str(), Some(b) if b != "None"), "the Canvas3D connected to {}", r["connect"]);
        assert_eq!((r["picW"].as_u64(), r["picH"].as_u64()), (Some(64), Some(48)));
        assert_eq!(r["picBytes"].as_u64(), Some(64 * 48 * 4), "the copy is the whole picture");
        assert!(r["picLit"].as_u64().unwrap_or(0) > 100, "a lit box was rendered: {} lit pixels", r["picLit"]);
        assert!(r["picDistinct"].as_u64().unwrap_or(0) >= 2, "the box differs from the background (a face seen straight on is one colour): {} distinct", r["picDistinct"]);
        format!("backend {}, {} draw call(s), {} lit pixels of {}, {} distinct colours", r["backend"], r["draws"], r["picLit"], 64 * 48, r["picDistinct"])
    };
    println!(
        "T26/T27 parts in Chrome: canvas [{}] GetPixel {} · DrawImage {} · font: {} face(s), {} inked pixels · graphics [{}] 1 series, SVG {} bytes · graphics3d [{}] {three_d} · {} ms",
        kinds("canvasKinds"),
        r["pixel"],
        r["imageInside"],
        r["facesAdded"],
        r["inkedPixels"],
        kinds("graphicsKinds"),
        r["svg"].as_str().map_or(0, str::len),
        kinds("kinds3d"),
        started.elapsed().as_millis()
    );
}
