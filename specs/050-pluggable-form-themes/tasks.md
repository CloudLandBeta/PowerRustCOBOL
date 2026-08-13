<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Pluggable, self-contained form themes

- **Status:** draft → in progress → **done** (T1–T15; see the deviations note)
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-08-12
- **Classification:** **FIX** in its entirety (operator, 2026-08-12) — fix
  commits on the fixes branch, **f=97 only**, no f=96 post.

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it.

> **Standing verification for every task below.** `cargo test -p cobolt-forms`
> does **not** compile without `--features render` (pre-existing, see D7) — always
> pass the flag. And run
> `cargo test -p cobolt-forms --features render elegance_baseline` after **each**
> task in T3–T9: the eight golden shape-leaf counts are the only tripwire proving
> Liquid Glass has not moved. If one moves, the task is wrong — never re-bless a
> number to get green. This standing check, unmoved through T3–T15, **is**
> → **AC13** (R21); T15 records the final run.

---

## Deviations from the plan (recorded during /implement)

1. **T2's deletion moved into T3/T4.** The plan had T2 move `ElegancePalette`,
   `elegance_palette`, `draw_elegance_surface` and `install_elegance_theme` out
   of `paint.rs`. They could not be *deleted* there — eight painter sites still
   called them, and those are T3/T4's job. T2 created the new home; the old code
   was removed as the last act of T4, which is the only order that compiles.
2. **Five publishers, not four.** The plan listed the canvas, preview, host and
   compiler. `cobolt-cli/src/form_gui.rs:349` (the Run Form glue) is a fifth.
