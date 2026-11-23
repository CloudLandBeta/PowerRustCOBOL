// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a Build tells the developer about the way it made the program**
//! (DESKTOP-WASM-001 T18; R18, R24, R25).
//!
//! A Build takes the WebAssembly path when it can and the Cargo path otherwise.
//! When it succeeds, the Output says which path it took (and, for Cargo, why)
//! and how long each phase took, in the IDE's language. When it stops because
//! the Cargo path needs a Rust toolchain this machine does not have, the failure
//! is typed, not matched by its text, and the IDE offers to install Rust: the
//! first-run offer, saying why this Build needs it.

use std::time::Duration;

use cobolt_compiler::warmup::{PlatformPrep, Why};
use cobolt_compiler::{BuildPath, BuildPhase, BuildResult, CargoReason, CompilerError};

use crate::i18n::Tr;
use crate::toolchain::{FirstRunPrompt, Status};

/// A Build that failed, as it travels from the build thread to the window.
#[derive(Clone, Debug)]
pub struct BuildFailure {
    /// The compiler's message, as the Output and the Building dialog show it.
    pub message: String,
    /// The Build stopped because it needs a Rust toolchain this machine does
    /// not have (R25).
    pub needs_rust: bool,
    /// Why the Build took the Cargo path, when the compiler said (it does for
    /// every project Build).
    pub reason: Option<CargoReason>,
    /// The Build stopped, before compiling anything, to ask whether the platform may be compiled on this
    /// machine first (2.1.19): what that takes, and why the Build needs Cargo. It is a question, not an error.
    pub platform: Option<(PlatformPrep, CargoReason)>,
}

impl BuildFailure {
    /// The failure of `e`, typed: a missing toolchain is known by its variant.
    pub fn from_error(e: &CompilerError) -> Self {
        let (needs_rust, reason) = match e {
            CompilerError::CargoNeedsToolchain { reason, .. } => (true, Some(reason.clone())),
            CompilerError::Toolchain { .. } => (true, None),
            _ => (false, None),
        };
        let platform = match e {
            CompilerError::PlatformPrep { prep, reason } => Some((prep.clone(), reason.clone())),
            _ => None,
        };
        Self { message: e.to_string(), needs_rust, reason, platform }
    }
}

/// The question a Build asks before it compiles the platform (2.1.19), in the IDE's language: why it has
/// to, what else the developer can do, how long it takes, and the words of the button that says yes. Short
/// on purpose: a reason, an alternative and a time, one line each.
pub struct PlatformQuestion {
    pub why: String,
    pub instead: String,
    pub time: String,
    pub yes: String,
}

/// The [`PlatformQuestion`] for `prep`, with `reason` (why the Build takes the Cargo path, already in
/// the IDE's language). `None` for [`PlatformPrep::Ready`]: there is nothing to ask.
pub fn platform_question(prep: &PlatformPrep, reason: &str) -> Option<PlatformQuestion> {
    use crate::ui_text::{ide, idef};
    let (why, instead, minutes) = match prep {
        PlatformPrep::Ready => return None,
        PlatformPrep::OwnCrates => (
            idef("Why: this program needs Cargo ({}), and PowerRustCOBOL's own crates are compiled once on this machine.", &[&reason]),
            ide("Instead: a program that needs no Cargo is built in seconds, without compiling the platform.").to_owned(),
            true,
        ),
        PlatformPrep::Everything(Why::DebugProfile) => (
            ide("Why: Debug builds compile the whole platform without optimisation, and none of it is shipped.").to_owned(),
            ide("Instead: use Build or Run, not Debug.").to_owned(),
            false,
        ),
        PlatformPrep::Everything(Why::OtherCompiler { release, here }) => {
            let here = if here.is_empty() { ide("no rustc").to_owned() } else { here.clone() };
            (
                idef("Why: this machine has {}, and the libraries shipped with PowerRustCOBOL need Rust {}.", &[&here, release]),
                idef("Instead: install Rust {} with `rustup toolchain install {}` (a few minutes), then Build again.", &[release, release]),
                false,
            )
        }
        PlatformPrep::Everything(Why::NoLibraries) => (
            ide("Why: this installation has no precompiled libraries for this machine.").to_owned(),
            ide("Instead: install PowerRustCOBOL again, which brings them.").to_owned(),
            false,
        ),
    };
    Some(PlatformQuestion {
        why,
        instead,
        time: if minutes {
            ide("Time: about 10 to 30 minutes on a modest machine, once per version. The machine stays busy meanwhile.").to_owned()
        } else {
            ide("Time: an hour or more on a modest machine, once per version. The machine stays busy meanwhile.").to_owned()
        },
        yes: if minutes { ide("Yes, wait 10 to 30 minutes") } else { ide("Yes, wait an hour or more") }.to_owned(),
    })
}

