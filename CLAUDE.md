<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Agent Context

This file is the single source of truth for any AI agent continuing development
on this project. Read it fully before touching any code.

---

## Product Names (CRITICAL — never use "cobolt" in user-facing text)

| Name | Role |
|------|------|
| **RustCOBOL** | The language / compiler |
| **PowerRustCOBOL** | The RAD IDE (desktop app) |
| **rcrun** | The CLI runtime binary |

Internal crate names (`cobolt-lexer`, `cobolt-runtime`, etc.) are build-only
and are **not** user-facing — do NOT rename them.

---

## CRITICAL Constraint

**COBOL data-item names, paragraph names, and all generated COBOL source code
must always remain in English, regardless of the selected UI language.**
The i18n system translates the IDE interface only.

---

## GOLDEN RULES (never break these)

1. **No pushing during Brazilian work hours.** NEVER `git push` between **09:00
   and 18:00 (America/Sao_Paulo), Monday–Friday**, except on Brazilian national
   holidays — even if explicitly asked. Outside those hours, pushing is allowed
   without asking. (Enforced by a PreToolUse hook.)

2. **User-provided tests: report-or-fix, never unilaterally extend.** When a test
   *the user provides* fails:
   - If it fails because it uses **syntax that is NOT already implemented** (it
     does not match the grammar / feature support already in the codebase), **do
     nothing to the implementation — just report the problem** clearly so the
     user can provide a fix or decide. Do **NOT** invent or extend the
     language/grammar on your own to make the test pass.
   - If the test's **syntax is correct** (valid and within already-supported
     scope) **but our side fails** (lexer / parser / runtime bug), then **fix our
     side**.

   In short: a feature/grammar *gap* → report and wait; a *bug* handling
   correct, in-scope syntax → fix it.

---

## Version Convention  x.y.z

- **x** — new platform component added (Web/WASM, Android, iOS, etc.)
- **y** — new features: new widgets, properties, IDE panels, language features
- **z** — bug fixes, polish, performance

Current version: **1.6.0**

---

## Project Layout

```
PowerRustCOBOL/
├── Cargo.toml                  ← workspace root
├── CHANGELOG.md                ← versioned changelog
├── BUGS.md                     ← bug tracker (open + resolved)
├── tools/
│   └── check_bugs.sh           ← automated cargo check + BUGS.md updater
├── crates/
│   ├── cobolt-lexer/           ← COBOL tokenizer (fixed + free form)
│   ├── cobolt-ast/             ← AST types (Serialize/Deserialize already derived)
│   ├── cobolt-parser/          ← recursive-descent parser
│   ├── cobolt-semantic/        ← semantic analyser / diagnostics
│   ├── cobolt-runtime/         ← tree-walking interpreter
│   │   ├── src/interpreter.rs  ← main executor, all built-in CALLs
│   │   ├── src/db_runtime.rs   ← SQLite engine (DbRegistry)
│   │   ├── src/http_runtime.rs ← REST client (HttpClient)
│   │   ├── src/debugger.rs     ← debug channels (DebugCmd/DebugEvent)
│   │   ├── src/channels.rs     ← FormEvent / StateUpdate GUI channels
│   │   ├── src/files.rs        ← RecordLayout (materialize/distribute), KeySpec
│   │   ├── src/indexed.rs      ← INDEXED/ISAM engine (IndexedFile, IndexedEngine)
│   │   ├── src/numedit.rs      ← numeric-edited PICTURE edit engine
│   │   └── src/copybook.rs     ← COPY / REPLACE preprocessor
│   ├── cobolt-stdlib/          ← standard library stubs
│   ├── cobolt-forms/           ← .cfrm form model + XML serialization
│   │   └── src/model.rs        ← Control, Form, ControlType, animations, props
│   ├── cobolt-codegen/         ← Form → RustCOBOL source generator
│   ├── cobolt-compiler/        ← embed+bundle binary compiler (Phase 11)
│   │   └── src/lib.rs          ← build_project(), AST serialization pipeline
│   ├── cobolt-cli/             ← rcrun CLI binary
│   │   └── src/main.rs         ← run/check/build/package commands
│   └── cobolt-ide/             ← PowerRustCOBOL desktop app (egui/eframe)
│       ├── src/main.rs         ← window title "PowerRustCOBOL v{VERSION}"
│       ├── src/version.rs      ← VERSION constant
│       ├── src/app.rs          ← CoboltApp, update loop, all dialog state
│       ├── src/form_runtime.rs ← FormRuntime (live form interpreter thread)
│       ├── src/runner.rs       ← Runner + DebugRunner background threads
│       ├── src/i18n.rs         ← Tr struct, Language enum, 5 languages
│       ├── src/project_model.rs ← CoboltProject, package_project()
│       └── src/panels/
│           ├── designer.rs     ← DesignerPanel, canvas, draw_control(),
│           │                      glass_combo_header/popup, draw_chart_preview
│           ├── editor.rs       ← CodeEditor, breakpoint gutter
│           ├── properties.rs   ← properties inspector
│           ├── toolbox.rs      ← drag-and-drop widget toolbox
│           ├── debugger.rs     ← DebuggerPanel (var watch, step controls)
│           ├── output.rs       ← OutputPanel
│           ├── project.rs      ← ProjectPanel (file tree / project mode)
│           ├── toolbar.rs      ← main IDE toolbar (rcrun run/stop/check)
│           └── forms_list.rs   ← forms list sidebar
```

