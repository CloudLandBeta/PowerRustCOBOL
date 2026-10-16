# Spec — INDEXED file crash recovery to the last `COMMIT`

- **Status:** implemented in two parts. **Part 1** (the crash-safe `redb` engine) shipped
  in 1.10.0–1.11.0; it was made the default in 1.62.73 and **returned to opt-in in 1.70.24**
  (operator ruling 2026-09-14), so the default engine is `PRCIDXD1`. **Part 2** (the default
  engine returns to the last `COMMIT` after a crash, however large the transaction) shipped in
  **1.91.176**. This text was rewritten on 2026-10-15 to say what was built; the draft of
  2026-06-15 proposed making redb the default and a logical write-ahead log for `PRCIDXD1`,
  and neither is what the code does.
- **Folder:** specs/001-indexed-wal-crash-recovery/
- **Author:** Emerson Lopes   **Date:** 2026-06-15   **Rewritten:** 2026-10-15

## 1. Overview

PowerRustCOBOL INDEXED files support the COBOL `COMMIT` and `ROLLBACK` verbs, which bound
program-controlled transactions over every open keyed file. A `STORAGE IS DISK` file is held by
one of two engines:

| Engine | Name | Container | Default |
|---|---|---|---|
| The built-in paged B+tree store | `rust` (`IndexedEngine::Rust`) | `PRCIDXD1` | **yes** — since 1.70.24 |
| A copy-on-write ACID store on `redb` | `redb` (`IndexedEngine::Redb`) | a redb database | no — by name, `--indexed-engine redb`, or **Settings ▸ Default Indexed File Engine** |

The requirement of this spec is the same for both: **after an abnormal termination (`kill -9`,
a panic, a power loss), reopening the file leaves it at the state of the last `COMMIT` — or of
the `OPEN`, if there was none.** What the program wrote, rewrote or deleted since is gone, however
much of it there was, and nothing that was committed is.

- **Part 1 — `redb`.** redb is crash-safe by construction (dual meta pages, copy-on-write
  commits). The draft proposed making it the default so that every new program had crash safety
  immediately. It was, from 1.62.73. The operator reversed that on 2026-09-14 (1.70.24) for
  **concurrency**: redb takes `flock(LOCK_EX)` on a container opened for writing, so exactly one
  process may write one and nobody else may open it meanwhile (file status 93), where `PRCIDXD1`
  admits concurrent readers and writers. The trade is the developer's, per file. This part's
  requirements are therefore *the redb engine meets R1–R5*, not *redb is the default*.
- **Part 2 — `PRCIDXD1`.** The default engine already held a transaction's changed pages in
  memory and put them in the container only behind an undo journal (`<file>.jrn`), so a crash
  *inside one commit* was undone completely. What it did **not** do was keep that promise for a
  transaction too big for memory: every 256 changed pages (1 MiB) it took a commit point of its
  own, and a crash found every such slice durable though the program had never committed it.
  Measured on 1.91.175: 2 500 uncommitted `WRITE`s, `SIGKILL`, reopen — 2 194 records present,
  where the redb engine held the three that were committed. Part 2 makes the journal last as long
  as the transaction.

> **Scope reminder:** COBOL `COMMIT`/`ROLLBACK` act on **INDEXED files only**, not SQL
> connections. SQL transactions use `COBOL::"EXEC-SQL"` with `BEGIN`/`COMMIT`/`ROLLBACK`
> ([`docs/database-runtime-en.md`](../../docs/database-runtime-en.md)).

## 2. Goals / Non-goals

### Goals

- After an abnormal termination, a `STORAGE IS DISK` file opens at its last `COMMIT`, on **both**
  engines, whatever the size of the uncommitted transaction.
- Preserve the observable COBOL semantics of in-run `COMMIT`, `ROLLBACK`, `OPEN`, `CLOSE` and
  `FILE STATUS` on every engine.
