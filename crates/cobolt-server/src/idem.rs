// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Idempotency keys (WASM-014 R9, R10).
//!
//! A call may carry a key. The first call with a key runs; its answer is kept for
//! a stated time under `(caller, key)`; a repeat with the same key and the same
//! request is answered **from the store without running the program again**; a
//! repeat that arrives while the first is still running waits for it; the same
//! key with a different request is refused. Only a success and a failure the
//! program itself reported are kept: a timeout, a limit or a lost connection
//! must be retryable, so they are forgotten.
//!
//! The store is in memory and bounded (oldest entries leave first). It does not
//! survive a restart of the backend, which is the only honest answer for a
//! backend that has lost its memory.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use cobolt_wire::error::CallError;

/// What a finished call answered.
#[derive(Debug, Clone)]
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

pub struct Store {
    inner: Mutex<Inner>,
    ttl: Duration,
    max: usize,
}

struct Inner {
    slots: HashMap<(String, String), Slot>,
    order: Vec<(String, String)>,
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

impl Store {
    pub fn new(ttl: Duration, max: usize) -> Self {
        Store { inner: Mutex::new(Inner { slots: HashMap::new(), order: Vec::new() }), ttl, max: max.max(1) }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|i| i.slots.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn begin(&self, caller: &str, key: &str, fp: &str) -> Begin {
        let Ok(mut inner) = self.inner.lock() else { return Begin::Fresh };
        let now = Instant::now();
        let k = (caller.to_string(), key.to_string());
        // Expired entries leave when they are met.
        if let Some(Slot::Done { at, .. }) = inner.slots.get(&k) {
            if now.duration_since(*at) > self.ttl {
                inner.slots.remove(&k);
                inner.order.retain(|x| x != &k);
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
                while inner.slots.len() >= self.max {
                    if inner.order.is_empty() {
                        break;
                    }
                    let oldest = inner.order.remove(0);
                    inner.slots.remove(&oldest);
                }
                inner.slots.insert(k.clone(), Slot::Running { fingerprint: fp.to_string() });
                inner.order.push(k);
                Begin::Fresh
            }
        }
    }

    /// The call finished. `keep` is false for an outcome that must stay retryable.
    pub fn finish(&self, caller: &str, key: &str, fp: &str, outcome: Option<Outcome>) {
        let Ok(mut inner) = self.inner.lock() else { return };
        let k = (caller.to_string(), key.to_string());
        match outcome {
            Some(o) => {
                inner.slots.insert(k.clone(), Slot::Done { fingerprint: fp.to_string(), outcome: o, at: Instant::now() });
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

    fn reply(n: &str) -> Outcome {
        Outcome::Reply { args: vec![Some(n.to_string())], returning: None, warnings: vec![] }
    }

    #[test]
    fn a_repeat_is_answered_from_the_store_and_a_different_request_is_refused() {
        let s = Store::new(Duration::from_secs(60), 10);
        let fp = fingerprint("PAY", &["{\"A\":1}".to_string()]);
        assert!(matches!(s.begin("u1", "k1", &fp), Begin::Fresh));
        assert!(matches!(s.begin("u1", "k1", &fp), Begin::Wait), "the first is still running");
        s.finish("u1", "k1", &fp, Some(reply("done")));
        match s.begin("u1", "k1", &fp) {
            Begin::Replay(Outcome::Reply { args, .. }) => assert_eq!(args[0].as_deref(), Some("done")),
            other => panic!("{other:?}"),
        }
        let other = fingerprint("PAY", &["{\"A\":2}".to_string()]);
        assert!(matches!(s.begin("u1", "k1", &other), Begin::Mismatch));
        // Another caller's key is another key.
        assert!(matches!(s.begin("u2", "k1", &fp), Begin::Fresh));
    }

    #[test]
    fn what_must_stay_retryable_is_forgotten_and_old_entries_leave() {
        let s = Store::new(Duration::from_millis(30), 2);
        let fp = fingerprint("P", &[]);
        assert!(matches!(s.begin("u", "a", &fp), Begin::Fresh));
        s.finish("u", "a", &fp, None);
        assert!(matches!(s.begin("u", "a", &fp), Begin::Fresh), "a timeout is not remembered");
        s.finish("u", "a", &fp, Some(Outcome::Failure(CallError::new(ErrorClass::Application, 4201, "declined"))));
        std::thread::sleep(Duration::from_millis(60));
        assert!(matches!(s.begin("u", "a", &fp), Begin::Fresh), "past its time to live");
        // Bounded: with room for two, the oldest goes when a third arrives.
        s.finish("u", "a", &fp, Some(reply("1")));
        assert!(matches!(s.begin("u", "b", &fp), Begin::Fresh));
        s.finish("u", "b", &fp, Some(reply("2")));
        assert!(matches!(s.begin("u", "c", &fp), Begin::Fresh));
        assert_eq!(s.len(), 2);
        assert!(matches!(s.begin("u", "a", &fp), Begin::Fresh), "the oldest was evicted");
        let _ = codes::INTERNAL;
    }
}
