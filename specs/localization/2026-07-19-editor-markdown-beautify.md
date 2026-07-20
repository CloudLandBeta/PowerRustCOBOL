<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: Markdown editor status bar

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#5-the-ide-at-a-glance`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

Clarify that the code editor's **Beautify** command is shown only for
non-Markdown documents. `.md` and `.markdown` tabs omit it because the command
applies COBOL formatting rules.

## Translation rules

- Do not translate `PowerRustCOBOL`, `Beautify`, `.md`, `.markdown`, or code
  identifiers.
- Preserve Markdown structure and inline code.
- Translate only the changed paragraph.
