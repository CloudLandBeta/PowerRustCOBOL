<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<p align="center">
  <img src="docs/assets/powerrustcobol-mascot.png" alt="PowerRustCOBOL — a chibi samurai robot mascot" width="360">
</p>

<p align="center">
  <em>A modern, Rust-powered RAD (Rapid Application Development) environment for COBOL —<br>
  design forms visually, run them on a fast tree-walking runtime, and compile to a single self-contained binary.</em>
</p>

<p align="center">
  <a href="#license"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-Rust-orange.svg">
  <img alt="Status: active development" src="https://img.shields.io/badge/status-active%20development-success.svg">
</p>

---

## Overview

**PowerRustCOBOL** brings COBOL into a modern desktop development experience. It pairs a
practical subset of the **COBOL-85 standard** with a visual form designer, a rich widget
toolbox, an interactive debugger, and a compiler that turns a project into one
**self-contained native binary** — no COBOL source shipped inside it.

| Name | Role |
|------|------|
| **RustCOBOL** | The language and compiler (a COBOL dialect with visual RAD extensions). |
| **PowerRustCOBOL** | The RAD IDE — the desktop application you design and build with. |
| **rcrun** | The command-line runtime/build tool. |

> ⚠️ COBOL data-item names, paragraph names, and all generated COBOL source always remain
> in **English**, regardless of the IDE's selected interface language.

## Goals

- **Make COBOL approachable** with a visual, drag-and-drop form designer and live preview.
- **Run COBOL fast** on a clean tree-walking interpreter — no external runtime required.
- **Ship real apps**: compile a project into a single native executable that embeds its
  forms and program logic.
- **Stay self-contained**: the default toolchain needs no system COBOL, no FFmpeg, and no
  proprietary dependencies.
- **Be honest about scope**: implement the parts of COBOL-85 that matter for building
  applications today, and clearly mark what is partial or out of scope.

## What's implemented

### The IDE (PowerRustCOBOL)
- **Visual form designer** with a glass/"liquid" theme, grid snapping, drag-resize of
  controls *and* the form canvas, multi-select, alignment tools, and z-ordering.
- **34 widgets** across Common, Container, Data, Graphics, Menu, Non-visual, Charts and
  Dialog categories — including a new **Animator** widget that plays **GIF / WebP / APNG**
  animations natively.
- **Properties inspector**, **toolbox** with vector icons, **forms list**, **project tree**,
  and an **output panel**.
- **System-font picker**: choose any installed font (rendered in its own typeface), applied
  live in the canvas, preview and run windows, with an Arial fallback.
- **Interactive debugger**: breakpoints, step controls, and a variable watch.
- **Multi-window** preview/run via per-form OS windows (egui multi-viewport).
- **Internationalised UI** in 5 languages (English, Spanish, Portuguese, Japanese, Chinese).
- **Non-blocking native file dialogs** (open/save/browse) that never stall the event loop.

### The RustCOBOL language & runtime
- **Fixed-form and free-form** source.
- **Divisions**: IDENTIFICATION, ENVIRONMENT (INPUT-OUTPUT / FILE-CONTROL), DATA, PROCEDURE.
- **DATA sections**: WORKING-STORAGE, LOCAL-STORAGE, LINKAGE, FILE.
- **Data description**: `PIC` X / A / 9 / S / V with `(n)` repetition; `USAGE` DISPLAY,
  COMP/BINARY, COMP-1/2, COMP-3/PACKED-DECIMAL, COMP-5, INDEX, POINTER; `OCCURS`
  (fixed and `DEPENDING ON`); level numbers incl. 01/77/88; `VALUE`; group items; `FILLER`;
  88-level condition names.
- **Verbs**: MOVE, DISPLAY, ACCEPT (`FROM DATE/TIME/DAY/DAY-OF-WEEK/ENVIRONMENT`),
  ADD / SUBTRACT / MULTIPLY / DIVIDE (incl. `REMAINDER`) / COMPUTE, IF…ELSE…END-IF,
  EVALUATE…WHEN / WHEN OTHER, PERFORM (inline, `TIMES`, `UNTIL`, `VARYING…AFTER`, `THRU`),
  GO TO, CONTINUE, STOP RUN, GOBACK, EXIT, STRING, UNSTRING, INSPECT (`TALLYING`/`REPLACING`).
- **CALL**: COBOL-85 **nested programs** *and* multiple sequential program units in one file,
  plus a library of runtime built-ins (see below).
