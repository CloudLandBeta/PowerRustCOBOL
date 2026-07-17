// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Grace's workflow runtime (spec 029 Phase B).
//!
//! The engine is transport-agnostic: it drives task state machines, review
//! gates, and bounded correction loops against an [`AgentInvoker`] the host
//! supplies (the IDE maps agent names to real models via the project agent
//! database; tests use mocks). Grace's own PLANNING happens upstream — the
//! engine executes a dependency-aware plan and produces an auditable
//! [`WorkflowRecord`]. Only Approved tasks contribute to a completed result.

use serde::{Deserialize, Serialize};

/// The task states mandated by Grace's prompt (spec 029 R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Ready,
    Running,
    AwaitingDependency,
    AwaitingReview,
    CorrectionRequired,
    Revalidating,
    Approved,
    Blocked,
    Failed,
    Completed,
}

/// One delegated task (the delegation contract, condensed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    /// Responsible agent (agent-database name).
    pub agent: String,
    pub objective: String,
    /// Context the agent needs (identifiers preserved exactly).
    #[serde(default)]
    pub context: String,
    /// Reviewing pedantic agent, when the workflow requires one.
    #[serde(default)]
    pub reviewer: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub acceptance: String,
}

/// A specialist's structured return. "done" without evidence is rejected:
/// `output` must be non-empty.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskResult {
    pub output: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// One review round's parsed verdict (the pedantic tooling contract).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRound {
    pub reviewer: String,
    pub defects: bool,
    pub correction_request: String,
    pub raw: String,
}

/// Full audit trail for one task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub spec: TaskSpec,
    pub states: Vec<TaskState>,
    pub submissions: Vec<String>,
    pub reviews: Vec<ReviewRound>,
    pub final_state: TaskState,
    #[serde(default)]
    pub failure_reason: String,
}

/// The workflow record (spec 029 observability): saved by the host under
/// `agentic_ai/Grace/runs/<workflow_id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRecord {
    pub workflow_id: String,
    pub tasks: Vec<TaskRecord>,
    /// completed | partial | failed
    pub status: String,
}

/// Host-supplied transport: invoke one named agent with a system+user prompt
/// and return its full reply. The host resolves models, keys, endpoints.
pub trait AgentInvoker {
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String>;
}

/// Extract the LAST fenced JSON block of a reply (the tooling contract).
pub fn last_json_block(reply: &str) -> Option<serde_json::Value> {
    let mut last = None;
    let mut rest = reply;
    while let Some(start) = rest.find("```") {
        rest = &rest[start + 3..];
        let Some(end) = rest.find("```") else { break };
        let block = rest[..end].trim();
        rest = &rest[end + 3..];
        let json = block.strip_prefix("json").map(str::trim).unwrap_or(block);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
            last = Some(v);
        }
    }
    last
}

/// Parse Grace's plan JSON (`{"workflow_id": ..., "tasks": [...]}`) from her
/// planning reply, per her tooling contract.
pub fn parse_plan(reply: &str) -> Result<(String, Vec<TaskSpec>), String> {
    let v = last_json_block(reply).ok_or("Grace's plan contained no JSON block")?;
    let wf = v
        .get("workflow_id")
        .and_then(|x| x.as_str())
        .unwrap_or("workflow")
        .to_string();
    let tasks = v
        .get("tasks")
        .and_then(|t| serde_json::from_value::<Vec<TaskSpec>>(t.clone()).ok())
        .ok_or("Grace's plan tasks were malformed")?;
    if tasks.is_empty() {
        return Err("Grace's plan contained no tasks".into());
    }
    Ok((wf, tasks))
}

/// Engine configuration.
pub struct GraceEngine {
    /// Bounded correction loops (spec 029: default 2 revisions).
    pub max_revisions: usize,
}

impl Default for GraceEngine {
    fn default() -> Self {
        Self { max_revisions: 2 }
    }
}

