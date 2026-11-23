// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Preparing the platform ahead of the first Build (spec 112).
//!
//! The Windows setup compiles PowerRustCOBOL's platform for **both profiles** into the installing developer's
//! shared compiled-crate cache, so the first Debug or EXEC RUST Build compiles only the program. This is the
//! part of that work that is not the installer: [`prepare_platform`] loops the profiles and the three stand-in
//! programs, tells its caller what cargo says as it says it ([`Progress`]), stops when it is told to, and
//! stamps a profile only when every one of its stand-ins finished.
//!
//! Cargo is run through [`CargoRunner`], so what the loop decides (which stamp, which marker, what a failure or
//! a cancel leaves behind) is tested with a recorded fake and no compile; [`SystemCargo`] is the real one.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{build_variant, mark_warm, seed_from_shipped, variants, Built, Stand};
use crate::build_cache;

/// Which of cargo's profiles a compile is for. The debug and the release platforms are separate compiles
/// into separate folders of the cache, and only the release one ever shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    /// `cargo build --release`: what EXEC RUST and a Release Build link.
    Release,
    /// `cargo build`: what a Debug Build links.
    Debug,
}

impl Profile {
    /// Both, release first: the shipped libraries make it the quick one.
    pub const ALL: [Profile; 2] = [Profile::Release, Profile::Debug];

    /// Whether this is the debug profile (the `debug` of [`super::platform_prep`] and the stamps).
    pub fn is_debug(self) -> bool {
        self == Profile::Debug
    }

    /// The cache's folder for the profile.
    pub fn folder(self) -> &'static str {
        match self {
            Profile::Release => "release",
            Profile::Debug => "debug",
        }
    }

    /// What follows `cargo`.
    pub fn cargo_args(self) -> &'static [&'static str] {
        match self {
            Profile::Release => &["build", "--release"],
            Profile::Debug => &["build"],
        }
    }
}

/// How much of the machine a compile takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// What a Build made while the developer works asks for: the IDE and everything else keep their turn.
    BelowNormal,
    /// The setup's preparation: it has told the developer it takes the whole machine (R11).
    Normal,
}

impl Priority {
    /// The Windows process-creation flag that selects it.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) const fn windows_class(self) -> u32 {
        match self {
            Priority::BelowNormal => 0x0000_4000, // BELOW_NORMAL_PRIORITY_CLASS
            Priority::Normal => 0x0000_0020,      // NORMAL_PRIORITY_CLASS
        }
    }
}

/// What one run of cargo is asked to do.
#[derive(Debug, Clone, Copy)]
pub struct CargoBuild<'a> {
    /// The folder of the program being compiled.
    pub dir: &'a Path,
    /// The cache, as the target folder.
    pub cache: &'a Path,
    /// Release or debug.
    pub profile: Profile,
    /// Without the network (the first attempt of every Build).
    pub offline: bool,
    /// How much of the machine it takes.
    pub priority: Priority,
}

/// One thing cargo said that matters to a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// Cargo began compiling this crate.
    Compiling(String),
    /// Cargo is waiting for another cargo to release the cache's lock.
    Waiting,
    /// Nothing was said for a moment: it is still running. Arrives at least four times a second while cargo is quiet.
    Quiet,
}

/// What the caller answers to each [`Tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Go on.
    Go,
    /// Stop cargo now.
    Stop,
}

/// How a run of cargo ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// It built.
    Success,
    /// It ran and failed: its exit code, and the last lines of what it said, in reading order.
    Failed {
        /// The exit code, when there was one.
        code: Option<i32>,
        /// The last lines of cargo's stderr.
        tail: Vec<String>,
    },
    /// The caller stopped it.
    Stopped,
    /// It could not be started (no `cargo`), and why.
    NotRun(String),
}

