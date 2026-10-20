// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// The program worker (WASM-017 R19b): runs the program module off the main thread when the page is
// cross-origin isolated. The module's six imports are proxied to the main thread:
//
//   flush / abort  one message with a COPY of the batch (the view is dropped at once, R10);
//   query / remote the batch goes over, then this thread BLOCKS on a shared buffer until the main
//                  thread has answered — the module sees the same synchronous call as on the main
//                  thread, so its results do not depend on the mode;
//   now / random   answered here.
//
// A task that yields is resumed here, after a macrotask, so the worker stays able to receive the
// events of other form instances while a long loop runs; the main thread therefore never sees a
// YIELDED status from a worker.

'use strict';

let ex = null;
let base = 0;
let L = null;
let header = null;
let data = null;

const view = (off, len) => new Uint8Array(ex.arena.buffer, base + off, len);

function post(msg, transfer) {
  self.postMessage(msg, transfer || []);
}

function copyOut(off, len) {
  return view(off, len).slice();
}

function waitForReply() {
  // The main thread writes the length (or a negative code) at header[1], then sets header[0] to 1.
  Atomics.wait(header, 0, 0);
  const code = header[1];
  Atomics.store(header, 0, 0);
  return code;
}

function makeImports() {
  const batchOff = 0;
  const answerOff = L.batch_size;
  const frameOut = L.batch_size + L.answer_size;
  return {
    pr: {
      flush(len) {
        const bytes = copyOut(batchOff, len);
        post({ type: 'flush', bytes }, [bytes.buffer]);
      },
      query(len) {
        const bytes = copyOut(batchOff, len);
        Atomics.store(header, 0, 0);
        post({ type: 'query', bytes }, [bytes.buffer]);
        const code = waitForReply();
        if (code < 0) return code;
        view(answerOff, code).set(data.subarray(0, code));
        return code;
      },
      remote(len) {
        const bytes = copyOut(frameOut, len);
        Atomics.store(header, 0, 0);
        post({ type: 'remote', bytes }, [bytes.buffer]);
        return waitForReply();
      },
      now() {
        return Date.now();
      },
      random() {
        return Math.random();
      },
      abort(len) {
        const bytes = copyOut(batchOff, len);
        post({ type: 'abort', bytes }, [bytes.buffer]);
      },
    },
  };
}

const macrotask = () => new Promise((resolve) => setTimeout(resolve, 0));

// The task a status left non-DONE is read the moment the status is, before anything else can run.
const tickOf = (status) => (status === 1 || status === 2 ? ex.pr_tick() : -1);

// Run to a non-YIELDED status, giving other messages a turn between slices.
async function settle(status) {
  let task = tickOf(status);
  while (status === 2) {
    await macrotask();
    status = ex.pr_resume(task, 0);
    task = tickOf(status);
  }
  return [status, task];
}

async function perform(m) {
  const answerOff = L.batch_size;
  const frameIn = L.batch_size + L.answer_size + L.frame_size;
  const out = new Int32Array(4);
  switch (m.kind) {
    case 0: {
      const inst = ex.pr_open_form(m.a, m.b);
      const [status, task] = await settle(ex.pr_status());
      out[0] = inst;
      out[1] = status;
      out[2] = task;
      break;
    }
    case 1: {
      const raw = ex.pr_close_form(m.a);
      const [status, task] = await settle(raw === undefined ? 0 : raw);
      out[0] = status;
      out[1] = task;
      break;
    }
    case 2: {
      if (m.payload && m.payload.length) view(answerOff, m.payload.length).set(m.payload);
      const [status, task] = await settle(ex.pr_event(m.a, m.b, m.c, m.d));
      out[0] = status;
      out[1] = task;
      break;
    }
    case 3: {
      let len = 0;
      if (m.payload && m.payload.length) {
        view(frameIn, m.payload.length).set(m.payload);
        len = m.payload.length;
      }
      const [status, task] = await settle(ex.pr_resume(m.a, len));
      out[0] = status;
      out[1] = task;
      break;
    }
    case 4:
      ex.pr_cancel(m.a);
      break;
    default:
      throw new Error(`unknown call kind ${m.kind}`);
  }
  return out;
}

self.onmessage = async (ev) => {
  const m = ev.data;
  try {
    if (m.type === 'init') {
      L = m.manifest.limits;
      header = new Int32Array(m.sab, 0, 8);
      data = new Uint8Array(m.sab, 64);
      const instance = await WebAssembly.instantiate(m.module, makeImports());
      ex = instance.exports;
      const abi = ex.pr_abi();
      base = ex.pr_arena_base();
      if (ex.pr_init() !== 0) throw new Error('pr_init failed');
      post({ type: 'ready', abi, variant: ex.pr_variant() });
    } else if (m.type === 'perform') {
      try {
        const out = await perform(m);
        post({ type: 'result', id: m.id, out: Array.from(out) });
      } catch (e) {
        post({ type: 'trap', id: m.id, message: e && e.message ? e.message : String(e) });
      }
    } else if (m.type === 'live') {
      const n = typeof ex.pr_dbg_live === 'function' ? ex.pr_dbg_live() : -1;
      post({ type: 'result', id: m.id, out: [n, 0, 0, 0] });
    }
  } catch (e) {
    post({ type: 'fatal', message: e && e.message ? e.message : String(e) });
  }
};
