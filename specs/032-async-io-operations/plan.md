<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Async I/O for RestClient, SqlDatabase & IndexedFile

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-20

## 1. Approach

The one fact that makes this tractable without a core rewrite: `COBOL-WAIT-EVENT`
already blocks on `self.event_rx.recv()` between every dispatch
(`interpreter.rs:4068`), and dispatch is COBOL-side — the Rust handler only sets
`COBOL-EVENT-ID`/`COBOL-CONTROL-ID` and lets generated COBOL `EVALUATE`. So if an
async completion arrives as **just another `FormEvent` on that same channel**, the
existing machinery delivers it for free (R2). We add a small async engine beside
the event loop and teach the wait handler to drain it.

**New runtime state on `Interpreter`** (all `Option`/empty in CLI mode):

- `event_tx: Option<mpsc::Sender<FormEvent>>` — a **clone of the host's event
  sender** (the interpreter currently holds only the `Receiver`, `interpreter.rs:
  522`). Workers clone this to post the synthetic wakeup. Wired in
  `new_with_channels` (add a param) plus a `set_event_sender` setter for the debug
  constructor path (R2, spec §6 threading note).
- `async_result_tx/rx: (Sender, Receiver)<AsyncOpResult>` — an interpreter-owned
  `mpsc` pair created in the constructor; workers send results on `_tx`, the wait
  loop drains `_rx` (R1/R4).
- `async_pending: HashMap<String /*ctrl_id*/, PendingOp>` — the single in-flight
  op per control: its generation, kind (Rest/Sql/Indexed), the started-at tick,
  and its `TimeoutMs` (R3/R6/R13).
- `async_generations: HashMap<String /*ctrl_id*/, Arc<AtomicU64>>` — the live
  generation per control, shared with its worker so a stale result is detectable
  (R3). Bumped by `Cancel()`/timeout/supersede.
- `async_dispatch_queue: VecDeque<(String /*ctrl_id*/, String /*event_id*/)>` —
  completions ready to hand to COBOL, drained one-per-`WAIT-EVENT` (R5).

**`AsyncOpResult`** (new type, `interpreter.rs` or a new `async_op.rs` module):
`{ ctrl_id: String, generation: u64, outcome: AsyncOutcome }` where
`AsyncOutcome` is `Success { body, status } | HttpError { message } | Timeout |
QueryOk { rows } | DbError { message } | …`. The worker fills it; the interpreter
maps it to (state writes + event id).

**Start path (per verb, `Mode = Async`).** In `exec_method`, before the existing
blocking body: read the control's `Mode`; if `Async`, call a new
`self.spawn_async_op(obj, kind, inputs)`:

1. If `obj` is `Busy` → reject (R6), return empty.
2. Bump/create the control's `Arc<AtomicU64>` generation; snapshot it as `gen`.
3. `obj_set(obj, "Busy", "1")` (mirrors to UI via existing `state_tx`).
4. Record `async_pending[ctrl_id] = PendingOp { gen, kind, started_at,
   timeout_ms }`.
5. Spawn `std::thread`: clone the `HttpClient` (it holds only `headers`; derive
   `Clone`) / DB handle inputs / indexed inputs, do the **blocking** call
   (REST via a `ureq` agent built with the timeout, R12), build `AsyncOpResult`,
   `async_result_tx.send(result)`, **then** `event_tx.send(FormEvent{ ctrl_id,
   event_id: ASYNC_COMPLETE, instance_index: 0 })`. Sending the result *before*
   the wakeup guarantees the result is enqueued by the time the wait loop wakes.
6. Return immediately (empty string — the async verbs have no meaningful
   same-statement return).

`Mode = Sync` keeps the current code path verbatim (R8/R9).

**Drain path (`COBOL-WAIT-EVENT`, `interpreter.rs:4063-4116`).** Restructure the
handler entry:

1. **Before blocking:** if `async_dispatch_queue` is non-empty, pop one, present
   it to COBOL (set `COBOL-EVENT-ID`/`COBOL-CONTROL-ID`), return — no block (R5).
2. Otherwise `recv()` as today. On any received event, first call
   `self.drain_async_results()`:
   - `for r in async_result_rx.try_iter()`: look up `async_generations[ctrl_id]`;
     if `r.generation` ≠ live generation → discard (R3). Else apply outputs
     (`Busy=0`; REST writes `ResponseBody`/`StatusCode`; on error set an error
     property), remove `async_pending[ctrl_id]`, and push `(ctrl_id, event_id)`
     onto `async_dispatch_queue` where `event_id` is `onComplete`/`onError`.
   - **Timeout sweep:** for each remaining `async_pending`, if `now − started_at >
     timeout_ms > 0`: bump its generation (so the eventual worker result is
     dropped), `Busy=0`, enqueue `(ctrl_id, onTimeout)` (R13).
