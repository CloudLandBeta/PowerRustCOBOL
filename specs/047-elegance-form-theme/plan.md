# Plan — Elegance form theme

- **Status:** draft → **ready for review** (all decisions settled)
- **Spec:** ./spec.md   **Date:** 2026-08-08

> **Spec R5 was amended during this phase** (operator-approved): Elegance faces
> are hand-painted from the crate's public palette rather than substituting real
> `egui-elegance` widgets, because those widgets cannot be constrained to a
> caller-supplied rect. See §4 Decision 1 and spec Q5.
>
> **Version: 1.61.0** — the operator explicitly authorised the minor bump for
> this feature (§7).

## 1. Approach

### 1.1 What the code actually looks like (grounding)

Three facts from the survey drive everything below.

**Fact A — `paint::draw_control` is one painter shared by both surfaces.**
The designer canvas calls `render_faces` (static, `RenderMode::Static`); the
live form calls `render_form` → `render_interactive`. Both bottom out in
`paint::draw_control`. Roughly **15 control kinds** reach their face through
that single function on *both* surfaces: Label, Panel, GroupBox, Line, Shape
(via the one `_` arm at `render.rs:6049`), plus Button, CheckBox, RadioButton,
TextBox's frame, Slider, TabControl, ProgressBar, PictureBox, DateTimePicker's
closed face, and all six charts (their own arms, each calling `draw_control`).
Theming `draw_control` therefore satisfies most of R4 on both surfaces from one
place — this is the main lever.

**Fact B — eight controls bypass `draw_control` on the live surface only.**
`DataGrid` (3878), `TreeView` (5517), `MenuBar` (5599), `ToolBar`/`StatusBar`
(5855), `Splitter` (5578), `ListBox` (3709), `ComboBox` (3673, via the
`glass_combo_header`/`glass_combo_popup` helpers) and `NumericUpDown` (3648)
hand-paint with raw `painter` calls and hard-coded colours. Each of these has
**two** implementations — a static face in `draw_control` and a live one in
`render.rs` — so each must be themed twice and the two kept matching. This is
where the real work is, and where designer↔runtime parity can silently break.

**Fact C — the theme channel is context-published, per frame, by the host.**
`paint::set_active_theme(ctx, Option<Arc<ThemePack>>)` and
`paint::set_glass_style(ctx, GlassStyle)` are called once per frame by exactly
three production hosts before any drawing: `cobolt-form-host/src/host.rs:872`
(shared runtime — `rcrun run-form` *and* every compiled binary), the IDE
preview (`app.rs:11700`), and the IDE designer canvas (`designer.rs:4470`).
`render_form`/`render_interactive` never set them. Elegance rides this same
established contract.

### 1.2 The design

**Catalog (R1, R2).** Add `ELEGANCE: &str = "elegance"` and
`FormTheme::elegance()` (`ThemeKind::Procedural`, display name `"Elegance"`) to
`cobolt-forms/src/theme.rs`; `ThemeCatalog::builtin()` returns both procedural
entries. That module is **not** behind the `render` feature and holds only
strings, so no `egui`/`elegance` types leak into it. `resolve_theme_id`'s
signature and precedence are untouched — Elegance is just another id that can
appear in `Form.theme` / the project default, so R2 is satisfied with **zero**
changes to selection, persistence, or `.cfrm`/`cobolt.toml` schema.

**A third state on the wire (the core new mechanism).** `set_active_theme`
carries `Option<Arc<ThemePack>>`, where `None` means Liquid Glass. Elegance is
procedural, so it has no pack — `None` is already taken. Rather than widen that
function (breaking 3 hosts + tests), add a **parallel publisher that mirrors
`set_glass_style` exactly**:

```rust
// cobolt-forms/src/paint.rs
pub enum FormStyle { LiquidGlass, Elegance }   // default LiquidGlass
pub fn set_form_style(ctx: &egui::Context, style: FormStyle);
fn active_form_style(ctx: &egui::Context) -> FormStyle;   // defaults LiquidGlass
```

The three hosts in Fact C already have a "set both per frame" block; each gains
one line. Anything that forgets to call it keeps today's behaviour exactly
(R10).

**Painting (R4, R5 as amended).** `draw_control` gains one early branch: when
`active_form_style` is `Elegance`, paint the control's face from
`elegance::Palette::slate()` — whose fields and constructor are fully public and
need **no `Ui`/`Context`** (verified) — instead of `draw_glass_auto*`. Because
`draw_control` is shared (Fact A), this lands ~15 control kinds on both
surfaces at once, at the control's exact designer geometry.

**The shared seam (R13).** Seven sub-element sites currently call
`draw_glass_auto` unconditionally *after* the frame dispatch:

