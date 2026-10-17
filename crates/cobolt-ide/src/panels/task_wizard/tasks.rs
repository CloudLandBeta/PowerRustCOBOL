// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The tasks, as data (spec 104 R37).
//!
//! A task is a [`TaskDef`]: a key, its strings and an ordered list of [`StepDef`]s. Each step
//! names what it points at, what it lights, what to reopen when that is closed, what it says,
//! when it is complete, when it applies and what *Do this for me* does. The engine
//! (`super::Engine`) runs any table of them, so adding a task over surfaces the wizard
//! already knows is adding an entry here — `tests::a_fixture_task_runs_without_engine_changes`
//! in the engine proves it with a task that exists only in the test.

use super::anchors::AnchorId;
use super::conditions as c;
use super::facts::{ControlKind, Facts, Selected};
use crate::i18n::Tr;

/// What a condition reads.
pub struct Ctx<'a> {
    /// The IDE now.
    pub now: &'a Facts,
    /// The IDE when the step began — for "did it change while the step showed?".
    pub entry: &'a Facts,
    /// The control the task is about, once a step has captured it.
    pub subject: Option<&'a Selected>,
}

/// A predicate over the IDE.
pub type Cond = fn(&Ctx) -> bool;

/// Which keys a step lets through (R23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPolicy {
    /// Only to a widget focused inside the live region.
    LiveRegion,
    /// To the whole window the step is in — a canvas step, where the arrows and Delete are
    /// how the developer works.
    Viewport,
}

/// What *Do this for me* does. The IDE performs it through the same code path as the
/// developer's own click (R25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show Project Settings in the Main Pane.
    OpenProjectSettings,
    /// Put the provider's default endpoint into the manager's empty endpoint field — what its
    /// *Use the default* button does.
    ResetEndpoint,
    /// Add a column to the subject grid in grid-edit mode (one undo entry).
    AddGridColumn,
    /// Put a starter item into the menu editor's empty draft (a SideMenu's *Home*).
    StarterSideMenu,
    /// Put starter items into the menu editor's empty draft (a MenuBar's File / Edit / Help).
    StarterMenuBar,
    /// Add an animation to the subject (one undo entry).
    AddAnimation,
}

/// A *Do this for me*: the action, and when it may be offered — only while what it fills is
/// empty (R26).
#[derive(Clone, Copy)]
pub struct Mechanical {
    pub action: Action,
    pub available: Cond,
    /// What the balloon says it did.
    pub said: fn(&Tr) -> &'static str,
}

/// One step.
#[derive(Clone, Copy)]
pub struct StepDef {
    /// Stable across releases: progress is stored by it (R29).
    pub key: &'static str,
    /// What the ring goes round.
    pub ring: AnchorId,
    /// What stays live; `None` means the ring's own rect.
    pub live: Option<AnchorId>,
    /// What to light, saying "Reopen…", when the ring's container is closed or on another tab
    /// (R20); the first of them on screen is used.
    pub via: &'static [AnchorId],
    pub title: fn(&Tr) -> &'static str,
    pub body: fn(&Tr) -> &'static str,
    /// When the step is complete. `None`: an explanation, finished by **Next** and never
    /// counted as skipped.
    pub done: Option<Cond>,
    /// When the step applies (R19). A step that does not is skipped and not counted.
    pub applies: Cond,
    pub mechanical: Option<Mechanical>,
    pub keys: KeyPolicy,
    /// Leaving this step forward takes the selected control as the task's subject.
    pub captures: bool,
}

/// One task.
pub struct TaskDef {
    /// Stable across releases: *done* and progress are stored by it (R29).
    pub key: &'static str,
    pub title: fn(&Tr) -> &'static str,
    /// The catalogue's one-line description.
    pub blurb: fn(&Tr) -> &'static str,
    /// Where the result lives, for the summary (R21).
    pub result: fn(&Tr) -> &'static str,
    /// The control a selection offers this task for (R10), if any.
    pub offer_for: Option<ControlKind>,
    pub steps: &'static [StepDef],
}

impl TaskDef {
    /// The index of the step keyed `key`.
    pub fn step_index(&self, key: &str) -> Option<usize> {
        self.steps.iter().position(|s| s.key == key)
    }
}

