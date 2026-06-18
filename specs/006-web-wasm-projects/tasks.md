# Tasks — Web projects (WebAssembly target)

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-18

Ordered, independently-verifiable tasks grouped by the plan's phases (§8). The
project stays green (and Desktop unchanged) after each task. The transpiler's
**full-parity long tail is out of scope here** — T11 ships a representative
subset; the remaining verbs go to a dedicated **sub-spec 007**.

## Phase A — Project-type model + two-step New Project + gating

- [ ] **T1 — `ProjectKind` registry + manifest `kind`** (R1, R15)
  - Files: `crates/cobolt-ide/src/project_model.rs` (add `ProjectKind` registry:
    id ↔ display name ↔ capability flags `{interpreted_run, native_build,
    wasm_build, portable_only}`; `ProjectMeta.kind` with `#[serde(default)]` →
    `desktop`); `crates/cobolt-compiler/src/lib.rs` + `crates/cobolt-cli/src/main.rs`
    (their duplicate `CoboltProject` gain `kind`, default desktop).
  - Do: model the extensible type set (v1: `desktop`, `web`); old manifests with
    no `kind` load as `desktop`.
  - Verify: `cargo test -p cobolt-ide` — serde test: a `kind`-less manifest loads
    as `desktop`; `kind = "web"` round-trips. `cargo build -p cobolt-compiler -p
    cobolt-cli`.

