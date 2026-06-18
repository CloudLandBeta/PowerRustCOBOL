# Spec — Per-control test example projects

- **Status:** draft → approved
- **Folder:** specs/004-control-test-examples/
- **Author:** Eslopes (with Anthropic Claude Codex Agent)   **Date:** 2026-06-17

## 1. Overview

There is no systematic, runnable demonstration that each toolbox control's
**events** fire and each of its **properties** can be driven from COBOL at
runtime. This feature adds an `examples/` folder at the repository root holding
**one self-contained PowerRustCOBOL project per toolbox control**. Each project
opens in the IDE and builds with `rcrun`, prints a console line for every event
the control supports, and offers one button per property that changes that
property programmatically so the result can be confirmed visually. The set
doubles as living documentation and a manual regression suite for the control
surface (events, colours, geometry, animation, data binding, and
control-specific properties).

## 2. Goals / Non-goals

- **Goals:**
  - A committed, browsable `examples/<control>/` project for **every control in
    the toolbox** (34 controls; ModalWindow is excluded — it was removed).
  - Each project proves: (a) every supported event fires (console `DISPLAY`),
    (b) fore/back colours change from code, (c) every other supported property
    changes from code via a dedicated button, (d) the project builds cleanly.
  - Each project is tailored to **that control's** actual supported events and
    properties (no irrelevant buttons, no missing properties).
  - Clear, repeatable **build + fix-errors** instructions per project.
- **Non-goals:**
  - Automated pass/fail assertions — visual/console confirmation is **by the
    operator**; this is not a headless CI suite.
  - Re-testing the IDE designer itself, or the runtime engine internals.
  - Editing generated `.cbl` by hand (handlers live in the form's handler
    source; generated code stays a build artifact).
  - A general example gallery beyond the per-control test projects.

## 3. User stories

- As the maintainer, I want one runnable project per control, so that I can
  quickly confirm that every event fires and every property is settable from
  COBOL after a change to the form/codegen/runtime.
- As a new user, I want a minimal, correct example per control, so that I can
  see how to wire events and set properties from COBOL.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall provide an `examples/` directory at the
  repository root containing exactly one subdirectory per toolbox control, each
  a valid PowerRustCOBOL project (`cobolt.toml` + at least one `.cfrm` form +
  COBOL handler source), named after the control (kebab-case, e.g.
  `examples/label/`, `examples/date-time-picker/`).
- **R2 (ubiquitous):** For each control, the project shall instantiate that one
  control as the subject under test on its form.
- **R3 (event):** When the operator triggers any event in the control's
  **supported-events set** (the authoritative list is
  `ControlType::supported_events` in `crates/cobolt-forms/src/model.rs`), the
  project shall `DISPLAY` to the console a line of the form
  `"<EventName> working"` (e.g. `MouseEnter working`, `Click working`,
  `Tick working`) — one distinct line per supported event.
- **R4 (state):** Where the control exposes a foreground and/or background
  colour, the project shall provide a control (button) that, when clicked,
  programmatically changes that colour at least once at runtime; the visible
  change is confirmed by the operator.
- **R5 (event):** When the operator clicks the button associated with a given
  property, the project shall programmatically set/change **that one property**
  at runtime via COBOL (`INVOKE`/property set), so its effect is individually
  observable — one button per remaining supported property (geometry,
  animation, data binding, and the control-specific properties shown in the
  control's properties inspector / `default_props`).
- **R6 (ubiquitous):** Each property/event covered shall be derived from **that
  control's** actual supported set; the project shall not reference properties
  or events the control does not support, and shall cover all that it does.
- **R7 (ubiquitous):** Each project shall include written instructions to build
  it (IDE Build and `rcrun build <path>/cobolt.toml`) and a short "fix any build
  errors" loop; every project shall build with no errors before the feature is
  considered done.
- **R8 (optional):** Where a control depends on an external service (RestClient
  → HTTP endpoint, SqlDatabase → database, AgentObject → LLM), the project shall
  still cover all supported events/properties, assuming the required local
  service is available; each such project shall document the service it expects.
- **R9 (constraint):** All COBOL identifiers and source text, console `DISPLAY`
  strings, and example button captions shall be in **English**; no project shall
  use the internal "cobolt" prefix in user-facing text.
- **R10 (constraint):** Projects shall rely on the regenerate-on-Build/Run/Debug/
  Check contract; the COBOL event/property logic shall live in the form's
  handler source (the source of truth), never in hand-edited generated `.cbl`.

## 5. Acceptance criteria

- [ ] AC1 — `examples/` exists at the repo root with exactly 34 control
  subdirectories (full toolbox minus ModalWindow); each contains a `cobolt.toml`
  and at least one `.cfrm`.
- [ ] AC2 — For a sampled control (e.g. Label), every event in its
  `supported_events` produces exactly one matching `"<Event> working"` console
  line when triggered, and no unsupported event is referenced.
- [ ] AC3 — For each colour-bearing control, clicking the colour button visibly
  changes the fore/back colour at runtime.
- [ ] AC4 — For each control, there is exactly one button per remaining
  supported property, and clicking it changes that property at runtime in an
  observable way (visual for visible controls; console/echo for non-visual).
- [ ] AC5 — Every project builds with `rcrun build` (and opens/builds in the
  IDE) with zero errors.
- [ ] AC6 — Each project includes its own build + fix-errors instructions (e.g.
  a per-project `README.md`), and a top-level `examples/README.md` indexes them.
- [ ] AC7 — Spot-check confirms English-only identifiers/strings and no
  hand-edited generated code (handlers in form source).

## 6. Constraints & steering check

- **i18n (6 languages):** No impact. These are sample COBOL applications, not
  IDE UI; their button captions and `DISPLAY` text are English app content, not
  `Tr` strings. No new `i18n.rs` fields.
- **Generated-code / regenerate contract:** Honoured — example COBOL lives in
  the form handler source and is regenerated on Build/Run/Debug/Check; generated
  `.cbl` remains a build artifact (R10).
- **Docs (English guide):** Add a short subsection to
  `docs/developers-guide-en.md` pointing to `examples/` and how to run a
  per-control example (English only). Translations untouched.
- **Fix vs feature:** **Feature** — new committed artifacts (and possibly small
  scaffolding). Bump the minor (`y`) in `crates/cobolt-ide/src/version.rs` + a
  `CHANGELOG.md` entry. Commit separately from any fix.

## 7. Open questions

- Q1: Is `examples/` shipped in `rcrun package`/release artifacts, or repo-only?
  (Assumption: repo-only, not bundled into built binaries.)
- Q2: Per-project layout — single form with the subject control plus a button
  column, vs. a form per property group. (Assumption: single form per project;
  defer exact layout to `/plan`.)
- Q3: For controls with no colour and no mouse events (e.g. Line, Shape) or
  non-visual controls (Timer, RestClient, SqlDatabase, AgentObject), confirm
  that R4 is simply N/A and confirmation for R5 is console-based where there is
  nothing visual to observe.
- Q4: Should the example set be wired into any existing test harness
  (`tests/controls/`), or stay manual-only under `examples/`? (Assumption:
  manual-only, separate from `tests/controls/`.)
