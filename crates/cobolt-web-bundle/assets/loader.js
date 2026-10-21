// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// The PowerRustCOBOL loader (WASM-002 R15/R19, WASM-017 R3/R10/R14/R15/R18/R19).
//
// An ES module with no dependency, no `eval`, no `new Function`, no inline script: it runs under a
// CSP that allows `'wasm-unsafe-eval'` and nothing else. It
//
//   * probes the browser against the declared baseline and chooses, independently, the program
//     variant (multi-memory or single), the player variant (threaded or plain) and the execution
//     mode (Worker or main thread);
//   * fetches the player, the program module and the forms with streaming compilation and the
//     integrity hashes of config.json;
//   * hands the program module its six imports and the player its link to the module;
//   * takes a fresh view of the arena at every use and never keeps one across a call;
//   * disposes everything it exposed when the application unmounts, and shows a readable message
//     (with a retry or reload action) when anything fails — never a blank page.
//
// Nothing here is generated per build: the page's particulars are in config.json.

'use strict';

// ── the probes ───────────────────────────────────────────────────────────────

// WebAssembly 2.0 without SIMD, the baseline of WASM-017 R18: bulk memory, sign extension,
// saturating float-to-int and multi-value in one module.
const BASELINE_PROBE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x00, 0x02, 0x7f, 0x7f, 0x03, 0x02, 0x01, 0x00,
  0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, 0x0a, 0x1b, 0x01, 0x19, 0x00, 0x41, 0x00, 0x41,
  0x00, 0x41, 0x00, 0xfc, 0x0b, 0x00, 0x41, 0x01, 0xc0, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, 0xfc, 0x02, 0x0b,
]);

// Two memories and a load from the second: valid only with the multi-memory proposal.
const MULTI_MEMORY_PROBE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x05,
  0x05, 0x02, 0x00, 0x01, 0x00, 0x01, 0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x41, 0x00,
  0x28, 0x42, 0x01, 0x00, 0x0b,
]);

export const PRODUCTION_IMPORTS = ['flush', 'query', 'remote', 'now', 'random', 'abort'];
export const REQUIRED_EXPORTS = [
  'pr_abi', 'pr_variant', 'pr_arena_base', 'pr_init', 'pr_open_form', 'pr_close_form', 'pr_event', 'pr_resume',
  'pr_cancel', 'pr_tick', 'pr_status',
];
const ABI_VERSION = 1;

// ── errors ───────────────────────────────────────────────────────────────────

export class LoadError extends Error {
  constructor(message, code = 'load', detail = '') {
    super(message);
    this.name = 'LoadError';
    this.code = code;
    this.detail = detail;
  }
}

const DEFAULT_TEXTS = {
  loading: 'Loading the application…',
  stage_check: 'Checking this browser…',
  stage_player: 'Loading the player…',
  stage_program: 'Loading the program…',
  stage_start: 'Starting…',
  unsupported:
    'This browser cannot run the application. It needs WebAssembly with bulk memory, sign-extension, saturating conversions and multiple return values (Chrome 91, Firefox 79, Safari 15 or later).',
  failed: 'The application could not be loaded.',
  retry: 'Try again',
  reload: 'Reload',
  crashed: 'The application stopped because of an internal error.',
  noscript: 'This application needs JavaScript.',
};

// ── probing and choosing ─────────────────────────────────────────────────────

/** What this browser offers. `env` is the global object (replaced in tests). */
export function probe(env = globalThis) {
  const missing = [];
  const W = env.WebAssembly;
  let baseline = false;
  let multiMemory = false;
  if (!W || typeof W.validate !== 'function') {
    missing.push('WebAssembly');
  } else {
    try {
      baseline = W.validate(BASELINE_PROBE);
    } catch (_) {
      baseline = false;
    }
    if (!baseline) missing.push('WebAssembly baseline (bulk memory, sign extension, saturating conversions, multi-value)');
    try {
      multiMemory = baseline && W.validate(MULTI_MEMORY_PROBE);
    } catch (_) {
      multiMemory = false;
    }
  }
  if (typeof env.fetch !== 'function') missing.push('fetch');
  if (typeof env.TextEncoder !== 'function' || typeof env.TextDecoder !== 'function') missing.push('TextEncoder/TextDecoder');
  const isolated = env.crossOriginIsolated === true;
  const shared = typeof env.SharedArrayBuffer === 'function' && typeof env.Atomics === 'object';
  const worker = typeof env.Worker === 'function';
  return {
    baseline: missing.length === 0,
    missing,
    multiMemory,
    crossOriginIsolated: isolated,
    threads: isolated && shared && worker,
    worker: isolated && shared && worker,
    hardwareConcurrency: (env.navigator && env.navigator.hardwareConcurrency) || 4,
  };
}

