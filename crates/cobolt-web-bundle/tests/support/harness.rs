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
import { pathToFileURL } from 'node:url';

const dir = process.argv[2];
const shimPath = process.argv[3];
const BASE = 'http://pr.test/';
const results = {};
const metrics = {};
const files = fs.readdirSync(dir);
const loaderFile = files.find((f) => /^loader\.[0-9a-f]{8}\.js$/.test(f));
const { mount, probe, choose, LoadError, defaultTexts } = await import(pathToFileURL(path.join(dir, loaderFile)).href);
const config = JSON.parse(fs.readFileSync(path.join(dir, 'config.json'), 'utf8'));
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
    results[name] = { pass: false, error: String(e && e.stack ? e.stack : e) };
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
    navigator: { hardwareConcurrency: 4, language: 'en' },
    performance,
    setTimeout,
    location: { href: BASE + 'index.html' + (o.search || ''), search: o.search || '', origin: 'http://pr.test' },
    history: o.history,
  };
}

const importPlayer = (u) => import(pathToFileURL(path.join(dir, new URL(u).pathname.slice(1))).href);

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

async function core(program, mode) {
  const out = {};
  const hits = [];
  const workersBefore = WebWorkerLike.live.size;
  const app = await boot({ overrides: { program, mode }, env: { isolated: mode === 'worker', hits } });
  eq(app.choice.program, program, 'program variant chosen');
  eq(app.choice.mode, mode, 'execution mode chosen');
  eq(app.info.variant & 1, program === 'multi' ? 1 : 0, 'pr_variant bit 0');

  // loading: one program module, fetched once, at start; one player; one manifest
  eq(hits.filter((h) => /^program(\.single)?\.[0-9a-f]{8}\.wasm$/.test(h)).length, 1, 'program fetched once');
  eq(hits.filter((h) => h.endsWith('player_bg.wasm')).length, 1, 'player wasm fetched once');
  out.fetches = hits.length;

  eq(caption(app), 'loaded', 'the main form opened and its onLoad wrote the caption');

  // a click, UTF-8 out
  check(await click(app, 'BTN-GO'), 'BTN-GO has a handler');
  eq(caption(app), GREETING, 'UTF-8 of 1-4 bytes out of the module');
  check(app.player.display_log().includes('go'), 'DISPLAY reached the console');

  // UTF-8 in (event argument) and out again
  const sent = 'Ação 日本語 😀 — ñ';
  app.player.dispatch(0, 'BTN-ECHO', 'onClick', [sent]);
  await app.settled();
  eq(caption(app), sent, 'UTF-8 argument round trip');

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
  app.player.deliver_remote(pending[0].id, new TextEncoder().encode('pong ação 日本'));
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
  out.yield = { iterations: 3000000, resumes: y1.yields - y0.yields, longest_main_thread_gap_ms: Math.round(maxGap * 100) / 100, loop_ms: Math.round(loopMs) };
  if (mode === 'main') eq(y1.yields - y0.yields, 30, 'the main-thread host saw each yield');
  check(maxGap < 50, `the main thread was never blocked for ${maxGap} ms`);

  // a trap is reported and the application goes on
  await click(app, 'BTN-TRAP');
  let errs = JSON.parse(app.player.errors());
  check(errs.some((e) => e.code === -10), 'the trap was reported: ' + JSON.stringify(errs));
  await click(app, 'BTN-GO');
  eq(caption(app), GREETING, 'the application answers after a trap');
  await click(app, 'BTN-ERR');
  errs = JSON.parse(app.player.errors());
  check(errs.some((e) => e.code === 1001 && e.message === 'subscript out of range'), 'the COBOL error was reported with its number');

  // disposal: open and close a form 100 times
  const live0 = await app.link.debugLive();
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
  if (exBefore) eq(exBefore.pr_dbg_live(), 0, 'the module holds no instance after unmount');
  if (mode === 'worker') eq(WebWorkerLike.live.size, workersBefore, 'the worker was terminated');
  out.unmount = { instances_closed: summary.closed, cancelled_tasks: summary.cancelled };
  return out;
}

for (const program of ['multi', 'single']) {
  for (const mode of ['main', 'worker']) {
    await scenario(`core ${program}/${mode}`, () => core(program, mode));
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
// A browser Worker queues what arrives before its script has run; do the same, or the first
// message (the module handed over for instantiation) would be lost in the import below.
const early = [];
parentPort.on('message', (data) => {
  if (typeof globalThis.onmessage === 'function') globalThis.onmessage({ data });
  else early.push(data);
});
await import(pathToFileURL(workerData.script).href);
for (const data of early.splice(0)) globalThis.onmessage({ data });
"####;
