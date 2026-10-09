# Spec 091 — Form Layers and Non-visual deployment

- **Status:** draft → approved
- **Folder:** specs/091-form-layers-and-non-visual-deployment/
- **Author:** Claude (Sonnet 5.5) with the operator   **Date:** 2026-10-08
- **Classification:** feature (a capability the developer did not have). `features` line, `z` bump; only the operator raises `x` or `y`.
- **Source:** the operator's request of 2026-10-08 — twelve numbered rules, traced to requirements in §4.10. The operator's screenshot of the proposed tab bar (received 2026-10-08, after the first draft) is described in R23–R26: a strip under the canvas reading `Form` · `Layer 1` · `Layer 2` · `+`, a checkbox and a red X on each layer tab, and ◀ ▶ scroll arrows at its right edge.

## 1. Overview

A form is one flat plane: every control sits on the same canvas. When a program shows and hides controls according to its own logic — an error banner, a busy overlay, the steps of a wizard — the designer still shows all of them at once, stacked on top of each other, so the form cannot be read or edited.

This feature adds **layers**. The form as it exists today becomes the **base**. Above it the developer may add layers: transparent planes, stacked in an order the developer controls, each holding its own controls. Every layer can be shown or hidden in the designer and from COBOL, so the controls that belong together appear and disappear together. A form with no layers is exactly the form it is today.

## 2. Goals / Non-goals

- **Goals:**
  - G1 — Let the developer author overlapping, program-controlled controls on separate planes and show or hide each plane while designing.
  - G2 — Let COBOL show, hide and restyle a layer at run time by name.
  - G3 — Change nothing for a form that does not use layers: same file, same picture, same behaviour.
- **Non-goals:**
  - Nested layers. Layers form one flat stack above the base.
  - A layer is not a window, a form or a program: no WORKING-STORAGE of its own, no window properties, no corner radius.
  - Layer events (`onShow` and the like), and re-stacking a layer from COBOL (Q9).
  - Replacing `Panel`, `GroupBox`, `TabControl` pages or embedded forms. They keep working inside a layer.
  - Layout inside a layer: no Dock, Anchor, flex, grid or flow (R21).

## 3. User stories

- As a developer, I want the controls of a busy overlay on their own layer, so that I can hide the overlay while I design the screen underneath.
- As a developer, I want to hide a layer in the designer and see only the layers I am working on.
- As a developer, I want a mouse click on the empty part of an overlay to reach the button underneath it, so that an overlay with a label and a close button does not block the whole form.
- As a developer, I want `SET LAY-ERROR::Visible TO TRUE` to show a layer, so that the program decides when it appears.
- As a developer with an existing application, I want my forms to load, run and save as before.

## 4. Requirements (EARS)

### 4.1 Model and compatibility

- **R1 (ubiquitous):** The system shall give every form one **base** — the form as it exists today — and zero or more **layers** stacked above it.
- **R2 (ubiquitous):** A form with no layers shall load, render, run, generate COBOL and save exactly as before this feature. Saving it shall write no layer information, and a `.cfrm` that never had layers shall come back byte-identical.
- **R3 (ubiquitous):** The system shall give every control a **`Layer`** property naming the layer it belongs to. Its default is `Form`, the name of the base (the operator, 2026-10-08: the base is called `Form`; `base` was only a working name).
- **R4 (ubiquitous):** A control name shall be unique across the **whole form** — base and every layer — compared without regard to case.
- **R5 (event):** When a name would collide — a rename, a paste, a duplicate, a move between layers, or a load — the system shall refuse the rename and shall give a pasted or duplicated control the next free name. A `.cfrm` that already holds a collision shall be reported in the Output panel and shall not be repaired by deleting anything.
- **R6 (ubiquitous):** A layer shall have a name, with the same character rules as a control name and drawn from the **same namespace** as control names. `Form` is reserved, in any letter case, and no layer may carry it. A new layer shall be named `Layer-1`, `Layer-2`… the first free one.
- **R7 (constraint):** A form shall hold at most **64 layers** above the base (Q5). Adding another shall be refused with a message.
- **R8 (ubiquitous):** A control inside a container (`Panel`, `GroupBox`, a `TabControl` page…) shall belong to its container's layer. Moving a container to another layer shall move everything inside it.

