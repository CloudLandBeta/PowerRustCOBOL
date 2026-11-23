// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! A stand-in for `PowerRustCOBOL.exe --prepare-platform` (spec 112, task T13).
//!
//! The installer check (`.github/workflows/installer-check.yml`) installs the real `.msi`, built from the real WiX, on
//! a hosted Windows runner, and needs the steps the installer starts to be quick and observable. This program is
//! that step: installed as `PowerRustCOBOL.exe` in a tiny staged tree, it takes the very command line the WiX hands
//! the supervisor, prints the progress lines the real step prints, writes the outcomes the real step writes, makes
//! (and marks) the compile's cache folder the way the real compile does, and obeys the cancel file. What it proves
//! is the installer's plumbing: the sequence, the data each action is given, the supervisor, the bar, the rollback.
//! What it cannot prove is the compile itself, which `prepare-platform-smoke.yml` and the operator's clean-machine
//! run cover.
//!
//! ```text
//! prepare_stub --prepare-platform --step rust|compile --result <file> [--cancel-file <file>] [--build-root <dir>]
//! ```
//!
//! Beside the executable it reads `prepare_stub.cfg` (`compile_seconds=<n>`, default 6, how long the compile takes)
//! and appends one line per run to `prepare_stub.log`, so a check can tell what ran, and with what.

use cobolt_setup::prepare::CREATED_BY_SETUP;
use cobolt_setup::progress::Part;
use cobolt_setup::result::{append, Outcome};
use cobolt_setup::rollback::cache_of;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// The exit code that means "stopped because the cancel file appeared" (the headless step's own `CANCELLED`).
const CANCELLED: u8 = 3;
/// The compile's units, in the order the real step goes through them.
const UNITS: [&str; 3] = ["forms", "forms-sql", "forms-graphics"];

fn say(line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

/// The value after `flag`, if the command line has it.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// `compile_seconds` from the configuration file beside this executable.
fn compile_seconds(beside: &Path) -> u64 {
    std::fs::read_to_string(beside.join("prepare_stub.cfg"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.trim().strip_prefix("compile_seconds=").map(str::to_owned))
        })
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(6)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    // What ran, and with what: the check reads this.
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(beside.join("prepare_stub.log"))
    {
        let _ = writeln!(log, "{}", args.join(" "));
    }

    let (Some(step), Some(result)) = (flag(&args, "--step"), flag(&args, "--result")) else {
        eprintln!("prepare_stub: --step and --result are required");
        return ExitCode::from(2);
    };
    let result = PathBuf::from(result);
    let cancel = flag(&args, "--cancel-file").map(PathBuf::from);

    match step.as_str() {
        "rust" => {
            say("PRC step=rust unit=- crates=0 elapsed=0");
            let _ = append(
                &result,
                &Outcome::ok(Part::Rust, "Rust is already installed (the stand-in step)"),
            );
            ExitCode::SUCCESS
        }
        "compile" => {
            let Some(build_root) = flag(&args, "--build-root").map(PathBuf::from) else {
                eprintln!("prepare_stub: --build-root is required for the compile");
                return ExitCode::from(2);
            };
            // The real compile marks a cache it created, and only that one: this is what a Cancel then removes.
            let cache = cache_of(&build_root);
            let created = !cache.exists();
            if std::fs::create_dir_all(&cache).is_err() {
                eprintln!("prepare_stub: cannot make {}", cache.display());
                return ExitCode::from(2);
            }
            if created {
                let _ = std::fs::write(
                    cache.join(CREATED_BY_SETUP),
                    b"created by the setup (stand-in)\n",
                );
            }
            // Six slices (release and debug, three variants each), of equal length, like the bar's allotment.
            let slice = Duration::from_millis(compile_seconds(&beside) * 1000 / 6);
            let began = Instant::now();
            for part in ["release", "debug"] {
                for unit in UNITS {
                    for crates in [0u32, 10] {
                        say(&format!(
                            "PRC step={part} unit={unit} crates={crates} elapsed={}",
                            began.elapsed().as_secs()
                        ));
                        let until = Instant::now() + slice / 2;
                        while Instant::now() < until {
                            if cancel.as_ref().is_some_and(|c| c.exists()) {
                                say("PRC note=the cancel file appeared; the stand-in stops");
                                return ExitCode::from(CANCELLED);
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            let _ = append(
                &result,
                &Outcome::ok(Part::Release, "Release is compiled (the stand-in step)"),
            );
            let _ = append(
                &result,
                &Outcome::ok(Part::Debug, "Debug is compiled (the stand-in step)"),
            );
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("prepare_stub: --step is rust or compile, not '{other}'");
            ExitCode::from(2)
        }
    }
}
