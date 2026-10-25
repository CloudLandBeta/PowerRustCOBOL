// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A new project, exactly as the IDE's New Project makes it** (spec 084 R15,
//! WASM-001 R10).
//!
//! The IDE's New Project and a coding agent's `create_project` (in the IDE's
//! tool server and in `rcrun mcp`) write the same project: the folders, the
//! starter files and the manifest all come from here, for every project kind.
//! The manifest is [`NEW_PROJECT_TEMPLATE`], which is the IDE's own
//! `CoboltProject::new` + `save_project` output with the name and main as
//! placeholders; the kind-specific parts — `kind`, the file lists, the `[web]`,
//! `[mobile]`, `[backend]` and `[desktop]` tables — are added to it in the same
//! shapes the IDE's serializer writes. An IDE test renders every creatable
//! template the IDE's way and requires the two to be byte-identical, so they
//! cannot drift (regenerate the Desktop template with `COBOLT_WRITE_GOLDEN=1`).
//!
//! A creation is two steps so a caller can look before it writes: [`plan`]
//! checks the request against the template catalogue and says what would be
//! made; [`Plan::write_into`] makes it.

use std::path::Path;

use cobolt_compiler::project_kind::{
    DesktopSettings, MobileSettings, ProjectKind, WebSettings, BACKEND_FOLDER,
};
use serde_json::{json, Value};

pub use crate::templates::{CreateRequest, DEFAULT_TEMPLATE};
use crate::templates::{self, Resolved, TemplateDescriptor};

/// Standard project sub-folders of a Desktop project — one per category plus
/// working/build folders. Created when a project is made, and back-filled (if
/// missing) when one is opened.
pub const PROJECT_FOLDERS: &[&str] = &[
    "src",
    "forms",
    "indexed",
    "generated",
    "Assets",
    "assets",
    "Knowledge Base",
    "bin",
    "debug",
    "temp",
    "dist",
    "data",
    "COPYBOOKS",
];

/// The sub-folders of a project of `kind` (WASM-001 Q8): a Web or Mobile
/// project keeps its forms in `webforms/` or `mobileforms/`, builds into
/// `dist/` (no `bin/`), and — when `backend` — has the Backend's `backend/src`
/// and `backend/COPYBOOKS`.
pub fn folders_for(kind: ProjectKind, backend: bool) -> Vec<String> {
    if kind.is_desktop() {
        return PROJECT_FOLDERS.iter().map(|s| (*s).to_owned()).collect();
    }
    let mut folders: Vec<String> = PROJECT_FOLDERS
        .iter()
        .filter(|f| **f != "forms" && **f != "bin")
        .map(|s| (*s).to_owned())
        .collect();
    folders.insert(1, kind.forms_folder().to_owned());
    if backend {
        folders.push(BACKEND_FOLDER.to_owned());
        folders.push(format!("{BACKEND_FOLDER}/src"));
        folders.push(format!("{BACKEND_FOLDER}/COPYBOOKS"));
    }
    folders
}

/// The main program a new Desktop project starts with, relative to the project.
pub const DEFAULT_MAIN: &str = "src/main.cbl";

/// The name of the main form of a Web or Mobile project, and of its file.
pub const MAIN_FORM_NAME: &str = "MAIN-FORM";
/// The first program of a Backend: a health check, the one every Backend exports.
pub const BACKEND_STARTER: &str = "backend/src/health.cbl";
/// The list of the programs a remote caller can reach — the only one (WASM-003 D5).
pub const BACKEND_EXPORTS: &str = "backend/exports.toml";
/// The Backend's deployment: address, TLS, identity provider, limits.
pub const BACKEND_CONFIG: &str = "backend/backend.toml";

/// The manifest New Project writes, with `"@@PROJECT-NAME@@"` and
/// `"@@PROJECT-MAIN@@"` where the name and the main program go.
pub const NEW_PROJECT_TEMPLATE: &str = include_str!("new_project.toml");

/// Placeholders inside [`NEW_PROJECT_TEMPLATE`] (each a whole TOML string).
pub const NAME_PLACEHOLDER: &str = "@@PROJECT-NAME@@";
pub const MAIN_PLACEHOLDER: &str = "@@PROJECT-MAIN@@";

