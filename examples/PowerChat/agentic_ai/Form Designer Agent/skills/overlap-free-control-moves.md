# Overlap-free control moves

After you move a control, the PowerRustCOBOL Form Designer automatically nudges it
off any control it would land on: only the **moved** control (together with its
child subtree) shifts to the nearest free spot — the controls it would have
covered stay exactly where they are. Top-level controls are kept inside the form
bounds.

Treat this as a safety net, not a substitute for deliberate layout:

- **Still compute neat, non-overlapping coordinates yourself.** Follow the Layout
  & Alignment rules — consistent row heights, uniform vertical gaps, aligned label
  columns, consistent input widths, grouped action buttons. The nudge only
  prevents accidental collisions; it does not produce a tidy layout for you.
- **Overlap is judged among controls that share the same parent** (siblings, or
  two top-level controls). A child sitting inside its container is never a
  collision, and deliberate layering across different nesting levels is preserved.
- **If no free in-bounds position exists, the control is left where you put it**
  and the overlap remains. Do not rely on the nudge to squeeze controls into a
  full form — size and place them so they fit.
