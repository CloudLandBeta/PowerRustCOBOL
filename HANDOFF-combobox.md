# Handoff — ComboBox: bring it up to the ListBox's standard

Written 2026-08-18, at the end of the session that fixed the ListBox (1.61.82 –
1.61.89). Read `CLAUDE.md` first; this covers one control and assumes the
conventions there.

**The premise:** the ListBox was reported five times in one session and each
report has a ComboBox twin. The rules below are the ones the list now follows,
restated for a control that is a closed field plus a dropdown. Nothing here is
speculative — every "today" line was read out of the code at 1.61.89.

## State of the tree

`main` is at **1.61.89**, everything merged and pushed. The ListBox work landed
as 1.61.82 (SideMenu, unrelated), 1.61.84–1.61.88; the ComboBox already got
1.61.89.

Full sweep:

```bash
cargo test -q --workspace --exclude cobolt-bench --features cobolt-forms/render \
  --no-fail-fast -- --skip generated_binary_source_actually_compiles
```

109 suites. **`test_external_crates_e2e` fails on this machine** (2 tests,
`libsqlite3-sys` will not compile in its nested build) — it reproduces on a clean
tree, so it is environmental, not yours. Then run the skipped one alone:

```bash
cargo test -p cobolt-compiler --lib generated_binary_source_actually_compiles
```

## Already done — do NOT redo

| Version | What | Why the ComboBox already has it |
|---|---|---|
| 1.61.84 / 1.61.86 | The inspector's **Items (one per line)** box shows five lines and then scrolls, and its border stays whole at any scroll offset | `items_multiline` → `bounded_items_editor` in `crates/cobolt-ide/src/panels/properties.rs` is the SAME widget for both controls |
| 1.61.89 | **`ActiveItemColor`** (the selected item) and **`HoverItemColor`** (the item under the pointer) name the dropdown's two highlights | ComboBox-specific, already landed |

## The work

### 1. The face is the developer's — from ListBox 1.61.87

**The list's rule.** It painted a hardcoded navy surface over its own face, so a
background designed in the RAD never reached the running form: the designer
canvas showed the design, the preview, Run Form and the compiled binary showed
the navy. Its face now comes from `paint::draw_control` — the same call the
canvas uses — so `BackgroundColor`, the background gradient, the border and the
corner radius reach every surface.

**The ComboBox today.** `paint::glass_combo_header` opens with

```rust
draw_surface_auto(painter, rect, Color32::from_rgb(25, 38, 80), 6.0, …)
```

plus a hardcoded blue border stroke — the identical defect, on the closed field.
The popup (`glass_combo_popup`) hardcodes its two fills `(22,30,58)` and
`(30,42,80)`, its border stroke and its 6.0 radius as well.

**Wanted.** The header wears the designed face. The dropdown takes its surface
and border from the control instead of constants, with the same "empty means
exactly what it drew before" fallback discipline 1.61.89 used for the highlights
— a ComboBox designed earlier must not restyle itself.

### 2. The dropdown must scroll — from ListBox 1.61.85

**The ComboBox today.** `popup_h = (items.len() as f32 * 22.0).min(180.0)`, and
the item loop `break`s as soon as an item would fall past the bottom. **Items
beyond about eight are unreachable** — not clipped, not scrollable, simply
dropped. That is the same bug the list's scrolling menu pane fixed.

**Wanted.** Every item reachable; the popup scrolls.

### 3. Drag through the dropdown, with an anchor

**The list's rule.** A drag is ONE gesture with an anchor — the row the press
landed on — and what it selects is the range from that anchor to the row under
the pointer *now*, recomputed every frame, so reversing direction shrinks the
range. What it replaced was an accumulating "every row this press has touched"
set, which never let go: dragging back up crossed only rows already in the set
and the list went deaf in both directions until the button came up.

**Wanted.** Press on the header, drag into the list, release on an item to pick
it — the classic combo gesture, absent today (click only). Build it with the
anchor model. Do not build the accumulating one.

### 4. Stop at the first and last item

Dragging above the first item holds at the first, below the last at the last. A
drag that leaves the control stops on an item rather than selecting nothing.

### 5. Arrow keys

Up and down move the highlighted item, clamped at both ends, reporting
themselves with `onChange` and `onSelectedIndexChanged` exactly as a click does.

* Popup open: the arrows move the highlight, **Enter** picks, **Escape** closes
  without changing the value.
* Popup closed: the arrows change the value directly, as a Windows combo does.

⚠️ **Caveat.** `Editable` combos type into the field. Decide deliberately what
the arrows mean there, and write the decision down.

