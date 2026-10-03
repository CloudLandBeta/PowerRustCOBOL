// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Configure Claude Code** (spec 084 R1–R6): install or update the
//! PowerRustCOBOL plugin in the developer's Claude Code, at user scope, with
//! the IDE's port and access token — through Claude Code's own CLI only (R2).
//! It never writes `~/.claude.json` or `~/.claude/settings.json`.
//!
//! The commands (T1 spike, Claude Code 2.1.158), each idempotent:
//!
//! 1. `claude plugin list --json` — what is installed now;
//! 2. `claude plugin marketplace add <bundle>` — the IDE's local marketplace;
//! 3. `claude plugin marketplace update powerrustcobol` — read its new version;
//! 4. `claude plugin install powerrustcobol@powerrustcobol --scope user
//!    --config port=<p> --config token=<t>` — installs, or updates the options;
//! 5. `claude plugin update powerrustcobol@powerrustcobol` — when an older
//!    version is installed.
//!
//! The token is passed as an argument and never shown: every message this
//! module returns has it replaced by `<token>` (R38).

use std::path::{Path, PathBuf};

use super::bundle::{plugin_ref, MARKETPLACE};

/// What one command printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a program — `std::process::Command` in the IDE, a recorder in tests.
pub trait CommandRunner {
    fn run(&self, program: &Path, args: &[String]) -> std::io::Result<CmdOutput>;
}

