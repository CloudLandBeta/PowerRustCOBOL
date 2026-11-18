// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The RestClient control's own rules**, in one place for every host that runs one: the
//! interpreter, and the desktop runner of a program built on the WebAssembly path
//! (DESKTOP-WASM-001 T42b).
//!
//! The control's properties are read through a getter, so neither host's storage leaks in. What a
//! method asks for ([`start`]): the address (`BaseURL` and the argument), the verb, the body, the
//! headers and credentials the control carries, its redirect and TLS policy, its timeout, and
//! whether it runs now (`Mode = Sync`) or on a thread of its own. What each ending writes and raises:
//! [`finished`] for a request that ran now, [`delivered`] for one that ran on its own thread,
//! [`timed_out`] when its `TimeoutMs` passed first.
//!
//! The data items a control may name (`RequestDataItem`, `ResponseDataItem`, `StatusDataItem`) are
//! the host's own business: the interpreter reads and writes them by name, and a program built on
//! the WebAssembly path takes the Cargo path when a RestClient names one.

use crate::http_runtime::RequestConfig;

/// Does this string carry its own URL scheme (`https://...`)?
///
/// The test that decides whether a `RestClient` call argument is a complete
/// address or a path to resolve against the control's `BaseURL`.
pub fn url_has_scheme(s: &str) -> bool {
    match s.find("://") {
        Some(i) if i > 0 => {
            let scheme = &s[..i];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        _ => false,
    }
}

/// A control's property, as its program would read it.
pub type Prop<'a> = &'a dyn Fn(&str) -> String;

/// Unless `Mode = Sync`, a verb runs on a thread of its own (spec 032).
pub fn is_async(prop: Prop<'_>) -> bool {
    !prop("Mode").trim().eq_ignore_ascii_case("sync")
}

/// Effective REST timeout in milliseconds: `TimeoutMs` if set, else the
/// legacy `TimeoutSeconds × 1000`, else 0 (no interpreter-side timeout).
pub fn timeout_ms(prop: Prop<'_>) -> u64 {
    let ms = prop("TimeoutMs").trim().parse::<u64>().unwrap_or(0);
    if ms > 0 {
        return ms;
    }
    let secs = prop("TimeoutSeconds").trim().parse::<u64>().unwrap_or(0);
    secs.saturating_mul(1000)
}

/// The address a `RestClient` verb actually requests.
///
/// `BaseURL` is seeded on every RestClient and was read by nothing, so the
/// idiomatic handler — address configured in the designer, `REST-1::get()`
/// with no argument — requested the empty string and could only fail. The
/// call argument still wins whenever it carries its own scheme, so every
/// call that worked before resolves to exactly the same URL:
///
/// * no argument       -> `BaseURL`
/// * absolute argument -> the argument, untouched
/// * relative argument -> `BaseURL` + `/` + the argument
/// * `?...` / `#...`   -> appended to `BaseURL` directly
///
/// With no `BaseURL` configured the argument is the URL, which is what the
/// runtime did for its whole life before this.
pub fn url(prop: Prop<'_>, arg: &str) -> String {
    let arg = arg.trim();
    let base = prop("BaseURL");
    let base = base.trim();
    if arg.is_empty() {
        return base.to_string();
    }
    if base.is_empty() || url_has_scheme(arg) {
        return arg.to_string();
    }
    let base = base.trim_end_matches('/');
    if arg.starts_with('?') || arg.starts_with('#') {
        format!("{base}{arg}")
    } else {
        format!("{base}/{}", arg.trim_start_matches('/'))
    }
}

/// A boolean control property, tolerant of every spelling one is written in
/// across the designer, the `.cfrm` and the runtime (`true` / `1` / `on`).
/// An unset or unrecognised value keeps `default` rather than reading as
/// false — a missing `VerifyTLS` must not silently stop verifying.
pub fn ctrl_bool(prop: Prop<'_>, name: &str, default: bool) -> bool {
    let v = prop(name);
    let v = v.trim();
    if v.is_empty() {
        return default;
    }
    if v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on") {
        true
    } else if v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off") {
        false
    } else {
        default
    }
}

