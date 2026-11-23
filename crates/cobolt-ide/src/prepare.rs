// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Windows setup's headless steps (spec 112).
//!
//! ```text
//! PowerRustCOBOL.exe --prepare-platform --step <rust|compile> --result <file>
//!                    [--version <x.y.z>] [--profiles release,debug] [--sdk <dir>] [--build-root <dir>]
//!                    [--cancel-file <file>]
//! ```
//!
//! The `.msi` runs one step at a time as the installing developer (the per-machine package executes elevated, so
//! an action that is not impersonated would prepare the system account's profile, which is nobody's). A step
//! prints its progress as the lines of [`cobolt_setup::progress`] on standard output, appends what it did to the
//! result file ([`cobolt_setup::result`]) and ends with [`exit::REPORTED`], whether it did its work or reported
//! that it could not: a failure here is never a failed install (R12). Like `--seed-examples`, it is handled before
//! logging and before any window.
//!
//! Everything the steps decide is a plain function over plain values, and everything that touches the machine
//! (rustup, the network, cargo) is behind [`Machine`] or [`plat::CargoRunner`], so what each step answers for each
//! situation is tested with recorded fakes and nothing is downloaded, installed or compiled.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cobolt_compiler::warmup::{self, prepare as plat};
use cobolt_setup::prepare::BUILD_TOOLS_URL;
use cobolt_setup::progress::{self, Line, Part};
use cobolt_setup::result::{self, exit, Failure, Outcome};
use sha2::{Digest, Sha256};

use crate::toolchain::{self, Status};

/// Which step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Install the pinned Rust.
    Rust,
    /// Check the requirements, then compile the platform.
    Compile,
}

/// What a step is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// The step.
    pub step: Step,
    /// The result file the step appends to.
    pub result: PathBuf,
    /// The version whose stamps are written (this executable's own, unless the installer says otherwise).
    pub version: String,
    /// The profiles to compile (`compile` only).
    pub release: bool,
    /// See [`Args::release`].
    pub debug: bool,
    /// The SDK to stage against; found beside the executable when absent.
    pub sdk: Option<PathBuf>,
    /// The build folder; the developer's own setting or the system default when absent.
    pub build_root: Option<PathBuf>,
    /// When this file exists the step stops cleanly and ends with [`exit::CANCELLED`].
    pub cancel_file: Option<PathBuf>,
}

/// Read the arguments after the executable's name. A mistake is a sentence for the installer's log.
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let (mut step, mut result, mut version) = (None, None, None);
    let (mut profiles, mut sdk, mut build_root, mut cancel_file) = (None, None, None, None);
    let mut it = args.iter().peekable();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--prepare-platform" => {}
            "--step" => {
                step = Some(match value("--step")?.as_str() {
                    "rust" => Step::Rust,
                    "compile" => Step::Compile,
                    other => return Err(format!("--step is rust or compile, not '{other}'")),
                })
            }
            "--result" => result = Some(PathBuf::from(value("--result")?)),
            "--version" => version = Some(value("--version")?),
            "--profiles" => profiles = Some(value("--profiles")?),
            "--sdk" => sdk = Some(PathBuf::from(value("--sdk")?)),
            "--build-root" => build_root = Some(PathBuf::from(value("--build-root")?)),
            "--cancel-file" => cancel_file = Some(PathBuf::from(value("--cancel-file")?)),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let (mut release, mut debug) = (true, true);
    if let Some(list) = profiles {
        (release, debug) = (false, false);
        for p in list.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match p {
                "release" => release = true,
                "debug" => debug = true,
                other => return Err(format!("--profiles names release and debug, not '{other}'")),
            }
        }
        if !release && !debug {
            return Err("--profiles names no profile".to_owned());
        }
    }
    Ok(Args {
        step: step.ok_or("--step is required")?,
        result: result.ok_or("--result is required")?,
        version: version.unwrap_or_else(|| crate::version::VERSION.to_owned()),
        release,
        debug,
        sdk,
        build_root,
        cancel_file,
    })
}

/// Whether `path` is inside the system account's profile. A step run by the `.msi` without being impersonated
/// finds itself there, and what it installed or compiled would belong to nobody who will ever use it (D7).
pub fn in_system_profile(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    lower.contains("\\config\\systemprofile\\") || lower.ends_with("\\config\\systemprofile")
}

// ── The check after the installs (R7) ────────────────────────────────────────

/// Whether the requirements are met, judged from what the machine answered: whether `rustup` lists the pinned
/// Rust, and what [`toolchain::with_link_check`] found with that Rust in force. Nothing is reported as there that
/// the check did not confirm; a requirement that is missing or does not work is named, with the next step (R7).
pub fn judge(pinned: &str, listed: bool, status: &Status) -> Result<(), Failure> {
    let rust = format!("Rust {pinned}");
    let again = format!("Run the setup again, or install it yourself with `rustup toolchain install {pinned}`.");
    if !listed {
        return Err(Failure::StillMissing { what: rust, next: again });
    }
    match status {
        Status::Ok { version, .. } if version.to_string() == pinned => Ok(()),
        Status::Ok { version, .. } | Status::TooOld { version, .. } => {
            Err(Failure::StillMissing { what: format!("{rust} (the Rust that answers is {version})"), next: again })
        }
        Status::NoLinker { linker, .. } => Err(Failure::StillMissing {
            what: format!("the C++ linker ({linker})"),
            next: format!(
                "Install the \"Desktop development with C++\" part of the Visual Studio Build Tools ({BUILD_TOOLS_URL}), then run the setup again."
            ),
        }),
        Status::Missing => Err(Failure::StillMissing { what: rust, next: again }),
    }
}