/// A file stem safe on every file system: path and reserved characters become
/// `-`, and an empty result is `project`.
pub fn sanitize_file_stem(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}

/// The project name as the IDE stores it: the developer's name with the
/// `.project` suffix, e.g. `Inventory.project`.
pub fn project_name(name: &str) -> String {
    let name = name.trim();
    if name.ends_with(".project") {
        name.to_owned()
    } else {
        format!("{name}.project")
    }
}

/// The starter main program: runnable as is, so a new project can be Run.
pub fn starter_main(project_name: &str, main_rel: &str) -> String {
    let prog: String = Path::new(main_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("MAIN")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '-' })
        .collect();
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. {prog}.\n\
         \x20     *> {project_name} — main program.\n\
         \n\
         \x20      PROCEDURE DIVISION.\n\
         \x20          DISPLAY \"Hello from {project_name}\".\n\
         \x20          GOBACK.\n"
    )
}

/// The first program of a Backend: `HEALTH`, which answers `OK`. A remote
/// caller reaches only the programs `backend/exports.toml` names, and this is
/// the one a new Backend names.
pub fn starter_backend(project_name: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. HEALTH.\n\
         \x20     *> {project_name} — Backend health check. A caller that is allowed to ask\n\
         \x20     *> gets OK back; it is the first program in backend/exports.toml.\n\
         \n\
         \x20      DATA DIVISION.\n\
         \n\
         \x20      LINKAGE SECTION.\n\
         \x20      01 LK-STATUS                              PIC    X(8).\n\
         \n\
         \x20      PROCEDURE DIVISION USING LK-STATUS.\n\
         \x20          MOVE    \"OK\"                          TO    LK-STATUS\n\
         \x20          GOBACK.\n"
    )
}

/// The starter `backend/exports.toml`: `HEALTH`, public, so a new Backend
/// answers its health check before anyone has signed in.
pub fn starter_backend_exports(project_name: &str) -> String {
    format!(
        "# {project_name} — the programs of the Backend a remote caller can reach.\n\
         # A program that is not listed here cannot be called, whatever its name.\n\
         # Each [[export]] may also say: roles = [\"admin\"], idempotent, independent,\n\
         # long_running, stream, directions = [\"in\", \"out\"].\n\
         \n\
         [contract]\n\
         version = \"1.0.0\"\n\
         \n\
         [[export]]\n\
         program = \"HEALTH\"\n\
         public = true\n"
    )
}

/// The starter `backend/backend.toml`: a development deployment — loopback,
/// no TLS, no identity provider, no secret and no `unsafe_*` setting. A
/// release start asks for the identity provider and the frontend's origins.
pub fn starter_backend_config(project_name: &str) -> String {
    format!(
        "# {project_name} — where and how the Backend runs.\n\
         # Plain ws:// is served on a loopback address only. For a network address,\n\
         # add a [tls] section, or terminate TLS in a reverse proxy in front of it.\n\
         # A secret is never written here by value: use \"env:NAME\" or \"file:PATH\".\n\
         \n\
         [server]\n\
         listen = \"127.0.0.1:8443\"\n\
         \n\
         # Before a release, name the frontend's origins and the identity provider:\n\
         # [server]  allowed_origins = [\"https://app.example.com\"]\n\
         # [auth]    issuer = [\"https://login.example.com\"], audiences = [\"api\"]\n"
    )
}

/// The device sizes the starter form is laid out for.
fn form_size(kind: ProjectKind, mobile: Option<(cobolt_compiler::project_kind::MobilePlatform, cobolt_compiler::project_kind::DeviceClass)>) -> (u32, u32) {
    use cobolt_compiler::project_kind::DeviceClass;
    match (kind, mobile.map(|m| m.1)) {
        (ProjectKind::Mobile, Some(DeviceClass::Tablet)) => (820, 1180),
        (ProjectKind::Mobile, _) => (390, 844),
        _ => (960, 640),
    }
}

