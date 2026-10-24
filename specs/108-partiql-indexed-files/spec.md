<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — PartiQL for indexed files

- **Status:** draft, written 2026-10-24 from the operator's two requests of that
  day; awaiting the operator's review. §9 lists the decisions to settle before
  `/plan`; each carries a recommendation so a "yes" is enough.
- **Folder:** specs/108-partiql-indexed-files/
- **Author:** Anthropic Claude Codex Agent (for Emerson Lopes)   **Date:** 2026-10-24
- **Classification:** feature — a query language on indexed files and the first
  `::` members of the `IndexedFile` control are capabilities beyond COBOL-85 and
  beyond the IDE's existing scope. `features` branch, next fix number `z` per
  change, forum f=96 if ever announced (GOLDEN RULE #5 / #4b).

## 1. Overview

The operator, 2026-10-24:

> add a new spec to support PartiQL for indexed files. Use Amazon DynamoDB
> PartiQL as the base for IndexedFile control.

and, the same day:

> add the ability to define the PartiQL query in all data bound controls, so
> they would retrieve data using the resultset provided by the indexedfile
> control upon the PartiQL execution.

PartiQL is the SQL-compatible query language Amazon DynamoDB exposes: four
statements (`SELECT`, `INSERT`, `UPDATE`, `DELETE`), a handful of functions
(`begins_with`, `contains`, `size`, `attribute_type`, `IS MISSING`, `EXISTS`),
the usual comparison and logical operators, `?` parameters, and two ways of
running several statements together (a transaction and a batch). This feature
gives RustCOBOL's **indexed files** that language, through the **`IndexedFile`
control**: a form handler writes

```cobol
           SET WS-FOUND TO IXF-CUST::Execute(
               "SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM CUSTOMERS WHERE CUST-BALANCE > ?"
               NUD-MIN::Value)
```

and reads the result set item by item, while a DataGrid, a ListBox, a chart or
a repeating group **bound to that control** names its own `SELECT` and fills
itself from the result set — with `?` parameters taken from the form, so a
grid of orders follows the customer the operator selects without a line of
COBOL.

The mapping is the natural one: a `.cidx` definition is the table's schema, a
record is an item, a field is an attribute, the RECORD KEY is the primary key
and an ALTERNATE RECORD KEY is a secondary index. Where DynamoDB's statements
and the indexed engines meet, this spec defines a **superset**: every statement
DynamoDB accepts in the forms listed in §5 is accepted here with the same
meaning, and the few things a fixed record layout makes different (hyphens in
names, fields that cannot be absent, typed comparisons) are listed as
departures, each with its reason. No statement that both accept gives a
different answer.

The language definition in §5 is written from the public DynamoDB PartiQL
reference (statement syntax, functions, operators, data types, transactions and
batches, read on 2026-10-24). That reference is a map of the subject only: no
AWS example, table or wording is reproduced here; every example in this spec
and in the work that follows is original (product steering, "Original work
only").

## 2. What exists today

*Mapped 2026-10-24 against `main` at 1.91.223 and the working tree. Line
numbers are the working tree's.*

**The `IndexedFile` control has thirteen file methods, generated as PERFORM
paragraphs; what it lacks is only the inline `::` form.** Its file-method
facade — OPEN, START, READ-NEXT, READ-PREVIOUS, READ-FIRST, READ-LAST,
READ-INVALID, WRITE, REWRITE, DELETE, COMMIT, ROLLBACK and CLOSE — is written
by `cobolt-codegen` into the *outer* form program as the paragraphs
`<id>-<OP>` (`crates/cobolt-codegen/src/lib.rs:1956-2147`); the generated main
program performs OPEN before `onLoad` when `AutoOpen` is on and CLOSE after
the event loop. What a handler cannot do is call one of them inline, as it
calls every other control's methods (`IXF-1::ReadNext()`): a handler is a
nested program and cannot `PERFORM` the outer program's paragraphs, the
run-time method dispatch has no arm for the control — `IXF-1::Open()` lands in
the SqlDatabase `OPEN` arm and any other name is read or written as a property
(`crates/cobolt-runtime/src/interpreter.rs:16098-16151`, `:17620`,
`:17792-17819`) — the compiler's own reference says so in as many words
(`crates/cobolt-compiler/src/lib.rs:4866`: "IndexedFile controls have no `::`
methods … There is currently no supported way to drive an IndexedFile control
from an event handler"), and a test pins it
(`crates/cobolt-ide/src/form_runtime.rs:1288-1324`). In practice a handler
reaches the file with the plain COBOL verbs on an `FD … IS GLOBAL` the
developer declares by hand in the form's COBOL structure, which the facade's
own `OPEN <file>` also needs. The control's properties are `IndexedFile` (the
`.cidx` path), `OpenMode` (`INPUT` default, or `I-O`), `AutoOpen`,
`RecordName`, `KeyName`, `CurrentKeyDataItem`, `StatusDataItem`,
`CurrentRecordDataItem` and `OperatorName` (`crates/cobolt-forms/src/model.rs:6441-6456`);
it declares `onError`, `onComplete`, `onCancelled` and `onTimeout`, and nothing
raises them (`model.rs:3908-3914`).

**The `.cidx` is the schema.** `IndexedDefinition { name, assign_path,
access_mode, record_format, storage, compression, persistence, keys, fields, … }`
(`crates/cobolt-indexed/src/model.rs:277-291`); `KeySchema { primary, alternates }`
with each `KeyDef { name, parts, duplicates_allowed, ordering }` — a name is
written for alternates only — and `IndexedField { level, name, pic, usage,
offset, length, occurs, redefines, children, … }`. The parsed PICTURE records
category, width and sign but **not the scale** (`pic.rs:27-31`, `:107-110`), so
`format_field_display` shows `PIC 9(6)V99` as digits without a decimal point.
The run-time record layout built from an FD does know the scale
(`crates/cobolt-runtime/src/files.rs:27-58`).

**The engines** (`crates/cobolt-runtime/src/indexed.rs`, `indexed_disk.rs`,
`indexed_redb.rs`) implement one trait, `IndexedStore`: `open`, `close`,
`write`, `read_key`, `read_seq(Next|Previous)`, `start(Eq|Gt|Ge|Lt|Le, key)`,
`rewrite`, `delete`, `set_key_of_reference`, `commit`, `rollback`, `unlock`
(`indexed.rs:35-60`). Keys compare bytewise; a START on a key prefix is already
a range (`generic_key_bounds`, `:1656`); duplicates on an alternate key are
read in the order they joined. **There is no predicate scan**: the only
filtering anywhere is the AI agents' read-only search tool (a full scan with
case-insensitive `contains`, `mcp_tool.rs:381-577`) and the DataGrid's display
filter over rows already loaded. Transactions are an undo journal per open
file, ended by the COBOL `COMMIT` / `ROLLBACK` verbs, which act on every open
indexed file (`interpreter.rs:5768-5785`); the file statuses are `00 02 10 21
22 23 24 35 37 39 46 47 48 49 90 92 93 99` (`indexed.rs:63-85`).

**Data binding lives on the form, not the control** (`Form::data_bindings`,
`model.rs:8826`; saved as JSON inside `<DataBindings>`). A binding has a
source, a target, field mappings and a mode. Sources today:
`IndexedFile { definition_path, record_name, fields, key_field, writable }` — a
`.cidx` path, never a control — `Sql { source_control_id, query, table_name, … }`
— **the one source that carries a query run by a named control**
(`model.rs:2186-2210`) — `CobolTable`, `RestApi` and `AgentAi`. Approved
targets (`model.rs:2903-2916`): DataGrid, ListBox, ComboBox, the seven charts,
Knob / Gauge / Switch (a scalar), Maps (markers) and a GroupBox with
`IsRepeatingGroup` (a control array). **At run time only a DataGrid can be
filled from an indexed file or SQL**; every other target fills from a COBOL
table only (`crates/cobolt-codegen/src/data_binding.rs:186-345`). The indexed
read is `refresh_indexed_file_binding` (`interpreter.rs:13691-13806`): it opens
its own `GridSession` on the data file, reads **every record in primary-key
order** — no filter, no key range, no limit — formats each cell and writes the
grid's `Rows` as TAB- and newline-separated text. Rows move only through
`RefreshBinding()`, which branches on the hidden `_BindingKind` property; the
generated `COBOL-DATA-BINDINGS-POPULATE` seeds the `_Binding*` properties and
invokes it after `onLoad` (`lib.rs:1161-1239`). Write-back (specs 094 and 095)
finds each edited row by the `.cidx` primary key, rewrites, and ends with one
`commit()` or a `rollback()`; `Save()`, `AutoCommit`, `CommitStatus`
(`00 21 23 90 91 92 95 96 98`), `onCommitError` and `onSaved` are the
developer's side of it (`interpreter/grid_unit.rs:515-691`).

**The binding editor** (`crates/cobolt-ide/src/panels/properties.rs`) offers
IndexedFile, SQL, COBOL table and REST sources; the SQL source has a
multi-line Query box and a Table box (`:2317-2356`). Three things it shows for
an indexed source are stand-ins, not the project's: the fields are samples
(`sample_indexed_field_rows`, `:1721-1776`), the record name is the constant
`CUSTOMER-RECORD` (`:1044`), and the SQL control list is `MOCK_SQL_CONTROLS`
(`:1300-1305`). The guardian (`crates/cobolt-project-tools/src/binding_guardian.rs`)
never loads a `.cidx`. The binding model has a `ListValue` mapping that
**nothing reads at run time** (`model.rs:2340-2380`; `data_binding.rs:593`).

**Nearest precedents for a query surface.** Embedded SQL (spec 087) binds
`:host` variables as parameters and reports through `SQLSTATE` / `SQLCODE`.
The `SqlDatabase` control answers `Query(sql)` with a row count, `Fetch()` with
the next row TAB-joined (empty when spent), `ColumnNames()`, `ColumnCount()`,
`ColumnName(n)`, and publishes `ResultCount`, `LastError` and `StatusCode`
(`model.rs:1631`; `interpreter.rs:17620-17791`). The `AwsDynamoDB` control
reaches a real DynamoDB table with `GetItem`, `Query(keyCondition,
valuesJson)`, `Scan`, `PutItem`, `UpdateItem`, `DeleteItem`, `GetRow(i)` and
`GetField(i, name)` — **it has no `ExecuteStatement`**, so nothing in the
product speaks PartiQL today (`rg -i partiql` finds nothing on any branch).

**The 2.x.x line.** The browser allows in-memory INDEXED files (WASM-010 R7,
operator 2026-10-22) and excludes disk files, `redb` and `std::fs`; an
IndexedFile control on the web goes through the backend, which runs the
native runtime (WASM-004 "C — through the backend"; WASM-005 R12–R13). The
WASM specs are on `2.x.x` / `features-2x`, not on `main`.

**Defects met on the way, outside this spec's scope** (recorded so the plan
does not build on them, and so they can be fixed on `fixes` separately): the
facade's `<id>-ROLLBACK` paragraph is `CLOSE` + `OPEN`, and every engine's
`close()` commits, so it commits instead of rolling back
(`lib.rs:2129-2133`; `indexed_disk.rs:2266`, `indexed.rs:698`,
`indexed_redb.rs:1073`); the agent prompts in `crates/cobolt-ide/src/llm.rs`
(`:4826`, `:4919`, `:5047`, `:5295`) and the skill text
`assets/agentic_ai/skills/rustcobol-extensions.md:176-177` tell a model to call
`::Open`, `::ReadNext`, `::Write` … on an IndexedFile, none of which exist; the
editor's autocomplete has no `IndexedFile` arm and offers it the visual
methods (`panels/editor.rs:389`, `:649`); the status `02` is defined and never
emitted.

