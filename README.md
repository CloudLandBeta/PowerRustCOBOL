<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<p align="center">
  <img src="assets/images/powerrustcobol-mascot.png" alt="PowerRustCOBOL — a chibi samurai robot mascot" width="360">
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

[SCREENSHOT]

| Name | Role |
|------|------|
| **RustCOBOL** | The language and compiler (a COBOL dialect with visual RAD extensions). |
| **PowerRustCOBOL AI** | The RAD IDE — the desktop application you design and build with. Window titles, the welcome screen and the About dialog all show the product under this name; folder names on disk keep the original spelling. |
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

[SCREENSHOT]

PowerRustCOBOL is a working visual RAD environment for COBOL, not a prototype: a
form designer with **42 widgets** and pixel-parity rendering from canvas to
compiled binary, a COBOL-85 language core with **exact fixed-point arithmetic**,
**INDEXED (ISAM)** files with real transactions, SQL / REST / chart integrations,
an **agentic AI assistant mesh**, and a compiler that emits a **single native
binary** with no `.cbl` source inside it.

> **Every capability has its own row in the
> [PowerRustCOBOL Support Matrix](docs/cobol-support-matrix-en.md)** — including
> whether it comes from **COBOL-85**, a **later ISO standard (2002–2023/26)**, or
> is a **PowerRustCOBOL extension**, and whether it is supported, partial,
> planned or out of scope.

