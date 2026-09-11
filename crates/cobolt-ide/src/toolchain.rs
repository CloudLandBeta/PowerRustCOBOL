// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Is Rust installed, and is it new enough to build a project?
//!
//! PowerRustCOBOL designs forms and *runs* programs entirely on its own: the
//! interpreter is linked into the IDE. **Build** is different — it compiles the
//! program through `cargo`, and so does any *Run* of a program containing an
//! `EXEC RUST` block. On a machine without Rust those are the only two things
//! that fail, and today they fail late: the developer installs the IDE, spends
//! an afternoon on a form, presses Build, and only then reads a toolchain error.
//!
//! So the IDE asks the question once, on its first run, and gets it out of the
//! way while the answer is still cheap.
//!
//! # Why this never troubles a developer running `cargo run`
//!
//! Nothing is shown when Rust is present and recent enough — and a developer
//! who started the IDE with `cargo run` self-evidently has it. The check
//! therefore needs no "am I the packaged app?" test: the packaged first run is
//! simply the only situation in which the answer can be *no*.
//!
//! # Why PATH alone is not enough
//!
//! A desktop app launched from Finder or Explorer inherits the session's PATH,
//! not the shell's — and `~/.cargo/bin` is put there by the shell profile that
//! rustup edits. Asking only PATH would tell somebody who installed Rust
//! yesterday that they have not, which is worse than never asking. Every
//! candidate in [`candidates`] is probed, and when the one that answers lives
//! outside PATH, [`ensure_on_path`] puts its directory there for the child
//! processes this IDE starts, so Build works in the session that found it.
//!
//! # The minimum
//!
//! Read from the workspace manifest at compile time, so the version this file
//! enforces is the one the build actually requires and cannot drift from it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

/// The workspace manifest, embedded at compile time — see the module note on
/// the minimum.
const WORKSPACE_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));

/// Used only if the manifest ever stops declaring `rust-version`. A unit test
/// asserts that it does, so this value is a floor and not a second opinion.
const FALLBACK_MINIMUM: RustVersion = RustVersion {
    major: 1,
    minor: 92,
    patch: 0,
};

#[cfg(windows)]
const RUSTC: &str = "rustc.exe";
#[cfg(not(windows))]
const RUSTC: &str = "rustc";

/// The official one-shot installer, exactly as rustup.rs publishes it. It is
/// both what the dialog shows and what [`install_argv`] runs — the developer
/// approves the command they can read, not a paraphrase of it.
#[cfg(not(windows))]
const INSTALL_COMMAND: &str =
    "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y";
#[cfg(all(windows, target_arch = "aarch64"))]
const INSTALL_COMMAND: &str = "$i = \"$env:TEMP\\rustup-init.exe\"; \
     Invoke-WebRequest -Uri https://win.rustup.rs/aarch64 -OutFile $i; & $i -y";
#[cfg(all(windows, not(target_arch = "aarch64")))]
const INSTALL_COMMAND: &str = "$i = \"$env:TEMP\\rustup-init.exe\"; \
     Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile $i; & $i -y";

// ── Versions ─────────────────────────────────────────────────────────────────

/// A Rust release, compared the way `rust-version` is compared.
///
/// A pre-release suffix is dropped rather than ordered: `1.93.0-nightly` counts
/// as 1.93.0. Someone running nightly has a *newer* compiler than the floor we
/// are checking for, and refusing it would be pedantry with no user behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RustVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for RustVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl RustVersion {
    /// Parse a dotted version, ignoring anything from the first `-` on. An
    /// absent patch reads as 0, which is how `rust-version = "1.92"` is meant.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.split('-').next()?;
        let mut parts = text.split('.');
        let major = parts.next()?.trim().parse().ok()?;
        let minor = parts.next()?.trim().parse().ok()?;
        let patch = parts
            .next()
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Parse the version out of `rustc --version` output, e.g.
    /// `rustc 1.92.0 (0a1b2c3d4 2026-01-15)`.
    fn from_rustc_output(out: &str) -> Option<Self> {
        let mut words = out.split_whitespace();
        if words.next()? != "rustc" {
            return None;
        }
        Self::parse(words.next()?)
    }
}

