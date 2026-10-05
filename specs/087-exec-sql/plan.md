<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Embedded SQL (`EXEC SQL`) in RustCOBOL

- **Status:** approved (operator, 2026-10-05)
- **Spec:** ./spec.md   **Date:** 2026-10-05

## 0. Where we start

Confirmed by running sample programs through the release `rcrun` and by
reading the code (2026-10-05):

- `EXEC SQL` fails five different ways today: data items after a block in
  WORKING-STORAGE are silently lost; SQL words in the PROCEDURE DIVISION are
  parsed as COBOL verbs (`OPEN`, `DELETE`, `COMMIT` …) and `ID` opens a
  phantom nested program; the COPY/REPLACE preprocessor rewrites SQL text;
  and an unterminated block is reported as an `EXEC RUST` error.
- The SQL bridge (`crates/cobolt-runtime/src/db_runtime.rs`) runs text only:
  no bound parameters (PostgreSQL uses `simple_query`), every value becomes a
  string and NULL becomes `""`, no transactions, no column types.
- Each form's interpreter has its own `DbRegistry`; nothing database-related
  is shared between forms. Only the Rust bridge is
  (`shared_rust_bridge`, interpreter.rs:15270).
- No host waits for the interpreter thread after the window closes
  (`host.rs:389–401`), so a commit at the end of a run could lose a race with
  process exit.
- No operating-system secure store is in use: the IDE's own is disabled
  (`crates/cobolt-ide/src/secrets/mod.rs:46`) and model API keys live in the
  IDE credential vault, session-only by default (`model_config_store.rs`).
- `rcrun package` ships source and `rcrun`, never a built binary.

## 1. Approach

```
.cbl / .cfrm
  → preprocessor: COPY + EXEC SQL INCLUDE; SQL bodies protected from REPLACE
  → lexer: Token::ExecSqlBlock(Box<SqlBlock>)
  → parser: Stmt::ExecSql(Box<ExecSql>) + Program.sql_cursors
  → semantic: exec_sql.rs (R46)
  → interpreter/exec_sql.rs → esql::SqlRunUnit (shared by every form of the run)
  → esql::backend::{sqlite, postgres, mysql}: typed values, cursors, units of work
  catalog of SQL connections: project file | sql-connections.toml | environment
                              | application key store | IDE-injected
```

`db_runtime.rs` keeps its behaviour for the `COBOL::"…-DB"` built-ins and the
`SqlDatabase` control. Its three connect helpers move to a shared
`db_connect.rs`; everything typed is new code under `esql/`.

### 1.1 Front end (R1–R5, R14, R16, R21, R31, R32)

- **SQL scanner** — `crates/cobolt-lexer/src/sql.rs`, pure and shared by the
  lexer, the preprocessor, the parser, the runtime (placeholder rewriting) and
  the IDE (highlighting). It knows `'…'` strings with `''`, `"…"` and backtick
  identifiers, `$tag$…$tag$`, `--` and `*>` line comments (stripped before
  sending, R4), `/* … */` comments (kept, so optimizer hints survive), `?`
  parameters and `::` casts. `block_end` finds `END-EXEC` outside all of them
  (R5).
- **Host variables** — a `:` not preceded by `:` and not followed by `:` or
  `=`, then a COBOL word that contains a letter; a trailing hyphen is not part
  of it, and a hyphen elsewhere in the SQL is the SQL operator (R16). `OF`
  qualifies anywhere; `IN` qualifies only outside parentheses (inside them it
  is SQL, as in `POSITION(:A IN col)`). `:A.B` is tried as A OF B, then as
  B OF A; if both resolve, Check reports it as ambiguous. Indicators:
  `:H:I` or `:H INDICATOR :I` (R9).
- **Lexer** — `try_capture_exec_sql` next to `try_capture_exec_rust`
  (`lexer.rs`, call site 303, model 472–540): skips newlines after `EXEC`,
  requires `SQL`, refuses when the previous token is `::` (an `obj::Exec`
  method call), keeps the body verbatim with the origin line of each of its
  lines (`SqlBlock{text, lines, first_col, end_line}`), re-lexes a raw token
  that straddles the block end, and turns a missing `END-EXEC` into one
  specific error. `tokenize_expansion` (`copybook.rs:38`) remaps the block's
  lines through the expansion map.
