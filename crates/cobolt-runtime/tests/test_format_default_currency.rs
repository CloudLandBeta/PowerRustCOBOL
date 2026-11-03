// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 T2.2: the project's `[project] currency` is the currency of a format that names
//! none. The default is process-wide, so this file is its own test binary and one test.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{publish_format_currency, publish_format_locale, Interpreter};

fn run(procedure: &str) -> Result<String, String> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. FMT.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-AMT PIC S9(9)V99 VALUE 1234.5.\nPROCEDURE DIVISION.\nMAIN.\n{procedure}\n    STOP RUN.\n"
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("no program");
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    match interp.run() {
        Ok(_) => Ok(display_rx.try_iter().collect::<Vec<_>>().join("|")),
        Err(e) => Err(e.to_string()),
    }
}

#[test]
fn a_format_without_a_currency_uses_the_projects_default() {
    let t0 = Instant::now();
    publish_format_locale("pt-BR");
    // No default: a bare `currency` is an error that says so.
    publish_format_currency("");
    let none = run("DISPLAY WS-AMT::Format(\"currency\")").unwrap_err();
    assert!(none.to_ascii_lowercase().contains("currency"), "{none}");
    // A default, written in lower case in the manifest, is read as its code.
    publish_format_currency("brl");
    assert_eq!(run("DISPLAY WS-AMT::Format(\"currency\")").unwrap(), "R$\u{a0}1.234,50");
    assert_eq!(run("DISPLAY WS-AMT::Format(\"currency-short\")").unwrap(), "R$\u{a0}1,2\u{a0}mil");
    // A currency named in the format wins over the default.
    assert_eq!(run("DISPLAY WS-AMT::Format(\"currency USD\")").unwrap(), "US$\u{a0}1.234,50");
    // Cleared again, the error returns.
    publish_format_currency("");
    assert!(run("DISPLAY WS-AMT::Format(\"currency\")").is_err());
    println!("default currency: absent -> error; brl -> BRL; named currency wins ({:?})", t0.elapsed());
    publish_format_locale("");
}
