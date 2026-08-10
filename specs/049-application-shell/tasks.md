<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Application shell, in-pane navigation & the `super` receiver

- **Status:** done (31/32 complete; T27 carries one explicitly blocked piece — multi-form pane hosting, gated on spec 037 T16)
- **Plan:** ./plan.md   **Date:** 2026-08-09

Ordered so the workspace stays green after every task. T1–T6 are additive model
work with no behaviour change; T7–T11 build the language surface; T12–T21 build
the shell; T22–T28 wire navigation and lifetime; T29–T32 harden and finish.

New `Tr` keys are added by the task that introduces the string; **T31** verifies
every key exists in all six languages.

> **Verification note (found during T1).** `cargo test -p cobolt-forms` does
> **not** compile on its own: `model.rs` calls `crate::paint::contrast_ratio`,
> which is gated behind the `render` feature. Every `cobolt-forms` verification
> below therefore reads `cargo test -p cobolt-forms --features render` — the same
> feature `cobolt-ide` already enables on its dependency. Pre-existing, unrelated
> to this spec.

## Model foundations

- [x] **T1 — `FormFormat` on `Form`** (R1) — *done 2026-08-09; 302 tests green,
      0 failed. `form_format_round_trips_049` reports: 3 values covered —
      Standalone (attr written: false), Embedded (true), Both (true); a file with
      no `form-format` attribute loads as Standalone.*
  - Files: `crates/cobolt-forms/src/model.rs`, `crates/cobolt-forms/src/xml.rs`
  - Do: add `FormFormat { Standalone, Embedded, Both }` and `Form.form_format`
    in the spec-037 window block; `.cfrm` round-trip with an absent attribute
    defaulting to `Standalone`.
  - Verify: `cargo test -p cobolt-forms` green; a new round-trip test covers all
    three values **and** a legacy `.cfrm` with no attribute loading as
    `Standalone`.

- [x] **T2 — `SideMenu` control type** (R45) — *done 2026-08-09; cobolt-forms
      303 green, cobolt-ide 752 green, 0 failed. `side_menu_control_round_trips_049`
      reports: SideMenu round-trips as type=SideMenu 200x400; a MenuBar form is
      unchanged (type=MenuBar, form_format=Standalone, no SideMenu markup, no
      form-format attribute). Note: the byte-identical check in this task's
      original wording is not achievable — loading normalises event bodies and
      drops empty properties, so save→load→save differs for **every** form in
      this format, predating 049. The test asserts type/markup stability instead.*
  - Files: `crates/cobolt-forms/src/model.rs`, `.../paint.rs`, `.../render.rs`,
    `crates/cobolt-ide/src/panels/toolbox.rs`, `.../panels/designer.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: add the `ControlType::SideMenu` variant with its `as_str`/`from_str`
    mapping, default size, property set and event set; draw it design-time and
    at runtime; add the toolbox entry; let the existing `SetMenuDefinition`
    editor accept it (the definition is keyed by control id, so no new storage).
    Leave `MenuBar` untouched.
  - Verify: `cargo test -p cobolt-forms && cargo test -p cobolt-ide` green; a
    round-trip test places a SideMenu with a `MenuDefinition` and reloads it; an
    existing form carrying a `MenuBar` is byte-identical after load/save.

- [x] **T3 — `PreservePreviousForm` on `MenuItem`** (R24) — *done 2026-08-09;
      cobolt-forms 304 green, 0 failed. `preserve_previous_form_defaults_false_049`
      reports: legacy menu (no key) => false; set on 1 item, key written 1 time
      across 1 top-level / 3 child items, survives load = true.*
  - Files: `crates/cobolt-forms/src/menu.rs`
  - Do: add the boolean with `#[serde(default)]`, default `false`.
  - Verify: `cargo test -p cobolt-forms` green; a menu JSON written before this
    change still deserialises and reports `false`.

- [x] **T4 — MenuPane background properties** (R39, model half) — *done
      2026-08-09; cobolt-forms 305 green, 0 failed, workspace check clean.
      `menu_pane_background_round_trips_049` reports: 8/8 fields round-trip
      (color, gradient start/end/direction, transparency, image, mode); a form
      without the group writes no element and loads `None`. Model type is
      `MenuPaneBackground` — a serializable mirror of `render::Backdrop`, which
      cannot be persisted directly (it holds a resolved egui `TextureId`).*
  - Files: `crates/cobolt-forms/src/model.rs`, `.../xml.rs`
  - Do: persist a `Backdrop`-shaped group on the main form for the MenuPane;
    absent ⇒ the shell's default chrome fill. Reuse the existing `Backdrop`
    fields rather than inventing a second background dialect.
  - Verify: `cargo test -p cobolt-forms` green; round-trip test for colour,
    gradient, image + `BgImageMode` and transparency.