### 4.2 Stacking and painting

- **R9 (ubiquitous):** Layers shall stack in order: the base lowest, then each layer above the one before it. A layer next to the base is **farther** from the observer than a layer far from the base.
- **R10 (ubiquitous):** Z-order happens **by layer** (operator, 2026-10-08). Every control of a layer shall paint above every control of every layer below it, whatever their `ZOrder` values: a control whose `ZOrder` is 0 — or negative — in a higher layer is still above all the controls of the lower layers, the base's included, even one whose `ZOrder` is 209. `ZOrder` shall order only controls that share a layer and a parent. `BringToFront` and `SendToBack` therefore move a control to the top or the bottom of **its own layer** and never across layers; changing the layer a control is on is R31's job.
- **R11 (ubiquitous):** The base shall always be the lowest and shall not be re-stacked. Any other layer may be moved to any position above the base, and the order shall be saved.
- **R12 (state):** While a layer is not visible, none of its controls and none of its background shall paint, and none of its controls shall receive mouse events or keyboard focus.
- **R13 (ubiquitous):** A layer shall cover exactly the form's rectangle. It shall have no position or size of its own, shall not change the form's size, and shall scroll and resize with the base.
- **R14 (constraint):** Whatever a layer paints outside the form's rectangle, or outside the arcs of the base's rounded corners, shall be clipped — for **every** control type, including the ones that do not clip themselves at a corner today (F5).

### 4.3 Layer properties

- **R15 (ubiquitous):** A layer shall have `Name`, `Visible`, and the form's own backdrop properties with the same meaning and ranges: `BackgroundColor`, the background gradient (enabled, start, end, direction), `BackgroundImage` with its scaling mode, and `Transparency` (Q6). A new layer shall be fully transparent, so adding one changes nothing that is drawn.
- **R16 (constraint):** A layer shall not have `CornerRadius`, and shall not have any window property (`Title`, `WindowState`, `Resizable`, the window-dock properties…). Its corners are the base's.

### 4.4 Mouse events

- **R17 (event):** When the pointer presses, releases, clicks, double-clicks, moves, enters, leaves or wheels over a point of the form, the system shall offer the event to the visible layers from the nearest to the observer down to the base, and give it to the first control that has a **painted part** at that point.
- **R18 (event):** When a point holds no painted part of any control in a layer — the layer's own background, the empty area of a control that paints nothing there, the area around a Label's text — the event shall pass to the layer below, unless the layer's background is opaque there (R20), and from the lowest layer to the base.
- **R19 (event):** When a point lies on a painted part of a control, the event shall belong to that control: its COBOL handler runs if one is bound, and the event is **discarded** if none is. It shall not fall through to the layers below.
- **R20 (constraint):** A layer's background shall never be the target of an event: no COBOL handler runs for it. Where the background is **opaque** at a point — it paints there at full opacity, so what is below cannot be seen — an event at that point shall not pass to the layers below and shall be discarded: an opaque layer holds the focus, and everything beneath it is shielded (operator, 2026-10-08). Where the background is transparent or translucent at a point, the event passes on as R18 says (Q8b). A control of the layer, painted above its background, still receives its own events (R19).

### 4.5 Layout

- **R21 (constraint):** Only the base shall have layout behaviour: `Dock`, `Anchor` and the flex, grid and flow layouts of spec 056. A control in a layer shall keep its designed `X`, `Y`, `Width` and `Height`, and the inspector shall not offer it those properties. When a control is moved from the base to a layer, its `Dock` shall become `None` and its `Anchor` the default; undoing the move shall give them back (Q7).
- **R22 (constraint):** The form-level window properties (`DockToOpener` and the rest of spec 037) shall stay on the form, and a layer shall not have them (R16).