## 3. Goals / Non-goals

**Goals**

- A form handler drives an indexed file with PartiQL statements through the
  `IndexedFile` control, in every host that runs a form: Run Form
  (`rcrun run-form`), an embedded child form and the compiled binary, with the
  same results in all three.
- A statement that runs on DynamoDB, in the forms §5 lists, runs here
  unchanged and means the same; the departures are few, listed and reasoned.
- Values keep their type: a `PIC 9(6)V99` field is a number with two decimals,
  a `PIC X(40)` field is a string without its trailing spaces.
- `?` parameters are bound to COBOL data items and control properties, never
  spliced into the text.
- A statement whose WHERE uses the key reads by the key; any other statement
  scans and filters. Both work; Check says which one a statement is.
- Several statements run as one all-or-nothing unit on the file's own undo
  journal.
- **Every data-bound control** (DataGrid, ListBox, ComboBox, the charts,
  Knob / Gauge / Switch, Maps, a repeating GroupBox) can name an `IndexedFile`
  control and a PartiQL `SELECT`, with parameters taken from the form, and
  fills itself from the result set — at form open, on demand, and, for a
  control that names no query of its own, after every statement the
  `IndexedFile` control runs.
- The outcome of every statement is a FILE STATUS the developer already knows,
  plus the DynamoDB exception name and a message.
- Check validates a literal statement against the `.cidx` before anything
  runs; the debugger shows what ran.

**Non-goals**

- **A PartiQL batch** (`BatchExecuteStatement`). Its value on DynamoDB is one
  network round trip for up to 25 independent singleton statements; on a
  local file a `PERFORM` loop of `Execute` is the same thing with a status per
  statement. Listed in §9 (Q5) in case the operator wants it for fidelity.
- PartiQL from a program that has no form (`rcrun run` of a plain `.cbl`), and
  an `EXEC PARTIQL … END-EXEC` block. The operator scoped this to the control;
  §9 (Q6) proposes the small built-in that would also let `tests/cobol/**`
  exercise the executor.
- Statements DynamoDB's PartiQL does not have: `JOIN`, `GROUP BY`, aggregates
  (`COUNT`, `SUM`, …), `DISTINCT`, `LIKE`, `LIMIT`, `OFFSET`, `AS` aliases,
  subqueries outside `EXISTS`, `CREATE`/`DROP`. Each is rejected with a
  message that names it (R6). `ResultCount` answers what `COUNT(*)` would.
- Values a fixed record layout cannot hold: Boolean, `NULL`, Binary, string and
  number sets (`<<…>>`), and maps or lists as *values*. A group *is* a map
  path and an `OCCURS` field *is* a list path (§5.2), but `list_append`,
  `set_add` and `set_delete` have no variable-length collection to act on and
  are rejected.
- Pagination (`Limit`, `NextToken`, `LastEvaluatedKey`) and `ConsistentRead`:
  a result set is complete and local. A scan of a very large file into a
  bound grid is the developer's choice, as the quick-bind scan is today; the
  Guide says so and names the key condition as the remedy.
- Collations: keys compare bytewise, as the engines do today
  (`indexed.rs:245-246`); `ORDER BY` is that order.
- A PartiQL filter in the IDE's Grid Browser, and PartiQL in the AI agents'
  indexed-file search tool. Natural follow-ups (§9, Q7), not this spec.
- Changing how the existing `.cidx` quick-bind (`DataSource = "<cidx> / <record>"`)
  and the existing `IndexedFile` binding source load or display. They stay as
  they are; the PartiQL source is a new, third way to bind.
- PartiQL on the `AwsDynamoDB` control (its `ExecuteStatement`). The language
  definition here is written so that a later spec can share the parser.
- The 2.x.x browser engine (`cobolt-wasm-rt`): §8 says what this feature means
  for that line.

## 4. User stories

- As a COBOL developer who built PowerCOBOL screens over indexed files, I want
  to find the customers whose balance exceeds a value with one statement, so
  that I stop writing a `START` / `READ NEXT` loop with an `IF` for every
  search screen.
- As a developer, I want a DataGrid to show the orders of the customer selected
  in another grid, by naming `SELECT … WHERE CUST-ID = ?` with the parameter
  taken from the first grid's selected row, so that a master-detail screen
  needs no handler code.
- As a developer, I want a ComboBox filled from `SELECT CUST-ID, CUST-NAME FROM
  CUSTOMERS` and to read the selected customer's id, so that a lookup list
  carries the key the user cannot see.
- As a developer, I want a search button whose handler runs one `Execute` and
  every control bound to that `IndexedFile` control to show the result, so that
  the search logic lives in one place.
- As a developer, I want to move an amount between two records as one unit
  that either happens entirely or not at all, with a condition check on the
  source record, so that a failure half-way leaves the file consistent.
- As a developer who knows DynamoDB, I want the statements I already write
  against DynamoDB tables to work on a local indexed file, so that a program
  can be developed and tested offline and keep its data access idiom.
