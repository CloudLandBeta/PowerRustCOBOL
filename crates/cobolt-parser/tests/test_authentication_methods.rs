// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 T3 — the Authentication control's methods are in the closed vocabulary, so
//! `AUTH-1::GetClaim( "sub" )` is a call and not a subscript, and `GrantConsent` /
//! `DenyConsent` are calls on their own line.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn parses(body: &str) -> (usize, String) {
    let code = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-SUB PIC X(64).\nPROCEDURE DIVISION.\nMAIN.\n{body}"
    );
    let result = parse(tokenize(&code, SourceFormat::Free));
    let errors = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
    (errors, format!("{:?}", result.program.map(|p| p.procedure.body)))
}

#[test]
fn get_claim_inline_is_a_method_call() {
    let (errors, body) = parses("    SET WS-SUB TO AUTH-1::GetClaim( \"sub\" )\n    STOP RUN.\n");
    assert_eq!(errors, 0, "{body}");
    assert!(body.contains("member: \"GETCLAIM\"") && body.contains("parens: true"), "{body}");
    // The runtime reads `member( … )` as a call exactly when the name is known (else a subscript).
    assert!(cobolt_ast::methods::is_known_method("GetClaim"));
    assert!(!cobolt_ast::methods::is_known_method("Claims"), "the negative control: an unknown name stays a subscript");
}

#[test]
fn consent_methods_parse_as_statements_and_through_invoke() {
    for m in ["GrantConsent", "DenyConsent", "GetClaim"] {
        // A statement `Ctrl::Name( arg )` opens a call only for a known name (`at_method_call`).
        let (errors, body) = parses(&format!("    AUTH-1::{m}( \"x\" )\n    DISPLAY \"after\"\n    STOP RUN.\n"));
        assert_eq!(errors, 0, "{m}: {body}");
        let member = format!("member: \"{}\"", m.to_ascii_uppercase());
        assert!(body.starts_with("Some(Paragraphs([Paragraph { name: \"MAIN\", stmts: [InvokeExpr") && body.contains(&member), "{m}: {body}");
        assert!(body.contains("\"after\""), "the next statement follows the call: {body}");
    }
    let (errors, body) = parses("    INVOKE AUTH-1 \"GrantConsent\"\n    INVOKE AUTH-1 \"DenyConsent\"\n    STOP RUN.\n");
    assert_eq!(errors, 0, "{body}");
    println!("GetClaim / GrantConsent / DenyConsent: inline, statement and INVOKE forms parse");
}
