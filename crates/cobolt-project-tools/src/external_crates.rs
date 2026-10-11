// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! External Crates — the registry client and the actions (spec 044), moved
//! down from the IDE so that the coding-agent tools can register a project's
//! crates too (operator, 2026-10-11: only the IDE's Project's Crates dialog
//! did, and a session that cannot reach the IDE could not generate Rust code
//! with `EXEC RUST`).
//!
//! Search the configured registry (R6), resolve-to-pin (R7), the two-layer
//! conflict check with cargo's resolver as the oracle (R11–R15), vendoring
//! into the project's `crates/` (R8), explicit updates (R16–R18), and removal
//! (R19). The **only** network code of the whole feature lives in this file —
//! builds consume the vendored source through `cobolt-compiler` alone.
//!
//! State contract: an action works on the project's pin list, which the
//! caller loads, and hands the changed list to a `save` callback at the one
//! moment it must be recorded — the IDE saves through its project model, the
//! headless tools write the manifest. The code here never reads or writes the
//! manifest itself, and progress narrates through a [`Note`] sink, so the same
//! code reports to the dialog's log pane and to a tool's answer.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cobolt_compiler::external_crates::{
    self, lib_name, name_collision, probe_manifest, vendor_dir, CollisionRefusal,
};
use cobolt_compiler::ExternalCrate;
use semver::Version;
use serde::Deserialize;

/// Identifying User-Agent, per crates.io's crawler policy. The caller says
/// which product it is (`PowerRustCOBOL/1.91.0 (Project's Crates)`).
fn default_user_agent() -> String {
    "PowerRustCOBOL (Project's Crates)".to_string()
}

/// Cap on a downloaded `.crate` body.
const MAX_CRATE_BYTES: u64 = 64 * 1024 * 1024;

/// The registry every action goes to unless the host names another.
pub const DEFAULT_REGISTRY: &str = "https://crates.io";

// ── Progress notes ───────────────────────────────────────────────────────────

/// One progress line for the dialog's log pane. Refusals and failures arrive
/// as `Err(String)` from the action instead.
pub enum Note {
    Info(String),
    Warn(String),
}

pub type Log<'a> = &'a mut dyn FnMut(Note);

/// Where an action records the project's changed pin list, at the one moment
/// it must be recorded. Its failure is the action's.
pub type Save<'a> = &'a mut dyn FnMut(&[ExternalCrate]) -> Result<(), String>;

fn info(log: Log, text: impl Into<String>) {
    log(Note::Info(text.into()));
}

// ── Registry client (R4, R6–R9) ──────────────────────────────────────────────

/// Why a registry interaction failed — R9 wants the *which*, not "failed".
#[derive(Debug)]
pub enum RegistryError {
    UnknownCrate(String),
    NoMatchingVersion { name: String, requirement: String },
    BadRequirement { requirement: String, detail: String },
    Registry(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::UnknownCrate(name) => {
                write!(f, "the registry does not know a crate named `{name}`")
            }
            RegistryError::NoMatchingVersion { name, requirement } => {
                write!(f, "`{name}` has no version matching `{requirement}`")
            }
            RegistryError::BadRequirement { requirement, detail } => {
                write!(f, "version requirement `{requirement}` is invalid: {detail}")
            }
            RegistryError::Registry(msg) => {
                write!(f, "registry unreachable or unusable: {msg}")
            }
        }
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    crates: Vec<SearchCrate>,
    /// `meta.total` — how many crates match in total, which is what makes
    /// paging possible: the dialog shows page N of ⌈total/50⌉ (spec 044 R6).
    #[serde(default)]
    meta: SearchMeta,
}

#[derive(Deserialize, Default)]
struct SearchMeta {
    #[serde(default)]
    total: usize,
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
    /// All-time downloads — the honest popularity signal when a query like
    /// "maps" returns hundreds of crates and the developer has to choose.
    #[serde(default)]
    downloads: u64,
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

/// One row of the dialog's search results (R6).
pub struct SearchHit {
    pub name: String,
    pub newest: String,
    pub description: String,
    pub downloads: u64,
}

/// One page of search results plus how many matches exist in total, so the
/// dialog can page through everything the registry has rather than showing a
/// truncated handful (spec 044 R6).
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub total: usize,
}

/// Results per page in the dialog. crates.io caps `per_page` at 100; 50 keeps
/// a page readable while still covering most searches in one or two pages.
pub const RESULTS_PER_PAGE: usize = 50;

