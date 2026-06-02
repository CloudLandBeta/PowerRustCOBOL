<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Cobolt IDE — Changelog

## [PowerRustCOBOL 1.1.0] — 2026-06-01

### Form Designer & rendering

- **New widget: Animator.** Plays animated images — **GIF, WebP and APNG** (and
  any still image) — decoded natively via the `image` crate (no external/FFmpeg
  dependency). Properties: `Source`, `AutoPlay`, `Loop`, `SizeMode`
  (Fit/Fill/Stretch/Center), back/border. Decoding + frame-timed egui playback
  live in the new shared `cobolt-media` crate, so the widget animates in the
  designer canvas, the preview, the run-form **and** the compiled standalone
  binary. (MP4 support is planned via a native decoder behind the same API.)


- **System font picker** — the Font property is now a dropdown of the fonts
  installed on the machine (via `fontdb`), each name rendered **in its own
  font**. The list is virtualised, so only the families you actually scroll
  past are loaded. The chosen font **family and size** are now applied to the
  rendered text in the **designer canvas, preview window and run form**, with a
  graceful fallback to the built-in (Arial-like) proportional font when a family
  is Arial/default or unavailable on the target system. Bitmap-only faces (e.g.
  `GB18030 Bitmap`) that egui can't rasterise are rejected up-front, fixing a
  crash when scrolling the font list. (`cobolt-ide/src/fonts.rs`)

- **#69 — Resize the form canvas by dragging its border.** Right, bottom and
  bottom-right corner grips; live resize with grid snap and a minimum size.
  (`designer.rs`)

- **#70 — Double-click an event row to jump to its COBOL paragraph.** The
  generated `.cbl` is opened in the editor and scrolled to the paragraph (or
  `PROGRAM-ID`) definition. Single-click still opens the per-event modal editor.
  (`properties.rs`, `app.rs`, `editor.rs`; i18n key `hint_dblclick_event`)

- **#129 — Preview animations now apply `scale`.** Zoom/spin/flip animations
  resize widgets in the preview window, via the shared
  `designer::scale_rect_about_center()` (also used by the canvas). (`app.rs`)

### Runtime / language

- **COBOL sequential file I/O — `ORGANIZATION IS SEQUENTIAL` and
  `LINE SEQUENTIAL`.** The ENVIRONMENT DIVISION's `FILE-CONTROL` is now parsed
  (`SELECT … ASSIGN TO … ORGANIZATION IS [LINE] SEQUENTIAL [ACCESS MODE …]
  [FILE STATUS IS …]`), and the runtime implements `OPEN INPUT/OUTPUT/EXTEND/I-O`,
  `WRITE record [FROM …]`, `READ file [INTO …] [AT END …] [NOT AT END …]`, and
  `CLOSE`, updating the FILE STATUS item (00/10/30/35/…). LINE SEQUENTIAL writes
  newline-terminated records (trailing spaces dropped); record SEQUENTIAL writes
  fixed-length records. `ASSIGN TO` accepts a literal path or a data item holding
  the path. `READ … AT END` accepts the two-word `AT END` / `NOT AT END` forms.
  (`cobolt-ast`, `cobolt-parser`, `cobolt-runtime`)

- **New built-in CALLs `COBOL-APPEND-FILE` / `COBOL-WRITE-FILE`** —
  `USING path text [status]` append a line to (or truncate+write) a text file.
  COBOL `OPEN/WRITE` file I/O is still unimplemented; these cover the common
  "write a results/log file" need. (`interpreter.rs`)

- **PICTURE repetition counts are now honored.** `analyze_pic` ignored `(n)`, so
  `PIC X(20)` held 1 char and `PIC 9(5)` had 1 digit. Templates are now expanded
  (`X(20)`→20, `9(7)V99`→7.2), and `PicClause.digits/decimals` widened to `u16`
  so wide fields like `PIC X(4096)` / `PIC X(32767)` are exact. (`cobolt-parser`,
  `cobolt-ast`)

- **Alphanumeric comparison pads with spaces.** `compare_values` compared raw
  strings, so a space-padded `PIC X(64)` field never equalled a short literal
  (e.g. `EVALUATE control-id WHEN "BTN-OK"` never matched). The shorter operand
  is now space-padded per COBOL rules. (`interpreter.rs`)

- **`STRING … DELIMITED BY SIZE` works.** The bare word `SIZE` lexes to the
  `SizeError` token (reserved for ON SIZE ERROR); the STRING parser now accepts
  it as the SIZE delimiter, so `STRING` no longer dropped all operands.
  (`cobolt-parser`)

### Compiler (standalone binary)

- **Richer Label rendering in the generated form app.** The compiled binary's
  Label now honors BackColor, ForeColor, FontSize, Bold/Italic/Underline/
  Strikethrough, TextAlign, WordWrap, Padding, Opacity, BorderStyle/BorderColor,
  Cursor (on hover), per-control geometry overrides (`X/Y/Width/Height`) and
  `Dock` from `COBOL-SET-PROPERTY`, plus a short input warm-up so a click already
  underway as the window opens can't trigger a control. (`cobolt-compiler`)

### Fixes

- Fixed a long-broken `cobolt-codegen` test target (ambiguous `.into()` in
  `Control::new` calls) and corrected stale form-event paragraph-name
  expectations (`MAIN-FORM--ONLOAD`, not `--ON-LOAD`).

- **Lexer — fixed-form identification area now stripped.** `flatten_fixed` /
  `preprocess_fixed` were slicing active source out to char-column 255 instead
  of 72, so anything a program placed in columns 73–80 (the identification area)
  leaked into the token stream. Now correctly cut at column 72. (`source.rs`)

- **Lexer — `END-PERFORM` is a scope-terminator keyword.** Corrected stale tests
  that asserted it should be an identifier; the keyword table and parser have
  always treated it as `Token::EndPerform` (like `END-IF` / `END-EVALUATE`).

- **Parser — sequential program units in one file are no longer dropped.**
  `parse_program` now collects sibling program units that follow the first
  program's `END PROGRAM` terminator (e.g. `OUTER. … END PROGRAM OUTER.` then
  `SET-RESULT. … END PROGRAM SET-RESULT.`) into `nested_programs`, so the runtime
  can `CALL` them. True nesting (inner units before the outer terminator, the
  codegen shape) is unchanged. Fixes all 6 `cobolt-runtime` nested-program tests.
  New regression tests in `cobolt-parser/tests/test_nested_programs.rs`.

### Tests

- Added unit/behavioural tests: `fonts::tests` (enumeration, fallback, on-demand
  load, bitmap rejection), `designer::form_resize_tests`,
  `designer::anim_behavior_tests::scale_rect_…`, and `editor::goto_tests`.
  `cargo test -p cobolt-ide` → 35 passing.

## [2.5.0] — 2026-05-30

### Phase 11 — Embed+Bundle Binary Compiler

Cobolt projects can now be compiled into a **single self-contained native
executable** with no source code included.  The output binary embeds the
compressed AST and all form files, then runs them through the existing
interpreter at launch.

#### New crate: `cobolt-compiler`

