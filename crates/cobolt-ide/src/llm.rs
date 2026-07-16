// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use cobolt_agents::Orchestrator;

pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an expert pair programmer for PowerRustCOBOL...";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_cobol_proficiency_prompt")]
    pub cobol_proficiency_prompt: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub verbose_log: bool,
    /// TCP port for the egui inspection endpoint (agent access, spec 027 R3).
    /// Always bound on 127.0.0.1 only; a change takes effect on restart.
    #[serde(default = "default_inspection_port")]
    pub inspection_port: u16,
    /// Per-model API keys, keyed by `"{provider}::{model}"` — selecting a
    /// model in Project Settings restores its remembered key (or clears the
    /// field when none is stored, so a stale key never masquerades as valid).
    #[serde(default)]
    pub api_keys: std::collections::HashMap<String, String>,
    /// Optional second model powering the **Pedantic Agent** (reviewer). When
    /// configured it must differ from the primary provider+model pair; its
    /// API key lives in [`Self::api_keys`] like any other model's.
    #[serde(default)]
    pub reviewer_provider: String,
    #[serde(default)]
    pub reviewer_endpoint: String,
    #[serde(default)]
    pub reviewer_model: String,
    /// The Pedantic Agent's system prompt (operator-authored, 2026-07-16).
    #[serde(default = "default_pedantic_prompt")]
    pub pedantic_prompt: String,
}

/// Map key for [`LlmConfig::api_keys`]: keys are provider-scoped so the same
/// model name under two providers keeps two independent credentials.
pub fn api_key_slot(provider: &str, model: &str) -> String {
    format!("{}::{}", provider.trim(), model.trim())
}

/// Default localhost port for the egui inspection / MCP agent endpoint.
pub fn default_inspection_port() -> u16 {
    5719
}

