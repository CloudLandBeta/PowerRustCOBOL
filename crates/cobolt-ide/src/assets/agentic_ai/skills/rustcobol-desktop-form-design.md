<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-desktop-form-design
description: >-
  Desktop form design rules for the PowerRustCOBOL Form Designer Agent. Use
  whenever creating, modifying, validating, or reorganizing forms: typography,
  colors, corner radius, label/control alignment, themes, Neumorphic defaults,
  tabs, containers, preservation, property validation, resizing, accessibility,
  and structured operation safety.
---

# PowerRustCOBOL Desktop Form Design

## Purpose

Use this skill whenever the Form Designer Agent creates, modifies, validates, or
reorganizes a desktop form in PowerRustCOBOL.

The objective is to produce interfaces that are visually consistent, correctly
aligned, theme-aware, usable, and safe to apply through the IDE.

## Scope

This skill applies to:

- new forms;
- existing form modifications;
- control placement;
- layout correction;
- theme application;
- tab and container organization;
- typography;
- spacing;
- border geometry;
- shadows;
- resizing behavior;
- validation of generated form operations.

## Core Rules

### Default Typography

Unless the user explicitly requests another value:

```text
Font size: 14px
```

Apply this default to:

- labels;
- buttons;
- tabs;
- input controls;
- lists;
- combo boxes;
- grids;
- captions;
- other visible form controls.

Theme defaults may still define the font family, weight, or style, but the
default font size remains 14px.

### Default Foreground Colors

Unless overridden by the user or required to maintain readable contrast:

```text
Buttons:             black foreground
Tabs:                black foreground
Data input controls: black foreground
```

Data input controls include:

- TextBox;
- NumericBox;
- ComboBox;
- ListBox;
- DatePicker;
- Slider;
- CheckBox;
- RadioButton;
- DataGrid editors.

Never apply black foreground when it would make the text unreadable against the
selected background.

### Default Corner Radius

Unless the user requests another value:

```text
Corner radius: 15px
```

Apply this to every control that supports rounded corners.

The same corner geometry must be respected by:

- borders;
- backgrounds;
- shadows;
- focus indicators;
- hover states;
- pressed states;
- selected states;
- clipping regions.

Do not render a square border or rectangular shadow around a rounded control.

## Label And Control Alignment

For vertically arranged fields, labels must be placed in a left column and their
related controls in a right column.

### Shared Input Column

All controls in the same logical group must start at the same horizontal
position.

Determine the width of the largest visible label in the group, then calculate
the input column as:

```text
Input X = Label column X + largest label width + 20px
```

The distance after the largest label must not exceed 20px.

Example:

```text
Label column X:       32px
Largest label width: 140px
Maximum gap:          20px
Input column X:      192px
```

Do not calculate a different input X position for each field.

### Vertical Alignment

Each label must be visually centered against its associated control.

For single-line controls:

- align the visible label text with the input text;
- do not rely only on matching the outer control bounds;
- account for different control heights and internal padding.

### Row Spacing

Unless another spacing is defined:

```text
Vertical row spacing: 12px to 16px
```

Use the same spacing throughout a logical group.

## General Layout Rules

- Use consistent outer margins.
- Keep related controls visually grouped.
- Use at least 20px between separate logical sections.
- Avoid excessive empty space.
- Avoid placing controls too close to form edges.
- Keep action buttons visually separated from input fields.
- Prevent control overlap.
- Prevent text clipping.
- Use consistent widths for controls serving comparable purposes.
- Size buttons according to caption, icon, font, and padding.
- Preserve clear visual hierarchy.

## Theme Awareness

Before adding or modifying controls:

1. Identify the active form theme.
2. Read the theme defaults.
3. Apply this skill's mandatory rules.
4. Preserve the theme's visual language.
5. Avoid mixing properties from unrelated themes.
6. Validate contrast and readability.

## Neumorphic Theme

When the active form theme is `Neumorphic`, use the following defaults unless
explicitly overridden.

### Background

```text
Form background:    E1E6F8FF
Control background: E1E6F8FF
```

The form and compatible control surfaces must share the same base color.

### Drop Shadow

```text
DropShadow enabled: true
Shadow opacity:     6%
Shadow distance:    7px
Blur enabled:       true
Blur strength:      8
```

The shadow must follow the control's corner radius.

Do not render:

- hard black shadows;
- rectangular shadows around rounded controls;
- glossy effects;
- metallic effects;
- excessive contrast.

### Neumorphic States

- Raised controls must appear subtly elevated.
- Pressed controls may appear recessed.
- Selected controls must remain distinguishable.
- Focus states must remain visible.
- Internal padding must remain consistent.
- Text must remain readable and centered.

### Mandatory Neumorphic Defaults

```text
Font size:       14px
Corner radius:   15px
Button text:     black
Tab text:        black
Input text:      black
Background:      E1E6F8FF
Shadow opacity:  6%
Shadow distance: 7px
Blur strength:   8
```

## Other Themes

For themes other than Neumorphic:

