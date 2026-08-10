<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Application shell, in-pane navigation & the `super` receiver

- **Status:** draft
- **Folder:** specs/049-application-shell/
- **Author:** Emerson Lopes   **Date:** 2026-08-09

## 1. Overview

An enterprise application — an ERP with CRM, HR, Sales and e-Commerce
subsystems — cannot be built out of dozens of independent windows. This spec
adds an **application shell**: one window divided into a MenuPane, a breadcrumb
strip and a ContentPane, where forms are loaded *into the pane* instead of
opening as windows. A menu-carrying form becomes the current subsystem, its
menu mounts into the pane, and the forms it loads become its children. The
resident chain of those parents is what the breadcrumb renders and what the new
**`super`** receiver resolves against, so a form can address the form that
loaded it exactly as it already addresses itself with `me`.

Shell mode is opt-in and additive: a project whose main form carries no sidebar
menu behaves exactly as it does today, one window per form.

## 2. Goals / Non-goals

- **Goals:**
  - One-window shell — MenuPane (root + contextual menu slots, Open/Collapsed,
    its own background, a width the ContentPane cannot push around), breadcrumb
    chrome, ContentPane — activated by the main form carrying a sidebar menu.
    Each pane scrolls on its own.
  - A **FormFormat** property (`Standalone` | `Embedded` | `Both`) that decides
    how a form may be loaded, checked at build time against the load path.
  - In-pane forms that keep their designed size, anchored top-left, with the
    loaded form's background painting the whole pane and restoring on unload.
  - A navigation chain of resident forms, rendered as the breadcrumb, with
    defined displayed / resident / destroyed states and teardown events.
  - **PreservePreviousForm** on menus so a costly sibling screen can survive a
    switch instead of being destroyed.
  - `super` as an object receiver — the form that loaded or opened this one —
    chainable, and usable for properties as well as methods.
  - Form-as-receiver in member access, so `me::<property>` works (it does not
    today) alongside `me::<method>()`.
- **Non-goals:**
  - **Cross-form data sharing** — the shared run unit, qualified `EXTERNAL`
    (`data-1 OF form-1`) and LINKAGE `USING BY REFERENCE` on `OpenFormSync`.
    Own spec; this one leaves each form its own run unit.
  - **Menu authoring** — the sidebar menu resource, its designer, icons and
    accelerators. This spec consumes a menu; it does not define how one is
    built.
  - **Partial menu replacement** — dropped by decision; menus mount whole.
  - **Responsive layout** — no anchor/dock constraints, no reflow. Forms keep
    their designed pixel geometry.
  - Pane transition animations between routes.
  - Retiring classic multi-window mode or the spec 037 window model; both
    remain fully supported.

## 3. User stories

- As a COBOL developer, I want my ERP's subsystems to load inside one window
  with a persistent menu, so users navigate an application instead of managing
  a screenful of windows.
- As a COBOL developer, I want a subsystem's data to stay alive while the user
  works through its screens, so I do not re-read the database on every hop.
- As a COBOL developer, I want to mark one costly screen "preserve previous"
  so returning to it is instant, while every other screen is torn down on exit.
- As a COBOL developer, I want to read and set my parent form's properties with
  `super::Width` and call its procedures with `super::"Refresh"()`, so a child
  screen can drive the shell around it.
- As an end user, I want a breadcrumb that shows where I am and takes me back,
  so I am never stranded inside a subsystem.

## 4. Requirements (EARS)

### Shell activation & layout

- **R1 (ubiquitous):** Every form shall have a **FormFormat** property with
  values `Standalone` | `Embedded` | `Both` (default `Standalone`), editable in
  the RAD Form properties and persisted in the `.cfrm`.
- **R2 (event):** When the project's main form carries a **SideMenu** control
  (R45), the application shall start in **shell mode**.
- **R45 (ubiquitous):** The control catalogue shall gain a **SideMenu** control
  type — distinct from `MenuBar`, carrying a `MenuDefinition` the same way — so
  a form declares a sidebar menu explicitly and an existing form's `MenuBar`
  never changes meaning.
- **R3 (state):** While the main form carries no SideMenu control, the application
  shall behave exactly as it does today — one window per form, no shell regions,
  no change to the spec 037 flow.
- **R4 (ubiquitous):** In shell mode the main window shall present exactly three
  regions: a **MenuPane**, a **breadcrumb** strip, and a **ContentPane**.
- **R5 (constraint):** The main form's FormFormat shall be `Standalone` and
  shall not be editable — it owns the window.

### MenuPane

