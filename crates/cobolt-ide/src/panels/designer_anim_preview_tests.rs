// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The designer's animation preview plays what the form designed.**
//!
//! Two defects kept the preview silent. The toolbar's Preview Anims button played only the animations
//! whose trigger is `OnFormLoad`, so a form whose animations are `OnShow`, `OnClick`, `OnHover`,
//! `OnFocus` or `Programmatic` showed nothing. And the Properties pane's ▶ Preview, with several
//! controls selected, went through the multi-edit fan-out, which keeps only the controls that carry
//! the key as a property: `_PreviewAnim<N>` is no property, so no control played.

use super::*;
use cobolt_forms::model::{AnimTrigger, AnimationDef, PropValue};
use cobolt_forms::{Control, ControlType, Form};

fn panel(triggers: &[(&str, AnimTrigger)]) -> DesignerPanel {
    let mut form = Form::new("F", "F", 800, 600);
    for (id, trigger) in triggers {
        let mut c = Control::new(*id, ControlType::PictureBox, 10, 10);
        let mut a = AnimationDef::new("anim1");
        a.trigger = trigger.clone();
        c.animations.push(a);
        form.controls.push(c);
    }
    DesignerPanel::new(form)
}

fn playing(d: &DesignerPanel, id: &str) -> bool {
    d.anim_states.get(&format!("{id}:anim1")).is_some_and(|s| s.playing)
}

#[test]
fn the_toolbar_button_plays_an_animation_whatever_its_trigger() {
    let triggers = [
        ("LOAD", AnimTrigger::OnFormLoad),
        ("SHOW", AnimTrigger::OnShow),
        ("CLICK", AnimTrigger::OnClick),
        ("HOVER", AnimTrigger::OnHover),
        ("FOCUS", AnimTrigger::OnFocus),
        ("PROG", AnimTrigger::Programmatic),
    ];
    let mut d = panel(&triggers);
    d.play_all_anims();
    let started: Vec<&str> = triggers.iter().map(|(id, _)| *id).filter(|id| playing(&d, id)).collect();
    println!("\n  animations started by the toolbar button: {} of {} ({:?})", started.len(), triggers.len(), started);
    assert_eq!(started.len(), triggers.len());
}

#[test]
fn the_panes_preview_plays_with_one_control_selected() {
    let mut d = panel(&[("A", AnimTrigger::OnClick)]);
    d.selected_ids = vec!["A".into()];
    d.set_property_from_pane("A", "_PreviewAnim0", PropValue::Bool(true));
    assert!(playing(&d, "A"));
}

#[test]
fn the_panes_preview_plays_with_several_controls_selected() {
    let mut d = panel(&[("A", AnimTrigger::OnClick), ("B", AnimTrigger::OnClick)]);
    d.selected_ids = vec!["A".into(), "B".into()];
    d.set_property_from_pane("A", "_PreviewAnim0", PropValue::Bool(true));
    assert!(playing(&d, "A"), "the control the pane reports plays");
    assert!(!d.dirty && d.undo_stack.is_empty(), "a preview is no edit");
}
