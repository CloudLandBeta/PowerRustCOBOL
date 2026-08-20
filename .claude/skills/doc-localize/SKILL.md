---
name: doc-localize
description: "Produce localization work orders so an EXTERNAL agent does bulk translating. SUPERSEDED as the default path by GOLDEN RULE #8 (2026-08-20) — Claude now writes translations directly in /docsync. Use ONLY when the operator explicitly asks to route bulk translation outside; never as the automatic follow-up to a doc change."
---

# /doc-localize — route bulk translation outside (opt-in only)

> **Superseded as the default (2026-08-20).** GOLDEN RULE #8 requires every doc
> change to ship in all six languages in the same change, written by Claude —
> see `/docsync` step 6 and `specs/steering/docs.md` → Localization policy. Only
> use this skill when the operator explicitly asks for bulk work to be routed to
> an external agent.

Helper of the `/docsync` documentation phase. When invoked, this skill produces
a work order and does not translate itself — it prepares precise instructions
for an external agent to follow.

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

- **Superseded as the default path (2026-08-20).** GOLDEN RULE #8 requires every
  documentation change to ship in every language in the same change, and Claude
  now writes those translations directly (see `/docsync` step 6). Use this skill
  **only** when the operator explicitly asks for bulk translation work to be
  routed outside — not as the automatic follow-up to a doc change.
- When it *is* invoked, it produces/routes the work order only; it does not
  translate.
- Keep work orders small and section-scoped so translation cost tracks the actual
  change, not the whole document.
- Don't commit/push unless asked.
