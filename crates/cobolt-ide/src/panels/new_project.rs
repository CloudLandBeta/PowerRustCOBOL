// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The New Project dialog** (WASM-001 R1–R7, the operator's mock-up).
//!
//! Two columns. On the left, the **template list** with a search box — the
//! twelve entries of `cobolt_project_tools::templates::builtin()`, the ones that
//! cannot be created yet shown dimmed and marked — and *Recent Projects* at the
//! foot. On the right, the selected template's **Project Properties** (name,
//! version, entry file, target platform, output type, location, description,
//! UI style, locale) and its **Template-Specific Options**, which are whatever
//! the template's descriptor lists. Along the bottom: *Create from Template…*,
//! *Recent Projects ▾*, *Cancel* and *Create*. The title follows the template.
//!
//! **The dialog knows no template.** Everything it draws about one — its name,
//! options, defaults, whether it can be created — comes from the descriptor
//! (`TemplateDescriptor`), so adding a template adds a descriptor and nothing
//! here (WASM-001 AC5). The words come from `Tr` through two lookups keyed by
//! the ids; a descriptor the lookups do not know shows the descriptor's own
//! English, which is what a template registered by a test does.
//!
//! **The window never resizes itself** (the golden rule): it owns an explicit
//! `size`, is `resizable(false).fixed_size(size)`, lays every child out from
//! that stored number, and the one grip in its own foreground `Area` is the only
//! writer of the number — `leaderboard_modal.rs` is the reference.
//!
//! The logic — validation, defaults, what a selection changes, the request
//! built from the fields — is plain data and methods, tested without a window.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cobolt_project_tools::create::{self, CreateRequest};
use cobolt_project_tools::templates::{self, OptionDescriptor, OptionKind, OutputType, TemplateDescriptor};

use crate::i18n::{Language, Tr};
use crate::theme::Theme;

/// Opening size. From here on the size is whatever the developer dragged it to.
pub const DEFAULT_W: f32 = 960.0;
pub const DEFAULT_H: f32 = 620.0;
/// Hard stops for the grip.
const MIN_W: f32 = 780.0;
const MIN_H: f32 = 500.0;
const MAX_W: f32 = 1800.0;
const MAX_H: f32 = 1200.0;
/// Side of the resize grip, and its inset from the window's border.
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// The window's title bar, its margins and its stroke, with room to spare:
/// subtracted from the stored height to give the body. A constant — never
/// measured from the Ui — so the body can never ask for more than the box.
const CHROME_H: f32 = 84.0;
/// The button row along the bottom.
const BAR_H: f32 = 48.0;
/// The template list's column.
const LEFT_W: f32 = 290.0;
/// A row of the template list.
const ROW_H: f32 = 46.0;
/// The label column of the property grid.
const LABEL_W: f32 = 200.0;
const SZ_BODY: f32 = 14.0;
const SZ_SMALL: f32 = 13.0;
const SZ_TITLE: f32 = 18.0;

/// What is shown on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The template at this index of the catalogue.
    Template(usize),
    /// The list of recent projects.
    Recent,
}

/// Why *Create* cannot go ahead; shown beside the buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The selected template cannot be created yet.
    Unavailable,
    /// No name.
    Name,
    /// A character a file name cannot carry.
    NameChars,
    /// Not `major.minor.fix`.
    Version,
    /// No usable location.
    Location,
    /// The entry file is not a path inside the project.
    Entry,
    /// A project of that name is already in the folder; carries its path.
    Exists(String),
    /// The request was refused by the template catalogue; carries the words.
    Refused(String),
}

impl Problem {
    /// The sentence, in the language of `tr`.
    pub fn message(&self, tr: &Tr) -> String {
        match self {
            Problem::Unavailable => tr.pk_unavailable_note.to_owned(),
            Problem::Name => tr.pk_err_name.to_owned(),
            Problem::NameChars => tr.pk_err_name_chars.to_owned(),
            Problem::Version => tr.pk_err_version.to_owned(),
            Problem::Location => tr.pk_err_location.to_owned(),
            Problem::Entry => tr.pk_err_entry.to_owned(),
            Problem::Exists(path) => tr.pk_err_exists.replacen("{}", path, 1),
            Problem::Refused(why) => why.clone(),
        }
    }
}

/// A project in the Recent list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentRow {
    pub manifest: PathBuf,
    /// The project's name, from its file name.
    pub name: String,
    /// Is the manifest still there?
    pub exists: bool,
}

impl RecentRow {
    pub fn new(manifest: PathBuf) -> Self {
        let name = manifest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .map(|n| n.strip_suffix(".toml").unwrap_or(&n).to_owned())
            .map(|n| n.strip_suffix(".project").unwrap_or(&n).to_owned())
            .unwrap_or_default();
        let exists = manifest.is_file();
        Self { manifest, name, exists }
    }
}

/// A creation ready to run: the request and the folder it goes into.
#[derive(Debug, Clone)]
pub struct Ready {
    pub request: CreateRequest,
    /// The project's own folder: `<location>/<name>`.
    pub dir: PathBuf,
}

/// Where a copy of a project goes (*Create from Template…*).
#[derive(Debug, Clone)]
pub struct CopyTarget {
    pub dir: PathBuf,
    /// The name as stored, with the `.project` suffix.
    pub stored_name: String,
}

/// What a frame of the dialog asks the app to do.
#[derive(Default)]
pub struct DialogAction {
    pub create: Option<Ready>,
    /// The folder button: ask for a folder.
    pub browse_location: bool,
    /// *Create from Template…*
    pub from_template: bool,
    /// A recent project was chosen.
    pub open_recent: Option<PathBuf>,
}

pub struct NewProjectDialog {
    pub open: bool,
    catalogue: Vec<TemplateDescriptor>,
    pane: Pane,
    pub name: String,
    pub version: String,
    /// The Desktop entry file (`src/main.cbl`); a Web or Mobile project's entry
    /// is its main form, shown and not edited.
    pub main_file: String,
    /// The folder the project's own folder goes in.
    pub location: String,
    pub description: String,
    /// An IDE theme id; empty is the default.
    pub theme: String,
    /// A locale code such as `pt-BR`; empty is not set.
    pub locale: String,
    options: BTreeMap<String, String>,
    search: String,
    recents: Vec<RecentRow>,
    problem: Option<Problem>,
    /// The window's size, owned here rather than by egui: it changes only when
    /// the developer drags the grip.
    pub size: egui::Vec2,
}

/// The locales the dialog offers: the IDE's six languages, as tags.
pub const LOCALES: [(&str, Language); 6] = [
    ("en-US", Language::English),
    ("pt-BR", Language::Portuguese),
    ("es-ES", Language::Spanish),
    ("fr-FR", Language::French),
    ("ja-JP", Language::Japanese),
    ("zh-CN", Language::Chinese),
];

impl Default for NewProjectDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl NewProjectDialog {
    pub fn new() -> Self {
        Self::with_catalogue(templates::builtin())
    }

