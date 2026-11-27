// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T25 (R6, R8, R20; AC3 on the four shipped Backends, AC5 per call): the four Backends the repository ships,
//! served by the interpreter and by the module, answer alike.
//!
//! Each Backend (`PowerOficina`, `PowerAnalyticsWeb`, `PowerWebHello`, `PowerMobileHello`) is copied twice into a scratch
//! folder, files and database included, the copies are loaded exactly as the server loads them, and the same calls are made
//! to the interpreter's engine on the first copy and to the Backend runner's engine on the second. The calls are made from
//! each program's own interface: for every exported program, five requests built from the declared layout of its
//! parameters, an ordinary one, another ordinary one with other values, an empty one, one at every `PIC` and `OCCURS`
//! limit, and one past a limit. What a caller learns must be equal (the JSON of every parameter and of `RETURNING`, and for
//! a call that fails the class, the code and the member it names), and so must what the calls leave behind: the files of
//! the Backend's data folder and its database, byte for byte, after the same sequence of calls on each copy.
//!
//! Then, per Backend, the parallel case (AC5): the same read-only call from many threads at once on the module's engine,
//! each answered as the one made alone was.
//!
//! The clock and the random seed are pinned (`PRC_TEST_CLOCK`, `PRC_TEST_SEED`) so that a program that stamps a record or
//! draws a number gives the same value on both paths.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::backend::{Backend, ExportedProgram};
use cobolt_server::engine::{Engine, ExecOptions, ExecResult};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::standalone::Loaded;
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use cobolt_wire::contract::Direction;
use cobolt_wire::error::CallError;
use cobolt_wire::layout::{Item, Kind};