| Line | Sub-element |
|------|-------------|
| 1446 | non-visual control card (`NV_CARD`) |
| 1961 | Shape (rect / round-rect) |
| 2800 | ProgressBar fill bar |
| 3534 | CheckBox tick box |
| 4522 | PictureBox frame |
| 4948 | ComboBox header (`glass_combo_header`) |
| 5012 | ComboBox popup (`glass_combo_popup`) |

All seven route through **one** new dispatcher, `draw_surface_auto(...)`, which
switches on `active_form_style`: `Elegance` → Elegance paint; everything else →
the existing `draw_glass_auto` call, byte-for-byte. Per the R13 decision, only
the Elegance arm is implemented; the asset-pack arm stays a pass-through, so
spec 007's Phase 6 (T15–T17) has a defined home without this spec touching
asset-pack behaviour.

**The eight live-only painters (Fact B).** Each gets an Elegance palette path in
`render.rs` alongside its existing glass path, and its `draw_control` static
twin gets the matching treatment, verified as a pair (see §6).

**The four spec-039 widgets (R6).** `Knob`, `Gauge`, `Switch`, `FileDropZone`
stay **real `elegance` widgets** — no change to how they render. They currently
rely on `Theme::current(ctx)` silently falling back to `Theme::slate()` because
nothing ever calls `install`. The hosts will now call
`elegance::Theme::slate().install(ctx)` when Elegance is active, which (a) makes
the palette explicit rather than accidental, and (b) registers the crate's
bundled symbols font so its glyphs stop rendering as tofu. `install` is
documented as cheap to call every frame (it early-returns when unchanged), so it
sits in the same per-frame host block as `set_form_style`.

**Fallback (R7).** Any control kind without an Elegance path falls through to
the existing glass paint unchanged — no new failure mode. R11 requires no R4
family be left there at delivery; §6 has the test that proves it.

## 2. Affected crates / files

**`crates/cobolt-forms/src/theme.rs`** — add `ELEGANCE` const,
`FormTheme::elegance()`; `ThemeCatalog::builtin()` returns both procedural
entries. Not feature-gated; strings only.

**`crates/cobolt-forms/src/paint.rs`** — the bulk of the work:
- `FormStyle` enum + `set_form_style` / `active_form_style` (mirrors
  `set_glass_style` at 7569–7583).
- `elegance_palette()` helper wrapping `elegance::Palette::slate()` +
  `Theme::slate()`'s `control_radius`/`card_radius`/padding constants.
- `draw_surface_auto(...)` — the R13 seam; the 7 sites above rewired to it.
- The Elegance branch in `draw_control`'s frame chain (inserted before the
  `else if glass` at 3109, after the asset-pack branch at 3043).
- Elegance faces for the static twins of the Fact-B controls.

**`crates/cobolt-forms/src/render.rs`** — Elegance paths for the eight
live-only hand-rolled painters (DataGrid, TreeView, MenuBar, ToolBar/StatusBar,
Splitter, ListBox, ComboBox, NumericUpDown).

**`crates/cobolt-form-host/src/host.rs`** (~872) — publish `set_form_style` +
`Theme::install` in the existing per-frame block. Covers `rcrun run-form` **and
every compiled binary**, so R5-parity comes free.

**`crates/cobolt-ide/src/app.rs`** — `publish_theme_choices` (1281) currently
hard-codes Liquid Glass then packs; change it to enumerate
`ThemeCatalog::builtin()` so Elegance appears in **both** pickers with no picker
code change (AC1). `resolve_theme_pack` (1295) gains a sibling resolving the id
to a `FormStyle`. Preview publish site at 11700.

**`crates/cobolt-ide/src/panels/designer.rs`** (~4470) — designer-canvas publish.

**`crates/cobolt-cli/src/form_gui.rs`** (~339) and
**`crates/cobolt-compiler/src/lib.rs`** (1219 `wanted_theme_ids`, 1538) — these
resolve the theme id and map it to a pack; they must not treat `elegance` as a
missing pack and warn. `wanted_theme_ids` already drops the procedural default —
it must drop `elegance` too, or the build will hunt for a nonexistent
`assets/themes/elegance/` and print a spurious "falling back to Liquid Glass".

**`docs/developers-guide-en.md`** — extend the existing "Form themes and styles"
section (line 1464); English only, translations untouched.

**`crates/cobolt-ide/src/i18n.rs`** — **expected: no change.** "Elegance" is a
product term like "Liquid Glass", carried in `FormTheme::display_name`, not a
`Tr` key. `/tasks` must confirm no *new* picker string appears; if one does, it
is 6 languages.

## 3. Data / model changes

**None to any persisted format.** No `.cfrm` schema change, no `cobolt.toml`
change, no new control property, no new event. `Form.theme` and the project
default already accept an arbitrary id (spec 007); `elegance` is simply a value
they can now hold, and an unknown id already falls back safely, so a project
carrying `elegance` opened by an older build degrades to Liquid Glass rather
than failing.

