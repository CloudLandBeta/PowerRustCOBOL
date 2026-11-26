// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Package Project for a Web project (spec 113): `cobolt_project_tools::web::package::package_web` on a background
//! thread, its steps and its result handed to the frame as messages, and the words the Output panel says about them in
//! the developer's language. The work is the tools' (the same function `rcrun package` and the MCP tool call); this file
//! knows only threads and text.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use cobolt_project_tools::web::package::{package_web, PackageError, Packaged, Step};

use crate::i18n::Tr;

/// What the thread sends to the frame.
pub enum Msg {
    /// A step began.
    Step(Step),
    /// The package was made, or why not. Always the last message.
    Finished(Result<Packaged, PackageError>),
}

/// A package being made.
pub struct Run {
    rx: Receiver<Msg>,
    /// The folder the two files go into.
    pub dir: PathBuf,
    over: bool,
}

impl Run {
    /// Start packaging the project at `manifest` into `dir`; `lang` is the language code of the script.
    pub fn start(manifest: PathBuf, dir: PathBuf, lang: &'static str, ctx: egui::Context) -> Run {
        let (tx, rx) = channel();
        let out = dir.clone();
        let spawned = std::thread::Builder::new().name("package-web".into()).spawn({
            let tx = tx.clone();
            let ctx = ctx.clone();
            move || {
                let progress = tx.clone();
                let repaint = ctx.clone();
                let result = package_web(&manifest, &out, lang, &mut |step| {
                    let _ = progress.send(Msg::Step(step));
                    repaint.request_repaint();
                });
                let _ = tx.send(Msg::Finished(result));
                ctx.request_repaint();
            }
        });
        if let Err(e) = spawned {
            let _ = tx.send(Msg::Finished(Err(PackageError::Failed(format!("cannot start the packaging thread: {e}")))));
        }
        Run { rx, dir, over: false }
    }

    /// Everything that arrived since the last frame. A thread that ended without saying how (it panicked) is a failure.
    pub fn drain(&mut self) -> Vec<Msg> {
        let mut got = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(msg) => {
                    if matches!(msg, Msg::Finished(_)) {
                        self.over = true;
                    }
                    got.push(msg);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.over {
                        self.over = true;
                        got.push(Msg::Finished(Err(PackageError::Failed("the packaging stopped unexpectedly".into()))));
                    }
                    break;
                }
            }
        }
        got
    }

    /// Has the last message been drained?
    pub fn is_over(&self) -> bool {
        self.over
    }
}

