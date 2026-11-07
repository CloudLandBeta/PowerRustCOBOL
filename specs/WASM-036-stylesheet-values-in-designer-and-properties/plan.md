<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The stylesheet's values in the Form Designer and the Properties panel, and edits written back to the rule

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-11-07
- **Branch:** `features-2x` (2.x.x)
- **Ground truth:** read 2026-11-07 on `2.x.x` at `73fbccaf` (2.0.299). File:line references are to that tree.

## 0. What the code says, and what it changes in the spec

Five facts found while planning shape the design. The spec is corrected in the same change where they touch it (§0.6).

### 0.1 The cascade the player builds

The player declares its layers once (`cobolt-web-player/src/base_css.rs:335`):

`@layer pr-base, pr-theme, pr-props, pr-app;`

| Source | Where it lands in the page |
|---|---|
| The player's base sheet | `@layer pr-base` |
| The theme's CSS, whole (`theme_layer`, base_css.rs:352) | `@layer pr-theme` |
| The theme block copied into the form's sheet | `@layer pr-theme`, the same layer |
| The declarations made from properties (`wrap_layer`, style.rs:856; one rule per control, selector `[data-pr-inst="N"] [data-name="NAME"]`) | `@layer pr-props` |
| **The project stylesheet** (`app_layer`, base_css.rs:363) | `@layer pr-app` |
| **The form's stylesheet**, the developer's rules (`apply_form_sheet`, base_css.rs:458) | **Unlayered**, inside `@scope ([data-pr-form="<name>"])` |
| **`CssStyle`** (`inline_css`, style.rs:823) | **Unlayered**, adopted last, as the rule `[data-pr-inst="N"] #NAME` |

Three consequences:
- **Normal declarations**, lowest to highest: base < theme < props < project sheet < form sheet and `CssStyle`.
- **`!important` declarations** reverse the layer order, as CSS says: an important declaration in `pr-theme` beats one in the form sheet.
- **`CssStyle` is not an inline style.** It is a rule of specificity (1,1,0), adopted after every form-sheet rule. A form-sheet rule with two ids (`#PNL-1 #BTN-1`) beats it.

The cascade code the IDE has today does not model this:
- `css_overrides::overrides` puts the project and form sheets at one level, with `CssStyle` above both.
- `cobolt_forms::css::Stylesheet::parse` skips `@layer` and `@scope` blocks whole (css.rs:134-160).

### 0.2 What the properties layer declares is not one row per property

The player writes a declaration only for what it decides to write.

**Skips and defaults:**
- **BackgroundColor** `#F0F0F0` at alpha 255 on a control writes nothing (`is_default`, css.rs:293), so the theme's face shows. A chart writes navy instead.
- **CornerRadius** writes only when > 0. **Border** writes only when the `BorderStyle` key exists.
- **FontName** writes only when non-empty, **FontSize** only when > 0, **Cursor** only when non-empty, **ZOrder** only when not 0.
- **Shadow** writes only when ShadowEnabled is set and the control can cast one.

**Values the player computes for itself:**
- **Bold** writes `font-weight: 400` plus a `text-shadow` (css.rs:432-444), not `700`.
- **Padding** on Label, Button and TextBox: `kind_decls` replaces it with a computed one, the property plus the player's insets (style.rs:384-423).
- **The Spatial theme** writes its own declarations into the same rule (`spatial_decls`, theme_css.rs:81), the radius among them (theme_css.rs:89-95).
- **Transparency** fades the face, gradient stops, border and shadow colours of the props layer. A colour from a developer rule is used as written.

### 0.3 Loaded forms lack properties

`xml.rs::seed_missing_props` (xml.rs:858) does not backfill these, so a control loaded from disk can lack them, and then the player writes no declaration for them:
- FontName, FontSize, Bold, Italic, Underline, Strikethrough;
- Cursor, Padding, Transparency, Tooltip;
- TextAlignment, VerticalAlignment, WordWrap;
- BorderColor and BorderWidth on most types.

The theme's value then applies on the page, so **Q4's "from the theme" is common**, not an edge case.

### 0.4 The IDE side

**The Properties panel**
- Every row reads the `&Control` it is given (157 `get_prop` calls in `properties.rs`).
- A substituted control is already passed for breakpoints (`overridden_control`, designer.rs:15346, used at app.rs:20822). That is the pattern for showing effective values.
- `text_row_hint` (properties.rs:15197) and `text_prop_row` (properties.rs:15648) push `set_props` on every `lost_focus`, changed or not. With effective values shown, an untouched row would write its effective value back. §1.5 filters that.
- There is no "mixed values" display anywhere (properties.rs:622, 4805).

**Undo, the canvas and the modal**
- **Undo:** `Cmd` (designer.rs:1102) has one variant that writes a file beside the form, `SetMenuDefinition` (designer.rs:1304, `write_menu_yaml` at :8462). It is the pattern for a stylesheet write.
- **The canvas:** it paints the controls with one call, `render_faces_with(&painter, origin, &input, …)` (designer.rs:10947). It is fed by a chain of substitutable `controls_for_render: &[Control]` slices (designer.rs:10852-10898). The form background is read from `self.form`'s fields (designer.rs:10279-10598).
- **The stylesheet modal:** it already has a private `jump_to` line (stylesheet_modal.rs:215, used at :436-471). It only lacks a public setter.

