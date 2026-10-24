<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Web and mobile project types, the New Project selector and the IDE entries

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-001-project-types-and-ide-entries/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator items **1** (a new RAD for Web and mobile, with a project-type selector shown in the attached
  mock-up) and **10** (the IDE gets an entry for WebForms, one for MobileForms and one for Backend; the IDE stays the
  same, with new capabilities). The two overlap on "what kind of project is this and how does the IDE show it", so they
  are one spec.
- **Depends on:** nothing. **Needed by:** every other WASM spec (they all key off the project kind).

## 1. Overview

Today the IDE builds one kind of project, a desktop application. Version 2.x.x adds three more things a developer can
build: a **Web** application (WebAssembly) and a **Mobile** application (Android, iOS), each with a **Backend** — a part of the project, not a kind of its own — that it talks to.
This spec introduces the **project kind**, a **New Project selector** modelled on the operator's mock-up, and the new
**project-tree entries** WebForms, MobileForms and Backend. It does not say how a Web project is built (WASM-002),
how the backend works (WASM-003) or which controls are available (WASM-004); it makes them reachable.

The IDE stays one application. There is no second "web IDE": the same menus, panels, Form Designer and Properties
pane serve every kind, and what changes is which templates, tree entries, toolbox controls and build actions are offered.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **New Project is minimal.** `NewProjectDialog` (`crates/cobolt-ide/src/app.rs:204`) collects only *name*, *version* and
  *main file* (default `src/main.cbl`); it is drawn by `show_new_project_dialog` (`app.rs:14057-14095`). *Create…* opens a
  Save dialog for the manifest and `create_new_project_at` (`app.rs:4596-4679`) creates `PROJECT_FOLDERS` and a starter
  `src/main.cbl` (`crates/cobolt-project-tools/src/create.rs:25-38`, `:84`). There is no theme, language or target field.
- **There is no project kind.** A search for `ProjectKind`, `project_type` and `ProjectTemplate` finds nothing;
  `ProjectMeta` (`project_model.rs:640-680`) holds name, structure, version, main, copyright, license, destination folder
  and the version that built it. There is no template list anywhere. `add_powerchat` (`cobolt-project-tools/src/tools/mod.rs:345`)
  adds the chatbot sample to an existing project; it is an add-on, not a type. `UserControlDef` (`project_model.rs:331`)
  is a list inside the manifest, not a type.
- **The manifest shape is declared three times:** the IDE's `CoboltProject` (`project_model.rs:39`), a private serde subset in the
  compiler (`cobolt-compiler/src/lib.rs:793`, `:986`) and another in the CLI (`cobolt-cli/src/main.rs:671`, `:686`).
  Shape changes are versioned by `project_upgrade.rs` (`CURRENT_STRUCTURE = 1`, `:51`); a test requires the headless template
  `new_project.toml` to equal the IDE's output byte for byte (`project_model.rs:2586`).
- **The project tree has 8 top entries** — `Category::TOP` (`project_model.rs:1147`): Forms, IndexedFiles, SqlConnections,
  CommonCode, Generated, ExternalCrates, Assets, Documentation. A new top entry touches `Category` (`:1124`), `TOP`,
  `root_subdir` (`:1168`), `is_addable` (`:1161`), `of_kind` (`:1190`), `from_root_component` (`:1201`), `add_file_to` (`:873`),
  `list_mut` (`:901`), `files_in` (`:1048`), the `ProjectFiles` list (`:797`) and its tests (`:1764`); in the panel,
  `show_project_mode` (`panels/project.rs:622`), `show_category` (`:1021`, `:1041`, `:1067-1130`, `:1199`) and `creatable_kind`
  (`:2020`); `i18n.rs` (a `cat_*` field × 6 languages); `panels/walkthrough.rs:71-74`, `agent_kit/ide_host.rs:514-518` and the
  routing in `app.rs` (`:6918`, `:7063`, `:18696-18735`).
- **The headless path must match.** The MCP tool `create_project` takes only `folder` and `name`
  (`cobolt-project-tools/src/tools/mod.rs:367-385`) and forces the Spatial theme (`create.rs:115`, `:153-160`).
- **Prior art.** Spec 006 (*Web projects*, removed from the tree on 2026-10-14, recoverable with
  `git show ccfa3449^:specs/006-web-wasm-projects/spec.md`) already asked for a Desktop | Web picker with the type fixed at
  creation. Its picker idea survives here; its canvas rendering was replaced by the DOM (WASM-002) and its "no interpreter" choice was kept, without the Rust step (WASM-010).

