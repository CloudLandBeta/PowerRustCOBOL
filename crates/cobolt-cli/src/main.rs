// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun` — RustCOBOL command-line runtime, packager, and binary compiler.
//!
//! # Usage
//!
//! ```text
//! rcrun run     <file.cbl>            # run a COBOL source file
//! rcrun check   <file.cbl>            # parse + semantic analysis only
//! rcrun build   <file.cbl>            # compile a single console program → bin/<name>
//! rcrun build   [cobolt.toml]         # compile a project → single native binary in bin/
//! rcrun package [cobolt.toml]         # package project into a zip archive
//! rcrun version                       # print version and exit
//! ```
//!
//! ## `rcrun build` flags
//!
//! ```text
//! --quiet          Suppress progress output
//! ```
//!
//! ## `rcrun package` flags
//!
//! ```text
//! --output <path.zip>   Where to write the archive (default: <project-name>.zip)
//! ```
//!
//! ## Environment variables
//!
//! | Variable          | Effect                                                |
//! |-------------------|-------------------------------------------------------|
//! | `COBOLT_LOG`      | Tracing filter (e.g. `cobolt=debug`, `warn`)          |
//! | `COBOLT_FIXED`    | Force fixed-form source parsing (overrides auto-detect)|

use std::path::PathBuf;
use std::process;

