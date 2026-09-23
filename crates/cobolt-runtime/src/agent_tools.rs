// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tool calling for the `AgentObject` (spec 072) — the pure half.
//!
//! [`crate::agent_runtime`] sends one question and reads one answer. A model
//! that may use tools needs a conversation instead: the question, the model's
//! tool calls, their results, and so on until it answers in text. This module
//! keeps that conversation as protocol-neutral [`Turn`]s and renders it in each
//! protocol's own shape on every round ([`body_for_turns`]), and reads each
//! reply back into a neutral [`Reply`] plus the token [`Usage`] it reports
//! ([`parse_turn`]).
//!
//! An agent that offers NO tools never comes here: it keeps sending
//! [`crate::agent_runtime::body_for`]'s bytes exactly (072 R5).
//!
//! Two ways of offering tools:
//! - **native** — each protocol's own `tools` field and tool-call reply shape;
//! - **fenced** — for a model without native function calling: the tools are
//!   described in the system prompt, and a call is a fenced JSON block in the
//!   reply text (`ToolProtocol = Fenced`, never guessed from the model's name).

use crate::agent_runtime::{AskRequest, Protocol};
use serde_json::{json, Value};

/// One tool as offered to the model: a name, what it is for, and a JSON Schema
/// for its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// One call the model asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// The provider's id for the call, or one minted here when the protocol
    /// has none (Ollama, fenced).
    pub id: String,
    pub name: String,
    /// The arguments as the model sent them, verbatim.
    pub raw_arguments: String,
    /// Parsed; `None` when the model sent something that is not a JSON object.
    pub arguments: Option<Value>,
}

/// One step of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum Turn {
    User(String),
    Assistant(Reply),
    /// Results for the calls of the preceding assistant turn, in order.
    ToolResults(Vec<(ToolCall, String)>),
}

/// What the model answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Text(String),
    Calls {
        /// Any text that came with the calls.
        text: String,
        calls: Vec<ToolCall>,
        /// Anthropic's content array as received — sent back verbatim, which is
        /// what that protocol expects of the assistant turn.
        raw_content: Option<Value>,
    },
}

/// Token counts one response reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input += other.input;
        self.output += other.output;
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────────

/// The request body for this round.
pub fn body_for_turns(
    req: &AskRequest,
    protocol: Protocol,
    turns: &[Turn],
    tools: &[ToolSpec],
    fenced: bool,
) -> String {
    let temperature = (req.temperature.clamp(0, 100) as f64) / 100.0;
    let max_tokens = req.max_tokens.max(1);
    let mut system = req.system_prompt.trim().to_owned();
    if fenced && !tools.is_empty() {
        if !system.is_empty() {
            system.push_str("\n\n");
        }
        system.push_str(&fenced_system_block(tools));
    }
    let native = !fenced && !tools.is_empty();

    let value = match protocol {
        Protocol::Anthropic => {
            let mut v = json!({
                "model": req.model,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "messages": anthropic_messages(turns, fenced),
            });
            if !system.is_empty() {
                v["system"] = Value::String(system);
            }
            if native {
                v["tools"] = Value::Array(
                    tools
                        .iter()
                        .map(|t| {
                            json!({
                                "name": t.name,
                                "description": t.description,
                                "input_schema": t.parameters,
                            })
                        })
                        .collect(),
                );
            }
            v
        }
        Protocol::OllamaChat | Protocol::OpenAiChat => {
            let ollama = protocol == Protocol::OllamaChat;
            let mut messages = Vec::new();
            if !system.is_empty() {
                messages.push(json!({"role": "system", "content": system}));
            }
            messages.extend(chat_messages(turns, fenced, ollama));
            let mut v = if ollama {
                json!({
                    "model": req.model,
                    "messages": messages,
                    "stream": false,
                    "options": {"temperature": temperature, "num_predict": max_tokens},
                })
            } else {
                json!({
                    "model": req.model,
                    "messages": messages,
                    "stream": false,
                    "temperature": temperature,
                    "max_tokens": max_tokens,
                })
            };
            if native {
                v["tools"] = Value::Array(
                    tools
                        .iter()
                        .map(|t| {
                            json!({
                                "type": "function",
                                "function": {
                                    "name": t.name,
                                    "description": t.description,
                                    "parameters": t.parameters,
                                },
                            })
                        })
                        .collect(),
                );
            }
            v
        }
    };
    value.to_string()
}

