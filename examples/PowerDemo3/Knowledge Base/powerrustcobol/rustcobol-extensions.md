<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Extensions & Syntax

PowerRustCOBOL extends COBOL-85 with inline RAD Form and UI Control access features:

## Property Get & Set Syntax
- **Retrieve a property**: Use `<control>::<property>`.
- **Set a property**: Use `SET <control>::<property> TO <value>`.
  - Example: `SET SAVE-BUTTON::Caption TO "Save".`
  - Example: `SET MAIN-PANEL::Visible TO 1.`
  - Example: `SET TOTAL-LABEL::ForegroundColor TO "#FF0000".`

## Method Invocation Syntax
- **Invoke a method**: Use `<control>::<method>(<parameters>)`.
  - Example: `LineChart-1::Clear().`
  - Example: `DataGrid-1::RefreshBinding().`
  - Example: `LineChart-1::AddPoint("January", 150).`
  - Example: `SqlDatabase-1::Open("sqlite::memory:").`
  - Example: `RestClient-1::Post("https://api.example.com/api/save", request_body).`
- **DO NOT** use `CALL` or legacy `INVOKE` for UI control properties or methods. Use the inline double-colon (`::`) syntax directly.
- The method vocabulary is **closed** — only the methods listed in the Control Methods Reference exist. A `::name(arg)` with an unrecognised name is treated as a PROPERTY WRITE of `name`, not a method call, so inventing a method silently does nothing useful.
- **IndexedFile controls have no `::` methods**, and their generated helpers (`<id>-OPEN`, `<id>-READ-NEXT`, …) are paragraphs of the OUTER program. A handler is a nested program, so it cannot `PERFORM` them: the compiler rejects it with "'<id>-OPEN' is not a paragraph or section of this program". **There is currently no supported way to drive an IndexedFile control from an event handler** — do not emit `PERFORM <id>-OPEN` in a handler and do not invent `<id>::Open()`, which is not a method this platform has. Use `SqlDatabase` (which does have `::` methods) when a handler must reach stored data.
- A method that returns a value can be used inline (`MOVE C::GetText() TO WS-X`) or with `RETURNING`.
- **A METHOD CALL IS A STATEMENT, NEVER A RECEIVING FIELD.** `<control>::<property>` may receive a value; `<control>::<method>(…)` may not. Writing a method call where a receiver belongs raises the runtime error *"'<control>::<method>' is a method call, not a receiving field — call it as a statement instead of using it as a MOVE/assignment target"*. It fails at the click, not at generation, so the handler reads correctly and throws. A method used as a SOURCE is fine (`MOVE C::GetText() TO WS-X`); only the receiving side is closed to it.
- **The usual way this happens is a missing period, not a misunderstanding.** A COBOL sentence runs to its period, so a `::` call written under an unclosed `MOVE`/`SET` becomes that statement's SECOND RECEIVER, however many blank lines separate them:

```cobol
      *> WRONG — the MOVE has no period, so `AddRow(...)` is a second
      *> receiving field of it, and the run raises the exception above.
       MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED

       dgReceipt::AddRow("Total", GLOBAL-TOTAL-ED).

      *> RIGHT — the MOVE is closed; the call is a statement of its own.
       MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED.

       dgReceipt::AddRow("Total", GLOBAL-TOTAL-ED).
```

  Several receiving fields under one `MOVE` remain perfectly legal when every one of them IS a receiver: `MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED  dgReceipt::X.` correctly writes the edited item AND the `X` property. Only a method among the receivers is the defect. The fix is a period on the preceding statement, or the explicit `INVOKE <control> "<method>" USING <parameters>` form, which cannot be read as a receiving field.

## RustCOBOL built-ins — the `COBOL` object
The runtime's built-in calls — HTTP, SQL, files, native dialogs, API keys, the model list, charts, data bindings — are the methods of the **`COBOL` object**, and are written INLINE:

```cobol
           COBOL::"MODEL-LIST" ( WS-PROV-ID WS-CUR-URL WS-KEY WS-N WS-STATUS )
           COBOL::"HTTP-GET" ( WS-URL WS-RESPONSE WS-HTTP-STATUS )
           COBOL::"OPEN-FILE-DIALOG" ( WS-TITLE "Data|idx" WS-PATH )
```

- `COBOL::"NAME"( args )` IS `CALL "COBOL-NAME" USING args` — the same call, the same arguments in the same order. A data item is passed BY REFERENCE, so what the built-in returns lands in it; a literal is passed BY CONTENT. **Write the inline form, never the `CALL`.**
- It is a STATEMENT, not a value: the results come back in the `out` arguments, never as a returned value (`MOVE COBOL::…` is wrong).
- The name may be quoted or bare (`COBOL::HTTP-GET( … )`), and may carry the `COBOL-` prefix; quoted is the house style.
- Arguments are separated by spaces (commas are allowed). `[…]` marks an optional argument, which may be left off the end.
- A name that is not a built-in is reported exactly as an unknown `CALL` is. A common procedure is still reached with `CALL "PROCEDURE-NAME"`.

