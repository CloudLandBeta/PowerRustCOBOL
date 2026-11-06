// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Every control type has a decision** (WASM-015 T8, AC4 second half).

use cobolt_forms::model::ControlType;
use cobolt_forms::web_class::{class_of, Class};
use cobolt_web_tests::coverage::{mismatches, Decision, COVERAGE};

fn letter(c: Class) -> char {
    match c {
        Class::A => 'A',
        Class::B => 'B',
        Class::C => 'C',
        Class::D => 'D',
        Class::E => 'E',
    }
}

#[test]
fn every_control_type_has_one_row_with_its_class_and_a_decision() {
    let all: Vec<&str> = ControlType::ALL.iter().map(|t| t.as_str()).collect();
    let (missing, extra) = mismatches(COVERAGE, &all);
    assert!(missing.is_empty(), "control types with no COVERAGE row: {missing:?}");
    assert!(extra.is_empty(), "COVERAGE rows for no control type: {extra:?}");
    assert_eq!(COVERAGE.len(), all.len(), "one row each");
    for t in ControlType::ALL {
        let row = COVERAGE.iter().find(|r| r.0 == t.as_str()).unwrap();
        assert_eq!(row.1, letter(class_of(t)), "{}: the row's class is the classification's", row.0);
    }
    let ab = COVERAGE.iter().filter(|r| matches!(r.1, 'A' | 'B')).count();
    let ab_covered = COVERAGE.iter().filter(|r| matches!(r.1, 'A' | 'B') && r.2 == Decision::CoveredNow).count();
    let gaps: Vec<&str> = COVERAGE.iter().filter(|r| matches!(r.2, Decision::KnownGap(_))).map(|r| r.0).collect();
    assert_eq!(ab_covered, ab, "every class A/B control is covered now");
    assert!(COVERAGE.iter().filter(|r| matches!(r.2, Decision::KnownGap(_))).all(|r| r.1 == 'C'), "a known gap is a class C control");
    println!("{} of {} rows, A/B covered {} of {}, known gaps {} (class C: {})", COVERAGE.len(), all.len(), ab_covered, ab, gaps.len(), gaps.join(", "));
}

/// A planted missing row, and a planted extra one, are found.
#[test]
fn a_missing_or_extra_row_fails_the_check() {
    let all = ["Button", "Label", "Brand-new"];
    let (missing, extra) = mismatches(COVERAGE, &all);
    assert_eq!(missing, ["Brand-new"]);
    assert!(extra.len() > 60, "every other row is extra against this three-name list");
    let short = &COVERAGE[1..];
    let names: Vec<&str> = COVERAGE.iter().map(|r| r.0).collect();
    let (missing, extra) = mismatches(short, &names);
    assert_eq!(missing, [COVERAGE[0].0]);
    assert!(extra.is_empty());
}
