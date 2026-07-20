// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE host glue for Grace's workflow runtime (spec 029 Phase B).
//!
//! Maps agent names to project agent-database entries (model, endpoint,
//! per-model key, prompt file), invokes them synchronously through the rig
//! mesh, and persists workflow records under `agentic_ai/Grace/runs/`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cobolt_agents::grace::{
    AgentInvoker, GraceEngine, GraceEvent, TaskRecord, TaskSpec, TaskState, WorkflowRecord,
};
use serde::{Deserialize, Serialize};

use crate::agents_db::{AgentKind, AgentsDb, DATA_INDEXED_FILE_AGENT, DOCUMENTATION_AGENT, GRACE};
use crate::git_exec::GitConfirmRequest;
use crate::llm::{LlmConfig, LlmResponse};
use crate::tool_exec::{IdeToolBackend, ToolEvidence, ToolExecutingInvoker};

/// Bound on tool-execution rounds per task (spec 030) — guards a model that
/// never stops emitting tool calls.
const MAX_TOOL_ROUNDS: usize = 6;

fn is_informational_grace_request(request: &str) -> bool {
    let normalized = request
        .trim()
        .trim_matches(|character: char| character.is_ascii_punctuation())
        .to_ascii_lowercase();
    let passive_openers = [
        "what ",
        "why ",
        "how ",
        "who ",
        "which ",
        "when ",
        "where ",
        "describe ",
        "explain ",
        "summarize ",
        "summarise ",
        "tell me ",
        "suggest ",
        "recommend ",
        "compare ",
        "outline ",
        "provide information ",
        "provide an overview ",
        "give me information ",
        "generate a description ",
        "generate an explanation ",
        "generate a summary ",
    ];
    let follow_on_action = [
        " then create",
        " and create",
        " then modify",
        " and modify",
        " then update",
        " and update",
        " then save",
        " and save",
        " then write",
        " and write",
        " then delete",
        " and delete",
        " then implement",
        " and implement",
        " then fix",
        " and fix",
    ]
    .iter()
    .any(|marker| normalized.contains(marker));
    !follow_on_action
        && (normalized == "help"
            || normalized.contains("what can you do")
            || normalized.contains("what are your capabilities")
            || normalized.contains("how can you help")
            || normalized.contains("who are you")
            || passive_openers
                .iter()
                .any(|opener| normalized.starts_with(opener)))
}

fn direct_grace_record(reply: String) -> WorkflowRecord {
    WorkflowRecord {
        workflow_id: format!("conversation-{}", crate::agents_db::new_uuid()),
        status: "completed".into(),
        tasks: vec![TaskRecord {
            spec: TaskSpec {
                id: "C1".into(),
                agent: GRACE.into(),
                objective: "Answer the developer's read-only request directly".into(),
                context: String::new(),
                reviewer: None,
                depends_on: Vec::new(),
                acceptance: "The answer supplies the requested information as readable Markdown without claiming project changes".into(),
            },
            states: vec![TaskState::Approved],
            submissions: vec![reply],
            reviews: Vec::new(),
            final_state: TaskState::Approved,
            failure_reason: String::new(),
        }],
    }
}

fn has_documentation_intent(request: &str) -> bool {
    let request = request.to_ascii_lowercase();
    request.contains("document")
        || request.contains("/docs/")
        || request.contains("knowledge base")
        || request.contains("plan the creation")
        || request.contains("create tasks")
}

fn task_claims_document_write(task: &TaskSpec) -> bool {
    let text =
        format!("{} {} {}", task.objective, task.context, task.acceptance).to_ascii_lowercase();
    if text.contains("documentation.write") {
        return true;
    }
    if text.contains("indexed_file.write") {
        return false;
    }
    let actions = [
        "write", "save", "create", "format", "publish", "store", "author", "update", "edit",
    ];
    actions.iter().any(|action| {
        text.match_indices(action).any(|(start, _)| {
            let before = &text[..start];
            let previous = before
                .get(before.len().saturating_sub(20)..)
                .unwrap_or(before);
            if previous.contains("do not")
                || previous.contains("must not")
                || previous.contains("without")
            {
                return false;
            }
            let after_action = start + action.len();
            if text
                .as_bytes()
                .get(after_action)
                .is_some_and(u8::is_ascii_alphanumeric)
            {
                return false;
            }
            let suffix = &text[after_action..];
            let window_end = suffix
                .char_indices()
                .nth(64)
                .map(|(index, _)| index)
                .unwrap_or(suffix.len());
            let window = &suffix[..window_end];
            let Some(document_end) = window
                .find("document")
                .map(|index| index + "document".len())
            else {
                return false;
            };
            let assignment = &window[..document_end];
            if assignment.contains("indexed file")
                || assignment.contains("indexed-file")
                || assignment.contains(".cidx")
            {
                return false;
            }
            ![
                "source material",
                "source information",
                "inventory",
                "authoritative facts",
                "schema handoff",
                "documentation agent handoff",
            ]
            .iter()
            .any(|source_phrase| assignment.contains(source_phrase))
        })
    })
}

fn pedantic_relationship_contract(db: &AgentsDb, name: &str) -> String {
    db.by_name(name)
        .map(|agent| match agent.kind {
            AgentKind::Pedantic => db
                .companion_owner(&agent.id)
                .map(|owner| {
                    format!(
                        "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nYou are the sole Pedantic companion for {}. Review only that agent's submitted work and return your independent verdict to Grace; never act as another agent's reviewer.\n",
                        owner.name
                    )
                })
                .unwrap_or_else(|| {
                    "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nYou are not assigned to an orchestrator or specialist. Do not review or approve another agent's work until the project assigns you as its Pedantic companion.\n".to_string()
                }),
            AgentKind::Orchestrator | AgentKind::Specialist => agent
                .companion
                .as_deref()
                .and_then(|id| db.by_id(id))
                .map(|reviewer| {
                    format!(
                        "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nYour sole Pedantic companion is {}. Submit your complete work for that review and do not self-approve or use another agent's reviewer.\n",
                        reviewer.name
                    )
                })
                .unwrap_or_else(|| {
                    "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nNo Pedantic companion is assigned to you. Do not claim that your work received Pedantic approval.\n".to_string()
                }),
        })
        .unwrap_or_default()
}

