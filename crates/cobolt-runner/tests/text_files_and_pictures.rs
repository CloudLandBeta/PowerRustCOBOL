// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T39c (R11, ABI A39): `COBOL::"APPEND-FILE"`, `"WRITE-FILE"`, `"IMAGE-FIT"` and
//! `"IMAGE-INFO"` behave on the WebAssembly path as they do under the interpreter. The program runs
//! twice, packaged into a copy of the built `prc-runner` and under `rcrun run`, each in a folder of
//! its own holding the same picture; the two runs must print the same lines and leave the same
//! files, byte for byte (the text files, and the PNG and JPEG pictures IMAGE-FIT writes).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// Every way the four built-ins take their arguments and give their answers:
/// - a path in a data item with spaces around it, as a literal, and through a value method;
/// - a line's text as a literal with trailing spaces, a numeric item, a signed decimal item, a
///   figurative constant, an empty item; the optional status left out, and a failure's status;
/// - a width as a decimal item (its whole part), a height as a text item; a size of 0 refused;
/// - a target of an unknown kind, an absent source, a file that is not a picture;
/// - the answers in numeric, alphanumeric and too-short receivers.
const PROGRAM: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39CTXT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PATH    PIC X(20) VALUE \"  log.txt \".
01 WS-STATUS  PIC X(60) VALUE \"UNTOUCHED\".
01 WS-NUM     PIC 9(4) VALUE 42.
01 WS-SIGNED  PIC S9(3)V99 VALUE -7.5.
01 WS-EMPTY   PIC X(8).
01 WS-SOURCE  PIC X(30) VALUE \"button.png\".
01 WS-W       PIC 99V9 VALUE 16.7.
01 WS-H       PIC X(6) VALUE \" 8 \".
01 WS-WIDTH   PIC 9(5).
01 WS-HEIGHT  PIC X(6).
01 WS-FORMAT  PIC X(5).
01 WS-WORD    PIC X(10).
01 WS-SHORT   PIC X(3).
01 WS-ONE     PIC X.
PROCEDURE DIVISION.
    COBOL::\"WRITE-FILE\" ( WS-PATH \"first line   \" WS-STATUS )
    DISPLAY \"write [\" WS-STATUS \"]\"
    COBOL::\"APPEND-FILE\" ( \"log.txt\" WS-NUM )
    COBOL::\"APPEND-FILE\" ( WS-PATH::Trim() WS-SIGNED WS-STATUS )
    DISPLAY \"append signed [\" WS-STATUS \"]\"
    COBOL::\"APPEND-FILE\" ( \"log.txt\" ZERO )
    COBOL::\"APPEND-FILE\" ( \"log.txt\" SPACES )
    COBOL::\"APPEND-FILE\" ( \"log.txt\" WS-EMPTY )
    COBOL::\"APPEND-FILE\" ( \"log.txt\" \"  last  \" )
    COBOL::\"APPEND-FILE\" ( \"no-such-folder/x.txt\" \"lost\" WS-STATUS )
    DISPLAY \"append missing folder [\" WS-STATUS \"]\"
    COBOL::\"WRITE-FILE\" ( \"bad.png\" \"not a picture\" )
    COBOL::\"WRITE-FILE\" ( \"twice.txt\" \"one\" )
    COBOL::\"WRITE-FILE\" ( \"twice.txt\" \"two\" WS-STATUS )
    DISPLAY \"rewrite [\" WS-STATUS \"]\"

    COBOL::\"IMAGE-INFO\" ( WS-SOURCE WS-WIDTH WS-HEIGHT WS-FORMAT WS-WORD )
    DISPLAY \"info source [\" WS-WIDTH \"] [\" WS-HEIGHT \"] [\" WS-FORMAT \"] [\" WS-WORD \"]\"
    COBOL::\"IMAGE-FIT\" ( WS-SOURCE \"small.png\" WS-W WS-H WS-WORD )
    DISPLAY \"fit png [\" WS-WORD \"]\"
    COBOL::\"IMAGE-INFO\" ( \"small.png\" WS-WIDTH WS-HEIGHT WS-FORMAT WS-WORD )
    DISPLAY \"info png [\" WS-WIDTH \"] [\" WS-HEIGHT \"] [\" WS-FORMAT \"] [\" WS-WORD \"]\"
    COBOL::\"IMAGE-FIT\" ( WS-SOURCE::Trim() \" wide.jpg \" 30 10 WS-WORD )
    DISPLAY \"fit jpeg [\" WS-WORD \"]\"
    COBOL::\"IMAGE-INFO\" ( \"wide.jpg\" WS-SHORT WS-SHORT WS-FORMAT WS-WORD )
    DISPLAY \"info jpeg [\" WS-SHORT \"] [\" WS-FORMAT \"] [\" WS-WORD \"]\"
    COBOL::\"IMAGE-INFO\" ( WS-SOURCE WS-ONE WS-HEIGHT WS-FORMAT WS-WORD )
    DISPLAY \"info one [\" WS-ONE \"] [\" WS-HEIGHT \"]\"
    COBOL::\"IMAGE-FIT\" ( WS-SOURCE \"zero.png\" 0 8 WS-WORD )
    DISPLAY \"fit zero [\" WS-WORD \"]\"
    COBOL::\"IMAGE-FIT\" ( WS-SOURCE \"small.gif\" 8 8 WS-WORD )
    DISPLAY \"fit gif [\" WS-WORD \"]\"
    COBOL::\"IMAGE-FIT\" ( \"absent.png\" \"x.png\" 8 8 WS-WORD )
    DISPLAY \"fit absent [\" WS-WORD \"]\"
    COBOL::\"IMAGE-INFO\" ( \"bad.png\" WS-WIDTH WS-HEIGHT WS-FORMAT WS-WORD )
    DISPLAY \"info bad [\" WS-WIDTH \"] [\" WS-HEIGHT \"] [\" WS-FORMAT \"] [\" WS-WORD \"]\"
    COBOL::\"IMAGE-INFO\" ( \"absent.png\" WS-WIDTH WS-HEIGHT WS-FORMAT WS-WORD )
    DISPLAY \"info absent [\" WS-WIDTH \"] [\" WS-HEIGHT \"] [\" WS-FORMAT \"] [\" WS-WORD \"]\"
    STOP RUN.
