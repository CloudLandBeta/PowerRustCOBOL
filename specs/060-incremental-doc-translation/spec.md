# Spec — Incremental, section-granular documentation translation

- **Status:** draft → approved
- **Folder:** specs/060-incremental-doc-translation/
- **Author:** Claude Opus 5 (agent), for Emerson Lopes   **Date:** 2026-09-14

## 1. Overview

GOLDEN RULE #8's regeneration cycle discards a document's five translations
whenever its English canonical changes, and rebuilds them whole — only on a
minor or major. The unit of work is therefore the entire corpus, which makes a
one-sentence correction unaffordable: on 2026-09-14 a single sentence recording
DB104A's timeout was parked to the next minor rather than land, because editing
`docs/cobol85-supported-syntax-en.md` would have deleted five files and reddened
two guards (see `NIST/progress.json` → `parked`,
`db104a-timeout-note-in-public-syntax-doc`).

The machinery to do better already exists and is under-used. Every document
carries a `<!-- powerrustcobol: x.y.z -->` stamp and every translation ends with
a `.<<` completion sentinel; `every_translation_is_complete_and_current`
(`crates/cobolt-ide/src/docs_embed.rs`) already compares the two and reports
`behind` / `unfinished` / `unstamped`. Staleness is **already detectable per
document**. The policy uses that detection only to decide what to delete.

This spec makes the translation unit a **section** rather than a file, keeps a
behind translation on disk behind a visible banner instead of deleting it, and
identifies stale prose by **content hash** rather than by version number — so
that unchanged text is never re-translated and no reader ever loses a document.

### Measured baseline (2026-09-14, at 1.70.12)

| Fact | Value |
|---|---|
| English canonicals in `docs/` | 13 files, **726 242 bytes** |
| Cost of one full regeneration | 726 KB × 5 languages ≈ **3.6 MB** of prose; Japanese expands +17–40 % |
| `developers-guide-en.md` | **520 306 bytes — 71.6 % of the whole corpus** |
| Canonicals over the 32 KB split threshold | **2 of 13** |
| Guide structure | 26 `##` sections, 99 `###` subsections |
| Guide `##` sections over 32 KB | **5**, the largest ("8. The control catalogue") **130 660 bytes** — 4× the threshold |
| Guide `###` subsections, mean size | ≈ **5.3 KB** — every one under threshold |

## 2. Goals / Non-goals

**Goals**

- A documentation change re-translates only the prose that actually changed.
- No document is ever absent in any of the six languages, at any moment.
- Splitting a large document stops being a manual, out-of-tree, order-sensitive
  procedure.
- The largest single translation unit is bounded and small.
- A reader always knows whether the page they are reading is current.

**Non-goals**

- Changing the six supported languages (en, es, pt, fr, jp, cn).
- The IDE's `Tr` strings. They stay translated in all six languages in
  `i18n.rs`, are unaffected by this spec, and remain a `tech.md` hard constraint
  with its own completeness test.
- Relaxing what stays English in every language: COBOL keywords, data-item and
  paragraph names, every line inside a `cobol` code block, CLI commands and
  flags, file paths, crate/type/function identifiers, property names, IDE menu
  labels, and the product names RustCOBOL / PowerRustCOBOL / rcrun.
- Translating automatically on every `z` bump. This spec reduces the cost of a
  regeneration; it does not change who triggers one.
- Machine translation without review. The translator remains an agent working to
  the existing verification bar.

## 3. User stories

- As a **Japanese-reading developer**, I want the page to exist and tell me it is
  slightly behind, so that I am not silently served English under a translated
  filename, nor a 404 for six weeks.
- As an **agent fixing one sentence**, I want to re-translate one section, so
  that a correction is not deferred to a minor release.
- As the **operator**, I want to see exactly which sections are behind before a
  minor, so that I can budget the regeneration instead of discovering its size.
- As a **maintainer splitting a large document**, I want the split to be the
  file layout rather than a temporary procedure, so that a half-finished run
  cannot leave artefacts where `include_dir!` will serve them as documents.

## 4. Requirements (EARS)

### Phase 1 — Split the Developer's Guide

- **R1 (ubiquitous):** The system shall hold the Developer's Guide as multiple
  per-topic English documents in `docs/`, each no larger than 32 KB.
- **R2 (ubiquitous):** Each resulting document shall carry the `-en` suffix and
  a row in the `specs/steering/docs.md` code↔document registry.
- **R3 (constraint):** The split shall not remove, reword or reorder any prose,
  diagram, screenshot placeholder or code block. It is a re-partitioning only.
