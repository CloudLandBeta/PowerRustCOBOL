<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-control-properties
description: >-
  How to choose the CORRECT control property. Never invent a property name — use
  only properties that exist on the control (the CONTEXT lists them per type). Maps
  natural-language intent (in any language) to the real property, e.g. a request to
  change a control's "depth" / "profundidad" under the Neumorphic style means
  ShadowBlurStrength — there is NO `Depth` property. Documents the shadow /
  Neumorphic property group in depth. Load and obey whenever you read or write a
  control `::` property or emit a `set_property` op.
---

# RustCOBOL control properties (agent skill) — pick the real one

The single most damaging mistake you can make is to **invent a property that does
not exist** (e.g. `MOVE X TO Ctrl::Depth` — there is **no `Depth` property**). It
compiles as a `::` member access but resolves to nothing at runtime and silently
does nothing. This skill prevents that.

## Rule 1 — only use properties that exist

- Every control has a **fixed, known property set**. The CONTEXT you are given
  lists the exact property names for each control type (the "property legend").
  **That list is authoritative.** If a name is not in it, the property does not
  exist — do not use it.
- The property name in the developer's request is often a *concept*, not the
  literal API name ("depth", "make it glow", "round the corners"). **Translate the
  concept to the real property** via Rule 2 before you emit code.
- Requests arrive in any language (Spanish, Portuguese, …). Translate the *intent*,
  not the words: `profundidad` = depth, `sombra` = shadow, `redondear` = round,
  `transparencia` = transparency, etc.
- If no real property matches the intent, do **not** guess. Emit a `*>` comment
  stating what the developer asked for and that no matching property exists, and
  leave the code otherwise unchanged.

## Rule 2 — concept → real property map

| Developer asks for (any language) | Real property |
|---|---|
| **depth / relief / elevation / “raised” / “sunken” / profundidad / relieve** (Neumorphic style) | **`ShadowBlurStrength`** |
| turn the drop shadow on/off | `ShadowEnabled` |
| shadow softness / blur / feather | `ShadowBlur` (on/off) + `ShadowBlurStrength` (amount) |
| shadow colour | `ShadowColor` (`#RRGGBB`) |
| shadow offset / distance | `ShadowDistance` (px) |
| shadow direction / angle | `ShadowDirection` (compass: `NorthWest`…`SouthEast`) |
| shadow strength / darkness / opacity | `ShadowOpacity` (0–100) |
| control transparency / opacity | `Opacity` (0–100; 100 = opaque) |
| background / fill colour | `BackgroundColor` (`#RRGGBB`) |
| text / foreground colour | `ForegroundColor` (`#RRGGBB`) |
| round the corners / corner radius | `CornerRadius` (px) |
| border on/off / colour / thickness | `BorderStyle` / `BorderColor` / `BorderWidth` |
| font face / size | `FontName` / `FontSize` |
| the text shown | `Text` (TextBox) · `Caption` (Button/Label) |
| the numeric value | `Value` (Slider/Spinner/ProgressBar/NumericUpDown) |
| show/hide · enable/disable · checked | `Visible` · `Enabled` · `Checked` |
| position (x / y) | `X` / `Y` (there is **no** `Left`/`Top`) |
| size (width / height) | `Width` / `Height` |

If the exact property for an intent is not in this table **and** not in the
CONTEXT legend, treat it as non-existent (Rule 1, last bullet).

## IndexedFile non-visual control

`IndexedFile` is a non-visual control representing one project-registered indexed
file (`.cidx`). Use it for CRUD/search/navigation forms over indexed files. Do
not invent file-object properties. Use only these properties:

- `IndexedFile` — selected project indexed file path/name from the project tree.
- `OpenMode` — `INPUT` or `I-O`.
- `LoadStrategy` — `Disk` or `Memory`.
- `AutoOpen` — opens with the form and closes when the form closes/deactivates.
- `RecordName` — COBOL FD record data item used by `WRITE`/`REWRITE`.
- `KeyName` / `CurrentKeyDataItem` — key data item used by `START` and keyed reads.
- `CurrentRecordDataItem` — optional bound/current record item.
- `StatusDataItem` — file-status item updated by generated paragraphs.
- `OperatorName` — optional `REGISTERED USER` name for `OPEN`.

Generated code exposes helper paragraphs named with the control id:
`<id>-OPEN`, `<id>-START`, `<id>-READ-INVALID`, `<id>-READ-NEXT`,
`<id>-READ-PREVIOUS`, `<id>-READ-FIRST`, `<id>-READ-LAST`, `<id>-WRITE`,
`<id>-REWRITE`, `<id>-DELETE`, `<id>-COMMIT`, `<id>-ROLLBACK`, and
`<id>-CLOSE`.