    /// A dialog over another catalogue — the way a template is added.
    pub fn with_catalogue(catalogue: Vec<TemplateDescriptor>) -> Self {
        let first = catalogue.iter().position(|t| t.creatable).unwrap_or(0);
        let mut d = Self {
            open: false,
            catalogue,
            pane: Pane::Template(first),
            name: "MyApp".into(),
            version: "1.0.0".into(),
            main_file: create::DEFAULT_MAIN.into(),
            location: String::new(),
            description: String::new(),
            theme: String::new(),
            locale: String::new(),
            options: BTreeMap::new(),
            search: String::new(),
            recents: Vec::new(),
            problem: None,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
        };
        d.select(first);
        d
    }

    /// Open the dialog on the first creatable template, with the folder the
    /// last project went into and the list of recent projects.
    pub fn open_with(&mut self, location: &str, recents: Vec<PathBuf>) {
        self.open = true;
        self.problem = None;
        self.search.clear();
        if self.location.trim().is_empty() {
            self.location = location.to_owned();
        }
        self.recents = recents.into_iter().map(RecentRow::new).collect();
        let first = self.catalogue.iter().position(|t| t.creatable).unwrap_or(0);
        self.pane = Pane::Template(first);
        self.select(first);
    }

    pub fn catalogue(&self) -> &[TemplateDescriptor] {
        &self.catalogue
    }

    pub fn pane(&self) -> Pane {
        self.pane
    }

    pub fn recents(&self) -> &[RecentRow] {
        &self.recents
    }

    pub fn problem(&self) -> Option<&Problem> {
        self.problem.as_ref()
    }

    /// Show the recent projects instead of a template.
    pub fn show_recent(&mut self) {
        self.pane = Pane::Recent;
        self.problem = None;
    }

    /// The selected template, if a template is selected.
    pub fn template(&self) -> Option<&TemplateDescriptor> {
        match self.pane {
            Pane::Template(i) => self.catalogue.get(i),
            Pane::Recent => None,
        }
    }

    /// Select the template at `index` (WASM-001 R1): its options take their
    /// defaults, and the fields every template shares keep what was typed.
    pub fn select(&mut self, index: usize) {
        let Some(t) = self.catalogue.get(index) else { return };
        self.options = t.default_options();
        self.pane = Pane::Template(index);
        self.problem = None;
        // A Desktop entry file is editable and keeps what was typed; the entry
        // of a Web or Mobile project is its main form.
        if t.kind.is_desktop() && self.main_file.trim().is_empty() {
            self.main_file = create::DEFAULT_MAIN.into();
        }
    }

    /// The indexes of the templates the search keeps, in catalogue order.
    pub fn visible(&self) -> Vec<usize> {
        self.catalogue
            .iter()
            .enumerate()
            .filter(|(_, t)| t.matches(&self.search))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn set_search(&mut self, text: &str) {
        self.search = text.to_owned();
    }

    /// The options the selected template asks for, in order — and no others
    /// (WASM-001 R4).
    pub fn options_shown(&self) -> &'static [OptionDescriptor] {
        self.template().map_or(&[], |t| t.options)
    }

    /// An option's current value (a choice id, or `true` / `false`).
    pub fn option(&self, id: &str) -> &str {
        self.options.get(id).map(String::as_str).unwrap_or("")
    }

    pub fn set_option(&mut self, id: &str, value: &str) {
        self.options.insert(id.to_owned(), value.to_owned());
    }

    pub fn flag(&self, id: &str) -> bool {
        self.option(id) == "true"
    }

    /// The path of the project's own folder: `<location>/<name>`.
    pub fn project_dir(&self) -> PathBuf {
        let stem = self.name.trim().strip_suffix(".project").unwrap_or_else(|| self.name.trim());
        Path::new(self.location.trim()).join(create::sanitize_file_stem(stem))
    }

    /// The entry file the project will have, as shown: the Desktop field, or
    /// the main form of a Web or Mobile one.
    pub fn entry_shown(&self) -> String {
        match self.template() {
            Some(t) if !t.kind.is_desktop() => {
                if self.flag("sample") {
                    format!("{}/{}.{}", t.kind.forms_folder(), create::MAIN_FORM_NAME.to_lowercase(), t.kind.form_extension())
                } else {
                    String::new()
                }
            }
            _ => self.main_file.clone(),
        }
    }

    /// The request the fields describe.
    pub fn request(&self) -> CreateRequest {
        let t = self.template();
        let mut request = CreateRequest::new(self.name.trim());
        request.template = t.map(|t| t.id.to_owned()).unwrap_or_default();
        request.version = self.version.trim().to_owned();
        // A Web or Mobile project's entry is its main form: nothing to pass.
        if t.map_or(true, |t| t.kind.is_desktop()) {
            let main = self.main_file.trim();
            if main != create::DEFAULT_MAIN {
                request.main = main.to_owned();
            }
        }
        request.description = self.description.clone();
        request.theme = self.theme.clone();
        request.locale = self.locale.clone();
        request.options = self.options.clone();
        request
    }

    /// Check the fields and make the creation: the first thing wrong, or the
    /// request and the folder (WASM-001 R3, R5).
    pub fn ready(&self) -> Result<Ready, Problem> {
        let Some(t) = self.template() else { return Err(Problem::Unavailable) };
        if !t.creatable {
            return Err(Problem::Unavailable);
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Problem::Name);
        }
        if name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
            return Err(Problem::NameChars);
        }
        if !version_ok(&self.version) {
            return Err(Problem::Version);
        }
        let location = self.location.trim();
        if location.is_empty() || !Path::new(location).is_absolute() {
            return Err(Problem::Location);
        }
        if t.kind.is_desktop() && !self.main_file.trim().is_empty() && !create::is_project_relative(&self.main_file) {
            return Err(Problem::Entry);
        }
        let request = self.request();
        let plan = create::plan(&request, &self.catalogue).map_err(Problem::Refused)?;
        let dir = self.project_dir();
        let manifest = dir.join(&plan.manifest_name);
        if manifest.exists() {
            return Err(Problem::Exists(manifest.display().to_string()));
        }
        Ok(Ready { request, dir })
    }

    /// Where *Create from Template…* puts the copy: the project's own folder
    /// and its stored name. Only the name and the location matter to a copy.
    pub fn copy_target(&self) -> Result<CopyTarget, Problem> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Problem::Name);
        }
        if name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
            return Err(Problem::NameChars);
        }
        let location = self.location.trim();
        if location.is_empty() || !Path::new(location).is_absolute() {
            return Err(Problem::Location);
        }
        Ok(CopyTarget { dir: self.project_dir(), stored_name: create::project_name(name) })
    }

    /// Show `problem` beside the buttons.
    pub fn set_problem(&mut self, problem: Problem) {
        self.problem = Some(problem);
    }

    /// Run *Create*: the action to hand the app, or the problem to show.
    pub fn press_create(&mut self) -> Option<Ready> {
        match self.ready() {
            Ok(ready) => {
                self.problem = None;
                Some(ready)
            }
            Err(p) => {
                self.problem = Some(p);
                None
            }
        }
    }

    /// The title: *New* <template> *Project*, or the Recent Projects heading.
    pub fn title(&self, tr: &Tr) -> String {
        match self.template() {
            Some(t) => tr.pk_dlg_title.replacen("{}", template_text(tr, t).0, 1),
            None => tr.pk_recent.to_owned(),
        }
    }

    /// Change the size by a grip drag, within the hard stops.
    fn drag(&mut self, delta: egui::Vec2) {
        self.size += delta;
        self.size.x = self.size.x.clamp(MIN_W, MAX_W);
        self.size.y = self.size.y.clamp(MIN_H, MAX_H);
    }
}

