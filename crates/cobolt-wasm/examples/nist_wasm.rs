// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The NIST CCVS85 programs against the WebAssembly generator.
//!
//! ```text
//! cargo run -p cobolt-wasm --example nist_wasm -- census NC      # compile every NC member, bucket what is refused
//! cargo run -p cobolt-wasm --example nist_wasm -- compile NC101A # one member, every diagnostic
//! ```
//!
//! `census` reads `NIST/newcob.val,cbl`, splits it into members and runs each
//! through the front end (as `rcrun --source-format=fixed` does) and then the
//! generator, then prints how many compiled and which constructs the rest
//! needed, most common first. It is the instrument of WASM-010 R16/AC7: two
//! numbers, never one — see also the `run` pass in the NIST ledger.

use cobolt_lexer::SourceFormat;
use std::collections::BTreeMap;

struct Member {
    kind: String,
    name: String,
    text: String,
}

fn split_members(source: &str) -> Vec<Member> {
    let mut out = Vec::new();
    let mut cur: Option<Member> = None;
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("*HEADER,") {
            if let Some(m) = cur.take() {
                out.push(m);
            }
            let mut p = rest.split(',');
            let kind = p.next().unwrap_or("").trim().to_string();
            let test = p.next().unwrap_or("").trim().to_string();
            let sub = p.next().unwrap_or("").trim().to_string();
            let subname = p.next().unwrap_or("").trim().to_string();
            let named_apart = (sub == "SUBPRG" || sub == "SUBRTN") && !subname.is_empty();
            let raw = if named_apart { subname } else { test };
            let name = raw.split_whitespace().next().unwrap_or("").to_string();
            cur = Some(Member { kind, name, text: String::new() });
        } else if line.starts_with("*END-OF,") {
            if let Some(m) = cur.take() {
                out.push(m);
            }
        } else if let Some(m) = cur.as_mut() {
            m.text.push_str(line);
            m.text.push('\n');
        }
    }
    if let Some(m) = cur.take() {
        out.push(m);
    }
    out
}

/// Compile one member; `Ok(())` or every diagnostic.
fn compile_member(m: &Member) -> Result<(), Vec<String>> {
    let path = std::path::PathBuf::from(format!("{}.cbl", m.name));
    let expansion = cobolt_lexer::expand_copybooks_for(&m.text, &path, SourceFormat::FixedStrict);
    let tokens = cobolt_lexer::tokenize_expansion(&expansion);
    let parsed = cobolt_parser::parse(tokens);
    let errs: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("parse: line {}: {}", d.span.line, d.message)).collect();
    let Some(program) = parsed.program else { return Err(vec!["parse: no program".into()]) };
    if !errs.is_empty() {
        return Err(errs);
    }
    let opts = cobolt_wasm::Options { conformance: true, name: m.name.clone(), ..Default::default() };
    match cobolt_wasm::compile_program(&program, &opts) {
        Ok(c) => {
            if let Err(e) = wasmparser::validate(&c.wasm) {
                return Err(vec![format!("invalid module: {e}")]);
            }
            Ok(())
        }
        Err(e) => Err(e.all().iter().map(|(l, m)| format!("line {l}: {m}")).collect()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("census");
    let filter = args.get(1).cloned().unwrap_or_default().to_uppercase();
    let root = cobolt_wasm::rtsrc::repo_root();
    let corpus = std::fs::read_to_string(root.join("NIST/newcob.val,cbl")).expect("NIST/newcob.val,cbl");
    let members: Vec<Member> = split_members(&corpus)
        .into_iter()
        .filter(|m| m.kind == "COBOL" && m.name.starts_with(&filter))
        .collect();
    match mode {
        "compile" => {
            for m in &members {
                match compile_member(m) {
                    Ok(()) => println!("{}: compiles", m.name),
                    Err(es) => {
                        println!("{}: {} diagnostic(s)", m.name, es.len());
                        for e in es.iter().take(40) {
                            println!("    {e}");
                        }
                    }
                }
            }
        }
        _ => {
            let mut ok = 0usize;
            let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for m in &members {
                // The generator panics are bugs; keep going.
                let r = std::panic::catch_unwind(|| compile_member(m));
                match r {
                    Ok(Ok(())) => ok += 1,
                    Ok(Err(es)) => {
                        let mut seen = std::collections::BTreeSet::new();
                        for e in es {
                            // strip the line number: bucket by the construct
                            let key = e.split_once(": ").map(|x| x.1).unwrap_or(&e).to_string();
                            let key = key.split(" is not defined").next().unwrap_or(&key).to_string();
                            if seen.insert(key.clone()) {
                                buckets.entry(key).or_default().push(m.name.clone());
                            }
                        }
                    }
                    Err(_) => buckets.entry("PANIC in the generator".into()).or_default().push(m.name.clone()),
                }
            }
            println!("=== {} members matching '{}': {} compile, {} do not ===", members.len(), filter, ok, members.len() - ok);
            let mut v: Vec<_> = buckets.into_iter().collect();
            v.sort_by_key(|(_, names)| std::cmp::Reverse(names.len()));
            for (k, names) in v.iter().take(60) {
                let sample: Vec<&str> = names.iter().take(4).map(String::as_str).collect();
                println!("{:4} programs  {}   [{}]", names.len(), k, sample.join(" "));
            }
        }
    }
}
