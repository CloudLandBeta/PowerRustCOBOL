---
name: doc-audit
description: Verify that PowerRustCOBOL's documentation matches the code — every "supported" claim traced to a runtime path, every "not supported" claim re-checked against what has since landed, every sample run, every number re-measured. Use when auditing a document, before publishing or translating one, when a claim looks doubtful, or whenever asked whether the docs reflect what is actually implemented. The claim-driven counterpart to /docsync, which is change-driven.
---

# /doc-audit — does the documentation describe the code that exists?

**Translating a document does not make it true.** Five accurate translations of a
wrong page are five wrong pages. Verify before translating, before publishing,
and before quoting a document as evidence.

## Why this exists alongside /docsync

`/docsync` is **change-driven**: code changed → find the documents that describe
it → update them. It cannot catch a claim that **no code change would ever
trigger**:

- **A claim that was never true.** `COMP-3` is lexed, parsed into `Usage::Comp3`
  and acknowledged by the semantic analyser — and appears **zero times** in
  `cobolt-runtime/src`. `compute_layout`'s `walk()` sizes every field as
  `pic.digits + pic.decimals` and never reads `usage`, so a `PIC 9(7)V99 COMP-3`
  takes 9 ASCII bytes where every other COBOL takes 5 packed. No commit ever
  broke this; it never worked. `/docsync` had nothing to react to.
- **A gap that closed while the doc slept.** The syntax reference said `INVOKE`
  does nothing long after it had an executor — corrected at 1.65.121 only
  because a human happened to read it.
- **A rule that rotted in place.** `tech.md` still says the translations are
  "user-maintained — never edit them" and names four language suffixes; the
  2026-08-24 ruling replaced that and there are five. No code change touches it.

## The claim surface

Roughly 700 verifiable claims live in `docs/*-en.md`: **343 ✅ supported**,
**119 ⚠️ caveats**, **4 ❌ not supported**, **216 fenced samples**, **32 named
files or symbols**.

⚠️ **343 against 4 is the finding, not the baseline.** A system this size has more
than four gaps. Under-recorded gaps are this project's default documentation
failure, so weight the audit toward **what is missing or overstated**, not toward
re-confirming what already reads as fine.

## The central heuristic: parsed ≠ implemented

A construct passes through five stages. It can clear four and be dropped by the
fifth, and every stage short of the last produces a **clean compile** that looks
like support:

```
lexer → parser → AST → semantic analyser → RUNTIME
```

**Only the last one means it works.** So for any "X is supported" claim, do not
stop at a parser test or a green compile. Grep the runtime:

```bash
grep -rn 'Comp3\|Usage::' crates/cobolt-runtime/src | head     # 0 semantic hits = ignored
```

If the runtime never reads it, the feature is *accepted and discarded*, and the
document must say so. The same shape recurs constantly: a SELECT clause parsed
into a field nothing dispatches on, a USAGE stored but never honoured, a phrase
consumed to keep parsing and then dropped.

**Silence is a claim too.** A page listing USAGE clauses that does not mention
`COMP-3` being ignored asserts, by omission, that it works.

## Claim classes, and how each is verified

| Claim | Verification | Failure it hides |
|---|---|---|
| ✅ "X is supported" | Trace to a **runtime** path, not a parser arm | Parsed-and-dropped |
| ❌ "X is not supported" | Search for an implementation that has since landed | A doc apologising for a feature that works |
| ⚠️ "out of scope **by intent**" | Is it still an operator ruling, or has it become debt? | A ruling silently reclassified — cross-process locking became a *fix* on 2026-09-14 |
| A code sample | **Run it** (`rcrun run …`, or the crate's test) | Sample rotted with an API |
| A number, score or count | **Re-measure it** | The NIST ledger stood at `1.62.132` while the code was `1.70.12` — 80 versions unverified |
| A file path or symbol | Does it still exist? | `numedit.rs` was misfiled for months; `rounded_clip.rs` was documented after deletion |
| A version or date stamp | Compare against the source of truth | Stale provenance on correct numbers |

## Steps

1. **Scope it.** One document, one section, or a claim the user doubts. A full
   pass over 700 claims is a project — say so and agree a slice.
2. **Extract the claims** — every ✅/❌/⚠️ line, every sample, every number, every
   named path or symbol.
3. **Verify each against the code, not against another document.** Two documents
   agreeing proves only that one was copied from the other. `CLAUDE.md`,
   `CONVENTIONS.md`, `AGENTS.md` and `specs/steering/*` restate each other and
   have drifted from each other more than once — they are **not** evidence.
4. **Run what can be run.** Samples, tests, the census. Verify-first: never
   report a measurement the run did not produce.
5. **Record a verdict per claim** — `confirmed` / `wrong` / `stale` /
   `unverifiable` — with the file:line that settles it.
6. **Report before editing.** Present the findings and let the operator decide
   what changes. An audit that silently rewrites the page destroys the evidence
   that it was wrong.

## Before you fix anything the audit found

**A correction to an English canonical is a documentation change**, so GOLDEN
RULE #8 applies: update the English, then **delete that document's five
translations**, which the next minor regenerates. Every document ships in all six
languages, so this is five real files and two red `docs_embed.rs` guards.

If the finding is one sentence, that cost is usually wrong to pay on a `z` bump.
**Park it** — say where — and batch it with the other pending edits to the same
file, so the minor pays once. Never quietly skip the doc instead.

**Never delete an English file.** Not under this rule, not under any other.

## Output

A table: claim → verdict → evidence (`file:line` or the command and its real
output) → recommended action. Lead with what is **wrong or missing**; confirmed
claims are a tally, not a list.

State plainly what you could **not** verify and why. "Unverifiable" is a finding;
recording it as "confirmed" is how a document earns trust it has not got.