**What does not exist yet**
- The designer has **no light or dark mode for a form**: no `theme_mode` anywhere.
- The **Form Designer agent has no property tools.** It reads a context (`agent::build_context_for`, agent.rs:2926) and writes a change set (`AgentOp::SetProperty`, agent.rs:74; applied at designer.rs:4081 as a raw `Cmd::SetProperty`).
- The MCP **`render_form`** (`snapshot::render_form_png`, snapshot.rs:203) and **`check`** (`tools/check.rs:199`) read no stylesheet.

### 0.5 Engine gaps to close

- **Attribute selectors.** `cobolt_forms::css` drops any selector with `[`, so attribute selectors are unsupported (css.rs:365-367). A developer rule such as `[data-name="BTN-1"]` is invisible to the IDE today.
- **`:host`.** The theme's tokens are written on `:host` (web_themes.rs:167-180).
- **`@media`.** It is decided at a fixed 1024 px, with `prefers-color-scheme` always light (css.rs:593-623).

### 0.6 Spec corrections made with this plan

| Requirement | Correction |
|---|---|
| R1 | The cascade levels are those of §0.1, with the project sheet in `pr-app` and `CssStyle` a (1,1,0) rule adopted last. |
| R4 | `prefers-color-scheme` stays "may": the designer has no form colour mode. |
| R5 | "From the theme" means the theme's whole CSS, which the player adopts as `pr-theme`. A value from the copy in the form's sheet names that file and line. |
| R17 | The agent reads its context and writes a change set. "Reads" means the context, and "the set-property tool" means applying an `AgentOp::SetProperty`. |
| Q1–Q6 | The operator accepted the six recommendations on 2026-11-07. |

## 1. Approach

The work splits into three layers, and only the last one knows egui:
- **The engine:** `cobolt-forms`, pure, no egui, one computation (R1).
- **Its consumers:** the canvas, the Properties panel, `render_form`, Check, the agent's context.
- **Write-back:** a pure planner in `cobolt-forms` that produces new texts and checks they take effect, and an IDE executor that writes files and records undo.

### 1.1 The engine: `cobolt_forms::css_effective` (R1–R4, R20)

A new module. `css_overrides` becomes a thin view over it, so its callers and tests keep working until they move.

**`SheetSet::parse(sources)`.** Each source is `(label, text, origin)`, where the origin is `Theme`, `Project` or `Form`. It is parsed once per change of the texts (§1.8) into rules. Each rule has:
- its selectors, parsed once (no more `Stylesheet::parse(format!("{sel} {{ x: y }}"))` per test, css_overrides.rs:279);
- its declarations, each with the **byte ranges** of its name, value and whole declaration, and whether it is `!important`;
- its 1-based line;
- its layer: the source's origin, or for a `Form` source the `@layer pr-theme` block it sits in;
- its condition list, from enclosing `@media`, `@container` and `@supports` preludes.

The layer-aware scanner is `css_overrides::rules()` (css_overrides.rs:130), extended to keep the layer name and the declarations' offsets. `cobolt_forms::css` gains `pub(crate)` access to its selector type and `parse_selector`.

**The theme source.** It is `web_themes::theme_css(<the form's resolved theme>)`, the whole theme as the player adopts it, labelled with the theme's name. The form sheet's `pr-theme` block is the same rules in the same layer; a value it gives names its file and line.

**`MediaEnv { width, height }`.** The width and height are the designer's `canvas_size()`: the designed size, or a responsive form's view size. `css::media_eval(list, &MediaEnv) -> Tri` answers `Holds`, `Fails` or `Unknown`:
- `width` / `min-width` / `max-width`, `height` (and its min/max), `orientation` and `aspect-ratio` are decided;
- `hover`, `pointer`, `prefers-*`, a `@container` query and `@supports` are `Unknown`, so the rule is "may" (R4, as corrected).

`css::media_matches(list)` keeps its signature and its desktop answers for the Viewer: `media_eval` at 1024 px, with `Unknown` read as its old fixed answer.

**The element path.** It is the player's, built as `css_overrides` does today (`ancestry`, `classes`), plus the attributes the player sets:
- each control: `id` and `data-name` = the control's name, case kept; `data-layer` on a layered control;
- the root: `section.pr-form-root` with `data-pr-form` and `data-pr-inst`;
- the extra elements the player mounts children into: a TabControl's page, a Splitter's pane. They are pinned by a player test (§6.2).

For a form's own properties, the path ends at `section.pr-form-root`, which is where the player writes the form's background (`form_decls`, style.rs:705).

**Selector matching.**
- `cobolt_forms::css` gains attribute selectors: `[a]`, `[a=v]`, `~=`, `|=`, `^=`, `$=`, `*=` and the `i` flag.
- `ElementRef` gains `attrs: &[(&str, &str)]`. The Viewer passes `&[]`, so its pages behave as before: a rule it used to drop now matches nothing.
- `:host` matches the root element of the path.
- A selector the subset cannot parse is counted per sheet and reported by Check (§1.6), never guessed.

