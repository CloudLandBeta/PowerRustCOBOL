// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What the setup's preparation does, decided from facts, and what its page says (R1 to R6, R16).
//!
//! This is the one place the decision is made. The wizard page (its text and
//! whether *Prepare now* is offered), the silent install and the headless mode
//! of the IDE that does the work all ask it, so what a developer is told and
//! what then happens cannot drift apart. It is plain functions over plain
//! values: no Windows call, no process, no file, tested on every operating system.
//!
//! The facts are gathered elsewhere (`vswhere`, `rustup toolchain list`, the
//! free space of two volumes, the stamps of this version); here they only arrive.
//! The detection that gathers them is a *hint for the page*. The check after the
//! installs, with the link probe, stays the authority (R7).

use crate::disk::{self, Free, Needs, Shortfall};
use crate::figures::{duration_words, human, Figures};

/// Where Rust comes from, and Microsoft's page for the Build Tools and their terms. The first two are shown
/// to the developer before anything is downloaded (R3).
pub const RUSTUP_URL: &str = "https://rustup.rs";
/// Microsoft's page for the C++ build tools; the IDE offers the same page (`linker_download`).
pub const BUILD_TOOLS_URL: &str = "https://visualstudio.microsoft.com/visual-cpp-build-tools/";
/// Microsoft's licence terms page for Visual Studio products. TO BE CHECKED (plan R-9) against the link the
/// bootstrapper's own wizard shows before the setup is released: it must be the page that lists the Build Tools'
/// terms, and it is never typed from memory.
pub const MICROSOFT_TERMS_URL: &str = "https://visualstudio.microsoft.com/license-terms/";

/// The file the platform's compile writes in the compiled-crate cache when the setup created that cache, and only
/// then: what tells a cancelled setup that it may remove the folder (R14). The DLL reads it without linking the
/// compiler, so the name is spelled here as well, and a test in the IDE holds the two equal.
pub const CREATED_BY_SETUP: &str = ".prepared-by-setup";

/// What is on the machine of Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustState {
    /// The release's pinned Rust is installed (`rustup toolchain list` names it).
    Pinned,
    /// `rustup` is there, the pinned Rust is not: it is installed beside the developer's own (R8).
    RustupWithoutPinned,
    /// There is no `rustup`: it is installed with the pinned Rust as the default (R8).
    NoRustup,
}

/// Whether a working linker is there (the page's hint; the link probe after the install decides).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linker {
    /// The C++ build tools are installed.
    Present,
    /// They are not.
    Missing,
}

/// How the setup is being run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// With the wizard: the developer answers on the page.
    Interactive,
    /// `/qn`, `/passive`: nobody can answer. `opt_in` is `PREPARE_PLATFORM=1`, which is the choice and the
    /// acceptance of the Microsoft terms (R5).
    Silent {
        /// Whether `PREPARE_PLATFORM=1` was given.
        opt_in: bool,
    },
}

/// Everything the decision needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The version being installed (`2.1.24`).
    pub version: String,
    /// The Rust release this version is built with (`1.95.0`).
    pub pinned_rust: String,
    /// A different version of PowerRustCOBOL installed now, which this one replaces (R16).
    pub previous_version: Option<String>,
    /// Rust.
    pub rust: RustState,
    /// The C++ build tools.
    pub linker: Linker,
    /// Whether this version's stamp for the release profile is in the developer's build folder.
    pub release_ready: bool,
    /// Whether this version's stamp for the debug profile is in the developer's build folder.
    pub debug_ready: bool,
    /// Free space where the setup writes.
    pub free: Free,
    /// Interactive or silent.
    pub mode: Mode,
}

/// What is done about Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustAction {
    /// Nothing: the pinned Rust is there.
    Nothing,
    /// `rustup toolchain install`, beside the developer's own.
    InstallBeside,
    /// `rustup-init`, with the pinned Rust as the default.
    InstallFresh,
}