| Method | Arguments, in order | What it does |
|---|---|---|
| `COBOL::"APPEND-FILE"` | path in, text in, [status out] | Append a line of text to a file, creating it if missing |
| `COBOL::"BINDING-LOAD"` | binding-id in, status out | Load a data binding's records from its source |
| `COBOL::"BINDING-MARK-CLEAN"` | binding-id in, dirty-flag out | Mark a binding clean: no pending edits |
| `COBOL::"BINDING-POPULATE"` | binding-id in, status out | Fill a binding's bound controls from its data |
| `COBOL::"BINDING-SET-PENDING"` | binding-id in, row-key in, value in, dirty-flag out | Record a pending edit for one row of a binding |
| `COBOL::"BINDING-SET-READ-ONLY"` | binding-id in, flag in | Make a binding read-only (flag not "0") or writable |
| `COBOL::"BINDING-UPDATE"` | binding-id in, row-key in, status out | Write a binding row's pending changes back to its source |
| `COBOL::"CHART-ADD-POINT"` | chart-id in, label in, value in, [more values in] | Add one point to a chart |
| `COBOL::"CHART-CLEAR"` | chart-id in | Remove all data from a chart |
| `COBOL::"CHART-REFRESH"` | chart-id in | Repaint a chart from its current data |
| `COBOL::"CHART-SET-TABLE"` | chart-id in, table in, count in | Replace a chart's data with count rows of a table |
| `COBOL::"CLOSE-DB"` | handle in | Close a database connection |
| `COBOL::"EXEC-SQL"` | handle in, sql in, row-count out, status out | Run a SQL statement; returns the row or affected count |
| `COBOL::"FETCH-ROW"` | handle in, column in, value out, status out | Read one column (1-based) of the current result row |
| `COBOL::"FILE-STATUS"` | file-name in, status out | Copy a file's last FILE STATUS code into a data item |
| `COBOL::"FOLDER-DIALOG"` | title in, [start-folder in], path out | Ask the operator for a folder; spaces when cancelled |
| `COBOL::"GET-PROPERTY"` | object in, property in, value out | Read a control's property into a data item |
| `COBOL::"HTTP-CLEAR-HEADERS"` | — | Remove every header set with HTTP-SET-HEADER |
| `COBOL::"HTTP-DELETE"` | url in, response out, http-status out | Send an HTTP DELETE; returns the body and status code |
| `COBOL::"HTTP-GET"` | url in, response out, http-status out | Send an HTTP GET; returns the body and status code |
| `COBOL::"HTTP-POST"` | url in, body in, response out, http-status out | Send an HTTP POST (JSON by default); returns body and status |
| `COBOL::"HTTP-PUT"` | url in, body in, response out, http-status out | Send an HTTP PUT; returns the body and status code |
| `COBOL::"HTTP-SET-HEADER"` | name in, value in | Add or replace a header sent on every later HTTP call |
| `COBOL::"INIT-FORM"` | [form-name in] | Initialise the form (generated code calls it) |
| `COBOL::"KEY-IS-SET"` | entry in, flag out | Whether an API key is stored for a model entry: Y or N |
| `COBOL::"KEY-REMOVE"` | entry in, [status out] | Delete the stored API key of a model entry |
| `COBOL::"KEY-SET"` | entry in, key in, [status out] | Store the API key of a model entry (never read back) |
| `COBOL::"MCP-SEARCH"` | tool in, arguments-json in, result out | Call an MCP tool with JSON arguments; returns its text |
| `COBOL::"MODEL-LIST"` | provider in, endpoint in, key in, count out, status out, [entry in] | Ask a provider which models it offers |
| `COBOL::"MODEL-LIST-GET"` | index in, model out | One model name (1-based) from the last MODEL-LIST |
| `COBOL::"MODEL-REMOVE"` | entry in, [status out] | Remove an entry from the model list |
| `COBOL::"MODEL-SET"` | entry in, api in, url in, model in, [status out] | Add or replace an entry in the model list |
| `COBOL::"MODEL-TEST"` | provider in, endpoint in, model in, key in, [status out], [entry in] | Send a model a tiny request to test the connection |
| `COBOL::"NEXT-ROW"` | handle in, more out | Move to the next result row: Y when there is one, N at the end |
| `COBOL::"OPEN-DB"` | connection-string in, handle out, status out | Open a SQLite, PostgreSQL or MySQL connection |
| `COBOL::"OPEN-FILE-DIALOG"` | title in, [filter in], [start-folder in], path out | Ask the operator for a file to open; spaces when cancelled |
| `COBOL::"PROVIDER-COUNT"` | count out | How many model providers there are |
| `COBOL::"PROVIDER-GET"` | index in, [id out], [label out], [endpoint out], [needs-key out] | One provider's details (1-based) |
| `COBOL::"ROW-COUNT"` | handle in, count out | How many rows the last result set has |
| `COBOL::"SAVE-FILE-DIALOG"` | title in, [filter in], [file-name in], [start-folder in], path out | Ask the operator where to save; spaces when cancelled |
| `COBOL::"SET-PROPERTY"` | object in, property in, value in | Set a control's property |
| `COBOL::"WAIT-EVENT"` | event-id out, control-id out | Wait for the next event (generated code calls it) |
| `COBOL::"WRITE-FILE"` | path in, text in, [status out] | Write a file holding one line of text, replacing it |

