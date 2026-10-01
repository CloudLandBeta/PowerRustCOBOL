<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL Indexed File Format (`PRCIDX1`)

This document describes the on-disk container that backs
`ORGANIZATION IS INDEXED` files in PowerRustCOBOL, and how it maps to the
metadata a future **Fujitsu COBOL-85 → PowerRustCOBOL importer** will need.

> **Not binary-compatible with Fujitsu.** `PRCIDX1` is PowerRustCOBOL's own
> self-describing container. It is *semantically* modelled on the metadata
> Fujitsu's File Access Subroutines expose via `cobfa_indexinfo()` (record
> format, record length, key count/total length, primary key, alternate keys),
> but it does **not** parse or reproduce Fujitsu `cobidx`/`cobi64` bytes. The
> importer is future work and lives outside PowerRustCOBOL.

Implementation: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Why the format is self-describing

The original container (`PRCISAM1`) stored only a magic, the record length, and
the record bytes — it carried **no key schema**. A converter (or any external
tool) could not tell what the keys were without the COBOL `FD`.

`PRCIDX1` embeds the full schema in the file: the record format and every key's
byte layout, ordering, duplicate policy and (optionally) its COBOL field name.
That makes the file **discoverable** — see [`inspect_path`](#discovery-api) —
and lets a Fujitsu importer write a faithful PowerRustCOBOL file from the
metadata it reads out of a Fujitsu file, without a matching `FD` on hand.

---

## Metadata model

These Rust types (re-exported from `cobolt_runtime`) are the schema. They mirror
`cobfa_indexinfo()` concepts; all offsets and lengths are **byte-based** (never
character counts — matching Fujitsu's Unicode-mode rule).

```rust
pub enum RecordFormat {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

pub enum KeyEncoding {
    Bytes, DisplayAscii, DisplayUtf8,
    Ucs2Le, Ucs2Be, Utf32Le, Utf32Be,
    PackedDecimal, BinaryBigEndian, BinaryLittleEndian,
}

pub enum KeyOrdering { Ascending, Descending }

pub struct KeyPart { pub offset: u32, pub length: u32, pub encoding: KeyEncoding }

pub struct KeyDescriptor {
    pub key_number: u16,          // 1 = primary, 2.. = alternates (declaration order)
    pub name: Option<String>,     // descriptive COBOL field name (optional)
    pub parts: Vec<KeyPart>,      // concatenated → composite key value
    pub duplicates_allowed: bool,
    pub ordering: KeyOrdering,
}

pub struct IndexedFileInfo {
    pub record_format: RecordFormat,
    pub key_count: u16,           // primary + alternates
    pub total_key_length: u32,
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}
```

The current runtime emits **single-part, `Bytes`-encoded, `Ascending`** keys
(that is what a COBOL `FD` `RECORD KEY` / `ALTERNATE RECORD KEY` resolves to).
Composite keys, alternate encodings and descending order are **representable in
the format** so an importer can record them losslessly; full runtime support
for them is future work.

---

## Container layout

All integers are **little-endian**. The file is:

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### Header

| Field            | Type      | Notes                                   |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | reserved (`0`)                          |
| `record_format`  | `u8`      | `1` = fixed, `2` = variable             |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | record length when fixed                |
| `min_length`     | `u32`     | min payload when variable               |
| `max_length`     | `u32`     | max payload when variable               |
| `key_count`      | `u16`     | primary + alternates                    |
| `created_unix_ms`| `u64`     | creation time, preserved across rewrites|
| `updated_unix_ms`| `u64`     | last-write time                         |

### Key schema — repeated `key_count` times (primary first)

| Field          | Type      | Notes                                   |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primary, `2..` alternates           |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` ascending, `1` descending           |
| `part_count`   | `u16`     | number of `KeyPart`s                    |
| `name_len`     | `u16`     | length of the UTF-8 name (`0` = none)   |
| `name`         | `[u8]`    | `name_len` bytes                        |
| `parts`        | repeated  | `part_count` × KeyPart (below)          |

Each **KeyPart**:

| Field      | Type  | Notes                          |
|------------|-------|--------------------------------|
| `offset`   | `u32` | byte offset into record payload|
| `length`   | `u32` | byte length                    |
| `encoding` | `u8`  | `KeyEncoding` discriminant     |
| `reserved` | `u8`  | `0`                            |

### Records

| Field          | Type   | Notes                              |
|----------------|--------|------------------------------------|
| `record_count` | `u64`  | number of live records             |
| per record     | repeat | `length: u32` then `length` bytes  |

Records are written in ascending **primary-key** order.

### Trailer

| Field   | Type  | Notes                                            |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, reflected) over all bytes before the trailer |

The CRC is validated on load; a mismatch yields FILE STATUS `90` (I/O error).

---

## Discovery API

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Returns `Some(IndexedFileInfo)` for a `PRCIDX1` file and `None` for the legacy
`PRCISAM1` container (which carries no schema). This is the `cobfa_indexinfo()`
analog a converter or inspection tool can call.

---

## Open-time validation (FILE STATUS)

When opening an **existing** indexed file for `INPUT` / `I-O`, the runtime
validates the declared `SELECT`/`FD` keys + record format against the stored
schema (strict mode, on by default). Relevant statuses:

| Status | Condition                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` of a non-existent file                   |
| `39`   | existing file's schema ≠ declared keys/record format  |
| `90`   | corrupt container (CRC mismatch) or other I/O error   |

The legacy `PRCISAM1` container has no schema, so strict validation is skipped
for it (it always loads leniently).

---

## Storage modes (`STORAGE IS MEMORY | DISK`)

The `STORAGE MODE` clause selects how a program holds an INDEXED file while it
is open, not how the file is stored: since 1.80.37 both native modes write the
same container, `PRCIDXD1`. **The default storage mode is `DISK`** (when no
`STORAGE` clause is present). `WITH COMPRESSION` applies to either mode;
`WITH PERSISTENCE` applies to `MEMORY` only.

| Mode | Engine | Container written | Notes |
|------|--------|-----------|-------|
| `MEMORY` | in-RAM `BTreeMap` (`indexed.rs`) | `PRCIDXD1` | the whole file loaded on `OPEN`. **Read-only without `WITH PERSISTENCE`**: `OPEN INPUT` only, and `OUTPUT`/`I-O`/`EXTEND` are refused with status 37, the file untouched. With `WITH PERSISTENCE`: writable, `COMMIT` stays in RAM, and the file is rewritten as `PRCIDXD1` on `CLOSE` only. |
| `DISK` (default) | the paged B+tree (`indexed_disk.rs`); the crash-safe redb store (`indexed_redb.rs`) when chosen by name | `PRCIDXD1`, or redb's own | records + indexes read on demand; bounded RAM; always persistent (per-op writes, `fsync` on `COMMIT`/`CLOSE`) |

The **`PRCIDXD1`** disk container is a single paged file (4 KiB pages):

* **page 0** — header: roots (one B+tree per key), free-list head, next page id,
  `RecordId` counter, record count, the key schema, and the compression flag.
* **B+tree pages** — internal / leaf nodes (variable byte-packed, split on
  insert, leaves doubly linked for ordered scans).
* **data pages** — slotted record cells (multiple records per page), plus an
  overflow page chain for records larger than a page.
* **directory pages** — the `RecordId` → physical-location map.
* a **free list** threads freed pages for reuse.

`WITH COMPRESSION` (`compress.rs`) is a dependency-free PackBits-style RLE
applied to each stored record (`PRCIDXD1`, and the records section of a legacy
`PRCIDX1` file); a one-byte tag guarantees the encoding never grows, and the
container header records that compression is on.

> `PRCIDXD1` carries the same key schema in its header (page 0), so either
> container is discoverable. An importer may still produce `PRCIDX1`, the
> simpler single-blob layout described above: PowerRustCOBOL reads it, and
> rewrites it as `PRCIDXD1` the first time a program opens it for writing.

## Backward compatibility

* `PRCIDXD1` (magic `PRCIDXD1`) — the paged B+tree container, and since
  1.80.37 the one format both native modes write (read + write).
* `PRCIDX1` (magic `PRCIDX1\0`) — the self-describing format MEMORY-mode
  programs wrote before 1.80.37. Read only: rewritten as `PRCIDXD1` the first
  time a program opens it for writing, every record kept.
* `PRCISAM1` (magic `PRCISAM1`) — legacy records-only container (read only;
  rewritten as `PRCIDXD1` on the next save).
* An empty file holds no records. Any other content is refused on `OPEN`
  (FILE STATUS 90), never taken for an empty file and never overwritten.

---

## Future Fujitsu import path

The intended migration flow (all outside PowerRustCOBOL's scope today):

```text
Fujitsu runtime
  └─ cobfa_indexinfo()  → record format, record length, key list (primary + alternates)
  └─ sequential export  → record payloads
        │
        ▼
  converter (future, external)
        │  builds IndexedFileInfo + records
        ▼
  PRCIDX1 file  → opened natively by PowerRustCOBOL
```

Because `PRCIDX1` can already *represent* composite keys, key encodings, key
ordering, duplicate policy, variable-length record bounds and key-field names,
the converter only has to translate Fujitsu metadata into `IndexedFileInfo` and
stream the records — no PowerRustCOBOL format change required.

**Do not** attempt to parse raw Fujitsu `cobidx`/`cobi64` bytes. Public Fujitsu
documentation exposes the metadata through the File Access Subroutines but does
not publish the physical byte layout.