/// The Backends the repository ships (spec 114 §0.1).
const SHIPPED: [&str; 4] = ["PowerOficina", "PowerAnalyticsWeb", "PowerWebHello", "PowerMobileHello"];
/// Per Backend, a program that only reads, for the parallel case: the health check every Backend has and, where there is one,
/// a program that reads the Backend's data.
const READERS: [(&str, &[&str]); 4] = [
    ("PowerOficina", &["HEALTH", "CATALOG-LIST"]),
    ("PowerAnalyticsWeb", &["HEALTH", "FUND-TOTALS"]),
    ("PowerWebHello", &["HEALTH"]),
    ("PowerMobileHello", &["HEALTH"]),
];
/// Requests written for the applications, with values their programs accept, so that the paths that save and answer with data
/// are walked and not only the ones that refuse a stranger's values: `(project, program, one JSON text per parameter)`. They run
/// after the generated ones, in this order, on each engine.
const DOMAIN: &[(&str, &str, &[&str])] = &[
    ("PowerOficina", "CATALOG-LIST", &[r#"{"CLQ-CURRENCY":"BRL","CLQ-KIND":"P"}"#]),
    ("PowerOficina", "CATALOG-LIST", &[r#"{"CLQ-CURRENCY":"BRL","CLQ-KIND":"S"}"#]),
    ("PowerOficina", "CATALOG-LIST", &[r#"{"CLQ-CURRENCY":"EUR","CLQ-KIND":"P"}"#]),
    (
        "PowerOficina",
        "CUSTOMER-SAVE",
        &[
            r#"{"CRQ-ID":0,"CRQ-NAME":"Maria Souza","CRQ-DOC":"52998224725","CRQ-ADDRESS":"Rua das Flores 10","CRQ-PHONE":"11987654321","CRQ-WHATSAPP":"11987654321","CRQ-EMAIL":"maria@example.com","CRQ-CONSENT-EMAIL":"S","CRQ-CONSENT-WA":"S","CRQ-OPERATOR":"ana"}"#,
        ],
    ),
    (
        "PowerOficina",
        "CUSTOMER-SAVE",
        &[
            r#"{"CRQ-ID":0,"CRQ-NAME":"Joao Lima","CRQ-DOC":"11144477735","CRQ-ADDRESS":"","CRQ-PHONE":"","CRQ-WHATSAPP":"","CRQ-EMAIL":"","CRQ-CONSENT-EMAIL":"N","CRQ-CONSENT-WA":"N","CRQ-OPERATOR":"ana"}"#,
        ],
    ),
    (
        "PowerOficina",
        "VEHICLE-SAVE",
        &[
            r#"{"VRQ-PLATE":"ABC1D23","VRQ-CUS-ID":1,"VRQ-CHASSIS":"9BWZZZ377VT004251","VRQ-BRAND":"VW","VRQ-MODEL":"Gol","VRQ-YEAR":2019,"VRQ-COLOR":"Prata","VRQ-MILEAGE":45000,"VRQ-OPERATOR":"ana"}"#,
        ],
    ),
];
/// Calls made together on the module's engine in the parallel case.
const PARALLEL: usize = 8;

// ── the requests, from the declared layouts ─────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// Ordinary values.
    Typical,
    /// Other ordinary values.
    Other,
    /// Nothing given: `{}`, `""`, `0`, `[]`.
    Empty,
    /// Every `PIC` and `OCCURS` at its limit.
    AtLimit,
    /// The first elementary member one past its limit.
    OverLimit,
    /// A request written for the application, with values it accepts (`DOMAIN`).
    Domain,
}

const SHAPES: [Shape; 5] = [Shape::Typical, Shape::Other, Shape::Empty, Shape::AtLimit, Shape::OverLimit];

fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("a string is JSON")
}

/// The text of one elementary value of `kind`.
fn leaf(kind: &Kind, shape: Shape, excess: &mut bool) -> String {
    let over = shape == Shape::OverLimit && std::mem::take(excess);
    let fill = |len: usize, c: char| std::iter::repeat(c).take(len).collect::<String>();
    match kind {
        Kind::Alphanumeric { len, .. } | Kind::National { len } | Kind::Utf8 { len, .. } | Kind::Edited { len } => {
            let len = *len;
            json_string(&match shape {
                Shape::Empty => String::new(),
                Shape::Typical => "ALPHA".chars().cycle().take(len.min(5)).collect(),
                Shape::Other => "beta 2".chars().cycle().take(len.min(6)).collect(),
                Shape::AtLimit => fill(len, 'Z'),
                Shape::OverLimit => fill(if over { len + 1 } else { len }, 'Z'),
                Shape::Domain => unreachable!("a domain request is written, not generated"),
            })
        }
        Kind::Numeric { digits, scale, signed } => {
            let (digits, scale) = (*digits as usize, *scale as usize);
            let whole = digits.saturating_sub(scale);
            let frac = |pattern: &str| pattern.chars().chain(std::iter::repeat('0')).take(scale).collect::<String>();
            let number = |whole_text: String, frac_text: String| if scale == 0 { whole_text } else { format!("{whole_text}.{frac_text}") };
            let sign = |negative: bool| if *signed && negative { "-" } else { "" };
            match shape {
                Shape::Empty => "0".into(),
                Shape::Typical => number(if whole == 0 { "0".into() } else { "7".into() }, frac("25")),
                Shape::Other => format!("{}{}", sign(true), number(if whole == 0 { "0".into() } else { "3".into() }, frac("125"))),
                Shape::AtLimit => format!("{}{}", sign(true), number(if whole == 0 { "0".into() } else { fill(whole, '9') }, fill(scale, '9'))),
                Shape::OverLimit => number(fill(if over { whole + 1 } else { whole.max(1) }, '9'), fill(scale, '9')),
                Shape::Domain => unreachable!("a domain request is written, not generated"),
            }
        }
        Kind::Group { .. } => unreachable!("a group is not a leaf"),
    }
}

/// The JSON text of the item itself, one occurrence of it when it is a table.
fn one(item: &Item, shape: Shape, excess: &mut bool) -> String {
    match &item.kind {
        Kind::Group { .. } => {
            if shape == Shape::Empty {
                return "{}".into();
            }
            let members: Vec<&Item> = item.members().collect();
            let mut texts: Vec<(String, String)> = members.iter().map(|m| (m.name.clone(), member(m, shape, excess))).collect();
            // The count a table `DEPENDING ON` is the table's length, not a value of its own.
            for m in &members {
                if let Some(depending) = m.occurs.as_ref().and_then(|o| o.depending_on.as_deref()) {
                    let len = texts.iter().find(|(n, _)| n == &m.name).map(|(_, t)| array_len(t)).unwrap_or(0);
                    if let Some(count) = texts.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(depending)) {
                        count.1 = len.to_string();
                    }
                }
            }
            let body: Vec<String> = texts.iter().map(|(n, t)| format!("{}:{t}", json_string(n))).collect();
            format!("{{{}}}", body.join(","))
        }
        kind => leaf(kind, shape, excess),
    }
}

/// How many elements the array text has: the elements are separated at depth 1 only.
fn array_len(text: &str) -> usize {
    let (mut depth, mut count, mut in_string, mut escaped, mut any) = (0i32, 0usize, false, false, false);
    for c in text.chars() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                any = true;
            }
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            ',' if depth == 1 => count += 1,
            c if !c.is_whitespace() && depth >= 1 => any = true,
            _ => {}
        }
    }
    if any {
        count + 1
    } else {
        0
    }
}

