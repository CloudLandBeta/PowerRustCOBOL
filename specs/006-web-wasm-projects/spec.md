# Spec — Web projects (WebAssembly target)

- **Status:** draft → approved
- **Folder:** specs/006-web-wasm-projects/
- **Author:** Anthropic Code Agent   **Date:** 2026-06-18

## 1. Overview

PowerRustCOBOL today builds exactly one kind of application: a **desktop** app —
an eframe window that embeds the RustCOBOL tree-walking interpreter plus the
program AST. This feature introduces **project types** and the first new one,
**Web**. A Web project **transpiles its COBOL to portable Rust and compiles that
to WebAssembly** (no interpreter shipped), packaged as an eframe **web** bundle
that renders the same egui form to a browser `<canvas>`. Web projects have **no
interpreted mode** — Preview/Run **build the bundle and open the default
browser**. Their code is a **100 % portable** subset (no `std::fs`, no native
I/O, no OS threads), **enforced at Check/Build**: indexed files are **memory
only**, and record/file data is loaded and saved via **REST API calls** to a
backend (the backend itself is a separate, future project type). Desktop projects
are unchanged.

## 2. Goals / Non-goals

### Goals
- A first-class **project type** (Desktop | Web), chosen at project creation,
  stored in the manifest, that drives build target, preview/run behaviour, and
  feature gating.
- A **COBOL → portable Rust transpiler** targeting **feature parity** with the
  interpreter for every *portable* construct (non-portable ones are rejected, not
  transpiled), with no interpreter dependency, compiled to the `wasm32` target.
- A **Web build** that produces a self-contained eframe-web bundle (host HTML,
  `.wasm`, JS loader/glue, form assets) that renders the form on a `<canvas>`.
- **Preview = `trunk serve` + open browser** for Web projects; no in-IDE
  interpreter.
- A **portability validator** that rejects non-portable COBOL in Web projects at
  Check and Build.
- **Memory-only** indexed files; persistent/record data via the **existing
  REST/HTTP client object** (explicit `INVOKE` calls; the developer marshals).
- Desktop projects, manifests, and outputs **unchanged**.

### Non-goals (explicitly out of scope)
- The **backend** project type (the REST server). Only the Web **client** side
  (calls to REST) is in scope here.
- **Implementing the future project types** (Command Prompt, Android, iPhone,
  iPad). v1 ships **Desktop App + Web**; the type model and the type picker are
  built to *accommodate* more types, but only those two are implemented now.
- **HTML/DOM** rendering of forms (the UI is the egui canvas, not generated DOM).
- **Multi-threaded** wasm.
- Hosting/deployment of the produced bundle to a real server (we emit it; a local
  `trunk serve` is only for Preview).
