// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The settings of a Web or Mobile project** (WASM-001, WASM-002 R18–R20,
//! WASM-017 R9), as a section of the project's Settings form.
//!
//! A Web project asks for: the application style (SPA or standalone forms), the
//! browsers the build promises to run in, whether to emit the multi-memory
//! program module (and so, with the matrix, which modules the bundle carries),
//! the cross-origin isolation the bundle is served with, the Backend's address,
//! and the programs the Backend exports. A Mobile project asks for its
//! platform and device (shown; they are fixed at creation), the orientation, the
//! minimum OS, the style, multi-memory (off by default for iOS), and the same
//! Backend address and exports. A Desktop project shows none of it.
//!
//! The section edits the form's draft ([`SettingsDraft`]); `apply_kind` writes
//! the draft back into the project, and [`write_exports`] writes the exported
//! programs to `backend/exports.toml`, their one home (WASM-003 D5). The Backend's address is checked against the
//! rule of WASM-003 R5 as it is typed — `wss://`, or `ws://` for a loopback
//! address — and a refused address is never written to the project.
//!
//! The **browser the Preview opens** (WASM-008 R12, Q7, D11) is one row: the
//! system's default, a browser of the closed list that is on this machine, or
//! *Other…* with an application's path. A path is started only after the
//! developer said yes once, on this machine, for this project folder: the
//! answer is kept in [`BrowserConfirmations`], beside `debug_settings.toml`,
//! never in the project — so a downloaded project cannot make the Preview
//! start a program by itself.

use std::path::{Path, PathBuf};

use egui::{RichText, Ui};

use cobolt_compiler::project_kind::{
    backend_url_problem, ApplicationStyle, BackendUrlProblem, Isolation, LoadingEffect, MobilePlatform, ModuleVariants, Orientation, BACKEND_FOLDER,
};
use cobolt_compiler::ProjectKind;
use cobolt_project_tools::templates;
use cobolt_project_tools::web::preview::{BrowserChoice, KnownBrowser};
use serde::{Deserialize, Serialize};

use crate::i18n::Tr;
use crate::panels::new_project::{choice_label, LOCALES};
use crate::panels::settings_form::{section, SettingsDraft};
use crate::project_model::CoboltProject;
use crate::theme::Theme;

/// What the Backend address box shows while it is empty: the shape of an
/// address, not a word of any language.
const BACKEND_URL_HINT: &str = "wss://\u{2026}";

/// The measurements of the Settings form's two-column rows, passed in so this
/// section lines up with the rows above and below it.
#[derive(Clone, Copy)]
pub struct RowLayout {
    pub splitter: f32,
    pub resizer_width: f32,
    pub gap_after_resizer: f32,
    pub property_indent: f32,
}

/// One row, laid out as the form's other rows are: the label in the left
/// column (cut at the splitter), the value in the right.
fn row(ui: &mut Ui, lay: RowLayout, label: &str, value: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        let left_rect = ui.allocate_exact_size(egui::vec2(lay.splitter, 0.0), egui::Sense::hover()).0;
        ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.set_min_width(lay.splitter);
            ui.add_space(lay.property_indent);
            ui.add(egui::Label::new(label).truncate());
        });
        ui.allocate_space(egui::vec2(lay.resizer_width, 0.0));
        ui.add_space(lay.gap_after_resizer);
        let right_w = ui.available_width();
        ui.allocate_ui(egui::vec2(right_w, 0.0), value);
    });
}

/// A small note under a row, in the dim ink.
fn note(ui: &mut Ui, lay: RowLayout, text: &str, colour: egui::Color32) {
    row(ui, lay, "", |ui| {
        ui.add(egui::Label::new(RichText::new(text).small().color(colour)).wrap());
    });
}

/// The programs a Backend exports, from the one-name-per-line text: trimmed,
/// no empty line, no name twice, in the order written.
pub fn parse_exports(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !out.iter().any(|n| n == line) {
            out.push(line.to_owned());
        }
    }
    out
}

/// Names in the exports text that cannot be a COBOL program name (letters,
/// digits and hyphens, starting with a letter).
pub fn invalid_export_names(text: &str) -> Vec<String> {
    parse_exports(text)
        .into_iter()
        .filter(|n| {
            let mut chars = n.chars();
            !(chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '-'))
        })
        .collect()
}

/// The modules a bundle will carry, in words: `multi-memory module, single-memory module`.
pub fn variants_text(tr: &Tr, v: ModuleVariants) -> String {
    let mut parts = Vec::new();
    if v.multi_memory {
        parts.push(tr.pk_set_variant_multi);
    }
    if v.single_memory {
        parts.push(tr.pk_set_variant_single);
    }
    parts.join(", ")
}

/// The sentence for a refused Backend address.
fn url_problem_text<'a>(tr: &'a Tr, p: BackendUrlProblem) -> &'a str {
    match p {
        BackendUrlProblem::Scheme => tr.pk_set_url_scheme,
        BackendUrlProblem::Host => tr.pk_set_url_host,
        BackendUrlProblem::PlainNotLoopback => tr.pk_set_url_plain,
    }
}

fn style_text<'a>(tr: &'a Tr, s: ApplicationStyle) -> &'a str {
    match s {
        ApplicationStyle::Spa => tr.pk_c_spa,
        ApplicationStyle::Standalone => tr.pk_c_standalone,
    }
}

fn loading_effect_text<'a>(tr: &'a Tr, e: LoadingEffect) -> &'a str {
    match e {
        LoadingEffect::None => tr.pk_c_loading_none,
        LoadingEffect::MovingStripes => tr.pk_c_loading_stripes,
    }
}

fn orientation_text<'a>(tr: &'a Tr, o: Orientation) -> &'a str {
    match o {
        Orientation::Auto => tr.pk_c_orient_auto,
        Orientation::Portrait => tr.pk_c_orient_portrait,
        Orientation::Landscape => tr.pk_c_orient_landscape,
    }
}

fn isolation_text<'a>(tr: &'a Tr, i: Isolation) -> &'a str {
    match i {
        Isolation::Off => tr.pk_set_iso_off,
        Isolation::RequireCorp => tr.pk_set_iso_corp,
        Isolation::Credentialless => tr.pk_set_iso_credentialless,
    }
}

/// The minimum-OS choices of a platform, from the template catalogue.
pub fn min_os_choices(platform: MobilePlatform) -> &'static [templates::Choice] {
    let id = match platform {
        MobilePlatform::Android => "android-phone",
        MobilePlatform::Ios => "iphone",
    };
    let catalogue = templates::builtin();
    templates::find(&catalogue, id)
        .and_then(|t| t.option("min_os"))
        .and_then(|o| match o.kind {
            templates::OptionKind::Choice(c) => Some(c),
            templates::OptionKind::Flag => None,
        })
        .unwrap_or(&[])
}

impl SettingsDraft {
    /// Write the kind's settings back into the project. A Desktop project's
    /// are untouched. The Backend's address is written only if it is one the
    /// channel accepts (WASM-003 R5), a Mobile project's Application id only if it is one
    /// (WASM-012 R6): the section told the developer why not.
    pub fn apply_kind(&self, p: &mut CoboltProject) {
        p.project.description = self.description.clone();
        p.project.locale = self.locale.clone();
        p.project.currency = self.currency.trim().to_ascii_uppercase();
        let accepted_url = |url: &str, kept: &str| -> String {
            if backend_url_problem(url).is_none() {
                url.trim().to_owned()
            } else {
                kept.to_owned()
            }
        };
        match self.kind {
            ProjectKind::Desktop => {}
            ProjectKind::Web => {
                let kept = p.web.as_ref().map(|w| w.backend_url.clone()).unwrap_or_default();
                let mut web = self.web.clone();
                web.backend_url = accepted_url(&self.web.backend_url, &kept);
                p.web = Some(web);
            }
            ProjectKind::Mobile => {
                if let Some(draft) = &self.mobile {
                    let kept = p.mobile.as_ref().map(|m| m.backend_url.clone()).unwrap_or_default();
                    let mut mobile = draft.clone();
                    mobile.backend_url = accepted_url(&draft.backend_url, &kept);
                    // The Application id likewise (WASM-012 R6): one that is not an id keeps the
                    // saved one, and the page says why.
                    mobile.app_id = if cobolt_compiler::project_kind::validate_app_id(&draft.app_id).is_ok() {
                        draft.app_id.trim().to_owned()
                    } else {
                        p.mobile.as_ref().map(|m| m.app_id.clone()).unwrap_or_default()
                    };
                    p.mobile = Some(mobile);
                }
            }
        }
        // The exports are not the project file's (WASM-003 D5): `write_exports`
        // writes them to backend/exports.toml. The state is.
        if self.kind.has_backend() && (self.has_backend || p.backend.is_some()) {
            let default = self.backend_state.trim().is_empty() || self.backend_state == STATE_PER_CALL;
            if !default || p.backend.is_some() {
                p.backend_settings_mut().state = self.backend_state.clone();
            }
        }
    }
}

/// `[backend] state`'s two spellings (WASM-003 Q2; the words shown are `Tr` keys).
pub const STATE_PER_CALL: &str = "per-call";
pub const STATE_PER_CONNECTION: &str = "per-connection";

fn state_text<'a>(tr: &'a Tr, state: &str) -> &'a str {
    if state == STATE_PER_CONNECTION {
        tr.pk_set_state_per_connection
    } else {
        tr.pk_set_state_per_call
    }
}

/// `backend/exports.toml` of the project in `project_dir`.
pub fn exports_path(project_dir: &Path) -> PathBuf {
    project_dir.join(BACKEND_FOLDER).join("exports.toml")
}

