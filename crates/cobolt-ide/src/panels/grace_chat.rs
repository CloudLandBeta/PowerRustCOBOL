// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project-wide Grace chatbot shown in the IDE main pane.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use egui::{CentralPanel, Color32, RichText, ScrollArea};

use crate::grace_host::GraceRoutingContext;
use crate::grace_session::GraceSession;
use crate::i18n::Tr;
use crate::llm::{ChatTurn, LlmConfig, LlmResponse};
use crate::panels::target_picker::TargetPicker;

const HISTORY_FILE: &str = "grace-conversation.json";
const GRACE_TITLE: &str = "👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator";

/// Prompt-box row limits: the box opens at 3 text rows; the bottom-right grip
/// may shrink it to 1 row or grow it to 6, and clamps at both limits.
const PROMPT_DEFAULT_ROWS: f32 = 3.0;
const PROMPT_MIN_ROWS: f32 = 1.0;
const PROMPT_MAX_ROWS: f32 = 6.0;
/// The TextEdit's total vertical inner margin inside the box.
const PROMPT_VERTICAL_MARGIN: f32 = 4.0;
/// Fixed chrome of the input slab besides the prompt box itself: separators,
/// token label and the button rows (the historical 158 − 72 split).
const INPUT_CHROME_HEIGHT: f32 = 86.0;
/// Extra slab height while a pending approval block is shown (230 − 158).
const APPROVAL_BLOCK_HEIGHT: f32 = 72.0;
/// The history keeps at least this height however tall the prompt is dragged.
const MIN_HISTORY_HEIGHT: f32 = 100.0;

/// Height of a prompt box holding `rows` text rows at the style's row height.
fn prompt_height_for(rows: f32, row_height: f32) -> f32 {
    rows * row_height + PROMPT_VERTICAL_MARGIN
}

/// Tallest prompt the panel itself allows: whatever leaves the input chrome,
/// the approval block (when shown) and a minimal history inside the panel. The
/// panel height comes from the window/panel layout — it is externally fixed,
/// never from our own content, so this clamp cannot create a feedback loop.
fn max_prompt_height(panel_height: f32, approval_extra: f32) -> f32 {
    (panel_height - INPUT_CHROME_HEIGHT - approval_extra - MIN_HISTORY_HEIGHT).max(0.0)
}

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

/// The post-workflow star row (agent performance ratings): which agents of
/// the finished workflow can be rated and what the developer selected so far.
/// Clicks are applied to the ratings book immediately; the row is a toggle —
/// re-rating replaces, clicking the same star clears.
struct PendingRating {
    workflow_id: String,
    agents: Vec<String>,
    given: std::collections::HashMap<String, u8>,
}

/// Rating block chrome heights for the input-slab math (fixed, never from
/// content, per this panel's no-self-inflation rule).
const RATING_HEADER_HEIGHT: f32 = 30.0;
const RATING_ROW_HEIGHT: f32 = 26.0;

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
    /// Agent questions not yet shown — surfaced one balloon at a time.
    pending_questions: std::collections::VecDeque<String>,
    /// The question currently awaiting the developer's answer.
    current_question: Option<String>,
    /// Answers collected so far, resubmitted to Grace when the last question
    /// is answered.
    collected_answers: Vec<(String, String)>,
    /// Token totals of the last finished workflow, shown under the input box.
    last_tokens: Option<(u64, u64)>,
    /// Height of the prompt box, user-authoritative: 0 = "never dragged"
    /// (renders at the 3-row default); only the corner-grip drag writes it,
    /// clamped between the 1-row and 6-row limits — never layout measurement.
    prompt_height: f32,
    /// The target-disambiguation modal for this surface (spec 034).
    target_picker: TargetPicker,
    /// The finished workflow awaiting the developer's star row, if any.
    pending_rating: Option<PendingRating>,
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
            pending_questions: std::collections::VecDeque::new(),
            current_question: None,
            collected_answers: Vec::new(),
            last_tokens: None,
            prompt_height: 0.0, // 0 = never dragged → 3-row default
            target_picker: TargetPicker::default(),
            pending_rating: None,
        }
    }
}

