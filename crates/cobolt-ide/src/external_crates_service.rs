// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! External Crates — the IDE-side service (spec 044).
//!
//! Everything the dialog does happens here, on a worker thread: search the
//! configured registry (R6), resolve-to-pin (R7), the two-layer conflict
//! check with cargo's resolver as the oracle (R11–R15), vendoring into the
//! project's `crates/` (R8), explicit updates (R16–R18), and removal (R19).
//! The **only** network code of the whole feature lives in this file —
//! builds consume the vendored source through `cobolt-compiler` alone.
//!
//! State contract: actions load `cobolt.toml`, mutate, and save it on disk
//! (`project_model::{load_project, save_project}`); the caller saves any
//! in-memory project state *before* spawning an action and reloads after it
//! finishes. Progress narrates through a [`Note`] sink, so the same code
//! reports to the dialog's log pane today and anything else tomorrow.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cobolt_compiler::external_crates::{
    self, lib_name, name_collision, probe_manifest, vendor_dir, CollisionRefusal,
};
use cobolt_compiler::ExternalCrate;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::project_model::{load_project, save_project};

/// Identifying User-Agent, per crates.io's crawler policy.
fn user_agent() -> String {
    format!("PowerRustCOBOL/{} (Project's Crates)", crate::version::VERSION)
}

/// Cap on a downloaded `.crate` body.
const MAX_CRATE_BYTES: u64 = 64 * 1024 * 1024;

// ── IDE-wide registry setting (spec R4/R5, Q7) ───────────────────────────────

pub const DEFAULT_REGISTRY: &str = "https://crates.io";

/// The pluggable registry endpoint — an **IDE-wide** setting (not stored in
/// `cobolt.toml`), persisted like `DebugSettings`: a small TOML in the IDE's
/// base dir. Changing it affects the *next* action; recorded pins keep the
/// URL they were added with (R5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCratesSettings {
    pub registry: String,
}

impl Default for ExternalCratesSettings {
    fn default() -> Self {
        Self { registry: DEFAULT_REGISTRY.to_string() }
    }
}

fn settings_path() -> PathBuf {
    crate::llm::base_dir().join("external_crates.toml")
}

impl ExternalCratesSettings {
    /// Load the persisted setting, falling back to crates.io on any error —
    /// a corrupt file must never keep the dialog from opening.
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = toml::to_string_pretty(self) {
            let path = settings_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }
}

// ── Analysis context (spec R20) ──────────────────────────────────────────────

/// The open project's registered crates as `use`-line names, published for
/// every semantic-analysis site in the IDE — the same per-frame in-process
/// sync `theme::set_active` and the debug switches use. `None` = no project
/// open (single-file semantics, R22). Behind an `RwLock` because Check/Run/
/// Debug analyze on worker threads.
static ACTIVE_LIB_NAMES: std::sync::OnceLock<std::sync::RwLock<Option<Vec<String>>>> =
    std::sync::OnceLock::new();

fn active_cell() -> &'static std::sync::RwLock<Option<Vec<String>>> {
    ACTIVE_LIB_NAMES.get_or_init(|| std::sync::RwLock::new(None))
}

/// Publish the open project's crates (the app calls this every frame, and
/// before spawning any analysis worker).
pub fn set_active_project_crates(names: Option<Vec<String>>) {
    if let Ok(mut cell) = active_cell().write() {
        *cell = names;
    }
}

/// [`cobolt_semantic::analyze_with`] under the active project's crates —
/// what every IDE analysis site calls instead of plain `analyze` (R20–R22).
pub fn analyze_project(
    program: &cobolt_ast::program::Program,
) -> cobolt_semantic::SemanticResult {
    let external_crates = active_cell().read().ok().and_then(|cell| cell.clone());
    cobolt_semantic::analyze_with(
        program,
        &cobolt_semantic::AnalyzeOptions { external_crates },
    )
}

// ── Progress notes ───────────────────────────────────────────────────────────

/// One progress line for the dialog's log pane. Refusals and failures arrive
/// as `Err(String)` from the action instead.
pub enum Note {
    Info(String),
    Warn(String),
}

pub type Log<'a> = &'a mut dyn FnMut(Note);

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
        // Mirror `http_runtime`: ureq with default features off has no TLS
        // until a connector is handed to it explicitly.
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent(&user_agent());
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
                description: c.description.unwrap_or_default().replace('\n', " "),
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