The core build pipeline lives in `crates/cobolt-compiler/src/lib.rs`:

1. **Load manifest** — reads `cobolt.toml`, resolves main source + additional
   sources + form files.
2. **Lex → parse → semantic** — validates all COBOL sources; aborts on any
   error so only correct programs are compiled.
3. **Serialize + compress** — the `Program` AST is serialised with `bincode`
   and deflate-compressed with `flate2` (best compression).  Typical savings:
   60–75% smaller than raw bincode.
4. **Generate build project** — writes a temporary Cargo project to
   `/tmp/cobolt-build-<name>/` containing:
   - `Cargo.toml` — depends on `cobolt-runtime`, `cobolt-forms`, `eframe`/`egui`
     via path references to the local workspace.
   - `src/main.rs` — embeds assets via `include_bytes!`, contains a lazy form
     dispatch table, and launches either a headless interpreter or an eframe
     form application depending on whether forms are present.
   - `assets/program.bin` — compressed AST.
   - `assets/forms/<ID>.cfrm` — raw form XML for each form.
5. **`cargo build --release`** — compiles the generated project to a native binary.
6. **Copy to `bin/`** — the executable is placed at
   `<project-root>/bin/<project-name>` (`bin/<name>.exe` on Windows) with
   executable permissions set on Unix.

New workspace dependencies: `bincode = "1"`, `flate2 = "1"`.

#### Lazy form loader

The generated binary contains a `static FORMS: &[(&str, &[u8])]` dispatch
table.  A form is only deserialised from its embedded bytes when first
requested by the running COBOL program, keeping startup time constant
regardless of how many forms the project contains.

#### `cobolt build` CLI command

```
cobolt build [cobolt.toml] [--quiet]
```

Calls `cobolt_compiler::build_project()` and prints a summary on success:

```
✅ Build complete!
   Binary : myapp/bin/myapp
   Sources: 3
   Forms  : 2
   AST    : 8 412 bytes (compressed)
```

#### IDE — 🔨 Build Binary menu item

`File → 🔨 Build Binary (bin/)` triggers `do_build_binary()`, which:
- Spawns the compiler on a background thread (IDE stays responsive).
- Shows a ⏳ spinner label while building.
- Prints the binary path and stats in the Output panel when done.
- Shows an error message if the build fails.

---

## [2.4.0] — 2026-05-30

### Phase 10 — REST Client Runtime

COBOL programs can now make real HTTP requests — GET, POST, PUT, DELETE — using
standard `CALL` statements handled entirely inside the interpreter.  No external
tools, FFI, or async runtime are required.

#### New dependency: `ureq` (`cobolt-runtime/Cargo.toml`)

`ureq = { version = "2", features = ["json"] }` — a minimal blocking HTTP
client with built-in TLS support.  No async executor is pulled in.

#### New: `HttpClient` (`cobolt-runtime/src/http_runtime.rs`)

`HttpClient` manages per-session HTTP state for the interpreter:

- `get(url) -> (body, status)` — HTTP GET; returns the response body and
  numeric status code.  On network failure status is `0`.
- `post(url, body) -> (body, status)` — HTTP POST; Content-Type defaults to
  `application/json` unless overridden by `set_header`.
- `put(url, body) -> (body, status)` — HTTP PUT with the same body semantics.
- `delete(url) -> (body, status)` — HTTP DELETE.
- `set_header(name, value)` — adds / overwrites a persistent header sent on
  every subsequent request.
- `clear_headers()` — removes all persistent headers.

All methods strip trailing COBOL spaces from URL and body arguments before
sending.

#### Updated: `Interpreter` — 6 HTTP built-in `CALL` handlers

An `http: HttpClient` field is now part of `Interpreter` (initialised in
`new()`, inherited by `new_with_debug_channels()`).  `exec_call()` handles:

| CALL name                  | Arguments (BY REFERENCE)                          |
|----------------------------|---------------------------------------------------|
| `COBOL-HTTP-GET`           | url-var, response-var, status-var                 |
| `COBOL-HTTP-POST`          | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-PUT`           | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-DELETE`        | url-var, response-var, status-var                 |
| `COBOL-HTTP-SET-HEADER`    | name-var, value-var                               |
| `COBOL-HTTP-CLEAR-HEADERS` | (no arguments)                                    |

`response-var` receives the full response body (truncated by the `PIC X(32767)`
declaration if needed).  `status-var` (PIC 9(4)) receives the HTTP status code.

#### Updated: Codegen REST stubs (`cobolt-codegen/src/lib.rs`)

The working-storage section for `RestClient` controls no longer uses INVOKE /
OO-style comments.  Generated variables are now:

```cobol
01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.
01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.
01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.
01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.
01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.
01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.
01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.
01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.
```

`write_rest_client_stubs()` now generates three CALL-based paragraphs per
RestClient control (replacing the `INVOKE`-based stubs):

- **`{ID}-GET`** — `CALL "COBOL-HTTP-GET"` with url, response, and status;
  dispatches to the response or error handler paragraph based on the status code.
- **`{ID}-POST`** — `CALL "COBOL-HTTP-POST"` with url, body, response, status.
- **`{ID}-PUT`** — `CALL "COBOL-HTTP-PUT"` with url, body, response, status.
- Response / error handler stub paragraphs are generated for each control.
- An optional `{ID}-SYNC-ITEMS` paragraph copies `WS-HTTP-RESPONSE` and
  `WS-HTTP-STATUS` into user-configured `ResponseDataItem` / `StatusDataItem`
  data fields.

---

## [2.3.0] — 2026-05-30

### Phase 9 — Project Packaging

Cobolt projects can now be bundled into a self-contained, runnable zip archive
both from the IDE and from the command line.

#### New: `cobolt package` CLI command (`cobolt-cli/src/main.rs`)

```
cobolt package [cobolt.toml] [--output path.zip]
```

- Reads a `cobolt.toml` project manifest (defaults to `./cobolt.toml`).
- Packs all tracked source files, forms, and assets with their relative paths
  preserved inside the archive.
- Generates a `run.sh` (Unix, executable) and `run.bat` (Windows) launcher
  so users can run the project without knowing `cobolt` CLI syntax.
- Generates a `README.txt` with installation instructions.
- If a `cobolt` / `cobolt.exe` binary is found next to the currently running
  executable it is automatically bundled, making the archive fully self-contained.
- `--output` / `-o` flag overrides the default output path (`<name>.zip`).
- Prints per-file progress, warnings for missing files, and a final summary.

New dependencies added to `cobolt-cli/Cargo.toml`:
`serde = { workspace = true }`, `toml = { workspace = true }`,
`zip = { version = "2", features = ["deflate"] }`.

#### New: `package_project()` (`cobolt-ide/src/project_model.rs`)

The same packaging logic is available as a library function consumed by the IDE:

- `package_project(project, project_dir, output_zip) -> Result<usize, ProjectError>`
  — packs all tracked files + launchers + README; returns the count of archived items.
- `find_cobolt_binary()` — looks for the runtime binary next to the IDE executable.

#### Updated: IDE — File → Package Project menu item

