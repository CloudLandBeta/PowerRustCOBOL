<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Documentation steering — PowerRustCOBOL

> Policy and the code↔document map for the `/docsync` phase (and `/doc-shots`,
> `/doc-localize`). Keep the registry below current — it is the source of truth
> for "which docs describe which code".

## Documents we maintain

| Document | Status | Audience |
|----------|--------|----------|
| `README.md` | current | repo landing page |
| `docs/developers-guide-en.md` | **canonical English** | developers using the IDE |
| `docs/developers-guide-{es,pt,jp,cn}.md` | translations — **user/external-maintained** | localized readers |
| `docs/cobol85-supported-syntax.md` | current | language reference |
| `docs/cobol85-verb-test-matrix.md` | current | verb coverage |
| `CHANGELOG.md` | current | release notes |
| `docs/compiler-manual.md` | **planned** | CLI / build deep-dive |

## Code ↔ document registry (traceability)

When code in a "Code areas" cell changes, **only** the matching documents/sections
are candidates for update. Sections use the doc's GitHub anchor.

| Document / section | Code areas (globs) | Topic |
|--------------------|--------------------|-------|
| `developers-guide-en.md#5-the-ide-at-a-glance` | `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/panels/**` | ide-ui |
| `…#7-the-form-designer-rad` | `crates/cobolt-ide/src/panels/designer.rs`, `crates/cobolt-forms/**` | designer |
| `…#8-the-widget-catalogue` | `crates/cobolt-forms/**` | widgets |
| `…#11-talking-to-the-ui-from-cobol` | `crates/cobolt-runtime/**` (dispatch), `crates/cobolt-codegen/**` | ui-calls |
| `…#12-generated-code` | `crates/cobolt-codegen/**` | codegen |
| `…#13-the-rustcobol-language` | `crates/cobolt-{lexer,parser,semantic,runtime,stdlib}/**` | language |
| `…#14-indexed-files--a-first-class-resource` | `crates/cobolt-runtime/**` (indexed engines) | indexed-files |
| `…#15-sql-databases` | `crates/cobolt-runtime/**` (sql), `crates/cobolt-stdlib/**` | sql |
| `…#16-http--rest-and-ai-agents` | `crates/cobolt-runtime/**` (rest), `crates/cobolt-ide/src/llm.rs` | rest-ai |
| `…#17-the-command-line-rcrun` | `crates/cobolt-cli/**` | cli-flags |
| `…#18-building-a-distributable-binary` | `crates/cobolt-compiler/**`, `crates/cobolt-cli/**` | build |
| `…#19-debugging` | `crates/cobolt-ide/src/panels/debugger.rs`, `crates/cobolt-runtime/**` | debug |
| `…#20-appearance-and-internationalisation` | `crates/cobolt-ide/src/i18n.rs`, `crates/cobolt-ide/src/fonts.rs` | i18n, theming |
| `cobol85-supported-syntax.md` | `crates/cobolt-{parser,semantic,runtime}/**` | language-support |
| `cobol85-verb-test-matrix.md` | `tests/cobol/**`, `crates/cobolt-runtime/**` | verb-tests |
| `compiler-manual.md` *(planned)* | `crates/cobolt-cli/**`, `crates/cobolt-compiler/**` | cli-flags, build |
| `README.md` | *(project overview — broad)* | overview |

> Maintenance rule: whenever a new document or major section is added, **add a row
> here** in the same change. A doc with no registry row is invisible to `/docsync`.

## Screenshot policy (`/doc-shots`)

- **Location:** `assets/images/screenshots/` (committed). **Names:** descriptive
  kebab-case matching the doc's placeholder, e.g. `ide-overview.png`,
  `project-settings-form.png`.
- **Placeholders:** docs mark needed images with a line like
  `> 📷 **Screenshot needed — \`name.png\`** …`. `/doc-shots` fills these.
- **Capture recipe (macOS):** run the IDE as a `.app` bundle; get the window id
  with a Swift `CGWindowListCopyWindowInfo` snippet; capture with
  `screencapture -x -o -l <window-id> <path>`. If `open` fails with launch error
  162, re-sign the bundle (`codesign --force --sign - <app>`). egui menus can't
  be clicked by automation on a bare binary, so navigation to a specific view may
  need the operator to drive while the skill captures.
- Reference an image from `docs/` as `../assets/images/screenshots/<name>.png`.

## Localization policy (`/doc-localize`)

- **Claude edits the English canonical only.** Translations
  (`developers-guide-{es,pt,jp,cn,fr}.md`) are produced/maintained **externally**
  (GOLDEN RULE #3) — **never spend Claude credits translating**; those are for
  building PowerRustCOBOL components.
- When the English doc changes, `/doc-localize` emits a **work order** under
  `specs/localization/` describing what changed (file, sections/anchors, English
  deltas) and the target languages, for an external/cheaper translation agent.
- Target languages: **es, pt, jp, cn** (existing) and **fr** (UI shipped; guide
  translation not yet created — flag for creation).
- **Glossary — keep untranslated:** `PowerRustCOBOL`, product/menu names, all
  COBOL keywords/identifiers and code samples. Never introduce "cobolt" in any
  language.
