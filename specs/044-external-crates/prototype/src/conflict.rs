// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Conflict detection at add/update time (spec 044 R11–R15).
//!
//! Two layers, cheap first:
//!
//! 1. **Name collision** (R12) — the candidate's name against the crates the
//!    generated program links *directly* (the `generate_cargo_toml` table) and
//!    against the `cobolt-*` workspace crates. Answered locally, instantly.
//! 2. **The probe** (R13/R14/R15) — synthesize the same manifest
//!    `cobolt-compiler::generate_cargo_toml` emits, add the registered crates
//!    and the candidate, and let **cargo's own resolver** judge it via
//!    `cargo metadata` (no compilation, no source downloads — index only).
//!    Cargo is the only honest oracle for links collisions and version-graph
//!    consistency; anything less re-implements the resolver badly.
//!
//! The probe also demonstrates the R15 mechanism: each vendored crate enters
//! through `[patch.crates-io]`, so the project-local copy *replaces* the
//! registry copy everywhere in the graph — one copy per unified version,
//! never a path copy and a registry copy side by side.
//!
//! Reuse map: `base_manifest` IS next-to-verbatim `generate_cargo_toml` plus
//! the new `[patch]` section — the final implementation extends that function
//! and calls the probe from the IDE's add dialog (background thread) and from
//! `build_core`. `DIRECT_LINKED` must then come from `cobolt-compiler` as the
//! single source of truth shared with `cobolt-semantic`'s `LINKED_CRATES`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use semver::{Version, VersionReq};

use crate::project::{lib_name, vendor_dir, RegisteredCrate};

/// The crates every generated program links directly, with the requirement
/// `generate_cargo_toml` writes for them (`cobolt-compiler/src/lib.rs:1293`).
/// A block's `use` name must denote exactly one crate, so these names are
/// reserved (R12).
const DIRECT_LINKED: &[(&str, &str)] = &[
    ("flate2", "1"),
    ("bincode", "1"),
    ("tracing", "0.1"),
    ("tracing-subscriber", "0.3"),
    ("eframe", "0.36"),
    ("egui", "0.36"),
    ("egui_extras", "0.36"),
    ("rfd", "0.14"),
    ("pollster", "0.3"),
];

/// Workspace crates the generated program links by path — reserved outright.
const WORKSPACE_LINKED: &[&str] = &[
    "cobolt-ast",
    "cobolt-runtime",
    "cobolt-form-host",
    "cobolt-forms",
    "cobolt-media",
];

pub enum Verdict {
    /// Coexists cleanly; `packages` is the resolved-graph size.
    Clean { packages: usize },
    /// Coexists, but subtly (R14): the messages name each duplicated crate.
    Warnings { messages: Vec<String>, packages: usize },
    /// Cannot be added (R12/R13); the reason is shown to the developer.
    Refused { reason: String },
}

/// Layer 1 — R12. `resolved_version` is the version the add would pin.
pub fn name_collision(candidate: &str, resolved_version: &Version) -> Option<Verdict> {
    let wanted = lib_name(candidate);
    if WORKSPACE_LINKED.iter().any(|w| lib_name(w) == wanted) {
        return Some(Verdict::Refused {
            reason: format!("`{candidate}` is part of PowerRustCOBOL itself and is always linked"),
        });
    }
    let (name, linked_req) = DIRECT_LINKED
        .iter()
        .find(|(n, _)| lib_name(n) == wanted)?;
    let req = VersionReq::parse(linked_req).expect("DIRECT_LINKED requirements parse");
    Some(if req.matches(resolved_version) {
        Verdict::Refused {
            reason: format!(
                "`{name}` {resolved_version} is already available — every program links \
                 `{name} {linked_req}`; use it in a block directly, no add needed"
            ),
        }
    } else {
        Verdict::Refused {
            reason: format!(
                "`{name}` {resolved_version} clashes with the built-in `{name} {linked_req}`: \
                 one `use {}` cannot denote two crates",
                lib_name(name)
            ),
        }
    })
}

