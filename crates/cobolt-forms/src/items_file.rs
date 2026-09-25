// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A ComboBox's or ListBox's items read from a text file (`ItemsFile`).
//!
//! The file is read when the form OPENS — Run Form and a built application
//! alike — so editing the text file changes the list without touching the
//! form. One item per line; blank lines are skipped. The designed `Items` is
//! what the list shows when the file is absent or unreadable.

use crate::model::{Control, ControlType, PropValue};

/// The property that names the file.
pub const ITEMS_FILE: &str = "ItemsFile";

/// Does this control take its items from a file?
pub fn takes_items_file(ctype: &ControlType) -> bool {
    matches!(ctype, ControlType::ComboBox | ControlType::ListBox)
}

/// A text file's lines as an `Items` value: one item per line, newline-
/// separated, line endings normalised, blank lines left out. `None` when the
/// file cannot be read.
pub fn read_items(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Some(
        text.lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Fill `Items` from `ItemsFile` for every ComboBox / ListBox in `controls`
/// (children included) whose file can be read. A stored path is resolved like
/// every other asset path: against the project folder under Run Form, beside
/// the executable in a built application.
pub fn apply(controls: &mut [Control]) {
    for c in controls.iter_mut() {
        if takes_items_file(&c.control_type) {
            let stored = c.get_prop(ITEMS_FILE).map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
            if !stored.is_empty() {
                if let Some(items) = read_items(&crate::assets::resolve(&stored)) {
                    c.set_prop("Items", PropValue::String(items));
                }
            }
        }
        apply(&mut c.children);
    }
}

/// [`apply`] for a surface that redraws every frame — the IDE's Preview —
/// reading each file again only when its modification time or size changes.
pub fn apply_cached(controls: &mut [Control]) {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Entry = (Option<std::time::SystemTime>, u64, String);
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf, Entry>>> = OnceLock::new();
    for c in controls.iter_mut() {
        if takes_items_file(&c.control_type) {
            let stored = c.get_prop(ITEMS_FILE).map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
            if !stored.is_empty() {
                let path = crate::assets::resolve(&stored);
                if let Ok(meta) = std::fs::metadata(&path) {
                    let stamp = (meta.modified().ok(), meta.len());
                    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
                    let fresh = cache.get(&path).is_some_and(|(m, l, _)| (*m, *l) == stamp);
                    if !fresh {
                        if let Some(items) = read_items(&path) {
                            cache.insert(path.clone(), (stamp.0, stamp.1, items));
                        }
                    }
                    if let Some((_, _, items)) = cache.get(&path) {
                        c.set_prop("Items", PropValue::String(items.clone()));
                    }
                }
            }
        }
        apply_cached(&mut c.children);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_file_becomes_the_items_of_a_combo_and_a_list() {
        let dir = std::env::temp_dir().join(format!("prc-items-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ufs.txt");
        std::fs::write(&file, "\u{feff}AC\r\nAL\r\n\r\nAM\n").unwrap();
        let mut combo = Control::new("CMB", ControlType::ComboBox, 0, 0);
        combo.set_prop(ITEMS_FILE, PropValue::String(file.display().to_string()));
        combo.set_prop("Items", PropValue::String("designed".into()));
        let mut list = Control::new("LST", ControlType::ListBox, 0, 0);
        list.set_prop(ITEMS_FILE, PropValue::String(file.display().to_string()));
        let mut missing = Control::new("GONE", ControlType::ComboBox, 0, 0);
        missing.set_prop(ITEMS_FILE, PropValue::String(dir.join("nope.txt").display().to_string()));
        missing.set_prop("Items", PropValue::String("fallback".into()));
        let mut controls = vec![combo, list, missing];
        apply(&mut controls);
        let items = |c: &Control| c.get_prop("Items").map(|v| v.to_string()).unwrap_or_default();
        assert_eq!(items(&controls[0]), "AC\nAL\nAM", "BOM, CRLF and blank lines handled");
        assert_eq!(items(&controls[1]), "AC\nAL\nAM");
        assert_eq!(items(&controls[2]), "fallback", "an unreadable file leaves the designed Items");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
