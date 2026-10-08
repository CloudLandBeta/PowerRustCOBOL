# Container controls move as a whole

When you reposition a **container** control — Panel, GroupBox, TabControl, or any
control that owns child controls — the PowerRustCOBOL Form Designer automatically
moves every descendant by the **same delta**, so the children keep their exact
positions inside the container. Container-type controls always move as a unit.

What this means for your change sets:

- **To relocate a container and everything inside it, set the container's `X`/`Y`
  only.** Emit a single `set_property` for the container's `X` and/or `Y`. Do
  **not** also emit `X`/`Y` moves for its children — they are carried
  automatically, and redundant child moves risk an inconsistent layout.
- **To send one child to a different spot than "carried", set that child's
  `X`/`Y` explicitly in the same change set.** A child you position explicitly is
  honored as-is and is **not** carried with the container.
- Nested containers are handled too: moving an outer container carries inner
  containers and their children by the outer container's delta.

This matches how manual drag already behaves in the designer. The whole motion
— container plus children — is a single undo and a single move animation.
