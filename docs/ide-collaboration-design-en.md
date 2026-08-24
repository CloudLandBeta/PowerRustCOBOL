<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL IDE — Collaboration (Phase B) — Design

> **Status: design only.** Nothing here is implemented yet. Phase A (the
> controlled project tree, read-only/blue generated code, toolbar build/run/
> debug, and compile-gating) is built; this document designs the *multi-developer
> collaboration* layer behind a **pluggable backend**, so we can start with a
> trivial local backend and grow into Google Drive / GitHub / git without
> rewriting the IDE.

## 1. Goals & non-goals

**Goals**
- Several developers edit the same project, each on their own machine.
- A file being edited by one developer **locks** for the others: the second
  developer is **warned once** on open and gets the file **read-only**.
- When the first developer **releases** a file (closes the editor / loses the
  lock), the IDE **offers** the waiting developers a re-open in read/write.
- Changes a developer commits are **propagated** to the other IDE instances
  reasonably promptly.
- The transport is **pluggable** — local-only, local git, GitHub, Google Drive,
  … selected per project, with the same IDE behavior on top.

**Non-goals (explicitly out of scope)**
- **Concurrent character-level co-editing** (Google-Docs / CRDT style). We use
  **pessimistic, file-level locking** — one writer per file at a time. This
  matches the requirement ("warn and do not allow … read only") and keeps COBOL
  source authoritative and diff-friendly.
- A bespoke always-on server (unless a future backend chooses to add one).

---

## 2. The pluggable backend — `SyncBackend`

All collaboration goes through one trait. The IDE core never names a specific
service; a backend is chosen per project (stored in `cobolt.toml`).

```rust
/// Identity of a developer in a collaboration session.
pub struct Peer { pub id: String, pub display_name: String }

/// A file lock held by a peer.
pub struct Lock { pub rel_path: String, pub holder: Peer, pub since: SystemTime }

/// Events a backend pushes up to the IDE (lock changes, remote edits, presence).
pub enum SyncEvent {
    LockAcquired(Lock),
    LockReleased { rel_path: String },
    FileChanged  { rel_path: String, by: Peer }, // remote saved a new version
    PeerJoined(Peer),
    PeerLeft(Peer),
    Error(String),
}

pub trait SyncBackend: Send {
    /// Human label + capabilities (does it support real-time push? locking?).
    fn capabilities(&self) -> Capabilities;

    /// Connect / open the shared project. Returns the initial lock table.
    fn connect(&mut self, project: &ProjectRef, me: &Peer) -> Result<Vec<Lock>, SyncError>;

    /// Try to take the write lock for `rel_path`. `Ok(None)` = granted;
    /// `Ok(Some(lock))` = already held by someone else (open read-only).
    fn try_lock(&mut self, rel_path: &str) -> Result<Option<Lock>, SyncError>;

    /// Release a lock we hold (on editor close / explicit unlock / app exit).
    fn release(&mut self, rel_path: &str) -> Result<(), SyncError>;

    /// Publish a new version of a file we hold the lock on.
    fn push_change(&mut self, rel_path: &str, bytes: &[u8]) -> Result<(), SyncError>;

    /// Fetch the latest bytes of a file (to refresh a read-only view).
    fn fetch(&mut self, rel_path: &str) -> Result<Vec<u8>, SyncError>;

    /// Drain backend events since the last poll (non-blocking). Backends that
    /// support push deliver promptly; polling backends synthesise these.
    fn poll(&mut self) -> Vec<SyncEvent>;
}

pub struct Capabilities {
    pub realtime: bool,      // true = push; false = the IDE must poll
    pub locking:  LockKind,  // Native | Advisory | None
    pub auth:     AuthKind,  // None | OAuth | Token | FsPermissions
}
```

- The IDE talks only to `SyncBackend` + drains `poll()` each frame into UI state.
- Backends that can't push (git, Drive) implement `poll()` by checking the remote
  on an interval (e.g. 2–5 s) and emitting synthetic events.
