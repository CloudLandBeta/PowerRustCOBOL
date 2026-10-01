<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Obsolete scaling style: proportional resize, reposition and font (`ObsoleteScalingStyle`)

- **Status:** approved (operator, 2026-10-01; Q1–Q5 settled in §7)
- **Folder:** specs/081-obsolete-scaling-style/
- **Author:** Anthropic Claude Codex Agent, for the operator   **Date:** 2026-10-01

## 1. Overview

A developer coming from Fujitsu PowerCOBOL is used to one form-level setting
that makes a whole form follow its window in proportion. The setting can:
- resize the controls;
- move them;
- scale their fonts;
- or do any combination of those.

PowerRustCOBOL's responsive engine (spec 056) does something different, and
does it better for new forms:
- **Anchors** keep each control's margins.
- **Containers** distribute space.
- **`FontScaling = Fluid`** follows the window's width.

Reproducing the PowerCOBOL behaviour with those tools means a Grid of `fr`
tracks or per-control tuning. Nothing a migrated form can switch on in one
place gives "multiply every rectangle by the window ratio".

This feature adds that switch: a form property shown in the designer as
**Obsolete scaling style** (`ObsoleteScalingStyle` in the `.cfrm` file and
in COBOL). The name says what it is (operator, 2026-10-01): a compatibility
mode for migrated forms, not the recommended way to build a new one. Anchors,
containers and `FontScaling` remain the modern tools. Its values
are the familiar combinations of resize, reposition and resize-the-font. It is
solved by the same single layout engine, so the designer, Run Form, child
forms and the compiled binary all scale identically.

## 2. Goals / Non-goals

**Goals**
- A migrated form behaves as its developer expects with one property: every
  control scales and/or moves with the window, and fonts follow if asked.
- One engine, every surface. `ObsoleteScalingStyle` is a mode of the spec-056 solver,
  not a second layout path.
- The behaviour is predictable and documented in our own words, with exact
  arithmetic.
- It works with what already exists:
  - the form minimum (`MinFormWidth`/`MinFormHeight`);
  - the collision limits (1.80.33);
  - the system text-size factor;
  - per-control `ScaleFont`;
  - `MinFontSize`/`MaxFontSize`.
- Settable in the designer, and readable and settable from COBOL at run time.

**Non-goals**
- Importing PowerCOBOL form files. This spec adds the behaviour; an importer
  is separate work.
- Reproducing any vendor's undocumented rounding or quirks. The arithmetic is
  ours, and stated here.
- Changing the spec-056 behaviour of a form whose `ObsoleteScalingStyle` is `0`
  (every existing form).
- Per-control scaling modes. The setting is form-level, as the developers it
  serves expect (see Q3).

## 3. User stories

- As a developer migrating a PowerCOBOL application, I want to set one
  property on a form and have it scale with its window the way it did
  before, so that my users see the same behaviour without my re-laying-out
  every screen.
- As that developer, I want to choose whether controls resize, move, or both,
  and whether the text grows, so that I can match each form's original
  setting.
- As a COBOL programmer, I want to read and change the setting while the
  program runs (e.g. a "zoom" option), so that the behaviour can depend on
  the user.
- As a developer, I want the designer's view-size grip to show exactly what
  the running form will do, so that I can check a form without running it.

## 4. Requirements (EARS)

**The property**
- **R1 (ubiquitous):** A form shall have a property `ObsoleteScalingStyle`, an integer
  read as a set of flags:
  - **1 = resize** the controls;
  - **2 = reposition** them;
  - **4 = resize the font**.
  
  The default is **0** (no scaling: the spec-056 behaviour, unchanged).
- **R2 (ubiquitous):** The designer's Properties pane shall offer the property
  under the label **Obsolete scaling style** (translated in all six
  languages), with a tooltip that recommends anchors and containers for new
  forms. It is a list, labelled in the IDE's language:
  - 0 – None
  - 1 – Resize only
  - 2 – Reposition only
  - 3 – Resize and reposition
  - 4 – Resize the font only
  - 5 – Resize and resize the font
  - 6 – Reposition and resize the font
  - 7 – Resize, reposition and resize the font
  
  All eight values are listed (Q1).
- **R3 (constraint):** `ObsoleteScalingStyle` shall be written to the `.cfrm` only when it
  is not 0. A form saved before this feature shall load with 0 and lay out
  exactly as before.

