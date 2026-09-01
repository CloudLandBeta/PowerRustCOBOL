# Spec 055 — Snackbar control

- **Status:** draft → approved
- **Folder:** specs/055-snackbar-control/
- **Author:** Anthropic Claude Codex Agent (from the operator's draft specification)
  **Date:** 2026-08-31

## 1. Overview

A **Snackbar** is a transient, non-modal notification: a short message, an
optional category icon, and up to a few action buttons, shown over the form for
a few seconds and then gone. It never blocks the program and never demands an
answer — a COBOL handler raises one and carries straight on.

It is the **43rd `ControlType`**, and a **non-visual** one: the developer drops a
single Snackbar on the form, where it appears in the designer's non-visual tray
beside `Timer`, `RestClient` and `IndexedFile`. That dropped control is not the
notification — it is the **template** that carries the defaults. Each
`Show()` from COBOL raises a *new* notification from those defaults, and several
may be alive at once, stacked vertically against the surface's chosen anchor.

The operator supplied a complete draft. This spec adapts its object model to
PowerRustCOBOL's existing conventions rather than introducing a second
vocabulary for things the catalogue already names (§8 records every rename and
why).

## 2. Goals / Non-goals

**Goals**

- A notification a COBOL program raises in one statement, that disappears by
  itself.
- Five categories (Critical, Error, Warning, Info, Question) that supply sensible
  defaults — colours, icon, timeout — while every property stays overridable.
- Several notifications at once, stacking vertically, reflowing when one leaves.
- Nine anchors, resolved against the form's own **surface**, so an Embedded
  form's messages stay inside its ContentPane.
- Hand-drawn category and button icons, as catalogue vector artwork.

**Non-goals**

- **Not modal.** No Snackbar ever waits for an answer or blocks the interpreter.
- **No horizontal stacking**, ever. The stack is vertical by contract.
- **No desktop-relative placement.** A notification belongs to a surface inside
  the application, not to the physical screen.
- **No per-side borders** (`BorderTop`/`BorderRight`/…) in this iteration.
- **No `Custom` size class** — the three predefined classes only.
- **No handle returned by `Show()`** in this iteration; a raised notification is
  owned by the stack and dismissed by its timeout, its buttons, or `DismissAll()`.

## 3. User stories

- As a COBOL developer, I want to tell the operator that a save failed **without
  a dialog they must dismiss**, so the workflow is not interrupted.
- As a COBOL developer, I want a `RETRY` button on that message, so the operator
  can act on it directly.
- As a COBOL developer, I want three messages raised in quick succession to stack
  and clear themselves, so I never have to manage their placement.
- As a developer of an Embedded form, I want my notifications inside my own
  ContentPane, so they appear where the operator is looking and cannot cover the
  shell's rail or breadcrumb.
- As a designer user, I want to set the look once on the dropped control, so
  every notification the program raises already matches the application.

## 4. Settled design decisions (operator, 2026-08-31)

- **D1 — Non-visual control.** The Snackbar is placed in the designer's
  non-visual tray, not on the canvas. It has no designed X/Y/Width/Height; its
  placement at run time comes from `Anchor` and `Margin`.
- **D2 — `Show()` is a factory.** Each call raises a NEW notification from the
  control's current property values and adds it to the stack. This is what makes
  `MaximumVisible`, `OverflowBehavior`, `StackSpacing` and reflow meaningful.
- **D3 — The anchor surface is the FORM's surface, not the top-level window.**
  An Embedded form anchors inside its ContentPane; a standalone form anchors to
  its window. This matches the rule already established for backgrounds
  (1.62.132/135): the surface is not always the window.

## 5. Requirements (EARS)

### Catalogue and designer

- **R1 (ubiquitous):** The system shall add `ControlType::Snackbar` as a
  non-visual control, listed in the designer's non-visual tray.
- **R2 (ubiquitous):** The system shall provide a `control-snackbar` catalogue
  icon, so that `every_control_type_has_an_icon` passes.
- **R3 (constraint):** The system shall not render a Snackbar on the designer
  canvas at a placed position, and shall not give it designed geometry.

### Raising and dismissing

- **R4 (event):** When COBOL calls `Show()`, the system shall raise a new
  notification built from the control's current property values and add it to
  the surface's stack.
- **R5 (event):** When a notification's `Timeout` elapses, the system shall
  dismiss it with reason `Timeout`.
- **R6 (state):** While `Timeout` is `0`, the system shall not dismiss the
  notification automatically.
- **R7 (state):** While `PauseTimeoutOnHover` is true and the pointer is over a
  notification or one of its buttons, the system shall hold that notification's
  timeout, resuming it when the pointer leaves.
- **R8 (event):** When a button whose `DismissOnClick` is true is clicked, the
  system shall raise `onButtonClick` and then dismiss that notification with
  reason `Action`.
- **R9 (event):** When COBOL calls `DismissAll()`, the system shall dismiss every
  live notification of that control with reason `Programmatic`.
- **R10 (constraint):** The system shall not block the interpreter, require an
  answer, or take focus away from the form while a notification is shown.

### Stacking

- **R11 (ubiquitous):** The system shall stack simultaneous notifications
  **vertically only**, separated by `StackSpacing`.
- **R12 (state):** While `Anchor` is a Top position, the stack shall grow
  downward with the newest nearest the anchor; while it is a Bottom position, the
  stack shall grow upward with the newest nearest the anchor.
- **R13 (state):** While `Anchor` is a Center-row position, the stack shall place
  the newest first and grow downward, unless `StackOrder` overrides it.
- **R14 (event):** When a notification is dismissed, the system shall
  immediately recalculate the remaining notifications' positions so no gap
  remains.
- **R15 (state):** While the number of live notifications has reached
  `MaximumVisible`, the system shall apply `OverflowBehavior` to any further
  `Show()`.

### Placement

- **R16 (ubiquitous):** The system shall resolve `Anchor` and `Margin` against
  the form's own **surface** — the ContentPane for an Embedded form, the window
  for a standalone one.
- **R17 (constraint):** The system shall not position a notification relative to
  the physical desktop.

### Appearance

- **R18 (ubiquitous):** The system shall vertically centre all notification
  content regardless of the size class.
- **R19 (ubiquitous):** The system shall lay a notification out as
  `Margin → [icon] → gap → text → flexible space → buttons → Margin`, with the
  text taking the space between icon and buttons.
- **R20 (ubiquitous):** The system shall paint in the order: background colour,
  then background image, then content.
- **R21 (constraint):** The background image shall not affect content layout.
- **R22 (state):** While the text does not fit its size class's line budget
  (Small 1, Medium 2, Large 3), the system shall ellipsize it.
- **R23 (ubiquitous):** The system shall derive unset colours, icon and timeout
  from `Category`, and shall let any explicitly set property override them.
- **R24 (ubiquitous):** The system shall draw category and button icons from the
  catalogue as vector artwork in the deliberately hand-drawn style, on the same
  24-unit grid as every other icon.

### Engine and surfaces

- **R25 (ubiquitous):** The system shall paint notifications through the single
  render engine, so `rcrun run-form` and a compiled binary are identical.
- **R26 (constraint):** A window shall never resize itself to accommodate a
  notification.

## 6. Object model, in PowerRustCOBOL conventions

### Properties

| Property | Domain | Default |
|---|---|---|
| `Text` | free text | `""` |
| `Category` | one of: `Info` \| `Question` \| `Warning` \| `Error` \| `Critical` | `Info` |
| `Size` | one of: `Small` \| `Medium` \| `Large` | `Medium` |
| `ShowCategoryIcon` | boolean | `true` |
| `CategoryIconSize` | pixels > 0, or `0` = from the size class | `0` |
| `CategoryIconColor` | `#RRGGBB[AA]`, empty = from the category | `""` |
| `BackgroundColor` | `#RRGGBB[AA]`, empty = from the category | `""` |
| `BackgroundImage` | image path or empty | `""` |
| `BackgroundImageMode` | one of: `Fill` \| `Fit` \| `Stretch` \| `Tile` \| `Center` | `Fill` |
| `BackgroundImageOpacity` | 0–100 (percent) | `15` |
| `ForegroundColor` | `#RRGGBB[AA]`, empty = from the category | `""` |
| `FontName` | font family or empty | `""` |
| `FontSize` | points > 0 | `14` |
| `Bold` | boolean | `false` |
| `TextWrap` | boolean | `true` |
| `CornerRadius` | pixels ≥ 0 | `12` |
| `CornerRadiusTopLeft` … `BottomRight` | pixels ≥ 0, or `-1` = use `CornerRadius` | `-1` |
| `BorderStyle` | one of: `None` \| `Solid` \| `Dash` \| `Dot` \| `DashDot` | `None` |
| `BorderWidth` | pixels ≥ 0 | `1` |
| `BorderColor` | `#RRGGBB[AA]` | `#00000000` |
| `ShadowEnabled` | boolean | `true` |
| `ShadowColor` | `#RRGGBB[AA]` | `#000000` |
| `ShadowOpacity` | 0–100 (percent) | `25` |
| `ShadowBlur` | pixels ≥ 0 | `12` |
| `ShadowDirection` | degrees 0–359 | `270` |
| `ShadowDistance` | pixels ≥ 0 | `4` |
| `Timeout` | milliseconds ≥ 0; `0` = never | from `Category` |
| `PauseTimeoutOnHover` | boolean | `true` |
| `Anchor` | one of: `TopLeft` \| `TopCenter` \| `TopRight` \| `CenterLeft` \| `Center` \| `CenterRight` \| `BottomLeft` \| `BottomCenter` \| `BottomRight` | `BottomRight` |
| `Margin` | pixels ≥ 0 | `16` |
| `StackSpacing` | pixels ≥ 0 | `8` |
| `StackOrder` | one of: `Auto` \| `NewestFirst` \| `NewestLast` | `Auto` |
| `MaximumVisible` | integer ≥ 1 | `5` |
| `OverflowBehavior` | one of: `Queue` \| `DiscardOldest` \| `DiscardNewest` | `Queue` |
| `Buttons` | `id\|text\|icon\|position\|dismiss` — one button per line | `""` |

### `Buttons` — one per line

The catalogue's established shape for a collection is a multi-line string
(`Columns`, `RowHeightOverrides`, `ColumnFilters` all work this way), so a
Snackbar's buttons are too. Fields are `|`-separated; trailing fields may be
omitted:

```
retry|Retry|Retry|Left|true
close||Close|Left|true
```

- **id** — the id reported by `onButtonClick`.
- **text** — the caption; empty means an icon-only button.
- **icon** — a catalogue icon name, or empty for none.
- **position** — `None` | `Left` | `Right` (default `Left` when an icon is given).
- **dismiss** — `true` | `false` (default `true`).

Maximum **3** buttons; a fourth is a diagnostic, not a silent truncation.

### Methods

| Method | Effect |
|---|---|
| `Show()` | Raise a new notification from the current property values (D2). |
| `DismissAll()` | Dismiss every live notification of this control. |

### Events

| Event | Raised when |
|---|---|
| `onShown` | A notification has appeared. |
| `onClosing` | It is about to leave (reason available). |
| `onClosed` | It has gone. |
| `onTimeout` | Its timeout elapsed (before `onClosing`). |
| `onButtonClick` | A button was clicked; supplies the button id and index. |

Dismiss reasons: `Timeout`, `User`, `Action`, `Programmatic`, `Overflow`.

## 7. Category defaults

| Category | Timeout | Intent |
|---|---|---|
| `Info` | 4000 ms | Neutral/informational |
| `Question` | 6000 ms | A decision is being invited |
| `Warning` | 6000 ms | A potential problem |
| `Error` | 8000 ms | An operation failed |
| `Critical` | `0` (stays) | Severe; requires attention |

Each category also supplies a default background, foreground and icon. These are
**defaults, not fixed appearance** — any explicitly set property wins (R23).

## 8. What was renamed from the draft, and why

Every rename exists because the catalogue already names the thing. A second
vocabulary for the same concept is the cost this avoids.

| Draft | Here | Why |
|---|---|---|
| `TextColor` | `ForegroundColor` | The name every other control uses for its ink. |
| `TextFont` / `TextSize` / `TextWeight` | `FontName` / `FontSize` / `Bold` | The catalogue's existing font trio. |
| `BackgroundGraphic*` | `BackgroundImage*` | Matches `GridBackgroundImage`/`…Mode`. |
| `Contain` / `Cover` | `Fit` / `Fill` | `BgImageMode` already has five modes; these are those two. |
| `BackgroundGraphicOpacity = 0.15` | `BackgroundImageOpacity = 15` | Opacity is an integer percent here (`AlternatingRowOpacity`). |
| `Dashed` / `Dotted` | `Dash` / `Dot` | `GridLineStyle`'s existing vocabulary. |
| `BorderVisible` | *(dropped)* | `BorderStyle = None` already means "no border" (the RadioButton precedent). |
| `DropShadow` | `ShadowEnabled` | The existing shadow property set. |
| `ShadowOffsetX/Y` | `ShadowDirection` + `ShadowDistance` | The catalogue's shadows are polar, not cartesian. |
| `TTL` / `PauseTTLOnHover` | `Timeout` / `PauseTimeoutOnHover` | Plain English for a COBOL audience; `Interval` is the sibling. |
| `Position` | `Anchor` | `Position` reads as the designer X/Y a non-visual control does not have. |
| `WindowMargin` / `Margin*` | `Margin` | One value; per-side margins are not needed for an anchored surface. |
| `PositionRelativeTo` | *(dropped)* | D3 settles it: always the form's surface. |
| `Buttons : Collection` | `Buttons` multi-line string | The catalogue's collection shape. |
| `Hide()` | *(dropped)* | Ambiguous under D2 — which notification? `DismissAll()` says it. |
| `ContentVerticalAlignment` | *(dropped)* | Vertical centring is contract (R18), not a choice. |

## 9. Acceptance criteria

- [ ] **AC1** — `ControlType::ALL` contains `Snackbar`, and
      `every_control_type_has_an_icon` passes with `control-snackbar`.
- [ ] **AC2** — A Snackbar dropped in the designer appears in the **non-visual
      tray** and paints nothing on the canvas.
- [ ] **AC3** — `Show()` twice in one handler yields **two** notifications,
      stacked vertically, `StackSpacing` apart.
- [ ] **AC4** — With `Anchor = BottomRight`, the newest is nearest the bottom;
      with `TopRight`, nearest the top.
- [ ] **AC5** — Dismissing the middle of three closes the gap immediately.
- [ ] **AC6** — `Timeout = 0` leaves the notification up indefinitely; a non-zero
      timeout removes it and raises `onTimeout` then `onClosed`.
- [ ] **AC7** — A button with `DismissOnClick = true` raises `onButtonClick`
      with its id and index, then dismisses; `false` leaves it up.
- [ ] **AC8** — Content is vertically centred in all three size classes.
- [ ] **AC9** — An Embedded form's notification lands inside its **ContentPane**,
      not over the rail or the breadcrumb (D3/R16).
- [ ] **AC10** — Every category's default appearance is overridden by an
      explicitly set property (R23).
- [ ] **AC11** — Raising more than `MaximumVisible` behaves per
      `OverflowBehavior`, and what was dropped or queued is observable.
- [ ] **AC12** — The engine paints identically under `rcrun run-form` and a
      compiled binary (the parity guard).
- [ ] **AC13** — No window changes size because a notification appeared (R26).
- [ ] **AC14** — Text beyond the size class's line budget is ellipsized, and the
      background image never moves the content (R21/R22).

## 10. Constraints & steering check

- **i18n (6 languages):** required. Every new IDE-facing string — the toolbox
  entry, the property labels, the diagnostic for a fourth button — is a `Tr`
  field in **all six** tables. The COBOL property names, method names, event
  names and `Buttons` field values stay **English in every language** (the
  CRITICAL constraint). The developer's own `Text` is their data, untouched.
- **System KB:** mandatory in the same change. A new control with properties,
  methods and events means the `cobolt-compiler` doc tables **and** a regenerated
  `assets/knowledge/chunked.data`; the freshness test is a real gate.
- **Generated code:** codegen must emit the control's declaration and its event
  handler stubs like any other control; generated COBOL stays English.
- **Docs:** the English Developer's Guide gains a Snackbar section. Translations
  are not touched (GOLDEN RULE #8 — regeneration only on a minor/major).
- **Serialization:** a `.cfrm` stores `ControlType` by NAME, so appending a
  variant is safe — but this must be **verified**, not assumed, before the
  variant lands (`Expr` is bincode-ordinal and carries the opposite rule).
- **Fix vs feature:** **FEATURE** — a new control, beyond COBOL-85 and beyond the
  IDE's existing scope. `features` branch, its own commit, forum **f=96** with
  prefix `[Noticia]` if ever announced. It must never share a commit with a fix.
- **Version:** `z` bump only, like every agent-made change.

## 11. Open questions

- **Q1 — Does a Snackbar belong to the FORM or to the application?** D3 anchors
  it to the form's surface. If form A raises one and the operator navigates to
  form B, does it travel, stay, or die with A? Affects the stack's owner.
- **Q2 — Is `Show()` legal from a non-main form's handler?** An application opens
  child forms (051); each runs its own program. Does each get its own stack, or
  is the stack per surface and shared?
- **Q3 — Should `Text` support the interpreter's own value substitution**, or is
  the developer expected to build the string in COBOL first? The latter is
  simpler and matches every other caption.
- **Q4 — Are the hand-drawn icons new catalogue entries or a new style?** Five
  category icons plus ~11 button icons are ~16 additions; the catalogue's
  existing style is a single 1.5-unit stroke on a 24-unit grid, which is not the
  same thing as "deliberately imperfect". Confirm they are new entries in the
  existing style, or that a new stroke treatment is wanted.
- **Q5 — What is the diagnostic for a fourth button** — a designer-time warning,
  a semantic error, or silently showing the first three?
