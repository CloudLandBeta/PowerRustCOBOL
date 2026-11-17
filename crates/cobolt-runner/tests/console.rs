// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T15 (R4 runner half, R5, R6, R10, R16, R28 runner half; AC5 runner half): a
//! console program, compiled by the generator as a desktop module and packaged with its machine
//! code into a copy of the built `prc-runner` (T13, T14), runs as `rcrun run` runs the same source:
//! the same standard output, byte for byte, and the same exit code, under the test pins. A module
//! that needs a capability the runner lacks is refused, naming it, in the system's language.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;
use cobolt_runner::messages::{text, Lang, Msg};
use cobolt_runner_engine::Target;

/// The console program: the console, the environment, the arguments, the clock, `RANDOM` and
/// `RETURN-CODE`, which are what a desktop module's `prn` imports and the pins carry.
const PROGRAM: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T15CONSOLE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NAME     PIC X(20).
01 WS-QTY      PIC 9(4).
01 WS-HOME     PIC X(30).
01 WS-NONE     PIC X(6) VALUE \"KEEP\".
01 WS-COUNT    PIC 99.
01 WS-LINE     PIC X(40).
01 WS-ARG      PIC X(12).
01 WS-DATE     PIC 9(6).
01 WS-TIME     PIC 9(8).
01 WS-DAY      PIC 9(5).
01 WS-DOW      PIC 9.
01 WS-STAMP    PIC X(21).
01 WS-R        PIC 9(6).
01 WS-I        PIC 9.
PROCEDURE DIVISION.
    DISPLAY \"T15 console fixture\"
    ACCEPT WS-NAME
    ACCEPT WS-QTY
    DISPLAY \"name [\" WS-NAME \"] qty \" WS-QTY
    ACCEPT WS-HOME FROM ENVIRONMENT \"PRC_T15_HOME\"
    DISPLAY \"home [\" WS-HOME \"]\"
    ACCEPT WS-NONE FROM ENVIRONMENT \"PRC_T15_NOT_SET\"
    DISPLAY \"none [\" WS-NONE \"]\"
    ACCEPT WS-COUNT FROM ARGUMENT-NUMBER
    ACCEPT WS-LINE FROM COMMAND-LINE
    ACCEPT WS-ARG FROM ARGUMENT-VALUE
    DISPLAY \"args \" WS-COUNT \" [\" WS-LINE \"] first [\" WS-ARG \"]\"
    ACCEPT WS-DATE FROM DATE
    ACCEPT WS-TIME FROM TIME
    ACCEPT WS-DAY FROM DAY
    ACCEPT WS-DOW FROM DAY-OF-WEEK
    DISPLAY \"date \" WS-DATE \" time \" WS-TIME \" day \" WS-DAY \" dow \" WS-DOW
    MOVE FUNCTION CURRENT-DATE TO WS-STAMP
    DISPLAY \"stamp \" WS-STAMP
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
        COMPUTE WS-R = FUNCTION RANDOM * 1000000
        DISPLAY \"random \" WS-I \" \" WS-R
    END-PERFORM
    MOVE 7 TO RETURN-CODE
    STOP RUN.
";

