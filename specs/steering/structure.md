<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Structure steering — PowerRustCOBOL

> Where things live, so specs/plans place changes correctly.

## Repository layout

```
crates/                  Rust workspace members (see tech.md table)
  cobolt-ide/src/
    app.rs               IDE app: menus, actions, modals, project glue
    i18n.rs              Tr table + Language enum (6 languages)
    fonts.rs             one-line re-export of cobolt-forms::fonts
                         (base_font_definitions lives in cobolt-forms)
    main.rs              eframe entry, window icon
    panels/              editor, designer, properties, project, doc_viewer,
                         settings_form, toolbar, output, debugger, md_render
docs/
  developers-guide-en.md Canonical English guide (keep current)
  developers-guide-*.md  Translations — USER-MAINTAINED, do not edit
assets/images/           Mascot, icon, banners, backgrounds
specs/                   Spec-driven development (this tree)
  steering/              product.md · tech.md · structure.md · docs.md ·
                         doc-style.md
  templates/             spec.md · plan.md · tasks.md
  NNN-<slug>/            One folder per feature: spec.md → plan.md → tasks.md
CHANGELOG.md             Per-release notes (bump with features)
```

## Where new work goes

- **New IDE UI string** → add a `Tr` field in `i18n.rs` with all six languages;
  reference it from the panel (never a literal).
- **New IDE panel/feature** → `crates/cobolt-ide/src/panels/` + wire in `app.rs`.
- **Language/runtime feature** → the relevant `cobolt-*` crate + tests in that
  crate; document standard support in `docs/developers-guide-en.md`.
- **Form/codegen change** → `cobolt-forms` (model) and/or `cobolt-codegen`
  (generator); keep the generated banner and regenerate-on-action contract.
- **User-facing docs** → `docs/developers-guide-en.md` (English only).
- **Assets** → `assets/images/`.

## Naming

- Feature spec folders: `specs/NNN-<kebab-slug>/` (NNN = zero-padded, next free).
  A number may be skipped: `008` was never used, and `064` was left as a gap
  when that work was reclassified as a fix (fixes get no spec folder). So "next
  free" means the next unused integer, never the count of folders.
- **Branches — GOLDEN RULE #5.** `CONVENTIONS.md` §Git is authoritative; this is
  the short form. Two long-lived working branches carry all work: **`features`**
  for new functionality, **`fixes`** for corrections. Classify the request
  *before* the first edit, check the matching branch out, and sync from `main`
  straight after the switch:
  - `git rebase main` normally — and only **before the first commit**, since
    rebasing published history needs a force-push that can destroy another
    session's work on a shared branch;
  - `git merge --ff-only main` when the shared tree is dirty, which `rebase`
    refuses. Equivalent whenever `main` is an ancestor.
  - **Never plain `git merge main`** (operator ruling 2026-09-14): the remote
    refuses merge commits on a working branch, and that is the form that makes
    one.

  `main` is never a workbench, and merging back into it happens **only when
  explicitly asked**. Committing and pushing a working branch needs no such
  request.
- **When another worktree holds the branch** — the normal state here, since
  sessions run in `.claude/worktrees/` — git refuses to check it out, and
  refuses to move its ref too. Work on a per-change branch off `main` named for
  the kind of change (`fixes-1.70.142` is one), then land it with
  `git push origin HEAD:fixes`. Never force a shared branch.
