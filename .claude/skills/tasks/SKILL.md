---
name: tasks
description: Phase 3 of spec-driven development for PowerRustCOBOL. Turn an approved plan.md into an ordered, verifiable task list at specs/NNN-<slug>/tasks.md. Use when the user runs /tasks after a plan is approved.
---

# /tasks — break the plan into tasks

You are in **phase 3** of the spec-driven workflow (see `specs/README.md`).

## Steps

1. **Locate the active feature folder** with an approved `plan.md`.
2. **Read** `plan.md`, `spec.md`, and `specs/steering/*.md`.
3. **Write `tasks.md`** from `specs/templates/tasks.md`: a sequence of **small,
   ordered, independently-verifiable** tasks. For each task give:
   - the files it touches,
   - the requirement(s) it satisfies (R1…Rn),
   - concrete **verification** (the exact `cargo build`/`cargo test` invocation
     and any observable check).
   Include explicit tasks for **docs** (`developers-guide-en.md`), **i18n** (new
   `Tr` keys ×6 languages), and a **finalize** task (version/CHANGELOG if a
   feature; full test run; manual launch check).
4. Order so the project stays green after each task where possible.
5. **Gate.** Summarise and tell the user to review and run **`/implement`**.

## Rules

- Tasks are the unit of execution and review — keep each one reviewable.
- Every acceptance criterion in `spec.md` must be covered by at least one task's
  verification.
- Do not implement anything in this phase.
