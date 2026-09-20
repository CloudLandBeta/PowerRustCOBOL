# Plan — Printing a report into a Viewer

- **Status:** draft → **ready for `/tasks`**
- **Spec:** ./spec.md   **Date:** 2026-09-20

## 1. Approach

The feature is mostly **wiring parts that already exist**. The survey below is
what the design rests on, and each line was read in the source rather than
remembered:

| Already there | Where |
|---|---|
| `ASSIGN TO <word>` captures the device as a plain word | `parser.rs::parse_file_control_entry` |
| A text line written with trailing spaces removed | `exec_write`, the `LineSequential` arm |
| `LINAGE`, `LINAGE-COUNTER`, `AT END-OF-PAGE` | `Interpreter::advance_linage`, `linage_counters` |
| A control property written from COBOL, announced to every host | `Interpreter::obj_set` → `StateUpdate` |
| A Viewer picks its renderer from the **file extension** | `viewer::detect_format` |
| A Viewer session keyed by **path**; a new path loads | `host.rs::sync_viewer_sessions` |
| **Plain text paginated on form feeds** | `viewer::index_text` ("cuts at … a form-feed") |
| A document held in memory, with Save As honouring its bytes | `LoadBytes` → `viewer_bytes` |

**One thing is genuinely missing**, and it is the heart of R18/R19: `advance_linage`
**counts and emits nothing**. It maintains `LINAGE-COUNTER`, fires `AT END-OF-PAGE`
and returns — no blank lines for `ADVANCING n LINES`, no form feed for
`ADVANCING PAGE`, and it returns early when the FD has no `LINAGE` clause at all.
A print file's page breaks are therefore in the program's head, not in the bytes.
§4 and §5 say how this plan handles that without disturbing what NIST verified.

The shape, end to end:

1. **Declare** (R1, R2). The parser recognises `VIEWER` after `ASSIGN [TO]` and
   takes the following literal as the control id; `parse_organization` gains two
   arms matched by word. `FileControl` carries a new `viewer_target:
   Option<String>` beside `storage_mode` and `persist`, which are the same kind
   of PowerRustCOBOL extension on the same struct.
2. **Check** (R4, R20). `cobolt-semantic` reports `MARKDOWN`/`HTML` without
   `ASSIGN TO VIEWER`, and page control on a `MARKDOWN`/`HTML` file.
3. **Open** (R7–R11). `exec_open` builds `<form>-<unique>.<ext>` under
   `std::env::temp_dir()` and opens a writer on it; if that fails for any reason
   it opens an in-memory sink instead. Either way the file is an `OpenFile`
   variant of its own so `WRITE` and `CLOSE` can tell it apart.
4. **Write** (R5, R6, R18). The record is materialised exactly as today, then
   written as one text line, trailing spaces removed — the `LineSequential` arm's
   own rule. For `SEQUENTIAL` only, the `ADVANCING` clause is *emitted*: blank
   lines before or after, and a form feed for `PAGE`. `advance_linage` keeps
   counting as it does now, untouched.
5. **Close** (R13, R16, R17, R22). Flush, then hand the document over:
   `obj_set(target, "View1Source", path)` for a file, or the `LoadBytes` route
   (`viewer_bytes` + `Format` + `onLoaded`) for a memory report. Both are
   existing, tested paths that already reach all three hosts.
6. **Show** (R19). Nothing to write: the form feeds the runtime now emits are
   the page breaks `index_text` already cuts on.

## 2. Affected crates / files

- `crates/cobolt-ast/src/program.rs` — `FileOrganization::{Markdown, Html}`;
  `FileControl.viewer_target: Option<String>` (`#[serde(default)]`, so every
  serialised AST in the wild still loads — the compiler embeds one).
- `crates/cobolt-parser/src/parser.rs` — `parse_organization` gains `MARKDOWN`
  and `HTML` (by word, like `PAGE` and `REGISTERED` before them);
  `parse_file_control_entry` recognises the `VIEWER` device and its literal.
- `crates/cobolt-semantic/src/*` — two diagnostics (R4, R20).
- `crates/cobolt-runtime/src/interpreter.rs` — the `OpenFile` variant, the
  `exec_open` / `exec_write` / `exec_close` arms, the vertical-movement emitter.
- `crates/cobolt-runtime/src/files.rs` — nothing expected; the record image is
  materialised by the existing path.
- `docs/developers-guide-en.md` — a section under the report/printing material,
  written for a developer who knows `ASSIGN TO PRINTER`. English canonical only.
- `crates/cobolt-compiler/src/lib.rs` — **only if** a Viewer property, method or
  event changes. None is planned, so `assets/knowledge/chunked.data` is expected
  to stay byte-identical; if any table text does change, regenerate in the same
  commit.
