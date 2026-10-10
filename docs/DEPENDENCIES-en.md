<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# Crate inventory

Every crate PowerRustCOBOL depends on **directly**, with the version actually
linked (not the requirement string — the resolved one from `Cargo.lock`).

First generated from `cargo metadata` on **2026-07-27** at product version
**1.37.0**; the resolved versions below were last reconciled against
`Cargo.lock` on **2026-09-11**, at **1.65.128**. Note the two numbering schemes:
the *product* version is the one in
`crates/cobolt-ide/src/version.rs` and shown in the IDE; the *crate* version in
`Cargo.toml` is `0.2.0` and is shared by all workspace crates.
Regenerate the version column with:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

The full dependency graph is **944 packages**. The tables below are the **59**
the workspace names itself; everything else arrives transitively through them.

---

## Workspace crates

The 21 crates that *are* PowerRustCOBOL — `cargo metadata --no-deps` is the
authority here, not a grep of `Cargo.toml`, where two members share a line and a
naive count reports 16. All share the workspace crate version `0.2.0` (see the
note above — the product version is its own sequence).

| Crate | Crate version | Layer | What it does |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Fujitsu COBOL tokenizer — fixed-form and free-form source — and the `COPY`/`REPLACE` preprocessor |
| `cobolt-parser` | 0.2.0 | front end | Recursive-descent parser: token stream → AST |
| `cobolt-ast` | 0.2.0 | front end | AST node types |
| `cobolt-semantic` | 0.2.0 | front end | Name resolution, type checking, `EXEC RUST` binding |
| `cobolt-runtime` | 0.2.0 | execution | Tree-walking interpreter, value system, `EXEC RUST` executor, DB/HTTP runtimes |
| `cobolt-stdlib` | 0.2.0 | execution | Intrinsic functions, I/O backend, console helpers |
| `cobolt-indexed` | 0.2.0 | execution | Indexed-file definition model (`.cidx`) |
| `cobolt-forms` | 0.2.0 | UI engine | Form/control model (`.cfrm`), the unified render engine, themes, animation |
| `cobolt-form-host` | 0.2.0 | UI engine | The one form host (spec 042) — shared by `rcrun run-form` and compiled applications |
| `cobolt-media` | 0.2.0 | UI engine | Animated image (GIF/WebP/APNG) decode + playback for the Animator widget |
| `cobolt-codegen` | 0.2.0 | tooling | Form → COBOL source generator |
| `cobolt-compiler` | 0.2.0 | tooling | Embed+bundle compiler: project → one native executable |
| `cobolt-dap` | 0.2.0 | tooling | Wire-compatible Debug Adapter Protocol — framing, types, client and adapter server |
| `cobolt-mcp` | 0.2.0 | tooling | Wire-compatible Model Context Protocol — JSON-RPC framing, types and server (spec 065) |
| `cobolt-project-tools` | 0.2.0 | tooling | The coding-agent tools (spec 080) — one MCP tool set over a project, served by `rcrun mcp` (stdio) and the IDE (HTTP on `127.0.0.1`) |
| `cobolt-agents` | 0.2.0 | AI | Agent mesh, Knowledge Base index, embeddings, retrieval |
| `cobolt-docs` | 0.2.0 | execution | Document import — Office, OpenDocument, PDF, HTML, tables and archives to Markdown (spec 074) |
| `cobolt-kb` | 0.2.0 | execution | The application Knowledge Base — chunking, embedding, a shared index, search (spec 068) |
| `cobolt-cli` | 0.2.0 | binary | `rcrun` — run, check, build, run-form, mcp |
| `cobolt-ide` | 0.2.0 | binary | The IDE itself |
| `cobolt-bench` | 0.2.0 | binary | Performance and allocation baseline harness (see [BENCHMARKS-en.md](BENCHMARKS-en.md)) |

---

## External dependencies

`Used by` names workspace crates with the `cobolt-` prefix dropped.

### UI and rendering

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | Immediate-mode GUI toolkit — the whole UI |
| `eframe` | 0.36.0 | cli, ide | Window + event loop host for egui |
| `egui_extras` | 0.36.0 | cli, ide | Tables, image loaders, extra widgets |
| `egui_commonmark` | 0.25.0 | ide | Markdown rendering in docs/chat panels |
| `egui_inspection` | 0.36.0 | ide | Live widget/layout inspector |
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
| `mysql` | 28.0.0 | runtime | MySQL driver (pure Rust, `minimal-rust` feature set — **no TLS**; see [database-runtime-en.md](database-runtime-en.md)) |
| `ureq` | 2.12.1 | runtime | Blocking HTTP client for the COBOL REST runtime |
| `native-tls` | 0.2.18 | runtime | TLS via the OS stack — no bundled crypto to compile |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | HTTP client for model and web calls |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` serialisation |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Serialisation framework |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (deprecated upstream; pinned) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, theme manifests |
| `toml_edit` | 0.22.27 | project-tools | Project-file edits that keep every byte the coding-agent tools do not own |
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
| `schemars` | 1.2.1 | agents, ide | JSON Schema for tool definitions |
| `tokio` | 1.52.3 | agents, ide | Async runtime for the agent layer |
| `futures` | 0.3.32 | agents | Async combinators |

### Sound — the runtime's `audio` feature

Added with spec 092 on **2026-10-10**, after the reconciliation stamped above
(the totals at the top do not count it yet).

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rodio` | 0.22.2 | runtime | Plays the `AUDIO-…` built-ins' files: the output device through `cpal`, decoding through `symphonia`. Optional — linked only into a build whose programs call one of the verbs; `rcrun` and the IDE always carry it |