- As a developer, I want Check to tell me that a statement names a field the
  record does not have, compares a number with a string, or will scan the
  whole file, so that the mistake surfaces while I type and not in the
  operator's hands.
- As a developer, I want a line chart of a customer's order totals by date to
  come from a `SELECT` with no COBOL in between, so that a report form is a
  query and a chart.

## 5. The language: DynamoDB PartiQL on an indexed file

This section defines what the executor accepts. §6 states the requirements
that refer to it; the Developer's Guide chapter (R56) is written from it.

### 5.1 Statements

Keywords are case-insensitive. A statement may span lines. String literals use
single quotes, a quote inside them is doubled (`'O''Brien'`); a backslash is
an ordinary character. Numbers are written as `12`, `12.50`, `-3`, `1e3`.
Table, index and field names may be written bare or in double quotes; a name
that is a PartiQL keyword (`VALUE`, `SET`, `FROM`, `ORDER`, `SIZE`, …) or does
not start with a letter must be quoted.

| Statement | Form | Outcome |
|---|---|---|
| `SELECT` | `SELECT projection FROM "TABLE"[."INDEX"] [WHERE condition] [ORDER BY key-field [ASC\|DESC]]` — `projection` is `*` or a comma-separated list of field paths, functions and arithmetic expressions | the matching records, as a result set |
| `INSERT` | `INSERT INTO "TABLE" VALUE {'FIELD': value, 'FIELD': value, …}` | one record written |
| `UPDATE` | `UPDATE "TABLE" (SET path = value \| REMOVE path)… WHERE condition [RETURNING (ALL OLD \| MODIFIED OLD \| ALL NEW \| MODIFIED NEW) *]` — any number of `SET` / `REMOVE` clauses, in any order | one record rewritten; with `RETURNING`, one item in the result set |
| `DELETE` | `DELETE FROM "TABLE" WHERE condition [RETURNING ALL OLD *]` | one record deleted; with `RETURNING`, one item in the result set |

A `value` is a literal, a `?` parameter, a field path, or an arithmetic
expression of those with `+` and `-`. In `UPDATE … SET`, the right-hand side
may read the record's own fields (`SET CUST-BALANCE = CUST-BALANCE - ?`).

**Conditions.** `=`, `<>`, `!=`, `<`, `<=`, `>`, `>=`; `BETWEEN a AND b`
(inclusive); `IN [v, v, …]` and `IN (v, v, …)`; `AND`, `OR`, `NOT`,
parentheses; `path IS MISSING`, `path IS NOT MISSING`; the functions below.
Precedence, highest first: comparisons, `IN`, `BETWEEN`, functions,
parentheses, `NOT`, `AND`, `OR` — DynamoDB's order.

**Functions.** `begins_with(path, 'prefix')`, `contains(path, value)`,
`size(path)`, `attribute_type(path, 'S' | 'N' | …)`, and, inside a
transaction only, `EXISTS(SELECT * FROM "TABLE" WHERE full-primary-key AND
condition)`. Names are case-insensitive.

**Parameters.** A `?` stands for the next argument after the statement, in
order. A parameter is a value; it cannot stand for a table, index or field
name.

**The table and the index.** `"TABLE"` is the control's file, named as the
generated SELECT and the facade name it: the upper-cased stem of the control's
`.cidx` path (`indexed/customers.cidx` → `CUSTOMERS`). `"TABLE"."INDEX"` reads
through an ALTERNATE RECORD KEY, named by its name in the `.cidx`; an alternate
without a name is addressed by the name of its first field. The RECORD KEY has
no index name: `FROM "TABLE"` reads through it.

### 5.2 How the record is an item

| In DynamoDB | On an indexed file |
|---|---|
| table | the file the `IndexedFile` control holds (one file per control) |
| item | a record |
| attribute | a field of the record: an elementary item of the `.cidx` record, by its name, case-insensitive; a name that is not unique in the record is qualified with its group as a path (`BILLING.CITY`) |
| nested map | a group item: `GROUP.FIELD` |
| list element | an occurrence of an `OCCURS` field: `ITEM-QTY[0]` is the first (0-based, as PartiQL counts — §9 Q1) |
| primary key (partition key, or partition + sort key) | the RECORD KEY, all its parts, in order |
| secondary index | an ALTERNATE RECORD KEY (`"ORDERS"."BY-DATE"`) |
| String | a field whose PICTURE is `X`, `A`, `N` or `U`: its content **without trailing spaces** (`'Ana   '` is `'Ana'`; a field of spaces is `''`) |
| Number | a field whose PICTURE is numeric (`9`, `S`, `V`, `P`), in any usage (DISPLAY, COMP, COMP-3, BINARY): its value **with its scale** — `PIC S9(6)V99` holding `0012345{` is `-123.45` |
| Boolean, Null, Binary, String Set, Number Set, Binary Set, Map value, List value | not representable; a literal of these kinds is a `ValidationException` (R6) |
| `MISSING` | a declared field is never missing: `IS MISSING` is false and `IS NOT MISSING` true for every field of the record; a name that is **not** a field is a `ValidationException` (the schema is known, unlike DynamoDB's) |
| `attribute_type(f, 'S')`, `('N')` | true when the field's PICTURE is alphanumeric / numeric; every other type code is false |
| `size(f)` | a String: its length without trailing spaces; an `OCCURS` field: its number of occurrences; a group: its number of subordinate fields |
| `contains(f, v)` | a String: substring; an `OCCURS` field: an occurrence equal to `v` |
| `begins_with(f, 'p')` | a String whose value starts with `p` |
| comparison between a Number field and a String literal or parameter, or the reverse | a `ValidationException` (DynamoDB answers *false*; here the mistake is reported — §9 Q10) |
| `INSERT` without every field | the fields not named take their initial value: spaces for a String, zero for a Number (a fixed layout has no absent field) |
| `REMOVE path` | the field takes its initial value |
| `REMOVE ITEM-QTY[2]` | that occurrence takes its initial value; nothing shifts |
| `list_append`, `set_add`, `set_delete` | rejected: no variable-length collection in a fixed layout |
| the exceptions | a FILE STATUS in `StatusCode`, the exception name and message in `LastError` (§6.5, R27) |

A field that `REDEFINES` another is an ordinary attribute; writing one changes
what the fields it redefines show. `FILLER` is not addressable.

### 5.3 How a statement reads the file

Every statement has a **key of reference**: the RECORD KEY, or the alternate
key `"TABLE"."INDEX"` names. Results of a `SELECT`, and the order `ORDER BY`
may ask for, are in that key's order.

| The WHERE, on the key of reference | How the file is read | Called |
|---|---|---|
| `=` on every part of the key (the parts joined with `AND`), or `IN` with such values | one random READ per value, in key order | **keyed** |
| `=` on the leading parts and, on the next part, `=`, `BETWEEN`, `<`, `<=`, `>`, `>=` or `begins_with`; or any of those on a one-part key | `START` at the range's edge, then `READ NEXT` (or `READ PREVIOUS` for `ORDER BY … DESC`) until the range ends | **range** |
| anything else, or no WHERE | `START` at the first record, `READ NEXT` to the end | **scan** |

Conditions on other fields, and the parts of the WHERE a keyed or range read
does not consume, filter each record read. A `SELECT` through an alternate key
`WITH DUPLICATES` returns every record whose key matches, in the order the
engine reads duplicates (the order they joined). `ORDER BY` may name a field of
the key of reference only; `DESC` reverses the read. An `UPDATE` or `DELETE`
names the table only, and its WHERE must pin **the whole RECORD KEY with `=`**
(or `?`); further conditions decide whether the one record qualifies.

### 5.4 Departures from DynamoDB, and why

Each is a superset or a reported error, never a different answer for a
statement both accept.

1. **A hyphen inside a name is part of the name.** `CUST-ID` is a field;
   subtraction is written with spaces, `QTY - 1`. COBOL names are hyphenated,
   and COBOL itself reads `QTY-1` as a name.
2. **Names and keywords are case-insensitive.** COBOL names are; DynamoDB's
   attribute names are case-sensitive.
3. **The schema is known.** An unknown field, table or index is an error; a
   declared field is never `MISSING`; a Number is never compared with a
   String. DynamoDB, schemaless, answers *false* or returns nothing.
4. **Fixed layout.** `INSERT` fills the fields it does not name with their
   initial values; `REMOVE` resets a field. In DynamoDB the attribute is absent.
5. **Range conditions on the key read by the key.** DynamoDB allows only `=`
   and `IN` on the partition key and ranges on the sort key; here the RECORD
   KEY as a whole behaves like a sort key, so `WHERE CUST-ID BETWEEN ? AND ?`
   is a range read, not a scan.
6. **`ORDER BY` through an alternate key**, by naming the index, and `DESC`
   by reading backward.
7. **Transactions may mix reads and writes**, have no statement limit, and,
   with `AutoCommit` off, join the file's open transaction (§6.4). DynamoDB's
   are reads-or-writes, 100 statements, and durable when they return.
8. **`IN` accepts `(…)` as well as `[…]`.**
9. **No pagination.** The result set is whole and local.
10. **List indexes count from 0**, as PartiQL does (§9 Q1 asks whether COBOL's
    1 should win).

## 6. Requirements (EARS)

### 6.1 Statements and values

- **R1 (ubiquitous):** The system shall execute the four statements of §5.1 —
  `SELECT`, `INSERT`, `UPDATE`, `DELETE` — in the forms listed there, with the
  conditions, functions, operators, literals and `?` parameters of §5.1, on the
  file an `IndexedFile` control holds.
- **R2 (ubiquitous):** The system shall read and write field values as §5.2
  defines them: numeric fields as Numbers with their scale and sign, in every
  usage; alphanumeric, alphabetic, national and UTF-8 fields as Strings without
  trailing spaces; groups as paths; `OCCURS` fields as lists.
- **R3 (ubiquitous):** The system shall bind each `?` to the argument in the
  same position after the statement — a data item, a control property or a
  literal — typed by the data item's PICTURE (a Number for a numeric item, a
  String without trailing spaces otherwise) or by the literal's form; a
  parameter shall never be spliced into the statement text.
