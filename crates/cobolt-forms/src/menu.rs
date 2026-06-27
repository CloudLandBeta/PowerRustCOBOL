// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Menu data model: hierarchical pulldown menus persisted as YAML with
//! HMAC-SHA256 integrity (spec 018).

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::Path;

type HmacSha256 = Hmac<Sha256>;

const HMAC_KEY: &[u8] = b"PowerRustCOBOL-menu-integrity-2026";
const MAX_DEPTH: usize = 3;

// ── Data model ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuDefinition {
    pub menu: Vec<MenuItem>,
    #[serde(default)]
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "type", default = "default_item_type")]
    pub item_type: MenuItemType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accelerator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuItemType {
    Action,
    Separator,
}

fn default_item_type() -> MenuItemType {
    MenuItemType::Action
}
fn default_true() -> bool {
    true
}

impl MenuDefinition {
    pub fn new() -> Self {
        Self {
            menu: Vec::new(),
            hash: String::new(),
        }
    }

    pub fn find_item(&self, id: &str) -> Option<&MenuItem> {
        fn find_in<'a>(items: &'a [MenuItem], id: &str) -> Option<&'a MenuItem> {
            for item in items {
                if item.id == id { return Some(item); }
                if let Some(found) = find_in(&item.items, id) { return Some(found); }
            }
            None
        }
        find_in(&self.menu, id)
    }

    pub fn find_item_mut(&mut self, id: &str) -> Option<&mut MenuItem> {
        fn find_in<'a>(items: &'a mut [MenuItem], id: &str) -> Option<&'a mut MenuItem> {
            for item in items {
                if item.id == id { return Some(item); }
                if let Some(found) = find_in(&mut item.items, id) { return Some(found); }
            }
            None
        }
        find_in(&mut self.menu, id)
    }

    pub fn set_item_enabled(&mut self, item_id: &str, enabled: bool) -> bool {
        if let Some(item) = self.find_item_mut(item_id) {
            item.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn get_item_enabled(&self, item_id: &str) -> Option<bool> {
        self.find_item(item_id).map(|i| i.enabled)
    }
}

impl Default for MenuDefinition {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuItem {
    pub fn new_action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            item_type: MenuItemType::Action,
            icon: None,
            accelerator: None,
            action: None,
            enabled: true,
            items: Vec::new(),
        }
    }

    pub fn new_separator(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: String::new(),
            item_type: MenuItemType::Separator,
            icon: None,
            accelerator: None,
            action: None,
            enabled: true,
            items: Vec::new(),
        }
    }
}

// ── Depth validation ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum MenuError {
    #[error("menu exceeds maximum depth of {0}")]
    DepthExceeded(usize),
    #[error("HMAC integrity check failed — menu file may have been tampered with")]
    HmacMismatch,
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

fn check_depth(items: &[MenuItem], current: usize, max: usize) -> Result<(), MenuError> {
    if current > max {
        return Err(MenuError::DepthExceeded(max));
    }
    for item in items {
        check_depth(&item.items, current + 1, max)?;
    }
    Ok(())
}

pub fn validate_depth(def: &MenuDefinition) -> Result<(), MenuError> {
    check_depth(&def.menu, 1, MAX_DEPTH)
}

// ── HMAC ────────────────────────────────────────────────────────────────────────

