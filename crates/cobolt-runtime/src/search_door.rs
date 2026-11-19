// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The WebSearch control's own rules**, in one place for every host that runs one: the
//! interpreter, and the desktop runner of a program built on the WebAssembly path
//! (DESKTOP-WASM-001 T47a).
//!
//! A search is a RestClient request whose address, headers and body come from the control's
//! search properties ([`crate::search_runtime`] builds them for each provider), so its `Mode`,
//! `TimeoutMs`, `Busy` and endings are the RestClient's ([`crate::rest_door`]). What is its own:
//! the key (the control's `ApiKey`, else the one the host resolved), the "not configured" refusal
//! before anything is sent, `onResultsReceived` before the uniform `onComplete`, the accessors over
//! the last answer, and what `Verbose` narrates, with every credential masked.
//!
//! The control's properties are read through a getter, so neither host's storage leaks in.

use crate::http_runtime::RequestConfig;
use crate::rest_door::{self, Prop, Request};
use crate::search_runtime::{self, Provider, SearchParams};

/// A URL with any credential in its query string masked.
///
/// Google Custom Search signs in the query (`?key=…`), so a verbose line that
/// printed the URL verbatim would put the developer's key in the program's
/// output — and that output gets pasted into bug reports and forum posts. The
/// convention is already set by the AgentObject's verbose switch, which logs
/// "whether an API key is set (never the key itself)".
///
/// Enough of the value survives to tell two keys apart and to see that one is
/// present at all, which is what the switch is for.
pub fn redact_query_secrets(url: &str) -> String {
    const SECRET_PARAMS: [&str; 3] = ["key", "api_key", "apikey"];
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let masked: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) if SECRET_PARAMS.iter().any(|p| p.eq_ignore_ascii_case(name)) => {
                format!("{name}={}", mask_secret(value))
            }
            _ => pair.to_owned(),
        })
        .collect();
    format!("{base}?{}", masked.join("&"))
}

/// `abcd…(24 chars)`, or `(empty)` — never the whole thing.
pub fn mask_secret(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return "(empty)".to_owned();
    }
    let head: String = v.chars().take(4).collect();
    format!("{head}…({} chars)", v.chars().count())
}

/// Is the control's `Verbose` switch on? It narrates the whole search into the program's output.
pub fn verbose(prop: Prop<'_>) -> bool {
    prop("Verbose").eq_ignore_ascii_case("true")
}

/// The results of the last answer, `(title, snippet, link)` each, one per result (spec 039
/// T15/R29).
///
/// Five providers answer in five shapes — Google's `items[]`, Brave's `web.results[]`, Serper's
/// `organic[]`, Tavily's and SearXNG's `results[]`, each naming the snippet and the link
/// differently. `search_runtime` owns that table, so the accessors read the same triples whichever
/// back end answered. An empty or malformed body (not yet searched, or an error payload) yields an
/// empty Vec rather than an error: absent data reads as nothing, not a crash.
pub fn items(prop: Prop<'_>) -> Vec<(String, String, String)> {
    search_runtime::parse_results(Provider::parse(&prop("Provider")), &prop("ResponseBody"))
}

/// What a method call or a search's ending does to the control: the lines it narrates on the
/// program's output, then the properties written, in order, then the events raised, in order, and
/// the method's answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    pub lines: Vec<String>,
    pub props: Vec<(String, String)>,
    pub events: Vec<String>,
    pub answer: String,
}

impl Effects {
    fn after(lines: Vec<String>, fx: rest_door::Effects) -> Self {
        Effects { lines, props: fx.props, events: fx.events, answer: fx.answer }
    }
}

/// What a WebSearch method asks for.
#[derive(Clone, Debug)]
pub enum Start {
    /// Not one of the search's own methods (`Cancel`, `IsBusy`, `GetResult()` with no argument, a
    /// property): the host's own.
    NotOwn,
    /// An answer read from the last response: `ResultCount`, `TopTitle`, `TopSnippet`, `TopLink`,
    /// `GetResult(n)`.
    Answer(String),
    /// `Search` on a control that is not configured: nothing is sent, and this is what follows.
    NotConfigured(Effects),
    /// `Search`: the request to send, and what `Verbose` narrates of it before it goes.
    Request { request: Request, lines: Vec<String> },
}

/// The answer of an accessor over the last answer (`method` upper case, the argument texts `args`),
/// or `None` when `method` is not one.
pub fn answer(prop: Prop<'_>, method: &str, args: &[String]) -> Option<String> {
    let first = |pick: fn(&(String, String, String)) -> &String| items(prop).first().map(|r| pick(r).clone()).unwrap_or_default();
    match method {
        "RESULTCOUNT" => Some(items(prop).len().to_string()),
        "TOPTITLE" => Some(first(|r| &r.0)),
        "TOPSNIPPET" => Some(first(|r| &r.1)),
        "TOPLINK" => Some(first(|r| &r.2)),
        // `GetResult()` with no argument is every control's `Result`, the host's own.
        "GETRESULT" if !args.is_empty() => {
            let n: usize = args[0].trim().parse().unwrap_or(0);
            let items = items(prop);
            Some(if n >= 1 && n <= items.len() {
                let (title, snippet, link) = &items[n - 1];
                format!("{title}\t{snippet}\t{link}")
            } else {
                String::new()
            })
        }
        _ => None,
    }
}

