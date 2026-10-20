// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Bearer-token validation (WASM-003 R9-R13).
//!
//! What a token must be to be accepted, in the order it is checked:
//!
//! 1. **Shaped like a JWT** and not absurdly long.
//! 2. **Signed with an allowed algorithm** — the configured list, which can
//!    never hold `none` or an HMAC (config refuses them) — **by a key the
//!    issuer publishes**, found by `kid`. The key's own `alg`, when it names
//!    one, must equal the token's: the classic algorithm-confusion attack has
//!    nothing to bite on.
//! 3. **From a configured issuer, not expired, not yet valid** — with a
//!    bounded clock skew.
//! 4. **For this application** — `aud`, or the app-client claim.
//! 5. **An access token, not an identity token** — by the provider's own
//!    marker (`token_use`, `typ`, `scp`/`roles`), and never one that carries
//!    the signs of an ID token (`at_hash`, `nonce`).
//!
//! Then the claims the programs may read are extracted: the subject, the
//! roles, and every scalar claim flattened to text for `COBOL::"WS-CLAIM"`.
//! A token is never logged, and the module has no function that prints one.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde_json::Value;
use sha2::{Digest, Sha256};

use cobolt_wire::error::{codes, CallError, ErrorClass};

use crate::config::{Profile, ResolvedAuth};
use crate::jwks::Jwks;

/// What a validated token says about its caller.
#[derive(Debug, Clone)]
pub struct Claims {
    pub sub: String,
    pub issuer: String,
    /// Seconds since the epoch.
    pub exp: i64,
    pub nbf: Option<i64>,
    pub roles: BTreeSet<String>,
    /// Every scalar claim as text, nested ones with dotted names; arrays comma-joined.
    pub flat: BTreeMap<String, String>,
    /// Set for the development token.
    pub dev: bool,
}

impl Claims {
    /// May a caller with these roles run a program that needs one of `required`?
    pub fn has_any_role(&self, required: &[String]) -> bool {
        required.is_empty() || required.iter().any(|r| self.roles.contains(r))
    }
}

