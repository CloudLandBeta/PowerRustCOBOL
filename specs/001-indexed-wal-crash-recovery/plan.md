# Plan — INDEXED crash recovery to the last `COMMIT`

- **Status:** implemented. Rewritten 2026-10-15 to the plan that was carried out; the draft of
  2026-06-15 (a logical sidecar WAL, and `redb` as the default engine) is superseded — see §4.
- **Spec:** ./spec.md   **Date:** 2026-06-15   **Rewritten:** 2026-10-15

## 1. Approach

Two tracks, independently testable.

### Track A — the `redb` engine (shipped 1.10.0–1.11.0; no work in this change)

redb already provides durable `COMMIT`/`ROLLBACK` through its ACID write transactions
([`indexed_redb.rs`](../../crates/cobolt-runtime/src/indexed_redb.rs)). Making it the default
(1.62.73) was **reversed by the operator on 2026-09-14 (1.70.24)** because a redb container takes
`flock(LOCK_EX)` for a writer: exactly one process may write one, where `PRCIDXD1` admits
concurrent readers and writers. `IndexedEngine::default()` is `Rust`. This change's work for the
track is a test that holds `redb` to the same crash requirement as the default engine
(`indexed_crash_recovery`, §6) — it passed without a change to the engine.

### Track B — `PRCIDXD1` returns to the last `COMMIT` after a crash (this change)

What the draft assumed, and what the code was. The draft proposed a sidecar *logical* WAL. The
engine already had, since 1.70.32, a **physical undo journal**: a transaction's changed pages are
held in a write set (`dirty`) in memory; at a commit point the *previous* image of every one of
them is written to `<container>.jrn` and made durable, the pages and the header are written, and
**removing the journal is the commit point**. A crash anywhere inside that sequence is undone in
full on the next `OPEN`. Measured against the spec, that satisfies R1–R5 for any transaction that
fits the write set — and fails them for one that does not: when the write set reached
`CHECKPOINT_PAGES` (256 pages, 1 MiB) the engine took a **commit point of its own**, so a crash
found every such slice durable though the program had never committed it. On 1.91.175, 2 500
uncommitted `WRITE`s (630 pages) and a `SIGKILL` left **2 194** records; `redb` left the three
that were committed.

The change is to make a slice a **spill** instead of a commit point:

| Piece | Detail |
|---|---|
| `tx_spill()` / `tx_flush(spill)` | Journals the write set, writes it to the container, syncs — and **keeps** the journal. `tx_commit()` is the same call with `spill = false`: it finishes by removing the journal. |
| Journal `PRCJRN02` | `magic · two checksummed slots (seq, pages, certified) · records (id, crc32, 4 KiB image)`. `pages` is the container's length when the transaction began; recovery cuts the file back to it. |
| What is journaled | The image a page had *when the transaction began*, once. A page past the original end of the container needs none (a bulk load that adds 254 pages journals 3). A page already journaled is never journaled again from the intermediate state a spill left. |
| Certification | After a batch of records is appended and `fsync`ed, `certified` is advanced in the slot the newer one is not, and `fsync`ed again — *before* the container is touched for the batch. A one-shot commit journal (the common case) is never certified and costs what it cost. |
| Recovery | Replay every valid record newest-first, cut the file, sync, remove. A torn tail is ignored. Fewer valid records than `certified`, or a header that is neither readable nor all-zero → `OPEN` answers 90 and touches nothing. `PRCJRN01` journals are still read. |
| Durability of the directory entry | A journal that outlives its first commit point is created, and later removed, with the directory entry synced too. |

## 2. Affected files

| File | Change |
|---|---|
| `crates/cobolt-runtime/src/indexed_disk.rs` | The journal section, `op_finish`, `tx_commit`/`tx_spill`/`tx_flush`, recovery for both journal forms, `needs_undo_image`, 14 unit tests, two test-only crash helpers |
| `crates/cobolt-runtime/src/indexed.rs` | Doc comment of `IndexedEngine::Rust` (crash safety is not what it trades away) |
| `crates/cobolt-cli/tests/indexed_crash_recovery.rs` | New: real `rcrun` processes and `SIGKILL`, both engines |
| `docs/developers-guide-en.md`, `docs/cobol85-supported-syntax-en.md`, `docs/indexed-file-internals-en.md`, `docs/indexed-redb-engine-en.md` | The guarantee, the notes, the engine table |
| `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` | 1.91.176 |

## 3. Data / model changes

| Artifact | Change |
|---|---|
| `PRCIDXD1` container | **Unchanged.** |
| `<container>.jrn` | New form `PRCJRN02`; `PRCJRN01` still recovered. Exists for the life of a transaction that has changed the container, not for the length of one commit. |
| redb file | Unchanged. |
| `IndexedEngine` default | Unchanged: `Rust`. |

## 4. Decisions and alternatives

| Decision | Why | Rejected |
|---|---|---|
| Keep the physical undo journal and let it span the transaction | It is what the engine already had, it protects against a half-written container whatever operation wrote it, and it makes R3 true with a change of one function's meaning | A logical WAL of records: replaying it needs the B+tree code to run on a half-written file |
| Certify with a second `fsync` only for journals that outlive a commit point | A one-shot commit journal cannot be truncated by a crash (the container is untouched until it is durable) | Certifying every journal: +1 `fsync` per `COMMIT` for no gain |
| Two slots for the certified count | A torn in-place update of one slot leaves the other | One slot: a crash during the update could fail an `OPEN` of a sound file |
| Do not hold the whole transaction in memory instead | Bounded memory is why the engine spills at all | Raising `CHECKPOINT_PAGES`: moves the cliff, does not remove it |
| Leave `redb` opt-in | Operator ruling 2026-09-14 (concurrency) | Re-promoting it |

## 5. Risks

| Risk | Mitigation |
|---|---|
| A long transaction's journal grows with the pages of *existing* data it changes (4 KiB each) | Documented in the Guide; appends are cheap by construction (3 images for 254 new pages) |
| A journal removal that is not durable would undo a committed transaction after a power cut | The directory is synced after removing a journal that outlived a commit point |
| An old release meeting a `PRCJRN02` journal deletes it as unknown | Only a crash leaves one, and only a downgrade after a crash meets it; the container format is unchanged, so no version gate is possible. Stated in the CHANGELOG |
| Performance of a commit-heavy program | Unchanged path (§ spec 8) |

## 6. Test strategy

- Unit (`indexed_disk`): spill + crash undone; commit after a spill survives; rollback after a spill;
  a bulk load keeps a small journal; an update transaction undone **to the byte**; a crash in the
  middle of a later spill; the journal cut below its certified records, a flipped bit in a certified
  image, a torn tail, a damaged header, a zero-filled header.
- Real processes (`cobolt-cli/tests/indexed_crash_recovery.rs`): kill between `COMMIT`s; `ROLLBACK`
  vs kill; a 630-page uncommitted transaction; new work after a kill; a damaged journal (`90`, files
  byte-identical, then restored); `MEMORY WITH PERSISTENCE`. `rust` and `redb` run the same program.
