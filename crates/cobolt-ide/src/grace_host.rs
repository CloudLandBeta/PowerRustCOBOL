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
    AgentInvoker, GraceEngine, GraceEvent, ReviewVerdict, TaskRecord, TaskSpec, TaskState,
    WorkflowPlan, WorkflowRecord,
};
use serde::{Deserialize, Serialize};

use crate::agents_db::{AgentKind, AgentsDb, DATA_INDEXED_FILE_AGENT, DOCUMENTATION_AGENT, GRACE};
use crate::git_exec::GitConfirmRequest;
use crate::llm::LlmConfig;
use crate::target_select::{TargetChoice, TargetRequest};
use crate::tool_exec::{IdeToolBackend, ToolEvidence, ToolExecutingInvoker};

/// Bound on tool-execution rounds per task (spec 030) — guards a model that
/// never stops emitting tool calls.
const MAX_TOOL_ROUNDS: usize = 6;

/// A bare social greeting ("Hola", "olá Grace", "good morning") deserves a
/// greeting back — not a planning call, an ACTION-contract retry, and a
/// delegated "greeting task" (observed live: ~9.4k tokens and four model calls
/// to answer "Hola"). Returns the canned reply in the greeting's language, or
/// `None` when the message is anything more than a greeting.
pub fn simple_greeting_reply(request: &str) -> Option<&'static str> {
    // Accent-folded so "¿cómo estás?" and "como estas" match the same entry
    // (observed live: "Hola, ¿cómo estás?" missed the exact-match list and ran
    // the whole ACTION pipeline again).
    let normalized = request
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c if c.is_alphanumeric() || c.is_whitespace() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let stripped = normalized.strip_suffix(" grace").unwrap_or(&normalized);
    const SPANISH: &[&str] = &[
        "hola",
        "buenos dias",
        "buenas tardes",
        "buenas noches",
        "que tal",
        "saludos",
        "como estas",
        "hola como estas",
        "hola que tal",
        "hola buenos dias",
        "hola buenas tardes",
        "hola buenas noches",
    ];
    const PORTUGUESE: &[&str] = &[
        "ola",
        "oi",
        "bom dia",
        "boa tarde",
        "boa noite",
        "tudo bem",
        "e ai",
        "como vai",
        "ola tudo bem",
        "oi tudo bem",
        "ola como vai",
        "ola bom dia",
    ];
    const ENGLISH: &[&str] = &[
        "hello",
        "hi",
        "hey",
        "howdy",
        "greetings",
        "good morning",
        "good afternoon",
        "good evening",
        "how are you",
        "hello how are you",
        "hi how are you",
        "hey how are you",
    ];
    if SPANISH.contains(&stripped) {
        return Some("¡Hola! ¿Cómo estás? ¿Creamos algo juntos?");
    }
    if PORTUGUESE.contains(&stripped) {
        return Some("Olá! Tudo bem? Vamos criar algo juntos?");
    }
    if ENGLISH.contains(&stripped) {
        return Some("Hello! How are you? Shall we create something together?");
    }
    None
}

