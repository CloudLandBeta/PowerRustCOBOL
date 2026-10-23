<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — `CALL … WITH WEBSOCKET LINKAGE` and the data controls through the backend

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-005-websocket-call-and-remote-data/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **5** — adapt the controls that read a database or the disk so that they reach it through a backend; the `CALL` verb gains a new call
  convention, `WEBSOCKET`:
  `CALL "<function-name>"|identifier WITH WEBSOCKET LINKAGE USING BY VALUE|BY REFERENCE <working-storage group> [RETURNING <working-storage group>]`;
  groups are turned into JSON using the COBOL data structure (hierarchy and tables included), and JSON comes back into the `RETURNING` group or, for `BY REFERENCE`,
  into the group that was sent; the plumbing, **the authentication token included**, is invisible to the developer.
- **Depends on:** WASM-002 (the bundle and host page), WASM-010 (the generated module and its pause-and-resume), WASM-016 (the player, which owns the connection), WASM-003 (the backend and its exported programs), WASM-004 (the control classes). **Needed by:** WASM-009.

## 1. Overview

This spec has two halves that share one mechanism. The **language half** gives `CALL` a calling convention for a remote program: the arguments are COBOL **groups**, they
travel as JSON built from the group's own structure, and the answer is written back into groups — with no code from the developer for connection, token, framing,
serialisation or reconnection. The **controls half** uses that mechanism to make the class C controls of WASM-004 (DataGrid with IndexedFile or SQL bindings, IndexedFile,
SqlDatabase, KnowledgeBase, RestClient, AgentObject, WebSearch, FileDropZone, the AWS controls, and the data operations of Maps and Viewer) work in the browser: the control keeps
its COBOL API; its work runs on the backend.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

**The `CALL` verb**
- Parsed by `parse_call` (`crates/cobolt-parser/src/stmt.rs:3181`); the program operand is an expression, so a literal or an identifier both parse (`:3185`). The AST is
  `Stmt::Call { program, using: Vec<CallArg>, returning, on_exception, not_on_exception, span }` (`cobolt-ast/src/stmt.rs:802`); `CallArg` is `ByReference`, `ByContent` or
  `ByValue` (`:51`), the `BY` word is optional and the mode is sticky (`stmt.rs:3201-3240`). `ON/NOT ON EXCEPTION|OVERFLOW` and `END-CALL` are implemented (`:3250-3290`).
- **`RETURNING` was parsed and ignored** when this was checked: `exec_call` received it as `_returning` (`cobolt-runtime/src/interpreter.rs:11256`) and never used it (the name appeared once). **Fixed since** (2026-10-19): the callee's
  `RETURNING` item is now moved into the caller's identifier — `8a15f00d` on `main` (1.91.219), forwarded as `afe22c51` on 2.x.x; see Q2.
- **There is no calling-convention phrase.** `Token::Linkage` (`cobolt-lexer/src/keywords.rs:51`) is only `LINKAGE SECTION`; nothing parses `WITH … LINKAGE`.
- **Dispatch** (`exec_call`, `interpreter.rs:11252`) is one `match` on the upper-cased name: about 100 `COBOL-*` / `COBOLT-*` built-in arms first (they win over any program of the
  same name), then the nested-program registry (event handlers, a form's procedures — generated as `IS COMMON` nested programs, `cobolt-codegen/src/lib.rs:2399-2475` — and Common
  Code, `cobolt-compiler/src/lib.rs:1519-1525`), then a paragraph of that name, else the call is unresolved and `ON EXCEPTION` runs (`:12628-12647`). The inline `COBOL::"NAME"(args)`
  is rewritten to `CALL "COBOL-NAME"` (`stmt.rs:3407`, `:3933`).
- **The analyser rejects an unknown literal target** unless `ON EXCEPTION` is present or the name starts with `COBOL-`/`COBOLT-` (`cobolt-semantic/src/calls.rs:109-125`); the compiler scans
  literal targets to decide which runtime features to link (`runtime_features.rs:275`).

