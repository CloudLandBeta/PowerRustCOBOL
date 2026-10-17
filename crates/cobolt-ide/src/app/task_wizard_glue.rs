// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 104 — the Task Wizard's ties to `CoboltApp`.
//!
//! The wizard itself (`panels::task_wizard`) never sees the application object, which a test
//! cannot build. This module is the only place the two meet: it reads the IDE into a
//! [`Facts`], hands it to the engine, carries out what the engine asks, and calls the overlay
//! at the right moment of each OS window's pass.
//!
//! The order within a frame:
//! 1. [`CoboltApp::task_wizard_begin_frame`] — before any key reader: open the anchor sink,
//!    filter the IDE window's keys.
//! 2. Each Form Designer window: [`CoboltApp::task_wizard_designer_begin`] before it draws,
//!    [`CoboltApp::task_wizard_designer_end`] after — its overlay, catalogue and offer, from
//!    the marks that window painted.
//! 3. [`CoboltApp::task_wizard_end_frame`] — last: the engine runs on everything the frame
//!    painted, then the IDE window's overlay and catalogue are drawn.

use std::path::Path;

use egui::{Context, Rect, ViewportId};

use super::CoboltApp;
use crate::i18n::Tr;
use crate::panels::task_wizard::{
    self as tw, anchors, catalogue, facts, offers, AnchorId, Effect, FrameInput,
};
use cobolt_forms::ControlType;
use facts::{ControlKind, DesignerFacts, Facts, Selected, SubjectFacts};

/// Which form a subject, a designer or a dismissal is about: the form file's path — two
/// open forms may share a name, never a path.
fn form_key(path: &Path) -> String {
    path.display().to_string()
}

/// A form's name as its designer window calls it.
fn form_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("form")
        .to_owned()
}

fn kind_of(form: &cobolt_forms::Form, c: &cobolt_forms::model::Control) -> ControlKind {
    match c.control_type {
        ControlType::DataGrid => ControlKind::DataGrid,
        ControlType::TreeView => ControlKind::TreeView,
        ControlType::SideMenu => ControlKind::SideMenu,
        ControlType::MenuBar => ControlKind::MenuBar,
        _ => ControlKind::Other {
            bindable: matches!(
                crate::panels::data_binding::visibility_for_control(form, c),
                crate::panels::data_binding::DataBindingVisibility::ApprovedTarget(_)
            ),
        },
    }
}

fn count_items(items: &[cobolt_forms::menu::MenuItem]) -> usize {
    items.iter().map(|i| 1 + count_items(&i.items)).sum()
}

fn any_item(items: &[cobolt_forms::menu::MenuItem], f: &dyn Fn(&cobolt_forms::menu::MenuItem) -> bool) -> bool {
    items.iter().any(|i| f(i) || any_item(&i.items, f))
}

fn has_target(i: &cobolt_forms::menu::MenuItem) -> bool {
    i.action.as_deref().is_some_and(|a| {
        a.starts_with("open-form:")
            || a.starts_with("open-standalone-sync:")
            || a.starts_with("open-standalone-async:")
            || a == "home"
            || a == "close-application"
    })
}

impl CoboltApp {
    /// The open project's key for what the wizard remembers: its manifest's path.
    fn task_wizard_project(&self) -> Option<String> {
        self.cobolt_project.as_ref()?;
        self.project_path.as_ref().map(|p| p.display().to_string())
    }

    /// Either toggle, or the Help item (R1, R3, R4, R6).
    pub(super) fn toggle_task_wizard(&mut self, ctx: &Context) {
        let w = &mut self.task_wizard;
        w.on = !w.on;
        if w.on {
            // R3 — the catalogue opens under the switch that was pressed.
            catalogue::open(ctx);
        } else {
            // R4 — a running task stops, keeping its place.
            for e in w.engine.stop() {
                w.remember(&e);
            }
            w.overlay.clear();
            w.pending = None;
            w.reveal = None;
            egui::Popup::close_id(ctx, catalogue::popup_id());
            let _ = anchors::take();
        }
        w.save();
    }