- [x] **T5 — Inspector rows and inapplicable states** (R1, R5, R36, R39) —
      *done 2026-08-09; cobolt-ide 754 green, 0 failed, i18n suite green (11 new
      Tr keys ×6 languages). `shell_prop_tests` reports: FormFormat 3 transitions
      + case-insensitive key + main form pinned Standalone (R5); MenuPane
      materialise + 8 field keys round-trip + clear to None. The
      `form_property_lists_agree` guard caught that `agent.rs`'s validator
      vocabulary also had to gain the 10 new keys — both lists now agree. Also
      added the R24 checkbox to the menu editor (open-form items only). Visual
      confirmation of the greyed rows rides T32's manual pass.*
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: add rows for `FormFormat`, the MenuPane background group and
    `PreservePreviousForm`; make the main form's `FormFormat` read-only (R5);
    present the window-only properties as inapplicable while `FormFormat` is
    `Embedded` (R36), reusing the treatment 037 R9 gives `TaskbarIcon`.
  - Verify: `cargo test -p cobolt-ide` green; launch the IDE and confirm the
    main form's FormFormat row is not editable and an Embedded form greys
    TitleVisible / WindowState / CanMinimize / CanMaximize / FullScreen.

- [x] **T6 — Load-path build check** (R17) — *done 2026-08-09. Landed as three
      layers (menus never reach cobolt-semantic, so the plan's file list was
      incomplete): (1) `AnalyzeOptions.form_formats` + `FormLoadFormat` +
      resolver `check_open_form_target` — both comma and space spellings,
      literal targets only; (2) `menu::validate_menu_targets` +
      `open_form_target` in cobolt-forms (menu items carry `open-form:<NAME>`
      actions); (3) `build_core` pre-scan in cobolt-compiler: builds the map
      (keyed by .cfrm stem + form name, uppercase), validates every
      SideMenu/MenuBar sidecar menu, fails the build naming form + item, and
      passes the map into `analyze_with`. IDE Check publication rides T27 as
      planned. Tests: `test_form_load_path.rs` 5/5 (Embedded target errors in
      both spellings naming form+format+call; Both and Standalone pass; no map
      ⇒ silent; dynamic + unknown skipped) and
      `validate_menu_targets_finds_standalone_targets_049` (5 items: 1 nested
      violation, Embedded/Both pass, unknown + non-open-form skipped). Suites:
      cobolt-semantic 46 green; cobolt-forms 306 green; cobolt-compiler lib 64
      green. **Expected failures (pre-existing, verified by stash-rerun):** the
      2 `test_external_crates_e2e` tests fail in this sandboxed shell — their
      nested `cargo build` cannot compile `libsqlite3-sys`; identical failure
      without the 049 changes.*

## Language surface

- [x] **T7 — Form as a member-chain receiver root** (R30) — *done 2026-08-09.
      Mechanism (from the pre-implementation analysis): the form's object entry
      already travels as control-shaped state keyed by the FORM OBJECT NAME —
      the host's FormState mirror and FullScreen echo both match that key — so
      the fix is (1) `member_root_key`: canonicalise a `ME` root to
      `self_form_object` at the ONE point every consumer receives it
      (`lower_member_chain`), plus the `exec_method` statement path (`INVOKE ME
      "SetProperty"` wrote a phantom "ME" object before); (2) seed the FORM
      ITSELF as an object in `build_object_seed` (15 universal-surface props:
      Title/Width/Height/X/Y/WindowState/FullScreen/TitleVisible/CanMinimize/
      CanMaximize/FormState=Ready/FormFormat/BackgroundColor/Transparency/Name)
      — before this the form had NO registry entry, which is why `me::Width`
      read empty; (3) reserve `me`/`super` as control ids in
      `is_valid_control_id`. Tests: `test_form_receiver.rs` 4/4 — designed
      reads (Width=640, Title), assignment visible through BOTH `me::` and
      `<FORM-NAME>::` with StateUpdates keyed by the form name (0 keyed "ME"),
      the statement path landing on the form object, and console-mode (no form
      host) staying inert. Suites: cobolt-runtime 289 green, cobolt-form-host
      18 green (2 seeding tests updated for the new form-first seed shape —
      they asserted the pre-049 controls-only shape positionally), cobolt-ide
      754 green.*

