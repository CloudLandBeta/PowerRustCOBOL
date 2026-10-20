// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run`, but on the WebAssembly generator: the stand-in the NIST
//! harness drives to run the CCVS85 programs on a generated module.
//!
//! ```text
//! RCRUN=target/debug/examples/nist_wasm_run \
//!   ./target/debug/examples/nist_conformance run NC
//! ```
//!
//! It takes the same arguments as `rcrun run <file> [--source-format fixed]
//! [--switch NAME=ON|OFF]…`, expands `COPY`, parses, compiles the program in a
//! **conformance** build (sequential files through host imports) and runs it on
//! the test host with the current directory as the file system. `DISPLAY` goes
//! to standard output; a run-time error or a trap to standard error with exit
//! status 1.

use cobolt_lexer::SourceFormat;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("run") || args.len() < 2 {
        eprintln!("usage: nist_wasm_run run <file.cbl> [--source-format fixed] [--switch NAME=ON]…");
        std::process::exit(2);
    }
    let path = std::path::PathBuf::from(&args[1]);
    let fmt = match args.iter().position(|a| a == "--source-format").and_then(|i| args.get(i + 1)) {
        Some(s) if s.eq_ignore_ascii_case("fixed") => SourceFormat::FixedStrict,
        Some(s) if s.eq_ignore_ascii_case("fixed-relaxed") => SourceFormat::Fixed,
        _ => SourceFormat::Free,
    };
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("cannot read {}: {e}", path.display());
        std::process::exit(2)
    });
    let expansion = cobolt_lexer::expand_copybooks_for(&source, &path, fmt);
    for (i, e) in expansion.errors.iter().enumerate() {
        let line = expansion.error_lines.get(i).copied().unwrap_or(0);
        eprintln!("{}:{line}:0: error: copybook error: {e}", path.display());
    }
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize_expansion(&expansion));
    let mut bad = false;
    for d in &parsed.diagnostics {
        if d.is_error() {
            bad = true;
            eprintln!("{}:{}:{}: error: {}", path.display(), d.span.line, d.span.col, d.message);
        }
    }
    let Some(program) = parsed.program else {
        eprintln!("parse failed");
        std::process::exit(1)
    };
    if bad {
        eprintln!("aborting due to parse errors");
        std::process::exit(1);
    }
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let opts = cobolt_wasm::Options { conformance: true, name, ..Default::default() };
    let compiled = match cobolt_wasm::compile_program(&program, &opts) {
        Ok(c) => c,
        Err(e) => {
            for (l, m) in e.all() {
                eprintln!("{}:{l}: not compiled: {m}", path.display());
            }
            std::process::exit(1);
        }
    };
    let cwd = std::env::current_dir().unwrap();
    let outcome = match cobolt_wasm::host::run_conformance(&compiled.wasm, cobolt_wasm::host::Conformance { cwd }) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("host: {e}");
            std::process::exit(1)
        }
    };
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(outcome.output.as_bytes());
    let _ = lock.flush();
    for e in &outcome.errors {
        eprintln!("run-time error {} at line {}", e.code, e.line);
    }
    if let Some(t) = &outcome.trapped {
        eprintln!("trap: {t}");
    }
    if outcome.status < 0 && outcome.status != -1 && outcome.status != -4 {
        std::process::exit(1);
    }
}
