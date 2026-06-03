<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Cobolt IDE — Full Architecture Plan
### A Rust-based reimplementation of modern COBOL loosely inspired by Fujitsu PowerCOBOL 3.0

> **Project name:** Cobolt (COBOL + Bolt — fast, modern, cross-platform)  
> **License:** MIT / Apache-2.0 dual  
> **Targets:** Windows x64, macOS (x64 + ARM), Linux x64  
> **Runtime model:** 64-bit interpreted (tree-walking)  
> **IDE toolkit:** egui (pure Rust, immediate-mode)  
> **Repository layout:** Cargo workspace (monorepo)

---

## 1. Vision & Goals

Cobolt is a spiritual successor to the best COBOL compiler of all times, the Fujitsu PowerCOBOL 3.0. It gives COBOL developers a modern RAD (Rapid Application Development) environment that:

- Accepts standard COBOL 85 and some extensions of COBOL 2002 source code
- Provides a drag-and-drop visual form designer that generates COBOL code
- Runs COBOL programs through a 64-bit interpreted runtime written entirely in Rust.
- Ships a single native binary on Windows, macOS, and Linux with no external runtime dependencies.
- Supports community-authored extensions (controls, runtime modules) via a stable Rust plugin API.

---

## 2. Repository Structure

```
cobolt/
├── Cargo.toml                  ← workspace manifest
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
│
├── crates/
│   ├── cobolt-lexer/           ← COBOL tokenizer
│   ├── cobolt-ast/             ← AST node types (shared crate)
│   ├── cobolt-parser/          ← Parser: token stream → AST
│   ├── cobolt-semantic/        ← Semantic analysis & symbol table
│   ├── cobolt-runtime/         ← 64-bit tree-walking interpreter
│   ├── cobolt-stdlib/          ← Built-in COBOL functions & I/O
│   ├── cobolt-forms/           ← Form/control data model
│   ├── cobolt-codegen/         ← Form model → COBOL source generator
│   ├── cobolt-plugin-api/      ← Stable C-ABI plugin interface
│   ├── cobolt-plugin-loader/   ← Dynamic library loader
│   └── cobolt-ide/             ← egui IDE application (main binary)
│
├── plugins/
│   └── example-plugin/         ← Reference plugin (Rust)
│
├── tests/
│   ├── cobol-suite/            ← .cbl test programs
│   └── ui-tests/               ← egui snapshot tests
│
└── .github/
    └── workflows/
        └── ci.yml              ← matrix build: Windows / macOS / Linux
```

### Crate dependency graph

```
cobolt-ide
  ├── cobolt-forms
  │     └── cobolt-codegen
  ├── cobolt-runtime
  │     ├── cobolt-semantic
  │     │     ├── cobolt-parser
  │     │     │     ├── cobolt-lexer
  │     │     │     └── cobolt-ast
  │     │     └── cobolt-ast
  │     └── cobolt-stdlib
  ├── cobolt-plugin-loader
  │     └── cobolt-plugin-api
  └── cobolt-plugin-api
```

---

## 3. COBOL Interpreter Pipeline

### 3.1 Lexer (`cobolt-lexer`)

The lexer converts raw UTF-8 source text into a flat stream of typed tokens.

**Key design choices:**
- Built with the `logos` crate for maximum tokenization speed.
- Supports both **fixed-form** (columns 7-72 active area, 1-6 sequence numbers) and **free-form** COBOL source.
- Tracks `Span { start: usize, end: usize, line: u32, col: u16 }` on every token for IDE diagnostics.

**Token taxonomy:**

| Category | Examples |
|---|---|
| Division keywords | `IDENTIFICATION`, `DATA`, `PROCEDURE`, `ENVIRONMENT` |
| Statement verbs | `MOVE`, `ADD`, `PERFORM`, `CALL`, `ACCEPT`, `DISPLAY` |
| Data keywords | `PIC`, `PICTURE`, `COMP`, `OCCURS`, `REDEFINES` |
| Literals | `"Hello"`, `42`, `3.14`, `LOW-VALUE`, `SPACES` |
| Identifiers | `WS-COUNTER`, `BUTTON1-CLICK`, `MAIN-PROC` |
| Punctuation | `.`, `,`, `(`, `)`, `-` (word separator) |
| Comments | `*` in col 7 (fixed), `*>` (free-form) |

