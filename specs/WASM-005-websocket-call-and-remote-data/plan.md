<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — `CALL … WITH WEBSOCKET LINKAGE` and the group <-> JSON mapping

- **Spec:** [spec.md](spec.md). **Status:** M1 and M2 implemented (the language half); the controls half (R12-R17) is not part of this work and is listed under *Not done* below.
- **Written:** 2026-10-20, by the implementing agent while the operator was away. Every decision below was taken without asking; each states what it chose, why, and what the alternative cost — for the operator to overturn.
- **Evidence** is in the final section; nothing is claimed there that was not run.

## 1. Decisions (answering the spec's open questions)

| Q | Decision | Why | Alternative |
|---|---|---|---|
| **Q1** elementary items | **Allowed.** An elementary `01`/`77` is its **bare scalar** on the wire (`12.50`, `"hello"`), not a one-field object. | A number or a string is the commonest argument; the position in the call, not the caller's variable name, identifies a parameter, so there is no name to put in an object. | A one-field object keyed by the caller's name would make the wire depend on a name the callee never sees. |
| **Q3** key spelling | **As declared**, case included, compared exactly. | The developer designs the wire by naming their data; a shared copybook gives both ends the same spelling; Check compares names exactly (a `Cust-Name` against a `CUST-NAME` is reported). | Upper-case: deterministic, ugly, and the browser module cannot recover the developer's intent. The interpreter reads the declared spelling from the AST (the environment upper-cases everything), so this cost the interpreter a declaration lookup (§3.3). |
| **Q4** trimming | **Trailing spaces trimmed on output, padded on input, all spaces is `""`; leading spaces kept.** | The only reversible rule that treats `JUSTIFIED RIGHT` honestly. | Trim both ends: loses the meaning of a right-justified value. |
| **Q5** `REDEFINES` | **Only the redefined (first) item is written; the redefining items and everything below them are not members.** An argument group may contain a `REDEFINES`; it is not refused. | Writing both would double-send the same bytes. | Refuse: forbids copybooks that legitimately overlay a view on a record. |
| **Q6** events during a pending call | **Queued in order; the statement after the `CALL` does not run.** In the interpreter this is how the architecture already works: the interpreter runs on its own thread, the transport blocks that thread, the UI thread keeps painting and its events queue on the channel. | Cheapest, safe, and what R9 asks. | Disabling controls is a UI policy the player (WASM-016) may add on top. |
| **Q7** reading the failure | **A built-in**, `COBOL::"WS-LAST-ERROR" ( class code message [detail] )`. | The project's convention for runtime facilities; testable; leaves `RETURN-CODE` alone. | A special register would need a lexer change. |
| **Q8** `RestApi`/`AgentAi` bindings | **Not part of this work** (the controls half, R12-R17, is the generator's and the controls' business). Recorded under *Not done*. | | |
| **Q9** wire format | **One JSON text frame per call**; chunking and file transfer are WASM-014. | | |

Further decisions the spec did not ask:

1. **A failed call with no `ON EXCEPTION` is a run-time error** (the handler stops; a form's `onUnhandledException` path applies). A `CALL` that silently did nothing would let the program go on with data it never received — the reason the local unknown-program check exists (2026-10-08).
2. **`BY CONTENT` is a parse error in a `WEBSOCKET` call**, with a message that names `BY VALUE`. A copy that is sent and never changed *is* `BY VALUE`; offering both would make two spellings of one thing. The default when no `BY` is written is `BY REFERENCE`, as for every `CALL`.
3. **A WEBSOCKET target is never a local program.** The analyser's "no program has that name" error and the `RETURNING`-from-a-callee-that-returns-nothing warning do not apply to it; the remote-call pass judges the name against the imported interface description instead — and there an `ON EXCEPTION` does **not** excuse a name the contract does not list (a remote name that is not exported can never answer).
4. **The `DEPENDING ON` item of an argument table must lie inside the argument group**, declared earlier (mapping-rules T2): a remote argument is self-contained. A layout that breaks this is refused at Check time.
5. **`COMP-1`/`COMP-2`, `INDEX`, `POINTER`, `OBJECT REFERENCE` and `P`-scaled pictures are refused** (no decimal scale, no value).
6. **Evolution without silent loss:** the strict reader of R6 and the compatibility rules of WASM-014 meet at *projection*: the backend writes an answer only with the members the caller's contract version knew (M4). Without it, adding a member to an answer would break every old client.

## 2. Design

### 2.1 Language (M1)
- **AST:** `CallConvention { Local, WebSocket }` appended as the last variant; `Stmt::Call` gains `convention` as its **last** field (bincode is by ordinal; `Local` is 0, and the test `the_convention_is_the_last_thing_in_the_encoding_and_local_is_zero` pins both).
- **Parser:** `WITH WEBSOCKET LINKAGE` is parsed right after the program operand. `WEBSOCKET` is a **contextual word** (as `REGISTERED`/`USER` are), so a data item may still be called `WEBSOCKET`; `LINKAGE` is the existing token. A `WITH WEBSOCKET` that is not followed by `LINKAGE` is a named error.
- **Analyser** (`cobolt-semantic/src/remote_calls.rs`): R2 (arguments are data items of WORKING-STORAGE/LINKAGE with a wire form), R4 (name against the interface description), R7 (layout fit by `cobolt_wire::compat`). New option `AnalyzeOptions::remote_interface`; the compiler and the coding-agent `validate` read it from `backend/contract/interface.json` (`cobolt_compiler::backend_interface`).
- **Interpreter:** `Stmt::Call` with `WebSocket` goes to `exec_remote_call`; every other call is `exec_call` as before. The call never touches the local dispatch.
- **Transport seam:** `cobolt_wire::call::Transport` — one method, `call(&CallRequest) -> Result<CallReply, CallError>`. The real client (M4), a test double and — as an idea, not code — the generated module all sit behind it.

### 2.2 The mapping (M2)
- **Where it lives:** `crates/cobolt-wire` (new). It knows nothing of egui, sockets, files or the runtime, and is the reference for the generator.
- **Model:** `layout::Item` (from a `DataDecl`) → `node::Node` (the value, shaped like the layout) → JSON text (`mapping::encode`) and back (`mapping::decode`, which builds a **new** tree). Numbers are canonical decimal text (`number.rs`), never an `f64`; JSON is read by a strict reader of our own (`json.rs`) because a general library cannot keep a number as the text it was written as, report a repeated key, or bound depth and size.
- **Rules** are `mapping-rules.md`; **vectors** are `vectors/group-json.v1.json` with `vectors/README.md`.

### 2.3 The interpreter's adapter
- The wire key is the **declared** spelling, but the environment upper-cases names. `interpreter/remote.rs` therefore finds the argument's `DataDecl` in the program's AST: it resolves the name to its environment key, reads the symbol's `origin` (the program that declared it) and its ancestor chain, and walks the AST of that program to the declaration. The layout comes from `layout::from_decl(decl)`; the **values** are read from the environment through the same declaration, leaf by leaf, each key built with `resolve_name(leaf, ancestors)` (so a name declared in two groups resolves to the right one) and, inside tables, `subscript_key`.
- Reply handling is all-or-nothing: every `BY REFERENCE` answer and the `RETURNING` answer are decoded first; only if all of them fit are the changed leaves written. Text leaves are written with `set_verbatim_bytes` (never re-edited, `JUSTIFIED` already applied by the decoder), numeric leaves with `CobolValue::Numeric` at the PICTURE scale.
- A group that was `MOVE`d (so a numeric child holds a byte image) is read through `as_exact`; spaces read as zero, other text is a named error, not a guess.

## 3. Hosts (interpreter / binary parity)
The behaviour is in the shared interpreter, so `rcrun run-form`, embedded child forms, the compiled binary and the backend all have it. What each host must do is **install a transport** (`Interpreter::set_remote_transport`); without one the call fails as `TRANSPORT 1003 no backend configured` — never silently. The default installer arrives with the client (M4) and is wired into the three hosts there; this is stated in M4's plan, not here, because it needs the client.

## 4. System KB
`COBOL::"WS-LAST-ERROR"` and `COBOL::"WS-CLAIM"` are in `builtins.rs` `BUILTINS` (the `the_table_is_every_builtin_the_interpreter_dispatches` test scans `interpreter.rs` for their arms), in the compiler's `rustcobol_extensions.md` table, and a section *Calling the backend — CALL … WITH WEBSOCKET LINKAGE* documents the phrase and the mapping. `assets/knowledge/chunked.data` was regenerated (`cargo run -p cobolt-ide --example build_chunked_kb`).

## 5. Not done in this work (stated plainly)
- **R12-R17, the controls half** (DataGrid/IndexedFile/SqlDatabase/… through the backend, generated endpoints, file moves, the build-time secret scan): these are the controls' and the generator's, need WASM-004/010, and are not part of the assignment that implemented M1-M5.
- **R9 in the browser module** (pause-and-resume) is the generator's.

## 6. Evidence
(Filled in at the end of the work — see the commit messages and the final report for the numbers.)