## Value Conventions (types and domains)
- **Boolean properties** store `1` (true) / `0` (false). Write `SET C::Visible TO 1`. On method arguments, `true`/`yes`/`on` (any case) also count as true.
- **Colors** are hex strings: `"#RRGGBB"` or `"#RRGGBBAA"` (e.g. `"#FF0000"`, `"#00000000"` = transparent).
- **Coordinates and sizes** (`X`, `Y`, `Width`, `Height`, paddings, radii) are integer pixels.
- **List content** (`Items` of ListBox/ComboBox/ToolBar/StatusBar/TreeView) is ONE ITEM PER LINE (newline-separated); TreeView nests children with two leading spaces per level, and a node may carry up to three TAB-separated fields of its own after its label (`label\\ticon\\tcolour\\tbackground`). Indexes (`SelectedIndex`, grid rows/columns) are 0-based; -1 = no selection. A TreeView node's handle is its **1-based** LINE in `Items` as written — the number a node event hands the handler in `CONTROL-NODE-INDEX` — and it is what every `Node…` method takes and every traversal method returns; `-1` (or `0`) names no node.
- **DataGrid data**: `Columns` is one `Name:Type` per line (`Type` ∈ `string`|`number`|`datetime`); `Rows` separates rows with newlines and cells with TAB.
- **Enumerated properties** accept only their listed values EXACTLY as spelled (e.g. `Orientation` is `Horizontal` or `Vertical`); an unrecognised value falls back to the default without an error.
- **Property names**: setting a misspelled property silently creates a new, unused property — never guess names; use the ones in the Form Controls Reference.
- **Charts**: feed data with `Chart::AddPoint(label, value)` / `Chart::Clear()` / `Chart::Refresh()`, with `PERFORM <id>-ADD-POINT` / `<id>-SET-TABLE` paragraphs, or with `COBOL::"CHART-ADD-POINT"( "<id>" label value )` — or bind a COBOL table via the `DataSource`/`DataCount` properties. Do NOT invent working-storage tables for charts.

## Event payloads — what a handler actually receives (LINKAGE)

Almost every event delivers **nothing**. The generated dispatcher calls a handler as `CALL "<handler-program>"` with no arguments, so the handler's `LINKAGE SECTION` is empty and its header is a plain `PROCEDURE DIVISION.` with no `USING`.

