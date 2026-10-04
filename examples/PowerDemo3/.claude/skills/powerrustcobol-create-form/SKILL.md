---
name: powerrustcobol-create-form
description: "Create a new form (window) in this PowerRustCOBOL project: write its .cfrm, add it to the project, generate and check its COBOL."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-create-form

Create a new form (window) in this PowerRustCOBOL project: write its .cfrm, add it to the project, generate and check its COBOL.

## Steps

1. Choose a form name that is a COBOL word (`ORDER-FORM`) and a file name for it, `forms/<name>.cfrm`. Read `docs/powerrustcobol/cfrm-format.md` first.
2. Write the `.cfrm`: a `<Form name="…" title="…" width="…" height="…">` element, its `<working-storage>` (form-level items, `GLOBAL` when a handler uses them), its `<form-events>` (`onLoad`, `onClose`) and its `<Control>` elements. Leave `main-form` out unless the developer asked for a new main form — there is exactly one.
3. Call `validate` with the path: the file must load.
4. Call `add_to_project` with the path. Never add it to the manifest yourself.
5. Call `regenerate` with the path, then `check` with the path, and fix every error in the `.cfrm` (never in `generated/`) until `check` reports none.

## Tools

`validate` (`mcp__powerrustcobol-ide__validate`, or `mcp__powerrustcobol__validate` with the IDE closed), `add_to_project` (`mcp__powerrustcobol-ide__add_to_project`, or `mcp__powerrustcobol__add_to_project` with the IDE closed), `regenerate` (`mcp__powerrustcobol-ide__regenerate`, or `mcp__powerrustcobol__regenerate` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.