- [x] **T8 — `super` binding and INVOKE dispatch** (R28, R29) — *done
      2026-08-09. As designed, `super` is a pre-bound handle
      (`super_window_handle` + `set_super_form`, `is_super` beside `is_me`),
      routed through the shared `window_method_roundtrip`. What the analysis
      forced beyond the task text: the parent's PROPERTY values had to live
      somewhere reachable — 037's supervisor was method-only with no property
      store, and no child interpreter even exists (`SpawnWindow` is a stub, 037
      T16 open). So the supervisor gained a per-handle published property
      surface: `HandleInfo.props`, `FormRequest::PublishFormProps`
      (fire-and-forget, sent at seed time and on every own-form write via
      `publish_own_form_prop`), `GETPROPERTY`/`SETPROPERTY` handle methods, and
      `HostAction::SetFormProperty` — which the host forwards into the target
      form's interpreter over the existing FullScreen-echo route, keeping the
      parent's own `me::X` coherent after a child's `super::X =` write.
      Reads/writes of `super::X` are supervisor round-trips (write-through, so
      a NULL parent errors at the write). Known limit, documented: `super`
      exposes single form properties only — `COMPUTE` into `super::X` (the
      property-shadow route) is not wired. Tests: `test_super_receiver.rs` 2/2
      — AC18 headless (child W1 reads opener's Title='Root Title', rewrites it,
      reads 'From Child' back, minimizes the parent; HostActions verified
      targeting W0), and unbound `super` raising 'super is NULL' on read, write
      and method (3/3). Suites: cobolt-runtime 291 green, cobolt-form-host 18
      green.*

- [x] **T9 — `super` as a chain segment, and NULL at the main form** (R31, R32)
      — *done 2026-08-09. `resolve_super_target` walks one `SUPERHANDLE`
      supervisor round-trip per leading `super` SEGMENT (the live caller edge,
      so a closed ancestor fails honestly); reads, writes and chained window
      methods (`super::super::"Close"()`) all route through it; a missing link
      anywhere raises the standard 'super is NULL' error. Test
      `super_chain_walks_one_loader_per_step` (AC14 headless): chain
      A(W0)→B(W1)→C(W2) — super::Title='B Title', super::super::Title='A
      Title', the write changed B (re-read + HostAction on W1), one step past
      the root errors. cobolt-runtime 293 green.*

- [x] **T10 — Async child's `super` goes NULL** (R46) — *done 2026-08-09. The
      R24 drain path (`drain_closed_handles`) now also NULLs
      `super_window_handle` when the broadcast names it, and
      `resolve_super_target` drains FIRST so the raised error is the standard
      NULL one, never a stale-handle supervisor error. The supervisor side
      needed nothing: 037 R26 already clears the caller edge on close. Test
      `async_child_super_goes_null_when_the_opener_closes` (AC26 headless):
      pre-close read works ('B Alive'), Close(W1) broadcast, the SAME read
      raises 'super is NULL'. One test-harness lesson recorded: an interpreter
      holds a clone of the supervisor sender, so tests must drop interpreters
      before `host.join()` — the first version deadlocked on exactly that.
      cobolt-runtime 293 green.*

- [x] **T11 — Universal-surface build check** (R33, R34) — *done 2026-08-09.
      Landed in `resolver.rs` (not type_checker.rs — the Member arm lives
      there): `UNIVERSAL_FORM_PROPS` (15 entries, exactly the T7 form-entry
      seed; the const doc records the coupling) +
      `check_form_receiver_property`, which flattens a chain, fires only on
      literal `ME`/`SUPER` roots, strips leading `super` segments, and requires
      the one remaining BARE property to be in the table — parens (methods)
      pass through to runtime dispatch (R34), control-rooted chains untouched.
      Tests: `test_universal_form_surface.rs` 3/3 — misspelt property rejected
      at super depth 1, depth 3 and on `me` (error lists the full surface;
      note: the lexer uppercases member names, so diagnostics say `WIDHT`);
      6 valid property forms + 3 method forms pass. Suites: cobolt-semantic 49
      green, cobolt-codegen 34 green, workspace check 0 errors.*

## The shell surface

