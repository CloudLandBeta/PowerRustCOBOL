// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Project structure upgrades — offered, never imposed.**
//!
//! A project file has a *shape*, and the shape changes as PowerRustCOBOL
//! grows: a new record the runtime relies on, a folder that moved, a setting
//! that became two. `[project] structure` numbers that shape.
//! [`CURRENT_STRUCTURE`] is what this IDE writes; anything lower is a project
//! from an earlier one.
//!
//! The IDE **never rewrites an older project's shape on its own**. It opens the
//! file, notices what is missing, and offers the upgrade — the developer's
//! project, the developer's call. Declining costs nothing: everything keeps
//! working exactly as it did, which is the promise every upgrade here has to
//! keep.
//!
//! # Adding an upgrade
//!
//! 1. Add a `STRUCTURE_*` constant one above the last and raise
//!    [`CURRENT_STRUCTURE`] to it.
//! 2. Write a unit struct implementing [`ProjectUpgrade`]. `applies` decides
//!    from the project as it is on disk — not from the number alone, so a
//!    project that already has what the upgrade would give it is not offered
//!    busywork. `apply` makes the change and raises `structure`.
//! 3. Register it in [`UPGRADES`], in ascending order.
//! 4. Add its two `Tr` fields (title, detail) in all six languages.
//!
//! Upgrades run in order, each on the result of the last, and the whole run is
//! saved once. An upgrade that fails stops the run and reports; the project
//! file is written with whatever succeeded before it, so a partial run is
//! still a consistent project.

use std::path::Path;

use crate::i18n::Tr;
use crate::project_model::CoboltProject;

/// Project files that predate `[project] structure` — every project written
/// before upgrades existed. Not a real shape, just "older than the first one".
pub const STRUCTURE_LEGACY: u32 = 0;

/// The main-form designation is recorded in the project file and sealed, so a
/// runtime can tell the IDE's designation from a hand-edited one. Only the
/// main form starts an application; this is the record that says which.
pub const STRUCTURE_MAIN_FORM_SEAL: u32 = 1;

/// The shape this IDE writes. New projects are born here.
pub const CURRENT_STRUCTURE: u32 = STRUCTURE_MAIN_FORM_SEAL;

/// One step from an older project shape to a newer one.
pub trait ProjectUpgrade: Sync {
    /// Stable identifier — logs and tests, never shown to a developer.
    fn id(&self) -> &'static str;

    /// The structure number this upgrade produces.
    fn to_structure(&self) -> u32;

    /// One line naming what the upgrade does.
    fn title(&self, tr: &Tr) -> &'static str;

    /// What actually changes, and what it buys — the developer is deciding.
    fn detail(&self, tr: &Tr) -> &'static str;

    /// Is this project missing what the upgrade gives? Read the files if the
    /// number alone cannot tell.
    fn applies(&self, project: &CoboltProject, dir: &Path) -> bool;

    /// Make the change. The caller saves; do not write the project file here.
    fn apply(&self, project: &mut CoboltProject, dir: &Path) -> Result<(), String>;
}

/// Every upgrade, in ascending structure order.
pub static UPGRADES: &[&dyn ProjectUpgrade] = &[&MainFormSeal];

/// The upgrades this project has not had yet, in order. Empty means current.
pub fn pending(project: &CoboltProject, dir: &Path) -> Vec<&'static dyn ProjectUpgrade> {
    UPGRADES
        .iter()
        .copied()
        .filter(|u| u.applies(project, dir))
        .collect()
}

