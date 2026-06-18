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
- A **COBOL → portable Rust transpiler** that, for the supported subset, emits
  Rust with no interpreter dependency, compiled to the `wasm32` target.
- A **Web build** that produces a self-contained eframe-web bundle (host HTML,
  `.wasm`, JS loader/glue, form assets) that renders the form on a `<canvas>`.
- **Preview/Run = build + open browser** for Web projects; no in-IDE interpreter.
- A **portability validator** that rejects non-portable COBOL in Web projects at
  Check and Build.
- **Memory-only** indexed files and **REST-based** record I/O for Web projects.
- Desktop projects, manifests, and outputs **unchanged**.

### Non-goals (explicitly out of scope)
- The **backend** project type (the REST server). Only the Web **client** side
  (calls to REST) is in scope here.
- A *general* COBOL → Rust compiler beyond the supported portable subset.
- **HTML/DOM** rendering of forms (the UI is the egui canvas, not generated DOM).
- **Multi-threaded** wasm.
- Hosting/deployment of the produced bundle to a real server (we emit it; a
  minimal local server is only for Preview — see R12/Q5).
- A web **debugger** in the IDE (browser devtools serve that role in v1).
- Changing a project's type after creation (v1: fixed at creation — see Q4).

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
- **R1 (ubiquitous):** The system shall support a project **type** attribute with
  at least `Desktop` and `Web`, persisted in the project manifest and reloaded
  with the project.
- **R2 (event):** When the user creates a new project, the system shall let them
  choose the project type and record it; the type is fixed for the life of the
  project (v1).
- **R3 (state):** While a Desktop project is open, the system shall behave exactly
  as today (interpreted Preview/Run/Debug, native single-binary build).

**Transpile + WASM build**
- **R4 (event):** When the user builds a Web project, the system shall transpile
  each form's COBOL (the generated program + handlers + user procedures) to
  portable Rust, compile it to the `wasm32` target, and emit an eframe-web bundle
  (host HTML page, the `.wasm`, the JS loader/glue, and form assets) into the
  project's web output folder.
- **R5 (constraint):** The Web build shall **not** embed the tree-walking
  interpreter; program logic shall be **transpiled Rust**.
- **R6 (constraint):** The transpiled Rust and the bundle shall be **100 %
  portable** — no `std::fs`, no native filesystem/OS access, no OS threads — so
  they run unmodified in a browser sandbox.
- **R7 (constraint):** The Web build shall use the `wasm32` Rust target and the
  crates/tools required for it (e.g. the eframe web backend, `wasm-bindgen`, and a
  bundler such as `trunk` or `wasm-pack`). When a required toolchain component is
  missing, the system shall report a clear, actionable error rather than failing
  opaquely.

**Preview / run**
- **R8 (event):** When the user invokes Preview or Run on a Web project, the
  system shall build the bundle and open the application in the **default web
  browser**.
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
- **R12 (optional):** Where a Web form reads or writes record/file data, the data
  shall be obtained via **REST API calls** to a backend rather than local disk;
  the system shall provide the client-side REST mechanism for record load/save.
  The backend is out of scope (Non-goals); the Web project contains only the
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

## 5. Acceptance criteria
- [ ] **AC1** — Creating a project offers Desktop/Web; the chosen type is written
  to and reloaded from the manifest. An existing (typeless) manifest loads as
  Desktop.
- [ ] **AC2** — Building a Web project produces a bundle (host HTML + `.wasm` +
  JS glue + assets) with **no interpreter** embedded; loaded in a browser it
  renders the form on a `<canvas>`.
- [ ] **AC3** — Preview/Run on a Web project builds and **opens the default
  browser**; no interpreted preview window appears.
- [ ] **AC4** — Check/Build of a Web project that uses a disk file (OPEN/ASSIGN to
  a path, or a disk-backed indexed file) reports a **portability error** and emits
  no bundle.
- [ ] **AC5** — A Web form that reads records via the REST mechanism shows data
  fetched over HTTP (against a stub/mock backend) — the data path is REST, not
  disk.
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
- **tech.md note:** Adds a `wasm32` build path and new build-time tooling
  (eframe-web/wasm-bindgen/trunk-or-wasm-pack). The "no external COBOL toolchain
  at runtime" rule still holds — the runtime dependency for Web is the *Rust/wasm*
  toolchain at **build** time only.
- **Risk:** The **COBOL → Rust transpiler is the central, highest-risk piece** and
  is large. /plan should **phase** it (e.g. Phase 1: project-type model +
  toolchain + portability validator + transpile a *trivial* form end-to-end to a
  running browser canvas; later phases broaden the supported verb/feature subset
  and the REST data model). It may warrant its own sub-spec.

## 7. Open questions
- **Q1 (transpiler subset):** Which COBOL subset must transpile for v1? Proposed:
  the constructs the form/handler model already uses — DATA DIVISION items,
  MOVE/arithmetic/COMPUTE, IF/EVALUATE/PERFORM, DISPLAY-to-control, `INVOKE`/
  `obj::method()` UI calls, the Rust-FFI bridge (spec 005), and REST record I/O.
  Which verbs are explicitly **non-portable** (rejected) vs **mapped** (e.g. file
  I/O → REST)? Define the list.
- **Q2 (REST data model):** How does a COBOL `FD`/`SELECT`/record map to a REST
  resource — a convention (`ASSIGN TO "https://…"`), a new clause, or project
  config? Define the read/write/marshal contract (record ↔ JSON?).
- **Q3 (toolchain delivery):** Does the IDE **require** the user to have the
  wasm target + bundler installed (detect + guide), or **manage/auto-install**
  them? Which bundler (trunk vs wasm-pack vs hand-rolled wasm-bindgen)?
- **Q4 (type immutability):** Fixed at creation (proposed) or convertible later?
- **Q5 (preview server):** A browser cannot load wasm from `file://`. Does Preview
  start a **local static HTTP server** (e.g. on `127.0.0.1:PORT`) serving the
  bundle and open that URL? (Proposed: yes, a minimal dev server for Preview.)
- **Q6 (debug):** No in-IDE web debugger in v1 (browser devtools). Confirm.
- **Q7 (phasing):** Confirm Phase-1 slice = type model + toolchain + validator +
  a trivial form transpiled and running in the browser, before broadening the
  transpiler. (See §6 Risk.)
