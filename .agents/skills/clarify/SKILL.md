---
name: clarify
description: Optional phase between /specify and /plan in PowerRustCOBOL spec-driven development. Interrogate spec.md for ambiguity, gaps, and contradictions, ask the user targeted questions, and fold the answers back into the spec. Use when the user runs /clarify, or before /plan when a spec has unresolved unknowns.
---

# /clarify — tighten the spec before designing

Optional **phase 1.5** of the spec-driven workflow (see `specs/README.md`): run
after `/specify` and before `/plan`, so the design is grounded rather than guessed.

## Steps

1. **Locate the active feature folder** — the `specs/NNN-<slug>/` with a `spec.md`
   that has not yet been planned (ask the user if ambiguous).
2. **Read** that `spec.md` and `specs/steering/*.md`.
3. **Hunt for under-specification.** Scan every requirement and acceptance
   criterion for:
   - **Vague terms** — "fast", "large", "soon", "user-friendly" with no number.
   - **Missing cases** — empty/zero/limit inputs, error and failure behaviour,
     concurrency, defaults, persistence, undo.
   - **Untestable acceptance criteria** — can't be checked objectively.
   - **Contradictions** — requirements that conflict with each other or with the
     steering constraints (`tech.md`).
   - **Unstated scope edges** — what is explicitly *out*.
4. **Ask the user** a small, focused batch of questions (prefer a multi-select /
   structured ask over open prose). Only ask what actually blocks a sound design.
5. **Fold answers back into `spec.md`** — refine the requirements/acceptance
   criteria, clear the "Open questions" section, and note any new non-goals.
6. **Gate.** Summarise what changed and tell the user to review the updated
   `spec.md` and run **`/plan`**.

## Rules

- Resolve unknowns; do **not** start design or write code here.
- Don't invent answers — if the user can't decide, record the assumption
  explicitly in the spec so it's visible.
- Keep the question batch short; group related questions.
