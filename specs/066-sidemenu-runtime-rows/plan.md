# Plan — SideMenu: run-time rows and hosted controls

- **Status:** draft → awaiting operator review
- **Spec:** ./spec.md   **Date:** 2026-09-22

## 1. Approach

Three phases, each shippable on its own, in dependency order. Phase A is the
bulk of the value (a conversation list is data); B and C are the two hosting
mechanisms the operator asked for (Q3 "both").

```text
  COBOL                       interpreter                    host state            surfaces
  ─────                       ───────────                    ──────────            ────────
  MENU-1::AddItem(…)  ──▶  exec_method (class = SideMenu) ─▶ RuntimeRows (JSON) ─▶ render_interactive ─┐
                           │ cobolt_forms::menu::runtime      StateUpdate           shell MenuPane ─────┤
                           │ (pure: add/replace/remove,                                                ├─ merge_rows(designed, runtime)
                           │  depth, designed-id guard)                                                │   then sidebar::layout
                           └ designed ids from the .menu.yaml, seeded at start-up                      ┘
```

### Phase A — run-time rows (R1–R10, R14–R16)

**One property carries the rows.** A SideMenu gets a run-time-only property
`RuntimeRows`: a JSON array of rows the program added, in the order it added
them. Each row is a `MenuItem` (the designed model, unchanged in meaning) plus
the id of the row it hangs under, if any. It travels exactly as every other
property a program writes — `obj_set` → `StateUpdate` → the host's per-control
state — so it reaches `rcrun run-form`, embedded forms and the compiled binary
through the path all three already share (the `interpreter-binary-parity` rule
is satisfied by construction for the transport). It is never seeded by
`Control::new` and never saved, so the designer canvas cannot show it (R16).