### 3.2 AST (`cobolt-ast`)

The AST is the shared data type crate used by the parser, semantic analyzer, and interpreter.

```rust
// Top level
pub struct Program {
    pub identification: IdentificationDivision,
    pub environment: Option<EnvironmentDivision>,
    pub data: Option<DataDivision>,
    pub procedure: ProcedureDivision,
    pub span: Span,
}

// Data Division sections
pub enum DataSection {
    FileSection(Vec<FileDescription>),
    WorkingStorage(Vec<DataDecl>),
    LocalStorage(Vec<DataDecl>),
    Linkage(Vec<DataDecl>),
    Screen(Vec<ScreenControl>),
}

// Data item declaration (level 01, 05, 77, 88, etc.)
pub struct DataDecl {
    pub level: u8,
    pub name: Option<String>,       // FILLER has no name
    pub picture: Option<PicClause>,
    pub value: Option<Literal>,
    pub usage: Usage,
    pub occurs: Option<OccursClause>,
    pub redefines: Option<String>,
    pub children: Vec<DataDecl>,    // nested group items
    pub span: Span,
}

pub struct PicClause {
    pub template: String,           // e.g. "9(5)V99"
    pub kind: PicKind,              // Alphabetic / Numeric / Alphanumeric / Edited
    pub digits: u8,
    pub decimals: u8,
}

// Statements
pub enum Stmt {
    Move { from: Expr, to: Vec<Expr>, span: Span },
    Add { operands: Vec<Expr>, to: Vec<Expr>, giving: Option<Expr>, span: Span },
    Subtract { operands: Vec<Expr>, from: Vec<Expr>, giving: Option<Expr>, span: Span },
    Multiply { lhs: Expr, by: Expr, giving: Option<Expr>, span: Span },
    Divide { lhs: Expr, by: Expr, giving: Option<Expr>, remainder: Option<Expr>, span: Span },
    Compute { target: Expr, expr: Expr, span: Span },
    If { condition: Condition, then_stmts: Vec<Stmt>, else_stmts: Vec<Stmt>, span: Span },
    Evaluate { subject: Expr, whens: Vec<WhenClause>, other: Vec<Stmt>, span: Span },
    Perform { target: PerformTarget, span: Span },
    Call { program: Expr, using: Vec<CallArg>, returning: Option<Expr>, span: Span },
    Accept { target: Expr, from: Option<AcceptSource>, span: Span },
    Display { operands: Vec<Expr>, upon: Option<String>, span: Span },
    Open { mode: OpenMode, files: Vec<String>, span: Span },
    Read { file: String, into: Option<Expr>, at_end: Vec<Stmt>, span: Span },
    Write { record: Expr, from: Option<Expr>, span: Span },
    Close { files: Vec<String>, span: Span },
    Stop { run: bool, span: Span },
    GoBack { span: Span },
    // PowerCOBOL / Fujitsu extensions
    WindowOp { op: WindowOperation, span: Span },
    ControlSet { control: Expr, property: String, value: Expr, span: Span },
}
```

### 3.3 Parser (`cobolt-parser`)

A hand-written **recursive descent parser** (no parser generator dependency). This choice gives:
- Full control over error recovery (insert/delete token strategies).
- Better IDE integration (partial parses return partial ASTs).
- Easy addition of Fujitsu-specific syntax rules.

**Error recovery strategy:** the parser emits `Diagnostic` values and continues parsing after synchronizing to the next `.` (period), which in COBOL marks statement/paragraph boundaries.

### 3.4 Semantic Analyzer (`cobolt-semantic`)

Performs:
- **Symbol table construction** — scans the DATA DIVISION and registers every named item.
- **Reference resolution** — every identifier in the PROCEDURE DIVISION is resolved to a `DataDecl`.
- **PICTURE type inference** — determines the runtime `CobolType` of each data item.
- **Paragraph/section index** — builds a map `name → Vec<Stmt>` for PERFORM dispatch.
- **Qualification resolution** — handles `MOVE A OF B TO C` (disambiguates redefined names).

