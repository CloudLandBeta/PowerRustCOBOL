// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The server loop and the seam a host implements.
//!
//! [`serve`] owns the protocol — envelopes, the handshake, dispatch and errors —
//! and knows nothing about what any tool does. [`McpHandler`] owns the tools and
//! knows nothing about JSON-RPC. That line is what lets the indexed-file tool
//! live in `cobolt-runtime`, where COBOL knowledge belongs, while this crate
//! stays linkable by anything.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::transport::{read_message, write_message};
use crate::types::{
    error_code, negotiate, InitializeResult, Request, Response, ServerInfo, Tool, ToolResult,
};

/// What a host must provide to be served.
///
/// Deliberately small. Everything the protocol can do on its own — framing,
/// the handshake, method dispatch, error envelopes — is [`serve`]'s job, so an
/// implementor writes only what is genuinely theirs.
pub trait McpHandler {
    /// Who is answering.
    fn server_info(&self) -> ServerInfo;

    /// The tools currently on offer.
    ///
    /// Called per `tools/list` rather than cached, so a host whose tool set
    /// depends on live state — which files are marked consultable, say — does
    /// not have to invalidate anything.
    fn list_tools(&mut self) -> Vec<Tool>;

    /// Run one tool.
    ///
    /// A tool that ran and could not do the job returns
    /// [`ToolResult::failed`] — that is a *tool* failure, reported to the model
    /// as content. It is not a JSON-RPC error, which means the call itself was
    /// malformed. Keeping them apart is what lets a caller tell "no such tool"
    /// from "that search matched nothing it could use".
    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult;

    /// What this server advertises at `initialize`.
    ///
    /// Defaulted, because a tools-only server is the common case and should not
    /// have to say so.
    fn capabilities(&self) -> Value {
        json!({ "tools": { "listChanged": false } })
    }
}

/// Serve MCP over one byte stream pair until the peer closes it.
///
/// Returns `Ok(())` on a clean end of stream. An I/O failure is returned; a
/// *protocol* failure never is — a malformed request, an unknown method or a
/// failing tool each produce an error **response** and the loop continues.
/// Closing the connection because one request was wrong would take down a
/// session over a typo.
pub fn serve<R: BufRead, W: Write, H: McpHandler>(
    input: &mut R,
    output: &mut W,
    handler: &mut H,
) -> io::Result<()> {
    while let Some(raw) = read_message(input)? {
        let Some(response) = handle_one(&raw, handler) else {
            continue; // a notification — no reply is the correct reply
        };
        let body = serde_json::to_vec(&response).map_err(io::Error::other)?;
        write_message(output, &body)?;
    }
    Ok(())
}

/// Dispatch one raw message. `None` means "no reply is owed".
fn handle_one<H: McpHandler>(raw: &[u8], handler: &mut H) -> Option<Response> {
    let request: Request = match serde_json::from_slice(raw) {
        Ok(r) => r,
        Err(e) => {
            // No id is recoverable from an unparseable body, and JSON-RPC says
            // to answer with a null id rather than stay silent.
            return Some(Response::err(
                Value::Null,
                error_code::PARSE_ERROR,
                format!("could not parse request: {e}"),
            ));
        }
    };

    let is_notification = request.is_notification();
    let id = request.id.clone().unwrap_or(Value::Null);
    let params = request.params.clone().unwrap_or(Value::Null);

    let reply = match request.method.as_str() {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let result = InitializeResult {
                protocol_version: negotiate(requested.as_deref()).to_string(),
                capabilities: handler.capabilities(),
                server_info: handler.server_info(),
            };
            match serde_json::to_value(result) {
                Ok(v) => Response::ok(id, v),
                Err(e) => Response::err(id, error_code::INTERNAL_ERROR, e.to_string()),
            }
        }

        // The client telling us it is ready. Acknowledged by saying nothing.
        "notifications/initialized" | "initialized" => return None,

        // A peer may ping to check the link is alive.
        "ping" => Response::ok(id, json!({})),

        "tools/list" => {
            let tools: Vec<Tool> = handler.list_tools();
            match serde_json::to_value(tools) {
                Ok(v) => Response::ok(id, json!({ "tools": v })),
                Err(e) => Response::err(id, error_code::INTERNAL_ERROR, e.to_string()),
            }
        }

        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return finish(
                    is_notification,
                    Response::err(
                        id,
                        error_code::INVALID_PARAMS,
                        "tools/call requires a 'name'",
                    ),
                );
            };
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            let result: ToolResult = handler.call_tool(name, &arguments);
            match serde_json::to_value(result) {
                Ok(v) => Response::ok(id, v),
                Err(e) => Response::err(id, error_code::INTERNAL_ERROR, e.to_string()),
            }
        }

        other => Response::err(
            id,
            error_code::METHOD_NOT_FOUND,
            format!("no such method: {other}"),
        ),
    };

    finish(is_notification, reply)
}