- **Preprocessor** — `expand_text_mapped` (`copybook.rs:326–430`) gains an
  `EXEC SQL` branch beside the EXEC RUST guard (345–372), so neither REPLACE
  nor a `COPY` word inside SQL ever touches an SQL body. `INCLUDE SQLCA` and
  `INCLUDE SQLDA` insert this project's own text (§3); any other
  `INCLUDE name` is loaded like `COPY name` through `load_and_expand`, with
  the same search path (R31). A missing copybook leaves blank lines in place
  and an error that carries its line (AC12). `has_directives` (116) also
  detects `EXEC SQL INCLUDE`.
- **AST** — `crates/cobolt-ast/src/sql.rs`. Because the AST is
  bincode-serialized by position, `Stmt::ExecSql(Box<ExecSql>)` is appended
  after `Throw`, and `Program.sql_cursors` after the last field.
  `ExecSql{kind, whenever, owner (PROGRAM-ID), span, last_line}`;
  `SqlKind` = Declarative | Execute(SqlText) | SelectInto{text, into} | Open |
  Fetch | Close | Commit | Rollback | Connect{target, alias, user, password} |
  SetConnection | Disconnect(Named|Current|All) | Prepare | ExecutePrepared |
  ExecuteImmediate | Describe{input}; `SqlText` = parts (Text | Input(n) |
  CurrentOf(cursor)) + its host-variable inputs.
- **Parser** — `crates/cobolt-parser/src/sql.rs`.
  - `parse_stmt` routes `ExecSqlBlock` and does **not** eat the period, so
    the existing statement-list logic gives R3 (a period ends the sentence;
    inside `IF`/`PERFORM` the scope stops at it). `parse_exec_rust` eats it —
    that is a separate fix, never copied here.
  - The top-level `INTO :host …` of a SELECT is cut out wherever it stands,
    which covers the samples' `… FROM … INTO … WHERE …` and `… WHERE … INTO …`.
  - Diagnostics owned by the parser: subscript or reference modification on a
    host variable, a figurative constant (R14), a block in the wrong place
    (R2), a `CONNECT … USING` with a literal password.
  - DATA DIVISION: `parse_data_declarations` (`data.rs:403`) accepts SQL
    blocks inline, so items after a declare section are no longer skipped;
    `BEGIN/END DECLARE SECTION` and `DECLARE … TABLE` are transparent (R7,
    R32); FILE SECTION rejects blocks.
  - `WHENEVER` is tracked in source order and copied onto every executable
    statement that follows (R21).
  - Each cursor records its owning program. A cursor declared in a DATA
    DIVISION is also visible to the programs that program contains (R28, as
    amended) — a form's handlers see the form's cursors. A post-pass marks the
    cursors that some `CURRENT OF` names.

### 1.2 Runtime (R6–R45, R51)

- **Modules** — `crates/cobolt-runtime/src/esql/` with `mod` (run unit,
  `test_connection`), `value` (`SqlValue{Null, Int(i64),
  Decimal{mantissa:i128, scale:u8}, Float, Text, Bytes, Bool}`), `state`
  (SQLSTATE normalization, SQLCODE rule), `catalog`, `session`, `rewrite`
  (placeholders, identity columns, `CURRENT OF`), `sqlda`, and
  `backend/{sqlite, postgres, mysql, unlinked}` behind the `sql` feature;
  `unlinked` answers `08001` with the existing "SQL not linked" explanation.
  Interpreter glue in `interpreter/exec_sql.rs` (the `interpreter/kb.rs`
  pattern). Backends implement one trait: `execute`, `query`, `prepare`,
  `describe`, `open_cursor`, `fetch`, `close_cursor`, `begin`, `commit`,
  `rollback`, plus a per-statement guard (PostgreSQL savepoints).
- **Executing one statement** — evaluate the inputs (no lock held); lock the
  run unit only for the driver call; convert every output first and assign
  them all only if every conversion succeeded (R11, R24); set the status
  items; write the debugger's SQL line; then apply `WHENEVER` by returning the
  interpreter's existing `RuntimeError::GoTo` (interpreter.rs:5525). An SQL
  failure is never a `RuntimeError` (R22).
