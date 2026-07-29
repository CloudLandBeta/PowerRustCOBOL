// SPDX-License-Identifier: Apache-2.0

//! Rig-based transport for project agents (Grace, specialists, Pedantic
//! reviewers) — replaces the hand-rolled HTTP orchestrator for the agentic
//! workflow path (Rig migration, phase 1).
//!
//! Design decisions (see the Rig alignment report):
//! - Every OpenAI-compatible provider (OpenAI, Ollama's `/v1` endpoint,
//!   Mistral, Groq, …) goes through rig's openai **chat-completions** client
//!   (`CompletionsClient`) with a custom base URL — never the Responses-API
//!   default, which compatible gateways do not reliably implement. Anthropic
//!   goes through its native client. No wire-format sniffing.
//! - No hooks in v1: token usage is read from the completion response's
//!   `usage`, and tool governance is by construction (an agent only receives
//!   the tools it declares), so nothing here depends on post-0.40 APIs.
//! - The blocking wrapper mirrors the previous per-request-runtime pattern so
//!   the synchronous `AgentInvoker` seam in `grace.rs` is preserved.

use futures::StreamExt;
use rig_core::client::CompletionClient;
use rig_core::completion::Completion;
use rig_core::completion::GetTokenUsage;
use rig_core::message::{AssistantContent, Message, Reasoning};
use rig_core::providers::{anthropic, openai};
use rig_core::streaming::{StreamedAssistantContent, StreamingCompletion};
use rig_core::tool::{Tool, ToolDyn};
use std::sync::Arc;

/// Host-supplied executor for one native tool: takes the model's JSON
/// arguments, returns the tool output (or an error string the model sees).
/// The host closure is also responsible for recording tool evidence.
pub type HostToolFn = Arc<dyn Fn(serde_json::Value) -> Result<String, String> + Send + Sync>;

/// Native tools the host grants THIS agent for THIS call. `None` = the tool is
/// not declared for the agent, so the model never sees its definition —
/// governance by construction, no policing prompt text required.
#[derive(Clone, Default)]
pub struct AgentTools {
    pub knowledge_search: Option<HostToolFn>,
    pub egui_tree: Option<HostToolFn>,
    pub egui_rects: Option<HostToolFn>,
}

impl AgentTools {
    pub fn is_empty(&self) -> bool {
        self.knowledge_search.is_none() && self.egui_tree.is_none() && self.egui_rects.is_none()
    }
}

/// Upper bound on tool rounds per call (mirrors the previous MAX_TOOL_ROUNDS
/// guard against a model that never stops calling tools).
const MAX_TOOL_ROUNDS: usize = 6;

/// Upper bound on continuation pages per reply. A reply that hits the output
/// cap is RESUMED rather than abandoned (see `continue_if_capped`); this bounds
/// the paging the same way MAX_TOOL_ROUNDS bounds tool calls, so a model that
/// never stops producing cannot spin forever.
const MAX_CONTINUATION_PAGES: u32 = 6;

/// Prompt that resumes a reply the provider cut off at the output cap. It must
/// forbid every form of restatement: the pages are concatenated verbatim, so a
/// model that re-introduces its answer would corrupt the artifact (a fenced
/// JSON block reopened mid-stream stops parsing).
const CONTINUE_DIRECTIVE: &str = "Your previous message was cut off because it reached the output token limit. Continue it from exactly where it stopped. Do not repeat any text you already sent, do not summarise it, do not restate the task, and do not add any preamble, apology, or heading — resume mid-sentence, or mid-word, if that is where it ended. If the cut-off point was inside a fenced code or JSON block, continue inside that block and close it properly.";

/// Whether the provider stopped because the OUTPUT CAP was reached rather than
/// because the answer was finished.
///
/// This is read from the raw provider payload instead of a rig abstraction:
/// rig 0.40 surfaces `choice` and `usage` but no normalised stop reason, while
/// both raw response types are `Serialize` by the `CompletionModel::Response`
/// bound. Anthropic reports `stop_reason: "max_tokens"` at the top level;
/// the OpenAI wire reports `finish_reason: "length"` per choice.
///
/// Silence here is what made truncation invisible: a capped reply is a normal
/// 200 carrying a mid-sentence body, indistinguishable from a complete one.
fn hit_output_cap<T: serde::Serialize>(raw: &T) -> bool {
    let Ok(value) = serde_json::to_value(raw) else {
        return false;
    };
    if value
        .get("stop_reason")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|reason| reason.eq_ignore_ascii_case("max_tokens"))
    {
        return true;
    }
    value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|choices| {
            choices.iter().any(|choice| {
                choice
                    .get("finish_reason")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|reason| reason.eq_ignore_ascii_case("length"))
            })
        })
}

/// Real OpenAI rejects the classic `max_tokens` on current chat models
/// ("Unsupported parameter … use 'max_completion_tokens' instead") while every
/// other OpenAI-compatible gateway still speaks `max_tokens` — the same
/// provider-keyed switch the legacy transport used. Rig's builder only knows
/// `max_tokens`, so for OpenAI the limit rides in as an additional parameter.
fn uses_max_completion_tokens(provider: &str) -> bool {
    provider.trim().eq_ignore_ascii_case("openai")
}

