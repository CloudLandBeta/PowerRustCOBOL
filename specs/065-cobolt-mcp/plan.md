# Plan — `cobolt-mcp`: the Model Context Protocol server

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-09-22

## 1. Approach

Three layers, split so the protocol knows nothing about COBOL and the runtime
knows nothing about transports.

```text
  external MCP client                the app's own COBOL agents
          │                                      │
          │ JSON-RPC over a transport            │ CALL "COBOL-MCP-SEARCH"
          │ the HOST owns (071, Q2)              │
          ▼                                      ▼
  ┌───────────────────────┐            ┌──────────────────────┐
  │ cobolt-mcp            │            │                      │
  │  framing · types ·    │──handler──▶│  the indexed-file    │
  │  serve loop · trait   │            │  tool  (runtime)     │
  │  serde + serde_json   │            │                      │
  └───────────────────────┘            └──────────┬───────────┘
        R1–R7                                     │
                                       .cidx ─────┤───── IndexedStore
                                    (descriptions)│      (records)
                                       R8–R11,R27–R31      R24–R26
```

**Layer 1 — `cobolt-mcp` (new crate).** Wire-compatible MCP: the JSON-RPC
envelope, the initialize handshake with protocol-version negotiation, tool
discovery and tool invocation, errors, and a `serve` loop. Transport is a
`Read`/`Write` pair the host supplies (R4). Mirrors `cobolt-dap` exactly —
`transport.rs` for framing, an `McpHandler` trait plus `serve<R, W, H>` for the
loop — because that shape is what lets the IDE, a built application and a test
link the protocol without linking each other. Dependencies: `serde`,
`serde_json`, nothing else (R2, R3). Joins `SDK_CRATES`, taking it to 11 (R5).

**Layer 2 — the tool, in `cobolt-runtime`.** This is where COBOL knowledge
lives, and `cobolt-runtime` already depends on `cobolt-indexed` and owns
`IndexedStore`. A new `mcp_tool.rs` builds the tool schema from the `.cidx`
descriptions and executes searches through the same `IndexedStore` surface the
COBOL verbs use (R25). It implements `cobolt_mcp::McpHandler`.

**Layer 3 — two front doors onto one definition (R20–R23).** The over-the-wire
path is `cobolt-mcp`'s `serve` driven by the host's transport. The in-process
path is a new built-in `CALL` arm in `interpreter.rs::exec_call`, alongside
`COBOL-HTTP-*` and `COBOL-EXEC-SQL`. Both call the *same* function on the same
struct; neither owns behaviour (R22), and a test drives both against one
fixture and compares (R23, AC14).

### One clarification the spec's wording invites

R15 says the tool "shall select the file and columns to search from the
question". Mechanically the **model** does the selecting — that is what MCP tool
discovery is *for*. The tool's job is to be described well enough that the model
can choose correctly, and to accept parameters rather than prose. So the
implementation obligation behind R15 is: emit one tool **per consultable file**,
each carrying that file's `comment` as its description and each column's
`comment` on the matching parameter (R13), so the choice is makeable. We do not
write a natural-language parser, and nothing here calls a model.

### Reuse, not reinvention

| Need | Already exists | Where |
|---|---|---|
| Anchor a project-relative path in a delivered app | `assets::resolve` | `cobolt-forms/src/assets.rs:55` |
| Parse a `.cidx` at run time | `load_indexed` | `cobolt-indexed` |
| Read records by key / sequentially | `IndexedStore` | `cobolt-runtime/src/indexed.rs:35` |
| Protocol-crate shape | `transport.rs` + handler trait + `serve` | `cobolt-dap` |
| Add a built-in verb | `exec_call` match arm | `interpreter.rs:10457` |

The runtime **already reads `.cidx` from disk at run time** — `interpreter.rs`
around 11876 does `assets::resolve` then `load_indexed` for DataGrid binding,
with a comment explaining why the CWD is the wrong anchor. R8/R27 need no new
mechanism; they need that one, called from a second place.

## 2. Affected crates / files

- `crates/cobolt-mcp/` — **new crate**: `lib.rs`, `types.rs`, `transport.rs`,
  `server.rs`. Workspace member; `serde` + `serde_json` only.
