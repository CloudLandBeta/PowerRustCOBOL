---
name: plan
description: Phase 2 of spec-driven development for PowerRustCOBOL. Turn an approved spec.md into a design at specs/NNN-<slug>/plan.md. Use when the user runs /plan after a spec is approved.
---

# /plan — write the design

You are in **phase 2** of the spec-driven workflow (see `specs/README.md`).

## Steps

1. **Locate the active feature folder** — the `specs/NNN-<slug>/` that has a
   `spec.md` but no approved `plan.md` (ask the user if ambiguous).
2. **Read** that `spec.md` and all `specs/steering/*.md`.
3. **Explore the codebase** as needed to ground the design: identify the exact
   crates/files to touch (use `structure.md` as the map), existing patterns to
   reuse, and data/model/format impacts.
4. **Write `plan.md`** from `specs/templates/plan.md`: approach (referencing
   R1…Rn), affected crates/files, data/model changes, key decisions (with
   rejected alternatives), risks + mitigations, **test strategy** (which tests,
   what they assert and report; manual/visual checks), and the steering-compliance
   checklist.
5. **Gate.** Summarise the design and the path; tell the user to review and run
   **`/tasks`**. Do **not** write tasks or code yet.

## Rules

- Respect every hard constraint in `tech.md` (i18n ×6, generated-code banner +
  regenerate-on-action, English-guide-only docs, versioning, fix/feature split).
- Prefer reusing existing modules/patterns over new ones; call them out.
- Surface real risks honestly; if the spec has unresolved questions, resolve or
  flag them before finishing.
- No code changes in this phase.
