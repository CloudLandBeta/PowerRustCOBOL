// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Building, serving and previewing a Web or Mobile project** (WASM-002,
//! WASM-008, WASM-015).
//!
//! The IDE and the coding-agent tools need three things from the machinery
//! that turns a project into a bundle: *build* it, say *where* the bundle is,
//! and *serve* it. They are a trait, [`WebBuildBackend`], so the IDE's Build
//! button, its Preview and the `build` tool are written once, against the
//! trait, whatever stands behind it:
//!
//! - **serving** is real already — [`devserver`], a loopback static server
//!   with the right MIME types, the isolation headers and live reload;
//! - **building** is the program-module generator and the bundler, which are
//!   other work (`cobolt-wasm`, `cobolt-web-bundle`). Until they are linked,
//!   the backend is [`NotYetLinked`], which fails with an error the IDE
//!   translates and the tools state plainly. Linking the real one is one call,
//!   [`link`], made once where the program starts; no screen changes.
//!
//! [`preview`] is the web view's side: the protocol with the helper process
//! that owns the system web view, and the launcher that falls back to the
//! system browser. [`presets`] are the device sizes the Preview offers.

pub mod devserver;
pub mod generator;
pub mod presets;
pub mod preview;

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use cobolt_compiler::project_kind::{ModuleVariants, ProjectKind, WebSettings};

pub use devserver::{ServeError, ServeOptions, ServerHandle};

/// The start of a report line the IDE says in the developer's language
/// (WASM-002 T21): the size-budget warning and the missing player. The tools
/// keep the English text.
pub use cobolt_web_bundle::OVER_BUDGET_PREFIX;
pub use generator::PLAYER_UNAVAILABLE_PREFIX;

/// Where a Build puts a Web bundle, relative to the project: the `dist/`
/// folder every kind already delivers into, and `web` inside it.
pub const BUNDLE_SUBDIR: &str = "dist/web";

/// Everything a build reads, taken from the manifest: what the project is, how
/// it is set up and which files are its forms, programs and Backend.
#[derive(Debug, Clone)]
pub struct WebBuildRequest {
    pub manifest: PathBuf,
    pub project_dir: PathBuf,
    pub name: String,
    pub kind: ProjectKind,
    /// The Web settings; for a Mobile project, the Web settings its bundle is
    /// built with (the engine matrix of its platform, no isolation headers).
    pub web: WebSettings,
    /// The program-module variants to emit (WASM-017 R9).
    pub variants: ModuleVariants,
    /// Project-relative forms (`webforms/…` or `mobileforms/…`).
    pub forms: Vec<String>,
    /// Project-relative Common Code.
    pub sources: Vec<String>,
    /// Project-relative Backend files.
    pub backend: Vec<String>,
    /// Project-relative Assets.
    pub assets: Vec<String>,
    /// Where the bundle goes: `<project>/dist/web`.
    pub out_dir: PathBuf,
    /// The project's default form theme (`[forms] theme`), for the backdrop a
    /// see-through theme needs on a page; `None` when the manifest sets none.
    pub form_theme: Option<String>,
    /// The folder of the current player, as the program that builds resolves
    /// its bundled assets (the IDE: beside its executable, WASM-016 T14);
    /// `None` = [`generator::player_dir`].
    pub player_dir: Option<PathBuf>,
}

