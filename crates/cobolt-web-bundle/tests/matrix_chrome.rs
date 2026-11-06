// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The R20 matrix in a real browser** (WASM-015 T13b; WASM-017 R20, AC1, AC2, AC6, AC8, AC10, AC13).
//!
//! `e2e_node` runs the scenario project in every cell of *program variant × player × execution mode* under
//! `node`. This runs the same project, built by the product's own Web Build (`GeneratorBackend`), in the same
//! eight cells in headless Chrome, served cross-origin isolated, so that the threaded player and the Worker are
//! the browser's own. A cell is chosen the way a developer would choose it: by what the Build's `config.json`
//! offers (`program` without the multi-memory module, `options.threads` and `options.worker` set to `off`), not
//! by a test-only door.
//!
//! A page raises its own events, so the test acts as a user: it presses the mouse on the buttons and the tree,
//! types into the text box, and reads what the page shows. Each cell opens a page of its own and checks that
//!
//! * the loader chose the cell it was asked for, and fetched one program module and one player at start;
//! * a DataGrid sort on 50 000 rows reports the thread count of the player chosen (more than one for the threaded);
//! * the handlers answer: UTF-8 out, a tree click's payload in, text typed by the user read by a query that follows
//!   the handler's own write, 1000 writes in one flush, a long loop that yields (the longest gap the main thread
//!   goes without a turn is measured), a trap and a COBOL error reported and survived;
//! * a form opened and closed 100 times leaves the module's live instances and, where the page can read it, its
//!   region in use at the baseline; the form's instance limit refuses the next open with -3 and the form's name;
//! * the player's own memory grows (a 40 MB text), and reads are still right afterwards;
//! * unmounting closes every instance.
//!
//! The remote call (AC4) needs a backend and is the business of `e2e_chrome`. Firefox and Safari are not installed
//! here: their cells are not claimed. With no Chrome on the machine (`CHROME` names one) the test says it was
//! skipped, and why.

mod support;

use std::path::Path;
use std::time::Instant;

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::Isolation;
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome, Page};

/// One cell of the matrix.
#[derive(Clone, Copy)]
struct Cell {
    program: &'static str,
    player: &'static str,
    mode: &'static str,
}

impl Cell {
    fn name(&self) -> String {
        format!("{}/{}/{}", self.program, self.player, self.mode)
    }
}

/// What the scenario evaluates in the page: the application, and the few things a user's eyes and hands need.
const HELPERS: &str = r#"
window.__m = {
  el: document.getElementById('pr-app'),
  app: null,
  async init() { this.app = await PowerRustCOBOL.mount(this.el); return true; },
  ctl(name) { const r = this.el.shadowRoot; return r && r.querySelector(`[data-pr-form="main-form"] [data-name="${name}"]`); },
  label() { const e = this.ctl('LBL-OUT'); return e ? e.textContent.replace(/\s+$/, '') : null; },
  rectOf(e) { if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; },
  rect(name) { return this.rectOf(this.ctl(name)); },
  // The pointer goes to the centre of the control, which must hit the control itself (not something drawn over it).
  hit(name) {
    const e = this.ctl(name); if (!e) return 'no element';
    const b = e.getBoundingClientRect();
    const t = this.el.shadowRoot.elementFromPoint(b.x + b.width / 2, b.y + b.height / 2);
    return t && (t === e || e.contains(t)) ? 'ok' : `covered by ${t ? (t.className || t.tagName) : 'nothing'}`;
  },
  // The row of a tree node, as a small target at the row's right end (the buttons are drawn over the row's left).
  nodeTarget(text) {
    const tree = this.ctl('TRV-ECHO'); if (!tree) return null;
    const all = [...tree.querySelectorAll('*')].filter((n) => n.children.length === 0 && n.textContent.trim() === text);
    if (!all.length) return null;
    let row = all[0];
    while (row.parentElement && row.parentElement !== tree && row.getBoundingClientRect().width < tree.getBoundingClientRect().width - 20) row = row.parentElement;
    const b = row.getBoundingClientRect();
    return { x: b.right - 30, y: b.y, width: 20, height: b.height };
  },
  snap() { return JSON.parse(this.app.player.snapshot()); },
  stats() { return JSON.parse(this.app.player.stats()); },
  errors() { return JSON.parse(this.app.player.errors()); },
  tick: null,
  startTicker() {
    const t = { maxGap: 0, last: performance.now(), running: true, ch: new MessageChannel() };
    // a MessageChannel ticker, which the browser does not clamp to 4 ms the way it clamps nested timers
    t.ch.port1.onmessage = () => { const n = performance.now(); t.maxGap = Math.max(t.maxGap, n - t.last); t.last = n; if (t.running) t.ch.port2.postMessage(0); };
    t.ch.port2.postMessage(0);
    t.y0 = this.stats().yields; t.t0 = performance.now();
    this.tick = t; return true;
  },
  stopTicker() {
    const t = this.tick; t.running = false; t.ch.port1.close(); t.ch.port2.close();
    return { iterations: 10000000, resumes: this.stats().yields - t.y0, longest_main_thread_gap_ms: Math.round(t.maxGap * 100) / 100, loop_ms: Math.round(performance.now() - t.t0) };
  },
};
true
"#;