/// A phase's name in the language of `tr`.
pub fn phase_name(tr: &Tr, phase: BuildPhase) -> &'static str {
    match phase {
        BuildPhase::Regenerate => tr.build_phase_regenerate,
        BuildPhase::FrontEnd => tr.build_phase_front_end,
        BuildPhase::Classify => tr.build_phase_classify,
        BuildPhase::Generate => tr.build_phase_generate,
        BuildPhase::MachineCode => tr.build_phase_machine_code,
        BuildPhase::Assemble => tr.build_phase_assemble,
        BuildPhase::Cargo => tr.build_phase_cargo,
        BuildPhase::Install => tr.build_phase_install,
    }
}

/// Why a Build took the Cargo path, in the language of `tr`. File names,
/// constructs and the generator's own reason are not translated.
pub fn reason_text(tr: &Tr, reason: &CargoReason) -> String {
    let at = |file: &str, line: u32| {
        if line > 0 {
            tr.build_reason_at_line.replace("{file}", file).replace("{line}", &line.to_string())
        } else {
            file.to_owned()
        }
    };
    match reason {
        CargoReason::DebugBuild => tr.build_reason_debug.to_owned(),
        CargoReason::Chosen => tr.build_reason_chosen.to_owned(),
        CargoReason::ExternalCrate => tr.build_reason_external_crate.to_owned(),
        CargoReason::ExecRust { file, line } => tr.build_reason_exec_rust.replace("{at}", &at(file, *line)),
        CargoReason::Refused { construct, reason, file, line } => tr
            .build_reason_refused
            .replace("{at}", &at(file, *line))
            .replace("{construct}", construct)
            .replace("{reason}", reason),
        CargoReason::DesktopControl { form, file, control, control_type, reason } => tr
            .build_reason_desktop_control
            .replace("{form}", form)
            .replace("{file}", file)
            .replace("{control}", control)
            .replace("{type}", control_type)
            .replace("{reason}", reason),
        CargoReason::RunnerUnavailable => tr.build_reason_runner_unavailable.replace("{name}", cobolt_compiler::RUNNER_NAME),
    }
}

/// The line that says which path a Build took, and why when it is Cargo.
pub fn path_line(tr: &Tr, result: &BuildResult) -> String {
    match (&result.cargo, result.path) {
        (Some(cargo), _) => tr.build_path_line_cargo.replace("{reason}", &reason_text(tr, &cargo.reason)),
        (None, BuildPath::Cargo) => tr.build_path_line_cargo.replace("{reason}", "?"),
        (None, BuildPath::Wasm) => tr.build_path_line_wasm.to_owned(),
    }
}

/// The line with the time of each phase that ran, in the order phases run.
pub fn phases_line(tr: &Tr, phases: &[(BuildPhase, Duration)]) -> String {
    let parts: Vec<String> = phases
        .iter()
        .map(|(phase, took)| format!("{} {:.1} ms", phase_name(tr, *phase), took.as_secs_f64() * 1000.0))
        .collect();
    tr.build_phases_line.replace("{phases}", &parts.join(", "))
}

/// What the Output shows after a successful Build: the path line, then the
/// phase times when the Build reported any.
pub fn report_lines(tr: &Tr, result: &BuildResult) -> Vec<String> {
    let mut lines = vec![path_line(tr, result)];
    if !result.phases.is_empty() {
        lines.push(phases_line(tr, &result.phases));
    }
    lines
}

