// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Check of a Web or Mobile project's sign-in (WASM-009 R8b, R9, R11; D11, D13; plan §9 R1):
//! what the forms, the project's `[web]` settings and the backend's `[identity]` section say
//! together. Each finding names the form, the control or the setting it is about.
//!
//! Errors: a form requires sign-in and there is no sign-in form; `Mode = InApp` on a control whose
//! provider has no in-app sign-in. Warnings: a control's `RecaptchaSiteKey` differs from the
//! backend's `site_key`; the page and the backend are on different sites, so the refresh cookie
//! (`SameSite=Strict`) is never sent and a session ends when its access token does.

use cobolt_forms::model::{Control, ControlType, Form};
use cobolt_server::identity_settings::Provider;

use crate::project_kind::WebSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityFinding {
    pub error: bool,
    /// The form, `FORM ▸ CONTROL`, or the setting.
    pub subject: String,
    pub message: String,
}

fn auth_controls<'a>(cs: &'a [Control], out: &mut Vec<&'a Control>) {
    for c in cs {
        if c.control_type == ControlType::Authentication {
            out.push(c);
        }
        auth_controls(&c.children, out);
    }
}

fn prop(c: &Control, name: &str) -> String {
    c.get_prop(name).map(|v| v.as_str().trim().to_owned()).unwrap_or_default()
}

/// The registrable part of a host, approximately: the last two labels, or three when the
/// second-last is a short second-level word (`example.co.uk`). An address or `localhost` is itself.
fn site_of(host: &str) -> String {
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
        return host;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let n = labels.len();
    let take = if n >= 3 && labels[n - 1].len() == 2 && ["co", "com", "org", "net", "gov", "ac", "edu"].contains(&labels[n - 2]) { 3 } else { 2 };
    labels[n.saturating_sub(take)..].join(".")
}

fn host_of(address: &str) -> Option<String> {
    let rest = address.trim().split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?.to_owned(),
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h).to_owned(),
    };
    (!host.is_empty()).then_some(host)
}

