<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order — developers-guide-en.md (2026-07-16)

- **Source:** `docs/developers-guide-en.md` (English canonical, branch `egui-035`)
- **Targets:** `docs/developers-guide-es.md`, `-pt.md`, `-jp.md`, `-cn.md`
  (update in place). **French:** `developers-guide-fr.md` does **not** exist yet —
  the UI ships French, so flag for creation from the full English canonical
  (separate, larger job; not part of this delta).
- **Screenshots:** shared and language-neutral — no image work.

## Delta 1 — section `#20-appearance-and-internationalisation`

**Change:** one **new bullet** appended after the existing "IDE languages."
bullet (no other text in the section changed). English text to translate:

> - **Text rendering.** The IDE renders text with the UI framework's modern font
>   engine (hinting enabled), so glyphs are noticeably crisper at small sizes than
>   in earlier releases. Form **Font** properties keep working exactly as before:
>   a face the engine cannot rasterise (for example a bitmap-only system font)
>   is skipped and the control falls back to Arial instead of failing.

## Delta 2 — section `#16-http--rest-and-ai-agents`

**Change:** one **new subsection** "Driving the IDE with an AI agent (MCP)"
appended after the existing caveat (three paragraphs + two bullets + one
caveat block). Translate the whole subsection; keep `egui-mcp`,
`127.0.0.1:5719`, `rcrun`, menu names and *Settings* untranslated.

## Delta 3 — section `#5` AI-settings table

**Change:** the **API key** row's description gained a second sentence
(keys remembered per provider + model; field cleared when none stored).
Translate the extended cell; keep `Authorization: Bearer` untranslated.

## Delta 4 — section `#5` AI-settings table

**Change:** new table row **Reviewer model (Pedantic Agent)** describing the
optional second model and the tandem COBOL Proficiency review flow. Translate
the row; keep "Pedantic Agent", "COBOL Proficiency" (feature names) as-is per
product-name policy if your language keeps feature names in English; otherwise
translate consistently with the UI strings.

## Delta 5 — section `#5` AI settings

**Change:** new **Agent Manager** paragraph before the "Test connection"
paragraph (project agent database, `agentic_ai/<agent name>/` layout, unique
immutable names, per-model machine-local keys, pedantic companion rule,
seeding). Translate the paragraph; keep paths, file names, and "Agent
Manager" / "Pedantic" feature terms consistent with the UI strings.

## Glossary — do NOT translate

`PowerRustCOBOL`, product and menu names (e.g. **Font** as the property name,
*Settings*, **Help → About**), all COBOL keywords/identifiers, code samples,
font family names ("Arial"), and file names/paths. Never introduce the word
"cobolt" in any language.

## Status

- [ ] es  - [ ] pt  - [ ] jp  - [ ] cn  - [ ] fr (create from scratch — flagged)
