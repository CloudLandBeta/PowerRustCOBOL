# Tasks — INDEXED crash recovery to the last `COMMIT`

Written after the fact, 2026-10-15, in the order the work was done.

- [x] **T1 — Baseline with real kills.** `indexed_crash_recovery`: the same COBOL program on `rust`
  and `redb`, killed with `SIGKILL` at `READY`. *Result: the small cases pass on both; the 630-page
  uncommitted transaction leaves 2 194 records on `rust`, 3 on `redb`.* (R1–R3, AC1, AC5b)
- [x] **T2 — The journal spans the transaction.** `tx_spill`, `tx_flush`, `needs_undo_image`, the
  `jrn_*` state; `PRCJRN02` writer. (R3, R4)
- [x] **T3 — Recovery.** `journal_recover_v2` (valid prefix, certified count, cut to `pages`), the
  `PRCJRN01` path kept. (R2, R13, R14)
- [x] **T4 — Unit tests** — 14 in `indexed_disk`, each one named for what it proves. (AC1–AC4, AC7)
- [x] **T5 — Real-process tests for the rest** — rollback vs kill, new work after a kill, the damaged
  journal, `MEMORY WITH PERSISTENCE`. (AC2, AC7, AC9)
- [x] **T6 — Docs** — Guide, cobol85 reference, internals §8, redb comparison table. (AC8, R15)
- [x] **T7 — Spec and plan rewritten** to what was built; part 1 stated as history.
- [x] **T8 — Finalize** — 1.91.176, CHANGELOG, runtime and CLI suites.
