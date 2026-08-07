// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The **single** control-state type (spec 042 R2) and its merge rules.
//!
//! Every host reads and writes control state through this module. It used to
//! exist three times (run-form, the compiled-app template, the IDE's retired
//! in-process host), which is how the 1.60.33 case-variant caption bug shipped
//! in exactly one of them.

use std::collections::HashMap;

/// Mutable UI-side state of a single control.
#[derive(Clone)]
pub struct CtrlState {
    pub props: HashMap<String, String>,
    pub visible: bool,
    pub enabled: bool,
}

/// A state entry created on the fly (a repeating-group card instance that the
/// interpreter writes to before it exists in the map) must start VISIBLE and
/// ENABLED. `#[derive(Default)]` would make it `false`, so the very controls a
/// data binding populates would be the ones the renderer skips — databound card
/// members blank in the run form while the DataGrid (whose key is pre-seeded
/// from the design) paints fine.
impl Default for CtrlState {
    fn default() -> Self {
        Self {
            props: HashMap::new(),
            visible: true,
            enabled: true,
        }
    }
}

impl CtrlState {
    pub fn from_control(ctrl: &cobolt_forms::Control) -> Self {
        let mut props = HashMap::new();
        for (k, v) in &ctrl.properties {
            props.insert(k.clone(), v.to_xml_string());
        }
        CtrlState {
            props,
            visible: ctrl.visible,
            enabled: ctrl.enabled,
        }
    }

    pub fn set(&mut self, key: &str, value: String) {
        match key.to_ascii_uppercase().as_str() {
            "VISIBLE" => self.visible = value != "0" && value != "false",
            "ENABLED" => self.enabled = value != "0" && value != "false",
            _ => {}
        }
        // Overwrite any case-insensitive duplicate so a runtime update (arrives
        // upper-cased from COBOL) never shadows the designed-case key.
        if !self.props.contains_key(key) {
            if let Some(existing) = self
                .props
                .keys()
                .find(|k| k.eq_ignore_ascii_case(key))
                .cloned()
            {
                self.props.remove(&existing);
            }
        }
        self.props.insert(key.to_owned(), value);
    }
}

/// The mutable state entry for a DRAWN control id, created when missing.
/// Repeating-group instances (`group.group-N.member`) never exist in the
/// initial map — it is seeded from the designed controls only — so a card
/// member's first databind write would otherwise land in a bare default
/// entry. Seed such an entry from the designed template control (last
/// dotted segment) so the instance inherits its designed visibility,
/// enablement, and properties.
pub fn state_entry_mut<'a>(
    state: &'a mut HashMap<String, CtrlState>,
    controls: &[cobolt_forms::Control],
    key: &str,
) -> &'a mut CtrlState {
    if !state.contains_key(key) {
        let base_id = key.rsplit('.').next().unwrap_or(key);
        let seeded = controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(base_id))
            .map(CtrlState::from_control)
            .unwrap_or_default();
        state.insert(key.to_owned(), seeded);
    }
    state.get_mut(key).expect("entry present or just inserted")
}

/// `FormState` over the live control-state map — merges runtime property values
/// onto each designed control so the unified engine paints the live state, and
/// supplies each control's current animation transform.
pub struct LiveState<'a> {
    pub state: &'a HashMap<String, CtrlState>,
    pub anim: &'a cobolt_forms::anim::AnimRuntime,
}

impl<'a> LiveState<'a> {
    /// Resolve `base.id` to its state entry case-INSENSITIVELY. Runtime updates
    /// arrive with COBOL-cased ids (unquoted identifiers are upper-cased) and
    /// databound repeating-group members are keyed by mixed-case mappings, so the
    /// designed-case id the renderer draws with does not always byte-match the
    /// state key. A byte-exact lookup here is how a databound card's per-row
    /// values silently fail to merge and the card shows its designed defaults
    /// (spec 015/024 × the split run-form process).
    pub fn entry(&self, base: &cobolt_forms::Control) -> Option<&CtrlState> {
        self.state
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&base.id))
            .and_then(|k| self.state.get(k))
    }
}