impl LlmConfig {
    pub fn load() -> Self {
        let path = base_dir().join("llm_config.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<Self>(&data) {
                if retired_model_message(&cfg.model).is_some() {
                    cfg.model.clear();
                    let _ = cfg.save();
                }
                return cfg;
            }
        }
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            cobol_proficiency_prompt: default_cobol_proficiency_prompt(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
            provider: String::new(),
            verbose_log: false,
            inspection_port: default_inspection_port(),
            api_keys: std::collections::HashMap::new(),
            reviewer_provider: String::new(),
            reviewer_endpoint: String::new(),
            reviewer_model: String::new(),
            pedantic_prompt: default_pedantic_prompt(),
        }
    }

    /// Whether the optional reviewer (Pedantic Agent) model is usable: fully
    /// configured AND different from the primary provider+model pair.
    pub fn reviewer_configured(&self) -> bool {
        !self.reviewer_provider.trim().is_empty()
            && !self.reviewer_model.trim().is_empty()
            && !(self.reviewer_provider.trim() == self.provider.trim()
                && self.reviewer_model.trim() == self.model.trim())
    }

    /// An [`LlmConfig`] view of the reviewer model (endpoint/key swapped in),
    /// for reuse of the existing request plumbing.
    pub fn reviewer_config(&self) -> LlmConfig {
        let mut c = self.clone();
        c.provider = self.reviewer_provider.clone();
        c.endpoint = self.reviewer_endpoint.clone();
        c.model = self.reviewer_model.clone();
        c.api_key = self
            .api_keys
            .get(&api_key_slot(&self.reviewer_provider, &self.reviewer_model))
            .cloned()
            .unwrap_or_default();
        c
    }

    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && !self.model.is_empty()
    }
    pub fn save(&self) -> Result<(), String> {
        let path = base_dir().join("llm_config.json");
        let mut cfg = self.clone();
        if retired_model_message(&cfg.model).is_some() {
            cfg.model.clear();
        }
        let data = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn retired_model_message(model: &str) -> Option<String> {
    let model = model.trim();
    if model.eq_ignore_ascii_case("qwen3-coder-next") {
        Some(
            "`qwen3-coder-next` was retired by Ollama on 2026-07-15. \
             Refresh the model list and select a currently available model."
                .to_string(),
        )
    } else {
        None
    }
}

pub fn default_system_prompt() -> String {
    DEFAULT_SYSTEM_PROMPT.to_string()
}
pub fn default_cobol_proficiency_prompt() -> String {
    COBOL_PROFICIENCY_BENCHMARK_PROMPT.to_string()
}
pub fn default_temperature() -> f32 {
    0.7
}
pub fn default_max_tokens() -> u32 {
    8192
}
pub fn default_timeout_secs() -> u32 {
    30
}

pub enum LlmResponse {
    Ok(String),
    Chunk(String),
    Err(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

impl ChatTurn {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

pub enum AiLogKind {
    Info,
    Detail,
    Reasoning,
    Question,
    Error,
}

#[derive(Clone)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub default_endpoint: &'static str,
}

impl Provider {
    pub fn from_id(id: &str) -> Option<Self> {
        PROVIDERS.iter().find(|p| p.id == id).cloned()
    }
    pub fn id(&self) -> &str {
        self.id
    }
    pub fn label(&self) -> &str {
        self.label
    }
    pub fn default_endpoint(&self) -> &str {
        self.default_endpoint
    }
}

pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "openai",
        label: "OpenAI",
        default_endpoint: "https://api.openai.com/v1",
    },
    Provider {
        id: "anthropic",
        label: "Anthropic",
        default_endpoint: "https://api.anthropic.com/v1",
    },
    Provider {
        id: "cohere",
        label: "Cohere",
        default_endpoint: "https://api.cohere.ai/v1",
    },
    Provider {
        id: "gemini",
        label: "Google Gemini",
        default_endpoint: "https://generativelanguage.googleapis.com/v1beta",
    },
    Provider {
        id: "perplexity",
        label: "Perplexity",
        default_endpoint: "https://api.perplexity.ai",
    },
    Provider {
        id: "groq",
        label: "Groq",
        default_endpoint: "https://api.groq.com/openai/v1",
    },
    Provider {
        id: "mistral",
        label: "Mistral",
        default_endpoint: "https://api.mistral.ai/v1",
    },
    Provider {
        id: "openrouter",
        label: "OpenRouter",
        default_endpoint: "https://openrouter.ai/api/v1",
    },
    Provider {
        id: "huggingface",
        label: "HuggingFace",
        default_endpoint: "https://api-inference.huggingface.co/models",
    },
    Provider {
        id: "together",
        label: "Together AI",
        default_endpoint: "https://api.together.xyz/v1",
    },
    Provider {
        id: "deepseek",
        label: "DeepSeek",
        default_endpoint: "https://api.deepseek.com/v1",
    },
    Provider {
        id: "xai",
        label: "xAI",
        default_endpoint: "https://api.x.ai/v1",
    },
    Provider {
        id: "voyageai",
        label: "Voyage AI",
        default_endpoint: "https://api.voyageai.com/v1",
    },
    Provider {
        id: "ollama",
        label: "Ollama (Local)",
        default_endpoint: "http://localhost:11434/api",
    },
    Provider {
        id: "ollama_cloud",
        label: "Ollama (Cloud)",
        // ollama.com serves both the native API (/api) and an OpenAI-
        // compatible one (/v1). NOT api.ollama.com — that host is wrong.
        default_endpoint: "https://ollama.com/v1",
    },
    Provider {
        id: "llamafile",
        label: "Llamafile (Local)",
        default_endpoint: "http://localhost:8080/v1",
    },
];

pub struct DetectedApi {
    pub models: Vec<String>,
    pub endpoint: String,
    pub provider: String,
}

pub fn base_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cobolt");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

use std::sync::{LazyLock, Mutex};

static AI_LOG_QUEUE: LazyLock<Mutex<Vec<AiLogEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static CONNECTION_LOG: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

pub fn load_history(_dir: &Path, key: &str) -> Vec<ChatTurn> {
    let path = base_dir().join(format!("{}.json", key));
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(history) = serde_json::from_str(&data) {
            return history;
        }
    }
    vec![]
}
pub fn save_history(_dir: &Path, key: &str, turns: &[ChatTurn]) {
    let path = base_dir().join(format!("{}.json", key));
    if let Ok(data) = serde_json::to_string_pretty(turns) {
        let _ = std::fs::write(&path, data);
    }
}

pub fn load_raw_preferred_indexed(_dir: &Path) -> HashSet<String> {
    let path = base_dir().join("preferred_indexed.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(rels) = serde_json::from_str(&data) {
            return rels;
        }
    }
    HashSet::new()
}
pub fn save_raw_preferred_indexed(_dir: &Path, rels: &HashSet<String>) {
    let path = base_dir().join("preferred_indexed.json");
    if let Ok(data) = serde_json::to_string_pretty(rels) {
        let _ = std::fs::write(&path, data);
    }
}

pub fn push_ai_log(kind: AiLogKind, text: impl Into<String>) {
    if let Ok(mut q) = AI_LOG_QUEUE.lock() {
        q.push(AiLogEntry {
            kind,
            text: text.into(),
        });
    }
}