- Recovery runs on `OPEN`, with no new COBOL syntax and no operator action.
- A recovery journal that cannot be trusted fails the `OPEN` with status `90` and changes nothing.
- Document the guarantee in the Developer's Guide and the language reference.

### Non-goals

- **Making `redb` the default.** It was, and was reversed (§1). Not a goal of this spec.
- **A logical write-ahead log of records.** The default engine journals *page images* (an undo
  log), which is what its page-oriented container needs; see §7, Q1.
- SQL durability, or one log across SQL and INDEXED files (separate runtimes).
- Multi-writer crash coordination across processes. (File locking across processes is its own
  fix, 1.70.21; a crash with two writers open is not specified here.)
- RELATIVE or SEQUENTIAL organisations.
- Changing the `PRCIDXD1` container layout. The journal is a sidecar; the container is untouched.
- Converting `PRCIDXD1` files to redb on open (a file always opens with the engine that wrote it).

## 3. User stories

- As a **COBOL developer** running a batch job — `OPEN I-O`, many `WRITE`/`REWRITE`/`DELETE`,
  `COMMIT` at checkpoints — I want a power failure to leave the file at the last `COMMIT`, so I
  can restart the job from that checkpoint.
- As a **COBOL developer** whose job wrote a hundred thousand records and had not yet committed,
  I want the crash to discard *all* of them, not a megabyte-sized prefix of them, so the file is
  never half a batch.
- As a **maintainer** comparing engines, I want both to give the same COBOL-visible result after
  a crash, so `--indexed-engine` stays a concurrency/footprint choice, not a semantic fork.
- As a **technical writer**, I want the reference to say that `COMMIT`/`ROLLBACK` are INDEXED-only
  and to describe what a crash leaves behind.

## 4. Requirements (EARS)

### Crash recovery — both engines

- **R1 (ubiquitous):** The last successful COBOL `COMMIT` — or an implicit commit at `CLOSE` or
  end of run — is the **durability boundary** for `STORAGE IS DISK` files.
- **R2 (event):** When a `STORAGE IS DISK` file is opened for writing after its hosting process
  ended abnormally, the system shall leave it reflecting only the mutations at or before the
  boundary in R1.
- **R3 (state):** While a transaction is open (after `OPEN` or `COMMIT`, before the next
  `COMMIT`/`ROLLBACK`/`CLOSE`), the system shall hold what is needed to satisfy R2 **on disk**,
  not in memory alone, **for the whole life of the transaction** — including after the engine
  has had to write part of the transaction into the container to bound its memory.
- **R4 (event):** When `COMMIT` executes, the system shall make all mutations since the previous
  boundary durable and discard what recovery would have needed.
- **R5 (event):** When `ROLLBACK` executes, the system shall restore the state at the last
  boundary without a restart, and leave the durable record consistent with it.

### Engines

- **R6 (ubiquitous):** The default engine for `STORAGE IS DISK` is `rust` (`PRCIDXD1`); `redb`
  is selected by name (`--indexed-engine`, `COBOL_INDEXED_ENGINE`, or the project setting).
- **R7 (ubiquitous):** The `redb` engine shall satisfy R1–R5. (It does by construction; this
  spec holds it to the same test as the default engine.)
- **R8 (ubiquitous):** The system shall preserve the `FILE STATUS` codes and verb outcomes of
  programs that do not crash (regression suites green).
- **R9 (constraint):** `COMMIT`/`ROLLBACK` keep their meaning for **SQL** handles: unchanged.

### MEMORY storage

- **R10 (state):** For `STORAGE IS MEMORY` without `WITH PERSISTENCE`, the file is ephemeral: a
  crash triggers no recovery and no durable log is written.
- **R11 (event):** When a `STORAGE IS MEMORY WITH PERSISTENCE` file is opened after an abnormal
  termination, the system shall reload the image saved at the most recent `CLOSE`; the
  uncommitted in-RAM work of the dead run is discarded.