- **R4 (event):** When the Guide is split, the system shall repoint every
  **live** reference to the old path in the same change — `README.md`,
  cross-document links in `docs/`, Rust doc comments and the IDE Help resolver's
  tests, and the three `.claude/skills/` files that name it.
- **R4b (constraint):** The system shall **not** rewrite references inside
  `CHANGELOG.md` or shipped `specs/NNN-*/` folders. Those record what was true
  when they were written; editing them falsifies the history rather than
  maintaining it.
- **R5 (ubiquitous):** The IDE Documentation viewer shall present the resulting
  documents as an ordered set that reads as one guide, with the guide's first
  document remaining the first row in the list in every language
  (`the_guide_sorts_first_in_every_language`).

### Phase 2 — Section-granular stamping

- **R6 (ubiquitous):** Each translatable section of an English canonical shall
  carry a machine-readable identity that is stable across edits to its prose.
- **R7 (ubiquitous):** Each section of a translation shall record the identity
  and content hash of the English section it was made from.
- **R8 (event):** When an English section changes, the system shall mark only
  that section's translations stale, leaving every other section current.
- **R9 (event):** When a regeneration runs, the system shall re-translate only
  sections marked stale.
- **R10 (constraint):** The system shall not split a paragraph, Markdown table,
  fenced code block or mermaid block across sections. Section boundaries are
  heading boundaries.
- **R11 (constraint):** The system shall not require temporary files inside
  `docs/`. `include_dir!` serves that directory wholesale, and a stray file
  there is published as a real document.

### Phase 3 — Stale-but-labelled translations

- **R12 (state):** While a translation section is behind its canonical, the
  system shall keep the translation on disk and render it.
- **R13 (state):** While a translation section is behind its canonical, the IDE
  Documentation viewer shall display a notice naming the version the text was
  made from and stating that the English has since changed.
- **R14 (constraint):** The system shall not delete a translation because it is
  behind. Deletion remains reserved for a document that no longer exists in
  English.
- **R15 (ubiquitous):** `every_translation_is_complete_and_current` shall report
  every behind section by name and **not fail the build** on staleness alone.
- **R16 (ubiquitous):** That test shall continue to **fail** on a translation
  that is truncated (missing its `.<<` sentinel), unstamped, or present for a
  document that has no English canonical.
- **R17 (constraint):** No English file shall ever be deleted, under this spec
  or any other.

### Phase 4 — Content hashing

- **R18 (ubiquitous):** Staleness shall be determined by the hash of the English
  section's source text, not by the product version.
- **R19 (event):** When a version bump changes no prose, the system shall mark
  nothing stale.
- **R20 (constraint):** The hash shall cover only the section's translatable
  prose, excluding content that stays English by the Non-goals above, so that a
  change confined to a code block does not invalidate a translation.

## 5. Acceptance criteria

**Phase 1**
- [ ] AC1 — No file in `docs/` exceeds 32 KB.
- [ ] AC2 — `cargo test -p cobolt-ide` is green, including
      `every_language_lists_each_document_exactly_once`, with the Guide's
      documents enumerated exactly once per language.
- [ ] AC3 — Byte-for-byte, the concatenation of the new English documents
      contains every non-heading line of the previous `developers-guide-en.md`.
- [ ] AC4 — Every **live** reference is repointed: the 12 tracked files that
      name `developers-guide-en` outside the historical record — `README.md` (1),
      `docs/` cross-links (6), `crates/` (2: `docs_embed.rs`, `doc_shots.rs`) and
      `.claude/skills/` (3: `implement`, `tasks`, `nist-grind`).
- [ ] AC4b — Every **historical** reference is left alone: `CHANGELOG.md` (1) and
      the 142 tracked files under `specs/` record what was true when written and
      are **not** rewritten. (The 318 further hits under `.claude/worktrees/` are
      untracked working copies, not references.)
- [ ] AC5 — Every new document has a registry row in `specs/steering/docs.md`.

**Phase 2**
- [ ] AC6 — Editing one sentence of one section and running the regeneration
      re-translates exactly one section per language, demonstrated by a diff
      touching only that section in each of the five files.
- [ ] AC7 — A regeneration run leaves no file outside `docs/` behind and creates
      none inside it beyond the six per document.
- [ ] AC8 — Every generated file passes `iconv -f UTF-8 -t UTF-8`, reports zero
      matches for double-encoded bytes (`grep -c $'\xc3\xa2\xc2\x80'`), and
      contains no leftover English prose or characters from another script.

**Phase 3**
- [ ] AC9 — With a section deliberately behind, the document still renders in
      all six languages and the viewer shows the staleness notice, in that
      language, naming the source version.
