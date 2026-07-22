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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowRecord {
    pub workflow_id: String,
    pub tasks: Vec<TaskRecord>,
    /// completed | partial | failed
    pub status: String,
    /// Total prompt tokens consumed by the whole workflow (0 when unknown).
    #[serde(default)]
    pub input_tokens: u64,
    /// Total completion tokens consumed by the whole workflow (0 when unknown).
    #[serde(default)]
    pub output_tokens: u64,
    /// Short (<=15 word) summary of the relevant Knowledge Base evidence, if any.
    #[serde(default)]
    pub knowledge_summary: String,
    /// Grace's concise, user-facing one-line summary of the completed work.
    #[serde(default)]
    pub final_summary: String,
}

/// Host-supplied transport: invoke one named agent with a system+user prompt
/// and return its full reply. The host resolves models, keys, endpoints.
pub trait AgentInvoker {
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String>;
}

/// A live workflow transition (spec 029 Phase C). Emitted by the engine so an
/// interactive host can show Grace's coordination as it happens without
/// blocking on the whole run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraceEvent {
    /// Grace dispatched a task to a specialist.
    TaskStarted {
        id: String,
        agent: String,
        objective: String,
    },
    /// The specialist returned a submission (with evidence).
    Submitted {
        id: String,
        agent: String,
    },
    /// A pedantic review round began.
    ReviewStarted {
        id: String,
        reviewer: String,
        round: usize,
    },
    /// A review round's verdict landed.
    Verdict {
        id: String,
        reviewer: String,
        approved: bool,
    },
    /// The reviewer found defects; a correction was requested.
    CorrectionRequested {
        id: String,
        round: usize,
    },
    /// A task reached a terminal state.
    Approved {
        id: String,
    },
    Failed {
        id: String,
        reason: String,
    },
    Blocked {
        id: String,
    },
}

