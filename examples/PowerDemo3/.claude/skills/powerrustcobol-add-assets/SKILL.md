---
name: powerrustcobol-add-assets
description: "Add an image, icon, font, sound or data file to the project so forms and the built application can use it."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-add-assets

Add an image, icon, font, sound or data file to the project so forms and the built application can use it.

## Steps

1. Put the file under `Assets/` (a sub-folder is fine). Refer to it from a form or from COBOL by its project-relative path: assets are resolved against the application's folder, so the same path works under Run Form and in a built application.
2. Call `add_to_project` with its path. The list is chosen from the extension (images, fonts, sounds and data go to `assets`; `.md`, `.txt`, `.pdf` and `.html` to `documentation`); pass `list` only to override it.
3. Where a property takes the asset (an image or icon path), look the property up first with `kb_lookup`; then `regenerate` and `check` the form.

## Tools

`add_to_project` (`mcp__powerrustcobol-ide__add_to_project`, or `mcp__powerrustcobol__add_to_project` with the IDE closed), `kb_lookup` (`mcp__powerrustcobol-ide__kb_lookup`, or `mcp__powerrustcobol__kb_lookup` with the IDE closed), `regenerate` (`mcp__powerrustcobol-ide__regenerate`, or `mcp__powerrustcobol__regenerate` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.
