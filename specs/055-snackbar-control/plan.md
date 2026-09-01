# Plan — Snackbar control

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-09-01

## 1. Approach

The Snackbar is the **43rd `ControlType`**, non-visual (D1). The dropped control
is a *template*: it carries the defaults and never paints itself. Each `Show()`
(D2) mints a **live notification** from those defaults into a per-surface
**stack**, which the form host owns and the render engine draws.

That split is the whole design, and it follows the shape the codebase already
uses for a control whose run-time presence is not its designed rect:

| Layer | Owns | Precedent it copies |
|---|---|---|
| `cobolt-forms` | the `Snackbar` variant, its seeded properties, `is_non_visual`, the icon, and a **pure** stack-layout function | `Timer` (non-visual), `sidebar::content_pane_size` (pure geometry, one home) |
| `cobolt-form-host` | the live stack: raise, expire, reflow, dismiss | `FormBody` already owns per-surface run-time state |
| `cobolt-runtime` | `Show()` / `DismissAll()` dispatch, `onShown`/`onButtonClick` delivery | `Stmt::Invoke` → the `"EXPORTCSV"`/`"REFRESHBINDING"` method table |
| `cobolt-ide` | toolbox entry, property editors, i18n | `Timer` in `toolbox.rs`, `properties.rs` |
| `cobolt-codegen` | declaration + handler stubs | every other control |
| `cobolt-compiler` | KB property/method/event docs | `"ShowColumnFilters" => (BOOL_DOMAIN, …)` |

**Requirement coverage.** R1–R3 are the catalogue/designer work; R4–R10 the
runtime; R11–R15 the stack; R16–R17 placement; R18–R24 appearance; R25–R26 the
engine contract.

**The one genuinely new mechanism** is the stack. Everything else is an
instance of a pattern already in the tree, and the plan deliberately keeps it
that way — a notification is drawn by `paint::draw_control`-style primitives on
the surface's own painter, not by a second renderer (R25).

### Why the stack lives in the host, not the engine

`cobolt-forms` is a **pure renderer**: given controls and state, it paints. It
has no clock and no ownership of anything that outlives a frame. A notification
has a lifetime, a timeout and a hover-pause — all state between frames. The
host already owns exactly this kind of state (`FormBody`), so the stack goes
there and the engine is handed a list of *what to draw this frame*.

This keeps R25 honest: `rcrun run-form` and a compiled binary both consume
`cobolt-form-host`, so they get one implementation for free. Putting the stack
in the engine would have made the designer own a live clock it has no business
having.

## 2. Affected crates / files

**`cobolt-forms`**
- `src/model.rs` — `ControlType::Snackbar`; `Control::new` seeds the 35
  properties in spec §6; `is_non_visual()` gains the variant; `as_str()` /
  `from_str()` round-trip; `SUPPORTED_EVENTS` gains the five events.
- `src/icons.rs` — `control-snackbar`, plus 5 category + ~11 button icons (§4 below).
- `src/snackbar.rs` **(new)** — the *pure* part: size classes → dimensions,
  category → defaults, `Buttons` parsing, and `stack_layout(anchor, margin,
  spacing, order, surface, &[NotificationSize]) -> Vec<Rect>`. No clock, no state.
- `src/paint.rs` — `draw_snackbar(painter, rect, &Notification, …)`, reusing
  `draw_glass_auto*`, `paint_background_gradient`, `face_color`, `caption_shadow`.

**`cobolt-form-host`**
- `src/snackbar_stack.rs` **(new)** — `SnackbarStack`: `raise()`, `tick(now)`,
  `dismiss(id, reason)`, `hover(pos)`, overflow policy, reflow.
- `src/host.rs` — `FormBody` owns one `SnackbarStack`; ticked and drawn per
  frame after the controls, before chrome.

**`cobolt-runtime`**
- `src/interpreter.rs` — `"SHOW"` / `"DISMISSALL"` in the control-method table
  (~line 10949) and the known-method list (~13730); a `FormRequest`-style
  message so the host raises it.
- `src/form_host.rs` — the request/event pair carrying the raise and the
  dismiss reason.

**`cobolt-ide`**
- `src/panels/toolbox.rs` — the entry (`ControlType::Timer` at line 186 is the shape).
- `src/panels/properties.rs` — grouped editors; `Buttons` gets a small
  line-per-button editor, not a raw text box.
- `src/i18n.rs` — every new label/diagnostic, **six languages**.

**`cobolt-codegen`** — `src/lib.rs`: declaration + `onButtonClick` stub.

