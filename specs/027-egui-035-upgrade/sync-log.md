<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# egui-035 — sync & audit log

**Branch ownership (R7):** `egui-035` is maintained exclusively by the
dedicated upgrade agent (operator directive, 2026-07-15). Foreign commits are
reverted to the agent's latest commit — standing authorization. Every session
starts with: fetch → foreign-commit check → port pending `main` commits.

**Sync policy (R6):** every commit landing on `main` is replicated here as one
adapted commit (re-implemented against the current egui step's APIs, never a
blind cherry-pick), same session it is noticed. Rows are appended below.

## Sync table (main → egui-035)

| Date | main SHA | branch SHA | Adaptation notes |
|------|----------|------------|------------------|
| 2026-07-15 | d99dd7b (1.30.9) | d99dd7b | Baseline: fast-forward sync at branch start; identical trees, no adaptation needed. |

## Known pre-existing failures (inherited from main, NOT upgrade regressions)

- `panels::designer::sticky_font_tests::first_widget_keeps_default_font` —
  fails identically on main @ d99dd7b under egui 0.29 (verified in a clean
  worktree, 2026-07-15): expects default font size 10, code now yields 14
  (likely the 1.30.x "Update control defaults" change). Reported to the
  operator; fix belongs on main, then ports here per R6. Branch test gate =
  160/161 until then.

## Verification records

(AC5 dependency diffs, AC4 isolation checks, and AC6 final audit are appended
here by their tasks.)
