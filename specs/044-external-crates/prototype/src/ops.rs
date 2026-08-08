// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The action layer — what the IDE's External Crates dialog calls.
//!
//! One function per user action (add, update, remove, manifest); each takes a
//! `log` sink for progress so the same code narrates to a CLI printer or to
//! the dialog's log pane. Reuse map: these become the IDE-side service
//! functions the egui dialog drives from a background thread — the dialog
//! never talks to the registry or the resolver directly.

use std::path::{Path, PathBuf};

use crate::conflict::{self, Verdict};
use crate::manifest;
use crate::project::{self, lib_name, vendor_dir, CratesFile, RegisteredCrate};
use crate::registry::Registry;

/// A progress line for whichever surface is listening (spec 044: the IDE's
/// dialog log / the Output panel).
pub enum Note {
    Info(String),
    Warn(String),
}

pub type Log<'a> = &'a mut dyn FnMut(Note);

fn info(log: Log, text: impl Into<String>) {
    log(Note::Info(text.into()));
}

// ── add (R7–R15) ─────────────────────────────────────────────────────────────

pub fn add(
    registry: &Registry,
    project_dir: &Path,
    workspace: Option<PathBuf>,
    skip_probe: bool,
    name: &str,
    requirement: Option<&str>,
    features: Vec<String>,
    log: Log,
) -> Result<String, String> {
    let name = name.trim().to_ascii_lowercase();
    let mut state = CratesFile::load(project_dir)?;
    if state.find(&name).is_some() {
        return Err(format!("`{name}` is already registered — use update"));
    }

    // Resolve first: the collision verdict depends on the exact version (R12).
    info(log, format!("resolving `{name}` on {} …", registry.base()));
    let resolved = registry
        .resolve(&name, requirement)
        .map_err(|e| e.to_string())?;
    info(log, format!("pinned {} {}", resolved.name, resolved.version));

    let candidate = RegisteredCrate {
        name: resolved.name.clone(),
        requirement: requirement.unwrap_or_default().to_string(),
        version: resolved.version.to_string(),
        features,
        url: resolved.url.clone(),
    };

    check_conflicts(project_dir, workspace, skip_probe, &state.crates, &candidate, log)?;

    info(log, "downloading into crates/ …");
    let vendored = registry
        .download_and_unpack(&resolved, &project_dir.join(project::VENDOR_DIR))
        .map_err(|e| e.to_string())?;
    info(log, format!("vendored at {}", vendored.display()));

    state.crates.push(candidate);
    state.save(project_dir)?;
    Ok(format!(
        "added `{name}` {} — blocks can now `use {}::…;`",
        resolved.version,
        lib_name(&name)
    ))
}

/// Layer 1 + layer 2; refusals are errors, warnings pass through `log` (R14).
fn check_conflicts(
    project_dir: &Path,
    workspace: Option<PathBuf>,
    skip_probe: bool,
    others: &[RegisteredCrate],
    candidate: &RegisteredCrate,
    log: Log,
) -> Result<(), String> {
    let version = semver::Version::parse(&candidate.version)
        .map_err(|e| format!("bad pinned version: {e}"))?;
    if let Some(Verdict::Refused { reason }) = conflict::name_collision(&candidate.name, &version) {
        return Err(reason);
    }
    if skip_probe {
        info(log, "(resolver probe skipped)");
        return Ok(());
    }
    let workspace = match workspace {
        Some(w) => w,
        None => find_workspace()?,
    };
    info(log, "probing the full dependency graph (cargo metadata) …");
    let scratch = project_dir.join(".probe");
    match conflict::probe(&workspace, project_dir, others, candidate, &scratch)? {
        Verdict::Clean { packages } => {
            info(log, format!("resolver: clean — {packages} packages in the graph"));
            Ok(())
        }
        Verdict::Warnings { messages, packages } => {
            for m in messages {
                log(Note::Warn(m));
            }
            info(log, format!("resolver: coexists with warnings — {packages} packages"));
            Ok(())
        }
        Verdict::Refused { reason } => Err(reason),
    }
}

// ── update (R16–R18) ─────────────────────────────────────────────────────────

