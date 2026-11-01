// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend's identity programs (WASM-009 R7, D4).
//!
//! A backend with an `[identity]` section carries thirteen **public** programs, one per identity
//! operation, each taking `AUTH-REQUEST` and returning `AUTH-RESULT`, and each doing one thing:
//! handing both to `COBOL::"AUTH-OP"`, which the backend's identity service answers. The provider
//! conversation, the secrets and the code exchange stay in Rust; the mapping, the limits and the
//! log are those of any program.
//!
//! They live in `backend/generated/identity/`, with the copybook `AUTH-GROUPS.cpy` and an
//! `exports.toml` fragment. Every file carries the generated-code banner and is rewritten whole;
//! the developer's `backend/exports.toml` and `backend/src/` are never written. The backend runs
//! these programs only from its `/auth/*` routes, never over the WebSocket.

use std::path::{Path, PathBuf};

use crate::remote_controls::GENERATED_DIR;

/// The folder, under `backend/generated/`, these files are in.
pub const IDENTITY_DIR: &str = "identity";
/// The copybook of the two groups.
pub const COPYBOOK: &str = "AUTH-GROUPS.cpy";
/// The line every generated identity file carries, by which it may be replaced or removed.
pub const GENERATED_MARK: &str = crate::remote_controls::GENERATED_MARK;

/// `(PROGRAM-ID, operation)`, in the order of D4.
pub const PROGRAMS: [(&str, &str); 13] = [
    ("AUTH-SIGN-IN", "SIGN-IN"),
    ("AUTH-SIGN-UP", "SIGN-UP"),
    ("AUTH-CONFIRM", "CONFIRM"),
    ("AUTH-RESEND", "RESEND"),
    ("AUTH-FORGOT", "FORGOT"),
    ("AUTH-RESET", "RESET"),
    ("AUTH-MFA", "MFA"),
    ("AUTH-MFA-SETUP", "MFA-SETUP"),
    ("AUTH-REFRESH", "REFRESH"),
    ("AUTH-SIGN-OUT", "SIGN-OUT"),
    ("AUTH-START-HOSTED", "START-HOSTED"),
    ("AUTH-FINISH-HOSTED", "FINISH-HOSTED"),
    ("AUTH-CAPABILITIES", "CAPABILITIES"),
];

/// `AUTH-REQUEST`: what the page (or the route) sends. Text, sized for the longest value a
/// provider documents (a reCAPTCHA token, a sealed continuation).
pub const REQUEST: &[(&str, u32)] = &[
    ("USERNAME", 254),
    ("PASSWORD", 256),
    ("ONE-TIME-CODE", 64),
    ("SESSION", 4096),
    ("CHOICE", 16),
    ("EMAIL", 254),
    ("PHONE", 32),
    ("GIVEN-NAME", 100),
    ("FAMILY-NAME", 100),
    ("CAPTCHA-TOKEN", 4096),
    ("REFRESH-TOKEN", 4096),
    ("ACCESS-TOKEN", 8192),
    ("REDIRECT-URI", 1024),
    ("RETURN-URI", 1024),
    ("PAGE-STATE", 128),
    ("STATE", 128),
    ("HOSTED", 4096),
];

/// `AUTH-RESULT`: what the identity service answers. `EXPIRES-IN` is the one number.
pub const RESULT: &[(&str, u32)] = &[
    ("OUTCOME", 32),
    ("OUTCOME-TEXT", 200),
    ("ACCESS-TOKEN", 8192),
    ("ID-TOKEN", 8192),
    ("REFRESH-TOKEN", 4096),
    ("USERNAME", 254),
    ("CHALLENGE", 32),
    ("DESTINATION", 64),
    ("CHOICES", 64),
    ("SECRET", 128),
    ("OTPAUTH-URI", 1024),
    ("SESSION", 4096),
    ("URL", 4096),
    ("STATE", 128),
    ("HOSTED", 4096),
    ("RETURN-URI", 1024),
    ("PAGE-STATE", 128),
    ("FLOW", 16),
    ("CAPABILITIES", 512),
    ("SITE-KEY", 128),
];