/// What was chosen, what was fetched, and the Rayon sort of the player chosen (a second copy of it, started
/// here so that its sort can be called).
const CHOICE: &str = r#"(async () => {
  const cell = window.__cell;
  const a = __m.app;
  const fail = (m) => { throw new Error(m); };
  const eq = (x, y, m) => { if (JSON.stringify(x) !== JSON.stringify(y)) fail(`${m}: got ${JSON.stringify(x)}, wanted ${JSON.stringify(y)}`); };
  eq(a.choice.program, cell.program, 'program variant chosen');
  eq(a.choice.player, cell.player, 'player variant chosen');
  eq(a.choice.mode, cell.mode, 'execution mode chosen');
  eq(a.info.variant & 1, cell.program === 'multi' ? 1 : 0, 'pr_variant bit 0');
  eq(self.crossOriginIsolated, true, 'the page is cross-origin isolated');
  const names = performance.getEntriesByType('resource').map((r) => r.name.replace(/^.*\//, ''));
  const programs = names.filter((n) => /^program(\.single)?\.[0-9a-f]{8}\.wasm$/.test(n));
  const players = names.filter((n) => /^player.*\.wasm$/.test(n));
  eq(programs.length, 1, `one program module fetched (${names.join(', ')})`);
  eq(players.length, 1, `one player fetched (${names.join(', ')})`);
  const pv = a.config.player[cell.player];
  const m = await import(new URL(pv.js, location.href).href + '?probe');
  await m.default({ module_or_path: new URL(pv.wasm, location.href).href });
  if (cell.player === 'threaded') await m.initThreadPool(Math.max(1, Math.min(8, navigator.hardwareConcurrency - 1)));
  const sw = JSON.parse(m.sort_workers(50000));
  if (!sw.sorted) fail('the sort left the rows out of order');
  if (cell.player === 'threaded') { if (!(sw.threaded && sw.threads_used > 1)) fail(`the Rayon sort used one thread: ${JSON.stringify(sw)}`); }
  else eq(sw.threads_used, 1, 'the plain player sorts on one thread');
  return { fetched: programs[0].replace(/\.[0-9a-f]{8}\./, '.<hash>.'), requests: names.length, sort: sw };
})()"#;

