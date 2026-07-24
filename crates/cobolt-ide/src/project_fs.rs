// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Path-safe, project-relative folder & file operations for the project tree.
//!
//! Every operation is confined to the project root and works purely with
//! **project-relative** paths — no absolute or user-home path is ever produced
//! or stored (spec 033, R21). Validation rejects traversal (`..`), absolute or
//! rooted paths, hidden/leading-dot names, path separators inside a name, and
//! collisions with an existing sibling. Category-root subdirectories
//! (`forms/`, `src/`, `indexed/`, `generated/`, `Assets/`, `Knowledge Base/`)
//! are protected from rename/delete (R8).
//!
//! Errors are returned as a typed [`FolderOpError`] so the caller can localise
//! the message via the `Tr` table (R19); [`FolderOpError::default_message`]
//! gives a plain-English fallback for logs and tests.

use std::path::{Component, Path, PathBuf};

/// A typed failure from a folder/file operation. The IDE maps each variant to a
/// localised `Tr` string; `default_message` is the English fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderOpError {
    /// The folder name was empty or whitespace-only.
    EmptyName,
    /// The name began with `.` (hidden / dotfile).
    DottedName,
    /// The name was not exactly one path component.
    NotSingleComponent,
    /// The name contained a path separator or a control character.
    IllegalChar,
    /// The relative path contained `..` or an absolute/rooted prefix.
    Traversal,
    /// The target is a protected category-root subdirectory.
    IsCategoryRoot,
    /// The parent directory does not exist.
    ParentMissing,
    /// A file or folder with that name already exists (holds the relative path).
    Collision(String),
    /// A move would place a path inside itself or one of its descendants.
    SelfDescendant,
    /// Source and destination are identical (nothing to do).
    NoOp,
    /// The source path does not exist.
    SourceMissing,
    /// The destination is not an existing directory.
    DestNotFolder,
    /// An underlying I/O error (holds the OS message).
    Io(String),
}

impl FolderOpError {
    /// English fallback message. User-facing text goes through `Tr`; this is for
    /// logs and tests.
    pub fn default_message(&self) -> String {
        match self {
            FolderOpError::EmptyName => "Folder name cannot be empty.".into(),
            FolderOpError::DottedName => "Folder name cannot start with a dot.".into(),
            FolderOpError::NotSingleComponent => {
                "Folder name must be a single path component.".into()
            }
            FolderOpError::IllegalChar => {
                "Folder name cannot contain a path separator or control character.".into()
            }
            FolderOpError::Traversal => {
                "Path must not contain traversal (`..`) or a drive/root prefix.".into()
            }
            FolderOpError::IsCategoryRoot => {
                "A category's root folder cannot be renamed or deleted.".into()
            }
            FolderOpError::ParentMissing => "The parent folder does not exist.".into(),
            FolderOpError::Collision(p) => format!("Already exists: {p}"),
            FolderOpError::SelfDescendant => {
                "Cannot move a folder into itself or one of its subfolders.".into()
            }
            FolderOpError::NoOp => "The source and destination are the same.".into(),
            FolderOpError::SourceMissing => "The source no longer exists.".into(),
            FolderOpError::DestNotFolder => "The destination is not a folder.".into(),
            FolderOpError::Io(e) => format!("File system error: {e}"),
        }
    }
}

impl std::fmt::Display for FolderOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.default_message())
    }
}

impl std::error::Error for FolderOpError {}

type FsResult<T> = Result<T, FolderOpError>;

/// Normalise a project-relative path: strip `.` components, reject `..`, and
/// reject any absolute / rooted / prefixed path. Returns a clean relative path.
fn clean_rel(rel: &Path) -> FsResult<PathBuf> {
    let mut clean = PathBuf::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(FolderOpError::Traversal),
        }
    }
    Ok(clean)
}

/// Validate a single new folder name (one non-hidden path component, no
/// separators or control characters).
pub fn validate_folder_name(name: &str) -> FsResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FolderOpError::EmptyName);
    }
    if trimmed.starts_with('.') {
        return Err(FolderOpError::DottedName);
    }
    if trimmed.contains(['/', '\\']) || trimmed.chars().any(|c| c.is_control()) {
        return Err(FolderOpError::IllegalChar);
    }
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(FolderOpError::NotSingleComponent),
    }
}

/// True if `folder_rel` names a protected category-root subdirectory
/// (`category_root_rel` itself), which must not be renamed or deleted.
fn is_category_root(folder_rel: &Path, category_root_rel: &Path) -> FsResult<bool> {
    Ok(clean_rel(folder_rel)? == clean_rel(category_root_rel)?)
}