/// Run one mesh request on a worker thread, streaming progress into the
/// Agentic AI activity log and the connection log. Shared by the agent,
/// direct-editor, and compaction entry points.
fn run_mesh_request(req: cobolt_agents::MeshRequest, label: &'static str) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    push_ai_log(
        AiLogKind::Info,
        format!("{label}: \"{}\"", truncate_for_log(&req.user_prompt, 160)),
    );
    if let Some(msg) = retired_model_message(&req.model) {
        push_ai_log(AiLogKind::Error, msg.clone());
        push_connection_log(&format!("=== ERROR ===\n{msg}\n"));
        let _ = tx.send(LlmResponse::Err(msg));
        return rx;
    }
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(LlmResponse::Err(format!(
                    "Failed to start async runtime: {e}"
                )));
                return;
            }
        };
        rt.block_on(async {
            let orch = Orchestrator::new();
            // Every orchestrator step lands in the activity log as it happens.
            let on_log = |line: String| push_ai_log(AiLogKind::Detail, format!("agent · {line}"));
            let tx_clone = tx.clone();
            let on_chunk = move |chunk: &str| {
                let _ = tx_clone.send(LlmResponse::Chunk(chunk.to_string()));
            };
            match orch.handle_request(&req, &on_log, &on_chunk).await {
                Ok((resp, trace)) => {
                    push_connection_log(&trace);
                    push_ai_log(
                        AiLogKind::Reasoning,
                        format!(
                            "reply: {} chars — {}",
                            resp.len(),
                            truncate_for_log(&resp, 200)
                        ),
                    );
                    let _ = tx.send(LlmResponse::Ok(resp));
                }
                Err(e) => {
                    push_connection_log(&format!("=== ERROR ===\n{e}\n"));
                    push_ai_log(AiLogKind::Error, e.clone());
                    let _ = tx.send(LlmResponse::Err(e));
                }
            }
        });
    });
    rx
}

fn truncate_for_log(s: &str, max: usize) -> String {
    let s = s.trim().replace('\n', " ");
    if s.chars().count() <= max {
        s
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn mesh_request_base(cfg: &LlmConfig) -> cobolt_agents::MeshRequest {
    cobolt_agents::MeshRequest {
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
        api_key: cfg.api_key.clone(),
        endpoint: cfg.endpoint.clone(),
        specialist: None,
        system_prompt: String::new(),
        skills: String::new(),
        context: String::new(),
        history: Vec::new(),
        user_prompt: String::new(),
        temperature: cfg.temperature,
        max_tokens: cfg.max_tokens,
        verbose: cfg.verbose_log,
    }
}

pub fn spawn_agent_request(
    cfg: &LlmConfig,
    sys: &str,
    skills: &str,
    history: &[ChatTurn],
    sent: &str,
    context: &str,
    specialist: Option<String>,
) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.specialist = specialist;
    // The composed prompt/skills/context ARE the dev-agent contract (spec 025
    // R14/R21/R2) — they must reach the model on every request.
    req.system_prompt = sys.to_string();
    req.skills = skills.to_string();
    req.context = context.to_string();
    let mut h: Vec<_> = history
        .iter()
        .map(|t| (t.role.clone(), t.content.clone()))
        .collect();
    if let Some(last) = h.last() {
        if last.0 == "user" && last.1 == sent {
            h.pop();
        }
    }
    req.history = h;
    req.user_prompt = sent.to_string();
    run_mesh_request(req, "agent request")
}

pub fn spawn_request(
    cfg: &LlmConfig,
    history: &[ChatTurn],
    prompt: &str,
    code: &str,
    file: &str,
    skills: &str,
    specialist: Option<String>,
) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.specialist = specialist;
    req.system_prompt = cfg.system_prompt.clone();
    req.skills = skills.to_string();
    // The editor request acts on the current file: include it as context so
    // the model sees the code it is asked to work on.
    if !code.trim().is_empty() {
        req.context = format!("Current file `{file}`:\n```cobol\n{code}\n```");
    }

    let mut h: Vec<_> = history
        .iter()
        .map(|t| (t.role.clone(), t.content.clone()))
        .collect();
    if let Some(last) = h.last() {
        if last.0 == "user" && last.1 == prompt {
            h.pop();
        }
    }
    req.history = h;
    req.user_prompt = prompt.to_string();
    run_mesh_request(req, "editor request")
}

pub fn spawn_compaction(cfg: &LlmConfig, history: &[ChatTurn]) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.system_prompt =
        "You summarize a development-assistant conversation, preserving decisions, \
         names, and unresolved items."
            .to_string();
    req.history = history
        .iter()
        .map(|t| (t.role.clone(), t.content.clone()))
        .collect();
    req.user_prompt = "Summarize the preceding chat history concisely.".to_string();
    run_mesh_request(req, "history compaction")
}

/// Heal endpoints saved with the previously shipped wrong Ollama Cloud host
/// (api.ollama.com → ollama.com). Applied on every outbound use so existing
/// configs keep working without the user re-picking the provider.
fn heal_endpoint(ep: &str) -> String {
    ep.trim().replace("api.ollama.com", "ollama.com")
}

pub fn spawn_test(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    let mut test_cfg = cfg.clone();
    test_cfg.endpoint = heal_endpoint(&test_cfg.endpoint);
    test_cfg.temperature = 0.0;
    test_cfg.max_tokens = test_cfg.max_tokens.clamp(1, 16);

    let mut req = mesh_request_base(&test_cfg);
    req.specialist = Some("CodeGenerator".to_string());
    req.system_prompt =
        "You are testing model access. Reply with the exact text OK and nothing else.".to_string();
    req.user_prompt = "Reply with OK only.".to_string();
    run_mesh_request(req, "connection/model access test")
}