`CoboltApp::do_package_project()` wires the menu entry to `package_project()`:

- Opens a native Save dialog pre-filled with `<project-name>.zip`.
- Requires a project to be open; otherwise shows a helpful status message.
- Reports the file count and output path in the Output panel on success.

---

## [2.2.0] — 2026-05-30

### Phase 8 — Database Runtime Engine

COBOL programs can now open real SQLite databases, execute SQL, and iterate
over result sets — all from standard `CALL` statements.  No host-language
embedding or FFI required.

#### New dependency: `rusqlite` (`cobolt-runtime/Cargo.toml`)

`rusqlite = { version = "0.31", features = ["bundled"] }` — SQLite is compiled
in from source; no system library or external install is needed.

#### New: `DbConn` and `DbRegistry` (`cobolt-runtime/src/db_runtime.rs`)

`DbConn` wraps a `rusqlite::Connection` and a cached result-set cursor:

- `open(conn_str)` — accepts a bare file path, `sqlite:<path>`, or `:memory:`.
- `exec(sql)` — auto-detects `SELECT`/`WITH`/`PRAGMA` vs. DML.  SELECT results
  are cached as `Vec<Vec<String>>`; DML returns the affected-row count.
- `fetch_col(col)` — returns column `col` (1-based) of the current row.
- `next_row()` — advances the cursor; returns `false` when exhausted.
- `row_count()` / `is_exhausted()` — query result-set metadata.

`DbRegistry` manages all open connections for one interpreter instance as a
`HashMap<u32, DbConn>` keyed by integer *handle*:

- `open(conn_str) -> u32` — opens a connection and returns its handle.
- `exec(handle, sql)`, `fetch_col(handle, col)`, `next_row(handle)`,
  `row_count(handle)`, `is_exhausted(handle)`, `close(handle)`, `close_all()`.

#### Updated: `Interpreter` — 6 SQL built-in `CALL` handlers

A `db: DbRegistry` field is now part of `Interpreter`.  `exec_call()` handles
six new built-in names (matched case-insensitively):

| CALL name            | Arguments (BY REFERENCE)                                  |
|----------------------|-----------------------------------------------------------|
| `COBOL-OPEN-DB`      | conn-string, handle-var (PIC 9(9)), status-var (PIC X)    |
| `COBOL-EXEC-SQL`     | handle, query, row-count-var, status-var                  |
| `COBOL-FETCH-ROW`    | handle, col-index (1-based), dest-var, status-var         |
| `COBOL-NEXT-ROW`     | handle, more-flag-var (`Y`/`N`)                           |
| `COBOL-ROW-COUNT`    | handle, count-var                                         |
| `COBOL-CLOSE-DB`     | handle                                                    |

On interpreter shutdown (`send_debug_finished`) `db.close_all()` is called
to release all connections.

#### Updated: Codegen SQL stubs (`cobolt-codegen/src/lib.rs`)

Working-storage for `SqlDatabase` controls no longer uses `USAGE IS OBJECT`
items.  The generated variables are now:

```cobol
01 WS-{ID}-CONN-STRING   PIC X(512)   VALUE ':memory:'.
01 WS-{ID}-HANDLE        PIC 9(9)     VALUE 0.
01 WS-{ID}-STATUS        PIC X(512)   VALUE SPACES.
01 WS-SQL-QUERY           PIC X(4096)  VALUE SPACES.
01 WS-SQL-ERROR            PIC X(512)   VALUE SPACES.
01 WS-SQL-ROW-COUNT        PIC 9(9)     VALUE 0.
01 WS-SQL-COL-INDEX        PIC 9(4)     VALUE 1.
01 WS-SQL-CURRENT-VALUE    PIC X(512)   VALUE SPACES.
01 WS-SQL-MORE             PIC X(1)     VALUE 'N'.
```

`write_sql_stubs()` generates four CALL-based paragraphs per control:

- **`{ID}-CONNECT`** — `CALL "COBOL-OPEN-DB"` with conn-string, handle, status.
- **`{ID}-EXEC`** — `CALL "COBOL-EXEC-SQL"` with handle, query, row-count,
  status; initialises `WS-SQL-MORE` to `'Y'`.
- **`{ID}-FETCH-ALL`** — loops `PERFORM UNTIL WS-SQL-MORE = 'N'` calling
  `COBOL-FETCH-ROW` for each column index and `COBOL-NEXT-ROW` to advance.
- **`{ID}-CLOSE`** — `CALL "COBOL-CLOSE-DB"` with handle.

---

## [2.1.0] — 2026-05-30

### Phase 7 — Debugger

The IDE now has a full interactive debugger for COBOL programs.

#### New: `DebugCmd` and `DebugEvent` channel types (`cobolt-runtime/src/debugger.rs`)

Two typed enums cross the thread boundary between the IDE and the interpreter:

- **`DebugCmd`** — `Continue`, `StepOver`, `Pause` — sent from the IDE to the
  interpreter to control execution.
- **`DebugEvent`** — `Paused { line, col, paragraph, vars }`, `Resumed`,
  `Finished` — sent from the interpreter back to the IDE.
- **`Breakpoints`** (`Arc<Mutex<HashSet<u32>>>`) — a thread-safe shared set of
  active breakpoint line numbers, written by the IDE and read by the interpreter.

#### Updated: `Interpreter` — per-statement debug hook

`Interpreter::new_with_debug_channels()` is a new constructor that wires the
debug channels into the interpreter.  Before every statement `exec_stmts()` now
calls `debug_check()`, which:

1. Extracts the statement's source line via `Stmt::span()`.
2. Checks whether the line matches a breakpoint **or** `debug_stepping` is true
   (StepOver mode).
3. If a pause condition is met, sends `DebugEvent::Paused` with a complete
   variable snapshot (`CobolEnvironment::iter()` → `VarSnapshot` list) and
   **blocks** on `debug_cmd_rx.recv()` until the IDE sends `Continue` or
   `StepOver`.
4. An async `Pause` command is handled via a non-blocking `try_recv()` poll on
   every statement when not already paused.
5. `DebugEvent::Finished` is sent when `run()` exits normally or via STOP RUN.

`current_paragraph` is updated as each paragraph is entered, so the Paused event
always carries the correct paragraph name.

#### New: `DebugRunner` (`cobolt-ide/src/runner.rs`)

`DebugRunner` is a sister to `Runner` that manages one debug session:

- `start(file_name, source)` — runs the full lex → parse → semantic pipeline,
  then spawns `Interpreter::new_with_debug_channels()` in a background thread.
- `send_cmd(DebugCmd)` — forwards a step/continue/pause command to the thread.
- `drain_events() -> Vec<DebugEvent>` — collects pending debug events each frame.
- `drain_run() -> Vec<RunMsg>` — collects pending run messages (diagnostics,
  output, finished).
- `pub breakpoints: Breakpoints` — the IDE writes breakpoint lines here before
  calling `start()`; the shared pointer is passed directly to the interpreter.
- `stop()` — drops `cmd_tx` (which unblocks any `recv()` in the interpreter,
  causing `Err(_)` → `StopRun`), then joins the thread.

