// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Keeping the model configuration — **including its API keys** — where the
//! developer chooses.
//!
//! # Why this exists
//!
//! An API key used to live for exactly one run. `LlmConfig::api_keys` is
//! `#[serde(skip)]` and the OS-native store is behind a kill switch until it has
//! a management UI, so the machine config carried no credential at all and every
//! session asked for the key again. That was a deliberate trade — "losing keys on
//! restart is the accepted cost; leaking one is not recoverable" — but it made
//! the AI features tedious to actually use.
//!
//! This is the other half of that trade: the developer says **where** the
//! configuration is kept, is told what it costs, and gets one guard rail that is
//! not negotiable.
//!
//! # The guard rail
//!
//! A chosen file may **never** sit inside a git working tree. That is the
//! accident this whole area exists to prevent — a key committed and pushed is
//! not recoverable, and "one careless `git add`" is exactly how it happens. The
//! check walks the path's ancestors looking for a `.git` entry and refuses by
//! naming the repository it found, so the refusal is actionable rather than
//! mysterious. `/tmp/llm_config.json` is offered first because it is outside
//! every repository by construction.
//!
//! # The OS vault
//!
//! [`Vault::OsVault`] is the right answer and it is **not available yet** — it
//! ships in **RC3**, once it has a UI that can inspect, rotate and clear what it
//! holds (see [`crate::secrets`]: writing a secret a developer can only remove by
//! hunting through Keychain is worse than not writing it). The choice is offered
//! and refused, rather than hidden, so nobody has to guess whether it is coming.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::llm::LlmConfig;

/// Whether the OS-native credential store may be chosen yet. `false` until the
/// secrets-management UI ships — see the module docs.
pub const OS_VAULT_AVAILABLE: bool = false;

/// The release that will offer [`Vault::OsVault`] (operator, 2026-08-17).
pub const OS_VAULT_SHIPS_IN: &str = "RC3";

/// The format version written into a config file, so a future reader can tell
/// what it is looking at instead of guessing.
pub const FILE_VERSION: u32 = 1;

/// Where the model configuration and its API keys are kept.
///
/// Serialised through [`Vault::parse`] rather than by derive, for two reasons: the
/// stored spelling is then exactly [`Vault::as_str`] (`"local-file"`, readable in
/// the config file) instead of serde's variant name, and an unrecognised value
/// degrades to [`Vault::Session`] instead of failing the whole `LlmConfig` parse —
/// a config written by a newer build must not cost the developer every other
/// setting in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Vault {
    /// Nowhere. Keys live in this process and are asked for again next run —
    /// the behaviour before any of this, and still the default: persisting a
    /// credential is the developer's decision to make, not ours to assume.
    #[default]
    Session,
    /// A JSON file the developer names, holding the whole model configuration
    /// and its keys. Never inside a git working tree.
    LocalFile,
    /// The platform's own credential store. Ships in [`OS_VAULT_SHIPS_IN`].
    OsVault,
}

impl Vault {
    /// The stored/CLI spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::LocalFile => "local-file",
            Self::OsVault => "os-vault",
        }
    }

    /// Lenient parse; anything unrecognised is [`Vault::Session`], so a config
    /// written by a newer build opens here with keys simply not persisted rather
    /// than refusing to load.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "local-file" | "localfile" | "file" => Self::LocalFile,
            "os-vault" | "osvault" | "vault" | "keychain" => Self::OsVault,
            _ => Self::Session,
        }
    }

    /// Whether this option can be chosen on this build.
    pub fn available(self) -> bool {
        match self {
            Self::Session | Self::LocalFile => true,
            Self::OsVault => OS_VAULT_AVAILABLE,
        }
    }

    /// Every option, in the order the UI offers them.
    pub const ALL: &'static [Vault] = &[Vault::Session, Vault::LocalFile, Vault::OsVault];
}

impl From<String> for Vault {
    fn from(raw: String) -> Self {
        Self::parse(&raw)
    }
}

impl From<Vault> for String {
    fn from(vault: Vault) -> Self {
        vault.as_str().to_owned()
    }
}

/// Why a chosen path was refused. Each carries the sentence to put in front of
/// the developer: a refusal that does not say what to do instead is half a
/// refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The path is inside a git working tree, and the repository root that makes
    /// it one. This is the one that matters.
    InsideGitRepo { root: PathBuf, message: String },
    /// Anything else — not absolute, names a directory, its folder does not
    /// exist, and so on.
    Unusable(String),
}

