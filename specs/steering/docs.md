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
| `docs/<doc>-{es,pt,fr,jp,cn}.md` | translations — **regenerated whole on every minor/major, deleted whenever their English changes** (operator ruling 2026-08-24, see Localization policy). Never patched, never partially updated. | localized readers |
| `docs/cobol85-supported-syntax-en.md` | current | language reference **and the NIST CCVS85 conformance scoreboard** — the PASS/FAIL/N-A table is the document's headline and must be re-measured (`cargo run -p cobolt-semantic --example nist_conformance -- strict`) and updated whenever a `specs/nist/` fix lands |
| `docs/cobol85-verb-test-matrix-en.md` | current | verb coverage |
| `CHANGELOG.md` | current (English only — release notes are not translated) | release notes |
| `docs/compiler-manual-en.md` | **planned** | CLI / build deep-dive |

> **Every English document carries `-en`** since 1.62.0. The other eight English
> docs — `BENCHMARKS`, `BUILDING`, `DEPENDENCIES`, `database-runtime`,
> `ide-collaboration-design`, `indexed-file-format`, `indexed-file-internals`,
> `indexed-redb-engine`, `observability` — follow the same `-en` naming and the
> same translation cycle.

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
| `BUILDING-en.md#installing-the-ide-elsewhere--ship-the-platform-sdk` | `crates/cobolt-compiler/src/lib.rs` (`SDK_CRATES`, `stage_sdk`, `resolve_workspace_root`), `crates/cobolt-compiler/examples/stage_sdk.rs` | platform-sdk |
| `cobol85-supported-syntax-en.md` | `crates/cobolt-{lexer,parser,semantic,runtime}/**`, `crates/cobolt-semantic/examples/nist_conformance.rs`, `specs/nist/**` | language-support, nist-conformance |
| `cobol85-verb-test-matrix-en.md` | `tests/cobol/**`, `crates/cobolt-runtime/**` | verb-tests |
| `compiler-manual-en.md` *(planned)* | `crates/cobolt-cli/**`, `crates/cobolt-compiler/**` | cli-flags, build |
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

## Localization policy — the regeneration cycle

> **Operator ruling, 2026-08-24.** Translations are no longer *patched*. They
> are **discarded and regenerated whole** from the English canonical. This
> replaces both the delta-into-every-language method and the 2026-08-23
> English-only suspension; `CLAUDE.md` GOLDEN RULE #8 carries the same text.
>
> **Scope:** documents only. The IDE's user-facing **strings** remain `Tr`
> fields in all six languages (EN/ES/PT/JA/ZH/FR) — a `tech.md` hard constraint
> with its own completeness test, unaffected by this.

**When the cycle runs.** Only on a **major or minor** version bump (`x`/`y`,
which only the operator raises). **Never on a fix** (`z`) — too expensive to run
per change.

**On any change that touches a document.** Update the **English canonical
only**, then **physically delete that document's five translations**. A stale
translation is worse than a missing one; the next minor/major rebuilds them all.
**No English file is ever deleted.**

**Naming.** Every English document carries `-en` (`observability-en.md`). A
document still lacking the suffix is renamed before its first translation, and
`README.md`, cross-document links, Rust doc comments and the IDE Help tests are
fixed in the same change. `docs_embed.rs::split_lang` already treats a
suffix-less name and `-en` alike, so the resolver itself needs no change.

**Per English document:**

| Size | Procedure |
|------|-----------|
| ≤ 32 KB | Translate the whole file directly into `<doc>-<lang>.md`. |
| > 32 KB | Split by ToC entry → `temp-<doc>-en.md` (head: title, intro, ToC) + one `temp-<doc>-<section>-en.md` per entry. Translate each into every language, `cat` them back **in original order** into `<doc>-<lang>.md`, delete every `temp-*`. |
| > 32 KB, no ToC | Add a ToC to the English canonical first, then split. |
| ToC entry itself > 32 KB | Subdivide that entry further. |

**Never split mid-context** — a paragraph, a markdown table, a fenced code block
and a mermaid block each stay whole in one temp file. Cut only at headings.

**Temp files** live in `docs/`, are **never committed**, and are **never
reused**. Recover an interrupted run by deleting every `temp-*` and starting
over — never by resuming.

**Links.** Translate section headings, regenerate the ToC anchors from the
*translated* headings, and repoint cross-document links at the same-language
file. One English file in → exactly one file per language out, six total.

- **Target languages:** **es, pt, fr, jp, cn** (fr included — the IDE ships French).
- **Never machine-copy English** into a translation file to make it "exist".
- **Verify before claiming done:** each generated file passes
  `iconv -f UTF-8 -t UTF-8`, has zero double-encoded sequences, and carries no
  leftover English prose or characters from another script.
- **Glossary — keep untranslated:** `PowerRustCOBOL`, product/menu names, all
  COBOL keywords/identifiers and code samples. Never introduce "cobolt" in any
  language.
- **Measured expansion** (this repo's own complete translations, 2026-08-24):
  es +5–13 %, pt +5–12 %, fr +9–18 %, cn +1–5 %, **jp +17–40 %** in bytes. Size
  splits against the Japanese worst case.