use serde::Deserialize;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{IndexedEngine, Interpreter};
use cobolt_semantic::{analyze, Severity};

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("COBOLT_LOG")
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("run")     => cmd_run(&args[2..]),
        Some("check")   => cmd_check(&args[2..]),
        Some("build")   => cmd_build(&args[2..]),
        Some("package") => cmd_package(&args[2..]),
        Some("version") => cmd_version(),
        Some("help") | Some("--help") | Some("-h") => cmd_help(),
        Some(other) => {
            eprintln!("cobolt: unknown command '{other}'");
            eprintln!("Run `rcrun help` for usage.");
            process::exit(2);
        }
        None => {
            cmd_help();
            process::exit(0);
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Expand COPY / REPLACE directives, resolving copybooks next to the source
/// file. Returns free-form text ready to tokenize; copybook errors are printed.
fn expand_copy(path: &PathBuf, source: &str, fmt: SourceFormat) -> String {
    let base = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let expansion = cobolt_lexer::expand_copybooks(source, base, fmt);
    for e in &expansion.errors {
        eprintln!("{}: copybook error: {e}", path.display());
    }
    expansion.text
}

fn cmd_run(args: &[String]) {
    let path   = require_path(args, "run");
    let source = read_source(&path);
    let fmt    = detect_format(&source, &path);
    let source = expand_copy(&path, &source, fmt);

    // COPY expansion flattens to free form.
    let tokens       = tokenize(&source, SourceFormat::Free);
    let parse_result = parse(tokens);

    // Print parser diagnostics.
    for d in &parse_result.diagnostics {
        let sev = match d.severity {
            cobolt_parser::Severity::Error   => "error",
            cobolt_parser::Severity::Warning => "warning",
        };
        eprintln!("{}:{}:{}: {sev}: {}", path.display(), d.span.line, d.span.col, d.message);
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => {
            eprintln!("cobolt: parse failed — aborting.");
            process::exit(1);
        }
    };

    // Semantic analysis.
    let sem = analyze(&program);
    let has_errors = print_diagnostics(&sem.diagnostics, &path.display().to_string());
    if has_errors {
        eprintln!("cobolt: aborting due to semantic errors.");
        process::exit(1);
    }

    // Execute.
    let mut interp = Interpreter::new(program);
    interp.set_indexed_engine(resolve_indexed_engine(args));
    interp.set_indexed_log_level(resolve_indexed_log_level(args));
    interp.set_indexed_log_format(resolve_indexed_log_format(args));
    interp.set_program_args(extract_program_args(args));
    match interp.run() {
        Ok(()) => {}
        Err(e) if e.is_exit_signal() => {}
        Err(e) => {
            eprintln!("cobolt: runtime error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_check(args: &[String]) {
    let path   = require_path(args, "check");
    let source = read_source(&path);
    let fmt    = detect_format(&source, &path);
    let source = expand_copy(&path, &source, fmt);

    let tokens       = tokenize(&source, SourceFormat::Free);
    let parse_result = parse(tokens);

    let mut has_errors = false;
    for d in &parse_result.diagnostics {
        if d.severity == cobolt_parser::Severity::Error { has_errors = true; }
        let sev = match d.severity {
            cobolt_parser::Severity::Error   => "error",
            cobolt_parser::Severity::Warning => "warning",
        };
        eprintln!("{}:{}:{}: {sev}: {}", path.display(), d.span.line, d.span.col, d.message);
    }

    match parse_result.program {
        None => {
            eprintln!("{}: check FAILED (parse error)", path.display());
            process::exit(1);
        }
        Some(prog) => {
            let sem = analyze(&prog);
            has_errors |= print_diagnostics(&sem.diagnostics, &path.display().to_string());

            if has_errors {
                eprintln!("{}: check FAILED", path.display());
                process::exit(1);
            } else {
                println!("{}: check OK ({} warning(s))",
                    path.display(),
                    sem.warnings().count());
            }
        }
    }
}

fn cmd_version() {
    println!("rcrun {} (RustCOBOL runtime)", env!("CARGO_PKG_VERSION"));
}

fn cmd_help() {
    println!(concat!(
        "rcrun — RustCOBOL runtime  (part of PowerRustCOBOL)\n",
        "\n",
        "USAGE:\n",
        "  rcrun run     <file.cbl>              Run a COBOL program\n",
        "         [--indexed-engine <name>]       ISAM engine: rust (default) | rm-cobol85 | fujitsu | redb\n",
        "         [--indexed-log <basic|full>]    Per-file INDEXED txn log → <assign-path>.log (redb)\n",
        "         [--indexed-log-format <text|json>]  Log line format (json = NDJSON for Grafana/Loki)\n",
        "  rcrun check   <file.cbl>              Parse and analyse without running\n",
        "  rcrun build   <file.cbl>             Compile a console program → bin/<name> (native binary)\n",
        "  rcrun build   [cobolt.toml]           Compile a project → bin/<name> (single executable)\n",
        "         [--quiet]                       Suppress build progress output\n",
        "  rcrun package [cobolt.toml]           Package project into a zip archive\n",
        "         [--output <path.zip>]           Override the output archive path\n",
        "  rcrun version                         Print version\n",
        "  rcrun help                            Print this message\n",
        "\n",
        "ENVIRONMENT:\n",
        "  COBOLT_LOG            Logging filter (e.g. warn, debug, cobolt-runtime=trace)\n",
        "  COBOLT_FIXED          Set to '1' to force fixed-form source parsing\n",
        "  COBOL_INDEXED_ENGINE  Indexed (ISAM) engine: rust | rm-cobol85 | fujitsu | redb\n",
        "  COBOL_INDEXED_LOG     INDEXED transaction log level: off (default) | basic | full\n",
    ));
}

// ── Build command (Phase 11) ──────────────────────────────────────────────────

/// Compile a Cobolt project (or a single COBOL source file) into a native binary.
///
/// Usage:
///   `rcrun build [cobolt.toml] [--quiet]`   — project build
///   `rcrun build prog.cbl [--quiet]`        — standalone console program
fn cmd_build(args: &[String]) {
    let mut target: Option<std::path::PathBuf> = None;
    let mut quiet = false;

    for arg in args {
        match arg.as_str() {
            "--quiet" | "-q" => quiet = true,
            a if !a.starts_with('-') => target = Some(std::path::PathBuf::from(a)),
            other => {
                eprintln!("rcrun build: unknown flag '{other}'");
                process::exit(2);
            }
        }
    }

    let opts = cobolt_compiler::BuildOptions {
        verbose: !quiet,
        workspace_root: None,
    };

    let target = target.unwrap_or_else(|| std::path::PathBuf::from("cobolt.toml"));

    // A bare COBOL source file → standalone single-file build (no manifest).
    let is_source = matches!(
        target.extension().and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("cbl" | "cob" | "cbk" | "cpy")
    );

    let result = if is_source {
        if !target.exists() {
            eprintln!("rcrun build: source file not found: '{}'", target.display());
            process::exit(1);
        }
        cobolt_compiler::build_single_file(&target, &opts)
    } else {
        if !target.exists() {
            eprintln!(
                "rcrun build: manifest not found: '{}'\n  \
                 Pass a COBOL source file (`rcrun build prog.cbl`) for a console-only\n  \
                 program, or run inside a project directory containing a cobolt.toml.",
                target.display()
            );
            process::exit(1);
        }
        cobolt_compiler::build_project(&target, &opts)
    };

    match result {
        Ok(result) => {
            println!(
                "✅ Build complete!\n   Binary : {}\n   Sources: {}\n   Forms  : {}\n   AST    : {} bytes (compressed)",
                result.binary_path.display(),
                result.source_count,
                result.form_count,
                result.ast_bytes,
            );
        }
        Err(e) => {
            eprintln!("rcrun build: {e}");
            process::exit(1);
        }
    }
}

// ── Package command ───────────────────────────────────────────────────────────

/// Package a Cobolt project into a self-contained zip archive.
///
/// Usage:
///   `rcrun package [cobolt.toml] [--output path.zip]`
///
/// If no manifest path is given, the function looks for `cobolt.toml` in the
/// current directory.  The output zip defaults to `<project-name>.zip` in the
/// current directory.
fn cmd_package(args: &[String]) {
    use std::io::{Read, Write};

    // ── Argument parsing ──────────────────────────────────────────────────────

    let mut manifest_path: Option<PathBuf> = None;
    let mut output_path:   Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("rcrun package: --output requires a path argument");
                    process::exit(2);
                }
                output_path = Some(PathBuf::from(&args[i]));
            }
            arg if !arg.starts_with('-') => {
                manifest_path = Some(PathBuf::from(arg));
            }
            other => {
                eprintln!("rcrun package: unknown flag '{other}'");
                process::exit(2);
            }
        }
        i += 1;
    }

    // Default: cobolt.toml in the current directory.
    let manifest_path = manifest_path.unwrap_or_else(|| PathBuf::from("cobolt.toml"));

    if !manifest_path.exists() {
        eprintln!(
            "rcrun package: manifest not found: '{}'",
            manifest_path.display()
        );
        eprintln!("  Run `rcrun package <path/to/cobolt.toml>` or cd into the project root.");
        process::exit(1);
    }

    // ── Load the manifest ─────────────────────────────────────────────────────

    #[derive(Deserialize)]
    struct ProjectMeta    { name: String, version: String, main: String }
    #[derive(Deserialize, Default)]
    struct ProjectFiles   {
        #[serde(default)] sources: Vec<String>,
        #[serde(default)] forms:   Vec<String>,
        #[serde(default)] assets:  Vec<String>,
    }
    #[derive(Deserialize)]
    struct CoboltProject  { project: ProjectMeta, #[serde(default)] files: ProjectFiles }

    let manifest_text = match std::fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("rcrun package: cannot read '{}': {e}", manifest_path.display());
            process::exit(1);
        }
    };

    let proj: CoboltProject = match toml::from_str(&manifest_text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rcrun package: malformed cobolt.toml: {e}");
            process::exit(1);
        }
    };

    let project_dir = manifest_path
        .canonicalize()
        .unwrap_or_else(|_| manifest_path.clone())
        .parent()
        .map(|p| p.to_owned())
        .unwrap_or_else(|| PathBuf::from("."));

    // ── Resolve output path ───────────────────────────────────────────────────

    let zip_name = format!(
        "{}.zip",
        proj.project.name.to_ascii_lowercase().replace(' ', "_")
    );
    let out_zip = output_path.unwrap_or_else(|| PathBuf::from(&zip_name));

    println!(
        "rcrun package: packaging '{}' v{} → {}",
        proj.project.name,
        proj.project.version,
        out_zip.display()
    );

    // ── Build zip ─────────────────────────────────────────────────────────────

    let file = match std::fs::File::create(&out_zip) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("rcrun package: cannot create '{}': {e}", out_zip.display());
            process::exit(1);
        }
    };

    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let exec_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let mut count = 0usize;
    let mut missing: Vec<String> = Vec::new();

    // ── cobolt.toml ───────────────────────────────────────────────────────────
    if let Err(e) = zip.start_file("cobolt.toml", opts) {
        eprintln!("rcrun package: zip error: {e}");
        process::exit(1);
    }
    zip.write_all(manifest_text.as_bytes()).unwrap();
    count += 1;

    // ── Required Apache-2.0 notices ───────────────────────────────────────────
    for (name, text) in [
        ("LICENSE", cobolt_compiler::LICENSE_TEXT),
        ("NOTICE", cobolt_compiler::NOTICE_TEXT),
        ("POWERRUSTCOBOL-NOTICE.txt", cobolt_compiler::RUNTIME_NOTICE_TEXT),
    ] {
        if zip.start_file(name, opts).is_ok() {
            zip.write_all(text.as_bytes()).unwrap();
            count += 1;
        }
    }

    // ── Project files ─────────────────────────────────────────────────────────
    let all_files: Vec<&str> = proj.files.sources.iter()
        .chain(proj.files.forms.iter())
        .chain(proj.files.assets.iter())
        .map(|s| s.as_str())
        .collect();

    for rel in &all_files {
        let abs = project_dir.join(rel);
        if !abs.exists() {
            missing.push(rel.to_string());
            continue;
        }
        let mut f = match std::fs::File::open(&abs) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  warning: cannot read '{}': {e}", abs.display());
                continue;
            }
        };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        if let Err(e) = zip.start_file(*rel, opts) {
            eprintln!("rcrun package: zip error: {e}");
            process::exit(1);
        }
        zip.write_all(&buf).unwrap();
        count += 1;
        println!("  + {rel}");
    }

    // ── cobolt binary (optional) ──────────────────────────────────────────────
    if let Some(bin) = find_cobolt_binary() {
        let mut f = std::fs::File::open(&bin).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let name = bin.file_name().and_then(|n| n.to_str()).unwrap_or("cobolt");
        zip.start_file(name, exec_opts).unwrap();
        zip.write_all(&buf).unwrap();
        count += 1;
        println!("  + {name}  (runtime binary)");
    }

    // ── run.sh ────────────────────────────────────────────────────────────────
    let main = &proj.project.main;
    let sh = format!(
        "#!/bin/sh\n\
         DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
         COBOLT=\"$DIR/cobolt\"\n\
         if [ ! -x \"$COBOLT\" ]; then COBOLT=cobolt; fi\n\
         exec \"$COBOLT\" run \"$DIR/{main}\" \"$@\"\n"
    );
    zip.start_file("run.sh", exec_opts).unwrap();
    zip.write_all(sh.as_bytes()).unwrap();

    // ── run.bat ───────────────────────────────────────────────────────────────
    let bat = format!(
        "@echo off\r\n\
         set DIR=%~dp0\r\n\
         if exist \"%DIR%cobolt.exe\" (\r\n\
             \"%DIR%cobolt.exe\" run \"%DIR%{main}\" %*\r\n\
         ) else (\r\n\
             rcrun run \"%DIR%{main}\" %*\r\n\
         )\r\n"
    );
    zip.start_file("run.bat", opts).unwrap();
    zip.write_all(bat.as_bytes()).unwrap();

    // ── README.txt ────────────────────────────────────────────────────────────
    let readme = format!(
        "# {} {}\n\
         \n\
         To run this project:\n\
         \n\
           On Linux / macOS:  chmod +x run.sh cobolt 2>/dev/null && ./run.sh\n\
           On Windows:        run.bat\n\
         \n\
         If the 'cobolt' binary is not included, install it first:\n\
           cargo install cobolt-cli  # installs as 'rcrun'\n\
         \n\
         Main entry point: {main}\n",
        proj.project.name,
        proj.project.version,
    );
    zip.start_file("README.txt", opts).unwrap();
    zip.write_all(readme.as_bytes()).unwrap();

    if let Err(e) = zip.finish() {
        eprintln!("rcrun package: zip finalisation failed: {e}");
        process::exit(1);
    }

    // ── Summary ───────────────────────────────────────────────────────────────
    println!(
        "\nrcrun package: done — {count} file(s) → {}",
        out_zip.display()
    );
    for m in &missing {
        eprintln!("  warning: missing file skipped: {m}");
    }
}

