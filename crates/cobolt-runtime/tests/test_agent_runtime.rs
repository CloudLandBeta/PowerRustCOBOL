// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the `AgentObject` sends and how it reads the answer.
//!
//! Every case here is pure — no socket is opened. The transport is the
//! interpreter's existing HTTP bridge and is exercised by its own tests; what
//! is new, and what silently did not exist before 1.65.57, is the request.

use cobolt_runtime::agent_runtime::{
    body_for, endpoint_for, headers_for, parse_reply, protocol_for, AskRequest, Protocol,
};

fn req() -> AskRequest {
    AskRequest {
        api: "Ollama".into(),
        url: "https://ollama.com/api/chat".into(),
        endpoint: String::new(),
        model: "gemma4:31b".into(),
        api_key: "sk-test".into(),
        system_prompt: "You are a COBOL assistant.".into(),
        prompt: "What does STORAGE MODE IS DISK change?".into(),
        temperature: 30,
        max_tokens: 400,
    }
}

/// `AgentAPI` decides the protocol when it names a provider; the URL breaks the
/// tie otherwise. Ollama serves both shapes, so its path is what settles it —
/// the operator's own control points at `/api/chat`, the native one.
#[test]
fn the_protocol_follows_the_provider_then_the_path() {
    assert_eq!(
        protocol_for("Ollama", "https://ollama.com/api/chat"),
        Protocol::OllamaChat
    );
    assert_eq!(
        protocol_for("Ollama", "https://ollama.com/v1/chat/completions"),
        Protocol::OpenAiChat,
        "Ollama's OpenAI-compatible endpoint speaks the OpenAI shape"
    );
    assert_eq!(protocol_for("OpenAI", "https://api.openai.com"), Protocol::OpenAiChat);
    assert_eq!(protocol_for("LMStudio", "http://localhost:1234"), Protocol::OpenAiChat);
    assert_eq!(protocol_for("Anthropic", "https://api.anthropic.com"), Protocol::Anthropic);
    // Custom / unset: the path is the only honest clue.
    assert_eq!(protocol_for("Custom", "https://x.test/api/chat"), Protocol::OllamaChat);
    assert_eq!(protocol_for("", "https://x.test/v1/messages"), Protocol::Anthropic);
    assert_eq!(protocol_for("", "https://x.test/"), Protocol::OpenAiChat);
}

/// A URL the developer spelled out is never rewritten; a bare origin gets the
/// protocol's own path; `AgentEndpoint` overrides both.
#[test]
fn the_endpoint_respects_what_the_developer_wrote() {
    let mut r = req();
    assert_eq!(
        endpoint_for(&r, Protocol::OllamaChat),
        "https://ollama.com/api/chat",
        "a path already chosen is left alone"
    );

    r.url = "http://localhost:11434".into();
    assert_eq!(
        endpoint_for(&r, Protocol::OllamaChat),
        "http://localhost:11434/api/chat",
        "a bare origin gets the protocol's default path"
    );
    assert_eq!(
        endpoint_for(&r, Protocol::OpenAiChat),
        "http://localhost:11434/v1/chat/completions"
    );

    r.endpoint = "https://proxy.internal/llm".into();
    assert_eq!(
        endpoint_for(&r, Protocol::OllamaChat),
        "https://proxy.internal/llm",
        "an explicit override wins outright"
    );

    let empty = AskRequest::default();
    assert!(endpoint_for(&empty, Protocol::OpenAiChat).is_empty());
}

/// The key travels the way each provider expects — and only when there is one.
/// A local Ollama wants no Authorization header at all, and an empty bearer is
/// worse than none.
#[test]
fn the_key_is_sent_only_when_set_and_in_the_providers_own_header() {
    let r = req();
    let h = headers_for(&r, Protocol::OpenAiChat);
    assert!(h.contains(&("Authorization".into(), "Bearer sk-test".into())));

    let h = headers_for(&r, Protocol::Anthropic);
    assert!(h.contains(&("x-api-key".into(), "sk-test".into())));
    assert!(h.iter().any(|(k, _)| k == "anthropic-version"));
    assert!(
        !h.iter().any(|(k, _)| k == "Authorization"),
        "Anthropic does not take a bearer"
    );

    let mut bare = req();
    bare.api_key = "   ".into();
    let h = headers_for(&bare, Protocol::OpenAiChat);
    assert_eq!(h.len(), 1, "content-type only: {h:?}");
    assert!(!h.iter().any(|(k, _)| k == "Authorization"));
}

