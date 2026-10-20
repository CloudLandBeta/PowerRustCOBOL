// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Whether one layout can stand in for another on the wire (WASM-005 R7,
//! WASM-014 R4).
//!
//! The same question is asked in two places. At Check time: does the group the
//! frontend passes fit the parameter the backend's interface description
//! declares? At release time: does the backend's new layout still accept what
//! last month's frontend sends and still produce what it can read? Both come
//! down to [`fits`] — *every value the sender can write is accepted by the
//! receiver* — and, for the answer travelling back, [`fits_projected`], where
//! the sender may know more members than the receiver (the backend projects its
//! answer to the members the caller knows).

use crate::layout::{Item, Kind};

/// One reason a layout does not fit another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// The dotted path of the member, e.g. `ORDER.LINES`.
    pub path: String,
    /// Which compatibility rule it breaks (the name WASM-014's report uses).
    pub rule: &'static str,
    pub reason: String,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} ({})", self.path, self.reason, self.rule)
    }
}

pub const R_MISSING: &str = "member-missing";
pub const R_TYPE: &str = "type-changed";
pub const R_NARROWED: &str = "narrowed";
pub const R_TABLE: &str = "table-changed";

/// Can everything `sender` writes be accepted by `receiver`? Every member of
/// the sender must exist in the receiver with a compatible type and room for
/// it. The receiver may have members the sender lacks (they stay unchanged).
pub fn fits(sender: &Item, receiver: &Item) -> Vec<Mismatch> {
    let mut out = Vec::new();
    walk(sender, receiver, &sender.name.clone(), Mode::Strict, &mut out);
    out
}

/// Can everything `sender` writes be read into `receiver`, ignoring the
/// sender's members the receiver does not know? (The answer direction: the
/// backend projects its reply to the members the caller declared.)
pub fn fits_projected(sender: &Item, receiver: &Item) -> Vec<Mismatch> {
    let mut out = Vec::new();
    walk(sender, receiver, &receiver.name.clone(), Mode::Projected, &mut out);
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Every member of the sender must exist in the receiver.
    Strict,
    /// Only members present on both sides are compared.
    Projected,
}

fn walk(sender: &Item, receiver: &Item, path: &str, mode: Mode, out: &mut Vec<Mismatch>) {
    let mut push = |rule: &'static str, reason: String| out.push(Mismatch { path: path.to_string(), rule, reason });
    // Tables.
    match (&sender.occurs, &receiver.occurs) {
        (None, None) => {}
        (Some(s), Some(r)) => {
            if s.max > r.max {
                push(R_NARROWED, format!("sends up to {} occurrences, the receiver holds {}", s.max, r.max));
            }
            if r.depending_on.is_some() && r.min > 0 && s.depending_on.is_some() && s.min < r.min {
                push(R_NARROWED, format!("may send {} occurrences, the receiver needs at least {}", s.min, r.min));
            }
        }
        (Some(_), None) => {
            push(R_TABLE, "is a table here and a single item there".to_string());
            return;
        }
        (None, Some(_)) => {
            push(R_TABLE, "is a single item here and a table there".to_string());
            return;
        }
    }
    match (&sender.kind, &receiver.kind) {
        (Kind::Group { .. }, Kind::Group { .. }) => {
            let theirs: Vec<&Item> = receiver.members().collect();
            for m in sender.members() {
                let child_path = format!("{path}.{}", m.name);
                match theirs.iter().find(|r| r.name == m.name) {
                    Some(r) => walk(m, r, &child_path, mode, out),
                    None if mode == Mode::Strict => out.push(Mismatch {
                        path: child_path,
                        rule: R_MISSING,
                        reason: "the receiver has no member of this name (names are compared as declared, case included)".to_string(),
                    }),
                    None => {}
                }
            }
            // The answer direction: the receiver is a client of an older contract,
            // and it READS every member it declared. An answer that no longer
            // carries one would reach it as blanks or zeros — silently wrong — so
            // a member the receiver knows and the sender lacks is a break.
            if mode == Mode::Projected {
                let ours: Vec<&Item> = sender.members().collect();
                for r in receiver.members() {
                    if !ours.iter().any(|s| s.name == r.name) {
                        out.push(Mismatch {
                            path: format!("{path}.{}", r.name),
                            rule: R_MISSING,
                            reason: "the answer no longer carries this member, which a client of the older contract reads (names are compared as declared, case included)".to_string(),
                        });
                    }
                }
            }
        }
        (Kind::Numeric { digits: sd, scale: ss, signed: sg }, Kind::Numeric { digits: rd, scale: rs, signed: rg }) => {
            if ss != rs {
                push(R_TYPE, format!("scale {ss} here, {rs} there"));
            }
            if sd > rd {
                push(R_NARROWED, format!("{sd} integer digits here, {rd} there"));
            }
            if *sg && !*rg {
                push(R_TYPE, "signed here, unsigned there".to_string());
            }
        }
        (Kind::Alphanumeric { len: s, .. }, Kind::Alphanumeric { len: r, .. })
        | (Kind::Edited { len: s }, Kind::Edited { len: r })
        | (Kind::National { len: s }, Kind::National { len: r }) => {
            if s > r {
                push(R_NARROWED, format!("{s} here, {r} there"));
            }
        }
        (Kind::Utf8 { len: s, bytes: sb }, Kind::Utf8 { len: r, bytes: rb }) => {
            if s > r {
                push(R_NARROWED, format!("{s} characters here, {r} there"));
            }
            if sb.unwrap_or(usize::MAX) > rb.unwrap_or(usize::MAX) {
                push(R_NARROWED, "a BYTE-LENGTH limit there that is smaller than here".to_string());
            }
        }
        (s, r) => push(R_TYPE, format!("{} here, {} there", kind_word(s), kind_word(r))),
    }
}