fn compute_hmac(menu_yaml: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(HMAC_KEY).expect("HMAC accepts any key length");
    mac.update(menu_yaml.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn menu_content_yaml(def: &MenuDefinition) -> Result<String, serde_yaml::Error> {
    // Serialise only the `menu` field (not the hash) for HMAC computation.
    #[derive(Serialize)]
    struct Content<'a> {
        menu: &'a Vec<MenuItem>,
    }
    serde_yaml::to_string(&Content { menu: &def.menu })
}

fn verify_hmac(def: &MenuDefinition) -> Result<(), MenuError> {
    let content = menu_content_yaml(def)?;
    let expected = compute_hmac(&content);
    if def.hash != expected {
        return Err(MenuError::HmacMismatch);
    }
    Ok(())
}

// ── hex encoding (tiny inline, no extra dep) ────────────────────────────────────

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

// ── Load / Save ─────────────────────────────────────────────────────────────────

pub fn load_menu(path: &Path) -> Result<MenuDefinition, MenuError> {
    let content = std::fs::read_to_string(path)?;
    let def: MenuDefinition = serde_yaml::from_str(&content)?;
    verify_hmac(&def)?;
    Ok(def)
}

pub fn save_menu(path: &Path, def: &MenuDefinition) -> Result<(), MenuError> {
    validate_depth(def)?;
    let content_yaml = menu_content_yaml(def)?;
    let hash = compute_hmac(&content_yaml);
    let mut final_def = def.clone();
    final_def.hash = hash;
    let yaml = serde_yaml::to_string(&final_def)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

/// Build the YAML file path for a menu control alongside its `.cfrm`.
pub fn menu_yaml_path(cfrm_dir: &Path, control_id: &str) -> std::path::PathBuf {
    cfrm_dir.join(format!("{}.menu.yaml", control_id))
}

// ── Accelerator parsing (T3) ────────────────────────────────────────────────────

/// Parsed accelerator: modifier flags + a key character.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Accelerator {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub cmd: bool,
    pub key: char,
}

pub fn parse_accelerator(s: &str) -> Option<Accelerator> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut cmd = false;
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }
    let key_part = parts.last()?;
    for &modifier in &parts[..parts.len() - 1] {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" | "opt" => alt = true,
            "cmd" | "command" | "meta" | "super" => cmd = true,
            _ => return None,
        }
    }
    let key = if key_part.len() == 1 {
        key_part.chars().next()?.to_ascii_uppercase()
    } else {
        match key_part.to_ascii_lowercase().as_str() {
            "f1" => '\u{F001}',
            "f2" => '\u{F002}',
            "f3" => '\u{F003}',
            "f4" => '\u{F004}',
            "f5" => '\u{F005}',
            "f6" => '\u{F006}',
            "f7" => '\u{F007}',
            "f8" => '\u{F008}',
            "f9" => '\u{F009}',
            "f10" => '\u{F00A}',
            "f11" => '\u{F00B}',
            "f12" => '\u{F00C}',
            "delete" | "del" => '\u{007F}',
            "backspace" => '\u{0008}',
            "tab" => '\t',
            "enter" | "return" => '\r',
            "escape" | "esc" => '\u{001B}',
            "space" => ' ',
            _ => return None,
        }
    };
    if !ctrl && !shift && !alt && !cmd {
        return None;
    }
    Some(Accelerator {
        ctrl,
        shift,
        alt,
        cmd,
        key,
    })
}

