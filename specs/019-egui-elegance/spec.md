# Spec — egui-elegance theme & widget integration

- **Status:** draft
- **Folder:** specs/019-egui-elegance/
- **Author:** Claude (spec-driven)   **Date:** 2026-06-27

## 1. Overview

Integrate the **egui-elegance** library as a theme option for the IDE and form
runtime. Elegance provides 4 built-in themes (Slate dark, Frost light, Charcoal
dark, Paper light), 40+ polished widgets (buttons, inputs, selects, sliders,
modals, drawers, tabs, cards, etc.), and a glyph font. The integration adds
elegance themes to the existing theme catalogue and replaces selected IDE/form
widgets with elegance equivalents for a more polished look.

**Critical constraint:** egui-elegance 0.10 requires **egui 0.34**;
PowerRustCOBOL is on **egui/eframe 0.29**. This spec includes the egui upgrade
as a prerequisite phase.

## 2. Goals / Non-goals

### Goals

- Upgrade egui/eframe from 0.29 to 0.34 across the workspace.
- Add egui-elegance as a dependency.
- Expose the 4 elegance themes (Slate, Frost, Charcoal, Paper) in the IDE
  settings and per-form theme chooser alongside Liquid Glass.
- Replace key IDE chrome widgets with elegance equivalents (buttons, text
  inputs, selects, tabs, modals, cards, sliders, checkboxes, switches).
- Replace key form-runtime widgets with elegance equivalents where the control
  type maps directly (Button, TextBox, ComboBox, CheckBox, Slider, ProgressBar,
  TabControl).
- Use elegance's MenuBar widget for the runtime MenuBar rendering when an
  elegance theme is active.
- Keep Liquid Glass (Classic and Enhanced) as the default — elegance themes
  are opt-in.

### Non-goals

- Replacing ALL egui usage with elegance — only mapped widgets.
- Porting elegance widgets back to egui 0.29 (we upgrade instead).
- Modifying elegance source (use it as a crate dependency).
- Replacing the form designer canvas rendering (that stays procedural glass/
  theme-pack based).
- Removing existing theme-pack support (spec 007).

## 3. User stories

- As a **form designer**, I want to select Slate/Frost/Charcoal/Paper as my
  form's theme so that my application has a modern, polished look without
  designing custom skins.
- As an **IDE user**, I want the IDE itself to use elegance widgets so the
  development experience feels premium.
- As a **COBOL developer**, I want my form's Buttons, TextBoxes, ComboBoxes,
  and other controls to render with elegance styling when an elegance theme is
  active, with no code changes.

## 4. Requirements (EARS)

### Phase 1: egui upgrade

- **R1 (ubiquitous):** The workspace shall upgrade `egui` and `eframe` from
  0.29 to 0.34, updating all API call sites that changed between versions.
- **R2 (ubiquitous):** All existing tests shall pass after the upgrade.
- **R3 (ubiquitous):** The IDE shall launch and all existing functionality
  (form designer, code editor, preview, run, build) shall work unchanged.

### Phase 2: elegance integration

- **R4 (ubiquitous):** The workspace shall add `egui-elegance` 0.10 as a
  dependency of `cobolt-ide`.
- **R5 (ubiquitous):** The IDE settings shall offer a "UI Theme" chooser with
  options: Default (egui dark), Slate, Frost, Charcoal, Paper. Selecting one
  calls `Theme::<name>().install(ctx)` each frame.
- **R6 (ubiquitous):** The per-form theme chooser (spec 007) shall include the
  4 elegance themes alongside Liquid Glass and any asset packs.
- **R7 (event):** When an elegance theme is active on a form, the runtime shall
  render the following controls using elegance widgets:
  - Button → `elegance::Button`
  - TextBox → `elegance::TextInput` / `elegance::TextArea`
  - ComboBox → `elegance::Select`
  - CheckBox → `elegance::Checkbox`
  - RadioButton → `elegance::SegmentedButton` (grouped)
  - Slider → `elegance::Slider`
  - ProgressBar → `elegance::ProgressBar`
  - TabControl → `elegance::TabBar`
  - MenuBar → `elegance::MenuBar` + `elegance::Menu`
- **R8 (state):** While a Liquid Glass theme is active (or no theme), the
  existing procedural glass rendering shall be used unchanged.
- **R9 (ubiquitous):** The IDE chrome (toolbar, properties panel, modals) shall
  use elegance widgets: `Card` for section cards, `TextInput` for property
  fields, `Select` for combo properties, `Button` for actions, `Switch` for
  boolean properties, `Modal` for editor dialogs.

### Phase 3: elegance extras

- **R10 (optional):** Where the form uses a `Modal` action (e.g. from a
  button), the runtime shall use `elegance::Modal` for the dialog.
- **R11 (optional):** The IDE shall use `elegance::Toast` for transient status
  messages (build success, save confirmation).
- **R12 (optional):** The menu editor modal shall use `elegance::Modal` and
  elegance form widgets internally.

## 5. Acceptance criteria

- [ ] AC1 — `cargo build -p cobolt-ide` succeeds with egui 0.34 + elegance.
- [ ] AC2 — All existing `cargo test` pass.
- [ ] AC3 — IDE launches with Default theme; switching to Slate/Frost/Charcoal/
      Paper changes the IDE look immediately.
- [ ] AC4 — A form with theme="Slate" renders Buttons, TextBoxes, ComboBoxes
      with elegance styling at runtime.
- [ ] AC5 — A form with theme="Liquid Glass" (or no theme) renders with the
      existing glass look, unchanged.
- [ ] AC6 — The elegance glyph font loads and symbols render correctly.
- [ ] AC7 — The IDE properties panel uses elegance Card, TextInput, Select
      widgets.

## 6. Constraints & steering check

- **i18n (6 languages):** New theme names ("Slate", "Frost", "Charcoal",
  "Paper") added to `Tr` in all 6 languages. Elegance widget labels use the
  existing `Tr` fields — no new strings needed for widgets themselves.
- **Generated-code / regenerate contract:** No impact — elegance is a runtime
  rendering choice, not a code-generation change.
- **Docs (English guide):** Add a "Themes" section listing the 4 elegance
  options with screenshots.
- **Fix vs feature:** This is a **feature** (new theme system). However, under
  the pre-prod override it may be treated as a fix (z bump). Confirm with
  operator.

## 7. Open questions

- **Q1:** The egui 0.29 → 0.34 upgrade is a major breaking change spanning
  5 minor versions. Estimated effort: 2–4 hours of API migration across ~15
  files. Should we do it incrementally (0.29→0.30→...→0.34) or jump directly?
  **Recommendation:** Jump directly — intermediate versions aren't needed.
- **Q2:** egui-elegance's MSRV is Rust 1.92. The workspace currently requires
  1.75. Should we bump the MSRV? **Recommendation:** Yes, bump to 1.92.
- **Q3:** Should the IDE default theme change from "Default (egui dark)" to
  "Slate"? **Recommendation:** Keep Default for now; let users opt in.
- **Q4:** egui-elegance's `MenuBar` widget overlaps with our custom pulldown
  menu system (spec 018). When an elegance theme is active, which menu renders?
  **Recommendation:** Use elegance's MenuBar widget, mapping our
  `MenuDefinition` data to elegance's `Menu`/`MenuItem` API.