impl GraceEngine {
    /// Execute a dependency-aware plan. Tasks whose dependencies are not
    /// Approved are blocked; every reviewed task passes its pedantic gate or
    /// fails; correction loops are bounded. Returns the full audit record.
    pub fn run(
        &self,
        workflow_id: &str,
        plan: &[TaskSpec],
        invoker: &mut dyn AgentInvoker,
        system_for: &dyn Fn(&str) -> String,
    ) -> WorkflowRecord {
        let mut records: Vec<TaskRecord> = plan
            .iter()
            .map(|t| TaskRecord {
                spec: t.clone(),
                states: vec![TaskState::Pending],
                submissions: Vec::new(),
                reviews: Vec::new(),
                final_state: TaskState::Pending,
                failure_reason: String::new(),
            })
            .collect();

        // Sequential dependency-order execution (parallelism is a Phase C
        // optimization; correctness first — spec 029 R4).
        loop {
            let approved: Vec<String> = records
                .iter()
                .filter(|r| r.final_state == TaskState::Approved)
                .map(|r| r.spec.id.clone())
                .collect();
            let terminal = |s: TaskState| {
                matches!(s, TaskState::Approved | TaskState::Failed | TaskState::Blocked)
            };
            let Some(idx) = records.iter().position(|r| {
                !terminal(r.final_state)
                    && r.spec.depends_on.iter().all(|d| approved.contains(d))
            }) else {
                // No runnable task left: block whatever still waits on a
                // failed dependency.
                for r in &mut records {
                    if !terminal(r.final_state) {
                        r.states.push(TaskState::Blocked);
                        r.final_state = TaskState::Blocked;
                        r.failure_reason = "dependency not approved".into();
                    }
                }
                break;
            };
            self.run_task(&mut records[idx], invoker, system_for);
        }

        let all_ok = records.iter().all(|r| r.final_state == TaskState::Approved);
        let any_ok = records.iter().any(|r| r.final_state == TaskState::Approved);
        WorkflowRecord {
            workflow_id: workflow_id.to_string(),
            tasks: records,
            status: if all_ok {
                "completed".into()
            } else if any_ok {
                "partial".into()
            } else {
                "failed".into()
            },
        }
    }

