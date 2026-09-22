# Tasks — `cobolt-mcp`: the Model Context Protocol server

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-09-22

Ordered so the workspace stays green after every task. T1–T5 build the protocol
crate standing alone; T6–T11 add the tool in the runtime; T12–T13 wire the two
front doors; T14–T16 close out.

**Prerequisite, already met:** the `.cidx` delivery staging shipped as fix
1.70.146 (`87cb17f`). It is on `fixes` and not yet merged to `main`, so this
branch does not carry it — not needed to compile or unit-test anything below,
needed only for an end-to-end delivery check (T15).

---

- [x] **T1 — Create the `cobolt-mcp` crate, empty but real** (R1, R2, R3)
  - Files: `Cargo.toml` (workspace members), `crates/cobolt-mcp/Cargo.toml`,
    `crates/cobolt-mcp/src/lib.rs`
  - Do: register the member after `cobolt-dap`, mirroring its comment style.
    Dependencies are **exactly** `serde` and `serde_json` from the workspace —
    nothing else. Module doc states the `cobolt-dap` contract: no COBOL, no
    egui, no filesystem, no `cobolt-*`, so the IDE, a built application and a
    test can each link it without linking one another.
  - Verify: `cargo build -p cobolt-mcp`; then
    `cargo tree -p cobolt-mcp` shows serde, serde_json and their transitive
    deps only — **no `cobolt-*`, no TLS, no HTTP crate** (AC1).

- [x] **T2 — JSON-RPC envelope types** (R1, R6, R7)
  - Files: `crates/cobolt-mcp/src/types.rs`
  - Do: request, response, notification and error envelopes per JSON-RPC 2.0.
    Bodies we do not model stay `serde_json::Value` — the `cobolt-dap` rule, so
    an unknown request survives a round trip instead of being dropped (R7).
  - Verify: `cargo test -p cobolt-mcp` — round-trip tests for each envelope, and
    one asserting an unmodelled body is byte-equal after decode+encode (AC5).

- [x] **T3 — Framing over a byte stream** (R4)
  - Files: `crates/cobolt-mcp/src/transport.rs`
  - Do: read/write one message over `BufRead`/`Write`, generic exactly as
    `cobolt-dap::transport` is. A clean EOF before a message is the peer
    closing; EOF mid-message is an error, because a silently short read is
    indistinguishable from a valid small one. Cap message size rather than
    allocating whatever a peer claims.
  - Verify: `cargo test -p cobolt-mcp` — round trip; truncated stream errors;
    clean EOF yields `None`; an oversized declared length is refused.

- [x] **T4 — MCP handshake and tool methods** (R1, R6)
  - Files: `crates/cobolt-mcp/src/types.rs`, `crates/cobolt-mcp/src/server.rs`
  - Do: `initialize` with protocol-version negotiation and `serverInfo`,
    `notifications/initialized`, `tools/list`, `tools/call`. Full wire
    compatibility (spec §7 Q4) — a real client must connect without
    special-casing.
  - Verify: `cargo test -p cobolt-mcp` — a scripted client completes the
    handshake and lists tools; version negotiation returns the agreed version
    (AC3).

- [x] **T5 — `McpHandler` + `serve` loop** (R1, R4, R6)
  - Files: `crates/cobolt-mcp/src/server.rs`
  - Do: `pub trait McpHandler` (list tools, call a tool) and
    `pub fn serve<R: BufRead, W: Write, H: McpHandler>`. An unimplemented method
    returns a JSON-RPC error and the loop **continues** — never closes the
    connection (R6).
  - Verify: `cargo test -p cobolt-mcp --no-fail-fast`, every `test result:` line
    read. An unknown method returns an error and the next request still succeeds
    on the same stream (AC4).

