<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 032 — Async I/O for RestClient, SqlDatabase & IndexedFile

- **Status:** draft → approved
- **Folder:** specs/032-async-io-operations/
- **Author:** Claude (agent), for the operator   **Date:** 2026-07-20
- **Depends on:** spec 021 (control events — the `supported_events()` allowlist and
  the nested-program event-dispatch path this feature rides on).

## 1. Overview

Today a `RestClient` HTTP call, a `SqlDatabase` query, and an `IndexedFile` read
all execute **synchronously and inline on the interpreter thread**. The REST
verbs (`interpreter.rs` `exec_method` "GET"/"POST"/…) call the blocking `ureq`
client; `SqlDatabase` "QUERY"/"EXECUTE" call the blocking `rusqlite`/`postgres`/
`mysql` drivers; `IndexedFile` `READ`/`START` call the blocking `redb` engine.
Each runs on the **same thread that is blocked in `COBOL-WAIT-EVENT`**, so a slow
network request or a heavy query **freezes the entire form** — no other control
repaints, no `Timer` ticks, nothing dispatches — until the call returns.

This spec introduces a **generic async-operation primitive** in the runtime so
these calls run on a background worker thread while the event loop stays live.
Completion is delivered as **an ordinary `FormEvent` on the existing event
channel**, so the whole event-dispatch machinery that `onClick`/`onTick` already
use handles it for free — no core-loop restructuring. Each control gains a
`Busy` flag, a `TimeoutMs` timeout, a `Cancel()` method, and four uniform
lifecycle events — `onComplete`, `onError`, `onCancelled`, `onTimeout` — fired
when the background op finishes, fails, is cancelled, or times out.

Because this changes call semantics (a `GET` no longer returns its body in the
same statement), the behaviour is **mode-gated per control type** to protect
existing generated forms: a new `Mode` property is **`Async` by default for
`RestClient`** (the one control with genuine network-stall risk) and **`Sync` by
default for `SqlDatabase` and `IndexedFile`** (usually fast local ops — maximum
speed, zero behaviour change), with either opt-in-able the other way.

## 2. Goals / Non-goals

**Goals**

- Keep the form responsive (repaints, `Timer` ticks, other controls' events)
  while a REST request, SQL query, or indexed read is in flight.
- A single generic async engine in the runtime that all three control types
  share, built on the existing `mpsc<FormEvent>` event channel — no new event
  loop, no async runtime (`tokio`), no thread pool (a plain `std::thread` per op).
- `Cancel()`, a `Busy` flag, a `TimeoutMs` timeout, and uniform `onComplete` /
  `onError` / `onCancelled` / `onTimeout` events on all three controls.
- **Backward-compatibility by default:** existing `SqlDatabase`/`IndexedFile`
  forms are byte-for-byte unchanged (`Sync` default); existing `RestClient`
  forms that rely on same-statement results have a one-property escape hatch
  (`Mode = Sync`).
- Correct behaviour under races: a cancelled/superseded op's late result is
  discarded and never corrupts control state.

**Non-goals**

- No `tokio`/async-await rewrite and no shared thread pool — one detached
  `std::thread` per in-flight op (simplicity first; pooling is a later spec).
- No cooperative mid-call interruption of `ureq`/`rusqlite`/`redb` (none expose
  it). `Cancel()`/timeout **abandon** the worker; its result is dropped by the
  generation check. The orphaned thread finishes or times out on its own.
- No change to the built-in `COBOL-HTTP-*` / `COBOL-EXEC-SQL` **CALL** surface
  (those write into COBOL data-division vars and have no control to fire events
  on) — they stay synchronous.
- No concurrency *within* a single control: one in-flight op per control at a
  time (a second call while `Busy` is rejected — see R6). Overlapping ops across
  *different* controls are fine.
- No `.cfrm` on-disk breakage: all new properties are additive with defaults.

## 3. User stories

- As a developer calling a slow REST endpoint from a form, the UI keeps
  repainting and my `Timer` keeps ticking while the request runs, and my
  `onComplete` handler fires when the body arrives — I don't have to do anything
  to get this; `RestClient` is async out of the box.
- As a developer with an existing form that reads `ResponseBody` on the line
  after `GET`, I set the RestClient's `Mode` to `Sync` and it behaves exactly as
  before.
- As a developer running a long report query, I set the SqlDatabase's `Mode` to
  `Async`, bind `onComplete`, and the form stays responsive while it runs.
- As a developer, I bind a **Cancel** button that calls the control's `Cancel()`
  and the in-flight request stops affecting my form immediately, firing
  `onCancelled` — regardless of what the server is doing.
- As a developer, I set `TimeoutMs` and get an `onTimeout` event if the operation
  overruns, without wiring my own timer.

## 4. Requirements (EARS)

**Generic async primitive & completion delivery**

- **R1 (ubiquitous):** The runtime shall provide a generic async-operation
  facility usable by `RestClient`, `SqlDatabase`, and `IndexedFile`: starting an
  operation spawns a background `std::thread` that performs the blocking call and
  reports its outcome back to the interpreter, while the interpreter returns
  immediately.
- **R2 (event):** When a background operation finishes (success, error, or
  transport timeout), the worker shall (a) deliver the outcome to the interpreter
  over a dedicated result channel it owns, and (b) enqueue a synthetic wakeup
  `FormEvent` on the existing event channel, so a `COBOL-WAIT-EVENT` blocked with
  no other activity in flight wakes and processes the completion.
- **R3 (ubiquitous):** The runtime shall track, per control, a **generation**
  counter and a single **pending operation** record; a delivered result shall be
  applied only if its generation matches the control's current generation, and
  discarded silently otherwise.

**Wait-loop integration**

- **R4 (event):** When `COBOL-WAIT-EVENT` receives any event, the runtime shall
  first drain the async-result channel (non-blocking); for each result whose
  generation is current it shall write the control's outputs (`Busy = 0`, and for
  REST `ResponseBody`/`StatusCode`) and enqueue that control's completion event
  (`onComplete`/`onError`/`onTimeout`) as the event dispatched to COBOL; stale
  results shall be discarded.
