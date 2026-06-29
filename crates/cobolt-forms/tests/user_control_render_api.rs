#![cfg(feature = "render")]

use std::collections::HashMap;

use cobolt_forms::render::{
    merge_props, read_user_control_property, resolve_user_control_property_path, FormState,
};
use cobolt_forms::{Control, ControlType, PropValue, Rect};

fn ctrl(id: &str, control_type: ControlType, x: i32, y: i32, w: i32, h: i32) -> Control {
    let mut control = Control::new(id, control_type, x, y);
    control.rect = Rect::new(x, y, w, h);
    control
}

#[test]
fn user_control_property_path_resolves_to_qualified_child() {
    assert_eq!(
        resolve_user_control_property_path("CustomerCard-1", "Button1.Caption"),
        Some(("CustomerCard-1-Button1".to_owned(), "Caption".to_owned()))
    );
    assert_eq!(
        resolve_user_control_property_path("CustomerCard-1", "Caption"),
        Some(("CustomerCard-1".to_owned(), "Caption".to_owned()))
    );
    assert_eq!(
        resolve_user_control_property_path("CustomerCard-1", "Button1."),
        None
    );
}

#[test]
fn read_user_control_property_reads_live_child_value() {
    let root = ctrl("CustomerCard-1", ControlType::GroupBox, 5, 6, 100, 60);
    let mut child = ctrl(
        "CustomerCard-1-Button1",
        ControlType::Button,
        10,
        12,
        80,
        24,
    );
    child.parent = Some("CustomerCard-1".to_owned());
    child.set_prop("Caption".to_owned(), PropValue::String("Default".into()));
    let controls = vec![root, child];

    struct State(HashMap<String, HashMap<String, String>>);
    impl FormState for State {
        fn live(&self, base: &Control) -> Control {
            match self.0.get(&base.id) {
                Some(props) => merge_props(base, props.iter()),
                None => base.clone(),
            }
        }
    }

    let mut overrides = HashMap::new();
    let mut child_props = HashMap::new();
    child_props.insert("Caption".to_owned(), "Live".to_owned());
    overrides.insert("CustomerCard-1-Button1".to_owned(), child_props);

    assert_eq!(
        read_user_control_property(
            &controls,
            &State(overrides),
            "CustomerCard-1",
            "Button1.Caption"
        ),
        Some("Live".to_owned())
    );
}