/// The Pedantic Agent's system prompt — authored verbatim by the operator
/// (2026-07-16), with a separated machine-readable response contract appended
/// so the tandem loop can parse verdicts and scores.
pub fn default_pedantic_prompt() -> String {
    DEFAULT_PEDANTIC_PROMPT.to_string()
}

pub const DEFAULT_PEDANTIC_PROMPT: &str = r#"The Pedantic Agent performs a comprehensive and uncompromising review of every response produced by the primary agent.
Its primary objective is to verify that the generated code strictly adheres to the COBOL-85 standard and correctly applies the RustCOBOL extensions, rules, conventions, and constraints defined in the prompt provided to the primary agent. The Pedantic Agent must use that prompt as the authoritative specification and must not redefine or restate those extensions unnecessarily.
It must rigorously inspect the generated code, technical reasoning, assumptions, explanations, and conclusions. The review must identify any response that is:

* technically incorrect;
* incompatible with COBOL-85 requirements;
* inconsistent with the RustCOBOL extensions defined in the primary prompt;
* ambiguous or insufficiently justified;
* based on fabricated information or unsupported assumptions;
* incomplete;
* outside the requested scope;
* inconsistent with the supplied requirements;
* noncompliant with explicit instructions;
* unnecessarily verbose, repetitive, or poorly structured;
* incompatible with the target compiler, runtime, language rules, or coding conventions;
* likely to introduce defects, regressions, security issues, portability problems, or maintenance risks.
The Pedantic Agent must verify syntax, semantics, data definitions, control flow, scope termination, paragraph structure, file handling, table usage, type compatibility, portability, runtime behavior, and every other relevant aspect of the submitted code.
It must also detect code that may appear plausible but does not actually conform to COBOL-85, incorrectly assumes support for undeclared language features, misuses RustCOBOL extensions, or invents syntax and behavior not authorized by the primary prompt.
The Pedantic Agent must challenge the work directly, precisely, and objectively. It must not soften criticism, approve partially correct work without qualification, overlook defects for the sake of politeness, or infer compliance merely because the response appears confident or well formatted.
Whenever problems are found, the primary agent must be instructed to correct them and submit the complete response again. The revised submission must fully replace the defective version rather than provide isolated patches, unless incremental changes were explicitly requested.
Each correction request must clearly identify:

1. the defective code or statement;
2. the violated COBOL-85 rule, RustCOBOL requirement, or explicit instruction;
3. why the current implementation is incorrect, ambiguous, unsafe, or inadequate;
4. the expected correction;
5. any related sections that must be revalidated after the change.
The Pedantic Agent must then review the revised submission with the same level of scrutiny. A revision must never be accepted merely because it addresses the previously listed defects; the entire response must be reviewed again for newly introduced errors, inconsistencies, regressions, and remaining violations.
If the primary agent still fails to satisfy the requirements after revision, the Pedantic Agent must produce a brutally honest final assessment containing:

1. a summary of the requested work;
2. the defects found in the original response;
3. the corrections requested;
4. the defects that remain after revision;
5. any COBOL-85 rules, RustCOBOL requirements, instructions, or constraints that were ignored or violated;
6. the technical and practical consequences of the remaining problems;
7. a clear verdict on whether the result is acceptable;
8. a numerical score proportional to the actual quality of the work.
The score must reflect:

* COBOL-85 compliance;
* correct use of the RustCOBOL extensions defined in the primary prompt;
* technical correctness;
* completeness;
* instruction adherence;
* scope compliance;
* code quality;
* maintainability;
* portability;
* safety;
* compiler credibility;
* runtime credibility.
No credit should be awarded for confident presentation, excessive explanation, superficial completeness, or plausible-looking code when the underlying implementation is incorrect, unverifiable, noncompliant, or fabricated.

--- Tooling contract (response format; does not alter the review rules above) ---

For a first or repeated review round, END your review with exactly one fenced JSON block:

```json
{"pedantic_verdict": "defects" | "acceptable", "correction_request": "<the numbered correction request, empty when acceptable>"}
```

For the FINAL assessment (after the revision round), END with exactly one fenced JSON block using the SAME metrics schema the primary prompt defines for its dashboard scores, with your own uncompromising values, plus:

```json
{"pedantic_final": true, "verdict": "<acceptable | not acceptable>", "overall_score": <0-100>}
```
merged into that schema (one JSON object). The dashboard reads this block; scores you do not state cannot be displayed."#;

const COBOL_PROFICIENCY_BENCHMARK_PROMPT: &str = r#"You are an expert evaluator of Large Language Models for COBOL-85 and PowerRustCOBOL development.

Run a deterministic self-contained proficiency assessment of this model. Do not browse. Do not ask follow-up questions. Do not modify any project files. This is a chat-only benchmark: you are not compiling, running, or externally verifying generated code.

