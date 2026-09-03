// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE panel modules.

pub mod agents_modal;
pub mod beautify;
pub mod cobol_structure;
pub mod code_search;
pub mod containers;
pub mod data_binding;
pub mod data_grid_columns;
pub mod debugger;
pub mod designer;
pub mod doc_viewer;
pub mod editor;
pub mod empty_blocks;
pub mod external_crates;
pub mod forms_list;
pub mod icon_picker;
pub mod grace_chat;
pub mod indexed_editor;
pub mod indexed_field_control;
pub mod indexed_grid;
pub mod indexed_new_dialog;
pub mod indexed_properties;
pub mod leaderboard_modal;
pub mod md_render;
pub mod models_modal;
pub mod output;
pub mod project;
pub mod prompt_review;
pub mod properties;
pub mod rounded_clip;
pub mod settings_form;
pub mod target_picker;
pub mod theme_defaults_modal;

pub mod toolbar;
// The ToolBar control's editor — groups, buttons and their actions. Every option
// a toolbar has lives in this modal, not in the properties pane.
pub mod toolbar_editor;
pub mod toolbox;

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
