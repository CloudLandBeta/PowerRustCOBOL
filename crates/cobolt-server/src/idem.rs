// SPDX-License-Ilet alive = !dead.contains(&k.1);entifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Idempotency keys, kept per session (WASM-014 R9, R10, Q5).
//!
//! A call may carry a key. The first call with a key runs; its answer is kept under `(caller, session, key)`; a repeat with
//! the same key and the same request in the same session is answered **from the store without running the program again**; a
//! repeat that arrives while the first is still running waits for it; the same key with a different request is refused. One
//! session never sees another's entries, and a caller never sees another caller's. Only a success and a failure the program
//! itself reported are kept: a timeout, a limit or a lost connection must be retryable, so they are forgotten.
//!
//! A **session** is what the client names in its `hello` (32 hex characters it keeps in memory only); with none, the
//! connection is the session. A session ends when the client says `bye`, or when nothing has touched it for the idle time.
//! Nothing is kept in the frontend (Q5): the store is in the backend's memory, or, when the project asks for `server` mode,
//! also in a file the backend writes before it answers, so a restart of the backend does not lose a stored answer.
//!
//! The store is bounded (oldest entries leave first) and may carry a cap on how long an answer is kept.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use cobolt_wire::error::CallError;

/// What a finished call answered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Outcome {
    Reply { args: Vec<Option<String>>, returning: Option<String>, warnings: Vec<String> },
    Failure(CallError),
}

enum Slot {
    Running { fingerprint: String },
    Done { fingerprint: String, outcome: Outcome, at: Instant },
}

/// What `begin` decided.
#[derive(Debug)]
pub enum Begin {
    /// Nothing is stored: run the program, then call `finish`.
    Fresh,
    /// The same request was answered already: this is the answer.
    Replay(Outcome),
    /// The same request is running now: try again shortly.
    Wait,
    /// The key was used with a different request.
    Mismatch,
}

/// Where the store keeps what it holds.
#[derive(Debug, Clone)]
pub enum Backing {
    /// In the backend's memory only: a restart forgets everything.
    Memory,
    /// Also in a file, written in one transaction before `finish` returns: a restart keeps it.
    File(PathBuf),
}

/// How the store is made (the `[contract]` section of `backend.toml`, WASM-014 T3).
#[derive(Debug, Clone)]
pub struct Options {
    /// How long an answer is kept, at most; `None` keeps it until its session ends.
    pub ttl: Option<Duration>,
    pub max: usize,
    /// How long a session may be left untouched before it ends.
    pub idle: Duration,
    pub backing: Backing,
}

impl Options {
    pub fn memory(ttl: Option<Duration>, max: usize) -> Self {
        Options { ttl, max, idle: Duration::from_secs(900), backing: Backing::Memory }
    }
}

type Key = (String, String, String);
type SessionKey = String;

pub struct Store {
    inner: Mutex<Inner>,
    opts: Options,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    disk: Option<Disk>,
}

struct Inner {
    slots: HashMap<Key, Slot>,
    order: Vec<Key>,
    sessions: HashMap<SessionKey, Instant>,
}