**`cobolt-compiler`** — `src/lib.rs`: property/method/event doc tables, then
`cargo run -p cobolt-ide --example build_chunked_kb` and commit
`assets/knowledge/chunked.data` **in the same change**.

**`docs/developers-guide-en.md`** — a Snackbar section. Translations untouched.

## 3. Data / model changes

- **`.cfrm`:** one new `<Control type="Snackbar">` with 35 `<Property>` children.
  `ControlType` serialises **by name**, so appending the variant is backward
  compatible — an older build reading a newer form fails on the *name*, which is
  the honest outcome. **To verify before landing**, not assume: spec §10 flags
  it, and `Expr` (bincode, ordinal) carries the opposite rule.
- **No AST change.** `Show()` is an ordinary `Stmt::Invoke`; no new variant, so
  the bincode append-only hazard is untouched.
- **Live model (new, host-side only):**
  ```
  Notification { id: u64, raised_at: Instant, paused_for: Duration,
                 text, category, size, colours…, buttons: Vec<SnackbarButton> }
  ```
  Never serialised — it does not exist at rest.
- **`Buttons`** is a multi-line string (`id|text|icon|position|dismiss`), matching
  `Columns` / `RowHeightOverrides` / `ColumnFilters`.

## 4. Key decisions & alternatives

- **Stack in the host, engine stays pure.** *Why:* the engine has no clock and
  no cross-frame ownership; the host already does, and both live surfaces share
  it (R25). *Rejected:* engine-owned stack — would give the designer a live
  clock, and fork behaviour between `rcrun` and compiled binaries.

- **`Show()` mints; the control is a template (D2).** *Why:* the only shape in
  which `MaximumVisible`, `OverflowBehavior` and reflow mean anything.
  *Rejected:* one control = one notification — reduces the spec to a toast with
  no stack, and contradicts §16–20.

