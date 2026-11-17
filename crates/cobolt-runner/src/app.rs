// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A form application, run by the runner** (DESKTOP-WASM-001 T30).
//!
//! The glue of a program built on the WebAssembly path, as the generated `main.rs`'s
//! `run_form_app` is the glue of one built on the Cargo path. It publishes the same settings,
//! from the module's `prsettings` section instead of baked constants. It resolves the same themes,
//! from the module's `prart` section instead of embedded tables. It registers the same menus, from
//! `prmenus`, and builds the same form host config. Only the program side differs: the forms'
//! programs are the module's, run by [`WasmEngine`]. A scripted run (`PRC_HEADLESS_SCRIPT`,
//! `PRC_HEADLESS_LIMIT`, `PRC_HEADLESS_SHOTS`) is driven exactly as a Cargo-built program's is.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};

use cobolt_form_host::engine::{ProgramEngine, ProgramRun};
use cobolt_form_host::state::CtrlState;
use cobolt_program_package::sections::{DesktopSections, ThemeArt};
use cobolt_runtime::{FormEvent, StateUpdate};
use cobolt_wasm_abi::manifest::Manifest;

use crate::forms::WasmEngine;
use crate::{Lang, Msg, Pins};

/// The runner's per-host part of the form host: the module's programs.
struct RunnerHooks {
    engine: Option<WasmEngine>,
}

impl cobolt_form_host::HostHooks for RunnerHooks {
    fn program_engine(&mut self) -> Option<Box<dyn ProgramEngine>> {
        self.engine.take().map(|e| Box::new(e) as Box<dyn ProgramEngine>)
    }
}

/// The asset-pack theme a form is painted with (`form ?? project ?? Liquid Glass`), built from the
/// art the module carries. `None` means Liquid Glass, whether the form asked for it or the pack
/// could not be built.
fn theme_pack(art: &[ThemeArt], project_theme: &str, form: &cobolt_forms::Form) -> Option<Arc<cobolt_forms::theme_pack::ThemePack>> {
    let id = cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), Some(project_theme));
    let entry = art.iter().find(|t| t.id == id)?;
    let files: Vec<(&str, &[u8])> = entry.files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect();
    match cobolt_forms::theme_pack::ThemePack::from_embedded(&entry.manifest, &files) {
        Ok(pack) => Some(Arc::new(pack)),
        Err(e) => {
            eprintln!("theme pack '{id}' unusable, falling back to Liquid Glass: {e}");
            None
        }
    }
}

/// The procedural look a form is painted in: the pack's when it owns the whole look, else the theme id's.
fn surface_theme(
    pack: Option<&Arc<cobolt_forms::theme_pack::ThemePack>>,
    project_theme: &str,
    form: &cobolt_forms::Form,
) -> Arc<dyn cobolt_forms::surface_theme::SurfaceTheme> {
    match pack {
        Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
        None => cobolt_forms::surface_theme::for_theme_id(&cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), Some(project_theme))),
    }
}

/// Anchor project-relative asset paths as a Cargo-built program does: the folder above the
/// executable that has `assets/` (the `dist/` bundle, or the project above `bin/`).
fn anchor_assets() {
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        let anchor = std::iter::successors(Some(exe_dir.as_path()), |p| p.parent())
            .find(|d| d.join("assets").is_dir())
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| exe_dir.clone());
        cobolt_forms::assets::set_base(anchor);
    }
}

/// The forms' designs, the main form first, each with its ItemsFile read as it opens.
fn designs(sections: &DesktopSections) -> Result<Vec<cobolt_forms::Form>, Msg> {
    sections
        .forms
        .iter()
        .map(|d| {
            let mut form = cobolt_forms::description::to_form(d).map_err(|e| Msg::Damaged(format!("the program's `prforms` section: {e}")))?;
            cobolt_forms::items_file::apply(&mut form.controls);
            Ok(form)
        })
        .collect()
}