/// The handler of the sample form's button: greets the name in the box.
const GREET_HANDLER: &str = "       ENVIRONMENT DIVISION.\n\
\n\
       DATA DIVISION.\n\
\n\
       WORKING-STORAGE SECTION.\n\
\n\
       LINKAGE SECTION.\n\
\n\
       PROCEDURE DIVISION.\n\
\n\
           *> Greets the person whose name is in the box. An empty box asks for a name\n\
           *> instead, so the first click already shows both ways through the handler.\n\
\n\
           IF    TXT-NAME::Text::Trim()::Len() = 0\n\
               SET     LBL-RESULT::Caption       TO    \"Type your name first.\"\n\
           ELSE\n\
               MOVE    SPACES                    TO    LBL-RESULT::Caption\n\
               STRING    \"Hello, \" TXT-NAME::Text::Trim() DELIMITED BY SIZE INTO LBL-RESULT::Caption\n\
           END-IF.\n";

/// Where the designer drops a SideMenu (the rail overhangs the window's left edge by 8) and how wide it is: it
/// ends at 256, and a Web project's sample sits to the right of that.
const SHELL_RAIL_X: i32 = -8;
const SHELL_RAIL_W: i32 = 264;
/// The id the designer gives a form's first SideMenu; its rows live in `<id>.menu.yaml` beside the form.
const SHELL_RAIL_ID: &str = "SideMenu-1";

/// The sample main form of a Web or Mobile project: a title, a box, a button
/// whose click handler greets the name typed, and the answer. Responsive, the
/// main form, wearing Spatial when the project does. A Web project's is an
/// application shell: a SideMenu rail (and its footer) on the left, the sample
/// to the right of it.
pub fn starter_form(
    project_name: &str,
    kind: ProjectKind,
    mobile: Option<(cobolt_compiler::project_kind::MobilePlatform, cobolt_compiler::project_kind::DeviceClass)>,
    spatial: bool,
) -> String {
    use cobolt_forms::{Control, ControlType, EventBinding, Form};
    let (w, h) = form_size(kind, mobile);
    let shown = project_name.strip_suffix(".project").unwrap_or(project_name);
    let mut form = Form::new(MAIN_FORM_NAME, shown, w, h);
    form.main_form = true;
    form.responsive = true;
    form.breakpoints = cobolt_forms::layout::breakpoints::from_text(
        &cobolt_forms::layout::breakpoints::to_text(&cobolt_forms::layout::defaults::default_breakpoints()),
        &form.breakpoints,
    );

    // A Web project opens as an application shell: a SideMenu rail on the left, the sample to the right of it.
    let shell = kind == ProjectKind::Web;
    let left = if shell { SHELL_RAIL_X + SHELL_RAIL_W + 16 } else { 24 };
    let inner = w as i32 - left - 24;
    let label = |id: &str, caption: String, y: i32, h: i32, size: i32| {
        let mut c = Control::new(id, ControlType::Label, left, y);
        c.rect.w = inner;
        c.rect.h = h;
        c.set_prop("Caption", caption);
        c.set_prop("FontSize", size);
        c.set_prop("Anchor", "Top,Left,Right");
        c
    };
    let title = label("LBL-TITLE", format!("Welcome to {shown}"), 24, 32, 24);
    let hint = label("LBL-HINT", "Type your name and press Greet.".to_owned(), 64, 24, 14);
    let result = label("LBL-RESULT", String::new(), 224, 32, 16);

    let mut name = Control::new("TXT-NAME", ControlType::TextBox, left, 104);
    name.rect.w = inner;
    name.rect.h = 40;
    name.set_prop("FontSize", "14");
    name.set_prop("Anchor", "Top,Left,Right");

    let mut greet = Control::new("BTN-GREET", ControlType::Button, left, 160);
    greet.rect.w = 112;
    greet.rect.h = 40;
    greet.set_prop("Caption", "Greet");
    greet.set_prop("FontSize", "14");
    let mut on_click = EventBinding::for_control("BTN-GREET", "onClick");
    on_click.code = GREET_HANDLER.to_owned();
    greet.events.push(on_click);

    form.controls = vec![title, hint, name, greet, result];
    if shell {
        // Dropped as the designer drops it: at x = -8, 264 wide, the height of the form; its footer panel follows.
        let mut rail = Control::new(SHELL_RAIL_ID, ControlType::SideMenu, SHELL_RAIL_X, 0);
        rail.rect.w = SHELL_RAIL_W;
        rail.rect.h = h as i32;
        form.controls.push(rail);
        form.sync_side_menu_footer_panels();
    }

    if spatial {
        form.apply_spatial_defaults();
        // Every text the operator reads is white on Spatial's dark glass, and a
        // data-entry box has its own dark face (the golden rules).
        for c in &mut form.controls {
            c.set_prop("ForegroundColor", "#FFFFFFFF");
            if c.control_type == ControlType::TextBox {
                c.set_prop("BackgroundColor", "#12131CFF");
                c.set_prop("BorderStyle", "Single");
                c.set_prop("BorderColor", "#FFFFFF33");
                c.set_prop("CornerRadius", "12");
            }
        }
    } else {
        // The IDE's own new form on the default theme: the glass shows through.
        form.background_color = "00000000".into();
    }
    cobolt_forms::form_to_string(&form).unwrap_or_default()
}

