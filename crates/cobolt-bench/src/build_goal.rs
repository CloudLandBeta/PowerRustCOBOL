// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T4: `cobolt-bench build-goal`** (R17, R19, R20; the harness of AC6, AC7
//! and AC18).
//!
//! ```sh
//! cargo run -p cobolt-bench --release -- build-goal            # five Builds of each project
//! cargo run -p cobolt-bench --release -- build-goal --runs 1   # one, for a quick local look
//! cargo run -p cobolt-bench --release -- build-goal --record   # also write specs/WASM-BUILD-GOAL.json
//! ```
//!
//! **What it builds:** a copy of each reference project, the five examples of C7 and the
//! ten-times project (T3). Nothing is ever built in place.
//!
//! **How it builds:** with no cache. Every Build gets a fresh build folder, which holds the
//! staging crate and the shared target, and the folder is deleted afterwards. Only Cargo's
//! download cache of crate sources survives, so the network is not timed.
//!
//! **What it reports:** the slowest of the Builds against the 30 s goal, each phase of that
//! Build, the peak memory across them, and the path each Build took. A Build over the goal fails
//! nothing (R17 is a goal, R20 reports only).
//!
//! **The generator** is timed on its own as well, where it accepts the project. It gets the
//! project as the WebAssembly path will give it: every form's program, with its Common Code
//! linked, in one module, with each form's controls and bound events. Where it refuses, the first
//! construct it refuses is named, with the count of the others. `--runs 0` times the generator
//! alone and builds nothing (it cannot be recorded).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::ten_times::{self, Dimensions};

/// The goal of R17.
const GOAL: Duration = Duration::from_secs(30);

/// The product version, read from the one place it is kept.
pub(crate) fn product_version() -> String {
    let text = include_str!("../../cobolt-ide/src/version.rs");
    text.lines()
        .find_map(|l| l.trim().strip_prefix("pub const VERSION: &str = \"")?.strip_suffix("\";").map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("the bench lives in crates/").to_path_buf()
}

/// The platform's name in the record: `<os>-<arch>`, as the CI matrix names it.
pub(crate) fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

struct Machine {
    cpu: String,
    cores: usize,
    physical: Option<usize>,
    memory_gb: f64,
}

impl Machine {
    fn read() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        Machine {
            cpu: sys.cpus().first().map(|c| c.brand().trim().to_owned()).unwrap_or_default(),
            cores: sys.cpus().len(),
            physical: sys.physical_core_count(),
            memory_gb: sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0),
        }
    }

    /// R17's reference box: 4 cores and 16 GB (an SSD is assumed; it cannot be read portably).
    fn is_reference_box(&self) -> bool {
        self.cores == 4 && (15.0..=17.0).contains(&self.memory_gb)
    }
}

struct Args {
    runs: usize,
    record: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args { runs: 5, record: false };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--record" => out.record = true,
            "--runs" => {
                out.runs = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .ok_or("--runs takes a number of Builds, 0 or more")?;
            }
            other => return Err(format!("build-goal: unknown argument {other} (known: --runs N, --record)")),
        }
    }
    if out.record && out.runs == 0 {
        return Err("build-goal: --record needs at least one Build (--runs 1 or more)".into());
    }
    Ok(out)
}

/// One Build of one project.
struct BuildRun {
    wall: Duration,
    phases: Vec<(cobolt_compiler::BuildPhase, Duration)>,
    peak: Option<u64>,
    path: cobolt_compiler::BuildPath,
}

/// The generator on its own: the time over the whole project, or the first refusal and how many
/// more there are.
enum Generator {
    Accepted { forms: usize, took: Duration },
    Refused { at: String, construct: String, more: usize },
}

struct ProjectResult {
    name: String,
    dims: Dimensions,
    builds: Vec<BuildRun>,
    failed: Option<String>,
    generator: Generator,
}

impl ProjectResult {
    fn slowest(&self) -> Option<&BuildRun> {
        self.builds.iter().max_by_key(|b| b.wall)
    }

    fn peak(&self) -> Option<u64> {
        self.builds.iter().filter_map(|b| b.peak).max()
    }
}

/// One form, read and parsed for the generator.
struct GenForm {
    rel: String,
    form: cobolt_forms::Form,
    program: cobolt_ast::program::Program,
    lines: Vec<cobolt_wasm::module::SourceLine>,
}

