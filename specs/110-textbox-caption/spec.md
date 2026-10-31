# Spec 110 — Caption do TextBox

Status: approved by the operator 2026-10-31 (line 2.x). Author: @Emerson.

## 1. Goal and scope
The TextBox gets an optional caption placed above, below, left or right of the box. It moves and resizes with the box at a fixed distance. Font and colour follow the TextBox until set; once set they prevail.

In scope: the TextBox of `cobolt-forms` (designer, runtime, form serialisation, COBOL access to the properties). Out of scope: the mobile framework, captions for other controls, converting existing Labels, multi-line or wrapped captions.

## 2. Model
The caption is part of the TextBox, not a separate Label. It has no coordinates: its rectangle is always computed from the box. `Left/Top/Width/Height` keep describing the box only. The control has two rectangles: inner bounds (the box) and outer bounds (box + caption). The caption takes no focus, has no tab stop and no events of its own.

## 3. Properties
| Property | Type | Default | Note |
|---|---|---|---|
| CaptionText | text (`&` mnemonic, `&&` literal) | empty | empty = no caption |
| CaptionPosition | Top, Bottom, Left, Right | Top | side of the box |
| CaptionAlign | Left, Center, Right | Left | Top/Bottom only |
| CaptionVAlign | Top, Middle, Bottom | Middle | Left/Right only |
| CaptionGap | integer >= 0 | 4 | distance box to caption |
| CaptionFontName / Size / Bold / Italic / Underline | font attributes | inherited | per attribute |
| CaptionForeColor | colour | inherited | inherits the box ForeColor |
| CaptionBackColor | colour or Transparent | Transparent | never inherited |

## 4. Inheritance
Each caption attribute is inherited or set. Inheritance is live and per attribute. An attribute becomes set when it receives a value, even one equal to the box's. Returning to inherited is explicit (`ResetCaption`, or "Restore default" in the designer). The form file stores only set attributes; the designer must never bake resolved values on save. In the property map an attribute is set exactly when its key is present.

## 5. Geometry
Box (x, y, w, h), measured text (tw, th), gap g; the caption is exactly tw x th.
- Top: x by Left / Center `x+(w-tw)/2` / Right `x+w-tw`; y = `y-g-th`. Bottom: same x; y = `y+h+g`.
- Left: x = `x-g-tw`; y by Top / Middle `y+(h-th)/2` / Bottom `y+h-th`. Right: x = `x+w+g`; same y.
- Wider text overflows (Center evenly, Left to the right, Right to the left); no clip, no ellipsis.
- Computed in local coordinates, so subforms and outlets translate it with the host.
- The caption rectangle is part of the clickable area (focus) but not of the text-edit area. Same z-layer as the box.

## 6. Runtime
- Visible follows the box; there is no CaptionVisible (empty text hides it).
- Enabled = False draws the caption in the theme's disabled colour.
- Click on the caption focuses the TextBox; no click event fires. `&N` underlines N and Alt+N focuses the box. The caption text without `&` is the accessible name.
- Any change of a Caption* property, the box font or its geometry relayouts in the same cycle.
- COBOL: `TXT-NOME::CaptionText`, `TXT-NOME::ResetCaption()`, `TXT-NOME::ResetCaption( "CaptionFontSize" )`, `TXT-NOME::CaptionIsSet( "CaptionFontSize" )`. Reading an inherited attribute returns the resolved value.

## 7. Designer
Clicking the caption selects the box; handles only on the box, dashed outline for outer bounds. Dragging box or caption moves both; double click edits CaptionText in place. Snap and align commands use inner bounds; rubber band and overlap use outer bounds. Property group "Caption": inherited in grey "(inherited)", set in bold; "Restore default" per property and for the group. Warnings (not blocking): caption outside the form, overlapping another control, repeated mnemonic. Every caption property change is one undo step.

## 8. Acceptance
Unit tests of `caption_rect` (box 100,50 200x22; text 60x15; gap 4): Top/Left (100,31), Top/Center (170,31), Top/Right (240,31), Bottom/Left (100,76), Left/Middle (36,53.5), Left/Top (36,50), Right/Bottom (304,57). Behaviour checklist:
- Moving the box moves the caption by the same vector for the 4 positions.
- Resizing with Top+Right keeps the caption's right edge on the box's.
- Centre-aligned wide text overflows evenly.
- Inherited caption follows the box FontSize; a set CaptionFontSize does not.
- Setting only CaptionFontBold keeps the rest inherited; setting a value equal to the box's still marks it set.
- Save and reopen preserves which attributes are inherited or set; `ResetCaption` restores them.
- Invisible box hides the caption; disabled uses the disabled colour.
- Click and Alt+mnemonic focus the box; tab order unchanged.
- Caption inside a subform or outlet lands in the right place.
- An old form with no Caption* opens identical.

## 9. Decisions (open questions closed 2026-10-31)
Per-attribute inheritance; Top + Left, gap 4; Middle by geometric centre; disabled overrides a set colour; no `CaptionWidth` (later); gap in logical form units (type `Unit`); COBOL access uses the project's `::` syntax.

## 10. Impact by crate
`cobolt-forms` (everything), `cobolt-compiler` / `cobolt-project-tools` / `cobolt-codegen` (property and method tables, System KB), `cobolt-ide` (designer), `cobolt-wasm` and `cobolt-web-player` (host parity). Mobile: none. `cobolt-semantic` has no per-control property table.