/// Anthropic model ids whose generation still accepts the sampling parameters.
/// Matched as a **prefix**, so dated snapshots (`claude-opus-4-5-20251101`) are
/// covered by their family entry.
const ANTHROPIC_SAMPLING_MODELS: &[&str] = &[
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-opus-4-1",
    "claude-opus-4-0",
    "claude-opus-4-2",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5",
    "claude-sonnet-4-0",
    "claude-sonnet-4-2",
    "claude-haiku-4-5",
    // Every Claude 3.x and the legacy 2.x line.
    "claude-3",
    "claude-2",
];

/// Whether `temperature` may be sent for this provider and model.
///
/// Anthropic **removed** the sampling parameters from Claude Opus 4.7 onward —
/// Opus 4.7/4.8, Opus 5, Sonnet 5, Fable 5 and Mythos 5 answer a request
/// carrying `temperature` with a 400 (`temperature is deprecated for this
/// model`), which fails the whole call rather than degrading.
///
/// The list above is therefore an **allowlist, not a denylist**: an Anthropic
/// model this build has never heard of — every model released after it — is
/// assumed to reject the parameter. That is the fail-safe direction. Omitting
/// `temperature` costs only the configured value, which those models ignore
/// anyway; sending it costs the entire request. Every other provider keeps
/// receiving it exactly as before.
///
/// Public because the same split identifies the REASONING-FIRST generation:
/// exactly the models outside this allowlist think before they answer and can
/// spend a small `max_tokens` budget entirely on hidden reasoning. The host's
/// model-policy layer (`cobolt-ide::model_policy`) derives its output-budget
/// floor from `!sends_sampling_params(..)`, so a new Anthropic model is
/// covered on release without editing any list — do not narrow this function's
/// contract without checking that consumer.
pub fn sends_sampling_params(provider: &str, model: &str) -> bool {
    if !provider.trim().eq_ignore_ascii_case("anthropic") {
        return true;
    }
    let model = model.trim().to_ascii_lowercase();
    // Gateways that re-expose Anthropic prefix the provider onto the id.
    let model = model.strip_prefix("anthropic.").unwrap_or(&model);
    ANTHROPIC_SAMPLING_MODELS
        .iter()
        .any(|allowed| model.starts_with(allowed))
}

/// One agent invocation: everything the transport needs, composed by the host
/// (per-agent model config from the agents DB + the engine's task prompt).
#[derive(Clone)]
pub struct AgentCall {
    /// Provider id from the model profile (lowercase, e.g. "openai",
    /// "anthropic", "ollama").
    pub provider: String,
    pub model: String,
    pub api_key: String,
    /// Configured endpoint; may carry legacy suffixes like
    /// `/chat/completions` or `/api/chat` which are normalised away.
    pub endpoint: String,
    /// The agent's core instructions (Rig preamble).
    pub system_prompt: String,
    /// Reference material (skills) — attached as a static context document.
    pub skills: String,
    /// The task prompt for this call.
    pub user_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Native tools granted to this agent (empty = plain completion).
    pub tools: AgentTools,
}

/// The transport's result: final text plus exact token usage as reported by
/// the provider (no log scraping).
#[derive(Debug, Clone, Default)]
pub struct AgentReply {
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The reply is STILL incomplete: the output cap was hit and continuation
    /// paging could not finish it within [`MAX_CONTINUATION_PAGES`]. Callers
    /// must not treat such text as a complete artifact — a verdict, plan, or
    /// change-set parsed out of it may be missing its tail.
    pub truncated: bool,
    /// How many continuation requests were needed to complete the reply.
    /// Zero for the overwhelming majority of calls.
    pub continuation_pages: u32,
}

/// Normalise a configured endpoint to the base URL a Rig provider client
/// expects. Legacy configs store full request URLs; Rig wants the API root.
/// Public so hosts can log the true request root in verbose mode.
pub fn normalize_base(provider: &str, endpoint: &str) -> String {
    let mut base = endpoint.trim().trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/completions", "/messages"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
        }
    }
    // Ollama's native `/api/chat` maps onto its OpenAI-compatible `/v1` root.
    if let Some(host) = base.strip_suffix("/api/chat").or_else(|| base.strip_suffix("/api")) {
        base = format!("{}/v1", host.trim_end_matches('/'));
    }
    // HuggingFace's legacy Inference API host was shut down (its DNS no
    // longer resolves); its path scheme died with it. Any stored endpoint
    // pointing there maps wholesale onto the Inference Providers router,
    // which speaks the OpenAI wire at /v1.
    if base.contains("api-inference.huggingface.co") {
        base = "https://router.huggingface.co/v1".to_string();
    }
    if base.is_empty() {
        base = match provider {
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
    }
    base
}

