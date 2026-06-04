# Storage / Compression Indexed File Test Variants

This package now contains six COBOL test programs for indexed-file storage and compression coverage.

| Set | Program | Indexed storage clause | Indexed output directory |
|---:|---|---|---|
| 1 | `tests/cobol/fileio/fileiot_storage_disk.cbl` | `STORAGE IS DISK` | `tests/cobol/fileio/INDEXED/STORAGE-DISK/` |
| 2 | `tests/cobol/fileio/fileiot_storage_disk_compression.cbl` | `STORAGE IS DISK WITH COMPRESSION` | `tests/cobol/fileio/INDEXED/STORAGE-DISK-COMPRESSION/` |
| 3 | `tests/cobol/fileio/fileiot_storage_memory.cbl` | `STORAGE IS MEMORY` | `tests/cobol/fileio/INDEXED/STORAGE-MEMORY/` |
| 4 | `tests/cobol/fileio/fileiot_storage_memory_compression.cbl` | `STORAGE IS MEMORY WITH COMPRESSION` | `tests/cobol/fileio/INDEXED/STORAGE-MEMORY-COMPRESSION/` |
| 5 | `tests/cobol/fileio/fileiot_default_disk.cbl` | no `STORAGE` clause; default is disk, no compression | `tests/cobol/fileio/INDEXED/DEFAULT-DISK/` |
| 6 | `tests/cobol/fileio/fileiot_default_compression.cbl` | no `STORAGE` clause; `WITH COMPRESSION` shorthand, default storage is disk | `tests/cobol/fileio/INDEXED/DEFAULT-DISK-COMPRESSION/` |

The original `tests/cobol/fileio/fileiot.cbl` is kept as the baseline/default test and now includes the same write/read performance profile changes.

## Intended grammar for the extension

The six sets assume the `SELECT` grammar accepts this form for indexed files:

```cobol
SELECT file-name
    ASSIGN TO external-file-name
    ORGANIZATION IS INDEXED
    [STORAGE IS { MEMORY | DISK }] [WITH COMPRESSION]
    ACCESS MODE IS { SEQUENTIAL | RANDOM | DYNAMIC }
    RECORD KEY IS data-name
    [ALTERNATE RECORD KEY IS data-name [WITH DUPLICATES]]
    [FILE STATUS IS data-name].
```

Semantic expectations:

- If the `STORAGE` clause is omitted, storage defaults to `DISK`.
- If `WITH COMPRESSION` appears without a `STORAGE` clause, the file uses the default storage backend, therefore disk with compression.
- Compression is transparent to COBOL logic. Keys are evaluated from the logical uncompressed record.
- `STORAGE IS MEMORY` keeps records and indexes in memory and flushes to disk at `COMMIT` or `CLOSE`.
- `STORAGE IS DISK` persists changes at `WRITE`, `REWRITE`, and `DELETE` time.

## Run examples

```sh
./target/debug/rcrun run tests/cobol/fileio/fileiot_storage_disk.cbl
./target/debug/rcrun run tests/cobol/fileio/fileiot_storage_disk_compression.cbl
./target/debug/rcrun run tests/cobol/fileio/fileiot_storage_memory.cbl
./target/debug/rcrun run tests/cobol/fileio/fileiot_storage_memory_compression.cbl
./target/debug/rcrun run tests/cobol/fileio/fileiot_default_disk.cbl
./target/debug/rcrun run tests/cobol/fileio/fileiot_default_compression.cbl
```

## Coverage in each variant

Each variant preserves the same logical indexed-file coverage:

- alphanumeric primary key
- alphabetic-only primary key content
- numeric DISPLAY primary key
- uppercase, lowercase, and mixed-case key content
- alternate keys with duplicates
- alternate keys without duplicates
- duplicate primary key detection
- duplicate alternate key detection
- random `READ` by primary key
- random `READ` by alternate key
- `START` with equality
- `START` with greater-than-or-equal
- `START` invalid key
- `READ NEXT`
- `REWRITE`
- `DELETE`
- `READ` after `DELETE` invalid-key path
- performance/profile creation of 1,000,000 indexed records with a 40-byte key and 1024-byte record size
- keyed read performance pass over the same 1,000,000 records after the write phase


## Performance phases

Each indexed-file performance variant now reports three separate measurements:

1. `INDEXED WRITE PERFORMANCE STATISTICS` — writes 1,000,000 records.
2. `INDEXED READ PERFORMANCE STATISTICS` — performs 1,000,000 keyed reads by the 40-byte primary key.
3. `INDEXED SCAN PERFORMANCE STATISTICS` — reopens the indexed file and scans all records using `READ NEXT`.

The scan test measures indexed sequential traversal performance, while the read test measures random/keyed lookup performance.
