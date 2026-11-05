// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend's configuration (`backend/backend.toml`), WASM-003 R5, R9-R17.
//!
//! Everything a deployment decides lives here and nowhere in a form, a
//! copybook or a frontend file: the address, the certificate, the identity
//! provider, the origins that may connect, the limits. Secrets are not written
//! in this file by value when they can be avoided: a value `env:NAME` is read
//! from the environment and `file:/path` from a file, at start-up, and never
//! logged.
//!
//! The reader is strict (`deny_unknown_fields`): a misspelt security setting
//! must stop the start, not be ignored. Every relaxation is a key whose name
//! says `unsafe_` (WASM-013 R11); a **release** run refuses a configuration that
//! sets one.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A value that must never be printed: a secret given in the configuration (or
/// read from the environment or a file). Its `Debug` says nothing about it, so
/// a configuration that is logged, dumped in a panic message or put in an error
/// cannot leak it. The text is reached only through [`Secret::expose`].
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(text: impl Into<String>) -> Self {
        Secret(text.into())
    }

    /// The secret itself. Call it where the secret is used, never where it is shown.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Whether the backend runs for development or for real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// `rcrun backend run`, the IDE's Start: loopback defaults, `unsafe_*` allowed with a warning.
    Development,
    /// The built binary: any `unsafe_*` setting stops the start.
    Release,
}

