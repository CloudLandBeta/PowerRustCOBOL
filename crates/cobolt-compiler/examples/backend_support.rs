// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the WebAssembly generator accepts of the Backends the repository ships (spec 114 R15, task T1).
//!
//! ```text
//! cargo run -p cobolt-compiler --example backend_support [-- <folder of projects> [<project>...]]
//! ```
//!
//! For each project's `backend/` folder the Backend is loaded exactly as the server loads it
//! (`Backend::load`: every program parsed and analysed, the copybooks resolved, every exported
//! program carrying the others nested in it), then each exported program is compiled alone as a
//! module the way a Desktop program is (`Options { native: true, .. }`). Nothing is built, written or
//! run: the figures say what the generator accepts today and, for what it refuses, which construct,
//! at which line and for what reason, so that the order of the work is read from them and not
//! guessed. One result block per Backend, then a total.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use cobolt_server::backend::Backend;
use cobolt_wasm::{compile_program, Options};

/// The Backends the repository ships (counted in spec 114 §0.1).
const SHIPPED: [&str; 4] = ["PowerOficina", "PowerAnalyticsWeb", "PowerWebHello", "PowerMobileHello"];

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples"));
    let named: Vec<String> = args.collect();
    let projects: Vec<String> = if named.is_empty() { SHIPPED.iter().map(|s| s.to_string()).collect() } else { named };

    let (mut tried_all, mut accepted_all, mut refusals_all) = (0usize, 0usize, 0usize);
    let mut by_reason_all: BTreeMap<String, usize> = BTreeMap::new();
    let started = Instant::now();

    for name in &projects {
        let dir = root.join(name).join("backend");
        println!("== {name}");
        let backend = match Backend::load(&dir) {
            Ok(b) => b,
            Err(errs) => {
                println!("   the Backend does not load ({} diagnostic(s)):", errs.len());
                for e in errs.iter().take(10) {
                    println!("     {e}");
                }
                continue;
            }
        };
        let mut programs: Vec<_> = backend.programs().collect();
        programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));

        let (mut tried, mut accepted) = (0usize, 0usize);
        let mut by_reason: BTreeMap<String, usize> = BTreeMap::new();
        let mut refused_lines: Vec<String> = Vec::new();
        let mut capabilities: BTreeMap<String, usize> = BTreeMap::new();
        let (mut bytes_total, mut ms_total) = (0usize, 0u128);
        for p in &programs {
            tried += 1;
            let opts = Options { native: true, name: p.sig.name.to_ascii_lowercase(), ..Options::default() };
            let t = Instant::now();
            match compile_program(&p.program, &opts) {
                Ok(module) => {
                    accepted += 1;
                    ms_total += t.elapsed().as_millis();
                    bytes_total += module.wasm.len();
                    if let Some(caps) = module.manifest.get("capabilities").and_then(|c| c.as_array()) {
                        for c in caps.iter().filter_map(|c| c.as_str()) {
                            *capabilities.entry(c.to_string()).or_default() += 1;
                        }
                    }
                }
                Err(e) => {
                    for d in e.all_kinded() {
                        let reason = format!("{:?}", d.kind);
                        *by_reason.entry(reason.clone()).or_default() += 1;
                        refused_lines.push(format!("{}: line {} in {}: [{reason}] {} {}", p.sig.name, d.line, if d.program.is_empty() { "(the program)" } else { &d.program }, d.subject, d.message));
                    }
                }
            }
        }
        let refusals: usize = by_reason.values().sum();
        println!("   exported programs tried:  {tried}");
        println!("   accepted as a module:     {accepted}");
        println!("   refused:                  {} program(s), {refusals} diagnostic(s)", tried - accepted);
        if accepted > 0 {
            println!("   accepted modules:         {bytes_total} bytes in all, generated in {ms_total} ms");
            let caps: Vec<String> = capabilities.iter().map(|(c, n)| format!("{c} ({n})")).collect();
            println!("   capabilities declared:    {}", if caps.is_empty() { "none".to_string() } else { caps.join(", ") });
        }
        for (reason, n) in &by_reason {
            println!("   refusal by reason:        {reason}: {n}");
        }
        for l in refused_lines.iter().take(40) {
            println!("     {l}");
        }
        if refused_lines.len() > 40 {
            println!("     … and {} more", refused_lines.len() - 40);
        }
        tried_all += tried;
        accepted_all += accepted;
        refusals_all += refusals;
        for (r, n) in by_reason {
            *by_reason_all.entry(r).or_default() += n;
        }
    }

    println!("== RESULT backend_support: {} Backend(s), {tried_all} exported program(s) tried, {accepted_all} accepted, {} refused ({refusals_all} diagnostic(s)), in {} ms", projects.len(), tried_all - accepted_all, started.elapsed().as_millis());
    for (reason, n) in &by_reason_all {
        println!("   {reason}: {n}");
    }
}