- [ ] AC10 — `every_translation_is_complete_and_current` names the behind
      section and exits zero.
- [ ] AC11 — Truncating a translation (removing `.<<`) fails that test.
- [ ] AC12 — The staleness notice is a `Tr` field present in all six languages.

**Phase 4**
- [ ] AC13 — Bumping `z` with no prose change marks zero sections stale.
- [ ] AC14 — Changing only a `cobol` code block inside a section marks zero
      sections stale.

## 6. Constraints & steering check

- **i18n (6 languages):** unaffected for IDE strings — `Tr` stays complete in
  all six. Phase 3 **adds** one `Tr` field for the staleness notice (R13, AC12),
  which must therefore ship in all six languages.
- **Generated-code / regenerate contract:** no impact. No `.cbl`, no
  `cobolt-codegen` path, no form model.
- **System KB:** no impact. This touches no compiler/runtime behaviour, control,
  property, method or event, so the `cobolt-compiler` doc tables and
  `assets/knowledge/chunked.data` are untouched.
- **Docs (English guide) update needed:** yes — Phase 1 *is* a change to the
  English canonical. Under the current rule that would delete five translations;
  Phase 1 should therefore land **on a minor**, or after Phase 3 makes deletion
  unnecessary. Sequencing is an open question (Q2).
- **PRIME DIRECTIVE:** any tool that stamps, hashes, splits or checks must be
  **Rust** if it is committed — a `cobolt-ide` example in the mould of
  `build_chunked_kb`, or a test. A throwaway script may drive a one-off
  migration only from outside the repository.
- **Fix vs feature:** proposed as a **fix** — the doc pipeline is technical
  debt, and by the 2026-08-29 precedent ("removing a requirement that should
  never have applied is a fix") withdrawing the all-or-nothing cost adds no
  capability. Operator to ratify (Q1).

### ⚠️ A steering constraint this spec contradicts, and which is already stale

`specs/steering/tech.md` §Hard constraints says:

> **Docs:** the **English** `docs/developers-guide-en.md` is canonical and kept
> current. The `-es/-pt/-jp/-cn` translations are **user-maintained — never edit
> them**.

That line is **already wrong**, independently of this spec, on two counts:

1. It contradicts GOLDEN RULE #8 and `specs/steering/docs.md`, both of which
   have the agent regenerate translations (operator ruling, 2026-08-24).
2. It names four language suffixes. There are five: `-fr` is missing, and the
   Guide has shipped in all six languages since 1.70.4.

It must be corrected before this spec is approved, because a plan written
against it would be written against a rule the project stopped following a year
ago. This is the "one rule lives in three files" failure mode: `CLAUDE.md`,
`specs/steering/*` and `.claude/skills/*` each state the rule, and they drift.

## 7. Open questions

- **Q1 — Classification.** Fix or feature? §6 argues fix. The operator's ruling
  governs, and it decides which branch the work lands on.
- **Q2 — Sequencing of Phase 1.** The Guide split edits the English canonical
  and so, under today's rule, deletes five translations. Land it on a minor
  (paying the regeneration once, deliberately), or hold it until Phase 3 removes
  the deletion? Recommendation: **land Phase 1 on the next minor**, since that
  minor already owes a full regeneration.
- **Q3 — Section granularity.** `##` or `###`? `###` gives the Guide 99 units
  averaging 5.3 KB and no unit over threshold; `##` gives 26 units, five of them
  over. Recommendation: **`###`**, falling back to `##` where a document has no
  third level.
- **Q4 — Where the hash lives.** Inline HTML comment beside each translated
  heading (self-contained, survives a file copy, visible in diffs) or a sidecar
  map per document (cleaner prose, another file to keep in step).
  Recommendation: **inline**, matching how the existing `<!-- powerrustcobol:
  … -->` stamp already works.
- **Q5 — Does Q3's choice bind the Guide split?** If sections become the unit,
  Phase 1 is an optimisation rather than a necessity. It is still recommended:
  it bounds the largest file, and 72 % of the corpus in one document makes every
  whole-file operation expensive regardless of translation.
- **Q6 — GOLDEN RULE #8's text.** This spec changes it. Who edits `CLAUDE.md`,
  `specs/steering/docs.md` and the `doc-translate` / `docsync` skills, and in
  which change? All four state the rule today.

---

**Decisions already taken by the operator (2026-09-14), recorded so `/plan` does
not reopen them:**

- Scope: one spec, four phases, Guide split as Phase 1.
- Stale guard: a behind translation **ships with a banner**; the test warns
  rather than fails (R15), while still failing on truncation and missing stamps
  (R16).