/// The body carries the model, the system prompt, the question, and the
/// designer's 0–100 temperature as the 0.0–1.0 the wire expects.
#[test]
fn the_body_carries_the_controls_settings_in_the_protocols_shape() {
    let r = req();

    let v: serde_json::Value =
        serde_json::from_str(&body_for(&r, Protocol::OllamaChat)).expect("json");
    assert_eq!(v["model"], "gemma4:31b");
    assert_eq!(v["stream"], false);
    assert_eq!(v["options"]["num_predict"], 400);
    assert_eq!(v["options"]["temperature"], 0.3, "30 of 100 is 0.3");
    assert_eq!(v["messages"][0]["role"], "system");
    assert_eq!(v["messages"][1]["content"], r.prompt);

    let v: serde_json::Value =
        serde_json::from_str(&body_for(&r, Protocol::OpenAiChat)).expect("json");
    assert_eq!(v["max_tokens"], 400);
    assert_eq!(v["temperature"], 0.3);
    assert_eq!(v["messages"][1]["content"], r.prompt);

    let v: serde_json::Value =
        serde_json::from_str(&body_for(&r, Protocol::Anthropic)).expect("json");
    assert_eq!(v["system"], "You are a COBOL assistant.");
    assert_eq!(v["max_tokens"], 400);
    assert_eq!(v["messages"][0]["content"], r.prompt);
    assert!(v["messages"][0]["role"] == "user");
}

/// No system prompt means no system message — an empty one changes some
/// providers' behaviour rather than being ignored.
#[test]
fn an_empty_system_prompt_is_omitted_entirely() {
    let mut r = req();
    r.system_prompt = "   ".into();
    let v: serde_json::Value =
        serde_json::from_str(&body_for(&r, Protocol::OpenAiChat)).expect("json");
    assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    assert_eq!(v["messages"][0]["role"], "user");

    let v: serde_json::Value =
        serde_json::from_str(&body_for(&r, Protocol::Anthropic)).expect("json");
    assert!(v.get("system").is_none(), "no empty system field: {v}");
}

/// Every known reply shape is read, whatever protocol was chosen — a `Custom`
/// endpoint may answer in any of them.
#[test]
fn a_reply_is_read_from_any_of_the_known_shapes() {
    let openai = r#"{"choices":[{"message":{"role":"assistant","content":"DISK stores on disk."}}]}"#;
    assert_eq!(parse_reply(200, openai).unwrap(), "DISK stores on disk.");

    let ollama = r#"{"message":{"role":"assistant","content":"On disk."},"done":true}"#;
    assert_eq!(parse_reply(200, ollama).unwrap(), "On disk.");

    let anthropic = r#"{"content":[{"type":"text","text":"On disk."}]}"#;
    assert_eq!(parse_reply(200, anthropic).unwrap(), "On disk.");

    let generate = r#"{"response":"On disk.","done":true}"#;
    assert_eq!(parse_reply(200, generate).unwrap(), "On disk.");
}

/// A failure has to arrive as a failure. Each of these used to be
/// indistinguishable from a successful empty answer, because no call was made
/// and `LastReply` was always empty.
#[test]
fn every_failure_shape_becomes_an_error_with_its_evidence() {
    let e = parse_reply(401, r#"{"error":{"message":"invalid api key"}}"#).unwrap_err();
    assert!(e.contains("401") && e.contains("invalid api key"), "{e}");

    // Some providers report an error on a 200.
    let e = parse_reply(200, r#"{"error":"model not found"}"#).unwrap_err();
    assert!(e.contains("model not found"), "{e}");

    let e = parse_reply(500, "<html>gateway</html>").unwrap_err();
    assert!(e.contains("500") && e.contains("not JSON"), "{e}");

    let e = parse_reply(200, r#"{"done":true}"#).unwrap_err();
    assert!(e.contains("no assistant message"), "{e}");
}

/// An error message is evidence pasted into bug reports: bounded, one line.
#[test]
fn an_error_body_is_clipped_to_one_bounded_line() {
    let huge = format!(r#"{{"junk":"{}"}}"#, "x".repeat(5000));
    let e = parse_reply(500, &huge).unwrap_err();
    assert!(e.chars().count() < 320, "{} chars", e.chars().count());
    assert!(!e.contains('\n'));
}

/// An empty reply says so. It used to be reported as "the reply was not JSON",
/// which is true of the empty string and useless as a diagnosis — the body is
/// empty because the transfer produced nothing, not because the provider
/// speaks a format we cannot read.
#[test]
fn an_empty_reply_is_reported_as_empty_not_as_bad_json() {
    for body in ["", "   ", "\n\n"] {
        let err = cobolt_runtime::agent_runtime::parse_reply(200, body)
            .expect_err("an empty body is not a reply");
        assert!(
            err.contains("the reply was empty"),
            "empty body reported as {err:?}"
        );
        assert!(
            !err.contains("not JSON"),
            "empty body still blamed on JSON: {err:?}"
        );
    }
    // A body that IS present but malformed still points at the format.
    let err = cobolt_runtime::agent_runtime::parse_reply(200, "{not json")
        .expect_err("malformed JSON is not a reply");
    assert!(err.contains("not JSON"), "malformed body reported as {err:?}");
}
