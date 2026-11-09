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
    // A form's stylesheet travels with it (WASM-007 Q2); neither moves when either would land on a file.
    let sheet = form_stylesheet(&absolute_src).map(|s| (s, form_stylesheet_target(&absolute_new)));
    if let Some((_, to)) = &sheet {
        if to.exists() {
            return Err(FolderOpError::Collision(rel_string(&cobolt_forms::web_themes::stylesheet_beside(&new_relative))));
        }
    }
    std::fs::rename(&absolute_src, &absolute_new).map_err(|e| FolderOpError::Io(e.to_string()))?;
    if let Some((from, to)) = sheet {
        if let Err(e) = std::fs::rename(&from, &to) {
            // Put the form back, so the two never end up in different folders.
            let _ = std::fs::rename(&absolute_new, &absolute_src);
            return Err(FolderOpError::Io(e.to_string()));
        }
    }
    Ok(new_relative)
}

/// Is `path` a form file (`.cfrm`, `.wcfrm`, `.mcfrm`)?
pub(crate) fn is_form_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["cfrm", "wcfrm", "mcfrm"].iter().any(|f| e.eq_ignore_ascii_case(f)))
}

/// The stylesheet beside a form file, when there is one (WASM-007 Q2: `<forms folder>/<Form>.css`).
pub fn form_stylesheet(form_file: &Path) -> Option<PathBuf> {
    let sheet = cobolt_forms::web_themes::stylesheet_beside(form_file);
    (is_form_file(form_file) && sheet.is_file()).then_some(sheet)
}

fn form_stylesheet_target(form_file: &Path) -> PathBuf {
    cobolt_forms::web_themes::stylesheet_beside(form_file)
}

/// What deleting a form does with its stylesheet (WASM-007 R6, user code is sacred).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetOnDelete {
    /// The form has no stylesheet.
    None,
    /// The sheet is exactly as the IDE generated it: it goes with the form.
    Delete(PathBuf),
    /// The developer wrote in it (or it cannot be read as text): ask before deleting it.
    Ask(PathBuf),
}

/// [`SheetOnDelete`] for a form file, read from disk as bytes.
pub fn sheet_on_form_delete(form_file: &Path) -> SheetOnDelete {
    let Some(sheet) = form_stylesheet(form_file) else {
        return SheetOnDelete::None;
    };
    let untouched = std::fs::read(&sheet)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .is_some_and(|t| cobolt_forms::web_themes::stylesheet_untouched(&t));
    if untouched {
        SheetOnDelete::Delete(sheet)
    } else {
        SheetOnDelete::Ask(sheet)
    }
}

/// The first text of a new form's stylesheet (WASM-007 R4), from its theme; none on a Desktop project (R14).
pub fn new_form_stylesheet(kind: cobolt_compiler::project_kind::ProjectKind, project_theme: Option<&str>, form: &cobolt_forms::Form) -> Option<String> {
    (!kind.is_desktop()).then(|| cobolt_forms::web_themes::form_stylesheet_for(form, project_theme))
}

/// What a theme switch does to a form's stylesheet (WASM-007 R4, R6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetOnThemeSwitch {
    /// The generated block is as written: the new text, the block replaced and every other byte kept.
    Replace(String),
    /// The developer edited the block (or removed its markers): ask; `can_replace` when the markers are there.
    Ask { can_replace: bool },
}

/// [`SheetOnThemeSwitch`] for a sheet's text, switching to `theme` for the control types `used`.
pub fn sheet_on_theme_switch(text: &str, theme: &str, used: &[cobolt_forms::ControlType]) -> SheetOnThemeSwitch {
    use cobolt_forms::web_themes::{generated_block_untouched, replace_generated_block};
    match replace_generated_block(text, theme, used) {
        Some(new) if generated_block_untouched(text) => SheetOnThemeSwitch::Replace(new),
        replaced => SheetOnThemeSwitch::Ask { can_replace: replaced.is_some() },
    }
}