**Groups and JSON**
- Storage is an `IndexMap<String, CobolValue>` (`environment.rs:129`); a group has no bytes of its own. `ItemSym` (`environment.rs:57`) carries the structure: `dims`, `children`,
  `child_keys`, `layout_keys` (declaration order, FILLER included), `quals`, `occurs`, `depending_on`, `pic`, `pic_decimals`, `pic_kind`, `char_class`. APIs: `group_bytes`, `display_bytes`,
  `set_group_bytes`, `decode_record_slice`, `subscript_key`. `RecordLayout` (`files.rs:63`) is built from the AST `DataDecl` and expands `OCCURS` to its maximum (`:98-103`).
  `USAGE` is not modelled in layouts; `COMP`/`COMP-3` are held as values, not packed bytes. `REDEFINES` is a write-driven refresh between separate slots (`environment.rs:972`), not
  shared bytes. A sign is a trailing overpunch or `SEPARATE`.
- **There is no JSON for groups**: no `JSON GENERATE`/`JSON PARSE`, nothing in the lexer, parser, AST or semantic crates; `serde_json` appears in the runtime for AWS, agents, MCP and
  DataGrid options only. RestClient and `COBOL-HTTP-*` bodies are plain strings the developer builds with `STRING`/`UNSTRING` (`http_runtime.rs:538`, `:619`; `interpreter.rs:3398`, `:3413-3423`, `:11763`).
- **There is no WebSocket client** and no JWT code (WASM-003).

**The class C controls (the runtime path of each)**
- **DataGrid:** `Rows`/`Columns` are in-memory strings; bindings (`Form.data_bindings`, `model.rs:8827`, seeded by `cobolt-form-host/src/seeding.rs:399-500`) are refreshed by
  `refresh_binding_rows` (`interpreter.rs:13494`) for three kinds — IndexedFile (`:13691`), Sql (`interpreter/grid_edit.rs:438`) and CobolTable (`:13555`); Save/AutoCommit write back
  (`grid_edit.rs:378`). `BindingSourceKind::RestApi` and `AgentAi` exist in the model, the IDE and the guardian (`model.rs:2092`) but **the runtime has no refresh path for them**.
- **IndexedFile:** no interpreter methods; codegen emits a COBOL facade (`cobolt-codegen/src/lib.rs:1954`) and the file verbs go to the engines. **SqlDatabase:** `interpreter.rs:17620-17782`,
  `db_runtime.rs`, `esql/`. **RestClient:** `:17303-17350`, `http_runtime.rs`. **AgentObject:** `Ask` `:17267`, `AllowFile`/`RegisterFile` `:17244-17257`. **KnowledgeBase:** `interpreter/kb.rs`.
  **WebSearch:** `:17353`. **Maps:** tile fetch inside the renderer (`cobolt-forms/src/map_tiles.rs:432`), geocode/directions `:17480-17570`. **FileDropZone:** `dropzone.rs:261-272` via
  `CommitFiles` (`:16605`). **Viewer:** `viewer.rs`, `viewer_remote.rs`, Save As `:16884-16920`. **AWS ×16:** `interpreter/aws.rs`, `runtime/aws/` (one path spawns a child process).
  The forms have properties where a key or a connection string can be typed (`AgentAPIKey`, `ConnectionString` and the like); the committed examples checked on
  2026-10-19 keep the key properties empty, but nothing in a form stops a developer from typing one — which is why R14 exists.

## 2. Goals / Non-goals

- **Goals:**
  - `CALL … WITH WEBSOCKET LINKAGE` with `USING` groups, `BY VALUE`/`BY REFERENCE` and `RETURNING`, over the channel of WASM-003.
  - A defined, lossless **COBOL group ⇄ JSON** mapping, in both directions, honouring the declared hierarchy, `OCCURS`, signs and decimals.
  - **Invisible plumbing**: connection, token, framing, serialisation, correlation, reconnection and refresh are not the developer's.
  - The class C controls work in the browser with unchanged COBOL APIs; the developer writes neither end of their remote calls.
  - No SQL text, secret or file path chosen by the browser ever runs on the backend (R17).
- **Non-goals:**
  - A general `JSON GENERATE` / `JSON PARSE` statement for local use (a possible later spec; the mapping here is designed so it can be reused).
  - Streaming, server push, binary payloads, large-file protocols (proposed specs).
  - Changing local `CALL` semantics. (But see Q2: `RETURNING` for local calls is a gap on 1.x.x.)
  - Remote calls through any transport but the one of WASM-003.

## 3. User stories

- As a COBOL developer, I want to write `CALL "GET-CUSTOMER" WITH WEBSOCKET LINKAGE USING BY REFERENCE CUSTOMER-REC` and find the customer in `CUSTOMER-REC` afterwards, so that a
  remote call feels like a local one.