/// The lowest Rust the workspace declares it needs.
pub fn minimum() -> RustVersion {
    manifest_rust_version(WORKSPACE_MANIFEST).unwrap_or(FALLBACK_MINIMUM)
}

/// Read `rust-version = "x.y"` out of a Cargo manifest.
fn manifest_rust_version(manifest: &str) -> Option<RustVersion> {
    manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("rust-version"))
        .and_then(|l| l.split('"').nth(1))
        .and_then(RustVersion::parse)
}

// ── Detection ────────────────────────────────────────────────────────────────

/// What the first run found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Usable: this `rustc`, this version.
    Ok { path: PathBuf, version: RustVersion },
    /// Present but below [`minimum`] — a different message, and a different
    /// fix (`rustup update stable`), from having nothing at all.
    TooOld { path: PathBuf, version: RustVersion },
    /// Rust is here and recent enough, and still cannot build: the machine has
    /// no **linker**, so the last step of a build has nothing to run.
    ///
    /// This is the answer rustup cannot give. The linker belongs to the
    /// platform — the Microsoft C++ build tools, Xcode's command line tools, a
    /// distribution's C toolchain — and rustup neither ships one nor says a
    /// word about needing one. So this status is *explained*, never offered:
    /// there is no button here that would help.
    NoLinker {
        path: PathBuf,
        version: RustVersion,
        linker: String,
    },
    /// No `rustc` answered, anywhere we know to look.
    Missing,
}

impl Status {
    /// Can this machine build a project?
    pub fn is_usable(&self) -> bool {
        matches!(self, Status::Ok { .. })
    }
}

/// Every `rustc` worth asking, in the order they are asked: PATH first, because
/// a developer who put one there means it; then the two standard rustup homes.
fn candidates() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(RUSTC)];
    if let Some(cargo_home) = std::env::var_os("CARGO_HOME") {
        out.push(Path::new(&cargo_home).join("bin").join(RUSTC));
    }
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".cargo").join("bin").join(RUSTC));
    }
    out.dedup();
    out
}

/// Ask one `rustc` for its version. `None` covers both "not there" and "there
/// but broken", which the caller treats alike: neither can build anything.
fn ask_version(program: &Path) -> Option<String> {
    let out = std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Probe this machine. One `rustc --version`, which is why it can run on every
/// start. It does **not** ask whether the machine can link — that costs a
/// compile, and [`with_link_check`] adds it on the one run that asks questions.
pub fn detect() -> Status {
    detect_with(&candidates(), ask_version)
}

/// The decision, separated from the machine so a test can hand it any world it
/// likes — no `rustc` at all, an ancient one, one only under `~/.cargo/bin`.
fn detect_with<F>(candidates: &[PathBuf], mut ask: F) -> Status
where
    F: FnMut(&Path) -> Option<String>,
{
    for path in candidates {
        let Some(version) = ask(path)
            .as_deref()
            .and_then(RustVersion::from_rustc_output)
        else {
            continue;
        };
        let path = path.clone();
        return if version >= minimum() {
            Status::Ok { path, version }
        } else {
            Status::TooOld { path, version }
        };
    }
    Status::Missing
}

/// Make the directory holding `program` visible to the child processes this IDE
/// starts, so a `cargo` found outside PATH is still a `cargo` Build can run.
///
/// Returns whether PATH was changed. A bare `rustc` (PATH resolved it) has no
/// directory to add, and a directory already on PATH is left alone.
pub fn ensure_on_path(program: &Path) -> bool {
    let Some(dir) = program.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return false;
    };
    let current = std::env::var_os("PATH").unwrap_or_default();
    if std::env::split_paths(&current).any(|p| p == dir) {
        return false;
    }
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&current));
    let Ok(joined) = std::env::join_paths(paths) else {
        return false;
    };
    std::env::set_var("PATH", joined);
    true
}

// ── Linking ──────────────────────────────────────────────────────────────────

/// Ask whether a usable Rust can actually produce an executable, turning a
/// [`Status::Ok`] into a [`Status::NoLinker`] when it cannot. Any other status
/// is returned untouched — there is no point asking a `rustc` that is missing
/// or too old to link something.
///
/// **Called on the first run only.** The probe compiles a program, which costs
/// a few hundred milliseconds; [`detect`] stays a single `--version` so that
/// every later start pays nothing.
pub fn with_link_check(status: Status) -> Status {
    with_link_check_by(status, probe_link)
}

