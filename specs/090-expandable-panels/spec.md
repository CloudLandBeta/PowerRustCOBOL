# Spec — Expandable Panel and GroupBox

- **Status:** approved (the operator asked for it in chat, 2026-10-08, and clarified the icon and the methods)
- **Folder:** specs/090-expandable-panels/
- **Author:** Claude (for the operator)   **Date:** 2026-10-08

## 1. Overview
A dashboard is a grid of cards, and a card is often worth the whole screen: a
chart read at a glance wants a closer look. A Panel or GroupBox can now be
**expanded** to take the horizontal and vertical room of the other panels in its
container, and **collapsed** back to its cell. The operator turns it on with a
property, which puts the standard expand/collapse icon at the card's top-right
corner; the program drives it with two methods.

## 2. Goals / Non-goals
- **Goals:** one Boolean property to offer the icon (`Expandable`); one to hold the
  state (`Expanded`); `Expand()` and `Collapse()` methods; the same result in the
  designer canvas, the preview, the running form and a built application.
- **Non-goals:** an animation of the transition; an event; expanding in a form
  that is not responsive (the layout engine is what places the cards, see §7);
  expanding a control that is not a Panel or a GroupBox.

## 3. User stories
- As a developer of a dashboard, I tick `Expandable` on a card so my user can open
  it full size, with no code.
- As a developer, I call `INVOKE CARD-1::Expand()` from a button or a menu, and
  `INVOKE CARD-1::Collapse()` to give the room back.

## 4. Requirements (EARS)
- **R1 (ubiquitous):** Panel and GroupBox shall carry `Expandable` (Boolean,
  default off) and `Expanded` (Boolean, default off).
- **R2 (state):** While `Expandable` is on, the control shall paint the standard
  expand icon (two arrows pointing apart) at its top-right corner when collapsed,
  and the collapse icon (two arrows pointing together) when expanded.
- **R3 (event):** When the operator clicks the icon of a running form, `Expanded`
  shall flip, and the click shall not be reported as a click on the card.
- **R4 (state):** While a control is expanded, it shall occupy the whole client
  area of its container (what its siblings together occupied), above them, and
  its siblings — with everything inside them — shall not be drawn or answer input.
- **R5 (state):** While a control is expanded, the controls inside it shall be laid
  out for its new size by their own anchors, docks and layout mode, exactly as for
  any other size change.
- **R6 (event):** When a program calls `Expand()` or `Collapse()`, or writes
  `Expanded`, the next frame shall show the new state, whether or not
  `Expandable` is on.
- **R7 (optional):** Where several siblings are expanded at once, the one highest
  in z-order wins.
- **R8 (constraint):** The system shall not draw the icon while the form is not
  laid out (a form that is not responsive), because nothing would happen on click.

## 5. Acceptance criteria
- [ ] AC1 — a card with `Expanded` on lays out at its container's client rect and
      every sibling is hidden; with it off, everything is back (layout unit test).
- [ ] AC2 — the controls inside the expanded card follow its new size (layout test).
- [ ] AC3 — the icon shows with `Expandable` on, in the right corner, in the right
      state, and not otherwise (render test).
- [ ] AC4 — clicking the icon of a running form flips `Expanded`; a program's
      `Expand()` / `Collapse()` do the same (host test).
- [ ] AC5 — `Expandable` / `Expanded` are saved and loaded (.cfrm round trip),
      listed in the Properties pane with help in six languages, and documented in
      the System KB and the Developer's Guide.

## 6. Constraints & steering check
- **i18n:** property help text in all six languages (`prop_help_data.rs`). The icon
  has no text.
- **Generated code:** none; the methods are interpreter built-ins on a control.
- **Docs:** English Developer's Guide updated; translations are absent already.
- **Fix vs feature:** a feature — a capability beyond the COBOL-85 standard.
  Branch `features-expandable-panels`.

## 7. Open questions (settled by assumption, reversible)
- *Where does an expanded card get its room?* From its parent's client rectangle (a
  grid's whole area including the gaps; the form's, for a top-level card). Chosen
  because "the room of the other panels" is exactly that.
- *Why only responsive forms?* The layout engine is the one place that knows where
  every card sits; a form that is not laid out keeps its designed rectangles.
- *No event?* Kept out as nobody asked; `Expanded` can be read at any time.