Output: a `SemanticModel { symbol_table, paragraph_map, diagnostics }` consumed by the runtime.

### 3.5 Runtime / Interpreter (`cobolt-runtime`)

A **64-bit tree-walking interpreter**. The semantic model drives execution; no bytecode compilation step is needed for the MVP.

**CobolValue type:**

```rust
pub enum CobolValue {
    Display(String),                        // PIC X, A, 9(n) DISPLAY
    Integer(i64),                           // PIC 9(n) COMP / BINARY / COMP-5
    Decimal { value: i128, scale: u8 },     // PIC 9(n)V9(m) COMP-3 / PACKED
    Float32(f32),                           // COMP-1
    Float64(f64),                           // COMP-2
    Group(IndexMap<String, CobolValue>),    // 01-level group item
    Table(Vec<CobolValue>),                 // OCCURS
    Pointer(u64),                           // POINTER / ADDRESS OF
    Boolean(bool),                          // condition-name (88-level)
    Null,                                   // uninitialized / LOW-VALUE
}
```

**Working storage model:**

```rust
pub struct RuntimeEnv {
    pub working_storage: IndexMap<String, CobolValue>,
    pub local_storage: IndexMap<String, CobolValue>,
    pub file_registry: HashMap<String, OpenFile>,
    pub call_stack: Vec<StackFrame>,
    pub return_code: i64,
    pub io: Box<dyn IoBackend>,             // swappable for tests / GUI
    pub event_bus: Arc<EventBus>,           // event dispatch
}

pub struct StackFrame {
    pub paragraph: String,
    pub pc: usize,
    pub local_overrides: IndexMap<String, CobolValue>,
}
```

**Arithmetic precision:** all `COMPUTE` and arithmetic verbs use 128-bit intermediates before rounding to target PIC precision, matching Fujitsu COBOL behavior.

**PERFORM implementation:**
- `PERFORM paragraph` → push frame, execute stmts, pop frame.
- `PERFORM VARYING` → inline loop with frame reuse.
- `PERFORM UNTIL` → loop with condition re-evaluated each iteration.
- Recursive PERFORM allowed (call stack depth limited to 4096 frames, configurable).

---

## 4. Form System

### 4.1 Form Data Model (`cobolt-forms`)

A `Form` is a structured description of the visual screen.

```rust
pub struct Form {
    pub name: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub background_color: Color,
    pub menu: Option<MenuDefinition>,
    pub controls: Vec<Control>,
    pub load_paragraph: Option<String>,
    pub close_paragraph: Option<String>,
}

pub struct Control {
    pub id: String,
    pub control_type: ControlType,
    pub rect: Rect,                         // x, y, width, height (pixels)
    pub tab_order: u32,
    pub visible: bool,
    pub enabled: bool,
    pub properties: IndexMap<String, PropValue>,
    pub events: Vec<EventBinding>,
    pub children: Vec<Control>,             // for containers (Panel, TabControl)
}

pub enum ControlType {
    // Core controls (built-in)
    Button,
    TextBox,
    Label,
    CheckBox,
    RadioButton,
    ListBox,
    ComboBox,
    GroupBox,
    Panel,
    TabControl,
    DataGrid,
    PictureBox,
    ProgressBar,
    MenuBar,
    ToolBar,
    StatusBar,
    // Plugin-provided
    Custom { plugin_id: String, control_id: String },
}

pub struct EventBinding {
    pub event: String,                      // "Click", "Change", "Load", "KeyPress"…
    pub paragraph: String,                  // COBOL paragraph to invoke
}
```

**Serialization format:** Forms serialize to/from XML (`.cfrm` extension):

```xml
<Form name="MAIN-FORM" width="800" height="600" title="My Application">
  <Control id="BUTTON1" type="Button" x="10" y="10" w="80" h="30">
    <Property name="Caption">OK</Property>
    <Event name="Click" paragraph="BUTTON1-CLICK"/>
  </Control>
  <Control id="NAME-BOX" type="TextBox" x="100" y="10" w="200" h="30">
    <Property name="MaxLength">64</Property>
    <Event name="Change" paragraph="NAME-CHANGED"/>
  </Control>
</Form>
```