**One merge, used by both surfaces.** `cobolt_forms::menu::merge_rows(designed,
runtime) -> Vec<MenuItem>` (non-render, so the interpreter can use it too)
returns the designed items followed by the run-time ones; a row with a parent is
appended as that parent's last child (R6). `render_interactive`'s SideMenu arm
and the shell's `draw_mounted_menus` both call it before `sidebar::layout`, so
the row list the two lay out is the same list (Q4's hard rule; AC7).

**The methods live in a pure module.** `cobolt_forms::menu::runtime` holds the
operations as functions over `(designed ids, rows) → Result<rows, Refusal>`:
add/replace (R1, R8), add section (R2), set label/icon/badge/enabled/action (R3,
R7a), remove with descendants (R4), clear (R4), count/has (R9), and the depth
limit (R10, needs `MAX_DEPTH` made `pub`). Unit-tested without an interpreter —
the Snackbar `Buttons` precedent (`snackbar::add_button`).

**The interpreter dispatches on class, never on name alone.** `ADDITEM`,
`REMOVEITEM`, `CLEAR` and `GETCOUNT` already exist as generic list methods that
edit `Items`, which a SideMenu does not have. Each gains a SideMenu branch
first (the `is_snackbar` pattern at `CLEAR`/`SHOW`), and the new names —
`ADDSECTION`, `SETITEMLABEL`, `SETITEMICON`, `SETITEMBADGE`, `SETITEMENABLED`,
`SETITEMACTION`, `HASITEM` — go into `exec_method` **and** `is_known_method`
(guarded by `every_dispatched_control_method_is_spellable_inline`). A refused
call returns `"0"` and the program continues (R5); success returns `"1"`.

**The interpreter must know the designed ids** (R5, R8, R10: refuse a designed
id, know the depth a parent sits at). Today it has no copy of any `.menu.yaml`.
A new `Interpreter::set_designed_menu(ctrl_id, MenuDefinition)` is called at
the three seed sites — `form_gui.rs` (beside `register_menus`), the compiled
template (from `MENUS`), and `host.rs` when it seeds an embedded/child form
(from the process-wide menu registry the host already fills). Three sites, per
`interpreter-binary-parity`; a test asserts each.

**Actions (R7, Q2).** A run-time row carries an `action` string exactly as a
designed one. On the shell MenuPane the existing `process_menu_clicks` already
dispatches on it, so run-time rows navigate for free. On `render_interactive`
(a SideMenu in a plain window, preview, Run Form outside shell mode) a designed
row's action is **ignored today** — only `onMenuItemClick` fires. R7 says a
run-time row does "exactly what a designed row with the same action does", so
it inherits that same per-surface behaviour; making the window-hosted SideMenu
navigate is a separate, pre-existing gap (§5, flagged, not fixed here).
An `AddItem` with no action stores `event`, so an iconed run-time row appears on
the collapsed rail like a designed one (`shows_on_rail` requires an action).

**The shell must read live state.** It draws from its mounted definition and a
designed clone of the control, never the host's state. A new
`FormHost::control_prop(ctrl_id, prop) -> Option<String>` (read-only) lets
`draw_mounted_menus` fetch the root SideMenu's `RuntimeRows` and live
`SelectedItemId`. Known ordering: the rail is drawn before the pane drains
`state_rx`, so a write shows one frame later in the shell than in a window —
invisible at 60 fps, noted in §5.

### Phase B — header band (R11–R13)

Mirror the footer Panel, which already works on every surface:

- `HeaderBandHeight` on the SideMenu (default **0** = no band).
- `Form::sync_side_menu_header_panels` (model.rs, beside
  `sync_side_menu_footer_panels`): while the height is > 0, a Panel
  `<side>-Header` marked `IsSideMenuHeader`, `parent = side`, pinned below the
  title (`y + HeaderHeight`), created if missing; nothing is created while 0.
- `sidebar::layout` emits a `RowKind::HeaderBand` row and moves `menu_band`'s
  top down by the band height **while expanded**; collapsed, the band is 0 high
  and its subtree hidden (R12 — unlike the footer, which only narrows).
- Designer: synced every frame and drag-locked, like the footer.
- Window host / preview / Run Form: the panel is an ordinary child container;
  `render_form_inner` skips a header subtree whose SideMenu is live-collapsed.
- Shell: `FormBody::draw_side_menu_footer` is generalised to
  `draw_side_menu_band(ui, rect, subtree)`, and the shell calls it for the
  header band's rect as it does for the footer's.

### Phase C — rows that host a control (R13a–R13b)

- `MenuItem` gains `control: Option<String>` — the id of a control on the same
  form. `#[serde(default, skip_serializing_if = "Option::is_none")]`, so every
  existing `.menu.yaml` round-trips byte-identical and its HMAC still verifies.
- Designer: the menu editor gains a *Hosted control* combo (controls whose
  parent is this SideMenu); run time: `SetItemControl(id, control-id)`.
- `sidebar::layout` reports the row's rect; the row paints its background and
  icon but no label.
- Positioning the control — the one genuinely new mechanism. Precedent: a
  Splitter pane's child rect is derived at render time from the owner's live
  state (`resolved_rect` → `splitter_pane_rect`, render.rs). A
  `side_menu_row_rect` hook in the same place derives a hosted control's rect
  from the SideMenu's layout. The shell draws each hosted control through the
  same band pass as Phase B (`draw_side_menu_band` with the row's rect).
- Collapsed (R13b): the row shows its icon only and the control is hidden.

## 2. Affected crates / files

- `crates/cobolt-forms/src/menu.rs` — `RuntimeRow`, `merge_rows`, the
  `runtime` operations, `MAX_DEPTH` → `pub`; (C) `MenuItem.control`.
- `crates/cobolt-forms/src/sidebar.rs` — (B) `RowKind::HeaderBand`, band height
  in `SidebarChrome`/`menu_band`; (C) hosted-row rects.
- `crates/cobolt-forms/src/model.rs` — `RuntimeRows` in
  `runtime_property_names_for("SideMenu")`; (B) `HeaderBandHeight`,
  `IsSideMenuHeader`, `sync_side_menu_header_panels`,
  `side_menu_header_subtree`.
- `crates/cobolt-forms/src/render.rs` — SideMenu arm merges rows; (B) skip a
  collapsed menu's header subtree; (C) `side_menu_row_rect` in `resolved_rect`.
- `crates/cobolt-form-host/src/host.rs` — `control_prop` accessor;
  `set_designed_menu` at the child seed site; (B/C) `draw_side_menu_band`.
- `crates/cobolt-form-host/src/shell.rs` — merge live rows in
  `draw_mounted_menus`; (B/C) band passes.
- `crates/cobolt-runtime/src/interpreter.rs` — SideMenu branches of
  `ADDITEM`/`REMOVEITEM`/`CLEAR`/`GETCOUNT`, the new methods,
  `is_known_method`, `set_designed_menu`.
- `crates/cobolt-cli/src/form_gui.rs`, `crates/cobolt-compiler/src/lib.rs`
  (template) — `set_designed_menu` at seed time.
- `crates/cobolt-ide/src/panels/designer.rs` — (B) sync + drag-lock the header
  panel; (C) menu editor *Hosted control* row.
- `crates/cobolt-ide/src/panels/properties.rs` — (B) `HeaderBandHeight` row.
- `crates/cobolt-ide/src/panels/editor.rs` — autocomplete entries for the
  SideMenu methods.
- `crates/cobolt-ide/src/i18n.rs` — (C) the menu editor's *Hosted control*
  label, ×6.
- `crates/cobolt-compiler/src/lib.rs` — KB: SideMenu methods, `RuntimeRows`,
  `HeaderBandHeight`, hosted rows; regenerate `assets/knowledge/chunked.data`.
- `docs/developers-guide-en.md` — SideMenu section: run-time rows, header band,
  control rows.

## 3. Data / model changes

- **`RuntimeRows`** (run-time only, never in `.cfrm`): JSON
  `[{"parent": "<id>" | null, "item": <MenuItem>}, …]`, in insertion order.
  Replacing an id keeps its position (R8).
- **`.menu.yaml`** (C): optional `control:` per item; absent ⇒ unchanged file
  and hash.
- **`.cfrm`** (B): `HeaderBandHeight` on SideMenu (absent ⇒ 0 ⇒ no band, so
  every existing form is unchanged); a `<side>-Header` Panel appears only when a
  developer gives the band a height. `seed_missing_props` backfills
  `HeaderBandHeight = 0` so the row shows in the pane.
- No codegen change: controls are learned from `seed_objects`, and neither the
  footer Panel nor the header Panel is referenced by generated COBOL.

## 4. Key decisions & alternatives

- **Rows as one JSON property** — Why: it reuses the one transport every host
  already has, with no new channel or host API on the write path. Rejected: a
  `HostAction` round-trip (like `SetMenuPaneCollapsed`) — it would need its own
  queue on three hosts and a separate replay for the shell; rejected: a
  tab-separated text format (Maps `Markers` precedent) — a row has nine fields,
  several free text, and a parent link.
- **Operations in `cobolt-forms::menu`, not the interpreter** — Why: pure,
  unit-testable, and the same code could later serve a designer preview.
  Precedent: `snackbar::add_button`.
- **Designed ids seeded into the interpreter** — Why: R5/R8/R10 are refusals the
  program must see as a return value, synchronously; the GUI cannot answer in
  time. Rejected: letting the renderer drop offending rows silently — the
  program would believe a call succeeded.
- **`Clear()` / `GetCount()` mean run-time rows only** — R4/R9: the designed
  menu is the developer's, not the program's. `HasItem(id)` answers for either.
- **Header band default 0 (off)** — every existing form stays byte-identical.
- **Phase C positions controls in the render pass** (the Splitter precedent),
  not by rewriting `rect` in the model — Why: a row's position depends on
  scroll, collapse and run-time rows, none of which exist at design time.

## 5. Risks & mitigations

- **`AddItem` on a SideMenu silently edits `Items`** if a branch is missed →
  every generic arm gets the SideMenu branch first, and a test calls each name
  on a SideMenu and asserts `Items` is untouched.
- **Shell reads state one frame late** → accepted (invisible at frame rate);
  AC7 compares layouts from the same inputs, not the same frame.
- **Window-hosted SideMenu ignores designed actions (pre-existing)** → out of
  scope; R7 holds per surface. Flagged for the operator as a separate fix: a
  `render_interactive` SideMenu could route `open-form:` through the same
  `open_form_via_supervisor` the `OpenStandAloneForm*` methods use.
- **Menu registry key has no form** (two forms' `SideMenu-1` collide in
  `register_menus`) → pre-existing; `set_designed_menu` at the child seed site
  inherits it. Flagged, not fixed here.
- **Phase C is the riskiest** (a new geometry hook, three surfaces) → it ships
  last and alone; Phases A and B do not depend on it.
- **Rounded-corner rules** (B/C paint inside the SideMenu) → read the
  `rounded-corners` skill before touching paint; a header Panel is a child at
  the SideMenu's edge and must self-clip like the footer.
- **No self-resizing** → the band's height is a property; its content never
  grows the SideMenu.

## 6. Test strategy

- **`cobolt-forms` (`menu.rs`, pure)** — a table of operations → expected rows:
  add, add under parent, replace keeps position, remove takes descendants,
  designed id refused on add/rename/remove, depth 4 refused, clear keeps the
  designed rows, merge order. Reports the table it ran.
- **`cobolt-runtime`** — a COBOL program adds 50 rows, a section and a nested
  row; drain `StateUpdate`s and assert the final `RuntimeRows` (AC1); refused
  calls return `0` and `Items` is never written (AC4); every new name spellable
  inline.
- **`cobolt-form-host` (`shell.rs`)** — extend
  `standalone_menu_actions_submit_shell_parented_opens`: feed `RuntimeRows`
  through the host's state channel, click a run-time row, assert
  `SelectedItemId` + `onMenuItemClick` (AC2) and an `open-form:` row opening its
  form (AC6b).
- **Parity (AC7)** — one test builds the same merged rows via the engine's path
  and the shell's path and asserts identical `sidebar::layout` rects.
- **Three hosts (AC8)** — assert `set_designed_menu` is called at
  `form_gui.rs`, the compiled template, and the host's child seed.
- **(B)** sidebar tests: band height moves the menu band while expanded, is 0
  collapsed; model tests: panel created only for height > 0, idempotent sync;
  shell test modelled on `shell_chrome_is_immune_to_the_occupants_theme`.
- **(C)** round-trip a `.menu.yaml` with and without `control:` and assert the
  hash verifies; hosted-row rect equals the layout row's rect on both paths.
- `cargo test -p cobolt-forms --features render` green, including
  `engine_reference_form_parity_static_vs_faces` (AC9).
- **Manual (operator):** a demo form adding conversation rows from a button,
  with a ComboBox in the header band and one in a row, in Run Form and in a
  built app.

## 7. Steering compliance

- [ ] i18n: the menu editor's *Hosted control* label in 6 languages; property
      names are identifiers.
- [ ] Generated-code contract untouched (no codegen change).
- [ ] English Developer's Guide updated (only `-en.md` exists for it today).
- [ ] System KB tables updated and `chunked.data` regenerated in each phase.
- [ ] Feature → `features` branch, `z` bump per phase, CHANGELOG entry.
- [ ] No "cobolt" in user-facing text; COBOL identifiers English.
