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

use cobolt_project_tools::web::package::{package_web, PackageError, PackageOptions, Packaged, Step};

use crate::i18n::Tr;

/// What the thread sends to the frame.
pub enum Msg {
    /// A step began.
    Step(Step),
    /// The package was made, or why not. Always the last message.
    Finished(Result<Packaged, PackageError>),
}

/// What a package is for, besides the folder it goes into: the system (`None` is this machine's own) and where the runner
/// of another system comes from (spec 115). The window of Package Project makes it, and so does the `package_project` tool.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Choice {
    pub system: Option<cobolt_compiler::backend_runners::System>,
    /// A folder that holds the runner of that system, when the developer named one.
    pub runner_dir: Option<PathBuf>,
    /// The download of the runner is allowed (the window asked the developer; the tool was told by its caller).
    pub fetch_runner: bool,
    /// The language code of the install script when the developer chose one in the window; none is the IDE's own.
    pub lang: Option<&'static str>,
}

impl Choice {
    /// What a `package_project` call asks for (spec 115 R2, R8): `target` is one of `offered` (the list `rcrun package
    /// --target` reads), `runner_dir` an existing absolute folder, `fetch_runner` the permission to download. No such
    /// argument is no choice, which is the package of this computer's own system. They are for a Web project only.
    pub fn from_tool_args(args: &serde_json::Value, web: bool, offered: &[cobolt_compiler::backend_runners::System]) -> Result<Choice, String> {
        let text = |key: &str| args.get(key).and_then(serde_json::Value::as_str).map(str::trim).filter(|s| !s.is_empty());
        let (target, runner_dir, fetch_runner) = (text("target"), text("runner_dir"), args.get("fetch_runner").and_then(serde_json::Value::as_bool).unwrap_or(false));
        if target.is_none() && runner_dir.is_none() && !fetch_runner {
            return Ok(Choice::default());
        }
        if !web {
            return Err("`target`, `runner_dir` and `fetch_runner` apply only to a Web project: only a Web project is packaged for another system".to_owned());
        }
        let Some(word) = target else {
            return Err("`runner_dir` and `fetch_runner` say where the runner of another system comes from: they need `target`".to_owned());
        };
        let system = cobolt_compiler::backend_runners::System::choose(word, offered)?;
        let runner_dir = match runner_dir {
            Some(d) => {
                let p = PathBuf::from(d);
                if !p.is_absolute() || !p.is_dir() {
                    return Err(format!("'{d}' is not an existing absolute folder"));
                }
                Some(p)
            }
            None => None,
        };
        Ok(Choice { system: Some(system), runner_dir, fetch_runner, lang: None })
    }
}

