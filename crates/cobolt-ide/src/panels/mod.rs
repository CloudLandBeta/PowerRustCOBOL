// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE panel modules.

pub mod agents_modal;
pub mod beautify;
pub(crate) mod fixed_dialog;
mod beautify_style;
pub mod magnifier;
pub mod claude_code_settings;
pub mod cobol_structure;
pub mod code_search;
pub mod connect_agent_wizard;
pub mod containers;
pub mod data_binding;
pub mod data_grid_columns;
pub(crate) mod datagrid_settings;
pub mod debugger;
pub mod designer;
pub mod doc_viewer;
pub mod editor;
pub mod empty_blocks;
pub mod external_crates;
pub mod forms_list;
pub(crate) mod grid_edit;
pub mod icon_picker;
pub mod item_ids;
pub mod grace_chat;
pub mod indexed_editor;
pub mod indexed_engine_modal;
pub mod indexed_field_control;
pub mod indexed_grid;
pub mod import_keys_modal;
pub mod indexed_new_dialog;
pub mod indexed_properties;
pub mod layer_tabs;
pub(crate) mod layout_grid_edit;
pub mod leaderboard_modal;
pub mod md_render;
pub mod model_search;
pub mod models_modal;
/// The New Project dialog (WASM-001).
pub mod new_project;
pub mod objects_list;
pub mod output;
pub mod project;
pub mod prompt_review;
pub mod properties;
pub mod settings_form;
/// The spotlight shared by the Walkthrough and the Task Wizard (spec 104).
pub mod spotlight;
pub mod sql_connections;
pub mod tab_order;
pub mod target_picker;
/// The Task Wizard (spec 104).
pub mod task_wizard;
pub mod theme_defaults_modal;

pub mod toolbar;
// The ToolBar control's editor — groups, buttons and their actions. Every option
// a toolbar has lives in this modal, not in the properties pane.
pub mod toolbar_editor;
pub mod toolbox;
pub mod walkthrough;
/// The web Preview: the build job, the server, the web view and its controls (WASM-008).
pub mod web_preview;
/// The settings of a Web or Mobile project (WASM-001, WASM-002).
pub mod web_settings;

pub(crate) const CHAT_SEND_BUTTON_WIDTH: f32 = 96.0;

pub(crate) fn chat_prompt_width(available_width: f32, item_spacing: f32) -> f32 {
    (available_width - CHAT_SEND_BUTTON_WIDTH - item_spacing).max(48.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_composer_reserves_the_send_button_on_the_right() {
        let available = 640.0;
        let item_spacing = 8.0;
        let prompt = chat_prompt_width(available, item_spacing);

        assert_eq!(prompt, 536.0);
        assert_eq!(prompt + item_spacing + CHAT_SEND_BUTTON_WIDTH, available);
    }
}

// Spec 096 — the Properties pane's 3D perspective section, drawn for every control type in
// every language.
#[cfg(test)]
mod properties_perspective_096_tests;

// Spec 104 — the Task Wizard where it touches the IDE's own surfaces.
#[cfg(test)]
mod task_wizard_104_tests;