- As a developer, I want a DataGrid bound to an indexed file or an SQL query to fill and save in the browser exactly as on the desktop, so that I do not rewrite my screens.
- As a security reviewer, I want to know that the browser cannot make the server run a query or open a path of its choosing, so that the backend is not a remote console.
- As a developer, I want a failed call to reach my `ON EXCEPTION` with a reason, so that I can tell the user.

## 4. Requirements (EARS)

**The language**
- **R1 (ubiquitous):** The `CALL` statement shall accept
  `CALL {literal | identifier} WITH WEBSOCKET LINKAGE USING [BY VALUE | BY REFERENCE] group-name … [RETURNING group-name] [ON EXCEPTION …] [NOT ON EXCEPTION …] [END-CALL]`.
  The phrase shall be parsed into the AST as a calling convention of the call. Every existing `CALL` form shall parse and run as it does today.
- **R2 (ubiquitous):** Each argument of a `WEBSOCKET` call shall be a **group item** declared in `WORKING-STORAGE` (Q1 on elementary items); the analyser shall reject anything else
  with a line-accurate diagnostic.
- **R3 (ubiquitous):** The modes shall mean: **`BY VALUE`** — the group is sent and the caller's copy is never changed; **`BY REFERENCE`** — the group is sent and the callee's final
  values of that parameter are written back into it; **`RETURNING`** — the callee's `RETURNING` item is written into the named group. A call may combine `BY REFERENCE` parameters and `RETURNING`.
- **R4 (ubiquitous):** The analyser shall accept a `WEBSOCKET` call to a name that the imported interface description declares (R7) without `ON EXCEPTION`, and shall report one that it does not.

**The mapping**
- **R5 (ubiquitous):** A group shall be written as a JSON object whose members follow the group's declared hierarchy and order. Rules, each to be pinned by a test:
  - **Names:** a member's key is the data-name as declared (its spelling preserved); `FILLER` and 88-level condition names are omitted (Q3).
  - **Tables:** `OCCURS n` is a JSON array; for `OCCURS … DEPENDING ON` the array has the current number of occurrences.
  - **Numbers:** a numeric item is a JSON number written from its decimal text with its `PICTURE` scale — never through a binary floating-point value — with its sign; a number whose digits a
    JavaScript number cannot hold exactly is still written in full and read by the module as a decimal.
  - **Text:** an alphanumeric item is a JSON string in UTF-8; trailing spaces are trimmed on output (Q4).
  - **`USAGE`:** `COMP`, `COMP-3` and the like are sent as their numeric value, never as bytes.
  - **`REDEFINES`:** only the redefined (first) item is written (Q5).
- **R5b (ubiquitous):** The mapping shall be implemented **twice, by the same rules**: in the code the generator embeds in the browser module (WASM-010 R6), and in the backend's native runtime. A single normative table
  of rules and one set of **golden vectors** (a group, its JSON text, and the group read back) shall be run against **both** implementations, so that a difference between them is a failing test, not a field in production.
- **R6 (ubiquitous):** JSON shall be read into a group **by name, not by position**, so that a reordered or extended answer still lands correctly; a key absent from the answer leaves the item
  unchanged; an unknown key, a value of the wrong type, a number that does not fit its `PICTURE`, a string longer than its `PIC` or a table longer than its `OCCURS` is an **error**, never a silent
  truncation. A group written and read back shall equal the original.

**The remote contract**
- **R7 (ubiquitous):** The layouts of the arguments shall be shared between the frontend and the backend through copybooks (`COPY`), and the backend's interface description (WASM-003 R4) shall be
  importable by the frontend project so that R4's check, and a layout-compatibility check, can run at Check time.
- **R8 (ubiquitous):** The developer shall write nothing for the connection, the bearer token (WASM-009 supplies it), serialisation, framing, correlation, keep-alive, reconnection or refresh. When
  the token has expired the runtime shall refresh it and repeat the call once before reporting failure.
- **R9 (state):** While a `WEBSOCKET` call is pending, the form shall stay responsive and the statement after the `CALL` shall not run; further events raised on that form shall be queued in order and
  handled when the call has finished (Q6). In the browser this is the module's pause-and-resume of WASM-010 R9; on the backend side nothing waits (a call is a request answered).