- [x] **T6 — `cobolt-mcp` joins `SDK_CRATES`** (R5)
  - Files: `crates/cobolt-compiler/src/lib.rs`
  - Do: add `"cobolt-mcp"` to `SDK_CRATES`, keeping the array's declared length
    and alphabetical order.
  - Verify: `cargo test -p cobolt-compiler --lib -- sdk_` — all four guards
    green: `sdk_manifest_members_match_the_copied_crates`,
    `sdk_crate_set_is_closed_under_path_dependencies`,
    `sdk_covers_every_compile_time_asset`, `sdk_extra_paths_all_exist` (AC2).
    The closure guard is the one that matters: it proves the enlarged set still
    has no dangling path dependency.

- [x] **T7 — Read descriptions from a delivered `.cidx`** (R8, R9, R10, R27,
      R29, R30, R31)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs` (new),
    `crates/cobolt-runtime/src/lib.rs`
  - Do: a narrow `FileDescription { name, purpose, columns: Vec<(name, text)> }`
    read through `cobolt_forms::assets::resolve` + `cobolt_indexed::load_indexed`
    — the same pair the DataGrid binding already uses, so the anchoring is the
    one that already works. **Take only descriptive fields** (R30): no offsets,
    lengths, keys, record format or storage mode ever leave this function. A
    missing or malformed file yields `None` and a warning, never a panic (R31).
  - Verify: `cargo test -p cobolt-runtime --lib -- mcp_tool` — purpose and a
    column description equal what the fixture `.cidx` holds (AC6); a malformed
    file yields `None` (AC21, first half).

- [x] **T8 — Generate the tool schema from those descriptions** (R12, R13, R14)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`
  - Do: one tool per consultable file (plan §4), the file's `comment` as the
    tool description and each column's `comment` on the matching parameter. The
    text is never copied into a second place — the `.cidx` is the only author
    (R14).
  - Verify: `cargo test -p cobolt-runtime --lib -- mcp_tool` — the generated
    schema carries the file purpose and the per-column text (AC8); editing the
    fixture's description changes the schema with no other file touched (AC7).

- [ ] **T9 — The consultable list gates everything** (R16, R32)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`
  - Do: read the application-side marking (spec Q3/R32). **Absent or empty means
    nothing is consultable** — never "everything", so a half-built application
    publishes no data by accident (plan §3).
  - Verify: `cargo test -p cobolt-runtime --lib -- mcp_tool` — an unmarked file
    appears in neither discovery nor results even when its description is the
    best match (AC10); an absent list exposes nothing.

- [ ] **T10 — Search through `IndexedStore`** (R17, R18, R24, R25, R26)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`
  - Do: execute the search through the same `IndexedStore` surface the COBOL
    verbs use — never a private path to the bytes (R25). Read-only: no `WRITE`,
    `REWRITE` or `DELETE` (R18). Results identify the answering file and the
    matched records (R17). Bound the work and report truncation (R26). Files
    open `STORAGE MODE IS MEMORY` (R24).
  - Verify: `cargo test -p cobolt-runtime --lib -- mcp_tool` — a result names
    its file and records (AC11); fixture bytes are identical before and after a
    search (AC12); a large fixture is bounded and reports truncation (AC16); an
    in-memory file is searchable (AC15).

- [ ] **T11 — Empty result is not an error** (R19)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`
  - Do: a search matching nothing says so explicitly, distinguishable from a
    failure.
  - Verify: `cargo test -p cobolt-runtime --lib -- mcp_tool` — the no-match case
    and the error case produce different, checkable outcomes (AC13).

- [ ] **T12 — `McpHandler` impl: the over-the-wire front door** (R21, R22)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`,
    `crates/cobolt-runtime/Cargo.toml`
  - Do: implement `cobolt_mcp::McpHandler` by **delegating** to the functions
    T8–T11 built. The impl holds no logic of its own (R22).
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` — a scripted client
    over `serve` lists tools and calls one, receiving a well-formed result
    (AC3, second half).

- [ ] **T13 — The in-process front door, and the parity guard** (R20, R22, R23)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    `crates/cobolt-runtime/tests/` (new integration test)
  - Do: one `exec_call` arm beside `COBOL-HTTP-*`, calling the **same** function
    the handler calls — no serialization, no port. Verb name English and
    COBOL-shaped.
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` — the parity test
    drives the same question both ways against one fixture and asserts the
    results are equal (AC14). This is the guard that stops R22 rotting, modelled
    on `engine_reference_form_parity_static_vs_faces`.