/// The transport configuration a `RestClient` control carries: its default
/// headers, its credentials, and its redirect / TLS policy.
///
/// Every one of these was seeded on the control, shown in the property pane
/// and documented in the System KB while being read by nothing at all.
pub fn config(prop: Prop<'_>) -> RequestConfig {
    let mut headers: Vec<(String, String)> = Vec::new();

    // `DefaultHeaders` -- `key: value`, one per line. A line carrying no
    // colon is not a header, so it is skipped rather than sent malformed.
    let raw = prop("DefaultHeaders");
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() {
                headers.push((k.to_string(), v.to_string()));
            }
        }
    }

    // `AuthType` / `AuthToken`. An empty token sends no header at all: an
    // `Authorization: Bearer ` with nothing after it is a 401 whose cause
    // reads like a server fault rather than an unconfigured control.
    let token = prop("AuthToken");
    let token = token.trim();
    if !token.is_empty() {
        let scheme = prop("AuthType");
        match scheme.trim().to_ascii_uppercase().as_str() {
            "BEARER" => headers.push(("Authorization".into(), format!("Bearer {token}"))),
            "BASIC" => {
                // `user:password` is what a developer types into the
                // property pane; a token with no colon is already base64
                // and is sent as it stands.
                let value = if token.contains(':') {
                    crate::http_runtime::encode_base64(token.as_bytes())
                } else {
                    token.to_string()
                };
                headers.push(("Authorization".into(), format!("Basic {value}")));
            }
            // No header name is universal for an API key. `X-API-Key` is
            // the common one and is what the KB documents; an API wanting
            // another spelling sets it through `DefaultHeaders`.
            "APIKEY" => headers.push(("X-API-Key".into(), token.to_string())),
            _ => {}
        }
    }

    RequestConfig {
        timeout_ms: 0, // set per call site: sync uses it directly, async adds a backstop
        follow_redirects: ctrl_bool(prop, "FollowRedirects", true),
        verify_tls: ctrl_bool(prop, "VerifyTLS", true),
        headers,
    }
}

/// A request a RestClient method asks for.
#[derive(Clone, Debug)]
pub struct Request {
    /// The verb, upper case (`GET`, `POST`, `PUT`, `DELETE`, or what `Call` names).
    pub verb: String,
    pub url: String,
    /// The body sent: `Some` for `POST`, `PUT` and `PATCH` only.
    pub body: Option<String>,
    /// The control's headers, credentials and policy, with the transport timeout this request takes:
    /// `TimeoutMs` for one that runs now; for one on its own thread, five seconds more, a backstop so
    /// a stalled request cannot outlive its thread, while [`timed_out`] owns `onTimeout`.
    pub cfg: RequestConfig,
    /// It runs now (`Mode = Sync`): its answer is the method's value.
    pub sync: bool,
    /// The control's `TimeoutMs` (0: none). On its own thread, [`timed_out`] ends it after this long.
    pub timeout_ms: u64,
}

/// What a RestClient method asks for.
#[derive(Clone, Debug)]
pub enum Start {
    /// Not one of the control's request methods (`Cancel`, `IsBusy`, a property: the host's own).
    NotOwn,
    /// A request.
    Request(Request),
    /// `SetTimeout( value )`: the value written to the control's `Timeout`.
    SetTimeout(String),
    /// `SetHeader( name value )`: a header on the window's HTTP client, the one the HTTP built-ins use.
    SetHeader(String, String),
    /// `ClearHeaders()`: the window's HTTP client sends no header set that way any more.
    ClearHeaders,
}

/// The body a RestClient sends: the verb's own argument, or — when that is empty — the value of the
/// item named by `RequestDataItem` (`request_item`), when the host has one.
pub fn body(given: String, request_item: &dyn Fn() -> Option<String>) -> String {
    if !given.trim().is_empty() {
        return given;
    }
    request_item().unwrap_or(given)
}

/// What a RestClient method `method` (upper case) with the argument texts `args` asks for.
/// `request_item` is the value of the data item `RequestDataItem` names, when the host has one: the
/// body sent when the verb's own is empty.
pub fn start(prop: Prop<'_>, method: &str, args: &[String], request_item: &dyn Fn() -> Option<String>) -> Start {
    let arg = |i: usize| args.get(i).map(|a| a.trim().to_string()).unwrap_or_default();
    let sent = |given: String| body(given, request_item);
    let sync = !is_async(prop);
    let timeout = timeout_ms(prop);
    let request = |verb: &str, url_arg: &str, body: Option<String>| {
        let mut cfg = config(prop);
        cfg.timeout_ms = if sync {
            timeout
        } else if timeout > 0 {
            timeout.saturating_add(5_000)
        } else {
            0
        };
        Start::Request(Request { verb: verb.to_string(), url: url(prop, url_arg), body, cfg, sync, timeout_ms: timeout })
    };
    match method {
        "GET" => request("GET", &arg(0), None),
        "DELETE" => request("DELETE", &arg(0), None),
        "POST" | "PUT" => request(method, &arg(0), Some(sent(arg(1)))),
        "CALL" => {
            // An omitted verb falls back to `DefaultMethod`, which is what
            // "Designer default verb" means and the only place it applies.
            let mut verb = arg(0).to_ascii_uppercase();
            if verb.is_empty() {
                verb = prop("DefaultMethod").trim().to_ascii_uppercase();
            }
            if verb.is_empty() {
                verb = "GET".to_string();
            }
            let has_body = matches!(verb.as_str(), "POST" | "PUT" | "PATCH");
            request(&verb, &arg(1), has_body.then(|| sent(arg(2))))
        }
        "SETTIMEOUT" => Start::SetTimeout(arg(0)),
        "SETHEADER" => Start::SetHeader(arg(0), arg(1)),
        "CLEARHEADERS" => Start::ClearHeaders,
        _ => Start::NotOwn,
    }
}

/// What a request's ending, or its start, does to the control: the properties written, in order,
/// then the events raised, in order, and the method's answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    pub props: Vec<(String, String)>,
    pub events: Vec<String>,
    pub answer: String,
}

