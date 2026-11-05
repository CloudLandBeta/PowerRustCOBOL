// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `node` side of the end-to-end tests, as a string the Rust test writes to a temporary folder
//! (outside the repository) and runs. It plays the browser: it provides the loader with an
//! environment (`fetch` over the bundle folder that checks Subresource Integrity exactly as a
//! browser does, `Worker` over `worker_threads`, `WebAssembly`, the probes' answers) and then drives
//! the REAL loader, the REAL player build and the fixture program module through the scenarios of
//! WASM-017 R20, printing one JSON report that the Rust test asserts on.

pub const HARNESS: &str = r####"
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { Worker as NodeWorker } from 'node:worker_threads';
import { pathToFileURL, fileURLToPath } from 'node:url';

let dir = process.argv[2];
const shimPath = process.argv[3];
const BASE = 'http://pr.test/';
const results = {};
const metrics = {};
const files = fs.readdirSync(dir);
const loaderFile = files.find((f) => /^loader\.[0-9a-f]{8}\.js$/.test(f));
const { mount, probe, choose, LoadError, defaultTexts } = await import(pathToFileURL(path.join(dir, loaderFile)).href);
let config = JSON.parse(fs.readFileSync(path.join(dir, 'config.json'), 'utf8'));
const fixtureDir = dir;
const fixtureConfig = config;
// The bundle the product's own Web Build wrote for the scenario project (WASM-017 T11), when the test built one.
const generatedDir = process.argv[5] || null;
const MIME = { wasm: 'application/wasm', js: 'text/javascript', json: 'application/json', css: 'text/css', cfrm: 'application/xml' };

function check(cond, msg) {
  if (!cond) throw new Error(msg);
}
function eq(a, b, msg) {
  if (a !== b) throw new Error(`${msg}: expected ${JSON.stringify(b)}, got ${JSON.stringify(a)}`);
}
const withTimeout = (p, ms, what) =>
  Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(`timed out after ${ms} ms: ${what}`)), ms))]);
setTimeout(() => {
  console.error('WATCHDOG: the harness took longer than 240 s');
  process.exit(3);
}, 240000).unref();

async function scenario(name, fn) {
  try {
    const detail = await withTimeout(fn(), 45000, name);
    results[name] = { pass: true };
    if (detail !== undefined) metrics[name] = detail;
  } catch (e) {
    results[name] = { pass: false, error: String(e && e.stack ? e.stack : e) + (e && e.detail ? " | detail: " + e.detail : "") };
  }
}

// ── the environment ──────────────────────────────────────────────────────────

class WebWorkerLike {
  constructor(url) {
    const rel = new URL(typeof url === 'string' ? url : url.toString()).pathname.slice(1);
    this.w = new NodeWorker(shimPath, { workerData: { script: path.join(dir, rel) } });
    this.terminated = false;
    WebWorkerLike.live.add(this);
  }
  set onmessage(fn) {
    this.w.removeAllListeners('message');
    if (fn) this.w.on('message', (data) => fn({ data }));
  }
  set onerror(fn) {
    this.w.removeAllListeners('error');
    if (fn) this.w.on('error', (e) => fn({ message: e.message }));
  }
  postMessage(m, t) {
    this.w.postMessage(m, t);
  }
  terminate() {
    this.terminated = true;
    WebWorkerLike.live.delete(this);
    this.w.terminate();
  }
}
WebWorkerLike.live = new Set();

// The global `Worker` the threaded player's Rayon glue (wasm-bindgen-rayon) spawns its pool with: a browser Worker takes a URL and
// speaks `addEventListener`; the pool's helper script is the one under `snippets/`, addressed by a file URL here.
class PoolWorker {
  constructor(url) {
    this.w = new NodeWorker(shimPath, { workerData: { script: fileURLToPath(url) } });
    this.handlers = new Map();
    PoolWorker.spawned++;
  }
  addEventListener(type, fn) {
    if (type !== 'message') return;
    const h = (data) => fn({ data });
    this.handlers.set(fn, h);
    this.w.on('message', h);
  }
  removeEventListener(type, fn) {
    const h = this.handlers.get(fn);
    if (h) this.w.off('message', h);
  }
  postMessage(m, t) {
    this.w.postMessage(m, t);
  }
  terminate() {
    this.w.terminate();
  }
}
PoolWorker.spawned = 0;