- `Capabilities` lets the UI adapt (e.g. show "locking is advisory" or "near
  real-time" badges), and lets us **degrade gracefully** when a backend lacks a
  feature.

---

## 3. The lock & propagation model (backend-agnostic)

This is the behavior the IDE enforces on top of any backend.

### Opening a file
1. IDE calls `try_lock(rel)`.
2. `Ok(None)` → open **read/write**; mark the tab "locked by me".
3. `Ok(Some(lock))` → **warn once** ("`{file}` is being edited by
   `{holder}` — opening read-only"), open the tab **read-only**, and remember
   we're *waiting* on `rel`.

### Editing / saving
- Saves on a write-locked file call `push_change(rel, bytes)`.
- The backend propagates; other IDEs receive `FileChanged` and, if they have the
  file open read-only, refresh the view (and the tree marks it updated).

### Releasing
- On editor-close / app-exit / explicit unlock, IDE calls `release(rel)`.
- Other IDEs receive `LockReleased`. For any developer *waiting* on `rel`, the
  IDE pops a prompt: **"`{file}` is now free — edit it?"** → Yes re-acquires the
  lock and switches the tab to read/write.

### Crash / disconnect safety
- Locks carry a **holder + timestamp** and a **lease TTL**. A backend (or the
  IDE) expires a stale lock after the TTL so a crashed editor can't block a file
  forever. (Generated code is never lockable — it's read-only for everyone.)

> Generated COBOL and Assets are read-only/binary; only **Common Code**,
> **Forms**, and **Documentation** participate in locking.

---

## 4. The four backends

All four implement the same trait; they differ only in *where the project of
record lives* and *how locks + changes travel*.

| Backend | Project of record | Locking | Propagation | Auth | Notes |
|---------|-------------------|---------|-------------|------|-------|
| **Local-only** | the local folder | in-process only (single machine, multiple windows) | direct | none | The trivial default. Validates the whole UX with zero infra; no cross-machine sync. |
| **Local git** | a git repo (possibly on a shared path/LAN remote) | **advisory lock refs** (a `refs/locks/<path>` or a `.cobolt/locks/` file committed/pushed) | commit + push on save; fetch on poll | ssh/https creds | Familiar, auditable history; "immediacy" = poll interval. |
| **GitHub** | a GitHub repo | a lock branch/file via the API (or **GraphQL/Issues**-based lock registry); optional GitHub-App webhooks for push | commits via the API; webhook → near-real-time, else poll | **OAuth / PAT** | Hosted, no infra to run; rate-limited; webhooks need a small relay for true push. |
| **Google Drive** | a Drive folder | a lock file (`<path>.lock` doc) or Drive's **content-restriction / file-locking** API | upload new revision on save; Drive **changes feed** on poll (or push notifications) | **OAuth** | Easy sharing for non-developers; Drive change notifications give near-real-time. |

Design implications baked into the trait:
- **Locking is `LockKind`** because git/Drive/GitHub give *advisory* locks (a
  convention everyone honors), not OS-enforced ones. The IDE treats advisory
  locks as authoritative *as long as every client is a PowerRustCOBOL IDE*.
- **Propagation is `realtime` or polled** — git is polled; Drive/GitHub can be
  near-real-time with their change feeds/webhooks; local-only is instant.
- Each backend serializes the lock table the same way (a small JSON/TOML
  `locks` document) so switching backends doesn't change the IDE.

---

## 5. Where state lives

- **`cobolt.toml`** gains a `[collaboration]` section:
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **Lock registry**: a single small document the backend owns (`.cobolt/locks.toml`
  in the repo/folder, or an API-side record), shape:
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identity**: a `Peer { id, display_name }` from the IDE settings (and, for
  OAuth backends, the authenticated account).

---

## 6. IDE-side integration points (Phase A already prepared)

- The **tree** categories that participate in locking are already isolated
  (Forms / Common Code / Documentation), and **generated code is read-only**
  for everyone — no lock needed.
- The **editor** already supports a per-tab `read_only` flag (used today for
  generated code); the collaboration layer reuses it for "locked by someone
  else", plus a one-time warning and a tab badge (`🔒 by {name}`).
- A new **`SyncManager`** (holds a `Box<dyn SyncBackend>`) is owned by the app,
  drained each frame into: tab read-only states, the warn-once set, the
  "waiting" set (for the re-offer prompt), and a presence list.

---

## 7. Phased rollout

1. **B0 — Local-only backend + the whole UX.** Implement `SyncBackend`,
   `SyncManager`, the warn-once/read-only/re-offer flow, tab badges — all against
   a trivial in-process backend (multiple IDE windows on one machine). This
   proves the model with zero infrastructure.
2. **B1 — Local git backend.** Advisory lock refs + commit/push on save +
   fetch-on-poll. First real cross-machine collaboration.
3. **B2 — GitHub backend.** API-based repo + lock registry; optional webhook
   relay for near-real-time.
4. **B3 — Google Drive backend.** OAuth + lock files + Drive changes feed.

Each phase is shippable on its own; the IDE behavior is identical across them.

---

## 8. Open questions (to resolve before B1)

- **Identity/auth UX**: how do developers sign in per backend (PAT paste vs
  OAuth browser flow), and how is `Peer.id` kept stable?
- **Granularity**: file-level locks only, or also lock a Form's generated output
  implicitly when its `.cfrm` is locked? (Recommendation: lock the `.cfrm`; its
  generated `.cbl` is already read-only.)
- **Conflict policy** when advisory locks are bypassed (someone edits outside the
  IDE): last-writer-wins with a visible "changed on disk/remote" banner.
- **Offline editing**: queue `push_change` and reconcile on reconnect, or block
  saves while disconnected?

---

## 9. Why pessimistic locking (not CRDT)

The requirement is explicit: a second developer must be **warned and blocked**
(read-only), not merged in live. Pessimistic, file-level locking:
- matches that requirement exactly,
- keeps COBOL source a clean, reviewable artifact (real diffs, no CRDT metadata),
- works over *any* of the four backends with the same semantics, and
- is dramatically less complex/risky than real-time CRDT convergence.

If true concurrent co-editing is ever wanted, it would be a separate, additive
mode — it does not block this design.
