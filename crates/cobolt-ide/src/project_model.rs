// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt project model — `cobolt.toml` load/save/package.
//!
//! A project is a single `cobolt.toml` file that records every file belonging
//! to it.  Packaging collects those files into a self-contained `.zip` that
//! can be unpacked and run with the bundled `cobolt` CLI binary.
//!
//! # `cobolt.toml` example
//!
//! ```toml
//! [project]
//! name    = "MyApp"
//! version = "1.0.0"
//! main    = "src/main.cbl"
//!
//! [files]
//! sources = ["src/main.cbl", "src/helpers.cbl"]
//! forms   = ["forms/main-form.cfrm", "forms/login.cfrm"]
//! assets  = ["images/logo.png"]
//!
//! [runtime]
//! fixed_format = false
//! ```

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoboltProject {
    pub project: ProjectMeta,
    #[serde(default)]
    pub files: ProjectFiles,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// Per-project IDE appearance (colour theme + background image).
    #[serde(default)]
    pub ide: IdeSettings,
    /// Per-project form appearance — the default **form** theme (spec 007).
    #[serde(default)]
    pub forms: FormsConfig,
    /// Project-scoped AI models and behavior. Credentials are deliberately not
    /// part of this structure; they remain in the machine-local secret store.
    #[serde(default)]
    pub ai: ProjectAiSettings,
    /// Project-scoped reusable composite controls (spec 020).
    #[serde(default, rename = "user-controls")]
    pub user_controls: Vec<UserControlDef>,
    /// Project-scoped third-party integration settings (spec 039) — the
    /// non-secret half only; the google_maps and Custom Search API keys
    /// stay in the machine-local secret store like every other credential
    /// (R31), never here.
    #[serde(default)]
    pub integrations: ProjectIntegrationSettings,
}

/// Non-secret configuration for the spec 039 Maps/WebSearch controls'
/// external services. `search_engine_id` (Custom Search's "cx" value) is
/// not a credential — Google's own docs treat it as a public identifier,
/// scoped by the API key that accompanies each request — so it round-trips
/// in `cobolt.toml` like any other project setting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectIntegrationSettings {
    #[serde(default)]
    pub google_search_engine_id: String,
}

/// AI configuration that belongs to one project and is persisted in
/// `cobolt.toml`. API keys are intentionally excluded and resolve from the
/// machine-local store by the stable model-profile id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiSettings {
    /// Zero means the project predates project-scoped AI settings and should
    /// receive a conservative one-time import from the legacy global config.
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub endpoint_user_edited: bool,
    #[serde(default)]
    pub model: String,
    #[serde(default = "crate::llm::default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "crate::llm::default_cobol_proficiency_prompt")]
    pub cobol_proficiency_prompt: String,
    #[serde(default = "crate::llm::default_temperature")]
    pub temperature: f32,
    #[serde(default = "crate::llm::default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "crate::llm::default_timeout_secs")]
    pub timeout_secs: u32,
    #[serde(default)]
    pub verbose_log: bool,
    #[serde(default = "crate::llm::default_max_review_revisions")]
    pub max_review_revisions: u32,
    #[serde(default = "crate::llm::default_unreviewed_temperature")]
    pub unreviewed_temperature: Option<f32>,
    #[serde(default = "crate::llm::default_agentic_ai_enabled")]
    pub agentic_ai_enabled: bool,
    #[serde(default)]
    pub reviewer_provider: String,
    #[serde(default)]
    pub reviewer_endpoint: String,
    #[serde(default)]
    pub reviewer_model: String,
    #[serde(default = "crate::llm::default_pedantic_prompt")]
    pub pedantic_prompt: String,
    #[serde(default = "crate::llm::default_pedantic_ui_prompt")]
    pub pedantic_ui_prompt: String,
    #[serde(default = "crate::llm::default_pedantic_event_prompt")]
    pub pedantic_event_prompt: String,
    #[serde(default)]
    pub model_profiles: Vec<crate::llm::ModelProfile>,
}

impl Default for ProjectAiSettings {
    fn default() -> Self {
        Self {
            schema_version: 0,
            provider: String::new(),
            endpoint: String::new(),
            endpoint_user_edited: false,
            model: String::new(),
            system_prompt: crate::llm::default_system_prompt(),
            cobol_proficiency_prompt: crate::llm::default_cobol_proficiency_prompt(),
            temperature: crate::llm::default_temperature(),
            max_tokens: crate::llm::default_max_tokens(),
            timeout_secs: crate::llm::default_timeout_secs(),
            verbose_log: false,
            max_review_revisions: crate::llm::default_max_review_revisions(),
            unreviewed_temperature: crate::llm::default_unreviewed_temperature(),
            agentic_ai_enabled: true,
            reviewer_provider: String::new(),
            reviewer_endpoint: String::new(),
            reviewer_model: String::new(),
            pedantic_prompt: crate::llm::default_pedantic_prompt(),
            pedantic_ui_prompt: crate::llm::default_pedantic_ui_prompt(),
            pedantic_event_prompt: crate::llm::default_pedantic_event_prompt(),
            model_profiles: Vec::new(),
        }
    }
}

