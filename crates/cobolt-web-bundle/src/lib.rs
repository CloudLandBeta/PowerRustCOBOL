// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Web bundle** (WASM-002, WASM-013 R2/R3/R7/R10, WASM-017 R9/R18–R20).
//!
//! [`assemble_bundle`] lays out the static files of a Web application: the host page, the loader,
//! the program module in both memory variants, the prebuilt player in both thread variants, the
//! manifest, the form descriptions, the stylesheets, the assets, the response headers for the
//! documented static hosts and a software bill of materials — all with content-hashed names and
//! Subresource Integrity values — **and refuses to lay out anything wrong**: a program module with an
//! import beyond ABI v1 §3, a `memory.grow`, a single-memory variant that uses a multi-memory
//! instruction, forms that disagree with the program, or a secret anywhere in the bundle is one
//! readable list of problems, never a half-written folder.
//!
//! It takes bytes and returns bytes: no file is read or written here (the caller — the IDE's Build —
//! owns the folder), which is what makes it testable and makes the build reproducible.

pub mod csp;
pub mod devserver;
pub mod hash;
pub mod hostpage;
pub mod notice;
pub mod secrets;

use std::collections::BTreeMap;

use cobolt_forms::description::{code_free, describe, to_json, FormDescription};
use cobolt_forms::model::{ControlType, Form};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::Manifest;
use cobolt_wasm_abi::scan;
use serde_json::{json, Value};

pub use csp::CspOptions;

/// The loader's version, in `PowerRustCOBOL.versions`.
pub const LOADER_VERSION: u32 = 1;
/// The version of the form-description format (WASM-016 R11): versioned JSON, owned by `cobolt-forms`.
pub const DESCRIPTION_VERSION: u32 = cobolt_forms::description::DESCRIPTION_FORMAT;

/// The size budget of the first download (WASM-002 Q5, answered 2026-10-22 "5 MB", **to confirm**). Working
/// assumption: it caps the first download — the larger program module the probe can pick, the plain
/// player, the loader and the page's glue, the page and its stylesheet — counted in 10⁶ bytes; over it
/// is a warning, never a failure, and there is no first-paint target yet.
pub const FIRST_DOWNLOAD_BUDGET: u64 = 5_000_000;

/// The first-download line of the report (WASM-002 R8, Q5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetLine {
    pub first_download_bytes: u64,
    pub budget_bytes: u64,
}

impl BudgetLine {
    pub fn over(&self) -> bool {
        self.first_download_bytes > self.budget_bytes
    }
}

/// The start of the budget warning, for a caller that shows it in another language.
pub const OVER_BUDGET_PREFIX: &str = "the first download is";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppStyle {
    /// One page; forms load into the shell's ContentPane or open as in-page dialogs and panels (WASM-011 R2).
    #[default]
    Spa,
    /// Each form is a page of its own.
    Standalone,
}

