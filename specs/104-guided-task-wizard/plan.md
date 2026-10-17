# Plan — Task Wizard

- **Status:** approved (operator, 2026-10-17 — "when you finish planning implement it")
- **Spec:** ./spec.md   **Date:** 2026-10-17

## 1. Approach

Four pieces, each testable on its own, glued to the app at a handful of call sites.

**1. Shared spotlight primitives** (`panels/spotlight.rs`). The Walkthrough's pure geometry
and paint move here unchanged — `scrim_bands`, `place_balloon`, `Balloon`, `Side`,
`balloon_palette`, `Palette`, `DIM`, the geometry constants — and `walkthrough.rs`
re-exports them, so its code and tests do not change (R38). A characterisation test dumps
the shapes the Walkthrough paints for each step before and after the move. Two additions:

- `blockers(screen, holes) -> Blockers` — the screen minus up to `MAX_HOLES` holes, as
  disjoint rectangles (a slab decomposition), in a fixed-size array; no new dependency.
- `place_balloon_avoiding(screen, avoid, aim, size)` — keeps the balloon off the live
  region while its tail aims at the anchor; `place_balloon(s, t, z)` is
  `place_balloon_avoiding(s, t, t, z)`.

**2. The engine** (`panels/task_wizard/`), pure:

- `tasks.rs` — the declarative table (R37): `TaskDef { id, title, steps }`, `StepDef { key,
  anchor, live, via, title, body, condition, applies, mechanical, keys }`. Texts are `fn(&Tr)
  -> &'static str`, conditions and preconditions `fn(&StepCtx) -> bool`.
- `conditions.rs` — the library of conditions over `Facts`.
- `facts.rs` — `Facts`, the read-only snapshot the app builds each frame while a task runs.
  It has no field derived from a secret field (R28).
- `mod.rs` — `Engine` with `frame(&FrameInput, &[TaskDef]) -> FrameOutput`: start / resume /
  restart / Next / Back / Exit; conditions are **edges seen while the step shows** (R17);
  steps that do not apply are skipped and not counted (R19); `via` recovery (R20); subject
  capture; the summary (R21); pause (R24). Output: one `ViewportPlan` per viewport (holes,
  blockers, ring, balloon content and placement, chip) and `Effect`s (persist progress, mark
  done, do a mechanical action, reveal an anchor, focus a viewport, ended with a summary).

**3. Overlay, anchors and input** (`panels/task_wizard/{anchors,overlay,input}.rs`):

- **Anchors** (R35): a thread-local `RefCell<Option<Sink>>`, installed per frame only while
  the Task Wizard is on. `anchors::mark(ui, id, rect)` is one `None` check when off. Each
  mark records rect, clip rect and viewport. Windows report from `Window::show`'s response;
  Properties rows from their widget responses or the cursor span around them — never a
  conditional `ui.scope`, which would re-id the widgets inside it.
- **Overlay** (R12–R16): a fixed pool of interactable `Area`s at `Order::Middle`, one per
  blocker rect, raised every frame and shown every frame while running (unused at zero size),
  each sensing click-and-drag over its exact rect. Popups (Foreground) and tooltips stay above
  them and live (R14). The balloon is an interactable `Order::Foreground` Area that is **not**
  raised every frame, so a popup opened from the live region lands above it. No
  `set_modal_layer`: it would deny focus and wheel-scroll to the target window. The anchor's
  window is raised every frame. A viewport that does not hold the anchor is blocked whole
  except its toggle and shows a "continue in" chip (R15).
- **Input** (R22, R23): `filter_input(ctx)` first thing in each viewport pass — the two-stage
  `Esc`, and stripping of key, text, IME and clipboard events unless focus is inside the live
  region (or the step lets keys reach the whole Designer). At the end of each pass, focus held
  outside the live region is surrendered. `handle_shortcuts` returns early while a task runs.

**4. Chrome** — the split toggles (R1–R4), the Help item (R6), the catalogue popup (R3, R8,
R9), offers (R10) and persistence (R5, R29–R31).

**App glue** — `src/app/task_wizard_glue.rs`, a child module of `app` (it sees `CoboltApp`'s
private fields): `build_facts`, `apply_effects`, the per-viewport paint hooks. `app.rs` gets
call sites only: the input filter (main pass and designer pass), the engine tick after the
Walkthrough's, the designer paint after `show_designer_window`, the `handle_shortcuts` gate,
toolbar and Help wiring.

## 2. Affected crates / files

All in `crates/cobolt-ide/src/` unless noted.

- `panels/spotlight.rs` — **new**: the shared primitives, `blockers`, `place_balloon_avoiding`.
- `panels/walkthrough.rs` — re-exports from `spotlight`; no behaviour change.
- `panels/task_wizard/{mod,tasks,conditions,facts,anchors,overlay,input,catalogue,offers}.rs` —
  **new**.
- `app/task_wizard_glue.rs` — **new**; `app.rs` — call sites.
- `panels/toolbar.rs` — the split toggle, `ToolbarAction::{ToggleTaskWizard,
  OpenTaskCatalogue}`; `aurora.rs` — an engaged pill.