impl ProjectAiSettings {
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            ..Self::default()
        }
    }

    pub fn from_llm(llm: &crate::llm::LlmConfig) -> Self {
        Self {
            schema_version: 1,
            provider: llm.provider.clone(),
            endpoint: llm.endpoint.clone(),
            endpoint_user_edited: llm.endpoint_user_edited,
            model: llm.model.clone(),
            system_prompt: llm.system_prompt.clone(),
            cobol_proficiency_prompt: llm.cobol_proficiency_prompt.clone(),
            temperature: llm.temperature,
            max_tokens: llm.max_tokens,
            timeout_secs: llm.timeout_secs,
            verbose_log: llm.verbose_log,
            max_review_revisions: llm.max_review_revisions,
            unreviewed_temperature: llm.unreviewed_temperature,
            agentic_ai_enabled: llm.agentic_ai_enabled,
            reviewer_provider: llm.reviewer_provider.clone(),
            reviewer_endpoint: llm.reviewer_endpoint.clone(),
            reviewer_model: llm.reviewer_model.clone(),
            pedantic_prompt: llm.pedantic_prompt.clone(),
            pedantic_ui_prompt: llm.pedantic_ui_prompt.clone(),
            pedantic_event_prompt: llm.pedantic_event_prompt.clone(),
            model_profiles: llm.model_profiles.clone(),
        }
    }

    pub fn apply_to_llm(&self, llm: &mut crate::llm::LlmConfig) {
        llm.provider = self.provider.clone();
        llm.endpoint = self.endpoint.clone();
        llm.endpoint_user_edited = self.endpoint_user_edited
            || (!self.endpoint.trim().is_empty()
                && !crate::llm::endpoint_is_provider_default(&self.provider, &self.endpoint));
        llm.model = self.model.clone();
        llm.system_prompt = self.system_prompt.clone();
        llm.cobol_proficiency_prompt = self.cobol_proficiency_prompt.clone();
        llm.temperature = self.temperature;
        llm.max_tokens = self.max_tokens;
        llm.timeout_secs = self.timeout_secs;
        llm.verbose_log = self.verbose_log;
        llm.max_review_revisions = self.max_review_revisions;
        llm.unreviewed_temperature = self.unreviewed_temperature;
        llm.agentic_ai_enabled = self.agentic_ai_enabled;
        llm.reviewer_provider = self.reviewer_provider.clone();
        llm.reviewer_endpoint = self.reviewer_endpoint.clone();
        llm.reviewer_model = self.reviewer_model.clone();
        llm.pedantic_prompt = self.pedantic_prompt.clone();
        llm.pedantic_ui_prompt = self.pedantic_ui_prompt.clone();
        llm.pedantic_event_prompt = self.pedantic_event_prompt.clone();
        llm.model_profiles = self.model_profiles.clone();
        // Profile-only projects (a model set per-agent but no top-level default,
        // e.g. Form Designer → ollama) must still drive the direct AI surfaces,
        // which call the top-level model. Adopt a usable profile as the default
        // before resolving the API key so the key matches (spec 031).
        llm.ensure_default_model_from_profiles();
        llm.api_key = llm
            .model_profiles
            .iter()
            .find(|profile| profile.provider == llm.provider && profile.model == llm.model)
            .map(|profile| profile.resolve(llm).api_key)
            .filter(|key| !key.is_empty())
            .or_else(|| {
                llm.api_keys
                    .get(&crate::llm::api_key_slot(&llm.provider, &llm.model))
                    .cloned()
            })
            .unwrap_or_default();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserControlDef {
    pub name: String,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub controls: Vec<UserControlEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserControlEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub control_type: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub z_order: i32,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// Per-project form appearance settings (spec 007). Distinct from [`IdeSettings`]
/// (which themes the IDE chrome); this is the default **form** theme applied to
/// the developer's designed forms, persisted in `cobolt.toml` under `[forms]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormsConfig {
    /// Default form-theme id (a `cobolt_forms::theme` catalog id). Empty / absent
    /// ⇒ Liquid Glass, so existing projects render exactly as before (R3, R9).
    #[serde(default)]
    pub theme: String,

    // ── 038 Window effects — PROJECT-level, applied to every form ──────────
    /// Entrance effect id (`cobolt_forms::window_fx::WindowEffect` id).
    /// Empty/absent ⇒ None, so pre-038 projects are unchanged (038 R5).
    #[serde(default, rename = "entrance-effect")]
    pub entrance_effect: String,
    #[serde(default = "FormsConfig::default_entrance_ms", rename = "entrance-ms")]
    pub entrance_ms: u32,
    #[serde(default, rename = "entrance-easing")]
    pub entrance_easing: String,
    /// Exit effect id; empty/absent ⇒ None.
    #[serde(default, rename = "exit-effect")]
    pub exit_effect: String,
    #[serde(default = "FormsConfig::default_exit_ms", rename = "exit-ms")]
    pub exit_ms: u32,
    #[serde(default, rename = "exit-easing")]
    pub exit_easing: String,
    /// Replay the entrance when a window is restored after minimize (038 R2).
    #[serde(default, rename = "entrance-on-restore")]
    pub entrance_on_restore: bool,
}

impl FormsConfig {
    fn default_entrance_ms() -> u32 {
        600
    }
    fn default_exit_ms() -> u32 {
        400
    }

    /// The defaults a NEWLY CREATED project starts with (038 R5): Matrix
    /// entrance, no exit effect, no restore replay. Deliberately different
    /// from [`Default`], which is the serde fallback for projects whose
    /// `cobolt.toml` predates 038 and therefore must stay effect-free.
    pub fn new_project_defaults() -> Self {
        Self {
            entrance_effect: "matrix-rain".into(),
            // Matrix-rain has its own 1500–4000 ms band (fly-through).
            entrance_ms: 2000,
            ..Self::default()
        }
    }
}

/// Serde fallback for a missing `[forms]` section or missing fields: NO
/// effects, so pre-038 projects behave exactly as before (038 R5).
impl Default for FormsConfig {
    fn default() -> Self {
        Self {
            theme: String::new(),
            entrance_effect: String::new(),
            entrance_ms: Self::default_entrance_ms(),
            entrance_easing: String::new(),
            exit_effect: String::new(),
            exit_ms: Self::default_exit_ms(),
            exit_easing: String::new(),
            entrance_on_restore: false,
        }
    }
}

impl CoboltProject {
    /// The project's default form theme as an `Option`, treating empty as unset
    /// so it resolves to Liquid Glass (R3).
    pub fn form_theme_default(&self) -> Option<&str> {
        let t = self.forms.theme.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }

    /// The project's window ENTRANCE effect as a typed spec (038 R1).
    /// Durations clamp into the EFFECT's own bounds (matrix-rain runs
    /// 1500–4000 ms, everything else 100–3000 ms).
    pub fn entrance_fx(&self) -> cobolt_forms::window_fx::FxSpec {
        use cobolt_forms::window_fx::{Easing, FxSpec, WindowEffect};
        let effect = WindowEffect::from_str(&self.forms.entrance_effect);
        let (min_ms, max_ms) = effect.duration_bounds();
        FxSpec {
            effect,
            duration_ms: self.forms.entrance_ms.clamp(min_ms, max_ms),
            easing: Easing::from_str(&self.forms.entrance_easing),
        }
    }

    /// The project's window EXIT effect as a typed spec (038 R1).
    pub fn exit_fx(&self) -> cobolt_forms::window_fx::FxSpec {
        use cobolt_forms::window_fx::{Easing, FxSpec, WindowEffect};
        let effect = WindowEffect::from_str(&self.forms.exit_effect);
        let (min_ms, max_ms) = effect.duration_bounds();
        FxSpec {
            effect,
            duration_ms: self.forms.exit_ms.clamp(min_ms, max_ms),
            // Exits default to ease-in (accelerating away) when unset.
            easing: if self.forms.exit_easing.trim().is_empty() {
                cobolt_forms::window_fx::Easing::EaseIn
            } else {
                Easing::from_str(&self.forms.exit_easing)
            },
        }
    }
}

/// Per-project IDE appearance settings (colour theme + background image),
/// persisted in `cobolt.toml` so the look travels with the project.
///
/// Debug switches used to live here too; they are machine-local developer aids,
/// not project data, so they moved to
/// [`DebugSettings`](crate::debug_settings::DebugSettings) (Help → Debug
/// Settings). Their old keys in an existing `cobolt.toml` are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeSettings {
    /// Colour-theme id (see `crate::theme`). Empty / unknown → default theme.
    #[serde(default)]
    pub theme: String,
    /// Optional background image (relative to the project root, or absolute).
    #[serde(default)]
    pub background_image: String,
    /// Optional project icon image for Run Form / built app windows.
    #[serde(default)]
    pub project_icon: String,
    /// Background-image opacity, 0 (invisible) … 100 (fully opaque).
    #[serde(default = "default_bg_opacity")]
    pub background_opacity: u8,
    /// Run-Form inspector: when it detects suspicious behaviour, also write a
    /// process/memory dump to `inspector_dump_path` (console output is always on).
    #[serde(default = "default_true")]
    pub inspector_dump_enabled: bool,
    /// Where the inspector writes its dump file.
    #[serde(default = "default_inspector_dump_path")]
    pub inspector_dump_path: String,
    /// Suppress the "set up AI" invitation shown when the project is opened with
    /// no usable model or no configured agent. Set by the modal's
    /// "don't show again" checkbox.
    #[serde(default)]
    pub hide_ai_setup_prompt: bool,
}

