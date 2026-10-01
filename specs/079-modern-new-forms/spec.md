<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — A new form starts modern, from a template, and stays readable

- **Status:** approved and implemented 2026-10-01 (operator: "do it")
- **Folder:** specs/079-modern-new-forms/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-01

> Operator, 2026-10-01, on the three gaps that kept forms looking like 1998: *"A
> modern default style for new forms … Responsive design on by default for new
> projects … Polished starting templates … do it"*; then *"make sure when moving
> from one theme to another, elements stay high contrasted (foreground vs
> background)"*; then *"when placing images, set ShowFrame unchecked by default"*.

## 1. Requirements (EARS)

**Modern style**
- **R1** A form shall carry a control style (`control-style="modern"` in the
  `.cfrm`), written only when set; a form without it reads as before.
- **R2** When a control is created on a modern form, it shall take the modern
  look: flat fields with a thin border, rounded corners, a light surface, dark
  text, one type size (`cobolt_forms::style`).
- **R3** `Control::new`'s defaults shall NOT change: they are the "not chosen"
  markers the painters and the theme switch read, and existing forms rely on
  them.
- **R4** The New Form dialog shall offer the modern style, on by default; a
  classic blank form stays available.

**Templates**
- **R5** The New Form dialog shall offer Blank, Record entry, List and details,
  and Dashboard.
- **R6** Every template shall be modern and responsive (anchors or flex rows),
  lay out with nothing leaving its container, overlapping or shrinking at its
  size and larger, and write its captions in the IDE's language.
- **R7** A template writes layout and look only, no event handler.

**Responsive by default (spec 056 Phase 9, R73–R77)**
- **R8** A project created now has `[forms] responsive = true` and the default
  breakpoint table; a project without the key keeps making non-responsive forms.
- **R9** Settings shall offer "New forms are responsive"; it never changes an
  existing form.

**Contrast across themes**
- **R10** After a theme or glass-style switch, every control whose text falls
  below WCAG AA (4.5:1) on the background it now sits on shall take black or
  white, whichever reads, as part of the same undoable change.
- **R11** Only backgrounds the form states are judged; a surface a theme paints
  on its own is left to the painters.

**Images**
- **R12** A PictureBox placed in the designer shall start with ShowFrame off; a
  saved form that does not state it keeps its frame.

## 2. Acceptance
- `form_templates_079` — every template round-trips, lays out at its size,
  +400×240 and 1920×1080 with no fault, reads at ≥ 4.5:1 as built and after
  every glass style.
- `modern_forms_tests_079` — a control dropped on a modern form is modern, on a
  classic form unchanged; five style switches keep every template readable.
- `style::tests` — the palette reads on itself; white on white is fixed; an
  unknown surface and a readable pair are left alone.
- `responsive_new_forms_tests_056` — R8.
- The example-corpus goldens do not move (R3).
