// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Git executor for the Version Control Agent (spec 030 R9–R14).
//!
//! Runs `git` **only inside the currently-open user project's repository**
//! (`project_dir`) — never the PowerRustCOBOL IDE/source repo, nor any path
//! outside the open project. Commands are built from an explicit argument vector
//! (never a shell string), the working directory is bound to the project root,
//! and arguments that could redirect git to another repository (`-C`,
//! `--git-dir`, `--work-tree`) are rejected.
//!
//! Operations are classified: read + local-mutation ops run autonomously;
//! network and history-rewriting ops are *gated* and must be confirmed by the
//! operator per-operation (the confirmation itself is wired in the tool
//! backend). Every run captures argv, cwd, exit status, stdout and stderr as
//! evidence; a non-zero exit is a **failure**, not a completion.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Safety classification of a git operation (spec 030 R12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitClass {
    /// Read or local-mutation op — runs autonomously.
    Autonomous,
    /// Network or history-rewriting op — requires explicit operator confirmation.
    Gated,
}

/// A confirmation the operator must approve before a gated op runs (spec 030 R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConfirmRequest {
    /// The exact command shown to the operator, e.g. `git push origin main`.
    pub command: String,
}

/// The captured result of a git subprocess (evidence, spec 030 R13).
#[derive(Debug, Clone)]
pub struct GitOutcome {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// Process exit code (`None` if the process was terminated by a signal).
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitOutcome {
    /// True only on a clean (zero) exit — a non-zero exit is a failure (R13).
    pub fn ok(&self) -> bool {
        self.status == Some(0)
    }

    /// One-line evidence summary.
    pub fn summary(&self) -> String {
        let code = self
            .status
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".into());
        format!("{} → exit {code}", command_string(&self.argv))
    }
}

/// Render an argv as the command the operator sees.
pub fn command_string(argv: &[String]) -> String {
    format!("git {}", argv.join(" "))
}

/// Subcommands that read or mutate only the local repository — safe to run
/// without prompting.
const AUTONOMOUS: &[&str] = &[
    "status", "diff", "log", "show", "add", "commit", "branch", "checkout", "switch", "restore",
    "stash", "tag", "init", "remote", "config", "rev-parse", "ls-files", "blame", "describe",
    "merge", "revert", "mv", "rm",
];

/// Subcommands that touch the network or rewrite history — gated behind an
/// explicit per-op operator confirmation.
const GATED: &[&str] = &["push", "fetch", "pull", "clone", "rebase", "filter-branch", "filter-repo"];

/// Arguments that could redirect git away from the open project — always rejected
/// (spec 030 R9 scope).
fn escapes_scope(arg: &str) -> bool {
    matches!(arg, "-C" | "--git-dir" | "--work-tree")
        || arg.starts_with("--git-dir=")
        || arg.starts_with("--work-tree=")
}

/// Classify a git op by its argument vector (spec 030 R12). An unrecognised
/// subcommand is rejected (R14).
pub fn classify(argv: &[String]) -> Result<GitClass, String> {
    let sub = argv
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .ok_or_else(|| "no git subcommand given".to_string())?;

    // `reset` is local unless it rewrites the working tree / index destructively.
    if sub == "reset" {
        let hard = argv
            .iter()
            .any(|a| a == "--hard" || a == "--keep" || a == "--merge");
        return Ok(if hard { GitClass::Gated } else { GitClass::Autonomous });
    }
    if GATED.contains(&sub) {
        return Ok(GitClass::Gated);
    }
    if AUTONOMOUS.contains(&sub) {
        return Ok(GitClass::Autonomous);
    }
    Err(format!(
        "git operation \u{201c}{sub}\u{201d} is not on the recognised allow-list"
    ))
}

/// Run `git argv` inside `project_dir`. Enforces the project-scope binding
/// (R9/R10) and captures full evidence (R13). Returns `Err` only when the op
/// could not be *attempted* (no project, not a repo, scope-escaping argument,
/// failure to launch git); a git command that ran and failed is a successful
/// `Ok(GitOutcome)` with `ok() == false`.
pub fn run_git(project_dir: &Path, argv: &[String]) -> Result<GitOutcome, String> {
    if !project_dir.is_dir() {
        return Err("no project is open — the git executor has no repository to act on".into());
    }
    if !is_git_repo(project_dir) {
        return Err(format!(
            "the open project is not a git repository: {}",
            project_dir.display()
        ));
    }
    for a in argv {
        if escapes_scope(a) {
            return Err(format!(
                "git argument \u{201c}{a}\u{201d} could escape the project repository and is not allowed"
            ));
        }
    }
    let out = Command::new("git")
        .args(argv)
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("failed to launch git: {e}"))?;
    Ok(GitOutcome {
        argv: argv.to_vec(),
        cwd: project_dir.to_path_buf(),
        status: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Whether `dir` is (the top of) a git repository. Accepts both a normal
/// `.git` directory and a `.git` file (worktrees / submodules).
fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    /// A throwaway directory under the system temp dir (no tempfile dep).
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let uniq = format!(
                "prc-git-{tag}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
                    + N.fetch_add(1, Ordering::Relaxed) as u128
            );
            let p = std::env::temp_dir().join(uniq);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Init a repo with an identity + one commit; return its dir.
    fn init_repo(tag: &str) -> TempDir {
        let d = TempDir::new(tag);
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .args(args)
                .current_dir(d.path())
                .output()
                .expect("git available")
                .status
                .success();
            assert!(ok, "setup `git {args:?}` failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Tester"]);
        std::fs::write(d.path().join("hello.txt"), "hi\n").unwrap();
        git(&["add", "hello.txt"]);
        git(&["commit", "-q", "-m", "seed"]);
        d
    }

    #[test]
    fn classifier_splits_autonomous_gated_rejected() {
        assert_eq!(classify(&argv(&["status"])).unwrap(), GitClass::Autonomous);
        assert_eq!(classify(&argv(&["commit", "-m", "x"])).unwrap(), GitClass::Autonomous);
        assert_eq!(classify(&argv(&["log", "--oneline"])).unwrap(), GitClass::Autonomous);
        assert_eq!(classify(&argv(&["reset"])).unwrap(), GitClass::Autonomous);
        // Network / rewrite → gated.
        assert_eq!(classify(&argv(&["push", "origin", "main"])).unwrap(), GitClass::Gated);
        assert_eq!(classify(&argv(&["push", "--force"])).unwrap(), GitClass::Gated);
        assert_eq!(classify(&argv(&["fetch"])).unwrap(), GitClass::Gated);
        assert_eq!(classify(&argv(&["pull"])).unwrap(), GitClass::Gated);
        assert_eq!(classify(&argv(&["rebase", "main"])).unwrap(), GitClass::Gated);
        assert_eq!(classify(&argv(&["reset", "--hard", "HEAD~1"])).unwrap(), GitClass::Gated);
        // Unrecognised → rejected.
        assert!(classify(&argv(&["frobnicate"])).is_err());
    }

    #[test]
    fn run_git_uses_project_cwd_and_succeeds() {
        let repo = init_repo("cwd");
        let out = run_git(repo.path(), &argv(&["status", "--porcelain"])).unwrap();
        assert!(out.ok(), "status on a clean repo exits 0: {out:?}");
        assert_eq!(out.cwd, repo.path(), "cwd is the project root, never the workspace root");
        let log = run_git(repo.path(), &argv(&["log", "--oneline"])).unwrap();
        assert!(log.stdout.contains("seed"), "log shows the seeded commit");
    }

    #[test]
    fn nonzero_exit_is_a_failure() {
        let repo = init_repo("fail");
        let out = run_git(repo.path(), &argv(&["checkout", "no-such-branch"])).unwrap();
        assert!(!out.ok(), "a failed git op reports failure, not completion");
        assert!(out.status != Some(0));
    }

    #[test]
    fn no_project_and_non_repo_error_cleanly() {
        // No project directory at all.
        let missing = std::env::temp_dir().join("prc-git-does-not-exist-xyz");
        assert!(run_git(&missing, &argv(&["status"])).is_err());
        // A real directory that is not a git repo.
        let plain = TempDir::new("plain");
        let err = run_git(plain.path(), &argv(&["status"])).unwrap_err();
        assert!(err.contains("not a git repository"), "{err}");
    }

    #[test]
    fn scope_escaping_arguments_are_rejected() {
        let repo = init_repo("scope");
        for bad in [
            argv(&["-C", "/etc", "status"]),
            argv(&["--git-dir=/tmp/other/.git", "status"]),
            argv(&["--work-tree", "/", "status"]),
        ] {
            assert!(
                run_git(repo.path(), &bad).is_err(),
                "must reject scope-escaping argv {bad:?}"
            );
        }
    }
}
