<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — spec 049 (application shell & the `super` receiver) + session state

- **Date:** 2026-08-09, evening. **Branch:** `features` (HEAD = `5e66ade`,
  1.61.13, level with `main`). **ALL spec-049 work is UNCOMMITTED working-tree
  changes on top of it.** `main` == `origin/main` == `fixes` == `5e66ade`.
  Local `features` is 2 commits ahead of `origin/features` (just main's
  1.61.12/1.61.13 arriving via a fast-forward — nothing new).
- **Version in tree:** 1.61.14 (`crates/cobolt-ide/src/version.rs`), CHANGELOG
  entry written (Added — spec 049).
- **Nothing committed, nothing pushed, nothing announced.**

## How this session went

The operator pasted a claude.ai share link (a design conversation about
embedding forms in a main form), we explored the design against the real
codebase, then ran the full spec-driven chain: `/specify` → `/plan` →
`/tasks` → `/implement`. **31 of 32 tasks are complete; T27 carries one
explicitly blocked piece** (below). `tasks.md` has a per-task outcome note
with the real numbers — **read them, they record several decisions and
corrections that are not obvious from the code.**

## What spec 049 is

An **opt-in application shell** for enterprise apps: ONE window with a
MenuPane, a breadcrumb and a ContentPane, instead of a window per form. Plus
the **`super`** receiver — the form that loaded or opened this one.

**The switch is a new `SideMenu` control on the main form.** A `MenuBar`
deliberately does NOT trigger it: the operator chose a distinct control type
precisely so no existing project can become a shell app by accident (spec
R3/R45). That choice cost more (a full new `ControlType`) and was made
knowingly.

## Architecture — the parts that are not obvious from the diff

### The three findings that shaped the plan
1. **`render_form(ui, …)` takes its origin from `ui.min_rect().min`** — so
   hosting a form in a pane is free of coordinate translation.
2. **`Backdrop.window_size`** already stretches a form background across a
   host area bigger than the form → R12/R13 were a parameter, not new paint
   code.
3. **`HostAction::SpawnWindow` is a STUB** (`host.rs`) — 037's T16 never
   landed, so no child-form interpreter has ever existed. This is the root of
   the one blocked item.

### `me` / `super` (cobolt-runtime)
- `member_root_key()` canonicalises a `ME` root to `self_form_object` at the
  ONE point every member-chain consumer sees it (`lower_member_chain`), plus
  `exec_method` for the statement path. Before 049, `me::Title` **wrote a
  phantom "ME" control** and `me::Width` read empty — the form had no registry
  entry at all. `build_object_seed` now seeds the FORM itself (15 universal
  props). That seed list and `resolver.rs::UNIVERSAL_FORM_PROPS` **must stay in
  sync** — both carry a doc comment saying so.
- `super` is a **pre-bound windowHandler** (`super_window_handle` +
  `set_super_form`), routed through the shared `window_method_roundtrip`.
- The parent's PROPERTY VALUES live in the **supervisor**, per handle
  (`HandleInfo.props`), fed by `FormRequest::PublishFormProps` (seed time +
  every own-form write via `publish_own_form_prop`) and read/written by the
  `GETPROPERTY`/`SETPROPERTY` handle methods. `HostAction::SetFormProperty`
  forwards a cross-form write back into the target's interpreter over the
  existing FullScreen-echo route, so the parent's own `me::X` stays coherent.
- `super::super::…` walks one `SUPERHANDLE` supervisor round-trip per leading
  `super` SEGMENT — the live `HandleInfo.caller` edge, so a closed ancestor
  fails honestly. `drain_closed_handles` NULLs `super` when its opener closes
  (R46), and `resolve_super_target` drains FIRST so the error is always the
  standard "super is NULL", never a stale-handle supervisor error.
- **Known limit, documented:** `super` exposes single form properties only —
  `COMPUTE` into `super::X` (the `__PROP$` property-shadow route) is not wired.

### The shell (`crates/cobolt-form-host/src/shell.rs`, ~1,950 lines, NEW)
- `Shell::show` / `show_with_host` lay out three regions on the **root `Ui`**
  using `egui::Panel::left/top(...).show(root_ui, …)` — this workspace's egui
  0.36 is **Ui-hosted**, not the ctx-hosted `SidePanel`/`TopBottomPanel` the
  plan assumed.