/// How long establishing the TCP/TLS connection to a provider may take.
/// Only the connect phase is bounded — streaming reads stay unlimited,
/// because a long generation is legitimate while an unreachable or
/// black-holed host is not (observed live: a dead provider held the COBOL
/// proficiency spinner open forever, with no way out but restarting the IDE).
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// The HTTP client injected into every rig provider client (same reqwest
/// version rig-core itself resolves to, so the type satisfies HttpClientExt).
fn transport_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|e| format!("transport client build failed: {e}"))
}

/// Invoke one agent synchronously. Creates a runtime per call — cheap next to
/// LLM latency and identical to the previous transport's threading model, so
/// callers on worker threads keep working unchanged.
pub fn run_agent_blocking(call: &AgentCall) -> Result<AgentReply, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to start async runtime: {e}"))?;
    rt.block_on(run_agent(call))
}

/// Invoke one agent asynchronously.
pub async fn run_agent(call: &AgentCall) -> Result<AgentReply, String> {
    let base = normalize_base(&call.provider, &call.endpoint);
    if call.provider.eq_ignore_ascii_case("anthropic") {
        let client = anthropic::Client::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("anthropic client build failed: {e}"))?;
        complete_with(client, call).await
    } else {
        // Every other configured provider speaks the OpenAI wire at its base
        // URL (Ollama via /v1); one client covers them all.
        // The chat-completions client, NOT the default `openai::Client`: rig
        // 0.40's default routes through the OpenAI Responses API
        // (`/responses`), which "OpenAI-compatible" gateways implement
        // partially or not at all (Ollama Cloud, for one, echoes tool
        // definitions with `"strict": null` where rig's Responses types
        // demand a boolean — a hard parse failure). `/chat/completions` is
        // the actual compatibility wire every configured provider speaks.
        let client = openai::CompletionsClient::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("openai-compatible client build failed: {e}"))?;
        complete_with(client, call).await
    }
}

/// One typed-extraction invocation (Rig migration, phase 3): pull a
/// schema-shaped value out of `source_text` with the provider's native
/// forced-tool-call extraction (`submit`). Used as the recovery path when a
/// reply's deterministic fenced-JSON parse fails — it replaces the old
/// "malformed plan, please resend everything" correction roundtrips.
#[derive(Clone)]
pub struct ExtractCall {
    /// Provider id from the model profile (lowercase, e.g. "openai").
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub endpoint: String,
    /// Extraction-specific instructions appended to Rig's extractor preamble.
    pub preamble: String,
    pub max_tokens: u32,
}

/// A typed extraction result plus exact token usage (all attempts included).
#[derive(Debug, Clone)]
pub struct ExtractedReply<T> {
    pub data: T,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Retries Rig performs internally when the model fails to call `submit` with
/// schema-valid arguments. Bounded like MAX_TOOL_ROUNDS: never an open loop.
const EXTRACT_RETRIES: u64 = 2;

/// Extract a `T` from `source_text` synchronously (worker-thread callers).
pub fn extract_typed_blocking<T>(call: &ExtractCall, source_text: &str) -> Result<ExtractedReply<T>, String>
where
    T: schemars::JsonSchema + serde::de::DeserializeOwned + serde::Serialize + Send + Sync + 'static,
{
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to start async runtime: {e}"))?;
    rt.block_on(extract_typed::<T>(call, source_text))
}

/// Extract a `T` from `source_text` asynchronously.
pub async fn extract_typed<T>(call: &ExtractCall, source_text: &str) -> Result<ExtractedReply<T>, String>
where
    T: schemars::JsonSchema + serde::de::DeserializeOwned + serde::Serialize + Send + Sync + 'static,
{
    let base = normalize_base(&call.provider, &call.endpoint);
    if call.provider.eq_ignore_ascii_case("anthropic") {
        let client = anthropic::Client::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("anthropic client build failed: {e}"))?;
        extract_with::<_, T>(client, call, source_text).await
    } else {
        // The chat-completions client, NOT the default `openai::Client`: rig
        // 0.40's default routes through the OpenAI Responses API
        // (`/responses`), which "OpenAI-compatible" gateways implement
        // partially or not at all (Ollama Cloud, for one, echoes tool
        // definitions with `"strict": null` where rig's Responses types
        // demand a boolean — a hard parse failure). `/chat/completions` is
        // the actual compatibility wire every configured provider speaks.
        let client = openai::CompletionsClient::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("openai-compatible client build failed: {e}"))?;
        extract_with::<_, T>(client, call, source_text).await
    }
}

async fn extract_with<C, T>(client: C, call: &ExtractCall, source_text: &str) -> Result<ExtractedReply<T>, String>
where
    C: CompletionClient,
    T: schemars::JsonSchema + serde::de::DeserializeOwned + serde::Serialize + Send + Sync + 'static,
{
    let mut builder = client.extractor::<T>(&call.model).preamble(&call.preamble);
    builder = if uses_max_completion_tokens(&call.provider) {
        builder.additional_params(serde_json::json!({
            "max_completion_tokens": call.max_tokens
        }))
    } else {
        builder.max_tokens(call.max_tokens as u64)
    };
    let extractor = builder.retries(EXTRACT_RETRIES).build();
    let response = extractor
        .extract_with_usage(source_text)
        .await
        .map_err(|e| format!("typed extraction failed: {e}"))?;
    Ok(ExtractedReply {
        data: response.data,
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
    })
}