/// A pinned resolution — what R8 records.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    pub version: Version,
    pub url: String,
}

pub struct Registry {
    base: String,
    agent: ureq::Agent,
}

impl Registry {
    /// `base` is the one pluggable thing (R4) — the IDE-wide setting.
    pub fn new(base: &str) -> Self {
        Self::with_user_agent(base, &default_user_agent())
    }

    /// As [`Registry::new`], naming the product and its version to the
    /// registry (the IDE sends its own version).
    pub fn with_user_agent(base: &str, user_agent: &str) -> Self {
        // Mirror `http_runtime`: ureq with default features off has no TLS
        // until a connector is handed to it explicitly.
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent);
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

    /// The crate's human-facing page on this registry — the manifest URL.
    pub fn crate_url(&self, name: &str) -> String {
        format!("{}/crates/{}", self.base, name)
    }

    /// R6 — one page of the dialog's search results. `page` is 1-based; the
    /// registry decides the ordering (crates.io ranks by relevance).
    pub fn search(
        &self,
        query: &str,
        per_page: usize,
        page: usize,
    ) -> Result<SearchPage, RegistryError> {
        let url = format!("{}/api/v1/crates", self.base);
        let response = self
            .agent
            .get(&url)
            .query("q", query)
            .query("per_page", &per_page.to_string())
            .query("page", &page.max(1).to_string())
            .call()
            .map_err(|e| RegistryError::Registry(e.to_string()))?;
        let parsed: SearchResponse = response
            .into_json()
            .map_err(|e| RegistryError::Registry(format!("bad search response: {e}")))?;
        let hits: Vec<SearchHit> = parsed
            .crates
            .into_iter()
            .map(|c| SearchHit {
                newest: c
                    .max_stable_version
                    .or(c.max_version)
                    .unwrap_or_else(|| "?".into()),
                description: c
                    .description
                    .unwrap_or_default()
                    .replace(['\n', '\r'], " "),
                downloads: c.downloads,
                name: c.name,
            })
            .collect();
        // A registry that omits `meta.total` (a minimal mirror) still pages:
        // fall back to what this page holds so the count is never a lie.
        let total = parsed.meta.total.max(hits.len());
        Ok(SearchPage { hits, total })
    }

    /// R7 — resolve to one exact version: the newest non-yanked release
    /// matching the requirement, prereleases excluded (empty requirement =
    /// newest stable).
    pub fn resolve(
        &self,
        name: &str,
        requirement: Option<&str>,
    ) -> Result<Resolved, RegistryError> {
        let req = match requirement.filter(|r| !r.trim().is_empty()) {
            None => semver::VersionReq::STAR,
            Some(raw) => {
                semver::VersionReq::parse(raw).map_err(|e| RegistryError::BadRequirement {
                    requirement: raw.to_string(),
                    detail: e.to_string(),
                })?
            }
        };

        let url = format!("{}/api/v1/crates/{}", self.base, name);
        let response = match self.agent.get(&url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(404, _)) => {
                return Err(RegistryError::UnknownCrate(name.into()))
            }
            Err(e) => return Err(RegistryError::Registry(e.to_string())),
        };
        let parsed: CrateResponse = response
            .into_json()
            .map_err(|e| RegistryError::Registry(format!("bad crate response: {e}")))?;

        let candidates: Vec<Version> = parsed
            .versions
            .iter()
            .filter(|v| !v.yanked)
            .filter_map(|v| Version::parse(&v.num).ok())
            .collect();
        match pick_version(&candidates, &req) {
            Some(version) => Ok(Resolved {
                url: self.crate_url(name),
                name: name.to_string(),
                version,
            }),
            None => Err(RegistryError::NoMatchingVersion {
                name: name.into(),
                requirement: requirement.unwrap_or("newest stable").into(),
            }),
        }
    }

    /// R8 — fetch the `.crate` tarball and unpack it under the project's
    /// `crates/`, returning the vendored directory.
    pub fn download_and_unpack(
        &self,
        resolved: &Resolved,
        project_dir: &Path,
    ) -> Result<PathBuf, RegistryError> {
        let url = format!(
            "{}/api/v1/crates/{}/{}/download",
            self.base, resolved.name, resolved.version
        );
        let response = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| RegistryError::Registry(format!("download failed: {e}")))?;

        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_CRATE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| RegistryError::Registry(format!("download read failed: {e}")))?;

        let target = vendor_dir(project_dir, &resolved.name, &resolved.version.to_string());
        let dest_root = target.parent().expect("vendor dir has a parent").to_path_buf();
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| {
                RegistryError::Registry(format!("cannot clear {}: {e}", target.display()))
            })?;
        }
        std::fs::create_dir_all(&dest_root).map_err(|e| {
            RegistryError::Registry(format!("cannot create {}: {e}", dest_root.display()))
        })?;

        // A `.crate` is a gzipped tar whose entries live under
        // `<name>-<version>/`; tar's unpack refuses path traversal.
        tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
            .unpack(&dest_root)
            .map_err(|e| RegistryError::Registry(format!("unpack failed: {e}")))?;

        if !target.is_dir() {
            return Err(RegistryError::Registry(format!(
                "archive did not contain the expected {}-{} directory",
                resolved.name, resolved.version
            )));
        }
        Ok(target)
    }
}

