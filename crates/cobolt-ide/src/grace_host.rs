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

use cobolt_agents::grace::{AgentInvoker, GraceEngine, GraceEvent, WorkflowRecord};
use serde::{Deserialize, Serialize};

use crate::agents_db::{AgentsDb, GRACE};
use crate::git_exec::GitConfirmRequest;
use crate::llm::{LlmConfig, LlmResponse};
use crate::tool_exec::{IdeToolBackend, ToolEvidence, ToolExecutingInvoker};

/// Bound on tool-execution rounds per task (spec 030) — guards a model that
/// never stops emitting tool calls.
const MAX_TOOL_ROUNDS: usize = 6;

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
    Ok(RunFile { record, tool_calls: Vec::new() })
}

/// Synchronous invoker over the project agent database. Each call resolves
/// the agent fresh (config can change between tasks) and blocks until the
/// model's full reply arrived.
pub struct DbAgentInvoker {
    pub project_dir: PathBuf,
    pub llm: LlmConfig,
}

impl DbAgentInvoker {
    fn config_for(&self, agent: &str) -> Result<(LlmConfig, String), String> {
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
        Ok((cfg, db.load_prompt(&a.name)))
    }
}

impl AgentInvoker for DbAgentInvoker {
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String> {
        let (cfg, prompt_file) = self.config_for(agent)?;
        let rx = crate::llm::spawn_named_agent_request(
            &cfg,
            if system.trim().is_empty() {
                &prompt_file
            } else {
                system
            },
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
        GraceEvent::TaskStarted { id, agent, objective } => {
            format!("▸ {id}: delegating to {agent} — {objective}")
        }
        GraceEvent::Submitted { id, agent } => format!("  {id}: {agent} submitted a result"),
        GraceEvent::ReviewStarted { id, reviewer, round } => {
            format!("  {id}: {reviewer} reviewing (round {})", round + 1)
        }
        GraceEvent::Verdict { id, reviewer, approved } => format!(
            "  {id}: {reviewer} verdict — {}",
            if *approved { "approved" } else { "defects found" }
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
    let db = AgentsDb::load(project_dir);
    if db.by_name(GRACE).is_none() {
        return Err("Grace is not configured — open the Agent Manager once to create her.".into());
    }
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
                if a.specialization.is_empty() { "—" } else { &a.specialization },
                if a.enabled { "" } else { " (DISABLED)" },
                comp,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let plan_user = format!(
        "USER REQUEST:\n{request}\n\nAVAILABLE AGENT REGISTRY:\n{registry}\n\nPlan the workflow per your tooling contract (END with the plan JSON). Assign each task's reviewer from the responsible agent's pedantic companion; leave reviewer null only where no companion exists."
    );
    on_progress("Grace is planning the workflow…".into());
    let plan_reply = invoker.invoke(GRACE, "", &plan_user)?;
    let (workflow_id, plan) = cobolt_agents::grace::parse_plan(&plan_reply)?;
    on_progress(format!(
        "Grace planned {} task(s) [{}].",
        plan.len(),
        workflow_id
    ));

    let db2 = AgentsDb::load(project_dir);
    let system_for = move |name: &str| {
        let base = db2.load_prompt(name);
        // Append the tool-calling contract for whatever tools this agent
        // declares (spec 030 R2) — always consistent with its actual grant.
        let declared: std::collections::HashSet<String> = db2
            .by_name(name)
            .map(|a| a.tools.iter().cloned().collect())
            .unwrap_or_default();
        let appendix = crate::tool_exec::tool_contract_appendix(&declared);
        if appendix.is_empty() {
            base
        } else {
            format!("{base}{appendix}")
        }
    };
    let ev_for_progress = evidence.clone();
    let mut emitted = 0usize;
    let mut flush_tools = move |on_progress: &mut dyn FnMut(String)| {
        let ev = ev_for_progress.lock().unwrap();
        while emitted < ev.len() {
            let e = &ev[emitted];
            on_progress(format!(
                "  \u{1f527} {} — {}",
                e.tool,
                e.summary
            ));
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
    on_progress(format!("Workflow {}: {}.", record.workflow_id, record.status));
    Ok((record, path))
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
        let run = RunFile { record: rec, tool_calls: ev };
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
    fn legacy_bare_record_file_still_loads() {
        // A file written before spec 030 held a bare WorkflowRecord.
        let legacy = serde_json::to_string(&sample_record()).unwrap();
        let dir = std::env::temp_dir().join(format!("prc-runfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.json");
        std::fs::write(&path, legacy).unwrap();
        let rf = load_run_file(&path).unwrap();
        assert_eq!(rf.record.workflow_id, "wf-1");
        assert!(rf.tool_calls.is_empty(), "legacy files have no tool evidence");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
