// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Only the MAIN form starts an application.**
//!
//! The IDE may run any form — that is what a designer is for. A *runtime*
//! may not: `rcrun` and a compiled binary always start the project's main
//! form, because that form is where an application puts the door. If a
//! sign-on form is the main form, letting anyone start the third form
//! directly would walk straight past it.
//!
//! # What is checked
//!
//! The main-form designation is recorded **twice**, in two files that a
//! tamperer has to keep in agreement:
//!
//! - inside the form itself — `Form::main_form`, the `main-form="true"`
//!   attribute of exactly one `.cfrm` (spec 037 R3); and
//! - in the project manifest — `[forms] main-form`, plus `main-form-seal`,
//!   a keyed digest over the designation and the project's form list.
//!
//! Any disagreement is **corruption**: the runtime says so and stops. It does
//! not repair the project the way the IDE does ([`crate::…`] has no business
//! rewriting an application it was merely asked to run), and it never guesses.
//!
//! # What this is not
//!
//! The seal is **tamper-evidence, not tamper-proofing**. Its key is a
//! constant in this file: anyone holding the source, or a copy of `rcrun`,
//! can recompute it. It catches an edited project — a flag moved to another
//! form, a manifest pointed at a different form, a seal left behind — which
//! is what the rule is for. It does not, and cannot, stop someone who owns
//! the machine, the project folder and the tools from rebuilding the
//! application as they please. A *compiled binary* is the strong case: its
//! forms are embedded in the executable and its main form is baked in at
//! build time, with nothing on disk left to edit.

use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Domain separator + key for the designation seal.
///
/// Deliberately a constant, and deliberately documented as one: see the
/// module note above. It exists so that a hand-edited manifest does not
/// happen to still verify, not to keep a secret.
const SEAL_KEY: &[u8] = b"PowerRustCOBOL/main-form-seal/v1";

/// The manifest subset this module reads. Separate from the build's own
/// mirror so that adding a build field cannot change what the guard accepts.
#[derive(Deserialize)]
struct GuardManifest {
    project: GuardMeta,
    #[serde(default)]
    files: GuardFiles,
    #[serde(default)]
    forms: GuardForms,
}

#[derive(Deserialize)]
struct GuardMeta {
    name: String,
}

#[derive(Deserialize, Default)]
struct GuardFiles {
    #[serde(default)]
    forms: Vec<String>,
}

#[derive(Deserialize, Default)]
struct GuardForms {
    /// The main form's id, as the IDE last wrote it. Empty in a project that
    /// predates the seal.
    #[serde(default, rename = "main-form")]
    main_form: String,
    /// The keyed digest over the designation. Empty ⇒ unsealed project.
    #[serde(default, rename = "main-form-seal")]
    main_form_seal: String,
}

/// A form's id: the uppercased stem of its `.cfrm`, which is also how COBOL
/// names it and how the build keys the embedded form table.
pub fn form_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase()
}

/// Which form a project starts, and the form list that designation was made
/// against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Designation {
    /// Id of the form that carries `main-form="true"` — or, in a project that
    /// predates the marker, the first form in the manifest's list.
    pub main_form_id: String,
    /// Every form id the manifest tracks, in manifest order.
    pub form_ids: Vec<String>,
}

/// The seal for a designation. Same inputs ⇒ same seal, on every platform.
pub fn seal(project_name: &str, main_form_id: &str, form_ids: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(SEAL_KEY);
    h.update(b"\nproject=");
    h.update(project_name.trim().as_bytes());
    h.update(b"\nmain=");
    h.update(main_form_id.trim().to_ascii_uppercase().as_bytes());
    for id in form_ids {
        h.update(b"\nform=");
        h.update(id.trim().to_ascii_uppercase().as_bytes());
    }
    let digest = h.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Read the designation straight from the files: which of `forms_rel` carries
/// the mark, over the list the manifest tracks.
///
/// `Err` is corruption — more than one form claims to be main. `Ok(None)`
/// means the project tracks no (loadable) form at all, so there is nothing to
/// designate.
pub fn read_designation(
    project_dir: &Path,
    forms_rel: &[String],
) -> Result<Option<Designation>, String> {
    let mut ids = Vec::with_capacity(forms_rel.len());
    let mut holders = Vec::new();
    for rel in forms_rel {
        let id = form_id(Path::new(rel));
        if id.is_empty() {
            continue;
        }
        let abs = project_dir.join(rel);
        // A form that is listed but missing or unreadable cannot claim the
        // mark; it still counts as one of the project's forms, so that
        // deleting a `.cfrm` from the folder does not silently re-seal.
        let claims = std::fs::read_to_string(&abs)
            .ok()
            .and_then(|xml| cobolt_forms::load_form_from_str(&xml).ok())
            .map(|f| f.main_form)
            .unwrap_or(false);
        if claims {
            holders.push(id.clone());
        }
        ids.push(id);
    }
    if ids.is_empty() {
        return Ok(None);
    }
    if holders.len() > 1 {
        return Err(format!(
            "{} forms are marked as the main form ({}); exactly one may be",
            holders.len(),
            holders.join(", ")
        ));
    }
    // No marker at all: a project written before spec 037 starts its first
    // form, which is what the build has always compiled as the entry point.
    let main_form_id = holders.into_iter().next().unwrap_or_else(|| ids[0].clone());
    Ok(Some(Designation {
        main_form_id,
        form_ids: ids,
    }))
}

/// What a runtime is allowed to do with a start request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartVerdict {
    /// Start it.
    Allowed,
    /// Start it, but the project carries no seal to verify — say so once.
    AllowedUnsealed,
    /// A form that is not this application's main form. Not an error in the
    /// project; an attempt to enter by the wrong door.
    Refused { requested: String, main: String },
    /// The project's own records disagree with each other. Report and stop.
    Corrupt(String),
}

