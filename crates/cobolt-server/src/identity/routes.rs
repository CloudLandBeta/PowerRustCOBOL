// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `/auth/*` routes (WASM-009 D5), transport-neutral: an [`AuthRequest`] in, an [`AuthResponse`]
//! out. `serve.rs` mounts them beside `/health` and `/dl/`; if the HTTP layer changes, only that
//! mount changes (plan §9 R3).
//!
//! Every operation runs its generated `AUTH-*` program (D4) through the runner, and the program asks
//! the identity service through `COBOL::"AUTH-OP"`. What only HTTP can do happens here, around the
//! program: the refresh token goes into a sealed `HttpOnly` cookie and never into the page's answer;
//! a hosted sign-in's continuation rides in its own cookie; the hosted return carries a single-use
//! handle, never a token.
//!
//! The closed table:
//!
//! | method  | route                                                                 |
//! |---------|-----------------------------------------------------------------------|
//! | POST    | sign-in, sign-up, confirm, resend, forgot, reset, mfa, mfa-setup, refresh, sign-out, hosted/finish |
//! | GET     | capabilities, hosted/start, hosted/callback                           |
//! | OPTIONS | any of the above (the CORS preflight)                                 |
//!
//! Every POST needs `Content-Type: application/json`, `X-PR-Identity: 1` (a header no HTML form can
//! send, so a cross-site form cannot post here) and an `Origin` in `[identity] origins`; otherwise 403.
//! A request with no `Origin` is accepted only from the mobile shell's own Rust side
//! (`X-PR-Client: native`, D6), which receives the refresh token in the body instead of a cookie: a
//! browser always sends `Origin` with a POST, so no page can take that path.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use cobolt_wire::error::{CallError, ErrorClass};

use crate::backend::Backend;
use crate::http::{self, SameSite, SetCookie};
use crate::identity::client::percent;
use crate::identity::oidc::random_token;
use crate::identity::ops::{self, IdentityService, OpContext, HOSTED_AGE, REQUEST_MEMBERS};

/// The cookie that holds the sealed refresh token and the user's name.
pub const REFRESH_COOKIE: &str = "__Host-pr_refresh";
/// The cookie that holds a hosted sign-in in flight.
pub const HOSTED_COOKIE: &str = "__Host-pr_hosted";
/// The longest a refresh cookie is kept and accepted (D7). The provider's own lifetime ends a session
/// sooner when it is shorter; this bounds a cookie the provider would still honour.
pub const REFRESH_AGE: Duration = Duration::from_secs(30 * 24 * 3600);
/// How long the handle of a hosted return is good for, and how many may wait at once.
pub const HANDLE_AGE: Duration = Duration::from_secs(60);
const MAX_HANDLES: usize = 10_000;

/// The POST routes that run one operation each.
const OPERATION_ROUTES: &[(&str, &str)] = &[
    ("sign-in", "SIGN-IN"),
    ("sign-up", "SIGN-UP"),
    ("confirm", "CONFIRM"),
    ("resend", "RESEND"),
    ("forgot", "FORGOT"),
    ("reset", "RESET"),
    ("mfa", "MFA"),
    ("mfa-setup", "MFA-SETUP"),
    ("refresh", "REFRESH"),
    ("sign-out", "SIGN-OUT"),
];

/// Every route, for the preflight.
const ROUTES: &[&str] = &[
    "sign-in", "sign-up", "confirm", "resend", "forgot", "reset", "mfa", "mfa-setup", "refresh", "sign-out", "hosted/finish", "capabilities",
    "hosted/start", "hosted/callback",
];

/// The members a page may send. The others are the routes' to fill: the refresh token comes from the
/// cookie, the hosted continuation from its cookie and the callback's query.
const PAGE_MEMBERS: &[&str] =
    &["USERNAME", "PASSWORD", "ONE-TIME-CODE", "SESSION", "CHOICE", "EMAIL", "PHONE", "GIVEN-NAME", "FAMILY-NAME", "CAPTCHA-TOKEN", "ACCESS-TOKEN"];

/// One request, as the routes need it.
#[derive(Debug, Clone, Default)]
pub struct AuthRequest {
    /// `GET`, `POST` or `OPTIONS`.
    pub method: String,
    /// The path after `/auth/` (`sign-in`, `hosted/start`).
    pub route: String,
    /// The query string, without `?`.
    pub query: String,
    /// Header names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The caller's address, for the rate limits.
    pub peer_ip: String,
    /// The `Host` the request was sent to, for the hosted callback's address.
    pub host: String,
    /// Whether the request came over TLS.
    pub secure: bool,
}