### 6. The current item is always visible

Opening the popup scrolls to the current value. Arrowing or dragging keeps the
highlighted item in view, landing it on the **first or last visible line**
(`align: None` — move by the least you can).

Scroll **immediately**:

```rust
ui.scroll_to_rect_animation(rect, None, egui::style::ScrollAnimation::none());
```

egui's default eased scroll is outrun by a fast drag — the list ended several
frames behind the hand, with the chosen row still below the frame, which is
exactly the "I cannot see what is selected" the operator reported.

And a drag is a **selection, not a swipe**: if the popup uses a `ScrollArea`,

```rust
.scroll_source(egui::containers::scroll_area::ScrollSource {
    drag: egui::containers::scroll_area::DragScroll::Never,
    ..Default::default()
})
```

otherwise the content slides under the pointer while the selection follows it,
and the item under the hand runs away from it.

### 7. The popup's typography — adjacent, and real

Item height `22.0`, `FontId::proportional(12.0)` and the two item text colours
are hardcoded in `glass_combo_popup`. The header already takes the control's own
font and colour (it used to hardcode 12 pt too). The popup should as well.

## Pitfalls that cost real time on the ListBox

* **The popup is drawn in a SECOND pass.** `open_combos` → `glass_combo_popup`
  runs when the `Control` is no longer in hand. Anything read from the control —
  colours, fonts, sizes — must be resolved in the first pass and carried through
  `OpenCombo`, the way `combo_popup_fills` already is.
* **egui answers a plain arrow key itself**, by moving focus to the widget lying
  in that direction — and every row of a list is one. The list had to own its
  arrows in its OWN temp state (taken on a press inside, given up on a press
  elsewhere or on Tab). Reading `memory.has_focus` alone buys you exactly one
  arrow press before the control goes deaf.
* **Focus on click is decided on RELEASE.** Requesting focus only on the press
  loses it a frame later to whatever the release lands on. Request on both.
* **`render_interactive` arms draw their own face**; the static/canvas path uses
  `draw_control`. That is precisely why the canvas and the running form
  disagreed about the ListBox background.
* **Bounding a widget's layout is not bounding its paint** (this bit the items
  editor): a `ScrollArea` floors its own clip at LAST frame's measured content,
  so the first frame the content grows can paint past the box. Clip to bounds
  computed fresh from this frame's own numbers.
* **A border inside a scrolling area scrolls away.** The items editor's rim was
  the `TextEdit`'s own frame and was cut open the moment the text was longer
  than the box; it is now drawn by the container. A dropdown's border has the
  same shape of problem once the popup scrolls.

## Testing

Harnesses live in the `render.rs` test module:

* `drive(controls, frames)` — feeds pointer/key events frame by frame, returns
  the events raised and the property overrides written.
* `drive_painted(controls, frames)` — the same, plus what the LAST frame
  painted: texts with their ink rects, filled rects with their colours, mesh
  bounds (a gradient is a mesh) and the control rects. This is how "the designed
  gradient really is painted" and "the row it reached really is on screen" are
  asserted rather than assumed.

Tests worth reading before writing the ComboBox ones — they are the same
questions, one control over:

* `reversing_a_drag_shrinks_the_selection_instead_of_freezing_it`
* `the_arrow_keys_walk_the_list_and_stop_at_both_ends`
* `the_row_a_drag_or_an_arrow_reaches_is_scrolled_into_view`
* `a_listbox_wears_the_background_designed_in_the_rad`
* `the_items_editor_shows_five_lines_and_then_scrolls` (in `properties.rs`)

## Conventions for the change

* Fixes go on a `fix/…` branch; never commit on `main` (a PreToolUse hook
  refuses it). Merge into `main` when asked.
* Bump `z` in `crates/cobolt-ide/src/version.rs` per logical change — never the
  minor without the operator's word.
* CHANGELOG entry, and the **Developer's Guide** in the same change (the ListBox
  section is the model — see "How the operator moves through a list" and "The
  face is yours").
* If you touch the compiler KB doc constants, rebuild the chunked store:
  `cargo run --release -p cobolt-ide --example build_chunked_kb`.
* Pushes obey the São Paulo window (Mon–Fri 09:00–18:00 is closed).
* Fixes are announced on cobolforo.es **f=97**, features on **f=96** — see
  `CLAUDE.md` rules 4 and 4b, and note the fixes forum is currently announced
  only up to 1.61.64 plus the 1.61.82–1.61.86 thread posted on 2026-08-18.
