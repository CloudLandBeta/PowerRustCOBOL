// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Carrying out a toolbar button's PLATFORM action.
//!
//! [`crate::toolbar`] decides which button was pressed and what it asked for,
//! and takes no dependency on a printer, a clipboard or a process launcher to
//! work that out. This is where the deed is done — the same division of labour
//! the FileDropZone's native picker uses.
//!
//! # Who calls this
//!
//! It sits beside [`crate::toolbar_paint`], behind the same `render` feature,
//! for the same reason: **every surface that draws a toolbar must also be able
//! to press one**. The running host does (`rcrun run-form` and a compiled
//! binary), and so does the designer's Preview — a button that works when the
//! form runs but does nothing in Preview is a button you cannot design against.
//!
//! The actions the form's own COBOL carries out (`event`, `procedure:`,
//! `open-modal:`) never reach here; they leave the renderer as ordinary events.
//!
//! # Nothing fails in silence
//!
//! Every action reports an [`Outcome`]. A press that could not be honoured says
//! why — the file was not there, the application would not start, no text field
//! had focus — because a toolbar button that appears to do nothing is the worst
//! outcome available. The host logs it and can show it.
//!
//! # Two-step captures
//!
//! A screenshot cannot be taken during the frame that asks for it: egui renders,
//! then hands the pixels back on a later frame. So `screenshot` and `share` ask
//! for a capture and remember what they wanted it for; [`Runner::poll_capture`]
//! finishes the job when the image arrives.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::toolbar::ToolbarAction;

/// What became of one press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Carried out. Carries a line fit to show the operator.
    Done(String),
    /// Asked for and still in flight — a capture waiting on the next frame.
    Pending(String),
    /// Could not be carried out, and why.
    Failed(String),
    /// This platform has no such facility. Distinct from `Failed`: nothing is
    /// broken, the deed simply cannot be done here.
    Unsupported(String),
}

impl Outcome {
    pub fn message(&self) -> &str {
        match self {
            Self::Done(m) | Self::Pending(m) | Self::Failed(m) | Self::Unsupported(m) => m,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Failed(_) | Self::Unsupported(_))
    }
}

/// What a capture was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capture {
    /// Put the image on the clipboard.
    ToClipboard,
    /// Write it out and hand it to the OS share sheet.
    ToShareSheet,
}

/// The text of whichever control has focus, so the clipboard verbs have
/// something to act on.
pub struct Focused<'a> {
    /// The control's id, for the message.
    pub control_id: &'a str,
    /// Its current text. `Cut` and `Paste` write back through this.
    pub text: String,
}

/// Carries out platform actions and finishes captures.
#[derive(Default)]
pub struct Runner {
    pending: Option<Capture>,
    /// The last thing that happened, for the host to surface.
    pub last: Option<Outcome>,
}

impl Runner {
    /// Do what `action` asks. `focused` is the text-bearing control that has
    /// keyboard focus, when there is one — the clipboard verbs need it and
    /// nothing else does.
    ///
    /// Returns the new text for the focused control when the action changed it
    /// (`cut` empties it, `paste` fills it), so the caller can write it back
    /// through the same channel a keystroke would have.
    pub fn perform(
        &mut self,
        ctx: &egui::Context,
        action: &ToolbarAction,
        focused: Option<Focused<'_>>,
    ) -> (Outcome, Option<String>) {
        let (outcome, new_text) = match action {
            // Not ours — the renderer never sends these here.
            ToolbarAction::Event | ToolbarAction::Procedure(_) | ToolbarAction::OpenModal(_) => (
                Outcome::Failed(
                    "this action belongs to the form's own COBOL, not the platform".into(),
                ),
                None,
            ),

            ToolbarAction::Screenshot => (self.request_capture(ctx, Capture::ToClipboard), None),
            ToolbarAction::Share => (self.request_capture(ctx, Capture::ToShareSheet), None),

            ToolbarAction::Copy => match &focused {
                None => (Outcome::Failed(no_focus()), None),
                Some(f) => {
                    ctx.copy_text(f.text.clone());
                    (
                        Outcome::Done(format!(
                            "Copied {} character(s) from {}",
                            f.text.chars().count(),
                            f.control_id
                        )),
                        None,
                    )
                }
            },
            ToolbarAction::Cut => match &focused {
                None => (Outcome::Failed(no_focus()), None),
                Some(f) => {
                    ctx.copy_text(f.text.clone());
                    (
                        Outcome::Done(format!(
                            "Cut {} character(s) from {}",
                            f.text.chars().count(),
                            f.control_id
                        )),
                        Some(String::new()),
                    )
                }
            },
            ToolbarAction::Paste => match &focused {
                None => (Outcome::Failed(no_focus()), None),
                Some(f) => match clipboard_text() {
                    Err(e) => (Outcome::Failed(format!("Clipboard unavailable: {e}")), None),
                    Ok(text) if text.is_empty() => (
                        Outcome::Failed("The clipboard holds no text".into()),
                        None,
                    ),
                    Ok(text) => (
                        Outcome::Done(format!(
                            "Pasted {} character(s) into {}",
                            text.chars().count(),
                            f.control_id
                        )),
                        Some(text),
                    ),
                },
            },

            ToolbarAction::Print(target) => (print_file(target), None),
            ToolbarAction::RunApp(target) => (run_app(target), None),
            ToolbarAction::OpenTerminal(dir) => (open_terminal(dir), None),
        };
        if outcome.is_error() {
            eprintln!("toolbar action: {}", outcome.message());
        }
        self.last = Some(outcome.clone());
        (outcome, new_text)
    }