- **R4 (event):** When a String value is stored into a field shorter than it,
  the system shall truncate it on the right and set `StatusCode` to `00`, as
  `MOVE` does; when a Number does not fit a field's integer digits, the
  statement shall fail with `ValidationException` and change nothing. A Number
  with more decimals than the field keeps shall be truncated, as `MOVE` does.
- **R5 (ubiquitous):** The system shall write a record through `INSERT` only
  when every part of the RECORD KEY is given; an alternate key's fields may be
  left to their initial values when the key allows duplicates.
- **R6 (constraint):** The system shall reject, with `ValidationException` and
  a message naming the construct and its position, any statement outside §5.1:
  the statements and clauses listed under Non-goals, a literal of a kind §5.2
  cannot hold, `list_append` / `set_add` / `set_delete`, `EXISTS` outside a
  transaction, a `?` where a name is expected, a parameter count that differs
  from the `?` count, a comparison between a Number and a String, `ORDER BY` a
  field that is not part of the key of reference, an `UPDATE` or `DELETE`
  whose WHERE does not pin the whole RECORD KEY with `=`, `SET` on a field of
  the RECORD KEY, and a name that is not a field, a key or the control's file.

### 6.2 Execution

- **R7 (ubiquitous):** A statement shall run on the file the control holds —
  the `.cidx` its `IndexedFile` property names — and shall refuse, with
  `ResourceNotFoundException`, a table name that is not that file's (§5.1).
- **R8 (event):** When a statement runs and the file is not open, the system
  shall open it as `AutoOpen` would (in `OpenMode`, with `OperatorName` as the
  registered user) and leave it open; a file opened this way shall be closed
  when the form closes, as an `AutoOpen` file is.
- **R9 (state):** While the form's program has the same file open through its
  own `FD` (the hand-declared `SELECT` / `FD … IS GLOBAL` of today), statements
  and COBOL verbs shall share **one** open file — one position, one set of
  record locks, one undo journal — so that a `COMMIT` verb commits a PartiQL
  write and a `ROLLBACK` verb undoes one (with `AutoCommit` off, R24).
- **R10 (ubiquitous):** A form may use an `IndexedFile` control through
  PartiQL and bindings **without** declaring the file in its COBOL structure:
  such a form shall check, run and build, and the control shall hold the open
  file itself. (Today the facade's `OPEN <file>` makes the hand-written
  `SELECT` / `FD` mandatory; §9 Q4.)
- **R11 (ubiquitous):** The system shall read the file as §5.3 classifies the
  statement — keyed, range or scan — and shall return a `SELECT`'s records in
  the order of the key of reference, reversed for `ORDER BY … DESC`.
- **R12 (ubiquitous):** A `SELECT` through an alternate key `WITH DUPLICATES`
  shall return every matching record; an `UPDATE` or `DELETE` shall act on
  exactly one record, the one the RECORD KEY in its WHERE names.
- **R13 (event):** When a `SELECT` matches no record, the statement shall
  succeed with an empty result set (`StatusCode` `00`, `ResultCount` 0).
- **R14 (event):** When an `INSERT` names a RECORD KEY that exists, or
  violates an alternate key declared without duplicates, the statement shall
  fail with `StatusCode` `22` and `LastError` `DuplicateItemException`, and
  change nothing.