/// Write a new form's stylesheet beside it (WASM-007 R4) — never over a file that is already there.
/// Returns the path written, `None` when one existed.
pub fn create_form_stylesheet(form_file: &Path, text: &str) -> std::io::Result<Option<PathBuf>> {
    let sheet = cobolt_forms::web_themes::stylesheet_beside(form_file);
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&sheet) {
        Ok(mut f) => {
            use std::io::Write;
            f.write_all(text.as_bytes())?;
            Ok(Some(sheet))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(e) => Err(e),
    }
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
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        // pid + nanos + a process-wide counter so parallel tests in the same
        // nanosecond can't collide on the same temp dir (flaky-test guard).
        let base = std::env::temp_dir().join(format!(
            "prc_fs_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, Ordering::Relaxed)
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

    /// WASM-007 T17: a form's stylesheet moves with it, and neither moves when the sheet would collide.
    #[test]
    fn move_path_carries_the_forms_stylesheet_and_refuses_a_collision_before_moving_either() {
        let root = tmp();
        fs::create_dir_all(root.join("webforms/a")).unwrap();
        fs::create_dir_all(root.join("webforms/b")).unwrap();
        fs::write(root.join("webforms/a/order.wcfrm"), b"x").unwrap();
        let odd = b"/* mine */\r\n#BTN-1   {color:red}\r\n\r\n".to_vec();
        fs::write(root.join("webforms/a/order.css"), &odd).unwrap();
        let moved = move_path(&root, Path::new("webforms/a/order.wcfrm"), Path::new("webforms/b")).unwrap();
        assert_eq!(moved, PathBuf::from("webforms/b/order.wcfrm"));
        assert_eq!(fs::read(root.join("webforms/b/order.css")).unwrap(), odd, "moved byte for byte");
        assert!(!root.join("webforms/a/order.css").exists());

        // The sheet's place is taken: nothing moves.
        fs::write(root.join("webforms/a/order.css"), b"theirs").unwrap();
        assert!(matches!(
            move_path(&root, Path::new("webforms/b/order.wcfrm"), Path::new("webforms/a")),
            Err(FolderOpError::Collision(c)) if c == "webforms/a/order.css"
        ));
        assert!(root.join("webforms/b/order.wcfrm").is_file() && root.join("webforms/b/order.css").is_file());
        assert_eq!(fs::read(root.join("webforms/a/order.css")).unwrap(), b"theirs");

        // A file that is not a form keeps its own name only.
        fs::write(root.join("webforms/b/notes.txt"), b"n").unwrap();
        fs::write(root.join("webforms/b/notes.css"), b"n").unwrap();
        move_path(&root, Path::new("webforms/b/notes.txt"), Path::new("webforms/a")).unwrap();
        assert!(root.join("webforms/b/notes.css").is_file());
        println!("move_path: order.wcfrm + order.css moved together, CRLF sheet byte-identical; a taken sheet name refused both");
        fs::remove_dir_all(&root).ok();
    }

    /// WASM-007 R6, T17: deleting a form deletes a sheet only as the IDE generated it; any other asks.
    /// AC4 (round-trip half): a new sheet is never written over an existing file, and the bytes the
    /// IDE writes are read back identical.
    #[test]
    fn a_forms_sheet_is_deleted_with_it_only_when_untouched_and_never_overwritten() {
        let root = tmp();
        fs::create_dir_all(&root).unwrap();
        let form = root.join("order.wcfrm");
        fs::write(&form, b"x").unwrap();
        assert_eq!(sheet_on_form_delete(&form), SheetOnDelete::None);
        // A new web or mobile form gets a sheet from its theme; a desktop one none.
        use cobolt_compiler::project_kind::ProjectKind;
        let mut model = cobolt_forms::Form::new("ORDER", "Order", 640, 480);
        model.controls.push(cobolt_forms::Control::new("BTN-1", cobolt_forms::ControlType::Button, 0, 0));
        assert_eq!(new_form_stylesheet(ProjectKind::Desktop, Some("spatial"), &model), None);
        assert!(new_form_stylesheet(ProjectKind::Mobile, None, &model).unwrap().contains("\"liquid-glass\""));
        let generated = new_form_stylesheet(ProjectKind::Web, Some("spatial"), &model).unwrap();
        assert!(generated.contains("\"spatial\"") && generated.contains(".pr-button"), "{generated}");
        assert_eq!(create_form_stylesheet(&form, &generated).unwrap(), Some(root.join("order.css")));
        assert_eq!(fs::read(root.join("order.css")).unwrap(), generated.as_bytes());
        assert_eq!(sheet_on_form_delete(&form), SheetOnDelete::Delete(root.join("order.css")));
        let odd = format!("{generated}/* mine */\r\n.x{{color:red}}  \r\n");
        fs::write(root.join("order.css"), &odd).unwrap();
        assert_eq!(sheet_on_form_delete(&form), SheetOnDelete::Ask(root.join("order.css")));
        assert_eq!(create_form_stylesheet(&form, &generated).unwrap(), None, "an existing sheet is never overwritten");
        assert_eq!(fs::read(root.join("order.css")).unwrap(), odd.as_bytes(), "byte for byte");
        let edited_block = generated.replacen(".pr-button {", ".pr-button  {", 1);
        fs::write(root.join("order.css"), &edited_block).unwrap();
        assert_eq!(sheet_on_form_delete(&form), SheetOnDelete::Ask(root.join("order.css")));
        fs::remove_dir_all(&root).ok();
    }

    /// WASM-007 T18: an untouched sheet follows the theme with the developer's section byte-identical;
    /// a touched one is a question, and nothing is written until it is answered.
    #[test]
    fn a_theme_switch_replaces_only_an_untouched_block_and_asks_otherwise() {
        use cobolt_forms::ControlType;
        let used = [ControlType::Button];
        let mine = "\r\n/* mine */  #BTN-1{color:red}\r\n";
        let sheet = format!("{}{mine}", cobolt_forms::web_themes::form_stylesheet("liquid-glass", &used));
        let SheetOnThemeSwitch::Replace(new) = sheet_on_theme_switch(&sheet, "elegance", &used) else {
            panic!("an untouched block follows the theme");
        };
        assert!(new.ends_with(mine), "the developer's section is byte-identical");
        assert!(new.contains("var(--pr-accent); border-color: var(--pr-accent)"), "Elegance's button: {new}");
        let touched = sheet.replacen(".pr-button {", ".pr-button { color: hotpink; ", 1);
        assert_eq!(sheet_on_theme_switch(&touched, "elegance", &used), SheetOnThemeSwitch::Ask { can_replace: true });
        assert_eq!(sheet_on_theme_switch("a { b: c }", "elegance", &used), SheetOnThemeSwitch::Ask { can_replace: false });
        println!("theme switch: untouched → replaced ({} → {} bytes, the {}-byte section kept), touched → ask", sheet.len(), new.len(), mine.len());
    }
}
