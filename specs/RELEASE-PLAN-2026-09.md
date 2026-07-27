<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Release plan — Candidate 2026-08-10, Production 2026-09-04

**Revision 4 — BASELINE** · drafted 2026-07-27 (Mon) · head `f795056` (1.36.21) · `main`

Operator decisions applied: **Option A (validation-first)** and **item 11 = 15
days**. Two consequences follow, and both change the plan rather than fitting
inside it — see §2.

---

## 1. Budget

| | |
|---|---|
| Working day | **5 h** — 1 h morning, 4 h after business hours |
| RC cut | Mon **2026-08-10** (unchanged) |
| Production gate | Fri **2026-09-04** — the end of "first week of September" |
| Working days 27 Jul → 3 Sep | 29 |
| **Capacity** | **145 h** |

Revision 3 put production on Mon Aug 31. That was the *last day of August*, not
the first week of September. Using the week the operator actually asked for
buys **4 working days / 20 h**, and the plan needs every one of them.

---

## 2. What the two decisions changed

### 2.1 Item 11 now consumes over half the release
At 15 days it is **75 h of 145 h — 52%** of everything available. In revision 3
(10 days, 50 h) it fitted inside the stabilisation window. It no longer does:
75 h *is* the whole Aug 10 → Aug 28 window, leaving nothing for carried defects
or buffer.

### 2.2 A hole in revisions 1–3: no time to FIX what the gate finds
Phase 0 budgeted 15 h to **find** regressions across six weeks of unrun work.
It budgeted **zero hours to fix them**. That was an error in the earlier
revisions — a verification gate that finds nothing is a wasted gate, and one
that finds something needs repair time that was never on the calendar.
Revision 4 books **15 h** for it.

### 2.3 Therefore: RC feature scope drops to one item
```
  capacity                      145 h
− verification gate              15 h
− gate defect repair             15 h
− item 11 (15 days)              75 h
− carried defects (§6.3)         20 h
− buffer                         14 h   (10%)
────────────────────────────────────
  left for new features           6 h
```
Six hours buys exactly **item 9 (language flags)**. Items **8** and **3**,
which revision 3 had in the RC, move to 1.38.

---

## 3. Baseline schedule

| Phase | Work | Window |
|---|---|---|
| 1 — Verification gate | 15 h | 27 – 29 Jul |
| 2 — Gate repair + RC feature | 21 h | 30 Jul – 5 Aug |
| **RC cut** | — | **Mon 10 Aug** — feature freeze |
| 3 — Validation & stabilisation | 109 h | 5 Aug – 3 Sep |
| **Production gate** | — | **Fri 4 Sep** |

Item 11 starts **5 Aug**, before the RC cut. Validation is not a feature, so it
is not frozen by the cut; starting it early is what makes 15 days fit at all.

### 3.1 Task list

| ID | WBS | Task | Work | Start | Finish | Pred. |
|---|---|---|---|---|---|---|
| 1 | 1 | **Verification gate** | 15 h | 27 Jul | 29 Jul | — |
| 2 | 1.1 | Rebuild & reinstall bundle | 2 h | 27 Jul | 27 Jul | — |
| 3 | 1.2 | Manual regression pass (§6.2) | 10 h | 27 Jul | 29 Jul | 2 |
| 4 | 1.3 | Grace first-run verification | 3 h | 29 Jul | 29 Jul | 3 |
| 5 | 1.4 | ◆ Gate cleared | 0 | 29 Jul | 29 Jul | 4 |
| 6 | 2 | **Gate repair + RC feature** | 21 h | 30 Jul | 05 Aug | 5 |
| 7 | 2.1 | Repair regressions found at the gate | 15 h | 30 Jul | 03 Aug | 5 |
| 8 | 2.2 | Item 9 — language flags | 6 h | 04 Aug | 05 Aug | 7 |
| 9 | 2.3 | ◆ RC cut — feature freeze | 0 | 10 Aug | 10 Aug | 8 |
| 10 | 3 | **Validation & stabilisation** | 109 h | 05 Aug | 03 Sep | 8 |
| 11 | 3.1 | Item 11 — validate each control | **15 d** | 05 Aug | 26 Aug | 8 |
| 12 | 3.2 | Popup initial-placement repro | 5 h | 26 Aug | 27 Aug | 11 |
| 13 | 3.3 | Spec 027 corner residual | 6 h | 27 Aug | 28 Aug | 12 |
| 14 | 3.4 | Ollama Cloud endpoint confirm | 2 h | 28 Aug | 28 Aug | 13 |
| 15 | 3.5 | Localization deltas 1–8 fold-in | 4 h | 28 Aug | 31 Aug | 14 |
| 16 | 3.6 | Guides regenerated | 3 h | 31 Aug | 01 Sep | 15 |
| 17 | 3.7 | Regression buffer | 14 h | 01 Sep | 03 Sep | 16 |
| 18 | 3.8 | ◆ Production gate | 0 | 04 Sep | 04 Sep | 17 |

