// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Auto-assign Form Designer controls from PIC clauses (R15).

use cobolt_forms::ControlType;

use crate::model::IndexedField;

/// Pick a default grid-browser control from field PIC/usage.
pub fn default_control_for_field(field: &IndexedField) -> ControlType {
    // Strip custom marker (if any) before inspecting PIC for control defaults.
    let pic = field
        .pic
        .trim_start_matches('\u{200B}')
        .to_ascii_uppercase();
    if pic == "9" || pic == "9(1)" {
        return ControlType::CheckBox;
    }
    if pic.contains('Z') || pic.contains('$') || pic.contains('.') {
        if pic.chars().any(|c| c == '9' || c == 'Z') {
            return ControlType::NumericUpDown;
        }
    }
    if pic.starts_with("9") {
        return ControlType::NumericUpDown;
    }
    if pic.contains('/') || pic.contains('-') {
        // crude date-edited detection
        if pic.contains('9') {
            return ControlType::DateTimePicker;
        }
    }
    ControlType::TextBox
}

/// Apply default controls to all leaves that have none set.
pub fn apply_default_controls(fields: &mut [IndexedField]) {
    for f in fields {
        if f.children.is_empty() {
            if f.grid_control.is_none() && f.offset.is_some() {
                f.grid_control = Some(default_control_for_field(f));
            }
        } else {
            apply_default_controls(&mut f.children);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FieldUsage;

    fn leaf(pic: &str) -> IndexedField {
        IndexedField {
            level: 5,
            name: "F".into(),
            pic: pic.into(),
            usage: FieldUsage::Display,
            offset: Some(0),
            length: Some(1),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn indicator_is_checkbox() {
        assert_eq!(default_control_for_field(&leaf("9")), ControlType::CheckBox);
    }

    #[test]
    fn numeric_is_spinner() {
        assert_eq!(
            default_control_for_field(&leaf("9(8)")),
            ControlType::NumericUpDown
        );
    }

    #[test]
    fn alpha_is_textbox() {
        assert_eq!(
            default_control_for_field(&leaf("X(20)")),
            ControlType::TextBox
        );
    }
}