/// The decision, separated from the machine so a test can hand it a world where
/// linking works, one where it does not, and one where the probe itself failed.
fn with_link_check_by<F>(status: Status, probe: F) -> Status
where
    F: FnOnce(&Path) -> Option<String>,
{
    let Status::Ok { path, version } = &status else {
        return status;
    };
    match probe(path) {
        Some(linker) => Status::NoLinker {
            path: path.clone(),
            version: *version,
            linker,
        },
        None => status,
    }
}

/// Link a program that does nothing, and report the linker rustc could not find.
///
/// Asking rustc to link is the question itself, which is why nothing here looks
/// for a compiler by name, reads a registry, or consults PATH: on Windows the
/// linker is found through the Visual Studio installation and none of those
/// would see it. rustc knows how to look, so rustc is asked.
///
/// `None` means "do not raise this" — the program linked, or the probe could
/// not be carried out at all (no temp directory, rustc would not start). A
/// probe that fails for its own reasons must never be reported as a missing
/// linker: the developer would go and install something they already have.
fn probe_link(rustc: &Path) -> Option<String> {
    let dir =
        std::env::temp_dir().join(format!("powerrustcobol-link-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let src = dir.join("probe.rs");
    let result = std::fs::write(&src, "fn main() {}\n")
        .ok()
        .and_then(|()| {
            std::process::Command::new(rustc)
                .arg(&src)
                .arg("--out-dir")
                .arg(&dir)
                .output()
                .ok()
        })
        .filter(|out| !out.status.success())
        .and_then(|out| cobolt_compiler::missing_linker(&String::from_utf8_lossy(&out.stderr)));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

// ── Installing ───────────────────────────────────────────────────────────────

/// The installer command, shown to the developer before they approve it.
pub fn install_command() -> &'static str {
    INSTALL_COMMAND
}

/// The program and arguments that run [`install_command`]. Kept as one function
/// so what is displayed and what is executed cannot drift apart.
pub fn install_argv() -> (&'static str, Vec<&'static str>) {
    if cfg!(windows) {
        (
            "powershell",
            vec!["-NoProfile", "-Command", INSTALL_COMMAND],
        )
    } else {
        ("sh", vec!["-c", INSTALL_COMMAND])
    }
}

/// How the installation ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    /// True only when the installer succeeded *and* a usable `rustc` answered
    /// afterwards — an installer that exits 0 while leaving nothing runnable
    /// has not installed anything.
    pub ok: bool,
    /// What was installed, when it worked.
    pub version: Option<RustVersion>,
    /// The `rustc` the probe found afterwards. Put on PATH by the UI thread
    /// rather than by the installer thread: the environment is process-wide,
    /// and writing it from a worker while the rest of the IDE reads it is the
    /// pattern Rust 2024 makes `unsafe` for good reason.
    pub program: Option<PathBuf>,
    /// The installer's own last words, for when it did not.
    pub detail: String,
    /// Set when rustup succeeded and the machine still cannot link. Kept apart
    /// from `ok` because the two say different things to the developer:
    /// `ok: false` with no linker here means "Rust did not install", and this
    /// means "Rust installed, and one more thing is missing".
    pub linker: Option<String>,
}

/// Run the installer on its own thread; the receiver yields exactly one
/// outcome. Never called on the IDE's behalf — only when the developer asks.
pub fn spawn_install() -> Receiver<InstallOutcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_install());
    });
    rx
}