function makeEnv(o = {}) {
  const hits = o.hits || [];
  const fetch = async (url, init = {}) => {
    const rel = new URL(url).pathname.slice(1);
    hits.push(rel);
    const file = path.join(dir, rel);
    if (o.replace && o.replace(rel)) {
      return new Response(o.replace(rel), { status: 200, headers: { 'content-type': MIME[rel.split('.').pop()] || 'application/octet-stream' } });
    }
    if (!fs.existsSync(file)) return new Response('not found', { status: 404 });
    let bytes = fs.readFileSync(file);
    if (o.tamper && o.tamper(rel)) {
      bytes = Buffer.from(bytes);
      bytes[bytes.length - 1] ^= 1;
    }
    if (init.integrity) {
      const [alg, b64] = init.integrity.split('-');
      const h = crypto.createHash(alg).update(bytes).digest('base64');
      if (h !== b64) throw new TypeError(`integrity check failed for ${rel}`);
    }
    return new Response(bytes, { status: 200, headers: { 'content-type': MIME[rel.split('.').pop()] || 'application/octet-stream' } });
  };
  const WA = o.validate
    ? new Proxy(WebAssembly, { get: (t, k) => (k === 'validate' ? o.validate : Reflect.get(t, k)) })
    : o.noWebAssembly
      ? undefined
      : WebAssembly;
  return {
    WebAssembly: WA,
    fetch,
    TextEncoder,
    TextDecoder,
    SharedArrayBuffer,
    Atomics,
    Worker: o.noWorker ? undefined : WebWorkerLike,
    crossOriginIsolated: !!o.isolated,
    navigator: { hardwareConcurrency: o.cores || 4, language: 'en' },
    performance,
    setTimeout,
    location: { href: BASE + 'index.html' + (o.search || ''), search: o.search || '', origin: 'http://pr.test' },
    history: o.history,
  };
}

// The largest slice measured with budget_blocks 20 000 (WASM-002 T8), until the operator answers Q2's first half.
const YIELD_BUDGET_MS = 6.9;
let importCount = 0;
let lastPlayerModule = null;
// A threaded player builds Rayon's global pool once per module instance, so each scenario imports its own copy.
const importPlayer = (u) => {
  const href = pathToFileURL(path.join(dir, new URL(u).pathname.slice(1))).href;
  return import(/threaded-/.test(href) ? `${href}?run=${++importCount}` : href).then((m) => {
    if (/\/player\.js/.test(href)) lastPlayerModule = m;
    return m;
  });
};

async function boot(o = {}) {
  const env = makeEnv(o.env || {});
  const app = await mount(null, { env, config: o.config || config, baseUrl: BASE, headless: true, importPlayer, overrides: o.overrides || {} });
  await app.settled();
  return app;
}

const snap = (app) => JSON.parse(app.player.snapshot());
const caption = (app, inst = 0, ctl = 'LBL-OUT') => snap(app)[String(inst)].controls[ctl].caption;
const stats = (app) => JSON.parse(app.player.stats());
const click = async (app, ctl, args = []) => {
  const bound = app.player.dispatch(0, ctl, 'onClick', args);
  await app.settled();
  return bound;
};
const GREETING = 'Olá, mundo! 日本語 😀';

// ── the core scenarios, once per (program variant × execution mode) ──────────

let growthProven = false;

// What differs between the fixture (hand-assembled instructions) and the module the generator writes from the
// scenario project: how the same observable is raised and how it is read back. Everything else in the script is shared.
const FIXTURE = {
  label: 'fixture',
  get dir() { return fixtureDir; },
  get config() { return fixtureConfig; },
  echo: (app, sent) => app.player.dispatch(0, 'BTN-ECHO', 'onClick', [sent]),
  remoteAnswer: () => new TextEncoder().encode('pong ação 日本'),
  yields: 30,
  spins: 3000000,
  trapCheck: (errs) => errs.some((e) => e.code === -10),
  errCheck: (errs) => errs.some((e) => e.code === 1001 && e.message === 'subscript out of range'),
};
const GENERATED = generatedDir && {
  label: 'generated',
  dir: generatedDir,
  config: JSON.parse(fs.readFileSync(path.join(generatedDir, 'config.json'), 'utf8')),
  // the tree's node click carries the node as its payload: text, index, level, checked (TAB-separated)
  echo: (app, sent) => app.player.dispatch(0, 'TRV-ECHO', 'onNodeClick', [sent + '\t1\t1\t0']),
  remoteAnswer: (p) => new TextEncoder().encode(JSON.stringify({ id: p.id, result: { args: [{ 'PONG-TEXT': 'pong ação 日本' }] } })),
  yields: 0,
  spins: 10000000,
  instances: 4,
  // a COBOL run-time error ends the run with the ERROR command first (code 1000 + the routine's number)
  trapCheck: (errs) => errs.some((e) => e.code === 1100 || e.code === -10),
  errCheck: (errs) => errs.some((e) => e.code === 1100),
};

