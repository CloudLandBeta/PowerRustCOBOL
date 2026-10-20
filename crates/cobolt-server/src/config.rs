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
    /// Allowed MIME types (exact, or `type/*`).
    pub mime: Vec<String>,
    /// Role required to upload (any of); empty: any authenticated caller.
    #[serde(default)]
    pub roles: Vec<String>,
}

/// The identity backend (WASM-009): which provider the generated public programs talk to.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySection {
    pub provider: String,
    /// `in-app` or `hosted`; default per provider (WASM-009 R8b).
    pub flow: Option<String>,
    /// Provider settings: region/pool/realm/tenant, client id, client secret (`env:`), and so on.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, String>,
    pub recaptcha: Option<RecaptchaSection>,
    /// Sign-in/sign-up/reset attempts allowed per source address per minute.
    pub attempts_per_minute: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecaptchaSection {
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    pub server: ServerSection,
    pub tls: TlsSection,
    pub auth: AuthSection,
    pub limits: LimitsSection,
    pub files: FilesSection,
    pub identity: Option<IdentitySection>,

    // ── the relaxations: each says `unsafe`, each stops a release start ──────
    /// Accept HS256 tokens signed with this secret (a development token, WASM-003 Q5).
    pub unsafe_dev_token_secret: Option<Secret>,
    /// Accept any `Origin`.
    pub unsafe_allow_any_origin: bool,
    /// Serve plain `ws://` even on a non-loopback address.
    pub unsafe_plain_ws: bool,
}

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
            .finish()
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
    pub identity: Option<IdentitySection>,
    pub dev_token_secret: Option<Secret>,
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

/// Read a secret given as `env:NAME`, `file:PATH` or a literal.
pub fn resolve_secret(spec: &str) -> Result<String, String> {
    if let Some(name) = spec.strip_prefix("env:") {
        std::env::var(name).map_err(|_| format!("the environment variable {name} is not set"))
    } else if let Some(path) = spec.strip_prefix("file:") {
        std::fs::read_to_string(path).map(|s| s.trim().to_string()).map_err(|e| format!("cannot read the secret file {path}: {e}"))
    } else {
        Ok(spec.to_string())
    }
}

impl ServerConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("backend.toml: {e}"))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_toml(&text)
    }

    /// Apply the defaults, check every rule, read the secrets. `base` resolves relative paths.
    ///
    /// The rules (all of them report together, so one start shows every fault):
    /// - a non-loopback address needs TLS (`ws://` only for loopback, WASM-003 R5), unless `unsafe_plain_ws`;
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
                if self.unsafe_plain_ws {
                    warnings.push(format!("unsafe_plain_ws: serving plain ws:// on {addr}, which is not a loopback address"));
                } else {
                    errs.push(format!(
                        "server.listen {addr} is not a loopback address and no TLS identity is configured: ws:// is served only on loopback (WASM-003 R5); configure [tls], or terminate TLS in front and listen on loopback"
                    ));
                }
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
        if self.unsafe_plain_ws {
            unsafe_set.push("unsafe_plain_ws");
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
        if self.auth.issuer.is_empty() && self.unsafe_dev_token_secret.is_none() {
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

        if !errs.is_empty() {
            return Err(errs);
        }
        let jwks_uri = self.auth.jwks_uri.clone().or_else(|| self.auth.issuer.first().map(|i| format!("{}/.well-known/jwks.json", i.trim_end_matches('/'))));
        Ok(Resolved {
            name: self.server.name.clone().unwrap_or_else(|| "prc-backend".to_string()),
            listen: listen.expect("checked"),
            health_listen,
            tls,
            allowed_origins: self.server.allowed_origins.clone(),
            allow_missing_origin: self.server.allow_missing_origin,
            allow_any_origin: self.unsafe_allow_any_origin,
            auth: ResolvedAuth {
                profile: self.auth.profile,
                issuers: self.auth.issuer.clone(),
                jwks_uri,
                audiences: self.auth.audiences.clone(),
                client_ids: self.auth.client_ids.clone(),
                algorithms,
                leeway,
                jwks_ttl: self.auth.jwks_ttl_seconds.unwrap_or(3600),
                jwks_min_refresh: self.auth.jwks_min_refresh_seconds.unwrap_or(30),
                roles_claims: self.auth.roles_claims.clone(),
            },
            limits,
            files,
            identity: self.identity.clone(),
            dev_token_secret,
            warnings,
        })
    }
}
