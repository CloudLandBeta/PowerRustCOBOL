// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Finding the sentence in a provider error that actually says what happened.
//!
//! The error modal shows the whole connection log — every request line, every
//! header, every retry — because when something is wrong you want all of it.
//! But the reason is one sentence buried in there, usually inside a JSON body
//! on a single very long line, and reading it meant hunting: *"this model
//! requires a subscription"*, *"temperature is not supported with this model"*
//! (operator, 2026-08-20).
//!
//! So the reason is lifted to the top of the modal and the log is left exactly
//! as it was underneath. Nothing is hidden, rewritten or summarised away — the
//! headline is the provider's **own words**, quoted verbatim from its payload.
//!
//! This is deliberately **structural, not interpretive**. It reads the fields
//! providers already fill in (`error.message`, `error.code`, `error.param`) and
//! never tries to work out from prose what kind of failure it was. A guess
//! about intent would be wrong exactly when the error is unusual, which is when
//! this matters most.

/// The part of a provider error worth reading first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorSummary {
    /// The provider's own explanation, verbatim.
    pub headline: String,
    /// Machine code / type when the payload carried one (`insufficient_quota`,
    /// `unsupported_parameter`, `permission_error`).
    pub code: Option<String>,
    /// The offending request field, when the provider named one — the whole
    /// answer to "which parameter is not supported?".
    pub param: Option<String>,
}

impl ErrorSummary {
    /// One line: the message, with the code and parameter appended when they
    /// add something the message does not already say.
    pub fn line(&self) -> String {
        let mut out = self.headline.clone();
        let mut extras: Vec<String> = Vec::new();
        if let Some(param) = &self.param {
            if !self.headline.contains(param.as_str()) {
                extras.push(format!("parameter: {param}"));
            }
        }
        if let Some(code) = &self.code {
            if !self.headline.contains(code.as_str()) {
                extras.push(code.clone());
            }
        }
        if !extras.is_empty() {
            out.push_str(&format!("  ({})", extras.join(", ")));
        }
        out
    }
}

/// Every `{...}` span in `text` that parses as JSON, in the order they appear.
///
/// A connection log is not JSON — it is prose with JSON *in* it, often several
/// bodies from several attempts — so the payloads have to be found rather than
/// parsed from the top. Brace counting is enough here and cannot run away: it
/// only ever scans forward, and a span that does not parse is simply skipped.
fn json_spans(text: &str) -> Vec<serde_json::Value> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut in_str = false;
        let mut escaped = false;
        let mut end = None;
        for (off, &b) in bytes[i..].iter().enumerate() {
            if in_str {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_str = false;
                }
                continue;
            }
            match b {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i + off + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text[i..e]) {
                    out.push(v);
                }
                i = e;
            }
            // Unbalanced from here on (a truncated log): nothing further can
            // close, so stop rather than rescanning the tail forever.
            None => break,
        }
    }
    out
}

/// A string field at `key`, non-empty after trimming.
fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Pull `(message, code, param)` out of one payload, following the shapes the
/// configured providers actually send.
fn from_value(v: &serde_json::Value) -> Option<ErrorSummary> {
    // `{"error": {...}}` (OpenAI, Anthropic, most gateways) — and one level
    // deeper for gateways that re-wrap an upstream body.
    let candidates = [
        v.get("error").and_then(|e| e.get("error")),
        v.get("error"),
        Some(v),
    ];
    for node in candidates.into_iter().flatten() {
        // `{"error": "some text"}` — the whole error is the string.
        if let Some(s) = node.as_str() {
            let s = s.trim();
            if !s.is_empty() {
                return Some(ErrorSummary {
                    headline: s.to_string(),
                    code: None,
                    param: None,
                });
            }
        }
        let headline = str_field(node, "message")
            .or_else(|| str_field(node, "detail"))
            .or_else(|| str_field(node, "reason"));
        if let Some(headline) = headline {
            return Some(ErrorSummary {
                headline,
                code: str_field(node, "code").or_else(|| str_field(node, "type")),
                param: str_field(node, "param"),
            });
        }
    }
    None
}

/// How much of the end of the text is scanned.
///
/// The connection log is append-only and uncapped for a whole session, and the
/// error modal re-renders every frame — so scanning all of it, every frame,
/// would get slower the longer the IDE stays open. The front of a long log is
/// both the expensive part and the part that cannot hold the answer: the
/// payload being asked about is the most recent one. 64 KiB is orders of
/// magnitude more than any provider error body.
const TAIL_SCAN_BYTES: usize = 64 * 1024;