/// Look for a `cobolt` / `cobolt.exe` binary next to the currently running
/// executable, so it can be bundled into the archive automatically.
fn find_cobolt_binary() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in &["rcrun", "rcrun.exe"] {
                let c = dir.join(name);
                if c.exists() && c != exe {
                    return Some(c);
                }
            }
        }
    }
    None
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// The COBOL program's own command-line arguments: everything after the source
/// path (rcrun's own flags before the path are skipped). These feed
/// `ACCEPT … FROM COMMAND-LINE / ARGUMENT-NUMBER / ARGUMENT-VALUE`.
fn extract_program_args(args: &[String]) -> Vec<String> {
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--indexed-engine" || a == "-I" || a == "--indexed-log" || a == "--indexed-log-format" {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        // `a` is the source path — the rest are program arguments.
        return args.get(i + 1..).map(|s| s.to_vec()).unwrap_or_default();
    }
    Vec::new()
}

fn require_path(args: &[String], cmd: &str) -> PathBuf {
    // The first non-flag argument is the source path. Skip recognised options
    // (and their values) so e.g. `--indexed-engine rust file.cbl` works.
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--indexed-engine" || a == "-I" || a == "--indexed-log" || a == "--indexed-log-format" {
            i += 2; // skip the flag and its separate value
            continue;
        }
        if a.starts_with('-') {
            i += 1; // skip `--flag=value` or any other lone flag
            continue;
        }
        return PathBuf::from(a);
    }
    eprintln!("cobolt {cmd}: missing <file> argument");
    process::exit(2);
}

