# Plan — The Form Designer magnifier

- **Status:** approved (operator, 2026-10-06)
- **Spec:** ./spec.md   **Date:** 2026-10-06

## 1. Approach

**Magnify what the canvas painted, not a second rendering.** The canvas paints
into one egui layer. While the magnifier is on, the canvas notes where its
shapes start in that layer and, once it has painted, copies the shapes that
fall near the pointer into a `MagnifierFeed` (the shapes, the pointer and the
form's rectangle) kept on the `DesignerPanel`. Next frame the toolbox — drawn
before the canvas in the same window — paints the feed into its square view (as wide as the
toolbox) through one transform: scale 4 about the pointer, translated so the
pointer lands on the view's centre. `epaint`'s `Shape::transform` scales
positions, clips, stroke widths, corner radii and blur, so every layer of a
rounded corner stays in agreement; text is laid out again from its
`LayoutJob` at 4× instead of being stretched, so it is sharp (R6). Every
clip is intersected with the view and with the form's rectangle, so only the
form is shown (R5).

The canvas is not touched otherwise: with the magnifier off, nothing is
copied and nothing is drawn (R7, AC6). The one-frame delay is invisible; the
canvas asks for one more frame whenever the pointer moved, so the view
catches up with the last position.

**Toolbar (R1):** a `ToggleMagnifier` action and button beside Grid/Glass.
**Toolbox (R2, R9):** `ToolboxPanel::show` takes the feed (`Some` while the
magnifier is on); the Zoom section is drawn above the category scroll area,
whose height budget shrinks by the section's height, so the sidebar's total is
unchanged.

## 2. Affected crates / files
- `crates/cobolt-ide/src/panels/magnifier.rs` (new) — `MagnifierFeed`,
  `capture`, `paint`, and the text re-layout. Unit-tested.
- `crates/cobolt-ide/src/panels/designer.rs` — `magnifier_on`,
  `magnifier_feed`; capture at the end of the canvas pass; toolbar button and
  action.
- `crates/cobolt-ide/src/panels/toolbox.rs` — the Zoom section.
- `crates/cobolt-ide/src/app.rs` — pass the feed to the toolbox; dispatch the
  toggle; the tooltip.
- `crates/cobolt-ide/src/i18n.rs` — `tb_magnifier`, `cat_zoom`,
  `magnifier_hint` ×6.
- `docs/developers-guide-en.md` — "The magnifier"; translations deleted.

## 3. Data / model changes
None: view state of one designer window.

## 4. Key decisions & alternatives
- **Copy the canvas's own shapes** — exactly what the canvas shows, overlays
  included, at no cost when off. Rejected: rendering the form a second time at
  4× (would need a scale in the engine — see the withdrawn first draft — and
  could drift from the canvas); reading back pixels (blurry, needs GPU
  readback).
- **Section above the scroll area** — always in view, and its height is
  accounted for exactly.

## 5. Risks & mitigations
- *Large forms copy many shapes* → only shapes whose bounds touch the
  magnified square (± a margin) are copied.
- *Text re-layout cost* → at most the few text shapes near the pointer; egui
  caches galleys.

## 6. Test strategy
- `magnifier` unit tests: a rect, a stroke and a radius scale by 4 about the
  pointer; a 9 px text becomes a 36 px galley at the view's centre; shapes
  outside the form are clipped away; the empty feed paints nothing.
- Designer/toolbox tests: the toggle starts off and is not an edit (AC1, AC5);
  the Zoom section is first and absent when off, sidebar height unchanged
  (AC2); a captured feed near a label magnifies it at 36 px (AC3).
- `cargo test -p cobolt-forms --features render` unchanged (AC6).