fn validate_documentation_coordination(request: &str, plan: &[TaskSpec]) -> Result<(), String> {
    for task in plan {
        if task_claims_document_write(task) && !task.agent.eq_ignore_ascii_case(DOCUMENTATION_AGENT)
        {
            return Err(format!(
                "task {} assigns documentation writing to {}; only {} may format and save project documents",
                task.id, task.agent, DOCUMENTATION_AGENT
            ));
        }
    }
    if !has_documentation_intent(request) {
        return Ok(());
    }
    let documentation_tasks = plan
        .iter()
        .filter(|task| task.agent.eq_ignore_ascii_case(DOCUMENTATION_AGENT))
        .collect::<Vec<_>>();
    if documentation_tasks.is_empty() {
        return Err(format!(
            "the request produces project documentation but the plan has no {DOCUMENTATION_AGENT} task"
        ));
    }
    let producer_tasks = plan
        .iter()
        .filter(|task| !task.agent.eq_ignore_ascii_case(DOCUMENTATION_AGENT))
        .collect::<Vec<_>>();
    for documentation in &documentation_tasks {
        let missing_sources = producer_tasks
            .iter()
            .filter(|producer| !documentation.depends_on.contains(&producer.id))
            .map(|producer| producer.id.as_str())
            .collect::<Vec<_>>();
        if !missing_sources.is_empty() {
            return Err(format!(
                "{} task {} must depend on every specialist source task; missing: {}",
                DOCUMENTATION_AGENT,
                documentation.id,
                missing_sources.join(", ")
            ));
        }
    }
    let request = request.to_ascii_lowercase();
    if request.contains("form") || request.contains("interface") {
        let designer_ids = producer_tasks
            .iter()
            .filter(|task| {
                task.agent
                    .eq_ignore_ascii_case(crate::agents_db::FORM_DESIGNER)
            })
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>();
        if designer_ids.is_empty()
            || documentation_tasks.iter().any(|documentation| {
                !designer_ids.iter().any(|id| {
                    documentation
                        .depends_on
                        .iter()
                        .any(|dependency| dependency == *id)
                })
            })
        {
            return Err(format!(
                "interface/form documentation requires {} to prepare the authoritative interface information and every {DOCUMENTATION_AGENT} writing task must depend on that output",
                crate::agents_db::FORM_DESIGNER
            ));
        }
    }
    Ok(())
}

fn has_indexed_file_intent(request: &str) -> bool {
    let request = request.to_ascii_lowercase();
    request.contains("indexed file")
        || request.contains("indexed-file")
        || request.contains(".cidx")
}

fn has_indexed_file_mutation_intent(request: &str) -> bool {
    if !has_indexed_file_intent(request) {
        return false;
    }
    let request = request.to_ascii_lowercase();
    [
        "create",
        "add",
        "new",
        "define",
        "modify",
        "update",
        "change",
        "maintain",
        "normalize",
        "delete",
        "remove",
    ]
    .iter()
    .any(|action| request.contains(action))
}

fn task_text(task: &TaskSpec) -> String {
    format!("{} {} {}", task.objective, task.context, task.acceptance).to_ascii_lowercase()
}

fn task_claims_indexed_mutation(task: &TaskSpec) -> bool {
    let text = task_text(task);
    if text.contains("indexed_file.write") {
        return true;
    }
    let explicitly_non_mutating = [
        "schema handoff",
        "schema proposal",
        "proposed schema",
        "source material",
        "normalization analysis",
        "without writing",
        "without mutation",
        "do not write",
        "do not mutate",
    ]
    .iter()
    .any(|marker| text.contains(marker));
    if explicitly_non_mutating {
        return false;
    }
    let names_resource =
        text.contains(".cidx") || text.contains("indexed file") || text.contains("indexed-file");
    names_resource
        && ["write", "save", "mutate", "delete", "remove"]
            .iter()
            .any(|action| text.contains(action))
}

fn validate_indexed_file_coordination(request: &str, plan: &[TaskSpec]) -> Result<(), String> {
    for task in plan {
        if task_claims_indexed_mutation(task)
            && !task.agent.eq_ignore_ascii_case(DATA_INDEXED_FILE_AGENT)
        {
            return Err(format!(
                "task {} assigns indexed-file mutation to {}; only {} may maintain .cidx resources",
                task.id, task.agent, DATA_INDEXED_FILE_AGENT
            ));
        }
    }
    let needs_schema_handoff =
        has_indexed_file_mutation_intent(request) || plan.iter().any(task_claims_indexed_mutation);
    if !needs_schema_handoff {
        return Ok(());
    }

    let documentation_tasks = plan
        .iter()
        .filter(|task| task.agent.eq_ignore_ascii_case(DOCUMENTATION_AGENT))
        .collect::<Vec<_>>();
    if documentation_tasks.is_empty() {
        return Err(format!(
            "indexed-file work requires {DOCUMENTATION_AGENT} to establish the file name, purpose, project knowledge, normalization, and ID policy"
        ));
    }
    let data_tasks = plan
        .iter()
        .filter(|task| task.agent.eq_ignore_ascii_case(DATA_INDEXED_FILE_AGENT))
        .collect::<Vec<_>>();
    if data_tasks.is_empty() {
        // A Documentation-only workflow is valid when it is explicitly asking
        // the developer for information required before mutation can begin.
        if documentation_tasks.iter().all(|task| {
            let text = task_text(task);
            text.contains("ask") || text.contains("clarif") || text.contains("obtain")
        }) {
            return Ok(());
        }
        return Err(format!(
            "indexed-file work has no {DATA_INDEXED_FILE_AGENT} task and is not an explicit clarification workflow"
        ));
    }
    for data_task in data_tasks {
        let missing = documentation_tasks
            .iter()
            .filter(|documentation| !data_task.depends_on.contains(&documentation.id))
            .map(|documentation| documentation.id.as_str())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "{} task {} must depend on every approved {} handoff; missing: {}",
                DATA_INDEXED_FILE_AGENT,
                data_task.id,
                DOCUMENTATION_AGENT,
                missing.join(", ")
            ));
        }
    }
    Ok(())
}

/// A Pedantic agent is a companion reviewer, never a task agent. Grace
/// sometimes plans a redundant "review the completed work" task assigning the
/// reviewer as the responsible agent; every task already carries its reviewer,
/// so that task is duplicate work. It also fails: reviewers are provisioned
/// with no model of their own (they inherit the reviewer config), so running
/// one through the specialist path resolves an empty provider/model/key and the
/// request is rejected by the provider.
fn validate_no_pedantic_task_agent(db: &AgentsDb, plan: &[TaskSpec]) -> Result<(), String> {
    for task in plan {
        let is_pedantic = db
            .by_name(&task.agent)
            .map(|agent| agent.kind == AgentKind::Pedantic)
            .unwrap_or(false);
        if is_pedantic {
            return Err(format!(
                "task {} assigns Pedantic reviewer \"{}\" as its responsible agent; a Pedantic agent \
                 only reviews as a companion. Assign the task to the specialist that owns the work \
                 and name the reviewer in that task's `reviewer` field instead.",
                task.id, task.agent
            ));
        }
    }
    Ok(())
}

