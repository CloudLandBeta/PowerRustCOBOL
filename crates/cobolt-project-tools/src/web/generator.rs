// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The real web build** (WASM-010 T19, R1, R15; WASM-002's one way from a
//! project to `dist/web`): the WebAssembly generator and the bundler behind
//! [`WebBuildBackend`].
//!
//! For each form: its `.cfrm` is read and its COBOL **regenerated** by
//! `cobolt-codegen` (the generated-code contract: the module is built from
//! the regenerated program, never from a stale or hand-edited `.cbl`), parsed,
//! and given the project's Common Code (`cobolt_compiler::link_common_code`,
//! as Run Form and the desktop build do). `cobolt_wasm::compile_web_project`
//! emits the variants the project asks for; `cobolt_web_bundle::assemble_bundle`
//! puts them with the player, the form descriptions and the assets; the bundle
//! replaces `dist/web` atomically. The start-up figure is the multi-memory (or
//! the only) module's `pr_init` + `pr_open_form(main)` on `wasmtime`, natively:
//! a proxy measured on this machine, not a browser timing (WASM-010 D5).

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_forms::model::{Control, Form};
use cobolt_wasm::module::{FormSpec, SourceLine};
use cobolt_web_bundle::{
    AppStyle, AssetInput, BundleInput, BundleOptions, FormInput, MenuInput, PlayerInput,
    PlayerPartInput, PlayerVariantInput,
};

use super::{
    BuildReport, BundleSize, PartSize, PlayerPinError, WebBuildBackend, WebBuildError, WebBuildRequest,
    WebDiagnostic, WebDiagnosticKind,
};

/// The generator and the bundler. Linked once where the program starts
/// (`web::link(Arc::new(GeneratorBackend))`).
#[derive(Debug, Clone, Copy, Default)]
pub struct GeneratorBackend;

/// The environment variable naming the folder of the built player, when it is
/// not the repository's `assets/web-player`.
pub const PLAYER_DIR_ENV: &str = "PRC_WEB_PLAYER_DIR";

/// One form, ready for the generator.
struct LoadedForm {
    rel: String,
    form: Form,
    cfrm: String,
    program: cobolt_ast::program::Program,
    lines: Vec<SourceLine>,
    /// The class C method calls of the program as the developer wrote them,
    /// before the lowering replaced them (the backend's programs are made from
    /// these; WASM-005 T11, T14).
    sites: Vec<cobolt_codegen::remote_controls::CallSite>,
}

/// Every control of a form, nested ones included, in the model's order.
fn all_controls(cs: &[Control], out: &mut Vec<Control>) {
    for c in cs {
        out.push(c.clone());
        all_controls(&c.children, out);
    }
}

fn diag(
    location: String,
    kind: WebDiagnosticKind,
    subject: String,
    message: String,
) -> WebDiagnostic {
    WebDiagnostic {
        location,
        kind,
        subject,
        message,
    }
}

fn kind_of(k: cobolt_wasm::ErrorKind) -> WebDiagnosticKind {
    use cobolt_wasm::ErrorKind as E;
    match k {
        E::Other => WebDiagnosticKind::Other,
        E::BackendOnly => WebDiagnosticKind::BackendOnly,
        E::Unsupported => WebDiagnosticKind::Unsupported,
        E::UnknownLayer => WebDiagnosticKind::UnknownLayer,
        E::UnknownLayerProperty => WebDiagnosticKind::UnknownLayerProperty,
        E::LayerWrite => WebDiagnosticKind::LayerWrite,
        E::TooManyHostReads => WebDiagnosticKind::TooManyHostReads,
    }
}

fn warning_kind(k: cobolt_wasm::WarningKind) -> WebDiagnosticKind {
    match k {
        cobolt_wasm::WarningKind::AcceptIgnored => WebDiagnosticKind::AcceptIgnored,
        cobolt_wasm::WarningKind::RedefinesMayDiffer => WebDiagnosticKind::RedefinesMayDiffer,
    }
}

/// `webforms/a.cfrm (A ▸ BTN ▸ onClick):8` — where the developer wrote the
/// generated line `gen_line` of `form`, as Check names it.
fn location(form: Option<&LoadedForm>, gen_line: u32) -> String {
    form.map(|f| location_in(&f.rel, &f.lines, gen_line))
        .unwrap_or_default()
}

fn location_in(rel: &str, lines: &[SourceLine], gen_line: u32) -> String {
    match lines.iter().find(|l| l.gen_line == gen_line) {
        Some(l) => format!("{rel} ({}):{}", l.site, l.line),
        None if gen_line > 0 => format!("{rel}:{gen_line}"),
        None => rel.to_owned(),
    }
}

