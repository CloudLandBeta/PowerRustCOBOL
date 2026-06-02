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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name:    String,
    pub version: String,
    /// Relative path (from project root) to the main COBOL source file.
    pub main:    String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectFiles {
    #[serde(default)]
    pub sources: Vec<String>, // relative paths
    #[serde(default)]
    pub forms:   Vec<String>,
    #[serde(default)]
    pub assets:  Vec<String>,
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
            },
            files:   ProjectFiles::default(),
            runtime: RuntimeConfig::default(),
        }
    }

    // ── File membership helpers ───────────────────────────────────────────────

    /// Add a file to the appropriate list (deduplicates).
    /// `rel` must be a relative path string (e.g. `"src/main.cbl"`).
    pub fn add_file(&mut self, rel: &str) {
        let kind = FileKind::from_path(rel);
        let list = match kind {
            FileKind::Source => &mut self.files.sources,
            FileKind::Form   => &mut self.files.forms,
            FileKind::Asset  => &mut self.files.assets,
        };
        let rel = rel.replace('\\', "/");
        if !list.contains(&rel) {
            list.push(rel);
        }
    }

    /// Remove a file from whichever list it belongs to.
    pub fn remove_file(&mut self, rel: &str) {
        let rel = rel.replace('\\', "/");
        self.files.sources.retain(|f| f != &rel);
        self.files.forms.retain(|f| f != &rel);
        self.files.assets.retain(|f| f != &rel);
    }

    /// True if `rel` is tracked by the project.
    pub fn contains(&self, rel: &str) -> bool {
        let rel = rel.replace('\\', "/");
        self.files.sources.contains(&rel)
            || self.files.forms.contains(&rel)
            || self.files.assets.contains(&rel)
    }

    /// All tracked files (sources + forms + assets) as relative path strings.
    pub fn all_files(&self) -> impl Iterator<Item = &str> {
        self.files.sources.iter()
            .chain(self.files.forms.iter())
            .chain(self.files.assets.iter())
            .map(|s| s.as_str())
    }
}

// ── FileKind ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Source,
    Form,
    Asset,
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
            "cfrm"                 => FileKind::Form,
            _                     => FileKind::Asset,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Source => "Sources",
            FileKind::Form   => "Forms",
            FileKind::Asset  => "Assets",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            FileKind::Source => "📄",
            FileKind::Form   => "🗔",
            FileKind::Asset  => "📦",
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