fn banner(comment: &str) -> String {
    let lines = [
        "───────────────────────────────────────────────────────────",
        " This code was generated automatically by PowerRustCOBOL RAD.",
        "",
        " DO NOT MODIFY IT DIRECTLY: it is regenerated on every Build,",
        " Run and Check of the project, so manual edits are lost. Edit",
        " backend.toml's [identity] section (the project's Identity",
        " settings) instead.",
        "",
        " PowerRustCOBOL and its components are distributed under the",
        " Apache 2.0 License.",
        "───────────────────────────────────────────────────────────",
    ];
    lines.iter().map(|l| format!("{comment}{}{l}\n", if l.is_empty() { "" } else { " " })).collect()
}

/// The two groups, as both the copybook and every program hold them.
pub fn groups() -> String {
    let mut out = String::from("       01 AUTH-REQUEST.\n");
    for (name, len) in REQUEST {
        out.push_str(&format!("          05 {name:<16} PIC X({len}).\n"));
    }
    out.push_str("       01 AUTH-RESULT.\n");
    for (name, len) in RESULT {
        out.push_str(&format!("          05 {name:<16} PIC X({len}).\n"));
        if *name == "REFRESH-TOKEN" {
            out.push_str(&format!("          05 {:<16} PIC 9(9).\n", "EXPIRES-IN"));
        }
    }
    out
}

/// `AUTH-GROUPS.cpy`.
pub fn copybook() -> String {
    format!("{}      *> The request and the result of every AUTH-* program of the backend.\n{}", banner("      *>"), groups())
}

/// One program: the request in, the identity service's answer out, nothing else.
pub fn program(name: &str, op: &str) -> String {
    format!(
        "{}\n       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {name}.\n      *> The identity operation {op}, answered by the backend's identity service.\n       DATA DIVISION.\n       LINKAGE SECTION.\n{}\n       PROCEDURE DIVISION USING AUTH-REQUEST RETURNING AUTH-RESULT.\n           COBOL::\"AUTH-OP\" ( \"{op}\" AUTH-REQUEST AUTH-RESULT )\n           GOBACK.\n",
        banner("      *>"),
        groups()
    )
}

/// The `exports.toml` fragment: public, because a person who is signing in has no token yet;
/// `identity`, so the backend serves them from its `/auth/*` routes only; the request read only, the
/// answer in the `RETURNING` group.
pub fn exports_toml() -> String {
    let mut out = banner("#");
    for (name, _) in PROGRAMS {
        out.push_str(&format!("\n[[export]]\nprogram = \"{name}\"\npublic = true\nidentity = true\ndirections = [\"in\"]\n"));
    }
    out
}

/// Every file of `backend/generated/identity/`, by name, in a fixed order.
pub fn files() -> Vec<(String, String)> {
    let mut out = vec![(COPYBOOK.to_string(), copybook()), ("exports.toml".to_string(), exports_toml())];
    for (name, op) in PROGRAMS {
        out.push((format!("{}.cbl", name.to_ascii_lowercase()), program(name, op)));
    }
    out
}

/// Is `text` a file this generator wrote (and may therefore replace or remove)?
pub fn is_generated(text: &str) -> bool {
    text.lines().take(4).any(|l| l.contains(GENERATED_MARK))
}

/// `backend/generated/identity/`.
pub fn dir_of(backend_dir: &Path) -> PathBuf {
    backend_dir.join(GENERATED_DIR).join(IDENTITY_DIR)
}

