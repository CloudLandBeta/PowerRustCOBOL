<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Pluggable, self-contained form themes

- **Status:** draft → **ready for review**
- **Spec:** ./spec.md   **Date:** 2026-08-12
- **Classification:** **FIX** (operator, 2026-08-12) — see §7.

## 0. What the operator's reclassification changes

The spec proposed splitting this into a fix (the leak) and a feature (the
registry). The operator has ruled the **whole thing a fix**: specs 007 and 047
already specified this behaviour and the implementation did not deliver it, so it
is technical debt, exactly like a missing COBOL-85 construct.

The code agrees. `draw_elegance_surface` carries the comment *"no frost, no
relief, and **no dependence on `GlassStyle`** (spec 047 R12)"* — the intent was
written down and then contradicted two call frames up, where `is_neumorphic` is
read unconditionally.

Two consequences for this plan:

1. **Spec §6's fix/feature split is superseded.** One classification, one forum
   post on **f=97**, no f=96 post. `spec.md` §6 and Q4 are amended to match.
2. **Q4 (commit ordering) dissolves, and the better order is now available.**
   Rule #5 was the only reason to consider writing the gate twice. With the wall
   gone, the registry lands **first** and the gate becomes a handful of one-line
   changes on top of it, written once.

## 1. Approach

Four movements, in order. Each is independently buildable and testable, and each
leaves Liquid Glass byte-identical (R21).

### Movement A — a theme is an implementation, not an identity (R11–R14)

New module `crates/cobolt-forms/src/surface_theme.rs`:

```
trait SurfaceTheme: Send + Sync + Debug
    fn id(&self) -> &str
    fn is_self_contained(&self) -> bool
    fn surface(&self, role: SurfaceRole, st: SurfaceState) -> Option<SurfaceSpec>
    fn token(&self, tok: ColorToken) -> Option<Color32>
    fn radius(&self, kind: RadiusKind) -> Option<f32>
    fn data_marks(&self) -> Option<Vec<Color32>>
    fn install_widget_visuals(&self, ctx: &egui::Context)   // default: no-op
```

Every accessor returns `Option`. **`None` means "I have nothing to say — use the
built-in Liquid Glass default"**, which is what makes `LiquidGlassTheme` a
one-line implementation returning `None` throughout and guarantees R21 by
construction: an unthemed path cannot change because the theme cannot answer.

Two implementations ship:

- `LiquidGlassTheme` — `is_self_contained() == false`, everything `None`.
- `EleganceTheme` — `is_self_contained() == true`; owns the third-party
  `elegance::Palette` **privately** and maps it out through `ColorToken` /
  `SurfaceSpec`. This is the abstraction boundary that keeps the crate's types
  from spreading into the trait, and it is also what satisfies R22 structurally
  rather than by review.

`ColorToken` is derived from what the 11 predicate sites actually ask for (§4,
decision 2): `Text`, `DimText`, `InputBg`, `Card`, `CardRaised`, `Border`,
`Focus`, `Accent(AccentName)`. `AccentName` is **ours**
(`Blue|Green|Red|Purple|Amber|Sky`); `EleganceTheme` maps it to `elegance::Accent`.
`RadiusKind` is `Control | Card`, which finally reads
`ElegancePalette::control_radius` / `card_radius` — dead fields today, and the
reason `cargo check` warns about them.

### Movement B — carry it per frame, kill the enum (R15, R16)

`SurfaceStyle`, `set_surface_style`, `active_surface_style` and `elegance_active`
are replaced by the pattern `set_active_theme` already uses for packs — a `Clone`
newtype in egui's per-frame store holding `Arc<dyn SurfaceTheme>`:

```
pub fn set_surface_theme(ctx, theme: Arc<dyn SurfaceTheme>)
pub(crate) fn active_surface_theme(ctx) -> Arc<dyn SurfaceTheme>   // Liquid Glass when unset
```

A context that never publishes gets `LiquidGlassTheme` (R15), so *"forgets to
publish"* and *"published Liquid Glass"* are the same path, not merely equal.

The two `match active_surface_style` seams (`draw_surface_auto_bg`,
`draw_surface_auto`) become: ask the theme for a `SurfaceSpec`; on `None`, call
`draw_glass_auto_bg` / `draw_glass_auto` exactly as today. The 11 predicate sites
become `active_surface_theme(ctx).token(...)`-style lookups with the existing
built-in value as the `unwrap_or_else`. **No painter tests which theme is
active** (R13), which is what makes R14/AC7 true.

