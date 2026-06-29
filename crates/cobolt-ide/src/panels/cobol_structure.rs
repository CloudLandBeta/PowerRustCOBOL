// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL Structure model helpers (spec 005).
//!
//! The form's property inspector lists the shared COBOL sections —
//! `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
//! `WORKING-STORAGE` — plus the user procedures. Selecting one opens a popup
//! that hosts the **same** COBOL [`EditorPanel`](super::editor::EditorPanel) used
//! everywhere else (IntelliSense, syntax colouring, find/replace) — see
//! `DesignerPanel::show_cobol_structure_window`. This module just describes which
//! block is being edited and reads/writes its text on the form.

use cobolt_forms::Form;

/// Which COBOL Structure block the popup editor is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsTarget {
    SpecialNames,
    Repository,
    FileControl,
    FileSection,
    WorkingStorage,
    /// A user procedure by index into [`Form::user_procedures`].
    Procedure(usize),
}

/// The five fixed structure sections, in division/section order.
pub const SECTIONS: [CsTarget; 5] = [
    CsTarget::SpecialNames,
    CsTarget::Repository,
    CsTarget::FileControl,
    CsTarget::FileSection,
    CsTarget::WorkingStorage,
];

impl CsTarget {
    /// The fixed COBOL section keyword, or `None` for a user procedure (whose
    /// title is its name).
    pub fn section_keyword(self) -> Option<&'static str> {
        Some(match self {
            CsTarget::SpecialNames => "SPECIAL-NAMES",
            CsTarget::Repository => "REPOSITORY",
            CsTarget::FileControl => "FILE-CONTROL",
            CsTarget::FileSection => "FILE SECTION",
            CsTarget::WorkingStorage => "WORKING-STORAGE",
            CsTarget::Procedure(_) => return None,
        })
    }

    /// A stable key for the synthetic editor buffer path of this block.
    pub fn buffer_key(self) -> String {
        match self {
            CsTarget::SpecialNames => "special-names".into(),
            CsTarget::Repository => "repository".into(),
            CsTarget::FileControl => "file-control".into(),
            CsTarget::FileSection => "file-section".into(),
            CsTarget::WorkingStorage => "working-storage".into(),
            CsTarget::Procedure(i) => format!("procedure-{i}"),
        }
    }
}

/// Current text of a fixed section block (not valid for a user procedure).
pub fn section_text(form: &Form, t: CsTarget) -> Option<&str> {
    Some(match t {
        CsTarget::SpecialNames => form.cobol_structure.special_names.as_str(),
        CsTarget::Repository => form.cobol_structure.repository.as_str(),
        CsTarget::FileControl => form.cobol_structure.file_control.as_str(),
        CsTarget::FileSection => form.cobol_structure.file_section.as_str(),
        CsTarget::WorkingStorage => form.user_ws_source.as_str(),
        CsTarget::Procedure(_) => return None,
    })
}

/// The editable code of a block (section text, or a procedure's body).
pub fn block_text(form: &Form, t: CsTarget) -> String {
    match t {
        CsTarget::Procedure(i) => form
            .user_procedures
            .get(i)
            .map(|p| p.code.clone())
            .unwrap_or_default(),
        other => section_text(form, other).unwrap_or_default().to_owned(),
    }
}

/// Write a block's edited code back to the form. Returns whether it changed.
pub fn set_block_text(form: &mut Form, t: CsTarget, text: String) -> bool {
    let slot: &mut String = match t {
        CsTarget::SpecialNames => &mut form.cobol_structure.special_names,
        CsTarget::Repository => &mut form.cobol_structure.repository,
        CsTarget::FileControl => &mut form.cobol_structure.file_control,
        CsTarget::FileSection => &mut form.cobol_structure.file_section,
        CsTarget::WorkingStorage => &mut form.user_ws_source,
        CsTarget::Procedure(i) => match form.user_procedures.get_mut(i) {
            Some(up) => &mut up.code,
            None => return false,
        },
    };
    if *slot != text {
        *slot = text;
        true
    } else {
        false
    }
}