### 4.6 Designer

- **R23 (ubiquitous):** The designer shall show, at all times, a **tab bar directly under the form canvas**: the base's tab at the left, named `Form` — its name, so it reads `Form` in every IDE language, like the `Layer` property value it stands for — and not movable, then one tab per layer in stack order, left to right from the layer nearest the base. After the last tab sits a `+` that adds a layer. A form with no layers shows the `Form` tab and the `+`. The bar has a fixed height; when its tabs do not fit, **◀ ▶ arrows at its right edge** scroll it, and it shall not make the window larger.
- **R24 (event):** When the developer selects a tab, that layer shall become **active** and the Properties inspector shall show that layer's properties, as it does the form's when the form is selected.
- **R25 (event):** When the developer clicks the visibility checkbox on a layer tab, the layer's `Visible` shall toggle and the canvas shall show or hide that layer's controls at once, except that the active layer stays drawn so it can be edited (Q2b). The checkbox shows and sets the layer's `Visible` property and nothing else, so the value designed is the value the program starts with (R35; operator, 2026-10-08). The `Form` tab has no checkbox: the base is always visible.
- **R26 (event):** The developer shall be able to add a layer (the `+`), delete it (the red X on its tab), rename it and re-stack it from the tab bar. A layer is re-stacked by **dragging its tab and dropping it** at another position among the layer tabs (operator, 2026-10-08). The `Form` tab is the only fixed one: it cannot be dragged, and no tab can be dropped to its left. The gesture for renaming is not in the screenshot (Q1).
- **R27 (event):** When the developer deletes a layer that holds controls, the system shall first ask for confirmation naming how many controls it holds. The controls shall leave through the same path as deleting a control — their handler code kept in the form's recycle bin — and the whole deletion shall be one undo step.
- **R28 (event):** When the developer drops a new control on the canvas, it shall be created in the active layer.
- **R29 (state):** While a layer is active, what the pointer and Select All can reach is limited to that layer's controls (R44, R45).
- **R30 (state):** While any layer other than the base is active, the form shall not be resizable in the designer: no resize grips on the form, and `Width` and `Height` not editable. Resizing shall be possible only while the base is active.
- **R31 (event):** The developer shall be able to move the selected controls to another layer from the inspector (`Layer`) or the context menu, as one undo step.
- **R32 (ubiquitous):** Every layer operation — add, rename, delete, re-stack, show or hide, move controls, change a layer property — shall be undoable and redoable.

### 4.7 Run time

- **R33 (ubiquitous):** A program shall be able to read and write a layer's properties by name, the way it does a control's: `LAYER-NAME::Visible`, `Transparency`, `BackgroundColor`, the gradient properties, `BackgroundImage` and its mode.
- **R34 (event):** When a program writes a layer property, the next frame shall show the new state.
- **R35 (ubiquitous):** A layer shall start in the state it was designed in.
- **R36 (ubiquitous):** Layers shall behave the same in every host that runs a form: `rcrun run-form`, a form embedded as a child, and the compiled binary.
- **R37 (ubiquitous):** A reference to a layer name or a layer property that does not exist shall be reported at build time, as an unknown control is.
- **R38 (ubiquitous):** A control in a layer shall be addressed by its name exactly as a control in the base is. `MY-BOX::Text` shall not depend on the layer `MY-BOX` is in.

### 4.8 Persistence and tooling

- **R39 (ubiquitous):** The `.cfrm` shall record the layers in stack order with their properties, and each control's layer. A form with no layers shall write none of it (R2).
- **R40 (event):** When a `.cfrm` names a layer that it does not define, the control shall be kept, shown in the base, and the problem reported in the Output panel. Nothing shall be deleted.
- **R41 (ubiquitous):** The generated-COBOL contract shall hold: the developer banner, regeneration on Build, Run, Debug and Check, English identifiers, and no change to the generated code of a form without layers.
- **R42 (ubiquitous):** A form with layers loaded into a shell's ContentPane shall behave as in a window: its layers cover the pane area the form occupies.