/// The offer a failed Build makes, given what this machine's probe found: the
/// first-run Rust offer when the Build needs a toolchain it does not have, and
/// nothing for any other failure (a mistake in the COBOL is never answered with
/// an offer to install Rust).
pub fn rust_offer(tr: &Tr, failure: &BuildFailure, status: Status) -> Option<FirstRunPrompt> {
    failure.needs_rust.then(|| {
        let reason = failure.reason.as_ref().map(|r| reason_text(tr, r)).unwrap_or_default();
        FirstRunPrompt::at_build(status, reason)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::toolchain::Decision;
    use std::path::PathBuf;

    fn result(path: BuildPath, reason: Option<CargoReason>) -> BuildResult {
        BuildResult {
            binary_path: PathBuf::from("bin/t18"),
            source_count: 1,
            form_count: 0,
            ast_bytes: 10,
            build_dir: PathBuf::from("build"),
            path,
            cargo: reason.map(|reason| cobolt_compiler::CargoBuild {
                reason,
                crates_compiled: 0,
                target_dir: PathBuf::from("target"),
                forced_full: Vec::new(),
            }),
            phases: vec![
                (BuildPhase::Regenerate, Duration::from_micros(100)),
                (BuildPhase::FrontEnd, Duration::from_micros(3_900)),
                (BuildPhase::Classify, Duration::from_micros(0)),
                (BuildPhase::Generate, Duration::from_micros(8_400)),
                (BuildPhase::MachineCode, Duration::from_micros(140_900)),
                (BuildPhase::Assemble, Duration::from_micros(1_827_600)),
                (BuildPhase::Install, Duration::from_micros(2_400)),
            ],
            peak_rss: None,
        }
    }

    fn every_reason() -> Vec<CargoReason> {
        vec![
            CargoReason::DebugBuild,
            CargoReason::Chosen,
            CargoReason::ExternalCrate,
            CargoReason::ExecRust { file: "src/main.cbl".into(), line: 5 },
            CargoReason::Refused {
                construct: "file LOG-FILE".into(),
                reason: "files on disk come with M3".into(),
                file: "src/files.cbl".into(),
                line: 13,
            },
            CargoReason::DesktopControl {
                form: "MAIN".into(),
                file: "forms/main.cfrm".into(),
                control: "MAP-CITY".into(),
                control_type: "Maps".into(),
                reason: "Maps come with M6".into(),
            },
            CargoReason::RunnerUnavailable,
        ]
    }

    const PHASES: [BuildPhase; 8] = [
        BuildPhase::Regenerate,
        BuildPhase::FrontEnd,
        BuildPhase::Classify,
        BuildPhase::Generate,
        BuildPhase::MachineCode,
        BuildPhase::Assemble,
        BuildPhase::Cargo,
        BuildPhase::Install,
    ];

    /// The report's lines come out in all six languages: every phase name, every
    /// reason and both path lines are filled, keep their file, line and
    /// construct, and differ from English outside English.
    #[test]
    fn build_path_report_lines_come_out_in_six_languages() {
        let started = std::time::Instant::now();
        let en = Language::English.tr();
        let mut checks = 0usize;
        let mut lines = 0usize;
        for lang in Language::ALL {
            let tr = lang.tr();
            for phase in PHASES {
                let name = phase_name(&tr, phase);
                assert!(!name.trim().is_empty(), "{lang:?}: phase {phase:?} has no name");
                if *lang != Language::English && phase != BuildPhase::Cargo {
                    assert_ne!(name, phase_name(&en, phase), "{lang:?}: phase {phase:?} is not translated");
                }
                checks += 1;
            }
            for reason in every_reason() {
                let text = reason_text(&tr, &reason);
                assert!(!text.contains('{'), "{lang:?}: {reason:?} left a placeholder: {text}");
                match &reason {
                    CargoReason::ExecRust { .. } => assert!(text.contains("src/main.cbl") && text.contains('5'), "{lang:?}: {text}"),
                    CargoReason::Refused { .. } => assert!(
                        text.contains("src/files.cbl") && text.contains("13") && text.contains("file LOG-FILE"),
                        "{lang:?}: {text}"
                    ),
                    CargoReason::RunnerUnavailable => assert!(text.contains(cobolt_compiler::RUNNER_NAME), "{lang:?}: {text}"),
                    CargoReason::DesktopControl { .. } => assert!(
                        ["MAIN", "forms/main.cfrm", "MAP-CITY", "Maps", "Maps come with M6"].iter().all(|w| text.contains(w)),
                        "{lang:?}: {text}"
                    ),
                    _ => {}
                }
                if *lang != Language::English {
                    assert_ne!(text, reason_text(&en, &reason), "{lang:?}: {reason:?} is not translated");
                }
                checks += 1;
            }
            let wasm = report_lines(&tr, &result(BuildPath::Wasm, None));
            assert_eq!(wasm.len(), 2, "{lang:?}: the path line and the phases line");
            assert!(wasm[0].contains("WebAssembly"), "{lang:?}: {}", wasm[0]);
            assert!(wasm[1].contains("1827.6 ms") && wasm[1].contains(phase_name(&tr, BuildPhase::Assemble)), "{lang:?}: {}", wasm[1]);
            let cargo = report_lines(&tr, &result(BuildPath::Cargo, Some(CargoReason::Chosen)));
            assert!(cargo[0].contains("Cargo") && cargo[0].contains(&reason_text(&tr, &CargoReason::Chosen)), "{lang:?}: {}", cargo[0]);
            for line in wasm.iter().chain(cargo.iter()) {
                assert!(!line.contains('{'), "{lang:?}: a placeholder is left: {line}");
            }
            if *lang != Language::English {
                assert_ne!(wasm[0], report_lines(&en, &result(BuildPath::Wasm, None))[0], "{lang:?}: the path line is not translated");
            }
            lines += wasm.len() + cargo.len();
            checks += 4;
        }
        println!("== build_path_report (DESKTOP-WASM-001 T18: R18, R24, R25) ==");
        println!(
            "languages {}, phase names {}, reasons {}, report lines {lines}; {checks} of {checks} checks passed in {:.1} ms",
            Language::ALL.len(),
            PHASES.len(),
            every_reason().len(),
            started.elapsed().as_secs_f64() * 1000.0
        );
    }

    /// A typed toolchain failure opens the offer, saying why the Build needs Cargo;
    /// any other failure opens nothing.
    #[test]
    fn build_path_report_a_typed_failure_opens_the_offer() {
        let tr = Language::English.tr();
        let needs = CompilerError::CargoNeedsToolchain {
            reason: CargoReason::ExecRust { file: "src/main.cbl".into(), line: 5 },
            tool: "rustc".into(),
            detail: "No such file or directory".into(),
        };
        let failure = BuildFailure::from_error(&needs);
        assert!(failure.needs_rust && failure.message.contains("EXEC RUST"));
        let mut offer = rust_offer(&tr, &failure, Status::Missing).expect("a typed failure opens the offer");
        let reason = offer.at_build.clone().expect("the offer is a Build's");
        assert!(reason.contains("src/main.cbl") && reason.contains("EXEC RUST"), "{reason}");
        assert_eq!(offer.status, Status::Missing);
        // One no is the answer at Build time; the next Build asks again.
        assert_eq!(offer.decline(), Decision::Accepted);

        // A probe that finds a usable rustc the Build could not run still offers the install.
        let found = Status::Ok { path: PathBuf::from("/elsewhere/rustc"), version: crate::toolchain::minimum() };
        assert_eq!(rust_offer(&tr, &failure, found).unwrap().status, Status::Missing);

        // The untyped toolchain error offers too, without a reason.
        let plain = BuildFailure::from_error(&CompilerError::Toolchain { tool: "rustc".into(), detail: "gone".into() });
        assert_eq!(rust_offer(&tr, &plain, Status::Missing).unwrap().at_build.as_deref(), Some(""));

        // Anything else is answered with the failure only.
        let other = BuildFailure::from_error(&CompilerError::Toml("expected `=`".into()));
        assert!(!other.needs_rust && rust_offer(&tr, &other, Status::Missing).is_none());

        // The first-run offer still asks twice.
        let mut first = FirstRunPrompt::for_status(Status::Missing).unwrap();
        assert!(first.at_build.is_none());
        assert_eq!(first.decline(), Decision::AskOnceMore);
        assert_eq!(first.decline(), Decision::Accepted);
    }

    /// A project without the new settings reads their defaults, and saving it
    /// writes no `[desktop]` table it did not have.
    #[test]
    fn build_path_report_a_project_without_the_settings_reads_the_defaults() {
        let dir = std::env::temp_dir().join(format!("prc-t18-defaults-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("T18.project.toml");
        let text = "[project]\nname = \"T18\"\nversion = \"0.1.0\"\nmain = \"src/main.cbl\"\n";
        std::fs::write(&manifest, text).unwrap();
        let p = crate::project_model::load_project(&manifest).expect("the project loads");
        assert!(p.desktop.is_none(), "no [desktop] table is invented");
        let settings = p.desktop.clone().unwrap_or_default();
        assert_eq!(settings.build_path, cobolt_compiler::project_kind::BUILD_PATH_WASM);
        assert!(settings.machine_code && !settings.chooses_cargo());

        let draft = crate::panels::settings_form::SettingsDraft::from_project(&p, &crate::llm::LlmConfig::load_defaults_for_test());
        let mut saved = p.clone();
        draft.apply_kind(&mut saved);
        assert!(saved.desktop.is_none(), "an unchanged Desktop section writes nothing");
        crate::project_model::save_project(&saved, &manifest).unwrap();
        let after = std::fs::read_to_string(&manifest).unwrap();
        assert!(!after.contains("[desktop]") && !after.contains("build_path") && !after.contains("machine_code"), "{after}");

        // A project that chooses Cargo keeps the choice; the machine code, left at its default, is not written.
        let mut draft = draft;
        draft.desktop.build_path = cobolt_compiler::project_kind::BUILD_PATH_CARGO.to_owned();
        let mut chosen = p.clone();
        draft.apply_kind(&mut chosen);
        assert!(chosen.desktop.as_ref().is_some_and(|d| d.chooses_cargo() && d.machine_code));
        crate::project_model::save_project(&chosen, &manifest).unwrap();
        let after = std::fs::read_to_string(&manifest).unwrap();
        assert!(after.contains("build_path = \"cargo\"") && !after.contains("machine_code"), "{after}");
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// **2.1.19 — the question before the platform is compiled** is a typed failure, not an error, and its
    /// four lines (why, instead, time, the button) come out in all six languages, whole: the reason, the
    /// compiler found here and the release needed are in the text, no hole is left, and the button says
    /// how long the wait is.
    #[test]
    fn build_path_report_the_platform_question_comes_out_in_six_languages() {
        let started = std::time::Instant::now();
        let reason = "THE-REASON";
        let preps = [
            PlatformPrep::OwnCrates,
            PlatformPrep::Everything(Why::DebugProfile),
            PlatformPrep::Everything(Why::OtherCompiler { release: "1.95.0".into(), here: "rustc 1.93.1 (01f6ddf75 2026-02-11)".into() }),
            PlatformPrep::Everything(Why::OtherCompiler { release: "1.95.0".into(), here: String::new() }),
            PlatformPrep::Everything(Why::NoLibraries),
        ];
        assert!(platform_question(&PlatformPrep::Ready, reason).is_none(), "a ready platform asks nothing");
        let english = |prep: &PlatformPrep| {
            crate::ui_text::begin(Language::English);
            platform_question(prep, reason).expect("a question")
        };
        let mut checks = 1usize;
        for lang in Language::ALL {
            for prep in &preps {
                let base = english(prep);
                crate::ui_text::begin(*lang);
                let q = platform_question(prep, reason).expect("a question");
                for (what, text) in [("why", &q.why), ("instead", &q.instead), ("time", &q.time), ("yes", &q.yes)] {
                    assert!(!text.trim().is_empty() && !text.contains("{}"), "{lang:?} {prep:?} {what}: {text}");
                    checks += 1;
                }
                if *lang != Language::English {
                    for (what, a, b) in [("why", &q.why, &base.why), ("instead", &q.instead, &base.instead), ("time", &q.time, &base.time), ("yes", &q.yes, &base.yes)] {
                        assert_ne!(a, b, "{lang:?} {prep:?}: {what} is not translated");
                        checks += 1;
                    }
                }
                match prep {
                    PlatformPrep::OwnCrates => assert!(q.why.contains(reason), "{lang:?}: the Cargo reason is named: {}", q.why),
                    PlatformPrep::Everything(Why::OtherCompiler { here, .. }) => {
                        assert!(q.why.contains("1.95.0"), "{lang:?}: the release needed is named: {}", q.why);
                        assert!(q.why.contains(if here.is_empty() { "rustc" } else { "1.93.1" }), "{lang:?}: the compiler found is named: {}", q.why);
                        assert!(q.instead.contains("rustup toolchain install 1.95.0"), "{lang:?}: the command is given: {}", q.instead);
                    }
                    _ => {}
                }
                checks += 1;
            }
            // The button says how long the wait is, in the words of the language: tens of minutes for the platform's own crates.
            crate::ui_text::begin(*lang);
            let short = platform_question(&PlatformPrep::OwnCrates, reason).unwrap();
            let long = platform_question(&PlatformPrep::Everything(Why::NoLibraries), reason).unwrap();
            assert_ne!(short.yes, long.yes, "{lang:?}: the two waits are named differently");
            assert_ne!(short.time, long.time, "{lang:?}: the two waits are timed differently");
            checks += 2;
        }
        crate::ui_text::begin(Language::English);
        assert_eq!(english(&preps[0]).yes, "Yes, wait 10 to 30 minutes");
        assert_eq!(english(&preps[4]).yes, "Yes, wait an hour or more");
        checks += 2;

        // The failure is typed, and it is not an offer to install Rust.
        let asked = CompilerError::PlatformPrep { prep: PlatformPrep::OwnCrates, reason: CargoReason::ExternalCrate };
        let failure = BuildFailure::from_error(&asked);
        assert!(!failure.needs_rust && failure.platform == Some((PlatformPrep::OwnCrates, CargoReason::ExternalCrate)));
        assert!(BuildFailure::from_error(&CompilerError::Toml("x".into())).platform.is_none());
        checks += 2;
        println!("== build_path_report: the platform question (2.1.19) ==");
        println!(
            "{} languages × {} cases (own crates, Debug, other compiler, no compiler, no libraries), {checks} of {checks} checks passed in {:.1} ms",
            Language::ALL.len(),
            preps.len(),
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}