/// Create folder `name` inside the project-relative directory `parent_rel`.
/// Returns the new folder's **project-relative** path.
pub fn create_folder(project_root: &Path, parent_rel: &Path, name: &str) -> FsResult<PathBuf> {
    validate_folder_name(name)?;
    let parent = clean_rel(parent_rel)?;
    let absolute_parent = project_root.join(&parent);
    if !absolute_parent.is_dir() {
        return Err(FolderOpError::ParentMissing);
    }
    let relative = parent.join(name.trim());
    let absolute = project_root.join(&relative);
    if absolute.exists() {
        return Err(FolderOpError::Collision(rel_string(&relative)));
    }
    std::fs::create_dir(&absolute).map_err(|e| FolderOpError::Io(e.to_string()))?;
    Ok(relative)
}

/// Rename the project-relative folder `folder_rel` to `new_name` (kept in the
/// same parent). Returns the new **project-relative** path. Refuses to rename a
/// category root (R8).
pub fn rename_folder(
    project_root: &Path,
    folder_rel: &Path,
    new_name: &str,
    category_root_rel: &Path,
) -> FsResult<PathBuf> {
    validate_folder_name(new_name)?;
    let folder = clean_rel(folder_rel)?;
    if is_category_root(&folder, category_root_rel)? {
        return Err(FolderOpError::IsCategoryRoot);
    }
    let parent = folder.parent().ok_or(FolderOpError::ParentMissing)?;
    let new_relative = parent.join(new_name.trim());
    let absolute_old = project_root.join(&folder);
    if !absolute_old.is_dir() {
        return Err(FolderOpError::SourceMissing);
    }
    let absolute_new = project_root.join(&new_relative);
    if absolute_new.exists() {
        return Err(FolderOpError::Collision(rel_string(&new_relative)));
    }
    std::fs::rename(&absolute_old, &absolute_new).map_err(|e| FolderOpError::Io(e.to_string()))?;
    Ok(new_relative)
}

/// Recursively delete the project-relative folder `folder_rel` and everything in
/// it. Refuses to delete a category root (R8). Returns the cleaned relative path
/// that was removed.
pub fn delete_folder(
    project_root: &Path,
    folder_rel: &Path,
    category_root_rel: &Path,
) -> FsResult<PathBuf> {
    let folder = clean_rel(folder_rel)?;
    if is_category_root(&folder, category_root_rel)? {
        return Err(FolderOpError::IsCategoryRoot);
    }
    let absolute = project_root.join(&folder);
    if !absolute.is_dir() {
        return Err(FolderOpError::SourceMissing);
    }
    std::fs::remove_dir_all(&absolute).map_err(|e| FolderOpError::Io(e.to_string()))?;
    Ok(folder)
}

/// Move the project-relative path `src_rel` into the project-relative directory
/// `dest_dir_rel`. Returns the new **project-relative** path. Rejects a no-op, a
/// self/descendant move, and a name collision.
pub fn move_path(
    project_root: &Path,
    src_rel: &Path,
    dest_dir_rel: &Path,
) -> FsResult<PathBuf> {
    let src = clean_rel(src_rel)?;
    let dest_dir = clean_rel(dest_dir_rel)?;
    let name = src.file_name().ok_or(FolderOpError::SourceMissing)?;
    // A path cannot be moved into itself or one of its own descendants.
    if dest_dir == src || dest_dir.starts_with(&src) {
        return Err(FolderOpError::SelfDescendant);
    }
    let new_relative = dest_dir.join(name);
    if new_relative == src {
        return Err(FolderOpError::NoOp);
    }
    let absolute_src = project_root.join(&src);
    if !absolute_src.exists() {
        return Err(FolderOpError::SourceMissing);
    }
    let absolute_dest_dir = project_root.join(&dest_dir);
    if !absolute_dest_dir.is_dir() {
        return Err(FolderOpError::DestNotFolder);
    }
    let absolute_new = project_root.join(&new_relative);
    if absolute_new.exists() {
        return Err(FolderOpError::Collision(rel_string(&new_relative)));
    }
    std::fs::rename(&absolute_src, &absolute_new).map_err(|e| FolderOpError::Io(e.to_string()))?;
    Ok(new_relative)
}

