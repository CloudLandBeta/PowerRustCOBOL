// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 037 R3 — the exactly-one main form invariant.
//!
//! The MainForm flag lives inside each `.cfrm` (`Form::main_form`), so the
//! invariant spans files: exactly one of the project's forms may carry it.
//! [`normalize_main_form`] repairs any violation deterministically — the
//! FIRST form in the project's `forms` list wins — and is run when a project
//! opens, after a form is created, and after a form is deleted. A repaired
//! file is saved immediately so every surface (tree crown, properties panel,
//! run host) reads the designation straight from the file, with no shadow
//! state to drift.

use std::path::{Path, PathBuf};

use cobolt_forms::{load_form, save_form};

/// What [`normalize_main_form`] did. `None`-like "nothing to do" is
/// represented by `Unchanged` so callers can log honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainFormOutcome {
    /// Exactly one holder already — nothing written.
    Unchanged {
        /// Project-relative path of the holder (when the project has forms).
        holder: Option<String>,
    },
    /// No form was marked: the first (loadable) form was assigned.
    Assigned { holder: String },
    /// More than one form was marked: the first kept it, the rest cleared.
    Trimmed { holder: String, cleared: usize },
}

impl MainFormOutcome {
    /// The project-relative path of the current holder, if any.
    pub fn holder(&self) -> Option<&str> {
        match self {
            MainFormOutcome::Unchanged { holder } => holder.as_deref(),
            MainFormOutcome::Assigned { holder } | MainFormOutcome::Trimmed { holder, .. } => {
                Some(holder)
            }
        }
    }
}

/// Enforce R3 over the project's forms (in list order). Unreadable forms are
/// treated as non-holders and never written; the "first form" is the first
/// loadable one. Returns the outcome, or an error string when a repair write
/// failed (the invariant is then retried on the next normalisation point).
pub fn normalize_main_form(
    project_dir: &Path,
    forms_rel: &[String],
) -> Result<MainFormOutcome, String> {
    // (rel, abs, form) for every loadable form, in project order.
    let mut loaded: Vec<(String, PathBuf, cobolt_forms::Form)> = Vec::new();
    for rel in forms_rel {
        let abs = project_dir.join(rel);
        if let Ok(form) = load_form(&abs) {
            loaded.push((rel.clone(), abs, form));
        }
    }
    if loaded.is_empty() {
        return Ok(MainFormOutcome::Unchanged { holder: None });
    }

    let holders: Vec<usize> = loaded
        .iter()
        .enumerate()
        .filter(|(_, (_, _, f))| f.main_form)
        .map(|(i, _)| i)
        .collect();

    match holders.len() {
        1 => Ok(MainFormOutcome::Unchanged {
            holder: Some(loaded[holders[0]].0.clone()),
        }),
        0 => {
            let (rel, abs, form) = &mut loaded[0];
            form.main_form = true;
            save_form(form, abs).map_err(|e| format!("assign main form to {rel}: {e}"))?;
            Ok(MainFormOutcome::Assigned {
                holder: rel.clone(),
            })
        }
        _ => {
            let keep = holders[0];
            let mut cleared = 0usize;
            for &i in &holders[1..] {
                let (rel, abs, form) = &mut loaded[i];
                form.main_form = false;
                save_form(form, abs).map_err(|e| format!("clear main form on {rel}: {e}"))?;
                cleared += 1;
            }
            Ok(MainFormOutcome::Trimmed {
                holder: loaded[keep].0.clone(),
                cleared,
            })
        }
    }
}

/// R2 file-level settlement of a MainForm claim: clear the flag in every
/// OTHER form file that carries it (skipping paths in `skip_abs` — forms open
/// in a designer are settled in memory by the caller instead, so their
/// unsaved edits are not committed as a side effect). Returns the
/// project-relative paths whose files were rewritten.
pub fn clear_other_holders_on_disk(
    project_dir: &Path,
    forms_rel: &[String],
    new_holder_rel: &str,
    skip_abs: &[PathBuf],
) -> Result<Vec<String>, String> {
    let mut cleared = Vec::new();
    for rel in forms_rel {
        if rel == new_holder_rel {
            continue;
        }
        let abs = project_dir.join(rel);
        if skip_abs.iter().any(|p| p == &abs) {
            continue;
        }
        let Ok(mut form) = load_form(&abs) else {
            continue;
        };
        if form.main_form {
            form.main_form = false;
            save_form(&form, &abs).map_err(|e| format!("clear main form on {rel}: {e}"))?;
            cleared.push(rel.clone());
        }
    }
    Ok(cleared)
}

