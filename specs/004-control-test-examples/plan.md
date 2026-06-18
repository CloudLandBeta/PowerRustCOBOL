# Plan — Per-control test example projects

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-17

## 1. Approach

Author **34 self-contained, committed PowerRustCOBOL projects** under
`examples/<control>/`, one per toolbox control (full toolbox minus ModalWindow).
Each is a normal project — `cobolt.toml` + `forms/<control>.cfrm` +
`src/main.cbl` + per-project `README.md` — created with the **IDE form designer**
(the real tool, so the `.cfrm` is always valid) and committed as a static
fixture. No product-side generator is added (the user chose hand-authored).

Per project (satisfying R1–R10):

- **Subject control (R2):** drop exactly one instance of the control on the form.
- **Events (R3):** for every event in the control's `supported_events`
  (`crates/cobolt-forms/src/model.rs`), attach an `EventBinding` whose handler
  body is `DISPLAY "<Event> working".` For controls whose events aren't raised
  by pointer interaction (Timer `onTick`, RestClient `onResponseReceived`,
  SqlDatabase `onQueryComplete`, AgentObject `onResponse`, …) add a small
  trigger button (e.g. *Start*, *Send*, *Query*, *Ask*) that fires the domain
  action so the event runs.
- **Colours (R4):** a *Set ForegroundColor* and a *Set BackgroundColor* button
  (only for controls that expose them) whose `onClick` changes the colour via
  `INVOKE <Subject> "SetProperty" USING "ForegroundColor" "#0066CC"` (and
  BackgroundColor). One change each is enough.
- **Every other property (R5):** one button per remaining supported property —
  caption = the property name — whose handler sets it from COBOL, using the
  generic `INVOKE <Subject> "SetProperty" USING "<Name>" "<value>"` (or the
  `<Subject>::<Name> = <value>` reference form, or a type-specific method where
  the value is structured). The property list per control is the union of
  `ControlType::default_props` keys and the control's section in the properties
  inspector (`crates/cobolt-ide/src/panels/properties.rs`).
- **Supporting data:** where a property needs data to be observable (DataGrid
  rows, chart series, ListBox/ComboBox items, PictureBox/Animator image), the
  project ships minimal sample data in `src/` or WORKING-STORAGE and a small
  asset under the project's `assets/`.
- **Build + fix loop (R7):** each `README.md` gives IDE Build and
  `rcrun build examples/<control>/cobolt.toml`, plus "read the error, fix the
  handler/form, rebuild" guidance. Every project must build clean.

A throwaway **enumerator** (a dev-only `cargo run`/test that prints, per control,
its `supported_events` and `default_props`) drives a completeness checklist so
each form covers exactly the right events/properties — verify-first, not a
shipped feature, not committed as product code.

## 2. Affected crates / files

- `examples/` *(new, repo root)* — 34 project dirs (`button/`, `label/`,
  `text-box/`, …, `donut-chart/`), each `cobolt.toml` + `forms/*.cfrm` +
  `src/main.cbl` + `README.md` + optional `assets/`.
- `examples/README.md` *(new)* — index table: control → folder → what it
  demonstrates → services required (if any).
- `docs/developers-guide-en.md` — new short "Per-control examples" subsection
  (English only; translations untouched).
- `specs/steering/docs.md` — add a registry row mapping the examples to
  `examples/**` so `/docsync` sees them.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor (`y`) bump (feature).
- *(optional, automatable guard)* `tests/examples-coverage/` — a structural test
  that loads each `.cfrm`, and asserts/report it has one `EventBinding` per
  `supported_events` and one property button per `default_props` key.
- **No product code changes expected** in `crates/*`, unless the enumerator
  reveals a supported property with **no** runtime setter — then a small
  `obj_set`/`SetProperty` gap in `crates/cobolt-runtime` is filed as a
  prerequisite (see Risk R-b).

## 3. Data / model changes

None to types or formats. Projects use the existing `cobolt.toml` schema and
`.cfrm` form schema as-is. `main` points to `src/main.cbl` per the IDE's
form-project convention; the form's program is regenerated into `generated/`
on Build (never hand-edited). No migration/compat concerns.

