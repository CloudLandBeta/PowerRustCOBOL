// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `add_to_project` — put an existing file in its manifest list (plan D4).
//!
//! The manifest carries a keyed seal over the project's form list; a form
//! added by editing the manifest by hand breaks it, and `rcrun` and the built
//! binary then report the application corrupted. This tool records the file
//! through the host, which re-seals exactly as an IDE save does.

use serde_json::{json, Value};

use crate::host::{FileList, ProjectHost};
use crate::root::ProjectRoot;

pub fn run<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    path: &str,
    list: Option<&str>,
) -> Result<Value, String> {
    let abs = root.resolve(path)?;
    if !abs.is_file() {
        return Err(format!("'{path}' does not exist; write the file first, then add it"));
    }
    let rel = root
        .relative(&abs)
        .ok_or_else(|| format!("'{path}' is not inside the project"))?;
    let list = match list {
        Some(key) => match FileList::from_key(key) {
            Some(FileList::Generated) | None => {
                return Err(format!(
                    "'{key}' is not a list a file can be added to (forms, indexed, sources, assets, documentation)"
                ))
            }
            Some(l) => l,
        },
        None => FileList::of_path(&rel),
    };
    if host.unsaved(root.manifest()) {
        return Err(
            "the project has unsaved changes in the IDE; ask the developer to save it, then try again"
                .to_owned(),
        );
    }
    host.record(&rel, list)?;
    host.written(&[]);
    Ok(json!({ "added": rel, "list": list.key() }))
}