fn all_controls<'a>(cs: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
    for c in cs {
        out.push(c);
        all_controls(&c.children, out);
    }
}

/// Times the generator on the project in `dir` (`forms` as [`ten_times::regenerate`] gives them),
/// as one module. Only `compile_project` is timed: reading and parsing the forms is the front end.
///
/// Until T16 gives a Desktop project its own form descriptions, they are built here as the Web
/// Build builds them (`form_specs` in `cobolt-project-tools`), without the Web-only lowering.
fn time_generator(dir: &Path, name: &str, forms: &[String]) -> Generator {
    use cobolt_lexer::SourceFormat;
    let refused = |at: &str, construct: String| Generator::Refused { at: at.to_owned(), construct, more: 0 };
    let mut loaded = Vec::new();
    for rel in forms {
        let path = dir.join(rel);
        let form = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| cobolt_forms::xml::load_form_from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(f) => f,
            Err(e) => return refused(rel, format!("the form cannot be read: {e}")),
        };
        let (src, map) = cobolt_codegen::generate_with_map(&form);
        // Copybooks resolve from the generated program's own folder, as the Desktop Build reads it.
        let stem = Path::new(rel).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let generated = dir.join("generated").join(format!("{stem}.cbl"));
        let tokens = match cobolt_lexer::preprocess_program(&src, &generated, SourceFormat::Free) {
            Some(exp) => {
                if let Some(e) = exp.errors.first() {
                    return refused(rel, format!("copybook: {e}"));
                }
                cobolt_lexer::tokenize_expansion(&exp)
            }
            None => cobolt_lexer::tokenize(&src, SourceFormat::Free),
        };
        let Some(mut program) = cobolt_parser::parse(tokens).program else {
            return refused(rel, "the form's program does not parse".into());
        };
        cobolt_compiler::link_common_code(&path, &mut program);
        let lines = (1..=src.lines().count() as u32)
            .filter_map(|g| {
                map.resolve(g).map(|(site, l)| cobolt_wasm::module::SourceLine {
                    gen_line: g,
                    site: site.display_path(&form.name),
                    line: l,
                    column: 0,
                })
            })
            .collect();
        loaded.push(GenForm { rel: rel.clone(), form, program, lines });
    }
    if loaded.is_empty() {
        return refused("", "the project has no form".into());
    }
    let main = loaded.iter().position(|f| f.form.main_form).unwrap_or(0);
    let specs: Vec<cobolt_wasm::module::FormSpec<'_>> = loaded
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut controls = Vec::new();
            all_controls(&f.form.controls, &mut controls);
            let mut bindings: Vec<(String, String)> =
                controls.iter().flat_map(|c| c.events.iter().map(|e| (c.id.clone(), e.event.clone()))).collect();
            bindings.extend(
                f.form.form_events.iter().filter(|e| !matches!(e.event.as_str(), "onLoad" | "onClose")).map(|e| (String::new(), e.event.clone())),
            );
            cobolt_wasm::module::FormSpec {
                name: f.form.name.clone(),
                main: i == main,
                instances: f.form.effective_instances(i == main),
                program: &f.program,
                controls: controls.iter().map(|c| (c.id.clone(), format!("{:?}", c.control_type))).collect(),
                bindings,
                layers: f.form.layers.iter().map(|l| l.name.clone()).collect(),
                lines: f.lines.clone(),
            }
        })
        .collect();
    let opts = cobolt_wasm::Options { name: name.to_owned(), ..cobolt_wasm::Options::default() };
    let t = Instant::now();
    let out = cobolt_wasm::module::compile_project(&specs, &opts);
    let took = t.elapsed();
    match out {
        Ok(_) => Generator::Accepted { forms: specs.len(), took },
        Err(e) => {
            let at = match e.form.and_then(|i| loaded.get(i)) {
                Some(f) => {
                    let stem = Path::new(&f.rel).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let site = f.lines.iter().find(|l| l.gen_line == e.line).map(|l| format!(" ({}, line {})", l.site, l.line));
                    format!("generated/{stem}.cbl:{}{}", e.line, site.unwrap_or_default())
                }
                None => format!("line {}", e.line),
            };
            Generator::Refused { at, construct: e.message, more: e.more.len() }
        }
    }
}

