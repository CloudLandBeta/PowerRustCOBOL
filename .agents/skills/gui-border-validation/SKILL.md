---
name: gui-border-validation
description: Use when updating or reviewing code that modifies geometry, rendering, radius, stroke, shadow, glow, inset, padding, clipping, or any visual effect applied to a control border. This invokes the GUI Border Validation Agent as a guardian before completing the change. Focus: rounded corners, border geometry, continuous curves at corners, and that all border effects follow the configured corner radius.
---

# /gui-border-validation — GUI Border Validation Agent

Read `.agents/agents/gui-border-validation-agent.md` first, then apply
its checklist to the current change.

## Required use

Use this skill whenever code changes or reviews touch control borders, rounded corners, or related visual geometry, including:

- Any modification to border rendering, stroke, fill, rounding, or effects in paint logic.
- Changes to corner radius calculation, clamping, or application (CornerRadius / BorderRadius properties).
- Container clipping to rounded borders (_ContainerClip, picturebox_container_border, ancestor clips).
- Notch masks, glass frames, inner/outer strokes, shadows, glows, highlights, or insets that affect borders.
- Geometry changes (rect, content_rect, padding, insets) that impact where borders are drawn.
- Theme, glass style, or visual style changes affecting borders.
- Clipping, with_clip_rect, painter_at, or render order changes for bordered controls.
- New controls, user controls, or modifications to GroupBox, Panel, Button, PictureBox, or any control that draws a border.
- Changes in render.rs, paint.rs, model.rs, or theme files that could affect how top/left/right/bottom borders connect at corners when radius > 0.

## Workflow

1. Identify all affected controls and surfaces: Designer canvas, Preview, Run Form, compiled/runtime forms, and any generated code that influences visuals.
2. Run the full agent checklist against the diff.
3. Require targeted automated tests (especially for corner_radius, rounded rect drawing, clipping) and visual/manual verification (screenshots or egui test harness) for pixel-level border behavior.
4. Verify that rounded borders (radius > 0) are **continuous curved transitions** and never four independent straight segments.
5. If any regression is found in primary border segments or corner continuity, block the change and collaborate with the implementation agent(s) to correct it before proceeding.
6. Confirm no unintended side-effects on the main visual border of **any** control.

## Report

Summarize in the response:

- Which controls and border aspects were affected (e.g. GroupBox rounded border, Button inner stroke + radius, Panel container clip).
- Checklist items validated (list key ones).
- Evidence: tests run + results, specific code locations inspected.
- Any risks or required follow-up visual checks.
- Confirmation that primary borders (top, left, right, bottom + corners) remain visually correct and respect the corner radius.
- If a break was found: observed vs. expected, and steps taken with implementation agents to restore it.