- `crates/cobolt-ide/src/i18n.rs` — **no new keys expected.** A diagnostic that
  reaches an IDE panel would need six languages; a `FILE STATUS` and a runtime
  message are not UI strings.

## 3. Data / model changes

- **AST.** Two enum variants and one optional field. Both additive; the field is
  `#[serde(default)]` because `cobolt-compiler` serialises the AST with
  `bincode` into every built binary, and an older AST must still deserialise.
- **No `.cfrm` change.** The Viewer gains no property: the report arrives
  through `View1Source`, which already exists and is already persisted.
- **On disk.** A new kind of file appears in the OS temp directory. It is not a
  format anyone else reads — `.txt`, `.md` and `.html` are what they say they
  are — and nothing is migrated.

## 4. Key decisions & alternatives

**D1 — A VIEWER file is a writer, not a new I/O engine.**
`OpenFile::ViewerReport { sink, target, org, path }` where `sink` is
`Box<dyn Write>` over a `BufWriter<File>` or a `Vec<u8>`.
*Why:* the report is a stream of text lines; the INDEXED and RELATIVE engines
exist because their verbs are keyed, and a report has none of that.
*Rejected:* reusing `OpenFile::Writer` with a flag. `WRITE` and `CLOSE` need to
know the target control and the sink kind, and a flag on a variant that means
"ordinary file" is how two behaviours end up in one arm.

**D2 — The vertical movement is emitted for VIEWER files only.**
*Why:* `advance_linage` emits nothing today, for every sequential file in the
product. Making it emit for all of them would change the bytes every existing
report program writes — including the CCVS85 programs behind **NIST NC and SQ,
both finished at 100 % on both axes**. That is a separate change with a full
regression behind it (`nist-fix-requires-full-regression`), and it is not this
spec's to make.
*Rejected:* emitting everywhere "because it is more correct". It may well be,
and the gap is recorded in §5 as its own item — but a feature that silently
rewrites a finished conformance module is not a feature.

**D3 — On CLOSE, hand over a path; hold bytes only when there is no path.**
*Why:* a path makes the Viewer's existing session, Save As, Print and Share work
with no new machinery (spec §6). Bytes are the fallback R11 asks for, and the
`LoadBytes` path already proves that route end to end.
*Rejected:* always using bytes. Save As would have to invent a filename, Print
and Share would have to write the file out anyway (`host.rs:1130` does exactly
that for a `LoadBytes` document), and a large report would be held twice.

**D4 — `<unique>` is a timestamp + process id + a per-run counter.**
*Why:* no crate in the workspace depends on `uuid` directly; the requirement is
only that two reports never collide, and the operator's "UUID" names the shape,
not the crate. A collision would need the same form, the same process, the same
millisecond and the same counter.
*Rejected:* adding the `uuid` crate for one filename.
*(Open for the operator: if a v4 UUID is wanted literally, the dependency is
small and the change is one line — say so before `/implement`.)*

**D5 — R20's diagnostic is semantic, at `rcrun check`.**
*Why:* the developer learns before running, which is the whole point of a
diagnostic. The analyser sees both the `SELECT` and the `FD` of a program.
*Fallback:* if joining the `FD`'s `LINAGE` to a distant `SELECT` proves awkward
in `cobolt-semantic`, a runtime status is acceptable and the requirement says so.

**D6 — `OPEN EXTEND` appends to the same backing file (spec §7.5).**
*Why:* it is what EXTEND means, and a second report that continues the first is a
sensible thing to write. The alternative — refusing it like `INPUT` — is easy to
add later and impossible to remove once programs rely on it.

## 5. Risks & mitigations

- **Risk — the vertical movement changes more than intended.** Emitting blank
  lines and form feeds is new behaviour in a verb every report program uses.
  → *Mitigation:* D2 confines it to `OpenFile::ViewerReport`. A test writes the
  same program to a disk file and to a Viewer and asserts the disk bytes are
  **unchanged** from today's.

- **Risk — NIST regression.** `cobolt-runtime`'s write path is the one CCVS85
  drives.
  → *Mitigation:* after implementation, re-run the whole-suite census **and** the
  execution pass for every finished module (NC, SQ), not only a spot check. That
  is a task in `tasks.md`, not a hope.

- **Risk — `LINAGE-COUNTER` is one name for the whole program.**
  `advance_linage` writes `env.set_i64("LINAGE-COUNTER", …)` with the comment
  that one print file is the common shape. Two report files open at once share
  it.
  → *Mitigation:* out of scope, but state it in the Guide rather than let a
  developer discover it. Do not "fix" it here.