- A web **debugger** in the IDE — browser devtools serve that role in v1 (the
  IDE's interpreted debugger stays Desktop-only).
- **Automatic FD/SELECT → REST mapping.** v1 uses the **explicit REST/HTTP client
  object**; the transpiler does not turn a file into a REST resource.
- Changing a project's type after creation (v1: **fixed at creation**).

## 3. User stories
- As a developer, I want to pick **Web** when creating a project, so my app
  compiles to WebAssembly and runs in a browser.
- As a developer, I want **Preview** to open my web app in the browser, so I see
  it exactly as end users will.
- As a developer, I want the IDE to **flag non-portable COBOL** in a Web project,
  so I never ship something that can't run in the browser.
- As a developer, I want record/file data to come from **REST calls**, so my web
  app has no local-disk dependency.
- As a developer building desktop apps, I want **nothing to change** for my
  existing projects.

## 4. Requirements (EARS)

**Project type**
- **R1 (ubiquitous):** The system shall support an **extensible** project **type**
  set — `Desktop App` and `Web` (WebAssembly) in v1, designed to admit further
  types later (e.g. Command Prompt, Android, iPhone, iPad) — persisted in the
  project manifest and reloaded with the project.
- **R2 (event):** When the user starts a new project, the system shall **first**
  present a **project-type picker** (Desktop App | WebAssembly in v1); only after a
  type is chosen shall the New Project details dialog (name, version, main file)
  open. The chosen type is recorded and **fixed** for the life of the project.
- **R3 (state):** While a Desktop project is open, the system shall behave exactly
  as today (interpreted Preview/Run/Debug, native single-binary build).

**Transpile + WASM build**
- **R4 (event):** When the user builds a Web project, the system shall transpile
  each form's COBOL (the generated program + handlers + user procedures) to
  portable Rust — **targeting feature parity with the interpreter for every
  portable construct** — compile it to the `wasm32` target, and emit an eframe-web
  bundle (host HTML page, the `.wasm`, the JS loader/glue, and form assets) into
  the project's web output folder.
- **R5 (constraint):** The Web build shall **not** embed the tree-walking
  interpreter; program logic shall be **transpiled Rust**.
- **R6 (constraint):** The transpiled Rust and the bundle shall be **100 %
  portable** — no `std::fs`, no native filesystem/OS access, no OS threads — so
  they run unmodified in a browser sandbox.
- **R7 (event):** The Web build shall use the `wasm32-unknown-unknown` Rust
  target, the eframe **web** backend, `wasm-bindgen`, and **`trunk`** as the
  bundler. When the wasm target or `trunk` is missing, the system shall
  **auto-install** it on first Web build (`rustup target add
  wasm32-unknown-unknown`; `cargo install trunk`), surfacing progress and
  reporting any install failure clearly.

**Preview / run**
- **R8 (event):** When the user invokes Preview or Run on a Web project, the
  system shall build the bundle, serve it over local HTTP via **`trunk serve`**
  (`127.0.0.1:PORT`), and open that URL in the **default web browser**; saving
  rebuilds (a browser cannot load wasm from `file://`).
- **R9 (constraint):** The system shall **not** run a Web project through the
  in-IDE interpreter, and shall not offer interpreted Preview/Run/Debug for Web
  projects.

**Portability enforcement**
- **R10 (event):** When the user runs Check or Build on a Web project, the system
  shall validate the COBOL against the **portable subset** and report every
  non-portable construct as an **error**, producing no bundle until they are
  resolved. Non-portable constructs include disk file `OPEN`/`ASSIGN` to a
  filesystem path, disk-backed indexed files, and any verb requiring native I/O,
  threads, or OS services.
- **R11 (state):** While a Web project is open, indexed files shall be
  **memory-only**; the system shall treat disk-backed indexed storage as
  non-portable.

**Data via REST**
- **R12 (optional):** Where a Web form reads or writes persistent/record data, the
  developer shall use the **existing REST/HTTP client object** (`INVOKE Rest
  "GET"/"POST"/"PUT"/"DELETE" …`), marshaling records to/from the response **by
  hand**. The transpiler shall **not** auto-map an `FD`/`SELECT` to a REST
  resource in v1. The backend is out of scope; the Web project holds only the
  client calls.

**UI rendering**
- **R13 (ubiquitous):** The Web application shall render the form via egui to a
  browser `<canvas>` (eframe web backend), matching the designer/desktop
  appearance — no HTML/DOM widget generation.

**Feature gating / desktop parity**
- **R14 (state):** While a Web project is open, the IDE shall gate actions by
  type — replace interpreted Preview/Run/Debug with the Web build + open-in-
  browser flow, and surface portability diagnostics.
- **R15 (constraint):** Desktop project behaviour, manifest schema (for existing
  fields), and build outputs shall be **unchanged** by this feature; an existing
  project with no type defaults to `Desktop`.

**Cross-cutting (steering)**
- **R16 (constraint):** Every new user-facing IDE string (type chooser, Web
  build/preview actions, portability diagnostics, toolchain errors) shall be a
  `Tr` field translated in **all six** languages.
- **R17 (constraint):** The English `docs/developers-guide-en.md` shall document
  Web projects: the portable subset, the REST data model, and build/preview to
  the browser. Translations are user-maintained (not edited here).
- **R18 (constraint):** There shall be **no in-IDE web debugger** in v1;
  developers use the browser devtools. The Web build should emit **source maps**
  where the toolchain supports it. The IDE's interpreted debugger remains
  Desktop-only.

## 5. Acceptance criteria
- [ ] **AC1** — Starting a new project first shows a **type picker** (Desktop App
  | WebAssembly); choosing a type then opens the details dialog (name/version/main),
  and the chosen type is written to and reloaded from the manifest. An existing
  (typeless) manifest loads as Desktop.
- [ ] **AC2** — Building a Web project produces a bundle (host HTML + `.wasm` +
  JS glue + assets) with **no interpreter** embedded; loaded in a browser it
  renders the form on a `<canvas>`.
- [ ] **AC3** — Preview/Run on a Web project builds and **opens the default
  browser**; no interpreted preview window appears.
- [ ] **AC4** — Check/Build of a Web project that uses a disk file (OPEN/ASSIGN to
  a path, or a disk-backed indexed file) reports a **portability error** and emits
  no bundle.
- [ ] **AC5** — A Web form that fetches data with the REST/HTTP client object
  (`INVOKE Rest "GET" …`) displays records parsed from the HTTP response (against
  a stub/mock backend) — the data path is REST, not disk.
- [ ] **AC6** — Indexed files in a Web project are memory-only (no disk artifacts
  created at build or run).
- [ ] **AC7** — Desktop projects build/run/preview exactly as before (regression
  green; existing tests unaffected).
- [ ] **AC8** — All new IDE strings exist in 6 languages (`cargo test -p
  cobolt-ide i18n` green); the English guide documents Web projects.

## 6. Constraints & steering check
- **i18n (6 languages):** Yes — new strings for the type chooser, Web build/
  preview/open-browser actions, portability diagnostics, and toolchain errors,
  all ×6 (R16).
- **Generated-code / regenerate contract:** The transpiled **Rust** is a new
  build artifact, analogous to the generated `.cbl`. The existing COBOL
  generation + banner + regenerate-on-Build/Run/Debug/Check contract is
  preserved; the Web build adds a **COBOL → Rust** transpile step downstream of
  the usual `.cbl` regeneration. Generated Rust is never hand-edited.
- **Docs (English guide):** Update needed — a "Web projects" section (R17).
- **Fix vs feature:** **Feature** → minor version bump + CHANGELOG entry at
  finalize.
- **product.md note:** product.md currently frames outputs as "standalone
  binaries". This feature broadens the product to **web (WASM) targets**;
  product.md should be updated (flag in /plan).
- **tech.md note:** Adds a `wasm32-unknown-unknown` build path and new build-time
  tooling (eframe-web / `wasm-bindgen` / `trunk`, auto-installed). The "no external
  COBOL toolchain at runtime" rule still holds — the new dependency is the
  *Rust/wasm* toolchain at **build** time only.
- **Risk / phasing:** The **COBOL → Rust transpiler is the central, highest-risk
  piece** and is large (it targets *feature parity* with the interpreter — R4).
  Per Q7, **Phase 1 is deliberately broad**: project-type model + auto-installed
  toolchain + portability validator + the **REST/HTTP client integration** + a
  **representative verb subset**, transpiled and running in the browser end-to-end
  — not merely a trivial form. /plan must still **phase the transpiler** (the
  full-parity goal cannot land in one phase) and likely give it its **own
  sub-spec**; later phases close the remaining verbs toward parity.

## 7. Resolved decisions
The /clarify pass resolved the open questions:
- **Transpiler subset (Q1):** **Full interpreter parity** for every *portable*
  construct; non-portable constructs (disk file I/O, disk-backed indexed, native
  threads/OS services) are **rejected** by the validator, not transpiled (R4, R6,
  R10).
- **REST data model (Q2):** **Explicit REST/HTTP client object** — the developer
  `INVOKE`s GET/POST/… and marshals records by hand. **No** automatic
  `FD`/`SELECT` → REST mapping in v1 (R12).
- **Toolchain (Q3):** **Auto-install** the `wasm32-unknown-unknown` target and
  **`trunk`** on first Web build (R7).
- **Type immutability (Q4):** **Fixed at creation**; no conversion in v1 (R2).
- **Preview server (Q5):** **`trunk serve`** on `127.0.0.1:PORT`, opened in the
  default browser, rebuilding on save (R8).
- **Debug (Q6):** **No in-IDE web debugger** in v1 (browser devtools); emit source
  maps where supported (R18).
- **Phasing (Q7):** **Broad Phase 1** (see §6 Risk / phasing) — type model +
  toolchain + validator + REST client + a representative subset, running in the
  browser; the transpiler keeps advancing toward parity in later phases.

### Assumptions recorded for /plan
- The Web app is one eframe-web binary per project (all forms in the project ship
  in the single wasm), mirroring the desktop single-binary model. Confirm at /plan.
- Indexed-file data and any persistence is **session/memory only** unless the
  developer explicitly persists it via the REST client.