/// Which identity provider's token conventions to apply (WASM-009 R8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Plain OIDC: `iss`, `aud`, `exp`; the audience in `aud`.
    #[default]
    Generic,
    /// Amazon Cognito: an access token has `token_use` = `access` and the app client in `client_id`, no `aud`.
    Cognito,
    /// Keycloak: an access token has `typ` = `Bearer`; `aud` carries the API's audience; roles in `realm_access.roles`.
    Keycloak,
    /// Microsoft Entra: `aud` is the API's application id; an access token carries `scp` or `roles`.
    Entra,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerSection {
    /// `host:port` to listen on. Default: loopback.
    pub listen: Option<String>,
    /// A second, plain-HTTP listener for the health probe only (loopback by default).
    pub health_listen: Option<String>,
    /// The `Origin` values a WebSocket may connect from (WASM-003 R14).
    pub allowed_origins: Vec<String>,
    /// Accept a connection with no `Origin` header (a non-browser client). Default false.
    pub allow_missing_origin: bool,
    /// A name for logs and the `hello`.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TlsSection {
    /// PEM certificate chain, leaf first.
    pub cert_chain: Option<PathBuf>,
    /// The private key, PEM, PKCS#8 (the "PRIVATE" kind of PEM block, not "RSA PRIVATE").
    pub private_key: Option<PathBuf>,
    /// Alternatively a PKCS#12 identity, with its password as `env:NAME` / `file:PATH`.
    pub pkcs12: Option<PathBuf>,
    pub pkcs12_password: Option<Secret>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AuthSection {
    pub profile: Profile,
    /// The issuer(s) whose tokens are accepted (`iss` must equal one).
    pub issuer: Vec<String>,
    /// Where the issuer's keys are. Default: `<first issuer>/.well-known/jwks.json`.
    pub jwks_uri: Option<String>,
    /// Acceptable `aud` values.
    pub audiences: Vec<String>,
    /// Acceptable app-client ids (Cognito `client_id`, Entra/Keycloak `azp`).
    pub client_ids: Vec<String>,
    /// Signing algorithms accepted. Never `none`, never an HMAC (that is a shared secret).
    pub algorithms: Vec<String>,
    /// Clock skew tolerated on `exp`/`nbf`, seconds. At most 300.
    pub leeway_seconds: Option<u64>,
    /// How long fetched keys are trusted, seconds.
    pub jwks_ttl_seconds: Option<u64>,
    /// The least time between two key fetches, whatever the cause, seconds.
    pub jwks_min_refresh_seconds: Option<u64>,
    /// Claims that hold the caller's roles/groups, as dotted paths (`realm_access.roles`).
    pub roles_claims: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LimitsSection {
    /// The largest WebSocket frame/message accepted, bytes.
    pub max_frame_bytes: Option<usize>,
    /// A call that runs longer is cancelled, milliseconds.
    pub call_timeout_ms: Option<u64>,
    /// Calls per second per connection, with this burst.
    pub connection_rate_per_sec: Option<u32>,
    pub connection_burst: Option<u32>,
    /// Calls per second per user (token subject), with this burst.
    pub user_rate_per_sec: Option<u32>,
    pub user_burst: Option<u32>,
    pub max_calls_per_connection: Option<usize>,
    pub max_calls_total: Option<usize>,
    pub max_connections: Option<usize>,
    pub max_subscriptions_per_connection: Option<usize>,
    /// A connection that sends nothing (not even a pong) for this long is dropped, seconds.
    pub idle_timeout_seconds: Option<u64>,
    /// Ping interval, seconds.
    pub ping_interval_seconds: Option<u64>,
    /// The TLS + HTTP head must complete within this, milliseconds.
    pub handshake_timeout_ms: Option<u64>,
    /// How long an idempotency key keeps its stored answer, seconds.
    pub idempotency_ttl_seconds: Option<u64>,
    /// How long a call waits for a file another call is using, milliseconds
    /// (WASM-003 R19); after that its OPEN answers file status 93.
    pub file_lock_wait_ms: Option<u64>,
    /// A `long_running` call may run this long, milliseconds (WASM-014 R13).
    pub long_call_timeout_ms: Option<u64>,
    /// After a cancel request, a program that has not reached a safe point is
    /// stopped at its next statement once this much time has passed, milliseconds.
    pub cancel_grace_ms: Option<u64>,
    /// Stream items waiting for a slow subscriber before it is ended (WASM-014 R14).
    pub event_queue: Option<usize>,
    /// The most idempotency results kept at once (WASM-014 R10).
    pub idempotency_max_entries: Option<usize>,
}

/// Where the idempotency results and the sessions are kept (WASM-014 T3, Q5): never in the frontend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdempotencyStore {
    /// In the backend's memory: a restart of the backend forgets them.
    #[default]
    Memory,
    /// Also in a file the backend writes before it answers: a restart keeps them.
    Server,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ContractSection {
    /// `memory` (the default) or `server`.
    pub idempotency_store: IdempotencyStore,
    /// A session left untouched this long ends, with what it holds, seconds. Default 900.
    pub session_idle_seconds: Option<u64>,
    /// Where `server` mode keeps its file. Default `data/contract/idempotency.redb`.
    pub idempotency_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ContractResolved {
    pub store: IdempotencyStore,
    pub session_idle: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FilesSection {
    /// Where uploads are kept (the backend names the files; the browser never does).
    pub dir: Option<PathBuf>,
    /// Upload limit, bytes.
    pub max_upload_bytes: Option<u64>,
    /// The upload slots and the types each takes (WASM-014 R15).
    pub slots: Vec<UploadSlot>,
    /// How long a download address lives, seconds.
    pub download_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadSlot {
    pub name: String,
    /// Allowed MIME types (exact, `type/*`, or `*/*` for any).
    pub mime: Vec<String>,
    /// Role required to upload (any of); empty: any authenticated caller.
    #[serde(default)]
    pub roles: Vec<String>,
    /// The file name extensions allowed, without the dot, any case; empty: any
    /// (a web form's FileDropZone's or Viewer's `AllowedExtensions`, WASM-005 T12).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// The largest file the slot takes, bytes; `files.max_upload_bytes` still
    /// bounds every slot.
    #[serde(default)]
    pub max_bytes: Option<u64>,
}

/// The identity backend (WASM-009): which provider the generated public programs talk to.
///
/// ```toml
/// [identity]
/// provider = "cognito"                 # cognito | keycloak | entra | entra-external
/// flow = "auto"                        # auto | in-app | hosted | native (entra-external only)
/// origins = ["https://app.example.com"]
/// cookie_key = "env:PR_COOKIE_KEY"     # 32 bytes, base64 or hex; never a literal in release
/// [identity.settings]
/// region = "eu-west-1"
/// user_pool_id = "eu-west-1_AbCdEf123"
/// client_id = "1example23456789"
/// client_secret = "env:COGNITO_CLIENT_SECRET"
/// [identity.recaptcha]
/// site_key = "6Lc…"                    # public: Build copies it into the page
/// secret = "env:RECAPTCHA_SECRET"
/// ```
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySection {
    pub provider: String,
    /// `auto` (the provider's own flow, the default), `in-app`, `hosted`, or `native` (WASM-009 R8b).
    pub flow: Option<String>,
    /// Provider settings: region/pool/realm/tenant, client id, client secret (`env:`), and so on
    /// ([`provider_settings`] lists them per provider).
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, String>,
    pub recaptcha: Option<RecaptchaSection>,
    /// Sign-in/sign-up/reset attempts allowed per source address, and per account, per minute.
    pub attempts_per_minute: Option<u32>,
    /// The page origins the `/auth/*` routes answer (CORS with credentials, D5). Default: `server.allowed_origins`.
    #[serde(default)]
    pub origins: Vec<String>,
    /// The key that seals the refresh cookie and the hosted-flow cookie (D7): `env:NAME` or `file:PATH`.
    pub cookie_key: Option<Secret>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecaptchaSection {
    /// The public site key. Build copies it into the page (`BundleOptions.recaptcha_site_key`) and
    /// `/auth/capabilities` serves it, so one setting turns both halves on (D9).
    pub site_key: Option<String>,
    /// The secret key: `env:NAME` or `file:PATH`; never in a form.
    pub secret: Secret,
    /// Refuse a score below this (default 0.5).
    pub min_score: Option<f64>,
    /// The hostnames the token may have been issued for.
    #[serde(default)]
    pub hostnames: Vec<String>,
    /// Override the verification endpoint (tests; default Google's).
    pub endpoint: Option<String>,
}

// The providers' vocabulary lives in `identity_settings` (the `loader` level), so the IDE's Identity
// page reads the same table the backend checks.
pub use crate::identity_settings::{is_secret_reference, provider_settings, resolve_secret, IdentityFlow, Provider};

impl Provider {
    /// The token conventions of its access tokens.
    pub fn profile(self) -> Profile {
        match self {
            Provider::Cognito => Profile::Cognito,
            Provider::Keycloak => Profile::Keycloak,
            Provider::Entra | Provider::EntraExternal => Profile::Entra,
        }
    }
}

/// `[identity]` with its rules checked and its secrets read.
#[derive(Debug, Clone)]
pub struct ResolvedIdentity {
    pub provider: Provider,
    pub flow: IdentityFlow,
    /// The provider's settings that are not secrets, by name.
    pub settings: std::collections::BTreeMap<String, String>,
    pub client_id: String,
    pub client_secret: Option<Secret>,
    pub origins: Vec<String>,
    /// 32 bytes.
    pub cookie_key: Secret,
    pub recaptcha: Option<ResolvedRecaptcha>,
    pub attempts_per_minute: u32,
}

#[derive(Debug, Clone)]
pub struct ResolvedRecaptcha {
    pub site_key: Option<String>,
    pub secret: Secret,
    pub min_score: f64,
    pub hostnames: Vec<String>,
    pub endpoint: Option<String>,
}

impl ResolvedIdentity {
    fn setting(&self, name: &str) -> &str {
        self.settings.get(name).map(String::as_str).unwrap_or("")
    }

    /// The OpenID authority: the issuer for Cognito and Keycloak, the tenant's base for Entra.
    pub fn authority(&self) -> String {
        match self.provider {
            Provider::Cognito => format!("https://cognito-idp.{}.amazonaws.com/{}", self.setting("region"), self.setting("user_pool_id")),
            Provider::Keycloak => self.setting("issuer").trim_end_matches('/').to_owned(),
            Provider::Entra => match self.settings.get("authority") {
                Some(a) => a.trim_end_matches('/').to_owned(),
                None => format!("https://login.microsoftonline.com/{}", self.setting("tenant")),
            },
            Provider::EntraExternal => match self.settings.get("authority") {
                Some(a) => a.trim_end_matches('/').to_owned(),
                None => {
                    let s = self.setting("tenant_subdomain");
                    format!("https://{s}.ciamlogin.com/{s}.onmicrosoft.com")
                }
            },
        }
    }

    /// Where the provider's OpenID metadata is.
    pub fn discovery_uri(&self) -> String {
        match self.provider {
            Provider::Cognito | Provider::Keycloak => format!("{}/.well-known/openid-configuration", self.authority()),
            Provider::Entra | Provider::EntraExternal => format!("{}/v2.0/.well-known/openid-configuration", self.authority()),
        }
    }

    /// Is `issuer`, as the metadata reports it, this provider's? Cognito and Keycloak name it
    /// exactly; Entra names the tenant by its id, which the settings may give by domain, so the
    /// issuer must be on Microsoft's sign-in host for the cloud the authority names.
    pub fn issuer_matches(&self, issuer: &str) -> bool {
        let issuer = issuer.trim_end_matches('/');
        match self.provider {
            Provider::Cognito | Provider::Keycloak => issuer == self.authority(),
            Provider::Entra => {
                let host = |u: &str| u.strip_prefix("https://").and_then(|r| r.split('/').next()).map(str::to_ascii_lowercase);
                issuer.ends_with("/v2.0") && host(issuer).is_some() && host(issuer) == host(&self.authority())
            }
            Provider::EntraExternal => {
                let host = issuer.strip_prefix("https://").and_then(|r| r.split('/').next()).unwrap_or("").to_ascii_lowercase();
                issuer.ends_with("/v2.0") && (host.ends_with(".ciamlogin.com") || self.settings.contains_key("authority"))
            }
        }
    }
}

/// The 32-byte sealing key of `cookie_key` (D7): base64 (standard or URL-safe, padded or not) or 64 hex digits.
pub fn cookie_key_bytes(text: &str) -> Result<[u8; 32], String> {
    use base64::Engine;
    let t = text.trim();
    let decoded = if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        (0..32).map(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())).collect::<Result<Vec<u8>, String>>()
    } else {
        let e = base64::engine::general_purpose::STANDARD.decode(t).or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(t));
        e.or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(t)).or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(t)).map_err(|_| "not base64 or hex".to_string())
    }?;
    decoded.try_into().map_err(|v: Vec<u8>| format!("{} bytes, not 32", v.len()))
}

/// An origin as CORS compares it: `https://host[:port]`, or `http://` on loopback; no path.
fn origin_ok(o: &str) -> bool {
    let rest = o.strip_prefix("https://").or_else(|| o.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    !rest.is_empty() && !rest.contains('/') && !rest.contains('*') && (o.starts_with("https://") || crate::jwks::address_allowed(o))
}

/// A named configuration of a web form's control (WASM-005 T11, operator
/// 2026-10-24, point 1): the values a control's `BackendConfig` names — its
/// secret-bearing properties (`AuthToken`, `AgentAPIKey`, `EmbeddingAPIKey`,
/// `ApiKey`) and the ones a deployment overrides (`BaseURL`…), by the
/// control's own property names. Each value may be `env:NAME` or `file:/path`,
/// like the file's other secrets. SQL connections keep their own catalogue.
///
/// ```toml
/// [[configuration]]
/// name = "CRM"
/// BaseURL = "https://crm.example.com/api"
/// AuthToken = "env:CRM_TOKEN"
/// ```
#[derive(Clone, Deserialize)]
pub struct ConfigurationEntry {
    pub name: String,
    #[serde(flatten)]
    pub properties: std::collections::BTreeMap<String, String>,
}

// Printing a configuration prints its property names, never a value.
impl std::fmt::Debug for ConfigurationEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigurationEntry")
            .field("name", &self.name)
            .field("properties", &self.properties.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// The `[[configuration]]` entries with their values read, by name. Every
/// value is a [`Secret`]: which of a control's properties hold one is the
/// control's business, so none of them is ever printed.
#[derive(Debug, Clone, Default)]
pub struct Configurations(Vec<(String, Vec<(String, Secret)>)>);

impl Configurations {
    /// The values of the configuration called `name` (any case).
    pub fn get(&self, name: &str) -> Option<&[(String, Secret)]> {
        self.0.iter().find(|(n, _)| n.eq_ignore_ascii_case(name.trim())).map(|(_, v)| v.as_slice())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(n, _)| n.as_str())
    }

    /// Read the entries: a name given once, every value resolved.
    fn resolve(entries: &[ConfigurationEntry], errs: &mut Vec<String>) -> Configurations {
        let mut out: Vec<(String, Vec<(String, Secret)>)> = Vec::new();
        for e in entries {
            let name = e.name.trim();
            if name.is_empty() {
                errs.push("configuration: an entry has an empty name".to_string());
                continue;
            }
            if out.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                errs.push(format!("configuration \"{name}\" is given twice"));
                continue;
            }
            let mut values = Vec::new();
            for (k, spec) in &e.properties {
                match resolve_secret(spec) {
                    Ok(v) => values.push((k.clone(), Secret::new(v))),
                    Err(err) => errs.push(format!("configuration \"{name}\" {k}: {err}")),
                }
            }
            out.push((name.to_owned(), values));
        }
        Configurations(out)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    pub server: ServerSection,
    pub tls: TlsSection,
    pub auth: AuthSection,
    pub limits: LimitsSection,
    pub files: FilesSection,
    /// How the idempotency store and the sessions are kept (WASM-014 T3).
    pub contract: ContractSection,
    pub identity: Option<IdentitySection>,
    /// The named configurations of web forms' controls (WASM-005 T11).
    #[serde(rename = "configuration")]
    pub configurations: Vec<ConfigurationEntry>,

    // ── the relaxations: each says `unsafe`, each stops a release start ──────
    /// Accept HS256 tokens signed with this secret (a development token, WASM-003 Q5).
    pub unsafe_dev_token_secret: Option<Secret>,
    /// Accept any `Origin`.
    pub unsafe_allow_any_origin: bool,
}

/// The relaxation that served plain `ws://` on a network address — removed
/// (WASM-003 D4): R5 and Q7 allow plain `ws://` on loopback only, in every mode.
const REMOVED_PLAIN_WS: &str = "unsafe_plain_ws";

// The provider settings hold the client secret next to the region and the pool;
// printing the section prints the names of the settings and none of the values.
impl std::fmt::Debug for IdentitySection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentitySection")
            .field("provider", &self.provider)
            .field("flow", &self.flow)
            .field("settings", &self.settings.keys().collect::<Vec<_>>())
            .field("recaptcha", &self.recaptcha)
            .field("attempts_per_minute", &self.attempts_per_minute)
            .field("origins", &self.origins)
            .field("cookie_key", &self.cookie_key)
            .finish()
    }
}