fn run_install() -> InstallOutcome {
    let (program, args) = install_argv();
    let output = match std::process::Command::new(program).args(&args).output() {
        Ok(o) => o,
        Err(e) => {
            return InstallOutcome {
                ok: false,
                version: None,
                program: None,
                detail: format!("{program}: {e}"),
                linker: None,
            }
        }
    };
    if !output.status.success() {
        return InstallOutcome {
            ok: false,
            version: None,
            program: None,
            detail: last_lines(&String::from_utf8_lossy(&output.stderr)),
            linker: None,
        };
    }
    // rustup writes its shims into `~/.cargo/bin` and edits a shell profile we
    // are not running under, so believe the probe rather than the exit code.
    //
    // The link check runs here too. Without it this dialog would say "Rust is
    // installed. Build is available." to a Windows machine with no C++ build
    // tools — true about Rust, false about Build, and the developer would find
    // out at the end of their first build instead of at the end of this
    // sentence.
    match with_link_check(detect()) {
        Status::Ok { path, version } => InstallOutcome {
            ok: true,
            version: Some(version),
            detail: path.display().to_string(),
            program: Some(path),
            linker: None,
        },
        // Rust arrived; it is the linker that is missing. `program` is still
        // set — the `cargo` Build spawns has to find this Rust once the build
        // tools are installed, and PATH is fixed from the outcome either way.
        Status::NoLinker {
            path,
            version,
            linker,
        } => InstallOutcome {
            ok: false,
            version: Some(version),
            program: Some(path),
            detail: cobolt_compiler::linker_install_command().to_owned(),
            linker: Some(linker),
        },
        Status::TooOld { version, .. } => InstallOutcome {
            ok: false,
            version: Some(version),
            program: None,
            detail: format!("{version}"),
            linker: None,
        },
        Status::Missing => InstallOutcome {
            ok: false,
            version: None,
            program: None,
            detail: last_lines(&String::from_utf8_lossy(&output.stdout)),
            linker: None,
        },
    }
}

/// The tail of a command's output — enough to show what went wrong without
/// pasting a screenful of progress bars into a dialog.
fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(4);
    lines[start..].join("\n")
}

// ── The first-run conversation ───────────────────────────────────────────────

/// Which question is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// "Rust is missing / too old — install it?"
    Offer,
    /// Asked once more after a refusal, this time saying what is lost. The
    /// operator asked for the second ask on purpose: the cost of declining is
    /// not obvious from the first question, and a developer who learns it at
    /// Build time has already lost the afternoon.
    LastChance,
}

/// What a refusal means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Ask the second time.
    AskOnceMore,
    /// They meant it. Record the answer and never ask again.
    Accepted,
}

/// Progress of an installation the developer asked for.
pub enum Install {
    Running(Receiver<InstallOutcome>),
    Finished(InstallOutcome),
}

/// The first-run prompt: what was found, which question is up, and whether an
/// installation is under way.
pub struct FirstRunPrompt {
    pub status: Status,
    pub stage: Stage,
    pub install: Option<Install>,
}

impl FirstRunPrompt {
    /// A prompt for `status`, or `None` when there is nothing to ask about.
    pub fn for_status(status: Status) -> Option<Self> {
        (!status.is_usable()).then_some(Self {
            status,
            stage: Stage::Offer,
            install: None,
        })
    }

    /// The developer said no. The first no advances to the warning; the second
    /// is their answer.
    pub fn decline(&mut self) -> Decision {
        match self.stage {
            Stage::Offer => {
                self.stage = Stage::LastChance;
                Decision::AskOnceMore
            }
            Stage::LastChance => Decision::Accepted,
        }
    }

    /// Start the installation they approved.
    pub fn start_install(&mut self) {
        self.install = Some(Install::Running(spawn_install()));
    }