/// A member: the item, or the array of its occurrences when it is a table.
fn member(item: &Item, shape: Shape, excess: &mut bool) -> String {
    let Some(occurs) = &item.occurs else { return one(item, shape, excess) };
    let n = match shape {
        Shape::Empty => 0,
        Shape::Typical | Shape::Other => 2usize.clamp(occurs.min, occurs.max.max(occurs.min)),
        Shape::AtLimit | Shape::OverLimit => occurs.max,
        Shape::Domain => unreachable!("a domain request is written, not generated"),
    };
    let elements: Vec<String> = (0..n).map(|_| one(item, shape, excess)).collect();
    format!("[{}]", elements.join(","))
}

/// The request of `prog` in `shape`: one JSON text per parameter.
fn request(prog: &ExportedProgram, shape: Shape) -> Vec<String> {
    let mut excess = true;
    prog.sig
        .params
        .iter()
        .map(|p| if p.direction == Direction::Out && shape != Shape::AtLimit { "{}".to_string() } else { member(&p.layout, shape, &mut excess) })
        .collect()
}

// ── what a caller learns ───────────────────────────────────────────────────────────────────────────────────────────────────

/// A finished call as a caller sees it. The words of a failure are not compared: after the member's path the interpreter and
/// the module word a refusal differently (recorded in `specs/WASM-PARITY.md`); the class, the code and the member are.
#[derive(Debug, PartialEq)]
enum Seen {
    Answered { args: Vec<String>, returning: Option<String> },
    Failed { class: String, code: u32, member: Option<String> },
}

fn seen(result: Result<ExecResult, CallError>) -> Seen {
    match result {
        Ok(r) => Seen::Answered { args: r.args, returning: r.returning },
        Err(e) => Seen::Failed { class: format!("{:?}", e.class), code: e.code, member: e.detail },
    }
}

fn options() -> ExecOptions {
    ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_millis(500))
}

fn call(engine: &dyn Engine, prog: &ExportedProgram, args: &[String]) -> Seen {
    seen(engine.execute(prog, args, options()))
}

// ── the copies the engines run in ──────────────────────────────────────────────────────────────────────────────────────────

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn copy_tree(from: &Path, to: &Path, keep: &dyn Fn(&Path) -> bool) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        if !keep(&path) {
            continue;
        }
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target, keep);
        } else {
            std::fs::copy(&path, &target).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
}

/// A copy of the project's Backend, its data folder, its copybooks and its project file, without the project's own knowledge bases and the
/// lock files of the original (the engines make their own).
fn copy_project(name: &str, scratch: &Path, tag: &str) -> PathBuf {
    let from = examples().join(name);
    let to = scratch.join(tag).join(name);
    let keep = |p: &Path| {
        let file = p.file_name().and_then(|f| f.to_str()).unwrap_or_default();
        !(file.ends_with(".lck") || file.ends_with(".redb") || file.ends_with("-chunked.data") || file == "grace-conversation.json" || file == ".DS_Store")
    };
    for sub in ["backend", "data", "COPYBOOKS"] {
        if from.join(sub).is_dir() {
            copy_tree(&from.join(sub), &to.join(sub), &keep);
        }
    }
    std::fs::copy(from.join(format!("{name}.project.toml")), to.join(format!("{name}.project.toml"))).unwrap();
    to
}

/// What the Backend's programs reach by a relative path or a connection name: this copy's folder, and the project's SQL
/// connections read from this copy's project file (`CONNECT TO 'NAME'`).
fn enter(project: &Path, name: &str) {
    cobolt_forms::assets::set_base(project);
    let manifest = project.join(format!("{name}.project.toml"));
    if let Ok(catalog) = cobolt_runtime::esql::catalog::SqlCatalog::from_project(&manifest, name) {
        cobolt_runtime::esql::catalog::install(catalog);
    }
}

