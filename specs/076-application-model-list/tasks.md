# Tasks — Application model list and key store

- **Status:** done (see spec §6a for the open operator checks)
- **Plan:** ./plan.md   **Date:** 2026-09-24

`features` branch; one `z` bump and CHANGELOG entry for the change. Tests print quantified
results (GOLDEN RULE #7).

- [x] **T1 — `model_list.rs`** (R1–R4) — process-wide entries with generations; unit tests.
- [x] **T2 — `key_store.rs`** (R5–R11) — seam, memory store, encrypted file store in
  `<app>/settings/model-keys.dat`; unit tests for AC5–AC7.
- [x] **T3 — CALLs** (R11a, R6) — `COBOL-MODEL-SET/REMOVE`, `COBOL-KEY-SET/REMOVE/IS-SET`.
- [x] **T4 — `AgentObject` uses an entry** (R12, R14, R15, R17, R7) — `ModelEntry`,
  early `onError`, masked verbose log.
- [x] **T5 — `onModelChanged`** (R16) — raised from `drain_async_ops`.
- [x] **T6 — `KnowledgeBase` uses an entry** (R13).
- [x] **T7 — Forms model and IDE** — `ModelEntry` property on both controls, the event,
  Properties-pane rows.
- [x] **T8 — End-to-end tests** — `test_model_list.rs`: AC1, AC3, AC4, AC5, AC8–AC11.
- [x] **T9 — System KB and Developer's Guide** — doc tables, `chunked.data`, guide section.
- [x] **T10 — Finalize** — sweeps, AC12 `cargo tree`, spec ACs, CHANGELOG + `z`.