/// `cfrm` with `theme="<id>"` on its `<Form>` element, unless the form names a theme of its own.
fn with_theme(cfrm: &str, theme: &str) -> String {
    let Some(at) = cfrm.find("<Form ") else { return cfrm.to_owned() };
    let end = cfrm[at..].find('>').map(|e| at + e).unwrap_or(cfrm.len());
    if cfrm[at..end].contains(" theme=") {
        return cfrm.to_owned();
    }
    // A theme id is a catalogue word (`spatial`, `liquid-glass`): nothing else is written into the markup.
    let id: String = theme.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
    format!("{} theme=\"{id}\"{}", &cfrm[..at + "<Form".len()], &cfrm[at + "<Form".len()..])
}

impl GeneratorBackend {
    /// Read, regenerate and parse every form of the request.
    fn load_forms(
        &self,
        req: &WebBuildRequest,
        warnings: &mut Vec<WebDiagnostic>,
    ) -> Result<Vec<LoadedForm>, WebBuildError> {
        let mut forms = Vec::new();
        let mut refused = Vec::new();
        for rel in &req.forms {
            let path = req.project_dir.join(rel);
            let cfrm = std::fs::read_to_string(&path)
                .map_err(|e| WebBuildError::Failed(format!("cannot read {rel}: {e}")))?;
            let form = cobolt_forms::xml::load_form_from_str(&cfrm)
                .map_err(|e| WebBuildError::Failed(format!("{rel}: {e}")))?;
            // The theme the form is drawn in — its own, else the project's default (`[forms] theme`) — travels
            // with the form, so the page draws what the Form Designer draws (a see-through theme above all).
            let theme = cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), req.form_theme.as_deref());
            let cfrm = if theme != cobolt_forms::theme::LIQUID_GLASS { with_theme(&cfrm, &theme) } else { cfrm };
            let form = if form.theme.as_deref().map(str::trim).is_none_or(str::is_empty) && theme != cobolt_forms::theme::LIQUID_GLASS {
                cobolt_forms::xml::load_form_from_str(&cfrm).map_err(|e| WebBuildError::Failed(format!("{rel}: {e}")))?
            } else {
                form
            };
            // WASM-017 R2: a form's instances are fixed memory; out of range
            // refuses Check and Build alike, the form named.
            if let Some(problem) = form.instances_problem() {
                refused.push(diag(
                    rel.clone(),
                    WebDiagnosticKind::InstancesOutOfRange,
                    form.name.clone(),
                    problem,
                ));
                continue;
            }
            // WASM-005 T13 (R14, AC9): a connection string or a key belongs to the
            // backend's `[[configuration]]`, named by the control's BackendConfig —
            // never to a form a browser downloads. Checked before anything is
            // generated; named by control and property, never by value.
            for s in cobolt_codegen::remote_controls::secrets_on_form(&form) {
                let subject = format!("{}.{}", s.control, s.property);
                refused.push(diag(
                    rel.clone(),
                    WebDiagnosticKind::SecretOnForm,
                    subject.clone(),
                    format!(
                        "{subject} holds a connection string or a key on a web form: clear it, put the value in a [[configuration]] of backend/backend.toml and name that configuration in {}",
                        cobolt_forms::web_class::BACKEND_CONFIG
                    ),
                ));
            }
            let (src, map) = cobolt_codegen::generate_with_map(&form);
            let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
                &src,
                cobolt_lexer::SourceFormat::Free,
            ));
            let n = src.lines().count() as u32;
            let lines: Vec<SourceLine> = (1..=n)
                .filter_map(|g| {
                    map.resolve(g).map(|(site, l)| SourceLine {
                        gen_line: g,
                        site: site.display_path(&form.name),
                        line: l,
                        column: 0,
                    })
                })
                .collect();
            let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).collect();
            if !errors.is_empty() || parsed.program.is_none() {
                for d in errors {
                    refused.push(diag(
                        location_in(rel, &lines, d.span.line),
                        WebDiagnosticKind::Other,
                        String::new(),
                        d.message.clone(),
                    ));
                }
                continue;
            }
            let mut loaded = LoadedForm {
                rel: rel.clone(),
                form,
                cfrm,
                program: parsed.program.expect("checked above"),
                lines,
                sites: Vec::new(),
            };
            for w in cobolt_compiler::link_common_code(&path, &mut loaded.program) {
                warnings.push(diag(
                    rel.clone(),
                    WebDiagnosticKind::Other,
                    String::new(),
                    w,
                ));
            }
            // WASM-005 T14: a class C control's method runs on the backend —
            // the call becomes `CALL … WITH WEBSOCKET LINKAGE` to the program
            // the Build generates for it, in the parsed program only.
            loaded.sites = cobolt_compiler::web_remote::class_c_call_sites(&loaded.program, &loaded.form);
            if let Err(errs) = cobolt_compiler::web_remote::lower_class_c_calls(&mut loaded.program, &loaded.form) {
                refused.extend(errs.into_iter().map(|e| {
                    diag(location_in(rel, &loaded.lines, e.line), WebDiagnosticKind::Other, String::new(), e.message)
                }));
                continue;
            }
            forms.push(loaded);
        }
        if !refused.is_empty() {
            return Err(WebBuildError::Refused(refused));
        }
        if forms.is_empty() {
            return Err(WebBuildError::Failed(
                "the project has no form to build".into(),
            ));
        }
        Ok(forms)
    }

    /// Steps 1–3 of the Build, shared with Check (WASM-002 R3, T14): read and
    /// regenerate the forms, then run the generator on every variant asked
    /// for. A construct a browser cannot run is a [`WebBuildError::Refused`]
    /// line at the developer's own handler line.
    fn compile(
        &self,
        req: &WebBuildRequest,
        warnings: &mut Vec<WebDiagnostic>,
    ) -> Result<(Vec<LoadedForm>, cobolt_wasm::WebModules), WebBuildError> {
        // WASM-017 R9, AC7: "multi-memory only" with WebKit in the matrix is
        // refused, never built without the module those browsers need.
        if req.kind == cobolt_compiler::ProjectKind::Web {
            if let Some(p) = req.web.variant_problem() {
                let manifest = req.manifest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                return Err(WebBuildError::Refused(vec![diag(
                    manifest,
                    WebDiagnosticKind::MultiMemoryOnlyWithWebKit,
                    p.browsers.join(", "),
                    p.to_string(),
                )]));
            }
        }
        let forms = self.load_forms(req, warnings)?;
        let specs = form_specs(&forms);
        // WASM-017 R8, D3: the project's payload limit is the arena's frame;
        // the batch and answer regions stay as ABI v1 §2 sizes them.
        let defaults = cobolt_wasm::Options::default();
        let opts = cobolt_wasm::Options {
            name: req.name.clone(),
            arena: cobolt_wasm::module::Arena { frame: req.web.max_payload_bytes, ..defaults.arena },
            ..defaults
        };
        let modules = cobolt_wasm::compile_web_project(
            &specs,
            &opts,
            req.variants.multi_memory,
            req.variants.single_memory,
        )
        .map_err(|e| {
            WebBuildError::Refused(
                e.all_kinded()
                    .into_iter()
                    .map(|d| {
                        diag(
                            location(d.form.and_then(|i| forms.get(i)), d.line),
                            kind_of(d.kind),
                            d.subject.clone(),
                            d.message.clone(),
                        )
                    })
                    .collect(),
            )
        })?;
        for w in &modules.warnings {
            // A warning carries its line but not its form; the main form's
            // lines are the first table, so it is named by line alone.
            warnings.push(diag(
                String::new(),
                warning_kind(w.kind),
                w.subject.clone(),
                format!("line {}: {}", w.line, w.message),
            ));
        }
        drop(specs);
        Ok((forms, modules))
    }
}

