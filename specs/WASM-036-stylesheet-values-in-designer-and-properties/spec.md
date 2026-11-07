<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The stylesheet's values in the Form Designer and the Properties panel, and edits written back to the rule

- **Status:** draft. The operator asked for it on 2026-11-07. Q1–Q6 carry a recommendation, and the requirements are written to it until the operator rules.
- **Folder:** specs/WASM-036-stylesheet-values-in-designer-and-properties/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-11-07
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the operator, 2026-11-07: *"for a mobile project, make sure the form/control properties reflect the CSS defined for it, and vice-versa. the rendered app in the emulator/device is completely different from the one in the designer"*.
  - The part of that report that was a defect shipped as fix **2.0.298**: a Line turned by `LineAngle`, the Spatial glass and the page's stand-in desktop on the canvas.
  - This spec is the rest: the values a stylesheet gives, shown where the developer designs, and edits that take effect.
- **Amends:** WASM-007 R12 ("shall not pretend to show the result"), for the properties the table maps back (R2 below). What the designer still cannot paint keeps R12's mark.
- **Depends on:** WASM-007 (the table R1, the cascade R8, the marks R12, the stylesheets of Q2/Q3, the layering guard R16) and WASM-004 (fidelity R6, parity R11).
- **Related:** WASM-035 (proposed, *Designer evolution*). The "CSS-aware designer" of that row is this spec. A web-view designer, device frames and component-state preview stay there.

## 1. Overview

On a Web or Mobile form **CSS wins** (WASM-007 R8). A property that a developer's rule overrides has no effect on the page. Today the Form Designer still paints that property, and the Properties panel still shows and edits it, with a mark saying it is overridden. So the designer shows one form and the device another, and an edit in the panel changes nothing the developer can see on the device.

This spec makes the designer and the panel show the **effective value**: what the cascade gives, and where it comes from. It also makes an edit in the panel **take effect**. The winning declaration is rewritten in its file. When that rule styles other controls too, the developer chooses. A write that would still have no effect is undone and explained, never left silently.

### What exists today (checked 2026-11-07, `2.x.x` at `510553c4`, 2.0.298)

**The cascade in the player.** Lowest to highest:
1. The generated theme block, in `@layer pr-theme` / `pr-base`.
2. `pr-props`: the declarations of the properties the control *stores* (`cobolt-web-player/src/css.rs::appearance` writes one only for a property that is present).
3. The developer's rules: the project stylesheet (`[web]` / `[mobile] stylesheet`), then the form's stylesheet beside the form (`web_themes::stylesheet_beside`).
4. The control's `CssStyle`.

**`cobolt_forms::css_overrides::overrides(form, id, sheets)`** has no egui. For one control it lists each property a developer rule or the `CssStyle` overrides, with the file, the line, and *certain* or *may*. `CSS_TO_PROPERTIES` maps 35 CSS properties back to model properties. What it does not do:
- It reports *which* properties and *where*, never the **value**.
- It covers controls only, not the form.
- It skips the theme layers.

**The Properties panel** (`panels/properties.rs`, `css_marks_for`) marks the row. Its tooltip reads "Overridden by CSS (file:line)", "May be overridden…" or "Overridden by this control's Style". The marks show for a single selection only. The row still shows and edits the stored value, which has no effect.

**The designer** draws the `css_styled` badge on a control with a `CssStyle` or a matching developer rule. The canvas paints the stored properties.

**The sheets** are read by `App::refresh_css_sheets` at most once a second: the project sheet and the form sheet.

**`@media`:** `css_overrides` marks every rule under `@media` as "may". The `cobolt_forms::css` cascade it reuses decides `@media` at a fixed `MEDIA_WIDTH_PX` of 1024 px, which is wrong for a phone form 412 px wide.

**The stylesheet modal** (`panels/stylesheet_modal.rs`) has Apply, Cancel, Revert to theme default and an undo of Apply. It cannot open at a line.

