<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The stylesheet's values in the Form Designer and the Properties panel, and edits written back to the rule (WASM-036)

- **Status:** in progress. T1 done (2.0.362), T2 done (2.0.363), T3 done (2.0.364), T4 done (2.0.365), T5 done (2.0.366), T6 done (2.0.367), T7 done (2.0.368); T8 next.
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-07 (2.0.302, on `features-2x`)

These are small, ordered tasks that can each be checked on their own. Each one names:
- the files it touches;
- the requirements it satisfies;
- how it is verified.

Native tests print one result block (GOLDEN RULE #7): the cases they ran, the counts, and for timed ones the milliseconds and the throughput. The browser test runs headless Chrome on 127.0.0.1 only. Any throwaway tooling stays outside the repository (PRIME DIRECTIVE).

**Order.** The engine comes first (`cobolt-forms`, no egui), then painting, then the IDE's showing, then writing, then the colour scheme, then Check and the agent. The workspace builds and its tests stay green after every task.

**Each task as it lands:**
- bumps `z` in `crates/cobolt-ide/src/version.rs`;
- adds a top `CHANGELOG.md` entry;
- updates this spec's row in `specs/WASM-PROGRESS.md`;
- is committed on `features-2x`, fast-forwarded into `2.x.x`, and pushed outside the forbidden window.

Tasks committed together share one bump. `README.md` and `docs/` are never touched on 2.x.x.

**Commands used below:**

| What | Command |
|---|---|
| forms | `cargo test -p cobolt-forms --features render <filter>` |
| forms as the player builds it (no `render`) | `cargo build -p cobolt-forms --no-default-features` |
| player (outside the workspace) | `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib <filter>` |
| project tools | `cargo test -p cobolt-project-tools <filter>` |
| `rcrun` | `cargo build -p cobolt-cli --bin rcrun` (built on its own: a `--bin cobolt-ide` filter leaves it stale) |
| IDE | `cargo test -p cobolt-ide --bin cobolt-ide <filter>` |
| Chrome | `cargo test -p cobolt-web-bundle --test css_effective_chrome -- --nocapture` |
| System KB | `cargo run -p cobolt-ide --example build_chunked_kb` |

**IDE tests that drive frames** go in a new `crates/cobolt-ide/src/panels/designer_css_effective_tests.rs`. It is included from `designer.rs` with `#[path]`, as `designer_page_backdrop_tests.rs` is (designer.rs:27650), and its fixtures are temporary projects.

**What `css_effective` may depend on.** `css_effective` and `css_edit` build without the `render` feature, because the player depends on `cobolt-forms` with `default-features = false`.
- `RenderInput.css_final` is the only part behind `render`.
- `CssFinal` itself is plain data.

**Out of scope:** the defect found while planning (`App::open_stylesheet_modal` returns at once for a form with no sheet file, app.rs:15489). It is a fix of its own on `fixes-2x`, proposed to the operator separately.

---

## Phase A — The engine (`cobolt-forms`, no egui)

- [x] **T1 — The CSS subset: attribute selectors, `:host`, decided media queries, declaration offsets** (R1, R4, R25 engine half)
  - Files: `crates/cobolt-forms/src/css.rs`, `crates/cobolt-forms/src/viewer.rs`.
  - Do:
    - **Attribute selectors:** `[a]`, `[a=v]`, `~=`, `|=`, `^=`, `$=`, `*=`, and the `i` flag. They replace `'[' => return None` (css.rs:365-367).
    - **`ElementRef` gains `attrs: &[(&str, &str)]`.** The Viewer passes `&[]` at its two construction sites, so a rule it used to drop now matches nothing there.
    - **`:host`** matches the root element of the path.
    - **`MediaEnv { width, height, scheme }`**, and `media_eval(list, &MediaEnv) -> Tri` answering `Holds`, `Fails` or `Unknown`:
      - decided: `width`, `height` and their min/max forms, `orientation`, `aspect-ratio`, and `prefers-color-scheme` against `scheme`;
      - `Unknown`: `hover`, `pointer`, the other `prefers-*` features.
    - **`media_matches` keeps its signature and answers:** it is `media_eval` at 1024 px, with `Unknown` read as its old fixed answer.
    - **Parsed declarations carry byte ranges** for their name, their value and the whole declaration, and say whether they are `!important`.
    - **`pub(crate)` access** to the selector type and `parse_selector`.
  - Verify:
    - forms `css` green, with new cases for:
      - each attribute operator and the `i` flag;
      - `:host`;
      - `media_eval` at 412 × 915 in Light and in Dark;
      - `hover` and `prefers-reduced-motion` giving `Unknown`;
      - byte ranges on a sheet with comments, CRLF line endings and odd spacing.
    - The Viewer's tests unchanged and green.
    - `media_matches` gives the same answers as before over its existing cases.
    - The forms build without `render` succeeds.
  - *Done 2026-11-09 (2.0.362):* `css.rs` reads `[a]`, `[a=v]`, `~=`, `|=`, `^=`, `$=`, `*=` and the `i`/`s` flag (an attribute counts as a class in specificity; `ElementRef.attrs`, `&[]` at the Viewer's construction site and everywhere else, so such a rule matches nothing there); `:host` matches the first element of the path; `MediaEnv`, `Scheme`, `Tri` and `media_eval` decide `width`, `height`, their min/max, `orientation`, `aspect-ratio` and `prefers-color-scheme`, and answer `Unknown` for `hover`, `pointer` and the other `prefers-*`; `media_matches` keeps its signature and answers `media_core` at 1024 px with the old fixed readings; `parse_declarations_spanned` and `blank_comments` give each declaration's name, value and whole byte ranges (offset-preserving comment blanking); `Selector`, `parse_selector`, `specificity` and `matches` are `pub(crate)`. Evidence: `cargo test -p cobolt-forms --features render css` 14 passed, 0 failed (4 new: every attribute operator, 24 selector cases + 8 malformed/specificity/cascade checks; `:host`; `media_eval` 25 queries x 2 schemes at 412 x 915; `media_matches` against a verbatim copy of the replaced implementation over 367 queries, identical; spans on a CRLF sheet with comments and an in-place edit); `cargo build -p cobolt-forms --no-default-features` builds. The existing selector test now expects `[attr]` kept (it asserted dropped, which R1 reverses).

- [x] **T2 — `css_effective`: the sheet set, the element path and the cascade** (R1, R4, R20)
  - Files: new `crates/cobolt-forms/src/css_effective.rs` (`pub mod` in `lib.rs`, not behind `render`), `crates/cobolt-forms/src/css_overrides.rs`.
  - Do:
    - **`SheetSet::parse(sources)`.** Each source is `(label, text, origin)`, with the origin `Theme`, `Project` or `Form`. It is built from `css_overrides::rules()` (css_overrides.rs:130), extended to keep:
      - each rule's parsed selectors;
      - its declarations, with their byte ranges;
      - its 1-based line;
      - its layer: `pr-theme` for a form sheet's theme block, and the origin otherwise;
      - its conditions, from the `@media`, `@container` and `@supports` preludes around it.
    - **The element path**, as the player mounts it:
      - each control: `id`, and `data-name` (its name, case kept), with `data-layer` on a layered control;
      - the root: `section.pr-form-root` with `data-pr-form` and `data-pr-inst`;
      - the TabControl page and the Splitter pane.
    - **The cascade:** the key `(rank, specificity, order)` of plan §1.1. Ranks 0–4 are normal declarations, 5–9 are `!important` in reverse layer order, and `CssStyle` is the (1,1,0) rule adopted last.
    - **`var()`** resolves through the custom properties cascaded on the element path (they inherit), with `css::substitute_vars`.
    - **Certainty:** a declaration under an `Unknown` condition, or whose subject has a pseudo-class, is "may". It is recorded, and it never wins.
    - **`css_overrides` becomes a thin view** over the engine. Its public items and three tests are unchanged.
  - Verify:
    - forms `css_effective` green, with one case per row of plan §6.1's cascade list:
      - each layer against the next;
      - `!important` reversing them;
      - `CssStyle` beaten by a two-id form-sheet rule;
      - source order;
      - specificity;
      - `var()` through the path and through `:host`;
      - the form's own path, the TabControl path and the Splitter path;
      - `[data-name="BTN-1"]`;
      - a `:hover` rule recorded as "may".
    - forms `css_overrides` green: its three existing tests, unchanged.
    - The forms build without `render` succeeds.

- [x] **T3 — The reverse table and `effective()`** (R2, R3, R7 data; AC1)
  - Files: `crates/cobolt-forms/src/css_effective.rs`.
  - Do:
    - **`REVERSE`**, one row per CSS property of `CSS_TO_PROPERTIES`, decided as in plan §1.2's table. Each row has:
      - its fidelity (`Exact`, `Approximate` or `NotReversible`);
      - `read(value, &Ctx)`;
      - for writable rows, a value writer to CSS text: `#RRGGBB` / `#RRGGBBAA`, `px`, and notation-keeping for `rgb()`, `hsl()` and names.
    - **`Ctx`** holds the control's type, its effective font size and the root font size, so `em` and `rem` resolve.
    - **Every value of R3 is `Unknown`** and carries its CSS text: `%`, `vw`, `vh`, `calc()`, `env()`, an unresolved `var()`, `url()`, a gradient the model lacks, several shadows, `inherit`, `unset`, `revert`.
    - **`effective(form, id, &SheetSet, &MediaEnv) -> Effective`** returns `Entry { property, value, source, certain, shared }` and `unpainted`:
      - `unpainted` lists what the canvas cannot paint: outside the table, `Unknown`, geometry or layout, "may";
      - `shared` is decided as in plan §1.4.
    - **`CssFinal`** is plain data: per control id, the face, gradient, border and shadow colours to paint as given.
  - Verify: forms `css_effective` green, with:
    - **AC1:** an enumeration of `CSS_TO_PROPERTIES` that finds a `REVERSE` row with a fidelity for each CSS property;
    - **AC1:** for each `Exact` row, a round trip in every notation (hex, `rgb()`, `hsl()`, a name; `px`, `em`, `rem`), property → CSS → the same property;
    - one case per `Unknown` kind of R3;
    - `unpainted` holding a `width: 50%` declaration with its line;
    - `shared` true for `.pr-button` and false for `#BTN-1`.

- [x] **T4 — The props layer as data, pinned to the player** (R1; plan §0.2)
  - Files:
    - `crates/cobolt-forms/src/css_effective.rs`: `props_layer(ctl, form) -> Vec<(&'static str, PropsDecl)>`, used at rank 2 by `effective()`;
    - `crates/cobolt-web-player/src/css.rs` and `crates/cobolt-web-player/src/dom/instance.rs`: the tests.
  - Do:
    - **Write §0.2 down as data:**
      - which CSS properties the player's `control_decls` writes for a control, and from what (`Own` or `Theme(value)`);
      - `#F0F0F0` writing nothing;
      - Bold as `font-weight: 400` plus a text-shadow;
      - the recomputed padding on Label, Button and TextBox;
      - Spatial's `spatial_decls`;
      - the Transparency fade.
    - **When the props layer wins,** the entry's value is the stored property.
  - Verify: player `props_layer`, `element_path` and `root_font` green:
    - `props_layer` equals `control_decls`'s CSS names and Spatial values over a corpus: every `ControlType`; seeded, absent and edge values (`#F0F0F0`, radius 0, no `BorderStyle`, Bold); Spatial on and off;
    - the IDE's element path equals what `dom/instance.rs` mounts for the corpus's containers;
    - the root font size equals the one `css_effective` uses for `rem`.

    The player's existing `the_ides_override_map_names_rows_of_this_table` stays green.

- [x] **T5 — `css_edit`: the planner and Remove** (R9, R10, R11 "Only this control", R12, R14 Remove, R16, R18; AC6, AC8)
  - Files: new `crates/cobolt-forms/src/css_edit.rs` (`pub mod`, not behind `render`).
  - Do: `plan(form, id, property, new, sources, env, choice) -> Result<Plan, Refusal>`, as in plan §1.5:
    1. **Not certain, or from the theme:** `Plan::StoreProperty`.
    2. **`NotReversible`, `Visible` or `ZOrder`:** `Refusal::NotWritable`.
    3. **The edit:**
       - a rewrite of the value's byte range in its own notation, keeping `!important`;
       - a shorthand's owned component replaced, or the longhand inserted after the shorthand's `;` with its indentation;
       - `CssStyle` as a `SetProperty("CssStyle")`;
       - **Only this control:** into the last unconditional, unlayered `#NAME` rule of the form sheet, or appended as a new rule (a leading digit escaped as `\3N `). `!important` only when the shared declaration has it. A form with no sheet file gets `web_themes::form_stylesheet_for`'s text, with the rule after `/* Your rules. */`.
    4. **The in-memory effect check:** `effective()` over the new texts. If the value is not the one typed, `Refusal::NoEffect { winner: file, line }`, and no text is returned.
    5. **The lint gate:** `stylesheet_lint::lint` may not gain an Error-severity finding.

    **Targets (R18):** a plan writes only to the project sheet, the form sheet and `CssStyle`. Never to the theme, or to the form sheet's `pr-theme` block: a winner there is `StoreProperty` (step 1). Never to a file outside the project.

    `remove(decl)` removes the declaration's byte range, and its line when the declaration had the line to itself. A rule left empty stays.
  - Verify: forms `css_edit` green.
    - **AC6:** byte diffs for:
      - a longhand;
      - `border: 1px solid red`, then a BorderColor edit;
      - a shorthand that needs a longhand;
      - `!important`;
      - `CssStyle`;
      - each colour notation;
      - an id starting with a digit;
      - a sheet with comments, CRLF line endings and odd spacing.

      Each shows that only the named bytes changed, and that the effective value afterwards is the one typed.
    - **AC8:** a later `!important` rule gives `NoEffect`, naming the file and line, with the texts unchanged.
    - **Lint:** a write that would add an Error-severity finding is refused.
    - **Targets:** a value won by the form sheet's `pr-theme` block plans `StoreProperty`, and no sheet text changes.
    - **Remove:** the declaration and its line go; the empty rule stays.
    - **AC7 (pure half):** "Only this control" appends `#BTN-1 { … }` at the end of the form sheet.

- [x] **T6 — Caches and the performance budget** (R20; AC15)
  - Files: `crates/cobolt-forms/src/css_effective.rs`: an `EffectiveCache`, held by the designer from T9.
  - Do:
    - **The `SheetSet` key:** a hash of the source texts and the theme id.
    - **Each control's `Effective` key:** its id, type, parent, layer, CssClass and CssStyle; the stored values of the mapped properties; the `MediaEnv`; the `SheetSet` key.
    - Selectors are parsed once.
    - A lookup with nothing changed computes nothing.
  - Verify: forms `css_effective_perf`. A fixture of 200 controls and 500 rules is generated in the test. The result block prints:
    - the whole recompute in ms, against a budget of ≤ 100 ms in the debug build;
    - a cache hit in ms;
    - controls × rules per second.

    A `--release` run is recorded in `WASM-PROGRESS.md`, expecting ≤ 16 ms. **AC15.**

## Phase B — Painting

- [x] **T7 — `RenderInput.css_final` on both render paths** (R6)
  - Files: `crates/cobolt-forms/src/render.rs` (the face, gradient, border and shadow colour functions of `render_form` and `render_faces`).
  - Do:
    - `RenderInput` gains `css_final: Option<&CssFinal>`, `None` at every existing call site.
    - A colour named there is painted as given, not faded by `Transparency`.
  - Verify:
    - `cargo test -p cobolt-forms --features render` green, the whole crate.
    - `engine_reference_form_parity_static_vs_faces` green, with a `css_final` case added.
    - A new test: a CSS colour on a `Transparency` 50 control paints unfaded on both paths.

- [ ] **T8 — `render_form` paints the effective values, with `color_scheme`** (R17; AC12 renderer half)
  - Files:
    - `crates/cobolt-forms/src/snapshot.rs` (`render_form_png`: the effective clones and `CssFinal` for a Web or Mobile project, at the size it lays out, plus the `color_scheme` option);
    - `crates/cobolt-project-tools/src/tools/mod.rs` (the schema: `"color_scheme": {"enum": ["light","dark"]}`, `light` when absent);
    - `crates/cobolt-project-tools/src/tools/render.rs`;
    - `crates/cobolt-ide/src/agent_kit/ide_host.rs` (`form_renderer`);
    - `crates/cobolt-cli/src/main.rs`.
  - Verify:
    - project tools `render_form` green, on the AC2 fixture (a temporary Mobile project):
      - without `color_scheme`, the styled button's pixel is the CSS colour within 1 per channel;
      - with `color_scheme: "dark"`, a rule under `@media (prefers-color-scheme: dark)` gives its colour;
      - a call without the argument answers as before;
      - a Desktop project's picture is byte-identical to before.
    - The `rcrun` build succeeds, and a test running both renderers on the fixture gives the same pixels.
    - Comparing with the canvas closes in T9. **AC12.**

## Phase C — Showing it in the IDE

- [ ] **T9 — The designer's canvas and badge** (R6, R7, R8, R20; AC4, AC5)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`, `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`, the new `designer_css_effective_tests.rs`.
  - Do:
    - **The `MediaEnv`** comes from `canvas_size()` and from `css_scheme`, which is Light until T15.
    - **The cache:** the designer holds an `EffectiveCache`.
    - **The effective-clone stage** joins `controls_for_render` (designer.rs:10852-10898).
    - **The background code** (designer.rs:10279-10598) reads an effective `FormLook`.
    - **`css_final`** is passed in `RenderInput`.
    - **The badge** (designer.rs:11087-11095) is drawn when `unpainted` is not empty. Its tooltip is `css_badge_header`, up to eight `file:line  declaration` lines, then `css_badge_more`. It replaces the `idef` literal.
    - **Refresh (R8):** an IDE write and Apply update `css_sheets` in memory; another program's save is picked up by the existing 1 s re-read.
    - **New `Tr` keys**, in six languages: `css_badge_header`, `css_badge_more`.
  - Verify: IDE `designer_css_effective` green.
    - **AC4:** the canvas fill of the button styled by `#BTN-1 { background-color: rgb(10 132 255) }`, read from the frame's shapes, is `#0A84FF`.
    - **AC4:** `render_form` of the same project gives that pixel within 1 per channel.
    - **AC4:** a `width: 50%` control keeps its designed rectangle, and the badge's tooltip lists that line.
    - **AC12:** the canvas pixel equals `render_form`'s in Light and in Dark.
    - **AC5:** the sheet is rewritten on disk; the fill changes within 1 s of simulated time, and the time is printed.
    - **R20:** a frame with nothing changed computes nothing (counted).
    - **R6:** the form file is byte-identical after the frames.

- [ ] **T10 — The Properties panel shows the effective values** (R5, R15 display; AC3, AC11 display half)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`, `designer_css_effective_tests.rs`.
  - Do:
    - **The effective clone:** `show_control` gets the selected control's effective clone, as the breakpoint view passes `overridden_control`. `show_form` gets the form with its effective look.
    - **`CSS_MARKS`** (properties.rs:14357) carries the source text and the stored value. Its tooltip uses `css_not_in_effect` or `css_from_theme`. "May be overridden" keeps its text.
    - **`CSS_UNKNOWN`:** `property_row_keyed` (properties.rs:14134) draws the CSS text read-only, with `css_unknown_value`. The rows not built on `property_row_keyed` (`grep`) are listed in this task's evidence, and each is given the same check.
    - **`CSS_MIXED`:** for a multiple selection, a "≠" mark whose tooltip (`css_mixed_values`) lists the values.
    - **New `Tr` keys**, in six languages: `css_from_theme`, `css_not_in_effect`, `css_unknown_value`, `css_mixed_values`.
  - Verify: IDE `designer_css_effective` green, driving real frames.
    - **AC3:** the BackgroundColor row shows `#0A84FF`.
    - **AC3:** the mark names the sheet and line in each of the six languages.
    - **AC3:** the stored value is in the tooltip.
    - **AC3:** a `calc()` value is drawn read-only.
    - **AC11 (display):** three selected buttons with two effective colours show mixed.
    - The existing `properties` tests stay green.

## Phase D — Writing it

- [ ] **T11 — The executor: `Cmd::SetStylesheet`, `css_apply`, `css_route`** (R9, R10, R12, R13, R18, R8; AC6 through the IDE, AC8, AC9)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`, `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/panels/stylesheet_modal.rs`, `crates/cobolt-ide/src/i18n.rs`, `designer_css_effective_tests.rs`.
  - Do:
    - **`Cmd::SetStylesheet { path, before, after }`**, following `SetMenuDefinition`. Execute and redo write `after` only while the file holds `before`; reverse writes `before` only while it holds `after`. Otherwise it stops with `css_undo_changed_on_disk`.
    - **`DesignerPanel::css_apply`:**
      - refuses while `stylesheet_modal` holds unapplied text for that sheet (`css_write_refused_unapplied`);
      - reads each sheet from disk, and plans again when the text differs from what the marks were computed on;
      - calls `css_edit::plan`, and reports `NoEffect` (`css_write_no_effect`, naming the file and line) or `NotWritable` (`css_write_not_writable`);
      - records one `Cmd::Batch` of `SetStylesheet` steps and any `SetProperty("CssStyle")`;
      - updates the in-memory sheets, and calls `web_preview.apply_form_stylesheet` (app.rs:15511).
    - **`css_route`** in both `set_props` consumers (app.rs:20883 and app.rs:8223). It drops a value equal to the one in effect: the `lost_focus` filter.
    - **The gesture rule:** while a picker or a drag is changing the value, the planned text replaces the sheet in memory only. One write and one undo step happen when the gesture ends.
    - **New `Tr` keys**, in six languages: `css_write_refused_unapplied`, `css_write_no_effect`, `css_write_not_writable`, `css_undo_changed_on_disk`.
  - Verify: IDE `designer_css_effective` green, through `css_route` on a temporary project.
    - **AC6:** a longhand write and a shorthand write, each shown by a byte diff.
    - **AC8:** `NoEffect` writes nothing, the files stay byte-identical, and the message names the winner.
    - **AC9:** Ctrl+Z and Ctrl+Y byte for byte; the unapplied-modal refusal; a sheet changed on disk between read and write is read again, and the other program's change kept; undo stops after an outside change.
    - **Gesture:** 30 frames of picker changes make one file write and one undo step.
    - **`lost_focus`:** focusing and unfocusing every text row of an overridden control writes nothing.

- [ ] **T12 — The shared-rule question window** (R11, R15 one question; AC7, AC13)
  - Files: new `crates/cobolt-ide/src/panels/css_rule_question.rs` (copied from `indexed_engine_modal.rs`), `crates/cobolt-ide/src/panels/designer.rs` (`pending_css_question`), `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`.
  - Do:
    - **The window:** its own `size`, `.resizable(false).fixed_size(size)`, one grip in a foreground `Area`. Read the skills `egui-paint-regressions` and `rounded-corners` first.
    - **Its content:** the rule (file, line, selector) and how many other controls it styles. A form-sheet rule counts on this form; a project-sheet rule counts on every form of the project, loaded once per question, said as "at least N" after a time limit.
    - **Its answers:** **Change the rule**, **Only this control**, **Cancel** (Cancel reuses the existing key).
    - **The pending edit** is held as `style_break_pending` holds the background edit (app.rs:20918).
    - **New `Tr` keys**, in six languages: `css_shared_title`, `css_shared_body`, `css_shared_change_rule`, `css_shared_only_this`.
  - Verify: IDE `designer_css_effective` and `css_rule_question` green.
    - **AC7:** a `.pr-button` rule styles three buttons, and the question names 2 others. Change the rule changes all three. Only this control adds `#BTN-1 { … }` at the end of the form sheet, and only BTN-1 changes. Cancel leaves both files byte-identical.
    - **AC13:** the window's rectangle holds over 120 frames in each of the six languages, and keeps the dragged size after a drag of its grip.

- [ ] **T13 — The row actions: Go to rule, Remove the CSS declaration** (R14, R16; AC10)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (the mark's context menu; `InspectorAction.css_go_to`, `css_remove`), `crates/cobolt-ide/src/panels/stylesheet_modal.rs` (`pub fn open_at(line)` over the existing `jump_to`), `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`.
  - Do:
    - **Go to rule** opens the stylesheet modal at the line, or the Style editor for `CssStyle`.
    - **Remove** asks first (`css_remove_confirm`), then runs `css_edit::remove` through the same `SetStylesheet` undo.
    - **Rows that are not writable** (R16: `NotReversible`, Visible, ZOrder) offer only these two actions.
    - **New `Tr` keys**, in six languages: `css_go_to_rule`, `css_remove_declaration`, `css_remove_confirm`.
  - Verify: IDE `designer_css_effective` green.
    - **AC10:** Remove asks first, removes only that declaration (byte diff), and undo puts it back.
    - **AC10:** Go to rule opens the modal with the caret on the rule's line.
    - A Visible row overridden by `display: none` is not written; it offers only the two actions.

- [ ] **T14 — Edits with several controls selected** (R15; AC11)
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/panels/designer.rs`.
  - Do:
    - An edit applies R9–R13 to each selected control.
    - One question covers every shared rule involved.
    - The whole edit is one undo step.
  - Verify: IDE `designer_css_effective` green.
    - **AC11:** three buttons with two different effective colours. One edit gives all three the typed value; there is one question; one Ctrl+Z restores every file byte for byte.

## Phase E — The colour scheme

- [ ] **T15 — Light / Dark in the designer** (R24, R25; AC20)
  - Files: `crates/cobolt-ide/src/panels/designer.rs` (`css_scheme`, a two-state button in `draw_icon_toolbar` for Web and Mobile forms), `crates/cobolt-ide/src/ui_prefs.rs` (`UiPrefs`, ui_prefs.rs:24: the scheme per project path), `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`.
  - Do:
    - **The choice** sets `MediaEnv.scheme` and invalidates the `Effective` caches.
    - **It is never written** to the form file or a stylesheet. A project starts in Light.
    - **The Preview, Run and the built application** are not driven by it.
    - **New `Tr` keys**, in six languages: `css_scheme_light`, `css_scheme_dark`, `css_scheme_tip`.
  - Verify: IDE `designer_css_effective` green, with `@media (prefers-color-scheme: dark) { #BTN-1 { background-color: #111111 } }`.
    - **AC20, Light:** the canvas and the row show the other value, and the rule is not marked "may".
    - **AC20, Dark:** they show `#111111` as certain.
    - **AC20, an edit in Dark:** it rewrites the declaration inside the dark block (byte diff).
    - **AC20, saved and reopened:** the form file is byte-identical in both schemes, and a fresh `UiPrefs` load restores the choice.
    - **AC20, the texts:** in the six languages.
    - The button is absent on a Desktop form.

## Phase F — Check and the agent

- [ ] **T16 — `check` notes the overrides** (R21; AC17)
  - Files: `crates/cobolt-project-tools/src/tools/check.rs` (`check_form`).
  - Do: for a Web or Mobile form, add `Severity::Info` notes:
    - one per certain override: the control, the property, the effective value, the winning file and line, and `[light]`, `[dark]` or `[light, dark]`;
    - one per selector the subset cannot evaluate, with its line.

    Both schemes are evaluated only when a sheet contains `prefers-color-scheme`; otherwise one pass serves both. The result and the Build are unchanged.
  - Verify: project tools `check` green.
    - **AC17:** the AC2 fixture lists one note per certain override, with file, line and scheme, and one note for a `:has()` selector.
    - **AC17:** its result and exit status equal those of the same project without the stylesheet.
    - A Desktop project's report is unchanged.

- [ ] **T17 — The agent reads: its context and `form.properties`** (R22; AC18)
  - Files: `crates/cobolt-ide/src/agent.rs` (`build_context_for`: one override line per control and for the form; the existing headers untouched), `crates/cobolt-ide/src/tool_exec.rs` (the read-only `form.properties` tool, its contract text beside the others, its test following `CHANGE_SET_CONTRACT`'s at tool_exec.rs:1946).
  - Do:
    - **The tool's arguments:** `{"control": "<name>"}`, or `{}` for the form.
    - **Its answer:** each mapped property's stored value, effective value and source (`property`, `theme`, `file:line`, `Style`), at the designer's `MediaEnv`.
    - **It runs** only while a designer is open, as `egui.tree` does.
  - Verify: IDE `form_properties` and `build_context` green.
    - **AC18:** every mapped property of each fixture control and of the form, in Light and in Dark.
    - **AC18:** the context carries the override lines.
    - The contract test is green.

- [ ] **T18 — The agent writes through the panel's path** (R23; AC19)
  - Files: `crates/cobolt-ide/src/panels/designer.rs` (`apply_agent_change_set`'s `SetProperty` arm, designer.rs:4081 → `css_apply` for a Web or Mobile form; operations held in `pending_css_question`), `crates/cobolt-ide/src/agent.rs` (the ledger lines in `last_change_outcome`).
  - Do:
    - **Each operation's ledger line** is one of: `stored`; `written <file>:<line>`; `held: waiting for the developer's answer`; `no effect: … wins`; `not writable`; `refused: …`.
    - **The whole change set** stays one undo step.
    - **A Desktop form** keeps the raw `Cmd::SetProperty`.
  - Verify: IDE `designer_css_effective` green.
    - **AC19:** BackgroundColor on a control a form-sheet rule overrides writes that declaration (byte diff); the ledger names the file and line; Ctrl+Z restores the sheet.
    - **AC19:** on a shared rule the operation is held until answered, and Cancel leaves the files byte-identical.
    - **AC19:** on a Desktop form the change set is applied as before.

## Phase G — Closing

- [ ] **T19 — i18n audit** (R24 texts; AC13 last bullet, AC20 texts)
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do:
    - Confirm the 20 keys of plan §3 exist, each in EN ES PT JA ZH FR (added by T9–T15).
    - Confirm the badge's literal is gone.
    - Confirm no user-facing text says "cobolt".
  - Verify: IDE `no_empty_ui_translations` and `non_english_is_actually_translated` green, and a `grep` of the 20 key names finds each one.

- [ ] **T20 — The System KB** (plan §1.7)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the WASM-007 CSS paragraph and the Properties panel's behaviour on Web and Mobile forms), `assets/knowledge/chunked.data` (regenerated).
  - Verify:
    - The KB regeneration runs.
    - IDE `prebuilt_chunked_kb_matches_the_published_documentation` green.
    - `strings` of `chunked.data` scanned for credentials before the commit.

- [ ] **T21 — The differential test in Chrome** (R1, R4; AC2)
  - Files: new `crates/cobolt-web-bundle/tests/css_effective_chrome.rs` (uses `tests/support/cdp.rs`, as `matrix_chrome.rs` does), and its fixture project under `crates/cobolt-web-bundle/tests/projects/`.
  - Do:
    - Mount the AC2 fixture at 412 × 915 in headless Chrome. It sets the same property at each level: theme, stored, project sheet, form sheet, class, id, `CssStyle`, `!important`, `@media (max-width: 500px)` and `:hover`.
    - For each case, compare `getComputedStyle` of the element the player writes to with `effective()`'s value and source.
  - Verify: the Chrome command prints "N of N" and the engine version. Where Chrome is absent the test says "not run" and claims nothing. **AC2.**

- [ ] **T22 — A Desktop project is unchanged** (R19; AC14)
  - Files: `designer_css_effective_tests.rs`.
  - Verify: IDE `designer_css_effective` green.
    - **AC14:** with a stylesheet placed beside a Desktop form, no mark shows and the canvas paints the stored properties.
    - The existing `designer` and `properties` tests stay green.

- [ ] **T23 — The operator's report, by eye** (AC16)
  - Files: none in the repository.
  - Do:
    - In a copy of `examples/PowerMobileHello` (never the example itself), give a form sheet button colours and a label font.
    - Compare the designer's canvas (`render_form`, or the operator's eye) with the emulator's screen. Run puts the application there through `rcrun` or the MCP tools; the IDE window is never driven.
    - The AVD runs with four cores.
  - Verify:
    - Both screenshots are kept in `PowerRustCOBOL-local-settings/evidence/WASM-036/`, outside the repository.
    - Their paths and what they show are written in `WASM-PROGRESS.md`. **AC16.**

- [ ] **T24 — Finalize**
  - Do:
    - Bump `z`, add the CHANGELOG entry, and mark this spec's row done in `WASM-PROGRESS.md` with the evidence of every AC.
    - Check off the ACs in `spec.md`, and set Status: done here and in the spec.
    - Before the commit, quarantine untracked program files in other languages and run the credential scan.
  - Verify:
    - `cargo test -p cobolt-forms --features render` green.
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`, with every `test result:` line read: only the three known reds of the 2.x line.
    - `cargo test -p cobolt-project-tools` green.
    - The player `--lib` green.
    - The `rcrun` build succeeds.
    - The Chrome test reported.

## Acceptance criteria → tasks

| AC | Requirements | Verified in |
|---|---|---|
| AC1 | R2 | T3 |
| AC2 | R1, R4 | T21 (engine cases in T2) |
| AC3 | R5 | T10 |
| AC4 | R6, R7 | T9 |
| AC5 | R8 | T9 |
| AC6 | R9, R10 | T5 (bytes), T11 (through the IDE) |
| AC7 | R11 | T5 (pure half), T12 |
| AC8 | R12 | T5, T11 |
| AC9 | R13 | T11 |
| AC10 | R14 | T13 |
| AC11 | R15 | T10 (display), T14 (edit) |
| AC12 | R17 | T8 (renderers), T9 (against the canvas) |
| AC13 | R11, i18n | T12, T19 |
| AC14 | R19 | T22 |
| AC15 | R20 | T6 |
| AC16 | the operator's report | T23 |
| AC17 | R21 | T16 |
| AC18 | R22 | T17 |
| AC19 | R23 | T18 |
| AC20 | R24, R25 | T15, T19 |

R18 (the write-back's safety and targets) is checked in T5 (the lint gate; theme blocks and files outside the project refused) and in T11.

## Done criteria

This spec is done when:
- every acceptance criterion in `spec.md` is checked, with its evidence named in `WASM-PROGRESS.md`;
- the sweeps of T24 are green apart from the three known reds;
- the System KB is regenerated;
- every task landed as a feature commit on `features-2x` (each with its own `z` and CHANGELOG entry), fast-forwarded into `2.x.x`, and pushed outside the forbidden window.

The Developer's Guide chapter is written when 2.x.x merges into main.