3. If the received event is the synthetic `ASYNC_COMPLETE` sentinel, it carries no
   COBOL-visible dispatch of its own — after draining, pop one from
   `async_dispatch_queue` (or present a no-op if empty: set `COBOL-EVENT-ID` to a
   value no `WHEN` matches, so the generated `EVALUATE` falls through). If the
   received event is a **real** UI event, present it as today; the drained async
   completions wait in the queue and flush on subsequent returns (each async op
   also posted its own wakeup, so there are enough returns to flush).

Because state writes happen here (interpreter thread), `ObjectRegistry` stays
single-threaded; the worker only ever touches its cloned inputs and the two
channels (spec §6).

**Cancel (`interpreter.rs` `exec_method`, new `"CANCEL"` verb, all three types).**
On the interpreter thread: if `async_pending` has no entry for `obj` → no-op
(R11). Else bump the generation, remove the pending entry, `obj_set(obj,"Busy",
"0")`, and enqueue `(ctrl_id, onCancelled)` (or fire immediately by presenting it
on the next queue drain). No wait on the worker; its late result is dropped by R3
(R10). The generic `CANCEL` verb sits beside the existing per-type method arms.

**REST specifics.** `HttpClient` (`http_runtime.rs`) gains `#[derive(Clone)]` and
a timeout-aware entry (`get_with_timeout`/`send_with_body_timeout` building a
`ureq::AgentBuilder::new().timeout(Duration::from_millis(ms)).build()`); the
worker uses it so a server stall returns as `Err` → `onError`, and a genuine
timeout (`ureq` timeout error) maps to `AsyncOutcome::Timeout` → `onTimeout`
(R12). `TimeoutMs = 0` falls back to `TimeoutSeconds × 1000` (Q2).

**Uniform events (Q1 resolved).** The engine fires only `onComplete`/`onError`/
`onCancelled`/`onTimeout`. Existing events (`onResponseReceived`,
`onQueryComplete`, …) remain in `supported_events()` for the sync path and are not
touched.

## 2. Affected crates / files

- `crates/cobolt-runtime/src/interpreter.rs` — new `Interpreter` fields (§1);
  `spawn_async_op` + `drain_async_results` + timeout sweep helpers; rework the
  `COBOL-WAIT-EVENT` arm (4063-4116); mode-gate the REST verbs (5904-5944) and
  (opt-in) SQL `QUERY`/`EXECUTE` (5966-5985) and indexed `READ`/`START` (3747-
  3758, 3949-3962); new `"CANCEL"` verb in `exec_method`; extend
  `new_with_channels`/`new_with_debug_channels` to accept/clone `event_tx`.
- `crates/cobolt-runtime/src/async_op.rs` — **new (optional module).**
  `AsyncOpResult`, `AsyncOutcome`, `PendingOp`, `AsyncKind`, the sentinel const
  `ASYNC_COMPLETE`. (May instead live inline in `interpreter.rs`.)
- `crates/cobolt-runtime/src/http_runtime.rs` — `#[derive(Clone)]` on
  `HttpClient`; timeout-aware request methods (build a `ureq` agent with a
  timeout); distinguish a timeout error from other transport errors.
- `crates/cobolt-runtime/src/db_runtime.rs` / `indexed_redb.rs` — no change for
  the default (`Sync`) path. For the opt-in async path, the worker needs
  owned/serialisable inputs and result rows; the existing `DbConn` caches rows
  in `self.rows` and the redb engine holds cursor state behind `&mut self`, so
  async SQL/Indexed runs a **self-contained** query on a cloned connection
  string / re-opened handle rather than sharing the live `&mut` handle (see
  Risks). v1 may ship async **REST only** and gate SQL/Indexed async behind the
  `Mode` flag as a follow-through once the handle-ownership story is settled.
- `crates/cobolt-runtime/src/channels.rs` — no change (reuses `FormEvent`); the
  sentinel is a reserved `event_id`/`ctrl_id` string.