---

## Architecture Decisions

### Multi-viewport (egui 0.29)
Each open form designer and each running form lives in its **own OS window**
via `ctx.show_viewport_immediate()`. All viewports share one egui Context.

### Channels for cross-thread communication
- `FormEvent` / `StateUpdate` — GUI ↔ interpreter (Run Form)
- `DebugCmd` / `DebugEvent` — debugger panel ↔ interpreter
- `display_tx` — DISPLAY output from interpreter to IDE output panel

### RustCOBOL built-in CALLs (interpreter.rs exec_call)
**SQL (Phase 8):**
`COBOL-OPEN-DB`, `COBOL-EXEC-SQL`, `COBOL-FETCH-ROW`,
`COBOL-NEXT-ROW`, `COBOL-ROW-COUNT`, `COBOL-CLOSE-DB`

**HTTP (Phase 10):**
`COBOL-HTTP-GET`, `COBOL-HTTP-POST`, `COBOL-HTTP-PUT`,
`COBOL-HTTP-DELETE`, `COBOL-HTTP-SET-HEADER`, `COBOL-HTTP-CLEAR-HEADERS`

**GUI (Phase 6):**
`COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`

### Form Designer — Custom Glass ComboBox
`glass_combo_header()` and `glass_combo_popup()` in `designer.rs` are
`pub(crate)` shared utilities used by **both** Preview and Run Form.
The system `egui::ComboBox` is NOT used in those surfaces.
State is stored in `DesignerPanel::preview_combo_open` and
`FormRuntime::combo_open` respectively.

### Glass / Liquid UI Theme
`apply_glass_visuals(ctx)` in `app.rs` sets the glass theme every frame.
`draw_glass()` and `draw_glass_circle()` in `designer.rs` are the primitive
painters used everywhere. The form preview/run windows use
`.with_transparent(true)` + `clear_color = [0,0,0,0]` so the OS desktop
shows through the form background.

### Binary Compiler (Phase 11)
`cobolt_compiler::build_project()` serializes the AST with `bincode`+`flate2`,
generates a temp Cargo project that embeds everything via `include_bytes!`,
runs `cargo build --release`, and copies the binary to `bin/`.

### File I/O — verb dispatch by ORGANIZATION (Phase 12)
**CRITICAL:** file verbs (`OPEN`/`CLOSE`/`READ`/`WRITE`/`REWRITE`/`DELETE`/
`START`) are NOT hard-wired to one file type. Each is dispatched by the file's
declared `ORGANIZATION` (from its `SELECT`), so SEQUENTIAL / LINE SEQUENTIAL /
INDEXED share the common verbs while each keeps its own semantics. RELATIVE is
planned. Dispatch lives in `interpreter.rs` (`OpenFile` enum:
`Reader`/`Writer`/`Indexed`; `exec_open/close/read/write/rewrite/delete/start`).

- `cobolt-runtime/src/files.rs` — `RecordLayout` (`compute_layout`) gives each
  FD record's byte layout; `materialize(env)` builds the record buffer from
  subfields (WRITE/REWRITE) and `distribute(env, buf)` scatters it back (READ).
  Key fields → `KeySpec { offset, len, duplicates }`.