/// Newest candidate matching `req`, prereleases excluded unless the
/// requirement itself asks for one.
fn pick_version(candidates: &[Version], req: &semver::VersionReq) -> Option<Version> {
    let wants_prerelease = req.comparators.iter().any(|c| !c.pre.is_empty());
    candidates
        .iter()
        .filter(|v| wants_prerelease || v.pre.is_empty())
        .filter(|v| req.matches(v))
        .max()
        .cloned()
}

// ── The resolver probe (R11–R15) ─────────────────────────────────────────────

enum ProbeVerdict {
    Clean { packages: usize },
    Warnings { messages: Vec<String>, packages: usize },
    Refused { reason: String },
}

/// Layer 2: stage `cobolt_compiler`'s probe manifest — the same text a real
/// build stages — and let `cargo metadata` judge it. Baseline-vs-candidate
/// diff yields the R14 coexistence warning; duplicate (name, version) trips
/// the R15 guard; a resolver error IS the R13 refusal.
fn probe(
    workspace_root: &Path,
    project_dir: &Path,
    others: &[ExternalCrate],
    candidate: &ExternalCrate,
) -> Result<ProbeVerdict, String> {
    let scratch = std::env::temp_dir().join(format!(
        "prc044-probe-{}",
        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
    ));
    let crates_path = workspace_root.join("crates");
    let baseline = resolve_graph(&crates_path, project_dir, others, None, &scratch.join("base"))?;
    let full = match resolve_graph(
        &crates_path,
        project_dir,
        others,
        Some(candidate),
        &scratch.join("full"),
    ) {
        Ok(graph) => graph,
        // The resolver refusing IS the verdict, not an internal error (R13).
        Err(reason) => return Ok(ProbeVerdict::Refused { reason }),
    };

    // R15 — one copy per (name, version); with the patch mechanism this
    // cannot trip, and the guard keeps a regression loud instead of silent.
    let mut seen = std::collections::BTreeSet::new();
    for entry in &full {
        if !seen.insert(entry.clone()) {
            return Ok(ProbeVerdict::Refused {
                reason: format!(
                    "internal: two copies of {} {} in one graph — the vendored \
                     patch is misconfigured",
                    entry.0, entry.1
                ),
            });
        }
    }

    // R14 — names already resolved in the baseline that now carry a second,
    // semver-incompatible version.
    let mut base_versions: std::collections::BTreeMap<String, Vec<Version>> =
        std::collections::BTreeMap::new();
    for (name, version) in &baseline {
        base_versions.entry(name.clone()).or_default().push(version.clone());
    }
    let mut warnings = Vec::new();
    for (name, version) in &full {
        let Some(existing) = base_versions.get(name) else { continue };
        if existing.contains(version) {
            continue;
        }
        let old = existing.iter().max().expect("non-empty");
        warnings.push(format!(
            "`{name}` will exist twice in the binary ({old} and {version}); \
             the copies' types do not interoperate"
        ));
    }
    warnings.sort();
    warnings.dedup();

    let packages = full.len();
    Ok(if warnings.is_empty() {
        ProbeVerdict::Clean { packages }
    } else {
        ProbeVerdict::Warnings { messages: warnings, packages }
    })
}

