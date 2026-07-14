<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-layout-designer
description: >-
  Rules for Form Layout, Alignment, Theming, and Visual Aesthetics. Explains how to position controls geometrically (X, Y, Width, Height) to avoid overlaps, align labels with inputs, manage TabOrder, and style the application with themes and colors since the framework uses absolute positioning instead of automatic layouts (like flexbox/grid).
---

# RustCOBOL Form Layout & Aesthetics (Agent Skill)

PowerRustCOBOL forms use an **absolute positioning** coordinate system. There is no auto-layout, flexbox, or grid system. As a designer, you are mathematically responsible for computing `X` and `Y` coordinates, preventing overlaps, and establishing an elegant, user-friendly layout.

## Rule 1: Absolute Positioning and Alignment

- **X, Y, Width, Height**: Every control is a bounding box. You must calculate these values manually using `SetProperty` operations.
- **Absolute Form Coordinates**: `X` and `Y` are ALWAYS relative to the top-left of the **entire form**, NEVER relative to the parent container! Even if you set `Parent` to a TabControl or Panel, the `X` and `Y` values MUST still be absolute form coordinates. E.g., if a TabControl is at X:200, Y:200, its child MUST be placed at X:220, Y:240 to be visible inside it. If you use relative coordinates, the control will be clipped and invisible.
- **Vertical Flow**: To stack controls vertically, calculate the next Y position as: `Y_new = Y_previous + Height_previous + Padding`. A standard vertical padding is 10px to 20px.
- **Horizontal Flow**: To put controls side-by-side, calculate: `X_new = X_previous + Width_previous + Padding`. 
- **Alignment**:
  - **Left-aligned**: Multiple controls are left-aligned if they share the exact same `X` coordinate.
  - **Top-aligned**: Multiple controls are on the same line if they share the exact same `Y` coordinate.

## Rule 2: Classic Form Pattern (Labels Left, Inputs Right)

When building data-entry forms or setting parameters, follow this strict visual pattern:
- **Labels** always go on the left (e.g., `X: 20`).
- **Inputs/Controls** (TextBoxes, Spinners, Sliders) go on the right (e.g., `X: 150`), aligned horizontally on the same line as their respective label (identical `Y`).
- If you have charts, buttons, or large visualizations, you can put them on the same line if space permits, or spanning the full width underneath.
- Example for a two-field form:
  - Label 1: `X: 20`, `Y: 20`
  - Input 1: `X: 150`, `Y: 20`
  - Label 2: `X: 20`, `Y: 60`
  - Input 2: `X: 150`, `Y: 60`

## Rule 3: Prevention of Overlaps and Spacing

- **No Overlapping**: Never place a control on top of another control unless you are explicitly placing child controls inside a container (like a `GroupBox` or `Panel`).
- **Form Resizing**: If your computed `Y` exceeds the form's `Height`, you must issue a `SetProperty` on the Form itself to increase its `Height`. The Form has no ID, or its ID is the root context. Ensure the canvas is large enough to contain all controls.

## Rule 4: Tab Order and Z-Order

- **TabOrder**: Defines keyboard navigation. The top-left control should have `TabOrder: 1`, the next input `TabOrder: 2`, and so on. Always assign sequential tab orders top-to-bottom, left-to-right.
- **ZOrder**: Controls stacking context. If placing a control inside a container visually (without structural reparenting), ensure the inner control has a higher `ZOrder`.

## Rule 5: Themes, Colors, and Visual Abilities

- **Colors**: Use hex strings (`#RRGGBB`) for `BackgroundColor` and `ForegroundColor`.
  - Prefer harmonious, modern color palettes (e.g., dark mode schemes with `#1E1E1E` backgrounds and `#4DA8DA` accents).
- **Corner Radius**: Modern UIs rarely use sharp corners. Set `CornerRadius` (e.g., 6 to 12) for panels, buttons, and text boxes.
- **Neumorphism (Soft UI)**: As described in `rustcobol-control-properties`, you can simulate modern "soft UI" or 3D reliefs by setting `ShadowEnabled` to 1, and manipulating `ShadowBlurStrength` (negative for sunken inputs, positive for raised buttons).
- **Themes**: If asked to apply a "theme", adjust the BackgroundColor/ForegroundColor uniformly across controls, and establish a consistent `CornerRadius` and `ShadowOpacity`.

## Golden Rules Summary for Operations:
If a user says "organize the controls" or "align them":
1. Gather all controls in the context.
2. Mathematically recompute their `X` and `Y` properties.
3. Emit a batch of `set_property` operations assigning the new `X` and `Y` values.
4. Update their `TabOrder` based on the new vertical positions.
