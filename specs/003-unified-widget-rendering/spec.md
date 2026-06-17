# Spec — Unified Form Widget Appearance & Rendering

- **Status:** draft
- **Folder:** specs/003-unified-control-rendering/
- **Author:** Grok (for xAI)   **Date:** 2026-06-16

## 1. Overview

PowerRustCOBOL's Form Designer implements rich, frosted-glass "liquid" visuals for all controls (custom pill tracks and thumbs with radial lens highlights and sheens for Sliders, draw_glass frames, shadow layers, tick styling, chart previews, animator, picture boxes, glass combo headers, etc.). These are the source of truth for "what a form looks like."

However, the live RAD Preview, Run Form windows (via `show_running_form_window` + `render_run_control`), and the standalone binary form runner (inlined `FormApp` code generated inside `cobolt-compiler`) use a mixture of native `egui::Slider`, `egui::ProgressBar`, `egui::TextEdit`, plain rect/text fallbacks and partial custom paint. The result is visual drift: a Slider designed with glass styling loses its track/thumb/lens/tick treatment at preview and runtime.

The goal of this feature is to add an explicit **spec for the appearance of controls**, drive all graphical rendering from a single shared implementation derived from the designer, and guarantee that designing, previewing (RAD), interpreting (live), and running (compiled binary) produce matching appearance, while ensuring that no designer-only development affordances (selection chrome, geometry-editing hooks, dev animation state, etc.) leak into non-designer contexts.

## 2. Goals / Non-goals

### Goals
- Create a documented, testable **Widget Appearance Specification** (initially captured in the developers guide, backed by the designer implementation) that defines the exact visual treatment (glass parameters, shapes, highlights, colours derived from properties, disabled/alpha states, etc.) for every `ControlType`.
- Make the Form Designer's graphical element rendering code (`draw_control`, `draw_glass`, `draw_glass_circle`, `draw_glass_pill` / lens helpers, `draw_picturebox`, `draw_animator`, `draw_chart_preview`, `glass_combo_header`/`popup`, and supporting paint utilities) the single source of truth.
- Ensure identical visuals for a given set of control properties + runtime state (value, enabled, alpha) across:
  - Form Designer canvas (non-dev layers)
  - RAD live preview / inspector previews
  - Live interpreter Run Form windows
  - Compiled self-contained binaries produced by `cobolt-compiler`
- Provide an explicit mechanism (parameter, wrapper, or split entry points) so that designer-only behaviour (selection outlines/handles, in-paint geometry mutation, dev-only animation preview hooks, rubber-band affordances, etc.) can be disabled for all other renderers.
- Update the three current rendering sites (designer canvas + IDE preview/run paths + compiler template) to converge on the shared renderer.
- Keep or improve WYSIWYG claims in documentation.

### Non-goals
- Redesigning or altering the frosted-glass aesthetic itself (source of truth stays the designer today).
- Unifying *input handling and state mutation* (native Sense/interact or egui controls for hit testing may stay; only the pixels drawn for the graphical face must match).
- Bit-for-bit identical pixels across OS / font rasterisers / egui versions (reasonable visual equivalence is sufficient).
- Adding new controls or properties.
- Changing how forms are modelled or serialised (`cobolt-forms` model stays the authority for *data*, the new paint code for *appearance*).
- Moving non-paint dev tooling (lasso, alignment, z-order UI, undo stack) — they remain designer-only.

## 3. User stories
- As a **RAD developer**, when I design a Slider with a blue thumb, custom ticks on both sides and a glass track, I want the exact same appearance in the designer canvas, the Preview button, the Run Form window, and the final executable so there are no surprises at delivery.
- As a **maintainer / themer**, I want a single place that draws every control's pixels so that a glass-effect improvement, shadow tweak, or new disabled style automatically applies everywhere without copy-paste drift.
- As a **user of compiled binaries**, I expect forms to look the same whether I am iterating in the IDE or have shipped the app.
- As a **documentation reader**, the control catalogue and "WYSIWYG" statements must be factually true.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall treat the graphical rendering routines that exist today inside the Form Designer as the authoritative definition of control appearance.
- **R2 (event-driven):** When a form control is painted for any purpose other than live design editing, the system shall invoke the designer-derived paint path (with dev-only features inactive) rather than a separate native-egui control or ad-hoc painter.
- **R3 (state):** While the context is the Form Designer canvas, the system may activate additional affordances (selection rectangles, handle drawing, geometry side-effects inside paint helpers, etc.) on top of or around the core appearance renderer. Outside the designer those affordances shall be absent.
- **R4 (constraint):** The system shall not regress the interactive behaviour of controls (value changes, clicks, drags) while unifying their visuals.
- **R5 (constraint):** The compiler-generated form runtime code embedded in a self-contained binary shall produce visuals that match the IDE RAD preview for the same form properties and state.
- **R6 (optional, where glass mode is on):** Where a control's `glass` / theme styling is active, the rendered pixels (track, thumb, body, highlights, shadows, text) shall follow the exact construction currently present in `draw_control` / `draw_glass*` (pill rounding, mesh sheens, radial lens, layered soft shadows, alpha multiplication, property-driven colours, etc.).
- **R7 (constraint):** No new hard-coded English UI strings shall be introduced for this feature (i18n rules apply only to user-facing text; none is required here).