fn default_bg_opacity() -> u8 {
    70
}

fn default_true() -> bool {
    true
}

/// Default dump location. Resolved per platform (`/tmp` on Linux/macOS,
/// `%TEMP%` on Windows) rather than hardcoded POSIX — a project created on
/// Windows would otherwise carry a path nothing can write to. An existing
/// project keeps whatever path it already has.
fn default_inspector_dump_path() -> String {
    cobolt_runtime::diag_path::diagnostics_file("prc_inspector_dump.txt")
        .display()
        .to_string()
}

fn default_debug_compilation() -> bool {
    true
}

impl Default for IdeSettings {
    fn default() -> Self {
        Self {
            theme: String::new(),
            background_image: String::new(),
            project_icon: String::new(),
            background_opacity: default_bg_opacity(),
            inspector_dump_enabled: default_true(),
            inspector_dump_path: default_inspector_dump_path(),
            hide_ai_setup_prompt: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    /// Semantic version `major.minor.fix` (the three parts are edited
    /// separately in the Settings form and recomposed here).
    pub version: String,
    /// Relative path (from project root) to the main COBOL source file.
    pub main: String,
    /// Custom copyright line embedded in generated headers / distributions.
    #[serde(default)]
    pub copyright: String,
    /// Short license identifier (e.g. "MIT", "Apache-2.0", "Proprietary").
    #[serde(default)]
    pub license_model: String,
    /// Full license text (editable).
    #[serde(default)]
    pub license_text: String,
    /// Destination folder of the project.
    #[serde(default)]
    pub destination_folder: String,
    /// Is this a debug or release compilation
    #[serde(default = "default_debug_compilation")]
    pub debug_compilation: bool,
}

impl ProjectMeta {
    /// Parse `version` into `(major, minor, fix)`, tolerating missing parts.
    pub fn version_parts(&self) -> (u32, u32, u32) {
        let mut it = self
            .version
            .split('.')
            .map(|s| s.trim().parse::<u32>().unwrap_or(0));
        (
            it.next().unwrap_or(1),
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
        )
    }
    /// Recompose `version` from its three parts.
    pub fn set_version_parts(&mut self, major: u32, minor: u32, fix: u32) {
        self.version = format!("{major}.{minor}.{fix}");
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectFiles {
    /// Hand-written pure COBOL-85 ("Common Code") — editable, CALLed by forms.
    #[serde(default)]
    pub sources: Vec<String>, // relative paths
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub assets: Vec<String>,
    /// Documentation files (Markdown, text, PDF, …).
    #[serde(default)]
    pub documentation: Vec<String>,
    /// RAD-generated COBOL (output of the form designer) — **read-only**.
    #[serde(default)]
    pub generated: Vec<String>,
    /// Indexed-file definitions (`.cidx`) — edited in the Indexed File Editor.
    #[serde(default)]
    pub indexed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Force fixed-format COBOL parsing (auto-detected when false).
    pub fixed_format: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            fixed_format: false,
        }
    }
}

impl CoboltProject {
    /// Create a blank project with sensible defaults.
    pub fn new(name: impl Into<String>, main: impl Into<String>) -> Self {
        let name_str = name.into();
        let destination_folder = if let Some(stripped) = name_str.strip_suffix(".project") {
            stripped.to_string()
        } else {
            name_str.clone()
        };
        Self {
            project: ProjectMeta {
                name: name_str,
                version: "1.0.0".into(),
                main: main.into(),
                copyright: String::new(),
                license_model: String::new(),
                license_text: String::new(),
                destination_folder,
                debug_compilation: true,
            },
            files: ProjectFiles::default(),
            runtime: RuntimeConfig::default(),
            ide: IdeSettings::default(),
            forms: FormsConfig::new_project_defaults(),
            ai: ProjectAiSettings::new(),
            user_controls: Vec::new(),
            integrations: ProjectIntegrationSettings::default(),
        }
    }

    // ── File membership helpers ───────────────────────────────────────────────

    /// Add a file to the appropriate list (deduplicates), routed by `Category`
    /// (so a `.cbl` can be added as Common Code even though its extension is the
    /// same as a generated file).
    pub fn add_file_to(&mut self, rel: &str, category: Category) {
        let rel = rel.replace('\\', "/");
        let list = self.list_mut(category);
        if !list.contains(&rel) {
            list.push(rel);
        }
    }

    /// Add a file, routing it to a category by its extension.
    pub fn add_file(&mut self, rel: &str) {
        self.add_file_to(rel, Category::of_path(rel));
    }

    /// Register a RAD-generated COBOL file (read-only). Also removes it from the
    /// editable Common Code list if it had been tracked there.
    pub fn add_generated(&mut self, rel: &str) {
        let rel = rel.replace('\\', "/");
        self.files.sources.retain(|f| f != &rel);
        if !self.files.generated.contains(&rel) {
            self.files.generated.push(rel);
        }
    }

    fn list_mut(&mut self, category: Category) -> &mut Vec<String> {
        match category {
            Category::Forms => &mut self.files.forms,
            Category::CommonCode => &mut self.files.sources,
            Category::Assets => &mut self.files.assets,
            Category::Documentation => &mut self.files.documentation,
            Category::Generated => &mut self.files.generated,
            Category::IndexedFiles => &mut self.files.indexed,
        }
    }

    /// Remove a file from whichever list it belongs to.
    pub fn remove_file(&mut self, rel: &str) {
        let rel = rel.replace('\\', "/");
        self.files.sources.retain(|f| f != &rel);
        self.files.forms.retain(|f| f != &rel);
        self.files.assets.retain(|f| f != &rel);
        self.files.documentation.retain(|f| f != &rel);
        self.files.generated.retain(|f| f != &rel);
        self.files.indexed.retain(|f| f != &rel);
    }

    /// Every tracked file list, mutably (spec 033 folder ops iterate all six).
    fn all_lists_mut(&mut self) -> [&mut Vec<String>; 6] {
        let ProjectFiles {
            sources,
            forms,
            assets,
            documentation,
            generated,
            indexed,
        } = &mut self.files;
        [sources, forms, assets, documentation, generated, indexed]
    }

    /// Rewrite every tracked path under the folder `old_dir` to sit under
    /// `new_dir` instead (folder rename/move). Both are project-relative,
    /// forward-slash directory paths without a trailing slash. All rewritten
    /// paths stay relative (spec 033, R4, R21).
    pub fn rename_prefix(&mut self, old_dir: &str, new_dir: &str) {
        let old_dir = old_dir.replace('\\', "/");
        let new_dir = new_dir.replace('\\', "/");
        if old_dir.is_empty() || old_dir == new_dir {
            return;
        }
        let prefix = format!("{old_dir}/");
        for list in self.all_lists_mut() {
            for entry in list.iter_mut() {
                if let Some(rest) = entry.strip_prefix(&prefix) {
                    *entry = format!("{new_dir}/{rest}");
                } else if *entry == old_dir {
                    // A tracked entry that *is* the directory (rare).
                    *entry = new_dir.clone();
                }
            }
        }
    }

    /// Rewrite a single tracked file path from `old_rel` to `new_rel` (drag-drop
    /// move). Preserves the list (category) it was in. Both stay relative
    /// (spec 033, R9, R21).
    pub fn move_entry(&mut self, old_rel: &str, new_rel: &str) {
        let old_rel = old_rel.replace('\\', "/");
        let new_rel = new_rel.replace('\\', "/");
        if old_rel == new_rel {
            return;
        }
        for list in self.all_lists_mut() {
            for entry in list.iter_mut() {
                if *entry == old_rel {
                    *entry = new_rel.clone();
                }
            }
        }
    }

    /// Remove and return every tracked file whose path sits under the folder
    /// `dir` (recursive folder delete). Returned paths are relative — the caller
    /// uses them to close editor tabs / views (spec 033, R6).
    pub fn drain_under(&mut self, dir: &str) -> Vec<String> {
        let dir = dir.replace('\\', "/");
        if dir.is_empty() {
            return Vec::new();
        }
        let prefix = format!("{dir}/");
        let mut removed = Vec::new();
        for list in self.all_lists_mut() {
            list.retain(|entry| {
                let under = entry.starts_with(&prefix) || *entry == dir;
                if under {
                    removed.push(entry.clone());
                }
                !under
            });
        }
        removed
    }

    /// True if `rel` is tracked by the project.
    pub fn contains(&self, rel: &str) -> bool {
        let rel = rel.replace('\\', "/");
        self.all_files().any(|f| f == rel)
    }

    /// True if `rel` is RAD-generated (read-only). Robust against legacy projects
    /// that tracked generated `.cbl` in `sources`: a `.cbl` whose stem matches a
    /// tracked `.cfrm` form is treated as generated.
    pub fn is_generated(&self, rel: &str) -> bool {
        let rel = rel.replace('\\', "/");
        if self.files.generated.iter().any(|f| f == &rel) {
            return true;
        }
        let stem = Path::new(&rel).file_stem().and_then(|s| s.to_str());
        let is_cobol = FileKind::from_path(&rel) == FileKind::Source;
        if is_cobol && stem.is_some() {
            let stem = stem.unwrap();
            if self
                .files
                .forms
                .iter()
                .any(|form| Path::new(form).file_stem().and_then(|s| s.to_str()) == Some(stem))
            {
                return true;
            }
            // `generated/<stem>-indexed.cbl` from a `.cidx` definition.
            if stem.ends_with("-indexed") {
                let base = stem.strip_suffix("-indexed").unwrap_or(stem);
                if self
                    .files
                    .indexed
                    .iter()
                    .any(|cidx| Path::new(cidx).file_stem().and_then(|s| s.to_str()) == Some(base))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Files in a given UI category (Generated is overlaid on Common Code in the
    /// tree, so callers usually iterate CommonCode + Generated separately).
    pub fn files_in(&self, category: Category) -> &[String] {
        match category {
            Category::Forms => &self.files.forms,
            Category::CommonCode => &self.files.sources,
            Category::Assets => &self.files.assets,
            Category::Documentation => &self.files.documentation,
            Category::Generated => &self.files.generated,
            Category::IndexedFiles => &self.files.indexed,
        }
    }

    /// All tracked files as relative path strings.
    pub fn all_files(&self) -> impl Iterator<Item = &str> {
        self.files
            .sources
            .iter()
            .chain(self.files.forms.iter())
            .chain(self.files.indexed.iter())
            .chain(self.files.assets.iter())
            .chain(self.files.documentation.iter())
            .chain(self.files.generated.iter())
            .map(|s| s.as_str())
    }

    /// Whether the project is compilable: it must contain at least one pure
    /// COBOL-85 program (hand-written or generated) **or** at least one form.
    pub fn is_compilable(&self) -> bool {
        !self.files.sources.is_empty()
            || !self.files.generated.is_empty()
            || !self.files.forms.is_empty()
    }
}

// ── Element status (the tree "semaphore") ──────────────────────────────────────

/// A traffic-light status shown to the left of each tree element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementStatus {
    /// Green — tested/compiled successfully and unchanged since.
    Tested,
    /// Yellow — changed since the last successful test (or never tested).
    #[default]
    Changed,
    /// Red — an issue was found / compilation or check failed.
    Failed,
}

impl ElementStatus {
    /// `(r, g, b)` for the status dot.
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            ElementStatus::Tested => (40, 200, 70),   // green
            ElementStatus::Changed => (245, 200, 30), // yellow
            ElementStatus::Failed => (235, 55, 55),   // red
        }
    }
    /// Hover text key idea (tooltip).
    pub fn tooltip(self) -> &'static str {
        match self {
            ElementStatus::Tested => "Tested OK",
            ElementStatus::Changed => "Changed — not tested",
            ElementStatus::Failed => "Issue / failed",
        }
    }
}

// ── Category (the IDE's fixed top-level tree nodes) ─────────────────────────────

/// The fixed top-level categories shown in the project tree. The IDE owns these
/// nodes; developers only add sub-entries within a category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Forms,
    /// Indexed-file definitions (`.cidx`).
    IndexedFiles,
    CommonCode,
    /// RAD output — its own read-only top category (one file per form).
    Generated,
    Assets,
    Documentation,
}

impl Category {
    /// The fixed top categories, in display order. `Generated` is IDE-owned and
    /// read-only (developers cannot add to it — forms populate it).
    pub const TOP: [Category; 6] = [
        Category::Forms,
        Category::IndexedFiles,
        Category::CommonCode,
        Category::Generated,
        Category::Assets,
        Category::Documentation,
    ];

