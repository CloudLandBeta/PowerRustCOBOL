// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What only the machine knows, gathered for the decision, and the installer properties the page reads (R1, R3,
//! R4, R16; task T11).
//!
//! [`gather`] asks a [`Probe`] four questions of the installing user's machine (is `rustup` there and does it list
//! the pinned Rust; does `vswhere` find the C++ build tools; which stamps of this version are in the default build
//! folder; how much room is on the two volumes the setup writes to) and hands the answers to
//! [`prepare::decide`](crate::prepare::decide). The probe is a trait so that what the page says for each situation
//! is tested with recorded answers on every operating system; the real one is the Windows one in
//! [`system`](crate::facts::system).
//!
//! **What is gathered here is a hint for the page.** It decides what is offered and what is said; the check after
//! the installs, with the link probe, stays the authority (R7). So every question that cannot be answered is
//! answered in the way that never blocks: an unknown amount of free space is plenty (the compile itself reports a
//! full disk in a sentence), a missing `vswhere` is a missing linker (the step that installs it is told to check
//! again), and a build folder that is not there has no stamps.

use crate::disk::{Free, Volume};
use crate::prepare::{Facts, Linker, Mode, Offer, Plan, RustAction, RustState, Steps};

/// The Visual Studio component that carries the C++ compiler and linker for x64 (`vswhere -requires`).
pub const VSWHERE_COMPONENT: &str = "Microsoft.VisualStudio.Component.VC.Tools.x86.x64";

/// The two folders under the user's local application-data folder that are the default build folder. The IDE's
/// own `default_build_root` ends the same way, and a test in `cobolt-ide` holds the two equal.
pub const BUILD_FOLDER: [&str; 2] = ["PowerRustCOBOL", "builds"];

/// Where the compiled-crate cache and the stamps of the platform live under the build folder: the cache is
/// `shared-target` and the stamps are its `warm` folder. A test in `cobolt-ide` writes real stamps with the
/// compiler and reads them back through this.
pub const STAMP_FOLDER: [&str; 2] = ["shared-target", "warm"];

/// What the machine can be asked. Paths are Windows paths, as text.
pub trait Probe {
    /// Run `program` with `args` as the installing user and answer what it printed on standard output, or `None`
    /// when it could not be run or did not end well.
    fn output(&mut self, program: &str, args: &[&str]) -> Option<String>;
    /// The names of the files in a folder, or `None` when the folder is not there.
    fn files_in(&mut self, dir: &str) -> Option<Vec<String>>;
    /// The volume that holds `path` (even when the path itself is not there yet) and its free space, or `None`
    /// when that cannot be told.
    fn free_on(&mut self, path: &str) -> Option<Volume>;
    /// An environment variable of the installing user.
    fn var(&mut self, name: &str) -> Option<String>;
}

/// What the installer itself knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    /// The version being installed.
    pub version: String,
    /// The Rust release this version is built with.
    pub pinned_rust: String,
    /// A different version installed now, which this replaces.
    pub previous_version: Option<String>,
    /// Interactive or silent.
    pub mode: Mode,
}

// ── Rust ─────────────────────────────────────────────────────────────────────

/// Whether the output of `rustup toolchain list` (`1.95.0-x86_64-pc-windows-msvc (default)`) names `release`.
/// The IDE's `warmup::lists_toolchain` answers the same way, and a test in `cobolt-ide` holds the two equal.
pub fn lists_toolchain(list: &str, release: &str) -> bool {
    list.lines().any(|line| {
        line.trim()
            .strip_prefix(release)
            .is_some_and(|rest| rest.starts_with('-'))
    })
}

/// What is there of Rust, from what `rustup toolchain list` printed (`None`: there is no `rustup` to ask).
pub fn rust_state(list: Option<&str>, pinned: &str) -> RustState {
    match list {
        None => RustState::NoRustup,
        Some(list) if lists_toolchain(list, pinned) => RustState::Pinned,
        Some(_) => RustState::RustupWithoutPinned,
    }
}