### 4.2 Code Generator (`cobolt-codegen`)

Transforms a `Form` into complete COBOL source.

**Generated structure:**

```cobol
      *================================================================*
      * Generated by Cobolt IDE — DO NOT EDIT THIS SECTION             *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *--- Form control data ---
       01 BUTTON1-CAPTION         PIC X(64)  VALUE "OK".
       01 BUTTON1-ENABLED         PIC 9      VALUE 1.
       01 NAME-BOX-TEXT           PIC X(64).
       01 NAME-BOX-MAXLENGTH      PIC 9(4)   VALUE 64.
      *--- Event queue ---
       01 COBOLT-EVENT-CONTROL    PIC X(32).
       01 COBOLT-EVENT-NAME       PIC X(32).

       PROCEDURE DIVISION.
      *================================================================*
      * MAIN ENTRY POINT                                               *
      *================================================================*
       COBOLT-MAIN.
           PERFORM FORM-LOAD
           PERFORM COBOLT-EVENT-LOOP UNTIL COBOLT-RUNNING = 0
           STOP RUN.

       COBOLT-EVENT-LOOP.
           CALL 'COBOLT-WAIT-EVENT' USING
               COBOLT-EVENT-CONTROL COBOLT-EVENT-NAME
           EVALUATE COBOLT-EVENT-CONTROL
               WHEN "BUTTON1"
                   EVALUATE COBOLT-EVENT-NAME
                       WHEN "CLICK"  PERFORM BUTTON1-CLICK
                   END-EVALUATE
               WHEN "NAME-BOX"
                   EVALUATE COBOLT-EVENT-NAME
                       WHEN "CHANGE" PERFORM NAME-CHANGED
                   END-EVALUATE
           END-EVALUATE.

      *================================================================*
      * USER CODE — EDIT BELOW THIS LINE                              *
      *================================================================*
       FORM-LOAD.
           *> TODO: initialization code here
           .

       BUTTON1-CLICK.
           *> TODO: button click handler
           .

       NAME-CHANGED.
           *> TODO: text change handler
           .
```

The generator preserves any existing user code paragraphs when regenerating (merge strategy: new control stubs are appended; existing paragraphs are never overwritten).

---

## 5. IDE Application (`cobolt-ide`)

### 5.1 Panel Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  File │ Edit │ View │ Build │ Run │ Tools │ Help                 [minimize][x]│
├──────────────────────────────────────────────────────────────────────────────┤
│  [New▼][Open][Save][SaveAll]  │  [Run ▶][Stop ■][Debug 🐛]  │  [Build ⚙]    │
├────────────────┬───────────────────────────────────┬──────────────────────────┤
│ Project        │  ┌─ MAIN-FORM.cfrm ─┬─ app.cbl ─┐│  Properties              │
│ ▼ MyApp        │  │                  │            ││  ─────────────────────── │
│   ├ Forms      │  │  [Form Designer] │[Code Edit] ││  Control: BUTTON1        │
│   │  ├ MAIN    │  │  ┌────────────┐  │            ││  ─────────────────────── │
│   │  └ DIALOG1 │  │  │ ┌────────┐ │  │  01 WS-..  ││  X:       10             │
│   └ Source     │  │  │ │  OK    │ │  │  PERFORM.. ││  Y:       10             │
│       └ app.cbl│  │  │ └────────┘ │  │            ││  Width:   80             │
│                │  │  │            │  │            ││  Height:  30             │
│ ──────────     │  │  └────────────┘  │            ││  Caption: "OK"           │
│ Toolbox        │  └──────────────────┴────────────┘│  Enabled: ☑             │
│ ──────────     │                                    │  Visible: ☑             │
│ ▼ Standard     │                                    │  ──────────────────────  │
│  [Button    ]  │                                    │  Events                  │
│  [TextBox   ]  │                                    │  ──────────────────────  │
│  [Label     ]  │                                    │  Click → BUTTON1-CLICK   │
│  [CheckBox  ]  │                                    │  [+ Add event binding]   │
│  [ListBox   ]  │                                    │                          │
│  [ComboBox  ]  │                                    │                          │
│ ▼ Containers   │                                    │                          │
│  [Panel     ]  │                                    │                          │
│  [GroupBox  ]  │                                    │                          │
│  [TabControl]  │                                    │                          │
│ ▼ Plugins      │                                    │                          │
│  [...       ]  │                                    │                          │
├────────────────┴───────────────────────────────────┴──────────────────────────┤
│ Output ╱ Errors ╱ Find Results                                                │
│ [12:34] Build succeeded — 0 errors, 0 warnings                                │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Key egui Crates

