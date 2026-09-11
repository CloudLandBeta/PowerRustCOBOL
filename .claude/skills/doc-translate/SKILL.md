---
name: doc-translate
description: Translate a PowerRustCOBOL English document into es/pt/fr/jp/cn yourself, following GOLDEN RULE #8's regeneration cycle — the 32 KB split, what stays in English, the version stamp and the `.<<` completion sentinel. Use when asked to translate or localize documentation, when a translation is behind its canonical, or when `every_translation_is_complete_and_current` names a file. NOT `/doc-localize`, which routes bulk work to an external agent and is superseded.
---

# Translating a document

The **procedure is not invented here.** It is written in three places that agree:
`CLAUDE.md` GOLDEN RULE #8, `specs/steering/docs.md` → *Localization policy*, and
`.claude/skills/docsync/SKILL.md`. Read the steering copy before a first run.
This skill is how to *execute* it, plus the two marks that make "done" checkable.

## What counts as done

A translation counts only when **both** are true. Anything else is **redone from
the current English** — not patched.

1. **It ends with the sentinel**, as its own last line:

   ```
   .<<
   ```

   Nothing else can tell you a translation finished. A truncated Markdown file is
   still valid Markdown: the IDE's Help would render the half that exists and
   look fine.

2. **Its version stamp matches its canonical's**, near the top, after the licence
   header:

   ```html
   <!-- powerrustcobol: 1.65.123 -->
   ```

   On the English file this is the version it was last changed at. On a
   translation it is the version of the English file it was **made from**. Lower
   means behind; absent means unverifiable, which means redo.

`crates/cobolt-ide/src/docs_embed.rs` →
`every_translation_is_complete_and_current` asserts both and **prints what is
outstanding**. Run it with `--ignored`; it is the work list, not a judgement
call. The cycle is finished when its `#[ignore]` comes off and it passes.

## Sizing

| English size | Route |
|---|---|
| ≤ 32 KB | translate the whole file into `<doc>-<lang>.md` |
| > 32 KB | split by ToC entry → `temp-<doc>-en.md` (title, intro, ToC) + one `temp-<doc>-<section>-en.md` per entry; translate each; `cat` back **in original order**; delete every `temp-*` |
| a ToC entry itself > 32 KB | subdivide that entry |
| > 32 KB with no ToC | add a ToC to the **English canonical** first |

Size against **Japanese**, the worst case: es +5–13 %, pt +5–12 %, fr +9–18 %,
cn +1–5 %, **jp +17–40 %**.

**Never split mid-context.** A paragraph, a table, a fenced code block and a
mermaid block each stay whole in one temp file. Cut only at a heading.

**Temp files** live in `docs/`, are **never committed**, and are **never reused**.

### Commit one language at a time

The steering rule says an interrupted run is recovered by deleting every `temp-*`
and starting over, never by resuming. That is right about *temp* files — a
half-written split is not trustworthy — but a finished `<doc>-<lang>.md` is not a
temp file.

So: **finish one language end to end, write the file, commit it, delete that
language's temps, then start the next.** A restart then costs one language, never
the document. On a 507 KB document like the Guide that distinction is the
difference between an hour lost and a day.

## What is translated, and what is not

Translate the prose. **Leave these exactly as they are, in every language:**

- every COBOL keyword, data-item name and paragraph name;
- **every line inside a `cobol` code block** — the CRITICAL constraint in
  `CLAUDE.md`: generated COBOL is English regardless of UI language;
- CLI commands and flags (`rcrun`, `--indexed-engine`), file paths, crate names,
  type and function identifiers, property names;
- IDE menu labels, and the product names **PowerRustCOBOL**, **RustCOBOL**,
  **rcrun**.

**Never write "cobolt" in user-facing text.** It is a build-only crate prefix.

**Never machine-copy English into a translation file to make it "exist."** That
is how `-pt`, `-jp` and `-cn` once became English text under a translated
filename — the failure this whole cycle was created to undo.

## Links and anchors

Translate the section headings, then **regenerate the ToC anchors from the
translated headings**, and repoint cross-document links at the same-language file
(`observability-en.md` → `observability-pt.md`). One English file in, exactly one
file per language out — six in total, no fragments left.

## Before claiming a file done

- [ ] Last line is `.<<`.
- [ ] Stamp matches the canonical's.
- [ ] `iconv -f UTF-8 -t UTF-8 <file> >/dev/null` — clean.
- [ ] `grep -c $'\xc3\xa2\xc2\x80' <file>` → **0** (no double-encoded bytes).
- [ ] No leftover English prose, and no characters from another script.
- [ ] Every ToC anchor resolves to a heading *in that file*.
- [ ] `cargo test -p cobolt-ide --bin cobolt-ide every_translation_is_complete_and_current -- --ignored`
      no longer names it.

## When the cycle runs

GOLDEN RULE #8: **minor/major only** — the operator raises `x` or `y`. It never
runs on a `z` bump, which is every agent-made change. Do not translate unless the
operator asks.

When a `z` change touches a document, update the **English canonical only** and
bump its stamp. The translation then reads as behind, which is true, and the test
will say so.