/// Runs cargo for [`prepare_platform_with`]. [`SystemCargo`] is the real one; a test brings a recorded fake.
pub trait CargoRunner {
    /// Run `request`, giving `on` every [`Tick`] as it happens, and stop at the first [`Flow::Stop`].
    fn build(&mut self, request: &CargoBuild<'_>, on: &mut dyn FnMut(Tick) -> Flow) -> Ended;
}

/// How long the real runner waits for cargo to say something before it sends a [`Tick::Quiet`].
const POLL: Duration = Duration::from_millis(250);
/// How many of cargo's last lines a failure keeps.
const TAIL_LINES: usize = 12;

/// The tick a line of cargo's stderr is, if it is one. Cargo is made to write plain text ([`build_cache::cargo`]).
pub fn tick_of(line: &str) -> Option<Tick> {
    if let Some(rest) = line.trim_start().strip_prefix("Compiling ") {
        return Some(Tick::Compiling(
            rest.split_whitespace().next().unwrap_or("").to_owned(),
        ));
    }
    build_cache::is_lock_wait_line(line).then_some(Tick::Waiting)
}

/// The cargo of this machine.
///
/// Stopping it ends `cargo`; on Windows the `rustc` processes it started finish the unit they are on first. The
/// setup's supervisor puts the whole step in a job object, which ends them too.
pub struct SystemCargo;

impl CargoRunner for SystemCargo {
    fn build(&mut self, request: &CargoBuild<'_>, on: &mut dyn FnMut(Tick) -> Flow) -> Ended {
        let mut command = build_cache::cargo_at(request.priority);
        command.args(request.profile.cargo_args());
        if request.offline {
            command.arg("--offline");
        }
        let spawned = command
            .current_dir(request.dir)
            .env("CARGO_TARGET_DIR", request.cache)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => return Ended::NotRun(e.to_string()),
        };
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Ended::NotRun("cargo's output could not be read".to_owned());
        };

        // A thread reads the pipe, so this loop can be woken by a quiet moment, not only by a line: a single
        // large crate compiles for minutes without a word, and the caller must still be able to stop it.
        let (lines, heard) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            use std::io::{BufRead as _, BufReader};
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if lines.send(line).is_err() {
                    break;
                }
            }
        });

        let mut tail: VecDeque<String> = VecDeque::with_capacity(TAIL_LINES + 1);
        let stopped = loop {
            let flow = match heard.recv_timeout(POLL) {
                Ok(line) => {
                    let tick = tick_of(&line);
                    tail.push_back(line);
                    if tail.len() > TAIL_LINES {
                        tail.pop_front();
                    }
                    match tick {
                        Some(tick) => on(tick),
                        None => Flow::Go,
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => on(Tick::Quiet),
                Err(mpsc::RecvTimeoutError::Disconnected) => break false,
            };
            if flow == Flow::Stop {
                break true;
            }
        };
        if stopped {
            // The reader is not joined: a `rustc` that outlives `cargo` still holds the pipe, and waiting for
            // its end of file would wait for its unit.
            let _ = child.kill();
            let _ = child.wait();
            return Ended::Stopped;
        }
        let _ = reader.join();
        match child.wait() {
            Ok(status) if status.success() => Ended::Success,
            Ok(status) => Ended::Failed {
                code: status.code(),
                tail: tail.into(),
            },
            Err(e) => Ended::NotRun(format!("cannot wait for cargo: {e}")),
        }
    }
}

/// Written in the cache when the setup created it, and only then: what a cancelled setup may remove (R14).
pub const CREATED_BY_SETUP: &str = ".prepared-by-setup";

/// What to prepare, where, and for which version.
#[derive(Debug, Clone, Copy)]
pub struct Prepare<'a> {
    /// The SDK the stand-ins are staged against (its `crates/`).
    pub sdk_root: &'a Path,
    /// The build folder of the installing developer (`resolve_build_root`), which holds `shared-target`.
    pub build_root: &'a Path,
    /// The PowerRustCOBOL version, which names the stamps.
    pub version: &'a str,
    /// Compile the release platform.
    pub release: bool,
    /// Compile the debug platform.
    pub debug: bool,
    /// Set by the caller to stop: checked at every line cargo says and at least four times a second.
    pub cancel: &'a AtomicBool,
    /// At most this often a [`Event::Beat`] says nothing new, only that it is still running.
    pub heartbeat: Duration,
}

impl Prepare<'_> {
    /// How often a quiet compile says it is still running.
    pub const HEARTBEAT: Duration = Duration::from_secs(2);
}

/// What the preparation is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// A stand-in program begins.
    Began,
    /// Cargo began compiling this crate.
    Compiling(&'a str),
    /// Cargo is waiting for another cargo.
    Waiting,
    /// Still running; nothing new.
    Beat,
}

/// Where the preparation is, for the caller to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress<'a> {
    /// The profile.
    pub profile: Profile,
    /// The stand-in program (`forms`, `forms-sql`, `forms-graphics`).
    pub variant: &'a str,
    /// Crates compiled so far in this profile, over every stand-in.
    pub crates: u32,
    /// Since this profile began.
    pub elapsed: Duration,
    /// What happened.
    pub event: Event<'a>,
}