/// One streamed chat invocation (Rig migration, phase 4): the transport for
/// the IDE's single-agent entry points (AI Dev Agent, editor assistant,
/// history compaction, connection test) — replaces the hand-rolled HTTP
/// orchestrator's request loop. Prior turns ride along as history and text
/// deltas stream through `on_chunk` as the model produces them.
#[derive(Clone)]
pub struct ChatCall {
    /// Provider id from the model profile (lowercase, e.g. "openai").
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub endpoint: String,
    /// Full system prompt composed by the host (host prompt + any specialist
    /// preamble) — the transport never invents or drops it.
    pub system_prompt: String,
    /// Reference material (skills) — attached as a static context document.
    pub skills: String,
    /// Prior conversation turns as (role, content); any non-"user" role is an
    /// assistant turn.
    pub history: Vec<(String, String)>,
    /// The final user message (context already appended by the host).
    pub user_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

/// Invoke one streamed chat synchronously (worker-thread callers).
pub fn run_chat_blocking(
    call: &ChatCall,
    on_chunk: &(dyn Fn(&str) + Send + Sync),
) -> Result<AgentReply, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to start async runtime: {e}"))?;
    rt.block_on(run_chat(call, on_chunk))
}

/// Invoke one streamed chat asynchronously.
pub async fn run_chat(
    call: &ChatCall,
    on_chunk: &(dyn Fn(&str) + Send + Sync),
) -> Result<AgentReply, String> {
    let base = normalize_base(&call.provider, &call.endpoint);
    if call.provider.eq_ignore_ascii_case("anthropic") {
        let client = anthropic::Client::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("anthropic client build failed: {e}"))?;
        chat_with(client, call, on_chunk).await
    } else {
        // The chat-completions client, NOT the default `openai::Client`: rig
        // 0.40's default routes through the OpenAI Responses API
        // (`/responses`), which "OpenAI-compatible" gateways implement
        // partially or not at all (Ollama Cloud, for one, echoes tool
        // definitions with `"strict": null` where rig's Responses types
        // demand a boolean — a hard parse failure). `/chat/completions` is
        // the actual compatibility wire every configured provider speaks.
        let client = openai::CompletionsClient::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .http_client(transport_client()?)
            .build()
            .map_err(|e| format!("openai-compatible client build failed: {e}"))?;
        chat_with(client, call, on_chunk).await
    }
}

async fn chat_with<C>(
    client: C,
    call: &ChatCall,
    on_chunk: &(dyn Fn(&str) + Send + Sync),
) -> Result<AgentReply, String>
where
    C: CompletionClient,
{
    let mut builder = client.agent(&call.model).preamble(&call.system_prompt);
    if sends_sampling_params(&call.provider, &call.model) {
        builder = builder.temperature(call.temperature as f64);
    }
    builder = if uses_max_completion_tokens(&call.provider) {
        builder.additional_params(serde_json::json!({
            "max_completion_tokens": call.max_tokens
        }))
    } else {
        builder.max_tokens(call.max_tokens as u64)
    };
    if !call.skills.trim().is_empty() {
        builder = builder.context(&call.skills);
    }
    let agent = builder.build();

    let history: Vec<Message> = call
        .history
        .iter()
        .map(|(role, content)| {
            if role == "user" {
                Message::user(content.clone())
            } else {
                Message::assistant(content.clone())
            }
        })
        .collect();

    let mut stream = agent
        .stream_completion(call.user_prompt.as_str(), history)
        .await
        .map_err(|e| format!("completion setup failed: {e}"))?
        .stream()
        .await
        .map_err(|e| format!("model request failed: {e}"))?;

    let mut text = String::new();
    let mut reasoning_chars = 0usize;
    while let Some(item) = stream.next().await {
        match item.map_err(|e| format!("stream error: {e}"))? {
            StreamedAssistantContent::Text(t) => {
                on_chunk(&t.text);
                text.push_str(&t.text);
            }
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                reasoning_chars += reasoning.chars().count();
            }
            StreamedAssistantContent::Reasoning(_) => {
                reasoning_chars += 1;
            }
            _ => {}
        }
    }
    let usage = stream.response.token_usage();

    if text.trim().is_empty() && reasoning_chars > 0 {
        return Err(format!(
            "The model returned {reasoning_chars} reasoning character(s) but no assistant \
             message content. PowerRustCOBOL cannot apply hidden reasoning as form \
             operations. Use a non-reasoning/chat model or disable thinking/reasoning for \
             this model."
        ));
    }
    if text.trim().is_empty() {
        return Err("the model returned no assistant text".to_string());
    }
    // Streamed chat feeds the IDE chatbot, where the developer sees the reply
    // arrive and can simply ask for more; continuation paging is reserved for
    // the agent path, whose replies are parsed as artifacts.
    Ok(AgentReply {
        text,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        truncated: false,
        continuation_pages: 0,
    })
}

