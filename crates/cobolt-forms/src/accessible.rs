// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A control's accessible name** (WASM-004 T34, R10, AC7; operator, 2026-10-31).
//!
//! The name a screen reader announces for a control, resolved in this order:
//! 1. the control's own `AccessibleLabel`, when set;
//! 2. the control's own visible caption, for the types that show one (a Button, a CheckBox, …);
//! 3. the Label that comes just before the control in tab order (on the web, `aria-labelledby` to it);
//! 4. the control's `Tooltip`;
//! 5. none — and a web or mobile Check warns, naming the form and the control.
//!
//! The desktop renderer (its AccessKit node), the browser player (`aria-label` / `aria-labelledby`) and
//! Check all ask here, so the three agree. Property values are read through the caller's `get`, so a
//! value a running program has written is the one announced.

use crate::model::{Control, ControlType};

/// The property every control carries for its accessible name.
pub const ACCESSIBLE_LABEL: &str = "AccessibleLabel";

/// Where a control's accessible name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSource {
    /// Its own `AccessibleLabel`.
    Own,
    /// The caption it shows itself (the native name: nothing to add).
    Caption,
    /// The Label at this index of the caller's list, just before it in tab order.
    Label(usize),
    /// Its `Tooltip`.
    Tooltip,
    /// No name.
    None,
}

/// The types that show their own caption, which is their name when it is not empty.
pub fn shows_own_caption(t: &ControlType) -> bool {
    matches!(
        t,
        ControlType::Label
            | ControlType::Hyperlink
            | ControlType::Button
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::GroupBox
    )
}

/// The types a screen reader must be able to name: every control the keyboard reaches (the tab stops, and
/// the Switch and the Knob, which a browser focuses), but a TabControl (its tabs are its names) and a plug-in
/// control (class D: it never reaches a browser).
pub fn needs_name(t: &ControlType) -> bool {
    if t.is_non_visual() || matches!(t, ControlType::TabControl | ControlType::Custom { .. }) {
        return false;
    }
    t.is_tab_stop() || matches!(t, ControlType::Switch | ControlType::Knob)
}

/// The Label just before `controls[idx]` in tab order — the order Tab walks: by layer `rank`, then
/// `TabOrder`, then place in the list (`render.rs::resolve_tab_traversal`), among the controls that hold a
/// place in it (`ControlType::takes_tab_order`). `None` when the control before it is not a Label, is in
/// another layer, or there is none.
pub fn preceding_label(controls: &[&Control], idx: usize, rank: &dyn Fn(usize) -> usize) -> Option<usize> {
    let key = |i: usize| (rank(i), controls[i].tab_order, i);
    let me = key(idx);
    let before = (0..controls.len())
        .filter(|&i| i != idx && controls[i].control_type.takes_tab_order() && key(i) < me)
        .max_by_key(|&i| key(i))?;
    (controls[before].control_type == ControlType::Label && rank(before) == me.0).then_some(before)
}

/// Where the name of `controls[idx]` comes from. `get(i, property)` reads a property of `controls[i]` (the
/// live value where the caller has one).
pub fn name_source(
    controls: &[&Control],
    idx: usize,
    rank: &dyn Fn(usize) -> usize,
    get: &dyn Fn(usize, &str) -> String,
) -> NameSource {
    name_source_with(&controls[idx].control_type, idx, preceding_label(controls, idx, rank), get)
}

/// [`name_source`] for a caller that has found the Label before the control itself (`label_before`, an
/// index `get` reads, as [`preceding_label`] finds it): the browser player, whose tree is not a list of
/// [`Control`]s.
pub fn name_source_with(
    t: &ControlType,
    idx: usize,
    label_before: Option<usize>,
    get: &dyn Fn(usize, &str) -> String,
) -> NameSource {
    let set = |i: usize, k: &str| !get(i, k).trim().is_empty();
    if set(idx, ACCESSIBLE_LABEL) {
        return NameSource::Own;
    }
    if shows_own_caption(t) && set(idx, "Caption") {
        return NameSource::Caption;
    }
    if let Some(l) = label_before.filter(|&l| set(l, "Caption")) {
        return NameSource::Label(l);
    }
    if set(idx, "Tooltip") {
        return NameSource::Tooltip;
    }
    NameSource::None
}

/// The text of the name `source` gives `controls[idx]`, or `None` for [`NameSource::None`].
pub fn name_text(source: NameSource, idx: usize, get: &dyn Fn(usize, &str) -> String) -> Option<String> {
    let t = |i: usize, k: &str| Some(get(i, k).trim().to_owned());
    match source {
        NameSource::Own => t(idx, ACCESSIBLE_LABEL),
        NameSource::Caption => t(idx, "Caption"),
        NameSource::Label(l) => t(l, "Caption"),
        NameSource::Tooltip => t(idx, "Tooltip"),
        NameSource::None => None,
    }
}

