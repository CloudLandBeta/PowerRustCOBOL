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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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

/// Grace's typed workflow plan (Rig migration, phase 3). Extractable: the
/// schema is what the provider's forced `submit` tool call must satisfy.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowPlan {
    /// Short kebab-case workflow identifier.
    pub workflow_id: String,
    /// The ordered, dependency-aware task list. Never empty for an executable plan.
    pub tasks: Vec<TaskSpec>,
}

/// A Pedantic reviewer's typed round verdict (Rig migration, phase 3).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReviewVerdict {
    /// "acceptable" when the submission passes review; any other value
    /// (canonically "defects") means corrections are required.
    pub pedantic_verdict: String,
    /// The numbered correction list when defects were found; empty otherwise.
    #[serde(default)]
    pub correction_request: String,
    /// The operations the defects belong to, named as the submission names
    /// them (`generate_event_handler txt8.onChange`, `deploy_control txt3`).
    ///
    /// When the reviewer fills this in, the engine sends back ONLY those
    /// operations: everything it approved is kept verbatim and never passes
    /// through the model again. Empty means "not attributed" and the whole
    /// submission is redone, which is what always used to happen — and it
    /// churned correct work, since a specialist asked to resubmit everything
    /// routinely rewrites operations nobody complained about.
    #[serde(default)]
    pub defective_ops: Vec<String>,
}

impl ReviewVerdict {
    pub fn has_defects(&self) -> bool {
        !self.pedantic_verdict.eq_ignore_ascii_case("acceptable")
    }
}

/// A defective submission split in two, so a correction round rewrites only
/// what was actually wrong (see [`AgentInvoker::scope_correction`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionScope {
    /// The operations that stand, as a change-set JSON block. Kept by the
    /// engine, never sent to the model, merged back when the fix returns.
    pub accepted: String,
    /// The operations to redo, as a change-set JSON block — the only thing
    /// the specialist is shown and asked to rewrite.
    pub defective: String,
    /// How many operations are on each side, for the correction prompt.
    pub accepted_count: usize,
    pub defective_count: usize,
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

/// One live-action status entry (spec 036): the *action* an agent performed,
/// never the content the action produced or consumed. `kind` is a stable
/// snake_case identifier (e.g. `retrieving_context`) so saved records stay
/// language-neutral — the IDE localizes it at render time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionLogEntry {
    /// The agent performing the action ("Grace", a specialist, a reviewer).
    pub agent: String,
    /// Stable snake_case action kind.
    pub kind: String,
    /// Short dynamic fragment (task id, tool name…), verbatim; may be empty.
    #[serde(default)]
    pub detail: String,
    /// Milliseconds since the Unix epoch when the action started.
    #[serde(default)]
    pub at: u64,
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
    /// The ordered live-action history of the run (spec 036 R11). Absent in
    /// records written before 1.38.0 — `serde(default)` keeps them readable.
    #[serde(default)]
    pub action_log: Vec<ActionLogEntry>,
    /// Estimated tokens of the ENTIRE indexed Knowledge Base corpus (system +
    /// project) — what a push-everything approach would have injected. 0 when
    /// unknown (older records).
    #[serde(default)]
    pub kb_available_tokens: u64,
    /// Estimated tokens of the Knowledge Base material actually injected into
    /// the planning context (essential documents + retrieved excerpts).
    #[serde(default)]
    pub kb_injected_tokens: u64,
    /// Grace's private 0–10 rating of the developer's request clarity and
    /// conciseness, evaluated BEFORE any Knowledge Base retrieval. Telemetry
    /// for the record only: it must never be injected into any agent task
    /// prompt. `None` on older records and when the pre-check was skipped or
    /// unparseable.
    #[serde(default)]
    pub request_clarity: Option<u8>,
    /// Per-agent usage totals across the run (model calls + typed extraction).
    /// Empty on records written before the statistics footer existed.
    #[serde(default)]
    pub agent_stats: Vec<AgentStat>,
    /// Wall-clock duration of the whole run in milliseconds (request received
    /// → record finalized), including retrieval/indexing time between calls.
    /// 0 when unknown (older records).
    #[serde(default)]
    pub total_elapsed_ms: u64,
    /// The largest single-call prompt (input-token) size observed — the peak
    /// context any one request actually reached. 0 when unknown.
    #[serde(default)]
    pub peak_context_tokens: u64,
}

/// One agent's accumulated usage across a workflow run: how many model calls
/// it made, the exact token counts those calls reported, and the wall-clock
/// time spent waiting on them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStat {
    pub agent: String,
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub elapsed_ms: u64,
}