- **Binding and conversion** (R6–R15)
  - A host item is classified from what the value store already knows:
    numeric capacity (`integer_capacity`, `decimal_places`), alphanumeric
    capacity, an edit template, a float value, a group. A new
    `is_unsigned_numeric` accessor exposes the unsigned flag. USAGE is not
    needed, because COMP, COMP-3 and DISPLAY all store the same value.
  - Host structures expand through a new `host_structure_leaves`, a sibling
    of `flat_leaf_spans` that skips FILLER, `REDEFINES` and 88s and refuses
    OCCURS (R8); indicator tables are addressed through `subscript_key`.
  - Input: numeric → `Int` (scale 0, fits i64) or `Decimal`; alphanumeric →
    text with trailing spaces removed (documented; required for equality on
    SQLite and PostgreSQL text columns); negative indicator → NULL (R12);
    every value bound, never spliced (R13, R51).
  - Output: numbers are truncated to the item's scale; an integer part that
    does not fit, or a negative value for an unsigned item, gives `22003` and
    assigns nothing (checked before `set`, which would silently cut digits);
    text longer than the item gives `01004` and the original length in the
    indicator; NULL sets the indicator to −1, or gives `22002` without one;
    floating values go through their shortest exact decimal; dates and times
    arrive as ISO text; a wrong number of targets gives `07002`; text that is
    not a number gives `22018`.
  - Per backend: SQLite binds decimals of up to 15 significant digits as REAL
    and longer ones as TEXT; PostgreSQL sends every parameter in text format
    (a custom `ToSql` whose `encode_format` is `Text`, so the server parses
    NUMERIC and dates itself) and decodes results from binary with our own
    NUMERIC, DATE, TIME and TIMESTAMP decoders; MySQL sends decimals as exact
    text.
- **Status** (R17–R19) — the stand-alone `SQLSTATE`, `SQLCODE` and `SQLMSG`
  are found like any other name, so the samples' GLOBAL items in the outer
  program are found from the handlers; SQLCA fields are found as
  `X OF SQLCA`; every declared form is set. SQLSTATE is the backend's own on
  PostgreSQL, MySQL's state refined by its error code, and SQLite's mapped
  from its extended result codes, plus the codes the runtime raises itself
  (02000, 01004, 01005, 21000, 22002, 22003, 22018, 24000, 07002, 07003,
  08001–08003, 0A000). SQLCODE is derived from SQLSTATE by a published rule:
  0 for success, +100 for no data, the five digits as a positive number for a
  warning (`01004` → +1004), as a negative number for an all-digit error
  (`23505` → −23505), −class×1000 when the subclass has letters (`42P01` →
  −42000), otherwise −99000. An INSERT, UPDATE or DELETE that affects no row
  reports no data.
- **Units of work** (R22, R29, R30, R36, R38) — a unit of work starts with the
  first statement after a connect or the end of the previous one; on SQLite
  only before a statement that writes, so a reading program never holds the
  database lock. `EXEC SQL COMMIT/ROLLBACK` end it; the COBOL verbs keep
  governing INDEXED files only. PostgreSQL aborts a whole transaction after
  any error, so each statement runs under a savepoint (one combined round
  trip) and an error rolls back to it — the statement fails, the unit of work
  survives, as R22 requires. MySQL's implicit commit after DDL reopens the
  unit of work; SQLite's own rollbacks are detected with `is_autocommit()`.
  COMMIT closes cursors without `WITH HOLD`; ROLLBACK closes every cursor
  (R30 as amended).