#### New: Debugger side panel (`cobolt-ide/src/panels/debugger.rs`)

`DebuggerPanel` renders in a resizable right-side panel while a debug session
is active:

- **Step toolbar** — ▶ Continue (F5), ⤵ Step Over (F10), ⏸ Pause.  Buttons
  are disabled when the interpreter is running (not paused).
- **Location indicator** — paragraph name and source line, with a colour-coded
  ● Running / ● Paused status indicator.
- **Variable watch table** — displays all `CobolEnvironment` data items as
  a two-column striped grid (name / value), searchable via a filter text box.

#### New: Breakpoint gutter in editor.rs

The code editor's line-number column is now a fully interactive breakpoint
gutter:

- **Click** any line number to toggle a red breakpoint circle (●) on that line.
- When the debugger pauses, a **yellow arrow (→)** and highlighted row mark the
  current execution line.
- `EditorPanel::breakpoints: HashMap<PathBuf, HashSet<u32>>` stores active
  breakpoints per file.
- `breakpoints_for(path)` returns the line set for a given file, used by
  `do_debug()` to initialise the shared `Breakpoints` before starting the session.

#### New: 🐛 Debug toolbar button and keyboard shortcuts

A secondary toolbar strip appears below the main toolbar:

- **🐛 Debug** — starts a debug session for the active file (disabled while a
  normal run is active).  Automatically syncs breakpoints from the editor gutter
  into `DebugRunner::breakpoints` before starting.
- **■ Stop Debug** — drops the command channel (graceful stop), resets the
  debugger panel, and clears the editor debug-line highlight.
- **F5** — Continue (while a session is active).
- **F10** — Step Over (while a session is active).

#### i18n additions (all 5 languages)

New keys: `panel_debugger`, `dbg_continue`, `dbg_step_over`, `dbg_pause`,
`dbg_stop`, `dbg_variables`, `dbg_filter_hint`, `dbg_debug`.

---

## [2.0.0] — 2026-05-29

### Phase 6 — Form Runtime Engine

Forms can now be **executed interactively** from inside the IDE.  Pressing the
new **▶ Run Form** button in the designer toolbar compiles the form's generated
COBOL and runs it in a live, interactive OS window — no external tools required.

#### New: `FormEvent` and `StateUpdate` channel types (`cobolt-runtime`)

`crates/cobolt-runtime/src/channels.rs` introduces two typed messages that cross
the thread boundary between the egui UI and the background interpreter:

- **`FormEvent`** — sent from the UI thread to the interpreter when the user
  interacts with a control (`click()`, `change()`, `got_focus()`, `lost_focus()`).
  A special `quit()` sentinel (`ctrl_id = "__QUIT__"`) is used to unblock and
  terminate the interpreter cleanly.
- **`StateUpdate`** — sent from the interpreter to the UI whenever
  `COBOL-SET-PROPERTY` executes, carrying `ctrl_id`, `prop`, and `value` so the
  UI can update the live control snapshot immediately.

#### Updated: `Interpreter` — GUI channel support

`Interpreter::new_with_channels()` is a new constructor that wires three
`mpsc` channels into the interpreter for GUI-mode execution:

- `event_rx: Receiver<FormEvent>` — **`COBOL-WAIT-EVENT`** now _blocks_ on this
  receiver instead of immediately setting `COBOL-QUIT = 1`, enabling a real COBOL
  event loop.  Receiving the quit sentinel sets `COBOL-QUIT = 1` and exits.
- `state_tx: Sender<StateUpdate>` — **`COBOL-SET-PROPERTY`** sends a
  `StateUpdate` through this channel in addition to writing to the ObjectRegistry,
  so property changes are reflected in the UI on the next frame.
- `display_tx: Sender<String>` — **`DISPLAY`** statements route their output
  through this channel instead of stdout when in GUI mode; the IDE output panel
  receives each line via `OutputPanel::push_line()`.

CLI-mode behaviour (channels `None`) is completely unchanged.

#### New: `FormRuntime` (`cobolt-ide`)

`crates/cobolt-ide/src/form_runtime.rs` manages one live COBOL form execution:

- `FormRuntime::launch(form, form_path)` — generates COBOL from the form model,
  lexes, parses, and runs semantic analysis, then spawns
  `Interpreter::new_with_channels()` in a background thread.  Returns `Err` if
  parse or semantic analysis fails, displaying the errors in the output panel.
- `send_event(FormEvent)` — forwards a UI event to the interpreter thread.
- `drain_state() -> bool` — drains all pending `StateUpdate` messages and applies
  them to the `ctrl_state` snapshot; returns `true` when the UI should repaint.
- `drain_display() -> Vec<String>` — collects all `DISPLAY` lines produced since
  the last frame.
- `is_running() -> bool` — checks whether the interpreter thread is still alive.
- `stop()` — sends the quit sentinel and joins the thread.
- `Drop` impl ensures `stop()` is always called when the runtime is released.

Two supporting types are also defined here:

- **`CtrlMeta`** — immutable snapshot of a control's type, rect, z-order, and
  animations (populated at launch and used only for rendering order).
- **`CtrlState`** — mutable per-control state (`props`, `visible`, `enabled`),
  updated in-place by `drain_state()`.

#### New: **▶ Run Form** / **■ Stop Form** toolbar button

The designer toolbar now shows a **▶ Run Form** button when the form is not
running, and a **■ Stop Form** button while a runtime is active for that form.

- **▶ Run Form** saves the form, calls `FormRuntime::launch()`, and adds the
  runtime to `CoboltApp::form_runtimes`.
- **■ Stop Form** calls `stop()` on the matching runtime and removes it from the
  list.
- Multiple forms can run simultaneously in separate windows.

#### New: live interactive form viewport (`show_running_form_window`)

Each running `FormRuntime` gets its own OS window via `show_viewport_immediate`.
Every frame:

1. `drain_display()` output is forwarded to the IDE output panel.
2. `drain_state()` applies property updates to the live snapshot.
3. Controls are rendered in `z_order` from `ctrl_state` — buttons, labels,
   text boxes, checkboxes, combo boxes, list boxes, sliders, progress bars, and
   image controls are all handled.
4. User interactions fire the corresponding `FormEvent` back to the interpreter
   (`Click`, `Change`, `GotFocus`, `LostFocus`).
5. Non-visual controls (Timer, AgentObject, SqlDatabase, RestClient) are skipped.
6. Closing the window sends `FormEvent::quit()`, which unblocks
   `COBOL-WAIT-EVENT` and terminates the interpreter thread cleanly.

`ctx.request_repaint()` is called every frame while any form runtime is active,
ensuring the UI stays responsive to interpreter-driven state changes.

#### Output panel — `push_line()`

`OutputPanel::push_line(impl Into<String>)` was added to accept plain DISPLAY
output routed from the form runtime engine, displayed in the same monospace
light-grey style as normal program output.

---

## [1.1.0] — 2026-05-29

### New features & fixes

#### Form Designer — Save-on-close guard

Closing a dirty form designer window (one with unsaved changes) now triggers a
**Save / Discard / Cancel** confirmation dialog instead of silently discarding work:

- When the user clicks the OS close button (×) on a designer viewport that has
  unsaved changes, `ViewportCommand::CancelClose` is sent back to the OS to
  prevent the window from disappearing immediately
- A centred modal dialog appears with three choices:
  - **💾 Save & Close** — saves the `.cfrm` file and regenerates the `.cbl` COBOL
    source, then closes the window
  - **🗑 Discard & Close** — closes the window without saving
  - **Cancel** — dismisses the dialog, leaving the designer open and unchanged
- Closing via the dialog's own × button is treated as Cancel
- Clean (non-dirty) windows still close immediately without prompting

#### Form Designer — Save always regenerates COBOL

The **💾 Save** button in the designer toolbar now saves the `.cfrm` form file
**and** regenerates the `.cbl` COBOL source in a single action, keeping both files
in sync at all times.  The hover tooltip reads "Save form and regenerate COBOL".

Previously, Save only wrote the `.cfrm`; the user had to click "⚙ Generate COBOL"
separately to update the COBOL output.

#### Form Designer — Cmd+S in the designer window

**Cmd+S** (or Ctrl+S on Windows/Linux) now works inside designer viewport windows,
triggering the same save + regenerate action as the toolbar button.  Previously
Cmd+S was only handled in the main code-editor window and had no effect when the
designer was focused.

#### Properties panel — SqlDatabase `AutoConnect` type fix

`AutoConnect` was being pushed as `PropValue::String("true"/"false")` instead of
`PropValue::Bool(true/false)`.  The checkbox read the value back via `as_bool()`,
which checks for the `Bool` variant, so toggling `AutoConnect` had no effect.
Fixed: `PropValue::Bool(v)` is now used.

#### Properties panel — SqlDatabase COBOL Data Items grid layout

The "SQL Database — COBOL Data Items" section used an `egui::Grid` with
`num_columns(2)` but each `text_row_hint` call adds only one cell (a horizontal
layout containing both label and field).  The cells were therefore shifted by half
a column, causing labels and text edits to land in the wrong positions.  Fixed by:

- Changing the grid to `num_columns(1)` (each item gets its own full-width row)
- Adding `ui.end_row()` after each of the five `text_row_hint` calls
  (ConnDataItem, ResultSetDataItem, ConnectPara, QueryCompletePara, ErrorPara)

The same missing `ui.end_row()` was also present for the `ConnectionString` row
inside the "SQL Database — Connection" grid; that is fixed too.

#### Format painter — geometry copy

**Copy Style / Paste Style** (🖌 Format Painter) now also copies the source
control's position and size (X, Y, Width, Height) to the target control.

- `FormatPainter::WaitingForTarget` gains a `src_rect: cobolt_forms::model::Rect`
  field that captures the source control's `rect` at copy time
- The paste step writes `tgt.rect = src_rect` alongside the visual style properties
  and animations, so the target control becomes an exact geometric and visual copy
  of the source

#### Dead code removal — `bind_event` / `set_event_code` wiring

Removed all remnants of the old inline-editor event wiring that was superseded by
the modal `EventEditorModal` in v1.0.0:

- `pub bind_event: Option<(String, String, String)>` field removed from
  `InspectorAction` (was always `None` after the modal refactor)
- `bind_event()` and `set_event_code()` methods removed from `DesignerPanel`
- The three-line `bind_event` dispatch block removed from `DesignerPanel::handle_drag`

#### Label word wrap

Labels whose `Caption` text exceeded the control width were bleeding outside the
control border.  Two bugs were fixed:

1. **Wrong `max_width`** — `LayoutJob::wrap.max_width` was not set, so egui laid
   out the text as a single infinite line
2. **Wrong anchor for centred text** — with `halign = Align::Center`,
   `painter.galley(pos, ...)` treats `pos` as the **top-centre** anchor, not
   top-left.  `text_pos.x` was being set to `rect.min.x` (left edge), shifting
   the entire text block half a control-width to the left.  Fixed to
   `rect.center().x`.

#### IntelliSense — selection on click and Tab

Three bugs prevented selecting an autocomplete suggestion:

1. **Popup dismissal race** — `else { self.ac.visible = false; }` ran on the same
   frame the user clicked a row (the click briefly steals `TextEdit` focus, making
   `cursor_range` return `None`); the popup vanished before the click was processed.
   Fixed by removing the `else` branch entirely — the popup is now only dismissed
   by an explicit selection or Escape.

2. **Click detection on `Frame` rows** — `row_resp.response.interact(Sense::click())`
   does not detect clicks on `egui::Frame` responses because frames only sense hover.
   Fixed by replacing with `ui.interact(rect, id, Sense::click())`.

3. **Char vs byte index mismatch** — `trigger_pos` is a char index returned by
   `word_before_cursor`, but it was used directly as a byte offset in
   `String::replace_range`, causing a panic or wrong replacement on non-ASCII input.
   Fixed by converting via `tab.content.char_indices().nth(self.ac.trigger_pos)`.

#### Pointing-hand cursor on clickable elements

All interactive elements that use custom interaction (not standard egui buttons or
selectable labels) now show the `PointingHand` cursor on hover:

- **Toolbox cells** — `ui.ctx().set_cursor_icon(CursorIcon::PointingHand)` on hover
- **Canvas controls** — pointer becomes a hand when hovering any placed control
- **Properties panel event rows** — `.on_hover_cursor(CursorIcon::PointingHand)`
  on both control-event and form-event rows
- **Autocomplete popup rows** — `.on_hover_cursor(CursorIcon::PointingHand)` via
  the `click_resp` interact result

---

## [1.0.0] — 2026-05-29

### Major — Nested-program architecture (PowerCOBOL Option C model)

This is the first major version bump.  The entire code generation and form storage
model has been redesigned to match Fujitsu PowerCOBOL 3.0's approach: each event
handler becomes a COBOL-85 nested program; the `.cfrm` file is the single source of
truth; the generated `.cbl` is a build artifact the user never edits.

#### `.cfrm` file format (v1.0 — backward-compatible load)

Three new XML sections added to `.cfrm`:

- `<working-storage><![CDATA[...]]></working-storage>` — raw COBOL data declarations
  emitted verbatim into the outer program's WS; supports `GLOBAL` and `EXTERNAL`
  clauses for form-wide and cross-form data sharing
- `<form-events>` — `OnLoad` and `OnClose` lifecycle handlers stored as `<Event>`
  children with CDATA bodies
- `<deleted-controls>` — recycle bin: event code from deleted controls preserved
  here (never emitted into `.cbl`) so it can be restored later

`<Event>` elements now use start/end form with CDATA body for the user's COBOL
statements.  Old-format self-closing `<Event .../> ` tags still load correctly
(`code` will be empty).

#### Model changes (`cobolt-forms`)

- `EventBinding` gains `code: String` — raw COBOL statements for this handler
- `EventBinding::for_control(ctrl_id, event)` — auto-derives paragraph name as
  `"CTRL-ID--EVENT-NAME"` (double-hyphen separator)