async function core(program, mode, src = FIXTURE, player = 'plain') {
  dir = src.dir;
  config = src.config;
  const savedWorker = globalThis.Worker;
  // The threaded player starts its Rayon pool with the page's global `Worker`.
  const hadSelf = 'self' in globalThis;
  let addedListen = false;
  if (player === 'threaded') {
    globalThis.Worker = PoolWorker;
    if (!hadSelf) globalThis.self = globalThis;
    // In a page `self` is the window, which can listen; the pool's helper script listens on it too.
    if (!globalThis.addEventListener) {
      globalThis.addEventListener = () => {};
      globalThis.removeEventListener = () => {};
      addedListen = true;
    }
  }
  try {
    return await coreBody(program, mode, src, player);
  } finally {
    globalThis.Worker = savedWorker;
    if (!hadSelf) delete globalThis.self;
    if (addedListen) { delete globalThis.addEventListener; delete globalThis.removeEventListener; }
    dir = fixtureDir;
    config = fixtureConfig;
  }
}

async function coreBody(program, mode, src, player) {
  const out = {};
  const hits = [];
  const workersBefore = WebWorkerLike.live.size;
  const poolBefore = PoolWorker.spawned;
  const app = await boot({ overrides: { program, mode, player }, env: { isolated: mode === 'worker' || player === 'threaded', hits } });
  eq(app.choice.player, player, 'player variant chosen');
  // Workers = max(1, min(8, hardwareConcurrency - 1)) (WASM-017 Q2, decided); the page here reports 4 cores.
  out.pool_workers = PoolWorker.spawned - poolBefore;
  eq(out.pool_workers, player === 'threaded' ? 3 : 0, 'the Rayon pool size: the plain player starts none, the threaded one 3 on 4 cores');
  // A DataGrid sort on 50 000 rows: how many threads compared rows (WASM-017 T12: "a Rayon sort reports > 1 worker").
  const sw = JSON.parse(lastPlayerModule.sort_workers(50000));
  out.sort = sw;
  check(sw.sorted, 'the sort left the rows in order');
  if (player === 'threaded') {
    eq(sw.pool, 3, 'the pool the sort used');
    check(sw.threaded && sw.threads_used > 1, `the Rayon sort used more than one thread: ${sw.threads_used}`);
  } else {
    eq(sw.threads_used, 1, 'the plain player sorts on one thread');
  }
  out.ran = { player, program, execution: mode === 'worker' ? 'Worker' : 'main thread' };
  eq(app.choice.program, program, 'program variant chosen');
  eq(app.choice.mode, mode, 'execution mode chosen');
  eq(app.info.variant & 1, program === 'multi' ? 1 : 0, 'pr_variant bit 0');

  // loading: one program module, fetched once, at start; one player; one manifest
  eq(hits.filter((h) => /^program(\.single)?\.[0-9a-f]{8}\.wasm$/.test(h)).length, 1, 'program fetched once');
  eq(hits.filter((h) => h.endsWith('player_bg.wasm')).length, 1, 'player wasm fetched once');
  out.fetches = hits.length;
  out.fetched = hits.map((h) => h.replace(/\.[0-9a-f]{8}\./, '.<hash>.'));

  eq(caption(app), 'loaded', 'the main form opened and its onLoad wrote the caption');

  // a click, UTF-8 out
  check(await click(app, 'BTN-GO'), 'BTN-GO has a handler');
  eq(caption(app), GREETING, 'UTF-8 of 1-4 bytes out of the module');
  check(app.player.display_log().includes('go'), 'DISPLAY reached the console');

  // UTF-8 in (event argument) and out again
  const sent = 'Ação 日本語 😀 — ñ';
  const echoBound = src.echo(app, sent);
  await app.settled();
  eq(caption(app), sent, `UTF-8 argument round trip (handler bound: ${echoBound}; errors: ${app.player.errors()}; display: ${app.player.display_log()})`);

  // a query, and a read that reflects the handler's own earlier write
  app.player.type_into(0, 'TXT-IN', 'Text', 'digitado ação 日本');
  await click(app, 'BTN-QUERY');
  eq(caption(app), 'digitado ação 日本', 'query answer');
  await click(app, 'BTN-RAW');
  eq(caption(app), 'written first', 'read after write in one handler');

  // batching: 1000 writes, one host call
  const b0 = stats(app);
  await click(app, 'BTN-BURST');
  const b1 = stats(app);
  eq(b1.sets - b0.sets, 1000, 'commands applied');
  eq(b1.flushes - b0.flushes, 1, 'flushes for 1000 writes');
  out.burst = { writes: 1000, flushes: b1.flushes - b0.flushes, queries: b1.queries - b0.queries, bytes: b1.bytes_in - b0.bytes_in };

  // a remote request: the module suspends; the answer resumes it
  await click(app, 'BTN-REMOTE');
  const pending = JSON.parse(app.player.pending_requests());
  eq(pending.length, 1, 'one request in the outbox');
  check(pending[0].frame.includes('PING'), 'the frame is the module\'s');
  eq(caption(app), 'calling', 'suspended with the caption written before the call');
  app.player.deliver_remote(pending[0].id, src.remoteAnswer(pending[0]));
  await app.settled();
  eq(caption(app), 'pong ação 日本', 'the answer frame reached the module byte for byte');

  // yield: a very long loop; how long does the main thread go without a turn?
  let maxGap = 0;
  let last = performance.now();
  let running = true;
  const ticker = (async () => {
    while (running) {
      await new Promise((r) => setTimeout(r, 0));
      const n = performance.now();
      maxGap = Math.max(maxGap, n - last);
      last = n;
    }
  })();
  const y0 = stats(app);
  const t0 = performance.now();
  await click(app, 'BTN-YIELD');
  const loopMs = performance.now() - t0;
  running = false;
  await ticker;
  eq(caption(app), 'spun', 'the loop finished');
  const y1 = stats(app);
  out.yield = { iterations: src.spins, resumes: y1.yields - y0.yields, longest_main_thread_gap_ms: Math.round(maxGap * 100) / 100, loop_ms: Math.round(loopMs) };
  if (mode === 'main' && src.yields) eq(y1.yields - y0.yields, src.yields, 'the main-thread host saw each yield');
  check(maxGap < 50, `the main thread was never blocked for ${maxGap} ms`);
  // the frame deadline: one 16 ms frame plus the slice the module may run (YIELD_BUDGET_MS, WASM-017 Q2 first half)
  check(maxGap < 16 + YIELD_BUDGET_MS, `every gap is under the deadline: ${maxGap} ms against ${16 + YIELD_BUDGET_MS} ms`);
  out.yield.deadline_ms = 16 + YIELD_BUDGET_MS;

  // a trap is reported and the application goes on
  await click(app, 'BTN-TRAP');
  let errs = JSON.parse(app.player.errors());
  check(src.trapCheck(errs), 'the trap was reported: ' + JSON.stringify(errs));
  await click(app, 'BTN-GO');
  eq(caption(app), GREETING, 'the application answers after a trap');
  await click(app, 'BTN-ERR');
  errs = JSON.parse(app.player.errors());
  check(src.errCheck(errs), 'the COBOL error was reported with its number: ' + JSON.stringify(errs));

  // disposal: open and close a form 100 times
  const live0 = await app.link.debugLive();
  const exD = app.link.exports();
  const region0 = exD && typeof exD.pr_dbg_region_used === 'function' ? exD.pr_dbg_region_used() : null;
  const inst0 = stats(app).instances;
  for (let i = 0; i < 100; i++) {
    app.player.open_form('child-form');
    await app.settled();
    const ids = Object.keys(snap(app)).map(Number);
    const inst = Math.max(...ids);
    eq(snap(app)[String(inst)].controls['LBL-CHILD'].caption, 'child', `child ${i} loaded`);
    app.player.close_instance(inst);
    await app.settled();
  }
  eq(stats(app).instances, inst0, 'host instances back to baseline');
  eq(await app.link.debugLive(), live0, 'the module\'s live instances back to baseline');
  out.disposal = { cycles: 100, live_before: live0, live_after: await app.link.debugLive() };
  if (region0 !== null) {
    eq(exD.pr_dbg_region_used(), region0, 'the module\'s region in use is back to baseline after 100 open/close cycles');
    out.disposal.region_used = region0;
  }
  if (src.instances) {
    // AC2: the form allows `Instances` at once, and the next open is refused with -3 and the form's name
    const opened = [];
    for (let i = 0; i < src.instances; i++) {
      app.player.open_form('child-form');
      await app.settled();
      opened.push(Math.max(...Object.keys(snap(app)).map(Number)));
    }
    const seen = JSON.parse(app.player.errors()).length;
    app.player.open_form('child-form');
    await app.settled();
    const refused = JSON.parse(app.player.errors()).slice(seen);
    check(refused.some((e) => e.code === -3 && /child-form/i.test(JSON.stringify(e))), 'the one-too-many open is refused with -3 and the form name: ' + JSON.stringify(refused));
    for (const i of opened) {
      app.player.close_instance(i);
      await app.settled();
    }
    eq(await app.link.debugLive(), live0, 'the instances are back to baseline after the refusal');
    out.exhaustion = { allowed: src.instances, refused_code: refused[0].code };
  }

  // fixed memory: the data memory and the arena never changed size
  const ex = app.link.exports();
  if (ex) {
    out.memory = { data_bytes: ex.memory.buffer.byteLength, arena_bytes: ex.arena.buffer.byteLength };
  }

  // the player's own memory may grow; the host's reads stay right
  if (app.playerMemory) {
    const before = app.playerMemory.buffer.byteLength;
    app.player.type_into(0, 'TXT-IN', 'Text', 'x'.repeat(40 * 1024 * 1024));
    const grown = app.playerMemory.buffer.byteLength;
    app.player.type_into(0, 'TXT-IN', 'Text', 'depois do crescimento ✓ 日本');
    await click(app, 'BTN-QUERY');
    eq(caption(app), 'depois do crescimento ✓ 日本', 'reads are right after the player\'s memory grew');
    out.player_memory = { before, after_growth: grown };
    // The player's wasm module is instantiated once per process and shared by every scenario, and a
    // memory never shrinks: the first scenario grows it, a later one may find room already. What
    // must hold in every scenario is that the read above was right; that growth really happened at
    // least once is checked here.
    if (grown > before) growthProven = true;
    check(grown > before || growthProven, 'the player memory did grow');
  }

  // unmount: every instance is closed in the module, the worker is terminated
  const open = Object.keys(snap(app)).length;
  const exBefore = app.link.exports();
  const summary = JSON.parse(await app.unmount());
  eq(summary.closed, open, 'one pr_close_form per open instance at unmount');
  // WASM-017 R14: every listed object is freed — in node the Player is the one the page holds.
  eq(app.player.__wbg_ptr, 0, 'the Player is freed at unmount');
  if (exBefore) eq(exBefore.pr_dbg_live(), 0, 'the module holds no instance after unmount');
  if (mode === 'worker') eq(WebWorkerLike.live.size, workersBefore, 'the worker was terminated');
  out.unmount = { instances_closed: summary.closed, cancelled_tasks: summary.cancelled };
  return out;
}