- **Do not wrap the host in a second ScrollArea.** `FormHost::ui_impl` already
  renders through its own `CentralPanel` + `ScrollArea`; nested both-axis areas
  fight over the wheel. With a form loaded, the host's scroll IS the
  ContentPane scroll.
- `Surface::{Window, Pane}` on `FormHost`: Pane zeroes the fx specs at
  construction (R18) and **all 17 viewport commands funnel through one
  `viewport_cmd` helper** that no-ops in Pane (R42). The two viewport-READ echo
  blocks (fullscreen/minimized) are also gated — in Pane they'd observe the
  SHELL's window and fire bogus form events.
- **R41 backdrop:** in Pane, `ui_impl` paints the REAL backdrop pane-sized
  BEFORE the ScrollArea and hands `render_form` a fully transparent one — no
  engine API change (there are 20 `RenderInput` construction sites), nothing
  painted twice, background fixed while the form scrolls.
- **R43 transparency:** the shell window is created `with_transparent(true)`
  and `clear_color` is zeroed, so chrome must paint EXPLICITLY — `CHROME_FILL`
  is painted by the MenuPane (even with no custom background: skipping it would
  be a hole) and the breadcrumb. Only the pane's form backdrop may carry alpha.
- `NavChain` + the `Resident` trait: being in the chain (or parked) IS
  residency. `ChannelResident` turns lifecycle into `FormEvent`s so the
  generated program's own loop runs `onDeactivate`/`onDestroy`.

### Where the R17 load-path check lives (three layers)
Menus never reach `cobolt-semantic`, so the plan's file list was incomplete:
1. `AnalyzeOptions.form_formats` + `FormLoadFormat` + resolver
   `check_open_form_target` (both invoke spellings, literal targets only);
2. `menu::validate_menu_targets` in cobolt-forms (menu items carry
   `open-form:<NAME>`);
3. a `build_core` pre-scan in cobolt-compiler that builds the map, validates
   every SideMenu/MenuBar sidecar, fails the build naming form + item, and
   feeds the map into `analyze_with`.

## Test state (verified this session)

- **Full workspace sweep, `--no-fail-fast`, every result line collected:
  102 suites, 1,825 passed, 2 failed.**
- The **2 failures are pre-existing and environmental**:
  `test_external_crates_e2e::external_crates_alias_build_and_run` and
  `…::external_crates_build_run_manifest_and_determinism` — their nested
  `cargo build` cannot compile `libsqlite3-sys` in this sandboxed shell.
  **Verified by stash-rerun during T6: identical failure with all 049 changes
  stashed.**
- KB freshness green after `build_chunked_kb` (993 records / 5 docs). i18n
  green (11 new keys ×6 languages).
- New test files: `crates/cobolt-runtime/tests/test_form_receiver.rs`,
  `test_super_receiver.rs`, `crates/cobolt-semantic/tests/test_form_load_path.rs`,
  `test_universal_form_surface.rs`. Shell tests live **in `src/shell.rs`
  (`mod tests`)** — `Shell`'s state is private, same reason as 042's parity
  suite.
- **`cargo test -p cobolt-forms` does NOT compile on its own** —
  `model.rs` calls `crate::paint::contrast_ratio`, gated behind the `render`
  feature. Always `--features render`. Pre-existing; noted at the top of
  `tasks.md`.

## What remains (in order)

1. **Operator manual pass** — the five visual checks are written out in
   `tasks.md` T32. Rebuild `--release` first (the operator's running binary is
   otherwise stale). Nothing else blocks a commit.
2. **Split the guide edit before committing.** `docs/developers-guide-en.md`
   carries BOTH: (a) a **fix** — the old `EXTERNAL` bullet claimed two form
   modules share one copy of `WS-COUNTER`, which was never true in this code;
   and (b) the **feature** — the new §22 shell section. Golden rule #5 says
   these cannot ride one commit. The fix half also forward-references a
   *qualified `EXTERNAL`* section that does not exist yet (see below), so
   landing it alone needs a small reword.