Four publishers, one line each — designer canvas, preview, form host, compiled
binary (§2). `FormHost` is shared by Run Form *and* the compiled binary, so the
fourth surface comes free; this is the answer to Q6.

### Movement C — the gate (R4–R8)

With the theme in hand, the leak closes in four places:

- `paint.rs:1670` and `paint.rs:5974` —
  `let is_neumorphic = glass_config_applies(ctx) && active_glass_style(ctx).is_neumorphic();`
- `sidebar.rs:360` — the same, in the rail's `DropShadowSpec` resolution (added
  1.61.36; it inherits the bug).
- `glass_config_applies(ctx)` = `!active_surface_theme(ctx).is_self_contained()`
  — **the single gate** R6 demands.

Because `regular_drop_shadow`/`drop_shadow_spec` bail on `is_neumorphic`, gating
that one boolean is what restores R5's drop shadows; no change to the shadow code
itself.

R7 gates the **model writes**, which are in the IDE, not the model:
`apply_glass_style_defaults` is called from five sites (`app.rs:9723`,
`designer.rs:2122/2991/3497/4432`). Each becomes conditional on the resolved
theme not being self-contained. R8 then follows for free — nothing was
overwritten, so switching back to Liquid Glass finds the form as it was.

R9/R10 are already the shape of the code (`user_bg` leads; the `#FFFFFF`
sentinel rule at `paint.rs:2937`); Movement B extends the same rule to
`CornerRadius` via `RadiusKind` and to `Transparency` by leaving
`transparency_of` alone — it is the developer's, under every theme.

### Movement D — picker honesty + docs (R17–R20)

`properties.rs:7881-7930`: pass the **project default** into
`resolve_theme_id` (it currently passes `None`, R19), label an inherited value,
and wrap the Glass style row in `ui.add_enabled_ui(!self_contained, …)` with a
hint (R17). No write path changes, so R18 holds by omission.

Then the English guide, the System KB constants, and the chunked-store rebuild.

## 2. Affected crates / files

**`cobolt-forms` — the substance**

| File | Change |
|---|---|
| `src/surface_theme.rs` | **NEW.** `SurfaceTheme` trait, `SurfaceSpec`, `ColorToken`, `AccentName`, `RadiusKind`, `SurfaceState`; `LiquidGlassTheme`; `EleganceTheme` (absorbs `ElegancePalette`, `elegance_palette`, `draw_elegance_surface`, `install_elegance_theme`). |
| `src/paint.rs` | Delete `SurfaceStyle`/`set_surface_style`/`active_surface_style`/`elegance_active` (~7867–7933); add `set_surface_theme`/`active_surface_theme`/`glass_config_applies`; route the two seams (8040, 8075); the gate at 1670 + 5974; token lookups at 2273, 2539, 2856, 2937, 3194, 4198, 6061; `painted_leaf_count` test helper takes a theme. |
| `src/render.rs` | Token lookups at 3982, 5553, 5659. |
| `src/sidebar.rs` | The gate at 360. |
| `src/theme.rs` | `FormTheme` gains `self_contained: bool` + `surface: Arc<dyn SurfaceTheme>`; `liquid_glass()`/`elegance()` populate them; `ThemeCatalog::resolve` unchanged. |
| `src/theme_pack.rs` | `ThemeManifest.self_contained: bool` with `#[serde(default)]` (R3). |
| `src/lib.rs` | `pub mod surface_theme;` |

**`cobolt-ide`**

| File | Change |
|---|---|
| `src/app.rs` | 1315 → resolve a theme not a style; 11754 → `set_surface_theme`; 9723 → gate `apply_glass_style_defaults`. |
| `src/panels/designer.rs` | 4824 → `set_surface_theme`; 2122/2991/3497/4432 → gate the defaults. |
| `src/panels/properties.rs` | 7881–7930 — project default (R19), disabled Glass style row + hint (R17). |
| `src/i18n.rs` | 2 new `Tr` fields ×6 languages: theme-owns-the-look hint; "inherited from project". |
| `src/theme_ui.rs` | Publish `self_contained` alongside `(id, display_name)` so the picker can read it without a catalogue lookup. |
| `src/version.rs` | Bump `z`. |