fn validate_workflow_coordination(
    db: &AgentsDb,
    request: &str,
    plan: &[TaskSpec],
) -> Result<(), String> {
    validate_no_pedantic_task_agent(db, plan)?;
    validate_documentation_coordination(request, plan)?;
    validate_indexed_file_coordination(request, plan)
}

/// Advisory routing information supplied by the chatbot surface that opened
/// Grace. The preferred specialist is a starting point, never a restriction:
/// Grace remains responsible for decomposing cross-domain work and delegating
/// every part to the appropriate project specialist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraceRoutingContext {
    pub surface: String,
    pub preferred_specialist: Option<String>,
    pub context: String,
}

impl GraceRoutingContext {
    pub fn new(
        surface: impl Into<String>,
        preferred_specialist: Option<&str>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            surface: surface.into(),
            preferred_specialist: preferred_specialist.map(str::to_owned),
            context: context.into(),
        }
    }
}

/// The on-disk shape of a saved run (spec 030 R11): the workflow record plus the
/// tool-execution evidence. `tool_calls` defaults to empty so a legacy file that
/// held a bare [`WorkflowRecord`] still parses via [`load_run_file`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFile {
    pub record: WorkflowRecord,
    #[serde(default)]
    pub tool_calls: Vec<ToolEvidence>,
}

/// Read a saved run, accepting both the current `{record, tool_calls}` shape and
/// a legacy bare-`WorkflowRecord` file.
pub fn load_run_file(path: &Path) -> Result<RunFile, String> {
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if let Ok(rf) = serde_json::from_str::<RunFile>(&data) {
        return Ok(rf);
    }
    let record: WorkflowRecord =
        serde_json::from_str(&data).map_err(|e| format!("unreadable run file: {e}"))?;
    Ok(RunFile {
        record,
        tool_calls: Vec::new(),
    })
}

/// Synchronous invoker over the project agent database. Each call resolves
/// the agent fresh (config can change between tasks) and blocks until the
/// model's full reply arrived.
pub struct DbAgentInvoker {
    pub project_dir: PathBuf,
    pub llm: LlmConfig,
}

impl DbAgentInvoker {
    fn config_for(&self, agent: &str) -> Result<(LlmConfig, String, String), String> {
        let db = AgentsDb::load(&self.project_dir);
        let Some(a) = db.by_name(agent) else {
            return Err(format!(
                "Agent \u{201c}{agent}\u{201d} is not in the project database — Grace must report the missing capability, not fabricate it."
            ));
        };
        if !a.enabled {
            return Err(format!("Agent \u{201c}{agent}\u{201d} is disabled."));
        }
        // spec 031: resolve the connection from the referenced model profile
        // (with an embedded-config fallback for un-migrated agents).
        let cfg = crate::agents_db::resolve_agent_connection(a, &self.llm)
            .ok_or_else(|| format!("Agent \u{201c}{agent}\u{201d} has no model configured."))?;
        let core_instructions = db.load_agent_core_instructions(&a.name);
        let skills = db.load_agent_capabilities(&a.name);
        let knowledge = db.load_agent_knowledge(&a.name);
        let mut combined_skills = skills;
        if !knowledge.is_empty() {
            if !combined_skills.is_empty() {
                combined_skills.push_str("\n\n");
            }
            combined_skills.push_str(&knowledge);
        }
        Ok((cfg, core_instructions, combined_skills))
    }
}

impl AgentInvoker for DbAgentInvoker {
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String> {
        let (cfg, core_instructions, skills) = self.config_for(agent)?;
        // Report a blank credential as itself rather than letting the provider
        // answer 401, which reads like an account problem.
        if let Some(gap) = crate::llm::credential_gap(&cfg) {
            return Err(format!("{agent}: {gap}"));
        }
        let final_system = if system.trim().is_empty() {
            &core_instructions
        } else {
            system
        };
        let rx = crate::llm::spawn_named_agent_request(
            &cfg,
            final_system,
            &skills,
            user,
            agent,
        );
        // Drain the stream to the final result (chunks are progress only).
        loop {
            match rx.recv() {
                Ok(LlmResponse::Chunk(_)) => {}
                Ok(LlmResponse::Ok(full)) => return Ok(full),
                Ok(LlmResponse::Err(e)) => return Err(e),
                Err(_) => return Err("agent worker stopped unexpectedly".into()),
            }
        }
    }
}

/// Extract the change-sets an *approved* Form-Designer task produced, so the
/// host can apply them through the reviewable preview/apply path (spec 030
/// R6/R7). Only tasks that reached [`TaskState::Approved`] and were handled by
/// `designer_agent` are considered; each yields the parse result of its final
/// (approved) submission. Parsing/validation and application stay with the
/// existing change-set path — this only *selects* what to apply.
pub fn approved_form_change_sets(
    record: &WorkflowRecord,
    designer_agent: &str,
) -> Vec<Result<crate::agent::AgentChangeSet, String>> {
    use cobolt_agents::grace::TaskState;
    record
        .tasks
        .iter()
        .filter(|t| t.final_state == TaskState::Approved && t.spec.agent == designer_agent)
        .filter_map(|t| t.submissions.last())
        .map(|s| crate::agent::parse_change_set(s))
        .collect()
}