/** Choose the three variants (WASM-017 R18). `overrides` is for tests and diagnostics. */
export function choose(p, config, overrides = {}) {
  const reasons = [];
  const has = (v) => config.program && config.program[v];
  let program = p.multiMemory ? 'multi' : 'single';
  if (!has(program)) {
    const other = program === 'multi' ? 'single' : 'multi';
    if (has(other) && (other === 'single' || p.multiMemory)) program = other;
  }
  if (overrides.program && has(overrides.program) && (overrides.program === 'single' || p.multiMemory)) program = overrides.program;
  reasons.push(`program module: ${program}-memory (${p.multiMemory ? 'this browser supports multiple memories' : 'this browser has no multi-memory'})`);

  const opts = config.options || {};
  let player = 'plain';
  const canThread = p.threads && config.player && config.player.threaded && opts.threads !== 'off';
  if (canThread) player = 'threaded';
  if (overrides.player === 'plain') player = 'plain';
  if (overrides.player === 'threaded' && config.player && config.player.threaded && p.threads) player = 'threaded';
  reasons.push(`player: ${player} (${p.crossOriginIsolated ? 'the page is cross-origin isolated' : 'the page is not cross-origin isolated'})`);

  let mode = p.worker && opts.worker !== 'off' ? 'worker' : 'main';
  if (overrides.mode === 'main') mode = 'main';
  if (overrides.mode === 'worker' && p.worker) mode = 'worker';
  reasons.push(`execution: ${mode === 'worker' ? 'in a Worker' : 'on the main thread, yielding cooperatively'}`);
  return { program, player, mode, reasons };
}

// ── small helpers ────────────────────────────────────────────────────────────

function resolveUrl(env, rel, base) {
  const b = base || (env.location && env.location.href) || 'http://localhost/';
  return new URL(rel, b).href;
}

async function fetchChecked(env, url, integrity, what) {
  let response;
  try {
    const init = { credentials: 'same-origin' };
    if (integrity) init.integrity = integrity;
    response = await env.fetch(url, init);
  } catch (e) {
    throw new LoadError(`${what} could not be fetched (${e && e.message ? e.message : e}).`, 'fetch', String(url));
  }
  if (!response.ok) throw new LoadError(`${what} could not be fetched (HTTP ${response.status}).`, 'fetch', String(url));
  return response;
}

async function compile(env, response) {
  const W = env.WebAssembly;
  const type = (response.headers && response.headers.get && response.headers.get('content-type')) || '';
  if (typeof W.compileStreaming === 'function' && type.startsWith('application/wasm')) {
    return W.compileStreaming(response);
  }
  // A server that does not send application/wasm: compile from the bytes (slower, but works).
  return W.compile(await response.arrayBuffer());
}

function checkProgramModule(env, module, layout) {
  const W = env.WebAssembly;
  for (const i of W.Module.imports(module)) {
    const allowed = i.module === 'pr' && i.kind === 'function' && PRODUCTION_IMPORTS.includes(i.name);
    if (!allowed) {
      const conformance = i.module === 'pr' && /^file_/.test(i.name);
      throw new LoadError(
        conformance
          ? 'This program was built for conformance testing (it needs file access) and cannot run in a page.'
          : `The program asks the host for \`${i.module}.${i.name}\`, which this player does not provide: rebuild the application with the current IDE.`,
        'abi',
        `${i.module}.${i.name}`,
      );
    }
  }
  const names = new Set(W.Module.exports(module).map((e) => e.name));
  for (const r of REQUIRED_EXPORTS) {
    if (!names.has(r)) throw new LoadError(`The program does not provide \`${r}\`: rebuild the application with the current IDE.`, 'abi', r);
  }
  if (!names.has('arena') || !names.has('memory')) throw new LoadError('The program does not export its memory and arena.', 'abi');
  void layout;
}