/// May a runtime start `cfrm` (with the program at `cbl`)?
///
/// A form that belongs to no project — no manifest at or above it — is not an
/// application, and is allowed: there is no main form for it to bypass.
/// Everything else is judged against the project's designation.
pub fn authorize_form_start(cfrm: &Path, cbl: Option<&Path>) -> StartVerdict {
    let Some(manifest) = crate::find_project_manifest(cfrm) else {
        return StartVerdict::Allowed;
    };
    let Some(dir) = manifest.parent() else {
        return StartVerdict::Corrupt(format!(
            "the project file {} has no folder",
            manifest.display()
        ));
    };
    let text = match std::fs::read_to_string(&manifest) {
        Ok(t) => t,
        Err(e) => {
            return StartVerdict::Corrupt(format!(
                "the project file {} cannot be read ({e})",
                manifest.display()
            ))
        }
    };
    let proj: GuardManifest = match toml::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            return StartVerdict::Corrupt(format!(
                "the project file {} cannot be parsed ({e})",
                manifest.display()
            ))
        }
    };

    let designation = match read_designation(dir, &proj.files.forms) {
        Ok(Some(d)) => d,
        // The project tracks no forms, so this one is not its main form.
        Ok(None) => {
            return StartVerdict::Refused {
                requested: form_id(cfrm),
                main: String::new(),
            }
        }
        Err(e) => return StartVerdict::Corrupt(e),
    };

    // Record 1 vs record 2: the manifest's declaration against the marks in
    // the form files.
    let declared = proj.forms.main_form.trim().to_ascii_uppercase();
    if !declared.is_empty() && declared != designation.main_form_id {
        return StartVerdict::Corrupt(format!(
            "the project file names {declared} as the main form, but {} carries the mark",
            designation.main_form_id
        ));
    }

    // The seal over both. A project that predates it has neither field; one
    // that has a declaration but no seal has had the seal removed.
    let stored_seal = proj.forms.main_form_seal.trim();
    let sealed = if stored_seal.is_empty() {
        if !declared.is_empty() {
            return StartVerdict::Corrupt(
                "the project file declares a main form but carries no seal".to_owned(),
            );
        }
        false
    } else {
        let expect = seal(
            &proj.project.name,
            &designation.main_form_id,
            &designation.form_ids,
        );
        if expect != stored_seal {
            return StartVerdict::Corrupt(
                "the main-form seal does not match this project's forms".to_owned(),
            );
        }
        true
    };

    let requested = form_id(cfrm);
    if requested != designation.main_form_id {
        return StartVerdict::Refused {
            requested,
            main: designation.main_form_id,
        };
    }

    // The main form's layout paired with somebody else's program is the same
    // bypass wearing the right hat. Checked only when both paths resolve —
    // an unresolvable path is not evidence of tampering.
    if let Some(cbl) = cbl {
        if let Some(expected) = crate::form_program_path(cfrm, &designation.main_form_id) {
            let same = match (
                std::fs::canonicalize(cbl),
                std::fs::canonicalize(&expected),
            ) {
                (Ok(a), Ok(b)) => Some(a == b),
                _ => None,
            };
            if same == Some(false) {
                return StartVerdict::Corrupt(format!(
                    "form {} must run its own program ({}), not {}",
                    designation.main_form_id,
                    expected.display(),
                    cbl.display()
                ));
            }
        }
    }

    if sealed {
        StartVerdict::Allowed
    } else {
        StartVerdict::AllowedUnsealed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A project folder with `n` forms, the one at `main_at` marked.
    /// `sealed` writes the manifest's declaration + seal.
    fn project(dir: &Path, ids: &[&str], main_at: Option<usize>, sealed: bool) -> PathBuf {
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        let mut rels = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let mut form = cobolt_forms::Form::new(*id, *id, 800, 600);
            form.main_form = main_at == Some(i);
            let rel = format!("forms/{id}.cfrm");
            cobolt_forms::save_form(&form, &dir.join(&rel)).unwrap();
            rels.push(rel);
        }
        let mut toml = format!(
            "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\nmain = \"\"\n\n\
             [files]\nforms = [{}]\n",
            rels.iter()
                .map(|r| format!("\"{r}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        if sealed {
            let d = read_designation(dir, &rels).unwrap().unwrap();
            toml.push_str(&format!(
                "\n[forms]\nmain-form = \"{}\"\nmain-form-seal = \"{}\"\n",
                d.main_form_id,
                seal("Demo", &d.main_form_id, &d.form_ids)
            ));
        }
        let manifest = dir.join("Demo.project.toml");
        std::fs::write(&manifest, toml).unwrap();
        manifest
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-guard-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_main_form_starts() {
        let dir = tmp("main-starts");
        project(&dir, &["SIGNON", "MENU"], Some(0), true);
        let v = authorize_form_start(&dir.join("forms/SIGNON.cfrm"), None);
        assert_eq!(v, StartVerdict::Allowed, "the marked form is the door");
    }

    #[test]
    fn another_form_is_refused() {
        let dir = tmp("other-refused");
        project(&dir, &["SIGNON", "MENU"], Some(0), true);
        let v = authorize_form_start(&dir.join("forms/MENU.cfrm"), None);
        assert_eq!(
            v,
            StartVerdict::Refused {
                requested: "MENU".into(),
                main: "SIGNON".into()
            },
            "starting the second form would walk past SIGNON"
        );
    }

    #[test]
    fn moving_the_mark_is_corruption() {
        let dir = tmp("mark-moved");
        project(&dir, &["SIGNON", "MENU"], Some(0), true);
        // Tamper: the mark moves to MENU, consistently, in the form files.
        for (id, main) in [("SIGNON", false), ("MENU", true)] {
            let p = dir.join(format!("forms/{id}.cfrm"));
            let mut f = cobolt_forms::load_form(&p).unwrap();
            f.main_form = main;
            cobolt_forms::save_form(&f, &p).unwrap();
        }
        let v = authorize_form_start(&dir.join("forms/MENU.cfrm"), None);
        assert!(
            matches!(v, StartVerdict::Corrupt(_)),
            "the seal still names SIGNON: {v:?}"
        );
    }

    #[test]
    fn two_marked_forms_are_corruption() {
        let dir = tmp("two-marks");
        let rels = ["forms/A.cfrm".to_owned(), "forms/B.cfrm".to_owned()];
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        for id in ["A", "B"] {
            let mut form = cobolt_forms::Form::new(id, id, 800, 600);
            form.main_form = true;
            cobolt_forms::save_form(&form, &dir.join(format!("forms/{id}.cfrm"))).unwrap();
        }
        let e = read_designation(&dir, &rels).unwrap_err();
        assert!(e.contains("A") && e.contains("B"), "names both: {e}");
    }

    #[test]
    fn a_removed_seal_is_corruption() {
        let dir = tmp("seal-removed");
        let manifest = project(&dir, &["SIGNON", "MENU"], Some(0), true);
        let text = std::fs::read_to_string(&manifest).unwrap();
        let stripped: String = text
            .lines()
            .filter(|l| !l.starts_with("main-form-seal"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&manifest, stripped).unwrap();
        let v = authorize_form_start(&dir.join("forms/SIGNON.cfrm"), None);
        assert!(
            matches!(v, StartVerdict::Corrupt(_)),
            "a declaration with no seal is a seal that was taken off: {v:?}"
        );
    }

    #[test]
    fn a_project_that_predates_the_seal_still_runs_its_first_form() {
        let dir = tmp("legacy");
        project(&dir, &["SIGNON", "MENU"], None, false);
        assert_eq!(
            authorize_form_start(&dir.join("forms/SIGNON.cfrm"), None),
            StartVerdict::AllowedUnsealed,
            "no marker anywhere ⇒ the first form is the entry point, as the build has always \
             compiled it"
        );
        assert!(matches!(
            authorize_form_start(&dir.join("forms/MENU.cfrm"), None),
            StartVerdict::Refused { .. }
        ));
    }

    #[test]
    fn a_form_outside_any_project_is_not_an_application() {
        let dir = tmp("loose");
        let form = cobolt_forms::Form::new("LOOSE", "Loose", 400, 300);
        let p = dir.join("LOOSE.cfrm");
        cobolt_forms::save_form(&form, &p).unwrap();
        assert_eq!(authorize_form_start(&p, None), StartVerdict::Allowed);
    }

    #[test]
    fn the_seal_moves_with_the_designation() {
        let forms = vec!["SIGNON".to_owned(), "MENU".to_owned()];
        let a = seal("Demo", "SIGNON", &forms);
        let b = seal("Demo", "MENU", &forms);
        assert_ne!(a, b, "a different main form is a different seal");
        assert_eq!(a, seal("Demo", "signon", &forms), "id case is not a change");
        assert_ne!(
            a,
            seal("Other", "SIGNON", &forms),
            "the project name is part of it"
        );
    }
}
