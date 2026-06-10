<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL Indexed File Internals (`PRCIDXD1` paged engine)

This document is a conceptual schema of the **persistent, paged on-disk** engine
that backs `ORGANIZATION IS INDEXED` files declared with `STORAGE IS DISK`
(the default). It is a B+tree / slotted-page design that reads records on demand
so RAM stays bounded regardless of file size.

> **Scope.** This describes the *physical engine* (`DiskIndexedFile`, container
> magic `PRCIDXD1`). It is a different artifact from the single-blob,
> self-describing `PRCIDX1` container documented in
> [`indexed-file-format.md`](indexed-file-format.md), which models the metadata a
> future Fujitsu importer needs. The in-memory engine (`STORAGE IS MEMORY`,
> `IndexedFile`) is a simplified subset of the same logical model (BTreeMaps
> instead of on-disk B+trees).
>
> A second, **crash-safe** `STORAGE IS DISK` engine (opt-in, on the pure-Rust
> redb ACID store) addresses this engine's RAM-bound directory and CLOSE-only
> persistence — see [`indexed-redb-engine.md`](indexed-redb-engine.md).

Implementation:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
record (de)materialization in
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. Design in one sentence

A paged file of **one header page + N B+trees (one per key) → a RecordId
directory → slotted data pages of positional, fixed-width record images**, with
a free list, overflow chains, optional RLE compression, and an in-run undo log
for transactions.

---

## 2. The file is an array of fixed 4 KiB pages

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

Every page **after** page 0 identifies itself by its first byte (the page-type
tag). Freed pages are recycled through a free list, so physical page order on
disk does **not** track logical record order.

| Tag | Constant      | Page holds                                    |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | B+tree internal (routing) node                |
| `2` | `PT_LEAF`     | B+tree leaf node (doubly linked to siblings)  |
| `3` | `PT_DATA`     | slotted page packing several record images    |
| `4` | `PT_OVERFLOW` | continuation for a record too big to fit inline |
| `5` | `PT_DIR`      | a slice of the RecordId directory             |

---

## 3. Page 0 — the header