/// The backend programs of the forms' data controls (WASM-005 T9): a `READ`
/// per DataGrid bound to an indexed file or an SQL query, a `SAVE` when the
/// binding is writable; and one per method call on a class C control (T11).
/// A grid or a call the backend cannot serve refuses Check and Build alike,
/// at its form.
fn backend_programs(
    req: &WebBuildRequest,
    forms: &[LoadedForm],
) -> Result<cobolt_codegen::remote_controls::GeneratedBackend, WebBuildError> {
    let load = |path: &str| {
        let p = Path::new(path);
        let full = if p.is_absolute() { p.to_path_buf() } else { req.project_dir.join(p) };
        cobolt_indexed::load_indexed(&full).map_err(|e| format!("{e:?}"))
    };
    let mut all = cobolt_codegen::remote_controls::GeneratedBackend::default();
    let mut refused = Vec::new();
    for f in forms {
        match cobolt_codegen::remote_controls::generate_backend(std::slice::from_ref(&f.form), &load) {
            Ok(mut g) => all.programs.append(&mut g.programs),
            Err(errs) => {
                refused.extend(errs.into_iter().map(|e| diag(f.rel.clone(), WebDiagnosticKind::Other, String::new(), e)))
            }
        }
        // T11: a program per method call on a class C control, refused at
        // the developer's own handler line when the backend cannot serve it.
        match cobolt_codegen::remote_controls::generate_control_programs(&f.form, &f.sites) {
            Ok(mut programs) => all.programs.append(&mut programs),
            Err(errs) => refused.extend(
                errs.into_iter().map(|e| diag(location_in(&f.rel, &f.lines, e.line), WebDiagnosticKind::Other, String::new(), e.message)),
            ),
        }
        // T12: a FileDropZone's files and a Viewer's opened document go to an upload slot with its own limits.
        match cobolt_codegen::remote_controls::generate_slots(&f.form) {
            Ok(mut slots) => all.slots.append(&mut slots),
            Err(errs) => {
                refused.extend(errs.into_iter().map(|e| diag(f.rel.clone(), WebDiagnosticKind::Other, String::new(), e)))
            }
        }
    }
    if refused.is_empty() {
        Ok(all)
    } else {
        Err(WebBuildError::Refused(refused))
    }
}

