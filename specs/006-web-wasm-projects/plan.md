# Plan — Web projects (WebAssembly target)

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-18

## 1. Approach

Introduce a project **`kind`** (`desktop` | `web`) into the manifest and IDE
model, and make every type-sensitive action branch on it (R1–R3, R14–R15). A
**Web** project's build is a **second pipeline** alongside the existing
`cobolt-compiler` native binary: regenerate the `.cbl` (unchanged contract) →
parse → **transpile the COBOL AST to portable Rust** (new `cobolt-transpiler`) →
generate a small **eframe-web Cargo project** that links the transpiled program
against a portable runtime-support crate (new `cobolt-web-rt`) → build with
**`trunk`** to a wasm bundle (R4–R7). **Preview = `trunk serve` + open browser**
(R8). A **web-portability pass** in `cobolt-semantic` rejects non-portable COBOL
at Check/Build (R10–R11). Data persistence uses the **existing `INVOKE "GET"/…`
REST object**, re-backed by a wasm HTTP client in `cobolt-web-rt` (R12). The UI
is the **same egui form** rendered to a `<canvas>` via the eframe web backend
(R13), so we reuse `cobolt-forms` rendering rather than emitting DOM.

The transpiler targets **feature parity** with the interpreter for portable
constructs (R4) — too large for one feature. This plan ships a **representative
subset end-to-end** and spins the long tail of verbs into a **dedicated
transpiler sub-spec (007)**.

## 2. Affected crates / files

**New crates**
- `crates/cobolt-transpiler/` — COBOL `Program` AST → portable Rust source
  (paragraphs/handlers → Rust fns; DATA items → typed state; verbs → calls into
  `cobolt-web-rt`). The central new component; grows toward parity over phases.
- `crates/cobolt-web-rt/` — portable runtime support the transpiled code links
  against: COBOL value/PIC/edit/MOVE semantics, in-memory indexed store, the
  shared **form-app loop** (egui render + event dispatch), and a **wasm HTTP
  client**. Must compile to `wasm32-unknown-unknown` (no `std::fs`, no threads).

**Reused / changed**
- `crates/cobolt-runtime/` — extract or **feature-gate** the portable value core
  (`value.rs`, `numedit.rs`, `objects.rs`, in-mem `indexed.rs`) so `cobolt-web-rt`
  reuses COBOL semantics without the interpreter loop, disk engines, or threads.
- `crates/cobolt-forms/` — factor the control **rendering + event dispatch** into
  a portable `form_app` module shared by the desktop binary and the web binary
  (visual parity, R13).
- `crates/cobolt-compiler/src/lib.rs` — add `build_web_project` (the wasm
  pipeline) beside `build_project`; read `kind` to branch. Toolchain detect +
  auto-install (R7); `trunk build` / `trunk serve` invocation (R8).
- `crates/cobolt-semantic/` — new `web_portability.rs` pass (R10/R11), run for web
  projects at Check/Build.
- `crates/cobolt-ide/src/project_model.rs` — `ProjectMeta.kind` (serde default
  `desktop`); load/save.
- `crates/cobolt-ide/src/app.rs` — a **two-step New Project flow**: a new
  **project-type picker** dialog (Desktop App | WebAssembly in v1, driven by the
  `ProjectKind` registry) opens first; choosing a type then opens the existing
  `NewProjectDialog` (name/version/main), which records the chosen `kind` in
  `create_new_project`. Plus action gating by `kind` (web → Build-WASM / Preview-
  in-browser, hide interpreted Run/Debug) and wiring the web build + portability
  check.
- `crates/cobolt-ide/src/panels/{toolbar.rs,project.rs,settings_form.rs}` — show
  type; web-specific toolbar actions.
- `crates/cobolt-cli/src/main.rs` — its duplicate `CoboltProject` gains `kind`;
  `rcrun build` branches desktop/web; `rcrun check` runs the web pass for web.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6 (type chooser, web build/
  preview actions, portability diagnostics, toolchain progress/errors) (R16).