/// Layer 2 — the resolver probe (R13/R14/R15).
///
/// `others` are the already-registered crates (patched to their vendored
/// sources); `candidate` is the one being added/updated, `candidate_vendored`
/// its source if already on disk (updates re-probe after nothing changed on
/// disk yet, so usually `None` for a new version).
pub fn probe(
    workspace_root: &Path,
    project_dir: &Path,
    others: &[RegisteredCrate],
    candidate: &RegisteredCrate,
    scratch: &Path,
) -> Result<Verdict, String> {
    // Baseline graph (without the candidate), then the full graph. The
    // difference is what the candidate *brought in* — the honest basis for
    // the R14 "second incompatible copy" warning.
    let baseline = resolve_graph(workspace_root, project_dir, others, &scratch.join("base"))?;
    let full = match resolve_graph_with(
        workspace_root,
        project_dir,
        others,
        Some(candidate),
        &scratch.join("full"),
    ) {
        Ok(g) => g,
        // The resolver refusing IS the verdict, not an internal error (R13).
        Err(reason) => return Ok(Verdict::Refused { reason }),
    };

    // R15 — one copy per (name, version). With the [patch] mechanism this
    // cannot trip; the guard stays so a regression is loud, not silent bloat.
    let mut seen = BTreeSet::new();
    for (name, version) in &full {
        if !seen.insert((name.clone(), version.clone())) {
            return Ok(Verdict::Refused {
                reason: format!(
                    "internal: two copies of {name} {version} in one graph — \
                     the vendored patch is misconfigured"
                ),
            });
        }
    }

    // R14 — names that already resolved in the baseline and now carry an
    // additional, semver-incompatible version.
    let mut base_versions: BTreeMap<String, BTreeSet<Version>> = BTreeMap::new();
    for (name, version) in &baseline {
        base_versions.entry(name.clone()).or_default().insert(version.clone());
    }
    let mut warnings = Vec::new();
    for (name, version) in &full {
        let Some(existing) = base_versions.get(name) else {
            continue;
        };
        if existing.contains(version) {
            continue;
        }
        let old = existing.iter().next_back().expect("non-empty");
        warnings.push(format!(
            "`{name}` will exist twice in the binary ({old} and {version}); \
             the copies' types do not interoperate"
        ));
    }
    warnings.sort();
    warnings.dedup();

    let packages = full.len();
    Ok(if warnings.is_empty() {
        Verdict::Clean { packages }
    } else {
        Verdict::Warnings { messages: warnings, packages }
    })
}

fn resolve_graph(
    workspace_root: &Path,
    project_dir: &Path,
    registered: &[RegisteredCrate],
    scratch: &Path,
) -> Result<Vec<(String, Version)>, String> {
    resolve_graph_with(workspace_root, project_dir, registered, None, scratch)
}

/// Stage the probe package and ask `cargo metadata` for the resolved graph.
fn resolve_graph_with(
    workspace_root: &Path,
    project_dir: &Path,
    registered: &[RegisteredCrate],
    candidate: Option<&RegisteredCrate>,
    scratch: &Path,
) -> Result<Vec<(String, Version)>, String> {
    std::fs::create_dir_all(scratch.join("src"))
        .map_err(|e| format!("cannot stage probe at {}: {e}", scratch.display()))?;
    std::fs::write(scratch.join("src/lib.rs"), "")
        .map_err(|e| format!("cannot stage probe lib.rs: {e}"))?;
    let manifest = base_manifest(workspace_root, project_dir, registered, candidate);
    std::fs::write(scratch.join("Cargo.toml"), manifest)
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
        let version = Version::parse(version).map_err(|e| format!("bad version in graph: {e}"))?;
        graph.push((name.to_string(), version));
    }
    Ok(graph)
}