### 4.9 Designer — the active layer (added 2026-10-08)

- **R43 (state):** Exactly one tab is **active** at any time — the `Form` tab when the base is active. The active tab shall be drawn with a **blue face and white text**; every other tab shall be drawn with a **white face and blue text**. The colours are the same on every IDE theme.
- **R44 (state):** While a layer is active — or the base — every pointer interaction on the canvas shall be limited to the controls of that layer: click, Ctrl-click, drag to move, resize handles, double-click, context menu, hover feedback and the **rubber-band lasso**. The controls of every other layer are **inactive**. They are drawn, when their layer is visible, but the pointer treats them as absent: a press on one is a press on empty canvas and starts a lasso, an inactive control never blocks an active one beneath it, and a lasso selects only the active layer's controls, however many others its rectangle covers. This holds when the base is the active layer too. When the active layer changes, controls that are not in the new active layer shall leave the selection; so shall controls moved to another layer (R31).
- **R45 (event):** When the developer presses Cmd+A on macOS or Ctrl+A elsewhere, and no text field has focus, the designer shall select every control of the active layer — the base's, when the base is active — and **no** control of any other layer. It selects the same set as today's Select All (F10), restricted to the active layer; when that layer holds no controls, the selection becomes empty.

### 4.10 The operator's twelve rules → requirements
| 1 Unique control names | R4, R5, R6 |
| 2 Control property naming its layer; the base is named "Form" | R3, R6, R8 |
| 3 Tab navbar, visibility checkbox, re-stacking, stack direction | R9, R11, R23–R26 |
| 4 Mouse events through transparent areas | R17, R18, R19 |
| 5 A layer cannot resize the form; overflow clipped | R13, R14 |
| 6 Only the base defines dock behaviour | R21, R22 |
| 7 Layer background and transparency | R15 |
| 8 No layer corner radius; elements clipped by the base's | R14, R16 |
| 9 Design-time and run-time properties | R15, R25, R33–R35 |
| 10 Layer background handles no events | R20 |
| 11 Up to 64 layers | R7 |
| 12 Base resize only when the base is active | R30 |
| Added 2026-10-08 — active tab blue with white text, the others white with blue text | R43 |
| Added 2026-10-08 — pointer, lasso included, limited to the active layer's controls; the others inactive | R29, R44 |
| Added 2026-10-08 — Cmd/Ctrl+A selects only the active layer's controls | R45 |
| Added 2026-10-08 — z-order happens by layer: ZOrder 0 in a higher layer is above every control of the layers below | R10 |

## 5. Acceptance criteria

Each is verified by a test, never by driving the application (CLAUDE.md).

