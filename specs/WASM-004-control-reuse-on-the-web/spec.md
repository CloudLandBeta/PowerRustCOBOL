<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Reusing the control catalogue on the web

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-004-control-reuse-on-the-web/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **4** — reuse the current controls, with their behaviour, properties, methods and configuration interface, as far as
  possible.
- **Depends on:** WASM-001, WASM-002, WASM-010 (the module that drives the controls), WASM-016 (the player that implements them). **Amends:** spec 017 (*one renderer everywhere*): for the web kinds the DOM is the renderer. **Where the web controls live:** every class A and B control is implemented **once, in the
  prebuilt player** (WASM-016 R5) — never per project — so a new or fixed control reaches an application at its next build with the IDE's current player.
  **Needed by:** WASM-005 (the data controls), WASM-006 (WebLink), WASM-007 (CSS), WASM-008 (preview).

## 1. Overview

The control catalogue is the product's biggest asset: **62 control types**, each with properties, events, methods, a Properties-panel
section, a toolbox icon, documentation in the System KB and tests. A Web or Mobile project shall reuse them, so that a developer who knows
the desktop controls already knows these, and the agents and the KB keep working. This spec says **how much** can be reused and **how**:
which controls become ordinary DOM elements, which must be redrawn, which now work through the backend, and which cannot exist in a browser.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The catalogue.** `ControlType::ALL` has **62 variants** (`crates/cobolt-forms/src/model.rs:2926-2989`): 46 ordinary types plus **16 AWS**
  controls (`AwsMcp`, `AwsLambda`, `AwsKnowledgeBase`, `AwsAgentCore`, `AwsAgentMemory`, `AwsS3Tables`, `AwsGlue`, `AwsDynamoDB`, `AwsS3`,
  `AwsS3Vectors`, `AwsRekognition`, `AwsPolly`, `AwsComprehend`, `AwsTextract`, `AwsEC2`, `AwsCognito`). `Custom { plugin_id, control_id }` is a type
  with no painter and no runtime (`model.rs:2850`; the toolbox excludes it).
- **One renderer, painted.** The designer canvas (`render_faces`), the Preview, the Run Form window and the compiled binary all paint through
  `cobolt-forms` with egui (`paint::draw_control`, `render.rs`); `paint.rs` names 46 ordinary types. The model part of `cobolt-forms` compiles
  without egui (the runtime, codegen, parser and semantic crates depend on it without the `render` feature, `cobolt-runtime/Cargo.toml:149`), but
  carries browser risks: unconditional `lopdf`, `mermaid-rs-renderer` and `tl`; ungated `std::fs` in `viewer.rs`, `dropzone.rs`, `assets.rs`,
  `items_file.rs`; painters that read files directly (`paint.rs:7907`, `:8164`, `:8185`, `:8328`, `:9829`, `:10082`).
- **Properties.** Value types are `PropValue::{String, Int, Bool}` (`model.rs:54`); colours are `#RRGGBB[AA]` strings. Universal seeds
  (`model.rs:5240-5300`) and per-type ones (`:5347-5571`); edited in `panels/properties.rs`. Events: 22 mouse, hover, geometry and lifecycle events
  for a Label (`model.rs:3437`). The closed method vocabulary is `cobolt-ast/src/methods.rs:164` — a name not in it silently parses as a subscript.
- **Layout.** `LayoutMode` Absolute | Flex | Grid | Flow (`cobolt-forms/src/layout/mod.rs:119`) applies to Form, Panel, GroupBox and TabControl; the solver is
  pure Rust, no egui; breakpoint overrides are sparse (control, property, value) rules (`layout/breakpoints.rs:19-35`).
- **Tests.** `tests/controls/` has 35 files, one per control, each calling `assert_control` (set every property, read back, save to `.cfrm`, reload, read
  back) — 35 of the 62 types (not covered: Switch, Knob, Gauge, Maps, FileDropZone, Viewer, Snackbar, SideMenu, WebSearch, KnowledgeBase, IndexedFile, AWS).
  More than 20 test files iterate `ControlType::ALL`. The parity guard `engine_reference_form_parity_static_vs_faces` (`render.rs:22796`) compares geometry
  only, for a 5-control form.

## 2. Goals / Non-goals

- **Goals:**
  - One definition per control: the `.cfrm` model, the property, event and method names and the Properties-panel UI stay the single source for desktop and web.
  - Every control is **classified** for the web (A, B, C or D below), the classification is published, and the toolbox, Check, the KB and the agents obey it.
  - Class A and B controls behave on the web as on the desktop, with differences listed.
  - A web conformance harness so "works like the desktop" is measured.