/// The steps *Prepare now* would run, in order.
///
/// The Visual Studio Build Tools are **not** a step: Windows Installer runs one installation at a time, and
/// Microsoft's installer refuses to start inside this one (it fails with 1618, spec 112 decision D12). The bundle
/// that carries this package installs them before it; the package only notices when they are missing, says so, and
/// does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps {
    /// Install Rust.
    pub rust: RustAction,
    /// The C++ build tools are missing and this setup does not install them, so it cannot compile. Never set for
    /// a version that is already prepared, which needs no compile.
    pub build_tools_missing: bool,
    /// The platform still has to be compiled for the release profile.
    pub release: bool,
    /// The platform still has to be compiled for the debug profile.
    pub debug: bool,
}

impl Steps {
    /// Whether this setup compiles: something is left to compile and there is a linker to compile with.
    pub fn compiles(&self) -> bool {
        (self.release || self.debug) && !self.build_tools_missing
    }

    /// Whether there is nothing to prepare at all: nothing to install and nothing left to compile.
    pub fn is_empty(&self) -> bool {
        self.rust == RustAction::Nothing && !self.release && !self.debug
    }

    /// Whether *Prepare now* would do anything here: install Rust, or compile.
    pub fn does_something(&self) -> bool {
        self.rust != RustAction::Nothing || self.compiles()
    }
}

/// What the page offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// *Prepare now* (the default) and *Skip*.
    PrepareNow,
    /// This version is already prepared for this developer: no compile is offered (R16).
    AlreadyPrepared,
    /// *Prepare now* is not offered: a volume cannot hold it (R4).
    DiskLow(Vec<Shortfall>),
    /// There is something to prepare but nothing this setup can do: Rust is there and the C++ build tools, which
    /// the compile needs and this setup does not install, are not. The page says so and where to get them.
    BuildToolsNeeded,
}

/// The decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// What the page offers.
    pub offer: Offer,
    /// What *Prepare now* would do.
    pub steps: Steps,
    /// The page's text, in English.
    pub text: String,
}

impl Plan {
    /// Whether the setup prepares, given the developer's choice on the page (`None` when no page was shown).
    ///
    /// A silent install prepares only with the opt-in and only where *Prepare now* would have been offered; a
    /// wizard prepares only on a yes. Nothing is ever downloaded or compiled otherwise (R2, R5, AC2).
    pub fn runs(&self, mode: Mode, chose_prepare_now: Option<bool>) -> bool {
        if self.offer != Offer::PrepareNow || !self.steps.does_something() {
            return false;
        }
        match mode {
            Mode::Interactive => chose_prepare_now == Some(true),
            Mode::Silent { opt_in } => opt_in,
        }
    }
}

/// The steps for these facts.
pub fn steps_for(facts: &Facts) -> Steps {
    // A version that is already prepared for both profiles needs nothing, whatever else is on the machine:
    // the compile is what the page is for (R16).
    let prepared = facts.release_ready && facts.debug_ready;
    if prepared {
        return Steps {
            rust: RustAction::Nothing,
            build_tools_missing: false,
            release: false,
            debug: false,
        };
    }
    Steps {
        rust: match facts.rust {
            RustState::Pinned => RustAction::Nothing,
            RustState::RustupWithoutPinned => RustAction::InstallBeside,
            RustState::NoRustup => RustAction::InstallFresh,
        },
        build_tools_missing: facts.linker == Linker::Missing,
        release: !facts.release_ready,
        debug: !facts.debug_ready,
    }
}

/// How much each volume must hold for these steps. The Build Tools are not one of them, so the system volume is
/// asked for nothing.
pub fn needs_for(steps: &Steps, figures: &Figures) -> Needs {
    let mut build = 0u64;
    if steps.rust != RustAction::Nothing {
        build += figures.rust_download_bytes + figures.rust_installed_bytes;
    }
    // Both profiles in the cache: a single profile takes its share of it. Without a linker nothing is compiled.
    if steps.compiles() {
        let profiles = u64::from(steps.release) + u64::from(steps.debug);
        build += figures.cache_bytes / 2 * profiles;
    }
    Needs {
        build_bytes: build,
        system_bytes: 0,
    }
}