/// OpenAI-compatible and Ollama-native messages (they differ only in how a
/// call's arguments are written and how a result names its call).
fn chat_messages(turns: &[Turn], fenced: bool, ollama: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for turn in turns {
        match turn {
            Turn::User(text) => out.push(json!({"role": "user", "content": text})),
            Turn::Assistant(Reply::Text(text)) => {
                out.push(json!({"role": "assistant", "content": text}))
            }
            Turn::Assistant(Reply::Calls { text, calls, .. }) => {
                if fenced {
                    out.push(json!({"role": "assistant", "content": text}));
                    continue;
                }
                let tool_calls: Vec<Value> = calls
                    .iter()
                    .map(|c| {
                        if ollama {
                            json!({"function": {
                                "name": c.name,
                                "arguments": c.arguments.clone().unwrap_or_else(|| json!({})),
                            }})
                        } else {
                            json!({
                                "id": c.id,
                                "type": "function",
                                "function": {"name": c.name, "arguments": c.raw_arguments},
                            })
                        }
                    })
                    .collect();
                out.push(json!({
                    "role": "assistant",
                    "content": text,
                    "tool_calls": tool_calls,
                }));
            }
            Turn::ToolResults(results) => {
                if fenced {
                    out.push(json!({"role": "user", "content": fenced_results(results)}));
                    continue;
                }
                for (call, result) in results {
                    if ollama {
                        out.push(json!({
                            "role": "tool",
                            "content": result,
                            "tool_name": call.name,
                        }));
                    } else {
                        out.push(json!({
                            "role": "tool",
                            "tool_call_id": call.id,
                            "content": result,
                        }));
                    }
                }
            }
        }
    }
    out
}

fn anthropic_messages(turns: &[Turn], fenced: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for turn in turns {
        match turn {
            Turn::User(text) => out.push(json!({"role": "user", "content": text})),
            Turn::Assistant(Reply::Text(text)) => {
                out.push(json!({"role": "assistant", "content": text}))
            }
            Turn::Assistant(Reply::Calls {
                text,
                calls,
                raw_content,
            }) => {
                if fenced {
                    out.push(json!({"role": "assistant", "content": text}));
                    continue;
                }
                let content = raw_content.clone().unwrap_or_else(|| {
                    let mut blocks = Vec::new();
                    if !text.is_empty() {
                        blocks.push(json!({"type": "text", "text": text}));
                    }
                    for c in calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": c.id,
                            "name": c.name,
                            "input": c.arguments.clone().unwrap_or_else(|| json!({})),
                        }));
                    }
                    Value::Array(blocks)
                });
                out.push(json!({"role": "assistant", "content": content}));
            }
            Turn::ToolResults(results) => {
                if fenced {
                    out.push(json!({"role": "user", "content": fenced_results(results)}));
                    continue;
                }
                let blocks: Vec<Value> = results
                    .iter()
                    .map(|(call, result)| {
                        json!({
                            "type": "tool_result",
                            "tool_use_id": call.id,
                            "content": result,
                        })
                    })
                    .collect();
                out.push(json!({"role": "user", "content": blocks}));
            }
        }
    }
    out
}

// ── The fenced protocol ────────────────────────────────────────────────────────

/// What a fenced-protocol model is told about the tools, appended to its
/// system prompt.
pub fn fenced_system_block(tools: &[ToolSpec]) -> String {
    let mut s = String::from(
        "You can use tools. To call one or more, reply with ONLY this fenced block \
         and nothing else:\n```json\n{\"tool_calls\": [{\"tool\": \"<name>\", \"args\": {…}}]}\n```\n\
         The results come back in the next message; then answer normally, without a block.\n\
         Tools:\n",
    );
    for t in tools {
        s.push_str(&format!(
            "- {}: {} Arguments (JSON Schema): {}\n",
            t.name, t.description, t.parameters
        ));
    }
    s
}