**The props layer, as data.**

```rust
pub fn props_layer(ctl: &Control, form: &Form) -> Vec<(&'static str /*css*/, PropsDecl)>
// PropsDecl::Own  — the player declares it from the control's own property
// PropsDecl::Theme(String) — the Spatial theme's own value in that rule
```

It is the list of §0.2 written down in `cobolt-forms`. It says which CSS properties of `CSS_TO_PROPERTIES` the player's `control_decls` writes for this control, and from what. A player test holds it equal to the player's output over a corpus (§6.2), the same way `CSS_TO_PROPERTIES` is already held to `TABLE` (cobolt-web-player/src/css.rs:741).

**The cascade.** For each CSS property, every matching declaration gets a key:

`(importance-and-layer rank, specificity, source order)`

| Normal declarations | Rank |
|---|---|
| base | 0 |
| theme | 1 |
| props | 2 |
| project sheet | 3 |
| unlayered: form sheet, then `CssStyle` (specificity (1,1,0), order after the form sheet) | 4 |

| `!important` declarations | Rank |
|---|---|
| unlayered | 5 |
| project | 6 |
| props | 7 |
| theme | 8 |
| base | 9 |

`var()` resolves through the custom properties cascaded on each element of the path (they inherit), with `css::substitute_vars`. The highest key wins.

**The result.**

```rust
pub struct Effective { pub props: Vec<Entry>, pub unpainted: Vec<Unpainted>, pub styled: bool }
pub struct Entry {
    pub property: &'static str,
    pub value: Value,            // Known(PropValue) | Unknown(String /*the CSS text*/)
    pub source: Source,          // Property | Theme{label, line} | Rule{label, line, decl: DeclRef} | CssStyle{decl: DeclRef}
    pub certain: bool,           // false: under an undecided condition, or a pseudo-class
    pub shared: bool,            // the winning rule can match other elements (§1.4)
}
pub fn effective(form: &Form, id: Option<&str> /*None = the form*/, sheets: &SheetSet, env: &MediaEnv) -> Effective
```

Rules about what wins:
- A "may" declaration never becomes the effective value (R4). It is recorded so the mark and the badge can name it.
- When the props layer wins, the entry's value is the stored property.
- `unpainted` lists what the canvas cannot paint (R7): a declaration outside the reverse table, an `Unknown` value, a geometry or layout declaration, a "may" rule. Each has its label, line and text.

### 1.2 The reverse table: `css_effective::REVERSE` (R2, R3)

The table sits beside `CSS_TO_PROPERTIES`, one row per CSS property there. Each row has:
- a fidelity: `Exact`, `Approximate` or `NotReversible`;
- `read(value, &Ctx) -> Vec<(property, Value)>`, where `Ctx` has the control's type, effective font size and root font size;
- for writable rows, `write(property, &PropValue, &DeclRef) -> Edit`.

The helpers already in `cobolt_forms::css` do the parsing: `color` (every CSS Color 4 notation listed at css.rs:743), `length` (`px`, `em`, `rem`, `pt`, `%`, `calc()`) and `substitute_vars`. The table is decided from §0.2:

| CSS | Property | Fidelity | Notes |
|---|---|---|---|
| `background-color`, `color` | BackgroundColor, ForegroundColor | Exact | Written as `#RRGGBB`, or `#RRGGBBAA` when translucent |
| `background` | BackgroundColor, the gradient | Exact for one colour, or a two-stop `linear-gradient` at one of the 8 angles of `gradient_angle` | Anything else is `Unknown`; on the form a `url()` is the picture, `Unknown` |
| `background-image` | the gradient | Exact as above; `none` → BackgroundGradientEnabled false | |
| `font-size` | FontSize | Exact in `px`, `pt`, `em`, `rem` | `%` and `calc()` with `%` are `Unknown` |
| `font-weight` | Bold | Exact: `bold`, `bolder` or ≥ 600 → true; `normal` or ≤ 500 → false | Written as `700` / `400` |
| `font-style` | Italic | Exact | |
| `font-family` | FontName | Approximate: the first family, unquoted | Write-back replaces the first family and keeps the rest of the list |
| `font` | FontName, FontSize, Bold, Italic | Approximate (the shorthand is parsed) | Write-back adds the longhand after it (R10) |
| `text-decoration`, `text-decoration-line` | Underline, Strikethrough | Exact over the line tokens; `none` → both false | Write-back rewrites the line tokens and keeps style and colour tokens |
| `cursor` | Cursor | Exact over the inverse of `cursor()` (css.rs:190-202) | `default` / `auto` → `Default`; any other cursor is `Unknown`; the spellings are the Cursor combo's own values |
| `padding`, `padding-*` | Padding, Padding* | Exact; Approximate on Label, Button and TextBox | The player adds its insets there (§0.2); the typed value is written literally |
| `border-radius`, the four corners | CornerRadius, CornerRadius* | Exact for one `px` value; per corner only where the model has the corner properties (Snackbar) | Otherwise `Unknown` |
| `border`, `border-style`, `border-width`, `border-color` | BorderStyle, BorderWidth, BorderColor | Exact | `solid`→Single, `dashed`→Dashed, `dotted`→Dotted, `double`→Double, `none`→None, `outset`→Raised, `inset`→Sunken; `groove`, `ridge` and four different sides are `Unknown` |
| `box-shadow` | the Shadow* properties | Approximate: one shadow → Enabled, Direction (the nearest of 8 to (dx, dy)), Distance (the hypotenuse), Blur and Strength (blur / 2, negative when `inset`), Color, Opacity (alpha × 100); `none` → Enabled false | Several shadows are `Unknown`; written in the player's single-shadow form; R12 checks the round trip |
| `text-align` | TextAlignment | Exact for left, center, right on Label and TextBox; Approximate on Button (`MiddleCenter`-style names) | |
| `justify-content`, `align-items` | TextAlignment, VerticalAlignment | Approximate over the values the player writes; else NotReversible | Not written back |
| `white-space` | WordWrap | Exact on a Label: `pre-wrap` / `normal` → true; `pre` / `nowrap` → false | |
| `z-index`, `display: none` | ZOrder, Visible | Exact to read | **Never written back** (R16) |
| `transform`, `backdrop-filter` | RotateX/Y, OsBlur | NotReversible | |