/// `major.minor.fix`, three numbers.
pub fn version_ok(v: &str) -> bool {
    let parts: Vec<&str> = v.trim().split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 6 && p.chars().all(|c| c.is_ascii_digit()))
}

// ── Words: the lookups from ids to `Tr` ─────────────────────────────────────

/// A template's name and sentence in the language of `tr`; the descriptor's own
/// English for a template the table does not know.
pub fn template_text<'a>(tr: &'a Tr, t: &'a TemplateDescriptor) -> (&'a str, &'a str) {
    match t.id {
        "desktop" => (tr.pk_t_desktop, tr.pk_s_desktop),
        "chatbot-desktop" => (tr.pk_t_chatbot_desktop, tr.pk_s_chatbot_desktop),
        "chatbot-web" => (tr.pk_t_chatbot_web, tr.pk_s_chatbot_web),
        "web" => (tr.pk_t_web, tr.pk_s_web),
        "usercontrol-desktop" => (tr.pk_t_usercontrol_desktop, tr.pk_s_usercontrol_desktop),
        "usercontrol-web" => (tr.pk_t_usercontrol_web, tr.pk_s_usercontrol_web),
        "android-phone" => (tr.pk_t_android_phone, tr.pk_s_android_phone),
        "android-tablet" => (tr.pk_t_android_tablet, tr.pk_s_android_tablet),
        "android-watch" => (tr.pk_t_android_watch, tr.pk_s_android_watch),
        "iphone" => (tr.pk_t_iphone, tr.pk_s_iphone),
        "ipad" => (tr.pk_t_ipad, tr.pk_s_ipad),
        "apple-watch" => (tr.pk_t_apple_watch, tr.pk_s_apple_watch),
        _ => (t.name, t.summary),
    }
}

/// Is `id` a template the `Tr` table has words for?
pub fn template_has_words(id: &str) -> bool {
    let tr = Language::English.tr();
    let probe = TemplateDescriptor {
        id: "",
        name: "\u{0}",
        title_noun: "",
        platforms: "",
        summary: "\u{0}",
        kind: cobolt_compiler::ProjectKind::Desktop,
        mobile: None,
        creatable: false,
        options: &[],
    };
    let t = TemplateDescriptor { id: Box::leak(id.to_owned().into_boxed_str()), ..probe };
    template_text(&tr, &t).0 != "\u{0}"
}

/// An option's label in the language of `tr`.
pub fn option_label<'a>(tr: &'a Tr, t: &TemplateDescriptor, o: &'a OptionDescriptor) -> &'a str {
    match o.id {
        "window_style" => tr.pk_o_window_style,
        "ui_framework" => tr.pk_o_ui_framework,
        "high_dpi" => tr.pk_o_high_dpi,
        "gpu_acceleration" => tr.pk_o_gpu,
        "sample" if t.kind.is_desktop() => tr.pk_o_sample_program,
        "sample" => tr.pk_o_sample_form,
        "style" => tr.pk_o_style,
        "backend" => tr.pk_o_backend,
        "orientation" => tr.pk_o_orientation,
        "min_os" => tr.pk_o_min_os,
        _ => o.label,
    }
}

/// The label of a choice. The operating-system versions (`Android 14 (API 34)`,
/// `iOS 17.0`) are names, shown as the descriptor gives them.
pub fn choice_label<'a>(tr: &'a Tr, option_id: &str, c: &'a templates::Choice) -> &'a str {
    match (option_id, c.id) {
        ("window_style", "tabbed") => tr.pk_c_tabbed,
        ("window_style", "single") => tr.pk_c_single,
        ("ui_framework", "powerui") => tr.pk_c_powerui,
        ("style", "spa") => tr.pk_c_spa,
        ("style", "standalone") => tr.pk_c_standalone,
        ("orientation", "auto") => tr.pk_c_orient_auto,
        ("orientation", "portrait") => tr.pk_c_orient_portrait,
        ("orientation", "landscape") => tr.pk_c_orient_landscape,
        ("min_os", "auto") => tr.pk_c_os_auto,
        _ => c.label,
    }
}

/// What a project of this template builds into, in words.
pub fn output_text(tr: &Tr, o: OutputType) -> &str {
    match o {
        OutputType::Executable => tr.pk_out_executable,
        OutputType::WebBundle => tr.pk_out_web,
        OutputType::AndroidPackage => tr.pk_out_android,
        OutputType::IosArchive => tr.pk_out_ios,
    }
}

/// The catalogue icon (`cobolt_forms::icons`) that stands for a template.
pub fn template_icon(id: &str) -> &'static str {
    match id {
        "desktop" => "desktop-computer",
        "chatbot-desktop" | "chatbot-web" => "chat-dots",
        "web" => "globe",
        "usercontrol-desktop" | "usercontrol-web" => "component",
        "android-phone" | "iphone" => "smartphone",
        "android-tablet" | "ipad" => "tablet",
        "android-watch" | "apple-watch" => "smartwatch",
        _ => "doc-blank",
    }
}

/// The colour a template's icon is drawn in, by family, so the list reads at a
/// glance: Desktop violet, the chatbots indigo, Web the WebAssembly purple, user
/// controls lavender, Android green, iPhone and iPad blue, a watch amber.
pub fn template_tint(id: &str) -> egui::Color32 {
    match id {
        "desktop" => egui::Color32::from_rgb(0xB8, 0x6B, 0xFF),
        "chatbot-desktop" | "chatbot-web" => egui::Color32::from_rgb(0x7C, 0x83, 0xFF),
        "web" => egui::Color32::from_rgb(0x65, 0x4F, 0xF0),
        "usercontrol-desktop" | "usercontrol-web" => egui::Color32::from_rgb(0x8E, 0x92, 0xFF),
        "android-phone" | "android-tablet" | "android-watch" => egui::Color32::from_rgb(0x8B, 0xD4, 0x48),
        "iphone" | "ipad" => egui::Color32::from_rgb(0x4D, 0xA3, 0xFF),
        "apple-watch" => egui::Color32::from_rgb(0xF5, 0xB7, 0x31),
        _ => egui::Color32::from_rgb(0x9B, 0x8C, 0xFF),
    }
}