- **R6 (ubiquitous):** The MenuPane shall hold two mount slots: a **root slot**,
  mounted once from the main form's menu and never replaced for the life of the
  application, and a **contextual slot** holding the current subsystem's menu.
- **R7 (event):** When a form carrying a menu becomes current, the system shall
  replace the contextual slot **wholesale** with that form's menu.
- **R8 (ubiquitous):** The MenuPane shall have two states, **Open** and
  **Collapsed**; Collapsed shall render a narrow icon rail that keeps the root
  slot's items reachable.
- **R9 (ubiquitous):** The MenuPane state shall be an application-level user
  preference that survives navigation and application restart, and shall **not**
  be a Form property.
- **R10 (event):** When the MenuPane changes state, the ContentPane's origin
  shall move with the pane edge and the loaded form shall move with it, keeping
  its designed size.
- **R37 (ubiquitous):** The MenuPane shall scroll **independently** of the
  ContentPane: contents taller than the pane shall scroll within it, and its
  scroll position shall be unaffected by any scrolling in the ContentPane.
- **R38 (constraint):** The MenuPane's width shall not change when the
  ContentPane is resized; a change in window or ContentPane size shall be
  absorbed entirely by the ContentPane.
- **R39 (ubiquitous):** The MenuPane shall have its own background properties —
  colour, gradient, image with its `BgImageMode`, and transparency — and shall
  **not** be repainted by a form loaded into the ContentPane.
- **R44 (ubiquitous):** A mounted menu shall expose **`Open`** and **`Collapse`**
  methods, invocable through a member-access chain — `super::<menu-id>::Open()`
  and `super::<menu-id>::Collapse()` — so a form can drive the MenuPane state
  from COBOL. A state set this way shall persist under R9.

### ContentPane

- **R11 (ubiquitous):** The ContentPane shall host at most one embedded form at
  a time, anchored to the pane's **top-left** corner at the form's designed
  size.
- **R12 (event):** When a form is loaded into the ContentPane, the system shall
  paint the pane using that form's background properties — colour, gradient,
  image with its `BgImageMode`, and transparency — across the **entire pane**,
  and shall restore the ContentPane's design-time background when that form is
  unloaded.
- **R13 (constraint):** Background image and gradient modes shall be evaluated
  against the **ContentPane** rectangle, not the form rectangle.
- **R14 (constraint):** The breadcrumb shall be rendered as shell chrome outside
  the ContentPane, so its legibility never depends on a loaded form's
  background.
- **R40 (ubiquitous):** The ContentPane shall scroll **independently** of the
  MenuPane whenever the loaded form's designed size exceeds the pane, and its
  scroll position shall be unaffected by any scrolling in the MenuPane.
- **R41 (constraint):** While the ContentPane scrolls, the pane background of
  R12 shall stay fixed to the pane; only the loaded form's controls shall move.
- **R42 (constraint):** The ContentPane shall behave as a container for the
  loaded form — clipping it to the pane — and the loaded form shall not move,
  resize, or otherwise alter the pane. Its background (R12) shall be the only
  pane attribute a loaded form controls.
- **R43 (optional):** Where a loaded form's background carries transparency, the
  ContentPane region shall render transparent so that whatever lies **beneath
  the application window** shows through. The transparency shall not composite
  against the shell's own chrome, and the MenuPane and breadcrumb shall stay
  painted.

### Load paths & format

- **R15 (event):** When a menu item loads a form, the system shall load it
  **embedded** into the ContentPane.
- **R16 (event):** When `OpenFormSync` or `OpenFormAsync` opens a form, the
  system shall open it as a **standalone window**, in shell mode and classic
  mode alike.
- **R17 (constraint):** The build shall report an error when a menu item targets
  a form whose FormFormat is `Standalone`, or when `OpenFormSync`/
  `OpenFormAsync` targets a form whose FormFormat is `Embedded`. `Both` is valid
  on either path.
- **R18 (constraint):** Entrance and exit `WindowEffect` shall apply only to
  standalone forms; an embedded form shall appear in and leave the ContentPane
  without effect.

### Navigation chain & lifetime

- **R19 (ubiquitous):** The system shall maintain an ordered **navigation
  chain** of resident forms, from the main form to the currently displayed form.
- **R20 (ubiquitous):** Every form in the navigation chain shall remain
  **resident** — its WORKING-STORAGE alive and its menu handlers callable —
  whether or not it is currently displayed.
- **R21 (ubiquitous):** The breadcrumb shall render the navigation chain, one
  segment per entry, in chain order.
- **R22 (event):** When a breadcrumb segment is clicked, the system shall
  destroy every form below it in reverse chain order, make that form current,
  remount its menu into the contextual slot, and display its body in the
  ContentPane.