- **R15 (event):** When an `UPDATE`'s WHERE matches no record — no record has
  the key, or one has and a further condition is false — the statement shall
  fail with `StatusCode` `23` and `LastError` `ConditionalCheckFailedException`,
  and change nothing. When a `DELETE` names a key no record has, it shall
  succeed with 0 records affected (DynamoDB's rule); when the record exists and
  a further condition is false, it shall fail as the `UPDATE` does.
- **R16 (event):** When a write statement runs on a file open for `INPUT`, the
  system shall fail it with `StatusCode` `48` (`INSERT`) or `49` (`UPDATE`,
  `DELETE`) and change nothing; the Guide shall say `OpenMode` must be `I-O`.
- **R17 (ubiquitous):** The engine's own outcomes shall pass through
  unchanged: `35`, `37`, `39`, `93` from the open, `90` on an I/O error, `99`
  on a locked record — in `StatusCode`, with the engine's message in
  `LastError`.
- **R18 (ubiquitous):** Statements shall behave identically on every indexed
  engine (`PRCIDXD1`, redb, `STORAGE IS MEMORY`) and in every host: `rcrun
  run-form`, an embedded child form and `run_form_app` in the compiled binary
  (the `interpreter-binary-parity` skill).

### 6.3 The result set

- **R19 (ubiquitous):** A `SELECT`, and an `UPDATE` or `DELETE` with
  `RETURNING`, shall produce the control's **result set**: an ordered list of
  items, each a list of named values in projection order (`*` = every field of
  the record, leaves in record order). A new statement shall replace it. The
  result set shall be the same object the bound controls of §6.6 read.
- **R20 (ubiquitous):** The control shall offer, as `::` methods usable inside
  expressions (`SET`, `IF`, `MOVE`, as every RustCOBOL method is):
  - `Execute(statement [, param …])` — runs one statement; answers the number
    of items in the result set for a `SELECT` or a `RETURNING`, the number of
    records affected otherwise (1, or 0 for a `DELETE` of a missing record),
    and 0 when the statement failed (then `StatusCode` and `LastError` say
    why).
  - `Fetch()` — answers the next item of the result set as its values
    TAB-separated in projection order, and advances; past the last item it
    answers an empty value and sets `StatusCode` to `10`.
  - `GetField(name-or-position)` — answers one value of the item `Fetch()`
    last delivered, by field name (case-insensitive; a path for a group member
    or an occurrence) or by its position in the projection, counted from 1;
    an unknown name or position answers an empty value.
  - `ColumnNames()` and `ColumnCount()` — the projection's names TAB-separated
    (a computed expression is named by its text), and their count.
  - `ExecuteTransaction(statements [, param …])` — §6.4.
- **R21 (event):** When `Fetch()` delivers an item that was taken from a
  record (a `SELECT` of fields, or a `RETURNING`), and the form's program has
  the file's record area (R9) or the control's `CurrentRecordDataItem` is set,
  the system shall also place **that record** there, as a `READ` would, so the
  handler reads the fields by their COBOL names; a projection of computed
  expressions only shall leave them unchanged.
- **R22 (ubiquitous):** The control shall publish the run-time properties
  `ResultCount` (items in the result set), `StatusCode` (R27) and `LastError`
  (R27), readable as `IXF-1::ResultCount` and the like, as the `SqlDatabase`
  control publishes the same three; and `StatusDataItem`, when set, shall
  receive `StatusCode` after every statement, as it receives the facade's file
  status today.

### 6.4 Transactions

- **R23 (ubiquitous):** `ExecuteTransaction(statements [, param …])` shall run
  the statements in `statements` — separated by `;` — in order, as one
  all-or-nothing unit on the file's undo journal: `?` parameters are bound
  across the statements in order; `EXISTS(SELECT …)` is allowed and is a
  condition check, failing the unit with `ConditionalCheckFailedException`
  when no record has the key or the condition is false; the items of every
  `SELECT` and `RETURNING` in the unit form the result set, in order. It shall
  answer the number of statements that ran, and 0 when the unit was cancelled.
- **R24 (ubiquitous):** The control shall gain the design-time property
  **`AutoCommit`** (default `true`, as the DataGrid's). While it is on, every
  write statement and every completed unit shall be committed to the file as
  it completes (durable, as DynamoDB's are). While it is off, writes shall
  join the file's open transaction and become durable at the program's
  `COMMIT` verb or the file's close, and the `ROLLBACK` verb shall undo them.
- **R25 (event):** When a statement of a unit fails, the system shall undo
  **exactly the unit's own changes** — work done before the unit and not yet
  committed stays as it was — set `StatusCode` and `LastError` as R27's last
  row says, and leave the result set empty.
- **R26 (constraint):** A unit shall not be nested in another, and a statement
  of a unit shall not act on a record a previous statement of the same unit
  wrote, rewrote or deleted (DynamoDB's rule); either is a `ValidationException`
  before anything runs.

### 6.5 Status and errors

- **R27 (ubiquitous):** After every statement, and after every `Fetch()`, the
  system shall set `StatusCode` — a two-character FILE STATUS — and
  `LastError`, as follows; `LastError` is empty on success.

  | Outcome | `StatusCode` | `LastError` |
  |---|---|---|
  | the statement ran | `00` | — |
  | `Fetch()` past the last item | `10` | — |
  | `INSERT` of an existing RECORD KEY, or of a duplicate on an alternate key declared without duplicates | `22` | `DuplicateItemException` |
  | `UPDATE` with no qualifying record; `DELETE` of an existing record whose further condition is false | `23` | `ConditionalCheckFailedException`, naming the condition |
  | the statement is not valid (R6) | `94` | `ValidationException: <what and where>` |
  | the table or index named is not the control's file or one of its keys | `94` | `ResourceNotFoundException: <name>` |
  | a write on a file open `INPUT` | `48` / `49` | the file-status text |
  | the open or the I/O failed | `35` `37` `39` `90` `93` `99` | the engine's message |
  | a transaction unit was cancelled (R25) | the failing statement's code | `TransactionCanceledException: statement <n>: <its LastError>` |

  `94` is this feature's own code in the implementor-defined class 9 (the
  runtime uses `90`, `92`, `93`, `99`; the grid's `CommitStatus` uses `95`,
  `96`, `98`); it shall be added to the status table in `indexed.rs`.
- **R28 (constraint):** A failed statement shall never stop the program by
  itself: execution continues with the status set, as it does after a failed
  `READ`. The control's declared `onError` shall not be raised by a
  synchronous statement (nothing raises it today; §9 Q12).
- **R29 (constraint):** `LastError` shall never hold a parameter's value; it
  names fields, positions and conditions, not data.

### 6.6 Data-bound controls

- **R30 (ubiquitous):** A form's data bindings shall offer a **PartiQL**
  source, beside IndexedFile, SQL, COBOL table and REST. It names: the
  **source control** — an `IndexedFile` control of the same form; the
  **query** — a `SELECT` (§5.1), or empty (R34); the **parameters** — an
  ordered list of data items and control properties (`WS-CUST-ID`,
  `DG-CUSTOMERS::SelectedValue`, `TXT-FROM::Text`), one per `?`; the
  **fields** — the projection's names with their PICTUREs and types from the
  `.cidx` (`*` = every field), from which the target's mappings are drawn; and
  **writable** (R36).
- **R31 (ubiquitous):** Every approved binding target shall fill from the
  result set through the mapping paths it has today: a **DataGrid**'s columns;
  a **ListBox** / **ComboBox**'s display field, and a value field (R35); a
  **chart**'s category, value series and series labels; a **Knob**, **Gauge**
  or **Switch**'s value, from the first item; a **Maps** control's markers
  (latitude, longitude, label, info fields); a **repeating GroupBox**'s cards,
  one per item, each member property from its mapped field. Values shall be
  shown as §5.2 reads them: a Number with its decimal point and sign, a String
  without trailing spaces.
- **R32 (event):** When the form populates its bindings (after `onLoad`, where
  the generated `COBOL-DATA-BINDINGS-POPULATE` runs today), and whenever the
  program calls the target's `RefreshBinding()`, the system shall evaluate the
  parameters, have the source control execute the query, and fill the target
  from the result set; `RefreshBinding()` shall answer the number of items.
- **R33 (event):** When the query fails (R6, R17) or a parameter is missing,
  the target shall keep what it shows, and the failure shall be reported in
  the source control's `StatusCode` / `LastError` and on the debug channel;
  nothing shall stop the form.
- **R34 (state):** While a PartiQL binding's query is empty, the target shall
  **follow the source control's result set**: after every `Execute` or
  `ExecuteTransaction` on that control that produces a result set, the target
  shall fill from it (the control's own `Fetch()` cursor unaffected), and at
  populate time it shall show the result set the control holds, if any.
- **R35 (ubiquitous):** A ListBox or ComboBox bound to a PartiQL source shall
  map a display field to its items and may map a **value field**; the list
  shall publish the run-time property **`SelectedValue`** — the value field of
  the selected item, empty when none is selected or no value field is mapped
  (today's `ListValue` mapping, which nothing reads, becomes this; §9 Q8).
- **R36 (ubiquitous):** A **writable** PartiQL binding (a DataGrid or a
  repeating GroupBox) shall save its edits — changed cells, added rows,
  deleted rows — **through the source control**, as one all-or-nothing unit
  (R23 semantics), identifying each record by the RECORD KEY, which the
  projection must therefore include in full; the outcome shall reach the
  target's `CommitStatus`, `LastError`, `onCommitError` and `onSaved` exactly
  as the existing indexed write-back does (`00` written, `23` the record is
  gone, `96` the record changed since it was loaded, `21` a key cell edited,
  `91` not writable, `98` pending). A key cell shall never be editable.
- **R37 (ubiquitous):** The bound target shall publish the run-time property
  **`BindingQuery`**, readable and writable: writing a `SELECT` to it and
  calling `RefreshBinding()` fills the target from the new statement with the
  binding's parameters; a statement that is not a `SELECT` is refused with
  `ValidationException` and the previous query kept. A query written at run
  time is dynamic text and is validated at run time only (R38).
- **R38 (ubiquitous):** The binding guardian shall report, for a PartiQL
  binding: a blocker **`missing-source`** when the source control is empty or
  is not an `IndexedFile` control of the form; a blocker **`invalid-partiql`**
  when the query does not parse, is not a `SELECT`, or names a table, index or
  field the source control's `.cidx` does not have (R6); a blocker
  **`missing-row-identity`** when the binding is writable and the projection
  lacks a part of the RECORD KEY; a blocker **`missing-parameter`** when the
  `?` count and the parameter list differ, or a parameter names no data item
  or control property of the form; and a warning **`full-scan`** when the
  query's WHERE does not use the key of reference (§5.3). The guardian shall
  therefore load the `.cidx` the source control names.
- **R39 (ubiquitous):** The binding editor shall gain the PartiQL source: a
  picker of **the form's `IndexedFile` controls** (the real list, never a
  stand-in), a multi-line **Query** box, a **Parameters** list (add, remove,
  reorder; each a picker of the form's data items and control properties),
  the **Fields** grid seeded **from the projection against the `.cidx`** (real
  names and PICTUREs), the **Writable** switch with its "what the target still
  needs" list, and a **Test query** button that runs the `SELECT` read-only
  against the data file with the parameters' current design-time values left
  as `?` prompts, and shows up to 50 items, the item count, the read class
  (keyed / range / scan) and any error.
- **R40 (ubiquitous):** The generated form program shall seed a PartiQL
  binding as the existing kinds are seeded — the hidden `_Binding*` properties
  followed by `RefreshBinding` in `COBOL-DATA-BINDINGS-POPULATE` — with no
  COBOL read loop; the `.cfrm` shall carry the binding inside `<DataBindings>`
  as today, and forms saved before this feature shall load unchanged.
- **R41 (ubiquitous):** A PartiQL binding shall be a **third** way to bind an
  indexed file: the existing `IndexedFile` source and the `DataSource`
  quick-bind shall keep their behaviour and their generated code unchanged.

### 6.7 Tools: Check, editor, debugger

- **R42 (ubiquitous):** Check (`rcrun check`, the IDE's Check, the
  coding-agent `check` tool) shall validate every statement written as a
  **literal** argument of `Execute` / `ExecuteTransaction`, and every binding
  query, against the control's `.cidx`, without opening the data file: every
  R6 error, with the handler, line and position; and shall report, as an
  informational note, each statement that is a **scan** (§5.3). A statement
  held in a data item is validated at run time only.
- **R43 (ubiquitous):** The COBOL editor's autocomplete shall offer the
  `IndexedFile` control's methods (R20) and properties (R22, R24, R37 on
  targets) with descriptions in the six languages, replacing the visual
  methods it offers the control today; the parser's known-method vocabulary
  (`cobolt_ast::methods::is_known_method` and the interpreter's copy) shall
  know the new names, so `IXF-1::Execute(…)` is a method call, not a
  subscript.
- **R44 (ubiquitous):** The debugger shall step over a statement call as one
  line and, after it, show the statement text, the bound parameter values,
  the read class, `StatusCode`, `LastError` and `ResultCount`; for a bound
  control's refresh, the same on the debug channel.

### 6.8 Documentation, knowledge base, agents, strings

- **R45 (ubiquitous):** Every user-facing string this feature adds to the IDE
  — the PartiQL source and its editor (R39), the guardian's new findings and
  repairs (R38), the autocomplete and property descriptions (R43) — shall be
  translated in the six languages (`Tr` fields; `ide_text_data.rs` entries
  for descriptions). Check diagnostics, `LastError` texts and the DynamoDB
  exception names stay English.
- **R46 (ubiquitous):** The System KB's `cobolt-compiler` tables shall
  describe the `IndexedFile` control's new methods, properties and statuses,
  and its "no inline methods" text (`lib.rs:4866`, `:8155-8165`, `:9351-9357`)
  shall be rewritten to the truth; `assets/knowledge/chunked.data` shall be
  regenerated in the same change. The coding-agent reference
  (`cobolt-project-tools`: the `powerrustcobol-define-indexed-file` skill, the
  `grid-bound-to-indexed-file` pattern, the application rules) and the agent
  prompts that today name methods the control does not have (`llm.rs`,
  `rustcobol-extensions.md`) shall say what this spec ships, and nothing else.
- **R47 (ubiquitous):** The Developer's Guide shall gain, in §14 *Indexed
  files*, a section **"PartiQL — querying an indexed file"** written for the
  PowerCOBOL / isCOBOL developer: the mapping of §5.2, the statements and
  functions with original examples, the read classes of §5.3 with a
  mermaid diagram of how a statement becomes `START` / `READ`, the departures
  of §5.4, the control's members and the status table (R27), `AutoCommit`
  and transactions, and the PartiQL binding with a master-detail example and
  a `SelectedValue` example; §8's `IndexedFile` paragraph and *Data binding
  and the Guardian* shall be updated; `docs/cobol-support-matrix-en.md` shall
  gain rows for PartiQL statements, transactions and the PartiQL binding
  (PowerRustCOBOL extension). Per GOLDEN RULE #8, the five translations of
  every English document this changes shall be deleted in the same change.

### 6.9 Tests

- **R48 (ubiquitous):** The executor shall be covered in `cobolt-runtime` by
  tests over a 100,000-record file with a split RECORD KEY and an alternate
  key with duplicates, that exercise every statement form of §5.1, every
  function and operator, every row of R27, the three read classes with the
  same results as a `START` / `READ` loop, `ORDER BY … DESC`, duplicates,
  `?` typing, transactions (success, condition failure, mid-unit failure with
  R25's exactness, `AutoCommit` on and off against the `COMMIT` / `ROLLBACK`
  verbs), and the three engines; and shall report per GOLDEN RULE #7 a final
  block naming each form exercised, with elapsed time and throughput
  (statements per second, records per second) for keyed, range and scan reads,
  inserts, updates, deletes and transaction units.
- **R49 (ubiquitous):** The binding shall be covered by: codegen tests pinning
  the seeded properties (as `grid_write_back_seed.rs` pins the SQL kind); a
  guardian test per R38 finding; form-runtime tests for each target kind of
  R31 filling from one result set, for R32–R34 refresh moments, for R35, R36
  and R37; and parity tests that the same form gives the same rows under
  `rcrun run-form`, as an embedded child form and in a built binary (R18).

## 7. Acceptance criteria

The data for the criteria: `CUSTOMERS` (RECORD KEY `CUST-ID PIC 9(8)`;
alternate `CUST-NAME PIC X(40)` WITH DUPLICATES; `CUST-BALANCE PIC S9(6)V99`;
a group `BILLING` with `CITY PIC X(20)`) and `ORDERS` (RECORD KEY `CUST-ID` +
`ORDER-NO PIC 9(6)`, a split key; alternate `BY-DATE` on `ORDER-DATE PIC X(10)`
WITH DUPLICATES; `ORDER-TOTAL PIC 9(7)V99`; `ITEM-QTY PIC 9(3) OCCURS 5`),
each loaded with 100,000 records by the test itself. Every new test prints the
GOLDEN RULE #7 result block.

- [ ] **AC1 (R1–R3, R11–R13):** On `CUSTOMERS`, `SELECT CUST-ID, CUST-NAME
  FROM CUSTOMERS WHERE CUST-ID = ?` with a `PIC 9(8)` parameter returns one
  item by a keyed read; `WHERE CUST-ID BETWEEN ? AND ?` returns the same
  records, in the same order, as a `START … KEY NOT LESS THAN` / `READ NEXT`
  loop; `WHERE CUST-BALANCE > 1000.50` scans and returns exactly the records
  the loop with an `IF` returns; `WHERE CUST-NAME = 'Ana'` through
  `"CUSTOMERS"."CUST-NAME"` returns every duplicate; a `SELECT` matching
  nothing answers 0 with `StatusCode` `00`. Check classifies the four
  statements keyed, range, scan, keyed.
- [ ] **AC2 (R2, R4, R19–R22):** `Fetch()` on `SELECT CUST-ID, CUST-BALANCE,
  BILLING.CITY FROM CUSTOMERS WHERE CUST-ID = 42` answers
  `00000042<TAB>-123.45<TAB>Recife` for a record holding `0012345}` in
  `CUST-BALANCE` and `Recife` padded to 20 in `CITY`; `GetField("CUST-BALANCE")`
  and `GetField(2)` answer `-123.45`; `ColumnNames()` answers the three names;
  the record area's `CUST-NAME` holds the record's name after the `Fetch()`
  although it was not projected; a second `Fetch()` answers empty with
  `StatusCode` `10`; `ResultCount` is 1; a second `Execute` of a three-record `SELECT` replaces the result set and `ResultCount` is 3.
- [ ] **AC3 (R5, R14–R17, R27):** `INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?,
  'CUST-NAME': 'Ana'}` writes a record whose `CUST-BALANCE` is zero and `CITY`
  spaces; the same statement again answers 0 with `22` /
  `DuplicateItemException`; `UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE +
  ? WHERE CUST-ID = ? AND CUST-BALANCE >= ?` with a failing third parameter
  answers 0 with `23` / `ConditionalCheckFailedException` and changes nothing;
  `DELETE FROM CUSTOMERS WHERE CUST-ID = 99999999` (absent) answers 0 with
  `00`; on a control whose `OpenMode` is `INPUT` the `INSERT` answers 0 with
  `48` and the `UPDATE` with `49`; a `SELECT` on a control whose `OpenMode` is `INPUT` and whose data file does not exist answers 0 with `35` and the engine's message in `LastError`.
- [ ] **AC4 (R6, R27–R29):** One test runs each of these and gets `94` with a
  `LastError` that names the construct: `SELECT COUNT(*) FROM CUSTOMERS`;
  `SELECT * FROM CUSTOMERS LIMIT 10`; `… WHERE CUST-BALANCE = 'ten'`; `…
  ORDER BY CUST-BALANCE`; `UPDATE CUSTOMERS SET CUST-NAME = 'x' WHERE
  CUST-NAME = 'Ana'` (key not pinned); `UPDATE … SET CUST-ID = 1 WHERE CUST-ID
  = 2`; `… WHERE NO-SUCH-FIELD = 1`; `SELECT * FROM INVOICES` (`ResourceNotFound…`);
  `INSERT … VALUE {'CUST-NAME': 'x'}` (key missing); `SET ITEM-QTY =
  list_append(ITEM-QTY, [1])`; `EXISTS(…)` in `Execute`; a statement with two
  `?` and one argument; and `SELECT * FROM CUSTOMERS WHERE CUST-ID IS MISSING`
  answers 0 items with `00` (never missing). The program runs on after every failure and no `onError` handler is called; a type-mismatch failure whose `?` was bound to the value `SECRET-42` leaves no `SECRET-42` in `LastError`.
- [ ] **AC5 (R2, §5.2, §5.4):** On `ORDERS`, `SELECT ITEM-QTY[0], size(ITEM-QTY)
  FROM ORDERS WHERE CUST-ID = ? AND ORDER-NO = ?` answers the first
  occurrence and `5`; `… WHERE begins_with(ORDER-DATE, '2026-10')` through
  `"ORDERS"."BY-DATE"` is a range read; `WHERE CUST-ID = ? AND ORDER-NO
  BETWEEN ? AND ?` is a range read on the split key; `WHERE CUST-ID = ?
  ORDER BY ORDER-NO DESC` returns that customer's orders descending; `WHERE
  contains(CUST-NAME, 'na')` and `attribute_type(CUST-BALANCE, 'N')` filter as
  §5.2 says; `SET ORDER-TOTAL = ORDER-TOTAL - 10` and `QTY-1`-style names
  parse as a field and a subtraction respectively; `REMOVE ITEM-QTY[2]` zeroes
  the third occurrence and leaves the others.
- [ ] **AC6 (R23–R26):** `ExecuteTransaction("EXISTS(SELECT * FROM CUSTOMERS
  WHERE CUST-ID = ? AND CUST-BALANCE >= ?); UPDATE CUSTOMERS SET CUST-BALANCE =
  CUST-BALANCE - ? WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE =
  CUST-BALANCE + ? WHERE CUST-ID = ?" …)` moves the amount when the condition
  holds and answers 3; with an insufficient balance it answers 0 with `23` and
  `TransactionCanceledException: statement 1: …` and neither record changes;
  a unit whose third statement names an absent key leaves the first two undone
  while a `WRITE` done before the unit (uncommitted, `AutoCommit` off) is still
  there and is undone by the program's `ROLLBACK` verb; with `AutoCommit` on,
  a successful unit survives a `ROLLBACK` verb that follows it; a unit that
  updates the same record twice is refused with `94` before anything runs.
- [ ] **AC7 (R7–R10):** A form whose `IndexedFile` control names
  `indexed/customers.cidx` and declares no `SELECT` / `FD` checks, runs and
  builds; its handler's `Execute` opens the file in `OpenMode` and the file is
  closed when the form closes (the data file's lock sidecar is released). A
  second form that declares `FD CUSTOMERS IS GLOBAL` and `OPEN I-O` in
  `onLoad` runs `WRITE` with the verb and `SELECT` with the control and sees
  the written record; with `AutoCommit` off, the `ROLLBACK` verb undoes an
  `INSERT` the control ran. `Execute("SELECT * FROM ORDERS")` on the
  `CUSTOMERS` control answers 0 with `94` / `ResourceNotFoundException`.
- [ ] **AC8 (R18, R49):** The AC1, AC3 and AC6 programs give identical results
  under `rcrun run-form`, as an embedded child form and in a built binary, on
  the `PRCIDXD1` and redb engines and on a `STORAGE IS MEMORY` file.
- [ ] **AC9 (R30–R32, R40, R41):** A form binds a DataGrid to the `ORDERS` control
  with `SELECT ORDER-NO, ORDER-DATE, ORDER-TOTAL FROM ORDERS WHERE CUST-ID = ?`
  and the parameter `DG-CUSTOMERS::SelectedValue`; at form open the grid shows
  the first customer's orders with `ORDER-TOTAL` as `1234.50`; selecting
  another customer and calling `DG-ORDERS::RefreshBinding()` shows that
  customer's orders and answers their count; the generated program seeds the
  binding with `_Binding*` properties and one `RefreshBinding`, and contains
  no `READ`. A `.cfrm` saved by 1.91.223 loads unchanged, and the quick-bind form `examples/PowerDemo3/forms/Data/datagrid-form.cfrm` still generates byte-identical COBOL (its corpus golden is unchanged).
- [ ] **AC10 (R31):** One result set, `SELECT CUST-ID, CUST-NAME, CUST-BALANCE,
  BILLING.CITY FROM CUSTOMERS WHERE CUST-BALANCE > ?`, fills in one form: a
  DataGrid (four columns), a ListBox (display `CUST-NAME`, value `CUST-ID`), a
  Bar chart (category `CUST-NAME`, series `CUST-BALANCE`), a Gauge (first
  item's `CUST-BALANCE`), a Maps control (a `LAT` / `LNG` pair added to the
  record for the test), and a repeating GroupBox whose cards show `CUST-NAME`
  in a Label and `CUST-BALANCE` in a TextBox — each with the values §5.2
  defines, the Number with its decimal point.
- [ ] **AC11 (R34):** A grid bound to the `CUSTOMERS` control with an empty
  query is empty at form open; a button's `IXF-CUST::Execute("SELECT * FROM
  CUSTOMERS WHERE begins_with(CUST-NAME, ?)" TXT-SEARCH::Text)` fills it with
  the matches, and the handler's own `Fetch()` loop afterwards still reads
  every item from the first.
- [ ] **AC12 (R35):** A ComboBox bound to `SELECT CUST-ID, CUST-NAME FROM
  CUSTOMERS` with value `CUST-ID` shows the names; after the operator picks the
  third, `CMB-CUST::SelectedValue` answers that record's `CUST-ID`, and is
  empty before any pick.
- [ ] **AC13 (R36):** A writable grid bound to `SELECT CUST-ID, CUST-NAME,
  CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID BETWEEN 1 AND 50` with
  `AllowCellEditing` on: editing two balances, adding a row and deleting a row,
  then `Save()`, writes all four as one unit (`CommitStatus` `00`, `onSaved`),
  visible to a `SELECT` on another control; a balance changed on disk between
  load and save makes `Save()` fail with `96` and nothing written; the
  `CUST-ID` cells are not editable; a binding whose projection lacks
  `CUST-ID` is read-only and the guardian says `missing-row-identity`.
- [ ] **AC14 (R37, R33):** `MOVE "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS
  WHERE CUST-BALANCE < 0" TO DG-1::BindingQuery` followed by
  `DG-1::RefreshBinding()` shows the debtors; writing `DELETE FROM CUSTOMERS
  WHERE CUST-ID = 1` to it is refused with `94` and the previous query stays;
  a refresh whose parameter item holds a non-numeric value for a `PIC 9`
  field leaves the grid's rows as they were and sets the control's
  `StatusCode` to `94`.
- [ ] **AC15 (R38, R39, R42, R45):** The guardian reports, on one form: a
  PartiQL binding naming a Label as source (`missing-source`); one whose query
  is an `UPDATE` (`invalid-partiql`); one naming `NO-SUCH-FIELD`
  (`invalid-partiql`); a writable one projecting `CUST-NAME` only
  (`missing-row-identity`); one with two `?` and one parameter
  (`missing-parameter`); and one whose WHERE is on `CUST-BALANCE` (`full-scan`
  warning). The binding editor lists the form's two `IndexedFile` controls and
  no stand-in, seeds the Fields grid from the projection with the `.cidx`'s
  PICTUREs, and Test query shows the first 50 items and "range" for AC1's
  `BETWEEN` statement. Check reports each AC4 statement, written as a literal
  in a handler, at its line, with no data file present. Every new string
  passes the six-language test.
- [ ] **AC16 (R43, R44):** Autocomplete after `IXF-CUST::` offers `Execute`,
  `ExecuteTransaction`, `Fetch`, `GetField`, `ColumnNames`, `ColumnCount`,
  `ResultCount`, `StatusCode`, `LastError`, `AutoCommit` with descriptions in
  the selected language and offers no `Click` or `Show`; `IXF-CUST::Execute(
  WS-STMT )` parses as a method call. Stepping over an `Execute` in the
  debugger shows the statement, its parameter values, the read class, the
  status, the error text and the count.
- [ ] **AC17 (R46, R47):** `cargo run -p cobolt-ide --example build_chunked_kb`
  leaves `prebuilt_chunked_kb_matches_the_published_documentation` green; the
  System KB's IndexedFile page, the plugin skill and the agent prompts contain
  no "no `::` methods" and no `::ReadNext`; the Guide section exists with its
  diagram and examples; the support matrix has the new rows; no
  `developers-guide-<lang>.md` or `cobol-support-matrix-<lang>.md` remains.
- [ ] **AC18 (R48):** The runtime test's result block reports, for 100,000
  records on `PRCIDXD1`: statements per second for keyed reads, records per
  second for the range and scan reads, inserts, updates, deletes and units,
  with every number produced by the run (verify-first).

## 8. Constraints & steering check

- **i18n (6 languages):** yes — the PartiQL source label and editor, Test
  query's texts, the guardian's findings and repair labels, the autocomplete
  and property descriptions (`Tr` fields and `ide_text_data.rs` entries, R45).
  Statement text, field names, exception names and Check diagnostics stay
  English.
- **Generated-code / regenerate contract:** codegen seeds the binding and
  invokes `RefreshBinding` as for the existing kinds (R40); it generates no
  PartiQL and no read loop; the banner and regenerate-on-action contract are
  untouched. The facade paragraphs stay as they are; R10 asks only that a
  form without a hand-declared `FD` still check and build, which touches how
  the facade's verbs are emitted for such a form — the plan decides how.
- **Form file:** one new binding source inside `<DataBindings>` (JSON, serde
  default) and two new properties: `AutoCommit` on `IndexedFile`, `BindingQuery`
  on bound targets (run-time). Old `.cfrm` files load unchanged (R40).
- **The control's first `::` methods:** the run-time dispatch needs an `IndexedFile`
  arm before the shared `match` (today `::Open()` lands in SqlDatabase's), the
  parser's known-method vocabulary gains the names (R43), and the test
  `indexedfile_paragraphs_are_not_reachable_from_a_handler` keeps its claim
  about the paragraphs while a new one asserts the methods are reachable.
- **Three hosts:** `rcrun run-form`, embedded child forms and `run_form_app`
  in the compiled binary (R18, AC8) — the `interpreter-binary-parity` skill.
- **Shared source, two engines of the language:** the executor lives in
  `cobolt-runtime`, beside the indexed engines; `cobolt-decimal` is the
  decimal arithmetic to use for Numbers (R2), so the 2.x.x generator can share
  it when that line takes the feature.
- **System KB:** the `cobolt-compiler` doc tables change (R46), so
  `assets/knowledge/chunked.data` is regenerated in the same change; a red
  `prebuilt_chunked_kb_matches_the_published_documentation` is real.
- **Docs:** the Guide's §14 section, §8 paragraph and binding section, and the
  support matrix (R47); each English document's five translations deleted in
  the same change (GOLDEN RULE #8). `docs/cobol85-supported-syntax-en.md` is
  not touched: a PartiQL statement is the content of a literal, not COBOL
  syntax.
- **Licensing / originality:** the AWS reference was read as a map; no AWS
  example, table or wording is reproduced; every example here and in the Guide
  is original. "PartiQL" and "DynamoDB" are named as the base, as the product
  already names AWS services.
- **What exists and the plan must respect:** the binding runs today through
  its own `GridSession` with an I-O open of its own; a PartiQL binding runs
  through the source control's open file instead (R9, R36), so two opens of
  one file in one process are avoided — the redb engine takes one exclusive
  writer per container. The `.cidx` parsed PICTURE lacks the scale (§2); R2
  needs it, so the executor takes the scale from the PICTURE string.
- **Numbers shown with their decimal point** (R31) differ from today's
  indexed quick-bind display, which shows `PIC 9(6)V99` as digits only; this
  spec does not change the old path (R41) — §9 Q13 asks whether a separate fix
  should.
- **2.x.x:** the executor is runtime code, so the backend of the WebAssembly
  line gets it with the cherry-pick when the operator takes this feature
  there; the browser's in-memory engine (`cobolt-wasm-rt::idx`) does not, and
  a Web or Mobile project using `Execute` on an in-memory file would need a
  WASM-PARITY row and its own work. Nothing in this spec touches `README.md`
  or `docs/` on `2.x.x`.
- **Classification precedents:** none applies in the other direction — a
  query language and the control's first `::` methods are new capabilities, not
  debt (`CLAUDE.md`, Classification precedents).
- **Fix vs feature:** **feature.** `features` branch; `z` bump per change; the
  defects of §2 ("Defects met on the way") are fixes for `fixes`, in their own
  commits, never mixed in.

## 9. Open questions

Each carries a recommendation; a "yes" to the recommendation settles it.

- **Q1 — List indexes: PartiQL's 0 or COBOL's 1?** `ITEM-QTY[0]` is the first
  occurrence as DynamoDB counts; a COBOL developer writes `ITEM-QTY(1)`.
  *Recommendation:* keep 0, so a statement written for DynamoDB means the same
  here (the stated base); the Guide warns in a caveat. The alternative is
  accepting `ITEM-QTY(1)` with parentheses as the 1-based spelling beside
  `[0]` — two spellings for one thing.
- **Q2 — Fixed layout semantics** (§5.4 items 3–4): `INSERT` fills unnamed
  fields with initial values, `REMOVE` resets, declared fields are never
  `MISSING`, unknown names are errors. *Recommendation:* as written.
- **Q3 — `AutoCommit` default `true`** (R24): each statement durable as it
  completes, like DynamoDB and like a grid's `AutoCommit`; off, the COBOL
  verbs govern. *Recommendation:* `true`.
- **Q4 — A form without a hand-declared `SELECT` / `FD`** (R10) may use the
  control through PartiQL and bindings alone. *Recommendation:* yes — the
  binding story is otherwise not no-code, which today's quick-bind is.
- **Q5 — A batch** (`BatchExecuteStatement`): independent statements, each
  with its own status. *Recommendation:* no (a `PERFORM` loop); revisit if the
  operator wants it for fidelity.
- **Q6 — A built-in for programs without forms:** `COBOL::"PARTIQL" (
  FILE-NAME STATEMENT RESULT-COUNT STATUS [param …] )` with `COBOL::"PARTIQL-FETCH"`,
  so a plain `.cbl` under `tests/cobol/**` can exercise the executor with
  `rcrun run` and report per GOLDEN RULE #7. *Recommendation:* yes, in this
  spec — it is a thin wrapper over the executor R48 needs anyway; otherwise
  the executor's tests are Rust and form-driven only.
- **Q7 — The IDE's Grid Browser and the agents' search tool:** a PartiQL
  `WHERE` box in the Grid Browser; `kb_search`-style PartiQL for the AI search
  tool. *Recommendation:* separate specs after this one ships.
- **Q8 — `SelectedValue` on lists** (R35): the value field of the selected
  item. *Recommendation:* yes; it is what makes a PartiQL-bound ComboBox
  useful, and it gives the dormant `ListValue` mapping its meaning.
- **Q9 — Transactions may mix reads and writes** (§5.4 item 7).
  *Recommendation:* yes; the DynamoDB rule is an implementation limit, and the
  superset principle allows it.
- **Q10 — A Number compared with a String is an error, not *false*.**
  *Recommendation:* error — Check reports it before the program runs.
- **Q11 — The hyphen rule** (§5.4 item 1). *Recommendation:* as written;
  double quotes remain available for any name.
- **Q12 — The method and property names** — `Execute`, `ExecuteTransaction`,
  `Fetch`, `GetField`, `ColumnNames`, `ColumnCount`, `ResultCount`,
  `StatusCode`, `LastError`, `AutoCommit`, `BindingQuery`, `SelectedValue`,
  and `94` as the statement-invalid status. The first six follow the
  `SqlDatabase` and `AwsDynamoDB` controls; `onError` stays unraised (R28).
  *Recommendation:* as written; renaming is cheap now and expensive after the
  Guide, the KB and the agents learn them.
- **Q13 — Numbers with their decimal point in bound controls** (R31) while the
  existing indexed quick-bind shows digits only. *Recommendation:* ship R31 as
  written and open a fix on `fixes` for the old path, so the two agree.