/// `rustup toolchain list`, or `None` when there is no `rustup` to ask.
fn rustup_toolchain_list() -> Option<String> {
    let out = std::process::Command::new("rustup").args(["toolchain", "list"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The check, on this machine, as the IDE itself would make it ([`toolchain::detect`] and [`toolchain::with_link_check`]).
///
/// A Rust that a step ended a moment ago is on the user's `PATH` only in a session that starts afterwards, so it
/// is found where rustup puts it and put on this process's `PATH` for the cargo it starts. The pinned Rust is made
/// the one in force for the link probe too, since the IDE's Builds ask for it by name.
pub fn confirm_requirements(pinned: &str) -> Result<(), Failure> {
    match toolchain::detect() {
        Status::Ok { path, .. } | Status::TooOld { path, .. } | Status::NoLinker { path, .. } => {
            toolchain::ensure_on_path(&path);
        }
        Status::Missing => {}
    }
    let listed = rustup_toolchain_list().is_some_and(|list| warmup::lists_toolchain(&list, pinned));
    if listed {
        std::env::set_var("RUSTUP_TOOLCHAIN", pinned);
    }
    judge(pinned, listed, &toolchain::with_link_check(toolchain::detect()))
}

// ── Rust (R6, R8) ────────────────────────────────────────────────────────────

/// What a step can ask of the machine: the Rust one.
pub trait Machine {
    /// `rustup toolchain list`, or `None` when there is no `rustup`.
    fn rustup_toolchains(&mut self) -> Option<String>;
    /// Download a file whole.
    fn fetch(&mut self, url: &str) -> Result<Vec<u8>, String>;
    /// Keep a downloaded installer where it can be run, and say where.
    fn save(&mut self, name: &str, bytes: &[u8]) -> Result<PathBuf, String>;
    /// Run a program, telling `out` each line it prints (and, now and then, that it is quiet), and answer its exit code.
    fn run(&mut self, program: &str, args: &[&str], out: &mut dyn FnMut(Out<'_>)) -> Result<i32, String>;
}

/// What a running program does that a step can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out<'a> {
    /// A line of its output.
    Line(&'a str),
    /// Nothing for a moment: still running.
    Quiet,
}

/// Where the Rust project publishes the installer, and the checksum it publishes beside it.
fn rustup_init_url(triple: &str) -> String {
    format!("https://static.rust-lang.org/rustup/dist/{triple}/rustup-init.exe")
}

/// The Windows target this executable is, which names the installer to download.
pub fn host_triple() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("x86_64-pc-windows-msvc")
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some("aarch64-pc-windows-msvc")
    } else {
        None
    }
}

/// The checksum in the text the Rust project publishes next to the installer (`<hex> *rustup-init.exe`).
fn published_sha256(text: &str) -> Option<String> {
    let word = text.split_whitespace().next()?;
    (word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit())).then(|| word.to_ascii_lowercase())
}

/// SHA-256 as lower-case hex.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Install the Rust release this version is built with, and leave everything else about the developer's Rust alone.
///
/// * With `rustup` present, `rustup toolchain install <release> --profile minimal`: beside the developer's own,
///   which stays their default (R8).
/// * Without it, `rustup-init.exe` is downloaded from the Rust project, **checked against the SHA-256 the project
///   publishes**, and run with the pinned release as the default. A file that does not match is never run.
///
/// Every failure is a sentence in the result (R12). `triple` is `None` where the step cannot install (not Windows).
pub fn rust_step(m: &mut dyn Machine, release: &str, triple: Option<&str>, say: &mut dyn FnMut(&Line)) -> Outcome {
    let part = Part::Rust;
    let what = format!("Rust {release}");
    let began = Instant::now();
    let at = |say: &mut dyn FnMut(&Line)| {
        say(&Line::At { part, unit: None, crates: 0, elapsed_secs: began.elapsed().as_secs() })
    };
    at(say);

    let listing = m.rustup_toolchains();
    if listing.as_deref().is_some_and(|list| warmup::lists_toolchain(list, release)) {
        return Outcome::ok(part, &format!("{what} is already installed."));
    }
    let Some(triple) = triple else {
        return Outcome::failed(&Failure::Refused {
            part,
            why: "Installing Rust from the setup is only available on Windows.".to_owned(),
        });
    };

    let (program, args): (String, Vec<String>) = if listing.is_some() {
        say(&Line::Note(format!("Installing {what} beside the Rust you already have")));
        (
            "rustup".to_owned(),
            ["toolchain", "install", release, "--profile", "minimal", "--no-self-update"].map(str::to_owned).to_vec(),
        )
    } else {
        let url = rustup_init_url(triple);
        say(&Line::Note("Downloading rustup-init.exe from the Rust project".to_owned()));
        let installer = match m.fetch(&url) {
            Ok(bytes) => bytes,
            Err(_) => {
                return Outcome::failed(&Failure::Download { part, what: "rustup-init.exe".to_owned(), url });
            }
        };
        let published = match m.fetch(&format!("{url}.sha256")) {
            Ok(bytes) => published_sha256(&String::from_utf8_lossy(&bytes)),
            Err(_) => None,
        };
        if published.as_deref() != Some(sha256_hex(&installer).as_str()) {
            return Outcome::failed(&Failure::Checksum { part, what: "rustup-init.exe".to_owned() });
        }
        let path = match m.save("rustup-init.exe", &installer) {
            Ok(path) => path,
            Err(e) => {
                return Outcome::failed(&Failure::Refused {
                    part,
                    why: format!("The Rust installer could not be saved: {e}."),
                });
            }
        };
        say(&Line::Note(format!("Installing {what} with rustup")));
        (
            path.to_string_lossy().into_owned(),
            ["-y", "--default-toolchain", release, "--profile", "minimal"].map(str::to_owned).to_vec(),
        )
    };

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut last = Instant::now();
    let code = m.run(&program, &arg_refs, &mut |out| match out {
        Out::Line(line) => {
            say(&Line::Note(line.to_owned()));
            last = Instant::now();
        }
        Out::Quiet if last.elapsed() >= Duration::from_secs(2) => {
            at(say);
            last = Instant::now();
        }
        Out::Quiet => {}
    });
    match code {
        Ok(0) => {}
        Ok(code) => return Outcome::failed(&Failure::InstallFailed { part, what, code: Some(code) }),
        Err(_) => return Outcome::failed(&Failure::InstallFailed { part, what, code: None }),
    }
    // The installer's exit code is not the proof: ask again.
    if m.rustup_toolchains().is_some_and(|list| warmup::lists_toolchain(&list, release)) {
        Outcome::ok(part, &format!("{what} is installed."))
    } else {
        Outcome::failed(&Failure::StillMissing {
            what,
            next: format!("Install it yourself with `rustup toolchain install {release}` ({}), then run the setup again.", cobolt_setup::prepare::RUSTUP_URL),
        })
    }
}

// ── The platform (R9, R10, R13) ──────────────────────────────────────────────

/// How a profile's end reads in the result file.
fn outcome_of(profile: plat::Profile, end: &plat::End) -> Option<Outcome> {
    let (part, name) = if profile.is_debug() { (Part::Debug, "Debug") } else { (Part::Release, "Release") };
    Some(match end {
        plat::End::NotAsked => return None,
        plat::End::Done => Outcome::ok(part, &format!("PowerRustCOBOL AI's platform is compiled for {name}.")),
        plat::End::Cancelled => Outcome::skipped(part, &format!("The compile for {name} was cancelled.")),
        plat::End::Failed { unit, why } if unit.is_empty() => Outcome::failed(&Failure::Refused {
            part,
            why: format!("The platform could not be prepared for {name}: {}.", complaint(why)),
        }),
        plat::End::Failed { unit, why } => {
            Outcome::failed(&Failure::CompileFailed { part, unit: unit.clone(), detail: complaint(why) })
        }
    })
}

/// The line of a failure that says what went wrong: the last one that starts with `error`, else the last that says anything.
fn complaint(why: &str) -> String {
    let lines: Vec<&str> = why.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .rev()
        .find(|l| l.starts_with("error"))
        .or(lines.last())
        .map(|l| l.trim_end_matches('.').to_owned())
        .unwrap_or_default()
}

/// What the compile step needs besides the arguments.
pub struct Compile<'a> {
    /// What the step was told.
    pub args: &'a Args,
    /// The Rust release this version is built with.
    pub pinned: &'a str,
    /// The SDK the stand-ins are staged against.
    pub sdk: &'a Path,
    /// The developer's build folder.
    pub build_root: &'a Path,
    /// Raised to stop.
    pub cancel: &'a AtomicBool,
}

/// Check the requirements, then compile the platform for the profiles asked for. Returns what to write in the
/// result file and whether the developer's cancel stopped it.
///
/// A requirement that is not met ends the step before any compile, with the sentence of [`judge`], and the profiles
/// are reported as not compiled. Progress is one line per crate at most twice a second, and one for each stand-in
/// that begins (R13).
pub fn compile_step(
    c: &Compile<'_>,
    check: &mut dyn FnMut() -> Result<(), Failure>,
    runner: &mut dyn plat::CargoRunner,
    say: &mut dyn FnMut(&Line),
) -> (Vec<Outcome>, bool) {
    let wanted: Vec<(plat::Profile, Part)> = [(plat::Profile::Release, Part::Release), (plat::Profile::Debug, Part::Debug)]
        .into_iter()
        .filter(|(p, _)| if p.is_debug() { c.args.debug } else { c.args.release })
        .collect();

    say(&Line::At { part: Part::Check, unit: None, crates: 0, elapsed_secs: 0 });
    if let Err(failure) = check() {
        let mut out = vec![Outcome::failed(&failure)];
        out.extend(wanted.iter().map(|(_, part)| {
            Outcome::skipped(*part, &format!("The platform was not compiled for {}: a requirement is missing.", if *part == Part::Debug { "Debug" } else { "Release" }))
        }));
        return (out, false);
    }

    let prepare = plat::Prepare {
        sdk_root: c.sdk,
        build_root: c.build_root,
        version: &c.args.version,
        release: c.args.release,
        debug: c.args.debug,
        cancel: c.cancel,
        heartbeat: plat::Prepare::HEARTBEAT,
    };
    // Both callbacks speak through the same `say`, one at a time: a cell lets the two closures share it.
    let say = std::cell::RefCell::new(say);
    let mut told: Option<Instant> = None;
    let report = plat::prepare_platform_with(
        runner,
        &prepare,
        &mut |g| {
            let part = if g.profile.is_debug() { Part::Debug } else { Part::Release };
            let line = Line::At { part, unit: Some(g.variant.to_owned()), crates: g.crates, elapsed_secs: g.elapsed.as_secs() };
            let due = told.is_none_or(|at| at.elapsed() >= Duration::from_millis(500));
            match g.event {
                // A stand-in beginning is always said; the rest at most twice a second.
                plat::Event::Began => (say.borrow_mut())(&line),
                plat::Event::Waiting => {
                    (say.borrow_mut())(&Line::Note("Waiting for another build to finish with the compiled-crate cache".to_owned()));
                    told = Some(Instant::now());
                }
                _ if due => {
                    (say.borrow_mut())(&line);
                    told = Some(Instant::now());
                }
                _ => {}
            }
        },
        &mut |l| (say.borrow_mut())(&Line::Note(l.trim().to_owned())),
    );
    let outcomes = report.ends.iter().filter_map(|(p, e)| outcome_of(*p, e)).collect();
    (outcomes, report.cancelled())
}

// ── The real machine ─────────────────────────────────────────────────────────

/// This computer.
pub struct Here;

impl Here {
    /// `rustup`, wherever rustup keeps it: on the `PATH`, or in the user's `.cargo\bin`.
    fn rustup_program() -> PathBuf {
        let name = if cfg!(windows) { "rustup.exe" } else { "rustup" };
        if std::process::Command::new("rustup").arg("--version").output().is_ok_and(|o| o.status.success()) {
            return PathBuf::from("rustup");
        }
        dirs::home_dir().map_or_else(|| PathBuf::from("rustup"), |h| h.join(".cargo").join("bin").join(name))
    }
}

impl Machine for Here {
    fn rustup_toolchains(&mut self) -> Option<String> {
        let out = std::process::Command::new(Self::rustup_program()).args(["toolchain", "list"]).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn fetch(&mut self, url: &str) -> Result<Vec<u8>, String> {
        // The IDE's own HTTP client (as `update_check`): ureq with the system's TLS.
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(300))
            .user_agent(&format!("PowerRustCOBOL/{}", crate::version::VERSION));
        if let Ok(c) = native_tls::TlsConnector::new() {
            builder = builder.tls_connector(std::sync::Arc::new(c));
        }
        let response = builder.build().get(url).call().map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    fn save(&mut self, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join(format!("powerrustcobol-setup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(path)
    }

    fn run(&mut self, program: &str, args: &[&str], out: &mut dyn FnMut(Out<'_>)) -> Result<i32, String> {
        // No `CREATE_NO_WINDOW` here: this executable is a console-subsystem program (it has no `windows_subsystem`
        // attribute, and the setup starts it with `CREATE_NO_WINDOW` itself), so the console its children inherit is
        // the hidden one and none of them opens a window. If the IDE ever becomes a windowed program this must change.
        let program = if program == "rustup" { Self::rustup_program() } else { PathBuf::from(program) };
        let mut child = std::process::Command::new(&program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let mut readers = Vec::new();
        for stream in [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            readers.push(std::thread::spawn(move || {
                use std::io::BufRead as _;
                for line in std::io::BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(tx);
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(line) => out(Out::Line(&line)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => out(Out::Quiet),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        for reader in readers {
            let _ = reader.join();
        }
        child.wait().map(|s| s.code().unwrap_or(-1)).map_err(|e| e.to_string())
    }
}

/// Append outcomes to the result file, creating it and its folder when need be.
pub fn write_outcomes(path: &Path, outcomes: &[Outcome]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    for outcome in outcomes {
        write!(file, "{}\r\n", result::format(outcome))?;
    }
    Ok(())
}

/// Print a progress line and let whoever reads the pipe have it at once.
fn print_line(line: &Line) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", progress::format(line));
    let _ = out.flush();
}

/// Run the step the arguments name, on this machine. The process's exit code.
pub fn run(args: &[String]) -> i32 {
    let args = match parse_args(args) {
        Ok(args) => args,
        Err(why) => {
            eprintln!("PowerRustCOBOL: --prepare-platform: {why}");
            return exit::BAD_ARGUMENTS;
        }
    };
    let part = if args.step == Step::Rust { Part::Rust } else { Part::Release };

    // The system account's profile is nobody's: say so and do nothing (D7).
    let home = dirs::home_dir().unwrap_or_default();
    if in_system_profile(&home) {
        let why = "This step ran as the system account, whose profile is nobody's, so it did nothing.".to_owned();
        return finish(&args, vec![Outcome::failed(&Failure::Refused { part, why })], false);
    }

    let pinned = warmup::release_rust();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    if let Some(file) = args.cancel_file.clone() {
        let flag = cancel.clone();
        std::thread::spawn(move || loop {
            if file.exists() {
                flag.store(true, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        });
    }
    let mut say = |line: &Line| print_line(line);

    match args.step {
        Step::Rust => {
            let outcome = rust_step(&mut Here, pinned, host_triple(), &mut say);
            finish(&args, vec![outcome], false)
        }
        Step::Compile => {
            // What the installer names wins; then the developer's own setting; then the system default.
            let configured = args.build_root.clone().or_else(crate::ui_prefs::load_build_dir);
            let (build_root, _) = cobolt_compiler::resolve_build_root(configured.as_deref());
            if in_system_profile(&build_root) {
                let why = format!("The build folder {} is in the system account's profile, which is nobody's, so nothing was compiled.", build_root.display());
                return finish(&args, vec![Outcome::failed(&Failure::Refused { part, why })], false);
            }
            let Some(sdk) = cobolt_compiler::resolve_workspace_root(args.sdk.clone()) else {
                let why = "The platform's SDK was not found beside PowerRustCOBOL.exe, so nothing was compiled.".to_owned();
                return finish(&args, vec![Outcome::failed(&Failure::Refused { part, why })], false);
            };
            let compile = Compile { args: &args, pinned, sdk: &sdk, build_root: &build_root, cancel: &cancel };
            let (outcomes, cancelled) =
                compile_step(&compile, &mut || confirm_requirements(pinned), &mut plat::SystemCargo, &mut say);
            finish(&args, outcomes, cancelled)
        }
    }
}

/// Write the result file and answer the exit code.
fn finish(args: &Args, outcomes: Vec<Outcome>, cancelled: bool) -> i32 {
    if let Err(e) = write_outcomes(&args.result, &outcomes) {
        eprintln!("PowerRustCOBOL: could not write the result file {}: {e}", args.result.display());
    }
    for outcome in &outcomes {
        println!("{}", result::format(outcome));
    }
    if cancelled {
        exit::CANCELLED
    } else {
        exit::REPORTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::warmup::prepare::{CargoBuild, CargoRunner, Ended, Flow, Tick};
    use cobolt_setup::result::Status as Ran;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_arguments_are_read_and_a_mistake_is_a_sentence() {
        let a = parse_args(&args(&["--prepare-platform", "--step", "compile", "--result", "C:\\t\\r.txt"])).unwrap();
        assert_eq!(a.step, Step::Compile);
        assert_eq!(a.result, PathBuf::from("C:\\t\\r.txt"));
        assert_eq!(a.version, crate::version::VERSION);
        assert!(a.release && a.debug && a.sdk.is_none() && a.build_root.is_none() && a.cancel_file.is_none());

        let a = parse_args(&args(&[
            "--prepare-platform", "--step", "rust", "--result", "r", "--version", "9.9.9", "--profiles", "debug", "--sdk", "S", "--build-root", "B", "--cancel-file", "C",
        ]))
        .unwrap();
        assert_eq!((a.step, a.version.as_str(), a.release, a.debug), (Step::Rust, "9.9.9", false, true));
        assert_eq!((a.sdk, a.build_root, a.cancel_file), (Some("S".into()), Some("B".into()), Some("C".into())));

        let mistakes = [
            (vec!["--prepare-platform"], "--step is required"),
            (vec!["--prepare-platform", "--step", "compile"], "--result is required"),
            (vec!["--prepare-platform", "--step", "nowhere", "--result", "r"], "--step is rust or compile, not 'nowhere'"),
            (vec!["--prepare-platform", "--step"], "--step needs a value"),
            (vec!["--prepare-platform", "--step", "rust", "--result", "r", "--profiles", "fast"], "not 'fast'"),
            (vec!["--prepare-platform", "--step", "rust", "--result", "r", "--profiles", ","], "names no profile"),
            (vec!["--prepare-platform", "--step", "rust", "--result", "r", "--now"], "unknown argument '--now'"),
        ];
        for (given, said) in &mistakes {
            let why = parse_args(&args(given)).unwrap_err();
            assert!(why.contains(said), "{given:?}: {why}");
        }
        println!("--prepare-platform arguments: 2 good lines and {} mistakes, each a sentence", mistakes.len());
    }

    #[test]
    fn the_system_account_profile_is_recognised_and_nothing_else_is() {
        for (path, yes) in [
            ("C:\\Windows\\System32\\config\\systemprofile\\AppData\\Local\\PowerRustCOBOL\\builds", true),
            ("c:\\windows\\system32\\CONFIG\\SystemProfile", true),
            ("C:/Windows/System32/config/systemprofile/.cargo", true),
            ("C:\\Users\\dev\\AppData\\Local\\PowerRustCOBOL\\builds", false),
            ("C:\\Users\\config\\systemprofiles\\x", false),
            ("D:\\builds", false),
            ("", false),
        ] {
            assert_eq!(in_system_profile(Path::new(path)), yes, "{path}");
        }
    }

    #[test]
    fn the_check_names_what_is_missing_and_the_next_step_and_confirms_only_what_it_saw() {
        use crate::toolchain::RustVersion;
        let v = |major, minor, patch| RustVersion { major, minor, patch };
        let path = PathBuf::from("rustc");
        let ok = |version| Status::Ok { path: path.clone(), version };
        let cases: Vec<(bool, Status, Option<&str>)> = vec![
            (true, ok(v(1, 95, 0)), None),
            (false, ok(v(1, 95, 0)), Some("Rust 1.95.0")),
            (true, ok(v(1, 96, 1)), Some("the Rust that answers is 1.96.1")),
            (true, Status::TooOld { path: path.clone(), version: v(1, 80, 0) }, Some("the Rust that answers is 1.80.0")),
            (true, Status::NoLinker { path: path.clone(), version: v(1, 95, 0), linker: "link.exe".into() }, Some("the C++ linker (link.exe)")),
            (true, Status::Missing, Some("Rust 1.95.0")),
            (false, Status::Missing, Some("Rust 1.95.0")),
        ];
        for (listed, status, names) in &cases {
            match (judge("1.95.0", *listed, status), names) {
                (Ok(()), None) => {}
                (Err(Failure::StillMissing { what, next }), Some(needle)) => {
                    assert!(what.contains(needle), "{what}");
                    assert!(!next.is_empty() && (next.contains("run the setup again") || next.contains("Run the setup again")), "{next}");
                    if status.is_usable() || matches!(status, Status::NoLinker { .. }) {
                        // a named step, not a bare complaint
                        assert!(next.contains("rustup toolchain install") || next.contains(BUILD_TOOLS_URL), "{next}");
                    }
                }
                (other, expected) => panic!("{listed} {status:?}: {other:?} but expected {expected:?}"),
            }
        }
        // The sentence a developer reads names the Build Tools page.
        let Err(f) = judge("1.95.0", true, &Status::NoLinker { path, version: v(1, 95, 0), linker: "link.exe".into() }) else { panic!() };
        assert!(Outcome::failed(&f).text.contains("visual-cpp-build-tools"), "{}", Outcome::failed(&f).text);
        println!("requirement check: {} situations, each confirmed only when seen, each failure names it and the next step", cases.len() + 1);
    }

    // ── the Rust step, with a fake machine ──

    #[derive(Default)]
    struct Fake {
        /// What `rustup toolchain list` says before the install, and after it.
        before: Option<String>,
        after: Option<String>,
        installed: bool,
        files: std::collections::HashMap<String, Result<Vec<u8>, String>>,
        ran: Vec<(String, Vec<String>)>,
        fetched: Vec<String>,
        saved: usize,
        exit: i32,
        chatty: bool,
    }

    impl Machine for Fake {
        fn rustup_toolchains(&mut self) -> Option<String> {
            if self.installed { self.after.clone() } else { self.before.clone() }
        }
        fn fetch(&mut self, url: &str) -> Result<Vec<u8>, String> {
            self.fetched.push(url.to_owned());
            self.files.get(url).cloned().unwrap_or_else(|| Err("no network".to_owned()))
        }
        fn save(&mut self, name: &str, _: &[u8]) -> Result<PathBuf, String> {
            self.saved += 1;
            Ok(PathBuf::from(format!("C:\\Temp\\{name}")))
        }
        fn run(&mut self, program: &str, args: &[&str], out: &mut dyn FnMut(Out<'_>)) -> Result<i32, String> {
            self.ran.push((program.to_owned(), args.iter().map(|a| (*a).to_owned()).collect()));
            if self.chatty {
                out(Out::Line("info: downloading component 'rustc'"));
                out(Out::Quiet);
            }
            if self.exit == 0 {
                self.installed = true;
            }
            Ok(self.exit)
        }
    }

    const PINNED: &str = "1.95.0";
    const TRIPLE: &str = "x86_64-pc-windows-msvc";

    fn with_files(installer: &[u8], published: &str) -> Fake {
        let mut f = Fake { after: Some("1.95.0-x86_64-pc-windows-msvc (default)\n".into()), ..Fake::default() };
        let url = rustup_init_url(TRIPLE);
        f.files.insert(format!("{url}.sha256"), Ok(published.as_bytes().to_vec()));
        f.files.insert(url, Ok(installer.to_vec()));
        f
    }

    fn step(m: &mut Fake, triple: Option<&str>) -> (Outcome, Vec<String>) {
        let mut said = Vec::new();
        let outcome = rust_step(m, PINNED, triple, &mut |l| said.push(progress::format(l)));
        (outcome, said)
    }

    #[test]
    fn rust_already_there_is_not_installed_again() {
        let mut m = Fake { before: Some("stable-x86_64-pc-windows-msvc (default)\n1.95.0-x86_64-pc-windows-msvc\n".into()), ..Fake::default() };
        let (outcome, _) = step(&mut m, Some(TRIPLE));
        assert_eq!((outcome.status, outcome.text.as_str()), (Ran::Ok, "Rust 1.95.0 is already installed."));
        assert!(m.ran.is_empty() && m.fetched.is_empty(), "nothing is run or downloaded");
    }

    #[test]
    fn with_rustup_the_release_is_installed_beside_the_developers_and_the_default_is_never_named() {
        let mut m = Fake {
            before: Some("stable-x86_64-pc-windows-msvc (default)\n".into()),
            after: Some("stable-x86_64-pc-windows-msvc (default)\n1.95.0-x86_64-pc-windows-msvc\n".into()),
            chatty: true,
            ..Fake::default()
        };
        let (outcome, said) = step(&mut m, Some(TRIPLE));
        assert_eq!(outcome.status, Ran::Ok, "{}", outcome.text);
        assert_eq!(m.ran.len(), 1);
        let (program, args) = &m.ran[0];
        assert_eq!(program, "rustup");
        assert_eq!(args.join(" "), "toolchain install 1.95.0 --profile minimal --no-self-update");
        assert!(!args.iter().any(|a| a == "default" || a == "--default-toolchain" || a == "-y"), "{args:?}");
        assert!(m.fetched.is_empty(), "nothing downloaded here: rustup does it");
        assert!(said.iter().any(|l| l.contains("beside the Rust you already have")), "{said:?}");
        assert!(said.iter().any(|l| l.contains("info: downloading component")), "the installer's own lines are shown");
        assert!(said.first().is_some_and(|l| l.starts_with("PRC step=rust")), "{said:?}");
    }

    #[test]
    fn without_rustup_the_installer_is_downloaded_checked_and_run_with_the_release_as_the_default() {
        let installer = b"MZ pretend rustup-init".as_slice();
        let hash = sha256_hex(installer);
        let mut m = with_files(installer, &format!("{hash} *rustup-init.exe\n"));
        let (outcome, said) = step(&mut m, Some(TRIPLE));
        assert_eq!(outcome.status, Ran::Ok, "{}", outcome.text);
        let url = rustup_init_url(TRIPLE);
        assert_eq!(url, "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe");
        assert_eq!(m.fetched, [url.clone(), format!("{url}.sha256")]);
        assert_eq!(m.saved, 1);
        assert_eq!(m.ran.len(), 1);
        assert_eq!(m.ran[0].0, "C:\\Temp\\rustup-init.exe");
        assert_eq!(m.ran[0].1.join(" "), "-y --default-toolchain 1.95.0 --profile minimal");
        assert!(said.iter().any(|l| l.contains("Downloading rustup-init.exe from the Rust project")));
    }

    #[test]
    fn an_installer_that_does_not_match_the_published_checksum_is_never_run() {
        let installer = b"MZ pretend rustup-init".as_slice();
        for published in [
            "0000000000000000000000000000000000000000000000000000000000000000 *rustup-init.exe".to_owned(),
            "not a checksum".to_owned(),
            String::new(),
        ] {
            let mut m = with_files(installer, &published);
            let (outcome, _) = step(&mut m, Some(TRIPLE));
            assert_eq!(outcome.status, Ran::Failed, "{published:?}");
            assert!(outcome.text.contains("did not match the checksum") && outcome.text.contains("was not run"), "{}", outcome.text);
            assert!(m.ran.is_empty() && m.saved == 0, "a file that does not match is not even kept");
        }
        // The checksum that was published for a different file does not do either.
        let mut m = with_files(installer, &format!("{} *rustup-init.exe", sha256_hex(b"another file")));
        assert_eq!(step(&mut m, Some(TRIPLE)).0.status, Ran::Failed);
        // Upper-case hex, as some mirrors write it, is the same number.
        let mut m = with_files(installer, &format!("{} *rustup-init.exe", sha256_hex(installer).to_ascii_uppercase()));
        assert_eq!(step(&mut m, Some(TRIPLE)).0.status, Ran::Ok);
    }

    #[test]
    fn no_network_a_failing_installer_and_a_rust_that_did_not_appear_are_each_a_sentence() {
        // No network: neither file arrives.
        let mut m = Fake::default();
        let (outcome, _) = step(&mut m, Some(TRIPLE));
        assert_eq!(outcome.status, Ran::Failed);
        assert!(outcome.text.contains("could not be downloaded from https://static.rust-lang.org/"), "{}", outcome.text);
        assert!(m.ran.is_empty());

        // The installer fails.
        let installer = b"x".as_slice();
        let mut m = with_files(installer, &sha256_hex(installer));
        m.exit = 101;
        let (outcome, _) = step(&mut m, Some(TRIPLE));
        assert!(outcome.text.contains("did not install (its installer ended with code 101)"), "{}", outcome.text);

        // The installer says all is well and the toolchain still is not listed: the exit code is not the proof.
        let mut m = with_files(installer, &sha256_hex(installer));
        m.after = Some("stable-x86_64-pc-windows-msvc (default)\n".into());
        let (outcome, _) = step(&mut m, Some(TRIPLE));
        assert_eq!(outcome.status, Ran::Failed);
        assert!(outcome.text.contains("still missing or does not work") && outcome.text.contains("rustup toolchain install 1.95.0"), "{}", outcome.text);

        // Not Windows: the step says so and runs nothing.
        let mut m = Fake::default();
        let (outcome, _) = step(&mut m, None);
        assert!(outcome.text.contains("only available on Windows"), "{}", outcome.text);
        assert!(m.fetched.is_empty() && m.ran.is_empty());
    }

    #[test]
    fn the_install_argv_and_the_pinned_release_cannot_drift_from_the_one_the_ide_shows() {
        // The IDE shows `rust_install_command()` for a developer to run; the setup runs its own argv. Both name the same release.
        assert_eq!(PINNED, warmup::release_rust(), "the tests pin the release this version is built with");
        assert!(toolchain::install_command().ends_with(&format!("-y --default-toolchain {PINNED}")), "{}", toolchain::install_command());
    }

    // ── the compile step, with a fake cargo ──

    struct Building;

    impl CargoRunner for Building {
        fn build(&mut self, _: &CargoBuild<'_>, on: &mut dyn FnMut(Tick) -> Flow) -> Ended {
            for n in 0..3 {
                if on(Tick::Compiling(format!("crate{n}"))) == Flow::Stop {
                    return Ended::Stopped;
                }
            }
            Ended::Success
        }
    }

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("prc-ide-prepare-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("sdk")).unwrap();
            Scratch(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn compile(s: &Scratch, profiles: &[&str], check: Result<(), Failure>, runner: &mut dyn CargoRunner, cancel: &AtomicBool) -> (Vec<Outcome>, bool, Vec<String>) {
        let mut list = vec!["--prepare-platform", "--step", "compile", "--result", "r", "--version", "9.9.9"];
        let joined = profiles.join(",");
        if !profiles.is_empty() {
            list.extend(["--profiles", joined.as_str()]);
        }
        let a = parse_args(&args(&list)).unwrap();
        let (sdk, builds) = (s.0.join("sdk"), s.0.join("builds"));
        let c = Compile { args: &a, pinned: PINNED, sdk: &sdk, build_root: &builds, cancel };
        let mut said = Vec::new();
        let (outcomes, cancelled) = compile_step(&c, &mut || check.clone(), runner, &mut |l| said.push(progress::format(l)));
        (outcomes, cancelled, said)
    }

    #[test]
    fn a_missing_requirement_ends_the_step_before_any_compile_and_says_what_was_not_done() {
        let s = Scratch::new("missing");
        let failure = Failure::StillMissing { what: "the C++ linker (link.exe)".into(), next: "Install the Build Tools, then run the setup again.".into() };
        struct Never;
        impl CargoRunner for Never {
            fn build(&mut self, _: &CargoBuild<'_>, _: &mut dyn FnMut(Tick) -> Flow) -> Ended {
                panic!("nothing may compile when a requirement is missing")
            }
        }
        let (outcomes, cancelled, said) = compile(&s, &[], Err(failure), &mut Never, &AtomicBool::new(false));
        assert!(!cancelled);
        assert_eq!(outcomes.iter().map(|o| (o.part, o.status)).collect::<Vec<_>>(), [(Part::Check, Ran::Failed), (Part::Release, Ran::Skipped), (Part::Debug, Ran::Skipped)]);
        assert!(outcomes[0].text.contains("link.exe") && outcomes[1].text.contains("a requirement is missing"));
        assert!(!s.0.join("builds").exists(), "no cache is created for a compile that cannot start");
        assert_eq!(said.len(), 1, "{said:?}");
    }

    #[test]
    fn the_compile_reports_each_profile_with_progress_lines_and_the_stamps_follow() {
        let s = Scratch::new("done");
        let (outcomes, cancelled, said) = compile(&s, &[], Ok(()), &mut Building, &AtomicBool::new(false));
        assert!(!cancelled);
        assert_eq!(outcomes.iter().map(|o| (o.part, o.status)).collect::<Vec<_>>(), [(Part::Release, Ran::Ok), (Part::Debug, Ran::Ok)]);
        assert_eq!(outcomes[0].text, "PowerRustCOBOL AI's platform is compiled for Release.");
        let cache = s.0.join("builds").join("shared-target").join("warm");
        assert!(cache.join("9.9.9").is_file() && cache.join("9.9.9-debug").is_file());
        // Every line is of the grammar, the stand-ins begin with their own line, and the counts run on.
        let parsed: Vec<Line> = said.iter().map(|l| progress::parse(l).unwrap_or_else(|| panic!("not a progress line: {l}"))).collect();
        let units: std::collections::BTreeSet<(String, String)> = parsed
            .iter()
            .filter_map(|l| match l {
                Line::At { part, unit: Some(u), .. } if *part != Part::Check => Some((part.name().to_owned(), u.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(units.len(), 6, "each of the 3 stand-ins in each of the 2 profiles says where it is: {said:?}");
        // The count runs on across the stand-ins of a profile, and a burst of crates is said at most twice a second:
        // the fake cargo prints 18 crates at once and the lines for them are fewer than that.
        let at_lines = parsed.iter().filter(|l| matches!(l, Line::At { unit: Some(_), .. })).count();
        assert!(at_lines < 6 + 18, "{at_lines} lines for 18 crates and 6 stand-ins: {said:?}");
        assert!(matches!(parsed[0], Line::At { part: Part::Check, .. }));

        // Only the profile asked for.
        let s = Scratch::new("debugonly");
        let (outcomes, ..) = compile(&s, &["debug"], Ok(()), &mut Building, &AtomicBool::new(false));
        assert_eq!(outcomes.iter().map(|o| o.part).collect::<Vec<_>>(), [Part::Debug]);
        println!("compile step with a recorded cargo: both profiles stamped, {} progress lines of the grammar, one profile when one is asked", said.len());
    }

    #[test]
    fn a_failed_unit_is_one_sentence_naming_it_and_a_cancel_is_a_cancel() {
        let s = Scratch::new("fails");
        struct Fails;
        impl CargoRunner for Fails {
            fn build(&mut self, r: &CargoBuild<'_>, _: &mut dyn FnMut(Tick) -> Flow) -> Ended {
                if r.profile.is_debug() && r.dir.ends_with("cobolt-build-_warmup-forms-sql") {
                    Ended::Failed { code: Some(101), tail: vec!["   Compiling rusqlite v0.37.0".into(), "error: could not compile `rusqlite` (lib) due to 1 previous error".into(), "".into()] }
                } else {
                    Ended::Success
                }
            }
        }
        let (outcomes, cancelled, _) = compile(&s, &[], Ok(()), &mut Fails, &AtomicBool::new(false));
        assert!(!cancelled);
        assert_eq!(outcomes[0].status, Ran::Ok);
        assert_eq!(outcomes[1].status, Ran::Failed);
        assert!(outcomes[1].text.contains("The forms-sql part of PowerRustCOBOL AI did not compile for Debug: error: could not compile `rusqlite`"), "{}", outcomes[1].text);

        let s = Scratch::new("cancel");
        let (outcomes, cancelled, _) = compile(&s, &[], Ok(()), &mut Building, &AtomicBool::new(true));
        assert!(cancelled);
        assert!(outcomes.iter().all(|o| o.status == Ran::Skipped && o.text.contains("cancelled")), "{outcomes:?}");
        assert!(!s.0.join("builds").join("shared-target").join("warm").join("9.9.9").exists(), "a cancel writes no stamp");
    }

    #[test]
    fn the_result_file_collects_every_step_and_reads_back() {
        let s = Scratch::new("file");
        let path = s.0.join("deep").join("r.txt");
        write_outcomes(&path, &[Outcome::ok(Part::Rust, "Rust 1.95.0 is installed.")]).unwrap();
        write_outcomes(&path, &[Outcome::skipped(Part::Release, "later"), Outcome::failed(&Failure::TimedOut { part: Part::Debug, minutes: 30 })]).unwrap();
        let read = result::read(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(read.iter().map(|o| (o.part, o.status)).collect::<Vec<_>>(), [(Part::Rust, Ran::Ok), (Part::Release, Ran::Skipped), (Part::Debug, Ran::Failed)]);
    }

    #[test]
    fn the_checksum_text_is_read_the_way_the_rust_project_writes_it() {
        let h = sha256_hex(b"x");
        assert_eq!(published_sha256(&format!("{h} *rustup-init.exe\n")), Some(h.clone()));
        assert_eq!(published_sha256(&h), Some(h.clone()));
        assert_eq!(published_sha256(&format!("{}  f", h.to_ascii_uppercase())), Some(h));
        for bad in ["", "abc", "zz", &"g".repeat(64), &"a".repeat(63), &"a".repeat(65)] {
            assert_eq!(published_sha256(bad), None, "{bad}");
        }
        // The one number everyone knows: SHA-256 of nothing.
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn what_the_setup_asks_of_the_machine_is_what_the_compile_leaves_there() {
        use cobolt_setup::facts;
        // The setup's page is decided before any file of this program is installed, so it spells three things of
        // the compiler's by itself. Each is held equal here.

        // 1. A pinned Rust is found in a toolchain list the way the compiler finds it.
        let lists = [
            "stable-aarch64-apple-darwin (default)\n1.95.0-aarch64-apple-darwin\n",
            "stable-x86_64-pc-windows-msvc (default)\r\n1.92-x86_64-pc-windows-msvc\r\n",
            "1.95.01-x86_64-pc-windows-msvc\n",
            "no installed toolchains\n",
            "",
            "  1.95.0-x86_64-pc-windows-msvc  \n",
        ];
        for list in lists {
            assert_eq!(facts::lists_toolchain(list, PINNED), warmup::lists_toolchain(list, PINNED), "{list:?}");
        }

        // 2. The default build folder ends the way the compiler's does.
        let root = cobolt_compiler::default_build_root();
        let tail: PathBuf = facts::BUILD_FOLDER.iter().collect();
        assert!(root.ends_with(&tail), "{} does not end in {}", root.display(), tail.display());

        // 3. The stamps a compile writes are found where, and as, the setup looks for them.
        let s = Scratch::new("stamps");
        let (outcomes, ..) = compile(&s, &[], Ok(()), &mut Building, &AtomicBool::new(false));
        assert!(outcomes.iter().all(|o| o.status == Ran::Ok));
        let folder: PathBuf = std::iter::once(s.0.join("builds"))
            .chain(facts::STAMP_FOLDER.iter().map(PathBuf::from))
            .collect();
        let names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .collect();
        assert_eq!(facts::stamps("9.9.9", &names), (true, true), "{names:?}");
        assert_eq!(facts::stamps("9.9.8", &names), (false, false));
        println!("setup facts vs compiler: {} toolchain lists agree, build folder ends in {}, stamps {names:?} read back as (release, debug)", lists.len(), tail.display());
    }

    #[test]
    fn the_marker_the_setup_looks_for_is_the_one_the_compile_writes() {
        // The DLL removes a cancelled setup's cache by this name without linking the compiler, so the two spell it alike.
        assert_eq!(cobolt_setup::prepare::CREATED_BY_SETUP, plat::CREATED_BY_SETUP);
    }
}