Every value R3 names is `Unknown`, carrying its CSS text. `em` and `rem` convert against the control's effective font size and the player's root font size, pinned by a player test.

### 1.3 Painting (R6, R7, R8)

**Controls**
- The designer adds one stage to its `controls_for_render` chain (designer.rs:10852-10898): **effective clones**, the controls with each `Known` effective value written into their properties.
- The clones exist only for painting. They are never saved, and the selection, drag and the Properties panel's writes act on `self.form`.

**Colours used as written.** A colour that comes from CSS is not faded by `Transparency` in the page (§0.2), and the painter would fade it.
- `RenderInput` (render.rs:612) gains `css_final: Option<&CssFinal>`: per control id, which of face, gradient, border and shadow colours to paint as given.
- The render engine's face, border and shadow colour functions read it.
- Both paths take the same `RenderInput` (`render_form`, `render_faces`), so both get it, and the parity guard covers it (§6.3).

**The form**
- The background code (designer.rs:10279-10598) reads its colour, gradient and radius fields from an **effective form view** (a `FormLook` with those fields), built from `effective(form, None, …)`.
- Spatial still replaces them with its glass (2.0.298). A developer rule on `.pr-form-root` beats the glass in the page too, because it is in a higher layer than `spatial_decls`: the effective value says which wins.

**The badge** (designer.rs:11087-11095)
- Drawn when `unpainted` is not empty.
- Its tooltip, today the fixed text "Styled by CSS the designer does not paint", becomes a header plus up to eight `file:line  declaration` lines and "and N more".

**Refresh (R8)**
- An IDE write updates `d.properties.css_sheets` in memory at once and resets `css_sheets_checked`, as Apply does (app.rs:15516).
- Another program's save is seen by the existing 1 s re-read (app.rs:15461).

### 1.4 Showing it in the Properties panel (R5, R15)

**The effective clone**
- For a Web or Mobile form, `show_control` is given the effective clone of the selected control, as the breakpoint view gives `overridden_control`.
- So every row shows the effective value without a change of its own.
- For the form, `show_form` gets the form with its effective look fields.

**The marks**
- `CSS_MARKS` (properties.rs:14357) carries, per key, the source text and the stored value.
- Its tooltip reads either "Overridden by CSS (file:line). Property value, not in effect: X", or "From the theme (name or file:line)".
- "May be overridden" keeps its text.

**Unknown values**
- A new thread-local, `CSS_UNKNOWN`, holds the key and the CSS text.
- `property_row_keyed` (properties.rs:14134) draws that text read-only in place of `value(ui)`.
- Rows that bypass `property_row_keyed` are listed at task time and given the same check.

**Multiple selection**
- The effective appearance of each selected control is computed.
- A key whose values differ goes into `CSS_MIXED`, and the label cell shows a "≠" mark whose tooltip lists the values.
- The editor shows the primary control's value, as today.

**"Shared"**
- The winning rule's selector can match an element other than this control. Its subject names a type class, a `CssClass`, an attribute other than this control's name, or no id, or its value comes through a `var()` that other declarations read.
- The question (R11) counts the controls it actually matches:
  - on this form, for a form-sheet rule;
  - on every form of the project, for a project-sheet rule (each form file loaded once when the question is asked).

### 1.5 Writing it (R9–R16, R18)