/// Error type for host-executed tools; the message is fed back to the model.
#[derive(Debug)]
pub struct HostToolError(pub String);
impl std::fmt::Display for HostToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for HostToolError {}

/// Declare one concrete Rig tool backed by a host closure. Concrete types keep
/// us on the stable `Tool` trait (const NAME) instead of any dynamic-dispatch
/// API whose 0.40 surface is unverified.
macro_rules! host_tool {
    ($ty:ident, $name:literal, $desc:literal, $params:expr) => {
        struct $ty(HostToolFn);
        impl Tool for $ty {
            const NAME: &'static str = $name;
            type Error = HostToolError;
            type Args = serde_json::Value;
            type Output = String;
            fn description(&self) -> String {
                $desc.to_string()
            }
            fn parameters(&self) -> serde_json::Value {
                $params
            }
            async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                (self.0)(args).map_err(HostToolError)
            }
        }
    };
}

host_tool!(
    KnowledgeSearchTool,
    "knowledge_search",
    "Search the project-local Knowledge Base (SQLite vector index) for prior plans, requirements, decisions, and documentation. Returns PATH/SCORE/EXCERPT blocks.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "What to search for" },
            "limit": { "type": "integer", "description": "Max results (1-10)", "default": 5 }
        },
        "required": ["query"]
    })
);
host_tool!(
    EguiTreeTool,
    "egui_tree",
    "Read the live rendered UI widget tree (READ-ONLY observation of the IDE window; never a valid edit target). Returns one line per named node: role, label, bounds.",
    serde_json::json!({ "type": "object", "properties": {} })
);
host_tool!(
    EguiRectsTool,
    "egui_rects",
    "Read the live rendered UI widget geometry (READ-ONLY; alias of egui_tree emphasising bounds).",
    serde_json::json!({ "type": "object", "properties": {} })
);

/// Hidden reasoning seen in one response: how many blocks, and how much text
/// they rendered to (encrypted blocks render to nothing, hence both counters).
#[derive(Default)]
struct ReasoningTally {
    blocks: usize,
    chars: usize,
}

impl ReasoningTally {
    fn add(&mut self, reasoning: &Reasoning) {
        self.blocks += 1;
        self.chars += reasoning.display_text().chars().count();
    }
}

/// The error for a response that carried no assistant text.
///
/// The wording always contains "no assistant text": the host classifies that
/// phrase as retryable and answers it by dropping native tools and raising the
/// token budget (`grace_host::is_transient_model_error`). What follows names
/// the cause, because the two causes need different operator action — a model
/// that spends the whole budget on hidden reasoning is a model-choice problem,
/// while a bare cap hit is a budget problem.
fn empty_reply_error(capped: bool, reasoning: &ReasoningTally) -> String {
    let mut msg = String::from("the model returned no assistant text");
    if reasoning.blocks > 0 {
        msg.push_str(&format!(
            " — the reply carried only hidden reasoning ({} block(s), {} rendered character(s)), which cannot be applied as a plan, change-set, or review",
            reasoning.blocks, reasoning.chars
        ));
    }
    if capped {
        msg.push_str(if reasoning.blocks > 0 {
            "; the output cap (max_tokens) was reached before any assistant text was produced"
        } else {
            " — the output cap (max_tokens) was reached before any assistant text was produced"
        });
    }
    msg
}

/// A reply assembled from one or more provider responses. The pages are joined
/// with NO separator: page N+1 resumes page N mid-sentence, so any inserted
/// character would land inside the artifact.
struct ContinuedReply {
    text: String,
    truncated: bool,
    pages: u32,
}

impl ContinuedReply {
    fn start(text: String, capped: bool) -> Self {
        Self {
            text,
            truncated: capped,
            pages: 0,
        }
    }

    /// Request further pages until the model stops for its own reasons, the
    /// page budget runs out, or it has nothing left to add. `base_history` is
    /// the conversation up to and including the user turn being answered; each
    /// page re-sends it with the text so far as the assistant turn, which is
    /// what lets the model see exactly where to resume.
    async fn fill<M>(
        &mut self,
        agent: &rig_core::agent::Agent<M>,
        base_history: Vec<Message>,
        total_in: &mut u64,
        total_out: &mut u64,
    ) -> Result<(), String>
    where
        M: rig_core::completion::CompletionModel,
    {
        while self.truncated && self.pages < MAX_CONTINUATION_PAGES {
            self.pages += 1;
            let mut history = base_history.clone();
            history.push(Message::assistant(self.text.clone()));
            let response = agent
                .completion(CONTINUE_DIRECTIVE, history)
                .await
                .map_err(|e| format!("continuation setup failed: {e}"))?
                .send()
                .await
                .map_err(|e| format!("continuation request failed: {e}"))?;
            *total_in += response.usage.input_tokens;
            *total_out += response.usage.output_tokens;
            let capped = hit_output_cap(&response.raw_response);
            let mut more = String::new();
            for content in response.choice.into_iter() {
                if let AssistantContent::Text(t) = content {
                    more.push_str(&t.text);
                }
            }
            // The model added nothing: it considers the answer finished even
            // though the provider reported the cap. Stop instead of spending
            // the remaining pages on empty round-trips.
            if more.trim().is_empty() {
                self.truncated = false;
                break;
            }
            self.text.push_str(&more);
            self.truncated = capped;
        }
        Ok(())
    }
}

