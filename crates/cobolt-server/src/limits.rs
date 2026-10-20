// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rate limits (WASM-003 R16): a token bucket per connection and per user.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// `rate` tokens per second up to `burst`.
#[derive(Debug, Clone)]
pub struct Bucket {
    rate: f64,
    burst: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(rate_per_sec: u32, burst: u32) -> Self {
        let burst = burst.max(1) as f64;
        Bucket { rate: rate_per_sec.max(1) as f64, burst, tokens: burst, last: Instant::now() }
    }

    /// Take one token; false when the bucket is empty.
    pub fn take(&mut self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + dt * self.rate).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// One bucket per key (a user's subject, a source address), evicted when idle.
pub struct KeyedLimiter {
    rate: u32,
    burst: u32,
    buckets: Mutex<HashMap<String, Bucket>>,
}

const MAX_KEYS: usize = 20_000;

impl KeyedLimiter {
    pub fn new(rate: u32, burst: u32) -> Self {
        KeyedLimiter { rate, burst, buckets: Mutex::new(HashMap::new()) }
    }

    pub fn take(&self, key: &str) -> bool {
        let Ok(mut m) = self.buckets.lock() else { return true };
        if m.len() >= MAX_KEYS && !m.contains_key(key) {
            // Idle buckets are full buckets: drop those, which forgets nobody who is limited.
            m.retain(|_, b| b.tokens < b.burst);
            if m.len() >= MAX_KEYS {
                return false;
            }
        }
        m.entry(key.to_string()).or_insert_with(|| Bucket::new(self.rate, self.burst)).take()
    }
}