function readEmbeddedManifest(env, module) {
  const sections = env.WebAssembly.Module.customSections(module, 'prmanifest');
  if (!sections || sections.length === 0) return null;
  return new env.TextDecoder('utf-8').decode(new Uint8Array(sections[0]));
}

function macrotask(env) {
  return new Promise((resolve) => (env.setTimeout || setTimeout)(resolve, 0));
}

// ── the link to the module, main-thread mode ─────────────────────────────────

/**
 * Calls the exports of the program module on this thread and implements its six imports.
 * `bridge.player` is set once the player exists.
 */
function createMainLink(env, module, manifest, bridge) {
  const L = manifest.limits;
  const batchOff = 0;
  const answerOff = L.batch_size;
  const frameOut = L.batch_size + L.answer_size;
  const frameIn = frameOut + L.frame_size;
  let ex = null;
  let base = 0;
  // A view is taken at EACH use, from the memory object, and dropped at once (WASM-017 R10): the
  // buffer of a memory is replaced when it grows, and a view over the old one is dead.
  const view = (off, len) => new Uint8Array(ex.arena.buffer, base + off, len);
  const imports = {
    pr: {
      flush(len) {
        bridge.player.flush(view(batchOff, len));
      },
      query(len) {
        const answer = bridge.player.query(view(batchOff, len));
        if (answer === undefined || answer === null) return bridge.player.last_error();
        view(answerOff, answer.length).set(answer);
        return answer.length;
      },
      remote(len) {
        return bridge.player.remote(view(frameOut, len));
      },
      now() {
        return Date.now();
      },
      random() {
        return Math.random();
      },
      abort(len) {
        bridge.player.abort(view(batchOff, len));
      },
    },
  };
  return {
    async instantiate() {
      const instance = await env.WebAssembly.instantiate(module, imports);
      ex = instance.exports;
      const abi = ex.pr_abi();
      if (abi !== ABI_VERSION) {
        throw new LoadError(`The program speaks ABI ${abi} and this player speaks ABI ${ABI_VERSION}: rebuild the application with the current IDE.`, 'abi');
      }
      const variant = ex.pr_variant();
      if (variant & 2) throw new LoadError('This program was built for conformance testing and cannot run in a page.', 'abi');
      if (((variant & 1) === 1) !== !!manifest.variant.multi_memory) {
        throw new LoadError('The program module and its manifest disagree about the memory model: rebuild the application.', 'abi');
      }
      base = ex.pr_arena_base();
      if (ex.pr_init() !== 0) throw new LoadError('The program failed to initialise.', 'abi');
      return { abi, variant };
    },
    // perform(kind, a, b, c, d, payload) -> Int32Array — see api.rs
    perform(kind, a, b, c, d, payload) {
      const out = new Int32Array(4);
      try {
        switch (kind) {
          case 0: {
            const inst = ex.pr_open_form(a, b);
            const status = ex.pr_status();
            out[0] = inst;
            out[1] = status;
            out[2] = status === 1 || status === 2 ? ex.pr_tick() : -1;
            break;
          }
          case 1: {
            const raw = ex.pr_close_form(a);
            const status = raw === undefined ? 0 : raw; // a module that still returns nothing is DONE
            out[0] = status;
            out[1] = status === 1 || status === 2 ? ex.pr_tick() : -1;
            break;
          }
          case 2: {
            if (payload && payload.length) {
              if (payload.length > L.answer_size) throw new Error('event arguments larger than the answer region');
              view(answerOff, payload.length).set(payload);
            }
            const status = ex.pr_event(a, b, c, d);
            out[0] = status;
            out[1] = status === 1 || status === 2 ? ex.pr_tick() : -1;
            break;
          }
          case 3: {
            let len = 0;
            if (payload && payload.length) {
              if (payload.length > L.frame_size) throw new Error('answer frame larger than the frame region');
              view(frameIn, payload.length).set(payload);
              len = payload.length;
            }
            const status = ex.pr_resume(a, len);
            out[0] = status;
            out[1] = status === 1 || status === 2 ? ex.pr_tick() : -1;
            break;
          }
          case 4:
            ex.pr_cancel(a);
            break;
          default:
            throw new Error(`unknown call kind ${kind}`);
        }
      } catch (e) {
        // A trap: the player reports it and sends pr_cancel(-1).
        throw new Error(e && e.message ? e.message : String(e));
      }
      return out;
    },
    exports: () => ex,
    debugLive: () => (ex && typeof ex.pr_dbg_live === 'function' ? ex.pr_dbg_live() : -1),
    dispose() {
      ex = null;
    },
  };
}