- **R23 (event):** When a root-slot menu item selects a different subsystem, the
  system shall unwind the chain to the main form — destroying popped forms per
  R22 — before pushing the new subsystem.
- **R24 (ubiquitous):** Each **menu item** shall carry a
  **PreservePreviousForm** property, default **false**, so one costly screen can
  be preserved without making every screen in the subsystem immortal.
- **R25 (event):** When a menu item loads a form while a sibling form is
  displayed, the system shall destroy the previously displayed form where
  PreservePreviousForm is false, and keep it resident where it is true.
- **R26 (ubiquitous):** Every form shall have two distinct lifecycle events:
  **onDeactivate**, fired when its body leaves the ContentPane while it remains
  resident, and **onDestroy**, fired before its storage is released.
- **R27 (constraint):** onDestroy shall not fire when a form merely leaves the
  ContentPane while remaining resident, and onDeactivate shall not be treated as
  a teardown point.

### The `super` receiver

- **R28 (ubiquitous):** The system shall recognise **`super`** as an object
  receiver in `INVOKE` statements and in member-access chains, resolving to the
  form that loaded or opened the referencing form.
- **R29 (ubiquitous):** `super` shall be bound at runtime by the load path — a
  menu load or an `OpenFormSync`/`OpenFormAsync` call — in both shell and
  classic modes.
- **R30 (ubiquitous):** The member-access resolver shall recognise a **form** as
  a receiver root, so that `me::<property>` and `super::<property>` read and
  assign form properties (no parentheses ⇒ property, an assignable lvalue) and
  `me::<method>(…)` / `super::<method>(…)` invoke methods.
- **R31 (ubiquitous):** `super` shall be chainable — `super::super::…` — each
  step resolving to the loader of the previous form.
- **R32 (state):** While the referencing form is the main form, `super` shall be
  NULL, and any reference through it shall raise the standard NULL-object
  runtime error (spec 037 R24 precedent).
- **R46 (event):** When the opener of a form launched by `OpenFormAsync` closes,
  that form's `super` shall become NULL, and any reference through it shall raise
  the standard NULL-object runtime error. An async child shall not pin its opener
  resident (spec 037 R26 keeps the two independent).
- **R33 (ubiquitous):** References to the **universal form surface** — Width,
  Height, X, Y, Title, WindowState, FullScreen, TitleVisible, FormState and the
  background properties — shall be checked at build time on `me` and on `super`
  at any chain depth.
- **R34 (constraint):** References to form-specific procedures or controls
  through `super` shall be dispatched at run time and shall raise a runtime
  error when the bound parent does not provide them.
- **R35 (state):** While a form is embedded, its geometry properties (Width,
  Height, X, Y) shall report their **designed** values, and setting them shall
  neither move nor resize the ContentPane; while standalone they shall behave as
  spec 037 defines.
- **R36 (state):** While a form is embedded, the window-only properties
  (TitleVisible, CanMinimize, CanMaximize, WindowState, FullScreen) shall be
  inert, and the property inspector shall present them as inapplicable — the
  same treatment spec 037 R9 gives TaskbarIcon on non-main forms.

## 5. Acceptance criteria

- [ ] AC1 — A project whose main form has no sidebar menu opens every form in
  its own window, with no MenuPane, breadcrumb or ContentPane anywhere. (R2, R3)
- [ ] AC2 — A project whose main form carries a sidebar menu opens one window
  showing MenuPane, breadcrumb and ContentPane; the main form's FormFormat reads
  `Standalone` and cannot be edited. (R2, R4, R5)
- [ ] AC3 — Entering a subsystem replaces the contextual slot with that
  subsystem's menu while the root slot is unchanged; collapsing the MenuPane
  leaves the root items reachable as icons, and the state survives an
  application restart. (R6–R9)
- [ ] AC4 — Collapsing and re-opening the MenuPane moves the loaded form
  horizontally with the pane edge; the form's width and height are unchanged
  before and after. (R10, R11)
- [ ] AC5 — Loading a form with a distinct background colour, gradient and
  tiled background image paints the **whole** ContentPane, with the tile count
  following the pane width rather than the form width; unloading restores the
  pane's design-time background exactly. (R12, R13)
- [ ] AC6 — Breadcrumb text stays legible with a form whose background is set to
  the same colour the breadcrumb would use, proving the strip is not painted
  inside the pane. (R14)
- [ ] AC7 — A menu item targeting a `Standalone` form fails the build with an
  error naming the form and the load path; `OpenFormSync` targeting an
  `Embedded` form fails likewise; a `Both` form passes on either path. (R17)
