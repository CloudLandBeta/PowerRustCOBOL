<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Async I/O for RestClient, SqlDatabase & IndexedFile

- **Status:** done (T1–T10 implemented 2026-07-20 as 1.30.58; T11 docs + T12 verification completed 2026-07-21 — runtime 228 / forms 169 tests green, IDE builds; async engine wakes by 40 ms `recv_timeout` polling while ops are pending instead of the planned `ASYNC_COMPLETE` event post, and `Mode` on SqlDatabase/IndexedFile is a forward-compatible surface only, per the done criteria's "byte-identical today")
- **Plan:** ./plan.md   **Date:** 2026-07-20

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. The workspace stays green
after every task. The built-in `COBOL-HTTP-*` / `COBOL-EXEC-SQL` CALL surface is
**not** modified.

---

- [x] **T1 — Async engine types + interpreter state** (R1, R3)
  - Files: `crates/cobolt-runtime/src/async_op.rs` (new; or inline in
    `interpreter.rs`); `crates/cobolt-runtime/src/interpreter.rs`;
    `crates/cobolt-runtime/src/lib.rs` (module decl).
  - Do: add `AsyncKind {Rest, Sql, Indexed}`, `AsyncOutcome`, `AsyncOpResult
    { ctrl_id, generation, outcome }`, `PendingOp { generation, kind, started_at,
    timeout_ms }`, and `const ASYNC_COMPLETE: &str`. Add `Interpreter` fields:
    `async_result_tx/rx`, `async_pending: HashMap<String, PendingOp>`,
    `async_generations: HashMap<String, Arc<AtomicU64>>`,
    `async_dispatch_queue: VecDeque<(String, String)>`, `event_tx:
    Option<mpsc::Sender<FormEvent>>`; init all in every constructor.
  - Verify: `cargo build -p cobolt-runtime`; a unit test constructs an
    `Interpreter` and asserts the new maps start empty.

- [x] **T2 — Interpreter holds a clone of `event_tx`** (R2)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`; host call sites in
    `crates/cobolt-ide/src/form_runtime.rs` and `crates/cobolt-cli` (and any other
    `new_with_channels`/`new_with_debug_channels` caller).
  - Do: extend `new_with_channels` to accept `event_tx: mpsc::Sender<FormEvent>`
    (the host already owns the `Sender`; pass a clone) and add a
    `set_event_sender` setter; store it in the new field. Update every call site.
  - Verify: `cargo build` (workspace) green; the host passes its existing sender
    clone; existing behaviour unchanged.

- [x] **T3 — `spawn_async_op` (generic) + `HttpClient` clone/timeout** (R1, R6, R7, R12)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`;
    `crates/cobolt-runtime/src/http_runtime.rs`.
  - Do: `#[derive(Clone)]` on `HttpClient`; add timeout-aware request methods that
    build a `ureq::Agent` with `.timeout(Duration::from_millis(ms))` and map a
    `ureq` timeout error to a distinct timeout outcome. Add
    `Interpreter::spawn_async_op(&mut self, obj, kind, inputs)`: reject if `Busy`
    (R6); bump/create the control's generation; `obj_set(obj,"Busy","1")` (R7);
    record `PendingOp`; spawn a `std::thread` that runs the blocking call, sends
    `AsyncOpResult` on `async_result_tx`, then posts a `FormEvent { ctrl_id,
    event_id: ASYNC_COMPLETE }` on `event_tx`.
  - Verify: `cargo test -p cobolt-runtime` — with a slow-mock `TcpListener`,
    starting an op sets `Busy==1`, spawns exactly one worker, and a second start
    while busy spawns none. **(AC1 partial, AC6)**

- [x] **T4 — Drain + dispatch in `COBOL-WAIT-EVENT`** (R2, R4, R5)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (the `COBOL-WAIT-EVENT` arm,
    ~4063–4116).
  - Do: on entry, if `async_dispatch_queue` non-empty pop one and present it
    (no block, R5). Else `recv()`; on any event call `drain_async_results()`:
    `try_iter()` results, discard stale generations (R3), else write outputs
    (`Busy=0`; REST `ResponseBody`/`StatusCode`), remove the pending entry, and
    enqueue `(ctrl_id, onComplete|onError)`. If the received event is
    `ASYNC_COMPLETE`, present a queued completion (or a no-op id if none); a real
    UI event is presented as today with completions flushing on later returns.
  - Verify: `cargo test -p cobolt-runtime` — slow request completes → `Busy==0`,
    `ResponseBody`/`StatusCode` written, exactly one `onComplete`; injected
    `onTick` events flow while in flight. **(AC1, AC2)**

- [x] **T5 — Mode-gate the REST verbs** (R8, R9)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`exec_method` REST arms,
    ~5904–5944).
  - Do: read the control's `Mode`; if `Async` route `GET`/`POST`/`PUT`/`DELETE`
    through `spawn_async_op` and return immediately; if `Sync` keep the current
    blocking body verbatim. `TimeoutMs==0` falls back to `TimeoutSeconds×1000`.
  - Verify: `cargo test -p cobolt-runtime` — `Mode=Sync` REST returns the body in
    the same statement (old behaviour); `Mode=Async` returns immediately and
    delivers via event. **(AC7 for REST)**