    /// True if developers may add files to this category (Generated is populated
    /// by the form designer only).
    pub fn is_addable(self) -> bool {
        !matches!(self, Category::Generated)
    }

    /// The category's root subdirectory (project-relative), where its files and
    /// folders live on disk. Assets and Documentation have their own resolvers in
    /// the panel (legacy-case fallbacks); this returns their canonical names.
    pub fn root_subdir(self) -> &'static str {
        match self {
            Category::Forms => "forms",
            Category::IndexedFiles => "indexed",
            Category::CommonCode => "src",
            Category::Generated => "generated",
            Category::Assets => "Assets",
            Category::Documentation => cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT,
        }
    }

    /// Route a path to a category by extension.
    pub fn of_path(path: &str) -> Category {
        Category::of_kind(FileKind::from_path(path))
    }

    /// The category a file kind belongs to (its "home" category).
    pub fn of_kind(kind: FileKind) -> Category {
        match kind {
            FileKind::Form => Category::Forms,
            FileKind::Indexed => Category::IndexedFiles,
            FileKind::Source => Category::CommonCode,
            FileKind::Documentation => Category::Documentation,
            FileKind::Asset => Category::Assets,
        }
    }

    /// The category whose root subdir is the top component of `dir_rel`, if any.
    pub fn from_root_component(dir_rel: &str) -> Option<Category> {
        let top = dir_rel.replace('\\', "/");
        let top = top.split('/').next().unwrap_or("");
        Category::TOP
            .into_iter()
            .find(|c| c.root_subdir().eq_ignore_ascii_case(top))
    }

    // NOTE: Visual icons for the project tree are hand-written vector shapes
    // (see panels/project.rs: tree_icon + draw_*_icon). These emoji strings
    // are intentionally removed for cross-OS / font-independent rendering.
}

// ── FileKind ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Source,
    Form,
    Indexed,
    Asset,
    Documentation,
}

