<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL Observability

This is the home for everything about **observing** a running RustCOBOL program —
what it did, how fast, and how healthy the underlying stores are. It starts with
**indexed-file transaction logs** and will grow to cover other runtime surfaces.

| Surface | Status | Where |
|---------|--------|-------|
| **INDEXED file transaction log** | ✅ available | this document, §1 |
| Runtime tracing (`COBOLT_LOG`) | ✅ available | §2 |
| SQL database runtime | 🔭 planned | — |
| HTTP / REST client | 🔭 planned | — |

> **Guiding principle.** Observability is *passive*: enabling any of it must never
> change program behavior or results. Log/trace errors are swallowed, and the
> hot paths stay hot (anything expensive is opt-in and called sparingly).

---

## 1. INDEXED file transaction log

The crash-safe **redb** indexed engine can write a per-file log of every
transaction — useful for diagnostics, capacity planning, and dashboards. It is
**off by default** and specific to the redb engine
(`--indexed-engine redb`; see [`indexed-redb-engine.md`](indexed-redb-engine.md)).

### 1.1 Enabling it

| Flag / env | Values | Meaning |
|------------|--------|---------|
| `--indexed-log` / `COBOL_INDEXED_LOG` | `off` (default), `basic`/`true`, `full` | Log level |
| `--indexed-log-format` / `COBOL_INDEXED_LOG_FORMAT` | `text` (default), `json` | Line format |

```bash
# logfmt, per-transaction metrics
rcrun run app.cbl --indexed-engine redb --indexed-log basic

# NDJSON + index page stats on close (for Grafana/Loki)
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

- **`basic`** — per-transaction metrics only (cheap, self-tracked).
- **`full`** — `basic` plus redb index statistics on each `CLOSE`. Those stats
  **walk the index**, so their cost scales with file size; that is why `full` is
  opt-in and the stats are emitted only on CLOSE (never per commit).

### 1.2 Location

Each indexed file gets a **sidecar log next to its data file**, named by
appending `.log` to the `ASSIGN` path:

```
customers.idx        →  customers.idx.log
/var/data/orders.dat →  /var/data/orders.dat.log
```

Lines are **appended** (never truncated), so a log accumulates across runs.

#### Rotation (kept under 100 KiB)

To keep any single file small, the active log is **rotated** once it approaches
**100 KiB** (`MAX_LOG_BYTES`), logrotate/Grafana style:

1. the active `<datafile>.log` is renamed to
   **`<user|no-user>.<datafile>.log.<timestamp>`**, and
2. a fresh, empty active log is started.

The timestamp is a compact UTC stamp, e.g. `20260610T120230461Z`. The `<user>`
is the `OPEN … WITH REGISTERED USER` value (sanitized for the filesystem), or
**`no-user`** when none was supplied. Example after one rotation:

```
customers.idx.log                                 # active (< 100 KiB)
alice.customers.idx.log.20260610T120230461Z       # rotated archive (~100 KiB)
no-user.orders.dat.log.20260610T120051301Z        # rotated, no user supplied
```

Rotated files are never deleted by the runtime — prune or ship them with your
log pipeline (e.g. Promtail then delete). Each archive is a complete, parseable
log on its own.

### 1.3 What is recorded

One line per **transaction event**: `OPEN`, `COMMIT`, `ROLLBACK`, `CLOSE`.

| Field | Type | Meaning |
|-------|------|---------|
| `ts` | string | ISO-8601 UTC timestamp, ms precision (`2026-06-10T07:30:00.123Z`) |
| `file` | string | the indexed file name |
| `user` | string | the registered user (present only when supplied — see §1.3.1) |
| `tx` | number | transaction counter (**per OPEN session**) |
| `kind` | string | `OPEN` / `COMMIT` / `ROLLBACK` / `CLOSE` |
| `writes` | number | `WRITE`s in this transaction |
| `rewrites` | number | `REWRITE`s in this transaction |
| `deletes` | number | `DELETE`s in this transaction |
| `records` | number | total mutations (`writes+rewrites+deletes`) |
| `bytes` | number | record bytes written/rewritten |
| `dur_ms` | number | wall-clock duration of the transaction |
| `rec_per_s` | number | records per second |
| `bytes_per_s` | number | bytes per second |
| `order` | string | `ordered` if written keys were ascending, else `unordered` (`n/a` if no writes) |
| `in_order` | number | count of writes whose key advanced |
| `out_of_order` | number | count of writes whose key went backward |

**`full`-level CLOSE lines** add redb index statistics:

| Field | Meaning |
|-------|---------|
| `tree_height` | primary B+tree height |
| `leaf_pages` / `branch_pages` | page counts |
| `allocated_pages` | pages allocated in the file |
| `stored_bytes` | live record bytes |
| `fragmented_bytes` | free/fragmented space (includes pre-allocated file slack) |
| `page_size` | redb page size (4096) |

> **Why `order` matters.** Ascending-key writes hit one hot B+tree leaf; scattered
> keys touch random leaves (more I/O, more fragmentation). The `order` /
> `in_order` / `out_of_order` fields are an at-a-glance signal of write locality —
> a good proxy for whether a load was sequential or random.

> **`tx` is per session.** The engine is re-created on each `OPEN`, so the
> counter restarts at 1 per OPEN…CLOSE session; the `ts` field disambiguates.

#### 1.3.1 Recording the logged-in user — `OPEN … WITH REGISTERED USER`

COBOL programs rarely sit behind OAuth or any authentication engine, so the
operator/user is supplied **explicitly** on the `OPEN`, as a PowerRustCOBOL
extension:

```cobol
       OPEN I-O CUSTOMER-FILE WITH REGISTERED USER "ALICE"
       OPEN I-O CUSTOMER-FILE WITH REGISTERED USER WS-OPERATOR