## 4. Key decisions & alternatives

- **Author with the IDE designer, commit static output** — Why: guarantees valid
  `.cfrm` and exercises the real product; matches "hand-authored, committed".
  Rejected: a shipped generator command (user declined); raw hand-written `.cfrm`
  XML (valid but error-prone — allowed only as a fallback for tiny edits).
- **Generic `SetProperty(name, value)` as the default property setter** — Why:
  uniform "one button per property", minimal COBOL, works for any property name.
  Rejected: bespoke type-specific method per property everywhere (more authentic
  but far more code and inconsistent); used only where a property needs
  structured arguments.
- **One form per project, button column beside the subject** — Why: simplest to
  open, run, and verify in a single window. Rejected: multi-form or one form per
  property group (busier, no benefit). (Spec Q2.)
- **External-service controls included, services assumed** — Why: user choice;
  full coverage. Each ships a README naming the expected local service and
  placeholder connection settings. (Spec R8.)
- **Examples are manual fixtures, not part of `tests/controls/`** — Why: spec Q4;
  confirmation is visual/console by the operator. An optional structural coverage
  test is the only automatable guard.

## 5. Risks & mitigations

- **R-a Volume / completeness (34 forms, many props each).** → Author in control
  category waves; drive every form from the enumerator checklist; `rcrun build`
  each before moving on; the optional coverage test catches missing events/props.
- **R-b A supported property has no runtime setter** (read-only or designer-only,
  so `SetProperty` is a no-op). → The enumerator + a quick runtime check flag
  these; document such properties in the project README instead of shipping a
  dead button, or file a small runtime setter as a prerequisite. Verify-first:
  never claim a button "changes" a property the runtime ignores.
- **R-c External controls can't fully run without services.** → Build must still
  pass (compile-time). Runtime event/property confirmation is documented as
  requiring the local REST/DB/LLM service; the project still builds and opens.
- **R-d Observability of some properties** (e.g. TabOrder, non-visual flags). →
  For non-visual effects, the handler also `DISPLAY`s the new value so there's a
  console signal even when nothing visibly changes.
- **R-e Sample-data dependencies** (charts/grids/lists need data). → Ship minimal
  inline data so each property's effect is visible without external setup.

## 6. Test strategy

- **Build verification (automatable):** a loop that runs
  `rcrun build examples/<c>/cobolt.toml` for all 34 projects; **report**
  pass/fail per project; zero failures required (AC5). Can be a shell script or a
  CI step.
- **Structural coverage (optional, automatable):** a Rust test that loads each
  `.cfrm` via `cobolt-forms`, and for the subject control **reports** the count
  of event bindings vs `supported_events` and property buttons vs `default_props`
  keys, failing on any gap (AC2/AC4). Verify-first: it reports actual counts.
- **Manual / visual (operator, per AC2–AC4):** open each project in the IDE, Run,
  hover/click/interact to fire each event → confirm the console shows
  `"<Event> working"` once per event; click each property button → confirm the
  colour/geometry/animation/etc. change (or the `DISPLAY`ed value for non-visual).
- No changes to existing crate tests expected; if R-b forces a runtime setter,
  that change carries its own `cobolt-runtime` test.

## 7. Steering compliance

- [x] i18n: N/A — example projects are sample COBOL apps, not IDE UI; no `Tr`
  fields, no hard-coded-literal concern.
- [x] Generated-code banner + regenerate-on-action contract preserved — COBOL
  lives in form handler source; `generated/*.cbl` stays a build artifact.
- [x] English dev guide updated (new "Per-control examples" subsection);
  translations untouched; steering `docs.md` registry row added.
- [x] Fix vs feature: **feature** → minor (`y`) bump in `version.rs` +
  `CHANGELOG.md`; committed separately from any fix.
- [x] No "cobolt" in user-facing text; all COBOL identifiers, `DISPLAY` strings,
  and button captions in English.