/// The *Recent Projects* row's icon colour.
const RECENT_TINT: egui::Color32 = egui::Color32::from_rgb(0x9B, 0x8C, 0xFF);

/// The platform line of the dialog: the host the Desktop build is for. Cross-
/// building is not offered, so the choice is shown and fixed (WASM-001 Q4).
pub fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

/// A locale tag as the combo shows it: the language in its own name, then the tag.
pub fn locale_label(tag: &str) -> String {
    LOCALES
        .iter()
        .find(|(t, _)| *t == tag)
        .map(|(t, l)| format!("{} ({t})", l.native_name()))
        .unwrap_or_else(|| tag.to_owned())
}

// ── The window ───────────────────────────────────────────────────────────────

impl NewProjectDialog {
    /// Draw the dialog for one frame; what it asks of the app comes back.
    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr) -> DialogAction {
        let mut action = DialogAction::default();
        if !self.open {
            return action;
        }
        let mut open = self.open;
        let title = self.title(tr);
        let screen = ctx.content_rect();
        let default_pos = egui::pos2(
            ((screen.width() - self.size.x) * 0.5).max(12.0),
            ((screen.height() - self.size.y) * 0.35).max(12.0),
        );
        let window = egui::Window::new(egui::RichText::new(title).size(SZ_TITLE).strong())
            .id(egui::Id::new("new_project_dialog"))
            .open(&mut open)
            .collapsible(false)
            // NEVER `resizable(true)`: see the module comment.
            .resizable(false)
            .fixed_size(self.size)
            .default_pos(default_pos)
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                let width = self.size.x - margin - stroke;
                let body_h = (self.size.y - CHROME_H).max(MIN_H - CHROME_H);
                ui.set_width(width);
                ui.allocate_ui_with_layout(egui::vec2(width, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_size(egui::vec2(width, body_h));
                    ui.set_max_size(egui::vec2(width, body_h));
                    self.body(ui, theme, tr, width, &mut action);
                });
            });
        // The grip: its drag delta is the one and only writer of the size.
        if let Some(window) = window {
            self.resize_grip(ctx, theme, window.response.rect);
        }
        if !open {
            self.open = false;
        }
        action
    }

    /// The bottom bar, then the two columns — partitioned with embedded panels
    /// so the content is the box and nothing asks for more.
    fn body(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, width: f32, action: &mut DialogAction) {
        egui::Panel::bottom("npd_bar")
            .resizable(false)
            .exact_size(BAR_H)
            .show_separator_line(true)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.bar(ui, theme, tr, action));
        egui::Panel::left("npd_left")
            .resizable(false)
            .exact_size(LEFT_W)
            .show_separator_line(true)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.list(ui, theme, tr));
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let right_w = width - LEFT_W - 12.0;
            match self.pane {
                Pane::Template(_) => self.properties(ui, theme, tr, right_w, action),
                Pane::Recent => self.recent_pane(ui, theme, tr, action),
            }
        });
    }

    fn bar(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, action: &mut DialogAction) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(tr.pk_btn_from_template).size(SZ_BODY)).clicked() {
                action.from_template = true;
            }
            let recent_label = format!("{} \u{25be}", tr.pk_recent);
            ui.menu_button(egui::RichText::new(recent_label).size(SZ_BODY), |ui| {
                if self.recents.is_empty() {
                    ui.weak(tr.pk_recent_empty);
                }
                for row in self.recents.iter().take(10) {
                    let text = if row.exists {
                        row.name.clone()
                    } else {
                        format!("{} ({})", row.name, tr.pk_recent_missing)
                    };
                    if ui.add_enabled(row.exists, egui::Button::new(text)).on_hover_text(row.manifest.display().to_string()).clicked() {
                        action.open_recent = Some(row.manifest.clone());
                        ui.close();
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let creatable = self.template().is_some_and(|t| t.creatable);
                let create = ui.add_enabled(
                    creatable,
                    egui::Button::new(egui::RichText::new(tr.btn_create).size(SZ_BODY).strong()),
                );
                let create = if creatable { create } else { create.on_disabled_hover_text(tr.pk_unavailable_note) };
                if create.clicked() {
                    action.create = self.press_create();
                }
                if ui.button(egui::RichText::new(tr.dlg_cancel).size(SZ_BODY)).clicked() {
                    self.open = false;
                }
                if let Some(problem) = &self.problem {
                    let text = problem.message(tr);
                    // Clipped to the room the buttons leave: a long sentence in
                    // another language cannot make the bar ask for more.
                    ui.add(egui::Label::new(egui::RichText::new(text).size(SZ_SMALL).color(theme.error)).truncate());
                }
            });
        });
    }

    /// The left column: search box, the templates, Recent Projects at the foot.
    fn list(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr.pk_all_templates).size(SZ_BODY).strong());
        ui.add_space(4.0);
        let mut search = self.search.clone();
        let field = egui::TextEdit::singleline(&mut search)
            .hint_text(tr.pk_search_hint)
            .desired_width(LEFT_W - 24.0);
        if ui.add(field).changed() {
            self.search = search;
        }
        ui.add_space(6.0);
        let visible = self.visible();
        // The Recent entry is pinned below the scrolling list.
        egui::Panel::bottom("npd_left_recent")
            .resizable(false)
            .exact_size(ROW_H)
            .show_separator_line(true)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let selected = self.pane == Pane::Recent;
                if self.row(ui, theme, "doc-history", RECENT_TINT, tr.pk_recent, "", selected, true) {
                    self.show_recent();
                }
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("npd_list").auto_shrink([false, false]).show(ui, |ui| {
                if visible.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(tr.pk_no_template_match).size(SZ_SMALL).color(theme.text_dim));
                }
                for index in visible {
                    let t = self.catalogue[index];
                    let (name, _) = template_text(tr, &t);
                    let sub = if t.creatable { t.platforms.to_owned() } else { tr.pk_unavailable.to_owned() };
                    let selected = self.pane == Pane::Template(index);
                    if self.row(ui, theme, template_icon(t.id), template_tint(t.id), name, &sub, selected, t.creatable) {
                        self.select(index);
                    }
                }
            });
        });
    }

    /// One row of the template list: icon, name, and a small line under it.
    /// Painted, so a long name in another language is clipped to the row and
    /// cannot widen anything.
    #[allow(clippy::too_many_arguments)]
    fn row(&self, ui: &mut egui::Ui, theme: &Theme, icon: &str, tint: egui::Color32, name: &str, sub: &str, selected: bool, available: bool) -> bool {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(LEFT_W - 14.0, ROW_H), egui::Sense::click());
        let painter = ui.painter().with_clip_rect(rect);
        if selected {
            painter.rect_filled(rect, 8.0, theme.selection);
        } else if response.hovered() {
            painter.rect_filled(rect, 8.0, theme.bg_hover);
        }
        let ink = if available { theme.text_bright } else { theme.text_dim };
        let icon_rect = egui::Rect::from_min_size(rect.min + egui::vec2(10.0, (ROW_H - 22.0) * 0.5), egui::vec2(22.0, 22.0));
        // The icon keeps its family colour; a template that cannot be created
        // yet shows it faded, as its text is dimmed.
        let icon_ink = if available { tint } else { tint.gamma_multiply(0.45) };
        cobolt_forms::icons::draw_menu_icon(&painter, icon_rect, icon, icon_ink);
        let x = rect.min.x + 42.0;
        painter.text(
            egui::pos2(x, rect.min.y + 7.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(SZ_BODY),
            ink,
        );
        if !sub.is_empty() {
            painter.text(
                egui::pos2(x, rect.min.y + 26.0),
                egui::Align2::LEFT_TOP,
                sub,
                egui::FontId::proportional(SZ_SMALL - 1.0),
                theme.text_dim,
            );
        }
        response.clicked()
    }

    /// The right column for a template.
    fn properties(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, right_w: f32, action: &mut DialogAction) {
        let Some(t) = self.template().copied() else { return };
        let (name, summary) = template_text(tr, &t);
        let field_w = (right_w - LABEL_W - 40.0).max(160.0);
        egui::ScrollArea::vertical().id_salt("npd_props").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(name).size(SZ_TITLE).strong());
            ui.add(egui::Label::new(egui::RichText::new(summary).size(SZ_SMALL).color(theme.text_dim)).wrap());
            if !t.creatable {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.pk_unavailable_note).size(SZ_BODY).color(theme.warn));
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr.pk_tab_properties).size(SZ_BODY).strong());
            ui.separator();
            ui.add_enabled_ui(t.creatable, |ui| {
                // Rows, not a Grid: a Grid sizes its label column to the longest
                // label, and a translation that is longer than English would make
                // the whole pane ask for more width than the window has. Each row
                // is a label cell of one fixed width and a field of another, so
                // nothing a label says can move anything.
                form_row(ui, tr.pk_f_name, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(field_w));
                });
                form_row(ui, tr.pk_f_version, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.version).desired_width(field_w));
                });
                form_row(ui, if t.kind.is_desktop() { tr.pk_f_main_file } else { tr.pk_f_entry_file }, |ui| {
                    if t.kind.is_desktop() {
                        ui.add(egui::TextEdit::singleline(&mut self.main_file).desired_width(field_w));
                    } else {
                        let mut entry = self.entry_shown();
                        ui.add(egui::TextEdit::singleline(&mut entry).interactive(false).desired_width(field_w));
                    }
                });
                form_row(ui, tr.pk_f_target, |ui| {
                    ui.add(clipped(format!("{} ({})", target_text(&t), tr.pk_f_target_note)));
                });
                form_row(ui, tr.pk_f_output, |ui| {
                    ui.add(clipped(output_text(tr, t.output_type()).to_owned()));
                });
                form_row(ui, tr.pk_f_location, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.location).desired_width(field_w - 36.0));
                    if folder_button(ui, theme, tr.pk_f_location) {
                        action.browse_location = true;
                    }
                });
                form_row(ui, tr.pk_f_description, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.description).desired_width(field_w));
                });
                form_row(ui, tr.pk_f_theme, |ui| {
                    egui::ComboBox::from_id_salt("npd_theme")
                        .width(field_w)
                        .selected_text(self.theme_text(tr))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.theme, String::new(), tr.pk_theme_default);
                            for th in crate::theme::THEMES {
                                ui.selectable_value(&mut self.theme, th.id.to_owned(), th.name);
                            }
                        });
                });
                form_row(ui, tr.pk_f_locale, |ui| {
                    egui::ComboBox::from_id_salt("npd_locale")
                        .width(field_w)
                        .selected_text(if self.locale.is_empty() { tr.pk_locale_none.to_owned() } else { locale_label(&self.locale) })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.locale, String::new(), tr.pk_locale_none);
                            for (tag, _) in LOCALES {
                                ui.selectable_value(&mut self.locale, tag.to_owned(), locale_label(tag));
                            }
                        });
                });

                ui.add_space(10.0);
                if !t.options.is_empty() {
                    egui::CollapsingHeader::new(egui::RichText::new(tr.pk_options_header).size(SZ_BODY).strong())
                        .id_salt("npd_options")
                        .default_open(true)
                        .show(ui, |ui| {
                            for o in t.options {
                                self.option_row(ui, tr, &t, o, field_w);
                            }
                        });
                }
            });
        });
    }

    fn theme_text(&self, tr: &Tr) -> String {
        if self.theme.is_empty() {
            tr.pk_theme_default.to_owned()
        } else {
            crate::theme::THEMES.iter().find(|t| t.id == self.theme).map_or(self.theme.clone(), |t| t.name.to_owned())
        }
    }

    fn option_row(&mut self, ui: &mut egui::Ui, tr: &Tr, t: &TemplateDescriptor, o: &OptionDescriptor, field_w: f32) {
        let label = option_label(tr, t, o);
        match o.kind {
            OptionKind::Flag => {
                form_row(ui, label, |ui| {
                    let mut on = self.flag(o.id);
                    if ui.checkbox(&mut on, "").changed() {
                        self.set_option(o.id, if on { "true" } else { "false" });
                    }
                });
            }
            OptionKind::Choice(choices) => {
                let current = self.option(o.id).to_owned();
                let shown = choices.iter().find(|c| c.id == current).map_or(current.as_str(), |c| choice_label(tr, o.id, c)).to_owned();
                form_row(ui, label, |ui| {
                    egui::ComboBox::from_id_salt(("npd_opt", o.id)).width(field_w).selected_text(shown).show_ui(ui, |ui| {
                        for c in choices {
                            if ui.selectable_label(current == c.id, choice_label(tr, o.id, c)).clicked() {
                                self.set_option(o.id, c.id);
                            }
                        }
                    });
                });
            }
        }
    }

    /// The right column for Recent Projects.
    fn recent_pane(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, action: &mut DialogAction) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr.pk_recent).size(SZ_TITLE).strong());
        ui.separator();
        egui::ScrollArea::vertical().id_salt("npd_recent").auto_shrink([false, false]).show(ui, |ui| {
            if self.recents.is_empty() {
                ui.label(egui::RichText::new(tr.pk_recent_empty).size(SZ_BODY).color(theme.text_dim));
            }
            for row in &self.recents {
                let text = if row.exists { row.name.clone() } else { format!("{} ({})", row.name, tr.pk_recent_missing) };
                let response = ui
                    .add_enabled(row.exists, egui::Button::new(egui::RichText::new(text).size(SZ_BODY)).frame(false))
                    .on_hover_text(format!("{} — {}", tr.pk_recent_open_hover, row.manifest.display()));
                if response.clicked() {
                    action.open_recent = Some(row.manifest.clone());
                }
            }
        });
    }

    /// The resize grip, in its own foreground [`egui::Area`] pinned to the
    /// window's outer bottom-right corner. Its drag delta is the one and only
    /// writer of `self.size`.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("new_project_resize_grip"))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.drag(response.drag_delta());
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
                }
                let colour = if response.hovered() || response.dragged() { theme.accent } else { theme.text_dim };
                let painter = ui.painter();
                for (i, offset) in [2.0_f32, 6.0, 10.0].iter().enumerate() {
                    let len = GRIP - offset - 1.0;
                    if len <= 0.0 {
                        continue;
                    }
                    let stroke = egui::Stroke::new(if i == 0 { 1.6 } else { 1.2 }, colour);
                    painter.line_segment(
                        [egui::pos2(rect.right() - len, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - len)],
                        stroke,
                    );
                }
            });
    }
}

