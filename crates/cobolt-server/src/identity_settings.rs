// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The words of `backend.toml`'s `[identity]` section (WASM-009 R8, R8b, R9): the providers, the
//! flows, each provider's settings, and what a secret reference looks like. At the `loader` level, so
//! the IDE's Identity page offers exactly what the backend's configuration accepts without linking
//! the server.

/// The identity providers (WASM-009 R8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Cognito,
    Keycloak,
    /// Microsoft Entra ID, a workforce tenant.
    Entra,
    /// Microsoft Entra External ID (an external, `ciamlogin.com` tenant).
    EntraExternal,
}

impl Provider {
    pub fn parse(word: &str) -> Option<Provider> {
        match word.trim().to_ascii_lowercase().as_str() {
            "cognito" => Some(Provider::Cognito),
            "keycloak" => Some(Provider::Keycloak),
            "entra" => Some(Provider::Entra),
            "entra-external" => Some(Provider::EntraExternal),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Provider::Cognito => "cognito",
            Provider::Keycloak => "keycloak",
            Provider::Entra => "entra",
            Provider::EntraExternal => "entra-external",
        }
    }

    /// The flow `auto` means (WASM-009 O1, the working assumption — to confirm): Cognito in-app,
    /// the others hosted.
    pub fn default_flow(self) -> IdentityFlow {
        match self {
            Provider::Cognito => IdentityFlow::InApp,
            _ => IdentityFlow::Hosted,
        }
    }
}

/// How the provider's conversation reaches the user (R8b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityFlow {
    /// The control's own screens; the backend speaks to the provider.
    InApp,
    /// The provider's page; the backend builds the address and exchanges the code.
    Hosted,
    /// Entra External ID's native authentication API: in-app screens on the External ID endpoints.
    Native,
}

impl IdentityFlow {
    pub fn word(self) -> &'static str {
        match self {
            IdentityFlow::InApp => "in-app",
            IdentityFlow::Hosted => "hosted",
            IdentityFlow::Native => "native",
        }
    }
}

/// A provider's settings: `(name, required, secret)`. A secret setting is read through
/// the configuration's `resolve_secret` and never kept with the others.
pub fn provider_settings(p: Provider) -> &'static [(&'static str, bool, bool)] {
    match p {
        Provider::Cognito => &[
            ("region", true, false),
            ("user_pool_id", true, false),
            ("client_id", true, false),
            ("client_secret", false, true),
            // The managed login's domain, for the hosted flow (`https://<prefix>.auth.<region>.amazoncognito.com`).
            ("domain", false, false),
            ("scopes", false, false),
            // Another address for the Cognito API (a local emulator, a test).
            ("endpoint", false, false),
            // "true" when the app client rotates refresh tokens (`GetTokensFromRefreshToken`, plan §3.1).
            ("refresh_rotation", false, false),
            // The name an authenticator app shows for the account; default the pool id.
            ("totp_issuer", false, false),
        ],
        Provider::Keycloak => &[("issuer", true, false), ("client_id", true, false), ("client_secret", false, true), ("scopes", false, false), ("audience", false, false)],
        Provider::Entra => &[
            ("tenant", true, false),
            ("client_id", true, false),
            ("client_secret", false, true),
            ("scopes", false, false),
            // The `aud` of the access tokens the backend accepts (the API's application id URI); default the client id.
            ("audience", false, false),
            // Another authority (a national cloud); default `https://login.microsoftonline.com/<tenant>`.
            ("authority", false, false),
        ],
        Provider::EntraExternal => &[
            ("tenant_subdomain", true, false),
            ("client_id", true, false),
            ("client_secret", false, true),
            ("scopes", false, false),
            ("audience", false, false),
            ("authority", false, false),
        ],
    }
}