- use the theme's native colors;
- use the theme's native shadows;
- use the theme's native borders;
- use the theme's native visual states;
- do not apply Neumorphic colors or shadows;
- preserve the 14px default font size;
- preserve the 15px corner radius where supported, unless the theme explicitly
  requires another geometry;
- use black foreground for buttons, tabs, and input controls when readable.

## Tabs And Containers

A `TabControl` is a visual control placed on the form.

A tab is an ordered child page owned by a `TabControl`.

### Tab Rules

- Never create a tab as an independent top-level form control.
- Every tab must belong to exactly one `TabControl`.
- Controls placed on a tab must use the tab page as their parent.
- Child coordinates must be relative to the tab content area.
- The tab header is not part of the child coordinate area.
- Preserve child controls when tabs are renamed or reordered.
- Preserve tab order unless the request explicitly changes it.
- Do not remove a tab containing child controls without explicit confirmation.
- Controls on inactive tabs must remain attached to their original tab.

The same ownership rules apply to:

- Panels;
- GroupBoxes;
- Cards;
- Split containers;
- other supported container controls.

## Existing Form Preservation

Before modifying an existing form, inspect the complete control tree.

Preserve:

- control IDs;
- event bindings;
- data bindings;
- tab ownership;
- container ownership;
- user-defined properties;
- existing behavior;
- unrelated control positions.

Do not recreate a control merely to change one property.

Do not reset theme-specific properties unless required by the request.

Do not move unrelated controls.

## Property Validation

Before setting any property:

1. Confirm that the control supports the property.
2. Confirm the property type.
3. Confirm the valid value range.
4. Understand the property's effect.
5. Check interactions with related properties.
6. Reject unsupported or invented properties.

When changing corner geometry, also validate:

- border rendering;
- background clipping;
- shadow clipping;
- hover geometry;
- pressed geometry;
- selected geometry;
- focus outline;
- child clipping.

## Form Creation Procedure

When creating a complete form:

1. Identify the purpose of the form.
2. Identify the primary user workflow.
3. Divide the interface into logical sections.
4. Select the correct containers.
5. Determine the largest label in each group.
6. Calculate one shared input-column position.
7. Apply the active theme.
8. Apply the 14px font default.
9. Apply the 15px corner-radius default.
10. Apply theme-specific colors and effects.
11. Add controls in a predictable order.
12. Validate alignment.
13. Validate spacing.
14. Validate container ownership.
15. Validate property support.
16. Validate resize behavior.
17. Validate accessibility.
18. Apply the complete change set atomically.

## Resize Behavior

When the form is resizable:

- define anchors or docking intentionally;
- preserve margins;
- prevent overlap;
- allow expandable regions to grow;
- keep action buttons accessible;
- ensure tabs resize correctly;
- ensure grids resize correctly;
- respect the minimum usable form size;
- evaluate resize behavior relative to the parent container.

Do not apply anchoring or docking without considering the container hierarchy.

## Accessibility And Usability

Ensure that:

- text is readable;
- labels are associated with their controls;
- tab order follows the visible workflow;
- focus states are visible;
- disabled controls remain distinguishable;
- controls are large enough to use;
- validation messages are understandable;
- color is not the only state indicator;
- contrast remains sufficient under the selected theme.

## Structured Output Rules

When using the Form Designer protocol:

- return only the structured operations accepted by the IDE;
- do not mix explanations with the operation payload;
- do not invent control IDs;
- do not emit unsupported properties;
- do not return partial JSON;
- do not split an operation across responses;
- use valid pagination or batching when necessary;
- preserve operation order;
- validate the full change set before applying it;
- apply structural changes atomically.

If one operation is invalid, reject or roll back the complete change set.

## Validation Checklist

Before completing any form task, verify:

```text
[ ] The active theme was identified.
[ ] The default font size is 14px.
[ ] Buttons use black foreground where readable.
[ ] Tabs use black foreground where readable.
[ ] Input controls use black foreground where readable.
[ ] Supported controls use a 15px corner radius unless overridden.
[ ] Neumorphic forms use E1E6F8FF as the base background.
[ ] Neumorphic shadows use 6% opacity.
[ ] Neumorphic shadow distance is 7px.
[ ] Neumorphic blur is enabled.
[ ] Neumorphic blur strength is 8.
[ ] Shadows follow the corner geometry.
[ ] The largest label width was calculated.
[ ] Related input controls share the same X coordinate.
[ ] The gap after the largest label does not exceed 20px.
[ ] Labels are vertically aligned with their controls.
[ ] Controls do not overlap.
[ ] Text is not clipped.
[ ] Tab and container ownership are valid.
[ ] Existing IDs, events, and bindings were preserved.
[ ] Resize behavior remains usable.
[ ] Every assigned property exists.
[ ] Every assigned value is valid.
[ ] The complete change set can be applied atomically.
```

## Skill Directive

Every form must appear intentional and professionally designed.

Every control must have a clear purpose. Every position must be calculated.
Every property must be valid. Every visual effect must follow the active theme.
Every modification must preserve the structure and behavior of the existing
application.