    /// Whether a task (or its summary) is on screen — the IDE's own shortcuts wait (R23).
    pub(super) fn task_wizard_owns_keys(&self) -> bool {
        self.task_wizard.on && self.task_wizard.engine.is_active()
    }

    /// Start of the IDE window's frame (R23, R35).
    pub(super) fn task_wizard_begin_frame(&mut self, ctx: &Context) {
        let w = &mut self.task_wizard;
        if !w.on {
            return;
        }
        anchors::install(w.reveal.take());
        let vp = ViewportId::ROOT;
        if let Some(nav) = tw::input::filter(
            ctx,
            w.engine.plan(),
            w.overlay.live_rect(vp),
            w.overlay.field_focus(vp),
            false,
        ) {
            w.pending = Some(nav);
        }
    }

    /// Start of a Form Designer window's pass (R15, R22, R23).
    pub(super) fn task_wizard_designer_begin(&mut self, vp: &Context, idx: usize) {
        if !self.task_wizard.on {
            return;
        }
        // A grid in edit mode is a mode `Esc` leaves before it can mean Exit (R22).
        let in_mode = self.designers[idx].1.grid_edit.is_some();
        let w = &mut self.task_wizard;
        let id = vp.viewport_id();
        if vp.input(|i| i.viewport().focused.unwrap_or(false)) {
            w.focused_designer = Some(id);
        }
        if let Some(nav) = tw::input::filter(
            vp,
            w.engine.plan(),
            w.overlay.live_rect(id),
            w.overlay.field_focus(id),
            in_mode,
        ) {
            w.pending = Some(nav);
        }
    }

    /// End of a Form Designer window's pass: the marks only the designer's model knows, the
    /// overlay, the catalogue under this window's switch, and an offer (R10, R15).
    pub(super) fn task_wizard_designer_end(&mut self, vp: &Context, idx: usize, tr: &Tr) {
        if !self.task_wizard.on {
            return;
        }
        let vp_id = vp.viewport_id();
        if let Some(r) = self.designers[idx].1.grid_edit.as_ref().and_then(|g| g.toolbar) {
            anchors::mark_rect(vp_id, AnchorId::GridEditToolbar, r, r);
        }
        let marks = anchors::snapshot();
        let palette = crate::panels::spotlight::balloon_palette(self.current_theme());
        let names = self.task_wizard_window_names(tr);
        let w = &mut self.task_wizard;
        let plan = w.engine.plan().cloned();
        if let Some(nav) =
            w.overlay
                .paint(vp, plan.as_ref(), &marks, tr, &palette, &|v| names(v))
        {
            w.pending = Some(nav);
        }
        self.task_wizard_catalogue(vp, tr);
        self.task_wizard_offer(vp, idx, &marks, tr);
    }

    /// Names of the IDE's windows, for the chip that says where a task continues (R15).
    fn task_wizard_window_names(&self, tr: &Tr) -> impl Fn(ViewportId) -> String {
        let designers: Vec<(ViewportId, String)> = self
            .designers
            .iter()
            .map(|(p, _)| (ViewportId::from_hash_of(p), form_name(p)))
            .collect();
        let ide = tr.task_wizard_ide_window.to_owned();
        move |v| {
            designers
                .iter()
                .find(|(id, _)| *id == v)
                .map(|(_, n)| format!("“{n}”"))
                .unwrap_or_else(|| ide.clone())
        }
    }

