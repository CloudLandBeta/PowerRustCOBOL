<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — SideMenu / sidebar, icon catalogue, and the in-progress rail redesign

> Supersedes the 2026-08-09 spec-049 handoff, which was consumed and committed
> at the start of this session (it is in git history at `6977a0b`).

- **Date:** 2026-08-10. **Branch:** `features`.
- **Committed this session:** `156fdc4` (a documentation fix) and `6977a0b`
  (spec 049, 1.61.14). Local `features` is **4 commits ahead of
  `origin/features`** — nothing has been pushed.
- **Version in tree:** **1.61.16** (`crates/cobolt-ide/src/version.rs`).
  Everything after `6977a0b` is **uncommitted working-tree changes**.
- **Nothing merged to `main`. Nothing pushed. Nothing announced.**

## How this session went

The operator pointed at the previous handoff, then drove five waves of work:

1. **Committed spec 049** — split into a docs *fix* commit and the *feature*
   commit, because the tree held both (golden rule #5).
2. **Sidebar behaviour** (1.61.15) — the menu editor reaches the SideMenu, the
   ☰ moved onto the pane and survives an empty menu, `FullHeight`.
3. **The icon catalogue** (1.61.16) — rebuilt from scratch: one engine, 709
   icons, 30 categories, styleable.
4. **Sidebar icons + live rail** — icons render on every surface, `IconEffect`,
   menu-editor Indent/Outdent, the Preview sidebar became clickable.
5. **The AdminMart-style rail redesign** — specced by the operator from a
   reference screenshot. **Started, NOT finished** (see *Where it stops*).

## Committed and pushed state

| | |
|---|---|
| `origin/features`, `main`, `origin/main` | `5e66ade` (1.61.13) |
| local `features` | `6977a0b` (1.61.14) — 4 ahead of origin |

The **push window rule** (golden rule #1) blocked every push this session:
never push 09:00–18:00 São Paulo, Mon–Fri, *even when asked*. The operator
asked twice; both times the answer was to commit and hold. Check the clock, do
not assume.

## What is uncommitted, by theme

### A. Sidebar behaviour (1.61.15)
- `FullHeight` property (default on) — the sidebar owns the window's whole
  vertical extent and the breadcrumb insets; off, the breadcrumb spans the top.
  It is a **layout ORDER**, not a size: whichever egui panel is created first
  owns the corner (`shell.rs`).
- While `FullHeight` is on the control's `Y`/`Height` are inert — greyed in the
  inspector, and `Form::sync_side_menu_full_height()` pins the rect every
  designer frame so the control follows a form resize.
- `Collapsed` property — the state the application *opens* in; the operator's
  own remembered choice (persisted per application) wins at run time.
- The ☰ moved from the breadcrumb **onto the MenuPane**, drawn in both states
  and whether or not the menu has items.
- A pane-surface `FormHost` **withholds the SideMenu control from painting**
  (`host.rs`) — otherwise the shell paints the sidebar as chrome *and* the
  hosted form paints it again inside the ContentPane. State is untouched, so
  `SelectedItemId` still works.

### B. The icon catalogue (1.61.16)
- `crates/cobolt-forms/src/icons.rs` — **rewritten**. Icons are vector shape
  data on a 24-unit reference grid (`PathOp`/`IconShape` DSL), one 1.5-unit
  stroke, round caps via stroke-width dots on authored vertices, real
  beziers/arcs. **Resolution-independent** — the same data paints a 16 px menu
  row and a 128 px tile.
- **709 icons, 30 categories** (`MENU_ICON_CATEGORIES` — the ONE source; the
  menu editor's picker renders it instead of its old inline copy).
- `IconStyle` { color, accent, effect } + `IconEffect` { Plain, DropShadow,
  Neumorphic } → `draw_menu_icon_styled`. `icon_svg_styled` mirrors it with
  real `feDropShadow` filters.
- `crates/cobolt-forms/examples/icon_sheet.rs` (**untracked**) renders
  per-category SVG contact sheets + an effects demo. This is the visual QA
  tool — use it, and *look at the output*; several icons were redrawn only
  because a rendered sheet showed them reading as the wrong object.

### C. Sidebar icons and the live rail
- Menu-item icons render in the sidebar on all surfaces; the collapsed rail is
  icon-only with a first-letter fallback.
- `IconEffect` property on the SideMenu (None | Shadow | Neumorphic).
- `render.rs` gained a `CT::SideMenu` interactive arm: the ☰ toggles live
  (`onMenuOpen`/`onMenuClose`) and rows click (`SelectedItemId` +
  `onMenuItemClick`) — this is why Preview and Run Form both became live from
  one change.
- Menu editor **Indent / Outdent** — an item becomes a child of the one above
  or is promoted beside its parent, so items move between sections and levels.
  The 3-level cap is enforced *including the moved item's own subtree*.

### D. The rail redesign — **IN PROGRESS**
The operator supplied a reference screenshot (AdminMart) and this roadmap:

> Collapsed: a narrow rail, logo + centred icons, ellipsis separators dividing
> three groups, active item in a rounded square. Expanded: logo + app title,
> grouped sections, each row a left-aligned icon + text with optional right-side
> badge / chevron / counter / outlined tag, active row a wide rounded rectangle,
> a profile card near the bottom, generous spacing.
> **"Sections and Colors are suggestions… The sidebar should follow
> PowerRustCOBOL's themes for applications instead."**

Design decisions already taken (and implemented in 11–12 below):
- **Section headers are a `Separator` carrying a label** — the field existed and
  was unused; unlabelled separators stay hairline rules. No new concept, and
  every existing `.menu.yaml` still loads.
- **Chevron is derived** from having children, not a property.
- **Active row** is driven by the existing `SelectedItemId`.
- **Colours come from the control's existing theme props**
  (`ForegroundColor`, `SelectedBgColor`, `SelectedFgColor`,
  `HighlightBgColor`) — never constants. That is the operator's
  "follow the application's themes" instruction.

## Where it stops (read this first)

Tasks, in the order they must land:

| # | Task | State |
|---|------|-------|
| 11 | Menu model: labelled-separator sections, `badge` + `badge_style` | **done** |
| 12 | `crates/cobolt-forms/src/sidebar.rs` — the ONE shared painter | **done** |
| 13 | Route all four surfaces through it | **not started** |
| 14 | Chrome properties + inspector + menu-editor badge fields | **not started** |
| 15 | Visual QA, docs, KB, CHANGELOG, sweep | **not started** |

### ⚠️ `sidebar.rs` is currently DEAD CODE
It compiles and its 5 tests pass, but **nothing draws through it yet**. A
`--release` rebuild today still shows the OLD rail. Task 13 is what makes the
redesign visible; do not report the redesign as delivered before it lands.

### What task 13 actually is
Three implementations of the rail exist and must collapse into one:

1. `paint.rs` — the design-time canvas arm (`CT::SideMenu`, ~line 4056).
2. `render.rs` — the interactive arm (`CT::SideMenu`) used by Preview + Run Form.
3. `shell.rs` — `Shell::draw_mounted_menus`, egui widgets in the MenuPane.

Each should become: build a `SidebarState`, call `sidebar::layout`, `paint`,
and `row_at` for hit-testing. **This divergence is the root cause of every
sidebar bug the operator reported** — icons on one surface but not another, a
dead ☰ in Preview, labels escaping the rail. Do not fix a rail bug in one of
those three files again; fix it in `sidebar.rs`.

### The operator's screenshot (the bar to clear)
The Preview showed: text vertically centred instead of top-anchored; a label
**rendering outside the rail**; no ☰; the rail not reaching the bottom. The
first three are already addressed *in the new module* — the label is clipped to
the space left after badge and chevron, and a test asserts every laid-out row
is contained by the rail rect in both states. They will remain visible on
screen until task 13 routes the surfaces through it.

## Test state (verified)

- **Last full sweep: 102 suites, 1,833 passed, 2 failed.**
- The **2 failures are pre-existing and environmental** —
  `test_external_crates_e2e::external_crates_alias_build_and_run` and
  `…::external_crates_build_run_manifest_and_determinism`; their nested
  `cargo build` cannot compile `libsqlite3-sys` in this sandboxed shell. The
  previous session verified this by stash-rerun. Do **not** chase them.
- `cargo test -p cobolt-forms --features render` → **314 passed**, including
  the 5 new `sidebar::tests` and 5 `icons::tests`.
- **`cargo test -p cobolt-forms` does NOT compile without `--features render`**
  (`model.rs` calls `paint::contrast_ratio`). Always pass it. Pre-existing.
- KB freshness green (995 records / 5 docs) and i18n green after every change.

## Gotchas

- **The operator's running binary is always stale.** Every visual complaint this
  session was against a binary predating the fix. Rebuild `--release` before
  concluding anything about on-screen behaviour, and say so when reporting.
- **Never drive the application** to verify — build, test, and let the operator
  look. Visual QA of *icons* is the exception: the SVG contact sheets are
  inspectable without running anything.
- **The System KB is Rust constants**, not the markdown. Behaviour changes must
  edit the doc constants in `cobolt-compiler/src/lib.rs` **and** rerun
  `cargo run -p cobolt-ide --example build_chunked_kb`, or the freshness test
  goes red.
- **i18n is code-only.** New UI strings need a `Tr` field and a value in all six
  languages. `cobolt-form-host` **cannot reach `Tr`** — that is why the shell's
  ☰ has no tooltip. Keep shell chrome symbolic or add a translation route first.
- **egui headless frames:** always `full.textures_delta.clear()` after
  `ctx.run_ui`, or epaint panics on drop.
- **`egui::Context::run_ui`, not `run`** — this workspace's egui is Ui-hosted.
- Positional click tests are brittle: `Shell` records `toggle_rect()` and
  `item_rect(id)` so tests click the widget they mean. A magic coordinate broke
  the moment the pane gained chrome — don't reintroduce one.
- The menu sidecar is keyed by **control id**, which is why the MenuBar editor
  worked for SideMenu with no storage change.
- Version bumps are the **fix number only** (`z`); the operator alone raises
  `x`/`y`.

## What remains, in order

1. **Tasks 13 → 14 → 15** above. 13 is the one that makes the redesign real.
2. **Operator manual pass** — rebuild `--release`; check the rail expanded and
   collapsed, badges, the active pill, the profile card, and that icons appear
   in the designer, Preview, Run Form and the shell.
3. **Commit.** The tree is currently ONE feature bundle (1.61.16). If any part
   is reclassified as a fix, split it — fixes and features never share a commit
   (golden rule #5).
4. **Merge → push → announce**, only when the operator asks: push obeys the São
   Paulo window; features announce on **f=96** with the `[Noticia]` prefix,
   fixes on **f=97**; Spanish, vBulletin BBCode, signed "Anthropic Claude Codex
   Agent", title ≤ 50 chars, native browser submit (windows-1252), exact text
   confirmed with the operator first, and **only after it is on `main`**.

## Still open from before this session

- **Loading a SECOND form into the ContentPane** is still not implemented —
  it needs the same multi-form runtime as spec 037's T16. `open-form:` menu
  items report themselves at run time instead of loading. AC9's
  WORKING-STORAGE half remains unverified and the compiled-app template has no
  shell branch.
- Spec 049's unresolved questions Q5/Q6/Q8/Q9 (`spec.md` §7) are still the
  operator's call.
- The shell window opens at a fixed 1100×700, and the MenuPane still uses fixed
  220/48 widths rather than the designed control width. Task 14's
  `OpenWidth`/`CollapsedWidth` properties are meant to close the second half.