- `cobolt-runtime/src/indexed.rs` — the in-memory **INDEXED (ISAM) engine**
  (`IndexedFile`, `STORAGE IS MEMORY`): `BTreeMap` primary store +
  alternate-key indexes, ascending key order, journaled `commit`/`rollback`,
  record locking, `status` module (00/02/10/22/23/35/39/…). No external deps.
  Defines the `IndexedStore` trait both backends implement.
- `cobolt-runtime/src/indexed_disk.rs` — the **persistent paged on-disk B+tree
  engine** (`DiskIndexedFile`, `STORAGE IS DISK`, container `PRCIDXD1`):
  4 KiB pages + free list, one B+tree per key (split-on-insert, doubly-linked
  leaves), a RecordId directory, slotted data pages + overflow chain. Records
  read on demand → bounded RAM. Lazy index delete (data pages reclaimed).
- `cobolt-runtime/src/compress.rs` — `WITH COMPRESSION` (PackBits RLE, raw
  fallback, no deps); used by both storage modes.
- **`STORAGE [MODE] IS MEMORY | DISK [WITH [DATA] COMPRESSION|COMPRESSING]`** — a
  SELECT clause (PowerRustCOBOL extension) on
  `FileControl.storage_mode`/`.data_compressing`; `ASSIGN TO` still required.
  `MODE` optional; compression accepts `COMPRESSION` or `COMPRESSION`, and a
  standalone `WITH COMPRESSION` (no STORAGE clause) is allowed. **Default storage
  (no STORAGE clause) = DISK** (`StorageMode` derives `#[default] Disk`; the
  parser inits to `Disk`). `make_indexed_engine` dispatches by `StorageMode` to a
  `Box<dyn IndexedStore>`. Parser also handles the spaced `ALTERNATE RECORD KEY …
  [WITH DUPLICATES]` form.
- **Duplicate-alternate WRITE returns `00`** (not the informational `02`): a
  write creating a duplicate on a `WITH DUPLICATES` alternate is fully
  successful; `WITHOUT DUPLICATES` violations return `22`. (Both engines.)
- **Read dispatch by ORGANIZATION:** record `SEQUENTIAL` reads exactly
  `record_len` bytes per `READ`; `LINE SEQUENTIAL` reads a newline-delimited line.
  `rcrun`'s `detect_format` treats content past column 72 as free-form.
- **File I/O test pack:** `tests/cobol/fileio/` (baseline + 6 storage/compression
  variants), driven by `crates/cobolt-runtime/tests/test_fileio_storage.rs`; the
  vendored `.cbl` use `*>` free-form comments.
- **On-disk format `PRCIDX1`** — self-describing container: header + full key
  schema (`IndexedFileInfo`/`KeyDescriptor`: composite byte-ranged parts,
  `KeyEncoding`, `KeyOrdering`, duplicates, COBOL field name) + records + CRC-32.
  Models Fujitsu `cobfa_indexinfo()` metadata so a **future Fujitsu importer**
  (out of scope) can write faithful files. Legacy records-only `PRCISAM1` still
  reads (upgraded on next write). `IndexedFile::inspect_path()` discovers the
  schema without I/O. Strict OPEN validation → FS 39 (schema mismatch) / 35
  (missing INPUT) / 90 (CRC). NOT byte-compatible with Fujitsu. Spec:
  `docs/indexed-file-format.md`. NEVER claim Fujitsu binary compatibility.
- **Engine selection:** `IndexedEngine { Rust, RmCobol85, Fujitsu }`, chosen by
  `rcrun --indexed-engine <name>` (or `-I`) / `COBOL_INDEXED_ENGINE` env, default
  `rust`. `Interpreter::set_indexed_engine()`. rm/fujitsu currently delegate to
  the Rust container (behaviour-identical) until native formats land. NEVER
  mention CICS in code or user-facing text (the locking model is VSAM/RLS-style).
- `READ … NEXT/PREVIOUS` = sequential; unqualified `READ` = random (by RECORD
  KEY) under RANDOM/DYNAMIC. `INVALID KEY`/`NOT INVALID KEY` phrases parse on
  `READ`/`WRITE`/`REWRITE`/`DELETE`/`START` (`Stmt` fields + `run_key_outcome`).
- Tests: the comprehensive **File I/O suite** `tests/cobol/fileio/` (baseline +
  6 storage/compression variants) driven by
  `crates/cobolt-runtime/tests/test_fileio_storage.rs`; focused engine tests in
  `crates/cobolt-runtime/tests/test_indexed.rs`; unit tests in `indexed.rs` /
  `indexed_disk.rs`. (The old `indexed-files/` suite was consolidated into
  `fileio/`.)

