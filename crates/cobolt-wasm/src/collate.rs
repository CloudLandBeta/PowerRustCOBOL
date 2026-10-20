// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `PROGRAM COLLATING SEQUENCE`: the table the comparison routines use.
//!
//! The rule is the interpreter's (`cobolt-runtime/src/collation.rs`): the
//! alphabet lists character *positions*; two characters joined by `ALSO` share
//! one and compare equal; every character the alphabet never mentions sorts
//! after the listed ones, in native order; `LOW-VALUE` is the character at the
//! lowest position and `HIGH-VALUE` the one at the highest. `NATIVE`,
//! `STANDARD-1`/`-2` leave the native order in force.

use cobolt_ast::program::AlphabetSpec;

/// `[low][high]` then 256 little-endian `u16` ordinals, or `None` when the
/// native order stays.
pub fn table(spec: &AlphabetSpec) -> Option<Vec<u8>> {
    let AlphabetSpec::Literal(groups) = spec else { return None };
    if groups.is_empty() {
        return None;
    }
    let mut slot: [Option<u16>; 256] = [None; 256];
    let mut next: u16 = 0;
    let mut low: Option<u8> = None;
    let mut high: u8 = 0;
    for group in groups {
        let mut used = false;
        for &c in group {
            let Ok(b) = u8::try_from(c as u32) else { continue };
            if slot[b as usize].is_none() {
                slot[b as usize] = Some(next);
                used = true;
                if low.is_none() {
                    low = Some(b);
                }
                high = b;
            }
        }
        if used {
            next += 1;
        }
    }
    let mut ordinal = [0u16; 256];
    let mut trailing = next;
    for b in 0..256usize {
        match slot[b] {
            Some(o) => ordinal[b] = o,
            None => {
                ordinal[b] = trailing;
                trailing += 1;
                high = b as u8;
            }
        }
    }
    let mut out = vec![low.unwrap_or(0), high];
    for o in ordinal {
        out.extend_from_slice(&o.to_le_bytes());
    }
    Some(out)
}
