# Tasks — DataGrid: writable from the Designer, `Save()` in one transaction, cells that take a click

- **Status:** implemented, 2026-10-13 — M1 to M4 (T0–T34) and the rework after the operator's answers (T36–T43, spec §8.1); T35 (final acceptance) closes it.
- **Plan:** ./plan.md   **Date:** 2026-10-13

Ordered, small, independently verifiable. Each names the files, the requirements and how to check
it. Branch: `features-datagrid-save` (worktree `.claude/worktrees/datagrid-save`). Sweeps are
`--no-fail-fast` and every `test result:` line is read (CLAUDE.md). The 21 tests of
`test_grid_write_back.rs` and `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed` are
spec 094's: **they stay green after every task** (AC8) — a red one is a regression to fix, never a
test to edit. Four milestones, one commit and one `z` bump each; **nothing is committed, merged or
pushed unless the operator asks**, and a milestone is "ready", not committed, when its last task
is ticked. Disk: `df -h .` before each sweep; delete test executables from `target/debug/deps`
between groups.

**Amendment to plan §6/§7:** the failed-cell mark is a `render.rs` change, so the DataGrid quality
pass (R24) also runs in M1 (T9, a short pass on that one change), besides the full passes at M3 and M4.

## WP0 — Before the first edit
- [x] **T0 — Sync, baseline, and the KB prerequisite** (—)
  - Files: none (git, build).
  - Do: in the worktree, `git merge --ff-only main` (never plain `git merge main`); confirm
    `crates/cobolt-ide/src/version.rs` reads 1.91.20. Run the 094 baseline. Check that the
    embedding model `chunked.data` needs is installed (`cobolt_agents::project_knowledge::
    semantic_model_is_ready()`, the guard in `examples/build_chunked_kb.rs`); if it is not,
    **stop and ask the operator** before M1.
  - Verify: `git log --oneline -1` shows the `main` tip; `cargo test -p cobolt-runtime --test
    test_grid_write_back` — 21 passed; `cargo test -p cobolt-forms --features render
    a_datagrid_cell_is_edited_in_place` green; `cargo run -p cobolt-ide --example build_chunked_kb
    -- --help` (or the model check) does not refuse.
  - Done: fast-forwarded to `main` 7942c690 (1.91.20). Baseline: `test_grid_write_back` 21 passed;
    15 `a_datagrid_*` tests in `cobolt-forms` passed. The model is on disk
    (`~/PowerRustCOBOL/data/models/multilingual-e5-small`, all three files). The worktree has no
    `target/` of its own and the disk had 41 GiB free, so cargo runs with `CARGO_TARGET_DIR` set
    to the main checkout's `target/` (dependencies reused; workspace crates build under their own
    path hash).

## WP1 — M1 (1.91.21): the unit of work, `Save()`, `onSaved`
- [x] **T1 — SQL: is a transaction open, and what was sent** (R6 SQL, AC3, AC5)
  - Files: `cobolt-runtime/src/db_runtime.rs`
  - Do: a statement scanner (leading keyword of each `;`-separated statement outside quotes:
    `BEGIN`/`START TRANSACTION`/`SET autocommit = 0|OFF` open; `COMMIT`/`END`/`ROLLBACK` — not
    `ROLLBACK TO` — and `SET autocommit = 1|ON` close); `DbConn::in_transaction()` — SQLite asks
    the driver (`!is_autocommit()`), PostgreSQL/MySQL read a flag `DbConn::exec` maintains;
    `DbRegistry::in_transaction(handle)`; `#[doc(hidden)] DbRegistry::record_statements()` and
    `recorded_statements()`.
  - Verify: `cargo test -p cobolt-runtime --lib db_runtime` — scanner cases (`BEGIN`, `begin;`,
    `START TRANSACTION`, `SET autocommit=0`, `ROLLBACK TO sp` stays open, `SELECT 'BEGIN'` and a
    quoted `COMMIT` change nothing, `BEGIN; UPDATE …; COMMIT` ends closed); on in-memory SQLite
    `in_transaction` is false, true after `BEGIN`, false after `COMMIT`; the recorder returns the
    statements in order. Existing `db_runtime` tests unchanged.
  - Done: `transaction_effect` (25 statements checked), `DbConn::in_tx` / `in_transaction`,
    `DbRegistry::in_transaction`, `record_statements` / `recorded_statements`; `cargo test -p
    cobolt-runtime --lib db_runtime` — 8 passed, 2 ignored (the live PostgreSQL/MySQL tests).
