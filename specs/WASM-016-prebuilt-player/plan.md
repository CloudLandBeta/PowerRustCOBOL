<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The prebuilt player (WASM-016)

- **Status:** implemented for the browser half; one deviation awaits the operator's ruling (no Leptos, section 4)
- **Spec:** ./spec.md   **Date:** 2026-10-20

What is written here is what was built and measured, on these tools: rustc 1.95.0 (stable, MSRV 1.92),
`nightly-2026-10-08` (rustc 1.101.0-nightly `1d81eb4ad`, 2026-10-07) for the threaded variant only,
`wasm-bindgen` 0.2.129 (crate and CLI, equal), `wasm-opt` 133 (binaryen), node v23.3.0, Chrome 152.0.7977.149.

## 1. Approach

The player is **Rust compiled to WebAssembly plus a hand-written loader**. It is split in two halves so that almost
all of it is tested natively, without a browser:

- **The pure core** (host, tree, methods, view, auth, net, weblink, css): `host.rs` is the ABI v1 host
  (R7, R8, R9: commands, queries, tasks, suspension, remote calls, the Authentication machine); `tree.rs` is the
  form tree built from a form description (R3, R5); `methods.rs` the control methods; `view/` turns a tree and the
  page-only state into **plain `VNode` data** (no DOM type appears there); `net.rs` and `auth.rs` are sans-IO machines.
- **The browser half** (`api.rs`, `dom/*`, `superlink.rs`, compiled for `wasm32` only): `dom/apply.rs` patches the real
  DOM from the `VNode` data; events are routed `(Route, kind, key, event, root)` to a `Payload`, then
  `view::interact::on_event` returns `Act`s that `api.rs` applies. The page never receives HTML text: nodes are made with
  `create_element` / `create_text_node` and `set_attribute` only (R15; `tests/sinks.rs` greps for the forbidden APIs).

**R1/R2 — the bundle.** The player ships in two variants (R13c) and three **parts** loaded on first use (R13, Q3):
Mermaid, the PDF reader and the Canvas 2D rasteriser. The loader (`crates/cobolt-web-bundle/assets/loader.js`) chooses
the variant from cross-origin isolation, fetches with integrity digests and streaming instantiation, and hands the player
a `link` object (`perform`, `part`).

**R3, R4 — form descriptions.** Q1 was settled as *the `.cfrm` itself, sanitised*: the player reads the same XML the
desktop reads (`cobolt_forms::xml::load_form_from_str`), so the model has one parser; the bundle assembler removes
secrets and program code before writing it. **R11 — versions.** `versions()` answers the ABI, the form-description
format and the player; `tests/abi_host.rs` and the bundle tests refuse a module that needs a host import the player
lacks.

**R12 — build and freshness.** `examples/build_player.rs` (the `build_chunked_kb` pattern) builds both variants and the
parts with the pinned tools and writes `assets/web-player/player-build.json`: the source fingerprint
(`src/fingerprint.rs`, SHA-256 over every input file, in a fixed order), the sizes, the tools and the digest of every
shipped file. `tests/freshness.rs` (AC7) recomputes the fingerprint and the digests.

**R6, R10, R14** are delivered control by control and form by form; the list of what is and is not in the browser is in
tasks.md. **R16** (Preview accepts a new module, description or stylesheet without reloading the player) is the
`set_stylesheet` / `add_form` / re-`attach` paths of `api.rs`; the Preview itself is WASM-008.

## 2. Affected crates / files

- `crates/cobolt-web-player/` — **its own Cargo workspace** (reasons in its `Cargo.toml`): `src/{host,tree,methods,net,auth,
  weblink,css,base_css,layout_css,kinds,value,par,fingerprint}.rs`, `src/view/*`, `src/dom/*`, `src/api.rs`,
  `src/superlink.rs`, `examples/build_player.rs`, `parts/{mermaid,pdf,canvas}`, `vendor/{mermaid-rs-renderer,time}`,
  `tests/{abi_host,dependency_guard,sinks,freshness}.rs`.
- `crates/cobolt-web-bundle/` — the assembler, the loader (`assets/loader.js`), the page, the dev server and the node harness.
- `crates/cobolt-forms/` — `pdf-reader` and `mermaid` became default-on features so the player can depend on the form
  model with `default-features = false` (`lopdf` pulled `time` 0.2 and its `stdweb`, which does not build for the browser).
- `specs/WASM-ABI-v1.md` — amendments A5 (tasks across instances), A6 (error frames), A7 (the application and form verbs,
  the argument text of each event), A8 (the connection envelope).