- `crates/cobolt-compiler/src/lib.rs` — add `"cobolt-mcp"` to `SDK_CRATES`
  (R5). **The `.cidx` staging is already done** — it shipped as a fix in
  1.70.146 (`87cb17f`), so R28/R29 need nothing here beyond a test that the
  behaviour still holds.
- `crates/cobolt-runtime/src/mcp_tool.rs` — **new**: schema generation from
  `.cidx` descriptions, search execution, `McpHandler` impl.
- `crates/cobolt-runtime/src/interpreter.rs` — one `exec_call` arm for the
  in-process path (R20).
- `crates/cobolt-runtime/Cargo.toml` — depend on `cobolt-mcp`.
- `crates/cobolt-runtime/src/lib.rs` — declare the module.
- `Cargo.toml` (workspace) — register the member.
- `docs/developers-guide-en.md` — MCP is developer-observable, **and this
  feature pays F1's parked sentence too** (see §7).

Not touched: `cobolt-ide`, `cobolt-indexed`, the `.cidx` schema, the editor,
`i18n.rs`, the compiler doc tables. R32 and Q5 are what keep them out.

## 3. Data / model changes

**No `.cidx` schema change.** Q5 reuses `comment`; Q3 puts consultable marking
in the application's own store.

**New: the consultable list.** `065` defines the contract it reads — a set of
`.cidx` paths the application has marked — and `071` owns the UI that writes it
and the store it lives in (063 R42). `065` must therefore treat an absent or
empty list as "nothing is consultable" rather than "everything is", so that a
half-built application exposes no data by accident.

**New: delivered `.cidx` files.** Only those the application requires (R28), at
their project-relative paths (R29). No format change; the file is copied
verbatim.

**Read-only slice (R30).** `load_indexed` returns a whole `IndexedDefinition`.
The tool takes `name`, `comment`, and each field's `name` + `comment`. It must
**not** take `offset`, `length`, `keys`, `record_format` or `storage` from the
delivered file — those come from the compiled program. This is the one place
where the design depends on discipline, so §6 gives it a test of its own.

## 4. Key decisions & alternatives

- **Decision: the tool lives in `cobolt-runtime`, not in `cobolt-mcp`.**
  Why: R2 forbids `cobolt-mcp` depending on any `cobolt-*` crate or the
  filesystem, and the tool needs `cobolt-indexed`, `IndexedStore` and disk.
  Rejected: putting indexed knowledge in the protocol crate — it would break the
  property that makes `cobolt-dap` reusable, for no gain.

- **Decision: one tool per consultable file**, rather than one `search` tool
  taking a file name. Why: the descriptions are the selection mechanism (R13,
  R15); a single tool would bury each file's purpose in a parameter enum, where
  a model chooses worse. Rejected: one generic tool — simpler to implement,
  materially worse at the thing the feature exists to do.

