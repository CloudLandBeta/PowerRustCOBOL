<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.80.214 -->

# RustCOBOL Database Runtime

RustCOBOL programs talk to SQL databases through a small set of built-in
calls, written inline as methods of the `COBOL` object —
`COBOL::"OPEN-DB" ( WS-CONN WS-HANDLE WS-STATUS )`, which is exactly
`CALL "COBOL-OPEN-DB" USING WS-CONN WS-HANDLE WS-STATUS`. The same six verbs
work against **three backends** — the engine is selected automatically from
the connection string, so a program written for SQLite runs unchanged against
PostgreSQL or MySQL by changing one literal.

> **Two ways to reach a database.** This document covers the built-in calls,
> which pass SQL as text and hand every value back as text. **Embedded SQL**
> (`EXEC SQL … END-EXEC`, with typed host variables, cursors, a status area and
> units of work) is the classic COBOL way and is described in the
> [Developer's Guide](developers-guide-en.md#embedded-sql-exec-sql). Both reach
> the project's named SQL connections.

| Backend     | Driver (no system library needed)     | Connection string                                  |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`, `features = ["bundled"]` — compiles the SQLite **C** amalgamation, so this one is not pure Rust | `:memory:`, `sqlite:<path>`, or a bare file path   |
| **PostgreSQL** | `postgres` (rust-postgres, sync)   | `postgres://user:pass@host:port/db`                |
| **MySQL**   | `mysql` (`minimal-rust`, sync, no TLS) | `mysql://user:pass@host:port/db`                  |

All three drivers are statically linked and require **no external client
library** (`libpq`, `libmysqlclient`) and **no OpenSSL** to build — consistent
with the rest of PowerRustCOBOL.

---

## 1. Connection strings

The backend is chosen purely from the scheme of the connection string:

| Form                                       | Backend       | Notes                                  |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | In-RAM database, discarded on close.   |
| `sqlite:/var/data/app.db`                  | SQLite        | File is created if it does not exist.  |
| `/var/data/app.db`                         | SQLite        | A bare path is treated as SQLite.      |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` is also accepted.   |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

Matching is case-insensitive on the scheme and tolerant of surrounding
whitespace. Anything that is **not** a `postgres(ql)://` or `mysql://` URL —
or the prefix below — is treated as a SQLite target.

### A project SQL connection, by name

`sql-connection:<NAME>` names one of the project's **SQL connections** (the
project tree's *SQL Connections* item) instead of spelling the database out:

```cobol
           COBOL::"OPEN-DB" ( "sql-connection:SALES" WS-HANDLE WS-STATUS )
```

The name is compared without regard to case and resolved when the connection
opens — from the project file under the IDE and `rcrun`, from the
`sql-connections.toml` beside a built application — exactly as
`EXEC SQL CONNECT TO 'SALES'` resolves it. The environment variables
`<APP>_SQL_<NAME>_URL`, `_USER` and `_PASSWORD` override it, and a server is
reached field by field, so the password never travels inside a URL — nor
appears in the program, the form or the project. A SQLite file the SQL
connection does not allow creating must exist: a wrong path is an error, not
a new empty database. A name the project does not define fails with a message
naming it.

---

## 2. The built-in calls

A data item is passed by reference and a literal by content. Status and
handle values live in ordinary COBOL data items so they can be held and passed
across paragraphs. Each one is also reachable as `CALL "COBOL-<NAME>" USING …`
— the same call, written the older way.

| Built-in               | Arguments                                               |
|------------------------|---------------------------------------------------------|
| `COBOL::"OPEN-DB"`    | conn-string, handle-var `PIC 9(9)`, status-var          |
| `COBOL::"EXEC-SQL"`   | handle, query, row-count-var `PIC 9(9)`, status-var     |
| `COBOL::"FETCH-ROW"`  | handle, col-index `PIC 9(n)` (1-based), dest-var, status |
| `COBOL::"NEXT-ROW"`   | handle, more-flag-var `PIC X` (`Y`/`N`)                 |
| `COBOL::"ROW-COUNT"`  | handle, count-var `PIC 9(9)`                            |
| `COBOL::"CLOSE-DB"`   | handle                                                  |

### Semantics

- **`COBOL::"OPEN-DB"`** opens a connection and writes a positive integer handle
  into *handle-var*. On success *status-var* is set to spaces; on failure
  *handle-var* is `0` and *status-var* holds the driver error message.
- **`COBOL::"EXEC-SQL"`** runs one statement on *handle*.
  - For row-returning statements (`SELECT`, CTEs, …) the full result set is
    cached and *row-count-var* receives the **number of rows**. The cursor
    starts on the first row.
  - For `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* receives the
    **number of affected rows** and the result set is empty.
  - On error *status-var* holds the message and *row-count-var* is `0`.
- **`COBOL::"FETCH-ROW"`** copies column *col-index* (1-based) of the **current**
  row into *dest-var* as text. Out-of-range columns and an exhausted cursor
  yield spaces.
- **`COBOL::"NEXT-ROW"`** advances the cursor and sets *more-flag-var* to `Y` if a
  row is now available or `N` once the set is exhausted.
- **`COBOL::"ROW-COUNT"`** returns the cached row count of the last query.
- **`COBOL::"CLOSE-DB"`** closes the connection and frees its result set. Unknown
  handles are ignored. All open connections are closed when the program ends.

### Value normalisation

Every column value — regardless of backend or SQL type — is delivered to COBOL
as **text**, so it can be `MOVE`d straight into a `PIC X` field (or into a
numeric field, which reinterprets the digits). The normalisation is uniform:

| SQL value      | Text delivered to COBOL                |
|----------------|----------------------------------------|
| `NULL`         | spaces (empty string)                  |
| integer        | decimal digits, e.g. `42`, `-7`        |
| real / double  | shortest round-trip form, e.g. `3.14`  |
| text / varchar | the UTF-8 string                       |
| date           | `YYYY-MM-DD`                           |
| datetime       | `YYYY-MM-DD HH:MM:SS`                   |
| time (MySQL)   | `HH:MM:SS`                             |
| blob (SQLite)  | `<blob N bytes>` placeholder           |

---

## 3. Example — portable CRUD

This program runs against **any** of the three backends; only `WS-CONN`
changes. The test suite runs it exactly as written here
(`crates/cobolt-runtime/tests/test_sql.rs`,
`the_guides_crud_example_runs_as_written`).

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-CRUD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CONN     PIC X(64)  VALUE ":memory:".
      *>  PostgreSQL: VALUE "postgres://scott:tiger@localhost:5432/store".
      *>  MySQL:      VALUE "mysql://scott:tiger@localhost:3306/store".
       01 WS-HANDLE   PIC 9(9)   VALUE 0.
       01 WS-STATUS   PIC X(128) VALUE SPACES.
       01 WS-QUERY    PIC X(256) VALUE SPACES.
       01 WS-ROWCNT   PIC 9(9)   VALUE 0.
       01 WS-COL      PIC 9(4)   VALUE 1.
       01 WS-NAME     PIC X(16)  VALUE SPACES.
       01 WS-MORE     PIC X      VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"OPEN-DB" ( WS-CONN WS-HANDLE WS-STATUS )
           IF WS-STATUS NOT = SPACES
               DISPLAY "OPEN FAILED: " WS-STATUS
               STOP RUN
           END-IF

           MOVE "CREATE TABLE c (id INTEGER, name TEXT)" TO WS-QUERY
           COBOL::"EXEC-SQL" ( WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS )

           MOVE "INSERT INTO c VALUES (1,'ANA'),(2,'BRUNO'),(3,'CARLOS')"
               TO WS-QUERY
           COBOL::"EXEC-SQL" ( WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS )
           DISPLAY "INSERTED " WS-ROWCNT

           MOVE "SELECT name FROM c ORDER BY id" TO WS-QUERY
           COBOL::"EXEC-SQL" ( WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS )
           DISPLAY "ROWS " WS-ROWCNT

           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               MOVE 1 TO WS-COL
               COBOL::"FETCH-ROW" ( WS-HANDLE WS-COL WS-NAME WS-STATUS )
               DISPLAY "NAME " WS-NAME
               COBOL::"NEXT-ROW" ( WS-HANDLE WS-MORE )
           END-PERFORM

           COBOL::"CLOSE-DB" ( WS-HANDLE )
           STOP RUN.
```

Output (in-memory SQLite):

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### Reading multiple columns

`FETCH-ROW` reads one column per call; change `WS-COL` to read others from
the same row before advancing:

```cobol
           MOVE 1 TO WS-COL
           COBOL::"FETCH-ROW" ( WS-HANDLE WS-COL WS-ID WS-STATUS )
           MOVE 2 TO WS-COL
           COBOL::"FETCH-ROW" ( WS-HANDLE WS-COL WS-NAME WS-STATUS )
           COBOL::"NEXT-ROW" ( WS-HANDLE WS-MORE )
```

---

## 4. Transactions

Transactions are driven with ordinary SQL through `EXEC-SQL`, so the
behaviour is exactly your server's:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           COBOL::"EXEC-SQL" ( WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS )
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           COBOL::"EXEC-SQL" ( WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS )
```

> The COBOL `COMMIT` / `ROLLBACK` **verbs** are a separate feature that controls
> RustCOBOL **INDEXED file** transactions (see
> [`docs/indexed-file-format-en.md`](indexed-file-format-en.md)). They do **not** act
> on SQL connections — use `COBOL::"EXEC-SQL"` with `BEGIN`/`COMMIT`/`ROLLBACK`
> for the database, as shown above.

PostgreSQL and MySQL default to autocommit, so a lone statement is committed
immediately. Wrap a unit of work in `BEGIN … COMMIT` to make it atomic.

---

## 5. The IDE data control

In PowerRustCOBOL's form designer, an **SqlDatabase** control generates the
boilerplate paragraphs (`<id>-CONNECT`, `<id>-EXEC`, `<id>-FETCH-ALL`,
`<id>-CLOSE`) automatically. Two properties matter:

- **`ConnectionString`** — any of the connection strings above. This is what
  actually selects the backend at runtime.
- **`SqlConnection`** — the name of one of the project's SQL connections. Set,
  it **wins** over `ConnectionString`: the generated `<id>-CONNECT` opens
  `sql-connection:<NAME>`, and `Open()` with no argument does the same. The
  form keeps only the name; a name the project does not define is a Check
  error. The control's connection is its own — apart from the connections
  `EXEC SQL` uses, even when both name the same SQL connection.
- **`Driver`** — `sqlite` (default), `postgres`, or `mysql`. Cosmetic only: it
  labels the generated comments; routing is by the connection string.

---

## 6. Security & operational notes

- **TLS.** ⚠️ **Neither SQL driver speaks TLS today.** The MySQL driver is built
  with `default-features = false, features = ["minimal-rust"]`, and the resolved
  `mysql 28` pulls in no TLS crate at all — it cannot negotiate a secure
  connection, whatever the server asks for. The synchronous PostgreSQL driver
  connects with `NoTls` by construction. Both are suitable for local sockets and
  trusted networks. For a server that requires TLS, terminate it at a local proxy
  (e.g. `stunnel`/`pgbouncer`) or run over an SSH tunnel.
- **SQL injection.** Statements are sent as text. Build queries from trusted
  input, or pre-validate/escape any user-supplied values before composing the
  SQL string.
- **Connection lifetime.** Each handle owns one live connection. Close handles
  you no longer need with `COBOL-CLOSE-DB`; everything left open is closed when
  the program terminates.

---

## 7. Testing

- **Offline (always run):** connection-string routing, value normalisation, and
  a full in-memory SQLite CRUD round-trip —
  `cargo test -p cobolt-runtime --lib db_runtime` and
  `cargo test -p cobolt-runtime --test test_sql`.
- **Live servers (opt-in):** two `#[ignore]`d round-trip tests connect to real
  servers. Provide a URL and run them explicitly:

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implementation

`crates/cobolt-runtime/src/db_runtime.rs` holds the engine. A `DbConn` wraps a
`Backend` enum (`Sqlite` / `Postgres` / `MySql`); `BackendKind::classify`
chooses the backend from the connection string. Each backend has its own
`exec_*` path that normalises rows to `Vec<Vec<String>>`, after which the shared
cursor logic (`fetch_col` / `next_row` / `row_count`) is backend-agnostic. The
interpreter's `exec_call` (`crates/cobolt-runtime/src/interpreter.rs`) maps the
six COBOL CALLs onto `DbRegistry`, which pools connections by integer handle.
