# Tasks — Per-control test example projects

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-17

Ordered, small, independently-verifiable tasks. Each names files, the
requirement(s) it satisfies, and how to verify. Build each project before moving
on so the set stays green. **Do not commit/push until the operator asks.**

- [x] **T1 — Scaffold `examples/` + metadata enumerator + conventions** (R1, R6)
  - Files: `examples/` (new), `examples/README.md` (skeleton index),
    a throwaway enumerator (e.g. `cargo run -p cobolt-forms --example list_controls`
    or a `#[test]` that prints) — **not** shipped product code.
  - Do: Create the `examples/` dir. Write the enumerator that prints, per
    toolbox control, its `supported_events` and `default_props` keys (source of
    truth). Document the per-project skeleton in `examples/README.md`:
    `cobolt.toml` + `forms/<control>.cfrm` + `src/main.cbl` + `README.md`,
    single form = subject control + button column, naming = kebab-case folder.
  - Verify: enumerator lists all **34** controls (ModalWindow absent) with
    non-empty event/property sets; `examples/` and skeleton README exist.

- [x] **T2 — Reference project: Label (end-to-end pattern)** (R2, R3, R4, R5, R7, R9, R10)
  - Files: `examples/label/**` (`cobolt.toml`, `forms/label.cfrm`,
    `src/main.cbl`, `README.md`).
  - Do: Author in the IDE designer: one Label + a button per supported event is
    **not** needed (events fire on the Label itself) — attach an `EventBinding`
    per `supported_events` with body `DISPLAY "<Event> working".`; add Set
    ForegroundColor / Set BackgroundColor buttons; add one button per remaining
    property (Text/Caption, geometry X/Y/W/H, Font, Visible, Enabled, TabOrder,
    animation, data binding, …) using `INVOKE Label-1 "SetProperty" USING
    "<Name>" "<value>"`. Per-project README with Build + fix-errors steps.
  - Verify: `rcrun build examples/label/cobolt.toml` → zero errors (AC5).
    Manual: Run; hover/click → console shows each `"<Event> working"` once
    (AC2); colour buttons change colour (AC3); each property button changes its
    property or `DISPLAY`s the new value (AC4). English-only, handlers in form
    source (AC7).

- [x] **T3 — Common controls** (R2–R7, R9, R10)
  - Files: `examples/{button,text-box,check-box,radio-button,combo-box,list-box,numeric-up-down,date-time-picker}/**`.
  - Do: Same pattern as T2, tailored to each control's events/properties (from
    T1 enumerator). ComboBox/ListBox ship inline sample items; NumericUpDown
    min/max/step; DateTimePicker format/value.
  - Verify: `rcrun build` each → zero errors; spot-check one (e.g. ComboBox)
    manually for events + property buttons.

- [x] **T4 — Container controls** (R2–R7, R9, R10)
  - Files: `examples/{group-box,panel,tab-control,splitter}/**`.
  - Do: Pattern per control; TabControl ships ≥2 tabs; Splitter two panes.
  - Verify: `rcrun build` each → zero errors.

- [x] **T5 — Data controls** (R2–R7, R9, R10)
  - Files: `examples/{data-grid,tree-view}/**`.
  - Do: Ship minimal inline rows/nodes so property changes are observable;
    cover cell/row/node events.
  - Verify: `rcrun build` each → zero errors.

- [x] **T6 — Graphics / media controls** (R2–R7, R9, R10)
  - Files: `examples/{picture-box,animator,progress-bar,slider,line,shape}/**`,
    plus tiny sample image/animation assets under each project's `assets/`.
  - Do: Pattern per control. Line/Shape have no fore/back colour and no mouse
    events → R4 N/A; property buttons `DISPLAY` the new value (spec Q3).
  - Verify: `rcrun build` each → zero errors.

