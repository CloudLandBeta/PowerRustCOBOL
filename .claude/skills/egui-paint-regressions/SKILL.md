---
name: egui-paint-regressions
description: Post-mortem playbook for NON-corner egui upgrade regressions in PowerRustCOBOL — self-inflating Resize windows/modals and self-closing popups — plus the upgrade checklist. For anything at rounded corners (dark/light arcs, crescents, bleed) use the dedicated `rounded-corners` skill instead. Read when a window/modal grows on its own, when a popup opens and instantly closes, or before ANY future egui version bump.
---

# egui paint & UI regressions — what broke in 0.29→0.35 and how it was fixed

All three regression families below shipped during the egui 0.35 upgrade
(spec 027, July 2026), each looked mystifying on screen, and each has a
one-line root cause once you know it. **Do not re-derive these from scratch.**

## 1+2. Corner artifacts → the `rounded-corners` skill

Everything about rounded-corner rendering — the layered corner system, the
u8-radius / `StrokeKind::Inside` story, the notch-mask guardian, the GL clip,
and the shape-dump diffing workflow — lives in the dedicated
**`rounded-corners`** skill. Corners are the recurring problem class in this
codebase; that file is their single source of truth.

## 3. Self-inflating windows/modals (Resize ratchet)

**Symptom:** a resizable window/modal grows every frame until it fills the
screen.

**Root cause:** egui ≥ 0.35 `Resize` does
`desired_size = desired_size.max(measured_content_min)` **every frame**. Any
body that overflows the box — e.g. a layout with an *estimated* footer height
that font-metric changes (skrifa, 0.34) made 2px too small — ratchets forever.

**Fix:** never estimate interior heights. Partition the fixed box with
embedded panels (`egui::Panel::bottom` for the button row, `CentralPanel` for
the scrollable content) so measured content == box **exactly** regardless of
font metrics. See `error_modal_scaffold`/`error_modal_body_ui` in
`cobolt-ide/src/app.rs` and the 120-frame test
`error_modal_holds_seeded_size_across_frames`.

The older sibling rule still holds (memory `egui-resize-autogrow`): never size
a child from available/remaining space inside an auto-sizing container.

## 4. Popups that open and instantly close

**Symptom:** a hand-rolled popup (raw `Area` + manual open state) opens on
click and closes by itself one frame later (e.g. the properties color picker).

**Root cause:** since egui 0.32 the popup manager **force-closes any popup id
that is not re-registered through the `Popup::show` API each frame**.
Registering via `Popup::toggle_id`/`is_id_open` but drawing a raw `Area`
counts as "not shown" → killed after one frame.

**Fix:** either use the real `egui::Popup` API end-to-end, or keep the popup's
open flag **yourself** (a bool in `ui.memory` temp data) and don't touch the
popup manager at all. PowerRustCOBOL's color picker does the latter — see
`color_edit_button_closing` in `cobolt-ide/src/panels/properties.rs`.

## Checklist for the next egui bump

1. Step one minor at a time; fix deprecations at each step (0.34 deleted all).
2. grep for `shrink(half)`/`- half` near `rect_stroke` — any survivor is a
   corner-bleed candidate; convert to `StrokeKind::Inside`.
3. Run the `shape_dump` scenes against the previous version before trusting
   your eyes.
4. Re-run the modal 120-frame test and the concentric-arc guard.
5. Check every hand-rolled `Area` popup still opens.
