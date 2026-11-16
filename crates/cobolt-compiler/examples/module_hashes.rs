// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The modules this generator makes, as sha256 lines (DESKTOP-WASM-001 T22; R2; AC2): the
//! `determinism` job of `.github/workflows/desktop-wasm.yml` runs it from a build made by Rust
//! 1.95.0 and from one made by Rust `stable`, with the same `SOURCE_DATE_EPOCH`, and compares the
//! two lists. They must be identical.
//!
//! ```text
//! cargo run --release -p cobolt-compiler --example module_hashes -- --runner <prc-runner> --out <file>
//! ```
//!
//! One line per program, sorted:
//! - `fixture <name> <sha256> <bytes>`: the module corpus's console fixtures, built as a Desktop
//!   Build builds them (the module taken out of the built program);
//! - `reference <name> wasm <sha256> <bytes>` or `reference <name> cargo <reason>`: the reference
//!   projects of R17, built the same way. One still on the Cargo path gives its reason, which must
//!   also be the same. `PATH` is emptied first, so that path stops at the toolchain check instead
//!   of running `cargo`;
//! - `program <path> <sha256> <bytes>` or `program <path> refused <reason>`: every program in
//!   `tests/cobol`, compiled alone as a desktop module.
//!
//! The module carries its manifest, so a module's hash covers the manifest too (AC2).

#[path = "../../cobolt-runner/tests/support/corpus.rs"]
mod corpus;

use std::path::{Path, PathBuf};

/// The reference projects (R17's note, C7): the Desktop examples with neither `EXEC RUST` nor an
/// External Crate.
const REFERENCE: &[&str] = &["PowerCallOfDuty", "PowerCloud", "PowerCobolCraft", "PowerGraphics", "PowerSpatial"];

fn main() {
    if let Err(e) = run() {
        eprintln!("module_hashes: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut runner: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--runner" => runner = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            _ => return Err(format!("unknown argument {a}; usage: --runner <prc-runner> --out <file>")),
        }
    }
    let runner = runner.ok_or("--runner is required")?.canonicalize().map_err(|e| e.to_string())?;
    let out = out.ok_or("--out is required")?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().map_err(|e| e.to_string())?;
    let epoch = std::env::var(cobolt_wasm::module::SOURCE_DATE_EPOCH).unwrap_or_default();
    // The Cargo path must stop at its toolchain check: no `rustc`, no `cargo`.
    let scratch = std::env::temp_dir().join(format!("prc-module-hashes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(scratch.join("empty-path")).map_err(|e| e.to_string())?;
    std::env::set_var("PATH", scratch.join("empty-path"));

    let mut lines = Vec::new();
    // ── The corpus fixtures ──
    let fixtures = corpus::corpus_dir().join("fixtures");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&fixtures)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("run.toml").is_file())
        .collect();
    names.sort();
    for fixture in &names {
        let name = fixture.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        let run = corpus::read_run(&fixture.join("run.toml"))?;
        let project = scratch.join("fixtures").join(&name);
        std::fs::create_dir_all(&project).map_err(|e| e.to_string())?;
        for s in &run.sources {
            std::fs::copy(fixture.join(s), project.join(s)).map_err(|e| format!("{name}/{s}: {e}"))?;
        }
        let bin = format!("corpus{name}");
        let list = run.sources.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        let manifest = project.join(format!("{bin}.project.toml"));
        std::fs::write(&manifest, format!("[project]\nname = \"{bin}\"\nversion = \"0.1.0\"\nmain = \"{}\"\n\n[files]\nsources = [{list}]\n", run.sources[0]))
            .map_err(|e| e.to_string())?;
        lines.push(match build(&manifest, &runner, &scratch)? {
            Ok(module) => format!("fixture {name} {} {}", corpus::sha256_hex(&module), module.len()),
            Err(reason) => format!("fixture {name} cargo {reason}"),
        });
    }
    // ── The reference projects, copied so the checkout is not written to ──
    for name in REFERENCE {
        let to = scratch.join("reference").join(name);
        copy_project(&workspace.join("examples").join(name), &to)?;
        let manifest = to.join(format!("{name}.project.toml"));
        lines.push(match build(&manifest, &runner, &scratch)? {
            Ok(module) => format!("reference {name} wasm {} {}", corpus::sha256_hex(&module), module.len()),
            Err(reason) => format!("reference {name} cargo {reason}"),
        });
    }
    // ── Every program of tests/cobol, alone ──
    let mut programs = Vec::new();
    collect_cbl(&workspace.join("tests/cobol"), &mut programs);
    programs.sort();
    for p in &programs {
        let rel = p.strip_prefix(&workspace).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let src = cobolt_lexer::read_source_file(p).map_err(|e| format!("{rel}: {e}"))?;
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::detect(&src)));
        let line = match parsed.program {
            Some(program) if !parsed.diagnostics.iter().any(|d| d.is_error()) => {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("program").to_owned();
                let opts = cobolt_wasm::Options { native: true, name: stem, ..cobolt_wasm::Options::default() };
                match cobolt_wasm::compile_program(&program, &opts) {
                    Ok(c) => format!("program {rel} {} {}", corpus::sha256_hex(&c.wasm), c.wasm.len()),
                    Err(e) => format!("program {rel} refused {}", first_line(&e.to_string())),
                }
            }
            _ => format!("program {rel} refused it does not parse"),
        };
        lines.push(line);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    lines.sort();
    let text = lines.join("\n") + "\n";
    std::fs::write(&out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
    let modules = lines.iter().filter(|l| l.split(' ').any(|w| w.len() == 64 && w.chars().all(|c| c.is_ascii_hexdigit()))).count();
    println!("module_hashes: {} lines ({modules} modules) written to {}; SOURCE_DATE_EPOCH={epoch}", lines.len(), out.display());
    Ok(())
}

/// The module the Build makes, or why it took the Cargo path.
fn build(manifest: &Path, runner: &Path, scratch: &Path) -> Result<Result<Vec<u8>, String>, String> {
    let opts = cobolt_compiler::BuildOptions {
        verbose: false,
        build_dir: Some(scratch.join("builds")),
        runner: Some(runner.to_path_buf()),
        ..Default::default()
    };
    match cobolt_compiler::build_project(manifest, &opts) {
        Ok(built) if built.path == cobolt_compiler::BuildPath::Wasm => {
            let exe = std::fs::read(&built.binary_path).map_err(|e| e.to_string())?;
            let payload = cobolt_program_package::locate(&exe).map_err(|e| e.to_string())?;
            let decoded = cobolt_program_package::decode(payload).map_err(|e| e.to_string())?;
            Ok(Ok(decoded.module().map_err(|e| e.to_string())?.to_vec()))
        }
        Ok(built) => Ok(Err(built.cargo.map(|c| c.reason.to_string()).unwrap_or_default())),
        Err(cobolt_compiler::CompilerError::CargoNeedsToolchain { reason, .. }) => Ok(Err(reason.to_string())),
        Err(e) => Ok(Err(format!("the Build failed: {}", first_line(&e.to_string())))),
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_owned()
}

fn collect_cbl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_cbl(&p, out);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("cbl")) {
            out.push(p);
        }
    }
}

/// `from`'s files under `to`, without a previous Build's `bin/`, `target/` or `dist/`.
fn copy_project(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        if matches!(name.to_str(), Some("bin" | "target" | "dist")) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_project(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}