    /// End of the frame: the engine on everything painted, its effects, then the IDE
    /// window's overlay and catalogue.
    pub(super) fn task_wizard_end_frame(&mut self, ctx: &Context, tr: &Tr) {
        if !self.task_wizard.on {
            return;
        }
        // The tree keeps its own rects for the Walkthrough; the wizard reads the same ones.
        let tree = self.project.walkthrough_anchors().clone();
        let clip = tree.viewport.unwrap_or(Rect::EVERYTHING);
        if let Some(r) = tree.root {
            anchors::mark_rect(ViewportId::ROOT, AnchorId::ProjectRoot, r, clip);
        }
        if let Some((_, r)) = tree
            .categories
            .iter()
            .find(|(c, _)| *c == crate::project_model::Category::Forms)
        {
            anchors::mark_rect(ViewportId::ROOT, AnchorId::FormsCategory, *r, clip);
        }
        let marks = anchors::take();

        let facts = self.task_wizard_facts(ctx);
        let nav = self.task_wizard.pending.take();
        let out = self.task_wizard.engine.frame(FrameInput {
            facts: &facts,
            marks: &marks,
            nav,
            tr,
        });
        self.task_wizard_apply(ctx, out.effects, tr);

        let palette = crate::panels::spotlight::balloon_palette(self.current_theme());
        let names = self.task_wizard_window_names(tr);
        let w = &mut self.task_wizard;
        if let Some(nav) = w
            .overlay
            .paint(ctx, out.plan.as_ref(), &marks, tr, &palette, &|v| names(v))
        {
            w.pending = Some(nav);
        }
        if w.engine.is_active() || w.pending.is_some() {
            ctx.request_repaint();
        }
        self.task_wizard_catalogue(ctx, tr);
    }

    /// Carry out what the engine asked (R25, R29).
    fn task_wizard_apply(&mut self, ctx: &Context, effects: Vec<Effect>, tr: &Tr) {
        let mut save = false;
        for e in effects {
            save |= self.task_wizard.remember(&e);
            match e {
                Effect::Reveal(a) => {
                    self.task_wizard.reveal = Some(a);
                    use crate::panels::walkthrough::Anchor;
                    match a {
                        AnchorId::ProjectRoot => self.project.reveal_walkthrough(Anchor::ProjectRoot),
                        AnchorId::FormsCategory => self.project.reveal_walkthrough(Anchor::Category(
                            crate::project_model::Category::Forms,
                        )),
                        _ => {}
                    }
                    ctx.request_repaint();
                }
                Effect::Do(action) => self.task_wizard_do(ctx, action, tr),
                Effect::Progress { .. } | Effect::Done(_) => {}
            }
        }
        if save {
            self.task_wizard.save();
        }
    }

    /// *Do this for me* — through the same code path as the developer's own click (R25).
    fn task_wizard_do(&mut self, ctx: &Context, action: tw::Action, tr: &Tr) {
        use tw::Action;
        let subject = self.task_wizard.engine.subject().cloned();
        let designer = subject.as_ref().and_then(|s| {
            self.designers
                .iter()
                .position(|(p, _)| form_key(p) == s.form)
        });
        match action {
            Action::OpenProjectSettings => self.show_project_settings_pane(),
            Action::ResetEndpoint => {
                if let Some(m) = self.models_modal.as_mut() {
                    m.use_default_endpoint();
                }
            }
            Action::AddGridColumn => {
                if let Some(i) = designer {
                    self.designers[i]
                        .1
                        .grid_edit_action(ctx, crate::panels::grid_edit::GridAction::AddColumn);
                }
            }
            Action::StarterSideMenu | Action::StarterMenuBar => {
                if let Some(m) = designer.and_then(|i| self.designers[i].1.menu_modal.as_mut()) {
                    m.add_starter_items(tr);
                }
            }
            Action::AddAnimation => {
                if let (Some(i), Some(s)) = (designer, &subject) {
                    self.designers[i].1.set_property(
                        &s.control,
                        "_AddAnimation",
                        cobolt_forms::PropValue::String(String::new()),
                    );
                }
            }
        }
    }

