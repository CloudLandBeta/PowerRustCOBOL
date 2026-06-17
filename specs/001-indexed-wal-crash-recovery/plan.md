# Plan — INDEXED crash recovery + redb as default

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-15

## 1. Approach

Deliver R1–R16 in **two tracks** that can land in one release but are independently
testable:

### Track A — Make redb the default (closes #1 for the common case)

redb already provides durable `COMMIT`/`ROLLBACK` and crash recovery via its ACID
write transactions ([`indexed_redb.rs`](../../crates/cobolt-runtime/src/indexed_redb.rs)).
Change the runtime default so `STORAGE IS DISK` uses redb without flags.

| Change | Detail |
|--------|--------|
| Default enum | `IndexedEngine::default()` → `Redb`; update `parse("default")` tests |
| Interpreter | `Interpreter::indexed_engine` field default → `Redb` |
| CLI | `rcrun` help text: redb is default; `rust` opens `PRCIDXD1` |
| Docs | README, developers-guide §14, `indexed-redb-engine.md`, `cobol85-supported-syntax.md` |

**Compatibility:** redb and `PRCIDXD1` use **different on-disk formats** at the same
`ASSIGN` path. Existing `PRCIDXD1` files **cannot** be opened by redb. Programs with
legacy files must pass `--indexed-engine rust` until recreated or migrated. Document
this prominently; detect `PRCIDXD1` magic on `OPEN` with redb selected and return
status **`39`** (or **`90`**) with a clear tracing message.

### Track B — Durable WAL for legacy `PRCIDXD1` (`--indexed-engine rust`)

For users who keep the bespoke engine, add a **write-ahead log** so R3–R5 hold
without relying on the in-memory `undo` vec alone ([`indexed_disk.rs`](../../crates/cobolt-runtime/src/indexed_disk.rs)
§undo).

High-level WAL design (to detail in tasks):

1. **Sidecar WAL** at `<assign-path>.wal` (keeps `PRCIDXD1` container unchanged;
   resolves Q1; backup semantics are copy data + wal together).
2. Log **logical operations** (insert/update/delete with record image + key) at
   mutation time; mark **committed** on COBOL `COMMIT` / `CLOSE`.
3. On `OPEN`, if WAL has uncommitted entries after crash, **discard** them
   (rollback to last commit boundary — matches AC1).
4. `ROLLBACK` in-process continues to use the in-memory undo log (fast path); WAL
   tracks the same boundaries for crash recovery.
5. `fsync` WAL + data header on `COMMIT` (Q5: one fsync per COBOL `COMMIT` v1).

Profile Q3 during implementation: today btree/data pages may be written before
`COMMIT`; WAL must make recovery correct regardless (redo only committed entries,
or shadow writes until commit).

### MEMORY (R11–R12)

No WAL. Ephemeral MEMORY unchanged. `WITH PERSISTENCE`: reload last `CLOSE`
snapshot on reopen after crash (verify / fix if gap).

## 2. Affected crates / files

| File | Track | Change |
|------|-------|--------|
| `crates/cobolt-runtime/src/indexed.rs` | A | `IndexedEngine::default()`, `#[default]` on `Redb` |
| `crates/cobolt-runtime/src/interpreter.rs` | A,B | Default engine; optional format sniff on OPEN |
| `crates/cobolt-runtime/src/indexed_disk.rs` | B | WAL module integration, recovery on OPEN, COMMIT fsync |
| `crates/cobolt-runtime/src/indexed_redb.rs` | A | Module docs (no longer “opt-in”) |
| `crates/cobolt-runtime/src/indexed_log.rs` | A | Unchanged; redb log already supported |
| `crates/cobolt-cli/src/main.rs` | A | Help text, default engine resolution |
| `crates/cobolt-runtime/tests/test_indexed_redb.rs` | A | Run fixtures **without** `--indexed-engine redb` |
| `crates/cobolt-runtime/tests/test_transactions.rs` | A,B | Default + rust engine paths |
| New `crates/cobolt-runtime/tests/test_indexed_wal_crash.rs` | B | kill -9 child process harness |
| `tests/cobol/fileio/*.cbl` | A,B | Document engine flags where needed |
| `docs/developers-guide-en.md` §14 | A,B | Default engine, engine table, crash guarantee |
| `docs/indexed-redb-engine.md` | A | Default, not opt-in |
| `docs/indexed-file-internals.md` §8 | B | WAL replaces “future work” for rust engine |
| `docs/cobol85-supported-syntax.md` | A,B | COMMIT/ROLLBACK bullet |
| `README.md` | A | Indexed engine default, `--indexed-engine rust` for legacy |
| `CHANGELOG.md` + `version.rs` | A,B | Feature minor bump |