/// Shared completion body: build the Rig agent from the profile, then run a
/// bounded tool loop — the model's native tool calls are executed through the
/// host closures and their results fed back, all inside this one call.
async fn complete_with<C>(client: C, call: &AgentCall) -> Result<AgentReply, String>
where
    C: CompletionClient,
{
    let mut builder = client.agent(&call.model).preamble(&call.system_prompt);
    if sends_sampling_params(&call.provider, &call.model) {
        builder = builder.temperature(call.temperature as f64);
    }
    builder = if uses_max_completion_tokens(&call.provider) {
        builder.additional_params(serde_json::json!({
            "max_completion_tokens": call.max_tokens
        }))
    } else {
        builder.max_tokens(call.max_tokens as u64)
    };
    if !call.skills.trim().is_empty() {
        builder = builder.context(&call.skills);
    }
    // Attach tool DEFINITIONS so they reach the request; execution below goes
    // through the same host closures. `.tools()` is called once because the
    // builder is type-state (NoTools -> WithBuilderTools).
    let mut toolbox: Vec<Box<dyn ToolDyn>> = Vec::new();
    if let Some(f) = &call.tools.knowledge_search {
        toolbox.push(Box::new(KnowledgeSearchTool(f.clone())));
    }
    if let Some(f) = &call.tools.egui_tree {
        toolbox.push(Box::new(EguiTreeTool(f.clone())));
    }
    if let Some(f) = &call.tools.egui_rects {
        toolbox.push(Box::new(EguiRectsTool(f.clone())));
    }
    let agent = if toolbox.is_empty() {
        builder.build()
    } else {
        builder.tools(toolbox).build()
    };

    let mut history: Vec<Message> = Vec::new();
    let mut prompt = call.user_prompt.clone();
    let mut total_in = 0u64;
    let mut total_out = 0u64;

    for _round in 0..=MAX_TOOL_ROUNDS {
        let response = agent
            .completion(prompt.as_str(), history.clone())
            .await
            .map_err(|e| format!("completion setup failed: {e}"))?
            .send()
            .await
            .map_err(|e| format!("model request failed: {e}"))?;
        total_in += response.usage.input_tokens;
        total_out += response.usage.output_tokens;
        let capped = hit_output_cap(&response.raw_response);

        let mut text = String::new();
        let mut tool_calls: Vec<(String, serde_json::Value)> = Vec::new();
        // Reasoning blocks carry no artifact — a change-set, a plan, a verdict
        // are all assistant TEXT — but they are the usual reason a reply comes
        // back with none, so they are counted for the diagnostic below rather
        // than silently dropped.
        let mut reasoning = ReasoningTally::default();
        for content in response.choice.into_iter() {
            match content {
                AssistantContent::Text(t) => text.push_str(&t.text),
                AssistantContent::ToolCall(tc) => {
                    tool_calls.push((tc.function.name.clone(), tc.function.arguments.clone()));
                }
                AssistantContent::Reasoning(r) => reasoning.add(&r),
                _ => {}
            }
        }

        if tool_calls.is_empty() {
            if text.trim().is_empty() {
                return Err(empty_reply_error(capped, &reasoning));
            }
            // A long reply (a verbose Pedantic review, a large change-set) can
            // exhaust the output budget mid-sentence. Resume it page by page
            // instead of returning the fragment: raising max_tokens is not an
            // alternative, since a model's real output ceiling is fixed and
            // asking past it is an HTTP 400.
            let mut paged = ContinuedReply::start(text, capped);
            if paged.truncated {
                let mut base = history.clone();
                base.push(Message::user(prompt.clone()));
                paged.fill(&agent, base, &mut total_in, &mut total_out).await?;
            }
            return Ok(AgentReply {
                text: paged.text,
                input_tokens: total_in,
                output_tokens: total_out,
                truncated: paged.truncated,
                continuation_pages: paged.pages,
            });
        }

        // Host-executed (fenced-protocol) tools invoked via NATIVE function
        // calling are bridged back to the caller as the fenced JSON block the
        // host executor understands. Observed live: gemma called
        // `indexed_file.write` natively, the transport answered "tool is not
        // available", and the agent gave up ("the required tool is not
        // available in my current environment") with the whole schema in hand.
        let (native, foreign): (Vec<_>, Vec<_>) = tool_calls
            .into_iter()
            .partition(|(name, _)| matches!(name.as_str(), "knowledge_search" | "egui_tree" | "egui_rects"));
        if !foreign.is_empty() {
            let mut out = text.trim_end().to_string();
            // Any native calls in the same round still execute; their results
            // ride along as text so no information is lost.
            for (name, args) in &native {
                let exec = match name.as_str() {
                    "knowledge_search" => call.tools.knowledge_search.clone(),
                    "egui_tree" => call.tools.egui_tree.clone(),
                    "egui_rects" => call.tools.egui_rects.clone(),
                    _ => None,
                };
                if let Some(f) = exec {
                    let outcome = match f(args.clone()) {
                        Ok(detail) => format!("TOOL RESULT {name} [ok]:\n{detail}"),
                        Err(error) => format!("TOOL RESULT {name} [error]: {error}"),
                    };
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(&outcome);
                }
            }
            let calls: Vec<serde_json::Value> = foreign
                .iter()
                .map(|(name, args)| serde_json::json!({ "tool": name, "args": args }))
                .collect();
            let fenced = serde_json::json!({ "tool_calls": calls });
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("```json\n{fenced}\n```"));
            return Ok(AgentReply {
                text: out,
                input_tokens: total_in,
                output_tokens: total_out,
                // The fenced block above was assembled here, not by the model;
                // appending continuation pages after it would corrupt it.
                truncated: false,
                continuation_pages: 0,
            });
        }
        let tool_calls = native;

        // Execute this round's tool calls through the host closures and feed
        // the results back as the next turn.
        let mut results = String::from("TOOL RESULTS:\n");
        for (name, args) in &tool_calls {
            let exec = match name.as_str() {
                "knowledge_search" => call.tools.knowledge_search.clone(),
                "egui_tree" => call.tools.egui_tree.clone(),
                "egui_rects" => call.tools.egui_rects.clone(),
                _ => None,
            };
            let outcome = match exec {
                Some(f) => f(args.clone()),
                None => Err(format!("tool \u{201c}{name}\u{201d} is not available")),
            };
            match outcome {
                Ok(detail) => {
                    results.push_str(&format!("- {name} [ok]:\n{detail}\n\n"));
                }
                Err(error) => {
                    results.push_str(&format!("- {name} [error]: {error}\n\n"));
                }
            }
        }
        results.push_str(
            "Use these real results. When the task is complete, reply with your final result.",
        );

        history.push(Message::user(prompt.clone()));
        history.push(Message::assistant(if text.trim().is_empty() {
            format!(
                "(called {})",
                tool_calls
                    .iter()
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            text
        }));
        prompt = results;
    }
    Err(format!(
        "tool budget exhausted: the model kept calling tools for more than {MAX_TOOL_ROUNDS} rounds"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anthropic reports the output cap as a top-level `stop_reason`. Missing
    /// this is what let a Pedantic review be cut off mid-JSON and still be
    /// treated as a finished verdict.
    #[test]
    fn anthropic_output_cap_is_detected() {
        let capped = serde_json::json!({
            "id": "msg_1",
            "stop_reason": "max_tokens",
            "content": [{"type": "text", "text": "…"}]
        });
        assert!(hit_output_cap(&capped));
    }

    /// A reply that ended on its own is NOT a continuation candidate — paging
    /// a finished answer would append restated text to a complete artifact.
    #[test]
    fn natural_stop_is_not_a_cap() {
        let done = serde_json::json!({"id": "msg_1", "stop_reason": "end_turn"});
        assert!(!hit_output_cap(&done));
        let done_openai = serde_json::json!({
            "choices": [{"finish_reason": "stop", "message": {"content": "done"}}]
        });
        assert!(!hit_output_cap(&done_openai));
    }

    /// The OpenAI wire spells the same condition `finish_reason: "length"`,
    /// per choice rather than per response.
    #[test]
    fn openai_output_cap_is_detected() {
        let capped = serde_json::json!({
            "choices": [{"finish_reason": "length", "message": {"content": "…"}}]
        });
        assert!(hit_output_cap(&capped));
    }

    /// A provider that reports no stop reason at all (bare-bones gateways)
    /// must read as "finished", never as an endless continuation loop.
    #[test]
    fn absent_stop_reason_is_not_a_cap() {
        let quiet = serde_json::json!({"id": "msg_1"});
        assert!(!hit_output_cap(&quiet));
    }

    /// Pages are concatenated with no separator: the resumed text continues
    /// the previous one mid-sentence, so nothing may be inserted between them.
    #[test]
    fn continued_reply_joins_pages_verbatim() {
        let mut reply = ContinuedReply::start("the verdict is accep".into(), true);
        reply.text.push_str("table");
        assert_eq!(reply.text, "the verdict is acceptable");
    }

    /// Every empty-reply error keeps the phrase the host classifies as
    /// retryable, and names the cause when the response reveals one: hidden
    /// reasoning, the output cap, or both.
    #[test]
    fn empty_reply_error_states_the_cause_and_stays_retryable() {
        let bare = empty_reply_error(false, &ReasoningTally::default());
        assert_eq!(bare, "the model returned no assistant text");

        let capped = empty_reply_error(true, &ReasoningTally::default());
        assert!(capped.contains("no assistant text"));
        assert!(capped.contains("max_tokens"));

        let thinking = empty_reply_error(
            false,
            &ReasoningTally {
                blocks: 2,
                chars: 900,
            },
        );
        assert!(thinking.contains("no assistant text"));
        assert!(thinking.contains("hidden reasoning"));
        assert!(thinking.contains("2 block(s)"));

        let both = empty_reply_error(
            true,
            &ReasoningTally {
                blocks: 1,
                chars: 12,
            },
        );
        assert!(both.contains("hidden reasoning"));
        assert!(both.contains("max_tokens"));
    }

    /// Reasoning blocks are counted, not dropped: an encrypted block renders to
    /// no text, so the block count is what proves the model produced something.
    #[test]
    fn reasoning_tally_counts_blocks_and_rendered_text() {
        let mut tally = ReasoningTally::default();
        tally.add(&Reasoning::new("twelve chars"));
        tally.add(&Reasoning::encrypted("opaque"));
        assert_eq!(tally.blocks, 2);
        assert_eq!(tally.chars, "twelve chars".chars().count());
    }

    /// Real OpenAI needs `max_completion_tokens`; every compatible gateway
    /// keeps the classic `max_tokens` (the legacy transport's switch).
    #[test]
    fn token_limit_parameter_is_provider_keyed() {
        assert!(uses_max_completion_tokens("openai"));
        assert!(uses_max_completion_tokens(" OpenAI "));
        assert!(!uses_max_completion_tokens("openrouter"));
        assert!(!uses_max_completion_tokens("ollama"));
        assert!(!uses_max_completion_tokens("ollama_cloud"));
        assert!(!uses_max_completion_tokens("anthropic"));
        assert!(!uses_max_completion_tokens("mistral"));
    }

    /// Anthropic removed the sampling parameters from Claude Opus 4.7 onward:
    /// sending `temperature` returns a 400 that fails the whole call. These are
    /// the models that produced the report.
    #[test]
    fn temperature_is_withheld_from_models_that_reject_it() {
        for model in [
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-mythos-5",
            "claude-opus-4-8",
            "claude-opus-4-7",
        ] {
            assert!(
                !sends_sampling_params("anthropic", model),
                "{model} rejects temperature"
            );
        }
    }

    /// The list is an allowlist, so a model this build has never heard of — any
    /// model released after it — is assumed to reject the parameter. Omitting
    /// costs a setting those models ignore; sending costs the whole request.
    #[test]
    fn an_unknown_anthropic_model_is_assumed_to_reject_temperature() {
        assert!(!sends_sampling_params("anthropic", "claude-opus-9"));
        assert!(!sends_sampling_params("anthropic", "claude-something-new"));
        assert!(!sends_sampling_params("anthropic", ""));
    }

    /// The generations that still accept it keep sending it, dated snapshots
    /// included.
    #[test]
    fn temperature_still_reaches_the_models_that_accept_it() {
        for model in [
            "claude-opus-4-6",
            "claude-sonnet-4-6",
            "claude-sonnet-4-5",
            "claude-haiku-4-5",
            "claude-opus-4-5-20251101",
            "claude-3-7-sonnet-20250219",
            "CLAUDE-OPUS-4-6",
            "  claude-opus-4-6  ",
            // Gateways that re-expose Anthropic prefix the provider onto the id.
            "anthropic.claude-opus-4-6",
        ] {
            assert!(
                sends_sampling_params("anthropic", model),
                "{model} accepts temperature"
            );
        }
    }

    /// Only Anthropic is gated — every other provider is untouched, including
    /// gateways whose model ids happen to look like Claude's.
    #[test]
    fn other_providers_keep_their_temperature() {
        assert!(sends_sampling_params("openai", "gpt-5"));
        assert!(sends_sampling_params("ollama", "qwen3-coder-plus"));
        assert!(sends_sampling_params("openrouter", "claude-opus-5"));
        assert!(sends_sampling_params("", "claude-opus-5"));
    }

    #[test]
    fn endpoint_normalisation_covers_legacy_shapes() {
        assert_eq!(
            normalize_base("openai", "https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base("ollama", "https://ollama.com/api/chat"),
            "https://ollama.com/v1"
        );
        assert_eq!(
            normalize_base("anthropic", "https://api.anthropic.com/v1/messages"),
            "https://api.anthropic.com/v1"
        );
        assert_eq!(normalize_base("openai", ""), "https://api.openai.com/v1");
        assert_eq!(normalize_base("ollama", ""), "http://localhost:11434/v1");
        // The shut-down HuggingFace host maps wholesale onto the router —
        // whatever path the stored endpoint carried (the old scheme died
        // with the host).
        assert_eq!(
            normalize_base("huggingface", "https://api-inference.huggingface.co/models"),
            "https://router.huggingface.co/v1"
        );
        assert_eq!(
            normalize_base(
                "huggingface",
                "https://api-inference.huggingface.co/models/meta-llama/Llama-3.3-70B-Instruct"
            ),
            "https://router.huggingface.co/v1"
        );
    }
}
