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
6. **Translations:** for every English doc you changed that has translations, do
   **not** edit the translations (GOLDEN RULE #3). Instead invoke **`/doc-localize`**
   to emit a work order for the external translation agent.
7. **Report** a concise summary: which docs/sections changed, registry rows added,
   screenshots flagged, and the localization work order path.

## Rules

- **English canonical only.** Never edit `developers-guide-{es,pt,jp,cn,fr}.md`.
- Keep the registry in `specs/steering/docs.md` current — a new doc/section needs
  a row in the same change.
- New `README.md` files are hidden by the global gitignore — `git add -f` them.
- **Do not commit or push** unless the user asks; when asked, docs-only changes
  are a `docs:`/`chore:` commit (no version bump), per the operator's rules.
