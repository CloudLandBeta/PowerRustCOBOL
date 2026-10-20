// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Data parallelism with a sequential twin that gives **identical results**
//! (WASM-018 R20, WASM-017 R19).
//!
//! The crate never reduces across threads in an order that depends on the
//! thread count. Everything that runs here is "map over independent indexes or
//! chunks": the work for chunk `i` depends only on chunk `i`'s inputs, so
//! running the chunks one after the other on one thread, or at the same time on
//! many, writes the same bytes. The tests compare 1 thread against many.
//!
//! Rayon is used when the `parallel` feature is on **and** the target can run
//! threads: every native target, and `wasm32` built with the `atomics` target
//! feature (the threaded player, `wasm-bindgen-rayon`). The plain player
//! (`wasm32-unknown-unknown` without atomics) always takes the sequential path,
//! whatever the features say - there is no thread to run on.

/// Whether this build runs chunks on a thread pool.
pub const IS_PARALLEL: bool = cfg!(all(
    feature = "parallel",
    any(not(target_arch = "wasm32"), target_feature = "atomics")
));

#[cfg(all(feature = "parallel", any(not(target_arch = "wasm32"), target_feature = "atomics")))]
mod imp {
    use rayon::prelude::*;

    pub fn current_threads() -> usize {
        rayon::current_num_threads().max(1)
    }

    pub fn for_each_chunk_mut<T: Send, F: Fn(usize, &mut [T]) + Send + Sync>(
        data: &mut [T],
        chunk_len: usize,
        f: F,
    ) {
        let chunk_len = chunk_len.max(1);
        data.par_chunks_mut(chunk_len).enumerate().for_each(|(i, c)| f(i, c));
    }

    pub fn map_range<R: Send, F: Fn(usize) -> R + Send + Sync>(n: usize, f: F) -> Vec<R> {
        (0..n).into_par_iter().map(f).collect()
    }

    pub fn with_threads<R: Send, F: FnOnce() -> R + Send>(threads: usize, f: F) -> R {
        match rayon::ThreadPoolBuilder::new().num_threads(threads.max(1)).build() {
            Ok(pool) => pool.install(f),
            Err(_) => f(),
        }
    }
}

#[cfg(not(all(feature = "parallel", any(not(target_arch = "wasm32"), target_feature = "atomics"))))]
mod imp {
    pub fn current_threads() -> usize {
        1
    }

    pub fn for_each_chunk_mut<T: Send, F: Fn(usize, &mut [T]) + Send + Sync>(
        data: &mut [T],
        chunk_len: usize,
        f: F,
    ) {
        let chunk_len = chunk_len.max(1);
        for (i, c) in data.chunks_mut(chunk_len).enumerate() {
            f(i, c);
        }
    }

    pub fn map_range<R: Send, F: Fn(usize) -> R + Send + Sync>(n: usize, f: F) -> Vec<R> {
        (0..n).map(f).collect()
    }

    pub fn with_threads<R: Send, F: FnOnce() -> R + Send>(_threads: usize, f: F) -> R {
        f()
    }
}

/// How many threads a call made now would use (1 when sequential).
pub fn current_threads() -> usize {
    imp::current_threads()
}

/// Call `f(index, chunk)` for every `chunk_len`-element chunk of `data`
/// (the last chunk may be shorter). Chunks never overlap, so `f` may write its
/// chunk freely; the result is the same on any number of threads.
pub fn for_each_chunk_mut<T: Send, F: Fn(usize, &mut [T]) + Send + Sync>(
    data: &mut [T],
    chunk_len: usize,
    f: F,
) {
    imp::for_each_chunk_mut(data, chunk_len, f)
}

/// `(0..n).map(f).collect()` - results come back in index order.
pub fn map_range<R: Send, F: Fn(usize) -> R + Send + Sync>(n: usize, f: F) -> Vec<R> {
    imp::map_range(n, f)
}

/// Run `f` with the pool restricted to `threads` threads. Used by the tests and
/// the benchmarks to compare 1 thread against many; hosts that want a smaller
/// pool for the whole page configure Rayon's global pool instead.
pub fn with_threads<R: Send, F: FnOnce() -> R + Send>(threads: usize, f: F) -> R {
    imp::with_threads(threads, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_are_visited_with_their_indexes() {
        let mut v = vec![0u32; 1000];
        for_each_chunk_mut(&mut v, 64, |i, c| {
            for x in c.iter_mut() {
                *x = i as u32;
            }
        });
        for (k, x) in v.iter().enumerate() {
            assert_eq!(*x, (k / 64) as u32);
        }
    }

    #[test]
    fn map_range_keeps_index_order() {
        let r = map_range(500, |i| i * 2);
        assert_eq!(r, (0..500).map(|i| i * 2).collect::<Vec<_>>());
    }

    #[test]
    fn one_thread_and_many_agree() {
        let work = || {
            let mut v = vec![0u64; 100_000];
            for_each_chunk_mut(&mut v, 997, |i, c| {
                let mut h = 0xcbf29ce484222325u64 ^ i as u64;
                for x in c.iter_mut() {
                    h = h.wrapping_mul(0x100000001b3).rotate_left(5) ^ 0x9e3779b97f4a7c15;
                    *x = h;
                }
            });
            v
        };
        let one = with_threads(1, work);
        let many = with_threads(8, work);
        assert_eq!(one, many);
    }
}
