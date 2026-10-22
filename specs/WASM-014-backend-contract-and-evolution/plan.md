<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The backend contract and how it evolves (WASM-014)

- **Spec:** [spec.md](spec.md)   **Date:** 2026-10-22 (revision 2; revision 1 was written 2026-10-20)
- **Status:** draft, revised against the spec as it stands after the operator's answers of 2026-10-22. **The server side and the Rust
  client are built (milestone M4, commits `f305f4a0`, `4a611063`).** The browser's side of the contract, the operator's three changed
  answers (Q1, Q2, Q5) and the wire alignment between the player and the server are **not** built; they are milestones M5–M9 below.
- **Companions:** `specs/WASM-INDEX.md` (D4, D5), `specs/WASM-PLAN.md` (W2 owns this spec; "do not let anything be implemented on a later
  occasion"), `specs/WASM-ABI-v1.md` (§3 `remote`, A6 error frame, A8 connection envelope — A8 says it is "the player's proposal until
  the backend's wire specification (WASM-014) replaces it").
- Every claim about existing code below was read in the tree on `features-2x` at 2.0.30 (`cdad25cd`), file and symbol named.

## 0. What changed in this revision, and why

| # | Change | Why |
|---|---|---|
| C1 | **Q1 is now "both"**: the `WS-FAIL` built-in stays, and a declared **status group in `RETURNING`** is added; agents write only the built-in (§1, D-Status) | Operator, 2026-10-22: *both; agents (Codex) use only the built-in*. Revision 1 had rejected "both" |
| C2 | **Q2:** at-least-once is the stated guarantee **for programs declared safe**; at-most-once for the rest stays the **working assumption** until the operator confirms the scope (§1, §8 OQ1) | Operator, 2026-10-22: *at-least-once*, scope *to confirm*. Not decided silently |
| C3 | **Q5:** idempotency results live **per session**, in **memory or on the server** (a choice in the project's properties), **never in the frontend**; revision 1's fixed 600-second, memory-only store becomes one of the two modes (§2.4, D-Session) | Operator, 2026-10-22 |
| C4 | **Q3 and Q4** confirmed as built: streams are in 2.0; downloads go through a short-lived HTTPS address | Operator, 2026-10-22; also WASM-003 Q3 (HTTPS endpoints: yes) |
| C5 | **The wire is aligned (new M5).** The player (`cobolt-web-player/src/net.rs`) speaks ABI A8 — `{"type":…}`, no `hello`, its own error classes (`client`) and codes (`5`, `7`, `9`), the field `may_have_been_received`; the server speaks `cobolt-wire/src/proto.rs` — `{"t":…}`, a mandatory `hello`, the closed classes of `ErrorClass`, codes from `1000`, the field `may_have_run`. They cannot talk. WASM-003's plan (C6, risk R1) and WASM-009's plan (R4) name WASM-014 as the owner of the fix | Found while grounding this revision |
| C6 | **The browser side is planned, not deferred** (M6–M8): the contract version in the module's manifest, the version and warning events, progress and cancel, stream subscription, upload/download from a web form | Revision 1 listed these under *Not done*; WASM-PLAN forbids leaving them for later |
| C7 | The built-in list is completed: `WS-LAST-ERROR`, `WS-PAUSE` and `WS-CLAIM` exist beside the nine revision 1 named (`cobolt-runtime/src/interpreter/remote.rs`) | Read in the code |
| C8 | Evidence of revision 1 is kept as **history** (§9); nothing was re-run for this revision | Honesty: a plan revision is not a test run |

Kept unchanged because still right: the handshake and `negotiate()`, the compatibility rules and their direction-of-travel check, the closed
error list and codes, projection for older clients, ordering, cancel-as-request, files named by the backend, the hosts' single transport.

## 1. The spec's open questions

| Q | Operator's answer (2026-10-22) | Status | What the plan does |
|---|---|---|---|
| **Q1** application failure | **Both; agents use only the built-in** | Decided | `COBOL::"WS-FAIL" ( code message [detail] )` as built, **plus** an opt-in status group in `RETURNING` (D-Status). Both produce the *same* wire answer, class `application`. The System KB and the agents' prompts teach only `WS-FAIL` |
| **Q2** delivery guarantee | **At-least-once**; whether for every call or only declared-safe programs is **to confirm** | **Open** (OQ1) | Working assumption = R9 as written: a program declared `idempotent` is repeated automatically (at-least-once); any other is not repeated and fails with `transport` + `may_have_run` (at-most-once). The repeat a developer chooses reuses the idempotency key (R10), which the store answers without running twice. If the operator rules "every call", D-Repeat-All (§4) is the ready design, and R9's text must change first |
| **Q3** streams | **In 2.0** | Decided | As built on the server (`stream = true`, `WS-EMIT`); M8 adds the frontend's subscription |
| **Q4** downloads | **HTTPS** | Decided | As built: uploads in pieces over the WebSocket, downloads via `/dl/<token>` |
| **Q5** retention | **Per session; nothing in the frontend; in memory or on the server, configurable in the project properties** | Decided | D-Session (§2.4). The **released interface descriptions** are not runtime retention: R5 itself says they are "kept in the project", and they stay in `backend/contract/released/` |

## 2. Design

### 2.1 Versions and handshake (R1–R3, R16) — built

- Constants: `cobolt_wire::contract::PROTOCOL` and `PROTOCOL_MIN` (both `1`, `contract.rs:32,35`).
- `hello {protocol, contract, client}` → `hello {protocol, protocol_min, contract, accepted: ok|warn|refused, message, update_to, server,
  max_frame_bytes}` (`proto.rs` `Hello`, `ServerHello`, `Accepted`). `cobolt_server::serve::negotiate` (`serve.rs:772`) is a pure function:
  protocol below the window or above it → refused; contract major lower → refused with `update_to`; major higher → refused ("update the
  backend"); same major, client minor higher → refused; lower → `warn` with `update_to`; equal → `ok`. The client maps a refusal to class
  `version`: `6000` too old, `6001` too new, `6002` protocol unsupported (`cobolt-wire/src/error.rs` `codes`).
- **A refusal is still a `hello`** whatever the client's protocol, so an old client reads an update message, never a decoding error (R3).
- **New (M5):** `hello` gains an optional `session` member (§2.4). **Protocol 1 is not frozen until 2.0.0 is released** (the 2.x.x line has
  never shipped), so additions made before the release amend protocol 1; from 2.0.0 on, any change to `hello`, the error list or the file
  frames is a protocol-version change with the R3 window (R16).
- **New (M6):** the frontend's contract version is recorded in the program module's manifest (`prmanifest`, ABI §10: a `contract` member,
  taken from the `backend/contract/interface.json` that Build writes, `cobolt-server/src/backend.rs` module doc) and sent by the player in
  its `hello`.

### 2.2 Compatibility checking (R4, R5) — built, one rule added

- `cobolt_wire::evolution::check_against_released` (`evolution.rs:59`) compares the current interface with the newest released one and names
  each break with program, parameter, item path and rule. Rule names: `member-missing`, `type-changed`, `narrowed`, `table-changed`
  (`compat.rs:37-40` `R_*`), and `parameters-changed`, `returning-removed`, `access-changed`, `roles-changed`, `idempotency-removed`,
  `kind-changed`, `program-removed` (`evolution.rs`). Values are checked **in the direction they travel**: a request with the old client as
  writer, an answer with the new backend as writer and the old client as reader (the old client reads every member it declared).
- A raised major version turns breaks into notes. `rcrun backend check` / `build` / `release` (`cobolt-cli/src/main.rs:520-575`): a release
  build fails on a break; a debug build reports and continues; `release` records `backend/contract/released/<version>.json`.
- **New (M7):** rule `status-changed` — removing, renaming or reshaping a declared status group (D-Status) is a break; adding one is a
  compatible change (an older client reads the same `application` failures it already handles).
- **New (M9):** the IDE shows the report in the Output panel when Build runs the backend check (strings as `Tr` keys, six languages).

### 2.3 Errors (R7, R8)

- **Built:** `ErrorClass` (`error.rs:19`) is the closed list of R7; `ErrorClass::wire` spells it on the wire (`transport`, …, `not-found`,
  …), `word` for COBOL (`NOT-FOUND`), `from_wire` reads an unknown class as `Server`. `CallError {class, code, message, detail, may_have_run}`.
  Application codes are the program's own, `1`–`7999` (`codes::APPLICATION_MAX`); `8000` is the platform's `CANCELLED`. The caller reads a
  failure with `COBOL::"WS-LAST-ERROR" ( class code message [detail] )` (`remote.rs`).
- **New (M7), D-Status — the second way to report an application failure (Q1).** An export may declare
  `status = "<data-name>"` in `exports.toml` (`ExportDecl`, `backend.rs`): the named item is a group inside the program's `RETURNING` item,
  laid out as the platform copybook `WS-STATUS` (`WS-STATUS-CODE PIC 9(4)`, `WS-STATUS-MESSAGE PIC X(200)`, `WS-STATUS-DETAIL PIC X(1000)`),
  which the backend scaffold writes into `backend/COPYBOOKS/` from a Rust constant (`cobolt-project-tools/src/create.rs`, beside
  `BACKEND_STARTER`). When the program returns with a non-zero code the server answers exactly what `WS-FAIL` would have answered: class
  `application`, that code (a code outside `1`–`7999` is answered `server 9001` and logged), message, detail, and **the caller's `BY
  REFERENCE` groups unchanged**. The semantic check refuses a `status` that names no such group. `WS-FAIL` still ends the call at once; the
  status group is read only on a normal return. Both are deliberate, so the platform keeps telling them from a fault (`9001`).
  **Agents:** the System KB section *Calling the backend* states that generated and agent-written COBOL uses `WS-FAIL` only; the status group
  is documented for hand-written programs and for existing COBOL that already returns a status.

### 2.4 Repeating, ordering and the session (R9–R11, Q2, Q5)

- **Built — ordering:** calls of one connection run in the order made unless the export is `independent`; a refused call takes no turn;
  up to `limits.max_calls_per_connection` in flight (`serve.rs:532`); answers in completion order, correlated by id.
- **Built — repeat:** the Rust client (`client.rs` `call_observed`, `:406`) sends every call with a key (`random_hex(16)` unless the request
  carries one). On a lost connection a program whose interface says `idempotent` is sent again once on the new connection; any other fails
  with `transport 1001` and `may_have_run` true if its frame was written. Reconnection backs off 100 ms doubling to 5 s, at most
  `reconnect_deadline` per call.
- **Built — the store:** `cobolt-server/src/idem.rs` `Store`, keyed `(caller, key)` with `caller` = the token's subject or `ip:<addr>`
  (`serve.rs:906`); a repeat with the same fingerprint is replayed without running, one that arrives while the first runs waits, a different
  payload under the same key is `validation 4000`; only successes and application failures are stored. Today: in memory, TTL
  `idempotency_ttl_seconds` (default 600), at most `idempotency_max_entries` (10 000) (`config.rs:505,510`).
- **New (M5), D-Session — retention per session (Q5).**
  - **A session** is a random 128-bit id the client makes when it starts (the page load, the desktop process) and keeps **in memory only**:
    never in `localStorage`, `sessionStorage`, IndexedDB, a cookie or a file. It goes in `hello.session` on every connection, so a reconnect
    continues the session and a repeat after a lost connection finds its stored answer. Nothing about calls stays in the frontend.
  - The store is keyed `(caller, session, key)`. A session's entries end when the client says so (`{"t":"bye"}` on sign-out and on an
    orderly close) or after `session_idle_seconds` (default 900) with no connection carrying that session — a closed tab cannot be relied
    on to say goodbye. `idempotency_max_entries` stays as the bound; `idempotency_ttl_seconds` stays accepted as an optional cap inside a
    session (no default cap), so an existing `backend.toml` keeps starting (`deny_unknown_fields`).
  - **Where:** `idempotency_store = "memory"` (as today; lost on a backend restart, after which a repeat runs again) or `"server"` (a redb
    file under the backend's state directory, which survives a restart; written before the answer is sent). One backend instance per store:
    several instances sharing one store is WASM-019's (operations).
  - **Configured in the project properties:** a `[contract]` section of `backend/backend.toml` (`idempotency_store`,
    `session_idle_seconds`), edited in the Backend part of the project's Web/Mobile settings (`cobolt-ide/src/panels/web_settings.rs`), the
    same pattern WASM-009's plan uses for `[identity]` (its D13). Default `memory` is a **working assumption** (OQ2).
- **The guarantee, stated plainly (working assumption, OQ1):** declared-safe programs are delivered at least once and repeated automatically;
  other programs at most once, and the developer may repeat with the same key, which the store answers within the session without running
  twice. The runtime never asks the backend afterwards whether a call ran.

### 2.5 Long-running calls and streams (R13, R14) — server built, frontend new

- **Built:** a `long_running` export gets `long_call_timeout_ms` instead of `call_timeout_ms` (`serve.rs:925`); `WS-PROGRESS ( percent
  message )` is the safe point, at most ten progress frames a second, the 100 % frame never dropped (`serve.rs:1024`); `{"t":"cancel"}` sets a
  flag `WS-CANCELLED` reads; a program that reaches no safe point is stopped after `cancel_grace_ms` (10 s); the caller reads `application
  8000`; effects are not rolled back. Streams: `sub` → `subscribed`, `event {sub, seq, data}`, `sub-end`, `unsub`; one isolated run per
  subscription, ended (its thread too) on `unsub` or disconnect, so an absent client costs nothing; the event queue is bounded and a slow
  subscriber is ended with a `limit` error. The Rust client re-subscribes after a reconnection and reports the restart (`seq` from 1).
- **New (M8), D-Progress — progress and cancel are shown by the host, not by a handler.** While a form instance's task is suspended in a
  remote `CALL`, the player queues that instance's events (ABI A5), and the desktop interpreter thread is blocked in the call
  (`Interpreter::remote_cancel_handle`, `remote.rs:195`, exists because no handler can run). So the form **declares** before the call where
  progress goes: `COBOL::"WS-SHOW-PROGRESS" ( progress-control [label-control [cancel-control]] )`. During the next long-running call the host
  sets the progress control's value and the label's text from each progress frame, and a click on the cancel control sends `cancel` — no
  COBOL runs mid-call. When the call ends the declaration is spent. Same behaviour in the player and in every interpreter host (the form host
  receives the `RemoteEvent`s through `Interpreter::set_remote_events`, `remote.rs:190`). The "progress event" of R13 is that host event;
  OQ3 asks the operator to confirm this reading.
- **New (M8), streams from COBOL:** `COBOL::"WS-SUBSCRIBE" ( program sub-id status [args…] )` and `COBOL::"WS-UNSUBSCRIBE" ( sub-id )`; each
  item raises the form event **`onStreamItem`** (argument: the `sub-id`) after any running task of that instance is done, and the handler
  reads the item with `COBOL::"WS-STREAM-ITEM" ( sub-id group )`; `onStreamEnd` (argument `sub-id⇥reason`) when the program ends, the client
  unsubscribes or the limit ends it. Subscriptions belong to the form instance and end when it closes. After a reconnection the client
  re-subscribes and `onStreamItem` continues; the handler can read `WS-STREAM-ITEM`'s restart flag. The closed inline-call vocabulary is not
  touched: these are `COBOL::"…"` built-ins, not new methods.

### 2.6 Files (R15) — server built, browser new

- **Built:** `put-begin {slot, name, size, mime, sha256, resume}` → `put-ready {upload, have}`; `put-chunk {offset, data}` must start at
  `have`; `put-end` checks size and SHA-256 → `put-done {file, size}`; resume with the same `resume` id; a chunk that lands on a fresh
  connection makes the client begin again (`client.rs` `upload_state_lost`). Names: the browser's name is display-only and refused with `/`,
  `\`, `..`, NUL or a control character; stored as `<files.dir>/files/<random id>`. Download: `get-file` → `file-url {path, expires_in, size,
  sha256}`, `GET /dl/<256-bit token>` with `Range`, tied to subject and source address, `download_ttl_seconds` (60), one `404` for unknown,
  expired or foreign. Slots, MIME allow-lists, roles and limits in `backend.toml` `[files]` (`config.rs` `FilesSection`). Backend side:
  `WS-FILE-PATH`, `WS-FILE-PUBLISH`; desktop client side: `WS-UPLOAD`, `WS-DOWNLOAD` (`remote.rs`).
- **New (M8):** the player implements the same `put-*` and `get-file` frames, reading the file from an `<input type=file>` or drop
  (`onFilesDropped`, ABI §12.2) in pieces (`File.slice`), hashing as it goes, resuming after a reconnection; the download is the browser's own
  navigation to the issued address. `WS-UPLOAD` / `WS-DOWNLOAD` keep their desktop signatures; in a web form the `path` argument names the
  picked file (a handle the player gives), never a path the module could choose. Upload progress uses the D-Progress declaration.

### 2.7 Deprecation and the version events (R2, R6)

- **Built:** `deprecated = { message, removal }` in `exports.toml`; the reply carries `warnings`; the Rust client hands them to the observer;
  `WS-WARNING ( message )` reads the latest.
- **New (M6):** two application events raised on the **main form** (the player in a page and the desktop host alike):
  **`onBackendVersion`** after each `hello` that is not `ok` (argument `warn|refused⇥message⇥update-to`; a refused connection makes every
  later call fail `version 600x` until the application is updated) and **`onBackendWarning`** after a call whose reply carried a warning
  (argument: the program name; the text through `WS-WARNING`). On mobile the handler may `COBOL::Navigate` to the store page; where the store
  address comes from is WASM-012/WASM-025's.

### 2.8 The wire, aligned (C5) — new, M5

One protocol, `cobolt-wire/src/proto.rs`, is the contract; ABI A8 is superseded by an amendment row **A10** in `specs/WASM-ABI-v1.md`:

- The player opens with `hello` (with `session`), then `{"t":"auth","token":…}`, then `{"t":"call",…}` with `key` — the `type` envelope
  and the JSON `ping`/`pong` of A8 are retired (keep-alive, if needed, uses WebSocket control frames, which browsers answer natively).
- **Frame-in for the module** (A6) carries the wire's error object: `class` spelled as `ErrorClass::wire`, the wire's codes, `detail`,
  `may_have_run`. The player's own failures map onto the closed list: not connected → `transport 1000`, lost → `transport 1001`
  (`may_have_run` true once the frame was written), timeout → `transport 1002`, frame over the limit → `limit 7000`, a frame that is not
  UTF-8 or JSON → `validation 4001` (a generator bug, reported with the module's line). The class `client` and the field
  `may_have_been_received` disappear.
- Token expiry: the server's `authentication 2002` (`TOKEN_EXPIRED`) replaces A8's `"expired":true`; the player refreshes and repeats once.
- The player's own requests (A8's ids from `0x80000000`) keep that id range.
- **The interface the player is given** (for `idempotent`, `long_running`, `stream`) is the one the module's manifest carries (§2.1).

### 2.9 Hosts (interpreter / binary parity) — built, plus the new built-ins

`cobolt_server::client::install_default_from_env()` registers a factory with `cobolt_runtime::set_default_transport_factory`
(`remote.rs:77`); every interpreter — root, embedded child forms, the compiled binary — shares one connection. `rcrun` installs it at start;
the compiled application links it only when `RuntimeFeatures::remote` (`cobolt-compiler/src/runtime_features.rs:142`). Every new built-in
(`WS-SHOW-PROGRESS`, `WS-SUBSCRIBE`, `WS-UNSUBSCRIBE`, `WS-STREAM-ITEM`) and event is implemented in the interpreter **and** in the player,
and tested in `rcrun run-form`, an embedded child form, the compiled binary and the browser.

## 3. Affected crates / files

| File | Change | M |
|---|---|---|
| `crates/cobolt-wire/src/proto.rs` | `Hello.session`; `Bye` frame | M5 |
| `crates/cobolt-wire/src/contract.rs`, `evolution.rs` | `status` in the program signature; rule `status-changed` | M7 |
| `crates/cobolt-server/src/idem.rs` | key `(caller, session, key)`; session end and idle expiry; `memory` / `server` (redb) back ends | M5 |
| `crates/cobolt-server/src/config.rs` | `[contract]` `idempotency_store`, `session_idle_seconds`; `idempotency_ttl_seconds` an optional cap | M5 |
| `crates/cobolt-server/src/serve.rs` | session from `hello`; `bye`; status-group answer | M5, M7 |
| `crates/cobolt-server/src/backend.rs` | `ExportDecl::status` | M7 |
| `crates/cobolt-server/src/client.rs` | session id per client, `bye` on close | M5 |
| `crates/cobolt-server/Cargo.toml` | `redb`, the **same pinned version** as `cobolt-runtime` (`=4.3.0`), so one copy is linked | M5 |
| `crates/cobolt-web-player/src/net.rs` | the aligned wire (§2.8); session; keys; repeat for safe programs | M5 |
| `crates/cobolt-web-player/src/` (host, api) | version/warning events, D-Progress, streams, files | M6, M8 |
| `crates/cobolt-wasm` (`module.rs`, manifest via `cobolt-wasm-abi`) | `contract` in the manifest; the frame-in error decoder follows A10 (WASM-010 WP5 does the decoder) | M6 |
| `crates/cobolt-runtime/src/interpreter/remote.rs`, `builtins.rs` | the new built-ins; `onBackendVersion` / `onBackendWarning` / stream events through the form host | M6, M8 |
| `crates/cobolt-semantic` | `status` names a group of the `WS-STATUS` shape inside `RETURNING` | M7 |
| `crates/cobolt-project-tools/src/create.rs` | scaffold writes `backend/COPYBOOKS/WS-STATUS.cpy` (from a Rust constant) and `[contract]` defaults in `backend.toml` | M7 |
| `crates/cobolt-ide/src/panels/web_settings.rs`, `i18n.rs` | Backend settings fields; compatibility-report strings; `Tr` keys ×6 | M9 |
| `crates/cobolt-compiler/src/lib.rs` (doc tables, built-in table), `assets/knowledge/chunked.data` | System KB for every new built-in, event, declaration and error rule | M9 |
| `specs/WASM-ABI-v1.md` | amendment A10 (wire aligned, error frame), event rows in §12.2 | M5, M6 |

Not touched: `README.md`, anything under `docs/` (2.x.x rule).

## 4. Key decisions and alternatives

- **D-Status (Q1 both):** a declared status group, opt-in per export, answered identically to `WS-FAIL`. *Rejected:* recognising a status
  group by name or shape without a declaration — the platform would guess, and a program that merely returns a field called `STATUS` would
  start failing calls.
- **D-Session (Q5):** a client-made session id in `hello`, memory-only on the client. *Rejected:* the connection as the session (a reconnect
  would lose the very answer a repeat needs); the token as the session (a refreshed token would split it; an anonymous caller has none);
  persisting keys in the browser (forbidden by the answer).
- **D-Progress:** declared host-side display and cancel. *Rejected:* running a progress handler while the same instance's task is suspended —
  an ABI change to A5 and a re-entrant interpreter on the desktop, for a display a declaration gives.
- **D-Wire:** `proto.rs` wins over A8. *Rejected:* teaching the server A8 — the server's protocol has the handshake R2/R3 require, A8 has none.
- **D-Repeat-All (ready, not adopted — OQ1):** if the operator rules at-least-once for every call, the client repeats every call once after a
  lost connection with the **same key**, and the session store turns the repeat into a replay when the first run finished; R9's text changes
  first; a backend restart in `memory` mode then runs a non-safe program twice, which the plan would have to state in the KB.

## 5. Risks and mitigations

- **Two protocols today.** Until M5 lands, no browser can call a backend (WASM-PLAN end-to-end fact 2). → M5 is first; a test runs the player's
  `net.rs` machine against a real `cobolt-server` rig.
- **Store on disk.** The `server` mode adds writes on the call path. → one redb transaction per stored answer; measured in the contract test;
  `memory` stays available.
- **A second redb dependency.** → the same exact pin as `cobolt-runtime`; the dependency guard (`cobolt-server/tests/dependency_guards.rs`)
  asserts one redb 4.x in the backend graph.
- **Idle expiry versus a slow user.** A tab left 15 minutes loses its stored answers. → configurable; the KB states it.
- **Harness load.** Revision 1 found TLS handshakes timing out with thirteen rigs in parallel; the cause was not isolated
  (`tests/support/mod.rs` serialises rigs). → keep the serialisation; new rigs use it.
- **Untested platforms.** Nothing ran on Windows, Linux or behind a reverse proxy. → listed in the ledger, never claimed.

## 6. Test strategy (by acceptance criterion)

| AC | Test (crate) | Asserts and reports |
|---|---|---|
| AC1 | `negotiate` table test (`serve.rs`); `client_contract` (cobolt-server); **new** player `net.rs` against a rig | 1.0 vs 1.2 accepted with warning; major differs refused with update message; protocol below window refused as `version 6002` with text, never a decode error; the player raises `onBackendVersion` |
| AC2 | `evolution.rs` unit tests; **new** case per R4 change incl. `status-changed`; old-client-against-new-backend in `client_contract` | each compatible change builds and the old client works; each break fails `rcrun backend build --release` naming program, item, rule |
| AC3 | `client_contract` deprecation; **new** player test | call succeeds; `WS-WARNING` reads it; `onBackendWarning` raised (desktop and player) |
| AC4 | `client_contract` classes; **new** player failure mapping test | each class produced and read with its code; unknown class → `server`; no `client` class, no `may_have_been_received` remains (grep test) |
| AC5 | `client_contract`; **new** status-group test | `WS-FAIL` and a declared status group both arrive in `ON EXCEPTION` with code and detail, parameters unchanged |
| AC6 | `client_contract`; **new** session tests (both store modes, backend restart in each) | safe program repeated once and succeeds; other fails with `may_have_run`; same key replays without a second run; a new session does not see another's answers; `server` mode survives a restart, `memory` does not; nothing written to browser storage (player test inspects the storage APIs it calls) |
| AC7 | `client_contract` order test | prints the order (revision 1: `["INDEP","SLOW-A","FAST-B"]`) |
| AC8 | `client_contract`; **new** D-Progress and stream tests in the interpreter and the player | progress reaches the declared controls; cancel → `application 8000`; stream re-subscribes after a forced disconnect; server thread count back to baseline after `unsub` |
| AC9 | `client_contract` upload; **new** player upload test (mock `File`) | 3 MB in pieces with a forced disconnect, SHA-256 matches; `../` refused; expired address `404` |
| AC10 | **new** end-to-end: generated module + player + backend, both skew directions in the window | each AC above holds across the skew |
| Parity | `rcrun run-form`, embedded child form, compiled binary (`compiled_app_calls_backend`, `#[ignore]`, run with `--ignored`) | the new built-ins behave the same in every host |
| KB | the System KB freshness test (`chunked.data`) | red until the doc tables and the store are regenerated |

Manual: open a Web project, set the Backend settings in each of the six languages and check the fields do not overflow; run Build with a
deliberate break and read the Output panel report.

## 7. Steering compliance

- [ ] **i18n:** settings fields and the compatibility report as `Tr` keys in EN/ES/PT/JA/ZH/FR; wire messages stay codes + English.
- [ ] **Generated code:** interface description, compatibility report, `WS-STATUS.cpy`, the manifest's `contract` and the generated clients'
  handling are generated artefacts with the banner, regenerated on Build/Run, never hand-edited.
- [ ] **System KB:** compiler doc tables, `builtins.rs` `BUILTINS`, regenerated `assets/knowledge/chunked.data` in the same change.
- [ ] **Parity:** every runtime change reaches `rcrun run-form`, embedded child forms, the compiled binary, the backend and the player.
- [ ] **PRIME DIRECTIVE:** no non-Rust source added; the copybook is written by the scaffold from a Rust constant.
- [ ] **User code is sacred:** an existing `backend.toml` keeps starting; `exports.toml` is edited only where the developer asks.
- [ ] **Docs:** no `README.md` or `docs/` change on 2.x.x; what 2.x.x says lives in `specs/`.
- [ ] **Fix vs feature:** feature on `features-2x`; bump `z` in `crates/cobolt-ide/src/version.rs` with a `CHANGELOG.md` entry per change.
- [ ] **COBOL written for this** follows the extensions ruling and the call-comment ruling; agents use `WS-FAIL` only.

## 8. Open questions

- **OQ1 (spec Q2, "to confirm"):** at-least-once for **every** call, or only for programs declared safe? Working assumption: only declared
  safe (R9 as written). D-Repeat-All is ready if the answer is "every call"; R9 must change first.
- **OQ2 (from Q5):** the default store mode when the developer has not chosen. Working assumption: `memory`.
- **OQ3 (R13 reading):** is a host-shown progress declaration (D-Progress) an acceptable "progress event the form can show", given that no
  handler can run while its form waits on the call? Working assumption: yes.
- **Housekeeping, not a question:** `specs/WASM-INDEX.md` still lists D4 and D5 as *Open*; WASM-003 Q2 and Q3 were decided on 2026-10-22.
  That file is not edited in this run.

## 9. Evidence (revision 1, 2026-10-20 — history, not re-run for this revision)

Run on the M4 tree, `CARGO_INCREMENTAL=0`: `cobolt-wire` 23 passed; `cobolt-runtime` 1551; `cobolt-compiler` 197 (ignored tests excluded);
`cobolt-cli` 54; `cobolt-project-tools` 93; `cobolt-semantic` 157; `cobolt-parser` 222; `cobolt-ast` 73; `cobolt-server` 62 (three
consecutive runs), of which `client_contract` 13, seven consecutive whole-file runs; `compiled_app_calls_backend` passed on its own earlier
in M4. Figures: AC7 order `["INDEP", "SLOW-A", "FAST-B"]` in 0.44 s; AC9 3,000,000 bytes in 64 pieces with one forced disconnect in
0.70 s, SHA-256 matching. Two findings fixed in M4: an upload chunk landing on a fresh connection (`upload_state_lost`), and parallel rigs
timing out the TLS handshake (rigs serialised in `tests/support/mod.rs`; root cause not isolated). Not run: Windows, Linux, a reverse proxy,
a load test.