- `EventBinding::has_code()`, `code_line_count()` — UI helpers
- `derive_paragraph_name(ctrl_id, event) -> String` — public utility function
- `Form` gains `user_ws_source: String`, `form_events: Vec<EventBinding>`,
  `deleted_code: Vec<DeletedControlCode>`
- `Form::new()` pre-populates `form_events` with empty `OnLoad` / `OnClose` stubs
- `Form::recycle_control(id, timestamp)` — moves event code to recycle bin before
  deleting; `restore_from_recycle(timestamp, target_id)` recovers it
- `Form::control_has_code(id)` — returns `[(event, line_count)]` for UI dialog
- `Control::ensure_event(event)` — idempotent event binding with auto-derived name
- `DeletedControlCode` struct — `control_id`, `deleted_at` (ISO timestamp), `events`

#### Properties panel (`cobolt-ide`)

- "Event Bindings" section replaced by read-only "Events" section showing `●`/`○`
  status dots and line counts per supported event; user directed to Code View to edit
- "COBOL Paragraphs" section removed from chart controls (superseded by Code View)
- `new_ev_name` / `new_ev_para` staging fields removed from `PropertiesPanel`

#### Code generation (`cobolt-codegen`) — Phase 2 complete

- `write_procedure_division()` fully rewritten to emit COBOL-85 nested-program structure
- Outer program (`COBOL-MAIN`) calls `CALL "MAIN-FORM--ON-LOAD"` / `CALL "MAIN-FORM--ON-CLOSE"` for lifecycle events; event loop dispatches to handlers via `CALL "BTN-OK--CLICK"` (not `PERFORM`)
- New `write_nested_programs()` iterates form-level events then per-control events and emits a nested program for each
- New `write_nested_program(prog_id, code, comment)` emits a self-contained `IDENTIFICATION … PROCEDURE … GOBACK. END PROGRAM name.` block; empty handlers get `CONTINUE.` with a TODO comment
- Outer program closes with `END PROGRAM <form-name>.`
- Tests updated: `generate_contains_nested_program`, `generate_contains_form_events_nested`, `generate_calls_on_load_nested`

#### Backward-compatibility removal (`cobolt-forms`)

- `Form::load_paragraph` and `Form::close_paragraph` fields removed
- `OwnedEvent::EventEmpty(String, String)` variant removed
- `load-paragraph` / `close-paragraph` attributes removed from XML save/load
- `backward_compat_empty_event_tag` test removed
- `PropertiesPanel` "On Load" / "On Close" paragraph text-edit rows removed
- `set_form_prop("LoadPara")` / `set_form_prop("ClosePara")` arms removed from designer
- Raw string delimiter in XML test changed from `r#"..."#` to `r##"..."##` (fix: `"#FFFFFF"` terminated the former prematurely)

#### IDE — Interactive event code editor (interim, Phase 5 preview)

- Events section in Properties panel replaced by a collapsible inline COBOL editor per event
- Each event row shows a `▸`/`▾` arrow, `●`/`○` code-presence dot, and line count
- Expanding a row shows the derived `PROGRAM-ID` hint and a 6-row monospace `TextEdit`
- Edits are propagated back to `EventBinding.code` via `InspectorAction::set_event_code`
- `#[derive(Default)]` added to `InspectorAction`; `set_event_code: Option<(String,String,String)>` field added

#### Toolbox icon size

- Icon buttons enlarged from 39 × 39 px to 49 × 49 px (+25 %)
- Top and left padding increased from 5 px to 10 px (+5 px each)

#### Parser — Phase 3: COBOL-85 nested program support

- `cobolt-lexer`: added `Token::End` for the bare word `"END"` (distinct from `END-IF`, `END-PERFORM`, etc.)
- `cobolt-ast/DataDecl`: added `is_global: bool` and `is_external: bool` fields
- `cobolt-ast/Program`: added `nested_programs: Vec<Program>` and `end_program_name: Option<String>` fields
- `cobolt-parser/data.rs`: `GLOBAL` and `EXTERNAL` clauses now set flags on `DataDecl` instead of being silently skipped; `Token::End` added to all stop-condition lists so data parsing halts before `END PROGRAM`
- `cobolt-parser/procedure.rs`: `Token::End` added to every stop condition in `parse_sections`, `parse_paragraphs_until_section`, `parse_paragraphs`, and the `parse_stmts` stop closures so paragraph/section collection halts before `END PROGRAM`
- `cobolt-parser/parser.rs`: `parse_program` delegates to new free function `parse_single_program`; after the `PROCEDURE DIVISION` the function loops collecting nested programs (each starting at `IDENTIFICATION`) and terminates on `END PROGRAM name.` or EOF; nested programs are stored in `Program::nested_programs`
- `cobolt-ast` tests updated with `is_global`, `is_external`, `nested_programs`, `end_program_name` fields

#### Runtime (`cobolt-runtime`) — Phase 4 complete

**`CobolEnvironment` scope management**

- `push_local_scope(items)` — inserts a nested program's own WORKING-STORAGE
  items into the shared env store and returns the list of keys that were newly
  added (items that already exist, e.g. GLOBAL names, are not overwritten)
- `pop_local_scope(keys)` — removes those keys on GOBACK, restoring the env
  to its pre-call state
- `global_items_from_data_division(data)` — collects all `is_global`-flagged
  data items from a DATA DIVISION; utility used internally by the registry builder

**`Interpreter` nested-program registry**

- New `NestedProgram` struct — holds `para_map`, `para_order`, and
  `local_items: Vec<(String, CobolValue)>` for one nested program
- New `nested_registry: HashMap<String, NestedProgram>` field on `Interpreter`
- `register_nested(prog, registry)` — free function that recursively registers a
  `Program` and all of its `nested_programs` into the registry (keyed by
  PROGRAM-ID, uppercase); called from `Interpreter::new()` at startup
- New `run_para_sequence(para_map, para_order)` method — executes a paragraph
  sequence from an explicit map (not `self.para_map`); handles GO TO within
  the nested program's own paragraph space; GOBACK propagated to caller

**`exec_call` dispatch**

- Added `_ if self.nested_registry.contains_key(&prog_name)` arm before the
  legacy flat-paragraph fallback
- On match: clones para_map + para_order + local_items out of registry (to
  avoid simultaneous mutable borrow), calls `push_local_scope`, runs
  `run_para_sequence`, calls `pop_local_scope` even on error
- GOBACK from a nested program is treated as a normal return (not an error)
- GLOBAL items from the outer program are naturally visible to nested programs
  because they live in the same `CobolEnvironment` store — no copying needed

**Tests** — `tests/test_nested_programs.rs`

- `call_nested_program_runs_and_returns` — CALL dispatches, nested program sets outer WS, returns
- `nested_local_ws_is_removed_after_goback` — local items do not persist after GOBACK
- `global_items_shared_with_nested_program` — GLOBAL WS mutations are visible in outer env
- `nested_program_internal_goto` — GO TO works within nested para_map; does not escape
- `multiple_nested_programs_dispatch_independently` — each CALL routes to the right program
- `nested_program_without_end_program_terminator` — unterminated last nested program still callable

