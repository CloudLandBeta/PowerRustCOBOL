// SPDX-License-Identifier: Apache-2.0

//! Rig-based transport for project agents (Grace, specialists, Pedantic
//! reviewers) — replaces the hand-rolled HTTP orchestrator for the agentic
//! workflow path (Rig migration, phase 1).
//!
//! Design decisions (see the Rig alignment report):
//! - Every OpenAI-compatible provider (OpenAI, Ollama's `/v1` endpoint,
//!   Mistral, Groq, …) goes through `rig::providers::openai::Client` with a
//!   custom base URL. Anthropic goes through its native client. No wire-format
//!   sniffing.
//! - No hooks in v1: token usage is read from the completion response's
//!   `usage`, and tool governance is by construction (an agent only receives
//!   the tools it declares), so nothing here depends on post-0.40 APIs.
//! - The blocking wrapper mirrors the previous per-request-runtime pattern so
//!   the synchronous `AgentInvoker` seam in `grace.rs` is preserved.

use rig_core::client::CompletionClient;
use rig_core::completion::Completion;
use rig_core::message::{AssistantContent, Message};
use rig_core::providers::{anthropic, openai};
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
}

/// Normalise a configured endpoint to the base URL a Rig provider client
/// expects. Legacy configs store full request URLs; Rig wants the API root.
fn normalize_base(provider: &str, endpoint: &str) -> String {
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
    if base.is_empty() {
        base = match provider {
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
    }
    base
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
            .build()
            .map_err(|e| format!("anthropic client build failed: {e}"))?;
        complete_with(client, call).await
    } else {
        // Every other configured provider speaks the OpenAI wire at its base
        // URL (Ollama via /v1); one client covers them all.
        let client = openai::Client::builder()
            .api_key(call.api_key.as_str())
            .base_url(&base)
            .build()
            .map_err(|e| format!("openai-compatible client build failed: {e}"))?;
        complete_with(client, call).await
    }
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

/// Shared completion body: build the Rig agent from the profile, then run a
/// bounded tool loop — the model's native tool calls are executed through the
/// host closures and their results fed back, all inside this one call.
async fn complete_with<C>(client: C, call: &AgentCall) -> Result<AgentReply, String>
where
    C: CompletionClient,
{
    let mut builder = client
        .agent(&call.model)
        .preamble(&call.system_prompt)
        .temperature(call.temperature as f64)
        .max_tokens(call.max_tokens as u64);
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

        let mut text = String::new();
        let mut tool_calls: Vec<(String, serde_json::Value)> = Vec::new();
        for content in response.choice.into_iter() {
            match content {
                AssistantContent::Text(t) => text.push_str(&t.text),
                AssistantContent::ToolCall(tc) => {
                    tool_calls.push((tc.function.name.clone(), tc.function.arguments.clone()));
                }
                _ => {}
            }
        }

        if tool_calls.is_empty() {
            if text.trim().is_empty() {
                return Err("the model returned no assistant text".to_string());
            }
            return Ok(AgentReply {
                text,
                input_tokens: total_in,
                output_tokens: total_out,
            });
        }

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
    }
}
