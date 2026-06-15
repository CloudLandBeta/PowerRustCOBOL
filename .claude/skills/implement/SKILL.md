---
name: implement
description: Phase 4 of spec-driven development for PowerRustCOBOL. Execute the approved tasks.md task-by-task, building and testing each, and checking tasks off. Use when the user runs /implement after tasks are approved.
---

# /implement — build the feature

You are in **phase 4** of the spec-driven workflow (see `specs/README.md`).

## Steps

1. **Locate the active feature folder** with an approved `tasks.md`.
2. **Read** `tasks.md`, `plan.md`, `spec.md`, and `specs/steering/*.md`.
3. **Work task-by-task, in order:**
   - Implement the task.
   - Run its **verification** (the `cargo build` / `cargo test` it names).
   - If it passes, **check the box** in `tasks.md` (edit the file). If it fails,
     fix it; if blocked, stop and report — do not skip ahead silently.
4. After all tasks: run the **full** `cargo test`, update
   `docs/developers-guide-en.md` and the CHANGELOG/version if a feature, and
   confirm every acceptance criterion in `spec.md` is satisfied.
5. **Report** what shipped, test results (quantified, real numbers), and any
   manual/visual checks the user should do.

## Rules (honour the operator's golden rules)

- **i18n:** any new UI string is a `Tr` field in **all six** languages; never a
  literal. Run `cargo test -p cobolt-ide i18n`.
- **Generated COBOL:** preserve the developer banner and the regenerate-on-
  Build/Run/Debug/Check contract.
- **Docs:** update the **English** guide only; never touch the translations.
- **Tests:** report-or-fix user-provided tests; new tests report quantified
  results; **verify-first** (never claim an unmeasured result).
- **Commits/push:** do **NOT** commit or push unless the user explicitly asks.
  When asked, **never mix fixes and features** in one commit, respect the push
  window, and follow the forum-announcement rules — all per the operator's
  CLAUDE.md.
- If reality diverges from the plan, stop and surface it rather than forcing the
  plan through.
