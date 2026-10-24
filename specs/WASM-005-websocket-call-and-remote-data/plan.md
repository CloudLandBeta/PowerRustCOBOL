<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — `CALL … WITH WEBSOCKET LINKAGE`, the group ⇄ JSON mapping, and the data controls through the backend

- **Spec:** [spec.md](spec.md) (as it stands on 2026-10-22, with the operator's answers of that day). **Rules:** [mapping-rules.md](mapping-rules.md). **Vectors:** [vectors/](vectors/).
- **Status:** revision 2, 2026-10-22 — draft for review. Revision 1 (2026-10-20) was written by the implementing agent while the operator was away and covered only the language half.
- **Branch / line:** `features-2x`, 2.x.x. Series documents read for this revision: `specs/WASM-INDEX.md`, `specs/WASM-PLAN.md` (W2), `specs/WASM-ABI-v1.md` (§3 `remote`, A6, A8).
- **Evidence rule.** Every statement below about existing code names the file and symbol it was read from on 2026-10-22. No test was run for this revision (running `cargo test` was not permitted in this session); "exists" means the code and the named test functions were read, not that they were seen green.

## 0. What changed in this revision, and why

| # | Change | Why |
|---|---|---|
| C1 | **Q1, Q3, Q5, Q6, Q8, Q9 are now the operator's decisions** (2026-10-22), not the implementer's. Q1, Q3, Q5, Q6, Q9 match what revision 1 chose and what the code does; they are kept and re-labelled as binding. | The spec records them as *Decided (operator, 2026-10-22)*. |
| C2 | **Q8 is now in scope**: the `RestApi` and `AgentAi` binding kinds run **through the backend**; `CobolTable` stays local. Revision 1 put Q8 under *Not done*. | Operator decision of 2026-10-22. |
| C3 | **Q7 is no longer settled by the built-in alone.** The operator decided *both mechanisms, carrying the same data*; the built-in `COBOL::"WS-LAST-ERROR"` exists; the **second mechanism is not built** and which one it is is *to confirm* (§4, open question O1). | Revision 1 chose the built-in only and rejected the special register. |
| C4 | **Q4 stays *to confirm*.** The implemented rule (trim on output, pad on input, `""` for all spaces, leading spaces kept — `mapping-rules.md` S1/S2) is the **working assumption**, not a decision. | The operator's answer is "defined", which the spec records as *to confirm*. |
| C5 | **The controls half (R12–R17, AC7–AC9) is planned here** (§1.3, milestones M7–M12). Revision 1 listed it as *Not done*; it was never out of this spec's scope. | The run asks the plan to match the spec as it stands. |
| C6 | **Implementation status is restated from the code** (§1.1): the client (`cobolt-server/src/client.rs` `WsClient`) and its installation in `rcrun` and in the compiled binary exist, which revision 1 described as future work (its §3 "the default installer arrives with the client (M4)"). | Read on 2026-10-22. |
| C7 | **Three gaps found in what exists**, each turned into a milestone: the WebAssembly generator **ignores the calling convention** (M6); the client's timeout and payload limit are **not a project setting** (R11, M5); **AC6 has no test** in the interpreter host (M5). | §1.1. |
| C8 | **Cross-spec points stated**: DataGrid paging (WASM-004 Q4 decided server paging; WASM-023 is the proposed home of paging/sorting/filtering); D4 (stateless backend calls) still open in WASM-INDEX. | §5. |
| C9 | The layout follows `specs/templates/plan.md`; revision 1's *Evidence* section, which was never filled, is replaced by §6 (what each test asserts). | Template; honesty about what was run. |

Kept unchanged from revision 1: the AST shape, the contextual `WEBSOCKET` word, the `BY CONTENT` refusal, the `cobolt-wire` crate and its strict reader, the adapter's use of the declaration in the AST, all-or-nothing reply handling, and the six "further decisions" (now D7–D12 in §4).

## 1. Approach

### 1.1 What exists (the language half, read on 2026-10-22)

| R / AC | Where it is | State |
|---|---|---|
| **R1** phrase | `cobolt-ast/src/stmt.rs` `CallConvention` (`:67`), `Stmt::Call { …, convention }` (`:833`, last field); `cobolt-parser/src/stmt.rs` (`BY CONTENT` refusal at `:3228`); tests `cobolt-parser/tests/test_websocket_call.rs` (`the_phrase_makes_a_websocket_call_with_every_clause_kept`, `by_content_is_refused_in_a_websocket_call_and_named`, `websocket_without_linkage_is_named`, `websocket_remains_an_ordinary_data_name`) | exists |
| **R2, R4, R7** Check | `cobolt-semantic/src/remote_calls.rs` (module doc: arguments, exported name, layout fit through `cobolt_wire::compat`); `cobolt-compiler/src/lib.rs:1619` `backend_interface` reads `backend/contract/interface.json`; tests `cobolt-semantic/tests/test_remote_calls.rs` | exists; elementary `01`/`77` accepted (Q1) |
| **R3** modes | `cobolt-runtime/src/interpreter/remote.rs` `exec_remote_call` (`:270`); tests `cobolt-runtime/tests/test_remote_call.rs` (`by_value_never_changes_the_callers_copy`, `by_reference_writes_the_answer_back_by_name_and_leaves_absent_members_alone`, `returning_fills_its_group_and_by_reference_is_the_default`, `by_value_and_returning_together`, `an_elementary_item_is_a_bare_scalar`) | exists |
| **R5, R5b, R6** mapping | `crates/cobolt-wire` (`layout`, `node`, `mapping`, `json`, `number`, `compat`, `evolution`); vectors `vectors/group-json.v1.json`; runners `cobolt-wire/tests/vectors.rs`, `cobolt-runtime/tests/test_group_json_vectors.rs`; `a_reply_that_does_not_fit_changes_nothing_in_any_group` | exists for the **native** side; the **browser** side of R5b does not exist (WASM-010 WP5 plans `cobolt-wasm-rt/src/json.rs` and `cobolt-wasm/tests/json_vectors.rs`) |
| **R8** plumbing, refresh once | `cobolt-server/src/client.rs` `WsClient` implements `cobolt_wire::call::Transport` (`:401`); token as first message; expired token refreshed and the call repeated once (`:1004-1005`); `install_default_from_env` (`:495`) registers the process-wide factory `remote::set_default_transport_factory` (`remote.rs:77`); installed by `rcrun` (`cobolt-cli/src/main.rs:79`) and by the compiled binary's generated `main` (`cobolt-compiler/src/lib.rs:4041`, pinned by the test at `:11127`); embedded child forms share the process factory (`remote.rs:73-76` doc) | exists |
| **R9** pending call | interpreter thread blocks in `Transport::call`; the UI thread keeps painting and queues events on its channel (architecture, revision 1 Q6) | exists by architecture; **no test pins AC6** (no reference to the remote call in `cobolt-form-host`) |
| **R10** failures | `RemoteState.last_error` (`remote.rs:89`); `COBOL::"WS-LAST-ERROR"` (`remote.rs:442`, `cobolt-runtime/src/builtins.rs:99`); no `ON EXCEPTION` → run-time error (`remote.rs:292-298`); tests `every_failure_class_reaches_on_exception_with_class_code_and_message`, `success_runs_not_on_exception_and_clears_the_last_error`, `without_a_transport_the_call_fails_as_not_configured`, `a_failure_without_on_exception_is_a_run_time_error` | built-in exists; **second mechanism of Q7 does not** |
| **R11** limits | backend: `[limits]` in `backend.toml` (`call_timeout_ms`, `max_frame_bytes`; `cobolt-server/tests/config_rules.rs`, `loopback.rs` AC6 rig); client: `ClientConfig.call_timeout` default 60 s, `connect_timeout` 10 s (`client.rs:103-121`), max frame from the server's hello (`client.rs:263`) | backend side exists; **the client's values are not a project setting** — `config_from_env` (`client.rs:466`) reads URL, interface, contract, token, certificate and origin only |
| **AC10** KB | `builtins.rs` `BUILTINS` (`WS-LAST-ERROR`, `WS-CLAIM`, `WS-UPLOAD`, `WS-DOWNLOAD`, …); `cobolt-compiler/src/lib.rs:5108` table row, section *Calling the backend — `CALL … WITH WEBSOCKET LINKAGE`* (`:5130`) | exists; must be extended by every later milestone |
| browser module | `cobolt-wasm/src/lower.rs:1785` matches `Stmt::Call { program, using, returning, on_exception, not_on_exception, .. }` — **the `convention` field is dropped**, so `call_` (`lower_call.rs:40`) treats a `WEBSOCKET` call as a local one: a literal name not in the module goes to `call_missing` (`ON EXCEPTION`, or `rt_program_not_found`) | **gap**: silently wrong in a browser module until WASM-010 WP5 lands |

### 1.2 The language half — what remains

- **M5 (R9, R10, R11):** the second failure mechanism of Q7 once confirmed (O1); the client's timeout and maximum payload as a **project setting** read by every host that installs the client; an AC6 test in the interpreter host.
- **M6 (R5b, generator guard):** until WASM-010 WP5 implements the suspension and the rt mapping, `cobolt-wasm` must **refuse** a `WEBSOCKET` call with a line-accurate "not yet supported by the WebAssembly generator" error instead of lowering it as a local call. The match at `lower.rs:1785` binds `convention` and routes `CallConvention::WebSocket` to the refusal. This is a guard, not the feature: the feature (frames, `SUSPEND`, A6 decoding, `json_vectors.rs`) stays WASM-010 WP5's, as its plan says (`specs/WASM-010-…/plan.md` WP5).

### 1.3 The controls half (R12–R17) — the design

One idea carries all of it: **every class C operation is a remote call to a program the Build generates on the backend**, described once in a catalogue that both ends read. The developer's COBOL API does not change (R12); what changes is where the work runs.

1. **The catalogue (`cobolt-wire::controls`, new module).** A static table, one row per *(control type, operation)* of class C (the list is WASM-004's table, `specs/WASM-004-…/spec.md:65`): the operation's parameters and result as `cobolt_wire::layout::Item`s, whether it suspends, which binding kinds it serves, and the backend template that implements it. `cobolt-wire` knows nothing of egui, sockets or the runtime (its crate doc), so the generator, the player, the interpreter and the backend can all depend on it.
2. **The backend end (R16, R17): generated programs.** For each web form, Build walks its class C controls and their bindings and emits, under `backend/generated/` with the generated-code banner and regenerate-on-Build, one COBOL program per used operation, named deterministically from *(form, control, operation)*, plus `backend/generated/exports.toml` with their `[[export]]` entries (roles: the control's `BackendRoles`, default authenticated only — `ExportDecl.roles`/`public`, `cobolt-server/src/backend.rs:39`). The programs use what the native runtime already runs:
   - **SQL binding / SqlDatabase:** `EXEC SQL` (`cobolt-runtime/src/esql/`) with the **design-time** query of the binding (`BindingSourceDescriptor::Sql { query, table_name, key_fields }`, `cobolt-forms/src/model.rs:2194`) compiled into the program; the browser sends only the parameters and the row window.
   - **IndexedFile binding / IndexedFile:** the COBOL facade codegen already emits for a `.cidx` (`cobolt-codegen/src/lib.rs:1954`), called by the generated program.
   - **RestClient, AgentObject, KnowledgeBase, WebSearch, AWS, Maps geocode/directions:** the runtime's existing implementations of those controls (`interpreter.rs` RestClient, AgentObject, WebSearch, Maps arms; `interpreter/kb.rs`; `interpreter/aws.rs`) reached from the generated program with the configuration resolved **on the backend** (§1.3 point 5).
   - **R17** is structural: no generated program takes SQL text, a path, a URL or a command as a parameter; the catalogue's parameter layouts cannot express one (a test walks the catalogue, AC8).
   The developer's own `backend/exports.toml` is never written (user code is sacred); the loader (`cobolt-server/src/backend.rs` `Backend::build`) reads both files and a name in both is a Build error.
3. **The frontend end: one remote-lowering pass, shared by both engines.** The web build already feeds the generator "each form's program as the desktop path builds it: forms regenerated by `cobolt-codegen`, parsed, and with `cobolt_compiler::link_common_code` applied" (WASM-010 plan, WP-build). A new AST → AST pass in `cobolt-compiler` (`web_remote.rs`), run for web/mobile forms only, rewrites a member call or `INVOKE` on a class C control into the statements that move its arguments into a generated argument group, `CALL "<generated endpoint>" WITH WEBSOCKET LINKAGE`, and move the answer to the receiver; the argument groups are declared by `cobolt-codegen` in the form's generated `WORKING-STORAGE`. Because the pass runs **before** either engine sees the program, the interpreter (preview, tests, `rcrun` on a web form) and the generated module execute the same `CALL` — parity by construction, and the only browser-side machinery needed is WASM-010 WP5's.
4. **Bindings driven by the host, not by COBOL (R13, Q8).** A DataGrid refresh, a scroll to a new row window, an edit and `Save`/`AutoCommit` are host-initiated. In the browser the **player** calls the generated `…-READ (offset, count, parameters)` and `…-SAVE (changes)` endpoints on its own connection, with request ids from `0x80000000` (the range ABI A8 already reserves for the player's own calls). In the interpreter, `refresh_binding_rows` (`interpreter.rs:13649`) and the grid write-back (`interpreter/grid_edit.rs`) gain one branch: a binding seeded as **remote** (a `_BindingRemote` flag set by `cobolt-form-host/src/seeding.rs` for web forms) goes through the run's `Transport` to the same endpoints. `CobolTable` is never remote (Q8). `RestApi` and `AgentAi`, which have a descriptor (`model.rs` `BindingSourceDescriptor::RestApi`/`AgentAi`) but no refresh path today (`refresh_binding_rows` returns 0 for them), get **only** the remote path: on the web they run through the backend as Q8 decides; on the desktop they keep doing nothing, which is unchanged behaviour and out of this spec.
5. **Secrets and configuration names (R14, AC9).** A class C control on a web form holds the **name** of a backend-side configuration (new property `BackendConfig`; the value lives in `backend/backend.toml` and the secret store of WASM-003 R17). Build refuses a web form in which a class C control has a non-empty secret-bearing property (`ConnectionString`, `AgentAPIKey`, and the rest named in the catalogue), naming the control and the property, never the value. This is a **structural** check per control, in addition to the shape-based scan that already exists (`cobolt-web-bundle/src/secrets.rs`, which flags `…APIKey/…Password/…Secret/…Token` properties with a value and connection strings **with a password** — a connection string without one passes it today, which R14 does not allow). The same configuration on the backend side builds (AC9).
6. **Files (R15).** FileDropZone's `CommitFiles` and Viewer's open/Save As become `WS-UPLOAD`/`WS-DOWNLOAD` through the backend's **slots** (`cobolt-server/src/files.rs`: the backend names every file, the browser none; per-slot mime allow-list and roles; resumable uploads). The control's `AllowedTypes`/size limit are copied into the generated slot declaration; a slot is generated per control under `backend/generated/`.

## 2. Affected crates / files

| Crate / file | Change | Milestone |
|---|---|---|
| `crates/cobolt-runtime/src/interpreter/remote.rs`, `builtins.rs` | Q7 second mechanism (after O1) | M5 |
| `crates/cobolt-server/src/client.rs` `config_from_env`, `crates/cobolt-project-tools` (project file) | client `call_timeout` / maximum payload from the project | M5 |
| `crates/cobolt-form-host` (test) | AC6: a pending call, a repaint, a queued click handled after, in order | M5 |
| `crates/cobolt-wasm/src/lower.rs` (`Stmt::Call` arm), `lower_call.rs` | refuse `CallConvention::WebSocket` until WP5 | M6 |
| `crates/cobolt-wire/src/controls.rs` (new) | the class C catalogue | M7 |
| `crates/cobolt-codegen` (new module, e.g. `src/remote_controls.rs`) | backend programs, `backend/generated/exports.toml`, slots; argument groups in the form's generated WORKING-STORAGE; goldens in `crates/cobolt-codegen/tests/golden` | M8 |
| `crates/cobolt-server/src/backend.rs` | read `backend/generated/exports.toml` beside the developer's; collision is an error | M8 |
| `crates/cobolt-compiler/src/web_remote.rs` (new), web build path | the remote-lowering pass | M9 |
| `crates/cobolt-runtime/src/interpreter.rs` `refresh_binding_rows`, `interpreter/grid_edit.rs`; `crates/cobolt-form-host/src/seeding.rs` | remote branch for bindings; `_BindingRemote` seeding | M10 |
| `crates/cobolt-web-player` (`net.rs`, the DataGrid view) | the player's grid window/save calls (with WASM-004/016) | M10 |
| `crates/cobolt-forms/src/model.rs`, the IDE properties panel, `crates/cobolt-ide/src/i18n.rs` | `BackendConfig` (and `BackendRoles`) on class C controls in `.wcfrm`/`.mcfrm`; Tr keys ×6 | M11 |
| `crates/cobolt-web-bundle` (or the web build check in `cobolt-project-tools/src/web/mod.rs`) | structural secret check per class C control | M11 |
| `crates/cobolt-compiler/src/lib.rs` doc tables, `assets/knowledge/chunked.data` | KB: Q7 mechanism, every class C note, the generated endpoints | M5, M12 |
| `crates/cobolt-semantic` (tests) | AC1 rows still pinned | — |

No `README.md`, nothing under `docs/` (2.x.x rule). What 2.x.x says about itself stays in `specs/`.

## 3. Data / model changes

- **AST:** unchanged from revision 1 — `CallConvention { Local, WebSocket }`, the last field of `Stmt::Call`; serialised ASTs of 1.x.x decode as `Local` (bincode by ordinal; `Local` = 0).
- **`.wcfrm`/`.mcfrm`:** new properties `BackendConfig` and `BackendRoles` on class C controls, default empty / "authenticated". Desktop `.cfrm` unchanged. A web form saved before M11 loads with the defaults.
- **Generated artefacts:** `backend/generated/*.cbl`, `backend/generated/exports.toml`, the slot declarations — banner, regenerated on Build, never edited by hand.
- **Project file:** the client limits of R11 (key names fixed in M5; defaults: the client's current 60 s call / 10 s connect, the arena's frame size of ABI §2). **Fixed by T3 (2.0.47):** `[web]` / `[mobile]` `call_timeout_ms` (default 60 000, written only when set) and the existing `max_payload_bytes` (WASM-017 R8); the hosts read them as `COBOLT_BACKEND_CALL_TIMEOUT_MS` and `COBOLT_BACKEND_MAX_PAYLOAD_BYTES`.
- **Backend folder:** `backend/generated/` — the Build's programs for bound DataGrids and their `exports.toml` (`[[export]]` only), loaded beside `backend/exports.toml` and `backend/src` (T9, T10, 2.0.50).
  **T11/T12 (2.0.53):** the generated `exports.toml` also carries `[export.control]` (id, class, configuration, `[export.control.properties]`) for a class C method's program and `[[slot]]` (name, extensions, max_bytes, roles) for a FileDropZone — both refused in the developer's `exports.toml`. `backend/backend.toml` gains `[[configuration]]` (`name` + the control's property names, values plain / `env:` / `file:`), and `[files] slots` entries gain the optional `extensions` and `max_bytes`; files written before read unchanged.
- **Wire:** unchanged — one JSON text frame per call (Q9), the envelope of ABI A8, the error frame of A6.

## 4. Key decisions & alternatives

**The operator's decisions (binding, 2026-10-22):**

| Q | Decision | How the plan honours it |
|---|---|---|
| Q1 | accept an elementary `01`/`77` | its **bare scalar** on the wire (mapping-rules E1), as implemented |
| Q3 | key = data-name as declared | mapping-rules G1/G3; the interpreter reads the declared spelling from the AST |
| Q5 | write only the redefined item; a `REDEFINES` group is not refused | mapping-rules G2 |
| Q6 | queue the events | interpreter: by architecture, M5 adds the test; browser: ABI §4/A5 per-instance queue |
| Q8 | `RestApi`/`AgentAi` through the backend; COBOL tables stay local | §1.3 point 4 |
| Q9 | one JSON text frame per call | unchanged |
| Q2 | local `RETURNING` fixed on 1.x.x and forwarded (`8a15f00d` / `afe22c51`) | R3's `RETURNING` follows the same MOVE rules |

**Open — working assumptions, not decisions:**

- **O1 (Q7, *to confirm*):** "both mechanisms, carrying the same data; agents use the first". *Working assumption:* "the first" is the built-in `COBOL::"WS-LAST-ERROR"` (it is first in the spec's list and is what exists). The **second mechanism is not identified**: the spec lists a special register and `RETURN-CODE`. Only a special register can carry class, code **and** message, so *the same data* points to it; `RETURN-CODE` can carry the code only. **Closed 2026-10-23 (operator):** the first is the built-in; the second is **`RETURN-CODE`**, holding the code only — the class and message stay with the built-in.
- **O2 (Q4, *to confirm*):** trim on output, pad on input, `""` for all spaces, leading spaces kept — implemented as the working assumption (mapping-rules S1/S2).
- **O3 (paging):** WASM-004 Q4 decided that bound grids are **paged on the server** and assigns the request shape to WASM-005; WASM-INDEX proposes WASM-023 for server-side paging, sorting, filtering and optimistic concurrency. *Working assumption:* this plan delivers the row window (`offset`, `count`) only; sorting, filtering and concurrency on Save are WASM-023's. To confirm when WASM-023 is accepted.
- **O4 (D4, open in WASM-INDEX):** fresh `WORKING-STORAGE` per backend call. The generated programs of §1.3 are written to be correct either way (they keep no state between calls).
- **O5 (spec R2 text):** R2 still says "group item"; Q1 decided elementary items are accepted. The plan follows Q1; the spec's R2 wording should be aligned at the next `/clarify`.

**Decisions this plan takes (revisitable):**

- **D1 — One catalogue in `cobolt-wire`** for every class C operation. *Why:* four consumers (generator, player, interpreter, backend generator) must agree on names and layouts; one table is the R5b lesson applied to controls. *Rejected:* per-control hand-written endpoints (drift between ends; R16 forbids the developer writing them).
- **D2 — Backend endpoints are generated COBOL**, not Rust handlers in `cobolt-server`. *Why:* the server runs only exported programs (`backend.rs` module doc), so generated programs go through the same authentication, roles, limits, idempotency and logging as the developer's; R16's "both ends generated" is literal. *Rejected:* built-in server routes per control — a second, privileged execution path beside the exports table.
- **D3 — The remote-lowering pass is AST → AST in `cobolt-compiler`, before either engine.** *Why:* interpreter/binary/browser parity by construction (the 2026-09-07 ruling). *Rejected:* teaching the interpreter and the generator separately which member calls are remote (two implementations of one rule).
- **D4 — Generated exports go to their own file**, never into `backend/exports.toml`. *Why:* user code is sacred; regenerate-on-Build needs a file the Build owns.
- **D5 — A structural secret check per class C control**, beside the shape scan. *Why:* R14 forbids *any* connection string or key on the frontend, not only the shapes the scan recognises.
- **D6 — The generator refuses a `WEBSOCKET` call until WP5** (M6). *Why:* today it runs as a local call and lands in `ON EXCEPTION` with no remote attempt — a silent wrong result.

Kept from revision 1 (still right, now numbered):

- **D7** — a failed call with no `ON EXCEPTION` is a run-time error (`remote.rs:292-298`).
- **D8** — `BY CONTENT` is a parse error in a `WEBSOCKET` call; no `BY` means `BY REFERENCE`.
- **D9** — a `WEBSOCKET` target is never a local program; `ON EXCEPTION` does not excuse a name the contract does not list.
- **D10** — a `DEPENDING ON` item must lie inside the argument group (mapping-rules T2).
- **D11** — `COMP-1`/`COMP-2`, `INDEX`, `POINTER`, `OBJECT REFERENCE`, `P` scaling are refused (N3).
- **D12** — evolution by projection: the backend answers with the members the caller's contract version knew (`cobolt-wire/src/evolution.rs`; WASM-014).
- **D13** — `cobolt-wire` has its own strict JSON reader (numbers kept as text, repeated keys and depth reported); `serde_json` cannot keep a number as written.

## 5. Risks & mitigations

- **The browser half of R5b does not exist yet** → the vectors are the contract; WASM-010 WP5's `json_vectors.rs` must pass every vector of `group-json.v1.json` before any web form with a remote call ships; M6's refusal holds the line until then.
- **The controls half spans five specs** (004 classes, 010 lowering, 016 player, 003 backend, 014 contract) → the catalogue (M7) lands first and is the only shared surface; each consumer is a separate milestone with its own test.
- **Generated endpoint names leak into the contract** → names are derived deterministically from form, control and operation, so a rename of a control is a contract change; Check reports it through the existing compatibility rules (`cobolt-wire::compat`, WASM-014).
- **Paging scope (O3)** → only offset/count; a sorted or filtered grid on the web is reported as not available until WASM-023.
- **A remote grid behaves differently from the desktop** (latency, partial windows) → AC7 compares the rows shown for the same data, window by window.
- **System KB drift** → every milestone that adds a built-in, a property or an endpoint updates the compiler's doc tables and regenerates `chunked.data`; the freshness test is a gate.
- **Disk** (52 GiB free on 2026-10-22) → targeted `cargo test -p` runs; no workspace-wide test sweep in a worktree without checking `df` first.

## 6. Test strategy

| AC | Test (crate · file · function) | Asserts / reports | State |
|---|---|---|---|
| AC1 | `cobolt-parser/tests/test_websocket_call.rs` (the four tests of §1.1); `cobolt-semantic/tests/test_remote_calls.rs`; every existing `CALL` test | all R1 combinations parse; an elementary argument is **accepted** (Q1) | exists |
| AC2, AC3 | `cobolt-wire/tests/vectors.rs`, `cobolt-runtime/tests/test_group_json_vectors.rs` | every vector: group → JSON (text pinned) → group is the identity; each R6 error names the item and writes nothing | exists |
| AC4 | `cobolt-runtime/tests/test_remote_call.rs` (BY VALUE / BY REFERENCE / RETURNING / both); `cobolt-server/tests/loopback.rs` | against a double and a loopback backend | exists |
| AC5 | `test_remote_call.rs::every_failure_class_reaches_on_exception_with_class_code_and_message`; `loopback.rs::tokens_are_accepted_and_refused_for_the_right_reasons`, `…size_time_and_rate_limits…`; `client_contract.rs::an_expired_token_is_refreshed_at_connection_and_again_in_the_middle_of_a_session` | each failure reaches `ON EXCEPTION` with a readable class; success runs `NOT ON EXCEPTION` | exists; **M5 adds** the Q7 second mechanism reading the same data |
| AC6 | **new**, `cobolt-form-host` | a scripted transport holds the call; the host repaints; a click is queued and handled after the call, in order | M5 |
| R5b (browser) | `cobolt-wasm/tests/json_vectors.rs` (WASM-010 WP5) | module mapping = `cobolt-wire::mapping` on every vector | WASM-010 |
| M6 | **new**, `cobolt-wasm` | a `WEBSOCKET` call is refused with its line, never lowered as a local call | M6 |
| R11 | **new**, `cobolt-server` client | the project's timeout and payload reach `ClientConfig` | M5 |
| AC7 | **new**, `cobolt-server/tests` + `cobolt-codegen` goldens | a web form with an IndexedFile-bound and an SQL-bound DataGrid fills, edits and saves through a loopback backend; rows equal the desktop path's for the same data; generated programs pinned by golden files | M8–M10 |
| AC8 | **new**, `cobolt-wire` (catalogue walk) + `loopback.rs` | no catalogue parameter can carry SQL, a path, a URL or a command; a frame carrying one to a generated endpoint is refused and the backend log shows no execution | M7, M8 |
| AC9 | **new**, web build check | a class C control with a connection string or key in a `.wcfrm` stops Build naming control and property (never the value); the same configuration in `backend.toml` builds | M11 |
| Q8 | **new**, loopback | a `RestApi` and an `AgentAi` binding fill through the backend; a `CobolTable` binding makes no remote call | M10 |
| AC10 | `cargo test -p cobolt-parser -p cobolt-ast -p cobolt-semantic -p cobolt-runtime`; the KB freshness test | green; KB carries the phrase, the built-ins and every class C note | each milestone |

Manual check (the operator judges the look): the IDE's Build of a web project with a bound DataGrid, its preview, and the generated `backend/generated/` files opened in the editor showing the banner.

## 7. Milestones

| M | Content | State |
|---|---|---|
| M1 | AST, parser, analyser (R1, R2, R4, R7) | exists |
| M2 | `cobolt-wire` mapping, rules, vectors (R5, R5b native, R6) | exists |
| M3 | interpreter adapter, `WS-LAST-ERROR` (R3, R10) | exists |
| M4 | client, hosts, refresh-once (R8) | exists |
| M5 | Q7 second mechanism (`RETURN-CODE`, O1 closed), R11 client limits, AC6 test, KB | to do |
| M6 | generator refusal of `WEBSOCKET` until WASM-010 WP5 | to do |
| M7 | class C catalogue (`cobolt-wire::controls`) | to do |
| M8 | backend generation (programs, generated exports, slots) + loader | to do |
| M9 | remote-lowering pass | to do |
| M10 | remote bindings (IndexedFile, SQL, RestApi, AgentAi) in interpreter and player | to do; player part with WASM-016 |
| M11 | `BackendConfig`/`BackendRoles`, structural secret check, Tr ×6 | to do |
| M12 | System KB: every class C note; regenerate `chunked.data` | to do |

## 8. Steering compliance

- [ ] **i18n:** the new properties-panel labels and Build/Output messages of M11 are `Tr` keys in all six languages. Analyser diagnostics (`cobolt-semantic/src/remote_calls.rs` `err`) are English text like every other analyser diagnostic.
- [ ] **Generated-code contract:** `backend/generated/*` and the argument groups carry the banner and are regenerated on Build; codegen writes inline built-ins as `COBOL::"NAME"`; golden files pin them.
- [ ] **COBOL written under this spec** uses the RustCOBOL extensions (2026-10-09) and a comment block above every `CALL` to a procedure (2026-10-12).
- [ ] **Interpreter/binary parity:** the interpreter change reaches `rcrun run-form`, embedded child forms and the compiled binary (one process-wide client, §1.1) and the backend; the browser gets the same `CALL` from the remote-lowering pass and the mapping is tied by the vectors.
- [ ] **System KB:** doc tables + `builtins.rs` + regenerated `assets/knowledge/chunked.data` in M5 and M12; freshness test green.
- [ ] **Docs:** `README.md` and `docs/` untouched (2.x.x).
- [ ] **Fix vs feature:** feature (Q2's local `RETURNING` was the fix, already done); each commit bumps `z` only.
- [ ] **PRIME DIRECTIVE:** Rust and COBOL only; the generated artefacts are COBOL and TOML.
- [ ] **User code is sacred:** the developer's `backend/exports.toml`, handlers and copybooks are never rewritten; the remote-lowering pass works on the parsed program in the build, not on the source files.
- [ ] **No "cobolt" in user-facing text;** COBOL identifiers English.