/// The places `rustup` is looked for, in order: the user's `PATH` first (a developer who put one there means it),
/// then the two standard homes rustup installs to, which a session that started before the install does not have
/// on its `PATH` yet.
fn rustup_candidates(probe: &mut dyn Probe) -> Vec<String> {
    let mut out = vec!["rustup".to_owned()];
    for bin in [
        probe.var("CARGO_HOME").map(|h| join(&h, &["bin", "rustup.exe"])),
        probe
            .var("USERPROFILE")
            .map(|h| join(&h, &[".cargo", "bin", "rustup.exe"])),
    ]
    .into_iter()
    .flatten()
    {
        if !out.contains(&bin) {
            out.push(bin);
        }
    }
    out
}

fn rustup_toolchains(probe: &mut dyn Probe) -> Option<String> {
    rustup_candidates(probe)
        .iter()
        .find_map(|rustup| probe.output(rustup, &["toolchain", "list"]))
}

// ── The linker ───────────────────────────────────────────────────────────────

/// Whether `vswhere` found an installation that has the C++ tools: it prints the installation's path, one line
/// each, and nothing when there is none. A line that is not a path (an error text, a banner) is not an answer.
pub fn parse_vswhere(output: &str) -> Linker {
    let found = output
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .any(looks_like_windows_path);
    if found {
        Linker::Present
    } else {
        Linker::Missing
    }
}

fn looks_like_windows_path(line: &str) -> bool {
    let b = line.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || line.starts_with("\\\\")
}

fn linker(probe: &mut dyn Probe) -> Linker {
    let Some(program_files) = probe
        .var("ProgramFiles(x86)")
        .or_else(|| probe.var("ProgramFiles"))
    else {
        return Linker::Missing;
    };
    let vswhere = join(
        &program_files,
        &["Microsoft Visual Studio", "Installer", "vswhere.exe"],
    );
    probe
        .output(
            &vswhere,
            &[
                "-latest",
                "-products",
                "*",
                "-requires",
                VSWHERE_COMPONENT,
                "-property",
                "installationPath",
            ],
        )
        .map_or(Linker::Missing, |out| parse_vswhere(&out))
}

// ── The stamps and the volumes ───────────────────────────────────────────────

/// Whether the release and the debug stamps of `version` are among the files of the stamp folder.
pub fn stamps(version: &str, names: &[String]) -> (bool, bool) {
    let has = |name: &str| names.iter().any(|n| n.eq_ignore_ascii_case(name));
    (has(version), has(&format!("{version}-debug")))
}

/// The default build folder of the installing user.
pub fn build_folder(local_app_data: &str) -> String {
    join(local_app_data, &BUILD_FOLDER)
}

/// The folder that holds the stamps, for a build folder.
pub fn stamp_folder(build_folder: &str) -> String {
    join(build_folder, &STAMP_FOLDER)
}

fn join(base: &str, parts: &[&str]) -> String {
    let mut out = base.trim_end_matches(['\\', '/']).to_owned();
    for part in parts {
        out.push('\\');
        out.push_str(part);
    }
    out
}

/// The name of the volume a path is on: `C:` for `c:\Users\...`, the share for a UNC path.
pub fn volume_name(path: &str) -> String {
    let b = path.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return format!("{}:", (b[0] as char).to_ascii_uppercase());
    }
    if let Some(rest) = path.strip_prefix("\\\\") {
        let mut parts = rest.split(['\\', '/']).filter(|p| !p.is_empty());
        if let (Some(server), Some(share)) = (parts.next(), parts.next()) {
            return format!("\\\\{server}\\{share}");
        }
    }
    path.to_owned()
}

fn volume(probe: &mut dyn Probe, path: &str) -> Volume {
    probe.free_on(path).unwrap_or_else(|| Volume {
        name: volume_name(path),
        // Not knowing how much room there is never blocks the offer: the compile reports a full disk itself.
        free_bytes: u64::MAX,
    })
}

// ── Gathering ────────────────────────────────────────────────────────────────