/// Run the form application `module` carries, with the sections it was built with. Answers the
/// process's exit code.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    abi: u32,
    imports: Vec<(String, String)>,
    manifest: Manifest,
    sections: DesktopSections,
    files: crate::files::FileSetup,
    sql: Option<crate::sql::SqlSetup>,
    lang: Lang,
    pins: Pins,
) -> Result<i32, Msg> {
    anchor_assets();
    let s = &sections.settings;
    let forms = designs(&sections)?;
    let Some(first_form) = forms.first().cloned() else {
        return Err(Msg::Damaged("the program's `prforms` section has no form".into()));
    };
    if !s.main_form.trim().is_empty() && !first_form.name.trim().eq_ignore_ascii_case(s.main_form.trim()) {
        return Err(Msg::Damaged(format!(
            "the program starts at form {}, but its forms begin with {}",
            s.main_form.trim(),
            first_form.name.trim()
        )));
    }
    // A designer launch (`COBOLT_DESIGNER_FORM`: the IDE's Run of one form, a scripted run of one
    // form) starts at another of the application's own forms, with that form's program, as a
    // Cargo-built program does; a name the program does not carry is refused.
    let first_form = match cobolt_runtime::form_host::designer_form() {
        Some(want) => match forms.iter().find(|f| f.name.trim().eq_ignore_ascii_case(&want)) {
            Some(f) => f.clone(),
            None => return Err(Msg::NoSuchForm { want, forms: forms.iter().map(|f| f.name.trim().to_ascii_uppercase()).collect() }),
        },
        None => first_form,
    };

    // The settings every form of the run shares, published as a Cargo-built program publishes them.
    cobolt_form_host::seeding::publish_agent_connections_from_env();
    let connections = cobolt_forms::connections::Catalogue::from_json(&s.connections_json);
    cobolt_form_host::seeding::publish_connections(connections.rest.clone());
    cobolt_form_host::seeding::publish_search_connections(connections.search.clone());
    cobolt_form_host::seeding::publish_aws_connections(connections.aws.clone());
    cobolt_runtime::mcp_tool::publish_file_memory_limit(s.file_memory_limit_mb * 1024 * 1024);
    cobolt_runtime::publish_format_locale(&s.locale);
    cobolt_runtime::publish_format_currency(&s.currency);
    cobolt_form_host::seeding::publish_link_schemes(s.link_schemes.clone());
    cobolt_forms::render::set_focus_ring(cobolt_forms::render::FocusRing::from_settings(s.focus_ring.enabled, &s.focus_ring.color, s.focus_ring.pulse));
    cobolt_forms::paint::register_menus(
        sections.menus.iter().filter_map(|m| cobolt_forms::menu::parse_menu(&m.yaml).ok().map(|def| (m.key.clone(), def))),
    );

    // The window's effects: the project's × the form's own opt-out × the machine's kill switch.
    let fx_on = first_form.window_effects && !cobolt_form_host::diagnostics::env_flag("PRC_NO_WINDOW_FX");
    let (fx_entrance, fx_exit, fx_restore) = if fx_on {
        (
            cobolt_forms::window_fx::FxSpec::parse(&s.entrance_effect),
            cobolt_forms::window_fx::FxSpec::parse(&s.exit_effect),
            s.entrance_on_restore,
        )
    } else {
        (Default::default(), Default::default(), false)
    };
    let project_theme = s.theme.clone();
    let pack = theme_pack(&sections.art, &project_theme, &first_form);
    let surface = surface_theme(pack.as_ref(), &project_theme, &first_form);

    // A SideMenu on the main form puts the application in shell mode, as on the Cargo path.
    let shell_mode = first_form.has_side_menu();
    let root_menu = if shell_mode {
        first_form.side_menu_control_id().and_then(|ctrl_id| {
            cobolt_forms::paint::registered_menu_for(&first_form.name, &ctrl_id).map(|def| (ctrl_id, (*def).clone()))
        })
    } else {
        None
    };

    let engine = WasmEngine::start(engine, module, abi, imports, manifest, forms, files, sql, lang, pins)?;

    // The main form's instance: the same channels a Cargo-built program's interpreter is given.
    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    cobolt_form_host::flatten_controls(&first_form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);
    let state: HashMap<String, CtrlState> = cobolt_form_host::state::initial_state(&flat, &first_form.layers);
    let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let (form_req_tx, form_req_rx) = mpsc::channel::<cobolt_runtime::form_host::FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();
    let pending = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let form_object = first_form.name.trim().to_ascii_uppercase();
    let (_, start) = engine.open(&first_form.name).map_err(Msg::CannotStart)?;
    start(ProgramRun {
        handle: cobolt_runtime::form_host::ROOT_HANDLE.to_string(),
        form_object,
        control_ids: first_form.controls.iter().map(|c| c.id.clone()).collect(),
        events: ev_rx,
        state: state_tx,
        display: display_tx,
        input: input_rx,
        closed: closed_rx,
        requests: form_req_tx.clone(),
        pending: Arc::clone(&pending),
        finished: Arc::clone(&finished),
        super_handle: None,
        seed: Vec::new(),
        // The main form's SideMenus' designed rows, from the menus registered above, as the form
        // host hands every other form's (spec 066).
        designed_menus: first_form
            .controls
            .iter()
            .filter(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
            .filter_map(|c| cobolt_forms::paint::registered_menu_for(&first_form.name, &c.id).map(|d| (c.id.clone(), (*d).clone())))
            .collect(),
        debug: None,
    })
    .map_err(Msg::CannotStart)?;

    let exit = engine.clone();
    // The project's engine, else COBOL_INDEXED_ENGINE, else the default: the Cargo path's order.
    let indexed_engine = crate::files::indexed_engine(&s.indexed_engine);
    let art = Arc::new(sections.art.clone());
    let child_project_theme = project_theme.clone();
    let child_theme: cobolt_form_host::ChildThemeSource = Box::new(move |child| {
        let pack = theme_pack(&art, &child_project_theme, child);
        let st = surface_theme(pack.as_ref(), &child_project_theme, child);
        (pack, st)
    });
    let config = cobolt_form_host::FormHostConfig {
        form: first_form,
        flat,
        state,
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        pending,
        finished,
        form_req_rx,
        closed_tx,
        form_req_tx,
        form_source: None,
        child_theme: Some(child_theme),
        child_interpreter_setup: None,
        indexed_engine,
        shared_rust_bridge: None,
        fx_entrance,
        fx_exit,
        fx_restore,
        theme_pack: pack,
        surface_theme: surface,
        icon_path: None,
        title_fallback: format!("{} v{}", s.app_name, s.version),
        surface: cobolt_form_host::Surface::Window,
        hooks: Box::new(RunnerHooks { engine: Some(engine) }),
    };
    if let Ok(script) = std::env::var("PRC_HEADLESS_SCRIPT") {
        let limit = std::env::var("PRC_HEADLESS_LIMIT").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(60);
        let shots = std::env::var_os("PRC_HEADLESS_SHOTS").map(std::path::PathBuf::from);
        cobolt_form_host::headless::run_script_file(config, shell_mode.then_some(root_menu), std::path::Path::new(&script), limit, shots.as_deref());
        return Ok(0);
    }
    if shell_mode {
        cobolt_form_host::shell::run_shell(config, root_menu);
    } else {
        cobolt_form_host::run(config);
    }
    Ok(exit.exit_code().unwrap_or(0))
}
