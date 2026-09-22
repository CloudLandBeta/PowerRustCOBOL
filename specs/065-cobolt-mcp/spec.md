# Spec — `cobolt-mcp`: the Model Context Protocol server

- **Status:** draft → awaiting operator review
- **Folder:** specs/065-cobolt-mcp/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-21

> Feature `065` of the umbrella spec **063 — RAG / Transactional Chatbot
> boilerplate**. It delivers the *transactional* half: living data reached
> through a real MCP tool, chosen from metadata the developer wrote.
> Depends on fix **F1** (shipped 1.70.145).

## 1. Overview

A PowerRustCOBOL application can talk to a model, and it can open an indexed
file. It cannot let the model *decide* to open one. This feature closes that
gap: the application exposes a **Model Context Protocol** server speaking
JSON-RPC 2.0, whose tool searches the developer's own indexed files and whose
schema is generated from the purpose and column descriptions written on each
`.cidx`.

The point is that **the user names no file.** They ask "how many contractors
started in Q3" and the tool decides that `STAFF-FILE`, described as *"one row
per employment record"* with a column described as *"start date"*, is the one
to consult. The metadata is the whole mechanism; without it the tool is a
keyword search with extra steps.

**One tool, two consumers** (operator, 2026-09-21): the application's own agents
call it **in process** — no port, no serialization, no round trip per question —
and the *same* tool is served over a real MCP transport so outside clients can
reach it. Neither path is a special case of the other; both dispatch the same
definition.

### The precedent this follows

`cobolt-dap` is a wire-compatible Debug Adapter Protocol implementation that
*"knows nothing about COBOL beyond the shape of the extension fields — no
interpreter, no egui, no filesystem — which is what lets the debugger UI, the
debuggee and a test all link the same protocol and none of them link each
other."* Its entire dependency list is `serde` and `serde_json`.

`cobolt-mcp` is built the same way, and the payoff is the same: the crate can be
linked by a compiled application, by the IDE, and by a test, without any of them
linking each other — and **without a network stack**. That matters more here
than it did for DAP, because this workspace has spent real effort staying clear
of rustls, aws-lc-rs and ring, and an MCP crate that reached for a TLS-enabled
HTTP server would drag all three back in.

### What F1 did and did not do

Fix **F1** (1.70.145) made the developer's descriptions appear in the generated
`.cbl`. That is documentation for a human reading the source — **COBOL comments
are stripped before parsing by design**, so nothing in the running program can
read them. Making the same metadata available *as data* is this feature's work,
and it is the prerequisite for the tool schema.

## 2. Goals / Non-goals

**Goals**

- A wire-compatible MCP server: JSON-RPC 2.0, correct envelopes and errors.
- A tool whose schema is derived from `.cidx` metadata, not hand-written.
- Automatic file and column selection from a natural-language question.
- One definition, dispatched identically in process and over the wire.
- No C toolchain, no TLS stack, no network dependency in the crate itself.

**Non-goals**

- MCP *client* behaviour — this application serves; it does not consume other
  servers. (063 §7 listed that as a separate option; it was not chosen.)
- Resources, prompts, sampling, or any MCP capability beyond tools.
- Authentication or authorisation. The transport's reach is the security
  boundary, and `071` decides what that is.
- Writing through the tool. Search only — no `WRITE`, `REWRITE` or `DELETE`.
- SQL, HTTP or Knowledge Base tools. Indexed files only; the KB is `068`.

## 3. User stories

- As an **end user**, I want to ask about live business data in my own words and
  get an answer, so that I do not have to know which file holds it.
- As a **developer**, I want the files I describe to become tools automatically,
  so that documenting a `.cidx` is the whole integration.
- As a **developer**, I want to choose which files the model may consult, so
  that describing a file does not publish it.
- As an **operator of another tool**, I want to query the application's data
  over MCP, so that it is reachable from clients the developer did not write.

## 4. Requirements (EARS)

### 4.1 The protocol crate

- **R1 (ubiquitous):** A new workspace crate `cobolt-mcp` shall implement the
  Model Context Protocol over JSON-RPC 2.0 — envelopes, request and response
  bodies, errors, and a server dispatch loop.
- **R2 (constraint):** `cobolt-mcp` shall not depend on COBOL, egui, the
  filesystem, or any `cobolt-*` crate.
- **R3 (constraint):** `cobolt-mcp` shall not depend on a TLS implementation, an
  HTTP server, or any crate requiring a C toolchain.
- **R4 (ubiquitous):** Transport shall be supplied by the host, so the crate
  serves a byte stream it does not own.
- **R5 (ubiquitous):** `cobolt-mcp` shall be a member of `SDK_CRATES`, so a
  compiled application links it.