/// A fingerprint of a request: the program and every argument's text.
pub fn fingerprint(program: &str, args: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(program.as_bytes());
    for a in args {
        h.update([0u8]);
        h.update(a.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ── the file ────────────────────────────────────────────────────────────────

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

const SLOTS: TableDefinition<&str, &[u8]> = TableDefinition::new("slots");
const SESSIONS: TableDefinition<&str, u64> = TableDefinition::new("sessions");

struct Disk {
    db: Database,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    fingerprint: String,
    outcome: Outcome,
    /// Seconds since the epoch.
    at: u64,
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn flat(k: &Key) -> String {
    format!("{}\u{1}{}\u{1}{}", k.0, k.1, k.2)
}

fn split<const N: usize>(s: &str) -> Option<[String; N]> {
    let parts: Vec<String> = s.split('\u{1}').map(str::to_owned).collect();
    parts.try_into().ok()
}

impl Disk {
    fn open(path: &Path) -> Result<Disk, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let db = Database::create(path).map_err(|e| format!("cannot open the session store {}: {e}", path.display()))?;
        let w = db.begin_write().map_err(|e| e.to_string())?;
        {
            w.open_table(SLOTS).map_err(|e| e.to_string())?;
            w.open_table(SESSIONS).map_err(|e| e.to_string())?;
        }
        w.commit().map_err(|e| e.to_string())?;
        Ok(Disk { db })
    }

    fn put_slot(&self, k: &Key, stored: &Stored) {
        let Ok(bytes) = serde_json::to_vec(stored) else { return };
        let Ok(w) = self.db.begin_write() else { return };
        {
            let Ok(mut t) = w.open_table(SLOTS) else { return };
            let _ = t.insert(flat(k).as_str(), bytes.as_slice());
        }
        let _ = w.commit();
    }

    fn put_session(&self, k: &SessionKey) {
        let Ok(w) = self.db.begin_write() else { return };
        {
            let Ok(mut t) = w.open_table(SESSIONS) else { return };
            let _ = t.insert(k.as_str(), unix_now());
        }
        let _ = w.commit();
    }

    fn remove(&self, slots: &[Key], sessions: &[SessionKey]) {
        let Ok(w) = self.db.begin_write() else { return };
        {
            if let Ok(mut t) = w.open_table(SLOTS) {
                for k in slots {
                    let _ = t.remove(flat(k).as_str());
                }
            }
            if let Ok(mut t) = w.open_table(SESSIONS) {
                for k in sessions {
                    let _ = t.remove(k.as_str());
                }
            }
        }
        let _ = w.commit();
    }

    /// Everything the file holds: the answers (with their age in seconds) and the sessions (with the seconds since they were touched).
    #[allow(clippy::type_complexity)]
    fn load(&self) -> (Vec<(Key, Stored)>, Vec<(SessionKey, u64)>) {
        let (mut slots, mut sessions) = (Vec::new(), Vec::new());
        let Ok(r) = self.db.begin_read() else { return (slots, sessions) };
        if let Ok(t) = r.open_table(SLOTS) {
            if let Ok(iter) = t.iter() {
                for row in iter.flatten() {
                    if let (Some([c, s, k]), Ok(st)) = (split::<3>(row.0.value()), serde_json::from_slice::<Stored>(row.1.value())) {
                        slots.push(((c, s, k), st));
                    }
                }
            }
        }
        if let Ok(t) = r.open_table(SESSIONS) {
            if let Ok(iter) = t.iter() {
                for row in iter.flatten() {
                    sessions.push((row.0.value().to_string(), row.1.value()));
                }
            }
        }
        (slots, sessions)
    }
}

impl Store {
    pub fn new(ttl: Duration, max: usize) -> Self {
        Self::with_options(Options::memory(Some(ttl), max), Arc::new(Instant::now)).expect("a memory store cannot fail to open")
    }

    /// A store with a clock of its own (a test moves it), opening the file when the backing is one. An older file's entries and
    /// sessions are brought back, aged by the time the file was closed.
    pub fn with_options(opts: Options, clock: Arc<dyn Fn() -> Instant + Send + Sync>) -> Result<Self, String> {
        let disk = match &opts.backing {
            Backing::Memory => None,
            Backing::File(p) => Some(Disk::open(p)?),
        };
        let mut inner = Inner { slots: HashMap::new(), order: Vec::new(), sessions: HashMap::new() };
        if let Some(d) = &disk {
            let now = clock();
            let ago = |secs: u64| now.checked_sub(Duration::from_secs(unix_now().saturating_sub(secs))).unwrap_or(now);
            let (slots, sessions) = d.load();
            for (k, at) in sessions {
                inner.sessions.insert(k, ago(at));
            }
            for (k, st) in slots {
                inner.slots.insert(k.clone(), Slot::Done { fingerprint: st.fingerprint, outcome: st.outcome, at: ago(st.at) });
                inner.order.push(k);
            }
        }
        Ok(Store { inner: Mutex::new(inner), opts: Options { max: opts.max.max(1), ..opts }, clock, disk })
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|i| i.slots.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many sessions are alive.
    pub fn sessions(&self) -> usize {
        self.inner.lock().map(|i| i.sessions.len()).unwrap_or(0)
    }

    /// Sessions idle for too long end, with their entries.
    fn sweep(&self, inner: &mut Inner, now: Instant) {
        let idle = self.opts.idle;
        let dead: Vec<SessionKey> = inner.sessions.iter().filter(|(_, at)| now.saturating_duration_since(**at) > idle).map(|(k, _)| k.clone()).collect();
        if dead.is_empty() {
            return;
        }
        let mut gone = Vec::new();
        for s in &dead {
            inner.sessions.remove(s);
        }
        inner.slots.retain(|k, _| {
            let alive = !dead.contains(&k.1);
            if !alive {
                gone.push(k.clone());
            }
            alive
        });
        inner.order.retain(|k| !gone.contains(k));
        if let Some(d) = &self.disk {
            d.remove(&gone, &dead);
        }
    }

    /// The `hello` names a session: `true` when the store already holds it (the client is continuing), `false` when it is new. Either
    /// way the session is alive from now.
    pub fn open_session(&self, session: &str) -> bool {
        let Ok(mut inner) = self.inner.lock() else { return false };
        let now = (self.clock)();
        self.sweep(&mut inner, now);
        let k = session.to_string();
        let known = inner.sessions.contains_key(&k);
        inner.sessions.insert(k.clone(), now);
        if !known {
            if let Some(d) = &self.disk {
                d.put_session(&k);
            }
        }
        known
    }

    /// The client said `bye`: the session and everything kept for it go. The answer is how many entries went.
    pub fn end_session(&self, caller: &str, session: &str) -> usize {
        let Ok(mut inner) = self.inner.lock() else { return 0 };
        inner.sessions.remove(session);
        let mut gone = Vec::new();
        inner.slots.retain(|key, _| {
            let mine = key.0 == caller && key.1 == session;
            if mine {
                gone.push(key.clone());
            }
            !mine
        });
        inner.order.retain(|key| !gone.contains(key));
        if let Some(d) = &self.disk {
            d.remove(&gone, &[session.to_string()]);
        }
        gone.len()
    }

    pub fn begin(&self, caller: &str, session: &str, key: &str, fp: &str) -> Begin {
        let Ok(mut inner) = self.inner.lock() else { return Begin::Fresh };
        let now = (self.clock)();
        self.sweep(&mut inner, now);
        inner.sessions.insert(session.to_string(), now);
        let k = (caller.to_string(), session.to_string(), key.to_string());
        // Expired entries leave when they are met.
        if let (Some(ttl), Some(Slot::Done { at, .. })) = (self.opts.ttl, inner.slots.get(&k)) {
            if now.saturating_duration_since(*at) > ttl {
                inner.slots.remove(&k);
                inner.order.retain(|x| x != &k);
                if let Some(d) = &self.disk {
                    d.remove(std::slice::from_ref(&k), &[]);
                }
            }
        }
        match inner.slots.get(&k) {
            Some(Slot::Running { fingerprint }) => {
                if fingerprint == fp {
                    Begin::Wait
                } else {
                    Begin::Mismatch
                }
            }
            Some(Slot::Done { fingerprint, outcome, .. }) => {
                if fingerprint == fp {
                    Begin::Replay(outcome.clone())
                } else {
                    Begin::Mismatch
                }
            }
            None => {
                let mut evicted = Vec::new();
                while inner.slots.len() >= self.opts.max {
                    if inner.order.is_empty() {
                        break;
                    }
                    let oldest = inner.order.remove(0);
                    inner.slots.remove(&oldest);
                    evicted.push(oldest);
                }
                if let (false, Some(d)) = (evicted.is_empty(), &self.disk) {
                    d.remove(&evicted, &[]);
                }
                inner.slots.insert(k.clone(), Slot::Running { fingerprint: fp.to_string() });
                inner.order.push(k);
                Begin::Fresh
            }
        }
    }

    /// The call finished. `outcome` is `None` for an outcome that must stay retryable. With a file behind it, the answer is on disk
    /// before this returns.
    pub fn finish(&self, caller: &str, session: &str, key: &str, fp: &str, outcome: Option<Outcome>) {
        let Ok(mut inner) = self.inner.lock() else { return };
        let k = (caller.to_string(), session.to_string(), key.to_string());
        match outcome {
            Some(o) => {
                if let Some(d) = &self.disk {
                    d.put_slot(&k, &Stored { fingerprint: fp.to_string(), outcome: o.clone(), at: unix_now() });
                }
                inner.slots.insert(k.clone(), Slot::Done { fingerprint: fp.to_string(), outcome: o, at: (self.clock)() });
                if !inner.order.contains(&k) {
                    inner.order.push(k);
                }
            }
            None => {
                inner.slots.remove(&k);
                inner.order.retain(|x| x != &k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_wire::error::{ErrorClass, codes};
    use std::sync::atomic::{AtomicU64, Ordering};

    const S1: &str = "s1";

    fn reply(n: &str) -> Outcome {
        Outcome::Reply { args: vec![Some(n.to_string())], returning: None, warnings: vec![] }
    }

    /// A clock a test moves by hand.
    fn clock() -> (Arc<AtomicU64>, Arc<dyn Fn() -> Instant + Send + Sync>) {
        let base = Instant::now();
        let ms = Arc::new(AtomicU64::new(0));
        let m = ms.clone();
        (ms, Arc::new(move || base + Duration::from_millis(m.load(Ordering::SeqCst))))
    }

    #[test]
    fn a_repeat_is_answered_from_the_store_and_a_different_request_is_refused() {
        let s = Store::new(Duration::from_secs(60), 10);
        let fp = fingerprint("PAY", &["{\"A\":1}".to_string()]);
        assert!(matches!(s.begin("u1", S1, "k1", &fp), Begin::Fresh));
        assert!(matches!(s.begin("u1", S1, "k1", &fp), Begin::Wait), "the first is still running");
        s.finish("u1", S1, "k1", &fp, Some(reply("done")));
        match s.begin("u1", S1, "k1", &fp) {
            Begin::Replay(Outcome::Reply { args, .. }) => assert_eq!(args[0].as_deref(), Some("done")),
            other => panic!("{other:?}"),
        }
        let other = fingerprint("PAY", &["{\"A\":2}".to_string()]);
        assert!(matches!(s.begin("u1", S1, "k1", &other), Begin::Mismatch));
        // Another caller's key is another key.
        assert!(matches!(s.begin("u2", S1, "k1", &fp), Begin::Fresh));
    }

    #[test]
    fn what_must_stay_retryable_is_forgotten_and_old_entries_leave() {
        let s = Store::new(Duration::from_millis(30), 2);
        let fp = fingerprint("P", &[]);
        assert!(matches!(s.begin("u", S1, "a", &fp), Begin::Fresh));
        s.finish("u", S1, "a", &fp, None);
        assert!(matches!(s.begin("u", S1, "a", &fp), Begin::Fresh), "a timeout is not remembered");
        s.finish("u", S1, "a", &fp, Some(Outcome::Failure(CallError::new(ErrorClass::Application, 4201, "declined"))));
        std::thread::sleep(Duration::from_millis(60));
        assert!(matches!(s.begin("u", S1, "a", &fp), Begin::Fresh), "past its time to live");
        s.finish("u", S1, "a", &fp, Some(reply("1")));
        assert!(matches!(s.begin("u", S1, "b", &fp), Begin::Fresh));
        s.finish("u", S1, "b", &fp, Some(reply("2")));
        assert!(matches!(s.begin("u", S1, "c", &fp), Begin::Fresh));
        assert_eq!(s.len(), 2);
        assert!(matches!(s.begin("u", S1, "a", &fp), Begin::Fresh), "the oldest was evicted");
        let _ = codes::INTERNAL;
    }

    #[test]
    fn same_key_other_session_runs_again() {
        let s = Store::new(Duration::from_secs(60), 10);
        let fp = fingerprint("PAY", &[]);
        assert!(matches!(s.begin("u", "sa", "key-1", &fp), Begin::Fresh));
        s.finish("u", "sa", "key-1", &fp, Some(reply("a")));
        assert!(matches!(s.begin("u", "sa", "key-1", &fp), Begin::Replay(_)));
        assert!(matches!(s.begin("u", "sb", "key-1", &fp), Begin::Fresh), "another session has its own entries");
    }

    #[test]
    fn bye_ends_the_session_and_frees_its_entries() {
        let s = Store::new(Duration::from_secs(60), 10);
        let fp = fingerprint("PAY", &[]);
        assert!(!s.open_session("sa"), "a new session");
        assert!(s.open_session("sa"), "the same session again is a continuation");
        for k in ["k1", "k2", "k3"] {
            assert!(matches!(s.begin("u", "sa", k, &fp), Begin::Fresh));
            s.finish("u", "sa", k, &fp, Some(reply(k)));
        }
        assert!(matches!(s.begin("u", "sb", "k1", &fp), Begin::Fresh));
        s.finish("u", "sb", "k1", &fp, Some(reply("b")));
        assert_eq!(s.end_session("u", "sa"), 3);
        assert_eq!(s.len(), 1, "the other session's entry stays");
        assert!(matches!(s.begin("u", "sa", "k1", &fp), Begin::Fresh));
        println!("bye freed 3 entries; 1 left for the other session");
    }

    #[test]
    fn idle_session_expires_after_the_configured_seconds() {
        let (ms, clk) = clock();
        let opts = Options { ttl: None, max: 10, idle: Duration::from_secs(900), backing: Backing::Memory };
        let s = Store::with_options(opts, clk).unwrap();
        let fp = fingerprint("PAY", &[]);
        s.open_session("sa");
        assert!(matches!(s.begin("u", "sa", "k1", &fp), Begin::Fresh));
        s.finish("u", "sa", "k1", &fp, Some(reply("x")));
        ms.store(899_000, Ordering::SeqCst);
        assert!(matches!(s.begin("u", "sa", "k1", &fp), Begin::Replay(_)), "899 s: still alive (and touched)");
        ms.store(899_000 + 901_000, Ordering::SeqCst);
        assert!(matches!(s.begin("u", "sa", "k1", &fp), Begin::Fresh), "901 s without a touch: gone");
        assert!(!s.open_session("sa") || s.sessions() == 1);
    }

    #[test]
    fn redb_store_survives_reopen_and_memory_store_does_not() {
        let dir = std::env::temp_dir().join(format!("prc-idem-{}-{}", std::process::id(), unix_now()));
        let path = dir.join("idem.redb");
        let fp = fingerprint("PAY", &["1".into()]);
        {
            let s = Store::with_options(Options { backing: Backing::File(path.clone()), ..Options::memory(None, 10) }, Arc::new(Instant::now)).unwrap();
            s.open_session("sa");
            assert!(matches!(s.begin("u", "sa", "key-1", &fp), Begin::Fresh));
            s.finish("u", "sa", "key-1", &fp, Some(reply("paid")));
        }
        let s = Store::with_options(Options { backing: Backing::File(path.clone()), ..Options::memory(None, 10) }, Arc::new(Instant::now)).unwrap();
        assert!(s.open_session("sa"), "the session came back");
        match s.begin("u", "sa", "key-1", &fp) {
            Begin::Replay(Outcome::Reply { args, .. }) => assert_eq!(args[0].as_deref(), Some("paid")),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.end_session("u", "sa"), 1);
        drop(s);
        let s = Store::with_options(Options { backing: Backing::File(path), ..Options::memory(None, 10) }, Arc::new(Instant::now)).unwrap();
        assert!(s.is_empty() && s.sessions() == 0, "bye is on disk too");
        let m = Store::new(Duration::from_secs(60), 10);
        m.finish("u", "sa", "key-1", &fp, Some(reply("x")));
        drop(m);
        assert!(Store::new(Duration::from_secs(60), 10).is_empty(), "a memory store forgets");
        let _ = std::fs::remove_dir_all(dir);
    }
}
