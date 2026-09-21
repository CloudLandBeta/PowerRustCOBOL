# Tasks — Printing a report into a Viewer

- **Status:** **implemented** at 1.70.137 (2026-09-20) — uncommitted; the
  operator commits when they choose
- **Plan:** ./plan.md   **Date:** 2026-09-20

Ordered so the tree builds and the suites stay green after every task. Each
names its files, the requirements it satisfies and the exact command that proves
it. **Nothing is committed or pushed until the operator asks** — see *Done
criteria*.

Everything here lands on **`features`** (synced to `main` at `26a79e3`), in
commits separate from any fix, one `z` bump per commit.

---

- [x] **T1 — The AST learns two organizations and a target** (R1, R2)
  - **Finding:** the compiler named **no** match sites — every `match` on
    `FileOrganization` in the workspace already has a `_` arm, so `Markdown` and
    `Html` silently inherit record-`Sequential` behaviour rather than failing to
    compile. The map had to be made by hand: 14 sites in
    `cobolt-runtime/src/interpreter.rs` and 5 in the parser, all equality tests
    against `Indexed`/`Relative`/`LineSequential`. Nothing breaks, because T5's
    `OpenFile::ViewerReport` is matched before any of them — but a `MARKDOWN`
    file declared **without** `ASSIGN TO VIEWER` would quietly become a
    record-sequential disk file, which is precisely why T3's diagnostic is not
    optional.
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

- [x] **T2 — The parser reads the declaration** (R1, R2 → AC1, AC2)
  - Files: `crates/cobolt-parser/src/parser.rs`,
    `crates/cobolt-parser/tests/report_to_viewer.rs` (new)
  - Do: `parse_organization` gains `MARKDOWN` and `HTML`, matched **by word**
    (they arrive as identifiers — the same shape as `PAGE` in
    `advancing_lines` and `REGISTERED` in `parse_open`, so no lexer change).
    `parse_file_control_entry` recognises `VIEWER` after `ASSIGN [TO]` and takes
    the following literal into `viewer_target`.
  - **Done, with one thing the plan had not seen:** the four original
    organizations reach the clause loop as their own *tokens*, so the bare form
    (`ORGANIZATION IS` omitted) is matched by arms that do not exist for a word.
    A bare `MARKDOWN` fell to the loop's catch-all and was discarded, leaving
    the file SEQUENTIAL — the same silent failure the comment beside that arm
    already records for IX103A. A guarded `Token::Identifier` arm fixes it, and
    the test covers the bare form for exactly this reason.
  - Verify: `cargo test -p cobolt-parser --test report_to_viewer -- --nocapture`
    — `a_report_declares_its_viewer_and_its_organization` prints the parsed
    `(assign, viewer_target, organization)` for each of: the three
    organizations, `ORGANIZATION IS` omitted, and lower-case spellings.