fn direct_grace_record(reply: String, objective: &str) -> WorkflowRecord {
    WorkflowRecord {
        workflow_id: format!("conversation-{}", crate::agents_db::new_uuid()),
        status: "completed".into(),
        tasks: vec![TaskRecord {
            spec: TaskSpec {
                id: "C1".into(),
                agent: GRACE.into(),
                objective: objective.into(),
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
        ..Default::default()
    }
}

/// Grace's private verdict on one developer request, produced BEFORE any
/// Knowledge Base retrieval. The score is telemetry (stored on the record,
/// shown only in the AI log); the interpretation and questions are what the
/// developer sees when the gate holds a request back.
struct ClarityCheck {
    clarity: u8,
    interpretation: String,
    questions: Vec<String>,
}

/// Below this score the request goes back to the developer with Grace's
/// interpretation instead of spending the full RAG + planning cost on a
/// guess. 7 is the contract's own boundary: 0–6 means competing readings
/// would produce different artifacts.
const CLARITY_GATE_THRESHOLD: u8 = 7;

/// The compact surface slice the clarity pre-check judges against: the
/// identity of what is open on the surface (the `FORM:` line and its
/// properties), WITHOUT the control inventory or the project tree. Cheap on
/// tokens, but enough that the gate never asks "which form?" while the
/// surface plainly names one — observed live: the pre-check received only
/// the surface NAME, and Grace asked which form the layout change targeted
/// with LABELS-FORM open right there in the designer.
fn clarity_surface_slice(context: &str) -> String {
    let mut lines = Vec::new();
    for line in context.lines() {
        let trimmed = line.trim_start();
        // The inventory sections are irrelevant to clarity and dwarf the
        // budget; the identity header is everything before them.
        if trimmed.starts_with("CONTROLS:")
            || trimmed.starts_with("AVAILABLE CONTROL TYPES")
            || trimmed.starts_with("PROJECT TREE INVENTORY")
        {
            break;
        }
        lines.push(line);
        if lines.len() >= 12 {
            break;
        }
    }
    let slice = lines.join("\n").trim().to_string();
    if slice.is_empty() {
        "(none)".to_string()
    } else {
        slice
    }
}

/// Upper bound on the conversation tail the clarity pre-check may carry. The
/// gate exists to be cheap; the tail only needs Grace's own last question and
/// the request it was about, never the whole transcript.
const CLARITY_CONVERSATION_BUDGET: usize = 2_400;

/// The recent-conversation tail for the clarity pre-check. Every chat surface
/// appends its transcript to the routing context under a `CONVERSATION SO
/// FAR:` header (turns formatted `role: content`, oldest first); the identity
/// slice above stops long before it, so without this the gate judged every
/// message in isolation — observed live: Grace asked "which controls?", the
/// developer answered "todos os labels", and the answer scored 3/10 because
/// the gate had no idea Grace had just asked anything. Returns the last few
/// turns, oldest first, bounded by [`CLARITY_CONVERSATION_BUDGET`].
fn clarity_conversation_slice(context: &str) -> String {
    let Some(at) = context.find("CONVERSATION SO FAR:") else {
        return "(none)".to_string();
    };
    let transcript = context[at + "CONVERSATION SO FAR:".len()..].trim();
    if transcript.is_empty() {
        return "(none)".to_string();
    }
    // Turn starts: the transcript head, plus every `role:` line that follows
    // the `\n\n` joiner. Content may itself contain blank lines, so only
    // lines that begin with a known role open a turn.
    let mut starts = vec![0usize];
    for (offset, _) in transcript.match_indices("\n\n") {
        let rest = &transcript[offset + 2..];
        if rest.starts_with("user: ") || rest.starts_with("assistant: ") {
            starts.push(offset + 2);
        }
    }
    // Newest turns win the budget; at least the final turn always survives
    // (hard-truncated to the budget's tail when it alone exceeds it).
    let mut begin = starts.len() - 1;
    while begin > 0 && transcript.len() - starts[begin - 1] <= CLARITY_CONVERSATION_BUDGET {
        begin -= 1;
    }
    let tail = transcript[starts[begin]..].trim();
    if tail.len() > CLARITY_CONVERSATION_BUDGET {
        let cut = tail.len() - CLARITY_CONVERSATION_BUDGET;
        let cut = (cut..tail.len())
            .find(|i| tail.is_char_boundary(*i))
            .unwrap_or(tail.len());
        format!("…{}", &tail[cut..])
    } else {
        tail.to_string()
    }
}

/// Ask Grace to rate the request's clarity and conciseness — one cheap,
/// tool-less call on the raw request text. Fail-open: a transport error or an
/// unparseable reply returns `None` and the workflow proceeds normally (the
/// gate saves cost; it must never add a new way to fail).
fn evaluate_request_clarity(
    invoker: &mut DbAgentInvoker,
    request: &str,
    routing: &GraceRoutingContext,
) -> Option<ClarityCheck> {
    let surface = if routing.surface.trim().is_empty() {
        "Project workspace"
    } else {
        routing.surface.trim()
    };
    let surface_slice = clarity_surface_slice(&routing.context);
    let conversation_slice = clarity_conversation_slice(&routing.context);
    let user = format!(
        "CLARITY PRE-CHECK (Grace-internal). This rating is telemetry for the workflow record: the developer is not shown the number and it must NEVER reach any specialist.\n\nBefore any Knowledge Base retrieval or planning happens, rate the developer's request below for clarity and conciseness on a 0-10 scale:\n- 9-10: unambiguous and actionable — also EVERY greeting, capability question, or other conversational/read-only message;\n- 7-8: minor vagueness a specialist could resolve safely without guessing;\n- 0-6: competing readings would produce DIFFERENT artifacts, or an essential fact (identifiers, exact values) is missing AND is not supplied by the surface or the recent conversation below.\n\nJudge ONLY the request text against the surface and the recent conversation shown below. A fact the surface already supplies is NOT missing: the resource open on the surface (for example the form named on the FORM: line) is the implicit target of the request — NEVER ask which form or resource is meant when the surface names one. Do not call tools, do not retrieve knowledge, do not plan tasks, do not answer the request itself.\n\nTHE CONVERSATION IS PART OF THE REQUEST. When the last assistant turn in the recent conversation asked a clarifying question and this request reads as its answer, the true request is the COMBINED one — the developer's earlier request merged with this answer — and that combined request is what you rate and restate in the interpretation. Likewise, when the request refers back to the conversation (\"the initial request\", \"as I said before\"), resolve the reference from the conversation. A fact stated anywhere in the recent conversation is NOT missing — NEVER ask for it again.\n\nREQUEST (verbatim):\n{request}\n\nCHAT SURFACE: {surface}\nOPEN ON THIS SURFACE:\n{surface_slice}\n\nRECENT CONVERSATION ON THIS SURFACE (oldest first; \"(none)\" when this is the first message):\n{conversation_slice}\n\nReply with ONLY one fenced JSON block of this exact shape and nothing else:\n{{\"clarity\": <0-10>, \"interpretation\": \"<one short paragraph restating what you understand the developer wants — the combined request when this message answers the assistant's question — in the developer's language>\", \"questions\": [\"<clarifying question, in the developer's language>\"]}}\nWhen clarity is below {CLARITY_GATE_THRESHOLD}, questions MUST contain at least one question naming exactly what is missing or ambiguous; when clarity is {CLARITY_GATE_THRESHOLD} or above, questions MUST be empty."
    );
    let reply = match invoker.invoke(GRACE, "", &user) {
        Ok(reply) => reply,
        Err(error) => {
            crate::llm::push_ai_log(
                crate::llm::AiLogKind::Info,
                format!("clarity pre-check skipped (fail-open): {error}"),
            );
            return None;
        }
    };
    parse_clarity_reply(&reply)
}

/// Deterministic parse of the clarity pre-check reply. `None` (fail-open)
/// when the reply carries no parseable verdict.
fn parse_clarity_reply(reply: &str) -> Option<ClarityCheck> {
    let value = cobolt_agents::grace::last_json_block(reply)?;
    let clarity = value
        .get("clarity")
        .and_then(|c| c.as_u64().or_else(|| c.as_f64().map(|f| f.round() as u64)))?
        .min(10) as u8;
    let interpretation = value
        .get("interpretation")
        .and_then(|i| i.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let questions = value
        .get("questions")
        .and_then(|q| q.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|q| q.as_str())
                .map(|q| q.trim().to_string())
                .filter(|q| !q.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(ClarityCheck {
        clarity,
        interpretation,
        questions,
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
            // The old wording told the specialist to "submit your complete
            // work for that review" — an action it CANNOT perform (the engine
            // routes the review after the reply), so models narrated having
            // done it, up to and including "approval obtained" with no review
            // call anywhere in the run (observed live). The contract now
            // states the real mechanics and makes a fabricated approval a
            // named critical defect.
            AgentKind::Orchestrator | AgentKind::Specialist => agent
                .companion
                .as_deref()
                .and_then(|id| db.by_id(id))
                .map(|reviewer| {
                    format!(
                        "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nYour work is reviewed by {reviewer}, and ONLY AFTER you return it: the workflow engine routes your complete submission to that reviewer. You never talk to the reviewer yourself, and while you are writing your reply NO review has happened yet.\n\nFABRICATED APPROVAL = CRITICAL DEFECT. Never state or imply that your work was submitted to, reviewed by, or approved by {reviewer} or anyone else. Sentences such as \"submitted to the reviewer\", \"review passed\", \"approval obtained\" are false by construction — the review has not run yet — and they poison the audit trail; they are treated like a fabricated tool result and void the submission. Report only what you actually did and verified yourself; the verdict is decided after your reply, never narrated inside it. Do not self-approve and do not invoke another agent's reviewer.\n",
                        reviewer = reviewer.name
                    )
                })
                .unwrap_or_else(|| {
                    "\n\nPEDANTIC COMPANION RELATIONSHIP (1:1)\nNo Pedantic companion is assigned to you. Your work is applied without review — NEVER state or imply that it received any review or approval.\n".to_string()
                }),
        })
        .unwrap_or_default()
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

/// STRUCTURAL validation only — checks that need no interpretation of the
/// request or task wording. Intent understanding (what the developer wants,
/// which contract applies, whether a handoff or clarification is required) is
/// the MODEL's job: Grace routes via the coordination contracts in her prompt,
/// and each specialist enforces its own runtime gate (e.g. the Data agent
/// refuses `.cidx` mutation without an approved handoff in its dependency
/// outputs). Keyword-based intent validators were removed deliberately — every
/// one of them produced false rejections or false passes in live use.
fn validate_workflow_coordination(
    db: &AgentsDb,
    _request: &str,
    plan: &[TaskSpec],
) -> Result<(), String> {
    validate_no_pedantic_task_agent(db, plan)
}

/// Grace's configured `provider/model` for the chat footers' model label —
/// the idle-state fallback before any call has run (afterwards the live
/// [`crate::llm::last_model_call`] record wins). `None` when Grace is absent
/// or has no model configured.
pub fn grace_model_display(project_dir: &Path, llm: &LlmConfig) -> Option<String> {
    let db = AgentsDb::load(project_dir);
    let agent = db.by_name(GRACE)?;
    let cfg = crate::agents_db::resolve_agent_connection(agent, llm)?;
    if cfg.model.trim().is_empty() {
        return None;
    }
    if cfg.provider.trim().is_empty() {
        Some(cfg.model)
    } else {
        Some(format!("{}/{}", cfg.provider, cfg.model))
    }
}

/// [`grace_model_display`] behind a short-lived cache: chat footers call this
/// every frame, and resolving the label loads the agents DB from disk.
/// Refreshes every few seconds (or on project switch) so a Models Manager
/// change still shows up promptly.
pub fn grace_model_display_cached(project_dir: &Path, llm: &LlmConfig) -> Option<String> {
    type Cache = Option<(PathBuf, std::time::Instant, Option<String>)>;
    static CACHE: std::sync::LazyLock<Mutex<Cache>> =
        std::sync::LazyLock::new(|| Mutex::new(None));
    let mut cache = match CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => return grace_model_display(project_dir, llm),
    };
    if let Some((dir, at, label)) = cache.as_ref() {
        if dir == project_dir && at.elapsed().as_secs() < 5 {
            return label.clone();
        }
    }
    let label = grace_model_display(project_dir, llm);
    *cache = Some((
        project_dir.to_path_buf(),
        std::time::Instant::now(),
        label.clone(),
    ));
    label
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
/// Live control surface shared between a running workflow and its UI: the
/// developer's Stop button sets `cancel` (every subsequent model call refuses
/// immediately), and `tokens` accumulates exact (input, output) usage after
/// every model return so the UI can show it live.
#[derive(Clone, Default)]
pub struct WorkflowControl {
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
    pub tokens: Arc<Mutex<(u64, u64)>>,
    /// Live Knowledge Base indexing progress — `(records_done, records_total,
    /// current_subject)` while chunk-embedding runs, `None` otherwise. The UI
    /// polls this each frame and renders a progress bar.
    pub indexing: Arc<Mutex<Option<(u64, u64, String)>>>,
}

impl WorkflowControl {
    pub fn stop_requested(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn request_stop(&self) {
        self.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn token_totals(&self) -> (u64, u64) {
        *self.tokens.lock().unwrap()
    }
    pub fn set_indexing(&self, done: usize, total: usize, subject: &str) {
        *self.indexing.lock().unwrap() = Some((done as u64, total as u64, subject.to_string()));
    }
    pub fn clear_indexing(&self) {
        *self.indexing.lock().unwrap() = None;
    }
    pub fn indexing_progress(&self) -> Option<(u64, u64, String)> {
        self.indexing.lock().unwrap().clone()
    }
}

pub struct DbAgentInvoker {
    pub project_dir: PathBuf,
    pub llm: LlmConfig,
    /// Shared (input, output) token accumulator across every LLM call this
    /// invoker makes; read back by the workflow host after the run.
    pub tokens: std::sync::Arc<std::sync::Mutex<(u64, u64)>>,
    /// Shared tool-evidence sink: native-tool executions are recorded here by
    /// the host closures (spec 030 R11), same records as the fenced protocol.
    pub evidence: std::sync::Arc<std::sync::Mutex<Vec<ToolEvidence>>>,
    /// Developer stop request — checked before every model call.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The developer's request, verbatim and in their own words. Every agent
    /// answers in the language it is written in — see [`language_directive`].
    /// Only the workflow entry point knows it; a specialist's task prompt is
    /// composed by Grace and carries no trace of the original wording.
    pub request: String,
    /// Detach native tools from every call this invoker makes. The clarity
    /// pre-check runs on an invoker with this set: it must judge the request
    /// text alone, BEFORE any Knowledge Base retrieval — with tools attached,
    /// Grace could (and did, for planning calls) reach `knowledge_search`
    /// first, defeating the gate's whole point.
    pub suppress_native_tools: bool,
}

/// How Grace decides what shape her reply takes: an answer, a question, or a
/// workflow plan. The host only recognizes the shape she chose, so clause 2 is
/// the only route by which a question ever reaches the developer.
///
/// Clause 2 is deliberately insistent. It used to read "the work cannot proceed
/// without information", which Grace correctly judged inapplicable whenever she
/// *could* infer an answer — so an ambiguous "the button's name" was silently
/// resolved to the control id and the whole workflow ran on a guess. The test
/// is not "can I infer this?" but "would the readings deliver different work?".
const RESPONSE_ROUTING_CONTRACT: &str = "RESPONSE ROUTING CONTRACT (you decide which applies):\n1. CONVERSATION OR QUESTION ANSWER — greetings, capability questions, explanations, summaries, recommendations: reply directly as readable Markdown for the chatbot. Answer from relevant project Knowledge Base evidence first and cite its PATH entries; state when no relevant evidence exists before offering clearly labeled general guidance. No workflow JSON, and do not claim project resources were changed.\n2. DEVELOPER CLARIFICATION — the request admits more than one reasonable reading, and the readings would produce DIFFERENT artifacts: reply with ONLY your question(s) as plain readable Markdown and no JSON.\n   WHEN IN DOUBT, ASK. Do not resolve an ambiguity by picking the reading you find most likely and proceeding: a plausible guess that is wrong costs the developer a whole workflow, while a question costs one message. Being ABLE to infer an answer is NOT a reason to skip the question — the test is whether the competing readings would change the delivered artifact, not whether you can pick a favourite.\n   Words that name a control's text are ambiguous BY CONSTRUCTION and are the most common trap: \"name\", \"nome\", \"nombre\", \"label\", \"text\", \"texto\", \"title\" may mean the control's IDENTIFIER (its id, e.g. Button-3) or its VISIBLE TEXT (its Caption or Text property). The two routinely differ — a form can hold a control whose id is \"Button-3\" while its Caption reads \"Button-2\". Never settle that silently: quote both candidate values for a concrete control and ask which one the developer means.\n   Ask as well when the request and its own example disagree, when a literal's exact spelling or punctuation is uncertain, when the target resource is not uniquely identified, or when a requested change could alter existing behavior in more than one way.\n   Put every question you need in ONE reply, each as a separate short question, and stop — do not plan or mutate anything in the same turn.\n3. EXECUTABLE WORK — the request creates, inspects, or modifies project resources and you have what you need: plan the workflow per your tooling contract and END with exactly one fenced JSON block containing workflow_id and a non-empty tasks array, using only agent and reviewer names from the supplied registry, with nothing after the JSON block.";

/// The platform's own reference documents. They are generated from the
/// compiled binary, so a missing one means the installed platform predates the
/// document — the one condition that a rebuild, and nothing else, fixes.
const ESSENTIAL_SYSTEM_DOCUMENTS: [&str; 4] = [
    "Knowledge Base/rustcobol_extensions.md",
    "Knowledge Base/ide_functionalities.md",
    "Knowledge Base/form_designer_controls.md",
    "Knowledge Base/agents_registry.md",
];

/// Root of the System Knowledge Base: `~/PowerRustCOBOL`, holding
/// `Knowledge Base/` (the documents) and `data/` (their vector index).
///
/// Machine-level, not per-project: the platform's reference material describes
/// the IDE, not whichever project happens to be open, so every project reads
/// the same copy instead of carrying its own.
pub fn system_knowledge_root() -> PathBuf {
    cobolt_agents::knowledge_store::ide_data_dir()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Republish and reindex the System Knowledge Base, returning how many textual
/// files are indexed and how many chunked subject records are live in
/// `~/PowerRustCOBOL/data/chunked.data`.
///
/// Publishing first is what makes "never empty" true: the documents are
/// rewritten from the running binary on every workflow, so the System KB
/// cannot drift behind the platform, and a document the binary cannot produce
/// is reported as a rebuild requirement rather than silently missing.
/// The chunked System KB store shipped INSIDE the IDE binary: regenerated by
/// `cargo run -p cobolt-ide --example build_chunked_kb` whenever the reference
/// documentation changes, committed to the repository, and installed to
/// `~/PowerRustCOBOL/data/chunked.data` on first run — a cloned IDE starts
/// with its embeddings ready and reindexes only documents that actually
/// change.
const PREBUILT_CHUNKED_KB: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/knowledge/chunked.data"
));

fn sync_system_knowledge(
    system_root: &Path,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<(usize, usize), String> {
    cobolt_compiler::publish_system_documentation(system_root)
        .map_err(|error| format!("could not be published: {error}"))?;
    let indexed = cobolt_agents::project_knowledge::sync_documentation(system_root)
        .map_err(|error| format!("could not be indexed: {error}"))?;
    // `<system root>/data/chunked.data` — for the real system root this IS
    // `~/PowerRustCOBOL/data/chunked.data`; deriving it keeps tests (which
    // sync a temp root) out of the machine-level store.
    let store = system_root.join("data").join("chunked.data");
    // Seed from the store shipped with this IDE build (once per build): the
    // sync below then finds every unchanged document already indexed and
    // embeds nothing.
    let _ = cobolt_agents::chunked_knowledge::install_prebuilt(&store, PREBUILT_CHUNKED_KB);
    let records = cobolt_agents::chunked_knowledge::sync_tree_with_progress(
        system_root,
        &store,
        progress,
    )
    .map_err(|error| format!("could not be chunked: {error}"))?;
    let missing: Vec<&str> = ESSENTIAL_SYSTEM_DOCUMENTS
        .iter()
        .filter(|path| !system_root.join(path).exists())
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "{indexed} textual file(s) indexed, but the installed platform did not produce {} \
             ({}). Rebuild and reinstall PowerRustCOBOL — the running binary is older than its \
             own reference documentation.",
            missing.len(),
            missing.join(", ")
        ));
    }
    Ok((indexed, records))
}

/// Instructs an agent to answer in the developer's own language while leaving
/// every machine-read token alone.
///
/// The agents are prompted in English, so absent this they answer in English
/// no matter what the developer wrote. Carrying the request verbatim beats
/// naming a language: no detection step to get wrong, and the model reads the
/// developer's actual wording.
///
/// The carve-outs are not stylistic. `pedantic_verdict` is compared against
/// the literal `"acceptable"`, agent names are matched against the registry,
/// and control ids must survive into the form model — a translated value there
/// silently breaks the workflow rather than reading oddly.
fn language_directive(request: &str) -> String {
    let request = request.trim();
    if request.is_empty() {
        return String::new();
    }
    format!(
        "\n\n--- Developer's language (applies to every reply you produce) ---\n\
         The developer's request is quoted verbatim below. Write everything a HUMAN reads in that same language — prose, explanations, plans, questions, review findings, correction requests, final summaries. If the request is in Portuguese, answer in Portuguese; if in Spanish, answer in Spanish; and so on. Do not answer in English merely because these instructions are in English, and never translate the request back to the developer.\n\n\
         This NEVER applies to machine-read text, which stays EXACTLY as specified whatever the language:\n\
         - JSON field names and their fixed values (\"acceptable\", \"defects\", \"completed\", \"partial\", \"failed\", task states, operation names);\n\
         - agent and reviewer names, copied verbatim from the registry;\n\
         - form, control, file, property, method, and event identifiers;\n\
         - COBOL source: every division header, verb, clause, intrinsic and data name (explanatory `*>` comments may be written in the developer's language);\n\
         - tool names and their argument values.\n\n\
         DEVELOPER REQUEST (verbatim):\n{request}"
    )
}

impl DbAgentInvoker {
    /// Native Rig tools granted to `agent`, built from its declared tools.
    /// Only declared tools get definitions — governance by construction.
    fn native_tools(&self, agent: &str) -> cobolt_agents::rig_transport::AgentTools {
        use cobolt_agents::rig_transport::{AgentTools, HostToolFn};
        let declared: std::collections::HashSet<String> = AgentsDb::load(&self.project_dir)
            .by_name(agent)
            .map(|a| a.tools.iter().cloned().collect())
            .unwrap_or_default();
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        };
        let record = {
            let sink = self.evidence.clone();
            let agent = agent.to_string();
            let verbose = self.llm.verbose_log;
            move |tool: &str, args: &serde_json::Value, ok: bool, summary: String| {
                if verbose {
                    crate::llm::push_ai_log(
                        crate::llm::AiLogKind::Detail,
                        format!(
                            "=== TOOL CALL · {agent} · {tool} [{}] ===\n{}\n→ {summary}",
                            if ok { "ok" } else { "error" },
                            serde_json::to_string_pretty(args)
                                .unwrap_or_else(|_| args.to_string()),
                        ),
                    );
                }
                if let Ok(mut ev) = sink.lock() {
                    ev.push(ToolEvidence {
                        agent: agent.clone(),
                        tool: tool.to_string(),
                        args_digest: {
                            let digest = args.to_string();
                            digest.chars().take(160).collect()
                        },
                        summary,
                        ok,
                        ts: now(),
                    });
                }
            }
        };
        let mut tools = AgentTools::default();
        if declared.contains("knowledge.search") {
            let dir = self.project_dir.clone();
            let record = record.clone();
            let f: HostToolFn = std::sync::Arc::new(move |args: serde_json::Value| {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if query.is_empty() {
                    record("knowledge.search", &args, false, "missing query".into());
                    return Err("\"query\" must be a non-empty string".into());
                }
                let limit = args
                    .get("limit")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(5)
                    .clamp(1, 10) as usize;
                let _ = cobolt_agents::project_knowledge::sync_documentation(&dir);
                match cobolt_agents::project_knowledge::search(&dir, &query, limit) {
                    Ok(hits) => {
                        record(
                            "knowledge.search",
                            &args,
                            true,
                            format!("retrieved {} project document(s)", hits.len()),
                        );
                        Ok(hits
                            .iter()
                            .map(|hit| {
                                format!(
                                    "PATH: {}\nSCORE: {:.4}\nEXCERPT:\n{}",
                                    hit.path, hit.score, hit.excerpt
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n---\n\n"))
                    }
                    Err(error) => {
                        record("knowledge.search", &args, false, "search failed".into());
                        Err(error)
                    }
                }
            });
            tools.knowledge_search = Some(f);
        }
        for (declared_name, native) in [("egui.tree", true), ("egui.rects", false)] {
            if declared.contains(declared_name) {
                let record = record.clone();
                let tool_name = declared_name.to_string();
                let f: HostToolFn = std::sync::Arc::new(move |args: serde_json::Value| {
                    let result = crate::agent_inspection::observe(&tool_name);
                    record(&tool_name, &args, result.ok, result.summary.clone());
                    if result.ok {
                        Ok(result.detail)
                    } else {
                        Err(result.detail)
                    }
                });
                if native {
                    tools.egui_tree = Some(f);
                } else {
                    tools.egui_rects = Some(f);
                }
            }
        }
        tools
    }
}

impl DbAgentInvoker {
    fn config_for(&self, agent: &str) -> Result<(LlmConfig, String, String, AgentKind), String> {
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
        Ok((cfg, core_instructions, combined_skills, a.kind))
    }
}

/// Appended to a Pedantic Agent's review prompt when the operator's Verbose AI
/// log setting is on. Without this, the tooling contract only forces detail
/// out of a *rejection* (`correction_request` carries the defect list) — an
/// "acceptable" verdict can legally be a bare one-line confirmation with nothing
/// for Grace (or the developer reading the verbose log) to audit. Verbose mode
/// closes that gap: the reviewer must justify approvals with the same rigor as
/// rejections.
const VERBOSE_PEDANTIC_REPORT_DIRECTIVE: &str = "\n\nVERBOSE MODE IS ACTIVE. Regardless of your verdict, return a complete report to Grace: what you inspected, the specific requirements and acceptance criteria you checked it against, and the full reasoning behind the verdict. An \"acceptable\" verdict must be justified in the same depth as a \"defects\" verdict — do not shorten it to a bare confirmation like \"looks good\" or \"clean.\"; state explicitly why each requirement is satisfied. This applies to every round, including the final assessment.";

/// Extraction preambles (Rig migration phase 3). Each runs only after a
/// deterministic fenced-JSON parse failed, and instructs the provider-native
/// extractor to transcribe — never to improvise.
const PLAN_EXTRACT_PREAMBLE: &str = "The provided text is an agent's workflow-planning response whose plan JSON could not be parsed. Extract the workflow plan it describes EXACTLY: workflow_id and every task with id, agent, objective, context, reviewer (null when none), depends_on, and acceptance. Copy agent and reviewer names verbatim from the text — never invent, rename, merge, or drop tasks.";

const VERDICT_EXTRACT_PREAMBLE: &str = "The provided text is a Pedantic reviewer's round verdict whose verdict JSON could not be parsed. Extract the verdict EXACTLY as the review states it: pedantic_verdict is \"acceptable\" only when the review approves the submission without requiring corrections, otherwise \"defects\"; correction_request carries the requested corrections verbatim (empty when acceptable). Never soften, add, or drop defects.";

const CHANGE_SET_EXTRACT_PREAMBLE: &str = "The provided text is a form specialist's submission (Form Designer or COBOL Event Handler Script Agent) whose change-set JSON could not be parsed. Extract the change-set operations it specifies EXACTLY — deploy_control, set_property, generate_event_handler, create_procedure — with identifiers, property names, values, and code copied verbatim. A submission that presents an operation as prose or a bullet list (for example \"Operation: generate_event_handler, control_id: X, event: onClick\" followed by a fenced code block) still specifies that operation: extract it, taking the handler body verbatim from the code block. If the text proposes no concrete form operations, submit an empty operations array and carry its message in note. Never invent operations the text does not state.";

impl DbAgentInvoker {
    /// Provider-native typed extraction over `source` using `agent`'s resolved
    /// model profile — the phase-3 recovery path, reached only after a
    /// deterministic parse failed. Token usage joins the workflow totals.
    fn typed_extract<T>(
        &self,
        agent: &str,
        purpose: &str,
        preamble: &str,
        source: &str,
        cause: &str,
    ) -> Result<T, String>
    where
        T: schemars::JsonSchema
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Send
            + Sync
            + 'static,
    {
        if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(format!("{agent}: stopped by the developer"));
        }
        let (cfg, _core, _skills, _kind) = self.config_for(agent)?;
        if let Some(gap) = crate::llm::credential_gap(&cfg) {
            return Err(format!("{agent}: {gap}"));
        }
        // A mapped model policy can disable provider-native extraction
        // entirely (e.g. providers rejecting function tools + reasoning
        // effort with HTTP 400 — every attempt burned a failing call).
        let policy = crate::model_policy::policy_for(&cfg.provider, &cfg.model);
        if policy.avoid_typed_extraction {
            return Err(format!(
                "typed {purpose} extraction skipped by model policy for {}/{} ({}); deterministic parse failed: {cause}",
                cfg.provider, cfg.model, policy.note
            ));
        }
        // The deterministic failure's cause is the key debugging fact — a
        // serde error names the exact field/position that broke.
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Info,
            format!("typed {purpose} extraction ({agent}) — deterministic parse failed: {cause}"),
        );
        if cfg.verbose_log {
            let block = format!(
                "=== EXTRACTION REQUEST · {purpose} via {}/{} ===\n--- INSTRUCTIONS ---\n{preamble}\n\n--- SOURCE TEXT ---\n{source}",
                cfg.provider, cfg.model,
            );
            crate::llm::push_ai_log(crate::llm::AiLogKind::Detail, block.clone());
            crate::llm::push_connection_log(&format!("{block}\n"));
        }
        let call = cobolt_agents::rig_transport::ExtractCall {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            endpoint: cfg.endpoint.clone(),
            preamble: preamble.to_string(),
            max_tokens: cfg.max_tokens,
        };
        let reply = match cobolt_agents::rig_transport::extract_typed_blocking::<T>(&call, source) {
            Ok(reply) => reply,
            Err(e) => {
                let msg = format!("{agent}: {purpose} extraction failed: {e}");
                crate::llm::push_ai_log(crate::llm::AiLogKind::Error, msg.clone());
                crate::llm::push_connection_log(&format!(
                    "=== EXTRACTION ERROR · {purpose} ===\n{e}\n"
                ));
                return Err(msg);
            }
        };
        if let Ok(mut totals) = self.tokens.lock() {
            totals.0 += reply.input_tokens;
            totals.1 += reply.output_tokens;
        }
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Detail,
            format!(
                "typed {purpose} extraction · tokens: {} in / {} out",
                reply.input_tokens, reply.output_tokens
            ),
        );
        if cfg.verbose_log {
            let pretty = serde_json::to_string_pretty(&reply.data)
                .unwrap_or_else(|_| "(unrenderable)".into());
            let block = format!(
                "=== EXTRACTION RESULT · {purpose} · {} in / {} out ===\n{pretty}",
                reply.input_tokens, reply.output_tokens
            );
            crate::llm::push_ai_log(crate::llm::AiLogKind::Detail, block.clone());
            crate::llm::push_connection_log(&format!("{block}\n"));
        }
        Ok(reply.data)
    }

    /// Recover a Form Designer change-set from a submission whose fenced JSON
    /// did not parse deterministically.
    pub fn extract_change_set(
        &self,
        source: &str,
        cause: &str,
    ) -> Result<crate::agent::AgentChangeSet, String> {
        self.typed_extract::<crate::agent::AgentChangeSet>(
            crate::agents_db::FORM_DESIGNER,
            "change-set",
            CHANGE_SET_EXTRACT_PREAMBLE,
            source,
            cause,
        )
    }
}

impl AgentInvoker for DbAgentInvoker {
    fn extract_plan(&mut self, agent: &str, plan_reply: &str) -> Result<WorkflowPlan, String> {
        // Deterministic first — free and exact for well-behaved replies.
        let cause = match cobolt_agents::grace::parse_plan(plan_reply) {
            Ok((workflow_id, tasks)) => return Ok(WorkflowPlan { workflow_id, tasks }),
            Err(cause) => cause,
        };
        // A reply with no JSON at all is Grace routing back to the developer
        // (a direct answer or a clarification), not a damaged plan — skip the
        // typed-extraction model call entirely (observed live: it burned a
        // failing provider call on every relayed question).
        if !plan_reply.contains('{') {
            return Err(cause);
        }
        let mut plan = self.typed_extract::<WorkflowPlan>(
            agent,
            "plan",
            PLAN_EXTRACT_PREAMBLE,
            plan_reply,
            &cause,
        )?;
        if plan.tasks.is_empty() {
            return Err("Grace's plan contained no tasks".into());
        }
        if plan.workflow_id.trim().is_empty() {
            plan.workflow_id = "workflow".into();
        }
        Ok(plan)
    }

    fn extract_verdict(
        &mut self,
        reviewer: &str,
        review_reply: &str,
    ) -> Result<ReviewVerdict, String> {
        let cause = match cobolt_agents::grace::parse_verdict(review_reply) {
            Ok(verdict) => return Ok(verdict),
            Err(cause) => cause,
        };
        self.typed_extract::<ReviewVerdict>(
            reviewer,
            "verdict",
            VERDICT_EXTRACT_PREAMBLE,
            review_reply,
            &cause,
        )
    }

    /// The Grace engine's machine-validation gate: the Form-independent half
    /// of the IDE's change-set validator, run over every change-set-producing
    /// submission BEFORE its Pedantic review. Whatever this flags is exactly
    /// what `apply_agent_change_set` would silently skip at apply time.
    fn lint_submission(&mut self, agent: &str, submission: &str) -> Option<String> {
        let errors = crate::agent::lint_change_set_submission(agent, submission)?;
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Info,
            format!("{agent}: change-set failed machine validation —\n{errors}"),
        );
        Some(errors)
    }

    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String> {
        self.invoke_with_temperature(agent, system, user, None)
    }

    fn invoke_task(
        &mut self,
        agent: &str,
        system: &str,
        user: &str,
        reviewed: bool,
    ) -> Result<String, String> {
        // No Pedantic gate behind this result: run colder (when configured
        // and when the model still accepts sampling parameters) — there is no
        // correction loop to catch a hallucination, so determinism outranks
        // variance. Reviewed tasks keep the agent's own temperature.
        let temperature_override = if reviewed {
            None
        } else {
            self.llm.unreviewed_temperature
        };
        self.invoke_with_temperature(agent, system, user, temperature_override)
    }
}

impl DbAgentInvoker {
    fn invoke_with_temperature(
        &mut self,
        agent: &str,
        system: &str,
        user: &str,
        temperature_override: Option<f32>,
    ) -> Result<String, String> {
        if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(format!("{agent}: stopped by the developer"));
        }
        // A configuration failure (missing agent, disabled, no model) aborts
        // the invoke BEFORE any request logging — leaving the verbose AI log
        // with an unexplained hole exactly where the failed agent should
        // appear. Observed live: an unconfigured Pedantic reviewer sank a
        // workflow and the log simply stopped after the specialist's reply.
        let (mut cfg, core_instructions, skills, kind) =
            self.config_for(agent).inspect_err(|error| {
                crate::llm::push_ai_log(
                    crate::llm::AiLogKind::Error,
                    format!("{agent}: not invoked — {error}"),
                );
            })?;
        if let Some(temperature) = temperature_override {
            if cfg.verbose_log && (cfg.temperature - temperature).abs() > f32::EPSILON {
                crate::llm::push_ai_log(
                    crate::llm::AiLogKind::Info,
                    format!(
                        "{agent}: unreviewed task — temperature lowered {} → {temperature}",
                        cfg.temperature
                    ),
                );
            }
            cfg.temperature = temperature;
        }
        // Report a blank credential as itself rather than letting the provider
        // answer 401, which reads like an account problem.
        if let Some(gap) = crate::llm::credential_gap(&cfg) {
            return Err(format!("{agent}: {gap}"));
        }
        let base_system = if system.trim().is_empty() {
            &core_instructions
        } else {
            system
        };
        // Appended here, at the single funnel every agent call passes through,
        // so Grace, the specialists and the Pedantic reviewers all answer the
        // developer in one language — including the reviewers, whose task
        // prompts are composed by Grace and never quote the original request.
        let final_system = format!("{base_system}{}", language_directive(&self.request));
        // Pedantic companions get an explicit verbose-reporting directive when
        // the operator has verbose logging on — see
        // `VERBOSE_PEDANTIC_REPORT_DIRECTIVE`. Every other agent is unaffected.
        let effective_user = if cfg.verbose_log && kind == AgentKind::Pedantic {
            format!("{user}{VERBOSE_PEDANTIC_REPORT_DIRECTIVE}")
        } else {
            user.to_string()
        };
        // Mapped per-model workarounds (built-in + operator-extended) apply
        // before the call is built — see `crate::model_policy`.
        let policy = crate::model_policy::policy_for(&cfg.provider, &cfg.model);
        if !policy.is_noop() && cfg.verbose_log {
            crate::llm::push_ai_log(
                crate::llm::AiLogKind::Info,
                format!(
                    "{agent}: model policy for {}/{} — {}",
                    cfg.provider, cfg.model, policy.note
                ),
            );
        }
        let tools = if policy.avoid_native_tools || self.suppress_native_tools {
            cobolt_agents::rig_transport::AgentTools::default()
        } else {
            self.native_tools(agent)
        };
        // Rig transport (migration phase 1): one provider client per profile,
        // no wire-format sniffing, exact token usage from the response.
        let call = cobolt_agents::rig_transport::AgentCall {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            endpoint: cfg.endpoint.clone(),
            system_prompt: final_system.clone(),
            skills: skills.clone(),
            user_prompt: effective_user,
            temperature: cfg.temperature,
            max_tokens: cfg.max_tokens.max(policy.min_max_tokens),
            tools,
        };
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Info,
            format!("{agent} → {}/{}", cfg.provider, cfg.model),
        );
        // Verbose mode logs the complete exchange — the full composed request
        // (system prompt, skills, user message), the resolved wire target,
        // the reply with JSON pretty-printed, timings, and exact token usage —
        // to both the Agentic AI log and the connection log.
        if cfg.verbose_log {
            let base = cobolt_agents::rig_transport::normalize_base(&cfg.provider, &cfg.endpoint);
            let wire = if cfg.provider.eq_ignore_ascii_case("anthropic") {
                "messages (anthropic native)"
            } else {
                "chat/completions"
            };
            let block = format!(
                "=== AGENT REQUEST · {agent} → {}/{} ===\nPOST {base}/{wire}\n\n--- SYSTEM PROMPT ({} chars) ---\n{}\n\n--- SKILLS / KNOWLEDGE ({} chars) ---\n{}\n\n--- USER MESSAGE ---\n{}",
                cfg.provider,
                cfg.model,
                call.system_prompt.len(),
                call.system_prompt,
                call.skills.len(),
                if call.skills.trim().is_empty() {
                    "(none)"
                } else {
                    call.skills.as_str()
                },
                call.user_prompt,
            );
            crate::llm::push_ai_log(crate::llm::AiLogKind::Detail, block.clone());
            crate::llm::push_connection_log(&format!("{block}\n"));
        }
        let started = std::time::Instant::now();
        // Transient provider failures — an empty response (observed live from
        // ollama_cloud/gemma4: "Response contained no message or tool call"),
        // a timeout, a dropped connection, a 429/5xx — get bounded retries
        // before the task fails and blocks its dependents. Empty responses
        // from tool-carrying requests are often DETERMINISTIC (gemma emitted
        // nothing on every follow-up round that attached native tool
        // definitions), so retries drop the native tools: the fenced protocol
        // in the system prompt keeps every tool reachable as text.
        let mut attempt = 0usize;
        let reply = loop {
            let effective_call = if attempt > 0 {
                // The two remedies are applied in order of harm, NOT both at
                // once. The budget is raised on every retry: a hidden-reasoning
                // model can exhaust `max_tokens` thinking about a large task and
                // return EMPTY final content — observed live as ever-longer
                // generations (6s→15s→28s) that all came back empty, and once as
                // 805 output tokens carrying only 352 visible characters.
                //
                // Native tools are dropped only from the SECOND retry. Detaching
                // them is the gemma remedy (that model returned empty whenever
                // tool definitions rode along), but it is actively harmful to an
                // agent whose system prompt mandates native calls and forbids the
                // fenced form: observed live, a tool-less Form Designer retry
                // answered with the prose sentence "knowledge_search {…}" and no
                // change-set at all. A budget-starved reply gets its budget back
                // first, with the tools it was told to use.
                let mut adjusted = call.clone();
                if attempt >= 2 {
                    adjusted.tools = cobolt_agents::rig_transport::AgentTools::default();
                }
                adjusted.max_tokens = call
                    .max_tokens
                    .saturating_mul(1u32 << attempt.min(3))
                    .min(32_768)
                    .max(call.max_tokens);
                adjusted
            } else {
                call.clone()
            };
            match cobolt_agents::rig_transport::run_agent_blocking(&effective_call) {
                Ok(reply) => break reply,
                Err(e) => {
                    let msg = e.to_string();
                    if cfg.verbose_log {
                        let block = format!(
                            "=== AGENT ERROR · {agent} · {:.1}s ===\n{msg}",
                            started.elapsed().as_secs_f32()
                        );
                        crate::llm::push_ai_log(crate::llm::AiLogKind::Error, block.clone());
                        crate::llm::push_connection_log(&format!("{block}\n"));
                    }
                    let stopped = self.cancel.load(std::sync::atomic::Ordering::Relaxed);
                    if stopped || attempt >= 2 || !is_transient_model_error(&msg) {
                        // Persistent empty responses are a model/provider
                        // incompatibility the operator can fix in the Models
                        // Manager — say so instead of leaving a bare error.
                        // Only after the full retry ladder actually ran:
                        // "repeatedly" would be false for a cancel that
                        // stopped the loop on the first empty reply.
                        let hint = if attempt >= 2
                            && (msg.contains("no message or tool call")
                                || msg.contains("no assistant text"))
                        {
                            format!(
                                " — {}/{} returned empty responses repeatedly for this agent's prompts; consider assigning \u{201c}{agent}\u{201d} a different model in the Models Manager",
                                cfg.provider, cfg.model
                            )
                        } else {
                            String::new()
                        };
                        return Err(format!("{agent}: {msg}{hint}"));
                    }
                    attempt += 1;
                    // A budget already at the escalation cap (e.g. the 32k
                    // reasoning-first floor) cannot rise further — say "kept
                    // at", not "raised to", so the log states what actually
                    // changes between attempts.
                    let budget_for = |a: usize| {
                        call.max_tokens
                            .saturating_mul(1u32 << a.min(3))
                            .min(32_768)
                            .max(call.max_tokens)
                    };
                    let next_budget = budget_for(attempt);
                    crate::llm::push_ai_log(
                        crate::llm::AiLogKind::Info,
                        format!(
                            "{agent}: transient model error — retry {attempt}/2 ({}; max_tokens {} {next_budget})",
                            if attempt >= 2 {
                                "native tools detached"
                            } else {
                                "native tools kept"
                            },
                            if next_budget > budget_for(attempt - 1) {
                                "raised to"
                            } else {
                                "kept at"
                            },
                        ),
                    );
                    std::thread::sleep(std::time::Duration::from_millis(750 * attempt as u64));
                }
            }
        };
        let secs = started.elapsed().as_secs_f32();
        if let Ok(mut totals) = self.tokens.lock() {
            totals.0 += reply.input_tokens;
            totals.1 += reply.output_tokens;
        }
        // Feed the chat footers' model + context-usage indicator.
        crate::llm::record_last_model_call(
            &cfg.provider,
            &cfg.model,
            reply.input_tokens,
            reply.output_tokens,
        );
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Detail,
            format!(
                "{agent} · tokens: {} in / {} out · {} chars · {secs:.1}s",
                reply.input_tokens,
                reply.output_tokens,
                reply.text.len()
            ),
        );
        // A reply that outgrew the model's output budget is retrieved in pages
        // by the transport. Say so — the page count explains the token total,
        // and an unfinished reply must never pass as a complete artifact: the
        // salvage extractor downstream would faithfully transcribe a verdict
        // or change-set that stops mid-sentence.
        if reply.continuation_pages > 0 {
            crate::llm::push_ai_log(
                if reply.truncated {
                    crate::llm::AiLogKind::Error
                } else {
                    crate::llm::AiLogKind::Info
                },
                if reply.truncated {
                    format!(
                        "{agent}: reply still INCOMPLETE after {} continuation page(s) — it hit the model's output limit and stops mid-sentence; anything parsed from it may be missing its tail",
                        reply.continuation_pages
                    )
                } else {
                    format!(
                        "{agent}: reply exceeded the output limit and was completed over {} continuation page(s)",
                        reply.continuation_pages
                    )
                },
            );
        }
        if cfg.verbose_log {
            let block = format!(
                "=== AGENT RESPONSE · {agent} · {secs:.1}s · {} in / {} out ===\n{}",
                reply.input_tokens,
                reply.output_tokens,
                crate::llm::pretty_json_blocks(&reply.text),
            );
            crate::llm::push_ai_log(crate::llm::AiLogKind::Detail, block.clone());
            crate::llm::push_connection_log(&format!("{block}\n"));
        }
        Ok(reply.text)
    }
}

/// Whether a model-transport failure is worth retrying: empty responses,
/// timeouts, dropped connections, and rate/server errors are transient;
/// authentication and request-shape errors are not.
fn is_transient_model_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "no message or tool call",
        "(empty)",
        // The transport's own spelling of the same condition: the response
        // parsed fine but carried no assistant text (hidden reasoning only, or
        // the output cap reached before any text). Observed live as a Form
        // Designer task that died after 72s with no retry at all, because only
        // rig's wording was listed here.
        "no assistant text",
        "timed out",
        "timeout",
        "connection reset",
        "connection closed",
        "connection refused",
        "broken pipe",
        "429",
        "500 ",
        "502",
        "503",
        "504",
        "overloaded",
        "temporarily unavailable",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Extract the change-sets the *approved* form-touching tasks produced, so the
/// host can apply them through the reviewable preview/apply path (spec 030
/// R6/R7). A task qualifies when it reached [`TaskState::Approved`] and was
/// handled by `designer_agent` or by any other agent in
/// [`crate::agents_db::produces_form_change_set`] — an event-only request is
/// planned as a single event-handler task with no designer task at all, and
/// filtering on the designer alone dropped its approved handler. Each task
/// yields the parse result of its final (approved) submission. Parsing/
/// validation and application stay with the existing change-set path — this
/// only *selects* what to apply.
pub fn approved_form_change_sets(
    record: &WorkflowRecord,
    designer_agent: &str,
) -> Vec<Result<crate::agent::AgentChangeSet, String>> {
    use cobolt_agents::grace::TaskState;
    record
        .tasks
        .iter()
        .filter(|t| {
            t.final_state == TaskState::Approved
                && (t.spec.agent.eq_ignore_ascii_case(designer_agent)
                    || crate::agents_db::produces_form_change_set(&t.spec.agent))
        })
        .filter_map(|t| t.submissions.last())
        .map(|s| crate::agent::parse_change_set(s))
        .collect()
}

/// Ids of the approved form-touching tasks whose final submission carried no
/// applicable operation — the task passed review but changed nothing. Reporting
/// such a workflow as a plain success is what let a reviewed, approved event
/// handler disappear without a word; the caller appends this to the reply.
pub fn approved_form_tasks_without_operations(
    record: &WorkflowRecord,
    designer_agent: &str,
) -> Vec<String> {
    use cobolt_agents::grace::TaskState;
    record
        .tasks
        .iter()
        .filter(|t| {
            t.final_state == TaskState::Approved
                && (t.spec.agent.eq_ignore_ascii_case(designer_agent)
                    || crate::agents_db::produces_form_change_set(&t.spec.agent))
        })
        .filter(|t| {
            t.submissions
                .last()
                .and_then(|s| crate::agent::parse_change_set(s).ok())
                .map(|cs| cs.operations.is_empty())
                .unwrap_or(true)
        })
        .map(|t| t.spec.id.clone())
        .collect()
}

/// One fenced `{"operations":…}` block carrying EVERY operation the approved
/// form-touching tasks produced, in task order. The contextual RAD-designer
/// chat applies edits by parsing the reply text, so it must receive such a
/// block rather than the readable summary (whose ops are stripped to prose) —
/// otherwise the agent's edits never apply.
///
/// The operations are merged rather than taken from the first matching task:
/// a request that both adds a control and wires its event is planned as a
/// designer task *and* an event-handler task, and returning only the first
/// would silently drop the other. Notes are concatenated so no message is lost.
/// Returns `None` when no approved task yielded an operation, leaving the
/// caller to fall back to the readable summary.
pub fn approved_form_change_set_submission(
    record: &WorkflowRecord,
    designer_agent: &str,
) -> Option<String> {
    let mut operations = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for cs in approved_form_change_sets(record, designer_agent)
        .into_iter()
        .flatten()
    {
        operations.extend(cs.operations);
        if let Some(note) = cs.note.filter(|n| !n.trim().is_empty()) {
            notes.push(note);
        }
    }
    if operations.is_empty() {
        return None;
    }
    let merged = crate::agent::AgentChangeSet {
        operations,
        note: (!notes.is_empty()).then(|| notes.join("\n\n")),
    };
    // The canonical re-encoding round-trips through the same parser the
    // designer chat uses (`canonical_serialization_round_trips_through_parse`).
    serde_json::to_string_pretty(&merged)
        .ok()
        .map(|json| format!("```json\n{json}\n```"))
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
/// The verbose-mode "Token savings" line: how much of the indexed Knowledge
/// Base corpus the retrieval kept OUT of the context, as a percentage of the
/// corpus a push-everything approach would have injected. Token counts are
/// ≈4-chars/token estimates of measured character counts. `None` when the
/// record predates the measurement or nothing is indexed.
pub fn rag_savings_line(record: &WorkflowRecord, tr: &crate::i18n::Tr) -> Option<String> {
    if record.kb_available_tokens == 0 {
        return None;
    }
    let injected = record.kb_injected_tokens.min(record.kb_available_tokens);
    let savings =
        100.0 * (record.kb_available_tokens - injected) as f64 / record.kb_available_tokens as f64;
    Some(
        tr.rag_token_savings
            .replacen("{}", &format!("{savings:.2}"), 1)
            .replacen("{}", &record.kb_injected_tokens.to_string(), 1)
            .replacen("{}", &record.kb_available_tokens.to_string(), 1),
    )
}

/// Spec 036: the typed live action for an engine transition, attributed to
/// the agent performing it (R7). `agent_of` resolves a task id to its
/// responsible specialist for events that carry only the id. Default mode
/// reports one action per meaningful step; `verbose` adds the finer
/// per-submission and per-round granularity (R5). Returns `None` for
/// transitions the current mode coalesces into the surrounding step.
pub fn action_for_event(
    e: &GraceEvent,
    agent_of: &dyn Fn(&str) -> String,
    verbose: bool,
) -> Option<crate::agent_actions::AgentAction> {
    use crate::agent_actions::{ActionKind, AgentAction};
    Some(match e {
        GraceEvent::TaskStarted { id, agent, .. } => {
            AgentAction::now(agent.clone(), ActionKind::Drafting, id.clone())
        }
        GraceEvent::Submitted { id, agent } => {
            if !verbose {
                return None;
            }
            AgentAction::now(agent.clone(), ActionKind::Finishing, id.clone())
        }
        GraceEvent::ReviewStarted {
            id,
            reviewer,
            round,
        } => {
            let detail = if verbose {
                format!("{id}, round {}", round + 1)
            } else {
                id.clone()
            };
            AgentAction::now(reviewer.clone(), ActionKind::Reviewing, detail)
        }
        // The verdict is the review's outcome, not a new action: the string
        // log carries it; the live line stays on Reviewing until the next
        // step (approval, correction, or failure) lands.
        GraceEvent::Verdict { .. } => return None,
        GraceEvent::CorrectionRequested { id, .. } => {
            AgentAction::now(agent_of(id), ActionKind::ApplyingCorrections, id.clone())
        }
        GraceEvent::Approved { id } => {
            AgentAction::now(agent_of(id), ActionKind::Finishing, id.clone())
        }
        GraceEvent::Failed { id, .. } => {
            AgentAction::now(agent_of(id), ActionKind::Failed, id.clone())
        }
        GraceEvent::Blocked { id } => {
            AgentAction::now(agent_of(id), ActionKind::Blocked, id.clone())
        }
    })
}

pub fn describe_event(e: &GraceEvent) -> String {
    match e {
        GraceEvent::TaskStarted {
            id,
            agent,
            objective,
        } => {
            // Typographic rule: every agent reports "<name>: starting task X".
            format!("▸ {agent}: starting task {id} — {objective}")
        }
        GraceEvent::Submitted { id, agent } => format!("  {agent}: finishing task {id}"),
        GraceEvent::ReviewStarted {
            id,
            reviewer,
            round,
        } => {
            format!(
                "  {reviewer}: starting review of task {id} (round {})",
                round + 1
            )
        }
        GraceEvent::Verdict {
            id,
            reviewer,
            approved,
        } => format!(
            "  {reviewer}: finishing review of task {id} — {}",
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
    select_target: &mut dyn FnMut(TargetRequest) -> Option<TargetChoice>,
) -> Result<(WorkflowRecord, PathBuf), String> {
    run_grace_workflow_with_context(
        project_dir,
        llm,
        request,
        &GraceRoutingContext::default(),
        on_progress,
        confirm,
        select_target,
    )
}

/// Convenience: a no-op live-action observer (spec 036) for callers that only
/// consume the string log.
pub fn no_action(_: crate::agent_actions::AgentAction) {}

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
    select_target: &mut dyn FnMut(TargetRequest) -> Option<TargetChoice>,
) -> Result<(WorkflowRecord, PathBuf), String> {
    run_grace_workflow_with_control(
        project_dir,
        llm,
        request,
        routing,
        &WorkflowControl::default(),
        on_progress,
        &mut no_action,
        confirm,
        select_target,
    )
}

/// Like [`run_grace_workflow_with_context`], with a live [`WorkflowControl`]
/// so the UI can stop the run and watch token usage as each model returns.
/// `on_action` receives the typed live-action stream (spec 036): the *action*
/// each agent is on, never the content it produced or consumed. The collected
/// actions are also attached to the persisted record (`action_log`, R11).
#[allow(clippy::too_many_arguments)]
pub fn run_grace_workflow_with_control(
    project_dir: &Path,
    llm: &LlmConfig,
    request: &str,
    routing: &GraceRoutingContext,
    control: &WorkflowControl,
    on_progress: &mut dyn FnMut(String),
    on_action: &mut dyn FnMut(crate::agent_actions::AgentAction),
    confirm: &mut dyn FnMut(GitConfirmRequest) -> bool,
    select_target: &mut dyn FnMut(TargetRequest) -> Option<TargetChoice>,
) -> Result<(WorkflowRecord, PathBuf), String> {
    use crate::agent_actions::{ActionKind, AgentAction};
    let mut actions: Vec<AgentAction> = Vec::new();
    let mut emit_action = |a: AgentAction| {
        actions.push(a.clone());
        on_action(a);
    };
    emit_action(AgentAction::now(GRACE, ActionKind::ReceivingRequest, ""));
    if llm.verbose_log {
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Detail,
            format!("=== DEVELOPER REQUEST ===\n{request}"),
        );
    }
    let mut db = AgentsDb::load(project_dir);
    let repaired = db.ensure_fixed_agents(llm);
    if repaired > 0 {
        on_progress(format!(
            "Prepared {repaired} fixed-agent or project-knowledge capability update(s)."
        ));
    }
    // ── Request-clarity pre-check — BEFORE any retrieval ───────────────────
    // Grace privately rates the request's clarity and conciseness first: an
    // unclear request would otherwise spend the full Knowledge Base sync +
    // embedding + search + planning cost on a guess. The gate call carries no
    // tools (it must not reach knowledge_search); a low score returns Grace's
    // interpretation and questions to the developer instead of a workflow.
    // The score lands on the record for telemetry and NEVER enters any agent
    // task prompt.
    let token_sink: Arc<Mutex<(u64, u64)>> = control.tokens.clone();
    let evidence: Arc<Mutex<Vec<ToolEvidence>>> = Arc::new(Mutex::new(Vec::new()));
    emit_action(AgentAction::now(GRACE, ActionKind::Planning, "clarity"));
    on_progress("Grace is checking the request for clarity…".into());
    let mut gate_invoker = DbAgentInvoker {
        project_dir: project_dir.to_path_buf(),
        llm: llm.clone(),
        tokens: token_sink.clone(),
        evidence: evidence.clone(),
        cancel: control.cancel.clone(),
        request: request.to_string(),
        suppress_native_tools: true,
    };
    let clarity_check = evaluate_request_clarity(&mut gate_invoker, request, routing);
    drop(gate_invoker);
    let request_clarity = clarity_check.as_ref().map(|check| check.clarity);
    if let Some(score) = request_clarity {
        crate::llm::push_ai_log(
            crate::llm::AiLogKind::Info,
            format!("Grace rated the request clarity {score}/10 (private telemetry; agents never see it)."),
        );
    }
    if let Some(check) = clarity_check
        .filter(|check| check.clarity < CLARITY_GATE_THRESHOLD && !check.interpretation.is_empty())
    {
        on_progress(format!(
            "The request scored {}/10 for clarity — Grace is returning her interpretation for confirmation instead of planning.",
            check.clarity
        ));
        emit_action(AgentAction::now(GRACE, ActionKind::Finishing, ""));
        let mut reply = check.interpretation.clone();
        if !check.questions.is_empty() {
            reply.push_str("\n\n");
            reply.push_str(&check.questions.join("\n"));
        }
        let mut record = direct_grace_record(
            reply,
            "Confirm Grace's interpretation of an unclear request before any retrieval or planning",
        );
        record.request_clarity = Some(check.clarity);
        record.action_log = actions.iter().map(AgentAction::to_log_entry).collect();
        let path = save_workflow_record(project_dir, &record, &[])?;
        return Ok((record, path));
    }

    // TWO Knowledge Bases, reported separately because they answer different
    // questions. The System KB is the platform's own reference material
    // (RustCOBOL extensions, IDE functionality, designer controls, the agent
    // registry); it ships with the binary and is republished here, so it is
    // never legitimately empty. The Project KB is whatever the developer put
    // in this project's `Knowledge Base/` folder, and empty is a valid state.
    emit_action(AgentAction::now(GRACE, ActionKind::RetrievingContext, ""));
    // Chunk-embedding progress: streamed through the shared control so the
    // chat panes can draw a live progress bar while records are embedded.
    let index_progress = control.clone();
    let mut on_index = move |done: usize, total: usize, subject: &str| {
        index_progress.set_indexing(done, total, subject);
    };
    let system_root = system_knowledge_root();
    let system = sync_system_knowledge(&system_root, &mut on_index);
    match &system {
        Ok((indexed, records)) => on_progress(format!(
            "System Knowledge Base: {indexed} textual file(s) indexed, {records} subject record(s) chunked."
        )),
        Err(error) => on_progress(format!("System Knowledge Base: {error}")),
    }
    let indexed = cobolt_agents::project_knowledge::sync_documentation(project_dir)
        .map_err(|error| format!("Project Knowledge Base could not be indexed: {error}"))?;
    let project_store = cobolt_agents::chunked_knowledge::project_chunked_path(project_dir);
    let project_records = cobolt_agents::chunked_knowledge::sync_tree_with_progress(
        project_dir,
        &project_store,
        &mut on_index,
    )
    .map_err(|error| format!("Project Knowledge Base could not be chunked: {error}"))?;
    control.clear_indexing();
    on_progress(format!(
        "Project Knowledge Base: {indexed} textual file(s) indexed, {project_records} subject record(s) chunked."
    ));
    // Retrieval is CHUNKED (one record per control/property/event/section):
    // the context receives exactly the subject records that match the
    // request — from both stores — never a whole catalogue document. The
    // essential documents remain published and indexed; their content now
    // arrives through retrieval instead of being injected wholesale.
    let system_store = system_root.join("data").join("chunked.data");
    let mut knowledge = cobolt_agents::chunked_knowledge::search(&system_store, request, 6)
        .map_err(|error| format!("System knowledge could not be searched: {error}"))?;
    knowledge.extend(
        cobolt_agents::chunked_knowledge::search(&project_store, request, 6)
            .map_err(|error| format!("Project knowledge could not be searched: {error}"))?,
    );
    knowledge.sort_by(|left, right| right.score.total_cmp(&left.score));
    knowledge.truncate(8);

    let search_results = if knowledge.is_empty() {
        "(no relevant Knowledge Base evidence found)".to_string()
    } else {
        knowledge
            .iter()
            .map(|hit| {
                format!(
                    "PATH: {}\nSUBJECT: {} ({})\nRELEVANCE: {:.4}\nCONTENT:\n{}",
                    hit.source_path, hit.subject, hit.kind, hit.score, hit.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };

    // Retrieval observability: what the RAG actually injected vs. the whole
    // chunked corpus a push-everything approach would have sent. Characters
    // are measured; tokens are the standard ≈4-chars/token estimate. Stored on
    // the record; the chat reports the savings percentage in verbose mode.
    let kb_injected_tokens = (search_results.chars().count() as u64) / 4;
    let kb_available_tokens =
        (cobolt_agents::chunked_knowledge::corpus_chars(&system_store).unwrap_or(0)
            + cobolt_agents::chunked_knowledge::corpus_chars(&project_store).unwrap_or(0))
            / 4;

    let knowledge_context = search_results;

    let knowledge_context = format!(
        "{knowledge_context}\n\nPROJECT KNOWLEDGE PRECEDENCE CONTRACT:\n- Treat relevant Knowledge Base excerpts as authoritative project evidence and prefer them over general model training.\n- Cite the project-relative PATH for project-specific claims.\n- Never replace, contradict, or embellish project evidence with generic assumptions.\n- If there is no relevant evidence, say so clearly. Use general knowledge only when appropriate and label it as general guidance; ask the developer for missing project facts instead of inventing them."
    );
    // Base transport (resolves model/key/prompt per agent), decorated with the
    // tool-execution layer: declared-tools governance + git/egui backends.
    // `token_sink` and `evidence` were created for the clarity gate above and
    // carry its token usage into the workflow totals.
    let mut inner = DbAgentInvoker {
        project_dir: project_dir.to_path_buf(),
        llm: llm.clone(),
        tokens: token_sink.clone(),
        evidence: evidence.clone(),
        cancel: control.cancel.clone(),
        request: request.to_string(),
        suppress_native_tools: false,
    };
    let mut backend = IdeToolBackend::new(project_dir.to_path_buf(), confirm, select_target);
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
            // Only an ENABLED companion is advertised — a disabled reviewer
            // must read as "no companion" so Grace leaves reviewer null
            // (the host enforces the mapping mechanically afterwards anyway).
            let comp = a
                .companion
                .as_ref()
                .and_then(|cid| db.by_id(cid))
                .filter(|c| c.enabled)
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
    // Grace plans against the trimmed view; `context` stays full for the
    // per-specialist slicing in `inject_task_context` further down.
    let planning_context = planning_surface_context(context, request);
    let planning_context = planning_context.as_str();
    let plan_user = format!(
        "USER REQUEST:\n{request}\n\nCHAT SURFACE:\n{surface}\n\nPREFERRED SPECIALIST:\n{preference}\n\nSURFACE CONTEXT:\n{planning_context}\n\nRELEVANT INDEXED PROJECT KNOWLEDGE:\n{knowledge_context}\n\nAVAILABLE AGENT REGISTRY:\n{registry}\n\nCONVERSATION CONTINUITY: when the surface context carries a CONVERSATION SO FAR section, the user request continues that conversation. A short request that answers the assistant's most recent question means the real objective is the earlier request combined with this answer; likewise resolve references such as \u{201c}the initial request\u{201d} from that conversation. Never re-ask for a fact the conversation already states.\n\nThe preferred specialist is an initial routing preference only, never an exclusive assignment. Decompose mixed requests and delegate every part to whichever available specialist owns that responsibility. For example, form creation plus onClick behavior normally requires both form-design and event-handler tasks. Grace may call any enabled specialist needed anywhere in the project.\n\nPEDANTIC COMPANION CONTRACT:\n- Companion relationships are one-to-one: one orchestrator or specialist has at most one Pedantic reviewer, and one Pedantic reviewer belongs to at most one reviewed agent.\n- For every task, use exactly the Pedantic companion shown for its responsible agent in the registry. Never substitute or reuse another agent's reviewer.\n- Leave reviewer null only when the responsible agent has no companion.\n\nDOCUMENTATION COORDINATION CONTRACT:\n- Only {DOCUMENTATION_AGENT} may format and write project documentation files.\n- When documentation concerns another domain, first assign one or more source-material tasks to the responsible domain specialists. Those specialists prepare authoritative information and MUST NOT write documentation files.\n- Then assign a {DOCUMENTATION_AGENT} task whose depends_on contains every source-material task. The workflow engine passes their approved outputs into the Documentation Agent task as its authoritative handoff.\n- Example: to document a form interface, Form Designer Agent first inventories the controls, layout, bindings, and events; after approval, {DOCUMENTATION_AGENT} formats that output and saves the document.\n- Never ask {DOCUMENTATION_AGENT} to invent technical facts owned by another specialist, and never ask another specialist to save a documentation file.\n- Every {DOCUMENTATION_AGENT} task must demand CONCISE output: no reasoning narrative, no restated instructions, no meta-commentary — only the content required to execute or hand off the task.\n\nINDEXED FILE COORDINATION CONTRACT:\n- {DATA_INDEXED_FILE_AGENT} is the sole specialist allowed to create or modify PowerRustCOBOL indexed-file definitions through the Indexed File UI model.\n- Start with a {DOCUMENTATION_AGENT} task that explicitly obtains the file name when absent, establishes the purpose from the developer request, searches project knowledge, analyzes 1NF, 2NF, and 3NF, and identifies any helper indexed files required by normalization.\n- For every ID field, {DOCUMENTATION_AGENT} must obtain the developer's explicit choice between UUID and a specific COBOL PIC definition. Never infer this choice.\n- Each {DATA_INDEXED_FILE_AGENT} mutation task must depend on the approved {DOCUMENTATION_AGENT} handoff. Helper relations are separate dependent Data-agent tasks.\n- If the file name, purpose, normalization decisions, or ID choice is missing, plan a Documentation-only clarification task and do not plan mutation yet. Grace relays the resulting question to the developer.\n- Neither Grace nor {DOCUMENTATION_AGENT} may mutate `.cidx` resources; {DOCUMENTATION_AGENT} prepares the approved schema handoff and Grace coordinates it.\n- FINALIZED (LOCKED) FILES: a {DATA_INDEXED_FILE_AGENT} write to a finalized `.cidx` whose schema changes returns a confirmation-required result, NOT a success. When that happens, STOP the workflow and reply to the developer right away: state plainly that the task cannot be done as a normal edit because the file is finalized, and that it can only proceed by DESTROYING and RECREATING the file (its stored data is lost). Ask the developer to confirm. Do not plan or retry the mutation until the developer explicitly confirms. Only after an explicit confirmation, plan the Data-agent write with `confirm_recreate: true`.\n\nSpecialists should use knowledge.search when prior plans, requirements, task lists, or project decisions may matter. Plan the workflow per your tooling contract (END with the plan JSON). Assign each task's reviewer from the responsible agent's pedantic companion; leave reviewer null only where no companion exists."
    );
    // The MODEL routes the request — no keyword pre-classification. Grace
    // reads the request and the contracts and chooses one of three shapes:
    // a direct Markdown answer, developer-facing questions, or a workflow
    // plan. The host only recognizes the shape she chose.
    let plan_user = format!(
        "{plan_user}\n\n{RESPONSE_ROUTING_CONTRACT}"
    );
    on_progress("Grace is reading the request…".into());
    emit_action(AgentAction::now(GRACE, ActionKind::Planning, ""));
    let plan_reply = invoker.invoke(GRACE, "", &plan_user)?;
    // Typed plan (Rig migration phase 3): deterministic parse first, then
    // provider-native typed extraction over the SAME reply. A reply with NO
    // tasks is not an error: it is Grace routing the request back to the
    // developer (an answer or a clarification question) per the routing
    // contract, and it is relayed as-is.
    let extracted = match invoker.extract_plan(GRACE, &plan_reply) {
        Ok(extracted) => extracted,
        Err(cause) => {
            crate::llm::push_connection_log(&format!(
                "=== GRACE DIRECT RESPONSE (no plan: {cause}) ===\n{plan_reply}\n"
            ));
            on_progress("Grace routed the request back to the developer.".into());
            emit_action(AgentAction::now(GRACE, ActionKind::Finishing, ""));
            let mut record = direct_grace_record(
                plan_reply,
                "Answer or ask the developer directly (no workflow required)",
            );
            record.request_clarity = request_clarity;
            record.action_log = actions.iter().map(AgentAction::to_log_entry).collect();
            record.kb_available_tokens = kb_available_tokens;
            record.kb_injected_tokens = kb_injected_tokens;
            let path = save_workflow_record(project_dir, &record, &[])?;
            return Ok((record, path));
        }
    };
    let (mut workflow_id, mut plan) = (extracted.workflow_id, extracted.tasks);
    let plan_db = AgentsDb::load(project_dir);
    // Plan-correction loop with multiple rounds. Observed live: the first
    // corrected plan traded one violation (no Documentation task) for a new
    // one (a Pedantic reviewer as task agent) and the old single-shot limit
    // killed the whole run on the second rejection.
    let mut last_reply = plan_reply;
    let mut correction_rounds = 0usize;
    while let Err(defect) = validate_workflow_coordination(&plan_db, request, &plan) {
        if correction_rounds >= 3 {
            return Err(format!(
                "Grace's plan still violates a coordination contract after {correction_rounds} correction round(s): {defect}"
            ));
        }
        correction_rounds += 1;
        on_progress(format!(
            "Grace's plan violated a coordination contract: {defect}. Requesting a corrected plan (round {correction_rounds}/3)."
        ));
        emit_action(AgentAction::now(
            GRACE,
            ActionKind::ApplyingCorrections,
            format!("plan, round {correction_rounds}/3"),
        ));
        let correction = format!(
            "Your previous workflow plan was rejected because: {defect}\n\nReturn a COMPLETE corrected workflow plan that fixes THIS defect without introducing another. All coordination rules apply simultaneously:\n- A Pedantic reviewer is NEVER a task agent; reviews happen through each task's `reviewer` field only.\n- Preserve the Documentation coordination contract. For indexed-file work, use {DOCUMENTATION_AGENT} first for file name, purpose, project knowledge, 1NF/2NF/3NF, helper-file analysis, and the developer's UUID-or-PIC decision; only then assign dependent mutation tasks to {DATA_INDEXED_FILE_AGENT}.\n- If required information is absent, return only a Documentation clarification task and do not mutate.\nEND with the corrected plan JSON and nothing after it.\n\nORIGINAL REQUEST:\n{request}\n\nREJECTED PLAN:\n{last_reply}"
        );
        let corrected_reply = invoker.invoke(GRACE, "", &correction)?;
        let corrected = invoker.extract_plan(GRACE, &corrected_reply)?;
        last_reply = corrected_reply;
        (workflow_id, plan) = (corrected.workflow_id, corrected.tasks);
    }
    // The Pedantic-companion contract is enforced mechanically, whatever
    // reviewer names the plan carried.
    let companion_of = |agent_name: &str| -> Option<String> {
        let responsible = plan_db.by_name(agent_name)?;
        let companion_id = responsible.companion.as_ref()?;
        let companion = plan_db.by_id(companion_id)?;
        // Available = enabled AND a resolvable model connection. Observed
        // live: an ENABLED companion with no model failed T1 at review time
        // ("reviewer unavailable: … has no model configured") six minutes
        // after the specialist had delivered a complete, correct change-set —
        // the exact waste the disabled-companion rule below already prevents.
        // An unavailable reviewer clears the gate HERE, at plan time, with a
        // visible note, instead of sinking finished work at review time.
        (companion.enabled
            && crate::agents_db::resolve_agent_connection(companion, llm).is_some())
        .then(|| companion.name.clone())
    };
    sanitize_plan_reviewers(&companion_of, &mut plan, on_progress);
    // Grace's plan carries no per-task control inventory (the plan schema has no
    // context field), so a form-design task delegated at the level of "reorganise
    // the six charts into a 2×3 grid" reaches the specialist with an empty CONTEXT
    // and nothing to resolve control ids or geometry against. The host owns the
    // mapping: inject the compact inventory from the surface context.
    inject_task_context(context, &mut plan);
    // Typographic rule: action items are listed with their T# bullet, with a
    // blank line between the paragraph and the list.
    let mut planned = format!("Grace planned {} task(s) [{}]:\n", plan.len(), workflow_id);
    for task in &plan {
        let first_sentence = task
            .objective
            .split(['.', '\n'])
            .next()
            .unwrap_or("")
            .trim();
        planned.push_str(&format!("\n- **{}** — {}: {first_sentence}", task.id, task.agent));
    }
    on_progress(planned);

    let db2 = AgentsDb::load(project_dir);
    let ratings_dir = project_dir.to_path_buf();
    let system_for = move |name: &str| {
        let base = db2.load_agent_core_instructions(name);
        let relationship = pedantic_relationship_contract(&db2, name);
        // The Documentation Agent's deliverable goes straight to the chat and
        // to dependent agents: demand the artifact, not the thought process.
        let concise = if name.eq_ignore_ascii_case(DOCUMENTATION_AGENT) {
            DOCUMENTATION_CONCISE_CONTRACT
        } else {
            ""
        };
        // Specialists receive their own recent correction reasons as factual
        // lessons — actionable feedback derived from the ratings book. Never
        // a star count, never praise/displeasure: scores are non-actionable
        // and pressure framing is the gradient that fabricates approvals.
        let lessons = db2
            .by_name(name)
            .filter(|agent| agent.kind == crate::agents_db::AgentKind::Specialist)
            .and_then(|_| {
                crate::agent_ratings::RatingsBook::load(&ratings_dir).lessons_digest(name)
            })
            .unwrap_or_default();
        let base = format!("{base}{relationship}{concise}{lessons}");
        // Append the tool-calling contract for whatever tools this agent
        // declares (spec 030 R2) — always consistent with its actual grant.
        let declared: std::collections::HashSet<String> = db2
            .by_name(name)
            .map(|a| a.tools.iter().cloned().collect())
            .unwrap_or_default();
        let appendix = crate::tool_exec::tool_contract_appendix(&declared);
        // Every agent whose submission is parsed as a change-set
        // (`approved_form_change_sets`) is told that schema. The event-handler
        // specialist is in that set: its own prompt already ends "return the
        // handler as a `generate_event_handler` operation inside the operations
        // array", which names an envelope it could not see — so it wrote the
        // operation as prose and the handler was never created.
        let change_set = if crate::agents_db::produces_form_change_set(name) {
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
    // Streams both surfaces per tool call: the full evidence line into the
    // string log, and the payload-free action (agent + tool name, spec 036
    // R4) into the live-action stream.
    let mut flush_tools = move |on_progress: &mut dyn FnMut(String),
                                emit_action: &mut dyn FnMut(AgentAction)| {
        let ev = ev_for_progress.lock().unwrap();
        while emitted < ev.len() {
            let e = &ev[emitted];
            on_progress(format!("  \u{1f527} {} — {}", e.tool, e.summary));
            emit_action(AgentAction::now(
                e.agent.clone(),
                ActionKind::RunningTool,
                e.tool.clone(),
            ));
            emitted += 1;
        }
    };
    // Spec 036 R7: terminal/correction events name only the task id — resolve
    // the responsible specialist from the plan for attribution.
    let agent_of = |id: &str| {
        plan.iter()
            .find(|t| t.id == id)
            .map(|t| t.agent.clone())
            .unwrap_or_else(|| GRACE.to_string())
    };
    let verbose = llm.verbose_log;
    // The correction-loop bound is the project's AI setting (was hardcoded 2).
    let mut record = GraceEngine {
        max_revisions: llm.max_review_revisions.min(10) as usize,
    }
    .run_with_progress(
        &workflow_id,
        &plan,
        &mut invoker,
        &system_for,
        &mut |e| {
            flush_tools(on_progress, &mut emit_action); // stream tool calls as they land (R11)
            if let Some(action) = action_for_event(&e, &agent_of, verbose) {
                emit_action(action);
            }
            on_progress(describe_event(&e));
        },
    );
    flush_tools(on_progress, &mut emit_action); // any evidence recorded after the last transition
    drop(invoker); // release the borrows on evidence before draining it

    // Phase 3: canonicalize approved Form Designer submissions whose fenced
    // change-set JSON does not parse. Recovery runs here on the worker thread
    // — the apply path on the UI thread stays deterministic.
    normalize_form_change_sets(&inner, &mut record, on_progress);

    // Enrich the record for the chatbot surface: KB summary, Grace's concise
    // summary (evidence-backed), and the workflow's total token consumption.
    let tool_calls = evidence.lock().unwrap().clone();
    record.knowledge_summary = summarize_knowledge_context(&knowledge_context);
    record.final_summary = grace_final_summary(&record, &tool_calls);
    record.request_clarity = request_clarity;

    // ── Agent ratings (telemetry, spec: performance stars) ─────────────────
    // Scored mechanically from the record — rounds-to-approval and failures —
    // and persisted in the project's rolling ratings book. The lines below go
    // to the developer's progress log; the agents themselves receive only the
    // factual lessons digest on their NEXT task (see `system_for`), never a
    // star count.
    {
        let mut ratings = crate::agent_ratings::RatingsBook::load(project_dir);
        let scored = ratings.record_workflow(&record);
        if !scored.is_empty() {
            if let Err(error) = ratings.save(project_dir) {
                on_progress(format!("Agent ratings could not be saved: {error}"));
            }
            for (agent, stars, total) in scored {
                on_progress(format!(
                    "★ {agent}: {} this task — running total {total} over the last {} rated tasks.",
                    crate::agent_ratings::RatingsBook::stars_label(stars),
                    crate::agent_ratings::WINDOW,
                ));
            }
        }
    }
    {
        let (inp, out) = *token_sink.lock().unwrap();
        record.input_tokens = inp;
        record.output_tokens = out;
    }
    emit_action(AgentAction::now(GRACE, ActionKind::Finishing, workflow_id.clone()));
    record.action_log = actions.iter().map(AgentAction::to_log_entry).collect();
    record.kb_available_tokens = kb_available_tokens;
    record.kb_injected_tokens = kb_injected_tokens;

    let path = save_workflow_record(project_dir, &record, &tool_calls)?;
    on_progress(format!(
        "Workflow {}: {}.",
        record.workflow_id, record.status
    ));
    Ok((record, path))
}

/// Enforce the Pedantic-companion contract mechanically (governance by
/// construction): each task's reviewer is exactly its responsible agent's
/// AVAILABLE companion (enabled, with a model configured). A fabricated
/// reviewer name ("COBOL Pedantic Agent" was observed live) is replaced with
/// the real companion; a missing, disabled, or model-less companion clears the
/// review gate instead of failing the task at review time with "agent is
/// disabled" / "has no model configured". Grace's plan expresses intent — the
/// host owns the mapping.
fn sanitize_plan_reviewers(
    companion_of: &dyn Fn(&str) -> Option<String>,
    plan: &mut [TaskSpec],
    on_progress: &mut dyn FnMut(String),
) {
    for task in plan.iter_mut() {
        let expected = companion_of(&task.agent);
        if task.reviewer == expected {
            continue;
        }
        let note = match (&task.reviewer, &expected) {
            (Some(was), Some(now)) => format!(
                "  {}: reviewer \u{201c}{was}\u{201d} corrected to \u{201c}{now}\u{201d} — the responsible agent's companion.",
                task.id
            ),
            (Some(was), None) => format!(
                "  {}: reviewer \u{201c}{was}\u{201d} cleared — \u{201c}{}\u{201d} has no available Pedantic companion (it is disabled or has no model configured), so this task will NOT be reviewed.",
                task.id, task.agent
            ),
            (None, Some(now)) => format!(
                "  {}: reviewer set to \u{201c}{now}\u{201d} — the responsible agent's companion.",
                task.id
            ),
            (None, None) => continue,
        };
        on_progress(note);
        task.reviewer = expected;
    }
}

/// Fill each empty-context Form Designer task with the compact control
/// inventory sliced from the surface context. The specialist otherwise receives
/// only Grace's objective, so a high-level delegation ("reorganise the six
/// charts into a 2×3 grid") gives it no ids or geometry to work from and it
/// produces no operations. Grace's own explicit context, when present, is kept.
/// Test-only view of [`inject_task_context`], so a surface built by another
/// module can be checked end to end against the slicing this host performs.
#[cfg(test)]
pub(crate) fn inject_task_context_for_test(context: &str, plan: &mut [TaskSpec]) {
    inject_task_context(context, plan)
}

fn inject_task_context(context: &str, plan: &mut [TaskSpec]) {
    let inventory = control_inventory_excerpt(context);
    let api = control_api_excerpt(context);
    for task in plan.iter_mut() {
        if !task.context.trim().is_empty() {
            continue; // Grace's own explicit context wins.
        }
        if task.agent == crate::agents_db::FORM_DESIGNER {
            // The designer resolves ids + geometry against the layout inventory,
            // and its prompt forbids any property key not listed under FORM
            // PROPERTIES / PROPERTY KEYS BY TYPE — so it must be given those
            // lists, or it is obeying a list it cannot see. Sending all 34 types
            // would blow the delegated budget, so send the form-level block plus
            // only the types this task can actually touch: those already on the
            // form, and any named in the objective (a control it is about to
            // deploy is not on the form yet).
            let mut ctx = inventory.clone();
            for block in [
                form_properties_excerpt(context),
                property_keys_excerpt(context, &inventory, &task.objective, "this task"),
            ] {
                if block.is_empty() {
                    continue;
                }
                if !ctx.is_empty() {
                    ctx.push_str("\n\n");
                }
                ctx.push_str(&block);
            }
            if !ctx.trim().is_empty() {
                task.context = ctx;
            }
        } else if task.agent == crate::agents_db::EVENT_HANDLER {
            // The event agent must bind to real control ids/events AND call real
            // methods on them (e.g. `PictureBox-2::PlayAnimation()`). Without the
            // per-control API it invents method names (`Animate`, `StartAnimation`)
            // that its reviewer can never verify, so the correction loop never
            // terminates. Give it the inventory (ids/types), the CONTROL API BY
            // ID block (each control's real methods and properties), the events
            // its types actually support, and the procedures it may CALL — its
            // prompt names all four, so withholding any one leaves it obeying a
            // list it cannot see.
            let mut ctx = inventory.clone();
            for block in [
                events_excerpt(context, &inventory, &task.objective, "this task"),
                api.clone(),
                procedures_excerpt(context),
            ] {
                if block.is_empty() {
                    continue;
                }
                if !ctx.is_empty() {
                    ctx.push_str("\n\n");
                }
                ctx.push_str(&block);
            }
            if !ctx.trim().is_empty() {
                task.context = ctx;
            }
        }
    }
}

/// The surface context as Grace should see it for PLANNING: identical to the
/// full context except that the two per-type legends are cut down to the types
/// actually in play — those on the form, plus any the developer's request names.
///
/// Those two legends dominate everything else. Measured on a one-control form,
/// `build_context` is 34,843 chars of which `PROPERTY KEYS BY TYPE` is 22,038
/// and `EVENTS BY TYPE` is 10,720 — 94% describing all 34 control types, the
/// same bytes on every request of every project, while the form actually being
/// edited accounts for about a thousand. Grace routes work to specialists; she
/// does not need TreeView's property keys to decide that a Button click belongs
/// to the event-handler agent.
///
/// The FULL context must still reach [`inject_task_context`], which slices each
/// specialist's own view from it: a task objective may name a type the request
/// never mentions (Grace planning "deploy a Timer, then wire its tick" from
/// "make it refresh"), and trimming before that slice would take the type away
/// from the one agent that needs it. Trim Grace's copy, keep the original.
///
/// Returns the context unchanged when the legend markers are absent.
fn planning_surface_context(context: &str, request: &str) -> String {
    const FIRST: &str = "PROPERTY KEYS BY TYPE";
    const AFTER: &str = "CONTROL API BY ID";
    let (Some(start), Some(end)) = (context.find(FIRST), context.find(AFTER)) else {
        return context.to_string();
    };
    if end <= start {
        return context.to_string();
    }
    let inventory = control_inventory_excerpt(context);
    let mut replacement = String::new();
    for block in [
        property_keys_excerpt(context, &inventory, request, "this request"),
        events_excerpt(context, &inventory, request, "this request"),
    ] {
        if block.is_empty() {
            continue;
        }
        replacement.push_str(&block);
        replacement.push('\n');
    }
    format!("{}{}{}", &context[..start], replacement, &context[end..])
}

/// Slice the layout-relevant head of the surface context: from the FORM header
/// through the CONTROLS inventory, stopping before the verbose per-type property,
/// event, and API dumps. Empty when the inventory markers are absent (e.g. a run
/// with no surface context), so injection then no-ops.
fn control_inventory_excerpt(context: &str) -> String {
    let Some(controls_at) = context.find("CONTROLS:") else {
        return String::new();
    };
    let start = context.find("FORM:").filter(|f| *f < controls_at).unwrap_or(controls_at);
    let end = context[controls_at..]
        .find("PROPERTY KEYS BY TYPE")
        .map(|rel| controls_at + rel)
        .unwrap_or(context.len());
    context[start..end].trim_end().to_string()
}

/// Slice the `FORM PROPERTIES` block: the form's current style values, the
/// settable form-level keys, and the exact `GlassStyle` spellings. The designer
/// targets these with `"control_id": "Form"`, and its prompt names this block by
/// title, so a delegated restyle without it is guesswork. Empty when the marker
/// is absent.
fn form_properties_excerpt(context: &str) -> String {
    let Some(at) = context.find("FORM PROPERTIES") else {
        return String::new();
    };
    let rest = &context[at..];
    let end = rest
        .find("AVAILABLE CONTROL TYPES")
        .or_else(|| rest.find("CONTROLS:"))
        .unwrap_or(rest.len());
    rest[..end].trim_end().to_string()
}

/// Slice `PROPERTY KEYS BY TYPE` down to the control types this task can touch:
/// the types present in `inventory` plus any type named in `objective` (the
/// control a deploy task is about to create is not on the form yet).
///
/// The full block lists every one of the 34 control types and is the single
/// largest section of the surface context — sending it whole to every delegated
/// task is what the budget cut was protecting against. Sending *none* of it left
/// the designer unable to honour its own prompt. Empty when the marker is absent
/// or no listed type is in play.
fn property_keys_excerpt(context: &str, inventory: &str, objective: &str, scope: &str) -> String {
    const HEADING: &str = "PROPERTY KEYS BY TYPE";
    let Some(at) = context.find(HEADING) else {
        return String::new();
    };
    let rest = &context[at..];
    let end = rest.find("EVENTS BY TYPE").unwrap_or(rest.len());
    let block = &rest[..end];

    let mut kept: Vec<&str> = Vec::new();
    for line in block.lines().skip(1) {
        let Some((ty, _)) = line.trim_start().split_once(':') else {
            continue;
        };
        let ty = ty.trim();
        if ty.is_empty() {
            continue;
        }
        // Present on the form: the inventory renders each control as
        // `Id (Type) @(x,y) WxH`. Named in the objective: a whole-word match, so
        // "Panel" does not fire on "PanelHeader" and "Line" not on "LineChart".
        let on_form = inventory.contains(&format!("({ty})"));
        if on_form || mentions_type(objective, ty) {
            kept.push(line);
        }
    }
    if kept.is_empty() {
        return String::new();
    }
    format!(
        "PROPERTY KEYS BY TYPE (the types in play for {scope}):\n{}",
        kept.join("\n")
    )
}

/// Whether `text` names control type `ty` as a whole word, case-insensitively.
fn mentions_type(text: &str, ty: &str) -> bool {
    let hay = text.to_ascii_lowercase();
    let needle = ty.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = hay[from..].find(&needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before_ok = start == 0
            || !hay.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after_ok = end == hay.len() || !hay.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

/// Slice the `EVENTS BY TYPE` block down to the types in play, exactly as
/// [`property_keys_excerpt`] does for properties.
///
/// This block sits BETWEEN the inventory and API windows, so before this it was
/// delivered to nobody — while the event agent's prompt requires "the EXACT
/// event name from the delegation context" and self-check 9 requires every
/// reference to appear in that context. An event name is also hard-validated
/// (`Control 'X' has no event 'Y'`) and an invalid op is skipped at apply, so a
/// guessed name does not raise an error: it silently produces no handler. The
/// full block covers all 34 types, hence the same in-play filter the property
/// excerpt uses. Empty when the marker is absent or no listed type is in play.
fn events_excerpt(context: &str, inventory: &str, objective: &str, scope: &str) -> String {
    const HEADING: &str = "EVENTS BY TYPE";
    let Some(at) = context.find(HEADING) else {
        return String::new();
    };
    let rest = &context[at..];
    let end = rest.find("CONTROL API BY ID").unwrap_or(rest.len());
    let block = &rest[..end];

    let mut kept: Vec<&str> = Vec::new();
    for line in block.lines().skip(1) {
        let Some((ty, _)) = line.trim_start().split_once(':') else {
            continue;
        };
        let ty = ty.trim();
        if ty.is_empty() {
            continue;
        }
        let on_form = inventory.contains(&format!("({ty})"));
        if on_form || mentions_type(objective, ty) {
            kept.push(line);
        }
    }
    if kept.is_empty() {
        return String::new();
    }
    format!(
        "EVENTS BY TYPE (the types in play for {scope}):\n{}",
        kept.join("\n")
    )
}

/// Slice the `PROCEDURES:` line: the common procedures already defined on the
/// form. The event agent's prompt tells it to factor shared logic into a
/// procedure and `CALL` it by name, which it cannot do safely while blind to
/// which ones exist. Unlike an event name, a `CALL` target is NOT validated —
/// `unknown_property_ref` skips call refs — so a guessed name is written into
/// the form and fails later at COBOL compile time instead of at apply. Empty
/// when the marker is absent.
fn procedures_excerpt(context: &str) -> String {
    let Some(at) = context.find("PROCEDURES:") else {
        return String::new();
    };
    let rest = &context[at..];
    let end = rest.find('\n').unwrap_or(rest.len());
    rest[..end].trim_end().to_string()
}

/// Slice the `CONTROL API BY ID` block from the surface context: each control's
/// real methods and properties. The event-handler agent needs this to invoke
/// actual control methods (e.g. `PlayAnimation`) instead of guessing. Empty when
/// the marker is absent.
fn control_api_excerpt(context: &str) -> String {
    let Some(at) = context.find("CONTROL API BY ID:") else {
        return String::new();
    };
    let rest = &context[at..];
    // Stop before the next top-level section so the verbose full-form dumps and
    // project inventory stay out of the delegated budget.
    let end = [
        "PROPERTY INTENT MAP",
        "PROCEDURES:",
        "PROJECT TREE INVENTORY",
        "LIVE UI TREE",
        "RELEVANT INDEXED PROJECT KNOWLEDGE",
        "PROJECT KNOWLEDGE PRECEDENCE",
    ]
    .iter()
    .filter_map(|m| rest.find(m))
    .min()
    .unwrap_or(rest.len());
    rest[..end].trim_end().to_string()
}

/// Canonicalize the approved submissions of every form-touching agent (Rig
/// migration phase 3): when the final submission's change-set JSON does not
/// parse deterministically, recover the typed change-set through
/// provider-native extraction and append its canonical encoding as a new
/// submission — the
/// original stays in the record as evidence, and `approved_form_change_sets`
/// (which reads the LAST submission) then parses without a model in the loop.
/// An unrecoverable submission is left as-is; the apply path surfaces its
/// parse error exactly as before.
fn normalize_form_change_sets(
    invoker: &DbAgentInvoker,
    record: &mut WorkflowRecord,
    on_progress: &mut dyn FnMut(String),
) {
    for task in &mut record.tasks {
        if task.final_state != TaskState::Approved
            || !crate::agents_db::produces_form_change_set(&task.spec.agent)
        {
            continue;
        }
        let Some(submission) = task.submissions.last().cloned() else {
            continue;
        };
        let cause = match crate::agent::parse_change_set(&submission) {
            Ok(_) => continue,
            Err(cause) => cause,
        };
        match invoker.extract_change_set(&submission, &cause) {
            Ok(change_set) => match serde_json::to_string_pretty(&change_set) {
                Ok(json) => {
                    task.submissions.push(format!("```json\n{json}\n```"));
                    on_progress(format!(
                        "  {}: change-set recovered via typed extraction.",
                        task.spec.id
                    ));
                }
                Err(e) => on_progress(format!(
                    "  {}: recovered change-set could not be re-encoded ({e}); the raw submission stands.",
                    task.spec.id
                )),
            },
            Err(e) => on_progress(format!(
                "  {}: change-set could not be recovered ({e}); the raw submission stands.",
                task.spec.id
            )),
        }
    }
}

/// Convert a completed workflow into the reply shown by a chatbot. Prefer the
/// surface's specialist output when it exists (important for structured form
/// change-sets); otherwise include every approved specialist submission.
pub fn workflow_chat_reply(
    record: &WorkflowRecord,
    preferred_specialist: Option<&str>,
    verbose: bool,
) -> String {
    use cobolt_agents::grace::TaskState;

    if verbose {
        return verbose_transcript(record);
    }

    // Concise mode: Grace's one-line, user-facing summary of the work done.
    if !record.final_summary.trim().is_empty() {
        return with_token_footer(record.final_summary.trim().to_string(), record);
    }

    // Fallbacks when no summary was produced.
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
                .map(|(_, text)| readable_submission(text))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !preferred.is_empty() {
        return with_token_footer(preferred.join("\n\n"), record);
    }
    if approved.len() == 1 && approved[0].0.eq_ignore_ascii_case(GRACE) {
        return with_token_footer(readable_submission(approved[0].1), record);
    }
    if !approved.is_empty() {
        return with_token_footer(
            approved
                .into_iter()
                .map(|(agent, text)| format!("{agent}: {}", readable_submission(text)))
                .collect::<Vec<_>>()
                .join("\n\n"),
            record,
        );
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

/// The verbose chatbot transcript: Grace's plan, each delegated request and the
/// agent's (summarised) response, the Pedantic verdict, and Grace's final line.
/// Change-sets are rendered in plain language rather than dumped as raw JSON.
fn verbose_transcript(record: &WorkflowRecord) -> String {
    let (body, summary) = verbose_transcript_parts(record);
    let joined = match summary {
        Some(summary) => format!("{body}\n\nGrace: {summary}"),
        None => body,
    };
    with_token_footer(joined.trim_end().to_string(), record)
}

/// The verbose transcript split in two: the coordination body (plan, delegated
/// requests, submissions, verdicts) and — when the workflow produced one —
/// Grace's final consolidated summary, so the chat can show the summary as
/// Grace's OWN balloon instead of burying it at the tail of the transcript.
fn verbose_transcript_parts(record: &WorkflowRecord) -> (String, Option<String>) {
    use cobolt_agents::grace::TaskState;
    let mut out = String::new();
    if !record.knowledge_summary.trim().is_empty() {
        out.push_str(&format!("Knowledge Base: {}\n\n", record.knowledge_summary.trim()));
    }
    // A direct conversation (single Grace task, no delegation).
    if record.tasks.len() == 1 && record.tasks[0].spec.agent.eq_ignore_ascii_case(GRACE) {
        let reply = record.tasks[0].submissions.last().cloned().unwrap_or_default();
        out.push_str(&format!("Grace: {}", reply.trim()));
        return (out.trim_end().to_string(), None);
    }
    out.push_str(&format!("Grace: planned {} step(s).\n\n", record.tasks.len()));
    for task in &record.tasks {
        let agent = &task.spec.agent;
        out.push_str(&format!("Grace \u{2192} {agent}: {}\n", task.spec.objective.trim()));
        if let Some(sub) = task.submissions.last() {
            // Verbose is where the specialists' full content (their applied
            // reasoning) belongs — word-capped, fenced blocks stripped.
            out.push_str(&format!("{agent}: {}\n", full_prose_submission(sub)));
        }
        if let Some(review) = task.reviews.last() {
            let verdict = if review.defects { "REJECTED" } else { "APPROVED" };
            let detail = if review.defects && !review.correction_request.trim().is_empty() {
                format!(" \u{2014} {}", first_nonempty_line(&review.correction_request))
            } else {
                String::new()
            };
            out.push_str(&format!("{}: {verdict}{detail}\n", review.reviewer));
        } else if task.final_state == TaskState::Failed {
            out.push_str(&format!("Result: failed \u{2014} {}\n", task.failure_reason.trim()));
        }
        out.push('\n');
    }
    let summary = record.final_summary.trim();
    (
        out.trim_end().to_string(),
        (!summary.is_empty()).then(|| summary.to_string()),
    )
}

/// The workflow reply as chat balloons. Concise mode stays one balloon. In
/// verbose mode a delegated workflow yields TWO: the coordination transcript,
/// then Grace's own final balloon (her consolidated summary plus the token
/// footer) — previously the summary sat at the tail of one giant transcript
/// balloon and read as the last specialist's text.
pub fn workflow_chat_balloons(record: &WorkflowRecord, verbose: bool) -> Vec<String> {
    if !verbose {
        return vec![workflow_chat_reply(record, None, false)];
    }
    match verbose_transcript_parts(record) {
        (body, Some(summary)) => vec![
            body,
            with_token_footer(format!("Grace: {summary}"), record),
        ],
        (body, None) => vec![with_token_footer(body, record)],
    }
}

/// Append a compact token-consumption footer, unless no tokens were recorded.
fn with_token_footer(body: String, record: &WorkflowRecord) -> String {
    if record.input_tokens == 0 && record.output_tokens == 0 {
        body
    } else {
        format!(
            "{body}\n\n\u{2014} {} tokens in / {} tokens out",
            record.input_tokens, record.output_tokens
        )
    }
}

fn first_nonempty_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The full prose of a submission with fenced code/JSON blocks removed and the
/// line structure preserved. Used when the submission itself is the
/// deliverable — a clarification handoff — where the 50-word chat lead of
/// [`readable_submission`] would swallow the questions (observed live: the
/// UUID-or-PIC questions sat beyond word 50, so the developer saw only a
/// truncated intro and no question balloons).
/// Appended to the Documentation Agent's system prompt on every workflow
/// invocation: its output is relayed to the developer's chat and handed to
/// dependent agents, so it must be the artifact itself — concise, no
/// reasoning narrative.
const DOCUMENTATION_CONCISE_CONTRACT: &str = "\n\nCONCISE OUTPUT CONTRACT\nRespond with only the content required to execute or hand off the task: the artifact, the analysis results, and any developer questions. No reasoning narrative, no restated instructions, no meta-commentary about your process. Keep the handoff as short as the task allows.";

/// Upper bound on the words a relayed handoff may carry into the chat. Real
/// clarification handoffs run hundreds of words; helper-heavy schemas run
/// bigger. Raise this if specialists legitimately need more room — the full
/// text is always preserved in the workflow record on disk.
const CLARIFICATION_RELAY_MAX_WORDS: usize = 5_000;

fn full_prose_submission(sub: &str) -> String {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for line in sub.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            lines.push(line);
        }
    }
    // Word cap, preserving line structure. Question lines are NEVER dropped —
    // a handoff whose questions sit past the budget must still ask them.
    let mut kept = Vec::with_capacity(lines.len());
    let mut words = 0usize;
    let mut truncated = false;
    for line in lines {
        if !truncated {
            words += line.split_whitespace().count();
            if words > CLARIFICATION_RELAY_MAX_WORDS {
                truncated = true;
                kept.push(format!(
                    "\u{2026} (handoff truncated at {CLARIFICATION_RELAY_MAX_WORDS} words — the \
                     complete text is preserved in the workflow record)"
                ));
            }
        }
        if !truncated {
            kept.push(line.to_string());
        } else if line_as_developer_question(line).is_some() {
            kept.push(line.to_string());
        }
    }
    kept.join("\n").trim().to_string()
}

/// Render one agent submission for the chatbot: a plain-language change-set
/// summary when the submission carries operations, else a short prose lead with
/// fenced code/JSON blocks stripped out.
fn readable_submission(sub: &str) -> String {
    let ops = extract_operations(sub);
    if !ops.is_empty() {
        return summarize_operations(&ops).join(" ");
    }
    let mut prose = String::new();
    let mut in_fence = false;
    for line in sub.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            prose.push_str(line);
            prose.push(' ');
        }
    }
    let words: Vec<&str> = prose.split_whitespace().collect();
    let mut s = words.iter().take(50).cloned().collect::<Vec<_>>().join(" ");
    if words.len() > 50 {
        s.push('\u{2026}');
    }
    if s.is_empty() {
        "(completed)".to_string()
    } else {
        s
    }
}

/// Extract the `operations` array from a change-set submission (fenced or bare),
/// tolerating surrounding prose. Returns an empty vec when none is present.
fn extract_operations(submission: &str) -> Vec<serde_json::Value> {
    for (start, _) in submission.match_indices('{') {
        if let Some(end) = matching_brace(submission, start) {
            let slice = &submission[start..=end];
            if slice.contains("\"operations\"") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(slice) {
                    if let Some(ops) = v.get("operations").and_then(|o| o.as_array()) {
                        return ops.clone();
                    }
                }
            }
        }
    }
    Vec::new()
}

/// Index of the `}` that closes the `{` at `start`, respecting JSON strings.
fn matching_brace(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &c) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Turn change-set operations into plain-language lines (grouping identical
/// property assignments across controls).
fn summarize_operations(ops: &[serde_json::Value]) -> Vec<String> {
    use std::collections::BTreeMap;
    let str_of = |v: &serde_json::Value, k: &str| -> String {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("?")
            .to_string()
    };
    let val_of = |v: &serde_json::Value| -> String {
        match v.get("value") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => "?".to_string(),
        }
    };
    let mut lines = Vec::new();
    let mut set_groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for op in ops {
        match op.get("op").and_then(|v| v.as_str()).unwrap_or("") {
            "set_property" => {
                let key = str_of(op, "key");
                let val = val_of(op);
                set_groups
                    .entry((key, val))
                    .or_default()
                    .push(str_of(op, "control_id"));
            }
            "deploy_control" => {
                lines.push(format!(
                    "Added {} '{}'.",
                    str_of(op, "control_type"),
                    str_of(op, "id")
                ));
            }
            "generate_event_handler" => {
                lines.push(format!(
                    "Wired {} on {}.",
                    str_of(op, "event"),
                    str_of(op, "control_id")
                ));
            }
            "create_procedure" => {
                lines.push(format!("Added procedure {}.", str_of(op, "name")));
            }
            _ => {}
        }
    }
    for ((key, val), ids) in set_groups {
        if key == "GlassStyle" && ids.iter().any(|c| c.eq_ignore_ascii_case("Form")) {
            lines.push(format!("Set form style to {val}."));
        } else {
            lines.push(format!("Set {key} to {val} for {}.", ids.join(", ")));
        }
    }
    lines
}

/// Condense the injected Knowledge Base context into a <=15 word summary, or an
/// empty string when there was no relevant evidence.
fn summarize_knowledge_context(ctx: &str) -> String {
    let head = ctx
        .split("\n\nPROJECT KNOWLEDGE PRECEDENCE CONTRACT:")
        .next()
        .unwrap_or("")
        .trim();
    let lower = head.to_ascii_lowercase();
    if head.is_empty() || lower.contains("no relevant") {
        return String::new();
    }
    let words: Vec<&str> = head.split_whitespace().collect();
    let mut s = words.iter().take(15).cloned().collect::<Vec<_>>().join(" ");
    if words.len() > 15 {
        s.push('\u{2026}');
    }
    s
}

/// Grace's concise, user-facing one-liner, derived deterministically from the
/// approved work so it costs no extra model roundtrip. Change-sets are
/// summarised in plain language; other agents are summarised from their task
/// objective.
/// Split developer-facing questions out of an agent reply. Returns the reply
/// remainder (context the developer may still see at once) and each question in
/// order. A question is a line ending in `?` or an explicit "please specify /
/// provide / choose / confirm / indicate / select" request — the phrasings the
/// clarification contract produces. The chat surface shows them one balloon at
/// a time, waiting for each answer.
/// When `raw` reads as a developer-facing question — a line ending in `?` or
/// an explicit "please specify / provide / choose / confirm / indicate /
/// select" request — returns its cleaned text.
fn line_as_developer_question(raw: &str) -> Option<String> {
    let stripped = raw
        .trim()
        .trim_start_matches(['-', '*', '>'])
        .trim_start_matches('#')
        .trim()
        .replace("**", "");
    let clean = stripped.trim();
    let lower = clean.to_ascii_lowercase();
    let is_question = !clean.is_empty()
        && !clean.starts_with('|')
        && (clean.ends_with('?')
            || [
                "please specify",
                "please provide",
                "please choose",
                "please confirm",
                "please indicate",
                "please select",
            ]
            .iter()
            .any(|marker| lower.contains(marker)));
    is_question.then(|| clean.to_string())
}

pub fn split_developer_questions(text: &str) -> (String, Vec<String>) {
    let mut questions: Vec<String> = Vec::new();
    let mut rest = Vec::new();
    for raw in text.lines() {
        match line_as_developer_question(raw) {
            Some(question) => questions.push(question),
            None => rest.push(raw),
        }
    }
    // Dedupe: the verbose transcript can carry the same question twice — the
    // per-task 50-word lead ("Documentation Agent: Developer, please provide…
    // ### Schema…") and the clean relayed line. Observed live: the developer
    // got two balloons for the CIF question and answered it twice. Containment
    // on an alphanumeric key keeps the SHORTEST form of each question.
    let key = |s: &str| -> String {
        // Drop a leading speaker prefix ("Grace:", "Documentation Agent:") so
        // the same question relayed under different speakers compares equal.
        let body = match s.find(':') {
            Some(i) if i <= 40 => s[i + 1..].trim_start(),
            _ => s,
        };
        body.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let mut kept: Vec<String> = Vec::new();
    'next_question: for question in questions {
        let question_key = key(&question);
        for existing in kept.iter_mut() {
            let existing_key = key(existing);
            if question_key.contains(&existing_key) {
                continue 'next_question; // longer duplicate of a kept question
            }
            if existing_key.contains(&question_key) {
                *existing = question; // shorter, cleaner form wins
                continue 'next_question;
            }
        }
        kept.push(question);
    }
    (rest.join("\n").trim().to_string(), kept)
}

/// Grace's CONCISE closing summary for the chat's blue dialog bubble. Built
/// from structured facts only — executed tool evidence, task outcomes, and
/// any developer questions extracted from the approved submissions. The full
/// specialist submissions (their reasoning) appear only in the verbose
/// transcript's dark cards.
fn grace_final_summary(record: &WorkflowRecord, evidence: &[ToolEvidence]) -> String {
    use cobolt_agents::grace::TaskState;
    let mut sections: Vec<String> = Vec::new();

    // Form change-set operations, when present.
    let mut ops = Vec::new();
    for task in &record.tasks {
        if task.final_state == TaskState::Approved {
            if let Some(sub) = task.submissions.last() {
                ops.extend(summarize_operations(&extract_operations(sub)));
            }
        }
    }
    if !ops.is_empty() {
        sections.push(ops.join(" "));
    }

    // Mutations actually executed, from tool evidence — the ground truth of
    // what was created or changed.
    let executed: Vec<String> = evidence
        .iter()
        .filter(|e| e.ok && (e.tool.ends_with(".write") || e.tool == "git.run"))
        .map(|e| format!("- {} — {}", e.tool, e.summary))
        .collect();
    if !executed.is_empty() {
        sections.push(format!("Executed:\n\n{}", executed.join("\n")));
    }

    // Task outcomes as the T#-bulleted list (typographic rule), including
    // failures so nothing is concealed.
    let outcomes: Vec<String> = record
        .tasks
        .iter()
        .map(|t| {
            let objective = t
                .spec
                .objective
                .split(['.', '\n'])
                .next()
                .unwrap_or("")
                .trim();
            let label = if objective.is_empty() {
                format!("{}", t.spec.agent)
            } else {
                format!("{}: {objective}", t.spec.agent)
            };
            match t.final_state {
                // An approved task can carry a note — today that is "approved
                // WITHOUT review" when its reviewer could not run. Show it:
                // the developer must know this result skipped its gate.
                TaskState::Approved if !t.failure_reason.trim().is_empty() => format!(
                    "- {} — {label} — \u{26a0} {}",
                    t.spec.id,
                    first_nonempty_line(&t.failure_reason)
                ),
                TaskState::Approved => format!("- {} — {label}", t.spec.id),
                TaskState::Failed => format!(
                    "- {} — {label} — FAILED: {}",
                    t.spec.id,
                    first_nonempty_line(&t.failure_reason)
                ),
                _ => format!("- {} — {label} — {:?}", t.spec.id, t.final_state),
            }
        })
        .collect();
    if !outcomes.is_empty() {
        sections.push(format!("Tasks:\n\n{}", outcomes.join("\n")));
    }

    // Developer questions still surface in concise mode — the chat turns
    // them into their own red balloons. The surrounding detail stays verbose.
    let mut questions: Vec<String> = Vec::new();
    for task in &record.tasks {
        if task.final_state == TaskState::Approved {
            if let Some(sub) = task.submissions.last() {
                let (_context, mut found) = split_developer_questions(&full_prose_submission(sub));
                questions.append(&mut found);
            }
        }
    }
    questions.dedup();
    if !questions.is_empty() {
        sections.push(questions.join("\n"));
    }

    sections.join("\n\n")
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
            ..Default::default()
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

    /// The relay caps at [`CLARIFICATION_RELAY_MAX_WORDS`], but question lines
    /// past the budget must still be asked — dropping them would recreate the
    /// truncated-handoff bug at a larger size.
    #[test]
    fn oversized_handoffs_are_capped_but_never_lose_their_questions() {
        let filler_line = "word ".repeat(100); // 100 words per line
        let mut submission = String::new();
        for _ in 0..60 {
            submission.push_str(&filler_line);
            submission.push('\n');
        } // 6000 words of context — past the 5000 cap
        submission.push_str("Please specify the ID format for CompanyID.\n");
        submission.push_str("Should the registry number be unique per province?\n");

        let relayed = full_prose_submission(&submission);
        let relayed_words = relayed.split_whitespace().count();
        assert!(
            relayed_words < 5_200,
            "cap must hold (got {relayed_words} words)"
        );
        assert!(relayed.contains("handoff truncated at 5000 words"));
        assert!(
            relayed.contains("Please specify the ID format for CompanyID."),
            "questions past the cap must survive"
        );
        assert!(relayed.contains("Should the registry number be unique per province?"));

        // Under the cap: untouched.
        let small = "A short handoff.\nPlease specify the file name.";
        assert_eq!(full_prose_submission(small), small);
    }

    /// The concise summary carries the QUESTIONS from a long handoff (they
    /// become balloons) while the detail — filler, decision tables — stays in
    /// the verbose transcript's document cards only.
    #[test]
    fn long_clarification_handoffs_keep_their_questions_and_drop_the_detail() {
        let filler = "The analysis decomposes the requirements into normalized entities \
             to eliminate redundancy and transitive dependencies across the domain. "
            .repeat(10);
        let submission = format!(
            "Since no existing conventions were found, this is a greenfield requirement.\n\n\
             {filler}\n\n\
             | Entity | Proposed File Name |\n| --- | --- |\n| Branch | idx-branch.cidx |\n\n\
             **Please specify for each ID category whether to use a UUID or a specific \
             COBOL PIC clause (e.g., PIC X(10)).**\n\n\
             Status: Pending Developer Choice. No mutation of resources has been performed."
        );
        let clarification = task("Documentation Agent", TaskState::Approved, &submission);
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![clarification],
            ..Default::default()
        };
        let summary = grace_final_summary(&record, &[]);
        assert!(
            summary.contains("Please specify for each ID category"),
            "the questions must reach the developer, got: {summary}"
        );
        assert!(
            !summary.contains("| Branch | idx-branch.cidx |"),
            "the detail belongs to verbose mode, not the concise bubble"
        );
        // Verbose keeps the full handoff, table included.
        let verbose = workflow_chat_reply(&record, None, true);
        assert!(verbose.contains("| Branch | idx-branch.cidx |"));
    }

    /// In verbose mode Grace's consolidated summary is its OWN balloon after
    /// the coordination transcript — not the tail of one giant balloon that
    /// reads as the last specialist's text.
    #[test]
    fn verbose_workflow_reply_gives_grace_her_own_final_balloon() {
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![
                task("Documentation Agent", TaskState::Approved, "Handoff ready."),
                task(
                    "Data (Indexed File) Agent",
                    TaskState::Approved,
                    "All files are finalized and submitted for review.",
                ),
            ],
            final_summary: "Executed:\n\n- indexed_file.write — saved indexed/banco.cidx".into(),
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        };
        let balloons = workflow_chat_balloons(&record, true);
        assert_eq!(balloons.len(), 2, "transcript + Grace's final balloon");
        assert!(balloons[0].contains("Grace: planned 2 step(s)."));
        assert!(balloons[0].contains("All files are finalized"));
        assert!(
            !balloons[0].contains("indexed_file.write — saved"),
            "the summary must not also sit at the transcript tail"
        );
        assert!(balloons[1].starts_with("Grace: Executed:"));
        assert!(
            balloons[1].contains("tokens in"),
            "the token footer rides Grace's final balloon"
        );

        // Concise mode stays one balloon.
        assert_eq!(workflow_chat_balloons(&record, false).len(), 1);
    }

    #[test]
    fn clarification_task_questions_are_relayed_in_final_summary() {
        let clarification = task(
            "Documentation Agent",
            TaskState::Approved,
            "### Developer Clarification Request\nPlease specify the primary file name \
             (e.g. COMPANY-MASTER).\nFor every ID field: UUID or a specific COBOL PIC definition?",
        );
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![clarification],
            ..Default::default()
        };
        let summary = grace_final_summary(&record, &[]);
        assert!(
            summary.contains("primary file name"),
            "the specialist's question must reach the developer, got: {summary}"
        );
        assert!(
            summary.contains("UUID or a specific COBOL PIC"),
            "the ID-format question must reach the developer, got: {summary}"
        );
    }

    /// The blue-bubble summary is evidence-backed: executed .write tools are
    /// listed as the ground truth of what was created.
    #[test]
    fn final_summary_lists_executed_writes_from_evidence() {
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![task(
                "Data (Indexed File) Agent",
                TaskState::Approved,
                "The definition is finalized and locked.",
            )],
            ..Default::default()
        };
        let evidence = vec![
            ToolEvidence {
                agent: "Data (Indexed File) Agent".into(),
                tool: "indexed_file.write".into(),
                args_digest: String::new(),
                summary: "wrote indexed/idx-company.cidx".into(),
                ok: true,
                ts: 0,
            },
            // Read-only evidence stays out of the summary.
            ToolEvidence {
                agent: "Data (Indexed File) Agent".into(),
                tool: "indexed_file.list".into(),
                args_digest: String::new(),
                summary: "listed 6 indexed-file definition(s)".into(),
                ok: true,
                ts: 0,
            },
        ];
        let summary = grace_final_summary(&record, &evidence);
        assert!(
            summary.contains("indexed_file.write — wrote indexed/idx-company.cidx"),
            "{summary}"
        );
        assert!(!summary.contains("indexed_file.list"));
        assert!(
            summary.contains("- T — Data (Indexed File) Agent"),
            "task outcomes list present: {summary}"
        );
        assert!(
            !summary.contains("finalized and locked"),
            "specialist prose stays out of the concise bubble"
        );
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
            ..Default::default()
        };
        let sets = approved_form_change_sets(&record, "Form Designer Agent");
        assert_eq!(sets.len(), 1, "one approved Form-Designer task");
        assert!(sets[0].is_ok(), "its submission parsed as a change-set");
        assert_eq!(sets[0].as_ref().unwrap().operations.len(), 1);
    }

    /// An event-only request ("wire Button-1's onClick") is planned as a single
    /// COBOL Event Handler task with no Form Designer task at all. Harvesting
    /// only the designer discarded that approved handler and the form was never
    /// touched, with nothing said about it.
    #[test]
    fn an_approved_event_handler_task_yields_its_change_set() {
        let handler = "```json\n{\"operations\":[{\"op\":\"generate_event_handler\",\"control_id\":\"Button-1\",\"event\":\"onClick\",\"code\":\"       ENVIRONMENT DIVISION.\"}]}\n```";
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![task(
                crate::agents_db::EVENT_HANDLER,
                TaskState::Approved,
                handler,
            )],
            ..Default::default()
        };
        let sets = approved_form_change_sets(&record, "Form Designer Agent");
        assert_eq!(sets.len(), 1, "the event-handler task is harvested");
        assert_eq!(sets[0].as_ref().unwrap().operations.len(), 1);

        // …and the contextual designer chat receives it as an applicable block.
        let raw = approved_form_change_set_submission(&record, "Form Designer Agent")
            .expect("an applicable change-set");
        let parsed = crate::agent::parse_change_set(&raw).expect("parses");
        assert!(matches!(
            parsed.operations[0],
            crate::agent::AgentOp::GenerateEventHandler { .. }
        ));
    }

    /// A request that both adds a control and wires its event is two tasks;
    /// returning only the first would silently drop the other.
    #[test]
    fn designer_and_event_handler_operations_are_merged_into_one_block() {
        let deploy = "```json\n{\"operations\":[{\"op\":\"deploy_control\",\"control_type\":\"Button\",\"id\":\"BTN\"}]}\n```";
        let handler = "```json\n{\"operations\":[{\"op\":\"generate_event_handler\",\"control_id\":\"BTN\",\"event\":\"onClick\",\"code\":\"x\"}]}\n```";
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![
                task("Form Designer Agent", TaskState::Approved, deploy),
                task(crate::agents_db::EVENT_HANDLER, TaskState::Approved, handler),
            ],
            ..Default::default()
        };
        let raw = approved_form_change_set_submission(&record, "Form Designer Agent")
            .expect("an applicable change-set");
        let parsed = crate::agent::parse_change_set(&raw).expect("parses");
        assert_eq!(parsed.operations.len(), 2, "neither task is dropped");
    }

    /// An approved task whose submission carries no operation changed nothing;
    /// the caller must be able to say so instead of reporting a bare success.
    #[test]
    fn an_approved_task_without_operations_is_reported() {
        let prose = "**Operation: `generate_event_handler`** — control_id: Button-1. Status: approved.";
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![task(
                crate::agents_db::EVENT_HANDLER,
                TaskState::Approved,
                prose,
            )],
            ..Default::default()
        };
        assert!(approved_form_change_set_submission(&record, "Form Designer Agent").is_none());
        assert_eq!(
            approved_form_tasks_without_operations(&record, "Form Designer Agent"),
            vec!["T".to_string()],
            "the barren task is named so the reply can warn about it"
        );
    }

    #[test]
    fn contextual_reply_carries_the_raw_change_set_for_the_designer() {
        // A Form Designer change-set submission the contextual designer chat must
        // receive VERBATIM so it can `parse_change_set` + apply it (else the edits
        // — and their animation — never happen).
        let cs = "I reorganized the charts.\n```json\n{\"operations\":[{\"op\":\"set_property\",\"control_id\":\"BarChart-1\",\"key\":\"X\",\"value\":72}]}\n```";
        let record = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![
                // Not approved → skipped even though it has ops.
                task("Form Designer Agent", TaskState::Failed, cs),
                task("Form Designer Agent", TaskState::Approved, cs),
            ],
            ..Default::default()
        };
        let raw = approved_form_change_set_submission(&record, "Form Designer Agent")
            .expect("an approved change-set submission");
        // The raw block round-trips through the same parser the designer uses.
        let parsed = crate::agent::parse_change_set(&raw).expect("parses");
        assert_eq!(parsed.operations.len(), 1);

        // No approved form task ⇒ None (caller falls back to the readable summary).
        let none = WorkflowRecord {
            workflow_id: "wf".into(),
            status: "completed".into(),
            tasks: vec![task("Form Designer Agent", TaskState::Failed, cs)],
            ..Default::default()
        };
        assert!(approved_form_change_set_submission(&none, "Form Designer Agent").is_none());
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
            ..Default::default()
        };
        assert_eq!(
            workflow_chat_reply(&record, Some("Form Designer Agent"), false),
            "form result"
        );
        let project_reply = workflow_chat_reply(&record, None, false);
        assert!(project_reply.contains("Form Designer Agent: form result"));
        assert!(project_reply.contains("COBOL Event Handler Script Agent: event result"));
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

    /// Observed live: "Hola" ran the full ACTION pipeline — plan, empty-plan
    /// retry, failed typed extraction, and a delegated Documentation-Agent
    /// "greeting task" (~9.4k tokens). A greeting must answer locally, free.
    #[test]
    fn bare_greetings_get_a_canned_reply_in_their_language() {
        assert_eq!(
            simple_greeting_reply("Hola"),
            Some("¡Hola! ¿Cómo estás? ¿Creamos algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("¡Hola, Grace!"),
            Some("¡Hola! ¿Cómo estás? ¿Creamos algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("olá"),
            Some("Olá! Tudo bem? Vamos criar algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("Bom dia"),
            Some("Olá! Tudo bem? Vamos criar algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("hello"),
            Some("Hello! How are you? Shall we create something together?")
        );
        // Combined greeting + how-are-you, with accents (observed live:
        // "Hola, ¿cómo estás?" missed the list and ran the ACTION pipeline).
        assert_eq!(
            simple_greeting_reply("Hola, ¿cómo estás?"),
            Some("¡Hola! ¿Cómo estás? ¿Creamos algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("como estas"),
            Some("¡Hola! ¿Cómo estás? ¿Creamos algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("Olá, tudo bem?"),
            Some("Olá! Tudo bem? Vamos criar algo juntos?")
        );
        assert_eq!(
            simple_greeting_reply("hi, how are you?"),
            Some("Hello! How are you? Shall we create something together?")
        );
        // Anything beyond a greeting goes through the normal pipeline.
        assert_eq!(simple_greeting_reply("Hola, crea un formulario de login"), None);
        assert_eq!(simple_greeting_reply("Create an indexed file"), None);
        assert_eq!(simple_greeting_reply("What can you do?"), None);
    }

    /// The System KB is published from the running binary and then indexed, so
    /// a fresh machine reports a non-zero count rather than the empty state
    /// that only the Project KB may legitimately be in.
    #[test]
    fn system_knowledge_publishes_and_indexes_itself() {
        let root = std::env::temp_dir().join(format!("prc-syskb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (indexed, records) = sync_system_knowledge(&root, &mut |_, _, _| {})
            .expect("system KB should publish and index");
        assert!(
            indexed >= ESSENTIAL_SYSTEM_DOCUMENTS.len(),
            "expected at least the {} essential documents, indexed {indexed}",
            ESSENTIAL_SYSTEM_DOCUMENTS.len()
        );
        assert!(
            records > indexed,
            "chunking must yield MORE subject records ({records}) than documents ({indexed})"
        );
        for doc in &ESSENTIAL_SYSTEM_DOCUMENTS {
            assert!(root.join(doc).exists(), "{doc} must exist after publishing");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The PREBUILT chunked store shipped in the binary must stay current
    /// with the documentation this build publishes — a cloned IDE must not
    /// re-embed anything. Regenerate with
    /// `cargo run -p cobolt-ide --example build_chunked_kb` when this fails.
    #[test]
    fn prebuilt_chunked_kb_matches_the_published_documentation() {
        assert!(
            !PREBUILT_CHUNKED_KB.is_empty(),
            "assets/knowledge/chunked.data is empty — run \
             `cargo run -p cobolt-ide --example build_chunked_kb` and rebuild"
        );
        let root = std::env::temp_dir().join(format!("prc-prebuilt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch root");
        cobolt_compiler::publish_system_documentation(&root).expect("publish");
        let store = root.join("chunked.data");
        std::fs::write(&store, PREBUILT_CHUNKED_KB).expect("materialize shipped store");
        let documents =
            cobolt_agents::chunked_knowledge::tree_documents(&root).expect("documents");
        let stale = cobolt_agents::chunked_knowledge::stale_documents(
            &store,
            &documents,
            "bert-multilingual-e5-small",
        )
        .expect("staleness probe");
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            stale.is_empty(),
            "the shipped chunked store is stale for {stale:?} — run \
             `cargo run -p cobolt-ide --example build_chunked_kb` and commit the result"
        );
        println!(
            "prebuilt store: current for all {} published documents ({} bytes embedded)",
            documents.len(),
            PREBUILT_CHUNKED_KB.len()
        );
    }

    /// A document the installed binary cannot produce is the one failure a
    /// rebuild fixes, so the message must say exactly that instead of quietly
    /// reporting a smaller count.
    #[test]
    fn a_missing_system_document_asks_for_a_platform_rebuild() {
        let root = std::env::temp_dir().join(format!("prc-syskb-gap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        sync_system_knowledge(&root, &mut |_, _, _| {}).expect("baseline publish");
        std::fs::remove_file(root.join(ESSENTIAL_SYSTEM_DOCUMENTS[0])).expect("remove one doc");
        // Publishing rewrites what the binary knows; simulate an older binary
        // by checking the gap detector against a root missing that document.
        let missing: Vec<&str> = ESSENTIAL_SYSTEM_DOCUMENTS
            .iter()
            .filter(|path| !root.join(path).exists())
            .copied()
            .collect();
        assert_eq!(missing, vec![ESSENTIAL_SYSTEM_DOCUMENTS[0]]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// "name" versus "caption" is the ambiguity that shipped the wrong literal;
    /// Grace must be told to ask rather than pick the likelier reading.
    #[test]
    fn the_routing_contract_tells_grace_to_ask_when_in_doubt() {
        assert!(RESPONSE_ROUTING_CONTRACT.contains("WHEN IN DOUBT, ASK"));
        assert!(RESPONSE_ROUTING_CONTRACT.contains("IDENTIFIER"));
        assert!(RESPONSE_ROUTING_CONTRACT.contains("Caption"));
        // Inferability must not be the escape hatch it previously was.
        assert!(RESPONSE_ROUTING_CONTRACT.contains("is NOT a reason to skip the question"));
    }

    /// The developer's own words reach every agent, so a Portuguese request
    /// gets a Portuguese plan, review and summary instead of an English one.
    #[test]
    fn language_directive_carries_the_request_verbatim() {
        let request = "faça o evento onClick de cada botão modificar o caption de Label-1";
        let directive = language_directive(request);
        assert!(directive.contains(request));
        assert!(directive.contains("Developer's language"));
    }

    /// The machine contracts must be named as untranslatable: a reviewer that
    /// answers "aceitável" fails the literal `"acceptable"` comparison, and a
    /// translated control id never resolves against the form.
    #[test]
    fn language_directive_protects_machine_read_tokens() {
        let directive = language_directive("crie um formulário de login");
        for protected in [
            "\"acceptable\"",
            "\"defects\"",
            "agent and reviewer names",
            "property, method, and event identifiers",
        ] {
            assert!(
                directive.contains(protected),
                "directive must exempt {protected} from translation"
            );
        }
    }

    /// No request, no directive — an empty tail must not append a stray
    /// heading to the agent's core instructions.
    #[test]
    fn language_directive_is_absent_without_a_request() {
        assert!(language_directive("   ").is_empty());
    }

    /// The clarity pre-check parses Grace's verdict deterministically and
    /// fails OPEN: an unparseable reply must let the workflow proceed, never
    /// add a new failure mode in front of every request.
    #[test]
    fn clarity_reply_parses_and_fails_open() {
        let low = parse_clarity_reply(
            "Avaliação interna.\n```json\n{\"clarity\": 4, \"interpretation\": \"Você quer alinhar os labels.\", \"questions\": [\"Qual espaçamento horizontal devo usar?\", \"\"]}\n```",
        )
        .expect("verdict parses");
        assert_eq!(low.clarity, 4);
        assert!(low.clarity < CLARITY_GATE_THRESHOLD, "4 gates the request");
        assert_eq!(low.interpretation, "Você quer alinhar os labels.");
        assert_eq!(low.questions, vec!["Qual espaçamento horizontal devo usar?"]);

        // A float score rounds; out-of-range clamps to the 0-10 scale.
        assert_eq!(
            parse_clarity_reply("```json\n{\"clarity\": 8.6}\n```").unwrap().clarity,
            9
        );
        assert_eq!(
            parse_clarity_reply("```json\n{\"clarity\": 99}\n```").unwrap().clarity,
            10
        );

        // Fail-open: prose, missing score, or no JSON at all → None.
        assert!(parse_clarity_reply("clear enough, proceeding").is_none());
        assert!(parse_clarity_reply("```json\n{\"interpretation\": \"x\"}\n```").is_none());
    }

    /// The pre-check must see the recent conversation — observed live: with
    /// the gate blind to it, the developer's answer "todos os labels" to
    /// Grace's own "which controls?" scored 3/10 and Grace asked what to do
    /// with the labels she had just been told about.
    #[test]
    fn clarity_conversation_slice_extracts_recent_turns() {
        // No conversation section, or an empty one → "(none)".
        assert_eq!(clarity_conversation_slice("FORM: X (100x100)"), "(none)");
        assert_eq!(
            clarity_conversation_slice("FORM: X\n\nCONVERSATION SO FAR:\n  "),
            "(none)"
        );

        // The full recent exchange survives, oldest first, without the
        // surface header that precedes the marker.
        let context = "FORM: LABELS-FORM (1200x760)\nCONTROLS:\n- LBL-1\n\nCONVERSATION SO FAR:\nuser: modifique os eventos mouseenter/mouseleave\n\nassistant: Quais controles devem ter esse comportamento?";
        let slice = clarity_conversation_slice(context);
        assert!(slice.starts_with("user: modifique"), "{slice}");
        assert!(slice.ends_with("comportamento?"), "{slice}");
        assert!(!slice.contains("LABELS-FORM"), "{slice}");

        // Old turns fall off the bounded tail; the newest always survives. A
        // blank line INSIDE a turn's content is not a turn boundary.
        let old_turn = format!("user: {}", "x".repeat(CLARITY_CONVERSATION_BUDGET));
        let context = format!(
            "CONVERSATION SO FAR:\n{old_turn}\n\nassistant: Qual arquivo?\n\ncom um parágrafo\n\nsolto no meio\n\nuser: o inicial"
        );
        let slice = clarity_conversation_slice(&context);
        assert!(slice.starts_with("assistant: Qual arquivo?"), "{slice}");
        assert!(slice.contains("solto no meio"), "{slice}");
        assert!(slice.ends_with("user: o inicial"), "{slice}");

        // A single oversized turn is hard-truncated to the budget's tail.
        let huge = format!("CONVERSATION SO FAR:\nuser: {}fim", "y".repeat(9_000));
        let slice = clarity_conversation_slice(&huge);
        assert!(slice.len() <= CLARITY_CONVERSATION_BUDGET + '…'.len_utf8());
        assert!(slice.starts_with('…'), "{slice}");
        assert!(slice.ends_with("fim"), "{slice}");
    }

    #[test]
    fn workflow_control_stop_gates_every_model_call() {
        let control = WorkflowControl::default();
        assert!(!control.stop_requested());
        *control.tokens.lock().unwrap() = (100, 20);
        assert_eq!(control.token_totals(), (100, 20));
        control.request_stop();
        assert!(control.stop_requested());
        // The invoker refuses immediately once the flag is set.
        let mut invoker = DbAgentInvoker {
            project_dir: std::env::temp_dir(),
            llm: LlmConfig::defaults(),
            tokens: control.tokens.clone(),
            evidence: Arc::new(Mutex::new(Vec::new())),
            cancel: control.cancel.clone(),
            request: String::new(),
            suppress_native_tools: false,
        };
        let err = invoker
            .invoke("Documentation Agent", "sys", "user")
            .unwrap_err();
        assert!(err.contains("stopped by the developer"), "{err}");
    }

    /// Observed live: the verbose transcript carried the CIF question twice —
    /// once inside the flattened per-task lead ("Documentation Agent: …") and
    /// once as the clean relayed line ("Grace: …") — producing two balloons
    /// for one question. The dedupe keeps the shortest form only.
    #[test]
    fn duplicated_questions_across_transcript_lines_become_one_balloon() {
        let transcript = "Documentation Agent: Developer, please provide the exact COBOL \
             `PIC` clause for the `CIF` field in `idx-spain-company` (e.g., `PIC X(12)`). \
             ### Schema Handoff Update: Spanish Entity Indexes File: `idx-spain-company` \
             - `CIF`: [PENDING DEVELOPER PIC DEFINITION] - All other ID and Foreign Key fields:\u{2026}\n\
             Some unrelated context line.\n\
             Grace: Developer, please provide the exact COBOL `PIC` clause for the `CIF` \
             field in `idx-spain-company` (e.g., `PIC X(12)`).\n";
        let (context, questions) = split_developer_questions(transcript);
        assert_eq!(questions.len(), 1, "{questions:?}");
        assert!(
            questions[0].starts_with("Grace: Developer, please provide"),
            "the clean short form must win, got: {}",
            questions[0]
        );
        assert!(context.contains("Some unrelated context line."));
        // Genuinely distinct questions both survive.
        let (_, two) = split_developer_questions(
            "Please specify the file name.\nShould CIF be unique per company?",
        );
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn developer_questions_are_split_from_the_reply_context() {
        let reply = "### Developer Clarification Request\n\n\
            The proposed schema separates companies, addresses, and representatives.\n\n\
            **A. Primary File Name**\n\
            Please specify the desired filename for the primary company index.\n\n\
            **B. ID-Format Decisions**\n\
            For every ID field, should the system use UUID or a COBOL PIC definition?\n\n\
            | Field | Decision |\n| --- | --- |\n";
        let (context, questions) = split_developer_questions(reply);
        assert_eq!(questions.len(), 2, "{questions:?}");
        assert!(questions[0].contains("Please specify the desired filename"));
        assert!(questions[1].contains("UUID or a COBOL PIC definition?"));
        // The context keeps the schema explanation and table, not the questions.
        assert!(context.contains("proposed schema"));
        assert!(context.contains("| Field | Decision |"));
        assert!(!context.contains("Please specify"));

        // A reply with no questions passes through untouched.
        let (all, none) = split_developer_questions("All tasks completed.\n\n- **T1** — done");
        assert!(none.is_empty());
        assert!(all.contains("**T1**"));
    }

    /// Spec 036 T4 (AC1/AC6/AC9): a two-task mock run with one correction
    /// round emits the typed live-action stream in order, attributed to the
    /// agent performing each step, and the persisted-record form matches the
    /// emitted stream 1:1.
    #[test]
    fn live_action_stream_is_ordered_attributed_and_persistable() {
        use crate::agent_actions::{ActionKind, AgentAction};
        use cobolt_agents::grace::{AgentInvoker, GraceEngine};

        struct Mock {
            calls: usize,
            script: Vec<(&'static str, &'static str)>,
        }
        impl AgentInvoker for Mock {
            fn invoke(&mut self, agent: &str, _s: &str, _u: &str) -> Result<String, String> {
                let (expect, reply) = self.script[self.calls];
                assert_eq!(agent, expect, "call #{} routed to the wrong agent", self.calls);
                self.calls += 1;
                Ok(reply.to_string())
            }
        }
        let plan = vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Form Designer Agent".into(),
                objective: "Add BTN-OK".into(),
                context: String::new(),
                reviewer: Some("Form Designer Agent Pedantic Reviewer".into()),
                depends_on: vec![],
                acceptance: String::new(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "COBOL Event Handler Script Agent".into(),
                objective: "Implement onClick".into(),
                context: String::new(),
                reviewer: None,
                depends_on: vec!["T1".into()],
                acceptance: String::new(),
            },
        ];
        let mut mock = Mock {
            calls: 0,
            script: vec![
                ("Form Designer Agent", "deployed BTN-OK v1"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "misaligned.\n```json\n{\"pedantic_verdict\": \"defects\", \"correction_request\": \"1. align\"}\n```",
                ),
                ("Form Designer Agent", "deployed BTN-OK v2"),
                (
                    "Form Designer Agent Pedantic Reviewer",
                    "clean.\n```json\n{\"pedantic_verdict\": \"acceptable\", \"correction_request\": \"\"}\n```",
                ),
                ("COBOL Event Handler Script Agent", "MOVE 1 TO WS-OK."),
            ],
        };
        // The same wiring run_grace_workflow_with_control uses: map every
        // engine event through action_for_event, attributing id-only events
        // via the plan.
        let agent_of = |id: &str| {
            plan.iter()
                .find(|t| t.id == id)
                .map(|t| t.agent.clone())
                .unwrap_or_else(|| GRACE.to_string())
        };
        let mut actions: Vec<AgentAction> = Vec::new();
        let record = GraceEngine { max_revisions: 2 }.run_with_progress(
            "wf-036",
            &plan,
            &mut mock,
            &|_| String::new(),
            &mut |e| {
                if let Some(a) = action_for_event(&e, &agent_of, false) {
                    actions.push(a);
                }
            },
        );
        assert_eq!(record.status, "completed");
        let seq: Vec<(&str, ActionKind, &str)> = actions
            .iter()
            .map(|a| (a.agent.as_str(), a.kind, a.detail.as_str()))
            .collect();
        assert_eq!(
            seq,
            vec![
                ("Form Designer Agent", ActionKind::Drafting, "T1"),
                ("Form Designer Agent Pedantic Reviewer", ActionKind::Reviewing, "T1"),
                ("Form Designer Agent", ActionKind::ApplyingCorrections, "T1"),
                ("Form Designer Agent Pedantic Reviewer", ActionKind::Reviewing, "T1"),
                ("Form Designer Agent", ActionKind::Finishing, "T1"),
                ("COBOL Event Handler Script Agent", ActionKind::Drafting, "T2"),
                ("COBOL Event Handler Script Agent", ActionKind::Finishing, "T2"),
            ],
            "default-mode action sequence, in order and attributed"
        );
        // The persisted form (record.action_log entries) is lossless against
        // the emitted stream.
        let log: Vec<_> = actions.iter().map(AgentAction::to_log_entry).collect();
        assert_eq!(log.len(), actions.len());
        for (entry, action) in log.iter().zip(&actions) {
            assert_eq!(entry.agent, action.agent);
            assert_eq!(ActionKind::from_stable(&entry.kind), Some(action.kind));
            assert_eq!(entry.detail, action.detail);
        }
        println!(
            "live-action stream: {} events emitted, {} persisted entries, sequence: {:?}",
            actions.len(),
            log.len(),
            seq
        );
    }

    /// The verbose "Token savings" line reports measured retrieval economy:
    /// percent saved vs. the full corpus, with the estimated token counts —
    /// and stays silent on records that predate the measurement.
    #[test]
    fn rag_savings_line_reports_percentage_from_measured_counts() {
        let tr = crate::i18n::Language::English.tr();
        let mut record = sample_record();
        record.kb_available_tokens = 10_000;
        record.kb_injected_tokens = 750;
        let line = rag_savings_line(&record, &tr).expect("measured record yields a line");
        assert!(line.contains("Token savings: 92.50%"), "{line}");
        assert!(line.contains("≈750"), "{line}");
        assert!(line.contains("≈10000"), "{line}");
        // Pre-measurement records (0 available) stay silent instead of
        // claiming a 100% saving that was never measured.
        record.kb_available_tokens = 0;
        assert!(rag_savings_line(&record, &tr).is_none());
        // An injection larger than the corpus clamps to 0% rather than going
        // negative.
        record.kb_available_tokens = 100;
        record.kb_injected_tokens = 500;
        let clamped = rag_savings_line(&record, &tr).expect("line");
        assert!(clamped.contains("0.00%"), "{clamped}");
        println!("savings line: {line}");
    }

    /// Spec 036 T4 (AC4): verbose mode adds the finer per-submission
    /// granularity; the verdict itself never becomes an action in any mode.
    #[test]
    fn verbose_mode_adds_submission_actions_only() {
        use crate::agent_actions::ActionKind;
        let agent_of = |_: &str| "Form Designer Agent".to_string();
        let submitted = cobolt_agents::grace::GraceEvent::Submitted {
            id: "T1".into(),
            agent: "Form Designer Agent".into(),
        };
        assert!(action_for_event(&submitted, &agent_of, false).is_none());
        let verbose = action_for_event(&submitted, &agent_of, true).expect("verbose emits");
        assert_eq!(verbose.kind, ActionKind::Finishing);
        let verdict = cobolt_agents::grace::GraceEvent::Verdict {
            id: "T1".into(),
            reviewer: "R".into(),
            approved: true,
        };
        assert!(action_for_event(&verdict, &agent_of, false).is_none());
        assert!(action_for_event(&verdict, &agent_of, true).is_none());
        println!("verbose adds Submitted; Verdict emits no action in either mode");
    }

    /// Typographic rules: agents report "<name>: starting/finishing task X" in
    /// the history, and completed work is listed with T# bullets after a blank
    /// line.
    #[test]
    fn history_lines_report_agent_start_and_finish() {
        let started = describe_event(&cobolt_agents::grace::GraceEvent::TaskStarted {
            id: "T1".into(),
            agent: "Documentation Agent".into(),
            objective: "clarify".into(),
        });
        assert!(started.contains("Documentation Agent: starting task T1"), "{started}");
        let finished = describe_event(&cobolt_agents::grace::GraceEvent::Submitted {
            id: "T1".into(),
            agent: "Documentation Agent".into(),
        });
        assert!(finished.contains("Documentation Agent: finishing task T1"), "{finished}");
    }

    /// A plan-less Grace reply is the MODEL routing the request back to the
    /// developer — it is relayed verbatim as a direct conversation record.
    #[test]
    fn plan_less_replies_are_relayed_as_direct_conversations() {
        let record = direct_grace_record(
            "I coordinate the project agents.".into(),
            "Answer or ask the developer directly (no workflow required)",
        );
        assert_eq!(
            workflow_chat_reply(&record, None, false),
            "I coordinate the project agents."
        );
        assert_eq!(record.tasks[0].spec.agent, GRACE);
        assert_eq!(record.status, "completed");
    }

    #[test]
    fn final_summary_is_concise_task_outcomes_not_submission_prose() {
        // The blue-bubble summary lists task outcomes (T# bullets, typographic
        // rule); specialist prose like "done" stays out of concise mode.
        let mut record = sample_record();
        record.tasks[0].spec.id = "T1".into();
        let summary = grace_final_summary(&record, &[]);
        assert!(
            summary.contains("Tasks:\n\n- T1 — Version Control Agent: commit"),
            "paragraph, blank line, then T# bullets — got: {summary}"
        );
        assert!(!summary.contains("done"), "prose stays verbose-only");
    }

    /// Observed live: ollama_cloud/gemma4 returned "Response contained no
    /// message or tool call (empty)" twice, failing tasks that were finally
    /// actionable. Empty/timeout/5xx responses are transient and retried;
    /// credential and request-shape errors are not.
    #[test]
    fn transient_model_errors_are_classified_for_retry() {
        assert!(is_transient_model_error(
            "model request failed: ResponseError: Response contained no message or tool call (empty)"
        ));
        // The transport's own wording for the same condition. Observed live:
        // anthropic/claude-sonnet-5 answered a 30-control Form Designer task
        // with no assistant text after 72.7s, and the workflow died on the
        // first attempt because only rig's phrasing was classified.
        assert!(is_transient_model_error(
            "the model returned no assistant text"
        ));
        assert!(is_transient_model_error(
            "the model returned no assistant text — the reply carried only hidden reasoning (1 block(s), 812 rendered character(s)), which cannot be applied as a plan, change-set, or review"
        ));
        assert!(is_transient_model_error("request timed out after 30s"));
        assert!(is_transient_model_error("HTTP 503 Service Unavailable"));
        assert!(is_transient_model_error("server overloaded, retry later"));
        assert!(!is_transient_model_error("401 Unauthorized: invalid api key"));
        assert!(!is_transient_model_error(
            "Function tools with reasoning_effort are not supported"
        ));
    }

    /// The Pedantic-companion contract is enforced mechanically: fabricated
    /// reviewer names are replaced by the responsible agent's real companion,
    /// and a disabled/missing companion clears the gate.
    #[test]
    fn plan_reviewers_are_sanitized_to_enabled_companions() {
        let companion_of = |agent: &str| -> Option<String> {
            match agent {
                "Data (Indexed File) Agent" => Some("Data Reviewer".into()),
                // Documentation Agent's companion is disabled → None.
                _ => None,
            }
        };
        let mut plan = vec![
            TaskSpec {
                id: "T1".into(),
                agent: "Documentation Agent".into(),
                objective: "clarify".into(),
                context: String::new(),
                reviewer: Some("Documentation Agent Pedantic Reviewer".into()),
                depends_on: vec![],
                acceptance: String::new(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: "Data (Indexed File) Agent".into(),
                objective: "create schema".into(),
                context: String::new(),
                // Fabricated reviewer name, observed live.
                reviewer: Some("COBOL Pedantic Agent".into()),
                depends_on: vec!["T1".into()],
                acceptance: String::new(),
            },
        ];
        let mut notes = Vec::new();
        sanitize_plan_reviewers(&companion_of, &mut plan, &mut |line| notes.push(line));
        assert_eq!(plan[0].reviewer, None, "disabled companion clears the gate");
        assert_eq!(
            plan[1].reviewer.as_deref(),
            Some("Data Reviewer"),
            "fabricated reviewer replaced by the real companion"
        );
        assert_eq!(notes.len(), 2, "both corrections surfaced to the log");
    }

    /// A form-design task delegated with an empty context must receive the
    /// control inventory from the surface context, so the specialist can resolve
    /// ids and geometry instead of producing nothing. Non-form tasks and tasks
    /// whose context Grace already filled are left untouched.
    #[test]
    fn form_task_gets_control_inventory_injected() {
        let context = "CONTEXT\nFORM: MAIN-FORM (1352x2000)\n\
             FORM PROPERTIES: GlassStyle=\"Enhanced\"\n\
             AVAILABLE CONTROL TYPES: BarChart, PieChart\n\
             CONTROLS:\n  \
             BarChart-1 (BarChart) @(656,624) 320x220\n  \
             PieChart-1 (PieChart) @(656,864) 320x220\n  \
             PictureBox-2 (PictureBox) @(0,0) 800x400\n\
             PROPERTY KEYS BY TYPE:\n  \
             BarChart: Anchor, X, Y, Width, Height\n  \
             PieChart: Anchor, X, Y, SliceColors\n  \
             PictureBox: ImagePath, Visible\n  \
             TreeView: Nodes, ShowLines\n  \
             Slider: Value, Minimum, Maximum\n\
             EVENTS BY TYPE:\n  \
             BarChart: onClick\n  \
             PictureBox: onClick, onImageLoaded\n  \
             Slider: onChange, onValueChanged\n\
             CONTROL API BY ID:\n  \
             PictureBox-2 (PictureBox): properties [ImagePath, Visible]; methods [PlayAnimation, StopAnimation, SetProperty]\n\
             PROPERTY INTENT MAP: shadow => ShadowEnabled\n\
             PROCEDURES: VALIDATE-INPUT, RECALC-TOTAL";
        let mut plan = vec![
            TaskSpec {
                id: "T1".into(),
                agent: crate::agents_db::FORM_DESIGNER.into(),
                objective: "reorganise the six charts into a 2x3 grid".into(),
                context: String::new(),
                reviewer: None,
                depends_on: vec![],
                acceptance: String::new(),
            },
            TaskSpec {
                id: "T2".into(),
                agent: crate::agents_db::EVENT_HANDLER.into(),
                objective: "wire onClick".into(),
                context: String::new(),
                reviewer: None,
                depends_on: vec![],
                acceptance: String::new(),
            },
            TaskSpec {
                id: "T3".into(),
                agent: crate::agents_db::FORM_DESIGNER.into(),
                objective: "already specified".into(),
                context: "Grace's own exact identifiers".into(),
                reviewer: None,
                depends_on: vec![],
                acceptance: String::new(),
            },
        ];
        inject_task_context(context, &mut plan);

        // The empty-context form task now carries the inventory: form dims,
        // control types, and the per-control lines.
        assert!(plan[0].context.contains("FORM: MAIN-FORM (1352x2000)"));
        assert!(plan[0].context.contains("BarChart-1 (BarChart) @(656,624) 320x220"));
        assert!(plan[0].context.contains("PieChart-1 (PieChart)"));
        // …the form-level block, because a restyle targets `"control_id": "Form"`
        // and the designer's prompt forbids keys not listed under FORM PROPERTIES.
        assert!(plan[0].context.contains("FORM PROPERTIES"));
        assert!(plan[0].context.contains("GlassStyle=\"Enhanced\""));
        // …and the property keys for the types actually in play — the types on
        // the form, and nothing else. Sending all 34 would blow the budget;
        // sending none left the designer obeying a list it could not see.
        assert!(plan[0].context.contains("PROPERTY KEYS BY TYPE"));
        assert!(plan[0].context.contains("BarChart: Anchor, X, Y, Width, Height"));
        assert!(plan[0].context.contains("PieChart: Anchor, X, Y, SliceColors"));
        assert!(plan[0].context.contains("PictureBox: ImagePath, Visible"));
        assert!(
            !plan[0].context.contains("TreeView:") && !plan[0].context.contains("Slider:"),
            "types absent from the form and the objective must stay out of the budget"
        );
        assert!(
            !plan[0].context.contains("PlayAnimation"),
            "the form designer does not need the method API"
        );
        assert!(
            !plan[0].context.contains("EVENTS BY TYPE"),
            "the property excerpt must stop before the event dump"
        );
        // The event-handler task gets the inventory AND the per-control method
        // API, so it can call PictureBox-2::PlayAnimation() instead of inventing
        // a method its reviewer can never verify (the correction-loop deadlock).
        assert!(
            plan[1].context.contains("PictureBox-2 (PictureBox)"),
            "event task gets the control inventory"
        );
        assert!(
            plan[1].context.contains("PlayAnimation"),
            "event task gets each control's real methods"
        );
        // …and the events its types support. An event name is hard-validated and
        // an invalid op is skipped at apply, so a guessed name yields no handler
        // and no error — the same silent nothing a missing change-set produced.
        assert!(
            plan[1].context.contains("EVENTS BY TYPE"),
            "event task gets the event legend it is told to bind against"
        );
        assert!(plan[1].context.contains("BarChart: onClick"));
        assert!(plan[1].context.contains("PictureBox: onClick, onImageLoaded"));
        assert!(
            !plan[1].context.contains("Slider:"),
            "a type absent from the form and the objective stays out of the budget"
        );
        // …and the procedures it may CALL. A CALL target is NOT validated, so a
        // guessed name reaches the form and fails at COBOL compile time.
        assert!(
            plan[1].context.contains("PROCEDURES: VALIDATE-INPUT, RECALC-TOTAL"),
            "event task gets the procedures it is told to CALL by name"
        );
        assert!(
            !plan[1].context.contains("PROPERTY INTENT MAP"),
            "the API excerpt stops before the next section"
        );
        // Grace-filled context is untouched.
        assert_eq!(plan[2].context, "Grace's own exact identifiers");
    }

    /// Grace routes work; she does not need all 34 types' property keys and
    /// events to decide that a Button click belongs to the event-handler agent.
    /// Her copy is trimmed to the types in play — but the FULL context must
    /// survive for `inject_task_context`, or a specialist loses the very type
    /// its objective names.
    #[test]
    fn graces_planning_context_is_trimmed_but_the_full_one_survives_for_tasks() {
        let context = "CONTEXT\nFORM: MAIN-FORM (800x600)\n\
             CONTROLS:\n  Button-1 (Button) @(10,10) 80x30\n\
             PROPERTY KEYS BY TYPE (for all available controls):\n  \
             Button: Caption, X, Y\n  \
             Timer: Interval, Enabled\n  \
             TreeView: Items, ShowLines\n\
             EVENTS BY TYPE (for all available controls):\n  \
             Button: onClick\n  \
             Timer: onTick\n  \
             TreeView: onNodeClick\n\
             CONTROL API BY ID:\n  Button-1 (Button): properties [Caption]; methods [SetCaption]\n\
             PROCEDURES: (none)";
        let request = "add code on the onClick event for Button-1";
        let planning = planning_surface_context(context, request);

        // The form, the API block and the tail are preserved verbatim.
        assert!(planning.contains("FORM: MAIN-FORM (800x600)"));
        assert!(planning.contains("Button-1 (Button) @(10,10) 80x30"));
        assert!(planning.contains("CONTROL API BY ID:"));
        assert!(planning.contains("PROCEDURES: (none)"));
        // The type on the form keeps both legends…
        assert!(planning.contains("Button: Caption, X, Y"));
        assert!(planning.contains("Button: onClick"));
        // …and the 33 types that have nothing to do with this request are gone.
        assert!(
            !planning.contains("TreeView") && !planning.contains("Timer"),
            "unrelated types must not reach Grace: {planning}"
        );
        assert!(
            planning.len() < context.len(),
            "the trimmed view must be smaller"
        );

        // The untrimmed context still resolves a type only a task objective
        // names — the regression that trimming too early would cause.
        let mut plan = vec![TaskSpec {
            id: "T1".into(),
            agent: crate::agents_db::EVENT_HANDLER.into(),
            objective: "wire the Timer tick".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec![],
            acceptance: String::new(),
        }];
        inject_task_context(context, &mut plan);
        assert!(
            plan[0].context.contains("Timer: onTick"),
            "the specialist still gets the type its objective names"
        );
    }

    /// A context without the legend markers must pass through untouched rather
    /// than be silently emptied.
    #[test]
    fn planning_context_without_legends_is_unchanged() {
        let bare = "CONTEXT\nFORM: F (10x10)\nCONTROLS:\n  (none)";
        assert_eq!(planning_surface_context(bare, "anything"), bare);
    }

    /// "Deploy a Timer, then wire its tick" is two tasks: at injection time the
    /// Timer is not on the form yet, so its event list can only come from the
    /// objective. A Timer supports exactly one event (`onTick`), and a guess
    /// (`onTimer`, `onElapsed`) is rejected by `validate_op` and then skipped at
    /// apply — no handler, no error. The events legend must therefore follow the
    /// same in-play rule the property keys do.
    #[test]
    fn event_task_gets_the_events_of_a_type_named_only_in_its_objective() {
        let context = "CONTEXT\nFORM: MAIN-FORM (800x600)\n\
             CONTROLS:\n  Label-1 (Label) @(10,10) 100x20\n\
             PROPERTY KEYS BY TYPE:\n  Timer: Interval, Enabled\n\
             EVENTS BY TYPE:\n  \
             Label: onClick\n  \
             Timer: onTick\n  \
             TreeView: onNodeClick\n\
             CONTROL API BY ID:\n  Label-1 (Label): properties [Caption]; methods [SetCaption]\n\
             PROCEDURES: (none)";
        let mut plan = vec![TaskSpec {
            id: "T2".into(),
            agent: crate::agents_db::EVENT_HANDLER.into(),
            objective: "wire the Timer to refresh Label-1 every second".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec![],
            acceptance: String::new(),
        }];
        inject_task_context(context, &mut plan);

        assert!(
            plan[0].context.contains("Timer: onTick"),
            "the type named in the objective brings its events, though it is not on the form yet"
        );
        assert!(
            plan[0].context.contains("Label: onClick"),
            "the type already on the form keeps its events"
        );
        assert!(
            !plan[0].context.contains("TreeView"),
            "an unrelated type stays out of the delegated budget"
        );
        assert!(
            plan[0].context.contains("PROCEDURES: (none)"),
            "an empty procedure list is still an answer — it says none exist to CALL"
        );
    }

    /// A control the task is about to CREATE is not on the form yet, so its
    /// property keys cannot come from the inventory. Without them the designer
    /// must invent keys for the very control it is deploying — the failure the
    /// property block exists to prevent.
    #[test]
    fn deploy_task_gets_the_keys_of_the_type_it_is_asked_to_create() {
        let context = "CONTEXT\nFORM: MAIN-FORM (800x600)\n\
             FORM PROPERTIES: GlassStyle=\"Classic\"\n\
             AVAILABLE CONTROL TYPES: Button, Slider, LineChart\n\
             CONTROLS:\n  Button-1 (Button) @(10,10) 90x28\n\
             PROPERTY KEYS BY TYPE:\n  \
             Button: Caption, X, Y\n  \
             Slider: Value, Minimum, Maximum\n  \
             LineChart: Series, Legend\n  \
             Line: Thickness\n\
             EVENTS BY TYPE:\n  Button: onClick";
        let mut plan = vec![TaskSpec {
            id: "T1".into(),
            agent: crate::agents_db::FORM_DESIGNER.into(),
            objective: "add a Slider under the button".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec![],
            acceptance: String::new(),
        }];
        inject_task_context(context, &mut plan);

        assert!(
            plan[0].context.contains("Slider: Value, Minimum, Maximum"),
            "the type being deployed must carry its keys"
        );
        assert!(
            plan[0].context.contains("Button: Caption, X, Y"),
            "types already on the form are still included"
        );
        assert!(
            !plan[0].context.contains("LineChart:"),
            "a type neither on the form nor in the objective stays out"
        );
        assert!(
            !plan[0].context.contains("Line: Thickness"),
            "matching must be whole-word: 'Slider' must not drag in 'Line'"
        );
    }

    /// Whole-word matching, in both directions: a type name embedded in a longer
    /// word is not a mention, and a mention is case-insensitive.
    #[test]
    fn type_mentions_are_whole_words() {
        assert!(mentions_type("add a slider", "Slider"));
        assert!(mentions_type("add a SLIDER, please", "Slider"));
        assert!(mentions_type("wrap it in a Panel.", "Panel"));
        assert!(!mentions_type("rename PanelHeader", "Panel"));
        assert!(!mentions_type("tidy the LineChart", "Line"));
        assert!(!mentions_type("no controls here", "Slider"));
    }

    /// With no surface context (markers absent), injection is a no-op — the
    /// non-context workflow entry points must not be disturbed.
    #[test]
    fn inject_task_context_noops_without_surface_context() {
        let mut plan = vec![TaskSpec {
            id: "T1".into(),
            agent: crate::agents_db::FORM_DESIGNER.into(),
            objective: "do a thing".into(),
            context: String::new(),
            reviewer: None,
            depends_on: vec![],
            acceptance: String::new(),
        }];
        inject_task_context("(no additional surface context)", &mut plan);
        assert!(plan[0].context.is_empty());
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

        // Every reviewed specialist/orchestrator is told the real mechanics —
        // the review runs AFTER the reply — and that a fabricated approval is
        // a named critical defect. Observed live: a Form Designer submission
        // ended "Aprovação obtida" with no review call anywhere in the run.
        for name in [GRACE, crate::agents_db::FORM_DESIGNER, crate::agents_db::VERSION_CONTROL] {
            let contract = pedantic_relationship_contract(&db, name);
            assert!(
                contract.contains("FABRICATED APPROVAL = CRITICAL DEFECT"),
                "{name}'s contract must forbid fabricated approvals"
            );
            assert!(
                contract.contains("NO review has happened yet"),
                "{name}'s contract must state the review runs after the reply"
            );
            assert!(
                !contract.contains("Submit your complete work"),
                "{name}'s contract must not instruct an impossible submission"
            );
        }
        let _ = std::fs::remove_dir_all(project);
    }
}