/// Put the generated backend programs in `backend/generated/`, replacing what
/// an earlier Build left there. Only files a Build generated (they carry its
/// banner) are ever removed or overwritten; anything else in the folder is
/// the developer's and stays.
pub fn write_backend_programs(
    project_dir: &Path,
    generated: &cobolt_codegen::remote_controls::GeneratedBackend,
) -> std::io::Result<()> {
    use cobolt_codegen::remote_controls::{is_generated, EXPORTS_FILE, GENERATED_DIR};
    let dir = project_dir.join("backend").join(GENERATED_DIR);
    let mut wanted: Vec<(String, String)> =
        generated.programs.iter().map(|p| (p.file_name.clone(), p.source.clone())).collect();
    if !generated.is_empty() {
        wanted.push((EXPORTS_FILE.to_owned(), generated.exports_toml()));
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let ours = std::fs::read_to_string(&path).map(|t| is_generated(&t)).unwrap_or(false);
            if ours && !wanted.iter().any(|(n, _)| *n == name) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    for (name, text) in &wanted {
        let path = dir.join(name);
        if let Ok(old) = std::fs::read_to_string(&path) {
            if !is_generated(&old) {
                return Err(std::io::Error::other(format!(
                    "{} is not a generated file; move it out of backend/{GENERATED_DIR}/, which the Build owns",
                    path.display()
                )));
            }
            if old == *text {
                continue;
            }
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, text)?;
    }
    // An emptied folder the Build made goes with its last file.
    if generated.is_empty() && std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(&dir);
    }
    Ok(())
}

/// The form specs the generator takes: controls in the model's order, every
/// bound event, the layers above the base, the form's `Instances` (WASM-017
/// R2). The first form of the manifest is the main form.
fn form_specs(forms: &[LoadedForm]) -> Vec<FormSpec<'_>> {
    forms
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut controls = Vec::new();
            all_controls(&f.form.controls, &mut controls);
            let mut bindings: Vec<(String, String)> = Vec::new();
            for c in &controls {
                for e in &c.events {
                    bindings.push((c.id.clone(), e.event.clone()));
                }
            }
            for e in &f.form.form_events {
                if !matches!(e.event.as_str(), "onLoad" | "onClose") {
                    bindings.push((String::new(), e.event.clone()));
                }
            }
            FormSpec {
                name: f.form.name.clone(),
                main: i == 0,
                instances: f.form.effective_instances(i == 0),
                program: &f.program,
                controls: controls
                    .iter()
                    .map(|c| (c.id.clone(), format!("{:?}", c.control_type)))
                    .collect(),
                bindings,
                layers: f.form.layers.iter().map(|l| l.name.clone()).collect(),
                lines: f.lines.clone(),
            }
        })
        .collect()
}

/// How a missing player is reported; the IDE shows its own heading above a
/// line that starts with it (WASM-002 T21).
pub const PLAYER_UNAVAILABLE_PREFIX: &str = "the web player is not available";

/// The built player (WASM-002 T15): `PRC_WEB_PLAYER_DIR` when it is set, else
/// the bundled-asset pattern the IDE and `rcrun` use for their other assets —
/// `<exe dir>/assets/web-player` if it is a folder, else `assets/web-player` —
/// and, in a clone, the repository's `assets/web-player` (where `build_player`
/// commits it). Shipping it beside an installed IDE is WASM-016 R12's.
pub fn player_dir() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    player_dir_from(std::env::var_os(PLAYER_DIR_ENV).map(PathBuf::from), exe_dir)
}

