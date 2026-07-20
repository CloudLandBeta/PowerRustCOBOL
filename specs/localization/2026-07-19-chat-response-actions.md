<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: chatbot response actions

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#the-ai-assistant-optional`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

Add the following chatbot behavior after the composer-layout paragraph:

- Completed agent-response balloons show icon-only Copy and Save as Markdown
  actions with hover tooltips.
- Save starts in the open project's `Documentation/` folder and rejects a
  destination outside that folder.
- The saved response always uses the `.md` extension, is indexed in the project
  SQLite knowledge database, and appears in the Documentation project-tree
  branch without reopening the project.
- Developer messages, static welcome text, and active streaming balloons do not
  show response actions.

## Translation rules

- Do not translate `PowerRustCOBOL`, `Documentation/`, `.md`, SQLite, paths, or
  code identifiers.
- Preserve Markdown structure and inline code.
- Translate only the changed section; do not rewrite unrelated content.