fn auth_err(code: u32, message: impl Into<String>) -> CallError {
    CallError::new(ErrorClass::Authentication, code, message)
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub struct TokenValidator {
    cfg: ResolvedAuth,
    jwks: Option<Arc<Jwks>>,
    dev_secret: Option<String>,
    /// SHA-256(token) -> claims, so a token that has been verified is not
    /// verified again on every call. Expiry is re-checked each time it is used.
    cache: Mutex<HashMap<[u8; 32], Claims>>,
}

const MAX_TOKEN_BYTES: usize = 16 * 1024;
const CACHE_LIMIT: usize = 2048;

impl TokenValidator {
    pub fn new(cfg: ResolvedAuth, jwks: Option<Arc<Jwks>>, dev_secret: Option<String>) -> Self {
        TokenValidator { cfg, jwks, dev_secret, cache: Mutex::new(HashMap::new()) }
    }

    /// Validate a token, or say why not.
    pub async fn validate(&self, token: &str) -> Result<Claims, CallError> {
        if token.is_empty() {
            return Err(auth_err(codes::TOKEN_MISSING, "no token"));
        }
        if token.len() > MAX_TOKEN_BYTES || token.matches('.').count() != 2 {
            return Err(auth_err(codes::TOKEN_INVALID, "not a JSON Web Token"));
        }
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if let Some(c) = self.cache.lock().ok().and_then(|c| c.get(&digest).cloned()) {
            self.recheck(&c)?;
            return Ok(c);
        }
        let claims = self.verify(token).await?;
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= CACHE_LIMIT {
                cache.clear();
            }
            cache.insert(digest, claims.clone());
        }
        Ok(claims)
    }

    /// Re-check, at the time of a call, that an already verified token has not
    /// expired meanwhile (WASM-003 R13).
    pub fn recheck(&self, c: &Claims) -> Result<(), CallError> {
        let t = now();
        let leeway = if c.dev { 0 } else { self.cfg.leeway as i64 };
        if c.exp + leeway < t {
            return Err(auth_err(codes::TOKEN_EXPIRED, "the token has expired"));
        }
        if let Some(nbf) = c.nbf {
            if nbf - leeway > t {
                return Err(auth_err(codes::TOKEN_INVALID, "the token is not valid yet"));
            }
        }
        Ok(())
    }

    async fn verify(&self, token: &str) -> Result<Claims, CallError> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| auth_err(codes::TOKEN_INVALID, "the token header cannot be read, or names an algorithm this backend does not know"))?;

        // The development token: HS256 with the configured secret, development only.
        if header.alg == Algorithm::HS256 || header.alg == Algorithm::HS384 || header.alg == Algorithm::HS512 {
            let Some(secret) = &self.dev_secret else {
                return Err(auth_err(codes::TOKEN_INVALID, "the token's algorithm is not accepted"));
            };
            let mut v = Validation::new(Algorithm::HS256);
            v.leeway = 0;
            v.validate_aud = false;
            v.validate_nbf = true;
            v.set_required_spec_claims(&["exp", "sub"]);
            let data = jsonwebtoken::decode::<Value>(token, &DecodingKey::from_secret(secret.as_bytes()), &v).map_err(map_jwt_error)?;
            return self.extract(&data.claims, true);
        }

        if !self.cfg.algorithms.contains(&header.alg) {
            return Err(auth_err(codes::TOKEN_INVALID, "the token's algorithm is not accepted"));
        }
        let Some(jwks) = &self.jwks else {
            return Err(auth_err(codes::TOKEN_INVALID, "no issuer is configured"));
        };
        let key = jwks.key_for(header.kid.as_deref(), header.alg).await?;
        let mut v = Validation::new(header.alg);
        v.leeway = self.cfg.leeway;
        v.validate_exp = true;
        v.validate_nbf = true;
        v.validate_aud = false; // judged below, with the provider's own rules
        v.set_issuer(&self.cfg.issuers);
        v.set_required_spec_claims(&["exp", "iss"]);
        let data = jsonwebtoken::decode::<Value>(token, &key, &v).map_err(map_jwt_error)?;
        self.check_application(&data.claims)?;
        self.check_access_token(&data.claims)?;
        self.extract(&data.claims, false)
    }

    /// The token is for this application (audience / app client).
    fn check_application(&self, c: &Value) -> Result<(), CallError> {
        let bad = |m: &str| auth_err(codes::TOKEN_INVALID, m.to_string());
        let aud: Vec<String> = match c.get("aud") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
            _ => Vec::new(),
        };
        let in_aud = aud.iter().any(|a| self.cfg.audiences.contains(a));
        let client = ["client_id", "azp", "appid"].iter().filter_map(|k| c.get(*k).and_then(|v| v.as_str())).any(|v| self.cfg.client_ids.iter().any(|id| id == v));
        let ok = match self.cfg.profile {
            // Cognito's access token has no `aud`: its application is `client_id`.
            Profile::Cognito => c.get("client_id").and_then(|v| v.as_str()).is_some_and(|v| self.cfg.client_ids.iter().any(|id| id == v)) || in_aud,
            // Entra: `aud` is the API's application id (v2) or its URI (v1).
            Profile::Entra => in_aud,
            _ => in_aud || client,
        };
        if ok {
            Ok(())
        } else {
            Err(bad("the token is not for this application"))
        }
    }

    /// An access token, not an identity token.
    fn check_access_token(&self, c: &Value) -> Result<(), CallError> {
        let bad = |m: &str| auth_err(codes::TOKEN_INVALID, m.to_string());
        // The marks of an identity token, for any provider.
        if c.get("at_hash").is_some() || c.get("nonce").is_some() {
            return Err(bad("an identity token cannot be used as an access token"));
        }
        if let Some(u) = c.get("token_use").and_then(|v| v.as_str()) {
            if u != "access" {
                return Err(bad("the token is not an access token"));
            }
        }
        match self.cfg.profile {
            Profile::Cognito => {
                if c.get("token_use").and_then(|v| v.as_str()) != Some("access") {
                    return Err(bad("the token is not an access token (token_use)"));
                }
            }
            Profile::Keycloak => {
                match c.get("typ").and_then(|v| v.as_str()) {
                    Some(t) if t.eq_ignore_ascii_case("Bearer") => {}
                    _ => return Err(bad("the token is not a Bearer access token (typ)")),
                }
            }
            Profile::Entra => {
                if c.get("scp").is_none() && c.get("roles").is_none() {
                    return Err(bad("the token carries neither scp nor roles: it is not an access token for this API"));
                }
            }
            Profile::Generic => {
                if let Some(t) = c.get("typ").and_then(|v| v.as_str()) {
                    if t.eq_ignore_ascii_case("ID") || t.eq_ignore_ascii_case("Refresh") {
                        return Err(bad("the token is not an access token (typ)"));
                    }
                }
            }
        }
        Ok(())
    }

    fn extract(&self, c: &Value, dev: bool) -> Result<Claims, CallError> {
        let sub = c.get("sub").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if sub.is_empty() && !dev {
            return Err(auth_err(codes::TOKEN_INVALID, "the token has no subject"));
        }
        let exp = c.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
        let nbf = c.get("nbf").and_then(|v| v.as_i64());
        let issuer = c.get("iss").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let default_roles: &[&str] = match self.cfg.profile {
            Profile::Cognito => &["cognito:groups"],
            Profile::Keycloak => &["realm_access.roles"],
            Profile::Entra => &["roles"],
            Profile::Generic => &["roles", "groups"],
        };
        let paths: Vec<String> = if self.cfg.roles_claims.is_empty() { default_roles.iter().map(|s| s.to_string()).collect() } else { self.cfg.roles_claims.clone() };
        let mut roles = BTreeSet::new();
        for p in &paths {
            if let Some(v) = lookup_path(c, p) {
                match v {
                    Value::Array(a) => roles.extend(a.iter().filter_map(|x| x.as_str().map(str::to_owned))),
                    Value::String(s) => roles.extend(s.split([' ', ',']).filter(|x| !x.is_empty()).map(str::to_owned)),
                    _ => {}
                }
            }
        }
        let mut flat = BTreeMap::new();
        flatten("", c, 0, &mut flat);
        Ok(Claims { sub, issuer, exp, nbf, roles, flat, dev })
    }
}