    /// The IDE as the wizard sees it (R17, R28, R36). Nothing here describes a secret field.
    fn task_wizard_facts(&self, ctx: &Context) -> Facts {
        let mut f = Facts {
            project: self.task_wizard_project(),
            paused: self.walkthrough.is_running()
                || self.build_modal_visible()
                || ctx.memory(|m| m.top_modal_layer()).is_some(),
            settings_open: self.cobolt_project.is_some()
                && !self.show_grace_chat
                && self.show_project_settings
                && self.settings_form.is_some(),
            models_open: self.models_modal.is_some(),
            models_saves: self.task_wizard.models_saves,
            agents_open: self.agents_modal.is_some(),
            ..Default::default()
        };
        if let Some(m) = &self.models_modal {
            let (provider, endpoint_empty, test) = m.task_wizard_view();
            f.models_provider = Some(provider.to_owned());
            f.models_provider_needs_key = crate::llm::provider_requires_key(provider);
            f.models_endpoint_empty = endpoint_empty;
            f.models_test = test;
            f.provider_configured = self.llm.provider_is_configured(provider);
        }
        if let Some(a) = &self.agents_modal {
            let (model_tab, scope, sig) = a.task_wizard_view();
            f.agents_model_tab = model_tab;
            f.agents_scope = Some(scope);
            f.agents_models_sig = sig;
        }
        let focused = self.task_wizard.focused_designer;
        for (path, d) in &self.designers {
            let viewport = ViewportId::from_hash_of(path);
            let form = form_key(path);
            let selected = match d.selected_ids.as_slice() {
                [only] => d.form.find_control(only).map(|c| Selected {
                    form: form.clone(),
                    control: c.id.clone(),
                    kind: kind_of(&d.form, c),
                }),
                _ => None,
            };
            f.designers.push(DesignerFacts {
                viewport,
                form,
                focused: focused == Some(viewport),
                selected,
            });
        }
        if let Some(s) = self.task_wizard.engine.subject() {
            f.subject = self.task_wizard_subject_facts(ctx, s);
        }
        f
    }

    /// What the wizard can see of its subject (R28: no value the developer typed — a
    /// signature where it must notice a change).
    fn task_wizard_subject_facts(&self, ctx: &Context, s: &Selected) -> SubjectFacts {
        let Some((_, d)) = self.designers.iter().find(|(p, _)| form_key(p) == s.form) else {
            return SubjectFacts::default();
        };
        let Some(c) = d.form.find_control(&s.control) else {
            return SubjectFacts::default();
        };
        let pane = d.properties.task_wizard_view(&c.id);
        let binding = d.form.binding_for_control(&c.id);
        let menu = d
            .menu_modal
            .as_ref()
            .filter(|m| m.ctrl_id.eq_ignore_ascii_case(&c.id));
        let is_menu = matches!(c.control_type, ControlType::MenuBar | ControlType::SideMenu);
        SubjectFacts {
            exists: true,
            selected: matches!(d.selected_ids.as_slice(), [only] if *only == c.id),
            bound: binding.is_some(),
            binding_sig: binding.map(|b| facts::signature(&format!("{b:?}"))).unwrap_or(0),
            binding_editor_open: pane.binding_editor_open,
            binding_has_source: pane.binding_has_source,
            grid_settings_open: pane.grid_settings_open,
            grid_columns_sig: facts::signature(&format!(
                "{:?}{:?}",
                c.get_prop("AdvancedGrid"),
                c.get_prop("Columns")
            )),
            grid_column_count: if c.control_type == ControlType::DataGrid {
                cobolt_forms::model::DataGridAdvanced::from_control(c).columns.len()
            } else {
                0
            },
            grid_edit_mode: d.grid_edit.as_ref().is_some_and(|g| g.id == c.id),
            tree_items_sig: facts::signature(&format!("{:?}", c.get_prop("Items"))),
            events_tab: pane.events_tab,
            menu_editor_open: menu.is_some(),
            menu_draft_items: menu.map_or(0, |m| count_items(&m.def.menu)),
            menu_draft_has_target: menu.is_some_and(|m| any_item(&m.def.menu, &has_target)),
            menu_draft_has_shortcut: menu.is_some_and(|m| {
                any_item(&m.def.menu, &|i| i.accelerator.as_deref().is_some_and(|a| !a.trim().is_empty()))
            }),
            menu_sig: if is_menu {
                cobolt_forms::paint::get_menu_cache(ctx, &c.id)
                    .map(|m| facts::signature(&serde_json::to_string(&*m).unwrap_or_default()))
                    .unwrap_or(0)
            } else {
                0
            },
            anim_tab: pane.anim_tab,
            animations: c.animations.len(),
            anim_sig: facts::signature(&format!("{:?}", c.animations)),
            anim_previews: self.task_wizard.anim_previews.get(&c.id).copied().unwrap_or(0),
        }
    }