- **Non-goals:**
  - New controls (WebLink is WASM-006; Authentication is WASM-009).
  - A redesign of any control's look.
  - Moving the Form Designer canvas to the web (it stays egui; WASM-008 shows the real result in a webview).

### The proposed classification (to confirm at `/clarify`)

| Class | Meaning | Controls |
|---|---|---|
| **A — native DOM** | Becomes ordinary HTML/CSS, behaviour from the existing model | Button, TextBox, Label, CheckBox, RadioButton, ListBox, ComboBox, GroupBox, Panel, TabControl, ProgressBar, StatusBar, Line, Shape, NumericUpDown, Slider, Switch, Splitter, TreeView, MenuBar, SideMenu, ToolBar (its visible part), DateTimePicker, PictureBox, Timer, Snackbar |
| **B — redrawn** | Drawn by the egui painter today; needs an SVG or canvas implementation with the same data properties and methods | BarChart, LineChart, PieChart, AreaChart, ScatterChart, DonutChart, RadarChart, Knob, Gauge, Animator, Maps (tiles), Viewer (PDF, HTML subset, Mermaid) |
| **C — through the backend** | Reads or writes disk, a database or the network, or holds a secret; works through WASM-005 with the same COBOL API | DataGrid (IndexedFile and SQL bindings), IndexedFile, SqlDatabase, KnowledgeBase, RestClient, AgentObject, WebSearch, FileDropZone, Maps (geocode, directions), Viewer (open and save), and the 16 AWS controls |
| **D — not available** | Needs the local machine | ToolBar actions that start a program, open a terminal or print through the OS; Viewer print/share through the OS; `Custom` plugin controls (until a later spec) |

## 3. User stories

- As a developer who knows the desktop controls, I want the same controls, with the same property, event and method names, on the web, so that I do not
  relearn them.
- As a developer, I want the toolbox to show only what works in my kind of project, so that I never place a control the build will refuse.
- As an agent, I want the System KB to say which controls exist for which kind, so that I never generate an unavailable one.
- As the maintainer, I want one test per control that proves it behaves on the web as in the model, so that a regression is caught.

## 4. Requirements (EARS)