fn fenced_results(results: &[(ToolCall, String)]) -> String {
    let list: Vec<Value> = results
        .iter()
        .map(|(call, result)| json!({"tool": call.name, "result": result}))
        .collect();
    format!(
        "Tool results:\n```json\n{}\n```",
        serde_json::to_string_pretty(&list).unwrap_or_default()
    )
}

/// The calls in a fenced reply, or `None` when the text is an ordinary answer.
pub fn parse_fenced(text: &str) -> Option<Vec<ToolCall>> {
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let Some(end) = after[body_start..].find("```") else {
            break;
        };
        let block = after[body_start..body_start + end].trim();
        if let Ok(v) = serde_json::from_str::<Value>(block) {
            if let Some(list) = v.get("tool_calls").and_then(Value::as_array) {
                let calls: Vec<ToolCall> = list
                    .iter()
                    .enumerate()
                    .filter_map(|(n, c)| {
                        let name = c.get("tool").or_else(|| c.get("name"))?.as_str()?.to_owned();
                        let args = c.get("args").or_else(|| c.get("arguments")).cloned();
                        Some(ToolCall {
                            id: format!("call-{}", n + 1),
                            name,
                            raw_arguments: args.as_ref().map(|a| a.to_string()).unwrap_or_default(),
                            arguments: args.filter(Value::is_object),
                        })
                    })
                    .collect();
                if !calls.is_empty() {
                    return Some(calls);
                }
            }
        }
        rest = &after[body_start + end + 3..];
    }
    None
}

// ── Reading a reply ────────────────────────────────────────────────────────────

/// Read one round's reply: the answer or the calls, and the usage it reports.
/// Errors are the same ones [`crate::agent_runtime::parse_reply`] reports.
pub fn parse_turn(status: u16, body: &str, fenced: bool) -> Result<(Reply, Usage), String> {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let usage = usage_of(&json);

    if !fenced {
        if let Some(reply) = native_calls(&json) {
            if (200..300).contains(&status) && json.get("error").is_none() {
                return Ok((reply, usage));
            }
        }
    }
    let text = crate::agent_runtime::parse_reply(status, body)?;
    if fenced {
        if let Some(calls) = parse_fenced(&text) {
            return Ok((
                Reply::Calls {
                    text,
                    calls,
                    raw_content: None,
                },
                usage,
            ));
        }
    }
    Ok((Reply::Text(text), usage))
}

/// Native tool calls, in whichever of the three shapes the reply uses.
fn native_calls(json: &Value) -> Option<Reply> {
    // OpenAI-compatible.
    if let Some(list) = json.pointer("/choices/0/message/tool_calls").and_then(Value::as_array) {
        let calls: Vec<ToolCall> = list
            .iter()
            .enumerate()
            .filter_map(|(n, c)| {
                let name = c.pointer("/function/name")?.as_str()?.to_owned();
                let raw = match c.pointer("/function/arguments") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => String::new(),
                };
                Some(ToolCall {
                    id: c
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("call-{}", n + 1)),
                    name,
                    arguments: parse_object(&raw),
                    raw_arguments: raw,
                })
            })
            .collect();
        if !calls.is_empty() {
            let text = json
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Some(Reply::Calls {
                text,
                calls,
                raw_content: None,
            });
        }
    }
    // Ollama native — arguments arrive as an object, and calls carry no id.
    if let Some(list) = json.pointer("/message/tool_calls").and_then(Value::as_array) {
        let calls: Vec<ToolCall> = list
            .iter()
            .enumerate()
            .filter_map(|(n, c)| {
                let name = c.pointer("/function/name")?.as_str()?.to_owned();
                let args = c.pointer("/function/arguments").cloned().unwrap_or(Value::Null);
                let (raw, arguments) = match args {
                    Value::String(s) => (s.clone(), parse_object(&s)),
                    Value::Object(_) => (args.to_string(), Some(args)),
                    other => (other.to_string(), None),
                };
                Some(ToolCall {
                    id: format!("call-{}", n + 1),
                    name,
                    raw_arguments: raw,
                    arguments,
                })
            })
            .collect();
        if !calls.is_empty() {
            let text = json
                .pointer("/message/content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Some(Reply::Calls {
                text,
                calls,
                raw_content: None,
            });
        }
    }
    // Anthropic — `tool_use` blocks among the content.
    if let Some(blocks) = json.get("content").and_then(Value::as_array) {
        let calls: Vec<ToolCall> = blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
            .filter_map(|b| {
                let input = b.get("input").cloned().unwrap_or(Value::Null);
                Some(ToolCall {
                    id: b.get("id")?.as_str()?.to_owned(),
                    name: b.get("name")?.as_str()?.to_owned(),
                    raw_arguments: input.to_string(),
                    arguments: Some(input).filter(Value::is_object),
                })
            })
            .collect();
        if !calls.is_empty() {
            let text = blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
            return Some(Reply::Calls {
                text,
                calls,
                raw_content: Some(Value::Array(blocks.clone())),
            });
        }
    }
    None
}

