---
name: doc-shots
description: Capture and insert screenshots into PowerRustCOBOL's English documentation. Fills "📷 Screenshot needed" placeholders and refreshes images invalidated by UI changes. Use when the user runs /doc-shots or when /docsync flags screenshots.
---

# /doc-shots — capture & insert documentation screenshots

Helper of the `/docsync` documentation phase (see `specs/steering/docs.md` →
Screenshot policy).

## Steps

1. **Find what's needed.** Scan the English docs for `📷 Screenshot needed —
   \`name.png\`` placeholders, plus any images `/docsync` flagged as stale. Build a
   shot list: each item = file name + the exact IDE view/state to show.
2. **Run the IDE.** Build it (`cargo build -p cobolt-ide`) and launch the `.app`
   bundle (re-sign with `codesign --force --sign - <app>` if `open` fails with
   launch error 162). For a fresh build, copy the new binary into the bundle first.
3. **Capture each shot.** Get the target window id with a Swift
   `CGWindowListCopyWindowInfo` snippet, then
   `screencapture -x -o -l <window-id> assets/images/screenshots/<name>.png`.
   - egui menus can't be clicked by automation on the bare binary; if a shot needs
     a specific menu/dialog open, **ask the operator to drive to that view**, then
     capture. Confirm each capture by reading the PNG back.
4. **Insert into the doc.** Replace the placeholder line with a centered image:
   `<p align="center"><img src="../assets/images/screenshots/<name>.png" alt="…" width="…"></p>`
   (path is relative to `docs/`). Keep the alt text descriptive.
5. **Report** which shots were captured/inserted and any still pending operator
   help.

## Rules

- **English docs only** (GOLDEN RULE #3). Screenshots are language-neutral, so
  translations can reference the same files.
- Save to `assets/images/screenshots/` with the placeholder's exact name.
- Don't commit/push unless asked.
