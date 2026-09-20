// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 058 R19/R20 — handing a document to the operating system.
//!
//! The division of labour is the one `file_picker_requests` and the Save As
//! panel already draw: the runtime owns the document and this crate owns the
//! platform. Neither can call the other, so a request crosses the state
//! channel, the handoff happens here, and the answer crosses back.
//!
//! **What "complete" means is stated, not implied.** R32 says the
//! Complete/Cancelled events report what the platform actually did, and what
//! this can observe is whether the platform ACCEPTED the document:
//!
//! * **Print** goes to the print system — `lp` on macOS and Linux, the shell's
//!   own `print` verb on Windows. A document accepted by the spooler is a
//!   document printing, so `onPrintComplete` is the truth there. A spooler that
//!   refuses it, or a machine with no printer configured, is `onPrintCancelled`
//!   with the reason in `LastError`.
//! * **Share** hands the document to the platform's default opener — `open`,
//!   `xdg-open`, `start`. That is the mechanism `/clarify` named for Linux and
//!   it is what is reachable on every platform without native bindings.
//!   **It is not the system share sheet**, and the Developer's Guide says so:
//!   macOS's `NSSharingServicePicker` and the Windows share contract need
//!   native code this does not yet have.
//!
//! Every handoff runs on a thread of its own. A print spooler or a desktop
//! opener can take seconds, and a form that stops painting while one does is
//! the defect the whole non-blocking dialog rule exists to prevent.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Which handoff — kept apart from the runtime's own enum, which this crate
/// cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    Print,
    Share,
}

impl Handoff {
    /// The property a request arrives on.
    pub fn request_prop(self) -> &'static str {
        match self {
            Self::Print => "_PrintRequest",
            Self::Share => "_ShareRequest",
        }
    }

    /// The property the answer goes back on.
    pub fn answer_prop(self) -> &'static str {
        match self {
            Self::Print => "_PrintAnswer",
            Self::Share => "_ShareAnswer",
        }
    }
}

/// What the platform did, ready to cross back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub accepted: bool,
    /// Why not, when it was not. Empty on success.
    pub reason: String,
}

impl Outcome {
    fn ok() -> Self {
        Self { accepted: true, reason: String::new() }
    }

    fn refused(reason: impl Into<String>) -> Self {
        Self { accepted: false, reason: reason.into() }
    }
}

/// The command this platform uses for `what`, and the arguments before the
/// file — split out from running it so the choice itself is testable on every
/// platform rather than only on the one the test happens to run on.
pub fn command_for(what: Handoff, os: &str) -> Option<(&'static str, &'static [&'static str])> {
    match (what, os) {
        // CUPS, on both. `lp` queues to the default printer and reports a job
        // id; there is no dialog, which is why a refusal is the only kind of
        // "cancelled" print this can honestly report.
        (Handoff::Print, "macos" | "linux") => Some(("lp", &[])),
        // `-Verb Print` is the shell's own print verb, which is what a
        // right-click on a file offers.
        (Handoff::Print, "windows") => {
            Some(("powershell", &["-NoProfile", "-Command", "Start-Process", "-Verb", "Print", "-FilePath"]))
        }
        (Handoff::Share, "macos") => Some(("open", &[])),
        (Handoff::Share, "linux") => Some(("xdg-open", &[])),
        (Handoff::Share, "windows") => Some(("cmd", &["/C", "start", ""])),
        _ => None,
    }
}

/// Hand `path` to the platform, and say what happened.
///
/// Blocking — run it on a thread. `hand_off_async` does exactly that.
pub fn hand_off(what: Handoff, path: &Path) -> Outcome {
    if !path.is_file() {
        return Outcome::refused(format!("no document at {}", path.display()));
    }
    let Some((program, args)) = command_for(what, std::env::consts::OS) else {
        return Outcome::refused(format!(
            "this platform offers no {} the Viewer can reach",
            match what {
                Handoff::Print => "print command",
                Handoff::Share => "way to hand a document on",
            }
        ));
    };
    let mut cmd = Command::new(program);
    cmd.args(args).arg(path);
    match cmd.status() {
        Ok(status) if status.success() => Outcome::ok(),
        Ok(status) => Outcome::refused(format!("{program} refused the document ({status})")),
        // The commonest failure by far, and the one worth naming: the tool is
        // not installed. "No such file or directory" about `lp` reads as if the
        // DOCUMENT were missing, which it is not.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Outcome::refused(format!("{program} is not installed on this machine"))
        }
        Err(e) => Outcome::refused(format!("{program} could not be run: {e}")),
    }
}

/// [`hand_off`] on a thread, answering through `reply`.
///
/// A print spooler or a desktop opener can take seconds. A form that stops
/// painting while one does is exactly the defect the non-blocking dialog rule
/// exists to prevent, and this is the same rule applied to the same kind of
/// wait.
pub fn hand_off_async(
    what: Handoff,
    path: PathBuf,
    ctrl_id: String,
    reply: std::sync::mpsc::Sender<(String, Handoff, Outcome)>,
    ctx: egui::Context,
) {
    std::thread::spawn(move || {
        let outcome = hand_off(what, &path);
        let _ = reply.send((ctrl_id, what, outcome));
        // The answer arrives on a thread egui knows nothing about; without
        // this it would sit in the channel until something else caused a frame.
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every platform has an answer for both**, and they are the platform's
    /// own tools rather than something invented here.
    ///
    /// Checked for all three rather than only the one this test runs on: a
    /// missing Windows arm would otherwise be found by a Windows operator
    /// instead of by this.
    #[test]
    fn each_platform_has_a_print_and_a_share_command() {
        println!("  os        print                 share");
        println!("  -------   -------------------   ------------");
        for os in ["macos", "linux", "windows"] {
            let p = command_for(Handoff::Print, os);
            let s = command_for(Handoff::Share, os);
            println!(
                "  {os:<7}   {:<19}   {}",
                p.map(|(c, _)| c).unwrap_or("NONE"),
                s.map(|(c, _)| c).unwrap_or("NONE")
            );
            assert!(p.is_some(), "{os} must have a print command");
            assert!(s.is_some(), "{os} must have a way to hand a document on");
        }
        // An unknown platform says so rather than guessing at a command that
        // does not exist there.
        assert!(command_for(Handoff::Print, "haiku").is_none());
    }

    /// A document that is not there is refused **before** the platform is
    /// troubled with it, and the refusal names the path rather than leaving the
    /// operator to guess which document was missing.
    #[test]
    fn a_missing_document_is_refused_by_name() {
        let path = std::env::temp_dir().join("prc-no-such-document-9e3f.pdf");
        let _ = std::fs::remove_file(&path);
        let outcome = hand_off(Handoff::Print, &path);
        println!("  {outcome:?}");
        assert!(!outcome.accepted);
        assert!(
            outcome.reason.contains("no document at"),
            "the refusal must say what was missing: {outcome:?}"
        );
        assert!(
            outcome.reason.contains("prc-no-such-document-9e3f"),
            "and which one: {outcome:?}"
        );
    }

    /// The properties the two halves speak through, so a rename on one side
    /// cannot quietly stop matching the other.
    #[test]
    fn the_request_and_answer_properties_are_the_ones_the_runtime_writes() {
        assert_eq!(Handoff::Print.request_prop(), "_PrintRequest");
        assert_eq!(Handoff::Print.answer_prop(), "_PrintAnswer");
        assert_eq!(Handoff::Share.request_prop(), "_ShareRequest");
        assert_eq!(Handoff::Share.answer_prop(), "_ShareAnswer");
    }
}