/// Resolve the indexed (ISAM) engine: `--indexed-engine <name>` /
/// `--indexed-engine=<name>` (or `-I`) takes priority, then the
/// `COBOL_INDEXED_ENGINE` environment variable, then the default (Rust).
fn resolve_indexed_engine(args: &[String]) -> IndexedEngine {
    let mut chosen: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(v) = a.strip_prefix("--indexed-engine=") {
            chosen = Some(v.to_string());
        } else if a == "--indexed-engine" || a == "-I" {
            chosen = args.get(i + 1).cloned();
            i += 1;
        }
        i += 1;
    }
    let chosen = chosen.or_else(|| std::env::var("COBOL_INDEXED_ENGINE").ok());
    match chosen {
        Some(name) => match IndexedEngine::parse(&name) {
            Some(e) => e,
            None => {
                eprintln!(
                    "cobolt: unknown indexed engine '{name}' \
                     (expected: rust | rm-cobol85 | fujitsu | redb); using rust"
                );
                IndexedEngine::Rust
            }
        },
        None => IndexedEngine::Rust,
    }
}

/// Resolve the INDEXED observability log level: `--indexed-log <basic|full>` /
/// `--indexed-log=<...>` takes priority, then `COBOL_INDEXED_LOG`, then Off.
/// `--indexed-log true` is an alias for `basic`. The log (redb engine only) is
/// written to `<assign-path>.log`.
fn resolve_indexed_log_level(args: &[String]) -> cobolt_runtime::indexed_log::LogLevel {
    let mut chosen: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(v) = a.strip_prefix("--indexed-log=") {
            chosen = Some(v.to_string());
        } else if a == "--indexed-log" {
            chosen = args.get(i + 1).cloned();
            i += 1;
        }
        i += 1;
    }
    let chosen = chosen.or_else(|| std::env::var("COBOL_INDEXED_LOG").ok());
    chosen
        .map(|s| cobolt_runtime::indexed_log::LogLevel::parse(&s))
        .unwrap_or(cobolt_runtime::indexed_log::LogLevel::Off)
}