- [x] **T7 — Menu & bar controls** (R2–R7, R9, R10)
  - Files: `examples/{menu-bar,tool-bar,status-bar}/**`.
  - Do: Pattern per control; minimal items/panels.
  - Verify: `rcrun build` each → zero errors.

- [x] **T8 — Non-visual / service controls** (R2–R8, R9, R10)
  - Files: `examples/{timer,agent-object,rest-client,sql-database}/**`.
  - Do: Trigger buttons fire domain events (Start→onTick, Send→onResponse…,
    Query→onQueryComplete, Ask→onResponse); property buttons via SetProperty;
    each README names the expected local service (REST endpoint / DB connection
    string / LLM host+model) and placeholder config (R8). Non-visual effects
    `DISPLAY` their value (R-d).
  - Verify: `rcrun build` each → zero errors (runtime confirmation noted as
    service-dependent in the README).

- [x] **T9 — Chart controls** (R2–R7, R9, R10)
  - Files: `examples/{bar-chart,line-chart,pie-chart,area-chart,scatter-chart,donut-chart}/**`.
  - Do: Ship a minimal inline data series per chart; cover onDataChanged /
    onSeriesClick etc.; property buttons for series/appearance.
  - Verify: `rcrun build` each → zero errors.

- [x] **T10 — Build-all loop + structural coverage guard** (R1–R6; AC1, AC2, AC4, AC5)
  - Files: `examples/build-all.sh` (or doc'd loop); optional
    `tests/examples-coverage/**`.
  - Do: A loop running `rcrun build examples/<c>/cobolt.toml` for all 34,
    reporting pass/fail per project. Optional Rust test: load each `.cfrm` via
    `cobolt-forms`, **report** event-binding count vs `supported_events` and
    property-button count vs `default_props`, failing on any gap.
  - Verify: build-all reports **34/34 pass** (AC5); coverage test (if built)
    reports zero gaps (AC2/AC4) and lists actual counts (verify-first).

- [x] **T11 — Docs & registry** (AC6)
  - Files: `examples/README.md` (complete index table), `docs/developers-guide-en.md`
    (new "Per-control examples" subsection), `specs/steering/docs.md` (registry
    row → `examples/**`). **i18n: none** (sample apps, not IDE UI — no `Tr` keys).
  - Verify: index lists all 34 with folder + demo + service-required column;
    `docs/` builds/renders; translations untouched.

- [ ] **T12 — Finalize** (AC1, AC5, AC7)
  - Files: `crates/cobolt-ide/src/version.rs` (minor `y` bump), `CHANGELOG.md`.
  - Do: Bump version (feature) + CHANGELOG entry; confirm all 34 dirs present
    and English-only; no hand-edited `generated/*.cbl`.
  - Verify: `cargo build --workspace` + `cargo test --workspace` green;
    `examples/build-all.sh` → 34/34; manual launch: open ≥1 project per category
    in the IDE and run through events + property buttons per plan §6. Commit as a
    **feature**, separate from any fix (only when the operator asks).

- [ ] **T13 — Publish forum announcement (f=98, Spanish)**
  - Channel: cobolforo **f=98 = tests** subforum. Spanish, vBulletin BBCode,
    present tense, title ≤50 chars, signed **"Anthropic Code Agent"**. Post via
    native browser submit (windows-1252; set a prefix if f=98 requires one — see
    the cobolforo subforums/prefixes memory).
  - Do: After T12 lands on `main` (and is pushed, per the operator's window),
    publish a news post announcing the new per-control test example projects
    under `examples/` — confirm the exact Spanish text with the operator first.
  - Verify: thread created in f=98 with accents intact; operator confirms text.

## Done criteria
All acceptance criteria in `spec.md` (AC1–AC7) are checked, `cargo build/test
--workspace` pass, all 34 `examples/` projects build via `rcrun`, docs updated
(English guide + steering registry), and the change is a single **feature**
commit per the operator's rules (do **not** commit/push unless asked).