/// The rail's sidecar: one row, "Home", sealed as the designer seals a menu (`menu::save_menu` writes the
/// hash, and only writes to a file, so it goes through a temporary one).
fn starter_menu() -> String {
    use cobolt_forms::menu::{save_menu, MenuDefinition, MenuItem};
    let def = MenuDefinition { menu: vec![MenuItem::new_action("home", "Home")], hash: String::new() };
    let path = std::env::temp_dir().join(format!("prc-starter-menu-{}-{:?}.yaml", std::process::id(), std::thread::current().id()));
    let sealed = save_menu(&path, &def).ok().and_then(|_| std::fs::read_to_string(&path).ok()).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    sealed
}

/// [`NEW_PROJECT_TEMPLATE`] with the name and main filled in, TOML-escaped.
pub fn render_manifest(project_name: &str, main_rel: &str) -> String {
    let quoted = |s: &str| toml_edit::Value::from(s).to_string().trim().to_owned();
    NEW_PROJECT_TEMPLATE
        .replace(&format!("\"{NAME_PLACEHOLDER}\""), &quoted(project_name))
        .replace(&format!("\"{MAIN_PLACEHOLDER}\""), &quoted(main_rel))
}

/// The theme an application made by a coding agent wears (the golden rules):
/// Spatial, for the IDE and as every form's default.
pub const SPATIAL: &str = "spatial";

/// A file a creation writes, if it is not there already.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Project-relative, `/` separators.
    pub rel: String,
    pub contents: String,
}

/// What a creation would make: checked, nothing written yet.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The name as stored, e.g. `Inventory.project`.
    pub stored_name: String,
    /// The manifest's suggested file name, e.g. `Inventory.project.toml`.
    pub manifest_name: String,
    /// The manifest text, before the main-form designation is sealed (the seal
    /// reads the form files, so it is written after them).
    pub manifest: String,
    pub kind: ProjectKind,
    pub template: &'static str,
    /// `[project] main`.
    pub main: String,
    pub folders: Vec<String>,
    pub files: Vec<PlannedFile>,
    /// The forms the manifest lists.
    pub forms: Vec<String>,
    /// The Backend files the manifest lists.
    pub backend: Vec<String>,
    /// The IDE theme id the project starts in; empty is the IDE's default.
    pub theme: String,
}

/// Is `path` one the manifest may hold: project-relative, nothing that climbs
/// out of the project (the New Project dialog asks before it plans)?
pub fn is_project_relative(path: &str) -> bool {
    safe_relative(path).is_ok()
}

/// A path the manifest may hold: project-relative, forward slashes, nothing
/// that climbs out of the project.
fn safe_relative(path: &str) -> Result<String, String> {
    let p = path.trim().replace('\\', "/");
    let bad = p.is_empty()
        || p.starts_with('/')
        || p.contains(':')
        || p.split('/').any(|seg| seg == ".." || seg.is_empty());
    if bad {
        return Err(format!("'{path}' is not a project-relative path"));
    }
    Ok(p)
}

