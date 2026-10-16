// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The drawn ids of a repeating group's cards and of their members (spec 024 T1).
//!
//! A repeating group draws one card per instance, and each card's members carry ids of
//! their own so that a member's id can never collide with the designed base id or with a
//! same-named member of another group:
//!
//! ```text
//! <group>.<group>-<n>            the n-th card (1-based)
//! <group>.<group>-<n>.<member>   one member of that card
//! ```
//!
//! This module is the one place that **builds** those ids (the renderer) and **takes them
//! apart** (the runtime, when the operator edits a member), so the two cannot drift. It
//! needs no `render` feature: the runtime links it.

/// Id of the `inst`-th (**1-based**) card of repeating group `group_id`:
/// `"<group>.<group>-<inst>"`.
pub fn group_instance_id(group_id: &str, inst: usize) -> String {
    format!("{group_id}.{group_id}-{inst}")
}

/// Id of member `member_id` inside the `inst`-th card of `group_id`:
/// `"<group>.<group>-<inst>.<member>"`.
pub fn member_instance_id(group_id: &str, member_id: &str, inst: usize) -> String {
    format!("{group_id}.{group_id}-{inst}.{member_id}")
}

/// A drawn member id taken apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberInstance<'a> {
    pub group: &'a str,
    /// The 1-based card.
    pub instance: usize,
    pub member: &'a str,
}

/// `"<group>.<group>-<n>.<member>"` (what [`member_instance_id`] builds), or `None` for
/// any other id — a plain control, a card, a name with a different number of parts, or a
/// card number that is not a positive integer.
pub fn parse_member_instance_id(id: &str) -> Option<MemberInstance<'_>> {
    let mut parts = id.split('.');
    let (group, card, member) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || group.is_empty() || member.is_empty() {
        return None;
    }
    // The card is `<group>-<n>` — the group's own name, a dash, and digits — so an id whose card
    // names another group, or carries a sign, is not one this module built.
    let digits = card.strip_prefix(group)?.strip_prefix('-')?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let instance = digits.parse::<usize>().ok().filter(|n| *n >= 1)?;
    Some(MemberInstance { group, instance, member })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_member_id_parses_back_to_what_built_it() {
        for (group, member, n) in [("groupbox-2", "txt-name", 3), ("Cards", "Lbl-1", 1), ("A", "B", 500), ("Card-Group", "Nud-Fee", 12)] {
            let id = member_instance_id(group, member, n);
            assert_eq!(parse_member_instance_id(&id), Some(MemberInstance { group, instance: n, member }), "{id}");
            assert!(id.starts_with(&group_instance_id(group, n)), "{id}");
        }
    }

    #[test]
    fn anything_else_is_not_a_member_id() {
        for not_a_member in ["txt-name", "a.b", "a.b-0.c", "a.b-x.c", "a.b-1.c.d", ".b-1.c", "a.b-1.", "a.b-.c", "a.b--1.c", "a.b-1.c", "a.a-+1.c"] {
            assert!(parse_member_instance_id(not_a_member).is_none(), "{not_a_member}");
        }
        assert!(parse_member_instance_id(&group_instance_id("g", 2)).is_none(), "a card is not a member");
    }
}