/// Write the identity files of a backend with an `[identity]` section (`Some`) or remove them
/// (`None`, the section is gone). Only `backend/generated/identity/` is touched, and a file there
/// without the generated mark is never overwritten or removed: it is reported instead. Answers the
/// files written or removed; a file whose text is unchanged is not rewritten.
pub fn sync(backend_dir: &Path, has_identity: bool) -> Result<Vec<PathBuf>, String> {
    let dir = dir_of(backend_dir);
    let mut changed = Vec::new();
    let foreign = |p: &Path| -> Result<bool, String> {
        match std::fs::read_to_string(p) {
            Ok(t) => Ok(!is_generated(&t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("{}: {e}", p.display())),
        }
    };
    if !has_identity {
        if !dir.is_dir() {
            return Ok(changed);
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            if p.is_file() && !foreign(&p)? {
                std::fs::remove_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                changed.push(p);
            }
        }
        // Gone only when nothing of the developer's is left in it.
        let _ = std::fs::remove_dir(&dir);
        return Ok(changed);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let wanted = files();
    for (name, text) in &wanted {
        let p = dir.join(name);
        if foreign(&p)? {
            return Err(format!("{} is not a generated file (it has no \"{GENERATED_MARK}\" line): move it out of {}, then build again", p.display(), dir.display()));
        }
        if std::fs::read_to_string(&p).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        changed.push(p);
    }
    // A generated file this version no longer writes.
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if p.is_file() && !wanted.iter().any(|(n, _)| *n == name) && !foreign(&p)? {
            std::fs::remove_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            changed.push(p);
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};

    #[test]
    fn every_program_parses_and_checks_clean() {
        for (name, op) in PROGRAMS {
            let src = program(name, op);
            let parsed = cobolt_parser::parse(tokenize(&src, SourceFormat::Free));
            assert!(parsed.diagnostics.iter().all(|d| !d.is_error()), "{name}: {:?}", parsed.diagnostics);
            let program = parsed.program.expect("a program");
            assert_eq!(program.identification.program_id, name);
            let semantic = cobolt_semantic::analyze(&program);
            let errors: Vec<_> = semantic.diagnostics.iter().filter(|d| matches!(d.severity, cobolt_semantic::Severity::Error)).collect();
            assert!(errors.is_empty(), "{name}: {errors:?}");
        }
    }

    #[test]
    fn the_output_is_deterministic_and_every_file_carries_the_banner() {
        let a = files();
        assert_eq!(a, files());
        assert_eq!(a.len(), 2 + PROGRAMS.len());
        for (name, text) in &a {
            assert!(is_generated(text), "{name} has no generated mark in its first lines");
        }
        let exports = exports_toml();
        assert_eq!(exports.matches("[[export]]").count(), 13);
        assert_eq!(exports.matches("public = true").count(), 13);
        assert_eq!(exports.matches("identity = true").count(), 13);
    }

    #[test]
    fn sync_writes_only_its_folder_and_never_a_developer_file() {
        let root = std::env::temp_dir().join(format!("prc-identity-gen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("exports.toml"), "# mine\n").unwrap();
        std::fs::write(root.join("src/MINE.cbl"), "mine\n").unwrap();

        let written = sync(&root, true).unwrap();
        assert_eq!(written.len(), 15);
        assert!(written.iter().all(|p| p.starts_with(dir_of(&root))));
        assert!(sync(&root, true).unwrap().is_empty(), "an unchanged file is not rewritten");

        // A file of the developer's in the folder is refused, not replaced.
        std::fs::write(dir_of(&root).join("auth-mfa.cbl"), "my own\n").unwrap();
        assert!(sync(&root, true).unwrap_err().contains("not a generated file"));
        assert_eq!(std::fs::read_to_string(dir_of(&root).join("auth-mfa.cbl")).unwrap(), "my own\n");

        // The section removed: the generated files go, the developer's stays.
        sync(&root, false).unwrap();
        let left: Vec<_> = std::fs::read_dir(dir_of(&root)).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from("auth-mfa.cbl")]);
        assert_eq!(std::fs::read_to_string(root.join("exports.toml")).unwrap(), "# mine\n");
        assert_eq!(std::fs::read_to_string(root.join("src/MINE.cbl")).unwrap(), "mine\n");
        let _ = std::fs::remove_dir_all(&root);
    }
}