- **R6 (event):** When a request names a method the server does not implement,
  it shall return a JSON-RPC error rather than close the connection.
- **R7 (ubiquitous):** A request body the server does not model shall survive a
  round trip rather than be dropped — the `cobolt-dap` rule, for the same
  reason.

### 4.2 Metadata reaching the running program

- **R8 (ubiquitous):** A running application shall be able to read each indexed
  file's declared purpose and each column's description.
- **R9 (constraint):** That metadata shall not be carried as COBOL comments,
  which are stripped before parsing.
- **R10 (ubiquitous):** The metadata available at run time shall be the same
  text the developer entered in the Indexed File Editor, with no second place to
  edit it.
- **R11 (event):** When a delivered `.cidx` changes, the metadata the running
  application reads shall change with it **on the next run**, without a rebuild.
- **R27 (ubiquitous):** The metadata shall be read at run time from the `.cidx`
  files delivered beside the application — not embedded in the binary, and not
  duplicated into a second format.
- **R28 (constraint):** The build shall copy **only the `.cidx` files the
  application requires**, never the whole `indexed/` directory. Nothing a
  delivery does not need is shipped.
- **R29 (ubiquitous):** A delivered `.cidx` shall keep its project-relative
  path, so the same stored reference resolves in the delivery exactly as it does
  in the project — the rule `assets/` and `data/` already follow.
- **R30 (constraint):** The runtime shall take **only descriptive metadata**
  from a delivered `.cidx`. Record layout, offsets, keys and storage mode remain
  authoritative from the compiled program.
- **R31 (event):** When a required `.cidx` is missing or unreadable, the tool
  shall omit that file and continue serving the rest.
- **R32 (ubiquitous):** Which files are consultable (R16) shall be held in the
  **application's own store**, set by the end user at run time — not as a
  project setting and not as an IDE control property.

> **R30 is what makes reading from disk safe.** A file in the delivery folder is
> editable by whoever runs the application, and a `.cidx` describes both *what a
> file means* and *how its records are laid out*. Taking only the first from
> disk means a tampered or stale `.cidx` can give a file a wrong description —
> visible, recoverable — but can never move an offset, change a key or
> misinterpret a record. The layout is already compiled in; there is no reason
> to read it twice and every reason not to.
>
> It also bounds R28: the delivery needs the descriptive part, so "only the
> required files" is a real constraint rather than a euphemism for the folder.

### 4.3 The tool

- **R12 (ubiquitous):** The server shall expose a tool for searching indexed
  files, listed through MCP's tool discovery.
- **R13 (ubiquitous):** The tool's schema shall be generated from the indexed
  files' metadata — the file's purpose as its description, each column's
  description as the corresponding parameter's.
- **R14 (constraint):** The tool schema shall not be hand-authored or duplicated
  anywhere; a description exists once, in the `.cidx`.
- **R15 (event):** When a question arrives, the tool shall select the file and
  columns to search from the question and the metadata, without the caller
  naming a file, table or column.
- **R16 (state):** While a file is not marked consultable in the application's
  configuration, the server shall neither list it nor search it.
- **R17 (ubiquitous):** A tool result shall identify which file answered and
  which records matched, so an answer can be traced to its source.
- **R18 (constraint):** The tool shall not modify data. Search only.
- **R19 (event):** When a search finds nothing, the tool shall say so rather
  than return an empty success indistinguishable from an error.

### 4.4 Two consumers, one definition

- **R20 (ubiquitous):** The application's own agents shall call the tool in
  process, without a transport, a port, or JSON serialization of the result.
- **R21 (ubiquitous):** The same tool shall be reachable by an external MCP
  client over the served transport.
- **R22 (constraint):** The two paths shall dispatch **one** definition. Neither
  shall carry behaviour the other lacks.
- **R23 (ubiquitous):** A test shall exercise both paths against the same
  fixture and assert the results agree.

### 4.5 Data access

- **R24 (ubiquitous):** Files searched by the tool shall be openable with
  `STORAGE MODE IS MEMORY`.
- **R25 (constraint):** The tool shall not bypass the indexed engine; it reads
  through the same `IndexedStore` surface COBOL verbs use.
- **R26 (event):** When a search would scan an unbounded number of records, the
  tool shall bound the work and report that the result was truncated.

## 5. Acceptance criteria

- [ ] **AC1** — `cargo tree -p cobolt-mcp` shows `serde` and `serde_json` and no
      transport, TLS or `cobolt-*` dependency. *(R2, R3)*
- [ ] **AC2** — A compiled application links `cobolt-mcp`, and the SDK staging
      test accepts the enlarged crate set. *(R5)*