    /// Collect the outcome when the installer thread reports one, and make what
    /// it installed reachable by the `cargo` that Build spawns — from *this*
    /// thread, for the reason [`InstallOutcome::program`] gives.
    pub fn poll_install(&mut self) {
        if let Some(Install::Running(rx)) = &self.install {
            if let Ok(outcome) = rx.try_recv() {
                if let Some(program) = &outcome.program {
                    ensure_on_path(program);
                }
                self.install = Some(Install::Finished(outcome));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> RustVersion {
        RustVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn a_normal_rustc_line_parses() {
        assert_eq!(
            RustVersion::from_rustc_output("rustc 1.92.0 (0a1b2c3d4 2026-01-15)\n"),
            Some(v(1, 92, 0))
        );
    }

    #[test]
    fn a_prerelease_counts_as_its_release() {
        assert_eq!(
            RustVersion::from_rustc_output("rustc 1.93.0-nightly (9f8e7d6c 2026-02-01)"),
            Some(v(1, 93, 0))
        );
    }

    #[test]
    fn output_from_something_that_is_not_rustc_does_not_parse() {
        assert_eq!(RustVersion::from_rustc_output("cargo 1.92.0"), None);
        assert_eq!(RustVersion::from_rustc_output("command not found"), None);
        assert_eq!(RustVersion::from_rustc_output(""), None);
    }

    #[test]
    fn a_missing_patch_reads_as_zero() {
        assert_eq!(RustVersion::parse("1.92"), Some(v(1, 92, 0)));
    }

    /// The floor has to come from the workspace, not from this file — see the
    /// module note. If the manifest stops declaring it, this fails rather than
    /// letting [`FALLBACK_MINIMUM`] quietly become the real rule.
    #[test]
    fn the_minimum_is_read_from_the_workspace_manifest() {
        let declared = manifest_rust_version(WORKSPACE_MANIFEST)
            .expect("the workspace manifest must declare rust-version");
        assert_eq!(minimum(), declared);
        println!("workspace requires Rust {declared}");
    }

    #[test]
    fn manifest_parsing_reads_the_quoted_value() {
        let manifest = "[workspace.package]\nedition = \"2021\"\nrust-version = \"1.92\"\n";
        assert_eq!(manifest_rust_version(manifest), Some(v(1, 92, 0)));
        assert_eq!(manifest_rust_version("[package]\nname = \"x\"\n"), None);
    }

    #[test]
    fn nothing_answering_anywhere_is_missing() {
        let cands = vec![PathBuf::from("rustc"), PathBuf::from("/opt/rustc")];
        assert_eq!(detect_with(&cands, |_| None), Status::Missing);
    }

    #[test]
    fn the_first_candidate_that_answers_wins() {
        let path = PathBuf::from("rustc");
        let cargo_home = PathBuf::from("/home/dev/.cargo/bin/rustc");
        let cands = vec![path.clone(), cargo_home.clone()];
        let found = detect_with(&cands, |p| {
            Some(if p == path {
                "rustc 1.95.0 (aaaa 2026-05-01)".to_owned()
            } else {
                "rustc 1.92.0 (bbbb 2026-01-01)".to_owned()
            })
        });
        assert_eq!(
            found,
            Status::Ok {
                path,
                version: v(1, 95, 0)
            }
        );
        // And when PATH holds nothing, the rustup home is still found — the
        // case a Finder-launched app hits on a machine that does have Rust.
        let only_cargo_home = detect_with(&cands, |p| {
            (p == cargo_home).then(|| "rustc 1.92.0 (bbbb 2026-01-01)".to_owned())
        });
        assert_eq!(
            only_cargo_home,
            Status::Ok {
                path: cargo_home,
                version: v(1, 92, 0)
            }
        );
    }

    #[test]
    fn an_old_rustc_is_too_old_not_missing() {
        let path = PathBuf::from("rustc");
        let found = detect_with(&[path.clone()], |_| {
            Some("rustc 1.70.0 (cccc 2023-06-01)".to_owned())
        });
        assert_eq!(
            found,
            Status::TooOld {
                path,
                version: v(1, 70, 0)
            }
        );
        assert!(!found.is_usable());
    }

    #[test]
    fn an_acceptable_toolchain_raises_no_prompt() {
        let ok = Status::Ok {
            path: PathBuf::from("rustc"),
            version: minimum(),
        };
        assert!(ok.is_usable());
        assert!(FirstRunPrompt::for_status(ok).is_none());
    }

    #[test]
    fn declining_once_asks_again_declining_twice_is_final() {
        let mut prompt =
            FirstRunPrompt::for_status(Status::Missing).expect("a missing toolchain must ask");
        assert_eq!(prompt.stage, Stage::Offer);
        assert_eq!(prompt.decline(), Decision::AskOnceMore);
        assert_eq!(prompt.stage, Stage::LastChance);
        assert_eq!(prompt.decline(), Decision::Accepted);
    }

    /// The dialog shows a command and then runs one. They are the same string.
    #[test]
    fn the_command_shown_is_the_command_run() {
        let (program, args) = install_argv();
        assert!(!program.is_empty());
        assert_eq!(
            args.last().copied(),
            Some(install_command()),
            "the executed command must be the one on screen"
        );
        assert!(
            install_command().contains("rustup.rs"),
            "only the official installer: {}",
            install_command()
        );
    }

    /// The dialog leaves "Installing…" only when the installer thread answers,
    /// and it collects that answer exactly once.
    #[test]
    fn a_finished_installation_is_collected_when_it_arrives() {
        let (tx, rx) = mpsc::channel();
        let mut prompt = FirstRunPrompt::for_status(Status::Missing).expect("must ask");
        prompt.install = Some(Install::Running(rx));

        prompt.poll_install();
        assert!(
            matches!(prompt.install, Some(Install::Running(_))),
            "nothing has been reported yet"
        );

        tx.send(InstallOutcome {
            ok: true,
            version: Some(v(1, 92, 0)),
            program: None, // no PATH edit from a test
            detail: String::new(),
            linker: None,
        })
        .expect("the prompt still holds the receiver");
        prompt.poll_install();
        match &prompt.install {
            Some(Install::Finished(outcome)) => assert_eq!(outcome.version, Some(v(1, 92, 0))),
            _ => panic!("the outcome should have been collected"),
        }
    }

    #[test]
    fn a_bare_program_name_has_no_directory_to_add() {
        assert!(!ensure_on_path(Path::new("rustc")));
    }

    #[test]
    fn only_four_lines_of_a_failure_are_kept() {
        let text = (1..=10).map(|i| format!("line {i}\n")).collect::<String>();
        assert_eq!(last_lines(&text), "line 7\nline 8\nline 9\nline 10");
        assert_eq!(last_lines("only one\n\n"), "only one");
    }

    // ── The linker ───────────────────────────────────────────────────────────

    /// A good Rust on a machine that cannot link is not a good answer, and the
    /// version survives the change — the dialog says which Rust is installed.
    #[test]
    fn a_rust_that_cannot_link_is_not_usable() {
        let status = Status::Ok {
            path: PathBuf::from("/usr/bin/rustc"),
            version: v(1, 92, 0),
        };
        let refined = with_link_check_by(status, |_| Some("link.exe".to_owned()));
        assert_eq!(
            refined,
            Status::NoLinker {
                path: PathBuf::from("/usr/bin/rustc"),
                version: v(1, 92, 0),
                linker: "link.exe".to_owned(),
            }
        );
        assert!(!refined.is_usable(), "it cannot build, so it is not usable");
    }

    /// The ordinary machine: the probe links, and nothing about the answer
    /// changes.
    #[test]
    fn a_rust_that_links_is_left_alone() {
        let status = Status::Ok {
            path: PathBuf::from("/usr/bin/rustc"),
            version: v(1, 92, 0),
        };
        let refined = with_link_check_by(status.clone(), |_| None);
        assert_eq!(refined, status);
        assert!(refined.is_usable());
    }

    /// There is nothing to ask a `rustc` that is absent or too old — the probe
    /// must not even run, or a machine with no Rust would be told its linker is
    /// the problem.
    #[test]
    fn nothing_is_probed_when_rust_itself_is_the_problem() {
        for status in [
            Status::Missing,
            Status::TooOld {
                path: PathBuf::from("/usr/bin/rustc"),
                version: v(1, 70, 0),
            },
        ] {
            let refined = with_link_check_by(status.clone(), |_| {
                panic!("the probe must not run for {status:?}")
            });
            assert_eq!(refined, status);
        }
    }

    /// The real probe, against the `rustc` running this test. A machine that
    /// can build the test suite can certainly link, so any answer but `None`
    /// means the probe itself is broken — and a broken probe would send every
    /// developer off to install build tools they already have.
    #[test]
    fn the_real_probe_finds_no_fault_with_a_working_toolchain() {
        assert_eq!(probe_link(Path::new(RUSTC)), None);
    }

    /// A prompt for a missing linker exists — it is the whole point — and it
    /// opens on the first question, not on the second-ask stage, because there
    /// is nothing here for the developer to decline.
    #[test]
    fn a_missing_linker_raises_a_prompt_with_nothing_to_decline() {
        let prompt = FirstRunPrompt::for_status(Status::NoLinker {
            path: PathBuf::from("/usr/bin/rustc"),
            version: v(1, 92, 0),
            linker: "cc".to_owned(),
        })
        .expect("a machine that cannot build has something to say");
        assert_eq!(prompt.stage, Stage::Offer);
        assert!(prompt.install.is_none());
    }
}