Page 0 is the only place a *schema* is stored, and it is written once. Fields are
little-endian, in this order:

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (each u64)
 primary_root   dir_head         directory_len                 (each u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (one B+tree root per alt key)
 ──────────────────────────────────────────────────────────────────────
 KEY SCHEMA:  key_count (u16) → for each key (primary first, then alternates):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × parts   (composite-key parts)
```

| Header field      | Meaning                                                       |
|-------------------|---------------------------------------------------------------|
| `version`         | Format version (currently `1`).                               |
| `page_size`       | Page size in bytes (4096).                                    |
| `rec_fmt`         | Record format: `1` = fixed length.                            |
| `compressing`     | `1` if record payloads are RLE-compressed on disk.            |
| `record_len`      | Logical (uncompressed) record length in bytes.               |
| `next_page_id`    | Next page id to allocate when the free list is empty.        |
| `free_list_head`  | First page on the reclaimed-page free list (`0` = none).     |
| `record_count`    | Number of live records.                                       |
| `data_tail`       | Current `PT_DATA` page accepting inline writes (`0` = none). |
| `primary_root`    | Root page of the primary-key B+tree.                          |
| `dir_head`        | First `PT_DIR` page of the RecordId directory (`0` = none).  |
| `directory_len`   | Number of directory entries (RecordIds ever allocated).      |
| `alt_root[k]`     | Root page of alternate key *k*'s B+tree.                      |
| KEY SCHEMA        | Per-key duplicate policy + composite-part byte ranges.        |

**What is deliberately *not* in the header:** there are **no data-field names**
and **no per-record metadata**. The schema is purely *key geometry* (byte
ranges). Everything else about a record is positional — see §6.

---

## 4. The access path (how a keyed `READ` resolves)

```
  COBOL key value (bytes)
        │
        ▼
  ┌──────────────┐   Start at primary_root (random READ by RECORD KEY) or
  │  B+tree      │   alt_roots[k] (READ KEY IS <alt>). Internal nodes route by
  │  (one per    │   key; leaves hold (key_bytes → RecordId) and are doubly
  │  key)        │   linked (next/prev) for READ NEXT / READ PREVIOUS / START.
  └──────┬───────┘
         │  RecordId (a stable integer, independent of physical location)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = free/tombstone, 1 = inline, 2 = overflow head
  │  directory   │     len : stored (possibly compressed) byte length
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Slotted DATA page → slot directory → (offset, len) →
  │  DATA page   │   raw record image (decompressed if `compressing`).
  └──────┬───────┘
         ▼
  the fixed-width record bytes
        │  RecordLayout.distribute()
        ▼
  scattered into the FD's elementary items in working memory
```

**One record, many keys.** Primary and every alternate key point at the *same*
RecordId, so there is exactly one stored copy of each record. Alternate indexes
are just additional B+trees layered over the shared RecordId directory; a
duplicate alternate value is allowed when that key was declared
`WITH DUPLICATES`.

---

## 5. Page internals

### 5.1 B+tree node (`PT_INTERNAL` / `PT_LEAF`)

A node is loaded into memory for an operation, mutated, split if needed, and
written back.

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- Leaves are **doubly linked** (`next`/`prev`) so an ordered scan after a `START`
  walks siblings directly — that is RustCOBOL's ascending-key `READ NEXT`.
- Insertion **splits on overflow** when the serialized node would exceed
  `PAGE_SIZE`; the median key is promoted to the parent.
- Internal nodes hold `child0` plus *(separator key, child)* pairs.

### 5.2 Slotted data page (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- 5-byte page header, then a **slot directory** that grows from the front while
  **record payloads** grow from the back; a record fits inline while the two
  regions have not met.
- A slot is `(offset, len)`; deleting a record sets its slot `len = 0`
  (tombstone). When every slot on a page is free, the whole page is returned to
  the free list.
- The `slot` field of a `RecLoc` indexes into this slot directory.

### 5.3 Overflow chain (`PT_OVERFLOW`)

A record larger than the inline limit (`PAGE_SIZE − header − one slot`) is stored
as a linked chain of overflow pages; its `RecLoc.kind = 2` and `page` points at
the chain head.

### 5.4 RecordId directory (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

The directory is held in RAM as a `Vec<RecLoc>` while the file is open (so a
RecordId lookup is an O(1) index) and is persisted as a chain of `PT_DIR` pages
(starting at `dir_head`) on close. The B+trees store RecordIds, never physical
addresses, so a record can be moved on disk without touching any index.

---

## 6. The record image itself (positional, no names)

A record on disk is a single **fixed-width byte buffer** laid out by field
*offset* — there are no field names, tags, or delimiters in the payload. For:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

the stored image is 23 bytes:

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- `RecordLayout::materialize()` packs the FD's elementary items into this buffer
  by offset for `WRITE`/`REWRITE`; `RecordLayout::distribute()` reverses it on
  `READ`. The field → offset map lives only in the program's `RecordLayout`
  (derived from the `FD`), **never** in the file.
- **Identity is position.** This is the limit case of "don't repeat keys per
  record": field identity costs *zero* bytes per record, and field access is O(1)
  by precomputed offset (no parsing). Renaming a non-key field changes nothing on
  disk; renaming a key field rewrites only the header's key schema, not the
  records or indexes. Changing a field's offset/width is the one change that
  requires rewriting the data — inherent to fixed-length records (and to real
  ISAM/VSAM).

### Compression

With `STORAGE IS DISK WITH COMPRESSION`, the **stored** payload is PackBits-RLE
compressed (`compress.rs`), and `RecLoc.len` is the *stored* length; the buffer
is expanded back to `record_len` on read. Compression is transparent to the key
geometry and the access path.

---

## 7. Free space & reuse

- **Free list.** `free_list_head` chains pages reclaimed from emptied data pages,
  split-orphaned nodes, etc.; `allocate` pops from it before bumping
  `next_page_id`, so space is reused and the file does not grow monotonically.
- **Tombstones.** A `DELETE` frees the slot (and lazily the data page) and marks
  the directory entry `RecLoc::FREE`; the RecordId is retired.

---

## 8. Transactions (in-run undo log)

The disk engine keeps an **undo log** of inverses for every mutation since the
last `COMMIT`/`OPEN`:

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- `OPEN` starts a transaction (clears the log); `COMMIT` makes changes durable
  and starts a new one; `ROLLBACK` replays the inverses in reverse order; `CLOSE`
  flushes (implicit commit). A `tx_replay` guard stops the inverse operations
  from re-logging themselves.
- This is **program-level** rollback. Crash-recovery via a durable write-ahead
  log is future work. See the COBOL `COMMIT`/`ROLLBACK` verbs in the language
  reference; note those verbs act on **INDEXED files**, not SQL connections.

---

## 9. OPEN validation

On `OPEN`, the header's stored key schema is compared against the program's
`SELECT` (record length, key count, each key's parts and duplicate policy). A
mismatch returns COBOL file status `39`; a missing file opened `INPUT` returns
`35`; a corrupt/short header returns `90`. (Strict validation can be relaxed via
the engine's `strict_metadata` flag.)

---

## 10. Quick reference — who stores what

| Thing                         | Where it lives                         | Copies      |
|-------------------------------|----------------------------------------|-------------|
| Key geometry (offsets/widths) | Header (page 0) key schema             | once        |
| Data-field names              | Program `FD` only                      | not in file |
| Record bytes                  | `PT_DATA` / `PT_OVERFLOW` pages         | one/record  |
| key → RecordId                | one B+tree per key                     | one/key     |
| RecordId → physical location  | RecordId directory (`PT_DIR` chain)    | one/record  |
| Free pages                    | Free list (`free_list_head`)           | —           |
| Uncommitted change inverses   | In-RAM undo log                        | per-tx      |
```
