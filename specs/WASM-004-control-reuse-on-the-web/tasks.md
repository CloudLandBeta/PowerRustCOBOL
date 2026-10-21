<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Control reuse on the web (WASM-004)

- **Status:** in progress — the player half is done and tested; the IDE half and the desktop-comparison harnesses are not
- **Plan:** ./plan.md   **Date:** 2026-10-20

- [x] **T1 — The class table** (R2, AC1): `src/kinds.rs`; `the_table_and_the_model_agree_on_every_control` fails when `ControlType::ALL` and the table disagree.
- [x] **T2 — Class A controls as native elements** (R3, R9, R10): `src/view/controls.rs`, `data.rs`, `shell.rs`, `auth.rs`; roles, names and `tabindex` asserted natively; TabOrder -> `tabindex` (stop types only).
- [x] **T3 — Class B controls** (R4): charts (`charts.rs`), Maps (`maps.rs`), Knob / Gauge / Switch / Slider, Shape / Line, Animator, Viewer, Canvas2D.
- [x] **T4 — Layout by CSS** (R8): `src/layout_css.rs` (four modes, breakpoints); the Rust solver does not run in the browser.
- [x] **T5 — TextBox Picture** (R3, spec 004's numeric-edit): `view/controls.rs::text_box`, `view/interact.rs::text_box`, `tree.rs::picture_of`; decimal comma and currency from `SPECIAL-NAMES`. Chrome: edited form at rest, raw while focused, invalid keys refused, sign normalised on blur.
- [x] **T6 — Viewer: Markdown, HTML, text, Mermaid, PDF, Find** (R4): the parts (WASM-016); Find with Custom Highlight API; PDF page size and text. Chrome: Find `1 / 14`, wrap, match case, no `<mark>`; PDF `Page 1 of 1 (612 x 792 pt)` plus its text.
- [x] **T7 — Maps: pan, zoom, tiles** (R4): drag, wheel, buttons, keys; 6 of 6 tiles on an isolated page. Chrome: a drag moved the tiles by the drag's distance; the zoom buttons changed the zoom level.
- [x] **T8 — Knob drag and FileDropZone** (R4, R9): Chrome: knob 35 -> 50 by a drag to the top; a dropped file listed by the zone.
- [x] **T9 — Canvas2D through the Canvas part** (R4; WASM-018 R7): `methods.rs::canvas_method`, `dom/canvas.rs`, `parts/canvas`. Chrome: fills, a line and `Present` put the expected pixels (red 255,0,0,255; blue 32,96,192,255; untouched 0,0,0,0) on a 240x120 canvas, the part fetched only after the first call.
- [x] **T10 — DataGrid sort and filter on Rayon** (WASM-017 R19a): threaded variant; result equals a stable JS sort on 300,000 rows (see WASM-016).
- [ ] **T11 — Canvas2D read-backs and Canvas3D** (R4): `GetPixel`, `MeasureText`, `IsPointInPath`, `ToImage`, `Capture` are refused with the reason (a synchronous answer is not possible while the part loads asynchronously); a design that keeps the Canvas part resident from the form's open and answers them from it, or reads back through a Promise-based method, is the next step. Canvas3D has no part yet.
- [ ] **T12 — Toolbox hides class D, Check reports one** (R5, AC4): IDE.
- [ ] **T13 — Properties panel marks approximate / not-on-web properties** (R6): IDE; the player's table (`css.rs::TABLE`) carries the fidelity of each.
- [ ] **T14 — Web-only members in the closed member list** (R7): none were added by the player; any future one goes to `cobolt-ast/src/methods.rs`.
- [ ] **T15 — Property round-trip web test for every class A and B control** (R11, AC2, AC5): not run as one harness.
- [ ] **T16 — Class B equal to the desktop engine on fixed data** (AC3) and **event order desktop vs browser** (AC6): needs the shared desktop harness.
- [ ] **T17 — Keyboard-only walk and an automated accessibility check** (AC7): not run.
- [ ] **T18 — System KB tables and `chunked.data`** (R12, AC8): compiler doc tables; IDE side.

## Done criteria

The player half of R2-R4, R8-R10 has tests and Chrome runs as listed. AC1 is met. AC2, AC3, AC5, AC6, AC7, AC4 and AC8 are not met: they need the IDE (toolbox, Check, KB) and harnesses that compare the desktop and the browser, which do not exist yet.
