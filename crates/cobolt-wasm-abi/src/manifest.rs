// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The manifest (ABI v1 §10) — `manifest.json`, also embedded in the module as
//! the custom section `prmanifest` — and the arena layout it implies (§2).

use serde::{Deserialize, Serialize};

use crate::codes::{ctrl, ABI_VERSION};

/// The custom-section name that carries the manifest inside a module.
pub const MANIFEST_SECTION: &str = "prmanifest";
/// The custom section that maps a line id to `(unit, line, column)`.
pub const LINES_SECTION: &str = "prlines";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantInfo {
    #[serde(default)]
    pub multi_memory: bool,
    #[serde(default)]
    pub conformance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub arena_size: u32,
    pub batch_size: u32,
    pub answer_size: u32,
    pub frame_size: u32,
    #[serde(default)]
    pub budget_blocks: u32,
    #[serde(default)]
    pub data_pages: u32,
}

impl Default for Limits {
    /// The defaults of §2: a 1 MiB arena, 64 KiB batch and answer, 384 KiB frames.
    fn default() -> Self {
        Limits { arena_size: 1 << 20, batch_size: 64 * 1024, answer_size: 64 * 1024, frame_size: 384 * 1024, budget_blocks: 20_000, data_pages: 17 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEntry {
    pub id: u32,
    pub name: String,
    /// The control type as `ControlType::as_str` spells it (`Button`, `TextBox`, …).
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerEntry {
    /// A control id of the form (`0` for the form itself).
    pub ctrl: u32,
    /// An index into [`Manifest::events`].
    pub event: u32,
    /// The generator's entry point; the host never interprets it.
    #[serde(default)]
    pub entry: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormEntry {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub main: bool,
    /// How many instances may be alive at once (WASM-017 R2).
    pub instances: u32,
    #[serde(default)]
    pub region_size: u32,
    pub controls: Vec<ControlEntry>,
    #[serde(default)]
    pub handlers: Vec<HandlerEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sizes {
    #[serde(default)]
    pub module: u64,
    #[serde(default)]
    pub rt_functions: u64,
    #[serde(default)]
    pub rt_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub abi: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub project: String,
    pub variant: VariantInfo,
    pub limits: Limits,
    pub names: Vec<String>,
    pub events: Vec<String>,
    pub forms: Vec<FormEntry>,
    #[serde(default)]
    pub builtins: Vec<String>,
    #[serde(default)]
    pub imports: Vec<String>,
    #[serde(default)]
    pub sizes: Sizes,
}

/// Why a manifest cannot be used. The text is meant for the person who built
/// the application, so it says what to rebuild, not how the host failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Json(String),
    AbiMismatch { found: u32, expected: u32 },
    Layout(String),
    Invalid(String),
}

impl core::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ManifestError::Json(e) => write!(f, "the manifest is not valid JSON: {e}"),
            ManifestError::AbiMismatch { found, expected } => write!(
                f,
                "this application was built for ABI {found} and this player speaks ABI {expected}: rebuild the application with the current IDE"
            ),
            ManifestError::Layout(e) => write!(f, "the transport arena does not fit: {e}"),
            ManifestError::Invalid(e) => write!(f, "the manifest is inconsistent: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Where each region of the arena starts and how long it is (§2). Offsets are
/// relative to the arena base (`pr_arena_base()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaLayout {
    pub batch_off: u32,
    pub batch_size: u32,
    pub answer_off: u32,
    pub answer_size: u32,
    pub frame_out_off: u32,
    pub frame_in_off: u32,
    pub frame_size: u32,
    pub total: u32,
}

impl ArenaLayout {
    /// Lay the regions out in the order of §2: batch, answer, frame-out, frame-in.
    pub fn new(l: &Limits) -> Result<ArenaLayout, ManifestError> {
        let total = (l.batch_size as u64) + (l.answer_size as u64) + 2 * (l.frame_size as u64);
        if l.batch_size == 0 || l.answer_size == 0 || l.frame_size == 0 {
            return Err(ManifestError::Layout("a region has size 0".into()));
        }
        if total > l.arena_size as u64 {
            return Err(ManifestError::Layout(format!(
                "batch {} + answer {} + 2 x frame {} = {} bytes is more than the arena of {} bytes",
                l.batch_size, l.answer_size, l.frame_size, total, l.arena_size
            )));
        }
        let answer_off = l.batch_size;
        let frame_out_off = answer_off + l.answer_size;
        let frame_in_off = frame_out_off + l.frame_size;
        Ok(ArenaLayout {
            batch_off: 0,
            batch_size: l.batch_size,
            answer_off,
            answer_size: l.answer_size,
            frame_out_off,
            frame_in_off,
            frame_size: l.frame_size,
            total: total as u32,
        })
    }
}

impl Manifest {
    /// Parse and validate. Everything the host later indexes with (`forms`,
    /// `names`, `events`) is checked here once, so a bad manifest is one readable
    /// error at load rather than a panic in the middle of an event.
    pub fn parse(json: &str) -> Result<Manifest, ManifestError> {
        let m: Manifest = serde_json::from_str(json).map_err(|e| ManifestError::Json(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.abi != ABI_VERSION {
            return Err(ManifestError::AbiMismatch { found: self.abi, expected: ABI_VERSION });
        }
        ArenaLayout::new(&self.limits)?;
        let mains = self.forms.iter().filter(|f| f.main).count();
        if mains != 1 {
            return Err(ManifestError::Invalid(format!("exactly one form must be the main form, found {mains}")));
        }
        for (i, f) in self.forms.iter().enumerate() {
            if f.id as usize != i {
                return Err(ManifestError::Invalid(format!("form `{}` has id {} at position {i}", f.name, f.id)));
            }
            if f.instances == 0 {
                return Err(ManifestError::Invalid(format!("form `{}` allows no instance", f.name)));
            }
            let mut seen = std::collections::BTreeSet::new();
            for c in &f.controls {
                if c.id < ctrl::FIRST_CONTROL {
                    return Err(ManifestError::Invalid(format!(
                        "control `{}` of form `{}` has the reserved id {}",
                        c.name, f.name, c.id
                    )));
                }
                if !seen.insert(c.id) {
                    return Err(ManifestError::Invalid(format!("form `{}` repeats control id {}", f.name, c.id)));
                }
            }
            for h in &f.handlers {
                if h.event as usize >= self.events.len() {
                    return Err(ManifestError::Invalid(format!(
                        "a handler of form `{}` names event {} but there are {} events",
                        f.name,
                        h.event,
                        self.events.len()
                    )));
                }
                if h.ctrl != ctrl::FORM && !seen.contains(&h.ctrl) {
                    return Err(ManifestError::Invalid(format!(
                        "a handler of form `{}` names the unknown control id {}",
                        f.name, h.ctrl
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn layout(&self) -> ArenaLayout {
        ArenaLayout::new(&self.limits).expect("validated")
    }

    pub fn main_form(&self) -> &FormEntry {
        self.forms.iter().find(|f| f.main).expect("validated: exactly one main form")
    }

    pub fn form_by_name(&self, name: &str) -> Option<&FormEntry> {
        self.forms.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The index of an event name in `events`.
    pub fn event_index(&self, name: &str) -> Option<u32> {
        self.events.iter().position(|e| e.eq_ignore_ascii_case(name)).map(|i| i as u32)
    }

    pub fn name(&self, id: u32) -> Option<&str> {
        self.names.get(id as usize).map(String::as_str)
    }
}

impl FormEntry {
    pub fn control(&self, id: u32) -> Option<&ControlEntry> {
        self.controls.iter().find(|c| c.id == id)
    }
    pub fn control_by_name(&self, name: &str) -> Option<&ControlEntry> {
        self.controls.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }
    /// Is a handler bound to `(ctrl, event)`? Events fire only for bound handlers.
    pub fn has_handler(&self, ctrl: u32, event: u32) -> bool {
        self.handlers.iter().any(|h| h.ctrl == ctrl && h.event == event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Manifest {
        Manifest {
            abi: 1,
            generator: "test".into(),
            project: "demo".into(),
            variant: VariantInfo { multi_memory: true, conformance: false },
            limits: Limits::default(),
            names: vec!["Caption".into(), "Text".into()],
            events: vec!["onClick".into(), "onLoad".into()],
            forms: vec![FormEntry {
                id: 0,
                name: "main-form".into(),
                main: true,
                instances: 1,
                region_size: 4096,
                controls: vec![ControlEntry { id: 3, name: "BTN-OK".into(), kind: "Button".into() }],
                handlers: vec![HandlerEntry { ctrl: 3, event: 0, entry: 17 }],
            }],
            builtins: vec![],
            imports: vec!["flush".into(), "query".into()],
            sizes: Sizes::default(),
        }
    }

    #[test]
    fn the_spec_example_manifest_parses_and_validates() {
        let json = r#"{
          "abi": 1, "generator": "cobolt-wasm 2.0.x", "project": "name",
          "variant": { "multi_memory": true, "conformance": false },
          "limits": { "arena_size": 1048576, "batch_size": 65536, "answer_size": 65536, "frame_size": 393216,
                      "budget_blocks": 20000, "data_pages": 17 },
          "names": ["Caption", "Text"], "events": ["onClick", "onChange"],
          "forms": [ { "id": 0, "name": "main-form", "main": true, "instances": 1, "region_size": 4096,
                       "controls": [ { "id": 3, "name": "BTN-OK", "type": "Button" } ],
                       "handlers": [ { "ctrl": 3, "event": 0, "entry": 17 } ] } ],
          "builtins": ["x"], "imports": ["flush", "query", "remote", "now", "random", "abort"],
          "sizes": { "module": 0, "rt_functions": 0, "rt_bytes": 0 } }"#;
        let m = Manifest::parse(json).unwrap();
        assert_eq!(m.main_form().name, "main-form");
        assert!(m.main_form().has_handler(3, 0));
        assert!(!m.main_form().has_handler(3, 1), "events fire only for bound handlers");
        let l = m.layout();
        assert_eq!((l.batch_off, l.answer_off, l.frame_out_off, l.frame_in_off), (0, 65536, 131072, 524288));
        assert_eq!(l.total, 65536 + 65536 + 2 * 393216);
    }

    #[test]
    fn a_manifest_round_trips_through_json() {
        let m = sample();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(Manifest::parse(&json).unwrap(), m);
    }

    #[test]
    fn each_inconsistency_is_a_readable_error() {
        let mut m = sample();
        m.abi = 2;
        assert!(matches!(m.validate(), Err(ManifestError::AbiMismatch { found: 2, expected: 1 })));

        let mut m = sample();
        m.limits.arena_size = 1000;
        assert!(matches!(m.validate(), Err(ManifestError::Layout(_))));

        let mut m = sample();
        m.forms[0].main = false;
        assert!(m.validate().unwrap_err().to_string().contains("main form"));

        let mut m = sample();
        m.forms[0].controls[0].id = 1;
        assert!(m.validate().unwrap_err().to_string().contains("reserved id"));

        let mut m = sample();
        m.forms[0].handlers[0].event = 9;
        assert!(m.validate().unwrap_err().to_string().contains("event 9"));

        let mut m = sample();
        m.forms[0].handlers[0].ctrl = 99;
        assert!(m.validate().unwrap_err().to_string().contains("unknown control id 99"));

        assert!(matches!(Manifest::parse("{"), Err(ManifestError::Json(_))));
    }
}
