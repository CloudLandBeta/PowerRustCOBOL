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
//!   session token from the application's own environment or from a route.
//!   The credential chain is the AWS CLI's: a named profile, SSO, `aws login`.
//!   **Amendment A6 (spec 092)** adds one exception: the keys the program set
//!   for the connection with `COBOL::"AWS-SET-CREDENTIALS"` ([`Credentials`]).
//!   When a launch carries them, the server receives exactly
//!   `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and (when one was given)
//!   `AWS_SESSION_TOKEN` from them, and no profile at all. [`built_env`] is
//!   the only code that reads them; they are never an argument.
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
    /// The keys the program set for the connection (spec 092 A6). When
    /// present they sign the server's calls and the profile is left out; see
    /// [`Launch::with_credentials`]. A different set is a different launch,
    /// so the pool restarts the server.
    pub credentials: Option<Arc<Credentials>>,
    /// Spec 103: where an `https` server is reached. When present there is no
    /// program to start — `command`, `args` and `env` are empty — and the
    /// requests are signed by [`super::https`].
    pub https: Option<HttpsEndpoint>,
}

/// Where an `https` server is reached and what its requests are signed for
/// (spec 103), out of the route table with its placeholders expanded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HttpsEndpoint {
    pub url: String,
    /// The SigV4 service name.
    pub service: String,
    pub signing_region: String,
    /// Sent in the `_meta` of every request (today `AWS_REGION`).
    pub metadata: Vec<(String, String)>,
}

impl Launch {
    /// This launch signing with `credentials` instead of a profile (A6): no
    /// profile, and no `--profile` argument a route override may have added —
    /// an explicit profile makes the AWS SDK ignore keys in the environment.
    pub fn with_credentials(mut self, credentials: Arc<Credentials>) -> Self {
        let mut args = Vec::with_capacity(self.args.len());
        let mut skip_value = false;
        for a in self.args {
            if skip_value {
                skip_value = false;
            } else if a == "--profile" {
                skip_value = true;
            } else if !a.starts_with("--profile=") {
                args.push(a);
            }
        }
        self.args = args;
        self.profile = None;
        self.credentials = Some(credentials);
        self
    }
}

/// AWS keys a program set for one connection at run time (spec 092 A6).
///
/// Nothing can read them back. The fields are private to this module, which
/// has two readers: [`built_env`], for a server process that signs for itself,
/// and [`Credentials::with_keys`], for the runtime's own signer (spec 103) —
/// which lends them for one call and keeps nothing. `Debug` and `Display` print
/// a placeholder; and the text is overwritten when the last holder lets it go.
/// Two sets are equal only when they are the same set — every
/// `AWS-SET-CREDENTIALS` makes a new one — so a server started with other
/// keys never matches the launch and is restarted.
pub struct Credentials {
    serial: u64,
    key_id: Secret,
    secret: Secret,
    /// Empty when no session token was given.
    token: Secret,
}

impl Credentials {
    /// Take an access key id, a secret access key and a session token (empty
    /// for none), surrounding spaces removed. The texts handed in are
    /// overwritten.
    pub fn new(key_id: String, secret: String, token: String) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(1);
        Credentials {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            key_id: Secret::take_trimmed(key_id),
            secret: Secret::take_trimmed(secret),
            token: Secret::take_trimmed(token),
        }
    }

    /// Lend the keys to `f` for the length of the call (spec 103): the one way
    /// the runtime's own signer reads them. What `f` is given has no `Debug`,
    /// `Clone` or `Copy`, so it cannot be formatted, stored or passed on by
    /// accident; `f` must not keep what it borrows.
    pub(crate) fn with_keys<R>(&self, f: impl FnOnce(KeyRef<'_>) -> R) -> R {
        let token = (!self.token.0.is_empty()).then_some(self.token.0.as_str());
        f(KeyRef { key_id: &self.key_id.0, secret: &self.secret.0, token })
    }
}

/// The keys of one [`Credentials`], borrowed for the length of a signing call.
/// Deliberately without `Debug`, `Clone` or `Copy`.
pub(crate) struct KeyRef<'a> {
    pub(crate) key_id: &'a str,
    pub(crate) secret: &'a str,
    pub(crate) token: Option<&'a str>,
}

impl PartialEq for Credentials {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials(redacted)")
    }
}

impl std::fmt::Display for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(AWS credentials, redacted)")
    }
}

/// One held value: no `Debug`, no `Clone`, wiped when dropped.
struct Secret(String);