// ── the link to the module, Worker mode ──────────────────────────────────────

/**
 * Runs the program module in a dedicated Worker (WASM-017 R19b). The imports are proxied to the main
 * thread: writes are batched exactly as in main-thread mode (one message per flush); a read blocks
 * the worker on a shared buffer until the main thread has answered, so the module sees the same
 * synchronous `query` either way.
 */
function createWorkerLink(env, module, manifest, bridge, workerUrl, policy) {
  const L = manifest.limits;
  const sab = new env.SharedArrayBuffer(64 + Math.max(L.answer_size, 64));
  const header = new Int32Array(sab, 0, 8);
  const data = new Uint8Array(sab, 64);
  const pending = new Map();
  let nextId = 1;
  let worker = null;
  let readyResolve;
  let readyReject;
  const ready = new Promise((res, rej) => {
    readyResolve = res;
    readyReject = rej;
  });
  const reply = (code, bytes) => {
    if (bytes) data.set(bytes);
    header[1] = code;
    Atomics.store(header, 0, 1);
    Atomics.notify(header, 0);
  };
  const onMessage = (ev) => {
    const m = ev.data;
    switch (m.type) {
      case 'ready':
        readyResolve(m);
        break;
      case 'fatal':
        readyReject(new LoadError(m.message, 'abi'));
        break;
      case 'flush':
        bridge.player.flush(m.bytes);
        break;
      case 'query': {
        const answer = bridge.player.query(m.bytes);
        if (answer === undefined || answer === null) reply(bridge.player.last_error());
        else reply(answer.length, answer);
        break;
      }
      case 'remote':
        reply(bridge.player.remote(m.bytes));
        break;
      case 'abort':
        bridge.player.abort(m.bytes);
        break;
      case 'result': {
        const p = pending.get(m.id);
        if (p) {
          pending.delete(m.id);
          p.resolve(Int32Array.from(m.out));
        }
        break;
      }
      case 'trap': {
        const p = pending.get(m.id);
        if (p) {
          pending.delete(m.id);
          p.reject(new Error(m.message));
        }
        break;
      }
      default:
        break;
    }
  };
  return {
    async instantiate() {
      const url = policy ? policy.createScriptURL(workerUrl) : workerUrl;
      worker = new env.Worker(url, { type: 'module' });
      worker.onmessage = onMessage;
      worker.onerror = (e) => readyReject(new LoadError(`The program worker failed to start (${e && e.message ? e.message : 'error'}).`, 'worker'));
      worker.postMessage({ type: 'init', module, manifest, sab });
      const m = await ready;
      if (m.abi !== ABI_VERSION) throw new LoadError(`The program speaks ABI ${m.abi} and this player speaks ABI ${ABI_VERSION}: rebuild the application.`, 'abi');
      return { abi: m.abi, variant: m.variant };
    },
    perform(kind, a, b, c, d, payload) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        const copy = payload && payload.length ? payload.slice() : null;
        worker.postMessage({ type: 'perform', id, kind, a, b, c, d, payload: copy }, copy ? [copy.buffer] : []);
      });
    },
    exports: () => null,
    debugLive: async () => {
      const id = nextId++;
      return new Promise((resolve) => {
        pending.set(id, { resolve: (v) => resolve(v[0]), reject: () => resolve(-1) });
        worker.postMessage({ type: 'live', id });
      });
    },
    dispose() {
      for (const p of pending.values()) p.reject(new Error('the application was unmounted'));
      pending.clear();
      if (worker) {
        worker.onmessage = null;
        worker.onerror = null;
        worker.terminate();
        worker = null;
      }
    },
  };
}

// ── the page ────────────────────────────────────────────────────────────────

// The loading and message panels are styled by a constructed stylesheet (CSSOM): the CSP allows no
// inline <style> and no style attribute, and the shadow root keeps the host page's CSS out.
const LOADER_CSS = `
:host { display: block; }
.pr-loading, .pr-message { box-sizing: border-box; display: flex; flex-direction: column; align-items: center;
  justify-content: center; gap: .75rem; width: 100%; height: 100%; min-height: 8rem; padding: 1.5rem;
  font: 14px/1.4 system-ui, sans-serif; color: CanvasText; text-align: center; }
.pr-loading-bar { width: min(20rem, 80%); }
.pr-message-title { margin: 0; font-weight: 600; }
.pr-message-detail { margin: 0; max-width: 40rem; opacity: .8; overflow-wrap: anywhere; }
.pr-message-action { font: inherit; padding: .4rem 1rem; cursor: pointer; }
.pr-root { width: 100%; height: 100%; }
`;