```

- The value is a **string literal** or a **data item** (`USER` is optional;
  `WITH REGISTERED "ALICE"` also parses).
- It applies to the whole `OPEN…CLOSE` session: **every** event line for that
  file (`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE`) carries a `user=` field.
- It is purely observational — it does not authenticate or authorize anything,
  and it has no effect when logging is off.

Example log lines (one session per user):

```
ts=…Z file=customers.idx user=ALICE        tx=1 kind=OPEN   …
ts=…Z file=customers.idx user=ALICE        tx=2 kind=COMMIT …
ts=…Z file=customers.idx user=BOB-FROM-WS  tx=1 kind=OPEN   …
```

### 1.4 Formats

#### logfmt (`text`, default)

```
ts=2026-06-10T07:30:00.123Z file=customers.idx tx=2 kind=COMMIT writes=1 rewrites=0 \
   deletes=0 records=1 bytes=12 dur_ms=3 rec_per_s=272 bytes_per_s=3266 \
   order=ordered in_order=1 out_of_order=0
```

String values containing spaces are quoted. Loki parses this with `| logfmt`.

#### NDJSON (`json`)

```json
{"ts":"2026-06-10T07:30:00.123Z","file":"customers.idx","tx":2,"kind":"COMMIT","writes":1,"rewrites":0,"deletes":0,"records":1,"bytes":12,"dur_ms":3,"rec_per_s":272,"bytes_per_s":3266,"order":"ordered","in_order":1,"out_of_order":0}
```

One JSON object per line. **Numeric fields are bare JSON numbers** so Grafana can
graph them directly; string fields are quoted. Loki parses this with `| json`.

### 1.5 Grafana / Loki

Grafana does not read files directly — ship the logs to **Loki** with an agent,
then query. Recommended: `json` format.

1. **Collect** `*.idx.log` with Promtail / Grafana Agent / Alloy → Loki. Keep
   *labels* low-cardinality (e.g. `job`, `file`, `kind`); leave `tx`, `ts`, and
   the numeric metrics as parsed fields.
2. **Query** in Grafana (LogQL):

   ```logql
   # commit throughput over time
   {job="rustcobol"} | json | kind="COMMIT" | unwrap rec_per_s

   # rolled-back work
   sum by (file) (count_over_time({job="rustcobol"} | json | kind="ROLLBACK" [5m]))

   # index growth (full level)
   {job="rustcobol"} | json | kind="CLOSE" | unwrap allocated_pages
   ```

Example Promtail scrape (logfmt is also fine — swap the pipeline stage for
`logfmt`):

```yaml
scrape_configs:
  - job_name: rustcobol
    static_configs:
      - targets: [localhost]
        labels: { job: rustcobol, __path__: /var/data/*.idx.log }
    pipeline_stages:
      - json:
          expressions: { kind: kind, file: file }
      - labels: { kind: kind, file: file }
```

### 1.6 Cost & safety

- `basic` logging adds a few counters per operation and one appended line per
  transaction event — negligible.
- `full` adds an index walk **on CLOSE only**; avoid it on very large files
  unless you want that snapshot.
- Logging never affects program behavior: all log I/O errors are silently
  ignored, and the data path is unchanged.

### 1.7 Implementation

`crates/cobolt-runtime/src/indexed_log.rs` — `LogLevel`, `LogFormat`, the
`LogRecord` builder that renders to logfmt or NDJSON (dependency-free JSON), the
appending `LogWriter`, and a no-dependency ISO-8601 formatter. The
per-transaction accumulators live in
`crates/cobolt-runtime/src/indexed_redb.rs`; the flags are resolved in
`crates/cobolt-cli/src/main.rs` and applied via
`Interpreter::set_indexed_log_level` / `set_indexed_log_format`.

---

## 2. Runtime tracing (`COBOLT_LOG`)

`rcrun` uses the `tracing` framework with an env-filter. Set `COBOLT_LOG` to
raise the verbosity of internal runtime/diagnostic messages (warnings by
default):

```bash
COBOLT_LOG=debug rcrun run app.cbl
COBOLT_LOG=cobolt-runtime=trace rcrun run app.cbl
```

This is developer-facing diagnostic output (to stderr), distinct from the
structured per-file transaction log in §1.

---

## Roadmap

Planned additions, to keep this document the single observability reference:

- **SQL runtime** — per-connection/statement timing and row counts for the
  SQLite/PostgreSQL/MySQL engines (see [`database-runtime.md`](database-runtime.md)).
- **HTTP client** — request/latency/status logging for the REST built-ins.
- **Aggregate run summary** — an optional end-of-run report across all files.
