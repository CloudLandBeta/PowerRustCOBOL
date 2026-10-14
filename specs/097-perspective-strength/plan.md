# Plan — `PerspectiveStrength`

- **Status:** implemented in 1.91.144
- **Spec:** ./spec.md   **Date:** 2026-10-14

## 1. Approach
The projection of spec 096 is `screen = c + (x₂, y₁) · F / (D + z₂)`. The size of a flat control is `F / D` and the
convergence is the `z₂` in the divisor. **Strength scales that term**: with `m = strength / 100`,

```
screen = c + (x₂, y₁) · F / (D + m · z₂)
```

* `m = 1` (strength 100) is the formula of spec 096, so every existing form is drawn as before (R4);
* `z₂ = 0` for a control that is not tilted, so strength cannot change its size (R2), and the centre has
  `x = y = 0` so it cannot move;
* `m = 0` is `F / D` for every point: a parallel projection, the tilt only foreshortens, opposite edges stay equal (R3);
* for `0 < m ≤ 1` it is **a pin-hole camera** at distance `D / m` with focal length `F / m` (`F/(D + m z₂) = (F/m)/(D/m + z₂)`),
  so straight lines stay straight and the outline stays convex; and `D + m·z₂ ≥ D − |z₂|` for `m ≤ 1`, which spec 096
  proved positive (at least 0.29 of the larger side), so nothing is behind the camera (R6).

No mesh, clip, hook, pointer or paint-order code changes: they all consume `Camera::project`.

## 2. Affected files
- `crates/cobolt-forms/src/model.rs` — the property name, range and default; seeded for the 12 types; the range table
  `perspective_property_range`, so `property_runtime_text` clamps it with the other three.
- `crates/cobolt-forms/src/perspective.rs` — `Perspective::strength` (`new` keeps its three arguments and a default
  of 100; `with_strength` sets it), read by `Perspective::of`; `Camera` multiplies `z₂` by `m` in `project` and `divisor`.
- `crates/cobolt-ide/src/panels/properties.rs`, `prop_help_data.rs`, `ui_text_data.rs` — the row, its help and its label.
- `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data` — the System KB.
- `docs/developers-guide-en.md`, `assets/images/perspective-strength.png` — the Guide and its picture.
- Tests: `perspective_strength.rs` (new, forms), and the existing perspective tests extended where they list the
  properties (`perspective_properties`, the pane and IntelliSense tests, the three-host test).
- `version.rs` (**1.91.144**), `CHANGELOG.md`.

## 3. Decisions
- **Scale the convergence term, not the distance** — the distance already means "size and strength"; changing that
  would move every saved form. A separate property is the only compatible way to get the two apart.
- **0 … 100, default 100** — "100" is *the perspective the distance gives*, which is what every form already has;
  lowering it flattens. A range above 100 would need a bound to keep `D + m·z₂` positive (m ≤ 1.41 at the nearest
  distance) and nobody asked for it.
- **Linear in `m`** — equal steps of strength change the divisor's depth term by equal amounts; simple to reason
  about and to state in the Guide.

## 4. Risks
- *A saved form's drawn outline moving.* → AC4 compares the new projection at strength 100 with the three-property
  one over a sweep, and the corpus goldens run unregenerated.
- *The property listed on a control that cannot be tilted.* → seeded only where `has_perspective()` is true; a test
  counts the 12 and checks the others.