- [ ] **AC3** — An MCP client completes the handshake, lists tools, and calls
      the search tool over the transport, receiving a well-formed result.
      *(R1, R12, R21)*
- [ ] **AC4** — An unknown method returns a JSON-RPC error and the connection
      stays open. *(R6)*
- [ ] **AC5** — An unmodelled body round-trips unchanged. *(R7)*
- [ ] **AC6** — A running application reads a file's purpose and a column's
      description, and the values equal what the `.cidx` holds. *(R8, R10)*
- [ ] **AC7** — Editing a description in the Indexed File Editor and rebuilding
      changes what the tool's schema reports, with no other file edited.
      *(R14)*
- [ ] **AC17** — Editing a description in a **delivered** `.cidx` changes the
      tool's schema on the next run, with no rebuild. *(R11, R27)*
- [ ] **AC18** — A delivery contains only the `.cidx` files the application
      requires; a definition the application never opens is absent, and no
      `indexed/` directory is copied wholesale. *(R28)*
- [ ] **AC19** — A delivered `.cidx` keeps the same project-relative path it had
      in the project. *(R29)*
- [ ] **AC20** — A delivered `.cidx` edited to declare a different offset, key
      or record length does **not** change how records are read; only the
      description it reports changes. *(R30)*
- [ ] **AC21** — A missing or malformed `.cidx` removes that file from tool
      discovery, and every other file stays searchable. *(R31)*
- [ ] **AC8** — The generated tool schema carries the file's purpose as its
      description and each column's description on the matching parameter.
      *(R13)*
- [ ] **AC9** — A question naming no file returns records from the correct file.
      *(R15)*
- [ ] **AC10** — An unmarked file appears in neither tool discovery nor results,
      even when its description matches the question best. *(R16)*
- [ ] **AC11** — A result names the file and the matched records. *(R17)*
- [ ] **AC12** — No tool call writes: a search against a fixture leaves its
      records byte-identical. *(R18)*
- [ ] **AC13** — A search with no matches is distinguishable from a failure.
      *(R19)*
- [ ] **AC14** — The in-process and over-the-wire paths return equal results for
      the same question against the same fixture. *(R20, R21, R22, R23)*
- [ ] **AC15** — A file opened `STORAGE MODE IS MEMORY` is searchable, and the
      tool reads through `IndexedStore`. *(R24, R25)*
- [ ] **AC16** — A search over a large fixture is bounded and reports
      truncation. *(R26)*

## 6. Constraints & steering check

**i18n.** Any IDE-facing string this feature adds — marking a file consultable
(R16) most likely — is a `Tr` field in all six languages. The tool's *schema*
text is the developer's own from the `.cidx` and is not translated; it is their
content, like a data-item name.

**COBOL stays English.** Data-item names, paragraph names and generated COBOL
remain English regardless of UI language, including anything this feature
generates.

**Generated-code contract.** R11 ties metadata freshness to the existing
regenerate-on-Build/Run/Debug/Check rule. The developer banner stays; generated
output stays read-only and is never hand-edited.

**System KB — the gate does not fire.** It would have, had marking a file
consultable become a control property. It did not: Q3 puts that in the
application's own store (R32), so this feature adds no control, property, method
or event, touches no `cobolt-compiler` doc table, and needs no
`chunked.data` regeneration. If that changes during `/plan`, the gate returns
with it.

**i18n — nothing to add here either.** R32 keeps the marking out of the IDE, so
no new `Tr` field is required by `065`. The application's own Arquivos form is
localised in COBOL, inside `071` (063 R48/R49).

**Documentation.** MCP is developer-observable and belongs in
`docs/developers-guide-en.md`. Under GOLDEN RULE #8 that edit deletes the five
translations and reddens two `docs_embed.rs` guards until the next minor. **F1
already parked a Guide sentence for this same area** (1.70.145), so this
feature should pay that cost once and carry both.

**Fix vs feature.** A **feature** — MCP is a capability beyond the COBOL-85
standard and beyond the IDE's existing scope. `features` branch, `z` bump,
f=96 if ever announced. Note R8–R11 are *not* a continuation of fix F1: F1
completed an incomplete implementation, while runtime-readable metadata is new
capability.

