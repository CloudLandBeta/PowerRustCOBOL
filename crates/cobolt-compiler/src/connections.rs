// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Named service connections a project defines once and its forms point at.
//!
//! A `RestClient` carried its whole connection — base URL, auth, timeouts — on
//! the control, which meant two things. A project talking to the same API from
//! six forms configured it six times and drifted; and the **credential had
//! nowhere to live but the `.cfrm`**, which is a file people commit. That is
//! how a live key reached this repository's own `main`.
//!
//! A connection is now definable once, at project level, and referenced by
//! name. The control keeps its local properties and a `Configuration` property
//! that selects between them:
//!
//! * empty — **local**: the control's own properties, exactly as before, so
//!   every existing form behaves identically and nothing had to be migrated.
//! * a connection id — **project**: that connection's fields are resolved into
//!   the control before the form runs, and the control's own are ignored.
//!
//! **The secret is never part of this.** Like [`ExternalCrate`](crate::external_crates::ExternalCrate),
//! the record here is the non-secret half and round-trips in `cobolt.toml`; the
//! key lives in the machine-local store under `connection::<id>` and reaches a
//! running form through the environment, the same discipline the Maps and Web
//! Search keys have always used (R31). A colleague who checks the project out
//! gets the connections and supplies their own key.
//!
//! The type lives in the **compiler**, not the IDE, for the reason
//! `ExternalCrate` does: `rcrun build` reads the same records from the same
//! `cobolt.toml` with no IDE involved.

use serde::{Deserialize, Serialize};

/// The control property that selects local or project configuration.
pub const CONFIGURATION_PROP: &str = "Configuration";

fn default_true() -> bool {
    true
}

fn default_timeout_seconds() -> u32 {
    30
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_auth_type() -> String {
    "None".to_string()
}

/// One named REST connection: everything a `RestClient` needs to reach a
/// service, minus the credential.
///
/// Field names mirror the control's own properties deliberately — the whole
/// operation of "use the project's configuration" is copying these onto the
/// control, and a reader should be able to see that they line up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestConnection {
    /// UUID v4, generated at creation. What a form stores, so renaming a
    /// connection does not break the forms that use it.
    pub id: String,
    /// Unique, human-facing display name — what the properties pane shows and
    /// what a reviewer sees in a diff.
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default = "default_method")]
    pub default_method: String,
    /// `None` | `Bearer` | `Basic` | `APIKey` — the control's own vocabulary.
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    /// `key: value` pairs, newline-separated, as on the control.
    #[serde(default)]
    pub default_headers: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u32,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    #[serde(default = "default_true")]
    pub verify_tls: bool,
}

impl RestConnection {
    /// A new connection with the control's own defaults, so "add a connection"
    /// starts from what a fresh `RestClient` would have done.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base_url: String::new(),
            default_method: default_method(),
            auth_type: default_auth_type(),
            default_headers: String::new(),
            timeout_seconds: default_timeout_seconds(),
            follow_redirects: true,
            verify_tls: true,
        }
    }
}

/// The machine-local credential slot holding one connection's secret.
///
/// Deliberately keyed by **id**, not by name: renaming a connection must not
/// orphan its key. Same shape as the model providers' `providerkey::<id>`.
pub fn connection_key_slot(id: &str) -> String {
    format!("connection::{}", id.trim())
}

/// The connection a control is bound to, or `None` when it is on its own
/// local settings.
///
/// An id that names no connection also yields `None` *and* is reported by
/// [`unresolved_configuration`], because silently falling back to the local
/// settings of a control that was configured to ignore them would send a
/// request somewhere the developer did not choose.
pub fn bound_connection<'a>(
    ctrl: &cobolt_forms::Control,
    connections: &'a [RestConnection],
) -> Option<&'a RestConnection> {
    let id = configuration_id(ctrl)?;
    connections.iter().find(|c| c.id == id)
}