/// Check `request` against `catalogue` and say what creating it would make.
pub fn plan(request: &CreateRequest, catalogue: &[TemplateDescriptor]) -> Result<Plan, String> {
    if request.name.trim().is_empty() {
        return Err("a project needs a 'name'".to_owned());
    }
    let resolved = templates::resolve(request, catalogue)?;
    let t = &resolved.template;
    let kind = t.kind;
    let stored = project_name(&request.name);
    let manifest_name = format!("{}.toml", sanitize_file_stem(&stored));
    let sample = resolved.flag("sample");
    let backend = kind.has_backend() && resolved.flag("backend");

    // The theme: an agent's project wears Spatial, and so does a Web project whose dialog picked none (a page
    // has no desktop behind it, and Spatial is the look the Web player is built to match); otherwise the pick.
    let theme = if request.spatial || (kind == ProjectKind::Web && request.theme.trim().is_empty()) { SPATIAL.to_owned() } else { request.theme.trim().to_owned() };
    let forms_theme = cobolt_forms::theme::ThemeCatalog::builtin().get(&theme).is_some();
    let wears_spatial = theme == SPATIAL;

    let mut files = Vec::new();
    let mut forms = Vec::new();
    let mut backend_files = Vec::new();
    let main = if kind.is_desktop() {
        let main = if request.main.trim().is_empty() { DEFAULT_MAIN.to_owned() } else { safe_relative(&request.main)? };
        if sample {
            files.push(PlannedFile { rel: main.clone(), contents: starter_main(&stored, &main) });
        }
        main
    } else {
        if !request.main.trim().is_empty() {
            return Err(format!(
                "a {} project's entry is its main form, {}/{}.{}; 'main' is for a Desktop project",
                kind.as_str(),
                kind.forms_folder(),
                MAIN_FORM_NAME.to_lowercase(),
                kind.form_extension()
            ));
        }
        if sample {
            let rel = format!("{}/{}.{}", kind.forms_folder(), MAIN_FORM_NAME.to_lowercase(), kind.form_extension());
            files.push(PlannedFile { rel: rel.clone(), contents: starter_form(&stored, kind, t.mobile, wears_spatial) });
            if kind == ProjectKind::Web {
                files.push(PlannedFile { rel: format!("{}/{SHELL_RAIL_ID}.menu.yaml", kind.forms_folder()), contents: starter_menu() });
            }
            forms.push(rel.clone());
            rel
        } else {
            String::new()
        }
    };
    if backend {
        files.push(PlannedFile { rel: BACKEND_STARTER.to_owned(), contents: starter_backend(&stored) });
        backend_files.push(BACKEND_STARTER.to_owned());
        // The exports and the deployment are the Backend's settings, not COBOL:
        // written once, never listed in `[files] backend`.
        files.push(PlannedFile { rel: BACKEND_EXPORTS.to_owned(), contents: starter_backend_exports(&stored) });
        files.push(PlannedFile { rel: BACKEND_CONFIG.to_owned(), contents: starter_backend_config(&stored) });
    }

    let manifest = build_manifest(
        request,
        &resolved,
        &stored,
        &main,
        &theme,
        forms_theme,
        &forms,
        &backend_files,
        backend,
    )?;
    Ok(Plan {
        stored_name: stored,
        manifest_name,
        manifest,
        kind,
        template: t.id,
        main,
        folders: folders_for(kind, backend),
        files,
        forms,
        backend: backend_files,
        theme,
    })
}

// One wrapper per table, so the serializer writes `[web]`, `[web.browsers]` … in the
// order the structs declare their fields — the order the IDE's own save writes
// them. (A `toml::Value` table would sort the keys.)
#[derive(serde::Serialize)]
struct DesktopTable<'a> {
    desktop: &'a DesktopSettings,
}
#[derive(serde::Serialize)]
struct WebTable<'a> {
    web: &'a WebSettings,
}
#[derive(serde::Serialize)]
struct MobileTable<'a> {
    mobile: &'a MobileSettings,
}