/// Copies, regenerates and builds one project `runs` times, each from nothing.
fn measure_project(name: &str, source: &Path, scratch: &Path, runs: usize) -> Result<ProjectResult, String> {
    let copy = scratch.join(name);
    let _ = std::fs::remove_dir_all(&copy);
    ten_times::copy_project(source, &copy).map_err(|e| format!("copy {name}: {e}"))?;
    let (dims, forms) = ten_times::regenerate(&copy)?;
    let generator = time_generator(&copy, name, &forms);
    let manifest = ten_times::manifest_of(&copy)?;

    let mut builds = Vec::new();
    let mut failed = None;
    for run in 1..=runs {
        let build_dir = scratch.join(format!("build-{name}-{run}"));
        let _ = std::fs::remove_dir_all(&build_dir);
        let opts = cobolt_compiler::BuildOptions {
            verbose: false,
            workspace_root: Some(repo_root()),
            build_dir: Some(build_dir.clone()),
            full: true,
            ..Default::default()
        };
        eprintln!("build-goal: {name}, Build {run} of {runs}…");
        let t = Instant::now();
        let built = cobolt_compiler::build_project(&manifest, &opts);
        let wall = t.elapsed();
        let _ = std::fs::remove_dir_all(&build_dir);
        match built {
            Ok(b) => builds.push(BuildRun { wall, phases: b.phases, peak: b.peak_rss, path: b.path }),
            Err(e) => {
                failed = Some(e.to_string());
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&copy);
    Ok(ProjectResult { name: name.to_owned(), dims, builds, failed, generator })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn tenth(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn phase_ms(b: &BuildRun, phase: cobolt_compiler::BuildPhase) -> Option<f64> {
    b.phases.iter().find(|(p, _)| *p == phase).map(|(_, d)| ms(*d))
}

fn path_name(p: cobolt_compiler::BuildPath) -> &'static str {
    match p {
        cobolt_compiler::BuildPath::Cargo => "cargo",
    }
}

fn record_json(machine: &Machine, args: &Args, results: &[ProjectResult], ten: &(Dimensions, String)) -> serde_json::Value {
    use serde_json::json;
    let projects: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let slowest = r.slowest();
            let phases: serde_json::Map<String, serde_json::Value> = slowest
                .map(|b| b.phases.iter().map(|(p, d)| (p.name().to_owned(), json!(tenth(ms(*d))))).collect())
                .unwrap_or_default();
            let generator = match &r.generator {
                Generator::Accepted { forms, took } => json!({"accepted": true, "forms": forms, "ms": tenth(ms(*took))}),
                Generator::Refused { at, construct, more } => json!({"accepted": false, "at": at, "construct": construct, "more_refusals": more}),
            };
            json!({
                "project": r.name,
                "forms": r.dims.forms, "cobol_lines": r.dims.cobol_lines, "indexed_files": r.dims.indexed,
                "path": slowest.map(|b| path_name(b.path)),
                "builds": r.builds.len(),
                "slowest_ms": slowest.map(|b| tenth(ms(b.wall))),
                "over_goal": slowest.map(|b| b.wall > GOAL),
                "phases_ms": phases,
                "peak_mb": r.peak().map(|p| tenth(p as f64 / 1048576.0)),
                "failed": r.failed,
                "generator": generator,
            })
        })
        .collect();
    json!({
        "version": product_version(),
        "machine": {
            "cpu": machine.cpu, "cores": machine.cores, "physical_cores": machine.physical,
            "memory_gb": tenth(machine.memory_gb),
            "reference_box": machine.is_reference_box(),
        },
        "runs_per_project": args.runs,
        "goal_ms": ms(GOAL),
        "ten_times": { "forms": ten.0.forms, "cobol_lines": ten.0.cobol_lines, "indexed_files": ten.0.indexed, "sha256": ten.1 },
        "projects": projects,
    })
}

/// The `build-goal` mode.
pub fn run(args: &[String]) -> Result<(), String> {
    let args = parse_args(args)?;
    let machine = Machine::read();
    let scratch = std::env::temp_dir().join(format!("prc-build-goal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let started = Instant::now();

    // The ten-times project, generated from the five as they are in this checkout.
    let refs = ten_times::measure_references(&scratch.join("refs"))?;
    let goal = ten_times::target(&refs.iter().map(|(_, d)| *d).collect::<Vec<_>>());
    let ten_dir = scratch.join("generated").join(ten_times::PROJECT_NAME);
    let made = ten_times::generate(&goal, &ten_dir)?;
    let ten = (made, ten_times::tree_hash(&ten_dir)?);

    let mut results = Vec::new();
    for name in ten_times::REFERENCE_PROJECTS {
        results.push(measure_project(name, &ten_times::examples_dir().join(name), &scratch.join("work"), args.runs)?);
    }
    results.push(measure_project(ten_times::PROJECT_NAME, &ten_dir, &scratch.join("work"), args.runs)?);
    let _ = std::fs::remove_dir_all(&scratch);

    // The result block (GOLDEN RULE #7).
    println!("== cobolt-bench build-goal (DESKTOP-WASM-001 T4: R17, R19, R20) ==");
    println!(
        "machine: {} — {}, {} cores{}, {:.1} GB — {}",
        platform(),
        machine.cpu,
        machine.cores,
        machine.physical.map(|p| format!(" ({p} physical)")).unwrap_or_default(),
        machine.memory_gb,
        if machine.is_reference_box() { "a reference box (4 cores, 16 GB)" } else { "not a reference box (4 cores, SSD, 16 GB)" }
    );
    println!("version {}, {} Build(s) per project, no cache (a fresh build folder each), goal {} s", product_version(), args.runs, GOAL.as_secs());
    println!("ten-times project: forms {}, COBOL lines {}, indexed files {}; sha256 {}", ten.0.forms, ten.0.cobol_lines, ten.0.indexed, ten.1);
    println!(
        "{:<16} {:>5} {:>6} {:>7} {:>5} {:>9} {:>6} {:>9} {:>9} {:>8} {:>8} {:>8}  generator",
        "project", "forms", "lines", "indexed", "path", "slowest s", "goal", "regen ms", "front ms", "cargo s", "inst ms", "peak MB"
    );
    use cobolt_compiler::BuildPhase as P;
    let mut over = 0;
    let mut built = 0;
    for r in &results {
        let s = r.slowest();
        let num = |v: Option<f64>, scale: f64| v.map_or("—".to_string(), |v| format!("{:.1}", v / scale));
        let over_goal = s.map(|b| b.wall > GOAL);
        over += usize::from(over_goal == Some(true));
        built += r.builds.len();
        let generator = match &r.generator {
            Generator::Accepted { forms, took } => format!("accepted, {forms} forms in one module in {:.1} ms", ms(*took)),
            Generator::Refused { at, construct, more } => format!("refused at {at}: {construct} (and {more} more)"),
        };
        println!(
            "{:<16} {:>5} {:>6} {:>7} {:>5} {:>9} {:>6} {:>9} {:>9} {:>8} {:>8} {:>8}  {generator}",
            r.name,
            r.dims.forms,
            r.dims.cobol_lines,
            r.dims.indexed,
            s.map_or("—", |b| path_name(b.path)),
            num(s.map(|b| ms(b.wall)), 1000.0),
            match over_goal {
                Some(true) => "over",
                Some(false) => "within",
                None => "—",
            },
            num(s.and_then(|b| phase_ms(b, P::Regenerate)), 1.0),
            num(s.and_then(|b| phase_ms(b, P::FrontEnd)), 1.0),
            num(s.and_then(|b| phase_ms(b, P::Cargo)), 1000.0),
            num(s.and_then(|b| phase_ms(b, P::Install)), 1.0),
            num(r.peak().map(|p| p as f64 / 1048576.0), 1.0),
        );
        if let Some(e) = &r.failed {
            println!("  {} failed: {e}", r.name);
        }
    }
    println!(
        "Builds: {built} of {} succeeded; projects over the goal: {over} of {}; total {:.1} min",
        results.len() * args.runs,
        results.len(),
        started.elapsed().as_secs_f64() / 60.0
    );

    if args.record {
        let file = repo_root().join("specs/WASM-BUILD-GOAL.json");
        let mut all: serde_json::Map<String, serde_json::Value> =
            std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        all.insert(platform(), record_json(&machine, &args, &results, &ten));
        let text = serde_json::to_string_pretty(&serde_json::Value::Object(all)).map_err(|e| e.to_string())? + "\n";
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
        println!("recorded: {}", file.display());
    }
    Ok(())
}
