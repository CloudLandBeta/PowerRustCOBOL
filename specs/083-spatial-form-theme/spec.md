# Spec — The Spatial form theme, window blur, and the Glow border style

- **Status:** approved (operator, 2026-10-02: "go directly into the
  implementation after registering the spec; it is already implemented
  correctly in the IDE")
- **Folder:** specs/083-spatial-form-theme/
- **Author:** Claude, for the operator   **Date:** 2026-10-02

## 1. Overview

The IDE gained a **Spatial** theme (1.80.90–1.80.94): dark, warm-grey
translucent glass, white text, large radii, a see-through window over the
system-blurred desktop. The operator wants the same look for the
applications they build: a selectable **form theme**, the running form's
window transparent and blurred, and a **multi-gradient border** (the
corner-glow border the Aurora and Spatial IDE themes draw) available to every
control that has a `BorderStyle`.

## 2. Goals / Non-goals

- **Goals:**
  - A procedural form theme **Spatial** in the form theme catalogue.
  - A running form under Spatial is see-through and its desktop is blurred,
    in Run Form and in a compiled application, on **macOS, Windows and
    Linux** (Linux where the compositor offers blur).
  - A new `BorderStyle` value **`Glow`**, with four colour properties for
    the four corners, on every control that carries `BorderStyle`.
- **Non-goals:**
  - Changing any other form theme's look.
  - Blur on Linux compositors that offer none (X11 without KDE, GNOME):
    the window is still see-through there, unblurred.

## 3. User stories

- As a developer, I want to choose **Spatial** for a form, so my
  application looks like a floating glass panel over the user's desktop.
- As a developer, I want to give any control a glowing border whose four
  corner colours I choose, and change them from COBOL at run time.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The form theme catalogue shall offer **Spatial**
  (id `spatial`), a self-contained procedural theme: translucent warm-grey
  glass surfaces, white text, large corner radii.
- **R2 (state):** While a form's resolved theme is Spatial, its window shall
  be created transparent and its form backdrop translucent.
- **R3 (state):** While such a window is shown, the system shall ask the
  operating system to blur the desktop under every window of the
  application: macOS (window-server background blur), Windows (DWM backdrop
  / blur-behind), Linux (the compositor's blur, through winit, where
  offered).
- **R4 (ubiquitous):** One crate, `cobolt-os-blur`, shall implement R3 for
  both the IDE and the form host, with no third-party dependency.
- **R5 (ubiquitous):** `BorderStyle` shall accept **`Glow`** on every control
  that carries it: a border whose edges are `BorderColor`, discreet, and
  whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`,
  `BorderGlowBottomRight`, `BorderGlowBottomLeft`, each fading out along the
  edges. It shall follow `CornerRadius` and `BorderWidth` like every style.
- **R6 (ubiquitous):** The four glow colours shall be ordinary colour
  properties: settable in the inspector, by `SET … TO` and by the agents;
  unset, they default to a white specular glow.
- **R7 (constraint):** A form saved before this spec shall load and save
  byte-identically: the glow properties are written only when set.
- **R8 (ubiquitous):** The System KB property tables shall document `Glow`
  and the four properties, and the shipped store shall be regenerated.

## 5. Acceptance criteria

- [ ] AC1 (R1): `spatial` is in `ThemeCatalog::builtin()` and
  `surface_theme::for_theme_id("spatial")` returns a self-contained theme.
- [ ] AC2 (R2, R3): the form host treats a Spatial form as see-through and
  requests blur; a Liquid Glass form does neither.
- [ ] AC3 (R4): `cobolt-os-blur` type-checks for `aarch64-apple-darwin`,
  `x86_64-pc-windows-gnu` and `x86_64-unknown-linux-gnu`.
- [ ] AC4 (R5): `BorderStyle = Glow` paints corner colours near the corners
  and the discreet edge colour mid-edge, on every face path
  (`every_border_style_survives_every_face_path` covers `Glow`).
- [ ] AC5 (R6, R7): the example corpus round-trips unchanged; a control
  with the four properties set paints those colours.
- [ ] AC6 (R8): the KB freshness test passes with the regenerated store.

## 6. Constraints & steering check

- **i18n:** inspector labels for the new properties go through `Tr` in all
  six tables (or reuse the property names, as other colour rows do).
- **Generated code:** no codegen change; properties travel as data.
- **Docs:** English Developer's Guide; translations follow GOLDEN RULE #8.
- **Fix vs feature:** a feature (new theme, new border style, new
  properties): `features` work, a `z` bump.
- **Parity:** the designer canvas, Run Form and the compiled binary paint
  through the one engine (spec 017) and host (spec 042).