/// The reason to put at the top of the error modal, or `None` when the text
/// carries no provider payload to quote — in which case the log is shown as it
/// always was, with nothing invented above it.
///
/// The **last** payload wins: the connection log is append-only, so the most
/// recent attempt is the one being asked about.
pub fn summarize(raw: &str) -> Option<ErrorSummary> {
    let tail = if raw.len() > TAIL_SCAN_BYTES {
        let mut cut = raw.len() - TAIL_SCAN_BYTES;
        // Never slice through a multi-byte character.
        while cut < raw.len() && !raw.is_char_boundary(cut) {
            cut += 1;
        }
        &raw[cut..]
    } else {
        raw
    };
    json_spans(tail).iter().rev().find_map(from_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator's first case: a model behind a paid plan. The reason is one
    /// sentence inside a body that also carries request ids, types and a stack
    /// of log lines around it.
    #[test]
    fn a_subscription_error_is_quoted_not_hunted_for() {
        let raw = "\
POST https://api.openai.com/v1/chat/completions
headers: {\"authorization\": \"Bearer sk-…\"}
stream error: HttpError: Invalid status code 429
body: {\"error\":{\"message\":\"You exceeded your current quota, please check your plan and billing details.\",\"type\":\"insufficient_quota\",\"param\":null,\"code\":\"insufficient_quota\"}}
";
        let s = summarize(raw).expect("a payload is present");
        assert_eq!(
            s.headline,
            "You exceeded your current quota, please check your plan and billing details."
        );
        assert_eq!(s.code.as_deref(), Some("insufficient_quota"));
        assert_eq!(s.param, None, "a JSON null param is not a parameter");
    }

    /// The operator's second case: a parameter this model does not take. The
    /// field that names it is the whole answer, so it is carried through.
    #[test]
    fn an_unsupported_parameter_names_the_parameter() {
        let raw = "body: {\"error\":{\"message\":\"Unsupported value: 'temperature' does not \
                   support 0.1 with this model.\",\"type\":\"invalid_request_error\",\
                   \"param\":\"temperature\",\"code\":\"unsupported_value\"}}";
        let s = summarize(raw).expect("a payload is present");
        assert!(s.headline.starts_with("Unsupported value:"));
        assert_eq!(s.param.as_deref(), Some("temperature"));
        // The message already names it, so the one-liner does not repeat it.
        assert!(
            !s.line().contains("parameter: temperature"),
            "no redundant restatement: {}",
            s.line()
        );
        assert!(s.line().contains("unsupported_value"), "{}", s.line());
    }

    /// Anthropic nests differently and calls the kind `type`. Same treatment,
    /// no per-provider special case.
    #[test]
    fn an_anthropic_permission_error_reads_the_same_way() {
        let raw = "{\"type\":\"error\",\"error\":{\"type\":\"permission_error\",\
                   \"message\":\"Your credit balance is too low to access the Claude API.\"}}";
        let s = summarize(raw).expect("a payload is present");
        assert_eq!(
            s.headline,
            "Your credit balance is too low to access the Claude API."
        );
        assert_eq!(s.code.as_deref(), Some("permission_error"));
    }

    /// A gateway that re-wraps the upstream body must not stop at its own
    /// envelope and report nothing useful.
    #[test]
    fn a_double_wrapped_gateway_body_still_yields_the_upstream_reason() {
        let raw = "{\"error\":{\"error\":{\"message\":\"This model requires a Pro subscription.\",\
                   \"code\":\"subscription_required\"},\"status\":403}}";
        let s = summarize(raw).expect("a payload is present");
        assert_eq!(s.headline, "This model requires a Pro subscription.");
        assert_eq!(s.code.as_deref(), Some("subscription_required"));
    }

    /// An append-only log holds every attempt. The one being asked about is the
    /// last.
    #[test]
    fn the_most_recent_payload_wins() {
        let raw = "\
attempt 1: {\"error\":{\"message\":\"first failure\"}}
attempt 2: {\"error\":{\"message\":\"second failure\"}}
";
        assert_eq!(summarize(raw).unwrap().headline, "second failure");
    }

    /// `{\"error\": \"text\"}` — some providers send the whole thing as a string.
    #[test]
    fn a_bare_string_error_is_taken_as_the_message() {
        let raw = "{\"error\":\"model not found\"}";
        assert_eq!(summarize(raw).unwrap().headline, "model not found");
    }

    /// **Nothing is invented.** Text with no payload gets no headline, and the
    /// modal shows the log exactly as it did before — an error the extractor
    /// does not understand must not be given a confident-looking summary.
    #[test]
    fn text_with_no_payload_summarises_to_nothing() {
        assert!(summarize("connection refused (os error 61)").is_none());
        assert!(summarize("").is_none());
        assert!(summarize("{ not json at all }").is_none());
        assert!(
            summarize("{\"usage\":{\"input_tokens\":12}}").is_none(),
            "a body with no error field is not an error"
        );
    }

    /// A session-long log still finds the newest payload, and does it without
    /// reading the whole thing — the modal re-renders every frame, so this runs
    /// far more often than it fails.
    #[test]
    fn a_huge_log_is_scanned_from_the_end() {
        let mut raw = String::new();
        raw.push_str("{\"error\":{\"message\":\"ancient failure\"}}\n");
        // Comfortably past the tail window, with multi-byte characters in it so
        // a careless slice would panic on a char boundary.
        while raw.len() < TAIL_SCAN_BYTES * 2 {
            raw.push_str("POST /v1/chat — 200 OK — café ☕ noise noise noise\n");
        }
        raw.push_str("{\"error\":{\"message\":\"the one that matters\"}}\n");

        let s = summarize(&raw).expect("the recent payload is in the tail");
        assert_eq!(s.headline, "the one that matters");
    }

    /// A truncated log must terminate, not spin looking for a closing brace.
    #[test]
    fn an_unbalanced_tail_terminates() {
        let raw = "{\"error\":{\"message\":\"real one\"}} then a cut-off body: {\"error\":{\"mess";
        assert_eq!(summarize(raw).unwrap().headline, "real one");
    }
}
