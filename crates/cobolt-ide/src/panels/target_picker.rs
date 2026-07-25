// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Grace target-disambiguation modal (spec 034).
//!
//! A centered [`egui::Window`] that lets the developer pick a target on the
//! project tree when a create/edit is ambiguous: a **folder** for a create (with
//! an inline new-folder action), or one of the matching **elements** for an edit.
//! It is rendered by whichever surface owns the paused [`GraceSession`], and its
//! result is fed back through `respond_select`.

use std::path::Path;

use egui::{Align2, Context, RichText, ScrollArea, Vec2};

use crate::i18n::Tr;
use crate::project_fs;
use crate::project_model::{Category, FileKind};
use crate::target_select::{TargetChoice, TargetOp, TargetRequest};

/// Transient UI state for the picker. One per surface that can host it.
#[derive(Default)]
pub struct TargetPicker {
    /// Currently highlighted option (a project-relative folder or element path).
    selected: Option<String>,
    /// Inline new-folder editor (create mode only).
    new_folder_open: bool,
    new_folder_name: String,
}

impl TargetPicker {
    /// Render the modal for `req`. Returns:
    /// * `None` — still open (no decision yet this frame);
    /// * `Some(Some(choice))` — the developer selected a target;
    /// * `Some(None)` — the developer cancelled.
    pub fn show(
        &mut self,
        ctx: &Context,
        req: &TargetRequest,
        root: &Path,
        tr: &Tr,
    ) -> Option<Option<TargetChoice>> {
        let prompt = match req.op {
            TargetOp::Create => format!("{} “{}”", tr.pick_folder_for_creating, req.name),
            TargetOp::Edit => format!("{} “{}”", tr.pick_element_for_editing, req.name),
        };
        let options: Vec<String> = match req.op {
            TargetOp::Create => folder_options(root, req.kind),
            TargetOp::Edit => req.candidates.clone(),
        };
        if self.selected.is_none() {
            self.selected = options.first().cloned();
        }

        let mut outcome: Option<Option<TargetChoice>> = None;
        egui::Window::new(tr.pick_target_title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(prompt);
                ui.add_space(6.0);

                if options.is_empty() {
                    ui.label(RichText::new(tr.pick_no_candidates).italics());
                } else {
                    ScrollArea::vertical()
                        .max_height(280.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for opt in &options {
                                let sel = self.selected.as_deref() == Some(opt.as_str());
                                if ui.selectable_label(sel, opt).clicked() {
                                    self.selected = Some(opt.clone());
                                }
                            }
                        });
                }

                // Inline new-folder (create only) — reuses spec-033 creation (R6).
                if req.op == TargetOp::Create {
                    ui.add_space(4.0);
                    if self.new_folder_open {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_folder_name)
                                    .hint_text(tr.dlg_folder_name_hint)
                                    .desired_width(180.0),
                            );
                            let ok = !self.new_folder_name.trim().is_empty();
                            if ui
                                .add_enabled(ok, egui::Button::new(tr.btn_create))
                                .clicked()
                            {
                                let parent = self
                                    .selected
                                    .clone()
                                    .unwrap_or_else(|| category_root(req.kind));
                                match project_fs::create_folder(
                                    root,
                                    Path::new(&parent),
                                    &self.new_folder_name,
                                ) {
                                    Ok(rel) => {
                                        self.selected = Some(project_fs::rel_string(&rel));
                                    }
                                    Err(_) => {}
                                }
                                self.new_folder_name.clear();
                                self.new_folder_open = false;
                            }
                        });
                    } else if ui.button(format!("📁 {}", tr.tree_new_folder)).clicked() {
                        self.new_folder_open = true;
                    }
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        outcome = Some(None);
                    }
                    let can = self.selected.is_some();
                    if ui.add_enabled(can, egui::Button::new(tr.btn_select)).clicked() {
                        outcome = Some(Some(TargetChoice {
                            rel_path: self.selected.clone().unwrap_or_default(),
                        }));
                    }
                });
            });

        if outcome.is_some() {
            self.reset();
        }
        outcome
    }

    fn reset(&mut self) {
        self.selected = None;
        self.new_folder_open = false;
        self.new_folder_name.clear();
    }
}

fn category_root(kind: FileKind) -> String {
    Category::of_kind(kind).root_subdir().to_string()
}

/// The selectable destination folders for a create: the category root plus every
/// directory under it on disk (so empty and freshly-created folders appear).
fn folder_options(root: &Path, kind: FileKind) -> Vec<String> {
    let sub = category_root(kind);
    let mut dirs = vec![sub.clone()];
    collect_dirs(&root.join(&sub), root, &mut dirs);
    dirs.sort();
    dirs.dedup();
    dirs
}

fn collect_dirs(abs: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(abs) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false);
        if hidden {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
        collect_dirs(&path, root, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_options_lists_root_and_disk_subfolders() {
        let base = std::env::temp_dir().join(format!("prc_picker_{}", std::process::id()));
        let _ = std::fs::create_dir_all(base.join("forms/customers"));
        let _ = std::fs::create_dir_all(base.join("forms/empty"));
        let opts = folder_options(&base, FileKind::Form);
        assert!(opts.contains(&"forms".to_string()));
        assert!(opts.contains(&"forms/customers".to_string()));
        // An empty on-disk folder still appears (R6 inline-created folders show).
        assert!(opts.contains(&"forms/empty".to_string()));
        assert!(opts.iter().all(|o| !Path::new(o).is_absolute()));
        let _ = std::fs::remove_dir_all(&base);
    }
}