- `crates/cobolt-forms/src/model.rs` — add `Mode`/`Busy`/`TimeoutMs` to the
  default-property arms (RestClient ~3370, SqlDatabase ~3388, IndexedFile ~3403);
  add the four events to `supported_events()` arms (2506/2509/2516), de-duping
  `onError`/`onTimeout` where they already exist.
- `crates/cobolt-ide/src/panels/properties.rs` — add `Mode` (combo Sync/Async),
  `TimeoutMs` (int row), read-only `Busy` rows to the RestClient (5873-6027),
  SqlDatabase (6030-6105), IndexedFile (6108-6187) arms. Events surface
  automatically via `show_events` (4363) reading `supported_events()`.
- `crates/cobolt-ide/src/llm.rs` — the "PROPERTY KEYS BY TYPE" allowlist is
  **prose** (line 1204) whose key set is derived from the model default map;
  confirm the context builder enumerates the model defaults (so new keys flow
  automatically) and adjust if it hard-codes a list.
- `crates/cobolt-ide/src/i18n.rs` — only if property labels are promoted to `Tr`
  (see spec §6/R16); default plan uses literals, so no change.
- `docs/developers-guide-en.md` — async controls subsection (English only).

## 3. Data / model changes

- **New properties (all three controls, additive, `#[serde]`-defaulted via the
  default-property map):** `Mode: String` (`"Async"` for RestClient, `"Sync"`
  for SqlDatabase/IndexedFile), `Busy: Bool` (`false`), `TimeoutMs: Int`
  (RestClient `30000`; SqlDatabase/IndexedFile `0` = no timeout). Old `.cfrm`
  forms with no `Mode` load with these defaults (R9) — `Control::new` always
  seeds them, and existing serialized forms merge over the seeded defaults, so a
  missing key keeps the default.
- **New events (all three):** `onComplete`, `onError`, `onCancelled`,
  `onTimeout` appended to each `supported_events()` array (skip duplicates).
- **New runtime types:** `AsyncOpResult`/`AsyncOutcome`/`PendingOp`/`AsyncKind`
  (runtime-internal, not serialized).
- **Interpreter constructor signature** changes (adds `event_tx`); update all
  call sites (`FormRuntime` host in `cobolt-ide`/`cobolt-cli`). A `set_event_sender`
  setter keeps the debug/CLI paths simple.
- **Compat:** no on-disk format breaks; the `FormEvent` wire type is unchanged
  (the sentinel is a normal string value, harmless to the IPC runner which will
  just see an event with an unmatched id).

## 4. Key decisions & alternatives

- **Decision:** Deliver completion as a `FormEvent` on the existing channel; the
  interpreter holds a clone of `event_tx`. — **Why:** reuses the entire
  event-dispatch path (R2); no second loop. — **Rejected:** a bespoke completion
  loop / condvar (duplicates machinery, risks missed wakeups).
- **Decision:** One detached `std::thread` per op, no pool. — **Why:** simplicity
  first; in-flight ops are few (one per control). — **Rejected:** a thread pool
  or `tokio` (scope creep; a later spec).
- **Decision:** Generation counter + single pending op per control; results
  validated by generation. — **Why:** makes `Cancel()`/timeout/supersede
  instant and race-safe without interrupting the worker (R3/R10). — **Rejected:**
  trying to kill the worker (`ureq`/`rusqlite`/`redb` have no cancellation).
- **Decision:** `Mode` per control, `Async` default for REST only. — **Why:**
  matches the operator's compatibility call — REST is the stall-prone case, so it
  gets the benefit by default; SQL/Indexed stay byte-identical for max speed,
  opt-in when wanted (R9). — **Rejected:** async-default everywhere (silently
  breaks same-statement-result code) / behind-a-flag everywhere (delays the REST
  benefit).
- **Decision:** Uniform `onComplete/onError/onCancelled/onTimeout` (Q1). — **Why:**
  keeps the engine fully generic; codegen/IDE already data-driven. — **Rejected:**
  per-control completion-event mapping (re-introduces per-type special-casing).
- **Decision:** State writes only on the interpreter thread (worker touches only
  channels + cloned inputs). — **Why:** `ObjectRegistry` stays single-threaded;
  no locking. — **Rejected:** letting the worker `obj_set` directly (needs a
  shared, locked registry).
- **Decision:** Ship async **REST** fully; land async SQL/Indexed behind `Mode`
  once handle-ownership is settled. — **Why:** REST is cleanly offloadable (stateless
  `HttpClient` clone); the DB/redb handles are live `&mut` cursor state that a
  worker can't safely borrow. — **Rejected:** forcing shared `Arc<Mutex<>>` DB
  handles in v1 (bigger, riskier change than the compat decision requires).

