<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Spec 081: Obsolete scaling style

## Engine (`cobolt-forms/src/layout`)

`ObsoleteScalingStyle` is a mode of `solve`, not a second layout path.

- **`defaults.rs`**
  - The default `OBSOLETE_SCALING_STYLE = 0`.
  - The flags `SCALING_RESIZE` / `SCALING_REPOSITION` / `SCALING_FONT` (1, 2,
    4) and `SCALING_STYLE_MAX` (7).
  - A `form_defaults()` entry. That one entry wires `.cfrm` save and load
    (written only when it is not 0), the designer's generic get/set, the
    host's `write_form_layout`, and the `me::` seed.
- **`scale.rs`**
  - `style()` reads the value: anything outside 0–7 is 0.
  - `opted_out()` is true when the anchor is not the type's default, or the
    control is docked.
  - `place()` is the arithmetic of R6–R7: offsets from the parent's client
    times `rx`/`ry`, sizes times `rx`/`ry`, clamped to the limits.
  - `designed_rect()` is the inverse.
  - `font_factor()` is `min(rx, ry)` between `Min/MaxFontScale`, times the
    system factor; a pin wins.
- **`mod.rs`**
  - The form font factor comes from `scale::font_factor` while the font flag
    is set.
  - In `place_children`'s Absolute branch, a scaled control takes
    `scale::place` and is recorded as the new `Placement::Scaled`.
  - Opted-out controls dock and anchor as before.
  - Recursion already hands each container its laid-out client (R8).
- **`inverse.rs`** gains the `Placement::Scaled` arm (designer drags and
  COBOL geometry writes).
- **`minsize.rs`**: a scaled, resized child with `MinWidth`/`MinHeight` needs
  `designed_client × min / designed size`.
- **`Form::lays_out()`** = `responsive` OR style ≠ 0. It replaces
  `form.responsive` wherever the engine is gated: `min_size_of`, the designer,
  the Properties pane and the IDE preview.

## Host (`cobolt-form-host/src/host.rs`)

- **Layout switch.** `ResponsiveSpec` gains `switched_on`, the stored
  `Responsive` switch, and `lays_out()`. `ResponsiveSpec::of` and
  `responsive_off` use `Form::lays_out()`.
- **`write_form_layout`**
  - A `Responsive` write updates `switched_on`, and every write ends in
    `settle_layout_switch`, which moves the spec between `responsive` and
    `responsive_off` as the design now says.
  - An `ObsoleteScalingStyle` write outside 0–7 is refused. The kept value is
    sent back to the interpreter, so `me::` reads it.
- **Hosts covered.** One code path serves the root window, child windows,
  pane occupants, `rcrun run-form` and the compiled binary.

## IDE

- **Designer:** `FORM_PROP_KEYS` and the property-list test.
- **Properties pane:** a translated combo of the eight values, copying the
  `ModalOverlayStyle` pattern. It sits above the `Responsive` early return,
  so it shows on every form.
- **`agent.rs`:** `form_property_valid`.
- **`cobolt-semantic` resolver:** `UNIVERSAL_FORM_PROPS`.
- **`i18n.rs`:** 9 keys in six languages.
- **`prop_help_data.rs`:** a Form entry in six languages.

## Docs and KB

- **System KB:** the `cobolt-compiler` `form_layout_and_events.md` prose.
  `chunked.data` is regenerated.
- **Guide:** "Migrating a PowerCOBOL form: *Obsolete scaling style*".
