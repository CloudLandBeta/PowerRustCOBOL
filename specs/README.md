<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec-driven development

PowerRustCOBOL features are built spec-first, in four gated phases. Each phase is
a Claude Code skill (slash command) and produces one document; **you approve each
document before the next phase begins.**

| Phase | Command | Produces | Gate |
|-------|---------|----------|------|
| 1 | `/specify <idea>` | `specs/NNN-<slug>/spec.md` (requirements) | approve spec |
| 2 | `/plan` | `…/plan.md` (design) | approve plan |
| 3 | `/tasks` | `…/tasks.md` (ordered tasks) | approve tasks |
| 4 | `/implement` | code + tests, tasks checked off | review diff |

## Steering (always read first)

`specs/steering/` grounds every phase:
- **product.md** — what/why, goals, non-goals, branding.
- **tech.md** — stack, conventions, hard constraints (i18n ×6, generated-code
  contract, versioning, commits).
- **structure.md** — repo layout and where new work goes.

## Conventions

- One folder per feature: `specs/NNN-<kebab-slug>/` (NNN = next free, zero-padded).
- Templates live in `specs/templates/`.
- Phases are sequential; don't skip a gate. Re-run a phase to revise.
- `/implement` honours the operator's golden rules (fix/feature split, i18n,
  push window, forum announcements) and **does not commit or push unless asked**.

## The skills

The slash commands are project skills under `.claude/skills/` (note: `.claude/`
is git-ignored in this repo, so the skills are local to your machine; the specs
and steering here are committed and shared).
