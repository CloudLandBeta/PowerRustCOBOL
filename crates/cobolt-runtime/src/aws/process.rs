// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Starting an AWS MCP server as a child process (spec 078 R7, R10, R11).
//!
//! Three rules shape this file:
//!
//! - **The environment is built, never inherited wholesale** (R11). A server
//!   gets the few variables a program needs to run and find its AWS
//!   configuration, the profile and region the connection names, and the
//!   variables its route declares — and never an access key, a secret key or a
//!   session token, even when the application's own environment holds them.
//!   The credential chain is the AWS CLI's: a named profile, SSO, `aws login`.
//! - **stderr never reaches the protocol stream or an end user raw** (R10). A
//!   thread drains it into a bounded ring the runtime may consult — to
//!   recognise a credential failure, say — and `Verbose` shows it masked.
//! - **A missing program is named, with its fix** (R22): "No such file or
//!   directory" about `uvx` reads as if something in the APPLICATION were
//!   missing, which it is not.

use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

/// How to start one server.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
    /// The route's own variables (`FUNCTION_PREFIX`, …).
    pub env: Vec<(String, String)>,
    /// The AWS CLI profile the connection names — a NAME, never a key.
    pub profile: Option<String>,
    pub region: Option<String>,
}

/// The variables a server may never receive, whoever offers them.
pub const SECRET_VARS: [&str; 4] =
    ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "AWS_SECURITY_TOKEN"];

/// The parent variables a server inherits: enough to run a program, find its
/// caches and read the AWS configuration files — nothing else.
const INHERITED_VARS: [&str; 16] = [
    "PATH",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "SystemRoot",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "UV_CACHE_DIR",
    "UV_TOOL_DIR",
    "AWS_CONFIG_FILE",
    "AWS_SHARED_CREDENTIALS_FILE",
];

fn is_secret(name: &str) -> bool {
    SECRET_VARS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// The environment a server is started with, from the parent's (R11).
pub fn built_env(launch: &Launch, parent: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = parent
        .into_iter()
        .filter(|(k, _)| INHERITED_VARS.iter().any(|v| v.eq_ignore_ascii_case(k)))
        .collect();
    let mut set = |k: &str, v: &str| {
        env.retain(|(name, _)| !name.eq_ignore_ascii_case(k));
        env.push((k.to_owned(), v.to_owned()));
    };
    if let Some(p) = launch.profile.as_deref().filter(|p| !p.trim().is_empty()) {
        set("AWS_PROFILE", p.trim());
    }
    if let Some(r) = launch.region.as_deref().filter(|r| !r.trim().is_empty()) {
        set("AWS_REGION", r.trim());
    }
    for (k, v) in &launch.env {
        if !is_secret(k) {
            set(k, v);
        }
    }
    env.retain(|(k, _)| !is_secret(k));
    env
}

/// The last bytes a server wrote to stderr, bounded (R10).
#[derive(Debug, Clone, Default)]
pub struct StderrRing(Arc<Mutex<VecDeque<u8>>>);

impl StderrRing {
    pub const CAPACITY: usize = 64 * 1024;

    fn push(&self, bytes: &[u8]) {
        let mut ring = self.0.lock().unwrap_or_else(|e| e.into_inner());
        ring.extend(bytes);
        let excess = ring.len().saturating_sub(Self::CAPACITY);
        ring.drain(..excess);
    }

    /// What is held, as text — raw: [`mask`] it before showing anyone.
    pub fn text(&self) -> String {
        let ring = self.0.lock().unwrap_or_else(|e| e.into_inner());
        String::from_utf8_lossy(&ring.iter().copied().collect::<Vec<u8>>()).into_owned()
    }
}

/// A started server.
pub struct Spawned {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: StderrRing,
}

/// The message an end user sees when the program a server needs is missing
/// (R22): what is missing, and how to install it.
pub fn missing_program_message(program: &str) -> String {
    let base = std::path::Path::new(program)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| program.to_owned());
    if base.eq_ignore_ascii_case("uvx") || base.eq_ignore_ascii_case("uvx.exe") || base.eq_ignore_ascii_case("uv") {
        "AWS access needs the program \"uvx\", which is not installed on this machine. \
         Install uv (https://docs.astral.sh/uv/) — for example \"brew install uv\", \
         \"pip install uv\" or \"winget install astral-sh.uv\" — then try again."
            .to_owned()
    } else {
        format!(
            "AWS access needs the program \"{base}\", which is not installed on this machine \
             (or is not on the PATH). Install it, then try again."
        )
    }
}

/// Start a server (R7): stdin and stdout piped for the protocol, stderr
/// drained into a ring by a thread of its own.
pub fn spawn(launch: &Launch) -> Result<Spawned, String> {
    let mut cmd = Command::new(&launch.command);
    cmd.args(&launch.args)
        .env_clear()
        .envs(built_env(launch, std::env::vars()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::lifetime::prepare(&mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(missing_program_message(&launch.command));
        }
        Err(e) => return Err(format!("the AWS service \"{}\" could not be started: {e}", launch.command)),
    };
    super::lifetime::adopt(&child);
    let stdin = child.stdin.take().ok_or("the AWS service has no input stream")?;
    let stdout = child.stdout.take().ok_or("the AWS service has no output stream")?;
    let ring = StderrRing::default();
    if let Some(mut err) = child.stderr.take() {
        let sink = ring.clone();
        std::thread::Builder::new()
            .name("aws-mcp-stderr".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = err.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    sink.push(&buf[..n]);
                }
            })
            .ok();
    }
    Ok(Spawned { child, stdin, stdout, stderr: ring })
}