/// Ask the machine what the decision needs.
pub fn gather(probe: &mut dyn Probe, inputs: &Inputs) -> Facts {
    let rust = rust_state(rustup_toolchains(probe).as_deref(), &inputs.pinned_rust);
    let linker = linker(probe);

    let local = probe
        .var("LOCALAPPDATA")
        .or_else(|| {
            probe
                .var("USERPROFILE")
                .map(|u| join(&u, &["AppData", "Local"]))
        })
        .unwrap_or_else(|| "C:\\".to_owned());
    let build = build_folder(&local);
    let (release_ready, debug_ready) = probe
        .files_in(&stamp_folder(&build))
        .map_or((false, false), |names| stamps(&inputs.version, &names));

    let system_root = join(
        &probe.var("SystemDrive").unwrap_or_else(|| "C:".to_owned()),
        &[],
    ) + "\\";
    let free = Free {
        build: volume(probe, &build),
        system: volume(probe, &system_root),
    };

    Facts {
        version: inputs.version.clone(),
        pinned_rust: inputs.pinned_rust.clone(),
        previous_version: inputs.previous_version.clone(),
        rust,
        linker,
        release_ready,
        debug_ready,
        free,
        mode: inputs.mode,
    }
}

// ── The installer properties ─────────────────────────────────────────────────

/// Windows Installer reads `[Name]` in a property as a reference to another one, so a page text must not carry
/// square brackets.
fn msi_safe(text: &str) -> String {
    text.replace('[', "(").replace(']', ")")
}

/// The sentence the cancel confirmation gets when *Prepare now* is chosen: what a Cancel leaves installed (R14).
/// Empty when the setup would only compile, because then there is nothing of its own to leave.
pub fn cancel_note(steps: &Steps) -> String {
    let rust = steps.rust != RustAction::Nothing;
    match (rust, steps.build_tools) {
        (true, true) => "Rust and the Visual Studio Build Tools stay installed if this setup has already installed them.".to_owned(),
        (true, false) => "Rust stays installed if this setup has already installed it.".to_owned(),
        (false, true) => "The Visual Studio Build Tools stay installed if this setup has already installed them.".to_owned(),
        (false, false) => String::new(),
    }
}

/// The properties `PrcFacts` sets for the page, as `(name, value)`. A flag is `1` or empty (empty is false in a
/// condition and removes the property).
///
/// * `PRC_TEXT`: the page's text.
/// * `PRC_NEED`: there is something to prepare (the page is shown).
/// * `PRC_DISK_OK`: *Prepare now* is offered; empty when a volume falls short.
/// * `PRC_ALREADY`: this version is already prepared for this user.
/// * `PRC_CANCEL_TEXT`: the sentence the page copies into `PRC_CANCEL_NOTE` when *Prepare now* is chosen.
pub fn properties(plan: &Plan) -> Vec<(&'static str, String)> {
    let flag = |on: bool| if on { "1".to_owned() } else { String::new() };
    vec![
        ("PRC_TEXT", msi_safe(&plan.text)),
        ("PRC_NEED", flag(!matches!(plan.offer, Offer::AlreadyPrepared))),
        ("PRC_DISK_OK", flag(plan.offer == Offer::PrepareNow)),
        ("PRC_ALREADY", flag(plan.offer == Offer::AlreadyPrepared)),
        ("PRC_CANCEL_TEXT", msi_safe(&cancel_note(&plan.steps))),
    ]
}

/// The property a silent install sets for the three actions (R5, AC2): `PRC_PREPARE` is `1` only with the opt-in
/// (`PREPARE_PLATFORM=1`) and only where *Prepare now* would have been offered. A wizard sets it from the page's
/// radio button instead, so nothing is returned for it.
pub fn prepare_property(plan: &Plan, mode: Mode) -> Option<(&'static str, String)> {
    match mode {
        Mode::Interactive => None,
        Mode::Silent { .. } => Some((
            "PRC_PREPARE",
            if plan.runs(mode, None) { "1".to_owned() } else { String::new() },
        )),
    }
}

// ── The real machine ─────────────────────────────────────────────────────────