/// The reviewer name recorded for a machine-validation rejection (the lint
/// gate below): the IDE's deterministic change-set validator, not a Pedantic
/// model. Kept distinct so audit records and progress lines never read as if
/// an LLM reviewer produced the verdict.
pub const MACHINE_VALIDATOR: &str = "change-set validator";

/// Host-supplied transport: invoke one named agent with a system+user prompt
/// and return its full reply. The host resolves models, keys, endpoints.
///
/// The `extract_*` hooks obtain the *typed* structures the engine needs from a
/// reply (Rig migration, phase 3). The defaults are the deterministic
/// fenced-JSON parsers — sufficient for mocks and well-behaved replies. A real
/// transport overrides them to fall back to provider-native typed extraction
/// when the deterministic parse fails, which is what deleted the old
/// "malformed plan, resend everything" correction roundtrips.
pub trait AgentInvoker {
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String>;

    /// Deterministic machine validation of a specialist submission, run by the
    /// engine BEFORE the Pedantic review gate. `Some(errors)` means the
    /// submission carries defects a validator can *prove* — e.g. change-set
    /// operations the IDE would reject and silently skip at apply time — and
    /// the engine charges the specialist a bounded correction round carrying
    /// those exact errors, like a Pedantic rejection but without spending a
    /// review call. `None` means nothing machine-checkable is wrong (including
    /// "this agent's output is not machine-checkable at all").
    /// `objective` and `dependency_outputs` let a host resolve the resource the
    /// task targets and the approved upstream work, so a deterministic check
    /// can judge the submission against what the change-set will really
    /// produce — not against the submission text alone.
    fn lint_submission(
        &mut self,
        agent: &str,
        submission: &str,
        objective: &str,
        dependency_outputs: &str,
    ) -> Option<String> {
        let _ = (agent, submission, objective, dependency_outputs);
        None
    }

    /// Split a defective submission into the part the host still accepts and
    /// the part the specialist must redo.
    ///
    /// `defect_refs` are the operations a Pedantic reviewer attributed its
    /// findings to; empty means the host should work out the defective
    /// operations itself (the machine validator can — it proved them).
    /// `None` means the submission cannot be scoped, and the engine falls
    /// back to asking for a full replacement.
    ///
    /// The point is not to save tokens. A specialist told to resubmit
    /// everything rewrites operations nobody complained about — observed
    /// live: three malformed handlers were flagged and the model silently
    /// reimplemented a fourth, correct one on the way past.
    fn scope_correction(
        &mut self,
        agent: &str,
        submission: &str,
        defect_refs: &[String],
    ) -> Option<CorrectionScope> {
        let _ = (agent, submission, defect_refs);
        None
    }

    /// Merge a scoped correction back onto the operations that were kept.
    /// `None` means the merge failed and the reply must stand on its own.
    fn merge_correction(&mut self, agent: &str, accepted: &str, corrected: &str) -> Option<String> {
        let _ = (agent, accepted, corrected);
        None
    }

    /// Invoke `agent` to EXECUTE a task, saying whether a Pedantic review gate
    /// stands behind the result. Defaults to [`invoke`]: the distinction only
    /// matters to hosts that adjust the call for unreviewed work (the IDE runs
    /// unreviewed tasks at a lower temperature — no correction loop will catch
    /// a hallucination, so determinism outranks variance there).
    ///
    /// [`invoke`]: AgentInvoker::invoke
    fn invoke_task(
        &mut self,
        agent: &str,
        system: &str,
        user: &str,
        reviewed: bool,
    ) -> Result<String, String> {
        let _ = reviewed;
        self.invoke(agent, system, user)
    }

    /// Obtain the typed workflow plan from `agent`'s planning reply.
    fn extract_plan(&mut self, agent: &str, plan_reply: &str) -> Result<WorkflowPlan, String> {
        let _ = agent;
        parse_plan(plan_reply).map(|(workflow_id, tasks)| WorkflowPlan { workflow_id, tasks })
    }