| Looking for | Document |
|---|---|
| Does it support X, and is X standard COBOL? | [Support matrix](docs/cobol-support-matrix-en.md) |
| Which exact spelling of a statement is accepted + the NIST CCVS85 scoreboard | [Supported syntax reference](docs/cobol85-supported-syntax-en.md) |
| What to test for each verb | [Verb test matrix](docs/cobol85-verb-test-matrix-en.md) |
| How to build applications with it | [Developer's guide](docs/developers-guide-en.md) |
| Indexed files — format, internals, redb engine | [format](docs/indexed-file-format-en.md) · [internals](docs/indexed-file-internals-en.md) · [redb](docs/indexed-redb-engine-en.md) |
| SQL runtime · logging and metrics | [database](docs/database-runtime-en.md) · [observability](docs/observability-en.md) |

![PowerRustCOBOL Agentic AI assistant architecture](docs/AI_Assistant_Architecture.jpg)

## Getting started

Get from a clean machine to the running IDE in four steps. For the full per-OS
walkthrough — package lists for Debian/Ubuntu, Fedora and Arch, where the
artifacts land, and what to do when the build fails — see
**[docs/BUILDING-en.md](docs/BUILDING-en.md)**.

### 1. Install the requirements

| Requirement | Why | Install |
|-------------|-----|---------|
| **Rust toolchain** (stable, **1.92 or newer**) | builds the whole workspace | [rustup.rs](https://rustup.rs) — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Git** | clone the repository | [git-scm.com](https://git-scm.com/downloads) |
| **A C toolchain + native GUI libraries** | the platform linker Rust itself needs, two C dependencies (bundled SQLite and the Oniguruma regex engine), and the native file dialogs | see the per-OS notes below |

There is **no Python, Node, JVM, CMake, NASM or C++ compiler** anywhere in the
build — a C compiler and a linker are the whole of it, and on every platform they
arrive with the package Rust already needs in order to link.

Per-OS native dependencies:

- **macOS** — install the Xcode Command Line Tools: `xcode-select --install`. Nothing else is needed.

- **Windows** — install the **Visual Studio Build Tools** with the *"Desktop
  development with C++"* workload ([download](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)),
  then install Rust from [rustup.rs](https://rustup.rs) — it selects the MSVC
  toolchain automatically. That single workload provides everything: `link.exe`
  and the Windows SDK (which rustc requires for *any* Rust binary, C code or not)
  and `cl.exe` for the two C dependencies. Nothing else to download.

  ```powershell
  # after both installs, from a normal PowerShell prompt
  rustc --version
  cargo build --release -p cobolt-ide -p cobolt-cli
  ```

- **Linux (Debian/Ubuntu)** — install the build + GUI/dialog libraries:

  ```sh
  sudo apt update && sudo apt install -y \
      build-essential pkg-config \
      libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
      libxkbcommon-dev libssl-dev
  ```

  (Fedora: `gtk3-devel`, `libxcb-devel`, `libxkbcommon-devel`, `openssl-devel`, `@development-tools`.)

  `libssl-dev` / `openssl-devel` is load-bearing on Linux: HTTPS goes through the
  operating system's TLS (schannel on Windows, Security.framework on macOS,
  OpenSSL here) rather than through a bundled crypto library that would have to
  be compiled from C on every machine.

Verify Rust is ready:

```sh
rustc --version && cargo --version
```

### 2. Download the code

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
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

A **single console-only program** needs no `cobolt.toml` — just point `build` at
the `.cbl` file:

```sh
# Compile one source file → ./bin/<file-stem>  (native binary, next to the source)
cargo run -p cobolt-cli -- build hello.cbl
./bin/hello
```

For a **project** (multiple sources and/or forms), pass the manifest:

```sh
# From inside the project directory:
cargo run -p cobolt-cli -- build cobolt.toml
#   → produces ./bin/<app-name>  (self-contained native executable)
#   → plus ./bin/LICENSE, ./bin/NOTICE, ./bin/POWERRUSTCOBOL-NOTICE.txt

# Then just run it — no IDE, no source, no runtime install:
./bin/<app-name>
```

`rcrun build` decides by the argument: a `.cbl`/`.cob`/`.cbk`/`.cpy` file is a
standalone build (project metadata is synthesized from the file name); anything
else is treated as a `cobolt.toml` manifest. If the project has forms, the binary
launches the GUI application; otherwise it runs headless. The compressed AST and
forms are embedded inside the executable.

### Package a project for distribution

```sh
cargo run -p cobolt-cli -- package cobolt.toml --output myapp.zip
```

The zip bundles the manifest, sources, forms, assets, an optional runner, and the
required `LICENSE` / `NOTICE` / runtime-notice files.

> Prefer a short command? Build once with `cargo build --release` and use the produced
> `target/release/rcrun` binary directly: `rcrun run main.cbl`, `rcrun build cobolt.toml`, …

## Powered by PowerRustCOBOL

<p align="center">
  <img src="assets/images/made-with-powerrustcobol.png" alt="Powered by PowerRustCOBOL" width="320">
</p>

Built something with PowerRustCOBOL? Show it off — add the **"Powered by
PowerRustCOBOL"** badge to your application's **About box** (and, if you like, your
own README).

- Badge image: [`assets/images/made-with-powerrustcobol.png`](assets/images/made-with-powerrustcobol.png) (800×268, transparent PNG).
- Need it larger / for print? A high-resolution master is provided at
  [`assets/images/made-with-powerrustcobol.webp`](assets/images/made-with-powerrustcobol.webp)
  (6785×2270) — scale it down to whatever size you need.

Markdown:

```markdown
[![Powered by PowerRustCOBOL](assets/images/made-with-powerrustcobol.png)](https://github.com/CloudLandBeta/PowerRustCOBOL)
```

HTML:

```html
<a href="https://github.com/CloudLandBeta/PowerRustCOBOL">
  <img src="made-with-powerrustcobol.png" alt="Powered by PowerRustCOBOL" width="320">
</a>
```

## COBOL-85 standard support

PowerRustCOBOL targets a **practical, application-oriented subset** of COBOL-85
plus visual RAD extensions. It is **not** (yet) a certified COBOL-85
implementation — and conformance here is **measured** against the official NIST
CCVS85 validation suite rather than asserted.

The full picture is one table per area, with a column for each origin
(**COBOL-85**, **COBOL 2002–2023/26**, **PowerRustCOBOL extension**) and a
status for every row:

> ### → [PowerRustCOBOL Support Matrix](docs/cobol-support-matrix-en.md)

It covers source format and program structure, the DATA DIVISION, every verb,
conditions and expressions, the complete intrinsic-function set, file
organizations, the INDEXED engine, runtime integrations, what is explicitly out
of scope, and the platform itself. For the exact accepted spelling of each
statement and the NIST scoreboard, see the
[supported-syntax reference](docs/cobol85-supported-syntax-en.md).

## Repository layout

PowerRustCOBOL is a Rust workspace. The internal build crates use a `cobolt-*` prefix
(build-only identifiers; the product is **PowerRustCOBOL**, the language **RustCOBOL**,
the CLI **rcrun**):

| Path | Responsibility |
|------|----------------|
| `specs/` | Spec-driven development — steering docs, templates, and per-feature `NNN-<slug>/` specs (`specs/README.md`). |
| `.claude/skills/` | Slash-command skills for the workflow (`/specify`, `/plan`, `/implement`, `/docsync`, …). |
| `docs/` | Developer guide (English canonical), language reference, and internals docs. |
| `tests/cobol/` | COBOL integration programs exercised against the runtime. |

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
