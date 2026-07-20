<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: Project Knowledge Base

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed sections: `#the-ide-at-a-glance`, `#projects-and-the-project-model`,
  and the AI assistant guidance
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

- Rename the project-owned `Documentation/` category and folder to
  `Knowledge Base/`.
- Explain automatic, conflict-preserving migration from legacy project
  `Documentation/` and `docs/` folders.
- Document recursive Knowledge Base folder display, creation, and confirmed
  deletion with manifest and vector-index cleanup.
- Explain that Grace searches the project Knowledge Base before every request,
  prefers relevant project evidence over model training, cites evidence paths,
  and does not invent missing project facts.

## Translation rules

- Do not translate `PowerRustCOBOL`, canonical agent names, SQLite, Markdown,
  `Knowledge Base/`, `Documentation/`, `docs/`, or code identifiers.
- Preserve Markdown structure, inline code, and the folder tree.
- Translate only the changed sections; do not rewrite unrelated content.