for (const program of ['multi', 'single']) {
  for (const mode of ['main', 'worker']) {
    await scenario(`core ${program}/${mode}`, () => core(program, mode));
    if (GENERATED) await scenario(`core ${program}/${mode} (generated)`, () => core(program, mode, GENERATED));
    if (GENERATED && process.env.PRC_THREADED !== 'off') await scenario(`core ${program}/${mode} threaded (generated)`, () => core(program, mode, GENERATED, 'threaded'));
  }
}

// The Rayon pool: Workers = max(1, min(8, hardwareConcurrency - 1)) (Q2, decided), with the Worker count read from the pool itself.
if (GENERATED && process.env.PRC_THREADED !== 'off') {
  for (const [cores, want] of [[1, 1], [2, 1], [4, 3], [9, 8], [32, 8]]) {
    await scenario(`pool size: ${cores} cores start ${want} Rayon workers`, async () => {
      dir = GENERATED.dir;
      config = GENERATED.config;
      const savedWorker = globalThis.Worker;
      const hadSelf = 'self' in globalThis;
      globalThis.Worker = PoolWorker;
      if (!hadSelf) globalThis.self = globalThis;
      const hadListen = !!globalThis.addEventListener;
      if (!hadListen) { globalThis.addEventListener = () => {}; globalThis.removeEventListener = () => {}; }
      const before = PoolWorker.spawned;
      try {
        const app = await boot({ overrides: { program: 'multi', mode: 'main', player: 'threaded' }, env: { isolated: true, cores } });
        eq(PoolWorker.spawned - before, want, `${cores} cores`);
        await app.unmount();
        return { cores, workers: PoolWorker.spawned - before };
      } finally {
        globalThis.Worker = savedWorker;
        if (!hadSelf) delete globalThis.self;
        if (!hadListen) { delete globalThis.addEventListener; delete globalThis.removeEventListener; }
        dir = fixtureDir;
        config = fixtureConfig;
      }
    });
  }
}

