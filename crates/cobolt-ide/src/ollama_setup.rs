// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Getting a model provider on the first run: Ollama.
//!
//! Without a model provider PowerRustCOBOL AI keeps only its basic functions —
//! the Form Designer, the editor, Run and the debugger. Grace, the agents and
//! the proficiency tests all need a language model. Ollama is the simplest way
//! to get one: it runs models on this machine and reaches larger ones in its
//! cloud. Any other provider will do instead; this only makes the first one
//! one click away (operator, 2026-09-27).
//!
//! The same shape as [`crate::toolchain`]'s first-run Rust question: asked
//! once per machine, the command shown before it runs, the installer run on its
//! own thread, and the answer remembered in [`crate::ui_prefs`].

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

/// Where each platform's official installer comes from.
pub const WINDOWS_INSTALLER: &str = "https://ollama.com/download/OllamaSetup.exe";
pub const MACOS_INSTALLER: &str = "https://ollama.com/download/Ollama.dmg";
pub const LINUX_INSTALLER: &str = "https://ollama.com/install.sh";

/// The models the setup recommends: small and local for Grace and the Judge,
/// large and in the cloud for everyone else.
pub const LOCAL_MODEL: &str = "gemma4:e2b";
pub const CLOUD_MODEL: &str = "gemma4:31b";

/// Is Ollama on this machine? Its program on `PATH`, or where each platform's
/// installer puts it.
pub fn is_installed() -> bool {
    let exe = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(exe).is_file()))
        .unwrap_or(false);
    on_path || known_locations().iter().any(|p| p.exists())
}

fn known_locations() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(target_os = "macos") {
        v.push(PathBuf::from("/Applications/Ollama.app"));
        v.push(PathBuf::from("/usr/local/bin/ollama"));
        if let Some(home) = std::env::var_os("HOME") {
            v.push(Path::new(&home).join("Applications/Ollama.app"));
        }
    } else if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(Path::new(&local).join("Programs").join("Ollama").join("ollama.exe"));
        }
    } else {
        v.push(PathBuf::from("/usr/local/bin/ollama"));
        v.push(PathBuf::from("/usr/bin/ollama"));
    }
    v
}

/// How this platform installs Ollama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// A command this IDE runs: the program, its arguments, and the line the
    /// dialog shows for it — built from the same pieces so they cannot drift.
    Run { program: String, args: Vec<String>, shown: String },
    /// Nothing here can ask for the administrator's password (Linux without
    /// `pkexec`): the developer runs `shown` in a terminal.
    Manual { shown: String },
}

/// The installation method for this machine.
pub fn method() -> Method {
    if cfg!(windows) {
        let script = format!(
            "$f = Join-Path $env:TEMP 'OllamaSetup.exe'; \
             Invoke-WebRequest -UseBasicParsing '{WINDOWS_INSTALLER}' -OutFile $f; \
             Start-Process -FilePath $f -Wait"
        );
        Method::Run {
            program: "powershell".into(),
            args: vec!["-NoProfile".into(), "-Command".into(), script.clone()],
            shown: script,
        }
    } else if cfg!(target_os = "macos") {
        // The official app: downloaded and opened; the developer drags it to
        // Applications as with any Mac app.
        let script = format!(
            "f=\"${{TMPDIR:-/tmp}}/Ollama.dmg\"; curl -fL -o \"$f\" {MACOS_INSTALLER} && open \"$f\""
        );
        Method::Run { program: "sh".into(), args: vec!["-c".into(), script.clone()], shown: script }
    } else {
        // The official script needs root; `pkexec` asks for the password in a
        // desktop window, which a program started from the menu can show.
        let script = format!("curl -fsSL {LINUX_INSTALLER} | sh");
        if program_on_path("pkexec") {
            Method::Run {
                program: "pkexec".into(),
                args: vec!["sh".into(), "-c".into(), script.clone()],
                shown: format!("pkexec sh -c '{script}'"),
            }
        } else {
            Method::Manual { shown: script }
        }
    }
}

fn program_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
        .unwrap_or(false)
}

/// How an installation ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Ollama answers on this machine now.
    Installed,
    /// The platform's own installer is open and the developer finishes it there
    /// (the Mac's disk image).
    Handed,
    /// It did not work; the installer's last words.
    Failed(String),
}

/// Run the installer on its own thread; the receiver yields exactly one outcome.
/// Only ever called because the developer pressed the button.
pub fn spawn_install() -> Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_install());
    });
    rx
}

fn run_install() -> Outcome {
    let Method::Run { program, args, .. } = method() else {
        return Outcome::Failed(String::new());
    };
    match std::process::Command::new(&program).args(&args).output() {
        Err(e) => Outcome::Failed(format!("{program}: {e}")),
        Ok(o) if !o.status.success() => Outcome::Failed(last_lines(&String::from_utf8_lossy(&o.stderr))),
        Ok(_) if is_installed() => Outcome::Installed,
        // Exit 0 and nothing installed yet: the Mac's disk image is open and
        // waiting for the drag to Applications.
        Ok(_) if cfg!(target_os = "macos") => Outcome::Handed,
        Ok(o) => Outcome::Failed(last_lines(&String::from_utf8_lossy(&o.stdout))),
    }
}

fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(4)..].join("\n")
}

/// The first-run question: whether an installation is under way, and how it
/// ended.
pub struct FirstRunPrompt {
    pub install: Option<Install>,
}

pub enum Install {
    Running(Receiver<Outcome>),
    Finished(Outcome),
}

impl FirstRunPrompt {
    /// A prompt, or `None` when Ollama is already here.
    pub fn when_missing() -> Option<Self> {
        (!is_installed()).then_some(Self { install: None })
    }

    pub fn start_install(&mut self) {
        self.install = Some(Install::Running(spawn_install()));
    }

    pub fn poll_install(&mut self) {
        if let Some(Install::Running(rx)) = &self.install {
            if let Ok(outcome) = rx.try_recv() {
                self.install = Some(Install::Finished(outcome));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the dialog shows is what runs: the displayed line is built from
    /// the same script the command carries, and every script names the
    /// official installer for its platform.
    #[test]
    fn the_command_shown_is_the_command_run() {
        match method() {
            Method::Run { program, args, shown } => {
                assert!(!program.is_empty());
                let script = args.last().expect("a script");
                assert!(shown.contains(script.as_str()) || shown == *script, "{shown} / {script}");
                assert!(script.contains("ollama.com"), "{script}");
            }
            Method::Manual { shown } => assert!(shown.contains(LINUX_INSTALLER)),
        }
    }

    #[test]
    fn the_recommended_models_are_the_operators() {
        assert_eq!(LOCAL_MODEL, "gemma4:e2b");
        assert_eq!(CLOUD_MODEL, "gemma4:31b");
    }
}
