<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Embedded SQL (`EXEC SQL`) in RustCOBOL

- **Status:** done except T27/T28's live-server runs (2026-10-06) — see T39
- **Plan:** ./plan.md   **Date:** 2026-10-05

Ordered, small, independently verifiable. Each task names its files, the
requirements it satisfies and how it is verified. Milestones (M1–M11) follow
`plan.md` §6; each milestone ends with a wrap-up task (`z` bump, CHANGELOG
entry, and — for behaviour a developer can observe — the Guide and System KB
slice, per GOLDEN RULE #3 and the steering KB rule). Nothing is committed or
pushed unless the operator asks.

**Working rules for every task**

- Branch: `features-spec-087-exec-sql` (it holds the spec and plan). It is not
  pushed yet, so `git rebase main` is safe until its first push; after that,
  sync only with `git merge --ff-only main`.
- Disk: `df -h /` before any sweep; run tests with `CARGO_INCREMENTAL=0`.
- Sweeps: `--no-fail-fast`, and read every `test result:` line. Known
  expected failure: `docs_embed::tests::every_document_ships_in_every_language`.
- Every new COBOL test program prints a GOLDEN RULE #7 result block: the
  statement forms exercised, row counts, elapsed time and throughput per phase.
- The operator's PowerCOBOL samples are a client's code: tests read them from
  `PRC_LEGACY_CBL_DIR` and print `SKIPPED` without it. Never copy them into the
  repository.
- Live servers: `PRC_TEST_PG_URL` / `PRC_TEST_MYSQL_URL`; without them the
  live tests report `SKIPPED`.

## M0 — Baseline

- [x] **T0 — Record the baseline** (all)
  - Files: none in the repository (results kept in the session scratchpad).
  - Do: build `rcrun` (`cargo build --release -p cobolt-cli`), then record
    `cargo run -p cobolt-semantic --example nist_conformance -- strict` (per
    module compile scores) and `… -- run NC`, `SQ`, `IX`, `RL`, `IC`
    (programs and assertions passed); record the `test result:` totals of
    `cobolt-lexer`, `cobolt-ast`, `cobolt-parser`, `cobolt-semantic`,
    `cobolt-runtime`, `cobolt-form-host`, `cobolt-compiler`, `cobolt-cli`,
    `cobolt-project-tools`, `cobolt-codegen`, `cobolt-forms --features render`
    and `cobolt-ide --bin cobolt-ide`.
  - Verify: the numbers are written down before T1 starts; every later
    regression check compares against them.

## M1 — Scanner, token, preprocessor

- [x] **T1 — SQL scanner** (R1, R4, R5, R6, R9, R16)
  - Files: `crates/cobolt-lexer/src/sql.rs` (new), `crates/cobolt-lexer/src/lib.rs`.
  - Do: `scan` (strings with `''`, `"…"`/backtick identifiers, `$tag$…$tag$`,
    `--` and `*>` line comments, `/* */`, `?`, `::`), `block_end`, host
    variables (qualification with `OF`, `IN` outside parentheses, `:A.B` in
    both orders; indicators `:H:I` and `INDICATOR`; hyphen rule), and the
    cross-line `LineState`.
  - Verify: `cargo test -p cobolt-lexer sql::` — `END-EXEC` inside each kind
    of string, identifier, comment and dollar quote does not end a block;
    `x::int`, `arr[1:2]` and `:=` are not host variables; `QTY-1` is SQL
    while `:WS-QTY-LESS-ONE` is one name; each qualification and indicator
    form; a string and a block comment that span lines keep their state.

- [x] **T2 — `EXEC SQL` lexer token** (R1, R5)
  - Files: `crates/cobolt-lexer/src/token.rs`, `crates/cobolt-lexer/src/lexer.rs`.
  - Do: `Token::ExecSqlBlock(Box<SqlBlock>)` (verbatim text, origin line per
    line, first column, end line); `try_capture_exec_sql` beside
    `try_capture_exec_rust`, refusing after `::`; re-lex a raw token that
    straddles the block end; one specific error for an unterminated block.
  - Verify: `cargo test -p cobolt-lexer` — the same program in fixed and free
    format gives identical block text and origin lines; `EXEC SQL` inside an
    EXEC RUST body, a literal or a comment is not captured; `obj::Exec` is
    unchanged; an apostrophe inside a `--` comment does not disturb the COBOL
    that follows; a missing `END-EXEC` gives one error on the `EXEC` line;
    every existing lexer test still passes.

- [x] **T3 — Preprocessor: SQL bodies protected, `INCLUDE`** (R4, R5, R17, R31, R41)
  - Files: `crates/cobolt-lexer/src/copybook.rs`; consumers of copy errors in
    `crates/cobolt-project-tools/src/validate_source.rs` and
    `crates/cobolt-ide/src/app.rs` (`do_check`).
  - Do: an `EXEC SQL` branch beside the EXEC RUST guard so REPLACE and COPY
    never touch SQL bodies; `INCLUDE SQLCA` / `INCLUDE SQLDA` insert this
    project's own layouts (plan §3); `INCLUDE name` through `load_and_expand`
    with COPY's search path; a missing copybook leaves blank lines and a lined
    diagnostic; `has_directives` detects `EXEC SQL INCLUDE`;
    `tokenize_expansion` remaps the block's origin lines; `CopyExpansion`
    gains lined diagnostics (additive field).
  - Verify: `cargo test -p cobolt-lexer copybook` — REPLACE leaves an SQL
    body alone; INCLUDE of SQLCA and of a copybook expands, and a diagnostic
    after the INCLUDE still names its original line; a missing INCLUDE
    reports its line; `cargo test -p cobolt-project-tools` and
    `cargo build -p cobolt-ide` green.

- [x] **T4 — M1 wrap-up**
  - Do: `z` bump and CHANGELOG entry; NIST compile census.
  - Verify: `cargo build --release -p cobolt-cli`, then
    `cargo run -p cobolt-semantic --example nist_conformance -- strict` equals
    the T0 baseline module by module; lexer sweep green.

## M2 — AST, parser, data division, WHENEVER

- [x] **T5 — AST types** (R2, R21, R25, R28)
  - Files: `crates/cobolt-ast/src/sql.rs` (new), `crates/cobolt-ast/src/stmt.rs`
    (`Stmt::ExecSql` appended after `Throw`; `child_stmts`, `span`),
    `crates/cobolt-ast/src/program.rs` (`sql_cursors` appended last); the
    exhaustive matches in `cobolt-parser`, `cobolt-semantic` (resolver skips
    `ExecSql`) and `cobolt-runtime` (dispatch arm that does nothing until T14).
  - Verify: `cargo build --workspace`; `cargo test -p cobolt-ast` — a program
    holding every `SqlKind` and a cursor declaration round-trips through
    bincode unchanged.

- [x] **T6 — Statement parser** (R3, R6, R7, R9, R14, R16, R23, R25, R26, R29, R32, R34, R36, R42–R44)
  - Files: `crates/cobolt-parser/src/sql.rs` (new), `crates/cobolt-parser/src/stmt.rs`.
  - Do: route `ExecSqlBlock` in `parse_stmt` without eating the period;
    classify every statement form; cut the top-level `INTO :host …` wherever
    it stands; build `SqlText` (text, inputs, `CURRENT OF`); diagnostics for
    subscripts, reference modification and figurative words (R14) and for a
    literal password in `CONNECT … USING`.
  - Verify: `cargo test -p cobolt-parser --test test_exec_sql` — every
    statement form parses to the expected kind; `INTO` after `FROM` and after
    `WHERE` (the samples' shapes); a period after `END-EXEC` ends the
    sentence and a block inside `IF` without a period stays in the `IF` (AC1);
    each R14 error is reported once on its line; every existing parser test
    passes.

- [x] **T7 — Data division, placement, WHENEVER, cursor ownership** (R2, R7, R21, R25, R28, R32)
  - Files: `crates/cobolt-parser/src/data.rs`, `crates/cobolt-parser/src/parser.rs`,
    `crates/cobolt-parser/src/sql.rs`.
  - Do: SQL blocks inline in WORKING-STORAGE, LOCAL-STORAGE and LINKAGE
    (declare sections transparent, `DECLARE … TABLE` ignored, `DECLARE …
    CURSOR` recorded with its owner); FILE SECTION and other places rejected;
    `WHENEVER` copied in source order onto later statements; DATA DIVISION
    cursors visible to contained programs (R28); the post-pass that marks
    positioned cursors.
  - Verify: `cargo test -p cobolt-parser --test test_exec_sql` — the items
    after a declare section are declared (the case that is silently lost
    today); each wrong placement is one error; `WHENEVER` placed after a
    statement in the source does not apply to it (AC6, parse side); a form
    handler resolves the form's cursor; cursors named by `CURRENT OF` are
    marked.

- [x] **T8 — M2 wrap-up**
  - Do: `z` bump and CHANGELOG entry.
  - Verify: NIST compile census equals T0; `cobolt-parser`, `cobolt-semantic`
    and `cobolt-runtime` sweeps equal their T0 totals plus the new tests.

## M3 — Check

- [x] **T9 — Semantic pass** (R14, R21, R28, R46)
  - Files: `crates/cobolt-semantic/src/exec_sql.rs` (new),
    `crates/cobolt-semantic/src/lib.rs`, `crates/cobolt-semantic/src/resolver.rs`.
  - Do: an index of declarations (ancestors, PICTURE digits and sign, usage,
    OCCURS, FILLER, line) carried down nested programs with GLOBAL items;
    report undeclared and ambiguous host variables, invalid indicators,
    cursors used before they are declared or declared twice, EXECUTE or
    DESCRIBE before PREPARE (a `DECLARE … CURSOR FOR s` is not a use), and a
    missing `WHENEVER` target.
  - Verify: `cargo test -p cobolt-project-tools --test exec_sql_check` (new,
    through `validate_text`, the path the agent `check` tool and the IDE
    share) — one file holding every R46 error (the unterminated block last)
    gives exactly one diagnostic per error, each on its line, with no database
    reachable (AC12); `rcrun check` on the same file prints the same lines
    (`cargo test -p cobolt-cli`).

- [x] **T10 — Check on the operator's samples** (R46; AC15 Check part)
  - Files: `crates/cobolt-project-tools/tests/exec_sql_check.rs` (gated case).
  - Do: run Check on `$PRC_LEGACY_CBL_DIR/M-ARTICULOS/Debug/F-ART-PURGA.cob`
    and `$PRC_LEGACY_CBL_DIR/TyC/Debug/TyC.cob`.
  - Verify: no diagnostic comes from an `EXEC SQL` block; `SKIPPED` without
    the variable. **If non-SQL PowerCOBOL constructs in these listings fail
    Check, stop and report**: AC15's "pass Check" would then depend on
    converting them.
  - Result (2026-10-06): 0 diagnostics inside the 40 SQL blocks
    (F-ART-PURGA 35, TyC 5); the programs still fail Check on non-SQL
    PowerCOBOL constructs (`#FILE`/`#LINE`, `POW-…`, `CALL … WITH STDCALL`,
    `BY VALUE`) — reported to the operator, who narrowed AC15 to the SQL
    (spec §7 Q14, 2026-10-06).

- [x] **T11 — M3 wrap-up**
  - Do: `z` bump, CHANGELOG entry, Guide (the Check section of the new
    chapter) and System KB slice; regenerate `chunked.data`.
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb`, then
    `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb` green;
    NIST compile census equals T0.

## M4a — SQLite: static statements, conversion, status

- [x] **T12 — Shared connect helpers** (no behaviour change)
  - Files: `crates/cobolt-runtime/src/db_connect.rs` (new),
    `crates/cobolt-runtime/src/db_runtime.rs`, `crates/cobolt-runtime/Cargo.toml`
    (rusqlite `column_decltype`).
  - Verify: `cargo test -p cobolt-runtime db_runtime`, `--test test_sql`,
    `--test test_methods` green — `classify_routes_by_scheme` and
    `sqlite_end_to_end_crud` unchanged.

- [x] **T13 — Core types and the SQLite backend** (R13, R18, R19, R23, R24, R29, R34, R36)
  - Files: `crates/cobolt-runtime/src/esql/{mod.rs, value.rs, state.rs, session.rs, rewrite.rs}`,
    `crates/cobolt-runtime/src/esql/backend/{mod.rs, sqlite.rs, unlinked.rs}`.
  - Do: `SqlValue`, `SqlError`, the SQLSTATE normalization table and the
    SQLCODE rule; SQLite execute and query with bound parameters, typed
    values, lazy unit of work (only before a writing statement), COMMIT and
    ROLLBACK, `prepare_cached`; the R34 rule for connection strings (a bare
    unknown name gives `08001`).
  - Verify: `cargo test -p cobolt-runtime esql::` — the SQLCODE rule's
    examples (`00000`→0, `02000`→+100, `01004`→+1004, `23505`→−23505,
    `42P01`→−42000); real SQLite errors map to `23505`, `42601`, `42P01`; a
    value is bound, never spliced; NULL survives; decimals up to 15 digits
    bind as REAL and longer ones as TEXT; a reading program holds no lock
    (a second connection can write); COMMIT and ROLLBACK.

- [x] **T14 — Executing static statements from COBOL** (R6–R13, R15, R17–R24, R34–R36, R48 runtime side, R51)
  - Files: `crates/cobolt-runtime/src/interpreter/exec_sql.rs` (new),
    `crates/cobolt-runtime/src/interpreter.rs`, `crates/cobolt-runtime/src/environment.rs`
    (`host_structure_leaves`, `is_unsigned_numeric`, `top_level_item`),
    `crates/cobolt-runtime/src/debugger.rs` (`OutputChannel::Sql`),
    `crates/cobolt-dap/src/types.rs` (mirror), `tests/cobol/esql/*.cbl`,
    `crates/cobolt-runtime/tests/test_esql_static.rs` (new).
  - Do: build inputs (host structures, indicators, trimmed PIC X); run under
    the session lock only; convert every output and assign all or none; set
    stand-alone items and SQLCA; apply `WHENEVER` through `GoTo`; `CONNECT`,
    `SET CONNECTION`, `DISCONNECT` with connection strings; the SQL debugger
    line (password masked).
  - Verify: `cargo test -p cobolt-runtime --test test_esql_static`:
    - AC2: a host structure of five items (one under FILLER, one REDEFINES)
      fills the four named ones in order; the row `x' OR '1'='1` matches only
      itself;
    - AC3: NULL with an indicator (−1, item unchanged), without one (`22002`);
      indicator −1 stores NULL; 30 characters into `PIC X(10)` → indicator 30
      and `01004`;
    - AC4: round trips of `S9(7)V99 COMP-3`, `S9(9) COMP-5`, `9(5)`,
      `X(40)` and an ISO date, negatives and maxima; one more → `22003`, item
      unchanged;
    - AC5: the same failure sets the stand-alone items and the SQLCA to the
      same SQLSTATE, SQLCODE and message; success, no data, `01004`, `23505`
      and `42601` give the documented pairs;
    - AC6: source-order `WHENEVER`; with none in force, an error continues;
    - the SQL debugger line masks a `CONNECT … USING` password.

- [x] **T15 — M4a wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (delimiters, host variables,
    indicators, the type table, status items, SQLSTATE and SQLCODE,
    `WHENEVER`, the SQLCA layout, `CONNECT` by connection string) and System
    KB slice; regenerate `chunked.data`.
  - Verify: KB freshness test green; `cargo test -p cobolt-runtime` sweep
    equals T0 plus the new tests.

## M4b — Cursors and units of work

- [x] **T16 — Cursors on SQLite** (R25–R30)
  - Files: `crates/cobolt-runtime/src/esql/{session.rs, rewrite.rs, backend/sqlite.rs}`,
    `crates/cobolt-runtime/src/interpreter/exec_sql.rs`, `tests/cobol/esql/*.cbl`,
    `crates/cobolt-runtime/tests/test_esql_cursors.rs` (new).
  - Do: OPEN reads the rows; keys (interpreter instance, owner, name); `24000`
    on wrong states; COMMIT closes cursors without `WITH HOLD`, ROLLBACK closes
    all; `WHERE CURRENT OF` through an added `rowid`; `0A000` for a
    multi-table positioned cursor.
  - Verify: `cargo test -p cobolt-runtime --test test_esql_cursors`:
    - AC7: 10,000 rows fetched to the end, the 10,001st fetch gives `02000`,
      rows per second in the result block; updating every third row
      `WHERE CURRENT OF` changes exactly those rows; fetching a closed cursor
      gives `24000`;
    - AC8: INSERT + ROLLBACK leaves no row, INSERT + COMMIT leaves it; the
      COBOL `ROLLBACK` verb does not undo the committed row; `EXEC SQL
      ROLLBACK` does not undo an INDEXED-file write; a `WITH HOLD` cursor
      survives COMMIT, another does not, and ROLLBACK closes both.

- [x] **T17 — M4b wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (cursors, positioned updates, units
    of work) and System KB slice; regenerate `chunked.data`.
  - Verify: KB freshness test green.

## M4c — Dynamic SQL and the SQLDA

- [x] **T18 — Dynamic SQL and the SQLDA** (R41–R45)
  - Files: `crates/cobolt-runtime/src/esql/{sqlda.rs, session.rs, backend/sqlite.rs}`,
    `crates/cobolt-runtime/src/interpreter/exec_sql.rs`, `tests/cobol/esql/*.cbl`,
    `crates/cobolt-runtime/tests/test_esql_dynamic.rs` (new).
  - Do: PREPARE (text snapshot), EXECUTE [USING], EXECUTE IMMEDIATE,
    `DECLARE c CURSOR FOR s`, OPEN USING, FETCH USING DESCRIPTOR, DESCRIBE
    [INPUT]; SQLDA through each entry's pointers (`addr_target`) with the text
    fallback; NEEDED and `01005`; `07003`.
  - Verify: first a smoke test that `USAGE POINTER` inside an OCCURS entry
    can be set with `SET … TO ADDRESS OF` and read back; then
    `cargo test -p cobolt-runtime --test test_esql_dynamic` — AC11: a
    too-small descriptor sets NEEDED and `01005`; a large enough one is
    filled; rows fetched once through pointers into the program's items and
    once inline; names, types and values match the table; EXECUTE IMMEDIATE
    creates a table and EXECUTE … USING inserts into it.

- [x] **T19 — Mixed forms** (R6, R16, R31, R32, R34, R36, R44, R45)
  - Files: `tests/cobol/esql/*.cbl`, `crates/cobolt-runtime/tests/test_esql_mixed.rs` (new).
  - Verify: `cargo test -p cobolt-runtime --test test_esql_mixed` — AC18 in
    one program: `OF` and period qualification of two same-named items;
    `SELECT QTY-1 INTO :WS-QTY-LESS-ONE`; INCLUDE of a project copybook and a
    `DECLARE … TABLE`; `CONNECT … AS SECOND`, `SET CONNECTION` between two
    files, `DISCONNECT ALL`; a known `USING` password found in no output,
    trace or debug event; `DESCRIBE INPUT` reports the parameter count; a
    statement re-executed after its source item changed runs as prepared.

- [x] **T20 — M4c wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (dynamic SQL, the SQLDA layout and
    its pointer and inline modes, the R51 caveat) and System KB slice;
    regenerate `chunked.data`.
  - Verify: KB freshness test green.

## M5 — Run unit, SQL connections, hosts

- [x] **T21 — The SQL connection record and the project file** (R33)
  - Files: `crates/cobolt-forms/src/connections.rs`, `crates/cobolt-ide/src/project_model.rs`,
    `crates/cobolt-compiler/src/lib.rs` (its own copy of the project file).
  - Do: the `SqlConnection` record and `sql_env_var`; the
    `[[sql-connections]]` table, written only when non-empty.
  - Verify: `cargo test -p cobolt-forms --features render connections` (round
    trip; environment-variable names); `cargo test -p cobolt-ide --bin
    cobolt-ide the_headless_new_project_manifest_is_the_ides` (golden
    unchanged).

- [x] **T22 — The catalog** (R34, R35, R39 runtime part, R52)
  - Files: `crates/cobolt-runtime/src/esql/catalog.rs`, `crates/cobolt-runtime/src/esql/mod.rs`.
  - Do: published project definitions, the deployment-file finder, an
    injected catalog; precedence of `_URL`, `_USER`, `_PASSWORD`; the
    key-store entry `SQL:<NAME>` (binary only); `28000` for a password in the
    file; a `Secret` type; the default connection; `test_connection`.
  - Verify: `cargo test -p cobolt-runtime esql::catalog` — each precedence
    case; the password-in-file message names the file and both sources;
    `Secret` prints `***` in `Debug` and `Display`; a default is used when no
    connection is current, `08003` without one.

- [x] **T23 — One run unit for every form; hosts** (R33, R35, R37, R38, R39)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (roles, end reason,
    `set_sql_catalog`), `crates/cobolt-runtime/src/esql/mod.rs`,
    `crates/cobolt-cli/src/form_gui.rs`, `crates/cobolt-cli/src/main.rs`,
    `crates/cobolt-compiler/src/lib.rs` (`run_form_app`, `run_headless`),
    `crates/cobolt-form-host/src/host.rs`, `crates/cobolt-form-host/src/shell.rs`,
    `crates/cobolt-ide/src/runner.rs`.
  - Do: an `Arc` run unit created by the root and joined by children through
    `child_interpreter_setup`; members release their own cursors and
    statements; the root commits or rolls back by end reason (an IDE Stop is
    a cancel); the hosts wait up to 10 s for the root thread; `rcrun run` and
    `rcrun run-form` publish the project's SQL connections; the IDE runner
    injects the catalog.
  - Verify:
    - `cargo test -p cobolt-form-host`: a child form reads through the
      connection the main form opened;
    - `cargo test -p cobolt-runtime`: commit at a normal end, rollback after
      an error;
    - `cargo test -p cobolt-cli`: `rcrun run` in a project with an SQL
      connection `SALES` runs `CONNECT TO 'SALES'` (AC9, runtime part).

- [x] **T24 — M5 wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (SQL connections, the default
    connection, the run unit and its end) and System KB slice; regenerate
    `chunked.data`; `interpreter-binary-parity` checklist for the three hosts.
  - Verify: KB freshness test green; `cargo test -p cobolt-runtime
    -p cobolt-form-host -p cobolt-compiler -p cobolt-cli` green.

## M6 — Build

- [x] **T25 — Build links SQL and writes the deployment file** (R39)
  - Files: `crates/cobolt-compiler/src/runtime_features.rs`,
    `crates/cobolt-compiler/src/lib.rs`.
  - Do: the feature scan sees `ExecSql` and `sql_cursors`; Build writes
    `sql-connections.toml` beside the binary in `bin/` and in the destination
    folder only when absent (absolute SQLite paths, no user or password, a
    header naming the variables and the key-store entry); the generated
    `main` installs the deployment catalog in both entry points.
  - Verify: `cargo test -p cobolt-compiler` — `scan_marks_exec_sql`;
    `starting_toml_no_credentials_never_overwritten`; the gated build test
    (AC10, AC14 binary part): the built binary reads the file, then the
    `<APP>_SQL_SALES_URL` override; a `password` key fails with the
    documented message; uncommitted work is committed at a normal end and
    rolled back after an error.

- [x] **T26 — M6 wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (the deployment file, the variables,
    the key-store entry and its limits) and System KB slice; regenerate
    `chunked.data`.
  - Verify: KB freshness test green.

## M7 — PostgreSQL and MySQL

- [ ] **T27 — PostgreSQL backend** (R13, R15, R18, R22, R25–R30, R41, R44)
  - Files: `crates/cobolt-runtime/src/esql/backend/postgres.rs` (new),
    `crates/cobolt-runtime/tests/test_esql_live.rs` (new, gated).
  - Do: text-format parameters with a redacted `Debug`; binary decoders for
    NUMERIC, DATE, TIME, TIMESTAMP(TZ), bool, integers, floats, text and
    bytea; metadata from `params()` and `columns()`; native SQLSTATE;
    per-statement savepoints; native cursors with batch fetch; native
    `CURRENT OF`; `postgres::Config` built field by field.
  - Verify: `cargo test -p cobolt-runtime esql::backend::postgres` —
    NUMERIC decoding (signs, NaN, negative weight, scale), date and time
    decoding, redacted parameter `Debug`, `?` → `$n`; then
    `PRC_TEST_PG_URL=… cargo test -p cobolt-runtime --test test_esql_live -- --ignored pg`
    reruns the AC2–AC8, AC11 and AC18 programs. **First check `$n` inside
    `DECLARE … CURSOR`; if PostgreSQL refuses it, switch to reading rows at
    OPEN and report it.**
  - Status (2026-10-06): **code complete, live run pending.** No server is
    on this machine and installing one needs the operator's permission.
    Done: `backend/postgres.rs` (text parameters with a redacted `Debug`,
    binary decoders, savepoint per statement, native SQLSTATE, field-by-field
    `Config`, `ctid` row key); unit tests `numeric_decoding`,
    `date_and_time_decoding`, `redacted_parameter_debug`; CONNECT to a closed
    port → `08001` with the driver's reason. **Deviation, reported:** rows are
    read at OPEN on PostgreSQL too, not through a native cursor — the
    `$n`-in-`DECLARE` question cannot be settled without a server, and reading
    at OPEN is the same path SQLite and MySQL take. Native cursors with batch
    fetch remain to do for very large results. To finish: set
    `PRC_TEST_PG_URL` and run `cargo test -p cobolt-runtime --test
    test_esql_live -- --ignored pg`.

- [ ] **T28 — MySQL backend** (R13, R15, R18, R25–R30, R44)
  - Files: `crates/cobolt-runtime/src/esql/backend/mysql.rs` (new),
    `crates/cobolt-runtime/tests/test_esql_live.rs`.
  - Do: positional parameters, value mapping, column flags, the error
    refinement table, `CLIENT_FOUND_ROWS`, the reopened unit of work after
    DDL, the primary-key lookup for `CURRENT OF`, `OptsBuilder` built field by
    field (`Opts` never formatted).
  - Verify: `cargo test -p cobolt-runtime esql::backend::mysql`
    (`mysql_error_refinement`); `PRC_TEST_MYSQL_URL=… cargo test
    -p cobolt-runtime --test test_esql_live -- --ignored mysql` reruns the same
    programs.
  - Status (2026-10-06): **code complete, live run pending** (as T27).
    Done: `backend/mysql.rs` (positional parameters, value mapping by column
    type and charset, `CLIENT_FOUND_ROWS`, the error refinement table, the
    unit reopened after DDL, primary-key `CURRENT OF` through
    `information_schema`, `OptsBuilder` field by field); unit tests
    `mysql_error_refinement`, `mysql_values`. To finish: set
    `PRC_TEST_MYSQL_URL` and run the `mysql` case.

- [x] **T29 — M7 wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (per-database notes and limits) and
    System KB slice; regenerate `chunked.data`.
  - Verify: KB freshness test green.

## M8 — `SqlDatabase.SqlConnection`

- [x] **T30 — The property end to end** (R40, R49)
  - Files: `crates/cobolt-forms/src/model.rs`, `crates/cobolt-codegen/src/lib.rs`,
    `crates/cobolt-runtime/src/interpreter.rs` (`OPEN`, `COBOL-OPEN-DB`),
    `crates/cobolt-runtime/src/db_runtime.rs` (`open_resolved`),
    `crates/cobolt-project-tools/src/validate_source.rs`,
    `crates/cobolt-compiler/src/lib.rs` (property row),
    `crates/cobolt-ide/src/prop_help_data.rs`, `crates/cobolt-ide/src/panels/properties.rs`.
  - Do: seed the property; codegen writes `'sql-connection:<NAME>'`; both
    open paths resolve the prefix through the catalog; Check reports an
    unknown name; the property row in the System KB; help in six languages;
    the designer drop-down with a stale-name warning.
  - Verify: `cargo test -p cobolt-codegen` (the generated connect string);
    `cargo test -p cobolt-runtime sqlconnection_wins` (it wins over
    `ConnectionString`); `cargo test -p cobolt-form-host` (Run Form path);
    the gated build test (binary path); `cfrm_holds_name_only`;
    `cargo test -p cobolt-project-tools unknown_sql_connection`;
    `cargo test -p cobolt-compiler every_control_property_is_documented`;
    `cargo test -p cobolt-ide --bin cobolt-ide every_control_property_is_explained_in_six_languages`
    (AC17).

- [x] **T31 — M8 wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (`SqlConnection`) and System KB
    slice; regenerate `chunked.data`.
  - Verify: KB freshness test green.

## M9 — IDE

- [x] **T32 — SQL Connections in the project tree** (R33)
  - Files: `crates/cobolt-ide/src/project_model.rs` (`Category::SqlConnections`,
    `TOP` of eight, every category match), `crates/cobolt-ide/src/panels/project.rs`,
    `crates/cobolt-ide/src/i18n.rs` (×6).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide sql_connections_category_rows`
    — the item comes right after Indexed Files, lists the project's
    connections with the default marked, and its `[+]` emits the new-connection
    event; `indexed_category_tree_order` and the External Crates tree tests
    updated and green.

- [x] **T33 — The connection editor, credentials, Test connection** (R33, R49, R52)
  - Files: `crates/cobolt-ide/src/panels/sql_connections.rs` (new),
    `crates/cobolt-ide/src/app.rs` (main-pane branch, events),
    `crates/cobolt-ide/src/llm.rs` (vault slots), `crates/cobolt-ide/src/form_runtime.rs`
    (`credential_env_for`), `crates/cobolt-ide/src/i18n.rs` (×6).
  - Do: fields, Save, Remove with confirmation, Test connection on a worker
    thread; unique names after normalization; user and password in the vault
    slots `sql::<APP>::<NAME>::…`, moved on rename; Check reports `SqlDatabase`
    controls still naming the old name; Run Form gets the
    `<APP>_SQL_<NAME>_*` variables.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` —
    `test_connection_sqlite_ok_pg_closed_port_driver_error` (the function
    behind the button), `rename_moves_credentials`, the i18n tests; after a
    save, a search of the project folder finds no password (AC9, AC16).

- [x] **T34 — Highlighting and go to definition** (R47)
  - Files: `crates/cobolt-ide/src/panels/editor.rs`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide sql_highlight_crosses_lines`
    (keywords, strings, comments and host variables coloured across lines)
    and `host_var_goto_definition` (F12 and Cmd/Ctrl-click reach the data
    item, also from a form handler); `goto_tests` green.

- [x] **T35 — The SQL debugger tab and breakpoints** (R48)
  - Files: `crates/cobolt-ide/src/panels/debugger.rs` (`DockTab::Sql`),
    `crates/cobolt-ide/src/exec_rust_run.rs` (block ranges include SQL).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` —
    `debug_sql_channel_masks_password` (text with placeholders, bound values,
    SQLSTATE, SQLCODE, rows; the password as `******`) and
    `breakpoint_refused_inside_sql_block` (AC13).

- [x] **T36 — M9 wrap-up**
  - Do: `z` bump, CHANGELOG entry; Guide (the SQL Connections item, the
    editor, Test connection, highlighting, go to definition, the SQL debugger
    tab, with `📷 Screenshot needed` placeholders; the "seven categories" text
    becomes eight) and System KB slice; regenerate `chunked.data`.
  - Verify: KB freshness test green; `cargo test -p cobolt-ide --bin cobolt-ide`
    sweep equals T0 plus the new tests (the known expected failure only).

## M10 — Documentation and the agent reference

- [x] **T37 — Final documentation pass** (R50, R51)
  - Files: `docs/developers-guide-en.md` (chapter complete; Appendix A
    "Embedded SQL / ODBC" row), `docs/cobol85-supported-syntax-en.md`
    (extensions entry), `docs/database-runtime-en.md` (the `sql-connection:`
    prefix; delete its five translations), `crates/cobolt-project-tools/src/content.rs`
    (an embedded-SQL section and a skill that asks the developer to create SQL
    connections in the tree), `assets/knowledge/chunked.data`.
  - Verify: `cargo test -p cobolt-project-tools` (reference and skill tests);
    KB freshness test green; `docs_embed` tests unchanged apart from the known
    failure; a search of the repository and of every test output for the test
    passwords finds none (AC16).

## M11 — Migration acceptance

- [x] **T38 — The operator's samples' SQL** (AC15, narrowed by Q14)
  - Files: `crates/cobolt-runtime/tests/test_esql_live.rs` (gated cases).
  - Verify: with `PRC_LEGACY_CBL_DIR` set, the samples' `EXEC SQL` blocks,
    read from the files at test time (never copied into the repository), run
    in a test program declaring their host variables, against a SQLite copy of
    the tables they use; `DELETE … LIMIT 1` sets its syntax-error SQLSTATE
    while the program continues; with `PRC_TEST_MYSQL_URL` too, the same
    statements run against MySQL. `SKIPPED` without the variables.
  - Result (2026-10-06): F-ART-PURGA's 24 distinct statements and TyC's 3
    run as written against SQLite; `DELETE … LIMIT 1` → `42601`, the program
    continues, nothing deleted. The MySQL half waits on M7 and a server.

## Finalize

- [x] **T39 — Full sweep and acceptance**
  - Do: `cargo test --no-fail-fast` on `cobolt-lexer`, `cobolt-ast`,
    `cobolt-parser`, `cobolt-semantic`, `cobolt-runtime`, `cobolt-dap`,
    `cobolt-forms --features render`, `cobolt-codegen`, `cobolt-form-host`,
    `cobolt-compiler`, `cobolt-cli`, `cobolt-project-tools` and
    `cobolt-ide --bin cobolt-ide`; NIST compile census and the execution
    passes for NC, SQ, IX, RL and IC; AC14 parity (the AC2, AC7 and AC9
    programs under `rcrun run`, Run Form, an embedded child form and the
    compiled binary); tick every AC in `spec.md`; final `z` bump and
    CHANGELOG entry; set this file's status to done.
  - Verify: every `test result:` line read — totals equal T0 plus the new
    tests, with only the known expected failure; NIST equals T0 on every
    module; every AC1–AC18 ticked with the task that proved it.
  - Result (2026-10-06): every crate green — lexer 172, ast 32, parser 184,
    semantic 98, runtime 1152 (11 ign), dap 37, form-host 186, compiler 157
    (1 ign; the three old "environmental" failures fixed at 1.80.210), cli
    23, project-tools 62 (1 ign), codegen 76, forms (render) 1365 (1 ign),
    IDE 1338 with the one known failure (`every_document_ships_in_every_language`
    — translations deleted under GOLDEN RULE #8, regenerated at the next
    minor). NIST identical to T0: strict 420/420, NC
    4614, SQ 624, IX 574, RL 354, IC 309 assertions, 0 failures. AC14 proved
    on all four hosts with the AC2, AC7 and the new AC9 program. 17 of 18 ACs
    ticked; AC15's MySQL half and T27/T28 wait for a live server. Found and
    fixed on the way: Run Form lost a program's last DISPLAY lines (1.80.217).
  - Manual (operator, never driven by the agent): open a project, add an SQL
    connection in the tree, Test connection, write a handler with `EXEC SQL`,
    see the highlighting and go to definition, run it, and look at the SQL
    tab while debugging.

## Acceptance criteria → tasks

| AC | Verified by |
|---|---|
| AC1 | T1, T2, T6 |
| AC2–AC5 | T14 (SQLite), T27–T28 (servers) |
| AC6 | T7 (parse), T14 (run) |
| AC7, AC8 | T16, T27–T28 |
| AC9 | T23, T32, T33 |
| AC10 | T25 |
| AC11 | T18 |
| AC12 | T9 |
| AC13 | T34, T35 |
| AC14 | T23, T25, T39 |
| AC15 | T10, T38 |
| AC16 | T33, T37 |
| AC17 | T30 |
| AC18 | T19 |

## Done criteria

All acceptance criteria in `spec.md` are checked, tests pass, the Guide and
System KB are current, and the work is split into feature commits per the
operator's rules (no commit or push unless the operator asks).