Evaluate engineering quality, not natural-language style. Evaluate only features currently supported by PowerRustCOBOL. Do not test, reward, or penalize for unsupported or superseded features. Be adversarial and skeptical: the evaluator must look for concrete defects in the generated COBOL, not praise the sample by default.

Supported benchmark scope:
1. COBOL source structure: fixed/free form source; IDENTIFICATION, ENVIRONMENT, DATA, PROCEDURE divisions; CONFIGURATION, INPUT-OUTPUT, FILE-CONTROL, FILE, WORKING-STORAGE, LOCAL-STORAGE, and LINKAGE sections; nested programs and multiple program units; complete programs with no omitted divisions.
2. Data descriptions: level 01-49, 66, 77, 88; FILLER; VALUE; GLOBAL; EXTERNAL; REDEFINES; RENAMES; OCCURS and OCCURS DEPENDING ON; ASCENDING/DESCENDING KEY; INDEXED BY; USAGE DISPLAY, BINARY/COMP/COMP-4, COMP-1, COMP-2, COMP-3/PACKED-DECIMAL, COMP-5, INDEX, POINTER; PIC X/A/9/S/V/P plus supported edited symbols.
3. Implemented statements and control flow: MOVE, COMPUTE, ADD, SUBTRACT, MULTIPLY, DIVIDE, STRING, UNSTRING, INSPECT, INITIALIZE, SEARCH, SEARCH ALL, SORT, MERGE, RELEASE, RETURN, PERFORM, EVALUATE, IF, GO TO, ALTER, CONTINUE, NEXT SENTENCE, SET, CALL for COBOL nested programs/user procedures, CANCEL, EXIT, GOBACK, STOP RUN, ACCEPT, DISPLAY, TRY/CATCH/FINALLY/END-TRY, THROW, EXEC RUST.
4. File handling: sequential, line sequential, and indexed files; random/dynamic indexed access; primary and alternate keys, including duplicate alternate keys; OPEN/CLOSE; READ, READ NEXT, READ PREVIOUS; START; WRITE; REWRITE; DELETE; COMMIT; ROLLBACK; FILE STATUS; EOF/INVALID KEY handling; advisory per-run locking phrases that PowerRustCOBOL supports. Do not require RELATIVE files or cross-process locking semantics.
5. Existing-code modification: add/remove/rename/refactor while preserving comments, formatting, identifiers, behavior, and required divisions/sections.
6. Bug fixing: incorrect PIC/USAGE, numeric overflow or truncation, wrong file key, invalid file status handling, incorrect OCCURS/ODO bounds, off-by-one table searches, broken nested IF/EVALUATE logic, decimal mistakes, COMP/COMP-3 misuse, stale generated handler structure.
7. PowerRustCOBOL GUI/extensions: RAD forms, controls, events, generated event handlers, Form Designer/runtime behavior, data binding, indexed-file controls, REST Client and SQL Database controls where present, themes, animation properties, Rust FFI repository objects via INVOKE, and inline method/property syntax such as `Control-1::Text`, `Control-1::Refresh()`, and `SET Control-1::ShadowEnabled TO 1`.

Out of scope for this benchmark: REPORT SECTION and report writer verbs; COMMUNICATION SECTION; field-level SCREEN SECTION editing; RELATIVE file organization; COBOL OO class/method definitions; undocumented `cbl_`/runtime helper calls; invented REST/SQLite verbs; unimplemented controls, events, properties, methods, or compiler internals.

Important PowerRustCOBOL rule: for controls and form objects, prefer inline object syntax `ControlName::Method(args)` and `ControlName::Property` / `SET ControlName::Property TO value`. Do not propose `CALL "COBOL-SET-PROPERTY"`, `CALL "COBOL-GET-PROPERTY"`, chart helper CALLs, or legacy `INVOKE Control "Method" USING ...` forms for control work. If the required method/property is not listed in context, the correct behavior is to ask for directions instead of guessing.

The report must include these sections in this order before the metrics JSON:
1. Executive summary: concise recommendation and major risks.
2. Generated COBOL sample: include every COBOL or PowerRustCOBOL code sample produced during the assessment in fenced `cobol` code blocks. Do not omit, summarize, or replace the generated code with prose.
3. Code accuracy analysis: analyze the generated COBOL sample against the benchmark rules. Explicitly discuss division completeness, DATA DIVISION correctness, PROCEDURE DIVISION behavior, file handling, inline PowerRustCOBOL object syntax, unsupported-feature avoidance, code preservation risks, formatting, and runtime plausibility. Tie each issue or strength to the relevant score.
4. Detailed tested points: describe what was tested and how the score was assigned for each metric.