/// Project-relative path as a forward-slash string (the `cobolt.toml` form).
pub fn rel_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "prc_fs_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn validate_rejects_bad_names() {
        assert_eq!(validate_folder_name(""), Err(FolderOpError::EmptyName));
        assert_eq!(validate_folder_name("   "), Err(FolderOpError::EmptyName));
        assert_eq!(validate_folder_name(".hidden"), Err(FolderOpError::DottedName));
        assert_eq!(validate_folder_name(".."), Err(FolderOpError::DottedName));
        assert_eq!(validate_folder_name("a/b"), Err(FolderOpError::IllegalChar));
        assert_eq!(validate_folder_name("a\\b"), Err(FolderOpError::IllegalChar));
        assert!(validate_folder_name("customers").is_ok());
        assert!(validate_folder_name("Order 2026").is_ok());
    }

    #[test]
    fn clean_rel_rejects_traversal_and_absolute() {
        assert_eq!(clean_rel(Path::new("../etc")), Err(FolderOpError::Traversal));
        assert_eq!(clean_rel(Path::new("/etc/passwd")), Err(FolderOpError::Traversal));
        assert_eq!(
            clean_rel(Path::new("forms/./a")).unwrap(),
            PathBuf::from("forms/a")
        );
    }

    #[test]
    fn create_then_collision() {
        let root = tmp();
        fs::create_dir_all(root.join("forms")).unwrap();
        let rel = create_folder(&root, Path::new("forms"), "customers").unwrap();
        assert_eq!(rel, PathBuf::from("forms/customers"));
        assert!(root.join("forms/customers").is_dir());
        assert!(!rel.is_absolute(), "returned path must be relative (R21)");
        // Second create with the same name collides.
        assert!(matches!(
            create_folder(&root, Path::new("forms"), "customers"),
            Err(FolderOpError::Collision(_))
        ));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_rejects_missing_parent() {
        let root = tmp();
        assert_eq!(
            create_folder(&root, Path::new("forms/nope"), "x"),
            Err(FolderOpError::ParentMissing)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rename_moves_dir_and_protects_category_root() {
        let root = tmp();
        fs::create_dir_all(root.join("forms/customers")).unwrap();
        let new_rel =
            rename_folder(&root, Path::new("forms/customers"), "clients", Path::new("forms"))
                .unwrap();
        assert_eq!(new_rel, PathBuf::from("forms/clients"));
        assert!(root.join("forms/clients").is_dir());
        assert!(!root.join("forms/customers").exists());
        assert!(!new_rel.is_absolute());
        // Renaming the category root itself is refused.
        assert_eq!(
            rename_folder(&root, Path::new("forms"), "sources", Path::new("forms")),
            Err(FolderOpError::IsCategoryRoot)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_is_recursive_and_protects_category_root() {
        let root = tmp();
        fs::create_dir_all(root.join("forms/customers/orders")).unwrap();
        fs::write(root.join("forms/customers/orders/a.cfrm"), b"x").unwrap();
        let removed =
            delete_folder(&root, Path::new("forms/customers"), Path::new("forms")).unwrap();
        assert_eq!(removed, PathBuf::from("forms/customers"));
        assert!(!root.join("forms/customers").exists());
        assert!(root.join("forms").is_dir(), "category root survives");
        assert_eq!(
            delete_folder(&root, Path::new("forms"), Path::new("forms")),
            Err(FolderOpError::IsCategoryRoot)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_path_guards() {
        let root = tmp();
        fs::create_dir_all(root.join("forms/a")).unwrap();
        fs::create_dir_all(root.join("forms/b")).unwrap();
        fs::write(root.join("forms/a/order.cfrm"), b"x").unwrap();

        // Happy path: move the file from a/ to b/.
        let moved = move_path(&root, Path::new("forms/a/order.cfrm"), Path::new("forms/b")).unwrap();
        assert_eq!(moved, PathBuf::from("forms/b/order.cfrm"));
        assert!(root.join("forms/b/order.cfrm").is_file());
        assert!(!moved.is_absolute());

        // No-op: moving into its current parent.
        assert_eq!(
            move_path(&root, Path::new("forms/b/order.cfrm"), Path::new("forms/b")),
            Err(FolderOpError::NoOp)
        );
        // Self/descendant: move a folder under itself.
        assert_eq!(
            move_path(&root, Path::new("forms/a"), Path::new("forms/a")),
            Err(FolderOpError::SelfDescendant)
        );
        // Collision.
        fs::write(root.join("forms/a/order.cfrm"), b"y").unwrap();
        assert!(matches!(
            move_path(&root, Path::new("forms/a/order.cfrm"), Path::new("forms/b")),
            Err(FolderOpError::Collision(_))
        ));
        fs::remove_dir_all(&root).ok();
    }
}