impl FileKind {
    pub fn from_path(path: &str) -> Self {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "cbl" | "cob" | "cpy" => FileKind::Source,
            "cfrm" => FileKind::Form,
            "cidx" => FileKind::Indexed,
            "md" | "markdown" | "txt" | "rst" | "adoc" | "pdf" | "html" | "htm" => {
                FileKind::Documentation
            }
            _ => FileKind::Asset,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Source => "Common Code",
            FileKind::Form => "Forms",
            FileKind::Indexed => "Indexed Files",
            FileKind::Asset => "Assets",
            FileKind::Documentation => "Knowledge Base",
        }
    }

    // Visual icons are drawn with vectors in the project tree (see panels/project.rs).
    // Emoji versions removed for portability across OSes and font configurations.
}

// ── Recursive resource discovery ────────────────────────────────────────────
//
// Every project resource type has exactly one canonical top-level folder
// (`Category::root_subdir`), and the developer is free to nest subfolders
// under it (`forms/Common/textboxes-form.cfrm` is exactly as valid as
// `forms/textboxes-form.cfrm`). A lookup that only checks the top level of
// that folder — or that trusts a possibly-stale in-memory file list instead
// of the real folder on disk — silently fails to find a resource sitting
// right there in the project tree. These helpers always re-walk the actual
// folder, at any depth, and match leniently enough to survive a small typo
// in the name being searched for (operator, 2026-08-01).

/// Recursively collect every file under `project_dir/category.root_subdir()`,
/// as project-relative paths using `/` separators, at any nesting depth.
/// Returns an empty list if the folder does not exist.
pub fn recursive_category_files(project_dir: &Path, category: Category) -> Vec<String> {
    let root = project_dir.join(category.root_subdir());
    let mut out = Vec::new();
    collect_files_recursive(project_dir, &root, &mut out);
    out.sort();
    out
}

fn collect_files_recursive(project_dir: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(project_dir, &path, out);
        } else if let Ok(rel) = path.strip_prefix(project_dir) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Best-effort match of `needle` (free text — a developer request or a
/// delegated task's objective — that names a resource somewhere inside it)
/// against `candidates` (project-relative paths, typically from
/// [`recursive_category_files`]). Never assumes the resource sits directly
/// under the top-level folder: every candidate, at any depth, is considered
/// equally. Tried in order, cheapest and most exact first:
///
/// 1. the candidate's full relative path appears verbatim in `needle` (case-insensitive);
/// 2. the candidate's bare file name appears verbatim in `needle` (case-insensitive);
/// 3. a whitespace-delimited token of `needle`, normalized (lowercased, with
///    `-`/`_`/`.`/`/` and other punctuation stripped), is within a small edit
///    distance of the candidate's own normalized path or file name — enough
///    to survive one missing/extra/substituted character (e.g. a developer
///    typing "texboxes-form.cfrm" for "textboxes-form.cfrm").
///
/// Returns the single closest candidate, or `None` if nothing clears the bar.
pub fn best_resource_match<'a>(needle: &str, candidates: &'a [String]) -> Option<&'a str> {
    let needle_lower = needle.to_ascii_lowercase();

    if let Some(hit) = candidates
        .iter()
        .find(|c| needle_lower.contains(&c.to_ascii_lowercase()))
    {
        return Some(hit.as_str());
    }

    if let Some(hit) = candidates.iter().find(|c| {
        Path::new(c.as_str())
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| needle_lower.contains(&name.to_ascii_lowercase()))
    }) {
        return Some(hit.as_str());
    }

    let tokens: Vec<String> = needle_lower
        .split_whitespace()
        .map(normalize_resource_name)
        .filter(|t| t.len() >= 5)
        .collect();
    let mut best: Option<(&str, usize)> = None;
    for candidate in candidates {
        let path_norm = normalize_resource_name(candidate);
        let name_norm = Path::new(candidate.as_str())
            .file_name()
            .and_then(|n| n.to_str())
            .map(normalize_resource_name)
            .unwrap_or_default();
        for token in &tokens {
            for norm in [path_norm.as_str(), name_norm.as_str()] {
                if norm.is_empty() {
                    continue;
                }
                let distance = levenshtein_distance(token, norm);
                let threshold = (norm.len().max(token.len()) / 6).max(1).min(2);
                if distance <= threshold
                    && best.as_ref().is_none_or(|(_, best_distance)| distance < *best_distance)
                {
                    best = Some((candidate.as_str(), distance));
                }
            }
        }
    }
    best.map(|(path, _)| path)
}

/// Lowercase, alphanumeric-only rendering of `s` (drops `-`, `_`, `.`, `/`,
/// spaces, …), so `"texboxes-form.cfrm"` and `"forms/TextBoxes Form.cfrm"`
/// compare on the same footing.
fn normalize_resource_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Classic iterative Levenshtein edit distance (insertions, deletions,
/// substitutions, each one step), computed in O(a·b) time and O(b) space.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod resource_discovery_tests {
    use super::*;

    #[test]
    fn recursive_walk_finds_forms_nested_under_a_subfolder() {
        let dir = std::env::temp_dir().join(format!(
            "cobolt-resource-discovery-test-{}",
            std::process::id()
        ));
        let forms_common = dir.join("forms").join("Common");
        std::fs::create_dir_all(&forms_common).unwrap();
        std::fs::write(forms_common.join("textboxes-form.cfrm"), "<Form/>").unwrap();
        std::fs::write(forms_common.join("buttons-form.cfrm"), "<Form/>").unwrap();

        let found = recursive_category_files(&dir, Category::Forms);
        assert_eq!(
            found,
            vec![
                "forms/Common/buttons-form.cfrm".to_string(),
                "forms/Common/textboxes-form.cfrm".to_string(),
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn best_match_finds_exact_nested_path() {
        let candidates = vec![
            "forms/Common/buttons-form.cfrm".to_string(),
            "forms/Common/textboxes-form.cfrm".to_string(),
        ];
        let objective =
            "Inspect the form at forms/Common/textboxes-form.cfrm and identify all controls.";
        assert_eq!(
            best_resource_match(objective, &candidates),
            Some("forms/Common/textboxes-form.cfrm")
        );
    }

    #[test]
    fn best_match_survives_a_missing_letter_typo() {
        let candidates = vec![
            "forms/Common/buttons-form.cfrm".to_string(),
            "forms/Common/textboxes-form.cfrm".to_string(),
        ];
        // Developer typed "texboxes-form.cfrm" — missing the second "t".
        let objective =
            "Inspect the form at forms/Common/texboxes-form.cfrm and identify all controls.";
        assert_eq!(
            best_resource_match(objective, &candidates),
            Some("forms/Common/textboxes-form.cfrm")
        );
    }

    #[test]
    fn best_match_returns_none_when_nothing_is_close() {
        let candidates = vec!["forms/Common/buttons-form.cfrm".to_string()];
        let objective = "Inspect the checkboxes form and identify all controls.";
        assert_eq!(best_resource_match(objective, &candidates), None);
    }
}

// ── Load / Save ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    Toml(String),
    Zip(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::Io(e) => write!(f, "I/O error: {e}"),
            ProjectError::Toml(s) => write!(f, "TOML error: {s}"),
            ProjectError::Zip(s) => write!(f, "Zip error: {s}"),
        }
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self {
        ProjectError::Io(e)
    }
}

pub fn load_project(path: &Path) -> Result<CoboltProject, ProjectError> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text).map_err(|e| ProjectError::Toml(e.to_string()))
}