fn resolve_graph(
    crates_path: &Path,
    project_dir: &Path,
    pins: &[ExternalCrate],
    candidate: Option<&ExternalCrate>,
    scratch: &Path,
) -> Result<Vec<(String, Version)>, String> {
    std::fs::create_dir_all(scratch.join("src"))
        .map_err(|e| format!("cannot stage probe at {}: {e}", scratch.display()))?;
    std::fs::write(scratch.join("src/lib.rs"), "")
        .map_err(|e| format!("cannot stage probe lib.rs: {e}"))?;
    std::fs::write(
        scratch.join("Cargo.toml"),
        probe_manifest(crates_path, project_dir, pins, candidate),
    )
    .map_err(|e| format!("cannot stage probe manifest: {e}"))?;

    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(scratch)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        return Err(tail(&String::from_utf8_lossy(&output.stderr)));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("bad cargo metadata output: {e}"))?;
    let mut graph = Vec::new();
    for package in json["packages"].as_array().into_iter().flatten() {
        let (Some(name), Some(version)) = (package["name"].as_str(), package["version"].as_str())
        else {
            continue;
        };
        if name == "prc-probe" {
            continue;
        }
        let version =
            Version::parse(version).map_err(|e| format!("bad version in graph: {e}"))?;
        graph.push((name.to_string(), version));
    }
    Ok(graph)
}

// ── System awareness (spec 045 R5–R12) ───────────────────────────────────────

/// The platform's own resolved dependency closure, split into what it links
/// **directly** and everything else the closure pulls in only transitively.
/// A search result matching a name in either set cannot be registered as an
/// External Crate — `direct` refuses with (or, per R1, offers an alias for)
/// today's `name_collision`; `transitive` refuses unconditionally (R11).
#[derive(Debug, Clone, Default)]
pub struct SystemClosure {
    pub direct: std::collections::BTreeSet<String>,
    pub transitive: std::collections::BTreeSet<String>,
}

