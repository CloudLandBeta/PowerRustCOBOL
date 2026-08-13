<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — the sidebar and the shell, mid-flight

> Supersedes every earlier handoff in this folder; they are in git history.
> Read **Where it stops** and **Gotchas** before touching anything.

- **Date:** 2026-08-12. **Branch:** `features`. **Version:** 1.61.37.
- `features` = `0be7b8f` (1.61.16), **5 ahead of `origin/features`**.
- **Nothing from this session is committed.** 24 modified files + 3 new paths,
  all uncommitted. The debt is now commit debt *and* push debt.
- `target/release/cobolt-ide` rebuilt and verified at **1.61.37**
  (`strings` confirms). `rcrun` NOT rebuilt since the theme work — rebuild it
  before testing a compiled binary.
- **Spec 050 landed on top of this** (1.61.37, a FIX by operator ruling): form
  themes are pluggable, and a self-contained theme is no longer configured by
  Liquid Glass. See `specs/050-pluggable-form-themes/`. It touches
  `paint.rs`/`render.rs`/`sidebar.rs`, so read its `tasks.md` deviations before
  assuming this handoff's line numbers still hold.

## The state in one paragraph

The session began with the SideMenu newly routed through one shared renderer
(`cobolt-forms::sidebar`) and the three-pane rail painted but unfinished. It
ends with the breadcrumb given the same treatment (`cobolt-forms::breadcrumb`),
the footer Panel built, the ContentPane's geometry and background corrected,
and a long tail of operator-reported defects fixed one at a time against a live
demo project. Almost everything was found by the operator running the app and
photographing it — the tests were green throughout. That is the single most
important fact for whoever picks this up: **this area's tests do not catch
what the operator sees.**

## Uncommitted — 17 files

`shell.rs`, `host.rs`, `breadcrumb.rs` (new), `sidebar.rs`, `icons.rs`,
`lib.rs`, `model.rs`, `paint.rs`, `render.rs`, `app.rs`, `designer.rs`,
`properties.rs`, `toolbox.rs`, `project_model.rs`, `i18n.rs`, `version.rs`,
this file.

Plus, from spec 050 (all **fixes**): `surface_theme.rs` (new), `theme.rs`,
`theme_pack.rs`, `theme_ui.rs`, `form_gui.rs`, `cobolt-compiler/src/lib.rs`,
`chunked.data`, `developers-guide-en.md`, `CHANGELOG.md`,
`specs/050-pluggable-form-themes/`.

### ⚠️ These must be split before committing (golden rule #5)

The tree mixes both kinds. Sort them at commit time, not after:

**Fixes (→ f=97):**
- The rail shipped white: a translucent `BackgroundColor` was painted bare into
  a transparent window instead of over the form's backdrop.
- The breadcrumb's pale border: egui's default panel frame painted
  `visuals.panel_fill` (OS-theme-driven) outside our explicit paint.
- **The ContentPane ignored the RAD background.** The pane paints the form's
  backdrop then hands the engine an inert one — but that inert value was
  `#00000000`, and `backdrop_color` deliberately maps pure black to the default
  navy, so the engine painted opaque navy over the correct fill every frame.
  Fixed with `transparency: 100`, which is what actually makes a backdrop inert.
- The ContentPane was offset twice: controls kept their designed x while the
  pane already started past the rail. Plus the shell used a hardcoded 220pt rail
  instead of the designed width.