/// A form opened and closed 100 times; the instance limit; the module's memory.
const DISPOSAL: &str = r#"(async () => {
  const a = __m.app;
  const fail = (m) => { throw new Error(m); };
  const eq = (x, y, m) => { if (JSON.stringify(x) !== JSON.stringify(y)) fail(`${m}: got ${JSON.stringify(x)}, wanted ${JSON.stringify(y)}`); };
  const live0 = await a.link.debugLive();
  const ex = a.link.exports();
  const region0 = ex && typeof ex.pr_dbg_region_used === 'function' ? ex.pr_dbg_region_used() : null;
  const inst0 = __m.stats().instances;
  const t = performance.now();
  for (let i = 0; i < 100; i++) {
    a.player.open_form('child-form');
    await a.settled();
    const inst = Math.max(...Object.keys(__m.snap()).map(Number));
    eq(__m.snap()[String(inst)].controls['LBL-CHILD'].caption, 'child', `child ${i} loaded`);
    a.player.close_instance(inst);
    await a.settled();
  }
  const ms = Math.round(performance.now() - t);
  eq(__m.stats().instances, inst0, 'host instances back to baseline');
  eq(await a.link.debugLive(), live0, 'the module\'s live instances back to baseline');
  const out = { cycles: 100, ms, live_before: live0, live_after: await a.link.debugLive(), region_used: 'not readable from the page (the module lives in a Worker)' };
  if (region0 !== null) {
    eq(ex.pr_dbg_region_used(), region0, 'the module\'s region in use is back to baseline');
    out.region_used = region0;
  }
  // the form allows 4 at once; the fifth is refused with -3 and the form's name
  const opened = [];
  for (let i = 0; i < 4; i++) {
    a.player.open_form('child-form');
    await a.settled();
    opened.push(Math.max(...Object.keys(__m.snap()).map(Number)));
  }
  const seen = __m.errors().length;
  a.player.open_form('child-form');
  await a.settled();
  const refused = __m.errors().slice(seen);
  if (!refused.some((e) => e.code === -3 && /child-form/i.test(JSON.stringify(e)))) fail('the one-too-many open is refused with -3 and the form name: ' + JSON.stringify(refused));
  for (const i of opened) { a.player.close_instance(i); await a.settled(); }
  eq(await a.link.debugLive(), live0, 'the instances are back to baseline after the refusal');
  out.exhaustion = { allowed: 4, refused_code: refused[0].code };
  if (ex) out.module_memory = { data_bytes: ex.memory.buffer.byteLength, arena_bytes: ex.arena.buffer.byteLength };
  return out;
})()"#;

/// The text box takes 40 MB at once (a paste): the player's memory grows. Answers the sizes.
const PASTE_BIG: &str = r#"(() => {
  const input = __m.ctl('TXT-IN').matches('input,textarea') ? __m.ctl('TXT-IN') : __m.ctl('TXT-IN').querySelector('input,textarea');
  const before = __m.app.playerMemory.buffer.byteLength;
  input.value = 'x'.repeat(40 * 1024 * 1024);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  const after = __m.app.playerMemory.buffer.byteLength;
  input.value = 'depois do crescimento ✓ 日本';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  return { before, after_growth: after };
})()"#;

/// Unmount: every instance is closed in the module.
const UNMOUNT: &str = r#"(async () => {
  const a = __m.app;
  const open = Object.keys(__m.snap()).length;
  const exBefore = a.link.exports();
  const summary = JSON.parse(await a.unmount());
  if (summary.closed !== open) throw new Error(`one pr_close_form per open instance at unmount: closed ${summary.closed}, open ${open}`);
  if (exBefore && exBefore.pr_dbg_live() !== 0) throw new Error('the module holds an instance after unmount');
  return { instances_closed: summary.closed, cancelled_tasks: summary.cancelled };
})()"#;

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// A user's click on a control: the pointer goes to its centre, which must hit the control itself.
fn click(p: &mut Page, name: &str) -> Result<(), String> {
    let hit = p.eval(&format!("__m.hit({})", q(name)))?;
    if hit != json!("ok") {
        return Err(format!("{name} cannot be clicked: {}", hit.as_str().unwrap_or("?")));
    }
    p.click_at(&format!("__m.rect({})", q(name)))
}

/// What the page shows in the main form's label, once it is `want`.
fn shows(p: &mut Page, want: &str, secs: u64) -> Result<(), String> {
    p.wait_for(&format!("__m.label() === {}", q(want)), secs).map(|_| ()).map_err(|e| format!("{e}; the label shows {:?}", p.eval("__m.label()").unwrap_or(Value::Null)))
}

/// Press a button and wait for the label.
fn press(p: &mut Page, button: &str, want: &str) -> Result<(), String> {
    click(p, button)?;
    shows(p, want, 20).map_err(|e| format!("after {button}: {e}"))
}