**The planner** is pure and lives in `cobolt-forms`: `css_edit::plan(form, id, property, new: &PropValue, sources, env, choice) -> Result<Plan, Refusal>`.
1. Find the winning entry. Not certain, or from the theme: `Plan::StoreProperty`, as today (R9).
2. `NotReversible`, Visible or ZOrder: `Refusal::NotWritable` (R16). The panel offers Go to rule and Remove the CSS declaration.
3. Build the edit:
   - **Rewrite** the value's byte range in the declaration's own notation where it is exact, else the table's (R10). `!important` is kept. Hex stays hex, `rgb()` stays `rgb()`, `hsl()` stays `hsl()`, and a name is kept only if one names the exact colour.
   - **Shorthand:** replace the owned component when the shorthand parses into components (`border`, `padding` with one to four values, `background` with one colour, `text-decoration`, `font-family`'s first family). Otherwise **insert the longhand** right after the shorthand's `;`, in the same rule, with the shorthand's indentation.
   - **`CssStyle`:** the same rewrite on the property's text, emitted as a `SetProperty("CssStyle")`.
   - **Only this control** (`choice`, R11): add the declaration to the last unconditional, unlayered rule whose selector is exactly `#NAME` (CSS-escaped: a leading digit becomes `\3N `) in the form's sheet, or append `\n#NAME {\n  prop: value;\n}\n` at the end of the file. It carries `!important` only when the shared declaration has it. A form with no sheet file gets the generated one (`web_themes::form_stylesheet_for`) with the rule after its `/* Your rules. */` line.
4. **Check before anything is written (R12).** Compute `effective` again over the new texts, in memory. When the property's value is not the one typed, return `Refusal::NoEffect { winner }`. Nothing touches the disk, so the files stay byte-identical, which is stronger than writing and undoing.
5. **Lint (R18).** `stylesheet_lint::lint` of each new text may not have an Error-severity finding that the old text did not have.
6. **Remove the CSS declaration** (R14) is `css_edit::remove(decl)`: it removes the declaration's byte range and its line when the declaration had the line to itself. A rule left empty stays.

**The executor** is in the IDE.
- **Routing:** both consumers of `set_props` route through `css_route`:
  - the designer's, at app.rs:20883;
  - the Main-Pane inspector's, at app.rs:8223.

  `css_route` keeps a `set_props` whose value equals the effective value in effect from doing anything; that is the `lost_focus` filter of §0.4. Otherwise it calls the planner.
- **Before writing (R13):**
  - It refuses, with the reason, while `stylesheet_modal` holds unapplied text for that sheet (`changed()`, stylesheet_modal.rs:240).
  - It reads each sheet from disk, not the 1 s cache. When the text differs from what the marks were computed on, it plans again.
- **Undo:** one `Cmd::Batch` holding new `Cmd::SetStylesheet { path, before, after }` steps and any `SetProperty("CssStyle")`.
  - `SetStylesheet` follows `SetMenuDefinition`.
  - Its `execute` and redo write `after` only if the file still holds `before`. Its `reverse` writes `before` only if the file still holds `after`.
  - Otherwise it stops with "the stylesheet changed since; undo would discard that change", so nothing another program wrote is lost.
  - After a write: the in-memory sheets are updated, and the open Preview gets `web_preview.apply_form_stylesheet` (app.rs:15511), as Apply does.
- **The shared-rule question (R11):**
  - a new window, `panels/css_rule_question.rs`, copied from `indexed_engine_modal.rs`: its own `size`, `.resizable(false).fixed_size(size)`, one grip in a foreground `Area`, with **Change the rule**, **Only this control** and **Cancel**;
  - the pending edit is held as `style_break_pending` holds the form-background edit (app.rs:20918);
  - with several controls selected, one question covers the shared rules involved (R15).
- **Gestures:** a colour picker or a drag sends a `set_props` every frame.
  - The first change of a gesture asks the question, when one is needed.
  - Until the gesture ends, the planned text replaces the sheet in memory only, so the canvas follows.
  - The file is written, and one undo step recorded, when the gesture ends (the picker closes, or the field loses focus).
- **Row actions:**
  - The "CSS" mark in the row's label cell gets a context menu with **Go to rule** and **Remove the CSS declaration**.
  - Go to rule opens the stylesheet modal with a new `pub fn open_at(line)` (setting the existing `jump_to`), or the Style editor.
  - Remove asks first, then runs as an edit through the same `SetStylesheet` undo.

### 1.6 Check, `render_form` and the agent (R17)

**`render_form`**
- `snapshot::render_form_png` (snapshot.rs:203) builds the effective clones and `CssFinal` for a Web or Mobile project's form from the project and form sheets.
- The IDE's renderer and the CLI's both call it, so the coding agent's eye sees what the canvas shows.

**`check`**
- For a Web or Mobile form, `tools/check.rs::check_form` adds one **note** per overridden property: `BTN-1.BackgroundColor has no effect: mobileforms/main-form.css:2 (background-color: …) wins`.
- It adds one note per selector the subset cannot evaluate.
- Notes are not errors. The severity is the informational one Check already has; checked at task time.

**The agent**
- `agent::build_context_for` lists, for each control with overrides, the property, the effective value and the source.
- Applying an `AgentOp::SetProperty` on an overridden property still stores it (Q5). The apply's report to the agent says it has no effect and which declaration wins.
- The agent changes the stylesheet as a file, which the developer sees as a diff.

### 1.7 The System KB

- The compiler's documentation text on WASM-007 (the CSS paragraph and the Properties panel's behaviour on Web and Mobile forms) says what the designer and the panel now show and write.
- `assets/knowledge/chunked.data` is regenerated.