#### IDE — PowerCOBOL-style modal event code editor — Phase 5 complete

The inline 6-row TextEdit in the Properties panel is replaced by a full-screen modal
editor modelled on Fujitsu PowerCOBOL's behaviour:

- Clicking any event row (in either the control Properties or the Form Properties
  Events section) opens a centred `egui::Window` overlay
- The modal renders a read-only COBOL scaffold around two editable areas:
  - **WORKING-STORAGE SECTION** — local data items specific to this handler
    (e.g. `01 WS-MY-VAR PIC X(64) VALUE SPACES.`)
  - **PROCEDURE DIVISION body** — the user's COBOL statements
- Read-only scaffold lines are colour-coded (green for structural keywords, gray
  for division headers); editable areas use monospace 12pt with syntax hint text
- **Save** commits both `local_ws` and `code` to the model (dirty-flagged);
  **Cancel** discards changes and closes without writing
- A semi-transparent black overlay dims the canvas behind the modal
- `EventEditorModal` struct added to `designer.rs` with `ctrl_id`, `ctrl_display`,
  `event_name`, `program_id`, `ws_buf`, `proc_buf`, `orig_ws`, `orig_proc`, `saved`
- `DesignerPanel::open_event_modal(ctrl_id, event_name)` — opens the modal,
  pre-populating buffers from the model (or blank if the event has no binding yet)
- `DesignerPanel::save_event_handler(ctrl_id, event_name, ws, code)` — writes
  both buffers back into the form, for either control or form-level events
- `DesignerPanel::show_event_modal(ui)` — renders the modal; called at the end
  of `show()` so it floats above all other content

**Model** — `EventBinding` gains `local_ws: String` for per-handler WS declarations;
XML layer extended with `<LocalWS><![CDATA[...]]></LocalWS>` child element inside
`<Event>` (backward compatible: old files without `<LocalWS>` still load correctly);
codegen updated to emit `local_ws` content in the handler's WS section instead of a
placeholder comment.

**Properties panel**
- `selected_event` and `event_code_bufs` fields removed
- `InspectorAction::set_event_code` replaced by `open_event_editor: Option<(String, String)>`
  containing `(ctrl_id, event_name)`; empty `ctrl_id` = form-level event
- Form Properties section gains "⚡ Form Events" subsection with clickable `OnLoad` /
  `OnClose` rows that open the same modal

---

## [0.2.2] — 2026-05-29

### Fix — Chart SET-TABLE generates invalid COBOL when DataSource/DataCount not set

`write_chart_stubs()` used `.map().unwrap_or_else(fallback)` to default empty
DataSource / DataCount properties, but if the property exists as an empty string
`Some("")`, `unwrap_or_else` never fires.  The result was invalid generated COBOL:

```cobol
           MOVE         TO WS-LIN-13-SELECTED-IDX        *> missing source
           CALL "COBOL-CHART-SET-TABLE" USING "LIN-13"   *> missing args
```

Fix: added `.filter(|s| !s.is_empty())` before `unwrap_or_else` so empty strings
fall through to the placeholder-name fallback (`WS-<ID>-TABLE` / `WS-<ID>-COUNT`).
Generated code now compiles cleanly even when the chart has no data binding configured.

---

## [0.2.1] — 2026-05-29

### Fix — Runtime COBOL-* built-in calls not recognised (warn + infinite loop)

After task 64 renamed all generated identifiers from `COBOLT-*` to `COBOL-*`, the
cobolt interpreter's `match` still only recognised the old `COBOLT-WAIT-EVENT` /
`COBOLT-SET-PROPERTY` / `COBOLT-GET-PROPERTY` spellings.  Every generated form
program therefore hit `CALL to unknown program 'COBOL-WAIT-EVENT' — ignored` on
startup, and the event loop would spin forever in CLI mode.

Changes to `cobolt-runtime/src/interpreter.rs`:

- Added `"COBOL-INIT-FORM"` arm — no-op in CLI/non-GUI mode (suppress spurious warn)
- Renamed `"COBOLT-WAIT-EVENT"` → `"COBOL-WAIT-EVENT"` (old spelling kept as alias)
- **`COBOL-WAIT-EVENT` now sets `COBOL-QUIT = 1`** so the event loop exits cleanly
  in CLI mode instead of spinning until the process is killed
- Added `"COBOL-SET-PROPERTY"` / `"COBOL-GET-PROPERTY"` as primary spellings (old
  `COBOLT-*` aliases retained for backward compatibility)
- Added `"COBOL-CHART-SET-TABLE"`, `"COBOL-CHART-ADD-POINT"`, `"COBOL-CHART-CLEAR"`,
  `"COBOL-CHART-REFRESH"` stubs — log at DEBUG level in CLI mode, no warning

---

## [0.2.0] — 2026-05-29

### New feature — Rich chart controls

Six chart control types added to the Form Designer toolbox under a new **Charts**
category.  Charts are first-class form controls that participate in the full designer
workflow: placement on the canvas, property inspection, COBOL code generation, and
XML persistence.

**Control types added**

- `BarChart` — vertical bar chart; default size 320 × 220
- `LineChart` — line/trend chart; default size 320 × 220
- `PieChart` — pie chart; default size 240 × 240
- `AreaChart` — filled area chart; default size 320 × 220
- `ScatterChart` — scatter-plot chart; default size 320 × 220
- `DonutChart` — donut / ring chart; default size 240 × 240

**Data binding**

Charts accept data via two complementary mechanisms:

1. **COBOL table binding** — pass an existing WORKING-STORAGE table and its element
   count directly:
   ```cobol
   INVOKE CHART1 SET-TABLE USING WS-SALES-TABLE WS-SALES-COUNT
   ```
2. **Point-by-point accumulation**:
   ```cobol
   INVOKE CHART1 ADD-POINT USING 'January' WS-MONTHLY-TOTAL
   INVOKE CHART1 CLEAR
   INVOKE CHART1 REFRESH
   ```

**Properties inspector** — dedicated chart section covering:

- *Visual*: Title, ShowLegend, ShowGridLines, ShowTooltips, AnimateOnLoad,
  X-axis / Y-axis labels
- *Data Binding*: DataSource, DataCount, LabelField, ValueFields, SeriesLabels
- *Type-specific*: grouped/stacked bars, smooth/stepped lines, inner-radius for
  donut, log-scale Y axis, bubble size for scatter, fill-opacity for area
- *COBOL Paragraphs*: DataChanged event paragraph stub
- *INVOKE usage hint* displayed inline

**Designer canvas** — glass-styled chart previews rendered with sample data at
design time (bars, polylines, filled polygons, scatter dots, pie/donut fan slices).

**Code generation**

- `WORKING-STORAGE SECTION` — three items per chart:
  `WS-<ID>-SELECTED-IDX` (PIC 9(4)), `-SELECTED-LBL` (PIC X(64)),
  `-SELECTED-VAL` (PIC 9(12)V99)
- `PROCEDURE DIVISION` — four stub paragraphs per chart:
  `<ID>-SET-TABLE`, `<ID>-ADD-POINT`, `<ID>-CLEAR`, `<ID>-REFRESH`

