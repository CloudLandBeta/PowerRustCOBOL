// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The child the spike's custom actions start (spec 112, task T1).
//!
//! It stands in for `PowerRustCOBOL.exe --prepare-platform`: it prints lines the
//! way that mode will, so the supervisor has something to read, and it answers
//! the questions the spike exists to ask.
//!
//! ```text
//! spike_child env               who am I, where, with what environment
//! spike_child beat <seconds>    one line a second for that long (a step with no count)
//! spike_child run <prog> <args> run a program, say what it exits with (the nested installs)
//! ```

use std::io::Write;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

fn say(line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn env_line(name: &str) {
    match std::env::var(name) {
        Ok(v) if name == "PATH" => say(&format!(
            "env {name} = ({} entries) {}",
            v.split(';').count(),
            v.chars().take(160).collect::<String>()
        )),
        Ok(v) => say(&format!("env {name} = {v}")),
        Err(_) => say(&format!("env {name} is not set")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("env") => {
            #[cfg(windows)]
            say(&format!("identity: {}", cobolt_setup::ident::describe()));
            #[cfg(not(windows))]
            say("identity: (not Windows)");
            for name in [
                "USERNAME",
                "USERDOMAIN",
                "USERPROFILE",
                "LOCALAPPDATA",
                "APPDATA",
                "TEMP",
                "CARGO_HOME",
                "RUSTUP_HOME",
                "SystemDrive",
                "PATH",
            ] {
                env_line(name);
            }
            ExitCode::SUCCESS
        }
        Some("beat") => {
            let seconds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
            let begun = Instant::now();
            for n in 1..=seconds {
                std::thread::sleep(Duration::from_secs(1));
                say(&format!(
                    "beat {n}/{seconds} after {:.0}s",
                    begun.elapsed().as_secs_f64()
                ));
            }
            ExitCode::SUCCESS
        }
        // The ways a real step ends that its own result file does not say (task T12): it exits with an error,
        // it goes quiet, it prints the progress lines of the real step.
        Some("exit") => {
            let code: u8 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            say(&format!("exit: ending with code {code}"));
            ExitCode::from(code)
        }
        Some("silent") => {
            let seconds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
            std::thread::sleep(Duration::from_secs(seconds));
            ExitCode::SUCCESS
        }
        Some("lines") => {
            let begun = Instant::now();
            for part in ["release", "debug"] {
                for unit in ["forms", "forms-sql", "forms-graphics"] {
                    for crates in [0, 10] {
                        std::thread::sleep(Duration::from_millis(600));
                        println!(
                            "PRC step={part} unit={unit} crates={crates} elapsed={}",
                            begun.elapsed().as_secs()
                        );
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Some("run") if args.len() >= 2 => {
            say(&format!("run: {}", args[1..].join(" ")));
            let begun = Instant::now();
            // The child's output is ours: it inherits the pipe the supervisor reads.
            match Command::new(&args[1]).args(&args[2..]).status() {
                Ok(status) => {
                    let code = status
                        .code()
                        .map_or_else(|| "none".to_owned(), |c| c.to_string());
                    say(&format!(
                        "run: exit code {code} after {:.1}s",
                        begun.elapsed().as_secs_f64()
                    ));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    say(&format!("run: could not start '{}': {e}", args[1]));
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            say("usage: spike_child env | beat <seconds> | exit <code> | silent <seconds> | lines | run <program> [args...]");
            ExitCode::from(64)
        }
    }
}