### 1.8 Performance (R20)

- **Two caches:**
  - the `SheetSet`, keyed by a hash of the source texts and the theme id;
  - each control's `Effective`, keyed by the control's id, type, parent, layer, CssClass and CssStyle, the stored values of the mapped properties, the `MediaEnv` and the `SheetSet` key.
- Selectors are parsed once, and matching uses the parsed form.
- **Budget for 200 controls and 500 rules:** a whole form recomputed in **≤ 100 ms in the test (debug) build**, printed, and measured once with `--release`, recorded expecting ≤ 16 ms. A frame with nothing changed computes nothing.

## 2. Affected crates / files

**`cobolt-forms`**
- `src/css_effective.rs` (new): `SheetSet`, `MediaEnv`, the path, the cascade of §1.1, `props_layer`, `REVERSE`, `effective`, `CssFinal`.
- `src/css_edit.rs` (new): `plan`, `remove`, the notation-preserving value writers, the CSS ident escape.
- `src/css.rs`:
  - attribute selectors, and `ElementRef.attrs`;
  - `:host`;
  - `media_eval` and `MediaEnv`, with `media_matches` kept;
  - `pub(crate)` access to the selector parser and type;
  - declaration byte ranges.
- `src/css_overrides.rs`: a thin view over `css_effective`. Its three tests stay green.
- `src/viewer.rs`: `ElementRef { attrs: &[] }` where it builds them. Its behaviour is unchanged.
- `src/render.rs`: `RenderInput.css_final`, read by the face, border and shadow colour paths of both entry points.
- `src/snapshot.rs`: effective clones for a Web or Mobile project (§1.6).

**`cobolt-web-player`** (natively built tests)
- the `props_layer` parity test;
- the element-path test (TabControl page, Splitter pane, layers);
- the root-font test;
- the AC2 differential test in the browser runner of WASM-015 (Chromium).

**`cobolt-ide`**
- `src/panels/properties.rs`: the effective clone, the marks' new texts, `CSS_UNKNOWN`, `CSS_MIXED`, the mark's context menu, and the new `InspectorAction` fields `css_go_to` and `css_remove`.
- `src/panels/designer.rs`: the effective stage in `controls_for_render`, the effective form look for the background, `css_final` in `RenderInput`, the badge's tooltip, `Cmd::SetStylesheet` (execute and reverse), and the `SheetSet` and `Effective` caches.
- `src/panels/stylesheet_modal.rs`: `pub fn open_at(line)`.
- `src/panels/css_rule_question.rs` (new): the question window.
- `src/app.rs`: `css_route` in both `set_props` consumers, the pending edit and the question, gesture end, the row actions, the in-memory sheet update and the Preview update after a write, and the `MediaEnv` from `canvas_size()`.
- `src/agent.rs`: the effective values in `build_context_for`, and the "no effect" note on applying `SetProperty`.
- `src/i18n.rs`: the new `Tr` keys (§3), six languages each.

**Other crates**
- `cobolt-project-tools`, `src/tools/check.rs`: the notes of §1.6.
- `cobolt-compiler`, `src/lib.rs`: the KB text (§1.7). Then `assets/knowledge/chunked.data` is regenerated.

**`specs/`**
- `WASM-036/spec.md`: the corrections of §0.6.
- `tasks.md` at `/tasks`.
- `WASM-PROGRESS.md`: this spec's row.

## 3. Data / model changes

- **No format change.**
  - The `.wcfrm`/`.mcfrm` files and the project manifest are untouched.
  - Stylesheets stay the developer's text files: only the declarations an edit names change.
  - No property is added.
- **`cobolt_forms::css`**
  - `ElementRef` gains `attrs`. It is `pub` with two other construction sites, both in `cobolt-forms`, so this is a source change inside the crate only.
- **`RenderInput`**
  - gains `css_final: Option<&CssFinal>`, `None` everywhere but the designer and `snapshot`.
- **New `Tr` keys**, each in the six languages:

  | Group | Keys |
  |---|---|
  | Marks | `css_from_theme`, `css_not_in_effect`, `css_unknown_value`, `css_mixed_values` |
  | Badge | `css_badge_header`, `css_badge_more` |
  | Row actions | `css_go_to_rule`, `css_remove_declaration`, `css_remove_confirm` |
  | Question window | `css_shared_title`, `css_shared_body`, `css_shared_change_rule`, `css_shared_only_this` |
  | Messages | `css_write_refused_unapplied`, `css_write_no_effect`, `css_write_not_writable`, `css_undo_changed_on_disk` |

  Cancel reuses the existing key. The badge's `idef` literal is replaced by `css_badge_header`.

## 4. Key decisions & alternatives

