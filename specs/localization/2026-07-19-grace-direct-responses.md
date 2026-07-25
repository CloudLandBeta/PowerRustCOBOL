<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: Grace direct responses and indexed schema routing

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#the-ai-assistant-optional`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

- Explain that read-only questions and requests to describe, explain,
  summarize, compare, suggest, or recommend return Markdown directly and do not
  require workflow JSON.
- Explain that a mixed request containing a concrete project change still uses
  the structured workflow.
- Clarify that Documentation Agent schema preparation and normalization are
  analysis; only `indexed_file.write` or an explicit `.cidx` save is mutation.

## Translation rules

- Do not translate `PowerRustCOBOL`, canonical agent names, Markdown, JSON,
  `.cidx`, or `indexed_file.write`.
- Preserve Markdown structure and inline code.
- Translate only the changed section; do not rewrite unrelated content.