impl WebBuildRequest {
    /// Read the request from the project at `manifest`. `Err` says why it is
    /// not a Web or Mobile project, or why the manifest cannot be read.
    pub fn from_manifest(manifest: &Path) -> Result<Self, String> {
        let view = cobolt_compiler::project_manifest_view(manifest)?;
        if view.kind.is_desktop() {
            return Err(
                "this is a Desktop project; the web build is for a Web or Mobile project (create one with the 'web' or a mobile template)"
                    .to_owned(),
            );
        }
        let project_dir = manifest.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let form_theme = std::fs::read_to_string(manifest)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| t.get("forms")?.get("theme")?.as_str().map(str::to_owned));
        let (web, variants) = match (&view.web, &view.mobile) {
            (Some(web), _) if view.kind == ProjectKind::Web => (web.clone(), web.module_variants()),
            (_, Some(mobile)) if view.kind == ProjectKind::Mobile => {
                use cobolt_compiler::project_kind::{BrowserMatrix, Isolation, MobilePlatform};
                let engines = BrowserMatrix {
                    chromium: false,
                    firefox: false,
                    safari: false,
                    ios_webview: mobile.platform == MobilePlatform::Ios,
                    android_webview: mobile.platform == MobilePlatform::Android,
                };
                let web = WebSettings {
                    style: mobile.style,
                    browsers: engines,
                    multi_memory: mobile.multi_memory,
                    isolation: Isolation::Off,
                    backend_url: mobile.backend_url.clone(),
                    player: mobile.player.clone(),
                    max_payload_bytes: mobile.max_payload_bytes,
                    call_timeout_ms: mobile.call_timeout_ms,
                    // A Mobile build's variants are its platform's (below).
                    single_memory_fallback: true,
                };
                (web, mobile.module_variants())
            }
            // A manifest without its table is a project with every default.
            _ => {
                let web = WebSettings::default();
                let variants = web.module_variants();
                (web, variants)
            }
        };
        Ok(Self {
            out_dir: project_dir.join(BUNDLE_SUBDIR),
            manifest: manifest.to_path_buf(),
            project_dir,
            name: view.name,
            kind: view.kind,
            web,
            variants,
            forms: view.forms,
            sources: view.sources,
            backend: view.backend,
            assets: view.assets,
            form_theme,
            player_dir: None,
        })
    }
}

/// One part of a bundle and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSize {
    /// `program.wasm`, `player.wasm`, `app.js`, `app.css`, `assets` …
    pub name: String,
    pub bytes: u64,
}

/// The size of a bundle, in total and by part (WASM-002 R8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleSize {
    pub total_bytes: u64,
    pub parts: Vec<PartSize>,
}

impl BundleSize {
    /// The size of every file under `dir`, one part per file (a sub-folder's
    /// files named by their path), total and sorted largest first.
    pub fn of_dir(dir: &Path) -> Self {
        fn walk(base: &Path, dir: &Path, parts: &mut Vec<PartSize>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, parts);
                } else if let Ok(m) = e.metadata() {
                    let name = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                    parts.push(PartSize { name, bytes: m.len() });
                }
            }
        }
        let mut parts = Vec::new();
        walk(dir, dir, &mut parts);
        parts.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
        Self { total_bytes: parts.iter().map(|p| p.bytes).sum(), parts }
    }
}

/// What a build reports (WASM-002 R8, WASM-017 R16): where the bundle is, how
/// big it is, how fast it should start and what the build has to say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildReport {
    pub bundle_dir: PathBuf,
    pub size: BundleSize,
    /// Start-up, in milliseconds, as far as the build can tell: measured by
    /// running the module's start-up, or `None` when it did not.
    pub startup_ms: Option<f64>,
    /// Things the developer should know that did not stop the build.
    pub warnings: Vec<WebDiagnostic>,
    /// How long the build took.
    pub duration_ms: u64,
    /// The page's `connect-src`: where it may connect — itself and the
    /// configured backend, nothing else (WASM-003 R23, WASM-002 R14).
    pub connect_src: String,
    /// The player the bundle carries, as its build recorded it (WASM-016 R13):
    /// each variant (`plain`, `threaded`) and each part loaded on first use
    /// (`parts/mermaid` …), in bytes after `wasm-opt`.
    pub player: Vec<PartSize>,
}

impl BuildReport {
    /// The report as the tools answer it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "bundle_dir": self.bundle_dir.to_string_lossy(),
            "size": {
                "total_bytes": self.size.total_bytes,
                "parts": self.size.parts.iter().map(|p| serde_json::json!({"name": p.name, "bytes": p.bytes})).collect::<Vec<_>>(),
            },
            "startup_ms": self.startup_ms,
            "warnings": self.warnings.iter().map(WebDiagnostic::to_string).collect::<Vec<_>>(),
            "duration_ms": self.duration_ms,
            "connect_src": self.connect_src,
            "player": self.player.iter().map(|p| serde_json::json!({"name": p.name, "bytes": p.bytes})).collect::<Vec<_>>(),
        })
    }
}