- **There are exactly TWO event payloads in the platform.** The second is `CONTROL-NODE-DATA`, on every TreeView node event (`onNodeClick`, `onNodeSelect`, `onNodeDblClick`/`onNodeDoubleClick`, `onNodeCheck`, `onNodeCollapse`, `onNodeExpand`) — a LINKAGE group of `05 CONTROL-NODE PIC X(256)` (the node's label, the key every TreeView property uses), `05 CONTROL-NODE-INDEX PIC S9(4) COMP-5` (its 1-based line in `Items` as WRITTEN, so `Sorted` cannot renumber it), `05 CONTROL-NODE-LEVEL PIC S9(4) COMP-5` (1-based depth) and `05 CONTROL-NODE-CHECKED PIC 9` (`1` when its box is ticked, `0` when it is not or the tree has no boxes). Such a handler is written `PROCEDURE DIVISION USING CONTROL-NODE-DATA.` and the designer generates it that way. `onNodeDrop` (a tree with `AllowDrag`) carries the same group with two more items: `05 CONTROL-TARGET-INDEX PIC S9(4) COMP-5` (the node the dragged one was dropped on, 0 for the tree's empty space) and `05 CONTROL-TARGET-NODE PIC X(256)` (that node's label). Before 1.61.158 a handler for `onNodeCheck`/`onNodeCollapse`/`onNodeExpand` could not tell WHICH node had moved: those events write no `SelectedNode` and the value was dropped. The first is `CONTROL-ARRAY-INDEX PIC S9(4) COMP-5`, the 1-based index of the card that fired, and ONLY for a control inside a repeating group. That handler is called `USING CONTROL-ARRAY-INDEX` and writes `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.`.
- **No event carries a key code, a mouse button, a coordinate, a modifier or a character.** Do NOT declare an item such as `KEY-CODE` and do NOT write `PROCEDURE DIVISION USING KEY-CODE.` — nothing populates it, and the dispatcher passes no argument to bind it to.
- **A specific key has its own event.** For "do X when the user presses ENTER" bind `onEnterPressed`; for ESC bind `onEscapePressed`. `onKeyDown` / `onKeyUp` / `onKeyPress` fire for ANY key and tell you nothing about which one, so testing a key inside them is impossible.
- To know what the user typed, read the control's own text: `MOVE MY-BOX::Text TO WS-VALUE`. `onTextChanged` (alias `onChange`) fires after each edit.

## Naming rules — control ids and every COBOL word you write
Control ids are not just labels: each one becomes part of a COBOL **user-defined word** in the generated program. A control `SAVE-BTN` gets the storage group `WS-SAVE-BTN` with `WS-SAVE-BTN-TEXT`, `-VISIBLE`, `-ENABLED` (editable controls also get `-VALUE`), and file/database controls get paragraphs such as `SAVE-BTN-OPEN` / `SAVE-BTN-CONNECT`.

A COBOL word may contain **only letters (`A-Z`, `a-z`), digits (`0-9`) and hyphens (`-`)**. It may not begin or end with a hyphen, and a data-name must contain at least one letter.

- **Never put `_` (underscore), `.`, spaces, `/`, `#` or accented characters in a control id.** `TEXTBOX_1` is not a COBOL word; `TEXTBOX-1` is. An underscore is the most common mistake: the lexer reads `WS-TEXTBOX_1-TEXT` as the word `WS-TEXTBOX`, then an error token, then a number — the whole data item is discarded and the control ends up with no storage at all.
- The same rule applies to every name YOU declare: WORKING-STORAGE data items, level-01/05 group and field names, paragraph names, and `CALL`/`PERFORM` targets. Use `WS-ROW-COUNT`, never `ws_row_count`.
- Prefer short, hyphenated, meaningful ids: `CUST-NAME-TXT`, `TOTAL-LBL`, `SAVE-BTN`, `GRID-1`.
- Digits are fine anywhere except as the whole name: `TEXTBOX-1`, `COL-2-HDR`.
- The generator does normalise an invalid id (each character that is not a letter or digit becomes a hyphen, runs collapse, the ends are trimmed), so a legacy `textbox_1` still compiles — as `WS-textbox-1` — but then the id in the designer and the name in the COBOL no longer match. Create valid ids in the first place.

## Event Handler Division Structure
- Every developer-editable event-handler body must start from the program Divisions and contain:
  ```cobol
         ENVIRONMENT DIVISION.
         DATA DIVISION.
         WORKING-STORAGE SECTION.
         *> (Data declarations here)
         PROCEDURE DIVISION.
             *> (Statements here)
  ```
- Do not write `IDENTIFICATION DIVISION`, `PROGRAM-ID`, or `END PROGRAM` in the handler body; the IDE scaffold manages the program wrapper.
- `GOBACK` **is** yours to write, and it is an ordinary statement. The scaffold appends a closing one, but that lands after everything you wrote — so a body that declares its own paragraphs must end its main flow with `GOBACK.` before the first of them, or control falls through and runs that paragraph a second time.

## Nested programs — where `PERFORM` reaches, and where it does not
The generated source is a COBOL-85 **nest**: the form is the outer (main) program, and every event handler and every common procedure is a separate nested program inside it. That structure decides how one piece of code reaches another, and getting it wrong is the most common way a handler that reads correctly still fails.

- `PERFORM` transfers control to a **paragraph or section of the SAME program**. It never crosses a program boundary. A `PERFORM` naming anything outside the body it sits in has no target, and that body is rejected.
- A **common procedure** is a nested program, not a paragraph of yours. Reach it with `CALL "ITS-NAME"` — `CALL "UPDATE-TOTAL".`, `CALL "RECALC" USING WS-QTY WS-PRICE.` — never `PERFORM UPDATE-TOTAL`.
- Use `PERFORM` for paragraphs you declared yourself, inside the body you are writing.
- The generated infrastructure paragraphs (`<id>-OPEN`, `<id>-READ-NEXT`, `<id>-COMMIT`, and the timer, chart, CSV-export and data-binding helpers) are emitted at OUTER program scope. They are `PERFORM`-able from the form's own procedure code, not from inside an event handler, which is a nested program of its own.

## Ownership in a COBOL-85 nest — what each program may declare
Never assume a containing program's declarations are visible to a nested one. Only items declared `GLOBAL` are, and only because they are declared `GLOBAL`.

**The one hard restriction is the `CONFIGURATION SECTION`.** COBOL-85 forbids a contained program from specifying one at all, so `SOURCE-COMPUTER`, `OBJECT-COMPUTER`, `SPECIAL-NAMES` and (this platform's) `REPOSITORY` may appear ONLY in the outermost program — the form — and they govern every program nested inside it.

**Everything else about files and storage is per-program.** A nested program MAY declare its own `INPUT-OUTPUT SECTION`, `FILE-CONTROL`, `SELECT`, `FD`/`SD`, record descriptions, `WORKING-STORAGE` and `LINKAGE`. Those are legitimate in the form OR in a single handler; which is correct depends on intent, not on a rule, so a request that does not say where must be clarified rather than guessed.

| Declaration | Owner | Written by |
| --- | --- | --- |
| `CONFIGURATION SECTION` — `SOURCE-COMPUTER`, `OBJECT-COMPUTER`, `SPECIAL-NAMES`, `REPOSITORY` | outermost program ONLY | `set_form_structure`, or the COBOL Structure panel |
| `INPUT-OUTPUT SECTION`, `FILE-CONTROL`, `SELECT` | each program may own its own | `set_form_structure` for the form's; the body itself for a handler's |
| `FILE SECTION`, `FD`/`SD`, record descriptions | each program may own its own | `set_form_structure` for the form's; the body itself for a handler's |
| `WORKING-STORAGE`, `LINKAGE` | each program owns its own | `set_form_structure` for the form's; the body itself for locals |
| `PROCEDURE DIVISION` | each program owns its own | the handler or common-procedure body |
| `GLOBAL` items | containing program, visible to every nested program | declared in the FORM |
| `EXTERNAL` items | the run unit | the form's `WORKING-STORAGE`, `EXTERNAL` clause |

`GLOBAL` is **not** a working-storage-only clause. It applies to `01`/`77` items in `WORKING-STORAGE` and equally to `FD`/`SD` entries and `01` record descriptions in the `FILE SECTION`. A `GLOBAL FD` with a `GLOBAL` record description is the better pattern for shared file data: every nested program reads the record area directly, with no `MOVE` traffic between the `FD` and working-storage.

`COMMON` is never requested — codegen marks every nested program `IS COMMON PROGRAM`, so a common procedure is always callable by its siblings. `INITIAL` is not emitted by this platform. `LOCAL-STORAGE` and `SCREEN SECTION` are parsed by the compiler but no operation writes them.

Checklist before emitting a change-set: no nested program declares a `CONFIGURATION SECTION` or `SPECIAL-NAMES`; every nested program has its own `DATA DIVISION`; `GLOBAL` items are referenced, never duplicated; `EXTERNAL` items are treated as run-unit-wide; cross-program invocation is `CALL`, never `PERFORM`.

## The ``` block literal — long or multi-line text

COBOL-85 has **no multi-line literal**. Continuation is a fixed-format column
mechanism, so free-format source cannot write one at all, and any literal full
of quotation marks needs every one of them doubled. PowerRustCOBOL adds a
**block literal**: a literal fenced the way a Markdown code block is.

````cobol
       MOVE
```
Un AgentObject es un punto final de modelo configurado.

Cree una clave en www.ollama.com
``` TO Lbl-Sub::Caption.
````

**A block literal has NO quotation marks.** The fences take their place — that
is the whole point of it. Writing both is the mistake to avoid, and it is the
one that actually happens:

````cobol
      *> WRONG — quotes AND fences, all on one line. The fences are then
      *> ordinary characters inside an ordinary quoted literal, the newlines
      *> have nowhere to go, and the caption comes out with ``` in it.
       MOVE "```An AgentObject is a configured model endpoint.

       In order to run this example you need a valid API Key."``` TO Lbl-Sub::Caption.

      *> WRONG — fences on the same line as the text. The opening fence ends
      *> its own line; anything after it is a language tag, not content.
       MOVE ```An AgentObject is a configured model endpoint.``` TO Lbl-Sub::Caption.

      *> RIGHT — no quotes; each fence owns its line; the text is the lines
      *> between them; the statement continues after the closing fence.
       MOVE
```
An AgentObject is a configured model endpoint.

In order to run this example you need a valid API Key.
``` TO Lbl-Sub::Caption.
````

The rules, and they are exact:

- The value is the lines **between** the fences. The opening fence's own line
  is not content (anything after ``` on it is a language tag, as in Markdown),
  and neither is the closing fence's line nor the newline before it.
- **Interior newlines are kept.** That is the entire point.
- **No escaping.** The text is taken verbatim, so quotation marks and
  apostrophes need no doubling — which is what makes it usable for JSON, SQL,
  HTML and paragraphs of prose.
- The closing fence is a line whose first non-blank text is ```.
- It is a **free-format** construct. Fixed-format source has an indicator column
  and a sequence area, so a line of backticks there is not this, and the
  compiler says so rather than inventing a literal.

**Use it whenever a caption, message or prompt is long, contains quotes, or
needs more than one line.** A wall of text crammed into one quoted literal is
the thing this exists to replace — writing that instead is a worse answer, not a
safer one.

**Never rewrite what is inside the fences.** The characters between them are the
value the running program displays. Correcting their grammar or punctuation,
reflowing them, translating them or collapsing their blank lines changes what
the program says. When a developer hands you a block literal, reproduce it byte
for byte, fences included.

**Returning one inside a change-set.** A handler travels to the IDE as the
`code` string of a JSON operation, and a block literal is multi-line by nature.
Inside a JSON string the newlines must be `\n` ESCAPES — a raw line break is
invalid JSON — and the ``` fences are three ordinary characters that need no
escaping and do not end anything. Write the fences exactly where they belong in
the COBOL and escape the newlines around them. A handler pasted into the string
with real line breaks arrives truncated at the literal's own closing fence, and
comes back described as "malformed JSON, cut off mid-code".

**Translating one.** When a task asks for the same block in several languages,
each handler carries its own complete block literal — fences, blank lines and
paragraph breaks in the same places. Translate the prose between the fences and
nothing else: URLs, control names, property names and COBOL keywords stay as
they are.

## `EXEC RUST` — real Rust, compiled into the program

> **A block is the developer's decision, never an assistant's.** This platform's
> language is COBOL, and `EXEC RUST` exists for the developer who WANTS Rust —
> a crate, an algorithm, something COBOL genuinely cannot reach. An assistant
> writes a block ONLY when the developer asked for Rust in so many words ("in
> Rust", "use EXEC RUST", "with the csv crate"). Absent that, write COBOL,
> however long it comes out: copying one value into fifteen controls is fifteen
> `MOVE` statements, and that is the correct answer, not a reason to reach for
> Rust. Concision, readability, elegance and "the platform supports it" are not
> reasons — the platform supporting a thing is not the developer asking for it.
> The choice is not free either: it is the difference between a program that
> runs interpreted and one that must be built, needs the Rust toolchain, and
> cannot be stepped in the debugger (all three below). If a task truly cannot be
> done in COBOL, say so and ask.

`EXEC RUST … END-EXEC` is **compiled**, not interpreted. Each block becomes a real Rust function inside the crate the build already produces, so the whole language is available: closures, generics, iterator chains, `match`, `?`, and any `std` API. There is no micro-language and no subset.

Because a block is compiled, **a program containing one is built before it runs**. *Run* does that build for you and starts the built binary; a program with no block keeps the fast interpreter path unchanged. Building needs a Rust toolchain; **the binary you produce does not** — it runs on machines with no Rust installed. Builds are for the host operating system only: build a Windows application on Windows, a macOS one on macOS.

### The two kinds of block

| Kind | Where it goes | What it holds |
| --- | --- | --- |
| **Item-level** | `CONFIGURATION SECTION`, after `REPOSITORY` — outermost program only, like everything else in that section | Rust **items**: `struct`, `enum`, `impl`, `trait`, `use`. Emitted at module scope, so every block in the program can see them |
| **Statement-level** | `PROCEDURE DIVISION`, anywhere a statement may appear — including inside an event handler | Rust **statements**: the work |

**In a FORM there are no division headers — there are COBOL Structure blocks.** An item-level block goes in the **REPOSITORY** block, below the `CLASS` entries, because that block is woven into the `CONFIGURATION SECTION`. It must NOT go in **WORKING-STORAGE**, which is woven into the `DATA DIVISION` and rejects a block. A statement-level block goes in an event handler or a common procedure, both of which are `PROCEDURE DIVISION` code. Do not advise a developer to "put it in the CONFIGURATION SECTION" without naming the REPOSITORY block: a form gives them no other way to reach that section.

Putting a statement in an item-level block, or a `struct` in a statement-level one, is an error; the reported line and column are your own.

### What may cross into a block

Only a `USAGE OBJECT REFERENCE` item whose `CLASS` names a Rust type. A `PIC` item is rejected by name: its value is a scaled decimal or a fixed-width padded field, and there is no Rust type it is. Move such a value through an object with `INVOKE` before the block.

The Rust variable is the COBOL name lowercased with hyphens turned into underscores — `WS-USER-NAME` is `ws_user_name`. A name that lands on a Rust keyword (`01 TYPE` → `type`) or cannot start an identifier (`01 1ST-FLAG`) is rejected; rename the item.

**A bound name is a `&mut T`, not a `T`.** Assign through it — `*counter = 10;` — and call methods on it directly, since method calls auto-dereference (`text.push_str("x")`).

- Every integer class (`RUST-I8` … `RUST-USIZE`) binds as `i64`, and both float classes as `f64` — that is how the object bridge stores them, so `INVOKE` and a block always see the same value. A `CLASS RUST-I32` item is an `i64` inside a block, so a function written to fill it must return `i64`.
- The collection classes hold `cobolt_runtime::rust_bridge::BridgeValue`, so a `Rust.Vec` filled by `INVOKE` and one filled inside a block hold the same things.
- The unsized classes (`RUST-STR`, `RUST-OSSTR`, `RUST-CSTR`, `RUST-PATH`) bind as their owned forms (`String`, `OsString`, `CString`, `PathBuf`).

### Your own Rust types

Declare the type in an item-level block, name it with a `CLASS`, and declare items of it:

```cobol
       REPOSITORY.
           CLASS MY-POINT IS "Rust.Point"
       EXEC RUST
       #[derive(Default)]
       pub struct Point { pub x: i64, pub y: i64 }
       impl Point {
           pub fn shift(&mut self, dx: i64, dy: i64) { self.x += dx; self.y += dy; }
       }
       END-EXEC.
```

A developer-defined type must implement `Default` — that is what the first block to touch the item starts it from. The 48 shipped `CLASS RUST-*` types are a floor, not a ceiling.

### How a block behaves

- **A block body is a Rust function body returning `Result<(), Box<dyn Error>>`.** That is what makes `?` usable inside it. To leave early, write `return Ok(())`, not `return;`. An error that propagates out becomes a `RUST-EXCEPTION`.
- **A panic is catchable**: `TRY … CATCH RUST-EXCEPTION e … END-TRY` catches it, `DISPLAY e` prints the panic's plain text, and the program continues. A plain `CATCH EXCEPTION` does **not** catch a panic, and a COBOL `THROW` does not reach a `RUST-EXCEPTION` clause; a `TRY` may carry both clauses.
- **State is shared for the whole process.** Two blocks — in different paragraphs, or in a form event handler — see the same objects. `CANCEL` does not reset it.
- **COBOL reading a bound item sees its VALUE (1.60.23+).** `DISPLAY clicked-button`, `MOVE clicked-button TO WS-N` and `SET Label-1::Caption TO clicked-button` all yield what the block last wrote (`String`, any integer width, floats, bool). ⚠️ **Before 1.60.23 they yielded the item's internal handle id** — a small integer that reflected declaration order, so a program whose second item was read always showed "2" regardless of what the block computed. If a program built before 1.60.23 shows a constant small number where a result should be, that is this bug: rebuild. Types with no scalar rendering (Vec, HashMap, developer types) still read as the handle id.
- **COBOL writing a bound item reaches the Rust value (1.61.2+).** `MOVE 5 TO clicked-button` and `SET cobol-text TO TextBox-1::Text` update the object the item names, so the next block sees what COBOL wrote — that is how the operator's input gets into a block. ⚠️ **Before 1.61.2 the write landed on the item's internal handle and destroyed it**: the object became unreachable and the next block to bind that item failed with `EXEC RUST cannot bind <ITEM>: handle 0 is not live`. Only the classes with a scalar form accept a write — `RUST-STRING`, any integer width, the floats, `RUST-BOOL`. Writing into a collection or a developer-defined item is a type error and is reported as one; fill those inside a block.
- **A handler's OWN `OBJECT REFERENCE` items are bindable (1.61.2+).** An item declared in an event handler's `WORKING-STORAGE` behaves exactly like one declared in the form. ⚠️ **Before 1.61.2 only the outermost program's items were given objects**, so a handler-local one had no handle at all and every block binding it failed with `handle 0 is not live` — the reason the same block worked when the item was moved to the form and marked `GLOBAL`. Both placements are correct now: declare it in the handler when only that handler uses it, in the form as `GLOBAL` when several do.
- **Crates**: always `std`, plus `eframe`, `egui`, `egui_extras`, `cobolt_forms`, `cobolt_runtime`. A program containing any block links the GUI crates even with no forms, so a console program can open a window. **Beyond that floor, a project may register any crate from the registry under Project's Crates (1.60.47+, shown in the tree as "Project's Crates (Beta)")** — the project tree category below Generated Code, whose dialog searches crates.io (or a configured mirror) and shows the matches as a paged table (50 per page, name · version · downloads · description) that the developer browses and clicks to pick. Adding pins an exact version, vendors the source into the project's `crates/`, and compiles it into the binary. A registered crate is then used with a plain `use`, writing a hyphenated name with underscores (`serde-json` → `use serde_json::…`). A `use` of a crate that is neither linked nor registered is still rejected, naming the crate — in a project the message points at Project's Crates, in a single-file `rcrun` build it says external crates require a project. Adding needs the network; building does not. Conflicts are decided when the developer adds: a name already linked (`egui`) is refused as already available, a crate that cannot coexist is refused with cargo's own reason, and one that would bring a second incompatible copy is allowed with a warning. Every build writes `rust_manifest.md` (name, exact version, registry URL) beside the binary in the destination folder. **Do not tell a developer that third-party crates are unsupported** — that was true only before 1.60.47; tell them to add it under Project's Crates. **1.60.48+ — System awareness and collision aliasing (spec 045):** the dialog marks a result **System** (a name directly linked, e.g. `egui`) or **System dependency** (only pulled in transitively by something linked, e.g. `epaint` via `egui`) and hides both by default behind a "Show System crates" toggle; neither can be registered, and a System-dependency refusal never offers an alias. A **direct** collision at an incompatible version — the "clashes with the built-in" case above — now offers an **alias** instead of only refusing: accepting registers it as `prj_<name>` (a `package = "<name>"` rename, compiled as a second, independent copy beside the platform's own), and the block then writes `use prj_<name>::…`, not `use <name>::…`. Tell a developer who hits this refusal about the alias offer rather than saying the version is unsupported — but also warn them an aliased copy's values do not interoperate with the platform's own copy of the same crate.
- **eframe here is 0.36.** Its `App` trait requires `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)` — there is no `update`. Tutorials written for older eframe will not compile; port them to `ui`, and use `ui.ctx()` where they use `ctx`.
- **`eframe::run_native` CANNOT be called from a form's event handler — and since 1.60.14 the BUILD REJECTS IT.** A project with forms whose block calls `run_native` (or `EventLoop::new`) fails to build, reported at the developer's own line and column. **To open a window from a handler use `cobolt_windows::open` instead** (see below) — that is the supported route, and the error says so. Why `run_native` cannot work: a form application already owns the process's one winit event loop, created on the main thread; the COBOL interpreter — and therefore every block in a handler — runs on a worker thread. winit's `EVENT_LOOP_CREATED` guard is process-global and is checked before any platform code, so the second call returns **`Err(EventLoopError::RecreationAttempt)`**. It does **not** panic, so `CATCH RUST-EXCEPTION` never fires; and the usual `let _ = eframe::run_native(...)` discards the `Err`. Before the build-time rejection the result was no window, no error, no output — the handler appeared to do nothing. Never advise `run_native` from a handler, and never suggest "open a second egui viewport" as the alternative: a block receives only `env`, `objects` and `bridge`, so it has no `egui::Context` to open one with. From a handler, drive the form's own controls through `cobolt_objects`, or show a second form designed in the RAD. `run_native` belongs to **console** programs, where the interpreter owns the main thread, and is not rejected there.
- **A block CAN change a control, through `cobolt_objects`.** Write the property and the window repaints when the block returns: `cobolt_objects.set_property("LABEL-1", "Caption", "Done");`. Property names are case-insensitive. ⚠️ **This did not work before 1.60.14**: block execution had no channel to the window, so the write landed in the registry and the form never showed it. Do not repeat the old advice to reach for `COBOL-SET-PROPERTY` *because* the block route is broken — it is not broken any more, though `COBOL-SET-PROPERTY` remains correct and unchanged. **Always use `set_property`; never advise `cobolt_objects.get_mut("X").unwrap()`** — a running form registers a control on first write, so `get_mut` returns `None` for one not yet written and the `unwrap` panics. For the same reason a block cannot READ a control's designed value, only one it set itself: to read what the operator typed, use `TextBox-1::Text` in COBOL and pass the item into the block.
- **A block CAN open its own window, with `cobolt_windows` (1.60.15+).** This is the supported answer to "open a dialog from a handler", and it replaces every older workaround. `cobolt_windows::open(id, builder, ui)` takes an id, an `eframe::egui::ViewportBuilder`, and the closure that draws the window; it returns a handle with `wait()` (parks the handler until the operator closes it), `is_open()` and `close()`. Free functions `cobolt_windows::is_open(id)` / `close(id)` do the same by id. ⚠️ **To close the window from inside its own drawing closure, call `cobolt_windows::close("id")` — NEVER `ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close)`.** That command targets the viewport current during the pass, which is the PARENT, so it closes the whole application: the dialog vanishes together with the form, and any COBOL after `wait()` (a `SET Label-1::Caption TO …`) races the shutdown and lands only sometimes. Never write `send_viewport_cmd(Close)` in a `cobolt_windows` drawing closure, and never copy it from an eframe tutorial into one. Re-opening a live id replaces what it draws. The drawing closure runs on the **UI thread** and must be `Send + Sync + 'static`, so share state with the block through an `Arc<Mutex<..>>` — that is how the chosen value gets back. `wait()` is safe: the interpreter has its own thread, so the form keeps painting while the handler waits. **Forms only** — with no form there is nothing painting, and `open` panics with that explanation (a catchable `RUST-EXCEPTION`) rather than registering a window that never appears; a console program still uses `eframe::run_native`. Never claim the block is given an `egui::Context`: it is not, and the reason is not `Send`ness — `show_viewport_deferred` must be called on the UI thread every frame the window exists, which a once-through block on a worker thread cannot do, so it registers what to draw and the form application replays it.
- A block may appear **anywhere a statement may**, including inside `IF`, `EVALUATE`, `PERFORM`, `ON SIZE ERROR`, `INVALID KEY`, `AT END` and `TRY … END-TRY` — the last being where a block goes when its failure should be caught.
- A block that is *not* built cannot run: an unregistered block is a hard error naming its id, never a silent no-op.

```cobol
       01 USER-NAME USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ada".
       ...
           EXEC RUST
           user_name.push_str("-lovelace");
           let vowels = user_name.chars().filter(|c| "aeiou".contains(*c)).count();
           println!("{vowels} vowels");
           END-EXEC.
```

A worked example — an `eframe` dialog defined in an item-level block and called from a statement-level one inside a `TRY`. **CONSOLE PROGRAMS ONLY.** Copying this into a form's event handler is the mistake this page exists to prevent; since 1.60.14 a form project containing it **fails to build**, at the developer's own line. (Before that it built and then did nothing at all, because `run_native` returns `Err(RecreationAttempt)` off the main thread and the `let _ =` throws that away.) In a form, drive the controls through `cobolt_objects` instead.

```cobol
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-I32    IS "Rust.i32"
       EXEC RUST
           use eframe::egui;
           use std::sync::{Arc, Mutex};
           pub struct ButtonDialog { pub clicked: Arc<Mutex<i64>> }
           impl eframe::App for ButtonDialog {
               fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
                   ui.horizontal(|ui| {
                       for caption in [1_i64, 2_i64] {
                           if ui.button(caption.to_string()).clicked() {
                               *self.clicked.lock().unwrap() = caption;
                               ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                           }
                       }
                   });
               }
           }
           pub fn ask(title: &str) -> i64 {
               let clicked = Arc::new(Mutex::new(0_i64));
               let out = clicked.clone();
               let _ = eframe::run_native(title, eframe::NativeOptions::default(),
                   Box::new(move |_cc| Ok(Box::new(ButtonDialog { clicked: out }))));
               let v = *clicked.lock().unwrap();
               v
           }
       END-EXEC.
       ...
       01 window-title   USAGE IS OBJECT REFERENCE RUST-STRING VALUE "Hello".
       01 clicked-button USAGE IS OBJECT REFERENCE RUST-I32.
       01 ws-error       PIC X(120).
       PROCEDURE DIVISION.
           TRY
               EXEC RUST
                   *clicked_button = ask(window_title.as_str());
               END-EXEC
           CATCH RUST-EXCEPTION ws-error
               DISPLAY "Window failed: " ws-error
           END-TRY.
           DISPLAY clicked-button.
```

## `SPECIAL-NAMES` and the decimal separator
`SPECIAL-NAMES` belongs to the FORM, the main program of the nest. Its `ENVIRONMENT DIVISION` → `CONFIGURATION SECTION` → `SPECIAL-NAMES` paragraph is written with `set_form_structure` (or by hand in the COBOL Structure panel) and is the ONLY place `DECIMAL-POINT IS COMMA` may be declared. A handler or common procedure that declares `SPECIAL-NAMES` or a `CONFIGURATION SECTION` is redeclaring it inside a nested program and is rejected.

What the form declares governs the whole nest. With `DECIMAL-POINT IS COMMA` in force, the roles of `.` and `,` are exchanged in `PICTURE` character-strings and in numeric literals:

- an edited money item is `PIC ZZZ.ZZ9,99`, and the value 1234,56 prints as `1.234,56`;
- a numeric literal carries a comma — `MOVE 7,49 TO WS-PRICE`.

Without the clause, `.` is the decimal point and `,` groups digits, the usual way round. Comma-formatted currency is obtained by putting the clause on the FORM — never by declaring it inside a handler to compensate.