Scoring guardrails:
- Do not assign 100 to any metric in this chat-only benchmark. A score of 100 is reserved for independently compiled and runtime-verified code, which this benchmark does not perform.
- Because this is not a compiler-run benchmark, `compilation_score` must be 90 or lower, `runtime_correctness` must be 85 or lower, and `overall_score` must be 95 or lower.
- If the generated code has likely parser/compiler errors, `compilation_score` must be 70 or lower.
- If method calls are written as property assignments, such as `SET Control::Refresh()`, `forms_extensions_score` and `semantic_correctness` must be reduced.
- If a GUI control is invented as a PIC X data item instead of using a real form/control context, reduce `forms_extensions_score`.
- If a string literal is moved into a numeric field, reduce `semantic_correctness`, `runtime_correctness`, and `compilation_score`.
- If indexed file code omits important INVALID KEY/AT END handling, CLOSE/COMMIT behavior, or file-status checks, reduce `file_handling_score` and `runtime_correctness`.
- The `weaknesses` array must not be empty. If no defect is found, list residual uncertainty from the lack of compiler/runtime verification.
- The report must explicitly state that the scores are model-estimated and not independently verified by PowerRustCOBOL.

Return the report in clear language, followed by one fenced JSON block named `metrics` with this exact schema:
{
  "overall_score": 0-100,
  "compilation_score": 0-100,
  "functional_score": 0-100,
  "instruction_following": 0-100,
  "semantic_correctness": 0-100,
  "code_preservation": 0-100,
  "runtime_correctness": 0-100,
  "hallucination_resistance": 0-100,
  "formatting_preservation": 0-100,
  "cobol85_score": 0-100,
  "powerrustcobol_score": 0-100,
  "program_structure_score": 0-100,
  "data_description_score": 0-100,
  "control_flow_score": 0-100,
  "file_handling_score": 0-100,
  "forms_extensions_score": 0-100,
  "unsupported_feature_avoidance": 0-100,
  "recommended_usage": "short text",
  "strengths": ["..."],
  "weaknesses": ["..."],
  "typical_failure_patterns": ["..."]
}

Be honest about uncertainty: this is a lightweight interactive benchmark, not a full compiler-run benchmark. The overall score must use these weights: 15% compilation, 30% functional correctness, 15% instruction following, 10% semantic correctness, 10% code preservation, 5% runtime correctness, 5% formatting preservation, 10% unsupported-feature avoidance. The dashboard-specific scores must reflect only the supported benchmark scope listed above."#;

pub fn spawn_cobol_proficiency_benchmark(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    if cfg.reviewer_configured() {
        return spawn_cobol_proficiency_tandem(cfg);
    }
    spawn_benchmark_primary(cfg)
}

fn benchmark_primary_prompt(cfg: &LlmConfig) -> String {
    if cfg.cobol_proficiency_prompt.trim().is_empty() {
        COBOL_PROFICIENCY_BENCHMARK_PROMPT.to_string()
    } else {
        cfg.cobol_proficiency_prompt.clone()
    }
}

fn spawn_benchmark_primary(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    let mut bench_cfg = cfg.clone();
    bench_cfg.endpoint = heal_endpoint(&bench_cfg.endpoint);
    bench_cfg.temperature = 0.0;

    let mut req = mesh_request_base(&bench_cfg);
    req.specialist = Some("CodeGenerator".to_string());
    req.system_prompt = "You are a strict COBOL-85 and PowerRustCOBOL model evaluator.".to_string();
    req.user_prompt = benchmark_primary_prompt(&bench_cfg);
    run_mesh_request(req, "COBOL proficiency benchmark")
}

/// One primary-model round with an arbitrary user prompt (revision requests).
fn spawn_benchmark_primary_with(cfg: &LlmConfig, user_prompt: String) -> Receiver<LlmResponse> {
    let mut bench_cfg = cfg.clone();
    bench_cfg.endpoint = heal_endpoint(&bench_cfg.endpoint);
    bench_cfg.temperature = 0.0;
    let mut req = mesh_request_base(&bench_cfg);
    req.specialist = Some("CodeGenerator".to_string());
    req.system_prompt = "You are a strict COBOL-85 and PowerRustCOBOL model evaluator.".to_string();
    req.user_prompt = user_prompt;
    run_mesh_request(req, "COBOL proficiency benchmark (revision)")
}

/// One Pedantic Agent round. `final_round` switches the tooling contract to
/// the final-assessment JSON (metrics schema + pedantic_final).
fn spawn_pedantic_review(
    cfg: &LlmConfig,
    primary_prompt: &str,
    answer: &str,
    context: &str,
    final_round: bool,
) -> Receiver<LlmResponse> {
    let mut rev_cfg = cfg.reviewer_config();
    rev_cfg.endpoint = heal_endpoint(&rev_cfg.endpoint);
    rev_cfg.temperature = 0.0;
    let mut req = mesh_request_base(&rev_cfg);
    req.specialist = Some("CodeGenerator".to_string());
    req.system_prompt = if cfg.pedantic_prompt.trim().is_empty() {
        default_pedantic_prompt()
    } else {
        cfg.pedantic_prompt.clone()
    };
    let phase = if final_round {
        "This is the FINAL assessment round: produce the brutally honest final \
         assessment and END with the final metrics JSON per your tooling \
         contract (the metrics schema is defined in the authoritative primary \
         prompt below; include \"pedantic_final\": true)."
    } else {
        "This is a review round: review the response and END with the \
         round-verdict JSON per your tooling contract."
    };
    req.user_prompt = format!(
        "{phase}\n\n=== AUTHORITATIVE PRIMARY PROMPT (the specification) ===\n{primary_prompt}\n\n{context}=== PRIMARY AGENT RESPONSE UNDER REVIEW ===\n{answer}"
    );
    run_mesh_request(req, "Pedantic Agent review")
}