impl IdentitySection {
    /// Check `[identity]` and read its secrets (WASM-009 D3, D7, D13). A literal secret stops a
    /// release start, as an `unsafe_*` setting does, and warns in development.
    fn resolve(&self, mode: RunMode, default_origins: &[String], errs: &mut Vec<String>, warnings: &mut Vec<String>) -> Option<ResolvedIdentity> {
        let at = errs.len();
        let provider = Provider::parse(&self.provider);
        if provider.is_none() {
            errs.push(format!("identity.provider \"{}\" is not one of cognito, keycloak, entra, entra-external", self.provider));
        }
        let flow_word = self.flow.as_deref().unwrap_or("auto").trim().to_ascii_lowercase();
        let flow = match flow_word.as_str() {
            "auto" => provider.map(Provider::default_flow),
            "in-app" | "inapp" => Some(IdentityFlow::InApp),
            "hosted" => Some(IdentityFlow::Hosted),
            "native" => Some(IdentityFlow::Native),
            other => {
                errs.push(format!("identity.flow \"{other}\" is not one of auto, in-app, hosted, native"));
                None
            }
        };
        // A literal secret: refused in release, a warning in development.
        let mut literal = |name: &str, spec: &str, errs: &mut Vec<String>| {
            if !is_secret_reference(spec) {
                match mode {
                    RunMode::Release => errs.push(format!("{name} is written in backend.toml: give it as env:NAME or file:PATH (the file is committed; WASM-009 R9)")),
                    RunMode::Development => warnings.push(format!("{name} is written in backend.toml — for development only; a release build refuses it (use env:NAME or file:PATH)")),
                }
            }
        };
        let mut settings = std::collections::BTreeMap::new();
        let mut client_secret = None;
        if let (Some(p), Some(f)) = (provider, flow) {
            let known = provider_settings(p);
            for (k, v) in &self.settings {
                match known.iter().find(|(n, _, _)| n == k) {
                    None => errs.push(format!("identity.settings.{k} is not a {} setting (known: {})", p.word(), known.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(", "))),
                    Some((_, _, true)) => {
                        literal(&format!("identity.settings.{k}"), v, errs);
                        match resolve_secret(v) {
                            Ok(s) if !s.is_empty() => client_secret = Some(Secret::new(s)),
                            Ok(_) => errs.push(format!("identity.settings.{k} is empty")),
                            Err(e) => errs.push(format!("identity.settings.{k}: {e}")),
                        }
                    }
                    Some(_) => {
                        settings.insert(k.clone(), v.trim().to_owned());
                    }
                }
            }
            for (n, required, _) in known {
                if *required && self.settings.get(*n).is_none_or(|v| v.trim().is_empty()) {
                    errs.push(format!("identity.settings.{n} is required for provider {}", p.word()));
                }
            }
            match (p, f) {
                (Provider::Keycloak | Provider::Entra, IdentityFlow::InApp | IdentityFlow::Native) => errs.push(format!(
                    "identity.flow {} is not available for {}: its sign-in is the provider's page (hosted); see the capability table",
                    f.word(),
                    p.word()
                )),
                (Provider::Cognito, IdentityFlow::Native) => errs.push("identity.flow native is Entra External ID's API; Cognito's in-app flow is in-app".to_string()),
                (Provider::Cognito, IdentityFlow::Hosted) if !settings.contains_key("domain") => {
                    errs.push("identity.flow hosted for cognito needs identity.settings.domain (the managed login's domain)".to_string())
                }
                _ => {}
            }
            for (k, v) in &settings {
                if matches!(k.as_str(), "issuer" | "domain" | "authority" | "endpoint") && !(v.starts_with("https://") || crate::jwks::address_allowed(v)) {
                    errs.push(format!("identity.settings.{k} \"{v}\" must be an https:// address (http:// only for loopback)"));
                }
            }
        }
        // The sealing key: required, never a plaintext fallback (D7).
        let cookie_key = match &self.cookie_key {
            None => {
                errs.push("identity.cookie_key is missing: the refresh cookie is sealed with it and there is no unsealed fallback (WASM-009 D7); set it to env:NAME or file:PATH holding 32 random bytes (base64 or hex)".to_string());
                None
            }
            Some(spec) => {
                literal("identity.cookie_key", spec.expose(), errs);
                match resolve_secret(spec.expose()).and_then(|t| cookie_key_bytes(&t).map(|_| t).map_err(|e| format!("not a 32-byte key ({e})"))) {
                    Ok(t) => Some(Secret::new(t)),
                    Err(e) => {
                        errs.push(format!("identity.cookie_key: {e}"));
                        None
                    }
                }
            }
        };
        let origins = if self.origins.is_empty() { default_origins.to_vec() } else { self.origins.clone() };
        for o in &origins {
            if !origin_ok(o) {
                errs.push(format!("identity.origins: \"{o}\" is not an origin (https://host[:port], or http:// on loopback; no path, no *)"));
            }
        }
        if origins.is_empty() && mode == RunMode::Release {
            errs.push("identity.origins is empty (and so is server.allowed_origins): no page could sign in".to_string());
        }
        let recaptcha = self.recaptcha.as_ref().and_then(|r| {
            literal("identity.recaptcha.secret", r.secret.expose(), errs);
            let min_score = r.min_score.unwrap_or(0.5);
            if !(0.0..=1.0).contains(&min_score) {
                errs.push(format!("identity.recaptcha.min_score {min_score} is not between 0.0 and 1.0"));
            }
            if let Some(e) = &r.endpoint {
                if !(e.starts_with("https://") || crate::jwks::address_allowed(e)) {
                    errs.push(format!("identity.recaptcha.endpoint \"{e}\" must be an https:// address (http:// only for loopback)"));
                }
            }
            match resolve_secret(r.secret.expose()) {
                Ok(s) if !s.is_empty() => Some(ResolvedRecaptcha {
                    site_key: r.site_key.as_ref().map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()),
                    secret: Secret::new(s),
                    min_score,
                    hostnames: r.hostnames.clone(),
                    endpoint: r.endpoint.clone(),
                }),
                Ok(_) => {
                    errs.push("identity.recaptcha.secret is empty".to_string());
                    None
                }
                Err(e) => {
                    errs.push(format!("identity.recaptcha.secret: {e}"));
                    None
                }
            }
        });
        let attempts_per_minute = self.attempts_per_minute.unwrap_or(10);
        if attempts_per_minute == 0 {
            errs.push("identity.attempts_per_minute is 0: nobody could sign in".to_string());
        }
        if errs.len() > at {
            return None;
        }
        let client_id = settings.get("client_id").cloned().unwrap_or_default();
        Some(ResolvedIdentity {
            provider: provider?,
            flow: flow?,
            settings,
            client_id,
            client_secret,
            origins,
            cookie_key: cookie_key?,
            recaptcha,
            attempts_per_minute,
        })
    }
}