- **Risk — the temp file is read while it is being written.** The Viewer's
  session polls `View1Source` every frame; a path that appears before the writer
  has flushed would index a truncated document.
  → *Mitigation:* R14 already says nothing is shown before `CLOSE`, and `CLOSE`
  flushes and closes the file **before** `obj_set`. The test for R14 is what
  keeps the order honest.

- **Risk — a report that is never closed.** The program ends, the file is open,
  nothing is shown.
  → *Mitigation:* the runtime already closes open files at `STOP RUN`; the report
  path must go through the same close so the document appears. One test.

- **Risk — the three hosts drift.** A runtime behaviour that works under `rcrun
  run-form` and not in a compiled binary is this project's most repeated defect.
  → *Mitigation:* AC16, and the `interpreter-binary-parity` skill's checklist
  before the work is called done.

- **Risk — an enormous report.** A COBOL loop can write a million lines.
  → *Mitigation:* the file path costs one buffered write per record and the
  Viewer indexes lazily (that is what spec 058's background session is for). The
  **memory** fallback holds it all; note the asymmetry in the Guide.

## 6. Test strategy

**Parser / AST — `crates/cobolt-parser/tests/`**
- `a_report_declares_its_viewer_and_its_organization` — the three organizations,
  `ORGANIZATION IS` omitted, and the control id captured. Reports the parsed
  `(assign, viewer_target, organization)` triple per case. (AC1, AC2)

**Semantic — `crates/cobolt-semantic/tests/`**
- `markdown_without_a_viewer_is_reported` (AC-R4) and
  `page_control_on_a_markdown_report_is_reported` (AC12) — each asserting the
  message names the file, and printing the diagnostics it saw.

**Runtime — `crates/cobolt-runtime/tests/`**
- `a_report_is_written_to_a_file_beside_the_form` — 40 records, checks the temp
  file's name shape, its line count and that trailing spaces are gone. (AC3)
- `a_report_with_no_temp_directory_is_kept_in_memory` — temp dir pointed at an
  unwritable path; the document still arrives. (AC4)
- `a_closed_report_reaches_the_viewer_and_an_open_one_does_not` — asserts the
  `StateUpdate` for `View1Source` arrives on `CLOSE` and **not** before. (AC5,
  AC6)
- `a_second_report_replaces_the_first` (AC8), `an_empty_report_shows_an_empty_document` (AC-R22),
  `a_missing_viewer_fails_the_open_with_a_status` (AC13),
  `open_input_on_a_viewer_file_is_refused` (AC14).
- `advancing_moves_the_page_only_for_a_viewer_report` — the same program to a
  disk file and to a Viewer; the disk bytes match today's, the report carries the
  blank lines and the form feed. (D2's guard, AC10)

**Forms — `crates/cobolt-forms/tests/`**
- `a_report_breaks_its_pages_where_the_program_did` — feed the emitted text
  through `index_text` and assert the page count and the page boundaries. (AC11)

**COBOL, under GOLDEN RULE #7 — `tests/cobol/report-viewer/`**
- One program per organization, each printing a **result block**: the cases
  exercised (which `ADVANCING` forms, which organization), records written,
  elapsed time and records/sec, and the pass/fail tally. Not one line per record.

**Whole-suite, before it is called done**
- `cargo test -p cobolt-forms --features render`, `-p cobolt-runtime`,
  `-p cobolt-form-host`, `-p cobolt-ide --bin cobolt-ide`, each with
  `--no-fail-fast`, reading every `test result:` line.
- **NIST:** the whole-suite census and the NC + SQ execution passes.

**Manual / visual (the operator's, not the agent's — never drive the app)**
- A form with a Viewer and a Print button: run it, press it, see the report.
  Switch `Layout` to `Page` on a `SEQUENTIAL` report and turn the pages.
  Save As the result and open it outside the IDE.

## 7. Steering compliance

- [x] **i18n:** no new UI strings expected; if one appears it is a `Tr` field in
      all six languages.
- [x] **Generated code:** untouched — this is hand-written COBOL, and
      `cobolt-codegen` emits no `SELECT` for it.
- [x] **English dev guide** updated in the same change; translations already
      absent for the Guide, so GOLDEN RULE #8 costs nothing.
- [x] **Fix vs feature:** FEATURE → `features` branch (synced to `main` at
      `0e9d423` on 2026-09-20), commits separate from any fix, `z` bump per
      change, f=96 only if the operator asks.
- [x] **No "cobolt" in user-facing text; COBOL identifiers and generated source
      stay English** — including the report's own content, which is the
      developer's COBOL and never translated.
- [x] **System KB:** no control property/method/event changes planned, so
      `chunked.data` is expected unchanged; regenerate if that turns out false.