/// Forward one worker's stream to the tandem output, returning the final text.
fn drain_round(rx: Receiver<LlmResponse>, tx: &mpsc::Sender<LlmResponse>) -> Result<String, ()> {
    loop {
        match rx.recv() {
            Ok(LlmResponse::Chunk(c)) => {
                let _ = tx.send(LlmResponse::Chunk(c));
            }
            Ok(LlmResponse::Ok(full)) => return Ok(full),
            Ok(LlmResponse::Err(e)) => {
                let _ = tx.send(LlmResponse::Err(e));
                return Err(());
            }
            Err(_) => {
                let _ = tx.send(LlmResponse::Err(
                    "The benchmark worker stopped unexpectedly.".into(),
                ));
                return Err(());
            }
        }
    }
}

/// Last fenced JSON block of a review, parsed.
fn pedantic_round_json(review: &str) -> Option<serde_json::Value> {
    let mut last = None;
    let mut rest = review;
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

/// The tandem COBOL-proficiency run (spec: operator, 2026-07-16): primary
/// benchmark → Pedantic review → (on defects) one full-replacement revision by
/// the primary → Pedantic FINAL assessment with the authoritative scores. All
/// rounds stream into one transcript, separated by section headers.
fn spawn_cobol_proficiency_tandem(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        let hdr = |t: &str| format!("\n\n━━━ {t} ━━━\n\n");
        let mut transcript = String::new();
        let mut section = |tx: &mpsc::Sender<LlmResponse>, transcript: &mut String, title: &str| {
            let h = hdr(title);
            let _ = tx.send(LlmResponse::Chunk(h.clone()));
            transcript.push_str(&h);
        };
        let primary_prompt = benchmark_primary_prompt(&cfg);

        section(&tx, &mut transcript, "PRIMARY AGENT — BENCHMARK RUN");
        let Ok(a1) = drain_round(spawn_benchmark_primary(&cfg), &tx) else {
            return;
        };
        transcript.push_str(&a1);

        section(&tx, &mut transcript, "PEDANTIC AGENT — REVIEW");
        let Ok(r1) = drain_round(
            spawn_pedantic_review(&cfg, &primary_prompt, &a1, "", false),
            &tx,
        ) else {
            return;
        };
        transcript.push_str(&r1);

        let verdict = pedantic_round_json(&r1);
        let defects = verdict
            .as_ref()
            .and_then(|v| v.get("pedantic_verdict"))
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("defects"))
            // No parseable verdict: be pedantic about the pedant and assume
            // defects so the loop still exercises the revision round.
            .unwrap_or(true);

        let final_answer = if defects {
            let correction = verdict
                .as_ref()
                .and_then(|v| v.get("correction_request"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            section(&tx, &mut transcript, "PRIMARY AGENT — FULL REVISION");
            let revision_prompt = format!(
                "{primary_prompt}\n\n=== YOUR PREVIOUS COMPLETE RESPONSE ===\n{a1}\n\n=== PEDANTIC AGENT CORRECTION REQUEST ===\n{}\n\nCorrect the defects and submit the COMPLETE response again. The revised submission must fully replace the defective version — do not send isolated patches.",
                if correction.trim().is_empty() { r1.as_str() } else { correction.as_str() },
            );
            let Ok(a2) = drain_round(spawn_benchmark_primary_with(&cfg, revision_prompt), &tx)
            else {
                return;
            };
            transcript.push_str(&a2);
            a2
        } else {
            a1
        };

        section(&tx, &mut transcript, "PEDANTIC AGENT — FINAL ASSESSMENT");
        let revised_note = if defects {
            "The response below is the REVISED submission; re-review the whole \
             response for newly introduced errors, regressions, and remaining \
             violations — never accept it merely because the listed defects \
             were addressed.\n\n"
        } else {
            ""
        };
        let Ok(fin) = drain_round(
            spawn_pedantic_review(&cfg, &primary_prompt, &final_answer, revised_note, true),
            &tx,
        ) else {
            return;
        };
        transcript.push_str(&fin);

        let _ = tx.send(LlmResponse::Ok(transcript));
    });
    rx
}

pub fn spawn_detect(endpoint: &str) -> Receiver<Result<DetectedApi, String>> {
    let (tx, rx) = mpsc::channel();
    let _ep = endpoint.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(Err(
            "Auto-detect not implemented. Please manually select a provider.".into(),
        ));
    });
    rx
}