/// The programs `backend/exports.toml` lists, one per line, as the exports box
/// shows them; empty when there is no file or it cannot be read.
pub fn read_exports_text(project_dir: &Path) -> String {
    let Ok(doc) = std::fs::read_to_string(exports_path(project_dir)).unwrap_or_default().parse::<toml_edit::DocumentMut>() else {
        return String::new();
    };
    doc.get("export")
        .and_then(|e| e.as_array_of_tables())
        .map(|a| a.iter().filter_map(|t| t.get("program").and_then(|p| p.as_str()).map(str::to_owned)).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Write `names` as the exports of `backend/exports.toml`, in that order. An
/// entry that stays keeps every key it has (`roles`, `public`, `idempotent`, …)
/// and its comments; a new name gets an entry with only `program`; a name taken
/// out of the box is taken out of the file. Everything else in the file stays.
/// A missing file is created with contract `1.0.0`. Returns whether it changed.
pub fn write_exports(project_dir: &Path, names: &[String]) -> Result<bool, String> {
    let path = exports_path(project_dir);
    let before = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = before.as_deref().unwrap_or("[contract]\nversion = \"1.0.0\"\n").parse().map_err(|e| format!("{}: {e}", path.display()))?;
    let old: Vec<toml_edit::Table> = doc.get("export").and_then(|e| e.as_array_of_tables()).map(|a| a.iter().cloned().collect()).unwrap_or_default();
    let mut list = toml_edit::ArrayOfTables::new();
    for name in names {
        let kept = old.iter().find(|t| t.get("program").and_then(|p| p.as_str()) == Some(name.as_str())).cloned();
        list.push(kept.unwrap_or_else(|| {
            let mut t = toml_edit::Table::new();
            t.insert("program", toml_edit::value(name.as_str()));
            t
        }));
    }
    doc.insert("export", toml_edit::Item::ArrayOfTables(list));
    let after = doc.to_string();
    if before.as_deref() == Some(after.as_str()) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, after).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// A project from before `backend/exports.toml` was the list (WASM-003 C5):
/// a Web or Mobile project whose file has none and whose project file has a
/// non-empty `[backend] exports` gets the file, written from those names (no
/// roles, not public). The project file is left as it is. `Some(path)` when
/// the file was written.
pub fn migrate_exports(project_dir: &Path, p: &CoboltProject) -> Result<Option<PathBuf>, String> {
    let old = p.backend.as_ref().map(|b| b.exports.clone()).unwrap_or_default();
    let path = exports_path(project_dir);
    if !p.kind().has_backend() || old.is_empty() || path.exists() {
        return Ok(None);
    }
    write_exports(project_dir, &old)?;
    Ok(Some(path))
}

// ── The Backend's identity provider (WASM-009 T19, R9, D13) ─────────────────

/// `backend/backend.toml` of the project in `project_dir`.
pub fn backend_config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(BACKEND_FOLDER).join("backend.toml")
}

/// The Identity section as the page edits it: `[identity]` of `backend.toml`, every value as text.
/// An empty `provider` is "no `[identity]`"; an empty `flow` is `auto`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IdentityDraft {
    pub provider: String,
    pub flow: String,
    /// The provider's settings, by the names [`provider_settings`] gives them.
    pub settings: std::collections::BTreeMap<String, String>,
    /// One origin per line.
    pub origins_text: String,
    pub cookie_key: String,
    pub site_key: String,
    pub recaptcha_secret: String,
    /// Empty: the backend's default (0.5).
    pub min_score: String,
}

/// `[contract]` of `backend.toml` as the page edits it (WASM-014 T17): where the idempotency
/// record lives and how long a session may sit idle. Empty `idle_seconds` is the default (900).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDraft {
    /// `memory` or `server`.
    pub store: String,
    pub idle_seconds: String,
}

pub const STORE_MEMORY: &str = "memory";
pub const STORE_SERVER: &str = "server";

impl Default for ContractDraft {
    fn default() -> Self {
        Self { store: STORE_MEMORY.into(), idle_seconds: String::new() }
    }
}

impl ContractDraft {
    /// Is the idle time acceptable (empty, or a whole number of seconds above zero)?
    pub fn idle_ok(&self) -> bool {
        let t = self.idle_seconds.trim();
        t.is_empty() || t.parse::<u64>().is_ok_and(|v| v > 0)
    }
}

/// `[contract]` of the project's `backend.toml`; the defaults when there is none.
pub fn read_contract_settings(project_dir: &Path) -> ContractDraft {
    let Ok(doc) = std::fs::read_to_string(backend_config_path(project_dir)).unwrap_or_default().parse::<toml_edit::DocumentMut>() else {
        return ContractDraft::default();
    };
    let Some(c) = doc.get("contract").and_then(|c| c.as_table_like()) else { return ContractDraft::default() };
    ContractDraft {
        store: if c.get("idempotency_store").and_then(|v| v.as_str()) == Some(STORE_SERVER) { STORE_SERVER } else { STORE_MEMORY }.into(),
        idle_seconds: c.get("session_idle_seconds").and_then(|v| v.as_integer()).map(|i| i.to_string()).unwrap_or_default(),
    }
}