- **Anchor resolved against the FORM's surface (D3).** *Why:* an Embedded form's
  messages belong inside its ContentPane, matching the rule established for
  backgrounds at 1.62.132/135 — the surface is not always the window. *Rejected:*
  always the top-level window (the operator's first answer, then corrected).

- **No new render path.** Notifications are drawn by the surface's painter using
  the existing glass/gradient/shadow primitives. *Why:* R25 parity and the
  standing "controls are drawn by two paths" trap. *Rejected:* an egui `Area`
  per notification — a second renderer, and `Area` brings its own id/ordering
  problems, which is precisely the class of bug that cost this project a day on
  the DataGrid clip leak.

- **`Buttons` as a multi-line string.** *Why:* the catalogue's established
  collection shape. *Rejected:* a JSON blob like `AdvancedGrid` — opaque in the
  properties pane and hand-editing it is a support burden.

- **Icons are new entries in the EXISTING style.** *Why:* one stroke treatment
  across 1110 icons; "hand-drawn" is achievable in the path shapes. *Flagged*
  in §7 — an art call the operator may overrule.

## 5. Risks & mitigations

- **Risk: the 43rd control is a wide change** — ~11 files enumerate
  `ControlType`. → *Mitigation:* `every_control_type_has_an_icon` already fails
  until the icon exists; add the variant first and let the compiler's
  non-exhaustive-match errors enumerate the rest. The compiler is the checklist.

- **Risk: `.cfrm` forward-compat.** → *Mitigation:* verify by round-tripping a
  Snackbar form through save/load **and** confirm `ControlType` is name-keyed
  before writing the variant. Never assume from `Expr`'s rule.

- **Risk: the stack is timing state, and timing tests flake.** → *Mitigation:*
  `SnackbarStack::tick(now: Instant)` takes the clock as a parameter. Every test
  drives it with a fabricated instant; nothing sleeps. (`Date::now` is banned in
  workflow scripts for the same reason — determinism is the house style.)

- **Risk: an old form copied from another project carries a partial property
  set** — exactly what produced today's DataGrid hunt. → *Mitigation:* every
  property read goes through a defaulting accessor; no `unwrap()` on a seeded
  property. A missing property must mean "the default", never a panic or a
  silently wrong value.

- **Risk: hover-pause needs pointer state the engine does not own.** →
  *Mitigation:* the host passes the pointer position into `tick`; the engine
  reports each notification's rect, the same way `control_rects` already works.

- **Risk: scope.** 35 properties, 5 categories, ~16 icons, 5 events, a stack and
  an overflow policy is a large feature. → *Mitigation:* `/tasks` sequences it so
  a Snackbar that *shows and expires* lands before stacking, overflow and the
  full icon set. Each stage is independently green.

## 6. Test strategy

**`cobolt-forms` (pure, no clock)**
- `snackbar_size_classes_have_the_documented_dimensions` — the three classes,
  reporting the table.
- `a_stack_grows_away_from_its_anchor` — Top anchors grow down, Bottom grow up,
  newest nearest the anchor (R12); prints anchor → the y-order it produced.
- `centre_anchors_default_to_newest_first` (R13) and honour `StackOrder`.
- `dismissing_the_middle_closes_the_gap` (R14) — reports before/after rects.
- `buttons_parse_one_per_line` — including omitted trailing fields and a 4th
  button producing a diagnostic, not a silent drop (Q5).
- `every_control_type_has_an_icon` — already exists; must stay green.

**`cobolt-form-host` (the stack, deterministic)**
- `a_notification_expires_on_its_own_timeout` — driven by a fabricated clock,
  reporting the raise/expire instants.
- `timeout_zero_never_expires` (R6).
- `hover_holds_the_timeout_and_leaving_resumes_it` (R7) — reports remaining ms
  across the hover.
- `overflow_applies_the_configured_policy` (R15) — Queue / DiscardOldest /
  DiscardNewest, reporting which ids survived and **what was dropped** (no
  silent truncation).
- `dismiss_reasons_are_reported_verbatim` (Timeout/User/Action/Programmatic/Overflow).

**`cobolt-runtime`**
- `show_raises_one_notification_per_call` (D2/R4).
- `a_button_with_dismiss_on_click_raises_then_dismisses` (R8) — asserts event
  order: `onButtonClick` **then** `onClosed`.
- `dismiss_all_clears_the_stack` (R9).

**`cobolt-ide`** — `i18n_tests` covers the six languages automatically.

**Every test reports quantified results** (tech.md): counts, timings and the
ids/rects it produced, not a bare pass.

**Manual/visual** (operator, not agent — the app is never driven from here):
raise three notifications from a handler; confirm vertical stacking, correct
growth direction per anchor, reflow on dismiss, hover-pause, and that an
Embedded form's notifications land inside its ContentPane and not over the rail
or breadcrumb (AC9).

## 7. Open questions carried from the spec

Spec §11 listed five. All five are now answered — three as design consequences
below, and Q4/Q5 by the operator.

- **Q1 — form or application?** *Resolved:* the stack belongs to the **surface**,
  following D3. Navigating away disposes the form's notifications with reason
  `Programmatic`. A message about screen A has no meaning on screen B.
- **Q2 — `Show()` from a non-main form?** *Resolved:* yes — each surface owns its
  own stack, so a child form's notifications stack in that child.
- **Q3 — value substitution in `Text`?** *Resolved:* **no.** Build the string in
  COBOL (`STRING`/`MOVE`) first, exactly as every other caption works.
- **Q5 — a fourth button?** *Resolved:* a **designer-time warning** naming the
  control, and the first three render. Consistent with the ContentPane-overflow
  warning shipped at 1.62.133: tell the developer while the choice is still
  theirs; do not fail a build over a layout preference.
- **Q4 — icon style?** *Resolved (operator, 2026-09-01):* **new entries in the
  existing treatment** — 24-unit grid, single 1.5-unit stroke, fills as accents.
  The hand-drawn quality lives in the path shapes, not in a second stroke style.
  T14 is unblocked; **every question this feature opened is now answered.**

## 8. Steering compliance

- [ ] **i18n:** every new IDE string a `Tr` field in **all six** languages.
      COBOL property/method/event names and `Buttons` field values stay
      **English in every language** (the CRITICAL constraint).
- [ ] **Generated code:** declaration + handler stubs emitted with the standard
      banner; regenerate-on-action contract untouched. Generated COBOL English.
- [ ] **System KB:** property/method/event doc tables updated **and**
      `assets/knowledge/chunked.data` regenerated in the same change; a red
      `prebuilt_chunked_kb_matches_the_published_documentation` is a real failure.
- [ ] **Docs:** English `developers-guide-en.md` only; translations untouched.
- [ ] **Fix vs feature:** **FEATURE** — `features` branch, its own commit, never
      sharing with a fix; forum **f=96** prefix `[Noticia]` if ever announced.
- [ ] **Versioning:** `z` bump only, with a `CHANGELOG.md` entry.
- [ ] No "cobolt" in user-facing text.
- [ ] **A window never resizes itself** (R26) — a notification never grows its
      host surface.
