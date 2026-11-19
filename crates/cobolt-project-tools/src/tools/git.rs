// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `git` — version control of the project through MCP (spec 084 R64), so the
//! standing rule that a project lives in a repository never sends Claude Code
//! to a shell. A closed set of operations, each mapped to one `git` command run
//! in the project folder, with prompts off and a time limit. The developer's
//! own git configuration and credentials are used; an answer never shows a
//! credential.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cobolt_mcp::Tool;
use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;

/// Longest a network operation (push, pull, fetch) may take.
const NETWORK_LIMIT: Duration = Duration::from_secs(180);
/// Longest any other operation may take.
const LOCAL_LIMIT: Duration = Duration::from_secs(60);
/// Most bytes of output answered.
const OUTPUT_LIMIT: usize = 200_000;

pub fn tools() -> Vec<Tool> {
    vec![Tool {
        name: "git".into(),
        description: Some(
            "Version control of the project with git, run in the project folder with the developer's own git \
             settings and credentials. `command`: init, status, diff {path?, staged?}, log {limit?}, add {paths}, \
             commit {message}, branch {name?} (lists, or creates), switch {name, create?}, remote {name?, url?} \
             (lists, or adds), fetch, pull (fast-forward only), push {remote?, branch?, set_upstream?}. Answers git's \
             output. init, add, commit, branch, switch, remote, pull and push change the repository; push and \
             remote reach the network."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "enum": ["init", "status", "diff", "log", "add", "commit", "branch", "switch", "remote", "fetch", "pull", "push"] },
                "path": { "type": "string" },
                "staged": { "type": "boolean" },
                "limit": { "type": "integer" },
                "paths": { "type": "array", "items": { "type": "string" } },
                "message": { "type": "string" },
                "name": { "type": "string" },
                "create": { "type": "boolean" },
                "url": { "type": "string" },
                "remote": { "type": "string" },
                "branch": { "type": "string" },
                "set_upstream": { "type": "boolean" }
            },
            "required": ["command"],
            "additionalProperties": false
        }),
        annotations: None,
    }]
}

fn s(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

/// A git ref or remote name: letters, digits and `._/-`, never an option.
fn name_ok(n: &str) -> bool {
    !n.starts_with('-') && !n.contains("..") && n.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
}

/// `https://user:secret@host/…` → `https://***@host/…`, on every line.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail.find(|c: char| c.is_whitespace() || c == '/' || c == '\'' || c == '"').unwrap_or(tail.len());
        match tail[..end].rfind('@') {
            Some(at) => {
                out.push_str("***");
                out.push_str(&tail[at..end]);
            }
            None => out.push_str(&tail[..end]),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn git(dir: &Path, argv: &[String], limit: Duration) -> Result<(bool, String), String> {
    let mut child = Command::new("git")
        .args(argv)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git cannot be started ({e}); is git installed and on the PATH?"))?;
    // Read both pipes on their own threads, so a long output never blocks the child.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = std::io::Read::read_to_end(p, &mut b);
        }
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = std::io::Read::read_to_end(p, &mut b);
        }
        b
    });
    let start = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("git {} took longer than {} s and was stopped", argv.first().map_or("", String::as_str), limit.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut text = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned();
    let err = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
    if !err.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if text.len() > OUTPUT_LIMIT {
        let mut cut = OUTPUT_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n… (output cut)");
    }
    Ok((status.success(), redact(&text)))
}