/// Assemble the surface context Grace receives from the project-wide chatbot.
///
/// `surface` is the request CONTEXT for whatever is open — the form's control
/// inventory, property keys and project tree, or just the project tree when no
/// form is open. It leads, so the `FORM:` / `CONTROLS:` markers the workflow
/// host slices on (`inject_task_context`) are found in the context itself and
/// cannot be shadowed by something the developer typed. Without it a delegated
/// designer task arrives with no ids, no geometry and no property keys, which is
/// exactly the state this panel's own example prompts ("Add a data bound
/// datagrid to form xxxxx") walk the developer into.
fn project_chat_routing_context(surface: &str, conversation: &str) -> String {
    let preamble =
        "The developer opened the project-wide Grace chatbot. No specialist is preselected; route by capability.";
    let surface = surface.trim();
    if surface.is_empty() {
        return format!("{preamble}\n\nCONVERSATION SO FAR:\n{conversation}");
    }
    format!("{surface}\n\n{preamble}\n\nCONVERSATION SO FAR:\n{conversation}")
}

/// While a question balloon awaits its answer, decide whether the developer's
/// message is that answer or a different task. A task starts with (or pivots
/// via "instead" to) an action verb; anything else — "UUID", "use PIC 9(9)",
/// "COMPANY-MASTER.cidx" — is treated as the answer.
fn looks_like_new_task(message: &str) -> bool {
    let lower = message.trim().to_ascii_lowercase();
    if lower.contains("instead") {
        return true;
    }
    let action_verbs = [
        "create", "add", "remove", "delete", "build", "generate", "design", "implement",
        "modify", "change", "rename", "move", "update", "fix", "make", "write", "refactor",
        "plan", "deploy",
    ];
    lower
        .split_whitespace()
        .take(2)
        .any(|word| action_verbs.contains(&word.trim_matches(|c: char| !c.is_alphanumeric())))
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
        self.pending_questions.clear();
        self.current_question = None;
        self.collected_answers.clear();
        self.last_tokens = None;
        self.pending_rating = None;
        self.history = std::fs::read_to_string(Self::history_path(root))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
    }

    /// Show the next queued agent question as its own red balloon — one
    /// question at a time; the next appears only after this one is answered.
    fn ask_next_question(&mut self) {
        if let Some(question) = self.pending_questions.pop_front() {
            self.history.push(ChatTurn::question(&question));
            self.current_question = Some(question);
        } else {
            self.current_question = None;
        }
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

    fn poll(&mut self, ctx: &egui::Context, verbose: bool, tr: &Tr) {
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
            // Keep the finished run's exact token totals for the footer label.
            if let Some(session) = self.session.as_ref() {
                self.last_tokens = Some(session.token_totals());
            }
            // Spec 036 R3/R11: the run's typed action history persists as its
            // own "actions" turn (rendered as the collapsed history), stored
            // in the language-neutral record form so it re-localizes when the
            // IDE language changes. Supersedes the old "Coordination log"
            // markdown balloon distilled from the string log.
            let action_turn = self
                .session
                .as_ref()
                .filter(|session| !session.actions.is_empty())
                .and_then(|session| {
                    let entries: Vec<_> = session
                        .actions
                        .iter()
                        .map(crate::agent_actions::AgentAction::to_log_entry)
                        .collect();
                    serde_json::to_string(&entries).ok()
                })
                .map(|json| ChatTurn {
                    role: crate::agent_actions::ACTIONS_ROLE.into(),
                    content: json,
                });
            self.session = None;
            // A workflow may have created documentation before either completing
            // or failing a later task. Reconcile the project tree in both cases.
            self.rescan_documentation = true;
            match result {
                Ok((record, _)) => {
                    // Offer the developer's star row for every specialist
                    // that actually RAN in this workflow: blocked agents
                    // never ran, and Grace's own direct answers are not
                    // rateable specialist work.
                    let mut rating_agents: Vec<String> = Vec::new();
                    for task in &record.tasks {
                        let agent = &task.spec.agent;
                        if agent.eq_ignore_ascii_case(crate::agents_db::GRACE)
                            || task.final_state == cobolt_agents::grace::TaskState::Blocked
                            || rating_agents.iter().any(|known| known == agent)
                        {
                            continue;
                        }
                        rating_agents.push(agent.clone());
                    }
                    self.pending_rating = (!rating_agents.is_empty()).then(|| PendingRating {
                        workflow_id: record.workflow_id.clone(),
                        agents: rating_agents,
                        given: std::collections::HashMap::new(),
                    });
                    if let Some(turn) = action_turn {
                        self.history.push(turn);
                    }
                    // Verbose mode yields two balloons: the coordination
                    // transcript, then Grace's OWN final balloon — her summary
                    // no longer drowns at the tail of the transcript balloon.
                    let mut balloons =
                        crate::grace_host::workflow_chat_balloons(&record, verbose);
                    let reply = balloons.pop().unwrap_or_default();
                    for balloon in balloons {
                        self.history.push(ChatTurn::assistant(balloon));
                    }
                    // Agent questions live in their own red balloons, one at a
                    // time: show the surrounding context now, queue the
                    // questions, and surface only the first.
                    let (context, questions) =
                        crate::grace_host::split_developer_questions(&reply);
                    if questions.is_empty() {
                        self.history.push(ChatTurn::assistant(reply));
                    } else {
                        if !context.is_empty() {
                            self.history.push(ChatTurn::assistant(context));
                        }
                        self.pending_questions = questions.into();
                        self.collected_answers.clear();
                        self.ask_next_question();
                    }
                    // Verbose observability: what the retrieval kept OUT of
                    // the context, as its own history line after the reply.
                    if verbose {
                        if let Some(line) = crate::grace_host::rag_savings_line(&record, tr) {
                            self.history.push(ChatTurn::assistant(line));
                        }
                    }
                    self.status = Some(format!("Workflow {} completed.", record.workflow_id));
                    let _ = self.persist();
                }
                Err(error) => {
                    self.status = None;
                    // A developer-initiated stop is an outcome, not an error.
                    if error.contains("stopped by the developer") {
                        // The steps taken before the stop are still part of
                        // the run's reviewable history (spec 036 R3).
                        if let Some(turn) = action_turn {
                            self.history.push(turn);
                        }
                        self.history.push(ChatTurn::assistant(
                            "Stopped at your request. Nothing further was executed.",
                        ));
                        self.status = Some("Workflow stopped.".into());
                        let _ = self.persist();
                    } else {
                        self.error_modal = Some(error);
                    }
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
    /// `surface` is the request CONTEXT for what is currently open (see
    /// [`project_chat_routing_context`]); pass an empty string when there is
    /// nothing to describe.
    pub fn show(
        &mut self,
        panel_ui: &mut egui::Ui,
        root: &Path,
        llm: &LlmConfig,
        tr: &Tr,
        surface: &str,
    ) -> GraceChatAction {
        self.load_project(root);
        let ctx = panel_ui.ctx().clone();
        self.poll(&ctx, llm.verbose_log, tr);

        // A create/edit paused awaiting the developer's target pick (spec 034).
        if let Some(req) = self.session.as_ref().and_then(|s| s.pending_select()).cloned() {
            if let Some(outcome) = self.target_picker.show(&ctx, &req, root, tr) {
                if let Some(session) = self.session.as_mut() {
                    session.respond_select(outcome);
                }
                ctx.request_repaint();
            }
        }

        let busy = self.session.as_ref().is_some_and(GraceSession::is_running)
            || self.compact_rx.is_some();
        let history = self.history.clone();
        // Spec 036 R4: the raw progress log (which may carry payloads under
        // verbose) never reaches this pane — full traces live in the AI/LLM
        // log surfaces and the saved run record. The typed action stream is
        // the only live signal rendered here.
        let live_actions: Vec<crate::agent_actions::AgentAction> = self
            .session
            .as_ref()
            .map(|session| session.actions.clone())
            .unwrap_or_default();
        let current_action = self
            .session
            .as_mut()
            .and_then(|session| session.current_action().cloned());
        let indexing = self
            .session
            .as_ref()
            .and_then(GraceSession::indexing_progress);
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
        let mut stop = false;
        // Star-row interactions, applied after the panel closure releases its
        // borrows: `(agent, selected)` where selected 0 clears the rating.
        let mut rating_click: Option<(String, u8)> = None;
        let mut rating_done = false;
        let stop_requested = self
            .session
            .as_ref()
            .is_some_and(GraceSession::stop_requested);
        // Live token totals: the running session's accumulator the moment each
        // model returns, or the last finished workflow's totals.
        let token_totals = self
            .session
            .as_ref()
            .map(GraceSession::token_totals)
            .or(self.last_tokens);
        // Footer model indicator: the cached configured-Grace fallback, used
        // until the first agent call of the session records a live model.
        let fallback_model = crate::grace_host::grace_model_display_cached(root, llm);

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        CentralPanel::default().frame(frame).show(panel_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(GRACE_TITLE);
                if busy {
                    ui.add(egui::Spinner::new());
                    // Stop sign, visible only while the spinner is: halts
                    // Grace/agents as soon as the in-flight call returns.
                    if stop_requested {
                        ui.label(
                            RichText::new("Stopping…")
                                .small()
                                .color(crate::theme::active().text_dim),
                        );
                    } else if ui
                        .button(RichText::new("🛑").size(16.0))
                        .on_hover_text("Stop Grace and the agents")
                        .clicked()
                    {
                        stop = true;
                    }
                    ui.label(
                        RichText::new("Coordinating specialists...")
                            .small()
                            .color(crate::theme::active().text_dim),
                    );
                }
            });
            ui.separator();

            // The input slab is sized from the USER-chosen prompt height (only
            // the corner-grip drag writes `self.prompt_height`) plus fixed
            // chrome — never from content or remaining space, so the input can
            // neither inflate on its own nor slide under the Output panel.
            let rating_extra = self
                .pending_rating
                .as_ref()
                .map(|pending| {
                    RATING_HEADER_HEIGHT + RATING_ROW_HEIGHT * pending.agents.len() as f32
                })
                .unwrap_or(0.0);
            let approval_extra = if pending_command.is_some() {
                APPROVAL_BLOCK_HEIGHT
            } else {
                0.0
            } + rating_extra;
            // Row-based limits (1..=6 rows, default 3), from the style's row
            // height — never from content. The panel height (fixed by the
            // window layout) additionally caps growth on tiny windows.
            let prompt_row = ui.text_style_height(&egui::TextStyle::Body);
            let prompt_min_height = prompt_height_for(PROMPT_MIN_ROWS, prompt_row);
            let prompt_max_height = prompt_height_for(PROMPT_MAX_ROWS, prompt_row)
                .min(max_prompt_height(ui.available_height(), approval_extra))
                .max(prompt_min_height);
            let prompt_height = if self.prompt_height > 0.0 {
                self.prompt_height
            } else {
                prompt_height_for(PROMPT_DEFAULT_ROWS, prompt_row)
            }
            .clamp(prompt_min_height, prompt_max_height);
            let input_height = prompt_height + INPUT_CHROME_HEIGHT + approval_extra;
            let history_height = (ui.available_height() - input_height).max(MIN_HISTORY_HEIGHT);
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
                        // A persisted action-history turn renders as the
                        // collapsed widget, re-localized from its
                        // language-neutral entries (spec 036 R3/R9/R11).
                        if turn.role == crate::agent_actions::ACTIONS_ROLE {
                            if let Some(actions) =
                                crate::agent_actions::parse_actions_turn(&turn.content)
                            {
                                crate::panels::editor::chat_action_history(
                                    ui,
                                    egui::Id::new(("project_grace_actions", root, index)),
                                    &actions,
                                    tr,
                                    self.history_font_size,
                                );
                                continue;
                            }
                        }
                        crate::panels::editor::chat_bubble_with_response_actions(
                            ui,
                            &turn.role,
                            &turn.content,
                            self.history_font_size,
                            Some(root),
                            egui::Id::new(("project_grace_response", root, index)),
                        );
                    }
                    // The live run closes the transcript: the collapsed
                    // action history so far, then the throttled
                    // current-action line — what the agents are DOING, never
                    // what they produced (spec 036 R1–R4). Falls back to the
                    // generic indicator before the first action lands.
                    if busy {
                        crate::panels::editor::chat_action_history(
                            ui,
                            egui::Id::new(("project_grace_actions_live", root)),
                            &live_actions,
                            tr,
                            self.history_font_size,
                        );
                        match &current_action {
                            Some(action) => crate::panels::editor::chat_current_action(
                                ui,
                                action,
                                tr,
                                self.history_font_size,
                            ),
                            None => crate::panels::editor::chat_thinking_indicator(
                                ui,
                                tr.ai_thinking,
                                self.history_font_size,
                            ),
                        }
                        // Chunk-embedding progress: a live bar while records
                        // are indexed, so a first-time index never looks
                        // stuck behind a spinner.
                        if let Some((done, total, _)) = &indexing {
                            crate::panels::editor::chat_indexing_bar(
                                ui,
                                *done,
                                *total,
                                tr,
                                self.history_font_size,
                            );
                        }
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

            // Post-workflow star row (agent performance ratings): cumulative
            // gold stars per agent, applied on click — 4–5 record praise
            // (+5), 1–2 a rejection (−10), 3 neutral; clicking the selected
            // star again clears. The block stays until dismissed.
            if let Some(pending) = &self.pending_rating {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr.grace_rate_title).strong())
                        .on_hover_text(tr.grace_rate_hint);
                    if ui.small_button(tr.grace_rate_done).clicked() {
                        rating_done = true;
                    }
                });
                for agent in &pending.agents {
                    ui.horizontal(|ui| {
                        let current = pending.given.get(agent).copied().unwrap_or(0);
                        for star in 1..=5u8 {
                            let filled = star <= current;
                            let glyph = RichText::new(if filled { "★" } else { "☆" })
                                .size(16.0)
                                .color(if filled {
                                    Color32::from_rgb(255, 196, 0) // gold
                                } else {
                                    ui.visuals().weak_text_color()
                                });
                            if ui
                                .add(egui::Button::new(glyph).frame(false))
                                .on_hover_text(tr.grace_rate_hint)
                                .clicked()
                            {
                                rating_click = Some((
                                    agent.clone(),
                                    if current == star { 0 } else { star },
                                ));
                            }
                        }
                        ui.label(agent);
                    });
                }
            }

            ui.separator();
            let mut submit_shortcut = false;
            ui.horizontal(|ui| {
                let prompt_width =
                    super::chat_prompt_width(ui.available_width(), ui.spacing().item_spacing.x);
                // Plain Enter submits like the Send button; Shift+Enter inserts
                // a newline. The Enter event must be consumed BEFORE the
                // TextEdit runs, or submitting would also type a newline into
                // the prompt — hence the focus check against last frame's id.
                let prompt_edit_id = egui::Id::new("project_grace_prompt_edit");
                // NOT consume_key: that matches "logically" (extra Shift/Alt
                // ignored) and would swallow Shift+Enter too. Only an Enter
                // with NO modifiers submits; Shift+Enter falls through to the
                // TextEdit and types the newline.
                let enter_submit = !busy
                    && ui.memory(|memory| memory.has_focus(prompt_edit_id))
                    && ui.input_mut(|input| {
                        let mut pressed = false;
                        input.events.retain(|event| {
                            let plain_enter = matches!(
                                event,
                                egui::Event::Key {
                                    key: egui::Key::Enter,
                                    pressed: true,
                                    modifiers,
                                    ..
                                } if modifiers.is_none()
                            );
                            pressed |= plain_enter;
                            !plain_enter
                        });
                        pressed
                    });
                // User-resizable prompt box: a FIXED (prompt_width ×
                // prompt_height) allocation — the height comes only from the
                // 3-row default or the grip drag below, never from content or
                // remaining space, so the box cannot grow by itself.
                let box_size = egui::vec2(prompt_width, prompt_height);
                let inner = ui.allocate_ui(box_size, |ui| {
                    // Fill the box exactly, so the slab math above stays true.
                    ui.set_min_size(box_size);
                    // Longer text scrolls INSIDE the fixed box; the TextEdit
                    // keeps the cursor visible in the scroll.
                    ScrollArea::vertical()
                        .id_salt("project_grace_prompt_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // desired_rows follows the dragged size so the
                            // editor's frame fills the box at every height.
                            let rows = (((prompt_height - PROMPT_VERTICAL_MARGIN)
                                / prompt_row)
                                .round()
                                .max(1.0)) as usize;
                            ui.add(
                                egui::TextEdit::multiline(&mut self.prompt)
                                    .id(prompt_edit_id)
                                    .hint_text("How can I help you today?")
                                    .desired_rows(rows)
                                    .desired_width(f32::INFINITY)
                                    .interactive(!busy),
                            )
                        })
                        .inner
                });
                let box_rect = inner.response.rect;
                let response = inner.inner;
                // Bottom-right resize grip. Registered AFTER the TextEdit so it
                // wins the hit-test over it (egui: later widget is on top) —
                // an egui::Resize corner is registered before its contents and
                // a full-box TextEdit steals its drags as text selection.
                let grip_size = 14.0;
                let grip_rect = egui::Rect::from_min_size(
                    box_rect.max - egui::vec2(grip_size, grip_size),
                    egui::vec2(grip_size, grip_size),
                );
                let grip = ui.interact(
                    grip_rect,
                    egui::Id::new("project_grace_prompt_grip"),
                    egui::Sense::drag(),
                );
                if grip.dragged() {
                    // The ONLY writer of the height besides the 3-row default.
                    // The clamp pins the grip inside the 1-row/6-row limits.
                    self.prompt_height = (prompt_height + grip.drag_delta().y)
                        .clamp(prompt_min_height, prompt_max_height);
                }
                if grip.hovered() || grip.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                // Diagonal grip lines in the corner, like a window's.
                let stroke = if grip.hovered() || grip.dragged() {
                    ui.visuals().widgets.hovered.fg_stroke
                } else {
                    ui.visuals().widgets.inactive.fg_stroke
                };
                let corner = box_rect.max - egui::vec2(3.0, 3.0);
                for step in 1..=3 {
                    let offset = 3.0 * step as f32;
                    ui.painter().line_segment(
                        [
                            egui::pos2(corner.x - offset, corner.y),
                            egui::pos2(corner.x, corner.y - offset),
                        ],
                        stroke,
                    );
                }
                // Cmd/Ctrl+Enter keeps submitting too, for muscle memory.
                submit_shortcut = enter_submit
                    || (response.has_focus()
                        && ui.input(|input| {
                            input.key_pressed(egui::Key::Enter)
                                && (input.modifiers.command || input.modifiers.ctrl)
                        }));
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
            // Model-in-use + context gauge (operator, 2026-07-29), then the
            // up-to-date token usage, refreshed the moment each model returns.
            ui.horizontal(|ui| {
                crate::panels::editor::chat_model_context_indicator(
                    ui,
                    tr,
                    fallback_model.as_deref(),
                );
                if let Some((input_tokens, output_tokens)) = token_totals {
                    ui.label(
                        RichText::new(format!(
                            "Tokens: {input_tokens} in / {output_tokens} out"
                        ))
                        .small()
                        .color(crate::theme::active().text_dim),
                    );
                }
            });
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
        if let Some((agent, selected)) = rating_click {
            if let Some(pending) = self.pending_rating.as_mut() {
                if selected == 0 {
                    pending.given.remove(&agent);
                } else {
                    pending.given.insert(agent.clone(), selected);
                }
                let mut book = crate::agent_ratings::RatingsBook::load(root);
                let delta =
                    book.record_developer_feedback(&pending.workflow_id, &agent, selected);
                match book.save(root) {
                    Ok(()) => {
                        self.status = Some(if selected == 0 {
                            format!("Rating cleared: {agent}.")
                        } else {
                            format!("Rating saved: {agent} {delta:+}.")
                        });
                    }
                    Err(error) => self.error_modal = Some(error),
                }
            }
        }
        if rating_done {
            self.pending_rating = None;
        }
        if clear {
            self.history.clear();
            self.prompt.clear();
            self.status = None;
            let _ = self.persist();
        }
        if stop {
            if let Some(session) = self.session.as_ref() {
                session.stop();
            }
            self.status =
                Some("Stopping — waiting for the current agent call to return.".into());
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
                    // A bare greeting gets a greeting back — no workflow, no
                    // model calls, no tokens. A pending questionnaire (if any)
                    // stays exactly as it is.
                    if let Some(greeting) = crate::grace_host::simple_greeting_reply(&request) {
                        self.history.push(ChatTurn::user(&request));
                        self.history.push(ChatTurn::assistant(greeting));
                        self.prompt.clear();
                        self.status = None;
                        let _ = self.persist();
                        ctx.request_repaint();
                        send = false;
                    }
                }
                if send && !request.is_empty() {
                    // A pending agent question: the message is either its
                    // answer (advance the questionnaire) or a new task (drop
                    // the questionnaire and do what the developer asked).
                    let answered_question = self
                        .current_question
                        .clone()
                        .filter(|_| !looks_like_new_task(&request));
                    if let Some(question) = answered_question {
                        self.history.push(ChatTurn::user(&request));
                        self.prompt.clear();
                        self.status = None;
                        self.collected_answers.push((question, request));
                        if !self.pending_questions.is_empty() {
                            // More questions — next balloon, no workflow yet.
                            self.ask_next_question();
                            let _ = self.persist();
                        } else {
                            self.current_question = None;
                            let answers = self
                                .collected_answers
                                .drain(..)
                                .map(|(question, answer)| {
                                    format!("- {question}\n  Answer: {answer}")
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            let conversation = self
                                .history
                                .iter()
                                .map(|turn| format!("{}: {}", turn.role, turn.content))
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            let _ = self.persist();
                            self.session = Some(GraceSession::spawn_with_context(
                                root,
                                llm,
                                &format!(
                                    "The developer answered the pending clarification questions:\n\n{answers}\n\nContinue the requested work applying these decisions."
                                ),
                                GraceRoutingContext::new(
                                    "Project workspace",
                                    None,
                                    project_chat_routing_context(surface, &conversation),
                                ),
                            ));
                        }
                        ctx.request_repaint();
                    } else {
                        // A fresh request abandons any unanswered questionnaire.
                        self.pending_questions.clear();
                        self.current_question = None;
                        self.collected_answers.clear();
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
                                project_chat_routing_context(surface, &conversation),
                            ),
                        ));
                        ctx.request_repaint();
                    }
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

    #[test]
    fn answers_are_not_mistaken_for_new_tasks() {
        assert!(!looks_like_new_task("UUID"));
        assert!(!looks_like_new_task("Use PIC 9(9) for every ID"));
        assert!(!looks_like_new_task("COMPANY-MASTER.cidx"));
        assert!(!looks_like_new_task("yes, one registered address per company"));
        assert!(looks_like_new_task("create a login form"));
        assert!(looks_like_new_task("Actually, forget that — build the CRUD forms instead"));
        assert!(looks_like_new_task("please add a datagrid to FORM-1"));
    }

    #[test]
    fn prompt_height_opens_at_three_rows_and_clamps_between_one_and_six() {
        // "Never dragged" sentinel: renders at the 3-row default.
        assert_eq!(GraceChatPanel::new().prompt_height, 0.0);

        // Row-based limits at a representative row height.
        let row = 18.0;
        let min = prompt_height_for(PROMPT_MIN_ROWS, row);
        let default = prompt_height_for(PROMPT_DEFAULT_ROWS, row);
        let six = prompt_height_for(PROMPT_MAX_ROWS, row);
        assert!(min < default && default < six);
        assert_eq!(min, row + PROMPT_VERTICAL_MARGIN);
        assert_eq!(six, 6.0 * row + PROMPT_VERTICAL_MARGIN);

        // A roomy panel: the 6-row cap is the binding limit; drags clamp into
        // [1 row, 6 rows] at both ends.
        let max = six.min(max_prompt_height(600.0, 0.0)).max(min);
        assert_eq!(max, six);
        assert_eq!(900.0_f32.clamp(min, max), six);
        assert_eq!(1.0_f32.clamp(min, max), min);

        // The approval block reserves its own slice before the prompt's.
        assert_eq!(
            max_prompt_height(600.0, APPROVAL_BLOCK_HEIGHT),
            600.0 - INPUT_CHROME_HEIGHT - APPROVAL_BLOCK_HEIGHT - MIN_HISTORY_HEIGHT
        );

        // A tiny panel caps growth below 6 rows but never below the 1-row min.
        let tiny_max = six.min(max_prompt_height(120.0, 0.0)).max(min);
        assert_eq!(tiny_max, min);
    }

    #[test]
    fn questions_surface_one_balloon_at_a_time() {
        let mut panel = GraceChatPanel::new();
        panel.pending_questions = vec![
            "Please specify the primary file name.".to_string(),
            "For every ID field: UUID or a COBOL PIC definition?".to_string(),
        ]
        .into();
        panel.ask_next_question();
        // Exactly one red balloon so far, and it holds the FIRST question.
        let questions: Vec<_> = panel
            .history
            .iter()
            .filter(|turn| turn.role == "question")
            .collect();
        assert_eq!(questions.len(), 1);
        assert!(questions[0].content.contains("primary file name"));
        assert_eq!(panel.pending_questions.len(), 1, "second question is held back");
        assert!(panel.current_question.is_some());

        panel.ask_next_question();
        assert_eq!(
            panel
                .history
                .iter()
                .filter(|turn| turn.role == "question")
                .count(),
            2
        );
        assert!(panel.pending_questions.is_empty());
    }
}

