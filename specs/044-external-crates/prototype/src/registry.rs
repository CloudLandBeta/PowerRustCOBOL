// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The pluggable registry client (spec 044 R4–R9).
//!
//! One base URL, crates.io-compatible interface — "pluggable" means exactly
//! this URL changes (R4), everything else is protocol:
//!
//! - search:    `GET {base}/api/v1/crates?q=<query>&per_page=<n>`      (R6)
//! - versions:  `GET {base}/api/v1/crates/<name>`                      (R7)
//! - download:  `GET {base}/api/v1/crates/<name>/<version>/download`   (R8)
//!   (redirects to the `.crate` gzipped tarball; ureq follows it)
//!
//! Reuse map: this module becomes the IDE-side registry service the add
//! dialog calls from a background thread — same blocking `ureq` + native-tls
//! stack `cobolt-runtime::http_runtime` already uses, same explicit
//! `tls_connector` handoff.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use semver::{Version, VersionReq};
use serde::Deserialize;

/// crates.io policy requires an identifying User-Agent on API calls.
const USER_AGENT: &str = "PowerRustCOBOL-external-crates-prototype/0.1 (spec 044)";

/// Cap on a downloaded `.crate` body — a registry answering nonsense must not
/// fill the disk.
const MAX_CRATE_BYTES: u64 = 64 * 1024 * 1024;

// ── Errors (R9 wants the *which* of a failure, not just "failed") ────────────

#[derive(Debug)]
pub enum Error {
    /// The registry does not know the crate at all.
    UnknownCrate(String),
    /// The crate exists but no version satisfies the requirement.
    NoMatchingVersion { name: String, requirement: String },
    /// The requirement string itself does not parse.
    BadRequirement { requirement: String, detail: String },
    /// Network / protocol / registry-side trouble.
    Registry(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownCrate(name) => {
                write!(f, "the registry does not know a crate named `{name}`")
            }
            Error::NoMatchingVersion { name, requirement } => {
                write!(f, "`{name}` has no version matching `{requirement}`")
            }
            Error::BadRequirement { requirement, detail } => {
                write!(f, "version requirement `{requirement}` is invalid: {detail}")
            }
            Error::Registry(msg) => write!(f, "registry unreachable or unusable: {msg}"),
        }
    }
}

// ── Wire types (tolerant: unknown fields ignored, missing ones defaulted) ────

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    crates: Vec<SearchCrate>,
}

#[derive(Deserialize)]
struct SearchCrate {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    max_stable_version: Option<String>,
    #[serde(default)]
    max_version: Option<String>,
}

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(default)]
    versions: Vec<VersionRow>,
}

#[derive(Deserialize)]
struct VersionRow {
    num: String,
    #[serde(default)]
    yanked: bool,
}

// ── Public surface ───────────────────────────────────────────────────────────

/// One row of the add dialog's search results (R6).
pub struct SearchHit {
    pub name: String,
    pub newest: String,
    pub description: String,
}

/// A pinned resolution: what R8 records in the project.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    pub version: Version,
    /// The crate's page on the registry it came from — the manifest's URL
    /// column (R8, R24). Recorded at add time so a later registry switch
    /// cannot rewrite history (R5).
    pub url: String,
}

pub struct Registry {
    base: String,
    agent: ureq::Agent,
}