- **Cursors** (R25–R28) — keyed by (interpreter instance, owner program,
  name), so two open copies of a form never collide; a wrong-state operation
  gives `24000` and changes nothing. SQLite and MySQL read the rows at OPEN
  (SQLite statements cannot be shared across threads; MySQL must drain a
  result before the loop's UPDATEs). PostgreSQL uses a native
  `DECLARE … CURSOR [WITH HOLD]` and fetches in batches. `WHERE CURRENT OF`
  works on single-table cursors — SQLite through an added `rowid`, PostgreSQL
  natively, MySQL through the table's primary key read from
  `information_schema` — and gives `0A000` otherwise.
- **Dynamic SQL** (R42–R45) — PREPARE keeps a snapshot of the text (R45);
  `?` becomes `$n` for PostgreSQL only. PostgreSQL and MySQL keep their
  prepared-statement objects; SQLite keeps the text and uses its statement
  cache. `DECLARE c CURSOR FOR s` is resolved when the cursor opens.
- **SQLDA** (R41, R44 as amended) — each entry carries a pointer to the item
  and one to the indicator that receive its value, set with
  `SET SQLDA-DATA(i) TO ADDRESS OF item` and resolved through the
  environment's address table (`addr_target`); when the data pointer is NULL
  the value arrives as text inside the entry. A descriptor with too few
  entries gets `NEEDED` set, nothing filled, and SQLSTATE `01005`.
- **Run unit and SQL connections** (R33–R39)
  - One `SqlRunUnit` per run, held in an `Arc`: connections, the current
    connection, prepared statements and cursors. The interpreter that starts
    the run is its **root**; every child form joins it as a member through the
    hosts' existing `child_interpreter_setup` seam, so no `FormHostConfig`
    field is added (`rcrun run-form` passes it instead of `None`; the compiled
    binary composes it with its EXEC RUST registration;
    `build_form_instance_as`, host.rs:4998, already applies it).
  - Only the root ends the run unit: commit and close after `STOP RUN`,
    `GOBACK` or the end of the program; roll back and close after an error, a
    cancel or an IDE Stop (R38 as amended). A member ending releases only its
    own cursors and statements. `cobolt_form_host::run` and
    `shell::run_shell` wait up to 10 s for the root thread after the window
    closes, which also fixes the exit race in `rcrun` and in built binaries.
  - With no current connection, statements use the default SQL connection,
    or give `08003` (R35). `CONNECT TO name` tries the project's SQL
    connections first, then only clearly-shaped connection strings, otherwise
    `08001` (R34 as amended).
  - The `SqlConnection` record (name, backend, file path or host/port/
    database, default, create-if-missing) lives in
    `crates/cobolt-forms/src/connections.rs` beside `RestConnection`, the
    crate every host already depends on, with
    `sql_env_var(app, name, URL|USER|PASSWORD)`.
  - Where definitions come from: the project file's `[[sql-connections]]`
    under the IDE and `rcrun` (published by `rcrun run-form` and `rcrun run`);
    `sql-connections.toml` beside the built binary; the IDE's in-process runner
    hands the catalog over directly. Precedence: `<APP>_SQL_<NAME>_URL`,
    `_USER` and `_PASSWORD` first; in a built binary, the application key
    store entry `SQL:<NAME>` (`key_store.rs`) for the password. A `password`
    in the file fails with `28000` and a message naming the file and the two
    permitted sources.
  - Credentials never travel inside a URL: `postgres::Config` and
    `mysql::OptsBuilder` are built field by field, a `Secret` type prints
    `***`, the PostgreSQL parameter type's `Debug` prints `<param>`,
    `mysql::Opts` is never formatted, and SQLite's `expanded_sql()` (which
    splices values) is never called.
- **`SqlDatabase.SqlConnection`** (R40) — seeded empty; when set, codegen
  writes `'sql-connection:<NAME>'` instead of the connection string, and both
  `OPEN` without an argument and `COBOL-OPEN-DB` resolve that reserved prefix
  through the catalog. The control's connection stays its own.

### 1.3 Check (R46)

A new pass, `crates/cobolt-semantic/src/exec_sql.rs`, run from
`analyze_with`, walks the unit and carries GLOBAL declarations down the nest.
It builds its own index of declarations (name, ancestors, PICTURE digits and
sign, usage, OCCURS, FILLER, line) and reports undeclared or ambiguous host
variables, invalid indicators, cursors used before they are declared or
declared twice, EXECUTE or DESCRIBE before PREPARE (a `DECLARE … CURSOR FOR`
is not a use), and a `WHENEVER` target that does not exist. The lexer and
preprocessor report unterminated blocks and missing copybooks; the parser
reports placement and R14. The resolver skips `ExecSql`, so nothing is
reported twice. `rcrun check`, the coding-agent `check` tool and the IDE's
Check already share this path. An unknown `SqlConnection` name on a form is a
Check error through `validate_form_source`, which learns the project's SQL
connection names. Diagnostics stay in English (R49 as amended).

### 1.4 Build (R39)

