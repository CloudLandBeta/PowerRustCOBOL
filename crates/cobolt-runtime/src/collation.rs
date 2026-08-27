// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `PROGRAM COLLATING SEQUENCE` — the ordering every alphanumeric comparison
//! in a program uses.
//!
//! A program with no `OBJECT-COMPUTER. … PROGRAM COLLATING SEQUENCE` clause
//! compares characters in the machine's native (ASCII) order, which is what
//! every ordinary Rust string comparison already does. Naming an `ALPHABET`
//! replaces that order wholesale: the alphabet lists character *positions*, so
//! `"A" THRU "H" "I" ALSO "J" …` puts `A` first, makes `I` and `J` compare
//! **equal**, and pushes every character it never mentions after all of them.
//!
//! The clause also redefines the figurative constants: `LOW-VALUE` is whatever
//! character sits at the lowest position of the program's sequence, and
//! `HIGH-VALUE` the highest — not `0x00` and `0xFF`.
//!
//! ## Why a thread-local
//!
//! The sequence belongs to the *program*, but [`crate::interpreter::compare_values`]
//! is a free function reached from many call sites that have no interpreter to
//! hand. The interpreter runs on one thread, so the active table is published
//! here the same way the IDE publishes its active editor palette, and the
//! comparison reads it back. [`set_active`] is called once when a program
//! starts and cleared when it ends.

use std::cell::RefCell;

use cobolt_ast::program::AlphabetSpec;

/// A resolved collating sequence: an ordinal for each of the 256 byte values,
/// plus the characters the figurative constants name under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collation {
    /// `ordinal[b]` is where byte `b` sorts. Two bytes joined by `ALSO` share
    /// an ordinal and therefore compare equal.
    ordinal: [u16; 256],
    /// The character at the lowest position — what `LOW-VALUE` names.
    low: u8,
    /// The character at the highest position — what `HIGH-VALUE` names.
    high: u8,
}