    fn request_capture(
        &mut self,
        ctx: &egui::Context,
        what: Capture,
    ) -> Outcome {
        // A second press while one is in flight is not an error — it is the same
        // request again, and the reply will serve it.
        self.pending = Some(what);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
            egui::UserData::default(),
        ));
        Outcome::Pending(match what {
            Capture::ToClipboard => "Capturing the window…".into(),
            Capture::ToShareSheet => "Capturing the window to share…".into(),
        })
    }

    /// Call once a frame. Picks up a screenshot reply and finishes what asked
    /// for it. Does nothing when nothing is pending.
    pub fn poll_capture(&mut self, ctx: &egui::Context) -> Option<Outcome> {
        let what = self.pending?;
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        })?;
        self.pending = None;
        let outcome = match what {
            Capture::ToClipboard => match image_to_clipboard(&image) {
                Ok(()) => Outcome::Done(format!(
                    "Window image ({}×{}) copied to the clipboard",
                    image.width(),
                    image.height()
                )),
                Err(e) => Outcome::Failed(format!("Could not copy the image: {e}")),
            },
            Capture::ToShareSheet => match write_png(&image).and_then(|p| share_file(&p)) {
                Ok(msg) => Outcome::Done(msg),
                Err(e) => Outcome::Failed(e),
            },
        };
        if outcome.is_error() {
            eprintln!("toolbar capture: {}", outcome.message());
        }
        self.last = Some(outcome.clone());
        Some(outcome)
    }
}

fn no_focus() -> String {
    "No text field has focus — click into one first".into()
}

fn clipboard_text() -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.get_text().map_err(|e| e.to_string())
}

/// egui hands a capture back as PREMULTIPLIED RGBA; the clipboard and every
/// PNG encoder want straight RGBA. Both consumers need the same bytes, so the
/// un-premultiply happens once, here, through epaint's own conversion.
fn unmultiplied_rgba(image: &egui::ColorImage) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(image.width() * image.height() * 4);
    for px in &image.pixels {
        bytes.extend_from_slice(&px.to_srgba_unmultiplied());
    }
    bytes
}

fn image_to_clipboard(image: &egui::ColorImage) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let bytes = unmultiplied_rgba(image);
    cb.set_image(arboard::ImageData {
        width: image.width(),
        height: image.height(),
        bytes: bytes.into(),
    })
    .map_err(|e| e.to_string())
}

/// Write the capture out as a PNG so the OS has a file to share.
fn write_png(image: &egui::ColorImage) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir();
    // The name is stable per process so a form sharing repeatedly does not fill
    // the temp directory with captures.
    let path = dir.join(format!("powerrustcobol-share-{}.png", std::process::id()));
    let (w, h) = (image.width() as u32, image.height() as u32);
    let buf = image::RgbaImage::from_raw(w, h, unmultiplied_rgba(image))
        .ok_or_else(|| "the capture did not make a valid image".to_owned())?;
    buf.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Hand a file to the OS share mechanism.