/// Decide, and write the page.
pub fn decide(facts: &Facts, figures: &Figures) -> Plan {
    let steps = steps_for(facts);
    let shortfalls = disk::check(figures, &facts.free, &needs_for(&steps, figures));
    let offer = if steps.is_empty() {
        Offer::AlreadyPrepared
    } else if !steps.does_something() {
        Offer::BuildToolsNeeded
    } else if !shortfalls.is_empty() {
        Offer::DiskLow(shortfalls)
    } else {
        Offer::PrepareNow
    };
    let text = page_text(facts, figures, &steps, &offer);
    Plan { offer, steps, text }
}

/// What the page says about the Build Tools when they are missing: this setup does not install them (decision
/// D12), so it names them, where they are and what then happens.
fn build_tools_missing_text() -> String {
    format!(
        "The Visual Studio Build Tools (the \"Desktop development with C++\" part) are not installed, and this setup does not install them. Install them from Microsoft ({BUILD_TOOLS_URL}); Microsoft's installer asks you to accept its licence terms ({MICROSOFT_TERMS_URL}). After that, the first Debug or EXEC RUST Build asks first and then compiles PowerRustCOBOL AI's own parts."
    )
}

fn page_text(facts: &Facts, figures: &Figures, steps: &Steps, offer: &Offer) -> String {
    let mut t = String::new();
    let estimate = if figures.measured {
        ""
    } else {
        " (These figures are estimates.)"
    };

    // Only another version is "replaced": a repair or a reinstall of this one replaces nothing.
    if let Some(old) = facts
        .previous_version
        .as_ref()
        .filter(|old| **old != facts.version)
    {
        t.push_str(&format!(
            "This setup replaces version {old} with {}. The platform is compiled for each version, so it is prepared again for this one.\r\n\r\n",
            facts.version
        ));
    }

    if *offer == Offer::AlreadyPrepared {
        t.push_str(&format!(
            "PowerRustCOBOL AI {} is already prepared on this computer: Debug and EXEC RUST Builds start at once. There is nothing more to compile.\r\n",
            facts.version
        ));
        return t;
    }

    if *offer == Offer::BuildToolsNeeded {
        t.push_str(&format!(
            "Rust {} is already installed, and the platform's compile needs the Visual Studio Build Tools, so there is nothing this setup can prepare now.\r\n\r\n",
            facts.pinned_rust
        ));
        t.push_str(&build_tools_missing_text());
        t.push_str("\r\n");
        return t;
    }

    if steps.compiles() {
        t.push_str(&format!(
            "Take a seat and get a coffee. PowerRustCOBOL AI can get everything ready now, so that Debug and EXEC RUST Builds start at once and run at full speed. It takes {} on a modest machine and about {} of disk.{estimate}\r\n\r\n",
            duration_words(figures.compile_minutes),
            human(disk_total(steps, figures)),
        ));
    } else {
        t.push_str(&format!(
            "PowerRustCOBOL AI can install Rust {} now. It needs about {} of disk.{estimate} The platform itself is not compiled now, for the reason below.\r\n\r\n",
            facts.pinned_rust,
            human(disk_total(steps, figures)),
        ));
    }
    t.push_str(
        "If you choose Skip, nothing is lost: the first Debug or EXEC RUST Build asks first and then does the same work.\r\n\r\n",
    );

    if let Offer::DiskLow(shortfalls) = offer {
        t.push_str("Prepare now is not offered, because there is not enough free disk space:\r\n");
        for s in shortfalls {
            t.push_str(&format!(
                "  {} needs about {} and has {} free.\r\n",
                s.volume,
                human(s.need_bytes),
                human(s.free_bytes)
            ));
        }
        t.push_str("Free some space and run the setup again, or choose Skip.\r\n\r\n");
    }

    t.push_str("What Prepare now does, in order:\r\n");
    let mut n = 0;
    let mut item = |t: &mut String, text: String| {
        n += 1;
        t.push_str(&format!("  {n}. {text}\r\n"));
    };
    match steps.rust {
        RustAction::Nothing => item(&mut t, format!("Rust {} is already installed.", facts.pinned_rust)),
        RustAction::InstallBeside => item(
            &mut t,
            format!(
                "Installs Rust {} beside the Rust you already have, which stays your default. About {} to download, from the Rust project ({RUSTUP_URL}).",
                facts.pinned_rust,
                human(figures.rust_download_bytes)
            ),
        ),
        RustAction::InstallFresh => item(
            &mut t,
            format!(
                "Installs Rust {} with rustup. About {} to download, from the Rust project ({RUSTUP_URL}).",
                facts.pinned_rust,
                human(figures.rust_download_bytes)
            ),
        ),
    }
    if steps.build_tools_missing {
        item(&mut t, build_tools_missing_text());
    } else {
        item(
            &mut t,
            "The Visual Studio Build Tools (C++) are already installed.".to_owned(),
        );
        let compile = match (steps.release, steps.debug) {
            (true, true) => "Compiles PowerRustCOBOL AI's own parts for Release and for Debug, on this computer, at full speed.",
            (true, false) => "Compiles PowerRustCOBOL AI's own parts for Release, on this computer, at full speed. Debug is already compiled.",
            (false, true) => "Compiles PowerRustCOBOL AI's own parts for Debug, on this computer, at full speed. Release is already compiled.",
            (false, false) => "The compile is already done.",
        };
        item(&mut t, compile.to_owned());
    }

    t.push_str(
        "\r\nYou can press Cancel while this runs. Cancelling cancels the installation, and you are asked first.",
    );
    if steps.rust != RustAction::Nothing {
        t.push_str(" Rust stays installed if this setup has already installed it.");
    }
    t.push_str("\r\n");
    t
}