impl AppStyle {
    /// The style as `cobolt.toml` spells it (`[web] style`, `cobolt_compiler::project_kind::ApplicationStyle::as_str`).
    pub fn as_str(self) -> &'static str {
        match self {
            AppStyle::Spa => "spa",
            AppStyle::Standalone => "standalone",
        }
    }

    /// The bundle's style for the project's `[web] style` / `[mobile] style` (WASM-011 R1): the one mapping the Build
    /// and the test fixtures share. `None` for a name that is no style.
    pub fn from_name(name: &str) -> Option<AppStyle> {
        [AppStyle::Spa, AppStyle::Standalone].into_iter().find(|s| s.as_str().eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Embed {
    /// The application is the page and fills the window.
    Window,
    /// The application mounts into an element of someone else's page.
    Element { selector: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    Off,
    RequireCorp,
    Credentialless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threads {
    Auto,
    Off,
}

#[derive(Debug, Clone)]
pub struct FormInput {
    pub name: String,
    /// The `.cfrm` as the designer saved it (with the handler code; the assembler removes it).
    pub cfrm: String,
    /// The form's default stylesheet (WASM-007), if it has one.
    pub stylesheet: Option<String>,
}

/// The menu of one MenuBar or SideMenu control: the `<Control>.menu.yaml` file the IDE writes beside
/// the form, with its integrity hash (a tampered file is refused, as it is on the desktop).
#[derive(Debug, Clone)]
pub struct MenuInput {
    pub form: String,
    pub control: String,
    pub yaml: String,
}

#[derive(Debug, Clone)]
pub struct AssetInput {
    /// The path forms use, e.g. `Assets/logo.png`.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// One built variant of the player: the files below its directory, with the two entry points named.
#[derive(Debug, Clone)]
pub struct PlayerVariantInput {
    pub files: Vec<(String, Vec<u8>)>,
    pub entry_js: String,
    pub entry_wasm: String,
}

#[derive(Debug, Clone, Default)]
pub struct SbomComponent {
    pub name: String,
    pub version: String,
    pub license: String,
}

/// A part of the player, a module of its own that the player loads on first use (WASM-016 R13).
#[derive(Debug, Clone)]
pub struct PlayerPartInput {
    /// The name the player asks for it by (`mermaid`).
    pub name: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub entry_js: String,
    pub entry_wasm: String,
}

#[derive(Debug, Clone, Default)]
pub struct PlayerInput {
    pub plain: Option<PlayerVariantInput>,
    pub threaded: Option<PlayerVariantInput>,
    /// The parts loaded on demand; the same for both variants.
    pub parts: Vec<PlayerPartInput>,
    /// `player-build.json` of `build_player`: tool versions, nightly date, profile, sizes.
    pub build_info: Option<String>,
    /// The player's dependency list, for the SBOM.
    pub components: Vec<SbomComponent>,
}

#[derive(Debug, Clone)]
pub struct BundleOptions {
    pub style: AppStyle,
    pub embed: Embed,
    pub title: String,
    pub locale: String,
    pub backend_origins: Vec<String>,
    pub csp: CspOptions,
    pub isolation: Isolation,
    pub threads: Threads,
    /// The Worker pool size; `None` = `hardwareConcurrency - 1`, capped at 8.
    pub workers: Option<u32>,
    /// Add the optional service-worker shim for hosts that cannot set headers (off by default).
    pub service_worker_shim: bool,
    pub debug_level: u32,
    /// The project's extra link schemes for its Hyperlinks (WASM-006 R7), written to `config.json` as
    /// `options.link_schemes`.
    pub link_schemes: Vec<String>,
    pub recaptcha_site_key: String,
    pub auth_flow: String,
    /// The backend's identity secrets as they resolve here (WASM-009 D14): the bundle must hold none
    /// of their values. Searched even when `skip_secret_scan` is set: a value known to be a secret
    /// is never a false finding.
    pub known_secrets: secrets::KnownSecrets,
    /// The form a signed-out user is sent to when they open one that requires sign-in (`[web]
    /// sign_in_form`, WASM-009 D11); its description says so. `None`: the player takes the first form
    /// holding an Authentication control.
    pub sign_in_form: Option<String>,
    /// Override the host page's texts (the developer translates them): `loading`, `unsupported`, …; the player's
    /// "connecting" modal reads `connecting`, `unreachable` and `reload` (WASM-016 R20–R21), and its navigation
    /// notices `popup_blocked`, `route_not_found` (`{form}` is the name), `leave_unsaved`, `leave` and `stay`
    /// (WASM-011, `cobolt_web_player::texts::KEYS`).
    pub texts: BTreeMap<String, String>,
    /// Does the application follow and write the page's address (WASM-011 R5, Q7)? `None`: when it is the page
    /// ([`Embed::Window`]) yes, when it is mounted into someone else's element no — the host page's address is the
    /// host page's. Written to `config.json` as `options.embed.address` (`"own"` / `"none"`).
    pub own_address: Option<bool>,
    /// A phone's navigation stack (WASM-011 R13, Q5): every form opened over another is a full screen. `options.stack`.
    pub stack: bool,
    /// A release build refuses every `unsafe` setting (WASM-013 R11).
    pub release: bool,
    pub unsafe_plain_ws: bool,
    pub skip_secret_scan: bool,
    pub call_timeout_ms: Option<u64>,
    /// The largest frame the player sends or accepts, in bytes (WASM-017 R8:
    /// the project's payload limit); `None` keeps the player's own default.
    pub max_frame: Option<u32>,
    /// The backend contract version the application is built against (`backend/exports.toml`
    /// `contract.version`), which the player names in `hello` (WASM-014 R2); `None` = the player's default.
    pub backend_contract: Option<String>,
    /// The exported programs declared `idempotent`, which the player may repeat after a lost connection (R9).
    pub backend_idempotent: Vec<String>,
    /// The longest run flagged as high-entropy by the secret scan; 0 turns that off.
    pub entropy_len: usize,
    /// The Maps tile address template (`{z}`, `{x}`, `{y}`); `None` = the OpenStreetMap server, whose
    /// origin the Content-Security-Policy then allows for images when a form has a Maps control.
    pub tile_url: Option<String>,
    /// Further origins images may be loaded from (`img-src`), beyond the bundle itself.
    pub image_origins: Vec<String>,
    /// The colour painted behind the application when its form theme is see-through
    /// (Spatial). On the desktop the blurred desktop shows through; a page has nothing
    /// behind it, so without this the theme's white text sat on the page's own colour —
    /// white in a light scheme. `None` leaves the page's colour.
    pub backdrop: Option<String>,
    /// The web theme the player adopts as `pr-theme` (WASM-007 Q5), written as `theme.css`: the rules of a
    /// built-in theme (`cobolt_forms::web_themes::theme_rules`) or the project's whole-theme file
    /// (`[web] theme_file`), unlayered — the player puts it in the layer. `None`: the player's own Liquid Glass.
    pub theme: Option<String>,
    /// The browsers the stylesheets are checked against (WASM-007 R10).
    pub browsers: cobolt_forms::stylesheet_lint::Browsers,
}

/// The stand-in face the desktop's painter letters "Arial" in (egui's built-in proportional font), shipped with
/// every Web bundle so that a form reads the same on both surfaces (`assets/web-fonts`).
/// The asset-table key the stand-in face is listed under for the player's graphics parts.
pub const STAND_IN_ASSET_KEY: &str = "0-stand-in/ubuntu-light.ttf";

const STAND_IN_FONT: &[u8] = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");
/// Its licence, which travels with it.
const STAND_IN_FONT_LICENCE: &[u8] = include_bytes!("../../../assets/web-fonts/UFL.txt");

impl Default for BundleOptions {
    fn default() -> Self {
        BundleOptions {
            style: AppStyle::Spa,
            embed: Embed::Window,
            title: "PowerRustCOBOL application".into(),
            locale: "en".into(),
            backend_origins: Vec::new(),
            csp: CspOptions::strict(),
            isolation: Isolation::Credentialless,
            threads: Threads::Auto,
            workers: None,
            service_worker_shim: false,
            debug_level: 0,
            link_schemes: Vec::new(),
            recaptcha_site_key: String::new(),
            auth_flow: "in-app".into(),
            known_secrets: secrets::KnownSecrets::default(),
            sign_in_form: None,
            texts: BTreeMap::new(),
            own_address: None,
            stack: false,
            release: true,
            unsafe_plain_ws: false,
            skip_secret_scan: false,
            call_timeout_ms: None,
            max_frame: None,
            backend_contract: None,
            backend_idempotent: Vec::new(),
            entropy_len: secrets::DEFAULT_ENTROPY_LEN,
            tile_url: None,
            image_origins: Vec::new(),
            backdrop: None,
            theme: None,
            browsers: cobolt_forms::stylesheet_lint::Browsers::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BundleInput {
    pub project: String,
    /// The multi-memory program module (the default variant) and the single-memory one. A bundle needs at
    /// least one; one whose browser matrix includes Safari needs both (the Build decides, WASM-017 R9).
    pub program_multi: Option<Vec<u8>>,
    pub program_single: Option<Vec<u8>>,
    pub forms: Vec<FormInput>,
    /// The application-wide stylesheet (WASM-007 Q3), copied byte for byte; the player puts it in `@layer pr-app`.
    pub stylesheet: Option<String>,
    /// The menus of the forms' MenuBar and SideMenu controls.
    pub menus: Vec<MenuInput>,
    pub assets: Vec<AssetInput>,
    pub player: PlayerInput,
    pub options: BundleOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeLine {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct BuildReport {
    /// Every file with its size (WASM-002 R8).
    pub sizes: Vec<SizeLine>,
    pub total_bytes: u64,
    /// The first download against [`FIRST_DOWNLOAD_BUDGET`].
    pub budget: BudgetLine,
    pub csp_meta: String,
    pub csp_header: String,
    /// Directives (and modes) only a response header can deliver.
    pub header_only: Vec<String>,
    pub isolation_headers: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
    /// The origins the stylesheets load from (WASM-007 R13, AC7): images (`url()` in a declaration,
    /// allowed by `img-src`) and fonts (`@font-face`, allowed by `font-src`). Sorted, no repeats.
    pub stylesheet_img_sources: Vec<String>,
    pub stylesheet_font_sources: Vec<String>,
    /// The bytes of the font files the bundle carries: the stand-in face and every font file under `Assets/`
    /// (WASM-004 Q5), against [`BuildReport::total_bytes`].
    pub font_bytes: u64,
    /// How many files the secret scan read, and what it said that did not stop the Build (a finding
    /// names the file, the line and the pattern, never the value). `None` when the scan was skipped.
    pub secret_scan: Option<SecretScanLine>,
}

/// The secret scan's account of itself (WASM-015 R18).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecretScanLine {
    pub scanned_files: usize,
    pub findings: Vec<String>,
}

impl BuildReport {
    /// The report as the Build prints it.
    pub fn text(&self) -> String {
        let mut s = String::new();
        s.push_str("Web bundle\n");
        for l in &self.sizes {
            s.push_str(&format!("  {:>10}  {}\n", l.bytes, l.path));
        }
        s.push_str(&format!("  {:>10}  total\n", self.total_bytes));
        s.push_str(&format!("  {:>10}  of it font files ({}%)\n", self.font_bytes, font_percent(self.font_bytes, self.total_bytes)));
        let over = if self.budget.over() { ", over" } else { "" };
        s.push_str(&format!("  {:>10}  first download (budget {} bytes{over})\n\n", self.budget.first_download_bytes, self.budget.budget_bytes));
        s.push_str("Content-Security-Policy\n");
        s.push_str(&format!("  in the page (<meta>): {}\n", self.csp_meta));
        s.push_str(&format!("  only as a response header: {}\n", self.header_only.join(", ")));
        s.push_str(&format!("  the stylesheets load images from (img-src): {}\n", or_none(&self.stylesheet_img_sources)));
        s.push_str(&format!("  the stylesheets load fonts from (font-src): {}\n", or_none(&self.stylesheet_font_sources)));
        if !self.isolation_headers.is_empty() {
            s.push_str("\nCross-origin isolation (threads) needs these response headers; resources from other origins must then allow it (CORS or Cross-Origin-Resource-Policy):\n");
            for (k, v) in &self.isolation_headers {
                s.push_str(&format!("  {k}: {v}\n"));
            }
        }
        for n in &self.notes {
            s.push_str(&format!("\nnote: {n}"));
        }
        for w in &self.warnings {
            s.push_str(&format!("\nwarning: {w}"));
        }
        s.push('\n');
        s
    }
}

fn or_none(v: &[String]) -> String {
    if v.is_empty() {
        "none beyond the bundle".to_owned()
    } else {
        v.join(" ")
    }
}

/// The errors of one stylesheet as the Build reports them, `file:line: message` (WASM-007 R13, R16):
/// `label` names the file. Warnings (an unknown property, a value that does not parse) do not stop a
/// build; the stylesheet editor shows them.
pub fn stylesheet_errors(label: &str, css: &str, allowed_origins: &[String], browsers: cobolt_forms::stylesheet_lint::Browsers, layers: &[String]) -> Vec<String> {
    use cobolt_forms::stylesheet_lint::{lint, message, Options, Severity};
    lint(css, &Options { allowed_origins, browsers, layers })
        .iter()
        .filter(|d| d.code.severity() == Severity::Error)
        .map(|d| format!("{label}:{}: {}", d.line, message(d)))
        .collect()
}

/// The origins a stylesheet may load from (WASM-007 R13): what the images and fonts of the page may.
pub fn stylesheet_origins(o: &BundleOptions) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for x in o.image_origins.iter().chain(&o.csp.extra_img).chain(&o.csp.extra_font) {
        if !v.contains(x) {
            v.push(x.clone());
        }
    }
    v
}

/// The Preview's CSS-only fast path (WASM-007 R11, T23): a bundle's `config.json` with form `form`'s stylesheet
/// replaced by `css`, and the sheet file it now names — what the Build would write for that sheet, without a
/// Build. Only that form's `css` entry changes, so the page's `update()` takes it with `set_form_stylesheet`
/// alone; the form's description keeps naming the sheet it was built with (the player does not read that
/// name). The sheet is checked as the Build checks it (the same origins, the form's `layers`); a sheet with an
/// error is refused, `file:line: message`, and the Build that follows reports it. Like [`assemble_bundle`]
/// this reads and writes no file.
pub fn with_form_stylesheet(config_json: &[u8], form: &str, css: &str, layers: &[String]) -> Result<(BundleFile, Vec<u8>), Vec<String>> {
    let mut config: Value = serde_json::from_slice(config_json).map_err(|e| vec![format!("config.json: {e}")])?;
    let Some(entry) = config["forms"].as_array_mut().and_then(|fs| fs.iter_mut().find(|f| f["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(form)))) else {
        return Err(vec![format!("config.json: the bundle has no form `{form}`")]);
    };
    // The name the Build wrote, whatever the caller's spelling.
    let label = format!("forms/{}.css", entry["name"].as_str().unwrap_or(form));
    let problems = stylesheet_errors(&label, css, &stylesheet_origins(&BundleOptions::default()), cobolt_forms::stylesheet_lint::Browsers::default(), layers);
    if !problems.is_empty() {
        return Err(problems);
    }
    let bytes = css.as_bytes().to_vec();
    let path = hash::hashed_name(&label, &bytes);
    entry["css"] = json!({"url": path, "integrity": hash::sri(&bytes)});
    let text = serde_json::to_string_pretty(&config).expect("config serialises");
    Ok((BundleFile { path, bytes }, text.into_bytes()))
}

#[derive(Debug, Clone)]
pub struct BundleOutput {
    pub files: Vec<BundleFile>,
    pub report: BuildReport,
}

impl BundleOutput {
    pub fn file(&self, path: &str) -> Option<&BundleFile> {
        self.files.iter().find(|f| f.path == path)
    }
    pub fn text(&self, path: &str) -> Option<String> {
        self.file(path).map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
    }
    /// The `config.json`, parsed.
    pub fn config(&self) -> Value {
        serde_json::from_slice(&self.file("config.json").expect("config.json").bytes).expect("valid json")
    }
}

/// Everything wrong with the input, in words.
#[derive(Debug, Clone)]
pub struct BundleError {
    pub problems: Vec<String>,
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "the Web bundle cannot be built:")?;
        for p in &self.problems {
            writeln!(f, "  - {p}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BundleError {}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn manifest_of(bytes: &[u8]) -> Result<Manifest, String> {
    scan::scan_module(bytes)?.embedded_manifest()
}

/// The form description (WASM-016 R3/R4): the `.cfrm` read by the one parser, then described as
/// versioned JSON by `cobolt_forms::description`, which has no place for program code — handler
/// bodies, the working-storage text, the COBOL-structure blocks, the named procedures, the recycle bin.
/// What is left is what the player draws: controls, properties, layout, the names of the events bound
/// (their code is in the program module, referred to by id). Also answers the code-free form, which
/// the checks against the program read.
pub fn form_description(cfrm: &str) -> Result<(Form, FormDescription), String> {
    let form = load_form_from_str(cfrm).map_err(|e| e.to_string())?;
    Ok((code_free(&form), describe(&form)))
}

/// Compare what the two variants (and the forms) say about the program: same forms, controls, names,
/// events and limits. A bundle whose variants disagree would behave differently per browser.
fn variants_agree(a: &Manifest, b: &Manifest) -> Vec<String> {
    let mut p = Vec::new();
    if a.project != b.project {
        p.push("the two program variants name different projects".into());
    }
    if a.names != b.names || a.events != b.events {
        p.push("the two program variants disagree about the names or events tables: they were not built from the same program".into());
    }
    if serde_json::to_value(&a.forms).ok() != serde_json::to_value(&b.forms).ok() {
        p.push("the two program variants disagree about the forms, controls or handlers".into());
    }
    // `data_pages` is the size of memory 0, which differs by construction: the
    // single-memory variant holds the arena there (ABI §2). Every other limit
    // must agree.
    let same_limits = |m: &Manifest| {
        let mut l = m.limits.clone();
        l.data_pages = 0;
        serde_json::to_value(&l).ok()
    };
    if same_limits(a) != same_limits(b) {
        p.push("the two program variants have different limits".into());
    }
    p
}

fn sanitised_check(manifest: &Manifest, forms: &BTreeMap<String, Form>) -> Vec<String> {
    let mut p = Vec::new();
    for f in &manifest.forms {
        let Some(form) = forms.get(&f.name.to_ascii_lowercase()) else {
            p.push(format!("the program has a form `{}` but no form description was given for it", f.name));
            continue;
        };
        for c in &f.controls {
            match form.controls.iter().find(|x| x.id.eq_ignore_ascii_case(&c.name)) {
                None => p.push(format!("form `{}`: the program addresses control `{}`, which the form description does not contain", f.name, c.name)),
                Some(x) if ControlType::from_str(&c.kind) != x.control_type => p.push(format!(
                    "form `{}`: control `{}` is a {} in the program but a {} in the form description",
                    f.name,
                    c.name,
                    c.kind,
                    x.control_type.as_str()
                )),
                Some(_) => {}
            }
        }
    }
    for name in forms.keys() {
        if manifest.form_by_name(name).is_none() {
            p.push(format!("a form description `{name}` was given that is not a form of the program"));
        }
    }
    p
}

struct Builder {
    files: Vec<BundleFile>,
    /// path → integrity
    sri: BTreeMap<String, String>,
}

impl Builder {
    fn add(&mut self, path: String, bytes: Vec<u8>) -> String {
        self.sri.insert(path.clone(), hash::sri(&bytes));
        self.files.push(BundleFile { path: path.clone(), bytes });
        path
    }
    fn add_hashed(&mut self, path: &str, bytes: Vec<u8>) -> String {
        let name = hash::hashed_name(path, &bytes);
        self.add(name, bytes)
    }
    fn sri_of(&self, path: &str) -> String {
        self.sri.get(path).cloned().unwrap_or_default()
    }
}

/// The imports a program module needs that the player does not give it (WASM-016 R11, AC6): a player and a
/// module that disagree are a Build error, never a refusal at run time. The player's own record says what it
/// gives (`host_imports` in `player-build.json`); a player without a record is not judged here.
fn player_lacks_imports(input: &BundleInput) -> Vec<String> {
    let Some(rec) = input.player.build_info.as_deref().and_then(|t| serde_json::from_str::<Value>(t).ok()) else { return Vec::new() };
    let Some(given) = rec["host_imports"].as_array() else { return Vec::new() };
    let given: Vec<&str> = given.iter().filter_map(Value::as_str).collect();
    let player = rec["versions"]["player"].as_str().unwrap_or("?");
    let mut out = Vec::new();
    for (name, bytes) in [("program.wasm", &input.program_multi), ("program.single.wasm", &input.program_single)] {
        let Some(scan) = bytes.as_deref().and_then(|b| scan::scan_module(b).ok()) else { continue };
        for imp in &scan.imports {
            let full = format!("{}.{}", imp.module, imp.name);
            if !given.contains(&full.as_str()) && !out.iter().any(|p: &String| p.contains(&format!("`{full}`"))) {
                out.push(format!("{name} imports `{full}`, which the player {player} does not provide: build with the player that matches this compiler, or pin one that provides it"));
            }
        }
    }
    out
}

/// Lay out the bundle, or say everything that is wrong.
pub fn assemble_bundle(input: &BundleInput) -> Result<BundleOutput, BundleError> {
    let o = &input.options;
    let mut problems: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    // ── the program modules ────────────────────────────────────────────────
    if input.program_multi.is_none() && input.program_single.is_none() {
        problems.push("the bundle has no program module".into());
    }
    let mut manifest_multi = None;
    let mut manifest_single = None;
    if let Some(b) = &input.program_multi {
        let pr = scan::bundle_problems(b, false);
        problems.extend(pr.iter().map(|p| format!("program.wasm (multi-memory): {p}")));
        if pr.is_empty() {
            manifest_multi = manifest_of(b).ok();
        }
    }
    if let Some(b) = &input.program_single {
        let pr = scan::bundle_problems(b, true);
        problems.extend(pr.iter().map(|p| format!("program.single.wasm (single-memory): {p}")));
        if pr.is_empty() {
            manifest_single = manifest_of(b).ok();
        }
    }
    if let (Some(a), Some(b)) = (&manifest_multi, &manifest_single) {
        problems.extend(variants_agree(a, b));
    }
    let manifest = manifest_multi.clone().or(manifest_single.clone());
    if input.program_multi.is_none() {
        notes.push("no multi-memory program module: this bundle runs only where the single-memory variant does (every browser)".into());
    }
    if input.program_single.is_none() {
        warnings.push("no single-memory program module: browsers without multi-memory (Safari, the iOS web view) cannot run this bundle (WASM-017 R9)".into());
    }

    // ── the player ─────────────────────────────────────────────────────────
    if input.player.plain.is_none() && input.player.threaded.is_none() {
        problems.push("the bundle has no player".into());
    }
    if input.player.plain.is_none() {
        problems.push("the bundle has no plain player: the plain player is the baseline every browser and every page can run".into());
    }
    if input.player.threaded.is_none() && o.threads == Threads::Auto {
        notes.push("no threaded player: the application runs on the main thread everywhere".into());
    }
    problems.extend(player_lacks_imports(input));

    // ── options ────────────────────────────────────────────────────────────
    if o.release {
        if o.unsafe_plain_ws {
            problems.push("`unsafe_plain_ws` is set: a release build refuses plain ws:// to a non-local address (WASM-013 R11)".into());
        }
        if o.skip_secret_scan {
            problems.push("the secret scan is switched off: a release build refuses that (WASM-013 R11)".into());
        }
    } else {
        if o.unsafe_plain_ws {
            warnings.push("unsafe: plain ws:// to a non-local backend is allowed in this (non-release) build".into());
        }
        if o.skip_secret_scan {
            warnings.push("unsafe: the secret scan is skipped in this (non-release) build".into());
        }
    }
    for b in &o.backend_origins {
        let lower = b.to_ascii_lowercase();
        let local = lower.starts_with("ws://localhost") || lower.starts_with("ws://127.0.0.1") || lower.starts_with("ws://[::1]");
        if !(lower.starts_with("wss://") || local || (lower.starts_with("ws://") && o.unsafe_plain_ws)) {
            problems.push(format!("backend address `{b}` must be wss:// (or ws:// to the local machine)"));
        }
    }
    if matches!(o.embed, Embed::Element { .. }) && o.style == AppStyle::Standalone {
        notes.push("embedded mode supports the single-page style; the standalone entry pages are still written but open as pages of their own".into());
    }

    // ── the forms ──────────────────────────────────────────────────────────
    let mut described: BTreeMap<String, (Form, FormDescription)> = BTreeMap::new();
    for f in &input.forms {
        match form_description(&f.cfrm) {
            Ok((form, description)) => {
                described.insert(f.name.to_ascii_lowercase(), (form, description));
            }
            Err(e) => problems.push(format!("form `{}` cannot be read: {e}", f.name)),
        }
    }
    if let Some(m) = &manifest {
        let forms_only: BTreeMap<String, Form> = described.iter().map(|(k, (f, _))| (k.clone(), f.clone())).collect();
        problems.extend(sanitised_check(m, &forms_only));
    }

    // ── the stylesheets (WASM-007 R13, R16): checked, then copied byte for byte ──
    let allowed = stylesheet_origins(o);
    let (mut sheet_img, mut sheet_font) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    let mut sheets: Vec<(String, &str, Vec<String>)> = Vec::new();
    for f in &input.forms {
        if let Some(css) = &f.stylesheet {
            let layers = described.get(&f.name.to_ascii_lowercase()).map(|(form, _)| form.layers.iter().map(|l| l.name.clone()).collect()).unwrap_or_default();
            sheets.push((format!("forms/{}.css", f.name), css, layers));
        }
    }
    if let Some(css) = &input.stylesheet {
        sheets.push(("stylesheet.css".into(), css, Vec::new()));
    }
    if let Some(css) = &o.theme {
        sheets.push(("theme.css".into(), css, Vec::new()));
    }
    for (label, css, layers) in &sheets {
        problems.extend(stylesheet_errors(label, css, &allowed, o.browsers, layers));
        let (img, font) = cobolt_forms::stylesheet_lint::sources(css);
        sheet_img.extend(img);
        sheet_font.extend(font);
    }

    if !problems.is_empty() {
        return Err(BundleError { problems });
    }
    let manifest = manifest.expect("checked above");

    // ── files ──────────────────────────────────────────────────────────────
    let mut b = Builder { files: Vec::new(), sri: BTreeMap::new() };
    let manifest_json = serde_json::to_string_pretty(&manifest).expect("manifest serialises");
    let manifest_path = b.add_hashed("manifest.json", manifest_json.into_bytes());
    let program_multi = input.program_multi.as_ref().map(|bytes| b.add_hashed("program.wasm", bytes.clone()));
    let program_single = input.program_single.as_ref().map(|bytes| b.add_hashed("program.single.wasm", bytes.clone()));

    let mut forms_cfg = Vec::new();
    for mf in &manifest.forms {
        let mut description = described[&mf.name.to_ascii_lowercase()].1.clone();
        description.form.sign_in_form = o.sign_in_form.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&mf.name));
        // The stylesheet first, so the description can name it (WASM-016 R3).
        let css = input.forms.iter().find(|f| f.name.eq_ignore_ascii_case(&mf.name)).and_then(|f| f.stylesheet.clone()).map(|css| {
            let cp = b.add_hashed(&format!("forms/{}.css", mf.name), css.into_bytes());
            description.stylesheet = Some(cp.clone());
            json!({"url": cp, "integrity": b.sri_of(&cp)})
        });
        let path = b.add_hashed(&format!("forms/{}.json", mf.name), to_json(&description).into_bytes());
        let mut entry = json!({"name": mf.name, "url": path, "integrity": b.sri_of(&path), "main": mf.main, "entry": format!("{}.html", mf.name)});
        if let Some(css) = css {
            entry["css"] = css;
        }
        forms_cfg.push(entry);
    }
    // The menus: parsed (and their integrity hash checked) here, written as JSON the player reads.
    let mut menus_cfg = Vec::new();
    for m in &input.menus {
        match cobolt_forms::menu::parse_menu(&m.yaml) {
            Ok(def) => {
                let json_text = serde_json::to_string(&def).expect("a menu serialises");
                let p = b.add_hashed(&format!("forms/{}.{}.menu.json", m.form, m.control), json_text.into_bytes());
                menus_cfg.push(json!({"form": m.form, "control": m.control, "url": p, "integrity": b.sri_of(&p)}));
            }
            Err(e) => return Err(BundleError { problems: vec![format!("the menu of control `{}` in form `{}` cannot be used: {e}", m.control, m.form)] }),
        }
    }
    // Images: the bundle's own, `data:` and `blob:` are always allowed; a Maps control loads tiles from
    // the tile server, so exactly that origin is added when some form has one.
    let uses_maps = described.values().any(|(f, _)| {
        fn has(cs: &[cobolt_forms::model::Control]) -> bool {
            cs.iter().any(|c| c.control_type == ControlType::Maps || has(&c.children))
        }
        has(&f.controls)
    });
    let mut csp_opts = o.csp.clone();
    csp_opts.shim = o.service_worker_shim;
    // The notice of a page opened from disk (`notice`): one inline script, allowed by its hash and never
    // by 'unsafe-inline', so that the policy stays default-deny.
    let notice_source = notice::script_source();
    if !csp_opts.extra_script.contains(&notice_source) {
        csp_opts.extra_script.push(notice_source);
    }
    for origin in o.image_origins.iter().chain(&sheet_img) {
        if !csp_opts.extra_img.contains(origin) {
            csp_opts.extra_img.push(origin.clone());
        }
    }
    // A font a stylesheet loads from an allowed origin needs that origin in `font-src` (R13).
    for origin in &sheet_font {
        if !csp_opts.extra_font.contains(origin) {
            csp_opts.extra_font.push(origin.clone());
        }
    }
    if uses_maps {
        let tile = o.tile_url.clone().unwrap_or_else(|| "https://tile.openstreetmap.org/{z}/{x}/{y}.png".into());
        if let Some(origin) = origin_of(&tile) {
            if !csp_opts.extra_img.contains(&origin) {
                csp_opts.extra_img.push(origin.clone());
                notes.push(format!("a form has a Maps control: the Content-Security-Policy allows images from {origin} (the map tiles)"));
            }
        }
    }
    // The Authentication control's identity requests go to the backend's `/auth/*` routes over HTTPS
    // (WASM-009 D5): the backend's https origin joins `connect-src`, only when a form has the control.
    let uses_auth = described.values().any(|(f, _)| {
        fn has(cs: &[cobolt_forms::model::Control]) -> bool {
            cs.iter().any(|c| c.control_type == ControlType::Authentication || has(&c.children))
        }
        has(&f.controls)
    });
    if uses_auth {
        for origin in o.backend_origins.iter().filter_map(|b| http_origin_of_backend(b)) {
            if !csp_opts.extra_connect.contains(&origin) {
                csp_opts.extra_connect.push(origin.clone());
                notes.push(format!("a form has an Authentication control: the Content-Security-Policy allows requests to {origin} (the backend's /auth/* routes, WASM-009)"));
            }
        }
    }
    // reCAPTCHA v3 (WASM-009 R4): the Authentication control loads Google's script and the script loads
    // its own from gstatic; its badge is a frame. Exactly those prefixes are added, and only when a
    // site key is configured.
    if !o.recaptcha_site_key.trim().is_empty() {
        for s in ["https://www.google.com/recaptcha/", "https://www.gstatic.com/recaptcha/"] {
            if !csp_opts.extra_script.iter().any(|x| x == s) {
                csp_opts.extra_script.push(s.into());
            }
        }
        for s in ["https://www.google.com/recaptcha/", "https://recaptcha.google.com/recaptcha/"] {
            if !csp_opts.extra_frame.iter().any(|x| x == s) {
                csp_opts.extra_frame.push(s.into());
            }
        }
        notes.push("a reCAPTCHA site key is configured: the Content-Security-Policy allows Google's reCAPTCHA scripts and frame (WASM-009 R4)".into());
    }
    let stylesheet_cfg = input.stylesheet.as_ref().map(|css| {
        let p = b.add_hashed("stylesheet.css", css.clone().into_bytes());
        json!({"url": p, "integrity": b.sri_of(&p)})
    });
    let theme_cfg = o.theme.as_ref().map(|css| {
        let p = b.add_hashed("theme.css", css.clone().into_bytes());
        json!({"url": p, "integrity": b.sri_of(&p)})
    });
    let mut assets_cfg = serde_json::Map::new();
    for a in &input.assets {
        let p = b.add_hashed(&format!("assets/{}", a.path.trim_start_matches("Assets/").trim_start_matches('/')), a.bytes.clone());
        assets_cfg.insert(a.path.clone(), Value::String(p));
    }
    // The project's own fonts, for the families its forms name (WASM-004 Q5); a family it does not supply is named.
    let font_plan = font_faces(described.values().map(|(f, _)| f), &input.assets);
    let faces_css: String = font_plan.faces.iter().map(|f| f.rule(assets_cfg[&f.asset].as_str().unwrap_or_default())).collect();
    for (family, forms) in &font_plan.unbundled {
        warnings.push(format!(
            "{UNBUNDLED_FONT_PREFIX} `{family}` (form {}) is not bundled: no font file under Assets/ supplies it, and a font only the operating system provides is not shipped; the page letters it in the visitor's own copy, else in the stand-in face",
            forms.join(", ")
        ));
    }

    // the player variants live in a content-addressed directory
    let mut player_cfg = serde_json::Map::new();
    for (key, v) in [("plain", &input.player.plain), ("threaded", &input.player.threaded)] {
        let Some(v) = v else {
            player_cfg.insert(key.into(), Value::Null);
            continue;
        };
        let mut all: Vec<&(String, Vec<u8>)> = v.files.iter().collect();
        all.sort_by(|x, y| x.0.cmp(&y.0));
        let mut id = sha2_concat(&all);
        id.truncate(8);
        let dir = format!("player/{key}-{id}");
        let mut js = String::new();
        let mut wasm = String::new();
        for (rel, bytes) in all {
            let path = format!("{dir}/{rel}");
            b.add(path.clone(), bytes.clone());
            if *rel == v.entry_js {
                js = path.clone();
            }
            if *rel == v.entry_wasm {
                wasm = path;
            }
        }
        player_cfg.insert(key.into(), json!({"js": js, "wasm": wasm, "js_integrity": b.sri_of(&js), "wasm_integrity": b.sri_of(&wasm)}));
    }
    // The parts load on first use and not before: they are listed with their sizes, never linked from the page.
    let mut parts_cfg = serde_json::Map::new();
    for part in &input.player.parts {
        let mut all: Vec<&(String, Vec<u8>)> = part.files.iter().collect();
        all.sort_by(|x, y| x.0.cmp(&y.0));
        let mut id = sha2_concat(&all);
        id.truncate(8);
        let dir = format!("player/part-{}-{id}", part.name);
        let (mut js, mut wasm) = (String::new(), String::new());
        let mut bytes_total = 0usize;
        for (rel, bytes) in all {
            let path = format!("{dir}/{rel}");
            bytes_total += bytes.len();
            b.add(path.clone(), bytes.clone());
            if *rel == part.entry_js {
                js = path.clone();
            }
            if *rel == part.entry_wasm {
                wasm = path;
            }
        }
        if js.is_empty() || wasm.is_empty() {
            return Err(BundleError { problems: vec![format!("the player part `{}` lacks its entry files `{}` and `{}`", part.name, part.entry_js, part.entry_wasm)] });
        }
        parts_cfg.insert(part.name.clone(), json!({"js": js, "wasm": wasm, "js_integrity": b.sri_of(&js), "wasm_integrity": b.sri_of(&wasm), "bytes": bytes_total}));
        notes.push(format!("the `{}` part ({bytes_total} bytes) is loaded on first use, not at start", part.name));
    }
    player_cfg.insert("parts".into(), Value::Object(parts_cfg));

    // the loader and the worker
    let loader_path = b.add_hashed("loader.js", hostpage::loader_js().into_bytes());
    let worker_path = b.add_hashed("worker.js", hostpage::worker_js().into_bytes());
    let gfx_worker_path = b.add_hashed("gfx-worker.js", hostpage::gfx_worker_js().into_bytes());
    let app_js_path = b.add_hashed("app.js", hostpage::app_js(&loader_path).into_bytes());
    // The face the desktop draws "Arial" in, so the two surfaces letter the same text the same way, and its licence beside it
    // (the Ubuntu Font Licence asks for that). A shadow root cannot declare a font: the page's own stylesheet does.
    let font_path = b.add_hashed("fonts/Ubuntu-Light.ttf", STAND_IN_FONT.to_vec());
    b.add("fonts/UFL.txt".into(), STAND_IN_FONT_LICENCE.to_vec());
    // The same face is the first font of every graphics registry the player makes (WASM-018 T27): the desktop registers it
    // first too, so a generic or unknown family is lettered alike on every host. The key sorts before every project path.
    assets_cfg.insert(STAND_IN_ASSET_KEY.to_owned(), Value::String(font_path.clone()));
    let app_css_path = b.add_hashed("app.css", hostpage::app_css(o.backdrop.as_deref(), &font_path, &faces_css).into_bytes());

    // texts, options and the config the loader reads
    // Whose the page's address is (WASM-011 R5, Q7): the page's own application's, not one mounted into another page.
    let embed_cfg = {
        let address = if o.own_address.unwrap_or(matches!(o.embed, Embed::Window)) { "own" } else { "none" };
        match &o.embed {
            Embed::Window => json!({"mode": "window", "address": address}),
            Embed::Element { selector } => json!({"mode": "element", "selector": selector, "address": address}),
        }
    };
    let mut texts = serde_json::Map::new();
    for (k, v) in &o.texts {
        texts.insert(k.clone(), Value::String(v.clone()));
    }
    let backend_origins = o.backend_origins.clone();
    let mut config = json!({
        "v": 1,
        "project": manifest.project,
        "style": if o.style == AppStyle::Spa { "spa" } else { "standalone" },
        "program": {
            "multi": program_multi.as_ref().map(|p| json!({"url": p, "integrity": b.sri_of(p)})),
            "single": program_single.as_ref().map(|p| json!({"url": p, "integrity": b.sri_of(p)})),
        },
        "manifest": {"url": manifest_path, "integrity": b.sri_of(&manifest_path)},
        "player": Value::Object(player_cfg),
        "worker": {"url": worker_path, "integrity": b.sri_of(&worker_path)},
        "gfx_worker": {"url": gfx_worker_path, "integrity": b.sri_of(&gfx_worker_path)},
        "forms": forms_cfg,
        "menus": menus_cfg,
        "assets": Value::Object(assets_cfg),
        "backend": {"origins": backend_origins, "unsafe_plain_ws": o.unsafe_plain_ws, "call_timeout_ms": o.call_timeout_ms, "max_frame": o.max_frame, "contract": o.backend_contract, "idempotent": o.backend_idempotent},
        "options": {
            "threads": if o.threads == Threads::Off { "off" } else { "auto" },
            "workers": o.workers,
            "debug_level": o.debug_level,
            "locale": o.locale,
            "link_schemes": o.link_schemes,
            "recaptcha_site_key": o.recaptcha_site_key,
            "auth_flow": o.auth_flow,
            "tile_url": o.tile_url.clone().unwrap_or_default(),
            // What the player lets a stylesheet load from at run time (`css_is_safe`): the images' origins and the
            // fonts', the list the Build checked the sheets against (WASM-007 R13).
            "image_origins": csp_opts.extra_img.iter().chain(csp_opts.extra_font.iter().filter(|f| !csp_opts.extra_img.contains(f))).cloned().collect::<Vec<_>>(),
            "embed": embed_cfg,
            "stack": o.stack,
        },
        "texts": Value::Object(texts),
        "versions": {"abi": cobolt_wasm_abi::ABI_VERSION, "description": DESCRIPTION_VERSION, "loader": LOADER_VERSION},
    });
    if let Some(s) = stylesheet_cfg {
        config["stylesheet"] = s;
    }
    if let Some(t) = theme_cfg {
        config["theme"] = t;
    }
    let config_text = serde_json::to_string_pretty(&config).expect("config serialises");
    b.add("config.json".into(), config_text.into_bytes());

    // the page(s)
    let policy = csp::build(&csp_opts, &backend_origins, &o.embed);
    let shim_path = if o.service_worker_shim {
        let sw = b.add_hashed("coi-serviceworker.js", hostpage::coi_serviceworker_js().into_bytes());
        b.add_hashed("coi-register.js", hostpage::coi_register_js().into_bytes());
        Some(sw)
    } else {
        None
    };
    let shim_html = match (&shim_path, b.files.iter().find(|f| f.path.starts_with("coi-register."))) {
        (Some(sw), Some(reg)) => format!(
            "<script src=\"{}\" integrity=\"{}\" data-worker=\"{}\"></script>\n",
            reg.path,
            hash::sri(&reg.bytes),
            sw
        ),
        _ => String::new(),
    };
    let texts_all = |key: &str, default: &str| o.texts.get(key).cloned().unwrap_or_else(|| default.to_owned());
    let (body_class, app_class) = match &o.embed {
        Embed::Window => ("pr-window", "pr-window"),
        Embed::Element { .. } => ("", "pr-embedded"),
    };
    let (app_css_sri, loader_sri, app_js_sri) = (b.sri_of(&app_css_path), b.sri_of(&loader_path), b.sri_of(&app_js_path));
    let csp_meta = policy.meta_content();
    let noscript = texts_all("noscript", "This application needs JavaScript.");
    let render_page = |start: Option<&str>, title: &str| -> String {
        hostpage::page_html(&hostpage::Page {
            lang: &o.locale,
            csp: &csp_meta,
            title,
            app_css: &app_css_path,
            app_css_sri: &app_css_sri,
            font: &font_path,
            loader: &loader_path,
            loader_sri: &loader_sri,
            shim: &shim_html,
            app_js: &app_js_path,
            app_js_sri: &app_js_sri,
            body_class,
            app_class,
            config: "config.json",
            start,
            noscript: &noscript,
        })
    };
    b.add("index.html".into(), render_page(None, &o.title).into_bytes());
    if o.style == AppStyle::Standalone {
        for mf in &manifest.forms {
            let title = format!("{} — {}", mf.name, o.title);
            b.add(format!("{}.html", mf.name), render_page(Some(&mf.name), &title).into_bytes());
        }
    }

    // headers for the documented hosts
    let headers = csp::security_headers(&policy, o.isolation);
    let hashed_globs: Vec<&str> = vec!["/*.js", "/*.css", "/forms/*", "/assets/*", "/player/*", "/*.wasm", "/fonts/Ubuntu-Light.*"];
    for (path, content) in csp::host_files(&headers, &hashed_globs) {
        b.add(path, content.into_bytes());
    }

    // SBOM (CycloneDX) and integrity of the whole
    let mut components: Vec<Value> = b
        .files
        .iter()
        .map(|f| json!({"type": "file", "name": f.path, "hashes": [{"alg": "SHA-256", "content": hash::sha256_hex(&f.bytes)}]}))
        .collect();
    for c in &input.player.components {
        components.push(json!({"type": "library", "name": c.name, "version": c.version, "licenses": [{"license": {"name": c.license}}]}));
    }
    components.push(json!({"type": "data", "name": "Ubuntu Light", "licenses": [{"license": {"name": "Ubuntu-font-1.0"}}]}));
    let mut bom = json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "name": manifest.project}},
        "components": components,
    });
    if let Some(info) = &input.player.build_info {
        if let Ok(v) = serde_json::from_str::<Value>(info) {
            bom["metadata"]["properties"] = json!([{"name": "player-build", "value": v.to_string()}]);
        }
    }
    b.add("sbom.cdx.json".into(), serde_json::to_string_pretty(&bom).expect("bom").into_bytes());

    // ── the secret scan, over every text the bundle holds ──────────────────
    let mut secret_scan = None;
    if !o.skip_secret_scan {
        let mut findings = Vec::new();
        let mut scanned_files = 0usize;
        for f in &b.files {
            if secrets::is_text_path(&f.path) && !f.path.starts_with("player/") && f.path != "sbom.cdx.json" {
                if let Ok(t) = std::str::from_utf8(&f.bytes) {
                    scanned_files += 1;
                    findings.extend(secrets::scan_text(&f.path, t, o.entropy_len));
                }
            }
        }
        secret_scan = Some(SecretScanLine { scanned_files, findings: findings.iter().map(|f| f.to_string()).collect() });
        // Source assets (stylesheets) were added hashed; scan the originals' names too via the files above.
        let (errs, warns): (Vec<_>, Vec<_>) = findings.into_iter().partition(|f| f.severity == secrets::Severity::Error);
        if !errs.is_empty() {
            return Err(BundleError { problems: errs.iter().map(|f| f.to_string()).collect() });
        }
        warnings.extend(warns.iter().map(|f| f.to_string()));
    }
    if !o.known_secrets.0.is_empty() {
        let found: Vec<String> = b
            .files
            .iter()
            .filter(|f| secrets::is_text_path(&f.path))
            .filter_map(|f| std::str::from_utf8(&f.bytes).ok().map(|t| secrets::scan_values(&f.path, t, &o.known_secrets)))
            .flatten()
            .map(|f| f.to_string())
            .collect();
        if !found.is_empty() {
            return Err(BundleError { problems: found });
        }
    }

    // ── the first download against the budget (R8, Q5: a warning only) ─────
    let size_of = |path: &str| b.files.iter().find(|f| f.path == path).map_or(0, |f| f.bytes.len() as u64);
    let program_bytes = [&input.program_multi, &input.program_single].iter().filter_map(|p| p.as_ref().map(|v| v.len() as u64)).max().unwrap_or(0);
    let player_bytes = input.player.plain.as_ref().map_or(0, |v| {
        v.files.iter().filter(|(rel, _)| *rel == v.entry_js || *rel == v.entry_wasm).map(|(_, bytes)| bytes.len() as u64).sum()
    });
    let page_bytes = [&loader_path, &app_js_path, &app_css_path, "index.html"].iter().map(|p| size_of(p)).sum::<u64>();
    let budget = BudgetLine { first_download_bytes: program_bytes + player_bytes + page_bytes, budget_bytes: FIRST_DOWNLOAD_BUDGET };
    if budget.over() {
        warnings.push(format!(
            "{OVER_BUDGET_PREFIX} {} bytes, over the budget of {} bytes (the program module, the plain player, the loader, the page and its stylesheet)",
            budget.first_download_bytes, budget.budget_bytes
        ));
    }

    // ── the report ─────────────────────────────────────────────────────────
    let mut sizes: Vec<SizeLine> = b.files.iter().map(|f| SizeLine { path: f.path.clone(), bytes: f.bytes.len() as u64 }).collect();
    sizes.sort_by(|a, c| a.path.cmp(&c.path));
    let total_bytes = sizes.iter().map(|s| s.bytes).sum();
    // Q5: over half of the application in font files is a warning, never a failure.
    let font_bytes = STAND_IN_FONT.len() as u64 + input.assets.iter().filter(|a| font_format(&a.path).is_some()).map(|a| a.bytes.len() as u64).sum::<u64>();
    if font_bytes * 2 > total_bytes {
        warnings.push(format!(
            "{FONT_SHARE_PREFIX} {font_bytes} bytes, {}% of the application's {total_bytes} bytes (more than half): consider the system fonts only, by naming no font of your own (the stand-in face stays)",
            font_percent(font_bytes, total_bytes)
        ));
    }
    let isolation_headers: Vec<(String, String)> = headers.iter().filter(|(k, _)| k.starts_with("Cross-Origin")).cloned().collect();
    let report = BuildReport {
        sizes,
        total_bytes,
        budget,
        csp_meta: policy.meta_content(),
        csp_header: policy.header_value(),
        header_only: policy.header_only_directives(),
        isolation_headers,
        warnings,
        notes,
        stylesheet_img_sources: sheet_img.into_iter().collect(),
        stylesheet_font_sources: sheet_font.into_iter().collect(),
        font_bytes,
        secret_scan,
    };
    Ok(BundleOutput { files: b.files, report })
}

/// The origin a backend address (`wss://host[:port]/path`, or `ws://` to the local machine) answers
/// HTTPS on (`http://` for `ws://`): where the player sends the `/auth/*` requests.
pub fn http_origin_of_backend(address: &str) -> Option<String> {
    let (scheme, rest) = address.split_once("://")?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "wss" | "https" => "https",
        "ws" | "http" => "http",
        _ => return None,
    };
    origin_of(&format!("{scheme}://{rest}"))
}

/// `https://host[:port]` of an address (a tile template included), or `None` when it names no origin.
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "https" && scheme != "http" {
        return None;
    }
    let host = rest.split(['/', '?', '#']).next()?;
    if host.is_empty() || host.contains('{') {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

// ── fonts (WASM-004 Q5: bundle the fonts; warn above half the application) ──

/// The start of the warning when the font files are more than half of the bundle, for a caller that shows it
/// in another language.
pub const FONT_SHARE_PREFIX: &str = "the font files are";
/// The start of the warning that names a family a form uses and the bundle does not carry.
pub const UNBUNDLED_FONT_PREFIX: &str = "the font family";

/// The font files a browser loads with `@font-face`, by extension, and the `format()` each is declared in.
const FONT_FORMATS: [(&str, &str); 4] = [(".ttf", "truetype"), (".otf", "opentype"), (".woff2", "woff2"), (".woff", "woff")];

fn font_format(path: &str) -> Option<&'static str> {
    let p = path.to_ascii_lowercase();
    FONT_FORMATS.iter().find(|(ext, _)| p.ends_with(ext)).map(|(_, f)| *f)
}

/// Whether the desktop letters `family` in its built-in face, the stand-in the bundle always carries:
/// `cobolt_forms::fonts::is_builtin` (behind the `render` feature, which this crate does not build), the rule
/// the player's `css::font_family` spells too.
fn is_builtin_family(family: &str) -> bool {
    let f = family.trim();
    f.is_empty() || f.eq_ignore_ascii_case("Arial") || f.eq_ignore_ascii_case("Helvetica") || f.eq_ignore_ascii_case("sans-serif")
}

/// A family name as two spellings of it are compared: case, spaces, hyphens and underscores ignored.
fn family_key(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).flat_map(char::to_lowercase).collect()
}

/// One face the bundle declares for a family a form names (WASM-004 Q5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFace {
    /// The family as the form spells it: the name the player's `font-family` asks for.
    pub family: String,
    /// The file under `Assets/` that supplies it, as the project names it.
    pub asset: String,
    /// `truetype`, `opentype`, `woff2` or `woff`.
    pub format: &'static str,
    /// The face's weight from its `OS/2` table; `None` when the file is the family's only face (it then
    /// serves every weight, as the stand-in does) or cannot be read (WOFF and WOFF2 are compressed).
    pub weight: Option<u16>,
    pub italic: bool,
}

impl FontFace {
    /// The `@font-face` rule, with the file at `url` (its hashed name in the bundle).
    pub fn rule(&self, url: &str) -> String {
        let weight = self.weight.map_or_else(|| "100 900".to_owned(), |w| w.to_string());
        let style = if self.italic { "italic" } else { "normal" };
        format!(
            "@font-face {{ font-family: \"{}\"; src: url(\"{url}\") format(\"{}\"); font-weight: {weight}; font-style: {style}; font-display: block; }}\n",
            self.family, self.format
        )
    }
}

/// What the bundle does with the families its forms name: the faces it declares, and the families it does
/// not carry, each with the forms that name it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontPlan {
    pub faces: Vec<FontFace>,
    pub unbundled: Vec<(String, Vec<String>)>,
}

/// The names a font file answers to: the families its `name` table declares (the family and the typographic
/// family), and its file name without the extension.
fn names_of_font(path: &str, bytes: &[u8]) -> (Vec<String>, Option<u16>, bool) {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let mut names = vec![family_key(stem)];
    let (mut weight, mut italic) = (None, false);
    if let Ok(face) = ttf_parser::Face::parse(bytes, 0) {
        for n in face.names() {
            if n.name_id == ttf_parser::name_id::FAMILY || n.name_id == ttf_parser::name_id::TYPOGRAPHIC_FAMILY {
                if let Some(s) = n.to_string() {
                    names.push(family_key(&s));
                }
            }
        }
        weight = Some(face.weight().to_number());
        italic = face.is_italic();
    }
    names.sort();
    names.dedup();
    (names, weight, italic)
}

/// The fonts of a bundle (WASM-004 Q5, plan §1.6): every family a control names in `FontName` that is not the
/// built-in one is looked for among the project's font files under `Assets/` — by the family the file declares,
/// or by its file name — and each file that supplies it becomes a face. A family no file supplies is **not**
/// copied from the developer's machine (an operating system's fonts are not ours to ship): it is listed, and the
/// page letters it in the visitor's own copy, else in the stand-in face.
pub fn font_faces<'a>(forms: impl IntoIterator<Item = &'a Form>, assets: &[AssetInput]) -> FontPlan {
    fn walk(cs: &[cobolt_forms::model::Control], form: &str, out: &mut BTreeMap<String, (String, Vec<String>)>) {
        for c in cs {
            if let Some(cobolt_forms::model::PropValue::String(n)) = c.properties.get("FontName") {
                let spelled = n.trim().replace('"', "");
                if !is_builtin_family(&spelled) {
                    let e = out.entry(family_key(&spelled)).or_insert_with(|| (spelled, Vec::new()));
                    if !e.1.iter().any(|f| f == form) {
                        e.1.push(form.to_owned());
                    }
                }
            }
            walk(&c.children, form, out);
        }
    }
    let mut named: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for f in forms {
        walk(&f.controls, &f.name, &mut named);
    }
    let mut files: Vec<(&AssetInput, &'static str, Vec<String>, Option<u16>, bool)> = assets
        .iter()
        .filter_map(|a| {
            let format = font_format(&a.path)?;
            let (names, weight, italic) = names_of_font(&a.path, &a.bytes);
            Some((a, format, names, weight, italic))
        })
        .collect();
    files.sort_by(|x, y| x.0.path.cmp(&y.0.path));
    let mut plan = FontPlan::default();
    for (key, (spelled, forms)) in named {
        let supplying: Vec<_> = files.iter().filter(|f| f.2.contains(&key)).collect();
        if supplying.is_empty() {
            plan.unbundled.push((spelled, forms));
            continue;
        }
        let only = supplying.len() == 1;
        for (a, format, _, weight, italic) in supplying {
            plan.faces.push(FontFace {
                family: spelled.clone(),
                asset: a.path.clone(),
                format,
                weight: if only { None } else { *weight },
                italic: *italic && !only,
            });
        }
    }
    plan
}

/// `part` as a whole percentage of `whole`, rounded to the nearest.
fn font_percent(part: u64, whole: u64) -> u64 {
    (part * 100 + whole / 2).checked_div(whole).unwrap_or(0)
}

fn sha2_concat(files: &[&(String, Vec<u8>)]) -> String {
    let mut all = Vec::new();
    for (p, bytes) in files {
        all.extend_from_slice(p.as_bytes());
        all.push(0);
        all.extend_from_slice(hash::sha256_hex(bytes).as_bytes());
        all.push(0);
    }
    hash::sha256_hex(&all)
}