### The mock-up (operator, 2026-10-19)

A "New Desktop Project" dialog with two columns. **Left**, a list: *All Templates*, then Desktop (macOS, Windows, Linux),
Agentic AI Chatbot (macOS, Windows, Linux), Agentic AI Chatbot (Web), Web (WebAssembly), UserControl (Desktop),
UserControl (Web), Android (Cellphone), Android (Tablet), Android (Smartwatch), iPhone (iOS), iPad (iOS), Apple SmartWatch
(watchOS), and *Recent Projects*. **Right**, a *Project Properties* tab: Project name, Version, Main COBOL file / Entry file
(with a folder button), Target platform (macOS | Windows | Linux), Output type (Executable Application), Project location
(with a folder button), Description, UI style / Theme (Dark (Nebula)), Default language / Locale (English (en-US)); under it a
collapsible *Template-Specific Options*: Window style (Tabbed Document), UI framework (PowerUI), Default orientation
(Landscape), Minimum OS (Auto / Latest), and the check boxes Enable High DPI support, Enable GPU acceleration, Include sample
main program. **Bottom**: *Create from Template…*, *Recent Projects ▾*, *Cancel* and *Create*. The title follows the
selected template ("New Desktop Project"). The mock-up is "suggestive": it fixes the layout and the fields, not the wording.

## 2. Goals / Non-goals