/// A step with the defaults most steps share; the table overrides what differs.
const fn step(
    key: &'static str,
    ring: AnchorId,
    title: fn(&Tr) -> &'static str,
    body: fn(&Tr) -> &'static str,
) -> StepDef {
    StepDef {
        key,
        ring,
        live: None,
        via: &[],
        title,
        body,
        done: None,
        applies: c::always,
        mechanical: None,
        keys: KeyPolicy::LiveRegion,
        captures: false,
    }
}

use AnchorId as A;
use KeyPolicy::Viewport;

// ── Steps shared by several tasks ────────────────────────────────────────────────────────

/// Show Project Settings — skipped when it already shows, which is how a project opens.
const SETTINGS: StepDef = StepDef {
    done: Some(c::settings_open),
    applies: c::settings_closed,
    mechanical: Some(Mechanical {
        action: Action::OpenProjectSettings,
        available: c::settings_closed,
        said: |t| t.task_wizard_did_settings,
    }),
    ..step("settings", A::ProjectRoot, |t| t.task_wizard_settings_title, |t| t.task_wizard_settings_body)
};

/// Open a form — skipped when a Form Designer is already open.
const OPEN_FORM: StepDef = StepDef {
    done: Some(c::designer_open),
    applies: c::no_designer_open,
    ..step("open_form", A::FormsCategory, |t| t.task_wizard_open_form_title, |t| t.task_wizard_open_form_body)
};

/// Select the task's control on the canvas; leaving it forward makes that control the subject.
const fn select(done: Cond, body: fn(&Tr) -> &'static str) -> StepDef {
    StepDef {
        done: Some(done),
        keys: Viewport,
        captures: true,
        ..step("select", A::Canvas, |t| t.task_wizard_select_title, body)
    }
}

/// Where a Properties step's target goes when the pane is hidden or on another tab.
const PROPS_VIA: &[AnchorId] = &[A::PropertiesReopen, A::PropsTabVisuals];

/// A step lit inside the Properties pane.
const fn in_props(
    key: &'static str,
    ring: AnchorId,
    title: fn(&Tr) -> &'static str,
    body: fn(&Tr) -> &'static str,
) -> StepDef {
    StepDef {
        live: Some(A::PropertiesPane),
        via: PROPS_VIA,
        applies: c::has_subject,
        ..step(key, ring, title, body)
    }
}

/// A step lit inside a window, reopened from `via` when it is closed.
const fn in_window(
    key: &'static str,
    ring: AnchorId,
    window: AnchorId,
    via: &'static [AnchorId],
    title: fn(&Tr) -> &'static str,
    body: fn(&Tr) -> &'static str,
) -> StepDef {
    StepDef {
        live: Some(window),
        via,
        ..step(key, ring, title, body)
    }
}

const MODELS_VIA: &[AnchorId] = &[A::SettingsModelProviders, A::ProjectRoot];
const AGENTS_VIA: &[AnchorId] = &[A::SettingsManageAgents, A::ProjectRoot];
const MENU_VIA: &[AnchorId] = &[A::MenuEditButton, A::PropertiesReopen, A::PropsTabVisuals];

const MENU_EDIT: StepDef = StepDef {
    done: Some(c::menu_editor_open),
    ..in_props("edit", A::MenuEditButton, |t| t.task_wizard_menu_edit_title, |t| t.task_wizard_menu_edit_body)
};

const MENU_SAVE: StepDef = StepDef {
    done: Some(c::menu_saved),
    applies: c::has_subject,
    ..in_window("save", A::MenuSave, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_save_title, |t| t.task_wizard_menu_save_body)
};

const BINDING_APPLY: StepDef = StepDef {
    done: Some(c::binding_applied),
    applies: c::has_subject,
    ..in_window("apply", A::BindingApply, A::BindingWindow, &[A::PropertiesBinding], |t| t.task_wizard_apply_title, |t| t.task_wizard_apply_body)
};

// ── The tasks ────────────────────────────────────────────────────────────────────────────