/// Apply `upgrades` in order, in memory. Returns the ids that ran; stops at
/// the first failure and reports it, with the successful ones already applied
/// to `project` so the caller can still save them.
pub fn apply_all(
    project: &mut CoboltProject,
    dir: &Path,
    upgrades: &[&'static dyn ProjectUpgrade],
) -> (Vec<&'static str>, Option<String>) {
    let mut done = Vec::new();
    for u in upgrades {
        match u.apply(project, dir) {
            Ok(()) => done.push(u.id()),
            Err(e) => return (done, Some(format!("{}: {e}", u.id()))),
        }
    }
    (done, None)
}

// ── 1 — the main-form designation, recorded and sealed ───────────────────────

struct MainFormSeal;

impl ProjectUpgrade for MainFormSeal {
    fn id(&self) -> &'static str {
        "main-form-seal"
    }

    fn to_structure(&self) -> u32 {
        STRUCTURE_MAIN_FORM_SEAL
    }

    fn title(&self, tr: &Tr) -> &'static str {
        tr.upgrade_main_form_seal
    }

    fn detail(&self, tr: &Tr) -> &'static str {
        tr.upgrade_main_form_seal_detail
    }

    fn applies(&self, project: &CoboltProject, dir: &Path) -> bool {
        if project.project.structure >= STRUCTURE_MAIN_FORM_SEAL {
            return false;
        }
        // Nothing to designate, nothing to seal: a project with no forms is
        // not an application with a front door. Its shape still moves up when
        // it gains one — `applies` is asked again on every open.
        if project.files.forms.is_empty() {
            return false;
        }
        // A project whose forms disagree about the mark cannot be sealed into
        // an answer. The IDE's own R3 repair settles that first; until then
        // this upgrade stays out of the way rather than freezing a guess.
        matches!(
            cobolt_compiler::main_form_guard::read_designation(dir, &project.files.forms),
            Ok(Some(_))
        )
    }

    fn apply(&self, project: &mut CoboltProject, dir: &Path) -> Result<(), String> {
        // Confirm the designation is still readable at the moment of the
        // change — `applies` ran when the project opened, and files move.
        cobolt_compiler::main_form_guard::read_designation(dir, &project.files.forms)?
            .ok_or_else(|| "this project tracks no forms to designate".to_owned())?;
        // The seal itself is written by `save_project`, from the files, for
        // every project at this structure or above. Raising the number is the
        // whole change: one writer of the record, and it is not this one.
        project.project.structure = STRUCTURE_MAIN_FORM_SEAL;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::{load_project, save_project};
    use cobolt_compiler::main_form_guard::{authorize_form_start, StartVerdict};

    fn legacy_project(dir: &Path) -> (CoboltProject, std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        for (id, main) in [("SIGNON", true), ("MENU", false)] {
            let mut form = cobolt_forms::Form::new(id, id, 640, 480);
            form.main_form = main;
            cobolt_forms::save_form(&form, &dir.join(format!("forms/{id}.cfrm"))).unwrap();
        }
        let mut proj = CoboltProject::new("Legacy", "src/main.cbl");
        // What an older PowerRustCOBOL wrote: forms, and no structure number.
        proj.project.structure = STRUCTURE_LEGACY;
        proj.files.forms = vec!["forms/SIGNON.cfrm".into(), "forms/MENU.cfrm".into()];
        let path = dir.join("cobolt.toml");
        save_project(&proj, &path).unwrap();
        (proj, path)
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("prc-upgrade-{name}"))
    }

    /// An older project is left exactly as it was until the developer says
    /// otherwise — saving it must not quietly change its shape.
    #[test]
    fn an_older_project_is_not_upgraded_behind_the_developers_back() {
        let dir = tmp("untouched");
        let (_, path) = legacy_project(&dir);

        let saved = load_project(&path).unwrap();
        assert_eq!(saved.project.structure, STRUCTURE_LEGACY);
        assert!(
            saved.forms.main_form.is_empty() && saved.forms.main_form_seal.is_empty(),
            "a save must not seal a project of an older shape"
        );
        // And it still runs, unsealed, exactly as before.
        assert_eq!(
            authorize_form_start(&dir.join("forms/SIGNON.cfrm"), None),
            StartVerdict::AllowedUnsealed
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// …and when they do say otherwise, one offer covers it.
    #[test]
    fn the_offer_appears_and_the_upgrade_seals_the_project() {
        let dir = tmp("accepted");
        let (mut proj, path) = legacy_project(&dir);

        let todo = pending(&proj, &dir);
        assert_eq!(todo.len(), 1, "one upgrade is due");
        assert_eq!(todo[0].id(), "main-form-seal");

        let (done, err) = apply_all(&mut proj, &dir, &todo);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(done, vec!["main-form-seal"]);
        save_project(&proj, &path).unwrap();

        let saved = load_project(&path).unwrap();
        assert_eq!(saved.project.structure, CURRENT_STRUCTURE);
        assert_eq!(saved.forms.main_form, "SIGNON");
        assert!(!saved.forms.main_form_seal.is_empty());
        assert_eq!(
            authorize_form_start(&dir.join("forms/SIGNON.cfrm"), None),
            StartVerdict::Allowed,
            "the runtime can now verify the designation"
        );
        assert!(
            pending(&saved, &dir).is_empty(),
            "and it is never offered again"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A project with nothing to designate is not handed busywork.
    #[test]
    fn a_project_with_no_forms_is_not_offered_the_seal() {
        let dir = tmp("no-forms");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut proj = CoboltProject::new("Console", "src/main.cbl");
        proj.project.structure = STRUCTURE_LEGACY;
        assert!(pending(&proj, &dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ambiguity is the IDE's R3 repair to settle, not an upgrade's to freeze.
    #[test]
    fn a_project_whose_forms_disagree_is_not_offered_the_seal() {
        let dir = tmp("ambiguous");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        for id in ["A", "B"] {
            let mut form = cobolt_forms::Form::new(id, id, 640, 480);
            form.main_form = true;
            cobolt_forms::save_form(&form, &dir.join(format!("forms/{id}.cfrm"))).unwrap();
        }
        let mut proj = CoboltProject::new("Ambiguous", "src/main.cbl");
        proj.project.structure = STRUCTURE_LEGACY;
        proj.files.forms = vec!["forms/A.cfrm".into(), "forms/B.cfrm".into()];
        assert!(pending(&proj, &dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A new project is born current, so the dialog never greets a project
    /// this IDE just created.
    #[test]
    fn a_new_project_is_born_current() {
        let proj = CoboltProject::new("Fresh", "src/main.cbl");
        assert_eq!(proj.project.structure, CURRENT_STRUCTURE);
    }

    /// The registry is ordered and each entry announces the number it reaches;
    /// a new upgrade added out of order would apply before its predecessor.
    #[test]
    fn the_registry_is_in_ascending_order() {
        let mut last = STRUCTURE_LEGACY;
        for u in UPGRADES {
            assert!(
                u.to_structure() > last,
                "{} is out of order (reaches {}, after {last})",
                u.id(),
                u.to_structure()
            );
            last = u.to_structure();
        }
        assert_eq!(last, CURRENT_STRUCTURE, "the last upgrade reaches current");
    }
}