- `docs/developers-guide-en.md` — new "Web projects" section (R17).
- `specs/steering/product.md`, `tech.md` — broaden "standalone binaries" → also
  web/WASM; record the wasm build-time toolchain.
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — feature minor bump at
  finalize.

## 3. Data / model changes
- **Manifest `[project].kind`** (`"desktop"` default via `#[serde(default)]`), in
  the three `CoboltProject`/`ProjectMeta` definitions (ide `project_model.rs`,
  `cobolt-compiler`, `cobolt-cli`). Old manifests with no `kind` load as
  `desktop` (R15, AC1). **No `.cfrm` change.**
- **`ProjectKind` is an extensible registry** (string id ↔ display name ↔
  capabilities), v1 entries `desktop` and `web`, with a single extension point so
  future types (`cli`/Command Prompt, `android`, `ios-iphone`, `ios-ipad`, …) are
  added in one place and automatically surface in the type picker (R1). Each kind
  carries flags driving the IDE (interpreted-run? native-build? wasm-build?
  portable-only?), so gating reads capabilities, not a hard-coded match.
- **Web output folder** (e.g. `web/` or `dist/`) for the bundle; treated as
  generated output (gitignore-able), like `generated/` and `bin/`.
- The transpiled Rust + the generated wasm Cargo project are **build artifacts**
  (never hand-edited); the transpiled Rust carries its own generated header.

## 4. Key decisions & alternatives
- **Transpile to Rust (not interpreter-in-wasm).** — Why: the user chose no
  interpreter in the artifact + smallest/most portable output (Q1). Rejected:
  compiling `cobolt-runtime` to wasm (simpler, but ships the interpreter and the
  non-portable disk/thread code).
- **`cobolt-web-rt` reuses `cobolt-runtime`'s value core via feature-gating.** —
  Why: avoid duplicating COBOL numeric/edit/MOVE semantics. Rejected: a brand-new
  COBOL value implementation (drift risk) or full duplication.
- **Shared `form_app` (egui) loop for desktop + web.** — Why: visual parity (R13)
  and one renderer. Rejected: a separate web renderer (divergence, double
  maintenance). *Phase note:* the shared refactor can trail a Phase-C stub.
- **`trunk` as bundler + dev server, auto-installed (Q3/Q5).** — Why: the
  egui/eframe-standard web tool; `trunk serve` solves the `file://` problem. Auto-
  install per Q3. Rejected: wasm-pack (npm-oriented), hand-rolled wasm-bindgen.
- **Explicit REST object, no FD↔REST mapping (Q2).** — Why: user choice; far less
  transpiler surface. Rejected: `ASSIGN TO "https://…"` auto-marshal (large, can
  be a later enhancement).
- **`kind` fixed at creation (Q4).** — Why: simplest, avoids re-validate/migrate.
  Rejected: convertible projects (future).