**`cobolt-form-host`** — `src/host.rs` 1072 → `set_surface_theme`; 1091 → replace
`== SurfaceStyle::Elegance` with `theme.install_widget_visuals(ctx)` (⚠ stays
host-only, see §5).

**`cobolt-compiler`** — `src/lib.rs:1666` `resolve_surface_style` →
`resolve_surface_theme`; plus the System KB documentation constants.

**Docs / assets** — `docs/developers-guide-en.md` (form-theme section: some
themes own the whole look; Glass style does not apply to them; your explicit
properties still win). `assets/knowledge/chunked.data` regenerated via
`cargo run -p cobolt-ide --example build_chunked_kb`. `CHANGELOG.md`.

## 3. Data / model changes

- **`.cfrm`: none.** `Form::theme` and `Form::glass_style` keep their meaning and
  serialisation. A form saved before this change loads identically; a form saved
  after it is readable by an older build. `GlassStyle` is *ignored* under a
  self-contained theme, never rewritten (R18) — so the round-trip is lossless in
  both directions.
- **`cobolt.toml`: none.** `[forms] theme` already exists and already resolves.
- **`theme.toml` (asset packs):** one new optional key, `self_contained`, absent
  ⇒ `false` (R3). Every existing pack keeps its behaviour with no edit.
- **In-memory only:** `FormTheme` gains two fields; `FormHost`/`DesignerPanel`
  swap a `SurfaceStyle` field for an `Arc<dyn SurfaceTheme>`.
- **Migration: none required.** The behaviour change is that Elegance forms
  render *as specified* — shadows reappear, theme radii apply. That is the fix,
  and it is called out in §5 as an intended visual change.

## 4. Key decisions & alternatives

1. **Every trait accessor returns `Option`; `None` = "use the built-in".**
   Why: makes R21 structural instead of a promise — `LiquidGlassTheme` answers
   `None` to everything, so the glass paths are literally the same code with the
   same constants. Also lets a partial theme cover what it wants and inherit the
   rest, which is spec 007 R11's fallback rule expressed in the type.
   *Rejected:* a total trait where each theme must supply every value — forces
   `LiquidGlassTheme` to restate ~30 built-in constants, and any transcription
   slip is a silent Liquid Glass regression.

2. **Two orthogonal accessors — `surface(role)` and `token(tok)` — and
   `SurfaceRole` stays at five variants. (Resolves Q5.)**
   Why: reading all 11 predicate sites, only **3** ask for a structural face
   (`paint.rs:3194`, `render.rs:5659`, plus the two `draw_surface_auto` seams).
   The other **8** ask for a *named colour to default an unset property to* —
   slider track (2273), spec-039 widget accents (2539), progress trough (2856),
   text colour (2937, 4198), chart series (6061), grid header (`render.rs:3982`),
   tree foreground (`render.rs:5553`). Those are not structural registers, and
   inventing `SurfaceRole::SliderTrack`, `::ProgressTrough`, `::GridHeader` … to
   carry them would inflate the enum to a dozen variants that mean "a colour I
   needed once".
   *Rejected:* growing `SurfaceRole`. It conflates "which visual register is this
   surface in" (5 answers, genuinely closed) with "what colour defaults this
   property" (open-ended by nature).

3. **The registry lands before the gate.**
   Why: the operator's reclassification removes the rule-#5 wall that made this
   awkward, and the gate is then ~4 one-line edits against one helper instead of
   a temporary construction against the enum that gets rewritten immediately.
   *Rejected:* gate-first. It only paid off when the two halves had to ship in
   separate commits for separate forums; they no longer do.

4. **`EleganceTheme` owns the third-party palette privately.**
   Why: R22 becomes structural — no other module can name the crate's types, so
   no other module can leak them into a string. Also keeps the trait
   dependency-free for future themes.
   *Rejected:* exposing `elegance::Palette` on the trait.