- **R5 (state):** While more than one completion is ready at once, the runtime
  shall dispatch them **one per `COBOL-WAIT-EVENT` return** (queued internally),
  never dropping or coalescing a completion, mirroring the existing single-event
  dispatch contract.

**Per-control async behaviour, mode gating & Busy**

- **R6 (event):** When an async-mode operation is started on a control that is
  already `Busy`, the runtime shall reject the new call (no second worker) and
  leave the in-flight op untouched.
- **R7 (state):** While an async operation is in flight, the control's `Busy`
  property shall read `1`; it shall return to `0` exactly once, when the op
  completes, errors, times out, or is cancelled.
- **R8 (optional/where enabled):** Where a control's `Mode` is `Async`, its
  `RestClient` `GET`/`POST`/`PUT`/`DELETE` (and, when opted in, `SqlDatabase`
  `QUERY`/`EXECUTE` and `IndexedFile` `READ`/`START`) shall run via the async
  primitive and return immediately; where `Mode` is `Sync`, the call shall behave
  exactly as today (blocking, same-statement result).
- **R9 (constraint):** The default `Mode` shall be `Async` for `RestClient` and
  `Sync` for `SqlDatabase` and `IndexedFile`. Existing forms without a `Mode`
  property shall load with these defaults, and a `SqlDatabase`/`IndexedFile` form
  shall be behaviourally identical to today.

**Cancel**

- **R10 (event):** When `Cancel()` is invoked on a control, the runtime shall — on
  the interpreter thread, without waiting for the worker — bump the control's
  generation, set `Busy = 0`, and fire `onCancelled`; any later-arriving result
  from the abandoned worker shall be discarded by the generation check (R3).
- **R11 (state):** While no operation is in flight, `Cancel()` shall be a no-op
  (no event fired, `Busy` unchanged).

**Timeouts**

- **R12 (ubiquitous):** Each control shall expose a `TimeoutMs` property. For
  `RestClient` the timeout shall be applied to the transport (the `ureq` agent's
  connect/read timeout) so the worker returns on its own when the server stalls.
- **R13 (event):** When a pending op's elapsed time exceeds its `TimeoutMs`
  before a result arrives, the runtime (on the next wait-loop drain) shall treat
  it as timed out: bump generation, set `Busy = 0`, fire `onTimeout`, and discard
  any later result. `TimeoutMs = 0` shall mean "no timeout".

**Model, codegen & tooling**

- **R14 (ubiquitous):** The model (`cobolt-forms`) shall declare the new
  properties (`Mode`, `Busy`, `TimeoutMs`) and the four new events (`onComplete`,
  `onError`, `onCancelled`, `onTimeout`) on all three control types via the
  existing default-property map and `supported_events()` allowlist, so codegen
  event dispatch and the IDE Events panel pick them up automatically.
- **R15 (ubiquitous):** The IDE Properties panel shall render editors for `Mode`,
  `TimeoutMs`, and a read-only `Busy` indicator in the `RestClient`,
  `SqlDatabase`, and `IndexedFile` property arms; the Form Designer Agent's
  permitted-property/event allowlist shall include the new keys.
- **R16 (constraint):** Every new user-facing IDE string shall be handled per the
  existing convention for these control arms (hard-coded literals today; if
  promoted to `Tr`, all six languages EN/ES/PT/JA/ZH/FR).

## 5. Acceptance criteria

- [ ] **AC1 (R1/R2/R8):** With a `RestClient` (`Mode = Async`) hitting a
  deliberately slow mock HTTP server, a co-resident `Timer`'s `onTick` keeps
  firing and the interpreter keeps dispatching events while the request is in
  flight (interpreter-level test asserts other events flow before the response
  arrives).
