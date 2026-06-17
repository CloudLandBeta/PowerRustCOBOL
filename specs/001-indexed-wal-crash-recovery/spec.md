# Spec — INDEXED file durable WAL & crash recovery

- **Status:** draft (scope expanded 2026-06-15: redb becomes default + WAL for legacy `PRCIDXD1`)
- **Folder:** specs/001-indexed-wal-crash-recovery/
- **Author:** Emerson Lopes   **Date:** 2026-06-15

## 1. Overview

PowerRustCOBOL INDEXED files support COBOL `COMMIT` and `ROLLBACK` verbs that
bound program-controlled transactions over open keyed files. Today the **default
`STORAGE IS DISK` engine** (`PRCIDXD1`, `indexed_disk.rs`) keeps an **in-memory
undo log** so `ROLLBACK` works while the program runs, and `COMMIT` calls
`sync_all()` to make the current state durable. If the process crashes or power
is lost **after uncommitted `WRITE`/`REWRITE`/`DELETE` operations**, the on-disk
file may contain a **partial transaction** and the undo log is gone — there is no
crash recovery to the last committed boundary.

The **redb** engine (`STORAGE IS DISK` + `--indexed-engine redb`) is already
ACID/crash-safe via redb's dual-meta-page commits. This feature delivers crash
recovery in **two parts**:

1. **Make redb the default** `STORAGE IS DISK` engine so new programs and
   default `rcrun` runs get crash safety immediately.
2. **Add a durable WAL** to the legacy bespoke engine (`PRCIDXD1`,
   `--indexed-engine rust`) so users who opt into it (or open existing
   `PRCIDXD1` files) get the same commit-boundary guarantee.

> **Scope reminder:** COBOL `COMMIT`/`ROLLBACK` verbs act on **INDEXED files
> only**, not SQL connections. SQL transactions continue to use
> `COBOL-EXEC-SQL` with `BEGIN`/`COMMIT`/`ROLLBACK` strings
> ([`docs/database-runtime.md`](../../docs/database-runtime.md) §4).

## 2. Goals / Non-goals

### Goals

- **Make redb the default** indexed engine for `STORAGE IS DISK` (replacing
  `IndexedEngine::Rust` as `default()` / `COBOL_INDEXED_ENGINE` unset).
- After an abnormal termination (kill -9, panic, power loss), reopening a
  `STORAGE IS DISK` INDEXED file shall leave the file in the **last committed**
  state — no torn indexes, no orphaned records visible to COBOL I/O — on **both**
  the default redb engine and the legacy `PRCIDXD1` engine (`rust`).
- Preserve existing **observable COBOL semantics** for in-run `COMMIT`,
  `ROLLBACK`, `OPEN`, `CLOSE`, and `FILE STATUS` codes across all INDEXED
  engines (`rust`, `redb`; behaviour-compatible aliases unchanged).
- Recovery shall run automatically on `OPEN` (no new COBOL syntax, no operator
  action).
- Document the guarantee in the English developer guide and language reference;
  remove the “future work” caveat in
  [`docs/cobol85-supported-syntax.md`](../../docs/cobol85-supported-syntax.md)
  and [`docs/indexed-file-internals.md`](../../docs/indexed-file-internals.md).

### Non-goals

- **SQL** transaction durability or a unified WAL across SQL and INDEXED (out of
  scope; separate runtimes).
- **Cross-process** file locking or multi-writer crash coordination (still
  single run-unit; see README §“Partial / in progress”).
- **RELATIVE** or **SEQUENTIAL** file organisations (separate features).
- Changing the on-disk **`PRCIDX1`/`PRCIDXD1` container layout** in a way that
  breaks existing files without a migration path (if the container version must
  bump, migration or backward-compatible recovery is required — see open
  questions).
- Automatic **in-place migration** of existing `PRCIDXD1` files to redb format
  on first `OPEN` (users with legacy files use `--indexed-engine rust` or
  recreate files; a standalone migration tool may follow later).

## 3. User stories

- As a **COBOL developer** running a batch job with `OPEN I-O` → many
  `WRITE`/`REWRITE`/`DELETE` → `COMMIT` boundaries, I want a power failure after
  `COMMIT` but before the next `COMMIT` to **not corrupt** my INDEXED file, so I
  can restart the job from the last committed checkpoint.
- As a **COBOL developer** who issued `WRITE`/`REWRITE`/`DELETE` since the last
  `COMMIT`/`OPEN` and has **not** called `COMMIT`, I want a crash to **discard**
  those uncommitted changes on reopen (equivalent to an implicit rollback to the
  last commit boundary), so partial updates never appear in subsequent `READ`s.
- As a **maintainer** comparing engines, I want the default disk engine and the
  redb engine to present the **same COBOL-visible results** after crash +
  reopen, so `--indexed-engine` remains a performance/safety trade-off, not a
  semantic fork.