- **R10 (event):** When a call cannot be completed — no connection, timeout, authentication failure or expiry, validation failure, an error in the remote program, an unknown function — `ON EXCEPTION` shall
  run, and the program shall be able to read the failure class and a message (a built-in in the inline `COBOL::"…"` form, Q7).
- **R11 (ubiquitous):** The timeout and the maximum payload shall have safe defaults and be configurable per project; the backend enforces its own limits (WASM-003 R16).

**The controls**
- **R12 (ubiquitous):** Every class C control (WASM-004) shall keep its COBOL API — properties, methods, events — on the web; the developer-visible differences shall be latency and the failure modes of R10.
- **R13 (ubiquitous):** A DataGrid with an IndexedFile or SQL binding shall refresh, edit, save and auto-commit through the backend; a CobolTable binding shall stay in the browser. The `RestApi` and `AgentAi`
  binding kinds, modelled but never run today, shall be either implemented through the backend or removed from the web offering (Q8).
- **R14 (ubiquitous):** Controls that hold a connector or a secret — SqlDatabase, RestClient, AgentObject, KnowledgeBase, WebSearch, the AWS controls, Maps geocode and directions — shall run on the
  backend; the frontend shall hold only the **name** of the backend-side configuration, never a connection string, key, token or password, and Build shall fail if one is present (WASM-002 R7).
- **R15 (ubiquitous):** FileDropZone and Viewer shall move files between the browser and the backend with a size limit and a type allow-list, and the backend shall never accept a path chosen by the browser.
- **R16 (ubiquitous):** For each class C control on a web form the build shall generate **both ends** — the frontend's remote calls and the backend's exported programs — from the control's
  configuration; the developer writes neither. The generated endpoints shall require an authenticated caller and shall be restrictable by role (default: authenticated only).
- **R17 (ubiquitous):** The backend shall run only **statements and operations declared at design time** (a named query or file operation with bound parameters); it shall never execute SQL text, a shell
  command, a path or a URL supplied by the browser.

## 5. Acceptance criteria

- [ ] **AC1** — Parser and AST: the new phrase parses in all combinations of R1; every existing `CALL` test still passes; the new phrase with an elementary argument is rejected (or accepted, per Q1).
- [ ] **AC2** — Round trip: for a reference group with nested groups, `OCCURS` and `DEPENDING ON`, signed and decimal fields, `FILLER`, an 88-level, `COMP` and `COMP-3` items and a
  `REDEFINES`, group → JSON → group is the identity; the JSON text is pinned in the test (names, order, number text, trimmed strings).
- [ ] **AC3** — Each failure of R6 (unknown key, wrong type, overflow, over-long string, over-long table) is reported with the item name and writes nothing.
- [ ] **AC4** — Against a loopback backend: `BY VALUE` leaves the caller's group unchanged; `BY REFERENCE` changes it; `RETURNING` fills the named group; both together fill both.
- [ ] **AC5** — With a stopped backend, a wrong token, an expired token (refreshed once, then repeated), a timeout, an unknown function and an error in the remote program, `ON EXCEPTION` runs and the failure
  class is readable; with none of them, `NOT ON EXCEPTION` runs.
- [ ] **AC6** — During a pending call the form repaints and a click on it is queued and handled afterwards, in order.
- [ ] **AC7** — A web form with a DataGrid bound to an indexed file and another bound to an SQL query fills, edits and saves through the backend and shows the same rows the desktop shows for the same data.
- [ ] **AC8** — A browser that sends SQL text, a file path or a URL to a generated endpoint gets a refusal; the backend log shows no execution (R17).
- [ ] **AC9** — A web form carrying a connection string or key for a class C control stops Build; the same configuration on the backend side builds.
- [ ] **AC10** — `cargo test` for the parser, AST, semantic and runtime crates is green; the System KB gains the phrase and the new built-ins and the freshness test passes.

## 6. Constraints & steering check

- **i18n:** the IDE strings (diagnostics, Output) — yes, six languages. Failure messages that reach the end user are the developer's.
- **Generated-code contract:** the generated frontend calls and backend programs (R16) are build artefacts with the banner and regenerate-on-Build rules; codegen writes the inline built-in form
  (`COBOL::"NAME"`) where it needs a built-in (CLAUDE.md, 2026-09-28) and the codegen golden files (`crates/cobolt-codegen/tests/golden`) pin it.