impl Refusal {
    pub fn message(&self) -> &str {
        match self {
            Self::InsideGitRepo { message, .. } => message,
            Self::Unusable(message) => message,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// The path offered first: outside every repository by construction, and gone on
/// reboot, which is the right default for a credential (operator, 2026-08-17).
pub fn default_local_path() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::temp_dir().join("llm_config.json")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/tmp/llm_config.json")
    }
}

/// The paths the UI suggests, best first.
///
/// `/tmp` leads because nothing there can be committed and it does not survive a
/// reboot. The home directory follows for a developer who wants the key to
/// outlive one — it is outside a repository as long as `$HOME` itself is not one,
/// which [`validate`] still checks rather than assumes.
pub fn suggested_paths() -> Vec<PathBuf> {
    let mut out = vec![default_local_path()];
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".powerrustcobol-llm.json"));
    }
    out
}

/// The git working tree `path` sits in, if any.
///
/// Walks the ancestors looking for a `.git` entry, and accepts a `.git` **file**
/// as readily as a directory: that is what a submodule and a `git worktree`
/// checkout have, and a key committed from one of those is just as published.
///
/// The path itself is not required to exist — a file about to be created inside a
/// repository is exactly the case worth refusing.
pub fn git_root_for(path: &Path) -> Option<PathBuf> {
    let start: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    // Skip the leaf itself: a file NAMED `.git` is not a repository, and the
    // question is which tree its directory belongs to.
    for dir in start.parent()?.ancestors() {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Judge a path the developer typed or picked.
pub fn validate(path: &Path) -> Result<(), Refusal> {
    let shown = path.display();
    if path.as_os_str().is_empty() {
        return Err(Refusal::Unusable(
            "Name a file for the model configuration.".into(),
        ));
    }
    if !path.is_absolute() {
        return Err(Refusal::Unusable(format!(
            "'{shown}' is a relative path. Give the full path, so which file it \
             means never depends on where the IDE was started."
        )));
    }
    if path.is_dir() {
        return Err(Refusal::Unusable(format!(
            "'{shown}' is a folder. Name the file itself, e.g. \
             {}.",
            default_local_path().display()
        )));
    }
    // The rail. Checked before the folder even has to exist, because being inside
    // a repository is a reason to say no whatever else is true of the path.
    if let Some(root) = git_root_for(path) {
        return Err(Refusal::InsideGitRepo {
            message: format!(
                "'{shown}' is inside the git repository at '{}'. An API key kept \
                 in a repository is one `git add` away from being published, and a \
                 published key cannot be taken back. Choose a file outside every \
                 repository — {} is offered for exactly this reason.",
                root.display(),
                default_local_path().display()
            ),
            root,
        });
    }
    match path.parent() {
        None => Err(Refusal::Unusable(format!(
            "'{shown}' has no folder to be written in."
        ))),
        Some(dir) if !dir.exists() => Err(Refusal::Unusable(format!(
            "The folder '{}' does not exist. Create it, or choose another file.",
            dir.display()
        ))),
        Some(_) => Ok(()),
    }
}

/// The model configuration as a portable file: everything the machine config
/// holds, plus the credentials it deliberately never carries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelConfigFile {
    /// [`FILE_VERSION`] at the time of writing.
    pub version: u32,
    /// Unix seconds, so a developer can tell how old the key on file is — the
    /// same question `api_key_saved_at` answers for a 401.
    pub saved_at: i64,
    /// A warning to whoever opens the file expecting it to be harmless.
    pub warning: String,
    /// The configuration itself, exactly as the machine config serialises it.
    pub config: LlmConfig,
    /// Slot name → key. `BTreeMap` so the file has a stable order and a diff of
    /// two saves is readable.
    pub api_keys: BTreeMap<String, String>,
}

const FILE_WARNING: &str =
    "This file contains API keys in clear text. Keep it out of every git \
     repository and out of any backup you share.";

/// Write `config` and its keys to `path`, owner-readable only.
///
/// [`validate`] is applied here as well as in the UI: this is the last gate
/// before a key reaches a disk, and a caller that forgot to ask must not be the
/// reason a credential lands in a repository.
pub fn save(path: &Path, config: &LlmConfig, now_unix: i64) -> Result<(), String> {
    validate(path).map_err(|refusal| refusal.message().to_owned())?;
    let file = ModelConfigFile {
        version: FILE_VERSION,
        saved_at: now_unix,
        warning: FILE_WARNING.to_owned(),
        config: config.clone(),
        api_keys: config
            .api_keys
            .iter()
            .filter(|(_, key)| !key.trim().is_empty())
            .map(|(slot, key)| (slot.clone(), key.clone()))
            .collect(),
    };
    let data = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    write_owner_only(path, data.as_bytes())
}

/// Read a config file back. `Err` for a missing or unreadable file, so the
/// caller can say which it was.
pub fn load(path: &Path) -> Result<ModelConfigFile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Could not read '{}': {e}", path.display()))?;
    serde_json::from_str::<ModelConfigFile>(&text)
        .map_err(|e| format!("'{}' is not a model configuration file: {e}", path.display()))
}