// ── choosing and probing (WASM-017 R18, R20 baseline/matrix and isolation) ───

await scenario('probe: the baseline', async () => {
  const good = probe(makeEnv());
  check(good.baseline && good.multiMemory, 'node supports the baseline and multi-memory');
  const old = probe(makeEnv({ validate: () => false }));
  check(!old.baseline && old.missing.length > 0, 'a browser that validates nothing is below the baseline');
  const noMulti = probe(makeEnv({ validate: (b) => b.length > 50 }));
  check(noMulti.baseline && !noMulti.multiMemory, 'baseline without multi-memory');
  check(!probe({ ...makeEnv(), WebAssembly: undefined }).baseline, 'no WebAssembly at all');
});

await scenario('choose: each variant by its probe', async () => {
  const withThreads = { ...config, player: { ...config.player, threaded: { js: 'x', wasm: 'y' } } };
  const iso = choose(probe(makeEnv({ isolated: true })), withThreads);
  eq(iso.player, 'threaded', 'isolated page, threaded player available');
  eq(iso.mode, 'worker', 'isolated page runs the module in a Worker');
  const plain = choose(probe(makeEnv({ isolated: false })), withThreads);
  eq(plain.player, 'plain', 'not isolated: plain player');
  eq(plain.mode, 'main', 'not isolated: main thread');
  const single = choose(probe(makeEnv({ validate: (b) => b.length > 50 })), config);
  eq(single.program, 'single', 'no multi-memory: the single-memory module');
  const off = choose(probe(makeEnv({ isolated: true })), { ...withThreads, options: { ...config.options, threads: 'off' } });
  eq(off.player, 'plain', 'threads switched off by the build');
  return { reasons: iso.reasons };
});