impl<'a> cobolt_forms::render::FormState for LiveState<'a> {
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        match self.entry(base) {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        }
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        self.entry(base).map(|s| s.visible).unwrap_or(true)
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        self.entry(base).map(|s| s.enabled).unwrap_or(true)
    }
    fn transform(&self, base: &cobolt_forms::Control) -> cobolt_forms::render::RenderTransform {
        self.anim.transform(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType};

    /// The 1.60.33 rule: a property has exactly ONE entry whatever its
    /// spelling. Before it, "Caption" (designed) and "CAPTION" (runtime) both
    /// lived in the map and HashMap iteration order decided which one the
    /// renderer painted — an intermittent, patternless coin toss.
    #[test]
    fn a_runtime_write_replaces_the_designed_case_key() {
        let mut ctrl = Control::new("Label-1", ControlType::Label, 0, 0);
        ctrl.set_prop("Caption", cobolt_forms::PropValue::String("designed".into()));
        let mut st = CtrlState::from_control(&ctrl);

        st.set("CAPTION", "from-handler".into());

        let caption_keys: Vec<&String> = st
            .props
            .keys()
            .filter(|k| k.eq_ignore_ascii_case("caption"))
            .collect();
        assert_eq!(
            caption_keys.len(),
            1,
            "exactly one caption entry, got {caption_keys:?}"
        );
        assert_eq!(st.props.get("CAPTION").map(String::as_str), Some("from-handler"));
    }

    /// VISIBLE/ENABLED writes update the booleans whatever the spelling, and
    /// falsy is exactly "0"/"false".
    #[test]
    fn visible_and_enabled_parse_case_insensitively() {
        let mut st = CtrlState::default();
        st.set("VISIBLE", "0".into());
        st.set("Enabled", "false".into());
        assert!(!st.visible);
        assert!(!st.enabled);
        st.set("visible", "1".into());
        assert!(st.visible);
    }

    /// `LiveState` resolves a state key case-insensitively: an upper-cased
    /// runtime key still merges onto the designed-case control id.
    #[test]
    fn live_state_reads_state_keys_case_insensitively() {
        let mut ctrl = Control::new("Label-1", ControlType::Label, 0, 0);
        ctrl.set_prop("Caption", cobolt_forms::PropValue::String("designed".into()));

        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let mut entry = CtrlState::default();
        entry.set("Caption", "live".into());
        state.insert("LABEL-1".into(), entry);

        let anim = cobolt_forms::anim::AnimRuntime::new(100.0, 100.0);
        let live = LiveState {
            state: &state,
            anim: &anim,
        };
        assert!(live.entry(&ctrl).is_some(), "LABEL-1 must resolve for Label-1");
    }

    /// An on-the-fly entry for a repeating-group instance is seeded from the
    /// designed template control (last dotted segment), not from a bare
    /// default — the instance inherits designed props/visibility/enablement.
    #[test]
    fn instance_entries_seed_from_the_designed_template() {
        let mut tmpl = Control::new("Member-1", ControlType::Label, 0, 0);
        tmpl.set_prop("Caption", cobolt_forms::PropValue::String("tmpl".into()));
        tmpl.enabled = false;
        let controls = vec![tmpl];

        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let entry = state_entry_mut(&mut state, &controls, "Group.Group-2.MEMBER-1");
        assert_eq!(entry.props.get("Caption").map(String::as_str), Some("tmpl"));
        assert!(!entry.enabled, "designed enablement inherited");

        // An id with no designed template still gets the visible+enabled
        // default (not the derived all-false one).
        let bare = state_entry_mut(&mut state, &controls, "NOWHERE-9");
        assert!(bare.visible && bare.enabled);
    }
}