/// Delete a config file, and say so plainly. A missing file is already the
/// wanted state, not an error.
pub fn forget(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Could not remove '{}': {e}", path.display())),
    }
}

/// Write via a temporary file in the same folder and rename, with owner-only
/// permissions set BEFORE any byte lands — the same care
/// `llm.rs::write_config_file` takes, and for a file that certainly holds keys
/// rather than one that merely might.
fn write_owner_only(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let temp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(data).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rail: a file inside a git working tree is refused, and the refusal
    /// names the repository so the developer can act on it.
    #[test]
    fn a_file_inside_a_git_repository_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("project");
        std::fs::create_dir_all(repo.join(".git")).expect("fake repo");
        std::fs::create_dir_all(repo.join("src/deep/deeper")).expect("subdirs");

        // At the root, and buried three levels down: both are in the repository.
        for candidate in [
            repo.join("llm_config.json"),
            repo.join("src/deep/deeper/keys.json"),
        ] {
            let refusal = validate(&candidate).expect_err("must be refused");
            let Refusal::InsideGitRepo { root, message } = &refusal else {
                panic!("wrong refusal for {}: {refusal:?}", candidate.display());
            };
            assert_eq!(root, &repo, "the refusal must name the repository root");
            assert!(
                message.contains("cannot be taken back"),
                "the refusal must say why: {message}"
            );
            assert!(
                message.contains(&default_local_path().display().to_string()),
                "…and offer somewhere to put it instead: {message}"
            );
        }

        // A `.git` FILE, not a directory — a submodule or a `git worktree`
        // checkout. A key committed from one of those is just as published.
        let linked = tmp.path().join("worktree");
        std::fs::create_dir_all(&linked).expect("worktree dir");
        std::fs::write(linked.join(".git"), "gitdir: /elsewhere/.git\n").expect("gitdir file");
        assert!(matches!(
            validate(&linked.join("keys.json")),
            Err(Refusal::InsideGitRepo { .. })
        ));

        // Outside every repository: allowed.
        let clean = tmp.path().join("outside.json");
        assert_eq!(validate(&clean), Ok(()), "a path with no repo above it is fine");
        assert!(git_root_for(&clean).is_none());

        println!(
            "\n  Credential file guard — refused at a repo root, three directories \
             down, and in a `git worktree`/submodule checkout (a `.git` FILE); the \
             refusal names the repository and offers {} instead; a path outside every \
             repository is accepted\n",
            default_local_path().display()
        );
    }

    /// The other reasons a path cannot be used, each said in words.
    #[test]
    fn an_unusable_path_says_what_is_wrong_with_it() {
        assert!(matches!(
            validate(Path::new("")),
            Err(Refusal::Unusable(_))
        ));
        let relative = validate(Path::new("llm_config.json")).expect_err("relative");
        assert!(
            relative.message().contains("full path"),
            "{}",
            relative.message()
        );

        let tmp = tempfile::tempdir().expect("tempdir");
        let as_dir = validate(tmp.path()).expect_err("a folder is not a file");
        assert!(as_dir.message().contains("is a folder"), "{}", as_dir.message());

        let missing = validate(&tmp.path().join("nope/keys.json")).expect_err("no folder");
        assert!(
            missing.message().contains("does not exist"),
            "{}",
            missing.message()
        );

        println!(
            "\n  Credential file guard — a blank path, a relative path, a folder and a \
             missing parent folder are each refused with the reason in words\n"
        );
    }

    /// The keys survive the round trip, and the file says what it is.
    #[test]
    fn the_configuration_and_its_keys_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("llm_config.json");

        let mut cfg = LlmConfig::load_defaults_for_test();
        cfg.provider = "anthropic".into();
        cfg.model = "claude-opus-5".into();
        cfg.api_keys
            .insert(crate::llm::provider_key_slot("anthropic"), "sk-secret".into());
        // A blank key is not worth writing — it is the absence of a credential.
        cfg.api_keys
            .insert(crate::llm::provider_key_slot("openai"), "   ".into());

        save(&path, &cfg, 1_766_000_000).expect("saves");
        let back = load(&path).expect("loads");

        assert_eq!(back.version, FILE_VERSION);
        assert_eq!(back.saved_at, 1_766_000_000);
        assert_eq!(back.config.provider, "anthropic");
        assert_eq!(back.config.model, "claude-opus-5");
        assert_eq!(
            back.api_keys
                .get(&crate::llm::provider_key_slot("anthropic"))
                .map(String::as_str),
            Some("sk-secret"),
            "the key is the whole point of the file"
        );
        assert!(
            !back
                .api_keys
                .contains_key(&crate::llm::provider_key_slot("openai")),
            "a blank key is not a credential"
        );

        // Whoever opens the file is told what they are holding.
        let raw = std::fs::read_to_string(&path).expect("readable");
        assert!(raw.contains("clear text"), "the file must warn: {raw}");

        // Owner-only, from the first byte.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the file must not be readable by others");
        }

        // Saving again replaces it rather than accumulating temp files.
        save(&path, &cfg, 1_766_000_100).expect("saves again");
        assert_eq!(load(&path).expect("loads").saved_at, 1_766_000_100);
        assert!(!path.with_extension("json.tmp").exists(), "no temp left behind");

        // And it can be taken back off the disk, twice, without complaint.
        forget(&path).expect("removes");
        assert!(!path.exists());
        forget(&path).expect("a missing file is already the wanted state");

        println!(
            "\n  Model configuration file — provider/model and 1 key round-trip \
             (a blank key is dropped), the file carries a clear-text warning, mode is \
             0600, re-saving leaves no temp file, and forget() is idempotent\n"
        );
    }

    /// `save` refuses a repository path even if a caller forgot to ask first.
    /// This is the last gate before a key reaches a disk.
    #[test]
    fn save_refuses_a_repository_path_on_its_own() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("project");
        std::fs::create_dir_all(repo.join(".git")).expect("fake repo");
        let path = repo.join("llm_config.json");

        let err = save(&path, &LlmConfig::load_defaults_for_test(), 0).expect_err("refused");
        assert!(err.contains("git repository"), "{err}");
        assert!(!path.exists(), "nothing may be written on a refusal");

        println!(
            "\n  Model configuration file — save() applies the git guard itself, so a \
             caller that skipped the check cannot be the reason a key lands in a \
             repository; the file is not created\n"
        );
    }

    /// The vault choices, and the one that is not ready.
    #[test]
    fn the_os_vault_is_offered_and_refused_until_rc3() {
        assert!(Vault::Session.available());
        assert!(Vault::LocalFile.available());
        assert!(
            !Vault::OsVault.available(),
            "the OS vault ships in {OS_VAULT_SHIPS_IN}"
        );
        assert_eq!(Vault::default(), Vault::Session, "nothing is persisted unasked");

        // Offered rather than hidden, so nobody has to guess whether it is coming.
        assert_eq!(Vault::ALL.len(), 3);
        assert!(Vault::ALL.contains(&Vault::OsVault));

        for v in Vault::ALL {
            assert_eq!(Vault::parse(v.as_str()), *v, "{} must round-trip", v.as_str());
        }
        // A newer build's spelling degrades to "not persisted" rather than
        // refusing to load a config.
        assert_eq!(Vault::parse("tpm-sealed"), Vault::Session);
        assert_eq!(Vault::parse(""), Vault::Session);
        // Spellings a developer might reasonably type or a config might carry.
        assert_eq!(Vault::parse("Local-File"), Vault::LocalFile);
        assert_eq!(Vault::parse("keychain"), Vault::OsVault);

        println!(
            "\n  Credential vaults — 3 offered (session, local-file, os-vault), \
             os-vault refused until {OS_VAULT_SHIPS_IN}; the default persists nothing; \
             every spelling round-trips and an unknown one degrades to session\n"
        );
    }
}
