// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A backend, packed (WASM-003 R2).
//!
//! The headless backend binary embeds its programs the way a built application
//! does: the parsed programs, serialised with `bincode` and deflated, plus the
//! text of `exports.toml` and of the released interface descriptions. Nothing
//! is parsed again at start-up, and nothing of the source tree is needed beside
//! the binary except its configuration.

use std::io::{Read, Write};
use std::path::Path;

use cobolt_ast::program::Program;
use serde::{Deserialize, Serialize};

use crate::backend::{load_history, load_programs, Backend, ExportsFile, StateMode};

#[derive(Serialize, Deserialize)]
pub struct Bundle {
    pub exports_toml: String,
    pub programs: Vec<Program>,
    /// The text of each released `contract/released/*.json`.
    pub history: Vec<String>,
    /// The project's `[backend] state`; the build sets it from the manifest.
    pub state: StateMode,
}

impl Bundle {
    /// Read a backend folder: the programs are parsed and analysed (every
    /// diagnostic reported together), `exports.toml` is checked.
    pub fn from_dir(dir: &Path) -> Result<Bundle, Vec<String>> {
        let exports_toml = std::fs::read_to_string(dir.join("exports.toml")).map_err(|e| vec![format!("{}: {e}", dir.join("exports.toml").display())])?;
        ExportsFile::from_toml(&exports_toml).map_err(|e| vec![e])?;
        let mut errs = Vec::new();
        let programs = load_programs(&dir.join("src"), &mut errs);
        let released = dir.join("contract").join("released");
        let history_docs = load_history(&released, &mut errs);
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Bundle { exports_toml, programs, history: history_docs.iter().map(|d| d.to_json()).collect(), state: StateMode::default() })
    }

    /// Build the runnable backend from the pack, checking it as `Backend::load` does.
    pub fn into_backend(self) -> Result<Backend, Vec<String>> {
        let exports = ExportsFile::from_toml(&self.exports_toml).map_err(|e| vec![e])?;
        let mut history = Vec::new();
        for h in &self.history {
            history.push(cobolt_wire::contract::InterfaceDescription::from_json(h).map_err(|e| vec![e])?);
        }
        let mut backend = Backend::build(self.programs, &exports, history)?;
        backend.state = self.state;
        Ok(backend)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let raw = bincode::serialize(self).map_err(|e| format!("serialising the backend: {e}"))?;
        let mut gz = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&raw).map_err(|e| e.to_string())?;
        gz.finish().map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Bundle, String> {
        let mut raw = Vec::new();
        flate2::read::DeflateDecoder::new(bytes).read_to_end(&mut raw).map_err(|e| format!("this backend's embedded programs are damaged: {e}"))?;
        bincode::deserialize(&raw).map_err(|e| {
            format!("this backend's embedded programs were written by a different version of the platform ({e}); rebuild the backend")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WASM-003 T3: the packed backend carries the project's `[backend] state`.
    #[test]
    fn a_bundle_round_trip_carries_the_state() {
        for state in [StateMode::PerCall, StateMode::PerConnection] {
            let b = Bundle { exports_toml: "[contract]\nversion = \"1.0.0\"\n".into(), programs: Vec::new(), history: Vec::new(), state };
            let bytes = b.to_bytes().unwrap();
            let back = Bundle::from_bytes(&bytes).unwrap();
            assert_eq!(back.state, state);
            println!("{:<14} → {} bytes packed → {}", state.as_str(), bytes.len(), back.state.as_str());
        }
    }
}