    /// The catalogue under this window's switch (R3, R7–R9).
    fn task_wizard_catalogue(&mut self, ctx: &Context, tr: &Tr) {
        let Some(anchor) = catalogue::toggle_rect(ctx) else {
            return;
        };
        let project = self.task_wizard_project();
        let w = &self.task_wizard;
        let state = |key: &str| {
            let in_progress = project
                .as_deref()
                .is_some_and(|p| w.progress_of(p, key).is_some());
            catalogue::state_of(in_progress, w.done.contains(key))
        };
        let table = w.engine.table();
        let Some(pick) = catalogue::show(ctx, anchor, table, &state, project.is_some(), tr) else {
            return;
        };
        let (key, resume) = match pick {
            catalogue::Pick::Start(k) | catalogue::Pick::Restart(k) => (k, None),
            catalogue::Pick::Resume(k) => {
                let at = project
                    .as_deref()
                    .and_then(|p| self.task_wizard.progress_of(p, k))
                    .map(str::to_owned);
                (k, at)
            }
        };
        self.task_wizard_start(ctx, key, resume.as_deref(), tr);
    }

    /// Start (or resume) a task (R9). Never while the Walkthrough runs (R24).
    fn task_wizard_start(&mut self, ctx: &Context, key: &'static str, resume: Option<&str>, tr: &Tr) {
        if self.walkthrough.is_running() {
            return;
        }
        let facts = self.task_wizard_facts(ctx);
        let effects = self.task_wizard.engine.start(key, resume, &facts, tr);
        self.task_wizard_apply(ctx, effects, tr);
        ctx.request_repaint();
    }

    /// The offer beside a selected DataGrid, TreeView, SideMenu or MenuBar (R10, R30): only
    /// while no task runs, never for a task already done, never once dismissed for it.
    fn task_wizard_offer(&mut self, vp: &Context, idx: usize, marks: &tw::Marks, tr: &Tr) {
        if self.task_wizard.engine.is_active() {
            return;
        }
        let Some(project) = self.task_wizard_project() else {
            return;
        };
        let (path, d) = &self.designers[idx];
        let [only] = d.selected_ids.as_slice() else {
            return;
        };
        let Some(c) = d.form.find_control(only) else {
            return;
        };
        let selected = Selected {
            form: form_key(path),
            control: c.id.clone(),
            kind: kind_of(&d.form, c),
        };
        let table = self.task_wizard.engine.table();
        let Some(task) = offers::task_for(table, &selected) else {
            return;
        };
        let key = offers::dismissal_key(&project, &selected.form, &selected.control, task.key);
        if self.task_wizard.done.contains(task.key) || self.task_wizard.dismissed.contains(&key) {
            return;
        }
        let Some(at) = marks.find(AnchorId::SubjectOnCanvas, Some(vp.viewport_id())) else {
            return;
        };
        match offers::show(vp, at.rect, task, tr) {
            Some(offers::OfferPick::Start(k)) => self.task_wizard_start(vp, k, None, tr),
            Some(offers::OfferPick::Dismiss) => {
                self.task_wizard.dismissed.insert(key);
                self.task_wizard.save();
            }
            None => {}
        }
    }

    /// R30 — a dismissal is forgotten once its control is gone from an open form.
    pub(super) fn task_wizard_prune_dismissals(&mut self) {
        let Some(project) = self.task_wizard_project() else {
            return;
        };
        let prefix = format!("{project}|");
        let before = self.task_wizard.dismissed.len();
        let designers = &self.designers;
        self.task_wizard.dismissed.retain(|k| {
            let Some(rest) = k.strip_prefix(&prefix) else {
                return true;
            };
            let mut parts = rest.split('|');
            let (Some(form), Some(control)) = (parts.next(), parts.next()) else {
                return true;
            };
            match designers.iter().find(|(p, _)| form_key(p) == form) {
                Some((_, d)) => d.form.find_control(control).is_some(),
                None => true,
            }
        });
        if self.task_wizard.dismissed.len() != before {
            self.task_wizard.save();
        }
    }
}
