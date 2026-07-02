---
name: doc-localize
description: Produce localization work orders for PowerRustCOBOL docs so an EXTERNAL/cheaper agent does the translating (never Codex credits). Emits, under specs/localization/, what changed in the English canonical and the target languages. Use when the user runs /doc-localize or when /docsync marks translations stale.
---

# /doc-localize — drive external localization (no Codex credits)

Helper of the `/docsync` documentation phase (see `specs/steering/docs.md` →
Localization policy). **Codex never translates the documents** — translating the
growing guide is expensive and Codex credits are reserved for building
PowerRustCOBOL. This skill prepares precise instructions for an external agent.

## Steps

1. **Read** `specs/steering/docs.md` (localization policy + glossary) and the
   English doc(s) that changed (from `/docsync`, or `git diff` since the last
   localization).
2. **Compute the deltas.** For each changed English document, list the changed
   **sections (by anchor)** with the English before/after text, plus whole-new
   sections. Note removed sections too.
3. **Write a work order** at `specs/localization/<YYYY-MM-DD>-<doc-slug>.md`:
   - source file + the target languages: **es, pt, jp, cn** (existing) and **fr**
     (UI shipped; guide not yet created — mark "create from scratch");
   - the section-by-section deltas to translate;
   - the **glossary of do-not-translate terms** (PowerRustCOBOL, menu/product
     names, all COBOL keywords/identifiers and code samples);
   - a reminder that screenshots are shared (language-neutral) and need no work.
4. **Hand off.** If an external localization agent/integration is configured,
   dispatch the work order to it; otherwise leave the work order for the operator
   to route. Report the work-order path.

## Rules

- **Never edit the translation files** `developers-guide-{es,pt,jp,cn,fr}.md`
  (GOLDEN RULE #3) and **never translate with Codex** — only produce/route the
  work order.
- Keep work orders small and section-scoped so translation cost tracks the actual
  change, not the whole document.
- Don't commit/push unless asked.
