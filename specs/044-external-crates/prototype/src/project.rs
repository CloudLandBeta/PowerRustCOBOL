// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The project-side record of registered crates (spec 044 R8, R10).
//!
//! Reuse map: `RegisteredCrate` becomes a `[[crates]]` array on
//! `CoboltProject` in `cobolt-ide/src/project_model.rs` (serde `default`, so
//! old projects load unchanged) and the vendored sources live under the
//! project's `crates/` — the External Crates category's on-disk root (R1).
//! The prototype keeps its own `external-crates.toml` instead of touching a
//! real `cobolt.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const STATE_FILE: &str = "external-crates.toml";
pub const VENDOR_DIR: &str = "crates";

/// Everything R8 says an add records. `requirement` is the developer's own
/// words (empty = newest stable); `version` is the exact pin every build
/// uses until an explicit update (R10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredCrate {
    pub name: String,
    #[serde(default)]
    pub requirement: String,
    pub version: String,
    #[serde(default)]
    pub features: Vec<String>,
    /// The crate's page on the registry it came from — recorded at add time
    /// so a registry switch never rewrites it (R5).
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CratesFile {
    #[serde(default, rename = "crate")]
    pub crates: Vec<RegisteredCrate>,
}

impl CratesFile {
    pub fn load(project_dir: &Path) -> Result<CratesFile, String> {
        let path = project_dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(CratesFile::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("bad {}: {e}", path.display()))
    }

    pub fn save(&self, project_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(project_dir)
            .map_err(|e| format!("cannot create {}: {e}", project_dir.display()))?;
        let path = project_dir.join(STATE_FILE);
        let text =
            toml::to_string_pretty(self).map_err(|e| format!("cannot serialize state: {e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    pub fn find(&self, name: &str) -> Option<&RegisteredCrate> {
        let wanted = lib_name(name);
        self.crates.iter().find(|c| lib_name(&c.name) == wanted)
    }

    pub fn remove(&mut self, name: &str) -> Option<RegisteredCrate> {
        let wanted = lib_name(name);
        let idx = self.crates.iter().position(|c| lib_name(&c.name) == wanted)?;
        Some(self.crates.remove(idx))
    }
}

/// Where a registered crate's source is vendored: `<project>/crates/<name>-<version>`.
pub fn vendor_dir(project_dir: &Path, name: &str, version: &str) -> PathBuf {
    project_dir.join(VENDOR_DIR).join(format!("{name}-{version}"))
}

/// R20 — the name a block writes in `use` lines: crates.io `-` becomes `_`.
/// Also the normalization for name comparisons (cargo treats `foo-bar` and
/// `foo_bar` as the same package name).
pub fn lib_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R20 — `serde-json` is written `serde_json` inside a block.
    #[test]
    fn lib_name_swaps_dashes() {
        assert_eq!(lib_name("serde-json"), "serde_json");
        assert_eq!(lib_name("  CSV "), "csv");
    }

    /// R8/R10 — the pin round-trips through the state file untouched.
    #[test]
    fn state_round_trips() {
        let dir = std::env::temp_dir().join(format!("prc044-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut file = CratesFile::default();
        file.crates.push(RegisteredCrate {
            name: "csv".into(),
            requirement: String::new(),
            version: "1.3.1".into(),
            features: vec!["serde".into()],
            url: "https://crates.io/crates/csv".into(),
        });
        file.save(&dir).unwrap();
        let back = CratesFile::load(&dir).unwrap();
        assert_eq!(back.crates.len(), 1);
        assert_eq!(back.crates[0].version, "1.3.1");
        assert_eq!(back.crates[0].features, vec!["serde".to_string()]);
        // `find` matches through dash/underscore normalization.
        assert!(back.find("CSV").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