/// Rewrite the Build's `config.json` so that the loader's own choice is `cell`.
fn configure(dir: &Path, base: &Value, cell: Cell) -> Result<(), String> {
    let mut config = base.clone();
    if cell.program == "single" {
        config["program"].as_object_mut().ok_or("config.json has no `program`")?.remove("multi");
    }
    let options = config.as_object_mut().ok_or("config.json is not an object")?.entry("options").or_insert_with(|| json!({}));
    if cell.player == "plain" {
        options["threads"] = json!("off");
    }
    if cell.mode == "main" {
        options["worker"] = json!("off");
    }
    std::fs::write(dir.join("config.json"), serde_json::to_vec_pretty(&config).unwrap()).map_err(|e| e.to_string())
}

/// The scenario of one cell, as a user would go through it. Answers the line the report prints.
fn run_cell(chrome: &Chrome, url: &str, cell: Cell) -> Result<String, String> {
    const GREETING: &str = "Olá, mundo! 日本語 😀";
    let mut p = chrome.new_page()?;
    p.call("Network.enable", json!({}))?;
    p.call("Network.setCacheDisabled", json!({"cacheDisabled": true}))?;
    p.resize(1024, 768)?;
    p.navigate(url)?;
    p.eval(&format!("window.__cell = {{program: '{}', player: '{}', mode: '{}'}}; true", cell.program, cell.player, cell.mode))?;
    p.eval(HELPERS)?;
    p.eval("__m.init()")?;

    let choice = p.eval(CHOICE)?;
    p.wait_for("__m.label() === 'loaded'", 30).map_err(|e| format!("the main form did not load: {e}"))?;

    // UTF-8 out of the module, DISPLAY to the console
    press(&mut p, "BTN-GO", GREETING)?;
    if p.eval("__m.app.player.display_log().includes('go')")? != json!(true) {
        return Err("DISPLAY did not reach the console".into());
    }
    // a click on a tree node: its payload (the node's text) reaches the handler
    let target = p.eval("__m.nodeTarget('Node 1')")?;
    if target.is_null() {
        return Err("the tree has no node `Node 1` in the page".into());
    }
    p.click_at("__m.nodeTarget('Node 1')")?;
    shows(&mut p, "Node 1", 20).map_err(|e| {
        let seen = p.eval("JSON.stringify({ errors: __m.errors(), display: __m.app.player.display_log(), target: __m.nodeTarget('Node 1'), tree: __m.ctl('TRV-ECHO').innerHTML.slice(0, 600) })").unwrap_or(Value::Null);
        format!("after a click on the tree node: {e}; {seen}")
    })?;
    // text typed by the user, read by a query
    click(&mut p, "TXT-IN")?;
    p.type_text("digitado ação 日本")?;
    press(&mut p, "BTN-QUERY", "digitado ação 日本")?;
    press(&mut p, "BTN-RAW", "written first")?;

    // 1000 writes, one host call
    let b0 = p.eval("__m.stats()")?;
    press(&mut p, "BTN-BURST", "x")?;
    let b1 = p.eval("__m.stats()")?;
    let writes = b1["sets"].as_i64().unwrap_or(0) - b0["sets"].as_i64().unwrap_or(0);
    let flushes = b1["flushes"].as_i64().unwrap_or(0) - b0["flushes"].as_i64().unwrap_or(0);
    if writes != 1000 || flushes != 1 {
        return Err(format!("1000 writes made {writes} commands and {flushes} flushes"));
    }

    // the long loop: the main thread is never held for long
    p.eval("__m.startTicker()")?;
    click(&mut p, "BTN-YIELD")?;
    shows(&mut p, "spun", 120).map_err(|e| format!("the loop did not finish: {e}"))?;
    let y = p.eval("__m.stopTicker()")?;
    let gap = y["longest_main_thread_gap_ms"].as_f64().unwrap_or(f64::MAX);
    if gap >= 50.0 {
        return Err(format!("the main thread was held for {gap} ms: {y}"));
    }

    // a trap and a COBOL error are reported and the application goes on
    click(&mut p, "BTN-TRAP")?;
    p.wait_for("__m.errors().some((e) => e.code === 1100 || e.code === -10)", 20).map_err(|e| format!("the trap was not reported: {e}; {}", p.eval("JSON.stringify(__m.errors())").unwrap_or(Value::Null)))?;
    press(&mut p, "BTN-GO", GREETING).map_err(|e| format!("the application does not answer after a trap: {e}"))?;
    click(&mut p, "BTN-ERR")?;
    p.wait_for("__m.errors().some((e) => e.code === 1100)", 20).map_err(|e| format!("the COBOL error was not reported with its number: {e}"))?;

    // 100 forms opened and closed; the instance limit; fixed memory
    let disposal = p.eval(DISPOSAL)?;

    // the player's memory grows when the text box takes 40 MB, and the next read is right
    let memory = p.eval(PASTE_BIG)?;
    if memory["after_growth"].as_i64() <= memory["before"].as_i64() {
        return Err(format!("the player memory did not grow: {memory}"));
    }
    press(&mut p, "BTN-QUERY", "depois do crescimento ✓ 日本").map_err(|e| format!("a read after the player's memory grew: {e}"))?;

    let unmount = p.eval(UNMOUNT)?;
    if !p.exceptions.is_empty() {
        return Err(format!("uncaught exceptions in the page: {:?}", p.exceptions));
    }
    Ok(format!(
        "fetched {} | burst 1000 writes -> {flushes} flush | yield 10000000 iterations, {} resumes, longest gap {gap} ms, loop {} ms | 100 cycles in {} ms, live {} -> {}, region {} | 4 allowed, 5th refused {} | player memory {} -> {} | sort {} threads of {} rows | unmount closed {}",
        choice["fetched"],
        y["resumes"],
        y["loop_ms"],
        disposal["ms"],
        disposal["live_before"],
        disposal["live_after"],
        disposal["region_used"],
        disposal["exhaustion"]["refused_code"],
        memory["before"],
        memory["after_growth"],
        choice["sort"]["threads_used"],
        50000,
        unmount["instances_closed"],
    ))
}