    /// Obtain the typed round verdict from `reviewer`'s review reply.
    fn extract_verdict(
        &mut self,
        reviewer: &str,
        review_reply: &str,
    ) -> Result<ReviewVerdict, String> {
        let _ = reviewer;
        parse_verdict(review_reply)
    }
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
/// A final fence the model never closed still counts — the remainder of the
/// reply is taken as the block body (observed live: a plan ending in
/// `…}]}` with no closing ```).
pub fn last_json_block(reply: &str) -> Option<serde_json::Value> {
    // Fences are recognised only where Markdown puts them: at the start of a
    // line, after optional indentation. Scanning for a bare "```" anywhere in
    // the text used to close the block on the first triple-backtick INSIDE a
    // JSON string — Grace writing an objective like `…num bloco ```cobol```.`
    // truncated her own plan mid-string, and the whole reply then read as
    // "contained no JSON block".
    let mut last = None;
    let mut lines = reply.lines();
    while let Some(open) = lines.next() {
        let Some(info) = fence_info(open) else {
            continue;
        };
        let mut block = String::new();
        for line in lines.by_ref() {
            if fence_info(line).is_some() {
                break;
            }
            block.push_str(line);
            block.push('\n');
        }
        // An info string of `json` (or none at all) is the contract's shape;
        // a `cobol` fence is source, never a plan.
        if !info.is_empty() && !info.eq_ignore_ascii_case("json") {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(block.trim()) {
            last = Some(v);
        }
    }
    last
}

/// The info string of a Markdown fence line (`"json"`, `"cobol"`, `""`), or
/// `None` when the line does not open or close a fence.
fn fence_info(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix("```").map(str::trim)
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

/// Deterministically parse a Pedantic review reply's round-verdict JSON (the
/// LAST fenced block carrying `pedantic_verdict`).
pub fn parse_verdict(reply: &str) -> Result<ReviewVerdict, String> {
    let v = last_json_block(reply).ok_or("the review contained no verdict JSON block")?;
    let verdict = v
        .get("pedantic_verdict")
        .and_then(|x| x.as_str())
        .ok_or("the review's JSON block carried no pedantic_verdict")?
        .to_string();
    let correction_request = v
        .get("correction_request")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let defective_ops = v
        .get("defective_ops")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Ok(ReviewVerdict {
        pedantic_verdict: verdict,
        correction_request,
        defective_ops,
    })
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
        let reviewed = spec.reviewer.is_some();
        let mut submission = match invoker.invoke_task(
            &spec.agent,
            &system_for(&spec.agent),
            &user,
            reviewed,
        ) {
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

        // Machine-validation gate (deterministic, no model call): a submission
        // whose change-set the IDE would reject gets a bounded correction round
        // with the exact validator errors — BEFORE any Pedantic round is spent
        // on it. Observed live: a "wire the events" task returned 60 placeholder
        // handlers without the three division headers; every operation was
        // silently skipped at apply time and nothing was created, with no one
        // told why. The gate shares one bounded budget across the whole task.
        let mut lint_rounds = 0usize;
        submission = match self.lint_gate(
            rec,
            dependency_outputs,
            submission,
            invoker,
            system_for,
            on_event,
            &mut lint_rounds,
        ) {
            Some(clean) => clean,
            None => return, // failed in the gate; already recorded
        };

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
                    // The reviewer cannot RUN (not configured, disabled,
                    // unreachable). That is the reviewer's absence, not a
                    // defect in the specialist's finished work — discarding
                    // the submission here threw away a complete, possibly
                    // correct result the developer never got to see. Approve
                    // it UNREVIEWED, with the reason on the record, and let
                    // the developer judge the outcome. A review that actually
                    // RAN and rejected still fails the task below.
                    rec.states.push(TaskState::Approved);
                    rec.final_state = TaskState::Approved;
                    rec.failure_reason = format!("approved WITHOUT review — reviewer unavailable: {e}");
                    on_event(GraceEvent::Approved {
                        id: spec.id.clone(),
                    });
                    return;
                }
            };
            // Typed verdict (phase 3). A malformed verdict used to be silently
            // read as "defects", charging the SPECIALIST a correction round for
            // the reviewer's formatting sin; now the transport extracts the
            // typed verdict (with provider-native recovery), and only a verdict
            // that cannot be obtained at all fails the task honestly.
            let verdict = match invoker.extract_verdict(&reviewer, &review) {
                Ok(v) => v,
                Err(e) => {
                    rec.states.push(TaskState::Failed);
                    rec.final_state = TaskState::Failed;
                    rec.failure_reason = format!("review verdict could not be extracted: {e}");
                    on_event(GraceEvent::Failed {
                        id: spec.id.clone(),
                        reason: rec.failure_reason.clone(),
                    });
                    return;
                }
            };
            let defects = verdict.has_defects();
            let correction = verdict.correction_request.clone();
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
            let req = if correction.trim().is_empty() {
                review.as_str()
            } else {
                correction.as_str()
            };
            // When the reviewer attributed its findings to operations, only
            // those come back: the rest is kept exactly as approved.
            let scope =
                invoker.scope_correction(&spec.agent, &submission, &verdict.defective_ops);
            let fix_user = match &scope {
                Some(s) => format!(
                    "TASK {id}: {obj}\n\n=== APPROVED DEPENDENCY OUTPUTS ===\n{dependency_outputs}\n\n=== ALREADY ACCEPTED — DO NOT RESUBMIT ===\n{accepted_count} operation(s) of your previous change-set passed review and are kept exactly as they are. Do not repeat them, do not restate them, do not rewrite them.\n\n=== THE {defective_count} OPERATION(S) TO CORRECT ===\n{defective}\n\n=== PEDANTIC CORRECTION REQUEST ===\n{req}\n\nReturn ONLY the corrected operation(s) in your change-set block — the complete body of each one, but nothing else. They are merged back onto the accepted work.",
                    id = spec.id,
                    obj = spec.objective,
                    accepted_count = s.accepted_count,
                    defective_count = s.defective_count,
                    defective = s.defective,
                ),
                None => format!(
                    "TASK {id}: {obj}\n\n=== APPROVED DEPENDENCY OUTPUTS ===\n{dependency_outputs}\n\n=== YOUR PREVIOUS COMPLETE RESPONSE ===\n{submission}\n\n=== PEDANTIC CORRECTION REQUEST ===\n{req}\n\nCorrect the defects and submit the COMPLETE result again — a full replacement, not isolated patches.",
                    id = spec.id,
                    obj = spec.objective,
                ),
            };
            let reply = match invoker.invoke_task(
                &spec.agent,
                &system_for(&spec.agent),
                &fix_user,
                reviewed,
            ) {
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
            // Splice the corrected operations back onto the kept ones. A
            // specialist that ignored the instruction and resubmitted
            // everything is handled too: a corrected operation supersedes the
            // accepted one it targets.
            submission = match &scope {
                Some(s) => invoker
                    .merge_correction(&spec.agent, &s.accepted, &reply)
                    .unwrap_or(reply),
                None => reply,
            };
            rec.submissions.push(submission.clone());
            rec.states.push(TaskState::Revalidating);
            on_event(GraceEvent::Submitted {
                id: spec.id.clone(),
                agent: spec.agent.clone(),
            });
            // A Pedantic correction can reintroduce a machine-provable defect
            // (e.g. a resubmitted change-set whose handler bodies lost their
            // division headers) — re-run the gate before burning the next
            // review round on it. The lint budget carries over from above.
            submission = match self.lint_gate(
                rec,
                dependency_outputs,
                submission,
                invoker,
                system_for,
                on_event,
                &mut lint_rounds,
            ) {
                Some(clean) => clean,
                None => return, // failed in the gate; already recorded
            };
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

    /// The machine-validation gate: loop while [`AgentInvoker::lint_submission`]
    /// proves defects in `submission`, charging the specialist one correction
    /// round per iteration with the validator's errors verbatim. Rejections are
    /// recorded as [`ReviewRound`]s under the [`MACHINE_VALIDATOR`] name so the
    /// audit trail shows exactly why a submission was sent back.
    ///
    /// `rounds_used` is the gate's shared budget for the WHOLE task (bounded by
    /// `max_revisions`, like the Pedantic loop): the gate runs again after every
    /// Pedantic correction, and without a shared budget a specialist that keeps
    /// resubmitting invalid operations would loop forever.
    ///
    /// Returns the first submission the validator accepts, or `None` when the
    /// task failed here — already recorded on `rec` and streamed to `on_event`.
    #[allow(clippy::too_many_arguments)]
    fn lint_gate(
        &self,
        rec: &mut TaskRecord,
        dependency_outputs: &str,
        mut submission: String,
        invoker: &mut dyn AgentInvoker,
        system_for: &dyn Fn(&str) -> String,
        on_event: &mut dyn FnMut(GraceEvent),
        rounds_used: &mut usize,
    ) -> Option<String> {
        let spec = rec.spec.clone();
        let reviewed = spec.reviewer.is_some();
        loop {
            let Some(errors) = invoker.lint_submission(
                &spec.agent,
                &submission,
                &spec.objective,
                dependency_outputs,
            ) else {
                return Some(submission);
            };
            rec.reviews.push(ReviewRound {
                reviewer: MACHINE_VALIDATOR.to_string(),
                defects: true,
                correction_request: errors.clone(),
                raw: errors.clone(),
            });
            on_event(GraceEvent::Verdict {
                id: spec.id.clone(),
                reviewer: MACHINE_VALIDATOR.to_string(),
                approved: false,
            });
            if *rounds_used >= self.max_revisions {
                rec.states.push(TaskState::Failed);
                rec.final_state = TaskState::Failed;
                rec.failure_reason = format!(
                    "change-set still fails machine validation after {} correction round(s): {errors}",
                    *rounds_used
                );
                on_event(GraceEvent::Failed {
                    id: spec.id.clone(),
                    reason: rec.failure_reason.clone(),
                });
                return None;
            }
            *rounds_used += 1;
            rec.states.push(TaskState::CorrectionRequired);
            on_event(GraceEvent::CorrectionRequested {
                id: spec.id.clone(),
                round: *rounds_used,
            });
            // The validator PROVED which operations are bad, so only those go
            // back; everything it did not flag is kept exactly as submitted.
            let scope = invoker.scope_correction(&spec.agent, &submission, &[]);
            let fix_user = match &scope {
                Some(s) => format!(
                    "TASK {id}: {obj}\n\n=== APPROVED DEPENDENCY OUTPUTS ===\n{dependency_outputs}\n\n=== ALREADY ACCEPTED — DO NOT RESUBMIT ===\n{accepted_count} operation(s) of your change-set passed machine validation and are kept exactly as they are. Do not repeat them, do not restate them, do not rewrite them.\n\n=== THE {defective_count} OPERATION(S) TO CORRECT ===\n{defective}\n\n=== MACHINE VALIDATION — THE IDE REJECTED THESE OPERATIONS ===\nThe IDE's change-set validator proved the following defects. These operations would be discarded at apply time, creating NOTHING:\n{errors}\n\nReturn ONLY the corrected operation(s) in your change-set block — the complete body of each one, but nothing else. They are merged back onto the accepted work.",
                    id = spec.id,
                    obj = spec.objective,
                    accepted_count = s.accepted_count,
                    defective_count = s.defective_count,
                    defective = s.defective,
                ),
                None => format!(
                    "TASK {id}: {obj}\n\n=== APPROVED DEPENDENCY OUTPUTS ===\n{dependency_outputs}\n\n=== YOUR PREVIOUS COMPLETE RESPONSE ===\n{submission}\n\n=== MACHINE VALIDATION — THE IDE REJECTED OPERATIONS IN YOUR CHANGE-SET ===\nThe IDE's change-set validator proved the following defects. These operations would be discarded at apply time, creating NOTHING:\n{errors}\n\nCorrect the defects and submit the COMPLETE result again — a full replacement, not isolated patches.",
                    id = spec.id,
                    obj = spec.objective,
                ),
            };
            let reply = match invoker.invoke_task(
                &spec.agent,
                &system_for(&spec.agent),
                &fix_user,
                reviewed,
            ) {
                Ok(s) if !s.trim().is_empty() => s,
                Ok(_) | Err(_) => {
                    rec.states.push(TaskState::Failed);
                    rec.final_state = TaskState::Failed;
                    rec.failure_reason =
                        "machine-validation correction round returned no result".into();
                    on_event(GraceEvent::Failed {
                        id: spec.id.clone(),
                        reason: rec.failure_reason.clone(),
                    });
                    return None;
                }
            };
            submission = match &scope {
                Some(s) => invoker
                    .merge_correction(&spec.agent, &s.accepted, &reply)
                    .unwrap_or(reply),
                None => reply,
            };
            rec.submissions.push(submission.clone());
            rec.states.push(TaskState::Revalidating);
            on_event(GraceEvent::Submitted {
                id: spec.id.clone(),
                agent: spec.agent.clone(),
            });
        }
    }
}

#[cfg(test)]
mod fence_tests {
    use super::*;

    /// Grace routinely tells a specialist to "return the code in a ```cobol
    /// block", and that inner fence sits INSIDE a JSON string. Scanning for a
    /// bare triple-backtick closed the block there, so a perfectly valid plan
    /// was reported as "contained no JSON block" and the workflow died in the
    /// extraction fallback with `missing field workflow_id`.
    #[test]
    fn a_fence_inside_a_json_string_does_not_truncate_the_plan() {
        let reply = "Vou encaminhar a correção.\n\n\
             ```json\n\
             {\"workflow_id\": \"wf-1\", \"tasks\": [{\"id\": \"T1\", \"agent\": \"COBOL Event Handler Script Agent\", \"objective\": \"Devolver o código num bloco ```cobol```. Nada depois.\", \"depends_on\": [], \"reviewer\": null, \"acceptance\": \"ok\"}]}\n\
             ```\n";
        let (workflow, tasks) = parse_plan(reply).expect("the plan must survive the inner fence");
        assert_eq!(workflow, "wf-1");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "T1");
    }

    /// The last fenced JSON block still wins when several are present.
    #[test]
    fn the_last_json_block_is_the_one_that_counts() {
        let reply = "```json\n{\"pedantic_verdict\": \"defects\"}\n```\n\
             later\n\
             ```json\n{\"pedantic_verdict\": \"acceptable\"}\n```\n";
        let verdict = parse_verdict(reply).expect("verdict");
        assert!(!verdict.has_defects());
    }

    /// A `cobol` fence is source, never a plan — it must not be parsed as one
    /// even in the unlikely case its body happens to be valid JSON.
    #[test]
    fn a_cobol_fence_is_never_read_as_a_plan() {
        let reply = "```cobol\n{\"workflow_id\": \"nope\"}\n```\n";
        assert!(last_json_block(reply).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 036 T1: a pre-1.38.0 record JSON (no `action_log`) still
    /// deserializes — the log defaults empty — and re-serializes cleanly.
    #[test]
    fn old_record_without_action_log_round_trips() {
        let old = r#"{
            "workflow_id": "wf-legacy",
            "tasks": [],
            "status": "completed",
            "input_tokens": 10,
            "output_tokens": 5
        }"#;
        let record: WorkflowRecord = serde_json::from_str(old).expect("old JSON must parse");
        assert_eq!(record.action_log.len(), 0, "absent field defaults empty");
        let json = serde_json::to_string(&record).expect("re-serialize");
        let again: WorkflowRecord = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(again.workflow_id, "wf-legacy");
        assert_eq!(again.action_log.len(), 0);
        println!(
            "action_log round-trip (legacy): read {} entries, wrote {} entries",
            record.action_log.len(),
            again.action_log.len()
        );
    }

    /// Spec 036 T1: a populated action log survives a lossless round-trip.
    #[test]
    fn populated_action_log_round_trips_losslessly() {
        let mut record = WorkflowRecord {
            workflow_id: "wf-036".into(),
            status: "completed".into(),
            ..Default::default()
        };
        record.action_log = vec![
            ActionLogEntry {
                agent: "Grace".into(),
                kind: "planning".into(),
                detail: String::new(),
                at: 1_722_000_000_000,
            },
            ActionLogEntry {
                agent: "Form Designer Agent".into(),
                kind: "drafting".into(),
                detail: "T1".into(),
                at: 1_722_000_001_000,
            },
        ];
        let json = serde_json::to_string_pretty(&record).expect("serialize");
        let again: WorkflowRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(again.action_log, record.action_log, "lossless round-trip");
        println!(
            "action_log round-trip (populated): wrote {} entries, read {} entries",
            record.action_log.len(),
            again.action_log.len()
        );
    }

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

    /// Operator rule (2026-07-30): a reviewer that attributes its findings to
    /// operations must have the rest KEPT — the specialist is shown only the
    /// defective operations and never gets the chance to rewrite the approved
    /// ones, and the correction is merged back onto them.
    #[test]
    fn an_attributed_review_scopes_the_correction_and_keeps_the_rest() {
        struct ScopingMock {
            calls: Vec<(String, String)>,
            script: Vec<(&'static str, &'static str)>,
            scoped: bool,
            merged: bool,
        }
        impl AgentInvoker for ScopingMock {
            fn invoke(&mut self, agent: &str, _s: &str, user: &str) -> Result<String, String> {
                self.calls.push((agent.to_string(), user.to_string()));
                let i = self.calls.len() - 1;
                let (expect, reply) = self.script[i];
                assert_eq!(agent, expect, "call #{i} routed to the wrong agent");
                Ok(reply.to_string())
            }
            fn scope_correction(
                &mut self,
                _agent: &str,
                _submission: &str,
                defect_refs: &[String],
            ) -> Option<CorrectionScope> {
                assert_eq!(defect_refs, ["generate_event_handler A2.onClick"]);
                self.scoped = true;
                Some(CorrectionScope {
                    accepted: "{\"operations\":[A1,A3]}".into(),
                    defective: "{\"operations\":[A2]}".into(),
                    accepted_count: 2,
                    defective_count: 1,
                })
            }
            fn merge_correction(
                &mut self,
                _agent: &str,
                accepted: &str,
                corrected: &str,
            ) -> Option<String> {
                self.merged = true;
                Some(format!("MERGED[{accepted}|{corrected}]"))
            }
        }

        let mut mock = ScopingMock {
            calls: Vec::new(),
            scoped: false,
            merged: false,
            script: vec![
                ("Form Designer Agent", "three handlers"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "A2 is a stub.\n```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. A2 has no body\", \"defective_ops\": [\"generate_event_handler A2.onClick\"]}\n```",
                ),
                ("Form Designer Agent", "only A2, fixed"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "clean.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
            ],
        };
        let plan = vec![TaskSpec {
            id: "T1".into(),
            agent: "Form Designer Agent".into(),
            objective: "three handlers".into(),
            context: String::new(),
            reviewer: Some("Form Designer Agent Pedantic Reviewer".into()),
            depends_on: vec![],
            acceptance: "all three exist".into(),
        }];
        let rec = GraceEngine::default().run("wf-scope", &plan, &mut mock, &|_| "sys".into());

        assert!(mock.scoped, "the review's attribution must scope the fix");
        assert!(mock.merged, "the fix must be merged onto the kept work");
        let fix_prompt = &mock.calls[2].1;
        assert!(
            fix_prompt.contains("ALREADY ACCEPTED — DO NOT RESUBMIT")
                && fix_prompt.contains("2 operation(s)"),
            "the specialist must be told what is kept: {fix_prompt}"
        );
        assert!(
            fix_prompt.contains("{\"operations\":[A2]}")
                && !fix_prompt.contains("{\"operations\":[A1,A3]}"),
            "only the defective operation may be shown: {fix_prompt}"
        );
        assert!(
            !fix_prompt.contains("a full replacement"),
            "a scoped correction must not ask for a full replacement"
        );
        assert_eq!(rec.tasks[0].final_state, TaskState::Approved);
        assert!(
            rec.tasks[0].submissions.last().unwrap().starts_with("MERGED["),
            "the approved submission is the merged one"
        );
        println!("scoped correction: 1 defective operation resent, 2 kept, merged and approved");
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

    /// Scripted mock whose lint gate rejects any submission containing
    /// "PLACEHOLDER" — the machine-validation analogue of the IDE's
    /// change-set validator.
    struct LintMock {
        calls: Vec<(String, String)>,
        script: Vec<(&'static str, &'static str)>,
    }
    impl AgentInvoker for LintMock {
        fn invoke(&mut self, agent: &str, _s: &str, user: &str) -> Result<String, String> {
            self.calls.push((agent.to_string(), user.to_string()));
            let i = self.calls.len() - 1;
            let (expect, reply) = self.script[i];
            assert_eq!(agent, expect, "call #{i} routed to the wrong agent");
            Ok(reply.to_string())
        }
        fn lint_submission(
            &mut self,
            _agent: &str,
            submission: &str,
            _objective: &str,
            _dependency_outputs: &str,
        ) -> Option<String> {
            submission.contains("PLACEHOLDER").then(|| {
                "1. generate_event_handler LBL-01.onHoverEnter: Code must start from the \
                 nested-program body and include ENVIRONMENT DIVISION."
                    .to_string()
            })
        }
    }

    fn lint_plan(reviewer: Option<&str>) -> Vec<TaskSpec> {
        vec![TaskSpec {
            id: "T1".into(),
            agent: "Form Designer Agent".into(),
            objective: "Wire hover events".into(),
            context: "form LABELS-FORM".into(),
            reviewer: reviewer.map(str::to_owned),
            depends_on: vec![],
            acceptance: "handlers exist".into(),
        }]
    }

    /// A submission the IDE's validator would reject at apply time is sent
    /// back to the specialist with the exact errors — BEFORE any Pedantic
    /// round — and the corrected resubmission proceeds through review.
    /// (Observed live: 60 placeholder handlers were silently skipped at apply
    /// time and the workflow reported success with nothing created.)
    #[test]
    fn machine_validation_returns_the_errors_and_recovers() {
        let mut mock = LintMock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "ops v1 PLACEHOLDER"),
                ("Form Designer Agent", "ops v2 full bodies"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "clean.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
            ],
        };
        let rec = GraceEngine::default().run(
            "wf-lint",
            &lint_plan(Some("Form Designer Agent Pedantic Reviewer")),
            &mut mock,
            &|_| "sys".into(),
        );
        assert_eq!(rec.status, "completed");
        let t1 = &rec.tasks[0];
        assert_eq!(t1.final_state, TaskState::Approved);
        assert!(t1.states.contains(&TaskState::CorrectionRequired));
        assert_eq!(t1.submissions.len(), 2, "both submissions preserved");
        // The rejection is on the audit record under the validator's name,
        // never attributed to the Pedantic reviewer.
        assert_eq!(t1.reviews[0].reviewer, MACHINE_VALIDATOR);
        assert!(t1.reviews[0].defects);
        // The correction round carried the validator's errors verbatim.
        assert!(mock.calls[1].1.contains("MACHINE VALIDATION"));
        assert!(mock.calls[1].1.contains("ENVIRONMENT DIVISION"));
        // The Pedantic reviewer saw only the corrected submission.
        assert!(mock.calls[2].1.contains("ops v2 full bodies"));
    }

    /// The gate also guards UNREVIEWED tasks (no Pedantic companion): they
    /// used to be auto-approved with whatever they returned.
    #[test]
    fn machine_validation_guards_unreviewed_tasks_too() {
        let mut mock = LintMock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "ops v1 PLACEHOLDER"),
                ("Form Designer Agent", "ops v2 full bodies"),
            ],
        };
        let rec =
            GraceEngine::default().run("wf-lint2", &lint_plan(None), &mut mock, &|_| "sys".into());
        assert_eq!(rec.status, "completed");
        assert_eq!(rec.tasks[0].final_state, TaskState::Approved);
        assert_eq!(mock.calls.len(), 2, "no reviewer call for an unreviewed task");
    }