## 5. Risks & mitigations

- **Risk:** cancel-then-late-arrival corrupts control state. → **Mitigation:**
  generation check discards any result whose generation ≠ live; test asserts a
  post-cancel worker result changes nothing (AC3).
- **Risk:** a completion is dropped or double-dispatched under simultaneous
  completions. → **Mitigation:** internal `async_dispatch_queue`, one dispatch per
  `WAIT-EVENT`; each op posts its own wakeup; test with two near-simultaneous ops
  (AC5).
- **Risk:** the worker's wakeup races the result (wakeup observed before result
  enqueued). → **Mitigation:** worker sends the result **before** the wakeup on
  ordered `mpsc`; by the time `recv()` returns the wakeup, `try_iter()` sees the
  result. If a spurious wakeup ever arrives with nothing to drain, present a
  no-op (harmless).
- **Risk:** a slow/hung request blocks shutdown. → **Mitigation:** threads are
  detached; the transport timeout bounds REST; on interpreter drop the channels
  close and orphaned results are dropped.
- **Risk:** REST `Mode=Async` silently breaks an existing form that reads
  `ResponseBody` on the next line. → **Mitigation:** documented escape hatch
  `Mode = Sync`; CHANGELOG + dev-guide call it out explicitly.
- **Risk:** async SQL/Indexed with a shared live `&mut` handle is unsound. →
  **Mitigation:** v1 gates them `Sync` by default; async path re-opens/owns its
  own handle (documented as a follow-through, not forced now).
- **Risk:** constructor-signature change breaks host call sites. → **Mitigation:**
  add the `event_tx` param + a setter; update `FormRuntime` and CLI call sites in
  the same change; `cargo build` across the workspace gates it.

## 6. Test strategy

Unit/integration in `cobolt-runtime` (add; each asserts + reports real values).
A tiny **slow mock HTTP server** (a `std::net::TcpListener` on `127.0.0.1:0` in a
thread that sleeps then replies, or never replies) backs the REST tests — no
external network.

- **Responsiveness (AC1):** start an async `GET` against the slow server; assert
  the interpreter processes injected `onTick`/other `FormEvent`s and `Busy == 1`
  before the response; then the response arrives and `onComplete` dispatches.
- **Completion & write-back (AC2):** on completion `Busy == 0`,
  `ResponseBody`/`StatusCode` set, exactly one `onComplete` (or `onError` when the
  socket is closed) enqueued.
- **Cancel race (AC3):** `Cancel()` mid-flight returns immediately, fires
  `onCancelled`, `Busy == 0`; then release the server so the worker result
  arrives late — assert no property changed (generation check).
- **Timeout (AC4):** point at a never-responding listener with a short
  `TimeoutMs`; assert `onTimeout` fires and `Busy == 0` with no `Cancel()`.
- **Multiple completions (AC5):** two controls' ops complete near-simultaneously;
  assert two distinct completion events dispatched, one per wait return, none
  lost.
- **Busy rejection (AC6):** second start while `Busy` spawns no worker, first op
  intact.
- **Sync default preserved (AC7):** a `SqlDatabase`/`IndexedFile` op with no
  `Mode` returns its result in the same statement; existing runtime/forms tests
  stay green.
- **Model/forms (AC8):** `cargo test -p cobolt-forms --features render` — new
  props in defaults, new events in `supported_events()`.

Manual / visual (operator-run — I do not drive the app):

- A form with a ticking `Timer` and a `RestClient` hitting a slow endpoint: the
  clock keeps ticking and the UI repaints while the request runs; `onComplete`
  populates a label; a **Cancel** button stops it instantly. (AC1/AC2/AC3)
- Properties panel shows `Mode`/`TimeoutMs`/`Busy` on all three controls; Events
  list shows the four new events. (AC8)

## 7. Steering compliance

- [ ] i18n: new property labels follow the control-arm **literal** convention; if
      promoted to `Tr`, all six languages.
- [x] Generated-code banner + regenerate-on-action contract preserved — codegen
      event dispatch is generic; no change (R14).
- [ ] English dev guide updated (translations untouched) — async-controls
      subsection.
- [ ] Fix vs feature: **feature** → bump `z` + dated `CHANGELOG.md` entry on
      completion (pre-production rule).
- [x] No "cobolt" in user-facing text; COBOL identifiers/events stay English
      (`onComplete`, etc.).