- **COBOL written under this spec** shall use the RustCOBOL extensions (the 2026-10-09 ruling) and a comment block above every `CALL` to a procedure (the 2026-10-12 ruling); an inline built-in needs none.
- **System KB:** **required** — the `CALL` phrase, each new built-in (also in `builtins.rs` `BUILTINS`) and every class C control note; regenerate `chunked.data`.
- **Interpreter/binary parity (the 2026-09-07 ruling):** the runtime change reaches all three hosts that build the interpreter — `rcrun run-form`, embedded child forms and the compiled binary — which now includes the
  **backend** (it runs on that runtime). The browser has **no interpreter**: the generator (WASM-010) must implement the phrase on its own, and R5b ties the two implementations together by golden vectors.
- **Docs:** `docs/` untouched on 2.x.x; planned in WASM-INDEX.
- **Fix vs feature:** **feature**. The part of Q2 about local `RETURNING` is a **fix** (1.x.x, then forward-ported).

## 7. Open questions

- **Q1 — Elementary items.** The operator wrote *group*. Allow an elementary `01`/`77` item as a one-field group (a scalar), or reject it? Recommended: allow; a number or a string is the commonest argument.
  **Decided (operator, 2026-10-22):** accept an elementary `01`/`77` item.
- **Q2 — `RETURNING` on local calls — decided.** It is parsed and ignored today (`interpreter.rs:11256`), which is a gap independent of this series. The operator ruled on 2026-10-19 that it is **a fix on 1.x.x,
  forward-ported to 2.x.x** by the standing rule, so that the two conventions mean the same thing. It is **done**: the fix is `8a15f00d` (1.91.219) on the local `main`, forwarded as `afe22c51` on 2.x.x. It moves the callee's `RETURNING` item into the caller's identifier with ordinary `MOVE` rules (the receiver's `PICTURE` converts, edits and
  truncates; a group moves as a group), and leaves the receiver untouched when nothing is returned (a callee with no `RETURNING` item, a built-in, a paragraph, an unresolved program, or a callee that stops on a run-time error). This spec's `RETURNING`
  semantics (R3) follow that.
- **Q3 — JSON key spelling.** As declared (recommended), upper-case, or lower-camel (`CUSTOMER-NAME` → `customerName`)? A per-item override (a `NAME` clause, as other COBOLs have) is the usual escape hatch.
  **Decided (operator, 2026-10-22):** as declared.
- **Q4 — Trimming.** Trailing spaces trimmed on output and restored (padded) on input; leading spaces and all-space values (`""`) need a rule. Recommended: trim on output, pad on input, empty string for all spaces.
  **Answered (operator, 2026-10-22), to confirm:** "defined". This record takes the recommendation (trim on output, pad on input, empty string for all spaces).
- **Q5 — `REDEFINES`.** Writing only the first item is the safe default; should a `REDEFINES` group be refused as an argument instead?
  **Decided (operator, 2026-10-22):** the safe default: write only the first item; a `REDEFINES` group is not refused.
- **Q6 — Concurrency of events while a call is pending:** queue them (recommended), disable the form's controls, or let handlers run re-entrantly (dangerous)?
  **Decided (operator, 2026-10-22):** queue the events.
- **Q7 — How the failure is read:** a new built-in such as `COBOL::"WS-LAST-ERROR"( class message )`, a special register, or `RETURN-CODE`? Needs the operator's taste; the built-in is the project's convention.
  **Decided (operator, 2026-10-22):** both mechanisms, carrying the same data. Agents (Codex) use the first, the built-in; humans may use either. **Confirmed (operator, 2026-10-23):** the first is the built-in `COBOL::"WS-LAST-ERROR"( class message )`; the second is `RETURN-CODE`, set to the failure's numeric code (for example 7000). `RETURN-CODE` carries only the code; the class and the message are read through the built-in.
- **Q8 — `RestApi` / `AgentAi` bindings:** implement through the backend, or hide on the web and leave to a later spec?
  **Decided (operator, 2026-10-22):** through the backend, except COBOL tables, which stay local.
- **Q9 — Wire format:** one JSON text frame per call (recommended), or a binary framing for large tables? The JSON mapping is the contract either way.
  **Decided (operator, 2026-10-22):** the recommendation: one JSON text frame per call.