- **Intrinsic functions**: `LENGTH`, `UPPER-CASE`, `LOWER-CASE`, `NUMVAL`, `NUMVAL-C`,
  `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `RANDOM`,
  `CURRENT-DATE`, `TRIM`, `REVERSE`, `CONCATENATE`.
- COBOL-correct alphanumeric comparison (space-padded) and figurative constants
  (SPACES, ZEROS, HIGH/LOW-VALUES, QUOTES, NULLS).

### File I/O
- **`ORGANIZATION IS SEQUENTIAL`** (fixed-length records) and **`LINE SEQUENTIAL`**
  (newline-terminated text; trailing spaces dropped on write).
- **`ORGANIZATION IS INDEXED`** (ISAM) — a built-in, **dependency-free** keyed-file
  engine: primary `RECORD KEY` + `ALTERNATE RECORD KEY [WITH DUPLICATES]`,
  records kept in ascending key order on disk, journaled with `COMMIT`/`ROLLBACK`.
  - **`STORAGE MODE IS MEMORY | DISK [WITH DATA COMPRESSING]`** (PowerRustCOBOL
    extension) selects the backend per file. `MEMORY` (default) is the in-RAM
    engine (whole file in memory, persisted to the `ASSIGN` path on close).
    `DISK` is a **persistent paged B+tree** engine — records and indexes live in
    the `ASSIGN` file and are read on demand (free-list page reuse + a `RecordId`
    directory), so RAM is bounded for very large files. `WITH DATA COMPRESSING`
    applies to either mode: a fast, dependency-free RLE that crushes the padded
    runs in typical COBOL records well past 50 %. `ASSIGN TO` is always required.
    ```cobol
    SELECT CUSTOMER-FILE
        STORAGE MODE IS DISK WITH DATA COMPRESSING
        ASSIGN TO "customers.dat"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUSTOMER-ID
        ALTERNATE RECORD KEY IS CUSTOMER-ZIP WITH DUPLICATES
        FILE STATUS IS CUSTOMER-STATUS.
    ```
  - `ACCESS MODE SEQUENTIAL / RANDOM / DYNAMIC` all execute.
  - `READ` (random by key, and `READ … NEXT/PREVIOUS` sequential), `WRITE`,
    `REWRITE`, `DELETE`, and `START … KEY IS = / > / >= / < / <=` (incl.
    `GREATER/LESS THAN`, `NOT LESS THAN`, …).
  - `INVALID KEY` / `NOT INVALID KEY` phrases and full **FILE STATUS** codes
    (00/02/10/22/23/…).
  - Engine selectable via `rcrun --indexed-engine <rust|rm-cobol85|fujitsu>` or
    the `COBOL_INDEXED_ENGINE` env var (all behaviour-compatible; `rust` default).
  - **Self-describing `PRCIDX1` container** — embeds the file schema (record
    format + key descriptors) so files are discoverable and a future Fujitsu
    importer can write faithful data; strict open-time validation maps schema
    mismatch → `39`, missing file → `35`. Not byte-compatible with Fujitsu. See
    [`docs/indexed-file-format.md`](docs/indexed-file-format.md).
- `SELECT … ASSIGN TO … ORGANIZATION … [ACCESS MODE …] [RECORD KEY …] [FILE STATUS IS …]`.
- File verbs are dispatched by each file's declared `ORGANIZATION` (per its
  `SELECT`), so sequential and indexed files share `OPEN`/`CLOSE`/`READ`/`WRITE`
  while each honours its own organization-specific semantics.
- `OPEN INPUT/OUTPUT/EXTEND/I-O`, `READ … [INTO] [AT END / NOT AT END]`,
  `WRITE … [FROM]`, `CLOSE`, with **FILE STATUS** codes (00/10/30/35/…).

### Built-in integrations (runtime `CALL`s and `INVOKE`)
- **SQL (SQLite)** — open/exec/fetch/row-count/close.
- **HTTP / REST** — GET/POST/PUT/DELETE with custom headers.
- **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM`.
- **Charts** — 6 chart types bound to COBOL tables (bar/line/pie/area/scatter/donut).
- **Text files** — `COBOL-APPEND-FILE` / `COBOL-WRITE-FILE` helpers.
- **Timers** and an **AI agent** object hook.

### The compiler (single-binary)
- Serialises the program AST with `bincode` + `flate2`, embeds it and all forms via
  `include_bytes!`, builds with `cargo build --release`, and emits one native binary in
  `bin/` — **with no `.cbl` source included**.
- The output `bin/` automatically receives `LICENSE`, `NOTICE`, and a redistribution
  notice, so distributions carry the required Apache-2.0 notices.

## Getting started

Get from a clean machine to the running IDE in four steps.

### 1. Install the requirements