fn parse_object(raw: &str) -> Option<Value> {
    if raw.trim().is_empty() {
        return Some(json!({}));
    }
    serde_json::from_str::<Value>(raw).ok().filter(Value::is_object)
}

/// The token counts a raw reply body reports (zero when it reports none).
pub fn usage_from_body(body: &str) -> Usage {
    usage_of(&serde_json::from_str(body).unwrap_or(Value::Null))
}

/// The token counts, under whichever names the provider uses.
fn usage_of(json: &Value) -> Usage {
    let first = |paths: &[&str]| {
        paths
            .iter()
            .find_map(|p| json.pointer(p).and_then(Value::as_u64))
            .unwrap_or(0)
    };
    Usage {
        input: first(&["/usage/prompt_tokens", "/usage/input_tokens", "/prompt_eval_count"]),
        output: first(&["/usage/completion_tokens", "/usage/output_tokens", "/eval_count"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> AskRequest {
        AskRequest {
            model: "m".into(),
            system_prompt: "Be brief.".into(),
            prompt: "How many actors?".into(),
            temperature: 50,
            max_tokens: 100,
            ..Default::default()
        }
    }

    fn search_tool() -> ToolSpec {
        ToolSpec {
            name: "search_actors_file".into(),
            description: "One row per actor.".into(),
            parameters: json!({"type": "object", "properties": {"NAME": {"type": "string"}}}),
        }
    }

    fn call() -> ToolCall {
        ToolCall {
            id: "call_1".into(),
            name: "search_actors_file".into(),
            raw_arguments: "{\"NAME\":\"Kim\"}".into(),
            arguments: Some(json!({"NAME": "Kim"})),
        }
    }

    fn conversation() -> Vec<Turn> {
        vec![
            Turn::User("How many actors?".into()),
            Turn::Assistant(Reply::Calls {
                text: String::new(),
                calls: vec![call()],
                raw_content: None,
            }),
            Turn::ToolResults(vec![(call(), "2 records".into())]),
        ]
    }

    /// Each protocol's native request carries the tools and the full exchange
    /// in that protocol's own shape.
    #[test]
    fn each_protocol_renders_tools_and_the_exchange_in_its_own_shape() {
        let tools = [search_tool()];
        let turns = conversation();

        let openai: Value =
            serde_json::from_str(&body_for_turns(&req(), Protocol::OpenAiChat, &turns, &tools, false)).unwrap();
        assert_eq!(openai["tools"][0]["type"], "function");
        assert_eq!(openai["tools"][0]["function"]["name"], "search_actors_file");
        assert_eq!(openai["messages"][0]["role"], "system");
        assert_eq!(openai["messages"][2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(openai["messages"][2]["tool_calls"][0]["function"]["arguments"], "{\"NAME\":\"Kim\"}");
        assert_eq!(openai["messages"][3]["role"], "tool");
        assert_eq!(openai["messages"][3]["tool_call_id"], "call_1");

        let ollama: Value =
            serde_json::from_str(&body_for_turns(&req(), Protocol::OllamaChat, &turns, &tools, false)).unwrap();
        assert_eq!(ollama["tools"][0]["function"]["name"], "search_actors_file");
        assert_eq!(ollama["messages"][2]["tool_calls"][0]["function"]["arguments"]["NAME"], "Kim");
        assert_eq!(ollama["messages"][3]["tool_name"], "search_actors_file");
        assert_eq!(ollama["options"]["num_predict"], 100);

        let anthropic: Value =
            serde_json::from_str(&body_for_turns(&req(), Protocol::Anthropic, &turns, &tools, false)).unwrap();
        assert_eq!(anthropic["system"], "Be brief.");
        assert_eq!(anthropic["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(anthropic["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(anthropic["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(anthropic["messages"][2]["content"][0]["tool_use_id"], "call_1");
    }

    /// With the fenced protocol there is no `tools` field; the tools ride in
    /// the system prompt and results come back as a user turn.
    #[test]
    fn the_fenced_protocol_describes_tools_in_the_prompt() {
        let body: Value = serde_json::from_str(&body_for_turns(
            &req(),
            Protocol::OllamaChat,
            &conversation(),
            &[search_tool()],
            true,
        ))
        .unwrap();
        assert!(body.get("tools").is_none());
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.starts_with("Be brief.") && system.contains("search_actors_file"));
        assert!(body["messages"][3]["content"].as_str().unwrap().starts_with("Tool results:"));

        let reply = "Checking.\n```json\n{\"tool_calls\":[{\"tool\":\"search_actors_file\",\"args\":{\"NAME\":\"Kim\"}}]}\n```";
        let calls = parse_fenced(reply).unwrap();
        assert_eq!(calls[0].name, "search_actors_file");
        assert_eq!(calls[0].arguments, Some(json!({"NAME": "Kim"})));
        assert!(parse_fenced("Just an answer with ```code``` in it.").is_none());
    }

    /// Calls and usage are read from each provider's reply shape.
    #[test]
    fn replies_are_read_in_every_shape() {
        let openai = r#"{"choices":[{"message":{"content":null,"tool_calls":[{"id":"c9","type":"function","function":{"name":"t","arguments":"{\"A\":\"1\"}"}}]}}],"usage":{"prompt_tokens":11,"completion_tokens":3}}"#;
        let (reply, usage) = parse_turn(200, openai, false).unwrap();
        assert!(matches!(&reply, Reply::Calls { calls, .. } if calls[0].id == "c9" && calls[0].arguments == Some(json!({"A":"1"}))));
        assert_eq!(usage, Usage { input: 11, output: 3 });

        let ollama = r#"{"message":{"content":"","tool_calls":[{"function":{"name":"t","arguments":{"A":"1"}}}]},"prompt_eval_count":20,"eval_count":5}"#;
        let (reply, usage) = parse_turn(200, ollama, false).unwrap();
        assert!(matches!(&reply, Reply::Calls { calls, .. } if calls[0].id == "call-1"));
        assert_eq!(usage, Usage { input: 20, output: 5 });

        let anthropic = r#"{"content":[{"type":"text","text":"Let me look."},{"type":"tool_use","id":"tu_1","name":"t","input":{"A":"1"}}],"usage":{"input_tokens":30,"output_tokens":7}}"#;
        let (reply, usage) = parse_turn(200, anthropic, false).unwrap();
        match reply {
            Reply::Calls { text, calls, raw_content } => {
                assert_eq!(text, "Let me look.");
                assert_eq!(calls[0].id, "tu_1");
                assert!(raw_content.is_some(), "sent back verbatim");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(usage, Usage { input: 30, output: 7 });

        let text = r#"{"choices":[{"message":{"content":"Two actors."}}],"usage":{"prompt_tokens":40,"completion_tokens":4}}"#;
        let (reply, usage) = parse_turn(200, text, false).unwrap();
        assert_eq!(reply, Reply::Text("Two actors.".into()));
        assert_eq!(usage.input, 40);

        // Malformed arguments are kept raw and flagged, not dropped.
        let bad = r#"{"choices":[{"message":{"tool_calls":[{"id":"x","function":{"name":"t","arguments":"{not json"}}]}}]}"#;
        let (reply, _) = parse_turn(200, bad, false).unwrap();
        assert!(matches!(&reply, Reply::Calls { calls, .. } if calls[0].arguments.is_none() && calls[0].raw_arguments == "{not json"));

        // A provider error is still an error.
        assert!(parse_turn(401, r#"{"error":{"message":"bad key"}}"#, false).is_err());
    }
}