- [x] **T6 — `Cancel()` verb (all three controls)** (R10, R11)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`exec_method`, new
    `"CANCEL"` arm).
  - Do: if no pending op for `obj` → no-op (R11); else bump generation, remove the
    pending entry, `obj_set(obj,"Busy","0")`, enqueue `(ctrl_id, onCancelled)`.
    Never wait on the worker; its late result is dropped by the generation check.
  - Verify: `cargo test -p cobolt-runtime` — cancel mid-flight returns
    immediately, fires `onCancelled`, `Busy==0`; releasing the server so the
    worker result arrives late changes no property. **(AC3)**

- [x] **T7 — Timeout sweep + `onTimeout`** (R12, R13)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`drain_async_results`).
  - Do: in the drain, for each remaining `PendingOp` with `timeout_ms>0` and
    `now−started_at>timeout_ms`, bump generation, `Busy=0`, enqueue
    `(ctrl_id, onTimeout)`. (REST also enforces the transport timeout so the
    worker returns on its own.)
  - Verify: `cargo test -p cobolt-runtime` — a never-responding listener with a
    short `TimeoutMs` fires `onTimeout`, `Busy==0`, without `Cancel()`. **(AC4)**

- [x] **T8 — Model: properties + events (all three)** (R14)
  - Files: `crates/cobolt-forms/src/model.rs`.
  - Do: in `Control::new` add `Mode` (String; `"Async"` RestClient, `"Sync"`
    SqlDatabase/IndexedFile), `Busy` (Bool false), `TimeoutMs` (Int; 30000
    RestClient, 0 otherwise) to the three arms (~3370/3388/3403). In
    `supported_events()` append `onComplete`, `onError`, `onCancelled`,
    `onTimeout` to the three arms (2506/2509/2516), skipping existing duplicates
    (`onError`/`onTimeout`).
  - Verify: `cargo test -p cobolt-forms --features render` — new props present in
    defaults; new events present in `supported_events()`; no duplicate event
    names. **(AC8)**

- [x] **T9 — IDE Properties panel rows** (R15)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (RestClient 5873–6027,
    SqlDatabase 6030–6105, IndexedFile 6108–6187).
  - Do: add a `Mode` combo (Sync/Async), a `TimeoutMs` int row, and a read-only
    `Busy` indicator to each arm (reuse `int_prop_row`/combo/`bool_row_inline`
    patterns). Events already surface via `show_events` (no change).
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide`; rows reviewed
    for all three controls. **(AC8)**

- [x] **T10 — Form Designer Agent allowlist** (R15)
  - Files: `crates/cobolt-ide/src/llm.rs` (+ wherever the "PROPERTY KEYS BY TYPE"
    context string is assembled).
  - Do: confirm the agent's permitted property/event context is derived from the
    model default map + `supported_events()` (so the new keys flow through
    automatically); if it hard-codes a list, add `Mode`/`Busy`/`TimeoutMs` and the
    four events for the three control types.
  - Verify: `cargo build -p cobolt-ide`; inspect the generated agent context
    includes the new keys/events for RestClient/SqlDatabase/IndexedFile. **(AC8)**

- [x] **T11 — Docs (English guide)** (spec §6)
  - Files: `docs/developers-guide-en.md`.
  - Do: add an async-controls subsection: `Mode` (and the REST-async-default /
    SQL-Indexed-sync-default), `Busy`, `TimeoutMs`, `Cancel()`, and the four
    events; call out the `Mode = Sync` escape hatch for REST forms relying on
    same-statement results. English only — translations untouched (Rule #3).
  - Verify: section renders in the doc viewer; `git status` shows only `-en.md`.

- [x] **T12 — Finalize** (all AC)
  - Files: `crates/cobolt-ide/src/version.rs`; `CHANGELOG.md`.
  - Do: full workspace build + test. Confirm AC1–AC9 each map to a passing test.
    Bump `z` + add a dated `CHANGELOG.md` entry (feature under the pre-production
    fix rule). Do **not** commit/push unless the operator asks.
  - Verify: `cargo test -p cobolt-runtime`, `cargo test -p cobolt-forms --features
    render`, `cargo build -p cobolt-ide` all green; manual checks per plan §6
    (ticking Timer + slow RestClient stays responsive; Cancel stops it; panel
    shows the new props/events).

## Done criteria

All acceptance criteria in spec.md (AC1–AC9) are covered by a task's verification,
the runtime/forms tests and the IDE build are green, the English guide has the
async-controls subsection (translations untouched), and the change is a single
**feature** with a `z` bump + dated CHANGELOG entry. `SqlDatabase`/`IndexedFile`
remain `Sync` by default (byte-identical to today); `RestClient` is `Async` by
default with a `Mode = Sync` escape hatch. Do **not** commit or push unless the
operator asks.