fn kind_word(k: &Kind) -> &'static str {
    match k {
        Kind::Group { .. } => "a group",
        Kind::Alphanumeric { .. } => "alphanumeric",
        Kind::National { .. } => "national",
        Kind::Utf8 { .. } => "UTF-8",
        Kind::Numeric { .. } => "numeric",
        Kind::Edited { .. } => "an edited picture",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Occurs;

    fn alpha(name: &str, len: usize) -> Item {
        Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Alphanumeric { len, justified: false }, value: None }
    }
    fn num(name: &str, digits: u16, scale: u16, signed: bool) -> Item {
        Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Numeric { digits, scale, signed }, value: None }
    }
    fn group(name: &str, children: Vec<Item>) -> Item {
        Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Group { children }, value: None }
    }

    #[test]
    fn an_added_member_is_compatible_in_the_answer_direction_only() {
        let old = group("R", vec![alpha("A", 4)]);
        let new = group("R", vec![alpha("A", 4), num("B", 3, 0, false)]);
        assert!(fits(&old, &new).is_empty(), "old client's request is accepted by the new layout");
        assert!(fits_projected(&new, &old).is_empty(), "the new answer, projected, is readable by the old client");
        assert_eq!(fits(&new, &old).len(), 1, "but the new member cannot be sent to the old layout");
    }

    #[test]
    fn a_member_the_answer_no_longer_carries_is_a_break_for_an_older_reader() {
        // The answer of the new contract lost A (renamed to Z): an old client that
        // reads A would be handed blanks.
        let old = group("R", vec![alpha("A", 4), num("N", 5, 2, true)]);
        let renamed = group("R", vec![alpha("Z", 4), num("N", 5, 2, true)]);
        let found = fits_projected(&renamed, &old);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].rule, R_MISSING);
        assert_eq!(found[0].path, "R.A");
        // Nested groups are walked: the same loss one level down is found with its path.
        let old_deep = group("R", vec![group("INNER", vec![alpha("A", 4)])]);
        let new_deep = group("R", vec![group("INNER", vec![alpha("B", 4)])]);
        assert_eq!(fits_projected(&new_deep, &old_deep)[0].path, "R.INNER.A");
        // A new member alongside is still fine.
        assert!(fits_projected(&group("R", vec![alpha("A", 4), num("N", 5, 2, true), alpha("EXTRA", 2)]), &old).is_empty());
    }

    #[test]
    fn every_breaking_change_is_named() {
        let base = group("R", vec![alpha("A", 4), num("N", 5, 2, true)]);
        for (changed, rule) in [
            (group("R", vec![alpha("A", 3), num("N", 5, 2, true)]), R_NARROWED),
            (group("R", vec![alpha("Z", 4), num("N", 5, 2, true)]), R_MISSING),
            (group("R", vec![alpha("A", 4), num("N", 5, 3, true)]), R_TYPE),
            (group("R", vec![alpha("A", 4), num("N", 5, 2, false)]), R_TYPE),
            (group("R", vec![num("A", 4, 0, false), num("N", 5, 2, true)]), R_TYPE),
        ] {
            let found = fits(&base, &changed);
            assert!(found.iter().any(|m| m.rule == rule), "expected {rule}, got {found:?}");
        }
    }

    #[test]
    fn widening_a_table_is_compatible_narrowing_is_not() {
        let mut t = alpha("T", 2);
        t.occurs = Some(Occurs { min: 3, max: 3, depending_on: None });
        let mut wider = t.clone();
        wider.occurs = Some(Occurs { min: 5, max: 5, depending_on: None });
        assert!(fits(&t, &wider).is_empty());
        assert!(!fits(&wider, &t).is_empty());
    }
}
