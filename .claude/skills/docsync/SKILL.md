---
name: docsync
description: Documentation phase for PowerRustCOBOL. Keep the docs in sync with the code using the code↔document registry, so a code change updates only the documents/sections that describe it. Use when the user runs /docsync, after /implement, or whenever code changed and docs may be stale. (Not a document-format tool — see anthropic-skills:docx/pdf for those.)
---

# /docsync — keep documentation in sync with the code

The documentation phase of the spec-driven workflow (see `specs/README.md`).
Drives English-canonical doc updates from the **code↔document registry** in
`specs/steering/docs.md`, and hands screenshots to `/doc-shots` and translations
to `/doc-localize`.

## Steps

1. **Read** `specs/steering/docs.md` (the registry + policy) and the other
   `specs/steering/*.md`.
2. **Determine scope:**
   - Default: the code changed since docs were last synced — `git diff --name-only`
     for the relevant range (ask the user for the range/commit if unclear), or
   - a feature folder's `plan.md` "Affected crates/files", or
   - "audit everything" if the user asks for a full pass.
3. **Map changes → docs.** For each changed path, find registry rows whose "Code
   areas" globs match. Collect the affected documents/sections. If a change has
   **no** matching row, that's a registry gap — add a row (and tell the user).
4. **Update only those English docs/sections** so they match current behaviour:
   features, caveats, and **sample code/CLI examples**. Do not rewrite accurate
   prose. Verify samples against the code (verify-first). Write to the conventions
   in `specs/steering/doc-style.md` (voice, paragraph/bullet/heading rules, when
   to add a diagram, colour/emphasis).
5. **Screenshots:** if a change invalidates an image (UI/layout), or a section is
   missing one, list it and invoke **`/doc-shots`** (or note it for the operator).
6. **Translations (GOLDEN RULE #8 — required, not optional):** carry **the same
   delta** you just wrote into **every** supported language — **es, pt, fr, jp,
   cn** — as `<doc>-<lang>.md` beside the English file. Translate the prose only:
   COBOL keywords, `cobol` blocks, CLI commands/flags, paths, identifiers,
   property names, menu labels and the product names stay English. Never
   translate from a translation, and never copy English in to make a file exist.
   Then verify each touched file: `iconv -f UTF-8 -t UTF-8` passes, zero
   double-encoded sequences, no leftover English prose.
7. **Report** a concise summary: which docs/sections changed in which languages,
   registry rows added, and screenshots flagged.

## Rules

- **English first, every language same change.** Write/patch the English
  canonical, then carry it into all five translations before calling the change
  done. (This **supersedes** the old "English canonical only" rule — Claude now
  writes translations directly.)
- Keep the registry in `specs/steering/docs.md` current — a new doc/section needs
  a row in the same change.
- New `README.md` files are hidden by the global gitignore — `git add -f` them.
- **Do not commit or push** unless the user asks; when asked, docs-only changes
  are a `docs:`/`chore:` commit (no version bump), per the operator's rules.