/// How one profile ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Every stand-in compiled and the stamp is written.
    Done,
    /// A stand-in failed (or the cache could not be used); no stamp was written.
    Failed {
        /// The stand-in that failed (`forms-sql`), or empty when the failure is not one stand-in's (the cache, the stamp).
        unit: String,
        /// What went wrong, in words: the last lines of cargo's complaint, or the operating system's.
        why: String,
    },
    /// The caller stopped it; no stamp was written.
    Cancelled,
    /// It was not asked for.
    NotAsked,
}

/// What [`prepare_platform`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The cache, when it could be opened.
    pub cache: Option<PathBuf>,
    /// Whether this run created the cache folder (then it carries [`CREATED_BY_SETUP`]).
    pub created_cache: bool,
    /// How each profile ended, release first.
    pub ends: Vec<(Profile, End)>,
}

impl Report {
    /// How `profile` ended.
    pub fn end(&self, profile: Profile) -> &End {
        self.ends
            .iter()
            .find(|(p, _)| *p == profile)
            .map_or(&End::NotAsked, |(_, e)| e)
    }

    /// Whether the caller stopped it.
    pub fn cancelled(&self) -> bool {
        self.ends.iter().any(|(_, e)| *e == End::Cancelled)
    }

    /// Whether every profile asked for is done.
    pub fn all_done(&self) -> bool {
        self.ends
            .iter()
            .all(|(_, e)| matches!(e, End::Done | End::NotAsked))
    }
}

/// [`prepare_platform_with`] with this machine's cargo.
pub fn prepare_platform(
    p: &Prepare<'_>,
    on: &mut dyn FnMut(&Progress<'_>),
    log: &mut dyn FnMut(&str),
) -> Report {
    prepare_platform_with(&mut SystemCargo, p, on, log)
}

/// Compile the platform into the developer's cache, for each profile asked for, every stand-in program in turn.
///
/// * A profile gets its stamp (`warm/<version>`, `warm/<version>-debug`) **only when every stand-in of it
///   finished**. A failure ends that profile and the next is still tried; a cancel ends everything.
/// * The cache folder is marked [`CREATED_BY_SETUP`] **when this run created it**, before anything is compiled in
///   it, so a cancelled setup can tell what it may remove.
/// * The release profile starts from the libraries the release shipped, when there are any for this compiler
///   ([`seed_from_shipped`]): most of its time is saved.
pub fn prepare_platform_with(
    runner: &mut dyn CargoRunner,
    p: &Prepare<'_>,
    on: &mut dyn FnMut(&Progress<'_>),
    log: &mut dyn FnMut(&str),
) -> Report {
    let asked = |profile: Profile| {
        if profile.is_debug() {
            p.debug
        } else {
            p.release
        }
    };
    let all_end = |why: &str| {
        Profile::ALL
            .iter()
            .map(|&profile| {
                (
                    profile,
                    if asked(profile) {
                        End::Failed { unit: String::new(), why: why.to_owned() }
                    } else {
                        End::NotAsked
                    },
                )
            })
            .collect::<Vec<_>>()
    };
    let existed = build_cache::cache_dir(p.build_root).is_dir();
    let cache = match build_cache::open(p.build_root) {
        Ok(cache) => cache,
        Err(why) => {
            return Report {
                cache: None,
                created_cache: false,
                ends: all_end(&why),
            }
        }
    };
    let created_cache = !existed;
    if created_cache {
        // Before anything is compiled: a setup cancelled in the middle must still be able to tell this folder is its own.
        if let Err(e) = std::fs::write(cache.join(CREATED_BY_SETUP), p.version) {
            log(&format!(
                "   could not mark the cache as created by the setup: {e}"
            ));
        }
    }
    let mut report = Report {
        cache: Some(cache.clone()),
        created_cache,
        ends: Vec::new(),
    };

    let check = match build_cache::check_platform(p.sdk_root, &cache) {
        Ok(check) => check,
        Err(e) => {
            report.ends = all_end(&format!("cannot judge the platform crates: {e}"));
            return report;
        }
    };
    for (name, hash) in &check.record {
        if let Err(e) = build_cache::write_marker(&cache, name, hash) {
            log(&format!(
                "   could not record {name} in the compiled-crate cache: {e}"
            ));
        }
    }

    let mut cleaned: HashSet<&'static str> = HashSet::new();
    let mut staged: Vec<PathBuf> = Vec::new();
    let mut cancelled = false;
    for profile in Profile::ALL {
        if !asked(profile) {
            report.ends.push((profile, End::NotAsked));
            continue;
        }
        if cancelled || p.cancel.load(Ordering::Relaxed) {
            cancelled = true;
            report.ends.push((profile, End::Cancelled));
            continue;
        }
        if profile == Profile::Release {
            match seed_from_shipped(p.sdk_root, &cache, log) {
                Ok(_) => {}
                Err(why) => log(&format!("   {why} - compiling what is missing here")),
            }
        }

        let began = Instant::now();
        let mut crates = 0u32;
        let end = 'profile: {
            for (label, features) in variants() {
                if p.cancel.load(Ordering::Relaxed) {
                    break 'profile End::Cancelled;
                }
                let dir = p.build_root.join(format!("cobolt-build-_warmup-{label}"));
                staged.push(dir.clone());
                log(&format!(
                    "   compiling the platform libraries ({label}, {})...",
                    profile.folder()
                ));
                let said = std::cell::Cell::new(Instant::now());
                let mut tell = |event: Event<'_>, crates: u32| {
                    said.set(Instant::now());
                    on(&Progress {
                        profile,
                        variant: label,
                        crates,
                        elapsed: began.elapsed(),
                        event,
                    });
                };
                tell(Event::Began, crates);
                let stand = Stand {
                    sdk_root: p.sdk_root,
                    dir: &dir,
                    cache: &cache,
                    features,
                    check: &check,
                    profile,
                    priority: Priority::Normal,
                };
                let built = build_variant(&stand, &mut cleaned, runner, &mut |tick| {
                    if p.cancel.load(Ordering::Relaxed) {
                        return Flow::Stop;
                    }
                    match tick {
                        Tick::Compiling(name) => {
                            crates += 1;
                            tell(Event::Compiling(&name), crates);
                        }
                        Tick::Waiting => tell(Event::Waiting, crates),
                        Tick::Quiet if said.get().elapsed() >= p.heartbeat => {
                            tell(Event::Beat, crates)
                        }
                        Tick::Quiet => {}
                    }
                    Flow::Go
                });
                match built {
                    Ok(Built::Done) => {}
                    Ok(Built::Stopped) => break 'profile End::Cancelled,
                    Err(why) => break 'profile End::Failed { unit: label.to_owned(), why },
                }
            }
            match mark_warm(&cache, p.version, profile.is_debug()) {
                Ok(()) => End::Done,
                Err(e) => End::Failed { unit: String::new(), why: format!("the stamp could not be written: {e}") },
            }
        };
        cancelled |= end == End::Cancelled;
        report.ends.push((profile, end));
    }

    for dir in staged {
        let _ = std::fs::remove_dir_all(dir);
    }
    report
}