/// Convenience: a no-op observer for the non-streaming [`GraceEngine::run`].
fn no_progress(_: GraceEvent) {}

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
        self.run_with_progress(workflow_id, plan, invoker, system_for, &mut no_progress)
    }

    /// Like [`Self::run`], but streams a [`GraceEvent`] at every transition so
    /// an interactive host can render Grace's progress live (spec 029 Phase C).
    pub fn run_with_progress(
        &self,
        workflow_id: &str,
        plan: &[TaskSpec],
        invoker: &mut dyn AgentInvoker,
        system_for: &dyn Fn(&str) -> String,
        on_event: &mut dyn FnMut(GraceEvent),
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
                matches!(
                    s,
                    TaskState::Approved | TaskState::Failed | TaskState::Blocked
                )
            };
            let Some(idx) = records.iter().position(|r| {
                !terminal(r.final_state) && r.spec.depends_on.iter().all(|d| approved.contains(d))
            }) else {
                // No runnable task left: block whatever still waits on a
                // failed dependency.
                for r in &mut records {
                    if !terminal(r.final_state) {
                        r.states.push(TaskState::Blocked);
                        r.final_state = TaskState::Blocked;
                        r.failure_reason = "dependency not approved".into();
                        on_event(GraceEvent::Blocked {
                            id: r.spec.id.clone(),
                        });
                    }
                }
                break;
            };
            let dependency_outputs = records[idx]
                .spec
                .depends_on
                .iter()
                .filter_map(|dependency_id| {
                    records
                        .iter()
                        .find(|record| {
                            record.spec.id == *dependency_id
                                && record.final_state == TaskState::Approved
                        })
                        .and_then(|record| {
                            record.submissions.last().map(|submission| {
                                format!(
                                    "DEPENDENCY {id} — {agent}\nOBJECTIVE: {objective}\nAPPROVED OUTPUT:\n{submission}",
                                    id = record.spec.id,
                                    agent = record.spec.agent,
                                    objective = record.spec.objective,
                                )
                            })
                        })
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");
            self.run_task(
                &mut records[idx],
                &dependency_outputs,
                invoker,
                system_for,
                on_event,
            );
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
            ..Default::default()
        }
    }

    fn run_task(
        &self,
        rec: &mut TaskRecord,
        dependency_outputs: &str,
        invoker: &mut dyn AgentInvoker,
        system_for: &dyn Fn(&str) -> String,
        on_event: &mut dyn FnMut(GraceEvent),
    ) {
        rec.states.push(TaskState::Ready);
        rec.states.push(TaskState::Running);
        let spec = rec.spec.clone();
        on_event(GraceEvent::TaskStarted {
            id: spec.id.clone(),
            agent: spec.agent.clone(),
            objective: spec.objective.clone(),
        });
        let dependency_outputs = if dependency_outputs.trim().is_empty() {
            "(no dependency outputs)"
        } else {
            dependency_outputs
        };
        let user = format!(
            "TASK {id}: {obj}\n\nCONTEXT (identifiers are exact — do not paraphrase):\n{ctx}\n\nAPPROVED DEPENDENCY OUTPUTS (authoritative handoff from prior specialists):\n{dependency_outputs}\n\nACCEPTANCE CRITERIA:\n{acc}\n\nReturn the complete result. A bare claim of completion without the actual artifact is a failure.",
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
                on_event(GraceEvent::Failed {
                    id: spec.id.clone(),
                    reason: rec.failure_reason.clone(),
                });
                return;
            }
            Err(e) => {
                rec.states.push(TaskState::Failed);
                rec.final_state = TaskState::Failed;
                rec.failure_reason = e.clone();
                on_event(GraceEvent::Failed {
                    id: spec.id.clone(),
                    reason: e,
                });
                return;
            }
        };
        rec.submissions.push(submission.clone());
        on_event(GraceEvent::Submitted {
            id: spec.id.clone(),
            agent: spec.agent.clone(),
        });

        let Some(reviewer) = spec.reviewer.clone() else {
            // No mandated review gate for this task.
            rec.states.push(TaskState::Approved);
            rec.final_state = TaskState::Approved;
            on_event(GraceEvent::Approved {
                id: spec.id.clone(),
            });
            return;
        };

        // Review gate + bounded correction loop.
        for round in 0..=self.max_revisions {
            rec.states.push(TaskState::AwaitingReview);
            on_event(GraceEvent::ReviewStarted {
                id: spec.id.clone(),
                reviewer: reviewer.clone(),
                round,
            });
            let review_user = format!(
                "This is a review round: review the response and END with the round-verdict JSON per your tooling contract.\n\n=== AUTHORITATIVE TASK ===\n{obj}\n\nCONTEXT:\n{ctx}\n\nAPPROVED DEPENDENCY OUTPUTS:\n{dependency_outputs}\n\nACCEPTANCE CRITERIA:\n{acc}\n\n=== SUBMISSION UNDER REVIEW ===\n{submission}",
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
                    on_event(GraceEvent::Failed {
                        id: spec.id.clone(),
                        reason: rec.failure_reason.clone(),
                    });
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
            on_event(GraceEvent::Verdict {
                id: spec.id.clone(),
                reviewer: reviewer.clone(),
                approved: !defects,
            });
            if !defects {
                rec.states.push(TaskState::Approved);
                rec.final_state = TaskState::Approved;
                on_event(GraceEvent::Approved {
                    id: spec.id.clone(),
                });
                return;
            }
            if round == self.max_revisions {
                break;
            }
            // Correction round: the specialist resubmits the COMPLETE result.
            rec.states.push(TaskState::CorrectionRequired);
            on_event(GraceEvent::CorrectionRequested {
                id: spec.id.clone(),
                round: round + 1,
            });
            let fix_user = format!(
                "TASK {id}: {obj}\n\n=== APPROVED DEPENDENCY OUTPUTS ===\n{dependency_outputs}\n\n=== YOUR PREVIOUS COMPLETE RESPONSE ===\n{submission}\n\n=== PEDANTIC CORRECTION REQUEST ===\n{req}\n\nCorrect the defects and submit the COMPLETE result again — a full replacement, not isolated patches.",
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
                    on_event(GraceEvent::Failed {
                        id: spec.id.clone(),
                        reason: rec.failure_reason.clone(),
                    });
                    return;
                }
            };
            rec.submissions.push(submission.clone());
            rec.states.push(TaskState::Revalidating);
            on_event(GraceEvent::Submitted {
                id: spec.id.clone(),
                agent: spec.agent.clone(),
            });
        }
        rec.states.push(TaskState::Failed);
        rec.final_state = TaskState::Failed;
        rec.failure_reason = format!(
            "not approved after {} revision(s) — bounded correction loop exhausted",
            self.max_revisions
        );
        on_event(GraceEvent::Failed {
            id: spec.id.clone(),
            reason: rec.failure_reason.clone(),
        });
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
                reviewer: Some("Form Designer Agent Pedantic Reviewer".into()),
                depends_on: vec![],
                acceptance: "button exists, tab order intact".into(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "COBOL Event Handler Script Agent".into(),
                objective: "Implement BTN-OK onClick".into(),
                context: "control BTN-OK, event onClick".into(),
                reviewer: Some("COBOL Event Handler Script Agent Pedantic Reviewer".into()),
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
                    "Form Designer Agent Pedantic Reviewer",
                    "misaligned.\n```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. align BTN-OK to the button row\"}\n```",
                ),
                ("Form Designer Agent", "deployed BTN-OK v2, aligned"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "clean.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
                ("COBOL Event Handler Script Agent", "MOVE 1 TO WS-OK."),
                (
                    "COBOL Event Handler Script Agent Pedantic Reviewer",
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
        assert_eq!(
            t1.submissions.len(),
            2,
            "both submissions preserved as evidence"
        );
        assert_eq!(t1.reviews.len(), 2);
        assert!(t1.reviews[0].defects && !t1.reviews[1].defects);
        // The correction request reached the specialist verbatim.
        assert!(mock.calls[2].1.contains("align BTN-OK to the button row"));
        let t2 = &rec.tasks[1];
        assert_eq!(t2.final_state, TaskState::Approved);
    }

    /// Phase C: the live event stream mirrors the workflow — task start,
    /// review, a correction, the second verdict approving, then the
    /// dependent task, ending with both Approved.
    #[test]
    fn progress_events_stream_the_workflow() {
        let mut mock = Mock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "v1"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. fix\"}\n```",
                ),
                ("Form Designer Agent", "v2"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
                ("COBOL Event Handler Script Agent", "MOVE 1 TO WS-OK."),
                (
                    "COBOL Event Handler Script Agent Pedantic Reviewer",
                    "```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
            ],
        };
        let mut events: Vec<GraceEvent> = Vec::new();
        let rec = GraceEngine::default().run_with_progress(
            "wf",
            &plan2(),
            &mut mock,
            &|_| "s".into(),
            &mut |e| events.push(e),
        );
        assert_eq!(rec.status, "completed");
        let started: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                GraceEvent::TaskStarted { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            started,
            vec!["T1", "T2"],
            "tasks started in dependency order"
        );
        assert!(events.iter().any(|e| matches!(e, GraceEvent::CorrectionRequested { id, round } if id == "T1" && *round == 1)));
        let approvals: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                GraceEvent::Approved { id } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(approvals, vec!["T1", "T2"]);
    }

    #[test]
    fn approved_specialist_output_is_handed_to_documentation_agent() {
        let plan = vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Form Designer Agent".into(),
                objective: "Prepare an authoritative inventory of the CUSTOMER form interface"
                    .into(),
                context: "CUSTOMER form".into(),
                reviewer: None,
                depends_on: vec![],
                acceptance: "Describe controls, layout, bindings, and events without writing files"
                    .into(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "Documentation Agent".into(),
                objective: "Format and save the CUSTOMER interface documentation".into(),
                context: "/Documentation/Forms/customer.md".into(),
                reviewer: None,
                depends_on: vec!["T1".into()],
                acceptance: "Write the document using the approved Form Designer output".into(),
            },
        ];
        let authoritative = "CUSTOMER contains BTN-SAVE and GRID-ORDERS bound to SQLConnection-1.";
        let mut mock = Mock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", authoritative),
                (
                    "Documentation Agent",
                    "saved Documentation/Forms/customer.md",
                ),
            ],
        };

        let record = GraceEngine::default().run("wf-doc", &plan, &mut mock, &|_| "sys".into());

        assert_eq!(record.status, "completed");
        assert_eq!(mock.calls[1].0, "Documentation Agent");
        assert!(mock.calls[1]
            .1
            .contains("DEPENDENCY T1 — Form Designer Agent"));
        assert!(mock.calls[1].1.contains(authoritative));
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
                ("Form Designer Agent Pedantic Reviewer", reject),
                ("Form Designer Agent", "v2"),
                ("Form Designer Agent Pedantic Reviewer", reject),
                ("Form Designer Agent", "v3"),
                ("Form Designer Agent Pedantic Reviewer", reject),
            ],
        };
        let rec = GraceEngine::default().run("wf-fail", &plan2(), &mut mock, &|_| "sys".into());
        assert_eq!(rec.status, "failed");
        assert_eq!(rec.tasks[0].final_state, TaskState::Failed);
        assert!(rec.tasks[0].failure_reason.contains("bounded"));
        assert_eq!(rec.tasks[1].final_state, TaskState::Blocked);
        assert_eq!(
            mock.calls.len(),
            6,
            "loop terminated — no uncontrolled retries"
        );
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
            "plan…\n```json\n{\"workflow_id\": \"w1\", \"tasks\": [{\"id\": \"T1\", \"agent\": \"Form Designer Agent\", \"objective\": \"x\", \"reviewer\": \"Form Designer Agent Pedantic Reviewer\", \"depends_on\": [], \"acceptance\": \"a\"}]}\n```",
        )
        .unwrap();
        assert_eq!(wf, "w1");
        assert_eq!(
            tasks[0].reviewer.as_deref(),
            Some("Form Designer Agent Pedantic Reviewer")
        );
    }
}