| Requirement | Why | Install |
|-------------|-----|---------|
| **Rust toolchain** (stable) | builds the whole workspace | [rustup.rs](https://rustup.rs) — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Git** | clone the repository | [git-scm.com](https://git-scm.com/downloads) |
| **A C toolchain + native GUI libraries** | the desktop IDE (egui/eframe) and native file dialogs | see the per-OS notes below |

Per-OS native dependencies:

- **macOS** — install the Xcode Command Line Tools: `xcode-select --install`. Nothing else is needed.
- **Windows** — install the **Visual Studio C++ Build Tools** (the "Desktop development with C++" workload). rustup selects the MSVC toolchain automatically.
- **Linux (Debian/Ubuntu)** — install the build + GUI/dialog libraries:

  ```sh
  sudo apt update && sudo apt install -y \
      build-essential pkg-config \
      libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
      libxkbcommon-dev libssl-dev
  ```

  (Fedora: `gtk3-devel`, `libxcb-devel`, `libxkbcommon-devel`, `openssl-devel`, `@development-tools`.)

Verify Rust is ready:

```sh
rustc --version && cargo --version
```

### 2. Download the code

```sh
git clone https://github.com/emerson-github/PowerRustCOBOL.git
cd PowerRustCOBOL
```

### 3. Build (downloads + compiles all dependencies)

```sh
cargo build
```

> The first build fetches every crate and compiles the workspace, so it takes a
> few minutes and the `target/` cache grows to ~1.5 GB. Later builds are
> incremental and fast. Run `cargo clean` to reclaim the space at any time.

### 4. Launch the IDE

```sh
cargo run -p cobolt-ide
```

> **Tip:** for the smoothest UI, run a release build: `cargo run --release -p cobolt-ide`
> (slower to compile the first time, much faster at runtime).

That's it — the **PowerRustCOBOL** window opens and you can start designing forms
and writing RustCOBOL. To work from the command line instead, see
[Run / check a program from the CLI](#run--check-a-program-from-the-cli-rcrun) below.

## Running applications

A PowerRustCOBOL **project** is a directory with a `cobolt.toml` manifest plus its
`.cbl` sources and `.cfrm` forms:

```toml
[project]
name = "MyApp"
version = "1.0.0"
main = "main.cbl"

[files]
sources = ["main.cbl"]
forms   = ["main-form.cfrm"]
assets  = []
```

### Launch the IDE

```sh
cargo run -p cobolt-ide
```

### Run / check a program from the CLI (`rcrun`)

```sh
# Run a COBOL program
cargo run -p cobolt-cli -- run main.cbl

# Parse + semantic analysis only (no execution)
cargo run -p cobolt-cli -- check main.cbl
```

### Generate a standalone binary

```sh
# From inside the project directory:
cargo run -p cobolt-cli -- build cobolt.toml
#   → produces ./bin/<app-name>  (self-contained native executable)
#   → plus ./bin/LICENSE, ./bin/NOTICE, ./bin/POWERRUSTCOBOL-NOTICE.txt

# Then just run it — no IDE, no source, no runtime install:
./bin/<app-name>
```

If the project has forms, the binary launches the GUI application; otherwise it runs
headless. The compressed AST and forms are embedded inside the executable.

### Package a project for distribution

```sh
cargo run -p cobolt-cli -- package cobolt.toml --output myapp.zip
```

The zip bundles the manifest, sources, forms, assets, an optional runner, and the
required `LICENSE` / `NOTICE` / runtime-notice files.

> Prefer a short command? Build once with `cargo build --release` and use the produced
> `target/release/rcrun` binary directly: `rcrun run main.cbl`, `rcrun build cobolt.toml`, …

## COBOL-85 standard support

PowerRustCOBOL targets a **practical, application-oriented subset** of COBOL-85 plus
visual RAD extensions. It is **not** (yet) a certified COBOL-85 implementation. Here is an
honest map of where things stand.

### ✅ Supported
- Fixed-form & free-form source; all four divisions.
- WORKING-STORAGE / LOCAL-STORAGE / LINKAGE / FILE sections.
- PICTURE (X/A/9/S/V with repetition counts), USAGE clauses, OCCURS, 01/77/88 levels,
  VALUE, group items, FILLER, condition-names.
- The procedural verbs and intrinsic functions listed above.
- Nested programs and multiple program units; `CALL` dispatch.
- **SEQUENTIAL**, **LINE SEQUENTIAL** and **INDEXED** (ISAM) file I/O with FILE STATUS.
- **Exact fixed-point arithmetic** — `ADD`/`SUBTRACT`/`MULTIPLY`/`DIVIDE`/`COMPUTE`
  run on an `i128` integer mantissa (no `f64` round-trips), so up to **18-digit**
  standard and **31-digit** extended precision stay exact. `ROUNDED` (round half
  away from zero) and `ON SIZE ERROR` / `NOT ON SIZE ERROR` are honored, and
  decimal literals are carried exactly from the lexer. Verified end-to-end by the
  COBOL suite at [`tests/cobol/numeric-precision/numprec.cbl`](tests/cobol/numeric-precision/numprec.cbl).
- **Numeric-edited PICTUREs** — the edit engine applies `Z` zero-suppression,
  `*` check-protection, fixed and floating `$`, fixed and floating `+`/`-` signs,
  `,` and `.` insertion, `B`/`0`/`/` insertion, and `CR`/`DB` on `MOVE`/`DISPLAY`
  into an edited field (e.g. `PIC ZZZ,ZZ9.99`, `$$$,$$9.99`, `9(6).99CR`). Plain
  numeric `DISPLAY` is rendered at full PIC width with leading zeros. Verified by
  [`tests/cobol/numeric-edited-pic/numedit.cbl`](tests/cobol/numeric-edited-pic/numedit.cbl).
- **`COPY` / `REPLACE` copybooks** — a preprocessor expands `COPY name [OF lib]
  [REPLACING ==a== BY ==b== …]` (pseudo-text and word replacement), resolves
  copybooks next to the source (`.cpy`/`.cbl`/`.cob`/…, case-insensitive),
  expands nested `COPY` recursively, and applies `REPLACE … BY …` / `REPLACE
  OFF` to following text. Verified by
  [`tests/cobol/copy-replace/copytest.cbl`](tests/cobol/copy-replace/copytest.cbl).
- **`INDEXED` (ISAM) files** — a built-in, dependency-free keyed-file engine with
  primary + alternate keys, ascending on-disk key order, journaled
  `COMMIT`/`ROLLBACK`, record locking, `ACCESS MODE SEQUENTIAL/RANDOM/DYNAMIC`,
  and the full `READ`/`WRITE`/`REWRITE`/`DELETE`/`START` verb set with
  `INVALID KEY` phrases. File verbs dispatch by each `SELECT`'s declared
  `ORGANIZATION`. Engine selectable (`--indexed-engine` / `COBOL_INDEXED_ENGINE`).
  Verified by
  [`tests/cobol/indexed-files/idxbasic.cbl`](tests/cobol/indexed-files/idxbasic.cbl).

### 🚧 Partial / in progress
- **SCREEN SECTION** — parsed in simplified form; terminal screen handling is not executed
  (the visual form designer supersedes it).

### ⛔ Not yet implemented (planned)
- **RELATIVE** file organization.
- **`SORT` / `MERGE`**.
- File sharing / record locking.
- The complete intrinsic-function library.
- Object-Oriented COBOL **class/method definitions** (`INVOKE` is supported for GUI and
  runtime objects only).

### 🚫 Explicitly out of scope — by design (will never be implemented)
- **COMMUNICATION SECTION** (`CD` entries, message control / teleprocessing).
- **REPORT WRITER SECTION** (`RD` entries, `GENERATE` / `INITIATE` / `TERMINATE`).
- **ActiveX / OLE / COM** controls.

## Repository layout

PowerRustCOBOL is a Rust workspace. The internal build crates use a `cobolt-*` prefix
(build-only identifiers; the product is **PowerRustCOBOL**, the language **RustCOBOL**,
the CLI **rcrun**):

| Crate | Responsibility |
|-------|----------------|
| `cobolt-lexer` | COBOL tokenizer (fixed + free form). |
| `cobolt-ast` | AST node types. |
| `cobolt-parser` | Recursive-descent parser. |
| `cobolt-semantic` | Semantic analysis / diagnostics. |
| `cobolt-runtime` | Tree-walking interpreter, file I/O, SQL/HTTP/GUI built-ins. |
| `cobolt-stdlib` | Standard-library support. |
| `cobolt-forms` | `.cfrm` form model + XML serialization. |
| `cobolt-media` | Animated-image (GIF/WebP/APNG) decode + playback for the Animator. |
| `cobolt-codegen` | Form → RustCOBOL source generator. |
| `cobolt-compiler` | Embed-and-bundle single-binary compiler. |
| `cobolt-cli` | The `rcrun` command-line tool. |
| `cobolt-ide` | The PowerRustCOBOL desktop app (egui/eframe). |

```sh
# Build everything
cargo build

# Run the test suite
cargo test
```

## License

PowerRustCOBOL is licensed under the **Apache License, Version 2.0**.

Applications, source code, forms, assets, project files, binaries, and packages **created
by users** with PowerRustCOBOL are owned by their respective authors and may be licensed
under any terms they choose, including proprietary commercial terms.

PowerRustCOBOL's own components (runtime, standard library, compiler support code,
generated support modules, form-engine components, templates, helper libraries, and any
other PowerRustCOBOL-provided components bundled with a user application) remain
PowerRustCOBOL components licensed under the Apache License, Version 2.0. Distributions
that include them must preserve the required copyright, license, attribution, and NOTICE
information.

See [`LICENSE`](LICENSE), [`NOTICE`](NOTICE), and [`docs/licensing/`](docs/licensing/)
(runtime license, generated-code policy, third-party notices, and per-file header
templates) for the full details.