New in-memory types only: `FormStyle` (paint.rs) and the catalog entry.

**Generated COBOL: unaffected.** Theming is a rendering concern; the codegen
banner and regenerate-on-action contract are untouched.

**System KB: expected unaffected.** The KB tables document control properties,
methods and events — none change here. `/tasks` must still run the freshness
check and, if it goes red, treat that as a real failure (not an expected one).

## 4. Key decisions & alternatives

**Decision 1 — Hand-paint from the public palette; do *not* swap in real
`egui-elegance` widgets. (Amended spec R5 — operator-approved; spec Q5.)**

*Why.* The crate's widgets cannot honour caller geometry. Verified: `Button` has
**no height override** at all; `Checkbox` is a fixed 14pt box; `TabBar` and
`Switch` are fully intrinsic; `Select`/`TextInput`/`Slider` take a width but
their height is intrinsic (`TextInput` also wraps itself in a vertical layout
with the label *above* the field). `ui.put(rect, w)` centres a widget at its
intrinsic size inside the rect — it does not stretch it. In a RAD tool where the
developer drags a Button to exactly 200×40, a real `elegance::Button` would
render ~90×30 centred in that box. Worse, the designer canvas has only a
`Painter` (no `Ui`) and *must* hand-paint at the full rect — so live and
designer would disagree, breaking spec 007's own parity requirement (its R5/AC4)
that a form look identical in designer, desktop and WASM.

Hand-painting avoids all of it: exact geometry, automatic parity (one painter
feeds both surfaces), and one change reaching ~15 control kinds. The palette is
fully public (`Palette::slate()`, all fields `pub`, no `Context` needed), so the
result is a genuine colour match, not a guess. Interaction is unaffected —
Button/CheckBox/Slider/TabControl already hand-roll their own hit-testing on top
of a painted face, and that code is untouched.

*Rejected — real widgets where "rect-friendly".* Only `ProgressBar` and
`LinearGauge` expose both width and height. Mixing two rendering strategies
inside one theme buys a marginally more authentic ProgressBar at the cost of two
code paths and a parity seam, for one or two controls.

*Rejected — real widgets everywhere, letting geometry go.* Honest about what it
would mean: the developer's Width/Height stop being authoritative. That breaks
the RAD contract for a PowerCOBOL/isCOBOL audience whose instinct is absolute
positioning. Not acceptable.

*Note:* R6 is unaffected — the four existing spec-039 widgets stay real widgets
and genuinely benefit from `Theme::install`.

**Decision 2 — A parallel `set_form_style` rather than widening
`set_active_theme`.** Mirrors the proven `set_glass_style` shape, is purely
additive, and every host already has the per-frame block to extend. Rejected:
changing `set_active_theme` to an enum — a breaking change across 3 hosts and
several tests, for no gain.

**Decision 3 — Slate only, hard-coded.** Per resolved Q1. `elegance_palette()`
returns `Palette::slate()` from one place, so a future spec adding
Frost/Charcoal/Paper changes that one function plus catalog entries.

**Decision 4 — `Theme::install` belongs to the host, not the renderer.**
Consistent with Fact C (hosts own per-frame context publishing) and keeps
`cobolt-forms`' render path free of context mutation.

## 5. Risks & mitigations

**R-1 — Designer↔runtime divergence on the eight two-painter controls
(highest).** DataGrid, TreeView, MenuBar, ToolBar/StatusBar, Splitter, ListBox,
ComboBox, NumericUpDown each have an independent static and live implementation.
Nothing structurally forces them to agree, and a mismatch is a *visual* bug
tests won't catch by themselves. → Mitigation: derive **every** colour in both
implementations from the shared `elegance_palette()` helper (no literals), and
pair each with an explicit side-by-side visual check in `/tasks` (§6 M-2). This
is the risk I would watch most closely.

**R-2 — The seam regresses Liquid Glass.** The 7 rewired sites are shared with
Liquid Glass and asset packs, and AC8/AC10 demand pixel-identity. → Mitigation:
the non-Elegance arm is a literal pass-through, and the shape-count test in §6
(T-4) runs across all four `GlassStyle` values *before* the Elegance arm is
written, establishing a baseline.

**R-3 — Compiler warns / hunts for a nonexistent pack.** `wanted_theme_ids`
drops only the Liquid Glass default today. → Mitigation: drop procedural ids
generally (both, by kind — not a hard-coded pair), covered by a test.

**R-4 — Scope. R11 forbids a partial-coverage ship point**, and this is ~15
shared-painter kinds plus 8 doubled ones. → Mitigation: `/tasks` sequences
shared-painter work first (broad coverage, low risk) and the doubled controls
after, each independently verifiable; no task leaves the tree red.