### COBOL-85 language coverage (1.5 / 1.6)
The ground-truth reference is **`docs/cobol85-supported-syntax.md`** (read it
before claiming a verb works); the test plan is `docs/cobol85-verb-test-matrix.md`.

- **Occurrence-aware environment** (`environment.rs`) — `CobolEnvironment` keeps a
  per-item symbol table (`ItemSym`: OCCURS `dims`, group `children` + canonical
  `child_keys`, ancestor `quals`, `index_names`) plus a duplicate-name index.
  `resolve_name(name, quals)` maps a (possibly `OF`/`IN`-qualified) reference to a
  **canonical storage key** — unique names key by themselves (flat-store fast
  path); duplicated leaf names get a path-qualified key so `BALANCE OF ACCOUNT`
  ≠ `BALANCE OF SUMMARY`. Subscripts → `subscript_key` (`NAME(i[,j])`, lazy slots).
- **88-level condition-names are real** — `cond_names` maps each 88 → `CondName {
  parent, values }`. `IF 88-name` tests the host against its VALUEs/THRU ranges;
  `SET 88-name TO TRUE/FALSE` writes a satisfying/violating value to the host
  (handled in `exec_move`/`set_condition`). 88s are NOT data-item slots.
- **Control-flow signals** (`error.rs`): `ExitPerform{cycle}` (caught by inline
  PERFORM loops via `exec_loop_body`/`LoopStep`), `ExitParagraph`/`ExitSection`
  (absorbed by `exec_para_body` + the run loops). `NEXT SENTENCE` ≈ `CONTINUE`
  (no sentence-boundary tracking). Plain `EXIT` = no-op; `EXIT PROGRAM` = GoBack.
- **SORT/MERGE** — in-memory `sort_buffers`/`sort_cursors`; `exec_release` appends
  a materialized SD record, `exec_sort` fills (INPUT PROCEDURE or USING),
  `sort_records` stable-sorts by keys (`cob_ordering`), then delivers (OUTPUT
  PROCEDURE via `exec_return`, or GIVING via `write_all_records`). SD files use the
  same `FileSpec.layout` machinery as FDs.
- **Arithmetic receivers** are `Vec<(Expr, bool)>` (per-receiver `ROUNDED`);
  `giving` is a list. `INSPECT` carries an `InspectRegion` (BEFORE/AFTER INITIAL)
  per phrase. `EVALUATE` has `subjects: Vec<…>` (ALSO) and `WhenValue::Not`.