/// Resolve the INDEXED log line format: `--indexed-log-format <text|json>` /
/// `--indexed-log-format=<...>`, then `COBOL_INDEXED_LOG_FORMAT`, then text
/// (logfmt). `json` emits NDJSON for Grafana/Loki (`| json`).
fn resolve_indexed_log_format(args: &[String]) -> cobolt_runtime::indexed_log::LogFormat {
    let mut chosen: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(v) = a.strip_prefix("--indexed-log-format=") {
            chosen = Some(v.to_string());
        } else if a == "--indexed-log-format" {
            chosen = args.get(i + 1).cloned();
            i += 1;
        }
        i += 1;
    }
    let chosen = chosen.or_else(|| std::env::var("COBOL_INDEXED_LOG_FORMAT").ok());
    chosen
        .map(|s| cobolt_runtime::indexed_log::LogFormat::parse(&s))
        .unwrap_or(cobolt_runtime::indexed_log::LogFormat::Text)
}

fn read_source(path: &PathBuf) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cobolt: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    }
}

fn detect_format(_source: &str, _path: &PathBuf) -> SourceFormat {
    // PowerRustCOBOL source is treated as free form. (Set COBOLT_FIXED=1 to opt
    // into fixed-form parsing for legacy fixed-column sources.)
    if std::env::var("COBOLT_FIXED").as_deref() == Ok("1") {
        return SourceFormat::Fixed;
    }
    SourceFormat::Free
}

fn print_diagnostics(
    diagnostics: &[cobolt_semantic::SemanticDiagnostic],
    file: &str,
) -> bool {
    let mut has_errors = false;
    for d in diagnostics {
        let sev = match d.severity {
            Severity::Error   => { has_errors = true; "error" }
            Severity::Warning => "warning",
            Severity::Info    => "note",
        };
        eprintln!("{file}:{}:{}: {sev}: {}", d.span.line, d.span.col, d.message);
    }
    has_errors
}