/// Is `spec` a reference (`env:NAME`, `file:PATH`) rather than the secret written in the file?
pub fn is_secret_reference(spec: &str) -> bool {
    let s = spec.trim();
    (s.starts_with("env:") && s.len() > 4) || (s.starts_with("file:") && s.len() > 5)
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

/// What a Build takes from `backend.toml`'s `[identity]` (WASM-009 D9, D14).
#[derive(Default)]
pub struct BuildIdentity {
    /// `[identity.recaptcha] site_key`: public, copied into the page.
    pub site_key: Option<String>,
    /// `(setting, value)` of every secret the section names that resolves here: the provider's
    /// secret settings, `cookie_key` and `recaptcha.secret`. The Build searches its outputs for each
    /// value; one that does not resolve on this machine (an unset variable) has nothing to search for.
    pub secrets: Vec<(String, String)>,
}

impl std::fmt::Debug for BuildIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.secrets.iter().map(|(n, _)| n.as_str()).collect();
        f.debug_struct("BuildIdentity").field("site_key", &self.site_key).field("secrets", &names).finish()
    }
}

/// [`BuildIdentity`] of the `backend.toml` text `text`; a relative `file:` path is read from
/// `backend_dir`. No `[identity]`, or text that is not TOML, gives the default.
pub fn build_identity(text: &str, backend_dir: &std::path::Path) -> BuildIdentity {
    let mut out = BuildIdentity::default();
    let Ok(doc) = text.parse::<toml::Table>() else { return out };
    let Some(id) = doc.get("identity").and_then(|v| v.as_table()) else { return out };
    let recaptcha = id.get("recaptcha").and_then(|v| v.as_table());
    out.site_key = recaptcha.and_then(|r| r.get("site_key")).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let mut specs: Vec<(String, &str)> = Vec::new();
    if let (Some(p), Some(settings)) = (id.get("provider").and_then(|v| v.as_str()).and_then(Provider::parse), id.get("settings").and_then(|v| v.as_table())) {
        for (name, _, secret) in provider_settings(p) {
            if let (true, Some(v)) = (*secret, settings.get(*name).and_then(|v| v.as_str())) {
                specs.push((format!("identity.settings.{name}"), v));
            }
        }
    }
    if let Some(v) = id.get("cookie_key").and_then(|v| v.as_str()) {
        specs.push(("identity.cookie_key".into(), v));
    }
    if let Some(v) = recaptcha.and_then(|r| r.get("secret")).and_then(|v| v.as_str()) {
        specs.push(("identity.recaptcha.secret".into(), v));
    }
    for (name, spec) in specs {
        let spec = spec.trim();
        let spec = match spec.strip_prefix("file:") {
            Some(path) if std::path::Path::new(path).is_relative() => format!("file:{}", backend_dir.join(path).display()),
            _ => spec.to_owned(),
        };
        if let Ok(value) = resolve_secret(&spec) {
            if !value.trim().is_empty() {
                out.secrets.push((name, value.trim().to_owned()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_reads_the_site_key_and_every_secret_that_resolves_never_showing_one() {
        std::env::set_var("PRC_BUILD_ID_SECRET", "client-secret-value-0123");
        let dir = std::env::temp_dir().join(format!("prc-build-identity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cookie.key"), "cookie-key-value-4567\n").unwrap();
        let text = "[identity]\nprovider = \"keycloak\"\ncookie_key = \"file:cookie.key\"\n[identity.settings]\nissuer = \"https://kc.example.test/realms/r\"\nclient_id = \"app\"\nclient_secret = \"env:PRC_BUILD_ID_SECRET\"\n[identity.recaptcha]\nsite_key = \"6Lc-site\"\nsecret = \"env:PRC_BUILD_ID_UNSET\"\n";
        let b = build_identity(text, &dir);
        assert_eq!(b.site_key.as_deref(), Some("6Lc-site"));
        assert_eq!(
            b.secrets,
            vec![("identity.settings.client_secret".to_string(), "client-secret-value-0123".to_string()), ("identity.cookie_key".to_string(), "cookie-key-value-4567".to_string())],
            "an unset variable has nothing to search for"
        );
        let shown = format!("{b:?}");
        assert!(!shown.contains("client-secret-value") && !shown.contains("cookie-key-value"), "{shown}");
        assert!(build_identity("[server]\n", &dir).secrets.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