impl AuthRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("cookie")).flat_map(|(_, v)| http::parse_cookies(v)).find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

/// One answer. `headers` may hold several `set-cookie` entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl AuthResponse {
    fn new(status: u16, content_type: &str, body: impl Into<String>) -> AuthResponse {
        AuthResponse { status, headers: vec![("content-type".into(), content_type.into())], body: body.into() }
    }

    fn json(status: u16, v: &Value) -> AuthResponse {
        AuthResponse::new(status, "application/json", v.to_string())
    }

    fn text(status: u16, body: &str) -> AuthResponse {
        AuthResponse::new(status, "text/plain; charset=utf-8", body)
    }

    fn redirect(location: &str) -> AuthResponse {
        let mut r = AuthResponse::text(302, "");
        r.headers.push(("location".into(), location.into()));
        r
    }

    fn cookie(mut self, c: SetCookie) -> AuthResponse {
        self.headers.push(("set-cookie".into(), c.header()));
        self
    }

    /// The values of `name`, in order.
    pub fn header_values(&self, name: &str) -> Vec<&str> {
        self.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).collect()
    }
}

/// Runs a generated `AUTH-*` program: its name, the `AUTH-REQUEST` JSON, the caller. Answers the
/// `AUTH-RESULT` JSON.
pub type Runner = dyn Fn(&str, &str, &OpContext) -> Result<String, CallError> + Send + Sync;

/// The service as the `COBOL::"AUTH-OP"` hook of one call: the caller's address travels with it.
struct Hook {
    service: Arc<IdentityService>,
    ctx: OpContext,
}

impl cobolt_runtime::interpreter::remote::IdentityHook for Hook {
    fn run(&self, operation: &str, request_json: &str) -> Result<String, CallError> {
        self.service.run(operation, request_json, &self.ctx)
    }
}

/// The runner the backend uses: the program from `backend/generated/identity/`, run like any program,
/// with the identity service as its `AUTH-OP` hook.
pub fn cobol_runner(backend: Arc<Backend>, service: Arc<IdentityService>, file_lock_wait: Duration) -> Box<Runner> {
    Box::new(move |program: &str, request: &str, ctx: &OpContext| {
        let prog = backend.identity_program(program).ok_or_else(|| {
            CallError::new(ErrorClass::NotFound, cobolt_wire::error::codes::UNKNOWN_PROGRAM, format!("{program} is not generated: build the project"))
        })?;
        let mut opts = crate::exec::ExecOptions::plain(Arc::new(std::sync::atomic::AtomicBool::new(false)), file_lock_wait);
        opts.identity = Some(Arc::new(Hook { service: service.clone(), ctx: ctx.clone() }));
        let out = crate::exec::execute(prog, &[request.to_string()], opts)?;
        out.returning.ok_or_else(|| CallError::new(ErrorClass::Server, cobolt_wire::error::codes::INTERNAL, format!("{program} returned nothing")))
    })
}

pub struct AuthRoutes {
    service: Arc<IdentityService>,
    run: Box<Runner>,
    origins: Vec<String>,
    /// The hosted returns waiting to be finished: handle → (when, the answer without its refresh token).
    handles: Mutex<HashMap<String, (Instant, String)>>,
    handle_age: Duration,
}

/// The status a failed call answers with; the body says no more than the error's message.
fn status_of(e: &CallError) -> u16 {
    match e.class {
        ErrorClass::Validation => 400,
        ErrorClass::NotFound => 404,
        ErrorClass::Limit => 429,
        ErrorClass::Server | ErrorClass::Transport => 503,
        _ => 500,
    }
}

fn error(e: &CallError) -> AuthResponse {
    AuthResponse::json(status_of(e), &json!({"error": e.message, "code": e.code}))
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").trim().to_string()
}

