// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project-relative path storage for assign paths.

use std::path::{Component, Path, PathBuf};

/// Store a path for `.cidx`: project-relative when under `project_root`, absolute otherwise.
pub fn store_path(project_root: &Path, abs: &Path) -> String {
    if let Ok(rel) = abs.strip_prefix(project_root) {
        rel.components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                Component::CurDir => Some("."),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    } else {
        abs.display().to_string()
    }
}

/// Resolve a stored path against the project root (absolute paths pass through).
pub fn resolve_path(project_root: &Path, stored: &str) -> PathBuf {
    let p = Path::new(stored);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_root.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relativize_under_root() {
        let root = Path::new("/proj");
        let abs = Path::new("/proj/data/customers.idx");
        assert_eq!(store_path(root, abs), "data/customers.idx");
        assert_eq!(
            resolve_path(root, "data/customers.idx"),
            PathBuf::from("/proj/data/customers.idx")
        );
    }

    #[test]
    fn absolute_when_outside() {
        let root = Path::new("/proj");
        let abs = Path::new("/other/customers.idx");
        assert_eq!(store_path(root, abs), "/other/customers.idx");
        assert_eq!(resolve_path(root, "/other/customers.idx"), PathBuf::from("/other/customers.idx"));
    }
}