**Activation**
- **R4 (state):** While a form's `ObsoleteScalingStyle` is not 0, the form shall be laid
  out by the responsive engine, whether or not `Responsive` is on. Setting
  `ObsoleteScalingStyle` in the designer shall not change the stored `Responsive`
  value.
- **R5 (state):** While `ObsoleteScalingStyle` is not 0, every control shall be
  scaled (R6–R9) **except** one the developer anchored or docked on purpose.
  That control keeps its spec-056 anchoring or docking (Q3).
  - Every control stores an `Anchor`, because the engine seeds its type's
    default. So "on purpose" means: `Anchor` differs from the type's default
    (`Top,Left`, or `Top,Left,Right` for a MenuBar, `Bottom,Left,Right` for a
    StatusBar), or `Dock` is not `None`.
  - A migrated form, whose controls all carry the default, scales whole.
  - An explicit `Top,Left` cannot be told from the default, and scales.

**The arithmetic.** In what follows, `rx` = available width ÷ designed width
and `ry` = available height ÷ designed height, for the form's client area.
Positions are measured from the parent's client origin.
- **R6 (state):** While the **reposition** flag is set, a control's position
  shall be its designed offset times the ratio: `x' = x·rx`, `y' = y·ry`.
  While it is clear, the designed offset shall be kept.
- **R7 (state):** While the **resize** flag is set, a control's size shall be its
  designed size times the ratio: `w' = w·rx`, `h' = h·ry`.
  - The result is clamped to the control's `MinWidth`/`MaxWidth`/`MinHeight`/
    `MaxHeight`.
  - While the flag is clear, the designed size shall be kept.
- **R8 (ubiquitous):** A container's children shall be scaled within the
  container's scaled client area, using the container's own ratios. A
  container that lays out its children itself (Flex, Grid, Flow) shall keep
  doing so inside its scaled rectangle.
- **R9 (state):** While the **font** flag is set:
  - The form's font factor shall be `min(rx, ry)`, so that text grows only as
    far as both dimensions allow.
  - It is bounded by the form's `MinFontScale`/`MaxFontScale`.
  - It replaces `FontScaling` for that form, and is multiplied by the
    system's text-size factor.
  - A control with `ScaleFont = false` keeps its designed size, and
    `MinFontSize`/`MaxFontSize` still bound the result.
- **R10 (ubiquitous):** Placement shall be computed from the **designed**
  rectangles, never from a previous frame. Repeated resizing therefore never
  accumulates rounding: the spec-056 idempotence rule.

**Limits**
- **R11 (ubiquitous):** The window's minimum shall be the form's
  `MinFormWidth` × `MinFormHeight` floor, raised by the collision limits
  (1.80.33).
- **R12 (constraint):** No window size shall make two controls that are apart in
  the design touch. With both resize and reposition, uniform scaling cannot
  cause it. With resize only, growth can, and the existing collision ceiling
  stops it.

**Breakpoints**
- **R13 (state):** While a breakpoint is active on a form with `ObsoleteScalingStyle`
  not 0, its overrides shall apply to the designed values first, and scaling
  shall then apply to the result (see Q2).

**Run time**
- **R14 (event):** When a COBOL program sets the form's `ObsoleteScalingStyle`, the form
  shall lay out again in the new mode on the next frame. When the program
  reads it, the current value shall be returned. Only the values 0–7 are
  accepted; any other is refused with the usual property-error path.

**Surfaces**
- **R15 (ubiquitous):** The designer's view-size grip, Run Form, child forms,
  forms shown in a SideMenu pane, and the compiled binary shall all produce
  identical rectangles and font sizes for the same window size. This is
  guarded by a parity test.

**Documentation and KB**
- **R16 (constraint):** The property shall be added in the same change to:
  - the System KB property table, with `assets/knowledge/chunked.data`
    regenerated;
  - the property help in six languages;
  - the Guide, written for the PowerCOBOL developer: what each value does,
    the arithmetic, and when to prefer anchors instead.
- **R17 (constraint):** COBOL examples in the Guide shall use English
  identifiers and the inline built-in form. The text shall be original, not
  copied from any vendor's documentation.

## 5. Acceptance criteria

All use a form designed **400 × 300**, with a Button at (100, 50, 80, 30),
`FontSize` 12, shown at **800 × 450** (`rx` = 2, `ry` = 1.5).