5. **Reuse `painted_leaf_count` + `r4_fixture` + `ALL_GLASS_STYLES` (spec 047's
   own harness) as the verification vehicle.**
   Why: it already paints the full fixture across all four glass styles and
   asserts eight golden shape-leaf counts, and it is explicitly *"sensitive to
   geometry changes but blind to a pure colour swap — precisely the right
   sensitivity for proving a refactor moved nothing."* AC2's "identical across
   all four glass styles" is that helper called four times with the Elegance
   theme and the four results compared.
   *Rejected:* a new pixel-diff harness.

6. **`glass_config_applies` is one function, and the AC4 test enumerates its
   callers.**
   Why: R6 asks that `GlassStyle` not be readable for painting through more than
   one gate. A grep-based test over the source of `paint.rs`/`render.rs`/
   `sidebar.rs` asserting that every `active_glass_style(` occurrence is either
   the gate itself or guarded by it is crude but it is the only thing that
   actually stops the next painter reintroducing this.

## 5. Risks & mitigations

- **Elegance forms will legitimately change appearance** — drop shadows reappear
  (R5), theme corner radii start applying (R10). AC13 protects Liquid Glass, not
  Elegance, so nothing will catch this for us. → State it in the CHANGELOG and
  the f=97 post as the *substance* of the fix, and have the operator eyeball one
  Elegance form before the announcement. This is the one item that needs human
  visual confirmation.
- **The eight golden shape-leaf counts (1430 / 1252 / …) are the tripwire for
  R21** and the seams are being rewired underneath them. → Run
  `elegance_baseline_reports_untouched_paths` after **each** movement, not once
  at the end. Never re-bless a moved number to get green; a move means the
  refactor changed Liquid Glass and the movement is wrong.
- **`install_widget_visuals` mutates egui's global style.** Today's
  `install_elegance_theme` is host-only for that reason, documented at
  `host.rs:1074-1093`: the IDE shares one Context across every form window, so
  calling it there would restyle the IDE's own panels. Promoting it to a trait
  method makes it *look* callable from anywhere. → Keep the name explicit
  (`install_widget_visuals`, not `install_visuals`), carry the ⚠ warning onto the
  trait method, leave the single call site where it is, and add a test asserting
  the IDE crate contains no call to it.
- **`SurfaceStyle` and `set_surface_style` are `pub`**, used by three other
  crates — this is a workspace-wide API break. → All consumers are in-workspace
  (5 sites, §2); `cargo check --workspace` is the gate. No external consumers
  exist.
- **`Arc<dyn SurfaceTheme>` in egui's per-frame store** needs a `Clone` newtype
  and `Send + Sync + 'static`. → `ActiveTheme(Option<Arc<ThemePack>>)` at
  `paint.rs:8285` is the working precedent; copy its shape exactly.
- **`elegance_role_for` currently maps every unlisted control to
  `SurfaceRole::Card`** — a catch-all that will now be consulted through the
  trait for themes that are not Elegance. → Leave the mapping exactly as spec 047
  set it; it is the theme's business how to paint a role, not the caller's how to
  classify. Out of scope to revisit.
- **System KB freshness test goes red if `chunked.data` is not rebuilt** in the
  same change. → Movement D includes the rebuild; the red test is a real failure
  again since 2026-07-31.

## 6. Test strategy

All in `cobolt-forms` unless noted. Every test prints a quantified summary
(steering: quantified, human-readable results; verify-first).