- [ ] AC1 (R2, R39) — every `.cfrm` under `examples/` and `tests/` loads and saves back byte-identical; the existing `engine_reference_form_parity_static_vs_faces` and the `cobolt-forms` suite stay green.
- [ ] AC2 (R4–R6) — a rename, paste, duplicate and move that would collide are refused or renamed; a layer named `Form` (in any letter case), or like an existing control, is refused; a `.cfrm` with a collision is reported and loses nothing.
- [ ] AC3 (R7) — the 64th layer is added; the 65th is refused.
- [ ] AC4 (R8) — moving a `Panel` moves its children; a child cannot be put in a layer other than its container's.
- [ ] AC5 (R9–R11) — two overlapping buttons, the lower layer's with the higher `ZOrder`: the upper layer's paints on top, with `ZOrder` 0 against 209, and with a negative `ZOrder` against 10000; `BringToFront` on the lower one leaves it below the upper one, and `SendToBack` on the upper one leaves it above. Within one layer `ZOrder` still decides. Re-stacking changes the picture; the base cannot move.
- [ ] AC6 (R12) — a hidden layer paints nothing and receives no event.
- [ ] AC7 (R13, R14) — a control that crosses the form edge, in every control type, paints no pixel outside the form; in a form with a rounded corner, none outside the arc (extends `a_child_at_a_rounded_corner_stays_inside_the_arc`).
- [ ] AC8 (R15, R16) — a new layer is fully transparent; background, gradient, image and `Transparency` render as the form's do; no corner radius is offered.
- [ ] AC9 (R17–R20) — with a Button in the base under a layer: a click on the layer's empty area runs the Button's handler; a click on a layer Button runs only that handler; a click on a layer control with no handler runs nothing, and not the Button's; a click beside a transparent Label's text reaches the Button; a click on a fully opaque layer background runs nothing and does not reach the Button, while a layer control painted above that background still gets its own click; the same click on a translucent background reaches the Button; no COBOL handler ever runs for a background; the same cases with three layers.
- [ ] AC10 (R21, R22) — a layer control has no `Dock`/`Anchor`; moving a docked control to a layer resets them and undo restores them.
- [ ] AC11 (R23–R32) — designer tests: the tab bar exists on a form with no layers; the checkbox toggles `Visible` and the saved form carries the value the program starts with; dragging a layer tab to another position re-stacks the layers and changes the paint order, the `Form` tab cannot be dragged and nothing can be dropped to its left; a new control lands in the active layer; resize grips and `Width`/`Height` are disabled unless the base is active; adding 64 layers does not change the window size; deleting a layer with controls asks, keeps handler code and undoes in one step.
- [ ] AC12 (R33–R38) — one COBOL program sets `Visible`, `Transparency` and `BackgroundColor` of a layer and the result is identical in `rcrun run-form`, an embedded child and the compiled binary; an unknown layer or property fails the build.
- [ ] AC13 (R40, R41) — a `.cfrm` naming an undefined layer loads with the control in the base and a report; a form without layers generates the same COBOL as before (`tests/golden/`).
- [ ] AC14 (R42) — a layered form in a ContentPane paints its layers over the pane area.
- [ ] AC15 — a bench reports, for 64 layers × 50 controls against the same 3,200 controls in the base, the frame time and the hit-test time. The numbers are measured and printed (GOLDEN RULE #7); the plan sets any budget.
- [ ] AC16 (R43) — with the base active the `Form` tab is blue with white text and every layer tab white with blue text; selecting `Layer 2` swaps them; exactly one tab is ever in the active style; the colours are identical under every theme in the IDE registry.
- [ ] AC17 (R44) — with `Layer 1` active and a control in each of the base, `Layer 1` and `Layer 2`: a click selects only the `Layer 1` control; a click, a drag and a double-click on the other two do nothing to them and start a lasso instead; a lasso over all three selects only the `Layer 1` control; a `Layer 2` control drawn over a `Layer 1` control does not stop the click reaching the `Layer 1` one; the same four cases with the base active; switching the active layer drops the previous layer's controls from the selection; after a move to another layer (R31) the moved controls are no longer selected.
- [ ] AC18 (R45) — Cmd+A (macOS) and Ctrl+A (elsewhere) with three controls in each of base, `Layer 1` and `Layer 2` select exactly the three of the active one, children inside containers included as today; an active layer with no controls leaves the selection empty; with a text field focused nothing is selected; on a form with no layers the result is what it was before this feature.

## 6. Constraints & steering check

- **i18n (6 languages):** yes. Tab bar, tooltips, the add/rename/delete/re-stack commands, the delete confirmation, the inspector labels and every message above are `Tr` fields in all six languages. `Layer-1` and the property names stay English.
- **Generated code:** a form with layers regenerates on Build/Run/Debug/Check; a form without them generates what it generated before. Layer names are COBOL identifiers and stay English. `tests/golden/` pins both.
- **System KB:** the `Layer` property, the layer properties and the `layer::Property` syntax enter the `cobolt-compiler` property/method/event doc tables in the same change; `cargo run -p cobolt-ide --example build_chunked_kb` regenerates `assets/knowledge/chunked.data`; `prebuilt_chunked_kb_matches_the_published_documentation` must be green.
- **Docs:** `docs/developers-guide-en.md` gains a Layers section, written for a PowerCOBOL or isCOBOL developer, in COBOL and prose only, with screenshot placeholders; its five translations are deleted in the same change (GOLDEN RULE #8).
- **Interpreter parity:** `rcrun run-form`, embedded child forms and the compiled binary (`run_form_app`) all change; read the `interpreter-binary-parity` skill first.
- **Rounded corners:** R14 sits on spec 057; read the `rounded-corners` skill first.
- **Windows never resize themselves:** the tab bar has a fixed height and scrolls (R23), and the designer window does not change size when the 64th tab arrives.
- **Contrast:** the active and inactive tab colours (R43) are fixed, not read from the IDE theme, since the IDE ships 33 themes and glass ones render dark-on-dark (CLAUDE.md, chat-UI rule). The plan checks that the checkbox and the red X read on both faces.
- **User code is sacred:** deleting a layer keeps its controls' handler code (R27); a bad `.cfrm` is reported, never repaired by deleting (R5, R40).
- **Fix vs feature:** feature. Branch `features`. That branch is checked out in another worktree and is 200 commits behind `main`, so work goes on a per-change branch off `main` — `features-form-layers`, created for this spec — and lands with `git push origin HEAD:features` (`structure.md`). The push obeys GOLDEN RULE #1.
- **Version:** `z` bump from 1.90.21 and a `CHANGELOG.md` entry.
- **Forum:** no announcement unless the operator asks (GOLDEN RULE #4b).

## 7. Open questions

Each carries the default this spec assumes.

- **Q1 — What the screenshot leaves open.** (a) Resolved 2026-10-08: layers are re-stacked by dragging and dropping their tabs; the `Form` tab is the only fixed one (R26). (b) How a layer is renamed: double-click on its tab is assumed. (c) The checkbox draws as a boxed X; this spec reads boxed X as checked, so visible. (d) Resolved 2026-10-08: the base is named `Form`, and no layer may take that name (R6). (e) The screenshot's tabs read `Layer 1` and `Layer 2`, with a space; a name a program writes as `LAYER-1::Visible` cannot hold one. The spec names new layers `Layer-1`, `Layer-2` (R6). Should the tab show the name as is, or a spaced label for it?
- **Q2 — Resolved by the operator, 2026-10-08.** The checkbox **is** the layer's `Visible` property: a layer hidden in the designer starts hidden at run time (R25, R35).
- **Q2b — Left over from Q2.** Should the designer still draw the **active** layer when its box is off, so it can be edited? Default: yes, as R25 says; every other surface honours `Visible`.
- **Q3 — Resolved by the operator, 2026-10-08.** The controls of the other layers are inactive while one layer is active, for every mouse event including the lasso (R44), and Select All reaches only the active layer (R45).
- **Q4 — What may live in a layer.** Default: `MenuBar`, `SideMenu` and `StatusBar`, which are window chrome, and the non-visual controls (`Timer`, `AgentObject`, `RestClient`, `SqlDatabase`, `IndexedFile`…) belong to the base only. Every other control type may live in a layer.
- **Q5 — The limit of 64.** Default: 64 layers **above** the base, so 65 tabs with the `Form` tab. If 64 includes the base, change R7 to 63.
- **Q6 — `Transparency`.** Default: as on a form, it fades the layer's **backdrop**; the controls keep their own `Transparency`. The alternative, as on a `Panel`, fades the controls too.
- **Q7 — "Dock behaviour" (rule 6).** Default reading: the per-control `Dock`, `Anchor` and layout modes of spec 056, and the form-level window dock. If rule 6 meant only one of them, R21/R22 narrow.
- **Q8 — Resolved by the operator, 2026-10-08.** An opaque layer background does not let clicks through: opaque means focus on that layer, and what is below it is shielded (R20). The background still handles no event of its own.
- **Q8b — Left over from Q8: how opaque is "opaque"?** Default: fully opaque at the point — `Transparency` 0 and a colour, gradient or image pixel with full alpha there. A background with any transparency lets events through, so a dimming scrim that must also block clicks needs a `Panel` over the layer, as the `SCRIM` Panel in the screenshot is today. If any visible fill should block, R20 changes to "paints anything".
- **Q9 — More from COBOL.** Not in this spec: re-stacking a layer at run time, a layer `Enabled`, layer events. Add any of them?
- **Q10 — Keyboard focus.** Default: Tab walks the base first, then each layer from the lowest up; hidden layers are skipped, and so are the controls shielded by an opaque layer background (R20), which cannot be seen or clicked.
- **Q11 — A control's `Layer` at run time.** Default: readable, not writable. The alternative lets a program move a control to another layer.

## 8. Findings

Read on `main` at 7ff1c4f (1.90.21), 2026-10-08. Line numbers rot; `/plan` re-reads each one.

- **F1 — Flat controls with a parent link.** `Form.controls: Vec<Control>` (`cobolt-forms/src/model.rs:8039`); nesting comes from `Control.parent` (`:4130`) and the `<Children>` tree is rebuilt at save. A control's layer follows the same pattern: a field on the flat control.
- **F2 — `ZOrder`.** `Control.z_order` (`:4118`) orders siblings only (spec 012 R14), which is what R10 relies on.
- **F3 — One namespace.** A control's name is `Control.id`. The designer issues free names with `next_unique_id` / `next_unique_id_reserved` (`cobolt-ide/src/panels/designer.rs:4422`, `:4439`), and COBOL reaches a control as `receiver::member` (`control_refs_in_code`, `model.rs:1725`), so a layer joins the same flat namespace.
- **F4 — The painted-part rule exists.** `window_drag_blocked` (`cobolt-forms/src/render.rs:3078`) with `face_see_through` (`:3008`), `passive_to_the_mouse` (`:3041`) and `binds_a_press` (`:3031`) already decides which parts of a Label, Panel, GroupBox and PictureBox paint and which are see-through. R18/R19 ask the same question for every mouse event. **One difference:** a press on an unbound passive control moves a frameless window today; R19 discards the event instead. `/plan` should reuse the painted-part test, not copy it.
- **F5 — Corner clipping.** `self_clipping_type` (`render.rs:969`) and `notch_mask_rounding` (`:915`). Per CLAUDE.md (as of 1.65.2) `DataGrid`, `FileDropZone`, `Maps`, `TabControl`, `ToolBar` and `Custom` do not clip themselves; R14 requires a clip for them in a layer.
- **F6 — Two meanings of dock.** A control's `Dock`/`Anchor` and layout modes are spec 056. `DockEdge` (`model.rs:7561`) is the form-level window dock of spec 037. See Q7.
- **F7 — Form corner radius.** `Form.corner_radius` (`:8103`) rounds the window only while the title bar is hidden (`window_corner_radius`).
- **F8 — Load and save.** `load_form` (`cobolt-forms/src/xml.rs:536`) and `save_form` (`:1936`).
- **F9 — The form's backdrop fields** that R15 repeats on a layer: `background_color`, `background_gradient_*`, `transparency`, `background_image`, `bg_image_mode` (`model.rs:8022`–`8038`).
- **F10 — Select All and the lasso today.** Cmd/Ctrl+A is handled at `cobolt-ide/src/panels/designer.rs:9517` (`modifiers.command`, only while no text field has focus) and selects every id in `form.controls`, which is the flat list, so children inside containers are included. The rubber-band lasso starts from a drag on empty canvas (`:13875`) and is drawn at `:9199`. R44 and R45 narrow both to the active layer.