**The example:** `examples/PowerMobileHello/mobileforms/main-form.css` holds only the generated theme block, so nothing overrides a property there. This spec matters as soon as a developer writes a rule.

## 2. Goals / Non-goals

**Goals:**
- One computation of a control's and the form's **effective appearance**, in `cobolt-forms` with no egui. For each property CSS can set, it gives the value in effect and its source:
  - the property itself;
  - the theme block;
  - a rule, at file and line;
  - the control's `CssStyle`.
- The canvas paints the effective values, and the Properties panel shows them. The badge stays only for what the designer cannot paint, and it says what.
- An edit in the panel takes effect:
  - it is written into the winning declaration, with every other byte of the file kept;
  - a rule shared with other controls asks first;
  - every write is undoable;
  - a write that would have no effect is refused.
- Check and the agent tools see the same values.

**Non-goals:**
- Laying a form out by CSS in the designer: `width`, `height`, `top`, `left`, `margin`, `position`, flex and grid. The canvas keeps the form's geometry, and the badge lists such declarations. A web-view designer is WASM-035.
- Painting pseudo-class states (`:hover`, `:focus-visible`, `:checked`) or pseudo-elements (`::before`, `::after`). They stay "may". Component-state preview is WASM-035.
- Editing the generated theme block. It is regenerated by Revert to theme default, and the properties beat it anyway.
- Writing CSS for a property no rule overrides. The properties stay the store, and the cascade of WASM-007 R8 is unchanged (Q3).
- A visual CSS designer or a CSS formatter (WASM-007's non-goals).
- Any change to a Desktop form (WASM-007 R14).

## 3. User stories

- As a developer of a Mobile application who writes `.primary { background-color: #0A84FF }`, I want the designer to paint those buttons blue, so that the designer looks like the phone.
- As a developer who changes BackgroundColor in the Properties panel on a button that a rule colours, I want the button to change on the phone, so that my edit is not silently ignored.
- As a developer whose edit would change a rule that also styles other controls, I want to choose between changing all of them and changing only this one, so that one edit does not restyle my form behind my back.
- As a developer, I want to go from an overridden property to its rule, so that I can edit what the panel cannot express.
- As the Form Designer agent, I want the tools to give me the effective value and its source, so that I do not set a property that has no effect.

## 4. Requirements (EARS)

**The effective appearance: one computation**
- **R1 (ubiquitous):** For a control, or the form, of a Web or Mobile project, the IDE shall compute the **effective appearance**.
  - **Coverage:** every property that CSS can set (the reverse table, R2).
  - **What each entry gives:** the value in effect and its source, one of:
    - the stored property;
    - the theme block (`@layer pr-theme` / `pr-base`), for a property the control does not store;
    - a developer rule, at sheet file and line;
    - the control's `CssStyle`.
  - **How:** it shall follow the cascade of WASM-007 R8 as the player builds it: the layers, specificity, order, `!important`, and the element path of `dom/instance.rs`. For the form, that path is `section.pr-form-root` → `div.pr-form` → `div.pr-surface`, with each property on the element the player writes it to.
  - **Where:** one library with no egui, called by the Properties panel, the canvas, Check and the agent tools.
- **R2 (ubiquitous):** Each CSS property of `CSS_TO_PROPERTIES` shall have a **reverse rule** in one table beside it, marked with one of three fidelities. A test shall enumerate `CSS_TO_PROPERTIES` and fail on a CSS property that has no reverse rule.
  - ***exact*:** the value converts to the property without loss, and back to CSS that converts to the same value. This covers:
    - colours, in every CSS Color 4 notation the target browsers and web views take;
    - lengths in `px`;
    - `font-size`, `font-weight`, `font-style`;
    - `text-decoration`, `text-align`, `cursor`;
    - the `border-*` and `border-radius` properties;
    - the `padding*` properties;
    - `display: none`;
    - `z-index`.
  - ***approximate*:** the property is the closest the model has: one `box-shadow` to the Shadow* properties, `font-family` to its first family, the `font` shorthand.
  - ***not reversible*:** for example `transform`, `backdrop-filter`, and `justify-content` / `align-items` outside the values the player writes.
- **R3 (ubiquitous):** Some values cannot be expressed as the property's value. For these, the property's effective value shall be **unknown**: shown as the CSS text, not painted, and listed by the badge (R7). They are:
  - a relative unit whose base the IDE cannot know: `%`, `vw`, `vh`, `calc()`, `env()`;
  - a `var()` that cannot be resolved;
  - an image: `url()`, or a gradient other than the two-stop linear gradient the model has;
  - more than one shadow;
  - `inherit`, `unset`, `revert`.

  Two things shall still be resolved:
  - `em` and `rem` convert against the control's effective font size and the player's root font size;
  - `var()` resolves through the custom properties defined on the element path in the sheets, and the theme's published custom properties.
- **R4 (state):** While a rule applies only under a condition (`@media`, `@container`, `@supports`), the IDE shall decide the condition where the designer knows the answer, and count the rule as *applying* or *not applying*:
  - `width`, `height`, `orientation` and `aspect-ratio` are decided against the form's design size. For a Mobile form that is its phone size, not the 1024 px of `MEDIA_WIDTH_PX`.
  - `prefers-color-scheme` is decided against the theme mode the designer shows.

  The rest shall stay "may" (WASM-007 R12) and shall not change the effective value: a condition the designer cannot decide (`hover`, `pointer`, `@supports`, a container query), and a rule whose subject has a pseudo-class.

**Showing it**
- **R5 (state):** While a property's effective value comes from CSS, its row in the Properties panel shall show **the effective value** in its editor: the colour swatch, the number, the check box.
  - The row's mark names the source: file and line, or Style.
  - The mark's tooltip gives the stored value as "not in effect".
  - An unknown value (R3) shows the CSS text, read-only, with the mark.
  - A value that comes from the theme block shows with a "from the theme" mark.
- **R6 (state):** The Form Designer's canvas shall paint each control, and the form, with their effective values.
  - It shall go through the one render engine, on both of its paths (`render_form` and `render_faces`), so that the canvas, the designer's thumbnails and the `render_form` tool show the same thing.
  - This amends WASM-007 R12: the designer shows the result of the CSS it can paint.
  - Showing never changes a stored value: R1–R8 write nothing to the form file or to a stylesheet.
- **R7 (state):** While a control, or the form, has CSS the designer does not paint, the canvas shall keep WASM-007 R12's badge. What it does not paint:
  - a declaration outside the reverse table;
  - an unknown value (R3);
  - a geometry or layout declaration;
  - a "may" rule.

  The badge's tooltip shall list those declarations with their file and line: the first eight, then a count of the rest.
- **R8 (event):** When a stylesheet changes, the canvas and the Properties panel shall show the new effective values within 1 s, without reopening the form. A change is any of:
  - Apply in the stylesheet modal;
  - an undo;
  - a write-back (R9);
  - a save by another program.

**Editing it**
- **R9 (event):** When the developer edits a property whose effective value comes from CSS *with certainty*, the IDE shall make the new value **take effect**: it shall rewrite the winning declaration, not the stored property. Two cases are stored as today and are not written to CSS:
  - a property that is only "may be overridden";
  - a value that comes from the theme block.
- **R10 (ubiquitous):** A rewrite shall change only the value text of the winning declaration in its sheet.
  - The file's other bytes, comments, order and formatting are kept, and so is `!important`.
  - For `CssStyle`, only that declaration in the property's text changes.
  - **A shorthand** (`background`, `border`, `font`, `padding`, `box-shadow`): when it can be rewritten exactly, the rewrite replaces the component the property owns; otherwise it adds the longhand right after the shorthand, in the same rule.
  - **The notation:** the value is written in the declaration's own notation where that notation can carry it exactly. Otherwise it uses the table's notation: `#RRGGBB` for an opaque colour, `#RRGGBBAA` for a translucent one, lengths in `px`.
- **R11 (event):** When the winning rule also styles other controls, the IDE shall ask before writing. That happens with a type or class selector, or with a `var()` whose custom property other controls use.
  - **The question** names the rule (file, line, selector) and how many other controls it styles.
  - **Change the rule:** all of those controls change.
  - **Only this control:** a declaration that wins for this control alone. It goes in a rule for the control's id in the form's stylesheet, created at the end of the file outside any layer, with `!important` only when the shared declaration has it.
  - **Cancel:** nothing is written.
  - **The window** shall own its size and be resized only by its grip (WASM-007 R7, the project's self-resizing rule).
- **R12 (constraint):** After every write, the IDE shall compute the effective appearance again (R1) and check that the property's effective value is the one typed.
  - When it is not (a later rule, a higher specificity, an `!important` elsewhere), the IDE shall undo the write and tell the developer which declaration still wins, with its file and line.
  - An edit that has no effect shall never be reported as done.
- **R13 (ubiquitous):** Every write shall be one step of the designer's undo and redo, together with any change the same edit makes to the form file. That covers a rewrite, an added declaration and an added rule.
  - **Refused:** a write is refused, with its reason, while the stylesheet modal holds unapplied text for the same sheet.
  - **Changed on disk:** when the sheet changed on disk since it was read, the IDE shall read it again and find the target declaration again before writing. Nothing another program wrote is lost.
- **R14 (event):** On a row whose value comes from CSS, the IDE shall offer two actions:
  - **Go to rule** opens the stylesheet modal at that line, or the Style editor for `CssStyle`.
  - **Remove the CSS declaration** asks for confirmation and is undoable. It removes that one declaration and nothing else. A rule it leaves empty stays where it is, because it is the developer's code.
- **R15 (state):** While several controls are selected:
  - a property whose effective values differ between them shall show as mixed;
  - an edit shall apply to each control by R9–R13, with one question (R11) that covers every shared rule involved.
- **R16 (constraint):** Some properties shall not be written back. Their rows offer only Go to rule and Remove the CSS declaration (R14). They are:
  - a property whose reverse rule is *not reversible*;
  - `Visible` (`display`) and `ZOrder` (`z-index`), whose CSS also decides the player's layout and the layering rules of WASM-007 R16.

**Agents and Check**
- **R17 (ubiquitous):** The agent tools shall see what the developer sees:
  - The tools that read or render a form shall give the effective value and its source for an overridden property: `render_form`, the Form Designer agent's property reads, and `check`'s report.
  - The Form Designer agent's tool that sets a property shall answer, when CSS overrides it, that the value has no effect and which declaration wins.
  - The tools write no stylesheet (Q5).

**Safety and scope**
- **R18 (constraint):** A write-back shall pass WASM-007's stylesheet checks before it is written: the safety rules of R13 and the layering guard of R16.
  - It may write only to the project's stylesheet, the form's stylesheet and the control's `CssStyle`.
  - It shall never write into the theme block, or to a file outside the project.
- **R19 (constraint):** A Desktop project shall be unchanged. It has no stylesheet, so it has no effective values from CSS, no new marks and no write-back.
- **R20 (ubiquitous):** The effective appearance of a form shall be computed again only when one of these changes:
  - the form;
  - one of its sheets;
  - the designer's form size;
  - the designer's theme mode.

  Its cost for a reference form of 200 controls with a sheet of 500 rules shall be measured against the budget the plan sets, and reported.

## 5. Acceptance criteria

- [ ] **AC1** (R2) — A test enumerates `CSS_TO_PROPERTIES` and finds a reverse rule with a fidelity for each CSS property. For each *exact* rule, values in every notation round-trip: a colour as hex, `rgb()`, `hsl()` and a name; lengths in `px`, `em` and `rem`. Each converts to the property, back to CSS, and again to the same property value.
- [ ] **AC2** (R1, R4) — A fixture sets the same property at every level:
  - the theme block;
  - the stored property;
  - the project sheet;
  - the form sheet;
  - a class rule;
  - an id rule;
  - `CssStyle`;
  - an `!important` rule;
  - `@media (max-width: 500px)` on a 412 px Mobile form;
  - a `:hover` rule.

  The value and source the IDE computes equal the computed style the player gives in a headless browser at the form's size, for N of N cases.
- [ ] **AC3** (R5) — The test drives real frames of the Properties panel, as `designer_page_backdrop_tests.rs` does.
  - A button styled by `#BTN-1 { background-color: rgb(10 132 255) }` shows `#0A84FF` in its BackgroundColor row.
  - The row's mark names the sheet and line, in each of the six languages.
  - The stored value is in the tooltip.
  - A `calc()` value shows as read-only CSS text.
- [ ] **AC4** (R6, R7) — The canvas fill of that button, read back from the frame's shapes, is the CSS colour.
  - `render_form` of the same form gives the same pixel, within 1 per channel.
  - A control with `width: 50%` keeps its designed rectangle, and its badge's tooltip lists that declaration with its line.
- [ ] **AC5** (R8) — A sheet written by another program changes the canvas fill within 1 s. The time is measured and reported.
- [ ] **AC6** (R9, R10) — The edit's bytes are checked for each case:
  - a longhand;
  - a shorthand that can be rewritten (`border: 1px solid red`, then a BorderColor edit);
  - a shorthand that cannot (a longhand is added);
  - an `!important` declaration;
  - a `CssStyle`.

  For each case, a byte diff shows that the sheet changed only in the bytes the edit names, and the effective value afterwards is the one typed.
- [ ] **AC7** (R11) — A `.pr-button` rule styles three buttons, and the question names 2 others.
  - **Change the rule** changes all three.
  - **Only this control** adds `#BTN-1 { … }` at the end of the form's sheet, and only BTN-1 changes.
  - **Cancel** leaves both files byte-identical.
- [ ] **AC8** (R12) — A later `!important` rule beats the write. The write is undone, the sheet is byte-identical to before, and the message names the winning file and line.
- [ ] **AC9** (R13) — Undo and the two guards:
  - Ctrl+Z after a write restores the sheet byte for byte, and Ctrl+Y applies the write again.
  - With unapplied text in the modal, the write is refused with its message.
  - A sheet changed on disk between the read and the write is read again, and the other program's change is kept.
- [ ] **AC10** (R14) — The two row actions:
  - **Remove the CSS declaration** asks first, removes only that declaration, and undo puts it back.
  - **Go to rule** opens the modal with the caret on the rule's line.
- [ ] **AC11** (R15) — Three buttons are selected, two different effective colours among them. The row shows mixed; one edit gives all three the typed value; there is one question.
- [ ] **AC12** (R17) — On the AC2 fixture:
  - `render_form`, the agent's property read and `check` give the effective value and source;
  - the agent's set-property answer names the winning declaration.
- [ ] **AC13** (R11, i18n) — The question window keeps the same rectangle over 120 frames in each of the six languages. After a drag of its grip it keeps the dragged size. Every new string exists in the six languages.
- [ ] **AC14** (R19) — With a stylesheet placed beside a Desktop form, the Properties panel shows no mark and the canvas paints the stored properties. The existing designer and Properties tests stay green.
- [ ] **AC15** (R20) — The time for the 200-control, 500-rule fixture is printed in the test's result block, within the plan's budget.
- [ ] **AC16** (the operator's report) — In a copy of `examples/PowerMobileHello`, a form sheet colours the buttons and changes the label's font. The designer's canvas and the emulator's screen agree on those colours and that font. Both screenshots are kept as evidence.

## 6. Constraints & steering check

- **The frontend↔backend channel (WASM-INDEX D21, operator 2026-10-24):** this spec puts nothing on the channel between the frontend and the backend, and must not contradict the series-wide ruling.
  - The channel is asymmetrically encrypted, with a key pair generated when the application starts.
  - A broken connection is retried with a timeout of 10 s, raised by 10 s on each retry, for at most 3 cycles; then an exception is raised.
  - While it retries, all input is blocked behind the "connecting" modal of WASM-016 R20 (WASM-012 R15 on a device).
  - The owners are WASM-003 R24–R26, WASM-016 R20–R21, WASM-012 R15, WASM-005 R18, WASM-014 R18 and WASM-013 R17.
- **i18n:** yes, every new string in the six languages:
  - the new mark texts ("from the theme", "not in effect", the unknown value);
  - the question window and its three buttons;
  - Go to rule;
  - Remove the CSS declaration and its confirmation;
  - the refusal and "still wins" messages.
- **Generated-code contract:** none. Stylesheets are form data the developer owns, no COBOL is generated or changed, and the regenerate contract is untouched.
- **User code is sacred:** R10–R14.
  - Write only what the developer asked, and keep every other byte.
  - Ask before a shared rule changes.
  - Confirm and allow undo on every removal.
  - Never remove a rule the developer wrote.
- **Self-resizing windows:** R11's question window. Read the `egui-resize-guardian` and `egui-paint-regressions` notes before building it.
- **Render:** both paths of the render engine (`render_form`, `render_faces`). The guard `engine_reference_form_parity_static_vs_faces` stays green.
- **Docs:** `docs/` is untouched on 2.x.x. The Developer's Guide chapter is written when 2.x.x merges into main.
- **System KB:** required. The Properties panel and the designer behave differently for a Web or Mobile form:
  - update the WASM-007 text in `cobolt-compiler`'s documentation tables;
  - regenerate `assets/knowledge/chunked.data`.
- **Interpreter parity:** not concerned; nothing at run time changes. The player changes only if the reverse table's test finds `cobolt-web-player/src/css.rs::TABLE` and `CSS_TO_PROPERTIES` disagreeing.
- **Fix vs feature:** **feature** (`features-2x`). It is a capability beyond the IDE's existing scope, and it reverses a written requirement (WASM-007 R12). The defects in the same report shipped as fix 2.0.298.

## 7. Open questions

Each one carries a recommendation, and the requirements are written to it until the operator rules.

- **Q1 — A shared rule (R11):** ask on every edit, or remember the answer for that rule until the sheet changes?
  - *Recommended:* ask on every edit. One click more, and no edit ever restyles other controls unseen.
- **Q2 — Where "Only this control" writes (R11):**
  - an id rule in the form's stylesheet: the CSS stays in the stylesheet, where WASM-007 Q2 put it;
  - or the control's `CssStyle`: it wins without `!important`, but moves CSS into the form file.

  *Recommended:* the id rule.
- **Q3 — A property that no rule overrides:** keep storing it as a property, or write every edit of a Web or Mobile form as CSS?
  - *Recommended:* keep storing it as a property. The cascade of R8 and the panel's way of working stay as they are, and CSS appears only where the developer wrote it.
  - Writing everything as CSS gives one store, but it empties the form file's properties and WASM-004 R6's round trip between kinds.
- **Q4 — The theme block's values for a property a control does not store (R5):** show them, because the page shows them?
  - *Recommended:* yes.
  - The cost: a form saved before a property was seeded can show many rows marked "from the theme".
- **Q5 — The agents (R17):** report only, or let the Form Designer agent's set-property tool write back as the panel does, answering R11's question itself?
  - *Recommended:* report only. The agent then edits the stylesheet as a file, which the developer sees in the diff.
- **Q6 — `@media` decided against the design size (R4):** which size counts, when the developer runs on another phone?
  - the form's size;
  - or the device chosen in **Run on device**.

  *Recommended:* the form's size. It is what the designer shows. The Preview is the truth for any other device.
