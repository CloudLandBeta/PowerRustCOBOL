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
| `docs/developers-guide-{es,pt,fr,jp,cn}.md` | translations — **frozen until the doc restructure** (operator ruling 2026-08-23, see Localization policy); **stale**: pt/jp/cn are English copies, es partial, fr missing | localized readers |
| `docs/{BENCHMARKS,BUILDING,database-runtime,observability,cobol85-supported-syntax}-{es,pt,fr,jp,cn}.md` | translations — current as of 2026-08-20, frozen with the rest | localized readers |
| `docs/cobol85-supported-syntax.md` | current | language reference |
| `docs/cobol85-verb-test-matrix.md` | current | verb coverage |
| `CHANGELOG.md` | current | release notes |
| `docs/compiler-manual.md` | **planned** | CLI / build deep-dive |

## Code ↔ document registry (traceability)

When code in a "Code areas" cell changes, **only** the matching documents/sections
are candidates for update. Sections use the doc's GitHub anchor.

| Document / section | Code areas (globs) | Topic |
|--------------------|--------------------|-------|
| `developers-guide-en.md#5-the-ide-at-a-glance` | `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/project_model.rs`, `crates/cobolt-ide/src/panels/**` | ide-ui |
| `…#indexed-file-editor--grid-browser` | `crates/cobolt-indexed/**`, `crates/cobolt-ide/src/panels/indexed_*.rs`, `crates/cobolt-codegen/src/indexed.rs` | indexed-editor |
| `…#7-the-form-designer-rad` | `crates/cobolt-ide/src/panels/designer.rs`, `crates/cobolt-forms/**` | designer |
| `…#8-the-control-catalogue` | `crates/cobolt-forms/**` | controls |
| `…#per-control-examples` + `examples/README.md` | `examples/**`, `crates/cobolt-codegen/examples/build_examples.rs` | control-examples |
| `…#11-talking-to-the-ui-from-cobol` | `crates/cobolt-runtime/**` (dispatch), `crates/cobolt-codegen/**` | ui-calls |
| `…#12-generated-code` | `crates/cobolt-codegen/**` | codegen |
| `…#13-the-rustcobol-language` | `crates/cobolt-{lexer,parser,semantic,runtime,stdlib}/**` | language |
| `…#14-indexed-files--a-first-class-resource` | `crates/cobolt-runtime/**` (indexed engines) | indexed-files |
| `…#15-sql-databases` | `crates/cobolt-runtime/**` (sql), `crates/cobolt-stdlib/**` | sql |
| `…#16-http--rest-and-ai-agents` | `crates/cobolt-runtime/**` (rest), `crates/cobolt-ide/src/llm.rs` | rest-ai |
| `…#17-the-command-line-rcrun` | `crates/cobolt-cli/**` | cli-flags |
| `…#18-building-a-distributable-binary` | `crates/cobolt-compiler/**`, `crates/cobolt-cli/**` | build |
| `…#19-debugging` | `crates/cobolt-ide/src/panels/debugger.rs`, `crates/cobolt-runtime/**` | debug |
| `…#20-appearance-and-internationalisation` | `crates/cobolt-ide/src/i18n.rs`, `crates/cobolt-ide/src/fonts.rs`, `crates/cobolt-forms/src/fonts.rs` | i18n, theming, font-pipeline |
| `…#21-cobol-structure-and-shared-data` | `crates/cobolt-ide/src/panels/cobol_structure.rs`, `crates/cobolt-forms/**`, `crates/cobolt-codegen/**`, `crates/cobolt-runtime/src/{environment,interpreter}.rs` | cobol-structure, shared-data |
| `…#22-the-application-shell-and-the-super-receiver` | `crates/cobolt-form-host/src/shell.rs`, `crates/cobolt-form-host/src/host.rs`, `crates/cobolt-runtime/src/{interpreter,form_host}.rs`, `crates/cobolt-semantic/src/resolver.rs`, `crates/cobolt-forms/src/{model,menu}.rs` | application-shell, super-receiver |
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

## Localization policy

> **Documentation is ENGLISH-ONLY until the doc restructure** (operator ruling,
> 2026-08-23): *"No reason to translate something that will change anyways.
> Just keep English up-to-date."* This **suspends GOLDEN RULE #8** for
> documents — keep `docs/*-en.md` (and unlabelled English docs) current, and
> **do not touch any translation file**. This settles the former three-way
> contradiction with `tech.md`/`structure.md` in their favour. A suspension
> with a reason, not a permanent reversal: expect translations to resume after
> the restructure, using the method preserved below.
>
> **Scope:** the ruling governs *documents only*. The IDE's user-facing
> **strings** remain `Tr` fields in all six languages (EN/ES/PT/JA/ZH/FR) —
> that is a `tech.md` hard constraint with its own completeness test, and it
> is unaffected.

The method, for when translation resumes:

- **English first, then the same delta into each translation.** The English file
  is the canonical text and the only one to reason about correctness in. Never
  translate from a translation.
- **Naming:** `<doc>-<lang>.md` beside the English file — `observability-fr.md`,
  `BUILDING-jp.md`. The English canonical keeps its own name.
- **Claude writes the translations directly.** `/doc-localize` is retained only
  for bulk work the operator explicitly asks to route outside.
- **Verify before claiming done:** each touched file passes
  `iconv -f UTF-8 -t UTF-8`, has zero double-encoded sequences, and carries no
  leftover English prose or characters from another script.
- Target languages: **es, pt, fr, jp, cn** (fr included — the IDE ships French).
- **Glossary — keep untranslated:** `PowerRustCOBOL`, product/menu names, all
  COBOL keywords/identifiers and code samples. Never introduce "cobolt" in any
  language.