/// The real one.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &Path, args: &[String]) -> std::io::Result<CmdOutput> {
        let out = std::process::Command::new(program).args(args).output()?;
        Ok(CmdOutput {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Where `claude` is looked for besides `PATH`: an IDE started from the
/// desktop often has no shell `PATH`, so the usual install folders too.
pub fn candidate_dirs(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = home {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".claude/local"));
        dirs.push(home.join(".npm-global/bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

/// The `claude` executable, if one is installed.
pub fn find_claude(dirs: &[PathBuf]) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// What a successful run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The plugin version installed before, if any.
    pub before: Option<String>,
    /// The plugin version installed now.
    pub after: String,
}

impl Outcome {
    /// Nothing new was installed (the options may still have been restated).
    pub fn unchanged(&self) -> bool {
        self.before.as_deref() == Some(self.after.as_str())
    }
}

/// Why Configure did not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// No `claude` was found; the folders that were searched.
    NotFound(Vec<PathBuf>),
    /// A command failed: the command (token hidden) and its own message.
    Command { command: String, message: String },
}

/// The installed version of the plugin, from `claude plugin list --json`.
pub fn installed_version(list_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(list_json).ok()?;
    v.as_array()?
        .iter()
        .find(|p| p["id"].as_str() == Some(plugin_ref().as_str()))
        .and_then(|p| p["version"].as_str())
        .map(str::to_owned)
}

/// Run Configure with `runner` against `claude` (R1–R4). `bundle` is the
/// written marketplace; `version` the IDE's.
pub fn configure(
    runner: &dyn CommandRunner,
    claude: Option<&Path>,
    searched: &[PathBuf],
    bundle: &Path,
    port: u16,
    token: &str,
    version: &str,
) -> Result<Outcome, Failure> {
    let Some(claude) = claude else {
        return Err(Failure::NotFound(searched.to_vec()));
    };
    let hide = |s: &str| if token.is_empty() { s.to_owned() } else { s.replace(token, "<token>") };
    let step = |args: Vec<String>| -> Result<CmdOutput, Failure> {
        let shown = hide(&format!("claude {}", args.join(" ")));
        match runner.run(claude, &args) {
            Ok(out) if out.ok => Ok(out),
            Ok(out) => {
                let message = if out.stderr.trim().is_empty() { out.stdout } else { out.stderr };
                Err(Failure::Command { command: shown, message: hide(message.trim()) })
            }
            Err(e) => Err(Failure::Command { command: shown, message: hide(&e.to_string()) }),
        }
    };
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    let before = installed_version(&step(s(&["plugin", "list", "--json"]))?.stdout);
    step(vec!["plugin".into(), "marketplace".into(), "add".into(), bundle.to_string_lossy().into_owned()])?;
    step(s(&["plugin", "marketplace", "update", MARKETPLACE]))?;
    step(vec![
        "plugin".into(),
        "install".into(),
        plugin_ref(),
        "--scope".into(),
        "user".into(),
        "--config".into(),
        format!("port={port}"),
        "--config".into(),
        format!("token={token}"),
    ])?;
    if before.as_deref().is_some_and(|b| b != version) {
        step(vec!["plugin".into(), "update".into(), plugin_ref()])?;
    }
    let after = installed_version(&step(s(&["plugin", "list", "--json"]))?.stdout).ok_or_else(|| Failure::Command {
        command: "claude plugin list --json".into(),
        message: "the PowerRustCOBOL plugin is not listed after installing it".into(),
    })?;
    Ok(Outcome { before, after })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A stand-in `claude`: records every command; `list --json` answers with
    /// `installed`, which `install`/`update` then set to `version`; any command
    /// containing `fail_on` fails with `fail_msg`.
    struct Recorder {
        calls: RefCell<Vec<String>>,
        installed: RefCell<Option<String>>,
        version: String,
        fail_on: Option<&'static str>,
        fail_msg: String,
    }

    impl Recorder {
        fn new(installed: Option<&str>, version: &str) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                installed: RefCell::new(installed.map(str::to_owned)),
                version: version.to_owned(),
                fail_on: None,
                fail_msg: String::new(),
            }
        }
    }

    impl CommandRunner for Recorder {
        fn run(&self, program: &Path, args: &[String]) -> std::io::Result<CmdOutput> {
            let line = args.join(" ");
            self.calls.borrow_mut().push(format!("{} {line}", program.file_name().unwrap().to_string_lossy()));
            if self.fail_on.is_some_and(|f| line.contains(f)) {
                return Ok(CmdOutput { ok: false, stdout: String::new(), stderr: self.fail_msg.clone() });
            }
            if line == "plugin list --json" {
                let list = match &*self.installed.borrow() {
                    Some(v) => format!(r#"[{{"id":"powerrustcobol@powerrustcobol","version":"{v}","scope":"user"}}]"#),
                    None => "[]".into(),
                };
                return Ok(CmdOutput { ok: true, stdout: list, stderr: String::new() });
            }
            if line.starts_with("plugin install") || line.starts_with("plugin update") {
                *self.installed.borrow_mut() = Some(self.version.clone());
            }
            Ok(CmdOutput { ok: true, ..Default::default() })
        }
    }

    const TOKEN: &str = "tok-0123456789abcdef";

    fn run(rec: &Recorder) -> Result<Outcome, Failure> {
        configure(rec, Some(Path::new("/x/claude")), &[], Path::new("/data/marketplace"), 5720, TOKEN, &rec.version)
    }

    /// Spec 084 AC1/AC2: a first install runs exactly the `claude` commands
    /// above and nothing else; running it again changes nothing; a newer IDE
    /// updates the installed plugin.
    #[test]
    fn configure_runs_only_claude_commands_and_is_idempotent() {
        let fresh = Recorder::new(None, "1.80.115");
        let o = run(&fresh).unwrap();
        assert_eq!(o, Outcome { before: None, after: "1.80.115".into() });
        assert_eq!(
            *fresh.calls.borrow(),
            [
                "claude plugin list --json",
                "claude plugin marketplace add /data/marketplace",
                "claude plugin marketplace update powerrustcobol",
                &format!("claude plugin install powerrustcobol@powerrustcobol --scope user --config port=5720 --config token={TOKEN}"),
                "claude plugin list --json",
            ]
        );
        let again = Recorder::new(Some("1.80.115"), "1.80.115");
        assert!(run(&again).unwrap().unchanged(), "configuring again changes nothing");
        assert!(!again.calls.borrow().iter().any(|c| c.contains("plugin update")));
        let older = Recorder::new(Some("1.80.101"), "1.80.115");
        let o = run(&older).unwrap();
        assert_eq!((o.before.as_deref(), o.after.as_str()), (Some("1.80.101"), "1.80.115"));
        assert!(older.calls.borrow().iter().any(|c| c == "claude plugin update powerrustcobol@powerrustcobol"));
        for rec in [&fresh, &again, &older] {
            assert!(rec.calls.borrow().iter().all(|c| c.starts_with("claude plugin ")), "only claude commands");
        }
        println!("configure: first install 5 commands; again -> unchanged, no update; older plugin -> update; only `claude plugin` commands ever run");
    }

    /// Spec 084 AC3: no `claude` → NotFound with the folders searched, nothing
    /// run; a failing command → its own message, the token never shown.
    #[test]
    fn a_missing_or_failing_claude_is_reported_without_the_token() {
        let rec = Recorder::new(None, "1.80.115");
        let searched = vec![PathBuf::from("/opt/homebrew/bin")];
        let e = configure(&rec, None, &searched, Path::new("/m"), 5720, TOKEN, "1.80.115").unwrap_err();
        assert_eq!(e, Failure::NotFound(searched));
        assert!(rec.calls.borrow().is_empty(), "nothing run without claude");

        let mut failing = Recorder::new(None, "1.80.115");
        failing.fail_on = Some("plugin install");
        failing.fail_msg = format!("Error: config rejected ({TOKEN})");
        let Failure::Command { command, message } = run(&failing).unwrap_err() else { panic!("a command failure") };
        assert!(command.contains("plugin install") && command.contains("token=<token>"), "{command}");
        assert!(message.contains("config rejected") && !message.contains(TOKEN), "{message}");
        println!("configure: no claude -> not found (nothing run); failing install -> its message, token shown as <token>");
    }

    #[test]
    fn claude_is_found_in_a_listed_folder() {
        let dir = std::env::temp_dir().join(format!("prc-084-claude-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(find_claude(&[dir.clone()]), None);
        let exe = dir.join(if cfg!(windows) { "claude.exe" } else { "claude" });
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(find_claude(&[PathBuf::from("/nowhere"), dir.clone()]), Some(exe));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