/// What the WebSearch `control` asks for with the method `method` (upper case) and the argument
/// texts `args`.
pub fn start(prop: Prop<'_>, control: &str, method: &str, args: &[String]) -> Start {
    if method == "SEARCH" {
        return search(prop, control);
    }
    match answer(prop, method, args) {
        Some(a) => Start::Answer(a),
        None => Start::NotOwn,
    }
}

/// `Search`: the query, the key and the provider are the control's properties, never the call's
/// arguments.
fn search(prop: Prop<'_>, control: &str) -> Start {
    let provider = Provider::parse(&prop("Provider"));
    // The control's own ApiKey wins; otherwise the project's key, which the host resolved. A form
    // that needs a different account than the project default says so on the control instead of
    // forcing a second project.
    let own_key = prop("ApiKey");
    let api_key = if own_key.trim().is_empty() { prop("_ResolvedSearchApiKey") } else { own_key };
    let endpoint = prop("Endpoint");
    let verbose = verbose(prop);
    let block = |label: &str, text: &str| crate::narration::block("search", control, label, text);
    if let Some(err) = search_runtime::configuration_error(provider, &api_key, &endpoint) {
        // R33: "not configured" — fail at once, no request.
        let lines = if verbose { block("not configured", &err) } else { Vec::new() };
        return Start::NotConfigured(Effects { lines, props: vec![("LastError".into(), err)], events: vec!["onError".into()], answer: String::new() });
    }
    let (engine_id, query, safe) = (prop("SearchEngineId"), prop("Query"), prop("SafeSearch"));
    let req = search_runtime::build_request(
        provider,
        &SearchParams {
            api_key: &api_key,
            engine_id: &engine_id,
            endpoint: &endpoint,
            query: &query,
            num_results: prop("NumResults").trim().parse::<u32>().unwrap_or(10),
            safe_search: &safe,
        },
    );
    let sync = !rest_door::is_async(prop);
    let timeout = rest_door::timeout_ms(prop);
    // Brave, Serper and Tavily authenticate in a header, so the request carries a config of its
    // own. `TimeoutMs` bounds a search that runs now; one on its own thread gets five seconds more,
    // a backstop, as a RestClient's does, while its ending owns `onTimeout`.
    let cfg = RequestConfig {
        headers: req.headers.clone(),
        timeout_ms: if sync {
            timeout
        } else if timeout > 0 {
            timeout.saturating_add(5_000)
        } else {
            0
        },
        ..Default::default()
    };
    let mut lines = Vec::new();
    if verbose {
        // Everything needed to compare this against the provider's own documentation — except the
        // credential, which is masked. Program output gets pasted into bug reports.
        lines.extend(block("provider", provider.as_str()));
        lines.extend(block("method", req.method));
        lines.extend(block("url", &redact_query_secrets(&req.url)));
        let headers: String = req
            .headers
            .iter()
            .map(|(n, v)| {
                let secret = n.eq_ignore_ascii_case("authorization") || n.eq_ignore_ascii_case("x-api-key") || n.eq_ignore_ascii_case("x-subscription-token");
                if secret {
                    format!("{n}: {}", mask_secret(v))
                } else {
                    format!("{n}: {v}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        lines.extend(block("headers", &headers));
        lines.extend(block("body", &req.body));
        lines.extend(block("mode", if sync { "sync" } else { "async" }));
    }
    let body = (!req.body.is_empty()).then_some(req.body);
    Start::Request { request: Request { verb: req.method.to_owned(), url: req.url, body, cfg, sync, timeout_ms: timeout }, lines }
}

/// A search that ran now answered `body` with `status` (0: no answer came back, `body` saying why):
/// `ResponseBody` and `StatusCode` hold them, and the body is the method's value.
pub fn finished(verbose: bool, control: &str, body: &str, status: u16) -> Effects {
    let mut lines = Vec::new();
    if verbose {
        lines.extend(crate::narration::block("search", control, "status", &status.to_string()));
        lines.extend(crate::narration::block("search", control, "response", body));
    }
    Effects::after(lines, rest_door::finished(body, status))
}

/// A search on its own thread ended with `body` and `status`: a RestClient's ending
/// ([`rest_door::delivered`]), with the control's own `onResultsReceived` raised before
/// `onComplete` when an answer came back.
pub fn delivered(verbose: bool, control: &str, body: &str, status: u16) -> Effects {
    let mut lines = Vec::new();
    let mut fx = rest_door::delivered(body, status);
    if status == 0 {
        if verbose {
            lines.extend(crate::narration::block("search", control, "transport error", body));
        }
    } else {
        if verbose {
            lines.extend(crate::narration::block("search", control, "status", &status.to_string()));
            lines.extend(crate::narration::block("search", control, "response", body));
        }
        // The control's primary event, the one a double-click in the designer binds; the uniform
        // `onComplete` follows, so a form bound to it is served too.
        let at = fx.events.iter().position(|e| e == "onComplete").unwrap_or(fx.events.len());
        fx.events.insert(at, "onResultsReceived".into());
    }
    Effects::after(lines, fx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_ascii_uppercase(), (*v).to_owned())).collect()
    }

    #[test]
    fn a_search_asks_for_what_the_interpreter_sends_and_ends_as_it_does() {
        let mut checks = 0;
        // Google with no key anywhere: not configured, nothing sent.
        let p = props(&[("Provider", "Google"), ("Verbose", "true")]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let Start::NotConfigured(fx) = start(&get, "WEB-1", "SEARCH", &[]) else { panic!("no key: not configured") };
        assert_eq!(fx.events, ["onError"]);
        assert_eq!(fx.props, [("LastError".to_string(), "Google search API key not configured".to_string())]);
        assert_eq!(fx.lines, ["[search WEB-1] not configured: Google search API key not configured"]);
        checks += 3;
        // The project's key, resolved by the host, configures it; the URL shown masks it.
        let p = props(&[("Provider", "Google"), ("_ResolvedSearchApiKey", "secret-project-key"), ("Query", "a b"), ("Verbose", "true"), ("TimeoutMs", "2000")]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let Start::Request { request, lines } = start(&get, "WEB-1", "SEARCH", &["ignored".into()]) else { panic!("a key: a request") };
        assert!(request.url.contains("key=secret-project-key") && request.url.contains("q=a%20b"), "{}", request.url);
        assert!(!request.sync && request.timeout_ms == 2_000 && request.cfg.timeout_ms == 7_000 && request.body.is_none());
        assert!(lines.iter().all(|l| !l.contains("secret-project-key")) && lines.iter().any(|l| l.contains("key=secr…(18 chars)")), "{lines:?}");
        assert_eq!(lines.last().map(String::as_str), Some("[search WEB-1] mode: async"));
        checks += 4;
        // The control's own key wins over the project's.
        let p = props(&[("Provider", "Google"), ("ApiKey", "own"), ("_ResolvedSearchApiKey", "project"), ("Mode", "Sync")]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let Start::Request { request, lines } = start(&get, "WEB-1", "SEARCH", &[]) else { panic!("own key: a request") };
        assert!(request.url.contains("key=own&") && request.sync && lines.is_empty());
        checks += 1;
        // SearXNG: the address is the control's Endpoint.
        let p = props(&[("Provider", "SearXNG"), ("Endpoint", "http://127.0.0.1:9/")]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let Start::Request { request, .. } = start(&get, "WEB-1", "SEARCH", &[]) else { panic!("SearXNG: a request") };
        assert!(request.url.starts_with("http://127.0.0.1:9/"), "{}", request.url);
        checks += 1;
        // The accessors over the last answer.
        let body = r#"{"results":[{"title":"T1","content":"S1","url":"https://one"},{"title":"T2","content":"S2","url":"https://two"}]}"#;
        let p = props(&[("Provider", "SearXNG"), ("ResponseBody", body)]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let ask = |m: &str, a: &[&str]| match start(&get, "WEB-1", m, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>()) {
            Start::Answer(a) => Some(a),
            _ => None,
        };
        assert_eq!(ask("RESULTCOUNT", &[]).as_deref(), Some("2"));
        assert_eq!((ask("TOPTITLE", &[]), ask("TOPSNIPPET", &[]), ask("TOPLINK", &[])), (Some("T1".into()), Some("S1".into()), Some("https://one".into())));
        assert_eq!(ask("GETRESULT", &["02"]).as_deref(), Some("T2\tS2\thttps://two"));
        assert_eq!(ask("GETRESULT", &["3"]).as_deref(), Some(""));
        assert!(ask("GETRESULT", &[]).is_none() && ask("CANCEL", &[]).is_none());
        checks += 5;
        // The endings.
        assert_eq!(delivered(false, "WEB-1", "{}", 200).events, ["onResultsReceived", "onComplete"]);
        assert_eq!(delivered(false, "WEB-1", "{}", 500).events, ["onResultsReceived", "onComplete"]);
        let fx = delivered(true, "WEB-1", "refused", 0);
        assert_eq!((fx.events.as_slice(), fx.lines.as_slice()), (["onError".to_string()].as_slice(), ["[search WEB-1] transport error: refused".to_string()].as_slice()));
        let fx = finished(true, "WEB-1", "{}", 200);
        assert_eq!((fx.answer.as_str(), fx.lines.len(), fx.events.len()), ("{}", 2, 0));
        checks += 4;
        // Masking.
        assert_eq!(redact_query_secrets("https://api.tavily.com/search"), "https://api.tavily.com/search");
        assert_eq!(mask_secret("   "), "(empty)");
        checks += 2;
        println!("search_door: {checks} of {checks} checks (not configured, the key's precedence, masking, SearXNG's address, the accessors, the endings)");
    }
}
