---
name: analyze
description: Optional phase between /tasks and /implement in PowerRustCOBOL spec-driven development. Read-only cross-check of spec.md ↔ plan.md ↔ tasks.md (and the steering constraints) for coverage, traceability, consistency, and constraint compliance. Use when the user runs /analyze, or before /implement to catch gaps and contradictions.
---

# /analyze — cross-check before coding

Optional **phase 3.5** of the spec-driven workflow (see `specs/README.md`): run
after `/tasks` and before `/implement`. This is a **read-only audit** — it writes
no code and changes no spec/plan/tasks; it reports findings for you to act on.

## Steps

1. **Locate the active feature folder** with `spec.md`, `plan.md`, and `tasks.md`.
2. **Read** all three plus `specs/steering/*.md`.
3. **Audit** across the three documents:
   - **Coverage** — every requirement (R1…Rn) in `spec.md` is addressed by the
     design in `plan.md` **and** by at least one task in `tasks.md`.
   - **Traceability** — no orphan tasks (each task ties to a requirement); every
     acceptance criterion in `spec.md` is exercised by some task's verification.
   - **Consistency** — `plan.md` and `tasks.md` do not contradict `spec.md` or
     each other (no diverged names, scope, or behaviour).
   - **Constraint compliance** — nothing conflicts with `specs/steering/tech.md`
     (i18n in 6 languages, generated-code banner + regenerate-on-action, English
     guide only, fix/feature split, no "cobolt" in user-facing text, etc.).
   - **Sequencing/risk** — task order keeps the build green; high-risk items
     are surfaced.
4. **Report** a concise findings table: each issue with severity
   (blocker / should-fix / nit), where it is (which doc + R-id/T-id), and the
   suggested remedy (usually "re-run `/specify`, `/clarify`, `/plan`, or
   `/tasks`").
5. **Gate.** If clean, tell the user it's ready for **`/implement`**. If not,
   recommend the specific earlier phase to re-run.

## Rules

- **Do not modify** spec/plan/tasks or any code — analysis only.
- Be specific: cite the requirement/task ids, not vague concerns.
- Distinguish real gaps from style nits so the user can triage.