fn table_text<T: serde::Serialize>(table: &T) -> Result<String, String> {
    toml::to_string_pretty(table).map_err(|e| format!("cannot write the project's settings: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn build_manifest(
    request: &CreateRequest,
    resolved: &Resolved,
    stored: &str,
    main: &str,
    theme: &str,
    forms_theme: bool,
    forms: &[String],
    backend_files: &[String],
    backend: bool,
) -> Result<String, String> {
    use toml_edit::{value, Array, DocumentMut, Item};
    let t = &resolved.template;
    let kind = t.kind;
    let mut doc: DocumentMut = render_manifest(stored, main)
        .parse()
        .map_err(|e| format!("the manifest template does not parse: {e}"))?;

    // [project] — kind, description, locale (the order of `ProjectMeta`).
    {
        let project = doc["project"].as_table_mut().ok_or("the manifest template has no [project]")?;
        if !kind.is_desktop() {
            project.insert("kind", value(kind.as_str()));
        }
        let description = request.description.split_whitespace().collect::<Vec<_>>().join(" ");
        if !description.is_empty() {
            project.insert("description", value(description));
        }
        let locale = request.locale.trim();
        if !locale.is_empty() {
            project.insert("locale", value(locale));
        }
        let version = request.version_or_default();
        if version != "1.0.0" {
            project.insert("version", value(version));
        }
    }

    // [files] — the lists the kind starts with.
    if !kind.is_desktop() || !resolved.flag("sample") {
        let files = doc["files"].as_table_mut().ok_or("the manifest template has no [files]")?;
        // A Web or Mobile project has no starter program in Common Code; a
        // Desktop one without the sample has none either.
        files.insert("sources", value(Array::new()));
        let mut list = Array::new();
        for f in forms {
            list.push(f.as_str());
        }
        files.insert("forms", value(list));
    }
    if backend {
        let files = doc["files"].as_table_mut().ok_or("the manifest template has no [files]")?;
        let mut list = Array::new();
        for f in backend_files {
            list.push(f.as_str());
        }
        files.insert("backend", value(list));
    }

    // The look: the IDE theme, and the default form theme when the form
    // catalogue has one of that name.
    if !theme.is_empty() {
        doc["ide"]["theme"] = value(theme);
        if forms_theme {
            doc["forms"]["theme"] = value(theme);
        }
    }

    let mut out = doc.to_string();
    // The tables of the kind, after `[integrations]`, in the order `CoboltProject` declares them.
    let mut tables: Vec<String> = Vec::new();
    if kind.is_desktop() {
        let desktop = DesktopSettings {
            window_style: resolved.choice("window_style").to_owned(),
            ui_framework: resolved.choice("ui_framework").to_owned(),
            high_dpi: resolved.flag("high_dpi"),
            gpu_acceleration: resolved.flag("gpu_acceleration"),
        };
        if desktop != DesktopSettings::default() {
            tables.push(table_text(&DesktopTable { desktop: &desktop })?);
        }
    }
    match (kind, t.mobile) {
        (ProjectKind::Web, _) => {
            let mut web = WebSettings::default();
            web.style = cobolt_compiler::project_kind::ApplicationStyle::parse(resolved.choice("style")).unwrap_or_default();
            tables.push(table_text(&WebTable { web: &web })?);
        }
        (ProjectKind::Mobile, Some((platform, device))) => {
            let mut mobile = MobileSettings::new(platform, device);
            mobile.style = cobolt_compiler::project_kind::ApplicationStyle::parse(resolved.choice("style")).unwrap_or_default();
            mobile.orientation = cobolt_compiler::project_kind::Orientation::parse(resolved.choice("orientation")).unwrap_or_default();
            mobile.min_os = resolved.choice("min_os").to_owned();
            tables.push(table_text(&MobileTable { mobile: &mobile })?);
        }
        _ => {}
    }
    for table in tables {
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&table);
    }
    Ok(out)
}

impl Plan {
    /// Write the project into `dir`: the folders, the starter files that are
    /// not there already (an existing file is never overwritten), the manifest
    /// as `manifest_file`, and then the main-form designation sealed into it.
    pub fn write_into(&self, dir: &Path, manifest_file: &str) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create the folder: {e}"))?;
        for sub in &self.folders {
            std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("cannot create {sub}/: {e}"))?;
        }
        for file in &self.files {
            let path = dir.join(&file.rel);
            if path.exists() {
                continue;
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}/: {e}", parent.display()))?;
            }
            std::fs::write(&path, &file.contents).map_err(|e| format!("cannot write {}: {e}", file.rel))?;
        }
        let manifest_path = dir.join(manifest_file);
        std::fs::write(&manifest_path, &self.manifest).map_err(|e| format!("cannot write the manifest: {e}"))?;
        // The seal reads the form files, so it goes in after them — the same
        // re-seal an IDE save performs.
        if let Some(form) = self.forms.first() {
            crate::host::record_in_manifest(&manifest_path, form, crate::host::FileList::Forms)?;
        }
        Ok(())
    }
}

