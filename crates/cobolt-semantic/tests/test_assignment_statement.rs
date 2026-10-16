// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 093 R11 / Q3: `A = expression` is a `COMPUTE` without its verb, so a text receiver is the
//! same error — and the message must not blame a `COMPUTE` that is not in the line.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors(body: &str) -> Vec<String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n\
         01 WS-N PIC 9(4).\n01 WS-T PIC X(10).\n       PROCEDURE DIVISION.\n       MAIN-P.\n{body}\n           STOP RUN.\n"
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("a program");
    analyze(&program).diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

#[test]
fn a_numeric_receiver_is_accepted_in_both_spellings() {
    assert!(errors("           WS-N = 1 + 2\n           COMPUTE WS-N = 3").is_empty());
}

#[test]
fn a_text_receiver_is_refused_in_both_spellings_with_one_message_that_names_both() {
    let shorthand = errors("           WS-T = 1");
    let verbose = errors("           COMPUTE WS-T = 1");
    assert_eq!(shorthand, verbose, "the two spellings are one statement");
    assert_eq!(shorthand.len(), 1, "{shorthand:?}");
    let message = &shorthand[0];
    assert!(message.contains("'WS-T' is not numeric") && message.contains("requires a numeric receiver"), "{message}");
    assert!(message.contains("COMPUTE") && message.contains("name = expression"), "it names both spellings: {message}");
}