impl Registry {
    /// `base` is the one pluggable thing: `https://crates.io` by default,
    /// a mirror when the IDE setting says so (R4).
    pub fn new(base: &str) -> Self {
        // Mirror http_runtime: ureq with default-features off has no TLS
        // until a connector is handed to it explicitly.
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT);
        if let Ok(connector) = native_tls::TlsConnector::new() {
            builder = builder.tls_connector(Arc::new(connector));
        }
        Registry {
            base: base.trim_end_matches('/').to_string(),
            agent: builder.build(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The crate's human-facing page on this registry.
    pub fn crate_url(&self, name: &str) -> String {
        format!("{}/crates/{}", self.base, name)
    }

    /// R6 — search the configured registry; the add dialog renders these rows.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, Error> {
        let url = format!("{}/api/v1/crates", self.base);
        let response = self
            .agent
            .get(&url)
            .query("q", query)
            .query("per_page", &limit.to_string())
            .call()
            .map_err(|e| Error::Registry(e.to_string()))?;
        let parsed: SearchResponse = response
            .into_json()
            .map_err(|e| Error::Registry(format!("bad search response: {e}")))?;
        Ok(parsed
            .crates
            .into_iter()
            .map(|c| SearchHit {
                newest: c
                    .max_stable_version
                    .or(c.max_version)
                    .unwrap_or_else(|| "?".into()),
                description: c.description.unwrap_or_default().replace('\n', " "),
                name: c.name,
            })
            .collect())
    }

    /// R7 — resolve `name` + optional requirement to one exact version:
    /// the newest non-yanked version matching the requirement, prereleases
    /// excluded (an empty requirement means "newest stable").
    pub fn resolve(&self, name: &str, requirement: Option<&str>) -> Result<Resolved, Error> {
        let req = match requirement.filter(|r| !r.trim().is_empty()) {
            None => VersionReq::STAR,
            Some(raw) => VersionReq::parse(raw).map_err(|e| Error::BadRequirement {
                requirement: raw.to_string(),
                detail: e.to_string(),
            })?,
        };

        let url = format!("{}/api/v1/crates/{}", self.base, name);
        let response = match self.agent.get(&url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(404, _)) => return Err(Error::UnknownCrate(name.into())),
            Err(e) => return Err(Error::Registry(e.to_string())),
        };
        let parsed: CrateResponse = response
            .into_json()
            .map_err(|e| Error::Registry(format!("bad crate response: {e}")))?;

        let candidates: Vec<Version> = parsed
            .versions
            .iter()
            .filter(|v| !v.yanked)
            .filter_map(|v| Version::parse(&v.num).ok())
            .collect();
        match pick_version(&candidates, &req) {
            Some(v) => Ok(Resolved {
                url: self.crate_url(name),
                name: name.to_string(),
                version: v,
            }),
            None => Err(Error::NoMatchingVersion {
                name: name.into(),
                requirement: requirement.unwrap_or("newest stable").into(),
            }),
        }
    }

    /// R8 — fetch the `.crate` tarball and unpack it under `dest_root`,
    /// returning the vendored directory `dest_root/<name>-<version>`.
    pub fn download_and_unpack(&self, r: &Resolved, dest_root: &Path) -> Result<PathBuf, Error> {
        let url = format!(
            "{}/api/v1/crates/{}/{}/download",
            self.base, r.name, r.version
        );
        let response = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| Error::Registry(format!("download failed: {e}")))?;

        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_CRATE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::Registry(format!("download read failed: {e}")))?;

        let target = dest_root.join(format!("{}-{}", r.name, r.version));
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|e| Error::Registry(format!("cannot clear {}: {e}", target.display())))?;
        }
        std::fs::create_dir_all(dest_root)
            .map_err(|e| Error::Registry(format!("cannot create {}: {e}", dest_root.display())))?;

        // A `.crate` is a gzipped tar whose entries all live under
        // `<name>-<version>/`; tar's unpack refuses path traversal.
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        archive
            .unpack(dest_root)
            .map_err(|e| Error::Registry(format!("unpack failed: {e}")))?;

        if !target.is_dir() {
            return Err(Error::Registry(format!(
                "archive did not contain the expected {}-{} directory",
                r.name, r.version
            )));
        }
        Ok(target)
    }
}

/// Newest candidate matching `req`, prereleases excluded unless the
/// requirement itself asks for one.
fn pick_version(candidates: &[Version], req: &VersionReq) -> Option<Version> {
    let wants_prerelease = req
        .comparators
        .iter()
        .any(|c| !c.pre.is_empty());
    candidates
        .iter()
        .filter(|v| wants_prerelease || v.pre.is_empty())
        .filter(|v| req.matches(v))
        .max()
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    /// R7 — an empty requirement means newest stable: prereleases lose even
    /// when they are numerically newest.
    #[test]
    fn newest_stable_skips_prereleases() {
        let all = [v("1.2.0"), v("1.3.0-beta.1"), v("1.2.9")];
        assert_eq!(pick_version(&all, &VersionReq::STAR), Some(v("1.2.9")));
    }

    /// R7 — a requirement narrows the pick to its own range.
    #[test]
    fn requirement_narrows_the_pick() {
        let all = [v("0.8.5"), v("0.9.2"), v("1.0.0")];
        let req = VersionReq::parse("^0.8").unwrap();
        assert_eq!(pick_version(&all, &req), Some(v("0.8.5")));
    }

    /// R9 — nothing matching is a distinct outcome, not a panic.
    #[test]
    fn no_match_is_none() {
        let all = [v("1.0.0")];
        let req = VersionReq::parse("^2").unwrap();
        assert_eq!(pick_version(&all, &req), None);
    }

    /// R4 — the pluggable part is the base URL; derived URLs follow it.
    #[test]
    fn urls_follow_the_configured_base() {
        let r = Registry::new("https://mirror.example.com/");
        assert_eq!(r.base(), "https://mirror.example.com");
        assert_eq!(r.crate_url("csv"), "https://mirror.example.com/crates/csv");
    }
}