`runtime_features::scan_program` links the `sql` feature for any program with
SQL blocks. The compiler's own copy of the project file reads
`[[sql-connections]]`. After it installs the binary in `bin/` and in the
destination folder, Build writes a starting `sql-connections.toml` beside each
copy only if none is there (the `copy_missing` pattern): SQLite paths made
absolute, no user, no password, and a header that names the environment
variables and the key-store entry. The generated `main` installs the
deployment catalog before any interpreter starts, in both the form and the
console entry points. Nothing about connections is baked into the binary.

### 1.5 IDE (R33, R47–R49)

- **SQL Connections** — `Category::SqlConnections` right after Indexed Files
  (`Category::TOP` becomes eight items); rows from the project's
  `sql_connections`, the default marked; `[+]` adds one; a row opens the
  connection editor in the main pane (the indexed-file inspector pattern,
  `open_indexed_inspect`, app.rs:7300). The editor
  (`panels/sql_connections.rs`) holds name, backend, path or
  host/port/database, user, password (masked), default and create-if-missing;
  Save, Remove (with confirmation) and **Test connection** on a worker thread.
  Names must stay unique after the environment-variable normalization.
- **Credentials** — the IDE credential vault that holds model API keys, under
  its policy (R33 as amended), in slots `sql::<APP>::<NAME>::user` and
  `…::password`; a rename moves them (`rekey_credential`) and Check reports
  forms that still name the old one. Run Form receives them as the
  `<APP>_SQL_<NAME>_*` environment variables (`credential_env_for`,
  form_runtime.rs:337).
- **Editor** — the line highlighter carries the scanner's state across lines
  and colours SQL keywords, strings, comments and host variables with the
  theme's existing colours. **Go to definition** (F12, Cmd/Ctrl-click) for a
  host variable reaches its data item; form handlers go through the spec-053
  source map (R47).
- **Debugger** — a new `Sql` output channel and dock tab show, after each
  block, the statement with placeholders, the bound values (a CONNECT password
  as `******`), SQLSTATE, SQLCODE, rows and message (R48). Breakpoints inside a
  block body are refused, as for EXEC RUST.
- **Designer** — an `SqlConnection` drop-down of the project's names, with a
  stale-name warning (the IndexedFile and RestClient precedents).
- **Strings** — new `Tr` fields in all six tables and the property help in
  six languages (R49).

### 1.6 Documentation, System KB, agent reference (R50)

