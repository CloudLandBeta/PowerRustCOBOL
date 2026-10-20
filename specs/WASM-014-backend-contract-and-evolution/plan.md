<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The backend contract and how it evolves

- **Spec:** [spec.md](spec.md). **Status:** the server side and the Rust client are implemented (M4). The browser's side of the contract (the generated module's own client, the form events that surface progress, warnings and stream items) is the generator's and the player's, and is listed under *Not done*.
- **Written:** 2026-10-20, by the implementing agent while the operator was away. Every decision below was taken without asking; each states what it chose, why, and what the alternative cost — for the operator to overturn.
- **Evidence** is in the last section; nothing is claimed there that was not run.

## 1. Decisions (answering the spec's open questions)

| Q | Decision | Why | Alternative |
|---|---|---|---|
| **Q1** reporting an *application* failure | **A built-in:** `COBOL::"WS-FAIL" ( code message [detail] )` ends the call. The code is the program's own, `1` to `7999`; `8000` and above belong to the platform (`8000` is *cancelled*). Parameter changes are discarded: the caller's `BY REFERENCE` groups stay as they were. | The platform can tell a deliberate failure from a fault (`server 9001`), and a status group in `RETURNING` would have made every caller test it by convention. | Both; rejected as two ways to do one thing. |
| **Q2** "may have been received" | **At-most-once by default, at-least-once for a program declared safe.** A failure that may have reached the backend carries `may_have_run = true`; one that certainly did not (no connection was made, the frame was never written) carries `false`. The runtime does not ask the backend afterwards. | It is the honest guarantee a client can give. | A status query would need the backend to remember every call. |
| **Q3** streams | **In 2.0, as a server-side stream program** (`stream = true` in `exports.toml`): the program runs once per subscription, in its own isolated run, and sends items with `COBOL::"WS-EMIT" ( group )` until it ends or the client unsubscribes. The server keeps **no state for an unsubscribed or disconnected client** (the run is stopped and its thread ends). | It fits WASM-003 Q2: each subscription is a *call* that lives long, not a session; stateless calls stay stateless. | A pub/sub broker is a later spec. |
| **Q4** where a file is fetched from | **As the spec recommends:** uploads in pieces over the WebSocket; downloads through a short-lived **HTTPS address** on the backend's own listener. | One channel for uploads (no second origin, the same token checks), browser-native streaming and resuming (`Range`) for downloads. | Pieces both ways: no streaming into the browser. |
| **Q5** retention | **Idempotency results: in memory, `idempotency_ttl_seconds` (600), at most 10 000 entries, oldest evicted first.** They do not survive a restart of the backend (a repeat after a restart runs again, which is the only honest answer for a backend that lost its memory). **Released interface descriptions: `backend/contract/released/<version>.json`, in the project, under version control, kept forever.** Uploaded files: the backend's `files.dir`, kept until the program removes them. | | A persisted idempotency store would need a database the backend does not otherwise have. |

Further decisions the spec did not ask:

1. **The error codes are stable and the class list is closed** (WASM-005 R10; `cobolt-wire/src/error.rs`). An unknown class from a newer backend is read as `server` (`ErrorClass::from_wire`; unit test). One platform code in the application class is reserved: **8000 `CANCELLED`**.
2. **Compatibility is checked in the direction each value travels.** A request is checked with the *old* client as sender (every member it sends must still exist, with room for it); an answer with the *new* backend as sender and the old client as reader — and **the old client reads every member it declared, so an answer that loses one is a break**, not only a request that does (the build test found that the first version of this rule let a renamed `RETURNING` member through; `a_returning_group_that_loses_a_member_breaks_older_clients` pins the fix). The rules: `cobolt-wire/src/compat.rs`, `evolution.rs`.
3. **Projection, not silence, is how a newer backend serves an older client.** The answer is written with the members the caller's contract version declared (`mapping::project_json`), so adding a member to an answer breaks nobody. A client built against a *newer* minor than the backend is refused at the `hello` ("update the backend"), since it may send what the backend would reject.
4. **A refusal is still a `hello`.** Whatever the client's protocol version, the first reply is a `hello` whose `accepted` is `refused` with a message and, where the backend can say, `update_to`; a client older than the window gets that, not a decoding error. The client maps it to class `version`: 6000 *client too old*, 6001 *client too new*, 6002 *protocol unsupported*.
5. **Ordering is per connection.** Calls of one connection run in the order they were made unless the program is declared `independent`; a refused call takes no turn. Several calls may be in flight (up to `max_calls_per_connection`); answers arrive in completion order and the id correlates them.
6. **Idempotency key.** Every call the client sends carries a key it generated (a call the developer repeats *with identical data right after a failure that may have run* reuses the previous call's key — that is the repeat R10 means). The backend stores `(caller, key) → answer` for the TTL and answers a repeat from the store, **without running the program a second time**; a repeat that arrives while the first is still running waits for it; the same key with different data is a `validation 4000`. Only successes and application failures are stored: a timeout or a limit must be retryable.
7. **A cancel is a request, not a kill.** For a `long_running` program the cancel sets a flag that `COBOL::"WS-CANCELLED" ( flag )` reads and that `COBOL::"WS-PROGRESS"` — the safe point — acts on by ending the call. A program that never reaches a safe point is stopped at the next statement after `cancel_grace_ms` (10 s). Either way the caller reads `application 8000` and **the program's effects are not rolled back**: cancelling stops work, it does not undo it.
8. **The backend names the files; the browser names nothing.** An upload names a *slot* the backend declared, never a path; the browser's file name is kept only as a display name after being refused if it contains `/`, `\`, `..`, a NUL or a control character; the stored file is `<files.dir>/files/<random 128-bit id>`. A download address is `/dl/<random 256-bit token>`, tied to the **subject and the source address** that asked, short-lived (`download_ttl_seconds`, 60), and answered with the same `404` for a token that never existed, expired or belongs to someone else.
9. **Where the client's progress goes.** The Rust client reports progress, warnings and stream items to an observer the host installs (`Interpreter::set_remote_events`); `COBOL::"WS-WARNING" ( message )` reads the most recent backend warning. Turning those into *form events* is the form host's and the generator's work (no form-level event exists for them yet) and is not done here.

## 2. Design

### 2.1 Versions and handshake (R1-R3, R16)
`hello {protocol, contract, client}` → `hello {protocol, protocol_min, contract, accepted: ok|warn|refused, message, update_to, server, max_frame_bytes}`. `negotiate()` (in `serve.rs`) is a pure function with a table test. The window is `[PROTOCOL_MIN, PROTOCOL]` (both `1` today). Contract rules: lower major → refused (update the application); higher major → refused (update the backend); same major with a higher minor than the backend → refused; lower minor → `warn`; equal → `ok`. The backend keeps the released interface descriptions and builds each answer for the contract the client announced.

### 2.2 Compatibility checking (R4, R5)
`cobolt_wire::evolution::check_against_released(current, released)` compares with the newest released contract and names every break with program, parameter, item path and rule (`member-missing`, `type-changed`, `narrowed`, `table-changed`, `parameters-changed`, `returning-removed`, `access-changed`, `roles-changed`, `idempotency-removed`, `kind-changed`, `program-removed`). A raised major version turns breaks into notes. `rcrun backend build` runs it before cargo and **fails a release build** on a break; a debug build reports and goes on. `rcrun backend check` runs it on its own and `rcrun backend release` records the interface.

### 2.3 Errors (R7, R8)
Closed classes, stable codes, an English message, a `detail` the program sets, and `may_have_run` for transport failures. `ErrorMsg` carries all four; `CallError` is the Rust form on both sides.

### 2.4 Repeating and ordering (R9-R11)
See decisions 5 and 6. In the client (`client.rs`): a lost connection fails or repeats each pending call by its program's declaration (taken from the interface description the client is given): a safe program is sent again once on the new connection; any other is failed with `transport 1001` and `may_have_run` true if its frame had been written. Reconnection backs off 100 ms doubling to 5 s, for at most `reconnect_deadline` (20 s) per call.

### 2.5 Long-running calls and streams (R13, R14)
Both are runs that outlive the usual timeout and send things while they run. The server gives a `long_running` call `long_call_timeout_ms` (600 000) instead of `call_timeout_ms`; progress frames are limited to ten a second per call (the last one, 100 %, is never dropped). A stream is subscribed with `sub {id, program, args}` and answered with `subscribed {id}`, then `event {sub, seq, data}` frames in order, `sub-end {sub}` when the program ends; `unsub {id}` ends it. After a reconnection the Rust client subscribes again and the sequence restarts at 1 on the new subscription (the client reports the restart). A slow consumer cannot make the backend buffer without bound: the event queue is bounded and a subscriber that falls behind is ended with a `limit` error.

### 2.6 Files (R15)
`put-begin` (slot, name, size, mime, sha256, resume) → `put-ready {upload, have}`; `put-chunk {id, offset, data}` must start exactly at `have`; `put-end` checks the size and the SHA-256 and moves the file into place, answering `put-done {file, size}`. A lost connection is resumed with `put-begin` and the same `resume` id. `get-file {file}` answers `file-url {path, expires_in, size, sha256}` and `GET /dl/<token>` serves the bytes (with `Range`). The slots, their mime allow-lists and roles, the size limit and the address lifetime are in `backend.toml` `[files]`. A program reaches an uploaded file with `COBOL::"WS-FILE-PATH" ( file-id path status )` and offers a file for download with `COBOL::"WS-FILE-PUBLISH" ( path file-id status )`.

### 2.7 Deprecation (R6)
`deprecated = { message, removal }` in `exports.toml`: the call succeeds, the reply carries a `warnings` entry, the Rust client hands it to the observer and `COBOL::"WS-WARNING"` reads it.

## 3. Hosts (interpreter / binary parity)
The client is installed **once per process**: `cobolt_server::client::install_default_from_env()` registers a factory with the runtime (`cobolt_runtime::set_default_transport_factory`) that every interpreter — the root one, the embedded child forms', the compiled binary's — asks when it first makes a remote call, and all of them share **one connection**. It is called at the start of `rcrun` (covering `run`, `run-form` and `run-form-ipc`) and in the compiled application's `main`, which links the client only when the program contains a `WITH WEBSOCKET LINKAGE` call (`RuntimeFeatures::remote`; a program without one links no socket code). The environment: `COBOLT_BACKEND_URL` (the `wss://` address), `COBOLT_BACKEND_TOKEN` (`env:`/`file:` allowed; a development token for `rcrun backend run`), `COBOLT_BACKEND_CONTRACT`, `COBOLT_BACKEND_CERT` (an extra trusted root, PEM, for a development certificate). The IDE passes them to Run Form as it passes the other settings; that is the IDE agent's.

## 4. System KB
The builtins `WS-FAIL`, `WS-PROGRESS`, `WS-CANCELLED`, `WS-EMIT`, `WS-FILE-PATH`, `WS-FILE-PUBLISH`, `WS-WARNING`, `WS-UPLOAD`, `WS-DOWNLOAD` are in `BUILTINS`, in the compiler's built-in table and in the section *Calling the backend*; `chunked.data` is regenerated.

## 5. Not done in this work (stated plainly)
- **The browser side.** The generated module's client (WASM-010), the form events for progress, warnings, stream items and upload progress, and the `COBOL`-level syntax for subscribing to a stream are not here: the Rust client has the API (`subscribe`, `upload`, `download`) and the runtime exposes only what a COBOL program of a *desktop* host can use (`CALL … WITH WEBSOCKET LINKAGE`, `WS-WARNING`, `WS-UPLOAD`, `WS-DOWNLOAD`).
- **Versions side by side.** One backend serves one contract version plus the released interface descriptions it projects to; two backend versions at once are a non-goal of the spec.
- **A cancel button during a pending call.** The interpreter thread is blocked in the call, so a form's own `onClick` cannot run; a host cancels through the handle `Interpreter::remote_cancel_handle()`. No host uses it yet.

## 6. Evidence
Run on the final M4 tree, `CARGO_INCREMENTAL=0`, sweeps batched per crate (counts are the tests' own `test result:` totals):

| Suite | Result |
|---|---|
| `cobolt-wire` (all targets) | 23 passed, 0 failed |
| `cobolt-runtime` (all targets, includes `test_backend_hooks`) | 1551 passed, 0 failed |
| `cobolt-compiler` (all targets; the tests marked `#[ignore]` are not in the count) | 197 passed, 0 failed |
| `cobolt-cli` (all targets, includes `backend_cli`) | 54 passed, 0 failed |
| `cobolt-project-tools` | 93 passed, 0 failed |
| `cobolt-semantic` | 157 passed, 0 failed |
| `cobolt-parser` | 222 passed, 0 failed |
| `cobolt-ast` | 73 passed, 0 failed |
| `cobolt-server` (all test targets, default parallel mode, three consecutive runs: 9 + 13 + 10 + 4 + 11 + 15 = 62 passed, 0 failed each time) | 62 passed, 0 failed |
| of which `client_contract` (13 tests: AC1/AC10 versions, AC3 deprecation, AC4 classes and limits, AC6 repeat and idempotency, AC7 ordering, AC8 progress/cancel and streams, AC9 files, token refresh) | 13 passed, 0 failed, 7 consecutive whole-file runs after the fix below (about 18 s each) |
| `cobolt-compiler` `compiled_app_calls_backend` (a compiled application calls the backend; builds a binary, about 25 s) | passed on its own earlier in the milestone (before the upload fix below, which does not touch its path); `#[ignore]` in the ordinary sweep, run with `--ignored` |

Figures the contract tests print (`--nocapture`):
- **AC7, order:** `["INDEP", "SLOW-A", "FAST-B"]` after 0.44 s - the program declared independent overtakes; the two dependent ones keep their order.
- **AC9, upload:** 3,000,000 bytes in 64 pieces of at most 65,536-byte frames, with one forced disconnect, in 0.70 s (4.3 MB/s) (0.84-0.85 s, 3.5-3.6 MB/s in four earlier runs); the SHA-256 of what arrived matches.

**A defect found while collecting this and fixed in the same milestone.** In 2 of the first 9 whole-file runs the AC9 upload test failed with `4001 "begin an upload first"`: the backend keeps an upload's state on the connection that began it, and a chunk (or the end frame) could be the first frame on the reconnected connection. The client treated that Validation answer as final. It now treats it like a lost connection - it begins again with the upload's id and the backend reports how much it holds (`upload_state_lost` in `client.rs`). The test was not changed.

**A second finding, in the test harness.** With the rigs started in parallel, 10 of the 13 `client_contract` tests failed with "the TLS handshake took too long" (the client's 10 s budget, debug build, thirteen servers at once on a loaded machine); one after another they pass every time. `tests/support/mod.rs` now takes the rigs in turn (a per-process lock, released after the server stops; a test that starts two rigs on its own thread is let through). **The cause of the slow handshake under parallel load was not isolated** (a single `loopback` test once took 36 s on a loaded machine and did not repeat in 15 reruns; it may be the same thing). The product's own TLS timeouts are unchanged.

Not run: Windows or Linux; a backend behind a real reverse proxy; a load test beyond the figures above.