/// R2 file-level restore for an un-claim (undo): mark `holder_rel` main again
/// on disk (unless its path is in `skip_abs` — then the caller restores the
/// open designer in memory).
pub fn restore_holder_on_disk(
    project_dir: &Path,
    holder_rel: &str,
    skip_abs: &[PathBuf],
) -> Result<bool, String> {
    let abs = project_dir.join(holder_rel);
    if skip_abs.iter().any(|p| p == &abs) {
        return Ok(false);
    }
    let mut form =
        load_form(&abs).map_err(|e| format!("restore main form {holder_rel}: {e}"))?;
    if !form.main_form {
        form.main_form = true;
        save_form(&form, &abs).map_err(|e| format!("restore main form {holder_rel}: {e}"))?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::Form;

    /// Scratch project with the given forms; returns (dir, rel paths).
    fn project_with(forms: &[(&str, bool)]) -> (PathBuf, Vec<String>) {
        let dir = std::env::temp_dir().join(format!(
            "cobolt_main_form_{}_{}",
            std::process::id(),
            forms
                .iter()
                .map(|(n, m)| format!("{n}{}", *m as u8))
                .collect::<String>()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).expect("mkdir");
        let mut rels = Vec::new();
        for (name, main) in forms {
            let mut form = Form::new(name.to_uppercase(), *name, 640, 480);
            form.main_form = *main;
            let rel = format!("forms/{name}.cfrm");
            save_form(&form, &dir.join(&rel)).expect("save");
            rels.push(rel);
        }
        (dir, rels)
    }

    fn holder_flags(dir: &Path, rels: &[String]) -> Vec<bool> {
        rels.iter()
            .map(|rel| load_form(&dir.join(rel)).expect("load").main_form)
            .collect()
    }

    /// 037 R2/AC1 file half — claiming beta clears alpha on disk (beta's own
    /// file is the claiming designer's to save), and the undo path restores
    /// alpha; open-designer paths are skipped so unsaved edits are never
    /// committed as a side effect.
    #[test]
    fn main_form_reassign_files_clear_and_restore() {
        let (dir, rels) = project_with(&[("alpha", true), ("beta", false)]);
        let cleared =
            clear_other_holders_on_disk(&dir, &rels, "forms/beta.cfrm", &[]).expect("clear");
        assert_eq!(cleared, vec!["forms/alpha.cfrm".to_string()]);
        assert_eq!(holder_flags(&dir, &rels), [false, false]);

        // Undo: restore alpha.
        assert!(restore_holder_on_disk(&dir, "forms/alpha.cfrm", &[]).expect("restore"));
        assert_eq!(holder_flags(&dir, &rels), [true, false]);

        // A path listed as open must be left alone (settled in memory).
        let alpha_abs = dir.join("forms/alpha.cfrm");
        let cleared =
            clear_other_holders_on_disk(&dir, &rels, "forms/beta.cfrm", &[alpha_abs.clone()])
                .expect("clear with skip");
        assert!(cleared.is_empty(), "open designer's file untouched");
        assert!(
            !restore_holder_on_disk(&dir, "forms/alpha.cfrm", &[alpha_abs]).expect("skip"),
            "restore skips open designer paths"
        );
        println!("file settlement: alpha cleared+restored; open-designer paths skipped");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_main_form_zero_marked_assigns_first_in_list() {
        let (dir, rels) = project_with(&[("alpha", false), ("beta", false), ("gamma", false)]);
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(
            outcome,
            MainFormOutcome::Assigned {
                holder: "forms/alpha.cfrm".into()
            }
        );
        assert_eq!(holder_flags(&dir, &rels), [true, false, false]);
        println!("zero-marked → holder {:?}", outcome.holder());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_main_form_two_marked_keeps_first_clears_rest() {
        let (dir, rels) = project_with(&[("alpha", false), ("beta", true), ("gamma", true)]);
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(
            outcome,
            MainFormOutcome::Trimmed {
                holder: "forms/beta.cfrm".into(),
                cleared: 1
            }
        );
        assert_eq!(holder_flags(&dir, &rels), [false, true, false]);
        println!("two-marked → holder {:?} (1 cleared)", outcome.holder());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_main_form_single_holder_untouched() {
        let (dir, rels) = project_with(&[("alpha", false), ("beta", true)]);
        // Byte-stability: a healthy project must not be rewritten.
        let before = std::fs::read_to_string(dir.join(&rels[1])).expect("read");
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(
            outcome,
            MainFormOutcome::Unchanged {
                holder: Some("forms/beta.cfrm".into())
            }
        );
        let after = std::fs::read_to_string(dir.join(&rels[1])).expect("read");
        assert_eq!(before, after, "healthy project rewritten");
        println!("single holder untouched: {:?}", outcome.holder());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_main_form_first_created_becomes_main_and_delete_reassigns() {
        // First form created (the only form) becomes main …
        let (dir, mut rels) = project_with(&[("first", false)]);
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(outcome.holder(), Some("forms/first.cfrm"));

        // … a second form does NOT steal the role …
        let mut second = Form::new("SECOND", "second", 640, 480);
        second.main_form = false;
        save_form(&second, &dir.join("forms/second.cfrm")).expect("save");
        rels.push("forms/second.cfrm".into());
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(outcome.holder(), Some("forms/first.cfrm"));

        // … and deleting the holder auto-assigns the first remaining (R3 /
        // spec Q1 proposal).
        std::fs::remove_file(dir.join("forms/first.cfrm")).expect("rm");
        rels.remove(0);
        let outcome = normalize_main_form(&dir, &rels).expect("normalize");
        assert_eq!(
            outcome,
            MainFormOutcome::Assigned {
                holder: "forms/second.cfrm".into()
            }
        );
        println!("delete-main → holder {:?}", outcome.holder());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