**Tests.** Any new COBOL test reports quantified, human-readable results
(GOLDEN RULE #7). Verify-first — no measurement stated that a run did not
produce. `cobolt-forms` tests need `--features render`; sweeps use
`--no-fail-fast` and every `test result:` line is read.

**Never drive the application** to verify. Builds and tests only.

## 7. Open questions

- **Q1 — ✅ Resolved (operator, 2026-09-21): read `.cidx` from disk.** The
  application reads its metadata at run time from `.cidx` files delivered beside
  it (R27). The two alternatives — embedding the definitions in the binary, or
  moving descriptions into the `PRCIDX1` container header — were rejected.

  I had recommended embedding, on the grounds that `cobolt-indexed` is already
  in `SDK_CRATES` (so the parser is already linked, and it costs no dependency)
  and that shipping `indexed/` widens the delivery. The operator's ruling:
  **the extra directory is a minor nuisance, and the copy must be selective** —
  only the files the KB actually needs (R28).

  Reading from disk buys something embedding cannot: **descriptions can be
  corrected in a delivered application without a rebuild** (R11). For metadata
  whose entire job is to help a model choose a file, being able to fix a bad
  description in the field is worth more than self-containment.

  Two things make it safe rather than merely convenient. **R30** takes only the
  descriptive part from disk and leaves layout, offsets and keys compiled in, so
  a tampered `.cidx` can misdescribe a file but never misread one. **R31** makes
  a missing or broken file lose its tool entry instead of the application.

  The container-header option stays rejected on its own merits: changing a
  column *description* would mean rewriting the data file, which carries a CRC
  and strict OPEN validation — a documentation edit putting real records at
  risk.

- **Q2 — What transport does the host supply (R4, R21)?** The crate is
  transport-agnostic by R4, so this is about what `071` actually serves.
  Localhost HTTP reaches any client but needs a server in the binary; stdio is
  MCP's most common shape and needs no network code but is awkward for a GUI
  process; a named pipe or Unix socket avoids both but narrows which clients can
  connect. **Deferred to `071`**, which owns the application; `065` must only
  avoid foreclosing any of them.

- **Q3 — ✅ Resolved (operator, 2026-09-21): an application-side store.** The
  end user marks files in the running application's Arquivos form (063 R42), and
  the marking persists in the application's own data. **No IDE property, no
  compiler doc-table change, and therefore no System KB regeneration and no new
  `Tr` strings in this feature** — see §6, where the gate is recorded as not
  firing. It also matches R42's intent that the choice belongs to whoever runs
  the application, not to whoever built it.

  *(Superseded question text:)* Where is "marked consultable" stored (R16)? 063's R42 puts the marking
  in the application's Arquivos form. Whether that is a project setting, an
  application-side store, or a property on an `IndexedFile` control changes
  whether the IDE needs a new property — and therefore whether the System KB
  gate fires for this feature.

- **Q4 — ◐ Taken as recommended (2026-09-21), absent objection: full wire
  compatibility.** Stated to the operator as the assumption `/plan` would
  proceed on; not explicitly ruled. Reopen it if the protocol work proves
  disproportionate.

  *(Reasoning:)* How literal is "wire-compatible"? `cobolt-dap` chose full wire
  compatibility so real DAP clients work. The equivalent here is that Claude
  Desktop or any MCP client connects without special-casing. That implies
  honouring protocol version negotiation and the initialize handshake exactly,
  which is more work than a JSON-RPC endpoint that merely answers `tools/call`.
  *Recommend full compatibility* — the weaker version fails the moment a real
  client is pointed at it, which is the only way anyone will test it.

- **Q5 — ✅ Resolved (operator, 2026-09-21): reuse `comment`; no new field.**

  I had argued for a separate machine-facing field, on the evidence that the one
  real `.cidx` in the tree describes `ACTORS-FILE` as *"Actors sample data,
  loaded by `misc/load-actors-idx.cbl`…"* — a developer's note, useless to a
  model choosing a file. The operator's ruling: **that comment is a mistake, not
  a pattern.** `comment` is the description field and always was; the sample was
  written badly.

  So R13 stands as written and nothing changes in `cobolt-indexed`, the `.cidx`
  schema, the editor, its i18n, or F1's generated-comment behaviour. The whole
  feature still turns on description quality (R15) — but that is now a matter of
  writing good descriptions, not of adding somewhere to put them.

  **Worth acting on separately:** `examples/PowerDemo3/indexed/actors.cidx`
  carries the badly-written comment, and it is the example a developer copies.
  Rewriting it to describe what the file *holds* rather than how it was loaded
  would make the sample teach the right habit. Out of scope here.

## 8. Provenance

Requirements trace to umbrella spec `063` §4.4 (R19–R24) and the operator's
statements of 2026-09-21. The "both consumers" decision is the operator's, taken
2026-09-21. Every claim about existing code — `cobolt-dap`'s dependency list and
isolation, `SDK_CRATES` membership, comment stripping before parsing, the
`IndexedStore` surface — was verified against the tree at 1.70.145 on the same
date.
