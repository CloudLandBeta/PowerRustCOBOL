<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Crate inventory

Every crate PowerRustCOBOL depends on **directly**, with the version actually
linked (not the requirement string — the resolved one from `Cargo.lock`).

Generated from `cargo metadata` on **2026-07-27**, at product version
**1.37.0**. Note the two numbering schemes: the *product* version is the one in
`crates/cobolt-ide/src/version.rs` and shown in the IDE; the *crate* version in
`Cargo.toml` is `0.2.0` and is shared by all workspace crates.
Regenerate the version column with:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

The full dependency graph is **906 packages**. The tables below are the ~56 the
workspace names itself; everything else arrives transitively through them.

---

## Workspace crates

The 14 crates that *are* PowerRustCOBOL. All share the workspace crate version
`0.2.0` (see the note above — the product version is 1.37.0).

| Crate | Crate version | Layer | What it does |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Fujitsu COBOL tokenizer — fixed-form and free-form source |
| `cobolt-parser` | 0.2.0 | front end | Recursive-descent parser: token stream → AST |
| `cobolt-ast` | 0.2.0 | front end | AST node types |
| `cobolt-semantic` | 0.2.0 | front end | Name resolution, type checking, `EXEC RUST` binding |
| `cobolt-runtime` | 0.2.0 | execution | Tree-walking interpreter, value system, `EXEC RUST` executor, DB/HTTP runtimes |
| `cobolt-stdlib` | 0.2.0 | execution | Intrinsic functions, I/O backend, console helpers |
| `cobolt-indexed` | 0.2.0 | execution | Indexed-file definition model (`.cidx`) |
| `cobolt-forms` | 0.2.0 | UI engine | Form/control model (`.cfrm`), the unified render engine, themes, animation |
| `cobolt-media` | 0.2.0 | UI engine | Animated image (GIF/WebP/APNG) decode + playback for the Animator widget |
| `cobolt-codegen` | 0.2.0 | tooling | Form → COBOL source generator |
| `cobolt-compiler` | 0.2.0 | tooling | Embed+bundle compiler: project → one native executable |
| `cobolt-agents` | 0.2.0 | AI | Agent mesh, Knowledge Base index, embeddings, retrieval |
| `cobolt-cli` | 0.2.0 | binary | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binary | The IDE itself |

---

## External dependencies

`Used by` names workspace crates with the `cobolt-` prefix dropped.

### UI and rendering

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | Immediate-mode GUI toolkit — the whole UI |
| `eframe` | 0.35.0 | cli, ide | Window + event loop host for egui |
| `egui_extras` | 0.35.0 | cli, ide | Tables, image loaders, extra widgets |
| `egui_glow` | 0.35.0 | ide | OpenGL painter — the rounded-corner clip hook needs it |
| `egui_commonmark` | 0.24.0 | ide | Markdown rendering in docs/chat panels |
| `egui_inspection` | 0.35.0 | ide | Live widget/layout inspector |
| `image` | 0.25.10 | cli, forms, ide, media | PNG/JPEG/GIF/WebP/BMP decode |
| `resvg` | 0.46.0 | forms, ide | SVG rasterisation |
| `fontdb` | 0.23.0 | forms, ide | System font enumeration |
| `skrifa` | 0.42.1 | forms | Font-face validation with the parser epaint itself uses |
| `rfd` | 0.14.1 | ide | Native Open/Save dialogs |
| `syntect` | 5.3.0 | ide | Syntax highlighting in the editor |
| `pulldown-cmark` | 0.12.2 | ide | Markdown parsing |
| `mermaid-rs-renderer` | 0.2.2 | ide | Mermaid diagram rendering |
| `genpdf` | 0.2.0 | ide | PDF export |
| `pollster` | 0.3.0 | ide | Blocks on the few async calls the IDE makes |

### Language front end

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Lexer generator |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Insertion-ordered maps — COBOL declaration order is semantic |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Error types |

### Data, storage and I/O

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Pure-Rust embedded ACID store — INDEXED files and the KB index |
| `rusqlite` | 0.32.1 | runtime | SQLite for the COBOL database runtime (bundled; compiles C) |
| `postgres` | 0.19.13 | runtime | PostgreSQL driver (pure Rust, synchronous) |
| `mysql` | 28.0.0 | runtime | MySQL driver (pure Rust, rustls feature set) |
| `ureq` | 2.12.1 | runtime | Blocking HTTP client for the COBOL REST runtime |
| `native-tls` | 0.2.18 | runtime | TLS via the OS stack — no bundled crypto to compile |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | HTTP client for model and web calls |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` serialisation |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Serialisation framework |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (deprecated upstream; pinned) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, theme manifests |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Compact binary encoding of the compiled AST |
| `flate2` | 1.1.9 | compiler | Deflate — compresses the embedded AST |
| `zip` | 2.4.2 | cli, ide | Project archive import/export |
| `include_dir` | 0.7.4 | ide | Bakes the bundled docs into the binary |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Temporary files (also a dev-dependency) |
| `dirs` | 5.0.1 | ide | Per-platform config/data directories |

### AI and retrieval

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Agent/LLM orchestration (native-tls, not rustls) |
| `candle-core` | 0.11.0 | agents | Pure-Rust tensor runtime |
| `candle-nn` | 0.11.0 | agents | Neural-network layers for Candle |
| `candle-transformers` | 0.11.0 | agents | BERT and friends — runs `all-MiniLM-L6-v2` in-process |
| `tokenizers` | 0.23.1 | agents | HuggingFace tokenizer (`esaxx_fast` off, `onig` on) |
| `embedvec` | 0.8.0 | agents | Vector store: E8 quantization, cosine similarity |
| `schemars` | 1.2.1 | agents, ide | JSON Schema for tool definitions |
| `opentelemetry` | 0.32.0 | agents | Tracing/metrics API |
| `tokio` | 1.52.3 | agents, ide | Async runtime for the agent layer |
| `futures` | 0.3.32 | agents | Async combinators |

### Cross-cutting

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Structured logging |
| `tracing-subscriber` | 0.3.23 | cli, ide | Log filtering and formatting |
| `sysinfo` | 0.31.4 | ide | Process/memory stats |
| `num_cpus` | 1.17.0 | agents | Parallelism sizing |
| `rand` | 0.8.6 | ide | Random values |
| `hmac` | 0.12.1 | forms | HMAC for the binding signature |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Readable test diffs (dev-dependency) |

---

## Declared but not linked by default

These are named in a `Cargo.toml` behind a feature that is **off** in a default
build, so they contribute nothing to compile time or binary size unless you turn
the feature on:

| Crate | Feature | Why it is optional |
|---|---|---|
| `tantivy` | `local-retrieval` | Lexical index — the default path is `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | SQLite-backed vector search; enabling it brings bundled SQLite (and a C toolchain) into `cobolt-agents` |
| `ort`, `ndarray` | `local-retrieval` | ONNX Runtime inference path |
| `opentelemetry-otlp` | `otel` | OTLP export |

---

## The two crates that compile C

Worth knowing when setting up a machine (see [BUILDING.md](BUILDING.md)):

| Crate | Reached via | What it compiles |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (in `cobolt-runtime`) | The SQLite C amalgamation, bundled so no system SQLite has to match |
| `onig_sys` | `tokenizers` → `onig` | The Oniguruma regex engine |

Nothing in the tree compiles **C++**, and no build script invokes CMake, NASM,
Python, Node or a JVM.
