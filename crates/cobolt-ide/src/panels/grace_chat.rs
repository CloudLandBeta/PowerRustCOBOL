// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project-wide Grace chatbot shown in the IDE main pane.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use egui::{CentralPanel, Color32, RichText, ScrollArea};

use crate::grace_host::{workflow_chat_reply, GraceRoutingContext};
use crate::grace_session::GraceSession;
use crate::llm::{ChatTurn, LlmConfig, LlmResponse};

const HISTORY_FILE: &str = "grace-conversation.json";
const GRACE_TITLE: &str = "👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator";

const WELCOME_MESSAGE: &str = r#"Welcome to PowerRustCOBOL Grace Chatbot. If you are not sure what you can ask Grace, type:

What can you do?

Some suggestions for you:

Create Indexed Files for an Accounts Payable System. Prefix the files with aas-
Create CRUD forms for each Indexed File prefixed with aas-. Use Neumorphic Form style. Make it lean and mean.
Add a data bound datagrid to form xxxxx. The data source should be SQLConnection-1.

Advanced usage:

Plan the creation of an ERP called PowerRustERP. Put the plan in the /Knowledge Base/Projects/PowerRustERP/plan folder
Create tasks to implement the plan for the ERP called PowerRustERP (after having created and approved the plan)
Implement tasks for the ERP called PowerRustERP (after having created and approved the tasks)"#;

pub struct GraceChatPanel {
    project_root: Option<PathBuf>,
    prompt: String,
    history: Vec<ChatTurn>,
    session: Option<GraceSession>,
    compact_rx: Option<mpsc::Receiver<LlmResponse>>,
    status: Option<String>,
    error_modal: Option<String>,
    history_font_size: f32,
    rescan_documentation: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GraceChatAction {
    pub close: bool,
    pub rescan_documentation: bool,
}

impl Default for GraceChatPanel {
    fn default() -> Self {
        Self {
            project_root: None,
            prompt: String::new(),
            history: Vec::new(),
            session: None,
            compact_rx: None,
            status: None,
            error_modal: None,
            history_font_size: 14.0,
            rescan_documentation: false,
        }
    }
}

impl GraceChatPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn history_path(root: &Path) -> PathBuf {
        root.join("data").join(HISTORY_FILE)
    }

    fn load_project(&mut self, root: &Path) {
        if self.project_root.as_deref() == Some(root) {
            return;
        }
        self.project_root = Some(root.to_path_buf());
        self.prompt.clear();
        self.session = None;
        self.compact_rx = None;
        self.status = None;
        self.error_modal = None;
        self.rescan_documentation = false;
        self.history = std::fs::read_to_string(Self::history_path(root))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
    }