pub static TASKS: &[TaskDef] = &[
    TaskDef {
        key: "models",
        title: |t| t.task_wizard_models_title,
        blurb: |t| t.task_wizard_models_blurb,
        result: |t| t.task_wizard_models_result,
        offer_for: None,
        steps: &[
            SETTINGS,
            StepDef {
                done: Some(c::models_open),
                via: &[A::ProjectRoot],
                ..step("open", A::SettingsModelProviders, |t| t.task_wizard_models_open_title, |t| t.task_wizard_models_open_body)
            },
            StepDef {
                done: Some(c::provider_changed),
                ..in_window("provider", A::ModelsProviderList, A::ModelsWindow, MODELS_VIA, |t| t.task_wizard_provider_title, |t| t.task_wizard_provider_body)
            },
            StepDef {
                done: Some(c::endpoint_filled),
                mechanical: Some(Mechanical {
                    action: Action::ResetEndpoint,
                    available: c::endpoint_empty,
                    said: |t| t.task_wizard_did_endpoint,
                }),
                ..in_window("endpoint", A::ModelsEndpoint, A::ModelsWindow, MODELS_VIA, |t| t.task_wizard_endpoint_title, |t| t.task_wizard_endpoint_body)
            },
            StepDef {
                done: Some(c::test_passed),
                applies: c::provider_needs_key,
                ..in_window("key", A::ModelsKeyAndTest, A::ModelsWindow, MODELS_VIA, |t| t.task_wizard_key_title, |t| t.task_wizard_key_body)
            },
            StepDef {
                done: Some(c::models_saved),
                ..in_window("save", A::ModelsSave, A::ModelsWindow, MODELS_VIA, |t| t.task_wizard_save_title, |t| t.task_wizard_models_save_body)
            },
        ],
    },
    TaskDef {
        key: "agents",
        title: |t| t.task_wizard_agents_title,
        blurb: |t| t.task_wizard_agents_blurb,
        result: |t| t.task_wizard_agents_result,
        offer_for: None,
        steps: &[
            SETTINGS,
            StepDef {
                done: Some(c::agents_open),
                via: &[A::ProjectRoot],
                ..step("open", A::SettingsManageAgents, |t| t.task_wizard_agents_open_title, |t| t.task_wizard_agents_open_body)
            },
            StepDef {
                done: Some(c::agents_model_tab),
                applies: c::agents_other_tab,
                ..in_window("tab", A::AgentsModelTab, A::AgentsWindow, AGENTS_VIA, |t| t.task_wizard_agents_tab_title, |t| t.task_wizard_agents_tab_body)
            },
            StepDef {
                done: Some(c::agents_scope_changed),
                ..in_window("scope", A::AgentsScope, A::AgentsWindow, AGENTS_VIA, |t| t.task_wizard_agents_scope_title, |t| t.task_wizard_agents_scope_body)
            },
            StepDef {
                done: Some(c::agents_models_changed),
                ..in_window("model", A::AgentsGrid, A::AgentsWindow, AGENTS_VIA, |t| t.task_wizard_agents_model_title, |t| t.task_wizard_agents_model_body)
            },
            StepDef {
                done: Some(c::agents_closed),
                ..in_window("save", A::AgentsSave, A::AgentsWindow, &[], |t| t.task_wizard_save_title, |t| t.task_wizard_agents_save_body)
            },
        ],
    },
    TaskDef {
        key: "binding",
        title: |t| t.task_wizard_binding_title,
        blurb: |t| t.task_wizard_binding_blurb,
        result: |t| t.task_wizard_binding_result,
        offer_for: None,
        steps: &[
            OPEN_FORM,
            select(c::selected_bindable, |t| t.task_wizard_select_bindable_body),
            StepDef {
                done: Some(c::binding_editor_open),
                ..in_props("section", A::PropertiesBinding, |t| t.task_wizard_binding_section_title, |t| t.task_wizard_binding_section_body)
            },
            StepDef {
                applies: c::has_subject,
                ..in_window("source", A::BindingSource, A::BindingWindow, &[A::PropertiesBinding], |t| t.task_wizard_binding_source_title, |t| t.task_wizard_binding_source_body)
            },
            StepDef {
                applies: c::has_subject,
                ..in_window("fields", A::BindingFields, A::BindingWindow, &[A::PropertiesBinding], |t| t.task_wizard_binding_fields_title, |t| t.task_wizard_binding_fields_body)
            },
            BINDING_APPLY,
        ],
    },
    TaskDef {
        key: "datagrid",
        title: |t| t.task_wizard_grid_title,
        blurb: |t| t.task_wizard_grid_blurb,
        result: |t| t.task_wizard_grid_result,
        offer_for: Some(ControlKind::DataGrid),
        steps: &[
            OPEN_FORM,
            select(c::selected_datagrid, |t| t.task_wizard_select_grid_body),
            StepDef {
                done: Some(c::binding_editor_open),
                applies: c::subject_unbound,
                ..in_props("bind", A::PropertiesBinding, |t| t.task_wizard_grid_bind_title, |t| t.task_wizard_grid_bind_body)
            },
            StepDef {
                applies: c::binding_editor_open,
                ..BINDING_APPLY
            },
            StepDef {
                done: Some(c::grid_settings_open),
                ..in_props("settings", A::GridSettingsButton, |t| t.task_wizard_grid_settings_title, |t| t.task_wizard_grid_settings_body)
            },
            StepDef {
                done: Some(c::grid_columns_changed),
                applies: c::has_subject,
                ..in_window("columns", A::GridSettingsColumns, A::GridSettingsWindow, &[A::GridSettingsButton], |t| t.task_wizard_grid_columns_title, |t| t.task_wizard_grid_columns_body)
            },
            StepDef {
                done: Some(c::grid_settings_closed),
                applies: c::has_subject,
                ..in_window("close", A::GridSettingsWindow, A::GridSettingsWindow, &[], |t| t.task_wizard_grid_close_title, |t| t.task_wizard_grid_close_body)
            },
            StepDef {
                done: Some(c::grid_edit_mode),
                applies: c::subject_unbound,
                live: Some(A::Canvas),
                keys: Viewport,
                ..step("canvas", A::SubjectOnCanvas, |t| t.task_wizard_grid_canvas_title, |t| t.task_wizard_grid_canvas_body)
            },
            StepDef {
                done: Some(c::grid_column_added),
                applies: c::grid_edit_mode,
                live: Some(A::Canvas),
                keys: Viewport,
                mechanical: Some(Mechanical {
                    action: Action::AddGridColumn,
                    available: c::grid_edit_mode,
                    said: |t| t.task_wizard_did_column,
                }),
                ..step("add_column", A::GridEditToolbar, |t| t.task_wizard_grid_add_title, |t| t.task_wizard_grid_add_body)
            },
        ],
    },
    TaskDef {
        key: "treeview",
        title: |t| t.task_wizard_tree_title,
        blurb: |t| t.task_wizard_tree_blurb,
        result: |t| t.task_wizard_tree_result,
        offer_for: Some(ControlKind::TreeView),
        steps: &[
            OPEN_FORM,
            select(c::selected_treeview, |t| t.task_wizard_select_tree_body),
            StepDef {
                done: Some(c::tree_items_changed),
                ..in_props("items", A::TreeItems, |t| t.task_wizard_tree_items_title, |t| t.task_wizard_tree_items_body)
            },
            in_props("behaviour", A::PropertiesPane, |t| t.task_wizard_tree_behaviour_title, |t| t.task_wizard_tree_behaviour_body),
            StepDef {
                done: Some(c::events_tab),
                via: &[A::PropertiesReopen],
                ..in_props("events", A::PropsTabEvents, |t| t.task_wizard_tree_events_title, |t| t.task_wizard_tree_events_body)
            },
        ],
    },
    TaskDef {
        key: "sidemenu",
        title: |t| t.task_wizard_side_title,
        blurb: |t| t.task_wizard_side_blurb,
        result: |t| t.task_wizard_menu_result,
        offer_for: Some(ControlKind::SideMenu),
        steps: &[
            OPEN_FORM,
            select(c::selected_sidemenu, |t| t.task_wizard_select_side_body),
            MENU_EDIT,
            StepDef {
                done: Some(c::menu_item_added),
                applies: c::has_subject,
                mechanical: Some(Mechanical {
                    action: Action::StarterSideMenu,
                    available: c::menu_draft_empty,
                    said: |t| t.task_wizard_did_side_starter,
                }),
                ..in_window("add", A::MenuAddItem, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_menu_add_title, |t| t.task_wizard_menu_add_body)
            },
            StepDef {
                done: Some(c::menu_has_target),
                applies: c::has_subject,
                ..in_window("target", A::MenuItemTarget, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_menu_target_title, |t| t.task_wizard_side_target_body)
            },
            MENU_SAVE,
        ],
    },
    TaskDef {
        key: "menubar",
        title: |t| t.task_wizard_bar_title,
        blurb: |t| t.task_wizard_bar_blurb,
        result: |t| t.task_wizard_menu_result,
        offer_for: Some(ControlKind::MenuBar),
        steps: &[
            OPEN_FORM,
            select(c::selected_menubar, |t| t.task_wizard_select_bar_body),
            MENU_EDIT,
            StepDef {
                done: Some(c::menu_item_added),
                applies: c::has_subject,
                mechanical: Some(Mechanical {
                    action: Action::StarterMenuBar,
                    available: c::menu_draft_empty,
                    said: |t| t.task_wizard_did_bar_starter,
                }),
                ..in_window("add", A::MenuAddItem, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_menu_add_title, |t| t.task_wizard_menu_add_body)
            },
            StepDef {
                done: Some(c::menu_has_target),
                applies: c::has_subject,
                ..in_window("target", A::MenuItemTarget, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_menu_target_title, |t| t.task_wizard_bar_target_body)
            },
            StepDef {
                done: Some(c::menu_has_shortcut),
                applies: c::has_subject,
                keys: Viewport,
                ..in_window("shortcut", A::MenuItemShortcut, A::MenuEditorWindow, MENU_VIA, |t| t.task_wizard_bar_shortcut_title, |t| t.task_wizard_bar_shortcut_body)
            },
            MENU_SAVE,
        ],
    },
    // Operator, 2026-10-17: "add a wizard for the map". Where it opens and how close, pins
    // from a table, its colours, and its events — where pins, routes and the Google-backed
    // lookups are written. No step does anything for the developer: a location is theirs.
    TaskDef {
        key: "maps",
        title: |t| t.task_wizard_map_title,
        blurb: |t| t.task_wizard_map_blurb,
        result: |t| t.task_wizard_map_result,
        offer_for: Some(ControlKind::Maps),
        steps: &[
            OPEN_FORM,
            select(c::selected_maps, |t| t.task_wizard_select_map_body),
            StepDef {
                done: Some(c::map_center_changed),
                ..in_props("center", A::MapCenter, |t| t.task_wizard_map_center_title, |t| t.task_wizard_map_center_body)
            },
            StepDef {
                done: Some(c::map_zoom_changed),
                ..in_props("zoom", A::MapZoom, |t| t.task_wizard_map_zoom_title, |t| t.task_wizard_map_zoom_body)
            },
            StepDef {
                done: Some(c::binding_editor_open),
                applies: c::subject_unbound,
                ..in_props("pins", A::PropertiesBinding, |t| t.task_wizard_map_pins_title, |t| t.task_wizard_map_pins_body)
            },
            StepDef {
                applies: c::binding_editor_open,
                ..BINDING_APPLY
            },
            StepDef {
                done: Some(c::map_colors_changed),
                ..in_props("colours", A::MapColors, |t| t.task_wizard_map_colours_title, |t| t.task_wizard_map_colours_body)
            },
            StepDef {
                done: Some(c::events_tab),
                via: &[A::PropertiesReopen],
                ..in_props("events", A::PropsTabEvents, |t| t.task_wizard_map_events_title, |t| t.task_wizard_map_events_body)
            },
        ],
    },
    TaskDef {
        key: "animation",
        title: |t| t.task_wizard_anim_title,
        blurb: |t| t.task_wizard_anim_blurb,
        result: |t| t.task_wizard_anim_result,
        offer_for: None,
        steps: &[
            OPEN_FORM,
            select(c::selected_any, |t| t.task_wizard_select_any_body),
            StepDef {
                done: Some(c::anim_tab),
                via: &[A::PropertiesReopen],
                ..in_props("tab", A::AnimTab, |t| t.task_wizard_anim_tab_title, |t| t.task_wizard_anim_tab_body)
            },
            StepDef {
                done: Some(c::animation_added),
                via: &[A::PropertiesReopen, A::AnimTab],
                mechanical: Some(Mechanical {
                    action: Action::AddAnimation,
                    available: c::subject_has_no_animation,
                    said: |t| t.task_wizard_did_animation,
                }),
                ..in_props("add", A::AnimAdd, |t| t.task_wizard_anim_add_title, |t| t.task_wizard_anim_add_body)
            },
            StepDef {
                done: Some(c::animation_changed),
                via: &[A::PropertiesReopen, A::AnimTab],
                applies: c::subject_has_animation,
                ..in_props("fields", A::AnimFields, |t| t.task_wizard_anim_fields_title, |t| t.task_wizard_anim_fields_body)
            },
            StepDef {
                done: Some(c::animation_previewed),
                via: &[A::PropertiesReopen, A::AnimTab],
                applies: c::subject_has_animation,
                ..in_props("preview", A::AnimPreview, |t| t.task_wizard_anim_preview_title, |t| t.task_wizard_anim_preview_body)
            },
        ],
    },
];