/// The configuration with its defaults applied and its secrets read.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    pub listen: std::net::SocketAddr,
    pub health_listen: Option<std::net::SocketAddr>,
    pub tls: Option<TlsSource>,
    pub allowed_origins: Vec<String>,
    pub allow_missing_origin: bool,
    pub allow_any_origin: bool,
    pub auth: ResolvedAuth,
    pub limits: Limits,
    pub files: FilesResolved,
    pub contract: ContractResolved,
    pub identity: Option<ResolvedIdentity>,
    /// `[auth]` is to be filled from the provider's metadata at start ([`Resolved::discover`]): the
    /// configuration has `[identity]` and no `[auth] issuer` (WASM-009 D3, C7).
    pub auth_from_identity: bool,
    /// The provider's metadata, read at start ([`Resolved::discover`]) when `[auth]` is left to
    /// `[identity]` or the adapter needs the provider's endpoints (Keycloak, Entra).
    pub discovery: Option<Discovery>,
    pub dev_token_secret: Option<Secret>,
    /// The controls' named configurations, read (WASM-005 T11).
    pub configurations: std::sync::Arc<Configurations>,
    /// Warnings to print at start (an `unsafe_*` in a development run).
    pub warnings: Vec<String>,
}

/// The TLS identity, read. Its `Debug` gives sizes, never bytes.
#[derive(Clone)]
pub enum TlsSource {
    Pem { cert_chain: Vec<u8>, key: Vec<u8> },
    Pkcs12 { der: Vec<u8>, password: Secret },
}

