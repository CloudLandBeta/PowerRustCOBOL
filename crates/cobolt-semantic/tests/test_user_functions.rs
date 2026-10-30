// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 §5.1: a `FUNCTION name(…)` that names a `FUNCTION-ID` unit of the
//! source (or of the project's Common Code) passes; its arity is checked at
//! build time; a unit with an intrinsic function's name is refused naming both;
//! an unknown name is still the intrinsic error.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};

const UNIT: &str = "
IDENTIFICATION DIVISION.
FUNCTION-ID. TWICE.
DATA DIVISION.
LINKAGE SECTION.
01 A PIC 9(4).
01 R PIC 9(4).
PROCEDURE DIVISION USING BY VALUE A RETURNING R.
    COMPUTE R = A * 2
    GOBACK.
END FUNCTION TWICE.
";

fn errors(main_proc: &str, units: &str, known: Option<HashSet<String>>) -> Vec<String> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. M.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 W PIC 9(4).\nPROCEDURE DIVISION.\nMAIN-PARA.\n{main_proc}\n    STOP RUN.\n{units}"
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("parses");
    analyze_with(&program, &AnalyzeOptions { known_functions: known, ..Default::default() })
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn a_function_of_the_source_passes_and_its_arity_is_checked() {
    assert!(errors("    COMPUTE W = FUNCTION TWICE(3)", UNIT, None).is_empty());
    let e = errors("    COMPUTE W = FUNCTION TWICE(3 4)", UNIT, None);
    assert!(e.iter().any(|m| m.contains("takes 1 argument(s) and is given 2")), "{e:?}");
}

#[test]
fn a_common_code_function_passes_and_an_unknown_name_does_not() {
    let known: HashSet<String> = ["LATER".to_string()].into();
    assert!(errors("    COMPUTE W = FUNCTION LATER(3)", "", Some(known)).is_empty());
    let e = errors("    COMPUTE W = FUNCTION NOBODY(3)", "", None);
    assert!(e.iter().any(|m| m.contains("not an intrinsic function")), "{e:?}");
}

#[test]
fn a_function_named_like_an_intrinsic_is_refused_naming_it() {
    let unit = UNIT.replace("TWICE", "MAX");
    let e = errors("    DISPLAY W", &unit, None);
    assert!(e.iter().any(|m| m.contains("has the name of the intrinsic function MAX")), "{e:?}");
}