/// What a diagnostic of the generator is about, so that the IDE can say it in
/// the developer's language (WASM-010 D6). The tools keep the English text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebDiagnosticKind {
    /// Free text: said as it is.
    #[default]
    Other,
    /// Refused: the construct runs on the Backend.
    BackendOnly,
    /// Refused: no WebAssembly build can run the construct.
    Unsupported,
    /// Refused: `LAYER::Property` names no layer of the form.
    UnknownLayer,
    /// Refused: a layer has no such property.
    UnknownLayerProperty,
    /// Refused: a write to a control's read-only `Layer`.
    LayerWrite,
    /// Refused: a statement reads more host values than one statement may.
    TooManyHostReads,
    /// Warning: `ACCEPT` from the operator is ignored.
    AcceptIgnored,
    /// Warning: a `REDEFINES` may give a different result than on the desktop.
    RedefinesMayDiffer,
    /// Refused: a form's `Instances` is out of range; the subject is the form
    /// (WASM-017 R2).
    InstancesOutOfRange,
    /// Refused: "multi-memory only" with WebKit engines in the matrix; the
    /// subject names them (WASM-017 R9, AC7).
    MultiMemoryOnlyWithWebKit,
    /// Refused: a control holds a connection string or a key on a web or
    /// mobile form; the subject is `CONTROL.Property`, never the value
    /// (WASM-005 R14, AC9).
    SecretOnForm,
}

/// One refusal or warning of a web build: where, what kind, about what, and the
/// English text the tools answer with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebDiagnostic {
    /// `file:line`, or empty when the diagnostic is about the whole project.
    pub location: String,
    pub kind: WebDiagnosticKind,
    /// The construct, layer, property or limit a translated message names.
    pub subject: String,
    pub message: String,
}

impl std::fmt::Display for WebDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.location.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.location, self.message)
        }
    }
}

impl From<String> for WebDiagnostic {
    /// A line of free text (`file:line: construct — alternative`), kind `Other`.
    fn from(message: String) -> Self {
        WebDiagnostic { message, ..Default::default() }
    }
}

impl From<&str> for WebDiagnostic {
    fn from(message: &str) -> Self {
        WebDiagnostic::from(message.to_owned())
    }
}

/// Why a web build did not produce a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebBuildError {
    /// The generator and the bundler are not linked into this program yet.
    /// The IDE says so in the developer's language; the tools in English.
    NotYetLinked,
    /// The project cannot run in a browser: each entry says where, the
    /// construct and the alternative (`file:line: construct — alternative`
    /// when shown as text, WASM-002 R3).
    Refused(Vec<WebDiagnostic>),
    /// The build failed.
    Failed(String),
    /// The project pins a player (`player = "…"`) the Build cannot use (WASM-016 R11).
    PlayerPin(PlayerPinError),
}

/// Why a pinned player cannot be used (WASM-016 R11, Q6). The IDE says it in
/// the developer's language; the tools in English ([`std::fmt::Display`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerPinError {
    /// No player of that version is kept; `kept` lists the versions that are.
    NotKept { pinned: String, kept: Vec<String> },
    /// The pinned player speaks a host API or reads a description format this
    /// compiler no longer emits; `oldest` is the oldest kept player it can
    /// still build for.
    CannotEmit { pinned: String, abi: u32, description: u32, oldest: Option<String> },
}

impl std::fmt::Display for PlayerPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerPinError::NotKept { pinned, kept } => {
                write!(f, "the project pins player {pinned}, which this IDE does not keep; the players it keeps are {}", kept.join(", "))
            }
            PlayerPinError::CannotEmit { pinned, abi, description, oldest } => {
                write!(f, "the project pins player {pinned} (host API {abi}, description format {description}), which this compiler can no longer build for")?;
                match oldest {
                    Some(o) => write!(f, "; the oldest player it can build for is {o}"),
                    None => Ok(()),
                }
            }
        }
    }
}

impl std::fmt::Display for WebBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebBuildError::NotYetLinked => f.write_str(NOT_YET_LINKED_MESSAGE),
            WebBuildError::Refused(lines) => {
                let lines: Vec<String> = lines.iter().map(WebDiagnostic::to_string).collect();
                write!(f, "the project uses what a browser cannot run:\n{}", lines.join("\n"))
            }
            WebBuildError::Failed(why) => write!(f, "the web build failed: {why}"),
            WebBuildError::PlayerPin(e) => write!(f, "the web build failed: {e}"),
        }
    }
}

