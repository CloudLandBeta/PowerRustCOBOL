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

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoboltProject {
    pub project: ProjectMeta,
    #[serde(default)]
    pub files:   ProjectFiles,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// Per-project IDE appearance (colour theme + background image).
    #[serde(default)]
    pub ide:     IdeSettings,
}

/// Per-project IDE appearance settings (colour theme + background image),
/// persisted in `cobolt.toml` so the look travels with the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeSettings {
    /// Colour-theme id (see `crate::theme`). Empty / unknown → default theme.
    #[serde(default)]
    pub theme: String,
    /// Optional background image (relative to the project root, or absolute).
    #[serde(default)]
    pub background_image: String,
    /// Background-image opacity, 0 (invisible) … 100 (fully opaque).
    #[serde(default = "default_bg_opacity")]
    pub background_opacity: u8,
}

fn default_bg_opacity() -> u8 { 70 }

impl Default for IdeSettings {
    fn default() -> Self {
        Self {
            theme: String::new(),
            background_image: String::new(),
            background_opacity: default_bg_opacity(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name:    String,
    /// Semantic version `major.minor.fix` (the three parts are edited
    /// separately in the Settings form and recomposed here).
    pub version: String,
    /// Relative path (from project root) to the main COBOL source file.
    pub main:    String,
    /// Custom copyright line embedded in generated headers / distributions.
    #[serde(default)]
    pub copyright: String,
    /// Short license identifier (e.g. "MIT", "Apache-2.0", "Proprietary").
    #[serde(default)]
    pub license_model: String,
    /// Full license text (editable).
    #[serde(default)]
    pub license_text: String,
}

impl ProjectMeta {
    /// Parse `version` into `(major, minor, fix)`, tolerating missing parts.
    pub fn version_parts(&self) -> (u32, u32, u32) {
        let mut it = self.version.split('.').map(|s| s.trim().parse::<u32>().unwrap_or(0));
        (it.next().unwrap_or(1), it.next().unwrap_or(0), it.next().unwrap_or(0))
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
    pub forms:   Vec<String>,
    #[serde(default)]
    pub assets:  Vec<String>,
    /// Documentation files (Markdown, text, PDF, …).
    #[serde(default)]
    pub documentation: Vec<String>,
    /// RAD-generated COBOL (output of the form designer) — **read-only**.
    #[serde(default)]
    pub generated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Force fixed-format COBOL parsing (auto-detected when false).
    pub fixed_format: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self { Self { fixed_format: false } }
}

impl CoboltProject {
    /// Create a blank project with sensible defaults.
    pub fn new(name: impl Into<String>, main: impl Into<String>) -> Self {
        Self {
            project: ProjectMeta {
                name:    name.into(),
                version: "1.0.0".into(),
                main:    main.into(),
                copyright:     String::new(),
                license_model: String::new(),
                license_text:  String::new(),
            },
            files:   ProjectFiles::default(),
            runtime: RuntimeConfig::default(),
            ide:     IdeSettings::default(),
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
            Category::Forms         => &mut self.files.forms,
            Category::CommonCode    => &mut self.files.sources,
            Category::Assets        => &mut self.files.assets,
            Category::Documentation => &mut self.files.documentation,
            Category::Generated     => &mut self.files.generated,
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
        is_cobol
            && stem.is_some()
            && self.files.forms.iter().any(|form| {
                Path::new(form).file_stem().and_then(|s| s.to_str()) == stem
            })
    }

    /// Files in a given UI category (Generated is overlaid on Common Code in the
    /// tree, so callers usually iterate CommonCode + Generated separately).
    pub fn files_in(&self, category: Category) -> &[String] {
        match category {
            Category::Forms         => &self.files.forms,
            Category::CommonCode    => &self.files.sources,
            Category::Assets        => &self.files.assets,
            Category::Documentation => &self.files.documentation,
            Category::Generated     => &self.files.generated,
        }
    }

    /// All tracked files as relative path strings.
    pub fn all_files(&self) -> impl Iterator<Item = &str> {
        self.files.sources.iter()
            .chain(self.files.forms.iter())
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
            ElementStatus::Tested  => (40, 200, 70),    // green
            ElementStatus::Changed => (245, 200, 30),   // yellow
            ElementStatus::Failed  => (235, 55, 55),    // red
        }
    }
    /// Hover text key idea (tooltip).
    pub fn tooltip(self) -> &'static str {
        match self {
            ElementStatus::Tested  => "Tested OK",
            ElementStatus::Changed => "Changed — not tested",
            ElementStatus::Failed  => "Issue / failed",
        }
    }
}

// ── Category (the IDE's fixed top-level tree nodes) ─────────────────────────────

/// The fixed top-level categories shown in the project tree. The IDE owns these
/// nodes; developers only add sub-entries within a category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Forms,
    CommonCode,
    /// RAD output — its own read-only top category (one file per form).
    Generated,
    Assets,
    Documentation,
}

impl Category {
    /// The fixed top categories, in display order. `Generated` is IDE-owned and
    /// read-only (developers cannot add to it — forms populate it).
    pub const TOP: [Category; 5] = [
        Category::Forms,
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

    /// Route a path to a category by extension.
    pub fn of_path(path: &str) -> Category {
        match FileKind::from_path(path) {
            FileKind::Form          => Category::Forms,
            FileKind::Source        => Category::CommonCode,
            FileKind::Documentation => Category::Documentation,
            FileKind::Asset         => Category::Assets,
        }
    }

    /// A professional Unicode icon for the category header.
    pub fn icon(self) -> &'static str {
        match self {
            Category::Forms         => "🖼",
            Category::CommonCode    => "🧩",
            Category::Generated     => "⚙",
            Category::Assets        => "🎴",
            Category::Documentation => "📚",
        }
    }
}

// ── FileKind ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Source,
    Form,
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
            "cbl" | "cob" | "cpy"                          => FileKind::Source,
            "cfrm"                                          => FileKind::Form,
            "md" | "markdown" | "txt" | "rst" | "adoc"
            | "pdf" | "html" | "htm"                       => FileKind::Documentation,
            _                                              => FileKind::Asset,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Source        => "Common Code",
            FileKind::Form          => "Forms",
            FileKind::Asset         => "Assets",
            FileKind::Documentation => "Documentation",
        }
    }

    /// A professional Unicode icon for a file of this kind.
    pub fn icon(self) -> &'static str {
        match self {
            FileKind::Source        => "🧾",
            FileKind::Form          => "🖼",
            FileKind::Asset         => "🎴",
            FileKind::Documentation => "📄",
        }
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
            ProjectError::Io(e)   => write!(f, "I/O error: {e}"),
            ProjectError::Toml(s) => write!(f, "TOML error: {s}"),
            ProjectError::Zip(s)  => write!(f, "Zip error: {s}"),
        }
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self { ProjectError::Io(e) }
}