fn fill(template: &str, pairs: &[(&str, String)]) -> String {
    pairs.iter().fold(template.to_owned(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

/// The line that says the packaging begins.
pub fn started_line(tr: &Tr, name: &str) -> String {
    fill(tr.webpack_started, &[("name", name.to_owned())])
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// The Output panel's lines for one message, in the developer's language. What a refusal or a failure says (it names a
/// setting of the project, a file or a tool) stays as the tools wrote it, English like every report of the tools.
pub fn lines(tr: &Tr, msg: &Msg, dir: &Path) -> Vec<String> {
    match msg {
        Msg::Step(step) => vec![match step {
            Step::Check => tr.webpack_step_check,
            Step::Page => tr.webpack_step_page,
            Step::Backend => tr.webpack_step_backend,
            Step::Prove => tr.webpack_step_prove,
            Step::Zip => tr.webpack_step_zip,
            Step::Script => tr.webpack_step_script,
            // The last step has no line of its own: the result says it.
            Step::Done => return Vec::new(),
        }
        .to_owned()],
        Msg::Finished(Err(PackageError::Refused(why))) => vec![fill(tr.webpack_refused, &[("why", why.clone())])],
        Msg::Finished(Err(PackageError::Failed(why))) => vec![fill(tr.webpack_failed, &[("why", why.clone())])],
        Msg::Finished(Ok(p)) => {
            let mut out = vec![fill(tr.webpack_done, &[("dir", dir.display().to_string()), ("secs", format!("{:.0}", p.seconds))])];
            let kb = p.zip_bytes.div_ceil(1024);
            out.push(fill(tr.webpack_zip_line, &[("file", file_name(&p.zip)), ("kb", kb.to_string()), ("n", p.files_in_zip.to_string())]));
            out.push(format!("   {}", file_name(&p.script)));
            out.push(tr.webpack_how.to_owned());
            out.push(format!("   {}", p.command()));
            if p.wss {
                out.push(tr.webpack_cert.to_owned());
            }
            if !p.required_secrets.is_empty() {
                out.push(fill(tr.webpack_secrets, &[("names", p.required_secrets.join(", "))]));
            }
            out.extend(p.notes.iter().cloned());
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    fn packaged(wss: bool, secrets: &[&str]) -> Packaged {
        Packaged {
            zip: PathBuf::from("/out/PowerOficina.zip"),
            script: PathBuf::from("/out/PowerOficina-install.sh"),
            zip_bytes: 2_500_000,
            sha256: "0".repeat(64),
            files_in_zip: 321,
            has_backend: true,
            wss,
            required_secrets: secrets.iter().map(|s| (*s).to_owned()).collect(),
            notes: vec!["a note".into()],
            timings: Vec::new(),
            seconds: 183.4,
        }
    }

    #[test]
    fn every_language_says_every_step_and_the_result_with_nothing_left_unfilled() {
        let mut said = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            let dir = Path::new("/out");
            let start = started_line(&tr, "Power Oficina");
            assert!(start.contains("Power Oficina") && !start.contains('{'), "{}: {start}", lang.code());
            let mut all = vec![start];
            for step in [Step::Check, Step::Page, Step::Backend, Step::Prove, Step::Zip, Step::Script] {
                all.extend(lines(&tr, &Msg::Step(step), dir));
            }
            assert!(lines(&tr, &Msg::Step(Step::Done), dir).is_empty());
            all.extend(lines(&tr, &Msg::Finished(Ok(packaged(true, &["CRM_TOKEN", "COOKIE_KEY"]))), dir));
            all.extend(lines(&tr, &Msg::Finished(Err(PackageError::Refused("[server] listen is wrong".into()))), dir));
            all.extend(lines(&tr, &Msg::Finished(Err(PackageError::Failed("the disk is full".into()))), dir));
            for l in &all {
                assert!(!l.is_empty() && !l.contains('{') && !l.contains('}'), "{}: unfilled text in {l:?}", lang.code());
            }
            let text = all.join("\n");
            for must in ["/out", "PowerOficina.zip", "PowerOficina-install.sh", "321", "CRM_TOKEN, COOKIE_KEY", "[server] listen is wrong", "the disk is full", "sudo bash ./PowerOficina-install.sh", "--env-file FILE", "a note"] {
                assert!(text.contains(must), "{}: {must} in\n{text}", lang.code());
            }
            said += all.len();
        }
        println!("web package: {said} lines across {} languages, none with an unfilled slot", Language::ALL.len());
    }

    #[test]
    fn the_result_says_the_certificate_and_the_secrets_only_when_they_apply() {
        let tr = Language::English.tr();
        let plain = lines(&tr, &Msg::Finished(Ok(packaged(false, &[]))), Path::new("/out")).join("\n");
        assert!(!plain.contains("wss://") && !plain.contains("secrets"), "{plain}");
        let full = lines(&tr, &Msg::Finished(Ok(packaged(true, &["K"]))), Path::new("/out")).join("\n");
        assert!(full.contains("wss://") && full.contains("secrets"), "{full}");
    }

    #[test]
    fn a_thread_that_ends_silently_is_a_failure_and_the_run_is_over() {
        let (tx, rx) = channel();
        let mut run = Run { rx, dir: PathBuf::from("/out"), over: false };
        tx.send(Msg::Step(Step::Check)).unwrap();
        drop(tx);
        let got = run.drain();
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[1], Msg::Finished(Err(PackageError::Failed(_)))));
        assert!(run.is_over());
        assert!(run.drain().is_empty(), "once over, nothing more is said");
    }
}