/// Update the named crates (or all when `targets` is empty), reporting each
/// through `log`; returns the R17 summary line.
pub fn update(
    registry: &Registry,
    project_dir: &Path,
    workspace: Option<PathBuf>,
    skip_probe: bool,
    targets: &[String],
    log: Log,
) -> Result<String, String> {
    let mut state = CratesFile::load(project_dir)?;
    let targets: Vec<String> = if targets.is_empty() {
        state.crates.iter().map(|c| c.name.clone()).collect()
    } else {
        targets.to_vec()
    };
    if targets.is_empty() {
        return Ok("nothing registered — nothing to update".into());
    }

    let (mut updated, mut current, mut failed) = (0usize, 0usize, 0usize);
    for name in targets {
        match update_one(
            registry,
            project_dir,
            workspace.clone(),
            skip_probe,
            &mut state,
            &name,
            log,
        ) {
            Ok(Some((old, new))) => {
                info(log, format!("updated  {name}: {old} → {new}"));
                updated += 1;
            }
            Ok(None) => {
                info(log, format!("current  {name}"));
                current += 1;
            }
            // R18 — a failure leaves that crate exactly as it was.
            Err(reason) => {
                log(Note::Warn(format!("failed   {name}: {reason}")));
                failed += 1;
            }
        }
    }
    state.save(project_dir)?;
    Ok(format!("summary: {updated} updated, {current} current, {failed} failed"))
}

fn update_one(
    registry: &Registry,
    project_dir: &Path,
    workspace: Option<PathBuf>,
    skip_probe: bool,
    state: &mut CratesFile,
    name: &str,
    log: Log,
) -> Result<Option<(String, String)>, String> {
    let recorded = state
        .find(name)
        .cloned()
        .ok_or_else(|| format!("`{name}` is not registered"))?;
    // R16 — newest within the crate's own recorded requirement.
    let requirement = (!recorded.requirement.is_empty()).then_some(recorded.requirement.as_str());
    let resolved = registry
        .resolve(&recorded.name, requirement)
        .map_err(|e| e.to_string())?;
    let new_version = resolved.version.to_string();
    if new_version == recorded.version {
        return Ok(None);
    }

    let candidate = RegisteredCrate {
        version: new_version.clone(),
        url: resolved.url.clone(),
        ..recorded.clone()
    };
    let others: Vec<RegisteredCrate> = state
        .crates
        .iter()
        .filter(|c| lib_name(&c.name) != lib_name(name))
        .cloned()
        .collect();
    check_conflicts(project_dir, workspace, skip_probe, &others, &candidate, log)?;

    registry
        .download_and_unpack(&resolved, &project_dir.join(project::VENDOR_DIR))
        .map_err(|e| e.to_string())?;
    // Only after the new source is safely on disk does the old one go.
    let old_dir = vendor_dir(project_dir, &recorded.name, &recorded.version);
    if old_dir.is_dir() {
        std::fs::remove_dir_all(&old_dir)
            .map_err(|e| format!("cannot remove old source {}: {e}", old_dir.display()))?;
    }
    let slot = state
        .crates
        .iter_mut()
        .find(|c| lib_name(&c.name) == lib_name(name))
        .expect("looked up above");
    *slot = candidate;
    Ok(Some((recorded.version, new_version)))
}

// ── remove (R19) ─────────────────────────────────────────────────────────────

/// The caller owns the confirmation (dialog in the IDE, stdin in the CLI);
/// this performs the confirmed removal.
pub fn remove(project_dir: &Path, name: &str) -> Result<String, String> {
    let mut state = CratesFile::load(project_dir)?;
    let Some(found) = state.find(name).cloned() else {
        return Err(format!("`{name}` is not registered"));
    };
    state.remove(name);
    state.save(project_dir)?;
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

// ── manifest (R24–R26) ───────────────────────────────────────────────────────

pub fn write_manifest(project_dir: &Path, dest: Option<PathBuf>) -> Result<String, String> {
    let dest = dest.unwrap_or_else(|| project_dir.join("dist"));
    let state = CratesFile::load(project_dir)?;
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    Ok(match manifest::write(&dest, project_name, &state.crates)? {
        Some(path) => format!("wrote {}", path.display()),
        None => format!(
            "no external crates — no manifest (a stale {} was removed if present)",
            manifest::MANIFEST_FILE
        ),
    })
}

// ── plumbing ─────────────────────────────────────────────────────────────────

/// Walk up from the current directory to the PowerRustCOBOL checkout — the
/// probe needs its workspace crates as path dependencies.
pub fn find_workspace() -> Result<PathBuf, String> {
    let mut dir = std::env::current_dir().map_err(|e| e.to_string())?;
    loop {
        if dir.join("crates/cobolt-runtime/Cargo.toml").is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(
                "cannot find the PowerRustCOBOL workspace above the current directory — \
                 pass --workspace <dir>"
                    .into(),
            );
        }
    }
}