pub fn save_project(project: &CoboltProject, path: &Path) -> Result<(), ProjectError> {
    let text = toml::to_string_pretty(project).map_err(|e| ProjectError::Toml(e.to_string()))?;
    std::fs::write(path, text)?;
    Ok(())
}

// ── Package (zip) ─────────────────────────────────────────────────────────────

/// Package the project into a zip file at `output_zip`.
///
/// `project_dir` is the directory containing `cobolt.toml`.
/// All tracked files are copied with their relative paths preserved.
/// A `run.sh` / `run.bat` launcher and a `README.txt` are generated.
/// If a `cobolt` / `cobolt.exe` binary is found next to the running IDE,
/// it is included automatically.
pub fn package_project(
    project: &CoboltProject,
    project_dir: &Path,
    output_zip: &Path,
) -> Result<usize, ProjectError> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let file = std::fs::File::create(output_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let exec_opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let mut count = 0usize;

    // ── cobolt.toml ───────────────────────────────────────────────────────────
    let manifest =
        toml::to_string_pretty(project).map_err(|e| ProjectError::Toml(e.to_string()))?;
    zip.start_file("cobolt.toml", opts)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(manifest.as_bytes())?;
    count += 1;

    // ── Required Apache-2.0 notices ───────────────────────────────────────────
    for (name, text) in [
        ("LICENSE", cobolt_compiler::LICENSE_TEXT),
        ("NOTICE", cobolt_compiler::NOTICE_TEXT),
        (
            "POWERRUSTCOBOL-NOTICE.txt",
            cobolt_compiler::RUNTIME_NOTICE_TEXT,
        ),
    ] {
        zip.start_file(name, opts)
            .map_err(|e| ProjectError::Zip(e.to_string()))?;
        zip.write_all(text.as_bytes())?;
        count += 1;
    }

    // ── Project files ─────────────────────────────────────────────────────────
    for rel in project.all_files() {
        let abs = project_dir.join(rel);
        if !abs.exists() {
            continue; // skip missing files, warn in output
        }
        let mut f = std::fs::File::open(&abs)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        zip.start_file(rel, opts)
            .map_err(|e| ProjectError::Zip(e.to_string()))?;
        zip.write_all(&buf)?;
        count += 1;
    }

    // ── cobolt binary (optional) ──────────────────────────────────────────────
    if let Some(cobolt_bin) = find_cobolt_binary() {
        let mut f = std::fs::File::open(&cobolt_bin)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let name = cobolt_bin
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("cobolt");
        zip.start_file(name, exec_opts)
            .map_err(|e| ProjectError::Zip(e.to_string()))?;
        zip.write_all(&buf)?;
        count += 1;
    }

    // ── run.sh ────────────────────────────────────────────────────────────────
    let main = &project.project.main;
    let sh = format!(
        "#!/bin/sh\n\
         # Run the RustCOBOL project (PowerRustCOBOL)\n\
         DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
         RCRUN=\"$DIR/rcrun\"\n\
         if [ ! -x \"$RCRUN\" ]; then RCRUN=rcrun; fi\n\
         exec \"$RCRUN\" run \"$DIR/{main}\" \"$@\"\n"
    );
    zip.start_file("run.sh", exec_opts)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(sh.as_bytes())?;

    // ── run.bat ───────────────────────────────────────────────────────────────
    let bat = format!(
        "@echo off\r\n\
         rem Run the RustCOBOL project (PowerRustCOBOL)\r\n\
         set DIR=%~dp0\r\n\
         if exist \"%DIR%rcrun.exe\" (\r\n\
             \"%DIR%rcrun.exe\" run \"%DIR%{main}\" %*\r\n\
         ) else (\r\n\
             rcrun run \"%DIR%{main}\" %*\r\n\
         )\r\n"
    );
    zip.start_file("run.bat", opts)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(bat.as_bytes())?;

    // ── README.txt ────────────────────────────────────────────────────────────
    let readme = format!(
        "# {} {}\n\
         \n\
         To run this project:\n\
         \n\
         On Linux / macOS:\n\
           chmod +x run.sh rcrun 2>/dev/null; ./run.sh\n\
         \n\
         On Windows:\n\
           run.bat\n\
         \n\
         If the 'rcrun' binary is not included, install it first:\n\
           cargo install cobolt-cli  # installs as 'rcrun'\n\
         \n\
         Main entry point: {main}\n",
        project.project.name, project.project.version,
    );
    zip.start_file("README.txt", opts)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(readme.as_bytes())?;

    zip.finish().map_err(|e| ProjectError::Zip(e.to_string()))?;
    Ok(count)
}