- [ ] **AC1 (R1, R3)** — A form with no `ObsoleteScalingStyle` saves without the
  attribute. A form saved with 7 loads with 7. Every form in `examples/`
  lays out byte-identically before and after the change: the codegen and
  example-corpus goldens do not move.
- [ ] **AC2 (R6, R7)** — The Button lands at:
  - 1 → (100, 50, 160, 45);
  - 2 → (200, 75, 80, 30);
  - 3 → (200, 75, 160, 45).
- [ ] **AC3 (R9)** — With 5 and 7, the font is `12 × min(2, 1.5)` = 18. That
  becomes 1.5 × 12 when `MaxFontScale` is at least 1.5, and is clamped to
  `MaxFontScale` × 12 otherwise. With `ScaleFont = false` it stays 12. The
  system text factor multiplies it.
- [ ] **AC4 (R5)** — A Button anchored `Top,Right`, with style 3, follows R6/R7
  and not its anchor. Back at 0, it follows its anchor again.
- [ ] **AC5 (R7)** — `MinWidth` 100 on a Button 80 wide, at half size with style
  1, gives 100, not 40.
- [ ] **AC6 (R8)** — A Panel at (40, 40, 200, 100) with a child TextBox at
  (60, 60, 100, 20), style 3 at 800 × 450: the panel is (80, 60, 400, 150).
  The child is scaled by the panel's **client** ratios: its 2 px border
  inset makes the client (42, 42, 196, 96) designed and (82, 62, 396, 146)
  laid out, so the child is (82 + 18·396/196, 62 + 18·146/96, 100·396/196,
  20·146/96) in form space.
- [ ] **AC7 (R10)** — Resizing 400 → 801 → 400 returns every rectangle exactly
  to its designed value.
- [ ] **AC8 (R12)** — With style 1, two buttons 20 px apart in a row: the
  window's width ceiling is where they meet. With style 3 the same form has
  no ceiling.
- [ ] **AC9 (R14)** — A COBOL program sets `ObsoleteScalingStyle` to 3 and reads back 3,
  and the next frame's layout matches AC2. Setting 9 is refused, and the
  value stays 3.
- [ ] **AC10 (R15)** — The parity test lays out the AC2 form through the
  designer canvas, the run-form host and a child-form host. All three give
  identical rectangles and font sizes.
- [ ] **AC11 (R2, R16)** — The six list labels exist in all six `Tr` tables.
  The KB has the property, and
  `prebuilt_chunked_kb_matches_the_published_documentation` is green.

## 6. Constraints & steering check

- **i18n:** yes. The list labels and the property help are in six languages.
- **Generated code:** no change for forms at 0. If `ObsoleteScalingStyle` is
  reachable from COBOL through the existing property-access path (spec 010),
  no codegen is needed. Otherwise the plan says how.
- **One engine:** `ObsoleteScalingStyle` is a mode of `layout::solve`. No surface
  computes it on its own (spec 017 / 056 parity).
- **System KB:** a new property means the doc table and `chunked.data` are
  updated in the same change.
- **Docs:** a Guide section in the responsive / new-form area. The
  translations are deleted per GOLDEN RULE #8.
- **Interpreter–binary parity:** run-time setting (R14) must reach all three
  hosts. Read the `interpreter-binary-parity` skill before `/plan`.
- **Window rule:** a window never resizes itself. Scaling changes the
  layout inside the window the user sized, never the window.
- **Fix vs feature:** a **feature**. It is beyond COBOL-85, and a new form
  capability. It goes on `features`, with a z bump.

## 7. Questions — settled (operator, 2026-10-01)

- **Q1 — Values 4 and 6:** listed, with all eight values in the designer.
- **Q2 — Breakpoints:** overrides apply first, then scaling.
- **Q3 — Per-control opt-out:** yes. A control anchored or docked on purpose
  keeps that placement (R5, with the default-anchor reading above).
- **Q4 — Font ratio:** `min(rx, ry)`.
- **Q5 — Aspect ratio:** no "keep proportions" option now.
- **Deferred:** AC8 (the collision ceiling under style 1). The collision
  limits (1.80.33) are on `fixes`, and `features` cannot take them until both
  are merged into `main`. When they meet, `layout::size_limits_of` must also
  switch from `form.responsive` to `form.lays_out()`, and AC8 gets its test.
