// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Where generated COBOL goes** — the IDE's rule, moved here so the IDE's
//! Generate and the coding-agent `regenerate` tool write to one place by
//! construction (spec 080 AC6).

use std::path::{Path, PathBuf};

/// The tracked `generated` entry whose file name is `file_name`, if the
/// developer relocated it into a subfolder (spec 033, R7) — so regenerating
/// rewrites the moved file in place instead of resurrecting it at the default
/// path.
pub fn tracked_generated_rel(generated: &[String], file_name: &str) -> Option<String> {
    generated
        .iter()
        .find(|rel| Path::new(rel).file_name().and_then(|n| n.to_str()) == Some(file_name))
        .cloned()
}

/// A form's generated `.cbl`: the tracked (possibly relocated) entry when one
/// exists, else the project's `generated/` folder, else — a loose form with no
/// project — next to the `.cfrm`.
///
/// `generated` is the manifest's `[files] generated` list (`None` when no
/// project model is loaded); `project_dir` the project folder.
pub fn generated_cbl_path(
    generated: Option<&[String]>,
    project_dir: Option<&Path>,
    cfrm: &Path,
) -> PathBuf {
    let stem = cfrm.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
    in_generated(generated, project_dir, &format!("{stem}.cbl"))
        .unwrap_or_else(|| cfrm.with_extension("cbl"))
}

/// An indexed definition's generated facade, `<stem>-indexed.cbl`, by the
/// same rule as [`generated_cbl_path`].
pub fn generated_indexed_cbl_path(
    generated: Option<&[String]>,
    project_dir: Option<&Path>,
    cidx: &Path,
) -> PathBuf {
    let stem = cidx.file_stem().and_then(|s| s.to_str()).unwrap_or("indexed");
    in_generated(generated, project_dir, &format!("{stem}-indexed.cbl"))
        .unwrap_or_else(|| cidx.with_extension("cbl"))
}

fn in_generated(
    generated: Option<&[String]>,
    project_dir: Option<&Path>,
    file_name: &str,
) -> Option<PathBuf> {
    let dir = project_dir?;
    if let Some(rel) = generated.and_then(|g| tracked_generated_rel(g, file_name)) {
        return Some(dir.join(rel));
    }
    Some(dir.join("generated").join(file_name))
}

/// The record-descriptor copybooks the IDE writes beside an indexed facade:
/// `COPYBOOKS/<stem>.SEL` and `COPYBOOKS/<stem>.FD`.
pub fn indexed_copybook_paths(project_dir: &Path, cidx: &Path, fallback: &str) -> (PathBuf, PathBuf) {
    let stem = cidx.file_stem().and_then(|s| s.to_str()).unwrap_or(fallback);
    let dir = project_dir.join("COPYBOOKS");
    (dir.join(format!("{stem}.SEL")), dir.join(format!("{stem}.FD")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_paths_default_relocated_and_loose() {
        let dir = Path::new("/p");
        let cfrm = &dir.join("forms").join("Order.cfrm");
        let none: Vec<String> = vec![];
        assert_eq!(
            generated_cbl_path(Some(&none), Some(dir), cfrm),
            dir.join("generated").join("Order.cbl"),
            "default"
        );
        let moved = vec!["generated/sales/Order.cbl".to_owned()];
        assert_eq!(
            generated_cbl_path(Some(&moved), Some(dir), cfrm),
            dir.join("generated/sales/Order.cbl"),
            "relocated entry wins"
        );
        assert_eq!(
            generated_cbl_path(None, None, cfrm),
            cfrm.with_extension("cbl"),
            "a loose form writes beside itself"
        );
        assert_eq!(
            generated_indexed_cbl_path(Some(&none), Some(dir), &dir.join("indexed").join("actors.cidx")),
            dir.join("generated").join("actors-indexed.cbl")
        );
        assert_eq!(
            indexed_copybook_paths(dir, &dir.join("indexed").join("actors.cidx"), "x"),
            (dir.join("COPYBOOKS").join("actors.SEL"), dir.join("COPYBOOKS").join("actors.FD"))
        );
        assert_eq!(tracked_generated_rel(&moved, "Other.cbl"), None);
        println!("gen_paths: 6 cases — default, relocated, loose, indexed, copybooks, untracked");
    }
}
