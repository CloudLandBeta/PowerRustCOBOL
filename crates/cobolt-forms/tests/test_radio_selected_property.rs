//! A RadioButton's state property is **`Selected`**, not `Checked` — `Checked`
//! belongs to the CheckBox (operator, 2026-08-31).
//!
//! The rename has to be invisible to forms that already exist: every `.cfrm`
//! written before it stores `Checked` on its radios, and every handler written
//! before it says `Checked` too. So the file migrates on load, and both
//! spellings keep answering at run time.

use cobolt_forms::model::{
    selection_property, toggle_state_of, CHECKED_PROP, SELECTED_PROP,
};
use cobolt_forms::{Control, ControlType, PropValue};

#[test]
fn a_radio_seeds_selected_and_a_checkbox_seeds_checked() {
    let radio = Control::new("RB", ControlType::RadioButton, 0, 0);
    assert!(
        radio.get_prop(SELECTED_PROP).is_some(),
        "a radio's state property is Selected"
    );
    assert!(
        radio.get_prop(CHECKED_PROP).is_none(),
        "…and it must NOT also carry Checked — two names for one state is the \
         thing this rename ends"
    );

    for ct in [ControlType::CheckBox, ControlType::Switch] {
        let c = Control::new("C", ct.clone(), 0, 0);
        assert!(
            c.get_prop(CHECKED_PROP).is_some(),
            "{ct:?} keeps Checked — the rename is the RadioButton's only"
        );
        assert!(c.get_prop(SELECTED_PROP).is_none(), "{ct:?} gains no Selected");
    }

    assert_eq!(selection_property(&ControlType::RadioButton), "Selected");
    assert_eq!(selection_property(&ControlType::CheckBox), "Checked");
    assert_eq!(selection_property(&ControlType::Switch), "Checked");
}

#[test]
fn a_legacy_cfrm_migrates_checked_to_selected_on_load() {
    // Exactly the shape every existing project has on disk.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="400" height="300">
  <Control id="RadioButton-1" type="RadioButton" x="10" y="10" w="120" h="45" visible="true" enabled="true">
    <Property name="Caption">English</Property>
    <Property name="Checked">true</Property>
    <Property name="GroupName">lang-grp</Property>
  </Control>
  <Control id="CheckBox-1" type="CheckBox" x="10" y="60" w="120" h="22" visible="true" enabled="true">
    <Property name="Checked">true</Property>
  </Control>
</Form>"#;
    let form = cobolt_forms::xml::load_form_from_str(xml).expect("loads");
    let radio = form
        .controls
        .iter()
        .find(|c| c.id == "RadioButton-1")
        .expect("radio");

    assert_eq!(
        radio.get_prop(SELECTED_PROP).map(|v| v.as_bool()),
        Some(true),
        "the legacy Checked value moves to Selected, keeping its value"
    );
    assert!(
        radio.get_prop(CHECKED_PROP).is_none(),
        "and the legacy key is gone, so nothing downstream can read a stale copy"
    );
    assert!(
        toggle_state_of(radio),
        "the migrated radio still reads as selected"
    );

    // A CheckBox in the same file is untouched.
    let cb = form.controls.iter().find(|c| c.id == "CheckBox-1").unwrap();
    assert_eq!(cb.get_prop(CHECKED_PROP).map(|v| v.as_bool()), Some(true));
    assert!(cb.get_prop(SELECTED_PROP).is_none());
}

#[test]
fn toggle_state_reads_either_spelling() {
    // Canonical.
    let mut radio = Control::new("RB", ControlType::RadioButton, 0, 0);
    radio.set_prop(SELECTED_PROP, PropValue::Bool(true));
    assert!(toggle_state_of(&radio));

    // Legacy only — a control built by older code in memory, never through the
    // XML migration. It must still read as on.
    let mut legacy = Control::new("RB2", ControlType::RadioButton, 0, 0);
    legacy.properties.shift_remove(SELECTED_PROP);
    legacy.set_prop(CHECKED_PROP, PropValue::Bool(true));
    assert!(
        toggle_state_of(&legacy),
        "a radio carrying only the legacy Checked still reads as selected"
    );

    // A check box is read by its own name.
    let mut cb = Control::new("CB", ControlType::CheckBox, 0, 0);
    cb.set_prop(CHECKED_PROP, PropValue::Bool(true));
    assert!(toggle_state_of(&cb));
}

#[test]
fn a_selected_write_still_raises_the_change_events() {
    // The property grid and the runtime both ask which events a write fires;
    // the radio's own name must be recognised, and the legacy one too.
    let radio = Control::new("RB", ControlType::RadioButton, 0, 0);
    let canonical = radio.control_type.observer_events_for(SELECTED_PROP);
    let legacy = radio.control_type.observer_events_for(CHECKED_PROP);
    assert!(
        !canonical.is_empty(),
        "a write to a radio's own Selected must raise its change events"
    );
    assert_eq!(
        canonical, legacy,
        "both spellings must raise exactly the same events, or a pre-rename          handler would silently stop firing"
    );
    // A property that is not the state raises nothing, so the match above is
    // not simply answering yes to everything.
    assert!(radio.control_type.observer_events_for("Caption").is_empty());
    println!(
        "\n  RadioButton state property — seeds `Selected`; a legacy .cfrm's \
         `Checked` migrates on load (value preserved, legacy key dropped); \
         both spellings read back and both raise the change events; CheckBox \
         and Switch keep `Checked` untouched.\n"
    );
}