- [ ] AC8 — A form with an entrance effect configured shows it when opened by
  `OpenFormSync` and shows none when loaded by a menu item. (R18)
- [ ] AC9 — With chain main → CRM → Sales → Customer List, the breadcrumb shows
  four segments in order, and CRM's WORKING-STORAGE still holds values written
  before Sales was entered. (R19–R21)
- [ ] AC10 — Clicking the CRM segment destroys Customer List then Sales in that
  order, firing onDestroy on each, remounts CRM's menu into the contextual slot
  and displays CRM's body; CRM's storage is intact. (R22, R26)
- [ ] AC11 — Switching subsystems from the root slot unwinds to the main form
  first: every form below it receives onDestroy before the new subsystem is
  pushed. (R23)
- [ ] AC12 — With PreservePreviousForm false, switching between two sibling
  menu items fires onDestroy on the outgoing form and its storage re-initialises
  on return; with it true, no onDestroy fires and the storage still holds the
  earlier values on return. (R24, R25)
- [ ] AC13 — Navigating from a form to a sibling fires onDeactivate and **not**
  onDestroy on any form that stays resident; a destroyed form fires onDestroy
  and not a second onDeactivate. (R26, R27)
- [ ] AC14 — In C (loaded by B, loaded by A), `super::Title` reads B's title and
  `super::super::Title` reads A's; assigning `super::Title` changes B's. The
  same code in the main form raises the NULL-object runtime error. (R28, R30,
  R31, R32)
- [ ] AC15 — `me::Width` reads the form's own width and `me::Title` is
  assignable — both fail today and must pass after this spec. (R30)
- [ ] AC16 — A build fails when `super::Widht` (a misspelt universal property)
  is referenced at any chain depth; `super::"RecalcTotals"()` builds and raises
  a runtime error only when the bound parent has no such procedure. (R33, R34)
- [ ] AC17 — An embedded form reports its designed Width; assigning a new Width
  changes the reported value but neither moves nor resizes the ContentPane, and
  TitleVisible/WindowState are shown as inapplicable in the inspector. The same
  form opened standalone honours all of them. (R35, R36, R42)
- [ ] AC18 — `super` is bound in classic multi-window mode too: a form opened by
  `OpenFormAsync` reads its opener's Title through `super`. (R29)
- [ ] AC19 — A MenuPane holding more items than fit scrolls within the pane;
  scrolling it leaves the ContentPane's scroll position unchanged, and scrolling
  the ContentPane leaves the MenuPane's unchanged. (R37, R40)
- [ ] AC20 — Widening the window increases the ContentPane's width by the full
  delta while the MenuPane's width is unchanged. (R38)
- [ ] AC21 — A form designed larger than the ContentPane scrolls inside it; the
  pane's background image stays put while the form's controls move. (R40, R41)
- [ ] AC22 — The MenuPane renders its own configured background, and loading
  forms with different backgrounds into the ContentPane leaves it unchanged.
  (R39)
- [ ] AC23 — A form whose background carries transparency, loaded into the
  ContentPane, shows the desktop beneath the application window through the pane
  region, while the MenuPane and breadcrumb stay painted. (R43)
- [ ] AC24 — `INVOKE super::<menu-id>::Collapse()` from a loaded form collapses
  the MenuPane and `Open` restores it; the resulting state survives navigation
  and restart per R9. (R44)
- [ ] AC25 — A SideMenu control on the main form starts the shell; the same
  project with only a `MenuBar` starts in classic multi-window mode, and an
  existing project carrying a `MenuBar` on its main form is unaffected. (R2, R3,
  R45)
- [ ] AC26 — A form opened by `OpenFormAsync` reads its opener's Title through
  `super`; after the opener closes, the same reference raises the NULL-object
  runtime error and the child keeps running. (R46)

## 6. Constraints & steering check

- **i18n (6 languages):** yes — new RAD labels (FormFormat and its three values,
  PreservePreviousForm, the MenuPane background properties), the MenuPane
  Open/Collapsed control, breadcrumb tooltips, the inapplicable-property
  presentation, and the R17 build-error messages must be `Tr` fields translated
  in EN/ES/PT/JA/ZH/FR. COBOL-facing
  identifiers stay English: `super`, `me`, property names, `onDeactivate`,
  `onDestroy`, `Standalone`/`Embedded`/`Both`.
- **Generated-code contract:** yes — generated `.cbl` gains the onDeactivate and
  onDestroy event paragraphs and the FormFormat property; the developer banner
  and regenerate-on-Build/Run/Debug/Check contract are unchanged.
