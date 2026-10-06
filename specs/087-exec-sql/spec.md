<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Embedded SQL (`EXEC SQL`) in RustCOBOL

- **Status:** approved (operator, 2026-10-05; Q1–Q7 settled in §7);
  amended the same day (§7 Q8–Q13: SQL connections in the project tree,
  credentials, packaging, and the design review)
- **Folder:** specs/087-exec-sql/
- **Author:** Claude (for Emerson Lopes)   **Date:** 2026-10-05

## 1. Overview

COBOL programs that work with relational databases write their SQL inside
the program, between `EXEC SQL` and `END-EXEC`, and exchange values with
the database through COBOL data items called *host variables*. That is how
most business COBOL reaches a database, and it is what the PowerCOBOL
applications that spec 086 converts are full of: across the 32 compiled
`.cob` listings of the operator's sample application (a few of them the same
program in two build folders) there are 99 `SELECT … INTO`, 34 cursors
(declare, open, fetch, close), 40 `COMMIT`, 28 declare sections, 29
`INSERT`/`UPDATE`/`DELETE` and 13 dynamic statements (`PREPARE`).

RustCOBOL reaches SQL today only through built-in calls
(`COBOL::"OPEN-DB"`, `"EXEC-SQL"`, `"FETCH-ROW"` …), with every value
handled as text. A program written with embedded SQL does not compile, so
spec 086 keeps each `EXEC SQL` block as a marked comment (086 R27, Q1) and
the database logic of a converted application must be redone by hand.

This feature adds embedded SQL to RustCOBOL: the statement delimiters, host
and indicator variables, declare sections, the two ways a program learns how
a statement went (stand-alone `SQLSTATE` / `SQLCODE` / `SQLMSG` items, or an
`SQLCA`, with `WHENEVER`), static statements, cursors, transactions, the
**SQL connections** a project defines and a program connects to by name, and
dynamic SQL including `DESCRIBE` and an SQL descriptor area. The SQL text
itself goes to the connected database unchanged, so a program speaks the
dialect of the database it uses: SQLite, PostgreSQL or MySQL, the three the
runtime already supports.

The concepts follow the public description of COBOL programs that issue SQL
statements in the IBM Db2 for z/OS 12 documentation (delimiters, placement,
host variables and structures, indicator variables, declare sections,
SQLCA / SQLSTATE / SQLCODE, `WHENEVER`, cursors, `INCLUDE`, dynamic SQL and
descriptors, data-type correspondence, error handling). That page is a map of
the subject only: no IBM example, sample program, copybook or SQLDA layout is
used here, and every wording, layout and example in this spec and in the
work that follows is original.

## 2. Goals / Non-goals

**Goals**

- A program with embedded SQL compiles, runs and builds in every place a
  RustCOBOL program does: `rcrun run`, Run Form, embedded child forms and the
  compiled binary — the same behaviour in all of them.
- Programs migrated from PowerCOBOL (spec 086) keep their SQL as live code:
  the status items they declare, the `SQLSTATE` values they test and the name
  they connect to (an SQL connection of the project) keep working without
  edits.
- Programs written in the SQLCA style (`INCLUDE SQLCA`, `WHENEVER`) work too.
- Host variables reach the database as **bound parameters**, never by
  splicing their values into the SQL text.
- Typed values and NULL survive the round trip: numbers keep their scale,
  NULL is distinguishable from an empty value through indicator variables.
- A database is named in the program by one of the project's **SQL
  connections**, listed in the IDE's project tree; where it is and how to log
  in to it live outside the source and outside the repository.
- The IDE's Check, the editor and the debugger understand `EXEC SQL` blocks.

**Non-goals**

- Translating SQL between dialects. The text is the developer's and goes to
  the database as written (§7 Q4, settled).
- Checking SQL text against a live database at compile time (a "bind" step),
  or validating table and column names. Check validates the COBOL side only.
- New database backends. SQLite, PostgreSQL and MySQL only; another backend
  is its own spec.
- Stored procedures (`CALL procedure`), `ALLOCATE CURSOR`, result-set
  locators, LOB locators, row-set (multi-row) `FETCH`, scrollable cursors and
  `GET DIAGNOSTICS`. Listed for a later spec if a program needs them.
- Distributed units of work across two databases in one transaction.
- National (`PIC N`) host variables. RustCOBOL has no national category
  today; it is its own work before it can reach SQL.
- Changing spec 086. Once this lands, a follow-up to 086 stops commenting
  `EXEC SQL` out; that change is 086's, not this spec's.
- Replacing the `COBOL::"…-DB"` built-ins or the `SqlDatabase` control. They
  keep working unchanged; the control only gains a way to name an SQL
  connection instead of carrying a connection string (R40).

## 3. User stories

- As a developer migrating a PowerCOBOL application, I want its `EXEC SQL`
  blocks to compile and run as they are, so that the converted forms work
  against my database without rewriting the data layer.