A new Guide chapter beside §15 "SQL databases", written for PowerCOBOL and
isCOBOL developers with original examples: the type table, SQLSTATE
normalization and the SQLCODE rule, the SQLCA and SQLDA layouts, SQL
connections in the tree, the deployment file, the environment variables and
the key-store entry, `SqlConnection`, the dynamic-SQL caveat (R51), and the
known limits. The Guide's Appendix A row for "Embedded SQL / ODBC" and its
"seven categories" text change too. `cobol85-supported-syntax-en.md` gains an
extensions entry; `database-runtime-en.md` documents the `sql-connection:`
prefix and its five translations are deleted (GOLDEN RULE #8). The System KB
text and property rows change with each behavioural milestone and
`chunked.data` is regenerated. The coding-agent reference gains an embedded
SQL section and a skill (modelled on define-indexed-file) that asks the
developer to create SQL connections in the tree — agents cannot write them.

## 2. Affected crates / files

| Crate | Files |
|---|---|
| `cobolt-lexer` | `src/sql.rs` (new), `src/lexer.rs`, `src/token.rs`, `src/copybook.rs` |
| `cobolt-ast` | `src/sql.rs` (new), `src/stmt.rs` (variant, `child_stmts`, `span`), `src/program.rs` |
| `cobolt-parser` | `src/sql.rs` (new), `src/stmt.rs`, `src/data.rs`, `src/parser.rs` |
| `cobolt-semantic` | `src/exec_sql.rs` (new), `src/lib.rs`, `src/resolver.rs` |
| `cobolt-runtime` | `src/esql/**` (new), `src/interpreter/exec_sql.rs` (new), `src/interpreter.rs`, `src/environment.rs`, `src/db_runtime.rs` → `src/db_connect.rs`, `src/debugger.rs`, `Cargo.toml` (rusqlite `column_decltype`) |
| `cobolt-dap` | `src/types.rs` (output channel mirror) |
| `cobolt-forms` | `src/connections.rs`, `src/model.rs` |
| `cobolt-codegen` | `src/lib.rs` |
| `cobolt-form-host` | `src/host.rs`, `src/shell.rs` |
| `cobolt-cli` | `src/form_gui.rs`, `src/main.rs` |
| `cobolt-compiler` | `src/lib.rs`, `src/runtime_features.rs` |
| `cobolt-project-tools` | `src/validate_source.rs`, `src/content.rs` |
| `cobolt-ide` | `src/project_model.rs`, `src/panels/project.rs`, `src/panels/sql_connections.rs` (new), `src/app.rs`, `src/form_runtime.rs`, `src/runner.rs`, `src/panels/editor.rs`, `src/panels/debugger.rs`, `src/exec_rust_run.rs`, `src/panels/properties.rs`, `src/prop_help_data.rs`, `src/i18n.rs` |
| docs | `docs/developers-guide-en.md`, `docs/cobol85-supported-syntax-en.md`, `docs/database-runtime-en.md` (+ delete its 5 translations) |
| KB | `assets/knowledge/chunked.data` |
| tests | `crates/cobolt-runtime/tests/test_esql_*.rs`, `tests/cobol/esql/*.cbl`, plus unit tests in each crate |

## 3. Data / model changes

- **AST** — `Stmt::ExecSql` and `Program.sql_cursors`, both appended last
  (bincode). Old serialized programs never contain them.
- **Token** — `Token::ExecSqlBlock(Box<SqlBlock>)`.
- **Project file** — optional table, written only when non-empty, so the
  new-project golden stays byte-identical:

  ```toml
  [[sql-connections]]
  name = "SALES"
  backend = "sqlite"          # sqlite | postgresql | mysql
  path = "data/sales.db"      # sqlite; host / port / database for the others
  default = true
  ```
- **Deployment file** beside a built binary, written by Build when absent:

  ```toml
  # Never put a password here. Use MYAPP_SQL_SALES_PASSWORD, or the
  # application's key store entry SQL:SALES. MYAPP_SQL_SALES_URL and
  # MYAPP_SQL_SALES_USER override the values below.
  [[connection]]
  name = "SALES"
  backend = "postgresql"
  host = "db.example.local"
  port = 5432
  database = "sales"
  default = true
  ```
- **SQLCA** (this project's own layout) — `SQLCA-TAG "RCSQLCA1"`,
  `SQLCODE S9(9) COMP-5`, `SQLSTATE X(5)`, `SQLCA-ROWS S9(18) COMP-5`,
  `SQLCA-MESSAGE-LENGTH`, `SQLCA-MESSAGE X(512)`, `SQLCA-NATIVE-CODE`,
  `SQLCA-WARNING`, `SQLCA-TRUNCATED`, `SQLCA-CONNECTION X(64)`.
- **SQLDA** (this project's own layout) — `SQLDA-TAG "RCSQLDA1"`,
  `SQLDA-CAPACITY` (VALUE 100), `SQLDA-NEEDED`, `SQLDA-COUNT`, and
  `SQLDA-ENTRY OCCURS 100` with `SQLDA-NAME`, `SQLDA-TYPE`,
  `SQLDA-TYPE-NAME`, `SQLDA-LENGTH`, `SQLDA-PRECISION`, `SQLDA-SCALE`,
  `SQLDA-NULLABLE`, `SQLDA-DATA` and `SQLDA-IND-PTR` (`USAGE POINTER`),
  `SQLDA-IND` and `SQLDA-VALUE X(1024)`. A program that needs more entries
  declares its own copy with a larger OCCURS and the same tag.
- **Form controls** — `SqlDatabase.SqlConnection` (string, default empty).
  Old forms load unchanged.
- **IDE** — `Category::SqlConnections`; vault slots `sql::<APP>::<NAME>::…`.
- **Debugger protocol** — `OutputChannel::Sql`, appended, mirrored in
  `cobolt-dap`.
- **Reserved name** — the connection-string prefix `sql-connection:`.

## 4. Key decisions & alternatives

- **A new typed `esql` layer; `db_runtime.rs` untouched.** Why: the built-ins
  and `SqlDatabase` depend on the text API. Rejected: retrofitting types into
  the text API.
- **PostgreSQL: text-format parameters, our own binary result decoders.** Why:
  `simple_query` cannot bind, and tokio-postgres always asks for binary
  results (verified, `query.rs:324`); text parameters let the server parse
  NUMERIC and dates (`encode_format`, postgres-types 0.2.13 `lib.rs:926`).
  Rejected: new driver features or a decimal crate for one conversion.
- **Savepoint per statement on PostgreSQL.** Why: otherwise one failed
  statement poisons the unit of work, against R22. Rejected: leaving it to
  each program.
- **Rows read at OPEN on SQLite and MySQL; native cursors on PostgreSQL.**
  Why: SQLite statements borrow their connection and are not `Send`
  (verified, rusqlite 0.32 `statement.rs:547`), and MySQL must drain a result
  before the next command. Rejected: self-referential streaming.
- **The run unit travels through `child_interpreter_setup`.** Why: it is
  applied to every child already. Rejected: a new `FormHostConfig` field
  (about 40 test literals).
- **SQL connections resolved at run time, nothing baked in.** Why: R39 — the
  same build against different databases.
- **SQLDA by pointer with a text fallback.** Why: the runtime's address table
  lets an entry name the receiving item (R41); the fallback serves generic
  screens. Rejected: values inside the descriptor only (operator, Q12).

## 5. Risks & mitigations

- **PostgreSQL savepoints cost a round trip per statement** → combined into
  one round trip with the previous release; buffered fetches need none;
  measured in the gated suite.
- **`$n` parameters inside `DECLARE … CURSOR` on PostgreSQL are unproven** →
  tested first in M7; fallback: read rows at OPEN as on SQLite.
- **PostgreSQL types we cannot decode** (arrays, ranges, geometry) → `0A000`
  with a message saying to cast the column in the SELECT.
- **Behaviour that surprises a migrating developer** — trailing spaces
  trimmed on input, SQLite decimal precision, cursor rows held in memory on
  SQLite/MySQL, locks held until COMMIT, MySQL's implicit commit after DDL →
  each documented in the Guide; the unit of work starts lazily.
- **Credential leaks** → no URLs, a `Secret` type, redacted parameter
  `Debug`, `mysql::Opts` never formatted; the Guide warns that the key store
  under `rcrun` sits inside the project folder and that `settings/` belongs in
  `.gitignore`.
- **A root that never finishes** → the host's wait is bounded (10 s); past it
  the server or the SQLite journal rolls back, which is the safe direction.
- **`USAGE POINTER` inside an OCCURS entry** has not been exercised by the
  runtime's tests yet → covered by AC11 in M4c before the SQLDA is
  documented.
- **The operator's sample programs are a client's code** → the AC15 test
  reads them from the operator's machine through an environment variable
  (`PRC_LEGACY_CBL_DIR`) and is reported as skipped without it; no sample
  source is copied into the repository.
- **Lexer, parser and preprocessor changes reach every program** → after
  M1–M3, the full NIST census and every finished module's execution pass
  (NC, SQ, IX, RL, IC) must hold their baselines.

## 6. Test strategy

Every milestone is its own commit on the feature branch with its own `z`
bump and CHANGELOG entry. Every new COBOL test program ends with a GOLDEN
RULE #7 result block: the statement forms exercised, row counts, and elapsed
time and throughput per phase. Sweeps run with `--no-fail-fast` and every
`test result:` line is read.

| # | Scope | Tests (what they assert) | R / AC |
|---|---|---|---|
| M1 | scanner, token, preprocessor | `END-EXEC` inside strings, identifiers, comments and dollar quotes does not end a block; `::` and `arr[1:2]` are not host variables; fixed and free format give identical tokens; INCLUDE of SQLCA and of a copybook; a missing INCLUDE reports its line; REPLACE leaves SQL alone | R1, R4, R5, R31; AC1 |
| M2 | AST, parser, data division, WHENEVER | every statement form parses; INTO after FROM or WHERE; a period after `END-EXEC` ends the sentence, inside `IF` too; WHENEVER attached in source order; each wrong placement is one error | R2, R3, R6, R7, R9, R14, R16, R21, R25, R28, R32, R42–R44; AC1, AC6 |
| M3 | Check | one file with every R46 error gives exactly one diagnostic each, on its line; the migrated samples pass Check (gated by `PRC_LEGACY_CBL_DIR`) | R46; AC12, AC15 |
| M4a | SQLite: statements, conversion, status, SQLCA, WHENEVER | AC2 host structure and binding-not-splicing; AC3 NULL and truncation; AC4 round trips and `22003`; AC5 both status styles give the same pair; AC6 source-order WHENEVER and continuing after an error | R6–R13, R15, R17–R24, R34–R36, R51; AC2–AC6 |
| M4b | cursors, units of work | AC7 10,000-row cursor with rows/s, positioned update of every third row, `24000`; AC8 COMMIT/ROLLBACK against INDEXED files and WITH HOLD | R25–R30; AC7, AC8 |
| M4c | dynamic SQL, SQLDA | AC11 descriptor too small, then fetch through pointers and inline; AC18 mixed forms | R41–R45; AC11, AC18 |
| M5 | run unit, catalog, hosts | a child form reads through the main form's connection; commit at a normal end, rollback after an error; catalog precedence and the password-in-file message; no password in any output | R33–R39, R52; AC9, AC10, AC14 |
| M6 | build | the feature scan sees SQL; the starting file has no credentials and is never overwritten; a built binary reads the file, then the `_URL` override (gated build test) | R39; AC10, AC14 |
| M7 | PostgreSQL, MySQL | NUMERIC and date/time binary decoding, redacted parameter `Debug`, MySQL error refinement, `?` versus `$n`; the AC2–AC8, AC11 and AC18 programs rerun against live servers through `PRC_TEST_PG_URL` / `PRC_TEST_MYSQL_URL`, reported as skipped without them | R13, R15, R18 |
| M8 | `SqlConnection` | the property wins over `ConnectionString` in Run Form and the binary; the `.cfrm` holds only the name; an unknown name is a Check error; property documented | R40; AC17 |
| M9 | IDE | tree rows after Indexed Files with the default marked and `[+]`; Test connection succeeds on SQLite and reports the driver's error on a closed PostgreSQL port; highlighting across lines; go to definition; the SQL debugger channel masks the password; i18n tables | R33, R47–R49; AC9, AC13, AC16 |
| M10 | documentation | the System KB freshness test; a search of the repository and of all outputs for passwords | R50; AC16 |
| M11 | migration acceptance | F-ART-PURGA and TyC from the operator's machine: Check, SQLite paths, and the MySQL-gated full run | AC15 |

Also: AC14 parity — the same programs under `rcrun run`, Run Form, an
embedded child form and the compiled binary (`interpreter-binary-parity`).
The IDE pieces are checked by the operator's own eye; the agent never drives
the application.

## 7. Steering compliance

- [ ] i18n: every new IDE string in six languages; compiler diagnostics stay
      English (R49 as amended)
- [ ] Generated-code banner and regenerate-on-action contract preserved;
      codegen emits no SQL beyond the `sql-connection:` target
- [ ] English Guide updated; `database-runtime` translations deleted
      (GOLDEN RULE #8)
- [ ] System KB updated and `chunked.data` regenerated in each behavioural
      milestone
- [ ] All three hosts wired (`interpreter-binary-parity`)
- [ ] Fix vs feature: **feature** → feature branch, `z` bump and CHANGELOG
      entry per milestone; f=96 only when the operator asks
- [ ] No "cobolt" in user-facing text; COBOL identifiers English

## 8. Found in passing — separate fixes, not part of this feature

- `EXEC RUST` swallows the period after `END-EXEC` (`parse_exec_rust`,
  `stmt.rs:3634`), so a following statement joins an enclosing `IF`.
- `COBOL-FETCH-ROW` into a group or a subscripted item writes to the wrong
  place, and an empty result sets no status.
- SQLite "is this a query?" is decided by the first word, which misses
  `VALUES`, `EXPLAIN` and SQL that starts with a comment.
- Stale text: the System KB documents `DB-1-COMMIT`/`ROLLBACK` paragraphs that
  are never generated (`compiler lib.rs:7015`); the runtime's Cargo.toml says
  "rustls" though neither driver has TLS; `Driver` still offers `mssql`; the
  Guide calls `dist/` "reserved"; `rcrun package`'s `run.sh` calls
  `$DIR/cobolt`.