/// Write the draft into `[contract]`. Only the two keys change; every other section, key and
/// comment stays byte for byte, and a file without `[contract]` gains only that section. The
/// default (memory, no idle time) removes the keys it would add, and an emptied section it
/// created. Returns whether the file changed.
pub fn write_contract_settings(project_dir: &Path, d: &ContractDraft) -> Result<bool, String> {
    if !d.idle_ok() {
        return Err("contract.session_idle_seconds is not a whole number of seconds above zero".into());
    }
    let path = backend_config_path(project_dir);
    let before = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = before.parse().map_err(|e| format!("{}: {e}", path.display()))?;
    let want_store = d.store == STORE_SERVER;
    let idle = d.idle_seconds.trim().parse::<i64>().ok();
    let present = doc.get("contract").is_some();
    if !present && !want_store && idle.is_none() {
        return Ok(false);
    }
    if !present {
        doc.insert("contract", toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let c = doc["contract"].as_table_mut().ok_or("[contract] is not a table")?;
    let have_store = c.get("idempotency_store").and_then(|v| v.as_str()).map(str::to_owned);
    if want_store {
        if have_store.as_deref() != Some(STORE_SERVER) {
            c.insert("idempotency_store", toml_edit::value(STORE_SERVER));
        }
    } else if have_store.is_some() && have_store.as_deref() != Some(STORE_MEMORY) {
        c.insert("idempotency_store", toml_edit::value(STORE_MEMORY));
    }
    match idle {
        Some(v) if c.get("session_idle_seconds").and_then(|x| x.as_integer()) != Some(v) => {
            c.insert("session_idle_seconds", toml_edit::value(v));
        }
        None => {
            c.remove("session_idle_seconds");
        }
        _ => {}
    }
    let after = doc.to_string();
    if after == before {
        return Ok(false);
    }
    std::fs::write(&path, after).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// The port of the Backend's development run, as text: the port of `[server] listen` in the project's
/// `backend.toml`; empty when the file or the key is missing or has no usable port.
pub fn read_backend_port(project_dir: &Path) -> String {
    let Ok(doc) = std::fs::read_to_string(backend_config_path(project_dir)).unwrap_or_default().parse::<toml_edit::DocumentMut>() else {
        return String::new();
    };
    doc.get("server")
        .and_then(|s| s.as_table_like())
        .and_then(|s| s.get("listen"))
        .and_then(|v| v.as_str())
        .and_then(|l| l.trim().rsplit_once(':'))
        .and_then(|(_, p)| p.trim().parse::<u16>().ok())
        .filter(|p| *p != 0)
        .map(|p| p.to_string())
        .unwrap_or_default()
}

/// Is `text` a port a Backend can listen on (a whole number from 1 to 65535)?
pub fn backend_port_ok(text: &str) -> bool {
    text.trim().parse::<u16>().is_ok_and(|p| p != 0)
}

/// Write the port into `[server] listen`. Only that value changes: the host part stays as it is (`127.0.0.1` when the
/// file had none), every other section, key and comment stays byte for byte, and a file without `[server]` gains only
/// that section. Returns whether the file changed. An unusable port is refused and nothing is written.
pub fn write_backend_port(project_dir: &Path, text: &str) -> Result<bool, String> {
    if !backend_port_ok(text) {
        return Err(format!("`{}` is not a port from 1 to 65535", text.trim()));
    }
    let port: u16 = text.trim().parse().map_err(|_| "not a port".to_owned())?;
    let path = backend_config_path(project_dir);
    let before = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = before.parse().map_err(|e| format!("{}: {e}", path.display()))?;
    if doc.get("server").is_none() {
        doc.insert("server", toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let server = doc["server"].as_table_mut().ok_or("[server] is not a table")?;
    let host = server
        .get("listen")
        .and_then(|v| v.as_str())
        .and_then(|l| l.trim().rsplit_once(':'))
        .map(|(h, _)| h.to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_owned());
    let listen = format!("{host}:{port}");
    if server.get("listen").and_then(|v| v.as_str()) != Some(listen.as_str()) {
        // The value is replaced, its spacing and trailing comment kept.
        let decor = server.get("listen").and_then(|i| i.as_value()).map(|v| v.decor().clone());
        let mut item = toml_edit::value(listen);
        if let (Some(d), Some(v)) = (decor, item.as_value_mut()) {
            *v.decor_mut() = d;
        }
        server.insert("listen", item);
    }
    let after = doc.to_string();
    if after == before {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, after).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// What is wrong with a draft, said by the setting's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityProblem {
    /// A secret written by value (R9): never saved.
    LiteralSecret(String),
    /// The reCAPTCHA threshold is not a number from 0 to 1.
    Threshold,
}

use cobolt_server::identity_settings::{is_secret_reference, provider_settings, IdentityFlow, Provider};

const PROVIDERS: [Provider; 4] = [Provider::Cognito, Provider::Keycloak, Provider::Entra, Provider::EntraExternal];
const FLOWS: [IdentityFlow; 3] = [IdentityFlow::InApp, IdentityFlow::Hosted, IdentityFlow::Native];

impl IdentityDraft {
    fn provider(&self) -> Option<Provider> {
        Provider::parse(&self.provider)
    }

    /// The secret settings with their values: the provider's, the cookie key, reCAPTCHA's.
    fn secrets(&self) -> Vec<(String, &str)> {
        let mut out: Vec<(String, &str)> = Vec::new();
        if let Some(p) = self.provider() {
            for (name, _, secret) in provider_settings(p) {
                if *secret {
                    out.push((format!("identity.settings.{name}"), self.settings.get(*name).map(String::as_str).unwrap_or("")));
                }
            }
        }
        out.push(("identity.cookie_key".into(), self.cookie_key.as_str()));
        out.push(("identity.recaptcha.secret".into(), self.recaptcha_secret.as_str()));
        out
    }

    /// The draft's problems; a draft with any is not written.
    pub fn problems(&self) -> Vec<IdentityProblem> {
        if self.provider().is_none() {
            return Vec::new();
        }
        let mut out: Vec<IdentityProblem> = self
            .secrets()
            .into_iter()
            .filter(|(_, v)| !v.trim().is_empty() && !is_secret_reference(v))
            .map(|(name, _)| IdentityProblem::LiteralSecret(name))
            .collect();
        let score = self.min_score.trim();
        if !score.is_empty() && !score.parse::<f64>().is_ok_and(|v| (0.0..=1.0).contains(&v)) {
            out.push(IdentityProblem::Threshold);
        }
        out
    }
}

/// `[identity]` of the project's `backend.toml`; the default (no provider) when there is none.
pub fn read_identity(project_dir: &Path) -> IdentityDraft {
    let Ok(doc) = std::fs::read_to_string(backend_config_path(project_dir)).unwrap_or_default().parse::<toml_edit::DocumentMut>() else {
        return IdentityDraft::default();
    };
    let Some(id) = doc.get("identity").and_then(|i| i.as_table_like()) else { return IdentityDraft::default() };
    let text = |t: &dyn toml_edit::TableLike, k: &str| t.get(k).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let mut d = IdentityDraft { provider: text(id, "provider"), flow: text(id, "flow"), cookie_key: text(id, "cookie_key"), ..Default::default() };
    if d.flow == "auto" {
        d.flow.clear();
    }
    if let Some(settings) = id.get("settings").and_then(|s| s.as_table_like()) {
        for (k, v) in settings.iter() {
            if let Some(v) = v.as_str() {
                d.settings.insert(k.to_owned(), v.to_owned());
            }
        }
    }
    if let Some(origins) = id.get("origins").and_then(|o| o.as_array()) {
        d.origins_text = origins.iter().filter_map(|o| o.as_str()).collect::<Vec<_>>().join("\n");
    }
    if let Some(r) = id.get("recaptcha").and_then(|r| r.as_table_like()) {
        d.site_key = text(r, "site_key");
        d.recaptcha_secret = text(r, "secret");
        d.min_score = r.get("min_score").and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|f| f.to_string()).unwrap_or_default();
    }
    d
}

/// Write the draft as `[identity]` of `backend.toml`. Only that section changes: every other section,
/// and the comments around them, stay byte for byte; inside it, a key the page does not edit
/// (`attempts_per_minute`, `recaptcha.hostnames`) stays too. No provider removes the section. A draft
/// with a problem is refused (a secret by value is never written, R9). Returns whether the file changed.
pub fn write_identity(project_dir: &Path, d: &IdentityDraft) -> Result<bool, String> {
    if let Some(p) = d.problems().first() {
        return Err(match p {
            IdentityProblem::LiteralSecret(name) => format!("{name} holds a secret by value"),
            IdentityProblem::Threshold => "identity.recaptcha.min_score is not a number from 0 to 1".into(),
        });
    }
    let path = backend_config_path(project_dir);
    let before = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = before.as_deref().unwrap_or("").parse().map_err(|e| format!("{}: {e}", path.display()))?;
    match d.provider() {
        None => {
            doc.remove("identity");
        }
        Some(provider) => {
            if !doc.contains_key("identity") {
                doc.insert("identity", toml_edit::Item::Table(toml_edit::Table::new()));
            }
            let id = doc["identity"].as_table_mut().ok_or("[identity] is not a table")?;
            let set = |t: &mut toml_edit::Table, k: &str, v: &str| {
                if v.trim().is_empty() {
                    t.remove(k);
                } else if t.get(k).and_then(|x| x.as_str()) != Some(v.trim()) {
                    t.insert(k, toml_edit::value(v.trim()));
                }
            };
            set(id, "provider", provider.word());
            set(id, "flow", d.flow.trim());
            set(id, "cookie_key", &d.cookie_key);
            let origins: Vec<&str> = d.origins_text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            if origins.is_empty() {
                id.remove("origins");
            } else {
                let kept: Vec<&str> = id.get("origins").and_then(|o| o.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
                if kept != origins {
                    id.insert("origins", toml_edit::value(origins.iter().copied().collect::<toml_edit::Array>()));
                }
            }
            // The provider's settings: the ones it names, in its order; another provider's are dropped.
            let names: Vec<&str> = provider_settings(provider).iter().map(|(n, _, _)| *n).collect();
            if !id.contains_key("settings") {
                id.insert("settings", toml_edit::Item::Table(toml_edit::Table::new()));
            }
            let settings = id["settings"].as_table_mut().ok_or("[identity.settings] is not a table")?;
            let stale: Vec<String> = settings.iter().map(|(k, _)| k.to_owned()).filter(|k| !names.contains(&k.as_str())).collect();
            for k in stale {
                settings.remove(&k);
            }
            for n in &names {
                set(settings, n, d.settings.get(*n).map(String::as_str).unwrap_or(""));
            }
            let recaptcha_given = [&d.site_key, &d.recaptcha_secret, &d.min_score].iter().any(|v| !v.trim().is_empty());
            if recaptcha_given {
                if !id.contains_key("recaptcha") {
                    id.insert("recaptcha", toml_edit::Item::Table(toml_edit::Table::new()));
                }
                let r = id["recaptcha"].as_table_mut().ok_or("[identity.recaptcha] is not a table")?;
                set(r, "site_key", &d.site_key);
                set(r, "secret", &d.recaptcha_secret);
                match d.min_score.trim().parse::<f64>() {
                    Ok(v) if r.get("min_score").and_then(|x| x.as_float()) != Some(v) => {
                        r.insert("min_score", toml_edit::value(v));
                    }
                    Ok(_) => {}
                    Err(_) => {
                        r.remove("min_score");
                    }
                }
            } else {
                id.remove("recaptcha");
            }
        }
    }
    let after = doc.to_string();
    if before.as_deref().unwrap_or("") == after {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, after).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// The Identity section's Test connection (T18's `rcrun backend auth test`): running on a thread,
/// or the lines it printed last.
#[derive(Default)]
pub struct IdentityTest {
    running: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    lines: Vec<String>,
}

impl IdentityTest {
    /// Start `rcrun backend auth test <project_dir>`; its output (stdout, then stderr) comes back whole.
    fn start(&mut self, ctx: &egui::Context, project_dir: &Path) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (dir, ctx) = (project_dir.to_path_buf(), ctx.clone());
        std::thread::spawn(move || {
            let lines = match std::process::Command::new(crate::backend_run::rcrun()).args(["backend", "auth", "test"]).arg(&dir).output() {
                Ok(o) => String::from_utf8_lossy(&o.stdout).lines().chain(String::from_utf8_lossy(&o.stderr).lines()).map(str::to_owned).collect(),
                Err(e) => vec![format!("rcrun: {e}")],
            };
            let _ = tx.send(lines);
            ctx.request_repaint();
        });
        self.running = Some(rx);
        self.lines.clear();
    }

    fn poll(&mut self) {
        if let Some(rx) = &self.running {
            if let Ok(lines) = rx.try_recv() {
                self.lines = lines;
                self.running = None;
            }
        }
    }
}

fn flow_text<'a>(tr: &'a Tr, flow: &str) -> &'a str {
    match flow {
        "in-app" => tr.pk_set_id_flow_in_app,
        "hosted" => tr.pk_set_id_flow_hosted,
        "native" => tr.pk_set_id_flow_native,
        _ => tr.pk_set_id_flow_auto,
    }
}

/// A secret's box shows what it holds (a reference, never a secret) but hides a value typed by
/// mistake, so it is not shown on screen while the page refuses it.
fn secret_box(ui: &mut Ui, value: &mut String) {
    let w = ui.available_width();
    let hide = !value.trim().is_empty() && !is_secret_reference(value);
    ui.add(egui::TextEdit::singleline(value).hint_text("env:NAME").password(hide).desired_width(w));
}

/// The Identity rows (WASM-009 T19).
fn identity_rows(ui: &mut Ui, d: &mut IdentityDraft, tr: &Tr, theme: &Theme, lay: RowLayout, project_dir: Option<&Path>, test: &mut IdentityTest) {
    let dim = theme.text_dim;
    ui.add_space(8.0);
    row(ui, lay, "", |ui| {
        section(ui, tr.pk_set_sec_identity, theme);
    });
    row(ui, lay, tr.pk_set_id_provider, |ui| {
        let shown = Provider::parse(&d.provider).map_or(tr.pk_set_id_provider_none, |p| p.word());
        egui::ComboBox::from_id_salt("identity_provider").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut d.provider, String::new(), tr.pk_set_id_provider_none);
            for p in PROVIDERS {
                ui.selectable_value(&mut d.provider, p.word().to_owned(), p.word());
            }
        });
    });
    let Some(provider) = Provider::parse(&d.provider) else { return };
    row(ui, lay, tr.pk_set_id_flow, |ui| {
        egui::ComboBox::from_id_salt("identity_flow").selected_text(flow_text(tr, &d.flow)).show_ui(ui, |ui| {
            ui.selectable_value(&mut d.flow, String::new(), tr.pk_set_id_flow_auto);
            for f in FLOWS {
                ui.selectable_value(&mut d.flow, f.word().to_owned(), flow_text(tr, f.word()));
            }
        });
    });
    for (name, _required, secret) in provider_settings(provider) {
        let value = d.settings.entry((*name).to_owned()).or_default();
        row(ui, lay, name, |ui| {
            if *secret {
                secret_box(ui, value);
            } else {
                let w = ui.available_width();
                ui.add(egui::TextEdit::singleline(value).desired_width(w));
            }
        });
    }
    note(ui, lay, tr.pk_set_id_settings_hint, dim);
    row(ui, lay, tr.pk_set_id_origins, |ui| {
        let w = ui.available_width();
        ui.add(egui::TextEdit::multiline(&mut d.origins_text).desired_rows(2).desired_width(w));
    });
    note(ui, lay, tr.pk_set_id_origins_hint, dim);
    row(ui, lay, tr.pk_set_id_cookie_key, |ui| secret_box(ui, &mut d.cookie_key));
    note(ui, lay, tr.pk_set_id_cookie_key_hint, dim);
    row(ui, lay, tr.pk_set_id_recaptcha_site_key, |ui| {
        let w = ui.available_width();
        ui.add(egui::TextEdit::singleline(&mut d.site_key).desired_width(w));
    });
    row(ui, lay, tr.pk_set_id_recaptcha_secret, |ui| secret_box(ui, &mut d.recaptcha_secret));
    row(ui, lay, tr.pk_set_id_recaptcha_threshold, |ui| {
        ui.add(egui::TextEdit::singleline(&mut d.min_score).hint_text("0.5").desired_width(60.0));
    });
    note(ui, lay, tr.pk_set_id_secret_hint, dim);
    for p in d.problems() {
        let text = match p {
            IdentityProblem::LiteralSecret(name) => tr.pk_set_id_literal_secret.replacen("{}", &name, 1),
            IdentityProblem::Threshold => tr.pk_set_id_recaptcha_threshold_bad.to_owned(),
        };
        note(ui, lay, &text, theme.error);
    }
    test.poll();
    row(ui, lay, "", |ui| {
        let running = test.running.is_some();
        let label = if running { tr.pk_set_id_testing } else { tr.pk_set_id_test };
        if ui.add_enabled(!running && project_dir.is_some(), egui::Button::new(label)).clicked() {
            if let Some(dir) = project_dir {
                test.start(ui.ctx(), dir);
            }
        }
    });
    note(ui, lay, tr.pk_set_id_test_hint, dim);
    for line in &test.lines {
        let colour = if line.starts_with("FAIL") { theme.error } else { dim };
        row(ui, lay, "", |ui| {
            ui.add(egui::Label::new(RichText::new(line).monospace().small().color(colour)).wrap());
        });
    }
}

// ── The Preview's browser (WASM-008 R12, Q7, D11) ───────────────────────────

/// One answer to *Start "<path>" to show the Preview?*
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserConfirmation {
    /// The project folder, as the system names it (canonical).
    pub project: String,
    /// The application path the project names.
    pub path: String,
    pub allowed: bool,
}

/// The answers this machine's developer gave, kept in
/// `preview_browsers.toml` under the IDE's per-user folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserConfirmations {
    #[serde(default, rename = "confirmation")]
    pub entries: Vec<BrowserConfirmation>,
}

/// Where the answers are kept: beside `debug_settings.toml`.
pub fn confirmations_path() -> PathBuf {
    crate::llm::base_dir().join("preview_browsers.toml")
}

fn folder_key(project_dir: &Path) -> String {
    std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf()).to_string_lossy().into_owned()
}