- **Two-step New Project (type picker → details) + capability-driven gating.** —
  Why: the type must be chosen before the details make sense (a Web project may
  later want different details), and an extensible `ProjectKind` registry with
  capability flags lets future types (Command Prompt, Android, iPhone, iPad) drop
  in without touching every `match`. Rejected: a single dialog with a type
  dropdown (works for two types, but doesn't scale to per-type detail forms) and a
  closed `Desktop|Web` enum (forces edits across the codebase per new type).

## 5. Risks & mitigations
- **Transpiler scope = full parity (huge).** → Ship a **representative subset**
  (Phase D); move the long tail to **sub-spec 007**; golden-output + native
  execution tests gate each verb as it lands.
- **`cobolt-runtime` is not wasm-clean** (`std::fs`, threads, native `reqwest`).
  → Feature-gate a `portable` build (exclude disk engines + threads); audit
  `value.rs`/`numedit.rs`/`objects.rs`; CI builds `cobolt-web-rt` for
  `wasm32-unknown-unknown`.
- **Async REST in a sync egui loop.** → Use a poll-based fetch (e.g. `ehttp`)
  integrated with the egui frame; the `INVOKE "GET"` object fills its result on a
  later frame (document the async semantics).
- **eframe-web / `trunk` / `wasm-bindgen` version coupling.** → Pin versions in
  the generated Cargo project; surface `trunk`/target install/build failures with
  actionable messages (R7).
- **Auto-install side effects** (`rustup target add`, `cargo install trunk`). →
  Detect first; show a clear progress/console line; handle offline + failure
  without corrupting the project; never silently retry.
- **Visual parity desktop↔web.** → Drive both from `cobolt-forms` `form_app`;
  a Phase-C stub may differ until Phase D unifies — call out in tasks.

## 6. Test strategy
- **`cobolt-transpiler`** (per phase): golden tests (AST → expected Rust for each
  verb) + **native-execution** tests — compile the transpiled Rust *natively* in a
  test harness and assert it reproduces the interpreter's observable output for
  the same program (avoids wasm-in-CI). Report `N verbs covered / M`.
- **`cobolt-semantic` web pass:** unit tests — non-portable constructs (disk
  `OPEN`/`ASSIGN`-to-path, disk indexed, threads) flagged as errors; portable
  programs pass clean. Report counts.
- **`cobolt-web-rt`:** builds for `wasm32-unknown-unknown` in CI; value-core
  parity tests against `cobolt-runtime`.
- **`project_model`:** serde test — a `kind`-less manifest loads as `desktop`;
  round-trip of `kind = "web"`.
- **IDE:** `cargo test -p cobolt-ide i18n` (×6, no empty); gating unit checks.
- **Manual/visual:** create a Web project → Build emits a bundle (no interpreter)
  → Preview opens the browser at the `trunk serve` URL showing the form on a
  canvas (AC2/AC3); a disk-file program → Check error, no bundle (AC4); a REST
  form shows fetched data against a mock backend (AC5); a Desktop project builds/
  runs unchanged (AC7).

## 7. Steering compliance
- [ ] i18n: all new UI strings in 6 languages (R16).
- [ ] Generated-code banner + regenerate-on-action preserved (the `.cbl` contract
      is unchanged; the transpiled Rust is a new artifact with its own header).
- [ ] English dev guide updated; translations untouched (R17).
- [ ] Fix vs feature: **feature** → minor version bump + CHANGELOG at finalize.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/source English.
- [ ] product.md / tech.md steering updated for the web target + wasm toolchain.

## 8. Phasing (proposed for /tasks)
- **Phase A — Project-type model + two-step New Project + IDE gating.** The
  extensible `ProjectKind` registry + `kind` in model/compiler/cli; the **type
  picker** dialog (Desktop App | WebAssembly) feeding the existing details dialog;
  default-desktop compat; capability-driven action gating; i18n. Desktop
  unchanged. (R1–R3, R14–R16; AC1, AC7.)
- **Phase B — Web-portability validator.** `cobolt-semantic` web pass wired into
  Check/Build for web projects. (R10–R11; AC4.)
- **Phase C — WASM toolchain + eframe-web shell.** Toolchain detect/auto-install,
  generate a minimal eframe-web project (placeholder render), `trunk build`,
  Preview via `trunk serve` + browser. Proves the pipeline. (R7–R8, R13 skeleton.)
- **Phase D — Transpiler (representative subset) + `cobolt-web-rt`.** Transpile
  data items, MOVE/arithmetic/COMPUTE, IF/EVALUATE/PERFORM, INVOKE UI calls, and
  the REST object; `cobolt-web-rt` (portable value core + shared `form_app` +
  wasm http); a real form runs in the browser. (R4–R6, R12; AC2–AC3, AC5–AC6.)
  **Long-tail verbs → sub-spec 007.**
- **Phase E — Docs + finalize.** Guide section, source maps (R18), steering
  updates, version bump/CHANGELOG. (AC8.)