All links are finish-to-start. One resource, one serial chain: **every task is
on the critical path**, and total float is **zero**. The 3 h of slack that
revision 3 held before the RC cut was spent by §2.2.

---

## 4. Release content

**1.37 ships:** six weeks of agent, renderer and IDE work (§6.1) — *verified*,
which it currently is not — plus item 9 and whatever the gate repairs.

That is a substantial release. The value is in the validation evidence, not in
new feature count; §2 makes it plain that anything else added has to come out
of item 11, and item 11 is the evidence.

**Deferred to 1.38** (≈170 h): items 1, 2, 3, 4, 7, 8, 10, D1, the shared chat
component, the auto-scroll affordance. Plus items 5, D2, D3 unsized.
**Item 6 (Android/iOS)** is 2.0.0 and wants its own spec once 1.37 ships.

---

## 5. Risks against this baseline

| Risk | Signal it is materialising | Response |
|---|---|---|
| Gate finds more than 15 h of repair | Repair not finished by 3 Aug | Cut item 9; the RC becomes verification-only |
| Item 11 exceeds 15 days | Fewer than 8 controls validated by 14 Aug | Production moves to mid-September — decide by 14 Aug, not later |
| Carried defects need operator repros | 3.2 / 3.3 blocked at 26 Aug | Drop to 1.38; they are not release-blocking |
| Translator misses 26 Aug | Deltas not returned | 3.5 drops to 1.38 |
| Buffer spent before 1 Sep | — | Production date moves; do not spend item 11 |

**Checkpoint 14 Aug:** item 11 must be ~40% complete (6 of 15 days). This is
the single early-warning signal in the plan, because item 11 is 52% of it.

---

## 6. State of the code

### 6.1 Implemented (chronological)
- **Jul 17** — Version Control (Git) specialist; Grace orchestrates it.
- **Jul 18** — Agent database and Manager rail (028); agents execute tools;
  reusable model profiles (030, 031).
- **Jul 19** — Grace Knowledge Base, indexed-file agents, chat actions; agents
  bound to the project; blank-credential reporting.
- **Jul 22** — Rig transport + native tools (phases 1–4); review gate;
  chat-completions wire; verbose AI log; async RestClient; project-wide AI
  settings.
- **Jul 24** — egui 0.35 merged (027) with specs 028–035; project-tree folders
  (033); Grace target disambiguation (034); animated agent moves (035).
- **Jul 25** — diagnostics as project settings; DataGrid corner-bleed and
  sub-pixel seam fixes; run-form databind fixes.
- **Jul 26** — Debug Settings modal (IDE-wide); license text loading; AI setup
  invitation; language flags and remembered UI language; Neumorphic Cobalt;
  Models Manager fixes; Event Handler agent carries the RustCOBOL contract.
- **Jul 27 (1.36.21)** — continuation paging for replies that hit the output
  cap; agents answer in the developer's language; System Knowledge Base created
  and reported separately; change-set envelope no longer fed to the COBOL
  validator; fences recognised only at line start; Grace asks when a request is
  ambiguous; completion popup dismissal.

### 6.2 Implemented but never run by a human
egui 0.35 corners and DataGrid arcs · Debug Settings modal · language flags and
persistence · Neumorphic Cobalt contrast · Grace answering in Portuguese ·
System KB counts and the rebuild prompt · completion popup dismissal ·
continuation paging on a long review.

### 6.3 Known-unresolved, carried into Phase 3
- Completion popup **initial placement** — not reproducible from the code.
- **Spec 027 residual** — Preview/Run-Form corner square-vs-arc mismatch.
- **Ollama Cloud endpoint** — wants practical confirmation.
- **Localization deltas 1–8** — with the external translator.

### 6.4 Note on item 2 (deferred)
The COBOL proficiency prompt is not in a `.toml`: it is
`LlmConfig::cobol_proficiency_prompt` (`crates/cobolt-ide/src/llm.rs:28`),
persisted to `llm_config.json`. Converting it to a specialist needs a fixed
agent with a Pedantic companion, the prompt moved to core instructions, and a
migration that drops the config field without orphaning existing installs.
