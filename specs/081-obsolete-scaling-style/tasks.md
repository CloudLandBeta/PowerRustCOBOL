<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Spec 081: Obsolete scaling style

- [x] T1 — Add the defaults and flags, and the `form_defaults` entry (R1, R3).
- [x] T2 — `scale.rs`: `style`, `opted_out`, `place`, `designed_rect`,
  `font_factor`, with tests for AC2–AC7 and the inverse.
- [x] T3 — `solve`: the font factor and the `Placement::Scaled` branch (R5–R10,
  R13).
- [x] T4 — The `inverse.rs` arm, and the minimum-size rule in `minsize.rs`
  (R7, R11).
- [x] T5 — `Form::lays_out`, replacing the `form.responsive` gates in the
  engine, designer, Properties pane and preview (R4).
- [x] T6 — Host: `switched_on`, `settle_layout_switch`, refusing writes outside
  0–7 with an echo back to the program, and the host test (R4, R14, AC9).
- [x] T7 — Designer, agent and resolver property lists. A Properties combo
  with 8 translated values, plus prop help in six languages (R2, R16,
  AC11).
- [x] T8 — Designer canvas test, matching the host fixture (AC10).
- [x] T9 — KB prose, `chunked.data` regenerated, and the Guide section
  (R16, R17).
- [x] T10 — AC8: `size_limits_of` uses `lays_out()`, so a scaling form takes
  the collision limits even with `Responsive` off. Under style 1, two buttons
  20 px apart stop the window at 499 (they meet at 500); under style 3 there
  is no ceiling. Done in 1.80.38, after `fixes` and `features` met on `main`.