/// The console's lines, the arguments and the variables both runs get.
const STDIN: &str = "Ada Lovelace\r\n42\n";
const ARGS: &[&str] = &["alpha", "beta gamma"];
const VARS: &[(&str, &str)] = &[("PRC_T15_HOME", "/home/ada")];
/// The pins of the pixel-parity harness's Cargo-path test (`cobolt-compiler`), and a second clock
/// in another zone: `CURRENT-DATE` ends with the zone's offset on both paths (T24, WASM-PARITY #53).
const CLOCK: &str = "2026-01-02T03:04:05.67-03:00";
const CLOCK_EAST: &str = "2026-07-01T23:59:59.99+05:30";
/// Each run's clock and seed.
const RUNS: &[(&str, u64)] = &[(CLOCK, 20_261_114), (CLOCK, 7), (CLOCK_EAST, 7)];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t15-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("PRC_RUNNER_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// The program as the Build will compile it for the runner (T16): a desktop module.
fn desktop_module(src: &str) -> Vec<u8> {
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    cobolt_wasm::compile_program(&parse(src), &opts).unwrap_or_else(|e| panic!("the generator refused the program: {e}")).wasm
}

/// The built runner this test packages.
fn runner() -> Vec<u8> {
    std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner")
}

/// A copy of the runner carrying `parts`, written to `path`.
fn package(parts: &[(Kind, &[u8])], path: &Path) -> f64 {
    let t = Instant::now();
    let payload = cobolt_program_package::encode(parts);
    let exe = cobolt_program_package::embed(&runner(), &payload, "prc-t15-console").expect("the runner takes a payload");
    cobolt_program_package::write_executable(path, &exe).expect("write the program");
    t.elapsed().as_secs_f64() * 1000.0
}

/// Run `cmd` with the console's lines on its standard input.
fn run(mut cmd: Command, stdin: &str) -> (Output, f64) {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// `rcrun`, built now from the current source (a stale binary would compare against old code).
fn rcrun() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(cargo).current_dir(&root).args(["build", "-q", "-p", "cobolt-cli", "--bin", "rcrun"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-cli --bin rcrun failed");
    // This test runs from <target>/debug/deps; rcrun is in <target>/debug.
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().and_then(Path::parent).unwrap();
    dir.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

/// Both runs get the same environment: the pins, the variables, English.
fn with_env(mut cmd: Command, clock: &str, seed: u64, cache: &Path) -> Command {
    cmd.env("PRC_TEST_CLOCK", clock).env("PRC_TEST_SEED", seed.to_string()).env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8");
    for (k, v) in VARS {
        cmd.env(k, v);
    }
    cmd.env_remove("PRC_T15_NOT_SET");
    cmd
}

/// Rewrite the module's manifest: `edit` changes its JSON.
fn with_manifest(module: &[u8], edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
    let mut out = wasm_encoder::Module::new();
    for payload in wasmparser::Parser::new(0).parse_all(module) {
        let payload = payload.expect("the module parses");
        if let wasmparser::Payload::CustomSection(r) = &payload {
            if r.name() == cobolt_wasm_abi::manifest::MANIFEST_SECTION {
                let mut v: serde_json::Value = serde_json::from_slice(r.data()).unwrap();
                edit(&mut v);
                let data = serde_json::to_vec(&v).unwrap();
                out.section(&wasm_encoder::CustomSection { name: r.name().into(), data: data.into() });
                continue;
            }
        }
        if let Some((id, range)) = payload.as_section() {
            out.section(&wasm_encoder::RawSection { id, data: &module[range] });
        }
    }
    out.finish()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn a_packaged_console_program_matches_rcrun_run_byte_for_byte() {
    let dir = Scratch::new("console");
    let src = dir.0.join("t15console.cbl");
    std::fs::write(&src, PROGRAM).unwrap();

    let t = Instant::now();
    let module = desktop_module(PROGRAM);
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
    let manifest = scan.embedded_manifest().unwrap();
    assert_eq!(manifest.abi, 1, "the module is ABI 1");
    let prn: Vec<String> = scan.imports.iter().filter(|i| i.module == "prn").map(|i| i.name.clone()).collect();

    let t = Instant::now();
    let code = cobolt_runner_engine::precompile(&module, &Target::baseline(&Target::host().triple()).unwrap()).unwrap();
    let precompile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let exe = dir.0.join(format!("t15console{}", std::env::consts::EXE_SUFFIX));
    let package_ms = package(&[(Kind::Module, &module), (Kind::MachineCode, &code.bytes), (Kind::MachineCodeEngine, &code.compat)], &exe);
    let size = std::fs::metadata(&exe).unwrap().len();

    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let cache = dir.0.join("cache");

    let mut checks = 0;
    let mut rows = Vec::new();
    let mut outputs = Vec::new();
    for &(clock, seed) in RUNS {
        let mut packaged = with_env(Command::new(&exe), clock, seed, &cache);
        packaged.args(ARGS).env("PRC_RUNNER_TRACE", "1");
        let (theirs, runner_ms) = run(packaged, STDIN);
        let mut interp = with_env(Command::new(&rcrun), clock, seed, &cache);
        interp.arg("run").arg(&src).args(ARGS).current_dir(&dir.0);
        let (ours, rcrun_ms) = run(interp, STDIN);

        assert_eq!(
            String::from_utf8_lossy(&theirs.stdout),
            String::from_utf8_lossy(&ours.stdout),
            "seed {seed}: the runner's output differs from rcrun run's\nrunner stderr: {}\nrcrun stderr: {}",
            stderr(&theirs),
            stderr(&ours)
        );
        assert_eq!(theirs.stdout, ours.stdout, "seed {seed}: the same bytes");
        checks += 2;
        assert_eq!(theirs.status.code(), Some(7), "seed {seed}: the runner ends with RETURN-CODE\n{}", stderr(&theirs));
        assert_eq!(ours.status.code(), Some(7), "seed {seed}: rcrun run ends with RETURN-CODE");
        checks += 2;
        let trace = stderr(&theirs);
        assert!(trace.contains("loaded by MachineCode"), "seed {seed}: the Build's machine code is used: {trace}");
        checks += 1;
        assert!(!trace.to_ascii_lowercase().contains("cobolt"), "the runner never names its crate: {trace}");
        checks += 1;
        let lines = theirs.stdout.iter().filter(|&&b| b == b'\n').count();
        rows.push(format!("clock {clock}, seed {seed:>10}: {lines} lines, {} bytes, exit 7; runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms", theirs.stdout.len()));
        outputs.push(String::from_utf8_lossy(&theirs.stdout).into_owned());
    }
    // The pins are not a no-op: the clock line is the pinned instant, and the seed moves RANDOM.
    let first = &outputs[0];
    assert!(first.contains("date 260102 time 03040567 day 26002 dow 5"), "{first}");
    assert!(first.contains("stamp 2026010203040567-0300"), "{first}");
    assert!(outputs[2].contains("stamp 2026070123595999+0530"), "the other zone's offset: {}", outputs[2]);
    assert!(first.contains(&format!("name [{:<20}] qty 0042", "Ada Lovelace")), "{first}");
    assert!(first.contains(&format!("home [{:<30}]", "/home/ada")), "{first}");
    assert!(first.contains("none [      ]"), "{first}");
    assert!(first.contains("args 02 [alpha beta gamma"), "{first}");
    let randoms = |o: &str| o.lines().filter(|l| l.starts_with("random ")).map(str::to_owned).collect::<Vec<_>>();
    assert_ne!(randoms(&outputs[0]), randoms(&outputs[1]), "another seed, other RANDOM values");
    checks += 8;

    println!("── a_packaged_console_program_matches_rcrun_run_byte_for_byte ──");
    println!("  program: console ACCEPT ×2, ENVIRONMENT ×2 (set, not set), ARGUMENT-NUMBER, COMMAND-LINE, ARGUMENT-VALUE,");
    println!("           DATE, TIME, DAY, DAY-OF-WEEK, CURRENT-DATE, RANDOM ×3, RETURN-CODE 7");
    println!("  pins (clock, seed): {RUNS:?}");
    println!("  module: {} bytes, ABI {}, prn imports {prn:?}, capabilities {:?}; compiled in {compile_ms:.1} ms", module.len(), manifest.abi, manifest.capabilities);
    println!("  machine code: {} bytes, made in {precompile_ms:.1} ms; packaged program {size} bytes, written in {package_ms:.1} ms", code.bytes.len());
    println!("  rcrun built (or found current) in {rcrun_build_ms:.0} ms");
    for r in &rows {
        println!("  {r}");
    }
    println!("  checks: {checks} of {checks} passed");
}

#[test]
fn a_capability_the_runner_lacks_is_refused_by_name_in_six_languages() {
    let dir = Scratch::new("refuse");
    let module = desktop_module(PROGRAM);
    let http = with_manifest(&module, |v| v["capabilities"] = serde_json::json!(["console", "env", "http"]));
    let exe = dir.0.join(format!("t15http{}", std::env::consts::EXE_SUFFIX));
    package(&[(Kind::Module, &http)], &exe);

    let locales = [(Lang::En, "en_US.UTF-8"), (Lang::Es, "es_ES.UTF-8"), (Lang::Pt, "pt_BR.UTF-8"), (Lang::Fr, "fr_FR.UTF-8"), (Lang::Ja, "ja_JP.UTF-8"), (Lang::Zh, "zh_CN.UTF-8")];
    let want_msg = Msg::Capabilities(vec!["http".into()]);
    let mut checks = 0;
    let mut seen = Vec::new();
    for (lang, locale) in locales {
        let mut cmd = Command::new(&exe);
        cmd.env("LC_ALL", locale).env("PRC_RUNNER_CACHE", dir.0.join("cache"));
        let (out, ms) = run(cmd, "");
        let said = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{locale}: refused");
        assert!(out.stdout.is_empty(), "{locale}: nothing ran");
        assert_eq!(said.trim_end(), text(lang, &want_msg), "{locale}: the system's language");
        assert!(said.contains("http"), "{locale}: names the capability");
        assert!(!said.to_ascii_lowercase().contains("cobolt"), "{locale}: {said}");
        checks += 5;
        seen.push(format!("{locale:<12} exit 1 in {ms:.1} ms: {}", said.trim_end()));
    }

    // An ABI this runner does not run, and a bare runner, are refused too.
    let abi2 = with_manifest(&module, |v| v["abi"] = serde_json::json!(2));
    let exe2 = dir.0.join(format!("t15abi2{}", std::env::consts::EXE_SUFFIX));
    package(&[(Kind::Module, &abi2)], &exe2);
    let mut cmd = Command::new(&exe2);
    cmd.env("LC_ALL", "en_US.UTF-8");
    let (out, _) = run(cmd, "");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stderr(&out).trim_end(), text(Lang::En, &Msg::Abi { found: 2, max: 1 }));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_prc-runner"));
    cmd.env("LC_ALL", "pt_BR.UTF-8");
    let (out, _) = run(cmd, "");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stderr(&out).trim_end(), text(Lang::Pt, &Msg::NoProgram));
    checks += 4;

    println!("── a_capability_the_runner_lacks_is_refused_by_name_in_six_languages ──");
    println!("  a module whose manifest asks for console, env and http, packaged without machine code:");
    for s in &seen {
        println!("  {s}");
    }
    println!("  ABI 2 refused naming ABI 2 and 1 to 1; a bare runner says it carries no program (pt_BR)");
    println!("  checks: {checks} of {checks} passed");
}

#[test]
fn every_message_exists_in_six_languages_without_the_crate_name() {
    let msgs = [
        Msg::NoProgram,
        Msg::Damaged("the table of contents does not match its checksum".into()),
        Msg::Abi { found: 2, max: 1 },
        Msg::Capabilities(vec!["sql".into(), "http".into()]),
        Msg::Import("prn.sql_exec".into()),
        Msg::CannotStart("detail".into()),
        Msg::RuntimeError { code: 9001, line: 12, text: "subscript out of range".into() },
        Msg::Trapped("detail".into()),
        Msg::NoSuchForm { want: "ORDERS-FORM".into(), forms: vec!["MAIN-FORM".into(), "TOOLBAR-FORM".into()] },
    ];
    let mut checks = 0;
    for m in &msgs {
        let all: Vec<String> = Lang::ALL.iter().map(|&l| text(l, m)).collect();
        for (l, t) in Lang::ALL.iter().zip(&all) {
            assert!(!t.trim().is_empty(), "{l:?} {m:?}");
            assert!(!t.to_ascii_lowercase().contains("cobolt"), "{l:?}: {t}");
            checks += 2;
        }
        let distinct: std::collections::BTreeSet<&String> = all.iter().collect();
        assert_eq!(distinct.len(), 6, "six different texts for {m:?}: {all:#?}");
        checks += 1;
        if let Msg::Capabilities(names) = m {
            assert!(all.iter().all(|t| names.iter().all(|n| t.contains(n.as_str()))), "every language names every capability");
            checks += 1;
        }
    }
    let locales = [("pt_BR.UTF-8", Lang::Pt), ("ja_JP", Lang::Ja), ("zh-CN", Lang::Zh), ("C", Lang::En), ("de_DE.UTF-8", Lang::En), ("fr", Lang::Fr), ("es_MX@euro", Lang::Es)];
    for (name, want) in locales {
        assert_eq!(Lang::of_locale(name), Some(want), "{name}");
        checks += 1;
    }
    assert_eq!(Lang::of_locale(""), None);
    checks += 1;
    println!("── every_message_exists_in_six_languages_without_the_crate_name ──");
    println!("  {} messages × 6 languages (en es pt fr ja zh), none naming the crate; {} locale names read", msgs.len(), locales.len() + 1);
    println!("  checks: {checks} of {checks} passed");
}

/// Three external switches, read by their ON and OFF conditions (WASM-PARITY #55).
const SWITCHES: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T55SWITCHES.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SPECIAL-NAMES.
    SWITCH-1 IS SW-ONE
        ON STATUS IS ONE-ON
        OFF STATUS IS ONE-OFF
    SWITCH-2 IS SW-TWO
        ON STATUS IS TWO-ON
    SWITCH-3 IS SW-THREE
        OFF STATUS IS THREE-OFF.
PROCEDURE DIVISION.
    IF ONE-ON DISPLAY \"1 ON\" ELSE DISPLAY \"1 OFF\" END-IF
    IF ONE-OFF DISPLAY \"1 OFF STATUS\" END-IF
    IF TWO-ON DISPLAY \"2 ON\" ELSE DISPLAY \"2 OFF\" END-IF
    IF THREE-OFF DISPLAY \"3 OFF\" ELSE DISPLAY \"3 ON\" END-IF
    STOP RUN.
";

#[test]
fn external_switches_reach_the_program_as_rcrun_run_gives_them() {
    let dir = Scratch::new("switches");
    let src = dir.0.join("t55switches.cbl");
    std::fs::write(&src, SWITCHES).unwrap();
    let t = Instant::now();
    let module = desktop_module(SWITCHES);
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let manifest = cobolt_wasm_abi::scan::scan_module(&module).unwrap().embedded_manifest().unwrap();
    assert!(manifest.capabilities.iter().any(|c| c == "env"), "the switches are read through env: {:?}", manifest.capabilities);
    let exe = dir.0.join(format!("t55switches{}", std::env::consts::EXE_SUFFIX));
    package(&[(Kind::Module, &module)], &exe);
    let rcrun = rcrun();
    let cache = dir.0.join("cache");

    // `COBOL_SWITCHES` (None: not set), and what both runs must print.
    let cases: &[(Option<&str>, &str)] = &[
        (None, "1 OFF\n1 OFF STATUS\n2 OFF\n3 OFF\n"),
        (Some("SWITCH-1=ON"), "1 ON\n2 OFF\n3 OFF\n"),
        (Some(" switch-1 = on ,SW-TWO=yes,sw-three=TRUE"), "1 ON\n2 ON\n3 ON\n"),
        (Some("SWITCH-1=OFF,SWITCH-1=ON,SWITCH-2=MAYBE,SWITCH-2=1,NOEQUALS,,=ON,SW-THREE=0"), "1 OFF\n1 OFF STATUS\n2 ON\n3 OFF\n"),
        (Some("SWITCH-3=ON,SWITCH-2=no,SWITCH-1=true"), "1 ON\n2 OFF\n3 ON\n"),
        (Some("SWITCH-9=ON"), "1 OFF\n1 OFF STATUS\n2 OFF\n3 OFF\n"),
    ];
    let mut checks = 1;
    let mut rows = Vec::new();
    for (value, want) in cases {
        let set = |mut cmd: Command| {
            cmd.env("PRC_RUNNER_CACHE", &cache).env("LC_ALL", "en_US.UTF-8").env_remove("COBOL_SWITCHES");
            if let Some(v) = value {
                cmd.env("COBOL_SWITCHES", v);
            }
            cmd
        };
        let (theirs, runner_ms) = run(set(Command::new(&exe)), "");
        let mut interp = set(Command::new(&rcrun));
        interp.arg("run").arg(&src).current_dir(&dir.0);
        let (ours, rcrun_ms) = run(interp, "");
        assert_eq!(String::from_utf8_lossy(&ours.stdout), *want, "{value:?}: rcrun run\n{}", stderr(&ours));
        assert_eq!(theirs.stdout, ours.stdout, "{value:?}: the runner prints what rcrun run prints\nrunner stderr: {}", stderr(&theirs));
        assert_eq!((theirs.status.code(), ours.status.code()), (Some(0), Some(0)), "{value:?}: both end normally");
        checks += 3;
        rows.push(format!("{:<80} {}; runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms", format!("{value:?}"), want.trim_end().replace('\n', " | ")));
    }
    println!("── external_switches_reach_the_program_as_rcrun_run_gives_them ──");
    println!("  program: SWITCH-1 (ON and OFF STATUS), SWITCH-2 (ON STATUS), SWITCH-3 (OFF STATUS); module {} bytes, capabilities {:?}, compiled in {compile_ms:.1} ms", module.len(), manifest.capabilities);
    println!("  forms of COBOL_SWITCHES: unset; one pair; spaces, lower case and mnemonics; repeats, an unknown state and malformed pairs; every word; an undeclared switch");
    for r in &rows {
        println!("  {r}");
    }
    println!("  checks: {checks} of {checks} passed");
}

/// A program that displays its lines, then keeps running for five seconds by the wall clock before
/// it displays the last one. `CURRENT-DATE` is the host's clock (no test pin), and nothing in the
/// loop hands the module's batch over.
const PROMPT: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T56PROMPT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NOW.
   05 FILLER PIC X(8).
   05 WS-HH PIC 99.
   05 WS-MM PIC 99.
   05 WS-SS PIC 99.
   05 FILLER PIC X(9).
01 WS-START PIC 9(7).
01 WS-AT PIC 9(7).
01 WS-SPINS PIC 9(12) VALUE 0.
PROCEDURE DIVISION.
    DISPLAY \"LINE 1\"
    DISPLAY \"LINE 2\"
    DISPLAY \"PART \" WITH NO ADVANCING
    DISPLAY \"ONE\"
    MOVE FUNCTION CURRENT-DATE TO WS-NOW
    COMPUTE WS-START = WS-HH * 3600 + WS-MM * 60 + WS-SS
    MOVE WS-START TO WS-AT
    PERFORM UNTIL WS-AT >= WS-START + 5 OR WS-AT < WS-START
        ADD 1 TO WS-SPINS
        MOVE FUNCTION CURRENT-DATE TO WS-NOW
        COMPUTE WS-AT = WS-HH * 3600 + WS-MM * 60 + WS-SS
    END-PERFORM
    DISPLAY \"DONE\"
    STOP RUN.
";

#[test]
fn a_console_line_shows_while_the_program_is_still_running() {
    use std::io::BufRead;
    use std::time::Duration;

    let dir = Scratch::new("prompt");
    let src = dir.0.join("t56prompt.cbl");
    std::fs::write(&src, PROMPT).unwrap();
    let module = desktop_module(PROMPT);
    let exe = dir.0.join(format!("t56prompt{}", std::env::consts::EXE_SUFFIX));
    package(&[(Kind::Module, &module)], &exe);
    let cache = dir.0.join("cache");
    let want = ["LINE 1", "LINE 2", "PART ONE", "DONE"];

    // Each line as it reaches the pipe, with the time it arrived.
    let started = Instant::now();
    let mut child = Command::new(&exe)
        .env("PRC_RUNNER_CACHE", &cache)
        .env("LC_ALL", "en_US.UTF-8")
        .env_remove("PRC_TEST_CLOCK")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the program");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send((line, started.elapsed()));
        }
    });
    let mut got: Vec<(String, Duration)> = Vec::new();
    while let Ok(l) = rx.recv_timeout(Duration::from_secs(60)) {
        got.push(l);
    }
    let status = child.wait().unwrap();
    reader.join().unwrap();
    let mut err = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut err).unwrap();
    let lines: Vec<&str> = got.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(lines, want, "the program's lines\nstderr: {err}");
    assert_eq!(status.code(), Some(0), "the program ends normally\nstderr: {err}");

    // The first three lines came before the program's five-second wait ended, not with its end.
    let (first, done) = (got[0].1, got[3].1);
    let ahead = done.saturating_sub(got[2].1);
    assert!(ahead >= Duration::from_secs(3), "the lines before the wait arrived {:.0} ms before the last one: they waited for the end", ahead.as_secs_f64() * 1000.0);

    // The interpreter prints the same lines.
    let mut interp = Command::new(rcrun());
    interp.arg("run").arg(&src).current_dir(&dir.0).env("LC_ALL", "en_US.UTF-8").env_remove("PRC_TEST_CLOCK");
    let (ours, rcrun_ms) = run(interp, "");
    assert_eq!(String::from_utf8_lossy(&ours.stdout), "LINE 1\nLINE 2\nPART ONE\nDONE\n", "rcrun run\n{}", stderr(&ours));

    println!("── a_console_line_shows_while_the_program_is_still_running ──");
    println!("  program: three lines (one written in two parts, NO ADVANCING), a five-second wait on the wall clock, a last line; module {} bytes", module.len());
    for (l, at) in &got {
        println!("  {l:<10} arrived at {:>7.1} ms", at.as_secs_f64() * 1000.0);
    }
    println!("  first line {:.1} ms after the start; the lines before the wait arrived {:.1} ms ahead of the last one; rcrun run {rcrun_ms:.1} ms, same 4 lines", first.as_secs_f64() * 1000.0, ahead.as_secs_f64() * 1000.0);
    println!("  checks: 4 of 4 passed");
}