**These paragraphs belong to the OUTER form program, and an event handler is a
nested program — so a handler cannot reach them.** `PERFORM <id>-OPEN` inside a
handler fails to compile ("'<id>-OPEN' is not a paragraph or section of this
program"), and there is no `<id>::Open()` method to use instead: IndexedFile
controls have no `::` methods, so an invented one is silently treated as a
property write. **Driving an IndexedFile from an event handler is not currently
supported.** Do not emit either form, and say so plainly rather than generating
code that cannot run.

CRUD/grid recipe:
1. Inspect `PROJECT TREE INVENTORY` and choose a registered `.cidx`; ask if more
   than one plausible file matches.
2. Add one `IndexedFile` non-visual control, set `IndexedFile` to that project
   entry, `OpenMode` to `I-O` for save/update/delete or `INPUT` for browse-only,
   and usually set `AutoOpen` to true.
3. Set `RecordName`, `KeyName`, and `StatusDataItem` from the indexed definition
   context when available; otherwise ask instead of guessing.
4. Add TextBox/ComboBox/etc. controls for editable record fields and a DataGrid
   for browse/list views when useful.
5. Button wiring: the CRUD verbs above run in the OUTER program and cannot be
   invoked from a button handler (see the note above). `AutoOpen` and the
   declarative data bindings still work, so a browse/grid form built on
   bindings is fine; a Save/Update/Delete button is not implementable through
   the IndexedFile control today. Tell the developer that instead of emitting a
   handler that will not compile.

## Rule 3 — the shadow / Neumorphic property group (deep reference)

These properties exist on visual controls; they drive the drop-shadow, and — under
the form's **Neumorphic** style — the soft-UI relief. Document-quality descriptions
(what · when · effect · range · style meaning):

- **`ShadowBlurStrength`** *(integer, ≈ −20 … +20, default 8)* — **THIS is the
  “depth” property.** It sets the perceived depth / relief of a control.
  - **Effect:** magnitude = how deep the effect is (the shadow halo spreads
    logarithmically with `|value|`); **sign chooses the direction of relief**:
    **negative = sunken** (pressed-in), **0 = flat**, **positive = raised**
    (embossed).
  - **When to use:** any request to change a control's *depth / relief / elevation
    / “make it look pressed or raised.”* Under Neumorphic this is the correct and
    only property for that intent.
  - **Dependencies:** the visible relief also needs `ShadowEnabled = 1` and the
    form's style set to **Neumorphic**; under other styles the value is stored but
    the visual effect differs.
- **`ShadowEnabled`** *(boolean, default off)* — master on/off for the control's
  drop shadow / Neumorphic relief. Set to `1` before expecting `ShadowBlurStrength`
  / `ShadowDistance` to show.
- **`ShadowOpacity`** *(integer 0–100, default 6)* — strength/darkness of the dark
  shadow, as a percentage.
- **`ShadowColor`** *(`#RRGGBB` string, default `#000000`)* — colour of the dark
  shadow lobe.
- **`ShadowDirection`** *(compass enum: `North`,`NorthEast`,`East`,`SouthEast`,
  `South`,`SouthWest`,`West`,`NorthWest`; default `SouthEast`)* — the light
  direction; the dark shadow falls this way, the light highlight opposite.
- **`ShadowDistance`** *(integer px, default 7)* — how far the shadow is offset
  from the control.
- **`ShadowBlur`** *(boolean, default on)* — enable the soft-blur falloff of the
  shadow (a hard shadow when off).

## Examples

Correct — change TextBox-2's depth from Slider-1 (Neumorphic):

```cobol
           SET TextBox-2::ShadowBlurStrength TO Slider-1::Value.
```

Wrong — never do this (invented property; silently does nothing):

```cobol
           MOVE Slider-1::Value TO TextBox-2::Depth.   *> NO `Depth` property
```

Round a panel's corners from a spinner:

```cobol
           SET Panel-1::CornerRadius TO Corner-Spin::Value.
```

## Before you emit any `::Property` (checklist)
1. Is the name in the CONTEXT property legend for that control (or in Rule 2)? If
   not — it does not exist; re-map the intent or leave a `*>` note.
2. Did the request use a *concept* (depth, glow, rounding)? Map it (Rule 2) — in
   particular **depth ⇒ `ShadowBlurStrength`**, never `Depth`.
3. Does the value type match the property (numeric vs `#RRGGBB` string vs boolean)
   — see the `rustcobol-types` skill.