## 5. Acceptance criteria
- [ ] AC1 — Slider (horizontal and vertical, all TickStyle / Orientation / ShowValue combinations) renders with identical glass track pill (sheen + rim), thumb pill (lens highlight), tick marks and labels in designer, RAD preview, Run Form, and a freshly built binary.
- [ ] AC2 — All controls that currently bypass `draw_control` / designer helpers in `render_run_control`, `show_running_form_window`, or the compiler `FormApp` (Slider, ProgressBar, native TextEdit in some cases, etc.) now route their graphical face through the shared designer paint code (interaction layer may remain).
- [ ] AC3 — The public signature / call sites for core paint functions clearly separate "pure appearance" from "designer editing chrome"; calls from preview / binary paths pass parameters that disable editing features.
- [ ] AC4 — Changing a detail inside the designer's glass drawing (e.g. the lens mesh constants, a track width formula, or shadow falloff) visibly affects a live preview window and a recompiled binary without touching any other render code.
- [ ] AC5 — The claim "Preview and Run Form draw each control with the same renderer the designer canvas uses" in the developers guide is accurate and can be demonstrated for the full control catalogue.
- [ ] AC6 — `cargo test -p cobolt-ide` and `cargo build -p cobolt-compiler` (and full workspace tests) pass; no behavioural change for existing designed forms except the now-consistent visuals.
- [ ] AC7 — Steering constraints satisfied (generated-code contract for the compiler template, English docs only, no new i18n literals).

## 6. Constraints & steering check
- **i18n (6 languages) impact?** None. No new user-facing translatable strings; appearance is visual + property-driven. Any status messages stay within existing Tr keys if needed.
- **Generated-code / regenerate contract impact?** Yes — the inlined form-runner painting code inside `cobolt-compiler/src/lib.rs` (the big `form_runtime_code` string) is effectively generated runtime UI. Changes must keep the "regenerate on Build/Run/Debug" spirit and the banner comments. The compiler must continue to emit correct runnable code.
- **Docs (English guide) update needed?** Yes. Sections 7 (Form Designer RAD), 8 (control catalogue), possibly 20 (Appearance). Strengthen the WYSIWYG paragraph and add a note about the unified renderer. The registry in `specs/steering/docs.md` will require a new or updated row in the `/docsync` phase.
- **Fix vs feature classification:** Consistency / quality-of-life fix for a previously aspirational WYSIWYG promise; treated as a feature for versioning (minor) and changelog purposes.
- **Other steering:** Must honour `tech.md` build/test discipline (build + test touched crates). Paint code should remain pure-Rust / egui only (no new external deps).

## 7. Open questions
- Q1: Exact module home for the extracted pure renderer? `cobolt-forms::paint` (behind an "egui" or "render" feature so the model crate stays lightweight) or a new internal `cobolt-forms-render`? (plan will decide).
- Q2: For fully-interactive controls (Slider drag, TextBox caret, etc.) do we keep a thin egui control wrapper around the painted face, or implement custom `Sense` + manual drag logic on top of pure paint? Goal is identical pixels; interaction fidelity must not regress.
- Q3: Do we need a small "render context" / trait object or just booleans (`selected`, `interactive`, `dev_mode`) passed to the paint entry points?
- Q4: Should the appearance spec be emitted as data (JSON table of glass params per control) in addition to "the code is the spec"?
- Q5: Any impact on PDF export or other non-egui renderers of forms?

---

**Next step:** When this spec is approved, run `/plan` to produce the design (extraction strategy, call-site changes, binary codegen impact, test approach). Do not begin implementation until the plan and tasks are approved.