- [x] **T12 — `FormHost` embedded surface mode** (R18, R42) — *done 2026-08-09.
      `Surface { Window, Pane }` on `FormHostConfig`/`FormHost`. Two mechanisms
      instead of scattered guards: (1) Pane zeroes the fx specs at construction
      (R18) so every existing fx gate stays untouched; (2) ALL 17 viewport
      commands funnel through one `viewport_cmd` helper that no-ops in Pane
      (R42) — plus the two viewport-READ echo blocks (fullscreen/minimized)
      gated, since in Pane they'd observe the SHELL's window and fire bogus
      form events. Both glue sites and the compiled-app template pass
      `Surface::Window`, so classic mode is bit-identical. Test
      `pane_surface_plays_no_effects_and_issues_no_viewport_commands_049`
      (AC8's pane half; the Window half is the pre-existing entrance test):
      fade:3000 inert on Pane, animations on frame 1, 0 window commands across
      2 frames incl. an explicit SetWindowState (egui's own SetTheme
      bookkeeping excluded). cobolt-form-host 19 green.*

- [x] **T13 — Shell skeleton: three regions, fixed MenuPane, independent
      scrolls** (R4, R37, R38, R40) — *done 2026-08-09. New
      `shell.rs`: `Shell { menu_open_width: 220, menu_collapsed_width: 48,
      collapsed }` + `Shell::show(root_ui, menu, breadcrumb, content) ->
      ShellLayout`. API note: this workspace's egui 0.36 uses Ui-hosted
      `egui::Panel::left/top(...).show(root_ui, …)` (NOT ctx-hosted
      SidePanel/TopBottomPanel as the plan assumed) — same surface
      `FormHost::ui` renders on, which is exactly what T14 needs. Each pane has
      its own `ScrollArea` (`shell-menu-scroll` / `shell-content-scroll`), so
      R37/R40 are structural. Tests (headless frames, real wheel events):
      AC20 — 1000→1400px: MenuPane 220px in both frames, ContentPane +400px;
      AC19 — content wheel: content 64px/menu 0px, menu wheel: menu 73px/
      content unchanged at 64px; R8 — Open 220→Collapsed 48, ContentPane
      gained exactly 172px. cobolt-form-host 22 green.*

- [x] **T14 — ContentPane hosts the form** (R10, R11) — *done 2026-08-09.
      `Shell::show_with_host(root_ui, menu, breadcrumb, host)` +
      `FormHost::pane_frame` + `designed_size()`. One structural decision the
      code forced: `ui_impl` already renders through its OWN
      `CentralPanel` + `ScrollArea` on the passed `Ui`, so the shell must NOT
      wrap the host in a second scroll area (nested both-axis areas fight over
      the wheel) — the host's scroll IS the ContentPane scroll when a form is
      loaded (R40); the plain `show` keeps the shell's own scroll for formless
      content. Test `form_travels_with_the_pane_edge_at_designed_size` (AC4):
      collapse moved the ContentPane origin exactly 172px left (the rail
      delta) with a Pane-surface host mounted; designed size stayed 300x200;
      the entrance spec stayed inert. cobolt-form-host 23 green.*

- [x] **T15 — ContentPane backdrop, painted outside the scroll** (R12, R13,
      R41) — *done 2026-08-09. Zero engine-API change: instead of a
      `RenderInput` opt-out (20 construction sites), the Pane branch of
      `ui_impl` paints the REAL backdrop pane-sized before the ScrollArea and
      hands `render_form` a fully transparent one — nothing painted twice, and
      the panel fill goes transparent in Pane for the same reason. Added
      observability fields (`pane_backdrop_rect`, `pane_backdrop_fill`,
      `content_scroll`) for the tests and the T29 parity suite. Test
      `backdrop_stays_pane_fixed_while_the_form_scrolls` (AC21 + AC5's
      pane-rect half): 3000x3000 form → backdrop rect 780x672 (pane-sized, not
      form-sized), scrolled 80px with the rect unmoved; 300x200 form →
      backdrop covers the full pane. cobolt-form-host 24 green.*

- [x] **T16 — MenuPane background** (R39, paint half) — *done 2026-08-09.
      `Shell.menu_background: Option<MenuPaneBackground>` painted through the
      SAME `paint_backdrop` as every form background (one dialect); images are
      resolved by the hosting glue (the engine has no texture cache), colour +
      gradient paint now. Test `menu_background_is_immune_to_loaded_forms`
      (AC22): menu fill identical across 3 loads with clashing form
      backgrounds (red/green/same-as-menu).*

- [x] **T17 — Breadcrumb as shell chrome** (R14) — *done 2026-08-09.
      Structural: the breadcrumb is its own `Panel::top` with an explicit
      chrome fill, laid out before the ContentPane. AC6's headless half is in
      the same test as AC22: the form's pane-backdrop rect is disjoint from
      both the breadcrumb strip and the MenuPane, whatever background the form
      has — the visual half rides T32's manual pass.*

- [x] **T18 — Transparency reaches the desktop** (R43) — *done 2026-08-09.
      The paint contract: `CHROME_FILL` (opaque) is painted EXPLICITLY by the
      MenuPane (also when no custom background is set — previously a skip,
      which in a transparent window would be a hole) and by the breadcrumb;
      only the ContentPane's form backdrop may carry alpha. One engine rule
      surfaced by the test: a form's see-through-ness is the Transparency
      PROPERTY (0-100) — `backdrop_color` ignores the colour's alpha byte and
      maps pure black to the default navy. Test
      `transparent_form_reaches_the_desktop_through_the_pane_only` (AC23):
      Transparency=100 form → pane fill alpha 0, MenuPane alpha 255,
      breadcrumb chrome alpha 255. The `with_transparent` window creation
      itself lands with the T27 glue. cobolt-form-host 26 green.*

- [x] **T19 — MenuPane Open/Collapsed and its persistence** (R8, R9) — *done
      2026-08-09. Std-only persistence at
      `<data_dir>/cobolt/apps/<app>/shell.toml` (`shell_state_path` sanitises
      the app name; `save_collapsed_to`/`load_collapsed_from` are
      path-parameterised for tests). Widths landed with T13. Test
      `menu_pane_state_persists_across_restarts`: collapsed=true survives a
      simulated restart, absent file ⇒ Open, path sanitised. The Tr keys for
      the visual toggle ride T27's IDE glue.*
  - Files: `crates/cobolt-form-host/src/shell.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: two widths for the two states, Collapsed rendering a narrow icon rail
    that keeps root items reachable; persist the state per application at
    `<data_dir>/<app>/shell.toml`, following the `ui_prefs.rs` convention but
    keyed per app so shipped binaries each keep their own.
  - Verify: `cargo test -p cobolt-form-host` green; **AC3 (state half)** —
    collapsing, restarting the host and reading the state back returns
    Collapsed; root items remain hit-testable while collapsed.

- [x] **T20 — Menu mounting: root and contextual slots** (R6, R7) — *done
      2026-08-09. `MenuSlot { Root, Contextual }`, `MountedMenu`, `MenuClick`
      (carries slot, item id, action verbatim and the R25 preserve flag);
      `mount_root_menu` (first mount wins, R6), `mount_contextual_menu`
      (wholesale, R7), `take_menu_clicks` (drains once); `draw_mounted_menus`
      draws both slots in the pane scroll — Collapsed keeps ROOT items
      reachable as single-glyph buttons (R8). Test
      `menu_slots_mount_root_once_and_swap_contextual_wholesale` (AC3 mount
      half): impostor root mount ignored, CRM→HR wholesale swap with root
      untouched, and a REAL pointer click on 'crm' drained as
      Root/open-form:CRM/preserve=false.*
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: mount the main form's SideMenu into the root slot once and never replace
    it; replace the contextual slot **wholesale** when a menu-carrying form
    becomes current.
  - Verify: `cargo test -p cobolt-form-host` green; **AC3 (mount half)** —
    entering a subsystem replaces the contextual slot and leaves the root slot
    identical.

- [x] **T21 — Menu objects and `Open`/`Collapse` from COBOL** (R44) — *done
      2026-08-09. The chained-receiver branch resolves
      `super::<menu-id>::Collapse()`/`Open()` (bare-statement chains parse as
      `InvokeExpr`) → supervisor `SETMENUPANECOLLAPSED` → pane-wide state
      (Q10) + `HostAction::SetMenuPaneCollapsed` for the shell to apply and
      persist (R9); a classic window host ignores it (no pane). Test
      `menu_open_collapse_drive_the_pane_through_super` (AC24 COBOL half):
      Collapse/Open/Collapse produced pane actions [true, false, true] in
      order. The shell-side apply+persist and restart survival land with T27's
      glue. runtime 294 / form-host 28 green, workspace clean.*
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    `crates/cobolt-forms/src/menu.rs`, `crates/cobolt-form-host/src/shell.rs`
  - Do: let a form receiver yield a **menu object** by id, carrying `Open` and
    `Collapse`, so `super::<menu-id>::Collapse()` resolves; the methods act on
    the whole pane (spec Q10) and the resulting state persists under R9.
  - Verify: `cargo test -p cobolt-runtime && cargo test -p cobolt-form-host`
    green; **AC24** — `Collapse()` collapses and `Open()` restores, and the state
    survives a host restart.

## Navigation & lifetime

- [x] **T22 — Navigation chain and residency** (R19, R20, R21) — *done
      2026-08-09. `NavChain`/`NavEntry` + the `Resident` trait (deactivate/
      destroy — being in the chain or parked IS residency) + `draw_breadcrumb`
      (segments in chain order, click → index). Test
      `chain_keeps_ancestors_resident_and_orders_the_breadcrumb` (AC9's chain
      half): 4 segments in order, 3 deactivates, 0 destroys, resident_count 4,
      real click resolves to index 0. The WORKING-STORAGE half of AC9 needs a
      real spawned child interpreter — see T27's blocked note.*
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: `NavChain: Vec<NavEntry>`, each entry owning a `FormHost` and its
    interpreter thread; the breadcrumb renders one segment per entry in order.
  - Verify: `cargo test -p cobolt-form-host` green; **AC9** — with
    main → CRM → Sales → Customer List the breadcrumb shows four segments and
    CRM's WORKING-STORAGE still holds a value written before Sales was entered.

- [x] **T23 — `onDeactivate` and `onDestroy`** (R26, R27) — *done
      2026-08-09. Discovery: both events ALREADY exist in the designable form
      catalogue and the generated loop dispatches ANY bound form event — so no
      codegen surgery, just the 049 semantics documented on the catalogue and
      the delivery vehicle: `ChannelResident` (lifecycle → FormEvents on the
      form's own channel). Tests:
      `lifecycle_events_generate_handlers_and_dispatch_049` (codegen: 2 WHENs +
      2 handler programs when bound, none when unbound) and
      `lifecycle_events_flow_through_the_channel_resident` (AC13: ancestor
      [onDeactivate]; non-preserved sibling [onDestroy]; preserved sibling
      [onDeactivate]; popped ancestor [onDeactivate, onDestroy]).*
  - Files: `crates/cobolt-codegen/src/lib.rs`,
    `crates/cobolt-form-host/src/shell.rs`, `crates/cobolt-forms/src/model.rs`
  - Do: add the two form events, generate their nested-program handlers beside
    the existing event paragraphs, and dispatch them through the `FormEvent`
    channel. Keep the generated banner and the regenerate-on-action contract.
  - Verify: `cargo test -p cobolt-codegen && cargo test -p cobolt-form-host`
    green; **AC13** — a form that stays resident fires `onDeactivate` and never
    `onDestroy`; a destroyed form fires `onDestroy` and no second
    `onDeactivate`.

- [x] **T24 — Breadcrumb click unwinds the chain** (R22) — *done
      2026-08-09. `breadcrumb_pop(shell, chain, index, menu_of)`: pop_to
      destroys deepest-first, the target's menu remounts into the contextual
      slot (root never moves), index 0 clears the slot. Test
      `breadcrumb_click_unwinds_and_remounts` (AC10): destroyed [CUST-LIST,
      SALES] in order, CRM's 2-item menu remounted, CRM intact.*
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: truncate the chain at the clicked segment, firing `onDestroy` in reverse
    order before dropping each entry; remount that form's menu and display its
    body.
  - Verify: `cargo test -p cobolt-form-host` green; **AC10** — clicking CRM
    destroys Customer List then Sales in that order, remounts CRM's menu,
    displays CRM's body, and CRM's storage is intact.

- [x] **T25 — Root-slot switch unwinds first** (R23) — *done 2026-08-09.
      `root_switch`: `unwind_to_root` (chain deepest-first + the parking lot —
      a preserved sibling of a dead subsystem has no way back) then push +
      mount. Test `root_switch_unwinds_everything_first` (AC11): destroyed
      [LEADS, CRM, CUST-LIST] before HR pushed; MAIN never destroyed.*
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: selecting a different subsystem from the root slot unwinds to index 0,
    then pushes the new subsystem.
  - Verify: `cargo test -p cobolt-form-host` green; **AC11** — every form below
    the main form receives `onDestroy` before the new subsystem is pushed.

- [x] **T26 — `PreservePreviousForm` behaviour** (R25) — *done 2026-08-09.
      `replace_top`: preserve=false ⇒ destroy; true ⇒ deactivate + park; a
      return revives the SAME parked resident (instant, storage intact) and
      reports it. Test `preserve_previous_form_parks_and_revives` (AC12):
      false ⇒ destroy + fresh entry on return; true ⇒ parked (resident_count
      4), revived same box, 0 destroys, exactly 1 deactivate.*
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: on a sibling load, destroy the outgoing form when the clicked item's
    `PreservePreviousForm` is false, keep it resident when true.
  - Verify: `cargo test -p cobolt-form-host` green; **AC12** — false fires
    `onDestroy` and storage re-initialises on return; true fires none and the
    earlier values are still there.

- [x] **T27 — Load paths and shell activation** (R2, R3, R5, R15, R16) —
      *done 2026-08-09, with one honestly-blocked piece. Landed:
      `Form::has_side_menu()`/`side_menu_control_id()` (a MenuBar deliberately
      does NOT count — R3/R45); the run-form glue branches to the new
      `shell::run_shell` when the form carries a SideMenu, classic `run`
      otherwise (byte-identical path); `run_shell` builds the ONE transparent
      shell window (R43), restores the persisted pane state, mounts the root
      menu from the SideMenu's sidecar, hosts the MAIN form in the ContentPane
      (Pane surface forced), wires the ☰ toggle + R44 supervisor requests to
      `Shell.collapsed` with persistence, dispatches `event` menu items as
      `onMenuItemClick` (item id via `SelectedItemId`), and honours
      `close-application`. `OpenFormSync/Async` stay standalone by
      construction (the supervisor path is untouched, R16); the main form's
      format is pinned by T5 (R5). Test
      `shell_activation_is_side_menu_only_049` (AC1/AC2/AC25 decision half):
      none ⇒ classic, MenuBar ⇒ classic, SideMenu ⇒ shell, nested SideMenu ⇒
      shell (4/4). **BLOCKED (reported, not skipped silently):** menu items
      loading OTHER forms into the pane (`open-form:` → a second interpreter +
      its generated program in-process) is the SAME open work as spec 037's
      T16 child-window hosting — run-form receives one cfrm+cbl pair and a
      compiled binary embeds one PROGRAM_AST, so no glue can host a second
      form's COBOL yet, in either mode. `run_shell` says so at runtime instead
      of pretending. AC9's WORKING-STORAGE half and the visual three-regions
      check depend on it / on T32's manual pass. The IDE's Run and the
      compiled template reach the shell through the same run-form/host code;
      the compiled template's branch should be added when T16 lands.*
  - Files: `crates/cobolt-form-host/src/shell.rs`,
    `crates/cobolt-ide/src/app.rs`, `crates/cobolt-cli/src/form_gui.rs`,
    `crates/cobolt-compiler/src/lib.rs`
  - Do: start the shell when the main form carries a SideMenu, otherwise keep
    today's one-window-per-form path untouched; menu items load embedded,
    `OpenFormSync`/`OpenFormAsync` open standalone in both modes; the main form's
    `FormFormat` is forced `Standalone`.
  - Verify: `cargo test` across the touched crates green; **AC1** — a project
    with no SideMenu opens a window per form with no shell regions; **AC2** — a
    project with one shows all three regions and a non-editable Standalone main
    form; **AC25** — the same project carrying only a `MenuBar` still starts in
    classic mode.

- [x] **T28 — Embedded geometry is inert** (R35, R36) — *done 2026-08-09.
      Already enforced by construction: T7 seeds the designed geometry (and
      FormFormat) into the form object, T12's Pane surface drops every window
      command, and T5 greys the window-only inspector rows. Test
      `embedded_geometry_reports_designed_values_and_stays_inert` (AC17
      runtime half): FormFormat readable ('Embedded'), me::Width designed 300
      → assigned 800 reported back, exactly 1 Width StateUpdate which a Pane
      host applies to no window (pinned by T12's zero-window-commands test).
      Standalone behaviour remains spec 037's.*
  - Files: `crates/cobolt-form-host/src/host.rs`,
    `crates/cobolt-runtime/src/interpreter.rs`
  - Do: an embedded form reports designed Width/Height/X/Y and setting them
    changes only the reported value; window-only properties are inert.
  - Verify: `cargo test -p cobolt-form-host && cargo test -p cobolt-runtime`
    green; **AC17** — assigning Width changes the reported value but neither
    moves nor resizes the ContentPane; the same form opened standalone honours
    all of them.

## Hardening, docs, finish

- [x] **T29 — Embedded/standalone parity test** (Risk 1, Risk 2) — *done
      2026-08-09. `zz_pane_window_parity_report`: the SAME `Both` form through
      both surfaces — differences exactly as documented (backdrop
      Window=engine vs Pane=fixed 780x672 pane-sized; effects Window=playing
      vs Pane=inert) and the designed size IDENTICAL (300x200). Quantified
      navigation summary per the reporting rule: depth 100 pushed, 100
      resident, 99 destroyed deepest-first, 0.08ms total (0.4µs/hop).*
  - Files: `crates/cobolt-form-host/tests/`
  - Do: render the same `Both` form through both surfaces and assert **only**
    the documented differences — the backdrop rect and the absence of window
    chrome and effects. Any other divergence fails.
  - Verify: `cargo test -p cobolt-form-host` green; the test prints a summary
    block listing the cases exercised, chain depth, resident forms, thread count
    and per-hop timing in ms.

- [x] **T30 — System KB** (tech.md hard constraint) — *done 2026-08-09.
      Compiler doc tables gained the SideMenu control entry and a full
      "Application shell & the `super` receiver (spec 049)" section
      (FormFormat + load-path checks + inert embedded properties; shell
      regions and pane background rules; navigation lifecycle +
      PreservePreviousForm + the two events; `super` incl. chain walk,
      universal-surface build check, NULL rule, menu Open/Collapse, and
      `me::<property>`). `chunked.data` regenerated: 993 records from 5
      documents (4,747,264 bytes);
      `prebuilt_chunked_kb_matches_the_published_documentation` green.*
  - Files: `crates/cobolt-compiler/src/lib.rs`,
    `assets/knowledge/chunked.data`
  - Do: add `FormFormat`, `PreservePreviousForm`, the SideMenu control, the
    MenuPane background properties, `super`, the form property surface reachable
    through `me`/`super`, the menu `Open`/`Collapse` methods and the two new
    events to the documentation tables; regenerate the chunked store with
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    green.

- [x] **T31 — Docs & i18n** — *done 2026-08-09. New guide section "§22 The
      application shell and the `super` receiver" (PowerCOBOL framing; the
      SideMenu switch and the never-by-accident rule; the three regions;
      FormFormat + build pairing + embedded property rules + the Both-form
      background caveat; the navigation chain, preserve, and the two events
      with file-closing guidance; `super` with COBOL examples, the checked
      surface list, NULL rules; an honest availability caveat for the pending
      multi-form hosting). Old §22 Caveats renumbered to §23 (no internal
      references broke). Registry row added to specs/steering/docs.md. i18n
      suite green (T5's 11 keys ×6 were the spec's UI strings; T27's shell
      chrome is symbol-only so far — ☰ and the breadcrumb labels are data,
      not literals). Translations untouched.*
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: add the English guide section — shell mode and its SideMenu trigger, the
    three regions, `FormFormat` and the load-path rules, the navigation chain and
    breadcrumb, `PreservePreviousForm`, the two lifecycle events, and `super`
    including the checked-versus-runtime split and the embedded-versus-standalone
    background difference from T15. Register the section in
    `specs/steering/docs.md`. Verify every `Tr` key added by T2/T5/T19 exists in
    all six languages. Translations stay untouched.
  - Verify: `cargo test -p cobolt-ide i18n` green (no empty translations); the
    guide section renders in the IDE's documentation viewer.

- [x] **T32 — Finalize** — *done 2026-08-09. `version.rs` 1.61.13 → 1.61.14
      (fix number only, per the operator's rule) + the CHANGELOG entry (Added,
      spec 049, including the known multi-form-hosting limit). Full workspace
      sweep, `--no-fail-fast`, every result line collected: **102 suites,
      1,825 passed, 2 failed** — the 2 failures are exactly
      `test_external_crates_e2e::external_crates_alias_build_and_run` and
      `…::external_crates_build_run_manifest_and_determinism`, the
      PRE-EXISTING sandbox-environment failures verified by stash-rerun during
      T6 (their nested `cargo build` cannot compile `libsqlite3-sys` in this
      shell; identical without the 049 changes). Nothing committed — the
      operator commits/pushes.
  - **Manual pass for the operator** (plan §6): rebuild `--release`, then in
    the IDE: (1) main form's FormFormat row pinned Standalone; an Embedded
    form greys the five window rows with the hint (AC17 visual); (2) drop a
    SideMenu on a main form, define a menu, Run → ONE window with MenuPane /
    breadcrumb / ContentPane (AC2); collapse via ☰, restart, state kept (AC3);
    (3) form background paints the whole pane and stays put while scrolling
    (AC5/AC21 visual); (4) a Transparency-100 form shows the desktop through
    the pane only (AC23 visual); (5) a MenuBar-only project still opens
    classic windows (AC1/AC25).*

## Done criteria

All 26 acceptance criteria in `spec.md` are checked, tests pass, the English
guide and the System KB are updated, and the change is split into feature
commit(s) per the operator's rules. Do **not** commit or push unless the operator
asks; the f=96 `[Noticia]` announcement (title ≤ 50 chars) happens only after the
work is merged to `main`.
