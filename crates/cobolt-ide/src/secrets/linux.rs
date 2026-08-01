// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Linux Secret Service backend — shells out to the `secret-tool` CLI
//! (`libsecret-tools`), zero Rust crate dependencies. `secret-tool` talks to
//! whichever Secret Service D-Bus provider is running — GNOME Keyring
//! (default on stock Ubuntu/GNOME) or KWallet (KDE); the caller neither
//! knows nor cares which.
//!
//! Unlike macOS/Windows, this is **not guaranteed present**: there is no
//! OS-level equivalent to Keychain/Credential Manager on Linux. A minimal
//! install, a server, a tiling-WM setup, or an SSH session with no D-Bus
//! session bus typically has neither the binary nor a running keyring
//! daemon. Every path below treats that as [`StoreOutcome::Unavailable`],
//! not an error — the caller's plaintext fallback is the correct, expected
//! behaviour there, exactly as it is for every other unsupported target.

use super::StoreOutcome;
use std::io::Write;
use std::process::{Command, Stdio};

fn label(account: &str) -> String {
    format!("{}: {account}", super::SERVICE)
}

/// `true` when `secret-tool` itself could not even be launched — the
/// binary is missing, or (headless/no session) the call otherwise never
/// reaches a real Secret Service. Distinguishes "no native store here at
/// all" from "the store rejected this specific operation."
fn is_unavailable(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::NotFound
}

pub fn store(account: &str, secret: &str) -> StoreOutcome {
    let mut child = match Command::new("secret-tool")
        .args([
            "store",
            "--label",
            &label(account),
            "service",
            super::SERVICE,
            "account",
            account,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) if is_unavailable(&e) => return StoreOutcome::Unavailable,
        Err(e) => return StoreOutcome::Failed(format!("could not launch secret-tool: {e}")),
    };

    // secret-tool store reads the secret from stdin (no trailing newline
    // needed — it reads until EOF).
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(secret.as_bytes()) {
            return StoreOutcome::Failed(format!("could not write secret to secret-tool: {e}"));
        }
    }

    match child.wait_with_output() {
        Ok(out) if out.status.success() => StoreOutcome::Stored,
        Ok(out) => StoreOutcome::Failed(format!(
            "secret-tool store exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => StoreOutcome::Failed(format!("secret-tool store: {e}")),
    }
}

pub fn retrieve(account: &str) -> Option<String> {
    let output = Command::new("secret-tool")
        .args(["lookup", "service", super::SERVICE, "account", account])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?; // missing binary, no D-Bus session, etc. — all read as "not found"

    if !output.status.success() {
        return None; // "no matching secret" is secret-tool's normal not-found signal.
    }
    let text = String::from_utf8(output.stdout).ok()?;
    // secret-tool prints a trailing newline; a stored empty-string secret
    // (never produced by this codebase, but not this function's job to
    // assume) would otherwise round-trip as `"\n"` instead of `""`.
    let trimmed = text.strip_suffix('\n').unwrap_or(&text);
    Some(trimmed.to_string())
}

pub fn delete(account: &str) {
    // Best-effort: nothing further to do whether this succeeds, finds
    // nothing to clear, or the binary/session isn't there at all.
    let _ = Command::new("secret-tool")
        .args(["clear", "service", super::SERVICE, "account", account])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
}