- As a **technical writer**, I want the language reference to state clearly that
  `COMMIT`/`ROLLBACK` are INDEXED-only (not SQL), with crash-recovery behaviour
  documented, so users are not surprised by [`database-runtime.md`](../../docs/database-runtime.md).

## 4. Requirements (EARS)

### Crash recovery — default DISK engine

- **R1 (ubiquitous):** The system shall treat the last successful COBOL `COMMIT`
  (or an implicit commit at `CLOSE` / end-of-run close) as the **durability
  boundary** for `STORAGE IS DISK` files on the default Rust indexed engine.
- **R2 (event):** When a `STORAGE IS DISK` INDEXED file is opened after the
  hosting process terminated abnormally, the system shall **replay or discard**
  durable log data so the file reflects only mutations at or before the last
  durability boundary in R1.
- **R3 (state):** While a transaction is open (after `OPEN` or `COMMIT`, before
  the next `COMMIT`/`ROLLBACK`/`CLOSE`), the system shall record mutations in a
  **durable write-ahead log** (or equivalent crash-safe journal) such that R2 is
  satisfiable without relying on in-memory undo state alone.
- **R4 (event):** When `COMMIT` executes on a `STORAGE IS DISK` default-engine
  file, the system shall make all mutations since the previous durability
  boundary crash-recoverable as committed and truncate or checkpoint the WAL for
  that boundary.
- **R5 (event):** When `ROLLBACK` executes, the system shall restore the file to
  the state at the last durability boundary **without** requiring a process
  restart (existing in-run behaviour preserved), and shall leave the durable log
  consistent with that boundary.

### Default engine switch

- **R6 (ubiquitous):** When no `--indexed-engine` flag or `COBOL_INDEXED_ENGINE`
  env var is set, `STORAGE IS DISK` INDEXED files shall use the **redb** engine.
- **R7 (ubiquitous):** `--indexed-engine rust` (and aliases `default` remapped or
  documented) shall continue to open **`PRCIDXD1`** files via `DiskIndexedFile`.

### Semantic preservation & engines

- **R8 (ubiquitous):** The system shall preserve existing `FILE STATUS` codes and
  INDEXED verb outcomes for programs that do not experience a crash (regression
  suite green).
- **R9 (ubiquitous):** The **redb** indexed engine shall satisfy R1–R5 without
  regression; making it default is primarily a default-change plus verification.
- **R10 (constraint):** The system shall not change the meaning of COBOL
  `COMMIT`/`ROLLBACK` for **SQL** handles — those remain SQL-string driven per
  `database-runtime.md`.

### MEMORY storage

- **R11 (state):** For `STORAGE IS MEMORY` **without** `WITH PERSISTENCE`, the
  system shall continue to treat the file as **ephemeral**: a crash shall **not**
  trigger recovery of the in-memory image, and no durable WAL shall be written
  for that file.
- **R12 (event):** When a `STORAGE IS MEMORY WITH PERSISTENCE` file is opened
  after an abnormal termination, the system shall **not** attempt to recover the
  crashed in-RAM session. It shall **reload from disk** the last file image
  persisted at the most recent `CLOSE` (if any exists on the `ASSIGN` path) and
  continue as if the crash had not occurred — uncommitted in-RAM mutations from
  the terminated run are discarded. No WAL or crash-replay machinery is required
  for the MEMORY engine.

### Observability & failure modes

- **R13 (event):** When recovery on `OPEN` discards uncommitted WAL entries, the
  system may emit a **tracing** event at `INFO` or `WARN` (via `COBOLT_LOG`);
  no new IDE UI is required.
- **R14 (event):** When the WAL or data file is **unrecoverably corrupt**, `OPEN`
  shall fail with FILE STATUS **`90`** (or existing corrupt-file status used by
  the engine today), not silent data loss.

### Documentation

- **R15 (ubiquitous):** The English docs shall state that COBOL `COMMIT`/`ROLLBACK`
  verbs apply to **INDEXED** files, describe the crash-recovery guarantee for
  `STORAGE IS DISK`, and cross-link SQL transaction docs.
- **R16 (ubiquitous):** The docs shall remove or replace wording that crash
  recovery via a durable WAL is “future work” once this feature ships.

## 5. Acceptance criteria

- [ ] **AC1 — Committed survives crash:** Run a COBOL program that `OPEN I-O`s a
  `STORAGE IS DISK` file, `WRITE`s record A, `COMMIT`s, `WRITE`s record B (no
  `COMMIT`), then **kill the process** mid-run. Reopen the file in a fresh
  process: record A is readable; record B is **absent**. FILE STATUS `00` on
  successful reads.