    /// The gate's budget is bounded: a specialist that keeps resubmitting
    /// invalid operations fails honestly instead of looping or being approved.
    #[test]
    fn machine_validation_budget_is_bounded() {
        let mut mock = LintMock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "ops v1 PLACEHOLDER"),
                ("Form Designer Agent", "ops v2 PLACEHOLDER"),
            ],
        };
        let rec = GraceEngine { max_revisions: 1 }.run(
            "wf-lint3",
            &lint_plan(Some("Form Designer Agent Pedantic Reviewer")),
            &mut mock,
            &|_| "sys".into(),
        );
        assert_eq!(rec.status, "failed");
        let t1 = &rec.tasks[0];
        assert_eq!(t1.final_state, TaskState::Failed);
        assert!(
            t1.failure_reason.contains("machine validation"),
            "failure names the gate: {}",
            t1.failure_reason
        );
        assert_eq!(mock.calls.len(), 2, "budget spent, no further rounds");
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

    /// Phase 3: a review whose verdict cannot be extracted at all fails the
    /// task honestly — it no longer reads as "defects" and silently charges
    /// the specialist a correction roundtrip for the reviewer's formatting.
    #[test]
    fn unextractable_verdict_fails_the_task() {
        let mut mock = Mock {
            calls: Vec::new(),
            script: vec![
                ("Form Designer Agent", "v1"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "looks fine to me, approved!",
                ),
            ],
        };
        let rec = GraceEngine::default().run("wf-v", &plan2()[..1], &mut mock, &|_| "s".into());
        assert_eq!(rec.tasks[0].final_state, TaskState::Failed);
        assert!(rec.tasks[0].failure_reason.contains("verdict"));
        assert_eq!(mock.calls.len(), 2, "no correction roundtrip was burned");
    }

    /// Observed live: a valid plan whose final fence the model never closed.
    /// The unterminated block still parses deterministically.
    #[test]
    fn unterminated_final_fence_still_parses() {
        let reply = "I need two decisions first.\n\n```json\n{\"workflow_id\": \"w9\", \"tasks\": [{\"id\": \"T1\", \"agent\": \"Documentation Agent\", \"objective\": \"clarify\", \"reviewer\": \"Documentation Agent Pedantic Reviewer\", \"depends_on\": [], \"acceptance\": \"a\"}]}";
        let (wf, tasks) = parse_plan(reply).expect("unterminated fence parses");
        assert_eq!(wf, "w9");
        assert_eq!(tasks.len(), 1);
        // A closed fence still parses exactly as before.
        let closed = format!("{reply}\n```");
        assert!(parse_plan(&closed).is_ok());
        // Prose with a dangling fence and no JSON stays a parse failure.
        assert!(parse_plan("thoughts…\n```json\nnot json at all").is_err());
    }

    /// The deterministic verdict parser handles the tooling contract's shapes.
    #[test]
    fn verdict_parsing_contract() {
        let v = parse_verdict("ok.\n```json\n{\"pedantic_verdict\": \"Acceptable\"}\n```").unwrap();
        assert!(!v.has_defects());
        let v = parse_verdict(
            "```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. x\"}\n```",
        )
        .unwrap();
        assert!(v.has_defects());
        assert_eq!(v.correction_request, "1. x");
        assert!(parse_verdict("no json at all").is_err());
        assert!(parse_verdict("```json\n{\"other\": 1}\n```").is_err());
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