/// What a copy of a project made.
#[derive(Debug, Clone)]
pub struct CloneDone {
    /// Files copied (the manifest not counted).
    pub files: usize,
    pub bytes: u64,
    /// The new project's manifest.
    pub manifest: std::path::PathBuf,
}

/// Folders a copy leaves behind: build output and working folders, version
/// control and downloaded dependencies. What the developer wrote is what is copied.
const NOT_COPIED: &[&str] = &["bin", "debug", "temp", "dist", "target", ".git", "node_modules"];

/// *Create from Template…* (WASM-001 R7): copy the project whose manifest is
/// `source` into `dest_dir` as a new project named `stored_name`.
///
/// Everything the developer wrote is copied — forms, sources, indexed
/// definitions, assets, documents, the Backend — and nothing a build left
/// (`bin/`, `dist/`, `temp/`, `debug/`, `target/`). The manifest is the source's
/// with the new name and no recorded build, written as `<name>.toml`; the
/// main-form designation is sealed again, because the seal covers the name.
/// Symbolic links are not followed or copied. `dest_dir` must be new or empty
/// and must not lie inside the source (or the source inside it).
pub fn clone_project(source: &Path, dest_dir: &Path, stored_name: &str) -> Result<CloneDone, String> {
    let source_manifest = if source.is_dir() {
        crate::root::ProjectRoot::open(source).map_err(|_| "there is no project there".to_owned())?.manifest().to_path_buf()
    } else {
        source.to_path_buf()
    };
    cobolt_compiler::project_manifest_view(&source_manifest)?;
    let source_dir = source_manifest.parent().ok_or("the project has no folder")?;
    let source_dir = std::fs::canonicalize(source_dir).map_err(|e| format!("cannot read the project's folder: {e}"))?;
    if dest_dir.exists() {
        if !dest_dir.is_dir() {
            return Err("the destination is not a folder".to_owned());
        }
        if std::fs::read_dir(dest_dir).map_err(|e| format!("cannot read the destination: {e}"))?.next().is_some() {
            return Err("the destination folder is not empty".to_owned());
        }
    }
    let made_here = !dest_dir.exists();
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("cannot create the destination: {e}"))?;
    let dest_canonical = std::fs::canonicalize(dest_dir).map_err(|e| format!("cannot read the destination: {e}"))?;
    if dest_canonical.starts_with(&source_dir) || source_dir.starts_with(&dest_canonical) {
        // Leave nothing behind: the folder that was only just made for this
        // is taken away again (it is empty).
        if made_here {
            let _ = std::fs::remove_dir(dest_dir);
        }
        return Err("the copy cannot be inside the project it copies, nor the project inside the copy".to_owned());
    }

    fn walk(
        src: &Path,
        dst: &Path,
        top: bool,
        skip_manifest: &Path,
        files: &mut usize,
        bytes: &mut u64,
    ) -> Result<(), String> {
        std::fs::create_dir_all(dst).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
        let entries = std::fs::read_dir(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            let from = entry.path();
            let to = dst.join(&name);
            if kind.is_dir() {
                if top && NOT_COPIED.iter().any(|n| n.eq_ignore_ascii_case(&name_text)) {
                    continue;
                }
                if name_text == ".git" || name_text == "node_modules" || name_text == "target" {
                    continue;
                }
                walk(&from, &to, false, skip_manifest, files, bytes)?;
            } else if kind.is_file() {
                if from == skip_manifest {
                    continue;
                }
                let n = std::fs::copy(&from, &to).map_err(|e| format!("cannot copy {}: {e}", from.display()))?;
                *files += 1;
                *bytes += n;
            }
        }
        Ok(())
    }
    let skip = std::fs::canonicalize(&source_manifest).unwrap_or(source_manifest.clone());
    let (mut files, mut bytes) = (0usize, 0u64);
    walk(&source_dir, &dest_canonical, true, &skip, &mut files, &mut bytes)?;

    // The manifest: the source's, renamed, with no recorded build.
    let text = std::fs::read_to_string(&source_manifest).map_err(|e| format!("cannot read the manifest: {e}"))?;
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| format!("the manifest cannot be parsed: {e}"))?;
    doc["project"]["name"] = toml_edit::value(stored_name);
    if doc["project"].get("built_with_version").is_some() {
        doc["project"]["built_with_version"] = toml_edit::value("");
    }
    let manifest = dest_dir.join(format!("{}.toml", sanitize_file_stem(stored_name)));
    std::fs::write(&manifest, doc.to_string()).map_err(|e| format!("cannot write the manifest: {e}"))?;
    // The seal covers the project's name.
    let forms: Vec<String> = doc
        .get("files")
        .and_then(|f| f.get("forms"))
        .and_then(|i| i.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    if let Some(first) = forms.first() {
        crate::host::record_in_manifest(&manifest, first, crate::host::FileList::Forms)?;
    }
    Ok(CloneDone { files, bytes, manifest })
}