- **Decision: the in-process path is a built-in `CALL`.** Why: the agents are
  COBOL (063 R67), and `exec_call` is the established surface for exactly this.
  Rejected: an `EXEC RUST` block (pushes host-language detail into the
  boilerplate, against 063's readability goal).

- **Decision: `serve` is generic over `Read`/`Write`.** Why: it keeps Q2 open —
  stdio, a socket or a localhost HTTP body all satisfy it — and keeps TLS and
  HTTP out of `SDK_CRATES`. Rejected: building a transport into the crate now;
  it would foreclose `071`'s choice and drag in what R3 forbids.

- **Decision: absent consultable list means nothing is exposed.** Why: the
  failure mode of the opposite default is silently publishing every indexed file
  the developer has. Rejected: default-open convenience.

## 5. Risks & mitigations

- **✅ Closed — `.cidx` reached no delivery.** The compiler staged only `assets`
  and `data`, so a form's `indexed/actors.cidx` binding resolved to nothing in a
  hand-over bundle and a bound DataGrid came up empty — only there, which is why
  it went unseen. Root cause: the compiler's manifest deserializer had no
  `indexed` field at all, though `cobolt.toml` has declared one since indexed
  files were introduced.
  → **Shipped as a fix, 1.70.146 (`87cb17f`), ahead of this feature** as GOLDEN
  RULE #5 requires. Declared definitions now stage with their project-relative
  paths. `065` inherits working delivery, and keeps one test asserting it holds.
  *Caveat for sequencing:* that fix is on `fixes` and not yet merged to `main`,
  so this branch does not carry it. It is not needed to compile or unit-test
  `065`; it is needed to exercise the delivery end to end.

- **Risk: R30's discipline erodes.** Someone later reads an offset from the
  delivered `.cidx` because it is right there in the struct.
  → Mitigation: the tool takes a narrow descriptive struct, not the whole
  `IndexedDefinition`, and AC20's test tampers with a delivered file's offsets
  and asserts reads are unchanged.

- **Risk: unbounded scans.** `IndexedStore` will happily walk a million records.
  → Mitigation: R26 — a hard cap with truncation reported in the result, tested
  against a large fixture (AC16).

- **Risk: "wire-compatible" is asserted, not proven.** Our own client passing is
  not evidence a real MCP client connects.
  → Mitigation: the handshake, version negotiation and error envelopes get
  tests written against the published protocol shape, and Q4 is recorded as
  reopenable if the cost runs away.

- **Risk: the Guide bill.** GOLDEN RULE #8 means the doc update deletes five
  translations and reddens two `docs_embed.rs` guards until the next minor.
  → Mitigation: pay it once, carrying F1's parked sentence with it (§7). That
  is why it was parked.

## 6. Test strategy

**`cobolt-mcp` (unit, in-crate)**
- Framing round-trips; a truncated frame is an error, a clean EOF is not.
- An unknown method returns a JSON-RPC error and the loop continues (AC4).
- A body the server does not model survives a round trip unchanged (AC5).
- Handshake: version negotiation and `serverInfo` match the protocol shape (AC3).

**`cobolt-runtime` (integration)**
- Schema generation from a fixture `.cidx`: the file's `comment` becomes the
  tool description, each field's `comment` the matching parameter's (AC8).
- **Parity**: the same question, in-process and through `serve`, returns equal
  results (AC14). This is the guard that stops R22 rotting, and it is modelled
  on `engine_reference_form_parity_static_vs_faces`.
- **R30**: a delivered `.cidx` edited to claim a different offset, key and
  record length changes only the reported description; records read identically
  (AC20).
- An unmarked file is in neither discovery nor results, even when its
  description is the best match (AC10).
- A missing and a malformed `.cidx` each drop one tool and leave the rest
  serving (AC21).
- A search over a large fixture is bounded and reports truncation (AC16).
- No tool call writes: fixture bytes identical before and after (AC12).

**`cobolt-compiler`**
- Only required `.cidx` are staged; a definition the application never opens is
  absent; no directory is copied wholesale (AC18) — *moves to F3 if §5 is
  accepted*.
- The SDK staging tests accept the 11-crate set and stay dependency-closed
  (AC2).

**Reporting.** Any COBOL-level test added reports quantified results per GOLDEN
RULE #7 — cases exercised and timings, not a bare pass count.

**Manual / visual.** None for `065`; there is no UI. The operator may want to
point a real MCP client at a built application once `071` supplies a transport —
that is `071`'s check, not this one. Per the standing rule, verification here is
builds and tests only; the application is not driven.

## 7. Steering compliance

- [x] **i18n** — no new IDE strings. R32 keeps marking out of the IDE; the
      application's own form is localised in COBOL inside `071`.
- [x] **Generated-code banner + regenerate-on-action** — untouched. This feature
      generates no COBOL and changes no generator.
- [ ] **English dev guide updated (translations deleted, not edited)** — owed,
      and carries F1's parked sentence. Five translations go; two
      `docs_embed.rs` guards go red until the next minor. Intended.
- [x] **System KB** — gate does not fire: no control, property, method or event
      changes (Q3/R32). Re-check if `/tasks` reintroduces an IDE surface.
- [ ] **Fix vs feature** — `065` is a **feature**: `features` branch, `z` bump,
      f=96 if ever announced. The `.cidx` staging is a **fix** and is proposed to
      split out as **F3** (§5).
- [x] **No "cobolt" in user-facing text; COBOL identifiers English** — the crate
      name is build-only; the new `CALL` verb is English and COBOL-shaped.

## 8. Gate

Review this plan, then run **`/tasks`**.

The one decision this section used to hold — whether to split the `.cidx`
staging out as a fix — was taken and the fix shipped (1.70.146, `87cb17f`).
Nothing blocks `/tasks`.