- As a COBOL developer used to embedded SQL, I want to write `SELECT … INTO
  :host-variable` and cursors in a form handler, so that I use the idiom I
  know instead of a call-based API.
- As a developer, I want to name an SQL connection in my program, see the
  project's SQL connections in the project tree, and configure where each one
  points per machine, so that the same program runs against a test database
  on my machine and the production database on the operator's, without
  credentials in the source.
- As a developer, I want NULL columns, decimal amounts and dates to arrive in
  my COBOL items correctly, so that I do not lose data in the conversion to
  text.
- As a developer, I want to build a query at run time and read its result
  columns even when I do not know them in advance, so that report and
  search screens can be generic.
- As a developer, I want Check to tell me about an undeclared host variable
  or cursor before I run, so that errors surface while I type.
- As a developer using the `SqlDatabase` control, I want it to name an SQL
  connection instead of holding a connection string with a password in it, so
  that no credential sits in my form files or my repository.

## 4. Requirements (EARS)

### 4.1 Delimiters and placement

- **R1 (ubiquitous):** The system shall recognise an embedded SQL statement
  as the words `EXEC SQL`, followed by the statement, followed by `END-EXEC`,
  in both fixed and free source format. In fixed format the block shall lie in
  area B and may span any number of lines; an SQL statement continues across
  lines without a continuation indicator.
- **R2 (ubiquitous):** The system shall accept embedded SQL in the
  WORKING-STORAGE, LOCAL-STORAGE and LINKAGE sections (declare sections,
  `INCLUDE`, `DECLARE CURSOR`, `DECLARE TABLE`) and in the PROCEDURE DIVISION
  (every executable statement and `DECLARE CURSOR`), and shall report an
  error for a block anywhere else.
- **R3 (ubiquitous):** A period after `END-EXEC` shall end the sentence, as it
  does after any other statement; a block without it shall not end the
  sentence. An `EXEC SQL` block shall be a single statement wherever a
  statement may appear: inside `IF`, `EVALUATE`, `PERFORM … END-PERFORM` and
  every other scope.
- **R4 (ubiquitous):** Comments shall be allowed inside a block: COBOL comment
  lines in fixed format, `*>` comments in both formats, and SQL `--` comments
  to the end of the line. They shall not be sent to the database.
- **R5 (ubiquitous):** The words `EXEC SQL … END-EXEC` inside an
  `EXEC RUST … END-EXEC` block, a literal or a comment shall not start an SQL
  block, and an `END-EXEC` inside an SQL string literal shall not end one.

### 4.2 Host variables, host structures and indicators

- **R6 (ubiquitous):** Within a block, a COBOL data item shall be referenced
  as a host variable by a colon followed by its name (`:WS-NAME`). A name that
  is not unique may be qualified with `OF`/`IN` or with a period
  (`:CITY.CUSTOMER`).
- **R7 (ubiquitous):** Any data item the program declares may be a host
  variable; declaring it inside `BEGIN DECLARE SECTION` / `END DECLARE
  SECTION` shall be accepted and shall not be required. Items in a declare
  section shall remain ordinary data items for the rest of the program.
- **R8 (ubiquitous):** A group item used as a host variable shall be a *host
  structure*: it shall stand for its elementary subordinate items in order,
  as if each had been written separately. Subordinate `FILLER`, condition-names
  (88) and `REDEFINES` shall be skipped.
- **R9 (ubiquitous):** An indicator variable shall follow its host variable,
  either immediately (`:WS-PHONE:WS-PHONE-IND`) or after the word `INDICATOR`.
  An indicator shall be a `PIC S9(4)` item, in binary (`COMP`, `COMP-5`,
  `BINARY`) or display usage. For a host structure the indicator shall be a
  table of such items, one per elementary item.
- **R10 (event):** When a value read from the database is NULL, the system
  shall set the indicator to -1 and leave the host variable unchanged. When
  NULL is read into a host variable that has no indicator, the statement shall
  fail with SQLSTATE `22002`.
- **R11 (event):** When a character value is truncated on the way into a host
  variable, the system shall set the indicator (if present) to the value's
  original length and report the warning SQLSTATE `01004`. When a numeric
  value does not fit its host variable, the statement shall fail with SQLSTATE
  `22003` and the item shall be unchanged.
- **R12 (event):** When a host variable is used as input and its indicator is
  negative, the system shall send NULL in its place.
- **R13 (ubiquitous):** Host variables used as input shall be sent as bound
  parameters of the statement, never spliced into its text.
- **R14 (constraint):** A host variable reference shall not be subscripted or
  reference-modified; an indicator table is named without a subscript (R9).
  Figurative constants (`SPACES`, `ZERO` …) shall not be used inside a block;
  a column whose name is a figurative word is written as a quoted identifier.
  Each shall be a compile-time error naming the block and the line.
