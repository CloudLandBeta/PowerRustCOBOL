// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `regenerate` — the IDE's Generate, with its generator and its paths.
//!
//! A form becomes `<stem>.cbl`, an indexed definition `<stem>-indexed.cbl` plus
//! `COPYBOOKS/<stem>.SEL` / `.FD` (plan D5), each where the IDE writes it
//! ([`crate::gen_paths`]). Each generated program is recorded in the manifest
//! as generated — through the host, which owns the manifest.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::gen_paths;
use crate::host::{FileList, ProjectHost};
use crate::root::ProjectRoot;

pub fn run<H: ProjectHost>(host: &mut H, root: &ProjectRoot, path: Option<&str>) -> Result<Value, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    let targets: Vec<PathBuf> = match path {
        Some(rel) => {
            let abs = root.resolve(rel)?;
            if !abs.is_file() {
                return Err(format!("'{rel}' does not exist"));
            }
            vec![abs]
        }
        None => view
            .forms
            .iter()
            .chain(view.indexed.iter())
            .filter_map(|rel| root.resolve(rel).ok())
            .filter(|p| p.is_file())
            .collect(),
    };
    // Refuse before writing anything: an unsaved IDE edit of any target would
    // be overwritten by the IDE's next save, or would overwrite this.
    for abs in &targets {
        if host.unsaved(abs) {
            return Err(format!(
                "'{}' has unsaved changes in the IDE; ask the developer to save or close it, then try again",
                root.relative(abs).unwrap_or_default()
            ));
        }
    }
    let mut written: Vec<PathBuf> = Vec::new();
    for abs in &targets {
        match ext(abs).as_str() {
            "cfrm" => written.push(write_form(host, root, &view.generated, abs)?),
            "cidx" => written.extend(write_indexed(host, root, &view.generated, abs)?),
            _ => {
                return Err(format!(
                    "'{}' is not a form (.cfrm) or an indexed definition (.cidx)",
                    root.relative(abs).unwrap_or_default()
                ))
            }
        }
    }
    host.written(&written);
    Ok(json!({
        "written": written.iter().filter_map(|p| root.relative(p)).collect::<Vec<_>>(),
    }))
}

fn ext(p: &Path) -> String {
    p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The `.cbl` the IDE's Generate writes for `cfrm`, written.
pub fn write_form<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    generated: &[String],
    cfrm: &Path,
) -> Result<PathBuf, String> {
    let rel = root.relative(cfrm).unwrap_or_default();
    let form = cobolt_forms::load_form(cfrm).map_err(|e| format!("'{rel}' cannot be loaded: {e}"))?;
    let cbl = gen_paths::generated_cbl_path(Some(generated), Some(root.dir()), cfrm);
    let (src, _map) = cobolt_codegen::generate_with_map(&form);
    write_inside(root, &cbl, src.as_bytes())?;
    let gen_rel = root.relative(&cbl).ok_or("the generated program would land outside the project")?;
    host.record(&gen_rel, FileList::Generated)?;
    Ok(cbl)
}

/// The facade and the two copybooks the IDE writes for `cidx`, written.
fn write_indexed<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    generated: &[String],
    cidx: &Path,
) -> Result<Vec<PathBuf>, String> {
    let rel = root.relative(cidx).unwrap_or_default();
    let def = cobolt_indexed::load_indexed(cidx).map_err(|e| format!("'{rel}' cannot be loaded: {e}"))?;
    let cbl = gen_paths::generated_indexed_cbl_path(Some(generated), Some(root.dir()), cidx);
    write_inside(root, &cbl, cobolt_codegen::generate_indexed(&def).as_bytes())?;
    let (sel, fd) = gen_paths::indexed_copybook_paths(root.dir(), cidx, &def.name);
    write_inside(root, &sel, cobolt_codegen::generate_indexed_select(&def).as_bytes())?;
    write_inside(root, &fd, cobolt_codegen::generate_indexed_fd(&def).as_bytes())?;
    let gen_rel = root.relative(&cbl).ok_or("the generated program would land outside the project")?;
    host.record(&gen_rel, FileList::Generated)?;
    Ok(vec![cbl, sel, fd])
}

/// Write `bytes` to `abs`, which must resolve inside the project (a relocated
/// `generated` entry is a manifest value, so it is confined like any argument).
fn write_inside(root: &ProjectRoot, abs: &Path, bytes: &[u8]) -> Result<(), String> {
    let rel = root
        .relative(abs)
        .ok_or("a generated file would land outside the project")?;
    let abs = root.resolve(&rel)?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create the folder for '{rel}': {e}"))?;
    }
    std::fs::write(&abs, bytes).map_err(|e| format!("cannot write '{rel}': {e}"))
}