/// `url` with the hosted return's two parameters, before any fragment.
fn with_return_params(url: &str, handle: &str, page_state: &str) -> String {
    let (base, fragment) = match url.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (url, None),
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    let mut out = format!("{base}{sep}pr-code={}&pr-state={}", percent(handle), percent(page_state));
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

impl AuthRoutes {
    pub fn new(service: Arc<IdentityService>, run: Box<Runner>, origins: Vec<String>) -> AuthRoutes {
        AuthRoutes { service, run, origins, handles: Mutex::new(HashMap::new()), handle_age: HANDLE_AGE }
    }

    /// For the tests: a handle that lives `age` instead of a minute.
    pub fn with_handle_age(mut self, age: Duration) -> AuthRoutes {
        self.handle_age = age;
        self
    }

    fn origin_allowed(&self, origin: &str) -> bool {
        origin != "null" && self.origins.iter().any(|a| a.eq_ignore_ascii_case(origin))
    }

    /// Answer one request. Every answer carries `Cache-Control: no-store` and, for an allowed
    /// origin, the CORS headers for that origin with credentials.
    pub fn handle(&self, req: &AuthRequest) -> AuthResponse {
        let route = req.route.trim_matches('/');
        let mut res = match req.method.as_str() {
            "OPTIONS" if ROUTES.contains(&route) => self.preflight(req),
            "GET" => match route {
                "capabilities" => self.run_operation("CAPABILITIES", Map::new(), req, false),
                "hosted/start" => self.hosted_start(req),
                "hosted/callback" => self.hosted_callback(req),
                _ => AuthResponse::text(404, "not found"),
            },
            "POST" if route == "hosted/finish" => self.guard(req).unwrap_or_else(|native| self.hosted_finish(req, native)),
            "POST" => match OPERATION_ROUTES.iter().find(|(r, _)| *r == route) {
                Some((_, op)) => self.guard(req).unwrap_or_else(|native| self.operation(op, req, native)),
                None => AuthResponse::text(404, "not found"),
            },
            _ => AuthResponse::text(404, "not found"),
        };
        for (k, v) in http::cors_headers(req.header("origin").filter(|o| self.origin_allowed(o)), &self.origins) {
            res.headers.push((k.to_string(), v));
        }
        res.headers.push(("cache-control".into(), "no-store".into()));
        res
    }

    fn preflight(&self, req: &AuthRequest) -> AuthResponse {
        match req.header("origin") {
            Some(o) if self.origin_allowed(o) => {
                let mut r = AuthResponse::text(204, "");
                r.headers.extend(http::PREFLIGHT_HEADERS.iter().map(|(k, v)| (k.to_string(), v.to_string())));
                r
            }
            _ => AuthResponse::text(403, "origin not allowed"),
        }
    }

    /// The checks of every POST (D5). `Err(native)` lets the request through, saying whether it is the
    /// mobile shell's; `Ok` is the refusal.
    fn guard(&self, req: &AuthRequest) -> Result<AuthResponse, bool> {
        let json = req.header("content-type").is_some_and(|c| c.split(';').next().is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json")));
        if !json || req.header("x-pr-identity").map(str::trim) != Some("1") {
            return Ok(AuthResponse::text(403, "an identity request needs Content-Type: application/json and X-PR-Identity: 1"));
        }
        match req.header("origin") {
            Some(o) if self.origin_allowed(o) => Err(false),
            Some(_) => Ok(AuthResponse::text(403, "origin not allowed")),
            None if req.header("x-pr-client").map(str::trim) == Some("native") => Err(true),
            None => Ok(AuthResponse::text(403, "origin not allowed")),
        }
    }

    /// The page's JSON object: only the members a page may send, each a string.
    fn page_request(&self, req: &AuthRequest, native: bool) -> Result<Map<String, Value>, AuthResponse> {
        let v: Value = if req.body.iter().all(u8::is_ascii_whitespace) {
            json!({})
        } else {
            serde_json::from_slice(&req.body).map_err(|_| AuthResponse::text(400, "the body is not JSON"))?
        };
        let Value::Object(given) = v else { return Err(AuthResponse::text(400, "the body is not a JSON object")) };
        let mut out = Map::new();
        for (k, v) in given {
            let key = k.to_ascii_uppercase();
            let allowed = PAGE_MEMBERS.contains(&key.as_str()) || (native && key == "REFRESH-TOKEN");
            if !allowed {
                return Err(AuthResponse::text(400, &format!("{k} is not a member a page sends")));
            }
            let Value::String(s) = v else { return Err(AuthResponse::text(400, &format!("{k} must be text"))) };
            out.insert(key, Value::String(s));
        }
        Ok(out)
    }

    /// The refresh cookie's name and token, when it opens.
    fn refresh_cookie(&self, req: &AuthRequest) -> Option<(String, String)> {
        let v = self.service.sealer().open_json("refresh", &req.cookie(REFRESH_COOKIE)?, REFRESH_AGE)?;
        Some((text(&v, "u"), text(&v, "r")))
    }

    fn operation(&self, op: &str, req: &AuthRequest, native: bool) -> AuthResponse {
        let mut members = match self.page_request(req, native) {
            Ok(m) => m,
            Err(r) => return r,
        };
        if !native && matches!(op, "REFRESH" | "SIGN-OUT") {
            if let Some((user, token)) = self.refresh_cookie(req) {
                members.insert("USERNAME".into(), json!(user));
                members.insert("REFRESH-TOKEN".into(), json!(token));
            }
        }
        let mut res = self.run_operation(op, members, req, native);
        if !native && op == "SIGN-OUT" {
            res = res.cookie(SetCookie::delete(REFRESH_COOKIE, SameSite::Strict));
        }
        res
    }

    /// Run the operation's program and deliver its answer: on the web the refresh token goes into the
    /// cookie and out of the body; a refresh that failed removes the cookie.
    fn run_operation(&self, op: &str, members: Map<String, Value>, req: &AuthRequest, native: bool) -> AuthResponse {
        let mut request = Map::new();
        for k in REQUEST_MEMBERS {
            request.insert(k.to_string(), members.get(*k).cloned().unwrap_or_else(|| json!("")));
        }
        let ctx = OpContext { ip: req.peer_ip.clone() };
        let answer = match (self.run)(&format!("AUTH-{op}"), &Value::Object(request).to_string(), &ctx) {
            Ok(a) => a,
            Err(e) => return error(&e),
        };
        let Ok(Value::Object(mut result)) = serde_json::from_str::<Value>(&answer) else {
            return AuthResponse::text(500, "the identity program's answer is not JSON");
        };
        let mut cookies = Vec::new();
        if !native {
            let token = result.get("REFRESH-TOKEN").map(|v| text(&json!({"t": v}), "t")).unwrap_or_default();
            if !token.is_empty() {
                let user = result.get("USERNAME").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                let sealed = self.service.sealer().seal_json("refresh", &json!({"u": user, "r": token}));
                match SetCookie::host(REFRESH_COOKIE, &sealed, SameSite::Strict, Some(REFRESH_AGE.as_secs())) {
                    Ok(c) => cookies.push(c),
                    Err(e) => return AuthResponse::text(500, &e),
                }
            } else if op == "REFRESH" && text(&Value::Object(result.clone()), "OUTCOME") != ops::outcome::OK {
                cookies.push(SetCookie::delete(REFRESH_COOKIE, SameSite::Strict));
            }
            result.insert("REFRESH-TOKEN".into(), json!(""));
        }
        let mut res = AuthResponse::json(200, &Value::Object(result));
        for c in cookies {
            res = res.cookie(c);
        }
        res
    }

    /// `GET /auth/hosted/start?return=<page>&state=<the page's state>`: the provider's page, with this
    /// sign-in's continuation in its own cookie.
    fn hosted_start(&self, req: &AuthRequest) -> AuthResponse {
        let back = http::query_param(&req.query, "return").unwrap_or_default();
        let page_state = http::query_param(&req.query, "state").unwrap_or_default();
        if req.host.is_empty() || !req.host.chars().all(|c| c.is_ascii_alphanumeric() || "-.:[]".contains(c)) {
            return AuthResponse::text(400, "the request has no usable Host");
        }
        let scheme = if req.secure { "https" } else { "http" };
        let mut m = Map::new();
        m.insert("REDIRECT-URI".into(), json!(format!("{scheme}://{}/auth/hosted/callback", req.host)));
        m.insert("RETURN-URI".into(), json!(back));
        m.insert("PAGE-STATE".into(), json!(page_state));
        let res = self.run_operation("START-HOSTED", m, req, true);
        if res.status != 200 {
            return res;
        }
        let v: Value = serde_json::from_str(&res.body).unwrap_or_default();
        if text(&v, "OUTCOME") != ops::outcome::OK {
            return AuthResponse::text(400, &text(&v, "OUTCOME-TEXT"));
        }
        match SetCookie::host(HOSTED_COOKIE, &text(&v, "HOSTED"), SameSite::Lax, Some(HOSTED_AGE.as_secs())) {
            Ok(c) => AuthResponse::redirect(&text(&v, "URL")).cookie(c),
            Err(e) => AuthResponse::text(500, &e),
        }
    }

    /// `GET /auth/hosted/callback?code=…&state=…`: the provider's return. The state must be the one
    /// this browser's cookie was sealed with; the tokens wait under a single-use handle and the browser
    /// goes back to the page with the handle and the page's own state.
    fn hosted_callback(&self, req: &AuthRequest) -> AuthResponse {
        let failed = || AuthResponse::text(400, "The sign-in did not complete. Go back to the application and sign in again.").cookie(SetCookie::delete(HOSTED_COOKIE, SameSite::Lax));
        let Some(hosted) = req.cookie(HOSTED_COOKIE) else { return failed() };
        let code = http::query_param(&req.query, "code").unwrap_or_default();
        if code.is_empty() || http::query_param(&req.query, "error").is_some() {
            return failed();
        }
        let mut m = Map::new();
        m.insert("HOSTED".into(), json!(hosted));
        m.insert("STATE".into(), json!(http::query_param(&req.query, "state").unwrap_or_default()));
        m.insert("ONE-TIME-CODE".into(), json!(code));
        // The answer is read here, not sent: `native` keeps the refresh token in it for the cookie below.
        let res = self.run_operation("FINISH-HOSTED", m, req, true);
        let v: Value = serde_json::from_str(&res.body).unwrap_or_default();
        if res.status != 200 || text(&v, "OUTCOME") != ops::outcome::OK {
            return failed();
        }
        let back = text(&v, "RETURN-URI");
        let page_state = text(&v, "PAGE-STATE");
        let mut page = v.as_object().cloned().unwrap_or_default();
        for k in ["REFRESH-TOKEN", "RETURN-URI", "PAGE-STATE", "HOSTED", "STATE", "URL"] {
            page.insert(k.into(), json!(""));
        }
        let handle = random_token(32);
        if let Ok(mut h) = self.handles.lock() {
            let age = self.handle_age;
            h.retain(|_, (at, _)| at.elapsed() <= age);
            if h.len() >= MAX_HANDLES {
                return failed();
            }
            h.insert(handle.clone(), (Instant::now(), Value::Object(page).to_string()));
        }
        let mut out = AuthResponse::redirect(&with_return_params(&back, &handle, &page_state)).cookie(SetCookie::delete(HOSTED_COOKIE, SameSite::Lax));
        let token = text(&v, "REFRESH-TOKEN");
        if !token.is_empty() {
            let sealed = self.service.sealer().seal_json("refresh", &json!({"u": text(&v, "USERNAME"), "r": token}));
            if let Ok(c) = SetCookie::host(REFRESH_COOKIE, &sealed, SameSite::Strict, Some(REFRESH_AGE.as_secs())) {
                out = out.cookie(c);
            }
        }
        out
    }

    /// `POST /auth/hosted/finish {"HANDLE": …}`: the answer the handle holds, once. An unknown or old
    /// handle (another instance, a minute gone) falls back to the refresh cookie.
    fn hosted_finish(&self, req: &AuthRequest, native: bool) -> AuthResponse {
        let handle = serde_json::from_slice::<Value>(&req.body).ok().map(|v| text(&v, "HANDLE")).unwrap_or_default();
        let held = self.handles.lock().ok().and_then(|mut h| h.remove(&handle));
        match held {
            Some((at, answer)) if !handle.is_empty() && at.elapsed() <= self.handle_age => AuthResponse::new(200, "application/json", answer),
            _ if native => AuthResponse::text(400, "a native client finishes a hosted sign-in through its deep link"),
            _ => {
                let mut members = Map::new();
                if let Some((user, token)) = self.refresh_cookie(req) {
                    members.insert("USERNAME".into(), json!(user));
                    members.insert("REFRESH-TOKEN".into(), json!(token));
                }
                self.run_operation("REFRESH", members, req, false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_return_parameters_go_before_the_fragment() {
        assert_eq!(with_return_params("https://app.example/", "h1", "s 1"), "https://app.example/?pr-code=h1&pr-state=s%201");
        assert_eq!(with_return_params("https://app.example/?a=1#top", "h", "s"), "https://app.example/?a=1&pr-code=h&pr-state=s#top");
    }
}
