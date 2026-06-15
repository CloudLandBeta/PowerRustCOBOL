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
    fonts.rs             font loading (base_font_definitions: Latin + CJK)
    main.rs              eframe entry, window icon
    panels/              editor, designer, properties, project, doc_viewer,
                         settings_form, toolbar, output, debugger, md_render
docs/
  developers-guide-en.md Canonical English guide (keep current)
  developers-guide-*.md  Translations — USER-MAINTAINED, do not edit
assets/images/           Mascot, icon, banners, backgrounds
specs/                   Spec-driven development (this tree)
  steering/              product.md · tech.md · structure.md
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
- Branches follow the operator's convention: `feat/<slug>` and `fix/<slug>`,
  merged `--no-ff` into `main`.