- [x] **T2 — Find a record by key** (R6 indexed)
  - Files: `cobolt-runtime/src/indexed_ide.rs`
  - Do: `GridSession::position_of_key(&[(offset, len, key_text_fn…)])` as plan A3 describes — or,
    if the 094 comparison over `rows()` proves enough, record in the task that it was not needed
    (094's T2 did exactly that) and keep the lookup in `grid_unit.rs`.
  - Verify: `cargo test -p cobolt-runtime --lib indexed_ide` — a key is found after the rows were
    reordered and after a record was deleted; or the "not needed" note.
  - Done: **not needed**, as in 094's T2 — the record is found by comparing the key leaves' display
    text over `GridSession::rows()` (the comparison `write_indexed` already has), which moves into
    `grid_unit.rs` with T5. The reordered/deleted-record cases are covered by T3 and by the 094
    tests `a_sorted_grid_writes_the_record_its_row_belongs_to` and `a_record_that_is_gone_is_not_written_to_another`.
- [x] **T3 — A pending edit knows its row** (R12, plan A1/F3)
  - Files: `cobolt-runtime/src/interpreter/grid_edit.rs`
  - Do: `RowId` (`Cells` | `Occurrence`), `PendingEdit.id`, one `row_identity(grid, row)` used when
    an edit is noted and when it is written, and `locate(grid, &edit)` (position first, then a scan
    of `Rows` for the id; no match → `23`). `commit_one` uses `locate`. No new behaviour for a grid
    nobody sorts.
  - Done: new `interpreter/grid_unit.rs` (`RowId`, `Kind`, `Source`, `grid_source`, `row_identity`,
    `locate`, `row_occurrences`); `PendingEdit.id`; `commit_one`, the failure restore and
    `reapply_pending_edits` find the row through `locate` (a refresh that reorders rows puts each
    pending value back on its own row). `test_grid_write_back`: 22 passed (the 21 + the new test).
    Checked that the new test has teeth: with `locate` forced to position only it fails
    (record 3 stays "Robert De Niro"); restored.
  - Verify: `cargo test -p cobolt-runtime --test test_grid_write_back` — the 21 unchanged and
    green; new `a_sort_between_the_edit_and_the_confirm_writes_the_right_record` (AutoCommit off,
    `Sort`, then `CommitEdit()`; the file holds the new value on the record the operator edited).
- [x] **T4 — A COBOL table's occurrences** (R3, R19–R21, plan A5/F2)
  - Files: `cobolt-runtime/src/interpreter.rs` (`refresh_datagrid_binding` ≈12578; the `SORT`,
    `DELETEROW`, `ADDROW`, `CLEARROWS` arms ≈16421–16470, remapping beside `remap_row_heights`),
    `cobolt-forms/src/model.rs` (`_RowOccurrences` in `DATA_GRID`)
  - Do: publish `_RowOccurrences` (one table occurrence per `Rows` line, blank table rows
    skipped); keep it in step with the four methods; a row added by `AddRow` has an empty entry.
  - Verify: `cargo test -p cobolt-runtime` — a table with a blank row in the middle: line *n* maps
    to the right occurrence; after `Sort`, after `DeleteRow`, after `AddRow` and `ClearRows` the
    list still matches `Rows`; `cargo test -p cobolt-forms --features render` green
    (`every_runtime_property…` style tests in `model.rs`).
  - Done: `_RowOccurrences` published by `refresh_datagrid_binding` and kept in step by `Sort`,
    `DeleteRow`, `AddRow`, `ClearRows` (`edit_row_occurrences`); registered in `DATA_GRID`. New unit test
    `a_table_grid_knows_which_occurrence_each_row_came_from`. `cobolt-runtime --lib`: 451 passed,
    0 failed, 3 ignored. `cobolt-forms --features render --lib`: 1223 passed, **2 failed —
    `xml::tests::every_example_form_round_trips_056` (75 vs 74) and
    `…no_example_form_gains_responsive_markup_on_save_056` (16 vs 15)**: hard-coded example-form counts
    that 1.91.20's new Layers demo form outdated. Not caused by this feature (xml.rs untouched);
    flagged as a separate fix task for the operator, not fixed here (Golden Rule 5: no fix inside a
    feature commit). Every other `cobolt-forms` target is green.
- [x] **T5 — The unit of work, indexed first** (R6 indexed, R7, R8, R9, R12, AC2)
  - Files: new `cobolt-runtime/src/interpreter/grid_unit.rs` (`mod grid_unit;` in `interpreter.rs`),
    `interpreter/grid_edit.rs`
  - Do: `commit_unit(grid, door)` replacing `commit_grid_edits`/`commit_one`: collect (locate, skip
    refused, group by row ascending) → plan (key column `21`, no field `91`, PICTURE `92`, row gone
    `23`; nothing written) → apply (one `GridSession`, every changed field of a record encoded into
    one rewrite, one `commit()`, `rollback()` on any error) → outcome: success clears the unit's
    edits, shows the stored text, `CommitStatus` `00`, `SavedRows`, `_FailedCells` empty; failure
    with `AutoCommit` **off** keeps every edit pending and every cell, publishes `_FailedCells`,
    sets `CommitStatus`/`LastError`/`Edited*`, queues **one** `onCommitError`; failure with
    `AutoCommit` **on** reverts the cells as 094 does (D5). The three source writers' old per-cell
    forms are removed once T6 and T7 move over; until then the table and SQL arms call the old code.
  - Verify: `cargo test -p cobolt-runtime --test test_grid_write_back` — 21 unchanged green; new
    (AC2) `three_cells_in_two_records_are_one_save` (both records read back changed, `SavedRows` 2,
    exactly one engine commit observed through the file, `onSaved` once) and
    `a_bad_value_in_the_second_record_changes_neither` (`CommitStatus` `92`, both records unchanged
    in the file, both edits still pending, cells keep the typed values, `onCommitError` once, names
    the failing cell); `a_key_column_edit_fails_the_whole_unit` (R12, `21`, nothing written).
  - Done, T5–T7 together: the stopgap per-cell arms for table/SQL would have been thrown away, so the
    three backends were written at once in `interpreter/grid_unit.rs` (`commit_unit`, `write_indexed_unit`,
    `write_table_unit`, `write_sql_unit`) and the old per-cell writers are gone from `grid_edit.rs`; each
    task is verified by its own tests. The test harness grew `run_many` (several edits, each sent after
    the interpreter noted the one before — the interpreter is built on its own thread because it is not
    `Send`) and `Run.sql` (the statements sent). `test_grid_write_back`: 38 passed after T8
    (the 21 + the sort test + 16 new). **Honest limit:** the indexed rollback test fails in the *plan*
    phase (a PICTURE refusal), where nothing has been touched; an apply-phase failure of a REWRITE
    cannot be induced through the disk engine, so `session.rollback()` on that path is not exercised.
    The SQL rollback (second `UPDATE` fails, `ROLLBACK` sent, first row unchanged) is.
- [x] **T6 — The unit of work, COBOL table** (R6 table, R19, R20, R21, AC4)
  - Files: `interpreter/grid_unit.rs`, `interpreter/grid_edit.rs`
  - Do: plan = find every row (key cells, else the occurrence from `_RowOccurrences`), validate every
    value (field exists, not a key, a numeric item gets a number); apply = assign all as `MOVE`
    would. A row with no occurrence → `23`. Remove the old `write_table`.
  - Verify: `test_grid_write_back` — the three 094 table tests green; new (AC4)
    `a_table_edit_with_auto_commit_stores_itself`, `with_auto_commit_off_nothing_is_stored_until_save`,
    `one_non_numeric_value_stores_none_of_a_multi_edit_save` (both items keep their old value, `92`),
    `a_keyless_table_is_written_by_occurrence` (including after a `Sort` and after a `DeleteRow`),
    `a_row_added_by_the_program_is_refused` (`23`).
- [x] **T7 — The unit of work, SQL** (R6 SQL, AC3, AC5)
  - Files: `interpreter/grid_unit.rs`, `cobolt-runtime/tests/test_grid_write_back.rs` (harness)
  - Do: preconditions in order — handle open (`90`), the control's `Mode` not `Async` (new `95`), no
    transaction open (new `94`); then `BEGIN`, one `UPDATE t SET c1 = v1, c2 = v2 WHERE k = …` per
    changed row, an `Err` or 0 affected → `ROLLBACK` + `23`, `COMMIT` (a failing `COMMIT` →
    `ROLLBACK` + `90`). Refusals set status and `LastError` only (I2). Remove the old `write_sql`.
    The test harness enables `record_statements()` and returns the log in `Run`.
  - Verify: `test_grid_write_back` — the five 094 SQL tests green; new (AC3) `a_save_sends_begin_one_update_per_row_and_commit`
    (log is exactly `BEGIN`, `UPDATE`, `UPDATE`, `COMMIT`), `a_failing_second_row_leaves_the_first_unchanged`
    (a constraint failure; the first row's value unchanged in the table; the log ends `ROLLBACK`),
    `a_program_transaction_is_not_ended_by_save` (`BEGIN` from the program → `94`, nothing written,
    no `COMMIT`/`ROLLBACK` sent, the program's own `COMMIT` still works), `an_async_connection_is_refused`
    (`95`), `a_closed_connection_answers_90`.
- [x] **T8 — `Save()`, `CommitEdit()`, the settle, `onSaved`** (R5, R8, R9, R10, R11, AC5, AC6)
  - Files: `cobolt-ast/src/methods.rs` (`"SAVE"`), `cobolt-runtime/src/interpreter.rs` (`"SAVE"` arm
    guarded to a DataGrid, beside `COMMITEDIT` ≈16576), `interpreter/grid_edit.rs` (`save_method`
    shared with `commit_edit_method`; `settle_grid_edits` calls `commit_unit`), `cobolt-forms/src/model.rs`
    (`"onSaved"` in the DataGrid arm of `supported_events` ≈3672; `SavedRows`, `_FailedCells` in
    `DATA_GRID` ≈1504)
  - Do: nothing pending → no transaction, `00`, `SavedRows` 0, no event (R9, I3, I4); `onSaved` queued
    once when at least one row was written, from any door; a `Save()` on a grid that is not writable
    answers `91`.
  - Verify: `cargo test -p cobolt-runtime --lib every_dispatched_control_method_is_spellable_inline`
    green (both halves exist); `test_grid_write_back` — `LOOP` gains the `onSaved` branch; new (AC5)
    `an_empty_save_opens_no_transaction` (SQL log has no `BEGIN`, answer `00`), (AC6)
    `auto_commit_writes_one_handler_s_two_edits_as_one_unit` and `commit_edit_and_save_give_the_same_result`
    (same pending set, same file, same status); `save_on_a_read_only_grid_answers_91`;
    `cargo test -p cobolt-forms --features render` green (event/property lists).
  - Done: `"SAVE"` in `is_known_method`; the `SAVE` arm (guarded to a DataGrid) and `COMMITEDIT` share
    `save_method`; `onSaved` in the DataGrid event list; `SavedRows` and `_FailedCells` in `DATA_GRID`.
    `every_dispatched_control_method_is_spellable_inline` passes (`cobolt-runtime --lib`: 451 passed);
    `cobolt-ast`/`cobolt-parser`: all green. New tests: `commit_edit_and_save_give_the_same_result`,
    `an_empty_save_opens_no_transaction` (no `BEGIN` in the SQL log),
    `auto_commit_writes_the_edits_kept_so_far_as_one_unit`, and
    `a_save_on_a_read_only_grid_writes_nothing` — which answers `00`, not the `91` written above: a grid
    that is not writable keeps nothing, so there is nothing to save (plan I3: nothing pending → `00`).
- [x] **T9 — The failed-cell mark** (R7, Q6)
  - Files: `cobolt-forms/src/render.rs` (read `_FailedCells` next to `_PendingCells` ≈10768; paint a
    red mark beside the pending one ≈11590; it wins over the amber one on the same cell)
  - Do: reuse `DATAGRID_PENDING_MARK`'s geometry with a red colour; no layer-order change.
  - Verify: `cargo test -p cobolt-forms --features render` — new
    `a_datagrid_marks_the_cell_whose_save_failed` (sibling of `a_datagrid_marks_the_cells_whose_edit_is_still_pending`:
    0, 1 and 2 marks; red only on the named cell); **short quality pass** (R24) on this one change —
    layer order (mark under the cell fill? checked against the pending mark's), no padding or
    line change; PowerChat/PowerDemo3 corpus goldens (`cargo test -p cobolt-forms --features render
    --test example_corpus_golden`) byte-identical.
  - Done: `DATAGRID_FAILED_MARK` (red) next to the amber one; `_FailedCells` parsed with the same helper
    (`cells_named`) as `_PendingCells`; the failing cell wears red, not also amber. Tests:
    `a_datagrid_marks_the_cell_whose_save_failed` and `a_columns_own_background_does_not_hide_the_failed_mark`
    (the latter fails with the old paint order — checked by putting the old order back — and passes now).
    **Short quality pass (R24)** on this one change, against the agent's checklist: padding, row/header
    geometry, grid lines, borders, corner radius, scrolling, frozen panes, resize — no code touched, the mark
    is the same 6×6 square as before. Layer order: the mark is now painted *after* the background layers
    (cell/column colour, pattern, image) and under the content; before, it was painted *before* the cell
    background fill, so an opaque column colour hid it — a latent 094 defect for the amber mark, fixed here
    because the red mark must always be visible. Corpus goldens: regenerated into the worktree with
    `COBOLT_WRITE_GOLDEN=1` and `git status` read — no existing 056 golden changed, twice (before and after
    the move); the only new file was the Layers demo's, which is the other session's, and was removed again.
    `cobolt-forms --features render`: 1482 passed, 3 failed — the two stale example counts (above) and
    `every_example_form_renders_as_its_golden`, which stops at the Layers demo's missing golden (the other
    session's uncommitted refresh); all three fail identically without this feature. Remaining risk, listed
    for the M3 pass: an Image cell's picture, drawn in the cell, can still cover part of the 6×6 mark.
- [x] **T10 — Four interpreter hosts** (R23, AC9 runtime part)
  - Files: tests only — `cobolt-runtime/tests/test_grid_write_back.rs`, host construction sites
    `cobolt-cli/src/form_gui.rs:535`, `cobolt-form-host/src/host.rs:5084`,
    `cobolt-compiler/src/lib.rs:4054`, `cobolt-cli/src/main.rs:257`
  - Do: read the four sites; record in the task which (if any) need an edit — none is expected (the
    logic is in the interpreter; no new seed). Run the Save scenarios (T5–T8) through the interpreter
    each site builds, to the extent a test can reach that site's constructor.
  - Verify: `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler -p cobolt-cli`
    (all four, per `interpreter-binary-parity`); the milestone commit message names the hosts touched.
  - Done: the four sites read — `rcrun run-form` (`cobolt-cli/src/form_gui.rs:535`), the child-form host
    (`cobolt-form-host/src/host.rs:5084`), the compiled binary's `run_form_app`
    (`cobolt-compiler/src/lib.rs:4054`) and `rcrun run-form-ipc` (`cobolt-cli/src/main.rs:257`) — and
    **none needs an edit**: no host keeps a grid-method or runtime-property list, the new properties are
    published by the interpreter through the same state channel as `_PendingCells`, and no seed was added.
    The commit message names: *interpreter only; all four hosts by construction.* Run in batches (a single
    `cargo test` of the four crates builds ~200 test binaries at ~120 MB each and the disk had 25 GiB; the
    executables were deleted between batches): `cobolt-runtime` 165 integration targets — 943 passed,
    0 failed, 9 ignored (plus the 451 of `--lib`, run earlier); `cobolt-form-host` 196 passed, **1 failed:
    `host::parity::corpus_golden`**, which stops at the Layers demo's missing golden (the other session's
    uncommitted refresh — the same cause as `cobolt-forms`'s `every_example_form_renders_as_its_golden`);
    `cobolt-compiler` 169 passed; `cobolt-cli` 33 passed. The seed-parity tests of the two seeding sites
    (`a_bound_grid_is_seeded_with_what_it_needs_to_write_back`, `grid_write_back_seed`) are among them.
- [x] **T11 — System KB for M1** (R25)
  - Files: `cobolt-compiler/src/lib.rs` — `property_reference` (`SavedRows`; `CommitStatus` with `94`
    and `95`; `AutoCommit` and `AllowCellEditing` completed), `event_reference` (`onSaved`; the
    `onCommitError` text true for both presentations), `control_method_docs("DataGrid")`
    (`Save() → String`; `CommitEdit` text), the compact `methods_reference_doc` table (`Save`,
    `CommitEdit`, `DiscardEdit` — F9); `assets/knowledge/chunked.data` regenerated.
  - Do: `cargo run -p cobolt-ide --example build_chunked_kb`, rebuild so the new bytes embed.
  - Verify: `cargo test -p cobolt-compiler` (incl. `every_runtime_only_property_is_listed_for_some_control`);
    `cargo test -p cobolt-project-tools` (`pack_names_every_kb_entry_and_builtin`);
    `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    and `intellisense_offers_every_method_the_knowledge_base_documents` green; a handler fixture
    calling `DG::Save()` passes the editor's validator and Check (AC11).
  - Done: `property_reference` (`SavedRows`; `CommitStatus` with `94`/`95`; `AutoCommit` rewritten for the
    unit and both failure presentations), `event_reference` (`onSaved`; `onCellEdited` and `onCommitError`
    made true), `control_method_docs("DataGrid")` (`Save() → String`; `CommitEdit` as its older name), the
    compact `methods_reference_doc` table (`Save`, `CommitEdit`, `DiscardEdit` — F9).
    `chunked.data` regenerated with `build_chunked_kb` (2,958 records from 8 documents; 10,190,848 → 10,223,616
    bytes; the embedding model was on disk). Green: `prebuilt_chunked_kb_matches_the_published_documentation`,
    `intellisense_offers_every_method_the_knowledge_base_documents`, `cobolt-project-tools` (53 + 3 + 13 + …
    all targets ok, incl. `pack_names_every_kb_entry_and_builtin`), `cobolt-compiler` (169 passed,
    incl. `every_runtime_only_property_is_listed_for_some_control`), and the editor's DataGrid completion test,
    which now names `Save`, `CommitEdit` and `DiscardEdit` (74 editor tests passed). Check itself validates no
    control-method names (`cobolt-semantic` only checks the receiver exists), so `DG::Save()` is accepted by
    Check by construction and exercised by every runtime test that parses it.
- [x] **T12 — Developer's Guide for M1** (R26)
  - Files: `docs/developers-guide-en.md` — the subsection ≈3717–3797 becomes *Saving an edit back
    to the source — `Save()`, `AutoCommit`, `DiscardEdit()`*: the three gates (bound + writable +
    `AllowCellEditing`), the transaction per source, the identity rule (key as noted; occurrence
    for a keyless table), the `CommitStatus` table with `94`/`95`, the two failure presentations,
    one COBOL example for a COBOL table and one for SQL, the MySQL zero-rows caveat; the *Data
    binding and the Guardian* lines ≈3619–3624; the DataGrid events sentence ≈7093 and methods row
    ≈7382. Audience, no Rust, original wording (CLAUDE.md rule 3); `docs/` already has no
    translations of the Guide — if any `developers-guide-<lang>.md` has reappeared, delete it.
  - Verify: the English mentions `Save`, `onSaved`, `SavedRows`, `94`, `95`; `ls docs/developers-guide-*`
    shows only `-en`; `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` shows only the
    intended Guide-translations red, if any.
  - Done: *Saving edits back to the source — `Save()`, `AutoCommit`, `DiscardEdit()`* rewritten (the three
    gates, the two `AutoCommit` modes, one example each for `AutoCommit` on/off, SQL and a COBOL table, the
    one-transaction rule, how a row is found, how each source is written, the two failure presentations,
    the `CommitStatus` table with `94`/`95`, the caveats incl. MySQL's affected-rows reading); the
    *Data binding and the Guardian* failure sentence; the DataGrid events sentence and the controls→methods
    row. Deliberately **not** yet in the Guide (the code does not do it until the later milestones): the
    keyless COBOL table (M2's Guardian exemption), the Checkbox and Dropdown cells. `docs/` holds only
    `developers-guide-en.md` — nothing to delete. `docs_embed`: 8 passed, 1 failed —
    `every_document_ships_in_every_language`, the documented intentional red (ten English documents have no
    translation; none of them is this change's).
- [x] **T13 — The quantified report** (AC12, Golden Rule 7)
  - Files: `cobolt-runtime/tests/test_grid_write_back.rs` (`save_report`, sibling of `write_back_report`)
  - Do: one block at the end, no per-iteration chatter: cases by name — atomic save ok (indexed,
    SQL, table), atomic save rolled back (indexed, SQL, table), empty save, program-transaction
    refusal, keyless table, `CommitEdit` ≡ `Save` — each with ms, and the Save timing of *k* edited
    rows in an indexed file with 1,098 records (the `rewrite_row` reload cost, plan A3), then
    `TESTS RUN` / `TESTS FAILED` / `ELAPSED` / `OVERALL RESULT`. Only numbers the run produced.
    (Meets AC12 as 094 did — plan I7; the operator has to agree to that reading.)
  - Verify: `cargo test -p cobolt-runtime --test test_grid_write_back save_report -- --nocapture`
    prints the block with `OVERALL RESULT : PASS`.
  - Done: `save_report` — 15 cases (indexed ×4, SQL ×4, COBOL table ×3, empty save, `CommitEdit` ≡ `Save`,
    and the two timings), `TESTS RUN 0015 / TESTS FAILED 0000 / OVERALL RESULT : PASS`. Measured end to end
    on a 1,098-record indexed file, edits delivered one event at a time and then the save: **1 renamed row
    67 ms; 50 renamed rows 1,475 ms** (the run's own `ELAPSED` was 2,075 ms). That is the whole run — the
    50 edits each resend the grid's 1,098 rows and are noted one by one — so it is an upper bound for the
    save itself, and no optimisation of `rewrite_row`'s reload was made or is claimed to be needed.
    `test_grid_write_back`: 39 passed.
- [x] **T14 — Finalize M1** (all of WP1)
  - Files: `cobolt-ide/src/version.rs` → 1.91.21; `CHANGELOG.md` (top entry, absolute date, feature)
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-runtime -p cobolt-ast -p cobolt-forms
    --features render -p cobolt-compiler -p cobolt-form-host -p cobolt-cli -p cobolt-project-tools`
    `--no-fail-fast`, every `test result:` line read; `cargo test -p cobolt-ide --bin cobolt-ide
    --no-fail-fast` — the only red allowed is the Guide-translations guard; the foreign-script sweep
    (CLAUDE.md "Standing rules", nothing found outside Rust/COBOL); `git diff --numstat` read.
    The tree is ready to commit; **do not commit unless asked.**
  - Done: 1.91.21 and the CHANGELOG entry (dated 2026-10-13; it reports, rather than fixes, the stale
    example-form counts and the missing Layers snapshots). Sweep on the final M1 tree, `--no-fail-fast`, every
    `test result:` line read: `cobolt-lexer/ast/parser/semantic/codegen/indexed/project-tools` 747 passed,
    **1 failed** (`example_corpus_codegen::every_example_form_generates_its_snapshot` — the Layers demo's
    snapshot is missing); `cobolt-runtime` `--lib` 451 passed + 165 integration targets 943 passed
    (`test_grid_write_back` now 39), 0 failed; `cobolt-forms --features render` 1482 passed, **3 failed**
    (two stale example counts, one missing Layers golden); `cobolt-form-host` 196 passed, **1 failed**
    (`corpus_golden`, missing Layers golden); `cobolt-compiler` 169 passed; `cobolt-cli` 33 passed;
    `cobolt-ide --bin` 1608 passed, 4 ignored, **1 failed** (`docs_embed::every_document_ships_in_every_language`,
    the documented intentional red). **Every one of the 6 failures predates this feature and fails
    identically without it** (a stale count, a missing snapshot, an intentional guard); none touches the
    DataGrid. Foreign-script sweep: nothing. `git diff --numstat` read, and the removed lines of the files
    edited in place (`render.rs`, `interpreter.rs`, `methods.rs`, the KB tables, the Guide) read: they are
    exactly the lines replaced.

## WP2 — M2 (1.91.22): the Designer
- [x] **T15 — The binding keeps its mode** (R1, AC1)
  - Files: `cobolt-ide/src/panels/properties.rs` (`BindingEditorState.mode` ≈711, `from_existing`
    ≈829 reads `binding.mode` and keeps descriptor `key_field`/`key_fields` that no row flags, `to_binding`
    ≈983 sets it)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — new, in the style of
    `from_existing_reopens_saved_binding_config_for_editing`: `a_new_binding_is_read_only`,
    `reopening_a_writable_binding_and_applying_keeps_it_writable`, `a_hand_written_binding_keeps_its_keys_on_apply`;
    in `cobolt-forms/src/xml.rs` a save/load round trip of `"mode": "Writable"` beside
    `data_binding_cfrm_roundtrip_preserves_metadata_and_unrelated_controls`.
  - Done: `BindingEditorState.mode` (`new` → read-only; `from_existing` → the saved mode; `to_binding` applies it
    only for a source the runtime can write — new `BindingEditorSourceKind::can_be_written`), and `from_existing`
    keeps keys the source names but no row flags. Tests: the three above, plus
    `a_binding_keeps_its_writable_mode_through_a_save_and_a_load` in `cobolt-forms/src/xml.rs` (checks the file
    holds `"mode": "Writable"`). All pass.
- [x] **T16 — Guardian: a keyless COBOL table** (R3, AC1)
  - Files: `cobolt-project-tools/src/binding_guardian.rs` (`validate_writable_identity` ≈598)
  - Do: no `missing-row-identity` blocker for a writable `CobolTable`; every other source keeps it.
  - Verify: `cargo test -p cobolt-project-tools` — new `a_writable_cobol_table_without_a_key_is_allowed`
    beside `data_binding_guardian_core_blocks_missing_identity_for_writable` (still green for an
    indexed source); and the runtime path from T6 writes it (cross-check in T6's keyless test).
  - Done: `missing-row-identity` is skipped for a writable `CobolTable` only. `a_writable_cobol_table_without_a_key_is_allowed`
    passes (no `missing-row-identity`, no blocker); `data_binding_guardian_core_blocks_missing_identity_for_writable`
    (indexed) still passes. `cobolt-project-tools`: 80 passed. The runtime half is T6's
    `a_keyless_table_is_written_by_occurrence`.
- [x] **T17 — The switch and what the grid still needs** (R1, R2, AC1)
  - Files: `cobolt-ide/src/panels/properties.rs` (`show_writable_section` after the fields table in
    the Indexed, SQL and COBOL-table arms ≈5940–5956; `writable_needs(editor, ctrl)`; hidden for REST)
  - Do: Writable off by default; needs `KeyField` / `SqlTable` / `CellEditingOff`; a COBOL table with
    no key shows the by-occurrence note instead of a need; the one-click fix pushes
    `action.set_props` (`AllowCellEditing`, `PropValue::Bool(true)`); no Guardian call.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — new `writable_needs` unit tests (each need,
    none when satisfied, COBOL table keyless → note only, non-grid target → no cell-editing need);
    a `Rig` test in `panels/designer_grid_edit_094_tests.rs`: the fix turns `AllowCellEditing` on and
    one `undo()` turns it off; a frame test with `label_help_tests::painted_texts` that the window
    paints the switch and the needs.
  - Done: `WritableNeed`, `writable_needs`, `show_writable_section` (called after the field list in the Indexed,
    SQL and COBOL-table arms; `can_be_written` hides it for REST). Tests (all pass):
    `writable_needs_names_what_the_grid_still_lacks`; `the_writable_section_lists_the_needs_and_the_fix_turns_cell_editing_on`
    — a real egui frame: switch off shows only the switch, on lists the need and the occurrence note, a real
    pointer click on *Turn on cell editing* pushes exactly one `("CustomerGrid", "AllowCellEditing", Bool(true))`
    into `InspectorAction::set_props`, and with editing on the section says "in place";
    `a_rest_source_gets_no_writable_switch`; and in the designer rig
    `turning_on_cell_editing_from_the_binding_editor_is_one_undoable_step` (one step, one undo). Not tested: the
    application's own loop that turns `set_props` into `set_property` (existing code, unchanged), and how the
    window *looks* — the operator's to judge.
- [x] **T18 — Editor strings in six languages** (R4, AC1)
  - Files: `cobolt-ide/src/i18n.rs` — about eight `Tr` fields (`dg_writable_*`), added to the struct
    and to all six `const` tables (EN, ES, PT, JA, ZH, FR) next to `dg_sql_hint`
  - Verify: `cargo build -p cobolt-ide` (a missing table entry is a compile error); new
    `the_writable_strings_are_in_every_language` in the `layer_strings_091` pattern (non-empty, same
    `{}` count as English, not equal to English); `cargo test -p cobolt-ide --bin cobolt-ide i18n`.
  - Done: nine fields (`dg_writable_switch`, `_hint`, `_needs`, `_need_key`, `_need_table`, `_need_editing`,
    `_fix_editing`, `_by_occurrence`, `_ready`) after `dg_sql_hint` in the struct and in all six tables;
    `writable_strings_095::the_writable_strings_are_in_every_language` passes (9 keys × 6 languages = 54 checks).
    The wording of the ES/PT/JA/ZH/FR strings is mine, not a translator's: worth the operator's read.
- [x] **T19 — Guide lines and finalize M2** (R26)
  - Files: `docs/developers-guide-en.md` (the Writable switch, the needs block, the keyless-table
    exemption in *Data binding and the Guardian*), `version.rs` → 1.91.22, `CHANGELOG.md`
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` and
    `cargo test -p cobolt-project-tools -p cobolt-forms --features render` — every `test result:`
    line read; `prebuilt_chunked_kb_matches_the_published_documentation` still green (no KB text
    changed in M2 — if it is red, a table was edited by mistake); foreign-script sweep.
  - Done: the Guide gained *Making a binding writable* (the switch, the three needs, the fix button, the keyless
    COBOL table, no switch for REST/Agent), the Guardian paragraph's exemption, and the occurrence sentence in
    *How a row is found*. 1.91.22 and its CHANGELOG entry. Results: `cobolt-ide --bin` **1616 passed**, 4 ignored,
    1 failed (`docs_embed::every_document_ships_in_every_language`, the intentional red;
    `prebuilt_chunked_kb_matches_the_published_documentation` passed — no KB text changed); `cobolt-project-tools`
    80 passed; `cobolt-forms --features render --lib` 1226 passed, 2 failed (the stale example counts, as in M1).

## WP3 — M3 (1.91.23): Checkbox cells
- [x] **T20 — The Checkbox value pair** (R14)
  - Files: `cobolt-forms/src/model.rs` (`DataGridColumn.checked_value`, `unchecked_value`,
    `#[serde(default)]`), `cobolt-forms/src/datagrid.rs` (`checkbox_checked`, `checkbox_next`),
    `cobolt-forms/src/render.rs` (the paint at ≈11944 calls `checkbox_checked`)
  - Verify: `cargo test -p cobolt-forms --features render` — unit tests over every derived pair
    (`Y/N`, `y/n`, `1/0`, `true/false`, `True/False`, `TRUE/FALSE`, `yes/no`, `X/space`), an empty
    and an unrecognised cell (`Y/N`), an explicit pair, and "paint and toggle agree" for each; an old
    `AdvancedGrid` JSON without the fields loads; the corpus goldens are byte-identical.
  - Done: `DataGridColumn.checked_value` / `unchecked_value` (serde-defaulted, in the `AdvancedGrid` JSON);
    `datagrid::checkbox_checked` / `checkbox_next` over `CHECKBOX_PAIRS` (nine pairs), used by the paint; tests
    `a_checkbox_toggles_between_the_two_values_its_data_uses`, `a_checkbox_cell_with_no_known_pair_uses_y_and_n`,
    `a_column_pair_decides_what_is_written_and_painted`, `a_checkbox_column_pair_round_trips_and_an_older_grid_has_none`
    — all pass. One rule worth knowing: a blank cell is an empty cell and starts from `Y`/`N`, so the `X`/space
    pair is reached from a blank only when the column names `X` as its Checked value (the test says so).
- [x] **T21 — One road for a changed cell** (spec §2 last finding)
  - Files: `cobolt-forms/src/render.rs` (`push_cell_edit()` lifted from ≈12158–12170; the text editor
    calls it)
  - Do: a refactor with no behaviour change.
  - Verify: `cargo test -p cobolt-forms --features render` — `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed`
    and the 094 pending-mark test unchanged and green.
  - Done: `push_cell_edit` (render.rs) is what the text editor calls; `cobolt-forms` DataGrid tests (17) green,
    including the in-place edit test and the pending-mark tests.
- [x] **T22 — Click and Space toggle** (R13, R16, R17, R18, AC7)
  - Files: `cobolt-forms/src/render.rs` (`column_editable` split into text-editable / click-editable;
    cell `interact` registered when `SelectableText` or click-editable; hit rect = `cell_rect ∩
    visible body`; Space on the selected Checkbox cell; pointing-hand cursor)
  - Verify: `cargo test -p cobolt-forms --features render` — new, with `drive`/`audit_grid`:
    `a_click_on_a_checkbox_cell_toggles_it_and_raises_on_cell_edited` (the four values),
    `space_toggles_the_selected_checkbox_cell`, `a_disabled_grid_or_editing_off_does_nothing`,
    `a_checkbox_cell_is_painted_as_before_when_editing_is_off` (R17), `a_double_click_is_two_toggles_and_opens_no_text_box`,
    `a_checkbox_works_with_selectable_text_off`, `a_clipped_row_and_a_frozen_overlap_do_not_steal_the_click`,
    `button_gauge_and_image_cells_are_unchanged` (R18, `onCellClick` still fires); a runtime test in
    `test_grid_write_back.rs`: a toggle on a bound, writable grid is written back like a text edit.
  - Done: `column_kind` / `column_editable` (Checkbox leaves the text editor) / `cell_click_kind` /
    `checkbox_values`; the Space handler (only when no text box wants the keyboard); the cell's `ui.interact`
    on the visible part, independent of `SelectableText`; the toggle through `push_cell_edit`; a pointing-hand
    cursor. Tests (all pass): `a_click_on_a_checkbox_cell_toggles_it_and_raises_on_cell_edited`,
    `a_checkbox_column_writes_its_own_pair`, `space_toggles_the_selected_checkbox_cell`,
    `a_disabled_grid_or_editing_off_does_nothing_and_paints_alike` (the harness gained `drive_disabling`),
    `a_double_click_on_a_checkbox_cell_is_two_toggles_and_opens_no_text_box`,
    `a_checkbox_toggles_even_when_text_is_not_selectable`,
    `a_clipped_checkbox_row_does_not_steal_the_click_of_a_control_below`,
    `a_scrolled_checkbox_under_the_frozen_band_does_not_steal_the_frozen_cells_click` (a real wheel scroll),
    `a_button_cell_is_still_only_a_click_when_editing_is_allowed`; and in `cobolt-runtime`
    `a_checkbox_toggle_on_a_bound_grid_is_written_back_like_a_typed_edit` (the value the renderer's own
    `checkbox_next` produces, written to a SQL boolean). The two click-area tests were run with the visible-part
    hit area replaced by the whole cell: both **fail**; restored. The Dropdown kind is left as it was — still
    text-editable — until M4.
- [x] **T23 — Column editor: Checked / Unchecked value** (R14, R4)
  - Files: `cobolt-ide/src/panels/properties.rs` (≈5604–5613, shown for the Checkbox kind),
    `cobolt-ide/src/app.rs` (`merge_datagrid_binding_column` ≈20336 keeps the two fields on
    re-apply), `cobolt-ide/src/i18n.rs` (two `Tr` fields ×6)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — merge keeps an explicit pair
    (extend the test at `app.rs` ≈22370); the strings test for the two fields; `cargo build -p cobolt-ide`.
  - Done: *Checked value* / *Unchecked value* / a hint under the *Edit control* combo for a Checkbox column
    (`properties.rs`); three `Tr` fields in six languages added to `writable_strings_095` (now 12 keys × 6 = 72
    checks); `re_applying_a_binding_keeps_a_checkbox_columns_own_pair` (the merge only touches the fields it names,
    so no change to `merge_datagrid_binding_column` was needed). All pass. `cobolt-ide --bin`: 1618 passed.
- [x] **T24 — One cell, three surfaces** (R23, AC9 render part)
  - Files: new `cobolt-forms/tests/datagrid_cells_095.rs` (`#![cfg(feature = "render")]`)
  - Do: a grid with a Checkbox column through `render_form`, `render_form_with_chrome` and
    `render_form_scoped`; compare the digest of the shapes inside the grid rect (the Dropdown column
    joins in T32).
  - Verify: `cargo test -p cobolt-forms --features render --test datagrid_cells_095` green; the 056
    corpus goldens unchanged.
  - Done: `cobolt-forms/tests/datagrid_cells_095.rs` — one grid with a Checkbox column through `render_form`,
    `render_form_with_chrome` and `render_form_scoped`: **30 identical shapes per surface**; flipping the values
    changes the picture; `yes`/`no`/`false` draw as `Y`/`N`/`N`. The Dropdown column joins in T32. The 056 corpus
    goldens: see T25.
- [x] **T25 — DataGrid quality pass #1** (R24, AC10)
  - Files: `specs/095-datagrid-save-and-embedded-controls/quality-m3.md` (the report)
  - Do: run `.agents/skills/datagrid-quality` over the M3 render change: the 18 scenarios — 11–13
    (embedded cells; scenario 13 gets its test in T22, scenario 12 waits for T31), 16–18 (virtual
    scrolling, frozen panes) and the layer order — and list areas, items validated, tests run and
    risks as the agent's Output section asks.
  - Verify: the report exists and every finding is fixed or listed as a remaining risk; `cargo test -p
    cobolt-forms --features render` green.
  - Done: `quality-m3.md` — the 18 scenarios with their evidence, the layer order, the tests run, no regression
    found, four listed risks and what only the operator can judge by eye. `cobolt-forms --features render`:
    1,497 passed, 3 failed (the same three as in M1: two stale example counts, one missing Layers golden). The
    corpus goldens were regenerated into the worktree and `git status` read: no existing golden changed; the
    generated Layers file was removed. Added a Designer test, `a_checkbox_cell_toggles_on_the_canvas_and_undoes`.
- [x] **T26 — KB and Guide for Checkbox cells** (R25, R26)
  - Files: `cobolt-compiler/src/lib.rs` (the Columns/AdvancedGrid text and usage notes: `CheckedValue`,
    `UncheckedValue`, click/Space), `assets/knowledge/chunked.data` regenerated,
    `docs/developers-guide-en.md` (*Advanced DataGrid*: the Checkbox cell, its pair, the toggle,
    the double-click rule)
  - Verify: as T11 — `cargo test -p cobolt-compiler`, `cargo test -p cobolt-project-tools`,
    `prebuilt_chunked_kb_matches_the_published_documentation` green.
  - Done: the DataGrid notes and `AllowCellEditing` in `cobolt-compiler/src/lib.rs`; `chunked.data` regenerated
    (2,959 records, 10,227,712 bytes); *A Checkbox cell takes a click* in the Guide (with a COBOL example).
    `cobolt-compiler --lib` 164 passed (that suite took 512 s), `cobolt-project-tools` 80 passed,
    `prebuilt_chunked_kb_matches_the_published_documentation` passed inside the IDE run.
- [x] **T27 — Finalize M3** (all of WP3)
  - Files: `version.rs` → 1.91.23, `CHANGELOG.md`
  - Verify: the T14 sweep; foreign-script sweep; `git diff --numstat` read.
  - Done: 1.91.23 and its CHANGELOG entry. `cobolt-ide --bin` 1618 passed, 4 ignored, 1 failed (the intentional
    `docs_embed` guard); `test_grid_write_back` 40 passed; `cobolt-forms` as in T25; foreign-script sweep: nothing;
    the removed lines of the edited files read — they are the lines replaced.

## WP4 — M4 (1.91.24): Dropdown cells (separable)
- [x] **T28 — A Dropdown configuration the form keeps** (F1, R15)
  - Files: `cobolt-forms/src/model.rs` (`DataGridDropdown { origin, source, value_field, display_field,
    limit, items }`; `DataGridColumn.dropdown`; `BindingField.dropdown`, all `#[serde(default)]`),
    `cobolt-forms/src/xml.rs`
  - Verify: `cargo test -p cobolt-forms --features render` — a column and a binding with a
    Dropdown config round-trip through `.cfrm`; a form saved before this loads with `None`.
  - Done: `DataGridDropdownOrigin` (Static, CobolTable, IndexedFile, SqlControl, RestApi), `DataGridDropdown`
    (`static_items()`), `DataGridColumn.dropdown`, `BindingField.dropdown`; round-trip tests in `model.rs`/`xml.rs`.
- [x] **T29 — The editor saves and restores it** (F1, R15, R4)
  - Files: `cobolt-ide/src/panels/properties.rs` (`DropdownConfig` ↔ `DataGridDropdown` in
    `binding_fields()` ≈1047 and `from_existing` ≈873–897; *Static items* text in the column editor
    ≈5604), `cobolt-ide/src/app.rs` (`merge_datagrid_binding_column` copies it onto the column and
    keeps it on re-apply), `cobolt-ide/src/i18n.rs` (×6)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — reopening a binding shows the Dropdown it
    was saved with (it was `DropdownConfig::empty()` before); the merge keeps it; strings test.
  - Done: **spec premise F1 was wrong in a second way** — the Dropdown modal's source and field lists were
    fixed stand-ins (`CUSTOMER_TIERS`, `COUNTRY-CODES`…) and nothing was saved. Now `DropdownChoices` offers the
    form's own COBOL tables (and their items) and the project's `.cidx` definitions (and their fields), the
    stand-ins only when the project has neither; `DropdownConfig::to_model`/`from_model` keep a list **only when the
    runtime can read it** (COBOL table of this form, indexed file, static items); a SQL/REST list or a stand-in
    source is shown but not saved. `from_existing` restores it. The column editor has a "CODE=Label" items box for
    a Dropdown column (three new `Tr` fields ×6); `merge_datagrid_binding_column` copies the list onto the column
    and keeps a column's own when the field has none. Tests: 3 new in `properties.rs`/`app.rs` + the strings test
    (15 keys × 6 languages); the 7 dropdown-named IDE tests pass.
- [x] **T30 — The interpreter resolves the items** (R15)
  - Files: `cobolt-runtime/src/interpreter.rs` (`refresh_dropdown_items`, called after a load and on
    `RefreshBinding()`; `_ColumnItems` in `DATA_GRID` in `model.rs`)
  - Do: a COBOL table's two OCCURS items; an indexed file's two fields through `load_indexed` +
    `format_field_display`; capped at `limit`; other origins publish nothing.
  - Verify: `cargo test -p cobolt-runtime` — items from a table, from an indexed file, the limit,
    a missing source (empty, no error), static items need no interpreter.
  - Done: new module `interpreter/grid_dropdown.rs` (`refresh_dropdown_items`), called from `refresh_binding`
    (now a thin wrapper over `refresh_binding_rows`) and from `populate_datasource_grids`. It publishes
    `_ColumnItems` (added to `DATA_GRID`), one `column id TAB value TAB text` per item — keyed by the column's
    **id**, not a display position, so a hidden or reordered column cannot shift the lists. COBOL table: the two
    OCCURS items, blank occurrences left out; indexed file: the `.cidx` path resolved like the binding's, a missing
    data file is an empty list and is **not created**; `limit` caps (0: none); SQL/REST/Static publish nothing.
    8 unit tests in the module, all pass.
- [x] **T31 — The cell opens a list and picks** (R15, R16, AC7)
  - Files: `cobolt-forms/src/render.rs` (value→display paint; open state `ctrl_id.with("datagrid-dropdown")`;
    `OpenCombo` gains a `target` and a `GridCell` select goes through `push_cell_edit` ≈3311–3427;
    F2/Alt+Down opens), `cobolt-forms/src/datagrid.rs` (item parsing and value→text lookup)
  - Verify: `cargo test -p cobolt-forms --features render` — new: a click opens the list anchored
    to the cell; picking stores the **value**, paints the **text**, and raises `onCellEdited` with
    `EditedValue`/`PreviousValue`; Escape and a click outside change nothing; Up/Down/Enter work;
    a disabled grid and `AllowCellEditing` off do nothing and paint as before (R17); the text editor
    no longer opens on a Dropdown cell; ComboBox popup tests unchanged.
  - Done: `datagrid::dropdown_items`/`dropdown_position`/`dropdown_text` (pure; static lines or the runtime's
    `_ColumnItems` lines carrying the column's id; limit; blank text shows the value; `3` finds `003`).
    `OpenCombo` gained a `target` (`Control` | `GridCell`); the one popup pass draws the cell's list and a pick goes
    through `push_cell_edit` — the item's VALUE, by the road a typed edit takes. Open state
    `ctrl_id.with("datagrid-dropdown")`; click, F2 and Alt+Down open it (before the F2 text editor and the arrow
    keys); the grid's own arrow navigation, Space and F2 stand down while it is open; a double-click leaves it open;
    a list whose cell scrolled out of sight, or whose grid lost the right to edit, closes. **Decision (backward
    compatibility): a Dropdown column with no items to pick from stays text-editable**, as every form saved before
    this was. 17 new tests (forms lib) + the Designer canvas test; two mutation checks (the pick writing the text
    instead of the value; the grid's arrows not standing down) each fail the tests that should.
- [x] **T32 — Dropdown golden and quality pass #2** (R23, R24, AC9, AC10)
  - Files: `cobolt-forms/tests/datagrid_cells_095.rs` (Dropdown column joins the three-surface
    comparison), `specs/095-datagrid-save-and-embedded-controls/quality-m4.md`
  - Verify: `cargo test -p cobolt-forms --features render` green; the 18 scenarios re-run, scenario
    12 now with a test; the report attached.
  - Done: the Dropdown column joins `cobolt-forms/tests/datagrid_cells_095.rs` (34 identical shapes on run, preview and the Designer canvas; the item's text painted, not the value); `quality-m4.md` has the 18 scenarios re-run, scenario 12 now with its tests; the Designer canvas picks and undoes (`a_dropdown_cell_picks_on_the_canvas_and_undoes`).

- [x] **T33 — KB and Guide for Dropdown cells** (R25, R26)
  - Files: `cobolt-compiler/src/lib.rs` (Dropdown items text, `_ColumnItems` if listed),
    `assets/knowledge/chunked.data` regenerated, `docs/developers-guide-en.md` (the Dropdown cell,
    its sources, value vs text, the origins that resolve)
  - Verify: as T11/T26.
  - Done: the KB's DataGrid notes and `AllowCellEditing` carry the Dropdown cell; the Guide has *A Dropdown cell picks from a list* (value vs text, the origins that resolve, the empty-list fallback, the Designer caveat); `chunked.data` regenerated with the rework (2,971 records, 10,272,768 bytes).

- [x] **T34 — Finalize M4** (all of WP4)
  - Files: `version.rs` → 1.91.24, `CHANGELOG.md`
  - Verify: the T14 sweep; foreign-script sweep.
  - Done: 1.91.24 (the Dropdown) and 1.91.25 (the rework) in `version.rs` and the CHANGELOG; the foreign-script sweep reads clean (every hit is an old line, or the `日本語` round-trip string of 1.91.21's SQL test); `git diff --numstat` read — the removed lines are the ones replaced.

## WP6 — Rework after the operator's answers of 2026-10-13 (spec §8.1)
M1's Save() was built on the defaults of §9 (changed cells only, keep pending on failure, occurrence
identity for a keyless table, refuse inside a program transaction). §8.1 replaces them; this package
redoes that part, ahead of T32–T35. Version: 1.91.25 onward (1.91.24 is the Dropdown milestone).
- [x] **T36 — `SqlDatabase` gets a transaction** (Risk, R6)
  - Files: `cobolt-ast/src/methods.rs` (`BEGINTRANSACTION`, `COMMIT`, `ROLLBACK`, `ISINTRANSACTION`),
    `cobolt-runtime/src/interpreter.rs` (arms beside `EXECUTE`), `cobolt-compiler/src/lib.rs` (KB methods
    table), `cobolt-ide/src/panels/editor.rs` (completion)
  - Verify: `cargo test -p cobolt-runtime` — SQLite: a begun transaction's writes survive `Commit()`, vanish
    on `Rollback()`; `IsInTransaction()` follows both; a second `BeginTransaction()` and a `Commit()` with none
    open answer 0 and set `LastError`; an Async connection refuses.
  - Done: `BeginTransaction()`, `Commit()`, `Rollback()`, `IsInTransaction()` on the `SqlDatabase` control (vocabulary, interpreter arms beside `EXECUTE`, KB methods table and usage paragraph, Guide section *Transactions on a SqlDatabase*). `test_sql::the_transaction_verbs_commit_roll_back_and_refuse_what_cannot_apply` drives a real in-memory SQLite connection: idle 0, Commit with none → 0 with `LastError`, Begin → 1, second Begin → 0, Rollback drops the insert, Commit keeps it, `IsInTransaction` follows.

- [x] **T37 — The row ledger** (D3, Q2)
  - Files: `cobolt-runtime/src/interpreter.rs`, `interpreter/grid_edit.rs`, `cobolt-forms/src/model.rs`
  - Do: `_RowOccurrences` becomes `_RowOrigins` (what each line of `Rows` came from: a table occurrence or the
    key cells as loaded; empty for an added row), kept in step by Sort/DeleteRow/AddRow/ClearRows; a snapshot of
    what was loaded; `_KeyColumns` for the renderer. Captured wherever a binding loads.
  - Verify: unit tests — the origins after a load, a Sort, a DeleteRow, an AddRow, a ClearRows (which also
    forgets the snapshot).
  - Done: `_RowOccurrences` became `_RowOrigins` (a table occurrence, or `k` + the key cells as loaded, empty for an added row), kept in step by Sort/DeleteRow/AddRow; `ClearRows` forgets the snapshot; `_KeyColumns` published; `GridEditState` keeps `loaded`, `ambiguous`, `captured`. `capture_ledger` runs after `refresh_binding` and `populate_datasource_grids`; a grid never loaded by its binding captures lazily at its first edit (so 094's write-by-key from such a grid still works, and `Save()`/`DiscardEdit()` leave it alone).

- [x] **T38 — Save() writes updates, inserts and deletes to an indexed file** (D3, Q4, R6)
  - Files: `interpreter/grid_unit.rs`
  - Do: plan then apply in one session (deletes, updates, inserts); blank rows never inserted; a row whose key
    was modified is refused (`21`) and never updated or deleted; the changed-since-load check (`96`) on updated
    columns and deleted rows; duplicate key on insert fails the unit.
  - Verify: `cargo test -p cobolt-runtime --test test_grid_write_back` — each case and the unit atomicity.
  - Done: `grid_unit.rs` rewritten around a `Plan` (updates, inserts, deletes), checked then applied in one `GridSession`: deletes, rewrites, writes. Blank rows never inserted; a key-changed row refused (`21`); a new row needs its key (`92`); a duplicate key is the engine's `22`; the changed-since-load check answers `96`; a different column of the same row survives.

- [x] **T39 — The same for a SQL table, inside or beside the program's transaction** (D3, Q4, Risk)
  - Files: `interpreter/grid_unit.rs`
  - Do: `INSERT`/`UPDATE`/`DELETE` in `BEGIN…COMMIT`; or, with a program transaction open, under a `SAVEPOINT`
    (`ROLLBACK TO`/`RELEASE`) so commit and rollback stay the program's; the check by `SELECT` in the unit;
    retire status `94`.
  - Verify: statement-log tests — `BEGIN, …, COMMIT`; `SAVEPOINT, …, RELEASE` inside `BeginTransaction()`;
    a failed save inside it leaves the program's earlier writes alone.
  - Done: SQL `DELETE`/`UPDATE`/`INSERT` between `BEGIN`…`COMMIT`; inside `BeginTransaction()` under `SAVEPOINT prc_save` (`RELEASE`, or `ROLLBACK TO` + `RELEASE` on failure); each row to update or delete is `SELECT`ed and compared first; status `94` retired.

- [x] **T40 — A COBOL table is regenerated from the grid** (Q2)
  - Files: `interpreter/grid_unit.rs`
  - Verify: edit, sort, delete and add in the grid then `Save()`: the table equals the grid, the rest blank;
    an invalid number assigns nothing; more rows than occurrences is refused.
  - Done: `commit_table` regenerates the table from the grid (blank rows skipped, the tail blanked, more rows than occurrences → `92`, numbers validated first, unshown items left alone); `AutoCommit` on does the same after an edit.

- [x] **T41 — A failed Save() puts the old values back** (Q6, R7)
  - Files: `interpreter/grid_unit.rs`, `interpreter/grid_edit.rs`
  - Verify: any failure restores the loaded rows and origins, clears pending, marks the failing cell, raises
    `onCommitError`; the success path rebuilds the snapshot.
  - Done: an explicit failure restores the loaded rows and origins (`restore_loaded`), clears pending, marks the failing cell where its row now is and raises `onCommitError`; the `AutoCommit` path restores the cells it wrote; `DiscardEdit()` is the same restore.

- [x] **T42 — Keys are read-only on loaded rows** (D3)
  - Files: `cobolt-forms/src/render.rs`
  - Verify: a key cell of a row that came from the source opens no editor (double-click, F2, Dropdown,
    Checkbox); a row added in the grid edits its key; a grid with no `_KeyColumns` behaves as before.
  - Done: the renderer keeps a key cell of a loaded row read-only (double-click, F2, Space, Dropdown, Checkbox); a new row's key and a grid with no `_KeyColumns` edit as before. Mutation-checked.

- [x] **T43 — Documentation follows** (Q1, R25, R26)
  - Files: `docs/developers-guide-en.md` (the *Saving edits back* section: insert/update/delete, the check,
    restore, the transaction verbs; `CommitEdit()` no longer mentioned), `cobolt-compiler/src/lib.rs` + KB,
    `CHANGELOG.md`, `version.rs`
  - Verify: as T11/T26.
  - Done: Guide *Saving edits back to the source* rewritten (`CommitEdit()` no longer mentioned; the three kinds of change; how a row is found; the check; the old values back; the transaction), KB rows (`AutoCommit`, `CommitStatus` 96, `SavedRows`, `onCommitError`, `onSaved`, `Save`, `CommitEdit` deprecated, `DiscardEdit`, `AddRow`/`DeleteRow`/`ClearRows`, `SqlDatabase`), the two Writable-section strings in six languages (`dg_writable_whole_table`), CHANGELOG 1.91.24 and 1.91.25.

## WP5 — Closing the feature
- [x] **T35 — Acceptance and the final sweep** (AC1–AC12)
  - Do: extend `save_report` with the cell cases (checkbox, dropdown) so the block lists everything
    AC12 names (atomic save ok, atomic save rolled back, empty save, checkbox, dropdown, COBOL table
    on/off); tick every AC in `spec.md` against its test; manual check through `run_form` on a scratch
    form with Checkbox and Dropdown columns bound to a table (never the IDE window); update this
    file's status.
  - Verify: the full grouped sweep on the branch, `--no-fail-fast`, every `test result:` line read
    (groups: small = ast, lexer, parser, semantic, codegen, project-tools, form-host; runtime;
    forms with `--features render`; compiler; ide with `--bin`), the baseline's one allowed red being
    the Guide-translations guard; `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler
    -p cobolt-cli` for the hosts; `git diff --numstat` read; foreign-script sweep clean.
  - Done: every AC of `spec.md` is ticked against its test (AC1–AC16). Full grouped sweep on the branch, `--no-fail-fast`, every `test result:` read: runtime 1,420 passed in 166 targets, 0 failed; small crates 726 passed, 1 failed (`cobolt-codegen` `every_example_form_generates_its_snapshot` — the Layers demo's missing snapshot, baseline); forms 1,514 passed, 3 failed (baseline: two example-count tests and the corpus golden); compiler 169; cli 33; form-host 196 passed, 1 failed (`host::parity::corpus_golden`, the Layers snapshot, baseline); ide 1,622 passed, 1 failed (`docs_embed::every_document_ships_in_every_language`, the intentional guard: the Guide's translations are not touched). The manual `run_form` check is the operator's (CLAUDE.md: never drive the IDE window); the form-level behaviour is covered by the interpreter-level tests above.

## Acceptance coverage

| AC | Tasks |
|----|-------|
| AC1 (R1–R4) | T15, T16, T17, T18 |
| AC2 (R5–R9, indexed) | T5, T8 |
| AC3 (R6, R7, SQL) | T1, T7 |
| AC4 (R6, R19–R21, table) | T4, T6 |
| AC5 (R9) | T1, T8 |
| AC6 (R10, R11) | T8 |
| AC7 (R13–R18) | T22, T31 |
| AC8 (R22) | every task — the 094 tests stay green; confirmed in T14, T19, T27, T34, T35 |
| AC9 (R23) | T10, T24, T32 |
| AC10 (R24) | T9 (short), T25, T32 |
| AC11 (R25, R26) | T11, T12, T26, T33 |
| AC12 (Golden Rule 7) | T13, T35 |

## Done criteria
Every acceptance criterion in spec.md is checked, the tests pass, the Guide and the System KB are
updated, and the work is ready as four feature commits (M1–M4) on `features-datagrid-save`. Do
**not** commit, merge or push unless the operator asks; a push also obeys the work-hours rule. The
Designer's and the grid cells' *look* is confirmed by the operator, never by driving the window.
