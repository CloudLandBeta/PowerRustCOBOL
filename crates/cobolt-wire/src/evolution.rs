// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! How the contract changes without breaking whoever is already out there
//! (WASM-014 R4, R5).
//!
//! A frontend and a backend are released at different times. The backend's
//! **new** interface description is compared with the **last released** one, and
//! every change is judged by one question per direction a value travels:
//!
//! * **Request** (the old client sends, the new backend reads): can everything
//!   the old layout writes be accepted by the new one? ([`compat::fits`])
//! * **Answer** (the new backend writes, the old client reads): can the new
//!   layout's values be read into the old one, once the members the old client
//!   never heard of have been projected away? ([`compat::fits_projected`])
//!
//! A parameter exported only for sending (`direction = "in"`) is judged on the
//! request alone; one exported only for receiving (`"out"`), on the answer
//! alone; the default, `inout`, on both — a callee cannot know how each caller
//! passes it.
//!
//! The rules, as the report names them:
//!
//! | change | verdict |
//! |---|---|
//! | add an exported program | compatible |
//! | add a member at the end of a group, add an error class | compatible (an old client never sends it and never reads it) |
//! | widen a table's `OCCURS`, a string, a number's digits | compatible for `in`, **breaking** for `out`/`inout` (the answer may no longer fit the old client's field) |
//! | remove or rename a member, change a `PICTURE`'s scale or kind, a sign, narrow a table | **breaking** |
//! | remove an exported program, change its parameters (count, order), its `RETURNING`, its roles or `public` | **breaking** |
//! | an idempotent program no longer safe to repeat | **breaking** |
//! | deprecate a program | compatible (a warning to the callers) |
//!
//! Raising the **major** version of the contract is the deliberate way through.

use crate::compat::{fits, fits_projected};
use crate::contract::{Direction, InterfaceDescription, Param, ProgramSig};

/// What comparing two interface descriptions found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The released contract compared against (`"(none)"` when there is none).
    pub against: String,
    /// Changes that break a client built against `against`.
    pub breaking: Vec<String>,
    /// Compatible changes worth saying out loud.
    pub notes: Vec<String>,
}

fn semver(v: &str) -> Option<(u64, u64, u64)> {
    let p: Vec<u64> = v.split('.').map(|x| x.parse().ok()).collect::<Option<Vec<_>>>()?;
    (p.len() == 3).then(|| (p[0], p[1], p[2]))
}

/// Compare `current` with the newest of `released`.
pub fn check_against_released(current: &InterfaceDescription, released: &[InterfaceDescription]) -> Report {
    let last = released.iter().filter_map(|d| semver(&d.contract).map(|v| (v, d))).max_by_key(|(v, _)| *v);
    let Some((last_v, last)) = last else {
        return Report { against: "(none)".to_string(), ..Report::default() };
    };
    compare(last, current, last_v)
}

/// Compare one released interface (`old`) with the new one.
pub fn compare(old: &InterfaceDescription, new: &InterfaceDescription, old_v: (u64, u64, u64)) -> Report {
    let mut r = Report { against: old.contract.clone(), ..Report::default() };
    let Some(new_v) = semver(&new.contract) else {
        r.breaking.push(format!("the contract version \"{}\" is not major.minor.patch", new.contract));
        return r;
    };
    if new_v.0 < old_v.0 {
        r.breaking.push(format!("the contract version went backwards: {} is older than the released {}", new.contract, old.contract));
        return r;
    }
    let mut breaking = Vec::new();
    let mut notes = Vec::new();

    for o in &old.programs {
        match new.program(&o.name) {
            None => breaking.push(format!("program {} was removed (rule: program-removed)", o.name)),
            Some(n) => compare_program(o, n, &mut breaking, &mut notes),
        }
    }
    for n in &new.programs {
        if old.program(&n.name).is_none() {
            notes.push(format!("program {} is new", n.name));
        }
    }

    if new_v.0 > old_v.0 {
        // A deliberate major bump: the breaks are reported as notes.
        notes.extend(breaking.drain(..).map(|b| format!("allowed by the new major version {}: {b}", new.contract)));
    } else if new_v == old_v && (!breaking.is_empty() || !notes.is_empty()) {
        notes.push(format!("the interface differs from the released contract {} but its version was not raised: raise the minor version", old.contract));
    }
    r.breaking = breaking;
    r.notes = notes;
    r
}

