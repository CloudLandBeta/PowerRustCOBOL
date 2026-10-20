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
pub mod secrets;

use std::collections::BTreeMap;

use cobolt_forms::model::{ControlType, EventBinding, Form};
use cobolt_forms::xml::{form_to_string, load_form_from_str};
use cobolt_wasm_abi::manifest::Manifest;
use cobolt_wasm_abi::scan;
use serde_json::{json, Value};

pub use csp::CspOptions;

const LOADER_JS: &str = include_str!("../assets/loader.js");
const WORKER_JS: &str = include_str!("../assets/worker.js");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");
const PAGE_HTML: &str = include_str!("../assets/page.html");
const COI_SW: &str = include_str!("../assets/coi-serviceworker.js");
const COI_REGISTER: &str = include_str!("../assets/coi-register.js");

/// The loader's version, in `PowerRustCOBOL.versions`.
pub const LOADER_VERSION: u32 = 1;
/// The version of the form-description format (WASM-016 R11): the sanitised `.cfrm`.
pub const DESCRIPTION_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStyle {
    Spa,
    Standalone,
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
    pub weblink_schemes: Vec<String>,
    pub recaptcha_site_key: String,
    pub auth_flow: String,
    /// Override the host page's texts (the developer translates them): `loading`, `unsupported`, …
    pub texts: BTreeMap<String, String>,
    /// A release build refuses every `unsafe` setting (WASM-013 R11).
    pub release: bool,
    pub unsafe_plain_ws: bool,
    pub skip_secret_scan: bool,
    pub call_timeout_ms: Option<u64>,
    /// The longest run flagged as high-entropy by the secret scan; 0 turns that off.
    pub entropy_len: usize,
    /// The Maps tile address template (`{z}`, `{x}`, `{y}`); `None` = the OpenStreetMap server, whose
    /// origin the Content-Security-Policy then allows for images when a form has a Maps control.
    pub tile_url: Option<String>,
    /// Further origins images may be loaded from (`img-src`), beyond the bundle itself.
    pub image_origins: Vec<String>,
}

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
            weblink_schemes: Vec::new(),
            recaptcha_site_key: String::new(),
            auth_flow: "in-app".into(),
            texts: BTreeMap::new(),
            release: true,
            unsafe_plain_ws: false,
            skip_secret_scan: false,
            call_timeout_ms: None,
            entropy_len: secrets::DEFAULT_ENTROPY_LEN,
            tile_url: None,
            image_origins: Vec::new(),
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
    /// The application-wide stylesheet (WASM-007 Q3, reserved).
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
    pub csp_meta: String,
    pub csp_header: String,
    /// Directives (and modes) only a response header can deliver.
    pub header_only: Vec<String>,
    pub isolation_headers: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