    fn persist(&self) -> Result<(), String> {
        let root = self
            .project_root
            .as_deref()
            .ok_or_else(|| "No project is open.".to_string())?;
        let path = Self::history_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&self.history).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    fn poll(&mut self, ctx: &egui::Context, verbose: bool) {
        if let Some(session) = self.session.as_mut() {
            if session.poll() || session.is_running() {
                ctx.request_repaint();
            }
        }

        let finished = self
            .session
            .as_ref()
            .and_then(|session| session.finished().cloned());
        if let Some(result) = finished {
            self.session = None;
            // A workflow may have created documentation before either completing
            // or failing a later task. Reconcile the project tree in both cases.
            self.rescan_documentation = true;
            match result {
                Ok((record, _)) => {
                    self.history
                        .push(ChatTurn::assistant(workflow_chat_reply(
                            &record, None, verbose,
                        )));
                    self.status = Some(format!("Workflow {} completed.", record.workflow_id));
                    let _ = self.persist();
                }
                Err(error) => {
                    self.status = None;
                    self.error_modal = Some(error);
                }
            }
        }

        let compacted = self.compact_rx.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(response) => Some(response),
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint();
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => Some(LlmResponse::Err(
                "The history compaction worker stopped unexpectedly.".into(),
            )),
        });
        if let Some(response) = compacted {
            self.compact_rx = None;
            match response {
                LlmResponse::Ok(summary) => {
                    self.history = vec![ChatTurn::assistant(format!(
                        "Conversation summary:\n\n{}",
                        summary.trim()
                    ))];
                    self.status = Some("Conversation compacted.".into());
                    let _ = self.persist();
                }
                LlmResponse::Err(error) => self.error_modal = Some(error),
                LlmResponse::Chunk(_) => {}
            }
        }
    }

    /// Render the project-wide chat. Returns `true` when the developer asks to
    /// close it and return the main pane to its normal editor content.
    pub fn show(
        &mut self,
        panel_ui: &mut egui::Ui,
        root: &Path,
        llm: &LlmConfig,
    ) -> GraceChatAction {
        self.load_project(root);
        let ctx = panel_ui.ctx().clone();
        self.poll(&ctx, llm.verbose_log);

        let busy = self.session.as_ref().is_some_and(GraceSession::is_running)
            || self.compact_rx.is_some();
        let history = self.history.clone();
        let progress = self
            .session
            .as_ref()
            .map(|session| {
                session
                    .log
                    .iter()
                    .rev()
                    .take(8)
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.is_empty());
        let pending_command = self
            .session
            .as_ref()
            .and_then(GraceSession::pending_confirm)
            .map(|request| request.command.clone());

        let mut send = false;
        let mut save = false;
        let mut compact = false;
        let mut clear = false;
        let mut close = false;
        let mut confirm: Option<bool> = None;

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        CentralPanel::default().frame(frame).show(panel_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(GRACE_TITLE);
                if busy {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Coordinating specialists...")
                            .small()
                            .color(crate::theme::active().text_dim),
                    );
                }
            });
            ui.separator();

            let input_height = if pending_command.is_some() {
                230.0
            } else {
                158.0
            };
            let history_height = (ui.available_height() - input_height).max(100.0);
            ScrollArea::vertical()
                .id_salt("project_grace_chat_history")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(history_height)
                .show(ui, |ui| {
                    ui.set_min_height(history_height);
                    if history.is_empty() {
                        crate::panels::editor::chat_bubble_with_font_size(
                            ui,
                            "assistant",
                            WELCOME_MESSAGE,
                            self.history_font_size,
                        );
                    }
                    for (index, turn) in history.iter().enumerate() {
                        crate::panels::editor::chat_bubble_with_response_actions(
                            ui,
                            &turn.role,
                            &turn.content,
                            self.history_font_size,
                            Some(root),
                            egui::Id::new(("project_grace_response", root, index)),
                        );
                    }
                    if let Some(text) = &progress {
                        crate::panels::editor::chat_bubble_with_font_size(
                            ui,
                            "assistant",
                            text,
                            self.history_font_size,
                        );
                    }
                });

            if let Some(command) = &pending_command {
                ui.separator();
                ui.label(RichText::new("Grace requests approval").strong());
                ui.label(RichText::new(command).monospace().small());
                ui.horizontal(|ui| {
                    if ui.button("Approve").clicked() {
                        confirm = Some(true);
                    }
                    if ui.button("Deny").clicked() {
                        confirm = Some(false);
                    }
                });
            }

            ui.separator();
            let mut submit_shortcut = false;
            ui.horizontal(|ui| {
                let prompt_width =
                    super::chat_prompt_width(ui.available_width(), ui.spacing().item_spacing.x);
                let response = ui.add_sized(
                    [prompt_width, 72.0],
                    egui::TextEdit::multiline(&mut self.prompt)
                        .hint_text("How can I help you today?")
                        .desired_rows(3)
                        .interactive(!busy),
                );
                submit_shortcut = response.has_focus()
                    && ui.input(|input| {
                        input.key_pressed(egui::Key::Enter)
                            && (input.modifiers.command || input.modifiers.ctrl)
                    });
                if ui
                    .add_enabled(
                        !busy && !self.prompt.trim().is_empty(),
                        egui::Button::new("Send")
                            .min_size(egui::vec2(super::CHAT_SEND_BUTTON_WIDTH, 36.0)),
                    )
                    .clicked()
                {
                    send = true;
                }
            });
            if submit_shortcut {
                send = true;
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.history.is_empty(), egui::Button::new("Save history"))
                    .clicked()
                {
                    save = true;
                }
                if ui
                    .add_enabled(
                        !busy && !self.history.is_empty(),
                        egui::Button::new("Compact history"),
                    )
                    .clicked()
                {
                    compact = true;
                }
                if ui
                    .add_enabled(
                        !busy && !self.history.is_empty(),
                        egui::Button::new("Clear"),
                    )
                    .clicked()
                {
                    clear = true;
                }
                ui.separator();
                if ui.small_button("−").clicked() {
                    self.history_font_size = (self.history_font_size - 1.0).max(10.0);
                }
                ui.label(RichText::new(format!("{} pt", self.history_font_size as i32)).small());
                if ui.small_button("+").clicked() {
                    self.history_font_size = (self.history_font_size + 1.0).min(28.0);
                }
                if ui.button("Close Grace").clicked() {
                    close = true;
                }
                if let Some(status) = &self.status {
                    ui.label(
                        RichText::new(status)
                            .small()
                            .color(Color32::from_rgb(125, 214, 160)),
                    );
                }
            });
        });

        if let Some(approved) = confirm {
            if let Some(session) = self.session.as_mut() {
                session.respond_confirm(approved);
            }
            ctx.request_repaint();
        }
        if clear {
            self.history.clear();
            self.prompt.clear();
            self.status = None;
            let _ = self.persist();
        }
        if save {
            match self.persist() {
                Ok(()) => self.status = Some("Conversation saved in this project.".into()),
                Err(error) => self.error_modal = Some(error),
            }
        }
        if compact {
            self.compact_rx = Some(crate::llm::spawn_compaction(llm, &self.history));
            self.status = None;
        }
        if send {
            if !llm.agentic_ai_enabled {
                self.error_modal = Some(
                    "Agentic AI is disabled for this project. Enable it in Models Manager before using Grace."
                        .into(),
                );
            } else {
                let request = self.prompt.trim().to_string();
                if !request.is_empty() {
                    let conversation = self
                        .history
                        .iter()
                        .map(|turn| format!("{}: {}", turn.role, turn.content))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    self.history.push(ChatTurn::user(&request));
                    self.prompt.clear();
                    self.status = None;
                    let _ = self.persist();
                    self.session = Some(GraceSession::spawn_with_context(
                        root,
                        llm,
                        &request,
                        GraceRoutingContext::new(
                            "Project workspace",
                            None,
                            format!(
                                "The developer opened the project-wide Grace chatbot. No specialist is preselected; route by capability.\n\nCONVERSATION SO FAR:\n{conversation}"
                            ),
                        ),
                    ));
                    ctx.request_repaint();
                }
            }
        }

        if let Some(message) = self.error_modal.clone() {
            let mut open = true;
            let mut displayed_message = message.clone();
            egui::Window::new("Grace error")
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(800.0, 450.0))
                .open(&mut open)
                .show(&ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut displayed_message)
                                .desired_width(ui.available_width())
                                .interactive(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(message.clone());
                        }
                        if ui.button("OK").clicked() {
                            self.error_modal = None;
                        }
                    });
                });
            if !open {
                self.error_modal = None;
            }
        }
        GraceChatAction {
            close,
            rescan_documentation: std::mem::take(&mut self.rescan_documentation),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_history_is_scoped_to_the_project_root() {
        let base = std::env::temp_dir().join(format!(
            "prc-grace-chat-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let first = base.join("first");
        let second = base.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        let mut panel = GraceChatPanel::new();
        panel.load_project(&first);
        panel.history.push(ChatTurn::user("first project"));
        panel.persist().unwrap();

        panel.load_project(&second);
        assert!(panel.history.is_empty());
        panel.history.push(ChatTurn::user("second project"));
        panel.persist().unwrap();

        panel.load_project(&first);
        assert_eq!(panel.history.len(), 1);
        assert_eq!(panel.history[0].content, "first project");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn welcome_message_contains_the_getting_started_and_advanced_prompts() {
        assert!(WELCOME_MESSAGE.contains("What can you do?"));
        assert!(WELCOME_MESSAGE.contains("Accounts Payable System"));
        assert!(WELCOME_MESSAGE.contains("SQLConnection-1"));
        assert!(WELCOME_MESSAGE.contains("/Knowledge Base/Projects/PowerRustERP/plan"));
        assert!(WELCOME_MESSAGE.contains("Implement tasks for the ERP called PowerRustERP"));
    }

    #[test]
    fn property_pane_uses_the_full_grace_orchestrator_title() {
        assert_eq!(
            GRACE_TITLE,
            "👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator"
        );
    }
}
