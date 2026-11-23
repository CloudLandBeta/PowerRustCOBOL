// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The Visual Studio Build Tools step of the bundle (spec 112, task T19; decision D12).
//!
//! Microsoft's Build Tools cannot be installed from inside the `.msi` (Windows Installer runs one installation at
//! a time, and Microsoft's bootstrapper answers 1618 inside it: plan section 9). The bundle (`wix/bundle.wxs`, a
//! WiX Burn bootstrapper) therefore runs this step *before* the `.msi`, as a program of its own,
//! `prc-buildtools.exe`. Everything the program decides is here, as plain functions, so it is tested on every
//! operating system; the program itself (`src/bin/prc_buildtools.rs`, Windows only) asks the machine, downloads,
//! checks the signature, runs Microsoft's installer and hands the answers to these functions.
//!
//! ```text
//! linker present?  ->  yes: nothing to do (exit 0)
//!                  ->  no:  download vs_BuildTools.exe  ->  signed by Microsoft?  ->  run it
//!                           ->  exit code + is the linker there now?  ->  decide()  ->  the bundle's exit code
//! ```
//!
//! What the bundle does with the exit code (`ExitCode` rows of `bundle.wxs`, held to this module by a test in
//! `installer.rs`): `0` the chain goes on to the `.msi`; `3010` the chain goes on and Windows is asked to restart at
//! the end; `1602` and `1618` end the bundle (the developer cancelled, or another installation is running, which
//! the `.msi` could not run beside either). **Every other outcome is `0`**: a Build Tools install that did not
//! happen is not a reason to withhold the program (R12, "a failure never fails the install"), and the `.msi`'s
//! own finish page then says the linker is missing and what to do (`result::missing_linker`).

use crate::prepare::{Linker, BUILD_TOOLS_URL};

#[cfg(windows)]
pub mod system;

/// The Visual Studio workload that carries the C++ compiler and linker. The IDE's copy-paste text
/// (`cobolt_compiler::linker_install_command`) names the same one, and a test here reads that source to say so.
pub const WORKLOAD: &str = "Microsoft.VisualStudio.Workload.VCTools";

/// Where Microsoft publishes its Build Tools bootstrapper (always the current one). It is downloaded at install
/// time and never bundled: Microsoft's files are not ours to redistribute.
pub const DOWNLOAD_URL: &str = "https://aka.ms/vs/17/release/vs_BuildTools.exe";

/// The publisher the downloaded installer must be signed by.
pub const PUBLISHER: &str = "Microsoft Corporation";

/// Windows asks for a restart before the tools are used.
pub const RESTART_REQUIRED: u32 = 3010;
/// The developer cancelled (Windows Installer's `ERROR_INSTALL_USEREXIT`).
pub const CANCELLED: u32 = 1602;
/// Another installation is running (`ERROR_INSTALL_ALREADY_RUNNING`).
pub const BUSY: u32 = 1618;

/// How long Microsoft's installer may run before it is given up on. The install is a few gigabytes; an hour and a
/// half is far longer than a slow line needs and short enough that a hung installer does not hold the setup forever.
pub const LIMIT_SECONDS: u64 = 90 * 60;

/// The command line of Microsoft's bootstrapper: quiet, waited for (it returns when the install is done),
/// never restarting by itself, the C++ workload with its recommended components (the Windows SDK a link needs).
pub fn arguments() -> Vec<String> {
    [
        "--quiet",
        "--wait",
        "--norestart",
        "--add",
        WORKLOAD,
        "--includeRecommended",
    ]
    .iter()
    .map(|a| (*a).to_owned())
    .collect()
}

/// Whether Microsoft's installer has to run at all: only when no working linker is installed.
pub fn needed(linker: Linker) -> bool {
    linker == Linker::Missing
}

/// What the machine said about the downloaded file's signature (`WinVerifyTrust` and the signer's name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signature {
    /// Windows trusts the signature, and this is who signed.
    Valid { publisher: String },
    /// The file is not signed.
    Unsigned,
    /// Signed, but Windows does not trust it (a broken or revoked chain, a changed file); the text is the reason.
    Untrusted { reason: String },
}

/// Whether the downloaded installer may be run: signed, trusted, and by Microsoft. The refusal is a sentence.
pub fn accept(signature: &Signature) -> Result<(), String> {
    match signature {
        Signature::Valid { publisher } if publisher.trim().eq_ignore_ascii_case(PUBLISHER) => Ok(()),
        Signature::Valid { publisher } => Err(format!(
            "The file downloaded for the C++ build tools is signed by '{}', not by {PUBLISHER}, so it was not run.",
            ascii(publisher)
        )),
        Signature::Unsigned => Err(format!(
            "The file downloaded for the C++ build tools is not signed by {PUBLISHER}, so it was not run."
        )),
        Signature::Untrusted { reason } => Err(format!(
            "Windows does not trust the signature of the file downloaded for the C++ build tools ({}), so it was not run.",
            ascii(reason)
        )),
    }
}