### Failure modes

- **R12 (event):** When recovery discards uncommitted work, the system may log it at `INFO`/`WARN`
  (`COBOLT_LOG`); no UI is required.
- **R13 (event):** When the recovery journal is damaged — it holds fewer valid page images than it
  certified, or its header is neither readable nor blank — `OPEN` shall fail with `FILE STATUS`
  **`90`** and leave the container and the journal as they are. A torn *tail* (never certified,
  the container never touched for it) is not damage and is ignored.
- **R14 (constraint):** A journal written by an earlier release (`PRCJRN01`) shall still be
  recovered.

### Documentation

- **R15 (ubiquitous):** The Developer's Guide, the language reference and the indexed-file
  internals shall state the guarantee, say that `COMMIT`/`ROLLBACK` are INDEXED-only, and no longer
  call crash recovery future work.

## 5. Acceptance criteria

- [x] **AC1 — Committed survives, uncommitted does not (R1, R2).** A real `rcrun` process
  `OPEN I-O`s a `STORAGE IS DISK` file, `WRITE`s a record, `COMMIT`s, then `WRITE`s, `REWRITE`s
  and `DELETE`s without committing, and is killed with `SIGKILL`. A fresh process opens the file:
  the committed record is there, the uncommitted `WRITE` is absent, the `REWRITE` and `DELETE`
  are undone. *Proof: `indexed_crash_recovery::a_kill_leaves_the_file_at_its_last_commit`, on
  `rust` and `redb`.*
- [x] **AC2 — Rollback and crash leave the same file (R5).** *Proof: `a_rollback_and_a_kill_leave_the_same_file`.*
- [x] **AC3 — In-run `ROLLBACK` still works (R8).** *Proof: `test_transactions`, `test_indexed`,
  `test_fileio_storage` and the `indexed_disk` unit tests, green (§8).*
- [x] **AC4 — No index corruption (R2).** After a kill, `START`/`READ NEXT` walks the keys in
  ascending order with no repeat, gap or anomalous status. *Proof: the same harness reads every
  record after recovery; the unit tests check strict ascent over 1 500 records.*
- [x] **AC5 — The default is `PRCIDXD1`; `redb` is by name (R6).** *Proof: `IndexedEngine::default()`
  is `Rust` (`cobolt-runtime/src/indexed.rs`); the harness runs both engines by name.*
- [x] **AC5b — `PRCIDXD1` after a kill, however big the transaction (R3).** 2 500 uncommitted
  `WRITE`s of 1 020 bytes (about 630 pages, against a spill every 256) and a kill leave the three
  committed records, not 2 194. *Proof: `a_big_uncommitted_transaction_does_not_survive_a_kill`,
  on both engines.*
- [x] **AC6 — `COMMIT` is durable (R4).** A kill after `COMMIT` loses nothing committed. *Proof:
  AC1's committed record; unit `a_spilled_transaction_that_commits_survives_a_crash`.*
- [x] **AC7 — A damaged journal fails closed (R13).** A journal cut below the images it certified
  makes `OPEN` answer `90` with both files byte-identical to before; restored, the same `OPEN`
  recovers. A flipped bit inside a certified image, and a header that is neither readable nor
  blank, answer `90`; a torn tail and a zero-filled header are not damage. *Proof:
  `a_damaged_recovery_journal_fails_the_open_with_90_and_touches_nothing` (real processes) and
  the six journal unit tests.*
- [x] **AC8 — Docs (R15).** *Proof: Developer's Guide, *Crash-safe transactions* and the engine
  table; `cobol85-supported-syntax-en.md`, COMMIT/ROLLBACK bullet (the "future work" sentence is
  gone); `indexed-file-internals-en.md` §8; `indexed-redb-engine-en.md` (the comparison table).*
