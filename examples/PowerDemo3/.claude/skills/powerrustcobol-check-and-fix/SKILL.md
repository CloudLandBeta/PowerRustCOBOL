---
name: powerrustcobol-check-and-fix
description: "Check the project against the real compiler and fix what it reports; run it before calling any change done, and before building."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-check-and-fix

Check the project against the real compiler and fix what it reports; run it before calling any change done, and before building.

## Steps

1. Prefer the `powerrustcobol-ide` server; use `powerrustcobol` only when the IDE is closed. Every answer is JSON: read it, do not guess from it.
2. Call `regenerate` for every `.cfrm` or `.cidx` you changed (or with no path for all of them).
3. Call `check` (no path = the whole project). For each error, open the file it names — for a form, the `.cfrm`, at the control ▸ event and the line inside that handler — and fix it there. A diagnostic marked as generated code points at how a property or handler is set, never at a line to edit in `generated/`.
4. Repeat until `check` reports no error. Warnings: fix the ones your change caused.
5. Only when asked for a binary, call `build`. It refuses while `check` has errors. If it answers `running`, call `build` again: it keeps waiting on the same build.
6. If an error comes from something the product does not support, stop changing code around it and use the gap-report skill.

## Tools

`regenerate` (`mcp__powerrustcobol-ide__regenerate`, or `mcp__powerrustcobol__regenerate` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed), `build` (`mcp__powerrustcobol-ide__build`, or `mcp__powerrustcobol__build` with the IDE closed), `list_files` (`mcp__powerrustcobol-ide__list_files`, or `mcp__powerrustcobol__list_files` with the IDE closed).

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.