impl BrowserConfirmations {
    /// The answers in `file`; none when it is missing or unreadable.
    pub fn load_from(file: &Path) -> Self {
        std::fs::read_to_string(file).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Write the answers to `file` (best effort, as the IDE's other settings).
    pub fn save_to(&self, file: &Path) {
        if let Ok(text) = toml::to_string_pretty(self) {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, text);
        }
    }

    /// The answer for `path` in the project at `project_dir`: `None` when the
    /// developer has not been asked here (another folder is another question).
    pub fn answer(&self, project_dir: &Path, path: &Path) -> Option<bool> {
        let (project, path) = (folder_key(project_dir), path.to_string_lossy());
        self.entries.iter().find(|e| e.project == project && e.path == path).map(|e| e.allowed)
    }

    /// Keep the answer for `path` in the project at `project_dir`.
    pub fn record(&mut self, project_dir: &Path, path: &Path, allowed: bool) {
        let (project, path) = (folder_key(project_dir), path.to_string_lossy().into_owned());
        self.entries.retain(|e| !(e.project == project && e.path == path));
        self.entries.push(BrowserConfirmation { project, path, allowed });
    }
}

/// The known browsers on this machine, looked up once (the lookup reads the
/// file system, and on Windows asks the registry).
pub fn present_browsers() -> &'static [KnownBrowser] {
    static PRESENT: std::sync::OnceLock<Vec<KnownBrowser>> = std::sync::OnceLock::new();
    PRESENT.get_or_init(|| KnownBrowser::ALL.into_iter().filter(|b| b.is_present()).collect())
}

/// The setting as the row shows it.
fn browser_text(tr: &Tr, setting: &str) -> String {
    match BrowserChoice::parse(setting) {
        BrowserChoice::System => tr.pk_set_browser_system.to_owned(),
        BrowserChoice::Named(b) => b.name().to_owned(),
        BrowserChoice::Path(_) => tr.pk_set_browser_other.to_owned(),
    }
}

/// The row of the Preview's browser: `setting` is `[web]` or `[mobile]`
/// `preview_browser`, changed only by what the developer picks or types.
fn browser_row(ui: &mut Ui, lay: RowLayout, tr: &Tr, setting: &mut String) {
    let other_id = egui::Id::new("kind_preview_browser_other");
    let mut other = ui.data(|d| d.get_temp::<bool>(other_id)).unwrap_or(false) || matches!(BrowserChoice::parse(setting), BrowserChoice::Path(_));
    row(ui, lay, tr.pk_set_preview_browser, |ui| {
        let shown = if other { tr.pk_set_browser_other.to_owned() } else { browser_text(tr, setting) };
        egui::ComboBox::from_id_salt("kind_preview_browser").selected_text(shown).show_ui(ui, |ui| {
            if ui.selectable_label(!other && setting.is_empty(), tr.pk_set_browser_system).clicked() {
                setting.clear();
                other = false;
            }
            let mut listed: Vec<KnownBrowser> = present_browsers().to_vec();
            // A browser the project names that is not here is still shown, so it can be seen and changed.
            if let BrowserChoice::Named(b) = BrowserChoice::parse(setting) {
                if !listed.contains(&b) {
                    listed.push(b);
                }
            }
            for b in listed {
                let on = !other && BrowserChoice::parse(setting) == BrowserChoice::Named(b);
                if ui.selectable_label(on, b.name()).clicked() {
                    *setting = b.id().to_owned();
                    other = false;
                }
            }
            if ui.selectable_label(other, tr.pk_set_browser_other).clicked() && !other {
                other = true;
                if !matches!(BrowserChoice::parse(setting), BrowserChoice::Path(_)) {
                    setting.clear();
                }
            }
        });
    });
    if other {
        row(ui, lay, "", |ui| {
            let w = ui.available_width();
            ui.add(egui::TextEdit::singleline(setting).hint_text(BROWSER_PATH_HINT).desired_width(w));
        });
    }
    ui.data_mut(|d| d.insert_temp(other_id, other));
}

/// What the path box shows while it is empty: the shape of a path.
const BROWSER_PATH_HINT: &str = "/\u{2026}";

/// The outer width of the path confirmation.
pub const CONFIRM_W: f32 = 460.0;

/// *Start "<path>" to show the Preview?* — a dialog of one width
/// ([`crate::panels::fixed_dialog`]). `Some(answer)` once the developer
/// answers; closing it is a no.
pub fn show_browser_confirm(ctx: &egui::Context, tr: &Tr, path: &str) -> Option<bool> {
    let mut open = true;
    let mut answer = None;
    crate::panels::fixed_dialog::show(ctx, tr.pk_prev_confirm_title, CONFIRM_W, &mut open, |ui| {
        ui.add(egui::Label::new(tr.pk_prev_confirm_text.replacen("{}", path, 1)).wrap());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(tr.pk_prev_confirm_yes).clicked() {
                answer = Some(true);
            }
            if ui.button(tr.pk_prev_confirm_no).clicked() {
                answer = Some(false);
            }
        });
    });
    if !open {
        answer = Some(false);
    }
    answer
}

/// Draw the section for a Web or Mobile project; nothing for a Desktop one.
/// A Mobile project's Application id and its native project (WASM-012 R6, R17): whether the
/// Android Studio project is there and where, with the button that makes it (again); an iOS
/// project's Xcode project, unavailable, with the reason.
fn mobile_identity_rows(
    ui: &mut Ui,
    draft: &mut SettingsDraft,
    tr: &Tr,
    theme: &Theme,
    lay: RowLayout,
    project_dir: Option<&Path>,
    jobs: &mut PageJobs,
) {
    use crate::panels::native_project as np;
    let PageJobs { native: job, studio, .. } = jobs;
    use cobolt_compiler::mobile_native as mn;
    let dim = theme.text_dim;
    let version = format!("{}.{}.{}", draft.ver_major, draft.ver_minor, draft.ver_fix);
    let name = draft.name.clone();
    let Some(m) = &mut draft.mobile else {
        return;
    };
    row(ui, lay, tr.pk_set_app_id, |ui| {
        let w = ui.available_width();
        ui.add(egui::TextEdit::singleline(&mut m.app_id).hint_text("com.company.orders").desired_width(w));
    });
    let id_ok = cobolt_compiler::project_kind::validate_app_id(&m.app_id).is_ok();
    if id_ok {
        note(ui, lay, tr.pk_set_app_id_hint, dim);
    } else {
        note(ui, lay, tr.pk_set_app_id_bad, theme.error);
    }
    // A project made while the page is open is recorded in the draft: Save writes `[mobile.native]`.
    if job.poll().is_some_and(|o| o.is_ready()) {
        np::record_android(m);
    }
    match m.platform {
        MobilePlatform::Android => {
            let Some(dir) = project_dir else {
                return;
            };
            let exists = mn::android_exists(dir);
            let mut studio = studio.borrow_mut();
            let state = studio.state(dir);
            row(ui, lay, tr.pk_set_native_android, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let label = if exists { tr.pk_native_recreate } else { tr.pk_native_create };
                    // Not made again under the Tauri command line Android Studio builds through.
                    ui.add_enabled_ui(!job.running() && id_ok && !state.is_live(), |ui| {
                        if ui.button(label).clicked() {
                            job.start(ui.ctx(), np::request_from(dir, &name, &version, m.clone()));
                        }
                    });
                    if state.is_live() {
                        if ui.button(tr.pk_native_stop).clicked() {
                            studio.stop();
                        }
                    } else if exists {
                        ui.add_enabled_ui(!job.running() && id_ok, |ui| {
                            if ui.button(tr.pk_native_open).clicked() {
                                studio.open(ui.ctx(), np::request_from(dir, &name, &version, m.clone()));
                            }
                        });
                    }
                });
            });
            let (text, colour) = match &job.last {
                _ if job.running() => (tr.pk_native_working.to_owned(), dim),
                Some(o) => (o.text(tr), if o.is_ready() { dim } else { theme.error }),
                None if exists => (tr.pk_set_native_present.replacen("{}", &mn::android_studio_dir(dir).to_string_lossy(), 1), dim),
                None => (tr.pk_set_native_missing.replacen("{}", &mn::android_root(dir).to_string_lossy(), 1), dim),
            };
            note(ui, lay, &text, colour);
            let log = studio.log().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            if let Some(text) = state.text(tr, &log) {
                note(ui, lay, &text, if state.is_problem() { theme.error } else { dim });
            }
        }
        MobilePlatform::Ios => {
            row(ui, lay, tr.pk_set_native_xcode, |ui| {
                ui.add(egui::Label::new(RichText::new(tr.pk_set_native_xcode_unavailable).color(dim)).wrap());
            });
        }
    }
}

/// The page's work on threads of its own: the Identity section's Test connection, the making
/// of the Android Studio project, and the app's Open in Android Studio (lent: it outlives the page).
#[derive(Default)]
pub struct PageJobs {
    pub identity_test: IdentityTest,
    pub native: crate::panels::native_project::NativeJob,
    pub studio: crate::panels::native_project::SharedStudio,
}