| Purpose | Crate |
|---|---|
| Core UI framework | `egui` + `eframe` |
| Dockable panels | `egui_tiles` |
| Table/grid widgets | `egui_extras` |
| Syntax highlighting | `syntect` (COBOL `.sublime-syntax` definition) |
| File dialogs | `rfd` (native dialogs, cross-platform) |
| Image loading | `image` crate |
| Serialization | `serde` + `quick-xml` (forms) + `toml` (config/plugins) |
| Dynamic loading | `libloading` |
| Undo/redo | Custom command-pattern stack |

### 5.3 Form Designer Canvas

The form designer is a custom `egui` widget rendered inside a scrollable canvas.

**Interaction model:**
- **Drag from toolbox → canvas** — spawns a new control at drop position.
- **Click to select** — renders 8-handle resize border around selected control.
- **Drag selected control** — moves it; snaps to 4px grid by default (configurable).
- **Drag handles** — resizes the control.
- **Double-click** — opens the primary event paragraph in the code editor.
- **Right-click** — context menu: Cut / Copy / Paste / Delete / Bring to Front / Send to Back / Properties.
- **Ctrl+Z / Ctrl+Y** — undo/redo via command stack.
- **Tab key** — cycles selection by tab order.

**Rendering approach:**  
Each control type has a `fn render_in_designer(ui: &mut egui::Ui, ctrl: &Control, selected: bool)` implementation. These are simple egui widget approximations — not the actual runtime rendering — sufficient for layout purposes.

### 5.4 Code Editor

- A custom `egui` text editor widget with:
  - **COBOL syntax highlighting** via a bundled `syntect` scope (keywords, identifiers, literals, comments, picture clauses).
  - **Line numbers** rendered in a fixed-width gutter.
  - **Diagnostics underlines** (red for errors, yellow for warnings) sourced from the semantic analyzer, rerun on save.
  - **Go to paragraph** (Ctrl+G) — jump to any named paragraph.
  - **Find / Replace** panel (Ctrl+F / Ctrl+H).
  - **Auto-indent** on Enter after a period.
  - **Column ruler** at column 72 (for fixed-form mode) shown as a subtle vertical line.

### 5.5 Project Model

```rust
pub struct CoboltProject {
    pub name: String,
    pub root_dir: PathBuf,
    pub forms: Vec<PathBuf>,        // .cfrm files
    pub source: Vec<PathBuf>,       // .cbl files
    pub config: ProjectConfig,
}

pub struct ProjectConfig {
    pub source_format: SourceFormat, // Fixed / Free
    pub cobol_dialect: CobolDialect, // Fujitsu85 / Fujitsu2002
    pub charset: Charset,           // Ascii / Ebcdic (for file I/O compat)
    pub entry_program: Option<String>,
}
```

Project files are stored as `cobolt.toml` in the project root.

---

## 6. Plugin System

### 6.1 Plugin API (`cobolt-plugin-api`)

The plugin interface uses a **stable C ABI** so plugins compiled against one version of Cobolt continue to work with future versions, as long as the major ABI version matches.

