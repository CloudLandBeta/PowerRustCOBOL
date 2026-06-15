<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec-driven development

PowerRustCOBOL features are built spec-first, in gated phases (four core, plus
two optional hardening phases). Each phase is
a Claude Code skill (slash command) and produces one document; **you approve each
document before the next phase begins.**

| Phase | Command | Produces | Gate |
|-------|---------|----------|------|
| 1 | `/specify <idea>` | `specs/NNN-<slug>/spec.md` (requirements) | approve spec |
| 1.5 *(optional)* | `/clarify` | refined `spec.md` (unknowns resolved) | approve spec |
| 2 | `/plan` | `…/plan.md` (design) | approve plan |
| 3 | `/tasks` | `…/tasks.md` (ordered tasks) | approve tasks |
| 3.5 *(optional)* | `/analyze` | a findings report (read-only audit) | fix gaps, if any |
| 4 | `/implement` | code + tests, tasks checked off | review diff |
| 5 | `/docsync` | docs updated to match the code | review docs |

The optional phases harden the artifacts at the two riskiest seams:

- **`/clarify`** (after `/specify`) interrogates the spec for ambiguity, missing
  cases, and contradictions, asks you targeted questions, and folds the answers
  back in — so the design isn't built on guesses.
- **`/analyze`** (after `/tasks`) is a read-only cross-check of spec ↔ plan ↔
  tasks (and steering): coverage (every requirement has a task), traceability
  (no orphan tasks), consistency, and constraint compliance. It reports gaps for
  you to fix before any code is written.

### Documentation phase (`/docsync`)

`/docsync` keeps the docs in step with the code via the **code↔document registry**
in `specs/steering/docs.md`: a code change updates only the documents/sections that
describe it (e.g. changing the compiler's CLI flags touches the CLI sections and a
future compiler manual — nothing else). It has two helpers:

- **`/doc-shots`** — captures and inserts screenshots (fills the
  `📷 Screenshot needed` placeholders; refreshes images after UI changes).
- **`/doc-localize`** — emits **localization work orders** (under
  `specs/localization/`) so an **external/cheaper agent** translates; Claude never
  spends credits translating, and never edits the translation files (Rule #3).

## Steering (always read first)

`specs/steering/` grounds every phase:
- **product.md** — what/why, goals, non-goals, branding.
- **tech.md** — stack, conventions, hard constraints (i18n ×6, generated-code
  contract, versioning, commits).
- **structure.md** — repo layout and where new work goes.
- **docs.md** — the documentation policy + the code↔document registry (used by
  `/docsync`, `/doc-shots`, `/doc-localize`).
- **doc-style.md** — how the docs read: text style (voice, paragraphs, bullets,
  headings, colour) plus diagram, code-sample, callout and table conventions.

## Conventions

- One folder per feature: `specs/NNN-<kebab-slug>/` (NNN = next free, zero-padded).
- Templates live in `specs/templates/`.
- Phases are sequential; don't skip a gate. Re-run a phase to revise.
- `/implement` honours the operator's golden rules (fix/feature split, i18n,
  push window, forum announcements) and **does not commit or push unless asked**.

## The skills

The slash commands are project skills under `.claude/skills/`: the six workflow
phases (`specify`, `clarify`, `plan`, `tasks`, `analyze`, `implement`), the
documentation phase (`docsync`), and its helpers (`doc-shots`, `doc-localize`).
They are committed and shared: `.gitignore` keeps the rest of `.claude/` local but
includes `.claude/skills/` via a `!.claude/skills/` exception.