pub fn show(ui: &mut Ui, draft: &mut SettingsDraft, tr: &Tr, theme: &Theme, lay: RowLayout, project_dir: Option<&Path>, jobs: &mut PageJobs) {
    if draft.kind.is_desktop() {
        return;
    }
    let dim = theme.text_dim;
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        let left_rect = ui.allocate_exact_size(egui::vec2(lay.splitter, 0.0), egui::Sense::hover()).0;
        ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.set_min_width(lay.splitter);
            section(ui, if draft.kind == ProjectKind::Web { tr.pk_set_sec_web } else { tr.pk_set_sec_mobile }, theme);
        });
        ui.allocate_space(egui::vec2(lay.resizer_width, 0.0));
        ui.add_space(lay.gap_after_resizer);
        let right_w = ui.available_width();
        ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
    });

    // What the project is: fixed at creation.
    row(ui, lay, tr.pk_set_kind, |ui| {
        let kind = if draft.kind == ProjectKind::Web { tr.pk_kind_web } else { tr.pk_kind_mobile };
        ui.add(egui::Label::new(RichText::new(kind).strong()).truncate());
    });
    note(ui, lay, tr.pk_set_kind_fixed, dim);

    if let Some(m) = &draft.mobile {
        let platform = match m.platform {
            MobilePlatform::Android => "Android",
            MobilePlatform::Ios => "iOS",
        };
        row(ui, lay, tr.pk_set_platform, |ui| {
            ui.label(platform);
        });
        let device = match m.device {
            cobolt_compiler::project_kind::DeviceClass::Phone => tr.pk_set_device_phone,
            cobolt_compiler::project_kind::DeviceClass::Tablet => tr.pk_set_device_tablet,
        };
        row(ui, lay, tr.pk_set_device, |ui| {
            ui.label(device);
        });
    }
    mobile_identity_rows(ui, draft, tr, theme, lay, project_dir, jobs);

    row(ui, lay, tr.pk_f_description, |ui| {
        let w = ui.available_width();
        ui.add(egui::TextEdit::singleline(&mut draft.description).desired_width(w));
    });
    row(ui, lay, tr.pk_f_locale, |ui| {
        let shown = if draft.locale.is_empty() { tr.pk_locale_none.to_owned() } else { crate::panels::new_project::locale_label(&draft.locale) };
        egui::ComboBox::from_id_salt("kind_locale").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut draft.locale, String::new(), tr.pk_locale_none);
            for (tag, _) in LOCALES {
                ui.selectable_value(&mut draft.locale, tag.to_owned(), crate::panels::new_project::locale_label(tag));
            }
        });
    });

    row(ui, lay, tr.pk_f_currency, |ui| {
        ui.add(egui::TextEdit::singleline(&mut draft.currency).desired_width(80.0).char_limit(3));
    });

    // The application style is the project's, Web or Mobile.
    let mut style = match draft.kind {
        ProjectKind::Web => draft.web.style,
        _ => draft.mobile.as_ref().map(|m| m.style).unwrap_or_default(),
    };
    row(ui, lay, tr.pk_o_style, |ui| {
        egui::ComboBox::from_id_salt("kind_style").selected_text(style_text(tr, style)).show_ui(ui, |ui| {
            for s in [ApplicationStyle::Spa, ApplicationStyle::Standalone] {
                ui.selectable_value(&mut style, s, style_text(tr, s));
            }
        });
    });
    match draft.kind {
        ProjectKind::Web => draft.web.style = style,
        _ => {
            if let Some(m) = &mut draft.mobile {
                m.style = style;
            }
        }
    }

    // What a control whose LoadingEffect property is on shows while a call to the Backend is in flight (2.0.413): the
    // project's, Web or Mobile. The property chooses WHICH controls show it; this chooses WHAT they show.
    let mut effect = match draft.kind {
        ProjectKind::Web => draft.web.loading_effect,
        _ => draft.mobile.as_ref().map(|m| m.loading_effect).unwrap_or_default(),
    };
    row(ui, lay, tr.pk_o_loading_effect, |ui| {
        egui::ComboBox::from_id_salt("kind_loading_effect").selected_text(loading_effect_text(tr, effect)).show_ui(ui, |ui| {
            for e in LoadingEffect::ALL {
                ui.selectable_value(&mut effect, e, loading_effect_text(tr, e));
            }
        });
    });
    note(ui, lay, tr.pk_o_loading_effect_note, dim);
    match draft.kind {
        ProjectKind::Web => draft.web.loading_effect = effect,
        _ => {
            if let Some(m) = &mut draft.mobile {
                m.loading_effect = effect;
            }
        }
    }

    if draft.kind == ProjectKind::Web {
        row(ui, lay, tr.pk_set_browsers, |ui| {
            ui.vertical(|ui| {
                let b = &mut draft.web.browsers;
                ui.checkbox(&mut b.chromium, tr.pk_set_browser_chromium);
                ui.checkbox(&mut b.firefox, tr.pk_set_browser_firefox);
                ui.checkbox(&mut b.safari, tr.pk_set_browser_safari);
                ui.checkbox(&mut b.ios_webview, tr.pk_set_browser_ios);
                ui.checkbox(&mut b.android_webview, tr.pk_set_browser_android);
            });
        });
    }

    if let Some(m) = &mut draft.mobile {
        row(ui, lay, tr.pk_o_orientation, |ui| {
            egui::ComboBox::from_id_salt("kind_orientation").selected_text(orientation_text(tr, m.orientation)).show_ui(ui, |ui| {
                for o in [Orientation::Auto, Orientation::Portrait, Orientation::Landscape] {
                    ui.selectable_value(&mut m.orientation, o, orientation_text(tr, o));
                }
            });
        });
        let choices = min_os_choices(m.platform);
        row(ui, lay, tr.pk_o_min_os, |ui| {
            let shown = choices.iter().find(|c| c.id == m.min_os).map_or(m.min_os.as_str(), |c| choice_label(tr, "min_os", c)).to_owned();
            egui::ComboBox::from_id_salt("kind_min_os").selected_text(shown).show_ui(ui, |ui| {
                for c in choices {
                    ui.selectable_value(&mut m.min_os, c.id.to_owned(), choice_label(tr, "min_os", c));
                }
            });
        });
    }

    // Multi-memory, and what the bundle will therefore carry.
    let variants = match draft.kind {
        ProjectKind::Web => draft.web.module_variants(),
        _ => draft.mobile.as_ref().map(|m| m.module_variants()).unwrap_or(ModuleVariants { multi_memory: true, single_memory: false }),
    };
    row(ui, lay, tr.pk_set_multi_memory, |ui| {
        let multi = match draft.kind {
            ProjectKind::Web => &mut draft.web.multi_memory,
            _ => match &mut draft.mobile {
                Some(m) => &mut m.multi_memory,
                None => return,
            },
        };
        ui.checkbox(multi, "");
    });
    note(ui, lay, tr.pk_set_multi_memory_hint, dim);
    // WASM-017 R9, D5 — "multi-memory only" is the fallback turned off; with
    // WebKit in the matrix the Build refuses it, and so says the panel.
    if draft.kind == ProjectKind::Web {
        row(ui, lay, tr.pk_set_single_memory_fallback, |ui| {
            ui.checkbox(&mut draft.web.single_memory_fallback, "");
        });
        note(ui, lay, tr.pk_set_single_memory_fallback_hint, dim);
        if let Some(p) = draft.web.variant_problem() {
            note(ui, lay, &tr.pk_err_multi_memory_only_with_webkit.replacen("{}", &p.browsers.join(", "), 1), theme.error);
        }
    }
    note(ui, lay, &tr.pk_set_variants.replacen("{}", &variants_text(tr, variants), 1), dim);
    // WASM-017 R8 — one payload limit for the module, the player and the Backend.
    let payload = match draft.kind {
        ProjectKind::Web => Some(&mut draft.web.max_payload_bytes),
        _ => draft.mobile.as_mut().map(|m| &mut m.max_payload_bytes),
    };
    if let Some(bytes) = payload {
        row(ui, lay, tr.pk_set_max_payload, |ui| {
            let mut kb = *bytes / 1024;
            if ui.add(egui::DragValue::new(&mut kb).range(64..=16_384).speed(16)).changed() {
                *bytes = kb * 1024;
            }
        });
        note(ui, lay, tr.pk_set_max_payload_hint, dim);
    }

    // The browser of the Preview and Run (WASM-008 R12); a Mobile project's fallback (R15).
    match draft.kind {
        ProjectKind::Web => browser_row(ui, lay, tr, &mut draft.web.preview_browser),
        _ => {
            if let Some(m) = &mut draft.mobile {
                browser_row(ui, lay, tr, &mut m.preview_browser);
            }
        }
    }
    note(ui, lay, tr.pk_set_preview_browser_hint, dim);

    if draft.kind == ProjectKind::Web {
        row(ui, lay, tr.pk_set_isolation, |ui| {
            egui::ComboBox::from_id_salt("kind_isolation").selected_text(isolation_text(tr, draft.web.isolation)).show_ui(ui, |ui| {
                for i in [Isolation::RequireCorp, Isolation::Credentialless, Isolation::Off] {
                    ui.selectable_value(&mut draft.web.isolation, i, isolation_text(tr, i));
                }
            });
        });
        note(ui, lay, tr.pk_set_iso_hint, dim);
    }

    // The `unsafe` settings that are on, marked (WASM-013 R11): a release build refuses each.
    let table = if draft.kind == ProjectKind::Web { "web" } else { "mobile" };
    let unsafe_keys = match (draft.kind, &draft.mobile) {
        (ProjectKind::Web, _) => draft.web.unsafe_.enabled(table),
        (_, Some(m)) => m.unsafe_.enabled(table),
        _ => Vec::new(),
    };
    for key in unsafe_keys {
        note(ui, lay, &tr.pk_unsafe_on.replacen("{}", &key, 1), theme.warn);
    }

    // The Backend's address.
    let url = match draft.kind {
        ProjectKind::Web => &mut draft.web.backend_url,
        _ => match &mut draft.mobile {
            Some(m) => &mut m.backend_url,
            None => return,
        },
    };
    row(ui, lay, tr.pk_set_backend_url, |ui| {
        let w = ui.available_width();
        ui.add(egui::TextEdit::singleline(url).hint_text(BACKEND_URL_HINT).desired_width(w));
    });
    match backend_url_problem(url) {
        Some(p) => note(ui, lay, url_problem_text(tr, p), theme.error),
        None => note(ui, lay, tr.pk_set_backend_url_hint, dim),
    }

    // The Backend's exported programs.
    if draft.has_backend {
        row(ui, lay, tr.pk_set_exports, |ui| {
            let w = ui.available_width();
            ui.add(egui::TextEdit::multiline(&mut draft.exports_text).desired_rows(3).desired_width(w));
        });
        note(ui, lay, tr.pk_set_exports_hint, dim);
        row(ui, lay, tr.pk_set_state, |ui| {
            egui::ComboBox::from_id_salt("kind_backend_state").selected_text(state_text(tr, &draft.backend_state)).show_ui(ui, |ui| {
                for s in [STATE_PER_CALL, STATE_PER_CONNECTION] {
                    ui.selectable_value(&mut draft.backend_state, s.to_owned(), state_text(tr, s));
                }
            });
        });
        note(ui, lay, tr.pk_set_state_hint, dim);
        row(ui, lay, tr.pk_set_backend_port, |ui| {
            let w = ui.available_width();
            ui.add(egui::TextEdit::singleline(&mut draft.backend_port).hint_text("8443").desired_width(w));
        });
        if backend_port_ok(&draft.backend_port) {
            note(ui, lay, tr.pk_set_backend_port_hint, dim);
        } else {
            note(ui, lay, tr.pk_set_backend_port_bad, theme.error);
        }
        row(ui, lay, tr.pk_set_store, |ui| {
            egui::ComboBox::from_id_salt("kind_contract_store")
                .selected_text(if draft.contract.store == STORE_SERVER { tr.pk_set_store_server } else { tr.pk_set_store_memory })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.contract.store, STORE_MEMORY.to_owned(), tr.pk_set_store_memory);
                    ui.selectable_value(&mut draft.contract.store, STORE_SERVER.to_owned(), tr.pk_set_store_server);
                });
        });
        row(ui, lay, tr.pk_set_idle_seconds, |ui| {
            let w = ui.available_width();
            ui.add(egui::TextEdit::singleline(&mut draft.contract.idle_seconds).hint_text("900").desired_width(w));
        });
        if draft.contract.idle_ok() {
            note(ui, lay, tr.pk_set_contract_hint, dim);
        } else {
            note(ui, lay, tr.pk_set_idle_bad, theme.error);
        }
        identity_rows(ui, &mut draft.identity, tr, theme, lay, project_dir, &mut jobs.identity_test);
    } else {
        note(ui, lay, tr.pk_set_no_backend, dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::llm::LlmConfig;
    use cobolt_compiler::project_kind::{BrowserMatrix, DeviceClass, MobileSettings};

    fn web_project() -> CoboltProject {
        let mut p = CoboltProject::new("W.project", "");
        p.project.kind = ProjectKind::Web;
        p.web = Some(Default::default());
        p.backend = Some(Default::default());
        p.backend_settings_mut().exports = vec!["HEALTH".into()];
        p
    }

    fn mobile_project(platform: MobilePlatform) -> CoboltProject {
        let mut p = CoboltProject::new("M.project", "");
        p.project.kind = ProjectKind::Mobile;
        p.mobile = Some(MobileSettings::new(platform, DeviceClass::Phone));
        p
    }

    #[test]
    fn the_exports_are_one_name_per_line_without_blanks_or_repeats() {
        assert_eq!(parse_exports(" HEALTH \n\nPING\nHEALTH\n  \n"), ["HEALTH", "PING"]);
        assert!(parse_exports("").is_empty());
        assert_eq!(invalid_export_names("GOOD-1\n9BAD\nbad name\nOK"), ["9BAD", "bad name"]);
    }

    /// WASM-017 R9 in words: the line under the multi-memory option.
    #[test]
    fn the_variants_line_names_the_modules_the_bundle_carries() {
        let tr = Language::English.tr();
        assert_eq!(variants_text(&tr, ModuleVariants { multi_memory: true, single_memory: true }), "multi-memory module, single-memory module");
        assert_eq!(variants_text(&tr, ModuleVariants { multi_memory: true, single_memory: false }), "multi-memory module");
        assert_eq!(variants_text(&tr, ModuleVariants { multi_memory: false, single_memory: true }), "single-memory module");
        // And the project's own rule decides them.
        let mut p = web_project();
        let mut draft = SettingsDraft::from_project(&p, &LlmConfig::load_defaults_for_test());
        assert_eq!(draft.web.module_variants().count(), 2, "the default matrix includes Safari");
        draft.web.browsers = BrowserMatrix { chromium: true, firefox: true, safari: false, ios_webview: false, android_webview: true };
        assert_eq!(draft.web.module_variants().count(), 2, "WASM-017 D5: the fallback is on by default");
        draft.web.single_memory_fallback = false;
        assert_eq!(draft.web.module_variants().count(), 1, "fallback off, no WebKit: multi-memory only");
        draft.apply_kind(&mut p);
        assert!(!p.web.as_ref().unwrap().browsers.safari);
        assert!(!p.web.as_ref().unwrap().single_memory_fallback);
    }

    /// 2.0.413: the loading effect is a project setting, Web and Mobile; the draft reads it, the combobox names every effect in
    /// all six languages, and a chosen effect is written back (the default is not written, so an old project is saved as it was).
    #[test]
    fn the_loading_effect_setting_round_trips_and_is_named_in_six_languages() {
        let llm = LlmConfig::load_defaults_for_test();
        let mut web = web_project();
        let mut draft = SettingsDraft::from_project(&web, &llm);
        assert_eq!(draft.web.loading_effect, LoadingEffect::MovingStripes, "a project that never chose has the stripes");
        draft.web.loading_effect = LoadingEffect::None;
        draft.apply_kind(&mut web);
        assert_eq!(web.web.as_ref().unwrap().loading_effect, LoadingEffect::None);
        assert!(toml::to_string(&web.web).unwrap().contains("loading_effect = \"none\""), "a chosen effect is written");

        let mut mobile = mobile_project(MobilePlatform::Ios);
        let mut draft = SettingsDraft::from_project(&mobile, &llm);
        assert_eq!(draft.mobile.as_ref().unwrap().loading_effect, LoadingEffect::MovingStripes);
        draft.mobile.as_mut().unwrap().loading_effect = LoadingEffect::None;
        draft.apply_kind(&mut mobile);
        assert_eq!(mobile.mobile.as_ref().unwrap().loading_effect, LoadingEffect::None);

        let mut named = 0;
        for &lang in Language::ALL {
            let tr = lang.tr();
            let labels: Vec<&str> = LoadingEffect::ALL.iter().map(|&e| loading_effect_text(&tr, e)).collect();
            assert_ne!(labels[0], labels[1], "{lang:?}: the two effects read differently");
            for t in labels.into_iter().chain([tr.pk_o_loading_effect, tr.pk_o_loading_effect_note]) {
                assert!(!t.trim().is_empty() && !t.to_lowercase().contains("cobolt"), "{lang:?}: {t:?}");
            }
            named += 1;
        }
        println!("loading effect setting: Web and Mobile round-trip; {} effects named in {named} languages", LoadingEffect::ALL.len());
    }

    /// WASM-017 T7: "multi-memory only" with Safari in the matrix is said in
    /// the panel in every language, naming the browsers, and the payload limit
    /// is written back.
    #[test]
    fn multi_memory_only_with_webkit_is_said_in_six_languages_017() {
        let mut p = web_project();
        let mut draft = SettingsDraft::from_project(&p, &LlmConfig::load_defaults_for_test());
        assert!(draft.web.single_memory_fallback && draft.web.variant_problem().is_none());
        draft.web.single_memory_fallback = false;
        let problem = draft.web.variant_problem().expect("Safari is in the default matrix");
        let mut said = 0;
        for &lang in Language::ALL {
            let tr = lang.tr();
            for t in [tr.pk_set_single_memory_fallback, tr.pk_set_single_memory_fallback_hint, tr.pk_set_max_payload, tr.pk_set_max_payload_hint] {
                assert!(!t.trim().is_empty() && !t.to_lowercase().contains("cobolt"), "{lang:?}: {t:?}");
            }
            let text = tr.pk_err_multi_memory_only_with_webkit.replacen("{}", &problem.browsers.join(", "), 1);
            assert!(text.contains("Safari") && text.contains("iOS") && !text.contains("{}"), "{lang:?}: {text}");
            said += 1;
        }
        draft.web.max_payload_bytes = 512 * 1024;
        draft.apply_kind(&mut p);
        assert_eq!(p.web.as_ref().unwrap().max_payload_bytes, 512 * 1024);
        println!("WASM-017 T7: the refusal said in {said} languages naming {:?}; payload limit written back", problem.browsers);
    }

    /// WASM-001/002: the section reads the project and writes it back, and
    /// a Desktop project's settings are not touched by it.
    #[test]
    fn the_draft_round_trips_a_web_project_and_leaves_a_desktop_one_alone() {
        let mut p = web_project();
        let llm = LlmConfig::load_defaults_for_test();
        let mut draft = SettingsDraft::from_project(&p, &llm);
        assert_eq!(draft.kind, ProjectKind::Web);
        assert!(draft.has_backend);
        assert_eq!(draft.exports_text, "", "the box is filled from backend/exports.toml by the caller, never from the project file");
        assert_eq!(draft.backend_state, STATE_PER_CALL);
        draft.web.style = ApplicationStyle::Standalone;
        draft.web.multi_memory = false;
        draft.web.isolation = Isolation::Credentialless;
        draft.web.backend_url = "wss://api.example.com/ws".into();
        draft.backend_state = STATE_PER_CONNECTION.into();
        draft.description = "A shop".into();
        draft.locale = "fr-FR".into();
        draft.apply_kind(&mut p);
        let web = p.web.clone().unwrap();
        assert_eq!((web.style, web.multi_memory, web.isolation), (ApplicationStyle::Standalone, false, Isolation::Credentialless));
        assert_eq!(web.backend_url, "wss://api.example.com/ws");
        assert_eq!(p.backend.as_ref().unwrap().state, STATE_PER_CONNECTION);
        assert_eq!(p.backend.as_ref().unwrap().exports, ["HEALTH"], "the retired list is neither read nor rewritten (WASM-003 D5)");
        assert_eq!((p.project.description.as_str(), p.project.locale.as_str()), ("A shop", "fr-FR"));

        let mut desktop = CoboltProject::new("D.project", "src/main.cbl");
        let draft = SettingsDraft::from_project(&desktop, &llm);
        draft.apply_kind(&mut desktop);
        assert!(desktop.web.is_none() && desktop.mobile.is_none() && desktop.backend.is_none(), "a Desktop project gains no table");
    }

    /// WASM-003 R5: an address the channel refuses is never written; the last
    /// good one stays.
    #[test]
    fn a_refused_backend_address_is_not_written() {
        let mut p = web_project();
        p.web_settings_mut().backend_url = "wss://good.example.com".into();
        let mut draft = SettingsDraft::from_project(&p, &LlmConfig::load_defaults_for_test());
        for bad in ["ws://example.com", "http://example.com", "wss://"] {
            draft.web.backend_url = bad.into();
            draft.apply_kind(&mut p);
            assert_eq!(p.web.as_ref().unwrap().backend_url, "wss://good.example.com", "{bad} was written");
        }
        draft.web.backend_url = "ws://127.0.0.1:8750".into();
        draft.apply_kind(&mut p);
        assert_eq!(p.web.as_ref().unwrap().backend_url, "ws://127.0.0.1:8750", "loopback ws is allowed");
        draft.web.backend_url = String::new();
        draft.apply_kind(&mut p);
        assert_eq!(p.web.as_ref().unwrap().backend_url, "", "an empty address is allowed: not set yet");
    }

    #[test]
    fn a_refused_application_id_is_not_written_and_a_made_project_is_recorded_012() {
        let mut p = mobile_project(MobilePlatform::Android);
        p.mobile.as_mut().unwrap().app_id = "com.company.orders".into();
        let mut draft = SettingsDraft::from_project(&p, &LlmConfig::load_defaults_for_test());
        assert_eq!(draft.mobile.as_ref().unwrap().app_id, "com.company.orders");
        for bad in ["", "orders", "com.2fast.orders", "com.my-company.orders"] {
            draft.mobile.as_mut().unwrap().app_id = bad.into();
            draft.apply_kind(&mut p);
            assert_eq!(p.mobile.as_ref().unwrap().app_id, "com.company.orders", "{bad:?} was written");
        }
        draft.mobile.as_mut().unwrap().app_id = " com.company.sales ".into();
        crate::panels::native_project::record_android(draft.mobile.as_mut().unwrap());
        draft.apply_kind(&mut p);
        let m = p.mobile.as_ref().unwrap();
        assert_eq!(m.app_id, "com.company.sales", "a valid id is written, trimmed");
        assert_eq!(m.native.android, "android", "the made project is recorded");
        // And the record survives the project file.
        let text = toml::to_string(&p).unwrap();
        assert!(text.contains("[mobile.native]") && text.contains("android = \"android\""), "{text}");
        let back: CoboltProject = toml::from_str(&text).unwrap();
        assert_eq!(back.mobile.unwrap().native.android, "android");
        println!("application id: 4 refused ids keep the saved one, 1 valid id written; [mobile.native] round-trips");
    }

    #[test]
    fn a_mobile_project_edits_its_own_table_and_ios_starts_with_multi_memory_off() {
        let llm = LlmConfig::load_defaults_for_test();
        let mut p = mobile_project(MobilePlatform::Ios);
        let mut draft = SettingsDraft::from_project(&p, &llm);
        assert_eq!(draft.kind, ProjectKind::Mobile);
        let m = draft.mobile.as_mut().unwrap();
        assert!(!m.multi_memory, "iOS default");
        m.orientation = Orientation::Landscape;
        m.min_os = "26.0".into();
        m.backend_url = "wss://m.example.com".into();
        assert_eq!(draft.mobile.as_ref().unwrap().module_variants().multi_memory, false);
        draft.apply_kind(&mut p);
        let m = p.mobile.as_ref().unwrap();
        assert_eq!((m.orientation, m.min_os.as_str(), m.backend_url.as_str()), (Orientation::Landscape, "26.0", "wss://m.example.com"));
        assert!(p.web.is_none(), "a Mobile project gains no [web]");
        assert!(p.backend.is_none(), "no Backend part, none written");
        // Its minimum-OS choices are its platform's.
        let ios: Vec<&str> = min_os_choices(MobilePlatform::Ios).iter().map(|c| c.id).collect();
        let android: Vec<&str> = min_os_choices(MobilePlatform::Android).iter().map(|c| c.id).collect();
        assert!(ios.contains(&"26.0") && !ios.contains(&"36") && !ios.contains(&"17.0"));
        assert!(android.contains(&"36") && !android.contains(&"26.0") && !android.contains(&"29"));
    }

    // ── backend/exports.toml is the one list (WASM-003 C5, D5) ───────────

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-ide-wasm003-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An older project — its exports only in the project file — gets
    /// backend/exports.toml once, written from those names; the project file is
    /// not touched, and a second open writes nothing.
    #[test]
    fn opening_a_project_with_old_exports_writes_exports_toml_and_leaves_cobolt_toml() {
        let dir = scratch("migrate");
        let manifest = dir.join("Shop.project.toml");
        let text = "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nbackend = [\"backend/src/health.cbl\"]\n\n[backend]\nexports = [\"HEALTH\", \"ORDERS\"]\n";
        std::fs::write(&manifest, text).unwrap();
        let p = crate::project_model::load_project(&manifest).expect("an older Web project loads");
        let written = migrate_exports(&dir, &p).unwrap().expect("the file is written");
        assert_eq!(written, exports_path(&dir));
        let made = std::fs::read_to_string(&written).unwrap();
        let parsed: toml::Value = toml::from_str(&made).unwrap();
        assert_eq!(parsed["contract"]["version"].as_str(), Some("1.0.0"));
        let entries = parsed["export"].as_array().unwrap();
        assert_eq!(entries.iter().map(|e| e["program"].as_str().unwrap()).collect::<Vec<_>>(), ["HEALTH", "ORDERS"]);
        assert!(entries.iter().all(|e| e.as_table().unwrap().len() == 1), "no roles, not public: {made}");
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text, "the project file is byte for byte as it was");
        assert_eq!(read_exports_text(&dir), "HEALTH\nORDERS");
        assert_eq!(migrate_exports(&dir, &p).unwrap(), None, "once: the file exists now");
        println!("migration: [backend] exports = [HEALTH, ORDERS] → {} ({} bytes); project file unchanged ({} bytes)", written.display(), made.len(), text.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The box edits the file in place: a kept entry keeps its roles, `public`
    /// and comments, a new name is added bare, a removed one goes; saving the
    /// same list again changes nothing.
    #[test]
    fn an_exports_edit_keeps_each_entrys_roles() {
        let dir = scratch("edit");
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        let before = "# Shop — the programs a remote caller can reach.\n\n[contract]\nversion = \"2.3.0\"\n\n[[export]]\nprogram = \"HEALTH\"\npublic = true\n\n# Only the back office.\n[[export]]\nprogram = \"ORDERS\"\nroles = [\"admin\", \"clerk\"]\nidempotent = true\n";
        std::fs::write(exports_path(&dir), before).unwrap();
        assert_eq!(read_exports_text(&dir), "HEALTH\nORDERS");
        let changed = write_exports(&dir, &parse_exports("ORDERS\nREFUND\n")).unwrap();
        assert!(changed);
        let after = std::fs::read_to_string(exports_path(&dir)).unwrap();
        let parsed: toml::Value = toml::from_str(&after).unwrap();
        let entries = parsed["export"].as_array().unwrap();
        assert_eq!(entries.iter().map(|e| e["program"].as_str().unwrap()).collect::<Vec<_>>(), ["ORDERS", "REFUND"]);
        assert_eq!(entries[0]["roles"].as_array().unwrap().len(), 2, "{after}");
        assert_eq!(entries[0]["idempotent"].as_bool(), Some(true));
        assert_eq!(entries[1].as_table().unwrap().len(), 1, "a new name is added bare");
        assert_eq!(parsed["contract"]["version"].as_str(), Some("2.3.0"), "the rest of the file stays");
        assert!(after.starts_with("# Shop") && after.contains("# Only the back office."), "comments stay: {after}");
        assert!(!write_exports(&dir, &parse_exports("ORDERS\nREFUND")).unwrap(), "the same list again changes nothing");
        // A project with no file yet gets one.
        let fresh = scratch("edit-new");
        assert!(write_exports(&fresh, &["HEALTH".to_owned()]).unwrap());
        assert_eq!(read_exports_text(&fresh), "HEALTH");
        println!("exports edit: HEALTH, ORDERS(roles admin+clerk, idempotent) → ORDERS(kept), REFUND(new); {} → {} bytes, comments kept", before.len(), after.len());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    /// The state is written only when it says something: the default on a
    /// project with no `[backend]` table adds none.
    #[test]
    fn the_state_is_written_only_when_it_is_not_the_default() {
        let llm = LlmConfig::load_defaults_for_test();
        let mut p = CoboltProject::new("W.project", "");
        p.project.kind = ProjectKind::Web;
        p.files.backend.push("backend/src/health.cbl".into());
        let mut draft = SettingsDraft::from_project(&p, &llm);
        draft.apply_kind(&mut p);
        assert!(p.backend.is_none(), "per-call is the default: no [backend] table");
        draft.backend_state = STATE_PER_CONNECTION.into();
        draft.apply_kind(&mut p);
        assert_eq!(p.backend.as_ref().unwrap().state, STATE_PER_CONNECTION);
        draft.backend_state = STATE_PER_CALL.into();
        draft.apply_kind(&mut p);
        assert_eq!(p.backend.as_ref().unwrap().state, STATE_PER_CALL);
    }

    // ── The section never changes the Settings form's size ────────────────

    /// The section drawn in every language, for each kind: it keeps the width it
    /// was given and never asks for more.
    #[test]
    fn the_section_keeps_to_the_width_it_is_given_in_every_language() {
        let llm = LlmConfig::load_defaults_for_test();
        let theme = crate::theme::default_theme();
        let mut widest = 0.0f32;
        let mut drawn = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            for p in [web_project(), mobile_project(MobilePlatform::Android), mobile_project(MobilePlatform::Ios)] {
                let ctx = egui::Context::default();
                crate::app::apply_glass_visuals(&ctx, theme);
                let mut draft = SettingsDraft::from_project(&p, &llm);
                draft.web.backend_url = "ws://not.loopback.example.com".into();
                if let Some(m) = &mut draft.mobile {
                    m.backend_url = "http://nope".into();
                }
                // The Identity rows, every one of them, with a problem note of each kind.
                draft.identity = IdentityDraft { provider: "cognito".into(), cookie_key: "a-literal-key".into(), min_score: "2".into(), ..Default::default() };
                for _ in 0..4 {
                    let mut input = egui::RawInput::default();
                    input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 3000.0)));
                    let mut used = 0.0f32;
                    let mut out = ctx.run_ui(input, |ui| {
                        let before = ui.available_width();
                        let lay = RowLayout { splitter: 260.0, resizer_width: 5.0, gap_after_resizer: 10.0, property_indent: 12.0 };
                        show(ui, &mut draft, &tr, theme, lay, None, &mut PageJobs::default());
                        used = ui.min_rect().width();
                        assert!(ui.min_rect().width() <= before + 0.5, "[{lang:?}] the section is wider than the room it was given");
                    });
                    out.textures_delta.clear();
                    widest = widest.max(used);
                    drawn += 1;
                }
            }
        }
        println!("web/mobile settings section: {drawn} frames over 6 languages x 3 kinds, widest {widest:.0} px of 1400");
    }

    /// WASM-008 T37 (Q7, D11): a path is asked once per project folder on this
    /// machine; the answer is kept outside the project; a copy of the project
    /// in another folder is asked again; an unconfirmed path is never started.
    #[test]
    fn a_browser_path_is_confirmed_once_per_project_folder_008() {
        let dir = scratch("confirm");
        let project = dir.join("Shop");
        let copy = dir.join("Shop copy");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&copy).unwrap();
        let file = dir.join("settings").join("preview_browsers.toml");
        let path = std::env::current_exe().unwrap();

        let mut store = BrowserConfirmations::load_from(&file);
        assert_eq!(store.answer(&project, &path), None, "never asked: the IDE asks before it starts anything");
        store.record(&project, &path, true);
        store.save_to(&file);
        let again = BrowserConfirmations::load_from(&file);
        assert_eq!(again.answer(&project, &path), Some(true), "a confirmed path is used next time");
        assert_eq!(again.answer(&copy, &path), None, "the same project in another folder asks again");
        assert_eq!(again.answer(&project, Path::new("/another/program")), None, "another path asks again");
        let mut refused = again.clone();
        refused.record(&project, &path, false);
        assert_eq!(refused.answer(&project, &path), Some(false));
        assert_eq!(refused.entries.len(), 1, "one answer per folder and path");
        assert!(!project.join("preview_browsers.toml").exists() && file.is_file(), "the answer is kept outside the project");

        // Unconfirmed: the Preview never runs the path's launch plan, and says so.
        std::env::set_var(cobolt_project_tools::web::preview::NO_BROWSER_ENV, "1");
        let notes = crate::panels::web_preview::tests_support::open_with(BrowserChoice::Path(path.clone()), false);
        assert!(notes.iter().any(|n| matches!(n, crate::panels::web_preview::Note::BrowserNotConfirmed { .. })), "{notes:?}");
        let notes = crate::panels::web_preview::tests_support::open_with(BrowserChoice::Path(path.clone()), true);
        assert!(notes.iter().any(|n| matches!(n, crate::panels::web_preview::Note::Opened { browser, .. } if *browser == path.display().to_string())), "{notes:?}");
        println!("browser confirmation: asked once for {} in {}, kept in {}; a copy in another folder asks again", path.display(), project.display(), file.display());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WASM-008 T37: the browser row and the confirmation read in six
    /// languages without a gap, and the confirmation keeps its rectangle over
    /// 120 frames in each.
    #[test]
    fn the_browser_row_and_the_confirmation_read_in_six_languages_and_keep_their_size_008() {
        let mut checked = 0;
        let mut sizes = std::collections::BTreeSet::new();
        let long_path = "/Applications/A Browser With A Very Long Name Indeed For Testing.app/Contents/MacOS/browser";
        for &lang in Language::ALL {
            let tr = lang.tr();
            for t in [tr.pk_set_preview_browser, tr.pk_set_preview_browser_hint, tr.pk_set_browser_system, tr.pk_set_browser_other, tr.pk_prev_confirm_title, tr.pk_prev_confirm_yes, tr.pk_prev_confirm_no] {
                assert!(!t.trim().is_empty() && !t.contains("{}") && !t.to_lowercase().contains("cobolt"), "{lang:?}: {t:?}");
            }
            let q = tr.pk_prev_confirm_text.replacen("{}", long_path, 1);
            assert!(q.contains(long_path) && !q.contains("{}"), "{lang:?}: {q}");

            let ctx = egui::Context::default();
            crate::app::apply_glass_visuals(&ctx, crate::theme::default_theme());
            let mut first: Option<egui::Rect> = None;
            for frame in 0..120 {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0)));
                let mut out = ctx.run_ui(input, |ui| {
                    assert_eq!(show_browser_confirm(ui.ctx(), &tr, long_path), None, "nobody answered");
                });
                out.textures_delta.clear();
                let rect = ctx.memory(|m| {
                    m.layer_ids().filter(|l| l.order == egui::Order::Middle).find_map(|l| m.area_rect(l.id))
                });
                let Some(rect) = rect else { continue };
                if frame >= 2 {
                    match first {
                        None => first = Some(rect),
                        Some(f) => assert_eq!(rect, f, "{lang:?}: frame {frame} moved or resized the confirmation"),
                    }
                    checked += 1;
                }
            }
            let f = first.expect("the confirmation was drawn");
            assert!((f.width() - CONFIRM_W).abs() < 2.0, "{lang:?}: {:?}", f.size());
            sizes.insert(((f.width() * 10.0) as i32, lang.code()));
        }
        println!("browser confirmation: {checked} frames over 6 languages, one width {CONFIRM_W} for all ({} languages)", sizes.len());
    }

    #[test]
    fn a_desktop_project_draws_nothing() {
        let llm = LlmConfig::load_defaults_for_test();
        let p = CoboltProject::new("D.project", "src/main.cbl");
        let mut draft = SettingsDraft::from_project(&p, &llm);
        let ctx = egui::Context::default();
        let theme = crate::theme::default_theme();
        let tr = Language::English.tr();
        let mut height = 1.0;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0)));
        let mut out = ctx.run_ui(input, |ui| {
            let lay = RowLayout { splitter: 260.0, resizer_width: 5.0, gap_after_resizer: 10.0, property_indent: 12.0 };
            let before = ui.cursor().top();
            show(ui, &mut draft, &tr, theme, lay, None, &mut PageJobs::default());
            height = ui.cursor().top() - before;
        });
        out.textures_delta.clear();
        assert_eq!(height, 0.0, "a Desktop project's Settings form gains no row");
    }

    /// WASM-009 T19: the Identity section in every language has a text.
    #[test]
    fn every_identity_text_is_given_in_every_language() {
        for lang in Language::ALL {
            let tr = lang.tr();
            for (name, text) in [
                ("sec_identity", tr.pk_set_sec_identity),
                ("id_provider", tr.pk_set_id_provider),
                ("id_provider_none", tr.pk_set_id_provider_none),
                ("id_flow", tr.pk_set_id_flow),
                ("id_flow_auto", tr.pk_set_id_flow_auto),
                ("id_flow_in_app", tr.pk_set_id_flow_in_app),
                ("id_flow_hosted", tr.pk_set_id_flow_hosted),
                ("id_flow_native", tr.pk_set_id_flow_native),
                ("id_settings_hint", tr.pk_set_id_settings_hint),
                ("id_secret_hint", tr.pk_set_id_secret_hint),
                ("id_literal_secret", tr.pk_set_id_literal_secret),
                ("id_origins", tr.pk_set_id_origins),
                ("id_origins_hint", tr.pk_set_id_origins_hint),
                ("id_cookie_key", tr.pk_set_id_cookie_key),
                ("id_cookie_key_hint", tr.pk_set_id_cookie_key_hint),
                ("id_recaptcha_site_key", tr.pk_set_id_recaptcha_site_key),
                ("id_recaptcha_secret", tr.pk_set_id_recaptcha_secret),
                ("id_recaptcha_threshold", tr.pk_set_id_recaptcha_threshold),
                ("id_recaptcha_threshold_bad", tr.pk_set_id_recaptcha_threshold_bad),
                ("id_test", tr.pk_set_id_test),
                ("id_testing", tr.pk_set_id_testing),
                ("id_test_hint", tr.pk_set_id_test_hint),
                ("identity_write_failed", tr.pk_set_identity_write_failed),
            ] {
                assert!(!text.trim().is_empty(), "[{lang:?}] pk_set_{name} is empty");
            }
            assert!(tr.pk_set_id_literal_secret.contains("{}") && tr.pk_set_identity_write_failed.contains("{}"), "[{lang:?}] a placeholder is missing");
        }
    }

    const BACKEND_TOML: &str = "# The backend of the test.\n[server]\nlisten = \"127.0.0.1:8080\"   # loopback\nallowed_origins = [\"https://app.example.test\"]\n\n[limits]\nmax_frame_bytes = 65536\n";

    /// WASM-009 T19 (R9): a secret typed by value is refused and the file is not touched; a
    /// reference is written, every other section stays byte for byte, and the file reads back as
    /// the draft. No provider removes the section and gives the file back as it was.
    #[test]
    fn the_identity_is_written_only_with_references_and_the_rest_of_the_file_stays() {
        let dir = scratch("identity");
        let path = backend_config_path(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, BACKEND_TOML).unwrap();

        let mut d = IdentityDraft { provider: "cognito".into(), flow: "in-app".into(), cookie_key: "env:PRC_COOKIE".into(), origins_text: "https://app.example.test\n\n".into(), ..Default::default() };
        for (k, v) in [("region", "eu-west-1"), ("user_pool_id", "eu-west-1_Pool"), ("client_id", "client-1"), ("client_secret", "a-real-secret")] {
            d.settings.insert(k.into(), v.into());
        }
        assert_eq!(d.problems(), vec![IdentityProblem::LiteralSecret("identity.settings.client_secret".into())]);
        let e = write_identity(&dir, &d).expect_err("a secret by value is refused");
        assert!(!e.contains("a-real-secret"), "the refusal names the setting, not the value: {e}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), BACKEND_TOML, "a refused draft writes nothing");

        d.settings.insert("client_secret".into(), "env:PRC_CLIENT_SECRET".into());
        d.site_key = "site-key-public".into();
        d.recaptcha_secret = "file:/run/secrets/recaptcha".into();
        d.min_score = "0.7".into();
        assert!(d.problems().is_empty(), "{:?}", d.problems());
        assert_eq!(write_identity(&dir, &d), Ok(true));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(BACKEND_TOML), "the other sections stay byte for byte:\n{text}");
        assert!(!text.contains("a-real-secret"));
        let mut expected = d.clone();
        expected.origins_text = "https://app.example.test".into();
        assert_eq!(read_identity(&dir), expected, "the file reads back as the draft");
        text.parse::<toml_edit::DocumentMut>().expect("the written file is TOML");
        assert_eq!(write_identity(&dir, &d), Ok(false), "the same draft again changes nothing");

        // A threshold out of range is refused too.
        let bad = IdentityDraft { min_score: "1.5".into(), ..d.clone() };
        assert_eq!(bad.problems(), vec![IdentityProblem::Threshold]);
        assert!(write_identity(&dir, &bad).is_err());

        assert_eq!(write_identity(&dir, &IdentityDraft::default()), Ok(true));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), BACKEND_TOML, "no provider removes [identity] alone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WASM-014 T17: the store mode and the idle time are read and written in `[contract]`; the
    /// rest of the file stays byte for byte, a file without the section gains only it, and the
    /// defaults put the file back as it was.
    #[test]
    fn contract_settings_round_trip_and_keep_other_sections() {
        let dir = scratch("contract");
        let path = backend_config_path(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, BACKEND_TOML).unwrap();
        assert_eq!(read_contract_settings(&dir), ContractDraft::default());
        assert_eq!(write_contract_settings(&dir, &ContractDraft::default()), Ok(false), "the defaults add nothing");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), BACKEND_TOML);

        let d = ContractDraft { store: STORE_SERVER.into(), idle_seconds: "1200".into() };
        assert_eq!(write_contract_settings(&dir, &d), Ok(true));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(BACKEND_TOML), "the other sections stay byte for byte:\n{text}");
        assert_eq!(text[BACKEND_TOML.len()..].trim(), "[contract]\nidempotency_store = \"server\"\nsession_idle_seconds = 1200");
        assert_eq!(read_contract_settings(&dir), d, "the file reads back as the draft");
        assert_eq!(write_contract_settings(&dir, &d), Ok(false), "the same draft again changes nothing");

        // Another key of the section, and a comment, survive an edit of the two.
        let mut with_other = text.clone();
        with_other.push_str("# keep me\nstrict = true\n");
        std::fs::write(&path, &with_other).unwrap();
        let memory = ContractDraft { store: STORE_MEMORY.into(), idle_seconds: String::new() };
        assert_eq!(write_contract_settings(&dir, &memory), Ok(true));
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("# keep me") && after.contains("strict = true") && after.contains("idempotency_store = \"memory\""), "{after}");
        assert!(!after.contains("session_idle_seconds"));
        assert_eq!(read_contract_settings(&dir), memory);

        for bad in ["0", "-3", "abc", "1.5"] {
            let b = ContractDraft { store: STORE_SERVER.into(), idle_seconds: bad.into() };
            assert!(!b.idle_ok() && write_contract_settings(&dir, &b).is_err(), "{bad}");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after, "a refused draft writes nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Backend's development port is the project's own setting: read from `[server] listen`, written back with
    /// everything else in the file untouched, and an unusable one is refused.
    #[test]
    fn the_backend_port_is_read_and_written_in_server_listen_leaving_the_rest_alone() {
        let dir = std::env::temp_dir().join(format!("prc-be-port-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(BACKEND_FOLDER)).unwrap();
        let path = backend_config_path(&dir);
        assert_eq!(read_backend_port(&dir), "", "no file");
        std::fs::write(&path, "# where it runs\n[server]\nlisten = \"127.0.0.1:8443\"  # dev\n\n[contract]\nsession_idle_seconds = 60\n").unwrap();
        assert_eq!(read_backend_port(&dir), "8443");
        assert_eq!(write_backend_port(&dir, "9100"), Ok(true));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "# where it runs\n[server]\nlisten = \"127.0.0.1:9100\"  # dev\n\n[contract]\nsession_idle_seconds = 60\n", "only the port changed");
        assert_eq!(write_backend_port(&dir, " 9100 "), Ok(false), "the same port changes nothing");
        for bad in ["", "0", "65536", "80a", "-1"] {
            assert!(write_backend_port(&dir, bad).is_err() && !backend_port_ok(bad), "{bad:?}");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "a refused port writes nothing");
        // No file, or no [server]: the section is created, on loopback.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(write_backend_port(&dir, "8450"), Ok(true));
        assert_eq!(read_backend_port(&dir), "8450");
        assert!(std::fs::read_to_string(&path).unwrap().contains("listen = \"127.0.0.1:8450\""));
        println!("backend port: read 8443, wrote 9100 (comments and [contract] kept), refused 5 bad values, created [server] for 8450");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