/// [`player_dir`] with its inputs given: the variable's value and the
/// executable's folder. `Err` names every folder looked at, or the folder
/// whose `player-build.json` is missing.
pub fn player_dir_from(env: Option<PathBuf>, exe_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = match env {
        // Named by the developer: that folder or nothing.
        Some(dir) => vec![dir],
        None => exe_dir
            .map(|d| d.join("assets/web-player"))
            .into_iter()
            .chain([
                PathBuf::from("assets/web-player"),
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/web-player"),
            ])
            .collect(),
    };
    let Some(dir) = candidates.iter().find(|d| d.is_dir()) else {
        let tried: Vec<String> = candidates.iter().map(|d| d.display().to_string()).collect();
        return Err(format!(
            "{PLAYER_UNAVAILABLE_PREFIX}: no folder {} (set {PLAYER_DIR_ENV} to the folder build_player wrote)",
            tried.join(", ")
        ));
    };
    let info = dir.join("player-build.json");
    if !info.is_file() {
        return Err(format!(
            "{PLAYER_UNAVAILABLE_PREFIX}: {} is missing",
            info.display()
        ));
    }
    Ok(dir.clone())
}

/// The sizes `build_player` recorded for the player in `dir` (WASM-016 R13):
/// each variant at the record's chosen `opt-level`, then each part
/// (`parts/<name>`), in bytes after `wasm-opt`. Empty when the record says none.
pub fn player_sizes(dir: &Path) -> Vec<PartSize> {
    let Some(rec) = std::fs::read_to_string(dir.join("player-build.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        return Vec::new();
    };
    let level = rec["profile"]["opt_level"].as_str().unwrap_or("");
    let mut out = Vec::new();
    for m in rec["measurements"].as_array().into_iter().flatten() {
        if m["opt_level"].as_str() == Some(level) {
            if let (Some(name), Some(bytes)) = (m["variant"].as_str(), m["after_wasm_opt_bytes"].as_u64()) {
                out.push(PartSize { name: name.to_owned(), bytes });
            }
        }
    }
    for p in rec["parts"].as_array().into_iter().flatten() {
        if let (Some(name), Some(bytes)) = (p["name"].as_str(), p["after_wasm_opt_bytes"].as_u64()) {
            out.push(PartSize { name: format!("parts/{name}"), bytes });
        }
    }
    out
}

/// One player the IDE keeps: the current one, or an older one under
/// `archive/<version>/` that a project may pin (WASM-016 R11, R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptPlayer {
    pub version: String,
    /// The host-API version it gives a program module.
    pub abi: u32,
    /// The form-description format it reads.
    pub description: u32,
    pub dir: PathBuf,
}

impl KeptPlayer {
    fn read(dir: &Path) -> Option<KeptPlayer> {
        let text = std::fs::read_to_string(dir.join("player-build.json")).ok()?;
        let rec: serde_json::Value = serde_json::from_str(&text).ok()?;
        let v = &rec["versions"];
        Some(KeptPlayer {
            version: v["player"].as_str()?.to_owned(),
            abi: v["abi"].as_u64()? as u32,
            description: v["description"].as_u64()? as u32,
            dir: dir.to_path_buf(),
        })
    }

    /// Can this compiler build for it? It emits one host API and one
    /// description format, the current ones.
    pub fn buildable(&self) -> bool {
        self.abi == cobolt_wasm_abi::ABI_VERSION && self.description == cobolt_web_bundle::DESCRIPTION_VERSION
    }
}

