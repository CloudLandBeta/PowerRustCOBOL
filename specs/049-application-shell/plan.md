<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Application shell, in-pane navigation & the `super` receiver

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-08-09

## 1. Approach

The shell is a **runtime** surface, not an IDE panel. A shipped ERP binary has to
show it, so it lands in `cobolt-form-host` — the crate whose whole purpose is
"implemented once and consumed by both live surfaces" — and every consumer
(`rcrun run-form`, compiled binaries, the IDE's Run) drives the same code.

Three findings from the codebase set the shape of the work, and each removes a
chunk of it:

- **`render_form(ui: &mut egui::Ui, …)`** already renders a whole form into a
  `Ui`, taking its origin from `ui.min_rect().min`. Hosting a form in a pane is
  therefore a matter of handing it the pane's `Ui` — the coordinate translation
  that a container-based design would have needed is free (R11).
- **`Backdrop.window_size: Option<Vec2>`** already stretches a form's background
  across a host area bigger than the form, with `BgImageMode` evaluated against
  that larger rect. R12 and R13 are a matter of passing the pane size, not new
  painting code.
- **`FormHost::ui(&mut self, root_ui: &mut egui::Ui, …)`** is already
  `Ui`-shaped; only `FormHost::run(config)` owns a window. The ContentPane can
  drive the existing host rather than growing a third render loop.

### A. Model & designer (R1, R5, R24, R36, R39)

`Form` gains `form_format: FormFormat { Standalone, Embedded, Both }` (default
`Standalone`), placed in the existing spec-037 window block in `model.rs`, with
`.cfrm` round-trip in `xml.rs` and an inspector row in `properties.rs`. The main
form's row is read-only (R5), reusing the treatment 037 R9 already gives
`TaskbarIcon` on non-main forms. The same mechanism marks the window-only
properties inapplicable while `FormFormat` is `Embedded` (R36).

`PreservePreviousForm` joins the menu model (R24). The MenuPane background (R39)
reuses the existing `Backdrop` struct and `paint_backdrop`, so the pane inherits
colour, gradient, image, `BgImageMode` and transparency semantics identical to a
form's — one implementation, no second background dialect.

### B. The shell surface (R2–R14, R37–R43)

A new `crates/cobolt-form-host/src/shell.rs` owns the three regions, the
navigation chain and the MenuPane state:

- **MenuPane** — `egui::SidePanel::left`, `resizable(false)` at an explicit
  width, so a ContentPane or window resize cannot move it (R38). Open and
  Collapsed are two widths (R8).
- **Breadcrumb** — `TopBottomPanel::top` *inside* the remaining area, so it is
  chrome outside the ContentPane and cannot be repainted by a loaded form (R14).
- **ContentPane** — `CentralPanel`, hosting one `FormHost` in embedded mode.

Each pane wraps its content in its **own** `ScrollArea` with a distinct id, so
R37 and R40 are structural — there is no scroll bookkeeping to keep in sync and
no way for one pane to move the other.

The ContentPane paints the loaded form's backdrop **itself, outside its
ScrollArea**, sized to the pane, and suppresses the engine's own backdrop pass.
That is what R41 requires and it is a deliberate divergence from the shared
engine's current behaviour — see Risk 1.

Transparency (R43) works by creating the shell window with
`with_transparent(true)` and a zeroed clear colour, exactly as form windows do
today, then having the MenuPane and breadcrumb paint **explicit opaque fills**.
The ContentPane paints only the form's backdrop, so a transparent form leaves
the desktop visible through that region alone.

### C. Navigation & lifecycle (R15–R27)

The shell keeps a `NavChain: Vec<NavEntry>`, each entry owning a form's
`FormHost` and its interpreter thread. Residency (R20) is simply "still in the
chain" — nothing is torn down while an entry exists, so a menu host's storage and
its menu handlers stay live after its body leaves the pane.

- The breadcrumb renders the chain, one segment per entry (R21).
- A segment click truncates the chain, firing `onDestroy` in reverse order
  before dropping each entry (R22).
- A root-slot selection unwinds to index 0, then pushes the new subsystem (R23).
- A sibling load consults `PreservePreviousForm`: false destroys the outgoing
  form, true leaves it resident (R25).

`onDeactivate` and `onDestroy` (R26) become generated nested programs in
`cobolt-codegen` alongside the existing event paragraphs, dispatched through the
`FormEvent` channel that already carries form events.

### D. `super` and form-as-receiver (R28–R35, R44)

The runtime already has the two pieces this needs. `Interpreter` gains
`super_form_object` and a bound parent handle beside the existing
`self_form_object`, plus an `is_super()` alongside `is_me()`. Because
`try_exec_window_call` already dispatches methods on a `windowHandler` variable
through `FormRequest::HandleMethod`, **`super` is a pre-bound handle** — the
INVOKE path is a binding change, not a new dispatch mechanism.

Member chains need more. `lower_member_chain` flattens a chain to a flat
`(root: String, segs)` and the resolver treats the root as a **control**. Three
additions:

1. **Form as receiver root** — `ME` / `SUPER` resolve to a form object rather
   than a control. This is what makes `me::<property>` work at all; it does not
   today (AC15), so `super` cannot be built without fixing `me`.
2. **`super` as a chain *segment*** — `super::super::Title` flattens to
   root `SUPER` with segments `[super, Title]`, so the resolver must recognise a
   `super` segment as "the parent of the current form" rather than looking up a
   property of that name (R31).
3. **Menus as members of a form** — `super::<menu-id>::Collapse()` needs the form
   receiver to yield a *menu object* by id, which then carries its own methods
   (R44). This is one level deeper than reading a scalar property and is the
   largest single piece of resolver work in the spec.

`cobolt-semantic` gets a universal form-property table so `me::X` and
`super::…::X` are checked at build time when `X` is in that set, and passed to
runtime dispatch otherwise (R33, R34). The load-path check (R17) also lands
there: menu targets and `OpenFormSync/Async` arguments are both statically known,
so a `Standalone` form on a menu item and an `Embedded` form on `OpenForm*` are
build errors.

### E. Persistence (R9)

MenuPane state is a per-application preference, stored following the convention
`cobolt-ide/src/ui_prefs.rs` already uses (`<data_dir>/cobolt/ui.toml`) but keyed
per application, so each shipped binary keeps its own. It lives in
`cobolt-form-host` and therefore works in compiled apps, not only in the IDE.

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs` — `FormFormat` enum + `Form.form_format`;
  MenuPane backdrop fields on the main form.
- `crates/cobolt-forms/src/xml.rs` — `.cfrm` round-trip for both, with
  serde-style defaults so existing forms load unchanged.
- `crates/cobolt-forms/src/menu.rs` — `PreservePreviousForm` on `MenuItem`;
  `Open`/`Collapse` as menu methods.
- **`SideMenu` control (R45)** — a new `ControlType` variant threaded through
  `model.rs` (enum, `as_str`/`from_str`, default size, property set, event set),
  `paint.rs`/`render.rs` (design-time and runtime drawing),
  `cobolt-ide/src/panels/toolbox.rs` (catalogue entry) and `designer.rs` (the
  existing `SetMenuDefinition` editor, which is already keyed by control id and
  so needs only to accept the new type).
- `crates/cobolt-forms/src/render.rs` — an opt-out for the engine's backdrop pass
  so the ContentPane can paint it outside the scroll (Risk 1).
- `crates/cobolt-form-host/src/shell.rs` — **new**: regions, layout, navigation
  chain, breadcrumb, MenuPane state and persistence.
- `crates/cobolt-form-host/src/host.rs` — an embedded surface mode that
  neutralises window-only behaviour (title, clear colour, viewport commands,
  entrance/exit effects per R18).
- `crates/cobolt-runtime/src/interpreter.rs` — `is_super`, parent binding, form
  receiver in `lower_member_chain`/`resolve_member`, `super` as a segment, menu
  objects.
- `crates/cobolt-runtime/src/form_host.rs` — chain-aware `FormRequest`s (push,
  pop, breadcrumb truncate) beside the existing spawn/close actions.
- `crates/cobolt-semantic/src/type_checker.rs`, `resolver.rs` — universal
  property table (R33/R34) and the load-path check (R17).
- `crates/cobolt-codegen/src/lib.rs` — `onDeactivate` / `onDestroy` handler
  programs; `FormFormat` in the generated property block.
- `crates/cobolt-ide/src/panels/properties.rs` — the new rows and their
  inapplicable states.
- `crates/cobolt-ide/src/app.rs` — Run drives the shell when the main form
  carries a sidebar menu (R2), classic path untouched (R3).
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` keys ×6 languages.
- `crates/cobolt-compiler/src/lib.rs` — the shipped-binary template starts the
  shell when the project qualifies; System KB property/method/event tables.
- `assets/knowledge/chunked.data` — regenerated in the same change (tech.md).
- `docs/developers-guide-en.md` — new section (§21 registry row: shell, panes,
  FormFormat, navigation, `super`, lifecycle events).
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — `z` bump.

## 3. Data / model changes

- **`FormFormat`** — new enum, serialised in `.cfrm` as an attribute. Absent
  attribute ⇒ `Standalone`, so every existing form keeps today's behaviour and no
  migration runs (R3).
- **MenuPane backdrop** — a `Backdrop`-shaped group persisted on the main form.
  Absent ⇒ the shell's default chrome fill.
- **`PreservePreviousForm`** — boolean on `MenuItem`, default `false`,
  `#[serde(default)]` so existing menus load.
- **`ControlType::SideMenu`** — new variant. It carries a `MenuDefinition`
  exactly as `MenuBar` does (the definition is keyed by control id, not by
  control type), so the designer's menu editor and the `.cfrm` storage need no
  new mechanism. `MenuBar` keeps its present meaning, which is what protects R3.
- **Generated COBOL** — two new event paragraphs per form. The developer banner
  and the regenerate-on-Build/Run/Debug/Check contract are unchanged.
- **New on-disk file** — `<data_dir>/<app>/shell.toml` for MenuPane state.
  Absent ⇒ Open.

No `.cfrm` version bump is needed: every addition is a defaulted attribute.

## 4. Key decisions & alternatives

- **Decision: the shell lives in `cobolt-form-host`.** — Why: shipped binaries
  must show it, and that crate is already the shared surface for `rcrun run-form`
  and compiled apps. — Rejected: `cobolt-ide/src/panels/`, which `structure.md`
  suggests for IDE features, because it would make the shell IDE-only and
  unshippable.
- **Decision: the ContentPane drives the existing `FormHost` in an embedded
  mode.** — Why: `render_form` already has two call sites (`app.rs:11982` and
  `host.rs:1253`), and spec 042 exists precisely because drifted copies shipped
  the 1.60.33 caption bug. — Rejected: a third render loop in the shell.
- **Decision: the ContentPane paints the backdrop itself, outside its
  ScrollArea.** — Why: R41 requires the background to stay put while the form
  scrolls. — Rejected: the engine's in-`Ui` backdrop, which shares the form's
  origin and therefore scrolls with it.
- **Decision: `super` is a pre-bound `windowHandler`, not a new object kind.** —
  Why: `try_exec_window_call` already routes handle methods to the supervisor, so
  the whole 037 R23 method surface comes for free. — Rejected: a parallel
  parent-dispatch path.
- **Decision: the MenuPane reuses `Backdrop` + `paint_backdrop`.** — Why: one
  background implementation across designer, preview, running form, compiled
  binary and now the MenuPane. — Rejected: a bespoke pane-background type.
- **Decision: independent scrolling by construction.** — Why: one `ScrollArea`
  per pane makes R37/R40 unbreakable. — Rejected: shared scroll state with
  coordination rules.
- **Decision: `FormFormat` defaults to `Standalone`.** — Why: existing projects
  must not change behaviour (R3). — Rejected: defaulting to `Both`, which is
  friendlier for new shell apps but silently alters every existing form
  (spec Q5).

## 5. Risks & mitigations

1. **R41 contradicts the shared engine's documented behaviour.**
   `backdrop_size` states that a host area *smaller* than the form keeps a
   form-sized backdrop "which the form scrolls inside" — the background scrolls
   with the content, the opposite of R41. Honouring R41 means a `Both` form's
   background behaves differently embedded than standalone. → **Mitigation:**
   implement the ContentPane override, document the difference in the guide, and
   add a parity test that asserts exactly this difference and no other. **This is
   the one item I would settle with the operator before `/tasks`** — the
   alternative is to relax R41 for the form-larger-than-pane case.
2. **`FormHost` is window-shaped.** It sets a window title, a clear colour,
   viewport commands and entrance/exit effects. Embedded mode must neutralise all
   of them without forking the type, or 042's single-host guarantee erodes. →
   **Mitigation:** one `Surface { Window, Pane }` discriminator inside `FormHost`
   with a single branch per window-only action, plus a parity test running the
   same form through both surfaces.
3. **Resident ancestors are resident interpreter threads.** A five-deep chain
   keeps five threads and five run units alive. → **Mitigation:** measure it —
   the navigation tests report chain depth, resident form count, thread count and
   per-hop timing, per the quantified-results rule in `tech.md`.
4. **`super`'s build-time checking covers only the universal surface (R33).**
   Everything form-specific fails at run time, in production, in an ERP. →
   **Mitigation:** documented plainly in the guide; the spec's own open questions
   already record that a declared form tree is the fix, and that is a later spec.
5. **`super` must be both a root and a segment.** A control literally named
   `super` or `me` would collide. → **Mitigation:** reserve both words; the form
   checker rejects them as control ids with a clear message.
6. **A transparent shell window inverts painting.** Unpainted chrome becomes
   see-through, so a missed fill is a hole in the window rather than a wrong
   colour. → **Mitigation:** MenuPane and breadcrumb paint explicit opaque fills,
   with a test asserting non-zero alpha in both regions.
7. **System KB drift.** `tech.md` requires the compiler doc tables *and*
   `assets/knowledge/chunked.data` to be regenerated in the same change. →
   **Mitigation:** it is an explicit task in `/tasks`, not a checklist note, and
   the freshness test guards it.

## 6. Test strategy

**`cobolt-forms`**
- `.cfrm` round-trip for all three `FormFormat` values, plus a legacy file with
  no attribute loading as `Standalone`.
- MenuPane backdrop properties round-trip; `PreservePreviousForm` defaults false.

**`cobolt-form-host`** (headless egui frames, the pattern `host.rs` tests
already use)
- MenuPane width is unchanged across a window resize while the ContentPane
  absorbs the whole delta (AC20).
- Scrolling one pane leaves the other's offset untouched, both directions (AC19).
- A form larger than the pane scrolls while the backdrop rect stays fixed
  (AC21).
- The MenuPane's background is unchanged across loads of forms with different
  backgrounds (AC22).
- A transparent form leaves the ContentPane region unpainted while MenuPane and
  breadcrumb keep opaque alpha (AC23).
- Chain: push/pop, `onDestroy` in reverse order (AC10), root-slot unwind (AC11),
  `PreservePreviousForm` both ways (AC12), `onDeactivate` versus `onDestroy`
  discipline (AC13).
- Parity: the same `Both` form rendered embedded and standalone, asserting the
  documented differences and nothing else (Risk 1, Risk 2).

**`cobolt-runtime`**
- `me::Width` reads and `me::Title` assigns — both fail today (AC15).
- `super::Title`, `super::super::Title`, and the NULL-object error in the main
  form (AC14).
- `super::<menu-id>::Collapse()` and `Open()` (AC24).
- Embedded geometry is inert; standalone honours it (AC17).
- `super` bound after `OpenFormAsync`, in classic mode (AC18).

**`cobolt-semantic`**
- Menu item → `Standalone` form and `OpenFormSync` → `Embedded` form both fail
  the build with an error naming the form and path; `Both` passes either (AC7).
- A misspelt universal property fails at any chain depth; an unknown procedure
  builds and only fails at run time (AC16).

**Reporting.** The navigation and parity tests print a summary block — cases
exercised, chain depth, resident forms, threads, and per-hop timing in ms — so a
reader finishes knowing what ran and how it performed, not merely that it passed.

**Manual / visual.** Launch the IDE, open a two-subsystem sample project, Run:
collapse and expand the MenuPane and confirm the form travels with the pane edge
at unchanged size (AC4); scroll both panes; load a form with a tiled background
and check the tiling follows pane width (AC5); load a transparent form and
confirm the desktop shows through the pane only (AC23); walk three levels deep
and click back through the breadcrumb (AC10).

## 7. Steering compliance

- [ ] i18n: all new UI strings in 6 languages — FormFormat and its three values,
      PreservePreviousForm, MenuPane background rows, the Open/Collapsed control,
      breadcrumb tooltips, inapplicable-property presentation, R17 build errors.
      COBOL-facing names stay English (`super`, `me`, `onDeactivate`,
      `onDestroy`, `Standalone`/`Embedded`/`Both`).
- [ ] Generated-code banner + regenerate-on-action contract preserved.
- [ ] English dev guide updated; `-es/-pt/-jp/-cn` untouched.
- [ ] System KB doc tables updated **and** `chunked.data` regenerated in the same
      change.
- [ ] Fix vs feature: **feature** → bump `z` in `version.rs`, `CHANGELOG.md`
      entry, own commit(s) never mixed with fixes, forum **f=96** with
      `[Noticia]`, title ≤ 50 chars, inside the push window.
- [ ] No "cobolt" in user-facing text; COBOL identifiers English.
- [ ] `tech.md`: egui/eframe 0.36; the multi-viewport host is **not** removed —
      spec 037 AC4/AC5/AC15 stay open.

## 8. Open questions carried from the spec

Resolved by this plan (proposals, for the operator to confirm):

- **Q7** — MenuPane background properties live on the main form.
- **Q8** — MenuPane width is a property with Open and Collapsed values, not
  user-draggable.
- **Q9** — the MenuPane background is stable; a subsystem cannot restyle it.
- **Q10** — `Collapse`/`Open` act on the whole pane.
- **Q5** — `FormFormat` defaults to `Standalone`.
- **Q6** — the designer previews per the form's `FormFormat`; a `Both` form gets
  a preview toggle.

Settled by the operator on 2026-08-09, and folded into the spec:

- **Q3 → a new `SideMenu` control type** (spec R45). Reusing `MenuBar` was
  rejected: an existing project with a menu bar on its main form would silently
  become a shell app, which R3 forbids. This is the largest single addition the
  answer brought with it — a new control means catalogue, toolbox, drawing,
  codegen and System KB entries.
- **Q1 → `PreservePreviousForm` on `MenuItem`** (spec R24).
- **Q4 → an async child's `super` goes NULL when its opener closes** (spec R46),
  per the 037 R24 precedent; the child is not allowed to pin its opener.
- **Risk 1 → R41 stands as written.** The ContentPane paints the backdrop
  outside its ScrollArea and the background never scrolls. The resulting
  embedded-versus-standalone difference for a `Both` form is pinned by the
  parity test in §6 and documented in the guide.

Nothing is blocking; `/tasks` can be complete.
