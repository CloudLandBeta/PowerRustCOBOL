// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A project's generated COBOL, regenerated from its definitions.**
//!
//! A form's `.cbl` and an indexed file's facade are build artefacts: every
//! line the developer wrote lives in the `.cfrm` (event handlers, procedures)
//! or the `.cidx`, and the generated file is rewritten whole from it. The IDE
//! regenerates them before every Build, Run and Debug; a build started
//! anywhere else must do the same, or a `.cfrm` edited outside the IDE ships
//! its OLD generated code. This module is that regeneration, with the one rule
//! for where each generated file lives, so the IDE and `rcrun build` write the
//! same files to the same places.

use std::path::Path;

/// The project-relative path of the generated program called `file_name`
/// (`<form stem>.cbl`, `<cidx stem>-indexed.cbl`): the tracked generated
/// entry with that file name — the developer may have moved it into a
/// subfolder (spec 033 R7) — else `generated/<file_name>`.
pub fn generated_rel(generated: &[String], file_name: &str) -> String {
    generated
        .iter()
        .find(|rel| Path::new(rel.as_str()).file_name().and_then(|n| n.to_str()) == Some(file_name))
        .cloned()
        .unwrap_or_else(|| format!("generated/{file_name}"))
}

/// Track `rel` as generated (read-only) code: listed under `generated`, and no
/// longer under the editable `sources` — the IDE's `add_generated`.
pub fn track_generated(generated: &mut Vec<String>, sources: &mut Vec<String>, rel: &str) {
    let rel = rel.replace('\\', "/");
    sources.retain(|f| f != &rel);
    if !generated.contains(&rel) {
        generated.push(rel);
    }
}

/// What [`regenerate_project`] did.
#[derive(Debug, Default)]
pub struct Regenerated {
    /// Project-relative paths of the programs written, forms first.
    pub written: Vec<String>,
    /// Definitions that could not be read, with the reason: their generated
    /// file is left as it was, as the IDE leaves it.
    pub skipped: Vec<(String, String)>,
}