- [x] **T3 — The two diagnostics** (R4, R20 → AC12)
  - Files: `crates/cobolt-semantic/src/*`, its tests
  - Do: report `MARKDOWN`/`HTML` declared without `ASSIGN TO VIEWER`, and page
    control (`ADVANCING PAGE`, `LINAGE`, `AT END-OF-PAGE`) on a `MARKDOWN` or
    `HTML` file. Each message names the file. Plan D5: semantic if the analyser
    can join the `FD` to its `SELECT`; a runtime status is the sanctioned
    fallback, and taking it means saying so here.
  - **Done, semantic** (D5's preferred half): `cobolt-semantic/src/reports.rs`,
    run as pass 1d. The FD carries `LINAGE`, the `SELECT` carries the
    organization, and joining them by the file's name turned out to be
    straightforward — no fallback needed. It reuses `exec_rust`'s existing
    statement walker rather than adding a second one. **Interpretation
    recorded:** `ADVANCING n LINES` is *not* page control and is accepted on
    every organization — a blank line separates one Markdown paragraph from the
    next, so a program is right to ask for one. Only `PAGE`, `LINAGE` and
    `END-OF-PAGE` are refused on a flowing document.
  - Verify: `cargo test -p cobolt-semantic report_to_viewer -- --nocapture`;
    each test prints the diagnostics it saw so a wrong message is visible, not
    just a wrong count.

- [x] **T4 — The report's filename** (R10 → AC3)
  - Files: `crates/cobolt-runtime/Cargo.toml` (add
    `uuid = { version = "1", features = ["v4"] }` — already in the lock at
    1.23.2), `crates/cobolt-runtime/src/interpreter.rs`
  - Do: one pure function from `(form, organization)` to
    `<form>-<uuid>.<ext>`, `ext` ∈ `txt|md|html` (D4, operator 2026-09-20).
  - Verify: `cargo test -p cobolt-runtime report_filename -- --nocapture` —
    asserts the extension per organization and that two calls differ; prints
    three example names.

- [x] **T5 — OPEN: a temp file, or memory** (R7, R8, R9, R11, R21 → AC4, AC13, AC14, AC15)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: `OpenFile::ViewerReport { sink, target, org, path }` (D1), built by
    `exec_open`: `OUTPUT` starts empty, `EXTEND` appends (D6), `INPUT`/`I-O` are
    refused with a testable status. A temp dir that cannot be written falls back
    to an in-memory sink (R11). A control that is missing, is not a Viewer, or a
    program not running on a form fails the OPEN with the id in the message.
  - Verify: `cargo test -p cobolt-runtime --test test_report_to_viewer` — 9
    tests, all green.
  - **⚠️ R21 is only partly implementable, and this is the divergence to
    settle.** The runtime can refuse `OPEN INPUT`/`I-O` (**37**), a
    `VIEWER` with no control literal (**31**) and a program with no form at all
    (**93**) — all three are tested. It **cannot** tell whether `VWR-1` exists
    or is a Viewer: `ObjectRegistry` is populated on first write ("a running
    form does not pre-register its controls"), and `set_form_host` hands the
    interpreter a channel, a window handle and the form's object name — no
    control inventory. A wrong control id therefore writes the report and
    displays nothing. Closing that needs a request/reply on the form-host
    channel (`FormRequest` already has the shape, via `HandleMethod`),
    implemented in all three hosts — a spec of its own, not a line in this
    task.

- [x] **T6 — WRITE: one record, one line** (R5, R6 → AC3)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: the `ViewerReport` arm of `exec_write` materialises the record exactly
    as today and writes one text line with trailing spaces removed — the
    `LineSequential` arm's own rule, and the reason R6 is worded that way.
  - Verify: `cargo test -p cobolt-runtime viewer_report_write -- --nocapture` —
    40 records, asserts the line count, that a record with interior spaces keeps
    them, and that the file's name matches T4's shape.

- [x] **T7 — ADVANCING actually moves the page, for a report only** (R18 → AC10)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: emit the vertical movement for a `ViewerReport` whose organization is
    `SEQUENTIAL` — blank lines for `AFTER`/`BEFORE ADVANCING n LINES`, a form
    feed (`0x0C`) for `ADVANCING PAGE`. `advance_linage` keeps counting exactly
    as it does now; **do not change what it emits for any other file** (D2).
  - Verify: `cargo test -p cobolt-runtime advancing_moves_the_page_only_for_a_viewer_report -- --nocapture`
    — the same program written to a disk file and to a report; asserts the disk
    bytes are **identical to today's** and prints both byte counts, and that the
    report carries the blank lines and the form feed.

- [x] **T8 — CLOSE hands the document over** (R12, R13, R14, R15, R16, R17, R22 → AC5–AC9)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: flush and close the sink **before** announcing it (plan §5's ordering
    risk). A file report: `obj_set(target, "View1Source", path)`. A memory
    report: the `LoadBytes` route — `viewer_bytes` + `Format` + `onLoaded` —
    which is already proven end to end. A second report replaces the first; a
    report with no records shows an empty document; the backing file is not
    deleted.
  - Verify: `cargo test -p cobolt-runtime --test test_report_to_viewer`.
  - **Landed early**, forced by the compiler: adding the `OpenFile` variant in
    T5 made the CLOSE match non-exhaustive, and a stub arm would have been a
    lie about what CLOSE does.
  - **⚠️ This task broke every indexed file in the product for an hour**, and
    only the full sweep caught it. The hand-over branch was written as
    `if let Some(OpenFile::ViewerReport { .. }) = self.open_files.remove(&file)`
    — but `remove` runs whatever the pattern then decides, so EVERY file was
    taken out of the map and dropped, and `IndexedStore::close()` (which commits
    the container) never ran for any of them. `cargo test -p cobolt-runtime`
    went from 945/0 to 918/40, with `test_fileio_storage` failing on alternate
    keys and REWRITE — a corruption two layers away from anything this feature
    touches. Now: one `remove`, matched by value, and a non-report handle put
    straight back. The lesson is the sweep, not the bug: the nine tests written
    for this task were all green while it was broken.
  - **R14 restated, because the first version of its test was wrong.** It
    asserted that a program which never closes announces nothing — but COBOL's
    `STOP RUN` closes every open file (T9), so that test was asserting a bug.
    What R14 actually promises is that a report is announced **once, by the
    close**, not per record: 40 writes → 1 hand-over, and the flush-drop-announce
    order inside the close is what keeps a session from indexing half a
    document.

- [x] **T9 — STOP RUN closes an open report** (plan §5)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: a report still open when the run unit ends goes through the same close,
    so the document appears rather than being lost with the sink.
  - Verify: `cargo test -p cobolt-runtime --test test_report_to_viewer a_report_left_open_is_closed_at_stop_run`.
  - Found by the T8 test above rather than by reading: `STOP RUN` raises a
    signal and nothing closes the files — a `BufWriter` flushes when it is
    dropped, so the report reached the disk and nobody was ever told where.

- [x] **T10 — The pages the program wrote are the pages the reader turns** (R19 → AC11)
  - Files: `crates/cobolt-forms/tests/a_report_breaks_its_pages_where_the_program_did.rs` (new)
  - Do: no production change expected — `viewer::index_text` already cuts on
    form feeds. This test is what makes that a promise instead of a coincidence:
    feed T7's emitted text through `index_text` and assert the page count and
    each boundary.
  - Verify: `cargo test -p cobolt-forms --features render --test a_report_breaks_its_pages_where_the_program_did -- --nocapture`
    — prints the page spans it found.

- [x] **T11 — The same behaviour in all three hosts** (AC16)
  - Files: tests under `crates/cobolt-form-host/tests/` and the compiled-binary
    path in `crates/cobolt-compiler/`
  - Do: read the `interpreter-binary-parity` skill first, then prove a report
    reaches the Viewer under `rcrun run-form`, in an embedded child form **and**
    in a compiled binary. The compiled binary is the one that gets forgotten.
  - Verify: `cobolt-runtime` 958/0, `cobolt-form-host` 147/0, `cobolt-cli` 8/0,
    `cobolt-compiler` 131/0 in the lib (its two `external_crates_*` e2e tests
    fail on a staging directory and never load a form — outside this work).
  - **No host needed changing, and here is the reason** (the skill asks for one
    rather than a shrug): the whole feature lives in the interpreter and rides
    `obj_set` → `StateUpdate` on the state channel, which all four production
    sites already wire —
    `cobolt-compiler/src/lib.rs:3214` (**the compiled binary**),
    `cobolt-form-host/src/host.rs:3691` (embedded child forms),
    `cobolt-cli/src/form_gui.rs:386` (`rcrun run-form`) and
    `cobolt-cli/src/main.rs:220`. Nothing was added that a host must opt into:
    no new setup call, no new capability to enable. What is **not** proven here
    is a built binary printing a report end to end; that needs a demo build and
    is the operator's visual check.

- [x] **T12 — COBOL programs that report their own results** (GOLDEN RULE #7)
  - Files: `tests/cobol/report-viewer/*.cbl`, driven from
    `crates/cobolt-runtime/tests/`
  - Do: one program per organization. Each ends with a **single result block**:
    the organization, the `ADVANCING` forms exercised by name, records written,
    pages produced, elapsed ms and records/sec, and the pass/fail tally. No
    per-record DISPLAY.
  - Verify: `cargo test -p cobolt-runtime --test test_report_viewer_cobol --
    --nocapture --test-threads=1` — three programs, three blocks. Markdown
    505 records / 1 page / 50,500 rec-s, HTML 505 / 1 / 50,500, SEQUENTIAL
    404 records / **2 pages** / 40,400 rec-s, each 3 PASS 0 FAIL. The driver
    then checks the document itself, including that the Viewer reads the
    sequential report as the two pages the program wrote.

- [x] **T13 — NIST regression** (plan §5's second risk)
  - Files: none; `NIST/progress.json` only if a number moves
  - Do: the write path is what CCVS85 drives. Re-run the whole-suite compile
    census **and** the execution pass for every finished module, not a spot
    check (`nist-fix-requires-full-regression`).
  - Verify:
    `cargo run -p cobolt-semantic --example nist_conformance -- strict`,
    then `-- run NC` and `-- run SQ`. NC and SQ must still report **100 % on
    both axes**; quote the two numbers per module, never one as if it were both.
    Build the harness in its own command — `--example` does not rebuild `rcrun`.
  - **Run at the operator's request, and wider than the task asked:** the
    ledger lists **eight** finished modules, not two, so all eight were run.
    Compile census: 18 programs fail, every one of them in CM, RW, DB or OB —
    out-of-scope modules, and not one NC or SQ program among them. Execution:

    | module | programs | assertions | failures |
    |---|---|---|---|
    | NC (Nucleus) | 95 / 95 | 4614 | 0 |
    | SQ (Sequential I-O) | 85 / 85 | 624 | 0 |
    | IX (Indexed I-O) | 41 / 41 | 574 | 0 |
    | IF (If/Evaluate) | 45 / 45 | 841 | 0 |
    | IC (Inter-program Communication) | 25 / 25 | 309 | 0 |
    | ST (Sort/Merge) | 39 / 39 | 735 | 0 |
    | SM (Source Text Manipulation) | 16 / 16 | 311 | 0 |
    | RL (Relative I-O) | 34 / 34 | 354 | 0 |

    380 programs, **8,362 assertions, zero failures** — 100 % on both axes for
    every finished module. The programs-in-scope counts differ from the
    ledger's *compile* counts in three modules (IX 41 vs 42, IC 25 vs 47,
    SM 16 vs 17) because a compile-only program is not executed; the numbers
    above are this run's, not the ledger's.

- [x] **T14 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, and `crates/cobolt-ide/src/i18n.rs`
    **only if** a string reaches the IDE
  - Do: a Guide section written for a developer who knows `ASSIGN TO PRINTER` —
    the declaration, the three organizations and what each renders, where the
    file lives, that it appears on `CLOSE` in view 1, and the two caveats worth
    stating outright: `LINAGE-COUNTER` is one name for the program (plan §5),
    and a memory report is held whole while a file report is indexed lazily.
    COBOL examples only — never Rust (GOLDEN RULE #3). English canonical; do not
    write translations.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` — 9 passed.
  - **No new `Tr` keys were needed**, as the plan expected: the developer
    writes COBOL and the report appears in a control that already exists. The
    Guide gained "Printing a report into a Viewer: `ASSIGN TO VIEWER`" beside
    `SELECT OPTIONAL`, with COBOL examples only and no Rust anywhere in it.

- [x] **T15 — Finalize**
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
