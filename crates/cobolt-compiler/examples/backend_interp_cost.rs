// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What one call costs on the interpreter, the engine the Backend runs on today (spec 114, task T2 (c)).
//!
//! ```text
//! cargo run --release -p cobolt-compiler --example backend_interp_cost [-- <project> <program> <calls> [<json argument>...]]
//! ```
//!
//! The Backend is loaded as the server loads it, then one exported program is called through
//! `cobolt_server::exec::execute` the way a connection's call is, `calls` times (default 10,000), each
//! with a fresh `ExecOptions`. The call builds an interpreter from a copy of the program's tree, runs it
//! and encodes the answer; the figure printed is that whole cost, and beside it the cost of copying the
//! tree alone, which is the part a compiled module has no counterpart of. The module's side of the same
//! comparison is `cobolt-runner-engine`'s `backend_spikes` (set-up of one call) and the differential
//! tests of spec 114 (the answers).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_server::backend::Backend;
use cobolt_server::exec::{execute, ExecOptions};

fn pct(sorted: &[f64], p: f64) -> f64 {
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let project = args.next().unwrap_or_else(|| "PowerOficina".into());
    let program = args.next().unwrap_or_else(|| "HEALTH".into());
    let calls: usize = args.next().map(|n| n.parse().expect("the number of calls")).unwrap_or(10_000);
    let mut json: Vec<String> = args.collect();
    if json.is_empty() && program.eq_ignore_ascii_case("HEALTH") {
        json.push("\"\"".into());
    }

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(&project).join("backend");
    let backend = Backend::load(&dir).unwrap_or_else(|e| panic!("{project}: the Backend does not load: {e:?}"));
    let prog = backend.programs().find(|p| p.sig.name.eq_ignore_ascii_case(&program)).unwrap_or_else(|| panic!("{project} exports no {program}"));
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(&project);
    std::env::set_current_dir(&cwd).expect("the project folder");

    let call = || {
        let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_millis(100));
        execute(prog, &json, opts)
    };
    let first = call();
    let answer = match &first {
        Ok(r) => format!("answers args {:?}, returning {:?}", r.args, r.returning),
        Err(e) => format!("is refused: {e:?}"),
    };
    println!("== {project} {program}: the call {answer}");
    for _ in 0..200 {
        let _ = call();
    }
    let mut us = Vec::with_capacity(calls);
    for _ in 0..calls {
        let t = Instant::now();
        let _ = std::hint::black_box(call());
        us.push(t.elapsed().as_secs_f64() * 1e6);
    }
    us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = us.iter().sum::<f64>() / us.len() as f64;
    println!("   whole call through exec::execute      n={calls:>6}  median {:>9.2} us  p99 {:>9.2} us  max {:>10.2} us  mean {:>9.2} us", pct(&us, 0.5), pct(&us, 0.99), us[us.len() - 1], mean);

    let mut clone_us = Vec::with_capacity(calls);
    for _ in 0..calls {
        let t = Instant::now();
        let copy = std::hint::black_box(prog.program.clone());
        clone_us.push(t.elapsed().as_secs_f64() * 1e6);
        drop(copy);
    }
    clone_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cmean = clone_us.iter().sum::<f64>() / clone_us.len() as f64;
    println!("   the tree's copy alone                 n={calls:>6}  median {:>9.2} us  p99 {:>9.2} us  max {:>10.2} us  mean {:>9.2} us", pct(&clone_us, 0.5), pct(&clone_us, 0.99), clone_us[clone_us.len() - 1], cmean);
    println!("== RESULT backend_interp_cost: {project} {program}, {calls} calls, whole {:.0} us median, copy {:.0} us median ({:.0}% of it)", pct(&us, 0.5), pct(&clone_us, 0.5), pct(&clone_us, 0.5) / pct(&us, 0.5) * 100.0);
}