/// Every file under `dir` (relative path, bytes).
fn files_under(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if !p.to_string_lossy().ends_with(".lck") {
                out.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

// ── the module ────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// The Backend's programs as one module and the Backend the runner serves from it, as a Build puts them in a copy of the
/// runner and the runner reads them back. Returns the module's size and the time to compile it.
fn module_backend(name: &str, source: &Backend) -> (Result<Loaded, Vec<String>>, usize, u128) {
    let mut programs: Vec<_> = source.programs().collect();
    programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));
    let list: Vec<BackendProgram<'_>> =
        programs.iter().map(|p| BackendProgram { name: p.sig.name.clone(), program: p.program.as_ref().expect("the sources of the Backend") }).collect();
    let t = Instant::now();
    let wasm =
        compile_backend(&list, &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("the generator refused the Backend: {e}")).wasm;
    let compile_ms = t.elapsed().as_millis();
    let size = wasm.len();
    let container = BackendContainer {
        state: "per-call".into(),
        interface: source.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&source.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: wasm,
        name: name.to_string(),
    };
    (payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()), size, compile_ms)
}

// ── the run ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Tally {
    backends: usize,
    exports: usize,
    calls: usize,
    equal: usize,
    different: usize,
    answered: usize,
    refused: usize,
    files: usize,
    files_equal: usize,
    parallel_calls: usize,
    parallel_equal: usize,
    changed: usize,
    responding: usize,
    refused_by_variant: usize,
}