await scenario('isolation: with and without, results agree', async () => {
  const a = await boot({ env: { isolated: true }, overrides: { mode: 'worker' } });
  const b = await boot({ env: { isolated: false } });
  eq(a.choice.mode, 'worker', 'isolated => worker');
  eq(b.choice.mode, 'main', 'not isolated => main');
  for (const app of [a, b]) {
    await click(app, 'BTN-GO');
  }
  eq(caption(a), caption(b), 'the same result in both modes');
  await a.unmount();
  await b.unmount();
});

await scenario('below the baseline: a readable message, not a blank page', async () => {
  let err = null;
  try {
    await mount(null, { env: makeEnv({ validate: () => false }), config, baseUrl: BASE, headless: true, importPlayer });
  } catch (e) {
    err = e;
  }
  check(err instanceof LoadError && err.code === 'unsupported', 'LoadError unsupported');
  eq(err.message, defaultTexts.unsupported, 'the message of WASM-002 R15');
});

await scenario('integrity: a tampered file is refused', async () => {
  let err = null;
  try {
    await mount(null, { env: makeEnv({ tamper: (rel) => /^program\.[0-9a-f]{8}\.wasm$/.test(rel) }), config, baseUrl: BASE, headless: true, importPlayer, overrides: { program: 'multi' } });
  } catch (e) {
    err = e;
  }
  check(err instanceof LoadError && err.code === 'fetch' && /integrity/.test(err.message), 'integrity failure reported: ' + (err && err.message));
});

await scenario('a program that asks for an import ABI v1 does not have is refused', async () => {
  const crafted = fs.readFileSync(process.argv[4]);
  let err = null;
  try {
    await mount(null, {
      env: makeEnv({ replace: (rel) => (/^program\.[0-9a-f]{8}\.wasm$/.test(rel) ? crafted : null) }),
      config: { ...config, program: { ...config.program, multi: { ...config.program.multi, integrity: '' } } },
      baseUrl: BASE, headless: true, importPlayer, overrides: { program: 'multi' },
    });
  } catch (e) {
    err = e;
  }
  check(err instanceof LoadError && err.code === 'abi' && /env\.surprise/.test(err.message + err.detail), 'refused with the import named: ' + (err && (err.message + err.detail)));
});