/// Mask anything that looks like a credential before it is shown (R27):
/// access key ids (`AKIA…`, `ASIA…`), secret keys and session tokens (long
/// mixed-case base64 runs), and the signature, token and credential of a
/// pre-signed URL.
pub fn mask(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let is_b64 = |c: char| c.is_ascii_alphanumeric() || c == '/' || c == '+' || c == '=';
    let mut i = 0;
    while i < chars.len() {
        // A pre-signed URL's secret-bearing query values.
        let rest: String = chars[i..chars.len().min(i + 24)].iter().collect();
        if let Some(key) = ["X-Amz-Signature=", "X-Amz-Security-Token=", "X-Amz-Credential="]
            .iter()
            .find(|k| rest.starts_with(**k))
        {
            out.push_str(key);
            i += key.chars().count();
            while i < chars.len() && !matches!(chars[i], '&' | ' ' | '"' | '\'' | '\n' | '\r' | '\t') {
                i += 1;
            }
            out.push_str("****");
            continue;
        }
        if is_b64(chars[i]) {
            let start = i;
            while i < chars.len() && is_b64(chars[i]) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            let key_id = run.len() == 20
                && (run.starts_with("AKIA") || run.starts_with("ASIA"))
                && run.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            let secret = run.len() >= 40
                && run.chars().any(|c| c.is_ascii_uppercase())
                && run.chars().any(|c| c.is_ascii_lowercase())
                && run.chars().any(|c| c.is_ascii_digit());
            if key_id {
                out.push_str(&run[..4]);
                out.push_str("****************");
            } else if secret {
                out.push_str("****");
            } else {
                out.push_str(&run);
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> Vec<(String, String)> {
        vec![
            ("PATH".into(), "/usr/bin".into()),
            ("HOME".into(), "/home/ana".into()),
            ("AWS_ACCESS_KEY_ID".into(), "AKIAABCDEFGHIJKLMNOP".into()),
            ("AWS_SECRET_ACCESS_KEY".into(), "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into()),
            ("AWS_SESSION_TOKEN".into(), "FwoGZXIvYXdzEJr//////////wEaDEXAMPLE".into()),
            ("MY_APP_SECRET".into(), "hunter2".into()),
            ("AWS_CONFIG_FILE".into(), "/home/ana/.aws/config".into()),
        ]
    }

    /// R11: only the allow-list, the profile, the region and the route's own
    /// variables — never a key, a secret or a token, whoever offers them.
    #[test]
    fn the_built_environment_never_carries_aws_secrets() {
        let launch = Launch {
            command: "uvx".into(),
            profile: Some("dev".into()),
            region: Some("eu-west-1".into()),
            env: vec![("FUNCTION_PREFIX".into(), "app-".into()), ("AWS_SECRET_ACCESS_KEY".into(), "planted".into())],
            ..Default::default()
        };
        let env = built_env(&launch, parent());
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        for secret in SECRET_VARS {
            assert!(!names.contains(&secret), "{secret} leaked: {names:?}");
        }
        assert!(!names.contains(&"MY_APP_SECRET"), "an unlisted variable leaked");
        let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("AWS_PROFILE"), Some("dev"));
        assert_eq!(get("AWS_REGION"), Some("eu-west-1"));
        assert_eq!(get("FUNCTION_PREFIX"), Some("app-"));
        assert_eq!(get("PATH"), Some("/usr/bin"));
        assert_eq!(get("AWS_CONFIG_FILE"), Some("/home/ana/.aws/config"));
        println!("built env: {} variables passed of {} offered; 0 of 4 secret names", env.len(), parent().len() + 4);
    }

    /// R27: a planted key id, secret, session token and signature are masked.
    #[test]
    fn mask_hides_planted_credentials() {
        let text = "key AKIAABCDEFGHIJKLMNOP secret wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY \
                    url https://b.s3.amazonaws.com/k?X-Amz-Credential=AKIA%2F&X-Amz-Signature=abc123def&x=1 \
                    plain words stay, id 12345";
        let m = mask(text);
        for planted in ["ABCDEFGHIJKLMNOP", "wJalrXUtnFEMI", "abc123def", "AKIA%2F"] {
            assert!(!m.contains(planted), "{planted} survived: {m}");
        }
        assert!(m.contains("AKIA****************"));
        assert!(m.contains("X-Amz-Signature=****&x=1"));
        assert!(m.contains("plain words stay, id 12345"));
    }

    #[test]
    fn a_missing_uvx_names_the_program_and_the_fix() {
        let e = missing_program_message("uvx");
        assert!(e.contains("\"uvx\"") && e.contains("Install uv"), "{e}");
        let r = spawn(&Launch { command: "prc-no-such-program-078".into(), ..Default::default() });
        let e = r.err().expect("a missing program fails");
        assert!(e.contains("prc-no-such-program-078") && e.contains("not installed"), "{e}");
    }

    #[test]
    fn the_ring_keeps_only_the_last_64_kib() {
        let ring = StderrRing::default();
        ring.push(&vec![b'a'; StderrRing::CAPACITY]);
        ring.push(b"tail");
        let t = ring.text();
        assert_eq!(t.len(), StderrRing::CAPACITY);
        assert!(t.ends_with("tail"));
    }
}
