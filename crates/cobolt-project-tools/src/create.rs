// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A new project, exactly as the IDE's New Project makes it** (spec 084 R15).
//!
//! The IDE's New Project and a coding agent's `create_project` (in the IDE's
//! tool server and in `rcrun mcp`) write the same project: the folders, the
//! starter main program and the manifest all come from here. The manifest is
//! [`NEW_PROJECT_TEMPLATE`], which is the IDE's own `CoboltProject::new` +
//! `save_project` output with the name and main as placeholders — an IDE test
//! renders it the IDE's way and requires the two to be byte-identical, so they
//! cannot drift (regenerate with `COBOLT_WRITE_GOLDEN=1`).

use std::path::Path;

use serde_json::{json, Value};

/// Standard project sub-folders — one per category plus working/build folders.
/// Created when a project is made, and back-filled (if missing) when one is
/// opened.
pub const PROJECT_FOLDERS: &[&str] = &[
    "src",
    "forms",
    "indexed",
    "generated",
    "Assets",
    "assets",
    "Knowledge Base",
    "bin",
    "debug",
    "temp",
    "dist",
    "data",
    "COPYBOOKS",
];

/// The main program a new project starts with, relative to the project.
pub const DEFAULT_MAIN: &str = "src/main.cbl";

/// The manifest New Project writes, with `"@@PROJECT-NAME@@"` and
/// `"@@PROJECT-MAIN@@"` where the name and the main program go.
pub const NEW_PROJECT_TEMPLATE: &str = include_str!("new_project.toml");

/// Placeholders inside [`NEW_PROJECT_TEMPLATE`] (each a whole TOML string).
pub const NAME_PLACEHOLDER: &str = "@@PROJECT-NAME@@";
pub const MAIN_PLACEHOLDER: &str = "@@PROJECT-MAIN@@";

/// A file stem safe on every file system: path and reserved characters become
/// `-`, and an empty result is `project`.
pub fn sanitize_file_stem(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}

/// The project name as the IDE stores it: the developer's name with the
/// `.project` suffix, e.g. `Inventory.project`.
pub fn project_name(name: &str) -> String {
    let name = name.trim();
    if name.ends_with(".project") {
        name.to_owned()
    } else {
        format!("{name}.project")
    }
}

/// The starter main program: runnable as is, so a new project can be Run.
pub fn starter_main(project_name: &str, main_rel: &str) -> String {
    let prog: String = Path::new(main_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("MAIN")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '-' })
        .collect();
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. {prog}.\n\
         \x20     *> {project_name} — main program.\n\
         \n\
         \x20      PROCEDURE DIVISION.\n\
         \x20          DISPLAY \"Hello from {project_name}\".\n\
         \x20          GOBACK.\n"
    )
}

/// [`NEW_PROJECT_TEMPLATE`] with the name and main filled in, TOML-escaped.
pub fn render_manifest(project_name: &str, main_rel: &str) -> String {
    let quoted = |s: &str| toml_edit::Value::from(s).to_string().trim().to_owned();
    NEW_PROJECT_TEMPLATE
        .replace(&format!("\"{NAME_PLACEHOLDER}\""), &quoted(project_name))
        .replace(&format!("\"{MAIN_PLACEHOLDER}\""), &quoted(main_rel))
}

/// Create a project named `name` in `folder` (spec 084 R15, R18): the
/// manifest, every standard folder and the starter main program. Refuses a
/// folder that already holds anything, and never overwrites.
pub fn create_project(folder: &Path, name: &str) -> Result<Value, String> {
    if name.trim().is_empty() {
        return Err("create_project needs a 'name'".to_owned());
    }
    if folder.exists() {
        let has_entries = std::fs::read_dir(folder)
            .map_err(|e| format!("cannot read the folder: {e}"))?
            .next()
            .is_some();
        if has_entries {
            return Err("that folder already holds files; create_project needs an empty or new folder".to_owned());
        }
    }
    std::fs::create_dir_all(folder).map_err(|e| format!("cannot create the folder: {e}"))?;
    let stored = project_name(name);
    let manifest_name = format!("{}.toml", sanitize_file_stem(&stored));
    for sub in PROJECT_FOLDERS {
        std::fs::create_dir_all(folder.join(sub)).map_err(|e| format!("cannot create {sub}/: {e}"))?;
    }
    std::fs::write(folder.join(DEFAULT_MAIN), starter_main(&stored, DEFAULT_MAIN))
        .map_err(|e| format!("cannot write {DEFAULT_MAIN}: {e}"))?;
    std::fs::write(folder.join(&manifest_name), render_manifest(&stored, DEFAULT_MAIN))
        .map_err(|e| format!("cannot write the manifest: {e}"))?;
    Ok(json!({
        "created": true,
        "project": manifest_name,
        "name": stored,
        "main": DEFAULT_MAIN,
        "folders": PROJECT_FOLDERS,
    }))
}
