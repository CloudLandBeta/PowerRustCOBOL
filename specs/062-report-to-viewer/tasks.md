# Tasks — Printing a report into a Viewer

- **Status:** draft → **ready for `/implement`**
- **Plan:** ./plan.md   **Date:** 2026-09-20

Ordered so the tree builds and the suites stay green after every task. Each
names its files, the requirements it satisfies and the exact command that proves
it. **Nothing is committed or pushed until the operator asks** — see *Done
criteria*.

Everything here lands on **`features`** (synced to `main` at `26a79e3`), in
commits separate from any fix, one `z` bump per commit.

---

- [ ] **T1 — The AST learns two organizations and a target** (R1, R2)
  - Files: `crates/cobolt-ast/src/program.rs`
  - Do: add `FileOrganization::{Markdown, Html}` and
    `FileControl.viewer_target: Option<String>` with `#[serde(default)]` (the
    compiler embeds a `bincode` AST; an older one must still deserialise).
    Then make the workspace compile again: adding variants breaks every
    exhaustive `match` on `FileOrganization`, and each site is a decision —
    a disk-organization site treats the two as unreachable, a text site treats
    them as line-oriented. **List the sites the compiler names in the task's
    notes**; they are the map of everything that reads an organization.
  - Verify: `cargo build --workspace` green; `cargo test -p cobolt-ast`.

- [ ] **T2 — The parser reads the declaration** (R1, R2 → AC1, AC2)
  - Files: `crates/cobolt-parser/src/parser.rs`,
    `crates/cobolt-parser/tests/report_to_viewer.rs` (new)
  - Do: `parse_organization` gains `MARKDOWN` and `HTML`, matched **by word**
    (they arrive as identifiers — the same shape as `PAGE` in
    `advancing_lines` and `REGISTERED` in `parse_open`, so no lexer change).
    `parse_file_control_entry` recognises `VIEWER` after `ASSIGN [TO]` and takes
    the following literal into `viewer_target`.
  - Verify: `cargo test -p cobolt-parser --test report_to_viewer -- --nocapture`
    — `a_report_declares_its_viewer_and_its_organization` prints the parsed
    `(assign, viewer_target, organization)` for each of: the three
    organizations, `ORGANIZATION IS` omitted, and lower-case spellings.

- [ ] **T3 — The two diagnostics** (R4, R20 → AC12)
  - Files: `crates/cobolt-semantic/src/*`, its tests
  - Do: report `MARKDOWN`/`HTML` declared without `ASSIGN TO VIEWER`, and page
    control (`ADVANCING PAGE`, `LINAGE`, `AT END-OF-PAGE`) on a `MARKDOWN` or
    `HTML` file. Each message names the file. Plan D5: semantic if the analyser
    can join the `FD` to its `SELECT`; a runtime status is the sanctioned
    fallback, and taking it means saying so here.
  - Verify: `cargo test -p cobolt-semantic report_to_viewer -- --nocapture`;
    each test prints the diagnostics it saw so a wrong message is visible, not
    just a wrong count.

- [ ] **T4 — The report's filename** (R10 → AC3)
  - Files: `crates/cobolt-runtime/Cargo.toml` (add
    `uuid = { version = "1", features = ["v4"] }` — already in the lock at
    1.23.2), `crates/cobolt-runtime/src/interpreter.rs`
  - Do: one pure function from `(form, organization)` to
    `<form>-<uuid>.<ext>`, `ext` ∈ `txt|md|html` (D4, operator 2026-09-20).
  - Verify: `cargo test -p cobolt-runtime report_filename -- --nocapture` —
    asserts the extension per organization and that two calls differ; prints
    three example names.

- [ ] **T5 — OPEN: a temp file, or memory** (R7, R8, R9, R11, R21 → AC4, AC13, AC14, AC15)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: `OpenFile::ViewerReport { sink, target, org, path }` (D1), built by
    `exec_open`: `OUTPUT` starts empty, `EXTEND` appends (D6), `INPUT`/`I-O` are
    refused with a testable status. A temp dir that cannot be written falls back
    to an in-memory sink (R11). A control that is missing, is not a Viewer, or a
    program not running on a form fails the OPEN with the id in the message.
  - Verify: `cargo test -p cobolt-runtime viewer_report_open -- --nocapture` —
    prints the status and the chosen sink (path or memory) per case.

- [ ] **T6 — WRITE: one record, one line** (R5, R6 → AC3)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: the `ViewerReport` arm of `exec_write` materialises the record exactly
    as today and writes one text line with trailing spaces removed — the
    `LineSequential` arm's own rule, and the reason R6 is worded that way.
  - Verify: `cargo test -p cobolt-runtime viewer_report_write -- --nocapture` —
    40 records, asserts the line count, that a record with interior spaces keeps
    them, and that the file's name matches T4's shape.

- [ ] **T7 — ADVANCING actually moves the page, for a report only** (R18 → AC10)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: emit the vertical movement for a `ViewerReport` whose organization is
    `SEQUENTIAL` — blank lines for `AFTER`/`BEFORE ADVANCING n LINES`, a form
    feed (`0x0C`) for `ADVANCING PAGE`. `advance_linage` keeps counting exactly
    as it does now; **do not change what it emits for any other file** (D2).
  - Verify: `cargo test -p cobolt-runtime advancing_moves_the_page_only_for_a_viewer_report -- --nocapture`
    — the same program written to a disk file and to a report; asserts the disk
    bytes are **identical to today's** and prints both byte counts, and that the
    report carries the blank lines and the form feed.