- **R15 (ubiquitous):** Values shall be converted between COBOL and SQL types
  by a published correspondence table (alphanumeric ↔ character types;
  numeric DISPLAY, packed-decimal and binary items ↔ integer and decimal types
  keeping their scale; date, time and timestamp ↔ alphanumeric items in ISO
  form). The table shall be part of the Developer's Guide.
- **R16 (ubiquitous):** A hyphen inside a block shall be part of a COBOL name
  only where a host variable is named; elsewhere it shall be passed to the
  database unchanged (for example, the SQL subtraction operator).

### 4.3 How a statement went: status items, SQLCA, WHENEVER

- **R17 (ubiquitous):** After every executable SQL statement the system shall
  set each of these the program declares, in either style:
  - **stand-alone items** (the style of the migrated samples): `SQLSTATE`
    (`PIC X(5)`), `SQLCODE` (signed integer) and `SQLMSG` (alphanumeric, the
    database's message, truncated to the item);
  - **an SQLCA**, brought in by `EXEC SQL INCLUDE SQLCA END-EXEC`, whose
    layout is defined by this project and documented in the Guide. It shall
    carry at least SQLCODE, SQLSTATE, the message text and its length, the
    number of rows affected, and warning flags.
- **R18 (ubiquitous):** SQLSTATE shall follow the standard classes: `00000`
  success, class `01` warning, `02000` no data, any other class an error. The
  system shall map each backend's own error to the closest standard SQLSTATE
  and keep the backend's message.
- **R19 (ubiquitous):** SQLCODE shall be 0 for success, +100 for no data, a
  positive value for a warning and a negative value for an error. The mapping
  from SQLSTATE to SQLCODE shall be published in the Guide.
- **R20 (event):** When an executable statement completes, the system shall
  act on the `WHENEVER` declarations in force: `SQLERROR` (an error),
  `SQLWARNING` (a warning) and `NOT FOUND` (no data), each with `CONTINUE` or
  `GO TO` / `GOTO` a paragraph or section.
- **R21 (ubiquitous):** A `WHENEVER` declaration shall apply to the SQL
  statements that follow it **in the source**, until another `WHENEVER` for
  the same condition, regardless of the order in which they run. A `GO TO`
  target that does not exist shall be a compile-time error.
- **R22 (constraint):** An SQL error shall never stop the program by itself.
  With no `WHENEVER SQLERROR GO TO` in force, execution shall continue with
  the status set, as it does for a file status.

### 4.4 Statements

- **R23 (ubiquitous):** The system shall execute, with host variables bound:
  `SELECT … INTO` (a single row), `INSERT`, `UPDATE`, `DELETE`, and any other
  statement the database accepts that returns no rows (data definition
  included).
- **R24 (event):** When a `SELECT … INTO` returns no row, the system shall
  report no data (`02000` / +100) and leave the host variables unchanged.
  When it returns more than one row, it shall fail with SQLSTATE `21000`.
- **R25 (ubiquitous):** `DECLARE cursor CURSOR FOR select-statement` shall
  declare a cursor by name, optionally `WITH HOLD` and `FOR UPDATE [OF
  columns]`. `OPEN` shall evaluate the cursor's host variables at that moment,
  `FETCH cursor INTO host-variables` shall read the next row, and `CLOSE`
  shall release it. A fetch past the last row shall report no data.
- **R26 (ubiquitous):** `UPDATE … WHERE CURRENT OF cursor` and `DELETE …
  WHERE CURRENT OF cursor` shall act on the row the cursor last fetched.
- **R27 (event):** When a cursor that is not open is fetched or closed, or an
  open cursor is opened again, the system shall fail the statement with the
  standard SQLSTATE (`24000` invalid cursor state) and change nothing.
- **R28 (ubiquitous):** Cursor names shall be local to the program that
  declares them, except that a cursor declared in a program's DATA DIVISION
  shall also be known to the programs that program contains, as a GLOBAL data
  item is (a form's event handlers are contained programs). A cursor shall be
  declared before, in the source, any statement that uses it; a use without a
  declaration shall be a compile-time error.
- **R29 (ubiquitous):** `COMMIT [WORK]` and `ROLLBACK [WORK]` inside a block
  shall end the database unit of work on the current connection. They shall
  not affect INDEXED-file transactions, and the COBOL verbs `COMMIT` and
  `ROLLBACK` (which govern INDEXED files) shall not affect the database.
- **R30 (event):** When a unit of work ends with `COMMIT`, cursors without
  `WITH HOLD` shall be closed and `WITH HOLD` cursors shall stay open. When it
  ends with `ROLLBACK`, every cursor shall be closed.
- **R31 (ubiquitous):** `EXEC SQL INCLUDE name END-EXEC` shall bring in the
  copybook `name` as `COPY name` does (same search path), with `SQLCA` and
  `SQLDA` as the reserved names of R17 and R41.
- **R32 (ubiquitous):** `DECLARE … TABLE` shall be accepted as documentation
  and shall have no effect at run time.

### 4.5 SQL connections

- **R33 (ubiquitous):** A project shall define its **SQL connections**, each a
  name, a backend and a connection target (a file path for SQLite; host, port
  and database for PostgreSQL and MySQL), any one of which may be marked the
  default (R35). The IDE's project tree shall show them under an **SQL
  Connections** item, placed after Indexed Files: one row per connection, the
  default marked. The item's `[+]` shall add a connection, and selecting a row
  shall open that connection's editor, where it is changed or removed (removal
  asks for confirmation) and whose **Test connection** button connects with
  the values in the editor and reports success or the database's own message.
  The user name and password shall be kept outside the project folder and the
  repository, in the IDE's credential vault — the store that holds model API
  keys, under the same policy (kept for the session by default, or in the
  local file the developer chose). An SQL connection may be renamed: its
  stored credentials move to the new name, and Check reports every
  `SqlDatabase` control that still names the old one; COBOL code is never
  rewritten.
- **R34 (event):** When a program executes `CONNECT TO name`, where the name is
  a literal or a host variable, the system shall connect to the project's SQL
  connection of that name, compared without regard to case. Otherwise the name
  shall be taken as a connection string only when it starts with `sqlite:`,
  `:memory:`, `postgres://`, `postgresql://` or `mysql://`, contains `/` or
  `\`, or ends in `.db`, `.sqlite`, `.sqlite3` or `.db3`; any other name shall
  fail with SQLSTATE `08001` and a message naming it, so a mistyped name never
  creates an empty database. `CONNECT TO name AS alias` and `USER :user USING
  :password` shall be accepted; the password item shall never appear in a log,
  a trace or a diagnostic.
- **R35 (state):** While a program executes SQL without a current connection,
  the system shall use the project's **default SQL connection**, if one is
  marked, and otherwise fail the statement with SQLSTATE `08003` (connection
  does not exist).
- **R36 (ubiquitous):** `SET CONNECTION name` shall make a connection current;
  `DISCONNECT name`, `DISCONNECT CURRENT` and `DISCONNECT ALL` shall close
  connections, rolling back their uncommitted work.
- **R37 (ubiquitous):** Connections shall belong to the run unit: every program
  and every form of the application, main form and child forms alike, shall
  share them, so a connection opened by the main form is current in a form it
  opens.
- **R38 (event):** When the run unit ends normally — `STOP RUN`, a `GOBACK`
  from the main program, or the main window closing — the system shall commit
  the open unit of work on every connection and close it. When the run unit
  ends because of an error that stops it, or the developer stops it from the
  IDE, the system shall roll the open units of work back and close every
  connection. A runtime error in a child form does not end the run unit.
- **R39 (ubiquitous):** A built application shall resolve its SQL connections
  at run time, so that the same build runs against different databases:
  - from an `sql-connections.toml` file beside the binary, which names each SQL
    connection with its backend, its connection target and, optionally, its
    user name;
  - with the connection target overridable by the environment variable
    `<APP>_SQL_<NAME>_URL` and the user name by `<APP>_SQL_<NAME>_USER`, where
    `<APP>` is the application's name and `<NAME>` the SQL connection's, both
    upper-cased with every character other than a letter or a digit replaced
    by `_`;
  - with the password taken only from the environment variable
    `<APP>_SQL_<NAME>_PASSWORD` or, failing that, from the application's own
    encrypted key store, under the entry `SQL:<NAME>` (filled by the
    application, for example from a settings form). A password written in the
    file shall not be used: connecting through that SQL connection shall fail
    with a message that names the file and says where the password must come
    from.

  Build shall write a starting `sql-connections.toml` beside each binary it
  produces, listing the project's SQL connections with their backends and the
  developer's connection targets and no user name or password, and shall never
  overwrite one that is already there. A packaged project runs through `rcrun`
  with its project file, so it takes its SQL connections from there, with the
  same environment variables. The developer's credentials shall never be
  embedded in the binary.
- **R40 (ubiquitous):** The `SqlDatabase` control shall gain an `SqlConnection`
  property naming one of the project's SQL connections. When it is set, the
  control's generated `<id>-CONNECT` and its `Open()` with no argument shall
  connect through that SQL connection — resolved from the project (R33) in the
  IDE and `rcrun`, and as R39 resolves it in a built application — instead of
  using `ConnectionString`, and the form file shall hold the SQL connection's
  name and no connection target, user name or password. In the designer the
  property shall offer the project's SQL connections by name; a name that is
  not one of them shall be a Check error. The control's open connection shall
  stay its own, separate from the connections of `EXEC SQL`.

### 4.6 Dynamic SQL and the descriptor area

- **R41 (ubiquitous):** The system shall provide an SQL descriptor area
  (SQLDA), brought in by `EXEC SQL INCLUDE SQLDA END-EXEC`, with a layout
  defined by this project and documented in the Guide. It shall describe a
  number of columns or parameters, each with its SQL type, length, precision
  and scale, nullability and name, and a pointer to the data item and one to
  the indicator that hold its value (`SET … TO ADDRESS OF`). When an entry's
  data pointer is NULL, the value shall be placed as text, with its indicator,
  inside the entry itself. Where a database does not report one of these
  properties, the descriptor shall say it is unknown rather than guess.
- **R42 (ubiquitous):** `PREPARE statement-name FROM :host-variable` (or a
  literal) shall prepare the text the item holds; `?` markers in it shall be
  parameters. `EXECUTE statement-name [USING host-variables | USING
  DESCRIPTOR sqlda]` shall run it; `EXECUTE IMMEDIATE :host-variable` shall
  prepare and run in one step a statement that has no parameters and returns
  no rows.
- **R43 (ubiquitous):** `DECLARE cursor CURSOR FOR statement-name` shall
  declare a cursor over a prepared query; `OPEN cursor [USING host-variables |
  USING DESCRIPTOR sqlda]` shall supply its parameters; `FETCH cursor
  [INTO host-variables | USING DESCRIPTOR sqlda]` shall read it.
- **R44 (ubiquitous):** `DESCRIBE statement-name INTO sqlda` shall fill the
  descriptor with the prepared query's result columns, and `DESCRIBE INPUT`
  with its parameters — their number always, their types where the database
  reports them (SQLite and MySQL do not). When the descriptor has fewer entries
  than the statement needs, the system shall report how many are needed and
  fill none, so the program can allocate a larger one and describe again.
- **R45 (event):** When a prepared statement is executed after the text it
  came from changes, the system shall run the statement as prepared, not the
  new text.

### 4.7 Tools: Check, editor, debugger, documentation

- **R46 (ubiquitous):** Check (`rcrun check`, the IDE's Check and the
  coding-agent `check` tool) shall report, without contacting any database:
  an unterminated block, a block in the wrong place, an undeclared or
  ambiguous host variable, an invalid indicator, a subscripted or
  reference-modified host variable, a figurative constant in a block, a cursor
  used before it is declared or declared twice, a statement name used before
  it is prepared in the source (a `DECLARE … CURSOR FOR statement-name` is not
  a use of the statement), a `WHENEVER` target that does not exist, and
  an `INCLUDE` that is not found. Each diagnostic shall name the line.
- **R47 (ubiquitous):** The COBOL editor shall highlight an embedded SQL block
  as SQL, with its host variables highlighted as COBOL names, and its
  go-to-definition shall reach the data item a host variable names.
- **R48 (ubiquitous):** The debugger shall step over an `EXEC SQL` block as one
  statement, and after it shall show the SQL text sent, the bound values (with
  passwords masked), SQLSTATE, SQLCODE, the message and the rows affected.
- **R49 (ubiquitous):** Every user-facing string this feature adds to the IDE —
  the SQL Connections item in the project tree, the connection editor and its
  Test connection messages, and the `SqlConnection` property's help — shall be
  translated in all six languages. Compiler diagnostics stay in English, as
  every other diagnostic does.
- **R50 (ubiquitous):** The Developer's Guide shall gain a chapter on embedded
  SQL written from the PowerCOBOL / isCOBOL developer's point of view, with
  original examples only, the type correspondence table (R15), the SQLSTATE →
  SQLCODE mapping (R19), the SQLCA and SQLDA layouts (R17, R41), setting up
  SQL connections and the deployment file (R33, R39) and the `SqlDatabase`
  control's `SqlConnection` (R40). The System KB and the coding-agent reference
  shall describe the same.

### 4.8 Safety

- **R51 (constraint):** The system shall not build an SQL statement by
  concatenating host-variable values into its text (R13). Dynamic SQL runs
  the text the program prepared, which is the developer's responsibility; the
  Guide shall say so with a caveat.
- **R52 (constraint):** The system shall not write an SQL connection's password to
  the project folder, the repository, a generated program, the binary, a log,
  the Output panel or a crash report.

## 5. Acceptance criteria

All criteria run against **SQLite** in the ordinary test sweep (bundled, no
server). The same programs run against PostgreSQL and MySQL in an
environment-gated suite, skipped and reported as skipped when no server is
configured. Every new test program follows GOLDEN RULE #7: a final result
block naming each statement form exercised, the row counts and the elapsed
time and throughput per phase.

- [ ] **AC1 (R1–R5):** A program in fixed format and the same program in free
  format, each with blocks spanning several lines, with comments of all three
  kinds inside, one block inside an `IF` with no period and one ending a
  sentence, compile and produce identical results. An `END-EXEC` inside an SQL
  string literal does not end its block.
- [ ] **AC2 (R6–R9, R13, R51):** A `SELECT … INTO` a host structure of five
  elementary items (one under `FILLER`, one `REDEFINES`d) fills exactly the
  four named items in order. A table holding a row whose text is
  `x' OR '1'='1` is matched only by itself when used as an input host
  variable, which proves the value was bound, not spliced.
- [ ] **AC3 (R10–R12):** For a NULL column: with an indicator, the indicator is
  -1 and the item unchanged; without one, SQLSTATE is `22002`. Writing a row
  with an indicator of -1 stores NULL. A 30-character value fetched into
  `PIC X(10)` sets the indicator to 30 and SQLSTATE `01004`.
- [ ] **AC4 (R15):** Values round-trip unchanged between the database and
  `PIC S9(7)V99 COMP-3`, `PIC S9(9) COMP-5`, `PIC 9(5) DISPLAY`,
  `PIC X(40)` and an ISO date in `PIC X(10)`, including
  negative values and the largest value each item holds; a value one larger
  fails with `22003` and leaves the item unchanged.
- [ ] **AC5 (R17–R19):** The same failing statement sets, in one program,
  stand-alone `SQLSTATE` / `SQLCODE` / `SQLMSG` and, in another, the SQLCA's
  fields, to the same SQLSTATE, SQLCODE and message. Success, no data,
  truncation warning, a constraint violation and a syntax error each give the
  documented pair.
- [ ] **AC6 (R20–R22):** With `WHENEVER NOT FOUND GO TO END-OF-DATA` placed
  after a fetch in the source but executed before it at run time, that fetch
  is not affected (source order wins). A syntax error with no `WHENEVER
  SQLERROR` in force continues with the next statement.
- [ ] **AC7 (R23–R28):** A cursor over 10,000 rows is opened, fetched to the
  end (the 10,001st fetch reports `02000`) and closed; the result block
  reports rows per second. Updating every third row `WHERE CURRENT OF` the
  cursor changes exactly those rows. Fetching a closed cursor gives `24000`.
  A cursor used before its declaration is a Check error.
- [ ] **AC8 (R29–R30):** An `INSERT` followed by `ROLLBACK` leaves no row; one
  followed by `COMMIT` leaves it. A COBOL `ROLLBACK` verb in the same program
  does not undo the committed row, and an `EXEC SQL ROLLBACK` does not undo an
  INDEXED-file write. A `WITH HOLD` cursor survives `COMMIT`; another cursor
  does not.
- [ ] **AC9 (R33–R37):** A project with an SQL connection `SALES` pointing at
  a SQLite file runs `CONNECT TO 'SALES'` and reads it. The project folder and
  the repository contain no password afterwards (searched). A child form
  opened by the main form reads through the connection the main form opened.
  With no `CONNECT` and a default SQL connection marked, statements use it;
  with none marked, they give `08003`. The project tree's model shows an SQL
  Connections item after Indexed Files, listing `SALES` with the default
  marked, and its `[+]` adds a connection. The Test connection action (the
  function behind the button) reports success for `SALES` and, for a
  PostgreSQL SQL connection on a port nothing listens on, the driver's
  connection error.
- [ ] **AC10 (R38–R39):** Build writes a starting `sql-connections.toml`
  beside the binary with no user name or password, and a rebuild leaves an
  edited one untouched. The binary, with that file pointing `SALES` at a
  different SQLite file, reads that file; with `<APP>_SQL_SALES_URL` set to a
  third file, it reads the third; a `password` key in the file makes the
  connection fail with the documented message. Uncommitted work is committed
  at a normal end and rolled back when the program ends with an error.
- [ ] **AC11 (R41–R45):** A program prepares a query whose columns it does not
  know, describes it into a descriptor that is too small (the system reports
  the number needed), describes again into one large enough, opens a cursor,
  fetches all rows through the descriptor — once into the program's own items
  through the entries' pointers, once as text inside the descriptor — and
  prints them; the column names, types and values match the table.
  `EXECUTE IMMEDIATE` creates a table; `EXECUTE … USING` inserts into it with
  parameters.
- [ ] **AC12 (R2, R14, R46):** One file containing every error listed in R46
  produces exactly one diagnostic per error, each on the right line, with no
  database reachable.
- [ ] **AC13 (R47–R48):** The editor highlights an SQL block and its host
  variables; go-to-definition from a host variable reaches its declaration.
  Stepping over an `EXEC SQL` block in the debugger shows the SQL text, the
  bound values with the password masked, SQLSTATE, SQLCODE and rows affected.
- [ ] **AC14 (parity):** The AC2, AC7 and AC9 programs give the same results
  under `rcrun run`, Run Form, an embedded child form and the compiled binary
  (`interpreter-binary-parity`).
- [ ] **AC15 (migration, narrowed to the SQL — Q14):** In two of the
  operator's sample programs — `F-ART-PURGA.cob` (stand-alone status items,
  `PREPARE` + a cursor over the prepared query, `SELECT … INTO`, `COMMIT`) and
  `TyC.cob` (`CONNECT TO` / `DISCONNECT` a named SQL connection, resolved
  through R34) — no Check diagnostic comes from an `EXEC SQL` block. Their SQL
  statements, run by a test program that declares the host variables they use
  as the samples declare them, work against a SQLite copy of the tables they
  use; a statement SQLite rejects (F-ART-PURGA's `DELETE … LIMIT 1`) sets its
  syntax-error SQLSTATE and the program continues (R22). With a MySQL server
  configured, the same statements run against MySQL. The programs as a whole
  (their non-SQL PowerCOBOL constructs) are spec 086's to convert and are not
  part of this criterion.
- [ ] **AC16 (R49–R50, R52):** The SQL Connections item, the connection
  editor and its Test connection messages, and the `SqlConnection` property's
  help show in all six languages; the
  Guide chapter, the System KB and the agent reference describe embedded SQL;
  a search of the project, the generated programs, the binary, the
  `sql-connections.toml` that Build wrote and the logs after AC9–AC10 finds no
  password.
- [ ] **AC17 (R40):** A form whose `SqlDatabase` names `SALES` in
  `SqlConnection`, with `ConnectionString` left at its default, opens and
  queries the `SALES` file through `<id>-CONNECT` and through `Open()` with no
  argument, in Run Form and in the compiled binary (resolved through R39
  there). With both properties set, `SqlConnection` wins. The saved `.cfrm`
  holds the SQL connection's name and no connection target, user name or
  password, and an `SqlConnection` naming none of the project's SQL
  connections is a Check error.
- [ ] **AC18 (R6, R16, R31–R32, R34, R36, R44–R45):** In one program:
  - a host variable qualified with `OF` and one qualified with a period each
    reach the right item of two that share a name;
  - `SELECT QTY-1 INTO :WS-QTY-LESS-ONE …` returns the column minus one: the
    hyphen in the column expression is SQL subtraction, the one in the host
    variable is part of its name;
  - an `EXEC SQL INCLUDE` of a project copybook declares the host variables
    the next statement uses, and a `DECLARE … TABLE` changes nothing at run
    time;
  - `CONNECT TO` a `sqlite:` connection string `AS SECOND` connects under the
    alias; `SET CONNECTION` switches between it and `SALES`, each query
    reading its own file; `DISCONNECT ALL` closes both;
  - a `CONNECT … USER :WS-USER USING :WS-PASS` with a known password (SQLite
    ignores it) leaves that password in no trace, log or diagnostic
    (searched);
  - `DESCRIBE INPUT` reports the number of a prepared statement's `?`
    parameters;
  - changing the item a statement was prepared from and executing the
    statement again runs it as prepared.

## 6. Constraints & steering check

- **i18n (6 languages):** yes — the SQL Connections item, the connection
  editor and its Test connection messages, the `SqlConnection` property's
  help, and any IDE-shown diagnostic text (R49). COBOL keywords, SQL text and
  identifiers stay English.
- **Project tree:** a new top-level item, SQL Connections (R33). Like
  Project's Crates (spec 044), its rows come from the project, not from files
  on disk, and its `[+]` opens an editor instead of a file picker.
- **Generated-code / regenerate contract:** a form handler may contain `EXEC
  SQL`; the generated program carries the block as written and is regenerated
  as usual. Codegen itself generates no SQL.
- **Docs:** a new chapter in `docs/developers-guide-en.md` (R50); the
  supported-syntax reference `docs/cobol85-supported-syntax-en.md` gains an
  "extensions" entry. Per GOLDEN RULE #8 every English document this changes
  has its five translations deleted in the same change.
- **System KB:** compiler/runtime behaviour changes, so the KB documentation
  tables and `assets/knowledge/chunked.data` are updated in the same change —
  the `SqlDatabase` property table gains `SqlConnection` (R40) — and the
  coding-agent reference (`cobolt-project-tools`) learns `EXEC SQL` too.
- **Three hosts:** the runtime change must reach `rcrun run-form`, embedded
  child forms and `run_form_app` in the compiled binary (the
  `interpreter-binary-parity` skill).
- **What the runtime has today, which the plan must address:** the SQL bridge
  (`cobolt-runtime/src/db_runtime.rs`) runs statement text without bound
  parameters, turns every value — NULL included — into text, and has no
  explicit transaction control. R10–R13, R15, R29 and R30 cannot be met on
  top of it unchanged.
- **COMMIT / ROLLBACK words:** the COBOL verbs keep governing INDEXED files
  (CLAUDE.md, File I/O); only the forms inside `EXEC SQL` touch the database
  (R29).
- **Licensing / originality:** no IBM example, sample, copybook, SQLCA or
  SQLDA layout is copied; the SQLCA and SQLDA layouts are this project's own
  (R17, R41). Original examples only, in code and documentation (product
  steering: "Original work only").
- **Credentials:** never in sources, form files, the repository, the binary,
  the deployment file or logs (R33, R39, R40, R52), following the precedent
  of model API keys.
- **Fix vs feature:** **feature.** Embedded SQL is not part of COBOL-85; it is
  a capability beyond the standard. `features` branch, `z` bump per change,
  f=96 if announced.

## 7. Open questions

- **Q1 — Named connections (settled, operator 2026-10-05):** the project's
  SQL connections by name, credentials in the secure store, a connection
  string accepted directly, and an optional default SQL connection (R33–R35).
  First called "data sources"; renamed SQL connections by Q8.
- **Q2 — Dynamic SQL (settled, operator 2026-10-05):** full — `PREPARE`,
  `EXECUTE`, `EXECUTE IMMEDIATE`, dynamic cursors, `DESCRIBE` and the SQLDA
  (R41–R45).
- **Q3 — Status style (settled, operator 2026-10-05):** both — stand-alone
  `SQLSTATE` / `SQLCODE` / `SQLMSG`, and `INCLUDE SQLCA` with `WHENEVER`
  (R17–R22).
- **Q4 — Dialect (settled, operator 2026-10-05):** the SQL text is passed to
  the database unchanged; only host variables become parameters.
- **Q5 — End of the run unit (settled, operator 2026-10-05):** commit the open
  units of work at a normal end (`STOP RUN`, a `GOBACK` from the main program,
  the main window closing) and roll them back when the program ends with an
  error (R38).
- **Q6 — Deployment configuration (settled, operator 2026-10-05):** a
  `sql-connections.toml` beside the binary, which Build writes as a starting
  point and never overwrites; the connection target and user name overridable
  by `<APP>_SQL_<NAME>_URL` / `_USER`; the password only from
  `<APP>_SQL_<NAME>_PASSWORD` or the secure store, never from the file (R39).
- **Q7 — Editor and `SqlDatabase` (settled, operator 2026-10-05):** an editor
  with a "Test connection" button (R33) — first planned as a page in Project
  Settings, moved to the project tree by Q8 — and the `SqlDatabase` control
  can name an SQL connection through its `SqlConnection` property (R40).
- **Q8 — Where connections live, and their name (settled, operator
  2026-10-05, after approval):** an **SQL Connections** item in the IDE's
  project tree replaces the Project Settings page, and "SQL connection" is the
  name everywhere the developer meets it — the tree, the editor, the
  `SqlDatabase` property, the deployment file and its environment variables
  (R33, R39, R40).
- **Q9 — Credentials (settled, operator 2026-10-05, design review):** in the
  IDE, the credential vault that holds model API keys, under its policy; in a
  built application, the environment variable or the application's own
  encrypted key store, entry `SQL:<NAME>` (R33, R39).
- **Q10 — Packaging and renaming (settled, operator 2026-10-05, design
  review):** only Build writes `sql-connections.toml`; a packaged project takes
  its SQL connections from its project file; a rename moves the stored
  credentials and Check reports `SqlDatabase` controls still naming the old
  name (R33, R39).
- **Q11 — Diagnostics and AC15 (settled, operator 2026-10-05, design
  review):** compiler diagnostics stay in English (R49); `F-ART-PURGA.cob`
  runs partly on SQLite and in full in the MySQL-gated suite (AC15).
- **Q12 — Cursor scope and the SQLDA (settled, operator 2026-10-05, design
  review):** a DATA DIVISION cursor is known to the programs its program
  contains (R28); SQLDA entries carry pointers to the receiving items, with
  the value inside the entry when the pointer is NULL (R41).
- **Q14 — AC15 narrowed to the SQL (settled, operator 2026-10-06):** T10
  found no diagnostic inside the samples' 40 SQL blocks, while the programs
  still fail Check on non-SQL PowerCOBOL constructs (`#FILE`/`#LINE`, `POW-…`,
  `CALL … WITH STDCALL`, `BY VALUE`). AC15 covers the SQL only; converting the
  rest is spec 086's.
- **Q13 — Clarifications from the design review (2026-10-05):** the
  name-versus-connection-string rule of `CONNECT` (R34); `ROLLBACK` closes
  `WITH HOLD` cursors too (R30); an IDE Stop ends the run unit with a rollback,
  and a child form's runtime error does not end it (R38); an indicator table is
  named without a subscript, and a figurative word used as a column name is
  quoted (R14); MySQL reports no parameter types (R44); `DECLARE … CURSOR FOR
  statement-name` is not a use of the statement (R46).