/// The `Configuration` id set on a control, if any. Empty means local.
pub fn configuration_id(ctrl: &cobolt_forms::Control) -> Option<String> {
    ctrl.get_prop(CONFIGURATION_PROP)
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// `Some(id)` when a control names a configuration that does not exist.
///
/// A dangling reference is a configuration error, not a silent fallback: the
/// project once had that connection and no longer does, so the honest outcome
/// is to say so rather than quietly use settings the developer overrode.
pub fn unresolved_configuration(
    ctrl: &cobolt_forms::Control,
    connections: &[RestConnection],
) -> Option<String> {
    let id = configuration_id(ctrl)?;
    connections.iter().all(|c| c.id != id).then_some(id)
}

/// Copy a connection's fields onto a control, replacing its local ones.
///
/// Applied before the form runs, so the interpreter reads `BaseURL`,
/// `AuthType` and the rest exactly as it always has and needs to know nothing
/// about connections. `AuthToken` is **not** set here — it is not in this
/// record; it arrives separately from the machine-local store.
pub fn apply(ctrl: &mut cobolt_forms::Control, conn: &RestConnection) {
    use cobolt_forms::PropValue as P;
    ctrl.set_prop("BaseURL", P::String(conn.base_url.clone()));
    ctrl.set_prop("DefaultMethod", P::String(conn.default_method.clone()));
    ctrl.set_prop("AuthType", P::String(conn.auth_type.clone()));
    ctrl.set_prop("DefaultHeaders", P::String(conn.default_headers.clone()));
    ctrl.set_prop("TimeoutSeconds", P::Int(conn.timeout_seconds as i64));
    ctrl.set_prop("FollowRedirects", P::Bool(conn.follow_redirects));
    ctrl.set_prop("VerifyTLS", P::Bool(conn.verify_tls));
}

/// Resolve every `RestClient` in `controls` that is bound to a project
/// connection, returning the ids that named nothing.
///
/// The caller decides what an unresolved id means; the compiler refuses the
/// build, and the IDE reports it rather than running a form at an address
/// nobody chose.
pub fn resolve_all(
    controls: &mut [cobolt_forms::Control],
    connections: &[RestConnection],
) -> Vec<(String, String)> {
    let mut dangling = Vec::new();
    for ctrl in controls.iter_mut() {
        if ctrl.control_type != cobolt_forms::ControlType::RestClient {
            continue;
        }
        if let Some(id) = unresolved_configuration(ctrl, connections) {
            dangling.push((ctrl.id.clone(), id));
            continue;
        }
        if let Some(conn) = bound_connection(ctrl, connections).cloned() {
            apply(ctrl, &conn);
        }
    }
    dangling
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType, PropValue};

    fn conn() -> RestConnection {
        RestConnection {
            id: "11111111-2222-3333-4444-555555555555".into(),
            name: "Billing API (prod)".into(),
            base_url: "https://billing.example.com".into(),
            default_method: "POST".into(),
            auth_type: "Bearer".into(),
            default_headers: "Accept: application/json".into(),
            timeout_seconds: 45,
            follow_redirects: false,
            verify_tls: false,
        }
    }

    /// Every property, sorted — `Control` has no `PartialEq`, and "was this
    /// control left alone?" is exactly a question about its properties.
    fn snapshot(c: &Control) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = c
            .properties
            .iter()
            .map(|(k, p)| (k.clone(), p.to_xml_string()))
            .collect();
        v.sort();
        v
    }

    fn rest(configuration: &str) -> Control {
        let mut c = Control::new("REST-1", ControlType::RestClient, 0, 0);
        c.set_prop("BaseURL", PropValue::String("https://local.invalid".into()));
        c.set_prop("DefaultMethod", PropValue::String("GET".into()));
        c.set_prop("AuthType", PropValue::String("None".into()));
        if !configuration.is_empty() {
            c.set_prop(
                CONFIGURATION_PROP,
                PropValue::String(configuration.to_owned()),
            );
        }
        c
    }

    /// **An unset Configuration keeps the control's own settings, untouched.**
    ///
    /// Every form in existence is on this path, so it is the one that must not
    /// move: the feature is opt-in per control, and a project that defines no
    /// connections behaves exactly as it did.
    #[test]
    fn a_control_with_no_configuration_is_left_entirely_alone() {
        let mut controls = vec![rest("")];
        let before = snapshot(&controls[0]);
        let dangling = resolve_all(&mut controls, &[conn()]);
        assert!(dangling.is_empty());
        assert_eq!(snapshot(&controls[0]), before, "local control must not be rewritten");
        assert_eq!(configuration_id(&controls[0]), None);
    }

    /// **A bound control takes the project's connection, all of it.**
    #[test]
    fn a_bound_control_takes_every_field_of_its_connection() {
        let c = conn();
        let mut controls = vec![rest(&c.id)];
        assert!(resolve_all(&mut controls, &[c.clone()]).is_empty());
        let got = &controls[0];
        let s = |k: &str| got.get_prop(k).map(|v| v.as_str().to_owned()).unwrap_or_default();
        assert_eq!(s("BaseURL"), c.base_url, "the local URL must be replaced");
        assert_eq!(s("DefaultMethod"), "POST");
        assert_eq!(s("AuthType"), "Bearer");
        assert_eq!(s("DefaultHeaders"), "Accept: application/json");
        assert_eq!(got.get_prop("TimeoutSeconds").unwrap().as_i64(), 45);
        assert!(!got.get_prop("FollowRedirects").unwrap().as_bool());
        assert!(!got.get_prop("VerifyTLS").unwrap().as_bool());
        // The credential is not in the record and must not have been invented.
        assert_eq!(s("AuthToken"), "");
    }

    /// **A Configuration naming nothing is reported, not silently ignored.**
    ///
    /// The control was explicitly told to ignore its own settings. Falling
    /// back to them would send the request to an address the developer had
    /// already overridden — quietly, and only on the machine where the
    /// connection was missing.
    #[test]
    fn a_dangling_configuration_is_reported_and_nothing_is_applied() {
        let mut controls = vec![rest("no-such-connection")];
        let before = snapshot(&controls[0]);
        let dangling = resolve_all(&mut controls, &[conn()]);
        assert_eq!(
            dangling,
            vec![("REST-1".to_owned(), "no-such-connection".to_owned())],
            "a dangling reference must name the control and the missing id"
        );
        assert_eq!(
            snapshot(&controls[0]),
            before,
            "nothing may be applied from a connection that does not exist"
        );
    }

    /// **Only RestClients are touched.**
    #[test]
    fn controls_of_other_types_are_never_rewritten() {
        let mut other = Control::new("AGENT-1", ControlType::AgentObject, 0, 0);
        other.set_prop(
            CONFIGURATION_PROP,
            PropValue::String(conn().id.clone()),
        );
        let before = snapshot(&other);
        let mut controls = vec![other];
        assert!(resolve_all(&mut controls, &[conn()]).is_empty());
        assert_eq!(
            snapshot(&controls[0]),
            before,
            "AgentObject is a later pass and must not be half-resolved now"
        );
    }

    /// **The credential slot is keyed by id, so a rename cannot orphan a key.**
    #[test]
    fn the_key_slot_follows_the_id_not_the_name() {
        let mut c = conn();
        let slot = connection_key_slot(&c.id);
        c.name = "Billing API (renamed)".into();
        assert_eq!(connection_key_slot(&c.id), slot);
        assert!(slot.starts_with("connection::"), "{slot}");
    }

    /// **The record round-trips through `cobolt.toml` and carries no secret.**
    #[test]
    fn the_record_round_trips_and_holds_no_credential() {
        let c = conn();
        let toml = toml::to_string(&c).expect("serialize");
        assert!(
            !toml.to_lowercase().contains("token") && !toml.to_lowercase().contains("secret"),
            "a connection record must carry no credential field: {toml}"
        );
        let back: RestConnection = toml::from_str(&toml).expect("deserialize");
        assert_eq!(back, c);

        // A minimal record fills in the control's own defaults, not zeroes —
        // a connection written by hand with two lines must not silently
        // disable TLS verification.
        let minimal: RestConnection =
            toml::from_str("id = \"x\"\nname = \"Minimal\"\n").expect("minimal");
        assert!(minimal.verify_tls, "TLS verification defaults ON");
        assert!(minimal.follow_redirects);
        assert_eq!(minimal.timeout_seconds, 30);
        assert_eq!(minimal.default_method, "GET");
        assert_eq!(minimal.auth_type, "None");
    }
}