";

/// Lines the run must print (on both paths), so a twin that agrees on a wrong answer still fails.
const EXPECTED: &[&str] = &[
    "write [                                                            ]",
    "info source [00040] [    40] [PNG  ] [OK        ]",
    "fit png [OK        ]",
    "info png [00016] [     8] [PNG  ] [OK        ]",
    "fit jpeg [OK        ]",
    "info jpeg [ 10] [JPEG ] [OK        ]",
    "info one [4] [    40]",
    "fit zero [BADSIZE   ]",
    "fit gif [BADEXT    ]",
    "fit absent [NOFILE    ]",
    "info bad [00000] [     0] [     ] [BADIMAGE  ]",
    "info absent [00000] [     0] [     ] [NOFILE    ]",
];

/// A program that writes text files only asks for `files.text` and imports `t_write` alone.
const TEXT_ONLY: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39CONE.
PROCEDURE DIVISION.
    COBOL::\"APPEND-FILE\" ( \"one.txt\" \"one\" )
    STOP RUN.
";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t39c-{tag}-{}", std::process::id()));
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

/// `rcrun`, built now from the current source (a stale binary would compare against old code).
fn rcrun() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(cargo).current_dir(&root).args(["build", "-q", "-p", "cobolt-cli", "--bin", "rcrun"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-cli --bin rcrun failed");
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().and_then(Path::parent).unwrap();
    dir.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

fn run(mut cmd: Command, cache: &Path) -> (Output, f64) {
    cmd.env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8");
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    let _ = child.stdin.take().unwrap().write_all(b"");
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// Every file a run left in `dir`, by name, but the program's own source and executable.
fn files_in(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .filter(|(n, _)| !n.ends_with(".cbl") && !n.starts_with("prog"))
        .collect()
}

/// The capabilities and the `prn` imports of a native module.
fn needs(module: &[u8]) -> (Vec<String>, Vec<String>) {
    let scan = cobolt_wasm_abi::scan::scan_module(module).unwrap();
    let mut prn: Vec<String> =
        scan.imports.iter().filter(|i| i.module == cobolt_wasm_abi::codes::NATIVE_IMPORT_MODULE).map(|i| i.name.clone()).collect();
    prn.sort();
    (scan.embedded_manifest().unwrap().capabilities, prn)
}

#[test]
fn text_files_and_pictures_match_the_interpreter() {
    let root = Scratch::new("files");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let picture = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes/neumorphic/button.png")).expect("the test picture");
    let mut checks = 0;

    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let one = cobolt_wasm::compile_program(&parse(TEXT_ONLY), &opts).expect("the generator compiles the text-only program").wasm;
    assert_eq!(needs(&one), (vec!["files.text".to_string()], vec!["t_write".to_string()]), "a text file alone needs `files.text` and `t_write`");
    checks += 1;

    let t = Instant::now();
    let module = cobolt_wasm::compile_program(&parse(PROGRAM), &opts).unwrap_or_else(|e| panic!("the generator refused the program: {e}")).wasm;
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let (caps, prn) = needs(&module);
    assert_eq!(caps, vec!["files.text".to_string(), "images".to_string()], "the manifest asks for both capabilities");
    assert_eq!(prn, vec!["img_fit".to_string(), "img_info".to_string(), "t_write".to_string()], "the module imports the three verbs");
    checks += 2;

    let ours = root.0.join("runner");
    let theirs = root.0.join("rcrun");
    for dir in [&ours, &theirs] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("button.png"), &picture).unwrap();
    }
    let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
    let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
    let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t39c-files").expect("the runner takes a payload");
    cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
    let source = theirs.join("t39c.cbl");
    std::fs::write(&source, PROGRAM).unwrap();

    let mut a = Command::new(&exe);
    a.current_dir(&ours);
    let (runner_out, runner_ms) = run(a, &cache);
    let mut b = Command::new(&rcrun);
    b.arg("run").arg(&source).current_dir(&theirs);
    let (rcrun_out, rcrun_ms) = run(b, &cache);

    // The interpreter logs a failed write as a WARN line with a timestamp (rcrun and the Cargo-built
    // program alike); a log line is not the program's output, so both outputs are read without them.
    let said = |o: &Output| -> String {
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        text.lines().filter(|l| !(l.starts_with("\u{1b}[2m") && l.contains(" WARN"))).map(|l| format!("{l}\n")).collect()
    };
    let logged = String::from_utf8_lossy(&rcrun_out.stdout).lines().count() - said(&rcrun_out).lines().count();
|o: &Output| String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(runner_out.status.success(), "the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
    assert!(rcrun_out.status.success(), "rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
    assert_eq!(said(&runner_out), said(&rcrun_out), "the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
    checks += 3;
    let lines = said(&runner_out).lines().count();
    for e in EXPECTED {
        assert!(said(&runner_out).lines().any(|l| l == *e), "the run prints `{e}`:\n{}", said(&runner_out));
        checks += 1;
    }

    let left = files_in(&ours);
    let right = files_in(&theirs);
    assert_eq!(left.keys().collect::<Vec<_>>(), right.keys().collect::<Vec<_>>(), "the same files are made");
    checks += 1;
    let mut bytes = 0;
    for (name, data) in &left {
        assert!(data == &right[name], "{name} differs ({} bytes from the runner, {} from rcrun run)", data.len(), right[name].len());
        bytes += data.len();
        checks += 1;
    }
    assert_eq!(String::from_utf8_lossy(&left["log.txt"]), "first line\n42\n-7.50\n0\n\n\n  last\n", "the log holds the interpreter's lines");
    assert_eq!(left["twice.txt"], b"two\n", "WRITE-FILE replaces the file");
    checks += 2;

    println!("── text_files_and_pictures_match_the_interpreter ──");
    println!("  text:     WRITE-FILE / APPEND-FILE with a padded path item, a literal and a value method (::Trim());");
    println!("            texts: a literal with trailing spaces, PIC 9(4), PIC S9(3)V99, ZERO, SPACES, an empty item;");
    println!("            status: left out, written (spaces), a missing folder's error; WRITE-FILE twice replaces");
    println!("  pictures: IMAGE-INFO of a 40 x 40 PNG; IMAGE-FIT to 16 x 8 PNG (width PIC 99V9 16.7, height text \" 8 \")");
    println!("            and to 30 x 10 JPEG (a padded target); refusals BADSIZE, BADEXT, NOFILE, BADIMAGE;");
    println!("            receivers numeric and alphanumeric (a number right-justified there, as set_i64 puts it) and too short (PIC X(3), PIC X: the digits' left end)");
    println!("  manifest: files.text + images, imports t_write, img_fit, img_info; a text-only program: files.text, t_write");
    println!("  run:      {lines} lines printed and equal (rcrun's {logged} log line left out), {} files ({bytes} bytes) equal byte for byte", left.len());
    println!("  timings:  generate {compile_ms:.1} ms, runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms, rcrun built in {rcrun_build_ms:.0} ms");
    println!("  checks: {checks} passed, 0 failed");
}