function adoptLoaderStyles(env, host) {
  if (!host || !env.CSSStyleSheet || !('adoptedStyleSheets' in host)) return;
  try {
    const sheet = new env.CSSStyleSheet();
    sheet.replaceSync(LOADER_CSS);
    host.adoptedStyleSheets = [...host.adoptedStyleSheets, sheet];
  } catch (_) {
    /* a browser without constructed stylesheets shows the panels unstyled, which still works */
  }
}

function el(doc, tag, className, text) {
  const e = doc.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

function showStage(ui, text, step, steps) {
  if (!ui) return;
  ui.label.textContent = text;
  ui.bar.max = steps;
  ui.bar.value = step;
}

function showMessage(doc, host, texts, title, detail, actions) {
  const box = el(doc, 'div', 'pr-message');
  box.setAttribute('role', 'alert');
  box.appendChild(el(doc, 'p', 'pr-message-title', title));
  if (detail) box.appendChild(el(doc, 'p', 'pr-message-detail', detail));
  for (const [label, fn] of actions) {
    const b = el(doc, 'button', 'pr-message-action', label);
    b.type = 'button';
    b.addEventListener('click', fn);
    box.appendChild(b);
  }
  host.replaceChildren(box);
  void texts;
}

/**
 * Mount the application into `target` (an element) and return the running app.
 *
 * `options`: `configUrl` (default `config.json` beside the page), `config` (an object, instead),
 * `overrides` ({program, player, mode} — tests and diagnostics), `env` (a replacement for the
 * globals — tests), `headless` (no page: the node tests), and the callbacks `onReady`, `onError`.
 */
export async function mount(target, options = {}) {
  const env = options.env || globalThis;
  const doc = target && target.ownerDocument ? target.ownerDocument : env.document;
  const headless = options.headless === true || !target;
  let texts = DEFAULT_TEXTS;
  let ui = null;
  let host = null;
  let app = null;
  const t0 = env.performance ? env.performance.now() : 0;
  const timing = {};

  const fail = (error) => {
    const e = error instanceof LoadError ? error : new LoadError(DEFAULT_TEXTS.failed, 'load', error && error.message ? error.message : String(error));
    if (options.onError) options.onError(e);
    if (target && target.dispatchEvent && env.CustomEvent) target.dispatchEvent(new env.CustomEvent('pr-error', { detail: { code: e.code, message: e.message, detail: e.detail } }));
    if (!headless) {
      showMessage(doc, host || target, texts, e.code === 'unsupported' ? texts.unsupported : texts.failed, e.code === 'unsupported' ? '' : e.message, [
        [texts.retry, () => mount(target, options)],
      ]);
    }
    return e;
  };

  try {
    // 1. the page: a shadow root, so the host page's CSS does not leak in and ours does not leak out.
    if (!headless) {
      host = target.shadowRoot || target.attachShadow({ mode: 'open' });
      host.replaceChildren();
      if (!host.__prStyled) {
        adoptLoaderStyles(env, host);
        host.__prStyled = true;
      }
      const box = el(doc, 'div', 'pr-loading');
      const label = el(doc, 'p', 'pr-loading-label', texts.loading);
      const bar = el(doc, 'progress', 'pr-loading-bar');
      box.append(label, bar);
      host.appendChild(box);
      ui = { box, label, bar };
    }

    // 2. configuration
    const configUrl = resolveUrl(env, options.configUrl || (target && target.getAttribute && target.getAttribute('data-pr-config')) || 'config.json', options.baseUrl);
    let config = options.config;
    if (!config) {
      const r = await fetchChecked(env, configUrl, null, 'The application configuration');
      config = await r.json();
    }
    texts = { ...DEFAULT_TEXTS, ...(config.texts || {}) };
    const base = configUrl;
    const url = (rel) => resolveUrl(env, rel, base);

    // 3. the browser
    showStage(ui, texts.stage_check, 1, 6);
    const p = probe(env);
    if (!p.baseline) throw new LoadError(texts.unsupported, 'unsupported', p.missing.join('; '));
    const choice = choose(p, config, options.overrides || {});
    timing.probe = env.performance ? env.performance.now() - t0 : 0;

    // 4. Trusted Types: one default policy that lets same-origin script URLs through and nothing else
    //    (the CSP enforces `require-trusted-types-for 'script'`; a Worker's URL is a script URL) —
    //    except the reCAPTCHA scripts, and only when the application has a site key (WASM-009 R4).
    let policy = null;
    const recaptchaOn = !!(config.options && config.options.recaptcha_site_key);
    const RECAPTCHA_PREFIXES = ['https://www.google.com/recaptcha/', 'https://www.gstatic.com/recaptcha/'];
    if (env.trustedTypes && typeof env.trustedTypes.createPolicy === 'function') {
      try {
        const origin = (env.location && env.location.origin) || new URL(base).origin;
        policy = env.trustedTypes.createPolicy('default', {
          createScriptURL: (u) => {
            const abs = new URL(u, base);
            if (abs.origin === origin) return abs.href;
            if (recaptchaOn && RECAPTCHA_PREFIXES.some((p) => abs.href.startsWith(p))) return abs.href;
            throw new TypeError('script URL from another origin refused');
          },
          createHTML: () => {
            throw new TypeError('HTML strings are not accepted by this application');
          },
          createScript: () => {
            throw new TypeError('script strings are not accepted by this application');
          },
        });
      } catch (_) {
        policy = null; // a policy named `default` already exists, or Trusted Types are not enforced
      }
    }

    // 5. the player
    showStage(ui, texts.stage_player, 2, 6);
    const pv = config.player[choice.player] || config.player.plain;
    if (!pv) throw new LoadError('The bundle contains no player.', 'load');
    const playerJsUrl = url(pv.js);
    // dynamic import() takes no integrity: a modulepreload link carries it, and the import that
    // follows reuses the preloaded, checked response.
    const preloadChecked = (href, integrity, what) =>
      new Promise((resolve, reject) => {
        const link = doc.createElement('link');
        link.rel = 'modulepreload';
        link.href = href;
        link.integrity = integrity;
        link.crossOrigin = 'anonymous';
        link.onload = () => resolve();
        link.onerror = () => reject(new LoadError(`${what} failed its integrity check or could not be fetched.`, 'fetch', href));
        doc.head.appendChild(link);
      });
    if (pv.js_integrity && doc && !headless) {
      await preloadChecked(playerJsUrl, pv.js_integrity, 'The player script');
    }
    const playerModule = options.importPlayer ? await options.importPlayer(playerJsUrl) : await import(playerJsUrl);
    const playerWasmUrl = url(pv.wasm);
    let wasmSource;
    if (headless && options.playerWasmBytes) {
      wasmSource = options.playerWasmBytes;
    } else {
      const wr = await fetchChecked(env, playerWasmUrl, pv.wasm_integrity, 'The player');
      wasmSource = wr;
    }
    const playerExports = await playerModule.default({ module_or_path: wasmSource });
    if (choice.player === 'threaded' && typeof playerModule.initThreadPool === 'function') {
      const n = (config.options && config.options.workers) || Math.max(1, Math.min(8, p.hardwareConcurrency - 1));
      await playerModule.initThreadPool(n);
    }
    timing.player = env.performance ? env.performance.now() - t0 : 0;

    // The parts (WASM-016 R13): modules of their own that the player asks for the first time it needs one
    // (today the Mermaid renderer). Nothing is fetched here; a part is loaded once, on first use, with
    // the same integrity checks as the player, and a part that failed to load may be asked for again.
    const parts = new Map();
    const loadPart = (name) => {
      if (parts.has(name)) return parts.get(name);
      const pt = config.player && config.player.parts && config.player.parts[name];
      const loading = (async () => {
        if (!pt) throw new Error(`this application does not include the ${name} part`);
        const jsUrl = url(pt.js);
        if (pt.js_integrity && doc && !headless) await preloadChecked(jsUrl, pt.js_integrity, `The ${name} part`);
        const mod = options.importPlayer ? await options.importPlayer(jsUrl) : await import(jsUrl);
        const wr = await fetchChecked(env, url(pt.wasm), pt.wasm_integrity, `The ${name} part`);
        await mod.default({ module_or_path: wr });
        return mod;
      })();
      loading.catch(() => parts.delete(name));
      parts.set(name, loading);
      return loading;
    };
    // `part(name, fn, arg)`: call `fn` of the part with one text argument; the answer is text.
    const callPart = async (name, fn, arg) => {
      const mod = await loadPart(name);
      if (typeof mod[fn] !== 'function') throw new Error(`the ${name} part has no ${fn}`);
      return String(mod[fn](arg));
    };

    // 6. the program
    showStage(ui, texts.stage_program, 3, 6);
    const pg = config.program[choice.program];
    if (!pg) throw new LoadError(`The bundle has no ${choice.program}-memory program module.`, 'load');
    const mr = await fetchChecked(env, url(config.manifest.url), config.manifest.integrity, 'The manifest');
    const manifestText = await mr.text();
    const programResponse = await fetchChecked(env, url(pg.url), pg.integrity, 'The program');
    const module = await compile(env, programResponse);
    const embedded = readEmbeddedManifest(env, module);
    const chosenManifestText = embedded || manifestText;
    let manifest;
    try {
      manifest = JSON.parse(chosenManifestText);
    } catch (_) {
      throw new LoadError('The program\'s manifest is not valid JSON.', 'abi');
    }
    if (manifest.abi !== ABI_VERSION) {
      throw new LoadError(`This application was built for ABI ${manifest.abi} and this player speaks ABI ${ABI_VERSION}: rebuild the application with the current IDE.`, 'abi');
    }
    if (manifest.variant && manifest.variant.conformance) throw new LoadError('This program was built for conformance testing and cannot run in a page.', 'abi');
    checkProgramModule(env, module, manifest.limits);
    timing.program = env.performance ? env.performance.now() - t0 : 0;

    // 7. the player object, then the link, then the module
    showStage(ui, texts.stage_start, 4, 6);
    const bridge = { player: null };
    const link =
      choice.mode === 'worker'
        ? createWorkerLink(env, module, manifest, bridge, url(config.worker.url), policy)
        : createMainLink(env, module, manifest, bridge);
    const UrlParams = env.URLSearchParams || globalThis.URLSearchParams;
    const query = new UrlParams((env.location && env.location.search) || '');
    const pcfg = {
      headless,
      standalone: config.style === 'standalone',
      debug_level: (config.options && config.options.debug_level) || 0,
      backend_origins: (config.backend && config.backend.origins) || [],
      call_timeout_ms: config.backend && config.backend.call_timeout_ms,
      unsafe_plain_ws: !!(config.backend && config.backend.unsafe_plain_ws),
      weblink_schemes: (config.options && config.options.weblink_schemes) || [],
      recaptcha_site_key: (config.options && config.options.recaptcha_site_key) || '',
      auth_flow: (config.options && config.options.auth_flow) || 'in-app',
      // The hosted sign-in flow comes back with an authorisation code (WASM-009 R8b): the backend's
      // callback redirects here with `?pr-code=…&pr-state=…`. They are handed to the player and taken
      // out of the address at once, so the code is in neither the history nor a later Referer.
      hosted_code: query.get('pr-code') || '',
      hosted_state: query.get('pr-state') || '',
      locale: (config.options && config.options.locale) || (env.navigator && env.navigator.language) || 'en',
      // A standalone page is opened to run one form (WASM-011 R9): `?pr-form=Name&pr-opener=handle`.
      start_form: options.startForm || query.get('pr-form') || '',
      // What the form was opened with (WASM-011 R4): `pr-param-<name>=<value>` pairs, in the order given.
      start_params: [...query.entries()].filter(([k]) => k.startsWith('pr-param-')).map(([k, v]) => [k.slice('pr-param-'.length), v]),
      opener_handle: query.get('pr-opener') || '',
      // The private channel to the window that opened this one (WASM-011 R11): the token is the opener's.
      super_token: query.get('pr-super') || '',
      tile_url: (config.options && config.options.tile_url) || '',
      image_origins: (config.options && config.options.image_origins) || [],
      app_url: (config.options && config.options.app_url) || '',
    };
    if ((pcfg.hosted_code || pcfg.hosted_state) && env.history && typeof env.history.replaceState === 'function' && env.location) {
      try {
        const here = new URL(env.location.href);
        here.searchParams.delete('pr-code');
        here.searchParams.delete('pr-state');
        env.history.replaceState(env.history.state, '', here.pathname + here.search + here.hash);
      } catch (_) {
        /* the address stays as it is; the player still has what it needs */
      }
    }
    let panicked = false;
    const player = new playerModule.Player(JSON.stringify(pcfg), chosenManifestText);
    bridge.player = player;
    if (typeof playerModule.install_panic_hook === 'function') {
      playerModule.install_panic_hook((msg) => {
        if (panicked) return;
        panicked = true;
        if (options.onPanic) options.onPanic(msg);
        if (!headless) {
          showMessage(doc, host, texts, texts.crashed, msg, [
            [texts.reload, () => env.location && env.location.reload()],
            [texts.retry, () => app && app.unmount().then(() => mount(target, options))],
          ]);
        }
        if (target && target.dispatchEvent && env.CustomEvent) target.dispatchEvent(new env.CustomEvent('pr-error', { detail: { code: 'panic', message: msg } }));
      });
    }
    const info = await link.instantiate();

    // 8. the forms and the stylesheet
    for (const f of config.forms) {
      const fr = await fetchChecked(env, url(f.url), f.integrity, `The form ${f.name}`);
      player.add_form(await fr.text());
      if (f.css && !headless) {
        const sr = await fetchChecked(env, url(f.css.url), f.css.integrity, `The stylesheet of form ${f.name}`);
        player.set_form_stylesheet(f.name, await sr.text());
      }
    }
    // Menus (MenuBar and SideMenu) are separate files beside their forms.
    for (const m of config.menus || []) {
      const mr = await fetchChecked(env, url(m.url), m.integrity, `The menu ${m.control} of form ${m.form}`);
      player.add_menu(m.form, m.control, await mr.text());
    }
    if (config.stylesheet && !headless) {
      const sr = await fetchChecked(env, url(config.stylesheet.url), config.stylesheet.integrity, 'The application stylesheet');
      player.set_stylesheet(await sr.text());
    }
    if (config.assets && typeof player.set_assets === 'function') player.set_assets(JSON.stringify(config.assets));
    link.part = callPart;
    link.partModule = loadPart;
    player.attach(link);
    if (!headless) {
      const root = el(doc, 'div', 'pr-root');
      host.replaceChildren(root);
      player.mount(root);
    }
    showStage(ui, texts.stage_start, 6, 6);
    // A window another window opened waits (briefly) for that window's state, so the first handler reads it.
    if (pcfg.super_token && typeof player.attach_opener === 'function') await player.attach_opener(pcfg.super_token, 1500);
    player.start();
    timing.started = env.performance ? env.performance.now() - t0 : 0;

    // 9. the handle the host page gets
    let unmounted = false;
    app = {
      player,
      link,
      // The player's own memory (it may grow); exposed for the memory test, never viewed by the loader.
      playerMemory: playerExports && playerExports.memory,
      choice,
      probe: p,
      info,
      manifest,
      timing,
      config,
      // What the player was told about the page (diagnostics and the tests).
      playerConfig: pcfg,
      // Call a function of a part (loaded on first use); the player does this for the Viewer's diagrams.
      part: callPart,
      // The part's module itself (loaded on first use), for a part whose exports are objects and bytes
      // rather than text: the Canvas 2D part's `Canvas` (a display list in, RGBA out).
      partModule: loadPart,
      async settled() {
        return player.settled();
      },
      async unmount() {
        if (unmounted) return '{"closed":0,"cancelled":0}';
        unmounted = true;
        let summary = '{"closed":0,"cancelled":0}';
        try {
          summary = await player.unmount();
        } finally {
          // Every Rust object exposed to JavaScript is freed explicitly (WASM-017 R14).
          try {
            player.free();
          } catch (_) {
            /* already freed */
          }
          link.dispose();
          bridge.player = null;
          if (host && !headless) host.replaceChildren();
        }
        return summary;
      },
    };
    if (options.onReady) options.onReady(app);
    if (target && target.dispatchEvent && env.CustomEvent) target.dispatchEvent(new env.CustomEvent('pr-ready', { detail: { choice, timing } }));
    return app;
  } catch (e) {
    const err = fail(e);
    throw err;
  }
}

export { DEFAULT_TEXTS as defaultTexts };