3. **Commit as ONE feature commit on `features`** (1.61.14, spec 049).
   `git add` the untracked files: `crates/cobolt-form-host/src/shell.rs`, the
   four new test files, and `specs/049-application-shell/`.
4. When the operator asks: merge → push (**window rule: never 09:00–18:00
   São Paulo, Mon–Fri**) → announce on **f=96** (features) with the
   `[Noticia]` prefix, Spanish, vBulletin BBCode, signed "Anthropic Claude
   Codex Agent", title ≤ 50 chars, native browser submit (windows-1252),
   exact text confirmed with the operator first. The guide fix half, if
   landed separately, goes to **f=97**.

## The one blocked piece

**Loading a SECOND form into the ContentPane** (`open-form:` menu items) is
NOT implemented. It needs another interpreter + its generated program hosted
in-process — the same open work as **spec 037's T16** child-window hosting.
`run-form` receives one cfrm/cbl pair and a compiled binary embeds one
`PROGRAM_AST`, so no glue can host a second form's COBOL today, in either
mode. `run_shell` prints an honest notice at run time instead of pretending.

Consequences: **AC9's WORKING-STORAGE half** is unverified (the chain half is
machine-verified), and the compiled-app template has no shell branch yet —
add one when T16 lands. Everything else about the shell, navigation and
`super` is implemented and tested.

## Unresolved spec questions (operator's call)

Recorded in `spec.md` §7; none block the code as it stands:
- **Q5** — should new forms default to `Both` instead of `Standalone`?
  (Standalone was chosen to keep existing projects unchanged.)
- **Q6** — designer preview for a `Both` form: preview both framings, or pick
  one per form?
- **Q8** — should the MenuPane width be user-draggable? (Currently a property
  with Open/Collapsed values, not draggable.)
- **Q9** — may a subsystem restyle the MenuPane? (Currently no — stable
  chrome, the deliberate opposite of the ContentPane.)
- The **qualified `EXTERNAL`** design (`data-1 OF form-1`) the operator
  settled earlier in the session is **documented but NOT implemented** — it is
  its own spec, with the guide's availability caveat already in place.

## Gotchas for whoever continues

- **The shell's `Shell` methods take `&mut self` inside panel closures** —
  `paint_menu_background` and `draw_mounted_menus` are called from within
  `Panel::show(root_ui, |ui| …)`; the borrow works because the closure
  captures `self` disjointly. Adding another `&mut self` call there may need
  the same care.
- **Interpreters hold a clone of the supervisor sender.** Tests must `drop`
  the interpreter before `host.join()` or the host loop never sees the
  disconnect — the first version of `test_super_receiver.rs` deadlocked on
  exactly that (600 s timeout).
- **egui headless frames:** always `full.textures_delta.clear()` after
  `ctx.run_ui`, or epaint panics on drop.
- **A form's see-through-ness is the `Transparency` PROPERTY (0-100)**, not
  the background colour's alpha byte — `backdrop_color` ignores the alpha byte
  and maps pure black to the default navy. A test asserting `00000000` will
  fail confusingly.
- **The lexer uppercases member names**, so a diagnostic about `super::Widht`
  reads `super::WIDHT`.
- **save → load → save is NOT byte-identical for any `.cfrm`** — loading
  normalises event bodies (adds the ENVIRONMENT/DATA/PROCEDURE DIVISION
  scaffold) and drops empty properties. Predates 049; don't write a test that
  assumes otherwise (the first T2 test did, and failed correctly).
- **`form_property_lists_agree`** is a real guard: adding a form-prop key to
  `designer.rs::FORM_PROP_KEYS` also requires adding it to
  `agent.rs::form_property_valid`. It caught exactly that this session.
- **The shell's menu-item labels and the ☰ toggle are symbol/data**, not `Tr`
  literals — when the shell gains real chrome text, it needs new `Tr` keys in
  all six languages.
- The shell's `run_shell` currently opens at a fixed 1100×700; the main form's
  designed size is NOT used for the shell window (it sizes the ContentPane
  occupant). Deliberate, but worth an operator opinion.