/// Create a project named `name` in `folder` the way a coding agent does
/// (spec 084 R15, R18): a Desktop project in Spatial — the manifest, every
/// standard folder and the starter main program. Refuses a folder that already
/// holds anything, and never overwrites.
pub fn create_project(folder: &Path, name: &str) -> Result<Value, String> {
    let mut request = CreateRequest::new(name);
    request.spatial = true;
    create_project_with(folder, &request)
}

/// Create the project `request` describes in `folder` (WASM-001 R10): any
/// template of the built-in catalogue, with its options. Refuses a folder that
/// already holds anything, and never overwrites.
pub fn create_project_with(folder: &Path, request: &CreateRequest) -> Result<Value, String> {
    if request.name.trim().is_empty() {
        return Err("create_project needs a 'name'".to_owned());
    }
    let plan = plan(request, &templates::builtin())?;
    if folder.exists() {
        let has_entries = std::fs::read_dir(folder)
            .map_err(|e| format!("cannot read the folder: {e}"))?
            .next()
            .is_some();
        if has_entries {
            return Err("that folder already holds files; create_project needs an empty or new folder".to_owned());
        }
    }
    plan.write_into(folder, &plan.manifest_name)?;
    Ok(json!({
        "created": true,
        "project": plan.manifest_name,
        "name": plan.stored_name,
        "kind": plan.kind.as_str(),
        "template": plan.template,
        "main": plan.main,
        "folders": plan.folders,
        "forms": plan.forms,
        "backend": plan.backend,
        "theme": if plan.theme.is_empty() { Value::Null } else { Value::String(plan.theme.clone()) },
    }))
}

#[cfg(test)]
mod form_extension_tests {
    use super::*;

    /// WASM-001 Q2: each kind's entry form carries the kind's own extension.
    #[test]
    fn create_plans_the_entry_form_with_the_kinds_extension() {
        let catalogue = templates::builtin();
        for (template, expected) in [
            ("web", "webforms/main-form.wcfrm"),
            ("android-phone", "mobileforms/main-form.mcfrm"),
            ("ipad", "mobileforms/main-form.mcfrm"),
        ] {
            let p = plan(&CreateRequest::new("Shop").template(template), &catalogue).unwrap();
            println!("{template:<14} entry form {}", p.main);
            assert_eq!(p.main, expected);
            assert_eq!(p.forms, [expected]);
            assert!(p.files.iter().any(|f| f.rel == expected), "{template}: the form file is planned");
        }
        let desktop = plan(&CreateRequest::new("Shop"), &catalogue).unwrap();
        assert!(desktop.forms.is_empty() && desktop.main == DEFAULT_MAIN, "Desktop is unchanged");
        let mut wrong = CreateRequest::new("Shop").template("web");
        wrong.main = "src/x.cbl".into();
        let e = plan(&wrong, &catalogue).unwrap_err();
        assert!(e.contains("webforms/main-form.wcfrm"), "{e}");
    }
}