- [ ] **T14 — R30 under attack** (R30)
  - Files: `crates/cobolt-runtime/tests/` (new integration test)
  - Do: a test that edits a *delivered* `.cidx` to claim a different offset, key
    and record length, then reads records.
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` — only the reported
    description changes; every record reads identically (AC20). Without this the
    layout/description split is a comment, not a property.

- [ ] **T15 — Delivery staging still holds** (R26 of the fix, AC18, AC19)
  - Files: none (verification only), unless a gap appears
  - Do: confirm 1.70.146's staging covers what this feature needs — declared
    definitions present at their project-relative paths, undeclared absent.
  - Verify: `cargo test -p cobolt-compiler --lib -- indexed_definition` (AC18,
    AC19). **Needs `fixes` merged to `main` to exercise end to end** — if it is
    not merged when this task runs, say so rather than reporting a pass this
    branch cannot produce.

- [ ] **T16 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, and the five
    `docs/developers-guide-<lang>.md` **deleted**
  - Do: document the MCP server for a developer. **Carry the two parked
    sentences with it** — F1's (1.70.145) and the stale "ship the `indexed/`
    folder by hand" passage (1.70.146), which this feature's area rewrites
    anyway. Under GOLDEN RULE #8 update the English canonical and **delete** its
    five translations; never edit a translation, never delete an English file.
  - i18n: **no new `Tr` keys expected** — R32 keeps the marking out of the IDE.
    If `/implement` finds it needs one, that is a divergence to surface, not to
    absorb.
  - Verify: `cargo test -p cobolt-ide i18n`. Two `docs_embed.rs` guards go
    **red** — `every_document_ships_in_every_language` and
    `every_translation_is_complete_and_current`. That red is intended until the
    next minor regenerates them; **do not re-add their `#[ignore]`**.

- [ ] **T17 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: one `z` bump for the whole job (never per task) and a dated CHANGELOG
    entry. **Feature** → `features` branch; never mixed with a fix commit.
  - Verify: `cargo test --workspace --no-fail-fast`, reading **every**
    `test result:` line — never verdict from a grep for failures. Known
    environmental failures to report rather than chase:
    `test_external_crates_e2e` (2 cases, vendors from crates.io; verified
    pre-existing at 1.70.146) and `libsqlite3-sys`. `cobolt-forms` needs
    `--features render`; `cobolt-ide` needs `--bin`.
  - Then confirm every acceptance criterion AC1–AC21 has a green verification
    behind it, and say which ones do not.

## Done criteria

All AC1–AC21 checked with real, measured results; the workspace green apart from
the named environmental failures; the Guide updated with its five translations
deleted; and the work committed as a **feature** on `features`, separate from
any fix, with no commit or push unless the operator asks.

## Coverage map

| AC | Task | | AC | Task |
|---|---|---|---|---|
| AC1 | T1 | | AC12 | T10 |
| AC2 | T6 | | AC13 | T11 |
| AC3 | T4, T12 | | AC14 | T13 |
| AC4 | T5 | | AC15 | T10 |
| AC5 | T2 | | AC16 | T10 |
| AC6 | T7 | | AC17 | T7 *(no-rebuild reread)* |
| AC7 | T8 | | AC18 | T15 |
| AC8 | T8 | | AC19 | T15 |
| AC9 | T8, T10 | | AC20 | T14 |
| AC10 | T9 | | AC21 | T7, T9 |
| AC11 | T10 | | | |