pub fn format_accelerator(accel: &Accelerator) -> String {
    let mut parts = Vec::new();
    if cfg!(target_os = "macos") {
        if accel.ctrl {
            parts.push("\u{2303}");
        }
        if accel.alt {
            parts.push("\u{2325}");
        }
        if accel.shift {
            parts.push("\u{21E7}");
        }
        if accel.cmd {
            parts.push("\u{2318}");
        }
    } else {
        if accel.ctrl {
            parts.push("Ctrl+");
        }
        if accel.alt {
            parts.push("Alt+");
        }
        if accel.shift {
            parts.push("Shift+");
        }
        if accel.cmd {
            parts.push("Super+");
        }
    }
    let key_str = match accel.key {
        '\u{F001}' => "F1".to_string(),
        '\u{F002}' => "F2".to_string(),
        '\u{F003}' => "F3".to_string(),
        '\u{F004}' => "F4".to_string(),
        '\u{F005}' => "F5".to_string(),
        '\u{F006}' => "F6".to_string(),
        '\u{F007}' => "F7".to_string(),
        '\u{F008}' => "F8".to_string(),
        '\u{F009}' => "F9".to_string(),
        '\u{F00A}' => "F10".to_string(),
        '\u{F00B}' => "F11".to_string(),
        '\u{F00C}' => "F12".to_string(),
        '\u{007F}' => "Del".to_string(),
        '\u{0008}' => "Bksp".to_string(),
        '\t' => "Tab".to_string(),
        '\r' => "Enter".to_string(),
        '\u{001B}' => "Esc".to_string(),
        ' ' => "Space".to_string(),
        c => c.to_string(),
    };
    let mut s: String = parts.into_iter().collect();
    s.push_str(&key_str);
    s
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_menu() -> MenuDefinition {
        MenuDefinition {
            menu: vec![
                MenuItem {
                    id: "file".into(),
                    label: "File".into(),
                    item_type: MenuItemType::Action,
                    icon: None,
                    accelerator: None,
                    action: None,
                    enabled: true,
                    items: vec![
                        MenuItem {
                            id: "file-new".into(),
                            label: "New".into(),
                            item_type: MenuItemType::Action,
                            icon: Some("doc-new".into()),
                            accelerator: Some("Cmd+N".into()),
                            action: Some("event".into()),
                            enabled: true,
                            items: vec![],
                        },
                        MenuItem::new_separator("sep-1"),
                        MenuItem {
                            id: "file-exit".into(),
                            label: "Exit".into(),
                            item_type: MenuItemType::Action,
                            icon: Some("x-circle".into()),
                            accelerator: None,
                            action: Some("close-application".into()),
                            enabled: true,
                            items: vec![],
                        },
                    ],
                },
            ],
            hash: String::new(),
        }
    }

    #[test]
    fn round_trip_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.menu.yaml");
        let def = sample_menu();
        save_menu(&path, &def).unwrap();
        let loaded = load_menu(&path).unwrap();
        assert_eq!(def.menu, loaded.menu);
        assert!(!loaded.hash.is_empty());
        println!("round_trip_yaml: menu with {} items round-tripped OK", loaded.menu.len());
    }

    #[test]
    fn hmac_validates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.menu.yaml");
        save_menu(&path, &sample_menu()).unwrap();
        // Valid load
        assert!(load_menu(&path).is_ok(), "valid HMAC should pass");
        // Tamper
        let mut content = std::fs::read_to_string(&path).unwrap();
        content = content.replace("New", "Tampered");
        std::fs::write(&path, &content).unwrap();
        let result = load_menu(&path);
        assert!(result.is_err(), "tampered file should fail HMAC");
        match result {
            Err(MenuError::HmacMismatch) => println!("hmac_validates: tamper correctly detected"),
            Err(e) => panic!("wrong error type: {e}"),
            Ok(_) => panic!("should have failed"),
        }
    }

    #[test]
    fn depth_limit() {
        let mut def = MenuDefinition::new();
        // Level 1
        let mut l1 = MenuItem::new_action("l1", "L1");
        // Level 2
        let mut l2 = MenuItem::new_action("l2", "L2");
        // Level 3
        let mut l3 = MenuItem::new_action("l3", "L3");
        // Level 4 — should fail
        let l4 = MenuItem::new_action("l4", "L4");
        l3.items.push(l4);
        l2.items.push(l3);
        l1.items.push(l2);
        def.menu.push(l1);
        let result = validate_depth(&def);
        assert!(result.is_err(), "4 levels should exceed max depth of 3");
        match result {
            Err(MenuError::DepthExceeded(3)) => println!("depth_limit: correctly rejected 4-level tree"),
            Err(e) => panic!("wrong error: {e}"),
            Ok(_) => panic!("should have failed"),
        }
    }

    #[test]
    fn accelerator_parse_cmd_n() {
        let a = parse_accelerator("Cmd+N").unwrap();
        assert!(a.cmd);
        assert!(!a.ctrl);
        assert!(!a.shift);
        assert!(!a.alt);
        assert_eq!(a.key, 'N');
        let formatted = format_accelerator(&a);
        println!("accelerator Cmd+N → {formatted}");
    }

    #[test]
    fn accelerator_parse_shift_ctrl_s() {
        let a = parse_accelerator("Shift+Ctrl+S").unwrap();
        assert!(a.shift);
        assert!(a.ctrl);
        assert!(!a.cmd);
        assert_eq!(a.key, 'S');
    }

    #[test]
    fn accelerator_parse_alt_f4() {
        let a = parse_accelerator("Alt+F4").unwrap();
        assert!(a.alt);
        assert_eq!(a.key, '\u{F004}');
    }

    #[test]
    fn accelerator_no_modifier_rejected() {
        assert!(parse_accelerator("N").is_none());
        assert!(parse_accelerator("").is_none());
    }
}