/// R7–R15 — resolve, conflict-check, vendor, record. Returns the pinned
/// version on success.
#[allow(clippy::too_many_arguments)]
pub fn add(
    registry: &Registry,
    project_path: &Path,
    workspace_root: Option<PathBuf>,
    name: &str,
    requirement: Option<&str>,
    features: Vec<String>,
    log: Log,
) -> Result<String, String> {
    let project_dir = project_dir_of(project_path)?;
    let mut project = load_project(project_path).map_err(|e| e.to_string())?;
    let name = name.trim().to_ascii_lowercase();
    if project.crates.iter().any(|c| lib_name(&c.name) == lib_name(&name)) {
        return Err(format!("`{name}` is already registered — use Update"));
    }
    if let Some(reason) = foreign_crates_dir(&project_dir, &project.crates) {
        return Err(reason);
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
    };

    // Vendor BEFORE the probe: the candidate then enters the staged graph
    // through its `[patch.crates-io]` path — exactly as a real build sees it
    // — which also lets a crate that exists only on a non-crates.io registry
    // resolve (R4). A refusal cleans the download up again.
    info(log, "downloading into crates/ …");
    let vendored = registry
        .download_and_unpack(&resolved, &project_dir)
        .map_err(|e| e.to_string())?;
    info(log, format!("vendored at {}", vendored.display()));

    if let Err(reason) =
        check_conflicts(&project_dir, workspace_root, &project.crates, &candidate, log)
    {
        let _ = std::fs::remove_dir_all(&vendored);
        return Err(reason);
    }

    project.crates.push(candidate);
    save_project(&project, project_path).map_err(|e| e.to_string())?;
    Ok(format!(
        "added `{name}` {} — blocks can now `use {}::…;`",
        resolved.version,
        lib_name(&name)
    ))
}