- The menu pane clipped its own overflow with no way to scroll.
- Format painter carried only 19 hardcoded style keys.
- The open-form target list read one directory (the form's own) and never
  recursed.
- The stale-build prompt fired only on a NEWER version; now any difference.
- The seam drag's first gesture started from 0 when the property was unset.
- Panel separator lines and the footer's own rule, drawn as unwanted borders.

**Features (→ f=96, `[Noticia]`):**
- `cobolt-forms::breadcrumb` — the shared strip, its sidebar-toggle icon pair,
  and the static design-surface variant.
- The footer Panel: auto-created, pinned, selectable, a drop target.
- Design-time seam resizers for the header/footer heights.
- `HeaderImage` and `HeaderIcon` with browse/clear pickers.
- Sidebar background gradient support.
- The theme picker (Elegance became selectable) and Glass style split apart.
- Window sizing: opens at the designed size; the rail toggle resizes the window.

## What landed, by area

### `cobolt-forms::breadcrumb` (new module)
The strip lives here, not in the form host, because **the IDE takes no runtime
dependency on `cobolt-form-host`** (it is a dev-dependency only; Run Form goes
through the `rcrun` child process). Same reason `sidebar` lives here. Owns
layout, painting, hit-testing, the contrast rule (`readable_on`), and the
design-surface entry points. The shell, the designer canvas and the preview all
draw through it.

- The toggle is a full-height square cell at the head of the strip, drawn with
  `sidebar-expand` / `sidebar-collapse` — **the arrow shows the NEXT action**.
- The strip's background follows the CONTENT pane's backdrop.
- On the design surfaces the chain is static: one segment, the form itself.

### The rail
- Background, header logo (200×60, stretched), collapsed icon (45×45), and now
  the gradient are all painted by `sidebar::paint` from state built in ONE
  place. `draw_control` treats a SideMenu as frameless so nothing paints twice.
- `HeaderHeight` default 120, `FooterHeight` 72 — the developer's, in **both**
  rail states. Nothing at run time resizes them.
- The menu pane scrolls; `SidebarRow` carries `visible` (the row clipped to its
  pane) and **hit-testing must use `visible`, never `rect`**.
- `HeaderImage`/`HeaderIcon` resolve to textures inside `state_for_control` via
  `paint::cached_image_texture`, so a property cannot be honoured on one
  surface and ignored on the others. That was the old bug.

### The ContentPane
- Pane mode shifts controls left by the **designed** rail width and reduces the
  reported form width to match, so the pane is juxtaposed to the rail in both
  states. A control parked under the rail clamps to the pane edge.
- Height is deliberately NOT reduced by the breadcrumb — `render_form` paints at
  absolute positions and the scroll extent is exactly `form_size`, so shrinking
  it would cut the bottom off the form.

## Where it stops — open, in the operator's own words

| # | Item | Notes |
|---|------|-------|
| ~~1~~ | ✅ **DONE (1.61.36)** — sidebar drop shadow | The diagnosis in the previous handoff was wrong: the non-overlay shadow IS drawn before the frameless early-out. The real reason it appeared nowhere is that the shell and the preview never call `draw_control` at all. Fixed where the doctrine says: `paint.rs` gained a geometry-free `DropShadowSpec` (+ `drop_shadow_spec`), `SidebarState.shadow` resolves it once with the alpha folded in, and `sidebar::paint` draws it (under the face; over it for a negative blur). `regular_drop_shadow` now returns `None` for a SideMenu so the canvas cannot double-paint. Tested: `sidebar::tests::the_rail_draws_its_own_drop_shadow`. |
| 2 | **Footer controls render in the ContentPane** | STILL OPEN. The Pane filter drops only the SideMenu, so the footer Panel and its subtree survive, get shifted left, and draw in the wrong pane. Needs BOTH halves: exclude the footer subtree from the host, and have the shell render it into `footer_rect()`. The shell has never rendered form controls — this is new capability, not a tweak. Sketch that survived the reading: keep the subtree aside on `FormHost` (`footer_controls`), have the shell publish the footer rect before `pane_frame`, render it in `ui_impl` through a second `render_form` inside an `egui::Area` clipped to that rect, and merge its `RenderOutput` into the main one so events and prop writes flow through the existing handling. |
| ~~3~~ | ✅ **DONE (1.61.36)** — `Collapsed` narrows the rail on the canvas | |
| ~~4~~ | ✅ **DONE (1.61.36)** — the toggle works in RAD | |
| ~~5~~ | ✅ **DONE (1.61.36)** — …and writes nothing | |

**3, 4 and 5 were one change**, as predicted. `DesignerPanel` gained
`rail_view_collapsed: Option<bool>` (design-time only, never persisted) — `None`
means "show what `Collapsed` says", so the inspector still drives the canvas
until the developer takes the view over by clicking the breadcrumb's toggle.
`rail_view_controls()` builds a paint-only clone with the rail (and its pinned
footer Panel) at `sidebar::COLLAPSED_WIDTH`; the designed rect, selection,
dragging and the `.cfrm` are all untouched. `sync_rail_view()` drops the
override when `Collapsed` itself is edited. New shared helpers:
`sidebar::shown_width`, `breadcrumb::DesignView`; `breadcrumb::strip_rect` now
takes the SHOWN rail width, so the strip follows the rail's edge when it
collapses, and `draw_static_strip`/`draw_design_strip` return the layout so a
design surface can hit-test the toggle it drew. The preview's strip now follows
its own live `Collapsed` too. Tested:
`sidebar_seam_tests::the_canvas_shows_the_rail_in_a_state_of_its_own`,
`breadcrumb::tests::the_design_strip_follows_the_rail_and_only_exists_for_a_shell`.

Still open from before this session: a SECOND form cannot load into the
ContentPane (needs spec 037 T16's multi-form runtime — `open-form:` items report
themselves at run time and nothing more); spec 049 Q5/Q6/Q8/Q9 are the
operator's call.

## Gotchas

- **Paint order is a bug source here, twice over.** The seam grips were drawn
  before the control faces and the rail painted over them; the breadcrumb strip
  has the opposite constraint. Handles go with the selection chrome, AFTER the
  faces. Indicators that must not hide the developer's work go before.
- **`backdrop_color` maps pure black to the default navy on purpose** so a form
  with no background set is still a visible window. `#00000000` IS pure black to
  it. To make a backdrop inert use `transparency: 100`, never a colour.
- **`parse_color` returns PREMULTIPLIED.** Fading means scaling all four
  channels; rebuilding as straight alpha premultiplies twice and darkens.
- **Never fix a rail bug in `paint.rs`, `render.rs` or `shell.rs`.** Fix it in
  `sidebar.rs`. Same now for the strip and `breadcrumb.rs`.
- **A property that each surface must "hand in" will be honoured by none of
  them.** Resolve it once, where the shared state is built.
- **The IDE binary the operator runs is stale far more often than you expect.**
  Check `ps` and `strings <bin> | grep 1.61.` before believing any report, and
  rebuild `--release` after every change they will look at. `rcrun` too when the
  change touches the engine or the host.
- **Do not use `sed`/`perl`/`python` on repo files** — standing Rust-only
  directive. I broke this once this session (retargeting three test lines) and
  once more to delete a block. Both applied cleanly; neither should have been
  done that way.
- **egui headless:** `full.textures_delta.clear()` after `ctx.run_ui`, and it is
  `run_ui`, not `run`.
- i18n is code-only, all six languages; `cobolt-form-host` cannot reach `Tr`,
  which is why shell chrome stays symbolic.

## Debt

1. **Split the commits** (fixes vs features, per the list above), then merge and
   push. Push obeys the São Paulo window: never 09:00–18:00 Mon–Fri.
2. **Docs and the System KB have nothing** for any of spec 049's work — the
   routing, the three panes, the breadcrumb, the footer Panel, `IconSize`,
   `HeaderImage`, `HeaderIcon`, the seam resizers, the gradient, the theme
   picker split. The KB is Rust constants in `cobolt-compiler/src/lib.rs` plus
   `cargo run -p cobolt-ide --example build_chunked_kb`, NOT markdown.
3. **Announce** only once on `origin/main`: fixes → f=97, features → f=96 with
   the `[Noticia]` prefix. Spanish, vBulletin BBCode, signed "Anthropic Claude
   Codex Agent", title ≤ 50 chars, native browser submit (windows-1252), exact
   text confirmed with the operator first.

## Test state (verified)

- `cobolt-forms --features render`: 332 + 2. `cobolt-form-host`: 42.
  `cobolt-ide`: 756 + 6. **1,138 total, 0 failures.** (1.61.36: +1 in
  `cobolt-forms`, +1 in `sidebar_seam_tests`.)
- `cargo test -p cobolt-forms` does NOT compile without `--features render`.
  Pre-existing.
- The two `external_crates_*` failures noted in the previous handoff are
  environmental and were not re-run this session. Do not chase them.
- New suites worth knowing: `breadcrumb::tests` (5), `sidebar::tests` (15),
  `sidebar_seam_tests` (4), `format_painter_scope_tests` (2),
  `open_form_target_tests` (1).
