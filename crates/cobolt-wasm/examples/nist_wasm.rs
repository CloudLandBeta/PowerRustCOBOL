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
//! cargo run -p cobolt-wasm --example nist_wasm -- regress NC IF  # both numbers, per module, beside the interpreter's
//! cargo run -p cobolt-wasm --example nist_wasm -- regress NC IF IC --record  # and write the `wasm` ledger objects
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
    let opts = cobolt_wasm::Options { conformance: true, tolerate_undeclared: true, name: m.name.clone(), ..Default::default() };
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

/// Compile every member; how many compiled, and the refused ones bucketed by the construct.
fn census(members: &[Member]) -> (usize, BTreeMap<String, Vec<String>>) {
    let mut ok = 0usize;
    let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in members {
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
    (ok, buckets)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("census");
    if mode == "regress" {
        regress(&args[1..]);
        return;
    }
    let filter = args.get(1).cloned().unwrap_or_default().to_uppercase();
    let root = cobolt_wasm::rtsrc::repo_root();
    let corpus = std::fs::read_to_string(root.join("NIST/newcob.val,cbl")).expect("NIST/newcob.val,cbl");
    let members: Vec<Member> = split_members(&corpus)
        .into_iter()
        .filter(|m| m.kind == "COBOL" && m.name.starts_with(&filter))
        .collect();
    match mode {
        "extract" => {
            for m in &members {
                print!("{}", m.text);
            }
        }
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
            let (ok, buckets) = census(&members);
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

// ── regress: the one-command regression (WASM-015 T12) ───────────────────────

/// What `nist_conformance run <MODULE>` printed, with `nist_wasm_run` as the runner.
#[derive(Default)]
struct Executed {
    in_scope: usize,
    clean: usize,
    pass: usize,
    fail: usize,
    deleted: usize,
    /// "NAME  MOD  what" lines of the programs that compiled but did not run clean.
    unclean: Vec<String>,
}

fn number_after_colon(line: &str) -> usize {
    line.split_once(':').and_then(|(_, r)| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn parse_run(out: &str) -> Executed {
    let mut e = Executed::default();
    let mut in_problems = false;
    for line in out.lines() {
        let t = line.trim_start();
        if t.starts_with("in scope") {
            e.in_scope = number_after_colon(t);
        } else if t.starts_with("…reporting 0 failures") {
            e.clean = number_after_colon(t);
        } else if t.starts_with("PASS    :") {
            e.pass = number_after_colon(t);
        } else if t.starts_with("FAIL    :") {
            e.fail = number_after_colon(t);
        } else if t.starts_with("DELETED :") {
            e.deleted = number_after_colon(t);
        } else if t.starts_with("--- every program that compiled but did not run clean") {
            in_problems = true;
        } else if in_problems {
            if t.is_empty() || t.starts_with("NOTE:") {
                in_problems = false;
            } else if !t.starts_with('…') {
                e.unclean.push(t.to_string());
            }
        }
    }
    e
}

/// `[clean, total]` and the assertion tally the interpreter has in the ledger.
fn interpreter_numbers(ledger: &serde_json::Value, module: &str) -> Option<(usize, usize, usize, usize, usize, usize)> {
    let m = ledger.get("modules")?.get(module)?;
    let pair = |k: &str| Some((m.get(k)?.get(0)?.as_u64()? as usize, m.get(k)?.get(1)?.as_u64()? as usize));
    let (cc, ct) = pair("compile")?;
    let (ec, et) = pair("execution")?;
    let a = m.get("assertions")?;
    Some((cc, ct, ec, et, a.get("pass")?.as_u64()? as usize, a.get("fail")?.as_u64()? as usize))
}

fn current_version(root: &std::path::Path) -> String {
    let text = std::fs::read_to_string(root.join("crates/cobolt-ide/src/version.rs")).unwrap_or_default();
    text.lines()
        .find(|l| l.contains("pub const VERSION"))
        .and_then(|l| l.split('"').nth(1))
        .unwrap_or("unknown")
        .to_string()
}

/// Build the two harness binaries (a stale `nist_wasm_run` would score the wrong generator) and return their directory.
fn build_harness(root: &std::path::Path) -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let dir = exe.parent().expect("examples dir").to_path_buf();
    let release = dir.parent().and_then(|p| p.file_name()).is_some_and(|n| n == "release");
    let mut cmd = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root).args(["build", "-q", "-p", "cobolt-wasm", "--example", "nist_wasm_run", "-p", "cobolt-semantic", "--example", "nist_conformance"]);
    if release {
        cmd.arg("--release");
    }
    let st = cmd.status().expect("cargo build");
    assert!(st.success(), "building nist_wasm_run and nist_conformance failed");
    dir
}

/// The `"wasm"` object, indented to sit inside a module (6 spaces).
fn wasm_object(version: &str, compile: (usize, usize), exec: (usize, usize), x: &Executed) -> Vec<String> {
    vec![
        "      \"wasm\": {".into(),
        "        \"engine\": \"wasmtime\",".into(),
        format!("        \"measured_at_version\": \"{version}\","),
        "        \"compile\": [".into(),
        format!("          {},", compile.0),
        format!("          {}", compile.1),
        "        ],".into(),
        "        \"execution\": [".into(),
        format!("          {},", exec.0),
        format!("          {}", exec.1),
        "        ],".into(),
        "        \"assertions\": {".into(),
        format!("          \"pass\": {},", x.pass),
        format!("          \"fail\": {},", x.fail),
        format!("          \"deleted\": {}", x.deleted),
        "        }".into(),
        "      }".into(),
    ]
}

/// Put `obj` into `MODULE`'s object in the ledger text: replace a `"wasm"` key that is there, else add one
/// after the last key. Only the lines of the `wasm` object change; nothing else in the file is re-written.
fn put_wasm(lines: &mut Vec<String>, module: &str, obj: Vec<String>) -> Result<(), String> {
    let modules = lines.iter().position(|l| l == "  \"modules\": {").ok_or("no \"modules\" object")?;
    let start = (modules..lines.len()).find(|&i| lines[i] == format!("    \"{module}\": {{")).ok_or(format!("no module {module} in the ledger"))?;
    let end = (start + 1..lines.len()).find(|&i| lines[i] == "    }" || lines[i] == "    },").ok_or("unterminated module object")?;
    if let Some(w) = (start + 1..end).find(|&i| lines[i] == "      \"wasm\": {" || lines[i] == "      \"wasm\": {},") {
        let w_end = (w..end).find(|&i| lines[i] == "      }" || lines[i] == "      },").unwrap_or(w);
        let comma = lines[w_end].ends_with(',');
        let mut obj = obj;
        if comma {
            let last = obj.len() - 1;
            obj[last].push(',');
        }
        lines.splice(w..=w_end, obj);
    } else {
        if !lines[end - 1].ends_with(',') {
            lines[end - 1].push(',');
        }
        lines.splice(end..end, obj);
    }
    Ok(())
}

fn regress(args: &[String]) {
    let record = args.iter().any(|a| a == "--record");
    let modules: Vec<String> = args.iter().filter(|a| !a.starts_with("--")).map(|a| a.to_uppercase()).collect();
    if modules.is_empty() {
        eprintln!("usage: nist_wasm regress <MODULE>… [--record]   (e.g. regress NC IF IC)");
        std::process::exit(2);
    }
    let root = cobolt_wasm::rtsrc::repo_root();
    let ledger_path = root.join("NIST/progress.json");
    let ledger_text = std::fs::read_to_string(&ledger_path).expect("NIST/progress.json");
    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).expect("NIST/progress.json is JSON");
    let corpus = std::fs::read_to_string(root.join("NIST/newcob.val,cbl")).expect("NIST/newcob.val,cbl");
    let all = split_members(&corpus);
    let dir = build_harness(&root);
    let runner = dir.join("nist_wasm_run");
    let conformance = dir.join("nist_conformance");
    let version = current_version(&root);
    let mut lines: Vec<String> = ledger_text.lines().map(String::from).collect();
    let mut regressed = false;
    println!("=== WebAssembly generator on wasmtime, version {version} ===\n");
    for module in &modules {
        let members: Vec<&Member> = all.iter().filter(|m| m.kind == "COBOL" && m.name.starts_with(module.as_str())).collect();
        if members.is_empty() {
            eprintln!("{module}: no such module in NIST/newcob.val,cbl");
            std::process::exit(2);
        }
        let owned: Vec<Member> = members.iter().map(|m| Member { kind: m.kind.clone(), name: m.name.clone(), text: m.text.clone() }).collect();
        let t0 = std::time::Instant::now();
        let (ok, buckets) = census(&owned);
        let compile_ms = t0.elapsed().as_millis();
        let t1 = std::time::Instant::now();
        let out = std::process::Command::new(&conformance)
            .current_dir(&root)
            .env("RCRUN", &runner)
            .args(["run", module])
            .output()
            .expect("run nist_conformance");
        let x = parse_run(&String::from_utf8_lossy(&out.stdout));
        let run_ms = t1.elapsed().as_millis();
        let total = owned.len();
        println!("--- {module} ---");
        println!("  compile    : {ok} of {total} programs        ({compile_ms} ms)");
        println!("  execution  : {} of {} programs ran with 0 failures   ({run_ms} ms)", x.clean, x.in_scope);
        println!("  assertions : {} PASS, {} FAIL, {} DELETED", x.pass, x.fail, x.deleted);
        if let Some((cc, ct, ec, et, ip, if_)) = interpreter_numbers(&ledger, module) {
            println!("  interpreter: compile {cc} of {ct}, execution {ec} of {et}, assertions {ip} PASS, {if_} FAIL   (the ledger's numbers)");
            if ok < cc || x.clean < ec || x.fail > if_ {
                regressed = true;
                println!("  REGRESSION : the generator is behind the interpreter on this module");
            }
        } else {
            println!("  interpreter: no ledger row for {module}");
        }
        if ok < total {
            println!("  excluded (did not compile), by cause:");
            let mut v: Vec<_> = buckets.iter().collect();
            v.sort_by_key(|(_, n)| std::cmp::Reverse(n.len()));
            for (k, names) in v {
                println!("    {:4} programs  {}   [{}]", names.len(), k, names.iter().take(6).cloned().collect::<Vec<_>>().join(" "));
            }
        }
        if !x.unclean.is_empty() {
            println!("  compiled but not clean:");
            for u in x.unclean.iter().take(40) {
                println!("    {u}");
            }
        }
        println!();
        if record {
            if let Err(e) = put_wasm(&mut lines, module, wasm_object(&version, (ok, total), (x.clean, x.in_scope), &x)) {
                eprintln!("{module}: not recorded: {e}");
                std::process::exit(2);
            }
        }
    }
    if record {
        let mut text = lines.join("\n");
        if ledger_text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(&ledger_path, text).expect("write NIST/progress.json");
        println!("recorded the `wasm` object for {} in NIST/progress.json (nothing else was touched)", modules.join(", "));
    }
    if regressed {
        std::process::exit(1);
    }
}