fn share_file(path: &Path) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // `open -R` reveals it in Finder, where the OS share menu lives. macOS
        // has no command-line entry point to NSSharingServicePicker, and putting
        // the file in front of the operator with the system's own share menu one
        // click away beats claiming a share sheet we cannot raise.
        spawn("open", &["-R", &path.display().to_string()])?;
        Ok(format!(
            "Window image saved to {} and revealed for sharing",
            path.display()
        ))
    }
    #[cfg(target_os = "windows")]
    {
        spawn("explorer", &["/select,", &path.display().to_string()])?;
        Ok(format!(
            "Window image saved to {} and revealed for sharing",
            path.display()
        ))
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        spawn("xdg-open", &[&path.parent().unwrap_or(path).display().to_string()])?;
        Ok(format!("Window image saved to {}", path.display()))
    }
}

/// Send a document to the OS print dialog.
fn print_file(target: &str) -> Outcome {
    let target = target.trim();
    if target.is_empty() {
        return Outcome::Failed(
            "This Print button names no file — set its target to a path".into(),
        );
    }
    let path = Path::new(target);
    if !path.exists() {
        return Outcome::Failed(format!("Nothing to print: {target} does not exist"));
    }
    // Open the document in the platform's default viewer, which is where its
    // print dialog is. Printing it straight to the default printer would skip
    // the dialog the operator was promised.
    #[cfg(target_os = "macos")]
    let result = spawn("open", &[target]);
    #[cfg(target_os = "windows")]
    let result = spawn("cmd", &["/C", "start", "", target]);
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let result = spawn("xdg-open", &[target]);

    match result {
        Ok(()) => Outcome::Done(format!("Opened {target} for printing")),
        Err(e) => Outcome::Failed(format!("Could not open {target} to print: {e}")),
    }
}

/// Launch another application. The target is a path, then arguments.
///
/// Split on whitespace, and handed to the OS directly — never to a shell. A form
/// building this string out of a data item must not be able to turn it into a
/// shell command.
fn run_app(target: &str) -> Outcome {
    let mut parts = target.trim().split_whitespace();
    let Some(program) = parts.next() else {
        return Outcome::Failed(
            "This Run button names no application — set its target".into(),
        );
    };
    let args: Vec<&str> = parts.collect();
    // On macOS a `.app` bundle is opened, not executed.
    #[cfg(target_os = "macos")]
    if program.ends_with(".app") || !program.contains('/') {
        let mut open_args = vec!["-a", program];
        if !args.is_empty() {
            open_args.push("--args");
            open_args.extend(args.iter().copied());
        }
        return match spawn("open", &open_args) {
            Ok(()) => Outcome::Done(format!("Launched {program}")),
            Err(e) => Outcome::Failed(format!("Could not launch {program}: {e}")),
        };
    }
    match spawn(program, &args) {
        Ok(()) => Outcome::Done(format!("Launched {program}")),
        Err(e) => Outcome::Failed(format!("Could not launch {program}: {e}")),
    }
}

fn open_terminal(dir: &str) -> Outcome {
    let dir = dir.trim();
    let where_ = if dir.is_empty() {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_owned())
    } else {
        dir.to_owned()
    };
    #[cfg(target_os = "macos")]
    let result = spawn("open", &["-a", "Terminal", &where_]);
    #[cfg(target_os = "windows")]
    let result = spawn("cmd", &["/C", "start", "cmd", "/K", "cd", "/D", &where_]);
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let result = spawn("x-terminal-emulator", &["--working-directory", &where_]);

    match result {
        Ok(()) => Outcome::Done(format!("Opened a terminal in {where_}")),
        Err(e) => Outcome::Failed(format!("Could not open a terminal: {e}")),
    }
}

