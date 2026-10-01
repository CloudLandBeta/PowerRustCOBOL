// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Path confinement** (spec 080 R12, R13).
//!
//! Every path a tool is handed is project-relative, and every one goes through
//! [`ProjectRoot::resolve`] before it touches the disk. An absolute path, a
//! `..` component, a Windows drive or UNC prefix, and a symlink whose real
//! target leaves the project are all refused. A refusal names the rule it
//! broke — never the machine's own paths.

use std::path::{Component, Path, PathBuf};

/// One open project: its folder and its manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    /// The project folder, canonical.
    dir: PathBuf,
    /// The project manifest (`<Name>.project.toml`, or a legacy `cobolt.toml`).
    manifest: PathBuf,
}

impl ProjectRoot {
    /// Open the project whose manifest is `manifest_or_dir`, or which lives in
    /// the folder `manifest_or_dir`.
    ///
    /// A folder must hold the manifest **itself**: `find_project_manifest`
    /// walks upwards, and a project found in an ancestor is not the folder the
    /// caller named — so it is refused rather than silently adopted.
    pub fn open(manifest_or_dir: &Path) -> Result<Self, String> {
        let (dir, manifest) = if manifest_or_dir.is_dir() {
            let dir = canonical(manifest_or_dir)?;
            let manifest = cobolt_compiler::find_project_manifest(&dir)
                .ok_or_else(|| "no project file (*.project.toml) in that folder".to_owned())?;
            let found_in = manifest.parent().map(canonical).transpose()?;
            if found_in.as_deref() != Some(dir.as_path()) {
                return Err(
                    "that folder holds no project file of its own (the nearest one belongs to an enclosing folder)"
                        .to_owned(),
                );
            }
            (dir, manifest)
        } else if manifest_or_dir.is_file() {
            let manifest = canonical(manifest_or_dir)?;
            let dir = manifest
                .parent()
                .ok_or_else(|| "the project file has no folder".to_owned())?
                .to_path_buf();
            (dir, manifest)
        } else {
            return Err("no project file at that path".to_owned());
        };
        Ok(Self { dir, manifest })
    }

    /// The project folder (canonical).
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The project manifest.
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// The manifest's file name — what a tool may show without revealing
    /// where the project lives.
    pub fn manifest_name(&self) -> String {
        self.manifest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// A project-relative path resolved inside the project, or why not.
    ///
    /// `\` is read as a separator too, so `..\x` is the same escape as `../x`
    /// on every platform.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, String> {
        let rel = rel.trim();
        if rel.is_empty() {
            return Err("an empty path".to_owned());
        }
        let norm = rel.replace('\\', "/");
        let bytes = norm.as_bytes();
        if norm.starts_with('/')
            || (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic())
        {
            return Err(format!(
                "'{rel}' is absolute; paths are relative to the project folder"
            ));
        }
        for c in Path::new(&norm).components() {
            match c {
                Component::Normal(_) | Component::CurDir => {}
                Component::ParentDir => {
                    return Err(format!("'{rel}' climbs out of the project ('..' is refused)"))
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(format!(
                        "'{rel}' is absolute; paths are relative to the project folder"
                    ))
                }
            }
        }
        let joined = self.dir.join(&norm);
        // The deepest part of the path that exists, canonical: a symlink
        // anywhere along it is followed to where it really points.
        let mut probe = joined.as_path();
        let real = loop {
            if probe.exists() {
                break canonical(probe)?;
            }
            match probe.parent() {
                Some(p) => probe = p,
                None => break self.dir.clone(),
            }
        };
        if !real.starts_with(&self.dir) {
            return Err(format!(
                "'{rel}' leads outside the project (through a link)"
            ));
        }
        Ok(joined)
    }

    /// `abs` relative to the project folder, with `/` separators — or `None`
    /// when it is not inside.
    pub fn relative(&self, abs: &Path) -> Option<String> {
        let rel = abs.strip_prefix(&self.dir).ok()?;
        Some(
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }
}

fn canonical(p: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(p).map_err(|e| format!("cannot resolve a path ({e})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prc-080-root-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn project(dir: &Path) -> PathBuf {
        let m = dir.join("Demo.project.toml");
        std::fs::write(&m, "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\nmain = \"\"\n").unwrap();
        m
    }

    #[test]
    fn root_refuses_every_escape_and_resolves_inside() {
        let base = tmp("escape");
        let proj = base.join("proj");
        std::fs::create_dir_all(proj.join("forms")).unwrap();
        project(&proj);
        let outside = base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, proj.join("link-out")).unwrap();

        let root = ProjectRoot::open(&proj).expect("open by folder");
        let mut table = Vec::new();
        let refused = [
            "../x",
            "forms/../../x",
            "/etc/passwd",
            "C:\\x",
            "c:/x",
            "..\\x",
            "\\\\server\\share",
            "",
        ];
        for case in refused {
            let v = root.resolve(case);
            assert!(v.is_err(), "{case:?} must be refused, got {v:?}");
            table.push((case.to_owned(), "refused"));
        }
        #[cfg(unix)]
        {
            for case in ["link-out", "link-out/new.cfrm"] {
                assert!(root.resolve(case).is_err(), "{case} escapes through a link");
                table.push((case.to_owned(), "refused"));
            }
        }
        for case in ["forms/A.cfrm", "./generated/A.cbl", "docs\\compiler-requests\\r.md"] {
            let p = root.resolve(case).unwrap_or_else(|e| panic!("{case}: {e}"));
            assert!(p.starts_with(root.dir()));
            table.push((case.to_owned(), "resolved"));
        }
        for (c, v) in &table {
            println!("  {c:<28} {v}");
        }
        println!(
            "root: {} cases — {} refused, {} resolved",
            table.len(),
            table.iter().filter(|(_, v)| *v == "refused").count(),
            table.iter().filter(|(_, v)| *v == "resolved").count()
        );
    }

    #[test]
    fn root_refuses_an_ancestor_manifest() {
        let base = tmp("ancestor");
        project(&base);
        let sub = base.join("forms");
        std::fs::create_dir_all(&sub).unwrap();
        let err = ProjectRoot::open(&sub).unwrap_err();
        assert!(err.contains("enclosing"), "{err}");
        let by_manifest = ProjectRoot::open(&base.join("Demo.project.toml")).unwrap();
        assert_eq!(by_manifest.dir(), std::fs::canonicalize(&base).unwrap());
        assert!(ProjectRoot::open(&base.join("missing.project.toml")).is_err());
        println!("root: ancestor manifest refused, manifest path opened, missing path refused (3 cases)");
    }
}