/// A notification is never answered, not even to report that it was wrong.
fn finish(is_notification: bool, reply: Response) -> Option<Response> {
    if is_notification {
        None
    } else {
        Some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Content;
    use pretty_assertions::assert_eq;

    /// A handler that records what it was asked, so tests can check dispatch.
    #[derive(Default)]
    struct Spy {
        calls: Vec<(String, Value)>,
    }

    impl McpHandler for Spy {
        fn server_info(&self) -> ServerInfo {
            ServerInfo {
                name: "spy".into(),
                version: "1".into(),
            }
        }
        fn list_tools(&mut self) -> Vec<Tool> {
            vec![Tool {
                name: "search_actors".into(),
                description: Some("Actors, one row per performer".into()),
                input_schema: json!({"type":"object","properties":{}}),
            }]
        }
        fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
            self.calls.push((name.to_string(), arguments.clone()));
            ToolResult::ok(vec![Content::text("two records")])
        }
    }

    /// Drive `serve` with a scripted client and collect its replies.
    fn exchange(requests: &[Value]) -> Vec<Response> {
        let mut input = Vec::new();
        for r in requests {
            input.extend_from_slice(serde_json::to_string(r).unwrap().as_bytes());
            input.push(b'\n');
        }
        let mut reader = io::Cursor::new(input);
        let mut out = Vec::new();
        let mut spy = Spy::default();
        serve(&mut reader, &mut out, &mut spy).expect("clean stream");
        String::from_utf8(out)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("a reply is a Response"))
            .collect()
    }

    #[test]
    fn a_client_completes_the_handshake_and_lists_tools() {
        let replies = exchange(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize",
                   "params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"c","version":"1"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ]);

        assert_eq!(replies.len(), 2, "the notification must not be answered");

        let init = replies[0].result.as_ref().expect("initialize succeeded");
        assert_eq!(
            init["protocolVersion"], "2024-11-05",
            "the client's revision is echoed when we speak it"
        );
        assert_eq!(init["serverInfo"]["name"], "spy");

        let tools = replies[1].result.as_ref().unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "search_actors");
        assert!(
            tools[0]["inputSchema"].is_object(),
            "a tool must carry a schema"
        );
    }

    /// An unknown method is an error *response*, and the session survives it.
    #[test]
    fn an_unknown_method_errors_and_the_loop_continues() {
        let replies = exchange(&[
            json!({"jsonrpc":"2.0","id":1,"method":"no/such/thing"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ]);
        assert_eq!(replies.len(), 2, "the stream stayed open");
        assert_eq!(
            replies[0].error.as_ref().unwrap().code,
            error_code::METHOD_NOT_FOUND
        );
        assert!(
            replies[1].result.is_some(),
            "the request after the bad one still worked"
        );
    }

    #[test]
    fn an_unparseable_message_is_answered_and_does_not_end_the_session() {
        let mut input = Vec::from(&b"{ this is not json\n"[..]);
        input.extend_from_slice(
            serde_json::to_string(&json!({"jsonrpc":"2.0","id":9,"method":"ping"}))
                .unwrap()
                .as_bytes(),
        );
        input.push(b'\n');

        let mut reader = io::Cursor::new(input);
        let mut out = Vec::new();
        let mut spy = Spy::default();
        serve(&mut reader, &mut out, &mut spy).expect("a bad message is not an I/O failure");

        let replies: Vec<Response> = String::from_utf8(out)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(
            replies[0].error.as_ref().unwrap().code,
            error_code::PARSE_ERROR
        );
        assert_eq!(replies[0].id, Value::Null, "no id is recoverable");
        assert!(replies[1].result.is_some(), "the session continued");
    }

    #[test]
    fn a_tool_call_reaches_the_handler_with_its_arguments() {
        let mut reader = io::Cursor::new(
            serde_json::to_string(&json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"search_actors","arguments":{"query":"salary over 100"}}
            }))
            .unwrap()
                + "\n",
        );
        let mut out = Vec::new();
        let mut spy = Spy::default();
        serve(&mut reader, &mut out, &mut spy).unwrap();

        assert_eq!(spy.calls.len(), 1);
        assert_eq!(spy.calls[0].0, "search_actors");
        assert_eq!(spy.calls[0].1["query"], "salary over 100");
    }

    #[test]
    fn tools_call_without_a_name_is_an_invalid_params_error() {
        let replies = exchange(&[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"arguments":{}}}),
        ]);
        assert_eq!(
            replies[0].error.as_ref().unwrap().code,
            error_code::INVALID_PARAMS
        );
    }

    /// Even a *wrong* notification gets no reply — answering one is a protocol
    /// violation, and a peer that receives an unexpected response may abort.
    #[test]
    fn a_notification_is_never_answered_even_when_its_method_is_unknown() {
        let replies = exchange(&[json!({"jsonrpc":"2.0","method":"no/such/notification"})]);
        assert!(replies.is_empty(), "got {replies:?}");
    }
}