/// Regenerate, from disk, the generated COBOL of every form (`forms`, `.cfrm`
/// paths) and every indexed file (`indexed`, `.cidx` paths) of the project in
/// `dir`, exactly as the IDE's Generate does: the same generator, written to
/// [`generated_rel`], and tracked with [`track_generated`] in the lists given.
/// An indexed file's `COPYBOOKS/<stem>.SEL` and `.FD` are rewritten too.
///
/// A definition that cannot be read is skipped and reported. A file that
/// cannot be written is an error: building on without it would ship the stale
/// code this exists to prevent.
pub fn regenerate_project(
    dir: &Path,
    forms: &[String],
    indexed: &[String],
    generated: &mut Vec<String>,
    sources: &mut Vec<String>,
) -> std::io::Result<Regenerated> {
    let mut done = Regenerated::default();
    let write = |rel: &str, text: &str| -> std::io::Result<()> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, text)
            .map_err(|e| std::io::Error::new(e.kind(), format!("cannot write {}: {e}", path.display())))
    };
    let stem = |rel: &str| Path::new(rel).file_stem().and_then(|s| s.to_str()).map(str::to_owned);

    for rel in forms {
        let Some(stem) = stem(rel) else { continue };
        let form = match cobolt_forms::load_form(&dir.join(rel)) {
            Ok(f) => f,
            Err(e) => {
                done.skipped.push((rel.clone(), e.to_string()));
                continue;
            }
        };
        let out = generated_rel(generated, &format!("{stem}.cbl"));
        write(&out, &crate::generate(&form))?;
        track_generated(generated, sources, &out);
        done.written.push(out);
    }

    for rel in indexed {
        let def = match cobolt_indexed::load_indexed(dir.join(rel)) {
            Ok(d) => d,
            Err(e) => {
                done.skipped.push((rel.clone(), e.to_string()));
                continue;
            }
        };
        let stem = stem(rel).unwrap_or_else(|| def.name.clone());
        let out = generated_rel(generated, &format!("{stem}-indexed.cbl"));
        write(&out, &crate::generate_indexed(&def))?;
        write(&format!("COPYBOOKS/{stem}.SEL"), &crate::generate_indexed_select(&def))?;
        write(&format!("COPYBOOKS/{stem}.FD"), &crate::generate_indexed_fd(&def))?;
        track_generated(generated, sources, &out);
        done.written.push(out);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("prc-regen-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Spec 080 F9 — a project's generated COBOL is rewritten from its
    /// definitions on disk, as the IDE's Generate writes it:
    ///   * `forms/main.cfrm` → `generated/main.cbl` (stale text replaced),
    ///     moved from `sources` to `generated` when it was tracked there;
    ///   * `forms/child.cfrm`, whose generated file was relocated to
    ///     `generated/screens/child.cbl`, is rewritten THERE;
    ///   * `indexed/cust.cidx` → `generated/cust-indexed.cbl` plus
    ///     `COPYBOOKS/cust.SEL` and `.FD`;
    ///   * `forms/broken.cfrm`, unreadable, is skipped and its generated file
    ///     left exactly as it was.
    #[test]
    fn a_projects_generated_cobol_is_rewritten_from_its_definitions() {
        let dir = temp_dir("project");
        fs::create_dir_all(dir.join("forms")).unwrap();
        fs::create_dir_all(dir.join("indexed")).unwrap();
        fs::create_dir_all(dir.join("generated/screens")).unwrap();
        let main = cobolt_forms::Form::new("MAIN", "Main", 400, 300);
        let child = cobolt_forms::Form::new("CHILD", "Child", 300, 200);
        cobolt_forms::save_form(&main, &dir.join("forms/main.cfrm")).unwrap();
        cobolt_forms::save_form(&child, &dir.join("forms/child.cfrm")).unwrap();
        fs::write(dir.join("forms/broken.cfrm"), "not a form").unwrap();
        let def = cobolt_indexed::IndexedDefinition::new("CUST", "cust.dat");
        cobolt_indexed::save_indexed(dir.join("indexed/cust.cidx"), &def).unwrap();
        for f in ["generated/main.cbl", "generated/screens/child.cbl", "generated/broken.cbl"] {
            fs::write(dir.join(f), "      * STALE\n").unwrap();
        }

        let forms = vec!["forms/main.cfrm".to_owned(), "forms/child.cfrm".to_owned(), "forms/broken.cfrm".to_owned()];
        let indexed = vec!["indexed/cust.cidx".to_owned()];
        let mut generated = vec!["generated/screens/child.cbl".to_owned(), "generated/broken.cbl".to_owned()];
        let mut sources = vec!["src/app.cbl".to_owned(), "generated/main.cbl".to_owned()];
        let done = regenerate_project(&dir, &forms, &indexed, &mut generated, &mut sources).unwrap();

        let read = |f: &str| fs::read_to_string(dir.join(f)).unwrap();
        assert_eq!(read("generated/main.cbl"), crate::generate(&main));
        assert_eq!(read("generated/screens/child.cbl"), crate::generate(&child));
        assert!(!dir.join("generated/child.cbl").exists(), "the relocated file is rewritten in place");
        assert_eq!(read("generated/broken.cbl"), "      * STALE\n", "an unreadable form's code is left alone");
        assert_eq!(read("generated/cust-indexed.cbl"), crate::generate_indexed(&def));
        assert_eq!(read("COPYBOOKS/cust.SEL"), crate::generate_indexed_select(&def));
        assert_eq!(read("COPYBOOKS/cust.FD"), crate::generate_indexed_fd(&def));
        assert_eq!(done.written, vec!["generated/main.cbl", "generated/screens/child.cbl", "generated/cust-indexed.cbl"]);
        assert_eq!(done.skipped.len(), 1);
        assert_eq!(done.skipped[0].0, "forms/broken.cfrm");
        assert_eq!(sources, vec!["src/app.cbl"], "generated code is no longer editable source");
        assert_eq!(
            generated,
            vec!["generated/screens/child.cbl", "generated/broken.cbl", "generated/main.cbl", "generated/cust-indexed.cbl"]
        );
        let _ = fs::remove_dir_all(&dir);
        println!("regenerated {:?}; skipped {:?}", done.written, done.skipped.iter().map(|s| &s.0).collect::<Vec<_>>());
    }

    /// The file rule: a tracked entry with the same file name, else
    /// `generated/`.
    #[test]
    fn the_generated_file_rule() {
        let tracked = vec!["generated/a/order.cbl".to_owned()];
        assert_eq!(generated_rel(&tracked, "order.cbl"), "generated/a/order.cbl");
        assert_eq!(generated_rel(&tracked, "other.cbl"), "generated/other.cbl");
    }
}