Everything it brings in, with each crate's licence — the column matters here
because one family is not MIT/Apache:

| Crate(s) | Version | Licence | Built for | Role |
|---|---|---|---|---|
| `cpal` | 0.17.3 | Apache-2.0 | every platform | The output device (WASAPI on Windows through the `windows` crate already in the graph) |
| `dasp_sample` | 0.11.0 | MIT OR Apache-2.0 | every platform | Sample-format conversions |
| `symphonia`, `symphonia-core`, `symphonia-metadata`, `symphonia-bundle-mp3`, `symphonia-bundle-flac`, `symphonia-codec-vorbis`, `symphonia-codec-pcm`, `symphonia-format-ogg`, `symphonia-format-riff`, `symphonia-utils-xiph` | 0.5.5 | **MPL-2.0** | every platform | The MP3, FLAC, Vorbis-in-OGG and WAV decoders — pure Rust |
| `extended` | 0.1.0 | MIT | every platform | 80-bit floats in RIFF/AIFF headers (for `symphonia-format-riff`) |
| `alsa`, `alsa-sys` | 0.11.0, 0.4.0 | Apache-2.0/MIT, MIT | Linux | ALSA bindings. `alsa-sys` **links the system's `libasound`** — see below |
| `coreaudio-rs` | 0.14.2 | MIT/Apache-2.0 | macOS | CoreAudio |
| `objc2-core-audio`, `objc2-core-audio-types`, `objc2-audio-toolbox` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | CoreAudio and AudioToolbox bindings |
| `mach2` | 0.5.0 | BSD-2-Clause OR MIT OR Apache-2.0 | macOS | Mach kernel bindings |
| `objc2-avf-audio` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | iOS only | In the lock file, never built for a desktop target |
| `jni` 0.21.1, `cesu8` 1.1.0, `windows-sys` 0.45.0, `windows-targets` 0.42.2 and its seven `windows_*` 0.42.2 target crates | — | MIT/Apache-2.0, every one | Android only | `cpal`'s Android backend and that crate's own build dependencies — in the lock file, never built for a desktop target |

> **Note — `symphonia` is MPL-2.0.** The Mozilla Public License is a
> *file-level* copyleft: the obligation attaches to symphonia's own source
> files, not to the program that links them, and it combines with the
> Apache-2.0 code of this project as a "Larger Work" (MPL-2.0 §3.3). Used
> unmodified, as it is here, what it asks is that anyone who receives a binary
> can get symphonia's source — it is published on crates.io and GitHub, and
> [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) says where. A change to
> one of its files would have to be published under the MPL. rodio offers
> MIT/Apache decoders instead (`hound`, `claxon`, `lewton`, `minimp3`), but its
> MP3 one compiles C, which this project avoids.

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

## Optional features

`cobolt-agents` declares exactly one optional feature, and it is **off** in a
default build:

| Feature | Brings in | Why it is optional |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | NVIDIA GPU embedding on Linux and Windows. Building it needs the CUDA toolkit, which is why it is opt-in; without it the embedder runs on the CPU |

> **Removed at 1.41.4.** This section used to list `tantivy`, `sqlite-vec`,
> `rig-sqlite`, `tokio-rusqlite`, `ort`, `ndarray` and `opentelemetry-otlp`
> behind `local-retrieval` and `otel`. Neither feature exists any more and none
> of those crates is declared anywhere in the workspace — retrieval is served by
> the in-tree store in `cobolt-agents/src/knowledge_store.rs`, which keeps
> embeddings as `Vec<f32>` and compares them with a plain dot product.

---

## The two crates that compile C

Worth knowing when setting up a machine (see [BUILDING-en.md](BUILDING-en.md)):

| Crate | Reached via | What it compiles |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (in `cobolt-runtime`) | The SQLite C amalgamation, bundled so no system SQLite has to match |
| `onig_sys` | `tokenizers` → `onig` | The Oniguruma regex engine |

Nothing in the tree compiles **C++**, and no build script invokes CMake, NASM,
Python, Node or a JVM.

One more crate needs something from the system, though it compiles nothing:
`alsa-sys` (sound, through `rodio` → `cpal`, Linux only) finds the ALSA library
with `pkg-config` and links `libasound.so.2` dynamically. Building on Linux
needs `libasound2-dev` / `alsa-lib-devel`, and every Linux machine running the
IDE, `rcrun` or an application that plays sound needs `libasound2`.