/// Try to locate the `cobolt` CLI binary.
/// First looks in the same directory as the running IDE executable;
/// falls back to PATH.
fn find_cobolt_binary() -> Option<PathBuf> {
    // Look next to this executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in &["rcrun", "rcrun.exe"] {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

// ── Relative path helper ──────────────────────────────────────────────────────

/// Convert an absolute path to a relative path string from `base`.
/// Returns `None` if the path is not under `base`.
pub fn relative_to(path: &Path, base: &Path) -> Option<String> {
    path.strip_prefix(base)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// True when an indexed file's `assign_path` lies under `project_dir` and can be
/// bundled into a project zip (R24).
pub fn assign_path_is_packaged(assign: &str, project_dir: &Path) -> bool {
    let p = Path::new(assign);
    if p.is_absolute() {
        return false;
    }
    let abs = project_dir.join(p);
    abs.exists() && relative_to(&abs, project_dir).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> CoboltProject {
        CoboltProject::new("T", "src/main.cbl")
    }

    /// 038 — window-effect settings: pre-038 toml ⇒ no effects; a NEW project
    /// starts with the Matrix entrance; explicit values round-trip.
    #[test]
    fn forms_config_effects_defaults_and_round_trip() {
        use cobolt_forms::window_fx::{Easing, WindowEffect};

        // Pre-038 file: [forms] with only a theme (or absent entirely).
        let old: CoboltProject = toml::from_str(
            r#"
[project]
name = "Legacy"
version = "1.0.0"
main = "src/main.cbl"
"#,
        )
        .unwrap();
        assert_eq!(old.entrance_fx().effect, WindowEffect::None);
        assert_eq!(old.exit_fx().effect, WindowEffect::None);
        assert!(!old.forms.entrance_on_restore);
        println!(
            "pre-038 toml: entrance={} exit={} restore={}",
            old.entrance_fx().effect.as_str(),
            old.exit_fx().effect.as_str(),
            old.forms.entrance_on_restore
        );

        // New project: Matrix entrance by default (038 R5), inside the
        // matrix-rain 1500–4000 ms band.
        let fresh = proj();
        assert_eq!(fresh.entrance_fx().effect, WindowEffect::MatrixRain);
        assert_eq!(fresh.entrance_fx().duration_ms, 2000);
        assert_eq!(fresh.exit_fx().effect, WindowEffect::None);
        // A stored out-of-band duration clamps into the EFFECT's own bounds.
        let mut banded = proj();
        banded.forms.entrance_ms = 600;
        assert_eq!(banded.entrance_fx().duration_ms, 1500, "matrix min");
        println!(
            "new project: entrance={} ({}ms), exit={}",
            fresh.entrance_fx().effect.as_str(),
            fresh.entrance_fx().duration_ms,
            fresh.exit_fx().effect.as_str()
        );

        // Explicit values parse; exit easing falls back to ease-in when unset.
        let cfg: CoboltProject = toml::from_str(
            r#"
[project]
name = "Fx"
version = "1.0.0"
main = "src/main.cbl"

[forms]
entrance-effect = "radar-wipe"
entrance-ms = 900
entrance-easing = "ease-in-out"
exit-effect = "fade"
entrance-on-restore = true
"#,
        )
        .unwrap();
        assert_eq!(cfg.entrance_fx().effect, WindowEffect::RadarWipe);
        assert_eq!(cfg.entrance_fx().duration_ms, 900);
        assert_eq!(cfg.entrance_fx().easing, Easing::EaseInOut);
        assert_eq!(cfg.exit_fx().effect, WindowEffect::Fade);
        assert_eq!(cfg.exit_fx().easing, Easing::EaseIn, "exit default easing");
        assert!(cfg.forms.entrance_on_restore);
        println!(
            "explicit: entrance={}:{}:{} exit={}:{}:{} restore={}",
            cfg.entrance_fx().effect.as_str(),
            cfg.entrance_fx().duration_ms,
            cfg.entrance_fx().easing.as_str(),
            cfg.exit_fx().effect.as_str(),
            cfg.exit_fx().duration_ms,
            cfg.exit_fx().easing.as_str(),
            cfg.forms.entrance_on_restore
        );
    }

    #[test]
    fn rename_prefix_rewrites_only_entries_under_dir() {
        let mut p = proj();
        p.add_file_to("forms/customers/order.cfrm", Category::Forms);
        p.add_file_to("forms/customers/invoice.cfrm", Category::Forms);
        p.add_file_to("forms/login.cfrm", Category::Forms);
        p.add_file_to("src/customers/util.cbl", Category::CommonCode);

        p.rename_prefix("forms/customers", "forms/clients");

        assert!(p.files.forms.contains(&"forms/clients/order.cfrm".to_string()));
        assert!(p.files.forms.contains(&"forms/clients/invoice.cfrm".to_string()));
        // Untouched: a sibling not under the renamed dir, and a same-named dir in
        // another category root.
        assert!(p.files.forms.contains(&"forms/login.cfrm".to_string()));
        assert!(p.files.sources.contains(&"src/customers/util.cbl".to_string()));
        // No absolute path leaked (R21).
        assert!(p.all_files().all(|f| !Path::new(f).is_absolute()));
    }

    #[test]
    fn move_entry_rewrites_single_file_keeping_category() {
        let mut p = proj();
        p.add_file_to("forms/a/order.cfrm", Category::Forms);
        p.move_entry("forms/a/order.cfrm", "forms/b/order.cfrm");
        assert_eq!(p.files.forms, vec!["forms/b/order.cfrm".to_string()]);
    }

    #[test]
    fn drain_under_removes_and_returns_subtree() {
        let mut p = proj();
        p.add_file_to("forms/customers/order.cfrm", Category::Forms);
        p.add_file_to("generated/customers/order.cbl", Category::Generated);
        p.add_file_to("forms/login.cfrm", Category::Forms);

        let removed = p.drain_under("forms/customers");
        assert_eq!(removed, vec!["forms/customers/order.cfrm".to_string()]);
        assert!(p.files.forms.contains(&"forms/login.cfrm".to_string()));
        // A same-named folder under a different category root is not touched.
        assert!(p
            .files
            .generated
            .contains(&"generated/customers/order.cbl".to_string()));
    }

    #[test]
    fn project_ai_round_trip_contains_models_but_never_credentials() {
        let mut p = proj();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.model_profiles.push(crate::llm::ModelProfile {
            id: "project-profile".into(),
            name: "Project model".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            endpoint_user_edited: false,
            model: "gpt-5".into(),
            temperature: 0.4,
            max_tokens: 8192,
            timeout_secs: 120,
        });
        llm.store_api_key(
            crate::llm::profile_api_key_slot("project-profile"),
            "never-write-this-secret",
        );
        p.ai = ProjectAiSettings::from_llm(&llm);

        let text = toml::to_string_pretty(&p).unwrap();
        assert!(text.contains("project-profile"));
        assert!(!text.contains("never-write-this-secret"));
        assert!(!text.contains("api_key"));

        let loaded: CoboltProject = toml::from_str(&text).unwrap();
        assert_eq!(loaded.ai.schema_version, 1);
        assert_eq!(loaded.ai.model_profiles[0].model, "gpt-5");
    }

    #[test]
    fn project_integrations_round_trip_the_search_engine_id_never_the_keys() {
        // Spec 039 R31/AC10: the google_maps and Custom Search API keys live
        // only in the machine-local secret store (never in `cobolt.toml`);
        // `google_search_engine_id` is NOT a secret and round-trips like any
        // other project setting.
        let mut p = proj();
        p.integrations.google_search_engine_id = "017576662512468239146:omuauf_lfve".into();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.store_api_key(
            crate::llm::GOOGLE_MAPS_API_KEY_SLOT.to_owned(),
            "never-write-this-maps-secret",
        );
        llm.store_api_key(
            crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT.to_owned(),
            "never-write-this-search-secret",
        );

        let text = toml::to_string_pretty(&p).unwrap();
        assert!(text.contains("017576662512468239146:omuauf_lfve"));
        assert!(!text.contains("never-write-this-maps-secret"));
        assert!(!text.contains("never-write-this-search-secret"));
        assert!(!text.contains("api_key"));

        let loaded: CoboltProject = toml::from_str(&text).unwrap();
        assert_eq!(
            loaded.integrations.google_search_engine_id,
            "017576662512468239146:omuauf_lfve"
        );

        // The keys are exactly where they should be: resolvable from the
        // secret store by the well-known slot, nowhere near the project file.
        assert_eq!(
            llm.api_keys.get(crate::llm::GOOGLE_MAPS_API_KEY_SLOT),
            Some(&"never-write-this-maps-secret".to_owned())
        );
        assert_eq!(
            llm.api_keys
                .get(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT),
            Some(&"never-write-this-search-secret".to_owned())
        );
    }

    #[test]
    fn applying_project_ai_switches_profiles_without_touching_machine_keys() {
        let mut runtime = crate::llm::LlmConfig::load_defaults_for_test();
        runtime.store_api_key(crate::llm::profile_api_key_slot("a"), "key-a");
        runtime.store_api_key(crate::llm::profile_api_key_slot("b"), "key-b");

        let mut a = ProjectAiSettings::new();
        a.model_profiles.push(crate::llm::ModelProfile {
            id: "a".into(),
            name: "A".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            endpoint_user_edited: false,
            model: "model-a".into(),
            temperature: 0.4,
            max_tokens: 1000,
            timeout_secs: 30,
        });
        let mut b = ProjectAiSettings::new();
        b.model_profiles.push(crate::llm::ModelProfile {
            id: "b".into(),
            name: "B".into(),
            provider: "ollama_cloud".into(),
            endpoint: "https://ollama.com/api/chat".into(),
            endpoint_user_edited: false,
            model: "model-b".into(),
            temperature: 0.7,
            max_tokens: 2000,
            timeout_secs: 60,
        });

        a.apply_to_llm(&mut runtime);
        assert_eq!(runtime.model_profiles[0].id, "a");
        assert_eq!(
            runtime.profile("a").unwrap().resolve(&runtime).api_key,
            "key-a"
        );
        b.apply_to_llm(&mut runtime);
        assert_eq!(runtime.model_profiles[0].id, "b");
        assert_eq!(
            runtime.profile("b").unwrap().resolve(&runtime).api_key,
            "key-b"
        );
        assert_eq!(runtime.api_keys.len(), 2);
    }

    #[test]
    fn legacy_project_without_ai_table_is_marked_for_migration() {
        let text = r#"
[project]
name = "Legacy"
version = "1.0.0"
main = "src/main.cbl"
"#;
        let loaded: CoboltProject = toml::from_str(text).unwrap();
        assert_eq!(loaded.ai.schema_version, 0);
    }

    #[test]
    fn add_file_routes_by_category() {
        let mut p = proj();
        p.add_file("src/calc.cbl");
        p.add_file("forms/login.cfrm");
        p.add_file("img/logo.png");
        p.add_file("docs/manual.md");
        assert_eq!(p.files.sources, vec!["src/calc.cbl"]);
        assert_eq!(p.files.forms, vec!["forms/login.cfrm"]);
        assert_eq!(p.files.assets, vec!["img/logo.png"]);
        assert_eq!(p.files.documentation, vec!["docs/manual.md"]);
        assert_eq!(Category::of_path("a.cbl"), Category::CommonCode);
        assert_eq!(Category::of_path("a.cfrm"), Category::Forms);
        assert_eq!(Category::of_path("a.cidx"), Category::IndexedFiles);
        assert_eq!(Category::of_path("a.md"), Category::Documentation);
        assert_eq!(Category::of_path("a.png"), Category::Assets);
    }

    #[test]
    fn indexed_category_tree_order() {
        let top: Vec<_> = Category::TOP.iter().copied().collect();
        assert_eq!(
            top,
            vec![
                Category::Forms,
                Category::IndexedFiles,
                Category::CommonCode,
                Category::Generated,
                Category::Assets,
                Category::Documentation,
            ]
        );
    }

    #[test]
    fn indexed_generated_cbl_detected() {
        let mut p = proj();
        p.add_file("indexed/customers.cidx");
        p.add_generated("generated/customers-indexed.cbl");
        assert!(p.is_generated("generated/customers-indexed.cbl"));
    }

    #[test]
    fn external_assign_path_not_packaged() {
        let dir = std::env::temp_dir().join("prcidx_pkg_test");
        let _ = std::fs::create_dir_all(&dir);
        assert!(!assign_path_is_packaged("/tmp/outside.idx", &dir));
        let rel = dir.join("data").join("in.idx");
        let _ = std::fs::create_dir_all(rel.parent().unwrap());
        let _ = std::fs::write(&rel, b"");
        assert!(assign_path_is_packaged("data/in.idx", &dir));
    }

    #[test]
    fn generated_is_flagged_and_removed_from_common_code() {
        let mut p = proj();
        p.add_file("forms/login.cbl"); // landed in sources first
        p.add_generated("forms/login.cbl");
        assert!(p.files.sources.is_empty(), "moved out of common code");
        assert_eq!(p.files.generated, vec!["forms/login.cbl"]);
        assert!(p.is_generated("forms/login.cbl"));
    }

    #[test]
    fn legacy_generated_detected_by_stem_match_with_form() {
        // A legacy project that tracked the generated .cbl in `sources`.
        let mut p = proj();
        p.add_file("forms/login.cfrm");
        p.add_file("forms/login.cbl"); // same stem as the form → generated
        p.add_file("src/calc.cbl"); // hand-written, no matching form
        assert!(p.is_generated("forms/login.cbl"));
        assert!(!p.is_generated("src/calc.cbl"));
    }

    #[test]
    fn is_compilable_requires_program_or_form() {
        let mut p = proj();
        assert!(!p.is_compilable(), "empty project is not compilable");
        p.add_file("forms/a.cfrm");
        assert!(p.is_compilable(), "a form alone is enough");

        let mut p2 = proj();
        p2.add_file("src/a.cbl");
        assert!(p2.is_compilable(), "a COBOL program alone is enough");

        let mut p3 = proj();
        p3.add_generated("gen/a.cbl");
        assert!(p3.is_compilable(), "generated COBOL alone is enough");
    }

    #[test]
    fn ide_settings_default_when_missing_from_toml() {
        // A project file written before 1.15.0 has no [ide] section.
        let toml = r#"
[project]
name = "Legacy"
version = "1.0.0"
main = "src/main.cbl"
"#;
        let p: CoboltProject = toml::from_str(toml).expect("parse legacy toml");
        assert_eq!(
            p.ide.theme, "",
            "missing theme → empty (resolves to default)"
        );
        assert_eq!(p.ide.background_image, "");
        assert_eq!(p.ide.background_opacity, 70, "serde default opacity");
    }

    #[test]
    fn ide_settings_round_trip() {
        let mut p = proj();
        p.ide.theme = "monokai".into();
        p.ide.background_image = "assets/bg.png".into();
        p.ide.background_opacity = 35;
        let s = toml::to_string(&p).expect("serialize");
        let back: CoboltProject = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.ide.theme, "monokai");
        assert_eq!(back.ide.background_image, "assets/bg.png");
        assert_eq!(back.ide.background_opacity, 35);
    }

    #[test]
    fn forms_theme_default_when_missing_007() {
        // A project file with no [forms] section → empty → Liquid Glass.
        let toml = r#"
[project]
name = "Legacy"
version = "1.0.0"
main = "src/main.cbl"
"#;
        let p: CoboltProject = toml::from_str(toml).expect("parse");
        assert_eq!(p.forms.theme, "");
        assert_eq!(p.form_theme_default(), None, "empty → unset → Liquid Glass");
    }

    #[test]
    fn forms_theme_round_trip_007() {
        let mut p = proj();
        p.forms.theme = "stainless-steel".into();
        let s = toml::to_string(&p).expect("serialize");
        let back: CoboltProject = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.forms.theme, "stainless-steel");
        assert_eq!(back.form_theme_default(), Some("stainless-steel"));
    }

    #[test]
    fn user_control_toml_roundtrip() {
        let mut p = proj();
        let mut properties = HashMap::new();
        properties.insert("Caption".to_string(), "Name:".to_string());
        properties.insert("ForegroundColor".to_string(), "#FFFFFF".to_string());
        let def = UserControlDef {
            name: "CustomerCard".to_string(),
            width: 300,
            height: 200,
            controls: vec![UserControlEntry {
                id: "Label1".to_string(),
                control_type: "Label".to_string(),
                parent: None,
                x: 10,
                y: 12,
                w: 80,
                h: 20,
                z_order: 1,
                properties,
            }],
        };
        p.user_controls.push(def.clone());

        let s = toml::to_string(&p).expect("serialize");
        assert!(
            s.contains("[[user-controls]]"),
            "serialized project should use the public user-controls TOML key"
        );
        let back: CoboltProject = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.user_controls, vec![def]);
    }
}