impl Collation {
    /// Build the table for one `ALPHABET` spec, or `None` when the spec leaves
    /// the native order in force.
    ///
    /// `NATIVE` and `STANDARD-1`/`STANDARD-2` are the native order on this
    /// platform, so they need no table at all. `EBCDIC` is **not implemented**
    /// and also returns `None`, which leaves ASCII ordering in force — see the
    /// known-gaps list in `docs/cobol85-supported-syntax-en.md`.
    pub fn from_spec(spec: &AlphabetSpec) -> Option<Self> {
        let AlphabetSpec::Literal(groups) = spec else {
            return None;
        };
        if groups.is_empty() {
            return None;
        }

        // `None` until a position is assigned, so the unlisted characters can be
        // told apart from the ones written at position 0.
        let mut slot: [Option<u16>; 256] = [None; 256];
        let mut next: u16 = 0;
        let mut low: Option<u8> = None;
        let mut high: u8 = 0;

        for group in groups {
            let mut used = false;
            for &c in group {
                // The alphabet describes single-byte characters; anything wider
                // cannot name a position in a 256-entry sequence.
                let Ok(b) = u8::try_from(c as u32) else {
                    continue;
                };
                // First mention wins: a character written twice keeps its
                // earliest position rather than being silently moved.
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

        // Every character the alphabet never mentions sorts after all of them,
        // among themselves in native order.
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

        Some(Collation {
            ordinal,
            low: low.unwrap_or(0),
            high,
        })
    }

    /// Where `b` sorts in this sequence.
    #[inline]
    pub fn ordinal_of(&self, b: u8) -> u16 {
        self.ordinal[b as usize]
    }

    /// The character `LOW-VALUE` names under this sequence.
    pub fn low_value(&self) -> char {
        self.low as char
    }

    /// The character `HIGH-VALUE` names under this sequence.
    pub fn high_value(&self) -> char {
        self.high as char
    }

    /// Compare two strings in this sequence, position by position.
    ///
    /// The operands are already padded to a common width by the caller, the
    /// way COBOL pads the shorter one with spaces on the right.
    pub fn compare(&self, l: &str, r: &str) -> std::cmp::Ordering {
        for (a, b) in l.bytes().zip(r.bytes()) {
            let ord = self.ordinal_of(a).cmp(&self.ordinal_of(b));
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        l.len().cmp(&r.len())
    }
}

thread_local! {
    /// The collating sequence in force on this thread, if any.
    static ACTIVE: RefCell<Option<Collation>> = const { RefCell::new(None) };
}

/// Publish the collating sequence for the program about to run. `None` restores
/// native ordering.
pub fn set_active(c: Option<Collation>) {
    ACTIVE.with(|a| *a.borrow_mut() = c);
}

/// Read the active collating sequence, if a program declared one.
pub fn with_active<R>(f: impl FnOnce(Option<&Collation>) -> R) -> R {
    ACTIVE.with(|a| f(a.borrow().as_ref()))
}

/// `true` when a program collating sequence is in force — a cheap gate so the
/// ordinary native-order path stays a plain string comparison.
pub fn is_active() -> bool {
    ACTIVE.with(|a| a.borrow().is_some())
}

/// The character `LOW-VALUE` names, when a sequence is in force.
pub fn active_low_value() -> Option<char> {
    with_active(|c| c.map(Collation::low_value))
}

/// The character `HIGH-VALUE` names, when a sequence is in force.
pub fn active_high_value() -> Option<char> {
    with_active(|c| c.map(Collation::high_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NC215A's alphabet: `"A" THRU "H" "I" ALSO "J" ALSO "K" ALSO "L" ALSO
    /// "M" ALSO "N" "O" THROUGH "Z" "0" THRU "9"`.
    fn wild_one() -> Collation {
        let mut groups: Vec<Vec<char>> = ('A'..='H').map(|c| vec![c]).collect();
        groups.push(vec!['I', 'J', 'K', 'L', 'M', 'N']);
        groups.extend(('O'..='Z').map(|c| vec![c]));
        groups.extend(('0'..='9').map(|c| vec![c]));
        Collation::from_spec(&AlphabetSpec::Literal(groups)).expect("literal alphabet")
    }

    #[test]
    fn listed_characters_take_their_written_positions() {
        let c = wild_one();
        assert_eq!(c.ordinal_of(b'A'), 0);
        assert_eq!(c.ordinal_of(b'H'), 7);
        assert_eq!(c.ordinal_of(b'O'), 9);
    }

    /// `ALSO` folds its operands into one position, so they compare equal.
    #[test]
    fn also_makes_characters_compare_equal() {
        let c = wild_one();
        for ch in [b'I', b'J', b'K', b'L', b'M', b'N'] {
            assert_eq!(c.ordinal_of(ch), c.ordinal_of(b'I'), "{}", ch as char);
        }
        assert_eq!(c.compare("I", "N"), std::cmp::Ordering::Equal);
    }

    /// A digit written after `Z` sorts after it, whatever ASCII says.
    #[test]
    fn the_sequence_overrides_native_order() {
        let c = wild_one();
        assert!(c.ordinal_of(b'A') < c.ordinal_of(b'0'));
        assert_eq!(c.compare("A", "0"), std::cmp::Ordering::Less);
        // Unlisted characters follow every listed one.
        assert!(c.ordinal_of(b'9') < c.ordinal_of(b' '));
        assert!(c.ordinal_of(b'9') < c.ordinal_of(b'"'));
    }

    /// The figurative constants name the ends of the program's own sequence.
    #[test]
    fn low_and_high_value_follow_the_sequence() {
        assert_eq!(wild_one().low_value(), 'A');
    }

    /// NC219A's alphabet: `"F" "U" "N" ALSO HIGH-VALUE ALSO LOW-VALUE "Y"`.
    #[test]
    fn figurative_operands_take_their_native_characters() {
        let c = Collation::from_spec(&AlphabetSpec::Literal(vec![
            vec!['F'],
            vec!['U'],
            vec!['N', '\u{ff}', '\u{0}'],
            vec!['Y'],
        ]))
        .expect("literal alphabet");
        assert_eq!(c.low_value(), 'F');
        assert_eq!(c.ordinal_of(b'U'), 1);
        assert_eq!(c.ordinal_of(b'N'), 2);
        assert_eq!(c.ordinal_of(0xff), 2);
        assert_eq!(c.compare("U", "N"), std::cmp::Ordering::Less);
    }

    /// A named standard sequence leaves native ordering in force.
    #[test]
    fn native_and_standard_need_no_table() {
        assert!(Collation::from_spec(&AlphabetSpec::Native).is_none());
        assert!(Collation::from_spec(&AlphabetSpec::Standard).is_none());
    }
}