pub fn run<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let command = s(args, "command").ok_or("git needs a 'command'")?;
    let mut limit = LOCAL_LIMIT;
    let argv: Vec<String> = match command.as_str() {
        "init" => vec!["init".into()],
        "status" => vec!["status".into(), "--short".into(), "--branch".into()],
        "diff" => {
            let mut v = vec!["diff".into(), "--no-color".into()];
            if args.get("staged").and_then(Value::as_bool).unwrap_or(false) {
                v.push("--cached".into());
            }
            if let Some(p) = s(args, "path") {
                root.resolve(&p)?;
                v.push("--".into());
                v.push(p);
            }
            v
        }
        "log" => {
            let n = args.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 500);
            vec!["log".into(), "--no-color".into(), "--oneline".into(), "--decorate".into(), "-n".into(), n.to_string()]
        }
        "add" => {
            let paths: Vec<String> = args
                .get("paths")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                .unwrap_or_default();
            if paths.is_empty() {
                return Err("git add needs 'paths' (project-relative; \".\" for the whole project)".to_owned());
            }
            let mut v = vec!["add".into(), "--".into()];
            for p in paths {
                if p.trim() != "." {
                    root.resolve(&p)?;
                }
                v.push(p);
            }
            v
        }
        "commit" => {
            let message = s(args, "message").ok_or("git commit needs a 'message'")?;
            vec!["commit".into(), "-m".into(), message]
        }
        "branch" => match s(args, "name") {
            Some(n) if name_ok(&n) => vec!["branch".into(), n],
            Some(n) => return Err(format!("'{n}' is not a branch name")),
            None => vec!["branch".into(), "--list".into(), "--no-color".into()],
        },
        "switch" => {
            let n = s(args, "name").ok_or("git switch needs a 'name'")?;
            if !name_ok(&n) {
                return Err(format!("'{n}' is not a branch name"));
            }
            let mut v = vec!["switch".into()];
            if args.get("create").and_then(Value::as_bool).unwrap_or(false) {
                v.push("-c".into());
            }
            v.push(n);
            v
        }
        "remote" => match (s(args, "name"), s(args, "url")) {
            (Some(n), Some(u)) if name_ok(&n) && !u.starts_with('-') => vec!["remote".into(), "add".into(), n, u],
            (Some(_), Some(_)) => return Err("git remote: that name or url is not accepted".to_owned()),
            _ => vec!["remote".into(), "--verbose".into()],
        },
        "fetch" => {
            limit = NETWORK_LIMIT;
            vec!["fetch".into(), "--prune".into()]
        }
        "pull" => {
            limit = NETWORK_LIMIT;
            vec!["pull".into(), "--ff-only".into()]
        }
        "push" => {
            limit = NETWORK_LIMIT;
            let mut v = vec!["push".into()];
            if args.get("set_upstream").and_then(Value::as_bool).unwrap_or(false) {
                v.push("--set-upstream".into());
            }
            for key in ["remote", "branch"] {
                if let Some(x) = s(args, key) {
                    if !name_ok(&x) {
                        return Err(format!("'{x}' is not a {key} name"));
                    }
                    v.push(x);
                }
            }
            v
        }
        other => return Err(format!("git: '{other}' is not one of the operations this tool runs")),
    };
    let (ok, output) = git(root.dir(), &argv, limit)?;
    if matches!(command.as_str(), "pull" | "switch") && ok {
        // Files changed under the IDE: it reloads what it shows.
        host.written(&[]);
    }
    if !ok {
        return Err(format!("git {command} failed:\n{}", output.trim_end()));
    }
    Ok(json!({ "command": command, "output": output.trim_end() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_never_leave_in_an_answer() {
        assert_eq!(redact("remote: https://user:ghp_secret@github.com/o/r.git (push)"), "remote: https://***@github.com/o/r.git (push)");
        assert_eq!(redact("https://github.com/o/r.git"), "https://github.com/o/r.git");
        assert!(name_ok("feature/x-1") && !name_ok("--force") && !name_ok("a..b") && !name_ok("a b"));
    }

    /// AC29 (git): init, add, commit and log through the tool, in a temporary project.
    #[test]
    fn a_commit_through_the_tool_appears_in_the_log() {
        if Command::new("git").arg("--version").output().is_err() {
            println!("git: not installed here; skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("prc-084d-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Demo.project.toml"), "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\n").unwrap();
        std::fs::write(dir.join("src/A.cbl"), "      * a\n").unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let mut host = crate::host::HeadlessHost::new(dir.join("Demo.project.toml"), "t");
        run(&mut host, &root, &json!({"command": "init"})).unwrap();
        // A commit needs an identity; the test's own, local to this repository.
        let _ = Command::new("git").args(["config", "user.email", "t@example.invalid"]).current_dir(&dir).output();
        let _ = Command::new("git").args(["config", "user.name", "Test"]).current_dir(&dir).output();
        run(&mut host, &root, &json!({"command": "add", "paths": ["."]})).unwrap();
        run(&mut host, &root, &json!({"command": "commit", "message": "First form"})).unwrap();
        let log = run(&mut host, &root, &json!({"command": "log"})).unwrap();
        assert!(log["output"].as_str().unwrap().contains("First form"), "{log}");
        assert!(run(&mut host, &root, &json!({"command": "add", "paths": ["../x"]})).is_err(), "outside the project");
        assert!(run(&mut host, &root, &json!({"command": "rebase"})).is_err(), "not an offered operation");
        let _ = std::fs::remove_dir_all(&dir);
        println!("git: init, add, commit, log through the tool; '../x' and 'rebase' refused");
    }
}