/// Layer 1 + layer 2; refusals are `Err`, coexistence warnings pass through
/// the log (R14 allows them).
fn check_conflicts(
    project_dir: &Path,
    workspace_root: Option<PathBuf>,
    others: &[ExternalCrate],
    candidate: &ExternalCrate,
    log: Log,
) -> Result<(), String> {
    let version = Version::parse(&candidate.version)
        .map_err(|e| format!("bad pinned version: {e}"))?;
    if let Some(refusal) = name_collision(&candidate.name, &version) {
        // AlreadyAvailable is informational, but still a refusal: nothing to
        // add (R12).
        let _: &CollisionRefusal = &refusal;
        return Err(refusal.to_string());
    }
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
    project_path: &Path,
    workspace_root: Option<PathBuf>,
    targets: &[String],
    log: Log,
) -> Result<Vec<UpdateOutcome>, String> {
    let project_dir = project_dir_of(project_path)?;
    let mut project = load_project(project_path).map_err(|e| e.to_string())?;
    let targets: Vec<String> = if targets.is_empty() {
        project.crates.iter().map(|c| c.name.clone()).collect()
    } else {
        targets.to_vec()
    };

    let mut outcomes = Vec::new();
    for name in targets {
        let recorded = match project
            .crates
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
        match update_one(registry, &project_dir, workspace_root.clone(), &mut project, &recorded, log)
        {
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
    save_project(&project, project_path).map_err(|e| e.to_string())?;
    Ok(outcomes)
}

fn update_one(
    registry: &Registry,
    project_dir: &Path,
    workspace_root: Option<PathBuf>,
    project: &mut crate::project_model::CoboltProject,
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
    let others: Vec<ExternalCrate> = project
        .crates
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
    let slot = project
        .crates
        .iter_mut()
        .find(|c| lib_name(&c.name) == lib_name(&recorded.name))
        .expect("looked up by the caller");
    *slot = candidate;
    Ok(Some((recorded.version.clone(), new_version)))
}

/// R19 — the **confirmed** removal (the dialog owns the confirmation):
/// deletes the record and the vendored source, never any COBOL.
pub fn remove(project_path: &Path, name: &str) -> Result<String, String> {
    let project_dir = project_dir_of(project_path)?;
    let mut project = load_project(project_path).map_err(|e| e.to_string())?;
    let Some(found) = project
        .crates
        .iter()
        .find(|c| lib_name(&c.name) == lib_name(name))
        .cloned()
    else {
        return Err(format!("`{name}` is not registered"));
    };
    project.crates.retain(|c| lib_name(&c.name) != lib_name(name));
    save_project(&project, project_path).map_err(|e| e.to_string())?;
    let dir = vendor_dir(&project_dir, &found.name, &found.version);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
    }
    Ok(format!(
        "removed `{}` — a block still using it will fail Check as unregistered",
        found.name
    ))
}

fn project_dir_of(project_path: &Path) -> Result<PathBuf, String> {
    project_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no parent directory", project_path.display()))
}

// ── Flow tests: mock registry + live resolver verdicts (spec 044 T12) ────────
//
// In-src because `cobolt-ide` is a bin crate (integration tests cannot import
// it). The mock half exercises AC10/AC14 against a std-`TcpListener` server
// speaking the crates.io API shape — deterministic, no dependence on
// crates.io's release calendar. The live half exercises the real resolver
// verdicts (AC6/AC7/AC8), network + cargo, per the 041 heavy-test precedent.

#[cfg(test)]
mod flow_tests {
    use super::*;
    use std::io::Write as _;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    struct MockRegistry {
        base: String,
        /// Versions the mock "has published" — tests push to simulate time.
        versions: Arc<Mutex<Vec<String>>>,
        /// Versions whose download endpoint answers 500.
        broken_downloads: Arc<Mutex<Vec<String>>>,
    }

    fn start_mock() -> MockRegistry {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registry");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let versions = Arc::new(Mutex::new(vec!["0.1.0".to_string()]));
        let broken_downloads = Arc::new(Mutex::new(Vec::<String>::new()));
        let (versions_bg, broken_bg) = (Arc::clone(&versions), Arc::clone(&broken_downloads));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let versions = Arc::clone(&versions_bg);
                let broken = Arc::clone(&broken_bg);
                std::thread::spawn(move || handle(stream, &versions, &broken));
            }
        });
        MockRegistry { base, versions, broken_downloads }
    }

    fn handle(
        mut stream: TcpStream,
        versions: &Mutex<Vec<String>>,
        broken: &Mutex<Vec<String>>,
    ) {
        use std::io::Read as _;
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let path = request
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();

        let (status, content_type, body): (&str, &str, Vec<u8>) = if path
            .starts_with("/api/v1/crates?")
        {
            // A registry with more matches than fit on one page, so the
            // dialog's paging is exercised for real: `mockcrate` is always
            // first (the tests add it), then filler up to TOTAL_MATCHES.
            const TOTAL_MATCHES: usize = 120;
            let query_of = |key: &str| -> usize {
                path.split(['?', '&'])
                    .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
            };
            let per_page = query_of("per_page").max(1);
            let page = query_of("page").max(1);
            let newest = versions.lock().unwrap().last().cloned().unwrap_or_default();
            let first = (page - 1) * per_page;
            let rows: Vec<serde_json::Value> = (first..(first + per_page).min(TOTAL_MATCHES))
                .map(|i| {
                    let name = if i == 0 {
                        "mockcrate".to_string()
                    } else {
                        format!("mockfiller{i}")
                    };
                    serde_json::json!({
                        "name": name,
                        "description": "a mock crate served by the test registry",
                        "max_stable_version": newest,
                        "downloads": (TOTAL_MATCHES - i) * 1000,
                    })
                })
                .collect();
            let json = serde_json::json!({
                "crates": rows,
                "meta": { "total": TOTAL_MATCHES },
            });
            ("200 OK", "application/json", json.to_string().into_bytes())
        } else if path == "/api/v1/crates/mockcrate" {
            let rows: Vec<serde_json::Value> = versions
                .lock()
                .unwrap()
                .iter()
                .map(|v| serde_json::json!({ "num": v, "yanked": false }))
                .collect();
            let json = serde_json::json!({ "versions": rows });
            ("200 OK", "application/json", json.to_string().into_bytes())
        } else if let Some(version) = path
            .strip_prefix("/api/v1/crates/mockcrate/")
            .and_then(|rest| rest.strip_suffix("/download"))
        {
            if broken.lock().unwrap().iter().any(|b| b == version) {
                ("500 Internal Server Error", "text/plain", b"boom".to_vec())
            } else {
                ("200 OK", "application/gzip", tarball(version))
            }
        } else {
            ("404 Not Found", "application/json", b"{}".to_vec())
        };

        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(&body);
    }

    /// A minimal but real `.crate`: gzipped tar with `Cargo.toml` + `lib.rs`
    /// under `mockcrate-<version>/`.
    fn tarball(version: &str) -> Vec<u8> {
        fn append<W: std::io::Write>(builder: &mut tar::Builder<W>, path: &str, data: &[u8]) {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, data).unwrap();
        }
        let encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let manifest = format!(
            "[package]\nname = \"mockcrate\"\nversion = \"{version}\"\nedition = \"2021\"\n"
        );
        append(&mut builder, &format!("mockcrate-{version}/Cargo.toml"), manifest.as_bytes());
        append(
            &mut builder,
            &format!("mockcrate-{version}/src/lib.rs"),
            b"pub fn answer() -> i64 { 42 }\n",
        );
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc044-flow-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = crate::project_model::CoboltProject::new("FlowDemo", "src/main.cbl");
        crate::project_model::save_project(&project, &dir.join("cobolt.toml")).unwrap();
        dir
    }

    fn quiet() -> impl FnMut(Note) {
        |_| {}
    }

    /// AC10 + AC14, fully deterministic against the mock: search hits come
    /// from the configured registry; an add vendors from it and records its
    /// URL; switching the setting back leaves the pin untouched; a newer
    /// mock release updates old → new; a broken release fails and leaves
    /// everything as it was (R18). Timings reported.
    #[test]
    fn mock_registry_add_update_and_failure() {
        let t_all = Instant::now();
        let mock = start_mock();
        let dir = temp_project("mock");
        let manifest_path = dir.join("cobolt.toml");
        let registry = Registry::new(&mock.base);

        // Search — the rows come from the mock, not crates.io (R4/R6), a full
        // page at a time, and page 2 continues where page 1 stopped instead
        // of repeating it (no truncation to a handful).
        let first = registry
            .search("mock", RESULTS_PER_PAGE, 1)
            .expect("mock search page 1");
        assert_eq!(first.hits.len(), RESULTS_PER_PAGE);
        assert_eq!(first.total, 120);
        assert_eq!(first.hits[0].name, "mockcrate");
        assert!(first.hits[0].downloads > 0, "downloads must survive the parse");
        let second = registry
            .search("mock", RESULTS_PER_PAGE, 2)
            .expect("mock search page 2");
        assert_eq!(second.hits.len(), RESULTS_PER_PAGE);
        assert_eq!(second.total, 120);
        assert!(
            !second.hits.iter().any(|h| h.name == first.hits[0].name),
            "page 2 must not repeat page 1"
        );
        // The last page is the remainder, not a padded full page.
        let last = registry
            .search("mock", RESULTS_PER_PAGE, 3)
            .expect("mock search page 3");
        assert_eq!(last.hits.len(), 20);

        // Add — resolve/download from the mock; the probe runs for real.
        let t = Instant::now();
        let added = add(&registry, &manifest_path, None, "mockcrate", None, vec![], &mut quiet())
            .expect("mock add");
        let add_s = t.elapsed().as_secs_f32();
        assert!(added.contains("0.1.0"));
        assert!(vendor_dir(&dir, "mockcrate", "0.1.0").is_dir());
        let project = load_project(&manifest_path).unwrap();
        assert_eq!(project.crates.len(), 1);
        assert!(
            project.crates[0].url.starts_with(&mock.base),
            "the recorded URL must point into the mock registry (AC14)"
        );
        // The manifest row carries that URL (AC14's manifest clause).
        let written =
            cobolt_compiler::external_crates::write_rust_manifest(
                &dir.join("dist"),
                "FlowDemo",
                &project.crates,
            )
            .unwrap()
            .expect("manifest written");
        let manifest_text = std::fs::read_to_string(written).unwrap();
        assert!(manifest_text.contains(&mock.base));

        // Switching the setting back to crates.io touches nothing (R5).
        let _crates_io = Registry::new(DEFAULT_REGISTRY);
        let untouched = load_project(&manifest_path).unwrap();
        assert_eq!(untouched.crates[0].url, project.crates[0].url);
        assert!(vendor_dir(&dir, "mockcrate", "0.1.0").is_dir());

        // The mock "publishes" 0.2.0 → Update All moves old → new (AC10).
        mock.versions.lock().unwrap().push("0.2.0".into());
        let t = Instant::now();
        let outcomes =
            update(&registry, &manifest_path, None, &[], &mut quiet()).expect("mock update");
        let update_s = t.elapsed().as_secs_f32();
        assert!(matches!(
            outcomes.as_slice(),
            [UpdateOutcome::Updated { old, new, .. }] if old == "0.1.0" && new == "0.2.0"
        ));
        assert!(vendor_dir(&dir, "mockcrate", "0.2.0").is_dir());
        assert!(!vendor_dir(&dir, "mockcrate", "0.1.0").exists());

        // A broken 0.3.0 fails the update and leaves 0.2.0 fully intact (R18).
        mock.versions.lock().unwrap().push("0.3.0".into());
        mock.broken_downloads.lock().unwrap().push("0.3.0".into());
        let outcomes =
            update(&registry, &manifest_path, None, &[], &mut quiet()).expect("update runs");
        assert!(matches!(outcomes.as_slice(), [UpdateOutcome::Failed { .. }]));
        let kept = load_project(&manifest_path).unwrap();
        assert_eq!(kept.crates[0].version, "0.2.0");
        assert!(vendor_dir(&dir, "mockcrate", "0.2.0").is_dir());

        println!("──────────────────────────────────────────────");
        println!("spec 044 mock-registry flow (AC10/AC14)");
        println!("cases: search-from-mock; add+vendor+URL; manifest URL;");
        println!("       setting switch-back; update 0.1.0→0.2.0;");
        println!("       broken release leaves pin+source untouched");
        println!("add (incl. resolver probe): {add_s:.1} s");
        println!("update (incl. probe):       {update_s:.1} s");
        println!("total:                      {:.1} s", t_all.elapsed().as_secs_f32());
        println!("──────────────────────────────────────────────");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC6/AC7/AC8 — the live verdicts, exactly as a developer meets them
    /// (crates.io + the real resolver). Timings reported.
    #[test]
    fn live_resolver_verdicts() {
        let t_all = Instant::now();
        let registry = Registry::new(DEFAULT_REGISTRY);

        // AC6 — egui is refused as already available; nothing recorded.
        let dir = temp_project("egui");
        let manifest_path = dir.join("cobolt.toml");
        let err = add(&registry, &manifest_path, None, "egui", None, vec![], &mut quiet())
            .expect_err("egui must be refused");
        assert!(err.contains("already available"), "got: {err}");
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        let _ = std::fs::remove_dir_all(&dir);

        // AC7 — an old rusqlite pins a libsqlite3-sys major that clashes
        // with the SQL runtime's on the native `sqlite3` links key; the
        // resolver's reason is surfaced and the download is cleaned up.
        let dir = temp_project("links");
        let manifest_path = dir.join("cobolt.toml");
        let t = Instant::now();
        let err = add(
            &registry,
            &manifest_path,
            None,
            "rusqlite",
            Some("=0.24.2"),
            vec![],
            &mut quiet(),
        )
        .expect_err("conflicting rusqlite must be refused");
        let links_s = t.elapsed().as_secs_f32();
        assert!(
            err.contains("sqlite3") || err.contains("links"),
            "the resolver's links reason must surface; got: {err}"
        );
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        assert!(
            !vendor_dir(&dir, "rusqlite", "0.24.2").exists(),
            "a refused add must clean its download up"
        );
        let _ = std::fs::remove_dir_all(&dir);

        // AC8 — a disjoint same-major pin coexists WITH the warning.
        let dir = temp_project("warn");
        let manifest_path = dir.join("cobolt.toml");
        let mut warnings = Vec::new();
        let t = Instant::now();
        add(&registry, &manifest_path, None, "itoa", Some("=1.0.10"), vec![], &mut |n| {
            if let Note::Warn(text) = n {
                warnings.push(text);
            }
        })
        .expect("itoa =1.0.10 must be allowed with a warning");
        let warn_s = t.elapsed().as_secs_f32();
        assert!(
            warnings.iter().any(|w| w.contains("exist twice")),
            "the coexistence warning must fire; warnings: {warnings:?}"
        );
        assert_eq!(load_project(&manifest_path).unwrap().crates.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);

        println!("──────────────────────────────────────────────");
        println!("spec 044 live resolver verdicts (AC6/AC7/AC8)");
        println!("egui already-available: refused (no resolver run)");
        println!("rusqlite =0.24.2 links clash: refused in {links_s:.1} s");
        println!("itoa =1.0.10 coexistence: warned in {warn_s:.1} s");
        println!("total: {:.1} s", t_all.elapsed().as_secs_f32());
        println!("──────────────────────────────────────────────");
    }
}