/// Persist a workflow record under `agentic_ai/Grace/runs/<workflow-id>.json`
/// (spec 029 observability). Returns the file path.
pub fn save_workflow_record(
    project_dir: &Path,
    record: &WorkflowRecord,
    tool_calls: &[ToolEvidence],
) -> Result<PathBuf, String> {
    let dir = crate::agent::project_agentic_root(project_dir)
        .join(GRACE)
        .join("runs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.workflow_id));
    let run = RunFile {
        record: record.clone(),
        tool_calls: tool_calls.to_vec(),
    };
    let json = serde_json::to_string_pretty(&run).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// A one-line, human-readable rendering of a workflow transition, for the
/// activity log / progress pane.
pub fn describe_event(e: &GraceEvent) -> String {
    match e {
        GraceEvent::TaskStarted {
            id,
            agent,
            objective,
        } => {
            format!("▸ {id}: delegating to {agent} — {objective}")
        }
        GraceEvent::Submitted { id, agent } => format!("  {id}: {agent} submitted a result"),
        GraceEvent::ReviewStarted {
            id,
            reviewer,
            round,
        } => {
            format!("  {id}: {reviewer} reviewing (round {})", round + 1)
        }
        GraceEvent::Verdict {
            id,
            reviewer,
            approved,
        } => format!(
            "  {id}: {reviewer} verdict — {}",
            if *approved {
                "approved"
            } else {
                "defects found"
            }
        ),
        GraceEvent::CorrectionRequested { id, round } => {
            format!("  {id}: correction requested (revision {round})")
        }
        GraceEvent::Approved { id } => format!("✓ {id}: approved"),
        GraceEvent::Failed { id, reason } => format!("✗ {id}: failed — {reason}"),
        GraceEvent::Blocked { id } => format!("⊘ {id}: blocked (dependency not approved)"),
    }
}

/// Run one complete Grace workflow for `request`: Grace plans (structured
/// output), the engine executes with review gates + bounded corrections, and
/// specialists **execute their declared tools** (spec 030) through the
/// tool-executing invoker. `on_progress` receives one line per transition (and
/// per tool call); `confirm` gates network/history-rewriting git ops (R12).
/// The record + tool evidence are persisted. Returns (record, record_path).
/// Blocking — call from a worker thread.
pub fn run_grace_workflow(
    project_dir: &Path,
    llm: &LlmConfig,
    request: &str,
    on_progress: &mut dyn FnMut(String),
    confirm: &mut dyn FnMut(GitConfirmRequest) -> bool,
) -> Result<(WorkflowRecord, PathBuf), String> {
    run_grace_workflow_with_context(
        project_dir,
        llm,
        request,
        &GraceRoutingContext::default(),
        on_progress,
        confirm,
    )
}

/// Context-aware variant used by IDE chatbot surfaces. Surface preference is
/// deliberately advisory so Grace can combine form, event, code, data, and
/// other specialists whenever the requested outcome crosses responsibilities.
pub fn run_grace_workflow_with_context(
    project_dir: &Path,
    llm: &LlmConfig,
    request: &str,
    routing: &GraceRoutingContext,
    on_progress: &mut dyn FnMut(String),
    confirm: &mut dyn FnMut(GitConfirmRequest) -> bool,
) -> Result<(WorkflowRecord, PathBuf), String> {
    let mut db = AgentsDb::load(project_dir);
    let repaired = db.ensure_fixed_agents(llm);
    if repaired > 0 {
        on_progress(format!(
            "Prepared {repaired} fixed-agent or project-knowledge capability update(s)."
        ));
    }
    let indexed = cobolt_agents::project_knowledge::sync_documentation(project_dir)
        .map_err(|error| format!("Project Knowledge Base could not be indexed: {error}"))?;
    on_progress(format!(
        "Project Knowledge Base: {indexed} textual file(s) indexed."
    ));
    let knowledge = cobolt_agents::project_knowledge::search(project_dir, request, 5)
        .map_err(|error| format!("Project knowledge could not be searched: {error}"))?;

    let mut essential_knowledge = String::new();
    let essential_paths = [
        "Knowledge Base/rustcobol_extensions.md",
        "Knowledge Base/ide_functionalities.md",
        "Knowledge Base/form_designer_controls.md",
        "Knowledge Base/agents_registry.md",
    ];
    for path_str in &essential_paths {
        let p = project_dir.join(path_str);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if !essential_knowledge.is_empty() {
                    essential_knowledge.push_str("\n\n---\n\n");
                }
                essential_knowledge.push_str(&format!(
                    "PATH: /{path_str}\nEXCERPT:\n{}",
                    content.trim()
                ));
            }
        }
    }

    let search_results = if knowledge.is_empty() {
        "(no relevant project Knowledge Base evidence found)".to_string()
    } else {
        knowledge
            .iter()
            .map(|hit| {
                format!(
                    "PATH: {}\nRELEVANCE: {:.4}\nEXCERPT:\n{}",
                    hit.path, hit.score, hit.excerpt
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };

    let knowledge_context = if essential_knowledge.is_empty() {
        search_results
    } else {
        format!(
            "--- ESSENTIAL SYSTEM DOCUMENTATION ---\n\n{essential_knowledge}\n\n--- SEARCH RESULTS FOR DEVELOPER REQUEST ---\n\n{search_results}"
        )
    };

    let knowledge_context = format!(
        "{knowledge_context}\n\nPROJECT KNOWLEDGE PRECEDENCE CONTRACT:\n- Treat relevant Knowledge Base excerpts as authoritative project evidence and prefer them over general model training.\n- Cite the project-relative PATH for project-specific claims.\n- Never replace, contradict, or embellish project evidence with generic assumptions.\n- If there is no relevant evidence, say so clearly. Use general knowledge only when appropriate and label it as general guidance; ask the developer for missing project facts instead of inventing them."
    );
    // Base transport (resolves model/key/prompt per agent), decorated with the
    // tool-execution layer: declared-tools governance + git/egui backends.
    let mut inner = DbAgentInvoker {
        project_dir: project_dir.to_path_buf(),
        llm: llm.clone(),
    };
    let mut backend = IdeToolBackend::new(project_dir.to_path_buf(), confirm);
    let evidence: Arc<Mutex<Vec<ToolEvidence>>> = Arc::new(Mutex::new(Vec::new()));
    let dir_for_decl = project_dir.to_path_buf();
    let declared = move |agent: &str| -> std::collections::HashSet<String> {
        AgentsDb::load(&dir_for_decl)
            .by_name(agent)
            .map(|a| a.tools.iter().cloned().collect())
            .unwrap_or_default()
    };
    let mut invoker = ToolExecutingInvoker::new(
        &mut inner,
        &mut backend,
        declared,
        evidence.clone(),
        MAX_TOOL_ROUNDS,
    );
    // Registry census for the planning prompt: names, kinds, specializations,
    // companions — Grace selects by capability, never by name similarity.
    let registry: String = db
        .agents
        .iter()
        .map(|a| {
            let comp = a
                .companion
                .as_ref()
                .and_then(|cid| db.by_id(cid))
                .map(|c| format!(" · companion: {}", c.name))
                .unwrap_or_default();
            format!(
                "- {} [{:?}] specialization: {}{}{}",
                a.name,
                a.kind,
                if a.specialization.is_empty() {
                    "—"
                } else {
                    &a.specialization
                },
                if a.enabled { "" } else { " (DISABLED)" },
                comp,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let surface = if routing.surface.trim().is_empty() {
        "Project workspace"
    } else {
        routing.surface.trim()
    };
    let preference = routing
        .preferred_specialist
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("none; select by capability");
    let context = if routing.context.trim().is_empty() {
        "(no additional surface context)"
    } else {
        routing.context.trim()
    };
    let plan_user = format!(
        "USER REQUEST:\n{request}\n\nCHAT SURFACE:\n{surface}\n\nPREFERRED SPECIALIST:\n{preference}\n\nSURFACE CONTEXT:\n{context}\n\nRELEVANT INDEXED PROJECT KNOWLEDGE:\n{knowledge_context}\n\nAVAILABLE AGENT REGISTRY:\n{registry}\n\nThe preferred specialist is an initial routing preference only, never an exclusive assignment. Decompose mixed requests and delegate every part to whichever available specialist owns that responsibility. For example, form creation plus onClick behavior normally requires both form-design and event-handler tasks. Grace may call any enabled specialist needed anywhere in the project.\n\nPEDANTIC COMPANION CONTRACT:\n- Companion relationships are one-to-one: one orchestrator or specialist has at most one Pedantic reviewer, and one Pedantic reviewer belongs to at most one reviewed agent.\n- For every task, use exactly the Pedantic companion shown for its responsible agent in the registry. Never substitute or reuse another agent's reviewer.\n- Leave reviewer null only when the responsible agent has no companion.\n\nDOCUMENTATION COORDINATION CONTRACT:\n- Only {DOCUMENTATION_AGENT} may format and write project documentation files.\n- When documentation concerns another domain, first assign one or more source-material tasks to the responsible domain specialists. Those specialists prepare authoritative information and MUST NOT write documentation files.\n- Then assign a {DOCUMENTATION_AGENT} task whose depends_on contains every source-material task. The workflow engine passes their approved outputs into the Documentation Agent task as its authoritative handoff.\n- Example: to document a form interface, Form Designer Agent first inventories the controls, layout, bindings, and events; after approval, {DOCUMENTATION_AGENT} formats that output and saves the document.\n- Never ask {DOCUMENTATION_AGENT} to invent technical facts owned by another specialist, and never ask another specialist to save a documentation file.\n\nINDEXED FILE COORDINATION CONTRACT:\n- {DATA_INDEXED_FILE_AGENT} is the sole specialist allowed to create or modify PowerRustCOBOL indexed-file definitions through the Indexed File UI model.\n- Start with a {DOCUMENTATION_AGENT} task that explicitly obtains the file name when absent, establishes the purpose from the developer request, searches project knowledge, analyzes 1NF, 2NF, and 3NF, and identifies any helper indexed files required by normalization.\n- For every ID field, {DOCUMENTATION_AGENT} must obtain the developer's explicit choice between UUID and a specific COBOL PIC definition. Never infer this choice.\n- Each {DATA_INDEXED_FILE_AGENT} mutation task must depend on the approved {DOCUMENTATION_AGENT} handoff. Helper relations are separate dependent Data-agent tasks.\n- If the file name, purpose, normalization decisions, or ID choice is missing, plan a Documentation-only clarification task and do not plan mutation yet. Grace relays the resulting question to the developer.\n- Neither Grace nor {DOCUMENTATION_AGENT} may mutate `.cidx` resources; {DOCUMENTATION_AGENT} prepares the approved schema handoff and Grace coordinates it.\n\nSpecialists should use knowledge.search when prior plans, requirements, task lists, or project decisions may matter. Plan the workflow per your tooling contract (END with the plan JSON). Assign each task's reviewer from the responsible agent's pedantic companion; leave reviewer null only where no companion exists."
    );
    let direct_response = is_informational_grace_request(request);
    let plan_user = if direct_response {
        format!(
            "{plan_user}\n\nDIRECT INFORMATION RESPONSE CONTRACT:\nThis is a read-only request for an explanation, description, summary, recommendation, comparison, or answer. Answer first from relevant project Knowledge Base evidence and cite its PATH entries. If no relevant evidence exists, state that limitation before offering clearly labeled general guidance. Respond directly as readable Markdown for the chatbot. Do not create workflow tasks, do not emit workflow JSON, do not claim that project resources were changed, and do not reject Markdown merely because agent workflows use JSON."
        )
    } else {
        format!(
            "{plan_user}\n\nACTION RESPONSE CONTRACT:\nThis request may create, inspect, plan, or modify project work. Return an executable workflow and end with the required fenced workflow JSON."
        )
    };
    on_progress(if direct_response {
        "Grace is preparing a direct response…".into()
    } else {
        "Grace is planning the workflow…".into()
    });
    let plan_reply = invoker.invoke(GRACE, "", &plan_user)?;
    if direct_response {
        on_progress("Grace answered the read-only request directly.".into());
        let record = direct_grace_record(plan_reply);
        let path = save_workflow_record(project_dir, &record, &[])?;
        return Ok((record, path));
    }
    let (mut workflow_id, mut plan) = match cobolt_agents::grace::parse_plan(&plan_reply) {
        Ok(parsed) => parsed,
        Err(first_error) => {
            on_progress(format!(
                "Grace returned a malformed workflow plan ({first_error}). Requesting one corrected plan."
            ));
            let correction = format!(
                "Your previous response could not be executed because: {first_error}\n\nReturn a COMPLETE corrected workflow plan for the original request. END with exactly one fenced JSON block containing workflow_id and a non-empty tasks array. Use only agent and reviewer names from the supplied registry. Do not add any text after the JSON block.\n\nORIGINAL REQUEST:\n{request}\n\nMALFORMED RESPONSE:\n{plan_reply}"
            );
            let corrected_reply = invoker.invoke(GRACE, "", &correction)?;
            cobolt_agents::grace::parse_plan(&corrected_reply).map_err(|second_error| {
                let error = format!(
                    "Grace could not produce an executable workflow plan after one correction.\n\nFirst parser error: {first_error}\nCorrected parser error: {second_error}\n\nCorrected response payload:\n{corrected_reply}"
                );
                crate::llm::push_ai_log(crate::llm::AiLogKind::Error, error.clone());
                crate::llm::push_connection_log(&format!(
                    "=== GRACE WORKFLOW ERROR ===\n{error}\n"
                ));
                error
            })?
        }
    };
    let plan_db = AgentsDb::load(project_dir);
    if let Err(defect) = validate_workflow_coordination(&plan_db, request, &plan) {
        on_progress(format!(
            "Grace's plan violated a coordination contract: {defect}. Requesting a corrected plan."
        ));
        let correction = format!(
            "Your previous workflow plan was rejected because: {defect}\n\nReturn a COMPLETE corrected workflow plan. Preserve the Documentation coordination contract. For indexed-file work, use {DOCUMENTATION_AGENT} first for file name, purpose, project knowledge, 1NF/2NF/3NF, helper-file analysis, and the developer's UUID-or-PIC decision; only then assign dependent mutation tasks to {DATA_INDEXED_FILE_AGENT}. If required information is absent, return only a Documentation clarification task and do not mutate. END with the corrected plan JSON and nothing after it.\n\nORIGINAL REQUEST:\n{request}\n\nREJECTED PLAN:\n{plan_reply}"
        );
        let corrected_reply = invoker.invoke(GRACE, "", &correction)?;
        (workflow_id, plan) = cobolt_agents::grace::parse_plan(&corrected_reply)?;
        validate_workflow_coordination(&plan_db, request, &plan).map_err(|error| {
            format!("Grace's corrected plan still violates a coordination contract: {error}")
        })?;
    }
    on_progress(format!(
        "Grace planned {} task(s) [{}].",
        plan.len(),
        workflow_id
    ));

    let db2 = AgentsDb::load(project_dir);
    let system_for = move |name: &str| {
        let base = db2.load_agent_core_instructions(name);
        let relationship = pedantic_relationship_contract(&db2, name);
        let base = format!("{base}{relationship}");
        // Append the tool-calling contract for whatever tools this agent
        // declares (spec 030 R2) — always consistent with its actual grant.
        let declared: std::collections::HashSet<String> = db2
            .by_name(name)
            .map(|a| a.tools.iter().cloned().collect())
            .unwrap_or_default();
        let appendix = crate::tool_exec::tool_contract_appendix(&declared);
        // Only the Form Designer's submission is parsed as a change-set
        // (`approved_form_change_sets`), so only it is told that schema.
        let change_set = if name == crate::agents_db::FORM_DESIGNER {
            crate::tool_exec::CHANGE_SET_CONTRACT
        } else {
            ""
        };
        // The scope boundary is unconditional: it constrains what an agent may
        // change, so it must hold even for an agent that declares no tools.
        format!(
            "{base}{appendix}{change_set}{}",
            crate::tool_exec::PROJECT_SCOPE_BOUNDARY
        )
    };
    let ev_for_progress = evidence.clone();
    let mut emitted = 0usize;
    let mut flush_tools = move |on_progress: &mut dyn FnMut(String)| {
        let ev = ev_for_progress.lock().unwrap();
        while emitted < ev.len() {
            let e = &ev[emitted];
            on_progress(format!("  \u{1f527} {} — {}", e.tool, e.summary));
            emitted += 1;
        }
    };
    let record = GraceEngine::default().run_with_progress(
        &workflow_id,
        &plan,
        &mut invoker,
        &system_for,
        &mut |e| {
            flush_tools(on_progress); // stream tool calls as they land (R11)
            on_progress(describe_event(&e));
        },
    );
    flush_tools(on_progress); // any evidence recorded after the last transition
    drop(invoker); // release the borrows on evidence before draining it

    let tool_calls = evidence.lock().unwrap().clone();
    let path = save_workflow_record(project_dir, &record, &tool_calls)?;
    on_progress(format!(
        "Workflow {}: {}.",
        record.workflow_id, record.status
    ));
    Ok((record, path))
}

/// Convert a completed workflow into the reply shown by a chatbot. Prefer the
/// surface's specialist output when it exists (important for structured form
/// change-sets); otherwise include every approved specialist submission.
pub fn workflow_chat_reply(record: &WorkflowRecord, preferred_specialist: Option<&str>) -> String {
    use cobolt_agents::grace::TaskState;

    let approved: Vec<_> = record
        .tasks
        .iter()
        .filter(|task| task.final_state == TaskState::Approved)
        .filter_map(|task| task.submissions.last().map(|text| (&task.spec.agent, text)))
        .collect();
    let preferred: Vec<_> = preferred_specialist
        .map(|name| {
            approved
                .iter()
                .filter(|(agent, _)| agent.eq_ignore_ascii_case(name))
                .map(|(_, text)| (*text).clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !preferred.is_empty() {
        return preferred.join("\n\n");
    }
    if approved.len() == 1 && approved[0].0.eq_ignore_ascii_case(GRACE) {
        return approved[0].1.clone();
    }
    if !approved.is_empty() {
        return approved
            .into_iter()
            .map(|(agent, text)| format!("{agent}:\n{text}"))
            .collect::<Vec<_>>()
            .join("\n\n");
    }

    let failures: Vec<_> = record
        .tasks
        .iter()
        .filter(|task| !task.failure_reason.trim().is_empty())
        .map(|task| format!("{}: {}", task.spec.agent, task.failure_reason))
        .collect();
    if failures.is_empty() {
        format!("Grace finished the workflow with status {}.", record.status)
    } else {
        format!(
            "Grace finished the workflow with status {}.\n\n{}",
            record.status,
            failures.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_agents::grace::{TaskRecord, TaskSpec, TaskState};

    fn sample_record() -> WorkflowRecord {
        WorkflowRecord {
            workflow_id: "wf-1".into(),
            status: "completed".into(),
            tasks: vec![TaskRecord {
                spec: TaskSpec {
                    id: "T1".into(),
                    agent: "Version Control Agent".into(),
                    objective: "commit".into(),
                    context: String::new(),
                    reviewer: None,
                    depends_on: vec![],
                    acceptance: String::new(),
                },
                states: vec![TaskState::Approved],
                submissions: vec!["done".into()],
                reviews: vec![],
                final_state: TaskState::Approved,
                failure_reason: String::new(),
            }],
        }
    }

    #[test]
    fn run_file_roundtrips_record_and_tool_evidence() {
        let rec = sample_record();
        let ev = vec![ToolEvidence {
            agent: "Version Control Agent".into(),
            tool: "git.run".into(),
            args_digest: "{\"argv\":[\"status\"]}".into(),
            summary: "git status → exit 0".into(),
            ok: true,
            ts: 123,
        }];
        let run = RunFile {
            record: rec,
            tool_calls: ev,
        };
        let json = serde_json::to_string_pretty(&run).unwrap();
        // New format carries both fields.
        assert!(json.contains("\"record\""));
        assert!(json.contains("\"tool_calls\""));
        let back: RunFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.record.workflow_id, "wf-1");
        assert_eq!(back.tool_calls.len(), 1);
        assert_eq!(back.tool_calls[0].tool, "git.run");
    }

    fn task(agent: &str, state: TaskState, submission: &str) -> TaskRecord {
        TaskRecord {
            spec: TaskSpec {
                id: "T".into(),
                agent: agent.into(),
                objective: String::new(),
                context: String::new(),
                reviewer: None,
                depends_on: vec![],
                acceptance: String::new(),
            },
            states: vec![state],
            submissions: vec![submission.into()],
            reviews: vec![],
            final_state: state,
            failure_reason: String::new(),
        }
    }

    #[test]
    fn only_approved_form_designer_tasks_yield_change_sets() {
        let cs = "```json\n{\"operations\":[{\"op\":\"deploy_control\",\"control_type\":\"Button\",\"id\":\"BTN\"}]}\n```";
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![
                task("Form Designer Agent", TaskState::Approved, cs),
                // Not approved → ignored.
                task("Form Designer Agent", TaskState::Failed, cs),
                // Approved but a different agent → ignored.
                task("Version Control Agent", TaskState::Approved, "done"),
            ],
        };
        let sets = approved_form_change_sets(&record, "Form Designer Agent");
        assert_eq!(sets.len(), 1, "one approved Form-Designer task");
        assert!(sets[0].is_ok(), "its submission parsed as a change-set");
        assert_eq!(sets[0].as_ref().unwrap().operations.len(), 1);
    }

    #[test]
    fn chatbot_reply_prefers_its_surface_specialist() {
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![
                task("Form Designer Agent", TaskState::Approved, "form result"),
                task(
                    "COBOL Event Handler Script Agent",
                    TaskState::Approved,
                    "event result",
                ),
            ],
        };
        assert_eq!(
            workflow_chat_reply(&record, Some("Form Designer Agent")),
            "form result"
        );
        let project_reply = workflow_chat_reply(&record, None);
        assert!(project_reply.contains("Form Designer Agent:\nform result"));
        assert!(project_reply.contains("COBOL Event Handler Script Agent:\nevent result"));
    }

    #[test]
    fn read_only_questions_are_direct_grace_conversations() {
        assert!(is_informational_grace_request("What can you do?"));
        assert!(is_informational_grace_request("How can you help me?"));
        assert!(is_informational_grace_request(
            "Describe the fiscal information normally held for a Spanish company"
        ));
        assert!(is_informational_grace_request(
            "Explain how indexed-file keys work"
        ));
        assert!(is_informational_grace_request(
            "Generate a description of the fiscal fields used for Spanish companies"
        ));
        assert!(!is_informational_grace_request(
            "Plan an accounts payable application"
        ));
        assert!(!is_informational_grace_request(
            "Describe the customer schema and create the indexed file"
        ));

        let record = direct_grace_record("I coordinate the project agents.".into());
        assert_eq!(
            workflow_chat_reply(&record, None),
            "I coordinate the project agents."
        );
        assert_eq!(record.tasks[0].spec.agent, GRACE);
    }

    #[test]
    fn legacy_bare_record_file_still_loads() {
        // A file written before spec 030 held a bare WorkflowRecord.
        let legacy = serde_json::to_string(&sample_record()).unwrap();
        let dir = std::env::temp_dir().join(format!("prc-runfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.json");
        std::fs::write(&path, legacy).unwrap();
        let rf = load_run_file(&path).unwrap();
        assert_eq!(rf.record.workflow_id, "wf-1");
        assert!(
            rf.tool_calls.is_empty(),
            "legacy files have no tool evidence"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn interface_documentation_requires_designer_then_documentation_agent() {
        let invalid = vec![TaskSpec {
            id: "T1".into(),
            agent: "Form Designer Agent".into(),
            objective: "Write and save the interface documentation".into(),
            context: "/Knowledge Base/forms/customer.md".into(),
            reviewer: None,
            depends_on: vec![],
            acceptance: "document exists".into(),
        }];
        assert!(
            validate_documentation_coordination("Document the form interface", &invalid)
                .unwrap_err()
                .contains("only Documentation Agent")
        );

        let valid = vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Form Designer Agent".into(),
                objective: "Prepare the authoritative interface inventory".into(),
                context: "controls, layout, bindings, and events".into(),
                reviewer: None,
                depends_on: vec![],
                acceptance: "source material is complete; no file is written".into(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: DOCUMENTATION_AGENT.into(),
                objective: "Format and save the interface documentation".into(),
                context: "/Knowledge Base/forms/customer.md".into(),
                reviewer: None,
                depends_on: vec!["T1".into()],
                acceptance: "document is formatted and indexed".into(),
            },
        ];
        assert!(validate_documentation_coordination("Document the form interface", &valid).is_ok());
    }

    #[test]
    fn documentation_accepts_source_preparation_and_requires_every_source() {
        let mut plan = vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Form Designer Agent".into(),
                objective: "Create source material for the interface documentation".into(),
                context: "Inventory controls and layout without writing a project document".into(),
                reviewer: None,
                depends_on: vec![],
                acceptance: "Authoritative interface facts are returned to Grace".into(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "COBOL Event Handler Script Agent".into(),
                objective: "Produce the event-handler inventory for documentation".into(),
                context: "Report event bindings as source material only".into(),
                reviewer: None,
                depends_on: vec![],
                acceptance: "Authoritative event facts are returned to Grace".into(),
            },
            TaskSpec {
                id: "T3".into(),
                agent: DOCUMENTATION_AGENT.into(),
                objective: "Format and save the interface documentation".into(),
                context: "/Knowledge Base/forms/customer.md".into(),
                reviewer: None,
                depends_on: vec!["T1".into()],
                acceptance: "The approved source material is documented".into(),
            },
        ];

        let error =
            validate_documentation_coordination("Document the form interface", &plan).unwrap_err();
        assert!(error.contains("missing: T2"));

        plan[2].depends_on.push("T2".into());
        assert!(validate_documentation_coordination("Document the form interface", &plan).is_ok());
    }

    fn indexed_schema_task() -> TaskSpec {
        TaskSpec {
            id: "T1".into(),
            agent: DOCUMENTATION_AGENT.into(),
            objective: "Establish the indexed file name and purpose; search project knowledge"
                .into(),
            context: "Analyze 1NF, 2NF, and 3NF; obtain the developer's UUID or PIC choice for every ID field and identify helper files".into(),
            reviewer: Some(crate::agents_db::PEDANTIC_DOCUMENTATION_REVIEWER.into()),
            depends_on: vec![],
            acceptance: "Approved schema handoff or a focused clarification asking for missing information".into(),
        }
    }

    #[test]
    fn indexed_file_mutation_requires_documentation_handoff_then_data_agent() {
        let invalid = vec![TaskSpec {
            id: "T1".into(),
            agent: GRACE.into(),
            objective: "Create the customer indexed file".into(),
            context: ".cidx mutation".into(),
            reviewer: None,
            depends_on: vec![],
            acceptance: "file saved".into(),
        }];
        let error =
            validate_indexed_file_coordination("Create an indexed file for customers", &invalid)
                .unwrap_err();
        assert!(error.contains("only Data (Indexed File) Agent"));

        let mut valid = vec![indexed_schema_task()];
        valid.push(TaskSpec {
            id: "T2".into(),
            agent: DATA_INDEXED_FILE_AGENT.into(),
            objective: "Create the approved customer indexed file with indexed_file.write".into(),
            context: "Use only the approved normalized schema and ID policy".into(),
            reviewer: Some(crate::agents_db::PEDANTIC_DATA_INDEXED_FILE_REVIEWER.into()),
            depends_on: vec!["T1".into()],
            acceptance: ".cidx and generated artifacts have successful tool evidence".into(),
        });
        assert!(
            validate_indexed_file_coordination("Create an indexed file for customers", &valid)
                .is_ok()
        );
    }

    #[test]
    fn documentation_may_prepare_an_indexed_schema_without_being_called_a_mutator() {
        let documentation = TaskSpec {
            id: "T1".into(),
            agent: DOCUMENTATION_AGENT.into(),
            objective:
                "Create the normalized indexed-file schema handoff for detailed Spanish company fiscal information"
                    .into(),
            context: "Establish requirements and return a proposed schema without writing .cidx"
                .into(),
            reviewer: Some(crate::agents_db::PEDANTIC_DOCUMENTATION_REVIEWER.into()),
            depends_on: vec![],
            acceptance: "The approved schema proposal is ready for the Data agent".into(),
        };
        assert!(!task_claims_indexed_mutation(&documentation));

        let data = TaskSpec {
            id: "T2".into(),
            agent: DATA_INDEXED_FILE_AGENT.into(),
            objective: "Create the approved indexed file using indexed_file.write".into(),
            context: "Use the Documentation Agent handoff".into(),
            reviewer: Some(crate::agents_db::PEDANTIC_DATA_INDEXED_FILE_REVIEWER.into()),
            depends_on: vec!["T1".into()],
            acceptance: "The .cidx and generated artifacts are saved".into(),
        };
        assert!(validate_indexed_file_coordination(
            "Create an indexed file containing detailed company and Spanish fiscal information",
            &[documentation.clone(), data.clone()]
        )
        .is_ok());
        let empty_db = AgentsDb::load(&std::env::temp_dir().join(format!(
            "prc-coord-{}",
            crate::agents_db::new_uuid()
        )));
        let coordinated = validate_workflow_coordination(
            &empty_db,
            "Create an indexed file containing detailed company and Spanish fiscal information",
            &[documentation, data],
        );
        assert!(coordinated.is_ok(), "{coordinated:?}");
    }

    #[test]
    fn indexed_file_clarification_can_stop_before_mutation() {
        let clarification = vec![indexed_schema_task()];
        assert!(validate_indexed_file_coordination(
            "Create an indexed file for customers",
            &clarification
        )
        .is_ok());
    }

    #[test]
    fn indexed_file_inspection_does_not_require_a_schema_handoff() {
        let inspection = vec![TaskSpec {
            id: "T1".into(),
            agent: DATA_INDEXED_FILE_AGENT.into(),
            objective: "Inspect the customer indexed file with indexed_file.read".into(),
            context: "Report its existing fields and keys without changes".into(),
            reviewer: Some(crate::agents_db::PEDANTIC_DATA_INDEXED_FILE_REVIEWER.into()),
            depends_on: vec![],
            acceptance: "The existing definition is reported without mutation".into(),
        }];
        assert!(validate_indexed_file_coordination(
            "Inspect indexed file indexed/customers.cidx",
            &inspection
        )
        .is_ok());
    }

    /// Grace planned a second "review the completed change" task with the
    /// Pedantic reviewer as its responsible agent. Reviewers have no model of
    /// their own, so the specialist path resolves empty credentials and the
    /// provider rejects the call — the workflow dies after the real work passed.
    #[test]
    fn a_pedantic_reviewer_may_not_be_a_task_agent() {
        let project = std::env::temp_dir()
            .join(format!("prc-pedantic-task-{}", crate::agents_db::new_uuid()));
        std::fs::create_dir_all(&project).unwrap();
        let mut db = AgentsDb::load(&project);
        db.ensure_fixed_agents(&LlmConfig::load_defaults_for_test());

        let review_task = TaskSpec {
            id: "T2".into(),
            agent: crate::agents_db::PEDANTIC_FORM_DESIGNER_REVIEWER.into(),
            objective: "Review the completed ACTORS-FORM style change".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec!["T1".into()],
            acceptance: "Explicit approval of the final form state".into(),
        };
        let rejected = validate_no_pedantic_task_agent(&db, &[review_task]);
        assert!(
            rejected.is_err(),
            "a Pedantic reviewer assigned as a task agent must be rejected"
        );

        // The legitimate shape — specialist owns the task, reviewer reviews it.
        let proper = TaskSpec {
            id: "T1".into(),
            agent: crate::agents_db::FORM_DESIGNER.into(),
            objective: "Set Form.GlassStyle to \"Neumorphic Dark\"".into(),
            context: String::new(),
            reviewer: Some(crate::agents_db::PEDANTIC_FORM_DESIGNER_REVIEWER.into()),
            depends_on: vec![],
            acceptance: "GlassStyle is exactly \"Neumorphic Dark\"".into(),
        };
        assert!(validate_no_pedantic_task_agent(&db, &[proper]).is_ok());
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn runtime_contract_names_each_agents_exact_pedantic_relationship() {
        let project = std::env::temp_dir().join(format!(
            "prc-review-contract-{}",
            crate::agents_db::new_uuid()
        ));
        std::fs::create_dir_all(&project).unwrap();
        let mut db = AgentsDb::load(&project);
        let llm = LlmConfig::load_defaults_for_test();
        db.ensure_fixed_agents(&llm);

        let grace = pedantic_relationship_contract(&db, GRACE);
        assert!(grace.contains(crate::agents_db::PEDANTIC_GRACE_REVIEWER));
        let reviewer =
            pedantic_relationship_contract(&db, crate::agents_db::PEDANTIC_GRACE_REVIEWER);
        assert!(reviewer.contains("sole Pedantic companion for Grace"));
        let version_control =
            pedantic_relationship_contract(&db, crate::agents_db::VERSION_CONTROL);
        assert!(version_control.contains(crate::agents_db::PEDANTIC_VERSION_CONTROL_REVIEWER));
        let _ = std::fs::remove_dir_all(project);
    }
}