**Toolbox** — hand-drawn vector icons for all six chart types; unique ID prefixes
(`BAR`, `LIN`, `PIE`, `ARE`, `SCT`, `DNT`).

---

## [0.1.0] — 2026-05-29

### New feature — Snap-to-grid toggle

- Added `snap_to_grid: bool` field to the `Form` model (default `true`); persisted
  as a `snap-to-grid` XML attribute in `.cfrm` files (backward-compatible: missing
  attribute defaults to `true`)
- `snap()` in the designer canvas is now dynamic — it takes `grid_px` and `enabled`
  parameters instead of using a hardcoded 4 px constant; all move/resize/place
  operations respect the per-form setting
- Added **"Snap to grid"** checkbox to the Grid section of Form Properties (sits
  directly below "Grid size"); checking/unchecking takes effect immediately for
  move, resize, and new-control placement
- Updated all `Form` struct literals in test/codegen code to include
  `snap_to_grid: true`

Versioning rules
- **PATCH** (`0.0.x`): bug fixes, polish, build corrections
- **MINOR** (`0.x.0`): new features — resets PATCH to 0
- **MAJOR** (`x.0.0`): any change to the interpreter — resets MINOR and PATCH to 0

---

## [0.0.1] — 2026-05-29  *(initial tagged release)*

### Foundation (pre-tag, post-parser)

All work below was completed before the 0.0.1 tag was applied.
It is catalogued here as the baseline feature set.

---

#### Runtime & Toolchain

- **cobolt-semantic** — semantic analysis crate scaffolded; identifier resolution and
  basic type checking
- **cobolt-runtime / interpreter** — tree-walking interpreter for all AST statement
  types including `Stmt::TryCatch` and `Stmt::Throw` (try/catch/finally semantics,
  `UserException` error variant, exception variable binding)
- **cobolt-stdlib** — standard-library crate with built-in COBOL helper functions
- **cobolt-cli** — command-line binary (`cobolt run <file>`) wrapping the interpreter
- **INVOKE keyword** — added `Token::Invoke` to the lexer and a pass-through
  `Stmt::Invoke` to the parser; codegen emits `INVOKE` correctly
- **PLAY / STOP animation verbs** — `PLAY ANIMATION` / `STOP ANIMATION` statements
  added to lexer and parser
- **TRY / CATCH EXCEPTION / FINALLY** — full exception-handling block added to
  lexer and parser; interpreter executes all three clauses with correct fall-through

---

#### IDE Shell (`cobolt-ide`)

- **eframe/egui shell** — main application window with liquid-glass translucent
  visuals, dark-navy palette, rounded widgets, and frosted-glass panel fills
- **macOS dock icon** — programmatically generated 256×256 navy rounded-square
  with a blue "C" arc and terminal serifs
- **Code editor panel** — scrolling source editor, syntax-aware font (12 pt
  monospace), auto-completion stubs, search/replace with focus-restore fix
- **Output / console panel** — scrolling log for run output and diagnostics
- **Project system** — `cobolt.toml` project file, project explorer panel with
  grouped tree view (forms, sources, assets), new-project dialog
- **Run / stop** — background thread runner, real-time output streaming,
  diagnostic markers fed back into the editor
- **Keyboard shortcut handling** — Cmd/Ctrl+S save, Cmd/Ctrl+Z undo,
  Cmd/Ctrl+Shift+Z redo wired globally

---

#### Form Designer

- **cobolt-forms model** — `Form`, `Control`, `ControlRect`, `PropValue`,
  `Animation`, `AnimTrigger`, `AnimEasing`, `BgImageMode` data types;
  XML serialisation/deserialisation (`cobolt-forms/src/xml.rs`)
- **cobolt-codegen** — form-to-COBOL source generator; REST-API stub codegen;
  DataGrid CSV-export stubs; full PROCEDURE DIVISION with all control paragraphs
- **Multi-viewport designer windows** — each open `.cfrm` file gets its own OS
  window via `ctx.show_viewport_immediate`
- **Canvas** — pixel-accurate form canvas with dot grid (configurable density),
  drag-to-place, drag-to-move, rubber-band multi-select, snap-to-grid
- **Control types (29 total)**:
  Button, Label, TextBox, CheckBox, RadioButton, ComboBox, ListBox,
  NumericUpDown, DateTimePicker, GroupBox, Panel, TabControl, Splitter,
  DataGrid, TreeView, PictureBox, ProgressBar, Slider, Line, Shape,
  MenuBar, ToolBar, StatusBar, Timer, AgentObject, RestClient,
  SqlDatabase (non-visual), ModalWindow
- **Vector icon toolbox** — two-column icon grid with hand-drawn vector icons for
  every control type, collapsible categories, live search filter;
  buttons enlarged to 39 × 39 px with 5 px top/right padding
- **Properties inspector** — two-column table layout; universal properties
  (Name, Caption, Position, Size, Font, Colors, Opacity, Transparency, Enabled,
  Visible, Z-Order); per-type sections for every control type;
  `SqlDatabase` connection properties (driver, host, port, database, user,
  password, auto-connect, max connections); panel width capped at 320 px to
  prevent overflow
- **Forms list panel** — sidebar list of all `.cfrm` files in the project root,
  open-on-click
- **Undo / redo stack** — full snapshot-based undo/redo for all designer mutations
- **Alignment toolbar** — align left/right/top/bottom/center-H/center-V,
  bring-to-front/send-to-back, delete selected; double-height toolbar
- **Z-order** — per-control z_order field; `Bring to Front` / `Send to Back`
  commands; canvas renders controls in z-order
- **Multi-select** — rubber-band selection, Shift+click toggle, group move
- **Form background** — solid fill colour (hex picker), transparency slider (0–100 %),
  background image path + stretch/tile/center/fit display modes
- **Grid density** — grid size property (8/16/32 px) on the Form, adjustable in
  Form Properties
- **Animation system** — per-control animation list; properties: name, trigger
  (`OnFormLoad`, `OnClick`, `OnHover`), easing, direction, duration, delay,
  loop count; designer-time live preview with play/stop controls;
  `AnimState` struct tracks t, playing, forward, delay_remaining
- **Preview window** — live OS window (`with_transparent(true)`) showing the form
  with liquid-glass control rendering, per-control opacity/transparency, and
  `OnFormLoad` animations auto-started on open; glass visuals applied to preview
  viewport; main designer visuals restored every frame to prevent bleed-through
- **Delete key guard** — Delete/Backspace only removes selected controls when no
  text-input widget has keyboard focus (`ctx.memory focused().is_none()`)
- **Target device presets** — "Target" dropdown in Form Properties with 24 device
  presets (iPhone, iPad, Apple Watch, Android phone/tablet/watch, custom);
  selecting a preset auto-sets form width × height
- **COBOL identifier rename** — `COBOLT-*` data-division identifiers renamed to
  `COBOL-*` throughout codegen and semantic crates

---

*Next version: increment PATCH for fixes, MINOR for new features,
MAJOR for interpreter changes.*