/// The disk the steps use in all, for the sentence on the page.
fn disk_total(steps: &Steps, figures: &Figures) -> u64 {
    let n = needs_for(steps, figures);
    n.build_bytes + n.system_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::Volume;
    use crate::figures::{FIGURES, GB};

    fn free(build_gb: u64, system_gb: u64, same: bool) -> Free {
        Free {
            build: Volume {
                name: "C:".into(),
                free_bytes: build_gb * GB,
            },
            system: Volume {
                name: if same { "C:" } else { "D:" }.into(),
                free_bytes: system_gb * GB,
            },
        }
    }

    fn facts(rust: RustState, linker: Linker, rel: bool, dbg: bool) -> Facts {
        Facts {
            version: "2.1.24".into(),
            pinned_rust: "1.95.0".into(),
            previous_version: None,
            rust,
            linker,
            release_ready: rel,
            debug_ready: dbg,
            free: free(500, 500, true),
            mode: Mode::Interactive,
        }
    }

    #[test]
    fn every_combination_of_what_is_there_decides_and_says_what_it_decided() {
        let rusts = [
            RustState::Pinned,
            RustState::RustupWithoutPinned,
            RustState::NoRustup,
        ];
        let linkers = [Linker::Present, Linker::Missing];
        let stamps = [(false, false), (true, false), (false, true), (true, true)];
        let disks = [
            (500, 500, true),
            (3, 500, true),
            (500, 3, false),
            (500, 500, false),
        ];
        let (mut cases, mut offered, mut prepared, mut low, mut tools) = (0, 0, 0, 0, 0);

        for rust in rusts {
            for linker in linkers {
                for (rel, dbg) in stamps {
                    for (b, s, same) in disks {
                        let mut f = facts(rust, linker, rel, dbg);
                        f.free = free(b, s, same);
                        let plan = decide(&f, &FIGURES);
                        cases += 1;

                        // The steps follow the facts one by one.
                        if rel && dbg {
                            assert_eq!(
                                plan.offer,
                                Offer::AlreadyPrepared,
                                "stamped for both: {rust:?} {linker:?}"
                            );
                            assert!(plan.steps.is_empty());
                            prepared += 1;
                            continue;
                        }
                        // The Build Tools are never installed by this setup: a missing one only stops the compile.
                        assert_eq!(plan.steps.build_tools_missing, linker == Linker::Missing);
                        assert_eq!(plan.steps.compiles(), linker == Linker::Present);
                        assert_eq!(plan.steps.release, !rel);
                        assert_eq!(plan.steps.debug, !dbg);
                        assert_eq!(
                            plan.steps.rust,
                            match rust {
                                RustState::Pinned => RustAction::Nothing,
                                RustState::RustupWithoutPinned => RustAction::InstallBeside,
                                RustState::NoRustup => RustAction::InstallFresh,
                            }
                        );

                        // The page offers Prepare now exactly when no volume falls short, and says why when it does not.
                        match &plan.offer {
                            Offer::PrepareNow => {
                                offered += 1;
                                assert!(
                                    plan.text.contains("What Prepare now does, in order"),
                                    "{}",
                                    plan.text
                                );
                            }
                            Offer::DiskLow(short) => {
                                low += 1;
                                assert!(!short.is_empty());
                                assert!(plan.text.contains("not offered"), "{}", plan.text);
                                for sf in short {
                                    assert!(
                                        plan.text.contains(&sf.volume),
                                        "the page names {}",
                                        sf.volume
                                    );
                                }
                            }
                            Offer::BuildToolsNeeded => {
                                tools += 1;
                                // Only when there is nothing else to do: Rust is there, the linker is not.
                                assert_eq!((rust, linker), (RustState::Pinned, Linker::Missing));
                                assert!(!plan.steps.does_something());
                                assert!(
                                    plan.text.contains("nothing this setup can prepare now"),
                                    "{}",
                                    plan.text
                                );
                                assert!(!plan.text.contains("What Prepare now does"));
                            }
                            Offer::AlreadyPrepared => panic!("not stamped, yet AlreadyPrepared"),
                        }
                        // What the page says matches what will be done: the Build Tools are never installed
                        // here, a missing one is named, and a present one is said to be there.
                        assert!(!plan.text.contains("Installs the Visual Studio Build Tools"));
                        assert_eq!(
                            plan.text.contains("this setup does not install them"),
                            linker == Linker::Missing,
                            "{}",
                            plan.text
                        );
                        assert_eq!(
                            plan.text.contains("Build Tools (C++) are already installed"),
                            linker == Linker::Present,
                            "{}",
                            plan.text
                        );
                        // A compile is promised only where it will happen.
                        assert_eq!(
                            plan.text.contains("get a coffee"),
                            plan.steps.compiles(),
                            "{}",
                            plan.text
                        );
                    }
                }
            }
        }
        println!(
            "preparation decision: {cases} combinations (3 Rust x 2 linker x 4 stamps x 4 disks); \
             {offered} offered Prepare now, {low} found a volume short, {tools} waiting for the Build Tools, {prepared} already prepared; all consistent"
        );
        assert_eq!(cases, 96);
        assert_eq!(offered + low + tools + prepared, cases);
    }

    #[test]
    fn a_silent_install_prepares_only_with_the_opt_in_and_a_wizard_only_on_a_yes() {
        let plan = decide(
            &facts(RustState::NoRustup, Linker::Missing, false, false),
            &FIGURES,
        );
        assert_eq!(plan.offer, Offer::PrepareNow);
        let cases = [
            (Mode::Silent { opt_in: false }, None, false),
            (Mode::Silent { opt_in: false }, Some(true), false),
            (Mode::Silent { opt_in: true }, None, true),
            (Mode::Interactive, None, false),
            (Mode::Interactive, Some(false), false),
            (Mode::Interactive, Some(true), true),
        ];
        for (mode, choice, runs) in cases {
            assert_eq!(
                plan.runs(mode, choice),
                runs,
                "{mode:?} with choice {choice:?}"
            );
        }

        // The opt-in does not override a full disk, and nothing runs when there is nothing to do.
        let mut f = facts(RustState::NoRustup, Linker::Missing, false, false);
        f.free = free(1, 1, true);
        let low = decide(&f, &FIGURES);
        assert!(matches!(low.offer, Offer::DiskLow(_)));
        assert!(
            !low.runs(Mode::Silent { opt_in: true }, None),
            "the opt-in does not override a full disk"
        );
        let done = decide(
            &facts(RustState::Pinned, Linker::Present, true, true),
            &FIGURES,
        );
        assert!(
            !done.runs(Mode::Silent { opt_in: true }, None),
            "nothing to do: nothing runs"
        );
        println!("preparation choice: 6 mode/choice cases, a full disk and a finished preparation: all hold");
    }

    #[test]
    fn rust_is_installed_beside_the_developers_own_and_fresh_only_without_rustup() {
        let beside = decide(
            &facts(
                RustState::RustupWithoutPinned,
                Linker::Present,
                false,
                false,
            ),
            &FIGURES,
        );
        assert!(
            beside
                .text
                .contains("beside the Rust you already have, which stays your default"),
            "{}",
            beside.text
        );
        let fresh = decide(
            &facts(RustState::NoRustup, Linker::Present, false, false),
            &FIGURES,
        );
        assert!(fresh.text.contains("with rustup"), "{}", fresh.text);
        assert!(!fresh.text.contains("beside"));
        assert!(
            beside.text.contains("1.95.0") && fresh.text.contains("1.95.0"),
            "the pinned version is named"
        );
    }

    #[test]
    fn a_newer_version_over_an_older_says_it_prepares_again_and_the_same_version_offers_no_compile()
    {
        let mut f = facts(RustState::Pinned, Linker::Present, false, false);
        f.previous_version = Some("2.1.20".into());
        let upgrade = decide(&f, &FIGURES);
        assert_eq!(upgrade.offer, Offer::PrepareNow);
        assert!(
            upgrade
                .text
                .starts_with("This setup replaces version 2.1.20 with 2.1.24."),
            "{}",
            upgrade.text
        );

        let mut f = facts(RustState::Pinned, Linker::Present, true, true);
        f.previous_version = Some("2.1.24".into());
        let again = decide(&f, &FIGURES);
        assert_eq!(again.offer, Offer::AlreadyPrepared);
        assert!(
            again.text.contains("already prepared")
                && !again.text.contains("What Prepare now does")
        );
        assert!(
            !again.text.contains("replaces"),
            "the same version replaces nothing: {}",
            again.text
        );

        // One profile done: only the other is compiled, and the page says which.
        let half = decide(
            &facts(RustState::Pinned, Linker::Present, true, false),
            &FIGURES,
        );
        assert!(half.steps.debug && !half.steps.release);
        assert!(
            half.text.contains("for Debug") && half.text.contains("Release is already compiled"),
            "{}",
            half.text
        );
    }

    #[test]
    fn the_page_names_every_source_size_and_term_before_anything_is_downloaded() {
        // With a linker: the compile happens, so the page promises the wait, and the Rust download is named.
        let compiling = decide(
            &facts(RustState::NoRustup, Linker::Present, false, false),
            &FIGURES,
        );
        for needle in [
            RUSTUP_URL,
            "Take a seat and get a coffee",
            "about an hour",
            "Cancelling cancels the installation",
            "stays installed",
            &human(FIGURES.rust_download_bytes),
        ] {
            assert!(
                compiling.text.contains(needle),
                "the page does not say '{needle}':\n{}",
                compiling.text
            );
        }
        // Without one: Rust is installed, the compile is not promised, and the Build Tools are named with
        // their page and Microsoft's terms, because Microsoft's installer is where those terms are accepted.
        let waiting = decide(
            &facts(RustState::NoRustup, Linker::Missing, false, false),
            &FIGURES,
        );
        for needle in [
            RUSTUP_URL,
            BUILD_TOOLS_URL,
            MICROSOFT_TERMS_URL,
            "licence terms",
            "this setup does not install them",
            "the first Debug or EXEC RUST Build asks first",
        ] {
            assert!(
                waiting.text.contains(needle),
                "the page does not say '{needle}':\n{}",
                waiting.text
            );
        }
        assert!(
            !waiting.text.contains("get a coffee") && !waiting.text.contains("about an hour"),
            "no compile is promised without a linker:\n{}",
            waiting.text
        );
        for text in [&compiling.text, &waiting.text] {
            assert!(
                text.is_ascii(),
                "the setup's text is ASCII, like the rest of the installer"
            );
        }
    }

    #[test]
    fn an_estimate_is_never_presented_as_a_measurement() {
        let estimated = decide(
            &facts(RustState::NoRustup, Linker::Missing, false, false),
            &FIGURES,
        );
        assert!(estimated.text.contains("These figures are estimates"));
        let measured = Figures {
            measured: true,
            ..FIGURES
        };
        let plan = decide(
            &facts(RustState::NoRustup, Linker::Missing, false, false),
            &measured,
        );
        assert!(
            !plan.text.contains("estimates"),
            "a measured figure is not called an estimate"
        );
    }

    #[test]
    fn the_disk_the_page_mentions_is_the_disk_the_rule_checks() {
        let f = facts(RustState::NoRustup, Linker::Present, false, false);
        let needs = needs_for(&steps_for(&f), &FIGURES);
        // Rust and the whole cache on the build volume; nothing on the system volume (no Build Tools here).
        assert_eq!(
            needs.build_bytes,
            FIGURES.rust_download_bytes + FIGURES.rust_installed_bytes + FIGURES.cache_bytes
        );
        assert_eq!(needs.system_bytes, 0);
        // Without a linker nothing is compiled, so no cache is asked for.
        let f = facts(RustState::NoRustup, Linker::Missing, false, false);
        let needs = needs_for(&steps_for(&f), &FIGURES);
        assert_eq!(
            needs.build_bytes,
            FIGURES.rust_download_bytes + FIGURES.rust_installed_bytes
        );
        // One profile is half the cache.
        let half = needs_for(
            &Steps {
                rust: RustAction::Nothing,
                build_tools_missing: false,
                release: true,
                debug: false,
            },
            &FIGURES,
        );
        assert_eq!(half.build_bytes, FIGURES.cache_bytes / 2);
    }

    #[test]
    fn rust_is_there_and_the_linker_is_not_so_the_page_only_says_where_the_tools_are() {
        let plan = decide(
            &facts(RustState::Pinned, Linker::Missing, false, false),
            &FIGURES,
        );
        assert_eq!(plan.offer, Offer::BuildToolsNeeded);
        assert!(!plan.steps.does_something() && !plan.steps.is_empty());
        for needle in [
            "nothing this setup can prepare now",
            BUILD_TOOLS_URL,
            MICROSOFT_TERMS_URL,
            "1.95.0",
        ] {
            assert!(plan.text.contains(needle), "'{needle}' in:\n{}", plan.text);
        }
        // Nothing runs, whatever the mode: there is nothing to run.
        for mode in [
            Mode::Interactive,
            Mode::Silent { opt_in: true },
            Mode::Silent { opt_in: false },
        ] {
            assert!(!plan.runs(mode, Some(true)), "{mode:?}");
        }
    }
}