**R-5 — `Theme::install` mutates global egui style.** It calls
`ctx.global_style_mut`, which in the **IDE** shares a `Context` with the IDE's
own chrome — so installing it could restyle the IDE around the canvas. → This is
a real hazard specific to the IDE surfaces (designer/preview), not the
standalone host. Mitigation: `/tasks` must verify IDE chrome is unaffected; if
it bleeds, scope the install to the form viewport or drop it in the IDE and rely
on the documented `Theme::slate()` fallback (which is what happens today
anyway).

**R-6 — Font side-effect.** `install` registers the bundled symbols font as a
lowest-priority fallback via `add_font`. Additive and idempotent, but it touches
font state the IDE also configures. → Verify no glyph regressions in the editor.

## 6. Test strategy

**Automated** (`cargo test -p cobolt-forms`, `-p cobolt-ide`, `-p
cobolt-compiler`):

- **T-1 catalog** — `builtin()` contains exactly `liquid-glass` then `elegance`,
  both `ThemeKind::Procedural`, display names `"Liquid Glass"` / `"Elegance"`;
  `resolve_theme_id` precedence unchanged (regression); unknown id still falls
  back. *Reports* the catalog ids in order.
- **T-2 wire** — `set_form_style`/`active_form_style` round-trip; a context that
  was never told defaults to `LiquidGlass` (proves R10's "forgot to call it"
  path).
- **T-3 palette** — `elegance_palette()` matches `elegance::Palette::slate()`
  field-for-field, so a crate upgrade that shifts the palette fails loudly
  instead of drifting silently.
- **T-4 seam pass-through (AC10)** — using the existing `ctx.run_ui` +
  tessellated-shape-count harness already in `paint.rs` (cf. `shape_leaf_count`),
  assert that for a form under Liquid Glass **and** under an asset pack, the
  shape list is identical before and after the seam, across **all four**
  `GlassStyle` values. *Reports* the shape counts per style. This is the
  criterion I flagged as hardest — it is a structural proxy for "pixel-
  identical", not a true pixel diff, and §6-M1 backs it with an eyeball check.
- **T-5 coverage (AC5/R11)** — for every `ControlType` in R4, assert the
  Elegance path is taken (not the glass fallback), so an uncovered family fails
  the build rather than shipping quietly. *Reports* a covered/total tally by
  control name — the "which 15", not just "15".
- **T-6 compiler** — `wanted_theme_ids` drops `elegance` (no phantom pack
  lookup, no warning).
- **T-7 i18n** — `cargo test -p cobolt-ide i18n` stays green (expected: no new
  keys).

**Manual / visual** (`cargo run -p cobolt-ide`):

- **M-1 (AC2, AC3)** — a form carrying every R4 control; switch project default
  and per-form override to Elegance → canvas re-renders immediately; compare
  against the crate's own Slate palette.
- **M-2 (R-1, the parity check)** — for each of the eight two-painter controls,
  put the designer canvas and the running form side by side and confirm they
  match. The single most likely place for a defect.
- **M-3 (AC4, R6)** — a form with Knob/Gauge/Switch/FileDropZone under Elegance:
  those widgets share the palette with everything around them; symbol glyphs
  render (not tofu).
- **M-4 (AC6)** — set an explicit `BackgroundColor`/`ForegroundColor` under
  Elegance → the developer's colour wins.
- **M-5 (AC9)** — cycle all four `GlassStyle` values under Elegance → **nothing
  changes**, frames *and* sub-elements (watch the CheckBox tick box, the site
  that motivated R13).
- **M-6 (AC8, R-5, R-6)** — an untouched Liquid Glass form looks unchanged; IDE
  chrome and editor glyphs unaffected by `Theme::install`.
- **M-7 (R5 parity)** — `rcrun run-form` on an Elegance form matches the IDE.

## 7. Steering compliance

- [x] **i18n** — no new UI strings expected ("Elegance" is a product term, like
      "Liquid Glass"). `/tasks` re-checks; any new string is ×6.
- [x] **Generated-code banner + regenerate-on-action** — untouched; rendering
      only.
- [x] **English dev guide** — extend "Form themes and styles" (line 1464);
      translations never edited.
- [x] **System KB** — no control/property/method/event change expected; run the
      freshness check anyway and treat red as a real failure.
- [x] **No "cobolt" / no crate name in user-facing text (R9)** — "Elegance"
      only; `egui-elegance` stays build-only. AC7 greps for it.
- [x] **Fix vs feature / version** — **Feature → `VERSION = "1.61.0"`**
      (`1.60.49` → minor bump, `z` reset). This follows `tech.md`'s rule for
      features; the operator's standing rule reserves x/y bumps for their own
      say-so, and they **explicitly authorised this one**. Also required:
      a `CHANGELOG.md` entry, its **own commit** (never mixed with a fix), and
      an **f=96** announcement carrying the `[Noticia]` prefix — not f=97.