```rust
// --- cobolt-plugin-api/src/lib.rs ---

pub const COBOLT_ABI_VERSION: u32 = 1;

/// Every plugin must export this symbol.
pub type PluginInitFn = unsafe extern "C" fn() -> *const CoboltPluginDescriptor;

#[repr(C)]
pub struct CoboltPluginDescriptor {
    pub abi_version: u32,
    pub plugin_id: *const c_char,       // unique reverse-DNS id, e.g. "com.myco.charts"
    pub display_name: *const c_char,
    pub version: *const c_char,
    pub controls: *const ControlDescriptor,
    pub control_count: usize,
    pub runtime_calls: *const RuntimeCallDescriptor,
    pub runtime_call_count: usize,
}

#[repr(C)]
pub struct ControlDescriptor {
    pub control_id: *const c_char,      // e.g. "LineChart"
    pub display_name: *const c_char,    // shown in toolbox
    pub category: *const c_char,        // toolbox group, e.g. "Data Viz"
    pub icon_png: *const u8,            // 32x32 PNG bytes embedded in plugin
    pub icon_len: usize,
    pub render_fn: DesignerRenderFn,    // draws the control in form designer
    pub default_props_fn: DefaultPropsFn,
    pub codegen_fn: CodegenFn,          // generates COBOL for this control
}

/// Called by the runtime when a COBOL CALL targets this extension's entry.
#[repr(C)]
pub struct RuntimeCallDescriptor {
    pub cobol_entry: *const c_char,     // e.g. "LINECHART-UPDATE"
    pub handler: RuntimeCallFn,
}

pub type DesignerRenderFn   = unsafe extern "C" fn(ctx: *mut EguiCtxOpaque, ctrl: *const ControlOpaque);
pub type DefaultPropsFn     = unsafe extern "C" fn() -> *const PropListOpaque;
pub type CodegenFn          = unsafe extern "C" fn(ctrl: *const ControlOpaque) -> *const c_char;
pub type RuntimeCallFn      = unsafe extern "C" fn(env: *mut RuntimeEnvOpaque, args: *const CobolValueOpaque, arg_count: usize);
```

### 6.2 Plugin Manifest (`cobolt-plugin.toml`)

```toml
[plugin]
id          = "com.example.charts"
name        = "Chart Controls"
version     = "0.2.0"
author      = "Your Name"
description = "Line, bar, and pie chart controls for Cobolt IDE"
abi_version = 1

[[controls]]
id           = "LineChart"
display_name = "Line Chart"
category     = "Data Visualization"
icon         = "assets/linechart.png"

[[controls]]
id           = "BarChart"
display_name = "Bar Chart"
category     = "Data Visualization"
icon         = "assets/barchart.png"

[[runtime_calls]]
cobol_entry = "CHART-SET-DATA"
[[runtime_calls]]
cobol_entry = "CHART-REFRESH"
```

### 6.3 Plugin Loading (`cobolt-plugin-loader`)

Cobolt searches for plugins in:
1. `{project_dir}/plugins/`
2. `~/.cobolt/plugins/`
3. System-wide: `/usr/share/cobolt/plugins/` (Linux), `%ProgramData%\Cobolt\plugins\` (Windows)

For each directory it finds `*.cobolt_plugin` files (renamed `.dll` / `.so` / `.dylib` depending on platform) and a matching `cobolt-plugin.toml`.

Loading sequence:
```
1. Read cobolt-plugin.toml → validate abi_version matches COBOLT_ABI_VERSION
2. libloading::Library::new(path) → load dynamic library
3. Resolve symbol "cobolt_plugin_init" → call it → get *const CoboltPluginDescriptor
4. Register each ControlDescriptor into the IDE toolbox
5. Register each RuntimeCallDescriptor into the runtime CALL dispatcher
6. Log success or quarantine plugin on any failure (never crash the IDE)
```

### 6.4 Writing a Plugin (example skeleton)

```rust
// my_plugin/src/lib.rs
use cobolt_plugin_api::*;
use std::ffi::CString;

static MY_PLUGIN: CoboltPluginDescriptor = CoboltPluginDescriptor {
    abi_version: COBOLT_ABI_VERSION,
    plugin_id:  c"com.example.myplugin".as_ptr(),
    display_name: c"My Plugin".as_ptr(),
    version: c"0.1.0".as_ptr(),
    controls: MY_CONTROLS.as_ptr(),
    control_count: MY_CONTROLS.len(),
    runtime_calls: MY_CALLS.as_ptr(),
    runtime_call_count: MY_CALLS.len(),
};

