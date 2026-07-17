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

use cobolt_agents::grace::{AgentInvoker, GraceEngine, WorkflowRecord};

use crate::agents_db::{AgentsDb, GRACE};
use crate::llm::{api_key_slot, LlmConfig, LlmResponse};

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
        if a.model.trim().is_empty() || a.provider.trim().is_empty() {
            return Err(format!("Agent \u{201c}{agent}\u{201d} has no model configured."));
        }
        let mut cfg = self.llm.clone();
        cfg.provider = a.provider.clone();
        cfg.endpoint = a.endpoint.clone();
        cfg.model = a.model.clone();
        cfg.temperature = a.temperature;
        cfg.max_tokens = a.max_tokens;
        cfg.timeout_secs = a.timeout_secs;
        cfg.api_key = self
            .llm
            .api_keys
            .get(&api_key_slot(&a.provider, &a.model))
            .cloned()
            .unwrap_or_default();
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

/// Persist a workflow record under `agentic_ai/Grace/runs/<workflow-id>.json`
/// (spec 029 observability). Returns the file path.
pub fn save_workflow_record(
    project_dir: &Path,
    record: &WorkflowRecord,
) -> Result<PathBuf, String> {
    let dir = crate::agent::project_agentic_root(project_dir)
        .join(GRACE)
        .join("runs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.workflow_id));
    let json = serde_json::to_string_pretty(record).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Run one complete Grace workflow for `request`: Grace plans (structured
/// output), the engine executes with review gates + bounded corrections, the
/// record is persisted. Returns (record, record_path). Blocking — call from
/// a worker thread.
pub fn run_grace_workflow(
    project_dir: &Path,
    llm: &LlmConfig,
    request: &str,
) -> Result<(WorkflowRecord, PathBuf), String> {
    let db = AgentsDb::load(project_dir);
    if db.by_name(GRACE).is_none() {
        return Err("Grace is not configured — open the Agent Manager once to create her.".into());
    }
    let mut invoker = DbAgentInvoker {
        project_dir: project_dir.to_path_buf(),
        llm: llm.clone(),
    };
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
    let plan_reply = invoker.invoke(GRACE, "", &plan_user)?;
    let (workflow_id, plan) = cobolt_agents::grace::parse_plan(&plan_reply)?;

    let db2 = AgentsDb::load(project_dir);
    let system_for = move |name: &str| db2.load_prompt(name);
    let record = GraceEngine::default().run(&workflow_id, &plan, &mut invoker, &system_for);
    let path = save_workflow_record(project_dir, &record)?;
    Ok((record, path))
}