- **Not implemented (don't claim otherwise):** `SET ADDRESS OF`/pointers,
  `66 RENAMES`, `INITIALIZE … REPLACING`, identifier-object abbreviation
  (`a = b OR c`), faithful `NEXT SENTENCE`. `UNLOCK`/`ALTER` are recognized no-ops.
- Tests: `test_hierarchy`, `test_arith_receivers`, `test_control_flow`,
  `test_inspect`, `test_intrinsics_date`, `test_conditions`, `test_sort`
  (all in `crates/cobolt-runtime/tests/`).

---

## i18n Keys (Tr struct in i18n.rs)
All 5 languages (EN/ES/PT/JA/ZH) must have every key. When adding a new key:
1. Add `pub field_name: &'static str` to `struct Tr`
2. Add the value in all 5 language blocks (`tr_english`, `tr_spanish`, etc.)

---

## Form File Format (.cfrm)
XML serialized by `cobolt_forms::save_form()` / `load_form()`.
Key types: `Form`, `Control`, `ControlType`, `PropValue`, `EventBinding`,
`AnimationDef`, `AnimTrigger`, `BgImageMode`.

**Caption property rules:**
- Only Label, Button, CheckBox, RadioButton, GroupBox have Caption
- TextBox uses "Text"
- All other controls use control-type-specific props ("Value", "Items", etc.)

---

## Pending Tasks

| # | Task | Notes |
|---|------|-------|
| ~~69~~ | ✅ **DONE** — form canvas resize by dragging border | `designer.rs`: `DragState::ResizingForm` + `FormEdge{Right,Bottom,Corner}`, `detect_form_edge()`/`form_edge_cursor()`, `press_form_edge` capture, live `form.width/height` update with grid snap + `FORM_MIN_SIZE` clamp, visible grips via `draw_form_resize_grips()`. Tested: `form_resize_tests`. |
| ~~70~~ | ✅ **DONE** — double-click event para name → jump to COBOL editor | Event row in `properties.rs` now reports `(clicked, double_clicked)`; double-click sets `InspectorAction::open_event_in_code`. `app.rs::jump_to_event_code()` resolves the paragraph (binding or `derive_paragraph_name`), regenerates+opens the `.cbl`, queues `pending_goto_paragraph`; `editor.rs::goto_paragraph()` scrolls to the paragraph/PROGRAM-ID definition (reusing search-scroll). i18n key `hint_dblclick_event` (5 langs). Tested: `goto_tests`. |
| ~~129~~ | ✅ **DONE** — preview animations apply `scale` from anim_transform to rect | `show_preview_window` now scales the rect about its centre via the shared `designer::scale_rect_about_center()` (also used by `draw_control`), so zoom/spin/flip resize widgets in preview. Tested: `anim_behavior_tests::scale_rect_shrinks_and_grows_about_centre`. |
| ~~140~~ | ✅ **DONE** — DateTimePicker interactive **calendar popup** at runtime | Implemented in `render_run_control` (field → month-grid popup via `egui::Area`; nav ◀▶; day click sets `Value` + fires `Change`). Tested: `run_interaction_tests::datetimepicker_calendar_opens_and_picks_a_day`. |
| ~~141~~ | ✅ **DONE** — DataGrid **runtime cell rendering** with typed values | `render_run_control` parses `Columns` ("Name:Type") + `Rows` (TAB-separated; new prop) and paints a header + typed cells: string=left, number=right-aligned, datetime=reformatted "DD Mon YYYY", **image=loaded texture (`load_image_texture`, cached in egui memory)**, with alternating rows + grid lines. Tested: `run_interaction_tests::datagrid_renders_typed_cells` + `datagrid_renders_image_cells`. |
| ~~142~~ | ✅ **DONE** — runtime rendering for the remaining widgets | Added to `render_run_control`: RadioButton + NumericUpDown (interactive), TabControl (clickable tabs), TreeView (indented items), Splitter, MenuBar/ToolBar/StatusBar (item bars), and all 6 charts (reusing `draw_chart_preview` via a state→`Control` rebuild). Routed through the shared arm in `show_running_form_window`. Tested in `run_interaction_tests` (radiobutton/tabcontrol interaction + numericupdown/menubar/treeview/chart render). |

> **Testing:** `tests/widgets/` has property round-trip tests for all 34 widgets.
> Behavioral tests live in `cobolt-ide` (`cargo test -p cobolt-ide`):
> design-time render (`render_behavior_tests`), animations (`anim_behavior_tests`),
> i18n (`i18n_tests`), and runtime interaction (`run_interaction_tests`, driving the
> shared `render_run_control`).

> The unified 50px Form Designer icon toolbar is **done** — implemented as
> `designer.rs::draw_icon_toolbar()` and mounted from the `app.rs` "dtb_{idx}"
> `TopBottomPanel`. The old `draw_toolbar()` and `show_toolbar` field have been
> removed.

---

## Build Instructions

```bash
# Build everything
cargo build

# Run PowerRustCOBOL IDE
cargo run -p cobolt-ide

# Run rcrun CLI
cargo run -p cobolt-cli -- run myprogram.cbl

# Check for compiler errors (updates BUGS.md)
./tools/check_bugs.sh
```

**Note:** The `target/` directory is ~1.5 GB of build artifacts.
Run `cargo clean` to remove it. The source code itself is ~50 MB.

---

## How to Use with Claude Code (no zip needed)

Claude Code works directly on the local filesystem — no zip or upload required.

```bash
# Install Claude Code if not already installed
npm install -g @anthropic-ai/claude-code

# Navigate to project and launch
cd /Users/emersonlopes/Documents/PowerRustCOBOL
claude
```

The 1.6 GB size is almost entirely `target/` (Rust build cache).
`cargo clean` reduces the project to ~50 MB.
To create a minimal archive: `cargo clean && tar -czf PowerRustCOBOL.tar.gz .`

---

## Key Contacts / Repo
- Developer: Emerson Lopes (emersonlopes@gmail.com)
- Repo placeholder: https://github.com/yourusername/cobolt
  (update when real repo is created)
