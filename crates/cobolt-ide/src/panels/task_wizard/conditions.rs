// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The conditions the task table is written in (spec 104 R17, R19).
//!
//! Each is a plain function over [`Ctx`], so the table stays data. A condition says what is
//! true of the IDE; the engine decides what that means — it advances only when one becomes
//! true *while its step shows* (R17), which is why "changed since the step began" conditions
//! compare `now` with `entry`.

use super::facts::ControlKind;
use super::tasks::Ctx;

pub fn always(_: &Ctx) -> bool {
    true
}

// ── Project and designers ────────────────────────────────────────────────────────────────

/// At least one Form Designer is open.
pub fn designer_open(x: &Ctx) -> bool {
    !x.now.designers.is_empty()
}

/// No Form Designer is open — the *open a form* step applies.
pub fn no_designer_open(x: &Ctx) -> bool {
    x.now.designers.is_empty()
}

/// Exactly one bindable control is selected.
pub fn selected_bindable(x: &Ctx) -> bool {
    x.now.selected().is_some_and(|s| s.kind.bindable())
}

/// Exactly one control of `kind` is selected.
fn selected_kind(x: &Ctx, kind: ControlKind) -> bool {
    x.now.selected().is_some_and(|s| s.kind == kind)
}

pub fn selected_datagrid(x: &Ctx) -> bool {
    selected_kind(x, ControlKind::DataGrid)
}

pub fn selected_treeview(x: &Ctx) -> bool {
    selected_kind(x, ControlKind::TreeView)
}

pub fn selected_sidemenu(x: &Ctx) -> bool {
    selected_kind(x, ControlKind::SideMenu)
}

pub fn selected_menubar(x: &Ctx) -> bool {
    selected_kind(x, ControlKind::MenuBar)
}

pub fn selected_maps(x: &Ctx) -> bool {
    selected_kind(x, ControlKind::Maps)
}

/// Exactly one control is selected, of any kind.
pub fn selected_any(x: &Ctx) -> bool {
    x.now.selected().is_some()
}

/// The task has a subject that still exists.
pub fn has_subject(x: &Ctx) -> bool {
    x.subject.is_some() && x.now.subject.exists
}

// ── Task 1: a model provider ─────────────────────────────────────────────────────────────

pub fn settings_open(x: &Ctx) -> bool {
    x.now.settings_open
}

pub fn settings_closed(x: &Ctx) -> bool {
    !x.now.settings_open
}

pub fn models_open(x: &Ctx) -> bool {
    x.now.models_open
}

/// The developer picked a different provider while the step showed.
pub fn provider_changed(x: &Ctx) -> bool {
    x.now.models_provider.is_some() && x.now.models_provider != x.entry.models_provider
}

pub fn endpoint_filled(x: &Ctx) -> bool {
    x.now.models_open && !x.now.models_endpoint_empty
}

pub fn endpoint_empty(x: &Ctx) -> bool {
    x.now.models_open && x.now.models_endpoint_empty
}

/// The provider showing takes a key.
pub fn provider_needs_key(x: &Ctx) -> bool {
    x.now.models_provider_needs_key
}

/// The connection test passed — the key step's only completion signal, since the wizard
/// knows nothing about the key itself (R28).
pub fn test_passed(x: &Ctx) -> bool {
    x.now.models_test == super::facts::TestOutcome::Passed
}

/// The manager saved while the step showed.
pub fn models_saved(x: &Ctx) -> bool {
    x.now.models_saves > x.entry.models_saves
}

// ── Task 2: an agent and its model ──────────────────────────────────────────────────────

pub fn agents_open(x: &Ctx) -> bool {
    x.now.agents_open
}

pub fn agents_closed(x: &Ctx) -> bool {
    !x.now.agents_open
}

pub fn agents_model_tab(x: &Ctx) -> bool {
    x.now.agents_open && x.now.agents_model_tab
}

/// The manager is on another tab — the *open the tab* step applies.
pub fn agents_other_tab(x: &Ctx) -> bool {
    !agents_model_tab(x)
}

pub fn agents_scope_changed(x: &Ctx) -> bool {
    x.now.agents_scope.is_some() && x.now.agents_scope != x.entry.agents_scope
}

pub fn agents_models_changed(x: &Ctx) -> bool {
    x.now.agents_models_sig != x.entry.agents_models_sig
}

// ── The subject ──────────────────────────────────────────────────────────────────────────

pub fn subject_unbound(x: &Ctx) -> bool {
    has_subject(x) && !x.now.subject.bound
}

pub fn binding_editor_open(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.binding_editor_open
}

/// A binding was applied (or replaced) while the step showed.
pub fn binding_applied(x: &Ctx) -> bool {
    has_subject(x)
        && x.now.subject.bound
        && x.now.subject.binding_sig != x.entry.subject.binding_sig
}

pub fn grid_settings_open(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.grid_settings_open
}

pub fn grid_settings_closed(x: &Ctx) -> bool {
    has_subject(x) && !x.now.subject.grid_settings_open
}

pub fn grid_columns_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.grid_columns_sig != x.entry.subject.grid_columns_sig
}

pub fn grid_edit_mode(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.grid_edit_mode
}

pub fn grid_column_added(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.grid_column_count > x.entry.subject.grid_column_count
}

pub fn tree_items_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.tree_items_sig != x.entry.subject.tree_items_sig
}

// ── A map ────────────────────────────────────────────────────────────────────────────────

/// Its centre was moved while the step showed.
pub fn map_center_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.map_center_sig != x.entry.subject.map_center_sig
}

/// Its zoom was changed while the step showed.
pub fn map_zoom_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.map_zoom_sig != x.entry.subject.map_zoom_sig
}

/// One of its colours was changed while the step showed.
pub fn map_colors_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.map_colors_sig != x.entry.subject.map_colors_sig
}

pub fn events_tab(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.events_tab
}

pub fn menu_editor_open(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.menu_editor_open
}

pub fn menu_item_added(x: &Ctx) -> bool {
    menu_editor_open(x) && x.now.subject.menu_draft_items > x.entry.subject.menu_draft_items
}

pub fn menu_draft_empty(x: &Ctx) -> bool {
    menu_editor_open(x) && x.now.subject.menu_draft_items == 0
}

pub fn menu_has_target(x: &Ctx) -> bool {
    menu_editor_open(x) && x.now.subject.menu_draft_has_target
}

pub fn menu_has_shortcut(x: &Ctx) -> bool {
    menu_editor_open(x) && x.now.subject.menu_draft_has_shortcut
}

pub fn menu_saved(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.menu_sig != x.entry.subject.menu_sig
}

pub fn anim_tab(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.anim_tab
}

pub fn animation_added(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.animations > x.entry.subject.animations
}

pub fn subject_has_no_animation(x: &Ctx) -> bool {
    anim_tab(x) && x.now.subject.animations == 0
}

pub fn subject_has_animation(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.animations > 0
}

pub fn animation_changed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.anim_sig != x.entry.subject.anim_sig
}

pub fn animation_previewed(x: &Ctx) -> bool {
    has_subject(x) && x.now.subject.anim_previews > x.entry.subject.anim_previews
}
