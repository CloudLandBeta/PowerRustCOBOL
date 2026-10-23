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

use std::path::{Path, PathBuf};

use egui::{RichText, Ui};

use cobolt_compiler::project_kind::{
    backend_url_problem, ApplicationStyle, BackendUrlProblem, Isolation, MobilePlatform, ModuleVariants, Orientation, BACKEND_FOLDER,
};
use cobolt_compiler::ProjectKind;
use cobolt_project_tools::templates;

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
    /// channel accepts (WASM-003 R5): the section told the developer why not.
    pub fn apply_kind(&self, p: &mut CoboltProject) {
        p.project.description = self.description.clone();
        p.project.locale = self.locale.clone();
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

/// Draw the section for a Web or Mobile project; nothing for a Desktop one.
pub fn show(ui: &mut Ui, draft: &mut SettingsDraft, tr: &Tr, theme: &Theme, lay: RowLayout) {
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
    note(ui, lay, &tr.pk_set_variants.replacen("{}", &variants_text(tr, variants), 1), dim);

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
        assert_eq!(draft.web.module_variants().count(), 1);
        draft.apply_kind(&mut p);
        assert!(!p.web.as_ref().unwrap().browsers.safari);
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
    fn a_mobile_project_edits_its_own_table_and_ios_starts_with_multi_memory_off() {
        let llm = LlmConfig::load_defaults_for_test();
        let mut p = mobile_project(MobilePlatform::Ios);
        let mut draft = SettingsDraft::from_project(&p, &llm);
        assert_eq!(draft.kind, ProjectKind::Mobile);
        let m = draft.mobile.as_mut().unwrap();
        assert!(!m.multi_memory, "iOS default");
        m.orientation = Orientation::Landscape;
        m.min_os = "17.0".into();
        m.backend_url = "wss://m.example.com".into();
        assert_eq!(draft.mobile.as_ref().unwrap().module_variants().multi_memory, false);
        draft.apply_kind(&mut p);
        let m = p.mobile.as_ref().unwrap();
        assert_eq!((m.orientation, m.min_os.as_str(), m.backend_url.as_str()), (Orientation::Landscape, "17.0", "wss://m.example.com"));
        assert!(p.web.is_none(), "a Mobile project gains no [web]");
        assert!(p.backend.is_none(), "no Backend part, none written");
        // Its minimum-OS choices are its platform's.
        let ios: Vec<&str> = min_os_choices(MobilePlatform::Ios).iter().map(|c| c.id).collect();
        let android: Vec<&str> = min_os_choices(MobilePlatform::Android).iter().map(|c| c.id).collect();
        assert!(ios.contains(&"17.0") && !ios.contains(&"29"));
        assert!(android.contains(&"29") && !android.contains(&"17.0"));
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
                for _ in 0..4 {
                    let mut input = egui::RawInput::default();
                    input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 3000.0)));
                    let mut used = 0.0f32;
                    let mut out = ctx.run_ui(input, |ui| {
                        let before = ui.available_width();
                        let lay = RowLayout { splitter: 260.0, resizer_width: 5.0, gap_after_resizer: 10.0, property_indent: 12.0 };
                        show(ui, &mut draft, &tr, theme, lay);
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
            show(ui, &mut draft, &tr, theme, lay);
            height = ui.cursor().top() - before;
        });
        out.textures_delta.clear();
        assert_eq!(height, 0.0, "a Desktop project's Settings form gains no row");
    }
}