| Decision | Why | Rejected |
|---|---|---|
| One engine in `cobolt-forms`, with no egui | Check, `snapshot` and the agent need the same answer as the panel; a pure crate can be unit-tested and measured | Computing it in `properties.rs`, which would leave Check, `render_form` and the agent on a second answer |
| Model the props layer as data in `cobolt-forms`, pinned to the player by a test | The player crate is built for `wasm32` with its own manifest, and the IDE does not depend on it; the pin is the pattern `CSS_TO_PROPERTIES` already uses | Calling the player's `appearance()` from the IDE (a dependency on the wasm crate); assuming one declaration per stored property, which §0.2 shows is false |
| Check the effect in memory before writing (R12) | No file is touched when an edit would not take effect, so the files stay byte-identical by construction | Writing, re-reading and undoing: a window where the file is wrong, and an undo that can race another program |
| `Cmd::SetStylesheet` with compare-before-write on execute and on reverse | Undo never overwrites a change another program made after the IDE's write | Holding the stylesheet in the designer's memory and saving it with the form, which would make a sheet edit invisible to other tools until the form is saved |
| Colours from CSS reach the painter through `RenderInput.css_final` | The page uses a CSS colour as written; the painter fades by `Transparency`. An explicit input keeps both paths exact | Pre-compensating the alpha in the clone (impossible at `Transparency` 100); a hidden marker property in the clone, which could leak into a saved file |
| "May" stays a mark, never a value | A state or screen the designer cannot decide must not change what it shows (R4) | Picking the desktop answer, as the Viewer does |
| Attribute selectors in `cobolt_forms::css`, with the Viewer passing no attributes | The player's own selectors and `data-name` rules are attribute selectors; the Viewer's behaviour is unchanged | A private matcher for the IDE, a second CSS engine to keep equal |

## 5. Risks & mitigations

- **The IDE's cascade and the browser's disagree.**
  → The differential test (§6.2) runs the AC2 fixture and a corpus in Chromium through the player, comparing values and sources. `props_layer` and the element path are pinned by unit tests in the player. A difference is a failing test, never an accepted gap.
- **Selectors the subset cannot parse**, such as `:has()`, `:is()` and `:where()` with complex lists.
  → They are counted, and Check reports each with its line (§1.6). The panel's mark does not claim to know. Parsing more selectors is a later change to `cobolt_forms::css`, measured by the same corpus.
- **The painter and the browser draw the same values differently** (WASM-004 R11: the glass, shadows as rings, fake bold).
  → R6 promises the values, not the pixels. AC4 compares the canvas with `render_form`, the same engine; AC16 compares the canvas with the emulator by eye, as the operator's report did.
- **The question for a project-sheet rule loads every form of the project.**
  → Done only when the question is asked, once per question, from files on disk. For a project with very many forms, the count is said as "at least N" after a time limit.
- **Rows that bypass `property_row_keyed`** would show an editable stored value where they should show an unknown value read-only.
  → They are listed at task time (`grep` of rows not built on it), and each is given the check.
- **Per-frame writes from pickers.**
  → The gesture rule (§1.5): in memory until the gesture ends, then one write and one undo step. A test drives 30 frames of picker changes and asserts one file write and one undo step.
- **`lost_focus` pushes.**
  → `css_route` drops a value equal to the one in effect (§1.5). A test focuses and unfocuses every text row of an overridden control and asserts no write.
- **The theme's CSS is large**, and every control's path is matched against it.
  → The `SheetSet` cache and parsed selectors. The theme is parsed once per theme id, and the budget of §1.8 includes it.
- **Found while planning, not in this scope:** `App::open_stylesheet_modal` returns at once when the form has no stylesheet file (app.rs:15489, `project_fs::form_stylesheet` is `Some` only for an existing file). Its doc comment says it opens the generated text. That is a fix of its own (`fixes-2x`), reported to the operator, not made here.

## 6. Test strategy

