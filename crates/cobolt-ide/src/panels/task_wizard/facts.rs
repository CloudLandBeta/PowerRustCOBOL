// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the Task Wizard knows about the IDE, one frame at a time (spec 104 R17, R28, R36).
//!
//! The IDE builds a [`Facts`] at the end of every frame the wizard runs, and the engine reads
//! nothing else. That is what makes the engine testable — a test writes a `Facts` by hand —
//! and it is what keeps secrets out: **no field here describes a secret field in any way**,
//! not its value, not its length, not whether it is empty (R28). A step that asks for a key
//! completes on [`TestOutcome::Passed`] or on [`Facts::provider_configured`] instead.
//!
//! Changing values that are expensive to compare (a column layout, a menu, an item list) are
//! carried as a **signature** — a hash of the committed value — so a condition can ask "did
//! it change since the step began?" without the wizard holding the developer's content.

use eframe::egui::ViewportId;

/// The kind of control a task is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlKind {
    DataGrid,
    TreeView,
    SideMenu,
    MenuBar,
    /// Any other control; `bindable` says whether the Data Binding section is offered.
    Other { bindable: bool },
}

impl ControlKind {
    /// Does the Properties pane offer a Data Binding section for it?
    pub fn bindable(self) -> bool {
        match self {
            ControlKind::DataGrid => true,
            ControlKind::Other { bindable } => bindable,
            _ => false,
        }
    }
}

/// The one control selected in a designer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selected {
    /// The form's file path — two open forms may share a name, never a path.
    pub form: String,
    pub control: String,
    pub kind: ControlKind,
}

/// The outcome of the Model Providers Manager's connection test, for the provider showing.
/// Carries no key, no message and no endpoint: only whether it passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestOutcome {
    #[default]
    NotRun,
    Running,
    Passed,
    Failed,
}

/// One open Form Designer.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerFacts {
    pub viewport: ViewportId,
    /// The form's file path, as [`Selected::form`].
    pub form: String,
    /// The most recently focused designer.
    pub focused: bool,
    /// The selection, when it is exactly one control.
    pub selected: Option<Selected>,
}

/// What the wizard can see of the control a task is about (its *subject*), read in the
/// designer that holds it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubjectFacts {
    /// The control still exists in an open designer.
    pub exists: bool,
    /// It is the one control selected.
    pub selected: bool,
    /// It has a data binding.
    pub bound: bool,
    /// Signature of its committed binding (0 when it has none).
    pub binding_sig: u64,
    /// The Data Binding editor is open for it.
    pub binding_editor_open: bool,
    /// The editor has picked a source.
    pub binding_has_source: bool,
    /// The DataGrid settings window is open for it.
    pub grid_settings_open: bool,
    /// Signature of the grid's committed columns and widths.
    pub grid_columns_sig: u64,
    /// How many columns the grid has.
    pub grid_column_count: usize,
    /// The designer is in grid-edit mode on it.
    pub grid_edit_mode: bool,
    /// Signature of the TreeView's committed `Items`.
    pub tree_items_sig: u64,
    /// The Properties pane shows the Events tab.
    pub events_tab: bool,
    /// The menu editor is open for it.
    pub menu_editor_open: bool,
    /// How many items the menu editor's draft holds.
    pub menu_draft_items: usize,
    /// Some draft item has a target form or an action.
    pub menu_draft_has_target: bool,
    /// Some draft item has a shortcut or a handler.
    pub menu_draft_has_shortcut: bool,
    /// Signature of the control's committed menu.
    pub menu_sig: u64,
    /// The Properties pane shows the animations tab.
    pub anim_tab: bool,
    /// How many animations the control has.
    pub animations: usize,
    /// Signature of its committed animations.
    pub anim_sig: u64,
    /// How many times an animation preview was started for it.
    pub anim_previews: u32,
}

/// What the Properties pane shows for one control: which of its editors are open for it and
/// which tab is up. Never a value typed into it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaneView {
    pub binding_editor_open: bool,
    pub binding_has_source: bool,
    pub grid_settings_open: bool,
    pub events_tab: bool,
    pub anim_tab: bool,
}

/// The IDE, as the wizard sees it this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// The open project's key (its manifest path), or `None`.
    pub project: Option<String>,
    /// Something else owns the screen — the Walkthrough, a modal dialog — so the wizard
    /// draws nothing and takes no input (R24).
    pub paused: bool,

    // ── Task 1: a model provider ────────────────────────────────────────────────────────
    /// The Main Pane shows Project Settings.
    pub settings_open: bool,
    /// The Model Providers Manager is open.
    pub models_open: bool,
    /// The provider showing in the manager.
    pub models_provider: Option<String>,
    /// That provider takes an API key.
    pub models_provider_needs_key: bool,
    /// The endpoint field is empty. (An endpoint is not a secret.)
    pub models_endpoint_empty: bool,
    /// The connection test's outcome for the provider showing.
    pub models_test: TestOutcome,
    /// How many times the manager saved, this IDE session.
    pub models_saves: u32,
    /// The provider showing is configured in the saved configuration.
    pub provider_configured: bool,

    // ── Task 2: an agent and its model ─────────────────────────────────────────────────
    /// The Agents Manager is open.
    pub agents_open: bool,
    /// It shows its *Agent × Model* tab.
    pub agents_model_tab: bool,
    /// The provider scope picked on that tab.
    pub agents_scope: Option<String>,
    /// Signature of every agent's (provider, model) pair.
    pub agents_models_sig: u64,

    // ── The Form Designer ──────────────────────────────────────────────────────────────
    /// Every open designer.
    pub designers: Vec<DesignerFacts>,
    /// The subject of the running task, read in its designer; default when there is none.
    pub subject: SubjectFacts,
}

impl Facts {
    /// The designer the developer is working in: the focused one, else the first.
    pub fn active_designer(&self) -> Option<&DesignerFacts> {
        self.designers
            .iter()
            .find(|d| d.focused)
            .or_else(|| self.designers.first())
    }

    /// The designer showing `form`.
    pub fn designer_for(&self, form: &str) -> Option<&DesignerFacts> {
        self.designers.iter().find(|d| d.form == form)
    }

    /// The one control selected in the active designer.
    pub fn selected(&self) -> Option<&Selected> {
        self.active_designer()?.selected.as_ref()
    }
}

/// A stable 64-bit signature of anything hashable, for the `*_sig` fields.
pub fn signature<T: std::hash::Hash + ?Sized>(value: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