impl std::fmt::Debug for TlsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsSource::Pem { cert_chain, key } => write!(f, "TlsSource::Pem {{ cert_chain: {} bytes, key: {} bytes (not shown) }}", cert_chain.len(), key.len()),
            TlsSource::Pkcs12 { der, .. } => write!(f, "TlsSource::Pkcs12 {{ identity: {} bytes, password not shown }}", der.len()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedAuth {
    pub profile: Profile,
    pub issuers: Vec<String>,
    pub jwks_uri: Option<String>,
    pub audiences: Vec<String>,
    pub client_ids: Vec<String>,
    pub algorithms: Vec<jsonwebtoken::Algorithm>,
    pub leeway: u64,
    pub jwks_ttl: u64,
    pub jwks_min_refresh: u64,
    pub roles_claims: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_frame_bytes: usize,
    pub call_timeout_ms: u64,
    pub connection_rate: (u32, u32),
    pub user_rate: (u32, u32),
    pub max_calls_per_connection: usize,
    pub max_calls_total: usize,
    pub max_connections: usize,
    pub max_subscriptions_per_connection: usize,
    pub idle_timeout: u64,
    pub ping_interval: u64,
    pub handshake_timeout_ms: u64,
    pub idempotency_ttl: u64,
    pub file_lock_wait_ms: u64,
    pub long_call_timeout_ms: u64,
    pub cancel_grace_ms: u64,
    pub event_queue: usize,
    pub idempotency_max_entries: usize,
}

#[derive(Debug, Clone)]
pub struct FilesResolved {
    pub dir: PathBuf,
    pub max_upload_bytes: u64,
    pub slots: Vec<UploadSlot>,
    pub download_ttl: u64,
}

impl ServerConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        // A removed relaxation is named, not reported as an unknown field.
        if text.parse::<toml::Table>().is_ok_and(|t| t.contains_key(REMOVED_PLAIN_WS)) {
            return Err(format!(
                "backend.toml: `{REMOVED_PLAIN_WS}` was removed: plain ws:// is served on a loopback address only (WASM-003 R5); use TLS (a [tls] section), or terminate TLS in a reverse proxy in front and listen on loopback — then delete the line"
            ));
        }
        toml::from_str(text).map_err(|e| format!("backend.toml: {e}"))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_toml(&text)
    }

    /// Apply the defaults, check every rule, read the secrets. `base` resolves relative paths.
    ///
    /// The rules (all of them report together, so one start shows every fault):
    /// - a non-loopback address needs TLS (`ws://` only for loopback, WASM-003 R5, in every mode);
    /// - TLS is given as a PEM pair or a PKCS#12 identity, never half of either;
    /// - a release run refuses every `unsafe_*` setting (WASM-013 R11);
    /// - `none` and the HMAC algorithms are never accepted from a token (only the dev token uses HS256);
    /// - the leeway is bounded (at most 300 s);
    /// - without an issuer there is no way to accept a token, unless the dev token is on.
    pub fn resolve(&self, mode: RunMode, base: &Path) -> Result<Resolved, Vec<String>> {
        let mut errs: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let listen_text = self.server.listen.clone().unwrap_or_else(|| "127.0.0.1:8443".to_string());
        let listen = match listen_text.parse::<std::net::SocketAddr>() {
            Ok(a) => Some(a),
            Err(_) => {
                errs.push(format!("server.listen \"{listen_text}\" is not host:port with a numeric address"));
                None
            }
        };
        let health_listen = match &self.server.health_listen {
            Some(t) => match t.parse::<std::net::SocketAddr>() {
                Ok(a) => Some(a),
                Err(_) => {
                    errs.push(format!("server.health_listen \"{t}\" is not host:port with a numeric address"));
                    None
                }
            },
            None => None,
        };

        // TLS
        let abs = |p: &Path| if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
        let tls = match (&self.tls.cert_chain, &self.tls.private_key, &self.tls.pkcs12) {
            (Some(c), Some(k), None) => match (std::fs::read(abs(c)), std::fs::read(abs(k))) {
                (Ok(cert_chain), Ok(key)) => Some(TlsSource::Pem { cert_chain, key }),
                (Err(e), _) => {
                    errs.push(format!("tls.cert_chain {}: {e}", c.display()));
                    None
                }
                (_, Err(e)) => {
                    errs.push(format!("tls.private_key {}: {e}", k.display()));
                    None
                }
            },
            (None, None, Some(p)) => {
                let password = match &self.tls.pkcs12_password {
                    Some(spec) => Secret::new(resolve_secret(spec.expose()).map_err(|e| errs.push(format!("tls.pkcs12_password: {e}"))).unwrap_or_default()),
                    None => Secret::default(),
                };
                match std::fs::read(abs(p)) {
                    Ok(der) => Some(TlsSource::Pkcs12 { der, password }),
                    Err(e) => {
                        errs.push(format!("tls.pkcs12 {}: {e}", p.display()));
                        None
                    }
                }
            }
            (None, None, None) => None,
            _ => {
                errs.push("tls: give cert_chain AND private_key, or pkcs12 — not half of either, and not both forms".to_string());
                None
            }
        };
        if let Some(addr) = listen {
            if tls.is_none() && !addr.ip().is_loopback() {
                errs.push(format!(
                    "server.listen {addr} is not a loopback address and no TLS identity is configured: ws:// is served only on loopback (WASM-003 R5); configure [tls], or terminate TLS in front and listen on loopback"
                ));
            }
        }

        // The unsafe switches.
        let mut unsafe_set: Vec<&str> = Vec::new();
        if self.unsafe_dev_token_secret.is_some() {
            unsafe_set.push("unsafe_dev_token_secret");
        }
        if self.unsafe_allow_any_origin {
            unsafe_set.push("unsafe_allow_any_origin");
        }
        for u in &unsafe_set {
            match mode {
                RunMode::Release => errs.push(format!("{u} is set: a release backend refuses every unsafe_* setting (WASM-013 R11); remove it")),
                RunMode::Development => warnings.push(format!("{u} is on — for development only; a release build refuses it")),
            }
        }

        // Origins
        if self.server.allowed_origins.is_empty() && !self.unsafe_allow_any_origin && !self.server.allow_missing_origin && mode == RunMode::Release {
            errs.push("server.allowed_origins is empty: no browser could connect (set the origins of the frontend, or allow_missing_origin for non-browser clients)".to_string());
        }
        for o in &self.server.allowed_origins {
            if o == "*" {
                errs.push("server.allowed_origins: \"*\" is not an origin; list them, or use unsafe_allow_any_origin in development".to_string());
            }
        }

        // Authentication
        let mut algorithms = Vec::new();
        let names: Vec<String> = if self.auth.algorithms.is_empty() { vec!["RS256".into(), "ES256".into()] } else { self.auth.algorithms.clone() };
        for n in &names {
            match n.to_ascii_uppercase().as_str() {
                "RS256" => algorithms.push(jsonwebtoken::Algorithm::RS256),
                "RS384" => algorithms.push(jsonwebtoken::Algorithm::RS384),
                "RS512" => algorithms.push(jsonwebtoken::Algorithm::RS512),
                "PS256" => algorithms.push(jsonwebtoken::Algorithm::PS256),
                "PS384" => algorithms.push(jsonwebtoken::Algorithm::PS384),
                "PS512" => algorithms.push(jsonwebtoken::Algorithm::PS512),
                "ES256" => algorithms.push(jsonwebtoken::Algorithm::ES256),
                "ES384" => algorithms.push(jsonwebtoken::Algorithm::ES384),
                "EDDSA" => algorithms.push(jsonwebtoken::Algorithm::EdDSA),
                "NONE" => errs.push("auth.algorithms: \"none\" is never accepted".to_string()),
                "HS256" | "HS384" | "HS512" => errs.push(format!(
                    "auth.algorithms: {n} is a shared-secret algorithm; an identity provider signs with a key pair (the development token is the only HS256 use, via unsafe_dev_token_secret)"
                )),
                other => errs.push(format!("auth.algorithms: unknown algorithm \"{other}\"")),
            }
        }
        let leeway = self.auth.leeway_seconds.unwrap_or(60);
        if leeway > 300 {
            errs.push(format!("auth.leeway_seconds {leeway} is over 300: clock skew is bounded"));
        }
        // `[identity]` without `[auth] issuer`: the issuer and its keys come from the provider's
        // metadata at start (WASM-009 D3); an explicit `[auth]` wins.
        let auth_from_identity = self.identity.is_some() && self.auth.issuer.is_empty();
        if self.auth.issuer.is_empty() && self.unsafe_dev_token_secret.is_none() && !auth_from_identity {
            errs.push("auth.issuer is empty: with no issuer no token can be accepted, and only programs declared public would answer".to_string());
        }
        for i in &self.auth.issuer {
            if !(i.starts_with("https://") || (i.starts_with("http://") && (i.contains("//127.0.0.1") || i.contains("//localhost") || i.contains("//[::1]")))) {
                errs.push(format!("auth.issuer \"{i}\" must be an https:// URL (http:// only for loopback)"));
            }
        }
        if self.auth.audiences.is_empty() && self.auth.client_ids.is_empty() && !self.auth.issuer.is_empty() {
            errs.push("auth.audiences / auth.client_ids are both empty: a token for any other application of the same issuer would be accepted".to_string());
        }

        // Limits
        let l = &self.limits;
        let limits = Limits {
            max_frame_bytes: l.max_frame_bytes.unwrap_or(1024 * 1024),
            call_timeout_ms: l.call_timeout_ms.unwrap_or(30_000),
            connection_rate: (l.connection_rate_per_sec.unwrap_or(20), l.connection_burst.unwrap_or(40)),
            user_rate: (l.user_rate_per_sec.unwrap_or(50), l.user_burst.unwrap_or(100)),
            max_calls_per_connection: l.max_calls_per_connection.unwrap_or(8),
            max_calls_total: l.max_calls_total.unwrap_or(64),
            max_connections: l.max_connections.unwrap_or(1000),
            max_subscriptions_per_connection: l.max_subscriptions_per_connection.unwrap_or(16),
            idle_timeout: l.idle_timeout_seconds.unwrap_or(60),
            ping_interval: l.ping_interval_seconds.unwrap_or(20),
            handshake_timeout_ms: l.handshake_timeout_ms.unwrap_or(10_000),
            idempotency_ttl: l.idempotency_ttl_seconds.unwrap_or(600),
            file_lock_wait_ms: l.file_lock_wait_ms.unwrap_or(5000),
            long_call_timeout_ms: l.long_call_timeout_ms.unwrap_or(600_000),
            cancel_grace_ms: l.cancel_grace_ms.unwrap_or(10_000),
            event_queue: l.event_queue.unwrap_or(256),
            idempotency_max_entries: l.idempotency_max_entries.unwrap_or(10_000),
        };
        if limits.max_frame_bytes < 1024 {
            errs.push("limits.max_frame_bytes is under 1024: no call would fit".to_string());
        }

        let files = FilesResolved {
            dir: abs(self.files.dir.as_deref().unwrap_or(Path::new("data/uploads"))),
            max_upload_bytes: self.files.max_upload_bytes.unwrap_or(50 * 1024 * 1024),
            slots: self.files.slots.clone(),
            download_ttl: self.files.download_ttl_seconds.unwrap_or(60),
        };

        let contract = ContractResolved {
            store: self.contract.idempotency_store,
            session_idle: self.contract.session_idle_seconds.unwrap_or(900),
            path: abs(self.contract.idempotency_path.as_deref().unwrap_or(Path::new("data/contract/idempotency.redb"))),
        };

        let dev_token_secret = match &self.unsafe_dev_token_secret {
            Some(spec) => match resolve_secret(spec.expose()) {
                Ok(s) if s.len() >= 16 => Some(Secret::new(s)),
                Ok(_) => {
                    errs.push("unsafe_dev_token_secret is shorter than 16 characters".to_string());
                    None
                }
                Err(e) => {
                    errs.push(format!("unsafe_dev_token_secret: {e}"));
                    None
                }
            },
            None => None,
        };

        let configurations = Configurations::resolve(&self.configurations, &mut errs);
        let identity = self.identity.as_ref().and_then(|i| i.resolve(mode, &self.server.allowed_origins, &mut errs, &mut warnings));

        if !errs.is_empty() {
            return Err(errs);
        }
        let jwks_uri = self.auth.jwks_uri.clone().or_else(|| self.auth.issuer.first().map(|i| format!("{}/.well-known/jwks.json", i.trim_end_matches('/'))));
        // Derived from `[identity]`: the profile, and the client ids / audiences, now; the issuer
        // and the key address when the metadata is read.
        let (profile, client_ids, audiences) = match (&identity, auth_from_identity) {
            (Some(i), true) => {
                let audience = i.settings.get("audience").cloned();
                let audiences = match i.provider {
                    Provider::Entra | Provider::EntraExternal => vec![audience.unwrap_or_else(|| i.client_id.clone())],
                    _ => audience.into_iter().collect(),
                };
                (i.provider.profile(), vec![i.client_id.clone()], audiences)
            }
            _ => (self.auth.profile, self.auth.client_ids.clone(), self.auth.audiences.clone()),
        };
        Ok(Resolved {
            name: self.server.name.clone().unwrap_or_else(|| "prc-backend".to_string()),
            listen: listen.expect("checked"),
            health_listen,
            tls,
            allowed_origins: self.server.allowed_origins.clone(),
            allow_missing_origin: self.server.allow_missing_origin,
            allow_any_origin: self.unsafe_allow_any_origin,
            auth: ResolvedAuth {
                profile,
                issuers: self.auth.issuer.clone(),
                jwks_uri,
                audiences,
                client_ids,
                algorithms,
                leeway,
                jwks_ttl: self.auth.jwks_ttl_seconds.unwrap_or(3600),
                jwks_min_refresh: self.auth.jwks_min_refresh_seconds.unwrap_or(30),
                roles_claims: self.auth.roles_claims.clone(),
            },
            limits,
            files,
            contract,
            identity,
            auth_from_identity,
            discovery: None,
            dev_token_secret,
            configurations: std::sync::Arc::new(configurations),
            warnings,
        })
    }
}

