# Plan — egui-elegance theme & widget integration

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-27

## 1. Approach

The implementation is split into three phases:

### Phase 1: egui 0.29 → 0.34 upgrade (R1–R3)

The largest risk. egui 0.34 is 5 minor versions ahead with breaking API changes:

**Key API changes across 0.29→0.34:**
- `Response::drag_released()` → `drag_stopped()` (already warned)
- `Ui::allocate_ui_at_rect()` → `allocate_new_ui()`
- `Ui::set_enabled()` → `disable()` / `add_enabled_ui()`
- `Rounding` struct changes (fields may differ)
- `Memory` API changes (areas, data access)
- `Sense` API changes
- `TextEdit` API evolution
- `Frame` / `Margin` constructor changes
- `epaint::Mesh` vertex/index API may differ
- Font loading API changes
- Viewport API changes

**Approach:** Jump directly to 0.34. Search-and-fix all compiler errors.
Update `Cargo.toml` workspace dependencies, then fix each crate bottom-up
(cobolt-forms first, then cobolt-ide).

### Phase 2: elegance dependency + theme chooser (R4–R8)

- Add `egui-elegance = "0.10"` to `cobolt-ide/Cargo.toml`.
- In `app.rs`: call `Theme::slate().install(ctx)` (or the selected theme)
  at the start of each frame based on a stored setting.
- Add the 4 elegance themes to the theme catalogue (`theme.rs`).
- In `render.rs`: when the active form theme is an elegance theme, dispatch
  control rendering to elegance widgets instead of the glass renderer.
  Use a trait-dispatch or match to map `ControlType` → elegance widget.

### Phase 3: IDE chrome + extras (R9–R12)

- Replace IDE panel widgets (properties, toolbox, modals) with elegance
  equivalents. This is incremental — one panel at a time.
- Add Toast notifications for build/save feedback.
- Wire the menu editor to use elegance Modal.

## 2. Affected crates / files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Bump `egui` to 0.34, add `egui-elegance` |
| `crates/cobolt-forms/Cargo.toml` | Bump `egui` optional dep to 0.34 |
| `crates/cobolt-ide/Cargo.toml` | Bump `eframe`/`egui_extras` to 0.34, add `egui-elegance` |
| `crates/cobolt-forms/src/paint.rs` | Fix all egui 0.34 API changes (~50 call sites) |
| `crates/cobolt-forms/src/render.rs` | Fix API changes + add elegance widget dispatch |
| `crates/cobolt-forms/src/icons.rs` | Fix any `epaint` API changes |
| `crates/cobolt-ide/src/app.rs` | Theme chooser, elegance install per frame |
| `crates/cobolt-ide/src/panels/*.rs` | Fix API changes, replace widgets with elegance |
| `crates/cobolt-ide/src/i18n.rs` | 4 new theme name strings × 6 languages |
| `crates/cobolt-forms/src/theme.rs` | Add elegance theme entries to catalogue |
| `docs/developers-guide-en.md` | Themes section update |

## 3. Data / model changes

### Form model

The `Form::theme` field (currently `Option<String>`) gains 4 new valid values:
`"slate"`, `"frost"`, `"charcoal"`, `"paper"`. The existing `None` / asset-pack
values continue to work.

### IDE settings

New persistent setting: `ui_theme: String` (default `"default"`, options
`"default"`, `"slate"`, `"frost"`, `"charcoal"`, `"paper"`).

### No migration needed

Existing forms with `theme: None` or an asset-pack theme are unaffected.

## 4. Key decisions & alternatives

**D1: egui upgrade strategy — direct jump vs incremental**
- Decision: Jump directly from 0.29 to 0.34.
- Why: Intermediate versions serve no purpose; each would require its own
  migration. One big migration is more efficient.
- Rejected: Incremental (5 separate migrations).

**D2: elegance as a dependency vs porting its themes**
- Decision: Use elegance as a crate dependency.
- Why: Full widget library, maintained upstream, consistent styling.
- Rejected: Porting just the color palettes — loses the widget polish.

**D3: MSRV bump to 1.92**
- Decision: Accept the MSRV bump.
- Why: Required by egui-elegance. Rust 1.92 is current stable.
- Rejected: Pinning to an older elegance version — none exist for egui 0.29.

**D4: Elegance theme active → skip glass rendering**
- Decision: When an elegance theme is active, bypass the glass `draw_control`
  pipeline entirely for mapped controls and render with elegance widgets.
- Why: Mixing glass rendering with elegance styling would look inconsistent.
- Rejected: Layering elegance colors onto the glass renderer — visual clash.

## 5. Risks & mitigations

- **Risk:** egui 0.29→0.34 upgrade breaks many things across ~15 files.
  → **Mitigation:** Fix compiler errors methodically crate by crate. The
  existing test suite catches regressions. Budget 2–4 hours.

- **Risk:** egui-elegance widgets don't map 1:1 to PowerRustCOBOL controls.
  → **Mitigation:** Only map controls with clear equivalents (Button→Button,
  TextBox→TextInput, etc.). Unmapped controls fall back to glass rendering
  even under an elegance theme.

- **Risk:** MSRV 1.92 may not be available on all build targets.
  → **Mitigation:** Rust 1.92 is current stable; CI uses latest.

- **Risk:** elegance's `MenuBar` widget may not support our `MenuDefinition`
  data model directly.
  → **Mitigation:** Map `MenuDefinition` items to elegance `Menu`/`MenuItem`
  at render time. The data model stays the same.

- **Risk:** Performance impact of elegance widget rendering.
  → **Mitigation:** Elegance widgets are immediate-mode egui — same perf
  model. No extra allocations or retained state.

## 6. Test strategy

### Phase 1 (egui upgrade)
- `cargo test` across all crates — must match pre-upgrade results.
- Manual: launch IDE, open a form, run preview, verify glass rendering.

### Phase 2 (elegance integration)
- Manual: switch IDE theme to Slate → UI changes. Switch form theme to Frost →
  runtime controls render with elegance styling.
- Manual: verify Liquid Glass forms are unchanged.
- Manual: verify elegance MenuBar renders from MenuDefinition.

### Phase 3 (IDE chrome)
- Manual: verify properties panel uses elegance widgets.
- Manual: verify Toast shows on build success.

## 7. Steering compliance

- [ ] i18n: 4 new theme name strings in 6 languages
- [ ] Generated-code banner + regenerate-on-action contract preserved
- [ ] English dev guide updated (Themes section); translations untouched
- [ ] Fix vs feature: **feature** (new theme system) — confirm with operator
      whether pre-prod override applies (z bump) or this is a real feature (y bump)
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
- [ ] MSRV bump documented in workspace Cargo.toml