await scenario('loading: nothing is fetched twice', async () => {
  const hits = [];
  const app = await boot({ env: { hits } });
  const counts = {};
  for (const h of hits) counts[h] = (counts[h] || 0) + 1;
  const twice = Object.entries(counts).filter(([, n]) => n > 1);
  check(twice.length === 0, 'fetched more than once: ' + JSON.stringify(twice));
  await app.unmount();
  return { files: hits.length };
});

await scenario('parts: the Mermaid renderer is fetched on first use, once, and not before', async () => {
  const hits = [];
  const app = await boot({ env: { hits } });
  const partHit = (h) => /part-mermaid/.test(h);
  eq(hits.filter(partHit).length, 0, 'nothing of the part is fetched at start: ' + hits.filter(partHit).join(','));
  eq(Object.keys(app.config.player.parts).sort().join(','), 'canvas,graphics,graphics3d,mermaid,pdf', 'the parts are listed in the bundle');
  check(/^sha(256|384|512)-/.test(app.config.player.parts.mermaid.wasm_integrity), 'with an integrity digest');
  const t0 = performance.now();
  const svg = await app.part('mermaid', 'render', 'flowchart LR\n  A[Start] --> B[Review]\n');
  const firstMs = performance.now() - t0;
  check(svg.startsWith('<svg') && svg.includes('Review'), 'the diagram came back as SVG text: ' + svg.slice(0, 80));
  const fetched = hits.filter(partHit);
  // (node imports a script from disk, so only the module is a fetch here; a browser imports both over the network)
  eq(fetched.length, 1, 'the part module is fetched once: ' + fetched.join(','));
  const t1 = performance.now();
  const seq = await app.part('mermaid', 'render', 'sequenceDiagram\n  A->>B: hi\n');
  const secondMs = performance.now() - t1;
  check(seq.includes('<svg'), 'a sequence diagram too');
  eq(hits.filter(partHit).length, 1, 'the second diagram fetched nothing');
  let refused = '';
  try {
    await app.part('mermaid', 'render', 'classDiagram\n  A <|-- B\n');
  } catch (e) {
    refused = String((e && e.message) || e);
  }
  check(refused.includes('not supported'), 'a kind it does not draw says why: ' + refused);
  let noPart = '';
  try {
    await app.part('nothing', 'render', 'x');
  } catch (e) {
    noPart = String((e && e.message) || e);
  }
  check(/does not include/.test(noPart), 'a part the bundle lacks is a readable error: ' + noPart);
  await app.unmount();
  return { first_call_ms: Math.round(firstMs), second_call_ms: Math.round(secondMs), part_fetches: fetched.length };
});

await scenario('parts: the Canvas 2D part runs a display list and answers the picture', async () => {
  const hits = [];
  const app = await boot({ env: { hits } });
  const partHit = (h) => /part-canvas/.test(h);
  eq(hits.filter(partHit).length, 0, 'nothing of the part is fetched at start');
  const mod = await app.partModule('canvas');
  // PRGD v1: SetFillColor 0xFF0000FF, FillRect 0 0 4 4, Present.
  const payload = [];
  const u32 = (v) => payload.push(v & 255, (v >> 8) & 255, (v >> 16) & 255, (v >>> 24) & 255);
  const f32 = (v) => { const b = new Uint8Array(new Float32Array([v]).buffer); payload.push(...b); };
  payload.push(0x30); u32(0xFF0000FF);
  payload.push(0x20); f32(0); f32(0); f32(4); f32(4);
  payload.push(0x05);
  const head = [0x50, 0x52, 0x47, 0x44, 1, 0, 0, 0];
  const tail = []; const t32 = (v) => tail.push(v & 255, (v >> 8) & 255, (v >> 16) & 255, (v >>> 24) & 255);
  t32(3); t32(payload.length);
  const list = new Uint8Array([...head, ...tail, ...payload]);
  const c = new mod.Graphics(); c.make(1, 'Canvas2D', 8, 8, 1);
  const t0 = performance.now();
  eq(c.draw_list(1, list), 3, 'three commands ran');
  const px = c.rgba(1);
  const ms = performance.now() - t0;
  eq(c.device_width(1) + 'x' + c.device_height(1), '8x8', 'device size');
  eq(Array.from(px.slice(0, 4)).join(','), '255,0,0,255', 'red inside the rectangle');
  const last = (7 * 8 + 7) * 4;
  eq(Array.from(px.slice(last, last + 4)).join(','), '0,0,0,0', 'transparent outside');
  let bad = '';
  try { c.draw_list(1, new Uint8Array([1, 2, 3])); } catch (e) { bad = String((e && e.message) || e); }
  check(bad.length > 0, 'a damaged list is refused with a reason: ' + bad);
  c.free();
  await app.unmount();
  return { draw_and_read_ms: +ms.toFixed(2), part_fetches: hits.filter(partHit).length };
});