/// The systems this installation can package for (spec 115 R2): this computer's own, then each one the installer's list
/// has a runner for. Read once, since the list is a file of the installation and the settings panel asks every frame.
pub fn offered_systems() -> &'static [cobolt_compiler::backend_runners::System] {
    use cobolt_compiler::backend_runners::{RunnerManifest, System};
    static OFFERED: std::sync::OnceLock<Vec<System>> = std::sync::OnceLock::new();
    OFFERED.get_or_init(|| System::offered(&RunnerManifest::find()))
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
    pub fn start(manifest: PathBuf, dir: PathBuf, lang: &'static str, choice: Choice, ctx: egui::Context) -> Run {
        // The script is in the language the window chose, else the IDE's own.
        let lang = choice.lang.unwrap_or(lang);
        let (tx, rx) = channel();
        let out = dir.clone();
        let spawned = std::thread::Builder::new().name("package-web".into()).spawn({
            let tx = tx.clone();
            let ctx = ctx.clone();
            move || {
                let progress = tx.clone();
                let repaint = ctx.clone();
                let options = PackageOptions {
                    lang: lang.to_string(),
                    system: choice.system,
                    runner_dir: choice.runner_dir,
                    fetch_runner: choice.fetch_runner,
                    version: crate::version::VERSION.to_string(),
                    cache_root: None,
                };
                let result = package_web(&manifest, &out, &options, &mut |step| {
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

/// The line that says the packaging begins: for a system other than this computer's own, it names the system.
pub fn started_line(tr: &Tr, name: &str, system: Option<cobolt_compiler::backend_runners::System>) -> String {
    match system.filter(|s| Some(*s) != cobolt_compiler::backend_runners::System::host()) {
        Some(system) => fill(tr.webpack_started_for, &[("name", name.to_owned()), ("system", system.to_string())]),
        None => fill(tr.webpack_started, &[("name", name.to_owned())]),
    }
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
            // How the Backend was made (spec 114 R13, R16): the module path says its time, the Cargo path says why. The reasons stay as
            // the Build words them, English like every report of the tools.
            if let Some(made) = &p.backend {
                match made.engine {
                    cobolt_compiler::backend_module::BackendEngine::Module => {
                        let ms: u128 = made.phases.iter().map(|(_, ms)| ms).sum();
                        out.push(fill(tr.webpack_backend_module, &[("ms", ms.to_string())]));
                    }
                    cobolt_compiler::backend_module::BackendEngine::Cargo => {
                        for why in &made.reasons {
                            out.push(fill(tr.webpack_backend_cargo, &[("why", why.clone())]));
                        }
                    }
                }
            }
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
            // What Prove did and did not run for another system (spec 115 R6), in the developer's language: the library's
            // English sentence of it is the one left out here.
            let twin = p.system.filter(|_| !p.executable_run);
            if let Some(system) = twin {
                out.push(fill(tr.webpack_not_run, &[("system", system.to_string())]));
            }
            out.extend(p.notes.iter().filter(|n| twin.map_or(true, |s| **n != cobolt_project_tools::web::package::twin_note(s))).cloned());
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
            system: None,
            executable_run: true,
            wss,
            required_secrets: secrets.iter().map(|s| (*s).to_owned()).collect(),
            backend: None,
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
            let start = started_line(&tr, "Power Oficina", None);
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

    /// Spec 115 T14 (R6, R7, AC7): a package for another system says so when it begins, and says in the developer's own
    /// language what was proven on this machine and what was not run, instead of the library's English sentence; this
    /// computer's own system keeps the line it always had.
    #[test]
    fn a_package_for_another_system_names_it_and_says_what_was_not_run_in_every_language() {
        use cobolt_compiler::backend_runners::{Arch, Os, System};
        use cobolt_project_tools::web::package::twin_note;
        let linux = System { os: Os::Linux, arch: Arch::X86_64 };
        let other = if System::host() == Some(linux) { System { os: Os::Linux, arch: Arch::Aarch64 } } else { linux };
        let mut checked = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            let own = started_line(&tr, "Power Oficina", None);
            assert_eq!(started_line(&tr, "Power Oficina", System::host()), own, "{}: the machine's own system keeps its line", lang.code());
            let start = started_line(&tr, "Power Oficina", Some(other));
            assert!(start.contains("Power Oficina") && start.contains(&other.to_string()) && !start.contains('{'), "{}: {start}", lang.code());
            assert_ne!(start, own, "{}", lang.code());

            let mut foreign = packaged(false, &[]);
            foreign.system = Some(other);
            foreign.executable_run = false;
            foreign.notes = vec![twin_note(other), "a note".into()];
            let said = lines(&tr, &Msg::Finished(Ok(foreign)), Path::new("/out")).join("\n");
            assert!(said.contains(&fill(tr.webpack_not_run, &[("system", other.to_string())])), "{}: the translated sentence is said: {said}", lang.code());
            assert!(!said.contains(&twin_note(other)) && said.contains("a note") && !said.contains('{'), "{}: the library's English sentence is left out, the other notes stay: {said}", lang.code());

            let mut own_system = packaged(false, &[]);
            own_system.system = System::host();
            own_system.notes = vec!["a note".into()];
            let said = lines(&tr, &Msg::Finished(Ok(own_system)), Path::new("/out")).join("\n");
            assert!(!said.contains(&fill(tr.webpack_not_run, &[("system", other.to_string())])), "{}: nothing about a twin for this computer's own system", lang.code());
            checked += 6;
        }
        println!("web package for another system: {checked} checks across {} languages; PASS {checked:03} FAIL 000", Language::ALL.len());
    }

    /// Spec 114 R13, R16 (T33): the result says how the Backend was made, in every language: the module path its time, the Cargo
    /// path each reason (kept as the Build words it), and a project with no Backend nothing.
    #[test]
    fn the_result_says_how_the_backend_was_made_in_every_language() {
        use cobolt_compiler::backend_module::BackendEngine;
        use cobolt_project_tools::web::package::BackendMade;
        let mut checked = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            let mut module = packaged(false, &[]);
            module.backend = Some(BackendMade { engine: BackendEngine::Module, reasons: vec![], phases: vec![("check".into(), 7), ("assemble".into(), 4915)] });
            let said = lines(&tr, &Msg::Finished(Ok(module)), Path::new("/out")).join("\n");
            assert!(said.contains("4922") && !said.contains('{'), "{}: the module line says its time: {said}", lang.code());
            let mut cargo = packaged(false, &[]);
            cargo.backend = Some(BackendMade { engine: BackendEngine::Cargo, reasons: vec!["backend.toml has an [identity] section".into(), "backend/src/a.cbl line 3 holds an EXEC RUST block".into()], phases: vec![] });
            let said = lines(&tr, &Msg::Finished(Ok(cargo)), Path::new("/out")).join("\n");
            assert!(said.contains("[identity] section") && said.contains("EXEC RUST") && !said.contains('{'), "{}: each Cargo reason is named: {said}", lang.code());
            let none = lines(&tr, &Msg::Finished(Ok(packaged(false, &[]))), Path::new("/out")).join("\n");
            assert!(!none.contains("4922") && !none.contains("EXEC RUST"), "{}: no engine line without a Backend: {none}", lang.code());
            checked += 3;
        }
        println!("web package engine lines: {checked} checks across {} languages; PASS {checked:03} FAIL 000", Language::ALL.len());
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

    /// Spec 115 T11 (R2, R8, AC8): the arguments of `package_project` are read against the one list of systems, refused the way
    /// `rcrun package --target` refuses them, and mean nothing for a Desktop or Mobile project.
    #[test]
    fn the_arguments_of_package_project_are_checked_against_the_one_list() {
        use cobolt_compiler::backend_runners::{Arch, Os, System};
        use serde_json::json;
        let began = std::time::Instant::now();
        let linux = System { os: Os::Linux, arch: Arch::X86_64 };
        let offered = [System { os: Os::MacOs, arch: Arch::Aarch64 }, linux];
        let folder = std::env::temp_dir();
        let mut cases = 0;

        // No argument is no choice: this computer's own system, nothing downloaded.
        let none = Choice::from_tool_args(&json!({ "to": "/somewhere" }), true, &offered).unwrap();
        assert!(none.system.is_none() && none.runner_dir.is_none() && !none.fetch_runner);
        cases += 1;
        // A system of the list, with a folder and with permission to download.
        let chosen = Choice::from_tool_args(&json!({ "target": "linux-x86_64", "runner_dir": folder.to_str().unwrap(), "fetch_runner": true }), true, &offered).unwrap();
        assert_eq!((chosen.system, chosen.runner_dir.as_deref(), chosen.fetch_runner), (Some(linux), Some(folder.as_path()), true));
        cases += 1;
        // A word that is not in the list names the list: the same words as the command line (`System::choose`).
        let bogus = Choice::from_tool_args(&json!({ "target": "bogus" }), true, &offered).unwrap_err();
        assert_eq!(bogus, System::choose("bogus", &offered).unwrap_err(), "the command line and the tool refuse in the same words");
        assert!(bogus.contains("the systems are: macos-aarch64, linux-x86_64"), "{bogus}");
        let unlisted = Choice::from_tool_args(&json!({ "target": "linux-aarch64" }), true, &offered).unwrap_err();
        assert!(unlisted.contains("\"linux-aarch64\""), "{unlisted}");
        cases += 2;
        // The two arguments that say where the runner comes from need a target, and the folder must exist.
        for args in [json!({ "runner_dir": folder.to_str().unwrap() }), json!({ "fetch_runner": true })] {
            let why = Choice::from_tool_args(&args, true, &offered).unwrap_err();
            assert!(why.contains("need `target`"), "{why}");
            cases += 1;
        }
        let relative = Choice::from_tool_args(&json!({ "target": "linux-x86_64", "runner_dir": "runners" }), true, &offered).unwrap_err();
        let missing = Choice::from_tool_args(&json!({ "target": "linux-x86_64", "runner_dir": "/no/such/folder/115" }), true, &offered).unwrap_err();
        assert!(relative.contains("not an existing absolute folder") && missing.contains("not an existing absolute folder"));
        cases += 2;
        // A Desktop or Mobile project is not packaged for another system.
        for args in [json!({ "target": "linux-x86_64" }), json!({ "fetch_runner": true }), json!({ "runner_dir": folder.to_str().unwrap() })] {
            let why = Choice::from_tool_args(&args, false, &offered).unwrap_err();
            assert!(why.contains("only a Web project is packaged for another system"), "{why}");
            cases += 1;
        }
        assert!(Choice::from_tool_args(&json!({ "to": "/somewhere" }), false, &offered).is_ok(), "a zip of a Desktop project takes no choice and needs none");
        cases += 1;
        println!("package_project arguments: {cases} cases, {cases} passed, 0 failed, {} ms", began.elapsed().as_millis());
    }
}