impl std::error::Error for WebBuildError {}

/// What the tools say when no generator is linked.
pub const NOT_YET_LINKED_MESSAGE: &str =
    "this program does not include the WebAssembly generator and bundler yet, so a Web or Mobile project cannot be built here; \
     the project, its forms and its Backend can be created, edited and checked";

/// What the IDE and the tools need from the generator and the bundler.
pub trait WebBuildBackend: Send + Sync {
    /// A short name for the Output panel: `cobolt-wasm`, `not linked`.
    fn name(&self) -> &str;

    /// Is a real generator behind this? `false` for [`NotYetLinked`].
    fn linked(&self) -> bool {
        true
    }

    /// Build the bundle for `request` into [`WebBuildRequest::out_dir`].
    ///
    /// **Replace the bundle atomically** (build beside it, then rename): the
    /// Preview serves that folder while it is being rebuilt, and a page that
    /// reloads on a half-written bundle shows nothing. A build that fails must
    /// leave the previous bundle as it was — the Preview keeps showing it.
    fn build(&self, request: &WebBuildRequest) -> Result<BuildReport, WebBuildError>;

    /// Check `request` against the portable subset (WASM-002 R3) the way
    /// [`build`](Self::build) would, writing nothing: `Refused` lists every
    /// construct a browser cannot run. Check and Build share one source of
    /// refusals, the generator.
    fn check(&self, _request: &WebBuildRequest) -> Result<(), WebBuildError> {
        Err(WebBuildError::NotYetLinked)
    }

    /// Where the bundle for `request` is (or will be).
    fn bundle_dir(&self, request: &WebBuildRequest) -> PathBuf {
        request.out_dir.clone()
    }

    /// Serve `dir` on a loopback port and say where. The default is the
    /// development server of this crate; a backend needs to replace it only
    /// to serve something else.
    fn serve(&self, dir: &Path, options: &ServeOptions) -> Result<ServerHandle, ServeError> {
        devserver::start(dir, options)
    }
}

/// The backend used until the real one is linked: it builds nothing and says
/// why. It still serves, so a bundle somebody else produced can be previewed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotYetLinked;

impl WebBuildBackend for NotYetLinked {
    fn name(&self) -> &str {
        "not linked"
    }

    fn linked(&self) -> bool {
        false
    }

    fn build(&self, _request: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
        Err(WebBuildError::NotYetLinked)
    }
}

static LINKED: RwLock<Option<Arc<dyn WebBuildBackend>>> = RwLock::new(None);

/// Link the real generator and bundler. Called once, where the program starts
/// (the IDE's `main`, `rcrun`'s `mcp` command); a later call replaces the
/// earlier one.
pub fn link(backend: Arc<dyn WebBuildBackend>) {
    *LINKED.write().unwrap_or_else(|p| p.into_inner()) = Some(backend);
}

/// Take the backend away again (tests).
pub fn unlink() {
    *LINKED.write().unwrap_or_else(|p| p.into_inner()) = None;
}