impl BuildReport {
    /// The report as the Build prints it.
    pub fn text(&self) -> String {
        let mut s = String::new();
        s.push_str("Web bundle\n");
        for l in &self.sizes {
            s.push_str(&format!("  {:>10}  {}\n", l.bytes, l.path));
        }
        s.push_str(&format!("  {:>10}  total\n\n", self.total_bytes));
        s.push_str("Content-Security-Policy\n");
        s.push_str(&format!("  in the page (<meta>): {}\n", self.csp_meta));
        s.push_str(&format!("  only as a response header: {}\n", self.header_only.join(", ")));
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

/// The form description (WASM-016 R3/R4): the `.cfrm` with every piece of program code taken out —
/// handler bodies, the working-storage text, the COBOL-structure blocks, the named procedures and
/// the recycle bin. What is left is what the player draws: controls, properties, layout, the names
/// of the events bound (their code is in the program module, referred to by id).
pub fn form_description(cfrm: &str) -> Result<(Form, String), String> {
    let mut form = load_form_from_str(cfrm).map_err(|e| e.to_string())?;
    fn strip(events: &mut [EventBinding]) {
        for e in events {
            e.code.clear();
        }
    }
    fn walk(controls: &mut [cobolt_forms::model::Control]) {
        for c in controls {
            strip(&mut c.events);
            walk(&mut c.children);
        }
    }
    walk(&mut form.controls);
    strip(&mut form.form_events);
    form.user_ws_source.clear();
    form.cobol_structure = Default::default();
    form.user_procedures.clear();
    form.deleted_code.clear();
    let text = form_to_string(&form).map_err(|e| e.to_string())?;
    Ok((form, text))
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
    if serde_json::to_value(&a.limits).ok() != serde_json::to_value(&b.limits).ok() {
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
    let mut described: BTreeMap<String, (Form, String)> = BTreeMap::new();
    for f in &input.forms {
        match form_description(&f.cfrm) {
            Ok((form, text)) => {
                described.insert(f.name.to_ascii_lowercase(), (form, text));
            }
            Err(e) => problems.push(format!("form `{}` cannot be read: {e}", f.name)),
        }
    }
    if let Some(m) = &manifest {
        let forms_only: BTreeMap<String, Form> = described.iter().map(|(k, (f, _))| (k.clone(), f.clone())).collect();
        problems.extend(sanitised_check(m, &forms_only));
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
        let (_, text) = &described[&mf.name.to_ascii_lowercase()];
        let path = b.add_hashed(&format!("forms/{}.cfrm", mf.name), text.clone().into_bytes());
        let mut entry = json!({"name": mf.name, "url": path, "integrity": b.sri_of(&path), "main": mf.main, "entry": format!("{}.html", mf.name)});
        if let Some(css) = input.forms.iter().find(|f| f.name.eq_ignore_ascii_case(&mf.name)).and_then(|f| f.stylesheet.clone()) {
            let cp = b.add_hashed(&format!("forms/{}.css", mf.name), css.into_bytes());
            entry["css"] = json!({"url": cp, "integrity": b.sri_of(&cp)});
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
    for origin in &o.image_origins {
        if !csp_opts.extra_img.contains(origin) {
            csp_opts.extra_img.push(origin.clone());
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
    let mut assets_cfg = serde_json::Map::new();
    for a in &input.assets {
        let p = b.add_hashed(&format!("assets/{}", a.path.trim_start_matches("Assets/").trim_start_matches('/')), a.bytes.clone());
        assets_cfg.insert(a.path.clone(), Value::String(p));
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
    let loader_path = b.add_hashed("loader.js", LOADER_JS.as_bytes().to_vec());
    let worker_path = b.add_hashed("worker.js", WORKER_JS.as_bytes().to_vec());
    let app_js = APP_JS.replace("__LOADER__", &loader_path);
    let app_js_path = b.add_hashed("app.js", app_js.into_bytes());
    let app_css_path = b.add_hashed("app.css", APP_CSS.as_bytes().to_vec());

    // texts, options and the config the loader reads
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
        "forms": forms_cfg,
        "menus": menus_cfg,
        "assets": Value::Object(assets_cfg),
        "backend": {"origins": backend_origins, "unsafe_plain_ws": o.unsafe_plain_ws, "call_timeout_ms": o.call_timeout_ms},
        "options": {
            "threads": if o.threads == Threads::Off { "off" } else { "auto" },
            "workers": o.workers,
            "debug_level": o.debug_level,
            "locale": o.locale,
            "weblink_schemes": o.weblink_schemes,
            "recaptcha_site_key": o.recaptcha_site_key,
            "auth_flow": o.auth_flow,
            "tile_url": o.tile_url.clone().unwrap_or_default(),
            "image_origins": csp_opts.extra_img.clone(),
            "embed": match &o.embed { Embed::Window => json!({"mode": "window"}), Embed::Element { selector } => json!({"mode": "element", "selector": selector}) },
        },
        "texts": Value::Object(texts),
        "versions": {"abi": cobolt_wasm_abi::ABI_VERSION, "description": DESCRIPTION_VERSION, "loader": LOADER_VERSION},
    });
    if let Some(s) = stylesheet_cfg {
        config["stylesheet"] = s;
    }
    let config_text = serde_json::to_string_pretty(&config).expect("config serialises");
    b.add("config.json".into(), config_text.into_bytes());

    // the page(s)
    let policy = csp::build(&csp_opts, &backend_origins, &o.embed);
    let shim_path = if o.service_worker_shim {
        let sw = b.add_hashed("coi-serviceworker.js", COI_SW.as_bytes().to_vec());
        b.add_hashed("coi-register.js", COI_REGISTER.as_bytes().to_vec());
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
    let render_page = |start: Option<&str>, title: &str| -> String {
        PAGE_HTML
            .replace("{{LANG}}", &esc(&o.locale))
            .replace("{{CSP}}", &esc(&policy.meta_content()))
            .replace("{{TITLE}}", &esc(title))
            .replace("{{APP_CSS}}", &app_css_path)
            .replace("{{APP_CSS_SRI}}", &app_css_sri)
            .replace("{{LOADER}}", &loader_path)
            .replace("{{LOADER_SRI}}", &loader_sri)
            .replace("{{SHIM}}", &shim_html)
            .replace("{{APP_JS}}", &app_js_path)
            .replace("{{APP_JS_SRI}}", &app_js_sri)
            .replace("{{BODY_CLASS}}", body_class)
            .replace("{{APP_CLASS}}", app_class)
            .replace("{{CONFIG}}", "config.json")
            .replace("{{START_ATTR}}", &start.map(|s| format!(" data-pr-start=\"{}\"", esc(s))).unwrap_or_default())
            .replace("{{NOSCRIPT}}", &esc(&texts_all("noscript", "This application needs JavaScript.")))
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
    let hashed_globs: Vec<&str> = vec!["/*.js", "/*.css", "/forms/*", "/assets/*", "/player/*", "/*.wasm"];
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
    if !o.skip_secret_scan {
        let mut findings = Vec::new();
        for f in &b.files {
            if secrets::is_text_path(&f.path) && !f.path.starts_with("player/") && f.path != "sbom.cdx.json" {
                if let Ok(t) = std::str::from_utf8(&f.bytes) {
                    findings.extend(secrets::scan_text(&f.path, t, o.entropy_len));
                }
            }
        }
        // Source assets (stylesheets) were added hashed; scan the originals' names too via the files above.
        let (errs, warns): (Vec<_>, Vec<_>) = findings.into_iter().partition(|f| f.severity == secrets::Severity::Error);
        if !errs.is_empty() {
            return Err(BundleError { problems: errs.iter().map(|f| f.to_string()).collect() });
        }
        warnings.extend(warns.iter().map(|f| f.to_string()));
    }

    // ── the report ─────────────────────────────────────────────────────────
    let mut sizes: Vec<SizeLine> = b.files.iter().map(|f| SizeLine { path: f.path.clone(), bytes: f.bytes.len() as u64 }).collect();
    sizes.sort_by(|a, c| a.path.cmp(&c.path));
    let total_bytes = sizes.iter().map(|s| s.bytes).sum();
    let isolation_headers: Vec<(String, String)> = headers.iter().filter(|(k, _)| k.starts_with("Cross-Origin")).cloned().collect();
    let report = BuildReport {
        sizes,
        total_bytes,
        csp_meta: policy.meta_content(),
        csp_header: policy.header_value(),
        header_only: policy.header_only_directives(),
        isolation_headers,
        warnings,
        notes,
    };
    Ok(BundleOutput { files: b.files, report })
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