#[no_mangle]
pub extern "C" fn cobolt_plugin_init() -> *const CoboltPluginDescriptor {
    &MY_PLUGIN
}
```

---

## 7. Cross-Platform Strategy

### 7.1 Build targets

| Target triple | Platform |
|---|---|
| `x86_64-pc-windows-msvc` | Windows 10/11 64-bit |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-unknown-linux-gnu` | Linux x64 (glibc 2.31+) |

### 7.2 Platform differences handled in `cobolt-ide`

- **Native file dialogs:** `rfd` crate abstracts WinUI / AppKit / GTK dialogs transparently.
- **Menu bar:** `egui` renders menus inline on Windows/Linux; on macOS, `eframe` can optionally use the native macOS menu bar.
- **Fonts:** `egui` bundles fonts; a system monospace font (Consolas / SF Mono / Ubuntu Mono) is preferred for the code editor, with bundled fallback.
- **Plugin extension:** `.dll` on Windows, `.dylib` on macOS, `.so` on Linux. The loader detects the platform automatically.
- **Path separators:** all internal paths use `std::path::PathBuf` — no hardcoded separators.

### 7.3 CI / Release pipeline (GitHub Actions)

```yaml
strategy:
  matrix:
    include:
      - os: windows-latest  target: x86_64-pc-windows-msvc
      - os: macos-latest    target: x86_64-apple-darwin
      - os: macos-latest    target: aarch64-apple-darwin
      - os: ubuntu-latest   target: x86_64-unknown-linux-gnu
steps:
  - cargo test --workspace
  - cargo build --release --target ${{ matrix.target }}
  - package: msi (Windows) / dmg (macOS) / AppImage (Linux)
```

---

## 8. Fujitsu COBOL Compatibility Notes

### 8.1 Supported features (MVP scope)

- All COBOL 85 verbs: MOVE, ADD, SUBTRACT, MULTIPLY, DIVIDE, COMPUTE, IF, EVALUATE, PERFORM (all variants), CALL, ACCEPT, DISPLAY, STOP RUN, GO BACK.
- WORKING-STORAGE and LOCAL-STORAGE sections.
- Intrinsic functions: LENGTH, NUMVAL, UPPER-CASE, LOWER-CASE, MAX, MIN, SQRT, MOD, REM, RANDOM, CURRENT-DATE.
- Sequential and relative file I/O (using Rust `std::fs`).
- STRING, UNSTRING, INSPECT verbs.
- SCREEN SECTION (text-mode rendering via terminal or emulated screen widget).
- Event model: `CALL 'COBOLT-WAIT-EVENT'`, `CALL 'COBOLT-SET-PROPERTY'`, `CALL 'COBOLT-GET-PROPERTY'`.

### 8.2 Indexed file I/O

Indexed (VSAM-style) file I/O — a common Fujitsu COBOL use case — is implemented using **SQLite** via the `rusqlite` crate. Each indexed file maps to a SQLite table, with the record key as the primary key. This approach is:
- Fully cross-platform (SQLite ships as a single file).
- Reliable under concurrent access.
- Easy to inspect with standard SQL tools.

### 8.3 Out of scope for v1.0

- EBCDIC code page execution (file I/O can convert, but the runtime is ASCII/UTF-8).
- COBOL multithreading (THREAD-LOCAL).
- Network I/O intrinsics.
- MicroFocus / IBM COBOL dialects.
- Report Writer (REPORT SECTION).

---

## 9. Phased Roadmap

### Phase 1 — Foundation (est. 3–4 months)

- `cobolt-lexer`: complete Fujitsu COBOL tokenizer, all keywords, fixed+free form.
- `cobolt-ast`: full AST node library with spans.
- `cobolt-parser`: parses IDENTIFICATION, DATA, PROCEDURE divisions; partial ENVIRONMENT.
- `cobolt-semantic`: symbol table, reference resolution, diagnostics.
- `cobolt-runtime`: arithmetic, IF/EVALUATE, PERFORM (all variants), CALL, ACCEPT/DISPLAY.
- `cobolt-stdlib`: intrinsic functions, ACCEPT FROM DATE/TIME, console I/O.
- CLI tool: `cobolt run myprogram.cbl` (no IDE, runtime only).
- Test suite: 200+ COBOL programs testing core verbs.