- [x] **AC9 — `MEMORY WITH PERSISTENCE` reloads the last `CLOSE` (R11).** *Proof:
  `a_memory_file_with_persistence_reloads_its_last_close_after_a_kill`.*
- [x] **AC10 — Full suite (R8).** *See §8.*

## 6. Constraints & steering check

| Steering constraint | Impact |
|---|---|
| **i18n (6 languages)** | None — no IDE strings. |
| **Generated-code contract** | None. |
| **English docs only** | Updated the English canonical of four documents; none has translations in the tree. |
| **Fix vs feature** | **Fix** (operator, 2026-10-15), by the same reasoning as cross-process file locking (1.70.21): a verb doing half of what it says — `COMMIT` and a crash — is technical debt, not a capability. `fixes` branch, a `z` bump. Part 1 is history. |
| **Tests: verify-first** | The crash tests kill a real child process and reopen the file; nothing is asserted about recovery without the kill. |
| **No "cobolt" in user-facing text** | The documents say PowerRustCOBOL / RustCOBOL / `rcrun`. |

## 7. Decisions (the draft's open questions, answered by the build)

- **Q1 — Where does the log live, and what is in it?** A **sidecar**, `<container>.jrn`, as the
  default engine already had — but of **page images**, not logical operations. The container is
  page-oriented and its operations touch a data page, a leaf per key, sometimes a split, a
  directory leaf and the header; the undo of *all* of them is the earlier image of each page, which
  is exactly what is needed to put a half-written container back and is independent of what
  operation wrote it. A logical log would have to be replayed through the B+tree code in the
  state of a half-written file.
- **Q2 — Does the container version bump?** No. The container is unchanged; only the journal has a
  new form, `PRCJRN02`, and the earlier one is still read (R14).
- **Q3 — Does the engine write pages before `COMMIT`?** Not in the common case: a transaction's
  pages are held in a write set and reach the container only behind their journal. Once the write
  set reaches 256 pages it is *spilled* — journaled and written — and, since 1.91.176, the journal
  stays. A page the transaction adds, past the end of the container as it was, needs no image
  (recovery cuts the file back to its original length), so a load that mostly appends keeps a journal
  of a few pages: 254 pages added, 3 images journaled.
- ~~**Q4 — MEMORY WITH PERSISTENCE:**~~ R10/R11: no log; the last `CLOSE` snapshot is reloaded
  (proved by AC9).
- **Q5 — Performance.** A `COMMIT` that fits the write set costs what it cost: journal and `fsync`,
  pages and `fsync`, remove the journal. A spill is paid once per 256 pages and costs two more
  `fsync`s and the directory entry of a journal that is going to be long-lived. See §8 for the
  measurement.
- **Q6 — Compression.** Irrelevant to the journal: it holds page images, which are the bytes the
  container holds (already compressed).

## 8. Results

*(measured 2026-10-16, on a clean build of the `fixes` tree)*

- `cobolt-runtime`: 1 460 tests passed, 0 failed (28 ignored), among them the 54 of `indexed_disk` — the
  40 earlier ones and 14 for the transaction journal: a crash with a spilled transaction is undone (the
  container 8 → 262 pages while uncommitted, 3 page images journaled, back to 8 pages); an update
  transaction that journaled 312 page images is undone **to the byte**; a crash part-way through a later
  spill is undone; commit and rollback after a spill leave no journal.
- `cobolt-cli`: 42 passed, 0 failed (3 ignored), among them `indexed_crash_recovery`, 6 tests with real
  `rcrun` processes and `SIGKILL`, the same program on `rust` and `redb`. Before the change the
  630-page uncommitted transaction left **2 194** records on `rust`; after it, 3, as on `redb`.
- Commit cost (release builds, median of 5): 300 x (`WRITE`, `COMMIT`) 2.98 s before, 2.87 s after; one
  transaction of 100 000 `WRITE`s (about a dozen spills) 3.94 s before, 3.78 s after. No regression.