#[test]
fn the_four_shipped_backends_answer_and_leave_behind_the_same_on_the_module() {
    std::env::set_var("PRC_TEST_CLOCK", "2026-01-15T10:00:00-03:00");
    std::env::set_var("PRC_TEST_SEED", "20260115");
    let scratch = std::env::temp_dir().join(format!("prc-backend-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let started = Instant::now();
    let mut tally = Tally::default();
    let mut differences: Vec<String> = Vec::new();

    for name in SHIPPED {
        let copy_ms = {
            let t = Instant::now();
            copy_project(name, &scratch, "interp");
            copy_project(name, &scratch, "module");
            t.elapsed().as_millis()
        };
        let interp_dir = scratch.join("interp").join(name);
        let module_dir = scratch.join("module").join(name);

        enter(&interp_dir, name);
        let t = Instant::now();
        let interp = Backend::load(&interp_dir.join("backend")).unwrap_or_else(|e| panic!("{name} does not load: {e:#?}"));
        let load_ms = t.elapsed().as_millis();
        enter(&module_dir, name);
        let source = Backend::load(&module_dir.join("backend")).unwrap_or_else(|e| panic!("{name} does not load: {e:#?}"));
        let (loaded, module_bytes, compile_ms) = module_backend(name, &source);
        let loaded = match loaded {
            Ok(l) => l,
            // A variant of the runner without the SQL group refuses, by name, a Backend that declares it (T17).
            Err(said) if !cfg!(feature = "sql") && said.join(" ").contains("sql") => {
                println!("{name}: refused by name by this variant of the runner, which has no `sql` group: {}", said.join(" "));
                tally.refused_by_variant += 1;
                continue;
            }
            Err(said) => panic!("the runner refuses {name}: {said:?}"),
        };
        let wasm_engine = loaded.engine.clone().expect("the runner's engine");
        assert_eq!(wasm_engine.name(), "wasm");
        let on_module = loaded.backend;

        let mut names: Vec<String> = interp.programs().map(|p| p.sig.name.clone()).collect();
        names.sort();
        assert_eq!(
            names,
            {
                let mut n: Vec<String> = on_module.programs().map(|p| p.sig.name.clone()).collect();
                n.sort();
                n
            },
            "{name}: the runner's Backend exports the programs the sources do"
        );

        // The same calls, in the same order, to each engine in its own copy.
        let plan: Vec<(String, Shape, Vec<String>)> = names
            .iter()
            .flat_map(|n| SHAPES.iter().map(move |s| (n.clone(), *s)))
            .map(|(n, s)| {
                let args = request(interp.program(&n).unwrap(), s);
                (n, s, args)
            })
            .chain(
                DOMAIN
                    .iter()
                    .filter(|(p, n, _)| *p == name && interp.program(n).is_some())
                    .map(|(_, n, args)| (n.to_string(), Shape::Domain, args.iter().map(|a| a.to_string()).collect())),
            )
            .collect();
        let before = files_under(&interp_dir.join("data"));
        enter(&interp_dir, name);
        let t = Instant::now();
        let by_interpreter: Vec<Seen> = plan.iter().map(|(n, _, args)| call(&InterpreterEngine, interp.program(n).unwrap(), args)).collect();
        let interp_ms = t.elapsed().as_millis();
        enter(&module_dir, name);
        let t = Instant::now();
        let by_module: Vec<Seen> = plan.iter().map(|(n, _, args)| call(wasm_engine.as_ref(), on_module.program(n).unwrap(), args)).collect();
        let module_ms = t.elapsed().as_millis();

        let (mut equal, mut answered, mut refused) = (0usize, 0usize, 0usize);
        let show = std::env::var_os("PRC_PARITY_SHOW").is_some();
        for (((n, shape, args), a), b) in plan.iter().zip(&by_interpreter).zip(&by_module) {
            if show {
                println!("  {name} {n} {shape:?}: {}", format!("{a:?}").chars().take(220).collect::<String>());
            }
            if matches!(a, Seen::Answered { .. }) {
                answered += 1;
            } else {
                refused += 1;
            }
            if a == b {
                equal += 1;
            } else {
                differences.push(format!("{name} {n} ({shape:?}) request {args:?}:\n   interpreter {a:?}\n   module      {b:?}"));
            }
        }

        // Evidence that the calls did something: how many exports answer differently to different requests, and how many files the
        // calls made or changed.
        let responding = names
            .iter()
            .filter(|n| {
                let answers: Vec<&Seen> = plan.iter().zip(&by_interpreter).filter(|((p, _, _), _)| p == *n).map(|(_, a)| a).collect();
                answers.iter().any(|a| *a != answers[0])
            })
            .count();

        // What the calls left behind: the data folder, file by file.
        let (left_a, left_b) = (files_under(&interp_dir.join("data")), files_under(&module_dir.join("data")));
        let names_a: Vec<&String> = left_a.iter().map(|(n, _)| n).collect();
        let names_b: Vec<&String> = left_b.iter().map(|(n, _)| n).collect();
        let changed = left_a.iter().filter(|(n, b)| before.iter().find(|(m, _)| m == n).map(|(_, old)| old != b).unwrap_or(true)).count();
        let mut files_equal = 0usize;
        if names_a != names_b {
            differences.push(format!("{name}: the data folders hold different files: {names_a:?} against {names_b:?}"));
        }
        for ((fa, ba), (fb, bb)) in left_a.iter().zip(&left_b) {
            if fa == fb && ba == bb {
                files_equal += 1;
            } else {
                differences.push(format!("{name}: data/{fa} differs after the same calls ({} bytes against {} bytes)", ba.len(), bb.len()));
            }
        }

        // AC5: the same read-only call from many threads at once is answered as the one made alone.
        enter(&module_dir, name);
        let (mut parallel_calls, mut parallel_equal) = (0usize, 0usize);
        for reader in READERS.iter().find(|(n, _)| *n == name).map(|(_, r)| *r).unwrap_or_default() {
            let Some(prog) = on_module.program(reader) else { continue };
            let args = request(prog, Shape::Typical);
            let alone = call(wasm_engine.as_ref(), prog, &args);
            let together: Vec<Seen> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..PARALLEL).map(|_| s.spawn(|| call(wasm_engine.as_ref(), prog, &args))).collect();
                handles.into_iter().map(|h| h.join().expect("a call thread")).collect()
            });
            for each in together {
                parallel_calls += 1;
                if each == alone {
                    parallel_equal += 1;
                } else {
                    differences.push(format!("{name} {reader}: a call among {PARALLEL} at once answered {each:?}, alone {alone:?}"));
                }
            }
        }

        println!(
            "{name}: {} exports, {} calls ({answered} answered, {refused} refused or failed): {equal} equal; {responding} of the exports answer differently to different requests; the data folder {} files ({changed} made or changed by the calls), {files_equal} equal; module {module_bytes} bytes compiled in {compile_ms} ms; \
             load {load_ms} ms, copies {copy_ms} ms; interpreter {interp_ms} ms ({:.0} calls/sec), module {module_ms} ms ({:.0} calls/sec); {parallel_calls} parallel calls, {parallel_equal} equal",
            names.len(),
            plan.len(),
            left_a.len(),
            plan.len() as f64 * 1000.0 / interp_ms.max(1) as f64,
            plan.len() as f64 * 1000.0 / module_ms.max(1) as f64,
        );
        tally.backends += 1;
        tally.exports += names.len();
        tally.calls += plan.len();
        tally.equal += equal;
        tally.different += plan.len() - equal;
        tally.answered += answered;
        tally.refused += refused;
        tally.files += left_a.len();
        tally.files_equal += files_equal;
        tally.changed += changed;
        tally.responding += responding;
        tally.parallel_calls += parallel_calls;
        tally.parallel_equal += parallel_equal;
    }

    println!(
        "== spec 114 T25: {} Backends, {} exports, {} calls ({} answered, {} refused or failed) on the interpreter and on the module: {} equal, {} different; {} of the exports answer differently to different requests; {} files left behind ({} made or changed by the calls), {} equal; {} parallel calls ({PARALLEL} at once), {} equal; {} ms; {} more refused by name for want of the `sql` group ==",
        tally.backends,
        tally.exports,
        tally.calls,
        tally.answered,
        tally.refused,
        tally.equal,
        tally.different,
        tally.responding,
        tally.files,
        tally.changed,
        tally.files_equal,
        tally.parallel_calls,
        tally.parallel_equal,
        started.elapsed().as_millis(),
        tally.refused_by_variant
    );
    let failures = tally.different + (tally.files - tally.files_equal) + (tally.parallel_calls - tally.parallel_equal);
    println!("PASS {:03} FAIL {failures:03}", tally.equal + tally.files_equal + tally.parallel_equal);
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(differences.is_empty(), "{} difference(s):\n{}", differences.len(), differences.join("\n"));
}