impl SystemClosure {
    pub fn classify(&self, name: &str) -> Option<SystemCategory> {
        let wanted = lib_name(name);
        if self.direct.contains(&wanted) {
            Some(SystemCategory::Direct)
        } else if self.transitive.contains(&wanted) {
            Some(SystemCategory::Transitive)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCategory {
    /// Linked directly by every generated program (`DIRECT_LINKED`).
    Direct,
    /// Not linked directly, but pulled in by something that is.
    Transitive,
}

/// Spec 045 R12 — computed by running the *same* resolver-probe machinery
/// spec 044 built ([`resolve_graph`]) with an empty pin list and no
/// candidate: that is exactly the platform's own base dependency graph, with
/// nothing added. Reusing it (rather than a hand-maintained list) means this
/// can never drift from what a real build actually links, the same guarantee
/// `DIRECT_LINKED`'s own parity test gives the direct set.
pub fn system_closure(workspace_root: &Path) -> Result<SystemClosure, String> {
    let crates_path = workspace_root.join("crates");
    let scratch = std::env::temp_dir().join("prc045-system-closure");
    let graph = resolve_graph(&crates_path, workspace_root, &[], None, &scratch)?;
    let direct_names: std::collections::BTreeSet<String> =
        external_crates::direct_linked_lib_names().into_iter().collect();
    let mut closure = SystemClosure::default();
    for (name, _version) in graph {
        let lib = lib_name(&name);
        if direct_names.contains(&lib) {
            closure.direct.insert(lib);
        } else {
            closure.transitive.insert(lib);
        }
    }
    Ok(closure)
}

/// The last meaningful lines of a cargo error — the resolver's reason,
/// without the scrollback (R13 shows this to the developer).
fn tail(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let keep = lines.len().saturating_sub(12);
    lines[keep..].join("\n")
}

// ── Actions (R7–R19) ─────────────────────────────────────────────────────────

/// Plan §5 / user-code-is-sacred: before the FIRST crate is vendored, a
/// pre-existing `<project>/crates` folder with content we did not put there
/// refuses the add rather than writing into the developer's own files.
fn foreign_crates_dir(project_dir: &Path, registered: &[ExternalCrate]) -> Option<String> {
    let dir = project_dir.join(external_crates::VENDOR_SUBDIR);
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    let expected: Vec<String> = registered
        .iter()
        .map(|c| format!("{}-{}", c.name, c.version))
        .collect();
    let foreign: Vec<String> = entries
        .into_iter()
        .filter(|n| !expected.contains(n))
        .collect();
    if foreign.is_empty() {
        None
    } else {
        Some(format!(
            "the project's `crates/` folder already contains {} — it is not \
             managed by Project's Crates, and nothing will be written into it; \
             move that content elsewhere first",
            foreign.join(", ")
        ))
    }
}

/// What `add` finished with — either it's registered, or (spec 045 R1) its
/// name collided with a directly-linked platform crate at an incompatible
/// version and it is offered as an alias instead of refused outright.
#[derive(Debug)]
pub enum AddOutcome {
    Added(String),
    AliasOffered {
        candidate: ExternalCrate,
        linked_requirement: String,
        vendored: PathBuf,
    },
}

/// R7–R15 — resolve, conflict-check, vendor, record. Returns the pinned
/// version on success, or (spec 045 R1) an alias offer instead of a refusal
/// for the one narrow collision case that qualifies.
#[allow(clippy::too_many_arguments)]
pub fn add(
    registry: &Registry,
    project_dir: &Path,
    pins: &mut Vec<ExternalCrate>,
    workspace_root: Option<PathBuf>,
    name: &str,
    requirement: Option<&str>,
    features: Vec<String>,
    system: Option<&SystemClosure>,
    log: Log,
    save: Save,
) -> Result<AddOutcome, String> {
    let name = name.trim().to_ascii_lowercase();
    if pins.iter().any(|c| lib_name(&c.name) == lib_name(&name)) {
        return Err(format!("`{name}` is already registered — use Update"));
    }
    if let Some(reason) = foreign_crates_dir(project_dir, pins) {
        return Err(reason);
    }
    // Spec 045 R11 (relaxed 2026-10-11, operator) — a crate the application
    // links DIRECTLY is refused before any network call: a block can already
    // `use` it, and a second copy could only shadow it. Unlike a direct
    // incompatible collision (R1), there is no alias offer for this case.
    // A crate the application merely pulls in as a dependency of one it links
    // is NOT refused: no block can `use` it — the generated program does not
    // depend on it — so registering it is what makes it usable, and the
    // resolver probe below judges whether the two can share a graph.
    if let Some(category) = system.and_then(|s| s.classify(&name)) {
        match category {
            SystemCategory::Direct => {
                return Err(format!(
                    "`{name}` is already part of the application (directly linked) — it cannot be registered"
                ));
            }
            SystemCategory::Transitive => info(
                log,
                format!(
                    "`{name}` is already in the application as a dependency of a linked crate; \
                     registering it makes it usable from a block"
                ),
            ),
        }
    }

    // Resolve first: the collision verdict depends on the exact version (R12).
    info(log, format!("resolving `{name}` on {} …", registry.base()));
    let resolved = registry
        .resolve(&name, requirement)
        .map_err(|e| e.to_string())?;
    info(log, format!("pinned {} {}", resolved.name, resolved.version));

    let candidate = ExternalCrate {
        name: resolved.name.clone(),
        requirement: requirement.unwrap_or_default().to_string(),
        version: resolved.version.to_string(),
        features,
        url: resolved.url.clone(),
        alias: None,
    };

    // Vendor BEFORE the probe: the candidate then enters the staged graph
    // through its `[patch.crates-io]` path — exactly as a real build sees it
    // — which also lets a crate that exists only on a non-crates.io registry
    // resolve (R4). A refusal cleans the download up again — EXCEPT the R1
    // alias offer below, which keeps it: the developer may still want it.
    info(log, "downloading into crates/ …");
    let vendored = registry
        .download_and_unpack(&resolved, project_dir)
        .map_err(|e| e.to_string())?;
    info(log, format!("vendored at {}", vendored.display()));

    // Layer 1 (R12) first, on its own: an `Incompatible` direct collision is
    // an offer (spec 045 R1), not a refusal — `AlreadyAvailable`/`Reserved`
    // are unchanged (R2).
    match layer1_collision(&candidate)? {
        Some(CollisionRefusal::Incompatible { linked_requirement, .. }) => {
            return Ok(AddOutcome::AliasOffered { candidate, linked_requirement, vendored });
        }
        Some(refusal) => {
            let _ = std::fs::remove_dir_all(&vendored);
            return Err(refusal.to_string());
        }
        None => {}
    }

    if let Err(reason) = probe_conflicts(project_dir, workspace_root, pins, &candidate, log) {
        let _ = std::fs::remove_dir_all(&vendored);
        return Err(reason);
    }

    pins.push(candidate);
    save(pins)?;
    Ok(AddOutcome::Added(format!(
        "added `{name}` {} — blocks can now `use {}::…;`",
        resolved.version,
        lib_name(&name)
    )))
}

/// Spec 045 R1 — the developer accepted an `AddOutcome::AliasOffered`. The
/// candidate is already resolved and vendored (from the `add` call that
/// produced the offer); this only needs to stamp the alias on, run the
/// layer-2 probe against the now alias-shaped candidate (its `pin_sections`
/// form is a bare path dependency — see `external_crates::pin_sections`),
/// and record it. Layer 1 does not run again: the candidate's whole reason
/// for existing is that it collides, and `layer1_collision` already skips
/// aliased pins for exactly that reason.
pub fn confirm_alias(
    project_dir: &Path,
    pins: &mut Vec<ExternalCrate>,
    workspace_root: Option<PathBuf>,
    mut candidate: ExternalCrate,
    alias: &str,
    log: Log,
    save: Save,
) -> Result<String, String> {
    let alias = alias.trim().to_string();
    candidate.alias = Some(alias.clone());

    probe_conflicts(project_dir, workspace_root, pins, &candidate, log)?;

    let (name, version) = (candidate.name.clone(), candidate.version.clone());
    pins.push(candidate);
    save(pins)?;
    Ok(format!(
        "added `{name}` {version} as `{alias}` — blocks can now `use {}::…;`",
        lib_name(&alias)
    ))
}

/// Spec 045 R1 — the developer declined the offer. The download from `add`'s
/// resolve step never got a pin, so there's nothing to do but clear it —
/// mirrors the cleanup every other refusal path in this file already does.
pub fn discard_alias_offer(vendored: &Path) -> Result<(), String> {
    if vendored.is_dir() {
        std::fs::remove_dir_all(vendored)
            .map_err(|e| format!("cannot remove {}: {e}", vendored.display()))?;
    }
    Ok(())
}

/// Layer 1 (spec R12; spec 045 R1) — the candidate's name against the
/// reserved set. Skipped once a pin is already aliased: that pin exists
/// specifically because its name collides, so re-tripping the same refusal
/// on every later Update would leave an aliased pin permanently stuck.
fn layer1_collision(candidate: &ExternalCrate) -> Result<Option<CollisionRefusal>, String> {
    if candidate.alias.is_some() {
        return Ok(None);
    }
    let version = Version::parse(&candidate.version)
        .map_err(|e| format!("bad pinned version: {e}"))?;
    Ok(name_collision(&candidate.name, &version))
}

/// Layer 1 + layer 2; refusals are `Err`, coexistence warnings pass through
/// the log (R14 allows them). Used by `update_one` — unlike `add`, an update
/// has nowhere to offer an alias mid-flow, so a direct incompatible
/// collision here is still a plain refusal (this only happens if the linked
/// platform version itself changed since the pin was added).
fn check_conflicts(
    project_dir: &Path,
    workspace_root: Option<PathBuf>,
    others: &[ExternalCrate],
    candidate: &ExternalCrate,
    log: Log,
) -> Result<(), String> {
    if let Some(refusal) = layer1_collision(candidate)? {
        return Err(refusal.to_string());
    }
    probe_conflicts(project_dir, workspace_root, others, candidate, log)
}

/// Layer 2 only: the resolver probe (R11, R13–R15).
fn probe_conflicts(
    project_dir: &Path,
    workspace_root: Option<PathBuf>,
    others: &[ExternalCrate],
    candidate: &ExternalCrate,
    log: Log,
) -> Result<(), String> {
    let workspace = cobolt_compiler::resolve_workspace_root(workspace_root)
        .ok_or("cannot locate the PowerRustCOBOL workspace for the resolver probe")?;
    info(log, "probing the full dependency graph (cargo metadata) …");
    match probe(&workspace, project_dir, others, candidate)? {
        ProbeVerdict::Clean { packages } => {
            info(log, format!("resolver: clean — {packages} packages in the graph"));
            Ok(())
        }
        ProbeVerdict::Warnings { messages, packages } => {
            for m in messages {
                log(Note::Warn(m));
            }
            info(log, format!("resolver: coexists with warnings — {packages} packages"));
            Ok(())
        }
        ProbeVerdict::Refused { reason } => Err(reason),
    }
}

/// The outcome of one crate's update, for the R17 summary.
pub enum UpdateOutcome {
    Updated { name: String, old: String, new: String },
    Current { name: String },
    Failed { name: String, reason: String },
}

/// R16–R18 — update the named crates (empty = all). Every crate reports one
/// [`UpdateOutcome`]; a failure leaves that crate's pin and source untouched.
pub fn update(
    registry: &Registry,
    project_dir: &Path,
    pins: &mut Vec<ExternalCrate>,
    workspace_root: Option<PathBuf>,
    targets: &[String],
    log: Log,
    save: Save,
) -> Result<Vec<UpdateOutcome>, String> {
    let targets: Vec<String> = if targets.is_empty() {
        pins.iter().map(|c| c.name.clone()).collect()
    } else {
        targets.to_vec()
    };

    let mut outcomes = Vec::new();
    for name in targets {
        let recorded = match pins
            .iter()
            .find(|c| lib_name(&c.name) == lib_name(&name))
            .cloned()
        {
            Some(c) => c,
            None => {
                outcomes.push(UpdateOutcome::Failed {
                    name,
                    reason: "not registered".into(),
                });
                continue;
            }
        };
        match update_one(registry, project_dir, workspace_root.clone(), pins, &recorded, log) {
            Ok(Some((old, new))) => {
                info(log, format!("updated {}: {old} → {new}", recorded.name));
                outcomes.push(UpdateOutcome::Updated { name: recorded.name, old, new });
            }
            Ok(None) => outcomes.push(UpdateOutcome::Current { name: recorded.name }),
            Err(reason) => {
                outcomes.push(UpdateOutcome::Failed { name: recorded.name, reason })
            }
        }
    }
    save(pins)?;
    Ok(outcomes)
}

fn update_one(
    registry: &Registry,
    project_dir: &Path,
    workspace_root: Option<PathBuf>,
    pins: &mut [ExternalCrate],
    recorded: &ExternalCrate,
    log: Log,
) -> Result<Option<(String, String)>, String> {
    // R16 — newest within the crate's own recorded requirement.
    let requirement =
        (!recorded.requirement.is_empty()).then_some(recorded.requirement.as_str());
    let resolved = registry
        .resolve(&recorded.name, requirement)
        .map_err(|e| e.to_string())?;
    let new_version = resolved.version.to_string();
    if new_version == recorded.version {
        return Ok(None);
    }

    let candidate = ExternalCrate {
        version: new_version.clone(),
        url: resolved.url.clone(),
        ..recorded.clone()
    };
    let others: Vec<ExternalCrate> = pins
        .iter()
        .filter(|c| lib_name(&c.name) != lib_name(&recorded.name))
        .cloned()
        .collect();
    // Vendor the NEW version first (probe sees it via its patch, like a real
    // build); a refusal removes it and leaves the old pin untouched (R18).
    let new_dir = registry
        .download_and_unpack(&resolved, project_dir)
        .map_err(|e| e.to_string())?;
    if let Err(reason) = check_conflicts(project_dir, workspace_root, &others, &candidate, log) {
        let _ = std::fs::remove_dir_all(&new_dir);
        return Err(reason);
    }
    // Only after the new source survived the probe does the old one go (R18).
    let old_dir = vendor_dir(project_dir, &recorded.name, &recorded.version);
    if old_dir.is_dir() {
        std::fs::remove_dir_all(&old_dir)
            .map_err(|e| format!("cannot remove old source {}: {e}", old_dir.display()))?;
    }
    let slot = pins
        .iter_mut()
        .find(|c| lib_name(&c.name) == lib_name(&recorded.name))
        .expect("looked up by the caller");
    *slot = candidate;
    Ok(Some((recorded.version.clone(), new_version)))
}

/// R19 — the **confirmed** removal (the dialog owns the confirmation):
/// deletes the record and the vendored source, never any COBOL.
pub fn remove(
    project_dir: &Path,
    pins: &mut Vec<ExternalCrate>,
    name: &str,
    save: Save,
) -> Result<String, String> {
    let Some(found) = pins.iter().find(|c| lib_name(&c.name) == lib_name(name)).cloned() else {
        return Err(format!("`{name}` is not registered"));
    };
    pins.retain(|c| lib_name(&c.name) != lib_name(name));
    // The record goes first, the source after: a failed save leaves the pin and
    // the code it points at, never a pin with nothing behind it.
    save(pins)?;
    let dir = vendor_dir(project_dir, &found.name, &found.version);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
    }
    Ok(format!(
        "removed `{}` — a block still using it will fail Check as unregistered",
        found.name
    ))
}

// ── Unit tests (no network, except the one marked live) ──────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn pin(name: &str, version: &str) -> ExternalCrate {
        ExternalCrate {
            name: name.into(),
            requirement: String::new(),
            version: version.into(),
            features: vec![],
            url: format!("https://crates.io/crates/{name}"),
            alias: None,
        }
    }