#[test]
fn the_scenario_project_runs_in_every_cell_of_the_matrix_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));

    // The scenario project, built by the product's own Web Build with the shipped players.
    let project = support::scenario_project("matrix");
    let req = WebBuildRequest::from_manifest(&project.path().join("Core.project.toml")).unwrap();
    let built = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("the scenario project does not build: {e}"));
    let base: Value = serde_json::from_slice(&std::fs::read(built.bundle_dir.join("config.json")).unwrap()).unwrap();
    assert!(base["player"]["threaded"].is_object() && base["player"]["plain"].is_object(), "the Build ships both players: {}", base["player"]);
    assert!(base["program"]["multi"]["url"].is_string() && base["program"]["single"]["url"].is_string(), "the Build ships both program modules: {}", base["program"]);
    let server = serve(&built.bundle_dir, "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let url = format!("{}/index.html", server.url());

    let mut lines = Vec::new();
    let mut failed = Vec::new();
    let started = Instant::now();
    for program in ["multi", "single"] {
        for player in ["plain", "threaded"] {
            for mode in ["main", "worker"] {
                let cell = Cell { program, player, mode };
                // `PRC_MATRIX_ONLY=single/threaded/worker` runs one cell, to look at it.
                if std::env::var("PRC_MATRIX_ONLY").is_ok_and(|only| only != cell.name()) {
                    continue;
                }
                let t = Instant::now();
                let r = configure(&built.bundle_dir, &base, cell).and_then(|()| run_cell(&chrome, &url, cell));
                match r {
                    Ok(o) => lines.push(format!("  pass  {:<22} {:>6} ms  {o}", cell.name(), t.elapsed().as_millis())),
                    Err(e) => {
                        lines.push(format!("  FAIL  {:<22} {e}", cell.name()));
                        failed.push(format!("{}: {e}", cell.name()));
                    }
                }
            }
        }
    }
    server.stop();
    println!("Chrome ({}): the scenario project in 8 cells (program x player x mode), {} passed, {} failed, {} ms", exe.display(), 8 - failed.len(), failed.len(), started.elapsed().as_millis());
    for l in &lines {
        println!("{l}");
    }
    assert!(failed.is_empty(), "failed in Chrome:\n{}", failed.join("\n"));
}