/// The parts of a provider's OpenID metadata the backend uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    pub issuer: String,
    pub jwks_uri: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub revocation_endpoint: Option<String>,
    pub end_session_endpoint: Option<String>,
}

/// Read a provider's metadata document (`/.well-known/openid-configuration`) for `identity`:
/// the issuer it names must be the configured provider's, and every address in it must be
/// https (http only on loopback). The keys are wherever the metadata says, never a guessed
/// `<issuer>/.well-known/jwks.json` (Keycloak and Entra keep them elsewhere).
pub fn discover(identity: &ResolvedIdentity, fetcher: &dyn crate::jwks::JwksFetcher) -> Result<Discovery, String> {
    let uri = identity.discovery_uri();
    let doc = fetcher.fetch(&uri).map_err(|e| format!("the provider's metadata ({uri}): {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&doc).map_err(|_| format!("the provider's metadata ({uri}) is not JSON"))?;
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    let issuer = text("issuer").ok_or_else(|| format!("the provider's metadata ({uri}) has no issuer"))?;
    if !identity.issuer_matches(&issuer) {
        return Err(format!("the provider's metadata names the issuer {issuer}, which is not the configured {} ({})", identity.provider.word(), identity.authority()));
    }
    let need = |k: &str| -> Result<String, String> {
        let a = text(k).ok_or_else(|| format!("the provider's metadata has no {k}"))?;
        if crate::jwks::address_allowed(&a) {
            Ok(a)
        } else {
            Err(format!("the provider's metadata gives {k} = {a}, which is not https"))
        }
    };
    let optional = |k: &str| text(k).filter(|a| crate::jwks::address_allowed(a));
    Ok(Discovery {
        jwks_uri: need("jwks_uri")?,
        authorization_endpoint: need("authorization_endpoint")?,
        token_endpoint: need("token_endpoint")?,
        revocation_endpoint: optional("revocation_endpoint"),
        end_session_endpoint: optional("end_session_endpoint"),
        issuer: issuer.trim_end_matches('/').to_owned(),
    })
}

impl Resolved {
    /// Read the provider's metadata once, at start: it fills `[auth]` when the configuration leaves
    /// it to `[identity]` (WASM-009 D3, C7), and it gives the Keycloak and Entra adapters their
    /// endpoints. Nothing to do without `[identity]`, or for Cognito with `[auth]` given (its
    /// endpoints follow from its region and domain).
    pub fn discover(&mut self, fetcher: &dyn crate::jwks::JwksFetcher) -> Result<(), String> {
        let Some(identity) = &self.identity else { return Ok(()) };
        if !self.auth_from_identity && identity.provider == Provider::Cognito {
            return Ok(());
        }
        let d = discover(identity, fetcher)?;
        if self.auth_from_identity {
            self.auth.issuers = vec![d.issuer.clone()];
            self.auth.jwks_uri = Some(d.jwks_uri.clone());
        }
        self.discovery = Some(d);
        Ok(())
    }
}