**Milestone deliverable:** Run a significant subset of Fujitsu COBOL programs from the command line.

### Phase 2 — IDE Shell (est. 2–3 months)

- `cobolt-ide`: egui window with all panels (project explorer, toolbox placeholder, code editor, output console).
- COBOL syntax highlighting in the editor.
- Project file creation, open, save.
- Run/stop integration (spawns runtime in a thread, streams output to console panel).
- Diagnostics underlines in editor.

**Milestone deliverable:** A working COBOL IDE — open a `.cbl` file, edit it, run it, see output.

### Phase 3 — Form Designer (est. 3–4 months)

- `cobolt-forms`: form data model + XML serialization.
- `cobolt-codegen`: form → COBOL source code generation.
- Form designer canvas: drag-and-drop, selection, resize, grid snap, undo/redo.
- Built-in controls rendered in designer (Button, TextBox, Label, CheckBox, ListBox, ComboBox, Panel, GroupBox).
- Properties inspector with live editing.
- Event binding UI (connect event to paragraph, double-click to jump to code).
- New Form wizard.

**Milestone deliverable:** Design a form visually, generate COBOL, run the program.

### Phase 4 — Plugin System (est. 2 months)

- `cobolt-plugin-api`: stable C ABI, opaque handle types.
- `cobolt-plugin-loader`: plugin discovery, loading, ABI version check.
- Toolbox populated dynamically with plugin controls.
- Runtime CALL dispatcher extended with plugin entries.
- `example-plugin`: a simple "LED indicator" control demonstrating the full plugin cycle.
- Plugin SDK documentation.

**Milestone deliverable:** A third-party developer can write and load a Cobolt control plugin entirely in Rust.

### Phase 5 — Polish & v1.0 (est. 2–3 months)

- Indexed file I/O via SQLite.
- Runtime debugger: breakpoints, step over/into, watch variables panel.
- COBOL-to-form round-trip: import existing COBOL + form files generated by original PowerCOBOL 3.0.
- Full intrinsic function library.
- Installer packages: MSI (Windows), DMG (macOS), AppImage + DEB + RPM (Linux).
- Full documentation site (mdBook).
- GitHub release with pre-built binaries for all 4 targets.

---

## 10. Suggested First PRs (Getting Started)

If you or contributors want to begin coding immediately, here are natural starting points:

1. **`cobolt-lexer`** — implement the `logos`-based tokenizer. Write a `lex("...")` function returning `Vec<Token>` and start with the ~60 most common COBOL keywords. Unit-test each token type.

2. **`cobolt-ast`** — define the `DataDecl` and `Stmt` enums. No logic, just data types with `derive(Debug, Clone, PartialEq)`.

3. **`cobolt-forms` / XML** — implement `Form::from_xml` and `Form::to_xml` with `quick-xml`. Start with Button and TextBox only.

4. **`cobolt-ide` skeleton** — create the `eframe` window, draw the four-panel layout with `egui_tiles`, wire up a placeholder toolbox and an empty editor area.

---

## 11. Key Dependencies Summary

| Crate | Purpose |
|---|---|
| `egui` + `eframe` | IDE UI framework |
| `egui_tiles` | Dockable panel layout |
| `egui_extras` | Grid/table widgets |
| `logos` | Lexer generator |
| `syntect` | Syntax highlighting |
| `serde` + `quick-xml` | Form XML serialization |
| `toml` | Project config + plugin manifests |
| `libloading` | Plugin dynamic loading |
| `rfd` | Native file dialogs |
| `rusqlite` | Indexed file I/O backend |
| `indexmap` | Ordered symbol tables |
| `thiserror` | Error types |
| `tracing` | Logging / diagnostics |
| `rayon` | Parallel background analysis |
| `tempfile` | Test infrastructure |

---

*Document version 1.0 — generated by Cobolt Architecture Planning session, May 2026.*
