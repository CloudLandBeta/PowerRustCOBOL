// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL Structure editor (spec 005).
//!
//! The form's property inspector lists the shared COBOL sections —
//! `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
//! `WORKING-STORAGE` — plus the user procedures (nested programs the event
//! handlers can `CALL`). Selecting one opens this popup, which edits that single
//! block's code. Each block is woven verbatim into the generated program
//! ([`cobolt_codegen`]); the developer writes any `GLOBAL` / `EXTERNAL` clauses.

use cobolt_forms::Form;

use crate::i18n::Tr;

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

fn code_edit(ui: &mut egui::Ui, code: &mut String) -> bool {
    egui::ScrollArea::vertical()
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(code)
                    .code_editor()
                    .desired_rows(16)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        })
        .inner
}

/// Edit one section / user-procedure block in the popup. Returns `true` on edit.
pub fn show_editor(ui: &mut egui::Ui, form: &mut Form, target: CsTarget, tr: &Tr) -> bool {
    let mut changed = false;
    match target {
        CsTarget::Procedure(i) => {
            if i >= form.user_procedures.len() {
                return false;
            }
            let up = &mut form.user_procedures[i];
            ui.horizontal(|ui| {
                ui.label(tr.cs_proc_name);
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut up.name).desired_width(260.0))
                    .changed();
            });
            ui.add_space(4.0);
            changed |= code_edit(ui, &mut up.code);
        }
        section => {
            ui.label(
                egui::RichText::new(section.section_keyword().unwrap_or(""))
                    .monospace()
                    .strong(),
            );
            ui.add_space(4.0);
            let field = match section {
                CsTarget::SpecialNames => &mut form.cobol_structure.special_names,
                CsTarget::Repository => &mut form.cobol_structure.repository,
                CsTarget::FileControl => &mut form.cobol_structure.file_control,
                CsTarget::FileSection => &mut form.cobol_structure.file_section,
                CsTarget::WorkingStorage => &mut form.user_ws_source,
                CsTarget::Procedure(_) => unreachable!(),
            };
            changed |= code_edit(ui, field);
        }
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tr.cs_hint).weak().italics());
    changed
}
