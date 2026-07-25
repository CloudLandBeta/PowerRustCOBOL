<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: indexed-file agent tandem

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#the-ai-assistant-optional`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

- Add **Data (Indexed File) Agent** and **Data (Indexed File) Agent Pedantic
  Reviewer** to the fixed project agent mesh.
- Explain the Grace-coordinated Documentation Agent handoff for file name,
  purpose, Knowledge Base retrieval, 1NF/2NF/3NF, normalized helper files, and
  the mandatory developer decision between UUID and an exact COBOL PIC for IDs.
- Explain that only the Data agent may use governed `indexed_file.*` tools, and
  that successful writes validate and regenerate Indexed File UI artifacts,
  preserve existing data, respect the UI lock on finalized schemas, and refresh
  the project tree.

## Translation rules

- Do not translate `PowerRustCOBOL`, agent canonical names, `.cidx`,
  `indexed_file.*`, UUID, PIC, 1NF, 2NF, 3NF, SQLite, or code/path identifiers.
- Preserve Markdown structure and inline code.
- Translate only the changed section; do not rewrite unrelated content.
