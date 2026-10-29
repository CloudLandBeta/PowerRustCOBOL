<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — PartiQL for indexed files

- **Status:** approved (operator, 2026-10-29); implementation starts at T0
- **Plan:** ./plan.md   **Date:** 2026-10-29

Ordered, small, independently verifiable tasks. Each names its files, the requirements it satisfies and how to verify it. Paths are from the repo root.
Branch `features-108-partiql`. **Do not commit or push** unless the operator asks (the coordinator does). A fix found on the way goes to `fixes` in its own
commit, never into this feature. Tests run per crate: `cargo test -p <crate> --no-fail-fast` and **read every `test result:` line**; `cobolt-ide` needs
`--bin cobolt-ide`; `cobolt-forms` needs `--features render`. `df` first when a build fails oddly. Every new test ends with ONE result block (rule #7).

## Phase 0 — Baseline and spikes

- [x] **T0 — Baseline** (all)
  - Files: none (a block recorded at the end of this file, "Baseline").
  - Do: sync (`git rebase main`), `df`, then record the `test result:` totals of `cobolt-runtime`, `cobolt-ast`, `cobolt-forms --features render`, `cobolt-codegen`,
    `cobolt-project-tools`, `cobolt-compiler`, `cobolt-cli`, `cobolt-ide --bin cobolt-ide`, and the names of the tests already red (known: `docs_embed::every_document_ships_in_every_language`,
    `claude_validates_the_written_bundle`, two `external_crates_service` flow tests, per the 1.91.226 note).
  - Verify: the block exists with real numbers; every later phase compares against it.
  - Done: five of eight crates baselined, 0 red (see Baseline); compiler, cli, ide not baselined.

- [x] **T1 — Spikes that decide the design** (D1, D6, D9, D10, D14; plan findings 3 and 5)
  - Files: a throwaway test in `crates/cobolt-runtime/tests/` per question, deleted or turned into the real test of a later task; results written under "Spike results" at the end of this file.
  - Do: prove, with the real parser and interpreter and file:line evidence, (a) what `exec_method` receives for `TXT::Text` holding `2026`, for a `PIC 9(4)` item, for `NUD::Value`, for a literal; (b) that `IXF-1::CUST-NAME` lexes and
    parses as a property read, and works in `IF IXF-1::CUST-BALANCE < 0`; (c) what `Interpreter::run` does with `open_files` at the end and what an engine's `Drop` does with pending writes; (d) which host seeds
    the project root for `assets::resolve` and the control's properties in `rcrun run-form`, the embedded host and `run_form_app`, and whether a built binary carries the `.cidx` and data files; (e) how `ClickedRow` and `SelectedIndex` reach the interpreter's store.
  - Verify: each answer recorded with its evidence; if (a) contradicts D6, or (c) shows a verb-path defect, or (e) shows no seam, stop and report to the operator before T5 (O1 and O2 are decided; a contradiction reopens them). No production code changes.
  - Done: answers under Spike results.

## Phase 1 — The language core (pure; `cobolt-runtime::partiql`)

- [x] **T2 — Status 94, the value type, the lexer** (R6, R27)
  - Files: `crates/cobolt-runtime/src/indexed.rs` (`status::VALIDATION`), `crates/cobolt-runtime/src/partiql/{mod,value,lexer}.rs` (new), `crates/cobolt-runtime/src/lib.rs` (`pub mod partiql`).
  - Do: `Value { Num(CobolNumeric), Str(String) }` with the compare and `+` / `-` rules on `CobolNumeric` (no float); the lexer of §5.1 — keywords case-insensitive, names bare or in double quotes, the hyphen rule (`CUST-ID` is a name, `QTY - 1` is subtraction), single-quoted strings with doubled quotes, numbers `12 12.50 -3 1e3`, `?`, punctuation; every token carries a line and column.
  - Verify: `cargo test -p cobolt-runtime partiql::lexer` — table tests incl. `QTY-1` vs `QTY - 1`, keywords as quoted names, an unterminated string at its position.
  - Done: `cargo test -p cobolt-runtime --lib partiql` (lexer 10, parser 14, value 4, plan 3 tests).

- [x] **T3 — The parser** (R1, R6; AC4)
  - Files: `crates/cobolt-runtime/src/partiql/parser.rs` (new).
  - Do: the AST and parser for `SELECT`, `INSERT`, `UPDATE … SET/REMOVE … RETURNING`, `DELETE … RETURNING`; `WHERE` with `= <> != < <= > >= BETWEEN IN [..] IN (..) AND OR NOT ( ) IS [NOT] MISSING`, DynamoDB's precedence; `begins_with contains size attribute_type`, `EXISTS(SELECT …)`; value expressions with `+ -`; paths `A.B`, `A[n]`; `?`; `ORDER BY key [ASC|DESC]`; `"TABLE"."INDEX"`. Reject with `ValidationException: <construct> at line:col` every Non-goal (`COUNT`/aggregates, `LIMIT`, `OFFSET`, `JOIN`, `GROUP BY`, `DISTINCT`, `LIKE`, `AS`, subqueries outside `EXISTS`, `CREATE`/`DROP`, Boolean/`NULL`/Binary/set/map/list values, `list_append`, `set_add`, `set_delete`), an empty statement, and a `?` where a name belongs.
  - Do (cross-check step): derive a table of accepted-statement cases, in our own words and examples, from the official PartiQL ANTLR grammar (https://partiql.org/syntax/antlr, Apache-2.0; parser grammar `PartiQL`, lexer `PartiQLTokens`; reference only — plan D15) covering each accepted form of `SELECT`, `INSERT`, `UPDATE`, `DELETE` and `REMOVE`, each operator, and the precedence of §5.1 (comparison, `IN`, `BETWEEN`, functions, parentheses, `NOT`, `AND`, `OR`); run every case through the parser and assert the parse tree (grouping) the grammar implies. Where our subset deliberately differs (hyphenated names, `IN (…)`, list indexes from 1, rejected constructs), the case states the difference. No ANTLR, no Java, no generated code, no grammar file in the repo.
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_language` — every §5.1 form parses, and every case of the grammar-derived table parses to the grouping the grammar gives (or to the documented difference); every rejection names its construct and position; a truncation fuzz never panics. Prints the result block (statements parsed per second).
  - Done: `test_partiql_language`: 46 grammar-derived cases, 5 section-5.1 forms, 98,000 statements/s parse rate.

- [x] **T4 — The field codec** (R2, R4)
  - Files: `crates/cobolt-runtime/src/files.rs` (new `decode_field`, `encode_field`; `distribute` and `field_bytes` untouched), `crates/cobolt-runtime/tests/test_partiql_codec.rs` (new).
  - Do: decode a `FieldPos` slice to `Value` (numeric with its scale and overpunch or SEPARATE sign; alphanumeric/national/UTF-8 trimmed of trailing spaces) and encode a `Value` back (String truncated on the right, Number truncated to its decimals, integer overflow reported), by the rules `distribute` and `field_bytes` apply.
  - Verify: a differential test — for generated records over every usage and sign form (DISPLAY, COMP, COMP-3, BINARY, SIGN LEADING/TRAILING [SEPARATE], N, U, OCCURS, REDEFINES) `decode_field` equals what `distribute` leaves in the environment and `encode_field` what `field_bytes` reads back; result block reports fields checked. `cargo test -p cobolt-runtime --test test_partiql_codec`.
  - Done: `test_partiql_codec`: 5,023 fields, 0 failures. Differences found and kept: a trailing SIGN SEPARATE is mis-read by `distribute` (existing, listed for `fixes`); `encode_field` writes it correctly.

- [x] **T5 — Schema and analysis** (R2, R6, R7, R10, §5.2, §5.4; AC4, AC5)
  - Files: `crates/cobolt-runtime/src/partiql/{schema,analyze}.rs` (new).
  - Do: `Schema` from (a) FD text, (b) a `.cidx` via `indexed_ide::definition_text_of`, (c) a `RecordLayout`; the field table with qualifying path, occurrence list, scale, sign, class; the keys (RECORD KEY parts; alternates by name, else by first field name; duplicates flag); the table name (upper-cased `.cidx` stem). `analyze` resolves names case-insensitively, applies §5.2 (a declared field is never `MISSING`; a name that is not a field is a `ValidationException`; Number vs String is an error (Q10); `begins_with`/`contains`/`size` on a Number is an error; list indexes counted from 1, `[0]` and past-the-last refused; `size` per type), the key rules (`UPDATE`/`DELETE` pin the whole RECORD KEY with `=`; no `SET` on a key field; `ORDER BY` only a key-of-reference field), `ResourceNotFoundException` for another table or index, and the `?` count. Applies O2 and O3 (decided): `SELECT *` expands to one column per occurrence (`ITEM-QTY[1]` …) and `OCCURS` on a group is refused (`ValidationException`: not addressable).
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_language` extended — each AC4 statement gives its error with no file; a `.cidx`-built and an FD-text-built `Schema` of the same file are equal; the `PIC S9(6)V99` scale survives.
  - Done: `test_partiql_language`: 21 AC4 refusals with no file, `.cidx`- and FD-built schemas equal, scale kept.

- [x] **T6 — Read classification and key bytes** (R11; §5.3; AC1)
  - Files: `crates/cobolt-runtime/src/partiql/plan.rs` (new).
  - Do: classify keyed / range / scan; build the key bytes (split keys joined in order; numeric parts encoded by the T4 codec); the sound-by-construction rule of D5 (signed part, a literal that does not fit, a string longer than the part → scan with a filter); `IN` as several keyed reads in key order; `begins_with` on the leading string part as a range; the consumed vs leftover conditions.
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_language` — the classes of AC1's four statements are keyed, range, scan, keyed; a property test generates bounds on signed, decimal, unsigned and alphanumeric parts and compares the plan's result to a brute-force filter.
  - Done: `test_partiql_language`: 720 generated statements equal to a brute-force filter (146 keyed, 310 range, 264 scan).

- [x] **T7 — Executing `SELECT`** (R1, R11–R13, R19; AC1, AC2 core)
  - Files: `crates/cobolt-runtime/src/partiql/exec.rs` (new).
  - Do: run a planned `SELECT` on `&mut dyn IndexedStore` plus the schema — keyed reads, `start` + `read_seq` ranges (`Previous` for `DESC`), scans; every function and operator; projection (`*`, paths, functions, arithmetic) with names (a computed expression named by its text); the key of reference via `set_key_of_reference`; duplicates in read order; the result set as `Vec<Vec<Value>>`; the `Outcome` (status `00`, count, class). A new statement replaces the set, a failed one empties it.
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_executor` (first part) on a small file on each engine — equal to a `START` / `READ` loop.
  - Done: `test_partiql_executor` on three engines.

- [x] **T8 — Executing `INSERT`, `UPDATE`, `DELETE`** (R4, R5, R12, R14–R17, R27; AC3)
  - Files: `crates/cobolt-runtime/src/partiql/exec.rs`.
  - Do: `INSERT` (every RECORD KEY part required; the rest at initial values; `22` / `DuplicateItemException` for the key or a no-duplicates alternate); `UPDATE` (`SET` with the record's own fields and arithmetic, `REMOVE`, `RETURNING` all four forms; `23` / `ConditionalCheckFailedException` naming the condition; a Number that overflows its integer digits is a `ValidationException` and changes nothing); `DELETE` (a missing key answers 0 with `00`; a failed further condition `23`); `48` / `49` on an `INPUT` open; the engine's `35 37 39 90 93 99` passed through with its message; `LastError` built from names and positions only (R29).
  - Verify: the executor test, second part — AC3's table; a test binds `SECRET-42` to a mismatching `?` and asserts it appears in no error text.
  - Done: `test_partiql_executor` (AC3 rows, R29 secret check).

- [x] **T9 — Transactions** (R23, R25, R26; AC6)
  - Files: `crates/cobolt-runtime/src/partiql/exec.rs`.
  - Do: split on `;`, analyse every statement first (R26: nested unit, or a statement on a record an earlier one wrote, refused before anything runs), run in order binding `?` across the unit; `EXISTS(SELECT …)` as a condition check (`ConditionalCheckFailedException`); results of every `SELECT`/`RETURNING` concatenated; on failure undo **exactly the unit's own changes** by replaying before-image inverses in reverse (D3), status of the failing statement with `TransactionCanceledException: statement n: …`; a compensating-write failure is `90` naming the statement.
  - Verify: the executor test, third part — success answers 3; insufficient balance answers 0 with `23` and neither record changes; a failing third statement leaves the first two undone and a `WRITE` before the unit intact; a fault-injecting test engine proves the `90` path.
  - Done: `test_partiql_executor` incl. the fault-injecting engine (`90`).

- [x] **T10 — The executor suite at scale, three engines** (R18, R48; AC1, AC3, AC4, AC5, AC6, AC18)
  - Files: `crates/cobolt-runtime/tests/test_partiql_executor.rs` (complete).
  - Do: `CUSTOMERS` and `ORDERS` of spec §7, 100,000 records each, built by the test, on `PRCIDXD1`, redb and `STORAGE IS MEMORY`; every statement form, function, operator, every row of R27; the three read classes against a `START` / `READ` loop; `ORDER BY … DESC`; duplicates through an alternate key; `?` typing; `AutoCommit` handled in T13; the ONE result block names each form exercised, per engine and per phase the elapsed ms and the throughput (statements per second for keyed reads; records per second for range and scan reads, inserts, updates, deletes, units), and the pass/fail tally.
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_executor -- --nocapture` green on the three engines; the block's numbers are the run's own (AC18); temp containers removed (`df` before and after).
  - Done: 100,000 records on PRCIDXD1 and redb, 3,000 on MEMORY (its sequential read is O(n^2), see Spike results); numbers in the CHANGELOG/report.

## Phase 2 — The control, the files, the hosts

- [x] **T11 — The control's methods and properties** (R20–R22, R28, R43 parser half; AC2, AC16 parse half)
  - Files: `crates/cobolt-ast/src/methods.rs` (`"EXECUTETRANSACTION"`), `crates/cobolt-runtime/src/interpreter/partiql.rs` (new), `crates/cobolt-runtime/src/interpreter.rs` (class routing at the head of `exec_method`, as `is_aws` does; per-control result-set and cursor state), a fix in `cobolt-parser` only if T1(b) found one.
  - Do: `Execute`, `ExecuteTransaction`, `Fetch` (TAB-joined, a TAB inside a value shown as a space; past the end empty with `10`), `Get` (by name case-insensitive, path, or position from 1), `ColumnNames`, `ColumnCount`; `ResultCount`, `StatusCode`, `LastError` as properties; `Fetch` writes the item's leaves as control properties and clears the ones not projected; `CurrentRecordDataItem` receives the record; `StatusDataItem` receives `StatusCode`; a failed statement never stops the program and raises no `onError` (R28); arguments become parameters by D6. File access arrives in T12; here the arm runs against a stub provider in tests.
  - Verify: `cargo test -p cobolt-ast` and `cargo test -p cobolt-runtime --test test_partiql_control` — AC2's exact `Fetch` text (`00000042<TAB>-123.45<TAB>Recife`), `Get("CUST-BALANCE")` and `Get(2)` equal, `IF IXF-CUST::CUST-BALANCE < 0` true, `CUST-NAME` empty, second `Fetch` empty with `10`, `ResultCount` 1 then 3, failed `Execute` empties; `IXF-CUST::Execute( WS-STMT )` parses as a method call and `IXF-CUST::CUST-NAME` as a property read.
  - Done: `test_partiql_control` (AC2). Unsigned whole-number fields display zero-filled (`00000042`), the rule that makes AC2 and AC9's `1234.50` both true.

- [x] **T12 — File ownership: the control opens, borrows and closes the file** (R7–R10, R16–R18; AC7)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`path → key` index beside `open_files`; adoption branch in the INDEXED part of the OPEN verb; teardown in `run()`), `crates/cobolt-runtime/src/interpreter/partiql.rs`.
  - Do: D1 exactly: resolve the `.cidx` and data path; borrow an engine already open on the path (the form's `FD`, or another control); else synthesise a `FileSpec` from the definition and open it through `make_indexed_engine` under `acquire_share_lock`, creating a missing data file from the `.cidx` (`create_empty_from_definition`), registered under `<TABLE>:<id>`; a data file whose layout disagrees with the `.cidx` is `39`; a table name that is not the control's file is `ResourceNotFoundException` / `94`; `OpenMode` and `OperatorName` as `AutoOpen` uses them; the OPEN verb adopts a control-owned engine; control-owned engines are closed when the run ends; `AutoOpen` on a form without an FD opens through the interpreter's populate step.
  - Verify: `cargo test -p cobolt-runtime --test test_partiql_control` — AC7: a form with no `SELECT`/`FD` runs; the file is closed at the end (the `.lck` sidecar released); no data file → created from the `.cidx`; disagreeing layout → `39`; a form declaring `FD CUSTOMERS IS GLOBAL` and `OPEN I-O` in `onLoad` sees the verb's `WRITE` through the control and its `ROLLBACK` verb undoes the control's `INSERT`; two controls on one definition share one engine; `Execute("SELECT * FROM ORDERS")` on the `CUSTOMERS` control → `94`. **Then the full NIST census and the NC (95/95) and SQ (85/85) execution passes** (rule #9) and `cargo test -p cobolt-runtime --no-fail-fast`.
  - Done: `test_partiql_control`: no FD, created from the `.cidx`, INPUT on a missing file is 35, another table is 94, FD form shares one engine, `ROLLBACK` undoes the control's INSERT, lock released at the end. NOT done: the `39` check for a borrowed engine's layout; the NIST census/NC/SQ passes (the NIST program sources are not on this machine - only `newcob.val`); `AutoOpen` for a form with no FD at populate time.

- [x] **T13 — `AutoCommit`, and the facade skipped for a form with no FD** (R10, R24; AC6, AC7)
  - Files: `crates/cobolt-forms/src/model.rs` (`AutoCommit` on `IndexedFile`, three-state), `crates/cobolt-codegen/src/lib.rs` (`write_indexed_file_stubs`, the two `AutoOpen` loops, the `onDeactivate` close; a `declares_file` predicate over `cobol_structure.file_control`), `crates/cobolt-runtime/src/interpreter/partiql.rs`.
  - Do: D4 and D8. Default decided at run time by the predicate (on without a declared FD, off with one); a statement or unit commits per file when on; with it off, writes join the file's open transaction and the `COMMIT` / `ROLLBACK` verbs act. A form that declares the FD generates byte-identical facade COBOL.
  - Verify: `cargo test -p cobolt-codegen` (a golden: a declared-FD form's output unchanged; a no-FD form has no facade paragraphs and no `PERFORM <id>-OPEN`; that form passes Check), `cargo test -p cobolt-runtime --test test_partiql_control` — AC6's `AutoCommit` cases: off by default with the FD (a `WRITE` before the unit is undone by `ROLLBACK`), on: a successful unit survives a `ROLLBACK` that follows and commits a pending `WRITE`.
  - Done: `AutoCommit` property on `IndexedFile` (`Default` | `True` | `False`, default Default); `AutoOpen` opens at form start for an FD-less form (`test_partiql_control`, 30 checks); codegen golden `indexed-file-declared` pins the declared-FD output.

- [x] **T14 — Replace the "not reachable" claim** (R43; spec §8)
  - Files: `crates/cobolt-ide/src/form_runtime.rs`.
  - Do: keep `indexedfile_paragraphs_are_not_reachable_from_a_handler` for the paragraphs; add a test that `IXF-1::Execute(…)` in a handler generates, parses and analyses with no error.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide form_runtime`.
  - Done: `indexedfile_members_are_reachable_from_a_handler` in `form_runtime.rs`; the paragraphs test is unchanged.

- [x] **T15 — The built-ins `COBOL::"PARTIQL"`, `COBOL::"PARTIQL-ERROR"` and `COBOL::"PARTIQL-FETCH"`** (Q6, O4, D12, R20a, R48)
  - Files: `crates/cobolt-runtime/src/builtins.rs` (three rows, alphabetical: PARTIQL, PARTIQL-ERROR, PARTIQL-FETCH), `crates/cobolt-runtime/src/interpreter.rs` (`COBOL-PARTIQL`, `COBOL-PARTIQL-ERROR`, `COBOL-PARTIQL-FETCH` in `exec_call`), `tests/cobol/fileio/partiql_*.cbl` (new programs with one result block each), `crates/cobolt-compiler/src/lib.rs` (the built-in tables the KB prints, if they list built-ins).
  - Do: `( FILE-NAME STATEMENT RESULT-COUNT STATUS [param …] )` on the file the program's `FD` opened (`Schema` from its `RecordLayout`); `( FILE-NAME ROW STATUS )` answers the next item TAB-joined; arguments typed by PICTURE; the inline form only. `( FILE-NAME TEXT )` of `PARTIQL-ERROR` answers the file's last `LastError` (O4, decided), never a parameter value (R29); the test programs read it after a failing statement.
  - Verify: `cargo test -p cobolt-runtime builtins` (`the_table_is_every_builtin_the_interpreter_dispatches` green) and the `tests/cobol/fileio/partiql_*.cbl` programs via `rcrun run`, each printing its block (forms exercised, statements per second, tally).
  - Done: `test_partiql_builtin`; the builtins table test is green.

- [ ] **T16 — Three hosts, three engines** (R18, R49; AC8)
  - Files: `crates/cobolt-cli/tests/partiql_hosts.rs` (new), fixtures under `crates/cobolt-cli/tests/fixtures/` (new, a small project), host code only if T1(d) found a missing seed in `crates/cobolt-cli/src/form_gui.rs`, `crates/cobolt-form-host/src/host.rs` or `run_form_app` in `crates/cobolt-compiler/src/lib.rs`.
  - Do: the AC1, AC3 and AC6 forms run under `rcrun run-form`, as an embedded child form and in a built binary, on `PRCIDXD1`, redb and a `STORAGE IS MEMORY` file, and are compared.
  - Verify: `cargo test -p cobolt-cli --test partiql_hosts -- --nocapture` — identical results across the nine cells; the block lists each cell. (Rebuild `rcrun` before running: `cargo build -p cobolt-cli`; the `--bin` filter skips other binaries.)

## Phase 3 — The binding

- [~] **T17 — The PartiQL source in the model** (R30, R40, R41; AC9 load)
  - Files: `crates/cobolt-forms/src/model.rs` (`BindingSourceDescriptor::PartiQl`, `BindingSourceKind::PartiQl`, the `kind` / `fields` / `is_writable` arms), every exhaustive match over the descriptor (`crates/cobolt-codegen/src/data_binding.rs`, `src/lib.rs`, `crates/cobolt-project-tools/src/binding_guardian.rs`, `crates/cobolt-ide/src/panels/{data_binding,properties}.rs`).
  - Do: the variant with `source_control_id`, `query`, `parameters`, `fields`, `key_fields`, `writable` (all `#[serde(default)]`); round-trip; the loader's behaviour for an unknown variant is pinned (it reports the binding and keeps the form).
  - Verify: `cargo test -p cobolt-forms --features render` — a `.cfrm` saved by 1.91.223 (`examples/PowerDemo3/forms/Data/datagrid-form.cfrm` and a binding fixture) loads unchanged; the PartiQL source round-trips.
  - Done: `BindingSourceDescriptor::PartiQl` / `BindingSourceKind::PartiQl`, every field `#[serde(default)]`, appended last; `cobolt-forms/tests/partiql_binding_source.rs` (round trip, a short descriptor reads). The `PowerDemo3` datagrid form was not opened (credential rule); the existing source variants are untouched, and the codegen corpus goldens pass.

- [ ] **T18 — `RowSet`: one reader per source, one applier per target** (R31; D7)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`refresh_list_binding`, `refresh_chart_binding`, `refresh_marker_binding`, `refresh_scalar_binding`, `refresh_control_array_binding`, `refresh_datagrid_binding`).
  - Do: split each filler into "read a `RowSet` from the COBOL environment" and "apply a `RowSet` to the control"; **no behaviour change**.
  - Verify: run `cargo test -p cobolt-runtime --no-fail-fast`, `cargo test -p cobolt-forms --features render`, `cargo test -p cobolt-ide --bin cobolt-ide` before and after; every golden and binding test identical to T0. If any changes, revert this task alone and report.

- [~] **T19 — Codegen seeds the PartiQL binding** (R40, R41; AC9)
  - Files: `crates/cobolt-codegen/src/data_binding.rs` (`write_partiql_binding_seed`, the `BindingSourceDescriptor` matches near `:541`), `crates/cobolt-codegen/src/lib.rs` tests.
  - Do: for every approved target (DataGrid, ListBox / ComboBox, the seven charts, Knob / Gauge / Switch, Maps, repeating GroupBox) seed `_BindingKind=PartiQl`, the source control, the query, the parameter list, the fields and types, the target's own `_Binding*` mappings, then `RefreshBinding` in `COBOL-DATA-BINDINGS-POPULATE`. No `READ`, no COBOL loop.
  - Verify: `cargo test -p cobolt-codegen` — seed tests per target (the pattern of `grid_write_back_seed.rs`); the generated program has no `READ`; the existing `IndexedFile`, `Sql`, `CobolTable` seeds and the quick-bind corpus golden of `datagrid-form.cfrm` are byte-identical.
  - Done: `write_partiql_binding_seed` for DataGrid, ComboBox and ListBox: `cobolt-codegen/tests/partiql_binding_seed.rs` (seeds, one `RefreshBinding`, no `READ`). NOT done: charts, Knob/Gauge/Switch, Maps, repeating GroupBox.

- [x] **T20 — Refresh: parameters, the binding's own result set, following the control** (R32–R34, R37; AC9, AC11, AC14)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`refresh_binding_rows` arm for `"PartiQl"`), `crates/cobolt-runtime/src/interpreter/partiql.rs`.
  - Do: evaluate each parameter (a data item by its PICTURE; `CTRL::Property` as untyped text coerced to the compared field's type, a failed coercion being `94` — O2, R3a); run the query on the source control in a result set of the binding's own (the control's result set and `Fetch` cursor untouched); fill through T18's appliers; `RefreshBinding()` answers the item count; on failure keep what the target shows, set the control's `StatusCode` / `LastError`, write the debug channel, stop nothing. With an empty query, follow the control's result set after every `Execute` / `ExecuteTransaction` that produces one, and show it at populate. `BindingQuery` readable and writable (a non-`SELECT` refused with `94`, previous query kept).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide form_runtime` — AC9 (the master-detail grid; count answered), AC11 (empty at open, filled by a button's `Execute`, handler's own `Fetch` loop still reads from the first), AC14 (`BindingQuery`, the refused `DELETE`, the non-numeric parameter leaves the rows and sets `94`).
  - Done: runtime arm `refresh_partiql_binding` (own result set, follow-the-control, parameters from data items and `CTRL::Property` as untyped text, failure keeps the rows and sets `StatusCode`/`LastError`), `BindingQuery` refused with 94; `test_partiql_binding` (in `cobolt-runtime`, not `cobolt-ide`).

- [x] **T21 — `SelectedValue` on lists and on bound grids** (R35, R35a, Q8, O1, D9; AC9, AC12)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (where `SelectedIndex` and `ClickedRow` are written), `crates/cobolt-runtime/src/interpreter/partiql.rs`, `crates/cobolt-forms/src/model.rs` (the lists' value-field mapping; the dormant `ListValue` gains its meaning), `crates/cobolt-codegen/src/data_binding.rs`.
  - Do: a list publishes `SelectedValue` (the value field of the selected item; empty when none or unmapped); a DataGrid bound to a PartiQL source publishes it too (O1, decided; R35a): the text of the value column's cell (first column flagged `key`, else the first) in the row `ClickedRow` names, empty before any click, refreshed where `ClickedRow` is written and at each refresh.
  - Verify: AC12 — a ComboBox bound to `SELECT CUST-ID, CUST-NAME FROM CUSTOMERS` with value `CUST-ID`: empty before any pick, the third record's `CUST-ID` after picking the third; AC9's grid parameter `DG-CUSTOMERS::SelectedValue` is empty before a click and the clicked row's value-column cell after it, and a refresh with that parameter shows that customer's orders.
  - Done: `SelectedValue` on a PartiQL-bound ComboBox/ListBox (value field) and DataGrid (value column of `ClickedRow`), read on demand; `test_partiql_binding`.

- [ ] **T22 — Write-back of a writable DataGrid through the control** (R36; AC13)
  - Files: `crates/cobolt-runtime/src/interpreter/grid_unit.rs` (`Kind::PartiQl`, `grid_source` arm, `apply_partiql`), `crates/cobolt-runtime/src/interpreter/grid_edit.rs` if the ledger needs the key list.
  - Do: plan as today; apply through the executor as **one unit** (deletes, updates, inserts), identifying each record by the full RECORD KEY the projection must hold, with the compare against what was loaded (`96`), key cells never editable (`21`), `91` not writable, `98` pending, `23` the record is gone; the outcome reaches `CommitStatus`, `LastError`, `onCommitError`, `onSaved` as the indexed write-back does.
  - Verify: AC13 — editing two balances, adding and deleting a row, `Save()` writes all four as one unit (`00`, `onSaved`), visible to a `SELECT` on another control; a balance changed on disk in between → `96`, nothing written; `CUST-ID` cells not editable; a projection lacking `CUST-ID` is read-only.

- [ ] **T23 — Write-back of a repeating GroupBox through the control** (R36; O5)
  - Files: the interpreter's control-array card write-back (`refresh_control_array_binding` and its commit path), `crates/cobolt-runtime/src/interpreter/partiql.rs`.
  - Do: card edits, added and removed cards saved through the executor as one unit with the same statuses as T22. Kept (O5, decided); it stays last in the binding phase and nothing else depends on it.
  - Verify: a test form of cards over `CUSTOMERS`: an edited `TextBox` writes the record; a card with a changed key is refused `21`.

- [~] **T24 — The guardian** (R38; AC15)
  - Files: `crates/cobolt-project-tools/src/binding_guardian.rs` (`validate_source` arm; `validate_form_bindings_in(form, root)`), `crates/cobolt-ide/src/data_binding_guardian.rs`, `crates/cobolt-ide/src/panels/data_binding.rs` (repair labels), `crates/cobolt-project-tools/src/tools/check.rs` (pass the project root).
  - Do: blockers `missing-source`, `invalid-partiql`, `missing-row-identity`, `missing-parameter`; warning `full-scan`; loads the `.cidx` the source control names; `validate_form_bindings(form)` keeps its signature and behaviour.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` — one test per finding on one form: a Label as source; an `UPDATE` query; `NO-SUCH-FIELD`; a writable binding projecting `CUST-NAME` only; two `?` and one parameter; a `WHERE` on `CUST-BALANCE` (warning).
  - Done: three findings that need only the form - `missing-source`, `invalid-partiql`, `missing-parameter` (`cobolt-project-tools/tests/partiql_guardian.rs`). NOT done: `missing-row-identity`, `full-scan`, checking names against the `.cidx`.

- [ ] **T25 — The binding editor: source, parameters, fields, Test query** (R39, R45; AC15)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (PartiQL section beside `show_sql_source_section`; the real `IndexedFile` control list for this source), `crates/cobolt-ide/src/i18n.rs`, `crates/cobolt-ide/src/ide_text_data.rs`.
  - Do: the picker of the form's `IndexedFile` controls; a multi-line Query; the Parameters list (add, remove, reorder; each a picker of the form's data items and control properties); the Fields grid seeded from the projection against the `.cidx` (real names and PICTUREs, never `sample_indexed_field_rows`); the Writable switch with its "what the target still needs" list; Test query asking for one value per `?`, running the `SELECT` read-only (an `INPUT` open) and showing up to 50 items, the count, the read class and any error.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — the editor lists the form's two `IndexedFile` controls and no stand-in; the Fields grid carries the `.cidx`'s PICTUREs; Test query on AC1's `BETWEEN` statement shows "range" and ≤ 50 items. Visual look: the operator's, through `render_form`.

- [ ] **T26 — All targets from one result set, and the hosts for the binding** (R31, R49; AC10)
  - Files: `crates/cobolt-ide/src/form_runtime.rs` (tests), `crates/cobolt-cli/tests/partiql_hosts.rs`.
  - Do: one form, one `SELECT CUST-ID, CUST-NAME, CUST-BALANCE, BILLING.CITY FROM CUSTOMERS WHERE CUST-BALANCE > ?`, filling a DataGrid (four columns), a ListBox (display `CUST-NAME`, value `CUST-ID`), a Bar chart, a Gauge, a Maps control (`LAT` / `LNG` added for the test) and a repeating GroupBox, each with the values §5.2 gives (a Number with its decimal point); the binding forms run three ways.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` and `cargo test -p cobolt-cli --test partiql_hosts` — identical rows across hosts; the result block lists each target.

## Phase 4 — Tools

- [x] **T27 — Check validates literal statements** (R42; AC4, AC15)
  - Files: `crates/cobolt-project-tools/src/validate_source.rs` (`partiql_literal_problems`), `crates/cobolt-project-tools/src/tools/check.rs`, `crates/cobolt-project-tools/src/validate_source.rs` tests.
  - Do: tokenize the generated handler text with `cobolt_lexer`; for each `<ID>::Execute(` / `ExecuteTransaction(` whose first argument is a string literal and whose `<ID>` is an `IndexedFile` control, run `analyze` against the control's `.cidx` **without opening the data file**; every R6 error is a Diag with handler, line and position (resolved through the `SourceMap` to the developer's site); each scan is an informational note. A statement held in a data item is not checked.
  - Verify: `cargo test -p cobolt-project-tools` — AC4's statements written as literals in a handler are each reported at their line with no data file present; the AC1 `WHERE CUST-BALANCE > 1000.50` statement carries the scan note; `rcrun check` on the fixture project agrees (`cargo build -p cobolt-cli` first).
  - Done: `partiql_literal_problems` in `validate_source.rs`; one-line literals only; test `partiql_literals_are_checked_against_the_cidx`. `rcrun check` agreement on a fixture project was not run.

- [~] **T28 — Autocomplete and descriptions** (R43, R45; AC16)
  - Files: `crates/cobolt-ide/src/panels/editor.rs` (`methods_for_type` arm for `IndexedFile`, the properties), `crates/cobolt-ide/src/ide_text_data.rs`, `crates/cobolt-ide/src/ide_language_tests.rs`.
  - Do: after `IXF-CUST::` offer `Execute`, `ExecuteTransaction`, `Fetch`, `Get`, `ColumnNames`, `ColumnCount`, `ResultCount`, `StatusCode`, `LastError`, `AutoCommit` (and `BindingQuery` / `SelectedValue` on their targets) with descriptions in the six languages, and none of the visual methods (`Click`, `Show`).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` — the offered names and no `Click` or `Show`; every description has six non-empty translations.
  - Done: methods and six-language descriptions; the run-time properties (`ResultCount`, `StatusCode`, `LastError`) are not offered as property completions (they are not in the control model).

- [ ] **T29 — The debugger shows the statement** (R44; AC16)
  - Files: `crates/cobolt-runtime/src/interpreter/partiql.rs`, `crates/cobolt-runtime/src/interpreter.rs` (`debug_out` on `OutputChannel::FileIo`, as `debug_binding_io` does).
  - Do: a call is stepped over as one line; after it the File I/O channel carries a `PARTIQL` line with the statement text, the bound parameter values, the read class (keyed / range / scan), `StatusCode`, `LastError` and `ResultCount`; a bound control's refresh writes the same. Silent unless a debug session is listening (no formatting cost in a plain Run Form).
  - Verify: `cargo test -p cobolt-runtime` — with a listener attached the line has every field, without one nothing is built; the parameter value is present on the channel and absent from `LastError` (R29).

## Phase 5 — Strings, knowledge, agents, documents, finalize

- [ ] **T30 — i18n, six languages** (R45)
  - Files: `crates/cobolt-ide/src/i18n.rs`, `crates/cobolt-ide/src/ide_text_data.rs`.
  - Do: every string T24, T25, T28 added is a `Tr` field with EN ES PT JA ZH FR (Portuguese is pt-BR); Check diagnostics, `LastError` texts, exception names and statement text stay English.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` and `ide_language` — no empty or untranslated entry.

- [~] **T31 — The System KB** (R46; AC17)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the IndexedFile doc tables: the "no `::` methods" text, the property and method entries, the status table, `AutoCommit`, `BindingQuery`, `SelectedValue`, the PartiQL binding), `assets/knowledge/chunked.data` (regenerated).
  - Do: rewrite the text to the truth, then `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green; a text test that no table says "no `::` methods" or "`::ReadNext`".
  - Done so far: the three built-ins are in the KB table and `chunked.data` is rebuilt (`prebuilt_chunked_kb_matches_the_published_documentation` green). NOT done: the IndexedFile property/method tables, `AutoCommit`, `BindingQuery`, `SelectedValue`, the PartiQL binding text, and removing the 'no `::` methods' wording.
  - Done: the 'no `::` methods' wording is gone from the KB table and `chunked.data` is rebuilt; the property/method tables for `AutoCommit`, `BindingQuery`, `SelectedValue` are still to write.

- [~] **T32 — Agent prompts, skills, patterns** (R46; AC17)
  - Files: `crates/cobolt-ide/src/assets/agentic_ai/system-prompt.md`, `crates/cobolt-ide/src/assets/agentic_ai/skills/rustcobol-control-properties.md`, `…/skills/rustcobol-extensions.md`, `crates/cobolt-ide/src/llm.rs` (the four prompt sites), `crates/cobolt-project-tools/src/{reference,patterns}.rs`, the plugin skill `powerrustcobol-define-indexed-file`, the `grid-bound-to-indexed-file` pattern.
  - Do: say what this spec ships and nothing else: the members of the control, no facade `::Open` / `::ReadNext` / `::Write`, the PartiQL binding.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` and `cargo test -p cobolt-project-tools` — a grep test finds neither "no `::` methods" nor `::ReadNext` in any of those texts.
  - Done: system prompt, two skills and four `llm.rs` sites rewritten; `no_text_a_model_reads_denies_the_indexed_file_members`. NOT done: `reference.rs`, `patterns.rs`, the plugin skill `powerrustcobol-define-indexed-file`, the `grid-bound-to-indexed-file` pattern, and the seeded `agentic_ai/` folders.

- [~] **T33 — The Developer's Guide, the matrix, the translations** (R47; rules #3, #8; AC17)
  - Files: `docs/developers-guide-en.md` (§14 "PartiQL — querying an indexed file"; §8 `IndexedFile` paragraph; *Data binding and the Guardian*), `docs/cobol-support-matrix-en.md`; delete `docs/developers-guide-{es,pt,fr,jp,cn}.md` and `docs/cobol-support-matrix-{es,pt,fr,jp,cn}.md`.
  - Do: for the PowerCOBOL / isCOBOL developer, COBOL and prose only: the mapping of §5.2, the statements and functions with original examples, the read classes with a mermaid diagram of how a statement becomes `START` / `READ`, the departures of §5.4 (list indexes from 1, hyphens, no `LIMIT`/aggregates), the control's members and the status table, `AutoCommit` and transactions, the master-detail and `SelectedValue` examples, Notes, Caveats (a parameter value shows on the debug channel; a scan of a large file; `OpenMode` must be `I-O` to write), screenshot placeholders; rows for PartiQL statements, transactions and the binding (PowerRustCOBOL extension). English only; rule #8's cycle, never a translation edited.
  - Verify: the section exists with its diagram; `ls docs/ | grep -E '(developers-guide|cobol-support-matrix)-(es|pt|fr|jp|cn)'` prints nothing; `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` compared with T0 (the pre-existing red stays the same red).
  - Done so far: Guide §14 subsection (statements, mapping, read classes with a mermaid diagram, departures, members, statuses, transactions, `AutoCommit`, built-ins, binding) and three matrix rows. The five translations of both documents are already absent in this tree, so none was deleted. NOT done: the §8 `IndexedFile` paragraph and the *Data binding and the Guardian* section.

- [ ] **T34 — Finalize** (all; AC1–AC18 as listed below)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` (first 60 lines only when reading), this file's checkboxes and status.
  - Do: bump `z` (next free after 1.91.227 at commit time; each separately committed phase takes its own) and add the top entry with the absolute date; run the whole sweep (`--no-fail-fast`, every `test result:` line read) against T0; move any untracked non-Rust program file to `PowerRustCOBOL-local-settings/quarantine`, reporting each; scan every diff for keys, tokens and account ids and confirm no example `.cfrm` was opened; check MCP tools (`check`, `regenerate`, `validate`) if the IDE server is reachable and say so if not. Report to the operator: the executor's measured numbers (AC18), the nine-cell parity result, and the two follow-ups outside this feature — the Q13 fix on `fixes` and, if the operator wants it, the 2.x.x port with the WASM-PARITY row of plan §8.
  - Verify: all crates build and test as T0 (no new red); every acceptance criterion below is checked; nothing committed or pushed by the implementing agent.

## Acceptance-criteria coverage

| AC | Verified by |
|---|---|
| AC1 (R1–R3, R11–R13) | T6, T7, T10 (classes keyed / range / scan / keyed; loop equality), T27 (Check classes) |
| AC2 (R2, R4, R19–R22) | T11 |
| AC3 (R5, R14–R17, R27) | T8, T10 |
| AC4 (R6, R27–R29) | T3, T5, T8, T27 |
| AC5 (R2, §5.2, §5.4) | T5, T10 |
| AC6 (R23–R26) | T9, T10, T13 |
| AC7 (R7–R10) | T12, T13 |
| AC8 (R18, R49) | T16, T26 |
| AC9 (R30–R32, R35a, R40, R41) | T17, T19, T20, T21 |
| AC10 (R31) | T26 |
| AC11 (R34) | T20 |
| AC12 (R35, R35a) | T21 |
| AC13 (R36) | T22 (and T23 for cards) |
| AC14 (R37, R33) | T20 |
| AC15 (R38, R39, R42, R45) | T24, T25, T27, T30 |
| AC16 (R43, R44) | T11, T14, T28, T29 |
| AC17 (R46, R47) | T31, T32, T33 |
| AC18 (R48) | T10 |

## Done criteria

All acceptance criteria in `spec.md` are checked, tests pass against the T0 baseline with no new red, the Developer's Guide and the support matrix are updated and
their translations deleted, the System KB and `chunked.data` are regenerated, every string is in six languages, and the work is ready for the coordinator to split
into feature commits (never mixed with the `fixes` follow-ups). Nothing is committed or pushed by the implementing agent.

## Baseline

Recorded 2026-10-29 on `features-108-partiql` at 1.91.227 (`main` 795747a8), before any change, `cargo test --no-fail-fast`, every `test result:` line read
(run in batches of twelve integration tests with the test executables deleted between batches, because the volume had 495 MiB free):

| Crate | Test binaries | Passed | Failed | Ignored |
|---|---|---|---|---|
| `cobolt-runtime` | 181 | 1534 | 0 | 29 |
| `cobolt-ast` | 5 | 70 | 0 | 0 |
| `cobolt-forms --features render` | 91 | 1687 | 0 | 2 |
| `cobolt-codegen` | 10 | 98 | 0 | 0 |
| `cobolt-project-tools` | 9 | 93 | 0 | 1 |
| `cobolt-compiler`, `cobolt-cli`, `cobolt-ide --bin cobolt-ide` | **not baselined**: the compiler's feature set rebuilds ~700 dependencies and the run was stopped to keep the session moving; their known reds (`docs_embed::every_document_ships_in_every_language`, `claude_validates_the_written_bundle`, two `external_crates_service` flow tests) are taken from the 1.91.226 note, and any other failure at the end is checked against `main` before it is called new. |

No test was red in the five crates that were measured.

## Spike results

- **(a) What `exec_method` receives.** `prop_to_value` (`interpreter.rs`, near `is_known_method`) turns a property's text into `CobolValue::Numeric` when it parses as a number,
  so `TXT-SEARCH::Text` holding `2026` reaches the method as the Number 2026 and a `PIC 9(4)` item reaches it the same way: **the two cannot be told apart from the value**
  (leading zeros are lost too: `007` arrives as 7). D6 stands, with this seam: `lower_member_chain` is the one place member-call arguments are evaluated, so the interpreter
  notes there which arguments are a control's property and keeps their raw text (`partiql_note_origins`); the control's `Execute` takes those as untyped text (O2). Proven by
  `test_partiql_control` (`'42'` finds record 42; `'forty'` against a Number is 94; a `PIC 9` data item against a String field is 94).
- **(b) `IXF-1::CUST-NAME`.** Lexes and parses as a property read, including inside `IF IXF-CUST::CUST-BALANCE < 0` and `SET WS-V TO IXF-CUST::CUST-NAME` — `test_partiql_control`
  runs both. No parser change was needed. A property whose text is `00` reads back as the Number 0 (`prop_to_value`), so `IXF-CUST::StatusCode` displays `0`, not `00`;
  the same happens to every control's properties and is left as it is.
- **(c) `run()` and open files.** `Interpreter::run` (`interpreter.rs`, `pub fn run`) closes only the Viewer reports (`hand_over_open_reports`); the keyed engines in `open_files` are
  closed by the verbs or by the generated `<id>-CLOSE`, never by the run. For a verb-opened file that is an existing behaviour (the engine's `Drop` is not relied on); it is **not changed
  here** and is listed as a follow-up for `fixes`. The control's own files are closed by `partiql_close_owned`, called from `run()`.
- **(d) Hosts.** `cobolt_forms::assets::resolve` anchors on `assets::set_base`, which each host calls with the project root; the control resolves its `.cidx` and the data path through it, as
  the existing `IndexedFile` binding does. Not yet exercised in a built binary (T16).
- **(e) `ClickedRow` / `SelectedIndex`.** `ClickedRow` is pushed by the renderer as a property update (`prop_updates`) and lands in the interpreter's object store; `SelectedIndex` is written
  through `obj_set` and `sync_list_selection` (`interpreter.rs`), which is where a list's `SelectedValue` is kept. A seam exists for both (T21).
- **Engine finding (not this feature).** `STORAGE IS MEMORY` answers every `READ NEXT` by copying and searching its whole key list (`indexed.rs`, `read_seq`), so a sequential read of n records costs O(n²):
  100,000 records take hours. The executor test runs that engine at 3,000 records (`PARTIQL_N_MEMORY`). A memory file also opens `INPUT` only unless it has `WITH PERSISTENCE` (37 otherwise). Both go to `fixes`.