fn compare_program(o: &ProgramSig, n: &ProgramSig, breaking: &mut Vec<String>, notes: &mut Vec<String>) {
    let name = &o.name;
    if o.public != n.public {
        breaking.push(format!("{name}: its `public` setting changed (rule: access-changed)"));
    }
    let (mut or, mut nr) = (o.roles.clone(), n.roles.clone());
    or.sort();
    nr.sort();
    if or != nr {
        breaking.push(format!("{name}: its required roles changed from {or:?} to {nr:?} (rule: roles-changed)"));
    }
    if o.idempotent && !n.idempotent {
        breaking.push(format!("{name}: it is no longer safe to repeat (rule: idempotency-removed)"));
    } else if !o.idempotent && n.idempotent {
        notes.push(format!("{name}: it is now declared safe to repeat"));
    }
    if o.independent != n.independent {
        notes.push(format!("{name}: its ordering declaration changed (independent: {} -> {})", o.independent, n.independent));
    }
    if o.long_running != n.long_running || o.stream != n.stream {
        breaking.push(format!("{name}: its kind (long-running / stream) changed (rule: kind-changed)"));
    }
    if n.deprecated.is_some() && o.deprecated.is_none() {
        notes.push(format!("{name}: it is now deprecated"));
    }
    if o.params.len() != n.params.len() {
        breaking.push(format!("{name}: it takes {} parameter(s) now, {} before (rule: parameters-changed)", n.params.len(), o.params.len()));
    } else {
        for (i, (op, np)) in o.params.iter().zip(n.params.iter()).enumerate() {
            compare_param(name, &format!("parameter {} ({})", i + 1, op.name), op, np, breaking);
        }
    }
    match (&o.returning, &n.returning) {
        (Some(op), Some(np)) => compare_param(name, "RETURNING", op, np, breaking),
        (Some(_), None) => breaking.push(format!("{name}: it no longer returns anything (rule: returning-removed)")),
        (None, Some(_)) => notes.push(format!("{name}: it now returns a group (callers that ignore it are unaffected)")),
        (None, None) => {}
    }
}