/// Start a process and do not wait for it. No shell is involved.
fn spawn(program: &str, args: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action reports something. A press that cannot be honoured says why,
    /// because a button that appears dead is the worst answer available.
    #[test]
    fn an_action_that_cannot_be_honoured_says_why() {
        // A Print with no target, and one whose file is not there.
        assert!(matches!(print_file(""), Outcome::Failed(_)));
        let missing = print_file("/definitely/not/here/report.pdf");
        assert!(
            matches!(&missing, Outcome::Failed(m) if m.contains("does not exist")),
            "got {missing:?}"
        );

        // A Run with no target.
        assert!(matches!(run_app("   "), Outcome::Failed(_)));
        // A Run naming something unlaunchable fails with the OS's reason rather
        // than panicking.
        let bad = run_app("/definitely/not/here/app-binary");
        assert!(matches!(bad, Outcome::Failed(_)), "got {bad:?}");

        // The COBOL-side actions must never be routed here.
        let mut runner = Runner::default();
        let ctx = egui::Context::default();
        for action in [
            ToolbarAction::Event,
            ToolbarAction::Procedure("P".into()),
            ToolbarAction::OpenModal("F".into()),
        ] {
            let (outcome, text) = runner.perform(&ctx, &action, None);
            assert!(matches!(outcome, Outcome::Failed(_)), "{action:?}");
            assert!(text.is_none());
        }

        println!(
            "\n  Toolbar platform actions — Print with no target / a missing file, Run \
             with no target / an unlaunchable path, and the three COBOL-side actions \
             all report a reason instead of doing nothing\n"
        );
    }

    /// The clipboard verbs need a focused text field, and say so when there is
    /// none. Cut empties it; Copy leaves it alone.
    #[test]
    fn the_clipboard_verbs_need_a_focused_field_and_report_what_they_moved() {
        let mut runner = Runner::default();
        let ctx = egui::Context::default();

        for action in [ToolbarAction::Copy, ToolbarAction::Cut, ToolbarAction::Paste] {
            let (outcome, text) = runner.perform(&ctx, &action, None);
            assert!(
                matches!(&outcome, Outcome::Failed(m) if m.contains("focus")),
                "{action:?} with no focus must say so, got {outcome:?}"
            );
            assert!(text.is_none(), "{action:?} must not change anything");
        }

        // Copy reports the count and changes nothing.
        let (outcome, text) = runner.perform(
            &ctx,
            &ToolbarAction::Copy,
            Some(Focused {
                control_id: "TXT-1",
                text: "hello".into(),
            }),
        );
        assert!(matches!(&outcome, Outcome::Done(m) if m.contains("5 character(s)")), "{outcome:?}");
        assert_eq!(text, None, "Copy leaves the field as it was");

        // Cut reports the same and empties the field.
        let (outcome, text) = runner.perform(
            &ctx,
            &ToolbarAction::Cut,
            Some(Focused {
                control_id: "TXT-1",
                text: "hello".into(),
            }),
        );
        assert!(matches!(&outcome, Outcome::Done(_)), "{outcome:?}");
        assert_eq!(text, Some(String::new()), "Cut empties the field");

        println!(
            "\n  Toolbar clipboard — Copy/Cut/Paste with no focused field report it; \
             Copy of \"hello\" reports 5 characters and leaves the field, Cut reports \
             5 and empties it\n"
        );
    }

    /// A capture is asked for on one frame and finished on a later one. Until the
    /// image arrives the press reads as pending, not as done and not as failed.
    #[test]
    fn a_capture_is_pending_until_the_image_arrives() {
        let mut runner = Runner::default();
        let ctx = egui::Context::default();

        for action in [ToolbarAction::Screenshot, ToolbarAction::Share] {
            let (outcome, _) = runner.perform(&ctx, &action, None);
            assert!(
                matches!(outcome, Outcome::Pending(_)),
                "{action:?} must report pending, got {outcome:?}"
            );
            assert!(runner.pending.is_some(), "…and remember what it wanted");
            // No image on the input queue yet, so polling changes nothing.
            assert!(runner.poll_capture(&ctx).is_none());
            assert!(runner.pending.is_some(), "still waiting");
            runner.pending = None;
        }
        // Polling with nothing pending is a no-op.
        assert!(runner.poll_capture(&ctx).is_none());

        println!(
            "\n  Toolbar capture — screenshot and share both report Pending and hold \
             their intent; polling before the image arrives leaves them waiting; \
             polling with nothing pending is a no-op\n"
        );
    }
}