pub fn load_project(path: &Path) -> Result<CoboltProject, ProjectError> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text).map_err(|e| ProjectError::Toml(e.to_string()))
}

pub fn save_project(project: &CoboltProject, path: &Path) -> Result<(), ProjectError> {
    let text = toml::to_string_pretty(project)
        .map_err(|e| ProjectError::Toml(e.to_string()))?;
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
    project:     &CoboltProject,
    project_dir: &Path,
    output_zip:  &Path,
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
    let manifest = toml::to_string_pretty(project)
        .map_err(|e| ProjectError::Toml(e.to_string()))?;
    zip.start_file("cobolt.toml", opts)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(manifest.as_bytes())?;
    count += 1;

    // ── Required Apache-2.0 notices ───────────────────────────────────────────
    for (name, text) in [
        ("LICENSE", cobolt_compiler::LICENSE_TEXT),
        ("NOTICE", cobolt_compiler::NOTICE_TEXT),
        ("POWERRUSTCOBOL-NOTICE.txt", cobolt_compiler::RUNTIME_NOTICE_TEXT),
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
        project.project.name,
        project.project.version,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> CoboltProject {
        CoboltProject::new("T", "src/main.cbl")
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
        assert_eq!(Category::of_path("a.md"), Category::Documentation);
        assert_eq!(Category::of_path("a.png"), Category::Assets);
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
        assert_eq!(p.ide.theme, "", "missing theme → empty (resolves to default)");
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
}