/// `get` over the designed values of `controls`.
pub fn designed<'a>(controls: &'a [&'a Control]) -> impl Fn(usize, &str) -> String + 'a {
    move |i, k| controls[i].get_prop(k).map(|v| v.as_str().to_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PropValue;

    fn ctl(id: &str, t: ControlType, props: &[(&str, &str)]) -> Control {
        let mut c = Control::new(id, t, 0, 0);
        for (k, v) in props {
            c.set_prop(*k, PropValue::String((*v).into()));
        }
        c
    }

    fn sources(cs: &[Control], rank: &dyn Fn(usize) -> usize) -> Vec<(String, NameSource, Option<String>)> {
        let refs: Vec<&Control> = cs.iter().collect();
        let get = designed(&refs);
        (0..refs.len())
            .map(|i| {
                let s = name_source(&refs, i, rank, &get);
                (refs[i].id.clone(), s, name_text(s, i, &get))
            })
            .collect()
    }

    #[test]
    fn every_control_carries_an_empty_accessible_label() {
        for t in ControlType::ALL {
            let c = Control::new("X", t.clone(), 0, 0);
            assert_eq!(c.get_prop(ACCESSIBLE_LABEL).map(|v| v.as_str().to_owned()), Some(String::new()), "{t:?}");
        }
        println!("AccessibleLabel seeded empty on {} control types", ControlType::ALL.len());
    }

    #[test]
    fn the_name_is_the_own_label_then_the_caption_then_the_label_before_then_the_tooltip() {
        let cs = vec![
            ctl("LBL-NAME", ControlType::Label, &[("Caption", "Name")]),
            ctl("TXT-NAME", ControlType::TextBox, &[("Tooltip", "ignored: the Label comes first")]),
            ctl("TXT-OWN", ControlType::TextBox, &[(ACCESSIBLE_LABEL, "Customer"), ("Tooltip", "tip")]),
            ctl("NUD-TIP", ControlType::NumericUpDown, &[("Tooltip", "Quantity")]),
            ctl("BTN-OK", ControlType::Button, &[("Caption", "OK")]),
            ctl("BTN-OWN", ControlType::Button, &[("Caption", "X"), (ACCESSIBLE_LABEL, "Close")]),
            ctl("LBL-EMPTY", ControlType::Label, &[("Caption", "")]),
            ctl("SLD-NONE", ControlType::Slider, &[]),
            ctl("CH-1", ControlType::BarChart, &[]),
        ];
        let got = sources(&cs, &|_| 0);
        for g in &got {
            println!("{g:?}");
        }
        let s = |id: &str| got.iter().find(|g| g.0 == id).map(|g| (g.1, g.2.clone())).unwrap();
        assert_eq!(s("TXT-NAME"), (NameSource::Label(0), Some("Name".into())));
        assert_eq!(s("TXT-OWN"), (NameSource::Own, Some("Customer".into())), "its own label comes before the Label");
        assert_eq!(s("NUD-TIP"), (NameSource::Tooltip, Some("Quantity".into())), "the control before it is a TextBox");
        assert_eq!(s("BTN-OK"), (NameSource::Caption, Some("OK".into())));
        assert_eq!(s("BTN-OWN"), (NameSource::Own, Some("Close".into())));
        assert_eq!(s("SLD-NONE"), (NameSource::None, None), "the Label before it has no caption");
        // The chart holds no place in the tab order: the label before it is not looked for past the Slider.
        assert_eq!(s("CH-1").0, NameSource::None);
        assert!(needs_name(&ControlType::Slider) && needs_name(&ControlType::Switch) && needs_name(&ControlType::Knob));
        assert!(!needs_name(&ControlType::BarChart) && !needs_name(&ControlType::Label) && !needs_name(&ControlType::TabControl) && !needs_name(&ControlType::Timer));
    }

    #[test]
    fn the_label_before_is_taken_in_tab_order_and_in_the_same_layer() {
        let mut cs = vec![
            ctl("TXT-A", ControlType::TextBox, &[]),
            ctl("LBL-A", ControlType::Label, &[("Caption", "First")]),
            ctl("TXT-B", ControlType::TextBox, &[]),
        ];
        // TabOrder puts the Label first, then TXT-A: the list order does not decide.
        cs[0].tab_order = 2;
        cs[1].tab_order = 1;
        cs[2].tab_order = 3;
        let got = sources(&cs, &|_| 0);
        assert_eq!(got[0].1, NameSource::Label(1), "{got:?}");
        assert_eq!(got[2].1, NameSource::None, "TXT-A is before TXT-B: {got:?}");
        // The Label in the base, TXT-A in a layer: not its label.
        let layered = sources(&cs, &|i| usize::from(i == 0));
        assert_eq!(layered[0].1, NameSource::None, "{layered:?}");
    }
}