- **Goals:**
  - A **project kind** stored in the manifest and honoured by the whole IDE: `desktop` (today's behaviour), `web` and `mobile`.
    The Backend is **not** a kind: it is the `backend/` part of every Web or Mobile project (operator, 2026-10-20).
  - A New Project dialog with the template list and the per-template options of the mock-up, driven by data so a template
    can be added without changing the dialog's code.
  - The tree entries **WebForms**, **MobileForms** and **Backend**, shown according to the project kind.
  - One IDE: no change for a developer who only builds desktop applications.
  - The same result from the IDE, from `rcrun` and from the MCP `create_project` tool for the same inputs.
- **Non-goals:**
  - Building or running a Web or Mobile project (WASM-002, WASM-008), the backend itself (WASM-003), the control catalogue
    (WASM-004).
  - Watch targets (Android Smartwatch, Apple SmartWatch) and the two template entries whose features do not exist yet
    (Agentic AI Chatbot (Web), UserControl (Web)). They appear in the catalogue **as entries that cannot be created yet**
    (R5); see Q3.
  - Changing a project's kind after creation (see Q7).
  - A **backend-only project kind** and a New Project template for one (operator, 2026-10-20: the Backend lives inside Web and Mobile projects; a developer who only wants a backend creates a Web or Mobile project and uses its Backend entry).
  - A template marketplace or sharing of user templates.

## 3. User stories

- As a developer who knows desktop RustCOBOL, I want to pick **Web (WebAssembly)** when I create a project, so that I get a
  project laid out for the browser without learning a new tool.
- As a mobile developer, I want to pick **Android** or **iPhone** and set the orientation and minimum OS version once, so that
  the project starts with the right settings.
- As a developer, I want a **Backend** entry next to my forms, so that I can see and edit the programs that serve my
  frontend.
- As a developer of desktop applications, I want nothing to change for my existing projects.
- As an agent working through the MCP tools, I want `create_project` to create the same project the dialog would.

## 4. Requirements (EARS)

**The selector**
- **R1 (event):** When the developer chooses *New Project*, the IDE shall open a dialog with a template list on the left and the
  selected template's *Project Properties* and *Template-Specific Options* on the right, laid out as in the mock-up; choosing a
  template shall change the dialog's title and the options shown.
- **R2 (ubiquitous):** The template list shall be **data**: each template is a descriptor (id, display name, platforms, project
  kind, the options it asks for and their defaults, whether it can be created) held outside the dialog's drawing code, so that
  adding a template does not change that code.
- **R3 (ubiquitous):** The dialog shall collect, for every template: project name, version, main/entry file, project location,
  description, UI style / theme and default language / locale; the values shall be written to the manifest and to the settings the
  rest of the IDE already reads (the `[ide]` and `[forms]` theme, the default form language).
- **R4 (ubiquitous):** Each template shall add its own options (examples from the mock-up: window style and UI framework for
  Desktop; default orientation and minimum OS version for Mobile; High DPI and GPU acceleration for Desktop only; and for Web and Mobile the
  **application style** — **SPA** (the default) or **standalone forms**, WASM-011 R1). An option that does not apply to a template shall not be shown for it.
- **R5 (state):** While a template's features are not implemented, its entry shall be visible, marked as unavailable, and
  *Create* shall be disabled for it. In 2.0 the creatable templates are **Desktop**, **Web (WebAssembly)**, **Android
  (Cellphone)**, **Android (Tablet)**, **iPhone** and **iPad**; the rest are unavailable (Q3).
- **R6 (event):** When the developer presses *Create*, the IDE shall create the folders and starter files for that kind and open the
  project; for **Web** and **Mobile** it shall also create the project's backend scaffold (WASM-003 R22) unless the developer
  cleared that option (Q5).
- **R7 (ubiquitous):** The New Project footer shall offer *Recent Projects* and *Create from Template…*. *Create from Template…* shall copy a saved project into a new project in the chosen location; it is not a second list of the left column (Q6). *Recent Projects* shall list the projects opened recently (Q6). Neither is needed to create a project from a template.
  and shall not be needed to create a project.

**The manifest and the other readers**
- **R8 (ubiquitous):** The project kind (and, for Mobile, the platform and device class) shall be stored in the manifest and
  reloaded with the project. A manifest without a kind shall load as **desktop**.
- **R9 (ubiquitous):** The IDE, the compiler's manifest reader and the CLI's manifest reader shall agree on the new fields;
  `project_upgrade` shall carry old manifests forward without losing a field.
- **R10 (ubiquitous):** The MCP `create_project` tool shall accept a project kind (and its options), and the IDE and the tool
  shall produce identical files for the same inputs; the existing byte-identity test shall cover the new kinds.

**The IDE entries**
- **R11 (state):** While a project of kind **desktop** is open, the tree shall show exactly the entries it shows today.
- **R12 (state):** While a **web** project is open, the tree shall show **WebForms** and **Backend** in place of Forms; while a
  **mobile** project is open, **MobileForms** and **Backend**; the other entries (Indexed Files, SQL Connections, Common Code,
  Generated, External Crates, Assets, Documentation) stay (Q1).
- **R13 (ubiquitous):** Each new entry shall behave like Forms: a `[+]` that creates an item of its kind, a folder on disk, an
  icon, a label translated in all six languages, and a place in the Grace tree-disambiguation, the walkthrough and the agent
  kit's project listing.
- **R14 (state):** While a web or mobile project is open, the toolbox shall offer only the controls available for that kind
  (WASM-004 R5), and the toolbar's Run, Build and Preview actions shall do what WASM-002 and WASM-008 define for that kind.
- **R15 (ubiquitous):** The IDE shall remain a single application: no new executable, window mode or licence is introduced by this spec.

**Cross-cutting**
- **R16 (constraint):** Every string the dialog and the new entries show (template names and descriptions, option labels,
  unavailable-template notes) shall be a `Tr` field with all six languages.
- **R17 (constraint):** The dialog shall not resize itself: it owns an explicit size and a single grip is the only writer of it.
- **R18 (constraint):** A project already on disk shall open exactly as before; no existing file is rewritten merely by opening it.

## 5. Acceptance criteria

- [ ] **AC1** — The dialog shows the left list and right pane of the mock-up; selecting each template changes the title and the
  options; the entries of R5 that are unavailable cannot be created.
- [ ] **AC2** — Creating one project of each creatable kind writes the kind (and Mobile platform/device) to the manifest; closing
  and reopening gives the same kind; an old manifest loads as desktop and is not modified by loading.
- [ ] **AC3** — For the same inputs, the IDE's *Create*, `rcrun`'s creation path and the MCP `create_project` tool produce
  byte-identical manifests and file trees (extend `project_model.rs`'s byte-identity test).
- [ ] **AC4** — Web: the tree shows WebForms and Backend and no Forms; Mobile: MobileForms and Backend; Desktop: unchanged.
- [ ] **AC5** — A new template can be added by adding a descriptor only (demonstrated by a test that registers a fake template and
  sees it in the list with its options).
- [ ] **AC6** — `cargo test -p cobolt-ide i18n` is green and the dialog's strings exist in six languages.
- [ ] **AC7** — The existing desktop test suites are unaffected (no new failure in `cargo test -p cobolt-ide --bin cobolt-ide` and
  `-p cobolt-project-tools`).

## 6. Constraints & steering check

- **The frontend↔backend channel (WASM-INDEX D21, operator 2026-10-24):** whatever this spec puts on the channel between the frontend and the backend obeys the series-wide ruling — the channel is asymmetrically encrypted with a key pair generated when the application starts; a broken connection is retried with a timeout of 10 s, raised by 10 s on each retry, for at most 3 cycles, then an exception is raised; while the frontend retries, all mouse and keyboard input is blocked and the "connecting" modal of WASM-016 R20 (WASM-012 R15 on a device) is shown, appearing when the connection breaks and disappearing when it is re-established. The ruling is owned by WASM-003 R24–R26, WASM-016 R20–R21, WASM-012 R15, WASM-005 R18, WASM-014 R18 and WASM-013 R17; this spec adds nothing of its own to it and must not contradict it.
- **i18n (6 languages):** yes — R16.
- **Generated-code contract:** unchanged here; each kind's generated output is defined in WASM-002/003.
- **Docs:** the English Developer's Guide is **not** touched on the 2.x.x line (CLAUDE.md, 2026-10-19: README.md and `docs/` describe
  the official 1.x.x). The guide chapter for project kinds is planned in WASM-INDEX (proposed spec on documentation) and written
  at release.
- **System KB:** project kinds are not a control/property/method/event, so the KB tables are unaffected; the *plugin's* golden rules
  (`cobolt-project-tools/src/content.rs`) will need a sentence per kind (tracked in the documentation spec).
- **Fix vs feature:** **feature** — `features-2x`; version bump is `z` of 2.x.x.
- **Spec 017/042 note:** nothing here changes a renderer or the form host.

## 7. Open questions

- **Q1 — Which tree entries per kind? — decided (operator, 2026-10-20: the recommendation).** Recommended: Web = WebForms + Backend; Mobile = MobileForms + Backend; Desktop = Forms. Alternative:
  always show all three and grey out the ones that do not apply.
- **Q2 — File format — decided (operator, 2026-10-20).** Web forms are `.wcfrm` and mobile forms are `.mcfrm`; desktop forms stay `.cfrm`. The model is one (WASM-004 R1): only the extension differs, and the folder (`webforms/`, `mobileforms/`, Q8) keeps the kind visible. Everything that recognises a form file by its extension (the IDE tree, the loaders, the compiler, the project tools, the agent kit and the plugin rules) shall recognise the two new extensions; the `.cfrm`-only assumption is changed under T15.
- **Q3 — Unavailable entries and the set creatable in 2.0 — decided.** The operator confirmed on 2026-10-19: **Web + Android + iOS, no watch**, and **the template list of this spec as written** (R5: the entries that cannot be created yet stay visible, greyed, with a "not yet
  available" note). So the creatable templates in 2.0 are Desktop, Web (WebAssembly), Android (Cellphone), Android (Tablet), iPhone and iPad.
- **Q4 — "Target platform" for Desktop — decided (operator, 2026-10-20: the recommendation).** The compiler builds for the host triple only (spec 041 R17/R18), so macOS | Windows | Linux
  can be informational (the host) or a real choice only when cross-building exists. Recommended: show the host, disabled, until then.
- **Q5 — Backend scaffold always created? — decided (operator, 2026-10-20: the recommendation).** The operator's item 11 says every Web/Mobile project "gets" a backend. Recommended: created by
  default with an option to clear it for a project that talks to an existing backend.
- **Q6 — Recent Projects and Create from Template… — decided (operator, 2026-10-20).** *Create from Template…* copies a saved project into a new project; it is not the left column's list (R7). *Recent Projects* does not exist yet, according to the operator, and is an entry of the footer (R7). **To confirm before the footer is built:** the features-2x code already has a Recent view in the New Project dialog (`crates/cobolt-ide/src/panels/new_project.rs`), fed by the recent-projects store in `crates/cobolt-ide/src/ui_prefs.rs` through `app.rs`; the operator says whether that is this same entry.
- **Q7 — Kind immutable? — decided (operator, 2026-10-20: the recommendation).** Spec 006 fixed it at creation. Recommended: fixed in 2.0; a conversion is a later spec (WASM-INDEX, project conversion).
- **Q8 — Folder names — decided (operator, 2026-10-20: the recommendation).** on disk: `webforms/`, `mobileforms/`, `backend/`, or `forms/web/` …? Recommended: the three names above.