// ── T45: the measurements ─────────────────────────────────────────────────────

/// The calls per second of `callers` threads that each make `each` calls of `prog` with `args`, after three that are not counted
/// (the first call of an engine compiles, opens and warms). Returns the milliseconds the counted calls took and how many failed.
fn rate(engine: &dyn Engine, prog: &ExportedProgram, args: &[String], callers: usize, each: usize) -> (u128, usize) {
    for _ in 0..3 {
        let _ = engine.execute(prog, args, options());
    }
    let began = Instant::now();
    let failed: usize = std::thread::scope(|s| {
        let handles: Vec<_> = (0..callers).map(|_| s.spawn(|| (0..each).filter(|_| engine.execute(prog, args, options()).is_err()).count())).collect();
        handles.into_iter().map(|h| h.join().expect("a caller thread")).sum()
    });
    (began.elapsed().as_millis().max(1), failed)
}

/// Spec 114 T45 (R16, AC10): the Build's time per phase against the 30 s goal, and the throughput of each shipped Backend's
/// commonest call (the program that reads its data) on both engines, with one caller and with eight. It is a measurement: it never
/// fails on a number, only when something could not be measured. Run it on request, in release for numbers that mean something:
/// `cargo test -p cobolt-backend-runner --release --test parity -- --ignored --nocapture measured_build_and_throughput`
#[test]
#[ignore = "a measurement, run on request"]
fn measured_build_and_throughput_on_both_engines() {
    use cobolt_compiler::backend_build::build_backend;
    use cobolt_compiler::backend_module::BackendPath;
    std::env::set_var("PRC_TEST_CLOCK", "2026-01-15T10:00:00-03:00");
    std::env::set_var("PRC_TEST_SEED", "20260115");
    let _one = common::one_build_at_a_time();
    const GOAL_MS: u128 = 30_000;
    const ONE: usize = 200;
    const EACH: usize = 50;
    let scratch = std::env::temp_dir().join(format!("prc-backend-measure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let runner = common::runner_with_sbom(&scratch.join("runner"));
    let profile = if cfg!(debug_assertions) { "debug profile: the runner is not the shipped one, the numbers are not a release's" } else { "release profile" };
    let started = Instant::now();
    let (mut measured, mut failed) = (0u32, 0u32);
    let mut rows: Vec<String> = Vec::new();

    for name in SHIPPED {
        // The Build, phase by phase.
        let dir = copy_project(name, &scratch, "build");
        let began = Instant::now();
        let build = match build_backend(&dir, &common::options(&runner, Some(BackendPath::Wasm))) {
            Ok(b) => {
                measured += 1;
                let ms = began.elapsed().as_millis();
                let phases = b.phases.iter().map(|p| format!("{} {} ms", p.name, p.millis)).collect::<Vec<_>>().join(", ");
                format!("Build {ms} ms ({}): {phases}; the file is {} bytes", if ms <= GOAL_MS { "within the 30 s goal" } else { "OVER the 30 s goal" }, std::fs::metadata(&b.binary).map(|m| m.len()).unwrap_or(0))
            }
            Err(e) => {
                failed += 1;
                format!("Build FAILED: {e}")
            }
        };

        // The call, on each engine in its own copy.
        let interp_dir = copy_project(name, &scratch, "interp");
        let module_dir = copy_project(name, &scratch, "module");
        enter(&interp_dir, name);
        let interp = Backend::load(&interp_dir.join("backend")).unwrap_or_else(|e| panic!("{name} does not load: {e:#?}"));
        enter(&module_dir, name);
        let source = Backend::load(&module_dir.join("backend")).unwrap_or_else(|e| panic!("{name} does not load: {e:#?}"));
        let (loaded, module_bytes, compile_ms) = module_backend(name, &source);
        let loaded = loaded.unwrap_or_else(|e| panic!("the runner refuses {name}: {e:?}"));
        let wasm_engine = loaded.engine.clone().expect("the runner's engine");
        let on_module = loaded.backend;
        let reader = READERS.iter().find(|(n, _)| *n == name).and_then(|(_, r)| r.last().copied()).expect("a reader");
        let prog_i = interp.program(reader).expect("the reader exists");
        // A request the interpreter answers: one written for the application when there is one, else the ordinary one.
        enter(&interp_dir, name);
        let mut candidates: Vec<Vec<String>> =
            DOMAIN.iter().filter(|(p, n, _)| *p == name && *n == reader).map(|(_, _, a)| a.iter().map(|s| s.to_string()).collect()).collect();
        candidates.push(request(prog_i, Shape::Typical));
        let Some(args) = candidates.into_iter().find(|a| matches!(call(&InterpreterEngine, prog_i, a), Seen::Answered { .. })) else {
            failed += 1;
            rows.push(format!("{name}: no request of {reader} is answered by the interpreter; {build}"));
            continue;
        };
        let prog_m = on_module.program(reader).expect("the reader exists on the module");
        let (ia, ib) = (rate(&InterpreterEngine, prog_i, &args, 1, ONE), rate(&InterpreterEngine, prog_i, &args, PARALLEL, EACH));
        enter(&module_dir, name);
        let (ma, mb) = (rate(wasm_engine.as_ref(), prog_m, &args, 1, ONE), rate(wasm_engine.as_ref(), prog_m, &args, PARALLEL, EACH));
        let per_sec = |calls: usize, ms: u128| calls as f64 * 1000.0 / ms as f64;
        let errors = ia.1 + ib.1 + ma.1 + mb.1;
        if errors == 0 {
            measured += 1;
        } else {
            failed += 1;
        }
        rows.push(format!(
            "{name} {reader}: {build}; one caller ({ONE} calls) interpreter {:.0} calls/sec, module {:.0}; {PARALLEL} callers ({} calls) interpreter {:.0}, module {:.0}; the module is {module_bytes} bytes, compiled in {compile_ms} ms{}",
            per_sec(ONE, ia.0),
            per_sec(ONE, ma.0),
            PARALLEL * EACH,
            per_sec(PARALLEL * EACH, ib.0),
            per_sec(PARALLEL * EACH, mb.0),
            if errors == 0 { String::new() } else { format!("; {errors} calls FAILED") }
        ));
    }
    println!("== spec 114 T45, {profile}, {} cores:", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0));
    for r in &rows {
        println!("  {r}");
    }
    println!("measured, not gated: {measured} measurements in {} ms; PASS {measured:03} FAIL {failed:03}", started.elapsed().as_millis());
    let _ = std::fs::remove_dir_all(&scratch);
    assert_eq!(failed, 0, "{failed} measurements could not be made");
}