/// The probe manifest — `generate_cargo_toml` with `links_gui = true`
/// (a program containing any block links the GUI stack), plus the external
/// crates as exact-pinned dependencies and their vendored sources as
/// `[patch.crates-io]` entries (R10, R15).
fn base_manifest(
    workspace_root: &Path,
    project_dir: &Path,
    registered: &[RegisteredCrate],
    candidate: Option<&RegisteredCrate>,
) -> String {
    // Cargo resolves `path =` entries relative to the manifest that names
    // them — the probe is staged elsewhere, so every path must be absolute.
    let cp = absolutize(&workspace_root.join("crates"));
    let cp = cp.display();
    let mut s = format!(
        r#"[package]
name    = "prc-probe"
version = "0.0.0"
edition = "2021"

# Detached: the probe must resolve as its own package even when the folder
# it is staged in happens to live under someone else's cargo workspace.
[workspace]

[dependencies]
cobolt-ast      = {{ path = "{cp}/cobolt-ast" }}
cobolt-runtime  = {{ path = "{cp}/cobolt-runtime" }}
flate2          = "1"
bincode         = "1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
tracing         = "0.1"
cobolt-form-host = {{ path = "{cp}/cobolt-form-host" }}
cobolt-forms    = {{ path = "{cp}/cobolt-forms", features = ["render"] }}
cobolt-media    = {{ path = "{cp}/cobolt-media" }}
eframe          = {{ version = "0.36", features = ["default_fonts"] }}
egui            = "0.36"
egui_extras     = {{ version = "0.36", features = ["image"] }}
rfd             = "0.14"
pollster        = "0.3"
"#
    );

    let mut patches = String::new();
    for c in registered.iter().chain(candidate) {
        let features = if c.features.is_empty() {
            String::new()
        } else {
            format!(
                ", features = [{}]",
                c.features
                    .iter()
                    .map(|f| format!("\"{f}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        s.push_str(&format!(
            "{} = {{ version = \"={}\"{features} }}\n",
            c.name, c.version
        ));
        let vendored = vendor_dir(project_dir, &c.name, &c.version);
        if vendored.is_dir() {
            patches.push_str(&format!(
                "{} = {{ path = \"{}\" }}\n",
                c.name,
                absolutize(&vendored).display()
            ));
        }
    }
    if !patches.is_empty() {
        s.push_str("\n[patch.crates-io]\n");
        s.push_str(&patches);
    }
    s
}

/// A path cargo can use from anywhere: canonical when it exists, otherwise
/// anchored to the current directory.
fn absolutize(path: &Path) -> std::path::PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// The last few meaningful lines of a cargo error — the resolver's reason,
/// without the scrollback (R13 shows this to the developer).
fn tail(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let keep = lines.len().saturating_sub(12);
    lines[keep..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    /// R12 — a compatible duplicate of a directly-linked crate is refused as
    /// already available; an incompatible one is refused naming the built-in.
    #[test]
    fn direct_link_collisions_are_refused_both_ways() {
        match name_collision("egui", &v("0.36.1")) {
            Some(Verdict::Refused { reason }) => assert!(reason.contains("already available")),
            _ => panic!("compatible egui must be refused as already available"),
        }
        match name_collision("egui", &v("0.29.0")) {
            Some(Verdict::Refused { reason }) => assert!(reason.contains("clashes")),
            _ => panic!("incompatible egui must be refused as a clash"),
        }
    }

    /// R12 — workspace crates are reserved outright, dash/underscore blind.
    #[test]
    fn workspace_names_are_reserved() {
        match name_collision("cobolt_forms", &v("9.9.9")) {
            Some(Verdict::Refused { reason }) => assert!(reason.contains("PowerRustCOBOL")),
            _ => panic!("cobolt-forms must be reserved"),
        }
    }

    /// A name nobody links passes layer 1 untouched.
    #[test]
    fn unrelated_names_pass_layer_one() {
        assert!(name_collision("csv", &v("1.3.1")).is_none());
    }

    /// R15 — the probe manifest routes every vendored crate through
    /// [patch.crates-io], the mechanism that guarantees one copy.
    #[test]
    fn vendored_crates_enter_via_patch() {
        let project = std::env::temp_dir().join(format!("prc044-man-{}", std::process::id()));
        let vend = vendor_dir(&project, "csv", "1.3.1");
        std::fs::create_dir_all(&vend).unwrap();
        let c = RegisteredCrate {
            name: "csv".into(),
            requirement: String::new(),
            version: "1.3.1".into(),
            features: vec![],
            url: String::new(),
        };
        let manifest = base_manifest(Path::new("/ws"), &project, &[], Some(&c));
        assert!(manifest.contains("csv = { version = \"=1.3.1\" }"));
        assert!(manifest.contains("[patch.crates-io]"));
        assert!(manifest.contains("csv-1.3.1"));
        let _ = std::fs::remove_dir_all(&project);
    }
}
