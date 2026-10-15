<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.60 -->

# Crash-safe INDEXED engine (redb)

PowerRustCOBOL ships a second `STORAGE IS DISK` engine for `ORGANIZATION IS
INDEXED` files, built on **redb** — a pure-Rust embedded ACID key-value store
(copy-on-write B+tree, dual meta pages, per-page checksums). It presents the
*identical* observable COBOL behavior as the older `PRCIDXD1` engine, but is
designed around four operational goals the bespoke engine could not meet at
scale.

**It is opt-in — selected by name, not the default.** From 1.62.73 until
1.70.24 redb *was* the default engine, but the operator reversed that on
2026-09-14 (1.70.24): the default is now the `Rust` (`PRCIDXD1`) paged engine.
`IndexedEngine` derives `Default` with `#[default]` on `Rust`
(`crates/cobolt-runtime/src/indexed.rs`), and a test holds it there.

The reason is **concurrency**: a redb container open for *writing* takes an
exclusive `flock(LOCK_EX)` with no shared variant, so while one process writes
it no other process may open it at all. Programs that only read (`OPEN INPUT`)
have shared a container since 1.70.35, but `PRCIDXD1` also admits readers
alongside a writer. redb remains the
right choice wherever durability and crash-safety outrank concurrent access —
choose it by name (the trade-offs are laid out in
[Choosing an engine](#choosing-an-engine-redb-or-prcidxd1)):

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

The default paged engine, and the `rm-cobol85` / `fujitsu` aliases that delegate
to the built-in Rust container, are chosen the same way
(`--indexed-engine rust`, `--indexed-engine fujitsu`, …).

**Form applications and built binaries.** A project chooses its engine in
**Settings → Default Indexed File Engine** (`indexed_engine` under `[ide]` in
the project file: `prcidxd1`, `redb`, or empty for the default). Every surface
that runs the project uses that choice, and every form gets it, including the
forms another form opens:

- **Run** applies it to the program.
- **Run Form** and **Debug Form** pass it as `rcrun run-form --indexed-engine`.
  `run-form` reads `--indexed-engine`, `-I` and `COBOL_INDEXED_ENGINE` exactly
  as `rcrun run` does.
- **Build** writes it into the application, which has no project file beside
  it. When the project leaves it empty, the application uses
  `COBOL_INDEXED_ENGINE` on the machine that runs it, and otherwise the
  default.

The choice governs only the files a program **creates**. A file that already
exists always opens with the engine that wrote it.

Implementation:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Why — the four goals

| Goal | How the redb engine meets it |
|------|------------------------------|
| **OPEN is instantaneous, always** | redb reads only its meta page on open. There is **no in-RAM record directory to load and no recovery scan**, even after a crash. Measured: ~5 ms to OPEN a 200 000-record file (independent of record count). |
| **READ RANDOM / NEXT at light speed** | RANDOM is a B+tree descent; NEXT is a sequential range iterator. Both run over redb's page cache. Measured: ~21 µs per random read at 200 000 records. |
| **Up to 250 M records (data unbounded)** | Resident RAM is the working set (redb's cache), **not** the record count. There is no `O(records)` structure held in memory. |
| **Safety is paramount** | redb is fully ACID. `COMMIT` is a durable transaction commit (fsync); `ROLLBACK` is a transaction abort. A power loss can never expose a torn index — redb falls back to the last good commit via its dual meta pages. No data loss, no index corruption. |

Contrast with the `PRCIDXD1` engine as it stood when this one was written: its
RecordId directory was loaded entirely into RAM on OPEN (≈16 bytes × every
RecordId ever allocated) and its transactions were an in-RAM undo log persisted
only on CLOSE — so it could neither OPEN instantly at scale nor survive a
mid-run power loss. Both have changed since (1.70.31 and 1.70.32); the next
section says where the two engines stand today.

---

## Choosing an engine: redb or PRCIDXD1

Both engines implement the same COBOL verbs and the same FILE STATUS codes.
They differ in what happens when **several programs open one file at once**,
and in what a crash leaves behind. Neither is right for every file, so choose
per file, deliberately.

| | **redb** (`--indexed-engine redb`) | **PRCIDXD1** (the default) |
|---|---|---|
| Two programs reading (`OPEN INPUT`) | Both open | Both open |
| A reader while another program writes | Refused, status **93** | Admitted |
| Two plain `OPEN I-O` writers | Second refused, **93** | **Both succeed** — see the warning below |
| Two writers `OPEN I-O … WITH LOCK` | Second refused, **93** | Second refused, **93** |
| Crash before `COMMIT` | Back to the last `COMMIT` — nothing uncommitted survives | File intact, but writes since the last `COMMIT` may **partly** survive |
| `COMMIT` / `ROLLBACK` | ACID; `COMMIT` survives power loss | `COMMIT` and `CLOSE` are durable; `ROLLBACK` works within the run |
| `OPEN` cost | Meta page only, ~5 ms at 200 000 records | Directory read on demand; no slower at 200 000 records than at one |
| A one-record file on disk | **1 056 768 bytes** (~1 MB) | 16 384 bytes |

The crash row was measured by stopping a program with `kill -9` after 30 000
`WRITE`s and no `COMMIT`, on a file holding one committed record. Reopened,
the redb file held that one record. The PRCIDXD1 file opened cleanly and held
26 131: besides `COMMIT` and `CLOSE`, the engine makes its own commit point
each time 1 MiB of pages has changed, and those survive.

### Why a redb writer has the file to itself

redb takes an exclusive advisory lock on its container when a program opens it
`I-O`, `OUTPUT` or `EXTEND`, and there is no shared variant of a writing open.
`OPEN INPUT` takes a shared, read-only lock instead, so readers coexist with
each other — but never with a writer. Whichever side comes second is refused at
`OPEN`, before it reaches a record, with **file status 93**, whether the conflict
is the engine's lock or a COBOL sharing phrase. No `SHARING` phrase relaxes it.

⚠️ **A copy of a live container is not a way round the lock.** A byte copy taken
while a program is writing holds the file as of its last `COMMIT`, but redb
must recover it before it can be read: `OPEN INPUT` of such a copy answers
**90**, while `OPEN I-O` recovers it and then reads it. The copy is also of the
**whole** container, which on a large file is not viable.

### ⚠️ PRCIDXD1 permits unguarded concurrent writers

The permissiveness that lets PRCIDXD1 serve readers alongside a writer also
lets **two programs open the same file `OPEN I-O` and both succeed**, with no
coordination between them. Record locks do not cross processes either: one
program can read and `REWRITE` a record another holds `WITH LOCK`.

There are two ways to stay safe, and they are not equivalent:

- **Open every updating program `WITH LOCK`** (or `SHARING WITH NO OTHER`).
  That is enforced across processes: a second opener is refused with **file
  status 93**. It also refuses readers while the update runs, so the file then
  shares exactly as a redb file does — without redb's crash guarantee.
- **Let only one program update the file at a time**, by arrangement — a batch
  window, a single server program — while readers open `INPUT`. This is what
  only PRCIDXD1 offers: a reader scanning the file during an update reads it
  cleanly, and sees the writer's records up to its latest commit point. The
  engine does **not** enforce the arrangement: nothing stops a second updater.

### How to choose

- **Durability matters, and readers can wait while the file is updated** —
  choose **redb**.
- **Readers must keep working while the file is updated** — keep **PRCIDXD1**,
  and make sure only one program updates it at a time. You are trading
  atomicity across a crash for that concurrency, knowingly.
- **You need both, on one file** — neither engine gives you that today.
  Serialise access in the application, or split the file.

> **Note.** `SHARING WITH READ ONLY` — others may read but not write — is the
> phrase that would enforce the one-updater arrangement, but it is currently
> treated as `SHARING WITH ALL OTHER`. A single advisory lock cannot admit
> readers while refusing writers; expressing it needs a second lock covering
> the writer set.

---

## On-disk layout (redb tables)

| redb table | kind     | key → value                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | primary-key bytes → (optionally compressed) record |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | primary-key bytes → `u64` insertion sequence  |
| `meta`     | table    | `schema`, `compress`, `nextseq` descriptors, and `fd` — the record definition (the `FD` as COBOL text; see [`indexed-file-format-en.md`](indexed-file-format-en.md)) |

- A **single `alt` multimap** holds every alternate key, namespaced by a 2-byte
  big-endian key index. Byte order is therefore `(key index, alt value,
  insertion sequence)` — which makes duplicate alternates iterate in **creation
  order**, exactly matching the disk engine's RecordId ordering and the COBOL
  rule for duplicate alternate keys.
- The `seq` / `meta:nextseq` machinery exists **only** to order alternate-key
  duplicates. Files with no alternate keys skip it entirely and pay just one
  B+tree insert per `WRITE`.
- Records are stored as positional fixed-width images (see
  [`indexed-file-internals-en.md`](indexed-file-internals-en.md) §6); `WITH
  COMPRESSION` applies the same PackBits RLE used by the other engines.

---

## Transaction model

A writable open (`OUTPUT` / `I-O` / `EXTEND`) holds one redb `WriteTransaction`
open from OPEN. Reads through that transaction see the program's own uncommitted
writes (COBOL "read your writes"). The COBOL verbs map directly:

| COBOL | redb |
|-------|------|
| `OPEN`     | begin a write transaction (writable modes) |
| `COMMIT`   | `commit()` the transaction (durable), then begin a fresh one |
| `ROLLBACK` | `abort()` the transaction (discards everything since the last `COMMIT`/`OPEN`), then begin a fresh one |
| `CLOSE`    | `commit()` (implicit commit) |

`INPUT` opens use short read transactions. Because `ROLLBACK` is a true redb
abort, **no undo log is needed** — durability and rollback are the store's own
guarantees.

> The COBOL `COMMIT` / `ROLLBACK` verbs act on **INDEXED files**, not on SQL
> connections (those use `COBOL-EXEC-SQL` with `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Behavioral parity

The engine is held to the exact behavior of the default engine: the same
versioned fixtures (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) run under `--indexed-engine redb` and must produce identical
DISPLAY output — CRUD with primary + alternate `WITH DUPLICATES`, persistence
across a reopen, and `COMMIT`/`ROLLBACK`. File-status codes (`00/02/10/22/23/
35/39/46/47/48/49/90/...`), key-of-reference resolution, `START` semantics, and
the "REWRITE/DELETE need a current record" rule all match.

Tests: `crates/cobolt-runtime/tests/test_indexed_redb.rs` (fixtures under redb +
direct `IndexedStore` checks + an `#[ignore]`d scale smoke test).

---

## Limits

Because the engine is demand-paged, the practical limits are set by redb and the
filesystem, not by resident RAM:

| Dimension | Limit |
|-----------|-------|
| File size | redb / filesystem bound (terabytes) |
| Records | working-set RAM bound, not record-count bound (≥250 M with a small cache) |
| Record size | fixed-width image; large records stored as redb values |
| Key size | composite key bytes (multi-part keys supported by the COBOL layer) |
| Alternate keys | up to 65 535 (2-byte index namespace) |

---

## Performance notes

- **Sequential `READ NEXT`** by the primary key of reference returns the record
  straight from the range cursor — one B+tree descent per record, not two
  (~17 µs/record at 200 000). Alternate-key scans still do one alt descent plus a
  primary fetch.
- **`WRITE`** opens the `primary`/`alt` tables once per operation (duplicate
  check + insert share the handle). A micro-benchmark showed caching the handle
  *across* calls adds only ~8% over once-per-operation, so the engine keeps the
  simple, `unsafe`-free path. Write cost (~44 µs/record) is dominated by redb's
  ACID B+tree insert, which is the safe floor — none of the write optimizations
  change the commit points or durability.
- **Bulk `WRITE`** is therefore ~20 k records/s in a single transaction (a
  one-time load cost). OPEN, reads, and crash-safety are unaffected.

---

## Observability log (`--indexed-log`)

The redb engine can write an optional per-file transaction log (off by default)
at **`<assign-path>.log`** (e.g. `customers.idx` → `customers.idx.log`), with one
line per `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` recording timestamp, record/byte
counts, throughput, write key-ordering quality, and — at `full` level — redb
index page statistics.

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

The line format is `text` (logfmt) or `json` (NDJSON, Grafana/Loki-ready).

**The full reference** — flags, the field table, formats, the Grafana/Loki
pipeline (Promtail + LogQL), and cost/safety notes — lives in
[`observability-en.md`](observability-en.md) §1.