/// What the *Target platform* line says for a template: the host for a Desktop
/// one, the platform name for a Mobile one, the browsers for a Web one.
fn target_text(t: &TemplateDescriptor) -> &'static str {
    if t.kind.is_desktop() {
        host_platform()
    } else {
        t.platforms
    }
}

/// One row of the property list: the label in a cell of fixed width (wrapped
/// within it, so a long translation takes a second line rather than more room),
/// then the field.
fn form_row(ui: &mut egui::Ui, label: &str, field: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(LABEL_W, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_max_width(LABEL_W);
            ui.add(egui::Label::new(egui::RichText::new(label).size(SZ_BODY)).wrap());
        });
        field(ui);
    });
    ui.add_space(4.0);
}

/// A read-only line of text that is cut at the room it has instead of asking
/// for more.
fn clipped(text: String) -> egui::Label {
    egui::Label::new(egui::RichText::new(text).size(SZ_BODY)).truncate()
}

/// A square button that paints the catalogue's folder icon.
fn folder_button(ui: &mut egui::Ui, theme: &Theme, hover: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 24.0), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme.bg_hover);
    }
    cobolt_forms::icons::draw_menu_icon(ui.painter(), rect.shrink2(egui::vec2(5.0, 3.0)), "doc-open", theme.text_bright);
    response.on_hover_text(hover).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::ProjectKind;

    fn dialog() -> NewProjectDialog {
        let mut d = NewProjectDialog::new();
        d.open_with("/tmp/projects", Vec::new());
        d
    }

    fn index_of(d: &NewProjectDialog, id: &str) -> usize {
        d.catalogue().iter().position(|t| t.id == id).unwrap()
    }

    /// WASM-001 AC1: selecting each template changes the title and the options shown.
    #[test]
    fn each_template_changes_the_title_and_the_options() {
        let mut d = dialog();
        let tr = Language::English.tr();
        assert_eq!(d.title(&tr), "New Desktop Project", "the mock-up's own title");
        let mut seen: Vec<(String, Vec<&str>)> = Vec::new();
        for i in 0..d.catalogue().len() {
            d.select(i);
            let ids: Vec<&str> = d.options_shown().iter().map(|o| o.id).collect();
            seen.push((d.title(&tr), ids));
        }
        let titles: std::collections::BTreeSet<&String> = seen.iter().map(|(t, _)| t).collect();
        assert_eq!(titles.len(), d.catalogue().len(), "twelve templates, twelve titles: {seen:?}");
        let by = |title: &str| seen.iter().find(|(t, _)| t == title).unwrap().1.clone();
        assert!(by("New Desktop Project").contains(&"high_dpi") && !by("New Desktop Project").contains(&"orientation"));
        assert!(by("New Android (Cellphone) Project").contains(&"orientation"));
        assert!(by("New Web (WebAssembly) Project").contains(&"style") && !by("New Web (WebAssembly) Project").contains(&"high_dpi"));
        d.show_recent();
        assert_eq!(d.title(&tr), "Recent Projects");
        assert!(d.options_shown().is_empty());
        println!("new project: {} templates, {} distinct titles; options shown per template follow the descriptor", d.catalogue().len(), titles.len());
    }

    /// WASM-001 R5 / AC1: an unavailable template is listed and cannot be created.
    #[test]
    fn an_unavailable_template_cannot_be_created() {
        let mut d = dialog();
        for id in ["chatbot-desktop", "chatbot-web", "usercontrol-desktop", "usercontrol-web", "android-watch", "apple-watch"] {
            let i = index_of(&d, id);
            d.select(i);
            assert_eq!(d.ready().unwrap_err(), Problem::Unavailable, "{id}");
            assert!(d.press_create().is_none());
            assert_eq!(d.problem(), Some(&Problem::Unavailable));
        }
        for id in ["desktop", "web", "android-phone", "android-tablet", "iphone", "ipad"] {
            let i = index_of(&d, id);
            d.select(i);
            d.location = std::env::temp_dir().join("prc-npd-none").display().to_string();
            assert!(d.ready().is_ok(), "{id} can be created: {:?}", d.ready().err());
        }
    }

    /// A template change resets the options and keeps what was typed.
    #[test]
    fn selecting_a_template_resets_its_options_and_keeps_the_common_fields() {
        let mut d = dialog();
        d.name = "Shop".into();
        d.description = "A shop".into();
        d.select(index_of(&d, "web"));
        d.set_option("style", "standalone");
        d.set_option("backend", "false");
        assert_eq!(d.option("style"), "standalone");
        d.select(index_of(&d, "android-tablet"));
        assert_eq!(d.option("style"), "spa", "the defaults of the new template");
        assert_eq!(d.option("orientation"), "auto");
        assert!(d.flag("backend"));
        assert_eq!((d.name.as_str(), d.description.as_str()), ("Shop", "A shop"), "typed fields stay");
    }

    #[test]
    fn the_search_keeps_what_matches_and_the_selection_survives_it() {
        let mut d = dialog();
        d.set_search("android");
        let ids: Vec<&str> = d.visible().into_iter().map(|i| d.catalogue()[i].id).collect();
        assert_eq!(ids, ["android-phone", "android-tablet", "android-watch"]);
        d.set_search("ZZZ");
        assert!(d.visible().is_empty());
        d.set_search("");
        assert_eq!(d.visible().len(), d.catalogue().len());
    }

    #[test]
    fn the_first_thing_wrong_is_the_one_reported() {
        let mut d = dialog();
        d.location = std::env::temp_dir().join("prc-npd-problems").display().to_string();
        d.name = "  ".into();
        assert_eq!(d.ready().unwrap_err(), Problem::Name);
        d.name = "a/b".into();
        assert_eq!(d.ready().unwrap_err(), Problem::NameChars);
        d.name = "Shop".into();
        d.version = "1.0".into();
        assert_eq!(d.ready().unwrap_err(), Problem::Version);
        d.version = "1.0.0".into();
        d.location = String::new();
        assert_eq!(d.ready().unwrap_err(), Problem::Location);
        d.location = "relative/path".into();
        assert_eq!(d.ready().unwrap_err(), Problem::Location, "a location is absolute");
        d.location = std::env::temp_dir().join("prc-npd-problems").display().to_string();
        d.main_file = "../x.cbl".into();
        assert_eq!(d.ready().unwrap_err(), Problem::Entry);
        d.main_file = "src/main.cbl".into();
        assert!(d.ready().is_ok());
        for (v, ok) in [("1.0.0", true), ("10.20.300", true), ("1.0", false), ("1.0.0.0", false), ("a.b.c", false), ("1..0", false), ("", false)] {
            assert_eq!(version_ok(v), ok, "{v:?}");
        }
    }

    /// The fields become the request New Project and `create_project` share.
    #[test]
    fn the_fields_make_the_request() {
        let mut d = dialog();
        d.name = "Inventory".into();
        d.version = "2.1.0".into();
        d.description = "Stock control".into();
        d.theme = "spatial".into();
        d.locale = "pt-BR".into();
        d.location = std::env::temp_dir().join("prc-npd-request").display().to_string();
        d.select(index_of(&d, "ipad"));
        d.set_option("orientation", "landscape");
        d.set_option("min_os", "17.0");
        let ready = d.ready().unwrap();
        let r = &ready.request;
        assert_eq!((r.name.as_str(), r.template.as_str(), r.version.as_str()), ("Inventory", "ipad", "2.1.0"));
        assert_eq!((r.theme.as_str(), r.locale.as_str(), r.description.as_str()), ("spatial", "pt-BR", "Stock control"));
        assert_eq!(r.options.get("orientation").map(String::as_str), Some("landscape"));
        assert!(r.main.is_empty(), "a Mobile project's entry is its main form");
        assert!(ready.dir.ends_with("Inventory"), "{}", ready.dir.display());
        // The plan the request makes is a Mobile iPad project.
        let plan = create::plan(r, d.catalogue()).unwrap();
        assert_eq!(plan.kind, ProjectKind::Mobile);
        assert_eq!(plan.main, "mobileforms/main-form.mcfrm");
        assert_eq!(d.entry_shown(), plan.main, "the dialog shows the entry the plan writes");
        assert!(plan.manifest.contains("platform = \"ios\"") && plan.manifest.contains("min_os = \"17.0\""));
        // The Desktop request carries the entry only when it is not the default.
        d.select(index_of(&d, "desktop"));
        assert!(d.request().main.is_empty());
        d.main_file = "src/shop.cbl".into();
        assert_eq!(d.request().main, "src/shop.cbl");
    }

    #[test]
    fn a_project_already_in_the_folder_is_not_overwritten() {
        let base = std::env::temp_dir().join(format!("prc-npd-exists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mut d = dialog();
        d.name = "Taken".into();
        d.location = base.display().to_string();
        let ready = d.ready().unwrap();
        let plan = create::plan(&ready.request, d.catalogue()).unwrap();
        plan.write_into(&ready.dir, &plan.manifest_name).unwrap();
        match d.ready() {
            Err(Problem::Exists(path)) => assert!(path.ends_with("Taken.project.toml"), "{path}"),
            other => panic!("expected Exists, got {other:?}"),
        }
        let tr = Language::English.tr();
        assert!(Problem::Exists("x".into()).message(&tr).contains("already holds a project: x"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// WASM-001 AC5: a template is added by adding a descriptor — here one the
    /// `Tr` table has never heard of — and the dialog lists it, asks for its
    /// options and builds its request.
    #[test]
    fn a_new_template_appears_with_its_options_and_no_change_to_the_dialog() {
        const SIZES: &[templates::Choice] = &[
            templates::Choice { id: "small", label: "Small" },
            templates::Choice { id: "large", label: "Large" },
        ];
        const OPTIONS: &[OptionDescriptor] = &[
            OptionDescriptor { id: "fizz", label: "Fizz level", kind: OptionKind::Choice(SIZES), default: "small" },
            OptionDescriptor { id: "shiny", label: "Make it shiny", kind: OptionKind::Flag, default: "true" },
        ];
        let mut catalogue = templates::builtin();
        catalogue.push(TemplateDescriptor {
            id: "fake",
            name: "Fake Template",
            title_noun: "Fake",
            platforms: "Nowhere",
            summary: "Exists only in a test.",
            kind: ProjectKind::Web,
            mobile: None,
            creatable: true,
            options: OPTIONS,
        });
        let mut d = NewProjectDialog::with_catalogue(catalogue);
        d.open_with("/tmp/p", Vec::new());
        let i = d.catalogue().len() - 1;
        d.set_search("fake");
        assert_eq!(d.visible(), [i], "listed and searchable");
        d.select(i);
        assert_eq!(d.options_shown().iter().map(|o| o.id).collect::<Vec<_>>(), ["fizz", "shiny"]);
        assert_eq!((d.option("fizz"), d.flag("shiny")), ("small", true), "its own defaults");
        let tr = Language::English.tr();
        assert_eq!(d.title(&tr), "New Fake Template Project", "no Tr words: the descriptor's English");
        assert!(!template_has_words("fake"));
        let t = d.template().copied().unwrap();
        assert_eq!(option_label(&tr, &t, &OPTIONS[0]), "Fizz level");
        assert_eq!(choice_label(&tr, "fizz", &SIZES[1]), "Large");
        d.set_option("fizz", "large");
        d.location = std::env::temp_dir().join("prc-npd-fake").display().to_string();
        let ready = d.ready().unwrap();
        assert_eq!(ready.request.template, "fake");
        assert_eq!(ready.request.options.get("fizz").map(String::as_str), Some("large"));
    }

    /// Every template of the built-in catalogue has words in `Tr`, every option
    /// has a label, and every choice either has one or is a version name.
    #[test]
    fn the_builtin_catalogue_is_fully_worded_in_every_language() {
        let version_names = [
            "Android 7.0 (API 24)",
            "Android 8.0 (API 26)",
            "Android 10 (API 29)",
            "Android 12 (API 31)",
            "Android 14 (API 34)",
            "iOS 15.0",
            "iOS 16.0",
            "iOS 17.0",
            "iOS 18.0",
        ];
        let mut checked = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            for t in templates::builtin() {
                assert!(template_has_words(t.id), "{}: no Tr words", t.id);
                let (name, sentence) = template_text(&tr, &t);
                assert!(!name.trim().is_empty() && !sentence.trim().is_empty(), "{lang:?} {}", t.id);
                for o in t.options {
                    let label = option_label(&tr, &t, o);
                    assert!(!label.trim().is_empty());
                    if *lang == Language::English {
                        assert_eq!(label, o.label, "{}.{}: Tr English equals the descriptor's", t.id, o.id);
                    }
                    if let OptionKind::Choice(choices) = o.kind {
                        for c in choices {
                            let shown = choice_label(&tr, o.id, c);
                            assert!(!shown.trim().is_empty());
                            if shown == c.label {
                                // Either the descriptor's wording is the Tr wording (English),
                                // or it is a name that is not translated.
                                assert!(
                                    *lang == Language::English
                                        || version_names.contains(&c.label)
                                        || c.label == "PowerUI"
                                        // "Portrait" is the French word as well.
                                        || (*lang == Language::French && c.label == "Portrait"),
                                    "{lang:?} {}.{}.{}: {:?} is not translated",
                                    t.id,
                                    o.id,
                                    c.id,
                                    c.label
                                );
                            }
                            if *lang == Language::English {
                                assert_eq!(shown, c.label, "{}.{}.{}: Tr English equals the descriptor's", t.id, o.id, c.id);
                            }
                            checked += 1;
                        }
                    }
                    checked += 1;
                }
                if *lang == Language::English {
                    assert_eq!(name, t.name, "{}: Tr English name equals the descriptor's", t.id);
                    assert_eq!(sentence, t.summary, "{}: Tr English sentence equals the descriptor's", t.id);
                }
                checked += 2;
            }
        }
        println!("new project: {checked} template/option/choice wordings checked across {} languages", Language::ALL.len());
    }

    /// Every icon the list names exists in the catalogue.
    #[test]
    fn the_icons_of_the_list_exist_in_the_catalogue() {
        let known: std::collections::BTreeSet<&str> =
            cobolt_forms::icons::MENU_ICON_CATEGORIES.iter().flat_map(|(_, names)| names.iter().copied()).collect();
        for t in templates::builtin() {
            let icon = template_icon(t.id);
            assert!(known.contains(icon), "{}: icon {icon:?} is not in the catalogue", t.id);
            assert_ne!(icon, "doc-blank", "{}: has no icon of its own", t.id);
        }
        for icon in ["doc-open", "doc-history"] {
            assert!(known.contains(icon), "{icon}");
        }
    }

    /// Every template's icon has a colour of its own (none is the old white
    /// ink), and the families the reference design separates stay apart.
    #[test]
    fn every_template_icon_has_its_family_colour() {
        let white = egui::Color32::WHITE;
        for t in templates::builtin() {
            let c = template_tint(t.id);
            assert_ne!(c, white, "{}", t.id);
            let [r, g, b, _] = c.to_array();
            assert!(r.max(g).max(b) - r.min(g).min(b) > 60, "{}: {c:?} is too grey to read as a colour", t.id);
        }
        assert_eq!(template_tint("android-phone"), template_tint("android-tablet"));
        assert_ne!(template_tint("android-phone"), template_tint("iphone"));
        assert_ne!(template_tint("desktop"), template_tint("web"));
        println!("template icons: {} templates, each in its family colour", templates::builtin().len());
    }

    #[test]
    fn locales_are_the_ides_six_languages() {
        assert_eq!(LOCALES.len(), Language::ALL.len());
        assert_eq!(locale_label("pt-BR"), "Português (pt-BR)");
        assert_eq!(locale_label("en-US"), "English (en-US)");
        assert_eq!(locale_label("xx"), "xx");
    }

    #[test]
    fn recent_rows_know_their_name_and_whether_they_exist() {
        let r = RecentRow::new(PathBuf::from("/definitely/not/here/Inventory.project.toml"));
        assert_eq!((r.name.as_str(), r.exists), ("Inventory", false));
        let r = RecentRow::new(PathBuf::from("/x/Plain.toml"));
        assert_eq!(r.name, "Plain");
    }

    // ── The window itself: never resizes itself ───────────────────────────

    fn frames(lang: Language, pane: Pane, n: usize) -> Vec<(egui::Vec2, egui::Rect)> {
        let ctx = egui::Context::default();
        let theme = crate::theme::default_theme();
        crate::app::apply_glass_visuals(&ctx, theme);
        let tr = lang.tr();
        let mut d = dialog();
        d.recents = vec![RecentRow { manifest: PathBuf::from("/x/Some very long project name.project.toml"), name: "Some very long project name".into(), exists: true }];
        match pane {
            Pane::Template(i) => d.select(i),
            Pane::Recent => d.show_recent(),
        }
        d.name = "A very long project name that someone typed into the box".into();
        d.description = "A description that is long enough to want a second line if the box would let it".into();
        d.problem = Some(Problem::Entry);
        let mut seen = Vec::new();
        for _ in 0..n {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(3000.0, 2000.0)));
            ctx.begin_pass(input);
            d.show(&ctx, theme, &tr);
            ctx.end_pass().textures_delta.clear();
            let rect = ctx.memory(|m| m.area_rect(egui::Id::new("new_project_dialog"))).expect("the window registered an area");
            seen.push((d.size, rect));
        }
        seen
    }

    /// The window opens at its size and stays there while nobody touches it,
    /// in every language, on every kind of pane (the golden rule; the same
    /// test the leaderboard modal carries).
    #[test]
    fn the_window_does_not_grow_while_nobody_touches_it() {
        let mut panes: Vec<Pane> = (0..templates::builtin().len()).map(Pane::Template).collect();
        panes.push(Pane::Recent);
        let mut frames_run = 0;
        for lang in Language::ALL {
            for pane in &panes {
                let seen = frames(*lang, *pane, 12);
                frames_run += seen.len();
                for (size, _) in &seen {
                    assert_eq!(*size, egui::vec2(DEFAULT_W, DEFAULT_H), "[{lang:?} {pane:?}] the stored size moved with no drag");
                }
                let (_, first) = seen[2];
                for (_, rect) in &seen[2..] {
                    assert!(
                        (rect.width() - first.width()).abs() < 0.5 && (rect.height() - first.height()).abs() < 0.5,
                        "[{lang:?} {pane:?}] the window kept changing: {:?} then {:?}",
                        first.size(),
                        rect.size()
                    );
                }
                let (_, last) = seen[seen.len() - 1];
                assert!(
                    (last.width() - DEFAULT_W).abs() < 0.5,
                    "[{lang:?} {pane:?}] wider than its fixed size: {:.1} (the content is the driver)",
                    last.width()
                );
                assert!(
                    last.height() <= DEFAULT_H + 0.5,
                    "[{lang:?} {pane:?}] taller than its fixed size: {:.1}",
                    last.height()
                );
            }
        }
        println!("new project window: {frames_run} frames over 6 languages x {} panes; the rect stayed {}x{}", panes.len(), DEFAULT_W, DEFAULT_H);
    }

    #[test]
    fn the_size_only_changes_by_a_drag_and_stops_at_its_limits() {
        let mut d = dialog();
        assert_eq!(d.size, egui::vec2(DEFAULT_W, DEFAULT_H));
        d.drag(egui::vec2(100.0, 60.0));
        assert_eq!(d.size, egui::vec2(DEFAULT_W + 100.0, DEFAULT_H + 60.0));
        d.drag(egui::vec2(-9000.0, -9000.0));
        assert_eq!(d.size, egui::vec2(MIN_W, MIN_H));
        d.drag(egui::vec2(9000.0, 9000.0));
        assert_eq!(d.size, egui::vec2(MAX_W, MAX_H));
    }
}