/// How Microsoft's installer ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// It ran and returned this exit code.
    Exited(u32),
    /// It could not be downloaded, was refused, or could not be started; the text is the sentence.
    NotRun(String),
    /// It was still running when [`LIMIT_SECONDS`] ran out and was stopped.
    TimedOut,
}

/// What the bundle is told, and what it is for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The exit code `prc-buildtools.exe` returns to the bundle.
    pub exit: u32,
    /// Whether the C++ build tools are installed and the linker found.
    pub ready: bool,
    /// One plain sentence, ASCII.
    pub text: String,
}

/// Keep a name or a reason safe for a log line and a page: printable ASCII only, no line breaks.
fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c
            } else {
                '?'
            }
        })
        .take(160)
        .collect()
}

/// The decision table: what Microsoft's installer returned and whether the linker is there after it.
pub fn decide(run: &Run, linker_after: Linker) -> Decision {
    let found = linker_after == Linker::Present;
    let not_done = |why: &str| {
        Decision {
        exit: 0,
        ready: false,
        text: format!(
            "The C++ build tools were not installed: {why} The program is installed anyway; the first Debug or EXEC RUST Build compiles the platform once they are ({BUILD_TOOLS_URL})."
        ),
    }
    };
    match run {
        Run::NotRun(why) => not_done(ascii(why).trim_end()),
        Run::TimedOut => not_done("Microsoft's installer did not finish in an hour and a half and was stopped."),
        Run::Exited(0) if found => Decision {
            exit: 0,
            ready: true,
            text: "The C++ build tools are installed.".to_owned(),
        },
        Run::Exited(0) => not_done("Microsoft's installer finished, but the C++ linker was not found afterwards."),
        Run::Exited(code @ (RESTART_REQUIRED | 1641)) => Decision {
            exit: RESTART_REQUIRED,
            ready: found,
            text: if found {
                format!("The C++ build tools are installed; Windows needs a restart before every program sees them (code {code}).")
            } else {
                format!("The C++ build tools are installed, but Windows needs a restart before the linker is found (code {code}).")
            },
        },
        Run::Exited(CANCELLED | 5004) => Decision {
            exit: CANCELLED,
            ready: false,
            text: "The installation of the C++ build tools was cancelled.".to_owned(),
        },
        Run::Exited(BUSY | 1001) => Decision {
            exit: BUSY,
            ready: false,
            text: "Another installation is running (Windows Installer or Visual Studio). Finish it, then run this setup again.".to_owned(),
        },
        Run::Exited(code @ (5003 | 5007 | 5100 | 8002 | 8003)) => not_done(&format!(
            "Microsoft's installer reported that it could not go on (code {code}): the computer's system, disk or network did not meet what it needs."
        )),
        Run::Exited(code) => not_done(&format!("Microsoft's installer ended with code {code}.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decided(code: u32, linker: Linker) -> Decision {
        decide(&Run::Exited(code), linker)
    }

    #[test]
    fn it_runs_only_when_the_linker_is_missing() {
        assert!(needed(Linker::Missing));
        assert!(!needed(Linker::Present));
    }

    #[test]
    fn the_command_line_is_quiet_waited_for_and_never_restarts() {
        assert_eq!(
            arguments(),
            [
                "--quiet",
                "--wait",
                "--norestart",
                "--add",
                "Microsoft.VisualStudio.Workload.VCTools",
                "--includeRecommended"
            ]
        );
    }

    /// The setup and the IDE's copy-paste text for a developer must name the same workload: a developer who runs the
    /// IDE's command gets what the setup would have installed. `linker_install_command` is `cfg(windows)`, so this
    /// reads the source it is written in, as the other guards of this crate read the WiX.
    #[test]
    fn the_workload_is_the_one_the_ide_tells_a_developer_to_install() {
        let source = include_str!("../../cobolt-compiler/src/lib.rs");
        let from = source
            .find("pub fn linker_install_command")
            .expect("linker_install_command is in cobolt-compiler");
        let body = &source[from..source[from..]
            .find("\n}\n")
            .map_or(source.len(), |n| from + n)];
        assert!(
            body.contains(WORKLOAD),
            "the IDE's text must name {WORKLOAD}:\n{body}"
        );
    }

    #[test]
    fn the_download_is_microsofts_own_https_address() {
        assert!(DOWNLOAD_URL.starts_with("https://aka.ms/vs/"));
        assert!(DOWNLOAD_URL.ends_with("/vs_BuildTools.exe"));
    }

    #[test]
    fn only_a_trusted_signature_by_microsoft_is_accepted() {
        let ok = Signature::Valid {
            publisher: "Microsoft Corporation".into(),
        };
        assert_eq!(accept(&ok), Ok(()));
        let spaced = Signature::Valid {
            publisher: " microsoft corporation ".into(),
        };
        assert_eq!(accept(&spaced), Ok(()));

        for refused in [
            Signature::Valid {
                publisher: "Microsoft Corporation Ltd".into(),
            },
            Signature::Valid {
                publisher: "M\u{ed}crosoft Corporation".into(),
            },
            Signature::Valid {
                publisher: String::new(),
            },
            Signature::Unsigned,
            Signature::Untrusted {
                reason: "the hash does not match".into(),
            },
        ] {
            let why = accept(&refused).expect_err(&format!("{refused:?} must be refused"));
            assert!(why.ends_with("so it was not run."), "{why}");
            assert!(why.is_ascii(), "{why}");
        }
    }

    #[test]
    fn success_with_the_linker_is_ready_and_without_it_is_not() {
        let ok = decided(0, Linker::Present);
        assert_eq!((ok.exit, ok.ready), (0, true));
        let odd = decided(0, Linker::Missing);
        assert_eq!((odd.exit, odd.ready), (0, false));
        assert!(
            odd.text.contains("linker was not found afterwards"),
            "{}",
            odd.text
        );
    }

    #[test]
    fn a_restart_is_passed_on_and_counts_as_ready_only_when_the_linker_works() {
        for code in [3010, 1641] {
            let with = decided(code, Linker::Present);
            assert_eq!((with.exit, with.ready), (RESTART_REQUIRED, true), "{code}");
            let without = decided(code, Linker::Missing);
            assert_eq!(
                (without.exit, without.ready),
                (RESTART_REQUIRED, false),
                "{code}"
            );
            assert!(
                without.text.contains("restart before the linker is found"),
                "{}",
                without.text
            );
        }
    }

    #[test]
    fn a_cancel_and_a_busy_machine_end_the_bundle_and_nothing_else_does() {
        for code in [1602, 5004] {
            assert_eq!(decided(code, Linker::Missing).exit, CANCELLED, "{code}");
        }
        for code in [1618, 1001] {
            assert_eq!(decided(code, Linker::Missing).exit, BUSY, "{code}");
        }
        // Everything else lets the program install: exit 0, and a sentence that says what was not done.
        for code in [
            5003, 5007, 5100, 8002, 8003, 1, 5, 87, 1603, 1722, 4294967295,
        ] {
            let d = decided(code, Linker::Missing);
            assert_eq!((d.exit, d.ready), (0, false), "{code}");
            assert!(d.text.contains(BUILD_TOOLS_URL), "{}", d.text);
            assert!(
                d.text.contains("The program is installed anyway"),
                "{}",
                d.text
            );
        }
    }

    #[test]
    fn a_download_that_failed_a_refused_file_and_a_timeout_are_failures_that_let_the_program_install(
    ) {
        for run in [
            Run::NotRun("The download of the C++ build tools failed (no connection).".into()),
            Run::NotRun(accept(&Signature::Unsigned).unwrap_err()),
            Run::TimedOut,
        ] {
            let d = decide(&run, Linker::Missing);
            assert_eq!((d.exit, d.ready), (0, false), "{run:?}");
            assert!(d.text.contains(BUILD_TOOLS_URL), "{}", d.text);
        }
    }

    #[test]
    fn every_sentence_is_one_ascii_line_that_fits_an_installer_page() {
        let mut runs = vec![
            Run::TimedOut,
            Run::NotRun("A line\r\nbroken, with non-ASCII: \u{e9}".into()),
        ];
        runs.extend(
            [
                0, 3010, 1641, 1602, 5004, 1618, 1001, 5003, 5007, 5100, 8002, 8003, 1, 999,
            ]
            .map(Run::Exited),
        );
        for run in runs {
            for linker in [Linker::Present, Linker::Missing] {
                let d = decide(&run, linker);
                assert!(
                    d.text.is_ascii() && !d.text.contains('\n') && d.text.len() <= 400,
                    "{run:?}/{linker:?}: {} ({} characters)",
                    d.text,
                    d.text.len()
                );
                // `[` and `]` would be read as a property reference by Windows Installer if a sentence reached one.
                assert!(!d.text.contains('[') && !d.text.contains(']'), "{}", d.text);
            }
        }
    }
}
