<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Crash-safe INDEXED engine (redb)

PowerRustCOBOL ships a second `STORAGE IS DISK` engine for `ORGANIZATION IS
INDEXED` files, built on **redb** — a pure-Rust embedded ACID key-value store
(copy-on-write B+tree, dual meta pages, per-page checksums). It presents the
*identical* observable COBOL behavior as the default `PRCIDXD1` engine, but is
designed around four operational goals the bespoke engine could not meet at
scale.

It is **opt-in** today (the default disk engine is still `PRCIDXD1`):

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

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

Contrast with the `PRCIDXD1` engine, whose RecordId directory is loaded entirely
into RAM on OPEN (≈16 bytes × every RecordId ever allocated) and whose
transactions were an in-RAM undo log persisted only on CLOSE — so it could
neither OPEN instantly at scale nor survive a mid-run power loss.

---

## On-disk layout (redb tables)

| redb table | kind     | key → value                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | primary-key bytes → (optionally compressed) record |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | primary-key bytes → `u64` insertion sequence  |
| `meta`     | table    | `schema`, `compress`, `nextseq` descriptors   |

- A **single `alt` multimap** holds every alternate key, namespaced by a 2-byte
  big-endian key index. Byte order is therefore `(key index, alt value,
  insertion sequence)` — which makes duplicate alternates iterate in **creation
  order**, exactly matching the disk engine's RecordId ordering and the COBOL
  rule for duplicate alternate keys.
- The `seq` / `meta:nextseq` machinery exists **only** to order alternate-key
  duplicates. Files with no alternate keys skip it entirely and pay just one
  B+tree insert per `WRITE`.
- Records are stored as positional fixed-width images (see
  [`indexed-file-internals.md`](indexed-file-internals.md) §6); `WITH
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

## Known trade-off

Bulk `WRITE` throughput is currently ~20 k records/s in a single transaction
(redb opens the table per call under our borrow model). This is a **one-time
load** cost; OPEN, random/sequential reads, and crash-safety — the four stated
goals — are unaffected. Faster bulk loading (cached table handles / batched
commits) is a tracked future optimization.
