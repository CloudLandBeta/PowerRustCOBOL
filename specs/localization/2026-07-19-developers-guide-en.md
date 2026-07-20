<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: provisioned Pedantic reviewer tandems

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#the-ai-assistant-optional`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update French (`fr`) from the canonical English source.

## English delta

Update the Agents Manager section with these rules:

- Every project is provisioned with Grace, Form Designer Agent, COBOL Event
  Handler Script Agent, Documentation Agent, and Version Control Agent.
- Each primary is created and linked one-to-one with a purpose-specific
  Pedantic reviewer. The canonical reviewer name is always the complete primary
  name followed by ` Pedantic Reviewer`.
- The five reviewer names are `Grace Pedantic Reviewer`, `Form Designer Agent
  Pedantic Reviewer`, `COBOL Event Handler Script Agent Pedantic Reviewer`,
  `Documentation Agent Pedantic Reviewer`, and `Version Control Agent Pedantic
  Reviewer`.
- Reviewer prompts, descriptions, routing, and companion links are supplied by
  the project template. The developer chooses each reviewer model and may edit
  prompts, skills, tools, and knowledge.
- Opening an existing project recreates and relinks a missing built-in reviewer
  without replacing developer-customized configuration. Legacy reviewer names
  migrate in place with stable IDs and model profiles.
- New Agent and Delete Agent are hidden while the complete mesh is provisioned
  and repaired automatically. Both workflows remain implemented for future
  maintenance.

## Translation rules

- Do not translate canonical agent names, `PowerRustCOBOL`, `Agents Manager`,
  `agentic_ai/`, model-profile IDs, paths, or code identifiers.
- Preserve Markdown structure and inline code.
- Translate only the changed section; do not rewrite unrelated content.