/// The backend in use: the one linked, or [`NotYetLinked`].
pub fn backend() -> Arc<dyn WebBuildBackend> {
    LINKED
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_else(|| Arc::new(NotYetLinked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The registry is process-wide: tests that link one take this lock.
    static REGISTRY: Mutex<()> = Mutex::new(());

    fn project(tag: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-web-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("P.project.toml");
        std::fs::write(&manifest, text).unwrap();
        manifest
    }

    const WEB_MANIFEST: &str = "[project]\nname = \"P.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
        [files]\nforms = [\"webforms/main-form.cfrm\"]\nbackend = [\"backend/src/health.cbl\"]\n\n\
        [web]\nmulti_memory = false\nisolation = \"credentialless\"\n";

    #[test]
    fn a_request_is_read_from_a_web_manifest() {
        let manifest = project("request", WEB_MANIFEST);
        let r = WebBuildRequest::from_manifest(&manifest).unwrap();
        assert_eq!(r.kind, ProjectKind::Web);
        assert_eq!(r.forms, ["webforms/main-form.cfrm"]);
        assert_eq!(r.backend, ["backend/src/health.cbl"]);
        assert!(r.out_dir.ends_with("dist/web"));
        assert_eq!(r.web.isolation, cobolt_compiler::project_kind::Isolation::Credentialless);
        assert!(!r.variants.multi_memory && r.variants.single_memory, "multi-memory off: the single module only");
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn a_mobile_request_takes_its_engines_and_variants_from_the_platform() {
        let manifest = project(
            "mobile",
            "[project]\nname = \"M.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"mobile\"\n\n\
             [mobile]\nplatform = \"ios\"\ndevice = \"phone\"\nmulti_memory = false\n",
        );
        let r = WebBuildRequest::from_manifest(&manifest).unwrap();
        assert_eq!(r.kind, ProjectKind::Mobile);
        assert!(r.web.browsers.ios_webview && !r.web.browsers.android_webview && !r.web.browsers.chromium);
        assert!(r.variants.single_memory && !r.variants.multi_memory, "iOS: single-memory module");
        assert_eq!(r.web.isolation, cobolt_compiler::project_kind::Isolation::Off, "a shell serves no isolation headers");
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn a_desktop_project_has_no_web_build() {
        let manifest = project("desktop", "[project]\nname = \"D.project\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n");
        let e = WebBuildRequest::from_manifest(&manifest).unwrap_err();
        assert!(e.contains("Desktop project"), "{e}");
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn until_a_generator_is_linked_a_build_says_so() {
        let _g = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
        unlink();
        let manifest = project("notlinked", WEB_MANIFEST);
        let request = WebBuildRequest::from_manifest(&manifest).unwrap();
        let b = backend();
        assert!(!b.linked());
        assert_eq!(b.name(), "not linked");
        let e = b.build(&request).unwrap_err();
        assert_eq!(e, WebBuildError::NotYetLinked);
        assert!(e.to_string().contains("WebAssembly generator"), "{e}");
        assert_eq!(b.bundle_dir(&request), request.out_dir);
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    /// The seam the integration uses: link a backend and every caller sees it,
    /// with no change to the code that asks.
    #[test]
    fn a_linked_backend_replaces_the_stub_for_every_caller() {
        let _g = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
        struct Fake;
        impl WebBuildBackend for Fake {
            fn name(&self) -> &str {
                "fake"
            }
            fn build(&self, request: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
                std::fs::create_dir_all(&request.out_dir).map_err(|e| WebBuildError::Failed(e.to_string()))?;
                std::fs::write(request.out_dir.join("index.html"), "<html></html>").unwrap();
                std::fs::write(request.out_dir.join("program.wasm"), b"\0asm\x01\0\0\0").unwrap();
                Ok(BuildReport {
                    bundle_dir: request.out_dir.clone(),
                    size: BundleSize::of_dir(&request.out_dir),
                    startup_ms: Some(12.5),
                    warnings: vec!["a warning".into()],
                    duration_ms: 3,
                    ..BuildReport::default()
                })
            }
        }
        link(Arc::new(Fake));
        let manifest = project("linked", WEB_MANIFEST);
        let request = WebBuildRequest::from_manifest(&manifest).unwrap();
        let b = backend();
        assert!(b.linked() && b.name() == "fake");
        let report = b.build(&request).unwrap();
        assert_eq!(report.size.total_bytes, 13 + 8);
        assert_eq!(report.size.parts[0].name, "index.html", "largest first");
        let j = report.to_json();
        assert_eq!(j["size"]["total_bytes"], 21);
        assert_eq!(j["startup_ms"], 12.5);
        assert_eq!(j["warnings"][0], "a warning");
        // The default `serve` is the development server.
        let server = b.serve(&report.bundle_dir, &ServeOptions::default()).unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"));
        drop(server);
        unlink();
        assert!(!backend().linked(), "unlinked again");
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn an_error_reads_in_plain_words() {
        let refused = WebBuildError::Refused(vec!["webforms/a.cfrm:4: OPEN — use a remote CALL".into()]);
        assert!(refused.to_string().contains("OPEN") && refused.to_string().starts_with("the project uses what a browser cannot run"));
        assert_eq!(WebBuildError::Failed("x".into()).to_string(), "the web build failed: x");
    }
}