    fn run_task(
        &self,
        rec: &mut TaskRecord,
        invoker: &mut dyn AgentInvoker,
        system_for: &dyn Fn(&str) -> String,
    ) {
        rec.states.push(TaskState::Ready);
        rec.states.push(TaskState::Running);
        let spec = rec.spec.clone();
        let user = format!(
            "TASK {id}: {obj}\n\nCONTEXT (identifiers are exact — do not paraphrase):\n{ctx}\n\nACCEPTANCE CRITERIA:\n{acc}\n\nReturn the complete result. A bare claim of completion without the actual artifact is a failure.",
            id = spec.id,
            obj = spec.objective,
            ctx = spec.context,
            acc = spec.acceptance,
        );
        let mut submission = match invoker.invoke(&spec.agent, &system_for(&spec.agent), &user) {
            Ok(s) if !s.trim().is_empty() => s,
            Ok(_) => {
                rec.states.push(TaskState::Failed);
                rec.final_state = TaskState::Failed;
                rec.failure_reason = "empty result — completion without evidence".into();
                return;
            }
            Err(e) => {
                rec.states.push(TaskState::Failed);
                rec.final_state = TaskState::Failed;
                rec.failure_reason = e;
                return;
            }
        };
        rec.submissions.push(submission.clone());

        let Some(reviewer) = spec.reviewer.clone() else {
            // No mandated review gate for this task.
            rec.states.push(TaskState::Approved);
            rec.final_state = TaskState::Approved;
            return;
        };

        // Review gate + bounded correction loop.
        for round in 0..=self.max_revisions {
            rec.states.push(TaskState::AwaitingReview);
            let review_user = format!(
                "This is a review round: review the response and END with the round-verdict JSON per your tooling contract.\n\n=== AUTHORITATIVE TASK ===\n{obj}\n\nCONTEXT:\n{ctx}\n\nACCEPTANCE CRITERIA:\n{acc}\n\n=== SUBMISSION UNDER REVIEW ===\n{submission}",
                obj = spec.objective,
                ctx = spec.context,
                acc = spec.acceptance,
            );
            let review = match invoker.invoke(&reviewer, &system_for(&reviewer), &review_user) {
                Ok(r) => r,
                Err(e) => {
                    rec.states.push(TaskState::Failed);
                    rec.final_state = TaskState::Failed;
                    rec.failure_reason = format!("reviewer unavailable: {e}");
                    return;
                }
            };
            let verdict = last_json_block(&review);
            // No parseable verdict ⇒ pedantic about the pedant: defects.
            let defects = verdict
                .as_ref()
                .and_then(|v| v.get("pedantic_verdict"))
                .and_then(|v| v.as_str())
                .map(|v| !v.eq_ignore_ascii_case("acceptable"))
                .unwrap_or(true);
            let correction = verdict
                .as_ref()
                .and_then(|v| v.get("correction_request"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            rec.reviews.push(ReviewRound {
                reviewer: reviewer.clone(),
                defects,
                correction_request: correction.clone(),
                raw: review.clone(),
            });
            if !defects {
                rec.states.push(TaskState::Approved);
                rec.final_state = TaskState::Approved;
                return;
            }
            if round == self.max_revisions {
                break;
            }
            // Correction round: the specialist resubmits the COMPLETE result.
            rec.states.push(TaskState::CorrectionRequired);
            let fix_user = format!(
                "TASK {id}: {obj}\n\n=== YOUR PREVIOUS COMPLETE RESPONSE ===\n{submission}\n\n=== PEDANTIC CORRECTION REQUEST ===\n{req}\n\nCorrect the defects and submit the COMPLETE result again — a full replacement, not isolated patches.",
                id = spec.id,
                obj = spec.objective,
                req = if correction.trim().is_empty() { review.as_str() } else { correction.as_str() },
            );
            submission = match invoker.invoke(&spec.agent, &system_for(&spec.agent), &fix_user) {
                Ok(s) if !s.trim().is_empty() => s,
                Ok(_) | Err(_) => {
                    rec.states.push(TaskState::Failed);
                    rec.final_state = TaskState::Failed;
                    rec.failure_reason = "correction round returned no result".into();
                    return;
                }
            };
            rec.submissions.push(submission.clone());
            rec.states.push(TaskState::Revalidating);
        }
        rec.states.push(TaskState::Failed);
        rec.final_state = TaskState::Failed;
        rec.failure_reason = format!(
            "not approved after {} revision(s) — bounded correction loop exhausted",
            self.max_revisions
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripted mock: per-agent queued replies.
    struct Mock {
        calls: Vec<(String, String)>,
        script: Vec<(&'static str, &'static str)>,
    }
    impl AgentInvoker for Mock {
        fn invoke(&mut self, agent: &str, _s: &str, user: &str) -> Result<String, String> {
            self.calls.push((agent.to_string(), user.to_string()));
            let i = self.calls.len() - 1;
            let (expect, reply) = self.script[i];
            assert_eq!(agent, expect, "call #{i} routed to the wrong agent");
            Ok(reply.to_string())
        }
    }

    fn plan2() -> Vec<TaskSpec> {
        vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Form Designer Agent".into(),
                objective: "Add BTN-OK to MAIN-FORM".into(),
                context: "form MAIN-FORM, theme Liquid Glass".into(),
                reviewer: Some("Pedantic UI Agent".into()),
                depends_on: vec![],
                acceptance: "button exists, tab order intact".into(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "COBOL Event Handler Script Agent".into(),
                objective: "Implement BTN-OK onClick".into(),
                context: "control BTN-OK, event onClick".into(),
                reviewer: Some("Pedantic COBOL Companion".into()),
                depends_on: vec!["T1".into()],
                acceptance: "COBOL-85 clean".into(),
            },
        ]
    }

    /// AC5 (spec 029): design → review(defect) → correct → approve, then the
    /// dependent task; the record carries the mandated states + evidence.
    #[test]
    fn two_task_workflow_with_correction_loop() {
        let mut mock = Mock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "deployed BTN-OK v1"),
                (
                    "Pedantic UI Agent",
                    "misaligned.\n```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. align BTN-OK to the button row\"}\n```",
                ),
                ("Form Designer Agent", "deployed BTN-OK v2, aligned"),
                (
                    "Pedantic UI Agent",
                    "clean.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
                ("COBOL Event Handler Script Agent", "MOVE 1 TO WS-OK."),
                (
                    "Pedantic COBOL Companion",
                    "fine.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
            ],
        };
        let rec = GraceEngine::default().run("wf-test", &plan2(), &mut mock, &|_| "sys".into());
        assert_eq!(rec.status, "completed");
        let t1 = &rec.tasks[0];
        assert_eq!(t1.final_state, TaskState::Approved);
        assert!(t1.states.contains(&TaskState::CorrectionRequired));
        assert!(t1.states.contains(&TaskState::Revalidating));
        assert_eq!(t1.submissions.len(), 2, "both submissions preserved as evidence");
        assert_eq!(t1.reviews.len(), 2);
        assert!(t1.reviews[0].defects && !t1.reviews[1].defects);
        // The correction request reached the specialist verbatim.
        assert!(mock.calls[2].1.contains("align BTN-OK to the button row"));
        let t2 = &rec.tasks[1];
        assert_eq!(t2.final_state, TaskState::Approved);
    }

    /// Bounded loop: persistent defects exhaust max_revisions → Failed, and
    /// the dependent task is Blocked (never silently completed).
    #[test]
    fn exhausted_corrections_fail_and_block_dependents() {
        let reject = "no.\n```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. still wrong\"}\n```";
        let mut mock = Mock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "v1"),
                ("Pedantic UI Agent", reject),
                ("Form Designer Agent", "v2"),
                ("Pedantic UI Agent", reject),
                ("Form Designer Agent", "v3"),
                ("Pedantic UI Agent", reject),
            ],
        };
        let rec = GraceEngine::default().run("wf-fail", &plan2(), &mut mock, &|_| "sys".into());
        assert_eq!(rec.status, "failed");
        assert_eq!(rec.tasks[0].final_state, TaskState::Failed);
        assert!(rec.tasks[0].failure_reason.contains("bounded"));
        assert_eq!(rec.tasks[1].final_state, TaskState::Blocked);
        assert_eq!(mock.calls.len(), 6, "loop terminated — no uncontrolled retries");
    }

    /// "done" without evidence is rejected; plan parsing round-trips.
    #[test]
    fn evidence_and_plan_contract() {
        struct Empty;
        impl AgentInvoker for Empty {
            fn invoke(&mut self, _: &str, _: &str, _: &str) -> Result<String, String> {
                Ok("   ".into())
            }
        }
        let rec = GraceEngine::default().run("wf-e", &plan2()[..1], &mut Empty, &|_| "s".into());
        assert_eq!(rec.tasks[0].final_state, TaskState::Failed);
        assert!(rec.tasks[0].failure_reason.contains("evidence"));

        let (wf, tasks) = parse_plan(
            "plan…\n```json\n{\"workflow_id\": \"w1\", \"tasks\": [{\"id\": \"T1\", \"agent\": \"Form Designer Agent\", \"objective\": \"x\", \"reviewer\": \"Pedantic UI Agent\", \"depends_on\": [], \"acceptance\": \"a\"}]}\n```",
        )
        .unwrap();
        assert_eq!(wf, "w1");
        assert_eq!(tasks[0].reviewer.as_deref(), Some("Pedantic UI Agent"));
    }
}