| Test | Asserts | Reports |
|---|---|---|
| `catalog_declares_look_ownership` (AC1) | `liquid-glass` not self-contained, `elegance` is; a manifest with/without the key | a table: id → kind → self_contained |
| `a_self_contained_theme_ignores_every_glass_style` (AC2, AC3) | `painted_leaf_count` with `EleganceTheme` is **equal** across all four `ALL_GLASS_STYLES`; a `ShadowEnabled` control paints shapes outside its rect at every one | the four counts side by side, and the count with/without the shadow |
| `glass_style_is_read_through_one_gate` (AC4) | every `active_glass_style(` in `paint.rs`/`render.rs`/`sidebar.rs` is the gate or guarded by it | file:line of each occurrence and its verdict |
| `a_self_contained_theme_writes_nothing_to_the_model` (AC5) | select Elegance, change `GlassStyle` ×4 → form background, gradient flags and per-control shadow props byte-identical; back to Liquid Glass → earlier appearance reproduced | before/after property table |
| `explicit_properties_outrank_the_theme` (AC6) | explicit `BackgroundColor`/`CornerRadius`/`Transparency` honoured under Elegance; unset ⇒ theme's values | the six (property, explicit, unset) triples |
| `registering_a_theme_touches_no_painter` (AC7) | a throwaway `TestTheme` renders a fixture; painter call-site count changed = **0** | "painter sites changed: 0/11" |
| `an_unpublished_theme_is_liquid_glass` (AC8) | no-publish count == Liquid-Glass-publish count, all four styles | the two×four counts |
| `themed_surfaces_agree` (AC9, `cobolt-ide` + `cobolt-form-host`) | canvas / preview / host resolve the same theme and the same tokens for one form | three-way table |
| `the_glass_row_is_disabled_under_a_self_contained_theme` (AC10, `cobolt-ide`) | row disabled + hint present; stored `GlassStyle` byte-identical across theme toggles | the stored string before/after |
| `a_form_shows_the_theme_it_inherits` (AC11, `cobolt-ide`) | project=Elegance, form override absent ⇒ picker shows Elegance as inherited, render is Elegance | resolved id per (form, project) pair |
| i18n completeness (AC12) | existing `i18n_tests` covers the 2 new fields ×6 | — |
| `elegance_baseline_reports_untouched_paths` (AC13) | **unchanged**, golden counts unmoved | the existing 8-row table |
| `no_user_facing_string_names_the_crate` (AC14) | catalogue display names + new `Tr` values free of the crate name | the strings checked |
| `the_ide_never_installs_widget_visuals` (§5 risk) | no call to `install_widget_visuals` in `cobolt-ide` | — |

**Manual / visual verification** (the one thing tests cannot do — this area's
tests were green throughout the defect):

1. `cargo build --release -p cobolt-ide` (the operator's binary is stale far more
   often than expected — check `strings` for the version first).
2. Open a form, set Theme = Elegance. Confirm the **Glass style row is greyed**
   with its hint, and that cycling it changes nothing on the canvas.
3. Put `ShadowEnabled` on a Panel. Confirm the shadow is **visible under Elegance
   at Neumorphic Light** — this is the reported defect.
4. Set an explicit `BackgroundColor` and `CornerRadius`; confirm both win.
5. Switch back to Liquid Glass; confirm the form looks as it did before Elegance
   was ever selected.
6. Set the *project* default to Elegance, open a form with no override; confirm
   the picker says Elegance (inherited) and the form renders as Elegance.
7. Run Form and `rcrun build` the same form; confirm all four surfaces agree.

## 7. Steering compliance

- [x] **i18n:** 2 new `Tr` fields (Glass-style-disabled hint; "inherited from
      project"), all six languages, no literals.
- [x] **Generated-code banner + regenerate-on-action:** untouched — no codegen
      change, no COBOL change, no identifier change.
- [x] **English dev guide updated;** `-es/-pt/-jp/-cn` **not** edited
      (user-maintained).
- [x] **System KB:** `cobolt-compiler` doc constants updated **and**
      `assets/knowledge/chunked.data` regenerated in the same change.
- [x] **Fix vs feature: FIX** (operator, 2026-08-12 — specs 007/047 specified
      this; the implementation did not deliver it, so it is technical debt).
      → bump `z` in `version.rs`, `CHANGELOG.md` entry, commits on the **fixes**
      branch, announce on **f=97 only** (Spanish, vBulletin BBCode, signed
      "Anthropic Claude Codex Agent", title ≤ 50 chars, native browser submit for
      windows-1252, exact text confirmed with the operator first). **No f=96
      post.**
- [x] **No "cobolt" in user-facing text;** COBOL identifiers/source stay English;
      the third-party crate name never surfaces (R22, enforced by AC14 and by
      `EleganceTheme` owning the palette privately).
- [x] **Rust only** — no Python, shell, `sed`, `perl` or Node used to edit or
      generate repository files.

### Amendments to `spec.md`

`spec.md` §6's fix/feature split and Q4 are **superseded** by the operator's
ruling: the whole spec is a fix, announced on f=97 only. Q5 and Q6 are resolved
in §4 decision 2 and §1 Movement B respectively. *(`spec.md` is edited to record
this — see its §6 and Q4/Q5/Q6.)*

---

**Next step:** review this plan. When satisfied, run **`/tasks`**.