- [ ] **T8 — CLOSE hands the document over** (R12, R13, R14, R15, R16, R17, R22 → AC5–AC9)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: flush and close the sink **before** announcing it (plan §5's ordering
    risk). A file report: `obj_set(target, "View1Source", path)`. A memory
    report: the `LoadBytes` route — `viewer_bytes` + `Format` + `onLoaded` —
    which is already proven end to end. A second report replaces the first; a
    report with no records shows an empty document; the backing file is not
    deleted.
  - Verify: `cargo test -p cobolt-runtime viewer_report_close -- --nocapture` —
    asserts the `StateUpdate` for `View1Source` arrives on CLOSE and **not**
    before, that `Layout`/`Zoom`/`SplitMode`/`FontSize` are untouched, and that
    the first report's file still exists after the second.

- [ ] **T9 — STOP RUN closes an open report** (plan §5)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: a report still open when the run unit ends goes through the same close,
    so the document appears rather than being lost with the sink.
  - Verify: `cargo test -p cobolt-runtime a_report_left_open_is_closed_at_stop_run`.

- [ ] **T10 — The pages the program wrote are the pages the reader turns** (R19 → AC11)
  - Files: `crates/cobolt-forms/tests/a_report_breaks_its_pages_where_the_program_did.rs` (new)
  - Do: no production change expected — `viewer::index_text` already cuts on
    form feeds. This test is what makes that a promise instead of a coincidence:
    feed T7's emitted text through `index_text` and assert the page count and
    each boundary.
  - Verify: `cargo test -p cobolt-forms --features render --test a_report_breaks_its_pages_where_the_program_did -- --nocapture`
    — prints the page spans it found.

- [ ] **T11 — The same behaviour in all three hosts** (AC16)
  - Files: tests under `crates/cobolt-form-host/tests/` and the compiled-binary
    path in `crates/cobolt-compiler/`
  - Do: read the `interpreter-binary-parity` skill first, then prove a report
    reaches the Viewer under `rcrun run-form`, in an embedded child form **and**
    in a compiled binary. The compiled binary is the one that gets forgotten.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` and the compiler's
    demo-build test; report each host by name in the output.

- [ ] **T12 — COBOL programs that report their own results** (GOLDEN RULE #7)
  - Files: `tests/cobol/report-viewer/*.cbl`, driven from
    `crates/cobolt-runtime/tests/`
  - Do: one program per organization. Each ends with a **single result block**:
    the organization, the `ADVANCING` forms exercised by name, records written,
    pages produced, elapsed ms and records/sec, and the pass/fail tally. No
    per-record DISPLAY.
  - Verify: `cargo test -p cobolt-runtime report_viewer_cobol -- --nocapture`;
    the block is readable by eye and the numbers are the run's own.

- [ ] **T13 — NIST regression** (plan §5's second risk)
  - Files: none; `NIST/progress.json` only if a number moves
  - Do: the write path is what CCVS85 drives. Re-run the whole-suite compile
    census **and** the execution pass for every finished module, not a spot
    check (`nist-fix-requires-full-regression`).
  - Verify:
    `cargo run -p cobolt-semantic --example nist_conformance -- strict`,
    then `-- run NC` and `-- run SQ`. NC and SQ must still report **100 % on
    both axes**; quote the two numbers per module, never one as if it were both.
    Build the harness in its own command — `--example` does not rebuild `rcrun`.

- [ ] **T14 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, and `crates/cobolt-ide/src/i18n.rs`
    **only if** a string reaches the IDE
  - Do: a Guide section written for a developer who knows `ASSIGN TO PRINTER` —
    the declaration, the three organizations and what each renders, where the
    file lives, that it appears on `CLOSE` in view 1, and the two caveats worth
    stating outright: `LINAGE-COUNTER` is one name for the program (plan §5),
    and a memory report is held whole while a file report is indexed lazily.
    COBOL examples only — never Rust (GOLDEN RULE #3). English canonical; do not
    write translations.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green (no empty
    translations); `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` — the
    known `every_document_ships_in_every_language` red is expected and **must
    not** be `#[ignore]`d.

- [ ] **T15 — Finalize**
  - Do: bump `VERSION`, add a dated `CHANGELOG.md` entry, and confirm the System
    KB: no control property/method/event changed, so `assets/knowledge/chunked.data`
    should be byte-identical — if it is not, the wrong file was edited.
  - Verify, reading **every** `test result:` line, never a grep for failures:
    - `cargo test -p cobolt-forms --features render --no-fail-fast`
    - `cargo test -p cobolt-runtime --no-fail-fast`
    - `cargo test -p cobolt-parser -p cobolt-semantic -p cobolt-ast --no-fail-fast`
    - `cargo test -p cobolt-form-host --no-fail-fast`
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`
    - `cargo build --release -p cobolt-ide -p cobolt-cli` (no `--bin` filter, or
      `rcrun` is left stale)
  - Then hand the operator the manual check (plan §6): a form with a Viewer,
    run it, press the button, switch `Layout` to `Page` and turn the pages, and
    Save As the result. **The agent does not drive the app.**

## Done criteria

Every acceptance criterion in `spec.md` is covered by a task's verification:

| AC | Task | AC | Task |
|---|---|---|---|
| AC1 | T2 | AC9 | T8 |
| AC2 | T2 | AC10 | T7 |
| AC3 | T4, T6 | AC11 | T10 |
| AC4 | T5, T8 | AC12 | T3 |
| AC5 | T8 | AC13 | T5 |
| AC6 | T8 | AC14 | T5 |
| AC7 | T8 | AC15 | T5 |
| AC8 | T8 | AC16 | T11 |

Tests pass, the Guide carries the feature, NIST NC and SQ are still 100 % on
both axes, and the work is split into **feature** commits on `features`. Do not
commit or push unless the operator asks.
