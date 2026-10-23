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
    BuildReport, BundleSize, WebBuildBackend, WebBuildError, WebBuildRequest, WebDiagnostic,
    WebDiagnosticKind,
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
            };
            for w in cobolt_compiler::link_common_code(&path, &mut loaded.program) {
                warnings.push(diag(
                    rel.clone(),
                    WebDiagnosticKind::Other,
                    String::new(),
                    w,
                ));
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
        let forms = self.load_forms(req, warnings)?;
        let specs = form_specs(&forms);
        let opts = cobolt_wasm::Options {
            name: req.name.clone(),
            ..cobolt_wasm::Options::default()
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

/// The form specs the generator takes: controls in the model's order, every
/// bound event, the layers above the base. The first form of the manifest is
/// the main form.
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
                instances: 1,
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
    // The project's pin of an older player (WASM-016 R11) is not a manifest
    // field yet; until it is, the current player is the one copied.
    Ok(dir.clone())
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
        self.compile(req, &mut Vec::new()).map(|_| ())
    }

    fn build(&self, req: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
        let started = Instant::now();
        let mut warnings: Vec<WebDiagnostic> = Vec::new();
        let (forms, modules) = self.compile(req, &mut warnings)?;
        let player = player_dir().map_err(WebBuildError::Failed)?;
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