#[cfg(test)]
mod surface_context_tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, Form};

    /// The whole point of threading the open form into the project-wide chat is
    /// that the workflow host can slice it: `inject_task_context` keys off the
    /// `CONTROLS:` marker, so a routing context without it silently delegates a
    /// form task with nothing to work from.
    #[test]
    fn the_open_forms_context_survives_into_a_delegated_designer_task() {
        let mut form = Form::new("ACTORS-FORM", "Actors", 640, 480);
        form.controls
            .push(Control::new("Save-Button", ControlType::Button, 24, 400));
        let surface = crate::agent::build_context_with_project(&form, None, None);

        let routing = project_chat_routing_context(&surface, "user: add a slider");

        // The developer's conversation is still carried…
        assert!(routing.contains("CONVERSATION SO FAR:"));
        assert!(routing.contains("add a slider"));
        // …and the markers the host slices on are present and lead, so nothing
        // the developer typed can shadow them.
        assert!(routing.starts_with("CONTEXT"));
        assert!(routing.contains("FORM: ACTORS-FORM"));
        assert!(routing.contains("Save-Button (Button)"));
        assert!(routing.contains("PROPERTY KEYS BY TYPE"));

        // End to end: a designer task delegated with an empty context now gets
        // the ids, the form-level keys and the Button property keys.
        let mut plan = vec![cobolt_agents::grace::TaskSpec {
            id: "T1".into(),
            agent: crate::agents_db::FORM_DESIGNER.into(),
            objective: "add a slider under Save-Button".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec![],
            acceptance: String::new(),
        }];
        crate::grace_host::inject_task_context_for_test(&routing, &mut plan);
        assert!(plan[0].context.contains("Save-Button (Button)"));
        assert!(plan[0].context.contains("FORM PROPERTIES"));
        assert!(
            plan[0].context.contains("Button:"),
            "the designer must receive the property keys of the types in play"
        );
        assert!(
            plan[0].context.contains("Slider:"),
            "including the type the objective asks it to deploy"
        );
    }

    /// With no form open the panel still sends the project tree, so Grace can
    /// name real project resources instead of inventing them — and the absence
    /// of a form is not mistaken for "no context".
    #[test]
    fn without_a_surface_the_preamble_still_stands_alone() {
        let routing = project_chat_routing_context("", "user: hello");
        assert!(routing.starts_with("The developer opened the project-wide Grace chatbot."));
        assert!(routing.contains("CONVERSATION SO FAR:\nuser: hello"));
        assert!(!routing.contains("CONTROLS:"));
    }
}