#[cfg(windows)]
pub mod system;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{FIGURES, GB};
    use crate::prepare::decide;
    use std::collections::HashMap;

    const VERSION: &str = "2.1.24";
    const PINNED: &str = "1.95.0";
    const BUILD_TOOLS_PATH: &str =
        "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\r\n";

    /// Recorded answers; every call is written down.
    #[derive(Default)]
    struct Fake {
        vars: HashMap<&'static str, String>,
        /// What each program prints (the first element of the key is the program).
        printed: HashMap<String, String>,
        folders: HashMap<String, Vec<String>>,
        free: HashMap<String, Volume>,
        asked: Vec<String>,
    }

    impl Probe for Fake {
        fn output(&mut self, program: &str, args: &[&str]) -> Option<String> {
            self.asked.push(format!("{program} {}", args.join(" ")));
            self.printed.get(program).cloned()
        }
        fn files_in(&mut self, dir: &str) -> Option<Vec<String>> {
            self.folders.get(dir).cloned()
        }
        fn free_on(&mut self, path: &str) -> Option<Volume> {
            self.free.get(path).cloned()
        }
        fn var(&mut self, name: &str) -> Option<String> {
            self.vars.get(name).cloned()
        }
    }

    fn inputs() -> Inputs {
        Inputs {
            version: VERSION.into(),
            pinned_rust: PINNED.into(),
            previous_version: None,
            mode: Mode::Interactive,
        }
    }

    const LOCAL: &str = "C:\\Users\\dev\\AppData\\Local";

    fn build() -> String {
        build_folder(LOCAL)
    }

    /// A machine in the situation the decision's table names.
    fn world(
        rust: RustState,
        linker: Linker,
        stamps: (bool, bool),
        disk: (u64, u64, bool),
    ) -> Fake {
        let mut f = Fake::default();
        f.vars.insert("LOCALAPPDATA", LOCAL.into());
        f.vars.insert("USERPROFILE", "C:\\Users\\dev".into());
        f.vars
            .insert("ProgramFiles(x86)", "C:\\Program Files (x86)".into());
        f.vars.insert("SystemDrive", "C:".into());
        match rust {
            RustState::Pinned => {
                f.printed.insert(
                    "rustup".into(),
                    "stable-x86_64-pc-windows-msvc (default)\r\n1.95.0-x86_64-pc-windows-msvc\r\n".into(),
                );
            }
            RustState::RustupWithoutPinned => {
                f.printed.insert(
                    "rustup".into(),
                    "stable-x86_64-pc-windows-msvc (default)\r\n1.90-x86_64-pc-windows-msvc\r\n".into(),
                );
            }
            RustState::NoRustup => {}
        }
        let vswhere = "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe";
        f.printed.insert(
            vswhere.into(),
            if linker == Linker::Present {
                BUILD_TOOLS_PATH.into()
            } else {
                String::new()
            },
        );
        let mut names = vec!["2.0.1".to_owned()];
        if stamps.0 {
            names.push(VERSION.to_owned());
        }
        if stamps.1 {
            names.push(format!("{VERSION}-debug"));
        }
        f.folders.insert(stamp_folder(&build()), names);
        let (b, s, same) = disk;
        f.free.insert(
            build(),
            Volume {
                name: "C:".into(),
                free_bytes: b * GB,
            },
        );
        f.free.insert(
            "C:\\".into(),
            Volume {
                name: if same { "C:" } else { "D:" }.into(),
                free_bytes: s * GB,
            },
        );
        f
    }

    fn hand_made(
        rust: RustState,
        linker: Linker,
        stamps: (bool, bool),
        disk: (u64, u64, bool),
    ) -> Facts {
        let (b, s, same) = disk;
        Facts {
            version: VERSION.into(),
            pinned_rust: PINNED.into(),
            previous_version: None,
            rust,
            linker,
            release_ready: stamps.0,
            debug_ready: stamps.1,
            free: Free {
                build: Volume {
                    name: "C:".into(),
                    free_bytes: b * GB,
                },
                system: Volume {
                    name: if same { "C:" } else { "D:" }.into(),
                    free_bytes: s * GB,
                },
            },
            mode: Mode::Interactive,
        }
    }

    #[test]
    fn the_toolchain_list_is_read_as_rustup_prints_it() {
        let both = "stable-x86_64-pc-windows-msvc (default)\r\n1.95.0-x86_64-pc-windows-msvc\r\n";
        let other = "stable-x86_64-pc-windows-msvc (default)\r\n1.92-x86_64-pc-windows-msvc\r\n";
        let none = "no installed toolchains\r\n";
        let lookalike = "1.95.01-x86_64-pc-windows-msvc\r\n1.95-x86_64-pc-windows-msvc\r\n";
        let corrupt = "\u{0}\u{0}garbage ((( \r\n1.95.0\r\n";
        let cases = [
            (Some(both), RustState::Pinned),
            (Some(other), RustState::RustupWithoutPinned),
            (Some(none), RustState::RustupWithoutPinned),
            (Some(""), RustState::RustupWithoutPinned),
            (Some(lookalike), RustState::RustupWithoutPinned),
            (Some(corrupt), RustState::RustupWithoutPinned),
            (None, RustState::NoRustup),
        ];
        for (list, expected) in cases {
            assert_eq!(rust_state(list, PINNED), expected, "{list:?}");
        }
        println!(
            "rustup toolchain list: {} outputs read (found, other versions, none, empty, look-alikes, corrupt, no rustup)",
            cases.len()
        );
    }

    #[test]
    fn vswhere_found_none_and_a_line_that_is_not_a_path() {
        let found = [
            "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\r\n",
            "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\n",
            "\u{feff}D:\\VS\\BuildTools",
            "  c:/vs/buildtools  \n",
            "\\\\server\\share\\vs\n",
            "banner\r\nC:\\VS\\BuildTools\r\n",
            "C:\\VS\\2019\\BuildTools\r\nC:\\VS\\2022\\BuildTools\r\n",
        ];
        let not_found = [
            "",
            "\r\n",
            "   \n  \n",
            "Error 0x57: The parameter is incorrect.\r\n",
            "installationPath\r\n",
            "C:\r\n",
            "1.95.0\r\n",
        ];
        for text in found {
            assert_eq!(parse_vswhere(text), Linker::Present, "{text:?}");
        }
        for text in not_found {
            assert_eq!(parse_vswhere(text), Linker::Missing, "{text:?}");
        }
    }

    #[test]
    fn the_stamps_are_this_versions_and_a_prefix_or_another_version_is_not() {
        let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(stamps("2.1.24", &names(&[])), (false, false));
        assert_eq!(stamps("2.1.24", &names(&["2.1.24"])), (true, false));
        assert_eq!(stamps("2.1.24", &names(&["2.1.24-debug"])), (false, true));
        assert_eq!(
            stamps("2.1.24", &names(&["2.1.24", "2.1.24-debug", "2.1.9"])),
            (true, true)
        );
        // 2.1.2 is not 2.1.24, and 2.1.24 is not 2.1.240.
        assert_eq!(stamps("2.1.2", &names(&["2.1.24", "2.1.24-debug"])), (false, false));
        assert_eq!(stamps("2.1.24", &names(&["2.1.240", "2.1.240-debug"])), (false, false));
        assert_eq!(stamps("2.1.24", &names(&["2.1.24-DEBUG"])), (false, true));
    }

    #[test]
    fn paths_and_volumes_are_named_the_way_windows_names_them() {
        assert_eq!(build_folder("C:\\Users\\dev\\AppData\\Local"), "C:\\Users\\dev\\AppData\\Local\\PowerRustCOBOL\\builds");
        assert_eq!(build_folder("D:\\Cache\\"), "D:\\Cache\\PowerRustCOBOL\\builds");
        assert_eq!(
            stamp_folder("D:\\Cache\\PowerRustCOBOL\\builds"),
            "D:\\Cache\\PowerRustCOBOL\\builds\\shared-target\\warm"
        );
        assert_eq!(volume_name("c:\\Users\\dev"), "C:");
        assert_eq!(volume_name("D:/x"), "D:");
        assert_eq!(volume_name("\\\\nas\\builds\\dev"), "\\\\nas\\builds");
        assert_eq!(volume_name("relative"), "relative");
    }

    #[test]
    fn every_world_the_decision_knows_is_gathered_as_the_decision_was_tested_on_it() {
        let rusts = [
            RustState::Pinned,
            RustState::RustupWithoutPinned,
            RustState::NoRustup,
        ];
        let linkers = [Linker::Present, Linker::Missing];
        let stamp_sets = [(false, false), (true, false), (false, true), (true, true)];
        let disks = [
            (500, 500, true),
            (3, 500, true),
            (500, 3, false),
            (500, 500, false),
        ];
        let (mut worlds, mut offered, mut low, mut already) = (0, 0, 0, 0);
        for rust in rusts {
            for linker in linkers {
                for stamps in stamp_sets {
                    for disk in disks {
                        let mut machine = world(rust, linker, stamps, disk);
                        let gathered = gather(&mut machine, &inputs());
                        let expected = hand_made(rust, linker, stamps, disk);
                        assert_eq!(gathered, expected, "{rust:?} {linker:?} {stamps:?} {disk:?}");
                        worlds += 1;

                        // And what the page then says, and the properties it reads, agree with the offer.
                        let plan = decide(&gathered, &FIGURES);
                        let props: HashMap<_, _> = properties(&plan).into_iter().collect();
                        assert_eq!(props["PRC_TEXT"], plan.text, "no square bracket to rewrite");
                        assert!(
                            !props.values().any(|v| v.contains('[') || v.contains(']')),
                            "Windows Installer would read it as a reference: {props:?}"
                        );
                        match plan.offer {
                            Offer::PrepareNow => {
                                offered += 1;
                                assert_eq!((props["PRC_NEED"].as_str(), props["PRC_DISK_OK"].as_str(), props["PRC_ALREADY"].as_str()), ("1", "1", ""));
                            }
                            Offer::DiskLow(_) => {
                                low += 1;
                                assert_eq!((props["PRC_NEED"].as_str(), props["PRC_DISK_OK"].as_str(), props["PRC_ALREADY"].as_str()), ("1", "", ""));
                            }
                            Offer::AlreadyPrepared => {
                                already += 1;
                                assert_eq!((props["PRC_NEED"].as_str(), props["PRC_DISK_OK"].as_str(), props["PRC_ALREADY"].as_str()), ("", "", "1"));
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(worlds, 96);
        println!(
            "facts gathered from recorded machines: {worlds} worlds equal the decision's own table ({offered} offered Prepare now, {low} found a volume short, {already} already prepared)"
        );
    }

    #[test]
    fn a_machine_that_answers_nothing_is_offered_the_work_and_is_never_blocked() {
        let mut nothing = Fake::default();
        let facts = gather(&mut nothing, &inputs());
        assert_eq!(facts.rust, RustState::NoRustup);
        assert_eq!(facts.linker, Linker::Missing);
        assert!(!facts.release_ready && !facts.debug_ready);
        assert_eq!(facts.free.build.free_bytes, u64::MAX, "room that cannot be measured is not a shortfall");
        assert_eq!(facts.free.system.free_bytes, u64::MAX);
        assert_eq!(facts.free.system.name, "C:");
        let plan = decide(&facts, &FIGURES);
        assert_eq!(plan.offer, Offer::PrepareNow, "{}", plan.text);
    }

    #[test]
    fn rustup_is_looked_for_on_the_path_then_where_rustup_installs_itself() {
        // Not on the PATH of this session, but in CARGO_HOME\bin: found.
        let mut f = Fake::default();
        f.vars.insert("CARGO_HOME", "D:\\cargo".into());
        f.vars.insert("USERPROFILE", "C:\\Users\\dev".into());
        f.printed.insert(
            "D:\\cargo\\bin\\rustup.exe".into(),
            "1.95.0-x86_64-pc-windows-msvc\r\n".into(),
        );
        f.printed.insert(
            "C:\\Users\\dev\\.cargo\\bin\\rustup.exe".into(),
            "stable-x86_64-pc-windows-msvc (default)\r\n".into(),
        );
        let facts = gather(&mut f, &inputs());
        assert_eq!(facts.rust, RustState::Pinned, "CARGO_HOME wins over the profile's .cargo");
        let tried: Vec<&str> = f.asked.iter().filter(|a| a.ends_with("toolchain list")).map(String::as_str).collect();
        assert_eq!(
            tried,
            [
                "rustup toolchain list",
                "D:\\cargo\\bin\\rustup.exe toolchain list"
            ],
            "the first answer ends the search"
        );

        // Only the profile's .cargo has it.
        let mut f = Fake::default();
        f.vars.insert("USERPROFILE", "C:\\Users\\dev".into());
        f.printed.insert(
            "C:\\Users\\dev\\.cargo\\bin\\rustup.exe".into(),
            "stable-x86_64-pc-windows-msvc (default)\r\n".into(),
        );
        assert_eq!(gather(&mut f, &inputs()).rust, RustState::RustupWithoutPinned);

        // CARGO_HOME naming the same place as the profile is asked once.
        let mut f = Fake::default();
        f.vars.insert("CARGO_HOME", "C:\\Users\\dev\\.cargo\\".into());
        f.vars.insert("USERPROFILE", "C:\\Users\\dev".into());
        let _ = gather(&mut f, &inputs());
        assert_eq!(f.asked.iter().filter(|a| a.ends_with("toolchain list")).count(), 2);
    }

    #[test]
    fn the_build_folder_on_another_volume_is_judged_on_its_own_room() {
        let mut f = world(RustState::NoRustup, Linker::Missing, (false, false), (500, 500, false));
        // The local application-data folder moved to D: (a redirected profile); the system drive is still C:.
        f.vars.insert("LOCALAPPDATA", "D:\\Profiles\\dev\\AppData\\Local".into());
        let d_build = build_folder("D:\\Profiles\\dev\\AppData\\Local");
        f.free.insert(d_build, Volume { name: "D:".into(), free_bytes: 3 * GB });
        f.free.insert("C:\\".into(), Volume { name: "C:".into(), free_bytes: 500 * GB });
        let facts = gather(&mut f, &inputs());
        assert_eq!((facts.free.build.name.as_str(), facts.free.system.name.as_str()), ("D:", "C:"));
        let plan = decide(&facts, &FIGURES);
        let Offer::DiskLow(short) = &plan.offer else {
            panic!("D: has 3 GB and the compile needs more: {:?}", plan.offer)
        };
        assert_eq!(short.iter().map(|s| s.volume.as_str()).collect::<Vec<_>>(), ["D:"], "C: has room and is not named");
    }

    #[test]
    fn the_sentence_a_cancel_adds_names_only_what_this_setup_would_install() {
        use crate::prepare::Steps;
        let steps = |rust, build_tools| Steps { rust, build_tools, release: true, debug: true };
        let both = cancel_note(&steps(RustAction::InstallFresh, true));
        assert!(both.contains("Rust") && both.contains("Build Tools") && both.contains("has already installed"), "{both}");
        let rust = cancel_note(&steps(RustAction::InstallBeside, false));
        assert!(rust.contains("Rust") && !rust.contains("Build Tools"), "{rust}");
        let tools = cancel_note(&steps(RustAction::Nothing, true));
        assert!(tools.contains("Build Tools") && !tools.contains("Rust"), "{tools}");
        assert_eq!(cancel_note(&steps(RustAction::Nothing, false)), "", "only a compile: nothing of its own to leave");
        for note in [both, rust, tools] {
            assert!(note.is_ascii() && !note.contains(['[', ']', '\r', '\n']), "{note}");
        }
    }

    #[test]
    fn a_silent_install_prepares_only_with_the_opt_in_and_only_where_it_would_be_offered() {
        let plan_for = |disk: (u64, u64, bool), stamps: (bool, bool)| {
            decide(&gather(&mut world(RustState::NoRustup, Linker::Missing, stamps, disk), &inputs()), &FIGURES)
        };
        let offered = plan_for((500, 500, true), (false, false));
        let low = plan_for((3, 500, true), (false, false));
        let done = plan_for((500, 500, true), (true, true));
        assert_eq!(offered.offer, Offer::PrepareNow);
        let value = |plan: &Plan, mode| prepare_property(plan, mode);

        // A wizard sets the property from the page: nothing is set here.
        assert_eq!(value(&offered, Mode::Interactive), None);
        // Without the opt-in a silent install downloads and compiles nothing (R5, AC2).
        assert_eq!(value(&offered, Mode::Silent { opt_in: false }), Some(("PRC_PREPARE", String::new())));
        // With it, where Prepare now is offered, it prepares.
        assert_eq!(value(&offered, Mode::Silent { opt_in: true }), Some(("PRC_PREPARE", "1".to_owned())));
        // The opt-in does not override a full disk, and there is nothing to do for a version already prepared.
        assert_eq!(value(&low, Mode::Silent { opt_in: true }), Some(("PRC_PREPARE", String::new())));
        assert_eq!(value(&done, Mode::Silent { opt_in: true }), Some(("PRC_PREPARE", String::new())));
    }

    #[test]
    fn the_stamp_folder_and_the_build_folder_are_the_names_the_compiler_uses() {
        // The compiler's own constants are private; `cobolt-ide`'s test writes real stamps and reads them back.
        assert_eq!(BUILD_FOLDER, ["PowerRustCOBOL", "builds"]);
        assert_eq!(STAMP_FOLDER, ["shared-target", "warm"]);
    }
}