3. **Two extra glass reads found and gated.** The plan named `paint.rs:1670`,
   `paint.rs:5974` and `sidebar.rs:360`. `control_surface_tone`
   (`paint.rs:7054`) and the chart card face (`paint.rs:5974`'s `default_face`)
   also read it; AC4's scanner is what surfaced them.
4. **`SurfaceSpec.fill` is `Option<Color32>`, not `Color32`.** `Shape` and
   `Accent` are caller-led roles, so the theme must be able to say "the caller's
   `base` leads" rather than inventing a colour.
5. **Version is 1.61.37, not .36.** `.36` had already been taken by the spec-049
   sidebar work earlier in the same session.
6. **A THIRD picker had the same conflation** (operator, screenshot). The
   *New Form* dialog's row was labelled "Theme" — a hardcoded literal, not even a
   `Tr` key — and offered the four GLASS STYLES. A real theme therefore could not
   be chosen when a form was created. Now: a **Theme** row listing the full
   catalogue (defaulting to *(from project)*), a separate **Glass style** row
   disabled under a self-contained theme with the same hint, `Form::theme`
   written on create, and the neumorphic seed gated on the theme the form will
   *actually* use rather than on the project default alone. Guarded by
   `every_picker_offers_the_theme_catalogue_not_the_glass_styles`.
7. **R3 needed a painting path, not just a manifest field.** Adding
   `ThemeManifest.self_contained` and `FormTheme.self_contained` left the flag
   *stored and never consulted* — the exact bug pattern this spec exists to
   close. `surface_theme::for_pack(self_contained)` carries a pack's own
   declaration to the gate, and all four publishers resolve through it when a
   pack was resolved. Covered by the extra assertions in
   `catalog_declares_look_ownership` and by the compiler's template test.

## Movement A — a theme is an implementation, not an identity

- [x] **T1 — The `SurfaceTheme` trait and its vocabulary** (R11, R12, R13)
  - Files: `crates/cobolt-forms/src/surface_theme.rs` (**new**),
    `crates/cobolt-forms/src/lib.rs`
  - Do: define `SurfaceTheme` (`id`, `is_self_contained`, `surface`, `token`,
    `radius`, `data_marks`, `install_widget_visuals` with a no-op default) plus
    `SurfaceSpec`, `SurfaceState`, `ColorToken`
    (`Text|DimText|InputBg|Card|CardRaised|Border|Focus|Accent(AccentName)`),
    `AccentName` (`Blue|Green|Red|Purple|Amber|Sky` — **ours**, not the crate's)
    and `RadiusKind` (`Control|Card`). **Every accessor returns `Option`**;
    `None` = "use the built-in Liquid Glass default". Carry the ⚠ global-style
    warning from `host.rs:1074-1093` onto `install_widget_visuals`.
  - Verify: `cargo build -p cobolt-forms --features render`. No painter touched
    yet; test counts unchanged.

- [x] **T2 — `LiquidGlassTheme` and `EleganceTheme`** (R11, R12, R22)
  - Files: `crates/cobolt-forms/src/surface_theme.rs`,
    `crates/cobolt-forms/src/paint.rs`
  - Do: `LiquidGlassTheme` — `is_self_contained() == false`, every accessor
    `None`. `EleganceTheme` — `is_self_contained() == true`, owns
    `elegance::Palette` **privately** and maps it out through `ColorToken` /
    `SurfaceSpec` / `RadiusKind` (this is what finally reads `control_radius` and
    `card_radius`, dead today and the cause of the `cargo check` warning). Move
    `ElegancePalette`, `elegance_palette`, `draw_elegance_surface` and
    `install_elegance_theme` out of `paint.rs` into it.
  - Verify: `cargo build -p cobolt-forms --features render`; `grep -rn
    "elegance::" crates/cobolt-forms/src/` names **only** `surface_theme.rs`.

## Movement B — carry it per frame, kill the enum

- [x] **T3 — Publish the theme per frame, retire `SurfaceStyle`** (R15, R16)
  - Files: `crates/cobolt-forms/src/paint.rs`,
    `crates/cobolt-ide/src/{app.rs,panels/designer.rs,theme_ui.rs}`,
    `crates/cobolt-form-host/src/host.rs`, `crates/cobolt-compiler/src/lib.rs`
  - Do: add `set_surface_theme` / `active_surface_theme` (a `Clone` newtype over
    `Arc<dyn SurfaceTheme>` in egui's per-frame store — copy the shape of
    `ActiveTheme` at `paint.rs:8285`), defaulting to `LiquidGlassTheme` when
    unset. Delete `SurfaceStyle`, `set_surface_style`, `active_surface_style`,
    `elegance_active`. Update the four publishers (`designer.rs:4824`,
    `app.rs:11754`, `host.rs:1072`, compiler `lib.rs:1666`
    `resolve_surface_style` → `resolve_surface_theme`) and `host.rs:1091`
    (`== SurfaceStyle::Elegance` → `theme.install_widget_visuals(ctx)`).
    Route the two seams `draw_surface_auto_bg` (8040) and `draw_surface_auto`
    (8075) through `theme.surface(role, state)`, falling back to
    `draw_glass_auto_bg` / `draw_glass_auto` on `None`.
  - Verify: `cargo check --workspace` (this is a workspace-wide API break — 5
    in-workspace consumers, no external ones); `cargo test -p cobolt-forms
    --features render` green **including** the eight golden counts.

- [x] **T4 — The eight defaulted-colour sites read tokens** (R13, R14)
  - Files: `crates/cobolt-forms/src/paint.rs` (2273 slider track, 2539 spec-039
    accents, 2856 progress trough, 2937 + 4198 text colour, 6061 chart series),
    `crates/cobolt-forms/src/render.rs` (3982 grid header, 5553 tree foreground,
    5659 menu band)
  - Do: replace each `elegance_active(ctx)` predicate with
    `active_surface_theme(ctx).token(…)` (or `.data_marks()` at 6061), keeping
    today's built-in value as the `unwrap_or_else`. **No painter may test which
    theme is active** — that is what makes AC7 true.
  - Verify: `cargo test -p cobolt-forms --features render`; `grep -rn
    "elegance_active\|SurfaceStyle::" crates/` returns nothing.

- [x] **T5 — AC7: registering a theme touches no painter** (R14)
  - Files: `crates/cobolt-forms/src/surface_theme.rs` (tests)
  - Do: register a throwaway `TestTheme` and render the `r4_fixture` with it.
  - Verify: `cargo test -p cobolt-forms --features render
    registering_a_theme_touches_no_painter`; reports **"painter sites changed:
    0/11"**. → **AC7**

## Movement C — the gate

- [x] **T6 — One gate, and the shadows come back** (R4, R5, R6)
  - Files: `crates/cobolt-forms/src/paint.rs` (1670, 5974),
    `crates/cobolt-forms/src/sidebar.rs` (360)
  - Do: add `glass_config_applies(ctx)` =
    `!active_surface_theme(ctx).is_self_contained()`; at each of the three sites
    make it `glass_config_applies(ctx) && active_glass_style(ctx).is_neumorphic()`.
    Nothing in the shadow code changes — `regular_drop_shadow`/`drop_shadow_spec`
    bail on `is_neumorphic`, so gating that one boolean is what restores R5.
    ⚠ `sidebar.rs:360` was added in 1.61.36 and inherits the same bug.
  - Verify: `cargo test -p cobolt-forms --features render` — new
    `a_self_contained_theme_ignores_every_glass_style` asserts the four
    `painted_leaf_count` results under `EleganceTheme` are **equal** across
    `ALL_GLASS_STYLES`, and that a `ShadowEnabled` control paints shapes outside
    its rect at every one; reports the four counts and the with/without-shadow
    pair. → **AC2, AC3**

- [x] **T7 — AC4: prove there is only one gate** (R6)
  - Files: `crates/cobolt-forms/src/paint.rs` (tests)
  - Do: a test scanning the source of `paint.rs`, `render.rs` and `sidebar.rs`
    asserting every `active_glass_style(` occurrence is either the gate itself or
    guarded by it. Crude, and the only thing that stops the next painter
    reintroducing this.
  - Verify: `cargo test -p cobolt-forms --features render
    glass_style_is_read_through_one_gate`; reports file:line + verdict per
    occurrence; adding an ungated read fails it. → **AC4**

- [x] **T8 — A self-contained theme writes nothing to the model** (R7, R8)
  - Files: `crates/cobolt-ide/src/app.rs` (9723),
    `crates/cobolt-ide/src/panels/designer.rs` (2122, 2991, 3497, 4432)
  - Do: make all five `apply_glass_style_defaults` calls conditional on the
    resolved theme not being self-contained. R8 then follows for free — nothing
    was overwritten, so switching back to Liquid Glass finds the form as it was.
  - Verify: `cargo test -p cobolt-ide
    a_self_contained_theme_writes_nothing_to_the_model` — select Elegance, cycle
    `GlassStyle` ×4, assert form background / gradient flags / per-control shadow
    props byte-identical, then back to Liquid Glass and assert the earlier
    appearance reproduces; reports the before/after property table. → **AC5**

- [x] **T9 — Explicit properties outrank the theme** (R9, R10)
  - Files: `crates/cobolt-forms/src/paint.rs`, `crates/cobolt-forms/src/surface_theme.rs`
  - Do: confirm/extend the existing precedence (`user_bg` leads; the `#FFFFFF`
    sentinel rule at `paint.rs:2937`) to `CornerRadius` via `RadiusKind`. Leave
    `transparency_of` alone — `Transparency` is the developer's under every theme.
  - Verify: `cargo test -p cobolt-forms --features render
    explicit_properties_outrank_the_theme`; reports the six
    (property, explicit, unset) triples. → **AC6**

- [x] **T10 — AC1 + AC8: catalogue declaration and the unset default** (R1–R3, R15)
  - Files: `crates/cobolt-forms/src/theme.rs`, `crates/cobolt-forms/src/theme_pack.rs`
  - Do: `FormTheme` gains `self_contained: bool` + `surface: Arc<dyn SurfaceTheme>`;
    `ThemeManifest.self_contained` with `#[serde(default)]` so every existing
    pack keeps its behaviour unedited (R3).
  - Verify: `cargo test -p cobolt-forms --features render
    catalog_declares_look_ownership an_unpublished_theme_is_liquid_glass`;
    reports id → kind → self_contained, and the no-publish vs
    Liquid-Glass-publish counts across all four styles. → **AC1, AC8**

## Movement D — picker honesty, docs, finalize

- [x] **T11 — The picker stops offering settings that do nothing** (R17, R18, R19)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (7881–7930),
    `crates/cobolt-ide/src/theme_ui.rs`
  - Do: pass the **project default** into `resolve_theme_id` (it passes `None`
    today — that is R19's defect), label an inherited value, and wrap the Glass
    style row in `ui.add_enabled_ui(!self_contained, …)` with its hint. No write
    path changes, so R18 holds by omission.
  - Verify: `cargo test -p cobolt-ide
    the_glass_row_is_disabled_under_a_self_contained_theme
    a_form_shows_the_theme_it_inherits`; reports the stored `GlassStyle` before/
    after and the resolved id per (form, project) pair. → **AC10, AC11**

- [x] **T12 — Cross-surface parity** (R16)
  - Files: `crates/cobolt-ide` / `crates/cobolt-form-host` (tests)
  - Do: render one form through the canvas, the preview path and the host path
    under Elegance.
  - Verify: `cargo test -p cobolt-form-host themed_surfaces_agree`; reports the
    three-way table. → **AC9**

- [x] **T13 — Docs & i18n** (R20, R22)
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: guide passage in the form-theme section — some themes own the whole look,
    Glass style does not apply to them, your explicit control properties still
    win. Add 2 `Tr` fields ×**6 languages** (Glass-style-disabled hint;
    "inherited from project"). Translations `-es/-pt/-jp/-cn/-fr` **untouched**.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations); new
    `no_user_facing_string_names_the_crate` reports the strings checked.
    → **AC12, AC14**

- [x] **T14 — System KB** (steering)
  - Files: `crates/cobolt-compiler/src/lib.rs` (doc constants),
    `assets/knowledge/chunked.data`
  - Do: `GlassStyle` becomes conditional on the theme — an observable behaviour
    change, so update the Rust doc constants **and** re-run
    `cargo run -p cobolt-ide --example build_chunked_kb`, committing the
    regenerated store. The KB source is Rust constants, **not** `docs/*.md`; an
    unchanged `chunked.data` with a green freshness test means the wrong file was
    edited.
  - Verify: `cargo test -p cobolt-ide` — the KB freshness test is green *and*
    `chunked.data` actually changed (`git diff --stat`).

- [x] **T15 — Finalize** (steering)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump `z`; CHANGELOG entry stating plainly that **Elegance forms change
    appearance** — drop shadows reappear, theme corner radii start applying.
  - Verify: `cargo test --workspace` (expect ~1,138+ passing, 0 failures — see
    D8 for the two environmental `external_crates_*` failures, which are not to
    be chased). Then the **manual** checks from plan §6, which are the ones that
    matter here: this area's tests were green throughout the original defect.
    1. `cargo build --release -p cobolt-ide` and confirm the running binary is
       the new one (`ps` + `strings <bin> | grep 1.61.`) — the operator's binary
       is stale far more often than expected.
    2. Theme = Elegance ⇒ Glass style row greyed with its hint; cycling it
       changes nothing on the canvas.
    3. **`ShadowEnabled` on a Panel is visible under Elegance at Neumorphic
       Light** — the reported defect, and the one thing no test will catch.
    4. Explicit `BackgroundColor` + `CornerRadius` still win.
    5. Back to Liquid Glass ⇒ the form looks as it did before Elegance existed.
    6. Project default = Elegance, form with no override ⇒ picker says Elegance
       (inherited) and renders as Elegance.
    7. Run Form and `rcrun build` the same form ⇒ all four surfaces agree.

## Done criteria

All 14 acceptance criteria in `spec.md` checked, `cargo test --workspace` green,
guide + System KB updated, and the change committed as **fixes** on the fixes
branch (do **not** commit or push unless the operator asks; pushing obeys the
São Paulo window — never 09:00–18:00 Mon–Fri).

---

# Outstanding debt — unrelated to spec 050

Carried here at the operator's request so nothing is lost. **None of this is a
prerequisite for T1–T15**, and none of it should be started without the operator
saying so. Ordered roughly by how much it costs to leave alone.

- [ ] **D1 — Commit the working tree, which mixes fixes and features**
      (golden rule #5)
  - 18 uncommitted entries: `host.rs`, `shell.rs`, `breadcrumb.rs` (new),
    `sidebar.rs`, `icons.rs`, `lib.rs`, `model.rs`, `paint.rs`, `render.rs`,
    `app.rs`, `designer.rs`, `properties.rs`, `toolbox.rs`, `project_model.rs`,
    `i18n.rs`, `version.rs`, `specs/049-application-shell/HANDOFF.md`,
    `specs/050-pluggable-form-themes/`.
  - **Fixes** (→ f=97): the white rail, the breadcrumb's pale border, the
    ContentPane ignoring the RAD background, the double offset, the unscrollable
    menu pane, the 19-key format painter, the non-recursive open-form target
    list, the stale-build prompt, the seam drag starting from 0, the unwanted
    separator rules — **plus this session's** sidebar drop shadow (item 1) and
    the design-time rail view (items 3/4/5).
  - **Features** (→ f=96, `[Noticia]`): `cobolt-forms::breadcrumb`, the footer
    Panel, the design-time seam resizers, `HeaderImage`/`HeaderIcon`, the sidebar
    gradient, the theme-picker/Glass-style split, the window sizing.
  - Verify: two clean commits, neither mixing kinds; `cargo test --workspace`
    green at each.

- [ ] **D2 — Handoff item 2: footer controls render in the ContentPane**
      (spec 049, **not started**)
  - The Pane filter at `host.rs:335` drops only the SideMenu, so the footer Panel
    and its subtree survive, get shifted left by `side_dx` and draw in the wrong
    pane. Needs **both** halves: exclude the footer subtree from the host, and
    have the shell render it into `sidebar::footer_rect()`. The shell has never
    rendered form controls — new capability, not a tweak.
  - Sketch (from this session's reading): host keeps the excluded subtree aside;
    the shell publishes the footer rect before `pane_frame`; the host draws it in
    an `egui::Area` clipped to that rect and merges the `RenderOutput` so events
    and `prop_updates` flow through the existing handling.

- [x] **D3 — Test coverage for this session's design-time rail view** — **DONE.**
      `designer.rs::the_canvas_shows_the_rail_in_a_state_of_its_own` covers all
      four assertions below and passes: *"designed Collapsed=false, toggle ⇒
      shown collapsed at 72pt (designed 200pt kept, property untouched); footer
      Panel narrows with the rail; an inspector edit resets the override."*
      (handoff items 3/4/5)
  - `rail_view_collapsed` / `rail_designed_collapsed` / `crumb_toggle_rect` on
    `DesignerPanel`, `sidebar::shown_width`, `breadcrumb::DesignView`. Assert:
    the canvas draws the rail at `COLLAPSED_WIDTH` when shown collapsed; the
    breadcrumb toggle flips the view; **`Collapsed` on the control is never
    written**; editing `Collapsed` in the inspector takes the view back.
  - Verify: `cargo test -p cobolt-ide` — was green without this, so the gap is
    silent.

- [ ] **D4 — Docs and System KB have nothing for any of spec 049**
  - The routing, the three panes, the breadcrumb, the footer Panel, `IconSize`,
    `HeaderImage`, `HeaderIcon`, the seam resizers, the gradient, the theme
    picker split. KB = Rust constants in `cobolt-compiler/src/lib.rs` plus
    `cargo run -p cobolt-ide --example build_chunked_kb`, **not** markdown.

- [ ] **D5 — Announce spec 049 once it is on `origin/main`**
  - Fixes → f=97, features → f=96 with `[Noticia]`. Spanish, vBulletin BBCode,
    signed "Anthropic Claude Codex Agent", title ≤ 50 chars, native browser
    submit (windows-1252 — a UTF-8 `fetch` mojibakes accents), exact text
    confirmed with the operator first. Posts are editable for ~5 minutes only.

- [ ] **D6 — A second form cannot load into the ContentPane**
  - Needs spec 037 T16's multi-form runtime; `open-form:` items report themselves
    at run time and nothing more. Long-standing, larger than spec 049.

- [ ] **D7 — `cargo test -p cobolt-forms` does not compile without
      `--features render`** (pre-existing)

- [ ] **D8 — The two `external_crates_*` failures are environmental**
  - Not re-run recently; **do not chase them**.

- [ ] **D9 — Spec 049 Q5/Q6/Q8/Q9 are the operator's call** — still unanswered.

- [ ] **D10 — Open operator/external items** (from the standing memory)
  - Ollama Cloud endpoint; the French guide (`developers-guide-fr.md` does not
    exist); localization deltas 1–8; the spec-027 corner mismatch.

- [ ] **D11 — Translated guides are not what they look like**
  - `developers-guide-{pt,jp,cn}.md` are **byte-identical English copies** (same
    md5), `-es` is a partial machine translation, `-fr` is absent — all frozen at
    2026-07-02, ~1,800 lines behind the English guide. These are user-maintained;
    flag, do not edit.

- [ ] **D13 — Announce spec 050 once it is on `origin/main`** (this spec's own
      Rule #4 obligation)
  - A **fix** → **f=97 only**, no f=96 post. Spanish, vBulletin BBCode, signed
    "Anthropic Claude Codex Agent", title ≤ 50 chars, native browser submit
    (windows-1252), exact text confirmed with the operator first.
  - Lead with the observable behaviour: `ShadowEnabled` stopped working under a
    self-contained theme when the glass style was Neumorphic, and picking a glass
    style rewrote the form. Include the usage example (Theme = Elegance +
    `ShadowEnabled`) and link the guide.
  - ⚠️ Nothing is merged yet — **do not post** until it is on `origin/main`.

- [ ] **D12 — Unsettled convention conflicts** (operator to rule)
  - `CONVENTIONS.md` says the commit trailer is `Claude Opus 4.8`; the harness
    specifies `Claude Opus 5`, and commits currently use Opus 5.
  - `CONVENTIONS.md` says forum bodies stay plain ASCII; posts 10532/10533
    (2026-08-05) used accented Spanish through the native browser submit and
    verified clean. Either the rule is stricter than needed, or that was luck.
