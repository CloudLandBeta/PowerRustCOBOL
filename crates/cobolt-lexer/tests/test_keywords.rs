// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: keyword recognition (single-word and compound).

use cobolt_lexer::{tokenize, SourceFormat, Token};

fn tokens_from_free(src: &str) -> Vec<Token> {
    tokenize(src, SourceFormat::Free)
        .into_iter()
        .map(|st| st.token)
        .collect()
}

#[test]
fn identification_division() {
    let toks = tokens_from_free("IDENTIFICATION DIVISION.");
    assert_eq!(toks[0], Token::Identification);
    assert_eq!(toks[1], Token::Division);
    assert_eq!(toks[2], Token::Period);
}

#[test]
fn id_abbreviation() {
    let toks = tokens_from_free("ID DIVISION.");
    assert_eq!(toks[0], Token::Identification);
}

#[test]
fn data_division() {
    let toks = tokens_from_free("DATA DIVISION.");
    assert_eq!(toks[0], Token::Data);
    assert_eq!(toks[1], Token::Division);
}

#[test]
fn procedure_division() {
    let toks = tokens_from_free("PROCEDURE DIVISION.");
    assert_eq!(toks[0], Token::Procedure);
}

#[test]
fn working_storage_section() {
    let toks = tokens_from_free("WORKING-STORAGE SECTION.");
    assert_eq!(toks[0], Token::WorkingStorage, "WORKING-STORAGE should be a single token");
    assert_eq!(toks[1], Token::Section);
}

#[test]
fn local_storage_section() {
    let toks = tokens_from_free("LOCAL-STORAGE SECTION.");
    assert_eq!(toks[0], Token::LocalStorage);
}

#[test]
fn input_output_section() {
    let toks = tokens_from_free("INPUT-OUTPUT SECTION.");
    assert_eq!(toks[0], Token::InputOutput);
}

#[test]
fn file_control() {
    let toks = tokens_from_free("FILE-CONTROL.");
    assert_eq!(toks[0], Token::FileControl);
}

#[test]
fn end_if_token() {
    let toks = tokens_from_free("END-IF");
    assert_eq!(toks[0], Token::EndIf);
}

#[test]
fn end_evaluate_token() {
    let toks = tokens_from_free("END-EVALUATE");
    assert_eq!(toks[0], Token::EndEvaluate);
}

#[test]
fn end_perform_token() {
    // END-PERFORM is a scope terminator, exactly like END-IF / END-EVALUATE, and
    // the parser relies on `Token::EndPerform` to close inline PERFORM blocks.
    let toks = tokens_from_free("END-PERFORM");
    assert_eq!(toks[0], Token::EndPerform);
}

#[test]
fn comp_variants() {
    let cases = [
        ("COMP",           Token::Comp),
        ("COMPUTATIONAL",  Token::Comp),
        ("COMP-1",         Token::Comp1),
        ("COMP-2",         Token::Comp2),
        ("COMP-3",         Token::Comp3),
        ("COMP-5",         Token::Comp5),
        ("PACKED-DECIMAL", Token::PackedDecimal),
        ("BINARY",         Token::Binary),
    ];
    for (word, expected) in cases {
        let toks = tokens_from_free(word);
        assert_eq!(toks[0], expected, "Failed for {word}");
    }
}

#[test]
fn figurative_constants() {
    assert_eq!(tokens_from_free("SPACES")[0],      Token::Spaces);
    assert_eq!(tokens_from_free("SPACE")[0],       Token::Spaces);
    assert_eq!(tokens_from_free("ZEROS")[0],       Token::Zeros);
    assert_eq!(tokens_from_free("ZEROES")[0],      Token::Zeros);
    assert_eq!(tokens_from_free("ZERO")[0],        Token::Zeros);
    assert_eq!(tokens_from_free("HIGH-VALUES")[0], Token::HighValues);
    assert_eq!(tokens_from_free("HIGH-VALUE")[0],  Token::HighValues);
    assert_eq!(tokens_from_free("LOW-VALUES")[0],  Token::LowValues);
    assert_eq!(tokens_from_free("QUOTES")[0],      Token::Quotes);
    assert_eq!(tokens_from_free("NULLS")[0],       Token::Nulls);
}

#[test]
fn lowercase_keywords() {
    let toks = tokens_from_free("move ws-a to ws-b.");
    assert_eq!(toks[0], Token::Move);
    assert_eq!(toks[1], Token::Identifier("WS-A".to_string()));
    assert_eq!(toks[2], Token::To);
    assert_eq!(toks[3], Token::Identifier("WS-B".to_string()));
    assert_eq!(toks[4], Token::Period);
}

#[test]
fn mixed_case_keywords() {
    let toks = tokens_from_free("Move Ws-Counter To Ws-Total.");
    assert_eq!(toks[0], Token::Move);
    assert_eq!(toks[1], Token::Identifier("WS-COUNTER".to_string()));
    assert_eq!(toks[2], Token::To);
}

#[test]
fn powercobol_keywords() {
    assert_eq!(tokens_from_free("WINDOW-STATUS")[0],       Token::WindowStatus);
    assert_eq!(tokens_from_free("COBOLT-WAIT-EVENT")[0],   Token::CoboltWaitEvent);
    assert_eq!(tokens_from_free("COBOLT-SET-PROPERTY")[0], Token::CoboltSetProperty);
    assert_eq!(tokens_from_free("COBOLT-GET-PROPERTY")[0], Token::CoboltGetProperty);
}

#[test]
fn user_identifiers_preserved() {
    let names = [
        "WS-COUNTER", "MY-FLAG", "SCREEN-DATA", "END-PROGRAM-ID",
        "TOTAL-AMOUNT", "BUTTON1-CLICK", "CUSTOMER-NAME",
    ];
    for name in names {
        let toks = tokens_from_free(name);
        assert_eq!(
            toks[0],
            Token::Identifier(name.to_string()),
            "{name} should be an identifier, not a keyword"
        );
    }
}

#[test]
fn program_id_paragraph() {
    let src = "PROGRAM-ID. MY-APP.";
    let toks = tokens_from_free(src);
    assert_eq!(toks[0], Token::ProgramId);
    assert_eq!(toks[1], Token::Period);
    assert_eq!(toks[2], Token::Identifier("MY-APP".to_string()));
    assert_eq!(toks[3], Token::Period);
}