/// The findings of `forms` (`(form, controls)`), the project's `web` settings and the backend's
/// `backend.toml` text (`None`: no backend, or no file).
pub fn identity_rules(forms: &[&Form], web: &WebSettings, backend_toml: Option<&str>) -> Vec<IdentityFinding> {
    let mut out = Vec::new();
    let doc = backend_toml.and_then(|t| t.parse::<toml::Table>().ok());
    let identity = doc.as_ref().and_then(|d| d.get("identity")).and_then(|v| v.as_table());
    let id_text = |k: &str| identity.and_then(|i| i.get(k)).and_then(|v| v.as_str()).unwrap_or("").trim().to_owned();
    let provider = Provider::parse(&id_text("provider"));
    let backend_site_key = identity.and_then(|i| i.get("recaptcha")).and_then(|r| r.get("site_key")).and_then(|v| v.as_str()).unwrap_or("").trim().to_owned();

    // 1. A form requires sign-in: there must be a sign-in form.
    let protected: Vec<&str> = forms.iter().filter(|f| f.requires_sign_in).map(|f| f.name.as_str()).collect();
    let named = web.sign_in_form.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if let Some(n) = named {
        if !forms.iter().any(|f| f.name.eq_ignore_ascii_case(n)) {
            out.push(IdentityFinding { error: true, subject: "web.sign_in_form".into(), message: format!("web.sign_in_form names the form `{n}`, which the project does not have") });
        }
    } else if !protected.is_empty() {
        let has_one = forms.iter().any(|f| {
            let mut a = Vec::new();
            auth_controls(&f.controls, &mut a);
            !a.is_empty()
        });
        if !has_one {
            for name in &protected {
                out.push(IdentityFinding {
                    error: true,
                    subject: (*name).to_owned(),
                    message: format!("form `{name}` requires sign-in, and the project has no sign-in form: add a form with an Authentication control, or name one in web.sign_in_form"),
                });
            }
        }
    }

    for f in forms {
        let mut controls = Vec::new();
        auth_controls(&f.controls, &mut controls);
        for c in controls {
            let subject = format!("{} ▸ {}", f.name, c.id);
            // 2. In-app sign-in needs a provider that has it (plan §6).
            let in_app = matches!(prop(c, "Mode").to_ascii_lowercase().as_str(), "inapp" | "in-app" | "in_app");
            if let (true, Some(p)) = (in_app, provider) {
                let flow = id_text("flow").to_ascii_lowercase();
                let refused = match p {
                    Provider::Cognito => false,
                    Provider::Keycloak | Provider::Entra => true,
                    Provider::EntraExternal => flow != "native",
                };
                if refused {
                    let how = if p == Provider::EntraExternal { "set the backend's identity.flow to native, or Mode to Hosted" } else { "set Mode to Hosted" };
                    out.push(IdentityFinding {
                        error: true,
                        subject: subject.clone(),
                        message: format!("{subject}: Mode is InApp, and the backend's provider ({}) has no in-app sign-in: {how}", p.word()),
                    });
                }
            }
            // 3. One site key: the control's override should be the backend's.
            let own = prop(c, "RecaptchaSiteKey");
            if !own.is_empty() && !backend_site_key.is_empty() && own != backend_site_key {
                out.push(IdentityFinding {
                    error: false,
                    subject: subject.clone(),
                    message: format!("{subject}: RecaptchaSiteKey differs from the backend's identity.recaptcha.site_key; the backend verifies tokens of its own key only"),
                });
            }
        }
    }

    // 4. The refresh cookie needs the page and the backend on the same site.
    if identity.is_some() {
        if let Some(backend) = host_of(&web.backend_url) {
            let origins: Vec<String> = identity.and_then(|i| i.get("origins")).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|o| o.as_str().map(str::to_owned)).collect()).unwrap_or_default();
            for origin in origins {
                if let Some(page) = host_of(&origin) {
                    if site_of(&page) != site_of(&backend) {
                        out.push(IdentityFinding {
                            error: false,
                            subject: "identity.origins".into(),
                            message: format!("the page origin {origin} and the backend ({backend}) are on different sites: the browser does not send the refresh cookie, so a session ends when its access token expires"),
                        });
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::xml::load_form_from_str;

    fn form(name: &str, controls: &str, protected: bool) -> Form {
        let mut f = load_form_from_str(&format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"{name}\" title=\"t\" width=\"640\" height=\"480\" index-base=\"1\">\n{controls}\n</Form>"
        ))
        .unwrap();
        f.requires_sign_in = protected;
        f
    }

    fn auth(props: &str) -> String {
        format!("<Control id=\"AUTH-1\" type=\"Authentication\" x=\"10\" y=\"10\" w=\"300\" h=\"300\">{props}</Control>")
    }

    fn backend(provider: &str, extra: &str) -> String {
        format!("[server]\nlisten = \"127.0.0.1:0\"\n[identity]\nprovider = \"{provider}\"\n{extra}")
    }

    fn found(forms: &[&Form], web: &WebSettings, toml: Option<&str>) -> Vec<(bool, String, String)> {
        let f = identity_rules(forms, web, toml);
        for x in &f {
            println!("  {} {}: {}", if x.error { "error" } else { "warning" }, x.subject, x.message);
        }
        f.into_iter().map(|x| (x.error, x.subject, x.message)).collect()
    }

    #[test]
    fn a_protected_form_without_a_sign_in_form_is_an_error_naming_the_form() {
        let orders = form("orders", "", true);
        let web = WebSettings::default();
        let f = found(&[&orders], &web, None);
        assert_eq!(f.len(), 1);
        assert!(f[0].0 && f[0].1 == "orders" && f[0].2.contains("`orders` requires sign-in"));
        let login = form("login", &auth(""), false);
        assert!(found(&[&orders, &login], &web, None).is_empty(), "a form with the control is the sign-in form");
        let web = WebSettings { sign_in_form: Some("welcome".into()), ..WebSettings::default() };
        let f = found(&[&orders, &login], &web, None);
        assert!(f.len() == 1 && f[0].0 && f[0].1 == "web.sign_in_form" && f[0].2.contains("`welcome`"), "{f:?}");
    }

    #[test]
    fn in_app_for_a_provider_without_in_app_sign_in_is_an_error_naming_the_control() {
        let login = form("login", &auth("<Property name=\"Mode\">InApp</Property>"), false);
        let web = WebSettings::default();
        for (provider, extra, refused) in [("cognito", "", false), ("keycloak", "", true), ("entra", "", true), ("entra-external", "", true), ("entra-external", "flow = \"native\"\n", false)] {
            let f = found(&[&login], &web, Some(&backend(provider, extra)));
            assert_eq!(f.iter().any(|x| x.0 && x.1 == "login ▸ AUTH-1" && x.2.contains("Mode is InApp")), refused, "{provider} {extra}: {f:?}");
        }
        let hosted = form("login", &auth("<Property name=\"Mode\">Hosted</Property>"), false);
        assert!(found(&[&hosted], &web, Some(&backend("keycloak", ""))).is_empty());
    }

    #[test]
    fn a_site_key_that_differs_from_the_backends_is_a_warning_naming_the_control() {
        let login = form("login", &auth("<Property name=\"RecaptchaSiteKey\">6Lc-other</Property>"), false);
        let web = WebSettings::default();
        let f = found(&[&login], &web, Some(&backend("cognito", "[identity.recaptcha]\nsite_key = \"6Lc-backend\"\n")));
        assert!(f.len() == 1 && !f[0].0 && f[0].1 == "login ▸ AUTH-1" && f[0].2.contains("RecaptchaSiteKey"), "{f:?}");
        assert!(found(&[&login], &web, Some(&backend("cognito", "[identity.recaptcha]\nsite_key = \"6Lc-other\"\n"))).is_empty());
    }

    #[test]
    fn a_page_and_a_backend_on_different_sites_is_a_warning_naming_the_setting() {
        let login = form("login", &auth(""), false);
        let web = WebSettings { backend_url: "wss://api.example.com/ws".into(), ..WebSettings::default() };
        let toml = backend("cognito", "origins = [\"https://app.example.com\", \"https://shop.other.org\"]\n");
        let f = found(&[&login], &web, Some(&toml));
        assert!(f.len() == 1 && !f[0].0 && f[0].1 == "identity.origins" && f[0].2.contains("https://shop.other.org"), "{f:?}");
        assert_eq!(site_of("app.example.co.uk"), "example.co.uk");
        assert_eq!(site_of("127.0.0.1"), "127.0.0.1");
    }
}
