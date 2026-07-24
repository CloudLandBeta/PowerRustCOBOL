// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Target disambiguation for AI-agent (Grace) create/edit (spec 034).
//!
//! When Grace is asked to create or edit a project-tree element *by name*, folders
//! (spec 033) make the destination or target ambiguous. These pure helpers decide
//! whether a selection modal is required and gather its candidates:
//!
//! * **create** → always a folder pick ([`create_request`]); the selectable
//!   folders come from [`create_folders`].
//! * **edit** → only when the name matches **more than one** element
//!   ([`edit_candidates`]); one match resolves silently, none is a caller error.
//!
//! Every path returned here is **project-relative** (spec 033, R21).

use std::collections::BTreeSet;
use std::path::Path;

use crate::project_model::{Category, CoboltProject, FileKind};

/// Which operation the developer asked Grace to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOp {
    Create,
    Edit,
}

/// A request for the developer to pick a target on the project tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRequest {
    pub op: TargetOp,
    pub kind: FileKind,
    /// The element name as the developer named it.
    pub name: String,
    /// For an **edit**, the matching element rel paths; empty for a **create**.
    pub candidates: Vec<String>,
}

/// The developer's chosen target — a project-relative folder (create) or element
/// (edit) path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetChoice {
    pub rel_path: String,
}

/// Lower-cased file stem of a relative path, for case-insensitive name matching.
fn stem_ci(rel: &str) -> String {
    Path::new(rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Tracked elements in `kind`'s category whose **stem** matches `name`
/// (case-insensitive). `name` may be given with or without an extension.
pub fn edit_candidates(project: &CoboltProject, kind: FileKind, name: &str) -> Vec<String> {
    let cat = Category::of_kind(kind);
    let target = stem_ci(name);
    project
        .files_in(cat)
        .iter()
        .filter(|rel| stem_ci(rel) == target)
        .cloned()
        .collect()
}

/// Build the **edit** target request when the name is ambiguous. Returns `None`
/// when zero or one element matches (the caller errors on zero, uses the single
/// match on one) — no modal is shown in those cases (spec 034, R2, R4).
pub fn edit_request(project: &CoboltProject, kind: FileKind, name: &str) -> Option<TargetRequest> {
    let candidates = edit_candidates(project, kind, name);
    (candidates.len() >= 2).then(|| TargetRequest {
        op: TargetOp::Edit,
        kind,
        name: name.to_string(),
        candidates,
    })
}

/// Build the **create** target request. A create always prompts for a folder
/// (spec 034, R1), so this is unconditional.
pub fn create_request(kind: FileKind, name: &str) -> TargetRequest {
    TargetRequest {
        op: TargetOp::Create,
        kind,
        name: name.to_string(),
        candidates: Vec::new(),
    }
}

/// Selectable destination folders for a create in `kind`'s category: the category
/// root plus every folder that appears as a prefix of a tracked file. (The picker
/// UI may additionally surface empty on-disk folders; this membership-derived list
/// is the pure, testable core.) All paths are relative.
pub fn create_folders(project: &CoboltProject, kind: FileKind) -> Vec<String> {
    let cat = Category::of_kind(kind);
    let mut set: BTreeSet<String> = BTreeSet::new();
    set.insert(cat.root_subdir().to_string());
    for rel in project.files_in(cat) {
        let rel = rel.replace('\\', "/");
        if let Some(idx) = rel.rfind('/') {
            let mut dir = rel[..idx].to_string();
            loop {
                set.insert(dir.clone());
                match dir.rfind('/') {
                    Some(i) => dir.truncate(i),
                    None => break,
                }
            }
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::Category;

    fn project() -> CoboltProject {
        let mut p = CoboltProject::new("T", "src/main.cbl");
        p.add_file_to("forms/customers/order.cfrm", Category::Forms);
        p.add_file_to("forms/suppliers/order.cfrm", Category::Forms);
        p.add_file_to("forms/login.cfrm", Category::Forms);
        p.add_file_to("src/util.cbl", Category::CommonCode);
        p
    }

    #[test]
    fn edit_candidates_match_stem_case_insensitively_within_category() {
        let p = project();
        // Two "order" forms across folders → ambiguous.
        let two = edit_candidates(&p, FileKind::Form, "order");
        assert_eq!(two.len(), 2);
        assert!(two.contains(&"forms/customers/order.cfrm".to_string()));
        assert!(two.contains(&"forms/suppliers/order.cfrm".to_string()));
        // Case-insensitive, and an extension on the query is ignored.
        assert_eq!(edit_candidates(&p, FileKind::Form, "ORDER.cfrm").len(), 2);
        // A single match, and a non-match.
        assert_eq!(edit_candidates(&p, FileKind::Form, "login"), vec!["forms/login.cfrm".to_string()]);
        assert!(edit_candidates(&p, FileKind::Form, "missing").is_empty());
        // Same-named element in a different category does not count.
        assert!(edit_candidates(&p, FileKind::Source, "order").is_empty());
    }

    #[test]
    fn edit_request_only_when_more_than_one() {
        let p = project();
        assert!(edit_request(&p, FileKind::Form, "order").is_some()); // 2 → modal
        assert!(edit_request(&p, FileKind::Form, "login").is_none()); // 1 → silent
        assert!(edit_request(&p, FileKind::Form, "missing").is_none()); // 0 → caller errors
    }

    #[test]
    fn create_request_is_always_a_folder_pick() {
        let r = create_request(FileKind::Form, "invoice");
        assert_eq!(r.op, TargetOp::Create);
        assert!(r.candidates.is_empty());
    }

    #[test]
    fn create_folders_lists_category_folders_relative() {
        let p = project();
        let folders = create_folders(&p, FileKind::Form);
        assert_eq!(
            folders,
            vec![
                "forms".to_string(),
                "forms/customers".to_string(),
                "forms/suppliers".to_string(),
            ]
        );
        assert!(folders.iter().all(|f| !Path::new(f).is_absolute()));
    }
}