fn map_jwt_error(e: jsonwebtoken::errors::Error) -> CallError {
    use jsonwebtoken::errors::ErrorKind as K;
    match e.kind() {
        K::ExpiredSignature => auth_err(codes::TOKEN_EXPIRED, "the token has expired"),
        K::ImmatureSignature => auth_err(codes::TOKEN_INVALID, "the token is not valid yet"),
        K::InvalidSignature => auth_err(codes::TOKEN_INVALID, "the token's signature is not valid"),
        K::InvalidIssuer => auth_err(codes::TOKEN_INVALID, "the token is from an issuer this backend does not trust"),
        K::MissingRequiredClaim(c) => auth_err(codes::TOKEN_INVALID, format!("the token lacks the claim {c}")),
        K::InvalidAlgorithm | K::InvalidAlgorithmName => auth_err(codes::TOKEN_INVALID, "the token's algorithm is not accepted"),
        _ => auth_err(codes::TOKEN_INVALID, "the token is not valid"),
    }
}

/// `a.b.c` into nested objects; a key that itself contains dots is tried whole first.
fn lookup_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    if let Some(x) = v.get(path) {
        return Some(x);
    }
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn flatten(prefix: &str, v: &Value, depth: usize, out: &mut BTreeMap<String, String>) {
    match v {
        Value::Object(m) if depth < 4 => {
            for (k, x) in m {
                let name = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&name, x, depth + 1, out);
            }
        }
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(scalar_text).collect();
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), parts.join(","));
            }
        }
        other => {
            if let (Some(t), false) = (scalar_text(other), prefix.is_empty()) {
                out.insert(prefix.to_string(), t);
            }
        }
    }
}

fn scalar_text(v: &Value) -> Option<String> {
    let t = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    Some(t.chars().take(1024).collect())
}