Every test prints one result block: the cases it ran, the counts, and for the timed ones the milliseconds and rules × controls per second (GOLDEN RULE #7's form).

### 6.1 `cobolt-forms`

**`css_effective` tests**
- **The cascade of §0.1, one case per row:**
  - each layer against the next;
  - `!important` reversing the layers;
  - `CssStyle` beaten by a two-id form-sheet rule;
  - source order;
  - specificity.
- **The rest of the engine:**
  - `@media` width, height and orientation decided at a 412 × 915 `MediaEnv`; `hover` and `prefers-color-scheme` "may";
  - `var()` through the path and through `:host` tokens;
  - `em` and `rem`;
  - each `Unknown` kind of R3;
  - the form's own path, and the TabControl and Splitter paths;
  - attribute selectors (`[data-name="BTN-1"]`, `^=`, `i`);
  - the Viewer's existing tests unchanged.
- **AC1:** an enumeration of `CSS_TO_PROPERTIES` against `REVERSE`, plus a round trip in every notation for each `Exact` row.

**`css_edit` tests**
- **AC6, byte diffs:** a longhand, a rewritable shorthand, a shorthand needing a longhand, `!important`, `CssStyle`, each notation, an id that starts with a digit, and a sheet with comments, CRLF line endings and odd spacing. Each asserts that only the named bytes change.
- **AC8:** a later `!important` gives `NoEffect`, naming the file and line, with the texts unchanged.
- **Lint:** a write that would add an Error-severity finding is refused.
- **Remove:** the declaration, its line, and the empty rule kept.

**Performance (AC15)**
- 200 controls and 500 rules: the whole recompute and a cache hit, timed and printed against the budget.

**`css_overrides`'s three existing tests** keep passing, through the new engine.

### 6.2 `cobolt-web-player`

**Unit tests, run natively:**
- `props_layer` equals the CSS property names and Spatial values in `control_decls`'s output over a corpus: every `ControlType`, seeded, absent and edge values (`#F0F0F0`, radius 0, no `BorderStyle`, Bold, Spatial on and off).
- The element path the IDE builds equals the one `dom/instance.rs` mounts, for the corpus's containers.
- The root font size equals the one `css_effective` uses for `rem`.

**AC2, the differential test**, in the browser runner of WASM-015:
- The fixture form is mounted at 412 × 915 in Chromium.
- For each case, `getComputedStyle` of the element is compared with `effective`'s value.
- It runs where Chromium is present and is reported as not run elsewhere.

### 6.3 `cobolt-ide`

These tests drive real frames, as `designer_page_backdrop_tests.rs` does.
- **AC3 (Properties):**
  - the effective colour in the BackgroundColor row;
  - the mark's text with the file and line in the six languages;
  - the stored value in the tooltip;
  - a `calc()` row drawn read-only.
- **AC4 (canvas):**
  - the canvas fill of the styled button is the CSS colour;
  - a CSS colour on a `Transparency` 50 control is unfaded;
  - `render_form` of the same project gives the same pixel within 1 per channel;
  - a `width: 50%` control keeps its rectangle, and the badge's tooltip lists the line.
- **AC5:** a sheet rewritten on disk; the fill changes within 1 s of simulated time, and the time is measured.
- **AC6–AC11, through `css_route` with a temporary project:**
  - longhand and shorthand writes;
  - the question's three answers, with byte-identical files on Cancel;
  - `NoEffect`, with no write;
  - Ctrl+Z and Ctrl+Y byte for byte;
  - the unapplied-modal refusal;
  - the on-disk change kept;
  - undo stopping after an outside change;
  - Remove with confirmation and undo;
  - Go to rule's line;
  - the multi-selection mixed mark, with one question;
  - the 30-frame gesture (one write, one undo step);
  - the `lost_focus` test (no write).
- **AC13:** the question window's rectangle held over 120 frames in each language and after a grip drag; every new `Tr` key non-empty and translated (the existing guards `no_empty_ui_translations` and `non_english_is_actually_translated`).
- **AC14:** a stylesheet beside a Desktop form gives no mark and the stored paint. The existing designer and properties tests stay green.
- **The parity guards:**
  - `cargo test -p cobolt-forms --features render` green;
  - `engine_reference_form_parity_static_vs_faces` green, with a `css_final` case added;
  - the System KB guard green on the regenerated `chunked.data`;
  - the whole `cobolt-ide --bin` sweep with `--no-fail-fast`: only the three known reds of the 2.x line.

### 6.4 `cobolt-project-tools`

- **AC12:** `check` on the AC2 fixture lists the notes, and `render_form` gives the effective pixel.
- The agent's context lists the overrides.
- Applying a `SetProperty` on an overridden property reports which declaration wins.

### 6.5 Manual (AC16)

- In a copy of `examples/PowerMobileHello` (never the example itself), a form sheet colours the buttons and changes the label's font.
- Compare the designer's canvas (the operator's eye, or `render_form`) with the emulator's screen, which the IDE's Run puts there.
- Keep both screenshots as evidence.
- The IDE window itself is not driven.

## 7. Steering compliance

- [ ] **i18n:** the 17 new `Tr` keys of §3, in the six languages. The badge's literal is replaced.
- [x] **Generated-code banner and regenerate-on-action contract:** untouched. No COBOL is generated or changed.
- [x] **English developer's guide:** not touched on 2.x.x; the chapter is written when 2.x.x merges into main. What 2.x.x says is in `specs/` and the System KB.
- [ ] **System KB:** the WASM-007 text updated and `chunked.data` regenerated (§1.7).
- [x] **Fix vs feature:** a feature, on `features-2x`. Every task bumps `z` and adds a top CHANGELOG entry. The found `open_stylesheet_modal` defect is a separate fix.
- [ ] **No "cobolt" in user-facing text; COBOL identifiers in English:** the new texts name the product's UI only. Control names in messages are the developer's own, verbatim.
- [ ] **User code is sacred:** only the declarations an edit names change. Nothing is removed without a confirmation and an undo. An empty rule stays. Compare-before-write is used on every write and undo.
- [ ] **A window never resizes itself:** the question window follows `indexed_engine_modal.rs` (own size, `fixed_size`, one grip), and AC13 measures it.
- [ ] **Render through `cobolt-forms` on both paths:** `css_final` is read by both, and the parity guard has a case.
- [x] **Interpreter and binary parity:** nothing at run time changes.
- [x] **Rust only:** the throwaway scripts of the AC2 runner, if any, stay outside the repository.
