---
name: specify
description: Phase 1 of spec-driven development for PowerRustCOBOL. Turn a feature idea into a requirements spec at specs/NNN-<slug>/spec.md. Use when the user runs /specify or asks to start/spec a new feature.
---

# /specify — write the requirements spec

You are starting **phase 1** of the spec-driven workflow (see `specs/README.md`).

## Steps

1. **Read steering** — `specs/steering/product.md`, `tech.md`, `structure.md`.
   The spec must be consistent with them.
2. **Understand the request.** If the feature idea is ambiguous or under-specified,
   ask the user 1–3 focused clarifying questions **before** writing. Do not invent
   scope.
3. **Pick the folder.** Find the next free zero-padded number: scan `specs/` for
   existing `NNN-*` folders and use the next integer. Slug = kebab-case of the
   feature name. Create `specs/NNN-<slug>/`.
4. **Write `spec.md`** from `specs/templates/spec.md`. Fill: overview, goals/
   non-goals, user stories, **EARS requirements** (numbered R1…Rn), acceptance
   criteria (checkable), the steering-compliance check (i18n ×6? generated-code
   contract? English-guide doc update? fix-vs-feature?), and open questions.
5. **Resolve open questions** with the user where you can; leave the rest listed.
6. **Gate.** Present a short summary and the path. Tell the user to review
   `spec.md` and, when satisfied, run **`/plan`**. Do **not** start design or code.

## Rules

- Requirements describe **what/why**, not implementation (that's `/plan`).
- Honour `tech.md` hard constraints in the spec's constraints section.
- Keep it tight and testable; every acceptance criterion must be verifiable.
- Do not modify code in this phase.