/// Whether the setup created this build folder's cache (it carries [`CREATED_BY_SETUP`]): the only cache a
/// cancelled setup removes.
pub fn cache_created_by_setup(build_root: &Path) -> bool {
    build_cache::cache_dir(build_root)
        .join(CREATED_BY_SETUP)
        .is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warmup::{stamp_name, WARM_DIR};

    /// A cargo that says what it is told to and ends as it is told to; every request is kept.
    struct Fake {
        asked: Vec<(Profile, bool, Priority, PathBuf)>,
        script: Box<dyn FnMut(&CargoBuild<'_>, usize) -> (Vec<&'static str>, Ended)>,
    }

    impl Fake {
        fn new(
            script: impl FnMut(&CargoBuild<'_>, usize) -> (Vec<&'static str>, Ended) + 'static,
        ) -> Fake {
            Fake {
                asked: Vec::new(),
                script: Box::new(script),
            }
        }

        /// Every compile builds, saying three crates.
        fn building() -> Fake {
            Fake::new(|_, _| {
                (
                    vec![
                        "   Compiling libc v0.2.178",
                        "   Compiling cobolt-ast v0.2.0 (C:\\sdk\\crates\\cobolt-ast)",
                        "   Compiling prc-warmup v0.0.0 (C:\\build\\cobolt-build-_warmup-forms)",
                        "    Finished `release` profile [optimized] target(s) in 41.2s",
                    ],
                    Ended::Success,
                )
            })
        }
    }

    impl CargoRunner for Fake {
        fn build(&mut self, r: &CargoBuild<'_>, on: &mut dyn FnMut(Tick) -> Flow) -> Ended {
            let n = self.asked.len();
            self.asked
                .push((r.profile, r.offline, r.priority, r.dir.to_path_buf()));
            let (lines, end) = (self.script)(r, n);
            for line in lines {
                if let Some(tick) = tick_of(line) {
                    if on(tick) == Flow::Stop {
                        return Ended::Stopped;
                    }
                }
            }
            end
        }
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("prc-prepare-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn sdk(&self) -> PathBuf {
            let sdk = self.0.join("sdk");
            std::fs::create_dir_all(&sdk).unwrap();
            sdk
        }

        fn builds(&self) -> PathBuf {
            self.0.join("builds")
        }

        fn stamp(&self, version: &str, debug: bool) -> bool {
            build_cache::cache_dir(&self.builds())
                .join(WARM_DIR)
                .join(stamp_name(version, debug))
                .is_file()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn run(
        fake: &mut Fake,
        scratch: &Scratch,
        release: bool,
        debug: bool,
        cancel: &AtomicBool,
    ) -> (Report, Vec<String>) {
        let sdk = scratch.sdk();
        let builds = scratch.builds();
        let p = Prepare {
            sdk_root: &sdk,
            build_root: &builds,
            version: "9.9.9",
            release,
            debug,
            cancel,
            heartbeat: Duration::ZERO,
        };
        let mut seen = Vec::new();
        let report = prepare_platform_with(
            fake,
            &p,
            &mut |g| {
                seen.push(format!(
                    "{} {} {} {}",
                    g.profile.folder(),
                    g.variant,
                    g.crates,
                    match g.event {
                        Event::Began => "began".to_owned(),
                        Event::Compiling(n) => format!("compiling {n}"),
                        Event::Waiting => "waiting".to_owned(),
                        Event::Beat => "beat".to_owned(),
                    }
                ))
            },
            &mut |_| {},
        );
        (report, seen)
    }

    /// Recorded from cargo 1.95.0 building the platform on Windows; the lines the counter must and must not count.
    #[test]
    fn cargo_lines_become_ticks_and_only_compiling_lines_are_counted() {
        let recorded = [
            "    Updating crates.io index",
            "   Compiling proc-macro2 v1.0.101",
            "   Compiling cobolt-ast v0.2.0 (C:\\Program Files\\PowerRustCOBOL\\sdk\\crates\\cobolt-ast)",
            "    Blocking waiting for file lock on artifact directory",
            "warning: unused import: `x`",
            "     Running `rustc --crate-name build_script_build`",
            "   Compiling prc-warmup v0.0.0 (C:\\Users\\dev\\AppData\\Local\\PowerRustCOBOL\\builds\\cobolt-build-_warmup-forms)",
            "    Finished `release` profile [optimized] target(s) in 38m 12s",
            "",
        ];
        let ticks: Vec<Tick> = recorded.iter().filter_map(|l| tick_of(l)).collect();
        assert_eq!(
            ticks,
            [
                Tick::Compiling("proc-macro2".into()),
                Tick::Compiling("cobolt-ast".into()),
                Tick::Waiting,
                Tick::Compiling("prc-warmup".into()),
            ]
        );
        println!(
            "cargo lines: {} recorded lines -> {} ticks (3 crates counted, 1 lock wait)",
            recorded.len(),
            ticks.len()
        );
    }

    #[test]
    fn both_profiles_name_their_own_folder_and_arguments_and_the_priority_is_asked_for_by_name() {
        assert_eq!(Profile::ALL, [Profile::Release, Profile::Debug]);
        assert_eq!(
            (Profile::Release.folder(), Profile::Debug.folder()),
            ("release", "debug")
        );
        assert_eq!(Profile::Release.cargo_args(), ["build", "--release"]);
        assert_eq!(Profile::Debug.cargo_args(), ["build"]);
        assert!(!Profile::Release.is_debug() && Profile::Debug.is_debug());
        // Windows' own values: below normal is what the IDE's Builds ask for, normal is what the setup asks for.
        assert_eq!(Priority::BelowNormal.windows_class(), 0x4000);
        assert_eq!(Priority::Normal.windows_class(), 0x20);
        assert_ne!(
            Priority::Normal.windows_class(),
            0,
            "normal is asked for, never left to be inherited"
        );
    }

    #[test]
    fn everything_done_writes_both_stamps_and_the_marker_only_for_a_folder_it_created() {
        // A fresh build folder: the setup creates the cache and marks it.
        let fresh = Scratch::new("fresh");
        let mut fake = Fake::building();
        let (report, seen) = run(&mut fake, &fresh, true, true, &AtomicBool::new(false));
        assert!(report.all_done() && report.created_cache, "{report:?}");
        assert!(
            fresh.stamp("9.9.9", false) && fresh.stamp("9.9.9", true),
            "both stamps"
        );
        assert!(cache_created_by_setup(&fresh.builds()), "the marker");
        // Three stand-ins in each of two profiles, offline first and nothing retried; the preparation asks for normal priority.
        assert_eq!(fake.asked.len(), 6);
        assert_eq!(
            fake.asked.iter().map(|a| a.0).collect::<Vec<_>>(),
            [Profile::Release; 3]
                .into_iter()
                .chain([Profile::Debug; 3])
                .collect::<Vec<_>>()
        );
        assert!(fake.asked.iter().all(|a| a.1 && a.2 == Priority::Normal));
        assert_eq!(
            fake.asked
                .iter()
                .map(|a| a.3.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()[..3],
            [
                "cobolt-build-_warmup-forms",
                "cobolt-build-_warmup-forms-sql",
                "cobolt-build-_warmup-forms-graphics"
            ]
        );
        // The stand-ins' staging folders are not left behind.
        assert!(!fresh.builds().join("cobolt-build-_warmup-forms").exists());

        // The crates counted so far run on across a profile's three stand-ins and begin again for the next profile.
        let counts: Vec<&str> = seen
            .iter()
            .filter(|s| s.contains("compiling prc-warmup"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            counts,
            [
                "release forms 3 compiling prc-warmup",
                "release forms-sql 6 compiling prc-warmup",
                "release forms-graphics 9 compiling prc-warmup",
                "debug forms 3 compiling prc-warmup",
                "debug forms-sql 6 compiling prc-warmup",
                "debug forms-graphics 9 compiling prc-warmup",
            ]
        );
        assert_eq!(seen.iter().filter(|s| s.ends_with("began")).count(), 6);

        // A build folder that already had its cache: the setup did not create it, so it does not mark it.
        let existing = Scratch::new("existing");
        std::fs::create_dir_all(build_cache::cache_dir(&existing.builds())).unwrap();
        let (report, _) = run(
            &mut Fake::building(),
            &existing,
            true,
            true,
            &AtomicBool::new(false),
        );
        assert!(report.all_done() && !report.created_cache);
        assert!(
            !cache_created_by_setup(&existing.builds()),
            "a cache that was there is never marked as the setup's"
        );
        assert!(existing.stamp("9.9.9", false) && existing.stamp("9.9.9", true));
        println!(
            "prepare_platform with a recorded cargo: 2 profiles x 3 stand-ins = {} compiles, both stamps, marker only for the folder it created, {} progress events",
            fake.asked.len(),
            seen.len()
        );
    }

    #[test]
    fn a_failing_debug_unit_leaves_the_release_stamp_and_no_debug_one() {
        let scratch = Scratch::new("debugfails");
        // The second debug stand-in fails, offline and then online.
        let mut fake = Fake::new(|r, _| {
            let sql = r.dir.ends_with("cobolt-build-_warmup-forms-sql");
            if r.profile == Profile::Debug && sql {
                (
                    vec!["   Compiling rusqlite v0.37.0"],
                    Ended::Failed {
                        code: Some(101),
                        tail: vec!["error: could not compile `rusqlite`".into()],
                    },
                )
            } else {
                (vec!["   Compiling libc v0.2.178"], Ended::Success)
            }
        });
        let (report, _) = run(&mut fake, &scratch, true, true, &AtomicBool::new(false));
        assert_eq!(*report.end(Profile::Release), End::Done);
        let End::Failed { unit, why } = report.end(Profile::Debug) else {
            panic!("{report:?}")
        };
        assert!(
            unit == "forms-sql" && why.starts_with("cargo build failed:") && why.contains("rusqlite"),
            "{why}"
        );
        assert!(
            scratch.stamp("9.9.9", false),
            "the release stamp is written"
        );
        assert!(!scratch.stamp("9.9.9", true), "no debug stamp");
        assert!(!report.all_done() && !report.cancelled());
        // Offline, then online, for the failing stand-in; the third debug stand-in was never started.
        let debug_calls: Vec<(bool, String)> = fake
            .asked
            .iter()
            .filter(|a| a.0 == Profile::Debug)
            .map(|a| (a.1, a.3.file_name().unwrap().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(
            debug_calls,
            [
                (true, "cobolt-build-_warmup-forms".to_owned()),
                (true, "cobolt-build-_warmup-forms-sql".to_owned()),
                (false, "cobolt-build-_warmup-forms-sql".to_owned()),
            ]
        );
    }

    #[test]
    fn an_offline_failure_is_retried_online_and_a_failed_release_does_not_stop_the_debug_profile() {
        let scratch = Scratch::new("online");
        // The first attempt of every stand-in fails offline; online it builds.
        let mut fake = Fake::new(|r, _| {
            if r.offline {
                (
                    vec![],
                    Ended::Failed {
                        code: Some(101),
                        tail: vec!["error: no matching package named `x` found (--offline)".into()],
                    },
                )
            } else {
                (vec!["   Compiling libc v0.2.178"], Ended::Success)
            }
        });
        let (report, _) = run(&mut fake, &scratch, true, false, &AtomicBool::new(false));
        assert!(report.all_done());
        assert_eq!(fake.asked.len(), 6, "three stand-ins, offline then online");
        assert_eq!(*report.end(Profile::Debug), End::NotAsked);
        assert!(
            scratch.stamp("9.9.9", false) && !scratch.stamp("9.9.9", true),
            "only the profile asked for"
        );

        // The release profile fails at its first stand-in; the debug profile is still tried and, building, is stamped.
        let scratch = Scratch::new("releasefails");
        let mut fake = Fake::new(|r, _| {
            if r.profile == Profile::Release {
                (
                    vec![],
                    Ended::Failed {
                        code: Some(1),
                        tail: vec!["error: linker `link.exe` not found".into()],
                    },
                )
            } else {
                (vec![], Ended::Success)
            }
        });
        let (report, _) = run(&mut fake, &scratch, true, true, &AtomicBool::new(false));
        assert!(matches!(report.end(Profile::Release), End::Failed { why, .. } if why.contains("link.exe")));
        assert_eq!(*report.end(Profile::Debug), End::Done);
        assert!(!scratch.stamp("9.9.9", false) && scratch.stamp("9.9.9", true));
    }

    #[test]
    fn a_cancel_stops_before_the_next_unit_and_writes_no_stamp_for_what_it_interrupted() {
        // Set after the second compile has started: it stops at once, and the third stand-in never starts.
        let scratch = Scratch::new("cancel");
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let mut fake = Fake::new(move |_, n| {
            if n == 1 {
                flag.store(true, Ordering::Relaxed);
            }
            (
                vec![
                    "   Compiling libc v0.2.178",
                    "   Compiling cobolt-ast v0.2.0 (x)",
                ],
                Ended::Success,
            )
        });
        let (report, _) = run(&mut fake, &scratch, true, true, &cancel);
        assert!(report.cancelled());
        assert_eq!(*report.end(Profile::Release), End::Cancelled);
        assert_eq!(*report.end(Profile::Debug), End::Cancelled);
        assert_eq!(fake.asked.len(), 2, "no unit starts after the cancel");
        assert!(!scratch.stamp("9.9.9", false) && !scratch.stamp("9.9.9", true));
        assert!(
            cache_created_by_setup(&scratch.builds()),
            "what the setup created stays marked so that it can be removed"
        );

        // Set while the last release stand-in compiles, which then finishes: release is complete and stamped,
        // and the debug profile never starts.
        let scratch = Scratch::new("cancel-between");
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let mut fake = Fake::new(move |_, n| {
            if n == 2 {
                // The flag is raised after this stand-in's last line, so it is not interrupted.
                flag.store(true, Ordering::Relaxed);
                return (vec![], Ended::Success);
            }
            (vec![], Ended::Success)
        });
        let (report, _) = run(&mut fake, &scratch, true, true, &cancel);
        assert_eq!(*report.end(Profile::Release), End::Done);
        assert_eq!(*report.end(Profile::Debug), End::Cancelled);
        assert_eq!(fake.asked.len(), 3);
        assert!(scratch.stamp("9.9.9", false) && !scratch.stamp("9.9.9", true));

        // Already cancelled before it began: nothing is compiled at all.
        let scratch = Scratch::new("cancel-first");
        let mut fake = Fake::building();
        let (report, _) = run(&mut fake, &scratch, true, true, &AtomicBool::new(true));
        assert!(report.cancelled() && fake.asked.is_empty());
        println!("cancel: mid-compile (2 of 6 started), between profiles (release stamped, debug not started), before it began (0 started)");
    }

    #[test]
    fn a_quiet_compile_says_it_is_still_running_no_more_often_than_asked() {
        let scratch = Scratch::new("beat");
        let sdk = scratch.sdk();
        let builds = scratch.builds();
        let cancel = AtomicBool::new(false);
        // A cargo that says nothing for five quiet moments in each stand-in.
        struct Quiet;
        impl CargoRunner for Quiet {
            fn build(&mut self, _: &CargoBuild<'_>, on: &mut dyn FnMut(Tick) -> Flow) -> Ended {
                for _ in 0..5 {
                    on(Tick::Quiet);
                }
                Ended::Success
            }
        }
        for (heartbeat, expected) in [(Duration::ZERO, 15), (Duration::from_secs(3600), 0)] {
            let p = Prepare {
                sdk_root: &sdk,
                build_root: &builds,
                version: "9.9.9",
                release: true,
                debug: false,
                cancel: &cancel,
                heartbeat,
            };
            let mut beats = 0;
            let _ = prepare_platform_with(
                &mut Quiet,
                &p,
                &mut |g| beats += usize::from(g.event == Event::Beat),
                &mut |_| {},
            );
            assert_eq!(beats, expected, "heartbeat {heartbeat:?}");
        }
    }

    #[test]
    fn a_cache_that_cannot_be_opened_fails_every_profile_asked_for() {
        let scratch = Scratch::new("noopen");
        // A build folder that is a file: nothing can be created under it.
        let blocker = scratch.0.join("builds");
        std::fs::write(&blocker, "x").unwrap();
        let mut fake = Fake::building();
        let (report, _) = run(&mut fake, &scratch, true, false, &AtomicBool::new(false));
        assert!(report.cache.is_none() && !report.created_cache && fake.asked.is_empty());
        assert!(
            matches!(report.end(Profile::Release), End::Failed { why, .. } if why.contains("cannot create")),
            "{report:?}"
        );
        assert_eq!(*report.end(Profile::Debug), End::NotAsked);
    }

    #[test]
    fn a_stale_crate_is_cleaned_once_for_the_whole_run() {
        let graph_forms = ["cobolt-ast", "cobolt-runtime"];
        let graph_graphics = ["cobolt-ast", "cobolt-runtime", "cobolt-graphics"];
        let stale = ["cobolt-ast", "cobolt-graphics"];
        let mut cleaned = HashSet::new();
        let first = crate::warmup::to_clean(&stale, &graph_forms, &cleaned);
        assert_eq!(
            first,
            ["cobolt-ast"],
            "only what is in this stand-in's graph"
        );
        cleaned.extend(first);
        // The next stand-in must not drop what the first compiled again.
        let second = crate::warmup::to_clean(&stale, &graph_graphics, &cleaned);
        assert_eq!(
            second,
            ["cobolt-graphics"],
            "cobolt-ast was cleaned and compiled already"
        );
        cleaned.extend(second);
        assert!(crate::warmup::to_clean(&stale, &graph_graphics, &cleaned).is_empty());
    }

    /// One real stand-in program compiled by the real cargo into a scratch cache: the streaming, the counter
    /// and the stamp against what cargo really says. It compiles the platform, which takes minutes and about a
    /// gigabyte, so it only runs when asked: `cargo test -p cobolt-compiler warmup -- --ignored --nocapture one_real_variant`.
    #[test]
    #[ignore = "compiles the platform for real: minutes and about a gigabyte"]
    fn one_real_variant() {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let sdk = sdk.canonicalize().unwrap();
        let scratch = Scratch::new("real");
        let builds = scratch.builds();
        let cancel = AtomicBool::new(false);
        let p = Prepare {
            sdk_root: &sdk,
            build_root: &builds,
            version: "0.0.0-real",
            release: true,
            debug: false,
            cancel: &cancel,
            heartbeat: Prepare::HEARTBEAT,
        };
        let stand = variants()[0].0;
        let (mut crates, mut beats, mut first) = (0, 0, None);
        let began = Instant::now();
        // Only the first stand-in: stop as soon as the second begins.
        let report = prepare_platform(
            &p,
            &mut |g| {
                if g.variant != stand {
                    cancel.store(true, Ordering::Relaxed);
                    return;
                }
                crates = g.crates;
                beats += usize::from(g.event == Event::Beat);
                first.get_or_insert(g.elapsed);
            },
            &mut |line| println!("{line}"),
        );
        println!(
            "one real variant ({stand}, release): {crates} crates compiled, {beats} heartbeats, {:.0} s, first event at {:?}, ends {:?}",
            began.elapsed().as_secs_f64(),
            first,
            report.ends
        );
        assert!(crates > 0, "the counter saw nothing: {report:?}");
    }
}