/// A request that ran now answered `body` with `status` (0: no answer came back, `body` saying why):
/// `ResponseBody` and `StatusCode` hold them, and the body is the method's value.
pub fn finished(body: &str, status: u16) -> Effects {
    Effects {
        props: vec![("ResponseBody".into(), body.to_owned()), ("StatusCode".into(), status.to_string())],
        events: Vec::new(),
        answer: body.to_owned(),
    }
}

/// A request started on its own thread: the control is `Busy`, and the method answers nothing yet.
pub fn started() -> Effects {
    Effects { props: vec![("Busy".into(), "1".into())], events: Vec::new(), answer: String::new() }
}

/// A request on its own thread ended with `body` and `status`. A transport error (status 0, the body
/// saying why) writes `LastError` and `StatusCode = 0`, leaves `ResponseBody` as it was, and raises
/// `onError`; an answer, whatever its status, writes `ResponseBody` and `StatusCode` and raises
/// `onComplete`. Either way the control is no longer `Busy`.
pub fn delivered(body: &str, status: u16) -> Effects {
    if status == 0 {
        Effects {
            props: vec![("LastError".into(), body.to_owned()), ("StatusCode".into(), "0".into()), ("Busy".into(), "0".into())],
            events: vec!["onError".into()],
            answer: String::new(),
        }
    } else {
        let mut fx = finished(body, status);
        fx.props.push(("Busy".into(), "0".into()));
        fx.events.push("onComplete".into());
        fx.answer = String::new();
        fx
    }
}

/// A request on its own thread outlived its `TimeoutMs` (`ms`): `LastError` says so, the control is
/// no longer `Busy`, and `onTimeout` is raised; its answer, if one comes, is dropped.
pub fn timed_out(ms: u64) -> Effects {
    Effects {
        props: vec![("LastError".into(), crate::async_op::timed_out(ms)), ("Busy".into(), "0".into())],
        events: vec!["onTimeout".into()],
        answer: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_ascii_uppercase(), (*v).to_owned())).collect()
    }

    #[test]
    fn a_method_asks_for_the_request_the_interpreter_sends() {
        let p = props(&[
            ("BaseURL", "https://api.example.com/v1/"),
            ("DefaultMethod", "post"),
            ("TimeoutMs", "2500"),
            ("AuthType", "Basic"),
            ("AuthToken", "ada:secret"),
            ("DefaultHeaders", "X-One: 1\nnot a header\n# a comment\nX-Two :2"),
            ("FollowRedirects", "off"),
        ]);
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let none = || None;
        let mut checks = 0;
        let Start::Request(r) = start(&get, "GET", &["users/7".into()], &none) else { panic!("GET is a request") };
        assert_eq!((r.verb.as_str(), r.url.as_str(), r.body.as_deref(), r.sync), ("GET", "https://api.example.com/v1/users/7", None, false));
        assert_eq!(r.cfg.timeout_ms, 7_500, "on its own thread: TimeoutMs and a five-second backstop");
        assert_eq!(r.timeout_ms, 2_500);
        assert!(!r.cfg.follow_redirects && r.cfg.verify_tls);
        assert_eq!(
            r.cfg.headers,
            vec![
                ("X-One".to_string(), "1".to_string()),
                ("X-Two".to_string(), "2".to_string()),
                ("Authorization".to_string(), format!("Basic {}", crate::http_runtime::encode_base64(b"ada:secret"))),
            ]
        );
        checks += 6;
        let item = || Some("{\"from\":\"item\"}".to_string());
        let Start::Request(r) = start(&get, "CALL", &[String::new(), "?page=2".into()], &item) else { panic!("CALL is a request") };
        assert_eq!((r.verb.as_str(), r.url.as_str(), r.body.as_deref()), ("POST", "https://api.example.com/v1?page=2", Some("{\"from\":\"item\"}")));
        let Start::Request(r) = start(&get, "PUT", &["https://other.example/x".into(), "body".into()], &item) else { panic!("PUT is a request") };
        assert_eq!((r.url.as_str(), r.body.as_deref()), ("https://other.example/x", Some("body")));
        checks += 2;
        let sync = props(&[("Mode", "Sync"), ("TimeoutSeconds", "3")]);
        let get = |k: &str| sync.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let Start::Request(r) = start(&get, "DELETE", &["https://x.example/a".into()], &none) else { panic!("DELETE is a request") };
        assert!(r.sync && r.cfg.timeout_ms == 3_000 && r.body.is_none());
        assert!(matches!(start(&get, "SETTIMEOUT", &[" 9 ".into()], &none), Start::SetTimeout(t) if t == "9"));
        assert!(matches!(start(&get, "CANCEL", &[], &none), Start::NotOwn));
        checks += 3;
        assert_eq!(delivered("refused", 0).events, ["onError"]);
        assert_eq!(delivered("{}", 404).events, ["onComplete"]);
        assert_eq!(timed_out(1500).events, ["onTimeout"]);
        checks += 3;
        println!("rest_door: {checks} of {checks} checks (address, verb, body, headers, credentials, policy, timeouts, endings)");
    }
}