// ── Unit tests (no network) ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
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

    /// R4/Q7 — the setting round-trips through its IDE-wide file and a
    /// missing/corrupt file falls back to crates.io.
    #[test]
    fn settings_round_trip_and_default() {
        assert_eq!(ExternalCratesSettings::default().registry, DEFAULT_REGISTRY);
        let parsed: ExternalCratesSettings =
            toml::from_str("registry = \"https://mirror.example.com\"").unwrap();
        assert_eq!(parsed.registry, "https://mirror.example.com");
        let text = toml::to_string_pretty(&parsed).unwrap();
        let back: ExternalCratesSettings = toml::from_str(&text).unwrap();
        assert_eq!(back.registry, parsed.registry);
    }

    /// R19 (AC13's service half) — a confirmed removal deletes the record
    /// and the vendored source, and only them: other pins and their sources
    /// stay, and no other project file is touched.
    #[test]
    fn remove_deletes_record_and_vendored_source_only() {
        let dir = std::env::temp_dir().join(format!("prc044-svc-rm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("cobolt.toml");
        let mut project = crate::project_model::CoboltProject::new("RmDemo", "src/main.cbl");
        for (name, version) in [("csv", "1.4.0"), ("itoa", "1.0.10")] {
            project.crates.push(ExternalCrate {
                name: name.into(),
                requirement: String::new(),
                version: version.into(),
                features: vec![],
                url: format!("https://crates.io/crates/{name}"),
            });
            std::fs::create_dir_all(vendor_dir(&dir, name, version)).unwrap();
        }
        save_project(&project, &manifest_path).unwrap();

        let message = remove(&manifest_path, "csv").expect("remove csv");
        assert!(message.contains("csv"));
        let after = load_project(&manifest_path).unwrap();
        assert_eq!(after.crates.len(), 1);
        assert_eq!(after.crates[0].name, "itoa");
        assert!(!vendor_dir(&dir, "csv", "1.4.0").exists());
        assert!(vendor_dir(&dir, "itoa", "1.0.10").is_dir());
        // An unknown crate is a clean error, not a mutation.
        assert!(remove(&manifest_path, "nope").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// R20–R22 (AC3's Check half) — one test owns the process-global so the
    /// two states cannot race: with the project's crates published, a block
    /// using one passes analysis; with no project published, the same block
    /// fails with the single-file message.
    #[test]
    fn analyze_project_honours_the_published_crates() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   PROCEDURE DIVISION.\nMAIN.\n    EXEC RUST\n        use csv::x;\n    END-EXEC.\n    STOP RUN.\n";
        let program = cobolt_parser::parse(cobolt_lexer::tokenize(
            src,
            cobolt_lexer::SourceFormat::Free,
        ))
        .program
        .unwrap();

        set_active_project_crates(Some(vec!["csv".into()]));
        let allowed = analyze_project(&program);
        assert!(
            allowed
                .errors()
                .all(|d| !d.message.contains("does not link")),
            "registered csv must pass the Check path"
        );

        set_active_project_crates(None);
        let refused = analyze_project(&program);
        assert!(
            refused
                .errors()
                .any(|d| d.message.contains("require a project")),
            "with no project the single-file message must appear"
        );
    }

    /// Plan §5 — a pre-existing user `crates/` folder refuses the add; the
    /// folder’s own vendored entries do not.
    #[test]
    fn foreign_crates_dir_refuses_only_unmanaged_content() {
        let project =
            std::env::temp_dir().join(format!("prc044-svc-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project);
        // No crates/ at all → fine.
        assert!(foreign_crates_dir(&project, &[]).is_none());
        // A managed vendor dir → fine.
        let pin = ExternalCrate {
            name: "csv".into(),
            requirement: String::new(),
            version: "1.4.0".into(),
            features: vec![],
            url: String::new(),
        };
        std::fs::create_dir_all(project.join("crates/csv-1.4.0")).unwrap();
        assert!(foreign_crates_dir(&project, &[pin.clone()]).is_none());
        // The developer's own folder inside crates/ → refuse, naming it.
        std::fs::create_dir_all(project.join("crates/my-own-stuff")).unwrap();
        let reason = foreign_crates_dir(&project, &[pin]).expect("must refuse");
        assert!(reason.contains("my-own-stuff"));
        let _ = std::fs::remove_dir_all(&project);
    }
}
