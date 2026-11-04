// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The player's parallel work (WASM-017 R19a): sorting and filtering the rows of a DataGrid.
//!
//! On the threaded player the work runs on Rayon's worker pool; on the plain player the same
//! code runs sequentially. **The result never depends on the number of threads** (WASM-017
//! R19f): both sorts are stable, a stable sort has exactly one result, and a test compares them.

use std::cmp::Ordering;

fn cell_cmp(col: usize) -> impl Fn(&Vec<String>, &Vec<String>) -> Ordering + Sync {
    move |l, r| {
        let a = l.get(col).map(String::as_str).unwrap_or("");
        let b = r.get(col).map(String::as_str).unwrap_or("");
        a.cmp(b)
    }
}

/// Sort `rows` by column `col` (0-based), stably, comparing the text as the interpreter does.
pub fn sort_rows(rows: &mut Vec<Vec<String>>, col: usize) {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;
        // Rayon's `par_sort_by` is stable; below the threshold it is the sequential sort itself.
        rows.par_sort_by(cell_cmp(col));
    }
    #[cfg(not(feature = "rayon"))]
    {
        rows.sort_by(cell_cmp(col));
    }
}

/// The rows whose cell in `col` contains `needle`, case-insensitively, in their original order.
pub fn filter_rows(rows: &[Vec<String>], col: usize, needle: &str) -> Vec<Vec<String>> {
    let needle = needle.to_lowercase();
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;
        rows.par_iter().filter(|r| r.get(col).is_some_and(|c| c.to_lowercase().contains(&needle))).cloned().collect()
    }
    #[cfg(not(feature = "rayon"))]
    {
        rows.iter().filter(|r| r.get(col).is_some_and(|c| c.to_lowercase().contains(&needle))).cloned().collect()
    }
}

/// What a sort of `rows` synthetic rows did, as the DataGrid's `SORT` does it (WASM-017 T12): how many threads ran its comparisons.
/// On the plain player that is the calling thread alone; on the threaded one, the workers of Rayon's pool that took part.
pub struct SortReport {
    pub rows: usize,
    /// Threads the pool has (1 on the plain player).
    pub pool: usize,
    /// Distinct threads that compared two rows.
    pub threads_used: usize,
    pub sorted: bool,
}

pub fn sort_report(rows: usize) -> SortReport {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    let seen = AtomicU64::new(0);
    let mut data: Vec<Vec<String>> = (0..rows).map(|i| vec![format!("k{:05}", (i * 7919) % 10007), i.to_string()]).collect();
    let by_text = cell_cmp(0);
    let cmp = |l: &Vec<String>, r: &Vec<String>| {
        #[cfg(feature = "rayon")]
        let me = rayon::current_thread_index().map_or(63, |i| i.min(62));
        #[cfg(not(feature = "rayon"))]
        let me = 0;
        seen.fetch_or(1u64 << me, Relaxed);
        by_text(l, r)
    };
    #[cfg(feature = "rayon")]
    let pool = {
        use rayon::prelude::*;
        data.par_sort_by(cmp);
        rayon::current_num_threads()
    };
    #[cfg(not(feature = "rayon"))]
    let pool = {
        data.sort_by(cmp);
        1
    };
    let sorted = data.windows(2).all(|w| w[0][0] <= w[1][0]);
    SortReport { rows, pool, threads_used: seen.load(Relaxed).count_ones() as usize, sorted }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(n: usize) -> Vec<Vec<String>> {
        // Many equal keys, so stability matters: the second cell records the original position.
        (0..n).map(|i| vec![format!("k{}", (i * 7919) % 13), i.to_string()]).collect()
    }

    #[test]
    fn the_sort_is_stable_so_it_has_exactly_one_result() {
        let mut a = data(5000);
        sort_rows(&mut a, 0);
        let mut reference = data(5000);
        reference.sort_by(|l, r| l[0].cmp(&r[0]));
        assert_eq!(a, reference, "same rows in the same order as a plain stable sort, whatever the thread count");
        for w in a.windows(2) {
            if w[0][0] == w[1][0] {
                assert!(w[0][1].parse::<usize>().unwrap() < w[1][1].parse::<usize>().unwrap(), "equal keys keep their order");
            }
        }
    }

    #[test]
    fn the_filter_keeps_order_and_is_case_insensitive() {
        let rows = vec![vec!["Alpha".into()], vec!["beta".into()], vec!["ALPINE".into()]];
        let got = filter_rows(&rows, 0, "AL");
        assert_eq!(got, vec![vec!["Alpha".to_string()], vec!["ALPINE".to_string()]]);
        assert!(filter_rows(&rows, 3, "x").is_empty(), "a column past the end matches nothing");
    }
}