- Not touched on 2.x.x: `README.md`, `docs/`, `version.rs`, `CHANGELOG.md`, `WASM-PROGRESS.md` (the coordinator's).

## 3. Data / model changes

No `.cfrm` change. The player adds **page-only state** that is never written back: the open menu, grid windows and
sort, the find total, the map drag, the dropzone highlight, the focused control. Route parameters (WASM-011) are carried
by the host (`Params`) and the address; limits `MAX_PARAMS` 16, `MAX_PARAM_NAME` 64, `MAX_PARAM_VALUE` 512.
`player-build.json` gains `source_hash` and a `files` list (path, bytes, sha256).

## 4. Key decisions & alternatives

- **Decision: no Leptos (R1, Q2, Q5).** The player is plain Rust over `web-sys`, with a `Surface` trait between the
  host and the DOM. Why: the view layer is data (`VNode`) that native tests assert on, which a reactive framework would
  have hidden; a framework adds size (the budget is a startup cost the operator cares about) and a second build tool
  (`trunk`) in a build that must run with the stable toolchain only. **Rejected:** Leptos CSR with `trunk`. This
  contradicts the sentence in R1 and the ruling of 2026-10-19 (Q7 of WASM-002); **it needs the operator's ruling** and
  is contained: nothing outside `dom/` and `api.rs` knows it.
- **Decision: a separate Cargo workspace.** `wasm-bindgen` must equal the installed CLI exactly (0.2.129) while the main
  lock pins 0.2.126; the browser crates add about a hundred packages only a `wasm32` build needs; the nightly and
  `-Zbuild-std` stay confined to this build step (WASM-017 R16). `tests/dependency_guard.rs` checks the closure.
- **Decision: `opt-level = "z"`.** Measured on the same source: `z` 2,136,155 bytes (gzip 737,875), `s` 2,718,057.
  Plain, 2026-10-20, the shipped build (Find, TextBox pictures, route parameters, the PDF and Canvas wiring, `super::`): **2,222,253 bytes
  (gzip 772,899)**, glue 72,056; threaded **2,187,394 (gzip 773,318)**, glue 78,159.
- **Decision: parts instead of one file (Q3).** Mermaid was 1.9 MB of a 3.89 MB player; as a part the player is 2.14 MB.
  Parts: Mermaid 1,906,933 bytes (gzip 700,017), Canvas 2D 469,419 (gzip 180,008), PDF 219,273 (gzip 101,928). A form
  without a diagram, a PDF or a canvas fetches none of them (the bundle tests inspect what was fetched, AC8).
- **Decision: the loader's `part(name, fn, arg)` answers text; `partModule(name)` answers the module** (the Canvas part
  deals in bytes and objects). Both go through the same integrity-checked load.
- **Decision: `vendor/mermaid-rs-renderer` and `vendor/time` are patched copies** applied by `[patch.crates-io]` to this
  workspace only. The first differs from 0.2.2 by a clock that exists on `wasm32` (`Instant::now()` panicked); the second
  from `time` 0.2.27 by dropping `stdweb`. Each says so in its own `Cargo.toml` and is to be dropped when upstream or
  `cobolt-forms` (a PDF reader that does not need `time` 0.2) removes the need.
- **Decision: Rayon's pool cannot be disposed** (R13b caveat). `wasm-bindgen-rayon` offers no way to stop the workers;
  `unmount()` releases everything else and the page keeps the pool until it is closed.

## 5. Risks & mitigations

- Risk: the player and the module drift. → The ABI version and the import check refuse a mismatch (AC6); the generator
  is held equal by WASM-010's conformance runs.
- Risk: a stale shipped player. → `tests/freshness.rs` (AC7), shown to fail on an edited source and pass after a rebuild.
- Risk: the threaded variant needs a nightly. → Pinned (`nightly-2026-10-08`), built only by `build_player`, never
  required at run time: a page that is not isolated runs the plain variant on the main thread.
- Risk: the browser matrix is wider than the machine. → See WASM-015's plan: Chrome 152 is measured, the rest is named
  as unmeasured with the reason.

## 6. Test strategy

Native (no browser): `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml` — the lib (host, tree, methods,
every view, the interaction layer, the picture and Find logic, the parameters, `super::`), `tests/abi_host.rs` (the host
against the fixture module in `wasmi`), `tests/sinks.rs`, `tests/dependency_guard.rs`, `tests/freshness.rs`.
`cargo test -p cobolt-web-bundle` — the assembler, the CSP, the no-string-execution guard and the node end-to-end
scenarios (loader, parts, variants, hosted sign-in, backend). In Chrome 152, by serving the bundle on 127.0.0.1 only:

- **Variant choice, end to end.** Isolated page (COEP `require-corp` and `credentialless`): the threaded variant on a
  Worker with a pool of 8 workers. Non-isolated page: the plain variant on the main thread, and the threaded files are
  not fetched.
- **Rayon DataGrid sort**, 300,000 rows: the result equals a stable JavaScript sort; warm time 128-138 ms threaded
  against 150-172 ms plain (parsing and serialising are sequential, which bounds the gain).
- **Parts on demand:** Mermaid and PDF fetched on first use only; a PDF page showed its size and its text.
- **Authentication, WebSocket, tiles, find, pictures, map pan, knob, drop zone, `super::`:** see WASM-004 / 006 / 011.

## 7. Steering compliance

- [x] i18n: the player has no user-facing string of the IDE; its own texts are English by the loader's `texts` table.
- [x] COBOL identifiers English; no "cobolt" in user-facing text (crate names are build-only).
- [x] `README.md` and `docs/` untouched (2.x.x rule).
- [x] Fix vs feature: feature, on `features-2x`; `z` and the CHANGELOG are the coordinator's on each merge.
- [ ] Leptos (R1): **deviates, needs the operator's ruling.**