- **System KB:** yes — FormFormat, PreservePreviousForm, the MenuPane background
  properties, `super`, the form property surface reachable through `me`/`super`,
  the menu `Open`/`Collapse` methods, and the two new events are
  properties/methods/events, so the `cobolt-compiler` documentation tables must
  be updated **and** `assets/knowledge/chunked.data` regenerated
  (`cargo run -p cobolt-ide --example build_chunked_kb`) in the same change.
- **Docs:** English `docs/developers-guide-en.md` section required — shell mode
  and when it activates, the three regions, FormFormat and the load-path rules,
  the navigation chain and breadcrumb, PreservePreviousForm, the two lifecycle
  events, and `super` including the checked-versus-runtime split. Translations
  untouched (user-maintained).
- **Fix vs feature:** **feature** — capability beyond COBOL-85 and beyond the
  IDE's existing scope. Per `tech.md`, bump the fix number `z` in
  `crates/cobolt-ide/src/version.rs` and add a `CHANGELOG.md` entry; own
  commit(s), never mixed with fixes; announce on forum **f=96** with the
  `[Noticia]` prefix, title ≤ 50 characters, inside the operator's push window.
- **tech.md:** egui/eframe 0.36; classic multi-window mode still needs
  `ctx.show_viewport_immediate`, so this spec does **not** remove the
  multi-viewport host — spec 037's AC4/AC5/AC15 remain open work.
- **Placement (structure.md):** FormFormat and PreservePreviousForm in
  `cobolt-forms`; the shell regions and panes in `cobolt-ide/src/panels/`, wired
  in `app.rs`; `super` and form-as-receiver in `cobolt-parser`/`cobolt-semantic`
  (build-time checks) and `cobolt-runtime` (binding and dispatch); generated
  event paragraphs in `cobolt-codegen`.

## 7. Open questions

- ~~**Q1:** Does **PreservePreviousForm** live on the `MenuDefinition` or on the
  `MenuItem`?~~ — **Resolved 2026-08-09:** on the `MenuItem`. Folded into R24.
- ~~**Q2:** Does this spec add a stable root receiver for shell-level
  services?~~ — **Resolved 2026-08-09:** no. The MenuPane state is driven
  through the mounted menu object (`super::<menu-id>::Collapse()`), so the one
  shell service that needed a setter has a home without a root receiver. Folded
  into R44. See Q10 for what is still undecided about it.
- **Q10:** Does `Collapse`/`Open` on a mounted menu (R44) act on the **whole
  MenuPane** or only on that menu's **slot**? Pane-wide makes any mounted menu a
  usable handle from any depth; per-slot lets a subsystem collapse its own menu
  while the root rail stays open, but leaves the root menu reachable only by
  walking `super` to the main form. *(Proposed: pane-wide, since R8's
  Open/Collapsed is defined as a state of the pane, not of a slot.)*
- ~~**Q3:** What marks a form as carrying a "sidebar menu" (R2)?~~ —
  **Resolved 2026-08-09:** a new **SideMenu** control type, distinct from
  `MenuBar`. Folded into R2 and R45. Reusing `MenuBar` was rejected because an
  existing project with a menu bar on its main form would silently become a
  shell app, which R3 forbids.
- ~~**Q4:** Does an async child's `super` go NULL when its opener closes?~~ —
  **Resolved 2026-08-09:** yes, per the 037 R24 precedent. Folded into R46.
- **Q5:** Should `Both` be the default FormFormat for newly created forms rather
  than `Standalone`? `Standalone` preserves today's behaviour for existing
  projects, but a developer building a shell app will retype it on every form.
- **Q6:** When a `Both` form is loaded embedded, its background paints the pane
  and its designed size is invisible (R11–R13); opened standalone the same
  background paints only the form rectangle. Does the designer preview both
  framings, or does the developer pick one preview per form?
- **Q7:** Where do the MenuPane background properties (R39) live — on the main
  form, which owns the shell, or on a separate shell object in the inspector?
  *(Proposed: the main form, since it is already the shell's owner and is the
  only form guaranteed to exist.)*
- **Q8:** Is the MenuPane's width itself a property, and may the user drag it?
  R38 fixes it against ContentPane resizes but says nothing about direct
  resizing. *(Proposed: a property with separate Open and Collapsed widths, not
  user-draggable in this spec.)*
- **Q9:** May a subsystem form restyle the MenuPane while its menu is mounted —
  tinting CRM differently from HR — or is the MenuPane background stable for the
  application's life? *(Proposed: stable, by the same reasoning as R14: shell
  chrome should not be at the mercy of whatever form is loaded. Note this is the
  deliberate opposite of the ContentPane, which R12 lets the loaded form
  repaint.)*