    /// R7 — an empty requirement means newest stable: prereleases lose even
    /// when numerically newest; a requirement narrows the pick; no match is
    /// a distinct outcome.
    #[test]
    fn version_picking_matches_the_spec() {
        let all = [v("1.2.0"), v("1.3.0-beta.1"), v("1.2.9")];
        assert_eq!(pick_version(&all, &semver::VersionReq::STAR), Some(v("1.2.9")));
        let req = semver::VersionReq::parse("^1.2.0").unwrap();
        assert_eq!(pick_version(&all, &req), Some(v("1.2.9")));
        let req = semver::VersionReq::parse("^2").unwrap();
        assert_eq!(pick_version(&all, &req), None);
    }

    /// R4 — the pluggable part is the base URL; derived URLs follow it.
    #[test]
    fn urls_follow_the_configured_base() {
        let r = Registry::new("https://mirror.example.com/");
        assert_eq!(r.base(), "https://mirror.example.com");
        assert_eq!(r.crate_url("csv"), "https://mirror.example.com/crates/csv");
    }

    /// FIX (pre-existing, found 2026-08-08) — some crates.io descriptions
    /// carry a bare `\r` (e.g. `egui-cameras`, `egui-thematic`); a `\n`-only
    /// replace let it through. A raw CR reaching UI text is never wanted
    /// regardless of how that text is later rendered, so it is normalized
    /// once, here, at the source. **Live:** real registry data is what
    /// actually carries this character.
    #[test]
    fn search_results_never_carry_a_raw_carriage_return() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        let page = registry.search("egui", 100, 1).expect("live search");
        for hit in &page.hits {
            assert!(
                !hit.description.contains('\r'),
                "`{}`'s description still carries a raw CR: {:?}",
                hit.name,
                hit.description
            );
        }
    }

    /// Plan §5 — a pre-existing user `crates/` folder refuses the add; the
    /// folder's own vendored entries do not.
    #[test]
    fn foreign_crates_dir_refuses_only_unmanaged_content() {
        let project = std::env::temp_dir().join(format!("prc044-core-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project);
        // No crates/ at all → fine.
        assert!(foreign_crates_dir(&project, &[]).is_none());
        // A managed vendor dir → fine.
        std::fs::create_dir_all(project.join("crates/csv-1.4.0")).unwrap();
        assert!(foreign_crates_dir(&project, &[pin("csv", "1.4.0")]).is_none());
        // The developer's own folder inside crates/ → refuse, naming it.
        std::fs::create_dir_all(project.join("crates/my-own-stuff")).unwrap();
        let reason = foreign_crates_dir(&project, &[pin("csv", "1.4.0")]).expect("must refuse");
        assert!(reason.contains("my-own-stuff"));
        let _ = std::fs::remove_dir_all(&project);
    }

    /// R19 — a removal records first and deletes after: when the record cannot
    /// be saved, the pin stays in the list the caller holds and the vendored
    /// source is still on disk (never a pin with nothing behind it), and a
    /// successful one deletes the named crate's record and source only.
    #[test]
    fn remove_records_first_and_deletes_the_source_only_after() {
        let dir = std::env::temp_dir().join(format!("prc044-core-rm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut pins = vec![pin("csv", "1.4.0"), pin("itoa", "1.0.10")];
        for p in &pins {
            std::fs::create_dir_all(vendor_dir(&dir, &p.name, &p.version)).unwrap();
        }

        // The save fails: the source survives.
        let refused = remove(&dir, &mut pins.clone(), "csv", &mut |_| Err("disk full".to_string()));
        assert_eq!(refused.unwrap_err(), "disk full");
        assert!(vendor_dir(&dir, "csv", "1.4.0").is_dir(), "the source is kept when the record is not");

        // The save works: record and source go, the other crate stays.
        let mut saved: Vec<Vec<String>> = Vec::new();
        let message = remove(&dir, &mut pins, "CSV", &mut |p| {
            saved.push(p.iter().map(|c| c.name.clone()).collect());
            Ok(())
        })
        .expect("remove csv");
        assert!(message.contains("csv"));
        assert_eq!(saved, [vec!["itoa".to_string()]], "saved once, without the removed pin");
        assert!(!vendor_dir(&dir, "csv", "1.4.0").exists());
        assert!(vendor_dir(&dir, "itoa", "1.0.10").is_dir());

        // An unknown crate is a clean error, not a mutation.
        assert!(remove(&dir, &mut pins, "nope", &mut |_| panic!("nothing to save")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