- [ ] **AC2 (R4/R7):** When the slow request completes, `Busy` returns to `0`,
  `ResponseBody`/`StatusCode` are written, and exactly one `onComplete` (or
  `onError` on transport failure) is dispatched (test).
- [ ] **AC3 (R10/R3):** `Cancel()` on an in-flight op returns immediately, fires
  `onCancelled`, sets `Busy = 0`; a subsequently-arriving worker result does not
  change any control property (cancel-then-late-arrival test).
- [ ] **AC4 (R12/R13):** A request against a server that never responds fires
  `onTimeout` after `TimeoutMs`, with `Busy = 0`, **without** an explicit
  `Cancel()` (test using a short timeout).
- [ ] **AC5 (R5):** Two controls whose ops complete near-simultaneously each get
  their completion event dispatched exactly once, one per wait-loop return, none
  lost (test).
- [ ] **AC6 (R6):** Starting a second op on a `Busy` control does not spawn a
  second worker and does not disturb the first (test).
- [ ] **AC7 (R9):** A `SqlDatabase`/`IndexedFile` form with no `Mode` property
  loads as `Sync` and its query/read still returns its result in the same
  statement (test / existing `cobolt-forms` + interpreter tests stay green).
- [ ] **AC8 (R14/R15):** `Mode`/`Busy`/`TimeoutMs` appear in each control's
  default properties and Properties panel; `onComplete`/`onError`/`onCancelled`/
  `onTimeout` appear in each control's `supported_events()` and the IDE Events
  list; the Form Designer Agent allowlist accepts them (`cargo build -p
  cobolt-ide`; `cargo test -p cobolt-forms --features render`).
- [ ] **AC9 (R16):** New IDE strings follow the control-arm convention; if any
  `Tr` field is added it exists in all six language tables and `cargo test -p
  cobolt-ide` passes.

## 6. Constraints & steering check

- **i18n (6 languages):** The three property arms in `panels/properties.rs`
  currently use **hard-coded literals** for these controls (e.g. `"Timeout (s)"`,
  `"Follow redirects"`), not `Tr`. The new rows follow that local convention
  (literals `"Mode"`, `"Timeout (ms)"`, `"Busy"`). No new `Tr` keys are strictly
  required; if the operator prefers localized labels, they become `Tr` fields ×6.
- **Generated-code / regenerate contract:** Codegen event dispatch
  (`write_event_loop` / `write_nested_programs`) is **generic over `ctrl.events`**
  — the new events dispatch with **no codegen change**. The generated-code banner
  and regenerate-on-action contract are untouched. New COBOL forms that bind the
  new events regenerate normally.
- **Docs (English guide):** `docs/developers-guide-en.md` needs a subsection on
  async controls: `Mode`, `Busy`, `TimeoutMs`, `Cancel()`, and the four events —
  English only (translations user-maintained, Rule #3).
- **Fix vs feature:** **Feature** (new properties, events, method, runtime
  facility). Per the pre-production rule, bump `z` with a dated `CHANGELOG.md`
  entry on completion.
- **Threading:** The interpreter (`Interpreter`) currently holds only the
  `event_rx` **Receiver**. Worker threads need a `Sender<FormEvent>` to post
  wakeups, so the interpreter must additionally hold a clone of `event_tx`
  (constructor/setter change, see plan §3). All completion **state writes** are
  applied on the interpreter thread during the wait-loop drain, not on the worker
  thread, so `ObjectRegistry` stays single-threaded.

## 7. Open questions

- **Q1 — Uniform vs legacy events:** the async engine fires the **uniform four**
  (`onComplete`/`onError`/`onCancelled`/`onTimeout`) for all three controls, and
  the pre-existing per-control events (`onResponseReceived`, `onQueryComplete`,
  …) remain available but are **not** fired by the async path. Alternative: map
  completion onto each control's existing "complete" event. Recommend the uniform
  four (keeps the engine generic; least per-type special-casing). Resolve in
  `/plan`. **(Resolved in plan: uniform four.)**
- **Q2 — `TimeoutMs` vs existing `TimeoutSeconds`:** `RestClient` already has
  `TimeoutSeconds` (Int 30). Recommend: `TimeoutMs` is the authoritative async
  timeout; when `TimeoutMs = 0`, REST falls back to `TimeoutSeconds × 1000`.
  Resolve in `/plan`.
- **Q3 — Idle timeout precision:** with no transport timeout (SQL/Indexed) and an
  idle UI, `onTimeout` only fires on the next wait-loop drain (next event). Is a
  periodic wakeup warranted, or is "fires on next activity / worker return"
  acceptable? Recommend acceptable for v1 (REST, the stall-prone case, uses the
  transport timeout and returns on its own). Resolve in `/plan`.
- **Q4 — `Busy` editability:** `Busy` is a runtime state flag. Render it
  read-only/disabled in the Properties panel (recommended) vs not at all?