- [ ] **T2 — Two-step New Project (type picker → details)** (R2; AC1)
  - Files: `crates/cobolt-ide/src/app.rs` (new project-type picker dialog driven
    by the `ProjectKind` registry, opening before `NewProjectDialog`;
    `create_new_project` records the chosen `kind`); `crates/cobolt-ide/src/i18n.rs`
    (picker strings ×6).
  - Do: New Project first shows the picker (Desktop App | WebAssembly); choosing a
    type opens the details dialog; the created `cobolt.toml` has `kind`.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n`. Manual:
    New Project → picker → choose WebAssembly → details → created manifest has
    `kind = "web"`.

- [ ] **T3 — Capability-driven action gating** (R3, R14; AC7)
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/panels/{toolbar.rs,
    project.rs,settings_form.rs}`, `crates/cobolt-ide/src/i18n.rs` (web action
    strings ×6).
  - Do: read the kind's capabilities to gate — Desktop unchanged (interpreted
    Run/Debug, native Build); Web hides interpreted Run/Debug and shows Build
    (WASM) / Preview-in-browser. Show the type in the project view/settings.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide`. Manual: a Web
    project hides interpreted Run/Debug + shows web actions; a Desktop project is
    visually/behaviourally identical to before.

## Phase B — Web-portability validator

- [ ] **T4 — `cobolt-semantic` web-portability pass** (R10, R11)
  - Files: `crates/cobolt-semantic/src/web_portability.rs` (+ `lib.rs` wiring,
    opt-in by a `web` flag/param).
  - Do: flag non-portable constructs as errors — disk `OPEN`/`ASSIGN` to a path,
    disk-backed indexed files, and verbs needing native I/O / threads / OS
    services; portable programs pass clean.
  - Verify: `cargo test -p cobolt-semantic` — non-portable samples flagged,
    portable samples clean (report counts).

- [ ] **T5 — Wire the web pass into Check/Build** (R10; AC4)
  - Files: `crates/cobolt-ide/src/app.rs` (do_check/do_build branch on kind),
    `crates/cobolt-cli/src/main.rs` (`rcrun check`/`build` run the web pass for
    web projects).
  - Do: a Web project runs the portability pass at Check and Build; errors block
    the bundle.
  - Verify: `cargo build -p cobolt-cli`. `rcrun check` on a web project that opens
    a disk file → portability error, no bundle (AC4); a portable web project →
    clean.

## Phase C — WASM toolchain + eframe-web shell

- [ ] **T6 — Toolchain detect + auto-install** (R7)
  - Files: `crates/cobolt-compiler/src/` (new `wasm_toolchain.rs`).
  - Do: detect the `wasm32-unknown-unknown` target + `trunk`; auto-install when
    missing (`rustup target add …`; `cargo install trunk`) with progress; clear,
    actionable error on failure.
  - Verify: `cargo build -p cobolt-compiler`. Manual: first Web build installs (or
    detects) target + trunk; simulate-missing → actionable message.

- [ ] **T7 — Generate eframe-web project + `trunk build` (shell)** (R4 skeleton,
      R13 skeleton)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`build_web_project`: generate a
    wasm Cargo project — `Cargo.toml`, `index.html` for trunk, a placeholder
    eframe-web `main` that renders the form statically — then `trunk build` to a
    `web/`/`dist/` bundle).
  - Do: produce a self-contained bundle (host HTML + `.wasm` + JS glue + assets);
    no interpreter embedded.
  - Verify: web Build emits the bundle. Manual: open the bundle (served) → the
    form's controls render on a canvas (static, pre-transpiler).

- [ ] **T8 — Preview = `trunk serve` + open browser** (R8; AC3 skeleton)
  - Files: `crates/cobolt-ide/src/app.rs` (web Preview/Run action),
    `crates/cobolt-compiler/src/wasm_toolchain.rs` (serve helper),
    `crates/cobolt-ide/src/i18n.rs` (strings ×6).
  - Do: Preview builds + `trunk serve` on `127.0.0.1:PORT` + opens the default
    browser; rebuild-on-save.
  - Verify: `cargo build -p cobolt-ide`. Manual: Preview opens the browser at the
    serve URL showing the form; no interpreted preview window.

## Phase D — Transpiler (representative subset) + `cobolt-web-rt`

- [ ] **T9 — New crate `cobolt-web-rt` (portable runtime support)** (R6, R12)
  - Files: `crates/cobolt-web-rt/` (re-export/feature-gate `cobolt-runtime`'s
    portable value core — `value`, `numedit`, `objects`, in-mem `indexed`;
    add a **wasm HTTP client** behind the existing `INVOKE "GET"/…` object using a
    poll-based fetch e.g. `ehttp`); `crates/cobolt-runtime/Cargo.toml` (a
    `portable` feature excluding disk engines + threads).
  - Do: provide COBOL value/PIC/MOVE semantics, memory-only indexed store, and
    REST — all `wasm32`-clean (no `std::fs`, no threads).
  - Verify: `cargo build -p cobolt-web-rt --target wasm32-unknown-unknown`;
    value-parity unit tests vs `cobolt-runtime`.

- [ ] **T10 — Shared `form_app` (egui render + dispatch)** (R13)
  - Files: `crates/cobolt-forms/src/form_app.rs` (factor control rendering + event
    dispatch into a portable module parameterised by a program "driver" trait).
  - Do: one renderer usable by the desktop binary and the web binary (visual
    parity).
  - Verify: `cargo build -p cobolt-forms`; `cargo test -p cobolt-forms`; desktop
    output still renders identically (regression).

- [ ] **T11 — New crate `cobolt-transpiler` (representative subset)** (R4, R5)
  - Files: `crates/cobolt-transpiler/` (COBOL `Program` AST → portable Rust:
    DATA items → typed state; paragraphs/handlers → fns; verbs of the
    **representative subset** — MOVE/arithmetic/COMPUTE, IF/EVALUATE/PERFORM,
    `INVOKE`/`obj::method()` UI calls, the REST object — → calls into
    `cobolt-web-rt`). Long-tail verbs ⇒ **sub-spec 007**.
  - Do: emit Rust with no interpreter dependency, carrying a generated header.
  - Verify: `cargo test -p cobolt-transpiler` — golden tests (AST → expected Rust)
    **and** native-execution tests (compile the transpiled Rust natively; assert
    it reproduces the interpreter's observable output for the same program).
    Report `verbs covered / total`.

- [ ] **T12 — Wire transpiler into `build_web_project`** (R4, R5; AC2, AC6)
  - Files: `crates/cobolt-compiler/src/lib.rs` (replace T7's placeholder: the web
    `main` drives `form_app` with the transpiled program + `cobolt-web-rt`).
  - Do: a real form (representative subset) transpiles, builds, and runs in the
    browser; indexed files are memory-only.
  - Verify: web Build of a representative form → bundle (no interpreter). Manual:
    browser run — buttons/handlers work; no disk artifacts created (AC6).

- [ ] **T13 — REST data + memory-indexed example** (R12; AC5)
  - Files: `examples/web-rest/` (a small Web project: a form that
    `INVOKE Rest "GET"`s and shows records), builder if needed.
  - Do: demonstrate the REST data path against a mock/stub backend.
  - Verify: Manual: the form displays records fetched over HTTP (mock backend);
    data path is REST, not disk (AC5).

## Phase E — Docs & finalize

- [ ] **T14 — Docs & i18n** (R16, R17; AC8)
  - Files: `docs/developers-guide-en.md` (new "Web projects" section: type picker,
    portable subset, REST data model, build/preview to browser);
    `crates/cobolt-ide/src/i18n.rs` (confirm all new `Tr` keys ×6).
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations). English guide
    only (translations untouched).

- [ ] **T15 — Steering updates (R18 + source maps)**
  - Files: `specs/steering/product.md` (outputs include web/WASM),
    `specs/steering/tech.md` (wasm32 build path + trunk/wasm-bindgen toolchain);
    `crates/cobolt-compiler/` (emit source maps where the toolchain supports it —
    R18; no in-IDE web debugger).
  - Verify: review; `cargo build -p cobolt-compiler`. Bundle carries source maps.

- [ ] **T16 — Finalize** (all ACs)
  - Files: `crates/cobolt-ide/src/version.rs` (+ `CHANGELOG.md`) — feature minor
    bump.
  - Do: full workspace build + test; manual AC walkthrough.
  - Verify: `cargo build --workspace` + `cargo test --workspace` green. Manual:
    AC1 (picker → manifest), AC2 (bundle, no interpreter, canvas), AC3 (Preview →
    browser), AC4 (disk → portability error), AC5 (REST data), AC6 (memory
    indexed), AC7 (Desktop regression), AC8 (i18n + docs).

## Done criteria
All acceptance criteria in `spec.md` are covered (AC1: T1/T2 · AC2: T7/T12 · AC3:
T8/T12 · AC4: T5 · AC5: T13 · AC6: T12/T13 · AC7: T3/T16 · AC8: T14), tests pass,
docs updated, steering updated, and the work is split into feature commit(s) per
the operator's rules (do **not** commit/push unless the operator asks). The
transpiler's full-parity long tail continues in **sub-spec 007**.