fn compare_param(program: &str, what: &str, old: &Param, new: &Param, breaking: &mut Vec<String>) {
    // The released contract's declared direction decides what was promised.
    let dir = old.direction;
    if dir != Direction::Out {
        for m in fits(&old.layout, &new.layout) {
            breaking.push(format!("{program}: {what}: an old client's request no longer fits — {m}"));
        }
    }
    if dir != Direction::In {
        for m in fits_projected(&new.layout, &old.layout) {
            breaking.push(format!("{program}: {what}: the new answer no longer fits an old client — {m}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Item, Kind, Occurs};

    fn alpha(name: &str, len: usize) -> Item {
        Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Alphanumeric { len, justified: false }, value: None }
    }
    fn num(name: &str, digits: u16, scale: u16) -> Item {
        Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Numeric { digits, scale, signed: false }, value: None }
    }
    fn group(children: Vec<Item>) -> Item {
        Item { name: "G".into(), filler: false, redefines: false, occurs: None, kind: Kind::Group { children }, value: None }
    }
    fn iface(version: &str, params: Vec<Param>) -> InterfaceDescription {
        let mut d = InterfaceDescription::new(version);
        let mut p = ProgramSig::new("GET");
        p.params = params;
        d.programs.push(p);
        d
    }
    fn param(dir: Direction, layout: Item) -> Param {
        Param { name: "P".into(), direction: dir, layout }
    }

    #[test]
    fn compatible_changes_pass_each_direction_aware() {
        let old = iface("1.0.0", vec![param(Direction::InOut, group(vec![alpha("A", 4)]))]);
        // a trailing member is added
        let added = iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 4), num("B", 3, 0)]))]);
        let r = check_against_released(&added, &[old.clone()]);
        assert!(r.breaking.is_empty(), "{r:?}");
        // a new program
        let mut with_new = added.clone();
        with_new.programs.push(ProgramSig::new("NEW"));
        let r = check_against_released(&with_new, &[old]);
        assert!(r.breaking.is_empty() && r.notes.iter().any(|n| n.contains("NEW is new")), "{r:?}");
    }

    #[test]
    fn every_breaking_change_is_named_with_its_rule() {
        let base = iface("1.0.0", vec![param(Direction::InOut, group(vec![alpha("A", 4), num("N", 3, 2)]))]);
        let cases: Vec<(&str, InterfaceDescription)> = vec![
            ("member removed", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 4)]))])),
            ("renamed", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A2", 4), num("N", 3, 2)]))])),
            ("scale changed", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 4), num("N", 3, 1)]))])),
            ("narrowed", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 2), num("N", 3, 2)]))])),
            ("widened (breaks the answer)", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 8), num("N", 3, 2)]))])),
            ("a parameter added", iface("1.1.0", vec![param(Direction::InOut, group(vec![alpha("A", 4), num("N", 3, 2)])), param(Direction::In, group(vec![alpha("X", 1)]))])),
            ("program removed", InterfaceDescription::new("1.1.0")),
        ];
        for (label, new) in cases {
            let r = check_against_released(&new, &[base.clone()]);
            assert!(!r.breaking.is_empty(), "{label} must break: {r:?}");
        }
        // widening is fine for a request-only parameter
        let inonly = iface("1.0.0", vec![param(Direction::In, group(vec![alpha("A", 4)]))]);
        let wider = iface("1.1.0", vec![param(Direction::In, group(vec![alpha("A", 8)]))]);
        assert!(check_against_released(&wider, &[inonly]).breaking.is_empty());
    }

    #[test]
    fn a_returning_group_that_loses_a_member_breaks_older_clients() {
        fn with_returning(version: &str, layout: Item) -> InterfaceDescription {
            let mut d = InterfaceDescription::new(version);
            let mut p = ProgramSig::new("PING");
            p.returning = Some(param(Direction::Out, layout));
            d.programs.push(p);
            d
        }
        let released = with_returning("1.0.0", group(vec![alpha("REPLY", 4)]));
        // Renamed, i.e. REPLY is gone.
        let r = check_against_released(&with_returning("1.1.0", group(vec![alpha("ANSWER", 4)])), &[released.clone()]);
        assert_eq!(r.breaking.len(), 1, "{r:?}");
        assert!(r.breaking[0].contains("REPLY") && r.breaking[0].contains("RETURNING"), "{r:?}");
        // An extra member, or a wider text, does not break a reader that ignores it.
        let r = check_against_released(&with_returning("1.1.0", group(vec![alpha("REPLY", 4), alpha("NOTE", 20)])), &[released]);
        assert!(r.breaking.is_empty(), "{r:?}");
    }

    #[test]
    fn roles_idempotency_and_table_changes() {
        let mut old = iface("1.0.0", vec![]);
        old.programs[0].roles = vec!["clerk".into()];
        old.programs[0].idempotent = true;
        let mut new = old.clone();
        new.contract = "1.1.0".into();
        new.programs[0].roles = vec!["admin".into()];
        new.programs[0].idempotent = false;
        let r = check_against_released(&new, &[old]);
        assert!(r.breaking.iter().any(|b| b.contains("roles-changed")) && r.breaking.iter().any(|b| b.contains("idempotency-removed")), "{r:?}");

        let mut t = alpha("T", 2);
        t.occurs = Some(Occurs { min: 5, max: 5, depending_on: None });
        let old = iface("1.0.0", vec![param(Direction::In, group(vec![t.clone()]))]);
        let mut narrow = t;
        narrow.occurs = Some(Occurs { min: 3, max: 3, depending_on: None });
        let new = iface("1.1.0", vec![param(Direction::In, group(vec![narrow]))]);
        assert!(!check_against_released(&new, &[old]).breaking.is_empty(), "narrowing OCCURS breaks");
    }

    #[test]
    fn a_major_bump_is_the_deliberate_way_through() {
        let old = iface("1.4.0", vec![param(Direction::InOut, group(vec![alpha("A", 4)]))]);
        let new = iface("2.0.0", vec![param(Direction::InOut, group(vec![alpha("Z", 4)]))]);
        let r = check_against_released(&new, &[old]);
        assert!(r.breaking.is_empty(), "{r:?}");
        assert!(r.notes.iter().any(|n| n.contains("allowed by the new major version")));
    }

    #[test]
    fn no_release_no_comparison_and_going_backwards_is_refused() {
        let new = iface("1.0.0", vec![]);
        assert_eq!(check_against_released(&new, &[]).against, "(none)");
        let old = iface("2.0.0", vec![]);
        assert!(!check_against_released(&new, &[old]).breaking.is_empty());
    }
}
