# Plan — the corner pin

- **Status:** implemented in 1.91.145
- **Spec:** ./spec.md   **Date:** 2026-10-14

## 1. Approach
`Perspective` gains `pin: [[i32; 2]; 4]` (TL, TR, BR, BL) and `designed: [f32; 2]` (the control's
designed size, for R4). `quad(rect)`:

1. the tilt's four corners by the pin-hole camera (as now);
2. `+ pin[i] · (rect.width() / designed_w, rect.height() / designed_h)` (R4; scale 1 when `designed` is unset);
3. if the result is not strictly convex with the same winding as the tilt's quad → use the tilt's quad (R5).

`Camera` gains `pinned: Option<Homography>`: built from the designed rectangle's corners to the final
quad (Heckbert's square-to-quad closed form: `a…h`), only when some pin is non-zero and the quad is
accepted. `project` uses it when present, else the pin-hole formula (R3: no fitted homography at zero
pins, so the old path is byte-identical). `divisor` returns the homography's denominator when pinned.

The Designer: after `render_faces_with`, keep `hit_quads` beside `control_rects`; when the primary
selection has a quad, draw four corner handles (`draw_handles`, the same look as the knobs) at the quad's
corners and store `sel_pin_quad`; at press, if the pointer is within 8 px of a corner and **not** on a
resize knob, capture `press_pin_corner`; the drag (`DragState::DraggingPinCorner { id, corner, orig_pins }`)
writes the corner's two pins live = orig + (pointer − press) / scale, refusing a fold; the finish records one
`Cmd::SetProperty` per changed pin (two at most) grouped as it can be — if the undo stack has no grouping,
two steps for one gesture is acceptable and said in the Guide; check `Cmd` for a multi-property command first.

## 2. Affected files
- `cobolt-forms/src/model.rs` — eight constants, `PIN_LIMIT`, seeding, `perspective_property_range`.
- `cobolt-forms/src/perspective.rs` — `Perspective.pin/designed`, `Homography`, `Camera`, `quad`, `is_neutral`, `of`.
- `cobolt-ide/src/panels/designer.rs` — `hit_quads` kept, corner handles, press capture, `DragState` variant, apply/finish.
- `cobolt-ide/src/panels/properties.rs`, `prop_help_data.rs`, `ui_text_data.rs`; `cobolt-compiler/src/lib.rs` + `chunked.data`;
  `docs/developers-guide-en.md` + `assets/images/perspective-pin.png`.
- Tests: `tests/perspective_pin.rs` (new), the existing perspective tests where they list properties, the pane/IntelliSense test,
  `designer_perspective_096_tests.rs` (handles + drag), `perspective_hosts_096.rs`.
- `version.rs` **1.91.145**, `CHANGELOG.md`.

## 3. Decisions
- **Offsets, not absolute corners** — a pin survives moving or resizing the control, and zero means "as the tilt says".
- **Homography only when pinned** — keeps R3 exact rather than "to 1e-4".
- **Refuse a fold rather than clamp it** — a clamp needs a geometry the developer cannot predict; refusing is explicit, and the Designer never lets a drag reach it.
- **Handles only once active (Q1)** — avoids a second handle under each resize knob of a flat control.