- `panels/designer.rs` — the split toggle in `draw_icon_toolbar`
  (`DesignerToolbarAction::{ToggleTaskWizard, OpenTaskCatalogue}`, `icon_task_wizard`), anchor
  marks, facts getters, the menu-editor starter used by Do this for me.
- `panels/properties.rs`, `panels/models_modal.rs` (a key-free `TestOutcome`),
  `panels/agents_modal.rs`, `panels/grid_edit.rs`, `panels/settings_form.rs` — anchor marks and
  `pub(crate)` facts getters.
- `menu_layout.rs` — `Item::TaskWizard`; `ui_prefs.rs` — the four fields.
- `i18n.rs` — the `Tr` fields in six languages and a test module.
- `panels/mod.rs` — module declarations.
- `docs/developers-guide-en.md` — a *Task Wizard* section.
- `version.rs`, `CHANGELOG.md` (repo root).

## 3. Data / model changes

`UiPrefs` (`ui.toml`, `#[serde(default)]`, so an older file reads every new field as empty):

- `task_wizard_on: bool`
- `task_wizard_done: BTreeSet<String>` — task keys.
- `task_wizard_progress: BTreeMap<String, BTreeMap<String, String>>` — project key → task key →
  step key.
- `task_wizard_dismissed: BTreeSet<String>` — `project|form|control|task`.

No change to `.cfrm`, `cobolt.toml`, generated COBOL or any serialized format.

## 4. Key decisions & alternatives

- **A live hole made of blocker Areas** — Why: an egui interactable Area blocks its whole rect
  (a full-screen one cannot have a hole) and a modal layer denies focus to the target window.
  Rejected: reusing the Walkthrough's overlay; `set_modal_layer`.
- **Blockers at `Order::Middle`** — Why: popups and tooltips (Foreground, Tooltip) stay above
  them and live. Rejected: Foreground blockers (a reopened popup would land under them).
- **Edge-triggered conditions** — Why: Back must not bounce; pre-filled fields must not skip
  their step. Rejected: "condition holds now".
- **Facts with nothing about secrets** — Why: provable. Rejected: a "key typed" boolean.
- **Anchors via a thread-local sink** — Why: no lock, no allocation when off, isolated per test
  thread. Rejected: egui temp data (takes the context's write lock on every call).
- **Progress by step key** — Why: stable when a task is edited. Rejected: step index.
- **Connect an agent, not create one** — Why: creation is deliberately disabled.

## 5. Risks & mitigations

- **Hit-testing lags one frame behind layout** (egui) → blockers are shown every frame from a
  fixed pool, so a slot never appears late; the one-frame lag on a shrinking band is accepted.
- **A newly opened window sits above the blockers for one frame** → that is also R16's rule; the
  window becomes a hole.
- **The Designer is its own viewport** → each viewport paints its own plan at the end of its own
  pass from the shared engine state; one frame of lag on a step change.
- **The managers are drawn only while Project Settings is the main pane** → task steps start by
  making it the main pane, and its via is the project tree's root.
- **`app.rs` is very large** → it gets call sites only; the logic lives in the glue module.
- **Toolbar crowding** → checked at 900 px wide.

## 6. Test strategy

`cargo test -p cobolt-ide --bin cobolt-ide`. New tests report what they measured.

- **Spotlight:** the Walkthrough shape dump is identical before and after the move; `blockers`
  covers the screen minus the holes exactly, disjointly, for random holes; the balloon avoids the
  live region and its tail ends inside the anchor.
- **Engine (pure):** every route through a task; edges, already-done, skip and count, via
  recovery, progress keys, pause, summary; a fixture task runs with no engine change (AC26).
- **Overlay render tests** (`Context::run_ui` frames, no app): a click inside the hole reaches a
  button and one on the dim does not; the wheel over the dim scrolls nothing; a combo list over
  the dim can be picked; focus outside is released; `Esc` in two stages; no window resizes over
  120 frames in six languages; mode off installs no sink and creates no layer.
- **Per task:** the ring equals the painted anchor rect for every step (AC6); Do this for me
  equals the manual route with one undo (AC17, AC18); no forbidden step is mechanical (AC19).
- **Secrets:** a planted key appears in neither `ui.toml`, the logs nor the `Debug` of the facts
  (AC20).
- **i18n:** a `task_wizard_strings_104` module (non-empty, placeholders kept, different from
  English, CJK in JA/ZH) and the six-language render probe over the catalogue and a balloon of each
  task (AC22). Contrast on painted colours over the 32 themes (AC23).
- **Manual:** the operator looks at it in the IDE; nothing in this plan drives the application's
  window.

## 7. Steering compliance

- [ ] i18n: all new UI strings in 6 languages
- [ ] Generated-code banner + regenerate-on-action contract preserved (untouched)
- [ ] English dev guide updated (only the English guide exists)
- [ ] Feature → `z` bump + CHANGELOG
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
- [ ] DataGrid Quality & Compatibility review before completion (`AGENTS.md`)