- [ ] **AC2 — Rollback boundary matches crash discard:** Same as AC1 but call
  `ROLLBACK` instead of crash before reopen — observable file contents **match**
  the post-crash reopen from AC1.
- [ ] **AC3 — In-run ROLLBACK still works:** Without crash, existing
  `test_transactions.rs` and `tests/cobol/fileio/idx_tx.cbl` (or successor)
  continue to pass on the default disk engine.
- [ ] **AC4 — No index corruption:** After AC1, `START`/`READ NEXT` walks keys in
  ascending order with no duplicates, gaps, or FILE STATUS `90`/`30` anomalies
  beyond the pre-crash committed set.
- [ ] **AC5 — Default is redb:** With no engine flag, AC1 uses the **redb**
  substrate (verify via log or engine introspection hook in tests).
- [ ] **AC5b — Legacy rust engine:** Repeat AC1 with `--indexed-engine rust` after
  WAL ships; post-reopen contents match AC1.
- [ ] **AC6 — COMMIT fsync semantics:** After `COMMIT`, yanking power (simulated
  by immediate `kill -9` before further I/O) shall not lose committed records
  (AC1 committed half).
- [ ] **AC7 — Corrupt WAL fails closed:** Deliberately truncate or corrupt the WAL
  sidecar/container; `OPEN` returns status **`90`** (or documented corrupt
  status), not partial reads of torn data.
- [ ] **AC8 — Docs updated:** `developers-guide-en.md` §14, `cobol85-supported-syntax.md`
  (COMMIT/ROLLBACK bullet), and `indexed-file-internals.md` §8 reflect durable
  crash recovery; “future work” removed.
- [ ] **AC9 — MEMORY WITH PERSISTENCE reload:** After `CLOSE` persists a MEMORY
  file, start a new run that mutates the in-RAM image **without** `CLOSE`, kill
  the process, then `OPEN` again: the file contents match the **last `CLOSE`
  snapshot** on disk, not the uncommitted mutations from the killed run.
- [ ] **AC10 — Full suite:** `cargo test -p cobolt-runtime` (indexed + transactions)
  and relevant `tests/cobol/fileio/` programs pass with quantified results
  reported.

## 6. Constraints & steering check

| Steering constraint | Impact |
|---------------------|--------|
| **i18n (6 languages)** | None — no new IDE UI strings anticipated. Tracing/log messages are English diagnostic text. |
| **Generated-code contract** | None — no form/codegen changes. |
| **English docs only** | **Yes** — update `developers-guide-en.md`, `cobol85-supported-syntax.md`, `indexed-file-internals.md`, optionally `indexed-redb-engine.md` (clarify default vs redb). Do **not** edit translation guides. |
| **Fix vs feature** | **Feature** — new durability guarantee for default engine. Bump minor version in `version.rs` + `CHANGELOG.md` entry. |
| **Tests: verify-first** | Crash tests must **kill** a real child process and **reopen** the file; do not assert recovery without performing the crash step. |
| **No "cobolt" in user-facing text** | Docs use PowerRustCOBOL / RustCOBOL only. |
| **Code registry** (`specs/steering/docs.md`) | Rows for `cobol85-supported-syntax.md`, `indexed-file-internals.md`, `developers-guide-en.md` §14 — update in `/docsync` after implement. |

## 7. Open questions

- **Q1 — WAL placement:** Should the durable log live in a **sidecar file**
  (`<assign-path>.wal`), an **embedded region** in `PRCIDXD1`, or both (embedded
  metadata + sidecar for IO efficiency)? Preference affects backup/copy semantics.
- **Q2 — Container version bump:** Can recovery be added **without** bumping
  `PRCIDXD1` magic/layout, or is a `PRCIDXD2` with automatic migration on first
  `OPEN` required for existing deployments?
- **Q3 — Uncommitted page writes today:** The current disk engine may write btree
  pages before `COMMIT`. Does implementation require **write-ahead only** (no
  in-place commit of data pages until WAL replay) or is **checkpoint + undo in
  WAL** sufficient? (/plan must profile current flush points.)
- ~~**Q4 — MEMORY WITH PERSISTENCE:**~~ **Resolved (R10):** no WAL for MEMORY;
  reload last `CLOSE` snapshot on reopen after crash; ephemeral sessions are not
  recovered.
- **Q5 — Performance budget:** Is one `fsync` per COBOL `COMMIT` acceptable for
  the default engine, or is batching/group-commit required for workloads with
  frequent commits?
- **Q6 — Compression interaction:** For `STORAGE IS DISK WITH COMPRESSION`, must
  WAL entries store uncompressed record images (for replay), matching how undo
  works today?

---

**Next step:** Review this spec. When approved, run **`/plan`** to design WAL
layout, engine changes in `indexed_disk.rs`, tests (including process-kill
harness), and doc updates. Do **not** implement until `plan.md` and `tasks.md`
are approved.