## 3. Data / model changes

| Artifact | Change |
|----------|--------|
| `PRCIDXD1` container | **Unchanged** (WAL is sidecar) |
| `<assign>.wal` | **New** sidecar for rust engine; created on first mutating OPEN |
| redb database file | Becomes default on-disk shape for new DISK INDEXED files |
| `IndexedEngine` default | `Rust` → `Redb` |
| `parse("default")` | Maps to `Redb` (keep `rust`/`native` aliases → `Rust`) |

No `.cfrm` / codegen changes. No `cobolt.toml` schema change.

## 4. Key decisions & alternatives

| Decision | Why | Rejected |
|----------|-----|----------|
| **redb as default** | Already crash-safe, scales to large files; closes #1 immediately for new work | Building WAL first then switching default later (slower user benefit) |
| **WAL sidecar for `PRCIDXD1`** | Avoids `PRCIDXD2` migration of data pages (Q2 deferred) | Embedded WAL region in page 0 (tight space, complex) |
| **Keep `rust` engine** | Existing `PRCIDXD1` deployments + zero redb dependency path for debugging | Remove bespoke engine entirely (breaks legacy files) |
| **No auto PRCIDXD1→redb migration** | Large scope; risk of silent data transform | Auto-convert on OPEN (follow-up feature) |
| **Discard uncommitted WAL on crash** | Matches spec AC1 / user expectation | Replay uncommitted ops (would violate commit boundary) |

## 5. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Legacy `PRCIDXD1` files break when default switches to redb | Docs + `--indexed-engine rust`; optional OPEN sniff with clear status `39`/`90` |
| WAL + early page writes race (Q3) | Integration tests AC1–AC4; WAL records logical ops, recovery replays only committed epochs |
| redb adds ~dependency surface | Already in workspace; default aligns with “safety paramount” product goal |
| Performance regression on small files | redb OPEN is faster; WRITE ~44 µs/record documented; accept or tune cache |
| Test flakiness on kill -9 | Dedicated child-process harness with tempdir; retry policy in test only |

## 6. Test strategy

### Track A (redb default)

- `cargo test -p cobolt-runtime test_indexed_redb` — run with **no** engine flag.
- `idx_crud.cbl`, `idx_tx.cbl`, `idx_persist.cbl` via `rcrun` default → identical DISPLAY.
- AC1 crash test with **default** engine (committed survives, uncommitted discarded).
- Assert `IndexedEngine::default() == Redb`.

### Track B (PRCIDXD1 WAL)

- AC1–AC4 with `--indexed-engine rust` + kill -9 between WRITE B and COMMIT.
- AC7 corrupt/truncate `.wal` → OPEN status `90`.
- Existing `test_transactions.rs` green on rust engine post-WAL.
- `WITH COMPRESSION` file: WAL stores uncompressed images (R/Q6).

### Manual

- Open developers-guide §14; confirm engine table shows redb default.
- Open existing `PRCIDXD1` sample with default engine → expect clean failure message;
  repeat with `--indexed-engine rust` → succeeds.

## 7. Steering compliance

- [x] i18n: no new IDE UI strings
- [x] Generated-code contract: unchanged
- [x] English docs: developers-guide §14, indexed-* docs, cobol85-supported-syntax, README, CHANGELOG
- [x] Fix vs feature: **feature** → minor version bump
- [x] Verify-first: crash tests must actually kill child process
- [x] No "cobolt" in user-facing text