pub fn spawn_list_models(
    provider: Provider,
    endpoint: &str,
    key: &str,
) -> Receiver<Result<Vec<String>, String>> {
    let (tx, rx) = mpsc::channel();
    let ep = heal_endpoint(endpoint);
    let key = key.to_string();
    let pid = provider.id().to_string();

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Err(format!("Failed to start async runtime: {}", e)));
                return;
            }
        };
        rt.block_on(async {
            let client = match reqwest::Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to create HTTP client: {}", e)));
                    return;
                }
            };
            if pid == "ollama" {
                let url = format!(
                    "{}/api/tags",
                    ep.trim_end_matches("/api")
                        .trim_end_matches("/v1")
                        .trim_end_matches('/')
                );
                if let Ok(res) = client.get(&url).send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                            let mut names = Vec::new();
                            for m in models {
                                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                                    names.push(name.to_string());
                                }
                            }
                            let _ = tx.send(Ok(filter_retired_models(names)));
                            return;
                        }
                    }
                }
                let _ = tx.send(Err("Failed to fetch models from Ollama".into()));
            } else {
                let url = if ep.ends_with("/chat/completions") {
                    ep.replace("/chat/completions", "/models")
                } else if ep.ends_with("/v1") {
                    format!("{}/models", ep)
                } else if ep.ends_with('/') {
                    format!("{}v1/models", ep)
                } else {
                    format!("{}/v1/models", ep)
                };

                let mut req = client.get(&url);
                if !key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", key));
                } else if pid == "gemini" {
                    // Google Gemini uses x-goog-api-key or key= in query
                    req = req.header("x-goog-api-key", key.clone());
                }

                if let Ok(res) = req.send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            let mut names = Vec::new();
                            for m in data {
                                if let Some(id) = m.get("id").and_then(|n| n.as_str()) {
                                    names.push(id.to_string());
                                }
                            }
                            let _ = tx.send(Ok(filter_retired_models(names)));
                            return;
                        }
                        if let Some(models) = json.get("models").and_then(|d| d.as_array()) {
                            // Gemini might use this
                            let mut names = Vec::new();
                            for m in models {
                                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                                    names.push(name.to_string().replace("models/", ""));
                                }
                            }
                            let _ = tx.send(Ok(filter_retired_models(names)));
                            return;
                        }
                    }
                }
                let _ = tx.send(Err("Failed to fetch models from API".into()));
            }
        });
    });
    rx
}

fn filter_retired_models(models: Vec<String>) -> Vec<String> {
    models
        .into_iter()
        .filter(|m| retired_model_message(m).is_none())
        .collect()
}

pub fn has_connection_log() -> bool {
    true
}
pub fn connection_log_text() -> String {
    CONNECTION_LOG.lock().map(|l| l.clone()).unwrap_or_default()
}
pub fn clear_connection_log() {
    if let Ok(mut l) = CONNECTION_LOG.lock() {
        l.clear();
    }
}
pub fn push_connection_log(text: &str) {
    if let Ok(mut l) = CONNECTION_LOG.lock() {
        l.push_str(text);
        l.push('\n');
    }
}

pub struct AiLogEntry {
    pub kind: AiLogKind,
    pub text: String,
}

pub fn drain_ai_log() -> Vec<AiLogEntry> {
    if let Ok(mut q) = AI_LOG_QUEUE.lock() {
        std::mem::take(&mut *q)
    } else {
        vec![]
    }
}

pub fn normalize_comments(code: &str) -> String {
    code.to_string()
}
pub fn extract_code(reply: &str) -> Option<String> {
    let lower = reply.to_lowercase();
    if let Some(start) = lower.find("```cobol") {
        if let Some(end) = reply[start + 8..].find("```") {
            return Some(reply[start + 8..start + 8 + end].trim().to_string());
        }
    }
    if let Some(start) = reply.find("```") {
        if let Some(nl) = reply[start..].find('\n') {
            let body_start = start + nl + 1;
            if let Some(end) = reply[body_start..].find("```") {
                return Some(reply[body_start..body_start + end].trim().to_string());
            }
        }
    }
    None
}
pub fn ai_question(_reply: &str) {}

pub struct MeshSession {
    orchestrator: Orchestrator,
}

impl MeshSession {
    pub fn new() -> Self {
        Self {
            orchestrator: Orchestrator::new(),
        }
    }

    pub fn execute_request(&self, _request: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_qwen_coder_next_is_blocked_and_filtered() {
        assert!(retired_model_message("qwen3-coder-next").is_some());
        assert!(retired_model_message("QWEN3-CODER-NEXT").is_some());
        assert!(retired_model_message("qwen3-coder-plus").is_none());

        let models = filter_retired_models(vec![
            "qwen3-coder-next".into(),
            "qwen3-coder-plus".into(),
            "gpt-5".into(),
        ]);
        assert_eq!(models, vec!["qwen3-coder-plus", "gpt-5"]);
    }
}