/// The players kept in `dir` (the current player's folder): the current one
/// first, then those of `archive/`, oldest first.
pub fn kept_players(dir: &Path) -> Vec<KeptPlayer> {
    let mut out: Vec<KeptPlayer> = KeptPlayer::read(dir).into_iter().collect();
    let mut old: Vec<KeptPlayer> = std::fs::read_dir(dir.join("archive"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| KeptPlayer::read(&e.path()))
        .collect();
    old.sort_by(|a, b| version_order(&a.version, &b.version));
    out.extend(old);
    out
}

fn version_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

/// The player a Build copies (WASM-016 R11, Q6): the current one in `dir`
/// unless the project pins another; a pin must name a kept player this
/// compiler can still build for.
pub fn select_player(dir: &Path, pin: Option<&str>) -> Result<PathBuf, PlayerPinError> {
    let Some(pinned) = pin.map(str::trim).filter(|p| !p.is_empty()) else {
        return Ok(dir.to_path_buf());
    };
    let kept = kept_players(dir);
    let Some(p) = kept.iter().find(|k| k.version == pinned) else {
        let mut versions: Vec<String> = kept.iter().map(|k| k.version.clone()).collect();
        versions.sort_by(|a, b| version_order(a, b));
        return Err(PlayerPinError::NotKept { pinned: pinned.to_owned(), kept: versions });
    };
    if !p.buildable() {
        let oldest = kept.iter().filter(|k| k.buildable()).min_by(|a, b| version_order(&a.version, &b.version)).map(|k| k.version.clone());
        return Err(PlayerPinError::CannotEmit { pinned: pinned.to_owned(), abi: p.abi, description: p.description, oldest });
    }
    Ok(p.dir.clone())
}

fn files_under(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(base, &p, out);
            } else if let Ok(b) = std::fs::read(&p) {
                out.push((
                    p.strip_prefix(base)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/"),
                    b,
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

fn load_player(dir: &Path) -> PlayerInput {
    let variant = |name: &str| {
        let d = dir.join(name);
        d.is_dir().then(|| PlayerVariantInput {
            files: files_under(&d),
            entry_js: "player.js".into(),
            entry_wasm: "player_bg.wasm".into(),
        })
    };
    let mut parts = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir.join("parts")) {
        let mut names: Vec<_> = rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            let files = files_under(&dir.join("parts").join(&name));
            parts.push(PlayerPartInput {
                entry_js: format!("{name}.js"),
                entry_wasm: format!("{name}_bg.wasm"),
                name,
                files,
            });
        }
    }
    PlayerInput {
        plain: variant("plain"),
        threaded: variant("threaded"),
        parts,
        build_info: std::fs::read_to_string(dir.join("player-build.json")).ok(),
        components: Vec::new(),
    }
}

/// Write `files` into `out` so that the folder is replaced in one step: a new
/// folder beside it, renamed over the old one (which is removed after). A
/// failure leaves the previous bundle as it was.
fn replace_dir(out: &Path, files: &[cobolt_web_bundle::BundleFile]) -> std::io::Result<()> {
    let parent = out.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "web".into());
    let fresh = parent.join(format!(".{name}.new-{}", std::process::id()));
    let old = parent.join(format!(".{name}.old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&fresh);
    for f in files {
        let p = fresh.join(&f.path);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, &f.bytes)?;
    }
    let had_old = out.exists();
    if had_old {
        std::fs::rename(out, &old)?;
    }
    if let Err(e) = std::fs::rename(&fresh, out) {
        if had_old {
            let _ = std::fs::rename(&old, out);
        }
        return Err(e);
    }
    if had_old {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

/// `pr_init` + `pr_open_form(main)` of `wasm`, in milliseconds.
/// `wss://host[:port]` (or `ws://…`) of the project's `backend_url`, which is a
/// WebSocket address. `cobolt_web_bundle::origin_of` answers http(s) only, so a
/// `wss://` backend went missing from the page's `connect-src` (WASM-003 R23).
/// Any other scheme is passed on whole, for the bundle to refuse by name.
fn backend_origin(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let Some((scheme, rest)) = url.split_once("://") else { return Some(url.to_owned()) };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() || !(scheme.eq_ignore_ascii_case("wss") || scheme.eq_ignore_ascii_case("ws")) {
        return Some(url.to_owned());
    }
    Some(format!("{}://{host}", scheme.to_ascii_lowercase()))
}

/// The `connect-src` directive of a policy, as written (`connect-src 'self' wss://…`).
fn connect_src(policy: &str) -> String {
    policy.split(';').map(str::trim).find(|d| d.starts_with("connect-src")).unwrap_or_default().to_owned()
}

fn measure_startup(wasm: &[u8]) -> Option<f64> {
    let t = Instant::now();
    let mut host = cobolt_wasm::host::FormHost::new(wasm).ok()?;
    let (inst, _) = host.open_form(0, -1);
    (inst >= 0).then(|| t.elapsed().as_secs_f64() * 1000.0)
}

impl WebBuildBackend for GeneratorBackend {
    fn name(&self) -> &str {
        "cobolt-wasm"
    }

    fn check(&self, req: &WebBuildRequest) -> Result<(), WebBuildError> {
        // The same steps as the Build up to the generator; the bytes are
        // discarded, nothing is written (WASM-002 R3: Check and Build agree).
        let (forms, _) = self.compile(req, &mut Vec::new())?;
        backend_programs(req, &forms).map(|_| ())
    }

    fn build(&self, req: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
        let started = Instant::now();
        let mut warnings: Vec<WebDiagnostic> = Vec::new();
        let (forms, modules) = self.compile(req, &mut warnings)?;
        // WASM-005 T9: the backend end of the forms' data controls, regenerated
        // on every Build like the forms' own programs.
        let backend = backend_programs(req, &forms)?;
        write_backend_programs(&req.project_dir, &backend).map_err(|e| {
            WebBuildError::Failed(format!("cannot write backend/{}: {e}", cobolt_codegen::remote_controls::GENERATED_DIR))
        })?;
        let player = match &req.player_dir {
            Some(dir) => player_dir_from(Some(dir.clone()), None),
            None => player_dir(),
        }
        .map_err(WebBuildError::Failed)?;
        let player = select_player(&player, req.web.player.as_deref()).map_err(WebBuildError::PlayerPin)?;
        let mut menus = Vec::new();
        for f in &forms {
            let dir = req.project_dir.join(&f.rel);
            let dir = dir.parent().unwrap_or(&req.project_dir);
            let mut controls = Vec::new();
            all_controls(&f.form.controls, &mut controls);
            for c in controls {
                let p = cobolt_forms::menu::menu_yaml_path(dir, &c.id);
                if let Ok(yaml) = std::fs::read_to_string(&p) {
                    menus.push(MenuInput {
                        form: f.form.name.clone(),
                        control: c.id.clone(),
                        yaml,
                    });
                }
            }
        }
        let assets = req
            .assets
            .iter()
            .filter_map(|rel| {
                std::fs::read(req.project_dir.join(rel))
                    .ok()
                    .map(|bytes| AssetInput {
                        path: rel.replace('\\', "/"),
                        bytes,
                    })
            })
            .collect();
        use cobolt_compiler::project_kind::{ApplicationStyle, Isolation};
        let options = BundleOptions {
            style: match req.web.style {
                ApplicationStyle::Spa => AppStyle::Spa,
                ApplicationStyle::Standalone => AppStyle::Standalone,
            },
            title: req.name.clone(),
            isolation: match req.web.isolation {
                Isolation::Off => cobolt_web_bundle::Isolation::Off,
                Isolation::RequireCorp => cobolt_web_bundle::Isolation::RequireCorp,
                Isolation::Credentialless => cobolt_web_bundle::Isolation::Credentialless,
            },
            backend_origins: backend_origin(&req.web.backend_url).into_iter().collect(),
            max_frame: Some(req.web.max_payload_bytes),
            // A see-through theme (Spatial) needs something behind it: the same
            // stand-in for a desktop its pictures are drawn over.
            backdrop: {
                let main = forms.iter().find(|f| f.form.main_form).or(forms.first());
                let theme = cobolt_forms::theme::resolve_theme_id(
                    main.and_then(|f| f.form.theme.as_deref()),
                    req.form_theme.as_deref(),
                );
                cobolt_forms::theme::is_see_through(&theme).then(|| {
                    let (r, g, b) = cobolt_forms::theme::SEE_THROUGH_BACKDROP_RGB;
                    format!("#{r:02X}{g:02X}{b:02X}")
                })
            },
            ..BundleOptions::default()
        };
        let program_multi = modules.multi.as_ref().map(|c| c.wasm.clone());
        let program_single = modules.single.as_ref().map(|c| c.wasm.clone());
        let input = BundleInput {
            project: req.name.clone(),
            program_multi,
            program_single,
            forms: forms
                .iter()
                .map(|f| FormInput {
                    name: f.form.name.clone(),
                    cfrm: f.cfrm.clone(),
                    stylesheet: None,
                })
                .collect(),
            stylesheet: None,
            menus,
            assets,
            player: load_player(&player),
            options,
        };
        let bundle = cobolt_web_bundle::assemble_bundle(&input)
            .map_err(|e| WebBuildError::Failed(e.to_string()))?;
        for w in &bundle.report.warnings {
            warnings.push(WebDiagnostic::from(w.clone()));
        }
        replace_dir(&req.out_dir, &bundle.files).map_err(|e| {
            WebBuildError::Failed(format!("cannot write {}: {e}", req.out_dir.display()))
        })?;
        let startup_ms = modules
            .multi
            .as_ref()
            .or(modules.single.as_ref())
            .and_then(|c| measure_startup(&c.wasm));
        Ok(BuildReport {
            bundle_dir: req.out_dir.clone(),
            size: BundleSize::of_dir(&req.out_dir),
            startup_ms,
            warnings,
            duration_ms: started.elapsed().as_millis() as u64,
            connect_src: connect_src(&bundle.report.csp_meta),
            player: player_sizes(&player),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kept player: a folder with the record a `build_player` writes (only what the pin reads).
    fn kept(dir: &Path, version: &str, abi: u32, description: u32) {
        std::fs::create_dir_all(dir).unwrap();
        let rec = serde_json::json!({"versions": {"abi": abi, "description": description, "player": version}});
        std::fs::write(dir.join("player-build.json"), rec.to_string()).unwrap();
    }

    /// WASM-016 R11, Q6: no pin is the current player; a pin names a kept one this compiler can build for;
    /// a pin to a player not kept lists the kept ones; a pin to one the compiler can no longer emit for
    /// names it and the oldest it can.
    #[test]
    fn a_pinned_player_is_chosen_from_the_kept_ones_or_refused_with_the_reason() {
        let root = std::env::temp_dir().join(format!("pr-player-pin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cur = root.join("web-player");
        let (abi, fmt) = (cobolt_wasm_abi::ABI_VERSION, cobolt_web_bundle::DESCRIPTION_VERSION);
        kept(&cur, "2.2.0", abi, fmt);
        kept(&cur.join("archive/2.1.0"), "2.1.0", abi, fmt);
        kept(&cur.join("archive/2.0.0"), "2.0.0", abi, fmt);
        kept(&cur.join("archive/1.9.9"), "1.9.9", abi + 1, fmt);

        assert_eq!(select_player(&cur, None).unwrap(), cur, "no pin: the current player");
        assert_eq!(select_player(&cur, Some("  ")).unwrap(), cur, "an empty pin is no pin");
        assert_eq!(select_player(&cur, Some("2.2.0")).unwrap(), cur, "pinned to the current one");
        assert_eq!(select_player(&cur, Some("2.0.0")).unwrap(), cur.join("archive/2.0.0"));

        let absent = select_player(&cur, Some("2.0.5")).unwrap_err();
        assert_eq!(absent, PlayerPinError::NotKept { pinned: "2.0.5".into(), kept: vec!["1.9.9".into(), "2.0.0".into(), "2.1.0".into(), "2.2.0".into()] });
        assert!(absent.to_string().contains("2.0.5") && absent.to_string().contains("1.9.9, 2.0.0, 2.1.0, 2.2.0"), "{absent}");

        let old = select_player(&cur, Some("1.9.9")).unwrap_err();
        assert_eq!(old, PlayerPinError::CannotEmit { pinned: "1.9.9".into(), abi: abi + 1, description: fmt, oldest: Some("2.0.0".into()) });
        let text = WebBuildError::PlayerPin(old).to_string();
        assert!(text.contains("pins player 1.9.9") && text.contains("oldest player it can build for is 2.0.0"), "{text}");
        println!("{absent}\n{text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// WASM-005 T9: Build owns `backend/generated/` — it writes the programs
    /// and their exports, removes a program it no longer generates, and never
    /// touches (or overwrites) a file it did not write.
    #[test]
    fn the_build_regenerates_backend_generated_and_leaves_the_developers_files() {
        use cobolt_codegen::remote_controls::{fixtures, generate_backend, GeneratedBackend};
        let root = std::env::temp_dir().join(format!("pr-backend-gen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("backend").join("generated");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("NOTES.txt"), "mine").unwrap();
        let load = |_: &str| Ok(fixtures::customers_cidx("data/customers.idx"));
        let g = generate_backend(&fixtures::grid_forms(), &load).unwrap();

        write_backend_programs(&root, &g).unwrap();
        let mut names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), g.programs.len() + 2, "{names:?}");
        assert!(names.contains(&"exports.toml".to_string()) && names.contains(&"NOTES.txt".to_string()));

        // One grid less: its programs go, everything else stays.
        let fewer = GeneratedBackend { programs: g.programs[..2].to_vec(), ..Default::default() };
        write_backend_programs(&root, &fewer).unwrap();
        assert!(!dir.join(&g.programs[4].file_name).exists(), "a program no longer generated is removed");
        assert!(dir.join(&g.programs[0].file_name).exists());
        assert_eq!(std::fs::read_to_string(dir.join("NOTES.txt")).unwrap(), "mine");

        // A file of the developer's under a generated name is never overwritten.
        std::fs::write(dir.join(&g.programs[1].file_name), "hand written").unwrap();
        let err = write_backend_programs(&root, &fewer).unwrap_err();
        assert!(err.to_string().contains("is not a generated file"), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join(&g.programs[1].file_name)).unwrap(), "hand written");

        // Nothing generated: the generated files go, the developer's stay.
        std::fs::remove_file(dir.join(&g.programs[1].file_name)).unwrap();
        write_backend_programs(&root, &GeneratedBackend::default()).unwrap();
        let left: Vec<String> =
            std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["NOTES.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_backend_origin_keeps_the_websocket_scheme() {
        assert_eq!(backend_origin("wss://api.example.com/ws"), Some("wss://api.example.com".into()));
        assert_eq!(backend_origin("WSS://api.example.com:8443"), Some("wss://api.example.com:8443".into()));
        assert_eq!(backend_origin("ws://127.0.0.1:8443/ws"), Some("ws://127.0.0.1:8443".into()));
        assert_eq!(backend_origin("  "), None);
        // Not a WebSocket address: passed on whole, and the bundle refuses it by name.
        assert_eq!(backend_origin("https://api.example.com"), Some("https://api.example.com".into()));
        assert_eq!(connect_src("default-src 'none'; connect-src 'self' wss://a.b; object-src 'none'"), "connect-src 'self' wss://a.b");
    }
}