**One definition**
- **R1 (ubiquitous):** A control on a web or mobile form shall be the same `.cfrm` control as on a desktop form: same type name, same property names and value
  types, same events and methods. The web shall not introduce a parallel property set; a property that exists on the web only (WASM-007's `CssClass`, `Style`)
  shall be added to the model like any other.
- **R2 (ubiquitous):** Each control shall have exactly one **primary** class (A–D) recorded in one place the toolbox, Check, the KB and the agent kit all read, plus, where
  some of its operations differ from its class, the list of those operations and the class they take (Maps is B for its picture and C for geocode and directions; Viewer is B for
  its pages, C for open and save and D for print and share; ToolBar is A for its look and D for the actions that start a program).

**Behaviour**
- **R3 (ubiquitous):** A class A control shall render as the native HTML element that fits it (a `button`, `input`, `select`, …), with CSS from its properties
  (WASM-007), and shall raise the same events in the same order as on the desktop.
- **R4 (ubiquitous):** A class B control shall be drawn with SVG or canvas, shall accept the same data properties and methods, and shall document, per control,
  where its picture differs from the desktop.
- **R5 (state):** While the project kind is web or mobile, the toolbox shall hide class D controls, and Check shall report one that appears on a form (for example
  in a desktop form copied over) with its line and the alternative.
- **R6 (ubiquitous):** The Properties panel, the toolbox and the event-binding editor shall be unchanged for a web control. A property that has no effect or only an
  approximate one on the web (for example `RotateX`/`RotateY` perspective, an operating-system blur) shall stay editable, so that a form round-trips between kinds,
  and shall be marked as approximate or ignored in its tooltip (Q3).
- **R7 (ubiquitous):** Every web-only member (a property, event or method a control gets only on the web) shall be added to the closed method vocabulary
  (`cobolt-ast/src/methods.rs`) and to the System KB tables, so that it never parses as a subscript and agents can find it.

**Layout and input**
- **R8 (ubiquitous):** The four layout modes and the breakpoints shall be honoured on the web by **CSS layout** (operator, 2026-10-19): Flex, Grid and Flow map to the browser's own flex, grid and flow layout, Absolute to absolute positioning, and the breakpoints to media or container queries.
  "The same results the desktop's solver gives" is measured by the parity check of R11, and the differences it finds (a pixel of rounding, the browser's own text metrics) are documented.
- **R9 (ubiquitous):** Touch input shall raise the same click events as the mouse. Events that need a hover shall be documented as not raised on a touch device. The
  full mobile interaction model is a proposed spec (WASM-INDEX).
- **R10 (ubiquitous):** A control shall be operable by keyboard and expose a correct accessible name and role: the native element's where there is one, and the
  appropriate ARIA pattern for the composite controls (TreeView, TabControl, DataGrid, SideMenu, MenuBar).

**Assurance**
- **R11 (ubiquitous):** Every class A and B control shall have a web test that sets each property to a distinct value and reads it back from what the browser
  shows (the model's counterpart of `assert_control`), and a headless-browser parity check of geometry against the desktop engine for a reference form that
  contains at least one control of each class.
- **R12 (ubiquitous):** The System KB tables shall state the class of every control and the web-only members, and the regenerated `assets/knowledge/chunked.data`
  shall carry them (the freshness test is a real gate).

## 5. Acceptance criteria

- [ ] **AC1** — A table in one place lists each of the 62 controls with its class; a test fails when `ControlType::ALL` and the table disagree.
- [ ] **AC2** — For each class A control a headless browser shows the expected element and the property round-trip test passes.
- [ ] **AC3** — For each class B control the same data (a fixed series, a fixed value) renders in the browser and on the desktop engine, and the documented differences
  are the only ones a comparison finds.
- [ ] **AC4** — A web project's toolbox lists no class D control; a form containing one makes Check fail with its line.
- [ ] **AC5** — A desktop form opened as a web form keeps every property value (including the approximate ones) after a save and reload.
- [ ] **AC6** — Click, change, focus and key events fire in the same order on the desktop and in the browser for a scripted interaction on each class A control.
- [ ] **AC7** — A keyboard-only walk through a reference form reaches and operates every control; an automated accessibility check reports no critical issue.
- [ ] **AC8** — `prebuilt_chunked_kb_matches_the_published_documentation` is green with the new KB entries.

## 6. Constraints & steering check

- **i18n:** yes for any IDE string (the toolbox note on unavailable controls, tooltips marking approximate properties).
- **Generated-code contract:** unchanged for desktop; the web output is downstream of the generated COBOL (WASM-002).
- **Docs:** `docs/` untouched on 2.x.x; planned in WASM-INDEX.
- **System KB:** **required** — R7 and R12 (compiler documentation tables, `controls_reference_doc`, regenerated store, the freshness test).
- **Fix vs feature:** **feature** (`features-2x`).
- **Touch points** (from adding a type today, for the planners): `model.rs` (enum `:2746`, `ALL` `:2926`, `as_str`, `from_str`, `default_size` `:3140`,
  `primary_event` `:3207`, `supported_events` `:3371`, `is_non_visual` `:4152`, `Control::new`, `runtime_property_names_for` `:1527`), `paint.rs:2957`, `render.rs`,
  `methods.rs:164`, the interpreter's method arms, the form host's seeding, `runtime_features.rs:245`, the compiler's KB tables (`control_structure` `:7522`,
  `control_purpose` `:7534`, `control_method_docs` `:7585`, `control_usage_notes` `:8137`, `property_reference_for`, `event_reference`), the toolbox, `properties.rs`,
  `designer.rs:17579`, `agent.rs:4850`, `prop_help_data.rs`, `ide_text_data.rs` and `i18n.rs`.

## 7. Open questions

- **Q1 — Layout engine on the web — decided:** **CSS** (operator, 2026-10-19; R8). The existing pure-Rust solver is not run in the browser. The cost accepted is a possible pixel of difference from the desktop, which R11 measures.
- **Q2 — Which class a few borderline controls take:** Animator (a browser plays animated GIF/WebP/APNG natively, but the control's API is richer), Timer, DateTimePicker (the
  native `input type=date` differs from the desktop picker), TreeView and DataGrid (large data, virtualisation). Decide per control at `/plan`.
- **Q3 — Hide or mark** a property with no web effect? Recommended: keep and mark (R6), so no data is lost.
- **Q4 — DataGrid** keeps the `Rows` string model (`Rows` with `\n` and TAB, `Columns` as `Name:Type`, `model.rs:5617-5622`) in the browser for in-memory use; a large remote table
  needs paging, sorting and filtering on the server (WASM-INDEX, data controls beyond v1).
- **Q5 — Fonts:** the desktop ships its own font set (`fonts.rs`); on the web, bundle web fonts (licences, size) or use the system stack? Recommended: system stack by default, project fonts
  through `@font-face` from `Assets/`.
- **Q6 — Is the egui-elegance look** (Knob, glass shadows, Spatial's operating-system blur, spec 057's drawn corners) reproduced in CSS or accepted as approximate? WASM-007 maps the properties;
  the visual bar is the operator's call.