await scenario('hosted sign-in return: the code reaches the player and leaves the address', async () => {
  const replaced = [];
  const history = { state: null, replaceState: (s, t, url) => replaced.push(url) };
  const app = await boot({ env: { search: '?pr-code=c0de&pr-state=st4te&keep=1', history } });
  eq(app.playerConfig.hosted_code, 'c0de', 'the authorisation code');
  eq(app.playerConfig.hosted_state, 'st4te', 'the state');
  eq(replaced.length, 1, 'the address was rewritten once');
  check(!/pr-code|pr-state|c0de|st4te/.test(replaced[0]), 'the code is out of the address: ' + replaced[0]);
  check(replaced[0].includes('keep=1'), 'what else the address carried stays: ' + replaced[0]);
  await app.unmount();
  // No return, nothing to rewrite.
  const quiet = [];
  const app2 = await boot({ env: { history: { state: null, replaceState: (s, t, url) => quiet.push(url) } } });
  eq(app2.playerConfig.hosted_code, '', 'no code');
  eq(quiet.length, 0, 'the address is left alone');
  await app2.unmount();
  return { rewritten_to: replaced[0] };
});

// Last: a panic ends the player's instance, which every scenario above shares (WASM-017 R15, AC11).
await scenario('panic: a forced panic (test bundle) calls onPanic and raises the error event', async () => {
  const env = makeEnv();
  env.CustomEvent = CustomEvent;
  const target = new EventTarget();
  const events = [];
  target.addEventListener('pr-error', (e) => events.push(e.detail));
  let said = '';
  const app = await mount(target, { env, config, baseUrl: BASE, headless: true, importPlayer, onPanic: (m) => { said = m; } });
  await app.settled();
  const mod = await importPlayer(new URL(config.player[app.choice.player].js, BASE).href);
  // The shipped player has no test hook (R15): run on it, the scenario says so and checks that.
  if (typeof mod.force_panic_for_test !== 'function') {
    await app.unmount();
    return { shipped_player_has_no_hook: true };
  }
  let thrown = '';
  try {
    mod.force_panic_for_test();
  } catch (e) {
    thrown = String((e && e.message) || e);
  }
  check(said.includes('forced panic'), 'onPanic got the message: ' + said);
  eq(events.length, 1, 'one pr-error event');
  eq(events[0].code, 'panic', 'the event says panic');
  return { message: said.slice(0, 60), events: events.length, thrown: thrown.slice(0, 40) };
});

console.log(JSON.stringify({ results, metrics }));
process.exit(0);
"####;

/// The shim a Worker runs in node so that `worker.js` — written for a browser Worker — runs unchanged:
/// `self`, `postMessage` and `onmessage` over `worker_threads`.
pub const WORKER_SHIM: &str = r####"
import { parentPort, workerData } from 'node:worker_threads';
import { pathToFileURL } from 'node:url';
globalThis.self = globalThis;
globalThis.postMessage = (m, t) => parentPort.postMessage(m, t);
// The pool's helper script waits for its first message with `self.addEventListener` and takes it off again.
const listeners = new Map();
globalThis.addEventListener = (type, fn) => {
  if (type !== 'message') return;
  const h = (data) => fn({ data });
  listeners.set(fn, h);
  parentPort.on('message', h);
};
globalThis.removeEventListener = (type, fn) => {
  const h = listeners.get(fn);
  if (h) parentPort.off('message', h);
};
// A browser Worker queues what arrives before its script has run; do the same, or the first
// message (the module handed over for instantiation) would be lost in the import below.
const early = [];
parentPort.on('message', (data) => {
  if (typeof globalThis.onmessage === 'function') globalThis.onmessage({ data });
  else early.push(data);
});
await import(pathToFileURL(workerData.script).href);
for (const data of early.splice(0)) {
  if (typeof globalThis.onmessage === 'function') globalThis.onmessage({ data });
  for (const fn of [...listeners.keys()]) fn({ data });
}
"####;
