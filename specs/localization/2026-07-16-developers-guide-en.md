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

## Glossary — do NOT translate

`PowerRustCOBOL`, product and menu names (e.g. **Font** as the property name,
*Settings*, **Help → About**), all COBOL keywords/identifiers, code samples,
font family names ("Arial"), and file names/paths. Never introduce the word
"cobolt" in any language.

## Status

- [ ] es  - [ ] pt  - [ ] jp  - [ ] cn  - [ ] fr (create from scratch — flagged)