impl Secret {
    fn take_trimmed(mut text: String) -> Self {
        let kept = Secret(text.trim().to_owned());
        wipe(&mut text);
        kept
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Overwrite a string's bytes with zeros before its memory is released, in a
/// way the optimiser may not drop as a dead store, then empty it.
pub(crate) fn wipe(text: &mut String) {
    // SAFETY: zero bytes are valid UTF-8, so the string stays valid while it
    // is overwritten, and it is emptied straight after.
    let bytes = unsafe { text.as_mut_vec() };
    for b in bytes.iter_mut() {
        // SAFETY: `b` is a valid, aligned and exclusive reference.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    text.clear();
}

/// The variables a server may never receive from the application's
/// environment or a route. (The keys a program set, A6, are the one way in,
/// and [`built_env`] adds them itself.)
pub const SECRET_VARS: [&str; 4] =
    ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "AWS_SECURITY_TOKEN"];

/// The variables that name a profile — left out when keys sign instead (A6):
/// a profile the machine does not have is an error to the AWS SDK.
const PROFILE_VARS: [&str; 2] = ["AWS_PROFILE", "AWS_DEFAULT_PROFILE"];

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

fn is_profile_var(name: &str) -> bool {
    PROFILE_VARS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// The environment a server is started with, from the parent's (R11) — and,
/// when the launch carries keys the program set (A6), those keys instead of a
/// profile. The only reader of [`Credentials`].
fn built_env(launch: &Launch, parent: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let keys = launch.credentials.as_deref();
    let mut env: Vec<(String, String)> = parent
        .into_iter()
        .filter(|(k, _)| INHERITED_VARS.iter().any(|v| v.eq_ignore_ascii_case(k)))
        .collect();
    let mut set = |k: &str, v: &str| {
        env.retain(|(name, _)| !name.eq_ignore_ascii_case(k));
        env.push((k.to_owned(), v.to_owned()));
    };
    if let Some(p) = launch.profile.as_deref().filter(|p| keys.is_none() && !p.trim().is_empty()) {
        set("AWS_PROFILE", p.trim());
    }
    if let Some(r) = launch.region.as_deref().filter(|r| !r.trim().is_empty()) {
        set("AWS_REGION", r.trim());
    }
    for (k, v) in &launch.env {
        if !is_secret(k) && !(keys.is_some() && is_profile_var(k)) {
            set(k, v);
        }
    }
    env.retain(|(k, _)| !is_secret(k));
    if let Some(c) = keys {
        env.push(("AWS_ACCESS_KEY_ID".to_owned(), c.key_id.0.clone()));
        env.push(("AWS_SECRET_ACCESS_KEY".to_owned(), c.secret.0.clone()));
        if !c.token.0.is_empty() {
            env.push(("AWS_SESSION_TOKEN".to_owned(), c.token.0.clone()));
        }
    }
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
    let mut env = built_env(launch, std::env::vars());
    let mut cmd = Command::new(&launch.command);
    cmd.args(&launch.args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::lifetime::prepare(&mut cmd);
    let started = cmd.spawn();
    // The command keeps its own copy until it is dropped; this one goes now.
    for (_, v) in env.iter_mut() {
        wipe(v);
    }
    let mut child = match started {
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
    // One masker for every diagnostic in the product: the forms crate's, which the
    // diagnostics dump and the live trace use too.
    cobolt_forms::diagnostics::mask_credentials(text)
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

    /// Recognisable values, assembled here: they look like no real AWS key,
    /// so masking cannot hide a leak — a surface must simply never hold them.
    fn planted() -> (String, String, String) {
        let tag = std::process::id().to_string();
        (["prc", "a6", "keyid", &tag].join("-"), ["prc", "a6", "secret", &tag].join("_"), ["prc", "a6", "token", &tag].join("."))
    }

    fn keys(key: &str, secret: &str, token: &str) -> Arc<Credentials> {
        Arc::new(Credentials::new(format!("  {key} "), secret.to_owned(), token.to_owned()))
    }

    /// A6 (R5): keys set for the connection reach its server as exactly the
    /// three variables, and the profile — the launch's, a route's, the
    /// parent's — is left out; the parent's own secrets still never pass.
    #[test]
    fn credentials_replace_the_profile_in_the_built_environment() {
        let (key, secret, token) = planted();
        // Built by hand, profile AND keys: `built_env` must drop the profile
        // itself, not rely on `with_credentials` having done it.
        let launch = Launch {
            command: "uvx".into(),
            profile: Some("dev".into()),
            region: Some("eu-west-1".into()),
            env: vec![("FUNCTION_PREFIX".into(), "app-".into()), ("AWS_DEFAULT_PROFILE".into(), "route-profile".into())],
            credentials: Some(keys(&key, &secret, &token)),
            ..Default::default()
        };
        let mut offered = parent();
        offered.push(("AWS_PROFILE".into(), "parent-profile".into()));
        let env = built_env(&launch, offered);
        let get = |k: &str| env.iter().filter(|(n, _)| n == k).map(|(_, v)| v.as_str()).collect::<Vec<_>>();
        assert_eq!(get("AWS_ACCESS_KEY_ID"), [key.as_str()], "trimmed, once");
        assert_eq!(get("AWS_SECRET_ACCESS_KEY"), [secret.as_str()]);
        assert_eq!(get("AWS_SESSION_TOKEN"), [token.as_str()]);
        assert!(get("AWS_SECURITY_TOKEN").is_empty());
        for p in PROFILE_VARS {
            assert!(get(p).is_empty(), "{p} must be left out when keys sign: {env:?}");
        }
        assert_eq!(get("AWS_REGION"), ["eu-west-1"]);
        assert_eq!(get("FUNCTION_PREFIX"), ["app-"]);
        let all: String = env.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        for (name, value) in parent().into_iter().filter(|(k, _)| is_secret(k) || k == "MY_APP_SECRET") {
            assert!(!all.contains(&value), "the parent's {name} passed");
        }
        println!("A6 built env: 3 key variables, 0 profile variables, {} variables in all", env.len());
    }

    /// A6 (R5): an empty token leaves AWS_SESSION_TOKEN out altogether.
    #[test]
    fn an_empty_token_is_left_out() {
        let (key, secret, _) = planted();
        let launch = Launch::default().with_credentials(keys(&key, &secret, "   "));
        let env = built_env(&launch, Vec::new());
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"]);
    }

    /// A6 (R5): a launch with keys carries no profile and no `--profile`
    /// argument, in either spelling; every other argument stays, in order.
    #[test]
    fn keys_take_the_profile_argument_away() {
        let (key, secret, token) = planted();
        let launch = Launch {
            command: "uvx".into(),
            args: ["proxy@1", "--profile", "dev", "https://x", "--profile=dev", "--metadata", "AWS_REGION=eu-west-1"]
                .map(String::from)
                .to_vec(),
            profile: Some("dev".into()),
            ..Default::default()
        };
        let plain = launch.clone();
        let signed = launch.with_credentials(keys(&key, &secret, &token));
        assert_eq!(signed.args, ["proxy@1", "https://x", "--metadata", "AWS_REGION=eu-west-1"]);
        assert_eq!(signed.profile, None);
        assert!(!signed.args.iter().any(|a| a.contains(&key) || a.contains(&secret) || a.contains(&token)));
        assert_ne!(plain, signed, "keys make a different launch");
    }

    /// A6 (R7): the type that holds the keys, and everything holding it,
    /// renders without them; two sets are equal only when they are one set.
    #[test]
    fn credentials_redact_themselves() {
        let (key, secret, token) = planted();
        let c = keys(&key, &secret, &token);
        let launch = Launch { command: "uvx".into(), ..Default::default() }.with_credentials(c.clone());
        let renderings = [
            format!("{c:?}"),
            format!("{c:#?}"),
            format!("{c}"),
            format!("{launch:?}"),
            format!("{launch:#?}"),
        ];
        for r in &renderings {
            for v in [&key, &secret, &token] {
                assert!(!r.contains(v.as_str()), "{r}");
            }
        }
        assert_eq!(renderings[0], "Credentials(redacted)");
        assert_eq!(*c, *c.clone(), "one set equals itself");
        let again = keys(&key, &secret, &token);
        assert_ne!(*c, *again, "the same values set again are a new set");
        let same = Launch { command: "uvx".into(), ..Default::default() }.with_credentials(c);
        assert_eq!(launch, same);
    }

    /// The buffers are overwritten, not merely released.
    #[test]
    fn wipe_overwrites_the_bytes() {
        let mut s = String::from("prc-a6-wipe");
        let ptr = s.as_ptr();
        let len = s.len();
        wipe(&mut s);
        assert!(s.is_empty());
        // SAFETY: the string still owns its allocation; only its length changed.
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert!(bytes.iter().all(|b| *b == 0), "{bytes:?}");
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
